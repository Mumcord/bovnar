//! Port of python/tests/test_enums.py: the core enums carry the C values,
//! and the token-type and error-code enums agree with include/bovnar.h.
//!
//! Rust enums cannot be iterated, so "every member" is every raw value
//! `from_raw` accepts, scanned over a range far wider than any enum.
//!
//! Skipped (the unit enums are not mirrored: the Rust binding has no
//! `PrefixSystem`, `SIPrefix`, `IECPrefix`, `BaseUnit` or `Exponent`, nor the
//! `UNIT_NATIVE_*`/`CURRENCY_*` bounds built on them; units are handled as
//! `Unit` values):
//! - TestSIPrefix: test_none_is_zero, test_kilo, test_milli, test_all_unique,
//!   test_count, test_quecto_quetta
//! - TestIECPrefix: test_none_is_zero, test_kibi, test_quebi, test_count,
//!   test_all_unique
//! - TestBaseUnit: test_none_is_zero, test_si_base_units, test_digital_units,
//!   test_named_derived, test_non_si, test_angular_units, test_mass_pressure,
//!   test_atomic_units, test_area_time_units, test_all_unique,
//!   test_native_block_closed_against_gendata, test_native_bounds_match_gendata,
//!   test_currency_block_closed_against_gendata,
//!   test_no_members_outside_the_two_blocks
//! - TestExponent: test_positive_exponents, test_negative_exponents,
//!   test_all_unique

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use bovnar::{ErrorCode, Event, TokenType, ValueTypeFamily};

/// Every member of an enum: each raw value `from_raw` accepts.
fn members<T>(from_raw: fn(i32) -> Option<T>) -> Vec<(i32, T)> {
    (-1024..=1024)
        .filter_map(|raw| from_raw(raw).map(|m| (raw, m)))
        .collect()
}

