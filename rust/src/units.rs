//! Unit parsing, formatting and conversion helpers.

use crate::enums::{BaseUnit, Exponent, Prefix};
use crate::error::{Error, Result};
use crate::ffi::{self, PrefixId, ValueUnitPrefix, unit_fns};
use crate::unit::Unit;

/// Flags for [`unit_to_str_ex`].
pub struct UnitFlags;

impl UnitFlags {
    pub const NONE: u32 = 0;
    /// Reduce to the simplest named unit (kg·m·s⁻² → N).
    pub const REDUCE: u32 = 1 << 0;
    /// Write exponents as ASCII digits instead of superscripts.
    pub const ASCII_EXP: u32 = 1 << 1;
    /// Canonical component order.
    pub const NUM_CANONICAL: u32 = 1 << 2;
}

/// The names of the seven SI base dimensions, in the order of
/// [`unit_dimension_vector`].
pub const SI_DIM_NAMES: [&str; 7] = ["m", "kg", "s", "A", "K", "mol", "cd"];

/// The SI conversion of a unit: `si = value * factor + affine_offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SIConversion {
    pub factor: f64,
    pub is_affine: bool,
    pub affine_offset: f64,
}

/// A conversion between two units: multiply by `factor`, unless
/// `requires_affine` says a multiply is not enough.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitConversion {
    pub factor: f64,
    pub requires_affine: bool,
}

/// A simplified unit and the scale accumulated while simplifying it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReducedUnit {
    pub unit: Unit,
    pub scale: f64,
}

/// Parses a unit string, like [`Unit::parse`].
pub fn parse_unit(text: &str) -> Result<Unit> {
    Unit::parse(text)
}

/// The library error code explaining why `text` is not a unit (0 if it is).
pub fn unit_error_code(text: &str) -> i32 {
    ffi::unit_error_code(text)
}

/// The scale the prefixes of a unit string contribute, and nothing about the
/// base unit itself: this is `bvn_unit_prefix_factor`, so `M~Hz` is 1e6 while
/// `in` and `h` are 1.0. For the full factor to coherent SI use
/// [`Unit::factor`] or [`unit_to_si_factor`].
pub fn unit_factor(text: &str) -> Result<f64> {
    Ok(unit_prefix_factor(&Unit::parse(text)?))
}

/// The canonical spelling of `unit`.
pub fn unit_to_str(unit: &Unit) -> Result<String> {
    unit_to_str_ex(unit, UnitFlags::NONE)
}

/// The spelling of `unit` under [`UnitFlags`].
pub fn unit_to_str_ex(unit: &Unit, flags: u32) -> Result<String> {
    ffi::unit_to_string_ex(&unit.0, flags).map_err(|_| {
        Error::argument(if unit_valid(unit) {
            "unit_to_str_ex: the unit has no spelling under these flags"
        } else {
            "unit_to_str_ex: structurally invalid unit"
        })
    })
}

/// `unit` as a code in vocabulary `namespace`, without the `ns:` prefix.
pub fn unit_to_profile(namespace: &str, unit: &Unit) -> Result<String> {
    ffi::unit_to_profile(namespace, &unit.0).ok_or_else(|| {
        Error::argument(format!(
            "{unit} has no code in the {namespace:?} vocabulary"
        ))
    })
}

/// `unit` as a UCUM code.
pub fn unit_to_ucum(unit: &Unit) -> Result<String> {
    ffi::unit_to_ucum(&unit.0).ok_or_else(|| Error::argument(format!("{unit} has no UCUM code")))
}

/// True when `unit` can only be written in a profile notation.
pub fn unit_is_profile_only(unit: &Unit) -> bool {
    unit.is_profile_only()
}

/// The unit profile vocabularies compiled into this build.
pub fn available_profiles() -> Vec<String> {
    ffi::unit_profiles()
}

/// True when `unit` is structurally valid.
pub fn unit_valid(unit: &Unit) -> bool {
    unit_fns::valid(unit.0)
}

/// The combined prefix factor of `unit` (`k~m` → 1000).
pub fn unit_prefix_factor(unit: &Unit) -> f64 {
    unit_fns::prefix_factor(unit.0)
}

/// The combined decimal prefix exponent of `unit` (`k~m` → 3).
pub fn unit_prefix_exponent(unit: &Unit) -> i32 {
    unit_fns::prefix_exponent(unit.0)
}

/// True when `prefix` (system and id) may stand before base
/// unit `base`.
pub fn prefix_unit_valid(prefix: Prefix, base: BaseUnit) -> bool {
    let p = ValueUnitPrefix {
        system: prefix.system() as i32,
        id: PrefixId { si: prefix.id() },
    };
    unit_fns::prefix_unit_valid(p, base as i32)
}

