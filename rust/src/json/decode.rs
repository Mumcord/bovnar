//! Document to JSON, as `cmd_convert_bvnr_to_json` prints it, with the
//! survey of what JSON cannot carry (`json_loss_scan`).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

use super::{JsonError, Loss, LossKind};
use crate::num;
use crate::types::TypeFamily;
use crate::value::{Document, Kind, Number, Value};

/// `snprintf` into a `cap`-byte buffer: at most `cap - 1` bytes, cut at a
/// character boundary.
pub(super) fn truncate(mut s: String, cap: usize) -> String {
    let max = cap.saturating_sub(1);
    if s.len() > max {
        let mut cut = max;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
    }
    s
}

/// Whether the reference's tree holds `n` as an integer node: an integer
/// family, unless the literal is `nan`, `inf` or `ninf` (a float node).
fn is_int_node(n: &Number) -> bool {
    match n.spec().family {
        TypeFamily::Float | TypeFamily::FloatFix | TypeFamily::FloatDec => false,
        TypeFamily::Uint | TypeFamily::Sint => n.is_integer(),
        _ => !num::is_special_number(n.text().as_bytes()),
    }
}

/// The epoch-second count of a datetime, the signed value of any other
/// integer (`bvn_dom_get_i64`; 0 when it has none).
fn int_value(n: &Number) -> i64 {
    let spec = n.spec();
    if spec.family == TypeFamily::Datetime && num::str_is_iso_datetime(n.text().as_bytes()) {
        return num::iso_to_epoch_seconds(n.text().as_bytes(), spec).map_or(0, |i| i.seconds);
    }
    n.as_i64().unwrap_or(0)
}

/// `bvn_dom_get_float`.
fn float_value(n: &Number) -> f64 {
    let hex = n.spec().effective_base() == 16;
    let v = if hex && !num::is_special_number(n.text().as_bytes()) {
        num::parse_hex_float(n.text())
    } else {
        n.as_f64()
    };
    v.unwrap_or(0.0)
}

fn unit_of(v: &Value) -> Option<&str> {
    v.unit()
        .or_else(|| v.annotation().and_then(|a| a.unit.as_deref()))
        .filter(|u| !u.is_empty() && *u != "no_unit")
}

/// `print_json_string_escaped`.
fn escape(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `print_json_double`: the shortest `%g` that reads back, widened to plain
/// digits where that is no longer than 17 significant digits; non-finite
/// values are `null`.
fn push_double(out: &mut String, v: f64) {
    if !v.is_finite() {
        out.push_str("null");
        return;
    }
    let mut prec = 1;
    let mut buf = num::fmt_g(v, prec);
    while prec <= 17 {
        buf = num::fmt_g(v, prec);
        if buf.parse::<f64>().ok() == Some(v) {
            break;
        }
        prec += 1;
    }
    if let Some((_, exp)) = buf.split_once('e') {
        let e10: i64 = exp.parse().unwrap_or(-1);
        if (0..17).contains(&e10) {
            let wide = usize::try_from(e10 + 1).unwrap_or(0).max(prec);
            let alt = num::fmt_g(v, wide);
            if alt.parse::<f64>().ok() == Some(v) && !alt.contains('e') {
                buf = alt;
            }
        }
    }
    out.push_str(&buf);
}

/// A scalar (`print_json_node` for anything but a struct or an array).
fn push_scalar(out: &mut String, v: &Value) {
    match v.kind() {
        Kind::Null | Kind::Struct(_) | Kind::Array(_) => out.push_str("null"),
        Kind::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Kind::String(s) | Kind::Symbol(s) | Kind::Reference(s) => escape(out, s),
        Kind::Octets(bytes) => {
            out.push('"');
            for b in bytes {
                let _ = write!(out, "{b:02x}");
            }
            out.push('"');
        }
        Kind::Number(n) => {
            let spec = n.spec();
            if !is_int_node(n) {
                push_double(out, float_value(n));
            } else if spec.width > 64 {
                match num::int_to_decimal(n.text().as_bytes(), spec.effective_base()) {
                    Some(d) => {
                        out.push('"');
                        out.push_str(&d);
                        out.push('"');
                    }
                    None => out.push_str("null"),
                }
            } else if spec.family == TypeFamily::Uint {
                let _ = write!(out, "{}", n.as_u64().unwrap_or(0));
            } else {
                let _ = write!(out, "{}", int_value(n));
            }
        }
    }
}

fn indent(out: &mut String, level: usize) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

enum Frame<'a> {
    Struct {
        entries: &'a [(String, Value)],
        next: usize,
        indent: usize,
    },
    Array {
        items: &'a [Value],
        next: usize,
        indent: usize,
        /// Elements per row of a `/` array printed as nested arrays; 0 for a
        /// plain one.
        row_len: usize,
    },
}

