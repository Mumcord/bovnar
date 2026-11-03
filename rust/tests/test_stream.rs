//! Port of python/tests/test_stream.py: multi-document framing, octet
//! multiplexing and document-in-document embedding.
//!
//! Nothing skipped. test_mux_channel_out_of_range_is_refused keeps only its
//! last case: channels are `u64` here, so -1 and 2**64 cannot be passed.
//! test_mux_load_names_the_demux_error checks the error's C name: a Rust
//! parse error carries no free-text message, so "desync" is never in it.

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
    let Error::Parse { line, offset, .. } = err else {
        panic!("not a parse error: {err:?}");
    };
    assert_eq!((line, offset), (1, 3));
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
    assert!(matches!(err, Error::Argument(_)), "{err:?}");
    assert!(err.to_string().contains("NUL"), "{err}");
}
