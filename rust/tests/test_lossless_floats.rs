//! Port of python/tests/test_lossless_floats.py: typed floats of every family
//! and width keep their verbatim literal through a write and a typed read.
//!
//! Python's `Quantity.decimal` of a float is `Decimal(raw)`, so where a Python
//! test compares `.decimal` on a written-and-read value, the port compares the
//! literal `raw` itself. `from_number` takes the literal text here, the way
//! Python's `from_number` takes a `str`: `Decimal("x")` becomes `"x"`.
//!
//! Skipped (deliberately unmirrored: `BvnFloat`, the `Decimal`/`Fraction`
//! accessors `decimal`, `fraction`, `fixed_point`, `stored_value`,
//! `ieee_bits`, and NumPy):
//! - TestBinaryEncoderMatchesC: test_encoder_matches_c_oracle,
//!   test_negative_underflow_keeps_sign,
//!   test_decimal_encoder_correctly_rounded_and_matches_c (BvnFloat, ieee_bits)
//! - TestScalarLosslessRead: test_float128_literal_is_exact,
//!   test_float_dec_literal_is_exact, test_float_fix_fraction_is_exact,
//!   test_stored_value_rounds_to_format,
//!   test_extreme_exponent_binary_materialises_full_range,
//!   test_extreme_exponent_decimal_materialises_full_range,
//!   test_decimal_near_emax_full_coefficient_exact,
//!   test_genuine_format_underflow_is_zero_not_refused,
//!   test_binary128_stored_value_matches_ieee, test_decimal_works_for_ints,
//!   test_nonfinite, test_decimal_rejects_nonnumeric (decimal, fraction,
//!   fixed_point, stored_value, ieee_bits)
//! - TestScalarLosslessWrite: test_dumps_decimal_roundtrips,
//!   test_dumps_terminating_fraction, test_dumps_nonterminating_fraction_rejected
//!   (Python Decimal/Fraction values; `Value` has no such variant),
//!   test_from_number_float_fix_roundtrip (Fraction input, fraction accessor)
//! - TestAllWidths: test_binary_ieee_bits_and_stored_value,
//!   test_binary_bits_match_struct, test_decimal_stored_value,
//!   test_fixed_point_all_widths, test_wide_binary_float_beyond_256_is_lossless_literal
//!   (ieee_bits, BvnFloat, stored_value, fixed_point, fraction)
//! - TestNumpyScalarInputs (2 tests) and TestNumpyLossless (9 tests): NumPy
//! - TestDecimalNonFinite: test_ninf_bits_are_negative_infinity,
//!   test_ninf_decimal64, test_strtodec_bits_rejects_garbage,
//!   test_strtodec_bits_still_reads_numbers (ieee_bits, BvnFloat)
//! - TestExactNumberEdges: test_from_number_negative_infinity_is_ninf (formats
//!   a Python float; Rust takes the literal text),
//!   test_decimal_list_is_written_as_float_dec (Decimal/Fraction list values),
//!   test_stored_value_of_a_typed_null_is_none (decimal, stored_value, ieee_bits)

use bovnar::{Error, Quantity, Unit, Value, ValueTypeFamily as F, parse_typed, to_string};

/// A 38-digit value well beyond float64's ~15-17 significant digits.
const HIPI: &str = "1.10000000000000000022204460492503000007";

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

fn doc_of(key: &str, q: Quantity) -> Value {
    Value::Struct(vec![(key.into(), Value::Quantity(Box::new(q)))])
}

fn from_number(text: &str, family: F, width: u32, frac: Option<u32>) -> Result<Quantity, Error> {
    Quantity::from_number(text, family, width, frac, Unit::default())
}

fn assert_argument_error(r: Result<Quantity, Error>, needle: &str) {
    match r {
        Err(Error::Argument { message: msg, .. }) => {
            assert!(msg.contains(needle), "{msg:?} lacks {needle:?}")
        }
        other => panic!("expected an argument error matching {needle:?}, got {other:?}"),
    }
}

#[test]
fn test_from_number_float128_roundtrip() {
    let q = from_number(HIPI, F::Float, 128, None).unwrap();
    let out = to_string(&doc_of("a", q), true).unwrap();
    assert!(String::from_utf8_lossy(&out).contains("<float:128>"));
    let back = parse_typed(&out).unwrap();
    assert_eq!(quantity(field(&back, "a")).raw.as_deref(), Some(HIPI));
}

#[test]
fn test_from_number_float_fix_requires_frac() {
    assert_argument_error(from_number("1.5", F::FloatFix, 32, None), "frac");
}

#[test]
fn test_from_number_float_fix_range_checked() {
    assert_argument_error(from_number("1e9", F::FloatFix, 16, Some(8)), "does not fit");
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
        let q = from_number("0.5", family, width, frac).unwrap();
        let back = parse_typed(&to_string(&doc_of("a", q), true).unwrap()).unwrap();
        assert_eq!(
            quantity(field(&back, "a")).raw.as_deref(),
            Some("0.5"),
            "{family:?}:{width}"
        );
    }
}

#[test]
fn test_integers_to_256bit() {
    // 2**200 + 7: past i128, so the literal is compared (Python: `.decimal`).
    let big = "1606938044258990275541962092341162602522202993782792835301383";
    let doc = parse_typed(format!(".n=<uint:256>{big};").as_bytes()).unwrap();
    assert_eq!(quantity(field(&doc, "n")).raw.as_deref(), Some(big));
    let doc = parse_typed(b".n=<sint:128>-123;").unwrap();
    assert_eq!(quantity(field(&doc, "n")).value(), Value::Int(-123));
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
        let r = from_number("1.5", family, width, frac);
        if ok {
            assert!(r.is_ok(), "{family:?}:{width}: {r:?}");
        } else {
            assert_argument_error(r, "width");
        }
    }
}

#[test]
fn test_from_number_refuses_a_decimal_past_the_width() {
    // decimal64 tops out below 1e385; this was written as <float_dec:64>
    // 1e+400, which materialises to infinity. (Python's second case,
    // `dumps({'a': Decimal('-1e400')})`, needs a Decimal value: not mirrored.)
    assert!(matches!(
        from_number("1e+400", F::FloatDec, 64, None),
        Err(Error::Argument { .. })
    ));
    assert_eq!(
        from_number("1e+400", F::FloatDec, 128, None)
            .unwrap()
            .raw
            .as_deref(),
        Some("1e+400")
    );
    assert_eq!(
        from_number("9.99e+384", F::FloatDec, 64, None)
            .unwrap()
            .raw
            .as_deref(),
        Some("9.99e+384")
    );
}
