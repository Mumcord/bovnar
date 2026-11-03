//! Port of `python/tests/test_dom_vs_loads_parity.py`: the DOM tier and the
//! streaming tier read the same document to the same value.

use bovnar::{DomDoc, Value, parse};

/// NaN != NaN, and 1.0 == 1 only by luck: map NaN to a marker string and a
/// whole float to an integer before comparing.
fn norm(v: Value) -> Value {
    match v {
        Value::Struct(entries) => {
            Value::Struct(entries.into_iter().map(|(k, v)| (k, norm(v))).collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(norm).collect()),
        Value::Float(f) if f.is_nan() => Value::String("NAN".into()),
        Value::Float(f) if f.is_finite() && f.fract() == 0.0 => Value::Int(f as i128),
        other => other,
    }
}

fn get(v: Value, key: &str) -> Value {
    let Value::Struct(entries) = v else {
        panic!("not a struct")
    };
    entries
        .into_iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("no key {key}"))
        .1
}

fn ints(items: &[i128]) -> Value {
    Value::Array(items.iter().map(|&i| Value::Int(i)).collect())
}

fn rows(rows: &[Value]) -> Value {
    Value::Array(rows.to_vec())
}

#[test]
fn test_dimension_rows_keep_their_shape() {
    let cases: Vec<(&str, Value)> = vec![
        (
            ".a = [1,2,3]/[4,5,6];\n",
            rows(&[ints(&[1, 2, 3]), ints(&[4, 5, 6])]),
        ),
        (
            ".a = [1,2]/[3,4]/[5,6];\n",
            rows(&[ints(&[1, 2]), ints(&[3, 4]), ints(&[5, 6])]),
        ),
        (".a = [1]/[2];\n", rows(&[ints(&[1]), ints(&[2])])),
        (
            ".a = [[1,2]/[3,4]];\n",
            rows(&[rows(&[ints(&[1, 2]), ints(&[3, 4])])]),
        ),
        // must stay flat
        (".a = [1,2,3,4,5,6];\n", ints(&[1, 2, 3, 4, 5, 6])),
        (
            ".a = [[1,2],[3,4]];\n",
            rows(&[ints(&[1, 2]), ints(&[3, 4])]),
        ),
    ];
    for (body, expected) in cases {
        let doc = format!("#!bovnar 1.1\n{body}").into_bytes();
        let dom = DomDoc::parse(&doc).unwrap().to_value().unwrap();
        assert_eq!(norm(get(dom, "a")), expected, "dom: {body}");
        assert_eq!(
            norm(get(parse(&doc).unwrap(), "a")),
            expected,
            "parse: {body}"
        );
    }
}

#[test]
fn test_the_two_tiers_agree_on_rich_documents() {
    let rich = [
        ".a = <float:64,k~m/h> 88.5;\n.b = 9.81 m/s;\n.c = <uint:64,Gi~B> 4;\n",
        ".a = <float_dec:64,$USD> 19.99;\n.b = <uint:64,$BTC> 547820000;\n",
        ".a = <datetime:64,tai> 2016-12-31T23:59:60.5Z;\n.b = 2026-06-15;\n",
        ".a = 1;\n.r = &.a;\n.m = [[1,2],[3,4]];\n.i = &.m[0][1];\n",
        ".a = <uint:128> 340282366920938463463374607431768211455;\n",
        ".a = <float_dec:128> 1.2345678901234567890123456789;\n",
        ".a = <uint:32,_16> \"deadbeef\";\n.b = <float:64,_16> \"1.8p3\";\n",
        ".a = nan;\n.b = inf;\n.c = ninf;\n",
        ".a = \"tab\\there\";\n.b = \"caf\\u{e9}\";\n.c = \"\\x41\";\n",
        ".a = {.b = [{.c = 1;}, {.c = 2;}];};\n",
        ".a = [1, , 3];\n.b = ;\n",
    ];
    for body in rich {
        let doc = format!("#!bovnar 1.1\n{body}").into_bytes();
        assert_eq!(
            norm(DomDoc::parse(&doc).unwrap().to_value().unwrap()),
            norm(parse(&doc).unwrap()),
            "{body}"
        );
    }
}
