//! An exact decimal number, standing in for Python's `decimal.Decimal`
//! wherever the Python binding hands one out or takes one in
//! ([`crate::Quantity::decimal`], [`crate::Quantity::stored_value`],
//! [`crate::BvnFloat::to_decimal`], [`crate::Value::Decimal`]).
//!
//! It is the value Python's `Decimal` holds -- a sign, a coefficient of
//! decimal digits and a power-of-ten exponent, or one of the specials
//! infinity and NaN -- with none of the arithmetic or context machinery:
//! the binding only produces decimals exactly and compares them. A
//! `BigRational` would hold every finite value too, but not what Python's
//! `Decimal` also carries and the tests look at: the signed zero (`-0`),
//! the infinities and NaN, and the exponent that decides how it prints
//! (`Decimal("1.50")` is `1.50`, not `1.5`).
//!
//! Equality is numeric, as in Python: `1.5 == 1.50`, `0 == -0`, and NaN
//! equals nothing, itself included. `Display` is Python's `str()`, and
//! `FromStr` reads what Python's constructor reads (a signalling NaN is
//! read as a plain NaN).

use std::fmt;
use std::str::FromStr;

use num_bigint::{BigInt, BigUint, Sign};
use num_rational::BigRational;
use num_traits::{Pow, Zero};

use crate::error::{Error, Result};

#[derive(Clone, Debug)]
enum Repr {
    Finite { neg: bool, coeff: BigUint, exp: i64 },
    Infinite { neg: bool },
    NaN,
}

/// An exact decimal: `(-1)^sign * coefficient * 10^exponent`, an infinity
/// or NaN. See the module documentation.
#[derive(Clone, Debug)]
pub struct Decimal(Repr);

impl Decimal {
    /// `Decimal((sign, digits, exponent))`: `coefficient * 10^exponent`,
    /// negative when `neg` (a zero keeps its sign).
    pub fn new(neg: bool, coefficient: BigUint, exponent: i64) -> Self {
        Decimal(Repr::Finite {
            neg,
            coeff: coefficient,
            exp: exponent,
        })
    }

    /// `Decimal("NaN")`.
    pub fn nan() -> Self {
        Decimal(Repr::NaN)
    }

    /// `Decimal("Infinity")`, or `Decimal("-Infinity")` when `neg`.
    pub fn infinity(neg: bool) -> Self {
        Decimal(Repr::Infinite { neg })
    }

    pub fn is_nan(&self) -> bool {
        matches!(self.0, Repr::NaN)
    }

    pub fn is_infinite(&self) -> bool {
        matches!(self.0, Repr::Infinite { .. })
    }

    pub fn is_finite(&self) -> bool {
        matches!(self.0, Repr::Finite { .. })
    }

    /// Whether the sign is negative; true for `-0` and `-Infinity`.
    pub fn is_signed(&self) -> bool {
        match &self.0 {
            Repr::Finite { neg, .. } | Repr::Infinite { neg } => *neg,
            Repr::NaN => false,
        }
    }

    /// Whether this is a (signed) zero.
    pub fn is_zero(&self) -> bool {
        matches!(&self.0, Repr::Finite { coeff, .. } if coeff.is_zero())
    }

    /// The coefficient of a finite value (`as_tuple().digits`).
    pub fn coefficient(&self) -> Option<&BigUint> {
        match &self.0 {
            Repr::Finite { coeff, .. } => Some(coeff),
            _ => None,
        }
    }

    /// The exponent of a finite value (`as_tuple().exponent`).
    pub fn exponent(&self) -> Option<i64> {
        match &self.0 {
            Repr::Finite { exp, .. } => Some(*exp),
            _ => None,
        }
    }

    /// `Fraction(d)`: the exact value as a rational. Python raises for NaN
    /// and the infinities; so does this.
    pub fn to_fraction(&self) -> Result<BigRational> {
        let Repr::Finite { neg, coeff, exp } = &self.0 else {
            return Err(Error::argument(format!(
                "cannot convert {self} to a fraction"
            )));
        };
        let sign = if *neg { Sign::Minus } else { Sign::Plus };
        let c = BigInt::from_biguint(sign, coeff.clone());
        let p = BigInt::from(pow10(exp.unsigned_abs()));
        Ok(if *exp >= 0 {
            BigRational::from_integer(c * p)
        } else {
            BigRational::new(c, p)
        })
    }

