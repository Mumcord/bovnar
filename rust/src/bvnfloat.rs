//! The library's arbitrary-precision float (`bvn_float_t`), mirroring
//! `python/bovnar/_bvnfloat.py`: the bridge to the full precision of the
//! float formats instead of an `f64`.
//!
//! - exact text <-> [`BvnFloat`] conversion at any precision;
//! - the IEEE-754 binary interchange materialisation of a literal
//!   (binary16/32/64/128/256), correctly rounded in a single step;
//! - the IEEE-754 decimal interchange materialisation (decimal16..256);
//! - the fixed-point Q-format materialisation.
//!
//! The pure functions ([`fraction_to_binary_bits`],
//! [`decimal_to_interchange_bits`], [`decimal_stored_value`]) materialise
//! an exact value with no size limit; [`crate::Quantity`] falls back to
//! them where the C parser saturates an extreme exponent (beyond about
//! 1e±9865). Python's `Fraction` is [`BigRational`] here, its `Decimal` is
//! [`Decimal`].
//!
//! Byte strings are little-endian, as in Python. Where Python's `bytes`
//! argument may be shorter than the format (it is zero-extended), so may
//! the slice here; a longer one is refused.

use num_bigint::{BigInt, BigUint};
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{Signed, Zero};

use crate::decimal::{Decimal, pow10};
use crate::error::{Error, Result};
use crate::ffi::{self, FloatHandle};

/// Working precision (in bits) of the intermediate float used to
/// materialise a literal into a fixed format: about 1233 decimal digits,
/// far more than binary256's 237-bit significand or decimal256's 70 digits
/// need, so the narrowing step inside the encoder is the only rounding.
pub const WORK_PREC: u32 = 4096;

/// IEEE-754 binary interchange parameters: width -> (exponent bits,
/// mantissa bits, bias).
pub const BINARY_FORMATS: [(u32, (u32, u32, u32)); 5] = [
    (16, (5, 10, 15)),
    (32, (8, 23, 127)),
    (64, (11, 52, 1023)),
    (128, (15, 112, 16383)),
    (256, (19, 236, 262143)),
];

/// IEEE-754 decimal interchange significand digits per width. The
/// library's dec16 is a 2-digit miniature; the others are the standard
/// decimal32/64/128 and the 256-bit extension.
pub const DECIMAL_DIGITS: [(u32, u32); 5] = [(16, 2), (32, 7), (64, 16), (128, 34), (256, 70)];

/// The IEEE decimal interchange parameters of one width, mirroring the C
/// `DecFmt` tables in `bvn_float.c`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecimalFormat {
    pub total_bits: u32,
    pub exp_bits: u32,
    pub coeff_bits: u32,
    pub bias: i64,
    pub max_digits: u32,
}

const fn dec(
    total_bits: u32,
    exp_bits: u32,
    coeff_bits: u32,
    bias: i64,
    max_digits: u32,
) -> DecimalFormat {
    DecimalFormat {
        total_bits,
        exp_bits,
        coeff_bits,
        bias,
        max_digits,
    }
}

/// Width -> IEEE decimal interchange parameters.
pub const DECIMAL_FORMATS: [(u32, DecimalFormat); 5] = [
    (16, dec(16, 6, 9, 24, 2)),
    (32, dec(32, 8, 23, 101, 7)),
    (64, dec(64, 10, 53, 398, 16)),
    (128, dec(128, 14, 113, 6176, 34)),
    (256, dec(256, 20, 235, 611_867, 70)),
];

fn lookup<T: Copy>(table: &[(u32, T)], width: u32) -> Option<T> {
    table.iter().find(|(w, _)| *w == width).map(|(_, v)| *v)
}

/// `(exponent bits, mantissa bits, bias)` of an IEEE binary width.
pub fn binary_params(width: u32) -> Result<(u32, u32, u32)> {
    lookup(&BINARY_FORMATS, width).ok_or_else(|| {
        Error::argument(format!(
            "no IEEE binary interchange format for float width {width}"
        ))
    })
}

fn decimal_format(width: u32) -> Result<DecimalFormat> {
    lookup(&DECIMAL_FORMATS, width)
        .ok_or_else(|| Error::argument(format!("no decimal interchange format for width {width}")))
}

