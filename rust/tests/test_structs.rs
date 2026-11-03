//! Port of python/tests/test_structs.py, as far as the public API reaches.
//!
//! Python's `structs.py` (the ctypes mirrors and their `make_*` helpers) is
//! the private `ffi` module here; its layouts are checked against the C
//! compiler by the test_abi tests in src/ffi.rs (`abi_tests`). The public
//! pieces are `ValueTypeSpec` (built with a struct literal) and `Unit`
//! (which wraps `value_unit_t`).
//!
//! Python's `make_type_spec` is a `ValueTypeSpec` struct literal here,
//! `make_unit_none` / `make_unit_dimensionless` / `make_unit_si` /
//! `make_unit_iec` / `make_unit_compound` are `Unit::default` /
//! `Unit::dimensionless` / `Unit::si` / `Unit::iec` / `Unit::compound`, a
//! `ValueUnitComponent` is a `UnitComponent`, and `BvnrReadFlags` is
//! `ReadOptions`.
//!
//! Skipped (the C struct mirror or helper is private, and these tests check
//! its raw ctypes form, which has no public Rust counterpart):
//! - TestConstants: test_opaque_bytes (the opaque blob size is private)
//! - TestValueUnitComponent: test_size_reasonable (ctypes size),
//!   test_zero_init (`UnitComponent` has no zeroed form: a struct literal
//!   names every field), test_union_overlap (the raw prefix-id C union),
//!   test_repr (Python `repr`)
//! - TestValueUnit: test_size (ctypes size), test_active_components (writes
//!   the raw `num_components` field; `Unit::compound` sets it, see
//!   TestMakeUnitCompound::test_active_components_matches_num)
//! - TestValueTypeSpec: test_repr (Python's repr names the family; the Rust
//!   `Debug` prints the raw field values)
//! - TestBvnrData: test_zero_init, test_raw_bytes, test_raw_bytes_empty,
//!   test_repr, test_has_spec_1_1_frac_fields,
//!   test_has_spec_1_2_unit_source_fields (`BvnrData` is private; its field
//!   order is checked by the test_abi tests in src/ffi.rs)
//! - TestOpaqueTypes: test_source_size, test_sink_size,
//!   test_source_zero_init, test_sink_zero_init (private opaque blobs)
//! - TestBvnrReadFlags: test_size_reasonable (ctypes size)
//! - TestBvnrWriteFlags: test_zero_init (private; the writer takes no flags)
//! - TestMakeDataKey: test_ascii, test_unicode, test_data_pointer_valid
//! - TestMakeDataTyped: test_returns_bvnr_data (`make_data_*` build the
//!   private `BvnrData`)

use bovnar::{
    BaseUnit, Exponent, IECPrefix, MAX_UNIT_COMPONENTS, PrefixSystem, ReadOptions, SIPrefix, Unit,
    UnitComponent, ValueTypeFamily, ValueTypeSpec,
};

/// A compound component: `base` with an SI prefix and exponent.
fn comp(base: BaseUnit, prefix: SIPrefix, exponent: Exponent) -> UnitComponent {
    UnitComponent::si(base, prefix, exponent)
}

/// `{'base': base}`: no prefix, linear.
fn plain(base: BaseUnit) -> UnitComponent {
    comp(base, SIPrefix::None, Exponent::LINEAR)
}

mod constants {
    use super::*;

    #[test]
    fn test_max_unit_components() {
        // Mirrors BVNR_MAX_UNIT_COMPONENTS in include/bovnar.h; the test_abi
        // tests prove the two agree by comparing struct sizes.
        assert_eq!(MAX_UNIT_COMPONENTS, 32);
    }
}

mod value_unit_component {
    use super::*;

    #[test]
    fn test_set_fields() {
        let c = comp(BaseUnit::Meter, SIPrefix::Kilo, Exponent::SQUARE);
        assert_eq!(c.base, BaseUnit::Meter);
        assert_eq!(c.exponent, Exponent::SQUARE);
        assert_eq!(c.prefix_system(), PrefixSystem::Si);
        assert_eq!(c.prefix, SIPrefix::Kilo);
    }

    #[test]
    fn test_iec_prefix() {
        let c = UnitComponent::iec(BaseUnit::Byte, IECPrefix::Gibi, Exponent::LINEAR);
        assert_eq!(c.prefix_system(), PrefixSystem::Iec);
        assert_eq!(c.prefix, IECPrefix::Gibi);
    }
}

