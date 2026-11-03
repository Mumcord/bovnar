//! Streaming helpers. The frame-stream expectations were captured from the
//! C reference's `bvnr_doc_stream_read` over a memory source: the
//! `on_document` calls (index, verdict) and the final result and count.

use std::io::{self, Read};

use bovnar::stream::{
    embed_document, read_doc_stream, write_frame, DocStreamOptions, FrameDecoder, FrameError,
    FrameReader, ReadAdapter, StreamError, DEFAULT_MAX_DOCUMENT_SIZE,
};
use bovnar::{ErrorKind, Event, Limits, ReadOptions, Writer};

const G1: &[u8] = b".a = 1;";
const BAD: &[u8] = b".b = <uint:8> 256;";
const G2: &[u8] = b".c = \"x\";";

fn fr(doc: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    write_frame(&mut out, doc).expect("vec sink");
    out
}

fn raw(magic: &[u8; 4], len: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = magic.to_vec();
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

fn cat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

/// Runs `read_doc_stream` and renders it like the C harness:
/// `doc0:ok:none doc1:fail:value_out_of_range => fail count=2`.
fn c_style(wire: &[u8], continue_past_failed: bool, cap: u64, veto: Option<u64>) -> String {
    let options = DocStreamOptions::new()
        .continue_past_failed(continue_past_failed)
        .max_document_size(cap);
    let mut log = String::new();
    let result = read_doc_stream(wire, &options, |index, _, verdict| {
        let (ok, err) = match verdict {
            Ok(()) => ("ok", "none"),
            Err(e) => ("fail", e.kind().as_str()),
        };
        log.push_str(&format!(" doc{index}:{ok}:{err}"));
        veto != Some(index)
    });
    let (verdict, count) = match result {
        Ok(count) => ("ok", count),
        Err(StreamError::Document { index, .. } | StreamError::Stopped { index }) => {
            ("fail", index + 1)
        }
        // Framing and I/O errors happen between documents.
        Err(_) => ("fail", log.matches(" doc").count() as u64),
    };
    format!("{log} => {verdict} count={count}")
        .trim()
        .to_string()
}

/// Wire, continue past failed, cap, veto index, C output.
type Case = (Vec<u8>, bool, u64, Option<u64>, &'static str);

#[test]
fn doc_stream_matches_the_c_reference() {
    let big = b".abcdefghijklmnop = 1;";
    let cases: Vec<Case> = vec![
        (vec![], false, 0, None, "=> ok count=0"),
        (
            cat(&[&fr(G1), &fr(G2)]),
            false,
            0,
            None,
            "doc0:ok:none doc1:ok:none => ok count=2",
        ),
        (
            cat(&[&fr(G1), &fr(BAD), &fr(G2)]),
            false,
            0,
            None,
            "doc0:ok:none doc1:fail:value_out_of_range => fail count=2",
        ),
        (
            cat(&[&fr(G1), &fr(BAD), &fr(G2)]),
            true,
            0,
            None,
            "doc0:ok:none doc1:fail:value_out_of_range doc2:ok:none => ok count=3",
        ),
        (
            cat(&[&fr(G1), &raw(b"BVF2", 1, b";")]),
            false,
            0,
            None,
            "doc0:ok:none => fail count=1",
        ),
        (
            cat(&[&fr(G1), b"BVF1\x07"]),
            false,
            0,
            None,
            "doc0:ok:none => fail count=1",
        ),
        (
            cat(&[&fr(G1), &fr(G2)[..fr(G2).len() - 3]]),
            false,
            0,
            None,
            "doc0:ok:none => fail count=1",
        ),
        (
            cat(&[&fr(G1), &fr(big)]),
            false,
            16,
            None,
            "doc0:ok:none => fail count=1",
        ),
        (
            fr(b".abcdefgh = 1;"),
            false,
            14,
            None,
            "doc0:ok:none => ok count=1",
        ),
        (
            cat(&[&fr(b""), &fr(G1)]),
            false,
            0,
            None,
            "doc0:ok:none doc1:ok:none => ok count=2",
        ),
        (
            cat(&[&fr(G1), &fr(G2), &fr(G1)]),
            false,
            0,
            Some(1),
            "doc0:ok:none doc1:ok:none => fail count=2",
        ),
        (
            cat(&[&fr(G1), &fr(BAD), &fr(G2)]),
            true,
            0,
            Some(1),
            "doc0:ok:none doc1:fail:value_out_of_range => fail count=2",
        ),
        (
            cat(&[&fr(G1), &raw(b"BVF1", 1 << 63, b"x")]),
            false,
            0,
            None,
            "doc0:ok:none => fail count=1",
        ),
        (
            cat(&[&fr(G1), &raw(b"BVF1", 1 << 63, b"x")]),
            false,
            u64::MAX,
            None,
            "doc0:ok:none => fail count=1",
        ),
    ];
    for (wire, cont, cap, veto, want) in cases {
        assert_eq!(c_style(&wire, cont, cap, veto), want, "{wire:?}");
    }
}

#[test]
fn doc_stream_reports_why_it_stopped() {
    let opts = DocStreamOptions::new();
    let wire = cat(&[&fr(G1), &fr(BAD)]);
    match read_doc_stream(&wire[..], &opts, |_, _, _| true) {
        Err(StreamError::Document { index: 1, error }) => {
            assert_eq!(error.kind(), ErrorKind::ValueOutOfRange);
        }
        other => panic!("{other:?}"),
    }
    let wire = cat(&[&fr(G1), b"BVF1\x07"]);
    assert!(matches!(
        read_doc_stream(&wire[..], &opts, |_, _, _| true),
        Err(StreamError::Frame(FrameError::Truncated))
    ));
    let wire = raw(b"XXXX", 0, b"");
    assert!(matches!(
        read_doc_stream(&wire[..], &opts, |_, _, _| true),
        Err(StreamError::Frame(FrameError::BadMagic))
    ));
    let wire = raw(b"BVF1", DEFAULT_MAX_DOCUMENT_SIZE + 1, b"");
    assert!(matches!(
        read_doc_stream(&wire[..], &opts, |_, _, _| true),
        Err(StreamError::Frame(FrameError::TooLarge { .. }))
    ));
}

#[test]
fn doc_stream_applies_read_options() {
    let wire = cat(&[&fr(b".a = \x00\x00;"), &fr(G1)]);
    let opts = DocStreamOptions::new()
        .read(ReadOptions::new().text_only(true))
        .continue_past_failed(true);
    let mut kinds = Vec::new();
    let count = read_doc_stream(&wire[..], &opts, |_, _, v| {
        kinds.push(v.err().map(|e| e.kind()));
        true
    })
    .expect("continues");
    assert_eq!(count, 2);
    assert_eq!(kinds, [Some(ErrorKind::OctetStreamForbidden), None]);
}

#[test]
fn frame_decoder_agrees_with_frame_reader_at_any_split() {
    let wire = cat(&[&fr(G1), &fr(b""), &fr(BAD), &fr(&[7u8; 300])]);
    let mut expected = Vec::new();
    let mut reader = FrameReader::new(&wire[..], 0);
    while let Some(p) = reader.next_frame().expect("well-formed") {
        expected.push(p);
    }
    assert_eq!(expected.len(), 4);
    for chunk in [1, 2, 5, 12, 13, 64, wire.len()] {
        let mut decoder = FrameDecoder::new(0);
        let mut got = Vec::new();
        for piece in wire.chunks(chunk) {
            let mut input = piece;
            while let Some(p) = decoder.decode(&mut input).expect("well-formed") {
                got.push(p);
            }
            assert!(input.is_empty());
        }
        decoder.finish().expect("ends on a boundary");
        assert_eq!(got, expected, "chunk {chunk}");
    }
}

#[test]
fn frame_decoder_errors() {
    let mut decoder = FrameDecoder::new(0);
    let wire = fr(G1);
    let mut input = &wire[..5];
    assert_eq!(decoder.decode(&mut input), Ok(None));
    assert_eq!(decoder.finish(), Err(FrameError::Truncated));

    let mut decoder = FrameDecoder::new(4);
    let wire = fr(G1);
    let mut input = &wire[..];
    assert_eq!(
        decoder.decode(&mut input),
        Err(FrameError::TooLarge { len: 7, max: 4 })
    );
    // Sticky.
    assert_eq!(
        decoder.decode(&mut &b""[..]),
        Err(FrameError::TooLarge { len: 7, max: 4 })
    );
    assert!(decoder.finish().is_err());

    let mut decoder = FrameDecoder::new(0);
    let wire = raw(b"BVF0", 0, b"");
    assert_eq!(decoder.decode(&mut &wire[..]), Err(FrameError::BadMagic));
}

#[test]
fn frame_reader_stops_after_each_frame() {
    let wire = cat(&[&fr(G1), b"trailing"]);
    let mut reader = FrameReader::new(&wire[..], 0);
    assert_eq!(reader.next_frame().expect("frame").as_deref(), Some(G1));
    let mut rest = Vec::new();
    reader.into_inner().read_to_end(&mut rest).expect("slice");
    assert_eq!(rest, b"trailing");
}

/// A source that returns one byte per call and interrupts every other call.
struct Trickle<'a> {
    data: &'a [u8],
    interrupt: bool,
}

impl Read for Trickle<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.interrupt = !self.interrupt;
        if self.interrupt {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let Some((&first, rest)) = self.data.split_first() else {
            return Ok(0);
        };
        let Some(slot) = buf.first_mut() else {
            return Ok(0);
        };
        *slot = first;
        self.data = rest;
        Ok(1)
    }
}

