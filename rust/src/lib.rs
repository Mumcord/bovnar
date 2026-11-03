//! Thin Rust binding to the Bovnar (BVNR) C library.
//!
//! The module split mirrors the Python binding in `python/bovnar/`. All FFI
//! lives in the private `ffi` module; every other module is safe Rust on top
//! of it.

#![deny(unsafe_code)]

pub mod dom;
pub mod enums;
pub mod error;
#[allow(unsafe_code)]
mod ffi;
pub mod reader;
pub mod writer;

pub use dom::{DomDoc, DomNode, DomType};
pub use enums::{ErrorCode, Event, TokenType, ValueTypeFamily};
pub use error::{Error, Result};
pub use ffi::ValueTypeSpec;
pub use reader::{EventData, EventPayload, ReadOptions, Reader};
pub use writer::Writer;

/// A plain BVNR value, the Rust counterpart of what Python's `loads` returns
/// and `dumps` takes. Type annotations and units are not carried.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    /// Any integer up to 128 bits (uint64 and sint64 included).
    Int(i128),
    Float(f64),
    /// A string; symbols and references read back as strings too.
    String(String),
    /// An octet stream.
    Bytes(Vec<u8>),
    /// An array; a multi-row array is an array of row arrays.
    Array(Vec<Value>),
    /// A struct (or the whole document): keys in document order.
    Struct(Vec<(String, Value)>),
}
