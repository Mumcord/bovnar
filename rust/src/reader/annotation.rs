//! Type annotation parser (`bvn_parse_type_annotation` in
//! `src/utils/bovnar_utils.c`).

use core::ops::Range;

use crate::num;
use crate::types::{TypeFamily, TypeSpec};
use crate::units::{self, Unit};
use crate::{ErrorKind, SpecVersion};

/// Widest integer or datetime (`BVN_MAX_INT_WIDTH`).
const MAX_INT_WIDTH: u32 = 32768;
/// Widest binary float (`BVN_FLOAT_MAX_PREC`).
const FLOAT_MAX_PREC: u32 = 32768;
/// Deepest `{}` / `[]` grouping inside a unit (`BVN_UNIT_GROUP_MAX_DEPTH`).
const UNIT_GROUP_MAX_DEPTH: u32 = 16;

/// Outcome of parsing an annotation body such as `uint:8,_16,m/s`.
#[derive(Debug, Clone)]
pub(crate) struct ParsedType {
    pub spec: TypeSpec,
    /// False when the annotation is malformed (`error_illegal_value_type`).
    pub type_ok: bool,
    /// The unit parameter was refused: too long, or the unit parser's error.
    pub unit_error: Option<ErrorKind>,
    /// Where the unit parameter is in the annotation; empty without one.
    /// `no_unit` is a unit parameter too.
    pub unit_text: Range<usize>,
    /// The parsed unit; `None` when there is no unit or it is `no_unit`.
    pub unit: Option<Unit>,
}

impl ParsedType {
    fn new() -> Self {
        Self {
            spec: TypeSpec::PLAIN,
            type_ok: true,
            unit_error: None,
            unit_text: 0..0,
            unit: None,
        }
    }

    fn illegal(mut self) -> Self {
        self.type_ok = false;
        self
    }
}

const FAMILIES: [(&[u8], TypeFamily); 8] = [
    (b"float_fix", TypeFamily::FloatFix),
    (b"float_dec", TypeFamily::FloatDec),
    (b"float", TypeFamily::Float),
    (b"uint", TypeFamily::Uint),
    (b"utf8", TypeFamily::Utf8),
    (b"bool", TypeFamily::Bool),
    (b"sint", TypeFamily::Sint),
    (b"datetime", TypeFamily::Datetime),
];

fn at(s: &[u8], pos: usize) -> u8 {
    s.get(pos).copied().unwrap_or(0)
}

