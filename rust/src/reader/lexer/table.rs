//! The lexer's transition tables, ported from `src/lexer/bovnar_state_table.c`.
//!
//! The C file builds these tables with designated initialisers and range
//! macros. They are reproduced here as data, row by row, from the compiled C
//! tables, so every (state, byte) cell is exactly what the reference uses.
//! Each row lists the byte ranges that map to an action; every byte not
//! listed maps to [`Act::None`] ("byte illegal in this state").

/// Lexer states (`state_t`), in the C declaration order.
///
/// The order is load-bearing: [`ROWS`] is indexed by it. A few states are
/// never entered (vestigial in C too, e.g. `number_outro_nosp`) but keep
/// their place so the rows line up.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum St {
    Undefined,
    FirstBom,
    Utf8bomIntro,
    CommentIntro,
    IgnoreCommentByte,
    CommentOutro,
    FirstCommentIntro,
    FirstCommentAfterEf,
    FirstCommentAfterEfBb,
    FirstCommentOutro,
    IdentifierIntro,
    IdentifierBody,
    IdentifierOutro,
    TypeIntro,
    TypeNoparams,
    TypeOutro,
    TypeBodyOutro,
    CopyTypeByte,
    TypeBodyWs,
    TfU,
    TfUi,
    TfUin,
    TfUt,
    TfUtf,
    TfS,
    TfSi,
    TfSin,
    TfF,
    TfFl,
    TfFlo,
    TfFloa,
    TfB,
    TfBo,
    TfBoo,
    TfD,
    TfDa,
    TfDat,
    TfDate,
    TfDatet,
    TfDateti,
    TfDatetim,
    ValueIntro,
    ArrayIntro,
    NewArrayValue,
    ArrayOutro,
    ArrayDimSep,
    NegNumberIntro,
    ZeroIntro,
    CopyNumberByte,
    FractionIntro,
    FractionNoInt,
    CopyFractionByte,
    NumberOutro,
    ExpIntro,
    ExpSignIntro,
    CopyExpByte,
    DtlitAfterY,
    DtlitMonth,
    DtlitAfterM,
    DtlitDay,
    DtlitAfterT,
    DtlitHour,
    DtlitAfterH,
    DtlitMin,
    DtlitAfterMi,
    DtlitSec,
    DtlitZulu,
    DtlitFrac,
    DtlitOffSign,
    DtlitOffHour,
    DtlitOffColon,
    DtlitOffMin,
    SymbolIntro,
    SymbolBody,
    SymbolOutro,
    ReferenceIntro,
    ReferenceSegmentIntro,
    ReferenceSegmentBody,
    ReferenceIndexFirst,
    ReferenceIndex,
    ReferenceOutro,
    CopyStringByte,
    EscapeFromCopy,
    EscX1,
    EscX2,
    EscUOpen,
    EscUHex,
    StringIntro,
    StringOutroNosp,
    StringOutro,
    OctetStreamIntro,
    OctetStreamOutro,
    StructIntro,
    StructOutro,
    ValueOutro,
    NumberOutroNosp,
    InlineUnitBody,
    InlineUnitOutro,
    Resync,
    ResyncString,
    ResyncStringEscape,
    ResyncComment,
    ResyncDot,
}

/// Number of states (`dimension_state`).
const ST_COUNT: usize = 103;

/// Action indices (`enum action_id`), in the C declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Act {
    None,
    IgnoreWhitespace,
    CommentIntro,
    IgnoreCommentByte,
    CommentOutro,
    CopyUtf8bomByte,
    IdentifierIntro,
    CopyIdentifierByte,
    ToIdentifierOutro,
    ValueIntro,
    ValueOutro,
    TypeIntro,
    TypeOutro,
    CopyTypeByte,
    CopyTypeSepByte,
    TypeWsByte,
    TypeParamWhitespace,
    NegNumberIntro,
    CopyNumberByte,
    ZeroIntro,
    FractionIntro,
    FractionNoInt,
    CopyFractionByte,
    ToNumberOutro,
    KwAdvance,
    TfToU,
    TfToS,
    TfToF,
    TfToD,
    TfDatetimeDone,
    TfUToUi,
    TfUToUt,
    TfToB,
    TfUintDone,
    TfSintDone,
    TfFloatDone,
    TfUtf8Done,
    TfBoolDone,
    ArrayIntro,
    ArrayOutro,
    ArrayOutroEmpty,
    NewArrayValue,
    ArrayDimSep,
    ExpIntro,
    ExpSignIntro,
    CopyExpByte,
    StringIntro,
    StringOutro,
    CopyStringByte,
    ReplaceEscapedByte,
    EscapeFromCopy,
    EscapeXIntro,
    EscapeUIntro,
    EscapeXD1,
    EscapeXD2,
    EscapeUOpen,
    EscapeUHex,
    ArrStringIntro,
    SymbolIntro,
    CopySymbolByte,
    ToSymbolOutro,
    ReferenceIntro,
    CopyReferenceDot,
    CopyReferenceByte,
    ReferenceIndexOpen,
    ReferenceIndexByte,
    ReferenceIndexClose,
    ToReferenceOutro,
    OctetStreamIntro,
    StructIntro,
    StructOutro,
    FirstCommentIntro,
    FirstCommentByte,
    FirstCommentAfterEf,
    FirstCommentAfterEfBb,
    FirstCommentOutro,
    TypeNullThenValueOutro,
    TypeNullThenNewArrayValue,
    TypeNullThenArrayOutro,
    ResyncSkip,
    ResyncOpenBracket,
    ResyncCloseBracket,
    ResyncSemicolon,
    ResyncStringIntro,
    ResyncStringByte,
    ResyncStringEscape,
    ResyncStringEscapeByte,
    ResyncStringOutro,
    ResyncCommentIntro,
    ResyncCommentByte,
    ResyncCommentOutro,
    ResyncDot,
    ResyncResumeIdentifier,
    InlineUnitIntro,
    CopyInlineUnitByte,
    ToInlineUnitOutro,
    ToStringOutro,
    DtlitAfterY,
    DtlitMonth,
    DtlitAfterM,
    DtlitDay,
    DtlitAfterT,
    DtlitHour,
    DtlitAfterH,
    DtlitMin,
    DtlitAfterMi,
    DtlitSec,
    DtlitZulu,
    DtlitFrac,
    DtlitOffSign,
    DtlitOffHour,
    DtlitOffColon,
    DtlitOffMin,
}

