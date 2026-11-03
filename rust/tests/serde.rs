//! Serde mapping: round trips, the documented data model and errors.
#![cfg(feature = "serde")]

use std::collections::BTreeMap;

use bovnar::serde::{from_document, from_slice, from_str, to_document, to_string, to_vec};
use bovnar::{ErrorKind, Limits, ReadOptions};
use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Everything {
    b: bool,
    u8_: u8,
    u16_: u16,
    u32_: u32,
    u64_: u64,
    u128_: u128,
    i8_: i8,
    i16_: i16,
    i32_: i32,
    i64_: i64,
    i128_: i128,
    f32_: f32,
    f64_: f64,
    c: char,
    s: String,
    none: Option<u32>,
    some: Option<String>,
    list: Vec<u32>,
    empty: Vec<String>,
    pair: (f64, f64),
    nested: Inner,
    many: Vec<Inner>,
    map: BTreeMap<String, i32>,
    unit: (),
    marker: Marker,
    wrapped: Meters,
    modes: Vec<Mode>,
    shape: Shape,
    point: Shape,
    rect: Shape,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Inner {
    name: String,
    level: i16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Marker;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Meters(f64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Mode {
    Fast,
    Safe,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Shape {
    Circle(f64),
    Point(i32, i32),
    Rect { w: u32, h: u32 },
}

fn everything() -> Everything {
    Everything {
        b: true,
        u8_: u8::MAX,
        u16_: u16::MAX,
        u32_: u32::MAX,
        u64_: u64::MAX,
        u128_: u128::MAX,
        i8_: i8::MIN,
        i16_: i16::MIN,
        i32_: i32::MIN,
        i64_: i64::MIN,
        i128_: i128::MIN,
        f32_: 1.5e-7,
        f64_: -2.25e300,
        c: 'ß',
        s: "quote \" backslash \\ tab \t newline \n emoji 🎙".into(),
        none: None,
        some: Some("x".into()),
        list: vec![1, 2, 3],
        empty: vec![],
        pair: (0.5, -0.5),
        nested: Inner {
            name: "inner".into(),
            level: -3,
        },
        many: vec![
            Inner {
                name: "a".into(),
                level: 1,
            },
            Inner {
                name: "b".into(),
                level: 2,
            },
        ],
        map: [("alpha".to_string(), 1), ("beta".to_string(), -1)].into(),
        unit: (),
        marker: Marker,
        wrapped: Meters(9.81),
        modes: vec![Mode::Fast, Mode::Safe],
        shape: Shape::Circle(1.0),
        point: Shape::Point(-1, 2),
        rect: Shape::Rect { w: 3, h: 4 },
    }
}

#[test]
fn everything_round_trips() {
    let value = everything();
    let text = to_string(&value).expect("serializes");
    let back: Everything = from_str(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(back, value);
}

#[test]
fn data_model_matches_the_docs() {
    let text = to_string(&everything()).expect("serializes");
    for expected in [
        ".b = true;",
        ".u8_ = <uint:8> 255;",
        ".i64_ = -9223372036854775808;",
        ".u128_ = <uint:128> 340282366920938463463374607431768211455;",
        ".none = ;",
        ".modes = [Fast, Safe];",
        ".nested = {",
        ".shape = {\n\t.Circle = 1.0;\n};",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in\n{text}");
    }
    let doc = to_document(&everything()).expect("document");
    assert_eq!(doc.lookup(".rect.Rect.h").and_then(|v| v.as_u64()), Some(4));
    assert_eq!(
        doc.lookup(".point.Point[1]").and_then(|v| v.as_i64()),
        Some(2)
    );
}

/// Bytes through `serialize_bytes`, as `serde_bytes` does.
#[derive(Debug, PartialEq)]
struct Blob(Vec<u8>);

impl Serialize for Blob {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for Blob {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = Blob;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("bytes")
            }
            fn visit_bytes<E>(self, v: &[u8]) -> Result<Blob, E> {
                Ok(Blob(v.to_vec()))
            }
        }
        d.deserialize_bytes(V)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct WithBytes {
    data: Blob,
}

#[test]
fn bytes_become_octet_streams() {
    let value = WithBytes {
        data: Blob(vec![0, 1, 0x0d, 0x0a, 0xff]),
    };
    let err = to_string(&value).unwrap_err();
    assert_eq!(
        err.bovnar().map(|e| e.kind()),
        Some(ErrorKind::InvalidUtf8Byte)
    );
    let bytes = to_vec(&value).expect("binary output");
    let back: WithBytes = from_slice(&bytes).expect("reads back");
    assert_eq!(back, value);
}

#[derive(Debug, PartialEq, Deserialize)]
struct Borrowed<'a> {
    name: &'a str,
}

#[test]
fn from_document_borrows() {
    let doc = bovnar::parse(b".name = \"zero copy\";").expect("parses");
    let b: Borrowed<'_> = from_document(&doc).expect("borrows");
    assert_eq!(b.name, "zero copy");
}

#[derive(Debug, PartialEq, Deserialize)]
struct Lenient {
    float_from_int: f64,
    narrow: u8,
    mode: Mode,
    matrix: Vec<Vec<u32>>,
    hex: u32,
    missing: Option<u8>,
}

#[test]
fn reading_is_lenient_where_unambiguous() {
    let input = ".float_from_int = 3; .narrow = <uint:64> 7; .mode = \"Fast\"; \
                 .matrix = [1, 2] / [3, 4]; .hex = <uint:32,_16> \"ff\";";
    let v: Lenient = from_str(input).expect("reads");
    assert_eq!(
        v,
        Lenient {
            float_from_int: 3.0,
            narrow: 7,
            mode: Mode::Fast,
            matrix: vec![vec![1, 2], vec![3, 4]],
            hex: 255,
            missing: None,
        }
    );
}

#[derive(Debug, Serialize)]
struct Mixed {
    t: (u8, String),
}

#[test]
fn errors_never_panic() {
    // Top level must be a struct or map.
    assert!(to_string(&5u8).is_err());
    assert!(to_string(&vec![1, 2]).is_err());
    // Keys must be strings and valid identifiers.
    let numeric_keys: BTreeMap<u32, u32> = [(1, 2)].into();
    assert!(to_string(&numeric_keys).is_err());
    let bad_key: BTreeMap<String, u32> = [("not an identifier".to_string(), 2)].into();
    assert!(to_string(&bad_key).is_err());
    // Arrays are homogeneous.
    assert!(to_string(&Mixed { t: (1, "x".into()) }).is_err());
    // Type mismatches, out-of-range values, malformed input.
    assert!(from_str::<Inner>(".name = 1; .level = 2;").is_err());
    assert!(from_str::<Inner>(".name = \"a\"; .level = 70000;").is_err());
    assert!(from_str::<Inner>(".name = \"a\";").is_err());
    let err = from_str::<Inner>(".name = ").unwrap_err();
    assert_eq!(
        err.bovnar().map(|e| e.kind()),
        Some(ErrorKind::GotIncompleteBvnrStream)
    );
    assert!(from_str::<Lenient>(".mode = { .Fast = null; .Safe = null; };").is_err());
    assert!(from_str::<Lenient>(".mode = 3;").is_err());
    // Nesting beyond the limits is refused by the reader, not the stack.
    let deep = format!(".a = {}1{};", "[".repeat(100_000), "]".repeat(100_000));
    assert!(from_str::<BTreeMap<String, u8>>(&deep).is_err());
    let deep = format!(".a = {}{};", "{.a=".repeat(100_000), "};".repeat(100_000));
    assert!(from_str::<BTreeMap<String, u8>>(&deep).is_err());
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Gaps {
    v: Vec<Option<u32>>,
}

#[test]
fn null_elements_round_trip() {
    let value = Gaps {
        v: vec![Some(1), None, Some(3)],
    };
    let text = to_string(&value).expect("nulls mix with any element type");
    assert_eq!(from_str::<Gaps>(&text).expect("reads back"), value);
}

#[test]
fn read_options_apply() {
    let options = ReadOptions::new().text_only(true);
    let err = bovnar::serde::from_slice_with_options::<WithBytes>(b".data = \x00\x00;", &options)
        .unwrap_err();
    assert_eq!(
        err.bovnar().map(|e| e.kind()),
        Some(ErrorKind::OctetStreamForbidden)
    );
    let mut limits = Limits::default();
    limits.max_string_length = 3;
    let options = ReadOptions::new().limits(limits);
    assert!(bovnar::serde::from_slice_with_options::<Inner>(
        b".name = \"long\"; .level = 1;",
        &options
    )
    .is_err());
}

/// A small xorshift generator: enough variety for round trips without a
/// property-testing dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn string(&mut self) -> String {
        let len = self.next() % 12;
        (0..len)
            .map(|_| match self.next() % 6 {
                0 => '"',
                1 => '\\',
                2 => 'é',
                3 => '\n',
                4 => '🎧',
                _ => char::from(b'a' + (self.next() % 26) as u8),
            })
            .collect()
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Generated {
    u: u64,
    i: i64,
    small: i8,
    f: f64,
    g: f32,
    s: String,
    o: Option<i32>,
    v: Vec<u16>,
    inner: Vec<Inner>,
    shape: Shape,
    mode: Mode,
}

#[test]
fn generated_values_round_trip() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    for _ in 0..500 {
        let f = f64::from_bits(rng.next());
        let g = f32::from_bits(rng.next() as u32);
        let value = Generated {
            u: rng.next(),
            i: rng.next() as i64,
            small: rng.next() as i8,
            // NaN never equals itself; everything else, infinities included,
            // must survive.
            f: if f.is_nan() { 0.0 } else { f },
            g: if g.is_nan() { 0.0 } else { g },
            s: rng.string(),
            o: (rng.next() % 2 == 0).then(|| rng.next() as i32),
            v: (0..rng.next() % 5).map(|_| rng.next() as u16).collect(),
            inner: (0..rng.next() % 3)
                .map(|_| Inner {
                    name: rng.string(),
                    level: rng.next() as i16,
                })
                .collect(),
            shape: match rng.next() % 3 {
                0 => Shape::Circle(rng.next() as f64),
                1 => Shape::Point(rng.next() as i32, rng.next() as i32),
                _ => Shape::Rect {
                    w: rng.next() as u32,
                    h: rng.next() as u32,
                },
            },
            mode: if rng.next() % 2 == 0 {
                Mode::Fast
            } else {
                Mode::Safe
            },
        };
        let text = to_string(&value).unwrap_or_else(|e| panic!("{e}: {value:?}"));
        let back: Generated = from_str(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(back, value, "{text}");
    }
}
