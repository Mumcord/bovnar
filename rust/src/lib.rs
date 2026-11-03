//! Thin Rust binding to the Bovnar (BVNR) C library.
//!
//! The module split mirrors the Python binding in `python/bovnar/`. All FFI
//! lives in the private `ffi` module; every other module is safe Rust on top
//! of it.

#![deny(unsafe_code)]

pub mod currency;
pub mod dom;
pub mod enums;
pub mod error;
#[allow(unsafe_code)]
mod ffi;
pub mod json;
pub mod quantity;
pub mod reader;
pub mod stream;
pub mod unit;
pub mod units;
pub mod writer;

pub use dom::{DomDoc, DomNode, DomType};
pub use enums::{
    BaseUnit, ErrorCode, Event, Exponent, IECPrefix, PrefixSystem, SIPrefix, TokenType,
    ValueTypeFamily,
};
pub use error::{Error, Result};
pub use ffi::ValueTypeSpec;
pub use quantity::Quantity;
pub use reader::{EventData, EventPayload, ReadOptions, Reader, UnitPolicy, UnitRule};
pub use unit::{MAX_UNIT_COMPONENTS, Unit, UnitComponent};
pub use writer::Writer;

use writer::needs_annotation;

/// A BVNR value, the Rust counterpart of what Python's `loads` returns and
/// `dumps` takes. [`parse`] carries no annotations or units; [`parse_typed`]
/// returns typed scalars as [`Value::Quantity`].
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    /// Any integer up to 128 bits (uint64 and sint64 included).
    Int(i128),
    Float(f64),
    /// A string; symbols and references read back as strings too.
    String(String),
    /// An octet stream.
    Bytes(Vec<u8>),
    /// An array; a multi-row array is an array of row arrays.
    Array(Vec<Value>),
    /// A struct (or the whole document): keys in document order.
    Struct(Vec<(String, Value)>),
    /// A typed scalar with its literal, type and unit.
    Quantity(Box<Quantity>),
}

/// The library version, e.g. `"2.0.0"` (Python's `version`).
pub fn version() -> String {
    ffi::version_string()
}

/// The newest format version the library implements, as `(major, minor)`
/// (Python's `spec_version`).
pub fn spec_version() -> (u16, u16) {
    ffi::spec_version()
}

/// The `#!bovnar M.N` directive leading `data` (a UTF-8 BOM before it is
/// fine), or `None` when there is none (Python's `peek_version`).
pub fn peek_version(data: &[u8]) -> Option<(u16, u16)> {
    ffi::peek_version(data)
}

/// Parses a BVNR document into a [`Value::Struct`], like Python's `loads`.
///
/// This is the single-pass streaming parse: it enforces the grammar and each
/// value's own type, unit and range, but not the document-tier rules
/// (duplicate keys, array homogeneity, rectangular matrices). Use
/// [`DomDoc::parse`] for full validation.
pub fn parse(data: &[u8]) -> Result<Value> {
    parse_with(data, false)
}

/// Parses like [`parse`], but every typed scalar (number, datetime, symbol,
/// reference) is a [`Value::Quantity`] keeping its literal, type, unit and
/// datetime fraction, so [`to_string`] writes it back losslessly. Python's
/// `loads(typed=True)`.
pub fn parse_typed(data: &[u8]) -> Result<Value> {
    parse_with(data, true)
}

fn parse_with(data: &[u8], typed: bool) -> Result<Value> {
    let mut parser = dict_parser::DictParser::new(typed);
    let mut on_event = |d: &EventData<'_>| {
        parser.on_event(d);
        true
    };
    Reader::new()?.read_mem(data, &ReadOptions::default(), Some(&mut on_event), None)?;
    Ok(parser.result())
}

/// Serialises a [`Value::Struct`] to a BVNR document, like Python's `dumps`.
///
/// Returns bytes rather than `String`: an octet stream is written raw.
/// Everything [`parse`] and [`parse_typed`] return round-trips: arrays
/// (multi-row ones as `/`-separated rows), integers up to 128 bits and typed
/// quantities.
pub fn to_string(value: &Value, pretty: bool) -> Result<Vec<u8>> {
    let Value::Struct(entries) = value else {
        return Err(Error::Argument(
            "to_string() requires a struct at the top level".into(),
        ));
    };
    // A datetime or array-indexing reference needs spec 1.1, a profile unit
    // spelling 1.2; without the directive the output would not read back.
    let mut version = None;
    walk(std::slice::from_ref(value), true, &mut |v| {
        if let Value::Quantity(q) = v {
            version = version.max(spec_version_needed(q));
        }
    });
    writer::grow_write(writer::DEFAULT_MEM_CAP, pretty, |w| {
        if let Some(minor) = version {
            w.write_version(1, minor)?;
        }
        emit_entries(w, entries)
    })
}

