//! Port of python/tests/test_abi.py: the mirrored C structs have the C
//! compiler's size and field offsets, read from the manifest that
//! tests/bvnr_abi_dump.c prints.
//!
//! Only `ValueTypeSpec` is a public mirror; every other struct lives in the
//! private `ffi` module (Python's `structs.py`), out of reach of an
//! integration test. Those are checked field by field against the same
//! manifest by the `abi_tests` unit test in src/ffi.rs.
//!
//! Skipped:
//! - test_all_ctypes_structures_are_covered: scans loaded Python modules for
//!   ctypes Structures; Rust has no runtime reflection. The `abi_tests` unit
//!   test instead asserts that every struct and field in the manifest is
//!   mirrored.

use std::collections::HashMap;
use std::mem::{offset_of, size_of};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use bovnar::ValueTypeSpec;

/// (sizes, fields) from the C compiler, the source of truth.
type Manifest = (
    HashMap<String, usize>,
    HashMap<String, Vec<(String, usize, usize)>>,
);

/// Built once: both tests share the compiled manifest generator.
fn c_manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(build_manifest)
}

fn build_manifest() -> Manifest {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let exe = Path::new(env!("CARGO_TARGET_TMPDIR")).join("test_abi_dump");
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let status = Command::new(cc)
        .arg("-std=c99")
        .arg("-I")
        .arg(repo.join("include"))
        .arg(repo.join("tests/bvnr_abi_dump.c"))
        .arg("-o")
        .arg(&exe)
        .status()
        .expect("run the C compiler");
    assert!(status.success(), "compiling bvnr_abi_dump.c failed");
    let out = Command::new(&exe).output().expect("run bvnr_abi_dump");
    let mut sizes = HashMap::new();
    let mut fields: HashMap<String, Vec<_>> = HashMap::new();
    for line in String::from_utf8(out.stdout).unwrap().lines() {
        match line.split_whitespace().collect::<Vec<_>>().as_slice() {
            ["STRUCT", s, size] => {
                sizes.insert(s.to_string(), size.parse().unwrap());
            }
            ["FIELD", s, f, off, size] => fields.entry(s.to_string()).or_default().push((
                f.to_string(),
                off.parse().unwrap(),
                size.parse().unwrap(),
            )),
            _ => {}
        }
    }
    (sizes, fields)
}

#[test]
fn test_struct_size_matches_c() {
    let (sizes, _) = c_manifest();
    let c = sizes
        .get("value_type_spec_t")
        .expect("value_type_spec_t missing from the C ABI manifest");
    assert_eq!(
        size_of::<ValueTypeSpec>(),
        *c,
        "ValueTypeSpec is out of sync with C value_type_spec_t"
    );
}

#[test]
fn test_field_offsets_match_c() {
    let (_, fields) = c_manifest();
    let c = &fields["value_type_spec_t"];
    let spec = ValueTypeSpec::default();
    let rust = [
        (
            "family",
            offset_of!(ValueTypeSpec, family),
            size_of_val(&spec.family),
        ),
        (
            "width",
            offset_of!(ValueTypeSpec, width),
            size_of_val(&spec.width),
        ),
        (
            "base",
            offset_of!(ValueTypeSpec, base),
            size_of_val(&spec.base),
        ),
    ];
    assert_eq!(
        c.len(),
        rust.len(),
        "fields added/removed without updating the mirror"
    );
    // Compared in declaration order: (offset, size), not C type identity.
    for ((name, off, size), (cname, coff, csize)) in rust.iter().zip(c) {
        assert_eq!(
            (off, size),
            (coff, csize),
            "ValueTypeSpec.{name} vs C {cname}"
        );
    }
}