/// The sign of `num/den - 2^e` for positive `num`, `den`.
fn cmp_pow2(num: &BigInt, den: &BigInt, e: i64) -> std::cmp::Ordering {
    if e >= 0 {
        num.cmp(&(den << e))
    } else {
        (num << -e).cmp(den)
    }
}

/// `num/den` (both positive) rounded to the nearest integer, ties to even.
fn round_half_even(num: &BigInt, den: &BigInt) -> BigInt {
    let (mut q, r) = num.div_rem(den);
    let t: BigInt = r * 2;
    if t > *den || (t == *den && q.is_odd()) {
        q += 1;
    }
    q
}

/// `bits` as `nbytes` little-endian bytes.
fn to_le(bits: &BigUint, nbytes: usize) -> Vec<u8> {
    let mut out = bits.to_bytes_le();
    out.resize(nbytes, 0);
    out
}

/// The correctly rounded IEEE-754 binary`width` interchange bytes of an
/// exact rational, little-endian.
///
/// Full range: normals, subnormals, overflow to ±inf and underflow to ±0,
/// round to nearest even. This is the materialisation [`crate::Quantity`]
/// uses where the C parser saturates an extreme decimal exponent; exact
/// integers have no such limit. A rational has no signed zero, so an exact
/// zero gives +0.
pub fn fraction_to_binary_bits(value: &BigRational, width: u32) -> Result<Vec<u8>> {
    let Some((exp_bits, man_bits, bias)) = lookup(&BINARY_FORMATS, width) else {
        return Err(Error::argument(format!(
            "no IEEE binary format for width {width}"
        )));
    };
    let (exp_bits, man_bits, bias) = (u64::from(exp_bits), i64::from(man_bits), i64::from(bias));
    let nbytes = width as usize / 8;
    let neg = value.is_negative();
    let sign = if neg {
        BigUint::from(1u32) << (width - 1)
    } else {
        BigUint::zero()
    };
    if value.is_zero() {
        return Ok(to_le(&sign, nbytes));
    }
    let av = value.abs();
    let (num, den) = (av.numer(), av.denom());
    let mut e = num.bits() as i64 - den.bits() as i64;
    if cmp_pow2(num, den, e).is_lt() {
        e -= 1;
    }
    let (emax, emin) = (bias, 1 - bias);
    let q = if e < emin {
        emin - man_bits
    } else {
        e - man_bits
    };
    let mut sig = if q >= 0 {
        round_half_even(num, &(den << q))
    } else {
        round_half_even(&(num << -q), den)
    };
    let man_mask = (BigInt::from(1) << man_bits) - 1;
    let bits: BigInt = if e >= emin {
        if sig.bits() > (man_bits + 1) as u64 {
            sig >>= 1;
            e += 1;
        }
        if e > emax {
            BigInt::from(sign) | (((BigInt::from(1) << exp_bits) - 1) << man_bits)
        } else {
            BigInt::from(sign) | (BigInt::from(e + bias) << man_bits) | (sig & man_mask)
        }
    } else if sig.bits() > man_bits as u64 {
        BigInt::from(sign) | (BigInt::from(1) << man_bits)
    } else {
        BigInt::from(sign) | sig
    };
    Ok(to_le(bits.magnitude(), nbytes))
}

/// The outcome of rounding a finite decimal into a decimal format.
enum Materialised {
    Zero {
        neg: bool,
    },
    Inf {
        neg: bool,
    },
    Finite {
        neg: bool,
        c: BigUint,
        e: i64,
        be: i64,
    },
}

/// Mirror of `bvnf_dec_is_standard`: only dec256 is the library's own
/// width.
fn dec_is_standard(f: DecimalFormat) -> bool {
    f.bias == 3 * (1i64 << (f.exp_bits - 3)) + i64::from(f.max_digits) - 2
}

fn ndigits(c: &BigUint) -> u32 {
    c.to_str_radix(10).len() as u32
}

