//! A typed scalar, mirroring `python/bovnar/quantity.py`: the verbatim
//! literal with its type, unit and (for a datetime) sub-second digits, so a
//! value read with [`crate::parse_typed`] is written back losslessly.

use crate::Value;
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
                return Err(Error::Argument(
                    "float_fix requires the number of fractional bits".into(),
                ));
            }
            (ValueTypeFamily::Float | ValueTypeFamily::FloatDec, _) => 0,
            _ => {
                return Err(Error::Argument(
                    "from_number: family must be Float, FloatDec or FloatFix".into(),
                ));
            }
        };
        if text.parse::<f64>().is_err() {
            return Err(Error::Argument(format!("{text:?} is not a decimal number")));
        }
        let total = if width == 0 { 64 } else { width };
        if family == ValueTypeFamily::FloatFix && !ffi::float_str_fits_fix(text, total, base) {
            return Err(Error::Argument(format!(
                "{text} does not fit a signed float_fix:{width},q{base}"
            )));
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
}