/// The SI conversion of `unit`; fails for a unit with none (a currency).
pub fn unit_to_si_factor(unit: &Unit) -> Result<SIConversion> {
    let (factor, is_affine, affine_offset) = unit_fns::to_si_factor(unit.0)
        .ok_or_else(|| Error::argument(format!("{unit} has no SI conversion")))?;
    Ok(SIConversion {
        factor,
        is_affine,
        affine_offset,
    })
}

/// True when `a` and `b` share a dimension and no quantity-kind fence.
pub fn units_compatible(a: &Unit, b: &Unit) -> bool {
    unit_fns::compatible(a.0, b.0)
}

/// True when `a` and `b` are the same unit.
pub fn units_equal(a: &Unit, b: &Unit) -> bool {
    unit_fns::equal(a.0, b.0)
}

/// True when a value converts between `a` and `b` (compatible, or the same
/// unit apart from its prefixes).
pub fn units_convertible(a: &Unit, b: &Unit) -> bool {
    unit_fns::convertible(a.0, b.0)
}

/// `unit` in coherent SI base units, or `None` when it has no such form.
pub fn unit_si_normal_form(unit: &Unit) -> Option<Unit> {
    unit_fns::si_normal_form(unit.0).map(Unit)
}

/// The conversion from `from` to `to`; fails when they are incompatible.
pub fn unit_convert_factor(from: &Unit, to: &Unit) -> Result<UnitConversion> {
    let (factor, ok, requires_affine) = unit_fns::convert_factor(from.0, to.0);
    if !ok && !requires_affine {
        return Err(Error::argument(format!(
            "cannot convert {from} to {to}: the units are incompatible"
        )));
    }
    Ok(UnitConversion {
        factor,
        requires_affine,
    })
}

/// The exponents of the seven SI base dimensions, in [`SI_DIM_NAMES`] order.
pub fn unit_dimension_vector(unit: &Unit) -> Result<[i32; 7]> {
    unit_fns::dimension_vector(unit.0)
        .ok_or_else(|| Error::argument(format!("{unit} has no SI dimension vector")))
}

/// `unit` simplified to the fewest components, with the scale that costs.
pub fn unit_reduce(unit: &Unit) -> Result<ReducedUnit> {
    let (u, scale) = unit_fns::reduce(unit.0)
        .ok_or_else(|| Error::argument(format!("reducing {unit} overflows its scale")))?;
    Ok(ReducedUnit {
        unit: Unit(u),
        scale,
    })
}

/// The integer power of an exponent (`bvn_exponent_to_int`).
pub fn exponent_to_int(exp: Exponent) -> i32 {
    unit_fns::exponent_to_int(exp.get())
}

/// The exponent of power `n` (`bvn_int_to_exponent`); [`Exponent::INVALID`]
/// for a power the library has no exponent for.
pub fn int_to_exponent(n: i32) -> Exponent {
    Exponent::from_raw(unit_fns::int_to_exponent(n)).unwrap_or(Exponent::INVALID)
}

/// `value` in `from` converted to `to`, affine conversions included.
pub fn convert_value(value: f64, from: &Unit, to: &Unit) -> Result<f64> {
    unit_fns::convert_value(value, from.0, to.0)
        .ok_or_else(|| Error::argument(format!("cannot convert a value from {from} to {to}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers_answer_as_expected() {
        let km = parse_unit("k~m").unwrap();
        let m = parse_unit("m").unwrap();
        assert_eq!(unit_prefix_exponent(&km), 3);
        assert_eq!(unit_convert_factor(&km, &m).unwrap().factor, 1000.0);
        assert!(units_convertible(&km, &m) && !units_equal(&km, &m));
        assert_eq!(unit_dimension_vector(&m).unwrap().iter().sum::<i32>(), 1);
        let c = parse_unit("°C").unwrap();
        let k = parse_unit("K").unwrap();
        assert_eq!(convert_value(0.0, &c, &k).unwrap(), 273.15);
        assert!(unit_to_si_factor(&c).unwrap().is_affine);
        assert_eq!(unit_to_ucum(&km).unwrap(), "km");
        assert!(unit_convert_factor(&m, &parse_unit("s").unwrap()).is_err());
        assert_eq!(exponent_to_int(int_to_exponent(-2)), -2);
        assert_eq!(unit_factor("k~m").unwrap(), 1000.0);
        let _ = available_profiles();
    }
}