/// Rounds a finite decimal into the decimal`width` format as the library's
/// `to_ieee_decimal` does: round half to even with the combined
/// digit-count/subnormal drop, the canonical cohort, and the clamp near
/// Emax. Works on the exact literal, so it has no intermediate cap.
fn materialise_decimal(d: &Decimal, width: u32) -> Result<Materialised> {
    let f = decimal_format(width)?;
    let (Some(coeff), Some(mut e)) = (d.coefficient(), d.exponent()) else {
        return Err(Error::argument(format!(
            "cannot materialise the non-finite {d} as a decimal{width}"
        )));
    };
    let neg = d.is_signed();
    let max_digits = f.max_digits;
    let be_max = if dec_is_standard(f) {
        3i64 << (f.exp_bits - 2)
    } else {
        (1i64 << f.exp_bits) - 1
    };
    let mut c = coeff.clone();
    if c.is_zero() {
        return Ok(Materialised::Zero { neg });
    }
    let ten = BigUint::from(10u32);
    let drop = (i64::from(ndigits(&c)) - i64::from(max_digits)).max(-(e + f.bias));
    if drop > 0 {
        let factor = pow10(drop as u64);
        let dropped = &c % &factor;
        c /= &factor;
        e += drop;
        let below = pow10(drop as u64 - 1);
        let (top, rest) = dropped.div_rem(&below);
        let sticky = !rest.is_zero();
        let five = BigUint::from(5u32);
        if top > five || (top == five && (sticky || c.is_odd())) {
            c += 1u32;
            if ndigits(&c) > max_digits {
                c /= &ten;
                e += 1;
            }
        }
        if c.is_zero() {
            return Ok(Materialised::Zero { neg });
        }
    }
    while e + f.bias < be_max - 1 && (&c % &ten).is_zero() {
        c /= &ten;
        e += 1;
    }
    let mut be = (e + f.bias).max(0);
    while be >= be_max && ndigits(&c) < max_digits {
        c *= &ten;
        e -= 1;
        be -= 1;
    }
    if be >= be_max {
        return Ok(Materialised::Inf { neg });
    }
    Ok(Materialised::Finite { neg, c, e, be })
}

/// The IEEE-754 decimal`width` interchange bytes (little-endian) of a
/// finite decimal, over the full representable range; bit-identical to the
/// library's C encoder wherever the C parser reaches.
pub fn decimal_to_interchange_bits(d: &Decimal, width: u32) -> Result<Vec<u8>> {
    if !d.is_finite() {
        return Err(Error::argument(
            "decimal_to_interchange_bits needs a finite value",
        ));
    }
    let f = decimal_format(width)?;
    let total = f.total_bits;
    let one = BigUint::from(1u32);
    let res = materialise_decimal(d, width)?;
    let neg = match res {
        Materialised::Zero { neg }
        | Materialised::Inf { neg }
        | Materialised::Finite { neg, .. } => neg,
    };
    let sign = if neg {
        &one << (total - 1)
    } else {
        BigUint::zero()
    };
    let bits = match res {
        Materialised::Zero { .. } => sign,
        Materialised::Inf { .. } => {
            if dec_is_standard(f) {
                sign | (BigUint::from(0xFu32) << (total - 5))
            } else {
                sign | (((&one << f.exp_bits) - 1u32) << (total - 1 - f.exp_bits))
            }
        }
        Materialised::Finite { c, be, .. } => {
            let be = BigUint::from(be as u64);
            if c.bits() <= u64::from(f.coeff_bits) {
                sign | (be << f.coeff_bits) | c
            } else {
                // The "11" prefix form: the coefficient's implicit top bits.
                let t_l = total - 3 - f.exp_bits;
                let low = c & ((&one << f.coeff_bits) - 1u32);
                sign | (BigUint::from(3u32) << (total - 3)) | (be << t_l) | low
            }
        }
    };
    Ok(to_le(&bits, total as usize / 8))
}

/// The decimal materialised into the decimal`width` format, over the full
/// representable range (round half to even, signed zero and infinity).
pub fn decimal_stored_value(d: &Decimal, width: u32) -> Result<Decimal> {
    Ok(match materialise_decimal(d, width)? {
        Materialised::Zero { neg } => Decimal::new(neg, BigUint::zero(), 0),
        Materialised::Inf { neg } => Decimal::infinity(neg),
        Materialised::Finite { neg, c, e, .. } => Decimal::new(neg, c, e),
    })
}