/// Opens `v` (a struct or array) as a frame, or prints it if it is a scalar.
fn open<'a>(out: &mut String, v: &'a Value, level: usize, stack: &mut Vec<Frame<'a>>) {
    match v.kind() {
        Kind::Struct(s) => {
            out.push('{');
            if !s.is_empty() {
                out.push('\n');
            }
            stack.push(Frame::Struct {
                entries: s.entries(),
                next: 0,
                indent: level,
            });
        }
        Kind::Array(a) => {
            let cnt = a.len();
            let dims = a.row_count();
            out.push('[');
            let mut row_len = 0;
            if dims > 1 && cnt % dims == 0 {
                row_len = cnt / dims;
                if row_len == 0 {
                    for r in 0..dims {
                        if r > 0 {
                            out.push_str(", ");
                        }
                        out.push_str("[]");
                    }
                }
            }
            stack.push(Frame::Array {
                items: a.items(),
                next: 0,
                indent: level,
                row_len,
            });
        }
        _ => push_scalar(out, v),
    }
}

/// Prints the members of a document or struct body iteratively.
fn print(out: &mut String, entries: &[(String, Value)]) {
    out.push('{');
    if !entries.is_empty() {
        out.push('\n');
    }
    let mut stack = alloc::vec![Frame::Struct {
        entries,
        next: 0,
        indent: 0,
    }];
    while let Some(top) = stack.last_mut() {
        match top {
            Frame::Struct {
                entries,
                next,
                indent: level,
            } => {
                let level = *level;
                if *next > 0 {
                    // Close the previous member's line.
                    if *next < entries.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                let Some((key, value)) = entries.get(*next) else {
                    let had = !entries.is_empty();
                    stack.pop();
                    if had {
                        indent(out, level);
                    }
                    out.push('}');
                    continue;
                };
                *next += 1;
                indent(out, level + 1);
                escape(out, key);
                out.push_str(": ");
                open(out, value, level + 1, &mut stack);
            }
            Frame::Array {
                items,
                next,
                indent: level,
                row_len,
            } => {
                let (level, row_len, i) = (*level, *row_len, *next);
                let Some(item) = items.get(i) else {
                    if row_len > 0 && !items.is_empty() {
                        out.push(']');
                    }
                    stack.pop();
                    out.push(']');
                    continue;
                };
                *next += 1;
                if row_len > 0 {
                    if i % row_len == 0 {
                        if i > 0 {
                            out.push_str("], ");
                        }
                        out.push('[');
                    } else {
                        out.push_str(", ");
                    }
                } else if i > 0 {
                    out.push_str(", ");
                }
                open(out, item, level, &mut stack);
            }
        }
    }
}

/// Whether `v` is or holds a datetime with sub-second digits
/// (`find_fractional_datetime`).
fn has_fractional_datetime(v: &Value) -> bool {
    let mut stack = alloc::vec![v];
    while let Some(v) = stack.pop() {
        match v.kind() {
            Kind::Number(n) if n.is_datetime() && n.fraction().is_some() => return true,
            Kind::Struct(s) => stack.extend(s.iter().map(|(_, v)| v)),
            Kind::Array(a) => stack.extend(a.iter()),
            _ => {}
        }
    }
    false
}

/// The loss survey of one value (`json_loss_scan`) without its children.
fn scan_one(v: &Value, path: &str, unit: Option<&str>, loss: &mut Loss) {
    if let Some(u) = unit {
        loss.note(LossKind::Unit, path, Some(u));
    }
    match v.kind() {
        Kind::Symbol(_) => loss.note(LossKind::Symbol, path, None),
        Kind::Reference(_) => loss.note(LossKind::Reference, path, None),
        Kind::Octets(_) => loss.note(LossKind::Octets, path, None),
        Kind::Number(n) => {
            let spec = n.spec();
            if is_int_node(n) {
                if spec.family == TypeFamily::Datetime {
                    let epoch = num::epoch_name(spec.base).unwrap_or("unix");
                    let detail = alloc::format!("seconds since the {epoch} epoch");
                    loss.note(LossKind::Datetime, path, Some(&detail));
                } else if spec.width > 64 {
                    loss.note(
                        LossKind::WideInteger,
                        path,
                        Some("emitted as a JSON string"),
                    );
                } else if spec.effective_base() != 10 {
                    loss.note(
                        LossKind::NonDecimalBase,
                        path,
                        Some("written in a non-decimal base"),
                    );
                }
                return;
            }
            let f = float_value(n);
            let width = spec.effective_width();
            if !f.is_finite() {
                let detail = if f.is_nan() {
                    "nan"
                } else if f > 0.0 {
                    "inf"
                } else {
                    "ninf"
                };
                loss.note(LossKind::NonFiniteFloat, path, Some(detail));
            } else if (spec.family == TypeFamily::Float && spec.width > 64)
                || (matches!(spec.family, TypeFamily::FloatDec | TypeFamily::FloatFix)
                    && width >= 64)
            {
                let family = match spec.family {
                    TypeFamily::Float => "float",
                    TypeFamily::FloatDec => "float_dec",
                    _ => "float_fix",
                };
                let detail = alloc::format!("<{family}:{width}>");
                loss.note(LossKind::WideFloat, path, Some(&detail));
            }
        }
        Kind::Array(a) if a.row_count() > 1 => {
            let detail = alloc::format!("{}-dimensional, {} element(s)", a.row_count(), a.len());
            loss.note(LossKind::Dimensions, path, Some(&detail));
        }
        _ => {}
    }
}

/// `json_loss_scan` over a whole value, iteratively. Paths are cut at 255
/// bytes like the reference's. An annotation on an array also gives its
/// unit to the arrays and nulls in it that have no annotation of their own,
/// which the reference counts as values of their own.
fn scan(v: &Value, path: String, loss: &mut Loss) {
    let mut stack: Vec<(&Value, String, Option<&str>)> = alloc::vec![(v, path, None)];
    while let Some((v, path, inherited)) = stack.pop() {
        let unit = match v.kind() {
            Kind::Array(_) | Kind::Null if v.annotation().is_none() => inherited,
            _ => unit_of(v),
        };
        scan_one(v, &path, unit, loss);
        match v.kind() {
            Kind::Struct(s) => {
                for (key, child) in s.entries().iter().rev() {
                    stack.push((child, truncate(alloc::format!("{path}.{key}"), 256), None));
                }
            }
            Kind::Array(a) => {
                for (i, child) in a.items().iter().enumerate().rev() {
                    stack.push((child, truncate(alloc::format!("{path}[{i}]"), 256), unit));
                }
            }
            _ => {}
        }
    }
}

pub(super) fn decode(doc: &Document) -> Result<(String, Loss), JsonError> {
    let entries = doc.entries();
    if let Some((key, _)) = entries.iter().find(|(_, v)| has_fractional_datetime(v)) {
        return Err(JsonError::FractionalDatetime { key: key.clone() });
    }
    let mut loss = Loss::default();
    for (key, v) in entries {
        scan(v, truncate(alloc::format!(".{key}"), 256), &mut loss);
    }
    let mut out = String::new();
    print(&mut out, entries);
    out.push('\n');
    Ok((out, loss))
}
