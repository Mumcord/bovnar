//! Fixed-point range probe (`bvn_float_str_fits_fix`).
//!
//! The reference does not round the literal straight to the Q format. It
//! first parses it into an exact rational, rounds that to a binary float of
//! `width + q + 64` bits (round to nearest, ties to even), and only then
//! rounds the float to a multiple of `2^-q`. The double rounding is visible
//! right at the range limits, so it is reproduced here step by step, along
//! with the reference's 32768-bit big-integer budget: whenever the C code
//! runs out of that budget it gives up and reports "fits".

use super::bigint::Big;
use core::cmp::Ordering;

/// The reference's big-integer budget (`BVN_INT_MAX_BITS`); also the cap on
/// the float precision (`BVN_FLOAT_MAX_PREC`).
const MAX_BITS: u64 = 32768;

/// Whether the decimal (base 10) or hex-float (base 16) literal `s` fits a
/// signed Q-format of `width` total bits with `q` fraction bits.
///
/// Anything the reference cannot turn into a regular float counts as
/// fitting: other bases, malformed text, `nan`/`inf`, zero, and values
/// beyond the big-integer budget (so `1e10000` "fits" a `float_fix:16,q8`).
pub(crate) fn float_str_fits_fix(s: &[u8], base: u32, width: u32, q: u32) -> bool {
    let prec = width.wrapping_add(q).wrapping_add(64).min(MAX_BITS as u32);
    if prec == 0 {
        return true;
    }
    let Some(r) = parse_rational(s, base) else {
        return true;
    };
    let (mant, exp) = round_to_precision(&r.num, &r.den, u64::from(prec));
    !fix_overflows(&mant, exp, i64::from(prec), q, width, r.neg)
}

/// An exact, non-zero rational `num / den` with a sign.
struct Rational {
    neg: bool,
    num: Big,
    den: Big,
}

/// Digit value for the float parser: letters are case-insensitive.
fn float_digit(c: u8, base: u32) -> u32 {
    let d = match c {
        b'0'..=b'9' => u32::from(c - b'0'),
        b'a'..=b'z' => 10 + u32::from(c - b'a'),
        b'A'..=b'Z' => 10 + u32::from(c - b'A'),
        _ => return base,
    };
    if d < base {
        d
    } else {
        base
    }
}

fn over_budget(b: &Big) -> bool {
    b.bit_len() > MAX_BITS
}

