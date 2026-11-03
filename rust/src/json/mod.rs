//! Conversion between JSON and Bovnar, with the semantics of the C
//! reference's `bovnar convert`.
//!
//! # JSON to Bovnar
//!
//! [`to_bovnar`] returns exactly the text `bovnar convert --from json`
//! prints; [`to_document`] parses it into a [`Document`]. The JSON root must
//! be an object; its members become the document's assignments.
//!
//! | JSON | Bovnar |
//! |------|--------|
//! | object | struct (`{ ... }`) |
//! | array | bracket array; nested arrays stay nested, objects become structs |
//! | integer `>= 0` | `<uint>` (must fit 64 bits) |
//! | integer `< 0`, and `-0` | `<sint>` (must fit 64 bits) |
//! | number with `.` or exponent | `<float>` (must be finite as a 64-bit float) |
//! | string | string |
//! | `true`, `false` | bool |
//! | `null` | null (`.key = ;`) |
//!
//! It refuses what Bovnar cannot hold, with the reference's diagnostics:
//! a key that is not a Bovnar identifier ([`JsonError::InvalidKey`]), a
//! string with a control character other than tab, LF, VT, FF or CR
//! ([`JsonError::ControlCharacter`]) or `\u0000` ([`JsonError::Nul`]), an
//! integer outside 64 bits or a float that overflows, content after the
//! value ([`JsonError::TrailingContent`]), and anything whose Bovnar form
//! the reader would refuse ([`JsonError::Unrepresentable`]): duplicate keys
//! (`duplicate_struct_key`), arrays of objects with differing keys
//! (`struct_shape_mismatch`) and arrays mixing element types.
//!
//! ```
//! let text = bovnar::json::to_bovnar(br#"{"name": "probe", "temps": [20.5, 21.0]}"#, &Default::default())?;
//! assert_eq!(text, ".name = \"probe\";\n.temps = [20.5, <float> 21];\n");
//! # Ok::<(), bovnar::json::JsonError>(())
//! ```
//!
//! Arrays must be homogeneous, so `[1, "x"]` (and `[20.5, 21]`, a float
//! and an integer) are refused:
//!
//! ```
//! use bovnar::json::{to_bovnar, JsonError};
//!
//! let err = to_bovnar(br#"{"a": [1, "x"]}"#, &Default::default()).unwrap_err();
//! assert!(matches!(err, JsonError::Unrepresentable(_)));
//! ```
//!
//! # Bovnar to JSON
//!
//! [`from_document`] (or [`from_bovnar`] for text) returns the JSON
//! `bovnar convert --to json` prints, byte for byte, together with a
//! [`Loss`] report. The conversion is lossy: units, numeric types and bases,
//! symbols and references (become strings), octet streams (become hex
//! strings), non-finite floats (become `null`), datetimes (become epoch
//! seconds), integers wider than 64 bits (become decimal strings) and `/`
//! row structure (becomes nested arrays) do not survive. The reference
//! still prints the JSON and exits 1 when anything was lost; check
//! [`Loss::is_empty`] for the same verdict. A datetime with sub-second
//! digits is refused ([`JsonError::FractionalDatetime`]).
//!
//! ```
//! let out = bovnar::json::from_bovnar(b".speed = 3 m/s;\n.ok = true;\n", &Default::default())?;
//! assert_eq!(out.json, "{\n  \"speed\": 3,\n  \"ok\": true\n}\n");
//! assert_eq!(out.loss.count(bovnar::json::LossKind::Unit), 1);
//! # Ok::<(), bovnar::json::JsonError>(())
//! ```
//!
//! # Round trip
//!
//! JSON that converts to Bovnar converts back to the same JSON value: same
//! keys in the same order, same strings, booleans and nulls, same integers,
//! floats equal as 64-bit values (their spelling is normalised: `2.0` comes
//! back as `2`, `1e10` as `10000000000`), and empty containers kept.
//!
//! # Limits
//!
//! Everything is iterative and bounded. The JSON input is capped by
//! [`Limits::max_input_bytes`] (0 lifts the cap) and may nest at most 512
//! containers; the output is capped at eight times the input plus 64 KiB,
//! the reference's buffer. The writer and the reader that checks the result
//! use the nesting and length limits of `limits`. `bovnar convert` itself
//! runs with 255 levels of nesting (`max_struct_nesting` and
//! `max_array_nesting`), no input cap and no node cap; the default
//! [`Limits`] allow 64.

