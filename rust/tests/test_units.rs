//! The unit helpers, unit parsing and formatting, and read-time unit
//! conversion through the `want_unit` hook.
//!
//! A unit is built with `Unit::si` / `Unit::iec` / `Unit::compound` /
//! `Unit::dimensionless` / `Unit::default`, a float in a unit is written with
//! `Writer::write_quantity`, and files are read with
//! `Reader::read_reader_want_unit` over the opened file.

use bovnar::enums::ValueTypeFamily;
use bovnar::units::{
    UnitFlags, available_profiles, exponent_to_int, int_to_exponent, parse_unit, prefix_unit_valid,
    unit_convert_factor, unit_error_code, unit_factor, unit_prefix_exponent, unit_prefix_factor,
    unit_to_str, unit_to_str_ex, unit_valid, units_compatible,
};
use bovnar::{
    BaseUnit, Error, ErrorCode, Event, EventData, Exponent, IECPrefix, MAX_UNIT_COMPONENTS, Prefix,
    PrefixSystem, Quantity, ReadOptions, Reader, Result, SIPrefix, TokenType, Unit, UnitComponent,
    Value, ValueTypeSpec, Writer,
};

// Shared fixtures.
const SIMPLE_SCALAR_FLOAT: &[u8] = b".velocity = <float:64,m/s> 9.81;\n";
const IEC_UNIT_PAYLOAD: &[u8] = b".ram = <uint:64,Gi~B> 8;\n";
const COMPOUND_UNIT_PAYLOAD: &str = ".force = <float:64,k~g\u{b7}m/s\u{b2}> 9.81;\n";
const UNTYPED_INTEGER: &[u8] = b".count = 1000;\n";
const ALL_SI_UNITS: &[u8] = b".temperature = <float:64,K> 300.0;\n\
.pressure = <float:64,k~Pa> 101.325;\n\
.energy = <float:64,k~J> 5400.0;\n\
.speed = <float:64,m/s> 12.5;\n";

/// `BVNR_UNIT_STRING_MAX`, the longest unit string the library writes.
const UNIT_STRING_MAX: usize = 1088;

fn p(s: &str) -> Unit {
    parse_unit(s).unwrap_or_else(|e| panic!("{s:?} should parse: {e}"))
}

fn c0(s: &str) -> UnitComponent {
    p(s).components()[0]
}

fn is_arg_err<T: std::fmt::Debug>(r: Result<T>) -> bool {
    matches!(r, Err(Error::Argument { .. }))
}

/// `math.isclose` with its default relative tolerance.
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs())
}

fn si(base: BaseUnit) -> Unit {
    Unit::si(base, SIPrefix::None, Exponent::LINEAR)
}

fn si_p(base: BaseUnit, prefix: SIPrefix) -> Unit {
    Unit::si(base, prefix, Exponent::LINEAR)
}

fn iec_p(base: BaseUnit, prefix: IECPrefix) -> Unit {
    Unit::iec(base, prefix, Exponent::LINEAR)
}

mod unit_helpers_pure {
    use super::*;

    #[test]
    fn test_dimensionless_is_none_base() {
        let vu = Unit::dimensionless();
        assert_eq!(vu.num_components(), 1);
        assert_eq!(vu.components()[0].base, BaseUnit::None);
    }

    #[test]
    fn test_si_meter_no_prefix() {
        let c = si(BaseUnit::Meter).components()[0];
        assert_eq!(c.base, BaseUnit::Meter);
        assert_eq!(c.prefix_system(), PrefixSystem::Si);
        assert_eq!(c.prefix, SIPrefix::None);
        assert_eq!(c.exponent, Exponent::LINEAR);
    }

    #[test]
    fn test_si_kilogram() {
        let c = si_p(BaseUnit::Gram, SIPrefix::Kilo).components()[0];
        assert_eq!(c.prefix, SIPrefix::Kilo);
    }

    #[test]
    fn test_si_meter_squared() {
        let vu = Unit::si(BaseUnit::Meter, SIPrefix::None, Exponent::SQUARE);
        assert_eq!(vu.components()[0].exponent, Exponent::SQUARE);
    }

    #[test]
    fn test_iec_gibibyte() {
        let c = iec_p(BaseUnit::Byte, IECPrefix::Gibi).components()[0];
        assert_eq!(c.prefix_system(), PrefixSystem::Iec);
        assert_eq!(c.prefix, IECPrefix::Gibi);
        assert_eq!(c.base, BaseUnit::Byte);
    }

    #[test]
    fn test_unit_none_has_zero_components() {
        let vu = Unit::default();
        assert_eq!(vu.num_components(), 0);
        assert!(vu.is_dimensionless());
    }

    #[test]
    fn test_active_components_length() {
        assert_eq!(si(BaseUnit::Second).components().len(), 1);
    }
}

mod unit_flags {
    use super::*;

    #[test]
    fn test_none_is_zero() {
        assert_eq!(UnitFlags::NONE, 0);
    }

    #[test]
    fn test_reduce_bit() {
        assert_eq!(UnitFlags::REDUCE, 1);
    }

    #[test]
    fn test_ascii_exp_bit() {
        assert_eq!(UnitFlags::ASCII_EXP, 2);
    }

    #[test]
    fn test_combine_flags() {
        assert_eq!(UnitFlags::REDUCE | UnitFlags::ASCII_EXP, 3);
    }

    #[test]
    fn test_none_is_falsy() {
        assert_eq!(UnitFlags::NONE, 0);
    }

    #[test]
    fn test_reduce_is_truthy() {
        assert_ne!(UnitFlags::REDUCE, 0);
    }

    #[test]
    fn test_membership() {
        let f = UnitFlags::REDUCE | UnitFlags::ASCII_EXP;
        assert_eq!(f & UnitFlags::REDUCE, UnitFlags::REDUCE);
        assert_eq!(f & UnitFlags::ASCII_EXP, UnitFlags::ASCII_EXP);
    }

    #[test]
    fn test_from_int() {
        assert_eq!(0, UnitFlags::NONE);
        assert_eq!(1, UnitFlags::REDUCE);
        assert_eq!(2, UnitFlags::ASCII_EXP);
        assert_eq!(3, UnitFlags::REDUCE | UnitFlags::ASCII_EXP);
    }
}

mod exponent_to_int_tests {
    use super::*;

    #[test]
    fn test_linear() {
        assert_eq!(exponent_to_int(Exponent::LINEAR), 1);
    }

    #[test]
    fn test_square() {
        assert_eq!(exponent_to_int(Exponent::SQUARE), 2);
    }

    #[test]
    fn test_cubic() {
        assert_eq!(exponent_to_int(Exponent::CUBIC), 3);
    }

    #[test]
    fn test_neg_linear() {
        assert_eq!(exponent_to_int(Exponent::NEG_LINEAR), -1);
    }

    #[test]
    fn test_neg_square() {
        assert_eq!(exponent_to_int(Exponent::NEG_SQUARE), -2);
    }

    #[test]
    fn test_neg_cubic() {
        assert_eq!(exponent_to_int(Exponent::NEG_CUBIC), -3);
    }

    #[test]
    fn test_nonic() {
        assert_eq!(exponent_to_int(Exponent::NONIC), 9);
    }

    #[test]
    fn test_neg_nonic() {
        assert_eq!(exponent_to_int(Exponent::NEG_NONIC), -9);
    }

    #[test]
    fn test_invalid_returns_zero() {
        assert_eq!(exponent_to_int(Exponent::INVALID), 0);
    }

    #[test]
    fn test_raw_int_accepted() {
        assert_eq!(exponent_to_int(Exponent::try_from(1).unwrap()), 1);
    }
}

mod int_to_exponent_tests {
    use super::*;

    #[test]
    fn test_1_is_linear() {
        assert_eq!(int_to_exponent(1), Exponent::LINEAR);
    }

    #[test]
    fn test_2_is_square() {
        assert_eq!(int_to_exponent(2), Exponent::SQUARE);
    }

    #[test]
    fn test_3_is_cubic() {
        assert_eq!(int_to_exponent(3), Exponent::CUBIC);
    }

    #[test]
    fn test_neg1_is_neg_linear() {
        assert_eq!(int_to_exponent(-1), Exponent::NEG_LINEAR);
    }

    #[test]
    fn test_neg2_is_neg_square() {
        assert_eq!(int_to_exponent(-2), Exponent::NEG_SQUARE);
    }

    #[test]
    fn test_neg9_is_neg_nonic() {
        assert_eq!(int_to_exponent(-9), Exponent::NEG_NONIC);
    }

    #[test]
    fn test_9_is_nonic() {
        assert_eq!(int_to_exponent(9), Exponent::NONIC);
    }

    #[test]
    fn test_zero_is_invalid() {
        assert_eq!(int_to_exponent(0), Exponent::INVALID);
    }

    #[test]
    fn test_roundtrip_all_valid() {
        for n in (-9..10).filter(|&n| n != 0) {
            assert_eq!(exponent_to_int(int_to_exponent(n)), n);
        }
    }
}

mod unit_parsing {
    use super::*;

    #[test]
    fn test_simple_meter() {
        let vu = p("m");
        assert_eq!(vu.num_components(), 1);
        assert_eq!(vu.components()[0].base, BaseUnit::Meter);
    }

    #[test]
    fn test_kilometer() {
        let c = c0("k~m");
        assert_eq!(c.base, BaseUnit::Meter);
        assert_eq!(c.prefix, SIPrefix::Kilo);
    }

    #[test]
    fn test_gibibytes() {
        let c = c0("Gi~B");
        assert_eq!(c.base, BaseUnit::Byte);
        assert_eq!(c.prefix_system(), PrefixSystem::Iec);
        assert_eq!(c.prefix, IECPrefix::Gibi);
    }