fn emit_entries(w: &mut Writer, entries: &[(String, Value)]) -> Result<()> {
    entries.iter().try_for_each(|(k, v)| emit_value(w, k, v))
}

fn emit_value(w: &mut Writer, key: &str, value: &Value) -> Result<()> {
    match value {
        Value::Null => w.write_null(key),
        Value::Bool(b) => w.write_bool(key, *b),
        Value::Int(i) => {
            if let Ok(u) = u64::try_from(*i) {
                w.write_uint(key, u, 64, 10)
            } else if let Ok(s) = i64::try_from(*i) {
                w.write_sint(key, s, 64, 10)
            } else {
                w.write_bvni(key, *i, int_width(*i), 10, None)
            }
        }
        Value::Float(f) => w.write_float(key, *f, 64),
        Value::String(s) => w.write_string(key, s),
        Value::Bytes(b) => w.write_octets(key, b),
        Value::Struct(entries) => {
            w.begin_struct(key)?;
            emit_entries(w, entries)?;
            w.end_struct()
        }
        Value::Quantity(q) => w.write_quantity(key, q),
        Value::Array(rows) => {
            let mut vt = datetime_annotation(rows)?;
            let mut unit = None;
            if vt.is_none() {
                (vt, unit) = quantity_annotation(rows);
            }
            let has_unit = array_has_unit(rows);
            if unit.is_none() && has_unit && array_one_unit(rows) {
                // One unit, several numeric types: the format takes that
                // (homogeneity is about the unit), each element under its own
                // annotation.
                w.assignment(key)?;
                return emit_rows(w, rows, true, None);
            }
            if unit.is_none() && has_unit {
                return Err(Error::Argument(format!(
                    "cannot serialise {key:?}: its elements carry units but do not \
                     agree on one type and unit, and a Bovnar array has a single \
                     annotation for all of them; give them a common unit, or use a \
                     struct with one field per value"
                )));
            }
            if vt.is_none() && mixed_numeric_types(rows) {
                w.assignment(key)?;
                return emit_rows(w, rows, true, None);
            }
            if vt.is_none() {
                vt = int_annotation(rows);
            }
            if vt.is_none() && has_wide_int(rows) {
                return Err(Error::Argument(format!(
                    "cannot serialise {key:?}: it holds an integer wider than 64 \
                     bits beside elements that are not integers, and a Bovnar \
                     array has a single annotation to give the width in"
                )));
            }
            let src = unit.and_then(|_| array_unit_source(rows));
            write_array(w, key, rows, vt, unit.as_ref(), src.as_deref())
        }
    }
}

/// Bits a two's-complement (negative) or plain (non-negative) integer needs.
fn int_width(i: i128) -> u32 {
    if i < 0 {
        128 - (!i).leading_zeros() + 1
    } else {
        128 - i.leading_zeros()
    }
}

/// Calls `f` on every element inside `rows`, through nested arrays; with
/// `deep`, through structs too.
fn walk<'a>(rows: &'a [Value], deep: bool, f: &mut dyn FnMut(&'a Value)) {
    for v in rows {
        match v {
            Value::Array(inner) => walk(inner, deep, f),
            Value::Struct(entries) if deep => {
                for (_, v) in entries {
                    walk(std::slice::from_ref(v), deep, f);
                }
            }
            v => f(v),
        }
    }
}

/// The leaves of an array: its elements through nested arrays.
fn leaves<'a>(rows: &'a [Value], f: &mut dyn FnMut(&'a Value)) {
    walk(rows, false, f);
}

/// Python's `_uses_spec_1_1` / `_uses_spec_1_2`: the minor spec version a
/// quantity needs declared, if any.
fn spec_version_needed(q: &Quantity) -> Option<u16> {
    let profile_ns = q.unit_source.as_deref().is_some_and(|s| {
        s.split_once(':').is_some_and(|(ns, _)| {
            ns.bytes()
                .next()
                .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
                && ns
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        })
    });
    if profile_ns || (!q.unit.is_dimensionless() && q.unit.is_profile_only()) {
        Some(2)
    } else if q.family() == Some(ValueTypeFamily::Datetime)
        || (q.token_type == TokenType::Reference
            && q.raw.as_deref().is_some_and(|r| r.contains('[')))
    {
        Some(1)
    } else {
        None
    }
}

