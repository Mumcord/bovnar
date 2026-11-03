//! Entry points for the conformance test suite.
//!
//! Not part of the public API: these functions exist so that the IUT adapter
//! (`bovnar-iut`) and the data-driven tests can drive the reader the way the
//! upstream conformance protocol (`doc/13_bovnar_conformance.md` §6–7)
//! expects.

use alloc::string::String;

use core::fmt::Write as _;

use crate::{read_all, Error, Event, Limits};

/// Parses `input` with the streaming reader and returns the conformance event
/// log (doc/13 §7.1): one line per verified event, each terminated by `\n`,
/// formatted exactly as the C reference IUT adapter prints it.
///
/// # Errors
///
/// Returns the first error the reader reports.
pub fn event_log(input: &[u8], limits: &Limits) -> Result<String, Error> {
    let mut log = String::new();
    read_all(input, limits, |event| write_event(&mut log, &event))?;
    Ok(log)
}

fn write_event(log: &mut String, event: &Event<'_>) {
    match *event {
        Event::StreamStart => log.push_str("STREAM_START\n"),
        Event::AssignmentStart { key } => line(log, "ASSIGNMENT_START ", key),
        Event::TypeAnnotation(ann) => {
            line(log, "TYPE_ANN_START ", ann.text);
            line(log, "TYPE_FAMILY ", ann.text);
            let params = [
                ("TYPE_PARAM_WIDTH ", ann.width),
                ("TYPE_PARAM_BASE ", ann.base),
                ("TYPE_PARAM_Q ", ann.q),
            ];
            for (name, value) in params {
                if let Some(value) = value {
                    let _ = writeln!(log, "{name}{value}");
                }
            }
            if let Some(epoch) = ann.epoch {
                line(log, "TYPE_PARAM_UNIT ", epoch.as_bytes());
            }
            if let Some(unit) = ann.unit {
                line(log, "TYPE_PARAM_UNIT ", unit);
            }
            line(log, "TYPE_ANN_END ", ann.text);
        }
        Event::Data { kind, text, .. } => {
            log.push_str("DATA ");
            log.push_str(kind.as_str());
            line(log, " ", text);
        }
        Event::OctetStreamChunk(data) => {
            let _ = writeln!(log, "DATA octets {} bytes", data.len());
        }
        Event::StructStart => log.push_str("STRUCT_START\n"),
        Event::StructEnd => log.push_str("STRUCT_END\n"),
        Event::ArrayRowStart => log.push_str("ARRAY_ROW_START\n"),
        Event::ArrayRowEnd => log.push_str("ARRAY_ROW_END\n"),
        Event::ArrayDimStart => log.push_str("ARRAY_DIM_START\n"),
        Event::OctetStreamStart => log.push_str("OCTET_STREAM_START\n"),
        Event::OctetStreamEnd => log.push_str("OCTET_STREAM_END\n"),
        Event::StreamEnd | Event::Recovered(_) => {}
    }
}

/// Writes `prefix`, then `text` with every byte outside printable ASCII (and
/// the backslash) escaped as `\xNN`, then a newline.
fn line(log: &mut String, prefix: &str, text: &[u8]) {
    log.push_str(prefix);
    for &b in text {
        if (0x20..=0x7e).contains(&b) && b != b'\\' {
            log.push(char::from(b));
        } else {
            let _ = write!(log, "\\x{b:02x}");
        }
    }
    log.push('\n');
}

/// Builds the materialised document for `input` and applies the DOM-tier
/// checks (array homogeneity, struct shape, duplicate keys) that the
/// streaming protocol cannot observe.
///
/// # Errors
///
/// Returns the first error found.
pub fn dom_check(input: &[u8], limits: &Limits) -> Result<(), Error> {
    crate::parse_with(input, limits).map(drop)
}
