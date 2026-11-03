//! Building a [`Document`] from the reader's events, and the document-tier
//! checks the streaming reader cannot make (spec §7.4, §8.1).

mod check;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use crate::event::{Event, TokenKind, TypeAnnotation};
use crate::value::{Annotation, Array, Document, Kind, Number, Struct, Value};
use crate::{Error, ErrorKind, Limits, ReadOptions, Reader, SpecVersion};

/// Chunk size the input is fed to the reader with.
const FEED_CHUNK: usize = 64 * 1024;

/// Parses a whole document with the default [`Limits`].
///
/// Beyond what the streaming reader verifies, the document tier rejects
/// duplicate keys in a scope (`duplicate_struct_key`), arrays whose elements
/// differ in kind or unit (`array_element_type_mismatch`), ragged rows
/// (`array_row_size_mismatch`) and arrays of structs with different keys
/// (`struct_shape_mismatch`), with the same verdicts as the C reference's
/// `bvn_dom_parse`.
///
/// ```
/// let doc = bovnar::parse(b".port = 8080; .hosts = [\"a\", \"b\"];")?;
/// assert_eq!(doc.get("port").and_then(|v| v.as_u64()), Some(8080));
/// assert_eq!(doc.lookup(".hosts[1]").and_then(|v| v.as_str()), Some("b"));
///
/// let err = bovnar::parse(b".a = [1, \"two\"];").unwrap_err();
/// assert_eq!(err.kind(), bovnar::ErrorKind::ArrayElementTypeMismatch);
/// # Ok::<(), bovnar::Error>(())
/// ```
///
/// # Errors
///
/// The first error the reader reports, else the first document-tier error.
pub fn parse(input: &[u8]) -> Result<Document, Error> {
    parse_with(input, &Limits::default())
}

/// Parses a whole document under `limits`.
///
/// Besides the reader limits, [`Limits::max_dom_nodes`] caps the number of
/// values in the tree; a document over it fails with
/// [`ErrorKind::FileTooLong`].
///
/// # Errors
///
/// As [`parse`], plus the node limit.
pub fn parse_with(input: &[u8], limits: &Limits) -> Result<Document, Error> {
    parse_with_options(input, &ReadOptions::new().limits(limits.clone()))
}

/// Parses a whole document under `options`: its limits as in [`parse_with`],
/// plus the `strict_version` and `text_only` policies.
///
/// ```
/// use bovnar::{ErrorKind, ReadOptions};
///
/// let text_only = ReadOptions::new().text_only(true);
/// let err = bovnar::parse_with_options(b".a = \x00\x00;", &text_only).unwrap_err();
/// assert_eq!(err.kind(), ErrorKind::OctetStreamForbidden);
/// ```
///
/// # Errors
///
/// As [`parse_with`]. [`ReadOptions::recovery`] is refused with
/// [`ErrorKind::InvalidArgument`]: a tree built from a document with skipped
/// parts would silently miss values, so recovery is offered on the streaming
/// [`Reader`] only.
pub fn parse_with_options(input: &[u8], options: &ReadOptions) -> Result<Document, Error> {
    if options.recovery {
        return Err(Error::without_position(ErrorKind::InvalidArgument));
    }
    let limits = &options.limits;
    let mut reader = Reader::with_options(options);
    let mut builder = Builder::new(limits.max_dom_nodes);
    for chunk in input.chunks(FEED_CHUNK) {
        let fed = reader.feed(chunk);
        while let Some(event) = reader.next_event() {
            builder.event(&event)?;
        }
        fed?;
    }
    let finished = reader.finish();
    while let Some(event) = reader.next_event() {
        builder.event(&event)?;
    }
    finished?;
    let version = reader.declared_version();
    let entries = builder.finish();
    let doc = Document::from_parts(version, entries);
    check::document(&doc, version.unwrap_or(SpecVersion::V1_0)).map_err(Error::without_position)?;
    Ok(doc)
}

/// The document-tier checks [`parse`] applies, for a tree built in code.
#[cfg(feature = "serde")]
pub(crate) fn check_document(doc: &Document) -> Result<(), Error> {
    check::document(doc, doc.version().unwrap_or(SpecVersion::V1_0))
        .map_err(Error::without_position)
}

