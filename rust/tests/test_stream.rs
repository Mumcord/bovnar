//! Multi-document framing, octet multiplexing and document-in-document
//! embedding.
//!
//! Channels are `u64`, so test_mux_channel_out_of_range_is_refused checks
//! only an out-of-range channel a `u64` can hold. test_mux_load_names_the_demux_error
//! checks the error's C name, since a parse error carries no free-text
//! message.

use bovnar::stream::{self, BVNR_DOC_DEFAULT_MAX, BVNR_FILESIZE_UNLIMITED, BVNR_MUX_DEFAULT_MAX};
use bovnar::{Error, ErrorCode, Value, parse, to_string};

fn obj(fields: &[(&str, Value)]) -> Value {
    Value::Struct(
        fields
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    )
}

fn s(text: &str) -> Value {
    Value::String(text.into())
}

fn field<'a>(doc: &'a Value, key: &str) -> &'a Value {
    match doc {
        Value::Struct(fields) => fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
            .unwrap_or_else(|| panic!("no field {key:?} in {doc:?}")),
        other => panic!("not a struct: {other:?}"),
    }
}

fn bytes_of(v: &Value) -> &[u8] {
    match v {
        Value::Bytes(b) => b,
        other => panic!("not an octet stream: {other:?}"),
    }
}

fn dumps(v: &Value) -> Vec<u8> {
    to_string(v, true).unwrap()
}

fn load(blob: &[u8]) -> bovnar::Result<Vec<Option<Value>>> {
    stream::load_documents(blob, false, false, 0)
}

fn mux_dump(msgs: &[(u64, Vec<u8>)]) -> Vec<u8> {
    stream::mux_dump(msgs, "mux").unwrap()
}

fn mux_load(blob: &[u8]) -> Vec<(u64, Vec<u8>)> {
    stream::mux_load(blob, 0, None).unwrap()
}

fn msgs(list: &[(u64, &[u8])]) -> Vec<(u64, Vec<u8>)> {
    list.iter().map(|(c, p)| (*c, p.to_vec())).collect()
}

fn assert_parse_error<T: std::fmt::Debug>(r: bovnar::Result<T>) {
    assert!(
        matches!(r, Err(Error::Parse { .. })),
        "expected a parse error, got {r:?}"
    );
}

// --- Multi-document record framing ---------------------------------------

#[test]
fn test_roundtrip_dicts() {
    let docs = vec![
        obj(&[("id", Value::Int(1)), ("name", s("alpha"))]),
        obj(&[("id", Value::Int(2)), ("name", s("beta"))]),
        obj(&[("flag", Value::Bool(true)), ("ratio", Value::Float(3.5))]),
    ];
    let blob = stream::dump_documents(&docs, true).unwrap();
    assert!(blob.starts_with(b"BVF1"));
    let back = load(&blob).unwrap();
    assert_eq!(back, docs.into_iter().map(Some).collect::<Vec<_>>());
}

#[test]
fn test_empty_stream() {
    assert_eq!(load(b"").unwrap(), vec![]);
}

#[test]
fn test_single_empty_document() {
    // A frame with a zero-length payload is a valid empty document.
    let blob = stream::frame_documents(&[b""]).unwrap();
    assert_eq!(load(&blob).unwrap(), vec![Some(obj(&[]))]);
}

#[test]
fn test_frame_raw_bytes() {
    let good = dumps(&obj(&[("x", Value::Int(7))]));
    let blob = stream::frame_documents(&[&good, &good]).unwrap();
    let x7 = Some(obj(&[("x", Value::Int(7))]));
    assert_eq!(load(&blob).unwrap(), vec![x7.clone(), x7]);
}

#[test]
fn test_strict_aborts_on_bad_document() {
    let good = dumps(&obj(&[("ok", Value::Int(1))]));
    let blob = stream::frame_documents(&[&good[..], b".broken = @@@;", &good[..]]).unwrap();
    assert_parse_error(load(&blob)); // default: strict
}

#[test]
fn test_continue_past_failed() {
    let good = dumps(&obj(&[("ok", Value::Int(1))]));
    let blob = stream::frame_documents(&[&good[..], b".broken = @@@;", &good[..]]).unwrap();
    let res = stream::load_documents(&blob, false, true, 0).unwrap();
    let ok = Some(obj(&[("ok", Value::Int(1))]));
    assert_eq!(res, vec![ok.clone(), None, ok]);
}

#[test]
fn test_truncated_frame_rejected() {
    let blob = stream::frame_documents(&[dumps(&obj(&[("a", Value::Int(1))]))]).unwrap();
    assert_parse_error(load(&blob[..blob.len() - 3])); // lop off payload bytes
}

