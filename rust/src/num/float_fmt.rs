//! Text renderings of an `f64` that the C reference produces with `printf`
//! and with its own shortest-digits formatter.
//!
//! `fmt_g` is C's `%.{prec}g`; `shortest_bin53` is `bvn_format_double` for a
//! `<float>` of unspecified width (the shortest decimal that reads back at
//! 53-bit precision); `parse_hex_float` reads a base-16 float literal.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cmp::Ordering;

use super::bigint::Big;

/// C's `%.{prec}g` for a finite `v` (`prec` 0 counts as 1): the value rounded
/// to `prec` significant digits, in fixed notation when the decimal exponent
/// is in `-4..prec`, otherwise scientific with at least two exponent digits;
/// trailing zeros and a bare `.` are dropped.
pub(crate) fn fmt_g(v: f64, prec: usize) -> String {
    let p = prec.max(1);
    let sci = format!("{:.*e}", p - 1, v);
    let (mantissa, exp) = split_exp(&sci);
    let p_i = i64::try_from(p).unwrap_or(i64::MAX);
    if exp < -4 || exp >= p_i {
        let mut out = String::from(strip_fraction_zeros(mantissa));
        out.push('e');
        out.push(if exp < 0 { '-' } else { '+' });
        let abs = exp.unsigned_abs();
        if abs < 10 {
            out.push('0');
        }
        out.push_str(&abs.to_string());
        out
    } else {
        // exp is in -4..p here, so the precision is non-negative.
        let decimals = usize::try_from(p_i - 1 - exp).unwrap_or(0);
        let fixed = format!("{v:.decimals$}");
        String::from(strip_fraction_zeros(&fixed))
    }
}

/// Splits Rust's `{:e}` output into mantissa and exponent.
fn split_exp(s: &str) -> (&str, i64) {
    match s.split_once('e') {
        Some((m, e)) => (m, e.parse().unwrap_or(0)),
        None => (s, 0),
    }
}

/// Drops trailing zeros after a `.`, and the `.` itself when nothing is left.
fn strip_fraction_zeros(s: &str) -> &str {
    if !s.contains('.') {
        return s;
    }
    s.trim_end_matches('0').trim_end_matches('.')
}

/// `bvn_format_double` at 53-bit precision (a `<float>` of unspecified or
/// 64-bit width): `0.0`/`-0.0`, `nan`, `inf`, `ninf`, otherwise the shortest
/// digits that read back, as `d.ddde+N`.
///
/// A port of `bvnf_to_str_dec` rather than a shortest-digits algorithm of
/// its own: the reference extracts 21 truncated digits, searches the
/// shortest length that reads back at 53 bits with no exponent floor, and
/// picks the nearer of the two neighbours at that length judging from those
/// 21 digits only. Where 17 digits are needed that choice can differ from
/// the truly nearest one, and subnormals get more digits than IEEE needs.
pub(crate) fn shortest_bin53(v: f64) -> String {
    if v.is_nan() {
        return "nan".to_string();
    }
    if v.is_infinite() {
        return if v < 0.0 { "ninf" } else { "inf" }.to_string();
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0" } else { "0.0" }.to_string();
    }
    let (m, x) = normalise(v.abs());
    let (digits, exp10) = shortest_digits(m, x);
    let mut out = String::new();
    if v < 0.0 {
        out.push('-');
    }
    let mut it = digits.iter();
    if let Some(&d) = it.next() {
        out.push(char::from(b'0' + d));
    }
    if digits.len() > 1 {
        out.push('.');
        out.extend(it.map(|&d| char::from(b'0' + d)));
    }
    out.push('e');
    out.push(if exp10 < 0 { '-' } else { '+' });
    out.push_str(&exp10.unsigned_abs().to_string());
    out
}

/// Digits the reference extracts for a 53-bit value (`ndigits`).
const NDIGITS: usize = 21;

