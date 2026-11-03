//! Tests of the native unit system: one group per grammar rule, the unit
//! strings of the conformance corpus, and a differential fixture produced by
//! the C reference (`scripts/unit-oracle.sh`).

extern crate std;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::*;
use crate::ErrorKind;

const V: SpecVersion = SpecVersion::V1_2;

fn unit(s: &str) -> Unit {
    parse_unit(s.as_bytes(), V).unwrap_or_else(|e| panic!("{s:?} refused: {e:?}"))
}

fn err(s: &str) -> ErrorKind {
    match parse_unit(s.as_bytes(), V) {
        Ok(u) => panic!("{s:?} accepted as {u}"),
        Err(e) => e,
    }
}

fn canon(s: &str) -> String {
    unit(s).to_string()
}

#[test]
fn single_units_and_aliases() {
    assert_eq!(canon("m"), "m");
    assert_eq!(canon("metre"), "m");
    assert_eq!(unit("angstrom"), unit("Å"));
    assert_eq!(canon("°C"), "°C");
    assert_eq!(err("zzz_invalid_unit"), ErrorKind::UnitIllegal);
    assert_eq!(err(""), ErrorKind::UnitIllegal);
    assert_eq!(err(" m"), ErrorKind::UnitIllegal);
    assert_eq!(err("m\0"), ErrorKind::UnitIllegal);
}

#[test]
fn longest_suffix_wins() {
    // "mol" is the mole, not milli-ol; "min" is the minute.
    assert_eq!(canon("mol"), "mol");
    assert_eq!(unit("min"), unit("minute"));
    assert_eq!(canon("m~in"), "m~in");
    assert_ne!(unit("min"), unit("m~in"));
}

#[test]
fn prefixes_separated_and_compact() {
    assert_eq!(canon("k~m"), "k~m");
    assert_eq!(canon("km"), "k~m");
    assert_eq!(canon("kg"), "k~g");
    assert_eq!(canon("µm"), "µ~m");
    assert_eq!(canon("um"), "µ~m");
    assert_eq!(canon("μ~m"), "µ~m"); // Greek mu
    assert_eq!(canon("da~m"), "da~m");
    assert_eq!(err("kkg"), ErrorKind::UnitIllegal);
    assert_eq!(err("k~kg"), ErrorKind::UnitIllegal);
    assert_eq!(err("k~~m"), ErrorKind::UnitIllegal);
    assert_eq!(err("~m"), ErrorKind::UnitIllegal);
    assert_eq!(err("k~"), ErrorKind::UnitIllegal);
}

#[test]
fn compact_exceptions_only_block_the_compact_form() {
    assert_eq!(err("usb"), ErrorKind::UnitIllegal);
    assert_eq!(err("kt"), ErrorKind::UnitIllegal);
    assert_eq!(err("ppt"), ErrorKind::UnitIllegal);
    assert_eq!(canon("u~sb"), "µ~sb");
    assert_eq!(canon("k~t"), "k~t");
}

#[test]
fn prefix_policy() {
    // Binary prefixes only on information units, and those take SI from kilo up.
    assert_eq!(canon("KiB"), "Ki~B");
    assert_eq!(canon("Mi~b"), "Mi~b");
    assert_eq!(canon("kB"), "k~B");
    assert_eq!(err("mB"), ErrorKind::UnitIllegal);
    assert_eq!(err("Kim"), ErrorKind::UnitIllegal);
    assert_eq!(err("Ki~m"), ErrorKind::UnitIllegal);
    // Ratios and historical German units take none.
    assert_eq!(err("m~pH"), ErrorKind::UnitIllegal);
    assert_eq!(err("m~°dH"), ErrorKind::UnitIllegal);
    assert_eq!(err("k~%"), ErrorKind::UnitIllegal);
    assert_eq!(canon("p~H"), "p~H");
}

