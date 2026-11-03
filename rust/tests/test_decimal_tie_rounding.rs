//! Port of `python/tests/test_decimal_tie_rounding.py`: exact `float_dec:64`
//! ties round half to even.
//!
//! Skipped (the whole file):
//! - test_exact_decimal64_ties_round_half_to_even,
//! - test_named_tie_cases: both read the materialised decimal64 through
//!   `Quantity.stored_value`, which is built on `BvnFloat` and is not
//!   mirrored by this crate (README, "Not mirrored").
