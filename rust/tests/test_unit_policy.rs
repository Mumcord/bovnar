//! Port of `python/tests/test_unit_policy.py`: the reader-side unit policy,
//! its DOM and writer halves, and the unit predicates it builds on.
//!
//! Every Python test is ported. `DomDoc.parse_file(path, policy)` is
//! `DomDoc::parse_reader_with_policy` over the opened file. `Writer.write_float(..., unit_str=...)` is
//! `Writer::write_quantity` with a float `Quantity` in that unit, and a
//! `UnitPolicy` target given as a bare string is `(unit, 0)`.

use bovnar::units::{unit_si_normal_form, unit_to_str, units_compatible, units_convertible};
use bovnar::{
    DomDoc, Error, ErrorCode, Event, EventData, Quantity, ReadOptions, Reader, Unit, UnitPolicy,
    UnitRule, ValueTypeFamily, ValueTypeSpec, Writer,
};

/// One row per numeric value: (converted, converted text, native unit,
/// converted unit).
type Row = (bool, Option<String>, Option<String>, Option<String>);

fn collect(doc: &str, policy: Option<&UnitPolicy>) -> Result<Vec<Row>, Error> {
    let mut rows = Vec::new();
    let mut on_value = |d: &EventData<'_>| {
        if d.event != Event::Data {
            return true;
        }
        let numeric = matches!(
            ValueTypeFamily::from_raw(d.value_type.family),
            Some(
                ValueTypeFamily::Uint
                    | ValueTypeFamily::Sint
                    | ValueTypeFamily::Float
                    | ValueTypeFamily::FloatFix
                    | ValueTypeFamily::FloatDec
            )
        );
        if !numeric {
            return true;
        }
        rows.push((
            d.converted,
            d.converted
                .then(|| String::from_utf8_lossy(d.converted_text).into_owned()),
            Some(unit_to_str(&d.value_unit).unwrap()),
            d.converted.then(|| unit_to_str(&d.converted_unit).unwrap()),
        ));
        true
    };
    let mut r = Reader::new()?;
    r.set_unit_policy(policy)?;
    r.read_mem(
        doc.as_bytes(),
        &ReadOptions::default(),
        Some(&mut on_value),
        None,
    )?;
    Ok(rows)
}

fn rows(doc: &str, policy: &UnitPolicy) -> Vec<Row> {
    collect(doc, Some(policy)).unwrap()
}

fn head(row: &Row) -> (bool, Option<&str>) {
    (row.0, row.1.as_deref())
}

fn targets(units: &[&str]) -> Vec<(String, u32)> {
    units.iter().map(|u| (u.to_string(), 0)).collect()
}

fn strings(units: &[&str]) -> Vec<String> {
    units.iter().map(|u| u.to_string()).collect()
}

fn policy_targets(units: &[&str]) -> UnitPolicy {
    UnitPolicy {
        targets: targets(units),
        ..UnitPolicy::default()
    }
}

fn policy_rules(rules: Vec<UnitRule>) -> UnitPolicy {
    UnitPolicy {
        rules,
        ..UnitPolicy::default()
    }
}

fn assertion(path: &str, unit: &str) -> UnitRule {
    UnitRule {
        convert: false,
        ..UnitRule::new(path, unit)
    }
}

fn parse_err_code(r: Result<Vec<Row>, Error>) -> Option<ErrorCode> {
    match r {
        Err(e @ Error::Parse { .. }) => e.code(),
        other => panic!("expected a parse error, got {other:?}"),
    }
}

fn is_parse_err<T: std::fmt::Debug>(r: Result<T, Error>) -> bool {
    matches!(r, Err(Error::Parse { .. }))
}

fn is_arg_err<T: std::fmt::Debug>(r: Result<T, Error>) -> bool {
    matches!(r, Err(Error::Argument { .. }))
}

fn u(text: &str) -> Unit {
    Unit::parse(text).unwrap()
}

/// Python's `Writer.write_float(key, value, unit_str=unit)`.
fn write_float(w: &mut Writer, key: &str, value: f64, unit: Option<&str>) -> Result<(), Error> {
    let vt = ValueTypeSpec {
        family: ValueTypeFamily::Float as i32,
        width: 64,
        base: 0,
    };
    let q = Quantity::new(&format!("{value:?}"), vt, unit.map(u).unwrap_or_default());
    w.write_quantity(key, &q)
}

