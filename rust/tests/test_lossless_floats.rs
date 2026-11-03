//! float128 / float256 / float_fix / float_dec carried at full precision --
//! exact `Decimal` and rational accessors, bit-exact IEEE materialisation,
//! and exact writes.
//!
//! Exact rationals are `BigRational` and exact decimals [`Decimal`]. Where a
//! test checks a result against decimal rounding in a context (precision,
//! exponent limits, rounding mode), `create_decimal` below performs that
//! rounding of a finite value (the General Decimal Arithmetic `_fix` step)
//! for the three roundings the tests use.

use bovnar::bvnfloat::{
    decimal_stored_value, decimal_to_interchange_bits, fraction_to_binary_bits,
};
use bovnar::{
    BvnFloat, Decimal, Error, Quantity, Unit, Value, ValueTypeFamily as F, parse_typed, to_string,
};
use num_bigint::{BigInt, BigUint};
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};

/// A 38-digit value well beyond float64's ~15-17 significant digits.
const HIPI: &str = "1.10000000000000000022204460492503000007";
/// 34 significant digits (decimal128).
const PI34: &str = "3.141592653589793238462643383279503";

const WORK_PREC: u32 = bovnar::bvnfloat::WORK_PREC;

fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}

fn frac(n: i64, d: i64) -> BigRational {
    BigRational::new(n.into(), d.into())
}

fn field<'a>(doc: &'a Value, key: &str) -> &'a Value {
    match doc {
        Value::Struct(fields) => fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
            .unwrap_or_else(|| panic!("no field {key:?} in {doc:?}")),
        other => panic!("not a struct: {other:?}"),
    }
}

fn quantity(v: &Value) -> &Quantity {
    match v {
        Value::Quantity(q) => q,
        other => panic!("not a Quantity: {other:?}"),
    }
}

/// `loads(doc, typed=True)[key]`.
fn load(doc: &str, key: &str) -> Quantity {
    let v = parse_typed(doc.as_bytes()).unwrap();
    quantity(field(&v, key)).clone()
}

/// `dumps({key: value})`.
fn dumps(key: &str, value: Value) -> Result<Vec<u8>, Error> {
    to_string(&Value::Struct(vec![(key.into(), value)]), true)
}

fn quantity_value(q: Quantity) -> Value {
    Value::Quantity(Box::new(q))
}

fn from_number(
    value: impl Into<bovnar::ExactNumber>,
    family: F,
    width: u32,
    frac: Option<u32>,
) -> Result<Quantity, Error> {
    Quantity::from_number(value, family, width, frac, Unit::default())
}

fn assert_argument_error<T: std::fmt::Debug>(r: Result<T, Error>, needle: &str) {
    match r {
        Err(Error::Argument { message: msg, .. }) => {
            assert!(msg.contains(needle), "{msg:?} lacks {needle:?}")
        }
        other => panic!("expected an argument error matching {needle:?}, got {other:?}"),
    }
}

fn decimal(q: &Quantity) -> Decimal {
    q.decimal().unwrap().unwrap()
}

fn stored(q: &Quantity) -> Decimal {
    q.stored_value().unwrap().unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A deterministic LCG step, so the sampled values are reproducible.
fn lcg(x: u64) -> u64 {
    x.wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407)
}

#[derive(Clone, Copy)]
enum Rounding {
    HalfEven,
    Down,
    Up,
}

