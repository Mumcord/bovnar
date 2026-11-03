//! Port of `python/tests/test_write_array.py`. Each Python test class is a
//! module here so the test names stay the same.
//!
//! Skipped Python tests:
//! - `TestWriteArrayHeldToAnnotation::test_generator_rows_keep_their_first_element`:
//!   `write_array` takes a slice; Rust has no one-shot generator argument
//!   whose head a probe could consume.
//!
//! Partly expressible: `TestDumpsBigInt::test_scalar_bigint_roundtrip` drops
//! the cases 2**255 and -(2**255), which do not fit `Value::Int` (`i128`).

use bovnar::{
    Error, Unit, Value, ValueTypeFamily, ValueTypeSpec, Writer, parse, parse_typed, to_string,
    write_array,
};

/// Python's `DEFAULT_MEM_CAP`.
const CAP: usize = 4 * 1024 * 1024;

/// A plain value for the `v!` macro.
trait Lit {
    fn lit(self) -> Value;
}

impl Lit for i32 {
    fn lit(self) -> Value {
        Value::Int(self.into())
    }
}

impl Lit for i128 {
    fn lit(self) -> Value {
        Value::Int(self)
    }
}

impl Lit for f64 {
    fn lit(self) -> Value {
        Value::Float(self)
    }
}

impl Lit for bool {
    fn lit(self) -> Value {
        Value::Bool(self)
    }
}

impl Lit for &str {
    fn lit(self) -> Value {
        Value::String(self.into())
    }
}

impl Lit for Value {
    fn lit(self) -> Value {
        self
    }
}

/// A Python literal as a `Value`: `v!([1, [2, -3], null, "s"])`.
macro_rules! v {
    (@items [$($done:expr,)*]) => { vec![$($done),*] };
    (@items [$($done:expr,)*] [$($inner:tt)*] $(, $($rest:tt)*)?) => {
        v!(@items [$($done,)* v!([$($inner)*]),] $($($rest)*)?)
    };
    (@items [$($done:expr,)*] null $(, $($rest:tt)*)?) => {
        v!(@items [$($done,)* Value::Null,] $($($rest)*)?)
    };
    (@items [$($done:expr,)*] $e:expr $(, $($rest:tt)*)?) => {
        v!(@items [$($done,)* Lit::lit($e),] $($($rest)*)?)
    };
    ([$($t:tt)*]) => { Value::Array(v!(@items [] $($t)*)) };
    ($e:expr) => { Lit::lit($e) };
}

/// A Python dict as a `Value::Struct`.
fn st(fields: Vec<(&str, Value)>) -> Value {
    Value::Struct(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
}

fn items(v: Value) -> Vec<Value> {
    match v {
        Value::Array(items) => items,
        other => panic!("not an array: {other:?}"),
    }
}

fn field<'a>(doc: &'a Value, key: &str) -> &'a Value {
    let Value::Struct(entries) = doc else {
        panic!("not a struct: {doc:?}");
    };
    &entries.iter().find(|(k, _)| k == key).unwrap().1
}

fn has(out: &[u8], needle: &[u8]) -> bool {
    out.windows(needle.len()).any(|win| win == needle)
}

fn vt(family: ValueTypeFamily, width: u32, base: u32) -> ValueTypeSpec {
    ValueTypeSpec {
        family: family as i32,
        width,
        base,
    }
}

/// `with Writer.to_mem() as w: write_array(w, key, rows, vt=, vu=)`.
fn written(key: &str, rows: Value, spec: Option<ValueTypeSpec>, unit: Option<&Unit>) -> Vec<u8> {
    let mut w = Writer::to_mem(CAP, true).unwrap();
    write_array(&mut w, key, &items(rows), spec, unit, None).unwrap();
    w.finish().unwrap();
    w.get_output().to_vec()
}

/// Python's `_roundtrip`: write `rows` as `.data`, parse, return `.data`.
fn roundtrip(rows: Value) -> Value {
    let out = written("data", rows, None, None);
    field(&parse(&out).unwrap(), "data").clone()
}

fn dumps(doc: &Value) -> Vec<u8> {
    to_string(doc, true).unwrap()
}