/// `bvnf_parse_rational` restricted to what the range probe needs: the exact
/// value of a regular literal, or `None` for every outcome the probe treats
/// as "fits" (malformed, nan, inf, zero, underflow, overflow).
fn parse_rational(s: &[u8], base: u32) -> Option<Rational> {
    if base != 10 && base != 16 {
        return None;
    }
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let mut i = s
        .iter()
        .take_while(|c| matches!(c, b' ' | b'\t' | b'\n' | b'\r'))
        .count();
    let mut neg = false;
    match at(i) {
        b'+' => i += 1,
        b'-' => {
            neg = true;
            i += 1;
        }
        _ => {}
    }
    if base == 16
        && at(i) == b'0'
        && matches!(at(i + 1), b'x' | b'X')
        && (float_digit(at(i + 2), 16) < 16 || at(i + 2) == b'.')
    {
        i += 2;
    }
    let rest = s.get(i..).unwrap_or_default();
    if rest.eq_ignore_ascii_case(b"nan") || rest.eq_ignore_ascii_case(b"inf") || rest == b"ninf" {
        return None;
    }
    // Mantissa digits, fed in chunks like the reference. Running past the
    // budget ends in "fits" whatever follows, so stop right there.
    let chunk = if base == 10 { 9 } else { 7 };
    let mut num = Big::zero();
    let (mut acc, mut acc_digits) = (0u32, 0u32);
    let mut frac_digits: i64 = 0;
    let (mut has_int, mut has_frac) = (false, false);
    let mut in_frac = false;
    loop {
        let c = at(i);
        let d = float_digit(c, base);
        if d < base {
            if in_frac {
                has_frac = true;
                frac_digits += 1;
            } else {
                has_int = true;
            }
            acc = acc * base + d;
            acc_digits += 1;
            if acc_digits >= chunk {
                num.mul_add_small(base.pow(acc_digits), acc);
                (acc, acc_digits) = (0, 0);
                if over_budget(&num) {
                    return None;
                }
            }
            i += 1;
        } else if c == b'.' && !in_frac {
            in_frac = true;
            i += 1;
        } else {
            break;
        }
    }
    if acc_digits > 0 {
        num.mul_add_small(base.pow(acc_digits), acc);
        if over_budget(&num) {
            return None;
        }
    }
    if !has_int && !has_frac {
        return None;
    }
    let mut exp_val: i64 = 0;
    let exp_marker = if base == 10 {
        matches!(at(i), b'e' | b'E')
    } else {
        matches!(at(i), b'p' | b'P')
    };
    if exp_marker {
        i += 1;
        let mut eneg = false;
        match at(i) {
            b'+' => i += 1,
            b'-' => {
                eneg = true;
                i += 1;
            }
            _ => {}
        }
        let n = s
            .get(i..)
            .unwrap_or_default()
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        if n == 0 {
            return None;
        }
        // An exponent too large for a C long ends in zero, underflow or
        // infinity: all "fit".
        let eabs = s.get(i..i + n)?.iter().try_fold(0i64, |v, &c| {
            v.checked_mul(10)?.checked_add(i64::from(c - b'0'))
        })?;
        i += n;
        exp_val = if eneg { -eabs } else { eabs };
    }
    if i != s.len() || num.is_zero() {
        return None;
    }
    let mut den = Big::from_u64(1);
    if base == 16 {
        // Each hex fraction digit is four binary places.
        let net = exp_val.checked_sub(frac_digits.checked_mul(4)?)?;
        let target = if net >= 0 { &mut num } else { &mut den };
        let bits = net.unsigned_abs();
        if bits > i32::MAX as u64 || target.bit_len() + bits > MAX_BITS {
            return None;
        }
        target.shl(bits);
    } else {
        let net = exp_val.checked_sub(frac_digits)?;
        let target = if net >= 0 { &mut num } else { &mut den };
        // Multiplying by ten repeatedly only grows the value, so going over
        // the budget part-way means the full product would too.
        let mut left = net.unsigned_abs();
        while left > 0 {
            let k = left.min(9) as u32;
            target.mul_add_small(10u32.pow(k), 0);
            if over_budget(target) {
                return None;
            }
            left -= u64::from(k);
        }
    }
    Some(Rational { neg, num, den })
}

/// Rounds `num / den` to `prec` significant bits, ties to even
/// (`bvnf_rational_to_float`). Returns the mantissa, which has exactly
/// `prec` bits, and the exponent `e` with value `mant * 2^(e - prec)`.
fn round_to_precision(num: &Big, den: &Big, prec: u64) -> (Big, i64) {
    let nb = num.bit_len() as i64;
    let db = den.bit_len() as i64;
    // 2^e <= num/den < 2^(e+1)
    let mut e = nb - db;
    let below = if e >= 0 {
        let mut t = den.clone();
        t.shl(e.unsigned_abs());
        t.cmp(num) == Ordering::Greater
    } else {
        let mut t = num.clone();
        t.shl(e.unsigned_abs());
        den.cmp(&t) == Ordering::Greater
    };
    if below {
        e -= 1;
    }
    // prec mantissa bits plus guard and round bits.
    let shift = prec as i64 + 1 - e;
    let (mut quot, rem) = if shift >= 0 {
        let mut n = num.clone();
        n.shl(shift.unsigned_abs());
        n.div_rem(den)
    } else {
        let mut d = den.clone();
        d.shl(shift.unsigned_abs());
        num.div_rem(&d)
    };
    let sticky = !rem.is_zero();
    let round_bit = quot.bit(0);
    let guard = quot.bit(1);
    let lsb = quot.bit(2);
    quot.shr(2);
    let mut exp = e + 1;
    if guard && (round_bit || sticky || lsb) {
        quot.add_small(1);
        if quot.bit_len() > prec {
            quot.shr(1);
            exp += 1;
        }
    }
    (quot, exp)
}

