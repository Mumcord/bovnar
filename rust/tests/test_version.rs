//! The spec version directive (`#!bovnar M.N`).

use bovnar::{
    Error, ErrorCode, EventData, ReadOptions, Reader, Value, parse, peek_version, spec_version,
    version,
};

/// Reads `doc` with an accepting handler; returns the reader and the result.
fn read(doc: &[u8], options: &ReadOptions) -> (Reader, bovnar::Result<()>) {
    let mut r = Reader::new().unwrap();
    let mut on_verified = |_: &EventData<'_>| true;
    let result = r.read_mem(doc, options, Some(&mut on_verified), None);
    (r, result)
}

fn read_error(doc: &[u8], options: &ReadOptions) -> ErrorCode {
    match read(doc, options).1 {
        Err(e @ Error::Parse { .. }) => e.code().expect("known error code"),
        other => panic!("expected a parse error for {doc:?}, got {other:?}"),
    }
}

#[test]
fn test_runtime_versions() {
    assert_eq!(version(), "2.0.0");
    assert_eq!(spec_version(), (1, 2));
}

#[test]
fn test_peek_version() {
    assert_eq!(peek_version(b"#!bovnar 1.1\n.x = 1;"), Some((1, 1)));
    assert_eq!(
        peek_version(b"\xef\xbb\xbf#!bovnar 1.0\n.x = 1;"),
        Some((1, 0))
    );
    assert_eq!(peek_version(b".x = 1;"), None);
    assert_eq!(peek_version(b"#!bovnarish 9.9\n.x = 1;"), None);
}

#[test]
fn test_reader_declared_version() {
    let (r, result) = read(b"#!bovnar 1.0\n.x = 1;", &ReadOptions::default());
    result.unwrap();
    assert_eq!(r.declared_version(), Some((1, 0)));
    let (r, result) = read(b".x = 1;", &ReadOptions::default());
    result.unwrap();
    assert_eq!(r.declared_version(), None);
}

#[test]
fn test_lenient_accepts_future_version() {
    // A newer version is recorded but not rejected by default.
    let (r, result) = read(b"#!bovnar 2.5\n.x = 1;", &ReadOptions::default());
    result.unwrap();
    assert_eq!(r.declared_version(), Some((2, 5)));
}

#[test]
fn test_strict_rejects_future_version() {
    let strict = ReadOptions {
        strict_version: true,
        ..ReadOptions::default()
    };
    assert_eq!(
        read_error(b"#!bovnar 2.0\n.x = 1;", &strict),
        ErrorCode::UnsupportedSpecVersion
    );
}

#[test]
fn test_malformed_directive_is_error() {
    let docs: [&[u8]; 4] = [
        b"#!bovnar 1\n.x = 1;",        // missing minor
        b"#!bovnar a.b\n.x = 1;",      // non-numeric
        b"#!bovnar 1.01\n.x = 1;",     // leading zero
        b"#!bovnar 1.1 beta\n.x = 1;", // trailing junk
    ];
    for doc in docs {
        assert_eq!(
            read_error(doc, &ReadOptions::default()),
            ErrorCode::InvalidSpecVersion,
            "{}",
            String::from_utf8_lossy(doc)
        );
    }
}

#[test]
fn test_loads_ignores_directive() {
    assert_eq!(
        parse(b"#!bovnar 1.1\n.n = 7;").unwrap(),
        Value::Struct(vec![("n".into(), Value::Int(7))])
    );
}
