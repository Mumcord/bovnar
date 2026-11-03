//! The event-based writer (`bvnr_write_event` in the C reference) and the
//! document serialisation built on it.
//!
//! The writer takes the same [`Event`]s the reader produces, so a reader can
//! drive it directly (that is what `bovnar pretty-print` does), and it
//! validates everything it writes: a stream of events it accepts always
//! produces a document the reader accepts with the same values.

mod probe;
mod sink;
mod tree;

use alloc::format;
use alloc::vec::Vec;

#[cfg(feature = "std")]
pub use sink::IoSink;
pub use sink::{FmtSink, Sink};
pub use tree::{to_string, to_string_compact, to_vec, to_vec_compact};

use crate::event::{Event, TokenKind, TypeAnnotation};
use crate::types::{TypeFamily, TypeSpec};
use crate::{num, Error, ErrorKind, Limits, SpecVersion};

/// Longest annotation body between `<` and `>` (`BVN_TYPE_BUF_CAP - 1`).
const MAX_ANNOTATION: usize = 255;
/// Largest octet-stream chunk (a 16-bit length, 0 meaning 65536).
const MAX_CHUNK: usize = 65_536;

/// The annotation that governs the values after it: its text, for
/// validating a value against it, and its type.
#[derive(Clone)]
struct Gov {
    text: Vec<u8>,
    spec: TypeSpec,
}

/// Per-array-depth state (the C writer's `arr_*` arrays). Entries persist
/// across the rows of a `/` matrix, which close and reopen the depth.
#[derive(Clone, Default)]
struct Level {
    struct_depth_at_start: usize,
    need_comma: bool,
    emitted_unit: bool,
    gov: Option<Gov>,
    count: u64,
    dim_set: bool,
    dim_count: u64,
}

/// Writes a Bovnar document from [`Event`]s.
///
/// ```
/// use bovnar::{Event, TokenKind, TypeSpec, Writer};
///
/// let mut writer = Writer::new(Vec::new(), true);
/// writer.write_event(&Event::AssignmentStart { key: b"greeting" })?;
/// writer.write_event(&Event::Data {
///     kind: TokenKind::String,
///     text: b"hello",
///     frac: b"",
///     value_type: TypeSpec::PLAIN,
///     unit: b"",
///     inline_unit: b"",
/// })?;
/// writer.finish()?;
/// assert_eq!(writer.into_sink(), b".greeting = \"hello\";\n");
/// # Ok::<(), bovnar::Error>(())
/// ```
///
/// Events out of place (a struct end with no struct open, a value where a
/// key belongs) fail with [`ErrorKind::UnknownTokenType`]; a value that would
/// not read back fails with the error the reader would report for it. The
/// first error is sticky: every later call returns it again.
pub struct Writer<S: Sink> {
    sink: S,
    pretty: bool,
    limits: Limits,
    error: Option<Error>,
    position: u64,
    version: Option<SpecVersion>,
    stream_begun: bool,
    stream_ended: bool,
    finished: bool,
    need_semi: bool,
    indent: usize,
    had_type_annotation: bool,
    emitted_unit: bool,
    struct_depth: usize,
    array_depth: usize,
    levels: Vec<Level>,
    in_octet_stream: bool,
    awaiting_value: bool,
    after_row_end: bool,
    after_dim: bool,
    gov: Option<Gov>,
    /// The datetime annotation an untyped plain integer inherits: the
    /// reader types such an integer like the value before it in the same
    /// assignment when that one was a datetime (its
    /// `inferred_default_vtype`), so `[<datetime> 1, 2]` holds two datetimes.
    datetime_carry: Option<Vec<u8>>,
}

impl<S: Sink> Writer<S> {
    /// A writer to `sink`, pretty-printed (tabs, one assignment per line) or
    /// compact, nesting at most 64 levels like the C reference's default.
    pub fn new(sink: S, pretty: bool) -> Self {
        Self::with_limits(sink, pretty, &Limits::default())
    }

