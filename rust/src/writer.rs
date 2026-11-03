//! The streaming writer, mirroring `python/bovnar/writer.py`.

use crate::enums::{Event, TokenType, ValueTypeFamily};
use crate::error::{Error, Result};
use crate::ffi::{self, BvnrWriteFlags, ValueTypeSpec};
use crate::quantity::Quantity;
use crate::reader::UnitPolicy;
use crate::unit::Unit;

/// Default size of the memory sink (Python's `DEFAULT_MEM_CAP`).
pub const DEFAULT_MEM_CAP: usize = 4 * 1024 * 1024;

/// Largest octet-stream chunk: the most one chunk header can count.
const OCTET_CHUNK: usize = 65536;

/// A streaming BVNR writer into memory.
pub struct Writer {
    handle: ffi::WriterHandle,
    struct_depth: u32,
    finished: bool,
}

impl Writer {
    /// A writer into a memory buffer of `cap` bytes; `pretty` indents the
    /// output. Writing past `cap` fails with `sink_buffer_exhausted`.
    pub fn to_mem(cap: usize, pretty: bool) -> Result<Self> {
        Self::to_mem_with(cap, pretty, 0, false)
    }

    /// [`Writer::to_mem`] with the remaining keywords of Python's `to_mem`:
    /// `unit_flags` ([`crate::units::UnitFlags`]) rewrite units on output,
    /// and `emit_version` leads the document with `#!bovnar <major>.<minor>`.
    pub fn to_mem_with(
        cap: usize,
        pretty: bool,
        unit_flags: u32,
        emit_version: bool,
    ) -> Result<Self> {
        let handle = ffi::WriterHandle::new()
            .ok_or_else(|| Error::Argument("bvnr_writer_create() returned NULL".into()))?;
        let mut w = Writer {
            handle,
            struct_depth: 0,
            finished: false,
        };
        let mut flags = BvnrWriteFlags::zeroed();
        // The writer matches what the reader reads: the 255-level cap in both
        // directions, where zero would select the C default of 64.
        flags.max_struct_nesting = 255;
        flags.max_array_nesting = 255;
        flags.unit_flags = unit_flags;
        flags.emit_version = emit_version;
        if !w.handle.open_mem(cap, pretty, flags) {
            return Err(w.error());
        }
        Ok(w)
    }

    /// Sends one raw event, like Python's `Writer.emit`: `key` for an
    /// assignment, `value` (a literal) with its type `vt` for data.
    pub fn emit(
        &mut self,
        event: Event,
        key: Option<&str>,
        value: Option<&str>,
        vt: Option<ValueTypeSpec>,
    ) -> Result<()> {
        let vt = vt.unwrap_or_default();
        let (token, data) = match (value, key) {
            (Some(v), _) => {
                let family = ValueTypeFamily::from_raw(vt.family);
                let decimal_float = matches!(
                    family,
                    Some(ValueTypeFamily::FloatFix | ValueTypeFamily::FloatDec)
                );
                let token = if family == Some(ValueTypeFamily::Utf8)
                    || (vt.base != 0 && vt.base != 10 && !decimal_float)
                {
                    TokenType::String
                } else {
                    TokenType::Number
                };
                (token, v.as_bytes())
            }
            (None, Some(k)) => (TokenType::Identifier, k.as_bytes()),
            (None, None) => (TokenType::Identifier, &[][..]),
        };
        self.event(event, token, vt, data)
    }

    /// Emits a leading `#!bovnar <major>.<minor>` directive; must come before
    /// any value.
    pub fn write_version(&mut self, major: u16, minor: u16) -> Result<()> {
        self.check(|h| h.write_version(major, minor))
    }

    /// `.key = <uint:width[,_base]> value;`
    pub fn write_uint(&mut self, key: &str, value: u64, width: u32, base: u32) -> Result<()> {
        let vt = type_spec(ValueTypeFamily::Uint, width, base);
        let text = ffi::format_u64(value, base)
            .ok_or_else(|| Error::Argument(format!("cannot format {value} in base {base}")))?;
        self.write_scalar(key, vt, &text)
    }

