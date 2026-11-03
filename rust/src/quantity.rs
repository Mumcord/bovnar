//! A typed scalar, mirroring `python/bovnar/quantity.py`: the verbatim
//! literal with its type, unit and (for a datetime) sub-second digits, so a
//! value read with [`crate::parse_typed`] is written back losslessly.

use num_bigint::BigInt;
use num_rational::BigRational;

use crate::Value;
use crate::bvnfloat::{
    BvnFloat, WORK_PREC, decimal_stored_value, decimal_to_interchange_bits, fraction_to_binary_bits,
};
use crate::decimal::Decimal;
use crate::enums::{TokenType, ValueTypeFamily};
use crate::error::{Error, Result};
use crate::ffi::{self, ValueTypeSpec};
use crate::unit::Unit;

#[derive(Clone, Debug, PartialEq)]
pub struct Quantity {
    /// The literal exactly as written; `None` for a typed null.
    pub raw: Option<String>,
    pub vtype: ValueTypeSpec,
    pub unit: Unit,
    /// How the literal was written: number, quoted string, symbol, ...
    pub token_type: TokenType,
    /// A datetime's verbatim sub-second digits.
    pub frac: Option<String>,
    /// The unit as the document spelled it, when that is not canonical.
    pub unit_source: Option<String>,
}

impl Quantity {
    /// A number literal of type `vtype` in `unit`.
    pub fn new(raw: &str, vtype: ValueTypeSpec, unit: Unit) -> Self {
        Quantity {
            raw: Some(raw.into()),
            vtype,
            unit,
            token_type: TokenType::Number,
            frac: None,
            unit_source: None,
        }
    }

    /// A decimal number literal `text` (`"1.25"`) as a `float_dec`, `float`
    /// or `float_fix` (with `frac` fractional bits) of `width` bits.
    pub fn from_number(
        text: &str,
        family: ValueTypeFamily,
        width: u32,
        frac: Option<u32>,
        unit: Unit,
    ) -> Result<Self> {
        let base = match (family, frac) {
            (ValueTypeFamily::FloatFix, Some(q)) => q,
            (ValueTypeFamily::FloatFix, None) => {
                return Err(Error::argument(
                    "float_fix requires the number of fractional bits",
                ));
            }
            (ValueTypeFamily::Float | ValueTypeFamily::FloatDec, _) => 0,
            _ => {
                return Err(Error::argument(
                    "from_number: family must be Float, FloatDec or FloatFix",
                ));
            }
        };
        let total = if width == 0 { 64 } else { width };
        validate_float_width(family, total)?;
        if text.parse::<f64>().is_err() {
            return Err(Error::argument(format!("{text:?} is not a decimal number")));
        }
        if family == ValueTypeFamily::FloatFix && !ffi::float_str_fits_fix(text, total, base) {
            return Err(Error::argument(format!(
                "{text} does not fit a signed float_fix:{width},q{base}"
            )));
        }
        if family == ValueTypeFamily::FloatDec {
            check_decimal_fits(text, total)?;
        }
        let vtype = ValueTypeSpec {
            family: family as i32,
            width,
            base,
        };
        Ok(Quantity::new(text, vtype, unit))
    }

    /// The literal decoded to a plain value, as untyped parsing would.
    pub fn value(&self) -> Value {
        crate::decode_value(
            self.raw.as_deref().unwrap_or("").as_bytes(),
            self.vtype,
            Some(self.token_type),
        )
    }

    /// The type family, when it is one this binding knows.
    pub fn family(&self) -> Option<ValueTypeFamily> {
        ValueTypeFamily::from_raw(self.vtype.family)
    }

    /// A datetime's epoch name (`"unix"`, ...); `None` for other types.
    pub fn epoch_name(&self) -> Option<String> {
        self.is_datetime()
            .then(|| ffi::datetime_epoch(self.vtype).0)
            .flatten()
    }

    /// A datetime's epoch as a Modified Julian Date.
    pub fn epoch_mjd(&self) -> Option<i32> {
        self.is_datetime()
            .then(|| ffi::datetime_epoch(self.vtype).1)
    }

    /// The canonical unit spelling, empty when unitless.
    pub fn unit_str(&self) -> String {
        self.unit.to_string()
    }

    fn is_datetime(&self) -> bool {
        self.family() == Some(ValueTypeFamily::Datetime)
    }

    // ── lossless numeric access ──────────────────────────────────────────
    // `value` decodes through an f64 and loses precision for float_dec,
    // float_fix and the wide binary floats. These accessors use the verbatim
    // literal and the library's arbitrary-precision float instead.

    /// The declared width, 64 when the type leaves it out.
    fn width(&self) -> u32 {
        if self.vtype.width == 0 {
            64
        } else {
            self.vtype.width
        }
    }