/// The digit values and the decimal exponent of the first digit for
/// `m * 2^x` (`m` in `[2^52, 2^53)`).
fn shortest_digits(m: u64, x: i64) -> (Vec<u8>, i64) {
    // The exponent estimate of `bvnf_to_str_dec`: ceil((exp2 - 1) * log10 2)
    // with log10 2 taken as 0.30103.
    let ep = x + 52;
    let mut e10 = if ep >= 0 {
        (ep * 30103 + 99_999) / 100_000
    } else {
        -((-ep) * 30103 / 100_000)
    };
    let k = i64::try_from(NDIGITS).unwrap_or(21) - 1 - e10;
    // floor(m * 2^x * 10^k), of which the last NDIGITS digits are kept.
    let mut num = Big::from_u64(m);
    let mut den = Big::from_u64(1);
    mul_pow10(if k >= 0 { &mut num } else { &mut den }, k.unsigned_abs());
    if x >= 0 {
        num.shl(x.unsigned_abs());
    } else {
        den.shl(x.unsigned_abs());
    }
    let dec = num.div_rem(&den).0.to_decimal();
    let mut digs = [0u8; NDIGITS];
    for (slot, &c) in digs.iter_mut().rev().zip(dec.iter().rev()) {
        *slot = c.wrapping_sub(b'0');
    }
    let lead = digs
        .iter()
        .take(NDIGITS - 1)
        .take_while(|&&d| d == 0)
        .count();
    digs.rotate_left(lead);
    for d in digs.iter_mut().skip(NDIGITS - lead) {
        *d = 0;
    }
    e10 -= i64::try_from(lead).unwrap_or(0);
    let n = digs.iter().rposition(|&d| d != 0).map_or(1, |i| i + 1);
    let digs = digs.get(..n).unwrap_or_default();
    shorten(digs, e10, m, x)
}

/// The first `k` digits truncated (`up == false`) or rounded up by one unit
/// in the last place, without trailing zeros, and whether the round-up
/// carried into a new leading digit (`bvnf_take_k`).
fn take_k(digs: &[u8], k: usize, up: bool) -> (Vec<u8>, i64) {
    let mut v: Vec<u8> = digs.iter().take(k).copied().collect();
    let mut shift = 0;
    if up {
        let mut j = v.len();
        loop {
            if j == 0 {
                // Every digit carried: the result is 1 at the next decade.
                v.clear();
                v.push(1);
                shift = 1;
                break;
            }
            j -= 1;
            if let Some(d) = v.get_mut(j) {
                *d += 1;
                if *d < 10 {
                    break;
                }
                *d = 0;
            }
        }
    }
    while v.len() > 1 && v.last() == Some(&0) {
        v.pop();
    }
    (v, shift)
}

/// `bvnf_shorten_digits`: binary search for the fewest digits whose
/// truncation or round-up reads back, then the nearer of the two.
fn shorten(digs: &[u8], e10: i64, m: u64, x: i64) -> (Vec<u8>, i64) {
    let n = digs.len();
    if n <= 1 {
        return (digs.to_vec(), e10);
    }
    let feasible = |d: &[u8], shift: i64| reads_back(d, e10 + shift, m, x);
    let (mut lo, mut hi) = (1, n);
    while lo < hi {
        let k = lo + (hi - lo) / 2;
        let (f, fs) = take_k(digs, k, false);
        let (c, cs) = take_k(digs, k, true);
        if feasible(&f, fs) || feasible(&c, cs) {
            hi = k;
        } else {
            lo = k + 1;
        }
    }
    let k = lo;
    let (f, fs) = take_k(digs, k, false);
    let (c, cs) = take_k(digs, k, true);
    let next = u32::from(digs.get(k).copied().unwrap_or(0));
    let sticky = digs.iter().skip(k + 1).any(|&d| d != 0);
    let last = digs.get(k.wrapping_sub(1)).copied().unwrap_or(0);
    let near_is_ceil = 2 * next > 10 || (2 * next == 10 && (sticky || last % 2 == 1));
    let near_ok = if near_is_ceil {
        feasible(&c, cs)
    } else {
        feasible(&f, fs)
    };
    let use_ceil = if near_is_ceil { near_ok } else { !near_ok };
    if use_ceil {
        (c, e10 + cs)
    } else {
        (f, e10 + fs)
    }
}