#[test]
fn exponents() {
    assert_eq!(canon("m^2"), "m²");
    assert_eq!(canon("m^+2"), "m²");
    assert_eq!(canon("m^-2"), "m⁻²");
    assert_eq!(canon("m^002"), "m²");
    assert_eq!(canon("m⁺²"), "m²");
    assert_eq!(canon("m^100"), "m¹⁰⁰");
    assert_eq!(canon("m⁻¹⁰⁰"), "m⁻¹⁰⁰");
    assert_eq!(canon("m^10"), "m¹⁰");
    assert_eq!(unit("m^100"), unit("m¹⁰⁰"));
    assert_eq!(unit("m^1"), unit("m"));
    for bad in [
        "m^0", "m^101", "m^-101", "m^1000", "m^0002", "m⁰", "m¹⁰¹", "m²³⁴", "^2", "m^", "m⁻",
    ] {
        assert_eq!(err(bad), ErrorKind::UnitIllegal, "{bad}");
    }
    let u = unit("s^-100");
    assert_eq!(u.components()[0].exponent(), -100);
}

#[test]
fn compound_units() {
    assert_eq!(canon("m/s"), "m/s");
    assert_eq!(canon("k~g*m/s^2"), "k~g·m/s²");
    assert_eq!(canon("m/s/s"), "m/s·s");
    assert_eq!(canon("s⁻¹·m"), "m/s");
    assert_eq!(canon("s^-1"), "s⁻¹");
    assert_eq!(canon("m·m"), "m·m"); // components are never merged
    for bad in ["m/", "/m", "*m", "m*", "m**s", "m··s", "m//s", "m s"] {
        assert_eq!(err(bad), ErrorKind::UnitIllegal, "{bad}");
    }
}

#[test]
fn groups() {
    assert_eq!(canon("(m/s)/s"), "m/s·s");
    assert_eq!(canon("k~g/(m·s²)"), "k~g/m·s²");
    assert_eq!(unit("m/(s/A)"), unit("m·A/s"));
    assert_eq!(unit("m/(s/k~g)"), unit("m·k~g/s"));
    for bad in [
        "(m",
        "m)",
        "()",
        "(m)(s)",
        "m(s)",
        "(m)s",
        "(m)^2",
        "((no_unit))",
    ] {
        assert_eq!(err(bad), ErrorKind::UnitIllegal, "{bad}");
    }
    // A group spelling no_unit contributes nothing, as in C, so a level
    // holding nothing else is empty and refused.
    assert_eq!(unit("m·(no_unit)"), unit("m"));
    assert_eq!(err("(no_unit)"), ErrorKind::UnitIllegal);
}

#[test]
fn group_depth_is_bounded() {
    let nested = |n: usize| alloc::format!("{}m{}", "(".repeat(n), ")".repeat(n));
    assert_eq!(unit(&nested(16)), unit("m"));
    assert_eq!(err(&nested(17)), ErrorKind::UnitIllegal);
    // Deep input is refused without recursion.
    assert_eq!(err(&nested(100_000)), ErrorKind::UnitIllegal);
}

#[test]
fn component_count_is_bounded() {
    let join = |n: usize| ["m"; 64][..n].join("·");
    assert_eq!(unit(&join(32)).components().len(), 32);
    assert_eq!(err(&join(33)), ErrorKind::UnitIllegal);
    assert_eq!(
        err(&alloc::format!("({})·{}", join(20), join(13))),
        ErrorKind::UnitIllegal
    );
}

#[test]
fn no_unit() {
    let u = unit("no_unit");
    assert!(u.is_no_unit());
    assert_eq!(u, Unit::no_unit());
    assert_eq!(u.to_string(), "no_unit");
    assert_eq!(err("no_unit·m"), ErrorKind::UnitIllegal);
    assert_eq!(err("NO_UNIT"), ErrorKind::UnitIllegal);
}

#[test]
fn currencies() {
    assert_eq!(canon("$USD"), "$USD");
    assert_eq!(canon("k$USD"), "k~$USD");
    assert_eq!(canon("k~$EUR/h"), "k~$EUR/h");
    assert_eq!(canon("$USD^2"), "$USD²");
    assert!(unit("$USD").components()[0].base().is_currency());
    for bad in [
        "USD", "kUSD", "$usd", "$US", "$XYZ", "Ki$USD", "~$USD", "kk$USD", "$", "$$USD", "$USD$",
    ] {
        assert_eq!(err(bad), ErrorKind::UnitIllegal, "{bad}");
    }
}

