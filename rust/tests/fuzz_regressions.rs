//! Replays the inputs in `fuzz/regressions/<target>/` through the same
//! checks the cargo-fuzz targets run, so a fixed fuzz finding stays fixed
//! without a nightly toolchain.

#[path = "../fuzz/src/checks.rs"]
mod checks;

use std::fs;
use std::path::Path;

fn replay(target: &str, check: fn(&[u8])) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fuzz/regressions")
        .join(target);
    let mut entries: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| {
            !path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect();
    entries.sort();
    for path in entries {
        let data = fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        eprintln!("replaying {}", path.display());
        check(&data);
    }
}

#[test]
fn reader() {
    replay("reader", checks::reader);
}

#[test]
fn dom() {
    replay("dom", checks::dom);
}

#[test]
fn roundtrip() {
    replay("roundtrip", checks::roundtrip);
}

#[test]
fn writer() {
    replay("writer", checks::writer);
}

#[test]
fn units() {
    replay("units", checks::units);
}

#[test]
fn json() {
    replay("json", checks::json);
}

#[test]
fn detect() {
    replay("detect", checks::detect);
}
