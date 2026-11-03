//! Document streams, mirroring `python/bovnar/stream.py`: framing several
//! documents into one byte stream, multiplexing binary messages over
//! channels, and embedding one document inside another.

use std::cell::RefCell;

use crate::dict_parser::DictParser;
use crate::enums::ErrorCode;
use crate::error::{Error, Result};
use crate::ffi::{self, BvnrReadFlags, DemuxError};
use crate::reader::{EventData, wrap};
use crate::writer;
use crate::{Value, to_string};

/// `max_file_size` for no limit.
pub const BVNR_FILESIZE_UNLIMITED: u64 = 0;
/// Default per-document cap of [`load_documents`].
pub const BVNR_DOC_DEFAULT_MAX: u64 = 256 * 1024 * 1024;
/// Default per-message cap of [`mux_load`].
pub const BVNR_MUX_DEFAULT_MAX: u64 = 64 * 1024 * 1024;
/// Largest payload a single mux frame carries.
pub const BVNR_MUX_MAX_MESSAGE: u64 = 65536 - 20;

fn read_flags() -> BvnrReadFlags {
    let mut flags = BvnrReadFlags::zeroed();
    flags.max_file_size = BVNR_FILESIZE_UNLIMITED;
    flags.max_struct_nesting = 255;
    flags.max_array_nesting = 255;
    flags
}

fn parse_error(code: i32) -> Error {
    Error::Parse {
        code,
        line: 0,
        column: 0,
        offset: 0,
        byte: 0,
    }
}

/// Frames already-serialised documents into one stream.
pub fn frame_documents<D: AsRef<[u8]>>(docs: &[D]) -> Result<Vec<u8>> {
    let docs: Vec<&[u8]> = docs.iter().map(AsRef::as_ref).collect();
    ffi::frame_documents(&docs).ok_or_else(|| Error::Argument("bvnr_frame_write failed".into()))
}

/// Serialises each value with [`to_string`] and frames the results.
pub fn dump_documents(values: &[Value], pretty: bool) -> Result<Vec<u8>> {
    let docs = values
        .iter()
        .map(|v| to_string(v, pretty))
        .collect::<Result<Vec<_>>>()?;
    frame_documents(&docs)
}

/// Parses a framed stream, one value per document (typed like
/// [`crate::parse_typed`] when `typed`). With `continue_past_failed`, a
/// document that fails is `None` and the rest are still read; otherwise the
/// first failure is the error. `max_document_size` 0 is the default cap.
pub fn load_documents(
    data: &[u8],
    typed: bool,
    continue_past_failed: bool,
    max_document_size: u64,
) -> Result<Vec<Option<Value>>> {
    let parser = RefCell::new(DictParser::new(typed));
    let mut results = Vec::new();
    let mut failed: Option<(u64, i32)> = None;
    let mut on_event = |d: &EventData<'_>| {
        parser.borrow_mut().on_event(d);
        true
    };
    let mut verified = wrap(&mut on_event);
    let mut on_document = |index: u64, ok: bool, err: i32| {
        let done = parser.replace(DictParser::new(typed));
        results.push(ok.then(|| done.result()));
        if !ok {
            failed.get_or_insert((index, err));
        }
        true
    };
    let mut h = ffi::Handlers {
        verified: Some(&mut verified),
        on_document: Some(&mut on_document),
        ..ffi::Handlers::default()
    };
    let (ok, _) = ffi::doc_stream_read(
        data,
        read_flags(),
        continue_past_failed,
        max_document_size,
        &mut h,
    );
    if !ok {
        return Err(parse_error(
            failed.map_or(ErrorCode::GotIncompleteBvnrStream as i32, |(_, e)| e),
        ));
    }
    Ok(results)
}

/// Multiplexes `(channel, payload)` messages into one document under `key`
/// (Python's default is `"mux"`).
pub fn mux_dump<P: AsRef<[u8]>>(messages: &[(u64, P)], key: &str) -> Result<Vec<u8>> {
    let msgs: Vec<(u64, &[u8])> = messages.iter().map(|(c, p)| (*c, p.as_ref())).collect();
    let total: usize = msgs.iter().map(|(_, p)| p.len()).sum();
    writer::grow_write(total + msgs.len() * 64 + key.len(), false, |w| {
        w.check(|h| h.mux(key, &msgs))
    })
}

