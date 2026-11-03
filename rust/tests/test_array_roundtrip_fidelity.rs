//! Arrays and nested values survive a write/parse roundtrip: units, element
//! precision, declared widths, null holes, datetime fractions and multi-chunk
//! payloads.
//!
//! test_array_element_precision_survives reads 2^128 - 1 through
//! `Quantity::decimal`, since it does not fit the `i128` of `Value::Int`.

use bovnar::stream::embed_document;
use bovnar::{Error, Quantity, Value, parse, parse_typed, to_string};

const DOC: &[u8] = b"#!bovnar 1.1\n\
.dist  = <float:64,k~m> [1.5, 2.5];\n\
.exact = <float_dec:128> [1.2345678901234567890123456789];\n\
.big   = <uint:128> [340282366920938463463374607431768211455];\n\
.small = <uint:8> [1, 2, 3];\n\
.hole  = <float:64,m> [1.0, , 3.0];\n\
.plain = [1, 2, 3];\n\
.mixed = [1, 2.5, 3];\n";

fn field<'a>(v: &'a Value, key: &str) -> &'a Value {
    let Value::Struct(entries) = v else {
        panic!("not a struct: {v:?}")
    };
    entries
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
        .unwrap_or_else(|| panic!("no key {key}"))
}

fn elements(v: &Value) -> &[Value] {
    let Value::Array(items) = v else {
        panic!("not an array: {v:?}")
    };
    items
}

fn quantity(v: &Value) -> &Quantity {
    let Value::Quantity(q) = v else {
        panic!("not a quantity: {v:?}")
    };
    q
}

fn first(v: &Value, key: &str) -> Quantity {
    quantity(&elements(field(v, key))[0]).clone()
}