    #[test]
    fn test_meters_per_second() {
        let vu = p("m/s");
        assert_eq!(vu.num_components(), 2);
        let c = vu.components();
        assert_eq!(c[0].base, BaseUnit::Meter);
        assert_eq!(c[1].base, BaseUnit::Second);
        assert_eq!(c[1].exponent, Exponent::NEG_LINEAR);
    }

    #[test]
    fn test_acceleration_unicode() {
        let vu = p("m/s\u{b2}");
        assert_eq!(vu.num_components(), 2);
        assert_eq!(vu.components()[1].exponent, Exponent::NEG_SQUARE);
    }

    #[test]
    fn test_acceleration_ascii_caret() {
        let vu = p("m/s^2");
        assert_eq!(vu.num_components(), 2);
        assert_eq!(vu.components()[1].exponent, Exponent::NEG_SQUARE);
    }

    #[test]
    fn test_dalton_aliases() {
        // Da is the canonical symbol; dalton/amu/u are accepted input aliases
        // (amu and u = unified atomic mass unit). All resolve to bu_dalton and
        // serialise back to "Da".
        for tok in ["Da", "dalton", "amu", "u"] {
            let vu = p(tok);
            assert_eq!(vu.num_components(), 1, "{tok}");
            assert_eq!(vu.components()[0].base, BaseUnit::Dalton, "{tok}");
            assert_eq!(unit_to_str(&vu).unwrap(), "Da", "{tok}");
        }
    }

    #[test]
    fn test_single_char_u_does_not_shadow_longer_units() {
        // Adding the 1-char "u" alias must not preempt longest-suffix matching.
        assert_eq!(c0("au").base, BaseUnit::AstronomicalUnit);
        assert_eq!(c0("mol").base, BaseUnit::Mol);
        assert_eq!(c0("µ~u").base, BaseUnit::Dalton); // micro-dalton
    }

    #[test]
    fn test_ascii_u_micro_prefix() {
        // "u" is an ASCII input alias for the micro prefix "µ"; canonical
        // output always normalises back to "µ".
        for (ascii_form, canonical) in [("u~m", "µ~m"), ("u~s", "µ~s"), ("u~Pa", "µ~Pa")] {
            let vu = p(ascii_form);
            assert_eq!(vu.components()[0].prefix, SIPrefix::Micro);
            assert_eq!(unit_to_str(&vu).unwrap(), canonical);
        }
    }

    #[test]
    fn test_u_micro_prefix_and_dalton_base_coexist() {
        let vu = p("u~u");
        assert_eq!(vu.components()[0].prefix, SIPrefix::Micro);
        assert_eq!(vu.components()[0].base, BaseUnit::Dalton);
        assert_eq!(unit_to_str(&vu).unwrap(), "µ~Da");
    }

    #[test]
    fn test_force_kgms2() {
        let vu = p("k~g\u{b7}m/s\u{b2}");
        assert_eq!(vu.num_components(), 3);
        let c = vu.components();
        assert_eq!(c[0].base, BaseUnit::Gram);
        assert_eq!(c[0].prefix, SIPrefix::Kilo);
        assert_eq!(c[1].base, BaseUnit::Meter);
        assert_eq!(c[2].base, BaseUnit::Second);
        assert_eq!(c[2].exponent, Exponent::NEG_SQUARE);
    }

    #[test]
    fn test_no_unit_keyword() {
        assert!(p("no_unit").is_dimensionless());
    }

    #[test]
    fn test_kelvin() {
        assert_eq!(c0("K").base, BaseUnit::Kelvin);
    }

    #[test]
    fn test_megahertz() {
        let c = c0("M~Hz");
        assert_eq!(c.base, BaseUnit::Hertz);
        assert_eq!(c.prefix, SIPrefix::Mega);
    }

    #[test]
    fn test_tebibyte() {
        let c = c0("Ti~B");
        assert_eq!(c.base, BaseUnit::Byte);
        assert_eq!(c.prefix_system(), PrefixSystem::Iec);
        assert_eq!(c.prefix, IECPrefix::Tebi);
    }

    #[test]
    fn test_product_asterisk() {
        let vu = p("m*s");
        assert_eq!(vu.num_components(), 2);
        assert_eq!(vu.components()[0].base, BaseUnit::Meter);
        assert_eq!(vu.components()[1].base, BaseUnit::Second);
    }

    #[test]
    fn test_invalid_unit_raises() {
        assert!(is_arg_err(parse_unit("x-m")));
    }

    #[test]
    fn test_invalid_unit_empty_component() {
        assert!(is_arg_err(parse_unit("m//s")));
    }

    #[test]
    fn test_too_many_components() {
        // One past MAX_UNIT_COMPONENTS, built from the constant.
        let atoms = [
            "m", "s", "g", "A", "K", "mol", "cd", "b", "B", "Hz", "N", "Pa", "J", "W", "V", "F",
            "C", "S", "Wb", "T", "H", "lm", "lx", "Bq", "Gy", "Sv", "kat", "L", "min", "h", "d",
            "wk", "yr", "rad", "sr", "bar", "eV", "t", "ha", "au",
        ];
        assert!(
            atoms.len() > MAX_UNIT_COMPONENTS,
            "need one atom past the limit"
        );
        assert!(is_arg_err(parse_unit(
            &atoms[..=MAX_UNIT_COMPONENTS].join("*")
        )));
    }

    #[test]
    fn test_celsius() {
        assert_eq!(c0("\u{b0}C").base, BaseUnit::Celsius);
    }
}

mod unit_valid_tests {
    use super::*;

    #[test]
    fn test_meter_valid() {
        assert!(unit_valid(&si(BaseUnit::Meter)));
    }

    #[test]
    fn test_kilogram_valid() {
        assert!(unit_valid(&si_p(BaseUnit::Gram, SIPrefix::Kilo)));
    }

    #[test]
    fn test_gibibyte_valid() {
        assert!(unit_valid(&iec_p(BaseUnit::Byte, IECPrefix::Gibi)));
    }

    #[test]
    fn test_dimensionless_valid() {
        assert!(unit_valid(&Unit::dimensionless()));
    }

    #[test]
    fn test_unit_none_valid() {
        assert!(unit_valid(&Unit::default()));
    }

    #[test]
    fn test_celsius_valid() {
        assert!(unit_valid(&si(BaseUnit::Celsius)));
    }

    #[test]
    fn test_parsed_unit_valid() {
        assert!(unit_valid(&p("k~g\u{b7}m/s\u{b2}")));
    }
}

mod unit_prefix_factor_tests {
    use super::*;

    #[test]
    fn test_no_prefix_is_one() {
        assert!(close(unit_prefix_factor(&si(BaseUnit::Meter)), 1.0));
    }

    #[test]
    fn test_kilo_is_1000() {
        assert!(close(
            unit_prefix_factor(&si_p(BaseUnit::Meter, SIPrefix::Kilo)),
            1000.0
        ));
    }

    #[test]
    fn test_milli_is_1e_neg3() {
        assert!(close(
            unit_prefix_factor(&si_p(BaseUnit::Second, SIPrefix::Milli)),
            1e-3
        ));
    }

    #[test]
    fn test_mega_is_1e6() {
        assert!(close(
            unit_prefix_factor(&si_p(BaseUnit::Hertz, SIPrefix::Mega)),
            1e6
        ));
    }

    #[test]
    fn test_gibi_is_2_pow_30() {
        assert!(close(
            unit_prefix_factor(&iec_p(BaseUnit::Byte, IECPrefix::Gibi)),
            (1u64 << 30) as f64
        ));
    }

    #[test]
    fn test_kibi_is_2_pow_10() {
        assert!(close(
            unit_prefix_factor(&iec_p(BaseUnit::Byte, IECPrefix::Kibi)),
            1024.0
        ));
    }

    #[test]
    fn test_compound_kilo_numerator() {
        assert!(close(unit_prefix_factor(&p("k~m/s")), 1000.0));
    }

    #[test]
    fn test_compound_milli_denominator() {
        assert!(close(unit_prefix_factor(&p("m/m~s")), 1e3));
    }

    #[test]
    fn test_dimensionless_is_one() {
        assert!(close(unit_prefix_factor(&Unit::dimensionless()), 1.0));
    }
}

mod unit_prefix_exponent_tests {
    use super::*;

    #[test]
    fn test_no_prefix_is_zero() {
        assert_eq!(unit_prefix_exponent(&si(BaseUnit::Meter)), 0);
    }

    #[test]
    fn test_kilo_is_3() {
        assert_eq!(
            unit_prefix_exponent(&si_p(BaseUnit::Meter, SIPrefix::Kilo)),
            3
        );
    }

    #[test]
    fn test_mega_is_6() {
        assert_eq!(
            unit_prefix_exponent(&si_p(BaseUnit::Hertz, SIPrefix::Mega)),
            6
        );
    }

    #[test]
    fn test_milli_is_neg3() {
        assert_eq!(
            unit_prefix_exponent(&si_p(BaseUnit::Second, SIPrefix::Milli)),
            -3
        );
    }

    #[test]
    fn test_gibi_is_30() {
        assert_eq!(
            unit_prefix_exponent(&iec_p(BaseUnit::Byte, IECPrefix::Gibi)),
            30
        );
    }

    #[test]
    fn test_kibi_is_10() {
        assert_eq!(
            unit_prefix_exponent(&iec_p(BaseUnit::Byte, IECPrefix::Kibi)),
            10
        );
    }

    #[test]
    fn test_dimensionless_is_zero() {
        assert_eq!(unit_prefix_exponent(&Unit::dimensionless()), 0);
    }
}

mod prefix_unit_valid_tests {
    use super::*;

    #[test]
    fn test_si_none_on_meter_valid() {
        assert!(prefix_unit_valid(
            Prefix::from(SIPrefix::None),
            BaseUnit::Meter
        ));
    }