/// The quantity leaves of an array, nulls skipped; `None` if any other
/// leaf is not a quantity.
fn quantity_leaves(rows: &[Value]) -> Option<Vec<&Quantity>> {
    let mut out = Vec::new();
    let mut all = true;
    leaves(rows, &mut |v| match v {
        Value::Null => {}
        Value::Quantity(q) => out.push(&**q),
        _ => all = false,
    });
    all.then_some(out)
}

fn has_real_unit(q: &Quantity) -> bool {
    !q.unit.is_dimensionless()
}

/// Python's `_array_datetime_annotation`: one datetime annotation for an
/// array of datetimes (widest width wins); mixed epochs are refused.
fn datetime_annotation(rows: &[Value]) -> Result<Option<ValueTypeSpec>> {
    let (mut saw, mut other, mut width, mut base) = (false, false, 0, None);
    let mut mixed = false;
    leaves(rows, &mut |v| match v {
        Value::Quantity(q) if q.family() == Some(ValueTypeFamily::Datetime) => {
            saw = true;
            width = width.max(q.vtype.width);
            mixed |= *base.get_or_insert(q.vtype.base) != q.vtype.base;
        }
        _ => other = true,
    });
    if other || !saw {
        return Ok(None);
    }
    if mixed {
        return Err(Error::Argument(
            "cannot serialise a datetime array whose elements use different \
             epochs; split it into one array per epoch"
                .into(),
        ));
    }
    let width = if width == 0 { 64 } else { width };
    Ok(Some(writer::type_spec(
        ValueTypeFamily::Datetime,
        width,
        base.unwrap_or(0),
    )))
}

/// Python's `_array_quantity_annotation`: the annotation every quantity
/// leaf shares, when they agree on the effective type (`<uint:64>` is
/// `<uint:64,_10>`) and the unit (`m*s` is `s*m`) and the annotation says
/// something. The unit is `None` when it is no unit at all.
fn quantity_annotation(rows: &[Value]) -> (Option<ValueTypeSpec>, Option<Unit>) {
    use ValueTypeFamily as F;
    let Some(qs) = quantity_leaves(rows) else {
        return (None, None);
    };
    let key = |q: &Quantity| {
        let fam = q.family();
        let base = match (q.vtype.base, fam) {
            (0, Some(F::Uint | F::Sint | F::Float)) => 10,
            (b, _) => b,
        };
        let width = if q.vtype.width == 0 {
            64
        } else {
            q.vtype.width
        };
        (q.vtype.family, width, base)
    };
    let Some(first) = qs.first() else {
        return (None, None);
    };
    if qs.iter().any(|q| q.family() == Some(F::Datetime))
        || qs
            .iter()
            .any(|q| q.unit != first.unit || key(q) != key(first))
        || first.family() == Some(F::Plain)
        || !qs.iter().any(|q| needs_annotation(q))
    {
        return (None, None);
    }
    (
        Some(first.vtype),
        has_real_unit(first).then_some(first.unit),
    )
}

/// Python's `_array_has_unit`: any quantity leaf with a real unit.
fn array_has_unit(rows: &[Value]) -> bool {
    let mut any = false;
    leaves(rows, &mut |v| {
        any |= matches!(v, Value::Quantity(q) if has_real_unit(q));
    });
    any
}

/// Python's `_array_one_unit`: every non-null leaf a numeric quantity with
/// a literal, all with the same real unit.
fn array_one_unit(rows: &[Value]) -> bool {
    let Some(qs) = quantity_leaves(rows) else {
        return false;
    };
    let Some(first) = qs.first() else {
        return false;
    };
    qs.iter()
        .all(|q| writer::is_numeric(q) && has_real_unit(q) && q.unit == first.unit)
}

/// Python's `_array_unit_source`: the one spelling every quantity leaf
/// agrees on, else `None`.
fn array_unit_source(rows: &[Value]) -> Option<String> {
    let (mut src, mut agree) = (None::<&str>, true);
    leaves(rows, &mut |v| {
        if let Value::Quantity(q) = v {
            match (q.unit_source.as_deref(), src) {
                (None, _) => agree = false,
                (Some(s), None) => src = Some(s),
                (Some(s), Some(first)) => agree &= s == first,
            }
        }
    });
    src.filter(|_| agree).map(str::to_owned)
}

/// Python's `_array_mixed_numeric_types`: unit-less numeric quantities, at
/// least one with a type default synthesis would not give back.
fn mixed_numeric_types(rows: &[Value]) -> bool {
    use ValueTypeFamily as F;
    let Some(qs) = quantity_leaves(rows) else {
        return false;
    };
    let numeric = |q: &Quantity| {
        matches!(
            q.family(),
            Some(F::Float | F::FloatDec | F::FloatFix | F::Uint | F::Sint)
        )
    };
    if !qs.iter().all(|q| numeric(q) && !has_real_unit(q)) {
        return false;
    }
    qs.iter().any(|q| q.raw.is_some() && needs_annotation(q))
}