mod decode;
mod encode;
mod parse;

use alloc::string::String;
use core::fmt;

use crate::{Document, Error, Limits};

/// Why a conversion failed. The variants follow the C CLI's diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum JsonError {
    /// The input is larger than [`Limits::max_input_bytes`].
    TooLarge,
    /// Malformed JSON at `offset` ("Failed to parse JSON").
    Syntax {
        /// Byte offset where parsing stopped.
        offset: u64,
    },
    /// More than 512 nested arrays and objects.
    TooDeep {
        /// Byte offset of the value that is too deep.
        offset: u64,
    },
    /// A string contains `\u0000`, which a Bovnar string cannot hold.
    Nul {
        /// Byte offset of the string body.
        offset: u64,
    },
    /// An integer literal outside the signed (negative) or unsigned 64-bit
    /// range.
    IntegerOutOfRange {
        /// Byte offset of the literal.
        offset: u64,
    },
    /// A number too large for a 64-bit float (it would be infinite).
    FloatOverflow {
        /// The object key it sits under (`None` at the top level).
        key: Option<String>,
    },
    /// Something other than whitespace follows the JSON value.
    TrailingContent {
        /// Byte offset of the first trailing byte.
        offset: u64,
        /// The trailing byte is a NUL.
        nul: bool,
    },
    /// The JSON root is not an object.
    RootNotObject,
    /// An object key is not a Bovnar identifier (a letter or `_` first, then
    /// letters, digits, `_`, `+`, `-`).
    InvalidKey {
        /// The key (invalid UTF-8 replaced).
        key: String,
    },
    /// A string holds a control character a Bovnar string cannot carry.
    ControlCharacter {
        /// The object key it sits under.
        key: Option<String>,
        /// The byte.
        byte: u8,
    },
    /// A string is not valid UTF-8.
    InvalidUtf8 {
        /// The object key it sits under.
        key: Option<String>,
    },
    /// The Bovnar writer refused a value ("Bovnar writer error").
    Writer(Error),
    /// The result has no Bovnar representation: the reader refuses it with
    /// this error (duplicate keys, mixed or ragged arrays, objects of
    /// differing shape in one array).
    Unrepresentable(Error),
    /// The Bovnar input does not parse.
    Parse(Error),
    /// A datetime with sub-second digits, which JSON cannot carry.
    FractionalDatetime {
        /// The top-level key it sits under.
        key: String,
    },
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let under = |key: &Option<String>| key.clone().unwrap_or_else(|| "(top level)".into());
        match self {
            Self::TooLarge => f.write_str("input exceeds the size limit"),
            Self::Syntax { offset } => write!(f, "failed to parse JSON (at byte {offset})"),
            Self::TooDeep { offset } => {
                write!(f, "JSON nesting exceeds limit (512) at byte {offset}")
            }
            Self::Nul { .. } => f.write_str(
                "JSON string contains a NUL (\\u0000), which bovnar cannot represent in a string",
            ),
            Self::IntegerOutOfRange { .. } => f.write_str("integer literal out of range for 64-bit"),
            Self::FloatOverflow { key } => write!(
                f,
                "the JSON number under key '{}' is too large for a 64-bit float (it would become infinity)",
                under(key)
            ),
            Self::TrailingContent { nul, .. } => write!(
                f,
                "trailing content after JSON value{}",
                if *nul { " (a NUL byte)" } else { "" }
            ),
            Self::RootNotObject => f.write_str("JSON root must be an object"),
            Self::InvalidKey { key } => write!(
                f,
                "JSON key '{key}' is not a valid bovnar identifier (must start with a letter or '_' and contain only letters, digits, '_', '+', '-')"
            ),
            Self::ControlCharacter { key, byte } => write!(
                f,
                "the JSON string under key '{}' contains the control character U+{:04X}, which a bovnar string cannot carry",
                under(key),
                byte
            ),
            Self::InvalidUtf8 { key } => {
                write!(f, "the JSON string under key '{}' is not valid UTF-8", under(key))
            }
            Self::Writer(e) => write!(f, "Bovnar writer error: {}", e.kind()),
            Self::Unrepresentable(e) => {
                write!(f, "the JSON has no bovnar representation: {}", e.kind())
            }
            Self::Parse(e) => write!(f, "parse error: {e}"),
            Self::FractionalDatetime { key } => write!(
                f,
                "a datetime with sub-second digits cannot be represented in JSON (under top-level key '{key}')"
            ),
        }
    }
}

