//! Builds the upstream C library the same way CMakeLists.txt does: run the
//! table generators into OUT_DIR, then compile the library sources with `cc`.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The four generators CMake runs, in CMake's order.
const GENERATORS: [&str; 4] = [
    "gen_units.py",
    "gen_prefixes.py",
    "gen_currencies.py",
    "gen_profiles.py",
];

/// BVNR_SOURCES from CMakeLists.txt (lexer, validator, writer, io, dom,
/// stream, utils).
const SOURCES: [&str; 18] = [
    "src/lexer/bovnar_lexer.c",
    "src/lexer/bovnar_state_table.c",
    "src/validator/bovnar_validator.c",
    "src/writer/bovnar_writer.c",
    "src/writer/bovnar_write_utils.c",
    "src/writer/bovnar_canon_observer.c",
    "src/io/bovnar_io.c",
    "src/dom/bovnar_dom.c",
    "src/dom/bovnar_dom_builder.c",
    "src/stream/bovnar_stream.c",
    "src/utils/bovnar_utils.c",
    "src/utils/bovnar_si_units.c",
    "src/utils/bovnar_currency.c",
    "src/utils/bovnar_profiles.c",
    "src/utils/bvn_int.c",
    "src/utils/bvn_float.c",
    "src/utils/bvn_gregorian_date.c",
    "src/utils/bvn_datetime.c",
];

/// The profile switches, passed as 0/1 like CMake does: bvn_profile_impl.h
/// defaults each to 1 with #ifndef and tests it with #if.
const PROFILES: [(&str, u8); 7] = [
    ("BVNR_WITH_UCUM_PROFILE", 1),
    ("BVNR_WITH_UNECE_PROFILE", 1),
    ("BVNR_WITH_QUDT_PROFILE", 1),
    ("BVNR_WITH_QUDT_QK_PROFILE", 1),
    ("BVNR_WITH_UDUNITS_PROFILE", 1),
    ("BVNR_WITH_OM_PROFILE", 1),
    ("BVNR_WITH_CF_PROFILE", 1),
];

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo = manifest
        .parent()
        .expect("rust/ lives inside the bovnar repository")
        .to_path_buf();
    let gen_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("generated");
    std::fs::create_dir_all(&gen_dir).expect("create OUT_DIR/generated");

    println!("cargo:rerun-if-changed={}", repo.join("src").display());
    println!("cargo:rerun-if-changed={}", repo.join("include").display());
    println!(
        "cargo:rerun-if-changed={}",
        repo.join("bvnr_data.py").display()
    );
    for script in GENERATORS {
        println!("cargo:rerun-if-changed={}", repo.join(script).display());
    }
    println!("cargo:rerun-if-env-changed=WASI_SDK_PATH");

    generate_tables(&repo, &gen_dir);
    compile_library(&repo, &gen_dir);
}

fn generate_tables(repo: &Path, gen_dir: &Path) {
    let probe = Command::new("python3").arg("--version").output();
    if !matches!(probe, Ok(ref out) if out.status.success()) {
        panic!(
            "python3 was not found. Building bovnar runs the upstream table \
             generators (gen_units.py and friends), which need Python 3 on PATH."
        );
    }
    for script in GENERATORS {
        let status = Command::new("python3")
            .arg(repo.join(script))
            .current_dir(repo)
            .env("BVNR_GENERATED_DIR", gen_dir)
            // Keep the source tree free of __pycache__ from the build.
            .env("PYTHONDONTWRITEBYTECODE", "1")
            .status()
            .unwrap_or_else(|e| panic!("failed to run python3 {script}: {e}"));
        if !status.success() {
            panic!(
                "python3 {script} failed ({status}); the unit/prefix/currency tables were not generated"
            );
        }
    }
}

fn compile_library(repo: &Path, gen_dir: &Path) {
    let target = env::var("TARGET").expect("TARGET");
    let mut build = cc::Build::new();

    if target == "wasm32-wasip2" {
        let Some(sdk) = env::var_os("WASI_SDK_PATH").map(PathBuf::from) else {
            panic!(
                "building for wasm32-wasip2 needs a wasi-sdk: set WASI_SDK_PATH to \
                 its root (the directory containing bin/clang and share/wasi-sysroot)"
            );
        };
        build
            .compiler(sdk.join("bin/clang"))
            .archiver(sdk.join("bin/llvm-ar"))
            .flag(format!(
                "--sysroot={}",
                sdk.join("share/wasi-sysroot").display()
            ));
    }

    build
        .include(repo.join("include"))
        .include(gen_dir)
        .include(repo.join("src"));
    for sub in [
        "dom",
        "io",
        "lexer",
        "stream",
        "utils",
        "validator",
        "writer",
    ] {
        build.include(repo.join("src").join(sub));
    }
    for source in SOURCES {
        build.file(repo.join(source));
    }
    for (name, on) in PROFILES {
        build.define(name, on.to_string().as_str());
    }

    if build.get_compiler().is_like_msvc() {
        build
            .flag("/std:c11")
            .flag("/utf-8")
            .define("_CRT_SECURE_NO_WARNINGS", None)
            .define("_CRT_NONSTDC_NO_WARNINGS", None);
    } else {
        // -fno-strict-aliasing: the source/sink structs are opaque storage the
        // library reinterprets through an internal impl struct (see CMake).
        build
            .flag("-std=c99")
            .flag("-fno-strict-aliasing")
            .flag("-fvisibility=hidden");
        if target.contains("windows-gnu") {
            build
                .define("__USE_MINGW_ANSI_STDIO", "1")
                .flag("-fno-builtin-printf")
                .flag("-fno-builtin-fprintf")
                .flag("-fno-builtin-snprintf");
        }
    }

    // CMake's Release configurations add -DNDEBUG; Debug keeps the asserts.
    if env::var("PROFILE").as_deref() == Ok("release") {
        build.define("NDEBUG", None);
    }

    build.compile("bvnr");
}
