//! Token validation and event emission (`src/validator/bovnar_validator.c`).
//!
//! The lexer hands each finished token and structural boundary to the
//! [`Validator`], which checks it against its type annotation and queues the
//! verified events. Methods follow the C convention on purpose: they return
//! `false` after storing the error in [`Validator::last_error`], so the lexer
//! port can mirror the C control flow one to one.

use alloc::string::String;
use alloc::vec::Vec;

use super::annotation;
use super::queue::{EventQueue, QueuedEvent};
use crate::event::TokenKind;
use crate::num;
use crate::types::{TypeFamily, TypeSpec};
use crate::units::{self, Unit};
use crate::{Error, ErrorKind, SpecVersion};

/// Structural events the lexer reports (`bvnr_event_t`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ev {
    StreamStart,
    AssignmentStart,
    OctetStreamStart,
    OctetStreamEnd,
    StructStart,
    StructEnd,
    ArrayRowEnd,
    ArrayDimStart,
    Data,
    StreamEnd,
}

/// A finished token (`bvnr_raw_token_t`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct RawToken<'a> {
    pub kind: TokenKind,
    pub event: Ev,
    pub data: &'a [u8],
    pub type_data: &'a [u8],
    pub inline_unit: &'a [u8],
}

/// The literal accumulator (`acc_*` fields of `bvnr_validator_t`).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Acc {
    pub value: u64,
    pub overflow: bool,
    pub has_dot: bool,
    pub has_exp: bool,
    pub exp_state: u8,
}

/// Validator state (`bvnr_validator_t`, reader part only).
pub(crate) struct Validator {
    pub value_type: TypeSpec,
    pub parsed_unit: Unit,
    pub inferred_default_vtype: TypeSpec,
    pub parsed_unit_serial: u64,
    pub acc: Acc,
    pub has_annotation_unit: bool,
    /// Spelling of `parsed_unit` (`unit_src`), at most 255 bytes.
    pub unit_src: Vec<u8>,
    /// Set by the lexer once the version directive is parsed.
    pub declared_version: Option<SpecVersion>,
    pub last_error: Option<ErrorKind>,
    pub error_line: u64,
    pub error_column: u64,
    pub error_byte: u32,
    pub error_offset: u64,
    /// Verified events waiting to be handed out.
    pub queue: EventQueue,
}

/// `"no_unit"`, the unit parameter of a synthesised numeric annotation.
const NO_UNIT: &[u8] = b"no_unit";
/// Longest unit spelling kept for `unit_src` (`BVNR_UNIT_SOURCE_MAX`).
const UNIT_SOURCE_MAX: usize = 255;

/// The part of `s` a C function taking a NUL-terminated string would see.
fn c_str(s: &[u8]) -> &[u8] {
    s.iter()
        .position(|&b| b == 0)
        .and_then(|end| s.get(..end))
        .unwrap_or(s)
}

fn is_number(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::Number | TokenKind::ArrayNumber)
}

fn is_string(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::String | TokenKind::ArrayString)
}

/// Whether a unit is a single currency (`bvn_unit_is_currency` on the only
/// component).
fn unit_is_currency(unit: &Unit) -> bool {
    matches!(unit.components(), [c] if c.base().is_currency())
}

/// `bvn_default_type_equiv`: same effective type, and for datetimes the same
/// epoch.
fn default_type_equiv(a: TypeSpec, b: TypeSpec) -> bool {
    if a.family == TypeFamily::Datetime && b.family == TypeFamily::Datetime && a.base != b.base {
        return false;
    }
    a.family == b.family
        && a.effective_width() == b.effective_width()
        && a.effective_base() == b.effective_base()
        && a.effective_q() == b.effective_q()
}

const fn spec(family: TypeFamily) -> TypeSpec {
    TypeSpec {
        family,
        width: 64,
        base: 0,
    }
}

impl Validator {
    pub(crate) fn new() -> Self {
        Self {
            value_type: TypeSpec::PLAIN,
            parsed_unit: Unit::no_unit(),
            inferred_default_vtype: TypeSpec::PLAIN,
            parsed_unit_serial: 0,
            acc: Acc::default(),
            has_annotation_unit: false,
            unit_src: Vec::new(),
            declared_version: None,
            last_error: None,
            error_line: 0,
            error_column: 0,
            error_byte: 0,
            error_offset: 0,
            queue: EventQueue::default(),
        }
    }