mod value_unit {
    use super::*;

    #[test]
    fn test_zero_init() {
        let vu = Unit::default();
        assert_eq!(vu.num_components(), 0);
        assert!(vu.is_dimensionless());
    }

    #[test]
    fn test_dimensionless_single_none() {
        let vu = Unit::compound(&[plain(BaseUnit::None)]).unwrap();
        assert_eq!(vu.num_components(), 1);
        assert!(vu.is_dimensionless());
    }

    #[test]
    fn test_not_dimensionless() {
        let vu = Unit::compound(&[plain(BaseUnit::Meter)]).unwrap();
        assert!(!vu.is_dimensionless());
    }
}

mod value_type_spec {
    use super::*;

    #[test]
    fn test_zero_init() {
        let vt = ValueTypeSpec::default();
        assert_eq!(vt.family, 0);
        assert_eq!(vt.width, 0);
        assert_eq!(vt.base, 0);
    }

    #[test]
    fn test_set_fields() {
        let vt = ValueTypeSpec {
            family: ValueTypeFamily::Uint as i32,
            width: 32,
            base: 10,
        };
        assert_eq!(
            ValueTypeFamily::from_raw(vt.family),
            Some(ValueTypeFamily::Uint)
        );
        assert_eq!(vt.width, 32);
    }
}

mod bvnr_read_flags {
    use super::*;

    #[test]
    fn test_zero_init() {
        let f = ReadOptions::default();
        assert_eq!(f.max_file_size, 0);
        assert!(!f.continue_on_error);
    }

    #[test]
    fn test_set_max_file_size() {
        let f = ReadOptions {
            max_file_size: 16 * 1024 * 1024,
            ..ReadOptions::default()
        };
        assert_eq!(f.max_file_size, 16 * 1024 * 1024);
    }
}

mod make_type_spec {
    use super::*;

    #[test]
    fn test_uint() {
        let vt = ValueTypeSpec {
            family: ValueTypeFamily::Uint as i32,
            width: 16,
            base: 10,
        };
        assert_eq!(
            ValueTypeFamily::from_raw(vt.family),
            Some(ValueTypeFamily::Uint)
        );
        assert_eq!(vt.width, 16);
        assert_eq!(vt.base, 10);
    }

    #[test]
    fn test_float() {
        let vt = ValueTypeSpec {
            family: ValueTypeFamily::Float as i32,
            width: 64,
            base: 0,
        };
        assert_eq!(
            ValueTypeFamily::from_raw(vt.family),
            Some(ValueTypeFamily::Float)
        );
    }

    #[test]
    fn test_defaults() {
        let vt = ValueTypeSpec {
            family: ValueTypeFamily::Utf8 as i32,
            ..ValueTypeSpec::default()
        };
        assert_eq!(vt.width, 0);
        assert_eq!(vt.base, 0);
    }
}

mod make_unit_helpers {
    use super::*;

    #[test]
    fn test_dimensionless() {
        let vu = Unit::dimensionless();
        assert_eq!(vu.num_components(), 1);
        assert_eq!(vu.components()[0].base, BaseUnit::None);
        assert!(vu.is_dimensionless());
    }

    #[test]
    fn test_unit_none() {
        let vu = Unit::default();
        assert_eq!(vu.num_components(), 0);
        assert!(vu.is_dimensionless());
    }

    #[test]
    fn test_unit_si_no_prefix() {
        let vu = Unit::si(BaseUnit::Meter, SIPrefix::None, Exponent::LINEAR);
        assert_eq!(vu.num_components(), 1);
        let c = vu.components()[0];
        assert_eq!(c.base, BaseUnit::Meter);
        assert_eq!(c.prefix_system(), PrefixSystem::Si);
        assert_eq!(c.prefix, SIPrefix::None);
    }

    #[test]
    fn test_unit_si_with_prefix() {
        let vu = Unit::si(BaseUnit::Gram, SIPrefix::Kilo, Exponent::LINEAR);
        assert_eq!(vu.components()[0].prefix, SIPrefix::Kilo);
        assert_eq!(vu.components()[0].base, BaseUnit::Gram);
    }

    #[test]
    fn test_unit_si_with_exponent() {
        let vu = Unit::si(BaseUnit::Meter, SIPrefix::None, Exponent::SQUARE);
        assert_eq!(vu.components()[0].exponent, Exponent::SQUARE);
    }