    fn is_floatish(&self) -> bool {
        matches!(
            self.family(),
            Some(ValueTypeFamily::Float | ValueTypeFamily::FloatDec | ValueTypeFamily::FloatFix)
        )
    }

    /// The radix of the literal: a binary float's base (10 when unset);
    /// float_dec and float_fix are always decimal.
    fn numeric_base(&self) -> u32 {
        match self.family() {
            Some(ValueTypeFamily::Float) if self.vtype.base != 0 => self.vtype.base,
            _ => 10,
        }
    }

    /// NaN or a signed infinity when the literal is one of their spellings.
    fn nonfinite_decimal(&self) -> Option<Decimal> {
        let low = self
            .raw
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        match low.as_str() {
            "nan" => Some(Decimal::nan()),
            "inf" | "+inf" | "infinity" | "+infinity" => Some(Decimal::infinity(false)),
            "ninf" | "-inf" | "-infinity" => Some(Decimal::infinity(true)),
            _ => None,
        }
    }

    fn raw_str(&self) -> &str {
        self.raw.as_deref().unwrap_or("")
    }

    /// The exact value as a [`Decimal`], from the verbatim literal (Python's
    /// `decimal`): lossless for `uint`/`sint` and the three float families;
    /// `None` for a typed null. Any other family is an error.
    pub fn decimal(&self) -> Result<Option<Decimal>> {
        let family = self.family();
        match family {
            Some(ValueTypeFamily::Uint | ValueTypeFamily::Sint) => {
                let Some(raw) = self.raw.as_deref().filter(|r| !r.is_empty()) else {
                    return Ok(None);
                };
                let base = if self.vtype.base > 1 {
                    self.vtype.base
                } else {
                    10
                };
                crate::dict_parser::bigint_from_base(raw, base)
                    .map(|i| Some(Decimal::from(i)))
                    .ok_or_else(|| {
                        Error::argument(format!("{raw:?} is not an integer in base {base}"))
                    })
            }
            _ if self.is_floatish() => {
                if let Some(nf) = self.nonfinite_decimal() {
                    return Ok(Some(nf));
                }
                let Some(raw) = self.raw.as_deref() else {
                    return Ok(None);
                };
                match self.numeric_base() {
                    10 => raw.parse().map(Some),
                    base => BvnFloat::from_str(raw, base, WORK_PREC)?
                        .to_decimal()
                        .map(Some),
                }
            }
            _ => Err(Error::argument(format!(
                "decimal() is not defined for a {} value",
                family_name(self.vtype.family)
            ))),
        }
    }

    /// The exact value as a rational (Python's `fraction`): for a
    /// `float_fix` the Q-format datum `mantissa / 2^frac_bits`, for the
    /// others the exact literal; `None` for a typed null.
    pub fn fraction(&self) -> Result<Option<BigRational>> {
        if self.family() == Some(ValueTypeFamily::FloatFix) {
            let (m, q) = self.fixed_point()?;
            return Ok(Some(BigRational::new(m, BigInt::from(1) << q)));
        }
        match self.decimal()? {
            None => Ok(None),
            Some(d) => d.to_fraction().map(Some),
        }
    }

    /// `(mantissa, frac_bits)` of a `float_fix` value, whose exact value is
    /// `mantissa / 2^frac_bits` (Python's `fixed_point`). Any other family
    /// is an error.
    pub fn fixed_point(&self) -> Result<(BigInt, u32)> {
        if self.family() != Some(ValueTypeFamily::FloatFix) {
            return Err(Error::argument("fixed_point() is only valid for float_fix"));
        }
        let q = self.vtype.base;
        let f = BvnFloat::from_str(self.raw_str(), 10, WORK_PREC)?;
        Ok((f.to_fixed_mantissa(self.width(), q)?, q))
    }

    /// Whether the C parser collapsed a finite, nonzero literal to zero or
    /// infinity: its exact rational exceeds the 32768-bit intermediate cap
    /// (a decimal exponent beyond about 1e±9865).
    fn parser_saturated(f: &BvnFloat, lit: Option<&Decimal>) -> bool {
        lit.is_some_and(|d| d.is_finite() && !d.is_zero()) && (f.is_zero() || f.is_inf())
    }

    /// The IEEE decimal interchange bytes of this float_dec value, over the
    /// full range: the C encoder, or the exact one past the parser's cap.
    fn decimal_bits(&self) -> Result<Vec<u8>> {
        let width = self.width();
        // nan, inf and ninf have no digits to round: one spelling each,
        // straight to the encoder.
        if let Some(nf) = self.nonfinite_decimal() {
            let text = if nf.is_nan() {
                "nan"
            } else if nf.is_signed() {
                "ninf"
            } else {
                "inf"
            };
            return BvnFloat::strtodec_bits(text, width);
        }
        let f = BvnFloat::from_str(self.raw_str(), 10, WORK_PREC)?;
        let lit = self.decimal()?;
        if let Some(lit) = lit
            .as_ref()
            .filter(|_| Self::parser_saturated(&f, lit.as_ref()))
        {
            return decimal_to_interchange_bits(lit, width);
        }
        // Straight from the literal: encoding the parsed float would round a
        // decimal tie through a binary intermediate.
        BvnFloat::strtodec_bits(self.raw_str(), width)
    }