fn loads(out: &[u8]) -> Value {
    parse(out).unwrap()
}

mod write_array_flat {
    use super::*;

    #[test]
    fn test_ints_roundtrip() {
        assert_eq!(roundtrip(v!([1, 2, 3])), v!([1, 2, 3]));
    }

    #[test]
    fn test_single_element() {
        assert_eq!(roundtrip(v!([42])), v!([42]));
    }

    #[test]
    fn test_floats() {
        let result = items(roundtrip(v!([1.5, 2.5, 3.5])));
        assert_eq!(result.len(), 3);
        let Value::Float(f) = result[0] else {
            panic!("not a float: {:?}", result[0]);
        };
        assert!((f - 1.5).abs() < 1e-9);
    }

    #[test]
    fn test_strings() {
        assert_eq!(roundtrip(v!(["a", "b", "c"])), v!(["a", "b", "c"]));
    }

    #[test]
    fn test_negative_ints() {
        assert_eq!(roundtrip(v!([-1, -2, -3])), v!([-1, -2, -3]));
    }

    #[test]
    fn test_mixed_int_signs() {
        assert_eq!(roundtrip(v!([-5, 0, 5])), v!([-5, 0, 5]));
    }

    #[test]
    fn test_empty_list() {
        // An empty array "[]" is a genuinely empty row (zero elements) and is
        // distinct from "[null]" (one null element); it round-trips as [].
        assert_eq!(roundtrip(v!([])), v!([]));
    }

    #[test]
    fn test_output_contains_key() {
        let out = written("mykey", v!([1, 2]), None, None);
        assert!(has(&out, b"mykey"));
    }

    #[test]
    fn test_output_contains_values() {
        let out = written("vals", v!([10, 20, 30]), None, None);
        assert!(has(&out, b"10"));
        assert!(has(&out, b"20"));
        assert!(has(&out, b"30"));
    }
}

mod write_array_multi_row {
    use super::*;

    #[test]
    fn test_2x2_roundtrip() {
        assert_eq!(roundtrip(v!([[1, 2], [3, 4]])), v!([[1, 2], [3, 4]]));
    }

    #[test]
    fn test_3x1_roundtrip() {
        assert_eq!(roundtrip(v!([[10], [20], [30]])), v!([[10], [20], [30]]));
    }

    #[test]
    fn test_output_contains_slash_separator() {
        let out = written("m", v!([[1, 2], [3, 4]]), None, None);
        assert!(has(&out, b"/"));
    }

    #[test]
    fn test_1x3_treated_as_flat() {
        assert_eq!(roundtrip(v!([1, 2, 3])), v!([1, 2, 3]));
    }

    #[test]
    fn test_3x3_matrix() {
        let rows = v!([[1, 2, 3], [4, 5, 6], [7, 8, 9]]);
        assert_eq!(roundtrip(rows.clone()), rows);
    }

    #[test]
    fn test_uniform_rows() {
        // BVNR arrays are rectangular: all rows must have the same column count.
        let rows = v!([[1, 0], [2, 3], [4, 5]]);
        assert_eq!(roundtrip(rows.clone()), rows);
    }
}

mod write_array_dict_elements {
    use super::*;

    #[test]
    fn test_single_dict_element() {
        let result = items(roundtrip(v!([st(vec![("x", v!(1))])])));
        assert_eq!(result.len(), 1);
        assert_eq!(field(&result[0], "x"), &v!(1));
    }

    #[test]
    fn test_two_dict_elements() {
        let rows = v!([st(vec![("a", v!(1))]), st(vec![("a", v!(2))])]);
        assert_eq!(roundtrip(rows.clone()), rows);
    }

    #[test]
    fn test_dict_with_multiple_fields() {
        let elem = st(vec![("x", v!(10)), ("y", v!(20))]);
        let result = items(roundtrip(v!([elem.clone()])));
        assert_eq!(result[0], elem);
    }

    #[test]
    fn test_dict_with_nested_struct() {
        let elem = st(vec![("cfg", st(vec![("port", v!(80))]))]);
        let result = items(roundtrip(v!([elem])));
        assert_eq!(field(field(&result[0], "cfg"), "port"), &v!(80));
    }

