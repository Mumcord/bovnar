//! A physical unit, mirroring `python/bovnar/unit.py`.

use std::fmt;

use crate::enums::{Exponent, PrefixSystem, SIPrefix};
use crate::error::{Error, Result};
use crate::ffi::{self, PrefixId, ValueUnit, ValueUnitComponent, ValueUnitPrefix, unit_fns};

pub use crate::ffi::MAX_UNIT_COMPONENTS;

/// One factor of a unit (`value_unit_component_t`): a base unit with its
/// prefix and exponent. Python's `ValueUnitComponent`; `prefix` is the SI or
/// IEC prefix id, as `prefix_system` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UnitComponent {
    pub base: i32,
    pub exponent: i32,
    pub prefix_system: i32,
    pub prefix: i32,
}

impl UnitComponent {
    /// `base` with an SI prefix (`SIPrefix::NONE` for none).
    pub fn si(base: i32, prefix: i32, exponent: i32) -> Self {
        UnitComponent {
            base,
            exponent,
            prefix_system: PrefixSystem::SI,
            prefix,
        }
    }

    /// `base` with an IEC prefix.
    pub fn iec(base: i32, prefix: i32, exponent: i32) -> Self {
        UnitComponent {
            base,
            exponent,
            prefix_system: PrefixSystem::IEC,
            prefix,
        }
    }

    fn raw(self) -> ValueUnitComponent {
        let id = if self.prefix_system == PrefixSystem::IEC {
            PrefixId { iec: self.prefix }
        } else {
            PrefixId { si: self.prefix }
        };
        ValueUnitComponent {
            base: self.base,
            exponent: self.exponent,
            prefix: ValueUnitPrefix {
                system: self.prefix_system,
                id,
            },
        }
    }
}

/// A physical unit: the parsed form, with the operations that belong to it.
/// The default is the dimensionless unit (no unit at all).
#[derive(Clone, Copy)]
pub struct Unit(pub(crate) ValueUnit);

impl Unit {
    /// Parses a unit string: native (`"m/s"`, `"k~g"`) or a profile code
    /// (`"ucum:mm[Hg]"`).
    pub fn parse(text: &str) -> Result<Self> {
        ffi::parse_unit(text).map(Unit).ok_or_else(|| {
            let code = ffi::unit_error_code(text);
            Error::Argument(format!(
                "invalid unit string {text:?}: {}",
                ffi::error_name(code)
            ))
        })
    }

    /// A unit of the given components (Python's `make_unit_compound`);
    /// fails beyond [`MAX_UNIT_COMPONENTS`].
    pub fn compound(components: &[UnitComponent]) -> Result<Self> {
        if components.len() > MAX_UNIT_COMPONENTS {
            return Err(Error::Argument(format!(
                "Too many unit components: {} > {MAX_UNIT_COMPONENTS}",
                components.len()
            )));
        }
        let mut vu = ValueUnit::zeroed();
        vu.num_components = components.len() as u32;
        for (slot, c) in vu.components.iter_mut().zip(components) {
            *slot = c.raw();
        }
        Ok(Unit(vu))
    }

    /// One SI-prefixed base unit (Python's `make_unit_si`).
    pub fn si(base: i32, prefix: i32, exponent: i32) -> Self {
        Self::one(UnitComponent::si(base, prefix, exponent))
    }

    /// One IEC-prefixed base unit (Python's `make_unit_iec`).
    pub fn iec(base: i32, prefix: i32, exponent: i32) -> Self {
        Self::one(UnitComponent::iec(base, prefix, exponent))
    }

    /// The explicit dimensionless unit: one `BaseUnit::NONE` component
    /// (Python's `make_unit_dimensionless`). [`Unit::default`] is the empty
    /// unit (`make_unit_none`).
    pub fn dimensionless() -> Self {
        Self::one(UnitComponent::si(0, SIPrefix::NONE, Exponent::LINEAR))
    }

    fn one(c: UnitComponent) -> Self {
        let mut vu = ValueUnit::zeroed();
        vu.num_components = 1;
        vu.components[0] = c.raw();
        Unit(vu)
    }