    #[test]
    fn test_si_kilo_on_meter_valid() {
        assert!(prefix_unit_valid(
            Prefix::from(SIPrefix::Kilo),
            BaseUnit::Meter
        ));
    }

    #[test]
    fn test_si_milli_on_second_valid() {
        assert!(prefix_unit_valid(
            Prefix::from(SIPrefix::Milli),
            BaseUnit::Second
        ));
    }

    #[test]
    fn test_iec_gibi_on_byte_valid() {
        assert!(prefix_unit_valid(
            Prefix::from(IECPrefix::Gibi),
            BaseUnit::Byte
        ));
    }

    #[test]
    fn test_iec_kibi_on_bit_valid() {
        assert!(prefix_unit_valid(
            Prefix::from(IECPrefix::Kibi),
            BaseUnit::Bit
        ));
    }

    #[test]
    fn test_iec_gibi_on_meter_invalid() {
        assert!(!prefix_unit_valid(
            Prefix::from(IECPrefix::Gibi),
            BaseUnit::Meter
        ));
    }

    #[test]
    fn test_iec_kibi_on_second_invalid() {
        assert!(!prefix_unit_valid(
            Prefix::from(IECPrefix::Kibi),
            BaseUnit::Second
        ));
    }

    #[test]
    fn test_iec_mebi_on_gram_invalid() {
        assert!(!prefix_unit_valid(
            Prefix::from(IECPrefix::Mebi),
            BaseUnit::Gram
        ));
    }

    #[test]
    fn test_prefix_from_parsed_unit_component() {
        let c = c0("Gi~B");
        assert!(prefix_unit_valid(c.prefix, c.base));
    }
}

mod unit_serialisation {
    use super::*;

    #[test]
    fn test_meter() {
        assert_eq!(unit_to_str(&si(BaseUnit::Meter)).unwrap(), "m");
    }

    #[test]
    fn test_kilometer() {
        assert_eq!(
            unit_to_str(&si_p(BaseUnit::Gram, SIPrefix::Kilo)).unwrap(),
            "k~g"
        );
    }

    #[test]
    fn test_gibibyte() {
        assert_eq!(
            unit_to_str(&iec_p(BaseUnit::Byte, IECPrefix::Gibi)).unwrap(),
            "Gi~B"
        );
    }

    #[test]
    fn test_roundtrip_simple() {
        let vu1 = p("m/s");
        let vu2 = p(&unit_to_str(&vu1).unwrap());
        assert_eq!(vu2.num_components(), vu1.num_components());
    }

    #[test]
    fn test_roundtrip_compound() {
        let vu = p("k~g\u{b7}m/s\u{b2}");
        let vu2 = p(&unit_to_str(&vu).unwrap());
        assert_eq!(vu2.num_components(), vu.num_components());
    }
}

mod unit_to_str_ex_tests {
    use super::*;

    #[test]
    fn test_ascii_exp_flag_produces_caret() {
        let s = unit_to_str_ex(&p("m/s\u{b2}"), UnitFlags::ASCII_EXP).unwrap();
        assert!(s.contains('^'));
        assert!(!s.contains('\u{b2}'));
    }

    #[test]
    fn test_no_flags_produces_unicode() {
        let s = unit_to_str_ex(&p("m/s\u{b2}"), UnitFlags::NONE).unwrap();
        assert!(s.contains('\u{b2}'));
    }

    #[test]
    fn test_default_flags_same_as_none() {
        // Rust has no default argument; unit_to_str is the no-flags form.
        let vu = p("m/s\u{b2}");
        assert_eq!(
            unit_to_str(&vu).unwrap(),
            unit_to_str_ex(&vu, UnitFlags::NONE).unwrap()
        );
    }

    #[test]
    fn test_ascii_exp_roundtrips() {
        let vu1 = p("k~g\u{b7}m/s\u{b2}");
        let vu2 = p(&unit_to_str_ex(&vu1, UnitFlags::ASCII_EXP).unwrap());
        assert_eq!(vu2.num_components(), vu1.num_components());
        for (a, b) in vu1.components().iter().zip(vu2.components()) {
            assert_eq!(b.base, a.base);
        }
    }

    #[test]
    fn test_reduce_flag_simplifies_force() {
        let s = unit_to_str_ex(&p("k~g\u{b7}m/s\u{b2}"), UnitFlags::REDUCE).unwrap();
        assert_eq!(s, "N");
    }

    #[test]
    fn test_reduce_and_ascii_exp_combined() {
        let s = unit_to_str_ex(&p("m\u{b2}"), UnitFlags::REDUCE | UnitFlags::ASCII_EXP).unwrap();
        assert!(!s.is_empty());
    }

    #[test]
    fn test_int_flags_accepted() {
        assert!(unit_to_str_ex(&p("m/s"), 0).unwrap().contains('m'));
    }
}

mod unit_factor_tests {
    use super::*;

    #[test]
    fn test_no_prefix_factor_is_one() {
        assert!(close(unit_factor("m").unwrap(), 1.0));
    }

    #[test]
    fn test_kilo_factor() {
        assert!(close(unit_factor("k~m").unwrap(), 1000.0));
    }

    #[test]
    fn test_milli_factor() {
        assert!(close(unit_factor("m~s").unwrap(), 1e-3));
    }

    #[test]
    fn test_kilo_in_numerator() {
        assert!(close(unit_factor("k~m/s").unwrap(), 1000.0));
    }

    #[test]
    fn test_denominator_inverts() {
        assert!(close(unit_factor("m/k~s").unwrap(), 1e-3));
    }

    #[test]
    fn test_gibi_factor() {
        assert!(close(unit_factor("Gi~B").unwrap(), (1u64 << 30) as f64));
    }

    #[test]
    fn test_dimensionless_factor() {
        assert!(close(unit_factor("no_unit").unwrap(), 1.0));
    }

    #[test]
    fn test_micro_factor() {
        assert!(close(unit_factor("\u{b5}~m").unwrap(), 1e-6));
    }

    #[test]
    fn test_invalid_unit_raises() {
        assert!(is_arg_err(unit_factor("XYZ")));
    }
}

mod unit_in_event_stream {
    use super::*;

    /// The units on every type-annotation parameter event of `payload`.
    fn param_units(payload: &[u8]) -> Vec<Unit> {
        let mut found = Vec::new();
        let mut cb = |d: &EventData<'_>| {
            if d.event == Event::TypeAnnotationTypeFamilyParam {
                found.push(d.value_unit);
            }
            true
        };
        Reader::new()
            .unwrap()
            .read_mem(payload, &ReadOptions::default(), Some(&mut cb), None)
            .unwrap();
        found
    }

    /// The last non-dimensionless parameter unit of `payload`.
    fn unit_from_param(payload: &[u8]) -> Option<Unit> {
        param_units(payload)
            .into_iter()
            .rfind(|u| !u.is_dimensionless())
    }

    #[test]
    fn test_float_velocity_unit() {
        let vu = unit_from_param(SIMPLE_SCALAR_FLOAT).expect("a unit");
        assert_eq!(vu.num_components(), 2);
        let bases: Vec<BaseUnit> = vu.components().iter().map(|c| c.base).collect();
        assert!(bases.contains(&BaseUnit::Meter));
        assert!(bases.contains(&BaseUnit::Second));
    }

    #[test]
    fn test_iec_unit_in_event() {
        let c = unit_from_param(IEC_UNIT_PAYLOAD)
            .expect("a unit")
            .components()[0];
        assert_eq!(c.prefix_system(), PrefixSystem::Iec);
        assert_eq!(c.prefix, IECPrefix::Gibi);
        assert_eq!(c.base, BaseUnit::Byte);
    }

    #[test]
    fn test_compound_unit_in_event() {
        let vu = unit_from_param(COMPOUND_UNIT_PAYLOAD.as_bytes()).expect("a unit");
        assert_eq!(vu.num_components(), 3);
    }

    #[test]
    fn test_si_temperature_unit() {
        let found = param_units(ALL_SI_UNITS);
        assert!(found.iter().filter(|u| !u.is_dimensionless()).count() >= 1);
    }

    #[test]
    fn test_no_annotation_dimensionless() {
        let found = param_units(UNTYPED_INTEGER);
        assert!(found.iter().any(Unit::is_dimensionless));
    }
}

mod writer_unit_annotation {
    use super::*;

    fn written(key: &str, q: &Quantity) -> Vec<u8> {
        let mut w = Writer::to_mem(4096, false).unwrap();
        w.write_quantity(key, q).unwrap();
        w.finish().unwrap();
        w.get_output().to_vec()
    }

    fn vt(family: ValueTypeFamily, width: u32) -> ValueTypeSpec {
        ValueTypeSpec {
            family: family as i32,
            width,
            base: 0,
        }
    }

    #[test]
    fn test_unit_str_in_output() {
        let q = Quantity::new("9.81", vt(ValueTypeFamily::Float, 64), p("m/s"));
        assert!(written("velocity", &q).contains(&b'm'));
    }

    #[test]
    fn test_iec_unit_in_output() {
        let q = Quantity::new(
            "8",
            vt(ValueTypeFamily::Uint, 64),
            iec_p(BaseUnit::Byte, IECPrefix::Gibi),
        );
        let out = written("ram", &q);
        assert!(out.windows(3).any(|w| w == b"ram"));
    }
}

fn velocity_components() -> [UnitComponent; 2] {
    [
        UnitComponent::si(BaseUnit::Meter, SIPrefix::None, Exponent::LINEAR),
        UnitComponent::si(BaseUnit::Second, SIPrefix::None, Exponent::NEG_LINEAR),
    ]
}