fn writer() -> Writer {
    Writer::to_mem(1 << 16, false).unwrap()
}

// ---- TestTargets -----------------------------------------------------------

#[test]
fn test_mixed_document() {
    let r = rows(
        ".inlet_temp     = <float:64,°F>      212.0;\n\
         .tank_level     = <float:64,in>      12.0;\n\
         .duty_cycle     = <float:64,%>       35.0;\n\
         .spare_capacity = <float:64>         0.25;\n\
         .unit_price     = <float:64,k~$USD>  5.0;\n",
        &policy_targets(&["°C", "m"]),
    );
    assert_eq!(r.len(), 5);
    assert_eq!(head(&r[0]), (true, Some("100")));
    assert_eq!(head(&r[1]), (true, Some("0.3048")));
    assert!(!r[2].0); // % matches no target
    assert!(!r[3].0); // a bare number matches no target
    assert!(!r[4].0); // a currency is neither
}

#[test]
fn test_order_is_significant() {
    assert!(!rows(".d = 5000 m;", &policy_targets(&["m", "k~m"]))[0].0);
    let r = rows(".d = 5000 m;", &policy_targets(&["k~m", "m"]));
    assert_eq!(head(&r[0]), (true, Some("5")));
}

#[test]
fn test_per_target_output_base() {
    let p = UnitPolicy {
        targets: vec![("m".into(), 16)],
        ..UnitPolicy::default()
    };
    let r = rows(".d = 5 k~m;", &p);
    assert_eq!(head(&r[0]), (true, Some("1388")));
}

#[test]
fn test_unitless_is_fenced_from_the_ratio_units() {
    // A bare number is dimensionally a percentage. Delivering 0.25 as 25 is
    // exactly the silent rescale the format exists to prevent.
    assert!(!rows(".x = <float:64> 0.25;", &policy_targets(&["%"]))[0].0);
    assert!(!rows(".x = <float:64,%> 35.0;", &policy_targets(&["no_unit"]))[0].0);
    // ...while a real ratio-to-ratio conversion is untouched by the fence.
    let r = rows(".x = <float:64,%> 35.0;", &policy_targets(&["ppm"]));
    assert_eq!(head(&r[0]), (true, Some("350000")));
}

#[test]
fn test_currency_prefix_delta() {
    // units_compatible says no; units_convertible (and so the policy) says yes.
    let r = rows(".p = <float:64,k~$USD> 5.0;", &policy_targets(&["$USD"]));
    assert_eq!(head(&r[0]), (true, Some("5000")));
}

// ---- TestExactness ---------------------------------------------------------

#[test]
fn test_nonterminating_raises_by_default() {
    let r = collect(
        ".v = <float:64,k~m/h> 42.0;",
        Some(&policy_targets(&["m/s"])),
    );
    assert_eq!(parse_err_code(r), Some(ErrorCode::UnitInexact));
}

#[test]
fn test_leave_inexact_delivers_the_native_value() {
    let p = UnitPolicy {
        leave_inexact: true,
        ..policy_targets(&["m/s"])
    };
    let r = rows(
        ".v = <float:64,k~m/h> 42.0;\n.w = <float:64,k~m/h> 36.0;\n",
        &p,
    );
    assert!(!r[0].0 && r[0].2.as_deref() == Some("k~m/h"));
    assert_eq!(head(&r[1]), (true, Some("10"))); // 36 km/h IS 10 m/s exactly
}

#[test]
fn test_irrational_factor() {
    assert!(is_parse_err(collect(
        ".a = 90 °;",
        Some(&policy_targets(&["rad"]))
    )));
    let p = UnitPolicy {
        leave_inexact: true,
        ..policy_targets(&["rad"])
    };
    assert!(!rows(".a = 90 °;", &p)[0].0);
}

// ---- TestNormalise ---------------------------------------------------------

fn normalise() -> UnitPolicy {
    UnitPolicy {
        normalise_si: true,
        ..UnitPolicy::default()
    }
}

#[test]
fn test_si() {
    let r = rows(
        ".len  = <float:64,in>  12.0;\n\
         .temp = <float:64,°F>  212.0;\n\
         .mass = <float:64,g>   5.0;\n",
        &normalise(),
    );
    assert!(r[0].1.as_deref() == Some("0.3048") && r[0].3.as_deref() == Some("m"));
    assert!(r[1].1.as_deref() == Some("373.15") && r[1].3.as_deref() == Some("K"));
    assert!(r[2].1.as_deref() == Some("0.005") && r[2].3.as_deref() == Some("k~g"));
}