    /// `bvn_lex_supports_1_1`: the document declared spec 1.1 or later.
    pub(crate) fn supports_1_1(&self) -> bool {
        self.declared_version
            .is_some_and(|v| v >= SpecVersion::V1_1)
    }

    /// `bvn_lex_supports_1_2`: the document declared spec 1.2 or later.
    pub(crate) fn supports_1_2(&self) -> bool {
        self.declared_version
            .is_some_and(|v| v >= SpecVersion::V1_2)
    }

    /// The version handed to the unit parser.
    fn unit_version(&self) -> SpecVersion {
        self.declared_version.unwrap_or(SpecVersion::V1_0)
    }

    fn fail(&mut self, kind: ErrorKind) -> bool {
        self.last_error = Some(kind);
        false
    }

    fn set_unit_src(&mut self, text: &[u8]) {
        self.unit_src.clear();
        self.unit_src
            .extend_from_slice(text.get(..UNIT_SOURCE_MAX).unwrap_or(text));
    }

    /// Queues a token event: an assignment key or a value.
    fn emit_token(
        &mut self,
        ev: Ev,
        kind: TokenKind,
        text: &[u8],
        frac: &[u8],
        inline_unit: &[u8],
    ) {
        let text = self.queue.store(text);
        let event = match ev {
            Ev::AssignmentStart => QueuedEvent::AssignmentStart(text),
            _ => {
                let frac = self.queue.store(frac);
                // The effective type and unit travel with the value so that a
                // consumer (the document builder, the writer) does not have to
                // re-derive the reader's annotation scoping.
                let unit = if self.parsed_unit.is_no_unit() {
                    0..0
                } else {
                    let src = core::mem::take(&mut self.unit_src);
                    let range = self.queue.store(&src);
                    self.unit_src = src;
                    range
                };
                let inline_unit = self.queue.store(inline_unit);
                QueuedEvent::Data {
                    kind,
                    text,
                    frac,
                    value_type: self.value_type,
                    unit,
                    inline_unit,
                }
            }
        };
        self.queue.push(event);
    }

    /// `bvn_acc_reset`.
    pub(crate) fn acc_reset(&mut self) {
        self.acc = Acc::default();
    }

    /// `bvn_acc_digit`: folds one digit into the integer accumulator.
    fn acc_digit(&mut self, digit: u32, base: u32) {
        if self.acc.overflow || self.acc.has_dot || base < 2 {
            return;
        }
        let next = self
            .acc
            .value
            .checked_mul(u64::from(base))
            .and_then(|v| v.checked_add(u64::from(digit)));
        match next {
            Some(v) => self.acc.value = v,
            None => self.acc.overflow = true,
        }
    }

