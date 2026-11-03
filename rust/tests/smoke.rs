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

/// `to_string(parse(x))` reads back as `x`, for both parse modes.
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

/// Hands its bytes over one at a time, as a slow pipe would.
struct Trickle<'a>(&'a [u8]);

impl std::io::Read for Trickle<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match (self.0.split_first(), buf.first_mut()) {
            (Some((b, rest)), Some(slot)) => {
                *slot = *b;
                self.0 = rest;
                Ok(1)
            }
            _ => Ok(0),
        }
    }
}

#[test]
fn read_reader_honours_max_file_size_like_read_mem() {
    use bovnar::{Error, EventData, ReadOptions, Reader};
    let doc = b".a = 1;\n.b = 2;\n.c = 3;\n";
    let run = |from_reader: bool, max_file_size: u64| {
        let mut raws = Vec::new();
        let mut cb = |d: &EventData<'_>| {
            raws.push(d.raw.to_vec());
            true
        };
        let options = ReadOptions {
            max_file_size,
            ..ReadOptions::default()
        };
        let mut r = Reader::new().unwrap();
        let result: Result<(), Error> = if from_reader {
            r.read_reader(Trickle(doc), &options, Some(&mut cb), None)
        } else {
            r.read_mem(doc, &options, Some(&mut cb), None)
        };
        (result, raws)
    };
    for cap in [0, 10, doc.len() as u64, doc.len() as u64 - 1] {
        let (mem, mem_raws) = run(false, cap);
        let (io, io_raws) = run(true, cap);
        assert_eq!(io, mem, "cap {cap}");
        assert_eq!(io_raws, mem_raws, "cap {cap}");
    }
    let (err, _) = run(true, 10);
    assert_eq!(err.unwrap_err().code(), Some(ErrorCode::FileTooLong));
}

/// A sink that refuses every byte.
struct Full;

impl std::io::Write for Full {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::StorageFull,
            "no space left",
        ))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn to_writer_reports_a_failing_writer_on_finish() {
    use bovnar::{Base, Error, Writer};
    let mut w = Writer::to_writer(Full, false).unwrap();
    w.write_uint("a", 1, 64, Base::DECIMAL).unwrap();
    let err = w.finish().unwrap_err();
    assert_eq!(err.code(), Some(ErrorCode::WritingToSink));
    assert!(matches!(
        err,
        Error::Io {
            io_kind: std::io::ErrorKind::StorageFull,
            ..
        }
    ));
}

#[test]
fn to_writer_delivers_the_document_on_finish() {
    use bovnar::{Base, Writer};
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);
    impl std::io::Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let out = Shared::default();
    let mut w = Writer::to_writer(out.clone(), false).unwrap();
    w.write_uint("a", 1, 64, Base::DECIMAL).unwrap();
    assert!(out.0.lock().unwrap().is_empty());
    w.finish().unwrap();
    assert_eq!(*out.0.lock().unwrap(), b".a=<uint:64>1;");
}

#[test]
fn dom_parse_reader_reads_a_pipe_of_trickled_bytes() {
    let doc = DomDoc::parse_reader(Trickle(b".x = 1;\n.y = 2;\n")).unwrap();
    assert_eq!(doc.len(), 2);
    let err = DomDoc::parse_reader(Trickle(b".x = @;")).err();
    assert!(matches!(err, Some(bovnar::Error::Parse { .. })), "{err:?}");
}

/// A source whose every read fails.
struct Unreadable;

impl std::io::Read for Unreadable {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "pipe closed",
        ))
    }
}

#[test]
fn parse_reader_and_parse_typed_reader_match_their_in_memory_twins() {
    use bovnar::{parse_reader, parse_typed_reader};
    let doc = b".a = 1;\n.b = <uint:8>7;\n.s = \"x\";\n.l = [1, 2, 3];\n";
    assert_eq!(parse_reader(Trickle(doc)).unwrap(), parse(doc).unwrap());
    assert_eq!(
        parse_typed_reader(Trickle(doc)).unwrap(),
        parse_typed(doc).unwrap()
    );
    let bad = b".a = @;";
    assert_eq!(parse_reader(Trickle(bad)), parse(bad));
    assert_eq!(parse_typed_reader(Trickle(bad)), parse_typed(bad));
}

#[test]
fn parse_reader_maps_a_failing_reader_to_an_io_error() {
    use bovnar::{Error, parse_reader, parse_typed_reader};
    for err in [
        parse_reader(Unreadable).unwrap_err(),
        parse_typed_reader(Unreadable).unwrap_err(),
    ] {
        assert_eq!(err.code(), Some(ErrorCode::ReadingFromSourceFd));
        assert!(matches!(
            err,
            Error::Io {
                io_kind: std::io::ErrorKind::ConnectionReset,
                ..
            }
        ));
    }
}

#[test]
fn write_to_writes_what_to_string_returns() {
    let value = parse_typed(b".a = <uint:8>7; .s = \"x\"; .l = [1, 2];").unwrap();
    for pretty in [false, true] {
        let mut out = Vec::new();
        bovnar::write_to(&value, pretty, &mut out).unwrap();
        assert_eq!(out, to_string(&value, pretty).unwrap());
    }
    let mut out = Vec::new();
    let err = bovnar::write_to(&Value::Int(1), false, &mut out);
    assert_eq!(err, to_string(&Value::Int(1), false).map(|_| ()));
    assert!(out.is_empty());
}

#[test]
fn write_to_maps_a_failing_writer_to_an_io_error() {
    let value = parse(b".a = 1;").unwrap();
    let err = bovnar::write_to(&value, false, Full).unwrap_err();
    assert_eq!(err.code(), Some(ErrorCode::WritingToSink));
    assert!(matches!(
        err,
        bovnar::Error::Io {
            io_kind: std::io::ErrorKind::StorageFull,
            ..
        }
    ));
}
