//! Arbitrary text through from_json: refused or converted, never a crash. A
//! converted document converts back to JSON, and the JSON settles: each trip
//! may drop what JSON cannot carry (a float with an integral value becomes an
//! integer, then `-0` becomes `0`), but from the second trip on, converting
//! to bovnar and out again must give the same JSON.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(doc) = bovnar::json::from_json(text) else {
        return;
    };
    let json = bovnar::json::to_json(&doc).expect("from_json output must convert back");
    let doc2 = bovnar::json::from_json(&json).expect("to_json output must convert");
    let json2 = bovnar::json::to_json(&doc2).expect("from_json output must convert back");
    let doc3 = bovnar::json::from_json(&json2).expect("to_json output must convert");
    let json3 = bovnar::json::to_json(&doc3).expect("from_json output must convert back");
    assert_eq!(json3, json2, "first trip: {json}");
});