/// `Context(prec, Emax, Emin, rounding, traps off).create_decimal(d)` for a
/// finite `d` (the General Decimal Arithmetic `_fix` step, clamp 0).
fn create_decimal(d: &Decimal, prec: i64, emax: i64, emin: i64, rounding: Rounding) -> Decimal {
    let neg = d.is_signed();
    let (coeff, exp) = (d.coefficient().unwrap(), d.exponent().unwrap());
    let (etiny, etop) = (emin - prec + 1, emax - prec + 1);
    if coeff.is_zero() {
        return Decimal::new(neg, BigUint::zero(), exp.max(etiny).min(emax));
    }
    let overflow = || match rounding {
        Rounding::Down => Decimal::new(neg, BigUint::from(10u32).pow(prec as u32) - 1u32, etop),
        _ => Decimal::infinity(neg),
    };
    let digits = coeff.to_str_radix(10);
    let n = digits.len() as i64;
    let mut exp_min = n + exp - prec;
    if exp_min > etop {
        return overflow();
    }
    exp_min = exp_min.max(etiny);
    if exp >= exp_min {
        return d.clone();
    }
    let keep = n + exp - exp_min;
    let (kept, rest) = if keep < 0 {
        // Every digit goes: a single 1 stands just below the place
        // kept (the value is below half of it, and not zero).
        (String::new(), "1".to_string())
    } else {
        let (k, r) = digits.split_at(keep as usize);
        (k.to_string(), r.to_string())
    };
    let nonzero = rest.bytes().any(|b| b != b'0');
    let up = match rounding {
        Rounding::Down => false,
        Rounding::Up => nonzero,
        Rounding::HalfEven => {
            let first = rest.as_bytes()[0];
            let exact_half = first == b'5' && rest.bytes().skip(1).all(|b| b == b'0');
            let last_even = kept
                .bytes()
                .last()
                .is_none_or(|b| (b - b'0').is_multiple_of(2));
            if exact_half {
                !last_even
            } else {
                first >= b'5'
            }
        }
    };
    let mut c = if kept.is_empty() {
        BigUint::zero()
    } else {
        kept.parse::<BigUint>().unwrap()
    };
    if up {
        c += 1u32;
        if c.to_str_radix(10).len() as i64 > prec {
            c /= 10u32;
            exp_min += 1;
        }
    }
    if exp_min > etop {
        return overflow();
    }
    Decimal::new(neg, c, exp_min)
}

// ── TestBinaryEncoderMatchesC ───────────────────────────────────────────
// The pure IEEE encoders (used for extreme exponents the C parser can't
// reach) must be bit-identical to the C encoder over the range the C
// encoder can reach.

#[test]
fn test_encoder_matches_c_oracle() {
    // Exact zero is skipped: a rational has no signed zero, and the fallback
    // only ever runs for saturated nonzero values.
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut mism = 0;
    for _ in 0..8000 {
        x = lcg(x);
        let digits = (x >> 3) % 18 + 1;
        x = lcg(x);
        let mant = x % 10u64.pow(digits as u32);
        x = lcg(x);
        let exp = ((x >> 5) % 120) as i64 - 60;
        if mant == 0 {
            continue;
        }
        let sign = if x & 1 == 1 { "-" } else { "" };
        let s = format!("{sign}{mant}e{exp}");
        for w in [16, 32, 64, 128, 256] {
            let c = BvnFloat::from_str(&s, 10, WORK_PREC)
                .unwrap()
                .to_binary_bits(w)
                .unwrap();
            let p = fraction_to_binary_bits(&dec(&s).to_fraction().unwrap(), w).unwrap();
            if c != p {
                mism += 1;
            }
        }
    }
    assert_eq!(mism, 0);
}

#[test]
fn test_negative_underflow_keeps_sign() {
    // a negative value that underflows in the format materialises to -0.0
    let q = load(".a=<float:256>-1e-80000;", "a");
    assert!(q.ieee_bits().unwrap().last().unwrap() & 0x80 != 0);
}

#[test]
fn test_decimal_encoder_correctly_rounded_and_matches_c() {
    // (1) the materialised value equals the reference's round half to even
    //     (full range); (2) the bits equal the C to_dec* for non-tie values
    //     (at a tie the C path rounds its binary approximation of the
    //     literal).
    // width -> (exp_bits, bias, max_digits)
    let params = [
        (16, (6, 24, 2)),
        (32, (8, 101, 7)),
        (64, (10, 398, 16)),
        (128, (14, 6176, 34)),
        (256, (20, 611_867, 70)),
    ];
    let ctx_for = |w: u32, rounding: Rounding, d: &Decimal| {
        let (eb, bias, md) = params.iter().find(|p| p.0 == w).unwrap().1;
        let std = bias == 3 * (1i64 << (eb - 3)) + md - 2;
        let be_max = if std {
            3i64 << (eb - 2)
        } else {
            (1i64 << eb) - 1
        };
        create_decimal(d, md, be_max - 1 - bias + md - 1, -bias + md - 1, rounding)
    };
    let mut x: u64 = 0x123_4567;
    for _ in 0..5000 {
        x = lcg(x);
        let nd = (x >> 3) % 22 + 1;
        x = lcg(x);
        let mant = u128::from(x) % 10u128.pow(nd as u32);
        x = lcg(x);
        let exp = ((x >> 5) % 120) as i64 - 60;
        if mant == 0 {
            continue;
        }
        let s = format!("{}{mant}e{exp}", if x & 1 == 1 { "-" } else { "" });
        let d = dec(&s);
        for w in [16, 32, 64, 128, 256] {
            let reference = ctx_for(w, Rounding::HalfEven, &d);
            let mine = decimal_stored_value(&d, w).unwrap();
            if !mine.is_zero() {
                assert!(
                    mine == reference && mine.is_signed() == reference.is_signed(),
                    "{s} at {w}: {mine} != {reference}"
                );
            }
            if exp.abs() < 30 {
                let down = ctx_for(w, Rounding::Down, &d);
                let up = ctx_for(w, Rounding::Up, &d);
                let is_tie = down != up
                    && match (down.to_fraction(), up.to_fraction()) {
                        (Ok(a), Ok(b)) => a + b == d.to_fraction().unwrap() * BigInt::from(2),
                        _ => false,
                    };
                if !is_tie {
                    assert_eq!(
                        BvnFloat::from_str(&s, 10, WORK_PREC)
                            .unwrap()
                            .to_decimal_bits(w)
                            .unwrap(),
                        decimal_to_interchange_bits(&d, w).unwrap(),
                        "{s} at {w}"
                    );
                }
            }
        }
    }
}