    #[test]
    fn test_unit_iec() {
        let vu = Unit::iec(BaseUnit::Byte, IECPrefix::Gibi, Exponent::LINEAR);
        assert_eq!(vu.num_components(), 1);
        let c = vu.components()[0];
        assert_eq!(c.base, BaseUnit::Byte);
        assert_eq!(c.prefix_system(), PrefixSystem::Iec);
        assert_eq!(c.prefix, IECPrefix::Gibi);
    }
}

mod make_unit_compound {
    use super::*;

    #[test]
    fn test_single_component_si() {
        let vu = Unit::compound(&[plain(BaseUnit::Meter)]).unwrap();
        assert_eq!(vu.num_components(), 1);
        let c = vu.components()[0];
        assert_eq!(c.base, BaseUnit::Meter);
        assert_eq!(c.exponent, Exponent::LINEAR);
        assert_eq!(c.prefix, SIPrefix::None);
    }

    #[test]
    fn test_single_component_with_prefix() {
        let vu =
            Unit::compound(&[comp(BaseUnit::Meter, SIPrefix::Kilo, Exponent::LINEAR)]).unwrap();
        assert_eq!(vu.components()[0].prefix, SIPrefix::Kilo);
    }

    #[test]
    fn test_two_components_m_per_s() {
        let vu = Unit::compound(&[
            comp(BaseUnit::Meter, SIPrefix::None, Exponent::LINEAR),
            comp(BaseUnit::Second, SIPrefix::None, Exponent::NEG_LINEAR),
        ])
        .unwrap();
        assert_eq!(vu.num_components(), 2);
        let c = vu.components();
        assert_eq!(c[0].base, BaseUnit::Meter);
        assert_eq!(c[1].base, BaseUnit::Second);
        assert_eq!(c[1].exponent, Exponent::NEG_LINEAR);
    }

    #[test]
    fn test_three_components_force() {
        let vu = Unit::compound(&[
            comp(BaseUnit::Gram, SIPrefix::Kilo, Exponent::LINEAR),
            comp(BaseUnit::Meter, SIPrefix::None, Exponent::LINEAR),
            comp(BaseUnit::Second, SIPrefix::None, Exponent::NEG_SQUARE),
        ])
        .unwrap();
        assert_eq!(vu.num_components(), 3);
        let c = vu.components();
        assert_eq!(c[0].prefix, SIPrefix::Kilo);
        assert_eq!(c[2].exponent, Exponent::NEG_SQUARE);
    }

    #[test]
    fn test_iec_prefix_component() {
        let vu = Unit::compound(&[UnitComponent::iec(
            BaseUnit::Byte,
            IECPrefix::Gibi,
            Exponent::LINEAR,
        )])
        .unwrap();
        let c = vu.components()[0];
        assert_eq!(c.prefix_system(), PrefixSystem::Iec);
        assert_eq!(c.prefix, IECPrefix::Gibi);
    }

    // Python's `make_unit_compound` fills in a linear exponent and no prefix
    // for a missing key; `plain` is that default spelled out.
    #[test]
    fn test_default_exp_is_linear() {
        let vu = Unit::compound(&[plain(BaseUnit::Meter)]).unwrap();
        assert_eq!(vu.components()[0].exponent, Exponent::LINEAR);
    }

    #[test]
    fn test_default_si_prefix_is_none() {
        let vu = Unit::compound(&[plain(BaseUnit::Second)]).unwrap();
        assert_eq!(vu.components()[0].prefix, SIPrefix::None);
    }

    #[test]
    fn test_max_components() {
        let vu = Unit::compound(&vec![plain(BaseUnit::Meter); MAX_UNIT_COMPONENTS]).unwrap();
        assert_eq!(vu.num_components(), MAX_UNIT_COMPONENTS);
    }

    #[test]
    fn test_too_many_components_raises() {
        let err = Unit::compound(&vec![plain(BaseUnit::Meter); MAX_UNIT_COMPONENTS + 1]);
        assert!(matches!(err, Err(bovnar::Error::Argument { .. })));
    }

    #[test]
    fn test_empty_list() {
        let vu = Unit::compound(&[]).unwrap();
        assert_eq!(vu.num_components(), 0);
    }

    #[test]
    fn test_active_components_matches_num() {
        let vu = Unit::compound(&[
            plain(BaseUnit::Meter),
            comp(BaseUnit::Second, SIPrefix::None, Exponent::NEG_LINEAR),
        ])
        .unwrap();
        assert_eq!(vu.components().len(), 2);
    }
}