/// Parses an annotation body. `version` is handed to the unit parser.
pub(crate) fn parse_type_annotation(s: &[u8], version: SpecVersion) -> ParsedType {
    let mut r = ParsedType::new();
    if s.is_empty() {
        return r.illegal();
    }
    let Some(&(name, family)) = FAMILIES.iter().find(|(name, _)| s.starts_with(name)) else {
        return r.illegal();
    };
    r.spec.family = family;
    let len = s.len();
    let mut pos = name.len();
    if pos < len {
        if at(s, pos) != b':' {
            return r.illegal();
        }
        pos += 1;
        while pos < len && at(s, pos) == b' ' {
            pos += 1;
        }
        if pos >= len {
            return r.illegal();
        }
    }

    let (mut have_width, mut have_base, mut have_q, mut have_unit) = (false, false, false, false);
    let mut need_comma = false;
    while pos < len {
        if need_comma {
            if at(s, pos) != b',' {
                return r.illegal();
            }
            pos += 1;
            while pos < len && at(s, pos) == b' ' {
                pos += 1;
            }
        }
        need_comma = true;
        if pos >= len || at(s, pos) == b',' {
            return r.illegal();
        }
        let c = at(s, pos);
        if c.is_ascii_digit() {
            if have_width {
                return r.illegal();
            }
            have_width = true;
            let mut w: u64 = 0;
            while pos < len && at(s, pos).is_ascii_digit() {
                let d = u64::from(at(s, pos) - b'0');
                if w > (u64::from(u32::MAX) - d) / 10 {
                    return r.illegal();
                }
                w = w * 10 + d;
                pos += 1;
            }
            if pos < len && at(s, pos) != b',' {
                return r.illegal();
            }
            r.spec.width = u32::try_from(w).unwrap_or(u32::MAX);
        } else if c == b'_' {
            if have_base || family == TypeFamily::Datetime {
                return r.illegal();
            }
            have_base = true;
            pos += 1;
            if pos >= len || !at(s, pos).is_ascii_digit() {
                return r.illegal();
            }
            let mut b: u32 = 0;
            while pos < len && at(s, pos).is_ascii_digit() {
                b = b * 10 + u32::from(at(s, pos) - b'0');
                if b > 10000 {
                    return r.illegal();
                }
                pos += 1;
            }
            if pos < len && at(s, pos) != b',' {
                return r.illegal();
            }
            if matches!(family, TypeFamily::FloatFix | TypeFamily::FloatDec) {
                return r.illegal();
            }
            if !((2..=62).contains(&b) || b == 64 || b == 85) {
                return r.illegal();
            }
            r.spec.base = b;
        } else if c == b'q' && pos + 1 < len && at(s, pos + 1).is_ascii_digit() {
            if family != TypeFamily::FloatFix || have_q {
                return r.illegal();
            }
            have_q = true;
            pos += 1;
            let mut q: u32 = 0;
            while pos < len && at(s, pos).is_ascii_digit() {
                q = q * 10 + u32::from(at(s, pos) - b'0');
                if q > 256 {
                    return r.illegal();
                }
                pos += 1;
            }
            if pos < len && at(s, pos) != b',' {
                return r.illegal();
            }
            r.spec.base = q;
        } else if family == TypeFamily::Datetime {
            if have_unit {
                return r.illegal();
            }
            have_unit = true;
            let start = pos;
            while pos < len && at(s, pos) != b',' {
                pos += 1;
            }
            match num::epoch_index_of(s.get(start..pos).unwrap_or_default()) {
                Some(index) => r.spec.base = index,
                None => return r.illegal(),
            }
        } else {
            if have_unit {
                return r.illegal();
            }
            have_unit = true;
            let start = pos;
            let (mut braces, mut brackets) = (0u32, 0u32);
            while pos < len {
                match at(s, pos) {
                    b',' if braces == 0 && brackets == 0 => break,
                    b'{' => braces += 1,
                    b'}' => braces = braces.saturating_sub(1),
                    b'[' => brackets += 1,
                    b']' => brackets = brackets.saturating_sub(1),
                    _ => {}
                }
                if braces > UNIT_GROUP_MAX_DEPTH || brackets > UNIT_GROUP_MAX_DEPTH {
                    // C flags the unit but records no unit text, so the
                    // validator never looks at the flag: the annotation
                    // stands without a unit (see UPSTREAM_NOTES.md).
                    r.unit_error = Some(ErrorKind::UnitIllegal);
                    return r;
                }
                pos += 1;
            }
            if pos - start > usize::from(u8::MAX) {
                r.unit_error = Some(ErrorKind::UnitTooLong);
                return r;
            }
            r.unit_text = start..pos;
            let text = s.get(start..pos).unwrap_or_default();
            if text != b"no_unit" {
                match units::parse_unit(text, version) {
                    Ok(unit) => r.unit = Some(unit),
                    Err(kind) => r.unit_error = Some(kind),
                }
            }
        }
    }

    let spec = r.spec;
    let bad = match spec.family {
        TypeFamily::Sint | TypeFamily::Uint => {
            spec.width > MAX_INT_WIDTH
                || (spec.family == TypeFamily::Sint && (spec.base == 64 || spec.base == 85))
        }
        TypeFamily::Datetime => spec.width > MAX_INT_WIDTH,
        TypeFamily::Float => {
            (spec.base != 0 && spec.base != 10 && spec.base != 16)
                || (spec.width != 0
                    && spec.width != 16
                    && (spec.width % 32 != 0 || spec.width > FLOAT_MAX_PREC))
        }
        TypeFamily::FloatFix | TypeFamily::FloatDec => {
            !matches!(spec.width, 0 | 16 | 32 | 64 | 128 | 256)
                || (spec.family == TypeFamily::FloatFix && spec.base >= spec.effective_width())
        }
        TypeFamily::Bool | TypeFamily::Utf8 => {
            have_width || have_base || spec.width != 0 || spec.base != 0 || !r.unit_text.is_empty()
        }
        TypeFamily::Plain => false,
    };
    if bad {
        return r.illegal();
    }
    r
}

/// Whether a unit is spelled in a profile namespace such as `ucum:m`
/// (`bvni_unit_has_profile`).
pub(crate) fn unit_has_profile(s: &[u8]) -> bool {
    for (i, &c) in s.iter().enumerate() {
        if c == b':' {
            return i > 0;
        }
        let ok = c.is_ascii_lowercase() || c.is_ascii_digit() || (c == b'-' && i > 0);
        if !ok {
            return false;
        }
    }
    false
}