fn force_components() -> [UnitComponent; 3] {
    [
        UnitComponent::si(BaseUnit::Gram, SIPrefix::Kilo, Exponent::LINEAR),
        UnitComponent::si(BaseUnit::Meter, SIPrefix::None, Exponent::LINEAR),
        UnitComponent::si(BaseUnit::Second, SIPrefix::None, Exponent::NEG_SQUARE),
    ]
}

mod make_unit_compound_physics {
    use super::*;

    #[test]
    fn test_two_component_velocity_structure() {
        let vu = Unit::compound(&velocity_components()).unwrap();
        assert_eq!(vu.num_components(), 2);
    }

    #[test]
    fn test_force_structure() {
        let vu = Unit::compound(&force_components()).unwrap();
        assert_eq!(vu.num_components(), 3);
    }
}

mod make_unit_compound_roundtrip {
    use super::*;

    #[test]
    fn test_velocity_roundtrip() {
        let vu = Unit::compound(&velocity_components()).unwrap();
        assert_eq!(p(&unit_to_str(&vu).unwrap()).num_components(), 2);
    }

    #[test]
    fn test_force_roundtrip() {
        let vu = Unit::compound(&force_components()).unwrap();
        assert_eq!(
            p(&unit_to_str(&vu).unwrap()).num_components(),
            vu.num_components()
        );
    }

    #[test]
    fn test_gibibyte_roundtrip() {
        let vu = Unit::compound(&[UnitComponent::iec(
            BaseUnit::Byte,
            IECPrefix::Gibi,
            Exponent::LINEAR,
        )])
        .unwrap();
        assert_eq!(p(&unit_to_str(&vu).unwrap()).num_components(), 1);
    }
}

mod make_unit_compound_compatibility {
    use super::*;

    #[test]
    fn test_velocity_compatible_with_parsed() {
        let built = Unit::compound(&velocity_components()).unwrap();
        assert!(units_compatible(&built, &p("m/s")));
    }

    #[test]
    fn test_force_compatible_with_parsed() {
        let built = Unit::compound(&force_components()).unwrap();
        assert!(units_compatible(&built, &p("k~g\u{b7}m/s\u{b2}")));
    }
}

mod new_unit_parsing {
    use super::*;

    fn base(s: &str) -> BaseUnit {
        c0(s).base
    }

    #[test]
    fn test_kgf_symbol() {
        assert_eq!(base("kgf"), BaseUnit::KilogramForce);
    }

    #[test]
    fn test_kgf_long() {
        assert_eq!(base("kilogram_force"), BaseUnit::KilogramForce);
    }

    #[test]
    fn test_inhg_symbol() {
        assert_eq!(base("inHg"), BaseUnit::InchHg);
    }

    #[test]
    fn test_inhg_long() {
        assert_eq!(base("inch_hg"), BaseUnit::InchHg);
    }

    #[test]
    fn test_inch_mercury_long() {
        assert_eq!(base("inch_mercury"), BaseUnit::InchHg);
    }

    #[test]
    fn test_rpm_symbol() {
        assert_eq!(base("rpm"), BaseUnit::Rpm);
    }

    #[test]
    fn test_ft_lb_symbol() {
        assert_eq!(base("ft_lb"), BaseUnit::FootPound);
    }

    #[test]
    fn test_foot_pound_long() {
        assert_eq!(base("foot_pound"), BaseUnit::FootPound);
    }

    #[test]
    fn test_foot_pounds_plural() {
        assert_eq!(base("foot_pounds"), BaseUnit::FootPound);
    }

    #[test]
    fn test_dr_symbol() {
        assert_eq!(base("dr"), BaseUnit::Dram);
    }

    #[test]
    fn test_dram_long() {
        assert_eq!(base("dram"), BaseUnit::Dram);
    }

    #[test]
    fn test_drams_plural() {
        assert_eq!(base("drams"), BaseUnit::Dram);
    }

    #[test]
    fn test_dwt_symbol() {
        assert_eq!(base("dwt"), BaseUnit::Pennyweight);
    }

    #[test]
    fn test_pennyweight_long() {
        assert_eq!(base("pennyweight"), BaseUnit::Pennyweight);
    }

    #[test]
    fn test_pennyweights_plural() {
        assert_eq!(base("pennyweights"), BaseUnit::Pennyweight);
    }

    #[test]
    fn test_ch_symbol() {
        assert_eq!(base("ch"), BaseUnit::Chain);
    }

    #[test]
    fn test_chain_long() {
        assert_eq!(base("chain"), BaseUnit::Chain);
    }

    #[test]
    fn test_chains_plural() {
        assert_eq!(base("chains"), BaseUnit::Chain);
    }

    #[test]
    fn test_rd_symbol() {
        assert_eq!(base("rd"), BaseUnit::Rod);
    }

    #[test]
    fn test_rod_long() {
        assert_eq!(base("rod"), BaseUnit::Rod);
    }

    #[test]
    fn test_rods_plural() {
        assert_eq!(base("rods"), BaseUnit::Rod);
    }

    #[test]
    fn test_gi_symbol() {
        assert_eq!(base("gi"), BaseUnit::Gill);
    }

    #[test]
    fn test_gill_long() {
        assert_eq!(base("gill"), BaseUnit::Gill);
    }

    #[test]
    fn test_gills_plural() {
        assert_eq!(base("gills"), BaseUnit::Gill);
    }

    #[test]
    fn test_gi_uk_symbol() {
        assert_eq!(base("gi_uk"), BaseUnit::GillUk);
    }

    #[test]
    fn test_gill_uk_long() {
        assert_eq!(base("gill_uk"), BaseUnit::GillUk);
    }

    #[test]
    fn test_gills_uk_plural() {
        assert_eq!(base("gills_uk"), BaseUnit::GillUk);
    }
}

/// A prefix may be written without the '~' separator: "kg" == "k~g". The
/// compact spelling is an input alias only: it parses to the same unit and
/// serialises back to the separated canonical form.
mod compact_prefix_form {
    use super::*;

    #[test]
    fn test_compact_equals_separated() {
        for (compact, separated) in [
            ("kg", "k~g"),
            ("km", "k~m"),
            ("ms", "m~s"),
            ("us", "u~s"),
            ("µs", "µ~s"),
            ("MHz", "M~Hz"),
            ("hPa", "h~Pa"),
            ("mmol", "m~mol"),
            ("mL", "m~L"),
            ("MeV", "M~eV"),
            ("Mpc", "M~pc"),
            ("KiB", "Ki~B"),
            ("MiB", "Mi~B"),
            ("Gbit", "G~bit"),
            ("km/h", "k~m/h"),
            ("kg·m/s²", "k~g·m/s²"),
            ("k$USD", "k~$USD"), // currencies too: the '$' sigil separates
            ("M$EUR", "M~$EUR"),
            ("G$ETH", "G~$ETH"),
        ] {
            let (a, b) = (p(compact), p(separated));
            assert_eq!(a.num_components(), b.num_components(), "{compact}");
            for (ca, cb) in a.components().iter().zip(b.components()) {
                assert_eq!(ca.base, cb.base, "{compact}");
                assert_eq!(ca.prefix_system(), cb.prefix_system(), "{compact}");
                assert_eq!(ca.prefix, cb.prefix, "{compact}");
                assert_eq!(ca.exponent, cb.exponent, "{compact}");
            }
            assert_eq!(unit_prefix_factor(&a), unit_prefix_factor(&b), "{compact}");
        }
    }

    #[test]
    fn test_serialises_back_to_the_separated_form() {
        assert_eq!(unit_to_str(&p("kg")).unwrap(), "k~g");
        assert_eq!(unit_to_str(&p("MiB")).unwrap(), "Mi~B");
    }

    #[test]
    fn test_bare_alias_wins() {
        for (tok, base) in [
            ("min", BaseUnit::Minute),             // not milli-inch
            ("cd", BaseUnit::Candela),             // not centi-day
            ("ft", BaseUnit::Foot),                // not femto-tonne
            ("at", BaseUnit::AtmosphereTechnical), // not atto-tonne
            ("au", BaseUnit::AstronomicalUnit),    // not atto-dalton
            ("PS", BaseUnit::MetricHorsepower),    // not peta-siemens
        ] {
            let c = c0(tok);
            assert_eq!(c.base, base, "{tok}");
            assert_eq!(c.prefix, SIPrefix::None, "{tok}");
        }
    }

    #[test]
    fn test_rejected() {
        for tok in [
            "kkg", "k~kg", // a prefix cannot be stacked
            "k~~g", "kg~", // malformed separated form, not a compact one
            "Kim", "mB", // prefix policy still applies
            "usb", "kt", // refused by name (units.bvnr .compact_exceptions)
            "kWh", "mcg", // never were units, still are not
            "kUSD", "k$XYZ", // the currency sigil and a known code stay required
            "Ki$USD", "kg$USD",
        ] {
            assert!(is_arg_err(parse_unit(tok)), "{tok}");
        }
    }

    #[test]
    fn test_refusal_is_per_spelling() {
        for (tok, base, prefix) in [
            ("p~H", BaseUnit::Henry, SIPrefix::Pico),
            ("m~ph", BaseUnit::Phot, SIPrefix::Milli),
            ("nH", BaseUnit::Henry, SIPrefix::Nano),
            ("u~sb", BaseUnit::Stilb, SIPrefix::Micro),
            ("k~t", BaseUnit::Tonne, SIPrefix::Kilo),
        ] {
            let c = c0(tok);
            assert_eq!(c.base, base, "{tok}");
            assert_eq!(c.prefix, prefix, "{tok}");
        }
    }