    /// The materialised float_dec value, full range.
    fn decimal_stored(&self) -> Result<Decimal> {
        let width = self.width();
        let f = BvnFloat::from_str(self.raw_str(), 10, WORK_PREC)?;
        let lit = self.decimal()?;
        if let Some(lit) = lit
            .as_ref()
            .filter(|_| Self::parser_saturated(&f, lit.as_ref()))
        {
            return decimal_stored_value(lit, width);
        }
        BvnFloat::from_decimal_bits(width, &BvnFloat::strtodec_bits(self.raw_str(), width)?)?
            .to_decimal()
    }

    /// The IEEE binary interchange bytes of this binary float, full range:
    /// the C encoder, or the exact one past the parser's cap.
    fn binary_bits(&self) -> Result<Vec<u8>> {
        let width = self.width();
        let base = self.numeric_base();
        let f = BvnFloat::from_str(self.raw_str(), base, WORK_PREC)?;
        let lit = self.decimal()?;
        if base == 10
            && let Some(lit) = lit
                .as_ref()
                .filter(|_| Self::parser_saturated(&f, lit.as_ref()))
        {
            return fraction_to_binary_bits(&lit.to_fraction()?, width);
        }
        f.to_binary_bits(width)
    }

    /// The value as materialised into the declared IEEE or fixed-point
    /// format (round to nearest even), as an exact [`Decimal`] (Python's
    /// `stored_value`). It differs from [`Quantity::decimal`] only where the
    /// literal carries more precision than the format: a 40-digit
    /// `float_dec:64` literal rounds to 16 digits here. Defined for
    /// `float:16/32/64/128/256`, every `float_dec` and every `float_fix`
    /// width; `None` for a typed null. A binary float width with no IEEE
    /// encoding (`float:96`, `float:512`) is an error: use `decimal`.
    ///
    /// A `float_fix` value `m / 2^q` is always a terminating decimal and is
    /// given exactly. (Python divides in its default 28-digit context, so
    /// it rounds a datum with more significant digits than that.)
    pub fn stored_value(&self) -> Result<Option<Decimal>> {
        let family = self.family();
        let width = self.width();
        if self.raw.is_none() && self.is_floatish() {
            return Ok(None);
        }
        if family == Some(ValueTypeFamily::FloatFix) {
            let (m, q) = self.fixed_point()?;
            // m / 2^q = m * 5^q / 10^q, given with Python's ideal exponent
            // of an exact quotient (0): trailing zeros stripped down to it.
            let scaled = m * num_traits::Pow::pow(BigInt::from(5), q);
            return Ok(Some(
                Decimal::from_scaled(&scaled, -i64::from(q)).reduced_to(0),
            ));
        }
        if let Some(nf) = self.nonfinite_decimal() {
            return Ok(Some(nf));
        }
        match family {
            Some(ValueTypeFamily::Float) if !IEEE_WIDTHS.contains(&width) => {
                Err(Error::argument(format!(
                    "stored_value() materialises IEEE 16/32/64/128/256-bit floats; float:{width} \
                     has no such encoding — use .decimal for the exact literal value"
                )))
            }
            Some(ValueTypeFamily::Float) => {
                BvnFloat::from_binary_bits(width, &self.binary_bits()?)?
                    .to_decimal()
                    .map(Some)
            }
            Some(ValueTypeFamily::FloatDec) => self.decimal_stored().map(Some),
            _ => Err(Error::argument(format!(
                "stored_value() is not defined for a {} value",
                family_name(self.vtype.family)
            ))),
        }
    }

    /// The raw IEEE-754 interchange bytes of the materialised value,
    /// little-endian (Python's `ieee_bits`): binary16..256 for `float`, the
    /// decimal16..256 interchange encoding for `float_dec`. A typed null, a
    /// binary width with no IEEE encoding and any other family are errors.
    pub fn ieee_bits(&self) -> Result<Vec<u8>> {
        let family = self.family();
        let width = self.width();
        if self.raw.is_none() && self.is_floatish() {
            return Err(Error::argument(
                "ieee_bits() is not defined for a null value: it has a type and no number \
                 to encode",
            ));
        }
        match family {
            Some(ValueTypeFamily::Float) if !IEEE_WIDTHS.contains(&width) => {
                Err(Error::argument(format!(
                    "ieee_bits() materialises IEEE 16/32/64/128/256-bit floats; float:{width} \
                     has no such encoding — use .decimal instead"
                )))
            }
            Some(ValueTypeFamily::Float) => self.binary_bits(),
            Some(ValueTypeFamily::FloatDec) => self.decimal_bits(),
            _ => Err(Error::argument(
                "ieee_bits() is only valid for float (16..256) and float_dec values",
            )),
        }
    }
}

