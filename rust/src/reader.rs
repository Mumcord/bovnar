//! The streaming reader, mirroring `python/bovnar/reader.py`.

use crate::enums::{ErrorCode, Event, TokenType};
use crate::error::{Error, Result};
use crate::ffi::{self, BvnrReadFlags, ValueTypeSpec};

/// Options for [`Reader::read_mem`], the keyword arguments of Python's
/// `Reader.read_mem`. The default reads with the library's limits.
#[derive(Clone, Debug, Default)]
pub struct ReadOptions {
    /// Largest accepted input in bytes; 0 is the library default.
    pub max_file_size: u64,
    /// Recover from errors and keep reading instead of stopping.
    pub continue_on_error: bool,
    /// Refuse a document whose `#!bovnar` directive names a newer version.
    pub strict_version: bool,
    /// Refuse a document containing an octet stream.
    pub text_only: bool,
}

/// One event as a handler sees it, borrowed for the duration of the call
/// (Python's `(Event, BvnrData)` callback arguments).
#[derive(Clone, Copy, Debug)]
pub struct EventData<'a> {
    pub event: Event,
    /// The token type of the payload (`BvnrData.type`), when it has one.
    pub token_type: Option<TokenType>,
    pub value_type: ValueTypeSpec,
    /// The payload bytes: a key, a value literal or an octet chunk.
    pub raw: &'a [u8],
}

impl EventData<'_> {
    /// The payload as text, with invalid UTF-8 replaced.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(self.raw).into_owned()
    }
}

/// An owned copy of one event, as collected by [`Reader::iter_mem`]
/// (Python's `EventPayload`).
#[derive(Clone, Debug, PartialEq)]
pub struct EventPayload {
    pub event: Event,
    pub token_type: Option<TokenType>,
    pub value_type: ValueTypeSpec,
    pub raw: Vec<u8>,
}

impl EventPayload {
    /// The payload as text, with invalid UTF-8 replaced.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.raw).into_owned()
    }
}

impl From<&EventData<'_>> for EventPayload {
    fn from(d: &EventData<'_>) -> Self {
        EventPayload {
            event: d.event,
            token_type: d.token_type,
            value_type: d.value_type,
            raw: d.raw.to_vec(),
        }
    }
}

/// A handler for reader events; returning false stops the read.
pub type EventHandler<'h> = &'h mut dyn FnMut(&EventData<'_>) -> bool;

/// A streaming BVNR reader.
pub struct Reader {
    handle: ffi::ReaderHandle,
}

impl Reader {
    /// Creates a reader.
    pub fn new() -> Result<Self> {
        let handle = ffi::ReaderHandle::new()
            .ok_or_else(|| Error::Argument("bvnr_reader_create() returned NULL".into()))?;
        Ok(Reader { handle })
    }

    /// Reads `data`, calling `on_verified` for every event the validator
    /// accepted and `on_unverified` for every raw lexer event.
    pub fn read_mem<'h>(
        &mut self,
        data: &[u8],
        options: &ReadOptions,
        on_verified: Option<EventHandler<'h>>,
        on_unverified: Option<EventHandler<'h>>,
    ) -> Result<()> {
        let mut flags = BvnrReadFlags::zeroed();
        flags.max_file_size = options.max_file_size;
        // Reader.read_mem opens at the 255-level cap in both directions, like
        // the CLI and the DOM; zero would select the C default of 64.
        flags.max_struct_nesting = 255;
        flags.max_array_nesting = 255;
        flags.continue_on_error = options.continue_on_error;
        flags.strict_version = options.strict_version;
        flags.text_only = options.text_only;

        let mut verified = on_verified.map(wrap);
        let mut unverified = on_unverified.map(wrap);
        let ok = self.handle.read_mem(
            data,
            flags,
            verified.as_mut().map(|f| f as ffi::Handler<'_>),
            unverified.as_mut().map(|f| f as ffi::Handler<'_>),
        );
        if ok { Ok(()) } else { Err(self.error()) }
    }

    /// Reads `data` and returns its events, verified ones only unless
    /// `verified_only` is false.
    pub fn iter_mem(
        &mut self,
        data: &[u8],
        verified_only: bool,
        max_file_size: u64,
    ) -> Result<Vec<EventPayload>> {
        let events = std::cell::RefCell::new(Vec::new());
        let mut collect = |d: &EventData<'_>| {
            events.borrow_mut().push(EventPayload::from(d));
            true
        };
        let mut collect_raw = |d: &EventData<'_>| {
            events.borrow_mut().push(EventPayload::from(d));
            true
        };
        let options = ReadOptions {
            max_file_size,
            ..ReadOptions::default()
        };
        let unverified: Option<EventHandler<'_>> = if verified_only {
            None
        } else {
            Some(&mut collect_raw)
        };
        self.read_mem(data, &options, Some(&mut collect), unverified)?;
        Ok(events.into_inner())
    }

    /// The code of the last error, `ErrorCode::None` after a clean read.
    pub fn error_code(&self) -> Option<ErrorCode> {
        ErrorCode::from_raw(self.handle.error().code)
    }

    fn error(&self) -> Error {
        let e = self.handle.error();
        Error::Parse {
            code: e.code,
            line: e.line,
            column: e.column,
            offset: e.offset,
            byte: e.byte,
        }
    }
}

/// Adapts a public handler to the raw event view the FFI layer delivers.
fn wrap<'h>(f: EventHandler<'h>) -> impl FnMut(&ffi::EventView<'_>) -> bool + 'h {
    move |view| {
        let Some(event) = Event::from_raw(view.event) else {
            // An event this binding does not know yet: skip it, keep reading.
            return true;
        };
        let (token_type, value_type) = view.data.map_or((None, ValueTypeSpec::default()), |d| {
            (TokenType::from_raw(d.type_), d.value_type)
        });
        f(&EventData {
            event,
            token_type,
            value_type,
            raw: view.raw,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iter_mem_yields_key_and_value() {
        let events = Reader::new()
            .unwrap()
            .iter_mem(b".a = 1;", true, 0)
            .unwrap();
        let texts: Vec<(Event, String)> = events.iter().map(|e| (e.event, e.text())).collect();
        assert!(
            texts.contains(&(Event::AssignmentStart, "a".into())),
            "{texts:?}"
        );
        assert!(texts.contains(&(Event::Data, "1".into())), "{texts:?}");
    }

    #[test]
    fn handler_returning_false_stops_the_read() {
        let mut stop = |_: &EventData<'_>| false;
        let err = Reader::new()
            .unwrap()
            .read_mem(b".a = 1;", &ReadOptions::default(), Some(&mut stop), None)
            .unwrap_err();
        assert!(matches!(err, Error::Parse { .. }), "{err:?}");
    }
}
