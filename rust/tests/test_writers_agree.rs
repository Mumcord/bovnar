//! The C writer (the CLI's `pretty-print`) and this crate's `to_string`
//! write documents that read back alike, for every catalogue unit in every
//! unit-carrying shape.
//!
//! The sweep compares against the CLI built at `<repo>/build/bovnar`, so it is
//! `#[ignore]`d by default and fails, rather than passing silently, when run
//! without it. CI builds the CLI and runs it on Linux:
//!
//! ```sh
//! cmake -B build -DBVNR_BUILD_TESTS=OFF . && cmake --build build --target bovnar
//! cd rust && cargo test --test test_writers_agree -- --ignored
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

use bovnar::{Value, parse_typed, to_string};

/// One per type family that can carry a unit, plus the utf8 inline suffix
/// that was the defect and the quoted non-decimal number sharing its shape.
const SHAPES: [(&str, &str); 8] = [
    ("<float:64,", "> 1.5"),
    ("<float:32,", "> 1.5"),
    ("<uint:64,", "> 7"),
    ("<sint:64,", "> -7"),
    ("<float_dec:64,", "> 1.5"),
    ("<float_fix:32,q8,", "> 1.5"),
    ("<uint:8,_16,", "> \"FF\""),
    ("<utf8> \"x\" ", ""),
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Every `.symbol = "..."` of the unit catalogue.
fn catalogue() -> std::io::Result<Vec<String>> {
    let text = std::fs::read_to_string(repo().join("src/gendata/units.bvnr"))?;
    let mut out = Vec::new();
    for (i, _) in text.match_indices(".symbol") {
        let rest = text[i + ".symbol".len()..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix('"') else {
            continue;
        };
        if let Some(end) = rest.find('"')
            && end > 0
        {
            out.push(rest[..end].to_string());
        }
    }
    Ok(out)
}

/// What two read-back values are compared by: literal, fraction, type and the
/// canonical unit spelling.
#[derive(Debug, PartialEq)]
enum Cmp {
    Quantity(Option<String>, Option<String>, i32, u32, u32, String),
    Other(Value),
}

fn comparable(doc: &Value) -> Vec<(String, Cmp)> {
    let Value::Struct(entries) = doc else {
        panic!("not a struct")
    };
    entries
        .iter()
        .map(|(k, v)| {
            let c = match v {
                Value::Quantity(q) => Cmp::Quantity(
                    q.raw.clone(),
                    q.frac.clone(),
                    q.vtype.family,
                    q.vtype.width,
                    q.vtype.base,
                    q.unit_str(),
                ),
                other => Cmp::Other(other.clone()),
            };
            (k.clone(), c)
        })
        .collect()
}

#[test]
#[ignore = "needs the CLI built at <repo>/build/bovnar; see the module docs"]
fn test_the_two_writers_produce_documents_that_read_back_alike() {
    let cli = repo().join("build/bovnar");
    assert!(
        cli.exists(),
        "no CLI at {} to compare against; build it first",
        cli.display()
    );
    let symbols = catalogue().expect("read the unit catalogue in src/gendata");

    let mut lines = Vec::new();
    for (i, unit) in symbols.iter().enumerate() {
        for (j, (head, tail)) in SHAPES.iter().enumerate() {
            lines.push(format!(".v{i}_{j} = {head}{unit}{tail};"));
        }
    }

    let dir = std::env::temp_dir().join(format!("bovnar-writers-agree-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("doc.bvnr");
    let (mut disagreed, mut compared) = (Vec::new(), 0);
    for chunk in lines.chunks(200) {
        let body = chunk.join("\n") + "\n";
        std::fs::write(&path, &body).unwrap();
        let run = Command::new(&cli)
            .arg("pretty-print")
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr[..run.stderr.len().min(200)])
        );
        let doc = parse_typed(body.as_bytes()).unwrap();
        let from_c = comparable(&parse_typed(&run.stdout).unwrap());
        let from_rust = comparable(&parse_typed(&to_string(&doc, true).unwrap()).unwrap());
        compared += chunk.len();
        if from_c != from_rust {
            let differing: Vec<String> = from_c
                .iter()
                .filter(|(k, c)| {
                    from_rust.iter().find(|(rk, _)| rk == k).map(|(_, r)| r) != Some(c)
                })
                .map(|(k, _)| k.clone())
                .take(5)
                .collect();
            disagreed.extend(differing);
        }
    }
    std::fs::remove_dir_all(&dir).unwrap();

    assert!(compared > 1000, "the sweep stopped comparing anything");
    assert!(
        disagreed.is_empty(),
        "the C and Rust writers disagree on {} shape(s): {:?}",
        disagreed.len(),
        &disagreed[..disagreed.len().min(10)]
    );
}

/// Without this the sweep above could pass by testing nothing that matters.
#[test]
fn test_the_shape_that_broke_is_in_the_sweep() {
    let doc = parse_typed(b".s = \"x\" m;").unwrap();
    let Value::Struct(entries) = &doc else {
        unreachable!()
    };
    let Value::Quantity(q) = &entries[0].1 else {
        panic!("not a quantity: {:?}", entries[0].1)
    };
    assert_eq!(q.unit_str(), "m");
    let out = String::from_utf8(to_string(&doc, true).unwrap()).unwrap();
    assert_eq!(out.trim(), ".s = <utf8> \"x\" m;");
}