    /// A writer whose nesting and value limits are `limits`.
    pub fn with_limits(sink: S, pretty: bool, limits: &Limits) -> Self {
        Self {
            sink,
            pretty,
            limits: limits.clone(),
            error: None,
            position: 0,
            version: None,
            stream_begun: false,
            stream_ended: false,
            finished: false,
            need_semi: false,
            indent: 0,
            had_type_annotation: false,
            emitted_unit: false,
            struct_depth: 0,
            array_depth: 0,
            levels: Vec::new(),
            in_octet_stream: false,
            awaiting_value: false,
            after_row_end: false,
            after_dim: false,
            gov: None,
            datetime_carry: None,
        }
    }

    /// The sink, with everything written so far.
    pub fn into_sink(self) -> S {
        self.sink
    }

    /// Bytes written so far.
    pub fn bytes_written(&self) -> u64 {
        self.position
    }

    /// Writes the `#!bovnar M.N` version directive. Only allowed before the
    /// first event; values are then validated under that version.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::InvalidArgument`] once output has begun.
    pub fn write_version(&mut self, version: SpecVersion) -> Result<(), Error> {
        self.check_sticky()?;
        if self.finished || self.stream_begun || self.version.is_some() {
            return self.fail(ErrorKind::InvalidArgument);
        }
        let line = format!("#!bovnar {}.{}\n", version.major, version.minor);
        self.push(line.as_bytes())?;
        self.version = Some(version);
        Ok(())
    }

    /// Writes one event.
    ///
    /// # Errors
    ///
    /// The event is out of place or its value invalid; see [`Writer`].
    pub fn write_event(&mut self, event: &Event<'_>) -> Result<(), Error> {
        self.check_sticky()?;
        if self.finished {
            return self.fail(ErrorKind::InvalidArgument);
        }
        if !self.in_place(event) {
            return self.fail(ErrorKind::UnknownTokenType);
        }
        if !matches!(event, Event::ArrayRowEnd) {
            self.after_row_end = false;
        }
        let result = match *event {
            Event::StreamStart => {
                if self.stream_begun {
                    Err(ErrorKind::InvalidArgument)
                } else {
                    self.stream_begun = true;
                    Ok(())
                }
            }
            Event::StreamEnd => {
                self.stream_ended = true;
                Ok(())
            }
            Event::AssignmentStart { key } => self.assignment(key),
            Event::TypeAnnotation(ann) => self.annotation(&ann),
            Event::Data {
                kind,
                text,
                frac,
                inline_unit,
                ..
            } => self.data(kind, text, frac, inline_unit),
            Event::OctetStreamChunk(data) => self.octet_chunk(data),
            Event::StructStart => self.struct_start(),
            Event::StructEnd => self.struct_end(),
            Event::ArrayRowStart => self.row_start(),
            Event::ArrayRowEnd => self.row_end(),
            Event::ArrayDimStart => self.dim_start(),
            Event::OctetStreamStart => self.octet_start(),
            Event::OctetStreamEnd => self.octet_end(),
            Event::Recovered(_) => Err(ErrorKind::InvalidArgument),
        };
        result.or_else(|kind| self.fail(kind))
    }

    /// Ends the document: checks that nothing is left open and writes the
    /// final `;`.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::GotIncompleteBvnrStream`] when a struct, array, octet
    /// stream or assignment is still open.
    pub fn finish(&mut self) -> Result<(), Error> {
        self.check_sticky()?;
        if self.finished {
            return Ok(());
        }
        if self.struct_depth > 0
            || self.array_depth > 0
            || self.awaiting_value
            || self.in_octet_stream
            || self.after_dim
            || self.had_type_annotation
        {
            return self.fail(ErrorKind::GotIncompleteBvnrStream);
        }
        if self.need_semi {
            self.push(b";")?;
            self.newline()?;
            self.need_semi = false;
        }
        if let Err(kind) = self.sink.flush() {
            return self.fail(kind);
        }
        self.finished = true;
        Ok(())
    }

    fn check_sticky(&self) -> Result<(), Error> {
        match &self.error {
            Some(err) => Err(*err),
            None => Ok(()),
        }
    }

    fn fail<T>(&mut self, kind: ErrorKind) -> Result<T, Error> {
        let err = Error::new(kind, 0, 0, self.position);
        self.error = Some(err);
        Err(err)
    }

    // ---- placement (the gate at the top of bvn_writer_validate_event) ----

    fn is_direct(&self, idx: usize) -> bool {
        self.levels
            .get(idx)
            .is_some_and(|l| l.struct_depth_at_start == self.struct_depth)
    }