    /// `float(d)`: the nearest `f64`, correctly rounded.
    pub fn to_f64(&self) -> f64 {
        // Python's float() reads the decimal string too, and Rust's parser
        // rounds correctly and reads every spelling `Display` writes.
        self.to_string().parse().unwrap_or(f64::NAN)
    }

    /// `int * 10^exponent` as a decimal with that exponent.
    pub(crate) fn from_scaled(value: &BigInt, exponent: i64) -> Self {
        Decimal::new(
            value.sign() == Sign::Minus,
            value.magnitude().clone(),
            exponent,
        )
    }

    /// The same value with trailing zeros of the coefficient moved into the
    /// exponent, but never past `ideal` (Python's ideal exponent of an exact
    /// quotient).
    pub(crate) fn reduced_to(mut self, ideal: i64) -> Self {
        if let Repr::Finite { coeff, exp, .. } = &mut self.0 {
            let ten = BigUint::from(10u32);
            while *exp < ideal && !coeff.is_zero() && (&*coeff % &ten).is_zero() {
                *coeff /= &ten;
                *exp += 1;
            }
            if coeff.is_zero() && *exp < ideal {
                *exp = ideal;
            }
        }
        self
    }

    /// The coefficient with trailing zeros stripped, for comparing values.
    fn normalised(coeff: &BigUint, exp: i64) -> (BigUint, i64) {
        let ten = BigUint::from(10u32);
        let (mut c, mut e) = (coeff.clone(), exp);
        while !c.is_zero() && (&c % &ten).is_zero() {
            c /= &ten;
            e += 1;
        }
        (c, e)
    }
}

/// `10^n`.
pub(crate) fn pow10(n: u64) -> BigUint {
    Pow::pow(BigUint::from(10u32), n)
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Repr::Infinite { neg: a }, Repr::Infinite { neg: b }) => a == b,
            (
                Repr::Finite {
                    neg: na,
                    coeff: ca,
                    exp: ea,
                },
                Repr::Finite {
                    neg: nb,
                    coeff: cb,
                    exp: eb,
                },
            ) => {
                if ca.is_zero() || cb.is_zero() {
                    return ca.is_zero() && cb.is_zero();
                }
                na == nb && Decimal::normalised(ca, *ea) == Decimal::normalised(cb, *eb)
            }
            _ => false,
        }
    }
}

impl From<BigInt> for Decimal {
    /// `Decimal(int)`: the integer with exponent 0.
    fn from(value: BigInt) -> Self {
        Decimal::from_scaled(&value, 0)
    }
}

impl From<i128> for Decimal {
    fn from(value: i128) -> Self {
        Decimal::from(BigInt::from(value))
    }
}

impl fmt::Display for Decimal {
    /// Python's `str(Decimal)` (the to-scientific-string conversion).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (neg, coeff, exp) = match &self.0 {
            Repr::NaN => return f.write_str("NaN"),
            Repr::Infinite { neg } => {
                return f.write_str(if *neg { "-Infinity" } else { "Infinity" });
            }
            Repr::Finite { neg, coeff, exp } => (*neg, coeff, *exp),
        };
        let digits = coeff.to_str_radix(10);
        let n = digits.len() as i64;
        let left = exp + n;
        let dot = if exp <= 0 && left > -6 { left } else { 1 };
        let mut out = String::new();
        if neg {
            out.push('-');
        }
        if dot <= 0 {
            out.push_str("0.");
            out.extend(std::iter::repeat_n('0', dot.unsigned_abs() as usize));
            out.push_str(&digits);
        } else if dot >= n {
            out.push_str(&digits);
            out.extend(std::iter::repeat_n('0', (dot - n) as usize));
        } else {
            out.push_str(&digits[..dot as usize]);
            out.push('.');
            out.push_str(&digits[dot as usize..]);
        }
        if left != dot {
            out.push_str(&format!("E{:+}", left - dot));
        }
        f.write_str(&out)
    }
}

impl FromStr for Decimal {
    type Err = Error;

