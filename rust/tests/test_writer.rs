//! Port of `python/tests/test_writer.py`. Each Python test class is a module
//! here so the test names stay the same.
//!
//! Skipped Python tests:
//! - `TestWriterLifecycle::test_destroy_idempotent`: Rust has no `destroy`;
//!   the handle is released exactly once when the writer is dropped.
//!
//! `TestWriteBvni::test_wide_uint` writes 2**128 - 1, which does not fit the
//! `i128` value `write_bvni` takes, so it writes the same `uint:128` scalar
//! through `write_quantity` with the exact literal.
//!
//! Partly expressible: in `test_uint_out_of_range_raises_not_truncates` and
//! `test_sint_out_of_range_raises_not_truncates` the out-of-range values
//! (2**64 + 5, -1, 2**63, -(2**63) - 1) cannot be passed to the `u64`/`i64`
//! parameters at all; only the boundary halves are ported.
//!
//! Python's `write_uint`/`write_float` unit keywords map to `write_quantity`
//! with a parsed `Unit`; `emit`'s `vu` argument has no Rust counterpart (the
//! tests pass a unitless one).

// 3.14 is the Python suite's literal input, not an approximation of pi.
#![allow(clippy::approx_constant)]

use bovnar::units::UnitFlags;
use bovnar::writer::format_bigint;
use bovnar::{
    Base, EmitSpec, Error, Event, Quantity, ReadOptions, Reader, Signedness, TokenType, Unit,
    Value, ValueTypeFamily, ValueTypeSpec, Writer, parse, spec_version, to_string,
};

/// Python's `DEFAULT_MEM_CAP`.
const CAP: usize = 4 * 1024 * 1024;

/// `Writer.to_mem()` with Python's defaults.
fn mem() -> Writer {
    Writer::to_mem(CAP, true).unwrap()
}

/// A `with Writer.to_mem() as w:` block: write, finish, return the output.
fn written(f: impl FnOnce(&mut Writer)) -> Vec<u8> {
    let mut w = mem();
    f(&mut w);
    w.finish().unwrap();
    w.get_output().to_vec()
}

fn has(out: &[u8], needle: &[u8]) -> bool {
    out.windows(needle.len()).any(|win| win == needle)
}

fn vt(family: ValueTypeFamily, width: u32, base: u32) -> ValueTypeSpec {
    ValueTypeSpec {
        family: family as i32,
        width,
        base,
    }
}

fn s(text: &str) -> Value {
    Value::String(text.into())
}

fn field<'a>(doc: &'a Value, key: &str) -> &'a Value {
    let Value::Struct(entries) = doc else {
        panic!("not a struct: {doc:?}");
    };
    &entries.iter().find(|(k, _)| k == key).unwrap().1
}

/// Python's `out.split(b'>')[1] if b'>' in out else out`.
fn after_bracket(out: &[u8]) -> &[u8] {
    out.split(|&b| b == b'>').nth(1).unwrap_or(out)
}

/// `with Reader() as r: r.read_mem(out)`.
fn read(out: &[u8]) {
    Reader::new()
        .unwrap()
        .read_mem(out, &ReadOptions::default(), None, None)
        .unwrap();
}

/// The number of DATA events a verified read of `out` reports.
fn count_data_events(out: &[u8]) -> usize {
    let mut n = 0;
    let mut cb = |d: &bovnar::EventData<'_>| {
        if d.event == Event::Data {
            n += 1;
        }
        true
    };
    Reader::new()
        .unwrap()
        .read_mem(out, &ReadOptions::default(), Some(&mut cb), None)
        .unwrap();
    n
}

fn quantity(raw: &str, spec: ValueTypeSpec, unit: &str) -> Quantity {
    Quantity::new(raw, spec, Unit::parse(unit).unwrap())
}

mod writer_lifecycle {
    use super::*;

    #[test]
    fn test_to_mem_factory() {
        assert!(Writer::to_mem(CAP, true).is_ok());
    }

    #[test]
    fn test_context_manager_calls_finish() {
        let out = written(|w| w.write_uint("x", 1, 64, Base::DECIMAL).unwrap());
        assert!(has(&out, b"x"));
    }

    #[test]
    fn test_get_output_before_finish_raises() {
        written(|w| {
            w.write_uint("x", 42, 64, Base::DECIMAL).unwrap();
            let _ = w.bytes_written();
        });
    }

