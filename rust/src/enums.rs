//! The public C enums, mirroring `python/bovnar/enums.py` (same names and
//! values; the unit enums follow with the unit API).

/// Reader/writer events (`bvnr_event_t`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    StreamStart = 0,
    AssignmentStart = 1,
    OctetStreamStart = 2,
    OctetStreamEnd = 3,
    StructStart = 4,
    StructEnd = 5,
    ArrayRowStart = 6,
    ArrayRowEnd = 7,
    ArrayDimStart = 8,
    Data = 9,
    TypeAnnotationStart = 10,
    TypeAnnotationEnd = 11,
    TypeAnnotationTypeFamily = 12,
    TypeAnnotationTypeFamilyParam = 13,
    StreamEnd = 14,
}

impl Event {
    /// The variant for a raw C value, or `None` for a value this binding
    /// does not know.
    pub fn from_raw(raw: i32) -> Option<Self> {
        Some(match raw {
            0 => Self::StreamStart,
            1 => Self::AssignmentStart,
            2 => Self::OctetStreamStart,
            3 => Self::OctetStreamEnd,
            4 => Self::StructStart,
            5 => Self::StructEnd,
            6 => Self::ArrayRowStart,
            7 => Self::ArrayRowEnd,
            8 => Self::ArrayDimStart,
            9 => Self::Data,
            10 => Self::TypeAnnotationStart,
            11 => Self::TypeAnnotationEnd,
            12 => Self::TypeAnnotationTypeFamily,
            13 => Self::TypeAnnotationTypeFamilyParam,
            14 => Self::StreamEnd,
            _ => return None,
        })
    }
}

/// Token types (`token_type_t`), carried in `BvnrData.type`.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenType {
    Identifier = 0,
    String = 1,
    Number = 2,
    Symbol = 3,
    Reference = 4,
    ArrayNumber = 5,
    ArrayString = 6,
    Type = 7,
    OctetStream = 8,
    NullValue = 9,
    Structure = 10,
    Unit = 11,
    TypeWidth = 12,
    TypeBase = 13,
    TypeQ = 14,
    Bool = 15,
    Unknown = 16,
}

impl TokenType {
    /// The variant for a raw C value, or `None` for a value this binding
    /// does not know.
    pub fn from_raw(raw: i32) -> Option<Self> {
        Some(match raw {
            0 => Self::Identifier,
            1 => Self::String,
            2 => Self::Number,
            3 => Self::Symbol,
            4 => Self::Reference,
            5 => Self::ArrayNumber,
            6 => Self::ArrayString,
            7 => Self::Type,
            8 => Self::OctetStream,
            9 => Self::NullValue,
            10 => Self::Structure,
            11 => Self::Unit,
            12 => Self::TypeWidth,
            13 => Self::TypeBase,
            14 => Self::TypeQ,
            15 => Self::Bool,
            16 => Self::Unknown,
            _ => return None,
        })
    }
}

/// Value type families (`value_type_family_t`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ValueTypeFamily {
    Plain = 0,
    Utf8 = 1,
    Sint = 2,
    Uint = 3,
    Float = 4,
    FloatFix = 5,
    FloatDec = 6,
    Bool = 7,
    Datetime = 8,
    Illegal = 9,
}

impl ValueTypeFamily {
    /// The variant for a raw C value, or `None` for a value this binding
    /// does not know.
    pub fn from_raw(raw: i32) -> Option<Self> {
        Some(match raw {
            0 => Self::Plain,
            1 => Self::Utf8,
            2 => Self::Sint,
            3 => Self::Uint,
            4 => Self::Float,
            5 => Self::FloatFix,
            6 => Self::FloatDec,
            7 => Self::Bool,
            8 => Self::Datetime,
            9 => Self::Illegal,
            _ => return None,
        })
    }
}

