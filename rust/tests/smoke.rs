//! End-to-end smoke test of the public API: parse, serialise, parse again.

use bovnar::{DomDoc, ErrorCode, Value, parse, to_string};

fn s(text: &str) -> Value {
    Value::String(text.into())
}

#[test]
fn parse_reads_scalars_structs_and_arrays() {
    let doc = br#"
        .host   = "api.example.com";
        .port   = 443;
        .ratio  = 3.5;
        .on     = true;
        .none   = null;
        .neg    = <sint:64> -7;
        .hex    = <uint:16,_16> "ff";
        .inner  = { .name = "x"; };
        .matrix = [1, 2, 3]/[4, 5, 6];
    "#;
    let v = parse(doc).unwrap();
    let ints = |r: &[i128]| Value::Array(r.iter().map(|&i| Value::Int(i)).collect());
    assert_eq!(
        v,
        Value::Struct(vec![
            ("host".into(), s("api.example.com")),
            ("port".into(), Value::Int(443)),
            ("ratio".into(), Value::Float(3.5)),
            ("on".into(), Value::Bool(true)),
            ("none".into(), Value::Null),
            ("neg".into(), Value::Int(-7)),
            ("hex".into(), Value::Int(255)),
            ("inner".into(), Value::Struct(vec![("name".into(), s("x"))])),
            (
                "matrix".into(),
                Value::Array(vec![ints(&[1, 2, 3]), ints(&[4, 5, 6])])
            ),
        ])
    );
}

#[test]
fn to_string_round_trips_through_parse_and_dom() {
    let value = Value::Struct(vec![
        ("host".into(), s("api.example.com")),
        ("port".into(), Value::Int(443)),
        ("big".into(), Value::Int(u64::MAX.into())),
        ("neg".into(), Value::Int(i64::MIN.into())),
        ("ratio".into(), Value::Float(-0.125)),
        ("on".into(), Value::Bool(false)),
        ("none".into(), Value::Null),
        ("blob".into(), Value::Bytes(vec![0, 1, 2, 255])),
        (
            "inner".into(),
            Value::Struct(vec![("name".into(), s("x \"quoted\""))]),
        ),
    ]);
    for pretty in [false, true] {
        let text = to_string(&value, pretty).unwrap();
        assert_eq!(parse(&text).unwrap(), value, "pretty={pretty}");
        assert_eq!(
            DomDoc::parse(&text).unwrap().to_value(),
            value,
            "pretty={pretty}"
        );
    }
}

#[test]
fn malformed_input_reports_the_c_error_name() {
    let err = parse(b".a = 1").unwrap_err();
    assert_eq!(err.code(), Some(ErrorCode::GotIncompleteBvnrStream));
    assert_eq!(err.name(), Some("incomplete_bvnr_stream"));
    assert!(err.to_string().contains("incomplete_bvnr_stream"), "{err}");
}