    #[test]
    fn test_dict_with_array_field() {
        let elem = st(vec![("vals", v!([1, 2, 3]))]);
        let result = items(roundtrip(v!([elem])));
        assert_eq!(field(&result[0], "vals"), &v!([1, 2, 3]));
    }
}

mod write_array_nested_elements {
    use super::*;

    #[test]
    fn test_nested_list_as_element() {
        assert_eq!(roundtrip(v!([[1, 2], [3, 4]])), v!([[1, 2], [3, 4]]));
    }

    #[test]
    fn test_single_nested_list() {
        let out = written("d", v!([1, 2, 3]), None, None);
        assert!(has(&out, b"d"));
    }
}

mod write_array_with_type {
    use super::*;

    #[test]
    fn test_uint32_annotation_in_output() {
        let spec = vt(ValueTypeFamily::Uint, 32, 10);
        let out = written("ports", v!([80, 443]), Some(spec), None);
        assert!(has(&out, b"uint"));
        assert!(has(&out, b"32"));
    }

    #[test]
    fn test_float64_annotation() {
        let spec = vt(ValueTypeFamily::Float, 64, 0);
        let out = written("vals", v!([1.0, 2.0]), Some(spec), None);
        assert!(has(&out, b"float"));
    }

    #[test]
    fn test_unit_annotation() {
        let spec = vt(ValueTypeFamily::Float, 64, 0);
        let km = Unit::parse("k~m").unwrap();
        let out = written("distances", v!([1.0, 2.0]), Some(spec), Some(&km));
        assert!(has(&out, b"k~m") || has(&out, b"km"));
    }

    #[test]
    fn test_no_annotation_still_valid() {
        assert_eq!(roundtrip(v!([5, 10, 15])), v!([5, 10, 15]));
    }
}

/// Array elements must use the array token types; the C writer refuses the
/// scalar ones, so writing without an error is the check.
mod write_array_token_types {
    use super::*;

    #[test]
    fn test_int_array_parses_without_error() {
        written("x", v!([1, 2, 3]), None, None);
    }

    #[test]
    fn test_float_array_parses_without_error() {
        written("x", v!([1.0, 2.0, 3.0]), None, None);
    }

    #[test]
    fn test_string_array_parses_without_error() {
        written("x", v!(["hello", "world"]), None, None);
    }

    #[test]
    fn test_int_array_roundtrips() {
        assert_eq!(roundtrip(v!([100, 200, 300])), v!([100, 200, 300]));
    }

    #[test]
    fn test_string_array_roundtrips() {
        assert_eq!(
            roundtrip(v!(["foo", "bar", "baz"])),
            v!(["foo", "bar", "baz"])
        );
    }

    #[test]
    fn test_multirow_int_parses_without_error() {
        written("mat", v!([[1, 2], [3, 4]]), None, None);
    }

    #[test]
    fn test_multirow_roundtrips() {
        let rows = v!([[1, 2], [3, 4]]);
        assert_eq!(roundtrip(rows.clone()), rows);
    }
}

/// `write_array` exercised through `to_string` for list values.
mod write_array_in_dumps {
    use super::*;

    #[test]
    fn test_list_value_in_dumps() {
        let doc = loads(&dumps(&st(vec![("nums", v!([1, 2, 3]))])));
        assert_eq!(field(&doc, "nums"), &v!([1, 2, 3]));
    }

    #[test]
    fn test_nested_list_in_dumps() {
        let doc = loads(&dumps(&st(vec![("mat", v!([[1, 2], [3, 4]]))])));
        assert_eq!(field(&doc, "mat"), &v!([[1, 2], [3, 4]]));
    }

    #[test]
    fn test_list_of_dicts_in_dumps() {
        let list = v!([st(vec![("k", v!(1))]), st(vec![("k", v!(2))])]);
        let doc = loads(&dumps(&st(vec![("items", list.clone())])));
        assert_eq!(field(&doc, "items"), &list);
    }