    /// `.key = <sint:width[,_base]> value;`
    pub fn write_sint(&mut self, key: &str, value: i64, width: u32, base: u32) -> Result<()> {
        if base == 64 || base == 85 {
            return Err(Error::Argument(format!(
                "base {base} is unsigned: its alphabet uses '+' and '-' as digits, \
                 so a signed integer cannot be written in it"
            )));
        }
        let vt = type_spec(ValueTypeFamily::Sint, width, base);
        let text = ffi::format_i64(value, base)
            .ok_or_else(|| Error::Argument(format!("cannot format {value} in base {base}")))?;
        self.write_scalar(key, vt, &text)
    }

    /// `.key = <float:width> value;`
    pub fn write_float(&mut self, key: &str, value: f64, width: u32) -> Result<()> {
        let vt = type_spec(ValueTypeFamily::Float, width, 0);
        let text = ffi::format_f64(value, vt)
            .ok_or_else(|| Error::Argument(format!("cannot format {value}")))?;
        self.write_scalar(key, vt, &text)
    }

    /// `.key = <float_fix:width,qQ> value;` — a signed fixed point with `q`
    /// fractional bits.
    pub fn write_float_fix(&mut self, key: &str, value: f64, width: u32, q: u32) -> Result<()> {
        let text = format_float(value, width)?;
        let vt = type_spec(ValueTypeFamily::FloatFix, width, q);
        self.write_scalar(key, vt, &text)
    }

    /// `.key = <float_dec:width> value;` — an IEEE decimal float.
    pub fn write_float_dec(&mut self, key: &str, value: f64, width: u32) -> Result<()> {
        let text = format_float(value, width)?;
        self.write_scalar(key, type_spec(ValueTypeFamily::FloatDec, width, 0), &text)
    }

    /// `.key = <float:width[,_16]> literal;` — a float given as its literal,
    /// decimal (`base` 10) or hexadecimal (16), at any precision.
    pub fn write_bvnf_base(
        &mut self,
        key: &str,
        literal: &str,
        width: u32,
        base: u32,
    ) -> Result<()> {
        let vt_base = match base {
            10 => 0,
            16 => 16,
            _ => {
                return Err(Error::Argument(format!(
                    "write_bvnf_base: base must be 10 or 16, got {base}"
                )));
            }
        };
        self.write_scalar(
            key,
            type_spec(ValueTypeFamily::Float, width, vt_base),
            literal,
        )
    }

    /// `.key = <uint|sint:width[,_base]> value;` for an integer of any width
    /// up to 128 bits; `signed` picks sint or uint, and `None` writes negative
    /// values as sint.
    pub fn write_bvni(
        &mut self,
        key: &str,
        value: i128,
        width: u32,
        base: u32,
        signed: Option<bool>,
    ) -> Result<()> {
        let family = if signed.unwrap_or(value < 0) {
            refuse_signed_high_base(base)?;
            ValueTypeFamily::Sint
        } else {
            ValueTypeFamily::Uint
        };
        let text = format_bigint(value, base)?;
        let vt = type_spec(family, width, if base == 10 { 0 } else { base });
        self.write_scalar(key, vt, &text)
    }

    /// `.key = <annotation> literal;` for a typed scalar, as read by
    /// [`crate::parse_typed`]: type, unit, unit spelling and datetime
    /// fraction are kept. The annotation is left out where it says nothing
    /// the literal does not.
    pub fn write_quantity(&mut self, key: &str, q: &Quantity) -> Result<()> {
        if let Some(src) = q.unit_source.as_deref()
            && q.unit.is_dimensionless()
            && !Unit::parse(src).is_ok_and(|u| u.is_dimensionless())
        {
            return Err(Error::Argument(format!(
                "cannot serialise {key:?}: unit_source {src:?} does not spell this \
                 Quantity's unit, which is no unit at all"
            )));
        }
        self.assignment(key)?;
        if needs_annotation(q) {
            self.annotation(q.vtype, &q.unit, q.unit_source.as_deref())?;
        }
        self.value(q.token_type, q, false)
    }