// ── TestScalarLosslessRead ──────────────────────────────────────────────

#[test]
fn test_float128_literal_is_exact() {
    let q = load(&format!(".a=<float:128>{HIPI};"), "a");
    assert_eq!(decimal(&q), dec(HIPI));
    let Value::Float(v) = q.value() else {
        panic!("not a float")
    };
    assert!((v - 1.1).abs() < 1e-12); // the lossy double path
}

#[test]
fn test_float_dec_literal_is_exact() {
    let q = load(&format!(".p=<float_dec:128>{PI34};"), "p");
    assert_eq!(decimal(&q), dec(PI34));
}

#[test]
fn test_float_fix_fraction_is_exact() {
    // 3.27 is not exact in Q8; the stored datum is round(3.27*256)=837
    let q = load(".x=<float_fix:32,q8>3.27;", "x");
    assert_eq!(q.fraction().unwrap(), Some(frac(837, 256)));
    assert_eq!(q.fixed_point().unwrap(), (BigInt::from(837), 8));
    assert_eq!(stored(&q), dec("3.26953125"));
}

#[test]
fn test_stored_value_rounds_to_format() {
    // a 34-digit literal in decimal64 (16 digits) -> stored value is rounded
    let q = load(&format!(".p=<float_dec:64>{PI34};"), "p");
    assert_eq!(decimal(&q), dec(PI34)); // the literal
    assert_eq!(stored(&q), dec("3.141592653589793")); // decimal64
}

#[test]
fn test_extreme_exponent_binary_materialises_full_range() {
    // The C parser saturates an extreme exponent, but the exact encoder
    // materialises the correct binary256 value across the full range.
    let q = load(".a=<float:256>1e-78000;", "a");
    assert_eq!(decimal(&q), dec("1e-78000")); // exact literal
    let sv = stored(&q);
    assert!(!sv.is_zero());
    let ratio = sv.to_fraction().unwrap() / dec("1e-78000").to_fraction().unwrap();
    assert_eq!(ratio.to_f64(), Some(1.0));
    assert_eq!(q.ieee_bits().unwrap().len(), 32);
    // genuine range boundaries of binary256
    assert_eq!(
        stored(&load(".a=<float:256>1e80000;", "a")),
        dec("Infinity")
    );
    assert!(stored(&load(".a=<float:256>1e-80000;", "a")).is_zero());
}

#[test]
fn test_extreme_exponent_decimal_materialises_full_range() {
    // the decimal-interchange path also materialises beyond the C parser's
    // cap via the exact encoder.
    let q = load(".a=<float_dec:256>1e20000;", "a");
    assert_eq!(decimal(&q), dec("1e20000"));
    assert_eq!(stored(&q), dec("1E+20000"));
    assert_eq!(q.ieee_bits().unwrap().len(), 32);
    let qn = load(".a=<float_dec:256>-1.5e-20000;", "a");
    assert_eq!(stored(&qn), dec("-1.5E-20000"));
    // decimal64 range boundaries (Emax 384)
    assert_eq!(
        stored(&load(".a=<float_dec:64>1e400;", "a")),
        dec("Infinity")
    );
    assert!(stored(&load(".a=<float_dec:64>1e-400;", "a")).is_zero());
}