#[test]
fn equality_is_order_insensitive() {
    assert_eq!(unit("s³·m⁻⁵"), unit("m⁻⁵·s³"));
    assert_eq!(unit("k~g·m/s^2"), unit("kg·m/s²"));
    assert_ne!(unit("m·m"), unit("m²"));
    assert_ne!(unit("k~m"), unit("m"));
    assert_ne!(unit("N"), unit("k~g·m/s²"));
}

#[test]
fn temperature_differences() {
    assert_eq!(unit("ΔK"), unit("Δ°C"));
    assert_eq!(unit("delta_K"), unit("ΔK"));
    assert_ne!(unit("ΔK"), unit("K"));
    assert!(!convertible(&unit("ΔK"), &unit("K")));
    assert!(!convertible(&unit("Δ°F"), &unit("°F")));
    // Inside a compound a K already is an interval.
    assert_eq!(unit("W/(m²·ΔK)"), unit("W/(m²·K)"));
    assert_eq!(unit("ΔK^-1"), unit("K^-1"));
    assert_eq!(unit("ΔK/k~m"), unit("K/k~m"));
    // Kinds are judged after cancellation: ΔK·m/m is a lone ΔK.
    assert!(convertible(&unit("ΔK·m/m"), &unit("ΔK")));
    assert!(!convertible(&unit("ΔK·m/m"), &unit("K")));
    assert!(convertible(&unit("Δ°F"), &unit("ΔK")));
}

#[test]
fn convertibility() {
    assert!(convertible(&unit("N"), &unit("k~g·m/s²")));
    assert!(convertible(&unit("k~m"), &unit("m")));
    assert!(convertible(&unit("°F"), &unit("°C")));
    assert!(convertible(&unit("%"), &unit("no_unit")));
    assert!(!convertible(&unit("m"), &unit("s")));
    assert!(!convertible(&unit("b"), &unit("B")));
    assert!(!convertible(&unit("rad/s"), &unit("Hz")));
    assert!(convertible(&unit("°"), &unit("rad")));
    assert!(convertible(&unit("sr"), &unit("rad²")));
    assert!(!convertible(&unit("lm"), &unit("cd")));
    assert!(!convertible(&unit("pH"), &unit("no_unit")));
    assert!(!convertible(&unit("NTU"), &unit("FNU")));
    // Currencies have no dimension; only a change of prefix converts.
    assert!(convertible(&unit("k~$USD"), &unit("$USD")));
    assert!(convertible(&unit("$USD/h"), &unit("h^-1·k$USD")));
    assert!(!convertible(&unit("$USD"), &unit("$EUR")));
    assert!(!convertible(&unit("$USD"), &unit("no_unit")));
}

#[test]
fn base_conversion_data() {
    let inch = unit("in").components()[0].base().conversion().unwrap();
    assert_eq!(inch.factor_rational, ("127", "5000"));
    assert!(inch.exact);
    let f = unit("°F").components()[0].base().conversion().unwrap();
    assert!(f.affine);
    assert_eq!(f.factor_rational, ("5", "9"));
    assert_eq!(f.offset_rational, ("45967", "180"));
    let rev = unit("rev").components()[0].base().conversion().unwrap();
    assert!(!rev.exact);
    assert_eq!(rev.pi_factor, Some(("2", "1", 1)));
    assert!(unit("$USD").components()[0].base().conversion().is_none());
    let k = unit("k~m").components()[0].prefix().unwrap();
    assert_eq!(
        (k.system(), k.exponent(), k.symbol(), k.name()),
        (PrefixSystem::Si, 3, "k", "kilo")
    );
    let ki = unit("Ki~B").components()[0].prefix().unwrap();
    assert_eq!((ki.system(), ki.exponent()), (PrefixSystem::Iec, 10));
}

