//! The currency catalogue the binding reads must match
//! `src/gendata/currencies.bvnr`, the source of truth, which is read with
//! this crate's own parser.

use std::collections::{BTreeMap, BTreeSet};

use bovnar::currency::{self as cur, CurrencyInfo};
use bovnar::{BaseUnit, Value};

/// One record of `currencies.bvnr`.
struct SrcRow {
    code: String,
    numeric_code: i128,
    minor_unit: i128,
    is_crypto: bool,
    name: String,
}

fn field<'a>(record: &'a [(String, Value)], key: &str) -> &'a Value {
    &record
        .iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("record without .{key}"))
        .1
}

fn int(v: &Value) -> i128 {
    match v {
        Value::Int(n) => *n,
        other => panic!("expected an integer, got {other:?}"),
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => panic!("expected a string, got {other:?}"),
    }
}

fn tables() -> (BTreeMap<BaseUnit, SrcRow>, BTreeMap<BaseUnit, CurrencyInfo>) {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../src/gendata/currencies.bvnr"
    );
    let data = std::fs::read(path).expect("read currencies.bvnr");
    let Value::Struct(doc) = bovnar::parse(&data).expect("parse currencies.bvnr") else {
        panic!("document is not a struct");
    };
    let Value::Array(rows) = field(&doc, "currencies") else {
        panic!(".currencies is not an array");
    };
    let mut src = BTreeMap::new();
    for row in rows {
        let Value::Struct(r) = row else {
            panic!("currency record is not a struct");
        };
        let is_crypto = match field(r, "is_crypto") {
            Value::Bool(b) => *b,
            other => panic!("expected a bool, got {other:?}"),
        };
        src.insert(
            BaseUnit::from_raw(int(field(r, "id")) as i32).expect("a BaseUnit for every row"),
            SrcRow {
                code: text(field(r, "code")),
                numeric_code: int(field(r, "numeric_code")),
                minor_unit: int(field(r, "minor_unit")),
                is_crypto,
                name: text(field(r, "name")),
            },
        );
    }
    let lib = cur::all_currencies().map(|i| (i.base, i)).collect();
    (src, lib)
}

#[test]
fn test_the_same_currencies_are_present() {
    let (src, lib) = tables();
    let missing: BTreeSet<&str> = src
        .iter()
        .filter(|(i, _)| !lib.contains_key(i))
        .map(|(_, c)| c.code.as_str())
        .collect();
    let extra: BTreeSet<&str> = lib
        .iter()
        .filter(|(i, _)| !src.contains_key(i))
        .map(|(_, c)| c.code.as_str())
        .collect();
    assert!(
        missing.is_empty(),
        "currencies.bvnr has these, the binding does not: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "the binding has these, currencies.bvnr does not: {extra:?}"
    );
    assert_eq!(src.len(), lib.len());
    assert_eq!(lib.len(), 216);
}

#[test]
fn test_every_field_matches_the_source() {
    type Getter = (
        &'static str,
        fn(&SrcRow) -> String,
        fn(&CurrencyInfo) -> String,
    );
    let fields: [Getter; 5] = [
        ("code", |c| c.code.clone(), |i| i.code.clone()),
        (
            "numeric_code",
            |c| c.numeric_code.to_string(),
            |i| i.numeric_code.to_string(),
        ),
        (
            "minor_unit",
            |c| c.minor_unit.to_string(),
            |i| i.minor_unit.to_string(),
        ),
        (
            "is_crypto",
            |c| c.is_crypto.to_string(),
            |i| i.is_crypto.to_string(),
        ),
        ("name", |c| c.name.clone(), |i| i.name.clone()),
    ];
    let (src, lib) = tables();
    for (name, from_src, from_lib) in fields {
        let mut bad = Vec::new();
        for (base, s) in &src {
            let Some(p) = lib.get(base) else { continue };
            let (want, got) = (from_src(s), from_lib(p));
            if want != got {
                bad.push(format!(
                    "  {:<5} (id {base}): currencies.bvnr={want:?}  binding={got:?}",
                    s.code
                ));
            }
        }
        assert!(
            bad.is_empty(),
            "{} currency {name} value(s) drifted from the source of truth:\n{}",
            bad.len(),
            bad.join("\n")
        );
    }
}

#[test]
fn test_the_range_predicates_agree_with_the_source() {
    let (src, _) = tables();
    let mut bad = Vec::new();
    for (&base, c) in &src {
        if !cur::is_currency(base) {
            bad.push(format!("{}: is_currency({base}) is false", c.code));
        }
        if cur::is_crypto(base) != c.is_crypto {
            bad.push(format!("{}: is_crypto disagrees", c.code));
        }
        if cur::is_fiat(base) == c.is_crypto {
            bad.push(format!("{}: is_fiat disagrees", c.code));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn test_non_currency_bases_are_rejected() {
    // Ids that name no BaseUnit cannot be passed at all now; the rest must
    // not be currencies.
    for base in [0, 1, 133, 348, 377, 380, 396, 397, 100000]
        .into_iter()
        .filter_map(BaseUnit::from_raw)
    {
        assert!(
            !cur::is_currency(base),
            "base {base} is a physical unit or out of range, not a currency"
        );
    }
}
