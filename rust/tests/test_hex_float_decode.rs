//! Port of `python/tests/test_hex_float_decode.py`: float carriers written in
//! base 16 decode in base 16.

use bovnar::{Value, parse, parse_typed};

const DOC: &[u8] = b"#!bovnar 1.1
.a = <float:64,_16> \"1.8\";
.b = <float:64,_16> \"10.4\";
.c = <float:64,_16> \"1.0p+0\";
.d = <float:64,_16> \"-1.8\";
.e = <float:64> 1.8;
.f = <float_fix:32,q16> 1.5;
.g = <float_dec:64> 2.75;
";

const EXPECT: [(&str, f64); 7] = [
    ("a", 1.5),
    ("b", 16.25),
    ("c", 1.0),
    ("d", -1.5),
    ("e", 1.8),
    ("f", 1.5),
    ("g", 2.75),
];

#[test]
fn test_hex_float_carriers_decode_in_base_16() {
    let Value::Struct(got) = parse(DOC).unwrap() else {
        unreachable!()
    };
    for (k, want) in EXPECT {
        let v = &got.iter().find(|(key, _)| key == k).unwrap().1;
        let Value::Float(f) = v else {
            panic!("{k} decoded as {v:?}")
        };
        assert!((f - want).abs() < 1e-12, "{k}: got {f}, want {want}");
    }
}

/// The two decode paths must not disagree.
#[test]
fn test_loads_agrees_with_quantity_decimal() {
    let Value::Struct(typed) = parse_typed(DOC).unwrap() else {
        unreachable!()
    };
    let Value::Struct(plain) = parse(DOC).unwrap() else {
        unreachable!()
    };
    let get = |fields: &[(String, Value)], k: &str| {
        fields.iter().find(|(key, _)| key == k).unwrap().1.clone()
    };
    for (k, _) in EXPECT {
        let Value::Quantity(q) = get(&typed, k) else {
            panic!("{k} is not a quantity")
        };
        let Value::Float(p) = get(&plain, k) else {
            panic!("{k} is not a float")
        };
        let d = q.decimal().unwrap().unwrap().to_f64();
        assert!((d - p).abs() < 1e-12, "{k}");
    }
}
