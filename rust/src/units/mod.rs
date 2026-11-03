//! Units of measure (spec §11; unit system doc/05, policy doc/06).
//!
//! The native unit system of the C reference: 264 physical base units, the
//! SI and IEC prefixes, and 216 currencies, combined into compound units of up
//! to [`MAX_COMPONENTS`] factors with integer exponents in
//! [`EXPONENT_MIN`]`..=`[`EXPONENT_MAX`].
//!
//! [`parse_unit`] follows `bvn_parse_unit_n` / `bvn_unit_error_code`,
//! [`Unit`]'s `PartialEq` follows `bvn_unit_equal`, [`convertible`] follows
//! `bvn_units_convertible`, and [`Unit`]'s `Display` follows
//! `bvn_unit_to_string`.
//!
//! ```
//! use bovnar::units::{convertible, parse_unit};
//! use bovnar::SpecVersion;
//!
//! let a = parse_unit("k~g·m/s^2".as_bytes(), SpecVersion::V1_2).unwrap();
//! let b = parse_unit("s⁻²·kg·m".as_bytes(), SpecVersion::V1_2).unwrap();
//! assert_eq!(a, b);
//! assert_eq!(a.to_string(), "k~g·m/s²");
//! let n = parse_unit(b"N", SpecVersion::V1_2).unwrap();
//! assert_ne!(a, n);
//! assert!(convertible(&a, &n));
//! ```

mod convert;
mod data;
mod format;
mod gen;
mod parse;
mod profiles;
#[cfg(test)]
mod tests;

use crate::{ErrorKind, SpecVersion};
use data::PrefixPolicy;
use gen::units::{BU_DELTA_KELVIN, BU_KELVIN};

pub use format::AsciiExponents;

/// Most components one compound unit carries (`BVNR_MAX_UNIT_COMPONENTS`).
pub const MAX_COMPONENTS: usize = 32;
/// Smallest exponent a component may carry (`BVN_EXPONENT_MIN`).
pub const EXPONENT_MIN: i32 = -100;
/// Largest exponent a component may carry (`BVN_EXPONENT_MAX`).
pub const EXPONENT_MAX: i32 = 100;

/// A base unit: a native physical unit, a currency, or an opaque profile
/// unit.
///
/// The id is the C reference's `value_base_unit_t` value, which is stable
/// across releases and names its vocabulary by block: `100000..` native,
/// `300000..` UNECE's opaque units (packages and counts), `900000..`
/// currencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BaseUnit(u32);

impl BaseUnit {
    /// The stable numeric id (`value_base_unit_t`).
    pub fn id(self) -> u32 {
        self.0
    }

    /// The canonical symbol: `"m"`, `"°C"`, a currency's ISO code without
    /// its `$` sigil, or an opaque unit's profile code (`"XBX"`) without its
    /// namespace.
    pub fn symbol(self) -> &'static str {
        if let Some(row) = data::native_row(self.0) {
            row.symbol
        } else if let Some((_, _, code)) = profiles::opaque_row(self.0) {
            code
        } else {
            data::currency_code(self.0).unwrap_or("")
        }
    }

    /// The C enumerator suffix of a native or opaque unit (`"meter"` for
    /// `bu_meter`, `"unece_box"` for `bu_unece_box`).
    pub fn name(self) -> Option<&'static str> {
        data::native_row(self.0)
            .map(|row| row.name)
            .or_else(|| profiles::opaque_row(self.0).map(|(_, name, _)| name))
    }

    /// Whether this is an opaque profile unit: a unit with no native
    /// spelling and no SI dimension, such as a UNECE package. It converts to
    /// nothing but itself.
    pub fn is_opaque(self) -> bool {
        profiles::is_opaque(self.0)
    }

    /// Whether this is a currency (`$USD`).
    pub fn is_currency(self) -> bool {
        data::currency_code(self.0).is_some()
    }

    /// How this unit converts to coherent SI; `None` for a currency.
    pub fn conversion(self) -> Option<BaseConversion> {
        data::native_row(self.0).map(|row| BaseConversion {
            factor: row.factor,
            factor_rational: (row.factor_num, row.factor_den),
            offset: row.offset,
            offset_rational: (row.offset_num, row.offset_den),
            affine: row.affine,
            exact: row.exact,
            pi_factor: row.pi,
            dims: row.dims,
        })
    }

    fn policy(self) -> Option<PrefixPolicy> {
        data::native_row(self.0).map(|row| row.policy)
    }
}

