//! Canonical spelling of a unit (`bvn_unit_to_string`).

use core::fmt::{self, Display, Formatter, Write};

use super::{profiles, Component, Unit};

/// Superscript digits ⁰..⁹.
const SUPERSCRIPT: [&str; 10] = ["⁰", "¹", "²", "³", "⁴", "⁵", "⁶", "⁷", "⁸", "⁹"];

impl Display for Unit {
    /// The canonical spelling: `no_unit`, or the components joined by `·`
    /// with SI-style `~` prefixes and superscript exponents. A unit with both
    /// positive and negative exponents is written as a fraction, numerator
    /// first, in source order on each side: `k~g·m/s²`.
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write_unit(self, f, false)
    }
}

/// [`Unit`] spelled with ASCII exponents and `*` (`BVN_UNIT_ASCII_EXP`):
/// `k~g*m/s^2`. Re-parses to the same unit.
#[derive(Debug, Clone, Copy)]
pub struct AsciiExponents<'a>(pub &'a Unit);

impl Display for AsciiExponents<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write_unit(self.0, f, true)
    }
}

impl Unit {
    /// This unit spelled with ASCII exponents (`^2`) and `*` instead of `·`.
    pub fn ascii(&self) -> AsciiExponents<'_> {
        AsciiExponents(self)
    }
}

fn write_unit(unit: &Unit, f: &mut Formatter<'_>, ascii: bool) -> fmt::Result {
    // A unit with an opaque component has no native spelling; its canonical
    // form is the profile notation in the namespace that owns it.
    if unit.is_profile_only() {
        if let Some(spelling) = profiles::opaque_spelling(unit) {
            return f.write_str(&spelling);
        }
    }
    let components = unit.components();
    let separator = if ascii { "*" } else { "·" };
    let has_numerator = components.iter().any(|c| c.exponent > 0);
    let has_denominator = components.iter().any(|c| c.exponent < 0);
    if components.is_empty() {
        return f.write_str("no_unit");
    }
    if !(has_numerator && has_denominator) {
        return write_joined(f, components.iter().copied(), separator, ascii);
    }
    let numerator = components.iter().filter(|c| c.exponent > 0).copied();
    write_joined(f, numerator, separator, ascii)?;
    f.write_char('/')?;
    let denominator = components
        .iter()
        .filter(|c| c.exponent < 0)
        .map(|c| Component {
            exponent: -c.exponent,
            ..*c
        });
    write_joined(f, denominator, separator, ascii)
}

fn write_joined(
    f: &mut Formatter<'_>,
    components: impl Iterator<Item = Component>,
    separator: &str,
    ascii: bool,
) -> fmt::Result {
    for (i, c) in components.enumerate() {
        if i > 0 {
            f.write_str(separator)?;
        }
        write_component(f, &c, ascii)?;
    }
    Ok(())
}

fn write_component(f: &mut Formatter<'_>, c: &Component, ascii: bool) -> fmt::Result {
    if let Some(prefix) = c.prefix {
        f.write_str(prefix.symbol())?;
        f.write_char('~')?;
    }
    if c.base.is_currency() {
        f.write_char('$')?;
    }
    f.write_str(c.base.symbol())?;
    let exponent = c.exponent();
    if exponent == 1 {
        return Ok(());
    }
    let magnitude = exponent.unsigned_abs();
    if ascii {
        f.write_char('^')?;
        if exponent < 0 {
            f.write_char('-')?;
        }
        return write!(f, "{magnitude}");
    }
    if exponent < 0 {
        f.write_char('⁻')?;
    }
    // At most three digits: the exponent is within ±100.
    let mut divisor = 100;
    let mut started = false;
    while divisor > 0 {
        let digit = (magnitude / divisor) % 10;
        if digit != 0 || started || divisor == 1 {
            started = true;
            f.write_str(SUPERSCRIPT.get(digit as usize).copied().unwrap_or(""))?;
        }
        divisor /= 10;
    }
    Ok(())
}