    /// The number of components as stored (`num_components`).
    pub fn num_components(&self) -> usize {
        self.0.num_components as usize
    }

    /// The active components (Python's `active_components()`).
    pub fn components(&self) -> Vec<UnitComponent> {
        let n = self.num_components().min(MAX_UNIT_COMPONENTS);
        self.0.components[..n]
            .iter()
            .map(|c| UnitComponent {
                base: c.base,
                exponent: c.exponent,
                prefix_system: c.prefix.system,
                prefix: c.prefix.id_raw(),
            })
            .collect()
    }

    /// True when this carries no unit at all (the empty unit, or only
    /// `bu_none` components). `%` and `ppm` are real units and answer false.
    pub fn is_dimensionless(&self) -> bool {
        let n = (self.0.num_components as usize).min(self.0.components.len());
        self.0.components[..n].iter().all(|c| c.base == 0)
    }

    /// True when this unit has no native spelling and can only be written in
    /// a profile notation (a UCUM arbitrary unit, a UNECE package code).
    pub fn is_profile_only(&self) -> bool {
        unit_fns::is_profile_only(self.0)
    }

    /// The value of one of this unit in coherent SI units. Fails for a unit
    /// with no SI conversion and for an affine one (°C), whose SI value is
    /// not a factor; see [`crate::units::unit_to_si_factor`].
    pub fn factor(&self) -> Result<f64> {
        let conv = crate::units::unit_to_si_factor(self)?;
        if conv.is_affine {
            return Err(Error::Argument(format!(
                "{self} is an affine unit: its SI value is value * factor + offset, not a factor"
            )));
        }
        Ok(conv.factor)
    }

    /// True when a value in this unit can be converted to `other`.
    pub fn compatible_with(&self, other: &Unit) -> bool {
        unit_fns::compatible(self.0, other.0)
    }

    /// The multiplier from this unit to `other`. Fails when the two do not
    /// convert, or convert only affinely (°C to K).
    pub fn convert_factor(&self, other: &Unit) -> Result<f64> {
        let conv = crate::units::unit_convert_factor(self, other)?;
        if conv.requires_affine && conv.factor == 0.0 {
            return Err(Error::Argument(format!(
                "{self} -> {other} is an affine conversion and has no multiplicative factor"
            )));
        }
        Ok(conv.factor)
    }

    /// This unit as a code in `namespace` (`"ucum"`, `"unece"`, `"qudt"`,
    /// ...), without the `ns:` prefix.
    pub fn to_profile(&self, namespace: &str) -> Result<String> {
        crate::units::unit_to_profile(namespace, self)
    }
}

impl Default for Unit {
    fn default() -> Self {
        Unit(ValueUnit::zeroed())
    }
}

/// The canonical spelling, or nothing when dimensionless.
impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_dimensionless() {
            return Ok(());
        }
        f.write_str(&ffi::unit_to_string_ex(&self.0, 0).unwrap_or_default())
    }
}

impl fmt::Debug for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Unit({:?})", self.to_string())
    }
}

impl PartialEq for Unit {
    fn eq(&self, other: &Self) -> bool {
        unit_fns::equal(self.0, other.0)
    }
}

impl std::str::FromStr for Unit {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Unit::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_formats_and_converts() {
        let km = Unit::parse("k~m").unwrap();
        let m = Unit::parse("m").unwrap();
        assert_eq!(km.to_string(), "k~m");
        assert!(km.compatible_with(&m));
        assert_eq!(km.convert_factor(&m).unwrap(), 1000.0);
        assert_eq!(km.factor().unwrap(), 1000.0);
        assert_ne!(km, m);
        assert!(Unit::default().is_dimensionless());
        assert_eq!(Unit::default().to_string(), "");
    }

    #[test]
    fn refuses_garbage_and_affine_factor() {
        assert!(Unit::parse("not a unit").is_err());
        let c = Unit::parse("°C").unwrap();
        assert!(c.factor().is_err());
        assert!(c.convert_factor(&Unit::parse("K").unwrap()).is_err());
    }
}
