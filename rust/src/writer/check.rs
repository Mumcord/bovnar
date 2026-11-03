//! The C writer's own checks (`bvn_writer_validate_event` and the
//! `bvn_validate_*_for_writer` helpers), ported so the writer reports the
//! same error as `bvnr_write_event` for the same event.
//!
//! The C helpers take NUL-terminated copies of the event's bytes, so a value
//! with an embedded NUL is judged by the part before it; [`c_str`] keeps
//! that. Whatever these checks let through is then still read back by
//! [`super::probe`], which only refuses what the C writer would write
//! unreadably.

use crate::reader::annotation::unit_has_profile;
use crate::reader::lexer::validate_utf8;
use crate::types::{TypeFamily, TypeSpec};
use alloc::boxed::Box;

use crate::units::{parse_unit, Unit};
use crate::{num, ErrorKind, SpecVersion};

/// `BVN_MAX_INT_WIDTH`.
const MAX_INT_WIDTH: u32 = 32_768;
/// `BVN_FLOAT_MAX_PREC`.
const FLOAT_MAX_PREC: u32 = 32_768;
/// `BVNR_UNIT_SOURCE_MAX`.
pub(super) const UNIT_SOURCE_MAX: usize = 255;
/// Longest annotation body (`BVN_TYPE_BUF_CAP - 1`).
pub(super) const MAX_ANNOTATION: usize = 255;

/// The bytes a C helper sees: up to the first NUL.
fn c_str(text: &[u8]) -> &[u8] {
    text.iter()
        .position(|&b| b == 0)
        .map_or(text, |end| text.get(..end).unwrap_or_default())
}

pub(super) fn is_numeric(family: TypeFamily) -> bool {
    matches!(
        family,
        TypeFamily::Uint
            | TypeFamily::Sint
            | TypeFamily::Float
            | TypeFamily::FloatFix
            | TypeFamily::FloatDec
    )
}

/// `bvn_effective_width`.
pub(super) fn effective_width(width: u32) -> u32 {
    if width == 0 {
        64
    } else {
        width
    }
}

/// `bvn_effective_base`: float_fix, float_dec and datetime are decimal (their
/// `base` field is Q or the epoch).
pub(super) fn effective_base(vt: TypeSpec) -> u32 {
    match vt.family {
        TypeFamily::FloatFix | TypeFamily::FloatDec | TypeFamily::Datetime => 10,
        _ if vt.base == 0 => 10,
        _ => vt.base,
    }
}

/// `bvn_effective_q`.
pub(super) fn effective_q(vt: TypeSpec) -> u32 {
    if vt.family == TypeFamily::FloatFix {
        vt.base
    } else {
        0
    }
}

/// The special numbers as the writer recognises them: the C copies the
/// token into an 8-byte buffer first, so only short tokens qualify.
pub(super) fn is_special_token(text: &[u8]) -> bool {
    !text.is_empty() && text.len() < 8 && num::is_special_number(c_str(text))
}

/// `bvn_validate_identifier`: a letter, `_` or UTF-8 lead byte, then anything but
/// whitespace, controls and the format's punctuation, all valid UTF-8.
fn identifier(id: &[u8]) -> bool {
    let Some((&first, rest)) = id.split_first() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == b'_' || (0xc3..=0xf4).contains(&first)) {
        return false;
    }
    rest.iter().all(|&c| {
        c > 0x20 && c != 0x7f && c != 0xc2 && !b"\"#,./;<=>[]{}!$%&'()*:?@\\^`|~".contains(&c)
    }) && validate_utf8(id)
}

/// `bvn_validate_reference`: `.name` segments, each optionally indexed
/// `[digits]`, all valid UTF-8.
fn reference(link: &[u8]) -> bool {
    let at = |i: usize| link.get(i).copied().unwrap_or(0);
    if at(0) != b'.' {
        return false;
    }
    let mut p = 0usize;
    loop {
        p += 1;
        let c = at(p);
        if !(c.is_ascii_alphabetic() || c == b'_' || (0xc3..=0xf4).contains(&c)) {
            return false;
        }
        p += 1;
        while at(p) != 0 && at(p) != b'.' {
            let c = at(p);
            if c == b'[' {
                p += 1;
                if !at(p).is_ascii_digit() {
                    return false;
                }
                while at(p).is_ascii_digit() {
                    p += 1;
                }
                if at(p) != b']' {
                    return false;
                }
                p += 1;
                if at(p) != 0 && at(p) != b'.' && at(p) != b'[' {
                    return false;
                }
                continue;
            }
            if c <= 0x20 || c == 0x7f || c == 0xc2 || b"\"#,/;<=>]{}!$%&'()*:?@\\^`|~".contains(&c)
            {
                return false;
            }
            p += 1;
        }
        if at(p) != b'.' {
            break;
        }
    }
    validate_utf8(link)
}

