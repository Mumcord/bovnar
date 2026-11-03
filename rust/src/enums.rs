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

include!(concat!(env!("OUT_DIR"), "/generated/base_units.rs"));

impl BaseUnit {
    /// The Python member name (`BaseUnit.METER.name` is `"METER"`).
    pub fn name(self) -> &'static str {
        Self::MEMBERS
            .iter()
            .find(|m| m.1 == self)
            .map_or("NONE", |m| m.0)
    }

    /// The member with the Python name `name` (`BaseUnit[name]`).
    pub fn from_name(name: &str) -> Option<Self> {
        Self::MEMBERS.iter().find(|m| m.0 == name).map(|m| m.1)
    }
}

/// The Python member name, as `str(BaseUnit.METER)` gives `METER`.
impl std::fmt::Display for BaseUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The size of one block of the base-unit id space. The space is sparse by
/// design: this is not a membership test.
pub const UNIT_BLOCK_SIZE: i32 = 10_000;
/// The first currency id.
pub const CURRENCY_FIRST: i32 = 900_000;

/// Prefix systems (`unit_prefix_system_t`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum PrefixSystem {
    #[default]
    Si = 0,
    Iec = 1,
}

impl PrefixSystem {
    /// The variant for a raw C value, or `None` for a value this binding
    /// does not know.
    pub fn from_raw(raw: i32) -> Option<Self> {
        Some(match raw {
            0 => Self::Si,
            1 => Self::Iec,
            _ => return None,
        })
    }
}

/// SI prefixes (`si_prefix_t`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum SIPrefix {
    #[default]
    None = 0,
    Quecto = 1,
    Ronto = 2,
    Yocto = 3,
    Zepto = 4,
    Atto = 5,
    Femto = 6,
    Pico = 7,
    Nano = 8,
    Micro = 9,
    Milli = 10,
    Centi = 11,
    Deci = 12,
    Deca = 13,
    Hecto = 14,
    Kilo = 15,
    Mega = 16,
    Giga = 17,
    Tera = 18,
    Peta = 19,
    Exa = 20,
    Zetta = 21,
    Yotta = 22,
    Ronna = 23,
    Quetta = 24,
}

impl SIPrefix {
    const ALL: [Self; 25] = [
        Self::None,
        Self::Quecto,
        Self::Ronto,
        Self::Yocto,
        Self::Zepto,
        Self::Atto,
        Self::Femto,
        Self::Pico,
        Self::Nano,
        Self::Micro,
        Self::Milli,
        Self::Centi,
        Self::Deci,
        Self::Deca,
        Self::Hecto,
        Self::Kilo,
        Self::Mega,
        Self::Giga,
        Self::Tera,
        Self::Peta,
        Self::Exa,
        Self::Zetta,
        Self::Yotta,
        Self::Ronna,
        Self::Quetta,
    ];

    /// The variant for a raw C value, or `None` for a value this binding
    /// does not know.
    pub fn from_raw(raw: i32) -> Option<Self> {
        Self::ALL.get(usize::try_from(raw).ok()?).copied()
    }
}

/// IEC binary prefixes (`iec_prefix_t`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum IECPrefix {
    #[default]
    None = 0,
    Kibi = 1,
    Mebi = 2,
    Gibi = 3,
    Tebi = 4,
    Pebi = 5,
    Exbi = 6,
    Zebi = 7,
    Yobi = 8,
    Robi = 9,
    Quebi = 10,
}

impl IECPrefix {
    const ALL: [Self; 11] = [
        Self::None,
        Self::Kibi,
        Self::Mebi,
        Self::Gibi,
        Self::Tebi,
        Self::Pebi,
        Self::Exbi,
        Self::Zebi,
        Self::Yobi,
        Self::Robi,
        Self::Quebi,
    ];

    /// The variant for a raw C value, or `None` for a value this binding
    /// does not know.
    pub fn from_raw(raw: i32) -> Option<Self> {
        Self::ALL.get(usize::try_from(raw).ok()?).copied()
    }
}

/// A component's prefix: an SI or an IEC prefix (`value_unit_prefix_t`, the
/// system and the id in one value). Python's pair of `prefix_system` and
/// `prefix`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prefix {
    Si(SIPrefix),
    Iec(IECPrefix),
}

impl Default for Prefix {
    fn default() -> Self {
        Prefix::Si(SIPrefix::None)
    }
}

impl Prefix {
    /// The prefix system.
    pub fn system(self) -> PrefixSystem {
        match self {
            Prefix::Si(_) => PrefixSystem::Si,
            Prefix::Iec(_) => PrefixSystem::Iec,
        }
    }

    /// The prefix id within its system (0 for no prefix).
    pub fn id(self) -> i32 {
        match self {
            Prefix::Si(p) => p as i32,
            Prefix::Iec(p) => p as i32,
        }
    }