fn keys(adapter: &mut ReadAdapter<impl Read>) -> (Vec<Vec<u8>>, Option<StreamError>) {
    let mut keys = Vec::new();
    while let Some(event) = adapter.next_event() {
        match event {
            Ok(Event::AssignmentStart { key }) => keys.push(key.to_vec()),
            Ok(_) => {}
            Err(e) => return (keys, Some(e)),
        }
    }
    (keys, None)
}

#[test]
fn read_adapter_pulls_any_source() {
    let doc = b"#!bovnar 1.2\n.a = 1; .b = { .c = \x00\x01\x02\x00hi\x00; };";
    let mut adapter = ReadAdapter::new(
        Trickle {
            data: doc,
            interrupt: false,
        },
        &ReadOptions::new(),
    );
    let (k, err) = keys(&mut adapter);
    assert!(err.is_none(), "{err:?}");
    assert_eq!(k, [b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
    assert_eq!(
        adapter.reader().declared_version().map(|v| v.minor),
        Some(2)
    );
    assert!(adapter.next_event().is_none());
}

#[test]
fn read_adapter_hands_out_events_before_the_error() {
    let mut adapter =
        ReadAdapter::with_chunk_size(&b".a = 1; .b = <uint:8> 256;"[..], &ReadOptions::new(), 4);
    let (k, err) = keys(&mut adapter);
    assert_eq!(k, [b"a".to_vec(), b"b".to_vec()]);
    match err {
        Some(StreamError::Read(e)) => assert_eq!(e.kind(), ErrorKind::ValueOutOfRange),
        other => panic!("{other:?}"),
    }
    assert!(adapter.next_event().is_none());

    let mut adapter = ReadAdapter::new(&b".a = 1"[..], &ReadOptions::new());
    match keys(&mut adapter).1 {
        Some(StreamError::Read(e)) => assert_eq!(e.kind(), ErrorKind::GotIncompleteBvnrStream),
        other => panic!("{other:?}"),
    }
}

struct Broken;

impl Read for Broken {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("cable cut"))
    }
}