    fn in_array_directly(&self) -> bool {
        self.array_depth > 0 && self.is_direct(self.array_depth - 1)
    }

    fn value_position(&self) -> bool {
        self.in_array_directly() || self.awaiting_value || self.in_octet_stream
    }

    fn in_place(&self, event: &Event<'_>) -> bool {
        let ok = match event {
            Event::StreamStart => true,
            Event::TypeAnnotation(_) | Event::StructStart | Event::OctetStreamStart => {
                self.value_position() && !self.in_octet_stream
            }
            Event::AssignmentStart { .. } => {
                !self.awaiting_value && !self.in_array_directly() && !self.in_octet_stream
            }
            Event::Data { .. } => self.value_position() && !self.in_octet_stream,
            Event::OctetStreamChunk(_) => self.in_octet_stream,
            Event::ArrayRowStart => {
                !self.in_octet_stream && (self.value_position() || self.after_dim)
            }
            Event::ArrayRowEnd => !self.in_octet_stream && self.in_array_directly(),
            Event::ArrayDimStart => self.after_row_end,
            Event::OctetStreamEnd => self.in_octet_stream,
            Event::StructEnd => {
                !self.awaiting_value
                    && self.struct_depth > 0
                    && !self.in_octet_stream
                    && !self.in_array_directly()
            }
            Event::StreamEnd => {
                !self.awaiting_value
                    && self.array_depth == 0
                    && self.struct_depth == 0
                    && !self.in_octet_stream
            }
            Event::Recovered(_) => false,
        };
        let follows_annotation = matches!(
            event,
            Event::Data { .. }
                | Event::StructStart
                | Event::ArrayRowStart
                | Event::OctetStreamStart
        );
        ok && !self.stream_ended
            && (!self.had_type_annotation || follows_annotation)
            && (!self.after_dim || matches!(event, Event::ArrayRowStart))
    }

    // ---- events ----

    fn assignment(&mut self, key: &[u8]) -> Result<(), ErrorKind> {
        self.stream_begun = true;
        self.gov = None;
        self.datetime_carry = None;
        probe::key(key, self.version, &self.limits)?;
        self.awaiting_value = true;
        if self.need_semi {
            self.push_kind(b";")?;
            self.newline_kind()?;
        }
        self.indent_kind()?;
        self.push_kind(b".")?;
        self.push_kind(key)?;
        self.space_kind()?;
        self.push_kind(b"=")?;
        self.space_kind()?;
        self.need_semi = false;
        self.had_type_annotation = false;
        self.emitted_unit = false;
        Ok(())
    }

    fn annotation(&mut self, ann: &TypeAnnotation<'_>) -> Result<(), ErrorKind> {
        self.stream_begun = true;
        let unit = ann.unit.filter(|u| !u.is_empty());
        if unit.is_some() && !takes_unit(ann.family) {
            return Err(ErrorKind::IllegalValueType);
        }
        let (text, spec) = annotation_text(ann);
        if text.len().saturating_sub(2) > MAX_ANNOTATION {
            return Err(ErrorKind::TypeTooLong);
        }
        probe::annotation(&text, self.version, &self.limits)?;
        self.emit_pending_comma()?;
        self.push_kind(&text)?;
        self.space_kind()?;
        self.had_type_annotation = true;
        self.emitted_unit = unit.is_some() || ann.epoch.is_some();
        self.datetime_carry = (spec.family == TypeFamily::Datetime).then(|| text.clone());
        self.gov = Some(Gov { text, spec });
        Ok(())
    }