    /// True for no prefix, in either system.
    pub fn is_none(self) -> bool {
        self.id() == 0
    }

    /// The prefix for a raw C `(system, id)`, or `None` for a value this
    /// binding does not know.
    pub fn from_raw(system: i32, id: i32) -> Option<Self> {
        match PrefixSystem::from_raw(system)? {
            PrefixSystem::Si => SIPrefix::from_raw(id).map(Prefix::Si),
            PrefixSystem::Iec => IECPrefix::from_raw(id).map(Prefix::Iec),
        }
    }
}

/// A component's prefix compares equal to the bare SI or IEC prefix it is,
/// as Python compares `prefix` alone.
impl PartialEq<SIPrefix> for Prefix {
    fn eq(&self, other: &SIPrefix) -> bool {
        *self == Prefix::Si(*other)
    }
}

impl PartialEq<IECPrefix> for Prefix {
    fn eq(&self, other: &IECPrefix) -> bool {
        *self == Prefix::Iec(*other)
    }
}

impl From<SIPrefix> for Prefix {
    fn from(p: SIPrefix) -> Self {
        Prefix::Si(p)
    }
}

impl From<IECPrefix> for Prefix {
    fn from(p: IECPrefix) -> Self {
        Prefix::Iec(p)
    }
}

/// The smallest component exponent (`unit_exponent_t`).
pub const EXPONENT_MIN: i32 = -100;
/// The largest component exponent.
pub const EXPONENT_MAX: i32 = 100;

/// A component's exponent (`unit_exponent_t`). Like Python's deliberately
/// open `Exponent`, any integer in `EXPONENT_MIN..=EXPONENT_MAX` is one, not
/// only the named constants: a unit read off the wire may carry any of them,
/// so this is a checked newtype rather than a closed enum. Zero is
/// [`Exponent::INVALID`]: a component with exponent 0 is malformed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Exponent(i32);

impl Exponent {
    pub const INVALID: Self = Self(0);
    pub const LINEAR: Self = Self(1);
    pub const SQUARE: Self = Self(2);
    pub const CUBIC: Self = Self(3);
    pub const QUARTIC: Self = Self(4);
    pub const QUINTIC: Self = Self(5);
    pub const SEXTIC: Self = Self(6);
    pub const SEPTIC: Self = Self(7);
    pub const OCTIC: Self = Self(8);
    pub const NONIC: Self = Self(9);
    pub const NEG_LINEAR: Self = Self(-1);
    pub const NEG_SQUARE: Self = Self(-2);
    pub const NEG_CUBIC: Self = Self(-3);
    pub const NEG_QUARTIC: Self = Self(-4);
    pub const NEG_QUINTIC: Self = Self(-5);
    pub const NEG_SEXTIC: Self = Self(-6);
    pub const NEG_SEPTIC: Self = Self(-7);
    pub const NEG_OCTIC: Self = Self(-8);
    pub const NEG_NONIC: Self = Self(-9);

    /// The exponent `raw`, or `None` outside `EXPONENT_MIN..=EXPONENT_MAX`.
    pub fn from_raw(raw: i32) -> Option<Self> {
        (EXPONENT_MIN..=EXPONENT_MAX)
            .contains(&raw)
            .then_some(Self(raw))
    }

    /// The exponent as an integer.
    pub fn get(self) -> i32 {
        self.0
    }
}

/// A raw C value that names no variant: the `TryFrom<i32>` error of every
/// enum here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownValue {
    /// The enum's name, such as `"SIPrefix"`.
    pub type_name: &'static str,
    pub value: i32,
}

impl std::fmt::Display for UnknownValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} has no value {}", self.type_name, self.value)
    }
}

impl std::error::Error for UnknownValue {}

/// `TryFrom<i32>` (through `from_raw`) and `From<_> for i32` for the C enums.
macro_rules! raw_conversions {
    ($($t:ident => $to_raw:expr),* $(,)?) => {$(
        impl TryFrom<i32> for $t {
            type Error = UnknownValue;
            fn try_from(raw: i32) -> std::result::Result<Self, UnknownValue> {
                $t::from_raw(raw).ok_or(UnknownValue {
                    type_name: stringify!($t),
                    value: raw,
                })
            }
        }

        impl From<$t> for i32 {
            fn from(v: $t) -> i32 {
                let to_raw: fn($t) -> i32 = $to_raw;
                to_raw(v)
            }
        }
    )*};
}

raw_conversions!(
    Event => |v| v as i32,
    TokenType => |v| v as i32,
    ValueTypeFamily => |v| v as i32,
    ErrorCode => |v| v as i32,
    BaseUnit => |v| v as i32,
    PrefixSystem => |v| v as i32,
    SIPrefix => |v| v as i32,
    IECPrefix => |v| v as i32,
    Exponent => Exponent::get,
);