#[test]
fn test_dimensionless_and_currency_are_left_alone() {
    let p = UnitPolicy {
        leave_inexact: true,
        ..normalise()
    };
    let r = rows(
        ".ratio = <float:64,%>    35.0;\n\
         .gain  = <float:64,dB>   3.0;\n\
         .acid  = <float:64,pH>   7.2;\n\
         .angle = <float:64,°>    90.0;\n\
         .money = <float:64,$USD> 5.0;\n",
        &p,
    );
    assert_eq!(r.iter().map(|r| r.0).collect::<Vec<_>>(), vec![false; 5]);
}

#[test]
fn test_photometric_units_are_left_alone() {
    // lm/lx/ph carry the steradian's quantity kind, which no dimension vector
    // can express; rebuilding from dimensions alone would produce cd, which is
    // a different quantity and which the converter refuses.
    let r = rows(
        ".flux = <float:64,lm> 800.0;\n.d = <float:64,k~m> 5.0;\n",
        &normalise(),
    );
    assert!(!r[0].0);
    assert_eq!(head(&r[1]), (true, Some("5000")));
}

#[test]
fn test_targets_outrank_normalise() {
    let p = UnitPolicy {
        normalise_si: true,
        ..policy_targets(&["k~m/h"])
    };
    let r = rows(
        ".v = <float:64,m/s> 10.0;\n.len = <float:64,in> 12.0;\n",
        &p,
    );
    assert_eq!(head(&r[0]), (true, Some("36"))); // target won
    assert_eq!(head(&r[1]), (true, Some("0.3048"))); // fell through to normalise
}

// ---- TestAssertions --------------------------------------------------------

fn require_unit() -> UnitPolicy {
    UnitPolicy {
        require_unit: true,
        ..UnitPolicy::default()
    }
}

fn require_dims(units: &[&str]) -> UnitPolicy {
    UnitPolicy {
        require_dimension_of: strings(units),
        ..UnitPolicy::default()
    }
}

#[test]
fn test_require_unit() {
    assert_eq!(rows(".a = 5 m;\n.b = 25 °C;\n", &require_unit()).len(), 2);
    for doc in [
        ".a = 5 m;\n.b = 7;\n",
        ".a = 5 m;\n.b = <uint:32,no_unit> 7;\n",
    ] {
        let r = collect(doc, Some(&require_unit()));
        assert_eq!(parse_err_code(r), Some(ErrorCode::UnitMismatch));
    }
}

#[test]
fn test_require_dimension() {
    let p = require_dims(&["m"]);
    assert_eq!(rows(".a = 5 k~m;\n.b = 12 in;\n.c = 3 mi;\n", &p).len(), 3);
    assert!(is_parse_err(collect(
        ".a = 5 k~m;\n.b = 25 °C;\n",
        Some(&p)
    )));
    // several requirements: satisfying one is enough
    assert_eq!(
        rows(".a = 5 k~m;\n.b = 25 °C;\n", &require_dims(&["m", "K"])).len(),
        2
    );
}

#[test]
fn test_require_dimension_fences_unitless() {
    assert!(is_parse_err(collect(
        ".x = <float:64> 0.25;",
        Some(&require_dims(&["%"]))
    )));
    assert_eq!(
        rows(".x = <float:64> 0.25;", &require_dims(&["no_unit"])).len(),
        1
    );
}

#[test]
fn test_assertions_see_the_native_unit() {
    // Requirement and target are both temperatures; the document is a
    // length. Evaluating after conversion would have made this pass.
    let p = UnitPolicy {
        require_dimension_of: strings(&["K"]),
        ..policy_targets(&["K"])
    };
    assert!(is_parse_err(collect(".t = 25 °C;\n.d = 5 m;\n", Some(&p))));
}

// ---- TestPolicyLifecycle ---------------------------------------------------

fn converted_texts(r: &mut Reader, doc: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut h = |d: &EventData<'_>| {
        if d.event == Event::Data && d.converted {
            out.push(String::from_utf8_lossy(d.converted_text).into_owned());
        }
        true
    };
    r.read_mem(doc, &ReadOptions::default(), Some(&mut h), None)
        .unwrap();
    out
}