#[test]
fn ascii_spelling_round_trips() {
    for s in [
        "k~g·m/s²",
        "m⁻¹⁰⁰",
        "s³/m⁵",
        "k~$USD/h",
        "no_unit",
        "W/m²·K",
    ] {
        let u = unit(s);
        let ascii = u.ascii().to_string();
        assert!(ascii.is_ascii(), "{ascii}");
        assert_eq!(unit(&ascii), u, "{s} -> {ascii}");
    }
    assert_eq!(unit("k~g·m/s²").ascii().to_string(), "k~g*m/s^2");
    assert_eq!(unit("m⁻¹⁰⁰").ascii().to_string(), "m^-100");
}

#[test]
fn profile_notation() {
    // UCUM and CF are never registered: their namespaces are unknown, as in
    // a C build with them switched off.
    for s in [
        "ucum:m",
        "cf:air_temperature",
        "foo:m",
        "a-b:m",
        "x:",
        "m:s",
    ] {
        assert_eq!(err(s), ErrorKind::UnitProfileUnknown, "{s}");
    }
    // The notation is spec 1.2; below it the unit is simply not a unit.
    for v in [SpecVersion::V1_0, SpecVersion::V1_1] {
        assert_eq!(
            parse_unit(b"ucum:m", v).unwrap_err(),
            ErrorKind::UnitIllegal
        );
    }
    // Not namespaces: these fall through to the native grammar.
    for s in [":m", "-x:m", "A:m", "k~m:s"] {
        assert_eq!(err(s), ErrorKind::UnitIllegal, "{s}");
    }
    // Native units are not version-gated.
    assert!(parse_unit("k~$EUR/h".as_bytes(), SpecVersion::V1_0).is_ok());
}

/// Every line of the C reference's answers in `testdata/oracle.tsv`.
fn check_oracle(text: &str) -> usize {
    let mut failures = Vec::new();
    let mut checked = 0;
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<&str> = line.split('\t').collect();
        checked += 1;
        match f.as_slice() {
            ["u", s, code, canonical] => {
                let got = parse_unit(s.as_bytes(), V);
                let got_code = got.as_ref().map_or_else(|e| e.as_str(), |_| "ok");
                let got_canon = got
                    .as_ref()
                    .map_or_else(|_| "-".to_string(), |u| u.to_string());
                if got_code != *code || got_canon != *canonical {
                    failures.push(alloc::format!(
                        "{s:?}: C {code} {canonical}, Rust {got_code} {got_canon}"
                    ));
                } else if let Ok(u) = &got {
                    if unit(&got_canon) != *u {
                        failures.push(alloc::format!("{s:?}: {got_canon} does not re-parse equal"));
                    }
                }
            }
            ["p", a, b, eq, conv] => {
                let (ua, ub) = (unit(a), unit(b));
                let got = (u8::from(ua == ub), u8::from(convertible(&ua, &ub)));
                if (got.0.to_string(), got.1.to_string()) != (eq.to_string(), conv.to_string()) {
                    failures.push(alloc::format!(
                        "{a:?} vs {b:?}: C eq={eq} conv={conv}, Rust {got:?}"
                    ));
                }
            }
            ["w", ns, s, code] => {
                let got = parse_unit(s.as_bytes(), V)
                    .ok()
                    .and_then(|u| to_profile(&u, ns))
                    .unwrap_or_else(|| "-".to_string());
                if got != *code {
                    failures.push(alloc::format!("{ns} {s:?}: C {code}, Rust {got}"));
                }
            }
            _ => panic!("malformed oracle line {line:?}"),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {checked} differ:\n{}",
        failures.len(),
        failures[..failures.len().min(40)].join("\n")
    );
    checked
}

#[test]
fn matches_the_c_reference() {
    assert!(check_oracle(include_str!("testdata/oracle.tsv")) > 7000);
}