/// Bits Python's `int.bit_length()` gives for `|i|`.
fn bit_length(i: i128) -> u32 {
    128 - i.unsigned_abs().leading_zeros()
}

/// Python's `_array_has_wide_int`: whether a plain integer leaf needs more
/// than 64 bits (unsigned when non-negative, signed when negative).
fn has_wide_int(rows: &[Value]) -> bool {
    let mut wide = false;
    leaves(
        rows,
        &mut |v| wide |= matches!(v, Value::Int(i) if !(i128::from(i64::MIN)..=i128::from(u64::MAX)).contains(i)),
    );
    wide
}

/// Python's `_array_int_annotation`: for an all-integer array with an
/// element wider than 64 bits, the width that holds every element.
fn int_annotation(rows: &[Value]) -> Option<ValueTypeSpec> {
    let (mut saw, mut other, mut neg) = (false, false, false);
    let (mut pos_bits, mut neg_bits) = (0, 0);
    leaves(rows, &mut |v| match v {
        Value::Int(i) => {
            saw = true;
            if *i < 0 {
                neg = true;
                neg_bits = neg_bits.max(bit_length(*i));
            } else {
                pos_bits = pos_bits.max(bit_length(*i));
            }
        }
        Value::Null => {}
        _ => other = true,
    });
    if !saw || other {
        return None;
    }
    let (family, width) = if neg {
        (ValueTypeFamily::Sint, (pos_bits + 1).max(neg_bits + 1))
    } else {
        (ValueTypeFamily::Uint, pos_bits)
    };
    (width > 64).then(|| writer::type_spec(family, width, 10))
}

/// Writes `.key = [<annotation>] rows;`, like Python's `write_array`: a flat
/// list is one row, a rectangular list of lists is `/`-separated rows. With
/// `vt`, the annotation (and `unit`, spelled `unit_source` if given) is
/// written once for the whole array.
pub fn write_array(
    w: &mut Writer,
    key: &str,
    rows: &[Value],
    vt: Option<ValueTypeSpec>,
    unit: Option<&Unit>,
    unit_source: Option<&str>,
) -> Result<()> {
    if let Some(vt) = vt {
        check_rows(rows, vt)?;
    }
    w.assignment(key)?;
    if let Some(vt) = vt {
        w.annotation(vt, unit.unwrap_or(&Unit::default()), unit.and(unit_source))?;
    }
    emit_rows(w, rows, false, vt)
}

/// Python's `_check_rows`: runs every element through the refusals
/// [`emit_element`] would raise under `vt`, writing nothing. A refusal in the
/// middle of the array used to leave the writer holding `.p=<uint:16>[80,` --
/// an open array no later write could close, so the document was lost. (A
/// struct element's own fields are checked when they are written; without an
/// annotation every `Value` has a Bovnar form, so there is nothing to check.)
fn check_rows(rows: &[Value], vt: ValueTypeSpec) -> Result<()> {
    for elem in rows {
        match elem {
            Value::Quantity(_) | Value::Null => {}
            Value::Array(inner) => check_rows(inner, vt)?,
            Value::Struct(_) | Value::Bytes(_) => {
                return Err(refuse_under(elem, vt, "an annotated array holds scalars"));
            }
            _ => {
                held_element(elem, vt)?;
            }
        }
    }
    Ok(())
}

/// Python's `_emit_rows`: a single row, and rows of unequal length, are
/// nested rather than `/`-separated, so the shape reads back unchanged.
fn emit_rows(
    w: &mut Writer,
    rows: &[Value],
    annotate: bool,
    vt: Option<ValueTypeSpec>,
) -> Result<()> {
    let row_len = |v: &Value| match v {
        Value::Array(r) => Some(r.len()),
        _ => None,
    };
    let first_len = rows.first().and_then(row_len);
    if rows.len() > 1 && first_len.is_some() && rows.iter().all(|r| row_len(r) == first_len) {
        for (i, row) in rows.iter().enumerate() {
            if i > 0 {
                w.new_array_dim()?;
            }
            w.begin_array_row()?;
            if let Value::Array(elems) = row {
                for e in elems {
                    emit_element(w, e, annotate, vt)?;
                }
            }
            w.end_array_row()?;
        }
        return Ok(());
    }
    w.begin_array_row()?;
    for e in rows {
        emit_element(w, e, annotate, vt)?;
    }
    w.end_array_row()
}

