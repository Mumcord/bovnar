//! `to_string`'s spec 1.1 directive detection, and a string carrying a unit
//! round-tripping.
//!
//! The directive check is private, so these tests observe it through
//! `to_string`: a document that needs no 1.1 feature gains no `#!bovnar`
//! directive.

use bovnar::{Quantity, Unit, Value, ValueTypeFamily, ValueTypeSpec, parse_typed, to_string};

fn one(key: &str, v: Value) -> Value {
    Value::Struct(vec![(key.into(), v)])
}

/// Whether `to_string` declares spec 1.1 for `doc`.
fn uses_spec_1_1(doc: &Value) -> bool {
    to_string(doc, true).unwrap().starts_with(b"#!bovnar")
}

#[test]
fn test_uses_spec_1_1_stays_safe_on_containers() {
    // The directive-detection pass walks containers without failing.
    let vt = ValueTypeSpec {
        family: ValueTypeFamily::Uint as i32,
        width: 64,
        base: 0,
    };
    let q = Value::Quantity(Box::new(Quantity::new("1", vt, Unit::default())));
    assert!(!uses_spec_1_1(&one("q", q.clone())));
    assert!(!uses_spec_1_1(&one("d", one("k", q.clone()))));
    assert!(!uses_spec_1_1(&one("a", Value::Array(vec![q]))));
}

#[test]
fn test_uses_spec_1_1_false_for_plain_values() {
    // Plain values never require the directive.
    let doc = Value::Struct(vec![
        ("a".into(), Value::Int(1)),
        (
            "b".into(),
            Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
        ),
        ("c".into(), Value::String("x".into())),
    ]);
    assert!(!uses_spec_1_1(&doc));
}

// `.s = "x" m;` is legal, and the writer once emitted it in a form the reader
// refused. A string may carry a unit only as an INLINE suffix; `<utf8,m>` is
// error_illegal_value_type. The C writer emits `<utf8> "x" m;`, which is what
// to_string must match.

#[test]
fn test_string_with_a_unit_survives_a_round_trip() {
    let doc = parse_typed(b".s = \"x\" m;").unwrap();
    let out = to_string(&doc, true).unwrap();
    assert!(!out.windows(8).any(|w| w == b"<utf8:m>"));
    assert_eq!(parse_typed(&out).unwrap(), doc);
}

#[test]
fn test_the_emitted_form_is_the_one_the_c_writer_uses() {
    let out = to_string(&parse_typed(b".s = \"x\" m;").unwrap(), true).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap().trim(),
        ".s = <utf8> \"x\" m;"
    );
}

#[test]
fn test_the_unit_is_still_carried() {
    let doc = parse_typed(b".s = \"x\" m;").unwrap();
    let Value::Struct(entries) = &doc else {
        unreachable!()
    };
    let Value::Quantity(q) = &entries[0].1 else {
        panic!("not a quantity: {:?}", entries[0].1)
    };
    assert_eq!(q.unit_str(), "m");
    assert_eq!(q.raw.as_deref(), Some("x"));
}

#[test]
fn test_numeric_families_are_unchanged() {
    let cases: [(&[u8], &str); 3] = [
        (b".n = 5 m;", ".n = <uint:64,m> 5;"),
        (b".f = 1.5 m;", ".f = <float:64,m> 1.5;"),
        (
            b".h = <uint:8,_16,m> \"FF\";",
            ".h = <uint:8,_16,m> \"FF\";",
        ),
    ];
    for (src, want) in cases {
        let doc = parse_typed(src).unwrap();
        let out = to_string(&doc, true).unwrap();
        assert_eq!(String::from_utf8(out.clone()).unwrap().trim(), want);
        assert_eq!(parse_typed(&out).unwrap(), doc);
    }
}
