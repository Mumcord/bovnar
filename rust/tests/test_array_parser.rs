//! Port of `python/tests/test_array_parser.py`: the dict parser driven by
//! hand-built event sequences, without the C reader.

use bovnar::dict_parser::{ArrScope, DictParser};
use bovnar::{Event, EventData, Value, ValueTypeFamily, ValueTypeSpec};

/// One fake event: its kind and the raw payload with its type family/base.
type Ev = (Event, Option<(String, ValueTypeFamily, u32)>);

fn d(raw: &str, family: ValueTypeFamily, base: u32) -> Option<(String, ValueTypeFamily, u32)> {
    Some((raw.to_string(), family, base))
}

fn uint(n: u64) -> Option<(String, ValueTypeFamily, u32)> {
    d(&n.to_string(), ValueTypeFamily::Uint, 10)
}

/// The literal as Python's `repr(float)` writes it.
fn float(v: &str) -> Option<(String, ValueTypeFamily, u32)> {
    d(v, ValueTypeFamily::Float, 0)
}

fn string(s: &str) -> Option<(String, ValueTypeFamily, u32)> {
    d(s, ValueTypeFamily::Utf8, 0)
}

fn key(k: &str) -> Option<(String, ValueTypeFamily, u32)> {
    d(k, ValueTypeFamily::Plain, 0)
}

const AS: Event = Event::AssignmentStart;
const ARS: Event = Event::ArrayRowStart;
const ARE: Event = Event::ArrayRowEnd;
const ADS: Event = Event::ArrayDimStart;
const SS: Event = Event::StructStart;
const SE: Event = Event::StructEnd;
const D: Event = Event::Data;

fn run(events: Vec<Ev>) -> Value {
    let mut p = DictParser::new(false);
    for (event, data) in events {
        let (raw, family, base) = data.unwrap_or((String::new(), ValueTypeFamily::Plain, 0));
        p.on_event(&EventData::plain(
            event,
            ValueTypeSpec {
                family: family as i32,
                width: 0,
                base,
            },
            raw.as_bytes(),
        ));
    }
    p.result()
}

fn int(i: i128) -> Value {
    Value::Int(i)
}

fn arr(items: Vec<Value>) -> Value {
    Value::Array(items)
}

fn ints(values: &[i128]) -> Value {
    arr(values.iter().map(|&i| Value::Int(i)).collect())
}

