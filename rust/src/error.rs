//! Error type shared by every part of the crate.

use core::fmt;

/// What went wrong while reading a Bovnar document.
///
/// The variants mirror the C reference's `error_code_t` (upstream
/// `include/bovnar.h`) one to one and in the same order, minus `error_none`,
/// which a `Result` makes unnecessary. [`ErrorKind::as_str`] returns the name
/// the C function `bvn_error_to_string` prints, so diagnostics and conformance
/// logs match the reference byte for byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// A token of a type the parser does not know was produced.
    UnknownTokenType,
    /// The rows of an array differ in length.
    ArrayRowSizeMismatch,
    /// An identifier exceeds [`Limits::max_identifier_length`](crate::Limits::max_identifier_length).
    IdentifierTooLong,
    /// An identifier has no characters.
    EmptyIdentifier,
    /// Structs are nested deeper than [`Limits::max_struct_nesting`](crate::Limits::max_struct_nesting).
    StructNestingTooHigh,
    /// Arrays are nested deeper than [`Limits::max_array_nesting`](crate::Limits::max_array_nesting).
    ArrayNestingTooHigh,
    /// A `}` closes a struct that is not open.
    IllegalStructClose,
    /// A string exceeds [`Limits::max_string_length`](crate::Limits::max_string_length).
    StringTooLong,
    /// A malformed escape sequence (bad hex, missing brace, empty or over-long braces).
    IllegalEscapeSequence,
    /// A number literal exceeds [`Limits::max_number_length`](crate::Limits::max_number_length).
    NumberTooLong,
    /// A symbol exceeds [`Limits::max_symbol_length`](crate::Limits::max_symbol_length).
    SymbolTooLong,
    /// A reference exceeds [`Limits::max_reference_length`](crate::Limits::max_reference_length).
    ReferenceTooLong,
    /// A complete chunk of input could not be read.
    ReadCompleteChunkFailed,
    /// The framing of an octet stream is inconsistent (for example an unknown chunk tag).
    OctetStreamOutOfSync,
    /// A byte that is not allowed at this position.
    UnexpectedInputByte,
    /// The text part of the document exceeds the text size limit.
    TextDataTooLong,
    /// Reading from the source failed.
    ReadingFromSourceFd,
    /// The input ended in the middle of a construct.
    GotIncompleteBvnrStream,
    /// A byte sequence that is not valid UTF-8 (including overlong forms).
    InvalidUtf8Byte,
    /// A UTF-8 byte order mark inside the first comment line.
    InvalidByteOrderMark,
    /// A type annotation body is too long.
    TypeTooLong,
    /// A unit is too long.
    UnitTooLong,
    /// Reserved by the C reference; never produced.
    ExpectedStringInArray,
    /// Reserved by the C reference; never produced.
    ExpectedNumberInArray,
    /// A value type that is not allowed here.
    IllegalValueType,
    /// A callback of the scanner refused an event.
    ScannerCallbackFailed,
    /// The input exceeds the total input size limit.
    FileTooLong,
    /// An argument passed to the API is invalid.
    InvalidArgument,
    /// An array holds more items than [`Limits::max_array_items`](crate::Limits::max_array_items).
    TooManyArrayItems,
    /// Writing to the sink failed.
    WritingToSink,
    /// The sink buffer is full.
    SinkBufferExhausted,
    /// A unit is malformed or names something the vocabulary does not define.
    UnitIllegal,
    /// A non-decimal base was given for a value that is not a string literal.
    BaseRequiresStringLiteral,
    /// A value does not match its type annotation.
    TypeValueMismatch,
    /// A value does not fit the range of its type.
    ValueOutOfRange,
    /// A digit is not valid in the declared base.
    DigitNotInBase,
    /// Reserved by the C reference; never produced.
    Recovered,
    /// Two units that must agree do not.
    UnitMismatch,
    /// The non-null elements of an array differ in kind or unit.
    ArrayElementTypeMismatch,
    /// Sibling structs in an array do not share the same shape.
    StructShapeMismatch,
    /// A struct (or the top-level document) repeats a key.
    DuplicateStructKey,
    /// A `#!bovnar` version directive is malformed.
    InvalidSpecVersion,
    /// The document declares a spec version newer than supported, and strict version handling is on.
    UnsupportedSpecVersion,
    /// A `\u{…}` escape names a surrogate or a value above U+10FFFF.
    InvalidCodepoint,
    /// An ISO-8601 datetime literal is malformed or out of range.
    InvalidDatetimeLiteral,
    /// A datetime literal was given for a GNSS epoch, which only accepts integer seconds.
    DatetimeLiteralUnsupportedEpoch,
    /// A requested unit or base conversion cannot be performed losslessly.
    UnitInexact,
    /// An octet stream ended while a multiplexed message was still incomplete.
    OctetStreamTruncated,
    /// The namespace before `:` in a unit is not a profile this build supports.
    ///
    /// UCUM (`ucum:`) and CF (`cf:`) always produce this error in bovnar-rs.
    UnitProfileUnknown,
    /// A unit is valid in its profile vocabulary but has no representation in the unit model.
    UnitProfileUnsupported,
    /// The document contains an octet stream, but the reader was told to accept text only.
    OctetStreamForbidden,
    /// Whitespace splits a type-annotation parameter in two.
    TypeParamWhitespace,
}

