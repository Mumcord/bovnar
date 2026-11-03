---
title: "The Bovnar (BVNR) Unit-Safe Serialization Format"
abbrev: "The Bovnar Format"
docname: draft-sonntag-bovnar-00
category: info
submissiontype: independent
ipr: trust200902
area: "Applications and Real-Time"
workgroup: "Independent Submission"
consensus: false
v: 3
keyword:
  - serialization
  - units of measure
  - self-describing
  - dimensional analysis
  - scientific data
venue:
  home: https://www.bovnar.io
  repo: https://github.com/sothis/bovnar

author:
  -
    fullname: Janos Sonntag
    email: bovnar@mail.de
    uri: https://www.bovnar.io

normative:
  RFC3629:
  RFC5234:
  RFC7405:
  RFC6838:
  RFC6657:
  BOVNAR-UNITS:
    date: 2026
    title: "Bovnar - Unit and Currency Reference"
    author:
      - name: Janos Sonntag
    target: https://www.bovnar.io/doc/05_bovnar_unit_system.md
  UNICODE:
    title: "The Unicode Standard"
    author:
      - org: The Unicode Consortium
    target: https://www.unicode.org/versions/latest/
  IEEE754:
    title: "IEEE Standard for Floating-Point Arithmetic"
    seriesinfo:
      IEEE: 754-2019
    date: 2019
    author:
      - org: IEEE
  SI:
    title: "The International System of Units (SI), 9th edition"
    author:
      - org: Bureau International des Poids et Mesures
    date: 2019
  IEC80000-13:
    title: "Quantities and units - Part 13: Information science and technology"
    seriesinfo:
      IEC: 80000-13:2008
    date: 2008
    author:
      - org: International Electrotechnical Commission
  ISO4217:
    title: "Codes for the representation of currencies"
    seriesinfo:
      ISO: 4217:2015
    date: 2015
    author:
      - org: International Organization for Standardization
  ISO8601:
    title: "Date and time - Representations for information interchange"
    seriesinfo:
      ISO: 8601-1:2019
    date: 2019
    author:
      - org: International Organization for Standardization

informative:
  RFC8259:
  RFC8949:
  RFC9110:
  RFC3986:
  RFC5198:
  RFC9839:
  BOVNAR-SPEC:
    date: 2026
    title: "Bovnar - Specification, version 1.2"
    author:
      - name: Janos Sonntag
    target: https://www.bovnar.io/doc/03_bovnar_spec.md
  BOVNAR-EBNF:
    date: 2026
    title: "Bovnar - Formal Grammar (ISO/IEC 14977 EBNF)"
    author:
      - name: Janos Sonntag
    target: https://www.bovnar.io/doc/12_bovnar.ebnf
  BOVNAR-CONFORMANCE:
    date: 2026
    title: "Bovnar - Conformance Test Tool and Corpus"
    author:
      - name: Janos Sonntag
    target: https://www.bovnar.io/doc/13_bovnar_conformance.md
  UCUM:
    title: "The Unified Code for Units of Measure"
    author:
      - org: Regenstrief Institute
    target: https://ucum.org/ucum
  CF:
    title: "NetCDF Climate and Forecast (CF) Metadata Conventions"
    author:
      - org: CF Conventions Committee
    target: https://cfconventions.org/
  OM2:
    title: "Ontology of units of Measure (OM) 2"
    author:
      - org: Wageningen University and Research
    target: https://github.com/HajoRijgersberg/OM
  UNECE20:
    title: "UN/CEFACT Recommendation 20 - Codes for Units of Measure Used in International Trade"
    author:
      - org: United Nations Economic Commission for Europe
    target: https://unece.org/trade/uncefact/cl-recommendations
  QUDT:
    title: "QUDT - Quantities, Units, Dimensions and Data Types Ontologies"
    author:
      - org: QUDT.org
    target: https://qudt.org/
  UDUNITS:
    title: "UDUNITS-2 - Unidata Units Library"
    author:
      - org: UCAR/Unidata
    target: https://www.unidata.ucar.edu/software/udunits/
  MCO:
    title: "Mars Climate Orbiter Mishap Investigation Board Phase I Report"
    author:
      - org: NASA
    date: 1999-11-10
  ISO14977:
    title: "Information technology - Syntactic metalanguage - Extended BNF"
    seriesinfo:
      ISO/IEC: 14977:1996
    date: 1996
    author:
      - org: International Organization for Standardization

--- abstract

This document specifies Bovnar (BVNR), a typed, self-describing
serialization format in which every value may carry its own type family,
bit width, numeric base, and physical unit of measure inline, without
reference to an external schema. A Bovnar document is a text document
with an escape mechanism for opaque binary regions. The declared unit is
not a naming convention or a comment: it is part of the value, is
validated against a fixed registry of units, prefixes, and currency
codes, and a unit that contradicts the value's type annotation is a parse
error. This document defines the character encoding, the lexical and
syntactic grammar, the type and unit systems, the validation rules, the
error model, and the media type registration for the format.

--- middle

# Introduction

## Motivation

Interchange formats in wide use today are dimensionless. JSON
{{RFC8259}} and CBOR {{RFC8949}} describe how a number is encoded but say
nothing about what the number measures. The quantity a field carries is
conveyed out of band: in a schema, in a naming convention
(`altitude_m`), in a units attribute defined by a domain profile such as
{{CF}}, or in documentation that the consuming program never reads.

The failure mode this produces is not a syntax error. A value transmitted
in pound-force and consumed as newtons parses perfectly and is silently
wrong; a length in feet read as metres yields a plausible number. The
loss of the Mars Climate Orbiter {{MCO}} is the canonical instance, but
the pattern is routine wherever measurements cross an organizational
boundary. The parser is exactly the component positioned to catch it, and
in a dimensionless format it is exactly the component that cannot.

Bovnar closes that gap by making the unit part of the value:

~~~
#!bovnar 1.1
.altitude   = <float:64,m> 1250.0;
.thrust     = <float:64,k~g*m/s^2> 9.81;
.sample_at  = <datetime:64,unix> 1750000000;
.budget     = <float_dec:64,$USD> 19.99;
~~~

The first line declares the format version, which this example needs
because `datetime` was added in 1.1 ({{version}}); the other three values
are valid in every version.

A conforming parser validates `m`, `k~g*m/s^2`, and `$USD` against a
fixed registry, rejects a unit it does not recognize, and rejects a
document in which an annotation and an inline suffix disagree about the
unit of the same value. A recipient of the document holds everything
required to interpret - and to dimensionally distrust - every reading in
it, with no schema, no profile, and no prior agreement.

## Design Position

Bovnar occupies a deliberate position between three neighbours:

* Against **JSON** and **CBOR**, it adds per-value type and dimension at
  the cost of a larger grammar and a fixed unit registry that
  implementations must carry.

* Against **UCUM** {{UCUM}} and **CF** {{CF}}, which standardize unit
  *notation* for use inside some other format, it specifies the container
  as well, so that the unit is enforced by the parser rather than by a
  downstream convention. Format version 1.2 adds a facility for writing a
  unit in UCUM and other foreign notations; it is specified by
  {{BOVNAR-SPEC}} and not by this document; see {{unit-profiles}}.

* Against **schema languages**, it is schema-free. The description
  travels with each value rather than in a separate artifact that can be
  lost, versioned apart, or never consulted.

Bovnar does not attempt to be the smallest encoding, the fastest
encoding, or a general replacement for JSON. It targets scientific,
industrial, and financial data at rest and in transit, where the cost of
a dimensional error greatly exceeds the cost of the bytes describing the
dimension.

## Scope of This Document

This document specifies:

* the character encoding and byte classes of a Bovnar document
  ({{encoding}});
* the optional version declaration ({{version}});
* the lexical grammar ({{lexis}}) and the syntactic grammar
  ({{types}}, {{composites}}), given in ABNF {{RFC5234}} {{RFC7405}};
* the type annotation system, its families and parameters, and the
  synthesis of default types for unannotated values ({{types}});
* the unit system, its notation, prefixes, currencies, and the rules
  under which a unit is accepted or rejected ({{units}});
* validation rules, limits, and the error model ({{validation}});
* conformance requirements for producers and consumers
  ({{conformance}});
* the media type registration and IANA considerations ({{iana}}); and
* interoperability and security considerations ({{interop}},
  {{security}}).

This document does not specify an application programming interface, a
canonical binary encoding, a schema language, a query language, or any
unit conversion arithmetic. A reference implementation in C99 provides
all of these; they are outside the interchange format and outside this
document.

## Relationship to the Bovnar Specification

The normative project specification for the format is {{BOVNAR-SPEC}},
with the unit registry in {{BOVNAR-UNITS}} and a grammar in ISO/IEC 14977
{{ISO14977}} EBNF in {{BOVNAR-EBNF}}. This document is a restatement of
format version 1.2 of that specification in IETF conventions, with the
grammar translated into ABNF and with the media type, security, and
interoperability analysis that publication as an RFC requires.

Where this document and {{BOVNAR-SPEC}} disagree on a point of format
version 1.2 that both specify, that is a defect in one of them and should be reported. This
document deliberately omits material from {{BOVNAR-SPEC}} that describes
the reference implementation rather than the format: event callbacks,
C types, internal representations, and API entry points. It also excludes
{{BOVNAR-SPEC}} Section 11.9 (unit profiles), the one version 1.2 facility
this document names without specifying; see {{unit-profiles}}.

# Conventions and Definitions

## Requirements Language

{::boilerplate bcp14-tagged}

## Terminology {#terminology}

Document:
: A complete octet sequence conforming to the `stream` rule of
  {{collected-abnf}}.

Assignment:
: A key, an `=` sign, an optional type annotation, a value, and a
  terminating semicolon. A document is a sequence of assignments.

Key:
: The identifier naming an assignment, introduced by a leading dot.

Value kind:
: One of: null, boolean, number, datetime, string, symbol, reference,
  array, struct, octet stream. The kind is a property of the value token
  itself, independent of any type annotation.

Type annotation:
: An optional `<...>` construct preceding a value, declaring its type
  family and, where applicable, bit width, numeric base, fractional bit
  count, epoch, and unit.

Type family:
: One of `uint`, `sint`, `float`, `float_fix`, `float_dec`, `utf8`,
  `bool`, `datetime`.

Unit:
: A dimensional annotation on a numeric value, drawn from the registry
  described in {{units}}. A unit may be compound. The distinguished unit
  `no_unit` denotes an explicitly dimensionless quantity.

Quantity:
: A numeric value together with its unit.

Producer:
: An implementation that emits Bovnar documents.

Consumer:
: An implementation that reads Bovnar documents.

Streaming consumer:
: A consumer that processes a document as a sequence of events without
  materializing it. See {{conformance}}.

Materializing consumer:
: A consumer that constructs an in-memory tree of the whole document.
  See {{conformance}}.

## Grammar Notation {#grammar-notation}

Grammar rules use ABNF {{RFC5234}} as extended by {{RFC7405}}.

**All literal strings in this document's ABNF are case-sensitive.**
Bovnar keywords, type family names, unit symbols, and currency codes
distinguish case throughout: `Pa` (pascal) and `pa` are not the same
token, and `NAN` is an ordinary symbol while `nan` is a special number.
For readability, `%s`-prefixed literals are used only where the
distinction is most likely to be missed; readers and implementers MUST
treat every quoted literal in this document as though it were so
prefixed.

Bovnar is defined over octets, not over characters. Rules that mention
byte ranges (`%x80-BF` and similar) constrain the octet stream directly.
Where a rule admits a range of octets that could form an invalid UTF-8
sequence, the additional UTF-8 well-formedness requirement of
{{encoding}} applies and is not repeated in the grammar.

Several constraints of the format are not expressible in a context-free
grammar and are stated in prose only:

* **UTF-8 well-formedness** ({{encoding}}).

* **Byte order mark placement** ({{encoding}}).

* **Version declaration placement**, and the rule that a declaration
  **commits**: once its prefix and the following horizontal whitespace
  have matched, a malformed version is an error and does not fall back to
  being an ordinary comment ({{version}}). The grammar admits the
  fallback, because `version-decl` is optional and `comment` would match
  the same octets; the prose forbids it.

* **Version gating.** The grammar has no notion of the declared version,
  so it accepts every construct in every document. The constructs
  introduced after 1.0 - the `datetime` family, the `\x` and `\u{}`
  escapes, and reference array indexing - are valid only in a document
  that declares 1.1 or newer, and a `namespace:code` unit only in one
  that declares 1.2 or newer ({{version}}). This is the single largest
  class of rule the grammar cannot carry.

* **String escape semantics** ({{strings}}): that `\u{}` names a Unicode
  scalar value rather than a surrogate or a value above U+10FFFF, and
  that no escape may resolve to a rejected control octet. The grammar
  permits any two hexadecimal digits and any one-to-six-digit scalar.

* **Datetime literal field ranges and epoch compatibility**
  ({{datetime-literals}}): that a month is 01-12, a day valid for its
  month and year, an hour 00-23, a second 00-60; and that the atomic GNSS
  epochs reject a literal outright. The grammar counts digits and nothing
  more.

* **The numeric limits** of {{limits}}.

* **Octet-stream chunk length-prefixing** ({{octet-streams}}). The
  grammar writes `os-data = *OCTET`, which says only "some octets"; the
  binding requirement that the count equal the preceding `os-length`
  field is context-sensitive.

* **Type/value and unit compatibility** ({{types}}, {{units}}): valid
  widths, bases, and Q values; value ranges; digits within the declared
  base; that a unit resolves against the registry; and that an annotation
  unit and an inline suffix agree.

* **The sibling-comparison rules** enforced at the materializing tier
  only ({{conformance}}).

A document that satisfies the ABNF but violates any of these is not a
valid Bovnar document. Equivalently: the ABNF is a **necessary** condition
for validity, never a sufficient one, and an implementation that checks
only the grammar accepts documents this document declares invalid. The
one exception to *necessary* lies in the facility this document does not
specify: a unit parameter carrying a profile namespace may contain SP and
HTAB, which `unit-param` does not admit ({{unit-profiles}},
{{error-codes}}).

Two points of the grammar require lookahead rather than a single-token
decision, and an implementation built as a greedy lexer MUST account for
them:

* **Reserved words outrank `symbol`.** The eight reserved spellings
  ({{symbols}}) match `symbol` as well as their own rules. They are
  always the reserved value, never a symbol.
* **A datetime literal outranks a number.** `2026-06-15` begins with a
  digit run that also matches `number`. Only the whole-assignment parse
  distinguishes them, so the grammar is unambiguous but a lexer MUST look
  past the first digit run for the `-` that begins a `dt-literal`
  ({{datetime-literals}}).

The collected grammar appears in {{collected-abnf}}. Fragments in the
body of this document are excerpts from it.

# Document Model {#model}

A Bovnar document is an ordered sequence of assignments in a top-level
scope. Each assignment binds a key to a single value.

~~~ abnf
stream       = [ BOM ] *blank [ version-decl ] ws *( assignment ws )
assignment   = "." key ws "=" ws value ws ";"
~~~

A value is one of the ten kinds listed in {{terminology}}. Structs and
arrays nest; every other kind is a leaf.

A document with no assignments is valid and denotes the empty document.
A document consisting only of blanks, comments, and an optional version
declaration is such a document.

Keys MUST be unique within a scope. A scope is a single struct, or the
top-level document. Repeating a key within one scope is an error
(`error_duplicate_struct_key`). The same key in different scopes is
unrelated and always permitted. Because this rule requires comparing
siblings, it is enforced at the materializing tier only; see
{{conformance}}.