/// Python's `ValueTypeFamily(n).name`.
fn family_name(family: i32) -> String {
    use ValueTypeFamily as F;
    match ValueTypeFamily::from_raw(family) {
        Some(F::Plain) => "PLAIN".into(),
        Some(F::Utf8) => "UTF8".into(),
        Some(F::Sint) => "SINT".into(),
        Some(F::Uint) => "UINT".into(),
        Some(F::Float) => "FLOAT".into(),
        Some(F::FloatFix) => "FLOAT_FIX".into(),
        Some(F::FloatDec) => "FLOAT_DEC".into(),
        Some(F::Bool) => "BOOL".into(),
        Some(F::Datetime) => "DATETIME".into(),
        Some(F::Illegal) => "ILLEGAL".into(),
        None => format!("family {family}"),
    }
}

/// The IEEE interchange widths, the only ones decimal and fixed-point take.
const IEEE_WIDTHS: [u32; 5] = [16, 32, 64, 128, 256];

/// Rejects a width the library would refuse, early and with a clear message:
/// binary float takes 16 or any multiple of 32 up to 32768 (it keeps the
/// verbatim literal at that precision); decimal and fixed-point only the IEEE
/// interchange set.
fn validate_float_width(family: ValueTypeFamily, width: u32) -> Result<()> {
    let ok = match family {
        ValueTypeFamily::Float => {
            width == 16 || (width >= 32 && width.is_multiple_of(32) && width <= 32768)
        }
        _ => IEEE_WIDTHS.contains(&width),
    };
    if ok {
        return Ok(());
    }
    Err(Error::argument(match family {
        ValueTypeFamily::Float => {
            format!("binary float width must be 16 or a multiple of 32 (up to 32768), not {width}")
        }
        ValueTypeFamily::FloatDec => {
            format!("float_dec width must be one of 16/32/64/128/256, not {width}")
        }
        _ => format!("float_fix width must be one of 16/32/64/128/256, not {width}"),
    }))
}

/// Refuses a finite decimal literal that `float_dec:width` cannot hold: the
/// reader does not range-check a decimal float, so `1e400` written as a
/// decimal64 would silently materialise to infinity. Only overflow is
/// refused; rounding away extra digits or underflowing to zero is a rounding.
fn check_decimal_fits(text: &str, width: u32) -> Result<()> {
    // `nan`, `inf` and `ninf` have no digits and are not a range question.
    if !text.bytes().any(|b| b.is_ascii_digit()) {
        return Ok(());
    }
    let Some(bits) = ffi::float_strtodec(text, width) else {
        return Ok(());
    };
    // Combination field 11110 after the sign bit: infinity.
    let top = bits.last().copied().unwrap_or(0);
    if top & 0x7C == 0x78 {
        return Err(Error::argument(format!(
            "{text} is out of range for float_dec:{width} (it would materialise to infinity); \
             use a wider float_dec"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_and_validates() {
        let km = Unit::parse("k~m").unwrap();
        let q = Quantity::from_number("1.25", ValueTypeFamily::FloatDec, 64, None, km).unwrap();
        assert_eq!(q.value(), Value::Float(1.25));
        assert_eq!(q.unit_str(), "k~m");
        assert_eq!(q.epoch_name(), None);
        assert!(
            Quantity::from_number(
                "1e6",
                ValueTypeFamily::FloatFix,
                16,
                Some(8),
                Unit::default()
            )
            .is_err()
        );
        assert!(Quantity::from_number("x", ValueTypeFamily::Float, 64, None, km).is_err());
    }

    /// Python divides `m / 2**q` in its 28-digit default context, so a wide
    /// fixed-point datum lost digits; the stored value here is exact.
    #[test]
    fn fixed_point_stored_value_is_exact() {
        let vt = ValueTypeSpec {
            family: ValueTypeFamily::FloatFix as i32,
            width: 256,
            base: 100,
        };
        let q = Quantity::new("0.3333333333333333333333333333333", vt, Unit::default());
        let (m, frac) = q.fixed_point().unwrap();
        let exact = BigRational::new(m, BigInt::from(1) << frac);
        let stored = q.stored_value().unwrap().unwrap();
        assert!(stored.coefficient().unwrap().to_str_radix(10).len() > 28);
        assert_eq!(stored.to_fraction().unwrap(), exact);
    }
}