#[test]
fn test_oversized_declared_length_rejected() {
    // "BVF1" + len=100_000_000 (LE) + a few payload bytes only.
    let mut blob = b"BVF1".to_vec();
    blob.extend_from_slice(&100_000_000u64.to_le_bytes());
    blob.extend_from_slice(b".x = ");
    assert_parse_error(load(&blob));
}

#[test]
fn test_typed_documents() {
    let blob = stream::frame_documents(&[b".port = <uint:16> 8080;\n"]).unwrap();
    let res = stream::load_documents(&blob, true, false, 0).unwrap();
    assert_eq!(res.len(), 1);
    let doc = res[0].as_ref().unwrap();
    assert!(matches!(field(doc, "port"), Value::Quantity(_)));
}

// --- Octet multiplexing --------------------------------------------------

#[test]
fn test_simple_roundtrip() {
    let m = msgs(&[(1, b"hello"), (42, b"world!!"), (1, b"bye")]);
    assert_eq!(mux_load(&mux_dump(&m)), m);
}

#[test]
fn test_empty_message() {
    let m = msgs(&[(5, b""), (5, b"x")]);
    assert_eq!(mux_load(&mux_dump(&m)), m);
}

#[test]
fn test_large_cross_chunk_message() {
    // Far larger than one 64 KiB octet chunk: exercises split + reassembly.
    let big: Vec<u8> = (0..200_000u32).map(|i| (i * 7 + 1) as u8).collect();
    let m = vec![(7, big), (9, b"small".to_vec())];
    assert_eq!(mux_load(&mux_dump(&m)), m);
}

#[test]
fn test_boundary_sizes() {
    let sizes = [0usize, 1, 65535, 65536, 65537, 131072];
    let m: Vec<(u64, Vec<u8>)> = sizes
        .iter()
        .enumerate()
        .map(|(i, &n)| ((i % 3) as u64, (0..n).map(|k| (k + i) as u8).collect()))
        .collect();
    assert_eq!(mux_load(&mux_dump(&m)), m);
}

#[test]
fn test_interleaved_channels() {
    let m = msgs(&[(1, b"a"), (2, b"bb"), (1, b"ccc"), (3, b"dddd"), (2, b"e")]);
    assert_eq!(mux_load(&mux_dump(&m)), m);
}

#[test]
fn test_key_scope() {
    // A mux stream under "data": scoping to the right key round-trips, a
    // non-matching key ignores the stream, and no scope is the default.
    let m = msgs(&[(1, b"hi"), (2, b"there")]);
    let blob = stream::mux_dump(&m, "data").unwrap();
    assert_eq!(stream::mux_load(&blob, 0, Some("data")).unwrap(), m);
    assert_eq!(stream::mux_load(&blob, 0, Some("other")).unwrap(), vec![]);
    assert_eq!(mux_load(&blob), m);
}

// --- Document-in-document ------------------------------------------------

#[test]
fn test_embed_and_parse() {
    let inner = dumps(&obj(&[("nested", Value::Int(99)), ("label", s("inner"))]));
    let outer = stream::embed_document(&inner, "payload", true).unwrap();
    let top = parse(&outer).unwrap(); // octet stream surfaces as bytes
    let payload = bytes_of(field(&top, "payload"));
    assert_eq!(
        stream::parse_embedded(payload, false).unwrap(),
        obj(&[("nested", Value::Int(99)), ("label", s("inner"))])
    );
}

#[test]
fn test_embed_empty_document() {
    let outer = stream::embed_document(b"", "payload", true).unwrap();
    let top = parse(&outer).unwrap();
    let payload = bytes_of(field(&top, "payload"));
    assert_eq!(stream::parse_embedded(payload, false).unwrap(), obj(&[]));
}

#[test]
fn test_large_embed() {
    // An inner document larger than one 64 KiB octet chunk: exercises the
    // writer's buffer growth and multi-chunk embedding.
    let data = Value::Array((0..20000).map(Value::Int).collect());
    let inner = dumps(&obj(&[("data", data.clone())]));
    assert!(inner.len() > 65536);
    let outer = stream::embed_document(&inner, "payload", true).unwrap();
    let top = parse(&outer).unwrap();
    let rec = stream::parse_embedded(bytes_of(field(&top, "payload")), false).unwrap();
    assert_eq!(field(&rec, "data"), &data);
}

