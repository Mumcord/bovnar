//! The streaming reader: lexer, validator and event stream.
//!
//! Input is pushed in chunks of any size with [`Reader::feed`]; the verified
//! events of each chunk are then pulled with [`Reader::next_event`]. Only the
//! events of the current chunk are held, never the whole document, so a
//! caller bounds memory by bounding its chunk size.

mod annotation;
mod lexer;
mod options;
mod queue;
mod validator;

pub(crate) use self::lexer::validate_utf8;
use self::lexer::Lexer;
pub use self::options::ReadOptions;
use self::queue::QueuedEvent;
use self::validator::Validator;
use crate::event::Event;
use crate::{Error, Limits, SpecVersion};

/// Chunk size [`read_all`] feeds the reader with.
const READ_ALL_CHUNK: usize = 64 * 1024;

/// An incremental, push-based Bovnar reader (`bvnr_read` in the C reference).
///
/// ```
/// use bovnar::{Event, Limits, Reader};
///
/// let mut reader = Reader::new(&Limits::default());
/// let mut keys = Vec::new();
/// for chunk in [&b".a = 1; .b"[..], &b" = \"x\";"[..]] {
///     reader.feed(chunk)?;
///     while let Some(event) = reader.next_event() {
///         if let Event::AssignmentStart { key } = event {
///             keys.push(key.to_vec());
///         }
///     }
/// }
/// reader.finish()?;
/// while reader.next_event().is_some() {}
/// assert_eq!(keys, [b"a".to_vec(), b"b".to_vec()]);
/// # Ok::<(), bovnar::Error>(())
/// ```
pub struct Reader {
    lexer: Lexer,
    validator: Validator,
    current: Option<QueuedEvent>,
}

impl Reader {
    /// A reader that stops at the first error.
    pub fn new(limits: &Limits) -> Self {
        Self::build(limits, false)
    }

    /// A reader in recovery mode (`continue_on_error`, doc/08): after a
    /// syntax error it skips to the next point it can resynchronise on,
    /// reports the error as [`Event::Recovered`] and carries on.
    pub fn with_recovery(limits: &Limits) -> Self {
        Self::build(limits, true)
    }

    /// A reader configured by `options`: limits, recovery mode and the
    /// `strict_version` / `text_only` policies.
    ///
    /// ```
    /// use bovnar::{ErrorKind, ReadOptions, Reader};
    ///
    /// let mut reader = Reader::with_options(&ReadOptions::new().text_only(true));
    /// let err = reader.feed(b".blob = \x00").unwrap_err();
    /// assert_eq!(err.kind(), ErrorKind::OctetStreamForbidden);
    /// assert_eq!((err.line(), err.column()), (1, 9));
    /// ```
    pub fn with_options(options: &ReadOptions) -> Self {
        let mut reader = Self::build(&options.limits, options.recovery);
        reader
            .lexer
            .set_policy(options.strict_version, options.text_only);
        reader
    }

    fn build(limits: &Limits, continue_on_error: bool) -> Self {
        Self {
            lexer: Lexer::new(limits, continue_on_error),
            validator: Validator::new(),
            current: None,
        }
    }

    /// Reads the next bytes of the document.
    ///
    /// Events the chunk completes become available through
    /// [`Reader::next_event`]; on error, the events verified before it stay
    /// available too.
    ///
    /// # Errors
    ///
    /// The first error in the document; it is returned again by every later
    /// call.
    pub fn feed(&mut self, chunk: &[u8]) -> Result<(), Error> {
        self.validator.queue.reset_arena();
        self.lexer.feed(&mut self.validator, chunk)
    }

    /// Signals the end of the document and checks that it is complete.
    ///
    /// # Errors
    ///
    /// The document is incomplete or an earlier error occurred.
    pub fn finish(&mut self) -> Result<(), Error> {
        self.validator.queue.reset_arena();
        self.lexer.finish(&mut self.validator)
    }

    /// The next verified event, or `None` when the input fed so far has no
    /// more events.
    #[allow(clippy::should_implement_trait)]
    pub fn next_event(&mut self) -> Option<Event<'_>> {
        self.current = self.validator.queue.pop();
        let event = self.current.as_ref()?;
        Some(self.validator.queue.resolve(event))
    }

    /// Whether [`Reader::next_event`] has an event ready. Lets a pull loop
    /// decide to feed more input without holding a borrowed event.
    pub fn has_event(&self) -> bool {
        !self.validator.queue.is_empty()
    }

    /// The version the document declared with `#!bovnar M.N`, if any.
    pub fn declared_version(&self) -> Option<SpecVersion> {
        self.validator.declared_version
    }

    /// How many errors recovery mode skipped past.
    pub fn recovery_count(&self) -> u64 {
        self.lexer.recovery_count()
    }

    /// How many input bytes recovery mode skipped.
    pub fn skipped_bytes(&self) -> u64 {
        self.lexer.skipped_bytes()
    }
}

/// Reads a whole document from memory, handing each verified event to
/// `on_event` in document order.
///
/// ```
/// use bovnar::{read_all, Event, Limits};
///
/// let mut values = Vec::new();
/// read_all(b".x = <uint:8> 42;", &Limits::default(), |event| {
///     if let Event::Data { text, .. } = event {
///         values.push(text.to_vec());
///     }
/// })?;
/// assert_eq!(values, [b"42".to_vec()]);
/// # Ok::<(), bovnar::Error>(())
/// ```
///
/// # Errors
///
/// The first error in the document. Events verified before it have already
/// been handed out, as the C reference's callback does.
pub fn read_all(
    input: &[u8],
    limits: &Limits,
    on_event: impl FnMut(Event<'_>),
) -> Result<(), Error> {
    read_all_with(input, &ReadOptions::new().limits(limits.clone()), on_event)
}

/// [`read_all`] under `options`.
///
/// ```
/// use bovnar::{read_all_with, ErrorKind, ReadOptions};
///
/// let strict = ReadOptions::new().strict_version(true);
/// let err = read_all_with(b"#!bovnar 2.0\n.a = 1;", &strict, |_| {}).unwrap_err();
/// assert_eq!(err.kind(), ErrorKind::UnsupportedSpecVersion);
/// ```
///
/// # Errors
///
/// As [`read_all`]. In recovery mode, errors the reader skips past arrive as
/// [`Event::Recovered`] instead.
pub fn read_all_with(
    input: &[u8],
    options: &ReadOptions,
    mut on_event: impl FnMut(Event<'_>),
) -> Result<(), Error> {
    let mut reader = Reader::with_options(options);
    let mut drain = |reader: &mut Reader, result: Result<(), Error>| {
        while let Some(event) = reader.next_event() {
            on_event(event);
        }
        result
    };
    for chunk in input.chunks(READ_ALL_CHUNK) {
        let result = reader.feed(chunk);
        drain(&mut reader, result)?;
    }
    let result = reader.finish();
    drain(&mut reader, result)
}