/// `bvn_validate_number`: an optional `-`, decimal digits with at most one
/// `.`, an optional exponent.
fn decimal_number(s: &[u8]) -> bool {
    let s = s.strip_prefix(b"-").unwrap_or(s);
    if s.is_empty() {
        return false;
    }
    let (mut has_dot, mut has_exp, mut mant, mut exp_digit) = (false, false, false, false);
    let mut i = 0;
    while let Some(&c) = s.get(i) {
        match c {
            b'0'..=b'9' if has_exp => exp_digit = true,
            b'0'..=b'9' => mant = true,
            b'.' => {
                if has_dot || has_exp {
                    return false;
                }
                has_dot = true;
            }
            b'e' | b'E' => {
                if has_exp || !mant {
                    return false;
                }
                has_exp = true;
                if matches!(s.get(i + 1), Some(b'+' | b'-')) {
                    i += 1;
                }
            }
            _ => return false,
        }
        i += 1;
    }
    mant && (!has_exp || exp_digit)
}

/// `bvn_validate_id_for_writer`.
pub(super) fn key(key: &[u8]) -> Result<(), ErrorKind> {
    if key.is_empty() {
        return Err(ErrorKind::EmptyIdentifier);
    }
    if key.len() > 255 {
        return Err(ErrorKind::IdentifierTooLong);
    }
    if !identifier(c_str(key)) {
        return Err(ErrorKind::InvalidArgument);
    }
    Ok(())
}

/// `bvn_validate_type_spec_for_writer`.
pub(super) fn type_spec(vt: TypeSpec, version: Option<SpecVersion>) -> Result<(), ErrorKind> {
    let illegal = Err(ErrorKind::IllegalValueType);
    let (w, b) = (vt.width, vt.base);
    match vt.family {
        TypeFamily::Datetime if version.is_none() => return Err(ErrorKind::UnsupportedSpecVersion),
        TypeFamily::Uint | TypeFamily::Sint if w > MAX_INT_WIDTH => return illegal,
        TypeFamily::Sint if b == 64 || b == 85 => return illegal,
        TypeFamily::Datetime if w > MAX_INT_WIDTH => return illegal,
        TypeFamily::Float if !matches!(b, 0 | 10 | 16) => return illegal,
        TypeFamily::Float if w != 0 && w != 16 && (w % 32 != 0 || w > FLOAT_MAX_PREC) => {
            return illegal
        }
        TypeFamily::FloatFix | TypeFamily::FloatDec
            if !matches!(w, 0 | 16 | 32 | 64 | 128 | 256) =>
        {
            return illegal
        }
        TypeFamily::FloatFix if b >= effective_width(w) => return illegal,
        TypeFamily::FloatDec if b != 0 => return illegal,
        TypeFamily::Bool | TypeFamily::Utf8 if w != 0 || b != 0 => return illegal,
        _ => {}
    }
    if matches!(
        vt.family,
        TypeFamily::Uint | TypeFamily::Sint | TypeFamily::Float
    ) && b != 0
        && !((2..=62).contains(&b) || b == 64 || b == 85)
    {
        return illegal;
    }
    Ok(())
}

/// `bvn_check_type_value_compat`: which tokens a family takes at all.
pub(super) fn type_value_compat(kind: crate::TokenKind, vt: TypeSpec) -> Result<(), ErrorKind> {
    use crate::TokenKind as K;
    let ok = match vt.family {
        _ if kind == K::Null => true,
        TypeFamily::Plain | TypeFamily::Datetime => true,
        TypeFamily::Utf8 => matches!(kind, K::String | K::ArrayString),
        TypeFamily::Bool => kind == K::Bool,
        _ => matches!(
            kind,
            K::Number | K::ArrayNumber | K::String | K::ArrayString
        ),
    };
    if ok {
        Ok(())
    } else {
        Err(ErrorKind::TypeValueMismatch)
    }
}