#[test]
fn test_nested_two_levels() {
    let deepest = dumps(&obj(&[("v", Value::Int(1))]));
    let mid = stream::embed_document(&deepest, "inner", true).unwrap();
    let outer = stream::embed_document(&mid, "outer", true).unwrap();
    let lvl1 = parse(&outer).unwrap();
    let lvl2 = stream::parse_embedded(bytes_of(field(&lvl1, "outer")), false).unwrap(); // the mid doc...
    // ...whose "inner" octet payload is the deepest doc.
    let mid_doc = parse(&mid).unwrap();
    assert_eq!(
        stream::parse_embedded(bytes_of(field(&mid_doc, "inner")), false).unwrap(),
        obj(&[("v", Value::Int(1))])
    );
    let Value::Struct(fields) = &lvl2 else {
        panic!("not a struct: {lvl2:?}");
    };
    assert!(fields.iter().any(|(k, _)| k == "inner"));
}

// --- Constants -----------------------------------------------------------

#[test]
fn test_constants_present() {
    assert_eq!(BVNR_FILESIZE_UNLIMITED, 0);
    assert_eq!(BVNR_DOC_DEFAULT_MAX, 256 * 1024 * 1024);
    assert_eq!(BVNR_MUX_DEFAULT_MAX, 64 * 1024 * 1024);
}

#[test]
fn test_mux_channel_out_of_range_is_refused() {
    // The out-of-range channels (-1, 2**64) are refused by the u64 type; the
    // largest channel still round-trips.
    let m = msgs(&[(u64::MAX, b"x")]);
    assert_eq!(mux_load(&mux_dump(&m)), m);
}

#[test]
fn test_parse_embedded_reports_the_real_error() {
    // Every failure was GOT_INCOMPLETE_BVNR_STREAM, whatever went wrong.
    let err = stream::parse_embedded(b".a=@;", false).unwrap_err();
    assert_eq!(err.code(), Some(ErrorCode::UnexpectedInputByte));
    let Some(pos) = err.position() else {
        panic!("no position: {err:?}");
    };
    assert_eq!((pos.line, pos.offset), (1, 3));
    let err = stream::parse_embedded(b".a={.b=1;", false).unwrap_err();
    assert!(matches!(err, Error::Parse { .. }));
    assert_eq!(err.code(), Some(ErrorCode::GotIncompleteBvnrStream));
}

/// Every demux error used to read as a desync, including a stream that ends
/// part-way through a message, which bovnar_stream.h keeps distinct from a
/// framing desync. The error now names its code.
#[test]
fn test_mux_load_names_the_demux_error() {
    let named = |r: bovnar::Result<Vec<(u64, Vec<u8>)>>, code: ErrorCode, name: &str| {
        let e = r.unwrap_err();
        assert_eq!(e.code(), Some(code));
        assert!(e.to_string().contains(name), "{e}");
    };
    // 5 bytes declared, 2 sent
    named(
        stream::mux_load(b".mux=\x00\x01\x04\x00\x02\x05ab\x00;", 0, None),
        ErrorCode::OctetStreamTruncated,
        "octet_stream_truncated",
    );
    named(
        stream::mux_load(&mux_dump(&[(1, b"abcdef".to_vec())]), 3, None),
        ErrorCode::ValueOutOfRange,
        "value_out_of_range",
    );
    // an overlong message-length varint is a real desync
    let mut blob = b".mux=\x00\x01\x0c\x00\x01".to_vec();
    blob.extend_from_slice(&[0xff; 11]);
    blob.extend_from_slice(b"\x00;");
    named(
        stream::mux_load(&blob, 0, None),
        ErrorCode::OctetStreamOutOfSync,
        "octet_stream_out_of_sync",
    );
}

/// A NUL in the key is the caller's mistake, not an allocation failure.
#[test]
fn test_mux_load_refuses_a_key_with_a_nul() {
    let blob = mux_dump(&[(1, b"x".to_vec())]);
    let err = stream::mux_load(&blob, 0, Some("m\0ux")).unwrap_err();
    assert!(matches!(err, Error::Argument { .. }), "{err:?}");
    assert!(err.to_string().contains("NUL"), "{err}");
}

// The Read / Write twins.

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

fn assert_io<T: std::fmt::Debug>(r: bovnar::Result<T>, code: ErrorCode) {
    let e = r.unwrap_err();
    assert_eq!(e.code(), Some(code), "{e:?}");
    assert!(matches!(e, Error::Io { .. }), "{e:?}");
}

fn written(f: impl FnOnce(&mut Vec<u8>) -> bovnar::Result<()>) -> Vec<u8> {
    let mut out = Vec::new();
    f(&mut out).unwrap();
    out
}