/// Library error codes (`error_code_t`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    None = 0,
    UnknownTokenType = 1,
    ArrayRowSizeMismatch = 2,
    IdentifierTooLong = 3,
    EmptyIdentifier = 4,
    StructNestingTooHigh = 5,
    ArrayNestingTooHigh = 6,
    IllegalStructClose = 7,
    StringTooLong = 8,
    IllegalEscapeSequence = 9,
    NumberTooLong = 10,
    SymbolTooLong = 11,
    ReferenceTooLong = 12,
    ReadCompleteChunkFailed = 13,
    OctetStreamOutOfSync = 14,
    UnexpectedInputByte = 15,
    TextDataTooLong = 16,
    ReadingFromSourceFd = 17,
    GotIncompleteBvnrStream = 18,
    InvalidUtf8Byte = 19,
    InvalidByteOrderMark = 20,
    TypeTooLong = 21,
    UnitTooLong = 22,
    ExpectedStringInArray = 23,
    ExpectedNumberInArray = 24,
    IllegalValueType = 25,
    ScannerCallbackFailed = 26,
    FileTooLong = 27,
    InvalidArgument = 28,
    TooManyArrayItems = 29,
    WritingToSink = 30,
    SinkBufferExhausted = 31,
    UnitIllegal = 32,
    BaseRequiresStringLiteral = 33,
    TypeValueMismatch = 34,
    ValueOutOfRange = 35,
    DigitNotInBase = 36,
    Recovered = 37,
    UnitMismatch = 38,
    ArrayElementTypeMismatch = 39,
    StructShapeMismatch = 40,
    DuplicateStructKey = 41,
    InvalidSpecVersion = 42,
    UnsupportedSpecVersion = 43,
    InvalidCodepoint = 44,
    InvalidDatetimeLiteral = 45,
    DatetimeLiteralUnsupportedEpoch = 46,
    UnitInexact = 47,
    OctetStreamTruncated = 48,
    UnitProfileUnknown = 49,
    UnitProfileUnsupported = 50,
    OctetStreamForbidden = 51,
    TypeParamWhitespace = 52,
}

impl ErrorCode {
    /// The variant for a raw C value, or `None` for a value this binding
    /// does not know.
    pub fn from_raw(raw: i32) -> Option<Self> {
        Some(match raw {
            0 => Self::None,
            1 => Self::UnknownTokenType,
            2 => Self::ArrayRowSizeMismatch,
            3 => Self::IdentifierTooLong,
            4 => Self::EmptyIdentifier,
            5 => Self::StructNestingTooHigh,
            6 => Self::ArrayNestingTooHigh,
            7 => Self::IllegalStructClose,
            8 => Self::StringTooLong,
            9 => Self::IllegalEscapeSequence,
            10 => Self::NumberTooLong,
            11 => Self::SymbolTooLong,
            12 => Self::ReferenceTooLong,
            13 => Self::ReadCompleteChunkFailed,
            14 => Self::OctetStreamOutOfSync,
            15 => Self::UnexpectedInputByte,
            16 => Self::TextDataTooLong,
            17 => Self::ReadingFromSourceFd,
            18 => Self::GotIncompleteBvnrStream,
            19 => Self::InvalidUtf8Byte,
            20 => Self::InvalidByteOrderMark,
            21 => Self::TypeTooLong,
            22 => Self::UnitTooLong,
            23 => Self::ExpectedStringInArray,
            24 => Self::ExpectedNumberInArray,
            25 => Self::IllegalValueType,
            26 => Self::ScannerCallbackFailed,
            27 => Self::FileTooLong,
            28 => Self::InvalidArgument,
            29 => Self::TooManyArrayItems,
            30 => Self::WritingToSink,
            31 => Self::SinkBufferExhausted,
            32 => Self::UnitIllegal,
            33 => Self::BaseRequiresStringLiteral,
            34 => Self::TypeValueMismatch,
            35 => Self::ValueOutOfRange,
            36 => Self::DigitNotInBase,
            37 => Self::Recovered,
            38 => Self::UnitMismatch,
            39 => Self::ArrayElementTypeMismatch,
            40 => Self::StructShapeMismatch,
            41 => Self::DuplicateStructKey,
            42 => Self::InvalidSpecVersion,
            43 => Self::UnsupportedSpecVersion,
            44 => Self::InvalidCodepoint,
            45 => Self::InvalidDatetimeLiteral,
            46 => Self::DatetimeLiteralUnsupportedEpoch,
            47 => Self::UnitInexact,
            48 => Self::OctetStreamTruncated,
            49 => Self::UnitProfileUnknown,
            50 => Self::UnitProfileUnsupported,
            51 => Self::OctetStreamForbidden,
            52 => Self::TypeParamWhitespace,
            _ => return None,
        })
    }
}

