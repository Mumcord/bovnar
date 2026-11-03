//! Thin Rust binding to the Bovnar (BVNR) C library.
//!
//! All FFI
//! lives in the private `ffi` module; every other module is safe Rust on top
//! of it.

#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod ffi;

pub use ffi::ValueTypeSpec;
