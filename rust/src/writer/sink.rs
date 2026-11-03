//! Destinations a [`super::Writer`] writes to.

use alloc::vec::Vec;

use crate::ErrorKind;

/// A byte destination for the [`super::Writer`].
///
/// Octet streams are written as raw bytes, so a sink takes bytes, not text.
pub trait Sink {
    /// Writes all of `bytes`.
    ///
    /// # Errors
    ///
    /// The error the writer reports, normally [`ErrorKind::WritingToSink`].
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), ErrorKind>;

    /// Called once when the writer finishes.
    ///
    /// # Errors
    ///
    /// As [`Sink::write_bytes`].
    fn flush(&mut self) -> Result<(), ErrorKind> {
        Ok(())
    }
}

impl Sink for Vec<u8> {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), ErrorKind> {
        self.extend_from_slice(bytes);
        Ok(())
    }
}

impl<S: Sink + ?Sized> Sink for &mut S {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), ErrorKind> {
        (**self).write_bytes(bytes)
    }

    fn flush(&mut self) -> Result<(), ErrorKind> {
        (**self).flush()
    }
}

/// Adapts a [`core::fmt::Write`] (a `String`, a formatter) as a sink.
///
/// The output must be UTF-8: a document whose octet streams are not fails
/// with [`ErrorKind::InvalidUtf8Byte`]. A character split across two writes
/// is reassembled.
pub struct FmtSink<W> {
    inner: W,
    /// The start of a UTF-8 sequence the last write ended in.
    tail: [u8; 4],
    tail_len: usize,
}

impl<W: core::fmt::Write> FmtSink<W> {
    /// Wraps `inner`.
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            tail: [0; 4],
            tail_len: 0,
        }
    }

    /// The wrapped writer.
    pub fn into_inner(self) -> W {
        self.inner
    }

    fn put(&mut self, text: &str) -> Result<(), ErrorKind> {
        self.inner
            .write_str(text)
            .map_err(|_| ErrorKind::WritingToSink)
    }
}

impl<W: core::fmt::Write> Sink for FmtSink<W> {
    fn write_bytes(&mut self, mut bytes: &[u8]) -> Result<(), ErrorKind> {
        // Complete a sequence the previous write left open, byte by byte.
        while self.tail_len > 0 {
            let Some((&b, rest)) = bytes.split_first() else {
                return Ok(());
            };
            let slot = self
                .tail
                .get_mut(self.tail_len)
                .ok_or(ErrorKind::InvalidUtf8Byte)?;
            *slot = b;
            self.tail_len += 1;
            bytes = rest;
            let tail = self.tail;
            match core::str::from_utf8(tail.get(..self.tail_len).unwrap_or_default()) {
                Ok(s) => {
                    self.tail_len = 0;
                    self.put(s)?;
                }
                Err(e) if e.error_len().is_some() => return Err(ErrorKind::InvalidUtf8Byte),
                Err(_) => {}
            }
        }
        match core::str::from_utf8(bytes) {
            Ok(s) => self.put(s),
            Err(e) if e.error_len().is_some() => Err(ErrorKind::InvalidUtf8Byte),
            Err(e) => {
                let (valid, rest) = bytes.split_at(e.valid_up_to());
                self.put(core::str::from_utf8(valid).map_err(|_| ErrorKind::InvalidUtf8Byte)?)?;
                let slot = self
                    .tail
                    .get_mut(..rest.len())
                    .ok_or(ErrorKind::InvalidUtf8Byte)?;
                slot.copy_from_slice(rest);
                self.tail_len = rest.len();
                Ok(())
            }
        }
    }

    fn flush(&mut self) -> Result<(), ErrorKind> {
        if self.tail_len > 0 {
            return Err(ErrorKind::InvalidUtf8Byte);
        }
        Ok(())
    }
}

/// Adapts a [`std::io::Write`] as a sink.
#[cfg(feature = "std")]
pub struct IoSink<W>(W);

#[cfg(feature = "std")]
impl<W: std::io::Write> IoSink<W> {
    /// Wraps `inner`.
    pub fn new(inner: W) -> Self {
        Self(inner)
    }

    /// The wrapped writer.
    pub fn into_inner(self) -> W {
        self.0
    }
}

#[cfg(feature = "std")]
impl<W: std::io::Write> Sink for IoSink<W> {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), ErrorKind> {
        self.0
            .write_all(bytes)
            .map_err(|_| ErrorKind::WritingToSink)
    }

    fn flush(&mut self) -> Result<(), ErrorKind> {
        self.0.flush().map_err(|_| ErrorKind::WritingToSink)
    }
}