impl core::error::Error for JsonError {}

/// Converts JSON to Bovnar text, exactly as `bovnar convert --from json`.
///
/// # Errors
///
/// See [`JsonError`]; nothing is returned unless the whole input converts.
pub fn to_bovnar(json: &[u8], limits: &Limits) -> Result<String, JsonError> {
    let (text, _) = convert(json, limits)?;
    // The writer only emits UTF-8 (it validates every string and key).
    String::from_utf8(text).map_err(|_| JsonError::InvalidUtf8 { key: None })
}

/// Converts JSON to a [`Document`] (the tree of [`to_bovnar`]'s output).
///
/// # Errors
///
/// See [`JsonError`].
pub fn to_document(json: &[u8], limits: &Limits) -> Result<Document, JsonError> {
    convert(json, limits).map(|(_, doc)| doc)
}

fn convert(json: &[u8], limits: &Limits) -> Result<(alloc::vec::Vec<u8>, Document), JsonError> {
    if limits.max_input_bytes != 0 && json.len() as u64 > limits.max_input_bytes {
        return Err(JsonError::TooLarge);
    }
    let (tree, end) = parse::parse(json)?;
    if end != json.len() {
        return Err(JsonError::TrailingContent {
            offset: end as u64,
            nul: json.get(end) == Some(&0),
        });
    }
    let text = encode::encode(&tree, json.len(), limits)?;
    drop(tree);
    // The output is bounded by the input (see `encode`), so the input cap
    // does not apply to it a second time.
    let mut check = limits.clone();
    check.max_input_bytes = 0;
    let doc = crate::parse_with(&text, &check).map_err(JsonError::Unrepresentable)?;
    Ok((text, doc))
}

/// The JSON form of a document and what it lost.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Json {
    /// The JSON text, pretty-printed as `bovnar convert` prints it (with a
    /// final newline).
    pub json: String,
    /// What JSON could not carry.
    pub loss: Loss,
}

/// Converts a document to JSON, exactly as `bovnar convert --to json`.
///
/// # Errors
///
/// [`JsonError::FractionalDatetime`] for a datetime with sub-second digits.
pub fn from_document(doc: &Document) -> Result<Json, JsonError> {
    let (json, loss) = decode::decode(doc)?;
    Ok(Json { json, loss })
}

/// Parses Bovnar text and converts it to JSON (see [`from_document`]).
///
/// # Errors
///
/// [`JsonError::Parse`] when the input does not parse, otherwise as
/// [`from_document`].
pub fn from_bovnar(input: &[u8], limits: &Limits) -> Result<Json, JsonError> {
    let doc = crate::parse_with(input, limits).map_err(JsonError::Parse)?;
    from_document(&doc)
}

