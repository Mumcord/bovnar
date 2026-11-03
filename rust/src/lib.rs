//! Pure-Rust implementation of the Bovnar data format (spec 1.2).

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod error;
mod event;
mod limits;
pub(crate) mod num;
mod types;
pub mod units;
mod version;

pub use error::{Error, ErrorKind};
pub use event::{Event, TokenKind, TypeAnnotation};
pub use limits::Limits;
pub use types::{TypeFamily, TypeSpec};
pub use version::SpecVersion;