/// Base-unit ids (`base_unit_t`), Python's `BaseUnit`: plain `i32` ids, so
/// a unit read off the wire never fails to convert.
pub struct BaseUnit;

include!(concat!(env!("OUT_DIR"), "/generated/base_units.rs"));

impl BaseUnit {
    /// The member name of `id` (`BaseUnit(id).name`), or `None` when no
    /// member has that id.
    pub fn name(id: i32) -> Option<&'static str> {
        Self::MEMBERS.iter().find(|m| m.1 == id).map(|m| m.0)
    }

    /// The id of the member called `name` (`BaseUnit[name]`).
    pub fn from_name(name: &str) -> Option<i32> {
        Self::MEMBERS.iter().find(|m| m.0 == name).map(|m| m.1)
    }
}

/// The size of one block of the base-unit id space. The space is sparse by
/// design: this is not a membership test.
pub const UNIT_BLOCK_SIZE: i32 = 10_000;
/// The first currency id.
pub const CURRENCY_FIRST: i32 = 900_000;

/// Prefix systems (`unit_prefix_system_t`).
pub struct PrefixSystem;

impl PrefixSystem {
    pub const SI: i32 = 0;
    pub const IEC: i32 = 1;
}

/// SI prefixes (`si_prefix_t`).
pub struct SIPrefix;

impl SIPrefix {
    pub const NONE: i32 = 0;
    pub const QUECTO: i32 = 1;
    pub const RONTO: i32 = 2;
    pub const YOCTO: i32 = 3;
    pub const ZEPTO: i32 = 4;
    pub const ATTO: i32 = 5;
    pub const FEMTO: i32 = 6;
    pub const PICO: i32 = 7;
    pub const NANO: i32 = 8;
    pub const MICRO: i32 = 9;
    pub const MILLI: i32 = 10;
    pub const CENTI: i32 = 11;
    pub const DECI: i32 = 12;
    pub const DECA: i32 = 13;
    pub const HECTO: i32 = 14;
    pub const KILO: i32 = 15;
    pub const MEGA: i32 = 16;
    pub const GIGA: i32 = 17;
    pub const TERA: i32 = 18;
    pub const PETA: i32 = 19;
    pub const EXA: i32 = 20;
    pub const ZETTA: i32 = 21;
    pub const YOTTA: i32 = 22;
    pub const RONNA: i32 = 23;
    pub const QUETTA: i32 = 24;
}

/// IEC binary prefixes (`iec_prefix_t`).
pub struct IECPrefix;

impl IECPrefix {
    pub const NONE: i32 = 0;
    pub const KIBI: i32 = 1;
    pub const MEBI: i32 = 2;
    pub const GIBI: i32 = 3;
    pub const TEBI: i32 = 4;
    pub const PEBI: i32 = 5;
    pub const EXBI: i32 = 6;
    pub const ZEBI: i32 = 7;
    pub const YOBI: i32 = 8;
    pub const ROBI: i32 = 9;
    pub const QUEBI: i32 = 10;
}

/// The smallest component exponent (`unit_exponent_t`).
pub const EXPONENT_MIN: i32 = -100;
/// The largest component exponent.
pub const EXPONENT_MAX: i32 = 100;

/// Named component exponents. Like Python's open `Exponent`, any integer in
/// `EXPONENT_MIN..=EXPONENT_MAX` except 0 is a valid exponent.
pub struct Exponent;

impl Exponent {
    pub const INVALID: i32 = 0;
    pub const LINEAR: i32 = 1;
    pub const SQUARE: i32 = 2;
    pub const CUBIC: i32 = 3;
    pub const QUARTIC: i32 = 4;
    pub const QUINTIC: i32 = 5;
    pub const SEXTIC: i32 = 6;
    pub const SEPTIC: i32 = 7;
    pub const OCTIC: i32 = 8;
    pub const NONIC: i32 = 9;
    pub const NEG_LINEAR: i32 = -1;
    pub const NEG_SQUARE: i32 = -2;
    pub const NEG_CUBIC: i32 = -3;
    pub const NEG_QUARTIC: i32 = -4;
    pub const NEG_QUINTIC: i32 = -5;
    pub const NEG_SEXTIC: i32 = -6;
    pub const NEG_SEPTIC: i32 = -7;
    pub const NEG_OCTIC: i32 = -8;
    pub const NEG_NONIC: i32 = -9;
}