    /// Refuses to emit a value a reader under `policy` would refuse. Only the
    /// validation half applies: `require_unit`, `require_dimension_of` and
    /// non-converting rules. `None` clears it.
    pub fn set_unit_policy(&mut self, policy: Option<&UnitPolicy>) -> Result<()> {
        if let Some(p) = policy
            && (!p.targets.is_empty()
                || p.normalise_si
                || p.base != 0
                || p.leave_inexact
                || p.rules.iter().any(|r| r.convert))
        {
            return Err(Error::Argument(
                "the writer takes only the validation half of a UnitPolicy \
                 (require_unit / require_dimension_of, and rules with convert=false)"
                    .into(),
            ));
        }
        if let Some(p) = policy {
            p.check_counts()?;
        }
        if self
            .handle
            .set_unit_policy(policy.map(UnitPolicy::spec).as_ref())
        {
            Ok(())
        } else {
            // The write-side-only fields are refused above with their own
            // message, so what reaches here is a unit or a rule path the
            // parser will not take: what the reader's setter refuses too.
            Err(Error::Argument(match policy {
                Some(p) => crate::reader::describe_unit_policy_refusal(p),
                None => "bvnr_writer_set_unit_policy failed".into(),
            }))
        }
    }

    /// `.key = "value";`
    pub fn write_string(&mut self, key: &str, value: &str) -> Result<()> {
        self.assignment(key)?;
        self.event(
            Event::Data,
            TokenType::String,
            ValueTypeSpec::default(),
            value.as_bytes(),
        )
    }

    /// `.key = true;` or `.key = false;`
    pub fn write_bool(&mut self, key: &str, value: bool) -> Result<()> {
        self.assignment(key)?;
        let text: &[u8] = if value { b"true" } else { b"false" };
        self.event(Event::Data, TokenType::Bool, ValueTypeSpec::default(), text)
    }

    /// `.key = null;`
    pub fn write_null(&mut self, key: &str) -> Result<()> {
        self.assignment(key)?;
        self.event(
            Event::Data,
            TokenType::NullValue,
            ValueTypeSpec::default(),
            &[],
        )
    }

    /// `.key = <octet stream>;`
    pub fn write_octets(&mut self, key: &str, value: &[u8]) -> Result<()> {
        self.assignment(key)?;
        self.octets(value)
    }

    /// An octet stream value, chunked.
    pub(crate) fn octets(&mut self, value: &[u8]) -> Result<()> {
        let none = ValueTypeSpec::default();
        self.event(Event::OctetStreamStart, TokenType::Identifier, none, &[])?;
        for chunk in value.chunks(OCTET_CHUNK) {
            self.event(Event::Data, TokenType::OctetStream, none, chunk)?;
        }
        self.event(Event::OctetStreamEnd, TokenType::Identifier, none, &[])
    }

    /// `.key = {` — close with [`Writer::end_struct`].
    pub fn begin_struct(&mut self, key: &str) -> Result<()> {
        self.assignment(key)?;
        self.emit(Event::StructStart, None, None, None)?;
        self.struct_depth += 1;
        Ok(())
    }

    /// `};`
    pub fn end_struct(&mut self) -> Result<()> {
        self.emit(Event::StructEnd, None, None, None)?;
        self.struct_depth = self.struct_depth.saturating_sub(1);
        Ok(())
    }

    /// `[` of an array row.
    pub fn begin_array_row(&mut self) -> Result<()> {
        self.emit(Event::ArrayRowStart, None, None, None)
    }

    /// `]` of an array row.
    pub fn end_array_row(&mut self) -> Result<()> {
        self.emit(Event::ArrayRowEnd, None, None, None)
    }

    /// `/` between array rows.
    pub fn new_array_dim(&mut self) -> Result<()> {
        self.emit(Event::ArrayDimStart, None, None, None)
    }

    /// Completes the document. Fails while a struct is still open.
    pub fn finish(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        if self.struct_depth != 0 {
            return Err(Error::Argument(format!(
                "finish() called with {} unclosed struct(s)",
                self.struct_depth
            )));
        }
        self.finished = true;
        self.check(ffi::WriterHandle::finish)
    }