#[test]
fn test_decimal_near_emax_full_coefficient_exact() {
    // A full-coefficient value just under the format's Emax is exactly
    // representable; the materialised value must keep every digit.
    let s = "9999999999999999999999999999999999e6111"; // 34 nines, decimal128 max
    let q = load(&format!(".a=<float_dec:128>{s};"), "a");
    assert_eq!(stored(&q), dec(s));
    // the emitted bits decode back to exactly that value
    assert_eq!(
        BvnFloat::from_decimal_bits(128, &q.ieee_bits().unwrap())
            .unwrap()
            .to_decimal()
            .unwrap(),
        stored(&q)
    );
    // and it equals decimal128 rounding
    assert_eq!(
        stored(&q),
        create_decimal(&dec(s), 34, 6144, -6143, Rounding::HalfEven)
    );
}

#[test]
fn test_genuine_format_underflow_is_zero_not_refused() {
    // a literal within the parser's range that simply rounds below the
    // format's smallest value materialises to 0 / inf, not an error.
    assert!(stored(&load(".a=<float:16>1e-100;", "a")).is_zero());
    assert_eq!(stored(&load(".a=<float:16>1e100;", "a")), dec("Infinity"));
}

#[test]
fn test_binary128_stored_value_matches_ieee() {
    // the materialised binary128 of 0.1 differs from the literal
    let q = load(".a=<float:128>0.1;", "a");
    let sv = stored(&q);
    assert_ne!(sv, dec("0.1"));
    assert_eq!(sv.to_f64(), 0.1); // nearest double is 0.1
    assert_eq!(q.ieee_bits().unwrap().len(), 16); // binary128 = 16 bytes
}

#[test]
fn test_decimal_works_for_ints() {
    let q = load(".n=<uint:64>18446744073709551615;", "n");
    assert_eq!(decimal(&q), Decimal::from(18_446_744_073_709_551_615i128));
}

#[test]
fn test_nonfinite() {
    assert!(decimal(&load(".z=<float:64>nan;", "z")).is_nan());
    assert_eq!(decimal(&load(".z=<float:64>inf;", "z")), dec("Infinity"));
}

#[test]
fn test_decimal_rejects_nonnumeric() {
    let q = load(".s=\"hi\";", "s");
    assert_argument_error(q.decimal(), "");
}

// ── TestScalarLosslessWrite ─────────────────────────────────────────────

#[test]
fn test_dumps_decimal_roundtrips() {
    let d = dec(PI34);
    let out = dumps("p", Value::Decimal(d.clone())).unwrap();
    assert!(String::from_utf8_lossy(&out).contains("<float_dec:64>"));
    assert_eq!(decimal(&load(std::str::from_utf8(&out).unwrap(), "p")), d);
}

#[test]
fn test_dumps_terminating_fraction() {
    let out = dumps("x", Value::Fraction(frac(3, 8))).unwrap();
    assert!(String::from_utf8_lossy(&out).contains("0.375"));
}

#[test]
fn test_dumps_nonterminating_fraction_rejected() {
    assert_argument_error(dumps("x", Value::Fraction(frac(1, 3))), "terminating");
}

#[test]
fn test_from_number_float128_roundtrip() {
    let q = from_number(dec(HIPI), F::Float, 128, None).unwrap();
    let out = dumps("a", quantity_value(q)).unwrap();
    assert!(String::from_utf8_lossy(&out).contains("<float:128>"));
    assert_eq!(
        decimal(&load(std::str::from_utf8(&out).unwrap(), "a")),
        dec(HIPI)
    );
}

#[test]
fn test_from_number_float_fix_roundtrip() {
    let q = from_number(frac(837, 256), F::FloatFix, 32, Some(8)).unwrap();
    let out = dumps("x", quantity_value(q)).unwrap();
    let back = load(std::str::from_utf8(&out).unwrap(), "x");
    assert_eq!(back.fraction().unwrap(), Some(frac(837, 256)));
}

#[test]
fn test_from_number_float_fix_requires_frac() {
    assert_argument_error(from_number(dec("1.5"), F::FloatFix, 32, None), "frac");
}

#[test]
fn test_from_number_float_fix_range_checked() {
    assert_argument_error(
        from_number(dec("1e9"), F::FloatFix, 16, Some(8)),
        "does not fit",
    );
}

// ── TestAllWidths ───────────────────────────────────────────────────────
// Binary, decimal and fixed-point at 16..256 bits, integers to 256 bits:
// the lossless accessors cover every one of them.

