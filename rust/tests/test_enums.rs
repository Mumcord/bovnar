//! The core enums carry the C values, and the token-type and error-code enums
//! agree with include/bovnar.h.
//!
//! Rust enums cannot be iterated, so "every member" is every raw value
//! `from_raw` accepts, scanned over a range far wider than any enum. The
//! enums have no sentinel member, so the counts include only real values.

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

/// A record field of a src/gendata catalogue, read with this binding's own
/// parser. Same source the C enum is generated from, so a test built on it
/// compares the two halves rather than one half against a memory of it.
fn gendata(name: &str, table: &str) -> Vec<Vec<(String, bovnar::Value)>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../src/gendata")
        .join(name);
    let bovnar::Value::Struct(doc) = bovnar::parse(&std::fs::read(path).unwrap()).unwrap() else {
        panic!("{name} is not a struct");
    };
    let (_, bovnar::Value::Array(rows)) = doc.into_iter().find(|(k, _)| k == table).unwrap() else {
        panic!("{name}: .{table} is not an array");
    };
    rows.into_iter()
        .map(|row| match row {
            bovnar::Value::Struct(fields) => fields,
            other => panic!("{name}: row {other:?} is not a struct"),
        })
        .collect()
}

fn field<'a>(row: &'a [(String, bovnar::Value)], key: &str) -> &'a bovnar::Value {
    &row.iter().find(|(k, _)| k == key).unwrap().1
}

fn int_field(row: &[(String, bovnar::Value)], key: &str) -> i32 {
    match field(row, key) {
        bovnar::Value::Int(i) => i32::try_from(*i).unwrap(),
        other => panic!(".{key} = {other:?} is not an integer"),
    }
}

fn str_field(row: &[(String, bovnar::Value)], key: &str) -> String {
    match field(row, key) {
        bovnar::Value::String(s) => s.clone(),
        other => panic!(".{key} = {other:?} is not a string"),
    }
}

mod si_prefix {
    use super::*;
    use bovnar::SIPrefix;

    #[test]
    fn test_none_is_zero() {
        assert_eq!(SIPrefix::None as i32, 0);
    }

    #[test]
    fn test_kilo() {
        assert_eq!(SIPrefix::Kilo as i32, 15);
    }

    #[test]
    fn test_milli() {
        assert_eq!(SIPrefix::Milli as i32, 10);
    }

    #[test]
    fn test_all_unique() {
        let values: Vec<i32> = members(SIPrefix::from_raw)
            .iter()
            .map(|(_, p)| *p as i32)
            .collect();
        assert!(all_unique(&values));
    }

    // The enum has no sentinel variant, and 25 is refused as a value.
    #[test]
    fn test_count() {
        assert_eq!(members(SIPrefix::from_raw).len(), 25);
        assert_eq!(SIPrefix::from_raw(25), None);
    }

    #[test]
    fn test_quecto_quetta() {
        assert_eq!(SIPrefix::Quecto as i32, 1);
        assert_eq!(SIPrefix::Quetta as i32, 24);
    }
}

mod iec_prefix {
    use super::*;
    use bovnar::IECPrefix;

    #[test]
    fn test_none_is_zero() {
        assert_eq!(IECPrefix::None as i32, 0);
    }

    #[test]
    fn test_kibi() {
        assert_eq!(IECPrefix::Kibi as i32, 1);
    }

    #[test]
    fn test_quebi() {
        assert_eq!(IECPrefix::Quebi as i32, 10);
    }

    // No sentinel variant, and 11 is refused as a value, as for SIPrefix.
    #[test]
    fn test_count() {
        assert_eq!(members(IECPrefix::from_raw).len(), 11);
        assert_eq!(IECPrefix::from_raw(11), None);
    }

    #[test]
    fn test_all_unique() {
        let values: Vec<i32> = members(IECPrefix::from_raw)
            .iter()
            .map(|(_, p)| *p as i32)
            .collect();
        assert!(all_unique(&values));
    }
}

