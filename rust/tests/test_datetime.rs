//! The spec 1.1 native time family (`<datetime:width,epoch>`) through the
//! bindings.

use bovnar::{
    DomDoc, Error, ErrorCode, Event, EventData, Quantity, ReadOptions, Reader, Value,
    ValueTypeFamily, parse, parse_typed, to_string,
};

fn get<'a>(v: &'a Value, key: &str) -> &'a Value {
    let Value::Struct(entries) = v else {
        panic!("not a struct: {v:?}")
    };
    &entries
        .iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("no key {key}"))
        .1
}

fn quantity(v: &Value) -> &Quantity {
    match v {
        Value::Quantity(q) => q,
        other => panic!("not a quantity: {other:?}"),
    }
}

fn elements(v: &Value) -> &[Value] {
    match v {
        Value::Array(items) => items,
        other => panic!("not an array: {other:?}"),
    }
}

fn one(key: &str, v: Value) -> Value {
    Value::Struct(vec![(key.into(), v)])
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn parse_error(doc: &[u8]) -> ErrorCode {
    match parse(doc) {
        Err(e @ Error::Parse { .. }) => e.code().expect("known error code"),
        other => panic!("expected a parse error for {doc:?}, got {other:?}"),
    }
}

#[test]
fn test_loads_decodes_epoch_seconds() {
    assert_eq!(
        parse(b"#!bovnar 1.1\n.t = <datetime:64,unix> 1750000000;").unwrap(),
        one("t", Value::Int(1750000000))
    );
    assert_eq!(
        parse(b"#!bovnar 1.1\n.t = <datetime> -100;").unwrap(),
        one("t", Value::Int(-100))
    );
}

#[test]
fn test_family_and_epoch_on_event() {
    let mut seen = Vec::new();
    let mut on_event = |d: &EventData<'_>| {
        if d.event == Event::Data {
            seen.push((d.value_type.family, d.value_type.base));
        }
        true
    };
    Reader::new()
        .unwrap()
        .read_mem(
            b"#!bovnar 1.1\n.t = <datetime:64,gps> 99;",
            &ReadOptions::default(),
            Some(&mut on_event),
            None,
        )
        .unwrap();
    assert!(!seen.is_empty());
    let (family, epoch_index) = seen[0];
    assert_eq!(family, ValueTypeFamily::Datetime as i32);
    // gps is index 2 in the epoch table
    assert_eq!(epoch_index, 2);
}

#[test]
fn test_typed_epoch_recoverable_and_roundtrips() {
    let doc = parse_typed(b"#!bovnar 1.1\n.t = <datetime:64,gps> 1750000000;").unwrap();
    let q = quantity(get(&doc, "t"));
    assert_eq!(q.value(), Value::Int(1750000000));
    assert_eq!(q.epoch_name().as_deref(), Some("gps"));
    assert_eq!(q.epoch_mjd(), Some(44244));
    // to_string emits the annotation + epoch AND the #!bovnar 1.1 directive,
    // so it round-trips losslessly
    let out = to_string(&doc, true).unwrap();
    assert!(out.starts_with(b"#!bovnar 1.1\n"));
    assert!(contains(&out, b"<datetime:64,gps>"));
    let doc2 = parse_typed(&out).unwrap();
    let q2 = quantity(get(&doc2, "t"));
    assert_eq!(q2.value(), Value::Int(1750000000));
    assert_eq!(q2.epoch_name().as_deref(), Some("gps"));
}

#[test]
fn test_plain_dumps_has_no_version_directive() {
    // documents that use no 1.1 feature must not gain a directive
    let out = to_string(&one("a", Value::Int(1)), true).unwrap();
    assert!(!out.starts_with(b"#!bovnar"));
}

#[test]
fn test_unix_epoch_datetime_roundtrips() {
    // the default (unix) epoch must still emit a <datetime> annotation, else
    // the value reloads as a plain int and loses its family
    let doc = parse_typed(b"#!bovnar 1.1\n.t = <datetime:64,unix> 100;").unwrap();
    let out = to_string(&doc, true).unwrap();
    assert!(contains(&out, b"datetime") && out.starts_with(b"#!bovnar 1.1\n"));
    let doc2 = parse_typed(&out).unwrap();
    let q = quantity(get(&doc2, "t"));
    assert_eq!(q.vtype.family, ValueTypeFamily::Datetime as i32);
    assert_eq!(q.value(), Value::Int(100));
}

