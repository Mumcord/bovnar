//! Round trips: parse → write → parse yields the same document, for every
//! valid input of the conformance corpus and for generated documents.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use bovnar::{parse_with, Array, Document, Limits, Number, Struct, TypeFamily, TypeSpec, Value};
use serde_json::Value as Json;

const CORPUS: &str = include_str!("conformance/corpus.json");

fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_struct_nesting = 255;
    limits.max_array_nesting = 255;
    limits
}

/// Writes `doc` pretty and compact and checks both read back as `doc`.
fn assert_round_trip(id: &str, doc: &Document) -> Result<(), String> {
    for (mode, written) in [
        ("pretty", bovnar::to_vec(doc)),
        ("compact", bovnar::to_vec_compact(doc)),
    ] {
        let written =
            written.map_err(|e| format!("{id} ({mode}): write failed: {}", e.kind().as_str()))?;
        let back = parse_with(&written, &limits())
            .map_err(|e| format!("{id} ({mode}): re-read failed: {}", e.kind().as_str()))?;
        if &back != doc {
            return Err(format!(
                "{id} ({mode}): differs after the round trip: {:?}",
                String::from_utf8_lossy(&written)
            ));
        }
    }
    Ok(())
}

#[test]
fn corpus_round_trips() {
    let corpus: Json = serde_json::from_str(CORPUS).expect("corpus.json is valid JSON");
    let mut checked = 0usize;
    let mut failures = Vec::new();
    for case in corpus["cases"].as_array().expect("case list") {
        let id = case["id"].as_str().unwrap_or("?");
        let input = STANDARD
            .decode(case["input"].as_str().expect("input"))
            .expect("base64");
        // Every input the document tier accepts, whatever the corpus
        // expected of it under its own limits.
        let Ok(doc) = parse_with(&input, &limits()) else {
            continue;
        };
        checked += 1;
        if let Err(why) = assert_round_trip(id, &doc) {
            failures.push(why);
        }
    }
    println!("{checked} corpus documents round-tripped");
    for f in &failures {
        println!("FAIL {f}");
    }
    assert!(checked > 200, "too few documents checked: {checked}");
    assert!(
        failures.is_empty(),
        "{} round-trip failures",
        failures.len()
    );
}

/// A small deterministic generator (xorshift64*), so the property test
/// needs no extra dependency and every failure reproduces from its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn string(&mut self) -> String {
        const PIECES: [&str; 12] = [
            "a", "Z", " ", "\"", "\\", "\n", "\t", "é", "日本", "😀", "{", ";",
        ];
        (0..self.below(8))
            .map(|_| PIECES[self.below(PIECES.len() as u64) as usize])
            .collect()
    }

    fn key(&mut self) -> String {
        const KEYS: [&str; 6] = ["a", "b_c", "x1", "Name", "über", "k"];
        let mut key = KEYS[self.below(KEYS.len() as u64) as usize].to_owned();
        key.push_str(&self.below(100).to_string());
        key
    }

    /// A scalar of generator kind `kind`, so that one array holds one kind.
    fn scalar(&mut self, kind: u64) -> Value {
        match kind {
            0 => Value::from(self.next()),
            1 => Value::from(-((self.next() >> 2) as i64)),
            2 => Value::from(
                f64::from_bits(self.next() & 0x7fef_ffff_ffff_ffff)
                    * if self.below(2) == 0 { 1.0 } else { -1.0 },
            ),
            3 => Value::from(self.string()),
            4 => Value::from(self.below(2) == 0),
            5 => Value::from(Number::from(self.below(1000) as f64 / 8.0).with_unit("k~m/s")),
            6 => Value::from(Number::new(
                TypeSpec {
                    family: TypeFamily::Uint,
                    width: 32,
                    base: 16,
                },
                format!("{:x}", self.next() as u32),
            )),
            7 => Value::from(Number::from(self.next() as u8)),
            8 => Value::symbol(self.key()),
            _ => Value::null(),
        }
    }

    fn value(&mut self, depth: u32) -> Value {
        let pick = if depth == 0 { 0 } else { self.below(4) };
        match pick {
            1 => {
                let mut s = Struct::new();
                for _ in 0..self.below(4) {
                    let key = self.key();
                    s.insert(key, self.value(depth - 1));
                }
                Value::from(s)
            }
            2 => {
                let kind = self.below(10);
                let mut a = Array::new();
                for i in 0..self.below(5) {
                    if i == 2 && self.below(3) == 0 {
                        a.push(Value::null());
                    } else {
                        a.push(self.scalar(kind));
                    }
                }
                Value::from(a)
            }
            3 => {
                // A rectangular matrix.
                let kind = self.below(9);
                let (rows, cols) = (1 + self.below(3), 1 + self.below(3));
                let mut a = Array::new();
                for r in 0..rows {
                    if r > 0 {
                        a.new_row();
                    }
                    for _ in 0..cols {
                        a.push(self.scalar(kind));
                    }
                }
                let mut outer = Array::new();
                outer.push(a);
                Value::from(outer)
            }
            _ => {
                let kind = self.below(10);
                self.scalar(kind)
            }
        }
    }
}

#[test]
fn generated_documents_round_trip() {
    for seed in 1..=300u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        let mut doc = Document::new();
        for _ in 0..1 + rng.below(6) {
            let key = rng.key();
            doc.insert(key, rng.value(3));
        }
        let written = bovnar::to_vec(&doc).unwrap_or_else(|e| {
            panic!(
                "seed {seed}: write failed: {} for {doc:?}",
                e.kind().as_str()
            )
        });
        let parsed = parse_with(&written, &limits()).unwrap_or_else(|e| {
            panic!(
                "seed {seed}: re-read failed: {} in {:?}",
                e.kind().as_str(),
                String::from_utf8_lossy(&written)
            )
        });
        if let Err(why) = assert_round_trip(&format!("seed {seed}"), &parsed) {
            panic!("{why}");
        }
    }
}