    fn data(
        &mut self,
        kind: TokenKind,
        text: &[u8],
        frac: &[u8],
        inline_unit: &[u8],
    ) -> Result<(), ErrorKind> {
        self.stream_begun = true;
        self.awaiting_value = false;
        let gov = self.gov.clone();
        if gov.is_some() && matches!(kind, TokenKind::Symbol | TokenKind::Reference) {
            return Err(ErrorKind::TypeValueMismatch);
        }
        let carries = matches!(
            kind,
            TokenKind::Number | TokenKind::ArrayNumber | TokenKind::String | TokenKind::ArrayString
        );
        let inline_unit = if carries && inline_unit != b"no_unit" {
            inline_unit
        } else {
            b""
        };
        let number = matches!(kind, TokenKind::Number | TokenKind::ArrayNumber);
        // An untyped value moves the reader's inferred type along: a plain
        // integer after a datetime stays a datetime, an ISO literal starts
        // one, any other number, string or bool ends it.
        let carried = if gov.is_some() {
            None
        } else if number && num::str_is_plain_integer(text) {
            self.datetime_carry.clone()
        } else {
            None
        };
        if gov.is_none() {
            if number && num::str_is_iso_datetime(text) {
                self.datetime_carry = Some(b"<datetime>".to_vec());
            } else if carried.is_none()
                && matches!(
                    kind,
                    TokenKind::Number
                        | TokenKind::ArrayNumber
                        | TokenKind::String
                        | TokenKind::ArrayString
                        | TokenKind::Bool
                )
            {
                self.datetime_carry = None;
            }
        }
        let spec = gov.as_ref().map_or(TypeSpec::PLAIN, |g| g.spec);
        if !inline_unit.is_empty() && !self.emitted_unit {
            let number = matches!(kind, TokenKind::Number | TokenKind::ArrayNumber);
            if self.in_array_directly()
                || (number && num::is_special_number(text))
                || spec.family == TypeFamily::Datetime
            {
                return Err(ErrorKind::UnitIllegal);
            }
        }
        if kind == TokenKind::Bool && !matches!(text, b"true" | b"false" | b"on" | b"off") {
            return Err(ErrorKind::TypeValueMismatch);
        }
        let literal = literal(kind, text, frac, spec)?;
        let gov_text = gov
            .as_ref()
            .map(|g| g.text.as_slice())
            .or(carried.as_deref());
        let plain = inline_unit.is_empty() && frac.is_empty();
        if kind != TokenKind::Null
            && !(plain && probe::trivially_valid(kind, text, gov_text, &self.limits))
        {
            let probe_unit = if inline_unit.is_empty() {
                None
            } else {
                Some(inline_unit)
            };
            let expect = probe::Expect { kind, text, frac };
            probe::value(
                gov_text,
                &literal,
                probe_unit,
                &expect,
                self.version,
                &self.limits,
            )?;
        }
        self.element_begins();
        self.element_done(self.struct_depth);

        if !self.had_type_annotation {
            self.emit_pending_comma()?;
        }
        self.had_type_annotation = false;
        if kind == TokenKind::Null {
            if self.in_array_directly() {
                self.push_kind(b"null")?;
            }
        } else {
            self.push_kind(&literal)?;
            if !inline_unit.is_empty() && !self.emitted_unit {
                self.push_kind(b" ")?;
                self.push_kind(inline_unit)?;
            }
        }
        self.need_semi = true;
        self.mark_value_done();
        Ok(())
    }

    fn octet_start(&mut self) -> Result<(), ErrorKind> {
        self.stream_begun = true;
        self.element_begins();
        if !self.had_type_annotation {
            self.emit_pending_comma()?;
        }
        self.had_type_annotation = false;
        self.push_kind(&[0])?;
        self.in_octet_stream = true;
        Ok(())
    }

    fn octet_chunk(&mut self, data: &[u8]) -> Result<(), ErrorKind> {
        if data.is_empty() || data.len() > MAX_CHUNK {
            return Err(ErrorKind::InvalidArgument);
        }
        // The length is 16 bits wide; 0 stands for a full 65536-byte chunk.
        let len = u16::try_from(data.len()).unwrap_or(0).to_le_bytes();
        self.push_kind(&[1, len[0], len[1]])?;
        self.push_kind(data)
    }

    fn octet_end(&mut self) -> Result<(), ErrorKind> {
        self.awaiting_value = false;
        self.element_done(self.struct_depth);
        self.push_kind(&[0])?;
        self.in_octet_stream = false;
        self.need_semi = true;
        self.mark_value_done();
        Ok(())
    }