    #[test]
    fn test_bool_elements_in_array() {
        let out = written("flags", v!([true, false, true]), None, None);
        assert!(has(&out, b"true"));
        assert!(has(&out, b"false"));
    }
}

/// Integers beyond 64 bits are neither truncated nor refused.
mod dumps_big_int {
    use super::*;

    fn rt(key: &str, value: Value) -> Value {
        field(&loads(&dumps(&st(vec![(key, value)]))), key).clone()
    }

    #[test]
    fn test_scalar_bigint_roundtrip() {
        for v in [
            1i128 << 64,
            (1i128 << 64) - 1,
            1i128 << 100,
            -(1i128 << 63) - 1,
            -(1i128 << 100),
            0,
            42,
            -7,
        ] {
            assert_eq!(rt("x", Value::Int(v)), Value::Int(v), "{v}");
        }
    }

    #[test]
    fn test_scalar_bigint_not_truncated_to_zero() {
        // 2**100 mod 2**64 == 0 -- the exact pre-fix corruption.
        assert_eq!(rt("x", Value::Int(1 << 100)), Value::Int(1 << 100));
    }

    #[test]
    fn test_array_bigint_roundtrip() {
        for arr in [
            v!([1, 2, (1i128 << 70)]),
            v!([(1i128 << 100), (-(1i128 << 99))]),
            v!([-1, -2, (1i128 << 70)]),
            v!([[1, (1i128 << 80)], [3, 4]]),
            v!([9_223_372_036_854_775_808_i128, (1i128 << 101)]),
        ] {
            assert_eq!(rt("a", arr.clone()), arr);
        }
    }

    #[test]
    fn test_plain_int_arrays_unannotated() {
        // Values that fit 64 bits keep the unannotated form (no regression).
        let out = to_string(&st(vec![("a", v!([1, 2, 3]))]), false).unwrap();
        assert_eq!(out, b".a=[1,2,3];");
    }
}

/// inf/nan are written as the special-number keywords (inf, ninf, nan).
mod write_array_special_floats {
    use super::*;

    #[test]
    fn test_nan_in_array_does_not_raise() {
        written("v", v!([f64::NAN]), None, None);
    }

    #[test]
    fn test_pos_inf_in_array_does_not_raise() {
        written("v", v!([f64::INFINITY]), None, None);
    }

    #[test]
    fn test_neg_inf_in_array_does_not_raise() {
        written("v", v!([f64::NEG_INFINITY]), None, None);
    }

    #[test]
    fn test_nan_encoded_as_special_literal() {
        let out = written("v", v!([f64::NAN]), None, None);
        assert!(has(&out, b"nan"));
    }

    #[test]
    fn test_pos_inf_encoded_as_special_literal() {
        let out = written("v", v!([f64::INFINITY]), None, None);
        assert!(has(&out, b"inf"));
    }

    #[test]
    fn test_neg_inf_encoded_as_special_literal() {
        let out = written("v", v!([f64::NEG_INFINITY]), None, None);
        assert!(has(&out, b"ninf"));
    }

    #[test]
    fn test_mixed_specials_and_normal_floats() {
        let rows = v!([f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1.5]);
        let out = written("v", rows, None, None);
        for needle in [&b"nan"[..], b"inf", b"ninf", b"1.5"] {
            assert!(has(&out, needle));
        }
    }
}

/// `true`/`false` read back as booleans, not as the symbols' strings.
mod bool_roundtrip {
    use super::*;

    fn rt(doc: Value) -> Value {
        loads(&dumps(&doc))
    }

    #[test]
    fn test_true_roundtrips_to_python_bool() {
        let result = rt(st(vec![("flag", v!(true))]));
        assert_eq!(field(&result, "flag"), &Value::Bool(true));
    }

    #[test]
    fn test_false_roundtrips_to_python_bool() {
        let result = rt(st(vec![("flag", v!(false))]));
        assert_eq!(field(&result, "flag"), &Value::Bool(false));
    }

    #[test]
    fn test_bool_type_not_string() {
        let result = rt(st(vec![("flag", v!(true))]));
        assert!(
            matches!(field(&result, "flag"), Value::Bool(_)),
            "Expected bool, got {result:?}"
        );
    }