    #[test]
    fn test_tokens_that_became_real_units() {
        // Refused compact spellings until the units they name were added;
        // as bare aliases now, the bare-alias-wins rule keeps them unprefixed.
        for (tok, base) in [
            ("pH", BaseUnit::PhScale),      // acidity, not picohenry
            ("mph", BaseUnit::MilePerHour), // speed, not milliphot
            ("kph", BaseUnit::KilometerPerHour),
            ("kmh", BaseUnit::KilometerPerHour),
            ("ph", BaseUnit::Phot), // still the phot: case matters
            ("kn", BaseUnit::Knot),
        ] {
            let c = c0(tok);
            assert_eq!(c.base, base, "{tok}");
            assert_eq!(c.prefix, SIPrefix::None, "{tok}");
        }
    }

    #[test]
    fn test_the_new_units_take_no_prefix() {
        for tok in ["m~pH", "k~pH", "m~mph", "k~kph"] {
            assert!(is_arg_err(parse_unit(tok)), "{tok}");
        }
    }
}

/// Conductivity, dissolved solids, hardness and the instrument scales.
mod water_analysis_units {
    use super::*;

    #[test]
    fn test_spelling_resolves() {
        for (tok, canonical) in [
            ("µS/cm", "µ~S/c~m"), // EC, the everyday spelling
            ("uS/cm", "µ~S/c~m"),
            ("mS/cm", "m~S/c~m"),
            ("dS/m", "d~S/m"),      // soil salinity
            ("µmho/cm", "µ~S/c~m"), // pre-SI name for the same unit
            ("mho", "S"),
            ("℧", "S"),
            ("mg/L", "m~g/L"), // TDS
            ("mg/l", "m~g/L"),
            ("meq/L", "m~val/L"), // English spelling of mval/L
            ("mval/l", "m~val/L"),
            ("gpg", "gpg"),
            ("°dH", "°dH"),
            ("CF", "CF"),
            ("mNTU", "m~NTU"),
        ] {
            assert_eq!(unit_to_str(&p(tok)).unwrap(), canonical, "{tok}");
        }
    }

    #[test]
    fn test_conductivity_is_a_compound_not_a_unit() {
        let ec = p("µS/cm");
        assert_eq!(ec.num_components(), 2);
        assert_eq!(ec.components()[0].base, BaseUnit::Siemens);
        assert_eq!(ec.components()[1].base, BaseUnit::Meter);
        assert!(units_compatible(&ec, &p("S/m")));
    }

    #[test]
    fn test_hardness_scales_interconvert() {
        let mmol = p("m~mol/L");
        for (tok, per_mmol) in [
            ("°dH", 5.6077),
            ("°e", 7.0217),
            ("°fH", 10.0086),
            ("°rH", 40.078),
            ("°aH", 100.086),
            ("gpg", 5.8468),
            ("m~val/L", 2.0),
        ] {
            let f = unit_convert_factor(&mmol, &p(tok)).unwrap().factor;
            assert!(
                (f - per_mmol).abs() <= per_mmol * 2e-4,
                "{tok} {f} {per_mmol}"
            );
        }
    }

