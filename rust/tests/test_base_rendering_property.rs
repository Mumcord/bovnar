//! An exact read-time conversion renders in a base exactly when the
//! arithmetic says its expansion terminates there, and every rendering
//! decodes back to the exact rational.

use std::collections::BTreeMap;

use bovnar::units::parse_unit;
use bovnar::{Event, EventData, ReadOptions, Reader};

const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

/// (source unit, hook target). Chosen so the exact factors between them cover
/// a range of denominators: a pure power of ten, one with a 3 in it, the
/// inch's 254/10000, the foot's 10000/3048 and the pound's
/// 45359237/100000000.
const CASES: [(&str, &str); 7] = [
    ("m", "k~m"),
    ("in", "m"),
    ("m", "ft"),
    ("lb", "k~g"),
    ("k~m", "m"),
    ("h", "s"),
    ("k~m/h", "m/s"),
];

fn gcd(a: i128, b: i128) -> i128 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

/// A reduced fraction with a positive denominator.
fn fraction(num: i128, den: i128) -> (i128, i128) {
    let g = gcd(num, den).max(1) * den.signum();
    (num / g, den / g)
}

fn prime_factors(mut n: i128) -> Vec<i128> {
    let mut out = Vec::new();
    let mut d = 2;
    while d * d <= n {
        while n % d == 0 {
            if !out.contains(&d) {
                out.push(d);
            }
            n /= d;
        }
        d += 1;
    }
    if n > 1 && !out.contains(&n) {
        out.push(n);
    }
    out
}

/// p/q terminates in `base` iff every prime factor of q divides base.
fn terminates(frac: (i128, i128), base: u32) -> bool {
    if frac.1 == 1 {
        return true;
    }
    let base_factors = prime_factors(base as i128);
    prime_factors(frac.1)
        .iter()
        .all(|p| base_factors.contains(p))
}

fn decode(text: &str, base: u32) -> (i128, i128) {
    let (negative, text) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (integral, fractional) = text.split_once('.').unwrap_or((text, ""));
    let digit = |c: u8| DIGITS.iter().position(|&d| d == c).unwrap() as i128;
    let base = base as i128;
    let (mut num, mut den) = (0i128, 1i128);
    for c in integral.bytes() {
        num = num * base + digit(c);
    }
    for c in fractional.bytes() {
        num = num * base + digit(c);
        den *= base;
    }
    fraction(if negative { -num } else { num }, den)
}

/// The exact conversion of `1.0 unit` to `target`, and its rendering in
/// every base 2..62 (`None` where it does not terminate or fails).
struct Rendered {
    rational: Option<(i128, i128)>,
    bases: BTreeMap<u32, Option<String>>,
}

fn render(unit: &str, target: &str) -> Rendered {
    let mut out = Rendered {
        rational: None,
        bases: BTreeMap::new(),
    };
    let want = parse_unit(target).unwrap();
    let mut want_unit = |_: &EventData<'_>| Some((want, 0));
    let mut on_event = |d: &EventData<'_>| {
        if d.event != Event::Data || !d.converted {
            return true;
        }
        out.rational = d
            .converted_rational()
            .map(|(n, q)| fraction(n.parse().unwrap(), q.parse().unwrap()));
        for base in 2..63 {
            out.bases
                .insert(base, d.converted_in_base(base).ok().flatten());
        }
        true
    };
    let options = ReadOptions {
        want_unit_allow_nonterminating: true,
        ..ReadOptions::default()
    };
    Reader::new()
        .unwrap()
        .read_mem_want_unit(
            format!(".v = <float:64,{unit}> 1.0;").as_bytes(),
            &options,
            &mut want_unit,
            Some(&mut on_event),
        )
        .unwrap();
    out
}

#[test]
fn test_it_terminates_exactly_when_the_arithmetic_says_it_should() {
    let (mut checked, mut wrong) = (0, Vec::new());
    for (unit, target) in CASES {
        let rendered = render(unit, target);
        let exact = rendered
            .rational
            .unwrap_or_else(|| panic!("{unit} -> {target} produced no conversion"));
        for base in 2..63 {
            checked += 1;
            let produced = rendered.bases[&base].is_some();
            if produced != terminates(exact, base) {
                wrong.push((unit, target, base, produced, terminates(exact, base)));
            }
        }
    }
    assert!(checked > 300);
    assert!(
        wrong.is_empty(),
        "the renderer disagrees with the terminating rule: {:?}",
        &wrong[..wrong.len().min(8)]
    );
}

#[test]
fn test_every_terminating_rendering_decodes_back_to_the_exact_rational() {
    let (mut checked, mut wrong) = (0, Vec::new());
    for (unit, target) in CASES {
        let rendered = render(unit, target);
        let exact = rendered.rational.unwrap();
        for base in 2..63 {
            let Some(text) = &rendered.bases[&base] else {
                continue;
            };
            checked += 1;
            if decode(text, base) != exact {
                wrong.push((unit, target, base, text.clone(), decode(text, base), exact));
            }
        }
    }
    assert!(checked > 100);
    assert!(
        wrong.is_empty(),
        "renderings that do not decode back: {:?}",
        &wrong[..wrong.len().min(8)]
    );
}

/// Without this both tests above could pass over a set that always
/// terminates.
#[test]
fn test_the_non_terminating_case_is_really_in_the_set() {
    let rendered = render("k~m/h", "m/s");
    // 1 km/h in m/s is 5/18 = 5/(2 * 3**2): it needs a base carrying both a 2
    // and a 3, so base 6 renders it and base 10 cannot.
    assert_eq!(rendered.rational, Some((5, 18)));
    assert_eq!(rendered.bases[&10], None);
    assert_eq!(rendered.bases[&3], None); // a 3 alone is not enough
    assert_eq!(rendered.bases[&6].as_deref(), Some("0.14")); // 1/6 + 4/36 = 5/18
}