A value's *interpretation* is determined by its type annotation if it has
one, and otherwise by default type synthesis ({{synthesis}}). Every value
therefore has a definite type family, and every numeric value a definite
unit, whether or not the document states them.

A value's *kind* is determined by the value token alone and is not
affected by the annotation. An annotation whose family is incompatible
with the kind of the value that follows it is an error; it never
reinterprets the token. `<uint:32> hello` is not the integer zero, it
is `error_type_value_mismatch`. (A *quoted* string is a carrier `uint`
does accept, so `<uint:32> "hello"` is refused one step later, for its
digits: `error_digit_not_in_base`.)

# Character Encoding {#encoding}

## UTF-8

Outside octet-stream regions ({{octet-streams}}), the entire document
MUST be well-formed UTF-8 {{RFC3629}}. A consumer MUST validate this and
MUST reject a document containing:

* an ill-formed sequence, including a truncated or stray continuation
  byte (`error_invalid_utf8_byte`);
* an overlong encoding;
* an encoded surrogate code point, U+D800 through U+DFFF.

Validation of the text layer is suspended for the duration of an
octet-stream region, whose payload is an arbitrary octet sequence, and
resumes when the region ends.

This document does not require any Unicode normalization form. Two keys
that are canonically equivalent under {{UNICODE}} but differ in octets
are distinct keys. See {{security-confusable}} for the consequences.

Consumers SHOULD reject the "problematic" code points described in
{{RFC9839}} where their application permits; the format itself excludes
the C0 controls other than the whitespace controls, and excludes DEL,
from all text-layer contexts, but permits noncharacters and unassigned
code points in strings and keys.

## Byte Order Mark

A UTF-8 byte order mark (U+FEFF, encoded `EF BB BF`) is permitted only as
the first three octets of a document, where it MUST be ignored.

~~~ abnf
BOM          = %xEF.BB.BF
~~~

A BOM anywhere else is an error or is ordinary content, depending on
whether the first assignment has begun.

Before the first assignment:

* a BOM inside any comment - the first comment line, a version
  declaration line, or a later comment line - is
  `error_invalid_byte_order_mark`;
* a BOM after leading blanks, with no comment line before it, is
  `error_invalid_byte_order_mark`;
* a BOM at the start of a line that follows a comment line or a version
  declaration is `error_unexpected_input_byte`, as is a second BOM
  directly after the one at offset 0.

Once the first assignment has begun, `EF BB BF` inside a comment or a
string is ordinary well-formed UTF-8 and is accepted as content, and
between assignments it is `error_unexpected_input_byte`.

Producers SHOULD NOT emit a byte order mark. It carries no information in
a format whose encoding is fixed.

## Byte Classes

The following octet classes recur throughout the grammar.

| Class | Octets | Role |
|---|---|---|
| Blank | 09, 0A, 0B, 0C, 0D, 20 | Token separator |
| Rejected control | 00-08, 0E-1F, 7F | Error outside octet streams |
| ASCII printable | 20-7E | Identifier, symbol, unit, string content |
| UTF-8 lead | C2-F4 | First octet of a multi-octet sequence |
| UTF-8 continuation | 80-BF | Later octets of a multi-octet sequence |
{: title="Octet classes"}

~~~ abnf
blank        = HTAB / LF / VT / FF / CR / SP
VT           = %x0B
FF           = %x0C
utf8-cont    = %x80-BF
utf8-lead    = %xC3-DF / %xE0-EF / %xF0-F4
unit-lead    = %xC2-F4
~~~

Note the asymmetry between `utf8-lead` and `unit-lead`. Octet `C2` is
rejected at the start and in the body of keys, symbols, and reference
segments, which excludes U+0080 through U+00BF from identifiers
everywhere. It is accepted inside units, where it is required for the
micro prefix (U+00B5), the degree sign (U+00B0), and the middle-dot
product separator (U+00B7).

The rejected control octets are errors in every text-layer context,
including inside string literals and comments, and including when
produced by an escape sequence ({{strings}}). The whitespace controls
HTAB, LF, VT, FF, and CR are accepted as literal content inside strings.

# Version Declaration {#version}

A document MAY declare the format version it targets with a directive on
its first line.

~~~ abnf
version-decl = "#" %s"!bovnar" 1*hspace version-int "." version-int
               *hspace [ CR / LF ]
hspace       = HTAB / SP
version-int  = "0" / ( %x31-39 *DIGIT )
~~~

The directive is lexically a comment, so a consumer that predates the
directive skips it transparently. This is the one place in the format
where a comment carries meaning; comments are otherwise inert.

**An undeclared document is version 1.0.** The directive only ever opts
in to a newer version, and its absence is never ambiguous. A construct
introduced after 1.0 is therefore available only to a document that
declares a version at least as new: in an undeclared document, the 1.1
constructs (the `datetime` family, the `\x` and `\u{}` string escapes,
and reference array indexing) are invalid exactly as they would be to a
1.0 consumer, and the one 1.2 construct (a `namespace:code` unit,
{{unit-profiles}}) is invalid in any document that declares less than
1.2.

Recognition is strict:

* The directive is recognized only as the **very first comment**, after
  an optional BOM and leading blanks, blank lines included. `#!bovnar 1.1`
  after any earlier comment, or after any content, is an ordinary comment.
* `#!bovnar` not followed by horizontal whitespace, such as
  `#!bovnarish`, is an ordinary comment.
* Only HTAB and SP separate the directive's components. A line break
  terminates it.
* Each component is a decimal integer with no leading zero, save for a
  bare `0`, and MUST fit in 16 bits.
* A directive prefix followed by a malformed version - a missing
  component, a non-numeric component, a leading zero, or trailing junk -
  is `error_invalid_spec_version`, not an ordinary comment.

A consumer supports version `major.minor` when `major` equals its own
major and `minor` is less than or equal to its own minor.

By default a consumer MUST accept a declared version it does not support,
recording it, and MUST fail only if the document subsequently uses a
construct the consumer does not implement. A consumer MAY offer a strict
mode that rejects an unsupported declared version at the directive
(`error_unsupported_spec_version`). The rejection ends parsing even in
recovery mode ({{error-recovery}}). Consumers processing input from an
untrusted source SHOULD enable strict mode; see {{security-version}}.

# Lexical Structure {#lexis}

## Whitespace and Comments

Blanks and comments may appear freely between tokens, with one
restriction on where a comment may start, stated below.

~~~ abnf
ws           = *( blank / comment )
ws-req       = ( blank / comment ) *( blank / comment )
comment      = "#" *comment-char [ CR / LF ]
comment-char = HTAB / VT / FF / %x20-7E / %x80-FF
~~~

A comment runs from `#` to the next CR, LF, or end of document. The
rejected control octets are errors inside a comment body.

**`#` is not a terminator of a bare token.** A comment MUST NOT start
immediately after a key, a number, a symbol, a reserved word, a reference,
a datetime literal or a type-annotation parameter: there `#` is
`error_unexpected_input_byte`, and at least one blank must come first. After a string, `]`, `}`, `>`, `=`, `;`
or an inline unit, `#` may follow directly. The grammar's `ws` admits the
forbidden placements; this prose forbids them.

~~~
.ok1 = 42 # a blank first: valid
;
.ok2 = "text"# directly after a string: valid
;
# .bad = 42# directly after a number: error_unexpected_input_byte
~~~ `ws-req`
denotes a mandatory separator, required in exactly one place: before an
inline unit suffix ({{inline-units}}). A comment's own line end serves as
the blank, so after a string - where `#` may follow directly - a comment
alone separates the value from its unit.

## Keys

~~~ abnf
key          = id-start *id-body
id-start     = ALPHA / "_" / utf8-lead
id-body      = id-start / "+" / "-" / DIGIT / utf8-cont
~~~

A key MUST contain at least one character; `.=` is
`error_empty_identifier`. The leading `.` is a sigil and is not part of
the key.

The following ASCII punctuation characters are errors inside a key:

~~~
! " # $ % & ' ( ) * , . / : ; < = > ? @ [ \ ] ^ ` { | } ~
~~~

`=` terminates the key and begins the value part. A blank also
terminates it, after which only `=` may follow, with comments permitted
in between.

Keys are compared as octet sequences. See {{security-confusable}}.

## String Literals {#strings}

~~~ abnf
string       = string-lit *( ws string-lit )
string-lit   = DQUOTE *string-char DQUOTE
string-char  = safe-byte / escape
safe-byte    = %x09-0D / %x20-21 / %x23-5B / %x5D-7E / %x80-FF
escape       = "\" ( %s"t" / %s"n" / %s"v" / %s"f" / %s"r"
                   / DQUOTE / "\" / byte-esc / uni-esc )
byte-esc     = %s"x" 2HEXDIG                        ; version 1.1
uni-esc      = %s"u" "{" 1*6HEXDIG "}"              ; version 1.1
~~~

