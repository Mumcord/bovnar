//! Port of `python/tests/test_dom.py`.
//!
// Skipped Python tests:
// - test_repr_contains_ptr, test_repr_contains_type: Python `repr` output.
// - test_getitem_negative_index: Python negative indexing; `DomNode::at`
//   takes a `usize`.
// - test_null_pointer_leaves_nothing_for_del_to_trip_on: Python garbage
//   collection of a half-built object; a Rust `DomDoc` cannot hold a null
//   pointer.
// - test_non_str_key_is_simply_absent: a Python type check (an int key);
//   `get` takes a `&str`.
//
// Python raises KeyError / IndexError / TypeError / ValueError where the Rust
// accessors return `None`; those tests assert `None`.
//
// Python test names shared by several classes carry the class's subject here,
// since the port is one flat module: TestDomNodeIntAccessors /
// TestDomNodeFloatAccessor::test_wrong_type_raises are
// test_as_i64_wrong_type_raises / test_as_float_wrong_type_raises, and
// TestDomNodeStructAccess / TestDomNodeArrayAccess::test_getitem_wrong_type_raises
// are test_struct_getitem_wrong_type_raises /
// test_array_getitem_wrong_type_raises.

use bovnar::{
    DomDoc, DomNode, DomType, Error, ErrorCode, Value, ValueTypeFamily, parse, to_string,
};

const SCALAR_INT: &[u8] = b".x = 42;\n";
const SCALAR_UINT32: &[u8] = b".x = <uint:32> 42;\n";
const SCALAR_INT8: &[u8] = b".x = <sint:8> -5;\n";
const SCALAR_FLOAT: &[u8] = b".v = <float:64,m/s> 9.81;\n";
const SCALAR_STRING: &[u8] = b".s = \"hello\";\n";
const NULL: &[u8] = b".n = ;\n";
const SYMBOL: &[u8] = b".flag = ok;\n";
const STRUCT: &[u8] = b".obj = { .a = 1; .b = 2; };\n";
const ARRAY_1D: &[u8] = b".arr = [1, 2, 3];\n";
const ARRAY_2D: &[u8] = b".mat = [1, 2]/[3, 4];\n";
const NESTED_STRUCT: &[u8] = b".outer = { .inner = { .val = 99; }; };\n";
const MULTI_KEY: &[u8] = b".x = 1;\n.y = 2;\n.z = 3;\n";
const STRUCT_ARRAY: &[u8] = b".recs = [{.a = 10; .b = 20;}];\n";
const TYPED_UNIT: &[u8] = b".temp = <float:64,K> 300.0;\n";

fn doc(src: &[u8]) -> DomDoc {
    DomDoc::parse(src).unwrap()
}

fn int(i: i128) -> Value {
    Value::Int(i)
}

fn obj(pairs: &[(&str, Value)]) -> Value {
    Value::Struct(
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    )
}

fn ints(values: &[i128]) -> Value {
    Value::Array(values.iter().map(|&i| Value::Int(i)).collect())
}

fn is_close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * a.abs().max(b.abs())
}

