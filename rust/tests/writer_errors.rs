//! The writer refuses what it cannot write as a valid document.

use bovnar::{
    ErrorKind, Event, Limits, SpecVersion, TokenKind, TypeAnnotation, TypeFamily, TypeSpec, Writer,
};

fn data(kind: TokenKind, text: &[u8]) -> Event<'_> {
    Event::Data {
        kind,
        text,
        frac: b"",
        value_type: TypeSpec::PLAIN,
        unit: b"",
        inline_unit: b"",
    }
}

fn number(text: &[u8]) -> Event<'_> {
    data(TokenKind::Number, text)
}

fn with_unit<'a>(text: &'a [u8], unit: &'a [u8]) -> Event<'a> {
    Event::Data {
        kind: TokenKind::Number,
        text,
        frac: b"",
        value_type: TypeSpec::PLAIN,
        unit,
        inline_unit: unit,
    }
}

fn ann<'a>(family: TypeFamily, width: Option<u32>, unit: Option<&'a [u8]>) -> Event<'a> {
    Event::TypeAnnotation(TypeAnnotation {
        text: b"",
        family,
        width,
        base: None,
        q: None,
        unit,
        epoch: None,
        synthesized: false,
    })
}

fn key(k: &[u8]) -> Event<'_> {
    Event::AssignmentStart { key: k }
}