#[test]
fn test_writer_twins_write_what_the_in_memory_functions_return() {
    let values = [
        obj(&[("a", Value::Int(1))]),
        obj(&[("b", Value::Bool(true))]),
    ];
    let docs = [b".a = 1;".to_vec(), b".b = 2;".to_vec()];
    let msgs = [(0u64, b"hello".to_vec()), (9, vec![0, 255])];
    assert_eq!(
        written(|o| stream::frame_documents_to(&docs, o)),
        stream::frame_documents(&docs).unwrap()
    );
    for pretty in [false, true] {
        assert_eq!(
            written(|o| stream::dump_documents_to(&values, pretty, o)),
            stream::dump_documents(&values, pretty).unwrap()
        );
        assert_eq!(
            written(|o| stream::embed_document_to(&docs[0], "payload", pretty, o)),
            stream::embed_document(&docs[0], "payload", pretty).unwrap()
        );
    }
    assert_eq!(
        written(|o| stream::mux_dump_to(&msgs, "mux", o)),
        stream::mux_dump(&msgs, "mux").unwrap()
    );
}

#[test]
fn test_writer_twins_map_a_failing_writer_to_an_io_error() {
    let docs = [b".a = 1;".to_vec()];
    let code = ErrorCode::WritingToSink;
    assert_io(stream::frame_documents_to(&docs, Full), code);
    assert_io(stream::dump_documents_to(&[obj(&[])], false, Full), code);
    assert_io(stream::mux_dump_to(&[(1, b"x")], "mux", Full), code);
    assert_io(
        stream::embed_document_to(&docs[0], "payload", false, Full),
        code,
    );
}

#[test]
fn test_reader_twins_read_what_the_in_memory_functions_read() {
    let values = [
        obj(&[("a", Value::Int(1))]),
        obj(&[("b", Value::Bool(true))]),
    ];
    let framed = stream::dump_documents(&values, false).unwrap();
    let bad = stream::frame_documents(&[&b".n = 1;"[..], b".n = [1;", b".n = 3;"]).unwrap();
    for blob in [&framed, &bad] {
        for (typed, cont) in [(false, false), (true, false), (false, true)] {
            assert_eq!(
                stream::load_documents_reader(&blob[..], typed, cont, 0),
                stream::load_documents(blob, typed, cont, 0)
            );
        }
    }

    let muxed = mux_dump(&[(0, b"hello".to_vec()), (7, vec![0, 255])]);
    for key in [None, Some("mux"), Some("other")] {
        assert_eq!(
            stream::mux_load_reader(&muxed[..], 0, key),
            stream::mux_load(&muxed, 0, key)
        );
    }

    for inner in [&b".a = 1; .b = \"x\";"[..], b".a = "] {
        for typed in [false, true] {
            assert_eq!(
                stream::parse_embedded_reader(inner, typed),
                stream::parse_embedded(inner, typed)
            );
        }
    }
}

#[test]
fn test_reader_twins_map_a_failing_reader_to_an_io_error() {
    let code = ErrorCode::ReadingFromSourceFd;
    assert_io(
        stream::load_documents_reader(Unreadable, false, false, 0),
        code,
    );
    assert_io(stream::mux_load_reader(Unreadable, 0, None), code);
    assert_io(stream::parse_embedded_reader(Unreadable, false), code);
}

/// The reader twins read the whole input and leave the per-document and
/// per-message caps to the parse, so an input over a cap fails exactly as
/// its in-memory twin does.
#[test]
fn test_reader_twins_refuse_input_over_the_cap_like_the_in_memory_functions() {
    let doc = dumps(&obj(&[("a", Value::String("x".repeat(64)))]));
    let framed = stream::frame_documents(&[&doc]).unwrap();
    let small = doc.len() as u64 - 1;
    let mem = stream::load_documents(&framed, false, false, small);
    assert!(mem.is_err(), "{mem:?}");
    assert_eq!(
        stream::load_documents_reader(&framed[..], false, false, small),
        mem
    );
    assert!(stream::load_documents_reader(&framed[..], false, false, doc.len() as u64).is_ok());

    let muxed = mux_dump(&[(1, b"abcdef".to_vec())]);
    let mem = stream::mux_load(&muxed, 3, None);
    assert_eq!(
        mem.as_ref().unwrap_err().code(),
        Some(ErrorCode::ValueOutOfRange)
    );
    assert_eq!(stream::mux_load_reader(&muxed[..], 3, None), mem);
}