    #[test]
    fn test_bool_in_nested_struct() {
        let cfg = st(vec![("enabled", v!(true)), ("debug", v!(false))]);
        let result = rt(st(vec![("cfg", cfg)]));
        let cfg = field(&result, "cfg");
        assert_eq!(field(cfg, "enabled"), &Value::Bool(true));
        assert_eq!(field(cfg, "debug"), &Value::Bool(false));
    }

    #[test]
    fn test_bool_with_sibling_int() {
        let result = rt(st(vec![("active", v!(true)), ("count", v!(5))]));
        assert_eq!(field(&result, "active"), &Value::Bool(true));
        assert_eq!(field(&result, "count"), &v!(5));
    }

    #[test]
    fn test_bool_array_elements_roundtrip() {
        assert_eq!(roundtrip(v!([true, false, true])), v!([true, false, true]));
    }

    #[test]
    fn test_bool_array_element_type() {
        let result = items(roundtrip(v!([true, false])));
        assert!(matches!(result[0], Value::Bool(_)));
        assert!(matches!(result[1], Value::Bool(_)));
    }

    #[test]
    fn test_bool_in_struct_in_array() {
        let rows = v!([
            st(vec![("enabled", v!(true))]),
            st(vec![("enabled", v!(false))])
        ]);
        let result = items(roundtrip(rows));
        assert_eq!(field(&result[0], "enabled"), &Value::Bool(true));
        assert_eq!(field(&result[1], "enabled"), &Value::Bool(false));
    }

    #[test]
    fn test_bool_via_write_array_helper() {
        let out = written("flags", v!([true, false, true]), None, None);
        assert_eq!(field(&loads(&out), "flags"), &v!([true, false, true]));
    }

    #[test]
    fn test_bool_dumps_loads_cycle() {
        let doc = st(vec![
            ("x", v!(true)),
            ("y", v!(false)),
            ("z", v!(42)),
            ("s", v!("hello")),
        ]);
        let result = rt(doc);
        assert_eq!(field(&result, "x"), &Value::Bool(true));
        assert_eq!(field(&result, "y"), &Value::Bool(false));
        assert_eq!(field(&result, "z"), &v!(42));
        assert_eq!(field(&result, "s"), &v!("hello"));
    }

    #[test]
    fn test_raw_bvnr_true_symbol() {
        assert_eq!(
            field(&loads(b".flag = true;\n"), "flag"),
            &Value::Bool(true)
        );
    }

    #[test]
    fn test_raw_bvnr_false_symbol() {
        assert_eq!(
            field(&loads(b".flag = false;\n"), "flag"),
            &Value::Bool(false)
        );
    }

    #[test]
    fn test_other_symbols_remain_strings() {
        assert_eq!(field(&loads(b".status = ok;\n"), "status"), &v!("ok"));
    }
}

/// `to_string` picks an array's text from its whole shape, not from the
/// outer list alone.
mod array_shapes_survive_dumps {
    use super::*;

    fn compact(rows: Value) -> Vec<u8> {
        to_string(&st(vec![("a", rows)]), false).unwrap()
    }

    #[test]
    fn test_nested_lists_round_trip() {
        for rows in [
            v!([[1], [2, 3]]), // was `[1]/[2,3]`: error_array_row_size_mismatch
            v!([[1, 2, 3], [4]]),
            v!([[[1]]]), // was `[[1]]`: one dimension short
            v!([[[1.5]]]),
            v!([[[[7]]]]),
            v!([[0, 1, 2, 3]]), // one row: nested, or it reads as shape (4,)
            v!([[1, 2], [3, 4]]),
            v!([[[1, 2], [3, 4]], [[5, 6], [7, 8]]]),
            v!([[[1], [2, 3]], [[4]]]),
            v!([[], [1]]),
        ] {
            let out = compact(rows.clone());
            assert_eq!(
                loads(&out),
                st(vec![("a", rows)]),
                "{}",
                String::from_utf8_lossy(&out)
            );
        }
    }