#[test]
fn test_survives_reuse_and_clears() {
    let mut r = Reader::new().unwrap();
    r.set_unit_policy(Some(&policy_targets(&["m"]))).unwrap();
    for _ in 0..2 {
        assert_eq!(converted_texts(&mut r, b".d = 5 k~m;"), vec!["5000"]);
    }
    r.set_unit_policy(None).unwrap();
    let mut seen = Vec::new();
    let mut h = |d: &EventData<'_>| {
        if d.event == Event::Data && d.value_type.family == ValueTypeFamily::Uint as i32 {
            seen.push(d.converted);
        }
        true
    };
    r.read_mem(b".d = 5 k~m;", &ReadOptions::default(), Some(&mut h), None)
        .unwrap();
    assert_eq!(seen, vec![false]);
}

#[test]
fn test_bad_unit_raises_before_the_read() {
    let mut r = Reader::new().unwrap();
    assert!(is_arg_err(
        r.set_unit_policy(Some(&policy_targets(&["not_a_unit_at_all"])))
    ));
    assert!(is_arg_err(
        r.set_unit_policy(Some(&require_dims(&["nonsense"])))
    ));
    let based = UnitPolicy {
        targets: vec![("m".into(), 63)],
        ..UnitPolicy::default()
    };
    assert!(is_arg_err(r.set_unit_policy(Some(&based))));
    assert!(is_arg_err(
        r.set_unit_policy(Some(&policy_targets(&["m"; 9])))
    ));
}

#[test]
fn test_a_rejected_policy_leaves_the_previous_one() {
    let mut r = Reader::new().unwrap();
    r.set_unit_policy(Some(&policy_targets(&["m"]))).unwrap();
    assert!(is_arg_err(
        r.set_unit_policy(Some(&policy_targets(&["still_not_a_unit"])))
    ));
    assert_eq!(converted_texts(&mut r, b".d = 5 k~m;"), vec!["5000"]);
}

// ---- TestFieldRules --------------------------------------------------------

const NESTED: &str = ".inlet = {\n\
                      \x20 .temperature = <float:64,°F> 212.0;\n\
                      \x20 .flow        = <float:64,in> 12.0;\n\
                      };\n\
                      .outlet = {\n\
                      \x20 .temperature = <float:64,°F> 32.0;\n\
                      };\n";

#[test]
fn test_exact_path() {
    let r = rows(
        NESTED,
        &policy_rules(vec![UnitRule::new(".inlet.temperature", "°C")]),
    );
    assert_eq!(r.len(), 3);
    assert_eq!(head(&r[0]), (true, Some("100"))); // named
    assert!(!r[1].0); // its sibling
    assert!(!r[2].0); // same key, different parent
}

#[test]
fn test_wildcard_and_component_boundary() {
    let r = rows(
        ".in    = { .a = <float:64,in> 12.0; };\n\
         .inlet = { .a = <float:64,in> 12.0; };\n",
        &policy_rules(vec![UnitRule::new(".in.*", "m")]),
    );
    assert!(r[0].0); // .in.a
    assert!(!r[1].0); // .inlet.a is NOT under .in
}

#[test]
fn test_a_rule_is_an_assertion() {
    let p = policy_rules(vec![assertion(".speed", "m/s")]);
    let r = rows(".speed = <float:64,k~m/h> 42.0;", &p);
    assert!(!r[0].0 && r[0].2.as_deref() == Some("k~m/h"));
    for bad in [".speed = <float:64,°C> 42.0;", ".speed = <float:64> 42.0;"] {
        assert_eq!(
            parse_err_code(collect(bad, Some(&p))),
            Some(ErrorCode::UnitMismatch)
        );
    }
}

#[test]
fn test_rules_outrank_targets() {
    let p = UnitPolicy {
        rules: vec![UnitRule::new(".a", "k~m")],
        ..policy_targets(&["m"])
    };
    let r = rows(
        ".a = <float:64,in> 12000.0;\n.b = <float:64,in> 12.0;\n",
        &p,
    );
    assert_eq!(r[0].3.as_deref(), Some("k~m")); // the rule
    assert_eq!(r[1].3.as_deref(), Some("m")); // the target list
}