#[test]
fn test_typed_array_dumps_does_not_crash() {
    // parse_typed yields Quantity elements; to_string must handle them
    let out = to_string(&parse_typed(b".a = [1, 2, 3];").unwrap(), true).unwrap();
    assert_eq!(
        parse(&out).unwrap(),
        one(
            "a",
            Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)])
        )
    );
}

#[test]
fn test_datetime_array_roundtrips_family() {
    // a datetime ARRAY must keep its family through to_string, like the
    // scalar path; otherwise it silently reloads as plain uint
    let doc = parse_typed(b"#!bovnar 1.1\n.t = [2026-01-01, 2026-01-02];").unwrap();
    let out = to_string(&doc, true).unwrap();
    assert!(contains(&out, b"datetime"));
    let doc2 = parse_typed(&out).unwrap();
    let fams: Vec<i32> = elements(get(&doc2, "t"))
        .iter()
        .map(|v| quantity(v).vtype.family)
        .collect();
    assert_eq!(fams, vec![ValueTypeFamily::Datetime as i32; 2]);
}

#[test]
fn test_datetime_array_preserves_non_unix_epoch() {
    let doc = parse_typed(b"#!bovnar 1.1\n.t = [<datetime:64,tai> 100, 200];").unwrap();
    let out = to_string(&doc, true).unwrap();
    assert!(contains(&out, b"tai"));
    let doc2 = parse_typed(&out).unwrap();
    assert!(
        elements(get(&doc2, "t"))
            .iter()
            .all(|v| quantity(v).epoch_name().as_deref() == Some("tai"))
    );
}

#[test]
fn test_datetime_array_mixed_epochs_rejected() {
    // no single array-level annotation can express two epochs; reject rather
    // than silently dropping one
    let mut doc = parse_typed(b"#!bovnar 1.1\n.t = [<datetime:64,tai> 1];").unwrap();
    let extra = get(
        &parse_typed(b"#!bovnar 1.1\n.u = <datetime:64,unix> 2;").unwrap(),
        "u",
    )
    .clone();
    let Value::Struct(entries) = &mut doc else {
        unreachable!()
    };
    let Value::Array(items) = &mut entries[0].1 else {
        panic!("not an array")
    };
    items.push(extra);
    assert!(matches!(to_string(&doc, true), Err(Error::Argument { .. })));
}

#[test]
fn test_gated_on_version() {
    assert_eq!(
        parse_error(b".t = <datetime> 1;"),
        ErrorCode::IllegalValueType
    );
}

#[test]
fn test_invalid_datetimes() {
    let cases: &[(&[u8], ErrorCode)] = &[
        (
            b"#!bovnar 1.1\n.t = <datetime> 1.5;",
            ErrorCode::TypeValueMismatch,
        ),
        (
            b"#!bovnar 1.1\n.t = <datetime> 1e3;",
            ErrorCode::TypeValueMismatch,
        ),
        (
            b"#!bovnar 1.1\n.t = <datetime:xyz> 1;",
            ErrorCode::IllegalValueType,
        ),
        (
            b"#!bovnar 1.1\n.t = <datetime:_16> 1;",
            ErrorCode::IllegalValueType,
        ),
        (
            b"#!bovnar 1.1\n.t = <datetime:8> 300;",
            ErrorCode::ValueOutOfRange,
        ),
    ];
    for (doc, code) in cases {
        assert_eq!(parse_error(doc), *code, "{}", String::from_utf8_lossy(doc));
    }
}

// ISO-8601 datetime literals (spec 1.1)