    #[test]
    fn test_a_rectangle_is_still_written_as_rows() {
        assert_eq!(compact(v!([[1, 2], [3, 4]])), b".a=[1,2]/[3,4];");
    }

    #[test]
    fn test_ragged_rows_are_nested_not_slash_separated() {
        assert_eq!(compact(v!([[1], [2, 3]])), b".a=[[1],[2,3]];");
    }
}

/// `parse_typed` -> `to_string` on arrays the format takes.
mod typed_arrays_survive_dumps {
    use super::*;

    /// Python's `_again`: (typed doc, compact output, typed re-read).
    fn again(src: &[u8]) -> (Value, Vec<u8>, Value) {
        let doc = parse_typed(src).unwrap();
        let out = to_string(&doc, false).unwrap();
        let back = parse_typed(&out).unwrap();
        (doc, out, back)
    }

    fn plain_values(arr: &Value) -> Vec<Value> {
        let Value::Array(elems) = arr else {
            panic!("not an array: {arr:?}");
        };
        elems
            .iter()
            .map(|e| match e {
                Value::Quantity(q) => q.value(),
                other => panic!("not a quantity: {other:?}"),
            })
            .collect()
    }

    fn unit_of(v: &Value) -> Unit {
        match v {
            Value::Quantity(q) => q.unit,
            other => panic!("not a quantity: {other:?}"),
        }
    }

    #[test]
    fn test_one_unit_several_types_keeps_each_annotation() {
        // Homogeneity is about the unit: every element is in metres.
        let src = b".a=[<uint:8,m>1,<float:64,m>2.5];";
        let (doc, out, back) = again(src);
        assert_eq!(out, src);
        assert_eq!(back, doc);
    }

    #[test]
    fn test_one_type_under_two_spellings_is_one_annotation() {
        // <uint:64> and <uint:64,_10> are one type; m*s and s*m one unit.
        let (_, out, back) = again(b".a=[<uint:64,m>1,<uint:64,_10,m>2];");
        assert_eq!(out, b".a=<uint:64,m>[1,2];");
        assert_eq!(plain_values(field(&back, "a")), vec![v!(1), v!(2)]);
        let (_, out, back) = again(b".a=[<float:64,m*s>1.5,<float:64,s*m>2.5];");
        assert_eq!(out.windows(9).filter(|w| w == b"<float:64").count(), 1);
        let a = items(field(&back, "a").clone());
        assert!(unit_of(&a[0]) == unit_of(&a[1]));
    }

    #[test]
    fn test_two_units_are_still_refused() {
        let doc = parse_typed(b".a=[<float:64,s>1,<float:64,m>3];").unwrap();
        let r = to_string(&doc, true);
        assert!(matches!(r, Err(Error::Argument { .. })), "{r:?}");
    }

    #[test]
    fn test_a_reference_stays_a_reference() {
        // It was written as the string "&.k0".
        let src = b".a=[&.k0,&.k1];.k0=1;.k1=2;";
        let (doc, out, back) = again(src);
        assert_eq!(out, src);
        assert_eq!(back, doc);
    }

    #[test]
    fn test_a_symbol_stays_a_symbol() {
        let src = b".a=[sym,abc];";
        let (_, out, _) = again(src);
        assert_eq!(out, src);
    }

    #[test]
    fn test_a_quoted_literal_stays_quoted() {
        // Every base above ten requires the quotes; ff unquoted is a symbol.
        let src = b".a=<uint:64,_16>[\"ff\",\"10\"];";
        let (_, out, back) = again(src);
        assert_eq!(out, src);
        assert_eq!(plain_values(field(&back, "a")), vec![v!(255), v!(16)]);
    }
}

/// With a type given, every element is held to it.
mod write_array_held_to_annotation {
    use super::*;

    /// Python's `_write`: a compact `.p` array under `spec`.
    fn write(rows: Value, spec: ValueTypeSpec, unit: Option<&Unit>) -> bovnar::Result<Vec<u8>> {
        let mut w = Writer::to_mem(CAP, false)?;
        write_array(&mut w, "p", &items(rows), Some(spec), unit, None)?;
        w.finish()?;
        Ok(w.get_output().to_vec())
    }

