//! Thin Rust binding to the Bovnar (BVNR) C library.
//!
//! All FFI
//! lives in the private `ffi` module; every other module is safe Rust on top
//! of it.

#![deny(unsafe_code)]

pub mod currency;
pub mod enums;
pub mod error;
#[allow(unsafe_code)]
mod ffi;
pub mod unit;
pub mod units;

pub use enums::{
    BaseUnit, ErrorCode, Event, Exponent, IECPrefix, Prefix, PrefixSystem, SIPrefix, TokenType,
    UnknownValue, ValueTypeFamily,
};
pub use error::{Error, ErrorKind, Position, Result};
pub use ffi::ValueTypeSpec;
pub use unit::{MAX_UNIT_COMPONENTS, Unit, UnitComponent};