    fn struct_start(&mut self) -> Result<(), ErrorKind> {
        self.stream_begun = true;
        if self.struct_depth >= usize::from(self.limits.max_struct_nesting) {
            return Err(ErrorKind::StructNestingTooHigh);
        }
        self.element_begins();
        // The reader refuses every struct under a type annotation, even an
        // empty one, so refuse it here rather than at its first member as
        // the C writer does (UPSTREAM_NOTES.md).
        if self.gov.is_some() {
            return Err(ErrorKind::TypeValueMismatch);
        }
        self.awaiting_value = false;
        if !self.had_type_annotation {
            self.emit_pending_comma()?;
        }
        self.had_type_annotation = false;
        self.push_kind(b"{")?;
        self.newline_kind()?;
        self.indent += 1;
        self.need_semi = false;
        self.struct_depth += 1;
        Ok(())
    }

    fn struct_end(&mut self) -> Result<(), ErrorKind> {
        let Some(depth) = self.struct_depth.checked_sub(1) else {
            return Err(ErrorKind::IllegalStructClose);
        };
        self.element_done(depth);
        self.indent = self.indent.saturating_sub(1);
        if self.need_semi {
            // A member's value ended, and with it the inferred type.
            self.datetime_carry = None;
            self.push_kind(b";")?;
            self.newline_kind()?;
        }
        self.indent_kind()?;
        self.push_kind(b"}")?;
        self.need_semi = true;
        self.struct_depth = depth;
        self.mark_value_done();
        Ok(())
    }

    fn row_start(&mut self) -> Result<(), ErrorKind> {
        self.stream_begun = true;
        let d = self.array_depth;
        if d >= usize::from(self.limits.max_array_nesting) {
            return Err(ErrorKind::ArrayNestingTooHigh);
        }
        // A nested row is one element of its parent row, as the reader
        // counts it; a row continued with `/` belongs to the same element.
        // (The C writer counts 2 per new nested row and 1 per continued
        // one, which refuses `[1]/[[2]]` and writes `[1,2]/[[3]]`, both the
        // opposite of what its reader does; see UPSTREAM_NOTES.md.)
        if d > 0 && self.is_direct(d - 1) && !self.after_dim {
            if let Some(parent) = self.levels.get_mut(d - 1) {
                parent.count = parent.count.saturating_add(1);
            }
        }
        if self.levels.len() <= d {
            self.levels.resize(d + 1, Level::default());
        }
        let continued = self.after_dim;
        if !self.had_type_annotation {
            self.emit_pending_comma()?;
        }
        self.had_type_annotation = false;
        let (gov, struct_depth, emitted_unit) =
            (self.gov.clone(), self.struct_depth, self.emitted_unit);
        if let Some(level) = self.levels.get_mut(d) {
            level.gov = gov;
            level.count = 0;
            if !continued {
                level.dim_set = false;
            }
            level.struct_depth_at_start = struct_depth;
            level.need_comma = false;
            level.emitted_unit = emitted_unit;
        }
        self.awaiting_value = false;
        self.after_dim = false;
        self.push_kind(b"[")?;
        self.array_depth = d + 1;
        Ok(())
    }

    fn row_end(&mut self) -> Result<(), ErrorKind> {
        let d = self.array_depth - 1;
        let level = self.levels.get_mut(d).ok_or(ErrorKind::UnknownTokenType)?;
        if level.dim_set && level.dim_count != level.count {
            return Err(ErrorKind::ArrayRowSizeMismatch);
        }
        level.dim_set = true;
        level.dim_count = level.count;
        self.after_row_end = true;
        self.gov = level.gov.clone();
        self.emitted_unit = level.emitted_unit;
        self.array_depth = d;
        if d > 0 && self.is_direct(d - 1) {
            self.gov = self.levels.get(d - 1).and_then(|l| l.gov.clone());
        }
        self.push_kind(b"]")?;
        self.need_semi = true;
        self.mark_value_done();
        Ok(())
    }

    fn dim_start(&mut self) -> Result<(), ErrorKind> {
        self.after_dim = true;
        if let Some(level) = self.levels.get(self.array_depth) {
            self.gov = level.gov.clone();
            self.emitted_unit = level.emitted_unit;
        }
        self.push_kind(if self.pretty { b" / " } else { b"/" })?;
        if self.in_array_directly() {
            if let Some(level) = self.levels.get_mut(self.array_depth - 1) {
                level.need_comma = false;
            }
        }
        Ok(())
    }

    // ---- bookkeeping shared by the events ----