/// `v = m * 2^x` with `m` in `[2^52, 2^53)`.
fn normalise(v: f64) -> (u64, i64) {
    let bits = v.to_bits();
    let field = i64::try_from((bits >> 52) & 0x7ff).unwrap_or(0);
    let frac = bits & ((1u64 << 52) - 1);
    let (mut m, mut x) = if field == 0 {
        (frac, -1074)
    } else {
        (frac | (1u64 << 52), field - 1075)
    };
    if m == 0 {
        return (0, 0);
    }
    while m < (1u64 << 52) {
        m <<= 1;
        x -= 1;
    }
    (m, x)
}

fn mul_pow10(b: &mut Big, n: u64) {
    for _ in 0..n {
        b.mul_add_small(10, 0);
    }
}

/// `digits * 10^(e10 - len + 1)` (the digits read as `d.ddd * 10^e10`)
/// against `scaled * 2^(x-2)`.
fn compare_scaled(digits: &[u8], e10: i64, scaled: u64, x: i64) -> Ordering {
    let mut lhs = Big::zero();
    for &d in digits {
        lhs.mul_add_small(10, u32::from(d));
    }
    let mut rhs = Big::from_u64(scaled);
    let p10 = e10 - (i64::try_from(digits.len()).unwrap_or(1) - 1);
    mul_pow10(
        if p10 >= 0 { &mut lhs } else { &mut rhs },
        p10.unsigned_abs(),
    );
    let shift = x - 2;
    if shift >= 0 {
        rhs.shl(shift.unsigned_abs());
    } else {
        lhs.shl(shift.unsigned_abs());
    }
    lhs.cmp(&rhs)
}

/// Whether the decimal `digits` (first digit at `10^e10`) rounds to nearest,
/// ties to even, to `m * 2^x` at 53 bits of precision with an unbounded
/// exponent (`bvnf_rt_candidate`).
fn reads_back(digits: &[u8], e10: i64, m: u64, x: i64) -> bool {
    // Half the gap to each neighbour, in units of 2^(x-2); the neighbour
    // below a power of two is half as far away.
    let below = if m == 1u64 << 52 { 1 } else { 2 };
    let lo = m.saturating_mul(4).saturating_sub(below);
    let hi = m.saturating_mul(4).saturating_add(2);
    let inclusive = m % 2 == 0;
    let lo_cmp = compare_scaled(digits, e10, lo, x);
    let hi_cmp = compare_scaled(digits, e10, hi, x);
    if inclusive {
        lo_cmp != Ordering::Less && hi_cmp != Ordering::Greater
    } else {
        lo_cmp == Ordering::Greater && hi_cmp == Ordering::Less
    }
}