/// The shipped profiles against the C reference: every code of every table,
/// operator and exponent forms, mutations, and the writer.
#[test]
#[cfg(all(
    feature = "profile-unece",
    feature = "profile-qudt",
    feature = "profile-qudt-qk",
    feature = "profile-udunits",
    feature = "profile-om"
))]
fn profiles_match_the_c_reference() {
    assert!(check_oracle(include_str!("testdata/profile-oracle.tsv")) > 30000);
}

#[test]
#[cfg(all(
    feature = "profile-unece",
    feature = "profile-udunits",
    feature = "profile-qudt"
))]
fn profile_semantics() {
    // A profile code is an ordinary unit afterwards: equal to its native
    // spelling and to the same unit in another vocabulary.
    assert_eq!(unit("unece:KGM"), unit("k~g"));
    assert_eq!(unit("qudt:KiloGM"), unit("unece:KGM"));
    assert_eq!(unit("udunits:kg*m-2*s-1"), unit("udunits:kg m-2 s-1"));
    assert_ne!(unit("udunits:ms-1"), unit("udunits:m*s-1"));
    assert_eq!(unit("unece:KGM").to_string(), "k~g");
    // The decade fold divides by the exponent in a denominator.
    assert_eq!(unit("udunits:10^3/uL").to_string(), "n~L⁻¹");
    // Opaque units: canonical in profile notation, convertible to nothing
    // but themselves.
    let bx = unit("unece:XBX");
    assert!(bx.is_profile_only() && bx.components()[0].base().is_opaque());
    assert_eq!(bx.to_string(), "unece:XBX");
    assert_eq!(bx.components()[0].base().symbol(), "XBX");
    assert!(convertible(&bx, &bx));
    assert!(!convertible(&bx, &unit("unece:XPX")));
    assert!(!unit("k~g").is_profile_only());
    // The three refusals.
    assert_eq!(err("unece:DZN"), ErrorKind::UnitProfileUnsupported);
    assert_eq!(err("unece:NOPE"), ErrorKind::UnitIllegal);
    assert_eq!(
        err("udunits:dayssince1970-01-01"),
        ErrorKind::UnitProfileUnsupported
    );
    assert_eq!(err("udunits:10^4"), ErrorKind::UnitProfileUnsupported);
    // Writing back: a code is written only if it reads back as the unit.
    // "kt" would read back as the knot.
    assert_eq!(
        to_profile(&unit("k~t"), "udunits").as_deref(),
        Some("kilot")
    );
    assert_eq!(to_profile(&bx, "unece").as_deref(), Some("XBX"));
    assert_eq!(to_profile(&bx, "qudt"), None);
    assert_eq!(
        to_profile(&Unit::no_unit(), "udunits").as_deref(),
        Some("1")
    );
    assert_eq!(to_profile(&Unit::no_unit(), "unece"), None);
    assert!(profile_namespaces().any(|ns| ns == "udunits"));
    assert!(!profile_namespaces().any(|ns| ns == "ucum" || ns == "cf"));
}

/// Hostile shapes: deep groups, long digit runs, many components. None may
/// panic, and each gets C's answer.
#[test]
#[cfg(feature = "profile-udunits")]
fn profile_bounds() {
    let deep = |n: usize| alloc::format!("udunits:{}m{}", "(".repeat(n), ")".repeat(n));
    assert_eq!(unit(&deep(16)), unit("m"));
    assert_eq!(err(&deep(17)), ErrorKind::UnitIllegal);
    assert_eq!(err(&deep(10_000)), ErrorKind::UnitIllegal);
    assert_eq!(
        err("udunits:m999999999999999999"),
        ErrorKind::UnitProfileUnsupported
    );
    let many = alloc::format!("udunits:{}", ["m"; 33].join("."));
    assert_eq!(err(&many), ErrorKind::UnitProfileUnsupported);
    let factors = alloc::format!("udunits:{}m", "10^400.".repeat(20_000));
    assert_eq!(err(&factors), ErrorKind::UnitProfileUnsupported);
    let zeros = alloc::format!("udunits:1{}.m", "0".repeat(100_000));
    assert_eq!(err(&zeros), ErrorKind::UnitProfileUnsupported);
}