#[test]
fn test_a_bad_policy_is_refused_by_parse_fd_too() {
    let path =
        std::env::temp_dir().join(format!("bovnar-test-policy-fd-{}.bvnr", std::process::id()));
    std::fs::write(&path, b".a = 5 m;").unwrap();
    let file = std::fs::File::open(&path).unwrap();
    let result = DomDoc::parse_reader_with_policy(file, &policy_targets(&["nosuch"]));
    std::fs::remove_file(&path).unwrap();
    match result {
        Err(Error::Argument { message: m, .. }) => assert!(m.contains("nosuch"), "{m}"),
        Err(other) => panic!("expected an argument error, got {other:?}"),
        Ok(_) => panic!("expected an argument error, got a document"),
    }
}

#[test]
fn test_across_an_array_of_structs() {
    // One assignment, one key, many rows: the rule has to reach every row.
    let doc = ".holdings = [\n\
               \x20 { .amount = <float:64,in>  12.0; },\n\
               \x20 { .amount = <float:64,k~m>  1.0; },\n\
               \x20 { .amount = <float:64,in>  24.0; }\n\
               ];\n";
    let p = policy_rules(vec![UnitRule::new(".holdings.amount", "m")]);
    let r = rows(doc, &p);
    assert_eq!(
        r.iter().map(|r| r.1.as_deref()).collect::<Vec<_>>(),
        vec![Some("0.3048"), Some("1000"), Some("0.6096")]
    );

    // and the assertion half reaches every row too
    let bad = ".holdings = [ { .amount = <float:64,in> 12.0; },\n\
               \x20             { .amount = <float:64,°C>  5.0; } ];\n";
    assert_eq!(
        parse_err_code(collect(bad, Some(&p))),
        Some(ErrorCode::UnitMismatch)
    );
}

#[test]
fn test_bad_rule_paths_are_refused() {
    let mut r = Reader::new().unwrap();
    for bad in [
        UnitRule::new("a.b", "m"), // no leading dot
        UnitRule::new(".*", "m"),  // names the whole document
        UnitRule::new(".a", "not_a_unit"),
    ] {
        assert!(is_arg_err(
            r.set_unit_policy(Some(&policy_rules(vec![bad])))
        ));
    }
}

// ---- TestDomPolicy ---------------------------------------------------------

fn dom_err(r: Result<DomDoc, Error>) -> Error {
    match r {
        Ok(_) => panic!("expected the DOM parse to fail"),
        Err(e) => e,
    }
}

#[test]
fn test_conversion_is_stored() {
    let doc = DomDoc::parse_with_policy(
        NESTED.as_bytes(),
        &policy_rules(vec![UnitRule::new(".inlet.temperature", "°C")]),
    )
    .unwrap();
    let n = doc.lookup(".inlet.temperature").unwrap();
    assert!((n.as_float().unwrap() - 100.0).abs() < 1e-9);
    assert_eq!(n.unit_str(), "°C");
    // the sibling is as the document wrote it
    let flow = doc.lookup(".inlet.flow").unwrap().as_float().unwrap();
    assert!((flow - 12.0).abs() < 1e-9);
}

#[test]
fn test_validation() {
    let e = dom_err(DomDoc::parse_with_policy(
        b".a = 5 m;\n.b = 7;",
        &require_unit(),
    ));
    assert!(matches!(e, Error::Parse { .. }));
    assert_eq!(e.code(), Some(ErrorCode::UnitMismatch));
}

#[test]
fn test_a_bad_policy_is_not_a_parse_error() {
    // The DOM path validates the policy as Reader::set_unit_policy does and
    // gives the same argument error, naming the bad entry.
    let bad = policy_targets(&["not_a_unit_at_all"]);
    let dom_msg = match dom_err(DomDoc::parse_with_policy(b".a = 5 m;", &bad)) {
        Error::Argument { message: m, .. } => m,
        other => panic!("expected an argument error, got {other:?}"),
    };
    let reader_msg = match Reader::new().unwrap().set_unit_policy(Some(&bad)) {
        Err(Error::Argument { message: m, .. }) => m,
        other => panic!("expected an argument error, got {other:?}"),
    };
    assert!(dom_msg.contains("not_a_unit_at_all"), "{dom_msg}");
    assert_eq!(dom_msg, reader_msg);
    let rule = UnitPolicy {
        rules: vec![UnitRule::new(".a", "zz")],
        ..UnitPolicy::default()
    };
    match dom_err(DomDoc::parse_with_policy(b".a = 5 m;", &rule)) {
        Error::Argument { message: m, .. } => assert!(m.contains("\"zz\""), "{m}"),
        other => panic!("expected an argument error, got {other:?}"),
    }
}