    /// Python's `Decimal(text)`: surrounding whitespace is ignored, the
    /// specials are `Inf`, `Infinity`, `NaN` and `sNaN` in any case, and an
    /// underscore may group digits.
    fn from_str(text: &str) -> Result<Self> {
        let invalid = || Error::argument(format!("invalid literal for Decimal: {text:?}"));
        let s = text.trim();
        let (neg, body) = match s.as_bytes().first() {
            Some(b'-') => (true, &s[1..]),
            Some(b'+') => (false, &s[1..]),
            _ => (false, s),
        };
        let lower = body.to_ascii_lowercase();
        if lower == "inf" || lower == "infinity" {
            return Ok(Decimal::infinity(neg));
        }
        for prefix in ["nan", "snan"] {
            if let Some(payload) = lower.strip_prefix(prefix)
                && payload.bytes().all(|b| b.is_ascii_digit())
            {
                return Ok(Decimal::nan());
            }
        }
        let body = ungroup(body).ok_or_else(invalid)?;
        let (mantissa, exponent) = match body.find(['e', 'E']) {
            Some(i) => (&body[..i], Some(&body[i + 1..])),
            None => (body.as_str(), None),
        };
        let (int_part, frac_part) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let all_digits = |p: &str| p.bytes().all(|b| b.is_ascii_digit());
        if int_part.len() + frac_part.len() == 0 || !all_digits(int_part) || !all_digits(frac_part)
        {
            return Err(invalid());
        }
        let mut exp: i64 = match exponent {
            None => 0,
            Some(e) => {
                let digits = e.strip_prefix(['+', '-']).unwrap_or(e);
                if digits.is_empty() || !all_digits(digits) {
                    return Err(invalid());
                }
                e.parse().map_err(|_| invalid())?
            }
        };
        exp = exp
            .checked_sub(frac_part.len() as i64)
            .ok_or_else(invalid)?;
        let coeff = BigUint::parse_bytes(format!("{int_part}{frac_part}").as_bytes(), 10)
            .ok_or_else(invalid)?;
        Ok(Decimal::new(neg, coeff, exp))
    }
}

/// `text` with its digit-grouping underscores removed; `None` when an
/// underscore is not between two digits.
fn ungroup(text: &str) -> Option<String> {
    let b = text.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        if c == b'_' {
            let before = i > 0 && b[i - 1].is_ascii_digit();
            let after = b.get(i + 1).is_some_and(u8::is_ascii_digit);
            if !(before && after) {
                return None;
            }
        }
    }
    Some(text.replace('_', ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    #[test]
    fn prints_like_python() {
        for (input, want) in [
            ("0E-3", "0.000"),
            ("-0", "-0"),
            ("1e400", "1E+400"),
            ("1.5e-7", "1.5E-7"),
            ("123e3", "1.23E+5"),
            ("-1.5E-20000", "-1.5E-20000"),
            ("1.10", "1.10"),
            ("0.000001", "0.000001"),
            ("0.0000001", "1E-7"),
            ("12.5", "12.5"),
            ("-inf", "-Infinity"),
            ("nan", "NaN"),
            ("1_000", "1000"),
        ] {
            assert_eq!(d(input).to_string(), want, "{input}");
        }
    }

    #[test]
    fn equality_is_numeric() {
        assert_eq!(d("1.5"), d("1.50"));
        assert_eq!(d("0"), d("-0E+5"));
        assert_ne!(d("nan"), d("nan"));
        assert_ne!(d("1"), d("-1"));
        assert_eq!(d("-inf"), Decimal::infinity(true));
    }

    #[test]
    fn refuses_what_python_refuses() {
        for bad in ["", "abc", "1.2.3", "--1", "1e", ".", "1__0", "_1", "0x10"] {
            assert!(bad.parse::<Decimal>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn converts_exactly() {
        assert_eq!(
            d("-1.25").to_fraction().unwrap(),
            BigRational::new(BigInt::from(-5), BigInt::from(4))
        );
        assert_eq!(
            d("1e2").to_fraction().unwrap(),
            BigRational::from_integer(100.into())
        );
        assert!(d("inf").to_fraction().is_err());
        assert_eq!(d("0.1").to_f64(), 0.1);
    }
}
