//! `units_convertible` may answer true for a pair that `convert_value` then
//! refuses only in two documented shapes: an affine scale inside a compound,
//! and a factor too large for any representation. The sweep below draws its
//! pairs from a small deterministic generator with a fixed seed.

use bovnar::units::{convert_value, parse_unit, units_convertible};

const AFFINE: [&str; 6] = ["°C", "°F", "°De", "°N", "°Re", "°Ro"];

/// A fixed-seed xorshift generator, enough to sample and shuffle.
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

    /// `k` distinct elements of `items`, like `random.sample`.
    fn sample<'a>(&mut self, items: &'a [String], k: usize) -> Vec<&'a String> {
        let mut idx: Vec<usize> = (0..items.len()).collect();
        for i in 0..k {
            let j = i + self.below(idx.len() - i);
            idx.swap(i, j);
        }
        idx[..k].iter().map(|&i| &items[i]).collect()
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i + 1);
            items.swap(i, j);
        }
    }
}

fn catalogue() -> Vec<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/gendata/units.bvnr");
    let text = std::fs::read_to_string(path).expect("read src/gendata/units.bvnr");
    let mut out = Vec::new();
    let mut rest = text.as_str();
    while let Some(pos) = rest.find(".symbol") {
        rest = &rest[pos + ".symbol".len()..];
        let after = rest.trim_start();
        let Some(after) = after.strip_prefix('=') else {
            continue;
        };
        let Some(after) = after.trim_start().strip_prefix('"') else {
            continue;
        };
        if let Some(end) = after.find('"')
            && end > 0
        {
            out.push(after[..end].to_string());
        }
    }
    out
}

/// True when an affine scale appears anywhere but alone at exponent 1.
fn affine_misplaced(spelling: &str) -> bool {
    if AFFINE.contains(&spelling) {
        return false;
    }
    AFFINE.iter().any(|a| spelling.contains(a))
}

#[test]
fn test_convertible_that_cannot_convert_is_always_the_affine_shape() {
    let symbols = catalogue();
    assert!(!symbols.is_empty());

    let mut rng = Rng(20260731);
    let mut spellings = symbols.clone();
    for sym in &symbols {
        for other in rng.sample(&symbols, 2) {
            spellings.push(format!("{sym}/{other}"));
        }
        spellings.push(format!("{sym}²"));
    }

    let mut units: Vec<_> = spellings
        .into_iter()
        .filter_map(|s| parse_unit(&s).ok().map(|u| (s, u)))
        .collect();
    rng.shuffle(&mut units);

    let mut screened = 0;
    let mut unexplained = Vec::new();
    let pairs = units
        .iter()
        .enumerate()
        .flat_map(|(i, a)| units[i + 1..].iter().map(move |b| (a, b)))
        .take(120_000);
    for ((sa, a), (sb, b)) in pairs {
        if !units_convertible(a, b) {
            continue;
        }
        screened += 1;
        if convert_value(1.0, a, b).is_err() && !(affine_misplaced(sa) || affine_misplaced(sb)) {
            unexplained.push((sa.clone(), sb.clone()));
        }
    }

    assert!(screened > 500, "the sweep stopped screening anything");
    assert!(
        unexplained.is_empty(),
        "units_convertible passed a pair the conversion refused, and it is not \
         the documented affine-in-a-compound shape: {:?}",
        &unexplained[..unexplained.len().min(10)]
    );
}

#[test]
fn test_capacity_is_the_other_shape_in_the_gap() {
    let (a, b) = (
        parse_unit("Q~m^100").unwrap(),
        parse_unit("q~m^100").unwrap(),
    );
    assert!(units_convertible(&a, &b));
    assert!(convert_value(1.0, &a, &b).is_err());
    assert!(!affine_misplaced("Q~m^100"));
    assert!(!affine_misplaced("q~m^100"));
    let (small_a, small_b) = (parse_unit("Q~m").unwrap(), parse_unit("q~m").unwrap());
    let v = convert_value(1.0, &small_a, &small_b).unwrap();
    assert!((v - 1e60).abs() <= 1e60 * 1e-6);
}

#[test]
fn test_the_affine_shape_really_is_in_the_gap() {
    let (a, b) = (parse_unit("°C/h").unwrap(), parse_unit("K/h").unwrap());
    assert!(units_convertible(&a, &b));
    assert!(convert_value(1.0, &a, &b).is_err());
    let (c, k) = (parse_unit("°C").unwrap(), parse_unit("K").unwrap());
    assert!(units_convertible(&c, &k));
    let v = convert_value(25.0, &c, &k).unwrap();
    assert!((v - 298.15).abs() <= 298.15 * 1e-6);
}