#[test]
fn test_dom_across_an_array_of_structs() {
    let doc = DomDoc::parse_with_policy(
        b".h = [ { .a = <float:64,in> 12.0; },\n       { .a = <float:64,in> 24.0; } ];\n",
        &policy_rules(vec![UnitRule::new(".h.a", "m")]),
    )
    .unwrap();
    let rows = doc.lookup(".h").unwrap();
    let a = |i| rows.at(i).unwrap().get("a").unwrap().as_float().unwrap();
    assert!((a(0) - 0.3048).abs() < 1e-12);
    assert!((a(1) - 0.6096).abs() < 1e-12);
}

#[test]
fn test_integer_converting_to_a_fraction_becomes_a_float() {
    let doc = DomDoc::parse_with_policy(
        b".m = <uint:32,g> 5;",
        &policy_rules(vec![UnitRule::new(".m", "k~g")]),
    )
    .unwrap();
    assert!((doc.lookup(".m").unwrap().as_float().unwrap() - 0.005).abs() < 1e-12);
}

// ---- TestWriterPolicy ------------------------------------------------------

#[test]
fn test_writer_require_unit() {
    let mut w = writer();
    w.set_unit_policy(Some(&require_unit())).unwrap();
    write_float(&mut w, "speed", 9.81, Some("m/s")).unwrap(); // accepted
    let e = write_float(&mut w, "bare", 0.25, None).unwrap_err();
    assert!(matches!(e, Error::Write { .. }));
    assert_eq!(e.code(), Some(ErrorCode::UnitMismatch));
}

#[test]
fn test_writer_require_dimension() {
    let mut w = writer();
    w.set_unit_policy(Some(&require_dims(&["m"]))).unwrap();
    write_float(&mut w, "a", 12.0, Some("in")).unwrap(); // a length, accepted
    let e = write_float(&mut w, "b", 25.0, Some("°C")).unwrap_err();
    assert!(matches!(e, Error::Write { .. }));
    assert_eq!(e.code(), Some(ErrorCode::UnitMismatch));
}

#[test]
fn test_non_numeric_values_are_not_subject_to_it() {
    let mut w = writer();
    w.set_unit_policy(Some(&require_unit())).unwrap();
    // No error is the assertion: none of these carries a unit, and none of
    // them is the policy's business.
    w.write_string("name", "sensor-1").unwrap();
    w.write_bool("ok", true).unwrap();
    w.write_null("spare").unwrap();
    w.finish().unwrap();
}

#[test]
fn test_conversion_fields_are_refused() {
    let mut w = writer();
    for bad in [
        policy_targets(&["m"]),
        normalise(),
        UnitPolicy {
            base: 16,
            ..UnitPolicy::default()
        },
        UnitPolicy {
            leave_inexact: true,
            ..UnitPolicy::default()
        },
    ] {
        assert!(is_arg_err(w.set_unit_policy(Some(&bad))));
    }
    assert!(is_arg_err(
        w.set_unit_policy(Some(&require_dims(&["not_a_unit"])))
    ));
}

#[test]
fn test_what_the_writer_emits_the_reader_accepts() {
    // A document that passed the check on the way out passes it on the way
    // back in: it is the same check.
    let policy = UnitPolicy {
        require_unit: true,
        ..require_dims(&["m"])
    };
    let mut w = writer();
    w.set_unit_policy(Some(&policy)).unwrap();
    write_float(&mut w, "a", 12.0, Some("in")).unwrap();
    write_float(&mut w, "b", 5.0, Some("k~m")).unwrap();
    w.finish().unwrap();
    let doc = String::from_utf8(w.get_output().to_vec()).unwrap();

    assert_eq!(rows(&doc, &policy).len(), 2);
}

// ---- TestUnitPredicates ----------------------------------------------------

#[test]
fn test_units_convertible_covers_what_compatible_misses() {
    let (usd, kusd) = (u("$USD"), u("k~$USD"));
    assert!(!units_compatible(&kusd, &usd));
    assert!(units_convertible(&kusd, &usd));
    // and it still refuses genuinely unrelated units
    assert!(!units_convertible(&u("m"), &u("s")));
}

