//! `bovnar::json` against expectations captured from the C reference's
//! `bovnar convert` (`tests/json/expected.json`, written by
//! `bovnar-json-diff --capture`), plus the round-trip contract and the cases
//! too large to keep as fixtures.

use std::path::Path;

use base64::Engine as _;
use bovnar::json::{self, JsonError, LossKind};
use bovnar::{detect, ErrorKind, Format, Limits};

/// The limits `bovnar convert` runs with.
fn cli_limits() -> Limits {
    let mut l = Limits::default();
    l.max_struct_nesting = 255;
    l.max_array_nesting = 255;
    l.max_input_bytes = 0;
    l.max_dom_nodes = u64::MAX;
    l
}

/// The error class `bovnar-json-diff` derives from the C CLI's stderr.
fn class(e: &JsonError) -> String {
    match e {
        JsonError::Syntax { .. } => "syntax".into(),
        JsonError::TooDeep { .. } => "too_deep".into(),
        JsonError::Nul { .. } => "nul".into(),
        JsonError::IntegerOutOfRange { .. } => "integer_out_of_range".into(),
        JsonError::FloatOverflow { .. } => "float_overflow".into(),
        JsonError::TrailingContent { .. } => "trailing".into(),
        JsonError::RootNotObject => "root".into(),
        JsonError::InvalidKey { .. } => "invalid_key".into(),
        JsonError::ControlCharacter { .. } => "control_character".into(),
        JsonError::InvalidUtf8 { .. } => "invalid_utf8".into(),
        JsonError::Writer(e) => format!("writer:{}", e.kind().as_str()),
        JsonError::Unrepresentable(e) => format!("unrepresentable:{}", e.kind().as_str()),
        JsonError::Parse(e) => format!("parse:{}", e.kind().as_str()),
        JsonError::FractionalDatetime { .. } => "fractional_datetime".into(),
        other => format!("{other:?}"),
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn b64(s: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .expect("base64")
}

/// The input of a captured case: inline, from the conformance corpus, or an
/// upstream example (`None` when the upstream tree is not next to the crate).
fn input_of(case: &serde_json::Value, corpus: &serde_json::Value) -> Option<Vec<u8>> {
    if let Some(input) = case["input"].as_str() {
        return Some(b64(input));
    }
    let origin = case["origin"].as_str().unwrap_or_default();
    if let Some(id) = origin.strip_prefix("corpus ") {
        let c = corpus["cases"].as_array()?.iter().find(|c| c["id"] == id)?;
        return Some(b64(c["input"].as_str()?));
    }
    let name = origin.strip_prefix("example ")?;
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples")
            .join(name),
    )
    .ok()
}

#[test]
fn matches_the_c_cli() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let expected: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("json/expected.json")).unwrap())
            .unwrap();
    let corpus: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("conformance/corpus.json")).unwrap(),
    )
    .unwrap();
    let limits = cli_limits();
    let mut failures = Vec::new();
    let mut checked = 0;
    for case in expected["cases"].as_array().unwrap() {
        let origin = case["origin"].as_str().unwrap();
        let Some(input) = input_of(case, &corpus) else {
            continue;
        };
        checked += 1;
        let (status, stdout, got_class, loss) = if case["direction"] == "json" {
            match json::to_bovnar(&input, &limits) {
                Ok(text) => (0, text.into_bytes(), String::new(), None),
                Err(e) => (1, Vec::new(), class(&e), None),
            }
        } else {
            match json::from_bovnar(&input, &limits) {
                Ok(out) => {
                    let loss: Vec<u32> = LossKind::ALL.iter().map(|&k| out.loss.count(k)).collect();
                    (
                        i64::from(!out.loss.is_empty()),
                        out.json.into_bytes(),
                        String::new(),
                        Some(loss),
                    )
                }
                Err(e) => (1, Vec::new(), class(&e), Some(vec![0; LossKind::ALL.len()])),
            }
        };
        let stdout_ok = match (case["stdout"].as_str(), case["stdout_fnv1a"].as_str()) {
            (Some(s), _) => s.as_bytes() == stdout.as_slice(),
            (None, Some(h)) => format!("{:016x}", fnv1a(&stdout)) == h,
            (None, None) => false,
        };
        let loss_ok = match (&loss, case["loss"].as_array()) {
            (Some(l), Some(want)) => {
                l.iter().map(|&n| u64::from(n)).collect::<Vec<_>>()
                    == want
                        .iter()
                        .filter_map(serde_json::Value::as_u64)
                        .collect::<Vec<_>>()
            }
            (None, None) => true,
            _ => false,
        };
        if case["status"].as_i64() != Some(status)
            || !stdout_ok
            || case["class"] != got_class.as_str()
            || !loss_ok
        {
            failures.push(format!(
                "{origin}: status {status}, class {got_class:?}, loss {loss:?}"
            ));
        }
    }
    assert!(checked > 400, "only {checked} cases checked");
    assert!(
        failures.is_empty(),
        "{} mismatches:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Upstream's `convert_json_roundtrip.cmake`: json -> bvnr -> json gives the
/// input back once whitespace is removed.
/// Upstream `tests/json/roundtrip.json`, when the upstream tree is next to
/// the crate.
fn upstream_roundtrip() -> Option<Vec<u8>> {
    std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/json/roundtrip.json")).ok()
}

#[test]
fn upstream_roundtrip_fixture() {
    let Some(input) = upstream_roundtrip() else {
        return;
    };
    let doc = json::to_document(&input, &cli_limits()).unwrap();
    let out = json::from_document(&doc).unwrap();
    assert!(out.loss.is_empty());
    let strip = |b: &[u8]| {
        b.iter()
            .copied()
            .filter(|c| !b" \t\r\n".contains(c))
            .collect::<Vec<_>>()
    };
    assert_eq!(strip(out.json.as_bytes()), strip(&input));
}

#[test]
fn nesting_follows_the_cli() {
    let limits = cli_limits();
    // 255 braces are the document plus 254 structs.
    let ok = format!("{}1{}", "{\"a\":".repeat(255), "}".repeat(255));
    assert!(json::to_bovnar(ok.as_bytes(), &limits).is_ok());
    let arrays = |n: usize| format!("{{\"a\":{}1{}}}", "[".repeat(n), "]".repeat(n));
    assert!(json::to_bovnar(arrays(255).as_bytes(), &limits).is_ok());
    let err = json::to_bovnar(arrays(256).as_bytes(), &limits).unwrap_err();
    assert!(matches!(err, JsonError::Writer(e) if e.kind() == ErrorKind::ArrayNestingTooHigh));
    // More than 512 containers are refused while parsing.
    let deep = format!("{}1{}", "[".repeat(513), "]".repeat(513));
    assert!(matches!(
        json::to_bovnar(deep.as_bytes(), &limits),
        Err(JsonError::TooDeep { .. })
    ));
    // The default limits allow 64 levels.
    let err = json::to_bovnar(arrays(65).as_bytes(), &Limits::default()).unwrap_err();
    assert!(matches!(err, JsonError::Writer(_)));
}

#[test]
fn output_is_capped_like_the_cli() {
    let members: Vec<String> = (0..3000).map(|i| format!("\"k{i}\":1")).collect();
    let nested = |depth: usize| {
        format!(
            "{}{{{}}}{}",
            "{\"a\":".repeat(depth - 1),
            members.join(","),
            "}".repeat(depth - 1)
        )
    };
    // Deep indentation makes the output more than 8x the input.
    let err = json::to_bovnar(nested(200).as_bytes(), &cli_limits()).unwrap_err();
    assert!(
        matches!(err, JsonError::Writer(e) if e.kind() == ErrorKind::SinkBufferExhausted),
        "{err:?}"
    );
    assert!(matches!(err, JsonError::Writer(e) if e.kind() == ErrorKind::SinkBufferExhausted));
    assert!(json::to_bovnar(nested(21).as_bytes(), &cli_limits()).is_ok());
}

#[test]
fn errors_name_the_key() {
    let limits = Limits::default();
    let err = json::to_bovnar(br#"{"ok":1.5,"big":{"x":[1.0,1e999]}}"#, &limits).unwrap_err();
    assert_eq!(
        err,
        JsonError::FloatOverflow {
            key: Some("x".into())
        }
    );
    let err = json::to_bovnar(b"{\"list\":[\"ok\",\"x\x7fy\"]}", &limits).unwrap_err();
    assert_eq!(
        err,
        JsonError::ControlCharacter {
            key: Some("list".into()),
            byte: 0x7f
        }
    );
    assert!(err.to_string().contains("U+007F"));
    let err = json::to_bovnar(br#"{"a b":1}"#, &limits).unwrap_err();
    assert_eq!(err, JsonError::InvalidKey { key: "a b".into() });
    let err = json::to_bovnar(b"{\"a\":1}\0x", &limits).unwrap_err();
    assert!(matches!(err, JsonError::TrailingContent { nul: true, .. }));
}

#[test]
fn input_cap_applies() {
    let mut limits = Limits::default();
    limits.max_input_bytes = 4;
    assert_eq!(
        json::to_bovnar(b"{\"a\":1}", &limits),
        Err(JsonError::TooLarge)
    );
}

#[test]
fn fractional_datetime_is_refused() {
    let err = json::from_bovnar(
        b"#!bovnar 1.2\n.t = 2026-06-15T12:00:00.123Z;\n",
        &Limits::default(),
    )
    .unwrap_err();
    assert_eq!(err, JsonError::FractionalDatetime { key: "t".into() });
}

#[test]
fn loss_report_lists_each_kind() {
    let out = json::from_bovnar(
        b".a = 1 m;\n.b = ethereum;\n.c = nan;\n",
        &Limits::default(),
    )
    .unwrap();
    assert_eq!(
        out.json,
        "{\n  \"a\": 1,\n  \"b\": \"ethereum\",\n  \"c\": null\n}\n"
    );
    assert_eq!(out.loss.total(), 3);
    assert_eq!(out.loss.example(LossKind::Unit), Some(".a (m)"));
    assert_eq!(out.loss.example(LossKind::NonFiniteFloat), Some(".c (nan)"));
    let report = out.loss.to_string();
    assert!(report.contains("1 value(s) lose their unit entirely, e.g. .a (m)"));
    assert!(report.contains("1 symbol(s) become plain strings, e.g. .b"));
}

#[test]
fn detects_the_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/json/inputs");
    let nested = std::fs::read(root.join("nested_empty.json")).unwrap();
    assert_eq!(detect(&nested), Format::Json);
    assert_eq!(
        detect(&std::fs::read(root.join("root_str.json")).unwrap()),
        Format::Unknown
    );
    let out = json::to_bovnar(&nested, &cli_limits()).unwrap();
    assert_eq!(detect(out.as_bytes()), Format::Bovnar);
}