    fn assert_refused(rows: Value, spec: ValueTypeSpec) {
        let r = write(rows, spec, None);
        assert!(matches!(r, Err(Error::Argument { .. })), "{r:?}");
    }

    /// The refusal used to come at the bad element, after `.p=<uint:16>[80,443,`
    /// was already out: the writer was left inside an open array and every
    /// later write failed with it. Now the whole array is checked first.
    /// (Python's `10 ** 5000` and `object()` rows have no `Value` form.)
    #[test]
    fn test_a_refusal_writes_nothing() {
        let u16_ = vt(ValueTypeFamily::Uint, 16, 0);
        let cases = [
            (v!([80, 443, 80800]), u16_),
            (Value::Array(vec![v!([1, 2]), v!([3, 1.5])]), u16_),
            (
                Value::Array(vec![1.lit(), 2.lit(), st(vec![("a", 1.lit())])]),
                u16_,
            ),
            (
                Value::Array(vec!["a".lit(), 1.lit()]),
                vt(ValueTypeFamily::Utf8, 0, 0),
            ),
        ];
        for (rows, spec) in cases {
            let mut w = Writer::to_mem(CAP, false).unwrap();
            w.write_string("before", "x").unwrap();
            let mark = w.get_output().to_vec();
            let r = write_array(&mut w, "p", &items(rows), Some(spec), None, None);
            assert!(matches!(r, Err(Error::Argument { .. })), "{r:?}");
            assert_eq!(w.get_output(), mark.as_slice());
            write_array(&mut w, "q", &items(v!([1, 2])), Some(u16_), None, None).unwrap();
            w.write_string("after", "y").unwrap();
            w.finish().unwrap();
            assert_eq!(
                parse(w.get_output()).unwrap(),
                st(vec![
                    ("before", "x".lit()),
                    ("q", v!([1, 2])),
                    ("after", "y".lit()),
                ])
            );
        }
    }

    #[test]
    fn test_out_of_range_element_is_refused() {
        assert_refused(v!([80, 443, 80800]), vt(ValueTypeFamily::Uint, 16, 0));
    }

    #[test]
    fn test_negative_under_uint_is_refused() {
        assert_refused(v!([-1, 1]), vt(ValueTypeFamily::Uint, 8, 0));
    }

    #[test]
    fn test_float_under_integer_annotation_is_refused() {
        assert_refused(v!([1.5]), vt(ValueTypeFamily::Uint, 32, 0));
    }

    #[test]
    fn test_string_under_numeric_annotation_is_refused() {
        assert_refused(v!(["a"]), vt(ValueTypeFamily::Uint, 32, 0));
    }

    #[test]
    fn test_bool_under_numeric_annotation_is_refused() {
        assert_refused(v!([true]), vt(ValueTypeFamily::Uint, 8, 0));
    }

    #[test]
    fn test_in_range_elements_still_write() {
        let rows = v!([[-128, 127], [0, null]]);
        let out = write(rows.clone(), vt(ValueTypeFamily::Sint, 8, 0), None).unwrap();
        assert_eq!(field(&loads(&out), "p"), &rows);
    }

    #[test]
    fn test_non_decimal_base_writes_the_value() {
        let out = write(v!([255, 16]), vt(ValueTypeFamily::Uint, 16, 16), None).unwrap();
        assert_eq!(out, b".p=<uint:16,_16>[\"ff\",\"10\"];");
        assert_eq!(field(&loads(&out), "p"), &v!([255, 16]));
    }

    #[test]
    fn test_ints_under_a_float_annotation_are_floats() {
        let out = write(v!([1, 2]), vt(ValueTypeFamily::Float, 32, 0), None).unwrap();
        assert_eq!(field(&loads(&out), "p"), &v!([1.0, 2.0]));
    }

    #[test]
    fn test_a_unit_object_is_a_unit() {
        let m = Unit::parse("m").unwrap();
        let out = write(v!([1.0]), vt(ValueTypeFamily::Float, 64, 0), Some(&m)).unwrap();
        assert_eq!(out, b".p=<float:64,m>[1.0];");
    }
}
