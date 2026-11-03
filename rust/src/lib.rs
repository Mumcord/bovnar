//! Pure-Rust implementation of the Bovnar data format (spec 1.2).

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod dom;
mod error;
mod event;
mod limits;
pub(crate) mod num;
mod reader;
mod types;
pub mod units;
mod value;
mod version;

pub use error::{Error, ErrorKind};
pub use event::{Event, TokenKind, TypeAnnotation};
pub use limits::Limits;
pub use reader::{read_all, read_all_with, ReadOptions, Reader};
pub use types::{TypeFamily, TypeSpec};
pub use version::SpecVersion;

pub use dom::{parse, parse_with, parse_with_options};
pub use value::{Annotation, Array, Document, Kind, Number, Struct, Value};
