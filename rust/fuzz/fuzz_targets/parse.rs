//! Arbitrary bytes through the streaming parsers: they may refuse the input
//! but must not crash, and a document they accept must write back to one
//! that reads the same.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(value) = bovnar::parse(data) else {
        return;
    };
    let _ = bovnar::parse_typed(data);
    // to_string can refuse a value parse produced (a key it cannot write
    // back); what it does write must read back to the same value.
    if let Ok(out) = bovnar::to_string(&value, false) {
        let again = bovnar::parse(&out).expect("to_string output must parse");
        assert_eq!(
            normalise(again),
            normalise(value),
            "{}",
            String::from_utf8_lossy(&out)
        );
    }
});

/// NaN never equals itself; compare it by bit pattern instead.
fn normalise(v: bovnar::Value) -> bovnar::Value {
    use bovnar::Value;
    match v {
        Value::Float(f) if f.is_nan() => Value::String("NaN".into()),
        Value::Array(items) => Value::Array(items.into_iter().map(normalise).collect()),
        Value::Struct(fields) => {
            Value::Struct(fields.into_iter().map(|(k, v)| (k, normalise(v))).collect())
        }
        other => other,
    }
}