    /// `bvn_val_receive`: a finished token.
    pub(crate) fn receive(&mut self, raw: &RawToken<'_>) -> bool {
        let mut tt = raw.kind;
        if tt == TokenKind::Type {
            return self.receive_type(raw);
        }
        if tt == TokenKind::Symbol {
            let s = raw.data;
            let mut keyword_bool = None;
            match s {
                b"null" => tt = TokenKind::Null,
                b"true" | b"on" => keyword_bool = Some(true),
                b"false" | b"off" => keyword_bool = Some(false),
                b"nan" | b"inf" | b"ninf" => tt = TokenKind::Number,
                _ => {}
            }
            if let Some(value) = keyword_bool {
                if self.value_type.family == TypeFamily::Plain {
                    self.emit_default_type_annotation(TokenKind::Bool, s, false, None);
                } else if self.value_type.family != TypeFamily::Bool {
                    return self.fail(ErrorKind::TypeValueMismatch);
                }
                let text: &[u8] = if value { b"true" } else { b"false" };
                self.emit_token(Ev::Data, TokenKind::Bool, text, b"", b"");
                return true;
            }
        }
        if tt == TokenKind::Null {
            self.emit_token(raw.event, tt, b"", b"", b"");
            return true;
        }

        let inline_unit = if raw.inline_unit.is_empty() {
            None
        } else {
            Some(units::parse_unit(raw.inline_unit, self.unit_version()))
        };

        let is_iso_dt = is_number(tt) && num::str_is_iso_datetime(raw.data);
        if is_iso_dt {
            if !self.supports_1_1() {
                return self.fail(ErrorKind::IllegalValueType);
            }
            if inline_unit.is_some() {
                return self.fail(ErrorKind::UnitIllegal);
            }
        }

        if self.value_type.family == TypeFamily::Plain && (is_number(tt) || is_string(tt)) {
            let fold = match &inline_unit {
                Some(Ok(unit)) if is_number(tt) => Some((raw.inline_unit, unit.clone())),
                _ => None,
            };
            let is_currency = fold
                .as_ref()
                .is_some_and(|(_, unit)| unit_is_currency(unit));
            self.emit_default_type_annotation(tt, raw.data, is_currency, fold);
        }

        let seconds: String;
        let mut frac = 0..0;
        let mut value = raw.data;
        if is_iso_dt {
            if self.value_type.family != TypeFamily::Datetime {
                return self.fail(ErrorKind::TypeValueMismatch);
            }
            match num::iso_to_epoch_seconds(raw.data, self.value_type) {
                Ok(instant) => {
                    seconds = num::i64_to_decimal(instant.seconds);
                    frac = instant.frac;
                }
                Err(kind) => return self.fail(kind),
            }
            value = seconds.as_bytes();
        }

        if is_number(tt) || is_string(tt) {
            self.acc_reset();
            if !self.acc_parse_number(value, tt) {
                return false;
            }
        }
        if !self.validate_type_value_compat(tt, value) {
            return false;
        }

        if let Some(parsed) = inline_unit {
            if self.value_type.family == TypeFamily::Datetime {
                return self.fail(ErrorKind::UnitIllegal);
            }
            if annotation::unit_has_profile(raw.inline_unit) && !self.supports_1_2() {
                return self.fail(ErrorKind::UnitIllegal);
            }
            let unit = match parsed {
                Ok(unit) => unit,
                Err(kind) => return self.fail(kind),
            };
            if self.has_annotation_unit && self.parsed_unit != unit {
                return self.fail(ErrorKind::UnitMismatch);
            }
            let is_no_unit = unit == Unit::no_unit();
            if !self.has_annotation_unit {
                self.set_unit_src(raw.inline_unit);
                self.parsed_unit = unit;
            }
            if !is_no_unit {
                self.parsed_unit_serial = self.parsed_unit_serial.wrapping_add(1);
            }
        }

        let frac = raw.data.get(frac).unwrap_or_default();
        self.emit_token(raw.event, tt, value, frac, raw.inline_unit);
        true
    }

    /// The type annotation arm of `bvn_val_receive`.
    fn receive_type(&mut self, raw: &RawToken<'_>) -> bool {
        let parsed = annotation::parse_type_annotation(raw.type_data, self.unit_version());
        self.value_type = parsed.spec;
        if !parsed.type_ok {
            return self.fail(ErrorKind::IllegalValueType);
        }
        if self.value_type.family == TypeFamily::Datetime && !self.supports_1_1() {
            return self.fail(ErrorKind::IllegalValueType);
        }
        self.inferred_default_vtype = self.value_type;
        let unit_text = raw
            .type_data
            .get(parsed.unit_text.clone())
            .unwrap_or_default();
        match parsed.unit {
            Some(unit) => {
                self.set_unit_src(unit_text);
                self.parsed_unit = unit;
            }
            None => self.parsed_unit = Unit::no_unit(),
        }
        self.parsed_unit_serial = self.parsed_unit_serial.wrapping_add(1);
        if !unit_text.is_empty() {
            self.has_annotation_unit = true;
            if self.value_type.family != TypeFamily::Utf8 {
                if annotation::unit_has_profile(unit_text) && !self.supports_1_2() {
                    return self.fail(ErrorKind::UnitIllegal);
                }
                if let Some(kind) = parsed.unit_error {
                    return self.fail(kind);
                }
            }
        }

        let vt = self.value_type;
        let mut base = None;
        let mut q = None;
        let mut epoch = None;
        if vt.family == TypeFamily::Datetime {
            if vt.base != 0 {
                epoch = num::epoch_name(vt.base);
            }
        } else if vt.base != 0 {
            if vt.family == TypeFamily::FloatFix {
                q = Some(vt.effective_q());
            } else {
                base = Some(vt.effective_base());
            }
        }
        let text = self.queue.store(raw.type_data);
        let unit = if unit_text.is_empty() {
            None
        } else {
            Some(self.queue.store(unit_text))
        };
        self.queue.push(QueuedEvent::TypeAnnotation {
            text,
            family: vt.family,
            width: (vt.width != 0).then(|| vt.effective_width()),
            base,
            q,
            unit,
            epoch,
            synthesized: false,
        });
        true
    }