/// A namespace whose feature is off answers like a C build without it.
#[test]
#[cfg(not(feature = "profile-unece"))]
fn disabled_profile_is_unknown() {
    assert_eq!(err("unece:KGM"), ErrorKind::UnitProfileUnknown);
    assert!(!profile_namespaces().any(|ns| ns == "unece"));
}

/// A larger sweep against a fresh oracle run, e.g. every spelling under every
/// prefix: `BOVNAR_UNIT_ORACLE=answers.tsv cargo test -- --ignored`.
#[test]
#[ignore = "needs BOVNAR_UNIT_ORACLE, the output of scripts/unit-oracle.sh"]
fn matches_an_oracle_file() {
    let path = std::env::var("BOVNAR_UNIT_ORACLE").expect("BOVNAR_UNIT_ORACLE is set");
    let text = std::fs::read_to_string(path).expect("oracle file is readable");
    check_oracle(&text);
}

/// The units the `units` group of the conformance corpus uses, as
/// `(annotation units, inline unit)` per assignment line.
fn corpus_line_units(line: &str) -> (Vec<&str>, Option<&str>) {
    let mut annotations = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        let Some(close) = rest[open..].find('>') else {
            break;
        };
        let ann = &rest[open + 1..open + close];
        if let Some(comma) = ann.rfind(',') {
            annotations.push(&ann[comma + 1..]);
        }
        rest = &rest[open + close + 1..];
    }
    let inline = (|| {
        if line.contains('[') {
            return None;
        }
        let mut value = line[line.find('=')? + 1..].trim_start();
        if value.starts_with('<') {
            value = value[value.find('>')? + 1..].trim_start();
        }
        let value = value.strip_suffix(';')?;
        let (_, unit) = value.split_once(' ')?;
        Some(unit.trim())
    })();
    (annotations, inline)
}

#[test]
fn conformance_corpus_unit_strings() {
    use base64::Engine as _;
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/conformance/corpus.json")).unwrap();
    let mut cases = 0;
    for case in corpus["cases"].as_array().unwrap() {
        if case["group"] != "units" {
            continue;
        }
        cases += 1;
        let id = case["id"].as_str().unwrap();
        let input = base64::engine::general_purpose::STANDARD
            .decode(case["input"].as_str().unwrap())
            .unwrap();
        let input = core::str::from_utf8(&input).unwrap();
        // Without a version directive a document is 1.0.
        let version = if input.starts_with("#!bovnar 1.2") {
            V
        } else {
            SpecVersion::V1_0
        };
        let expected = case["expected_error"].as_str();

        let mut first_error = None;
        let mut mismatch = false;
        let mut annotated = Vec::new();
        for line in input.lines() {
            let (anns, inline) = corpus_line_units(line);
            let mut parsed = Vec::new();
            for s in anns.iter().chain(inline.iter()) {
                match parse_unit(s.as_bytes(), version) {
                    Ok(u) => parsed.push(u),
                    Err(e) => {
                        first_error.get_or_insert(e);
                    }
                }
            }
            if !anns.is_empty() && inline.is_some() && parsed.len() == 2 {
                mismatch |= parsed[0] != parsed[1];
            }
            annotated.extend(
                anns.iter()
                    .filter_map(|s| parse_unit(s.as_bytes(), version).ok()),
            );
        }
        match expected {
            None => {
                assert_eq!(first_error, None, "{id}");
                assert!(!mismatch, "{id}: annotation and inline unit differ");
            }
            Some("unit_illegal") => assert_eq!(first_error, Some(ErrorKind::UnitIllegal), "{id}"),
            Some("unit_mismatch") => {
                assert_eq!(first_error, None, "{id}");
                assert!(mismatch, "{id}: annotation and inline unit should differ");
            }
            Some("array_element_type_mismatch") => {
                assert_eq!(first_error, None, "{id}");
                assert!(annotated.windows(2).any(|w| w[0] != w[1]), "{id}");
            }
            Some(other) => panic!("{id}: no unit-level expectation for {other}"),
        }
    }
    assert_eq!(cases, 91);
}