    #[test]
    fn test_conductivity_factor_is_a_rescaled_ec() {
        let cf = p("CF");
        assert!((unit_convert_factor(&cf, &p("m~S/c~m")).unwrap().factor - 0.1).abs() < 1e-12);
        assert!((unit_convert_factor(&cf, &p("µ~S/c~m")).unwrap().factor - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_gpg_is_not_the_compound_grain_per_gallon() {
        assert!(!units_compatible(&p("gpg"), &p("gr/gal")));
    }

    #[test]
    fn test_instrument_scales_do_not_interconvert() {
        for (a, b) in [
            ("NTU", "FNU"),
            ("NTU", "FTU"),
            ("NTU", "FAU"),
            ("NTU", "JTU"),
            ("FNU", "FTU"),
            ("FNU", "FAU"),
            ("FNU", "JTU"),
            ("FTU", "FAU"),
            ("FTU", "JTU"),
            ("FAU", "JTU"),
            ("NTU", "no_unit"),
            ("JTU", "%"),
            ("PSU", "‰"),
            ("PSU", "g/k~g"),
            ("PSU", "NTU"),
        ] {
            assert!(!units_compatible(&p(a), &p(b)), "{a} {b}");
        }
    }

    #[test]
    fn test_each_scale_converts_to_itself() {
        for tok in ["NTU", "FNU", "FTU", "FAU", "JTU", "PSU"] {
            let u = p(tok);
            assert!(units_compatible(&u, &u), "{tok}");
            assert_eq!(unit_convert_factor(&u, &u).unwrap().factor, 1.0, "{tok}");
        }
    }

    #[test]
    fn test_lowercase_neighbours_keep_their_meaning() {
        for (tok, base, prefix) in [
            ("cF", BaseUnit::Farad, SIPrefix::Centi), // not CF
            ("fau", BaseUnit::AstronomicalUnit, SIPrefix::Femto), // not FAU
            ("dH", BaseUnit::Henry, SIPrefix::Deci),  // not °dH
        ] {
            let c = c0(tok);
            assert!(c.base == base && c.prefix == prefix, "{tok}");
        }
    }

    #[test]
    fn test_the_hardness_ppm_is_not_the_dimensionless_ppm() {
        assert!(!units_compatible(&p("°aH"), &p("ppm")));
    }
}

/// Sweeps src/gendata/units.bvnr, not just the hand-picked cases above, so
/// the shipped library and the build-time exception list cannot drift apart.
mod compact_prefix_form_against_the_table {
    use super::*;

    fn field<'v>(v: &'v Value, key: &str) -> &'v Value {
        let Value::Struct(fields) = v else {
            panic!("not a struct: {v:?}");
        };
        &fields
            .iter()
            .find(|(k, _)| k == key)
            .unwrap_or_else(|| panic!("no field {key}"))
            .1
    }

    fn items(v: &Value) -> &[Value] {
        match v {
            Value::Array(a) => a,
            other => panic!("not an array: {other:?}"),
        }
    }

    fn text(v: &Value) -> &str {
        match v {
            Value::String(s) => s,
            other => panic!("not a string: {other:?}"),
        }
    }

    fn gendata() -> Value {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/gendata/units.bvnr");
        bovnar::parse(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn test_every_bare_alias_still_parses_without_a_prefix() {
        let doc = gendata();
        let mut broken = Vec::new();
        for u in items(field(&doc, "units")) {
            let name = text(field(u, "name"));
            for alias in items(field(u, "aliases")).iter().map(text) {
                match parse_unit(alias) {
                    Err(_) => broken.push(format!("{alias} ({name}) no longer parses")),
                    Ok(vu) => {
                        let c = vu.components()[0];
                        if c.prefix != SIPrefix::None || c.prefix_system() != PrefixSystem::Si {
                            broken.push(format!("{alias} ({name}) parsed as a PREFIXED unit"));
                        }
                    }
                }
            }
        }
        assert!(
            broken.is_empty(),
            "compact form shadowed a bare alias: {broken:?}"
        );
    }

    #[test]
    fn test_every_compact_exception_is_refused() {
        for e in items(field(&gendata(), "compact_exceptions")) {
            let sp = text(field(e, "spelling"));
            assert!(is_arg_err(parse_unit(sp)), "{sp}");
        }
    }

    #[test]
    fn test_no_compact_exception_is_dead_weight() {
        // A listed spelling must be one the parser would otherwise accept:
        // inserting the '~' back must yield a real prefixed unit.
        let doc = gendata();
        let mut aliases: Vec<&str> = items(field(&doc, "units"))
            .iter()
            .flat_map(|u| items(field(u, "aliases")).iter().map(text))
            .collect();
        aliases.sort();
        aliases.dedup();
        aliases.sort_by_key(|a| std::cmp::Reverse(a.chars().count()));
        for e in items(field(&doc, "compact_exceptions")) {
            let sp = text(field(e, "spelling"));
            assert!(
                !aliases.contains(&sp),
                "{sp} is a bare alias; the compact path never runs for it"
            );
            let base = aliases
                .iter()
                .find(|a| sp.ends_with(**a) && sp != **a)
                .unwrap_or_else(|| panic!("{sp} ends in no known unit symbol"));
            let separated = format!("{}~{base}", &sp[..sp.len() - base.len()]);
            parse_unit(&separated).unwrap(); // fails if the entry blocks nothing
            assert!(
                !text(field(e, "why")).is_empty(),
                "{sp} has no stated reason"
            );
        }
    }
}

mod new_unit_prefix_factor_and_exponent {
    use super::*;

    #[test]
    fn test_kgf_prefix_factor_is_one() {
        assert!(close(unit_prefix_factor(&p("kgf")), 1.0));
    }

    #[test]
    fn test_rpm_prefix_factor_is_one() {
        assert!(close(unit_prefix_factor(&p("rpm")), 1.0));
    }

    #[test]
    fn test_kilo_kgf_prefix_factor_is_1000() {
        assert!(close(unit_prefix_factor(&p("k~kgf")), 1000.0));
    }

    #[test]
    fn test_kgf_prefix_exponent_is_zero() {
        assert_eq!(unit_prefix_exponent(&p("kgf")), 0);
    }

    #[test]
    fn test_kilo_kgf_prefix_exponent_is_3() {
        assert_eq!(unit_prefix_exponent(&p("k~kgf")), 3);
    }
}

mod new_unit_serialization {
    use super::*;

    fn roundtrip(sym: &str) {
        let vu1 = p(sym);
        let vu2 = p(&unit_to_str(&vu1).unwrap());
        assert_eq!(vu1.num_components(), vu2.num_components());
        assert_eq!(vu2.components()[0].base, vu1.components()[0].base);
    }

    #[test]
    fn test_roundtrip_kgf() {
        roundtrip("kgf");
    }

    #[test]
    fn test_roundtrip_inhg() {
        roundtrip("inHg");
    }

    #[test]
    fn test_roundtrip_rpm() {
        roundtrip("rpm");
    }

    #[test]
    fn test_roundtrip_ft_lb() {
        roundtrip("ft_lb");
    }

    #[test]
    fn test_roundtrip_dr() {
        roundtrip("dr");
    }

    #[test]
    fn test_roundtrip_dwt() {
        roundtrip("dwt");
    }

    #[test]
    fn test_roundtrip_ch() {
        roundtrip("ch");
    }

    #[test]
    fn test_roundtrip_rd() {
        roundtrip("rd");
    }

    #[test]
    fn test_roundtrip_gi() {
        roundtrip("gi");
    }

    #[test]
    fn test_roundtrip_gi_uk() {
        roundtrip("gi_uk");
    }

    #[test]
    fn test_a_long_unit_formats_at_all() {
        // The binding must not be narrower than the library: a legal
        // 32-component unit renders well past 256 bytes.
        let sym = [
            "da~ton_ref^-100",
            "da~cal_IT^100",
            "da~Btu_th^-100",
            "da~fath^100",
        ]
        .repeat(8)
        .join("·");
        let vu = p(&sym);
        assert_eq!(vu.num_components(), 32);

        let text = unit_to_str(&vu).unwrap();
        assert!(
            text.len() > 256,
            "the case is only interesting past the old 256-byte buffer"
        );
        assert!(text.len() <= UNIT_STRING_MAX);

        // ...and it is the same unit coming back.
        assert_eq!(unit_to_str(&p(&text)).unwrap(), text);

        // The other formatters share the buffer.
        let ascii_form = unit_to_str_ex(&vu, UnitFlags::ASCII_EXP).unwrap();
        assert!(ascii_form.len() > 256);
        assert_eq!(unit_to_str(&p(&ascii_form)).unwrap(), text);
    }
}

mod new_unit_compatibility {
    use super::*;

    fn compat(a: &str, b: &str) -> bool {
        units_compatible(&p(a), &p(b))
    }

    #[test]
    fn test_kgf_compatible_with_newton() {
        assert!(compat("kgf", "N"));
    }

    #[test]
    fn test_kgf_compatible_with_lbf() {
        assert!(compat("kgf", "lbf"));
    }

    #[test]
    fn test_kgf_compatible_with_dyn() {
        assert!(compat("kgf", "dyn"));
    }

    #[test]
    fn test_kgf_compatible_with_kip() {
        assert!(compat("kgf", "kip"));
    }

    #[test]
    fn test_inhg_compatible_with_pa() {
        assert!(compat("inHg", "Pa"));
    }

    #[test]
    fn test_inhg_compatible_with_atm() {
        assert!(compat("inHg", "atm"));
    }

    #[test]
    fn test_inhg_compatible_with_mmhg() {
        assert!(compat("inHg", "mmHg"));
    }

    #[test]
    fn test_inhg_compatible_with_psi() {
        assert!(compat("inHg", "psi"));
    }

    #[test]
    fn test_inhg_compatible_with_bar() {
        assert!(compat("inHg", "bar"));
    }

    #[test]
    fn test_rpm_compatible_with_hz() {
        assert!(compat("rpm", "Hz"));
    }

    #[test]
    fn test_rpm_compatible_with_bq() {
        assert!(compat("rpm", "Bq"));
    }

    #[test]
    fn test_ft_lb_compatible_with_joule() {
        assert!(compat("ft_lb", "J"));
    }

    #[test]
    fn test_ft_lb_compatible_with_cal() {
        assert!(compat("ft_lb", "cal"));
    }

    #[test]
    fn test_ft_lb_compatible_with_btu() {
        assert!(compat("ft_lb", "Btu"));
    }

    #[test]
    fn test_ft_lb_compatible_with_erg() {
        assert!(compat("ft_lb", "erg"));
    }

    #[test]
    fn test_ft_lb_compatible_with_ev() {
        assert!(compat("ft_lb", "eV"));
    }

    #[test]
    fn test_dr_compatible_with_oz() {
        assert!(compat("dr", "oz"));
    }

    #[test]
    fn test_dr_compatible_with_lb() {
        assert!(compat("dr", "lb"));
    }

    #[test]
    fn test_dr_compatible_with_gram() {
        assert!(compat("dr", "g"));
    }

    #[test]
    fn test_dwt_compatible_with_oz_t() {
        assert!(compat("dwt", "oz_t"));
    }

    #[test]
    fn test_dwt_compatible_with_lb() {
        assert!(compat("dwt", "lb"));
    }

    #[test]
    fn test_dwt_compatible_with_dr() {
        assert!(compat("dwt", "dr"));
    }

    #[test]
    fn test_ch_compatible_with_meter() {
        assert!(compat("ch", "m"));
    }

    #[test]
    fn test_ch_compatible_with_foot() {
        assert!(compat("ch", "ft"));
    }

    #[test]
    fn test_ch_compatible_with_mile() {
        assert!(compat("ch", "mi"));
    }

    #[test]
    fn test_rd_compatible_with_meter() {
        assert!(compat("rd", "m"));
    }

    #[test]
    fn test_rd_compatible_with_ch() {
        assert!(compat("rd", "ch"));
    }

    #[test]
    fn test_gi_compatible_with_fl_oz() {
        assert!(compat("gi", "fl_oz"));
    }

    #[test]
    fn test_gi_compatible_with_pint() {
        assert!(compat("gi", "pt"));
    }

    #[test]
    fn test_gi_compatible_with_liter() {
        assert!(compat("gi", "L"));
    }

    #[test]
    fn test_gi_uk_compatible_with_pt_uk() {
        assert!(compat("gi_uk", "pt_uk"));
    }

    #[test]
    fn test_gi_uk_compatible_with_gi() {
        assert!(compat("gi_uk", "gi"));
    }

    #[test]
    fn test_gi_uk_compatible_with_gal_uk() {
        assert!(compat("gi_uk", "gal_uk"));
    }
}

/// Lossless read-time unit/base conversion via the reader's want_unit hook.
mod reader_want_unit {
    use super::*;

    fn opts(allow_nonterminating: bool) -> ReadOptions {
        ReadOptions {
            want_unit_allow_nonterminating: allow_nonterminating,
            ..ReadOptions::default()
        }
    }

    /// Reads `doc` with `want` and returns what `on_data` collected from
    /// every data event.
    fn read_with<T>(
        doc: &[u8],
        allow_nonterminating: bool,
        mut want: impl FnMut(&EventData<'_>) -> Option<(Unit, u32)>,
        mut on_data: impl FnMut(&EventData<'_>) -> Option<T>,
    ) -> Result<Vec<T>> {
        let mut seen = Vec::new();
        let mut on_event = |d: &EventData<'_>| {
            if d.event == Event::Data
                && let Some(v) = on_data(d)
            {
                seen.push(v);
            }
            true
        };
        Reader::new()?.read_mem_want_unit(
            doc,
            &opts(allow_nonterminating),
            &mut want,
            Some(&mut on_event),
        )?;
        Ok(seen)
    }

    /// The converted literal, or `None` when the conversion has no text.
    fn converted_str(d: &EventData<'_>) -> Option<String> {
        (d.converted && !d.converted_text.is_empty())
            .then(|| String::from_utf8(d.converted_text.to_vec()).unwrap())
    }

    fn converted_strs(doc: &[u8], unit: Unit, base: Option<u32>) -> Vec<String> {
        read_with(
            doc,
            false,
            |_| Some((unit, base.unwrap_or(0))),
            |d| d.converted.then(|| converted_str(d).unwrap()),
        )
        .unwrap()
    }

    /// The `converted` flag of every non-key data event.
    fn converted_flags(
        doc: &[u8],
        want: impl FnMut(&EventData<'_>) -> Option<(Unit, u32)>,
    ) -> Vec<bool> {
        read_with(doc, false, want, |d| {
            (d.token_type != Some(TokenType::Identifier)).then_some(d.converted)
        })
        .unwrap()
    }

    fn error_code(doc: &[u8], unit: Unit, base: u32, allow: bool) -> Option<ErrorCode> {
        let err = read_with(doc, allow, |_| Some((unit, base)), |_| Some(())).unwrap_err();
        assert!(matches!(err, Error::Parse { .. }), "{err:?}");
        err.code()
    }

    #[test]
    fn test_multiplicative_conversion() {
        let seen = converted_strs(b".d = 5 k~m;", si(BaseUnit::Meter), None);
        assert_eq!(seen, ["5000"]); // exact
    }

    #[test]
    fn test_affine_conversion() {
        let seen = converted_strs(".t = 25 °C;".as_bytes(), si(BaseUnit::Kelvin), None);
        assert_eq!(seen, ["298.15"]); // exact
    }

    #[test]
    fn test_incompatible_is_unit_mismatch() {
        let code = error_code(b".d = 5 k~m;", si(BaseUnit::Second), 0, false);
        assert_eq!(code, Some(ErrorCode::UnitMismatch));
    }

    #[test]
    fn test_irrational_is_unit_inexact() {
        // degree -> radian is pi-based
        let code = error_code(".a = 90 °;".as_bytes(), si(BaseUnit::Radian), 0, false);
        assert_eq!(code, Some(ErrorCode::UnitInexact));
    }

    #[test]
    fn test_decline_leaves_value_native() {
        let flags = converted_flags(b".d = 5 k~m;", |_| None);
        assert!(!flags.is_empty() && !flags.iter().any(|&f| f));
    }

    #[test]
    fn test_pure_base_conversion() {
        // Same unit, different output base.
        let seen = read_with(
            b".n = 255;",
            false,
            |d| Some((d.value_unit, 16)),
            |d| d.converted.then(|| (converted_str(d), d.converted_base)),
        )
        .unwrap();
        assert_eq!(seen, [(Some("ff".to_string()), 16)]);
    }

    #[test]
    fn test_non_decimal_base_string_carrier_converts() {
        // A non-decimal base is written as a string literal but still has a
        // numeric family, so it must convert.
        let seen = converted_strs(
            b".x = <uint:32,_16> \"FF\" k~m;",
            si(BaseUnit::Meter),
            Some(10),
        );
        assert_eq!(seen, ["255000"]); // 0xFF k~m -> 255000 m, exact
    }

    #[test]
    fn test_real_string_never_converted() {
        let flags = converted_flags(b".s = \"hello\";", |_| Some((si(BaseUnit::Meter), 0)));
        assert!(!flags.is_empty() && !flags.iter().any(|&f| f));
    }

    #[test]
    fn test_multiprecision_integer_converts_losslessly() {
        // A >64-bit integer converts exactly: uint128 max times 1000.
        let big = b"340282366920938463463374607431768211455";
        let doc = [b".x = <uint:128> ".as_slice(), big, b" k~m;"].concat();
        let seen = read_with(
            &doc,
            false,
            |_| Some((si(BaseUnit::Meter), 10)),
            |d| {
                d.converted
                    .then(|| (converted_str(d).unwrap(), d.raw.to_vec()))
            },
        )
        .unwrap();
        assert_eq!(seen.len(), 1);
        let (val, raw) = &seen[0];
        assert_eq!(val, "340282366920938463463374607431768211455000"); // exact x1000
        assert_eq!(raw, big); // exact source preserved
    }

    #[test]
    fn test_wide_float_converts_losslessly() {
        // A 1056-bit binary float converts exactly: 70 significant digits.
        let digits = "1.234567890123456789012345678901234567890123456789012345678901234567891";
        let doc = format!(".d = <float:1056> {digits} k~m;");
        let seen = converted_strs(doc.as_bytes(), si(BaseUnit::Meter), Some(10));
        assert_eq!(
            seen,
            ["1234.567890123456789012345678901234567890123456789012345678901234567891"]
        );
    }

    #[test]
    fn test_high_output_bases() {
        // Any base bvnr can write is a valid output base, not just 2..36.
        for (base, expect) in [(36, "73"), (62, "47"), (64, "D/"), (85, "$!")] {
            let seen = read_with(
                b".n = 255;",
                false,
                |_| Some((Unit::dimensionless(), base)),
                |d| d.converted.then(|| (converted_str(d), d.converted_base)),
            )
            .unwrap();
            assert_eq!(seen, [(Some(expect.to_string()), base)]);
        }
    }

    #[test]
    fn test_native_high_base_is_kept() {
        // base 0 keeps the value's own base, including one above 36.
        let seen = read_with(
            b".x = <uint:64,_62> \"zz\" m;",
            false,
            |_| Some((si(BaseUnit::Meter), 0)),
            |d| d.converted.then(|| (converted_str(d), d.converted_base)),
        )
        .unwrap();
        assert_eq!(seen, [(Some("zz".to_string()), 62)]);
    }

    #[test]
    fn test_unusable_output_base_is_error() {
        // 63 is not a base bvnr writes.
        let code = error_code(b".n = 255;", Unit::dimensionless(), 63, false);
        assert_eq!(code, Some(ErrorCode::InvalidArgument));
    }

    #[test]
    fn test_unrepresentable_value_is_error() {
        let code = error_code(
            b".d = <float:64> 1e1000000 k~m;",
            si(BaseUnit::Meter),
            0,
            false,
        );
        assert_eq!(code, Some(ErrorCode::ValueOutOfRange));
    }

    #[test]
    fn test_special_floats_pass_through() {
        // nan/inf carry no finite value: nothing to convert, and no error.
        for lit in ["nan", "inf", "ninf"] {
            let doc = format!(".d = <float:64,k~m> {lit};");
            let flags = converted_flags(doc.as_bytes(), |_| Some((si(BaseUnit::Meter), 0)));
            assert!(!flags.is_empty() && !flags.iter().any(|&f| f), "{lit}");
        }
    }

    #[test]
    fn test_nonterminating_refused_by_default() {
        // 1 m -> mile is 125/201168: exact as a rational, no finite decimal.
        let code = error_code(b".d = 1 m;", si(BaseUnit::Mile), 10, false);
        assert_eq!(code, Some(ErrorCode::UnitInexact));
    }

    #[test]
    fn test_nonterminating_opt_in_delivers_rational() {
        let seen = read_with(
            b".d = 1 m;",
            true,
            |_| Some((si(BaseUnit::Mile), 10)),
            |d| d.converted.then(|| (converted_str(d), d.converted_base)),
        )
        .unwrap();
        assert_eq!(seen, [(None, 10)]);
    }

    #[test]
    fn test_irrational_still_refused_with_opt_in() {
        let code = error_code(".a = 90 °;".as_bytes(), si(BaseUnit::Radian), 0, true);
        assert_eq!(code, Some(ErrorCode::UnitInexact));
    }

    #[test]
    fn test_corrected_exact_factors() {
        // 1 Torr = 101325/760 Pa, so 760 Torr is exactly 101325 Pa.
        let seen = converted_strs(".p = 760 Torr;".as_bytes(), si(BaseUnit::Pascal), Some(10));
        assert_eq!(seen, ["101325"]);
    }

    #[test]
    fn test_nonterminating_factor_now_refuses() {
        // 1 rpm = 1/60 Hz has no finite decimal.
        let code = error_code(b".f = 1 rpm;", si(BaseUnit::Hertz), 10, false);
        assert_eq!(code, Some(ErrorCode::UnitInexact));
    }

    #[test]
    fn test_callback_data_survives_the_reader() {
        // Events kept from both callbacks stay usable after the reader is
        // gone: in Rust that means owned snapshots.
        let mut kept = Vec::new();
        {
            let kept_cell = std::cell::RefCell::new(&mut kept);
            let mut want = |d: &EventData<'_>| {
                kept_cell.borrow_mut().push(bovnar::EventPayload::from(d));
                Some((si(BaseUnit::Meter), 0))
            };
            let mut on_event = |d: &EventData<'_>| {
                if d.event == Event::Data {
                    kept_cell.borrow_mut().push(bovnar::EventPayload::from(d));
                }
                true
            };
            Reader::new()
                .unwrap()
                .read_mem_want_unit(
                    b".d = 5 k~m;",
                    &ReadOptions::default(),
                    &mut want,
                    Some(&mut on_event),
                )
                .unwrap();
        }
        assert!(!kept.is_empty());
        for d in &kept {
            assert!(d.token_type.is_some());
            let _width: u32 = d.value_type.width;
        }
    }

    /// `(a, b)` reduced, from decimal strings.
    fn reduced(pair: &(String, String)) -> (u128, u128) {
        let (a, b): (u128, u128) = (pair.0.parse().unwrap(), pair.1.parse().unwrap());
        let (mut x, mut y) = (a, b);
        while y != 0 {
            (x, y) = (y, x % y);
        }
        (a / x, b / x)
    }

    #[test]
    fn test_converted_rational_exposes_the_exact_value() {
        let seen = read_with(
            b".d = 1 m;",
            true,
            |_| Some((si(BaseUnit::Mile), 10)),
            |d| {
                d.converted
                    .then(|| (converted_str(d), d.converted_rational()))
            },
        )
        .unwrap();
        let (text, rat) = &seen[0];
        assert_eq!(*text, None); // no finite decimal expansion
        assert_eq!(
            reduced(rat.as_ref().unwrap()),
            reduced(&("1000".into(), "1609344".into()))
        );
    }

    #[test]
    fn test_converted_in_base_rerenders() {
        let seen = read_with(
            b".d = 5 k~m;",
            false,
            |_| Some((si(BaseUnit::Meter), 10)),
            |d| {
                d.converted.then(|| {
                    (
                        converted_str(d),
                        d.converted_in_base(2).unwrap(),
                        d.converted_in_base(16).unwrap(),
                        d.converted_rational(),
                    )
                })
            },
        )
        .unwrap();
        let (dec, b2, b16, rat) = &seen[0];
        assert_eq!(dec.as_deref(), Some("5000"));
        assert_eq!(b2.clone(), Some(format!("{:b}", 5000)));
        assert_eq!(b16.clone(), Some(format!("{:x}", 5000)));
        assert_eq!(rat.clone(), Some(("5000".to_string(), "1".to_string())));
    }

    #[test]
    fn test_read_file_forwards_every_option() {
        // read_file is read_reader_want_unit over the opened File, which
        // must forward the want_unit hook and strict_version.
        let path =
            std::env::temp_dir().join(format!("bovnar-test-units-{}.bvnr", std::process::id()));
        std::fs::write(&path, b".d = 5 k~m;").unwrap();
        let file = std::fs::File::open(&path).unwrap();

        let mut seen = Vec::new();
        let mut want = |_: &EventData<'_>| Some((si(BaseUnit::Meter), 0));
        let mut on_event = |d: &EventData<'_>| {
            if d.event == Event::Data && d.converted {
                seen.push(converted_str(d).unwrap());
            }
            true
        };
        let options = ReadOptions {
            strict_version: false,
            ..ReadOptions::default()
        };
        Reader::new()
            .unwrap()
            .read_reader_want_unit(file, &options, &mut want, Some(&mut on_event))
            .unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(seen, ["5000"]);
    }
}

/// Property test: an identity conversion must never change a document. Run
/// over the example corpus with and without the hook, the event streams must
/// match and every converted value must equal the literal it came from.
mod want_unit_over_the_example_corpus {
    use super::*;

    /// A non-negative big integer, little-endian base 2^32 limbs.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Big(Vec<u32>);

    impl Big {
        fn small(n: u32) -> Big {
            Big(vec![n]).trim()
        }

        fn trim(mut self) -> Big {
            while self.0.last() == Some(&0) {
                self.0.pop();
            }
            self
        }

        fn mul_add_small(&self, m: u32, a: u32) -> Big {
            let mut carry = a as u64;
            let mut out = Vec::with_capacity(self.0.len() + 1);
            for &l in &self.0 {
                let v = l as u64 * m as u64 + carry;
                out.push(v as u32);
                carry = v >> 32;
            }
            out.push(carry as u32);
            Big(out).trim()
        }

        fn add(&self, o: &Big) -> Big {
            let mut out = Vec::new();
            let mut carry = 0u64;
            for i in 0..self.0.len().max(o.0.len()) {
                let v =
                    *self.0.get(i).unwrap_or(&0) as u64 + *o.0.get(i).unwrap_or(&0) as u64 + carry;
                out.push(v as u32);
                carry = v >> 32;
            }
            out.push(carry as u32);
            Big(out).trim()
        }

        fn mul(&self, o: &Big) -> Big {
            let mut out = vec![0u64; self.0.len() + o.0.len() + 1];
            for (i, &a) in self.0.iter().enumerate() {
                let mut carry = 0u64;
                for (j, &b) in o.0.iter().enumerate() {
                    let v = out[i + j] + a as u64 * b as u64 + carry;
                    out[i + j] = v & 0xffff_ffff;
                    carry = v >> 32;
                }
                let mut k = i + o.0.len();
                while carry != 0 {
                    let v = out[k] + carry;
                    out[k] = v & 0xffff_ffff;
                    carry = v >> 32;
                    k += 1;
                }
            }
            Big(out.into_iter().map(|v| v as u32).collect()).trim()
        }

        fn pow(base: u32, exp: u32) -> Big {
            (0..exp).fold(Big::small(1), |acc, _| acc.mul_add_small(base, 0))
        }

        fn decimal(s: &str) -> Big {
            s.bytes().fold(Big::small(0), |acc, c| {
                acc.mul_add_small(10, (c - b'0') as u32)
            })
        }

        fn is_zero(&self) -> bool {
            self.0.is_empty()
        }
    }

    /// A rational: sign, numerator, denominator (not necessarily reduced).
    #[derive(Debug)]
    struct Frac(bool, Big, Big);

    impl Frac {
        fn same_value(&self, o: &Frac) -> bool {
            let zero = self.1.is_zero() && o.1.is_zero();
            (zero || self.0 == o.0) && self.1.mul(&o.2) == o.1.mul(&self.2)
        }
    }

    fn rational_frac(pair: &(String, String)) -> Frac {
        let (num, den) = pair;
        let neg = num.starts_with('-') != den.starts_with('-');
        Frac(
            neg,
            Big::decimal(num.trim_start_matches('-')),
            Big::decimal(den.trim_start_matches('-')),
        )
    }

    /// Mirror of C `bvn_effective_base`: float_fix, float_dec and datetime
    /// literals are always decimal.
    fn effective_base(vt: ValueTypeSpec) -> u32 {
        let fam = ValueTypeFamily::from_raw(vt.family);
        if matches!(
            fam,
            Some(ValueTypeFamily::FloatFix | ValueTypeFamily::FloatDec | ValueTypeFamily::Datetime)
        ) {
            return 10;
        }
        if vt.base == 0 { 10 } else { vt.base }
    }

    /// Exact value of a bovnar numeric literal.
    fn literal_fraction(lit: &str, base: u32) -> Frac {
        let mut t = lit.trim();
        let neg = t.starts_with('-');
        if t.starts_with('+') || t.starts_with('-') {
            t = &t[1..];
        }
        let mut exp: i64 = 0;
        let markers: &[char] = if base == 16 {
            &['p', 'P']
        } else if base <= 14 {
            &['e', 'E']
        } else {
            &[]
        };
        for &m in markers {
            if let Some((mantissa, e)) = t.split_once(m) {
                exp = e.parse().unwrap();
                t = mantissa;
                break;
            }
        }
        let (ip, fp) = t.split_once('.').unwrap_or((t, ""));
        // Digit alphabets, mirroring bvn_char_to_digit.
        let digits: String = match base {
            64 => "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".into(),
            85 => (0..85u8).map(|i| (33 + i) as char).collect(),
            b if b <= 36 => "0123456789abcdefghijklmnopqrstuvwxyz".into(),
            _ => "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".into(),
        };
        let val = |s: &str| {
            let s = if base <= 36 {
                s.to_lowercase()
            } else {
                s.to_string()
            };
            s.chars().fold(Big::small(0), |acc, c| {
                let d = digits
                    .find(c)
                    .unwrap_or_else(|| panic!("digit {c:?} in {lit:?}"));
                acc.mul_add_small(base, d as u32)
            })
        };
        let ip = if ip.is_empty() { "0" } else { ip };
        let scale = Big::pow(base, fp.chars().count() as u32);
        let mut num = val(ip).mul(&scale).add(&val(fp));
        let mut den = scale;
        let radix = if base == 16 && exp != 0 { 2 } else { base };
        let factor = Big::pow(radix, exp.unsigned_abs() as u32);
        if exp >= 0 {
            num = num.mul(&factor);
        } else {
            den = den.mul(&factor);
        }
        Frac(neg, num, den)
    }

    type StreamItem = (Event, Option<TokenType>, Vec<u8>);
    type ConvItem = (Vec<u8>, u32, Option<String>, u32, Option<(String, String)>);

    /// The event stream and the converted values; `base` `None` reads
    /// without a hook.
    fn read(
        doc: &[u8],
        base: Option<u32>,
        allow: bool,
    ) -> Result<(Vec<StreamItem>, Vec<ConvItem>)> {
        let mut stream = Vec::new();
        let mut conv = Vec::new();
        let mut on_event = |d: &EventData<'_>| {
            stream.push((d.event, d.token_type, d.raw.to_vec()));
            if d.converted {
                conv.push((
                    d.raw.to_vec(),
                    effective_base(d.value_type),
                    (!d.converted_text.is_empty())
                        .then(|| String::from_utf8_lossy(d.converted_text).into_owned()),
                    d.converted_base,
                    d.converted_rational(),
                ));
            }
            true
        };
        let mut reader = Reader::new()?;
        match base {
            None => reader.read_mem(doc, &ReadOptions::default(), Some(&mut on_event), None)?,
            Some(b) => {
                let options = ReadOptions {
                    want_unit_allow_nonterminating: allow,
                    ..ReadOptions::default()
                };
                let mut want = |d: &EventData<'_>| Some((d.value_unit, b));
                reader.read_mem_want_unit(doc, &options, &mut want, Some(&mut on_event))?
            }
        }
        Ok((stream, conv))
    }

    fn examples() -> Vec<std::path::PathBuf> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples");
        let mut out: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "bvnr"))
            .collect();
        out.sort();
        out
    }

    #[test]
    fn test_identity_hook_does_not_perturb_the_corpus() {
        for base in [0, 10, 16, 2, 36, 62] {
            let mut checked = 0;
            for path in examples() {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                let doc = std::fs::read(&path).unwrap();
                let (plain, _) = read(&doc, None, false).unwrap();
                let (hooked, conv) = match read(&doc, Some(base), false) {
                    Ok(r) => r,
                    Err(e) => {
                        // A decimal fraction has no finite expansion in a base
                        // without a factor of 5, so a strict run may stop; the
                        // opt-in must then carry the same stream through.
                        assert_eq!(e.code(), Some(ErrorCode::UnitInexact), "{name} {base}");
                        read(&doc, Some(base), true).unwrap()
                    }
                };
                assert!(
                    hooked == plain,
                    "{name}: identity hook changed the stream (base {base})"
                );

                for (raw, src_base, _text, _out_base, rational) in conv {
                    let rational =
                        rational.unwrap_or_else(|| panic!("{name}: no rational for {raw:?}"));
                    let lit = String::from_utf8_lossy(&raw).into_owned();
                    if ["nan", "inf", "ninf"].contains(&lit.to_lowercase().as_str()) {
                        continue;
                    }
                    assert!(
                        rational_frac(&rational).same_value(&literal_fraction(&lit, src_base)),
                        "{name}: identity changed {lit:?} -> {rational:?}"
                    );
                    checked += 1;
                }
            }
            assert!(checked > 0, "the corpus produced no conversions to check");
        }
    }
}