#[test]
fn read_adapter_reports_io_errors() {
    let mut adapter = ReadAdapter::new(Broken, &ReadOptions::new());
    assert!(matches!(keys(&mut adapter).1, Some(StreamError::Io(_))));
    assert!(adapter.next_event().is_none());
    assert!(matches!(
        FrameReader::new(Broken, 0).next_frame(),
        Err(StreamError::Io(_))
    ));
}

#[test]
fn read_adapter_streams_beyond_the_input_cap_when_unlimited() {
    // 4 MiB of assignments through 64 KiB chunks with the size cap lifted.
    struct Endless(u64);
    impl Read for Endless {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            const LINE: &[u8] = b".k = 12345678;\n";
            if self.0 == 0 {
                return Ok(0);
            }
            let n = buf.len() / LINE.len();
            let n = n.min(usize::try_from(self.0).unwrap_or(usize::MAX));
            for slot in buf.chunks_mut(LINE.len()).take(n) {
                slot.copy_from_slice(LINE);
            }
            self.0 -= n as u64;
            Ok(n * LINE.len())
        }
    }
    let mut limits = Limits::default();
    limits.max_input_bytes = 0;
    let lines = 4 * 1024 * 1024 / 15;
    let mut adapter = ReadAdapter::new(Endless(lines), &ReadOptions::new().limits(limits));
    let mut count = 0u64;
    while let Some(event) = adapter.next_event() {
        if let Event::AssignmentStart { .. } = event.expect("valid") {
            count += 1;
        }
    }
    assert_eq!(count, lines);
}

#[test]
fn embedded_documents_round_trip() {
    let inner = {
        let mut w = Writer::new(Vec::new(), false);
        let big = vec![b'x'; 70_000];
        embed_document(&mut w, b"blob", &big).expect("octets");
        w.finish().expect("done");
        w.into_sink()
    };
    let mut w = Writer::new(Vec::new(), true);
    embed_document(&mut w, b"inner", &inner).expect("octets");
    w.finish().expect("done");
    let outer = bovnar::parse(&w.into_sink()).expect("outer parses");
    let bytes = outer
        .get("inner")
        .and_then(|v| v.as_bytes())
        .expect("octets");
    assert_eq!(bytes, &inner[..]);
    let doc = bovnar::parse(bytes).expect("inner parses");
    assert_eq!(
        doc.get("blob").and_then(|v| v.as_bytes()).map(<[u8]>::len),
        Some(70_000)
    );

    let mut w = Writer::new(Vec::new(), false);
    assert!(embed_document(&mut w, b"not a key", b"").is_err());
}
