//! Integer range checks (`bvn_validate_uint_range` / `bvn_validate_sint_range`).
//!
//! Up to 64 bits the reference parses into a machine integer (`strtoull` /
//! `strtoll` for base 10, so their whitespace and sign rules apply). Wider
//! types compare decimal strings: a length prefilter first, then the exact
//! bound `2^w - 1` (or `2^(w-1)`). The reference renders that bound into a
//! 10000-byte scratch buffer, so any number or bound of 10000 or more decimal
//! digits is refused; this port keeps that limit.

use super::bigint::Big;
use super::{char_to_digit, is_special_number};
use alloc::vec::Vec;
use core::cmp::Ordering;

/// Size of the reference's decimal scratch buffers, terminating NUL included.
const DEC_SCRATCH: usize = 10_000;

/// A value with this many bits has more than 10000 decimal digits, so it
/// can never fit the scratch buffer. Lets conversions stop early.
const DEC_SCRATCH_BITS: u64 = 33_300;

/// Whether the unsigned literal `s` in `base` fits `width` bits. A width of 0
/// means "unspecified" and always fits; `inf`, `nan` and `ninf` always fit.
pub(crate) fn validate_uint_range(s: &[u8], width: u32, base: u32) -> bool {
    if width == 0 {
        return true;
    }
    // In bases 64 and 85 '+' and '-' are digits, not signs.
    let uses_sign = base != 64 && base != 85;
    let c0 = s.first().copied().unwrap_or(0);
    if !c0.is_ascii_digit() && c0 != b'+' {
        if is_special_number(s) {
            return true;
        }
        if uses_sign && c0 == b'-' {
            return false;
        }
    }
    let p = match s {
        [b'+', rest @ ..] if uses_sign => rest,
        _ => s,
    };
    if width <= 64 {
        let max = if width >= 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        if base == 10 {
            return strtoull(p).is_some_and(|v| v <= max);
        }
        return parse_u64(p, base).is_some_and(|v| v <= max);
    }
    let max_digits = dec_digits_2pow(width);
    let dec;
    let digits = if base == 10 {
        p
    } else {
        match digits_to_dec(p, base) {
            Some(d) => {
                dec = d;
                &dec
            }
            None => return false,
        }
    };
    if let Some(fits) = dec_digit_prefilter(digits, max_digits) {
        return fits;
    }
    pow2m1_dec(width).is_some_and(|max| cmp_dec(digits, &max) != Ordering::Greater)
}

/// Whether the signed literal `s` in `base` fits `width` bits
/// (`[-2^(w-1), 2^(w-1) - 1]`). Same exemptions as [`validate_uint_range`].
pub(crate) fn validate_sint_range(s: &[u8], width: u32, base: u32) -> bool {
    if width == 0 {
        return true;
    }
    let c0 = s.first().copied().unwrap_or(0);
    if !c0.is_ascii_digit() && c0 != b'-' && c0 != b'+' && is_special_number(s) {
        return true;
    }
    let neg = c0 == b'-';
    let abs = match s {
        [b'-' | b'+', rest @ ..] => rest,
        _ => s,
    };
    if width <= 64 {
        if base == 10 {
            let (lo, hi) = if width >= 64 {
                (i64::MIN, i64::MAX)
            } else {
                (-(1i64 << (width - 1)), (1i64 << (width - 1)) - 1)
            };
            return strtoll(s).is_some_and(|v| v >= lo && v <= hi);
        }
        let mag_max = match (neg, width >= 64) {
            (true, true) => 1u64 << 63,
            (true, false) => 1u64 << (width - 1),
            (false, true) => i64::MAX as u64,
            (false, false) => (1u64 << (width - 1)) - 1,
        };
        return parse_u64(abs, base).is_some_and(|v| v <= mag_max);
    }
    let max_digits = dec_digits_2pow(width - 1);
    let dec;
    let digits = if base == 10 {
        abs
    } else {
        match digits_to_dec(abs, base) {
            Some(d) => {
                dec = d;
                &dec
            }
            None => return false,
        }
    };
    if let Some(fits) = dec_digit_prefilter(digits, max_digits) {
        return fits;
    }
    // The bound 2^(w-1) - 1 must fit the scratch buffer even when the
    // negative bound 2^(w-1) is the one compared.
    if pow2m1_dec(width - 1).is_none() {
        return false;
    }
    let mut bound = Big::pow2(u64::from(width - 1));
    if !neg {
        bound.dec();
    }
    cmp_dec(digits, &bound.to_decimal()) != Ordering::Greater
}