/// An open container.
enum Frame {
    Struct {
        key: Option<String>,
        annotation: Option<Box<Annotation>>,
        entries: Vec<(String, Value)>,
        pending_key: Option<String>,
    },
    Array {
        key: Option<String>,
        annotation: Option<Box<Annotation>>,
        items: Vec<Value>,
        rows: Vec<usize>,
        row_start: usize,
    },
}

/// The event consumer that materialises the tree (`bvn_builder_t`). Its only
/// real state is the stack of open containers, so it never recurses.
struct Builder {
    entries: Vec<(String, Value)>,
    pending_key: Option<String>,
    stack: Vec<Frame>,
    annotation: Option<Box<Annotation>>,
    /// An array row closed; the array stays open only if a `/` follows.
    pending_array_pop: bool,
    /// A `/` was seen: the next row start continues the same array.
    array_dim_continue: bool,
    octets: Option<Vec<u8>>,
    nodes: u64,
    max_nodes: u64,
}

impl Builder {
    fn new(max_nodes: u64) -> Self {
        Self {
            entries: Vec::new(),
            pending_key: None,
            stack: Vec::new(),
            annotation: None,
            pending_array_pop: false,
            array_dim_continue: false,
            octets: None,
            nodes: 0,
            max_nodes,
        }
    }

    fn count_node(&mut self) -> Result<(), Error> {
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > self.max_nodes {
            return Err(Error::without_position(ErrorKind::FileTooLong));
        }
        Ok(())
    }

    fn event(&mut self, event: &Event<'_>) -> Result<(), Error> {
        match *event {
            Event::StreamStart | Event::StreamEnd | Event::Recovered(_) => {}
            Event::AssignmentStart { key } => {
                self.deferred_pop();
                let key = Some(utf8(key)?);
                match self.stack.last_mut() {
                    Some(Frame::Struct { pending_key, .. }) => *pending_key = key,
                    _ => self.pending_key = key,
                }
            }
            Event::TypeAnnotation(ann) => self.annotation = Some(Box::new(owned_annotation(&ann)?)),
            Event::StructStart => {
                self.deferred_pop();
                self.count_node()?;
                let key = self.take_key();
                self.stack.push(Frame::Struct {
                    key,
                    annotation: self.annotation.take(),
                    entries: Vec::new(),
                    pending_key: None,
                });
            }
            Event::StructEnd => {
                self.deferred_pop();
                self.close()?;
            }
            Event::ArrayRowStart => {
                if self.array_dim_continue {
                    self.array_dim_continue = false;
                    if let Some(Frame::Array {
                        items, row_start, ..
                    }) = self.stack.last_mut()
                    {
                        *row_start = items.len();
                    }
                    return Ok(());
                }
                self.deferred_pop();
                self.count_node()?;
                let key = self.take_key();
                self.stack.push(Frame::Array {
                    key,
                    annotation: self.annotation.take(),
                    items: Vec::new(),
                    rows: Vec::new(),
                    row_start: 0,
                });
            }
            Event::ArrayRowEnd => {
                self.deferred_pop();
                if let Some(Frame::Array {
                    items,
                    rows,
                    row_start,
                    ..
                }) = self.stack.last_mut()
                {
                    // A one-row array records no rows: its only row is the array.
                    if !rows.is_empty() {
                        rows.push(items.len().saturating_sub(*row_start));
                    }
                }
                self.pending_array_pop = true;
            }
            Event::ArrayDimStart => {
                self.pending_array_pop = false;
                self.array_dim_continue = true;
                if let Some(Frame::Array { items, rows, .. }) = self.stack.last_mut() {
                    if rows.is_empty() {
                        // The second row: the row that just closed was all
                        // of the array so far.
                        rows.push(items.len());
                    }
                }
            }
            Event::OctetStreamStart => {
                self.deferred_pop();
                self.octets = Some(Vec::new());
            }
            Event::OctetStreamChunk(data) => {
                if let Some(buf) = self.octets.as_mut() {
                    buf.extend_from_slice(data);
                }
            }
            Event::OctetStreamEnd => {
                let data = self.octets.take().unwrap_or_default();
                self.leaf(Kind::Octets(data))?;
            }
            Event::Data {
                kind,
                text,
                frac,
                value_type,
                unit,
                inline_unit,
            } => {
                self.deferred_pop();
                let quoted = matches!(kind, TokenKind::String | TokenKind::ArrayString);
                let value = match kind {
                    TokenKind::Null => Kind::Null,
                    TokenKind::Bool => Kind::Bool(text == b"true"),
                    TokenKind::Symbol => Kind::Symbol(utf8(text)?),
                    TokenKind::Reference => Kind::Reference(utf8(text)?),
                    // The declared family decides what the node is; a quoted
                    // literal under a numeric type is a number spelled as a
                    // string (`<uint:32,_16> "ff"`).
                    TokenKind::String | TokenKind::ArrayString if !value_type.is_numeric() => {
                        Kind::String(utf8(text)?)
                    }
                    TokenKind::String
                    | TokenKind::ArrayString
                    | TokenKind::Number
                    | TokenKind::ArrayNumber => Kind::Number(Number {
                        spec: value_type,
                        text: utf8(text)?,
                        unit: (!unit.is_empty()).then(|| utf8(unit)).transpose()?,
                        frac: utf8(frac)?,
                        quoted,
                        // Only a suffix the value's own annotation does not
                        // already carry is written back as one.
                        inline_unit: !inline_unit.is_empty()
                            && !self
                                .annotation
                                .as_ref()
                                .is_some_and(|a| a.unit.is_some() || a.epoch.is_some()),
                    }),
                    _ => Kind::Null,
                };
                self.leaf(value)?;
            }
        }
        Ok(())
    }