fn obj(pairs: Vec<(&str, Value)>) -> Value {
    Value::Struct(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn seal(rows: Vec<Vec<Value>>) -> Value {
    let mut scope = ArrScope::default();
    scope.rows = rows;
    scope.seal()
}

fn int_rows(rows: &[&[i128]]) -> Vec<Vec<Value>> {
    rows.iter()
        .map(|r| r.iter().map(|&i| Value::Int(i)).collect())
        .collect()
}

// TestSealArray

#[test]
fn test_single_row_unwraps() {
    assert_eq!(seal(int_rows(&[&[1, 2, 3]])), ints(&[1, 2, 3]));
}

#[test]
fn test_multi_row_stays_nested() {
    assert_eq!(
        seal(int_rows(&[&[1, 2], &[3, 4]])),
        arr(vec![ints(&[1, 2]), ints(&[3, 4])])
    );
}

#[test]
fn test_empty_single_row() {
    assert_eq!(seal(vec![vec![]]), arr(vec![]));
}

#[test]
fn test_three_rows() {
    assert_eq!(
        seal(int_rows(&[&[1], &[2], &[3]])),
        arr(vec![ints(&[1]), ints(&[2]), ints(&[3])])
    );
}

// TestFlatArrays

#[test]
fn test_int_row() {
    let r = run(vec![
        (AS, key("v")),
        (ARS, None),
        (D, uint(1)),
        (D, uint(2)),
        (D, uint(3)),
        (ARE, None),
    ]);
    assert_eq!(r, obj(vec![("v", ints(&[1, 2, 3]))]));
}

#[test]
fn test_float_row() {
    let r = run(vec![
        (AS, key("f")),
        (ARS, None),
        (D, float("1.5")),
        (D, float("2.5")),
        (ARE, None),
    ]);
    assert_eq!(
        r,
        obj(vec![("f", arr(vec![Value::Float(1.5), Value::Float(2.5)]))])
    );
}

#[test]
fn test_string_row() {
    let r = run(vec![
        (AS, key("tags")),
        (ARS, None),
        (D, string("a")),
        (D, string("b")),
        (ARE, None),
    ]);
    assert_eq!(
        r,
        obj(vec![(
            "tags",
            arr(vec![Value::String("a".into()), Value::String("b".into())])
        )])
    );
}

#[test]
fn test_single_element() {
    let r = run(vec![(AS, key("x")), (ARS, None), (D, uint(7)), (ARE, None)]);
    assert_eq!(r, obj(vec![("x", ints(&[7]))]));
}

#[test]
fn test_empty_row() {
    let r = run(vec![(AS, key("empty")), (ARS, None), (ARE, None)]);
    assert_eq!(r, obj(vec![("empty", arr(vec![]))]));
}

#[test]
fn test_mixed_int_float() {
    let r = run(vec![
        (AS, key("m")),
        (ARS, None),
        (D, uint(1)),
        (D, float("2.0")),
        (ARE, None),
    ]);
    assert_eq!(r, obj(vec![("m", arr(vec![int(1), Value::Float(2.0)]))]));
}

// TestMultiRowArrays

#[test]
fn test_2x2_matrix() {
    let r = run(vec![
        (AS, key("mat")),
        (ARS, None),
        (D, uint(1)),
        (D, uint(2)),
        (ARE, None),
        (ADS, None),
        (ARS, None),
        (D, uint(3)),
        (D, uint(4)),
        (ARE, None),
    ]);
    assert_eq!(
        r,
        obj(vec![("mat", arr(vec![ints(&[1, 2]), ints(&[3, 4])]))])
    );
}

#[test]
fn test_3x3_matrix() {
    let mut events = vec![(AS, key("m"))];
    for (i, row) in [[1, 2, 3], [4, 5, 6], [7, 8, 9]].iter().enumerate() {
        if i > 0 {
            events.push((ADS, None));
        }
        events.push((ARS, None));
        events.extend(row.iter().map(|&n| (D, uint(n))));
        events.push((ARE, None));
    }
    assert_eq!(
        run(events),
        obj(vec![(
            "m",
            arr(vec![ints(&[1, 2, 3]), ints(&[4, 5, 6]), ints(&[7, 8, 9])])
        )])
    );
}

#[test]
fn test_uneven_rows() {
    let r = run(vec![
        (AS, key("u")),
        (ARS, None),
        (D, uint(1)),
        (ARE, None),
        (ADS, None),
        (ARS, None),
        (D, uint(2)),
        (D, uint(3)),
        (D, uint(4)),
        (ARE, None),
    ]);
    assert_eq!(r, obj(vec![("u", arr(vec![ints(&[1]), ints(&[2, 3, 4])]))]));
}

#[test]
fn test_dim_continue_resets_row() {
    let r = run(vec![
        (AS, key("d")),
        (ARS, None),
        (D, uint(10)),
        (ARE, None),
        (ADS, None),
        (ARS, None),
        (D, uint(20)),
        (ARE, None),
        (ADS, None),
        (ARS, None),
        (D, uint(30)),
        (ARE, None),
    ]);
    assert_eq!(
        r,
        obj(vec![(
            "d",
            arr(vec![ints(&[10]), ints(&[20]), ints(&[30])])
        )])
    );
}

// TestNestedArrays

#[test]
fn test_nested_two_inner_rows() {
    let r = run(vec![
        (AS, key("n")),
        (ARS, None),
        (ARS, None),
        (D, uint(1)),
        (D, uint(2)),
        (ARE, None),
        (ARS, None),
        (D, uint(3)),
        (D, uint(4)),
        (ARE, None),
        (ARE, None),
    ]);
    assert_eq!(r, obj(vec![("n", arr(vec![ints(&[1, 2]), ints(&[3, 4])]))]));
}

#[test]
fn test_nested_three_inner_rows() {
    let r = run(vec![
        (AS, key("n")),
        (ARS, None),
        (ARS, None),
        (D, uint(1)),
        (ARE, None),
        (ARS, None),
        (D, uint(2)),
        (ARE, None),
        (ARS, None),
        (D, uint(3)),
        (ARE, None),
        (ARE, None),
    ]);
    assert_eq!(
        r,
        obj(vec![("n", arr(vec![ints(&[1]), ints(&[2]), ints(&[3])]))])
    );
}

#[test]
fn test_nested_empty_inner() {
    let r = run(vec![
        (AS, key("n")),
        (ARS, None),
        (ARS, None),
        (ARE, None),
        (ARE, None),
    ]);
    assert_eq!(r, obj(vec![("n", arr(vec![arr(vec![])]))]));
}

// TestStructElements

#[test]
fn test_one_struct_element() {
    let r = run(vec![
        (AS, key("recs")),
        (ARS, None),
        (SS, None),
        (AS, key("x")),
        (D, uint(1)),
        (SE, None),
        (ARE, None),
    ]);
    assert_eq!(r, obj(vec![("recs", arr(vec![obj(vec![("x", int(1))])]))]));
}

#[test]
fn test_two_struct_elements() {
    let r = run(vec![
        (AS, key("recs")),
        (ARS, None),
        (SS, None),
        (AS, key("x")),
        (D, uint(1)),
        (SE, None),
        (SS, None),
        (AS, key("y")),
        (D, uint(2)),
        (SE, None),
        (ARE, None),
    ]);
    assert_eq!(
        r,
        obj(vec![(
            "recs",
            arr(vec![obj(vec![("x", int(1))]), obj(vec![("y", int(2))])])
        )])
    );
}

#[test]
fn test_struct_with_multiple_fields() {
    let r = run(vec![
        (AS, key("items")),
        (ARS, None),
        (SS, None),
        (AS, key("a")),
        (D, uint(10)),
        (AS, key("b")),
        (D, uint(20)),
        (SE, None),
        (ARE, None),
    ]);
    assert_eq!(
        r,
        obj(vec![(
            "items",
            arr(vec![obj(vec![("a", int(10)), ("b", int(20))])])
        )])
    );
}

#[test]
fn test_struct_with_nested_array_field() {
    let r = run(vec![
        (AS, key("data")),
        (ARS, None),
        (SS, None),
        (AS, key("vals")),
        (ARS, None),
        (D, uint(10)),
        (D, uint(20)),
        (ARE, None),
        (SE, None),
        (ARE, None),
    ]);
    assert_eq!(
        r,
        obj(vec![(
            "data",
            arr(vec![obj(vec![("vals", ints(&[10, 20]))])])
        )])
    );
}

// TestSealTiming

#[test]
fn test_trailing_array_no_following_event() {
    let r = run(vec![
        (AS, key("last")),
        (ARS, None),
        (D, uint(9)),
        (ARE, None),
    ]);
    assert_eq!(r, obj(vec![("last", ints(&[9]))]));
}

#[test]
fn test_array_sealed_by_next_assignment() {
    let r = run(vec![
        (AS, key("a")),
        (ARS, None),
        (D, uint(1)),
        (ARE, None),
        (AS, key("b")),
        (D, uint(2)),
    ]);
    assert_eq!(r, obj(vec![("a", ints(&[1])), ("b", int(2))]));
}

#[test]
fn test_struct_trailing_array() {
    let r = run(vec![
        (AS, key("s")),
        (SS, None),
        (AS, key("arr")),
        (ARS, None),
        (D, uint(7)),
        (ARE, None),
        (SE, None),
    ]);
    assert_eq!(r, obj(vec![("s", obj(vec![("arr", ints(&[7]))]))]));
}

#[test]
fn test_struct_array_followed_by_scalar() {
    let r = run(vec![
        (AS, key("s")),
        (SS, None),
        (AS, key("arr")),
        (ARS, None),
        (D, uint(1)),
        (D, uint(2)),
        (ARE, None),
        (AS, key("n")),
        (D, uint(99)),
        (SE, None),
    ]);
    assert_eq!(
        r,
        obj(vec![(
            "s",
            obj(vec![("arr", ints(&[1, 2])), ("n", int(99))])
        )])
    );
}

#[test]
fn test_two_consecutive_top_level_arrays() {
    let r = run(vec![
        (AS, key("x")),
        (ARS, None),
        (D, uint(1)),
        (ARE, None),
        (AS, key("y")),
        (ARS, None),
        (D, uint(2)),
        (ARE, None),
    ]);
    assert_eq!(r, obj(vec![("x", ints(&[1])), ("y", ints(&[2]))]));
}

#[test]
fn test_three_consecutive_top_level_arrays() {
    let mut events = Vec::new();
    for (k, n) in [("a", 1), ("b", 2), ("c", 3)] {
        events.extend([(AS, key(k)), (ARS, None), (D, uint(n)), (ARE, None)]);
    }
    assert_eq!(
        run(events),
        obj(vec![
            ("a", ints(&[1])),
            ("b", ints(&[2])),
            ("c", ints(&[3]))
        ])
    );
}

// TestMixedDocuments

#[test]
fn test_scalar_then_array_then_scalar() {
    let r = run(vec![
        (AS, key("a")),
        (D, uint(10)),
        (AS, key("b")),
        (ARS, None),
        (D, uint(1)),
        (D, uint(2)),
        (ARE, None),
        (AS, key("c")),
        (D, uint(20)),
    ]);
    assert_eq!(
        r,
        obj(vec![("a", int(10)), ("b", ints(&[1, 2])), ("c", int(20))])
    );
}

#[test]
fn test_struct_then_array() {
    let r = run(vec![
        (AS, key("cfg")),
        (SS, None),
        (AS, key("x")),
        (D, uint(5)),
        (SE, None),
        (AS, key("ids")),
        (ARS, None),
        (D, uint(1)),
        (D, uint(2)),
        (D, uint(3)),
        (ARE, None),
    ]);
    assert_eq!(
        r,
        obj(vec![
            ("cfg", obj(vec![("x", int(5))])),
            ("ids", ints(&[1, 2, 3]))
        ])
    );
}

#[test]
fn test_deeply_nested_struct_with_array() {
    let r = run(vec![
        (AS, key("a")),
        (SS, None),
        (AS, key("b")),
        (SS, None),
        (AS, key("items")),
        (ARS, None),
        (D, uint(1)),
        (D, uint(2)),
        (ARE, None),
        (SE, None),
        (SE, None),
    ]);
    assert_eq!(
        r,
        obj(vec![(
            "a",
            obj(vec![("b", obj(vec![("items", ints(&[1, 2]))]))])
        )])
    );
}

#[test]
fn test_array_of_structs_multi_row() {
    let r = run(vec![
        (AS, key("recs")),
        (ARS, None),
        (SS, None),
        (AS, key("v")),
        (D, uint(1)),
        (SE, None),
        (ARE, None),
        (ADS, None),
        (ARS, None),
        (SS, None),
        (AS, key("v")),
        (D, uint(2)),
        (SE, None),
        (ARE, None),
    ]);
    assert_eq!(
        r,
        obj(vec![(
            "recs",
            arr(vec![
                arr(vec![obj(vec![("v", int(1))])]),
                arr(vec![obj(vec![("v", int(2))])])
            ])
        )])
    );
}

#[test]
fn test_parent_key_not_clobbered_by_struct_start() {
    let r = run(vec![
        (AS, key("outer")),
        (ARS, None),
        (SS, None),
        (AS, key("inner_key")),
        (D, string("value")),
        (SE, None),
        (ARE, None),
    ]);
    let Value::Struct(entries) = r else {
        panic!("{r:?}")
    };
    let outer = entries.iter().find(|(k, _)| k == "outer").map(|(_, v)| v);
    assert_eq!(
        outer,
        Some(&arr(vec![obj(vec![(
            "inner_key",
            Value::String("value".into())
        )])]))
    );
}