/// `bvn_w_bare_number_type`: what the reader types an unannotated number as.
pub(super) fn bare_number_type(text: &[u8]) -> TypeSpec {
    let family = if is_special_token(text) || text.iter().any(|c| matches!(c, b'.' | b'e' | b'E')) {
        TypeFamily::Float
    } else if text.first() == Some(&b'-') {
        TypeFamily::Sint
    } else {
        TypeFamily::Uint
    };
    TypeSpec {
        family,
        width: 64,
        base: 0,
    }
}

/// The checks on a number token: the bare-token rule above base 10, then
/// `bvn_validate_number_for_writer`.
pub(super) fn number(text: &[u8], vt: TypeSpec) -> Result<(), ErrorKind> {
    if is_numeric(vt.family) && effective_base(vt) > 10 && !is_special_token(text) {
        // Letters as digits need a string literal; so does anything that
        // would not lex as one number token.
        if !text
            .iter()
            .all(|c| c.is_ascii_digit() || b".+-eE".contains(c))
            || !decimal_number(c_str(text))
        {
            return Err(ErrorKind::BaseRequiresStringLiteral);
        }
    }
    if text.is_empty() {
        return Ok(());
    }
    let s = c_str(text);
    if num::is_special_number(s) {
        return if is_numeric(vt.family) || vt.family == TypeFamily::Plain {
            Ok(())
        } else {
            Err(ErrorKind::TypeValueMismatch)
        };
    }
    unsigned_sign(s, vt)?;
    let base = effective_base(vt);
    if is_numeric(vt.family) && base != 10 {
        if !num::validate_number_in_base(s, base) {
            return Err(ErrorKind::DigitNotInBase);
        }
    } else {
        if !decimal_number(s) {
            return Err(ErrorKind::TypeValueMismatch);
        }
        let fractional = text.iter().any(|c| matches!(c, b'.' | b'e' | b'E'));
        if fractional
            && !matches!(
                vt.family,
                TypeFamily::Float | TypeFamily::FloatFix | TypeFamily::FloatDec | TypeFamily::Plain
            )
        {
            return Err(ErrorKind::TypeValueMismatch);
        }
    }
    range(s, vt)
}

/// The sign rule of an unsigned type (bases 64 and 85 use `-` as a digit).
fn unsigned_sign(s: &[u8], vt: TypeSpec) -> Result<(), ErrorKind> {
    let base = effective_base(vt);
    if vt.family == TypeFamily::Uint && s.first() == Some(&b'-') && base != 64 && base != 85 {
        Err(ErrorKind::ValueOutOfRange)
    } else {
        Ok(())
    }
}

/// The range checks shared by number tokens and numeric strings.
fn range(s: &[u8], vt: TypeSpec) -> Result<(), ErrorKind> {
    let (base, width) = (effective_base(vt), effective_width(vt.width));
    let fits = match vt.family {
        TypeFamily::Uint => num::validate_uint_range(s, width, base),
        TypeFamily::Sint | TypeFamily::Datetime => num::validate_sint_range(s, width, base),
        TypeFamily::FloatFix => num::float_str_fits_fix(s, 10, width, effective_q(vt)),
        _ => true,
    };
    if fits {
        Ok(())
    } else {
        Err(ErrorKind::ValueOutOfRange)
    }
}

/// `bvn_validate_string_as_number`: a numeric value given as a string.
pub(super) fn string_as_number(text: &[u8], vt: TypeSpec) -> Result<(), ErrorKind> {
    if text.is_empty() {
        return Ok(());
    }
    let s = c_str(text);
    let base = effective_base(vt);
    unsigned_sign(s, vt)?;
    match vt.family {
        TypeFamily::Uint | TypeFamily::Sint => {
            if !num::validate_digits_for_base(s, base) {
                return Err(ErrorKind::DigitNotInBase);
            }
            if num::is_special_number(s) {
                // `inf` and friends are digit strings in a high enough
                // base; there they must fit as integers.
                let width = effective_width(vt.width);
                let bits = if vt.family == TypeFamily::Uint {
                    width
                } else {
                    width.saturating_sub(1)
                };
                let magnitude = s.strip_prefix(b"-").unwrap_or(s);
                if bits == 0 || !num::validate_uint_range(magnitude, bits, base) {
                    return Err(ErrorKind::ValueOutOfRange);
                }
            }
        }
        TypeFamily::Float | TypeFamily::FloatFix | TypeFamily::FloatDec
            if !num::validate_number_in_base(s, base) =>
        {
            return Err(ErrorKind::DigitNotInBase);
        }
        _ => {}
    }
    range(s, vt)
}