/// Horner parse of a digit string into a `u64` (empty is 0); `None` on a
/// non-digit or overflow.
fn parse_u64(s: &[u8], base: u32) -> Option<u64> {
    s.iter().try_fold(0u64, |v, &c| {
        let d = char_to_digit(c, base);
        if d >= base {
            return None;
        }
        v.checked_mul(u64::from(base))?.checked_add(u64::from(d))
    })
}

/// The parts of a C `strtoull`/`strtoll` scan: sign, magnitude (`None` on
/// overflow) and whether the whole input was consumed.
struct CScan {
    neg: bool,
    mag: Option<u64>,
    complete: bool,
}

/// Scans like glibc's base-10 `strto*`: leading white space, optional sign,
/// digits. Without digits nothing is consumed, so only the empty string
/// counts as complete.
fn c_scan(s: &[u8]) -> CScan {
    let mut i = s
        .iter()
        .take_while(|c| matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'))
        .count();
    let mut neg = false;
    match s.get(i) {
        Some(b'+') => i += 1,
        Some(b'-') => {
            neg = true;
            i += 1;
        }
        _ => {}
    }
    let digits = s.get(i..).unwrap_or_default();
    let n = digits.iter().take_while(|c| c.is_ascii_digit()).count();
    let mag = digits.iter().take(n).try_fold(0u64, |v, &c| {
        v.checked_mul(10)?.checked_add(u64::from(c - b'0'))
    });
    let complete = if n == 0 {
        s.is_empty()
    } else {
        i + n == s.len()
    };
    CScan { neg, mag, complete }
}

/// `strtoull(s, &end, 10)` accepted only when `*end == 0 && errno != ERANGE`.
/// A '-' negates modulo 2^64, as C does.
fn strtoull(s: &[u8]) -> Option<u64> {
    let scan = c_scan(s);
    if !scan.complete {
        return None;
    }
    let mag = scan.mag?;
    Some(if scan.neg { mag.wrapping_neg() } else { mag })
}

/// `strtoll(s, &end, 10)` with the same acceptance rule as [`strtoull`].
fn strtoll(s: &[u8]) -> Option<i64> {
    let scan = c_scan(s);
    if !scan.complete {
        return None;
    }
    let mag = scan.mag?;
    if scan.neg {
        0i64.checked_sub_unsigned(mag)
    } else {
        i64::try_from(mag).ok()
    }
}

/// Approximate decimal digit count of a `w`-bit value, computed in `double`
/// exactly like the reference so the prefilter boundaries match.
fn dec_digits_2pow(w: u32) -> u32 {
    if w == 0 {
        return 1;
    }
    (f64::from(w) * core::f64::consts::LOG10_2) as u32 + 1
}

/// Length-based decision: `Some(true)` when clearly shorter than the bound,
/// `Some(false)` when clearly longer, `None` when an exact compare is needed.
/// The input is not checked for digits (the base-10 path never was).
fn dec_digit_prefilter(d: &[u8], max_digits: u32) -> Option<bool> {
    let n = strip_zeros(d).len() as u64;
    let max = u64::from(max_digits);
    if n + 1 < max {
        Some(true)
    } else if n > max + 1 {
        Some(false)
    } else {
        None
    }
}

/// Drops leading '0's but keeps the last character.
fn strip_zeros(d: &[u8]) -> &[u8] {
    let lead = d
        .iter()
        .take(d.len().saturating_sub(1))
        .take_while(|&&c| c == b'0')
        .count();
    d.get(lead..).unwrap_or_default()
}

/// `bvn_cmp_dec`: longer wins, equal lengths compare bytewise (`strcmp`).
fn cmp_dec(a: &[u8], b: &[u8]) -> Ordering {
    let (a, b) = (strip_zeros(a), strip_zeros(b));
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

/// `2^n - 1` in decimal, or `None` when it has too many digits for the
/// reference's scratch buffer.
fn pow2m1_dec(n: u32) -> Option<Vec<u8>> {
    if u64::from(n) > DEC_SCRATCH_BITS {
        return None;
    }
    let mut b = Big::pow2(u64::from(n));
    b.dec();
    let d = b.to_decimal();
    (d.len() < DEC_SCRATCH).then_some(d)
}

/// Converts a digit string in `base` to decimal (empty is "0"). `None` on a
/// non-digit or a value with 10000 or more decimal digits.
fn digits_to_dec(s: &[u8], base: u32) -> Option<Vec<u8>> {
    // Feed several digits per big-integer step: base^k must fit a u32.
    let mut chunk_max = 1u32;
    let mut k = 0;
    while k < 16 {
        match chunk_max.checked_mul(base) {
            Some(m) if base > 1 => chunk_max = m,
            _ => break,
        }
        k += 1;
    }
    let k = k.max(1);
    let mut value = Big::zero();
    for chunk in s.chunks(k) {
        let mut mul = 1u32;
        let mut add = 0u32;
        for &c in chunk {
            let d = char_to_digit(c, base);
            if d >= base {
                return None;
            }
            mul = mul.wrapping_mul(base);
            add = add.wrapping_mul(base).wrapping_add(d);
        }
        value.mul_add_small(mul, add);
        // The value never shrinks, so once too big it stays too big.
        if value.bit_len() > DEC_SCRATCH_BITS {
            return None;
        }
    }
    let d = value.to_decimal();
    (d.len() < DEC_SCRATCH).then_some(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec;

    fn u(s: &str, w: u32, b: u32) -> bool {
        validate_uint_range(s.as_bytes(), w, b)
    }
    fn i(s: &str, w: u32, b: u32) -> bool {
        validate_sint_range(s.as_bytes(), w, b)
    }

    #[test]
    fn uint_narrow() {
        assert!(u("255", 8, 10));
        assert!(!u("256", 8, 10));
        assert!(u("+255", 8, 10));
        assert!(!u("-0", 8, 10));
        assert!(u("0", 1, 10) && u("1", 1, 10) && !u("2", 1, 10));
        assert!(u("18446744073709551615", 64, 10));
        assert!(!u("18446744073709551616", 64, 10));
        assert!(u("ff", 8, 16) && !u("100", 8, 16));
        assert!(u("FF", 8, 16));
        assert!(u("11111111", 8, 2) && !u("100000000", 8, 2));
        assert!(u("73", 8, 36) && !u("74", 8, 36));
        assert!(u("47", 8, 62) && !u("48", 8, 62));
        assert!(u("D/", 8, 64) && !u("EA", 8, 64));
        assert!(!u("x", 8, 16));
        assert!(u("inf", 8, 10) && u("nan", 8, 16) && u("ninf", 8, 85));
        assert!(u("123", 0, 10));
    }

    #[test]
    fn uint_c_quirks() {
        // The empty string and a lone '+' parse as zero.
        assert!(u("", 8, 10));
        assert!(u("+", 8, 10));
        // strtoull takes a second sign and wraps a negative modulo 2^64.
        assert!(u("++5", 8, 10));
        assert!(u("+-5", 64, 10));
        assert!(!u("+-5", 63, 10));
        // ...and skips leading white space.
        assert!(u(" 5", 8, 10));
        assert!(!u("5 ", 8, 10));
        // Bases 64/85 take no sign: '-' is digit 12 in Ascii85.
        assert!(u("-", 8, 85));
        assert!(!u("-", 8, 64));
    }

    #[test]
    fn uint_wide() {
        let max128 = "340282366920938463463374607431768211455";
        assert!(u(max128, 128, 10));
        assert!(!u("340282366920938463463374607431768211456", 128, 10));
        assert!(u(&["000", max128].concat(), 128, 10));
        assert!(u(&"f".repeat(32), 128, 16));
        assert!(!u(&["1", &"0".repeat(32)].concat(), 128, 16));
        assert!(u(&"1".repeat(128), 128, 2));
        assert!(!u(&"1".repeat(129), 128, 2));
        // Short base-10 strings pass the prefilter unchecked.
        assert!(u("12x", 128, 10));
        assert!(!u("1x", 128, 16));
        // Huge widths: 2^32768 - 1 has 9865 digits and still fits scratch.
        let big = Big::pow2(32768);
        let mut m = big.clone();
        m.dec();
        let m = String::from_utf8(m.to_decimal()).unwrap();
        let b = String::from_utf8(big.to_decimal()).unwrap();
        assert!(u(&m, 32768, 10));
        assert!(!u(&b, 32768, 10));
        assert!(u(&"1".repeat(32768), 32768, 2));
        assert!(!u(&["1", &"0".repeat(32768)].concat(), 32768, 2));
    }

    #[test]
    fn scratch_limit() {
        // 2^33216 has 10000 digits: the bound no longer fits the scratch, so
        // boundary-length numbers are refused while short ones still pass.
        assert!(pow2m1_dec(33215).is_some());
        assert!(pow2m1_dec(33216).is_none());
        assert!(u("1", 40000, 10));
        let len = dec_digits_2pow(40000) as usize;
        assert!(!u(&"1".repeat(len), 40000, 10));
        assert!(!u(&"1".repeat(len), 40000, 16));
        // A converted number with 10000 digits is refused outright.
        assert!(!u(&"1".repeat(40000), 40000, 2));
        assert!(digits_to_dec(&vec![b'1'; 33215], 2).is_some());
    }

    #[test]
    fn sint_narrow() {
        assert!(i("127", 8, 10) && !i("128", 8, 10));
        assert!(i("-128", 8, 10) && !i("-129", 8, 10));
        assert!(i("+127", 8, 10));
        assert!(i("-1", 1, 10) && i("0", 1, 10) && !i("1", 1, 10));
        assert!(i("-9223372036854775808", 64, 10));
        assert!(!i("-9223372036854775809", 64, 10));
        assert!(i("9223372036854775807", 64, 10));
        assert!(!i("9223372036854775808", 64, 10));
        assert!(i("-80", 8, 16) && !i("-81", 8, 16) && i("7f", 8, 16) && !i("80", 8, 16));
        assert!(i("-8000000000000000", 64, 16) && !i("8000000000000000", 64, 16));
        assert!(i("ninf", 8, 10) && !i("-inf", 8, 10));
        // strtoll needs the whole string; "+-5" stops after the '+'.
        assert!(!i("+-5", 8, 10) && !i("--5", 8, 10));
        assert!(i("", 8, 10) && !i("-", 8, 10) && i("-", 8, 16));
        assert!(i(" -5", 8, 10));
    }

    #[test]
    fn sint_wide() {
        let max = "170141183460469231731687303715884105727";
        let min = "-170141183460469231731687303715884105728";
        assert!(i(max, 128, 10) && i(min, 128, 10));
        assert!(!i("170141183460469231731687303715884105728", 128, 10));
        assert!(!i("-170141183460469231731687303715884105729", 128, 10));
        assert!(i(&["-8", &"0".repeat(31)].concat(), 128, 16));
        assert!(!i(&["8", &"0".repeat(31)].concat(), 128, 16));
        assert!(!i(&["-8", &"0".repeat(30), "1"].concat(), 128, 16));
        assert!(i(&["-1", &"0".repeat(127)].concat(), 128, 2));
        assert!(!i(&["1", &"0".repeat(127)].concat(), 128, 2));
    }

    #[test]
    fn c_scan_rules() {
        assert_eq!(strtoull(b"-1"), Some(u64::MAX));
        assert_eq!(strtoull(b"99999999999999999999"), None);
        assert_eq!(strtoll(b"-9223372036854775808"), Some(i64::MIN));
        assert_eq!(strtoll(b"\t\n+7"), Some(7));
        assert_eq!(strtoll(b" "), None);
    }
}