/// A kind of information JSON cannot carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LossKind {
    /// A value's unit.
    Unit,
    /// An integer wider than 64 bits, written as a JSON string.
    WideInteger,
    /// An integer's non-decimal base.
    NonDecimalBase,
    /// `nan`, `inf` or `ninf`, written as `null`.
    NonFiniteFloat,
    /// A float wider or more exact than a 64-bit binary float.
    WideFloat,
    /// A datetime, written as its epoch seconds.
    Datetime,
    /// A symbol, written as a string.
    Symbol,
    /// A reference, written as a string.
    Reference,
    /// An octet stream, written as a hex string.
    Octets,
    /// A `/`-separated multi-row array, written as nested arrays.
    Dimensions,
}

impl LossKind {
    /// All kinds, in the order the reference reports them.
    pub const ALL: [LossKind; 10] = [
        Self::Unit,
        Self::WideInteger,
        Self::NonDecimalBase,
        Self::NonFiniteFloat,
        Self::WideFloat,
        Self::Datetime,
        Self::Symbol,
        Self::Reference,
        Self::Octets,
        Self::Dimensions,
    ];

    const fn index(self) -> usize {
        self as usize
    }

    /// The reference's buffer size for this kind's example.
    const fn example_cap(self) -> usize {
        match self {
            Self::Unit | Self::WideFloat | Self::Datetime | Self::Dimensions => 160,
            _ => 128,
        }
    }

    const fn describe(self) -> &'static str {
        match self {
            Self::Unit => "value(s) lose their unit entirely",
            Self::WideInteger => "integer(s) wider than 64 bits become JSON strings",
            Self::NonDecimalBase => "integer(s) lose their numeric base",
            Self::NonFiniteFloat => "non-finite float(s) become null",
            Self::WideFloat => {
                "float(s) wider or more exact than a 64-bit binary float are rounded to one"
            }
            Self::Datetime => "datetime(s) become bare integers",
            Self::Symbol => "symbol(s) become plain strings",
            Self::Reference => "reference(s) become plain strings and stop resolving",
            Self::Octets => "octet stream(s) become hex strings",
            Self::Dimensions => {
                "\"/\"-separated multi-dimensional array(s) become nested JSON arrays and read back as nested arrays"
            }
        }
    }
}

/// What a Bovnar-to-JSON conversion dropped: a count per [`LossKind`] and
/// the first example of each (a key path such as `.a.b[2]`, with a detail).
///
/// Its `Display` form is the reference's report, one line per kind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Loss {
    counts: [u32; 10],
    examples: [Option<String>; 10],
}

impl Loss {
    /// Whether nothing was lost: the JSON converts back to the same
    /// document. (`bovnar convert` exits 0 exactly then.)
    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// Number of values that lost something.
    pub fn total(&self) -> u32 {
        self.counts.iter().fold(0u32, |a, &c| a.saturating_add(c))
    }

    /// How many values lost `kind`.
    pub fn count(&self, kind: LossKind) -> u32 {
        self.counts.get(kind.index()).copied().unwrap_or(0)
    }

    /// The first value that lost `kind`, as `path (detail)`.
    pub fn example(&self, kind: LossKind) -> Option<&str> {
        self.examples.get(kind.index()).and_then(Option::as_deref)
    }

    fn note(&mut self, kind: LossKind, path: &str, detail: Option<&str>) {
        let i = kind.index();
        if let Some(c) = self.counts.get_mut(i) {
            *c = c.saturating_add(1);
        }
        if let Some(slot @ None) = self.examples.get_mut(i) {
            let text = match detail {
                Some(d) => alloc::format!("{path} ({d})"),
                None => String::from(path),
            };
            *slot = Some(decode::truncate(text, kind.example_cap()));
        }
    }
}

impl fmt::Display for Loss {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for kind in LossKind::ALL {
            let n = self.count(kind);
            if n > 0 {
                writeln!(
                    f,
                    "  {n} {}, e.g. {}",
                    kind.describe(),
                    self.example(kind).unwrap_or_default()
                )?;
            }
        }
        Ok(())
    }
}