/// `bvn_validate_string_content`.
pub(super) fn string_content(text: &[u8]) -> Result<(), ErrorKind> {
    if text
        .iter()
        .any(|&c| c <= 0x08 || (0x0e..=0x1f).contains(&c) || c == 0x7f)
    {
        return Err(ErrorKind::UnexpectedInputByte);
    }
    if validate_utf8(text) {
        Ok(())
    } else {
        Err(ErrorKind::InvalidUtf8Byte)
    }
}

/// `bvn_validate_symbol_for_writer`.
pub(super) fn symbol(text: &[u8]) -> Result<(), ErrorKind> {
    if text.is_empty() {
        return Err(ErrorKind::EmptyIdentifier);
    }
    let s = c_str(text);
    let keyword = matches!(
        s,
        b"null" | b"true" | b"on" | b"false" | b"off" | b"nan" | b"inf" | b"ninf"
    );
    if s.is_empty() || !identifier(s) || keyword {
        return Err(ErrorKind::TypeValueMismatch);
    }
    Ok(())
}

/// `bvn_validate_reference_for_writer`.
pub(super) fn reference_value(text: &[u8], version: Option<SpecVersion>) -> Result<(), ErrorKind> {
    if text.is_empty() {
        return Err(ErrorKind::InvalidArgument);
    }
    let s = c_str(text);
    if s.is_empty() || !reference(s) {
        return Err(ErrorKind::TypeValueMismatch);
    }
    if version.is_none() && s.contains(&b'[') {
        return Err(ErrorKind::UnsupportedSpecVersion);
    }
    Ok(())
}

/// A unit as an event carries it: none (or one that means no unit), a
/// parsed one, or a spelling that does not parse. The C reference can only
/// be handed a parsed unit; a spelling that does not parse stands for a
/// unit no other spelling names, so it is checked and refused where the
/// spelling is checked.
#[derive(Clone, PartialEq)]
pub(super) enum EvUnit {
    None,
    Real(Box<Unit>),
    Unparsed,
}

impl EvUnit {
    /// The unit an inline suffix or an annotation's unit parameter names.
    pub(super) fn of(text: &[u8]) -> Self {
        if text.is_empty() {
            return Self::None;
        }
        match parse_unit(text, SpecVersion::V1_2) {
            Ok(u) if u.is_no_unit() => Self::None,
            Ok(u) => Self::Real(Box::new(u)),
            Err(_) => Self::Unparsed,
        }
    }

    pub(super) fn is_real(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub(super) fn is_opaque(&self) -> bool {
        matches!(self, Self::Real(u) if u.is_profile_only())
    }
}

/// Whether the document declares spec 1.2, which the profile notation
/// needs.
pub(super) fn declares_1_2(version: Option<SpecVersion>) -> bool {
    version.is_some_and(|v| v >= SpecVersion::V1_2)
}

/// `bvn_writer_check_unit_text`: a unit spelling about to be written into an
/// annotation (`used` bytes of whose body are out already) or as an inline
/// suffix, which must name `unit`. The read-back the C reference does last
/// is [`super::probe::unit`].
pub(super) fn unit_text(
    text: &[u8],
    unit: &EvUnit,
    inline: bool,
    used: usize,
    version: Option<SpecVersion>,
) -> Result<(), ErrorKind> {
    if text.len() > UNIT_SOURCE_MAX || (!inline && used + 1 + text.len() > MAX_ANNOTATION) {
        return Err(ErrorKind::UnitTooLong);
    }
    if text.contains(&0) {
        return Err(ErrorKind::UnitIllegal);
    }
    let parsed = parse_unit(text, SpecVersion::V1_2)?;
    let profile_gate = || {
        if unit_has_profile(text) && !declares_1_2(version) {
            Err(ErrorKind::UnsupportedSpecVersion)
        } else {
            Ok(())
        }
    };
    if parsed.is_no_unit() {
        if unit.is_real() {
            return Err(ErrorKind::UnitMismatch);
        }
        if !inline && text == b"no_unit" {
            return Ok(());
        }
        if inline {
            return Err(ErrorKind::UnitIllegal);
        }
        return profile_gate();
    }
    if *unit != EvUnit::Real(Box::new(parsed)) {
        return Err(ErrorKind::UnitMismatch);
    }
    profile_gate()
}