/// Python's `_emit_array_element`.
fn emit_element(
    w: &mut Writer,
    elem: &Value,
    annotate: bool,
    vt: Option<ValueTypeSpec>,
) -> Result<()> {
    let none = Unit::default();
    let data = |w: &mut Writer, token, vt, text: &[u8]| {
        w.data_event(Event::Data, token, vt, &none, [text, b"", b""])
    };
    if let Some(held) = vt
        && !matches!(elem, Value::Null | Value::Quantity(_) | Value::Array(_))
    {
        if matches!(elem, Value::Struct(_) | Value::Bytes(_)) {
            return Err(refuse_under(elem, held, "an annotated array holds scalars"));
        }
        if let Some((token, text)) = held_element(elem, held)? {
            return data(w, token, held, text.as_bytes());
        }
    }
    match elem {
        Value::Quantity(q) => {
            if annotate && writer::is_numeric(q) && q.raw.is_some() {
                w.annotation(q.vtype, &q.unit, q.unit_source.as_deref())?;
            }
            w.value(q.token_type, q, true)
        }
        Value::Null => data(w, TokenType::NullValue, ValueTypeSpec::default(), b""),
        Value::Bool(b) => {
            let text: &[u8] = if *b { b"true" } else { b"false" };
            data(w, TokenType::Bool, ValueTypeSpec::default(), text)
        }
        Value::Int(i) => {
            let vt = vt.unwrap_or_else(|| {
                let family = if *i < 0 {
                    ValueTypeFamily::Sint
                } else {
                    ValueTypeFamily::Uint
                };
                writer::type_spec(family, int_width(*i).max(64), 10)
            });
            data(w, TokenType::ArrayNumber, vt, i.to_string().as_bytes())
        }
        Value::Float(f) => {
            let vt = writer::type_spec(ValueTypeFamily::Float, 64, 0);
            let text = ffi::format_f64(*f, vt)
                .ok_or_else(|| Error::Argument(format!("cannot format {f}")))?;
            data(w, TokenType::ArrayNumber, vt, text.as_bytes())
        }
        Value::String(s) => data(
            w,
            TokenType::ArrayString,
            ValueTypeSpec::default(),
            s.as_bytes(),
        ),
        Value::Bytes(b) => w.octets(b),
        Value::Struct(entries) => {
            w.emit(Event::StructStart, None, None, None)?;
            emit_entries(w, entries)?;
            w.emit(Event::StructEnd, None, None, None)
        }
        Value::Array(rows) => emit_rows(w, rows, annotate, vt),
    }
}

/// Python's `_refuse_under`.
fn refuse_under(elem: &Value, vt: ValueTypeSpec, why: &str) -> Error {
    let fam = family_name(vt.family).unwrap_or("plain");
    let spec = if vt.width == 0 {
        fam.to_owned()
    } else {
        format!("{fam}:{}", vt.width)
    };
    Error::Argument(format!(
        "array element {elem:?} cannot be written under <{spec}>: {why}"
    ))
}

fn family_name(family: i32) -> Option<&'static str> {
    use ValueTypeFamily as F;
    Some(match ValueTypeFamily::from_raw(family)? {
        F::Utf8 => "utf8",
        F::Sint => "sint",
        F::Uint => "uint",
        F::Float => "float",
        F::FloatFix => "float_fix",
        F::FloatDec => "float_dec",
        F::Datetime => "datetime",
        F::Bool => "bool",
        _ => return None,
    })
}

/// Python's `_exact_number_text` for a float: the shortest round-tripping
/// decimal, with the format's spellings of the non-finite values.
fn exact_float_text(f: f64) -> String {
    if f.is_nan() {
        "nan".into()
    } else if f.is_infinite() {
        if f > 0.0 { "inf" } else { "ninf" }.into()
    } else {
        format!("{f:?}")
    }
}