/// How one base unit converts to coherent SI: `si = value · factor + offset`.
///
/// The rationals are decimal strings of arbitrary length (numerator,
/// denominator), reduced to lowest terms, exactly as the C reference's
/// conversion table carries them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaseConversion {
    /// The factor as the nearest `f64`.
    pub factor: f64,
    /// The factor as an exact rational; for an inexact unit (π-based angles)
    /// this is only the rational nearest to `factor`.
    pub factor_rational: (&'static str, &'static str),
    /// The affine offset in kelvin as the nearest `f64` (0 unless `affine`).
    pub offset: f64,
    /// The affine offset as an exact rational.
    pub offset_rational: (&'static str, &'static str),
    /// Whether this is an offset scale (°C, °F); it only means a temperature
    /// reading where it stands alone at exponent 1.
    pub affine: bool,
    /// Whether `factor_rational` is the true factor. False for units defined
    /// through π.
    pub exact: bool,
    /// For a π-defined unit, its factor as `(num / den) · π^power`, so two
    /// such units convert exactly wherever the powers of π cancel.
    pub pi_factor: Option<(&'static str, &'static str, i32)>,
    /// SI dimension exponents: length, mass, time, current, temperature,
    /// amount, luminous intensity.
    pub dims: [i8; 7],
}

/// The system a [`Prefix`] belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrefixSystem {
    /// Decimal SI prefixes (k, M, µ, ...).
    Si,
    /// Binary IEC prefixes (Ki, Mi, ...).
    Iec,
}

/// An SI or IEC prefix of a unit component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Prefix {
    system: PrefixSystem,
    /// `si_prefix_id_t` / `iec_prefix_id_t`, never 0 ("no prefix" is `None`).
    id: u8,
}

impl Prefix {
    /// The prefix system.
    pub fn system(self) -> PrefixSystem {
        self.system
    }

    /// The C enumerator id (`si_kilo` is 15, `iec_kibi` is 1).
    pub fn id(self) -> u8 {
        self.id
    }

    /// The canonical symbol (`"k"`, `"µ"`, `"Ki"`).
    pub fn symbol(self) -> &'static str {
        self.row().map_or("", |row| row.symbol)
    }

    /// The prefix name (`"kilo"`, `"kibi"`).
    pub fn name(self) -> &'static str {
        self.row().map_or("", |row| row.name)
    }

    /// The scale exponent: the prefix is 10^exponent for SI and 2^exponent
    /// for IEC.
    pub fn exponent(self) -> i32 {
        self.row().map_or(0, |row| row.exp)
    }

    fn row(self) -> Option<&'static data::PrefixRow> {
        data::prefix_row(self.system, self.id)
    }
}

/// One factor of a compound unit: `[prefix~]base[^exponent]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Component {
    base: BaseUnit,
    prefix: Option<Prefix>,
    /// In `EXPONENT_MIN..=EXPONENT_MAX`, never 0.
    exponent: i8,
}

impl Component {
    const EMPTY: Self = Self {
        base: BaseUnit(0),
        prefix: None,
        exponent: 1,
    };

    /// The base unit.
    pub fn base(&self) -> BaseUnit {
        self.base
    }

    /// The prefix, if any.
    pub fn prefix(&self) -> Option<Prefix> {
        self.prefix
    }

    /// The exponent, in [`EXPONENT_MIN`]`..=`[`EXPONENT_MAX`] and never 0.
    pub fn exponent(&self) -> i32 {
        i32::from(self.exponent)
    }
}

/// A parsed unit of measure: up to [`MAX_COMPONENTS`] components in source
/// order (a denominator's exponents negated). No components is `no_unit`.
///
/// Equality is `bvn_unit_equal`: the components compare as a multiset, so
/// `s³·m⁻⁵` equals `m⁻⁵·s³`, and a temperature difference `ΔK` equals `K`
/// everywhere except alone at exponent 1, where it is a different quantity.
/// It is identity of notation, not of dimension: `N` is not `k~g·m/s²`; use
/// [`convertible`] for that.
#[derive(Clone)]
pub struct Unit {
    len: u8,
    components: [Component; MAX_COMPONENTS],
}

impl Unit {
    /// The dimensionless `no_unit`.
    pub fn no_unit() -> Self {
        Self {
            len: 0,
            components: [Component::EMPTY; MAX_COMPONENTS],
        }
    }

    /// Whether this is `no_unit`.
    pub fn is_no_unit(&self) -> bool {
        self.len == 0
    }

    /// Whether this unit has no native spelling because it carries an opaque
    /// profile unit (`bvn_unit_is_profile_only`). Its canonical form is then
    /// the profile notation, e.g. `unece:XBX`.
    pub fn is_profile_only(&self) -> bool {
        self.components().iter().any(|c| c.base.is_opaque())
    }

