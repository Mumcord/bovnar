//! Value types: families, annotation parameters and their effective values.

/// The type family of a value (spec §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeFamily {
    /// No type: an untyped value before a default was inferred.
    Plain,
    /// Signed integer.
    Sint,
    /// Unsigned integer.
    Uint,
    /// Binary floating point.
    Float,
    /// Fixed-point (Q format); `base` holds the number of fraction bits.
    FloatFix,
    /// Decimal floating point.
    FloatDec,
    /// Boolean.
    Bool,
    /// UTF-8 string.
    Utf8,
    /// Point in time; `base` holds the epoch index.
    Datetime,
}

/// A value type: family plus raw width and base fields as the C reference
/// stores them (`value_type_spec_t`). A zero width or base means "default".
///
/// For [`TypeFamily::FloatFix`] the `base` field holds `q`; for
/// [`TypeFamily::Datetime`] it holds the epoch index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeSpec {
    /// The family.
    pub family: TypeFamily,
    /// Raw width in bits, 0 for the default.
    pub width: u32,
    /// Raw base, `q` or epoch index (see above), 0 for the default.
    pub base: u32,
}

impl TypeSpec {
    /// The plain (untyped) spec.
    pub const PLAIN: Self = Self {
        family: TypeFamily::Plain,
        width: 0,
        base: 0,
    };

    /// Width in bits, 64 when unset.
    pub const fn effective_width(self) -> u32 {
        if self.width == 0 {
            64
        } else {
            self.width
        }
    }

    /// Numeric base of the literal. float_fix, float_dec and datetime are
    /// always decimal: their `base` field means something else.
    pub fn effective_base(self) -> u32 {
        match self.family {
            TypeFamily::FloatFix | TypeFamily::FloatDec | TypeFamily::Datetime => 10,
            _ if self.base == 0 => 10,
            _ => self.base,
        }
    }

    /// Fraction bits of a float_fix, 0 for every other family.
    pub fn effective_q(self) -> u32 {
        if self.family == TypeFamily::FloatFix {
            self.base
        } else {
            0
        }
    }

    /// Whether the family is one of the numeric families (datetime is not).
    pub fn is_numeric(self) -> bool {
        matches!(
            self.family,
            TypeFamily::Sint
                | TypeFamily::Uint
                | TypeFamily::Float
                | TypeFamily::FloatFix
                | TypeFamily::FloatDec
        )
    }
}
