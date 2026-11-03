//! Pull adapters over [`std::io::Read`].

use std::io::{self, Read};
use std::vec::Vec;

use super::{effective_max, parse_header, FrameError, FRAME_HEADER_LEN};
use crate::event::Event;
use crate::reader::{read_all_with, ReadOptions, Reader};
use crate::Error;

/// Chunk size [`ReadAdapter`] reads with by default.
const DEFAULT_CHUNK: usize = 64 * 1024;

/// What can go wrong while reading from an [`std::io::Read`].
#[derive(Debug)]
#[non_exhaustive]
pub enum StreamError {
    /// The source failed.
    Io(io::Error),
    /// The document is malformed ([`ReadAdapter`]).
    Read(Error),
    /// The frame stream is malformed.
    Frame(FrameError),
    /// Document `index` (0-based) of a frame stream failed to read and
    /// [`DocStreamOptions::continue_past_failed`] is off.
    Document {
        /// Position of the document in the stream.
        index: u64,
        /// Its first error.
        error: Error,
    },
    /// The `on_document` callback asked to stop after document `index`.
    Stopped {
        /// Position of the document in the stream.
        index: u64,
    },
}

impl core::fmt::Display for StreamError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "reading the source failed: {e}"),
            Self::Read(e) => e.fmt(f),
            Self::Frame(e) => e.fmt(f),
            Self::Document { index, error } => write!(f, "document {index}: {error}"),
            Self::Stopped { index } => write!(f, "stopped after document {index}"),
        }
    }
}

impl std::error::Error for StreamError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Read(e) | Self::Document { error: e, .. } => Some(e),
            Self::Frame(e) => Some(e),
            Self::Stopped { .. } => None,
        }
    }
}

impl From<io::Error> for StreamError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<FrameError> for StreamError {
    fn from(e: FrameError) -> Self {
        Self::Frame(e)
    }
}

/// Where a [`ReadAdapter`] stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Reading,
    Done,
}

/// Pulls one document from an [`std::io::Read`] in chunks and hands out its
/// events, so a document of any length (also an endless one) is processed
/// in constant memory.
///
/// ```
/// use bovnar::{stream::ReadAdapter, Event, ReadOptions};
///
/// let source: &[u8] = b".a = 1; .b = 2;";
/// let mut adapter = ReadAdapter::new(source, &ReadOptions::new());
/// let mut keys = 0;
/// while let Some(event) = adapter.next_event() {
///     if let Event::AssignmentStart { .. } = event? {
///         keys += 1;
///     }
/// }
/// assert_eq!(keys, 2);
/// # Ok::<(), bovnar::stream::StreamError>(())
/// ```
pub struct ReadAdapter<R> {
    source: R,
    reader: Reader,
    buf: Vec<u8>,
    phase: Phase,
    error: Option<StreamError>,
}

impl<R: Read> ReadAdapter<R> {
    /// An adapter reading `source` in 64 KiB chunks.
    pub fn new(source: R, options: &ReadOptions) -> Self {
        Self::with_chunk_size(source, options, DEFAULT_CHUNK)
    }

    /// An adapter reading `source` in chunks of `chunk_size` bytes (at least
    /// one). Smaller chunks lower latency and memory, larger ones overhead.
    pub fn with_chunk_size(source: R, options: &ReadOptions, chunk_size: usize) -> Self {
        Self {
            source,
            reader: Reader::with_options(options),
            buf: std::vec![0; chunk_size.max(1)],
            phase: Phase::Reading,
            error: None,
        }
    }

    /// The next event, reading more input as needed; `None` once the
    /// document has ended or an error was returned.
    ///
    /// Events verified before an error are handed out first, then the error
    /// once.
    #[allow(clippy::should_implement_trait)]
    pub fn next_event(&mut self) -> Option<Result<Event<'_>, StreamError>> {
        loop {
            if self.reader.has_event() {
                return self.reader.next_event().map(Ok);
            }
            if let Some(error) = self.error.take() {
                return Some(Err(error));
            }
            if self.phase == Phase::Done {
                return None;
            }
            self.pull();
        }
    }

    /// Reads and feeds one chunk, or finishes the document at the end of the
    /// source.
    fn pull(&mut self) {
        let fed = match self.source.read(&mut self.buf) {
            Ok(0) => {
                self.phase = Phase::Done;
                self.reader.finish()
            }
            Ok(n) => self.reader.feed(self.buf.get(..n).unwrap_or_default()),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(()),
            Err(e) => {
                self.phase = Phase::Done;
                self.error = Some(StreamError::Io(e));
                return;
            }
        };
        if let Err(e) = fed {
            self.phase = Phase::Done;
            self.error = Some(StreamError::Read(e));
        }
    }

    /// The underlying reader, e.g. for
    /// [`Reader::declared_version`] or [`Reader::recovery_count`].
    pub fn reader(&self) -> &Reader {
        &self.reader
    }
}

/// Pulls frame payloads from an [`std::io::Read`].
///
/// It reads exactly one frame at a time and never past it, so the source
/// can be taken back with [`FrameReader::into_inner`] between frames.
#[derive(Debug)]
pub struct FrameReader<R> {
    source: R,
    max: u64,
}

