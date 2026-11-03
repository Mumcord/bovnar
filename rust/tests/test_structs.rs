//! Port of python/tests/test_structs.py, as far as the public API reaches.
//!
//! Python's `structs.py` (the ctypes mirrors and their `make_*` helpers) is
//! the private `ffi` module here; its layouts are checked against the C
//! compiler by test_abi.rs and the `abi_tests` unit test in src/ffi.rs. The
//! public pieces are `ValueTypeSpec` (built with a struct literal) and `Unit`
//! (which wraps `value_unit_t`).
//!
//! Skipped (the struct mirror or helper is private, or not mirrored):
//! - TestConstants: test_max_unit_components, test_opaque_bytes
//! - TestValueUnitComponent: test_size_reasonable, test_zero_init,
//!   test_set_fields, test_iec_prefix, test_union_overlap, test_repr
//! - TestValueUnit: test_size, test_active_components,
//!   test_dimensionless_single_none, test_not_dimensionless (raw components
//!   are not public)
//! - TestValueTypeSpec: test_repr (Python's repr names the family; the Rust
//!   `Debug` prints the raw field values)
//! - TestBvnrData: test_zero_init, test_raw_bytes, test_raw_bytes_empty,
//!   test_repr, test_has_spec_1_1_frac_fields,
//!   test_has_spec_1_2_unit_source_fields (field order is checked by the
//!   `abi_tests` unit test in src/ffi.rs)
//! - TestOpaqueTypes: test_source_size, test_sink_size,
//!   test_source_zero_init, test_sink_zero_init
//! - TestBvnrReadFlags: test_zero_init, test_set_max_file_size,
//!   test_size_reasonable
//! - TestBvnrWriteFlags: test_zero_init
//! - TestMakeTypeSpec: test_uint, test_float, test_defaults (no
//!   `make_type_spec`: a struct literal is the Rust form)
//! - TestMakeUnitHelpers: test_dimensionless, test_unit_none,
//!   test_unit_si_no_prefix, test_unit_si_with_prefix,
//!   test_unit_si_with_exponent, test_unit_iec
//! - TestMakeDataKey: test_ascii, test_unicode, test_data_pointer_valid
//! - TestMakeDataTyped: test_returns_bvnr_data
//! - TestMakeUnitCompound: all 11 tests (no `make_unit_compound`, and the
//!   unit enums it takes are not mirrored)

use bovnar::{Unit, ValueTypeFamily, ValueTypeSpec};

mod value_unit {
    use super::*;

    #[test]
    fn test_zero_init() {
        assert!(Unit::default().is_dimensionless());
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