impl ErrorKind {
    /// Returns the error name exactly as the C reference's `bvn_error_to_string`
    /// prints it (for example `"value_out_of_range"`).
    ///
    /// ```
    /// use bovnar::ErrorKind;
    /// assert_eq!(ErrorKind::ValueOutOfRange.as_str(), "value_out_of_range");
    /// assert_eq!(ErrorKind::GotIncompleteBvnrStream.as_str(), "incomplete_bvnr_stream");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownTokenType => "unknown_token_type",
            Self::ArrayRowSizeMismatch => "array_row_size_mismatch",
            Self::IdentifierTooLong => "identifier_too_long",
            Self::EmptyIdentifier => "empty_identifier",
            Self::StructNestingTooHigh => "struct_nesting_too_high",
            Self::ArrayNestingTooHigh => "array_nesting_too_high",
            Self::IllegalStructClose => "illegal_struct_close",
            Self::StringTooLong => "string_too_long",
            Self::IllegalEscapeSequence => "illegal_escape_sequence",
            Self::NumberTooLong => "number_too_long",
            Self::SymbolTooLong => "symbol_too_long",
            Self::ReferenceTooLong => "reference_too_long",
            Self::ReadCompleteChunkFailed => "read_complete_chunk_failed",
            Self::OctetStreamOutOfSync => "octet_stream_out_of_sync",
            Self::UnexpectedInputByte => "unexpected_input_byte",
            Self::TextDataTooLong => "text_data_too_long",
            Self::ReadingFromSourceFd => "reading_from_source_fd",
            // The C enum name and its printed name differ here.
            Self::GotIncompleteBvnrStream => "incomplete_bvnr_stream",
            Self::InvalidUtf8Byte => "invalid_utf8_byte",
            Self::InvalidByteOrderMark => "invalid_byte_order_mark",
            Self::TypeTooLong => "type_too_long",
            Self::UnitTooLong => "unit_too_long",
            Self::ExpectedStringInArray => "expected_string_in_array",
            Self::ExpectedNumberInArray => "expected_number_in_array",
            Self::IllegalValueType => "illegal_value_type",
            Self::ScannerCallbackFailed => "scanner_callback_failed",
            Self::FileTooLong => "file_too_long",
            Self::InvalidArgument => "invalid_argument",
            Self::TooManyArrayItems => "too_many_array_items",
            Self::WritingToSink => "writing_to_sink",
            Self::SinkBufferExhausted => "sink_buffer_exhausted",
            Self::UnitIllegal => "unit_illegal",
            Self::BaseRequiresStringLiteral => "base_requires_string_literal",
            Self::TypeValueMismatch => "type_value_mismatch",
            Self::ValueOutOfRange => "value_out_of_range",
            Self::DigitNotInBase => "digit_not_in_base",
            Self::Recovered => "recovered",
            Self::UnitMismatch => "unit_mismatch",
            Self::ArrayElementTypeMismatch => "array_element_type_mismatch",
            Self::StructShapeMismatch => "struct_shape_mismatch",
            Self::DuplicateStructKey => "duplicate_struct_key",
            Self::InvalidSpecVersion => "invalid_spec_version",
            Self::UnsupportedSpecVersion => "unsupported_spec_version",
            Self::InvalidCodepoint => "invalid_codepoint",
            Self::InvalidDatetimeLiteral => "invalid_datetime_literal",
            Self::DatetimeLiteralUnsupportedEpoch => "datetime_literal_unsupported_epoch",
            Self::UnitInexact => "unit_inexact",
            Self::OctetStreamTruncated => "octet_stream_truncated",
            Self::UnitProfileUnknown => "unit_profile_unknown",
            Self::UnitProfileUnsupported => "unit_profile_unsupported",
            Self::OctetStreamForbidden => "octet_stream_forbidden",
            Self::TypeParamWhitespace => "type_param_whitespace",
        }
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An error with the position in the input where it was detected.
///
/// `line` and `column` are 1-based; `offset` is the 0-based byte offset into
/// the input. All three are 0 when the error is not tied to an input position.
///
/// ```
/// use bovnar::{conformance, ErrorKind, Limits};
/// let err = conformance::event_log(b".x = <uint:8> 256;", &Limits::default()).unwrap_err();
/// assert_eq!(err.kind(), ErrorKind::ValueOutOfRange);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Error {
    kind: ErrorKind,
    line: u64,
    column: u64,
    offset: u64,
}

impl Error {
    /// Creates an error at the given position.
    #[must_use]
    pub const fn new(kind: ErrorKind, line: u64, column: u64, offset: u64) -> Self {
        Self {
            kind,
            line,
            column,
            offset,
        }
    }

    /// Creates an error that is not tied to an input position.
    #[must_use]
    pub const fn without_position(kind: ErrorKind) -> Self {
        Self::new(kind, 0, 0, 0)
    }

    /// What went wrong.
    #[must_use]
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// 1-based line of the offending byte, or 0 if there is none.
    #[must_use]
    pub const fn line(&self) -> u64 {
        self.line
    }

    /// 1-based column of the offending byte, or 0 if there is none.
    #[must_use]
    pub const fn column(&self) -> u64 {
        self.column
    }

    /// 0-based byte offset of the offending byte into the input.
    #[must_use]
    pub const fn offset(&self) -> u64 {
        self.offset
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self::without_position(kind)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.kind)
        } else {
            write!(
                f,
                "{} at line {}, column {} (byte offset {})",
                self.kind, self.line, self.column, self.offset
            )
        }
    }
}

impl core::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn display_with_and_without_position() {
        assert_eq!(
            Error::new(ErrorKind::StringTooLong, 3, 7, 42).to_string(),
            "string_too_long at line 3, column 7 (byte offset 42)"
        );
        assert_eq!(
            Error::from(ErrorKind::FileTooLong).to_string(),
            "file_too_long"
        );
    }
}
