//! A pure-Rust reader and writer for the [Bovnar](https://github.com/sothis/bovnar) data
//! format, specification version 1.2.
//!
//! Bovnar is a typed, unit-aware text format with an optional binary half
//! (octet streams). This crate reads it as a stream of events or as a
//! document tree, writes it back, and maps it to Rust data with serde. It is
//! built for untrusted network input and its behaviour is pinned to the C
//! reference implementation through the upstream conformance corpus.
//!
//! # Quick start
//!
//! Parse a whole document into a tree and query it:
//!
//! ```
//! let doc = bovnar::parse(b".server = { .host = \"localhost\"; .port = <uint:16> 64738; };")?;
//! assert_eq!(doc.lookup(".server.port").and_then(|v| v.as_u64()), Some(64738));
//! # Ok::<(), bovnar::Error>(())
//! ```
//!
//! Build a document in code and write it:
//!
//! ```
//! use bovnar::{Document, Struct};
//!
//! let mut server = Struct::new();
//! server.insert("host", "localhost");
//! server.insert("port", 64738u16);
//! let mut doc = Document::new();
//! doc.insert("server", server);
//! assert_eq!(
//!     bovnar::to_string(&doc)?,
//!     ".server = {\n\t.host = \"localhost\";\n\t.port = <uint:16> 64738;\n};\n"
//! );
//! # Ok::<(), bovnar::Error>(())
//! ```
//!
//! Stream events in constant memory, feeding input in chunks of any size
//! (`Reader`); with `std`, `stream::ReadAdapter` pulls from any
//! `std::io::Read`:
//!
//! ```
//! use bovnar::{Event, ReadOptions, Reader};
//!
//! let mut reader = Reader::with_options(&ReadOptions::new().strict_version(true));
//! let mut keys = 0;
//! for chunk in [&b".a = 1; .b"[..], &b" = 2;"[..]] {
//!     reader.feed(chunk)?;
//!     while let Some(event) = reader.next_event() {
//!         if let Event::AssignmentStart { .. } = event {
//!             keys += 1;
//!         }
//!     }
//! }
//! reader.finish()?;
//! assert_eq!(keys, 2);
//! # Ok::<(), bovnar::Error>(())
//! ```
//!
//! Map Rust data with serde (feature `serde`):
//!
//! ```
//! # #[cfg(feature = "serde")]
//! # {
//! #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
//! struct Peer {
//!     name: String,
//!     volume: f32,
//! }
//!
//! let peer = Peer { name: "alice".into(), volume: 0.75 };
//! let text = bovnar::serde::to_string(&peer)?;
//! assert_eq!(bovnar::serde::from_str::<Peer>(&text)?, peer);
//! # }
//! # Ok::<(), Box<dyn core::error::Error>>(())
//! ```
//!
//! Several documents travel over one connection with the length-prefixed
//! record framing in [`stream`].
//!
//! Convert JSON to Bovnar and back with the C CLI's `bovnar convert`
//! semantics ([`json`]), and sniff which format a buffer holds
//! ([`detect`]):
//!
//! ```
//! use bovnar::{detect, json, Format, Limits};
//!
//! let input = br#"{"name": "probe", "samples": [1, 2, 3]}"#;
//! assert_eq!(detect(input), Format::Json);
//! let text = json::to_bovnar(input, &Limits::default())?;
//! assert_eq!(text, ".name = \"probe\";\n.samples = [1, 2, 3];\n");
//! let back = json::from_bovnar(text.as_bytes(), &Limits::default())?;
//! assert!(back.loss.is_empty());
//! assert_eq!(back.json, "{\n  \"name\": \"probe\",\n  \"samples\": [1, 2, 3]\n}\n");
//! # Ok::<(), bovnar::json::JsonError>(())
//! ```
//!
//! # Safety and limits
//!
//! - No `unsafe` code (`#![forbid(unsafe_code)]`).
//! - No panics on any input, and no recursion on input depth in the reader,
//!   the tree builder, the writer or when dropping a tree. (Serde
//!   deserialization follows serde's recursive visitor model; its depth is
//!   bounded by the nesting limits below.)
//! - Every resource is bounded by [`Limits`]: input size (16 MiB by
//!   default), lengths of strings, numbers, identifiers and symbols, nesting
//!   depth (64), array items and the number of nodes in a tree. Tighten them
//!   for your protocol; `0` lifts [`Limits::max_input_bytes`] for endless
//!   streams.
//! - [`ReadOptions`] adds the C reader's policies: `strict_version` refuses
//!   documents declaring an unsupported spec version, `text_only` refuses
//!   octet streams at their first byte, `recovery` resynchronises after
//!   errors instead of stopping.
//! - Frame payloads ([`stream::FrameDecoder`]) are capped (256 MiB by
//!   default) and their buffers grow with the bytes that arrive, never with a
//!   declared length.
//! - Errors carry the C reference's error names ([`ErrorKind::as_str`]) and
//!   the position of the offending byte.
//!
//! The UCUM and CF unit profiles are not included: units in the `ucum:` and
//! `cf:` namespaces are refused with [`ErrorKind::UnitProfileUnknown`],
//! exactly as a C build with those profiles switched off refuses them.
//!
//! # Feature flags
//!
//! | Feature | Default | Adds |
//! |---------|---------|------|
//! | `std` | yes | `std::io` adapters: `IoSink`, `stream::ReadAdapter`, `stream::FrameReader`, `stream::read_doc_stream` |
//! | `serde` | no | the `bovnar::serde` module, and `Serialize`/`Deserialize` for `Limits` |
//! | `profile-unece` | yes | the UN/ECE Recommendation 20 unit profile (`unece:`) |
//! | `profile-qudt` | yes | the QUDT unit profile (`qudt:`) |
//! | `profile-qudt-qk` | yes | the QUDT quantity-kind profile (`qudt-qk:`) |
//! | `profile-udunits` | yes | the UDUNITS-2 unit profile (`udunits:`) |
//! | `profile-om` | yes | the Ontology of units of Measure profile (`om:`) |
//!
//! # `no_std`
//!
//! The crate is `#![no_std]` and needs only `alloc`. Disable default
//! features (`default-features = false`) for targets without `std`, such as
//! `wasm32-unknown-unknown`; re-enable the unit profiles you need. Everything
//! except the `std::io` adapters stays available, serde support included.
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[doc(hidden)]
pub mod conformance;
mod detect;
mod dom;
mod error;
mod event;
pub mod json;
mod limits;
pub(crate) mod num;
mod reader;
#[cfg(feature = "serde")]
pub mod serde;
pub mod stream;
mod types;
pub mod units;
mod value;
mod version;
mod writer;

pub use error::{Error, ErrorKind};
pub use event::{Event, TokenKind, TypeAnnotation};
pub use limits::Limits;
pub use reader::{read_all, read_all_with, ReadOptions, Reader};
pub use types::{TypeFamily, TypeSpec};
pub use version::SpecVersion;

pub use detect::{detect, Format};
pub use dom::{parse, parse_with, parse_with_options};
pub use value::{Annotation, Array, Document, Kind, Number, Struct, Value};
#[cfg(feature = "std")]
pub use writer::IoSink;
pub use writer::{to_string, to_string_compact, to_vec, to_vec_compact, FmtSink, Sink, Writer};