#[test]
fn test_iso_literal_values() {
    let cases: &[(&[u8], i128)] = &[
        (b"#!bovnar 1.1\n.t = 2026-06-15;", 1781481600),
        (b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00Z;", 1781524800),
        (b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00;", 1781524800),
        (b"#!bovnar 1.1\n.t = 1970-01-01T00:00:00Z;", 0),
        (b"#!bovnar 1.1\n.t = 1960-01-01;", -315619200),
        (b"#!bovnar 1.1\n.t = <datetime> 2026-06-15;", 1781481600),
        // tai is leap-second correct: 2017-01-01 carries +37 leap seconds
        (
            b"#!bovnar 1.1\n.t = <datetime:64,tai> 2017-01-01T00:00:00Z;",
            1861920037,
        ),
        // fractional seconds floor to the whole-second carrier (the digits are
        // preserved separately; see test_iso_fraction_is_visible_and_roundtrips)
        (b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00.999Z;", 1781524800),
        (
            b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00.000000123;",
            1781524800,
        ),
        // +-HH:MM offset folds to true UTC
        (b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00+02:00;", 1781517600),
        (b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00-05:00;", 1781542800),
        (b"#!bovnar 1.1\n.t = 2026-06-15T01:00:00+02:00;", 1781478000),
        (
            b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00.5+02:00;",
            1781517600,
        ),
        // tai offset is folded before the leap lookup -> stays leap-correct
        (
            b"#!bovnar 1.1\n.t = <datetime:64,tai> 2017-01-01T02:00:00+02:00;",
            1861920037,
        ),
    ];
    for (doc, expected) in cases {
        // parse goes through the C reader; an ISO literal decodes to its epoch
        // seconds carrier exactly like the equivalent integer would.
        let v = parse(doc).unwrap();
        assert_eq!(
            get(&v, "t"),
            &Value::Int(*expected),
            "{}",
            String::from_utf8_lossy(doc)
        );
    }
}

#[test]
fn test_iso_literal_array_is_homogeneous() {
    let v = parse(b"#!bovnar 1.1\n.ts = [2026-01-01, 2026-01-02];").unwrap();
    assert_eq!(
        get(&v, "ts"),
        &Value::Array(vec![Value::Int(1767225600), Value::Int(1767312000)])
    );
}

#[test]
fn test_iso_fraction_is_visible_and_roundtrips() {
    // spec 1.1: the carrier stays whole seconds, but a DOM consumer can read
    // the verbatim sub-second digits as a string.
    let doc = DomDoc::parse(b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00.000000123Z;\n").unwrap();
    let node = doc.lookup("t").unwrap();
    assert_eq!(node.to_value().unwrap(), Value::Int(1781524800)); // carrier = whole second
    assert_eq!(node.datetime_fraction(), Some("000000123")); // fraction verbatim

    // a datetime given as an integer carrier (or with no fraction) exposes none
    let plain = DomDoc::parse(b"#!bovnar 1.1\n.t = <datetime:64> 42;\n").unwrap();
    assert_eq!(plain.lookup("t").unwrap().datetime_fraction(), None);
    let none2 = DomDoc::parse(b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00Z;\n").unwrap();
    assert_eq!(none2.lookup("t").unwrap().datetime_fraction(), None);
}

#[test]
fn test_typed_datetime_roundtrips_losslessly() {
    // Regression: the typed parse/to_string path once (a) dropped a datetime's
    // sub-second fraction and (b) emitted an illegal "<datetime:...,no_unit>"
    // annotation for a bare ISO literal. Both must round-trip losslessly.
    let cases: &[&[u8]] = &[
        // explicit annotation + fraction
        b"#!bovnar 1.1\n.t = <datetime:64> 2026-06-15T12:00:00.123Z;\n",
        // bare literal + fraction
        b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00.123Z;\n",
        // pre-epoch (floored) + fraction
        b"#!bovnar 1.1\n.t = 1969-12-31T23:59:59.5Z;\n",
        // tai + fraction
        b"#!bovnar 1.1\n.t = <datetime:64,tai> 2026-05-28T00:00:00.25Z;\n",
        // bare literal, no fraction
        b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00Z;\n",
        // fraction array
        b"#!bovnar 1.1\n.t = [2026-06-15T12:00:00.5Z, 2026-06-16T00:00:00.25Z];\n",
        // tai fraction array
        b"#!bovnar 1.1\n.t = <datetime:64,tai> [2026-05-28T00:00:00.5Z, 2026-05-29T00:00:00.25Z];\n",
    ];
    for src in cases {
        let d = parse_typed(src).unwrap();
        let out = to_string(&d, true).unwrap();
        // re-parses cleanly, and is stable / idempotent
        let again = parse_typed(&out).unwrap();
        assert_eq!(
            to_string(&again, true).unwrap(),
            out,
            "{}",
            String::from_utf8_lossy(src)
        );
    }
}

#[test]
fn test_typed_datetime_fraction_survives_dumps() {
    let doc = parse_typed(b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00.123Z;\n").unwrap();
    let q = quantity(get(&doc, "t"));
    assert_eq!(q.frac.as_deref(), Some("123")); // captured on the Quantity
    let out = to_string(&one("t", Value::Quantity(Box::new(q.clone()))), true).unwrap();
    assert!(contains(&out, b"2026-06-15T12:00:00.123Z")); // re-emitted verbatim
}

#[test]
fn test_typed_datetime_array_fraction_survives_dumps() {
    // An array element's fraction must survive too (a separate emit path from
    // the scalar one): re-emit must contain the ISO literals, not the carriers.
    let src = b"#!bovnar 1.1\n.t = [2026-06-15T12:00:00.5Z, 2026-06-16T00:00:00.25Z];\n";
    let out = to_string(&parse_typed(src).unwrap(), true).unwrap();
    let out = String::from_utf8(out).unwrap();
    assert!(out.contains("2026-06-15T12:00:00.5Z") && out.contains("2026-06-16T00:00:00.25Z"));
    assert!(!out.contains("1781524800")); // not the dropped-fraction carrier
    // the captured fractions are visible on the element quantities
    let doc = parse_typed(src).unwrap();
    let fracs: Vec<Option<&str>> = elements(get(&doc, "t"))
        .iter()
        .map(|v| quantity(v).frac.as_deref())
        .collect();
    assert_eq!(fracs, vec![Some("5"), Some("25")]);
}

#[test]
fn test_datetime_write_path_struct_layout() {
    // Writing a datetime passes the event data to the C writer, which reads
    // the spec-1.1 fraction fields; a short struct mirror would read out of
    // bounds. Load a typed datetime and write it back to exercise that path.
    let cases: &[&[u8]] = &[
        b"#!bovnar 1.1\n.t = <datetime:64,tai> 1750000000;\n",
        b"#!bovnar 1.1\n.t = <datetime:64,unix> 1781524800;\n",
        b"#!bovnar 1.1\n.t = <datetime:64> -100;\n",
    ];
    for src in cases {
        let out = to_string(&parse_typed(src).unwrap(), true).unwrap();
        assert!(contains(&out, b"datetime"));
    }
    // round-trip preserves the carrier value
    let out = to_string(
        &parse_typed(b"#!bovnar 1.1\n.t = <datetime:64> 1781524800;\n").unwrap(),
        true,
    )
    .unwrap();
    assert_eq!(get(&parse(&out).unwrap(), "t"), &Value::Int(1781524800));
}

#[test]
fn test_invalid_iso_literals() {
    use ErrorCode::*;
    let cases: &[(&[u8], ErrorCode)] = &[
        (b"#!bovnar 1.1\n.t = 2026-13-01;", InvalidDatetimeLiteral),
        (b"#!bovnar 1.1\n.t = 2026-06-31;", InvalidDatetimeLiteral),
        (b"#!bovnar 1.1\n.t = 2025-02-29;", InvalidDatetimeLiteral),
        (b"#!bovnar 1.1\n.t = 2026-006-15;", InvalidDatetimeLiteral),
        (
            b"#!bovnar 1.1\n.t = <datetime:64,gps> 2026-01-01;",
            DatetimeLiteralUnsupportedEpoch,
        ),
        (
            b"#!bovnar 1.1\n.t = <sint:32> 2026-06-15;",
            TypeValueMismatch,
        ),
        // 1.0 gate
        (b".t = 2026-06-15;", IllegalValueType),
        // fractional/offset edge cases: '.' with no digit, 1-digit hour,
        // no colon, offset on date-only, Z and offset
        (
            b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00.Z;",
            InvalidDatetimeLiteral,
        ),
        (
            b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00+2:00;",
            InvalidDatetimeLiteral,
        ),
        (
            b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00+0200;",
            UnexpectedInputByte,
        ),
        (b"#!bovnar 1.1\n.t = 2026-06-15+02:00;", UnexpectedInputByte),
        (
            b"#!bovnar 1.1\n.t = 2026-06-15T12:00:00Z+02:00;",
            UnexpectedInputByte,
        ),
    ];
    for (doc, code) in cases {
        assert_eq!(parse_error(doc), *code, "{}", String::from_utf8_lossy(doc));
    }
}
