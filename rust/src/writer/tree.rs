//! Serialising a [`Document`] through the event writer.

use alloc::string::String;
use alloc::vec::Vec;

use super::{Sink, Writer};
use crate::event::{Event, TokenKind, TypeAnnotation};
use crate::types::TypeSpec;
use crate::value::{Annotation, Document, Kind, Value};
use crate::{Error, ErrorKind, Limits};

/// Largest chunk an octet stream is written in.
const CHUNK: usize = 65_536;

/// Writes `doc` pretty-printed, as the C reference's `bovnar pretty-print`
/// lays a document out: one assignment per line, tab indentation.
///
/// ```
/// let doc = bovnar::parse(b".a={.b=[1,2];};")?;
/// assert_eq!(
///     bovnar::to_string(&doc)?,
///     ".a = {\n\t.b = [<uint:64,_10,no_unit> 1, 2];\n};\n"
/// );
/// # Ok::<(), bovnar::Error>(())
/// ```
///
/// # Errors
///
/// A value the writer refuses (see [`Writer`]), or
/// [`ErrorKind::InvalidUtf8Byte`] for a document whose octet streams make the
/// output binary; write those with [`to_vec`].
pub fn to_string(doc: &Document) -> Result<String, Error> {
    utf8(to_vec(doc)?)
}

/// Writes `doc` without whitespace.
///
/// # Errors
///
/// As [`to_string`].
pub fn to_string_compact(doc: &Document) -> Result<String, Error> {
    utf8(to_vec_compact(doc)?)
}

/// Writes `doc` pretty-printed, as bytes.
///
/// # Errors
///
/// A value the writer refuses.
pub fn to_vec(doc: &Document) -> Result<Vec<u8>, Error> {
    write_document(doc, Vec::new(), true)
}

/// Writes `doc` without whitespace, as bytes.
///
/// # Errors
///
/// A value the writer refuses.
pub fn to_vec_compact(doc: &Document) -> Result<Vec<u8>, Error> {
    write_document(doc, Vec::new(), false)
}

fn utf8(bytes: Vec<u8>) -> Result<String, Error> {
    String::from_utf8(bytes).map_err(|_| Error::without_position(ErrorKind::InvalidUtf8Byte))
}

/// Writes `doc` to `sink` at the format's hard nesting cap (255), so that
/// every document [`crate::parse_with`] can build can be written back.
fn write_document<S: Sink>(doc: &Document, sink: S, pretty: bool) -> Result<S, Error> {
    let limits = Limits {
        max_struct_nesting: u8::MAX,
        max_array_nesting: u8::MAX,
        ..Limits::default()
    };
    let mut writer = Writer::with_limits(sink, pretty, &limits);
    if let Some(version) = doc.version() {
        writer.write_version(version)?;
    }
    doc.write_events(&mut writer)?;
    writer.finish()?;
    Ok(writer.into_sink())
}

/// Work left to do while walking the tree.
enum Task<'a> {
    Value(&'a Value),
    Entry(&'a str, &'a Value),
    Event(Event<'static>),
}

impl Document {
    /// Feeds the document to `writer` as events (without the version
    /// directive and without finishing), walking the tree with an explicit
    /// stack.
    ///
    /// # Errors
    ///
    /// The first error the writer reports.
    pub fn write_events<S: Sink>(&self, writer: &mut Writer<S>) -> Result<(), Error> {
        writer.write_event(&Event::StreamStart)?;
        let mut stack: Vec<Task<'_>> = Vec::new();
        for (key, value) in self.entries() {
            stack.push(Task::Entry(key, value));
            while let Some(task) = stack.pop() {
                match task {
                    Task::Event(event) => writer.write_event(&event)?,
                    Task::Entry(key, value) => {
                        writer.write_event(&Event::AssignmentStart {
                            key: key.as_bytes(),
                        })?;
                        stack.push(Task::Value(value));
                    }
                    Task::Value(value) => write_value(value, writer, &mut stack)?,
                }
            }
        }
        writer.write_event(&Event::StreamEnd)
    }
}

fn write_value<'a, S: Sink>(
    value: &'a Value,
    writer: &mut Writer<S>,
    stack: &mut Vec<Task<'a>>,
) -> Result<(), Error> {
    if let Some(ann) = value.annotation() {
        writer.write_event(&Event::TypeAnnotation(annotation_event(ann)))?;
    }
    match value.kind() {
        Kind::Null => writer.write_event(&data(TokenKind::Null, b"", b"", b"")),
        Kind::Bool(b) => writer.write_event(&data(
            TokenKind::Bool,
            if *b { b"true" } else { b"false" },
            b"",
            b"",
        )),
        Kind::Number(n) => {
            let kind = if n.is_quoted() {
                TokenKind::String
            } else {
                TokenKind::Number
            };
            let unit = match (n.unit(), n.inline_unit) {
                (Some(u), true) => u.as_bytes(),
                _ => b"",
            };
            writer.write_event(&data(kind, n.text().as_bytes(), n.frac.as_bytes(), unit))
        }
        Kind::String(s) => writer.write_event(&data(TokenKind::String, s.as_bytes(), b"", b"")),
        Kind::Symbol(s) => writer.write_event(&data(TokenKind::Symbol, s.as_bytes(), b"", b"")),
        Kind::Reference(s) => {
            writer.write_event(&data(TokenKind::Reference, s.as_bytes(), b"", b""))
        }
        Kind::Octets(bytes) => {
            writer.write_event(&Event::OctetStreamStart)?;
            for chunk in bytes.chunks(CHUNK) {
                writer.write_event(&Event::OctetStreamChunk(chunk))?;
            }
            writer.write_event(&Event::OctetStreamEnd)
        }
        Kind::Struct(s) => {
            writer.write_event(&Event::StructStart)?;
            stack.push(Task::Event(Event::StructEnd));
            for (key, field) in s.entries().iter().rev() {
                stack.push(Task::Entry(key, field));
            }
            Ok(())
        }
        Kind::Array(array) => {
            writer.write_event(&Event::ArrayRowStart)?;
            let rows: Vec<&[Value]> = array.rows().collect();
            for (i, row) in rows.iter().enumerate().rev() {
                stack.push(Task::Event(Event::ArrayRowEnd));
                for item in row.iter().rev() {
                    stack.push(Task::Value(item));
                }
                if i > 0 {
                    stack.push(Task::Event(Event::ArrayRowStart));
                    stack.push(Task::Event(Event::ArrayDimStart));
                }
            }
            Ok(())
        }
    }
}

fn data<'a>(kind: TokenKind, text: &'a [u8], frac: &'a [u8], inline_unit: &'a [u8]) -> Event<'a> {
    Event::Data {
        kind,
        text,
        frac,
        value_type: TypeSpec::PLAIN,
        unit: b"",
        inline_unit,
    }
}

fn annotation_event(ann: &Annotation) -> TypeAnnotation<'_> {
    TypeAnnotation {
        text: b"",
        family: ann.family,
        width: ann.width,
        base: ann.base,
        q: ann.q,
        unit: ann.unit.as_deref().map(str::as_bytes),
        epoch: ann.epoch,
        synthesized: ann.synthesized,
    }
}
