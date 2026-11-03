//! Port of `python/tests/test_roundtrip_property.py`: a typed parse and
//! serialise cycle keeps every value's family, width, unit and literal.
//!
//! The Python test filters its random documents through the `bovnar
//! validate` CLI; here the same full validation is `DomDoc::parse`, so the
//! test needs no CLI build.

use bovnar::{DomDoc, Value, parse_typed, to_string};

const FAMILIES: [(&str, &[u32], &[&str]); 4] = [
    (
        "uint",
        &[8, 16, 32, 64, 128],
        &["0", "1", "255", "65535", "12345678901234567890"],
    ),
    (
        "sint",
        &[8, 16, 32, 64],
        &["0", "-1", "127", "-128", "-9007199254740993"],
    ),
    (
        "float",
        &[16, 32, 64, 128],
        &["1.5", "-2.75", "0.0", "3.14159", "1e10", "nan", "inf"],
    ),
    (
        "float_dec",
        &[32, 64, 128],
        &["1.5", "19.99", "1.2345678901234567890123456789"],
    ),
];
const UNITS: [&str; 10] = [
    "",
    "m",
    "k~m",
    "m/s",
    "k~g*m/s²",
    "Hz",
    "$USD",
    "$JPY",
    "Gi~B",
    "°C",
];

/// A small deterministic xorshift generator standing in for `random.Random`.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn choice<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
    /// Inclusive range, like `random.randint`.
    fn randint(&mut self, lo: usize, hi: usize) -> usize {
        lo + self.below(hi - lo + 1)
    }
    fn random(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn annotation(fam: &str, width: u32, unit: &str) -> String {
    if unit.is_empty() {
        format!("<{fam}:{width}>")
    } else {
        format!("<{fam}:{width},{unit}>")
    }
}

fn pick(rnd: &mut Rng, arrays: bool) -> (String, String) {
    let (fam, widths, values) = *rnd.choice(&FAMILIES);
    let width = *rnd.choice(widths);
    let mut unit = *rnd.choice(&UNITS);
    if (fam == "uint" || fam == "sint") && unit.starts_with('$') {
        unit = ""; // a currency needs a decimal family
    }
    let body = if arrays {
        let values: Vec<&str> = values
            .iter()
            .copied()
            .filter(|v| *v != "nan" && *v != "inf")
            .collect();
        let n = rnd.randint(1, 4);
        let items: Vec<&str> = (0..n).map(|_| *rnd.choice(&values)).collect();
        format!("[{}]", items.join(", "))
    } else {
        let body = *rnd.choice(values);
        if body == "nan" || body == "inf" {
            unit = ""; // a special number takes no unit
        }
        body.to_string()
    };
    (annotation(fam, width, unit), body)
}

/// What must survive: family, width, unit and the exact literal.
#[derive(Debug, PartialEq)]
enum Signature {
    Scalar(i32, u32, String, Option<String>),
    List(Vec<Signature>),
    Other(Value),
}

fn signature(v: &Value) -> Signature {
    match v {
        Value::Quantity(q) => {
            Signature::Scalar(q.vtype.family, q.vtype.width, q.unit_str(), q.raw.clone())
        }
        Value::Array(items) => Signature::List(items.iter().map(signature).collect()),
        other => Signature::Other(other.clone()),
    }
}

fn signatures(doc: &Value) -> Vec<(String, Signature)> {
    let Value::Struct(entries) = doc else {
        panic!("not a struct")
    };
    entries
        .iter()
        .map(|(k, v)| (k.clone(), signature(v)))
        .collect()
}

#[test]
fn test_typed_roundtrip_preserves_value_and_type() {
    let mut rnd = Rng(17);
    let mut checked = 0;
    let mut failures = Vec::new();
    for _ in 0..400 {
        let mut lines = vec!["#!bovnar 1.1".to_string()];
        for j in 0..rnd.randint(1, 5) {
            let arrays = rnd.random() >= 0.6;
            let (ann, body) = pick(&mut rnd, arrays);
            let key = if arrays { "a" } else { "k" };
            lines.push(format!(".{key}{j} = {ann} {body};"));
        }
        let doc = (lines.join("\n") + "\n").into_bytes();
        // Only documents the reference implementation accepts are in scope.
        if DomDoc::parse(&doc).is_err() {
            continue;
        }
        let before = parse_typed(&doc).unwrap();
        let after = parse_typed(&to_string(&before, true).unwrap()).unwrap();
        checked += 1;
        let (a, b) = (signatures(&before), signatures(&after));
        if a != b {
            let (key, sig) = a
                .iter()
                .find(|(k, s)| b.iter().find(|(bk, _)| bk == k).map(|(_, bs)| bs) != Some(s))
                .unwrap_or(&a[0]);
            let other = b.iter().find(|(bk, _)| bk == key).map(|(_, s)| s);
            failures.push(format!("{key}: {sig:?} -> {other:?}"));
        }
    }
    assert!(checked >= 200, "only {checked} documents were in scope");
    assert!(
        failures.is_empty(),
        "{} of {checked} round-trips lost information, first: {:?}",
        failures.len(),
        &failures[..failures.len().min(3)]
    );
}