/// An owned `bvn_float_t`, Python's `BvnFloat`.
pub struct BvnFloat {
    handle: FloatHandle,
}

impl std::fmt::Debug for BvnFloat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = self.to_str(10).unwrap_or_else(|_| "?".into());
        write!(f, "BvnFloat({text}, prec={})", self.handle.prec())
    }
}

/// `data` checked against the byte length of a `width`-bit format.
fn check_len(data: &[u8], width: u32) -> Result<()> {
    if data.len() > width as usize / 8 {
        return Err(Error::argument(format!(
            "{} bytes do not fit a {width}-bit format",
            data.len()
        )));
    }
    Ok(())
}

impl BvnFloat {
    /// A float of `prec` significand bits (Python's `BvnFloat(prec)`;
    /// [`WORK_PREC`] is its default), holding zero.
    pub fn new(prec: u32) -> Result<Self> {
        FloatHandle::alloc(prec)
            .map(|handle| BvnFloat { handle })
            .ok_or(Error::OutOfMemory("bvn_float_alloc"))
    }

    /// `text` parsed in `base` at `prec` bits.
    pub fn from_str(text: &str, base: u32, prec: u32) -> Result<Self> {
        let mut f = BvnFloat::new(prec)?;
        if !text.is_ascii() || !f.handle.set_str(text, base) {
            return Err(Error::argument(format!(
                "bvn_float: cannot parse {text:?} in base {base}"
            )));
        }
        Ok(f)
    }

    /// The precision in bits.
    pub fn prec(&self) -> u32 {
        self.handle.prec()
    }

    pub fn is_nan(&self) -> bool {
        self.handle.is_nan()
    }

    pub fn is_inf(&self) -> bool {
        self.handle.is_inf()
    }

    pub fn is_zero(&self) -> bool {
        self.handle.is_zero()
    }

    pub fn is_neg(&self) -> bool {
        self.handle.is_neg()
    }

    /// The value as text in `base`.
    pub fn to_str(&self, base: u32) -> Result<String> {
        self.handle
            .to_str(base)
            .ok_or_else(|| Error::argument("bvn_float_to_str: buffer too small"))
    }

    /// The nearest `f64`.
    pub fn to_double(&self) -> f64 {
        self.handle.to_double()
    }

    /// The exact value as a [`Decimal`], from its decimal text.
    pub fn to_decimal(&self) -> Result<Decimal> {
        if self.is_nan() {
            return Ok(Decimal::nan());
        }
        if self.is_inf() {
            return Ok(Decimal::infinity(self.is_neg()));
        }
        self.to_str(10)?.parse()
    }

    /// The IEEE binary interchange bytes of `width` bits.
    pub fn to_binary_bits(&self, width: u32) -> Result<Vec<u8>> {
        self.handle.to_bin(width).ok_or_else(|| {
            Error::argument(format!(
                "no IEEE binary interchange format for float width {width}"
            ))
        })
    }

    /// The float that IEEE binary interchange bytes of `width` bits encode.
    pub fn from_binary_bits(width: u32, data: &[u8]) -> Result<Self> {
        if !matches!(width, 16 | 32 | 64 | 128 | 256) {
            return Err(Error::argument(format!(
                "no IEEE binary interchange format for float width {width}"
            )));
        }
        check_len(data, width)?;
        let mut f = BvnFloat::new(WORK_PREC)?;
        if f.handle.set_bin(width, data) != Some(true) {
            return Err(Error::argument("bvn_float_from_bin: malformed bits"));
        }
        Ok(f)
    }