    /// The components in source order.
    pub fn components(&self) -> &[Component] {
        self.components
            .get(..usize::from(self.len))
            .unwrap_or_default()
    }

    /// Appends a component; false when the unit is full.
    fn push(&mut self, component: Component) -> bool {
        match self.components.get_mut(usize::from(self.len)) {
            Some(slot) => {
                *slot = component;
                self.len += 1;
                true
            }
            None => false,
        }
    }

    /// The base a component counts as for equality (`bvni_equality_base`):
    /// ΔK folds into K except where it stands alone at exponent 1.
    fn equality_base(&self, c: &Component) -> BaseUnit {
        if c.base.0 == BU_DELTA_KELVIN && !(self.len == 1 && c.exponent == 1) {
            BaseUnit(BU_KELVIN)
        } else {
            c.base
        }
    }
}

impl PartialEq for Unit {
    fn eq(&self, other: &Self) -> bool {
        if self.len != other.len {
            return false;
        }
        let mut matched = [false; MAX_COMPONENTS];
        self.components().iter().all(|ca| {
            let a = Component {
                base: self.equality_base(ca),
                ..*ca
            };
            let hit = other
                .components()
                .iter()
                .zip(matched.iter_mut())
                .find(|(cb, used)| {
                    !**used
                        && a == Component {
                            base: other.equality_base(cb),
                            ..**cb
                        }
                });
            match hit {
                Some((_, used)) => {
                    *used = true;
                    true
                }
                None => false,
            }
        })
    }
}

impl Eq for Unit {}

impl core::fmt::Debug for Unit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Unit({self})")
    }
}

/// Parses a unit string as it appears in an annotation or an inline suffix.
///
/// `version` is the version the document declares; pass [`SpecVersion::V1_0`]
/// for a document without a version directive. The profile notation
/// (`ns:code`, doc/11) is spec 1.2 and is `UnitIllegal` below it; it is
/// translated into an ordinary unit, so `unece:KGM` equals `k~g`. A namespace this
/// build has no profile for is `UnitProfileUnknown`; every other refusal is
/// `UnitIllegal`, exactly as `bvn_unit_error_code` reports it. The length
/// limit (`UnitTooLong`) is the caller's to enforce.
pub fn parse_unit(text: &[u8], version: SpecVersion) -> Result<Unit, ErrorKind> {
    if profiles::has_namespace(text) {
        if version < SpecVersion::V1_2 {
            return Err(ErrorKind::UnitIllegal);
        }
        return profiles::parse(text);
    }
    parse::parse_expression(text).ok_or(ErrorKind::UnitIllegal)
}

/// Writes `unit` as a code of the profile `namespace`, without the `ns:`
/// prefix (`bvn_unit_to_profile`).
///
/// `None` when this build has no such profile or the unit has no spelling in
/// it: a flat vocabulary (UNECE, QUDT, OM) spells only a single component at
/// exponent 1, a unit outside the vocabulary has no code, and a code is only
/// written when it reads back as the same unit.
///
/// ```
/// use bovnar::units::{parse_unit, to_profile};
/// use bovnar::SpecVersion;
///
/// let kg = parse_unit(b"k~g", SpecVersion::V1_2).unwrap();
/// # #[cfg(feature = "profile-unece")]
/// assert_eq!(to_profile(&kg, "unece").as_deref(), Some("KGM"));
/// # #[cfg(feature = "profile-udunits")]
/// assert_eq!(to_profile(&kg, "udunits").as_deref(), Some("kg"));
/// assert_eq!(to_profile(&kg, "ucum"), None);
/// ```
pub fn to_profile(unit: &Unit, namespace: &str) -> Option<alloc::string::String> {
    profiles::to_profile(namespace, unit)
}

/// The profile namespaces this build carries, in the C registry order
/// (`bvn_unit_profile_name`). Each one is a cargo feature: `unece`, `qudt`,
/// `qudt-qk`, `udunits` and `om`. UCUM and CF are never available.
pub fn profile_namespaces() -> impl Iterator<Item = &'static str> {
    profiles::namespaces()
}

/// Whether a value in unit `a` can be converted to unit `b`
/// (`bvn_units_convertible`).
///
/// True when both have the same SI dimension and the same quantity kinds
/// (bit is not byte, an angle is not a count, `ΔK` alone is not `K`), or when
/// they differ only in their prefixes (`k~$USD` and `$USD`). It is a screen:
/// `s/°C` passes although no conversion of it is defined.
pub fn convertible(a: &Unit, b: &Unit) -> bool {
    convert::convertible(a, b)
}