    /// `bvn_emit_default_type_annotation`: infers the type of an untyped
    /// value and announces it, unless the previous value already announced
    /// the same effective type.
    fn emit_default_type_annotation(
        &mut self,
        tt: TokenKind,
        s: &[u8],
        is_currency: bool,
        inline_unit: Option<(&[u8], Unit)>,
    ) {
        let previous = self.inferred_default_vtype;
        let same_as_previous =
            |t: TypeSpec| previous.family != TypeFamily::Plain && default_type_equiv(t, previous);
        let (default_type, name, emit_width, emit_base, emit_unit): (TypeSpec, &[u8], _, _, _) =
            if is_string(tt) {
                let utf8 = TypeSpec {
                    family: TypeFamily::Utf8,
                    width: 0,
                    base: 0,
                };
                (utf8, b"utf8", false, false, false)
            } else if is_number(tt) && num::str_is_iso_datetime(s) {
                (spec(TypeFamily::Datetime), b"datetime", true, false, false)
            } else if is_number(tt)
                && previous.family == TypeFamily::Datetime
                && num::str_is_plain_integer(s)
            {
                (previous, b"datetime", true, false, false)
            } else if is_number(tt) {
                let is_neg = s.first() == Some(&b'-');
                let first_digit = s.get(usize::from(is_neg)).is_some_and(u8::is_ascii_digit);
                if !is_currency && (self.acc.has_dot || self.acc.has_exp) {
                    (spec(TypeFamily::Float), b"float", true, true, true)
                } else if !is_currency && first_digit {
                    if is_neg {
                        (spec(TypeFamily::Sint), b"sint", true, true, true)
                    } else {
                        (spec(TypeFamily::Uint), b"uint", true, true, true)
                    }
                } else if is_currency {
                    (spec(TypeFamily::FloatDec), b"float_dec", true, false, true)
                } else if num::is_special_number(c_str(s)) {
                    (spec(TypeFamily::Float), b"float", true, true, true)
                } else if is_neg {
                    (spec(TypeFamily::Sint), b"sint", true, true, true)
                } else {
                    (spec(TypeFamily::Uint), b"uint", true, true, true)
                }
            } else if tt == TokenKind::Bool {
                let boolean = TypeSpec {
                    family: TypeFamily::Bool,
                    width: 0,
                    base: 0,
                };
                (boolean, b"bool", false, false, false)
            } else {
                return;
            };

        if same_as_previous(default_type) {
            self.value_type = default_type;
            self.parsed_unit = Unit::no_unit();
            return;
        }
        self.inferred_default_vtype = default_type;
        self.value_type = default_type;
        let inline_unit = inline_unit.filter(|(text, _)| emit_unit && !text.is_empty());
        let unit_text: &[u8] = match inline_unit {
            Some((text, unit)) => {
                self.parsed_unit = unit;
                self.set_unit_src(text);
                text
            }
            None => {
                self.parsed_unit = Unit::no_unit();
                NO_UNIT
            }
        };
        let vt = self.value_type;
        let text = self.queue.store(name);
        let unit = emit_unit.then(|| self.queue.store(unit_text));
        self.queue.push(QueuedEvent::TypeAnnotation {
            text,
            family: vt.family,
            width: emit_width.then(|| vt.effective_width()),
            base: emit_base.then(|| vt.effective_base()),
            q: None,
            unit,
            epoch: None,
            synthesized: true,
        });
    }