    /// Decimal literal `text` straight to the IEEE decimal interchange
    /// bytes of `width` bits, with one rounding.
    ///
    /// [`BvnFloat::to_decimal_bits`] encodes a float that, parsed from a
    /// decimal string, was already rounded into binary; a decimal tie is in
    /// general not exact in binary, so the decimal rounding then follows the
    /// intermediate instead of round half to even. This goes straight from
    /// the string. It also reads `ninf` (negative infinity, positive
    /// infinity with the sign bit set) and refuses what is not a decimal
    /// literal, two things the C routine does not do.
    pub fn strtodec_bits(text: &str, width: u32) -> Result<Vec<u8>> {
        if !matches!(width, 16 | 32 | 64 | 128 | 256) {
            return Err(Error::argument(format!(
                "no decimal interchange format for width {width}"
            )));
        }
        let low = text.trim().to_ascii_lowercase();
        if matches!(low.as_str(), "ninf" | "-inf" | "-infinity") {
            let mut raw = BvnFloat::strtodec_bits("inf", width)?;
            if let Some(top) = raw.last_mut() {
                *top |= 0x80;
            }
            return Ok(raw);
        }
        // The C routine answers true with zeros for what it cannot read;
        // bvn_float_from_str is the same grammar and does say no.
        let mut probe = BvnFloat::new(64)?;
        let ascii: String = text
            .chars()
            .map(|c| if c.is_ascii() { c } else { '?' })
            .collect();
        if !probe.handle.set_str(&ascii, 10) {
            return Err(Error::argument(format!(
                "bvn_float_strtodec: {text:?} is not a decimal literal"
            )));
        }
        ffi::float_strtodec(text, width).ok_or_else(|| {
            Error::argument(format!(
                "bvn_float_strtodec rejected {text:?} at width {width}"
            ))
        })
    }

    /// The IEEE decimal interchange bytes of `width` bits.
    pub fn to_decimal_bits(&self, width: u32) -> Result<Vec<u8>> {
        self.handle.to_dec(width).ok_or_else(|| {
            Error::argument(format!("no decimal interchange format for width {width}"))
        })
    }

    /// The float that IEEE decimal interchange bytes of `width` bits
    /// encode.
    pub fn from_decimal_bits(width: u32, data: &[u8]) -> Result<Self> {
        if !matches!(width, 16 | 32 | 64 | 128 | 256) {
            return Err(Error::argument(format!(
                "no decimal interchange format for width {width}"
            )));
        }
        check_len(data, width)?;
        let mut f = BvnFloat::new(WORK_PREC)?;
        if f.handle.set_dec(width, data) != Some(true) {
            return Err(Error::argument("bvn_float_from_dec: malformed bits"));
        }
        Ok(f)
    }

    /// The signed integer datum `m` with value `m / 2^frac_bits` in the
    /// `total_bits` Q-format.
    pub fn to_fixed_mantissa(&self, total_bits: u32, frac_bits: u32) -> Result<BigInt> {
        self.handle
            .to_fix(total_bits, frac_bits)
            .map(|bytes| BigInt::from_signed_bytes_le(&bytes))
            .ok_or_else(|| Error::argument(format!("no fixed-point format for width {total_bits}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_the_formats() {
        let f = BvnFloat::from_str("0.1", 10, WORK_PREC).unwrap();
        assert_eq!(f.to_binary_bits(64).unwrap(), 0.1f64.to_le_bytes());
        assert_eq!(f.to_binary_bits(32).unwrap(), 0.1f32.to_le_bytes());
        let back = BvnFloat::from_binary_bits(64, &0.1f64.to_le_bytes()).unwrap();
        assert_eq!(back.to_double(), 0.1);
        assert!(BvnFloat::from_binary_bits(48, &[0; 6]).is_err());
        assert!(BvnFloat::from_binary_bits(16, &[0; 3]).is_err());
        let fix = BvnFloat::from_str("-2.5", 10, WORK_PREC).unwrap();
        for w in [16, 32, 64, 128, 256] {
            assert_eq!(fix.to_fixed_mantissa(w, 4).unwrap(), BigInt::from(-40));
        }
        assert!(BvnFloat::from_str("x", 10, WORK_PREC).is_err());
    }

    #[test]
    fn decimal_text_is_exact() {
        let f = BvnFloat::from_str("1.5e3", 10, WORK_PREC).unwrap();
        assert_eq!(f.to_decimal().unwrap(), "1500".parse::<Decimal>().unwrap());
        let n = BvnFloat::from_str("ninf", 10, WORK_PREC).unwrap();
        assert_eq!(n.to_decimal().unwrap(), Decimal::infinity(true));
    }
}