/// Byte ranges `(first, last, action)` of one state's row.
type Row = &'static [(u8, u8, Act)];

/// `bvn_after_state_idx_table`, one entry per state in [`St`] order.
const ROWS: [Row; ST_COUNT] = [
    // Undefined
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::FirstCommentIntro),
        (0x2e, 0x2e, Act::IdentifierIntro),
        (0xef, 0xef, Act::CopyUtf8bomByte),
    ],
    // FirstBom
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::FirstCommentIntro),
        (0x2e, 0x2e, Act::IdentifierIntro),
    ],
    // Utf8bomIntro
    &[(0x00, 0xff, Act::CopyUtf8bomByte)],
    // CommentIntro
    &[
        (0x09, 0x09, Act::IgnoreCommentByte),
        (0x0a, 0x0a, Act::CommentOutro),
        (0x0b, 0x0c, Act::IgnoreCommentByte),
        (0x0d, 0x0d, Act::CommentOutro),
        (0x20, 0x7e, Act::IgnoreCommentByte),
        (0x80, 0xff, Act::IgnoreCommentByte),
    ],
    // IgnoreCommentByte
    &[
        (0x09, 0x09, Act::IgnoreCommentByte),
        (0x0a, 0x0a, Act::CommentOutro),
        (0x0b, 0x0c, Act::IgnoreCommentByte),
        (0x0d, 0x0d, Act::CommentOutro),
        (0x20, 0x7e, Act::IgnoreCommentByte),
        (0x80, 0xff, Act::IgnoreCommentByte),
    ],
    // CommentOutro
    &[],
    // FirstCommentIntro
    &[
        (0x09, 0x09, Act::FirstCommentByte),
        (0x0a, 0x0a, Act::FirstCommentOutro),
        (0x0b, 0x0c, Act::FirstCommentByte),
        (0x0d, 0x0d, Act::FirstCommentOutro),
        (0x20, 0x7e, Act::FirstCommentByte),
        (0x80, 0xff, Act::FirstCommentByte),
    ],
    // FirstCommentAfterEf
    &[
        (0x09, 0x09, Act::FirstCommentAfterEf),
        (0x0a, 0x0a, Act::FirstCommentOutro),
        (0x0b, 0x0c, Act::FirstCommentAfterEf),
        (0x0d, 0x0d, Act::FirstCommentOutro),
        (0x20, 0x7e, Act::FirstCommentAfterEf),
        (0x80, 0xff, Act::FirstCommentAfterEf),
    ],
    // FirstCommentAfterEfBb
    &[
        (0x09, 0x09, Act::FirstCommentAfterEfBb),
        (0x0a, 0x0a, Act::FirstCommentOutro),
        (0x0b, 0x0c, Act::FirstCommentAfterEfBb),
        (0x0d, 0x0d, Act::FirstCommentOutro),
        (0x20, 0x7e, Act::FirstCommentAfterEfBb),
        (0x80, 0xff, Act::FirstCommentAfterEfBb),
    ],
    // FirstCommentOutro
    &[],
    // IdentifierIntro
    &[
        (0x09, 0x0d, Act::ToIdentifierOutro),
        (0x20, 0x20, Act::ToIdentifierOutro),
        (0x3d, 0x3d, Act::ValueIntro),
        (0x41, 0x5a, Act::CopyIdentifierByte),
        (0x5f, 0x5f, Act::CopyIdentifierByte),
        (0x61, 0x7a, Act::CopyIdentifierByte),
        (0xc3, 0xf4, Act::CopyIdentifierByte),
    ],
    // IdentifierBody
    &[
        (0x09, 0x0d, Act::ToIdentifierOutro),
        (0x20, 0x20, Act::ToIdentifierOutro),
        (0x2b, 0x2b, Act::CopyIdentifierByte),
        (0x2d, 0x2d, Act::CopyIdentifierByte),
        (0x30, 0x39, Act::CopyIdentifierByte),
        (0x3d, 0x3d, Act::ValueIntro),
        (0x41, 0x5a, Act::CopyIdentifierByte),
        (0x5f, 0x5f, Act::CopyIdentifierByte),
        (0x61, 0x7a, Act::CopyIdentifierByte),
        (0x80, 0xc1, Act::CopyIdentifierByte),
        (0xc3, 0xff, Act::CopyIdentifierByte),
    ],
    // IdentifierOutro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x3d, 0x3d, Act::ValueIntro),
    ],
    // TypeIntro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x62, 0x62, Act::TfToB),
        (0x64, 0x64, Act::TfToD),
        (0x66, 0x66, Act::TfToF),
        (0x73, 0x73, Act::TfToS),
        (0x75, 0x75, Act::TfToU),
    ],
    // TypeNoparams
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x3e, 0x3e, Act::TypeOutro),
    ],
    // TypeOutro
    &[
        (0x00, 0x00, Act::OctetStreamIntro),
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x22, 0x22, Act::StringIntro),
        (0x23, 0x23, Act::CommentIntro),
        (0x26, 0x26, Act::ReferenceIntro),
        (0x2c, 0x2c, Act::TypeNullThenNewArrayValue),
        (0x2d, 0x2d, Act::NegNumberIntro),
        (0x2e, 0x2e, Act::FractionNoInt),
        (0x30, 0x30, Act::ZeroIntro),
        (0x31, 0x39, Act::CopyNumberByte),
        (0x3b, 0x3b, Act::TypeNullThenValueOutro),
        (0x41, 0x5a, Act::SymbolIntro),
        (0x5b, 0x5b, Act::ArrayIntro),
        (0x5d, 0x5d, Act::TypeNullThenArrayOutro),
        (0x5f, 0x5f, Act::SymbolIntro),
        (0x61, 0x7a, Act::SymbolIntro),
        (0x7b, 0x7b, Act::StructIntro),
        (0xc3, 0xf4, Act::SymbolIntro),
    ],
    // TypeBodyOutro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x24, 0x25, Act::CopyTypeByte),
        (0x27, 0x2b, Act::CopyTypeByte),
        (0x2c, 0x2c, Act::CopyTypeSepByte),
        (0x2d, 0x39, Act::CopyTypeByte),
        (0x3a, 0x3a, Act::CopyTypeSepByte),
        (0x3e, 0x3e, Act::TypeOutro),
        (0x41, 0x5b, Act::CopyTypeByte),
        (0x5d, 0x5f, Act::CopyTypeByte),
        (0x61, 0x7b, Act::CopyTypeByte),
        (0x7d, 0x7e, Act::CopyTypeByte),
        (0x80, 0xbf, Act::CopyTypeByte),
        (0xc2, 0xf4, Act::CopyTypeByte),
    ],
    // CopyTypeByte
    &[
        (0x09, 0x0d, Act::TypeWsByte),
        (0x20, 0x20, Act::TypeWsByte),
        (0x24, 0x25, Act::CopyTypeByte),
        (0x27, 0x2b, Act::CopyTypeByte),
        (0x2c, 0x2c, Act::CopyTypeSepByte),
        (0x2d, 0x39, Act::CopyTypeByte),
        (0x3a, 0x3a, Act::CopyTypeSepByte),
        (0x3e, 0x3e, Act::TypeOutro),
        (0x41, 0x5b, Act::CopyTypeByte),
        (0x5d, 0x5f, Act::CopyTypeByte),
        (0x61, 0x7b, Act::CopyTypeByte),
        (0x7d, 0x7e, Act::CopyTypeByte),
        (0x80, 0xbf, Act::CopyTypeByte),
        (0xc2, 0xf4, Act::CopyTypeByte),
    ],
    // TypeBodyWs
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x24, 0x25, Act::TypeParamWhitespace),
        (0x27, 0x2b, Act::TypeParamWhitespace),
        (0x2c, 0x2c, Act::CopyTypeSepByte),
        (0x2d, 0x39, Act::TypeParamWhitespace),
        (0x3a, 0x3a, Act::CopyTypeSepByte),
        (0x3e, 0x3e, Act::TypeOutro),
        (0x41, 0x5b, Act::TypeParamWhitespace),
        (0x5d, 0x5f, Act::TypeParamWhitespace),
        (0x61, 0x7b, Act::TypeParamWhitespace),
        (0x7d, 0x7e, Act::TypeParamWhitespace),
        (0x80, 0xbf, Act::TypeParamWhitespace),
        (0xc2, 0xf4, Act::TypeParamWhitespace),
    ],
    // TfU
    &[(0x69, 0x69, Act::TfUToUi), (0x74, 0x74, Act::TfUToUt)],
    // TfUi
    &[(0x6e, 0x6e, Act::KwAdvance)],
    // TfUin
    &[(0x74, 0x74, Act::TfUintDone)],
    // TfUt
    &[(0x66, 0x66, Act::KwAdvance)],
    // TfUtf
    &[(0x38, 0x38, Act::TfUtf8Done)],
    // TfS
    &[(0x69, 0x69, Act::KwAdvance)],
    // TfSi
    &[(0x6e, 0x6e, Act::KwAdvance)],
    // TfSin
    &[(0x74, 0x74, Act::TfSintDone)],
    // TfF
    &[(0x6c, 0x6c, Act::KwAdvance)],
    // TfFl
    &[(0x6f, 0x6f, Act::KwAdvance)],
    // TfFlo
    &[(0x61, 0x61, Act::KwAdvance)],
    // TfFloa
    &[(0x74, 0x74, Act::TfFloatDone)],
    // TfB
    &[(0x6f, 0x6f, Act::KwAdvance)],
    // TfBo
    &[(0x6f, 0x6f, Act::KwAdvance)],
    // TfBoo
    &[(0x6c, 0x6c, Act::TfBoolDone)],
    // TfD
    &[(0x61, 0x61, Act::KwAdvance)],
    // TfDa
    &[(0x74, 0x74, Act::KwAdvance)],
    // TfDat
    &[(0x65, 0x65, Act::KwAdvance)],
    // TfDate
    &[(0x74, 0x74, Act::KwAdvance)],
    // TfDatet
    &[(0x69, 0x69, Act::KwAdvance)],
    // TfDateti
    &[(0x6d, 0x6d, Act::KwAdvance)],
    // TfDatetim
    &[(0x65, 0x65, Act::TfDatetimeDone)],
    // ValueIntro
    &[
        (0x00, 0x00, Act::OctetStreamIntro),
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x22, 0x22, Act::StringIntro),
        (0x23, 0x23, Act::CommentIntro),
        (0x26, 0x26, Act::ReferenceIntro),
        (0x2d, 0x2d, Act::NegNumberIntro),
        (0x2e, 0x2e, Act::FractionNoInt),
        (0x30, 0x30, Act::ZeroIntro),
        (0x31, 0x39, Act::CopyNumberByte),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x3c, 0x3c, Act::TypeIntro),
        (0x41, 0x5a, Act::SymbolIntro),
        (0x5b, 0x5b, Act::ArrayIntro),
        (0x5f, 0x5f, Act::SymbolIntro),
        (0x61, 0x7a, Act::SymbolIntro),
        (0x7b, 0x7b, Act::StructIntro),
        (0xc3, 0xf4, Act::SymbolIntro),
    ],
    // ArrayIntro
    &[
        (0x00, 0x00, Act::OctetStreamIntro),
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x22, 0x22, Act::ArrStringIntro),
        (0x23, 0x23, Act::CommentIntro),
        (0x26, 0x26, Act::ReferenceIntro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x2d, 0x2d, Act::NegNumberIntro),
        (0x2e, 0x2e, Act::FractionNoInt),
        (0x30, 0x30, Act::ZeroIntro),
        (0x31, 0x39, Act::CopyNumberByte),
        (0x3c, 0x3c, Act::TypeIntro),
        (0x41, 0x5a, Act::SymbolIntro),
        (0x5b, 0x5b, Act::ArrayIntro),
        (0x5d, 0x5d, Act::ArrayOutroEmpty),
        (0x5f, 0x5f, Act::SymbolIntro),
        (0x61, 0x7a, Act::SymbolIntro),
        (0x7b, 0x7b, Act::StructIntro),
        (0xc3, 0xf4, Act::SymbolIntro),
    ],
    // NewArrayValue
    &[
        (0x00, 0x00, Act::OctetStreamIntro),
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x22, 0x22, Act::ArrStringIntro),
        (0x23, 0x23, Act::CommentIntro),
        (0x26, 0x26, Act::ReferenceIntro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x2d, 0x2d, Act::NegNumberIntro),
        (0x2e, 0x2e, Act::FractionNoInt),
        (0x30, 0x30, Act::ZeroIntro),
        (0x31, 0x39, Act::CopyNumberByte),
        (0x3c, 0x3c, Act::TypeIntro),
        (0x41, 0x5a, Act::SymbolIntro),
        (0x5b, 0x5b, Act::ArrayIntro),
        (0x5d, 0x5d, Act::ArrayOutro),
        (0x5f, 0x5f, Act::SymbolIntro),
        (0x61, 0x7a, Act::SymbolIntro),
        (0x7b, 0x7b, Act::StructIntro),
        (0xc3, 0xf4, Act::SymbolIntro),
    ],
    // ArrayOutro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x2f, 0x2f, Act::ArrayDimSep),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // ArrayDimSep
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x5b, 0x5b, Act::ArrayIntro),
    ],
    // NegNumberIntro
    &[
        (0x2e, 0x2e, Act::FractionNoInt),
        (0x30, 0x30, Act::ZeroIntro),
        (0x31, 0x39, Act::CopyNumberByte),
    ],
    // ZeroIntro
    &[
        (0x09, 0x0d, Act::ToNumberOutro),
        (0x20, 0x20, Act::ToNumberOutro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x2d, 0x2d, Act::DtlitAfterY),
        (0x2e, 0x2e, Act::FractionIntro),
        (0x30, 0x39, Act::CopyNumberByte),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x45, 0x45, Act::ExpIntro),
        (0x5d, 0x5d, Act::ArrayOutro),
        (0x65, 0x65, Act::ExpIntro),
    ],
    // CopyNumberByte
    &[
        (0x09, 0x0d, Act::ToNumberOutro),
        (0x20, 0x20, Act::ToNumberOutro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x2d, 0x2d, Act::DtlitAfterY),
        (0x2e, 0x2e, Act::FractionIntro),
        (0x30, 0x39, Act::CopyNumberByte),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x45, 0x45, Act::ExpIntro),
        (0x5d, 0x5d, Act::ArrayOutro),
        (0x65, 0x65, Act::ExpIntro),
    ],
    // FractionIntro
    &[
        (0x09, 0x0d, Act::ToNumberOutro),
        (0x20, 0x20, Act::ToNumberOutro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x30, 0x39, Act::CopyFractionByte),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x45, 0x45, Act::ExpIntro),
        (0x5d, 0x5d, Act::ArrayOutro),
        (0x65, 0x65, Act::ExpIntro),
    ],
    // FractionNoInt
    &[(0x30, 0x39, Act::CopyFractionByte)],
    // CopyFractionByte
    &[
        (0x09, 0x0d, Act::ToNumberOutro),
        (0x20, 0x20, Act::ToNumberOutro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x30, 0x39, Act::CopyFractionByte),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x45, 0x45, Act::ExpIntro),
        (0x5d, 0x5d, Act::ArrayOutro),
        (0x65, 0x65, Act::ExpIntro),
    ],
    // NumberOutro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x24, 0x25, Act::InlineUnitIntro),
        (0x28, 0x28, Act::InlineUnitIntro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x41, 0x5a, Act::InlineUnitIntro),
        (0x5d, 0x5d, Act::ArrayOutro),
        (0x5f, 0x5f, Act::InlineUnitIntro),
        (0x61, 0x7a, Act::InlineUnitIntro),
        (0xc2, 0xf4, Act::InlineUnitIntro),
    ],
    // ExpIntro
    &[
        (0x2b, 0x2b, Act::ExpSignIntro),
        (0x2d, 0x2d, Act::ExpSignIntro),
        (0x30, 0x39, Act::CopyExpByte),
    ],
    // ExpSignIntro
    &[(0x30, 0x39, Act::CopyExpByte)],
    // CopyExpByte
    &[
        (0x09, 0x0d, Act::ToNumberOutro),
        (0x20, 0x20, Act::ToNumberOutro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x30, 0x39, Act::CopyExpByte),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // DtlitAfterY
    &[(0x30, 0x39, Act::DtlitMonth)],
    // DtlitMonth
    &[
        (0x2d, 0x2d, Act::DtlitAfterM),
        (0x30, 0x39, Act::DtlitMonth),
    ],
    // DtlitAfterM
    &[(0x30, 0x39, Act::DtlitDay)],
    // DtlitDay
    &[
        (0x09, 0x0d, Act::ToNumberOutro),
        (0x20, 0x20, Act::ToNumberOutro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x30, 0x39, Act::DtlitDay),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x54, 0x54, Act::DtlitAfterT),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // DtlitAfterT
    &[(0x30, 0x39, Act::DtlitHour)],
    // DtlitHour
    &[(0x30, 0x39, Act::DtlitHour), (0x3a, 0x3a, Act::DtlitAfterH)],
    // DtlitAfterH
    &[(0x30, 0x39, Act::DtlitMin)],
    // DtlitMin
    &[(0x30, 0x39, Act::DtlitMin), (0x3a, 0x3a, Act::DtlitAfterMi)],
    // DtlitAfterMi
    &[(0x30, 0x39, Act::DtlitSec)],
    // DtlitSec
    &[
        (0x09, 0x0d, Act::ToNumberOutro),
        (0x20, 0x20, Act::ToNumberOutro),
        (0x2b, 0x2b, Act::DtlitOffSign),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x2d, 0x2d, Act::DtlitOffSign),
        (0x2e, 0x2e, Act::DtlitFrac),
        (0x30, 0x39, Act::DtlitSec),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x5a, 0x5a, Act::DtlitZulu),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // DtlitZulu
    &[
        (0x09, 0x0d, Act::ToNumberOutro),
        (0x20, 0x20, Act::ToNumberOutro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // DtlitFrac
    &[
        (0x09, 0x0d, Act::ToNumberOutro),
        (0x20, 0x20, Act::ToNumberOutro),
        (0x2b, 0x2b, Act::DtlitOffSign),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x2d, 0x2d, Act::DtlitOffSign),
        (0x30, 0x39, Act::DtlitFrac),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x5a, 0x5a, Act::DtlitZulu),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // DtlitOffSign
    &[(0x30, 0x39, Act::DtlitOffHour)],
    // DtlitOffHour
    &[
        (0x30, 0x39, Act::DtlitOffHour),
        (0x3a, 0x3a, Act::DtlitOffColon),
    ],
    // DtlitOffColon
    &[(0x30, 0x39, Act::DtlitOffMin)],
    // DtlitOffMin
    &[
        (0x09, 0x0d, Act::ToNumberOutro),
        (0x20, 0x20, Act::ToNumberOutro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x30, 0x39, Act::DtlitOffMin),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // SymbolIntro
    &[],
    // SymbolBody
    &[
        (0x09, 0x0d, Act::ToSymbolOutro),
        (0x20, 0x20, Act::ToSymbolOutro),
        (0x2b, 0x2b, Act::CopySymbolByte),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x2d, 0x2d, Act::CopySymbolByte),
        (0x30, 0x39, Act::CopySymbolByte),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x41, 0x5a, Act::CopySymbolByte),
        (0x5d, 0x5d, Act::ArrayOutro),
        (0x5f, 0x5f, Act::CopySymbolByte),
        (0x61, 0x7a, Act::CopySymbolByte),
        (0x80, 0xc1, Act::CopySymbolByte),
        (0xc3, 0xff, Act::CopySymbolByte),
    ],
    // SymbolOutro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // ReferenceIntro
    &[(0x2e, 0x2e, Act::CopyReferenceDot)],
    // ReferenceSegmentIntro
    &[
        (0x41, 0x5a, Act::CopyReferenceByte),
        (0x5f, 0x5f, Act::CopyReferenceByte),
        (0x61, 0x7a, Act::CopyReferenceByte),
        (0xc3, 0xf4, Act::CopyReferenceByte),
    ],
    // ReferenceSegmentBody
    &[
        (0x09, 0x0d, Act::ToReferenceOutro),
        (0x20, 0x20, Act::ToReferenceOutro),
        (0x2b, 0x2b, Act::CopyReferenceByte),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x2d, 0x2d, Act::CopyReferenceByte),
        (0x2e, 0x2e, Act::CopyReferenceDot),
        (0x30, 0x39, Act::CopyReferenceByte),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x41, 0x5a, Act::CopyReferenceByte),
        (0x5b, 0x5b, Act::ReferenceIndexOpen),
        (0x5d, 0x5d, Act::ArrayOutro),
        (0x5f, 0x5f, Act::CopyReferenceByte),
        (0x61, 0x7a, Act::CopyReferenceByte),
        (0x80, 0xc1, Act::CopyReferenceByte),
        (0xc3, 0xff, Act::CopyReferenceByte),
    ],
    // ReferenceIndexFirst
    &[(0x30, 0x39, Act::ReferenceIndexByte)],
    // ReferenceIndex
    &[
        (0x30, 0x39, Act::ReferenceIndexByte),
        (0x5d, 0x5d, Act::ReferenceIndexClose),
    ],
    // ReferenceOutro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // CopyStringByte
    &[
        (0x09, 0x0d, Act::CopyStringByte),
        (0x20, 0x21, Act::CopyStringByte),
        (0x22, 0x22, Act::StringOutro),
        (0x23, 0x5b, Act::CopyStringByte),
        (0x5c, 0x5c, Act::EscapeFromCopy),
        (0x5d, 0x7e, Act::CopyStringByte),
        (0x80, 0xff, Act::CopyStringByte),
    ],
    // EscapeFromCopy
    &[
        (0x00, 0x74, Act::ReplaceEscapedByte),
        (0x75, 0x75, Act::EscapeUIntro),
        (0x76, 0x77, Act::ReplaceEscapedByte),
        (0x78, 0x78, Act::EscapeXIntro),
        (0x79, 0xff, Act::ReplaceEscapedByte),
    ],
    // EscX1
    &[(0x00, 0xff, Act::EscapeXD1)],
    // EscX2
    &[(0x00, 0xff, Act::EscapeXD2)],
    // EscUOpen
    &[(0x00, 0xff, Act::EscapeUOpen)],
    // EscUHex
    &[(0x00, 0xff, Act::EscapeUHex)],
    // StringIntro
    &[
        (0x09, 0x0d, Act::CopyStringByte),
        (0x20, 0x21, Act::CopyStringByte),
        (0x22, 0x22, Act::StringOutro),
        (0x23, 0x5b, Act::CopyStringByte),
        (0x5c, 0x5c, Act::EscapeFromCopy),
        (0x5d, 0x7e, Act::CopyStringByte),
        (0x80, 0xff, Act::CopyStringByte),
    ],
    // StringOutroNosp
    &[
        (0x09, 0x0d, Act::ToStringOutro),
        (0x20, 0x20, Act::ToStringOutro),
        (0x22, 0x22, Act::StringIntro),
        (0x23, 0x23, Act::CommentIntro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // StringOutro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x22, 0x22, Act::StringIntro),
        (0x23, 0x23, Act::CommentIntro),
        (0x24, 0x25, Act::InlineUnitIntro),
        (0x28, 0x28, Act::InlineUnitIntro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x41, 0x5a, Act::InlineUnitIntro),
        (0x5d, 0x5d, Act::ArrayOutro),
        (0x5f, 0x5f, Act::InlineUnitIntro),
        (0x61, 0x7a, Act::InlineUnitIntro),
        (0xc2, 0xf4, Act::InlineUnitIntro),
    ],
    // OctetStreamIntro
    &[],
    // OctetStreamOutro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // StructIntro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x2e, 0x2e, Act::IdentifierIntro),
        (0x7d, 0x7d, Act::StructOutro),
    ],
    // StructOutro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // ValueOutro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x2e, 0x2e, Act::IdentifierIntro),
        (0x7d, 0x7d, Act::StructOutro),
    ],
    // NumberOutroNosp
    &[
        (0x09, 0x0d, Act::ToNumberOutro),
        (0x20, 0x20, Act::ToNumberOutro),
        (0x23, 0x23, Act::CommentIntro),
        (0x2c, 0x2c, Act::NewArrayValue),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x5d, 0x5d, Act::ArrayOutro),
    ],
    // InlineUnitBody
    &[
        (0x09, 0x0d, Act::ToInlineUnitOutro),
        (0x20, 0x20, Act::ToInlineUnitOutro),
        (0x23, 0x23, Act::CommentIntro),
        (0x24, 0x25, Act::CopyInlineUnitByte),
        (0x27, 0x2b, Act::CopyInlineUnitByte),
        (0x2d, 0x3a, Act::CopyInlineUnitByte),
        (0x3b, 0x3b, Act::ValueOutro),
        (0x41, 0x5b, Act::CopyInlineUnitByte),
        (0x5d, 0x5f, Act::CopyInlineUnitByte),
        (0x61, 0x7b, Act::CopyInlineUnitByte),
        (0x7d, 0x7e, Act::CopyInlineUnitByte),
        (0x80, 0xbf, Act::CopyInlineUnitByte),
        (0xc2, 0xf4, Act::CopyInlineUnitByte),
    ],
    // InlineUnitOutro
    &[
        (0x09, 0x0d, Act::IgnoreWhitespace),
        (0x20, 0x20, Act::IgnoreWhitespace),
        (0x23, 0x23, Act::CommentIntro),
        (0x3b, 0x3b, Act::ValueOutro),
    ],
    // Resync
    &[
        (0x00, 0x21, Act::ResyncSkip),
        (0x22, 0x22, Act::ResyncStringIntro),
        (0x23, 0x23, Act::ResyncCommentIntro),
        (0x24, 0x2d, Act::ResyncSkip),
        (0x2e, 0x2e, Act::ResyncDot),
        (0x2f, 0x3a, Act::ResyncSkip),
        (0x3b, 0x3b, Act::ResyncSemicolon),
        (0x3c, 0x5a, Act::ResyncSkip),
        (0x5b, 0x5b, Act::ResyncOpenBracket),
        (0x5c, 0x5c, Act::ResyncSkip),
        (0x5d, 0x5d, Act::ResyncCloseBracket),
        (0x5e, 0x7a, Act::ResyncSkip),
        (0x7b, 0x7b, Act::ResyncOpenBracket),
        (0x7c, 0x7c, Act::ResyncSkip),
        (0x7d, 0x7d, Act::ResyncCloseBracket),
        (0x7e, 0xff, Act::ResyncSkip),
    ],
    // ResyncString
    &[
        (0x00, 0x21, Act::ResyncStringByte),
        (0x22, 0x22, Act::ResyncStringOutro),
        (0x23, 0x5b, Act::ResyncStringByte),
        (0x5c, 0x5c, Act::ResyncStringEscape),
        (0x5d, 0xff, Act::ResyncStringByte),
    ],
    // ResyncStringEscape
    &[(0x00, 0xff, Act::ResyncStringEscapeByte)],
    // ResyncComment
    &[
        (0x00, 0x09, Act::ResyncCommentByte),
        (0x0a, 0x0a, Act::ResyncCommentOutro),
        (0x0b, 0x0c, Act::ResyncCommentByte),
        (0x0d, 0x0d, Act::ResyncCommentOutro),
        (0x0e, 0xff, Act::ResyncCommentByte),
    ],
    // ResyncDot
    &[
        (0x00, 0x21, Act::ResyncSkip),
        (0x22, 0x22, Act::ResyncStringIntro),
        (0x23, 0x23, Act::ResyncCommentIntro),
        (0x24, 0x2d, Act::ResyncSkip),
        (0x2e, 0x2e, Act::ResyncDot),
        (0x2f, 0x3a, Act::ResyncSkip),
        (0x3b, 0x3b, Act::ResyncSemicolon),
        (0x3c, 0x40, Act::ResyncSkip),
        (0x41, 0x5a, Act::ResyncResumeIdentifier),
        (0x5b, 0x5b, Act::ResyncOpenBracket),
        (0x5c, 0x5c, Act::ResyncSkip),
        (0x5d, 0x5d, Act::ResyncCloseBracket),
        (0x5e, 0x5e, Act::ResyncSkip),
        (0x5f, 0x5f, Act::ResyncResumeIdentifier),
        (0x60, 0x60, Act::ResyncSkip),
        (0x61, 0x7a, Act::ResyncResumeIdentifier),
        (0x7b, 0x7b, Act::ResyncOpenBracket),
        (0x7c, 0x7c, Act::ResyncSkip),
        (0x7d, 0x7d, Act::ResyncCloseBracket),
        (0x7e, 0xc2, Act::ResyncSkip),
        (0xc3, 0xf4, Act::ResyncResumeIdentifier),
        (0xf5, 0xff, Act::ResyncSkip),
    ],
];

