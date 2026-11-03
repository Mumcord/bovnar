//! Events the streaming reader reports for a verified document.

use crate::types::{TypeFamily, TypeSpec};
use crate::Error;

/// Kind of a token carried by an [`Event::Data`] (`token_type_t` in the C
/// reference).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TokenKind {
    /// Struct key.
    Identifier,
    /// String literal.
    String,
    /// Number literal (including `nan`, `inf`, `ninf`).
    Number,
    /// Symbol (`'name`).
    Symbol,
    /// Reference (`@a.b[0]`).
    Reference,
    /// Number literal inside an array.
    ArrayNumber,
    /// String literal inside an array.
    ArrayString,
    /// Type annotation.
    Type,
    /// Octet stream chunk.
    OctetStream,
    /// `null`.
    Null,
    /// Struct.
    Structure,
    /// Unit.
    Unit,
    /// Type width parameter.
    TypeWidth,
    /// Type base parameter.
    TypeBase,
    /// Fixed-point `q` parameter.
    TypeQ,
    /// `true`, `false`, `on`, `off`.
    Bool,
    /// Not classified.
    Unknown,
}

impl TokenKind {
    /// The name the conformance protocol prints for this kind (doc/13 §7).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Number => "number",
            Self::String => "string",
            Self::Symbol => "symbol",
            Self::Reference => "reference",
            Self::ArrayNumber => "array_number",
            Self::ArrayString => "array_string",
            Self::Null => "null",
            Self::OctetStream => "octets",
            Self::Structure => "structure",
            Self::Identifier => "identifier",
            Self::Type => "type",
            Self::TypeWidth => "type_width",
            Self::TypeBase => "type_base",
            Self::TypeQ => "type_q",
            Self::Unit => "unit",
            Self::Bool => "bool",
            Self::Unknown => "unknown",
        }
    }
}

/// A type annotation, explicit (`<uint:8,_16>`) or synthesised by the reader
/// for an untyped value (spec §6.6 default types).
///
/// The parameters are `Some` exactly when the reader reported them: an
/// explicit annotation reports the parameters it spells out, a synthesised one
/// reports width, base and unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeAnnotation<'a> {
    /// The annotation body as written (`float:64,m/s`), or the bare family
    /// keyword for a synthesised annotation.
    pub text: &'a [u8],
    /// The type family.
    pub family: TypeFamily,
    /// Effective width in bits, if reported.
    pub width: Option<u32>,
    /// Effective numeric base, if reported.
    pub base: Option<u32>,
    /// Fraction bits of a `float_fix`, if reported.
    pub q: Option<u32>,
    /// The unit as written, if reported.
    pub unit: Option<&'a [u8]>,
    /// Datetime epoch name (`unix`, `tai`, ...), for the datetime family.
    pub epoch: Option<&'static str>,
    /// Whether the reader synthesised this annotation for an untyped value.
    pub synthesized: bool,
}

/// One event of a verified document, in document order.
///
/// Borrowed data lives until the next call into the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event<'a> {
    /// The document starts.
    StreamStart,
    /// An assignment `.key = ...` starts.
    AssignmentStart {
        /// The key, without the leading dot.
        key: &'a [u8],
    },
    /// The type of the following value(s).
    TypeAnnotation(TypeAnnotation<'a>),
    /// A scalar value.
    Data {
        /// The token kind.
        kind: TokenKind,
        /// The value text: unescaped for strings, the literal for numbers,
        /// epoch seconds for a datetime given as an ISO-8601 literal.
        text: &'a [u8],
        /// Sub-second digits of an ISO-8601 datetime literal, else empty.
        frac: &'a [u8],
        /// The effective type of the value: its annotation, explicit or
        /// synthesised, as it applies to this value. [`TypeSpec::PLAIN`] for
        /// an untyped `null`.
        value_type: TypeSpec,
        /// The effective unit as spelled in the document (from the
        /// annotation or an inline suffix), empty when the value has none.
        unit: &'a [u8],
        /// The unit suffix written directly after this value (`5 m`), empty
        /// when there is none.
        inline_unit: &'a [u8],
    },
    /// A struct `{` opens.
    StructStart,
    /// A struct `}` closes.
    StructEnd,
    /// An array row `[` opens.
    ArrayRowStart,
    /// An array row `]` closes.
    ArrayRowEnd,
    /// A `/` dimension separator inside an array.
    ArrayDimStart,
    /// An octet stream starts.
    OctetStreamStart,
    /// One chunk of an octet stream.
    OctetStreamChunk(&'a [u8]),
    /// The octet stream ends.
    OctetStreamEnd,
    /// The document ends.
    StreamEnd,
    /// In recovery mode only: an error the reader skipped past. Events
    /// resume with the next assignment it could resynchronise on.
    Recovered(Error),
}
