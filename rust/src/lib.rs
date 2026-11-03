//! Thin Rust binding to the Bovnar (BVNR) C library.
//!
//! The module split mirrors the Python binding in `python/bovnar/`. All FFI
//! lives in [`ffi`]; every other module is safe Rust on top of it.

#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod ffi;