/// Feeds `events` and returns the first error, or the output.
fn run(events: &[Event<'_>]) -> Result<String, ErrorKind> {
    let mut w = Writer::new(Vec::new(), false);
    for e in events {
        w.write_event(e).map_err(|e| e.kind())?;
    }
    w.finish().map_err(|e| e.kind())?;
    Ok(String::from_utf8_lossy(&w.into_sink()).into_owned())
}

#[test]
fn writes_valid_streams() {
    let out = run(&[
        key(b"a"),
        ann(TypeFamily::Float, Some(64), Some(b"m")),
        Event::ArrayRowStart,
        number(b"1"),
        number(b"2.5"),
        Event::ArrayRowEnd,
        key(b"b"),
        with_unit(b"5", b"k~g"),
        key(b"s"),
        Event::StructStart,
        key(b"t"),
        data(TokenKind::String, b"q\"\\"),
        Event::StructEnd,
    ]);
    assert_eq!(
        out.as_deref(),
        Ok(".a=<float:64,m>[1,2.5];.b=5 k~g;.s={.t=\"q\\\"\\\\\";};")
    );
}

#[test]
fn events_out_of_place() {
    assert_eq!(run(&[number(b"1")]), Err(ErrorKind::UnknownTokenType));
    assert_eq!(run(&[Event::StructEnd]), Err(ErrorKind::UnknownTokenType));
    assert_eq!(
        run(&[key(b"a"), key(b"b")]),
        Err(ErrorKind::UnknownTokenType)
    );
    assert_eq!(
        run(&[key(b"a"), Event::ArrayRowEnd]),
        Err(ErrorKind::UnknownTokenType)
    );
    assert_eq!(
        run(&[key(b"a"), Event::ArrayRowStart, key(b"b")]),
        Err(ErrorKind::UnknownTokenType)
    );
    assert_eq!(
        run(&[key(b"a"), ann(TypeFamily::Bool, None, None), key(b"b")]),
        Err(ErrorKind::UnknownTokenType)
    );
    assert_eq!(
        run(&[key(b"a"), Event::ArrayDimStart]),
        Err(ErrorKind::UnknownTokenType)
    );
    assert_eq!(
        run(&[Event::OctetStreamChunk(b"x")]),
        Err(ErrorKind::UnknownTokenType)
    );
    assert_eq!(
        run(&[Event::StreamStart, Event::StreamStart]),
        Err(ErrorKind::InvalidArgument)
    );
    assert_eq!(
        run(&[Event::StreamEnd, key(b"a")]),
        Err(ErrorKind::UnknownTokenType)
    );
}

#[test]
fn incomplete_documents() {
    assert_eq!(run(&[key(b"a")]), Err(ErrorKind::GotIncompleteBvnrStream));
    assert_eq!(
        run(&[key(b"a"), Event::StructStart]),
        Err(ErrorKind::GotIncompleteBvnrStream)
    );
    assert_eq!(
        run(&[key(b"a"), Event::ArrayRowStart]),
        Err(ErrorKind::GotIncompleteBvnrStream)
    );
    assert_eq!(
        run(&[key(b"a"), Event::OctetStreamStart]),
        Err(ErrorKind::GotIncompleteBvnrStream)
    );
}

#[test]
fn invalid_values() {
    assert_eq!(
        run(&[
            key(b"a"),
            ann(TypeFamily::Uint, Some(8), None),
            number(b"300")
        ]),
        Err(ErrorKind::ValueOutOfRange)
    );
    assert_eq!(
        run(&[
            key(b"a"),
            ann(TypeFamily::Uint, Some(8), None),
            data(TokenKind::Symbol, b"x")
        ]),
        Err(ErrorKind::TypeValueMismatch)
    );
    // The reader refuses any struct under an annotation, an empty one too.
    assert_eq!(
        run(&[
            key(b"a"),
            ann(TypeFamily::Sint, None, None),
            Event::StructStart,
            Event::StructEnd
        ]),
        Err(ErrorKind::TypeValueMismatch)
    );
    assert_eq!(
        run(&[
            key(b"a"),
            ann(TypeFamily::Sint, None, None),
            Event::ArrayRowStart,
            Event::StructStart
        ]),
        Err(ErrorKind::TypeValueMismatch)
    );
    assert_eq!(
        run(&[key(b"a"), data(TokenKind::Bool, b"yes")]),
        Err(ErrorKind::TypeValueMismatch)
    );
    assert_eq!(
        run(&[key(b"a"), ann(TypeFamily::Utf8, None, Some(b"m"))]),
        Err(ErrorKind::IllegalValueType)
    );
    assert!(run(&[
        key(b"a"),
        ann(TypeFamily::Float, Some(64), Some(b"furlongs"))
    ])
    .is_err());
    // A unit suffix is not allowed on an array element.
    assert_eq!(
        run(&[key(b"a"), Event::ArrayRowStart, with_unit(b"1", b"m")]),
        Err(ErrorKind::UnitIllegal)
    );
    // A suffix that contradicts the annotation's unit.
    assert_eq!(
        run(&[
            key(b"a"),
            ann(TypeFamily::Float, Some(64), Some(b"m")),
            with_unit(b"1", b"s")
        ]),
        Err(ErrorKind::UnitMismatch)
    );
    // A datetime needs a version directive that admits it.
    assert!(run(&[
        key(b"a"),
        ann(TypeFamily::Datetime, Some(64), None),
        number(b"1")
    ])
    .is_err());
    let mut w = Writer::new(Vec::new(), false);
    w.write_version(SpecVersion::V1_1).expect("version first");
    w.write_event(&key(b"a")).expect("key");
    w.write_event(&ann(TypeFamily::Datetime, Some(64), None))
        .expect("datetime under 1.1");
    w.write_event(&number(b"1")).expect("value");
    w.finish().expect("finish");
}

#[test]
fn text_that_would_read_back_differently() {
    // A symbol or key smuggling in another assignment is refused.
    assert!(run(&[key(b"a"), data(TokenKind::Symbol, b"x;.b=y")]).is_err());
    assert!(run(&[key(b"a = 1; .b"), data(TokenKind::Null, b"")]).is_err());
    assert!(run(&[key(b""), data(TokenKind::Null, b"")]).is_err());
    assert!(run(&[key(b"a"), data(TokenKind::Reference, b".x; .y")]).is_err());
    assert!(run(&[key(b"a"), number(b"12abc")]).is_err());
    assert!(run(&[key(b"a"), data(TokenKind::String, b"\xff")]).is_err());
}

#[test]
fn matrix_rows_must_match() {
    let row = |n: &'static [u8]| [Event::ArrayRowStart, number(n), Event::ArrayRowEnd];
    let mut events = vec![key(b"m"), Event::ArrayRowStart];
    events.extend(row(b"1"));
    events.push(Event::ArrayDimStart);
    events.extend([
        Event::ArrayRowStart,
        number(b"2"),
        number(b"3"),
        Event::ArrayRowEnd,
    ]);
    events.push(Event::ArrayRowEnd);
    assert_eq!(run(&events), Err(ErrorKind::ArrayRowSizeMismatch));
}

#[test]
fn octet_chunks() {
    assert_eq!(
        run(&[
            key(b"o"),
            Event::OctetStreamStart,
            Event::OctetStreamChunk(b""),
            Event::OctetStreamEnd
        ]),
        Err(ErrorKind::InvalidArgument)
    );
    let big = vec![7u8; 65_537];
    assert_eq!(
        run(&[
            key(b"o"),
            Event::OctetStreamStart,
            Event::OctetStreamChunk(&big),
            Event::OctetStreamEnd
        ]),
        Err(ErrorKind::InvalidArgument)
    );
    let full = vec![7u8; 65_536];
    let mut w = Writer::new(Vec::new(), false);
    for e in [
        key(b"o"),
        Event::OctetStreamStart,
        Event::OctetStreamChunk(&full),
        Event::OctetStreamEnd,
    ] {
        w.write_event(&e).expect("a full chunk is fine");
    }
    w.finish().expect("finish");
    let out = w.into_sink();
    assert_eq!(&out[3..7], b"\0\x01\0\0", "65536 is written as length 0");
    let doc = bovnar::parse(&out).expect("reads back");
    assert_eq!(
        doc.get("o").and_then(|v| v.as_bytes()).map(<[u8]>::len),
        Some(65_536)
    );
}

#[test]
fn nesting_limits() {
    let mut limits = Limits::default();
    limits.max_struct_nesting = 2;
    limits.max_array_nesting = 1;
    let mut w = Writer::with_limits(Vec::new(), false, &limits);
    w.write_event(&key(b"a")).expect("key");
    w.write_event(&Event::ArrayRowStart).expect("one array");
    let err = w
        .write_event(&Event::ArrayRowStart)
        .expect_err("two arrays");
    assert_eq!(err.kind(), ErrorKind::ArrayNestingTooHigh);

    let mut w = Writer::with_limits(Vec::new(), false, &limits);
    for _ in 0..2 {
        w.write_event(&key(b"s")).expect("key");
        w.write_event(&Event::StructStart).expect("struct");
    }
    w.write_event(&key(b"s")).expect("key");
    let err = w
        .write_event(&Event::StructStart)
        .expect_err("third struct");
    assert_eq!(err.kind(), ErrorKind::StructNestingTooHigh);
}

#[test]
fn errors_are_sticky() {
    let mut w = Writer::new(Vec::new(), true);
    let first = w.write_event(&Event::StructEnd).expect_err("misplaced");
    let again = w.write_event(&key(b"a")).expect_err("still failed");
    assert_eq!(first.kind(), again.kind());
    assert_eq!(w.finish().expect_err("finish too").kind(), first.kind());
}

#[test]
fn version_only_first() {
    let mut w = Writer::new(Vec::new(), true);
    w.write_event(&key(b"a")).expect("key");
    let err = w.write_version(SpecVersion::V1_2).expect_err("too late");
    assert_eq!(err.kind(), ErrorKind::InvalidArgument);
}

#[test]
fn fmt_sink_needs_utf8() {
    let mut w = Writer::to_fmt(String::new(), false);
    for e in [
        key(b"o"),
        Event::OctetStreamStart,
        Event::OctetStreamChunk(b"\xff"),
        Event::OctetStreamEnd,
    ] {
        if let Err(err) = w.write_event(&e) {
            assert_eq!(err.kind(), ErrorKind::InvalidUtf8Byte);
            return;
        }
    }
    panic!("a non-UTF-8 octet stream must not reach a fmt::Write");
}

#[test]
fn io_sink() {
    let mut out = Vec::new();
    let mut w = Writer::to_io(&mut out, true);
    w.write_event(&key(b"a")).expect("key");
    w.write_event(&number(b"1")).expect("value");
    w.finish().expect("finish");
    drop(w);
    assert_eq!(out, b".a = 1;\n");
}

/// Events for `.a = <rows>;`, from a tiny spelling: `[`, `]`, `/` and one
/// digit per number.
fn rows(spelling: &'static [u8]) -> Vec<Event<'static>> {
    let mut events = vec![key(b"a")];
    for (i, &b) in spelling.iter().enumerate() {
        events.push(match b {
            b'[' => Event::ArrayRowStart,
            b']' => Event::ArrayRowEnd,
            b'/' => Event::ArrayDimStart,
            _ => number(&spelling[i..=i]),
        });
    }
    events
}

#[test]
fn a_nested_row_counts_as_one_element_like_the_reader() {
    // Each case: the rows, and whether the reader takes them.
    let cases: [(&[u8], bool); 8] = [
        (b"[1]/[[5]]", true),
        (b"[[5][6]]/[12]", true),
        (b"[[1]/[2]]/[3]", true),
        (b"[[]]/[1]", true),
        (b"[[1]/[2][3]]/[45]", true),
        (b"[12]/[[5]]", false),
        (b"[[5][6]]/[1234]", false),
        (b"[[1]/[2]]/[345]", false),
    ];
    for (spelling, valid) in cases {
        let doc = run(&rows(spelling));
        assert_eq!(doc.is_ok(), valid, "{}", String::from_utf8_lossy(spelling));
        if let Ok(text) = doc {
            bovnar::read_all(text.as_bytes(), &Limits::default(), |_| {})
                .unwrap_or_else(|e| panic!("{text}: {e}"));
        } else {
            assert_eq!(doc, Err(ErrorKind::ArrayRowSizeMismatch));
        }
    }
}

/// Like `run`, under a `#!bovnar 1.2` directive.
fn run_1_2(events: &[Event<'_>]) -> Result<String, ErrorKind> {
    let mut w = Writer::new(Vec::new(), false);
    w.write_version(SpecVersion::V1_2).map_err(|e| e.kind())?;
    for e in events {
        w.write_event(e).map_err(|e| e.kind())?;
    }
    w.finish().map_err(|e| e.kind())?;
    Ok(String::from_utf8_lossy(&w.into_sink()).into_owned())
}

#[test]
fn an_untyped_integer_after_a_datetime_is_a_datetime() {
    let too_big = number(b"18446744073709551615");
    let datetime = ann(TypeFamily::Datetime, None, None);
    // The reader types the last element as a datetime, out of range.
    for between in [vec![], vec![data(TokenKind::Null, b"")]] {
        let mut events = vec![key(b"a"), Event::ArrayRowStart, datetime, number(b"1")];
        events.extend(between);
        events.extend([too_big, Event::ArrayRowEnd]);
        assert_eq!(run_1_2(&events), Err(ErrorKind::ValueOutOfRange));
    }
    // A struct with members, a string or a non-datetime number in between
    // ends the inherited type.
    let breaks: [&[Event<'_>]; 3] = [
        &[
            Event::StructStart,
            key(b"b"),
            number(b"1"),
            Event::StructEnd,
        ],
        &[data(TokenKind::String, b"x")],
        &[number(b"1.5")],
    ];
    for between in breaks {
        let mut events = vec![key(b"a"), Event::ArrayRowStart, datetime, number(b"1")];
        events.extend_from_slice(between);
        events.extend([too_big, Event::ArrayRowEnd]);
        let out = run_1_2(&events).expect("valid document");
        bovnar::read_all(out.as_bytes(), &Limits::default(), |_| {})
            .unwrap_or_else(|e| panic!("{out}: {e}"));
    }
}