    /// A value starts as a direct element of the innermost array.
    fn element_begins(&mut self) {
        if self.in_array_directly() {
            if let Some(level) = self.levels.get_mut(self.array_depth - 1) {
                level.count = level.count.saturating_add(1);
            }
        }
    }

    /// A value is complete: the annotation governing the next one is the
    /// array's again (or none outside arrays).
    fn element_done(&mut self, struct_depth: usize) {
        if self.array_depth == 0 {
            self.gov = None;
        } else if let Some(level) = self.levels.get(self.array_depth - 1) {
            if level.struct_depth_at_start == struct_depth {
                self.gov = level.gov.clone();
            }
        }
    }

    fn emit_pending_comma(&mut self) -> Result<(), ErrorKind> {
        if !self.in_array_directly() {
            return Ok(());
        }
        let idx = self.array_depth - 1;
        if !self.levels.get(idx).is_some_and(|l| l.need_comma) {
            return Ok(());
        }
        self.push_kind(b",")?;
        self.space_kind()?;
        if let Some(level) = self.levels.get_mut(idx) {
            level.need_comma = false;
        }
        Ok(())
    }

    fn mark_value_done(&mut self) {
        if !self.in_array_directly() {
            return;
        }
        if let Some(level) = self.levels.get_mut(self.array_depth - 1) {
            level.need_comma = true;
            self.emitted_unit = level.emitted_unit;
        }
    }

    // ---- output ----

    fn push(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.push_kind(bytes).or_else(|kind| self.fail(kind))
    }

    fn push_kind(&mut self, bytes: &[u8]) -> Result<(), ErrorKind> {
        self.sink.write_bytes(bytes)?;
        self.position = self.position.saturating_add(bytes.len() as u64);
        Ok(())
    }

    fn newline(&mut self) -> Result<(), Error> {
        self.newline_kind().or_else(|kind| self.fail(kind))
    }

    fn newline_kind(&mut self) -> Result<(), ErrorKind> {
        if self.pretty {
            self.push_kind(b"\n")?;
        }
        Ok(())
    }

    fn space_kind(&mut self) -> Result<(), ErrorKind> {
        if self.pretty {
            self.push_kind(b" ")?;
        }
        Ok(())
    }

