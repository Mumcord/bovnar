//! Streaming helpers (upstream `doc/10`): multi-document record framing,
//! document-in-document embedding and, with the `std` feature, pull adapters
//! over `std::io::Read`.
//!
//! # Endless streaming
//!
//! A single document of unbounded length needs no helper: push it into a
//! [`Reader`](crate::Reader) chunk by chunk with
//! [`Reader::feed`](crate::Reader::feed), drain
//! [`Reader::next_event`](crate::Reader::next_event) after each chunk and keep
//! nothing, and memory stays flat. Raise
//! [`Limits::max_input_bytes`](crate::Limits::max_input_bytes) (`0` is
//! unlimited) for a stream that never ends. `ReadAdapter` does the pulling
//! for an `std::io::Read` source.
//!
//! # Record framing
//!
//! The grammar has no document separator, so a sequence of independent
//! documents on one byte stream is framed (doc/10 §2.1):
//!
//! ```text
//! frame  = "BVF1" payload_len:u64-le payload[payload_len]
//! stream = { frame }   ; ends with EOF on a frame boundary
//! ```
//!
//! [`write_frame`] produces a frame, [`FrameDecoder`] splits pushed bytes
//! back into payloads, and with `std` `FrameReader` and `read_doc_stream`
//! pull them from an `std::io::Read`. Octet multiplexing (doc/10 §3) is not
//! offered.
//!
//! ```
//! use bovnar::stream::{write_frame, FrameDecoder};
//!
//! let mut wire = Vec::new();
//! write_frame(&mut wire, b".a = 1;")?;
//! write_frame(&mut wire, b".b = 2;")?;
//!
//! let mut decoder = FrameDecoder::new(0);
//! let mut input = &wire[..];
//! let mut docs = Vec::new();
//! while let Some(payload) = decoder.decode(&mut input)? {
//!     docs.push(bovnar::parse(&payload)?);
//! }
//! decoder.finish()?;
//! assert_eq!(docs.len(), 2);
//! # Ok::<(), Box<dyn core::error::Error>>(())
//! ```

#[cfg(feature = "std")]
mod io;

use alloc::vec::Vec;
use core::fmt;

use crate::event::Event;
use crate::writer::{Sink, Writer};
use crate::Error;

#[cfg(feature = "std")]
pub use io::{read_doc_stream, DocStreamOptions, FrameReader, ReadAdapter, StreamError};

/// The four magic bytes that open every frame.
pub const FRAME_MAGIC: [u8; 4] = *b"BVF1";
/// Length of a frame header: magic plus the 64-bit payload length.
pub const FRAME_HEADER_LEN: usize = 12;
/// The payload cap used when `0` is given (`BVNR_DOC_DEFAULT_MAX`, 256 MiB).
pub const DEFAULT_MAX_DOCUMENT_SIZE: u64 = 256 * 1024 * 1024;
/// Chunk size an embedded document is written in, as in C.
const EMBED_CHUNK: usize = 65_536;

/// Why a frame stream is malformed. The C reference reports all of these as
/// a plain failure of `bvnr_doc_stream_read`; they are told apart here so a
/// caller can log them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FrameError {
    /// A frame does not start with [`FRAME_MAGIC`].
    BadMagic,
    /// A frame declares a payload longer than the configured cap.
    TooLarge {
        /// The declared payload length.
        len: u64,
        /// The cap it exceeds.
        max: u64,
    },
    /// The input ends inside a frame header or payload.
    Truncated,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadMagic => f.write_str("frame does not start with BVF1"),
            Self::TooLarge { len, max } => {
                write!(f, "frame payload of {len} bytes exceeds the cap of {max}")
            }
            Self::Truncated => f.write_str("input ends inside a frame"),
        }
    }
}

impl core::error::Error for FrameError {}

/// Writes `doc` as one frame (`bvnr_frame_write`). `doc` is sent as is; it
/// should be a complete document.
///
/// # Errors
///
/// The sink's error.
pub fn write_frame<S: Sink + ?Sized>(sink: &mut S, doc: &[u8]) -> Result<(), Error> {
    let len = u64::try_from(doc.len()).unwrap_or(u64::MAX);
    let mut header = [0u8; FRAME_HEADER_LEN];
    let (magic, size) = header.split_at_mut(FRAME_MAGIC.len());
    magic.copy_from_slice(&FRAME_MAGIC);
    size.copy_from_slice(&len.to_le_bytes());
    sink.write_bytes(&header).map_err(Error::without_position)?;
    if !doc.is_empty() {
        sink.write_bytes(doc).map_err(Error::without_position)?;
    }
    Ok(())
}

/// Splits pushed bytes into frame payloads, for transports that hand data
/// over in arbitrary pieces.
///
/// A payload buffer grows with the bytes that actually arrive, never with
/// the declared length, so a crafted header cannot force a large allocation
/// up front; it is bounded by the cap given to [`FrameDecoder::new`].
#[derive(Debug, Clone)]
pub struct FrameDecoder {
    max: u64,
    header: [u8; FRAME_HEADER_LEN],
    header_len: usize,
    /// Payload bytes still to come, when inside a payload.
    remaining: Option<usize>,
    payload: Vec<u8>,
    error: Option<FrameError>,
}