    /// Bytes written so far.
    pub fn bytes_written(&self) -> u64 {
        self.handle.bytes_written()
    }

    /// The document written so far.
    pub fn get_output(&self) -> &[u8] {
        self.handle.output()
    }

    pub(crate) fn assignment(&mut self, key: &str) -> Result<()> {
        self.emit(Event::AssignmentStart, Some(key), None, None)
    }

    /// An assignment of one annotated scalar literal.
    fn write_scalar(&mut self, key: &str, vt: ValueTypeSpec, text: &str) -> Result<()> {
        self.assignment(key)?;
        self.annotation(vt, &Unit::default(), None)?;
        let special = matches!(text, "nan" | "inf" | "ninf");
        let decimal_float = matches!(
            ValueTypeFamily::from_raw(vt.family),
            Some(ValueTypeFamily::FloatFix | ValueTypeFamily::FloatDec)
        );
        let token = if vt.base != 0 && vt.base != 10 && !special && !decimal_float {
            TokenType::String
        } else {
            TokenType::Number
        };
        self.event(Event::Data, token, vt, text.as_bytes())
    }

    /// A type annotation: `<family:width[,_base|,qN|,epoch][,unit]>`, like
    /// Python's `_emit_annotation`; `unit_source` is the unit's spelling.
    pub fn annotation(
        &mut self,
        vt: ValueTypeSpec,
        unit: &Unit,
        unit_source: Option<&str>,
    ) -> Result<()> {
        use ValueTypeFamily as F;
        let fam = ValueTypeFamily::from_raw(vt.family);
        let none = Unit::default();
        let param = Event::TypeAnnotationTypeFamilyParam;
        self.data_event(
            Event::TypeAnnotationStart,
            TokenType::Type,
            vt,
            unit,
            [b"", b"", b""],
        )?;
        self.event(Event::TypeAnnotationTypeFamily, TokenType::Type, vt, &[])?;
        let sized = matches!(
            fam,
            Some(F::Uint | F::Sint | F::Float | F::FloatFix | F::FloatDec | F::Datetime)
        );
        if sized && vt.width != 0 {
            self.event(param, TokenType::TypeWidth, vt, &[])?;
        }
        if matches!(fam, Some(F::Float | F::Sint | F::Uint)) && vt.base != 0 && vt.base != 10 {
            self.event(param, TokenType::TypeBase, vt, &[])?;
        }
        if fam == Some(F::FloatFix) && vt.base != 0 {
            self.event(param, TokenType::TypeQ, vt, &[])?;
        }
        if fam == Some(F::Datetime)
            && vt.base != 0
            && let Some(epoch) = ffi::datetime_epoch(vt).0
        {
            self.data_event(
                param,
                TokenType::Unit,
                vt,
                &none,
                [epoch.as_bytes(), b"", b""],
            )?;
        }
        if unit.0.num_components > 0 && !matches!(fam, Some(F::Datetime | F::Utf8)) {
            let text = ffi::unit_to_string_ex(&unit.0, self.handle.unit_flags()).map_err(|_| {
                Error::Argument(
                    "cannot serialise this unit: bvn_unit_to_string_ex refused it, and \
                     writing the value without its unit would change what it means"
                        .into(),
                )
            })?;
            if !text.is_empty() {
                let src = unit_source.unwrap_or("").as_bytes();
                self.data_event(
                    param,
                    TokenType::Unit,
                    vt,
                    unit,
                    [text.as_bytes(), b"", src],
                )?;
            }
        }
        self.event(
            Event::TypeAnnotationEnd,
            TokenType::Type,
            ValueTypeSpec::default(),
            &[],
        )
    }

