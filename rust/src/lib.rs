//! Pure-Rust implementation of the Bovnar data format (spec 1.2).

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod error;

pub use error::{Error, ErrorKind};