/// The `(channel, payload)` messages of a multiplexed document, in order.
/// `max_message` 0 is the default cap; `key` `None` accepts any.
pub fn mux_load(data: &[u8], max_message: u64, key: Option<&str>) -> Result<Vec<(u64, Vec<u8>)>> {
    let key = key
        .map(std::ffi::CString::new)
        .transpose()
        .map_err(|_| Error::Argument("mux key contains a NUL byte".into()))?;
    ffi::demux_read(data, read_flags(), max_message, key.as_deref()).map_err(|e| match e {
        DemuxError::Demux(code) => parse_error(code),
        DemuxError::Read(r) => Error::Parse {
            code: r.code,
            line: r.line,
            column: r.column,
            offset: r.offset,
            byte: r.byte,
        },
        DemuxError::OutOfMemory(call) => Error::OutOfMemory(call),
    })
}

/// Embeds the document `inner` under `key` (Python's default `"payload"`)
/// in a new document.
pub fn embed_document(inner: &[u8], key: &str, pretty: bool) -> Result<Vec<u8>> {
    writer::grow_write(inner.len() + key.len(), pretty, |w| {
        w.check(|h| h.embed_document(key, inner))
    })
}

/// Parses an embedded document (the payload [`embed_document`] wrote).
pub fn parse_embedded(inner: &[u8], typed: bool) -> Result<Value> {
    let mut parser = DictParser::new(typed);
    let (ok, first_error) = {
        let mut on_event = |d: &EventData<'_>| {
            parser.on_event(d);
            true
        };
        let mut verified = wrap(&mut on_event);
        let mut h = ffi::Handlers {
            verified: Some(&mut verified),
            ..ffi::Handlers::default()
        };
        (
            ffi::parse_embedded(inner, read_flags(), &mut h),
            h.first_error,
        )
    };
    if !ok {
        return Err(match first_error {
            Some((code, line, column, byte, offset)) => Error::Parse {
                code,
                line,
                column,
                offset,
                byte,
            },
            None => parse_error(ErrorCode::GotIncompleteBvnrStream as i32),
        });
    }
    Ok(parser.result())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn doc(n: i128) -> Value {
        Value::Struct(vec![("n".into(), Value::Int(n))])
    }

    #[test]
    fn frames_and_loads_documents() {
        let stream = dump_documents(&[doc(1), doc(2)], false).unwrap();
        let docs = load_documents(&stream, false, false, 0).unwrap();
        assert_eq!(docs, vec![Some(doc(1)), Some(doc(2))]);

        let bad = frame_documents(&[&b".n = 1;"[..], b".n = [1;", b".n = 3;"]).unwrap();
        let r = load_documents(&bad, false, false, 0);
        assert!(r.is_err(), "{r:?}");
        let docs = load_documents(&bad, false, true, 0).unwrap();
        assert_eq!(docs, vec![Some(doc(1)), None, Some(doc(3))]);
    }

    #[test]
    fn mux_round_trips() {
        let msgs = vec![
            (0u64, b"hello".to_vec()),
            (7, vec![0, 255]),
            (0, Vec::new()),
        ];
        let text = mux_dump(&msgs, "mux").unwrap();
        assert!(parse(&text).is_ok());
        assert_eq!(mux_load(&text, 0, None).unwrap(), msgs);
        assert_eq!(mux_load(&text, 0, Some("mux")).unwrap(), msgs);
    }

    #[test]
    fn embeds_and_parses_back() {
        let inner = b".a = 1; .b = \"x\";";
        let outer = embed_document(inner, "payload", true).unwrap();
        let Value::Struct(entries) = parse(&outer).unwrap() else {
            unreachable!()
        };
        let Value::Bytes(payload) = &entries[0].1 else {
            panic!("{entries:?}")
        };
        let back = parse_embedded(payload, false).unwrap();
        assert_eq!(back, parse(inner).unwrap());
        assert!(parse_embedded(b".a = ", false).is_err());
    }
}