impl<R: Read> FrameReader<R> {
    /// A reader accepting payloads of at most `max_document_size` bytes; `0`
    /// selects [`DEFAULT_MAX_DOCUMENT_SIZE`](super::DEFAULT_MAX_DOCUMENT_SIZE).
    pub fn new(source: R, max_document_size: u64) -> Self {
        Self {
            source,
            max: effective_max(max_document_size),
        }
    }

    /// The next payload, or `None` at a clean end of input on a frame
    /// boundary.
    ///
    /// # Errors
    ///
    /// An I/O error, or a [`FrameError`] (an end of input inside a frame is
    /// [`FrameError::Truncated`]).
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, StreamError> {
        let mut header = [0u8; FRAME_HEADER_LEN];
        let mut have = 0;
        while let Some(free) = header.get_mut(have..).filter(|f| !f.is_empty()) {
            match self.source.read(free) {
                Ok(0) if have == 0 => return Ok(None),
                Ok(0) => return Err(FrameError::Truncated.into()),
                Ok(n) => have += n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e.into()),
            }
        }
        let len = parse_header(&header, self.max)?;
        // `take` + `read_to_end` grows the buffer with the bytes that arrive,
        // so a crafted length cannot force a large allocation up front.
        let mut payload = Vec::new();
        let limit = u64::try_from(len).unwrap_or(u64::MAX);
        (&mut self.source).take(limit).read_to_end(&mut payload)?;
        if payload.len() != len {
            return Err(FrameError::Truncated.into());
        }
        Ok(Some(payload))
    }

    /// The source, positioned after the last frame read.
    pub fn into_inner(self) -> R {
        self.source
    }
}

/// Options for [`read_doc_stream`] (`bvnr_doc_stream_opts_t`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct DocStreamOptions {
    /// How each document is read.
    pub read: ReadOptions,
    /// Report a failed document and go on with the next frame instead of
    /// stopping. Independent of [`ReadOptions::recovery`], which recovers
    /// within one document.
    pub continue_past_failed: bool,
    /// Largest payload; `0` selects
    /// [`DEFAULT_MAX_DOCUMENT_SIZE`](super::DEFAULT_MAX_DOCUMENT_SIZE).
    pub max_document_size: u64,
}

impl DocStreamOptions {
    /// Default read options, stop at the first failed document, 256 MiB cap.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the per-document read options.
    #[must_use]
    pub fn read(mut self, read: ReadOptions) -> Self {
        self.read = read;
        self
    }

    /// Sets [`DocStreamOptions::continue_past_failed`].
    #[must_use]
    pub fn continue_past_failed(mut self, on: bool) -> Self {
        self.continue_past_failed = on;
        self
    }

    /// Sets [`DocStreamOptions::max_document_size`].
    #[must_use]
    pub fn max_document_size(mut self, max: u64) -> Self {
        self.max_document_size = max;
        self
    }
}

/// Reads a frame stream to its end (`bvnr_doc_stream_read`), checking each
/// payload as a standalone document and calling `on_document` with its
/// 0-based index, its bytes and the verdict. Returning `false` from the
/// callback stops the stream.
///
/// Returns how many documents were read. Parse the bytes with
/// [`parse_with_options`](crate::parse_with_options) to get a tree.
///
/// ```
/// use bovnar::stream::{read_doc_stream, write_frame, DocStreamOptions};
///
/// let mut wire = Vec::new();
/// write_frame(&mut wire, b".ok = 1;")?;
/// write_frame(&mut wire, b".broken = <uint:8> 256;")?;
/// write_frame(&mut wire, b".ok = 2;")?;
///
/// let options = DocStreamOptions::new().continue_past_failed(true);
/// let mut failed = Vec::new();
/// let count = read_doc_stream(&wire[..], &options, |index, _bytes, verdict| {
///     if verdict.is_err() {
///         failed.push(index);
///     }
///     true
/// })?;
/// assert_eq!((count, failed), (3, vec![1]));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// As in C, the sequence stops at a framing or I/O error, at the first
/// failed document unless [`DocStreamOptions::continue_past_failed`] is set
/// ([`StreamError::Document`]), or when the callback returns `false`
/// ([`StreamError::Stopped`]). Documents up to and including the reported
/// index were handed to the callback.
pub fn read_doc_stream<R: Read>(
    source: R,
    options: &DocStreamOptions,
    mut on_document: impl FnMut(u64, &[u8], Result<(), Error>) -> bool,
) -> Result<u64, StreamError> {
    let mut frames = FrameReader::new(source, options.max_document_size);
    let mut count: u64 = 0;
    while let Some(payload) = frames.next_frame()? {
        let index = count;
        count = count.saturating_add(1);
        let verdict = read_all_with(&payload, &options.read, |_| {});
        if !on_document(index, &payload, verdict) {
            return Err(StreamError::Stopped { index });
        }
        if let Err(error) = verdict {
            if !options.continue_past_failed {
                return Err(StreamError::Document { index, error });
            }
        }
    }
    Ok(count)
}