    /// The data event of a typed value. Inside an array the token becomes
    /// its array form and the unit stays with the annotation.
    pub(crate) fn value(&mut self, token: TokenType, q: &Quantity, in_array: bool) -> Result<()> {
        let none = Unit::default();
        let (token, unit) = match (in_array, token) {
            (false, t) => (t, &q.unit),
            (true, TokenType::String | TokenType::ArrayString) => (TokenType::ArrayString, &none),
            (true, TokenType::Number) => (TokenType::ArrayNumber, &none),
            (true, t) => (t, &none),
        };
        let raw = q.raw.as_deref().unwrap_or("").as_bytes();
        let frac = q.frac.as_deref().unwrap_or("").as_bytes();
        let src = if in_array {
            &[][..]
        } else {
            q.unit_source.as_deref().unwrap_or("").as_bytes()
        };
        self.data_event(Event::Data, token, q.vtype, unit, [raw, frac, src])
    }

    /// One event with every payload field: literal, fraction, unit source.
    pub(crate) fn data_event(
        &mut self,
        event: Event,
        token: TokenType,
        vt: ValueTypeSpec,
        unit: &Unit,
        parts: [&[u8]; 3],
    ) -> Result<()> {
        if parts.iter().any(|p| u32::try_from(p.len()).is_err()) {
            return Err(Error::Argument("a single value cannot exceed 4 GiB".into()));
        }
        self.check(|h| h.write_event_full(event as i32, token as i32, vt, &unit.0, parts))
    }

    /// Sends one raw event with an explicit token type, like Python's direct
    /// `bvnr_write_event` call.
    pub fn event(
        &mut self,
        event: Event,
        token: TokenType,
        vt: ValueTypeSpec,
        data: &[u8],
    ) -> Result<()> {
        if u32::try_from(data.len()).is_err() {
            return Err(Error::Argument("a single value cannot exceed 4 GiB".into()));
        }
        self.check(|h| h.write_event(event as i32, token as i32, vt, data))
    }

    pub(crate) fn check(
        &mut self,
        call: impl FnOnce(&mut ffi::WriterHandle) -> bool,
    ) -> Result<()> {
        if call(&mut self.handle) {
            Ok(())
        } else {
            Err(self.error())
        }
    }

    fn error(&self) -> Error {
        let (code, offset) = self.handle.error();
        Error::Write { code, offset }
    }
}

/// A number with a literal of a numeric family.
pub(crate) fn is_numeric(q: &Quantity) -> bool {
    use ValueTypeFamily as F;
    matches!(
        q.family(),
        Some(F::Float | F::FloatDec | F::FloatFix | F::Uint | F::Sint)
    ) && q.raw.is_some()
}

/// Python's `_needs_annotation`: true only when the annotation carries
/// something the bare literal would not give back.
pub(crate) fn needs_annotation(q: &Quantity) -> bool {
    use ValueTypeFamily as F;
    let vt = q.vtype;
    match ValueTypeFamily::from_raw(vt.family) {
        None | Some(F::Plain | F::Bool | F::Illegal) => false,
        // A bare non-negative integer reads back as uint.
        Some(F::Sint)
            if q.raw
                .as_deref()
                .is_some_and(|r| !r.trim_start().starts_with('-')) =>
        {
            true
        }
        Some(F::Datetime | F::FloatFix | F::FloatDec) => true,
        Some(_) if !q.unit.is_dimensionless() => true,
        Some(_) if vt.base != 0 && vt.base != 10 => true,
        Some(_) => vt.width != 0 && vt.width != 64,
    }
}

fn format_float(value: f64, width: u32) -> Result<String> {
    ffi::format_f64(value, type_spec(ValueTypeFamily::Float, width, 0))
        .ok_or_else(|| Error::Argument(format!("cannot format {value}")))
}

fn refuse_signed_high_base(base: u32) -> Result<()> {
    if base == 64 || base == 85 {
        return Err(Error::Argument(format!(
            "base {base} is unsigned: its alphabet uses '+' and '-' as digits, \
             so a signed integer cannot be written in it"
        )));
    }
    Ok(())
}

