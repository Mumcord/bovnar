//! End-to-end smoke test of the public API: parse, serialise, parse again.

use bovnar::{DomDoc, ErrorCode, Value, parse, parse_typed, to_string};

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
            DomDoc::parse(&text).unwrap().to_value().unwrap(),
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

/// What Python's `dumps(loads(x)) == x` promises, for both parse modes.
#[test]
fn every_parse_result_round_trips_through_to_string() {
    let doc = br#"#!bovnar 1.1
        .flat   = [1, -2, 3];
        .floats = [1.5, -0.25];
        .single = [[1, 2, 3]];
        .grid   = [1, 2]/[3, 4]/[5, 6];
        .ragged = [[1], [2, 3]];
        .mixed  = [1, "two", true, null, { .x = 1; }];
        .wide   = <uint:127> 170141183460469231731687303715884105727;
        .wneg   = <sint:100> -633825300114114700748351602688;
        .warr   = <uint:65> [1, 18446744073709551616];
        .hexf   = <float:64,_16> "1.8";
        .speed  = <float:64,k~m/h> 42.5;
        .temps  = <float_dec:64,m> [1.5, 2.5];
        .when   = <datetime:64,unix> 1750000000;
        .tai    = <datetime:64,tai> 2016-12-31T23:59:60.5Z;
        .pos    = <sint:64> 7;
        .hexu   = <uint:16,_16> "ff";
        .fix    = <float_fix:32,q16> 1.5;
        .units  = [<float:64,m> 1.5, <uint:8,m> 2];
    "#;
    let plain = parse(doc).unwrap();
    let Value::Struct(entries) = &plain else {
        unreachable!()
    };
    assert_eq!(entries[10].1, Value::Float(42.5));
    assert_eq!(entries[9].1, Value::Float(1.5), "hex float decodes");
    assert_eq!(entries[6].1, Value::Int(i128::MAX));
    for pretty in [false, true] {
        let text = to_string(&plain, pretty).unwrap();
        assert_eq!(parse(&text).unwrap(), plain, "pretty={pretty}");

        let typed = parse_typed(doc).unwrap();
        let text = to_string(&typed, pretty).unwrap();
        let again = parse_typed(&text).unwrap();
        assert_eq!(again, typed, "{}", String::from_utf8_lossy(&text));
    }
}

#[test]
fn version_directive_is_written_when_needed_and_structs_in_arrays_stay_plain() {
    let typed = parse_typed(b"#!bovnar 1.1\n.s = { .t = <datetime:64,unix> 5; };").unwrap();
    let text = to_string(&typed, false).unwrap();
    assert!(
        text.starts_with(b"#!bovnar 1.1"),
        "{}",
        String::from_utf8_lossy(&text)
    );
    let v = parse(b".a = [{ .x = <uint:65> 18446744073709551616; }, 1];").unwrap();
    assert_eq!(parse(&to_string(&v, true).unwrap()).unwrap(), v);
}

/// bovnar.h (THREADING): distinct readers, writers and documents may live on
/// distinct threads, so each can be moved to one.
#[test]
fn reader_writer_and_dom_move_between_threads() {
    fn assert_send<T: Send>() {}
    assert_send::<bovnar::Reader>();
    assert_send::<bovnar::Writer>();
    assert_send::<bovnar::DomDoc>();

    let mut w = bovnar::Writer::to_mem(4096, false).unwrap();
    let doc = bovnar::DomDoc::parse(b".a = 1;").unwrap();
    let out = std::thread::spawn(move || {
        w.write_string("s", "x").unwrap();
        w.finish().unwrap();
        (w.get_output().to_vec(), doc.len())
    })
    .join()
    .unwrap();
    assert_eq!(out, (b".s=\"x\";".to_vec(), 1));
}

/// Each error carries the library's kind where there is one, and a position
/// only where the library reported one.
#[test]
fn errors_carry_their_kind_and_position() {
    use bovnar::{Error, ErrorKind, Unit};
    let reader = bovnar::parse(b".a = @;").unwrap_err();
    assert_eq!(reader.kind(), Some(ErrorKind::UnexpectedInputByte));
    assert_eq!(reader.position().map(|p| p.line), Some(1));
    let Err(dom) = bovnar::DomDoc::parse(b".a = @;") else {
        panic!("the DOM accepted a bad document");
    };
    assert_eq!(dom.kind(), Some(ErrorKind::UnexpectedInputByte));
    assert_eq!(dom.position(), None);
    let unit = Unit::parse("not_a_unit").unwrap_err();
    assert!(
        matches!(unit, Error::Argument { code: Some(_), .. }),
        "{unit:?}"
    );
    let caller = bovnar::to_string(&bovnar::Value::Int(1), false).unwrap_err();
    assert_eq!(caller.kind(), None);
}
