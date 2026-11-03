//! The native unit grammar: `bvn_parse_unit_expr` and
//! `bvn_parse_single_unit_component` of the C reference.
//!
//! Every refusal is `error_unit_illegal` in C, so the parser only reports
//! whether it accepted; the order in which it finds a fault does not matter.
//! Parenthesised groups are walked with an explicit, bounded stack instead of
//! the C recursion.

use super::data::{self, PrefixPolicy};
use super::gen::units::ALIAS_MAX_LEN;
use super::{BaseUnit, Component, Prefix, PrefixSystem, Unit, EXPONENT_MAX, EXPONENT_MIN};

/// Deepest group nesting the C parser accepts (`BVN_UNIT_GROUP_MAX_DEPTH`):
/// the top level is depth 0 and a group at depth 17 is refused.
const MAX_GROUP_DEPTH: usize = 16;

/// The C spelling of the empty unit.
const NO_UNIT: &[u8] = b"no_unit";

/// One level of the expression: the top level or a parenthesised group.
#[derive(Clone, Copy)]
struct Frame {
    /// End of this level's text (exclusive); for a group, its `)`.
    end: usize,
    /// A `/` has been seen at this level; later factors are negated.
    in_denominator: bool,
    /// The enclosing levels negate this level's factors (an odd number of
    /// them had seen `/` when the group started).
    negated: bool,
    /// Components the unit held when this level started; a level that adds
    /// none is refused.
    first_component: usize,
}

impl Frame {
    const TOP: Self = Self {
        end: 0,
        in_denominator: false,
        negated: false,
        first_component: 0,
    };

    fn negates(&self) -> bool {
        self.negated != self.in_denominator
    }
}

/// Parses a native unit expression; `None` is `error_unit_illegal`.
pub(crate) fn parse_expression(s: &[u8]) -> Option<Unit> {
    let mut unit = Unit::no_unit();
    if s.is_empty() {
        return None;
    }
    if s == NO_UNIT {
        return Some(unit);
    }
    let mut frames = [Frame::TOP; MAX_GROUP_DEPTH + 1];
    let mut depth = 0usize;
    frames[0].end = s.len();
    let mut i = 0usize;
    loop {
        let frame = *frames.get(depth)?;
        if i >= frame.end {
            // This level is complete.
            if usize::from(unit.len) == frame.first_component {
                return None;
            }
            if depth == 0 {
                return Some(unit);
            }
            depth -= 1;
            i = frame.end + 1; // past the group's ')'
        } else if s.get(i) == Some(&b'(') {
            let close = matching_paren(s, i, frame.end)?;
            let inner = s.get(i + 1..close)?;
            if inner.is_empty() || depth + 1 > MAX_GROUP_DEPTH {
                return None;
            }
            if inner != NO_UNIT {
                // A group that spells `no_unit` contributes nothing and is
                // accepted as such, while a level that holds only such groups
                // is refused below for adding no component, as in C.
                depth += 1;
                *frames.get_mut(depth)? = Frame {
                    end: close,
                    in_denominator: false,
                    negated: frame.negates(),
                    first_component: usize::from(unit.len),
                };
                i += 1;
                continue;
            }
            i = close + 1;
        } else {
            let start = i;
            while i < frame.end && !is_separator_or_group(s, i, frame.end) {
                i += 1;
            }
            let mut component = parse_component(s.get(start..i)?)?;
            if frame.negates() {
                component.exponent = -component.exponent;
            }
            if !unit.push(component) {
                return None;
            }
        }
        // Between two factors of the current level.
        let frame = frames.get_mut(depth)?;
        if i < frame.end {
            match s.get(i..frame.end)? {
                [b'/', ..] => {
                    frame.in_denominator = true;
                    i += 1;
                }
                [b'*', ..] => i += 1,
                [0xC2, 0xB7, ..] => i += 2,
                // '(' right after a factor (implicit multiplication) or an
                // exponent after ')'.
                _ => return None,
            }
            if i >= frame.end {
                return None; // trailing separator
            }
        }
    }
}

/// Whether `s[i]` ends a plain factor: `*`, `/`, `(` or `·` (U+00B7).
fn is_separator_or_group(s: &[u8], i: usize, end: usize) -> bool {
    matches!(
        s.get(i..end),
        Some([b'*' | b'/' | b'(', ..] | [0xC2, 0xB7, ..])
    )
}

/// The index of the `)` that closes the `(` at `open`, searching before `end`.
fn matching_paren(s: &[u8], open: usize, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, &b) in s.get(open + 1..end)?.iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' if depth == 0 => return Some(open + 1 + offset),
            b')' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Parses one factor `[prefix[~]]base[exponent]` or `[prefix[~]]$CODE[exponent]`.