    /// `bvn_acc_parse_number`: folds the literal into the accumulator and
    /// notes a dot or exponent.
    fn acc_parse_number(&mut self, s: &[u8], tt: TokenKind) -> bool {
        let vt = self.value_type;
        let base = vt.effective_base();
        // In bases 64 and 85 '+' and '-' are digits (those bases are
        // unsigned-only), so a leading '-' is never a sign there.
        let is_neg = base != 64 && base != 85 && s.first() == Some(&b'-');
        if vt.family == TypeFamily::Uint && is_neg {
            return self.fail(ErrorKind::ValueOutOfRange);
        }
        let digits = s.get(usize::from(is_neg)..).unwrap_or_default();
        if is_string(tt) {
            if !matches!(vt.family, TypeFamily::Uint | TypeFamily::Sint) {
                return true;
            }
            for &b in digits {
                let dv = num::char_to_digit(b, base);
                if dv >= base {
                    return self.fail(ErrorKind::DigitNotInBase);
                }
                self.acc_digit(dv, base);
            }
            return true;
        }
        if !is_number(tt) || num::is_special_number(c_str(s)) {
            return true;
        }
        for &b in digits {
            if b == b'.' {
                self.acc.has_dot = true;
                continue;
            }
            if (b == b'e' || b == b'E') && base <= 14 {
                self.acc.has_exp = true;
                self.acc.exp_state = 1;
                continue;
            }
            if self.acc.exp_state > 0 {
                // Inside the exponent: never part of the mantissa.
                self.acc.exp_state = 2;
                continue;
            }
            let dv = num::char_to_digit(b, base);
            if dv >= base {
                return self.fail(ErrorKind::DigitNotInBase);
            }
            self.acc_digit(dv, base);
        }
        true
    }

    /// `bvn_check_acc_range`: does the integer fit its width and signedness?
    fn check_acc_range(&mut self, s: &[u8], tt: TokenKind) -> bool {
        let vt = self.value_type;
        if !matches!(
            vt.family,
            TypeFamily::Uint | TypeFamily::Sint | TypeFamily::Datetime
        ) {
            return true;
        }
        let base = vt.effective_base();
        let is_neg = base != 64 && base != 85 && s.first() == Some(&b'-');
        // The special-number keywords are range-exempt (spec 6.4); a quoted
        // "inf" under a base-36 uint is digits and is not.
        if is_number(tt)
            && matches!(s.first(), Some(b'n' | b'i'))
            && num::is_special_number(c_str(s))
        {
            return true;
        }
        let width = vt.effective_width();
        if width > 64 {
            let in_range = if vt.family == TypeFamily::Uint {
                num::validate_uint_range(c_str(s), width, base)
            } else {
                num::validate_sint_range(c_str(s), width, base)
            };
            return in_range || self.fail(ErrorKind::ValueOutOfRange);
        }
        if self.acc.overflow {
            return self.fail(ErrorKind::ValueOutOfRange);
        }
        let max = if vt.family == TypeFamily::Uint {
            u64::MAX >> (64 - width)
        } else if is_neg {
            1u64 << (width - 1)
        } else {
            (1u64 << (width - 1)) - 1
        };
        self.acc.value <= max || self.fail(ErrorKind::ValueOutOfRange)
    }

    /// `bvn_validate_type_value_compat`: does the token suit the declared
    /// family, and does the value fit it?
    fn validate_type_value_compat(&mut self, tt: TokenKind, s: &[u8]) -> bool {
        let vt = self.value_type;
        if vt.family == TypeFamily::Plain || tt == TokenKind::Null {
            return true;
        }
        match vt.family {
            TypeFamily::Utf8 => {
                return is_string(tt) || self.fail(ErrorKind::TypeValueMismatch);
            }
            TypeFamily::Bool => {
                return tt == TokenKind::Bool || self.fail(ErrorKind::TypeValueMismatch);
            }
            _ => {}
        }
        if !vt.is_numeric() && vt.family != TypeFamily::Datetime {
            return true;
        }
        if !is_number(tt) && !is_string(tt) {
            return self.fail(ErrorKind::TypeValueMismatch);
        }
        if is_string(tt) {
            // A datetime is a bare decimal integer, never a string carrier.
            if vt.family == TypeFamily::Datetime {
                return self.fail(ErrorKind::TypeValueMismatch);
            }
            if !num::validate_number_in_base(c_str(s), vt.effective_base()) {
                return self.fail(ErrorKind::DigitNotInBase);
            }
        }
        let is_float = matches!(
            vt.family,
            TypeFamily::Float | TypeFamily::FloatFix | TypeFamily::FloatDec
        );
        if (self.acc.has_exp || self.acc.has_dot) && !is_float {
            return self.fail(ErrorKind::TypeValueMismatch);
        }
        if vt.family == TypeFamily::FloatFix && !s.is_empty() {
            let special =
                matches!(s.first(), Some(b'n' | b'i')) && num::is_special_number(c_str(s));
            if !special
                && !num::float_str_fits_fix(c_str(s), 10, vt.effective_width(), vt.effective_q())
            {
                return self.fail(ErrorKind::ValueOutOfRange);
            }
        }
        self.check_acc_range(s, tt)
    }