The escapes `\t`, `\n`, `\v`, `\f`, `\r`, `\"`, and `\\` denote HTAB, LF,
VT, FF, CR, `"`, and `\` respectively. Any other character after `\` is
`error_illegal_escape_sequence`.

Two further escapes are available in a document declaring version 1.1 or
newer:

* `\xHH` denotes the single octet `HH`, written with exactly two
  hexadecimal digits.
* `\u{H...}` denotes the Unicode scalar value U+H..., written with one to
  six hexadecimal digits, encoded as UTF-8.

`\u{}` MUST reject a surrogate code point or a value above U+10FFFF with
`error_invalid_codepoint`. A missing, empty, or over-long brace group, or
a non-hexadecimal digit, is `error_illegal_escape_sequence`.

`\x` writes a raw octet, but the resulting string MUST still be
well-formed UTF-8. `"\xC3\xA9"` is the string `é`; a lone `"\xFF"` is
`error_invalid_utf8_byte`. Arbitrary non-textual octets belong in an
octet stream ({{octet-streams}}), not in a string.

An escape that resolves to a rejected control octet - `00`-`08`, `0E`-`1F`,
or `7F`, that is, every C0 control except the whitespace controls - is
`error_unexpected_input_byte`, exactly as the raw octet would be. The
escape is not a way around the control-octet rule.

In a document that declares version 1.0 or declares nothing, `x` and `u`
after a backslash are unrecognized escapes and yield
`error_illegal_escape_sequence`.

Adjacent string literals separated only by blanks and comments are
concatenated into one string. The combined octet length is subject to
`max_string_length` ({{limits}}).

~~~
.long = "hello " "world";        # one string, "hello world"
~~~

## Symbols and Reserved Words {#symbols}

A symbol is an unquoted bare word in value position.

~~~ abnf
symbol       = id-start *id-body
~~~

A symbol has the same shape as a key but terminates differently: `,` ends
an array element, `]` closes an array row, and `;` ends the value, where
each of those is an error inside a key. Conversely `=` is an error inside
a symbol, where it terminates a key.

Eight exact spellings are reserved and are not symbols:

| Word | Meaning |
|---|---|
| `null` | The null value ({{null}}) |
| `true`, `on` | Boolean true |
| `false`, `off` | Boolean false |
| `nan` | IEEE 754 quiet NaN |
| `inf` | Positive infinity |
| `ninf` | Negative infinity |
{: title="Reserved words"}

~~~ abnf
bool-value   = %s"true" / %s"false" / %s"on" / %s"off"
special-num  = %s"nan" / %s"inf" / %s"ninf"
~~~

The reservation is exact. A longer word that merely begins with a
reserved spelling - `ontology`, `nullable`, `truthy`, `infinity` -
remains an ordinary symbol.

An unannotated boolean keyword synthesises the `bool` family
({{synthesis}}). An explicit `<bool>` annotation accepts only these four
keywords as its value.

## References {#references}

A reference is an unresolved path to another key.

~~~ abnf
reference    = "&" ref-seg *( ref-seg / ref-index )
ref-seg      = "." id-start *id-body
ref-index    = "[" 1*DIGIT "]"                      ; version 1.1
~~~

The stored text of a reference includes the leading dot and every
intermediate dot: `&.config.host` stores `.config.host`.

**A reference is never dereferenced by the parser.** This is a
substantive property of the format, not an implementation choice:

* The target need not exist. A reference to a missing key, a forward
  reference, and a reference to a value outside the document are all
  accepted and stored verbatim.
* Cycles are not detected. `.a = &.b; .b = &.a;` is a valid document.
* Resolution, including the treatment of dangling paths and cycles, is
  entirely the application's responsibility.

Within an array, references are homogeneous by kind: an array of
references is uniform regardless of what its targets would resolve to,
which the parser cannot know. A reference path is bounded by
`max_reference_length` ({{limits}}).

A version 1.1 document may address array elements with `[N]` index
suffixes, as in `&.matrix[0][1]`. The index is captured verbatim with the
rest of the path and is interpreted only when an application resolves the
stored path string itself. Resolution semantics, when an application
chooses to implement them, follow the array model of {{arrays}}: a flat
`/`-row matrix is addressed `[row][col]`, a one-dimensional array as
`[i]`, and genuine nested arrays descend one index per level. Three
cases do not resolve, and an application MUST distinguish "no such value"
from "the value is null" when it meets them: a **partial** index of a
flat matrix (`&.matrix[0]`, which names a row rather than an element), an
**out-of-range** index, and an index applied to something that is **not
an array**. In a 1.0 or undeclared document, `[` inside a reference is
`error_unexpected_input_byte`.

See {{security-references}} for the hazards of resolving references.

## Numbers {#numbers}

~~~ abnf
number       = [ "-" ] ( int-led / dot-led ) [ dec-exp ]
int-led      = 1*DIGIT [ "." *DIGIT ]
dot-led      = "." 1*DIGIT
dec-exp      = ( %s"e" / %s"E" ) [ "+" / "-" ] 1*DIGIT
~~~

Leading zeros are permitted (`007`). A trailing dot with no fractional
digits is permitted (`123.`). A bare `.` is an error.

Only `e` and `E` introduce an exponent in a bare literal.

A bare number literal under a non-decimal base is valid when every octet
of it is a digit of that base, and it is read **in that base**:
`<uint:_16> 10` is sixteen, not ten. In base 15 and above `e` and `E` are
digits, so a bare `1e3` under `_16` is the integer 0x1E3 (483) and not an
exponent form; below base 15 the `e` is still an exponent marker, which
under an integer family is `error_type_value_mismatch`. A value that
needs a letter the number rule does not admit cannot be written bare - a
token such as `ff` lexes as a symbol, and is `error_type_value_mismatch`
- so it is written as a quoted string. Producers SHOULD quote every
non-decimal value:

~~~
.hex = <uint:_16> "ff";          # 255 - the form to write
.bin = <sint:_2>  "101010";      # 42
.m   = <uint:_16> 10;            # valid, and sixteen
.k   = <uint:_16> 1e3;           # valid, and 483: 'e' is a digit here
~~~

Inside a quoted string, base-16 float values use `p` or `P` as the
exponent marker rather than `e` or `E`, because `e` is a valid
hexadecimal digit. The exponent following `p` is a decimal integer
denoting a binary exponent, as in C99 hexadecimal floating literals.

~~~
.hexfloat = <float:64,_16> "1.8p+2";   # 1.8(16) x 2^2 = 6.0
.hexmant  = <float:64,_16> "1.8e";     # no exponent: 'e' is a digit
~~~

A quoted integer literal (`uint`, `sint`) takes a leading `-` and no
`+`, as a bare one does: `<sint:8> "+1"` is `error_digit_not_in_base`. A
quoted float literal (`float`, `float_fix`, `float_dec`) may in addition
begin with a single `+`, which leaves the value unchanged:
`<float:64> "+1.5"` is 1.5. The sign is the first octet inside the
quotes and there is at most one. Producers SHOULD NOT write the `+`.

## Datetime Literals {#datetime-literals}

A version 1.1 document may write a `datetime` value as a literal in the
profile of {{ISO8601}} given below, in place of the integer carrier.

~~~ abnf
dt-literal   = dt-date [ %s"T" dt-time [ "." 1*DIGIT ] [ dt-zone ] ]
dt-date      = 4DIGIT "-" 2DIGIT "-" 2DIGIT
dt-time      = 2DIGIT ":" 2DIGIT ":" 2DIGIT
dt-zone      = %s"Z" / ( "+" / "-" ) 2DIGIT ":" 2DIGIT
~~~

Fields are strictly range-checked: month 01-12, a day valid for that
month and year, hour 00-23, minute 00-59, second 00-60, and a two-digit
offset. Which of two errors a malformed literal gets is decided by its
skeleton: the grammar above with every `4DIGIT` and `2DIGIT` read as
`1*DIGIT` and the fraction as `*DIGIT`. A literal that matches the
skeleton but not the grammar or the ranges - an out-of-range field, a
field of the wrong width (`2026-6-15`), or an empty fraction - is
`error_invalid_datetime_literal`. One that departs from the skeleton - a
missing seconds field, an offset without its colon - is
`error_unexpected_input_byte`. A `-` in front of a literal opens a number
token and is not a departure from the skeleton: `-2026-06-15`, a year
before 0000, is `error_invalid_datetime_literal`, reported where the
value ends. A `+` in front of one is `error_unexpected_input_byte` at the
`+`.

A date with no time part denotes `00:00:00Z`. A date-time with no zone is
UTC. An offset shifts the written civil time to UTC before conversion, so
`12:00:00+02:00` is `10:00:00Z`. The `.fraction`, `Z`, and `±HH:MM` parts
are valid only after a complete `HH:MM:SS`.

The literal is converted at parse time to the integer epoch-seconds
carrier of its declared epoch, and that integer is what the document
denotes. An unannotated literal infers `<datetime:64,unix>`, so
`.t = 2026-06-15;` is a timestamp with no annotation at all. An ISO
literal under any non-`datetime` annotation is
`error_type_value_mismatch`.

### Leap Seconds

A second value of `60` denotes a UTC leap second and MUST be accepted.
What it stores depends on the epoch:

* On the civil epochs (`unix`, `mjd`, `ntp`, `y2000`), it normalizes onto
  the following second: `2016-12-31T23:59:60Z` and `2017-01-01T00:00:00Z`
  store the same value. These scales run a uniform 86400-second day and
  have no second to spend on the insertion.
* On `tai`, it does not. TAI is a continuous atomic count, the inserted
  second is a distinct instant on it, and the two literals above store
  values differing by one. Over the insertions the implementation's
  leap-second table records, this makes the UTC-to-TAI mapping injective,
  so a `tai` value survives any number of read/write cycles unchanged.

A `:60` at an instant that the implementation's leap-second table does
not record as an insertion collapses on `tai` as well: it stores the
value of the following second, and that is the one case in which two
civil spellings share a `tai` value. The table is a
static snapshot, and an implementation built before an IERS announcement
MUST NOT reject a document spelling a genuine future leap second. See
{{security-leap}}.

### Fractional Seconds

A fractional part of any digit count is accepted. The carrier remains
whole seconds, so the fraction takes no part in the value's arithmetic or
comparison. The verbatim digits are nonetheless preserved and re-emitted,
so the literal round-trips. The fraction is informational; sub-second
values that participate in computation SHOULD use a finer integer
carrier, such as milliseconds since the epoch, with an explicit unit.

## Null {#null}

A null value is either the reserved word `null` or the absence of a value
token.

~~~
.a = ;                # null
.b = null;            # identical to .a
.c = [,1,,2,];        # five elements: null, 1, null, 2, null
.d = <uint:32> ;      # a null carrying the type uint:32
~~~

An empty array row is distinct from a row holding one null: `[]` has zero
elements and `[null]` has one.

# Type Annotations {#types}

## Syntax and Placement

~~~ abnf
type-ann     = "<" ws type-spec ws ">"
type-spec    = family [ ws ":" ws param-list ]
family       = %s"uint" / %s"sint" / %s"float_fix" / %s"float_dec"
             / %s"float" / %s"utf8" / %s"bool" / %s"datetime"
param-list   = param *( ws "," ws param )
param        = width-param / base-param / q-param
             / epoch-param / unit-param
width-param  = 1*DIGIT
base-param   = "_" 1*DIGIT
q-param      = %s"q" 1*DIGIT
~~~

The `family` alternatives are matched longest-first; `float_fix` and
`float_dec` are distinct families and are not `float` with a parameter.

An annotation MUST appear in one of four positions, always before the
value it describes:

1. Immediately after the `=` of an assignment: `.k = <uint:32> 42;`
2. Before the opening `[` of an array, where it is a **whole-array
   annotation** inherited by every element that does not carry its own:
   `.ports = <uint:16> [80, 443, 8080];`
3. After the opening `[` of an array, before the first element.
4. After a `,` inside an array, before the next element.

An annotation attached to the key rather than the value, as in
`.k<uint:32> = 42;`, is an error.

An element annotation describes that element only: in `[<uint:8> 1, 300]`
the `300` is an unannotated integer. The one exception is `datetime`,
after which a following bare integer is a `datetime` of the same width
and epoch ({{synthesis}}).

A struct takes no annotation - its members carry their own. An annotation
in force where a `{` opens, whether written before the struct or
inherited from the array it is an element of, is
`error_type_value_mismatch`: `<utf8> {}` and `<uint:8> [{.x = 1;}]` are
both refused.

A wholly empty annotation, `<>` or `< >`, is
`error_unexpected_input_byte`.

## Type Families

| Family | Parameters | Default width |
|---|---|---|
| `uint` | width, base, unit | 64 |
| `sint` | width, base, unit | 64 |
| `float` | width, base (10 or 16 only), unit | 64 |
| `float_fix` | width, `qN`, unit | 64 |
| `float_dec` | width, unit | 64 |
| `utf8` | none | - |
| `bool` | none | - |
| `datetime` | width, epoch | 64 |
{: title="Type families and their parameters"}

`utf8` and `bool` are parameterless; any parameter is
`error_illegal_value_type`, a width of zero (`<utf8:0>`) included. For
`float_fix` an omitted `qN` is `q0`.

`datetime` is a version 1.1 family and is `error_illegal_value_type` in a
1.0 or undeclared document.

### Value Compatibility

| Family | Accepts |
|---|---|
| unannotated | any value |
| `utf8` | string only |
| `bool` | `true`, `false`, `on`, `off` only |
| `uint` | number, or string of digits |
| `sint` | number, or string of digits, possibly negative |
| `float` | number or string, base 10 or 16 |
| `float_fix` | number or string, base 10 |
| `float_dec` | number or string, base 10 |
| `datetime` | decimal signed integer, or an ISO literal |
{: title="Type/value compatibility"}

A value whose kind is incompatible with the annotated family is
`error_type_value_mismatch`. A dot or exponent in an integer-typed value
is likewise `error_type_value_mismatch`.

Three value kinds stand outside this table. A null takes any annotation
and carries its type ({{null}}). A struct takes none
(`error_type_value_mismatch`, above). An **octet stream** under an annotation is
accepted under *every* family: the annotation is carried with the stream
and is **not checked** against it, so `<uint:8>`, `<utf8>` or `<bool>` in
front of an octet stream is not `error_type_value_mismatch`. It says
nothing about the payload, and producers SHOULD NOT write one.

### Special Numbers

`nan`, `inf`, and `ninf` are accepted under every numeric family -
`uint`, `sint`, `float`, `float_fix`, `float_dec`, and `datetime` - and
in unannotated position. Range validation is bypassed for them, so
`<uint:8> nan` is a valid document. They are rejected under `utf8` and
`bool` with `error_type_value_mismatch`.

This is a deliberate concession to instrument data, where a missing or
saturated reading is naturally NaN or infinity regardless of the
channel's declared integer width. Consumers MUST anticipate it; see
{{security-numeric}}.

A special number takes no inline unit suffix. Supply a unit through the
annotation instead: `<float:64,m/s> inf`.

## Parameters

Parameters are identified by their **syntactic class**, not by position,
so they may appear in any order and at most one of each class may appear.
These three annotations are identical:

~~~
<uint:32,_10,no_unit>   <uint:_10,no_unit,32>   <uint:no_unit,_10,32>
~~~

Because no positional slot is ever empty, the parameter list is parsed
strictly. Empty, trailing, or doubled components - `<uint:8,>`,
`<uint:8,,>`, `<uint:,_16>`, and the bare `<uint:>` - are
`error_illegal_value_type`.

An epoch name and a unit are syntactically indistinguishable; both are
bare words. They are disambiguated by family: under `datetime` a bare-word
parameter is an epoch name, and under every other family it is a unit.

### Width

Width is a count of bits.

| Family | Valid widths |
|---|---|
| `uint`, `sint` | 0, or 1 to 32768 |
| `float` | 0, 16, or any multiple of 32 up to 32768 |
| `float_fix`, `float_dec` | 0, 16, 32, 64, 128, 256 |
| `datetime` | as `sint` |
{: title="Valid widths by family"}

Width `0` selects the family default of 64. An out-of-range or
non-conforming width is `error_illegal_value_type`.

An integer value that does not fit the declared width is
`error_value_out_of_range`, including a negative value under `uint`.

A `float` or `float_dec` literal is **not** range- or precision-checked.
One with more precision or a larger exponent than the declared width can
hold is valid; the excess is the consumer's to round, and a consumer may
deliver positive or negative infinity. For these two families the width
bounds what a producer promises, not what a consumer rejects.

### Base

~~~
base-param   = "_" 1*DIGIT
~~~

The base applies to the digits of the value, not to its storage.

* `uint` and `sint` accept bases 2 through 62.
* Bases 64 and 85 are **`uint` only**. Their alphabets take the sign
  characters as digits - `+` in both, `-` in Ascii85 - so a sign cannot be
  distinguished; a signed value in these bases is
  `error_illegal_value_type`.
* Any other base (`_0`, `_1`, `_63`, `_99`) is `error_illegal_value_type`.
* `float` accepts only base 10 or 16. Any other base is
  `error_illegal_value_type`.
* `float_fix` and `float_dec` forbid the base parameter entirely; decimal
  is implicit. A base parameter on either is `error_illegal_value_type`.

The digits of a base, lowest value first, are:

| Base | Digits | Case |
|---|---|---|
| 2 to 36 | `0`-`9`, then `a`-`z` (10 to 35) | insensitive |
| 37 to 62 | `0`-`9`, `a`-`z` (10 to 35), `A`-`Z` (36 to 61) | sensitive |
| 64 | `A`-`Z`, `a`-`z`, `0`-`9`, `+`, `/` ({{?RFC4648}}, no padding) | sensitive |
| 85 | the 85 characters `!` (%x21, value 0) to `u` (%x75, value 84) | sensitive |
{: title="Digit alphabets"}

A digit outside the declared base is `error_digit_not_in_base`.

### Q (Fractional Bits)

`qN` applies only to `float_fix` and gives the number of fractional bits
in a signed Q-format integer of the declared width. The mathematical
value is `raw x 2^-Q`. `q` on any other family, or `N` not less than the
effective width, is `error_illegal_value_type`.

A `float_fix` value MUST be representable in the declared format:
`round(value x 2^Q)`, rounded once to nearest with ties to even, MUST fit
a signed field of `width` bits. The representable values are
`[-2^(width-1-Q), 2^(width-1-Q) - 2^-Q]`, and the test is made after
rounding, so a literal just past either end that rounds back onto it is
valid. A value whose rounded integer does not fit is
`error_value_out_of_range`. Special numbers are exempt.

~~~
.a = <float_fix:16,q8> 3.14;      # Q8/16 range [-128, 127.99609375]

# .b = <float_fix:16,q8> 128;     # error_value_out_of_range
# .c = <float_fix:16,q16> 1.0;    # Q >= width: illegal_value_type
~~~

`float_fix` is never synthesised ({{synthesis}}), because Q cannot be
inferred from a literal.

### Decimal Floating Point

`float_dec` denotes decimal floating point in the sense of {{IEEE754}}:
values whose significand is a decimal integer, so that decimal fractions
such as `0.1` are exact. It exists so that monetary and metrological
values are not held in a binary float.

The number of significant decimal digits available at each width is
given below. It is what the width can hold, not a limit a consumer
enforces ({{types}}): a literal with more digits is valid.

| Width | Max decimal digits |
|---|---|
| 16 | 2 |
| 32 | 7 |
| 64 | 16 |
| 128 | 34 |
| 256 | 70 |
{: title="float_dec precision by width"}

This document does not specify a wire encoding for `float_dec`; the
interchange representation is the decimal text of the value. An
implementation's in-memory encoding is its own affair and need not be a
{{IEEE754}} interchange format.

### Epoch

`datetime` takes one epoch name, defaulting to `unix`:

~~~ abnf
epoch-param  = %s"unix" / %s"tai" / %s"gps" / %s"mjd" / %s"ntp"
             / %s"galileo" / %s"glonass" / %s"y2000" / %s"beidou"
~~~

A `datetime` value is a **signed integer count of seconds** since the
named epoch, and denotes an instant. This is deliberately distinct from a
*duration*, which is an ordinary number carrying a time unit, such as
`<float:64,s>`. Negative values denote instants before the epoch.

A numeric base, `q`, or unit parameter on `datetime` is
`error_illegal_value_type`. A fractional or exponent numeric carrier -
`1.5`, `1e3`, as distinct from the sub-second fraction of an ISO literal
- is `error_type_value_mismatch`.

The four atomic GNSS epochs `gps`, `galileo`, `glonass`, and `beidou`
reject an ISO literal with
`error_datetime_literal_unsupported_epoch`. These scales have no
round-trippable civil-to-seconds inverse here; supply an integer carrier.

## Default Type Synthesis {#synthesis}

An unannotated value acquires a type by synthesis. The synthesised type
is indistinguishable from the same type written explicitly.

| Value form | Synthesised type |
|---|---|
| Quoted string | `<utf8>` |
| Boolean keyword | `<bool>` |
| Special number | `<float:64,_10,no_unit>` |
| Number with `.` or an exponent | `<float:64,_10,no_unit>` |
| Negative integer | `<sint:64,_10,no_unit>` |
| Other integer | `<uint:64,_10,no_unit>` |
| Any number with an inline currency unit | `<float_dec:64,$XXX>` |
| ISO datetime literal (version 1.1) | `<datetime:64,unix>` |
{: title="Default type synthesis"}

The currency row takes precedence over the float and integer rows. Any
number carrying an inline currency unit, integral or not, synthesises
`float_dec`, so money is never held as a binary float: `.price = 5 $USD;`
is `<float_dec:64,$USD>`, not `<uint:64>`.

A bare integer inside an array whose whole-array annotation is `datetime`
inherits that family, width, and epoch rather than synthesising `uint` or
`sint`, so the canonical form of a datetime array - annotation on the
first element, bare integers thereafter - remains homogeneous under
{{array-homogeneity}}.

# The Unit System {#units}

## Model {#unit-model}

A unit is an optional annotation on a numeric value. It is drawn from a
fixed registry of named base units, prefixes, and currency codes, and may
be compound. This section specifies the notation and the rules; the
catalogue itself is {{BOVNAR-UNITS}}.

That reference is **normative**, and it is the one point at which this
document is not self-contained. A consumer MUST validate every unit
against the registry, and the registry is too large to reproduce here and
grows in minor revisions ({{version-stability}}). An implementer needs
{{BOVNAR-UNITS}} to build a conforming parser; everything else required
is in this document.

The registry covers 264 named units across SI {{SI}}, IEC binary
{{IEC80000-13}}, imperial and US customary, CGS, radiation, electrical
power, rotational, surveying, textile, and other families, accepting 745
spellings in total once long-form and plural aliases are counted; plus
216 currency codes ({{currency}}).

Three properties govern how a unit behaves:

* **Descriptive, not prescriptive.** A consumer validates the *form* of a
  unit and the *consistency* of the two places it may be written. It does
  not reject a document because its units do not add up dimensionally
  across assignments, and it performs no conversion. Dimensional analysis
  and conversion are services an application may request; they are not
  part of parsing.

* **Total.** A unit expression either resolves to a unit in the registry
  or is an error. There is no state in which a value carries a unit the
  consumer cannot reason about.

* **Additive across minor versions.** New units, prefixes, currency
  codes, and accepted input spellings may be added in a minor revision. A
  document never depends on a code being *absent*, so this direction is
  safe; a consumer from an older point release may not recognize a unit
  added in a newer one, which is the expected direction of forward
  compatibility.

## Notation

~~~ abnf
resolved-unit = "no_unit" / unit-expr
unit-expr     = unit-factor *( unit-sep unit-factor )
unit-factor   = unit-component / "(" unit-expr ")"
unit-sep      = "*" / "/" / %xC2.B7        ; "*" or U+00B7 or "/"
unit-component = [ prefix [ "~" ] ] base-unit [ unit-exp ]
               / [ prefix [ "~" ] ] "$" currency-code [ unit-exp ]
unit-exp      = ( "^" [ "+" / "-" ] 1*3DIGIT )
              / ( [ sup-sign ] 1*3sup-digit )
~~~

This is the *semantic* grammar of a resolved unit, applied to the octets
that the lexical rules `unit-param` and `inline-unit`
({{collected-abnf}}) accumulate. It is separated from the lexical layer
because `base-unit` and `currency-code` are registry lookups rather than
alternations: the registry may grow in a minor revision ({{unit-model}}),
so the grammar cannot enumerate it.

The excerpt above omits the `prefix` rule, whose alternatives are listed
in {{prefixes}} and enumerated in full in {{collected-abnf}}.

`*` (U+002A) and `·` (U+00B7, encoded `C2 B7`) both denote
multiplication and are semantically identical; `·` is visually preferred.
`/` denotes division.

**Division latches.** The first `/` moves every subsequent factor at that
level into the denominator; further `/` separators never toggle back.
`m/s/s` therefore has the dimensions of `m·s⁻²`, not of `m`. It is not the
SAME unit as `m·s⁻²`: repeated bases are not merged, so `m/s/s` carries
`s⁻¹` twice where `m·s⁻²` carries `s⁻²` once, and the two compare unequal
while converting with factor 1.

**Grouping.** A parenthesised sub-expression is evaluated independently
and, like any factor, obeys the latching denominator: a `/` before a
group negates the group's net exponents as a whole. So `k~g/(m·s²)` is
`kg·m⁻¹·s⁻²`, identical to `k~g/m·s²`, while `(k~g/m)·s²` is
`kg·m⁻¹·s²`. An explicit separator is required before a group - `m·(s)`,
not `m(s)` - a group takes no exponent of its own, and parentheses MUST
balance and MUST NOT nest more than 16 deep.

**Exponents** are written either as Unicode superscripts or in ASCII
caret form:

| Form | Example | Notes |
|---|---|---|
| Superscript | `m²`, `m⁻³`, `m¹⁰⁰` | U+2070, U+00B9, U+00B2, U+00B3, U+2074-U+2079; sign U+207A (no-op) or U+207B (negate); one to three digits |
| Caret | `m^2`, `m^-3`, `m^100` | `^[+-]?[0-9]{1,3}` |
{: title="Unit exponent forms"}

An exponent is an integer in `[-100, 100]` (`BVN_EXPONENT_MIN`,
`BVN_EXPONENT_MAX`), with zero reserved: `m^0` and `m⁰` are not units.
At most three digits are scanned, so `m^1000` is an unrecognised token
rather than an over-large exponent, while `m^200` scans and is refused
on the range check. Both are `error_unit_illegal`.

A compound unit MUST NOT exceed **32 components**; more is
`error_unit_illegal`. An empty component between separators (`m//s`,
`m*·s`), an empty group, and unbalanced parentheses are likewise
`error_unit_illegal`.

`no_unit` in unit position means explicitly dimensionless. Omitting the
unit parameter from an annotation is equivalent, and an unannotated value
is dimensionless by synthesis; all three are compatible.

## Prefixes {#prefixes}

The 24 SI prefixes from quetta (`Q`, 10^30) to quecto (`q`, 10^-30) are
accepted. The micro prefix is `µ` (U+00B5, encoded `C2 B5`); ASCII `u`
is accepted as an input-only alias, and producers render it as `µ`.

The IEC binary prefixes `Ki` through `Yi` (2^10 to 2^80) are accepted, as
are `Ri` (2^90) and `Qi` (2^100). Note that `Ri` and `Qi` are a
forward-looking extension: {{IEC80000-13}} stops at yobi.

The `~` between a prefix and its unit is **optional**: `k~g` and `kg`
name the same unit, as do `Mi~B` and `MiB`. Disambiguation is by longest
alias suffix, so a bare unit always outranks a prefixed reading of the
same token: `min` is the minute, never milli-inch. A compact spelling is
therefore only ever accepted where the separated form would have been
`error_unit_illegal`, and no document changes meaning because of it.

Prefixes MUST NOT stack: `kkg` and `k~kg` are both `error_unit_illegal`.

The longest-suffix rule resolves most apparent collisions without a
special case, because a token that is itself a registered unit is read as
that unit: `pH`, `mph`, and `kph` are registry entries and are never read
as pico-henry, milli-phot, or kilo-phot. Exactly three compact spellings
are refused by name, because for them the longest-suffix rule would
silently pick one of two defensible readings:

| Spelling | Refused because |
|---|---|
| `usb` | the bus, not microstilb - write `u~sb` or `µ~sb` |
| `kt` | kilotonne or knot - write `k~t` or `kn` |
| `ppt` | parts per trillion or per thousand in use, the picopint by the rule - write `pptr`, `‰` or `p~pt` |
{: title="Compact spellings refused by name"}

All three are `error_unit_illegal`. The separated spellings remain valid, so
the refusal costs nothing but a character.

Which spelling a producer emits depends on what it is doing, and the two
cases differ:

* A producer **serializing a unit it holds as a value** - one it
  constructed, computed, or decoded, rather than copied from an input
  document - MUST emit the canonical separated form `k~g`. The compact
  form is an input convenience; emitting it would make the output
  unreadable to a consumer that predates it, for no gain.

* A producer **reproducing an existing document**, such as a
  pretty-printer or canonicalizer, MAY preserve the spelling its input
  used. Both spellings denote the same unit, so preserving one is not a
  loss, and rewriting it would make round-tripping a document alter
  octets that carry no information.

The reference implementation does exactly this: its writer helpers emit
`k~g`, while its pretty-print path reproduces `kg`, `MiB`, and `u~m` as
written. See {{interop-canonical}} for why this means documents cannot be
compared octet-wise.

IEC prefixes MUST NOT be applied to currency codes. SI prefixes may be.

## Currency {#currency}

216 monetary denominations are registered: 166 alphabetic {{ISO4217}}
codes, including the precious-metal X-codes, and 50 cryptocurrency
tickers.

**The `$` sigil is mandatory.** A component introduced by `$`, after any
prefix, is looked up in the currency table and nowhere else; if the code
is not there, the result is `error_unit_illegal`. A component without `$`
is looked up in the physical-unit table and nowhere else. A bare `USD` or
`CUP` is therefore never a currency.

The consequence is that the two namespaces are **disjoint by
construction**: no currency code can collide with a physical unit symbol,
present or future.

That this matters is easiest to see in the near-misses the registry
already contains. `BTU` is a registered alias of the British thermal
unit, so a currency registry that later adopted a `BTU` ticker would
collide head-on. `cup` is the US cup of volume and `$CUP` the Cuban peso,
which differ today only in case and in the sigil; without the sigil,
letter case alone would carry the distinction between a volume and a
denomination. Under the rule above neither is a question: `BTU` and `cup`
are units because they lack the sigil, `$CUP` is a currency because it
has one, and bare `CUP` is `error_unit_illegal` because it is neither.

~~~
.price = 19.99 $USD;                 # inline currency
.scaled = <float_dec:64,k~$EUR> 250.0;  # thousands of EUR
.rate  = 2351.40 $USD/oz_t;          # compound: USD per troy ounce

# .bad  = <float_dec:64,Ki~$USD> 1;  # IEC prefix on a currency
# .also = <float_dec:64,USD> 1;      # no sigil: error_unit_illegal
~~~

Each currency is its own dimension. A cross-currency conversion is always
refused rather than guessed: the format carries no exchange rates, and it
would be wrong to imply that the pairing is time-invariant.

The registry retains a small number of historical codes for
compatibility - `HRK`, `SLL`, `ZWL`, and `BGN` are examples - and
represents successor pairs such as `ANG` and `XCG` side by side. New data
SHOULD NOT use a retired code.

The registry records a `minor_unit` exponent N for each code, such that
one major unit equals 10^N minor units. Applications reading an
integer-annotated monetary value MUST consult it to place the decimal
point; N is not 2 for every currency.

## Inline Unit Suffix {#inline-units}

A scalar number or string value may carry its unit as a suffix rather
than in an annotation, separated by at least one blank.

~~~ abnf
value        = [ type-ann ws ] [ scalar-with-unit / raw-value ]
scalar-with-unit = ( number / string ) ws-req inline-unit
inline-unit  = inline-start *inline-char
inline-start = ALPHA / "_" / "$" / "%" / "(" / unit-lead
inline-char  = ALPHA / DIGIT / "_" / "$" / "%" / "+" / "-" / "."
             / "/" / ":" / "^" / "*" / "~" / "(" / ")" / "'"
             / "[" / "]" / "{" / "}" / unit-lead / utf8-cont
~~~

The separating blank is mandatory: `9.81 m/s` is valid and `9.81m` is an
error.

~~~
.distance = 100 m;
.speed    = 9.81 m/s;
.mass     = 70.0 k~g;
.storage  = 4 Gi~B;
.ratio    = 3.14 no_unit;
~~~

**An inline suffix is forbidden inside an array.** Any character that
could begin one - a letter, `_`, `$`, `%`, `(`, or a UTF-8 lead octet -
following a value inside `[...]` is `error_unexpected_input_byte`. Use a
whole-array annotation instead. This keeps array elements uniform by
construction rather than by check.

### Interaction With the Annotation

| Situation | Result |
|---|---|
| No annotation, inline unit present | The inline unit is the unit |
| Annotation with no unit, inline unit present | The inline unit is the unit |
| Both present, identical | Valid; redundant |
| Both present, different | `error_unit_mismatch` |
{: title="Annotation and inline unit"}

Two units are identical here when they resolve to the same set of
components - the same base unit, prefix and exponent in each. Order and
spelling are free (`m/s` and `m*s^-1`, `km` and `k~m`); algebra is not
(`N` is not `k~g*m/s^2`).

~~~
.a = <float:64,m> 1.5;        # unit from the annotation
.b = <float:64> 1.5 m;        # unit from the suffix
.c = <float:64,m> 1.5 m;      # valid, redundant

# .d = <float:64,m> 1.5 s;    # error_unit_mismatch
~~~

This last row is the format's central guarantee in its smallest form. The
comparison is structural and order-insensitive, since multiplication
commutes: `N·m` and `m·N` are the same unit and do not mismatch.

An unrecognized unit in either position is `error_unit_illegal`.

An inline unit after a `utf8` string is accepted. The suffix path does
not consult the type family: `<utf8> "text" m` and a bare `"text" m` are
valid and the unit is carried with the value, although the same unit as
an annotation parameter (`<utf8,m>`) is `error_illegal_value_type`. The
suffix exists for quoted *numeric* literals such as `<uint:_16> "ff" m`;
producers SHOULD NOT attach one to text.

## Unit Profiles {#unit-profiles}

Format version 1.2 adds a facility for expressing a unit in a foreign
notation, written `namespace:code`:

~~~
#!bovnar 1.2
.systolic = <float:64,ucum:mm[Hg]> 120.0;
~~~

Seven namespaces are defined: `ucum` {{UCUM}}, `unece` {{UNECE20}},
`qudt` and `qudt-qk` {{QUDT}}, `udunits` {{UDUNITS}}, `om` {{OM2}} and
`cf` {{CF}}. A code is translated, when the document is parsed, into the
unit a native spelling of the same quantity produces, so the facility
adds spellings and no values: every rule of this document applies to the
result unchanged.

**The facility is specified by {{BOVNAR-SPEC}} Section 11.9 and its
companion on unit profiles, not by this document.** Its translation
tables restate other publishers' vocabularies and are revised as those
vocabularies are, which is not a cadence this document can follow. What
this document fixes is the boundary, so that a consumer implementing it
alone behaves predictably:

* In a document that declares less than 1.2, or declares nothing,
  `namespace:code` in unit position is `error_unit_illegal`. In those
  versions it is not a unit.
* In a document that declares 1.2 or newer, a consumer that does not
  implement a namespace MUST reject it as `error_unit_profile_unknown`
  (49), and MUST NOT pass the code through uninterpreted. A consumer that
  implements none of them rejects every such unit that way.
* A consumer that does implement a namespace translates the code into a
  unit of {{units}}, carries it as an **opaque** unit, or rejects it. An
  opaque unit is one the vocabulary defines with no dimension - a UCUM
  arbitrary unit, a UN/ECE package or count code: it equals itself, is
  convertible to nothing, and is distinct from every other unit under
  every rule of this document. Rejection is `error_unit_illegal` for a
  code the vocabulary does not define and
  `error_unit_profile_unsupported` (50) for one it defines and this unit
  system cannot represent. There is no further outcome in which a value
  carries a unit the consumer cannot reason about.

# Composite Values {#composites}

## Arrays {#arrays}

~~~ abnf
array        = array-row *( ws "/" ws array-row )
array-row    = "[" ws [ row-content ] ws "]"
row-content  = filled-elem
             / array-elem 1*( ws "," ws array-elem )
filled-elem  = type-ann ws [ raw-value ] / raw-value
array-elem   = [ type-ann ws ] [ raw-value ]
~~~

An array is one or more bracketed **rows** separated by `/`. Elements
within a row are separated by `,`.

The two separators mean different things. `,` separates *elements*; `/`
separates *dimension rows* of a single rectangular block. `[1,2,3]/[4,5,6]`
is one 2x3 array, not two arrays.

All `/`-rows of one array MUST have the same element count;
`error_array_row_size_mismatch` otherwise. Zero is a valid width, so
`[]/[]` is two empty rows, while `[]/[1]` is an error.

A leading, trailing, or doubled comma denotes a null element, as does the
bare word `null`. `[,1,,2,]` has five elements. An element may be empty
only beside a comma, so `[]` is the empty row and never one null; a type
annotation alone is an element, so `[<uint:8>]` is one typed null.

Array nesting is bounded by `max_array_nesting` and the total element
count by `max_array_items` ({{limits}}).

### Element Homogeneity {#array-homogeneity}

The elements of an array MUST be homogeneous. The rule is *shape uniform,
fields free*:

* **Kind.** Every non-null element shares one kind. `[1, "two"]` and
  `[1, {.x=1;}]` are `error_array_element_type_mismatch`.

* **Unit.** Numeric elements MUST carry the same unit, not merely the
  same physical dimension, though the numeric encodings may mix freely.
  `[1, 2.5, 3]` is valid; `[<float:64,m> 1.0, <float:64,k~g> 2.0]` is
  not, and neither is `[<float:64,m> 1.0, <float:64,k~m> 2.0]`, whose
  elements differ only in scale. The comparison is of the unit and not
  of its spelling. Each currency is its own unit, so a bare array MUST
  NOT mix `$USD` and `$EUR`. (Through version 1.1 this rule read "the
  same dimension"; see {{version-stability}}.)

* **Datetime.** A `datetime` is its own kind and does not mix with the
  plain numeric encodings. Its **epoch is a dimension**, exactly as a
  currency is: a bare array MUST NOT mix epochs.

* **Rectangularity.** Sibling sub-arrays MUST have equal length and
  recursively matching element shape. `[[1,2],[3,4]]` is valid;
  `[[1,2],[3,4,5]]` is `error_array_row_size_mismatch`.

* **Structs: same keys, fields free.** Sibling structs MUST share the
  same keys, in the same order, with the same per-field kinds and
  nesting. Differing keys are `error_struct_shape_mismatch`; a field that
  is a number in one record and a string in another is
  `error_array_element_type_mismatch`. But a scalar field MAY carry a
  different unit in each record, and a list field a different length - so
  a multi-currency ledger and per-record argument lists are both valid.

* **Null is a hole.** A null element matches any shape and neither
  establishes nor breaks homogeneity, so `[1, , 3]` is valid.

The design intent is that a bare array of measurements is uniform, so a
consumer may treat its elements identically without inspection, while
genuinely heterogeneous data is modelled with a struct. One consequence
worth stating plainly: **a ragged or mixed-type JSON array has no Bovnar
representation.** A converter rejects it rather than losing the
structure; {{interop-json}} states that requirement normatively.

These checks require comparing siblings and are therefore enforced at the
materializing tier only; see {{conformance}}.

## Structs {#structs}

~~~ abnf
struct       = "{" ws *( assignment ws ) "}"
~~~

A struct is a nested scope of assignments.

~~~
.person = {
    .name    = "Alice";
    .age     = <uint:8> 30;
    .address = { .city = "Springfield"; };
};
~~~

Keys MUST be unique within one struct ({{model}}). Nesting is bounded by
`max_struct_nesting` ({{limits}}). A `}` at nesting level zero is
`error_illegal_struct_close`. An empty struct, `{}`, is valid.

Struct keys are ordered. A producer MUST preserve the order it was given,
and a consumer MUST make it available, because {{array-homogeneity}}
makes key order significant for sibling records.

## Octet Streams {#octet-streams}

A NUL octet where a value is expected switches to binary chunk mode.
Text-layer UTF-8 validation is suspended for its duration.

~~~ abnf
octet-stream = %x00 *os-chunk %x00
os-chunk     = %x01 os-length os-data
os-length    = 2OCTET      ; little-endian uint16; 0x0000 = 65536
os-data      = *OCTET       ; exactly os-length octets
~~~

Any tag octet other than `00` or `01` is
`error_octet_stream_out_of_sync`. A chunk that ends prematurely is
`error_read_complete_chunk_failed`.

Note the encoding of the length: a length field of `0x0000` denotes
**65536 octets, not zero**. This makes the full 16-bit range usable at
the cost of a special case that implementations MUST get right.

Chunks are **length-prefixed rather than delimited**. A payload may
therefore contain any octet, needs no escaping, does not expand, and can
be skipped without inspection. The cost is that a document containing an
octet stream is **not safe through a transport that rewrites octets**;
see {{security-transport}} and {{interop-transport}}.

Octet stream octets count toward `max_file_size` but not toward
`max_text_bytes`.

# Validation, Limits, and Errors {#validation}

## Limits {#limits}

Two kinds of limit apply, and they behave differently.

**Configurable limits** bound resource consumption. A consumer MUST
enforce each of them and SHOULD allow an application to set it. The
values below are the defaults of the reference implementation; they are
not part of the format, and a document is not invalid merely because some
other consumer is configured more tightly.

| Quantity | Default | Error |
|---|---|---|
| Key length | 255 | `error_identifier_too_long` |
| String length | 65535 | `error_string_too_long` |
| Number length | 65535 | `error_number_too_long` |
| Symbol length | 255 | `error_symbol_too_long` |
| Reference length | 65535 | `error_reference_too_long` |
| Array elements | 2147483647 | `error_too_many_array_items` |
| Text octets | 2147483647 | `error_text_data_too_long` |
| Document octets | **unlimited** | `error_file_too_long` |
| Struct nesting | 64 (hard cap 255) | `error_struct_nesting_too_high` |
| Array nesting | 64 (hard cap 255) | `error_array_nesting_too_high` |
{: title="Configurable limits and their default values"}

**Fixed maxima** are part of the format. They are the same for every
consumer, are not configurable, and a document exceeding one is invalid
everywhere.

| Quantity | Maximum | Error |
|---|---|---|
| Components in one compound unit | 32 | `error_unit_illegal` |
| Parenthesis nesting in a unit | 16 | `error_unit_illegal` |
| `uint` / `sint` / `float` / `datetime` width | 32768 bits | `error_illegal_value_type` |
| Type-annotation body | 255 octets | `error_type_too_long` |
| Inline unit suffix | 255 octets | `error_unit_too_long` |
| `\u{}` escape digits | 6 | `error_illegal_escape_sequence` |
| Octet-stream chunk payload | 65536 octets | (encoded in the length field) |
{: title="Fixed maxima"}

The annotation cap counts the octets of the family and its parameters
together with their `:` and `,`; blanks and comments beside those
separators are not counted. The inline cap counts the octets of the
suffix. A well-formed annotation can exceed its cap - a compound unit of
many long aliases, or a long foreign code ({{unit-profiles}}) - and is
then `error_type_too_long`, so the caps are limits on valid documents and
not only on malformed ones.

The document-size default of *unlimited* deserves emphasis. It sets no
cap on the total, so octet-stream payload runs without bound; the text
of a document is still bounded by the text-octet limit, which a consumer
of an endless text stream raises as well. That is the right default for
a telemetry pipeline and the wrong one for an untrusted input. Deployments that accept documents from an untrusted source MUST
set an explicit cap; see {{security-dos}}.

Setting a configurable limit to zero selects the default rather than
forbidding the construct, with one exception: a document-octet limit of
zero means *unlimited*.

## Error Model {#error-model}

A consumer reports an error with a code, and SHOULD report the line,
column, and octet offset at which it was detected.

The error codes named throughout this document each have a stable numeric
value, enumerated in {{error-codes}}. Those numeric values are part of
the format's stability contract: an existing value never changes, and a
new code is appended above the current maximum. Conformance harnesses
({{test-corpus}}) compare implementations at the level of these codes, so
an implementation that reports its own codes SHOULD provide a mapping
onto them.

A consumer MUST report the code this document specifies for a given
violation. Where a single input violates more than one rule, which code
is reported is implementation-defined, but a consumer MUST report at
least one and MUST reject the document.

## Error Recovery {#error-recovery}

A consumer MAY offer a **recovery mode** in which parsing continues after
an error rather than halting. When it does:

1. The error is reported.
2. Octets are skipped, tracking `[]` and `{}` nesting.
3. Parsing resumes at whichever comes first: a `;` at the nesting depth
   saved when recovery began, or the **start of the next assignment** - a
   `.` at that depth followed by an octet that can begin a key.
4. A count of recovery events and a running total of skipped octets MUST
   be maintained and MUST be readable by the application.

An error **inside a comment** - an invalid UTF-8 sequence or a rejected
control octet - is the exception to steps 2 and 3. It is reported and
counted, one recovery event and one skipped octet, and the comment runs
on to its line end; nothing is unwound and no value is lost.

Both resumption boundaries are needed, and for different errors. When the
error is *inside* a statement, that statement's own `;` is the next one,
so the `;` rule discards exactly the broken statement. When the error is
*between* statements - a stray octet in the whitespace separating two
assignments - the next `;` belongs to the following, perfectly good
statement, and resuming only there would discard it whole however large
it is. The assignment boundary stops recovery at the first point the
document plausibly becomes readable again.

The `.` must be followed by an octet that can begin a key, so a `.`
inside skipped junk - in `1.5`, in `.5`, in binary corruption - is just
another skipped octet. A `.` inside a bracket opened since recovery began
opens no assignment either.

If end of input is reached while recovering,
`error_got_incomplete_bvnr_stream` is reported **in addition to** the
original error, in that order.

The converse case is worth stating because it is easy to misread: if the
input simply ends part-way through a value, with no earlier error, only
the truncation is reported. A value that would have been rejected had it
been terminated is never checked, because the check happens at the
terminator. Truncating a document can therefore *hide* a defect rather
than add one, and a consumer MUST NOT infer from a lone truncation error
that the rest of the document was well-formed.

### What the Two Counters Mean {#recovery-counters}

The count of recovery events and the skipped-octet total answer different
questions, and **neither alone establishes that a document arrived
intact**:

* **Recovery count** is the number of times an error put the parser into
  recovery. It is the signal that *something was dropped*. Whenever it is
  non-zero, at least one value or assignment was rejected and is absent
  from what the consumer received - or, the one case that drops nothing,
  an octet inside a comment was.

* **Skipped octets** is how much raw input was discarded unparsed: from
  the octet the error was raised on up to, not including, the boundary
  parsing resumed at. It says what recovery *cost in text*, and it is the
  only way to learn that: the skipped octets were never parsed, so no
  other event mentions them.

A recovery can drop a value while skipping **zero** octets. Where a value
is rejected and the very next octet is the statement's own `;`, there is
nothing to skip - yet the assignment is delivered with no value attached,
and a consumer watching only the skipped total would conclude that
nothing was lost. Applications MUST therefore treat a non-zero recovery
count as authoritative for "the document did not arrive intact", and read the skipped total
only to learn how much text that cost. See {{security-recovery}}.

Recovery mode MUST NOT be the default.

# Conformance {#conformance}

## Producers {#conformance-producers}

A conforming producer emits documents that satisfy this document. Its
obligations are stated where each construct is defined; they are
collected here because an implementer writing a writer has no other
single place to find them.

A producer MUST:

* emit well-formed UTF-8 in the text layer ({{encoding}});
* **emit a version declaration when the document uses any construct
  introduced after version 1.0** ({{version}}) - the `datetime` family,
  the `\x` and `\u{}` string escapes, and reference array indexing,
  which need 1.1, and a `namespace:code` unit, which needs 1.2. A
  document using one without declaring its version is invalid, and the failure is
  easy to miss because every *other* value in it is usually fine;
* emit the canonical separated prefix form `k~g` when serializing a unit
  it holds as a value; a producer reproducing an existing document MAY
  instead preserve that document's spelling ({{prefixes}});
* preserve the order of struct keys ({{structs}});
* emit documents valid at the **materializing** tier ({{conformance-tiers}}) -
  unique keys within a scope, homogeneous arrays, rectangular sibling
  sub-arrays, consistent sibling record shapes - even when the intended
  consumer is a streaming one that would not detect a violation.

A producer SHOULD:

* omit the byte order mark ({{encoding}});
* use the `.bvnr` extension ({{interop-detection}});
* write line endings consistently, and consider the Net-Unicode profile
  of {{RFC5198}}, when targeting a text channel ({{interop-transport}});
* offer a mode that refuses to emit an octet stream, for use when the
  destination channel is not binary-safe ({{interop-transport}}).

A producer MUST NOT rely on a version declaration to cause an older
consumer to reject a document: to a version 1.0 consumer the declaration
is an ordinary comment ({{security-version}}).

## Tiers {#conformance-tiers}

Four of this document's rules require comparing sibling values that a
streaming consumer sees one at a time and cannot retain in bounded
memory. The format therefore defines two conformance tiers.

A **streaming consumer** processes a document as a sequence of events. It
MUST enforce every rule in this document except the four listed below.

A **materializing consumer** constructs a tree of the whole document. It
MUST additionally enforce:

| Rule | Error |
|---|---|
| Array element kind and unit ({{array-homogeneity}}) | `error_array_element_type_mismatch` |
| Ragged sibling sub-arrays ({{array-homogeneity}}) | `error_array_row_size_mismatch` |
| Sibling struct shape ({{array-homogeneity}}) | `error_struct_shape_mismatch` |
| Duplicate key in one scope ({{model}}) | `error_duplicate_struct_key` |
{: title="Materializing-tier rules"}

An implementation MUST document which tier it implements. An
implementation offering both interfaces MUST apply the materializing-tier
rules on its materializing interface.

The same distinction is drawn under other names elsewhere, and an
implementer reading more than one document will meet all three: what this
document calls the **materializing tier** is the *DOM tier* in
{{BOVNAR-SPEC}} and is reported as *document-tier* validation by the
reference command-line tool. They are one concept.

The consequence for interoperability is direct and is the most likely
source of disagreement between two conforming implementations: **a
streaming consumer accepts documents a materializing consumer rejects.**
See {{security-differential}}.

Concretely, each document below is accepted in full by a streaming
consumer and rejected by a materializing one.

| Document | Materializing-tier error |
|---|---|
| `.a = 1; .a = 2;` | `error_duplicate_struct_key` |
| `.b = [1, "two"];` | `error_array_element_type_mismatch` |
| `.c = [[1,2], [3,4,5]];` | `error_array_row_size_mismatch` |
| `.d = [{.x=1;}, {.y=1;}];` | `error_struct_shape_mismatch` |
| `.e = [<float:64,m> 1.0, <float:64,k~g> 2.0];` | `error_array_element_type_mismatch` |
{: title="Documents the two tiers judge differently"}

Note what the third row is *not*. `[1,2,3]/[4,5]` is a `/`-row width
mismatch, and a **streaming** consumer catches that one as each row
closes ({{arrays}}). The rule that needs the whole tree is the one about
**sibling sub-arrays**, because nothing obliges a sibling to be adjacent
in the octet stream. The two cases look alike and report the same error
code; only one of them splits the tiers.

## Test Corpus {#test-corpus}

A conformance corpus and a driver protocol, `bvnr-conformance-v1`, are
published with the reference implementation {{BOVNAR-CONFORMANCE}}. An
implementation claiming conformance to this document SHOULD pass that
corpus at its declared tier and SHOULD state which test groups it was run
against.

## Version Stability {#version-stability}

Within a major version, the format is additive only.

A document valid under version 1.0 remains valid, and decodes to the same
values, under every 1.x revision. This covers the lexical structure, the
type families and their annotations, arrays including the homogeneity
rules, structs, octet streams, references, and the numeric error-code
values.

What may grow in a minor revision: the unit and currency registry; the
accepted *input* spellings of a unit, given that canonical output is
unchanged; new error codes appended above the maximum; new optional
limits and options whose defaults preserve behaviour; and constructs
gated behind a version declaration ({{version}}).

What requires a major revision: anything that could render a valid
document invalid, change how it decodes, renumber an error code, or
change what an existing document's octets mean.

Version 1.2 made two corrections that refuse documents a 1.1 consumer
accepted, and they are recorded here because they bend the first
paragraph of this section. Whitespace inside a type-annotation parameter
is now code 52 ({{error-codes}}); the grammar never derived it, but the
reference consumer deleted it, so that `<float:64,m in>` was read as the
minute. And array homogeneity is by unit where it was by dimension
({{array-homogeneity}}), because the rule as written let a consumer that
treated the elements of `[1 m, 2 ft]`-shaped data identically - as the
same section told it it could - compute wrong values. Neither changes
what any accepted document means; both apply to every document whatever
version it declares.

The reference consumer that implements 1.2 (library 2.0.0) also fixed defects that had let through, or
misread, input this format already excluded: a byte-order mark after
leading blanks, a malformed version declaration ended by end-of-input, an array
left open at its `;`, a `,` after the value of a struct member inside an
array (`[{.x = 7, 1;}]`, which had been read as `71`), a `]` that
closed an array from inside a struct element (`[{.x = 1];}`), blanks or
a comment inside the `float_fix` or `float_dec` keyword, octets directly
after a reference index (`&.a[0]b`), a comment cut off inside a UTF-8
sequence at end-of-input, quoted digits that spell `nan`, `inf` or
`ninf` in a base of 24 or more and exceed their width, and a fractional
`datetime` literal whose UTC year leaves 0000-9999 are now errors, as are
a zero width on `utf8` or `bool` (`<utf8:0>`) and an annotation before
an empty struct (`<utf8> {}`; before a struct with members it always
was). The
width of a `/`-separated row counted a nested array twice, which
accepted some ragged arrays (`[[1]]/[null,null]`) and refused some
rectangular ones (`[[1],[2]]/[null,null]`); and a consumer that checks
homogeneity compared each element with the first non-null one only, so
that a `null` there hid a mismatch behind it, while some rectangular
nestings of `/`-matrices (`[[[1,1]/[2,2], null], [null, null]]`) were
refused as ragged and a path into one resolved to the wrong element. A
version declaration padded with more than 21 blanks was refused and is
accepted. An element annotation without a unit no
longer strips the unit from the array elements after it; and a `tai`
literal spelling a known leap second (`2016-12-31T23:59:60Z`) decodes one
second lower than it did, having previously collapsed onto the second
after it. That, the element that keeps its unit, and a zero mantissa
under an exponent too long to count (`0e999999999999999999999`, which
had been delivered as infinity) are the cases in which an accepted
document decodes to a different value. In the registry {{BOVNAR-UNITS}}, eight conversion
factors were corrected in the same release (`elle`, `klafter`, `rute`,
`morgen`, `inHg`, `Da`, `slug`, `ft_lb`): a document's digits are
unchanged, and what they convert to is not. Conversion is outside this
document, so the correction is noted here and specified there.

# Interoperability Considerations {#interop}

## Transport {#interop-transport}

A document containing an octet stream ({{octet-streams}}) MUST be treated
as binary end to end. Because chunks are length-prefixed rather than
delimited, a transport that rewrites line endings will desynchronize the
length fields, and the result is *unrecoverable rather than merely
mangled*: it reads as a malformed document, and no amount of care at the
receiver reconstructs it.

Deployments SHOULD therefore:

* store such documents as binary, marking them so in version control and
  in build tooling;
* transmit them over channels that do not transform content, and, over
  HTTP {{RFC9110}}, avoid any intermediary performing text
  transformation; and
* where a channel must remain text, have the *producer* refuse to emit an
  octet stream, rather than assume none will occur. Implementations
  SHOULD offer a **text-only mode** that turns this assumption into an
  enforced constraint: a consumer so configured rejects a document
  containing an octet stream with `error_octet_stream_forbidden`, even
  though that document is perfectly valid. The rejection is an assertion
  about the channel, not a judgement about the document
  ({{error-codes}}).

A document with no octet stream is well-formed UTF-8 text throughout and
is safe on a text channel, subject to the usual caveat about line-ending
normalization changing octets inside string literals. Producers targeting
a text channel SHOULD write line endings consistently and SHOULD consider
the Net-Unicode profile of {{RFC5198}}.

## Comparison and Signing {#interop-canonical}

This document does **not** define a canonical form, and two documents
that denote the same data need not be octet-identical. In particular:

* `k~g` and `kg` are the same unit spelled two ways, and a
  reader-driven pretty-printer may preserve the input spelling.
* `µ~m` and `u~m` are the same unit.
* `<uint:32,_10,no_unit>` may be written with its parameters in any
  order.
* A datetime may be written as an ISO literal or as its integer carrier.
* Omitting the unit parameter, writing `no_unit`, and writing no
  annotation at all are three spellings of dimensionless.

Applications MUST NOT compare documents octet-wise to decide whether they
denote the same data, and MUST NOT sign the text of a document unless it
has first been passed through a producer that emits the canonical
spellings, or unless the signature is understood to cover those exact
octets and nothing more.

## Mapping To and From JSON {#interop-json}

A Bovnar document maps to JSON {{RFC8259}} lossily in one direction and
partially in the other.

Bovnar to JSON loses the type annotation, the unit, the distinction
between `uint`, `sint`, `float`, `float_fix`, and `float_dec`, the
distinction between a symbol and a string, the reference kind, and any
octet stream. A converter SHOULD NOT discard that silently: it SHOULD
either carry the discarded information into a sidecar structure or report
what was lost. The reference converter reports, naming the values that
lost a unit and the symbols that became strings.

JSON to Bovnar succeeds for scalar values, objects, and homogeneous
arrays. It fails, and MUST fail rather than distort, for:

* a ragged or mixed-type array, which has no representation
  ({{array-homogeneity}});
* an object with duplicate keys ({{model}});
* an object key that is not a valid Bovnar key ({{lexis}}) - an empty
  key, one beginning with a digit, or one containing a blank or any of
  the excluded punctuation;
* a number whose magnitude exceeds the target annotation - an integer
  beyond its width, or a number that would overflow to infinity.

A JSON number with more precision than IEEE 754 binary64 carries is not
such a failure: it is rounded to nearest, as any JSON consumer rounds it
{{RFC8259}}.

JSON has no unit, so a converted document is dimensionless throughout.
Converting *to* Bovnar is therefore an opportunity to add units, not an
operation that recovers them.

## File Naming and Detection {#interop-detection}

The canonical file extension is `.bvnr`. The longer `.bovnar` is also
registered ({{media-type}}) and SHOULD be recognized by tooling that
dispatches on extension, but producers SHOULD emit `.bvnr`.

There is no magic number. A version-declaring document begins with
`#!bovnar `, which is a usable heuristic but is not required and is not
present in the majority of documents. Content-based detection SHOULD rely
on the media type where one is available, and otherwise on the extension.

# Media Type Registration {#media-type}

The media type `text/vnd.bovnar` is registered in the vendor tree of the
IANA Media Types registry. This section restates that registration in the
template of {{RFC6838}}, corrected and extended to match this document.
See {{iana}} for the action requested, and {{registration-corrections}}
for the substantive differences from the registration as it stands.

Type name:
: text

Subtype name:
: vnd.bovnar

Required parameters:
: N/A

Optional parameters:
: `charset` - per {{RFC6657}}. The encoding of a Bovnar document is fixed
  at UTF-8 by {{encoding}}, so `utf-8` is the only interoperable value
  and is the assumed default. The parameter carries no information and
  SHOULD be omitted. A recipient encountering any other value MUST reject
  the document rather than attempt transcoding.

Encoding considerations:
: **binary**. Although the text layer of a Bovnar document is UTF-8, a
  document may contain octet-stream regions ({{octet-streams}}) holding
  arbitrary octets, including NUL and unpaired CR. Such a document is not
  safe on a channel that transforms content; see {{interop-transport}}.
  A document containing no octet stream is 8-bit UTF-8 text.

Security considerations:
: See {{security}} of this document.

Interoperability considerations:
: See {{interop}} of this document. Two points bear on
  interchange specifically: implementations may conform at either of two
  tiers, and a streaming implementation accepts documents a materializing
  one rejects ({{conformance}}); and no canonical form is defined, so
  documents denoting the same data need not be octet-identical
  ({{interop-canonical}}).

Published specification:
: This document; {{BOVNAR-SPEC}}.

Applications that use this media type:
: Scientific and industrial data logging, instrument and telemetry
  interchange, laboratory and metrology records, engineering simulation
  input and output, financial records requiring exact decimal and
  denomination-tagged values, and configuration for systems where a
  quantity's dimension matters.

Fragment identifier considerations:
: None defined by this document. See {{iana-open}}.

Additional information:
: Deprecated alias names: none.
  Magic number(s): none. A document declaring a version begins with the
  octets `23 21 62 6F 76 6E 61 72` (`#!bovnar`), but the declaration is
  optional.
  File extension(s): `.bvnr` (canonical), `.bovnar`
  Macintosh file type code(s): none

Person and email address to contact for further information:
: Janos Sonntag <bovnar@mail.de>

Intended usage:
: COMMON

Restrictions on usage:
: None

Author:
: Janos Sonntag

Change controller:
: Janos Sonntag

# IANA Considerations {#iana}

IANA is requested to update the existing registration of
`text/vnd.bovnar` in the vendor tree of the Media Types registry to the
template given in {{media-type}}, and to add a reference to this
document.

No new registry is requested, and no other registry is affected.

## Corrections to the Existing Registration {#registration-corrections}

The template in {{media-type}} differs from the registration as it
currently stands in the following substantive ways. The first is a defect
correction and is the reason this document requests an update at all.

1. **Encoding considerations change from `8bit` to `binary`.** The
   existing registration states that embedded binary data is carried as
   escaped octet streams of the form `\xNN`, so that the octet stream
   remains text and no transfer encoding is required. This is incorrect.
   The `\xNN` form is a documentation convention used in {{BOVNAR-SPEC}}
   and in this document to write an octet stream on a printed page; it is
   not the wire format. On the wire an octet stream is introduced by a
   literal NUL octet and carries raw, length-prefixed chunks that may
   contain any octet ({{octet-streams}}). Pasting the escaped notation
   into a document verbatim is a parse error.

   The practical consequences of the correction are significant, and are
   the ones an implementer most needs to know: a document containing an
   octet stream contains NUL octets and unpaired CR, is not 8-bit clean
   text, requires a content-transfer-encoding on any channel that is not
   binary-safe, and is destroyed rather than merely altered by line-ending
   normalization ({{security-transport}}). A document containing no octet
   stream remains 8-bit UTF-8 text and is unaffected.

2. **The published specification is cited as this document, and by
   location-independent URL.** The existing registration cites five files
   in the reference implementation's repository -
   `github.com/sothis/bovnar/blob/main/doc/<name>` - naming the
   specification, the unit system, the grammar, the FAQ and the
   conformance suite. Those paths are an artifact of one repository's
   layout at one moment: the documentation set has since been renumbered,
   and every one of the five citations named a file that no longer exists
   under that name. Pointer files were restored at the cited paths to keep
   the published registration resolving, but a registry entry should not
   depend on a repository preserving a filename indefinitely. The template
   in {{media-type}} therefore cites this document first, with
   {{BOVNAR-SPEC}} as the versioned companion, under `www.bovnar.io` URLs
   that are redirected across renames rather than invalidated by them.

3. **Security considerations are substantially expanded.** The existing
   entry covers input size, nesting depth, reference validation, numeric
   range checking, and opaque binary payloads. {{security}} of this
   document retains all five and adds the parser-differential risk
   between conformance tiers ({{security-differential}}), silent data
   loss under error recovery ({{security-recovery}}), version leniency
   ({{security-version}}), transport corruption of length-prefixed chunks
   ({{security-transport}}), leap-second table drift
   ({{security-leap}}), non-normalized and confusable keys
   ({{security-confusable}}), and the bypass of range validation by
   special numeric values ({{security-numeric}}).

4. **Interoperability considerations gain the two facts most likely to
   cause disagreement between conforming implementations**: the two
   conformance tiers and the documents that separate them
   ({{conformance}}), and the absence of a canonical form
   ({{interop-canonical}}).

5. **Fragment identifier considerations change from "not applicable" to
   "none defined."** The distinction matters because a fragment syntax is
   plausible for this format and may yet be defined; see
   {{iana-open}}.

6. **The `charset` parameter entry** is unchanged in substance and is
   restated to name {{RFC6657}} explicitly.

7. **The contact address changes to `bovnar@mail.de`.** The registration
   currently names a personal address; the role address is preferred so
   that the contact survives independently of any individual. The person
   named as contact, author, and change controller is unchanged.

## Open Registration Questions {#iana-open}

The following are open and are called out for review rather than
resolved:

1. **Tree.** The format is registered in the vendor tree as
   `text/vnd.bovnar`. Publication of an open specification would
   ordinarily justify a standards-tree registration, `text/bovnar`, with
   the vendor name retained as a deprecated alias. This document does not
   request that, because it is an Informational individual submission;
   the question should be settled if the format is taken up on the
   standards track.

2. **Top-level type.** `text` is a poor fit for a format that may embed
   arbitrary octets. The alternatives are to keep `text` with the
   encoding considerations above; to move to `application/bovnar`; or to
   register both, with `text/...` restricted to documents containing no
   octet stream and `application/...` unrestricted. The third is the most
   honest and the most disruptive.

3. **Structured syntax suffix.** If formats come to be layered on Bovnar
   the way they are on JSON and XML, a `+bvnr` suffix would be warranted.
   None exist today, and registering the suffix before there is a
   consumer for it would be premature.

4. **Fragment identifiers.** The reference implementation exposes a path
   syntax for addressing a value within a document -
   `.matrix[0][1]` - which is a natural candidate for fragment
   identifier semantics under {{RFC3986}}. Defining it would require
   specifying resolution against the document tree, including the
   treatment of references, which this document deliberately leaves to
   the application ({{references}}). It is deferred.

# Security Considerations {#security}

## Resource Exhaustion {#security-dos}

The format admits several ways for a small input to demand a large amount
of work, and the defaults of {{limits}} are chosen for trusted pipelines
rather than for hostile input.

* **Document size is unbounded by default.** A consumer accepting input
  from an untrusted source MUST set an explicit cap. Only the text is
  otherwise bounded, by the text-octet limit; octet-stream payload is
  not.

* **Integer widths reach 32768 bits.** A single `<uint:32768>` value
  requires 4 KiB of storage and arbitrary-precision arithmetic to
  validate its range. `<float:32768>` and `<float_dec:256>` are
  comparable. An implementation that maps these onto a bignum library
  inherits that library's performance characteristics on adversarial
  input.

* **Non-decimal bases multiply digit counts.** A non-decimal value is
  written as a quoted string ({{numbers}}), so a 32768-bit integer in
  base 2 is a 32768-character string literal - comfortably inside the
  default `max_string_length` of 65535. Base conversion cost is
  superlinear in most implementations.

* **Nesting defaults to 64 with a hard cap of 255.** A recursive-descent
  consumer MUST bound its own stack independently rather than rely on the
  document's declared depth, since the depth is only known after the
  octets are read.

* **Array element counts default to 2^31-1.** Combined with the small
  per-element syntax `[,,,,,]`, an attacker obtains a high ratio of
  allocated elements to input octets. Materializing consumers are more
  exposed than streaming ones.

* **String concatenation** accumulates toward `max_string_length` across
  an unbounded number of adjacent literals, so a consumer MUST check the
  limit against the running total, not against each literal.

Deployments SHOULD set every limit in {{limits}} explicitly at a trust
boundary, and SHOULD prefer a streaming consumer where the application
permits.

## Parser Differentials Between Tiers {#security-differential}

The two conformance tiers of {{conformance}} accept different languages.
A streaming consumer accepts a document with duplicate keys, a
heterogeneous array, a ragged sibling sub-array, or mismatched sibling
records; a materializing consumer rejects all four.

This is the classic precondition for a parser-differential attack. If one
component validates a document and a second acts on it, and the two are
at different tiers, an attacker can craft a document that the validator
accepts and the actor interprets differently, or that the validator
rejects while a permissive actor proceeds. Duplicate keys are the sharpest
case: which binding wins is not defined by this document, because at the
materializing tier the document is invalid and at the streaming tier both
bindings are simply delivered in order.

Deployments MUST use the same tier for validation and for action on the
validated data, and SHOULD prefer the materializing tier wherever a
security decision depends on the document's content. An implementation
that validates at one tier and acts at another is misconfigured
regardless of the correctness of either.

## Recovery Mode Silently Discards Data {#security-recovery}

When recovery mode is enabled, a consumer continues past errors and
delivers a *subset* of the document without any indication in the data
itself that a subset is what it is.

An application that enables recovery and ignores the skipped-octet total
may therefore act on a document from which arbitrary assignments have
been removed. Where the document is a security policy, an access control
list, or a set of calibration constants, a removed assignment is likely
to fail open.

Recovery mode MUST NOT be enabled when the parsed document informs a
security decision. Where it is enabled, an application MUST check the
**recovery count** and MUST treat a non-zero value as a failure of the
document, not as a diagnostic.

Checking the skipped-octet total instead is not sufficient, and the
difference is a trap: a rejected value whose statement terminator follows
immediately skips **zero** octets, so the total stays at zero while the
assignment arrives stripped of its value ({{recovery-counters}}). An
access-control entry that loses its value this way is indistinguishable,
to a consumer watching only the byte total, from one that was never
there.

## Version Leniency {#security-version}

By default a consumer accepts a declared version newer than it supports
and fails only when it encounters a construct it does not implement
({{version}}). A document declaring `#!bovnar 2.0` and using only 1.0
constructs therefore parses without complaint on a 1.0 consumer, and the
producer's intent that it be read as a 2.0 document is lost.

Furthermore, a version 1.0 consumer treats the declaration as an ordinary
comment and does not see it at all. A producer MUST NOT rely on the
declaration to cause rejection by an older consumer.

Consumers handling input from an untrusted source SHOULD enable strict
version checking, so that an unsupported declared version is rejected at
the directive rather than possibly not at all.

## Transport Corruption of Octet Streams {#security-transport}

Octet-stream chunks are length-prefixed ({{octet-streams}}). A transport
that rewrites a CR inside a payload shifts every subsequent length field,
and the reader then interprets payload octets as tag and length fields.

The failure is loud rather than silent in the common case - it produces
`error_octet_stream_out_of_sync` quickly - but it is not guaranteed to
be. An attacker with the ability to induce or exploit such a rewrite can
in principle steer the desynchronized read, since the octets being
reinterpreted as length fields are attacker-supplied payload. A
sufficiently long chunk length read out of a corrupted field also becomes
an allocation request.

Implementations MUST bound the memory they commit to a single chunk
independently of the declared length, MUST treat the `0x0000` length as
65536 rather than zero, and MUST NOT assume a chunk boundary aligns with
anything in the payload. Deployments SHOULD carry documents containing
octet streams over transports that do not transform content.

## Reference Resolution {#security-references}

References are stored unresolved and are never followed by the parser
({{references}}). Every hazard therefore lands in the application that
chooses to resolve them:

* **Cycles are not detected.** A naive resolver following `.a = &.b;
  .b = &.a;` does not terminate. A resolver MUST bound its depth or
  detect cycles.
* **Dangling paths are valid.** A resolver MUST define what a missing
  target means and MUST NOT treat it as an empty or default value by
  accident.
* **A reference path is not a URI.** It is an opaque string with no
  authority, scheme, or network semantics. An application MUST NOT
  interpret a reference as a location to fetch, and MUST NOT pass a
  reference path to a resolver that might.
* **Index syntax is uninterpreted.** `&.a[0][1]` is stored verbatim,
  including the digits. A resolver MUST range-check the indices against
  the actual array rather than trust them.

## Numeric Interpretation {#security-numeric}

* **Special numbers bypass range validation.** `<uint:8> nan` and
  `<sint:16> ninf` are valid documents ({{types}}). A consumer that maps
  values onto fixed-width integers MUST handle a non-finite value
  arriving on an integer channel, and MUST NOT assume that a successful
  parse means the value fits the declared width.

* **Fixed-point encoders saturate.** An implementation encoding a
  `float_fix` value SHOULD saturate at the representable extreme rather
  than wrap, so that an out-of-range datum cannot silently decode to an
  unrelated value. The format itself rejects such a value, but a producer
  that computes one MUST NOT emit a wrapped result in its place.

* **Width is declared, not enforced by storage.** The annotation says how
  wide the value is; nothing in the document proves the producer honoured
  it. Range validation at the consumer is the only check.

* **Base 64 and base 85 are `uint` only.** Their alphabets use `+` and
  `-` as digits, so a signed value cannot be distinguished from a value
  containing those digits. An implementation that relaxes this to permit
  signed values in these bases introduces an ambiguity the format does
  not have.

## Units Are Validated, Not Verified

The unit system rejects a unit that is malformed, unregistered, or
contradicted by an inline suffix on the same value. It does not - and
cannot - verify that the value is a plausible measurement in that unit,
that two assignments in the same document are dimensionally consistent
with each other, or that the producer measured what it claims.

"It parsed" therefore means the dimension is *stated and internally
consistent*, not that it is *correct*. Applications performing physical
or financial computation MUST still validate ranges and cross-field
relationships.

Where an implementation offers read-time unit conversion, a further trap
awaits the caller. Asking for a value in some unit is a *request*, not a
guarantee. In the reference implementation a target *list* of units
leaves a value it cannot convert - the quantities are of different
dimension, or they are two different currencies - **in its original
unit**, unconverted, and reports no error; a rule naming one field by its
key path fails with `error_unit_mismatch` instead. Only the first needs
the check. An application that assumes its request succeeded then holds a
number it believes is in one unit and which is in fact in another, which
is precisely the failure this format exists to prevent. A caller
requesting a conversion MUST check whether it actually occurred before
using the result, and MUST NOT infer success from the absence of an
error.

Cross-currency conversion in particular is never performed rather than
approximated, which is the safe behaviour: an implementation that
silently applied a rate would be asserting a time-varying fact the
document does not contain. What the caller gets back is the original
amount in the original denomination.

## Leap-Second Table Drift {#security-leap}

The `tai` epoch requires a leap-second table, which is a static snapshot
of an IERS bulletin ({{datetime-literals}}). Two implementations built at
different times may convert the same civil literal to different `tai`
values for instants after the older build's table ends.

Applications MUST NOT assume `tai` conversion agreement across
implementations of differing vintage, and SHOULD transmit an integer
carrier rather than a literal where exact agreement matters. Comparing
timestamps that originated as literals converted by different builds may
yield an ordering that is wrong by one or more seconds.

## Confusable and Non-Normalized Keys {#security-confusable}

Keys admit arbitrary non-ASCII characters and are compared as octet
sequences with no Unicode normalization ({{encoding}}). Consequently:

* Two keys that are canonically equivalent under {{UNICODE}} but differ
  in octets are distinct keys and do not collide under the duplicate-key
  rule.
* Two keys that are visually identical but differ in code point - Latin
  and Cyrillic homoglyphs, for instance - are distinct keys, and a human
  reviewing the document cannot tell them apart.

Where keys drive authorization, routing, or any other decision, an
application MUST NOT rely on visual inspection and SHOULD apply its own
normalization and confusable-detection policy before matching. Producers
SHOULD restrict keys to ASCII where the application permits.

## No Confidentiality, Integrity, or Authenticity

A Bovnar document is passive data. The format defines no encryption, no
checksum, no signature, and no notion of an author. It carries no
executable content and no construct that instructs a consumer to fetch,
include, or execute anything - references are inert strings
({{security-references}}) and there is no include directive - so a
conforming consumer performs no I/O beyond reading the octets it is
given.

Confidentiality, integrity, and authenticity MUST therefore come from the
transport or from an enclosing envelope. An application signing a
document MUST first read {{interop-canonical}}: because no canonical form
is defined, a signature over the octets is a signature over one *spelling*
of the data, and a producer that re-emits the document invalidates it
without changing its meaning.

## Information Disclosure

Comments are preserved in the octets of a document and are not part of
its data. A producer that copies a document forward carries its comments
with it. Applications MUST NOT place secrets in comments, and SHOULD
strip comments from documents crossing a trust boundary outward.

A unit annotation is itself information: the fact that a field is
denominated in a particular currency, or measured in a particular unit,
may reveal more about a system than the value does.

--- back

# Collected ABNF {#collected-abnf}

All literals are case-sensitive ({{grammar-notation}}). Rules named in
{{RFC5234}} Appendix B.1 - `ALPHA`, `DIGIT`, `HEXDIG`, `HTAB`, `SP`,
`CR`, `LF`, `DQUOTE`, `OCTET` - are used without redefinition.

~~~ abnf
; ---- document --------------------------------------------------

stream       = [ BOM ] *blank [ version-decl ] ws *( assignment ws )
BOM          = %xEF.BB.BF

assignment   = "." key ws "=" ws value ws ";"

; ---- version declaration ---------------------------------------

version-decl = "#" %s"!bovnar" 1*hspace version-int "." version-int
               *hspace [ CR / LF ]
hspace       = HTAB / SP
version-int  = "0" / ( %x31-39 *DIGIT )

; ---- whitespace and comments -----------------------------------

ws           = *( blank / comment )
ws-req       = ( blank / comment ) *( blank / comment )
blank        = HTAB / LF / VT / FF / CR / SP
VT           = %x0B
FF           = %x0C
comment      = "#" *comment-char [ CR / LF ]
comment-char = HTAB / VT / FF / %x20-7E / %x80-FF

; ---- octet classes ---------------------------------------------

utf8-cont    = %x80-BF
utf8-lead    = %xC3-DF / %xE0-EF / %xF0-F4
unit-lead    = %xC2-F4

; ---- keys, symbols, references ---------------------------------

key          = id-start *id-body
id-start     = ALPHA / "_" / utf8-lead
id-body      = id-start / "+" / "-" / DIGIT / utf8-cont

symbol       = id-start *id-body

reference    = "&" ref-seg *( ref-seg / ref-index )
ref-seg      = "." id-start *id-body
ref-index    = "[" 1*DIGIT "]"

; ---- values ----------------------------------------------------

; An absent value is the null value; hence the outer option.
value        = [ type-ann ws ] [ scalar-with-unit / raw-value ]

; The reserved words below outrank "symbol", which they also
; match, and dt-literal outranks "number", whose leading digit
; run it shares. See Section 2.3.
raw-value    = %s"null"
             / bool-value
             / special-num
             / dt-literal
             / number
             / string
             / symbol
             / reference
             / array
             / struct
             / octet-stream

scalar-with-unit = ( number / string ) ws-req inline-unit

bool-value   = %s"true" / %s"false" / %s"on" / %s"off"
special-num  = %s"nan" / %s"inf" / %s"ninf"

; ---- numbers ---------------------------------------------------

number       = [ "-" ] ( int-led / dot-led ) [ dec-exp ]
int-led      = 1*DIGIT [ "." *DIGIT ]
dot-led      = "." 1*DIGIT
dec-exp      = ( %s"e" / %s"E" ) [ "+" / "-" ] 1*DIGIT

; ---- datetime literals -----------------------------------------

dt-literal   = dt-date [ %s"T" dt-time [ "." 1*DIGIT ] [ dt-zone ] ]
dt-date      = 4DIGIT "-" 2DIGIT "-" 2DIGIT
dt-time      = 2DIGIT ":" 2DIGIT ":" 2DIGIT
dt-zone      = %s"Z" / ( "+" / "-" ) 2DIGIT ":" 2DIGIT

; ---- strings ---------------------------------------------------

string       = string-lit *( ws string-lit )
string-lit   = DQUOTE *string-char DQUOTE
string-char  = safe-byte / escape
safe-byte    = %x09-0D / %x20-21 / %x23-5B / %x5D-7E / %x80-FF
escape       = "\" ( %s"t" / %s"n" / %s"v" / %s"f" / %s"r"
                   / DQUOTE / "\" / byte-esc / uni-esc )
byte-esc     = %s"x" 2HEXDIG
uni-esc      = %s"u" "{" 1*6HEXDIG "}"

; ---- type annotations ------------------------------------------

type-ann     = "<" ws type-spec ws ">"
type-spec    = family [ ws ":" ws param-list ]
family       = %s"uint" / %s"sint" / %s"float_fix" / %s"float_dec"
             / %s"float" / %s"utf8" / %s"bool" / %s"datetime"
param-list   = param *( ws "," ws param )
param        = width-param / base-param / q-param
             / epoch-param / unit-param
width-param  = 1*DIGIT
base-param   = "_" 1*DIGIT
q-param      = %s"q" 1*DIGIT
epoch-param  = %s"unix" / %s"tai" / %s"gps" / %s"mjd" / %s"ntp"
             / %s"galileo" / %s"glonass" / %s"y2000" / %s"beidou"

; unit-param and epoch-param overlap syntactically; they are
; disambiguated by the type family (see Section 7.3).
unit-param   = 1*unit-char
unit-char    = ALPHA / DIGIT / "_" / "$" / "%" / "+" / "-" / "."
             / "/" / ":" / "^" / "*" / "~" / "(" / ")" / "'"
             / "[" / "]" / "{" / "}" / unit-lead / utf8-cont

; ---- inline unit suffix ----------------------------------------

inline-unit  = inline-start *inline-char
inline-start = ALPHA / "_" / "$" / "%" / "(" / unit-lead
inline-char  = unit-char

; ---- arrays and structs ----------------------------------------

array        = array-row *( ws "/" ws array-row )
array-row    = "[" ws [ row-content ] ws "]"
row-content  = filled-elem
             / array-elem 1*( ws "," ws array-elem )
filled-elem  = type-ann ws [ raw-value ] / raw-value
array-elem   = [ type-ann ws ] [ raw-value ]

struct       = "{" ws *( assignment ws ) "}"

; ---- octet streams ---------------------------------------------

octet-stream = %x00 *os-chunk %x00
os-chunk     = %x01 os-length os-data
os-length    = 2OCTET      ; little-endian uint16; 0x0000 = 65536
os-data      = *OCTET       ; exactly os-length octets
~~~

The unit expression accumulated by `unit-param` and `inline-unit` is
parsed by a second grammar. {{units}} introduces it; this is its complete
form, including the prefix alternatives that the excerpt there omits.

`base-unit` and `currency-code` are the only rules the grammar does not
fully determine. Their productions below bound the *shape* of a token, so
that the grammar stays closed and mechanically checkable; whether a token
so shaped names anything is a lookup into the registry {{BOVNAR-UNITS}},
whose contents may grow in a minor revision ({{unit-model}}). A token
that satisfies the production and is absent from the registry is
`error_unit_illegal`, which is the ordinary case of the ABNF being
necessary and not sufficient ({{grammar-notation}}). The prefix sets, by
contrast, are closed and are enumerated here in full.

~~~ abnf
; resolved-unit is the entry point of the UNIT sub-grammar: the text a
; unit-param or an inline-unit carries must parse as one of these. It is
; deliberately not reachable from "stream", because at the document level
; a unit is an opaque run of unit-char and is resolved afterwards.

resolved-unit = %s"no_unit" / unit-expr
unit-expr     = unit-factor *( unit-sep unit-factor )
unit-factor   = unit-component / "(" unit-expr ")"
unit-sep      = "*" / "/" / %xC2.B7
unit-component = [ prefix [ "~" ] ] base-unit [ unit-exp ]
              / [ prefix [ "~" ] ] "$" currency-code [ unit-exp ]

; base-unit and currency-code bound the SHAPE of a token only; whether a
; token so shaped names anything is a registry lookup, per the paragraph
; above this block.
;
; base-unit is matched as the LONGEST alias suffix, so a token
; that is itself a registered unit outranks any prefixed reading
; of it. Section 8.3 lists the three spellings refused by name.

base-unit      = 1*base-unit-char
base-unit-char = ALPHA / DIGIT / "_" / "%"
               / %xC2.B0                    ; U+00B0 DEGREE SIGN
               / %xCE.94                    ; U+0394 GREEK CAPITAL DELTA
               / %xC3.85 / %xCC.8A          ; U+00C5, U+030A (angstrom)
               / %xCE.A9 / %xE2.84.A6       ; U+03A9, U+2126 (ohm)
               / %xE2.84.A7                 ; U+2127 MHO
               / %xE2.84.AB                 ; U+212B ANGSTROM SIGN
               / %xE2.80.B0 / %xE2.80.B1    ; U+2030, U+2031 (per mille)
currency-code  = 3*4( %x41-5A )             ; uppercase ASCII, registry lookup

; DIGIT may occur inside a symbol (mH2O) and at its end (cal_15,
; Btu_60). A trailing digit is never an exponent: an exponent needs
; "^" or a superscript, so "m2" is not a unit.

prefix        = si-prefix / iec-prefix
si-prefix     = "Q" / "R" / "Y" / "Z" / "E" / "P" / "T" / "G" / "M"
              / "k" / "h" / "da" / "d" / "c" / "m" / micro
              / "n" / "p" / "f" / "a" / "z" / "y" / "r" / "q"
micro         = %xC2.B5 / "u"   ; U+00B5, or its ASCII alias
iec-prefix    = "Ki" / "Mi" / "Gi" / "Ti" / "Pi" / "Ei" / "Zi" / "Yi"
              / "Ri" / "Qi"

; A currency component takes an si-prefix only; an iec-prefix on
; a currency is error_unit_illegal (Section 8.4).

unit-exp      = caret-exp / sup-exp
caret-exp     = "^" [ "+" / "-" ] 1*3DIGIT
sup-exp       = [ sup-sign ] 1*3sup-digit
sup-sign      = %xE2.81.BA / %xE2.81.BB     ; U+207A, U+207B
sup-digit     = %xE2.81.B0                  ; U+2070 SUPERSCRIPT ZERO
              / %xC2.B9 / %xC2.B2 / %xC2.B3 ; U+00B9, U+00B2, U+00B3
              / %xE2.81.B4 / %xE2.81.B5 / %xE2.81.B6
              / %xE2.81.B7 / %xE2.81.B8 / %xE2.81.B9  ; U+2074-U+2079

; The grammar bounds the SHAPE; the VALUE is bounded by Section 8.2.
; An exponent is an integer in [-100, 100] with zero reserved, so of
; the three-digit forms only 100 and -100 are units.
~~~

# Error Codes {#error-codes}

Every violation this document specifies is reported with one of the codes
below. The numeric values are stable: an existing value never changes,
and a new code is appended above the current maximum ({{version-stability}}).

The class column distinguishes eight kinds of code, only two of which say
the document is invalid:

D:
: **Document.** The document is invalid. Every conforming consumer
  rejects it.

M:
: **Materializing tier.** The document is invalid, but the rule requires
  comparing sibling values, so only a materializing consumer detects it
  ({{conformance}}). A streaming consumer accepts such a document.

I:
: **Interface.** A failure of the surrounding I/O, callback, or argument
  handling, not a property of the document. A consumer that does not
  expose the corresponding interface never reports these.

P:
: **Producer interface.** Raised by an interface for *writing* documents
  when it is handed something it cannot emit as a valid document. A
  consumer never raises these: no document provokes them.

R:
: **Reserved.** Assigned but never raised. They exist so that the
  numbering stays stable; a consumer MUST NOT raise them and MUST NOT
  reuse their values.

X:
: **Extension.** Raised by a facility layered on the wire format - opt-in
  read-time unit conversion, and the octet multiplexing convention -
  rather than by parsing. Neither facility is specified by this document.

C:
: **Consumer policy.** The document is valid; the consumer has declined it
  under a policy it was configured with. Not a defect in the document.

U:
: **Unit profile.** Raised by the unit-profile facility of format version
  1.2, whose boundary this document fixes and whose content it does not
  specify ({{unit-profiles}}).

| Value | Name | Class |
|---|---|---|
| 0 | `error_none` | - |
| 1 | `error_unknown_token_type` | P |
| 2 | `error_array_row_size_mismatch` | D |
| 3 | `error_identifier_too_long` | D |
| 4 | `error_empty_identifier` | D |
| 5 | `error_struct_nesting_too_high` | D |
| 6 | `error_array_nesting_too_high` | D |
| 7 | `error_illegal_struct_close` | D |
| 8 | `error_string_too_long` | D |
| 9 | `error_illegal_escape_sequence` | D |
| 10 | `error_number_too_long` | D |
| 11 | `error_symbol_too_long` | D |
| 12 | `error_reference_too_long` | D |
| 13 | `error_read_complete_chunk_failed` | D |
| 14 | `error_octet_stream_out_of_sync` | D |
| 15 | `error_unexpected_input_byte` | D |
| 16 | `error_text_data_too_long` | D |
| 17 | `error_reading_from_source_fd` | I |
| 18 | `error_got_incomplete_bvnr_stream` | D |
| 19 | `error_invalid_utf8_byte` | D |
| 20 | `error_invalid_byte_order_mark` | D |
| 21 | `error_type_too_long` | D |
| 22 | `error_unit_too_long` | D |
| 23 | `error_expected_string_in_array` | R |
| 24 | `error_expected_number_in_array` | R |
| 25 | `error_illegal_value_type` | D |
| 26 | `error_scanner_callback_failed` | I |
| 27 | `error_file_too_long` | D |
| 28 | `error_invalid_argument` | I |
| 29 | `error_too_many_array_items` | D |
| 30 | `error_writing_to_sink` | I |
| 31 | `error_sink_buffer_exhausted` | I |
| 32 | `error_unit_illegal` | D |
| 33 | `error_base_requires_string_literal` | P |
| 34 | `error_type_value_mismatch` | D |
| 35 | `error_value_out_of_range` | D |
| 36 | `error_digit_not_in_base` | D |
| 37 | `error_recovered` | R |
| 38 | `error_unit_mismatch` | D |
| 39 | `error_array_element_type_mismatch` | M |
| 40 | `error_struct_shape_mismatch` | M |
| 41 | `error_duplicate_struct_key` | M |
| 42 | `error_invalid_spec_version` | D |
| 43 | `error_unsupported_spec_version` | C |
| 44 | `error_invalid_codepoint` | D |
| 45 | `error_invalid_datetime_literal` | D |
| 46 | `error_datetime_literal_unsupported_epoch` | D |
| 47 | `error_unit_inexact` | X |
| 48 | `error_octet_stream_truncated` | X |
| 49 | `error_unit_profile_unknown` | U |
| 50 | `error_unit_profile_unsupported` | U |
| 51 | `error_octet_stream_forbidden` | C |
| 52 | `error_type_param_whitespace` | D |
{: title="Error codes"}

Five entries warrant a note.

`error_unknown_token_type` (1) is raised by a producer interface handed
an event that cannot occur at that point of a document.

`error_base_requires_string_literal` (33) is raised by a producer
interface asked to write, bare, a value whose digits the `number` rule
cannot carry - `ff` under base 16. A consumer never raises it: a bare
literal in a non-decimal base is either read in that base or lexes as a
symbol and is `error_type_value_mismatch` ({{numbers}}).

`error_got_incomplete_bvnr_stream` (18) is the one code a consumer may
report *in addition to* another, rather than instead of it: reaching end
of input while recovering from an earlier error yields the original code
first and this one second ({{error-recovery}}).

`error_octet_stream_forbidden` (51) is not the only code here that a
valid document can provoke, but it is the only one defined for nothing
else. The others are `error_unsupported_spec_version` (43), raised only
in the strict mode of {{version}}; `error_unit_profile_unknown` (49) on a
consumer lacking a namespace ({{unit-profiles}}); and the codes of the
configurable limits ({{limits}}) on a consumer configured more tightly
than the producer assumed. Code 51 is what a consumer opened in the text-only mode
of {{interop-transport}} reports on meeting an octet stream: the document
is well-formed and the octet stream is a first-class part of the format,
but this consumer has asserted that its channel carries text. Reporting
it is how a consumer whose pipeline normalizes line endings declines the
document at the door instead of discovering the damage downstream.

`error_type_param_whitespace` (52) is raised for whitespace inside a
type-annotation parameter. The grammar of {{collected-abnf}} places every
`ws` in `type-ann` beside a separator - after `family`, either side of the
`:` that introduces `param-list` or of a `,` between parameters, and
before the closing `>` - and derives none inside a `param`. A consumer
that skips whitespace uniformly within an annotation instead of at those
positions silently concatenates a split parameter, which yields a
different type or a different unit rather than an error: `<uint:6 4>`
becomes a 64-bit width, `<float:64,k g>` becomes the kilogram, and the
space-multiplied UDUNITS spelling `m s-1` becomes `ms-1`, a reciprocal
millisecond. Reporting code 52 at the first octet after the whitespace is
therefore a requirement of this document and not a lexical nicety.

The unit-profile facility ({{unit-profiles}}), which this document does not
specify, admits whitespace inside a parameter that carries a profile
namespace, where it is part of the foreign code rather than a separator. A
parameter carrying a namespace prefix is therefore never code 52 on account
of a space or a tab (a line break inside it still is): in a
document declaring 1.2 or newer it is the foreign code, or
`error_unit_profile_unknown` (49) for a namespace the consumer does not
implement; in a document declaring less it is `error_unit_illegal`,
whatever whitespace it contains.


# Examples {#examples}

## Configuration

~~~
# Application configuration
.app_name        = "Bovnar Demo";
.version         = 1;
.debug           = false;
.max_connections = <uint:16> 100;
.timeout         = <float:64,s> 30.0;
~~~

## Instrument Record

~~~
#!bovnar 1.1

.station = {
    .id        = "WX-114";
    .position  = {
        .lat = <float:64,deg>   47.3769;
        .lon = <float:64,deg>    8.5417;
        .alt = <float:64,m>    408.0;
    };
};

.sample = {
    .taken_at    = <datetime:64,unix> 2026-06-15T12:00:00Z;
    .temperature = <float:64,degC>      21.4;
    .pressure    = <float_dec:64,Pa> 101325;
    .wind_speed  = <float:64,m/s>        3.2;
    .humidity    = 47.5 %;
    .payload_len = <uint:32,Ki~B>       12;
};
~~~

## Unit Safety

~~~
.ok_1 = <float:64,m/s> 9.81;        # unit in the annotation
.ok_2 = <float:64>     9.81 m/s;    # unit in the suffix
.ok_3 = <float:64,m/s> 9.81 m/s;    # both, agreeing

# .no = <float:64,m/s> 9.81 s;      # error_unit_mismatch
# .no = <float:64,zz>  1.0;         # error_unit_illegal
~~~

## Compound Units

~~~
.force        = <float:64,k~g*m/s^2>  9.81;
.energy       = <float:64,k~g*m^2/s^2> 1000;
.density      = <float:64,k~g/m^3>    7800;
.pressure     = <float:64,k~g/(m*s^2)> 101325;
.moment       = <float:64,m*s>           1.0;
.three_factor = <float:64,k~g*m*s^-2>    9.81;
~~~

The last line is the same unit as the first: `k~g·m·s⁻²` is `k~g·m/s²`.
`.pressure` shows a grouped denominator, `k~g/(m·s²)`, which resolves
identically to `k~g/m·s²`.

## Arrays {#example-arrays}

~~~
.matrix    = [1, 2, 3]/[4, 5, 6];        # one 2x3 block
.nested    = [[1, 2], [3, 4]];           # rectangular
.ports     = <uint:16> [80, 443, 8080];  # whole-array annotation
.sparse    = [1, , 3];                   # null hole
.ledger    = [{ .cur = USD; .bal = <float_dec:64,$USD> 1.00; },
              { .cur = EUR; .bal = <float_dec:64,$EUR> 2.00; }];

# .bad = [1, "two"];           # error_array_element_type_mismatch
# .bad = [<float:64,m> 1.0,
#         <float:64,k~g> 2.0]; # mixed dimension: same error
# .bad = [[1, 2], [3, 4, 5]];  # error_array_row_size_mismatch
~~~

## References {#example-references}

~~~
#!bovnar 1.1
.config  = { .host = "example.com"; };
.primary = &.config.host;        # stored as ".config.host"
.matrix  = [10, 20, 30]/[40, 50, 60];
.cell    = &.matrix[0][1];       # stored verbatim; denotes 20
~~~

# Design Rationale {#rationale}

This appendix records why several of the format's more surprising
decisions are as they are. It is informative.

**Why the unit is inside the value rather than beside it.** A unit
carried in a sibling field, an attribute, or a naming convention can be
dropped by any transformation that does not know it is significant -
a projection, a serialization round-trip through a dimensionless format,
a schema migration. Carrying it inside the value's own annotation makes
dropping it a change to the value.

**Why an inline suffix exists at all, given annotations.** The suffix is
what makes a hand-written document readable: `.speed = 9.81 m/s;` reads
as a measurement, where `.speed = <float:64,m/s> 9.81;` reads as a
declaration. Permitting both raises the possibility of disagreement, and
turning that disagreement into `error_unit_mismatch` is what converts a
redundancy into a check.

**Why inline suffixes are forbidden inside arrays.** Allowing them would
let each element declare a different unit, defeating {{array-homogeneity}}
element by element, and would make the array's dimension a property to be
computed rather than declared. A whole-array annotation says it once.

**Why the `$` sigil is mandatory.** Currency codes and unit symbols
occupy the same lexical space, and the registry already holds the
near-misses: `BTU` is a real unit alias that a currency registry could
plausibly want as a ticker, and `cup` versus `$CUP` would otherwise be
separated by letter case alone ({{currency}}). Any scheme that adjudicates
such collisions one at a time must be revisited every time either
registry grows - and both grow. The sigil makes the namespaces disjoint
once and for all, and the cost is one character.

**Why money synthesises `float_dec`.** A binary float cannot represent
`0.10` exactly, and monetary values are decimal by definition. A number
carrying a currency unit is money, so the format infers the decimal
family rather than requiring every producer to remember to ask for it.

**Why homogeneity was tightened before 1.0.** Ragged and mixed-type
arrays were permitted in pre-1.0 drafts. Allowing them meant a consumer
could not treat an array's elements uniformly without inspecting every
one, which is the property that makes an array useful for measurements.
Structs model heterogeneous data. Because the change could invalidate a
previously valid document, it had to be made before the grammar froze,
and it cannot be revisited within a major version.

**Why octet-stream chunks are length-prefixed rather than delimited.**
Delimiting requires escaping, escaping expands the payload by an amount
that depends on its content, and a delimited region cannot be skipped
without being scanned. Length prefixing gives constant-factor
transparency and skippability. The price is fragility under transports
that rewrite octets, which {{security-transport}} treats as the design
tradeoff it is.

**Why `nan` is accepted on an integer channel.** Instrument data has
missing and saturated readings, and the channel's declared width is a
property of the encoding rather than of the measurement. Rejecting
`<uint:8> nan` would force producers to either widen every integer
channel to a float or invent a sentinel, and a sentinel is a unit error
waiting to happen.

**Why the version declaration is a comment.** Any other syntax would have
made a version-declaring document unreadable to every consumer that
predated the declaration - which is to say, the declaration would break
exactly the compatibility it exists to manage. Making it lexically a
comment means an older consumer skips it and reads the rest, which is
correct precisely when the document uses no newer construct.

# Acknowledgments
{:numbered="false"}

The unit registry draws on the SI Brochure {{SI}}, {{IEC80000-13}}, and
{{ISO4217}}. The unit profile facility referenced in {{unit-profiles}}
draws on {{UCUM}}.
