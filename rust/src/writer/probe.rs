//! Validation by reading back: each key, annotation and value the writer is
//! about to emit is first read by the reader in a one-assignment document.
//!
//! This holds the writer to exactly the rules the reader enforces (and
//! reports the reader's error for a value it would refuse), and the event
//! comparison catches text that would read back as something else, such as a
//! symbol that smuggles in a second assignment.

use alloc::format;
use alloc::vec::Vec;

use crate::event::{Event, TokenKind};
use crate::{read_all, ErrorKind, Limits, SpecVersion};

/// What the value must read back as.
pub(super) struct Expect<'a> {
    pub kind: TokenKind,
    pub text: &'a [u8],
    pub frac: &'a [u8],
}

fn document(version: Option<SpecVersion>, parts: &[&[u8]]) -> Vec<u8> {
    let mut doc = match version {
        Some(v) => format!("#!bovnar {}.{}\n", v.major, v.minor).into_bytes(),
        None => Vec::new(),
    };
    for part in parts {
        doc.extend_from_slice(part);
    }
    doc
}

/// Reads `doc`, handing each event to `seen`.
///
/// Like the C writer, the probe holds a value to what any reader
/// configuration can take: only the nesting limits are the writer's own.
fn read(doc: &[u8], limits: &Limits, seen: impl FnMut(Event<'_>)) -> Result<(), ErrorKind> {
    let limits = Limits {
        max_identifier_length: u16::MAX,
        max_string_length: u16::MAX,
        max_number_length: u16::MAX,
        max_symbol_length: u16::MAX,
        max_reference_length: u16::MAX,
        max_array_items: u64::MAX,
        max_input_bytes: u64::MAX,
        max_dom_nodes: u64::MAX,
        ..limits.clone()
    };
    read_all(doc, &limits, seen).map_err(|e| e.kind())
}

/// A unit spelling in an annotation's unit parameter or, `inline`, as a
/// suffix: must read back as one value carrying exactly this spelling
/// (`bvn_writer_probe_unit_source`). `None` when it does not, for the
/// caller to report as `UnitIllegal`.
pub(super) fn unit(text: &[u8], inline: bool) -> Option<()> {
    let mut doc = Vec::from(&b"#!bovnar 1.2\n.k="[..]);
    if inline {
        doc.extend_from_slice(b"1 ");
        doc.extend_from_slice(text);
        doc.push(b';');
    } else {
        doc.extend_from_slice(b"<uint:");
        doc.extend_from_slice(text);
        doc.extend_from_slice(b">1;");
    }
    let limits = Limits {
        max_array_nesting: 1,
        ..Limits::default()
    };
    let (mut keys, mut values, mut same) = (0usize, 0usize, false);
    read_all(&doc, &limits, |event| match event {
        Event::AssignmentStart { .. } => keys += 1,
        Event::Data { unit, .. } => {
            values += 1;
            same = unit == text;
        }
        _ => {}
    })
    .ok()?;
    (keys == 1 && values == 1 && same).then_some(())
}

/// A struct key: must read back as exactly this one key.
///
/// A plain ASCII identifier needs no reading back.
pub(super) fn key(
    key: &[u8],
    version: Option<SpecVersion>,
    limits: &Limits,
) -> Result<(), ErrorKind> {
    let plain = key
        .first()
        .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
        && key.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
        && key.len() <= usize::from(limits.max_identifier_length);
    if plain {
        return Ok(());
    }
    let doc = document(version, &[b".", key, b" = null;"]);
    let mut keys = 0usize;
    let mut same = false;
    read(&doc, limits, |event| {
        if let Event::AssignmentStart { key: k } = event {
            keys += 1;
            same = k == key;
        }
    })?;
    if keys == 1 && same {
        Ok(())
    } else {
        Err(ErrorKind::UnexpectedInputByte)
    }
}

/// A type annotation, as it would stand in front of an empty value.
pub(super) fn annotation(
    text: &[u8],
    version: Option<SpecVersion>,
    limits: &Limits,
) -> Result<(), ErrorKind> {
    if DEFAULT_ANNOTATIONS.contains(&text) {
        return Ok(());
    }
    let doc = document(version, &[b".v = ", text, b" ;"]);
    read(&doc, limits, |_| {})
}

/// The annotations the reader synthesises for untyped values; they are
/// valid as they stand.
const DEFAULT_ANNOTATIONS: [&[u8]; 4] = [
    b"<uint:64,_10,no_unit>",
    b"<sint:64,_10,no_unit>",
    b"<utf8>",
    b"<bool>",
];

/// Whether a value is valid on its face under `gov` (none, or the default
/// annotation for its kind), so that reading it back can be skipped: a short
/// decimal integer, which fits the 64-bit default type, or a string of
/// printable ASCII (escapes are the writer's).
pub(super) fn trivially_valid(
    kind: TokenKind,
    text: &[u8],
    gov: Option<&[u8]>,
    limits: &Limits,
) -> bool {
    let under = |default: &[u8]| gov.map_or(true, |g| g == default);
    match kind {
        TokenKind::Number | TokenKind::ArrayNumber => {
            let (digits, default) = match text.strip_prefix(b"-") {
                Some(d) => (d, DEFAULT_ANNOTATIONS[1]),
                None => (text, DEFAULT_ANNOTATIONS[0]),
            };
            under(default)
                && !digits.is_empty()
                && digits.len() <= 18
                && digits.iter().all(u8::is_ascii_digit)
        }
        TokenKind::String | TokenKind::ArrayString => {
            under(DEFAULT_ANNOTATIONS[2])
                && text.len() <= usize::from(limits.max_string_length)
                && text.iter().all(|b| (0x20..0x7f).contains(b))
        }
        TokenKind::Bool => under(DEFAULT_ANNOTATIONS[3]),
        _ => false,
    }
}

/// A value under the governing annotation `gov`, with an optional inline
/// unit: must read back as one value of the same kind and text.
pub(super) fn value(
    gov: Option<&[u8]>,
    literal: &[u8],
    inline_unit: Option<&[u8]>,
    expect: &Expect<'_>,
    version: Option<SpecVersion>,
    limits: &Limits,
) -> Result<(), ErrorKind> {
    let gov = gov.unwrap_or_default();
    let (sep, unit): (&[u8], &[u8]) = match inline_unit {
        Some(u) => (b" ", u),
        None => (b"", b""),
    };
    let doc = document(version, &[b".v = ", gov, b" ", literal, sep, unit, b";"]);
    let mut values = 0usize;
    let mut same = false;
    read(&doc, limits, |event| {
        if let Event::Data {
            kind, text, frac, ..
        } = event
        {
            values += 1;
            same = category(kind) == category(expect.kind)
                && normalized(kind, text) == normalized(expect.kind, expect.text)
                && frac == expect.frac;
        }
    })?;
    if values == 1 && same {
        Ok(())
    } else {
        Err(ErrorKind::UnexpectedInputByte)
    }
}

/// Token kinds that are the same value inside and outside an array.
fn category(kind: TokenKind) -> TokenKind {
    match kind {
        TokenKind::ArrayNumber => TokenKind::Number,
        TokenKind::ArrayString => TokenKind::String,
        other => other,
    }
}

fn normalized(kind: TokenKind, text: &[u8]) -> &[u8] {
    match (kind, text) {
        (TokenKind::Bool, b"on") => b"true",
        (TokenKind::Bool, b"off") => b"false",
        _ => text,
    }
}
