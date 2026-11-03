//! Thin Rust binding to the Bovnar (BVNR) C library.
//!
//! The module split mirrors the Python binding in `python/bovnar/`. All FFI
//! lives in the private `ffi` module; every other module is safe Rust on top
//! of it.

#![deny(unsafe_code)]

pub mod enums;
pub mod error;
#[allow(unsafe_code)]
mod ffi;

pub use enums::{ErrorCode, Event, TokenType, ValueTypeFamily};
pub use error::{Error, Result};