mod base_unit {
    use super::*;
    use bovnar::BaseUnit;
    use bovnar::enums::{CURRENCY_COUNT, CURRENCY_FIRST, CURRENCY_LAST};
    use bovnar::enums::{UNIT_BLOCK_SIZE, UNIT_NATIVE_FIRST, UNIT_NATIVE_LAST};

    #[test]
    fn test_none_is_zero() {
        assert_eq!(BaseUnit::None as i32, 0);
    }

    #[test]
    fn test_si_base_units() {
        assert_eq!(BaseUnit::Second as i32, 100002);
        assert_eq!(BaseUnit::Meter as i32, 100003);
        assert_eq!(BaseUnit::Gram as i32, 100004);
        assert_eq!(BaseUnit::Ampere as i32, 100005);
        assert_eq!(BaseUnit::Kelvin as i32, 100006);
        assert_eq!(BaseUnit::Mol as i32, 100007);
        assert_eq!(BaseUnit::Candela as i32, 100008);
    }

    #[test]
    fn test_digital_units() {
        assert_eq!(BaseUnit::Bit as i32, 100000);
        assert_eq!(BaseUnit::Byte as i32, 100001);
    }

    #[test]
    fn test_named_derived() {
        assert_eq!(BaseUnit::Hertz as i32, 100009);
        assert_eq!(BaseUnit::Newton as i32, 100010);
        assert_eq!(BaseUnit::Pascal as i32, 100011);
        assert_eq!(BaseUnit::Joule as i32, 100012);
        assert_eq!(BaseUnit::Watt as i32, 100013);
        assert_eq!(BaseUnit::Volt as i32, 100014);
        assert_eq!(BaseUnit::Ohm as i32, 100015);
        assert_eq!(BaseUnit::Farad as i32, 100016);
    }

    #[test]
    fn test_non_si() {
        assert_eq!(BaseUnit::Liter as i32, 100028);
        assert_eq!(BaseUnit::Minute as i32, 100029);
        assert_eq!(BaseUnit::Hour as i32, 100030);
        assert_eq!(BaseUnit::Day as i32, 100031);
        assert_eq!(BaseUnit::Degree as i32, 100032);
        assert_eq!(BaseUnit::Celsius as i32, 100033);
    }

    #[test]
    fn test_angular_units() {
        assert_eq!(BaseUnit::Radian as i32, 100034);
        assert_eq!(BaseUnit::Steradian as i32, 100035);
    }

    #[test]
    fn test_mass_pressure() {
        assert_eq!(BaseUnit::Tonne as i32, 100036);
        assert_eq!(BaseUnit::Bar as i32, 100037);
    }

    #[test]
    fn test_atomic_units() {
        assert_eq!(BaseUnit::Electronvolt as i32, 100038);
        assert_eq!(BaseUnit::Dalton as i32, 100039);
        assert_eq!(BaseUnit::AstronomicalUnit as i32, 100040);
    }

    #[test]
    fn test_area_time_units() {
        assert_eq!(BaseUnit::Hectare as i32, 100041);
        assert_eq!(BaseUnit::Week as i32, 100042);
        assert_eq!(BaseUnit::Year as i32, 100043);
    }

    #[test]
    fn test_all_unique() {
        let values: Vec<i32> = BaseUnit::MEMBERS.iter().map(|m| m.1 as i32).collect();
        assert!(all_unique(&values));
    }