    /// Applies an array pop that no `/` cancelled.
    fn deferred_pop(&mut self) {
        if core::mem::take(&mut self.pending_array_pop)
            && matches!(self.stack.last(), Some(Frame::Array { .. }))
        {
            // Closing an array frame cannot fail.
            let _ = self.close();
        }
    }

    fn take_key(&mut self) -> Option<String> {
        match self.stack.last_mut() {
            Some(Frame::Struct { pending_key, .. }) => pending_key.take(),
            Some(Frame::Array { .. }) => None,
            None => self.pending_key.take(),
        }
    }

    fn leaf(&mut self, kind: Kind) -> Result<(), Error> {
        self.count_node()?;
        let key = self.take_key();
        let value = Value::from_parts(kind, self.annotation.take());
        self.attach(key, value);
        Ok(())
    }

    /// Pops the top container and attaches it to its parent.
    fn close(&mut self) -> Result<(), Error> {
        let Some(frame) = self.stack.pop() else {
            return Err(Error::without_position(ErrorKind::IllegalStructClose));
        };
        let (key, value) = match frame {
            Frame::Struct {
                key,
                annotation,
                entries,
                ..
            } => (
                key,
                Value::from_parts(Kind::Struct(Struct::from_entries(entries)), annotation),
            ),
            Frame::Array {
                key,
                annotation,
                items,
                rows,
                ..
            } => (
                key,
                Value::from_parts(Kind::Array(Array::from_parts(items, rows)), annotation),
            ),
        };
        self.attach(key, value);
        Ok(())
    }

    fn attach(&mut self, key: Option<String>, value: Value) {
        match self.stack.last_mut() {
            Some(Frame::Array { items, .. }) => items.push(value),
            Some(Frame::Struct { entries, .. }) => entries.push((key.unwrap_or_default(), value)),
            None => self.entries.push((key.unwrap_or_default(), value)),
        }
    }

    fn finish(mut self) -> Vec<(String, Value)> {
        self.deferred_pop();
        core::mem::take(&mut self.entries)
    }
}

impl Drop for Builder {
    fn drop(&mut self) {
        // Containers left open by an error: hand their children to a flat
        // list so that dropping a deep partial tree does not recurse.
        while let Some(frame) = self.stack.pop() {
            let children: Vec<Value> = match frame {
                Frame::Struct { entries, .. } => entries.into_iter().map(|(_, v)| v).collect(),
                Frame::Array { items, .. } => items,
            };
            drop(Array::from_parts(children, Vec::new()));
        }
    }
}

fn utf8(bytes: &[u8]) -> Result<String, Error> {
    String::from_utf8(bytes.to_vec())
        .map_err(|_| Error::without_position(ErrorKind::InvalidUtf8Byte))
}

fn owned_annotation(ann: &TypeAnnotation<'_>) -> Result<Annotation, Error> {
    let mut owned = Annotation::new(ann.family);
    owned.width = ann.width;
    owned.base = ann.base;
    owned.q = ann.q;
    owned.epoch = ann.epoch;
    owned.unit = ann.unit.map(utf8).transpose()?;
    owned.synthesized = ann.synthesized;
    Ok(owned)
}
