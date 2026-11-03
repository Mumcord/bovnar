//! Replays a textual event sequence through [`bovnar::Writer`].
//!
//! The format is the one `diff/c/writer_replay.c` replays through the C
//! reference's `bvnr_write_event` (see that file for the grammar), so the two
//! writers can be compared sequence by sequence. Shared by the
//! `bovnar-writer-diff` tool and the `writer_error_parity` test.

use bovnar::{
    ErrorKind, Event, SpecVersion, TokenKind, TypeAnnotation, TypeFamily, TypeSpec, Writer,
};

/// Decodes `%XX` escapes.
fn decode(field: &str) -> Vec<u8> {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(u8::try_from(h * 16 + l).unwrap_or(0));
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

fn family(name: &[u8]) -> Option<TypeFamily> {
    Some(match name {
        b"plain" => TypeFamily::Plain,
        b"utf8" => TypeFamily::Utf8,
        b"sint" => TypeFamily::Sint,
        b"uint" => TypeFamily::Uint,
        b"float" => TypeFamily::Float,
        b"float_fix" => TypeFamily::FloatFix,
        b"float_dec" => TypeFamily::FloatDec,
        b"bool" => TypeFamily::Bool,
        b"datetime" => TypeFamily::Datetime,
        _ => return None,
    })
}

fn kind(name: &[u8]) -> TokenKind {
    match name {
        b"num" => TokenKind::Number,
        b"anum" => TokenKind::ArrayNumber,
        b"str" => TokenKind::String,
        b"astr" => TokenKind::ArrayString,
        b"sym" => TokenKind::Symbol,
        b"ref" => TokenKind::Reference,
        b"bool" => TokenKind::Bool,
        b"null" => TokenKind::Null,
        _ => TokenKind::Unknown,
    }
}

fn number(field: &[u8]) -> Option<u32> {
    if field.is_empty() {
        return None;
    }
    // Like strtoul: leading digits, 0 when there are none.
    let digits: Vec<u8> = field
        .iter()
        .copied()
        .take_while(u8::is_ascii_digit)
        .collect();
    Some(
        core::str::from_utf8(&digits)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
    )
}

/// The event wants a `&'static str`; leaking one per annotation is fine
/// for a test tool.
fn epoch(name: &[u8]) -> &'static str {
    Box::leak(String::from_utf8_lossy(name).into_owned().into_boxed_str())
}

fn opt(field: &[u8]) -> Option<&[u8]> {
    (!field.is_empty()).then_some(field)
}

fn write_one(writer: &mut Writer<Vec<u8>>, event: &str) -> Result<(), bovnar::Error> {
    let fields: Vec<Vec<u8>> = event.split(':').map(decode).collect();
    let field = |i: usize| fields.get(i).map_or(&[][..], Vec::as_slice);
    let simple = |e: Event<'static>| e;
    let ev = match field(0) {
        b"v" => {
            let text = String::from_utf8_lossy(field(1)).into_owned();
            let (major, minor) = text.split_once('.').unwrap_or((&text, "0"));
            return writer.write_version(SpecVersion {
                major: major.parse().unwrap_or(0),
                minor: minor.parse().unwrap_or(0),
            });
        }
        b"ss" => simple(Event::StreamStart),
        b"se" => simple(Event::StreamEnd),
        b"{" => simple(Event::StructStart),
        b"}" => simple(Event::StructEnd),
        b"[" => simple(Event::ArrayRowStart),
        b"]" => simple(Event::ArrayRowEnd),
        b"/" => simple(Event::ArrayDimStart),
        b"o(" => simple(Event::OctetStreamStart),
        b"o)" => simple(Event::OctetStreamEnd),
        b"k" => return writer.write_event(&Event::AssignmentStart { key: field(1) }),
        b"o" => return writer.write_event(&Event::OctetStreamChunk(field(1))),
        b"a" => {
            let base = number(field(3));
            let q = number(field(4));
            let ann = TypeAnnotation {
                text: field(1),
                family: family(field(1)).unwrap_or(TypeFamily::Plain),
                width: number(field(2)),
                base,
                q,
                unit: opt(field(6)),
                epoch: opt(field(5)).map(epoch),
                synthesized: false,
            };
            return writer.write_event(&Event::TypeAnnotation(ann));
        }
        b"d" => {
            return writer.write_event(&Event::Data {
                kind: kind(field(1)),
                text: field(2),
                frac: field(3),
                value_type: TypeSpec::PLAIN,
                unit: field(4),
                inline_unit: field(4),
            })
        }
        other => panic!("unknown event {}", String::from_utf8_lossy(other)),
    };
    writer.write_event(&ev)
}

/// Replays one sequence (a line of the format) and returns what the C
/// harness prints for it: `ok <hex>` or `err <name> <event index>`.
pub fn replay(line: &str) -> String {
    let mut writer = Writer::new(Vec::new(), true);
    let mut index = 0usize;
    let mut failed: Option<ErrorKind> = None;
    for event in line.split(' ').filter(|e| !e.is_empty()) {
        if let Err(err) = write_one(&mut writer, event) {
            failed = Some(err.kind());
            break;
        }
        index += 1;
    }
    if failed.is_none() {
        if let Err(err) = writer.finish() {
            failed = Some(err.kind());
        }
    }
    match failed {
        Some(kind) => format!("err {} {index}", kind.as_str()),
        None => {
            let out = writer.into_sink();
            let mut hex = String::with_capacity(out.len() * 2);
            for b in out {
                hex.push_str(&format!("{b:02x}"));
            }
            format!("ok {hex}")
        }
    }
}