#[test]
fn test_binary_ieee_bits_and_stored_value() {
    for width in [16, 32, 64, 128, 256] {
        let q = load(&format!(".a=<float:{width}>0.1;"), "a");
        let bits = q.ieee_bits().unwrap();
        assert_eq!(bits.len(), width as usize / 8);
        // the materialised value re-encodes to the same bytes (canonical)
        assert_eq!(
            BvnFloat::from_binary_bits(width, &bits)
                .unwrap()
                .to_binary_bits(width)
                .unwrap(),
            bits
        );
        // and the stored value is a Decimal
        assert!(q.stored_value().unwrap().is_some());
    }
}

#[test]
fn test_binary_bits_match_struct() {
    let q32 = load(".a=<float:32>0.1;", "a");
    let q64 = load(".a=<float:64>0.1;", "a");
    assert_eq!(q32.ieee_bits().unwrap(), 0.1f32.to_le_bytes());
    assert_eq!(q64.ieee_bits().unwrap(), 0.1f64.to_le_bytes());
}

#[test]
fn test_decimal_stored_value() {
    for width in [16, 32, 64, 128, 256] {
        let q = load(&format!(".p=<float_dec:{width}>1.5;"), "p");
        assert_eq!(stored(&q), dec("1.5"));
        assert_eq!(q.ieee_bits().unwrap().len(), width as usize / 8);
    }
}

#[test]
fn test_fixed_point_all_widths() {
    for width in [16, 32, 64, 128, 256] {
        let q = load(&format!(".x=<float_fix:{width},q4>2.5;"), "x");
        assert_eq!(q.fixed_point().unwrap(), (BigInt::from(40), 4)); // 2.5 * 2**4
        assert_eq!(q.fraction().unwrap(), Some(frac(5, 2)));
    }
}

#[test]
fn test_write_roundtrip_every_width() {
    let cases = [
        (F::Float, 16, None),
        (F::Float, 32, None),
        (F::Float, 64, None),
        (F::Float, 128, None),
        (F::Float, 256, None),
        (F::FloatDec, 16, None),
        (F::FloatDec, 256, None),
        (F::FloatFix, 16, Some(4)),
        (F::FloatFix, 256, Some(8)),
    ];
    for (family, width, frac) in cases {
        let q = from_number(dec("0.5"), family, width, frac).unwrap();
        let out = dumps("a", quantity_value(q)).unwrap();
        let back = load(std::str::from_utf8(&out).unwrap(), "a");
        assert_eq!(decimal(&back), dec("0.5"), "{family:?}:{width}");
    }
}

#[test]
fn test_integers_to_256bit() {
    let big = (BigInt::one() << 200) + 7;
    let q = load(&format!(".n=<uint:256>{big};"), "n");
    assert_eq!(decimal(&q), Decimal::from(big));
    assert_eq!(load(".n=<sint:128>-123;", "n").value(), Value::Int(-123));
}

#[test]
fn test_wide_binary_float_beyond_256_is_lossless_literal() {
    // binary float accepts any multiple of 32; the literal round-trips
    // exactly even where there is no IEEE bit encoding (>256).
    let q = load(&format!(".a=<float:512>{HIPI};"), "a");
    assert_eq!(decimal(&q), dec(HIPI));
    assert_argument_error(q.stored_value(), "no such encoding");
}

#[test]
fn test_width_validated_early() {
    let cases = [
        // binary float: 16 or any multiple of 32 (incl. non-powers-of-2), to 32768
        (F::Float, 48, None, false),
        (F::Float, 512, None, true),
        (F::Float, 96, None, true),
        (F::Float, 192, None, true),
        (F::Float, 24, None, false),
        (F::Float, 65536, None, false),
        // decimal/fixed: only the IEEE interchange set 16/32/64/128/256
        (F::FloatDec, 512, None, false),
        (F::FloatDec, 128, None, true),
        (F::FloatDec, 96, None, false),
        (F::FloatFix, 200, Some(4), false),
        (F::FloatFix, 64, Some(4), true),
    ];
    for (family, width, frac, ok) in cases {
        let r = from_number(dec("1.5"), family, width, frac);
        if ok {
            assert!(r.is_ok(), "{family:?}:{width}: {r:?}");
        } else {
            assert_argument_error(r, "width");
        }
    }
}

// ── TestDecimalNonFinite ────────────────────────────────────────────────
// `ieee_bits` on a float_dec `ninf` returned the encoding of +0 at every
// width while `stored_value` on the same object said -Infinity.

