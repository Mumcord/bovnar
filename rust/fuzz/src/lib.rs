//! Shared properties for the bovnar fuzz targets; see `checks.rs`.

mod checks;

pub use checks::{detect, dom, json, reader, roundtrip, units, writer};