fn all_unique(values: &[i32]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

/// Removes the `/* ... */` comments that open and close on this line.
fn strip_block_comments(line: &str) -> String {
    let mut out = line.to_string();
    while let Some(start) = out.find("/*") {
        let Some(len) = out[start + 2..].find("*/") else {
            break;
        };
        out.replace_range(start..start + 2 + len + 2, "");
    }
    out
}

fn is_word(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn is_int(s: &str) -> bool {
    let digits = s.strip_prefix('-').unwrap_or(s);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// Parses `typedef enum <name>_e { ... }` out of the C header.
///
/// Reading the header directly is the point: a hand-kept Rust copy of a C
/// enum only stays correct if something compares the two.
fn c_enum(name: &str) -> BTreeMap<String, i64> {
    let header = Path::new(env!("CARGO_MANIFEST_DIR")).join("../include/bovnar.h");
    let text = std::fs::read_to_string(&header).unwrap();
    let open = format!("typedef enum {name}_e {{");
    let start = text
        .find(&open)
        .unwrap_or_else(|| panic!("{name}_e not found"))
        + open.len();
    let body = &text[start..start + text[start..].find('}').unwrap()];
    let mut out = BTreeMap::new();
    let mut next = 0;
    for line in body.lines() {
        let line = strip_block_comments(line);
        let line = line
            .split("//")
            .next()
            .unwrap()
            .trim()
            .trim_end_matches(',');
        if line.is_empty() {
            continue;
        }
        let (ident, value) = match line.split_once('=') {
            Some((l, r)) => (l.trim_end(), Some(r.trim_start())),
            None => (line, None),
        };
        if !is_word(ident) || value.is_some_and(|v| !is_int(v)) {
            continue;
        }
        if let Some(v) = value {
            next = v.parse().unwrap();
        }
        out.insert(ident.to_string(), next);
        next += 1;
    }
    out
}

/// `token_is_array_number` -> `ArrayNumber`.
fn camel(snake: &str) -> String {
    snake
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

mod token_type_mirrors_c {
    use super::*;

    // BvnrData.type reaches users as this raw int, and the writer selects
    // behaviour on it, so a drift here is silent and consequential.
    #[test]
    fn test_every_c_token_type_is_mirrored() {
        let c = c_enum("token_type");
        assert!(!c.is_empty(), "parsed no token_type_e members");
        for (cname, cval) in &c {
            let want = camel(&cname["token_is_".len()..]);
            let got = TokenType::from_raw(*cval as i32);
            assert_eq!(
                got.map(|t| format!("{t:?}")),
                Some(want.clone()),
                "TokenType::{want} should be C {cname} = {cval}"
            );
        }
    }

    #[test]
    fn test_no_extra_python_members() {
        let c: BTreeSet<i64> = c_enum("token_type").into_values().collect();
        let rust: BTreeSet<i64> = members(TokenType::from_raw)
            .into_iter()
            .map(|(raw, _)| raw as i64)
            .collect();
        assert_eq!(rust, c);
    }
}

mod error_code_mirrors_c {
    use super::*;

    #[test]
    fn test_every_c_error_code_is_mirrored() {
        let c = c_enum("error_code");
        assert!(!c.is_empty(), "parsed no error_code_e members");
        for (cname, cval) in &c {
            assert!(
                ErrorCode::from_raw(*cval as i32).is_some(),
                "ErrorCode is missing {cname} = {cval}"
            );
        }
    }
}

mod event {
    use super::*;

    #[test]
    fn test_stream_begin_is_zero() {
        assert_eq!(Event::StreamStart as i32, 0);
    }

    #[test]
    fn test_assignment_start() {
        assert_eq!(Event::AssignmentStart as i32, 1);
    }

    #[test]
    fn test_data() {
        assert_eq!(Event::Data as i32, 9);
    }

    #[test]
    fn test_type_annotation_family_param() {
        assert_eq!(Event::TypeAnnotationTypeFamilyParam as i32, 13);
    }

    #[test]
    fn test_all_events_unique() {
        let values: Vec<i32> = members(Event::from_raw)
            .iter()
            .map(|(_, e)| *e as i32)
            .collect();
        assert!(all_unique(&values));
    }

    #[test]
    fn test_roundtrip_from_int() {
        for (_, ev) in members(Event::from_raw) {
            assert_eq!(Event::from_raw(ev as i32), Some(ev));
        }
    }

    #[test]
    fn test_count() {
        assert_eq!(members(Event::from_raw).len(), 15);
    }
}

mod value_type_family {
    use super::*;

    #[test]
    fn test_plain_is_zero() {
        assert_eq!(ValueTypeFamily::Plain as i32, 0);
    }

    #[test]
    fn test_illegal() {
        assert_eq!(ValueTypeFamily::Bool as i32, 7);
        assert_eq!(ValueTypeFamily::Datetime as i32, 8);
        assert_eq!(ValueTypeFamily::Illegal as i32, 9);
    }

    #[test]
    fn test_numeric_families() {
        assert_eq!(ValueTypeFamily::Uint as i32, 3);
        assert_eq!(ValueTypeFamily::Sint as i32, 2);
        assert_eq!(ValueTypeFamily::Float as i32, 4);
        assert_eq!(ValueTypeFamily::FloatFix as i32, 5);
        assert_eq!(ValueTypeFamily::FloatDec as i32, 6);
    }

    #[test]
    fn test_text_family() {
        assert_eq!(ValueTypeFamily::Utf8 as i32, 1);
    }
}

mod error_code {
    use super::*;

    #[test]
    fn test_none_is_zero() {
        assert_eq!(ErrorCode::None as i32, 0);
    }

    #[test]
    fn test_spot_checks() {
        assert_eq!(ErrorCode::EmptyIdentifier as i32, 4);
        assert_eq!(ErrorCode::IllegalEscapeSequence as i32, 9);
        assert_eq!(ErrorCode::InvalidUtf8Byte as i32, 19);
        assert_eq!(ErrorCode::UnitIllegal as i32, 32);
        assert_eq!(ErrorCode::ValueOutOfRange as i32, 35);
        assert_eq!(ErrorCode::DigitNotInBase as i32, 36);
        assert_eq!(ErrorCode::Recovered as i32, 37);
        assert_eq!(ErrorCode::UnitMismatch as i32, 38);
        assert_eq!(ErrorCode::ArrayElementTypeMismatch as i32, 39);
        assert_eq!(ErrorCode::StructShapeMismatch as i32, 40);
        assert_eq!(ErrorCode::DuplicateStructKey as i32, 41);
    }

    #[test]
    fn test_all_unique() {
        let values: Vec<i32> = members(ErrorCode::from_raw)
            .iter()
            .map(|(_, e)| *e as i32)
            .collect();
        assert!(all_unique(&values));
    }

    #[test]
    fn test_count() {
        assert_eq!(members(ErrorCode::from_raw).len(), 53);
    }

    /// Every code the LIBRARY recognises must have a Rust variant.
    ///
    /// A hardcoded count cannot catch a code added on the C side. This asks
    /// the library instead: bvn_error_to_string answers "unknown_error" for a
    /// value that is not a code, so the real ones are exactly the rest. Only
    /// membership is checked, not the name (the string is a diagnostic label).
    #[test]
    fn test_matches_the_c_enum() {
        // Scan well past the end so a newly appended code cannot hide.
        for value in 0..128 {
            let name = bovnar::Error::Write {
                code: value,
                offset: 0,
            }
            .name()
            .unwrap_or("");
            if name.is_empty() || name == "unknown_error" {
                continue;
            }
            assert!(
                ErrorCode::from_raw(value).is_some(),
                "the library knows error code {value} ({name:?}) but ErrorCode has no variant for it"
            );
        }
    }

    #[test]
    fn test_roundtrip() {
        for (_, code) in members(ErrorCode::from_raw) {
            assert_eq!(ErrorCode::from_raw(code as i32), Some(code));
        }
    }
}

#[test]
fn test_raw_conversions_round_trip_and_refuse_unknown_values() {
    use bovnar::{
        BaseUnit, ErrorCode, Event, Exponent, IECPrefix, Prefix, PrefixSystem, SIPrefix,
        UnknownValue,
    };
    assert_eq!(SIPrefix::try_from(15), Ok(SIPrefix::Kilo));
    assert_eq!(i32::from(SIPrefix::Kilo), 15);
    assert_eq!(IECPrefix::try_from(3), Ok(IECPrefix::Gibi));
    assert_eq!(PrefixSystem::try_from(1), Ok(PrefixSystem::Iec));
    assert_eq!(Event::try_from(9), Ok(Event::Data));
    assert_eq!(i32::from(ErrorCode::ValueOutOfRange), 35);
    assert_eq!(BaseUnit::try_from(100003), Ok(BaseUnit::Meter));
    assert_eq!(i32::from(BaseUnit::Meter), 100003);
    assert_eq!(BaseUnit::Meter.name(), "METER");
    assert_eq!(BaseUnit::from_name("CUP_"), Some(BaseUnit::Cup_));
    assert_eq!(BaseUnit::Cup_.to_string(), "CUP_");
    assert_eq!(BaseUnit::try_from(200000), Ok(BaseUnit::UcumIu));
    assert_eq!(
        SIPrefix::try_from(25),
        Err(UnknownValue {
            type_name: "SIPrefix",
            value: 25
        })
    );
    assert!(BaseUnit::try_from(42).is_err());
    assert!(PrefixSystem::try_from(2).is_err());
    // Exponent is open: any power in range, not only the named ones.
    assert_eq!(Exponent::try_from(42).map(Exponent::get), Ok(42));
    assert_eq!(Exponent::try_from(-1), Ok(Exponent::NEG_LINEAR));
    assert!(Exponent::try_from(101).is_err());
    assert_eq!(Prefix::from(SIPrefix::Kilo).system(), PrefixSystem::Si);
    assert!(Prefix::Iec(IECPrefix::None).is_none());
    assert_eq!(Prefix::from_raw(1, 3), Some(Prefix::Iec(IECPrefix::Gibi)));
}
