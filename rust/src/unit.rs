//! A physical unit, mirroring `python/bovnar/unit.py`.

use std::fmt;
use std::hash::{Hash, Hasher};

use crate::enums::{BaseUnit, Exponent, IECPrefix, Prefix, PrefixSystem, SIPrefix};
use crate::error::{Error, Result};
use crate::ffi::{self, PrefixId, ValueUnit, ValueUnitComponent, ValueUnitPrefix, unit_fns};

pub use crate::ffi::MAX_UNIT_COMPONENTS;

/// One factor of a unit (`value_unit_component_t`): a base unit with its
/// prefix and exponent. Python's `ValueUnitComponent`, whose `prefix_system`
/// and `prefix` are one [`Prefix`] here. Components compare by value; an
/// unprefixed component equals whatever its prefix system, as in Python.
#[derive(Clone, Copy, Debug)]
pub struct UnitComponent {
    pub base: BaseUnit,
    pub exponent: Exponent,
    pub prefix: Prefix,
}

impl UnitComponent {
    /// `base` with an SI prefix (`SIPrefix::None` for none).
    pub fn si(base: BaseUnit, prefix: SIPrefix, exponent: Exponent) -> Self {
        UnitComponent {
            base,
            exponent,
            prefix: Prefix::Si(prefix),
        }
    }

    /// `base` with an IEC prefix.
    pub fn iec(base: BaseUnit, prefix: IECPrefix, exponent: Exponent) -> Self {
        UnitComponent {
            base,
            exponent,
            prefix: Prefix::Iec(prefix),
        }
    }

    /// The prefix system (Python's `prefix_system`).
    pub fn prefix_system(&self) -> PrefixSystem {
        self.prefix.system()
    }

    /// The prefix as compared: `(system, id)`, or `(Si, 0)` without a prefix.
    pub fn prefix_key(&self) -> (PrefixSystem, i32) {
        if self.prefix.is_none() {
            (PrefixSystem::Si, 0)
        } else {
            (self.prefix.system(), self.prefix.id())
        }
    }

    fn key(&self) -> (BaseUnit, Exponent, (PrefixSystem, i32)) {
        (self.base, self.exponent, self.prefix_key())
    }

    fn raw(self) -> ValueUnitComponent {
        let id = match self.prefix {
            Prefix::Iec(p) => PrefixId { iec: p as i32 },
            Prefix::Si(p) => PrefixId { si: p as i32 },
        };
        ValueUnitComponent {
            base: self.base as i32,
            exponent: self.exponent.get(),
            prefix: ValueUnitPrefix {
                system: self.prefix.system() as i32,
                id,
            },
        }
    }

    /// The component of a C `value_unit_component_t`. The library is built
    /// from the same tables as [`BaseUnit`], and validates prefixes and
    /// exponents, so every component it hands out has a Rust form; a value
    /// that had none would be a build fault, not input, hence the panic.
    fn from_c(c: &ValueUnitComponent) -> Self {
        UnitComponent {
            base: BaseUnit::from_raw(c.base)
                .unwrap_or_else(|| panic!("C base unit {} is not in BaseUnit", c.base)),
            exponent: Exponent::from_raw(c.exponent)
                .unwrap_or_else(|| panic!("C exponent {} is out of range", c.exponent)),
            prefix: Prefix::from_raw(c.prefix.system, c.prefix.id_raw()).unwrap_or_else(|| {
                panic!(
                    "C prefix ({}, {}) is not a known prefix",
                    c.prefix.system,
                    c.prefix.id_raw()
                )
            }),
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
            let message = format!("invalid unit string {text:?}: {}", ffi::error_name(code));
            match crate::ErrorCode::from_raw(code) {
                Some(kind) => Error::argument_with(kind, message),
                None => Error::argument(message),
            }
        })
    }

    /// A unit of the given components (Python's `make_unit_compound`);
    /// fails beyond [`MAX_UNIT_COMPONENTS`].
    pub fn compound(components: &[UnitComponent]) -> Result<Self> {
        if components.len() > MAX_UNIT_COMPONENTS {
            return Err(Error::argument(format!(
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
    pub fn si(base: BaseUnit, prefix: SIPrefix, exponent: Exponent) -> Self {
        Self::one(UnitComponent::si(base, prefix, exponent))
    }

    /// One IEC-prefixed base unit (Python's `make_unit_iec`).
    pub fn iec(base: BaseUnit, prefix: IECPrefix, exponent: Exponent) -> Self {
        Self::one(UnitComponent::iec(base, prefix, exponent))
    }

    /// The explicit dimensionless unit: one `BaseUnit::None` component
    /// (Python's `make_unit_dimensionless`). [`Unit::default`] is the empty
    /// unit (`make_unit_none`).
    pub fn dimensionless() -> Self {
        Self::one(UnitComponent::si(
            BaseUnit::None,
            SIPrefix::None,
            Exponent::LINEAR,
        ))
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
            .map(UnitComponent::from_c)
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
            return Err(Error::argument(format!(
                "{} is an affine unit: its SI value is value * factor + offset, not a factor. \
                 Use unit_to_si_factor() for both, or convert_value() to convert a value.",
                if self.is_dimensionless() {
                    "this".to_string()
                } else {
                    self.to_string()
                }
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
            return Err(Error::argument(format!(
                "{self} -> {other} is an affine conversion and has no multiplicative factor; \
                 use convert_value()"
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

impl PartialEq for UnitComponent {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl Eq for UnitComponent {}

impl Hash for UnitComponent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key().hash(state);
    }
}

/// Consistent with `==` (`bvn_unit_equal`), which ignores component order:
/// the components' keys, sorted.
impl Hash for Unit {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut keys: Vec<_> = self.components().iter().map(UnitComponent::key).collect();
        keys.sort_unstable();
        keys.dedup();
        keys.hash(state);
    }
}

impl Eq for Unit {}

/// Compares with a unit string; one that does not parse is simply unequal.
impl PartialEq<str> for Unit {
    fn eq(&self, other: &str) -> bool {
        Unit::parse(other).is_ok_and(|u| *self == u)
    }
}

impl PartialEq<&str> for Unit {
    fn eq(&self, other: &&str) -> bool {
        *self == **other
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