#[test]
fn test_ninf_bits_are_negative_infinity() {
    for width in [16, 32, 64, 128, 256] {
        let q = load(&format!(".a=<float_dec:{width}>ninf;"), "a");
        let bits = q.ieee_bits().unwrap();
        assert_eq!(
            bits,
            BvnFloat::from_str("ninf", 10, WORK_PREC)
                .unwrap()
                .to_decimal_bits(width)
                .unwrap()
        );
        assert!(bits.last().unwrap() & 0x80 != 0);
        let pos = load(&format!(".a=<float_dec:{width}>inf;"), "a");
        let n = bits.len() - 1;
        assert_eq!(bits[..n], pos.ieee_bits().unwrap()[..n]);
    }
}

#[test]
fn test_ninf_decimal64() {
    let q = load(".a=<float_dec:64>ninf;", "a");
    assert_eq!(hex(&q.ieee_bits().unwrap()), "00000000000000f8");
}

#[test]
fn test_strtodec_bits_rejects_garbage() {
    // It promised an argument error and returned eight zero bytes.
    for text in ["abc", "", "1.2.3", "--1"] {
        assert_argument_error(BvnFloat::strtodec_bits(text, 64), "");
    }
}

#[test]
fn test_strtodec_bits_still_reads_numbers() {
    assert_eq!(
        hex(&BvnFloat::strtodec_bits("-1", 64).unwrap()),
        "010000000000c0b1"
    );
    assert_eq!(
        hex(&BvnFloat::strtodec_bits("nan", 64).unwrap()),
        "000000000000007c"
    );
}

// ── TestExactNumberEdges ────────────────────────────────────────────────

#[test]
fn test_from_number_negative_infinity_is_ninf() {
    // -inf is spelled 'ninf' in the format, and the
    // writer refused the Quantity from_number had just built.
    let q = from_number(f64::NEG_INFINITY, F::FloatDec, 64, None).unwrap();
    assert_eq!(q.raw.as_deref(), Some("ninf"));
    let doc = Value::Struct(vec![("a".into(), quantity_value(q))]);
    assert_eq!(to_string(&doc, false).unwrap(), b".a=<float_dec:64>ninf;");
    assert_eq!(
        from_number(f64::INFINITY, F::FloatDec, 64, None)
            .unwrap()
            .raw
            .as_deref(),
        Some("inf")
    );
    assert_eq!(
        from_number(f64::NAN, F::FloatDec, 64, None)
            .unwrap()
            .raw
            .as_deref(),
        Some("nan")
    );
}

#[test]
fn test_from_number_refuses_a_decimal_past_the_width() {
    // decimal64 tops out below 1e385; this was written as <float_dec:64>
    // 1e+400, which materialises to infinity.
    assert_argument_error(from_number(dec("1e400"), F::FloatDec, 64, None), "");
    assert_argument_error(dumps("a", Value::Decimal(dec("-1e400"))), "");
    assert_eq!(
        from_number(dec("1e400"), F::FloatDec, 128, None)
            .unwrap()
            .raw
            .as_deref(),
        Some("1e+400")
    );
    assert_eq!(
        from_number(dec("9.99e384"), F::FloatDec, 64, None)
            .unwrap()
            .raw
            .as_deref(),
        Some("9.99e+384")
    );
}

#[test]
fn test_decimal_list_is_written_as_float_dec() {
    // The scalar path annotates <float_dec:64>; the list path wrote the
    // elements bare, so 1.10 came back a binary float and 2 an integer.
    let list = Value::Array(vec![
        Value::Decimal(dec("1.10")),
        Value::Fraction(frac(1, 4)),
        Value::Null,
    ]);
    let out = to_string(&Value::Struct(vec![("k".into(), list)]), false).unwrap();
    assert_eq!(out, b".k=<float_dec:64>[1.10,0.25,null];");
    let back = parse_typed(&out).unwrap();
    let Value::Array(items) = field(&back, "k") else {
        panic!("not an array")
    };
    let qs: Vec<&Quantity> = items.iter().map(quantity).collect();
    let decimals: Vec<Option<Decimal>> = qs.iter().map(|q| q.decimal().unwrap()).collect();
    assert_eq!(decimals, [Some(dec("1.10")), Some(dec("0.25")), None]);
    assert!(qs.iter().all(|q| q.family() == Some(F::FloatDec)));
}

#[test]
fn test_stored_value_of_a_typed_null_is_none() {
    let q = load(".a=<float:64,m>;", "a");
    assert_eq!(q.decimal().unwrap(), None);
    assert_eq!(q.stored_value().unwrap(), None);
    assert_argument_error(q.ieee_bits(), "");
}