/// Python's `_emit_held_element`: the token and literal of plain scalar
/// `elem` held to the array annotation `vt`, refused where it does not fit.
/// `None` for an annotation that holds its elements to nothing.
fn held_element(elem: &Value, vt: ValueTypeSpec) -> Result<Option<(TokenType, String)>> {
    use ValueTypeFamily as F;
    let Some(fam) = family_name(vt.family).and(ValueTypeFamily::from_raw(vt.family)) else {
        return Ok(None);
    };
    let refuse = |why: &str| Err(refuse_under(elem, vt, why));
    let width = if vt.width == 0 { 64 } else { vt.width };
    let held = match (fam, elem) {
        (F::Bool, Value::Bool(b)) => (TokenType::Bool, b.to_string()),
        (F::Bool, _) => return refuse("it is not a bool"),
        (F::Utf8, Value::String(s)) => (TokenType::ArrayString, s.clone()),
        (F::Utf8, _) => return refuse("it is not a str"),
        (_, Value::Bool(_)) => return refuse("a bool is not a number"),
        (F::Uint | F::Sint | F::Datetime, Value::Int(i)) => {
            let (lo, hi) = if fam == F::Uint {
                (
                    0,
                    if width >= 127 {
                        i128::MAX
                    } else {
                        (1 << width) - 1
                    },
                )
            } else if width >= 128 {
                (i128::MIN, i128::MAX)
            } else {
                (-(1 << (width - 1)), (1 << (width - 1)) - 1)
            };
            if !(lo..=hi).contains(i) {
                return refuse(&format!("it is outside [{lo}, {hi}]"));
            }
            // A datetime's `base` is its epoch index, not a radix.
            let base = match (fam, vt.base) {
                (F::Datetime, _) | (_, 0) => 10,
                (_, b) => b,
            };
            if base == 10 {
                (TokenType::ArrayNumber, i.to_string())
            } else {
                if fam == F::Sint && (base == 64 || base == 85) {
                    return refuse(&format!("base {base} is unsigned"));
                }
                // Quoted, as the scalar writers do for every non-decimal base.
                (TokenType::ArrayString, writer::format_bigint(*i, base)?)
            }
        }
        (F::Uint | F::Sint | F::Datetime, _) => return refuse("it is not an integer"),
        (_, Value::Int(_) | Value::Float(_)) => {
            if fam == F::Float && vt.base != 0 && vt.base != 10 {
                return refuse(
                    "a non-decimal float literal has to be given verbatim, as a Quantity",
                );
            }
            let text = match elem {
                Value::Float(f) => exact_float_text(*f),
                Value::Int(i) => i.to_string(),
                _ => unreachable!("the arm matched an Int or a Float"),
            };
            // decimal32 tops out below 9.9999995e96; the wider formats hold
            // every f64.
            if fam == F::FloatDec
                && width == 32
                && matches!(elem, Value::Float(f) if f.is_finite() && f.abs() >= 9.999_999_5e96)
            {
                return refuse(&format!(
                    "{text} is out of range for float_dec:32 (it would materialise to infinity)"
                ));
            }
            if fam == F::FloatFix
                && !matches!(text.as_str(), "nan" | "inf" | "ninf")
                && !ffi::float_str_fits_fix(&text, width, vt.base)
            {
                return refuse(&format!("it does not fit q{}", vt.base));
            }
            (TokenType::ArrayNumber, text)
        }
        _ => return refuse("it is not a number"),
    };
    Ok(Some(held))
}

/// Python's `_decode_value`: one literal as a plain value. What cannot be
/// decoded stays its text, as in Python.
pub(crate) fn decode_value(raw: &[u8], vt: ValueTypeSpec, token: Option<TokenType>) -> Value {
    if raw.is_empty() {
        return match token {
            Some(TokenType::String | TokenType::ArrayString) => Value::String(String::new()),
            _ => Value::Null,
        };
    }
    let text = String::from_utf8_lossy(raw).into_owned();
    match token {
        // The validator normalises on/off to true/false.
        Some(TokenType::Bool) => return Value::Bool(text == "true"),
        Some(TokenType::Symbol) => return Value::String(text),
        _ => {}
    }
    let base = if vt.base > 1 { vt.base } else { 10 };
    match ValueTypeFamily::from_raw(vt.family) {
        // Epoch seconds; `base` holds the epoch index, not a radix.
        Some(ValueTypeFamily::Datetime) => dict_parser::int_from_base(&text, 10).map(Value::Int),
        Some(ValueTypeFamily::Uint | ValueTypeFamily::Sint) => {
            dict_parser::int_from_base(&text, base).map(Value::Int)
        }
        Some(
            family @ (ValueTypeFamily::Float
            | ValueTypeFamily::FloatFix
            | ValueTypeFamily::FloatDec),
        ) => match text.as_str() {
            "nan" => Some(Value::Float(f64::NAN)),
            "inf" => Some(Value::Float(f64::INFINITY)),
            "ninf" => Some(Value::Float(f64::NEG_INFINITY)),
            // float_fix keeps Q and float_dec nothing in `base`: both are
            // always decimal.
            _ if family == ValueTypeFamily::Float && base == 16 => {
                ffi::parse_float_base(&text, 16).map(Value::Float)
            }
            _ => text.parse().ok().map(Value::Float),
        },
        _ => None,
    }
    .unwrap_or(Value::String(text))
}

