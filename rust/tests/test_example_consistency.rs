//! The numbers inside the shipped examples agree with each other. A small
//! exact decimal (mantissa and scale) does the arithmetic on the literals.

use std::path::PathBuf;

use bovnar::{Value, parse_typed};

fn examples() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples"))
}

fn load(name: &str) -> Value {
    parse_typed(&std::fs::read(examples().join(name)).unwrap()).unwrap()
}

fn get<'a>(v: &'a Value, key: &str) -> &'a Value {
    let Value::Struct(entries) = v else {
        panic!("not a struct: {v:?}")
    };
    &entries
        .iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("no key {key}"))
        .1
}

/// An exact decimal: `mantissa * 10^-scale`.
#[derive(Clone, Copy, Debug)]
struct Dec(i128, u32);

impl Dec {
    fn parse(text: &str) -> Dec {
        let (int, frac) = text.trim().split_once('.').unwrap_or((text.trim(), ""));
        Dec(format!("{int}{frac}").parse().unwrap(), frac.len() as u32)
    }

    fn mul(self, o: Dec) -> Dec {
        Dec(self.0 * o.0, self.1 + o.1)
    }

    fn add(self, o: Dec) -> Dec {
        let scale = self.1.max(o.1);
        Dec(
            self.0 * 10i128.pow(scale - self.1) + o.0 * 10i128.pow(scale - o.1),
            scale,
        )
    }

    fn div_pow10(self, n: u32) -> Dec {
        Dec(self.0, self.1 + n)
    }
}

impl PartialEq for Dec {
    fn eq(&self, o: &Dec) -> bool {
        let scale = self.1.max(o.1);
        self.0 * 10i128.pow(scale - self.1) == o.0 * 10i128.pow(scale - o.1)
    }
}

/// The decimal literal of a typed scalar.
fn raw(v: &Value) -> Dec {
    match v {
        Value::Quantity(q) => Dec::parse(q.raw.as_deref().expect("a literal")),
        other => panic!("not a quantity: {other:?}"),
    }
}

#[test]
fn test_crypto_portfolio_total_matches_its_holdings() {
    let doc = load("crypto_portfolio.bvnr");
    let portfolio = get(&doc, "portfolio");
    let Value::Array(holdings) = get(portfolio, "holdings") else {
        panic!("holdings is not an array")
    };
    let total = holdings.iter().fold(Dec(0, 0), |sum, h| {
        sum.add(raw(get(h, "amount")).mul(raw(get(h, "current_price"))))
    });
    let declared = raw(get(portfolio, "total_value_usd"));
    assert_eq!(
        total, declared,
        "holdings sum to {total:?} but .total_value_usd says {declared:?}"
    );
}

#[test]
fn test_btc_decimal_matches_the_satoshi_balance() {
    let doc = load("crypto_portfolio.bvnr");
    let sats = raw(get(&doc, "btc_balance_sat"));
    let dec = raw(get(&doc, "btc_decimal"));
    assert_eq!(dec, sats.div_pow10(8), "{sats:?} sat is not {dec:?} BTC");
}

/// The section header states a count; the section must actually have it.
#[test]
fn test_units_example_derived_count_matches_its_own_header() {
    let text = std::fs::read_to_string(examples().join("units.bvnr")).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let marker = " named SI-derived units)";
    let mut claimed = None;
    let mut counted = 0;
    for (i, line) in lines.iter().enumerate() {
        // Find "(<digits> named SI-derived units)" in the line.
        let Some(end) = line.find(marker) else {
            continue;
        };
        let head = &line[..end];
        let digits = head.len() - head.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        if digits == 0 || !head[..head.len() - digits].ends_with('(') {
            continue;
        }
        claimed = Some(head[head.len() - digits..].parse::<usize>().unwrap());
        for rest in &lines[i + 1..] {
            if rest.starts_with("# 4.") {
                break;
            }
            if rest.starts_with(".u_") {
                counted += 1;
            }
        }
        break;
    }
    let claimed = claimed.expect("the SI-derived section header went missing");
    assert_eq!(
        counted, claimed,
        "header claims {claimed} named SI-derived units, section has {counted}"
    );
}