/// Python's `_format_bigint`: `value` in any bovnar base (2..62, 64, 85).
pub fn format_bigint(value: i128, base: u32) -> Result<String> {
    const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    if !(2..=62).contains(&base) && base != 64 && base != 85 {
        return Err(Error::Argument(format!("unsupported integer base {base}")));
    }
    let digit = |d: u32| -> char {
        let d8 = d as u8;
        match (base, d) {
            (64, _) => B64[d as usize] as char,
            (85, _) => (b'!' + d8) as char,
            (_, 0..=9) => (b'0' + d8) as char,
            (_, 10..=35) => (b'a' + d8 - 10) as char,
            _ => (b'A' + d8 - 36) as char,
        }
    };
    let mut mag = value.unsigned_abs();
    let mut digits = Vec::new();
    loop {
        digits.push(digit((mag % u128::from(base)) as u32));
        mag /= u128::from(base);
        if mag == 0 {
            break;
        }
    }
    let mut s: String = digits.iter().rev().collect();
    if value < 0 {
        s.insert(0, '-');
    }
    Ok(s)
}

pub(crate) fn type_spec(family: ValueTypeFamily, width: u32, base: u32) -> ValueTypeSpec {
    ValueTypeSpec {
        family: family as i32,
        width,
        base,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(f: impl FnOnce(&mut Writer) -> Result<()>) -> String {
        let mut w = Writer::to_mem(4096, false).unwrap();
        f(&mut w).unwrap();
        w.finish().unwrap();
        String::from_utf8(w.get_output().to_vec()).unwrap()
    }

    #[test]
    fn writes_scalars_and_structs() {
        let out = written(|w| {
            w.write_uint("u", 255, 64, 16)?;
            w.write_sint("i", -3, 64, 10)?;
            w.write_float("f", 1.5, 64)?;
            w.begin_struct("s")?;
            w.write_string("t", "x")?;
            w.write_null("n")?;
            w.end_struct()
        });
        assert!(out.contains("\"ff\""), "{out}");
        assert!(out.contains("-3") && out.contains("1.5"), "{out}");
        assert!(out.contains("\"x\""), "{out}");
    }

    #[test]
    fn writes_typed_annotations() {
        let out = written(|w| {
            w.write_float_fix("fx", 1.5, 32, 16)?;
            w.write_float_dec("fd", 0.1, 64)?;
            w.write_bvni("big", -(1i128 << 100), 102, 10, None)?;
            w.write_bvnf_base("hx", "1.8p1", 64, 16)?;
            let km = Unit::parse("k~m").unwrap();
            w.write_quantity(
                "q",
                &Quantity::new("2.5", type_spec(ValueTypeFamily::Float, 64, 0), km),
            )
        });
        assert!(out.contains("<float_fix:32,q16>"), "{out}");
        assert!(out.contains("<float_dec:64>"), "{out}");
        assert!(
            out.contains("<sint:102>-1267650600228229401496703205376"),
            "{out}"
        );
        assert!(out.contains("<float:64,_16>"), "{out}");
        assert!(out.contains("<float:64,k~m>2.5"), "{out}");
        assert_eq!(format_bigint(255, 16).unwrap(), "ff");
        assert_eq!(format_bigint(63, 64).unwrap(), "/");
    }

    #[test]
    fn writer_policy_refuses_bare_numbers() {
        let mut w = Writer::to_mem(4096, false).unwrap();
        let strict = UnitPolicy {
            require_unit: true,
            ..UnitPolicy::default()
        };
        w.set_unit_policy(Some(&strict)).unwrap();
        assert_eq!(
            w.write_float("bare", 0.25, 64).unwrap_err().name(),
            Some("unit_mismatch")
        );
        let converting = UnitPolicy {
            targets: vec![("m".into(), 0)],
            ..UnitPolicy::default()
        };
        assert!(w.set_unit_policy(Some(&converting)).is_err());
    }

    #[test]
    fn full_buffer_reports_sink_buffer_exhausted() {
        let mut w = Writer::to_mem(8, false).unwrap();
        let err = w
            .write_string("key", "a value longer than eight bytes")
            .and_then(|_| w.finish());
        assert_eq!(err.unwrap_err().name(), Some("sink_buffer_exhausted"));
    }
}