fn keys_of(entries: &[(String, DomNode<'_>)]) -> Vec<String> {
    entries.iter().map(|(k, _)| k.clone()).collect()
}

// TestDomDocParse

#[test]
fn test_parse_bytes() {
    DomDoc::parse(SCALAR_INT).unwrap();
}

#[test]
fn test_parse_bytearray() {
    let owned: Vec<u8> = SCALAR_INT.to_vec();
    assert_eq!(DomDoc::parse(&owned).unwrap().len(), 1);
}

/// `DomDoc::parse_reader` over a temporary file holding `data`.
fn parse_via_file(name: &str, data: &[u8]) -> Result<DomDoc, Error> {
    let path = std::env::temp_dir().join(format!("bovnar-test-{name}-{}.bvnr", std::process::id()));
    std::fs::write(&path, data).unwrap();
    let doc = DomDoc::parse_reader(std::fs::File::open(&path).unwrap());
    std::fs::remove_file(&path).unwrap();
    doc
}

#[test]
fn test_parse_file() {
    // Python's parse_file(path) is parse_reader over the opened File.
    assert_eq!(parse_via_file("dom", SCALAR_INT).unwrap().len(), 1);
}

#[test]
fn test_parse_fd() {
    // Python parses an fd opened on a temporary file; Rust reads the File.
    assert_eq!(parse_via_file("dom-fd", MULTI_KEY).unwrap().len(), 3);
}

#[test]
fn test_invalid_bvnr_raises() {
    assert!(matches!(
        DomDoc::parse(b"!!! this is not valid BVNR !!!"),
        Err(Error::Parse { .. })
    ));
}

#[test]
fn test_spec_1_0_homogeneity_raises() {
    // The materialised-document rules (spec 1.0 §7.4 / §8.1) carry frozen
    // error codes; the DOM parser surfaces them as parse errors.
    let cases: [(&[u8], ErrorCode); 6] = [
        (b".a = [1, \"two\"];", ErrorCode::ArrayElementTypeMismatch),
        (
            b".a = [<float:64,m> 1.0, <float:64,k~g> 2.0];",
            ErrorCode::ArrayElementTypeMismatch,
        ),
        (
            b".a = [[1, 2], [3, 4, 5]];",
            ErrorCode::ArrayRowSizeMismatch,
        ),
        (
            b".a = [{.x = 1;}, {.y = 1;}];",
            ErrorCode::StructShapeMismatch,
        ),
        (b".x = 1; .x = 2;", ErrorCode::DuplicateStructKey),
        (b".s = {.x = 1; .x = 2;};", ErrorCode::DuplicateStructKey),
    ];
    for (src, code) in cases {
        let err = DomDoc::parse(src).err().unwrap();
        assert!(matches!(err, Error::Parse { .. }), "{err:?}");
        assert_eq!(err.code(), Some(code), "{}", String::from_utf8_lossy(src));
    }
}

#[test]
fn test_empty_doc() {
    assert_eq!(doc(b"").len(), 0);
}

// TestDomDocAccess

#[test]
fn test_len() {
    assert_eq!(doc(MULTI_KEY).len(), 3);
}

#[test]
fn test_contains_existing_key() {
    assert!(doc(SCALAR_INT).get("x").is_some());
}

#[test]
fn test_contains_missing_key() {
    assert!(doc(SCALAR_INT).get("missing").is_none());
}

#[test]
fn test_getitem_returns_node() {
    assert!(doc(SCALAR_INT).get("x").is_some());
}

#[test]
fn test_getitem_missing_raises_keyerror() {
    assert!(doc(SCALAR_INT).get("no_such_key").is_none());
}

#[test]
fn test_entries_returns_list_of_tuples() {
    let d = doc(MULTI_KEY);
    let keys = keys_of(&d.entries());
    assert_eq!(keys.len(), 3);
    for k in ["x", "y", "z"] {
        assert!(keys.contains(&k.to_string()), "{k}");
    }
}

#[test]
fn test_iter_yields_tuples() {
    let d = doc(MULTI_KEY);
    let pairs = d.entries();
    assert_eq!(pairs.len(), 3);
    for (k, v) in pairs {
        assert!(!k.is_empty());
        assert!(v.dom_type().is_some());
    }
}

#[test]
fn test_to_dict_scalar() {
    assert_eq!(doc(SCALAR_INT).to_value().unwrap(), obj(&[("x", int(42))]));
}

#[test]
fn test_to_dict_multi() {
    assert_eq!(
        doc(MULTI_KEY).to_value().unwrap(),
        obj(&[("x", int(1)), ("y", int(2)), ("z", int(3))])
    );
}

// TestDomDocLookup

#[test]
fn test_lookup_top_level() {
    assert!(doc(SCALAR_INT).lookup("x").is_some());
}

#[test]
fn test_lookup_missing_returns_none() {
    assert!(doc(SCALAR_INT).lookup("no_such").is_none());
}

#[test]
fn test_lookup_nested_dot_path() {
    let d = doc(NESTED_STRUCT);
    assert_eq!(d.lookup("outer.inner.val").unwrap().as_i64(), Some(99));
}

#[test]
fn test_lookup_one_level_deep() {
    let d = doc(NESTED_STRUCT);
    assert_eq!(
        d.lookup("outer.inner").unwrap().dom_type(),
        Some(DomType::Struct)
    );
}

#[test]
fn test_lookup_array_index() {
    let d = doc(b"#!bovnar 1.1\n.matrix = [10, 20, 30]/[40, 50, 60];\n");
    assert_eq!(d.lookup("matrix[0][1]").unwrap().as_i64(), Some(20));
    assert_eq!(d.lookup("matrix[1][0]").unwrap().as_i64(), Some(40));
    assert!(d.lookup("matrix[0]").is_none()); // partial index of a matrix
    assert!(d.lookup("matrix[0][9]").is_none()); // out of range
}

#[test]
fn test_lookup_1d_and_nested_index() {
    let d = doc(b"#!bovnar 1.1\n.a = [1, 2, 3];\n.n = [[1, 2], [3, 4]];\n");
    assert_eq!(d.lookup("a[2]").unwrap().as_i64(), Some(3));
    assert_eq!(d.lookup("n[0][1]").unwrap().as_i64(), Some(2));
}

#[test]
fn test_lookup_index_then_field() {
    let d = doc(b"#!bovnar 1.1\n.rows = [{.name = \"a\";}, {.name = \"b\";}];\n");
    assert_eq!(d.lookup("rows[1].name").unwrap().as_str(), Some("b"));
}

// TestDomNodeType

fn type_of(src: &[u8], key: &str) -> Option<DomType> {
    doc(src).get(key).unwrap().dom_type()
}

#[test]
fn test_int_type() {
    assert_eq!(type_of(SCALAR_INT, "x"), Some(DomType::Int));
}

#[test]
fn test_float_type() {
    assert_eq!(type_of(SCALAR_FLOAT, "v"), Some(DomType::Float));
}

#[test]
fn test_string_type() {
    assert_eq!(type_of(SCALAR_STRING, "s"), Some(DomType::String));
}

#[test]
fn test_null_type() {
    assert_eq!(type_of(NULL, "n"), Some(DomType::Null));
}

#[test]
fn test_symbol_type() {
    assert_eq!(type_of(SYMBOL, "flag"), Some(DomType::Symbol));
}

#[test]
fn test_struct_type() {
    assert_eq!(type_of(STRUCT, "obj"), Some(DomType::Struct));
}

#[test]
fn test_array_type() {
    assert_eq!(type_of(ARRAY_1D, "arr"), Some(DomType::Array));
}

#[test]
fn test_slash_matrix_row_count_uncapped() {
    // The DOM used to clamp the recorded /-row count at 8; array_dims() must
    // report the true number of rows.
    let rows: Vec<String> = (0..10).map(|i| format!("[{},{}]", i, i + 1)).collect();
    let src = format!(".m=<uint:16>{};", rows.join("/"));
    let d = doc(src.as_bytes());
    let node = d.get("m").unwrap();
    assert_eq!(node.dom_type(), Some(DomType::Array));
    assert_eq!(node.array_dims(), 10);
    assert_eq!(node.len(), 20);
}

// TestDomNodeIntAccessors

#[test]
fn test_as_i64() {
    assert_eq!(doc(SCALAR_INT).get("x").unwrap().as_i64(), Some(42));
}

#[test]
fn test_as_u64_positive() {
    assert_eq!(doc(SCALAR_INT).get("x").unwrap().as_u64(), Some(42));
}

#[test]
fn test_as_i32() {
    assert_eq!(doc(SCALAR_UINT32).get("x").unwrap().as_i32(), Some(42));
}

#[test]
fn test_as_u32() {
    assert_eq!(doc(SCALAR_UINT32).get("x").unwrap().as_u32(), Some(42));
}

#[test]
fn test_as_i8_negative() {
    assert_eq!(doc(SCALAR_INT8).get("x").unwrap().as_i8(), Some(-5));
}

#[test]
fn test_as_u8_small() {
    let d = doc(b".x = <uint:8> 200;\n");
    assert_eq!(d.get("x").unwrap().as_u8(), Some(200));
}

#[test]
fn test_as_i16() {
    let d = doc(b".x = <sint:16> -1000;\n");
    assert_eq!(d.get("x").unwrap().as_i16(), Some(-1000));
}

#[test]
fn test_as_u16() {
    let d = doc(b".x = <uint:16> 8080;\n");
    assert_eq!(d.get("x").unwrap().as_u16(), Some(8080));
}

#[test]
fn test_as_i64_wrong_type_raises() {
    assert_eq!(doc(SCALAR_STRING).get("s").unwrap().as_i64(), None);
}

#[test]
fn test_as_int_str_base10() {
    assert_eq!(
        doc(SCALAR_INT).get("x").unwrap().as_int_str(10).as_deref(),
        Some("42")
    );
}

#[test]
fn test_as_int_str_base16() {
    let d = doc(b".x = <uint:8> 255;\n");
    let s = d.get("x").unwrap().as_int_str(16).unwrap().to_lowercase();
    assert!(s == "ff" || s == "0xff", "{s}");
}

#[test]
fn test_to_python_uint64_high_bit() {
    // Values are read by declared signedness, so a uint64 >= 2**63 does not
    // come back as a negative two's-complement number.
    let cases: [(i128, &[u8]); 3] = [
        (
            18446744073709551615,
            b".x = <uint:64> 18446744073709551615;\n",
        ),
        (
            9223372036854775808,
            b".x = <uint:64> 9223372036854775808;\n",
        ),
        (
            9223372036854775809,
            b".x = <uint:64> 9223372036854775809;\n",
        ),
    ];
    for (want, src) in cases {
        let d = doc(src);
        assert_eq!(
            d.get("x").unwrap().to_value().unwrap(),
            int(want),
            "to_python"
        );
        assert_eq!(d.to_value().unwrap(), obj(&[("x", int(want))]), "to_dict");
    }
}

#[test]
fn test_to_python_sint_and_bignum() {
    let d = doc(b".x = <sint:64> -5;\n");
    assert_eq!(d.get("x").unwrap().to_value().unwrap(), int(-5));
    for want in [1i128 << 100, -(1i128 << 70)] {
        let src = to_string(&obj(&[("x", int(want))]), true).unwrap();
        assert_eq!(doc(&src).get("x").unwrap().to_value().unwrap(), int(want));
    }
}

#[test]
fn test_to_python_datetime_is_signed() {
    // datetime is a signed epoch-seconds carrier: a pre-1970 date is negative.
    let d = doc(b"#!bovnar 1.1\n.t = 1960-01-01;\n");
    assert_eq!(d.get("t").unwrap().to_value().unwrap(), int(-315619200));
    let d2 = doc(b"#!bovnar 1.1\n.t = <datetime:64,unix> 1750000000;\n");
    assert_eq!(d2.get("t").unwrap().to_value().unwrap(), int(1750000000));
}

// TestDomNodeFloatAccessor

#[test]
fn test_as_float() {
    let v = doc(SCALAR_FLOAT).get("v").unwrap().as_float().unwrap();
    assert!(is_close(v, 9.81, 1e-9), "{v}");
}

#[test]
fn test_as_float_wrong_type_raises() {
    assert_eq!(doc(SCALAR_STRING).get("s").unwrap().as_float(), None);
}

// TestDomNodeStringAccessors

#[test]
fn test_as_str_string() {
    assert_eq!(doc(SCALAR_STRING).get("s").unwrap().as_str(), Some("hello"));
}

#[test]
fn test_as_str_symbol() {
    assert_eq!(doc(SYMBOL).get("flag").unwrap().as_str(), Some("ok"));
}

#[test]
fn test_as_str_wrong_type_raises() {
    assert_eq!(doc(SCALAR_INT).get("x").unwrap().as_str(), None);
}

// TestDomNodeNullIsNull

#[test]
fn test_is_null_true() {
    assert!(doc(NULL).get("n").unwrap().is_null());
}

#[test]
fn test_is_null_false() {
    assert!(!doc(SCALAR_INT).get("x").unwrap().is_null());
}

// TestDomNodeUnit

#[test]
fn test_unit_str_kelvin() {
    assert!(
        doc(TYPED_UNIT)
            .get("temp")
            .unwrap()
            .unit_str()
            .contains('K')
    );
}

#[test]
fn test_value_type_family_float() {
    let vt = doc(TYPED_UNIT).get("temp").unwrap().value_type();
    assert_eq!(
        ValueTypeFamily::from_raw(vt.family),
        Some(ValueTypeFamily::Float)
    );
}

#[test]
fn test_value_in_base_units_kelvin() {
    let v = doc(TYPED_UNIT).get("temp").unwrap().value_in_base_units();
    assert!(is_close(v, 300.0, 1e-6), "{v}");
}

#[test]
fn test_value_in_base_units_km() {
    let v = doc(b".d = <uint:32,k~m> 5;\n")
        .get("d")
        .unwrap()
        .value_in_base_units();
    assert!(is_close(v, 5000.0, 1e-6), "{v}");
}

#[test]
fn test_no_unit_str_empty() {
    let _: String = doc(SCALAR_INT).get("x").unwrap().unit_str();
}

// TestDomNodeStructAccess

#[test]
fn test_len_struct() {
    assert_eq!(doc(STRUCT).get("obj").unwrap().len(), 2);
}

#[test]
fn test_getitem_str_key() {
    let d = doc(STRUCT);
    assert_eq!(d.get("obj").unwrap().get("a").unwrap().as_i64(), Some(1));
}

#[test]
fn test_getitem_missing_struct_key_raises() {
    assert!(doc(STRUCT).get("obj").unwrap().get("no_such").is_none());
}

#[test]
fn test_contains_struct_key() {
    let d = doc(STRUCT);
    let o = d.get("obj").unwrap();
    assert!(o.get("a").is_some());
    assert!(o.get("z").is_none());
}

#[test]
fn test_entries_struct() {
    let d = doc(STRUCT);
    let mut keys = keys_of(&d.get("obj").unwrap().entries());
    keys.sort();
    assert_eq!(keys, ["a", "b"]);
}

#[test]
fn test_iter_struct() {
    let d = doc(STRUCT);
    let pairs = d.get("obj").unwrap().entries();
    assert_eq!(pairs.len(), 2);
    for (k, v) in pairs {
        assert!(!k.is_empty());
        assert!(v.dom_type().is_some());
    }
}

#[test]
fn test_struct_getitem_wrong_type_raises() {
    assert!(doc(SCALAR_INT).get("x").unwrap().get("key").is_none());
}

// TestDomNodeArrayAccess

#[test]
fn test_len_array() {
    assert_eq!(doc(ARRAY_1D).get("arr").unwrap().len(), 3);
}

#[test]
fn test_getitem_int_index() {
    let d = doc(ARRAY_1D);
    assert_eq!(d.get("arr").unwrap().at(0).unwrap().as_i64(), Some(1));
}

#[test]
fn test_getitem_last_element() {
    let d = doc(ARRAY_1D);
    assert_eq!(d.get("arr").unwrap().at(2).unwrap().as_i64(), Some(3));
}

#[test]
fn test_getitem_out_of_range_raises() {
    assert!(doc(ARRAY_1D).get("arr").unwrap().at(99).is_none());
}

#[test]
fn test_iter_array() {
    let d = doc(ARRAY_1D);
    let arr = d.get("arr").unwrap();
    let nodes: Vec<DomNode<'_>> = (0..arr.len()).filter_map(|i| arr.at(i)).collect();
    assert_eq!(nodes.len(), 3);
}

#[test]
fn test_array_values_correct() {
    let d = doc(ARRAY_1D);
    let arr = d.get("arr").unwrap();
    let values: Vec<Option<i64>> = (0..3).map(|i| arr.at(i).unwrap().as_i64()).collect();
    assert_eq!(values, [Some(1), Some(2), Some(3)]);
}

#[test]
fn test_2d_array_count() {
    assert_eq!(doc(ARRAY_2D).get("mat").unwrap().len(), 4);
}

#[test]
fn test_array_dims_1d() {
    assert_eq!(doc(ARRAY_1D).get("arr").unwrap().array_dims(), 1);
}

#[test]
fn test_array_dims_2d() {
    assert_eq!(doc(ARRAY_2D).get("mat").unwrap().array_dims(), 2);
}

#[test]
fn test_array_getitem_wrong_type_raises() {
    assert!(doc(SCALAR_INT).get("x").unwrap().at(0).is_none());
}

#[test]
fn test_getitem_str_key_on_array_raises() {
    assert!(doc(ARRAY_1D).get("arr").unwrap().get("bad").is_none());
}

// TestDomNodeToPython

#[test]
fn test_int_scalar() {
    assert_eq!(
        doc(SCALAR_INT).get("x").unwrap().to_value().unwrap(),
        int(42)
    );
}

#[test]
fn test_float_scalar() {
    match doc(SCALAR_FLOAT).get("v").unwrap().to_value().unwrap() {
        Value::Float(v) => assert!(is_close(v, 9.81, 1e-9), "{v}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn test_string_scalar() {
    assert_eq!(
        doc(SCALAR_STRING).get("s").unwrap().to_value().unwrap(),
        Value::String("hello".into())
    );
}

#[test]
fn test_null() {
    assert_eq!(doc(NULL).get("n").unwrap().to_value().unwrap(), Value::Null);
}

#[test]
fn test_struct() {
    assert_eq!(
        doc(STRUCT).get("obj").unwrap().to_value().unwrap(),
        obj(&[("a", int(1)), ("b", int(2))])
    );
}

#[test]
fn test_array() {
    assert_eq!(
        doc(ARRAY_1D).get("arr").unwrap().to_value().unwrap(),
        ints(&[1, 2, 3])
    );
}

#[test]
fn test_nested_struct() {
    assert_eq!(
        doc(NESTED_STRUCT).get("outer").unwrap().to_value().unwrap(),
        obj(&[("inner", obj(&[("val", int(99))]))])
    );
}

#[test]
fn test_struct_in_array() {
    assert_eq!(
        doc(STRUCT_ARRAY).get("recs").unwrap().to_value().unwrap(),
        Value::Array(vec![obj(&[("a", int(10)), ("b", int(20))])])
    );
}

// TestDomDocGcSafety

#[test]
fn test_node_survives_doc_in_scope() {
    let d = doc(SCALAR_INT);
    let node = d.get("x").unwrap();
    assert_eq!(node.as_i64(), Some(42));
}

#[test]
fn test_multiple_nodes_from_same_doc() {
    let d = doc(MULTI_KEY);
    let nodes: Vec<DomNode<'_>> = ["x", "y", "z"].iter().map(|k| d.get(k).unwrap()).collect();
    let values: Vec<Option<i64>> = nodes.iter().map(|n| n.as_i64()).collect();
    assert_eq!(values, [Some(1), Some(2), Some(3)]);
}

// TestDomSmallEdges

#[test]
fn test_empty_rows_keep_their_count() {
    // The streaming tier gives [[], []]; the DOM must keep the row count of
    // an empty matrix too.
    let src = b".a=[]/[];.b=[];.c=[]/[]/[];";
    let empty = || Value::Array(vec![]);
    assert_eq!(doc(src).to_value().unwrap(), parse(src).unwrap());
    assert_eq!(
        parse(src).unwrap(),
        obj(&[
            ("a", Value::Array(vec![empty(), empty()])),
            ("b", empty()),
            ("c", Value::Array(vec![empty(), empty(), empty()])),
        ])
    );
}

/// doc/09 §13.3 promises an empty unit string for a value with no unit, as
/// `Quantity::unit_str` gives; the DOM must not return the C formatter's
/// literal `no_unit`.
#[test]
fn test_unit_str_agrees_with_quantity() {
    let src: &[u8] =
        b".a = 1; .b = <uint:32,no_unit> 7; .s = \"x\"; .m = 5 m; .p = 5 %; .t = { .q = 1; };";
    let doc = DomDoc::parse(src).unwrap();
    let Value::Struct(typed) = bovnar::parse_typed(src).unwrap() else {
        panic!("not a struct");
    };
    let unit_of = |key: &str| doc.get(key).unwrap().unit_str();
    for (k, v) in &typed {
        if let Value::Quantity(q) = v {
            assert_eq!(unit_of(k), q.unit_str(), "{k}");
        }
    }
    assert_eq!(unit_of("a"), "");
    assert_eq!(unit_of("b"), "");
    assert_eq!(unit_of("m"), "m");
    assert_eq!(unit_of("p"), "%");
    assert_eq!(unit_of("t"), "");
}