/// The event consumer behind [`parse`], a port of Python's `_DictParser`.
///
/// Arrays are sealed lazily: a row end leaves its array pending, and the next
/// event decides whether a `/` continues it, an enclosing row takes it as an
/// element, or the assignment it belongs to is complete.
///
/// Public but hidden, like Python's importable `_DictParser`, so the event
/// handling can be tested with hand-built event sequences.
#[doc(hidden)]
pub mod dict_parser {
    use crate::{Event, EventData, Quantity, TokenType, Value, ValueTypeFamily, decode_value};

    #[derive(Default)]
    pub struct ArrScope {
        pub rows: Vec<Vec<Value>>,
        cur_row: Vec<Value>,
        dim_continue: bool,
        pending_seal: bool,
        /// The key the array is assigned to, saved when it opened.
        parent_key: Option<String>,
    }

    impl ArrScope {
        /// One row unwraps to a flat array; several become an array of rows.
        pub fn seal(mut self) -> Value {
            if self.rows.len() == 1 {
                Value::Array(self.rows.pop().unwrap_or_default())
            } else {
                Value::Array(self.rows.into_iter().map(Value::Array).collect())
            }
        }
    }

    enum Scope {
        Doc(Vec<(String, Value)>),
        Struct(Vec<(String, Value)>, Option<String>),
        Array(ArrScope),
    }

    pub struct DictParser {
        stack: Vec<Scope>,
        current_key: Option<String>,
        in_octet: bool,
        octets: Vec<u8>,
        /// Python's `_TypedDictParser`: typed scalars become quantities.
        typed: bool,
    }

    impl DictParser {
        pub fn new(typed: bool) -> Self {
            DictParser {
                stack: vec![Scope::Doc(Vec::new())],
                current_key: None,
                in_octet: false,
                octets: Vec::new(),
                typed,
            }
        }
    }

