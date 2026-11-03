//! Data-driven conformance test against the upstream corpus.
//!
//! `tests/conformance/corpus.json` holds every case of upstream
//! `tests/bvnr_conformance.c` together with the exact stdout and exit code of
//! the C reference IUT adapter (built with UCUM and CF off). Streaming cases
//! must reproduce that output byte for byte; DOM-tier cases must produce the
//! expected error. Regenerate the corpus with `scripts/extract-corpus.py`.

use std::collections::BTreeMap;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use bovnar::{conformance, Limits};
use serde_json::Value;

/// Whether the DOM-tier cases run through the document builder
/// (`conformance::dom_check`); switch off to test the reader alone.
const DOM_TIER: bool = true;

const CORPUS: &str = include_str!("conformance/corpus.json");

#[derive(Default)]
struct Tally {
    passed: usize,
    failed: usize,
    skipped: usize,
}

/// How a case went when it did not fail.
enum Outcome {
    Passed,
    /// DOM-tier case while [`DOM_TIER`] is off.
    Skipped,
}

#[test]
fn conformance_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("corpus.json is valid JSON");
    let cases = corpus["cases"].as_array().expect("corpus has a case list");

    let mut groups: BTreeMap<String, Tally> = BTreeMap::new();
    let mut failures = Vec::new();
    for case in cases {
        let id = str_field(case, "id");
        let tally = groups
            .entry(str_field(case, "group").to_owned())
            .or_default();
        match run_case(case) {
            Ok(Outcome::Passed) => tally.passed += 1,
            Ok(Outcome::Skipped) => tally.skipped += 1,
            Err(why) => {
                tally.failed += 1;
                failures.push(format!("{id}: {why}"));
            }
        }
    }

    for (group, tally) in &groups {
        println!(
            "{group:<20} {}/{} passed, {} failed, {} skipped (DOM tier)",
            tally.passed,
            tally.passed + tally.failed,
            tally.failed,
            tally.skipped
        );
    }
    for failure in &failures {
        println!("FAIL {failure}");
    }
    assert!(
        failures.is_empty(),
        "{} of {} conformance cases failed",
        failures.len(),
        cases.len()
    );
}

fn run_case(case: &Value) -> Result<Outcome, String> {
    let input = decode(str_field(case, "input"));
    let limits = limits(case);

    if case["dom_validate"].as_bool() == Some(true) {
        if !DOM_TIER {
            return Ok(Outcome::Skipped);
        }
        let expected = case["expected_error"].as_str();
        return match (conformance::dom_check(&input, &limits), expected) {
            (Ok(()), None) => Ok(Outcome::Passed),
            (Err(err), Some(name)) if err.kind().as_str() == name => Ok(Outcome::Passed),
            (Ok(()), Some(name)) => Err(format!("accepted, expected error {name}")),
            (Err(err), _) => Err(format!(
                "error {}, expected {}",
                err.kind().as_str(),
                expected.unwrap_or("success")
            )),
        };
    }

    let reference = &case["reference"];
    let want_stdout = decode(str_field(reference, "stdout"));
    let want_ok = reference["exit_code"].as_i64() == Some(0);
    let (got_stdout, got_ok) = match conformance::event_log(&input, &limits) {
        Ok(log) => (log.into_bytes(), true),
        Err(err) => (
            format!("ERROR {}\n", err.kind().as_str()).into_bytes(),
            false,
        ),
    };
    if got_ok != want_ok {
        return Err(format!(
            "exit status differs (expected {}); stdout: {}",
            if want_ok { "success" } else { "failure" },
            first_line(&got_stdout)
        ));
    }
    if got_stdout != want_stdout {
        let at = got_stdout
            .iter()
            .zip(&want_stdout)
            .take_while(|(a, b)| a == b)
            .count();
        let line = want_stdout[..at].iter().filter(|&&b| b == b'\n').count() + 1;
        return Err(format!(
            "stdout differs at byte {at} (line {line}): expected {:?}, got {:?}",
            first_line(&want_stdout[line_start(&want_stdout, at)..]),
            first_line(&got_stdout[line_start(&got_stdout, at)..]),
        ));
    }
    Ok(Outcome::Passed)
}

/// Builds the limits a case runs with: defaults plus its `--max-*` overrides.
fn limits(case: &Value) -> Limits {
    let mut limits = Limits::default();
    let Some(overrides) = case["limits"].as_object() else {
        return limits;
    };
    for (name, value) in overrides {
        let value = value
            .as_u64()
            .expect("limit overrides are unsigned integers");
        let narrow = |max: u64| value.min(max);
        match name.as_str() {
            "max_id" => limits.max_identifier_length = narrow(u16::MAX.into()) as u16,
            "max_str" => limits.max_string_length = narrow(u16::MAX.into()) as u16,
            "max_num" => limits.max_number_length = narrow(u16::MAX.into()) as u16,
            "max_sym" => limits.max_symbol_length = narrow(u16::MAX.into()) as u16,
            "max_ref" => limits.max_reference_length = narrow(u16::MAX.into()) as u16,
            "max_snest" => limits.max_struct_nesting = narrow(u8::MAX.into()) as u8,
            "max_anest" => limits.max_array_nesting = narrow(u8::MAX.into()) as u8,
            "max_aitems" => limits.max_array_items = value,
            other => panic!("unknown limit override {other}"),
        }
    }
    limits
}

fn str_field<'a>(value: &'a Value, name: &str) -> &'a str {
    value[name]
        .as_str()
        .unwrap_or_else(|| panic!("missing string field {name}"))
}

fn decode(b64: &str) -> Vec<u8> {
    STANDARD
        .decode(b64)
        .expect("corpus fields are valid base64")
}

fn line_start(bytes: &[u8], at: usize) -> usize {
    bytes[..at.min(bytes.len())]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1)
}

fn first_line(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|&b| b == b'\n')
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
