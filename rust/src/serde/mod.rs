//! [Serde](https://serde.rs) support: Rust data to and from Bovnar documents
//! (feature `serde`).
//!
//! ```
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! struct Config {
//!     name: String,
//!     port: u16,
//!     ratio: f64,
//!     tags: Vec<String>,
//!     backup: Option<Server>,
//!     mode: Mode,
//! }
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! struct Server {
//!     host: String,
//! }
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! enum Mode {
//!     Fast,
//!     Safe,
//! }
//!
//! let config = Config {
//!     name: "voice".into(),
//!     port: 64738,
//!     ratio: 0.5,
//!     tags: vec!["eu".into(), "main".into()],
//!     backup: Some(Server { host: "b.example".into() }),
//!     mode: Mode::Safe,
//! };
//! let text = bovnar::serde::to_string(&config)?;
//! assert!(text.contains(".port = <uint:16> 64738;"));
//! assert!(text.contains(".mode = Safe;"));
//! let back: Config = bovnar::serde::from_str(&text)?;
//! assert_eq!(back, config);
//! # Ok::<(), bovnar::serde::Error>(())
//! ```
//!
//! # Data model
//!
//! | Rust | Bovnar |
//! |------|--------|
//! | top-level struct or map | the document's assignments |
//! | nested struct, map | struct `{ .key = ...; }` (keys must be valid identifiers) |
//! | `Vec`, slice, tuple, tuple struct | array (elements must share one type, as Bovnar arrays do) |
//! | `Option`: `None` / `Some(v)` | null (written as an empty value, `.key = ;`) / `v` |
//! | `()`, unit struct | null |
//! | `u8`..`u64`, `u128` | `<uint:8>` .. `<uint:64>`, `<uint:128>` |
//! | `i8`..`i64`, `i128` | `<sint:8>` .. `<sint:64>`, `<sint:128>` |
//! | `f32`, `f64` | `<float:32>`, `<float:64>` |
//! | `bool` | `true` / `false` |
//! | `char`, `String` | UTF-8 string |
//! | bytes (`serialize_bytes`) | octet stream |
//! | unit enum variant | symbol: `.mode = Safe;` |
//! | newtype / tuple / struct variant | one-key struct, externally tagged: `{ .Variant = value; }` |
//!
//! Reading is lenient where it is unambiguous: any integer type reads any
//! integer that fits, floats read integers, a unit variant also reads from a
//! string, and a matrix array (`[1, 2] / [3, 4]`) reads as a sequence of rows.
//! Annotations and units are not part of the serde data model and are
//! ignored on reading.
//!
//! Octet streams make the output binary: write data with byte fields with
//! [`to_vec`], since [`to_string`] refuses non-UTF-8 output.
//!
//! # Limits
//!
//! Deserializing follows the nesting of the input, which the reader bounds
//! by [`Limits::max_struct_nesting`](crate::Limits::max_struct_nesting) and
//! [`Limits::max_array_nesting`](crate::Limits::max_array_nesting) (64 each by
//! default). Serde's visitors recurse once per level, so keep those limits
//! moderate on small stacks. Errors never panic.

mod de;
mod ser;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use ::serde::de::DeserializeOwned;
use ::serde::{Deserialize, Serialize};

use crate::{Document, ReadOptions};

/// A serialization or deserialization error.
#[derive(Debug, Clone, PartialEq)]
pub struct Error(Repr);

#[derive(Debug, Clone, PartialEq)]
enum Repr {
    Message(String),
    Bovnar(crate::Error),
}

impl Error {
    pub(crate) fn msg(message: impl fmt::Display) -> Self {
        Self(Repr::Message(message.to_string()))
    }

    /// The reader or writer error behind this one, if the document itself
    /// was refused rather than its mapping to Rust data.
    pub fn bovnar(&self) -> Option<&crate::Error> {
        match &self.0 {
            Repr::Bovnar(e) => Some(e),
            Repr::Message(_) => None,
        }
    }
}

impl From<crate::Error> for Error {
    fn from(e: crate::Error) -> Self {
        Self(Repr::Bovnar(e))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Repr::Message(m) => f.write_str(m),
            Repr::Bovnar(e) => e.fmt(f),
        }
    }
}

impl core::error::Error for Error {}

impl ::serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::msg(msg)
    }
}

impl ::serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::msg(msg)
    }
}

/// Maps `value` to a document. It must serialize as a struct or map, whose
/// fields become the document's assignments.
///
/// # Errors
///
/// `value` is not a struct or map, a map key is not a string, or its
/// `Serialize` implementation failed.
pub fn to_document<T: ?Sized + Serialize>(value: &T) -> Result<Document, Error> {
    ser::to_document(value)
}

/// Writes `value` as a pretty-printed document.
///
/// # Errors
///
/// As [`to_document`], a value the writer refuses (an invalid identifier, a
/// mixed-type array), or byte data, which needs [`to_vec`].
pub fn to_string<T: ?Sized + Serialize>(value: &T) -> Result<String, Error> {
    Ok(crate::to_string(&to_document(value)?)?)
}

/// Writes `value` as a pretty-printed document, as bytes.
///
/// # Errors
///
/// As [`to_document`], or a value the writer refuses.
pub fn to_vec<T: ?Sized + Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    Ok(crate::to_vec(&to_document(value)?)?)
}

/// Reads Rust data from a parsed document; strings and bytes may borrow from
/// it.
///
/// # Errors
///
/// The document does not match `T`.
pub fn from_document<'de, T: Deserialize<'de>>(doc: &'de Document) -> Result<T, Error> {
    T::deserialize(de::DocumentDeserializer::new(doc))
}

/// Parses `input` with default limits and reads Rust data from it.
///
/// # Errors
///
/// The input is not a valid document ([`Error::bovnar`]) or does not match
/// `T`.
pub fn from_slice<T: DeserializeOwned>(input: &[u8]) -> Result<T, Error> {
    from_slice_with_options(input, &ReadOptions::new())
}

/// [`from_slice`] for text.
///
/// # Errors
///
/// As [`from_slice`].
pub fn from_str<T: DeserializeOwned>(input: &str) -> Result<T, Error> {
    from_slice(input.as_bytes())
}

/// [`from_slice`] under `options` (see
/// [`parse_with_options`](crate::parse_with_options)).
///
/// # Errors
///
/// As [`from_slice`].
pub fn from_slice_with_options<T: DeserializeOwned>(
    input: &[u8],
    options: &ReadOptions,
) -> Result<T, Error> {
    let doc = crate::parse_with_options(input, options)?;
    from_document(&doc)
}