fn parse_component(s: &[u8]) -> Option<Component> {
    let (s, exponent) = match exponent_suffix(s) {
        Some((consumed, exponent)) => (s.get(..s.len() - consumed)?, exponent),
        None => (s, 1),
    };
    if s.is_empty() {
        return None;
    }
    let component = |base: u32, prefix: Option<Prefix>| -> Option<Component> {
        let base = BaseUnit(base);
        prefix_allowed(prefix, base).then_some(Component {
            base,
            prefix,
            exponent,
        })
    };

    // A currency carries a mandatory '$' sigil, which also separates it from
    // its prefix, so the '~' before it is optional.
    if let Some(sigil) = s.iter().position(|&b| b == b'$') {
        let code = s.get(sigil + 1..)?;
        if sigil == 0 {
            return component(data::currency_lookup(code)?, None);
        }
        let prefix_end = if s.get(sigil - 1) == Some(&b'~') {
            sigil - 1
        } else {
            sigil
        };
        if prefix_end == 0 {
            return None; // "~$EUR"
        }
        let base = data::currency_lookup(code)?;
        return component(base, Some(lookup_prefix(s.get(..prefix_end)?)?));
    }

    // The base is the longest spelling that is a suffix of the factor, which
    // keeps a bare unit ahead of any prefixed reading ("min" is the minute,
    // never milli-inch).
    let (base_len, base) = (1..=s.len().min(ALIAS_MAX_LEN))
        .rev()
        .find_map(|n| data::alias_lookup(s.get(s.len() - n..)?).map(|id| (n, id)))?;
    let prefix_len = s.len() - base_len;
    if prefix_len == 0 {
        return component(base, None);
    }
    let head = s.get(..prefix_len)?;
    let prefix = match head {
        [p @ .., b'~'] if !p.is_empty() => p,
        // A '~' anywhere else is a malformed separated form; a compact form
        // may be refused by name ("usb" is the bus, not microstilb).
        _ if head.contains(&b'~') || data::compact_refused(s) => return None,
        _ => head,
    };
    component(base, Some(lookup_prefix(prefix)?))
}

fn lookup_prefix(spelling: &[u8]) -> Option<Prefix> {
    data::prefix_lookup(spelling).map(|(system, id)| Prefix { system, id })
}

/// `bvn_prefix_unit_valid`: which prefixes may attach to which base unit.
pub(crate) fn prefix_allowed(prefix: Option<Prefix>, base: BaseUnit) -> bool {
    let Some(prefix) = prefix else {
        return true;
    };
    if base.is_currency() {
        return prefix.system == PrefixSystem::Si;
    }
    match (base.policy(), prefix.system) {
        (None, _) => false,
        (Some(PrefixPolicy::German | PrefixPolicy::Ratio | PrefixPolicy::None), _) => false,
        (Some(PrefixPolicy::Info), PrefixSystem::Iec) => true,
        (Some(PrefixPolicy::Info), PrefixSystem::Si) => prefix.exponent() >= 3,
        (Some(PrefixPolicy::Default), PrefixSystem::Iec) => false,
        (Some(PrefixPolicy::Default), PrefixSystem::Si) => true,
    }
}

/// Recognises an exponent at the end of a factor and returns the bytes it
/// takes and its value (`parse_unit_exponent_suffix`).
///
/// Two spellings: `^2`, `^-3`, `^+4` and superscripts `²`, `⁻³`, `⁺⁴`. At most
/// three digits are read, so `m^1000` has no exponent and fails as a base
/// symbol; a value that reads cleanly but is 0 or beyond ±100 is no exponent
/// either.
fn exponent_suffix(s: &[u8]) -> Option<(usize, i8)> {
    let digits = s
        .iter()
        .rev()
        .take(3)
        .take_while(|b| b.is_ascii_digit())
        .count();
    if digits > 0 {
        let first_digit = s.len() - digits;
        let (negative, before_sign) = match s.get(..first_digit)? {
            [rest @ .., b'-'] => (true, rest),
            [rest @ .., b'+'] => (false, rest),
            rest => (false, rest),
        };
        if let [.., b'^'] = before_sign {
            let magnitude = s
                .get(first_digit..)?
                .iter()
                .fold(0i32, |v, &d| v * 10 + i32::from(d - b'0'));
            let exponent = to_exponent(if negative { -magnitude } else { magnitude })?;
            return Some((s.len() - (before_sign.len() - 1), exponent));
        }
    }

    let mut end = s.len();
    let mut magnitude = 0i32;
    let mut scale = 1i32;
    let mut count = 0;
    while count < 3 {
        let Some((digit, width)) = superscript_digit_before(s, end) else {
            break;
        };
        magnitude += digit * scale;
        scale *= 10;
        end -= width;
        count += 1;
    }
    if count == 0 {
        return None;
    }
    let negative = match s.get(..end)? {
        [.., 0xE2, 0x81, 0xBB] => {
            end -= 3;
            true
        }
        [.., 0xE2, 0x81, 0xBA] => {
            end -= 3;
            false
        }
        _ => false,
    };
    let exponent = to_exponent(if negative { -magnitude } else { magnitude })?;
    Some((s.len() - end, exponent))
}

/// The superscript digit ending just before `end`, with its byte width:
/// ¹ ² ³ are two bytes (C2 B9/B2/B3), ⁰ and ⁴..⁹ three (E2 81 B0/B4..B9).
fn superscript_digit_before(s: &[u8], end: usize) -> Option<(i32, usize)> {
    match s.get(..end)? {
        [.., 0xE2, 0x81, 0xB0] => Some((0, 3)),
        [.., 0xE2, 0x81, b @ 0xB4..=0xB9] => Some((i32::from(*b - 0xB4) + 4, 3)),
        [.., 0xC2, 0xB9] => Some((1, 2)),
        [.., 0xC2, 0xB2] => Some((2, 2)),
        [.., 0xC2, 0xB3] => Some((3, 2)),
        _ => None,
    }
}

/// `bvn_int_to_exponent`: 0 and values beyond the range are not exponents.
fn to_exponent(n: i32) -> Option<i8> {
    if n == 0 || !(EXPONENT_MIN..=EXPONENT_MAX).contains(&n) {
        return None;
    }
    i8::try_from(n).ok()
}