#[test]
fn test_si_normal_form() {
    assert_eq!(
        unit_to_str(&unit_si_normal_form(&u("in")).unwrap()).unwrap(),
        "m"
    );
    assert_eq!(
        unit_to_str(&unit_si_normal_form(&u("g")).unwrap()).unwrap(),
        "k~g"
    );
    for none_case in ["%", "ppm", "dB", "pH", "rad", "$USD", "lm", "s/°C"] {
        assert!(unit_si_normal_form(&u(none_case)).is_none(), "{none_case}");
    }
}

// ---- TestPolicyRefusalNamesTheEntry ----------------------------------------

const BAD_UNIT: &str = "not_a_unit_at_all";

fn msg(policy: UnitPolicy) -> String {
    let mut r = Reader::new().unwrap();
    match r.set_unit_policy(Some(&policy)) {
        Err(Error::Argument { message: m, .. }) => m,
        other => panic!("expected an argument error, got {other:?}"),
    }
}

#[test]
fn test_targets_entry_is_named() {
    let m = msg(policy_targets(&[BAD_UNIT]));
    assert!(m.contains("targets") && m.contains(BAD_UNIT), "{m}");
}

#[test]
fn test_require_dimension_entry_is_named() {
    let m = msg(require_dims(&[BAD_UNIT]));
    assert!(
        m.contains("require_dimension_of") && m.contains(BAD_UNIT),
        "{m}"
    );
}

#[test]
fn test_a_bad_rule_unit_names_the_rule_and_the_unit() {
    let m = msg(policy_rules(vec![UnitRule::new(".inlet.speed", BAD_UNIT)]));
    assert!(m.contains(".inlet.speed") && m.contains(BAD_UNIT), "{m}");
    // ...and calls it a unit, not a path.
    assert!(!m.contains("path"), "{m}");
}

#[test]
fn test_a_bad_rule_path_is_called_a_path() {
    let long = format!(".{}", "a".repeat(97));
    for path in ["a.b", ".*", long.as_str()] {
        let m = msg(policy_rules(vec![UnitRule::new(path, "m")]));
        assert!(m.contains("path"), "{m}");
        // The old message blamed unit spellings for every one of these.
        assert!(!m.contains("unit spellings"), "{m}");
    }
}

#[test]
fn test_a_bad_base_is_called_a_base() {
    let based_target = UnitPolicy {
        targets: vec![("m".into(), 63)],
        ..UnitPolicy::default()
    };
    assert!(msg(based_target).contains("base"));
    let based_rule = UnitRule {
        base: 63,
        ..UnitRule::new(".a", "m")
    };
    assert!(msg(policy_rules(vec![based_rule])).contains("base"));
    let based_policy = UnitPolicy {
        base: 63,
        ..policy_targets(&["m"])
    };
    assert!(msg(based_policy).contains("base"));
}

#[test]
fn test_the_writer_names_rules_too() {
    // The writer's message listed only require_dimension_of, and its setter
    // takes rules (in convert=false form).
    let mut w = writer();
    let e = w
        .set_unit_policy(Some(&policy_rules(vec![assertion(".a", BAD_UNIT)])))
        .unwrap_err();
    let Error::Argument { message: m, .. } = e else {
        panic!("expected an argument error, got {e:?}");
    };
    assert!(m.contains(".a") && m.contains(BAD_UNIT), "{m}");
}

#[test]
fn test_the_first_bad_entry_is_the_one_reported() {
    // Good entries before the bad one must not be accused.
    let m = msg(policy_targets(&["m", "k~m/s", BAD_UNIT]));
    assert!(m.contains(BAD_UNIT) && !m.contains("k~m/s"), "{m}");
}

#[test]
fn test_a_valid_policy_is_still_accepted() {
    // The probe must not reject anything the setter takes.
    let mut r = Reader::new().unwrap();
    r.set_unit_policy(Some(&UnitPolicy {
        targets: vec![("m".into(), 0), ("k~m".into(), 16)],
        base: 10,
        normalise_si: true,
        leave_inexact: true,
        require_unit: false,
        require_dimension_of: strings(&["s"]),
        rules: vec![UnitRule::new(".a.b", "m"), assertion(".c.*", "s")],
    }))
    .unwrap();
}
