//! Shared properties for the bovnar fuzz targets; see `checks.rs`.

mod checks;

pub use checks::{dom, reader, roundtrip, units, writer};