    /// BaseUnit's native block IS units.bvnr -- every row, and no other.
    /// The names are checked too, not only the ids: the convention is the
    /// gendata `.name` upper-cased, and a member spelled differently is a
    /// member callers cannot find.
    #[test]
    fn test_native_block_closed_against_gendata() {
        let want: BTreeMap<i32, String> = gendata("units.bvnr", "units")
            .iter()
            .map(|u| (int_field(u, "id"), str_field(u, "name").to_uppercase()))
            .collect();
        let got: BTreeMap<i32, String> = BaseUnit::MEMBERS
            .iter()
            .filter(|m| {
                (UNIT_NATIVE_FIRST..UNIT_NATIVE_FIRST + UNIT_BLOCK_SIZE).contains(&(m.1 as i32))
            })
            .map(|m| (m.1 as i32, m.0.to_string()))
            .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn test_native_bounds_match_gendata() {
        let units = gendata("units.bvnr", "units");
        assert_eq!(UNIT_NATIVE_FIRST, int_field(&units[0], "id"));
        assert_eq!(UNIT_NATIVE_LAST, int_field(units.last().unwrap(), "id"));
    }

    /// The currency block IS currencies.bvnr, in order. A code that is
    /// already a native unit's name takes a trailing underscore (CUP is the
    /// Cuban Peso and the US cup); the rule is applied rather than the
    /// exception listed, so a second collision is handled too.
    #[test]
    fn test_currency_block_closed_against_gendata() {
        let units: BTreeSet<String> = gendata("units.bvnr", "units")
            .iter()
            .map(|u| str_field(u, "name").to_uppercase())
            .collect();
        let codes: Vec<String> = gendata("currencies.bvnr", "currencies")
            .iter()
            .map(|c| str_field(c, "code"))
            .collect();
        let want: Vec<String> = codes
            .iter()
            .map(|c| {
                if units.contains(c) {
                    format!("{c}_")
                } else {
                    c.clone()
                }
            })
            .collect();
        let got: Vec<String> = BaseUnit::MEMBERS
            .iter()
            .filter(|m| (CURRENCY_FIRST..=CURRENCY_LAST).contains(&(m.1 as i32)))
            .map(|m| m.0.to_string())
            .collect();
        assert_eq!(got, want);
        assert_eq!(CURRENCY_COUNT as usize, codes.len());
    }

    /// Nothing but NONE lives between or beyond the two blocks. The enum also
    /// names the unit profiles' opaque ids (`UCUM_IU`, blocks 20..80 of
    /// include/bovnar.h, 200000..899999); they are the one other range
    /// allowed here.
    #[test]
    fn test_no_members_outside_the_two_blocks() {
        let stray: Vec<&str> = BaseUnit::MEMBERS
            .iter()
            .filter(|m| {
                let id = m.1 as i32;
                id != 0
                    && !(UNIT_NATIVE_FIRST..=UNIT_NATIVE_LAST).contains(&id)
                    && !(CURRENCY_FIRST..=CURRENCY_LAST).contains(&id)
                    && !(2 * UNIT_NATIVE_FIRST..CURRENCY_FIRST).contains(&id)
            })
            .map(|m| m.0)
            .collect();
        assert_eq!(stray, Vec::<&str>::new());
    }
}

mod exponent {
    use super::*;
    use bovnar::Exponent;

    #[test]
    fn test_positive_exponents() {
        assert_eq!(Exponent::LINEAR.get(), 1);
        assert_eq!(Exponent::SQUARE.get(), 2);
        assert_eq!(Exponent::CUBIC.get(), 3);
        assert_eq!(Exponent::NONIC.get(), 9);
    }

    #[test]
    fn test_negative_exponents() {
        assert_eq!(Exponent::NEG_LINEAR.get(), -1);
        assert_eq!(Exponent::NEG_SQUARE.get(), -2);
        assert_eq!(Exponent::NEG_NONIC.get(), -9);
    }

    #[test]
    fn test_all_unique() {
        let named = [
            Exponent::INVALID,
            Exponent::LINEAR,
            Exponent::SQUARE,
            Exponent::CUBIC,
            Exponent::QUARTIC,
            Exponent::QUINTIC,
            Exponent::SEXTIC,
            Exponent::SEPTIC,
            Exponent::OCTIC,
            Exponent::NONIC,
            Exponent::NEG_LINEAR,
            Exponent::NEG_SQUARE,
            Exponent::NEG_CUBIC,
            Exponent::NEG_QUARTIC,
            Exponent::NEG_QUINTIC,
            Exponent::NEG_SEXTIC,
            Exponent::NEG_SEPTIC,
            Exponent::NEG_OCTIC,
            Exponent::NEG_NONIC,
        ];
        let values: Vec<i32> = named.iter().map(|e| e.get()).collect();
        assert!(all_unique(&values));
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