/// A hex-float literal (`[-+][0x]h.hhh[p[-+]d]`) correctly rounded to the
/// nearest `f64` (`bvn_parse_double_in_base` with base 16). `None` when it is
/// not one.
pub(crate) fn parse_hex_float(s: &str) -> Option<f64> {
    let (neg, body) = match s.as_bytes().first() {
        Some(b'-') => (true, s.get(1..)?),
        Some(b'+') => (false, s.get(1..)?),
        _ => (false, s),
    };
    let body = body
        .strip_prefix("0x")
        .or_else(|| body.strip_prefix("0X"))
        .unwrap_or(body);
    let (mantissa, exp) = match body.find(['p', 'P']) {
        Some(i) => (body.get(..i)?, Some(body.get(i + 1..)?)),
        None => (body, None),
    };
    let mut pexp: i64 = 0;
    if let Some(e) = exp {
        let (eneg, digits) = match e.as_bytes().first() {
            Some(b'-') => (true, e.get(1..)?),
            Some(b'+') => (false, e.get(1..)?),
            _ => (false, e),
        };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        for b in digits.bytes() {
            pexp = (pexp * 10 + i64::from(b - b'0')).min(1 << 20);
        }
        if eneg {
            pexp = -pexp;
        }
    }
    let mut mant: u64 = 0;
    let mut shift: i64 = 0;
    let mut sticky = false;
    let mut seen_point = false;
    let mut digits = 0usize;
    for c in mantissa.chars() {
        if c == '.' {
            if seen_point {
                return None;
            }
            seen_point = true;
            continue;
        }
        let d = c.to_digit(16)?;
        digits += 1;
        if mant < 1 << 56 {
            mant = mant * 16 + u64::from(d);
            if seen_point {
                shift -= 4;
            }
        } else {
            sticky |= d != 0;
            if !seen_point {
                shift += 4;
            }
        }
    }
    if digits == 0 {
        return None;
    }
    let sign = if neg { -1.0 } else { 1.0 };
    if mant == 0 {
        return Some(sign * 0.0);
    }
    let scale = shift + pexp;
    let len = i64::from(64 - mant.leading_zeros());
    let e = scale + len - 1;
    if e > 1023 {
        return Some(sign * f64::INFINITY);
    }
    // Significant bits the result can hold at this magnitude.
    let keep = if e >= -1022 { 53 } else { 53 - (-1022 - e) };
    if keep < 0 {
        return Some(sign * 0.0);
    }
    let drop = len - keep;
    let (m, k) = if drop <= 0 {
        (mant << drop.unsigned_abs(), scale + drop)
    } else {
        let d = drop.unsigned_abs();
        let kept = if d >= 64 { 0 } else { mant >> d };
        let half = if d > 64 {
            false
        } else {
            (mant >> (d - 1)) & 1 == 1
        };
        let below = d >= 2 && (d > 64 || mant & ((1u64 << (d - 1)) - 1) != 0);
        let up = half && (below || sticky || kept & 1 == 1);
        (kept + u64::from(up), scale + drop)
    };
    #[allow(clippy::cast_precision_loss)]
    let mut v = m as f64;
    let mut k = k;
    while k > 0 {
        let step = k.min(1000);
        v *= f64::from_bits(u64::try_from(1023 + step).unwrap_or(0) << 52);
        k -= step;
    }
    while k < 0 {
        let step = k.max(-1000);
        v *= f64::from_bits(u64::try_from(1023 + step).unwrap_or(0) << 52);
        k -= step;
    }
    Some(sign * v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g_matches_printf() {
        assert_eq!(fmt_g(0.1, 17), "0.10000000000000001");
        assert_eq!(fmt_g(2.0, 17), "2");
        assert_eq!(fmt_g(1e21, 17), "1e+21");
        assert_eq!(fmt_g(1.5e-7, 17), "1.4999999999999999e-07");
        assert_eq!(fmt_g(-0.0, 17), "-0");
        assert_eq!(fmt_g(0.000123, 3), "0.000123");
        assert_eq!(fmt_g(9.99, 2), "10");
        assert_eq!(fmt_g(1e100, 1), "1e+100");
    }

    #[test]
    fn hex_floats() {
        assert_eq!(parse_hex_float("1.0p+0"), Some(1.0));
        assert_eq!(parse_hex_float("-1.8p+4"), Some(-24.0));
        assert_eq!(
            parse_hex_float("1.921fb54442d18p+1"),
            Some(core::f64::consts::PI)
        );
        assert_eq!(parse_hex_float("1.0p-1022"), Some(f64::MIN_POSITIVE));
        assert_eq!(parse_hex_float("0x1p-1074"), Some(5e-324));
        assert_eq!(parse_hex_float("1p-1076"), Some(0.0));
        assert_eq!(parse_hex_float("1.fffffffffffff8p+0"), Some(2.0));
        assert_eq!(parse_hex_float("1p+1024"), Some(f64::INFINITY));
        assert_eq!(parse_hex_float("x"), None);
    }

    #[test]
    fn shortest_matches_reference() {
        assert_eq!(shortest_bin53(2.0), "2e+0");
        assert_eq!(shortest_bin53(-0.0), "-0.0");
        assert_eq!(shortest_bin53(1.5e-7), "1.5e-7");
        assert_eq!(shortest_bin53(12345.678), "1.2345678e+4");
        assert_eq!(shortest_bin53(5e-324), "4.9406564584124654e-324");
        assert_eq!(shortest_bin53(f64::NEG_INFINITY), "ninf");
        // The reference's neighbour choice from 21 truncated digits.
        assert_eq!(
            shortest_bin53(2.788_091_180_335_576_7e-266),
            "2.7880911803355766e-266"
        );
        assert_eq!(
            shortest_bin53(1.911_907_726_014_876_3e195),
            "1.9119077260148762e+195"
        );
    }
}
