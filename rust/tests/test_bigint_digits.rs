//! Port of `python/tests/test_bigint_digits.py`: integers far wider than any
//! machine integer, up to the format's 32768 bits.
//!
//! Not ported:
//! - `test_helpers_round_trip_past_the_limit`: tests Python's private
//!   `_bigint` helpers, which exist only to sidestep CPython's
//!   `int_max_str_digits`; Rust has no such limit and no such helpers.
//! - `test_loads_dumps_round_trip`, `test_array_round_trip`,
//!   `test_dom_to_python`, `test_write_array_under_a_wide_annotation`,
//!   `test_write_uint_refusal_message_does_not_raise_valueerror`,
//!   `test_quantity_from_number`: their values are Python ints of thousands of
//!   digits; a Rust `Value::Int` is an `i128`, so no such value can be built.
//! - `test_numpy_object_array`: NumPy is not mirrored (README).

use bovnar::{
    BaseUnit, Event, EventData, Exponent, ReadOptions, Reader, SIPrefix, Unit, Value, parse_typed,
};

/// A wide literal written by hand keeps every digit through the typed parse.
#[test]
fn test_loads_a_hand_written_wide_literal() {
    let digits = "9".repeat(5000);
    let doc = format!(".a = <uint:32768> {digits};");
    let Value::Struct(fields) = parse_typed(doc.as_bytes()).unwrap() else {
        panic!("not a struct");
    };
    let Value::Quantity(q) = &fields[0].1 else {
        panic!("not a quantity: {:?}", fields[0].1);
    };
    assert_eq!(q.raw.as_deref(), Some(digits.as_str()));
}

/// km -> m multiplies 5000 nines by 1000: the exact rational and the
/// converted literal carry every digit.
#[test]
fn test_converted_rational_of_a_wide_result() {
    let nines = "9".repeat(5000);
    let doc = format!(".d = <uint:32768,k~m> {nines};");
    let want_digits = format!("{nines}000");
    let mut seen = Vec::new();
    let mut want = |_: &EventData<'_>| {
        Some((
            Unit::si(BaseUnit::METER, SIPrefix::NONE, Exponent::LINEAR),
            10,
        ))
    };
    let mut on_event = |d: &EventData<'_>| {
        if d.event == Event::Data && d.converted {
            seen.push((
                d.converted_rational(),
                String::from_utf8(d.converted_text.to_vec()).unwrap(),
            ));
        }
        true
    };
    Reader::new()
        .unwrap()
        .read_mem_want_unit(
            doc.as_bytes(),
            &ReadOptions {
                max_conversion_length: 20000,
                ..ReadOptions::default()
            },
            &mut want,
            Some(&mut on_event),
        )
        .unwrap();
    let (rational, text) = &seen[0];
    assert_eq!(rational, &Some((want_digits.clone(), "1".into())));
    assert_eq!(text, &want_digits);
}