    fn indent_kind(&mut self) -> Result<(), ErrorKind> {
        if self.pretty {
            for _ in 0..self.indent {
                self.push_kind(b"\t")?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<W: std::io::Write> Writer<IoSink<W>> {
    /// A writer to a [`std::io::Write`].
    pub fn to_io(inner: W, pretty: bool) -> Self {
        Self::new(IoSink::new(inner), pretty)
    }
}

impl<W: core::fmt::Write> Writer<FmtSink<W>> {
    /// A writer to a [`core::fmt::Write`], such as a `String`.
    ///
    /// ```
    /// use bovnar::{Event, TokenKind, TypeSpec, Writer};
    ///
    /// let mut w = Writer::to_fmt(String::new(), false);
    /// w.write_event(&Event::AssignmentStart { key: b"on" })?;
    /// w.write_event(&Event::Data {
    ///     kind: TokenKind::Bool,
    ///     text: b"true",
    ///     frac: b"",
    ///     value_type: TypeSpec::PLAIN,
    ///     unit: b"",
    ///     inline_unit: b"",
    /// })?;
    /// w.finish()?;
    /// assert_eq!(w.into_sink().into_inner(), ".on=true;");
    /// # Ok::<(), bovnar::Error>(())
    /// ```
    pub fn to_fmt(inner: W, pretty: bool) -> Self {
        Self::new(FmtSink::new(inner), pretty)
    }
}

/// Whether a family takes a unit (the numeric families).
fn takes_unit(family: TypeFamily) -> bool {
    matches!(
        family,
        TypeFamily::Uint
            | TypeFamily::Sint
            | TypeFamily::Float
            | TypeFamily::FloatFix
            | TypeFamily::FloatDec
    )
}

fn family_name(family: TypeFamily) -> &'static str {
    match family {
        TypeFamily::Uint => "uint",
        TypeFamily::Sint => "sint",
        TypeFamily::Float => "float",
        TypeFamily::FloatFix => "float_fix",
        TypeFamily::FloatDec => "float_dec",
        TypeFamily::Utf8 => "utf8",
        TypeFamily::Bool => "bool",
        TypeFamily::Datetime => "datetime",
        TypeFamily::Plain => "plain",
    }
}

/// `<family:width,_base,qN,epoch,unit>` with the parameters the annotation
/// reports, in that order, plus the type it describes.
fn annotation_text(ann: &TypeAnnotation<'_>) -> (Vec<u8>, TypeSpec) {
    let mut text = Vec::from(&b"<"[..]);
    text.extend_from_slice(family_name(ann.family).as_bytes());
    let mut spec = TypeSpec {
        family: ann.family,
        width: 0,
        base: 0,
    };
    let mut first = true;
    let mut param = |text: &mut Vec<u8>, body: &[u8]| {
        text.push(if first { b':' } else { b',' });
        first = false;
        text.extend_from_slice(body);
    };
    if let Some(width) = ann.width {
        param(&mut text, format!("{width}").as_bytes());
        spec.width = width;
    }
    if let Some(base) = ann.base {
        param(&mut text, format!("_{base}").as_bytes());
        spec.base = base;
    }
    if let Some(q) = ann.q {
        param(&mut text, format!("q{q}").as_bytes());
        spec.base = q;
    }
    if let Some(epoch) = ann.epoch {
        param(&mut text, epoch.as_bytes());
        spec.base = num::epoch_index_of(epoch.as_bytes()).unwrap_or(0);
    }
    if let Some(unit) = ann.unit.filter(|u| !u.is_empty()) {
        param(&mut text, unit);
    }
    text.push(b'>');
    (text, spec)
}

/// The value as it goes into the document.
fn literal(
    kind: TokenKind,
    text: &[u8],
    frac: &[u8],
    spec: TypeSpec,
) -> Result<Vec<u8>, ErrorKind> {
    Ok(match kind {
        TokenKind::Null => Vec::new(),
        TokenKind::Number | TokenKind::ArrayNumber => match datetime_iso(text, frac, spec)? {
            Some(iso) => iso,
            None => text.to_vec(),
        },
        TokenKind::String | TokenKind::ArrayString => quote(text),
        TokenKind::Bool => {
            let truthy = matches!(text, b"true" | b"on");
            Vec::from(if truthy { &b"true"[..] } else { &b"false"[..] })
        }
        TokenKind::Symbol => text.to_vec(),
        TokenKind::Reference => {
            let mut out = Vec::from(&b"&"[..]);
            out.extend_from_slice(text);
            out
        }
        _ => return Err(ErrorKind::UnknownTokenType),
    })
}

/// A string literal with the escapes the C writer uses.
fn quote(text: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 2);
    out.push(b'"');
    for &c in text {
        let esc: &[u8] = match c {
            b'\t' => b"\\t",
            b'\n' => b"\\n",
            0x0b => b"\\v",
            0x0c => b"\\f",
            b'\r' => b"\\r",
            b'"' => b"\\\"",
            b'\\' => b"\\\\",
            _ => {
                out.push(c);
                continue;
            }
        };
        out.extend_from_slice(esc);
    }
    out.push(b'"');
    out
}

/// A datetime with sub-second digits is written back as its ISO literal,
/// since the whole-second carrier alone would drop the fraction
/// (`bvn_ser_datetime_to_civil`).
fn datetime_iso(text: &[u8], frac: &[u8], spec: TypeSpec) -> Result<Option<Vec<u8>>, ErrorKind> {
    if spec.family != TypeFamily::Datetime
        || frac.is_empty()
        || !frac.iter().all(u8::is_ascii_digit)
    {
        return Ok(None);
    }
    let Some(seconds) = core::str::from_utf8(text)
        .ok()
        .and_then(|t| t.parse::<i64>().ok())
    else {
        return Ok(None);
    };
    let Some([year, month, day, hour, minute, second]) = num::epoch_seconds_to_civil(seconds, spec)
    else {
        return Ok(None);
    };
    if !(0..=9999).contains(&year) {
        return Err(ErrorKind::InvalidDatetimeLiteral);
    }
    let mut out =
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.").into_bytes();
    out.extend_from_slice(frac);
    out.push(b'Z');
    Ok(Some(out))
}