    /// `bvn_val_receive_event`: a structural event.
    pub(crate) fn receive_event(&mut self, ev: Ev) -> bool {
        let event = match ev {
            Ev::StreamStart => QueuedEvent::StreamStart,
            Ev::OctetStreamStart => QueuedEvent::OctetStreamStart,
            Ev::OctetStreamEnd => QueuedEvent::OctetStreamEnd,
            Ev::StructStart => QueuedEvent::StructStart,
            Ev::StructEnd => QueuedEvent::StructEnd,
            Ev::ArrayRowEnd => QueuedEvent::ArrayRowEnd,
            Ev::ArrayDimStart => QueuedEvent::ArrayDimStart,
            Ev::StreamEnd => QueuedEvent::StreamEnd,
            // C would report these with empty data; the lexer never does.
            Ev::AssignmentStart | Ev::Data => {
                self.emit_token(ev, TokenKind::Unknown, b"", b"", b"");
                return true;
            }
        };
        self.queue.push(event);
        true
    }

    /// `bvn_val_receive_octet_chunk`: one complete octet-stream chunk.
    pub(crate) fn receive_octet_chunk(&mut self, data: &[u8]) -> bool {
        let data = self.queue.store(data);
        self.queue.push(QueuedEvent::OctetStreamChunk(data));
        true
    }

    /// Resets the per-value state so nothing leaks into the next value.
    fn reset_value_state(&mut self) {
        self.value_type = TypeSpec::PLAIN;
        self.parsed_unit = Unit::no_unit();
        self.inferred_default_vtype = TypeSpec::PLAIN;
        self.has_annotation_unit = false;
        self.acc_reset();
    }

    /// `bvn_val_on_value_intro`.
    pub(crate) fn on_value_intro(&mut self) -> bool {
        self.reset_value_state();
        true
    }

    /// `bvn_val_on_value_outro`.
    pub(crate) fn on_value_outro(&mut self) -> bool {
        self.reset_value_state();
        true
    }

    /// `bvn_val_on_array_intro`.
    pub(crate) fn on_array_intro(&mut self) -> bool {
        self.acc_reset();
        self.has_annotation_unit = false;
        self.queue.push(QueuedEvent::ArrayRowStart);
        true
    }

    /// `bvn_val_on_array_outro`: the first row to close fixes the row width
    /// (`array_row_size` is `u64::MAX` while unset); siblings must match.
    pub(crate) fn on_array_outro(&mut self, curr_row_size: u64, array_row_size: &mut u64) -> bool {
        if *array_row_size == u64::MAX {
            *array_row_size = curr_row_size;
        } else if *array_row_size != curr_row_size {
            return self.fail(ErrorKind::ArrayRowSizeMismatch);
        }
        self.queue.push(QueuedEvent::ArrayRowEnd);
        true
    }

    /// `bvn_val_on_new_array_value`.
    pub(crate) fn on_new_array_value(&mut self, curr_row_size: u64, array_row_size: u64) -> bool {
        if array_row_size != u64::MAX && curr_row_size >= array_row_size {
            return self.fail(ErrorKind::ArrayRowSizeMismatch);
        }
        self.acc_reset();
        self.has_annotation_unit = false;
        true
    }

    /// `bvn_notify_error` in recovery mode: reports the error held in
    /// `last_error` and the `error_*` fields as an [`crate::Event::Recovered`].
    pub(crate) fn notify_recovered(&mut self) {
        let kind = self.last_error.unwrap_or(ErrorKind::Recovered);
        let error = Error::new(kind, self.error_line, self.error_column, self.error_offset);
        self.queue.push(QueuedEvent::Recovered(error));
    }
}