/// Whether `±mant * 2^(exp - prec)` scaled by `2^q` and rounded to an
/// integer (ties to even) falls outside the signed `width`-bit range
/// (`bvnf_to_fix_core` in probe mode).
fn fix_overflows(mant: &Big, exp: i64, prec: i64, q: u32, width: u32, neg: bool) -> bool {
    let mut m = mant.clone();
    let sh = exp - prec + i64::from(q);
    if sh >= 0 {
        let bits = sh.unsigned_abs();
        // Past the budget the reference bails out without flagging overflow.
        if bits > i32::MAX as u64 || m.bit_len() + bits > MAX_BITS {
            return false;
        }
        m.shl(bits);
    } else {
        let d = sh.unsigned_abs();
        if d > m.bit_len() {
            m = Big::zero();
        } else {
            let guard = m.bit(d - 1);
            let sticky = m.any_bit_below(d - 1);
            m.shr(d);
            if guard && (sticky || m.bit(0)) {
                m.add_small(1);
            }
        }
    }
    // The reference takes the width as a C int.
    let total = i64::from(width as i32);
    let bl = m.bit_len() as i64;
    if !neg {
        bl >= total
    } else if bl != total {
        bl > total
    } else {
        // Only -2^(width-1) itself fits at full length.
        m.any_bit_below((total - 1).unsigned_abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fits(s: &str, w: u32, q: u32) -> bool {
        float_str_fits_fix(s.as_bytes(), 10, w, q)
    }

    #[test]
    fn q8_limits() {
        // float_fix:16,q8 holds [-128, 127.99609375] in steps of 1/256.
        assert!(fits("127.99609375", 16, 8));
        assert!(!fits("128", 16, 8));
        assert!(fits("-128", 16, 8));
        assert!(!fits("-128.00390625", 16, 8));
        // Rounding to the step: below half a step under 128 rounds down.
        assert!(fits("127.998046874", 16, 8));
        // Exactly half a step: ties to even rounds up to 128 (32768 is even).
        assert!(!fits("127.998046875", 16, 8));
        // -128.001953125 is a tie between -32768 and -32769: even wins, fits.
        assert!(fits("-128.001953125", 16, 8));
        assert!(!fits("-128.0019531251", 16, 8));
        assert!(fits("0.001", 16, 8));
        assert!(fits("-0.001", 16, 8));
    }

    #[test]
    fn double_rounding() {
        // 127.998046875 - 2^-90 is below the tie, but rounding to 88 bits
        // first lands on the tie, which then rounds up out of range.
        let just_below = "127.99804687499999999999999999919";
        assert!(!fits(just_below, 16, 8));
        // Far enough below the tie it survives the first rounding.
        assert!(fits("127.9980468749999999999999", 16, 8));
    }

    #[test]
    fn q0_and_wide() {
        assert!(fits("32767", 16, 0) && !fits("32768", 16, 0));
        assert!(fits("-32768", 16, 0) && !fits("-32769", 16, 0));
        assert!(!fits("32767.5", 16, 0));
        assert!(fits("32767.4999", 16, 0));
        assert!(fits("-32768.5", 16, 0));
        assert!(fits("1e5", 128, 8) && !fits("1e40", 128, 8));
        assert!(fits("1.0e-30", 16, 8));
    }

    #[test]
    fn budget_and_specials() {
        assert!(fits("nan", 16, 8) && fits("inf", 16, 8) && fits("ninf", 16, 8));
        assert!(fits("NaN", 16, 8));
        assert!(fits("0", 16, 8) && fits("-0.0e5", 16, 8));
        assert!(fits("abc", 16, 8) && fits("", 16, 8) && fits("1.2.3", 16, 8));
        // Beyond the big-integer budget the reference gives up: "fits".
        assert!(fits("1e10000", 16, 8));
        assert!(fits("1e99999999999999999999", 16, 8));
        assert!(fits(&"9".repeat(10000), 16, 8));
        // 1e9863 parses, but shifting its mantissa by q leaves the budget.
        assert!(fits("1e9863", 16, 8));
        assert!(!fits("1e9000", 16, 8));
        // Other bases are never parsed.
        assert!(float_str_fits_fix(b"99999", 2, 16, 8));
    }

    #[test]
    fn hex_float() {
        let h = |s: &str| float_str_fits_fix(s.as_bytes(), 16, 16, 8);
        assert!(h("7f.ff") && !h("80"));
        assert!(h("0x7f.ffp0") && !h("0x1p7"));
        assert!(h("-0x1p7") && !h("-0x1.01p7"));
    }
}