    /// Dict assignment: a repeated key keeps its first position, last value.
    fn insert(entries: &mut Vec<(String, Value)>, key: String, value: Value) {
        match entries.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => entries.push((key, value)),
        }
    }

    impl DictParser {
        pub fn on_event(&mut self, d: &EventData<'_>) {
            match d.event {
                Event::AssignmentStart => {
                    self.maybe_seal_array();
                    self.current_key = Some(String::from_utf8_lossy(d.raw).into_owned());
                }
                Event::StructStart => {
                    let key = self.current_key.take();
                    self.stack.push(Scope::Struct(Vec::new(), key));
                }
                Event::StructEnd => {
                    self.maybe_seal_array();
                    if self.stack.len() > 1
                        && matches!(self.stack.last(), Some(Scope::Struct(..)))
                        && let Some(Scope::Struct(entries, key)) = self.stack.pop()
                    {
                        self.commit(key, Value::Struct(entries));
                    }
                }
                Event::ArrayDimStart => {
                    if let Some(Scope::Array(a)) = self.stack.last_mut() {
                        a.pending_seal = false;
                        a.dim_continue = true;
                    }
                }
                Event::ArrayRowStart => match self.stack.last_mut() {
                    Some(Scope::Array(a)) if a.dim_continue => {
                        a.cur_row.clear();
                        a.dim_continue = false;
                    }
                    _ => {
                        if matches!(self.stack.last(), Some(Scope::Array(a)) if a.pending_seal) {
                            self.seal_top();
                        }
                        let parent_key = self.current_key.take();
                        self.stack.push(Scope::Array(ArrScope {
                            parent_key,
                            ..ArrScope::default()
                        }));
                    }
                },
                Event::ArrayRowEnd => {
                    self.flush_pending_inner_arrays();
                    if let Some(Scope::Array(a)) = self.stack.last_mut() {
                        let row = std::mem::take(&mut a.cur_row);
                        a.rows.push(row);
                        a.pending_seal = true;
                    }
                }
                Event::OctetStreamStart => {
                    self.in_octet = true;
                    self.octets.clear();
                }
                Event::OctetStreamEnd => {
                    self.in_octet = false;
                    let bytes = std::mem::take(&mut self.octets);
                    self.push_value(Value::Bytes(bytes));
                }
                Event::Data if self.in_octet => self.octets.extend_from_slice(d.raw),
                Event::Data => {
                    let value = if self.typed {
                        typed_value(d)
                    } else {
                        decode_value(d.raw, d.value_type, d.token_type)
                    };
                    self.push_value(value);
                }
                _ => {}
            }
        }

        pub fn result(mut self) -> Value {
            self.maybe_seal_array();
            while self.stack.len() > 1 {
                // An unterminated scope (continue_on_error) still lands.
                match self.stack.pop() {
                    Some(Scope::Struct(entries, key)) => self.commit(key, Value::Struct(entries)),
                    Some(Scope::Array(a)) => {
                        let key = a.parent_key.clone();
                        self.commit(key, a.seal());
                    }
                    _ => {}
                }
            }
            match self.stack.pop() {
                Some(Scope::Doc(entries)) => Value::Struct(entries),
                _ => Value::Struct(Vec::new()),
            }
        }

        /// A scalar or octet stream: into the open row, or under the
        /// current key.
        fn push_value(&mut self, value: Value) {
            match self.stack.last_mut() {
                Some(Scope::Doc(entries) | Scope::Struct(entries, _)) => {
                    if let Some(key) = self.current_key.take() {
                        insert(entries, key, value);
                    }
                }
                Some(Scope::Array(a)) => a.cur_row.push(value),
                None => {}
            }
        }

        /// A finished struct or array: into the open row, or under the key
        /// saved when it opened.
        fn commit(&mut self, key: Option<String>, value: Value) {
            match self.stack.last_mut() {
                Some(Scope::Doc(entries) | Scope::Struct(entries, _)) => {
                    if let Some(key) = key {
                        insert(entries, key, value);
                    }
                }
                Some(Scope::Array(a)) => a.cur_row.push(value),
                None => {}
            }
        }

        fn seal_top(&mut self) {
            if let Some(Scope::Array(a)) = self.stack.pop() {
                let key = a.parent_key.clone();
                self.commit(key, a.seal());
            }
        }

        fn maybe_seal_array(&mut self) {
            while matches!(self.stack.last(), Some(Scope::Array(a)) if a.pending_seal) {
                self.seal_top();
            }
        }

        /// Before a row ends, completed inner arrays become its elements.
        fn flush_pending_inner_arrays(&mut self) {
            while self.stack.len() >= 2
                && matches!(self.stack.last(), Some(Scope::Array(a)) if a.pending_seal)
            {
                self.seal_top();
            }
        }
    }

    /// Python's `_TypedDictParser._decode_data`.
    fn typed_value(d: &EventData<'_>) -> Value {
        let plain = ValueTypeFamily::from_raw(d.value_type.family) == Some(ValueTypeFamily::Plain);
        let symbolic = matches!(d.token_type, Some(TokenType::Symbol | TokenType::Reference));
        if plain && !symbolic {
            return decode_value(d.raw, d.value_type, d.token_type);
        }
        let text = |b: &[u8]| (!b.is_empty()).then(|| String::from_utf8_lossy(b).into_owned());
        Value::Quantity(Box::new(Quantity {
            raw: text(d.raw),
            vtype: d.value_type,
            unit: if plain {
                crate::Unit::default()
            } else {
                d.value_unit
            },
            token_type: d.token_type.unwrap_or(TokenType::Number),
            frac: text(d.frac),
            unit_source: text(d.unit_source),
        }))
    }

    /// Python's `_char_to_digit`: the digit value of `c` in `base`, or `None`.
    /// Bases 64 and 85 use the Base64 and Ascii85 alphabets; up to 36 letters
    /// are case-insensitive, 37..62 case-sensitive.
    fn char_to_digit(c: u8, base: u32) -> Option<u32> {
        let d = match (base, c) {
            (64, b'A'..=b'Z') => u32::from(c - b'A'),
            (64, b'a'..=b'z') => u32::from(c - b'a') + 26,
            (64, b'0'..=b'9') => u32::from(c - b'0') + 52,
            (64, b'+') => 62,
            (64, b'/') => 63,
            (64, _) => return None,
            (85, b'!'..=b'u') => u32::from(c - b'!'),
            (85, _) => return None,
            (_, b'0'..=b'9') => u32::from(c - b'0'),
            (_, b'a'..=b'z') => 10 + u32::from(c - b'a'),
            (_, b'A'..=b'Z') if base > 36 => 36 + u32::from(c - b'A'),
            (_, b'A'..=b'Z') => 10 + u32::from(c - b'A'),
            _ => return None,
        };
        (d < base).then_some(d)
    }

    /// Python's `_int_from_base`: an integer in any bovnar base (2..62, 64,
    /// 85). Bases 64 and 85 are unsigned: their '+' and '-' are digits.
    pub(super) fn int_from_base(text: &str, base: u32) -> Option<i128> {
        let bytes = text.as_bytes();
        let (neg, digits) = match bytes.first() {
            Some(b'+') if base != 64 && base != 85 => (false, &bytes[1..]),
            Some(b'-') if base != 64 && base != 85 => (true, &bytes[1..]),
            _ => (false, bytes),
        };
        if digits.is_empty() {
            return None;
        }
        let mut val: i128 = 0;
        for &c in digits {
            let d = char_to_digit(c, base)?;
            val = val
                .checked_mul(i128::from(base))?
                .checked_add(i128::from(d))?;
        }
        Some(if neg { -val } else { val })
    }
}
