//! Port of `python/tests/test_escapes.py`: the spec 1.1 string escapes
//! (`\u{...}` and `\xHH`).

use bovnar::{Error, ErrorCode, Value, parse};

fn s(text: &str) -> Value {
    Value::Struct(vec![("s".into(), Value::String(text.into()))])
}

fn parse_error(doc: &[u8]) -> ErrorCode {
    match parse(doc) {
        Err(e @ Error::Parse { .. }) => e.code().expect("known error code"),
        other => panic!("expected a parse error for {doc:?}, got {other:?}"),
    }
}

#[test]
fn test_unicode_escape() {
    assert_eq!(parse(b"#!bovnar 1.1\n.s = \"\\u{41}\";").unwrap(), s("A"));
    assert_eq!(
        parse(b"#!bovnar 1.1\n.s = \"\\u{1F600}\";").unwrap(),
        s("\u{1F600}")
    );
}

#[test]
fn test_byte_escape_forms_utf8() {
    // \xC3\xA9 is the UTF-8 encoding of e-acute
    assert_eq!(
        parse(b"#!bovnar 1.1\n.s = \"caf\\xC3\\xA9\";").unwrap(),
        s("caf\u{e9}")
    );
}

#[test]
fn test_lone_xff_is_invalid_utf8() {
    assert_eq!(
        parse_error(b"#!bovnar 1.1\n.s = \"\\xFF\";"),
        ErrorCode::InvalidUtf8Byte
    );
}

#[test]
fn test_invalid_codepoint() {
    let docs: [&[u8]; 2] = [
        b"#!bovnar 1.1\n.s = \"\\u{D800}\";",   // surrogate
        b"#!bovnar 1.1\n.s = \"\\u{110000}\";", // above U+10FFFF
    ];
    for doc in docs {
        assert_eq!(parse_error(doc), ErrorCode::InvalidCodepoint);
    }
}

#[test]
fn test_gated_on_version() {
    let docs: [&[u8]; 2] = [
        b".s = \"\\u{41}\";", // \u without a 1.1 declaration
        b".s = \"\\x41\";",   // \x without a 1.1 declaration
    ];
    for doc in docs {
        assert_eq!(parse_error(doc), ErrorCode::IllegalEscapeSequence);
    }
}

#[test]
fn test_control_bytes_rejected() {
    // a control byte may not be smuggled into a string via an escape
    let docs: [&[u8]; 3] = [
        b"#!bovnar 1.1\n.s = \"\\u{0}\";", // NUL
        b"#!bovnar 1.1\n.s = \"\\x01\";",  // control byte
        b"#!bovnar 1.1\n.s = \"\\x7f\";",  // DEL
    ];
    for doc in docs {
        assert_eq!(parse_error(doc), ErrorCode::UnexpectedInputByte);
    }
}

#[test]
fn test_whitespace_control_allowed() {
    assert_eq!(
        parse(b"#!bovnar 1.1\n.s = \"a\\x09b\";").unwrap(),
        s("a\tb")
    );
}