fn obj(pairs: Vec<(&str, Value)>) -> Value {
    Value::Struct(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn typed_roundtrip(src: &[u8]) -> Value {
    parse_typed(&to_string(&parse_typed(src).unwrap(), true).unwrap()).unwrap()
}

#[test]
fn test_array_unit_survives_a_typed_roundtrip() {
    let back = typed_roundtrip(DOC);
    let dist = first(&back, "dist");
    assert_eq!(dist.unit_str(), "k~m");
    assert_eq!(dist.value(), Value::Float(1.5));
    assert_eq!(first(&back, "hole").unit_str(), "m");
}

#[test]
fn test_array_element_precision_survives() {
    let back = typed_roundtrip(DOC);
    assert_eq!(
        first(&back, "exact")
            .decimal()
            .unwrap()
            .unwrap()
            .to_string(),
        "1.2345678901234567890123456789"
    );
    assert_eq!(
        first(&back, "big").decimal().unwrap().unwrap().to_string(),
        "340282366920938463463374607431768211455"
    );
}

#[test]
fn test_declared_width_survives() {
    let back = typed_roundtrip(DOC);
    assert_eq!(first(&back, "small").vtype.width, 8);
    assert_eq!(first(&back, "big").vtype.width, 128);
}

#[test]
fn test_null_hole_survives() {
    let back = typed_roundtrip(DOC);
    let values: Vec<Value> = elements(field(&back, "hole"))
        .iter()
        .map(|v| match v {
            Value::Quantity(q) => q.value(),
            other => other.clone(),
        })
        .collect();
    assert_eq!(values, [Value::Float(1.0), Value::Null, Value::Float(3.0)]);
}

#[test]
fn test_arrays_needing_no_annotation_get_none() {
    // A redundant annotation would be its own regression.
    let out = String::from_utf8(to_string(&parse_typed(DOC).unwrap(), true).unwrap()).unwrap();
    for line in out.lines() {
        if line.starts_with(".plain") || line.starts_with(".mixed") {
            assert!(!line.contains('<'), "{line}");
        }
    }
}

#[test]
fn test_datetime_fraction_is_part_of_equality() {
    // Two instants 500 ms apart used to compare equal.
    let d = parse_typed(
        b"#!bovnar 1.1\n\
.t1 = <datetime:64> 2024-01-01T00:00:00.250Z;\n\
.t2 = <datetime:64> 2024-01-01T00:00:00.750Z;\n\
.t3 = <datetime:64> 2024-01-01T00:00:00.250Z;\n",
    )
    .unwrap();
    let (t1, t2, t3) = (field(&d, "t1"), field(&d, "t2"), field(&d, "t3"));
    assert_ne!(t1, t2);
    assert_eq!(t1, t3);
    let mut distinct: Vec<&Value> = Vec::new();
    for t in [t1, t2, t3] {
        if !distinct.contains(&t) {
            distinct.push(t);
        }
    }
    assert_eq!(distinct.len(), 2);
}

#[test]
fn test_wide_int_beside_a_null_is_annotated() {
    // A null constrains nothing, but it used to switch the width annotation
    // off, and the array was written as a bare 71-bit number.
    let doc = obj(vec![(
        "k",
        Value::Array(vec![Value::Int(1 << 70), Value::Null]),
    )]);
    let out = to_string(&doc, false).unwrap();
    assert_eq!(out, b".k=<uint:71>[1180591620717411303424,null];");
    assert_eq!(parse(&out).unwrap(), doc);
}

#[test]
fn test_wide_int_in_an_array_with_no_annotation_is_refused() {
    // An array has one annotation; beside a float, a bool or a string there is
    // none to give a wide int, so serialising must refuse.
    for other in [
        Value::Float(1.5),
        Value::Bool(true),
        Value::String("x".into()),
    ] {
        let doc = obj(vec![("k", Value::Array(vec![Value::Int(1 << 70), other]))]);
        let err = to_string(&doc, true).unwrap_err();
        assert!(matches!(err, Error::Argument { .. }), "{err:?}");
    }
}

#[test]
fn test_mixed_type_unitless_array_keeps_its_element_types() {
    // Without a unit in common the elements used to be written bare and every
    // width was lost.
    for src in [
        &b".a=[<uint:8>1,<float:64>2.5];"[..],
        b".a=[<uint:8>1,<uint:8>2]/[<uint:16>3,<uint:16>4];",
        b".a=[<sint:16>5,null,<float:32>1.5];",
    ] {
        let doc = parse_typed(src).unwrap();
        let again = parse_typed(&to_string(&doc, true).unwrap()).unwrap();
        assert_eq!(again, doc, "{}", String::from_utf8_lossy(src));
    }
}

#[test]
fn test_plain_arrays_gain_no_element_annotations() {
    let doc = parse_typed(b".a=[1,2.5,-3];").unwrap();
    assert_eq!(to_string(&doc, false).unwrap(), b".a=[1,2.5,-3];");
}

// TestBytesAreOctetStreams

#[test]
fn test_top_level_struct_and_array() {
    let doc = obj(vec![
        ("p", Value::Bytes(b"ab\x00cd".to_vec())),
        ("s", obj(vec![("q", Value::Bytes(Vec::new()))])),
        (
            "a",
            Value::Array(vec![
                Value::Bytes(b"x".to_vec()),
                Value::Bytes(b"yz".to_vec()),
            ]),
        ),
        ("n", Value::Int(1)),
    ]);
    let out = to_string(&doc, true).unwrap();
    assert_eq!(parse(&out).unwrap(), doc);
    assert_eq!(
        field(&parse_typed(&out).unwrap(), "p"),
        &Value::Bytes(b"ab\x00cd".to_vec())
    );
}

#[test]
fn test_multi_chunk_payload() {
    let payload: Vec<u8> = (0..=255u8).cycle().take(256 * 800).collect(); // 204800 bytes
    let doc = obj(vec![("p", Value::Bytes(payload.clone()))]);
    let back = parse(&to_string(&doc, true).unwrap()).unwrap();
    assert_eq!(field(&back, "p"), &Value::Bytes(payload));
}

#[test]
fn test_loads_dumps_of_an_embedded_document() {
    let inner = to_string(&obj(vec![("v", Value::Int(1))]), true).unwrap();
    let outer = embed_document(&inner, "payload", true).unwrap();
    assert_eq!(
        to_string(&parse(&outer).unwrap(), false).unwrap(),
        embed_document(&inner, "payload", false).unwrap()
    );
}

#[test]
fn test_multi_chunk_array_element() {
    // The C writer used to put the element separator between the chunks of
    // one stream when it was an array element.
    let payload: Vec<u8> = (0..=255u8).cycle().take(256 * 300).collect(); // 76800 bytes
    let doc = obj(vec![(
        "a",
        Value::Array(vec![Value::Bytes(payload), Value::Bytes(b"tail".to_vec())]),
    )]);
    assert_eq!(parse(&to_string(&doc, true).unwrap()).unwrap(), doc);
}

#[test]
fn test_typed_loads_does_not_pin_the_reader_snapshot() {
    let d = parse_typed(b".a=<float:64,m>[1.5,2.5]; .b=<float:64,m>3.5; .c=5 s; .d=sym;").unwrap();
    let a = elements(field(&d, "a"));
    let (a0, a1) = (quantity(&a[0]), quantity(&a[1]));
    let b = quantity(field(&d, "b"));
    let c = quantity(field(&d, "c"));
    assert_eq!(a0.unit, a1.unit);
    assert_eq!(a1.unit, b.unit);
    assert_ne!(c.unit, a0.unit);
    assert_eq!(c.unit.to_string(), "s");
    assert_eq!(
        to_string(&d, false).unwrap(),
        b".a=<float:64,m>[1.5,2.5];.b=<float:64,m>3.5;.c=<uint:64,s>5;.d=sym;"
    );
}