impl FrameDecoder {
    /// A decoder accepting payloads of at most `max_document_size` bytes; `0`
    /// selects [`DEFAULT_MAX_DOCUMENT_SIZE`], as in C. Pass `u64::MAX` to lift
    /// the cap.
    #[must_use]
    pub fn new(max_document_size: u64) -> Self {
        Self {
            max: effective_max(max_document_size),
            header: [0; FRAME_HEADER_LEN],
            header_len: 0,
            remaining: None,
            payload: Vec::new(),
            error: None,
        }
    }

    /// Consumes bytes from the front of `input` until a payload is complete
    /// or `input` is empty. Returns the payload, or `None` when more input is
    /// needed. Call it again with the rest of `input` until it returns
    /// `None`.
    ///
    /// # Errors
    ///
    /// [`FrameError::BadMagic`] or [`FrameError::TooLarge`]; the error is
    /// sticky.
    pub fn decode(&mut self, input: &mut &[u8]) -> Result<Option<Vec<u8>>, FrameError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let remaining = match self.remaining {
            Some(remaining) => remaining,
            None => match self.header(input) {
                Ok(Some(len)) => len,
                Ok(None) => return Ok(None),
                Err(error) => {
                    self.error = Some(error);
                    return Err(error);
                }
            },
        };
        let (now, rest) = input.split_at(remaining.min(input.len()));
        self.payload.extend_from_slice(now);
        *input = rest;
        let remaining = remaining - now.len();
        if remaining == 0 {
            self.remaining = None;
            Ok(Some(core::mem::take(&mut self.payload)))
        } else {
            self.remaining = Some(remaining);
            Ok(None)
        }
    }

    /// Fills the header from `input`; returns the payload length once the
    /// header is complete.
    fn header(&mut self, input: &mut &[u8]) -> Result<Option<usize>, FrameError> {
        let free = self.header.get_mut(self.header_len..).unwrap_or_default();
        let n = free.len().min(input.len());
        let (now, rest) = input.split_at(n);
        free.get_mut(..n).unwrap_or_default().copy_from_slice(now);
        *input = rest;
        self.header_len += n;
        if self.header_len < FRAME_HEADER_LEN {
            return Ok(None);
        }
        self.header_len = 0;
        let len = parse_header(&self.header, self.max)?;
        self.remaining = Some(len);
        Ok(Some(len))
    }

    /// Checks that the input ended on a frame boundary (a clean end of the
    /// sequence).
    ///
    /// # Errors
    ///
    /// [`FrameError::Truncated`] when it ended inside a frame, or the
    /// decoder's earlier error.
    pub fn finish(&self) -> Result<(), FrameError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if self.header_len > 0 || self.remaining.is_some() {
            return Err(FrameError::Truncated);
        }
        Ok(())
    }
}

fn effective_max(max_document_size: u64) -> u64 {
    if max_document_size == 0 {
        DEFAULT_MAX_DOCUMENT_SIZE
    } else {
        max_document_size
    }
}

/// Validates a complete header and returns its payload length.
fn parse_header(header: &[u8; FRAME_HEADER_LEN], max: u64) -> Result<usize, FrameError> {
    let (magic, size) = header.split_at(FRAME_MAGIC.len());
    if magic != FRAME_MAGIC {
        return Err(FrameError::BadMagic);
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(size);
    let len = u64::from_le_bytes(bytes);
    if len > max {
        return Err(FrameError::TooLarge { len, max });
    }
    // On 32-bit targets a 64-bit length may not fit in memory at all.
    usize::try_from(len).map_err(|_| FrameError::TooLarge { len, max })
}

/// Writes `.key = <doc as octets>` (`bvnr_embed_document`): a complete
/// document carried as the opaque octet-stream payload of a key.
///
/// To read it back, take the value's bytes
/// ([`Value::as_bytes`](crate::Value::as_bytes), or the
/// [`Event::OctetStreamChunk`]s of the key) and parse them with an ordinary
/// reader, such as [`parse`](crate::parse).
///
/// ```
/// use bovnar::{stream::embed_document, Writer};
///
/// let mut writer = Writer::new(Vec::new(), false);
/// embed_document(&mut writer, b"inner", b".x = 1;")?;
/// writer.finish()?;
///
/// let outer = bovnar::parse(&writer.into_sink())?;
/// let bytes = outer.get("inner").and_then(|v| v.as_bytes()).unwrap_or_default();
/// let inner = bovnar::parse(bytes)?;
/// assert_eq!(inner.get("x").and_then(|v| v.as_u64()), Some(1));
/// # Ok::<(), bovnar::Error>(())
/// ```
///
/// # Errors
///
/// The writer's error, e.g. a key that is not a valid identifier.
pub fn embed_document<S: Sink>(
    writer: &mut Writer<S>,
    key: &[u8],
    doc: &[u8],
) -> Result<(), Error> {
    writer.write_event(&Event::AssignmentStart { key })?;
    writer.write_event(&Event::OctetStreamStart)?;
    for chunk in doc.chunks(EMBED_CHUNK) {
        writer.write_event(&Event::OctetStreamChunk(chunk))?;
    }
    writer.write_event(&Event::OctetStreamEnd)
}