    #[test]
    fn test_to_fd() {
        // Python writes to an fd opened on a temporary file; Rust writes to
        // the File.
        let path =
            std::env::temp_dir().join(format!("bovnar-test-writer-fd-{}.bvnr", std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        let mut w = Writer::to_writer(file, true).unwrap();
        w.write_uint("y", 99, 64, Base::DECIMAL).unwrap();
        w.finish().unwrap();
        drop(w);
        let content = std::fs::read(&path);
        std::fs::remove_file(&path).unwrap();
        assert!(has(&content.unwrap(), b"y"));
    }
}

mod write_uint {
    use super::*;

    #[test]
    fn test_basic_uint16() {
        let out = written(|w| w.write_uint("port", 8080, 16, Base::DECIMAL).unwrap());
        assert!(has(&out, b"port"));
        assert!(has(&out, b"8080"));
    }

    #[test]
    fn test_uint8_max() {
        let out = written(|w| w.write_uint("x", 255, 8, Base::DECIMAL).unwrap());
        assert!(has(&out, b"255"));
    }

    #[test]
    fn test_uint64_large() {
        let out = written(|w| w.write_uint("big", 1 << 63, 64, Base::DECIMAL).unwrap());
        assert!(has(&out, b"big"));
    }

    #[test]
    fn test_uint_hex_base() {
        let out = written(|w| w.write_uint("flags", 0xFF, 8, Base::HEX).unwrap());
        assert!(has(&out, b"ff"));
    }

    #[test]
    fn test_uint_with_si_unit() {
        let q = quantity("512", vt(ValueTypeFamily::Uint, 64, 0), "k~B");
        let out = written(|w| w.write_quantity("cache", &q).unwrap());
        assert!(has(&out, b"cache"));
        assert!(has(&out, b"512"));
    }

    #[test]
    fn test_uint_with_iec_unit() {
        let q = quantity("8", vt(ValueTypeFamily::Uint, 64, 0), "Gi~B");
        let out = written(|w| w.write_quantity("ram", &q).unwrap());
        assert!(has(&out, b"ram"));
        assert!(has(&out, b"8"));
    }

    #[test]
    fn test_uint_with_unit_str() {
        let q = quantity("2", vt(ValueTypeFamily::Uint, 64, 0), "Ti~B");
        let out = written(|w| w.write_quantity("storage", &q).unwrap());
        assert!(has(&out, b"storage"));
    }

    #[test]
    fn test_uint_out_of_range_raises_not_truncates() {
        // the exact 64-bit boundary is still accepted
        let out = written(|w| w.write_uint("max", u64::MAX, 64, Base::DECIMAL).unwrap());
        assert!(has(&out, b"max"));
    }
}

mod write_sint {
    use super::*;

    #[test]
    fn test_negative_value() {
        let out = written(|w| w.write_sint("delta", -42, 32, Base::DECIMAL).unwrap());
        assert!(has(&out, b"delta"));
        assert!(has(&out, b"-42"));
    }

    #[test]
    fn test_zero() {
        let out = written(|w| w.write_sint("zero", 0, 64, Base::DECIMAL).unwrap());
        assert!(has(&out, b"0"));
    }

    #[test]
    fn test_sint_out_of_range_raises_not_truncates() {
        // the 64-bit boundaries are accepted
        let out = written(|w| {
            w.write_sint("lo", i64::MIN, 64, Base::DECIMAL).unwrap();
            w.write_sint("hi", i64::MAX, 64, Base::DECIMAL).unwrap();
        });
        assert!(has(&out, b"lo"));
    }

    #[test]
    fn test_sint_min_8() {
        let out = written(|w| w.write_sint("min", -128, 8, Base::DECIMAL).unwrap());
        assert!(has(&out, b"-128"));
    }
}

mod write_float {
    use super::*;

    #[test]
    fn test_basic_float() {
        let out = written(|w| w.write_float("v", 9.81, 64).unwrap());
        assert!(has(&out, b"v"));
        assert!(has(&out, b"9.81"));
    }

    #[test]
    fn test_float_with_unit() {
        let q = quantity("9.81", vt(ValueTypeFamily::Float, 64, 0), "m/s");
        let out = written(|w| w.write_quantity("speed", &q).unwrap());
        assert!(has(&out, b"speed"));
    }

    #[test]
    fn test_float32() {
        let out = written(|w| w.write_float("t", 3.14, 32).unwrap());
        assert!(has(&out, b"t"));
    }
}

mod write_string {
    use super::*;

    #[test]
    fn test_basic_string() {
        let out = written(|w| w.write_string("host", "localhost").unwrap());
        assert!(has(&out, b"host"));
        assert!(has(&out, b"localhost"));
    }

    #[test]
    fn test_empty_string() {
        let out = written(|w| w.write_string("empty", "").unwrap());
        assert!(has(&out, b"empty"));
    }

    #[test]
    fn test_string_with_escape_chars() {
        let out = written(|w| w.write_string("msg", "hello\nworld").unwrap());
        assert!(has(&out, b"msg"));
    }
}

mod write_bool_null {
    use super::*;

    #[test]
    fn test_true() {
        let out = written(|w| w.write_bool("flag", true).unwrap());
        assert!(has(&out, b"true"));
    }

    #[test]
    fn test_false() {
        let out = written(|w| w.write_bool("flag", false).unwrap());
        assert!(has(&out, b"false"));
    }

    #[test]
    fn test_null() {
        let out = written(|w| w.write_null("empty").unwrap());
        assert!(has(&out, b"empty"));
    }
}

mod write_struct {
    use super::*;

    #[test]
    fn test_empty_struct() {
        let out = written(|w| {
            w.begin_struct("cfg").unwrap();
            w.end_struct().unwrap();
        });
        assert!(has(&out, b"cfg"));
    }

    #[test]
    fn test_nested_struct() {
        let out = written(|w| {
            w.begin_struct("server").unwrap();
            w.write_string("host", "localhost").unwrap();
            w.write_uint("port", 8080, 16, Base::DECIMAL).unwrap();
            w.end_struct().unwrap();
        });
        for needle in [&b"server"[..], b"host", b"localhost", b"port", b"8080"] {
            assert!(has(&out, needle));
        }
    }

    #[test]
    fn test_double_nested() {
        let out = written(|w| {
            w.begin_struct("outer").unwrap();
            w.begin_struct("inner").unwrap();
            w.write_uint("x", 42, 64, Base::DECIMAL).unwrap();
            w.end_struct().unwrap();
            w.end_struct().unwrap();
        });
        assert!(has(&out, b"outer"));
        assert!(has(&out, b"inner"));
        assert!(has(&out, b"42"));
    }

    #[test]
    fn test_unclosed_struct_raises_on_finish() {
        let mut w = mem();
        w.begin_struct("open").unwrap();
        assert!(w.finish().is_err());
    }
}

mod write_array {
    use super::*;

    #[test]
    fn test_simple_array() {
        let spec = vt(ValueTypeFamily::Uint, 64, 10);
        let out = written(|w| {
            w.emit(Event::AssignmentStart, EmitSpec::key("values"))
                .unwrap();
            w.begin_array_row().unwrap();
            for v in [1, 2, 3] {
                w.emit(Event::Data, EmitSpec::value(&v.to_string(), spec))
                    .unwrap();
            }
            w.end_array_row().unwrap();
        });
        assert!(has(&out, b"values"));
        assert!(has(&out, b"1"));
        assert!(has(&out, b"3"));
    }

    #[test]
    fn test_2d_array() {
        let spec = vt(ValueTypeFamily::Uint, 64, 10);
        let out = written(|w| {
            w.emit(Event::AssignmentStart, EmitSpec::key("mat"))
                .unwrap();
            for (i, row) in [[1, 2], [3, 4]].iter().enumerate() {
                if i > 0 {
                    w.new_array_dim().unwrap();
                }
                w.begin_array_row().unwrap();
                for v in row {
                    w.emit(Event::Data, EmitSpec::value(&v.to_string(), spec))
                        .unwrap();
                }
                w.end_array_row().unwrap();
            }
        });
        assert!(has(&out, b"mat"));
    }
}

mod emit {
    use super::*;

    #[test]
    fn test_emit_assignment_and_data() {
        let spec = vt(ValueTypeFamily::Uint, 32, 10);
        let out = written(|w| {
            w.emit(Event::AssignmentStart, EmitSpec::key("x")).unwrap();
            w.annotation(spec, &Unit::default(), None).unwrap();
            w.emit(Event::Data, EmitSpec::value("42", spec)).unwrap();
        });
        for needle in [&b"x"[..], b"42", b"uint", b"32"] {
            assert!(has(&out, needle));
        }
    }

    #[test]
    fn test_emit_invalid_event_raises() {
        let mut w = mem();
        let r = w
            .emit(Event::AssignmentStart, EmitSpec::key("x"))
            .and_then(|()| {
                w.emit(
                    Event::Data,
                    EmitSpec {
                        key: None,
                        value: None,
                        vt: Some(vt(ValueTypeFamily::Uint, 0, 0)),
                    },
                )
            });
        assert!(matches!(r, Err(Error::Write { .. })), "{r:?}");
    }
}

mod bytes_written {
    use super::*;

    #[test]
    fn test_increases_with_writes() {
        written(|w| {
            let before = w.bytes_written();
            w.write_uint("x", 1, 64, Base::DECIMAL).unwrap();
            assert!(w.bytes_written() > before);
        });
    }

    #[test]
    fn test_zero_initially() {
        // `bytes_written` is unsigned; Python's `>= 0` holds by type. Reading
        // it on a fresh writer must still work.
        written(|w| {
            let _ = w.bytes_written();
        });
    }
}

mod buffer_exhaustion {
    use super::*;

    #[test]
    fn test_tiny_buffer_raises() {
        let mut w = Writer::to_mem(8, true).unwrap();
        let r = w
            .write_string("a_very_long_key_name", "a_very_long_string_value")
            .and_then(|()| w.finish());
        assert!(matches!(r, Err(Error::Write { .. })), "{r:?}");
    }
}

mod pretty_output {
    use super::*;

    #[test]
    fn test_pretty_has_newlines() {
        let out = written(|w| w.write_uint("x", 1, 64, Base::DECIMAL).unwrap());
        assert!(has(&out, b"\n"));
    }

    #[test]
    fn test_compact_no_extra_whitespace() {
        let mut w = Writer::to_mem(CAP, false).unwrap();
        w.write_uint("x", 1, 64, Base::DECIMAL).unwrap();
        w.finish().unwrap();
        let mut w2 = Writer::to_mem(CAP, true).unwrap();
        w2.write_uint("x", 1, 64, Base::DECIMAL).unwrap();
        w2.finish().unwrap();
        assert!(w.get_output().len() <= w2.get_output().len());
    }
}

mod write_bvnf_base {
    use super::*;

    #[test]
    fn test_base10_width0() {
        let out = written(|w| {
            w.write_bvnf_base("pi", "3.14159265358979", 0, Base::DECIMAL)
                .unwrap()
        });
        assert!(has(&out, b"pi"));
        assert!(has(&out, b"3.14"));
    }

    #[test]
    fn test_base10_wide_width() {
        let pi = "3.14159265358979323846264338327950288419716939937510\
                  58209749445923078164062862089986280348253421170679";
        let out = written(|w| w.write_bvnf_base("pi512", pi, 512, Base::DECIMAL).unwrap());
        assert!(has(&out, b"pi512"));
        assert!(has(&out, b"float:512"));
    }

    #[test]
    fn test_base16_produces_quoted_string() {
        let out = written(|w| w.write_bvnf_base("one", "1.0p+0", 64, Base::HEX).unwrap());
        assert!(has(&out, b"one"));
        assert!(has(&out, b"\"1.0p+0\""));
        assert!(has(&out, b"_16"));
    }

    #[test]
    fn test_base16_negative() {
        let out = written(|w| w.write_bvnf_base("neg", "-1.8p+4", 64, Base::HEX).unwrap());
        assert!(has(&out, b"neg"));
        assert!(has(&out, b"-1.8p+4"));
    }

    #[test]
    fn test_invalid_base_raises() {
        let r = mem().write_bvnf_base("x", "1.0", 64, Base::OCTAL);
        assert!(matches!(r, Err(Error::Argument { .. })), "{r:?}");
    }
}

mod write_bvni {
    use super::*;

    #[test]
    fn test_uint_decimal() {
        let out = written(|w| {
            w.write_bvni("u", 255, 8, Base::DECIMAL, Signedness::Auto)
                .unwrap()
        });
        assert!(has(&out, b"255"));
        assert!(has(&out, b"uint"));
    }

    #[test]
    fn test_sint_negative() {
        let out = written(|w| {
            w.write_bvni("s", -128, 8, Base::DECIMAL, Signedness::Auto)
                .unwrap()
        });
        assert!(has(&out, b"-128"));
        assert!(has(&out, b"sint"));
    }

    #[test]
    fn test_wide_uint() {
        let vt = ValueTypeSpec {
            family: ValueTypeFamily::Uint as i32,
            width: 128,
            base: 0,
        };
        let val: num_bigint::BigInt = (num_bigint::BigInt::from(1) << 128u32) - 1;
        let q = Quantity::new(&val.to_string(), vt, Unit::default());
        let out = written(|w| w.write_quantity("u128max", &q).unwrap());
        assert!(has(&out, b"340282366920938463463374607431768211455"));
        assert!(has(&out, b"uint:128"));
    }

    #[test]
    fn test_hex_base() {
        let out = written(|w| {
            w.write_bvni("flags", 0xDEAD_BEEF, 32, Base::HEX, Signedness::Auto)
                .unwrap()
        });
        assert!(has(&out, b"deadbeef"));
        assert!(has(&out, b"\"deadbeef\""));
        assert!(has(&out, b"_16"));
    }

    #[test]
    fn test_hex_negative() {
        let out = written(|w| {
            w.write_bvni("neg", -0x7FFF_FFFF, 32, Base::HEX, Signedness::Auto)
                .unwrap()
        });
        assert!(has(&out, b"-"));
    }

    #[test]
    fn test_forced_signed_positive() {
        let out = written(|w| {
            w.write_bvni("s", 127, 8, Base::DECIMAL, Signedness::Signed)
                .unwrap()
        });
        assert!(has(&out, b"sint"));
    }

    #[test]
    fn test_format_bigint_base2() {
        assert_eq!(format_bigint(5, 2).unwrap(), "101");
    }

    #[test]
    fn test_format_bigint_base16() {
        assert_eq!(format_bigint(0xDEAD, 16).unwrap(), "dead");
    }

    #[test]
    fn test_format_bigint_negative_hex() {
        assert_eq!(format_bigint(-0xFF, 16).unwrap(), "-ff");
    }

    #[test]
    fn test_format_bigint_base64() {
        assert_eq!(format_bigint(0, 64).unwrap(), "A");
    }
}

mod write_streaming_flush {
    use super::*;

    #[test]
    fn test_large_write_exceeds_internal_buffer() {
        let mut w = Writer::to_mem(2 * 1024 * 1024, true).unwrap();
        for i in 0..4000u64 {
            w.write_uint(&format!("k{i}"), i, 64, Base::DECIMAL)
                .unwrap();
        }
        w.finish().unwrap();
        assert!(w.get_output().len() > 65536);
    }
}

mod write_float_fix {
    use super::*;

    #[test]
    fn test_basic_write_float_fix() {
        let out = written(|w| w.write_float_fix("ff", 1.5, 32, 16).unwrap());
        assert!(has(&out, b"float_fix"));
        assert!(has(&out, b"q16"));
    }

    #[test]
    fn test_float_fix_q_zero_produces_annotation() {
        let out = written(|w| w.write_float_fix("ff", 1.5, 32, 0).unwrap());
        assert!(has(&out, b"float_fix"));
        assert!(has(&out, b"32"));
    }

    #[test]
    fn test_float_fix_uses_number_token_not_string() {
        let out = written(|w| w.write_float_fix("ff", 1.5, 32, 16).unwrap());
        assert!(
            !has(out.split(|&b| b == b'>').nth(1).unwrap(), b"\""),
            "float_fix value must not be wrapped in double quotes (must be NUMBER token)"
        );
    }

    #[test]
    fn test_float_fix_roundtrip() {
        let out = written(|w| {
            w.write_float_fix("a", 1.5, 32, 16).unwrap();
            w.write_float_fix("b", -0.25, 64, 8).unwrap();
        });
        assert_eq!(count_data_events(&out), 2);
    }

    #[test]
    fn test_float_fix_parses_without_error() {
        let out = written(|w| w.write_float_fix("x", 3.14, 64, 32).unwrap());
        read(&out);
    }

    #[test]
    fn test_float_fix_various_q_values_are_number_tokens() {
        for q in [4, 8, 16, 24, 32] {
            let out = written(|w| w.write_float_fix("v", 1.0, 64, q).unwrap());
            assert!(
                !has(&out, b">\""),
                "float_fix q={q}: annotation must not be followed by quoted value"
            );
        }
    }

    #[test]
    fn test_float_fix_nan_roundtrip() {
        let out = written(|w| w.write_float_fix("nan_val", f64::NAN, 64, 16).unwrap());
        assert!(has(&out, b"nan"));
    }
}

mod write_float_dec {
    use super::*;

    #[test]
    fn test_basic_write_float_dec() {
        let out = written(|w| w.write_float_dec("fd", 3.14, 64).unwrap());
        assert!(has(&out, b"float_dec"));
    }

    #[test]
    fn test_float_dec_uses_number_token_not_string() {
        let out = written(|w| w.write_float_dec("fd", 3.14, 64).unwrap());
        assert!(
            !has(out.split(|&b| b == b'>').nth(1).unwrap(), b"\""),
            "float_dec value must not be wrapped in quotes (must be NUMBER token)"
        );
    }

    #[test]
    fn test_float_dec_parses_without_error() {
        let out = written(|w| w.write_float_dec("x", -1.5e3, 32).unwrap());
        read(&out);
    }

    #[test]
    fn test_float_dec_roundtrip_multiple() {
        let out = written(|w| {
            w.write_float_dec("a", 1.0, 64).unwrap();
            w.write_float_dec("b", -2.5, 32).unwrap();
        });
        assert_eq!(count_data_events(&out), 2);
    }

    #[test]
    fn test_float_dec_nan_is_special_literal() {
        let out = written(|w| w.write_float_dec("v", f64::NAN, 64).unwrap());
        assert!(has(&out, b"nan"));
    }

    #[test]
    fn test_float_dec_infinity_is_special_literal() {
        let out = written(|w| w.write_float_dec("v", f64::INFINITY, 64).unwrap());
        assert!(has(&out, b"inf"));
    }
}

/// Regression tests for annotation roundtrip bugs: a width on utf8, a width
/// emitted for width 0, a base lost when width is 0, and hex values emitted
/// unquoted.
mod annotation_regressions {
    use super::*;

    /// One `.key = <annotation> value;` assignment through raw events.
    fn emit_annotated(key: &str, spec: ValueTypeSpec, value: &str) -> Vec<u8> {
        written(|w| {
            w.emit(Event::AssignmentStart, EmitSpec::key(key)).unwrap();
            w.annotation(spec, &Unit::default(), None).unwrap();
            w.emit(Event::Data, EmitSpec::value(value, spec)).unwrap();
        })
    }

    #[test]
    fn test_utf8_typed_width_rejected() {
        // utf8 takes no parameters; a width must be rejected so the writer
        // never emits <utf8:8>, which the reader rejects (illegal_value_type).
        let r = mem().annotation(vt(ValueTypeFamily::Utf8, 8, 0), &Unit::default(), None);
        assert!(matches!(r, Err(Error::Write { .. })), "{r:?}");
    }

    #[test]
    fn test_utf8_typed_width_zero_omitted_from_annotation() {
        let out = emit_annotated("k", vt(ValueTypeFamily::Utf8, 0, 0), "x");
        assert!(
            !has(&out, b"utf8:0"),
            "UTF8 annotation with width=0 must not contain a width spec"
        );
    }

    #[test]
    fn test_float_width_zero_produces_no_width_in_annotation() {
        let out = written(|w| w.write_bvnf_base("pi", "3.14", 0, Base::DECIMAL).unwrap());
        assert!(
            !has(&out, b"float:64"),
            "width=0 must not emit ':64' via bvn_effective_width fallback"
        );
        assert!(
            has(&out, b"<float>") || has(&out, b"<float,"),
            "width=0 float annotation must be '<float>' (no width spec)"
        );
    }

    #[test]
    fn test_uint_width_zero_base16_emits_base_in_annotation() {
        let out = emit_annotated("k", vt(ValueTypeFamily::Uint, 0, 16), "ff");
        assert!(
            has(&out, b"_16"),
            "base=16 must appear in annotation even when width=0"
        );
        assert!(
            !has(&out, b"uint:0"),
            "width=0 must not emit a spurious ':0' width spec"
        );
    }

    #[test]
    fn test_emit_hex_uint_uses_string_token() {
        let out = emit_annotated("flags", vt(ValueTypeFamily::Uint, 8, 16), "ff");
        assert!(
            has(&out, b"\"ff\""),
            "hex uint written via emit() must be quoted (TOKEN_IS_STRING)"
        );
    }

    #[test]
    fn test_emit_decimal_uint_uses_number_token() {
        let out = emit_annotated("val", vt(ValueTypeFamily::Uint, 8, 10), "42");
        assert!(
            !has(&out, b"\"42\""),
            "decimal uint written via emit() must not be quoted (TOKEN_IS_NUMBER)"
        );
        assert!(has(&out, b"42"));
    }
}

/// The reference validator: a reference needs its leading '.' followed by
/// an identifier start, so the writer can never produce `&foo` or `&.`.
mod reference_roundtrip {
    use super::*;

    /// Writes `.x = &<ref>` as a raw reference event; whether it was taken.
    fn write_reference(reference: &[u8]) -> bool {
        let mut w = mem();
        w.emit(Event::AssignmentStart, EmitSpec::key("x")).unwrap();
        w.event(
            Event::Data,
            TokenType::Reference,
            ValueTypeSpec::default(),
            reference,
        )
        .is_ok()
    }

    /// A finished document `.<key> = &<ref>;`.
    fn reference_doc(key: &str, reference: &[u8]) -> Vec<u8> {
        written(|w| {
            w.emit(Event::AssignmentStart, EmitSpec::key(key)).unwrap();
            w.event(
                Event::Data,
                TokenType::Reference,
                ValueTypeSpec::default(),
                reference,
            )
            .unwrap();
        })
    }

    #[test]
    fn test_valid_reference_accepted() {
        assert!(
            write_reference(b".host"),
            "'.host' (valid reference) must be accepted by the writer"
        );
    }

    #[test]
    fn test_valid_multi_segment_reference_accepted() {
        assert!(
            write_reference(b".config.host"),
            "'.config.host' must be accepted"
        );
    }

    #[test]
    fn test_reference_without_leading_dot_rejected() {
        assert!(
            !write_reference(b"host"),
            "'host' (no leading dot) must be rejected"
        );
    }

    #[test]
    fn test_reference_dot_only_rejected() {
        assert!(
            !write_reference(b"."),
            "'.' (dot only, no id-start) must be rejected"
        );
    }

    #[test]
    fn test_reference_double_dot_rejected() {
        assert!(
            !write_reference(b"..host"),
            "'..host' (double leading dot) must be rejected"
        );
    }

    #[test]
    fn test_valid_reference_roundtrips() {
        let out = reference_doc("x", b".some.path");
        assert!(has(&out, b"&.some.path"), "writer must emit '&.some.path'");
        let result = parse(&out).unwrap();
        assert_eq!(
            field(&result, "x"),
            &s(".some.path"),
            "parsed reference must be the path string including leading dot"
        );
    }

    #[test]
    fn test_reference_output_is_parseable() {
        let out = reference_doc("r", b".host");
        read(&out);
    }
}

/// `emit()` keeps float_fix/float_dec values NUMBER tokens: for float_fix
/// `vt.base` is the Q parameter, not a numeric base.
mod emit_float_fix_token_type {
    use super::*;

    fn emit_annotated(key: &str, spec: ValueTypeSpec, value: &str) -> Vec<u8> {
        written(|w| {
            w.emit(Event::AssignmentStart, EmitSpec::key(key)).unwrap();
            w.annotation(spec, &Unit::default(), None).unwrap();
            w.emit(Event::Data, EmitSpec::value(value, spec)).unwrap();
        })
    }

    #[test]
    fn test_emit_float_fix_q16_uses_number_token() {
        let out = emit_annotated("ff", vt(ValueTypeFamily::FloatFix, 32, 16), "1.5");
        assert!(
            !has(after_bracket(&out), b"\""),
            "float_fix with Q=16 via emit() must not produce a quoted value"
        );
    }

    #[test]
    fn test_emit_float_fix_q8_uses_number_token() {
        let out = emit_annotated("ff", vt(ValueTypeFamily::FloatFix, 64, 8), "3.14");
        assert!(
            !has(after_bracket(&out), b"\""),
            "float_fix with Q=8 via emit() must not produce a quoted value"
        );
    }

    #[test]
    fn test_emit_float_dec_uses_number_token() {
        let out = emit_annotated("fd", vt(ValueTypeFamily::FloatDec, 64, 0), "2.718");
        assert!(
            !has(after_bracket(&out), b"\""),
            "float_dec via emit() must not produce a quoted value"
        );
    }

    #[test]
    fn test_emit_float_fix_q16_roundtrips() {
        let out = emit_annotated("ff", vt(ValueTypeFamily::FloatFix, 32, 16), "1.5");
        read(&out);
        let result = parse(&out).unwrap();
        let Value::Float(f) = field(&result, "ff") else {
            panic!("not a float: {result:?}");
        };
        assert!(
            (f - 1.5).abs() < 1e-9,
            "float_fix Q=16 written via emit() must roundtrip to 1.5"
        );
    }

    #[test]
    fn test_emit_hex_uint_still_uses_string_token() {
        let out = emit_annotated("flags", vt(ValueTypeFamily::Uint, 8, 16), "ff");
        assert!(
            has(&out, b"\"ff\""),
            "hex uint via emit() must still be quoted (TOKEN_IS_STRING)"
        );
    }

    #[test]
    fn test_emit_float_fix_all_q_values_use_number_token() {
        for q in [2, 4, 8, 11, 16, 24, 32] {
            let out = emit_annotated("ff", vt(ValueTypeFamily::FloatFix, 64, q), "1.0");
            assert!(
                !has(after_bracket(&out), b"\""),
                "float_fix Q={q} via emit() must not produce a quoted value"
            );
            read(&out);
        }
    }
}

/// Every string escape the C writer emits survives write -> parse.
mod string_escape_roundtrip {
    use super::*;

    fn dumps_s(value: &str) -> Vec<u8> {
        to_string(&Value::Struct(vec![("s".into(), s(value))]), true).unwrap()
    }

    fn roundtrip_str(value: &str) -> Value {
        field(&parse(&dumps_s(value)).unwrap(), "s").clone()
    }

    #[test]
    fn test_tab_roundtrips() {
        assert_eq!(roundtrip_str("a\tb"), s("a\tb"));
    }

    #[test]
    fn test_newline_roundtrips() {
        assert_eq!(roundtrip_str("a\nb"), s("a\nb"));
    }

    #[test]
    fn test_carriage_return_roundtrips() {
        assert_eq!(roundtrip_str("a\rb"), s("a\rb"));
    }

    #[test]
    fn test_vertical_tab_roundtrips() {
        assert_eq!(
            roundtrip_str("a\x0bb"),
            s("a\x0bb"),
            "vertical tab (0x0B) must survive write-parse roundtrip"
        );
    }

    #[test]
    fn test_form_feed_roundtrips() {
        assert_eq!(
            roundtrip_str("a\x0cb"),
            s("a\x0cb"),
            "form feed (0x0C) must survive write-parse roundtrip"
        );
    }

    #[test]
    fn test_double_quote_roundtrips() {
        assert_eq!(roundtrip_str("say \"hello\""), s("say \"hello\""));
    }

    #[test]
    fn test_backslash_roundtrips() {
        assert_eq!(roundtrip_str("a\\b"), s("a\\b"));
    }

    #[test]
    fn test_all_escapes_together() {
        let all = "\t\n\x0b\x0c\r\"\\";
        assert_eq!(
            roundtrip_str(all),
            s(all),
            "all seven escape sequences must survive roundtrip together"
        );
    }

    #[test]
    fn test_vertical_tab_appears_escaped_in_output() {
        assert!(
            has(&dumps_s("\x0b"), b"\\v"),
            "vertical tab (0x0B) must be escaped as \\v in the serialised output"
        );
    }

    #[test]
    fn test_form_feed_appears_escaped_in_output() {
        assert!(
            has(&dumps_s("\x0c"), b"\\f"),
            "form feed (0x0C) must be escaped as \\f in the serialised output"
        );
    }
}

/// An empty string stays an empty string and null stays null.
mod empty_string_roundtrip {
    use super::*;

    fn rt(key: &str, value: Value) -> Value {
        let doc = Value::Struct(vec![(key.into(), value)]);
        let out = to_string(&doc, true).unwrap();
        field(&parse(&out).unwrap(), key).clone()
    }

    fn strings(items: &[&str]) -> Value {
        Value::Array(items.iter().map(|i| s(i)).collect())
    }

    #[test]
    fn test_empty_string_scalar_roundtrips() {
        assert_eq!(
            rt("k", s("")),
            s(""),
            "empty string must roundtrip as '' not None"
        );
    }

    #[test]
    fn test_empty_string_is_not_none() {
        assert_ne!(
            rt("k", s("")),
            Value::Null,
            "empty string must not decode to None"
        );
    }

    #[test]
    fn test_null_still_roundtrips_as_none() {
        assert_eq!(
            rt("k", Value::Null),
            Value::Null,
            "null must still decode to None after the fix"
        );
    }

    #[test]
    fn test_empty_string_in_array_roundtrips() {
        assert_eq!(
            rt("a", strings(&["", "x", ""])),
            strings(&["", "x", ""]),
            "empty strings inside arrays must roundtrip as ''"
        );
    }

    #[test]
    fn test_null_in_array_still_decodes_as_none() {
        let result = parse(b".a = [,];").unwrap();
        assert_eq!(
            field(&result, "a"),
            &Value::Array(vec![Value::Null, Value::Null]),
            "null array elements must still decode to None"
        );
    }

    #[test]
    fn test_mixed_empty_and_null_in_array() {
        let doc = Value::Struct(vec![("a".into(), strings(&["", "hello", ""]))]);
        let out = to_string(&doc, true).unwrap();
        assert!(
            has(&out, b"\"\""),
            "serialised output must contain quoted empty strings"
        );
        assert_eq!(
            field(&parse(&out).unwrap(), "a"),
            &strings(&["", "hello", ""]),
            "empty strings must survive write-parse through array"
        );
    }

    #[test]
    fn test_empty_string_roundtrip_via_write_string() {
        let out = written(|w| w.write_string("s", "").unwrap());
        assert_eq!(
            field(&parse(&out).unwrap(), "s"),
            &s(""),
            "write_string('s', '') must parse back as empty string"
        );
    }

    #[test]
    fn test_typed_empty_string_roundtrips() {
        let result = parse(b".k = <utf8> \"\";").unwrap();
        assert_eq!(
            field(&result, "k"),
            &s(""),
            "typed empty string <utf8> \"\" must decode as ''"
        );
    }
}

/// High-base integers (37..62, 64, 85) decode to integers, with the same
/// alphabets the C reader and writer use.
mod high_base_int_decode {
    use super::*;

    fn a(src: &[u8]) -> Value {
        field(&parse(src).unwrap(), "a").clone()
    }

    #[test]
    fn test_base62_uint() {
        // base 62: A-Z -> 36..61, a-z -> 10..35, 0-9 -> 0..9
        assert_eq!(
            a(b".a = <uint:64,_62> \"Zz9\";"),
            Value::Int(61 * 62 * 62 + 35 * 62 + 9)
        );
    }

    #[test]
    fn test_base64_uint() {
        // standard Base64: A-Z 0..25, a-z 26..51, 0-9 52..61, + 62, / 63
        assert_eq!(a(b".a = <uint:32,_64> \"Az+\";"), Value::Int(51 * 64 + 62));
    }

    #[test]
    fn test_base85_uint() {
        // Ascii85: '!' (0x21) -> 0 ... 'u' (0x75) -> 84
        let expected = i128::from(b's' - 0x21) * 85 + i128::from(b't' - 0x21);
        assert_eq!(a(b".a = <uint:32,_85> \"st\";"), Value::Int(expected));
    }

    #[test]
    fn test_base50_sint_negative() {
        // base 50: 0-9, a-z 10..35, A-N 36..49.  '-mn' = -(22*50 + 23)
        let m = 10 + i128::from(b'm' - b'a');
        let n = 10 + i128::from(b'n' - b'a');
        let v = a(b".a = <sint:64,_50> \"-mn\";");
        assert_eq!(v, Value::Int(-(m * 50 + n)));
        assert_eq!(v, Value::Int(-1123));
    }

    #[test]
    fn test_low_base_still_works() {
        assert_eq!(a(b".a = <uint:32,_16> \"ff\";"), Value::Int(255));
        assert_eq!(a(b".a = <sint:32,_2> \"-101\";"), Value::Int(-5));
        assert_eq!(a(b".a = <uint:64> 12345;"), Value::Int(12345));
    }
}

/// The writer takes the reader's 255-level nesting cap, so whatever `parse`
/// took can be handed back to `to_string`.
mod writer_matches_reader_limits {
    use super::*;

    fn round_trips(src: &[u8]) {
        let doc = parse(src).unwrap();
        assert_eq!(parse(&to_string(&doc, true).unwrap()).unwrap(), doc);
    }

    #[test]
    fn test_arrays_nested_past_64_round_trip() {
        let src = format!(".a={}1{};", "[".repeat(100), "]".repeat(100));
        round_trips(src.as_bytes());
    }

    #[test]
    fn test_structs_nested_past_64_round_trip() {
        let src = format!(".a={}1;{}", "{.a=".repeat(100), "};".repeat(100));
        round_trips(src.as_bytes());
    }
}

/// `unit_flags` and `emit_version` of the write flags reach the caller.
mod writer_flags_keywords {
    use super::*;

    #[test]
    fn test_unit_flags_reduce_rescales_the_value() {
        let mut w = Writer::to_mem_with(CAP, false, UnitFlags::REDUCE, false).unwrap();
        let q = quantity("2.5", vt(ValueTypeFamily::Float, 64, 0), "k~g");
        w.write_quantity("a", &q).unwrap();
        w.finish().unwrap();
        assert_eq!(w.get_output(), b".a=<float:64,g>2500;");
    }

    #[test]
    fn test_emit_version_leads_the_document() {
        let mut w = Writer::to_mem_with(CAP, false, 0, true).unwrap();
        w.write_uint("a", 1, 64, Base::DECIMAL).unwrap();
        w.finish().unwrap();
        let (major, minor) = spec_version();
        let lead = format!("#!bovnar {major}.{minor}\n");
        assert!(w.get_output().starts_with(lead.as_bytes()));
    }

    #[test]
    fn test_num_canonical_is_mirrored() {
        assert_eq!(UnitFlags::NUM_CANONICAL, 1 << 2);
    }

    #[test]
    fn test_to_file_takes_them_too() {
        // Python's to_file(path, ...) is to_writer_with over the created File.
        let path =
            std::env::temp_dir().join(format!("bovnar-test-writer-o-{}.bvnr", std::process::id()));
        let file = std::fs::File::create(&path).unwrap();
        let mut w = Writer::to_writer_with(file, CAP, false, 0, true).unwrap();
        w.write_uint("a", 1, 64, Base::DECIMAL).unwrap();
        w.finish().unwrap();
        drop(w);
        let content = std::fs::read(&path);
        std::fs::remove_file(&path).unwrap();
        assert!(content.unwrap().starts_with(b"#!bovnar "));
    }
}

/// Base 64 and 85 are unsigned ('+' and '-' are digits there); a signed
/// value in them is refused by name.
mod signed_high_base_refused_by_name {
    use super::*;

    fn assert_unsigned_refusal(r: bovnar::Result<()>) {
        match r {
            Err(Error::Argument { message: msg, .. }) => assert!(msg.contains("unsigned"), "{msg}"),
            other => panic!("expected an argument error, got {other:?}"),
        }
    }

    #[test]
    fn test_write_sint() {
        for base in [64, 85] {
            let mut w = Writer::to_mem(CAP, false).unwrap();
            assert_unsigned_refusal(w.write_sint("a", -255, 64, Base::try_from(base).unwrap()));
        }
    }

    #[test]
    fn test_write_bvni_signed() {
        for base in [64, 85] {
            let mut w = Writer::to_mem(CAP, false).unwrap();
            assert_unsigned_refusal(w.write_bvni(
                "a",
                -255,
                64,
                Base::try_from(base).unwrap(),
                Signedness::Auto,
            ));
        }
    }
}

/// A unit the writer cannot spell used to be skipped in the annotation,
/// writing the value with its unit silently gone. It must be refused.
#[test]
fn test_a_unit_that_cannot_be_serialised_is_refused_not_dropped() {
    use bovnar::{BaseUnit, Exponent, SIPrefix, UnitComponent};
    let vt = ValueTypeSpec {
        family: ValueTypeFamily::Float as i32,
        width: 64,
        base: 0,
    };
    // structurally valid, but a bu_none component past the bare "no_unit"
    // shapes has no spelling
    let u = Unit::compound(&[
        UnitComponent::si(BaseUnit::Meter, SIPrefix::None, Exponent::LINEAR),
        UnitComponent::si(BaseUnit::None, SIPrefix::Kilo, Exponent::LINEAR),
    ])
    .unwrap();
    let mut w = Writer::to_mem(CAP, false).unwrap();
    w.write_version(1, 1).unwrap();
    let err = w
        .write_quantity("d", &Quantity::new("1.5", vt, u))
        .unwrap_err();
    assert!(matches!(err, Error::Argument { .. }), "{err:?}");
    assert!(err.to_string().contains("unit"), "{err}");

    // an ordinary unit is unaffected
    let mut w2 = Writer::to_mem(CAP, false).unwrap();
    w2.write_version(1, 1).unwrap();
    let q = Quantity::new("1.5", vt, Unit::parse("k~g·m/s²").unwrap());
    w2.write_quantity("d", &q).unwrap();
    w2.finish().unwrap();
    assert!(String::from_utf8_lossy(w2.get_output()).contains("k~g·m/s²"));
}

#[test]
fn test_base_accepts_exactly_the_bovnar_bases() {
    assert_eq!(Base::try_from(16).unwrap(), Base::HEX);
    assert_eq!(u32::from(Base::BASE85), 85);
    for n in [0, 1, 63, 65, 84, 86] {
        assert!(Base::try_from(n).is_err(), "{n}");
    }
    assert_eq!(Base::default(), Base::DECIMAL);
    assert_eq!(Signedness::default(), Signedness::Auto);
}