/// Which unit profiles this build carries, and how a missing one reports.
mod available_profiles_tests {
    use super::*;

    #[test]
    fn test_reports_the_names_the_library_reports() {
        let names = available_profiles();
        // No duplicates and no holes.
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), names.len());
    }

    #[test]
    fn test_every_reported_profile_actually_parses() {
        let probe = [
            ("ucum", "ucum:m"),
            ("unece", "unece:MTR"),
            ("qudt", "qudt:M"),
            ("qudt-qk", "qudt-qk:Length"),
            ("udunits", "udunits:m"),
            ("om", "om:metre"),
            ("cf", "cf:air_temperature"),
        ];
        for ns in available_profiles() {
            let code = probe
                .iter()
                .find(|(n, _)| *n == ns)
                .unwrap_or_else(|| panic!("unknown profile {ns:?}; extend this probe"))
                .1;
            parse_unit(code).unwrap(); // fails if the build lacks it
        }
    }

    #[test]
    fn test_a_namespace_not_reported_is_unknown_not_illegal() {
        assert!(!available_profiles().iter().any(|n| n == "nosuch"));
        assert_eq!(
            ErrorCode::from_raw(unit_error_code("nosuch:m")),
            Some(ErrorCode::UnitProfileUnknown)
        );
        // ...against a bad code inside a namespace the build does have.
        if available_profiles().iter().any(|n| n == "ucum") {
            assert_eq!(
                ErrorCode::from_raw(unit_error_code("ucum:notacode")),
                Some(ErrorCode::UnitIllegal)
            );
        }
    }
}
