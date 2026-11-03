//! Port of `python/tests/test_unicode_normalisation.py`: a unit symbol must
//! survive Unicode normalisation.
//!
//! Three of the symbols are characters Unicode declares equivalent to another:
//! U+2126 OHM SIGN -> U+03A9 (all forms), U+212B ANGSTROM SIGN -> U+00C5 (NFD:
//! U+0041 U+030A), U+00B5 MICRO SIGN -> U+03BC (NFKC/NFKD). A document passed
//! through any normalising step must still parse.
//!
//! Python uses `unicodedata`; with no dependency for it here, `normalize`
//! below carries the decompositions of exactly the non-ASCII characters the
//! unit catalogue uses, and fails loudly on any other character so a new
//! symbol cannot slip past the sweep untested.

use bovnar::units::parse_unit;
use bovnar::{Value, parse};

const FORMS: [&str; 4] = ["NFC", "NFD", "NFKC", "NFKD"];

/// Non-ASCII catalogue characters that every normalisation form leaves
/// alone.
const STABLE: [char; 10] = [
    '\u{00B0}', '\u{030A}', '\u{0394}', '\u{03A9}', '\u{03BC}', '\u{2014}', '\u{2030}', '\u{2031}',
    '\u{2127}', '\u{2248}',
];

/// Unicode normalisation `form` of `s`, for the characters the catalogue
/// uses.
fn normalize(form: &str, s: &str) -> String {
    let compat = form.starts_with("NFK");
    let compose = form.ends_with('C');
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\u{2126}' => out.push('\u{03A9}'),
            '\u{212B}' | '\u{00C5}' => out.push_str("A\u{030A}"),
            '\u{00B5}' if compat => out.push('\u{03BC}'),
            '\u{00B5}' => out.push(c),
            c if c.is_ascii() || STABLE.contains(&c) => out.push(c),
            c => panic!(
                "U+{:04X} is not in this test's normalisation table; add it",
                c as u32
            ),
        }
    }
    if compose {
        out = out.replace("A\u{030A}", "\u{00C5}");
    }
    out
}

const DOC: &str = "#!bovnar 1.1\n\
    .r = <float:64,\u{2126}> 100.0;\n\
    .d = <float:64,\u{00B5}~m> 2.5;\n\
    .a = <float:64,\u{212B}> 1.0;\n";

#[test]
fn test_document_survives_normalisation() {
    for form in FORMS {
        let normalised = normalize(form, DOC);
        let values = parse(normalised.as_bytes()).unwrap_or_else(|e| panic!("{form}: {e}"));
        assert_eq!(
            values,
            Value::Struct(vec![
                ("r".into(), Value::Float(100.0)),
                ("d".into(), Value::Float(2.5)),
                ("a".into(), Value::Float(1.0)),
            ]),
            "{form}"
        );
    }
}

/// Every non-empty, non-ASCII quoted string of the unit catalogue.
fn symbols() -> Vec<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/gendata/units.bvnr");
    let text = std::fs::read_to_string(path).unwrap();
    let mut out: Vec<String> = text
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|s| !s.is_empty() && !s.is_ascii())
        .map(str::to_string)
        .collect();
    out.sort();
    out.dedup();
    out
}

fn code_points(s: &str) -> String {
    s.chars()
        .map(|c| format!("U+{:04X}", c as u32))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Sweep the whole catalogue, not just the reported three.
#[test]
fn test_every_unit_symbol_survives_normalisation() {
    let symbols = symbols();
    assert!(
        symbols.len() >= 5,
        "found only {} non-ASCII symbols",
        symbols.len()
    );
    for form in FORMS {
        let mut broken = Vec::new();
        for sym in &symbols {
            if parse_unit(sym).is_err() {
                continue; // not a standalone unit symbol
            }
            let variant = normalize(form, sym);
            if variant == *sym {
                continue;
            }
            if parse_unit(&variant).is_err() {
                broken.push(format!(
                    "{sym} ({}) -> {}",
                    code_points(sym),
                    code_points(&variant)
                ));
            }
        }
        assert!(
            broken.is_empty(),
            "{form} destroys these symbols: {broken:?}"
        );
    }
}