/// Expands [`ROWS`] into the dense table at compile time.
const fn build() -> [[Act; 256]; ST_COUNT] {
    let mut t = [[Act::None; 256]; ST_COUNT];
    let mut s = 0;
    while s < ST_COUNT {
        let row = ROWS[s];
        let mut i = 0;
        while i < row.len() {
            let (lo, hi, act) = row[i];
            let mut b = lo as usize;
            while b <= hi as usize {
                t[s][b] = act;
                b += 1;
            }
            i += 1;
        }
        s += 1;
    }
    t
}

static TABLE: [[Act; 256]; ST_COUNT] = build();

/// `bvn_after_state_idx_table[state][byte]`.
pub(super) fn action(state: St, byte: u8) -> Act {
    // Cannot go out of bounds: `St` has exactly `ST_COUNT` variants.
    TABLE[state as usize][usize::from(byte)]
}

/// `bvn_action_target_state`: the next state of a pure transition. Actions
/// whose handler decides the state itself have no entry, which C stores as
/// 0, i.e. `undefined`.
pub(super) fn target(act: Act) -> St {
    match act {
        Act::IgnoreCommentByte => St::IgnoreCommentByte,
        Act::ToIdentifierOutro => St::IdentifierOutro,
        Act::ToNumberOutro => St::NumberOutro,
        Act::TfToU => St::TfU,
        Act::TfToS => St::TfS,
        Act::TfToF => St::TfF,
        Act::TfToD => St::TfD,
        Act::TfUToUi => St::TfUi,
        Act::TfUToUt => St::TfUt,
        Act::TfToB => St::TfB,
        Act::StringOutro => St::StringOutroNosp,
        Act::EscapeFromCopy => St::EscapeFromCopy,
        Act::ToSymbolOutro => St::SymbolOutro,
        Act::ToReferenceOutro => St::ReferenceOutro,
        Act::ToStringOutro => St::StringOutro,
        Act::DtlitAfterY => St::DtlitAfterY,
        Act::DtlitMonth => St::DtlitMonth,
        Act::DtlitAfterM => St::DtlitAfterM,
        Act::DtlitDay => St::DtlitDay,
        Act::DtlitAfterT => St::DtlitAfterT,
        Act::DtlitHour => St::DtlitHour,
        Act::DtlitAfterH => St::DtlitAfterH,
        Act::DtlitMin => St::DtlitMin,
        Act::DtlitAfterMi => St::DtlitAfterMi,
        Act::DtlitSec => St::DtlitSec,
        Act::DtlitZulu => St::DtlitZulu,
        Act::DtlitFrac => St::DtlitFrac,
        Act::DtlitOffSign => St::DtlitOffSign,
        Act::DtlitOffHour => St::DtlitOffHour,
        Act::DtlitOffColon => St::DtlitOffColon,
        Act::DtlitOffMin => St::DtlitOffMin,
        _ => St::Undefined,
    }
}

/// `bvn_kw_advance_state`: the next letter state of a type keyword.
pub(super) fn kw_advance(state: St) -> St {
    match state {
        St::TfUi => St::TfUin,
        St::TfUt => St::TfUtf,
        St::TfS => St::TfSi,
        St::TfSi => St::TfSin,
        St::TfF => St::TfFl,
        St::TfFl => St::TfFlo,
        St::TfFlo => St::TfFloa,
        St::TfB => St::TfBo,
        St::TfBo => St::TfBoo,
        St::TfD => St::TfDa,
        St::TfDa => St::TfDat,
        St::TfDat => St::TfDate,
        St::TfDate => St::TfDatet,
        St::TfDatet => St::TfDateti,
        St::TfDateti => St::TfDatetim,
        // Unset C entries are 0, i.e. `undefined`.
        _ => St::Undefined,
    }
}
