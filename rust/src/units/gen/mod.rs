//! Tables generated from the upstream data files by `scripts/gen-tables.py`.
//!
//! The files are checked in and must not be edited by hand; CI regenerates
//! them with `--check`. They are excluded from rustfmt so the generator's
//! output stays byte-identical to what is committed.

#[rustfmt::skip]
pub(crate) mod currencies;
#[rustfmt::skip]
pub(crate) mod prefixes;
#[rustfmt::skip]
// Every unit gets an id constant, only some are named in code; and a factor
// such as 2π is the unit's data, not a stand-in for a library constant.
#[allow(dead_code, clippy::approx_constant)]
pub(crate) mod units;
#[cfg(feature = "profile-unece")]
#[rustfmt::skip]
pub(crate) mod profile_unece;
#[cfg(feature = "profile-qudt")]
#[rustfmt::skip]
pub(crate) mod profile_qudt;
#[cfg(feature = "profile-qudt-qk")]
#[rustfmt::skip]
pub(crate) mod profile_qudt_qk;
#[cfg(feature = "profile-udunits")]
#[rustfmt::skip]
pub(crate) mod profile_udunits;
#[cfg(feature = "profile-om")]
#[rustfmt::skip]
pub(crate) mod profile_om;
