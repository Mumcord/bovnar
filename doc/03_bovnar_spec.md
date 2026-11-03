# Bovnar — Specification

> **Spec version:** 1.2
> **Status:** Normative — released (1.x line; additive over the frozen 1.0 baseline)
> **Scope:** Grammar, lexical structure, type system, units, limits, and error semantics.

---

## Table of Contents

1. [Overview](#1-overview)
    - 1.1 [Design Goals](#11-design-goals)
    - 1.2 [Architecture](#12-architecture)
    - 1.3 [Stream Model](#13-stream-model)
2. [File Format at a Glance](#2-file-format-at-a-glance)
    - 2.1 [Key Syntax Rules](#21-key-syntax-rules)
3. [Character Encoding & BOM](#3-character-encoding--bom)
    - 3.1 [UTF-8](#31-utf-8)
    - 3.2 [Byte Order Mark (BOM)](#32-byte-order-mark-bom)
    - 3.3 [Byte Classes](#33-byte-classes)
    - 3.4 [Version Directive (spec 1.1)](#34-version-directive-spec-11)
4. [Lexical Structure](#4-lexical-structure)
    - 4.1 [Whitespace & Comments](#41-whitespace--comments)
    - 4.2 [Identifiers (Keys)](#42-identifiers-keys)
    - 4.3 [String Literals](#43-string-literals)
    - 4.4 [Symbols](#44-symbols)
    - 4.5 [References](#45-references)
    - 4.6 [Numbers](#46-numbers)
    - 4.7 [Null Values](#47-null-values)
5. [Type Annotations](#5-type-annotations)
    - 5.1 [Syntax](#51-syntax)
    - 5.2 [Parameters](#52-parameters)
    - 5.3 [Parameter Order](#53-parameter-order)
    - 5.4 [Examples](#54-examples)
    - 5.5 [Non-decimal Base with Bare Numbers](#55-non-decimal-base-with-bare-numbers)
6. [Value Tokens](#6-value-tokens)
    - 6.1 [Type/Value Compatibility](#61-typevalue-compatibility)
    - 6.2 [Validation Rules per Numeric Type](#62-validation-rules-per-numeric-type)
    - 6.3 [Digit Validation](#63-digit-validation)
    - 6.4 [Special Number Semantics](#64-special-number-semantics)
    - 6.5 [Inline Unit Suffix](#65-inline-unit-suffix)
7. [Arrays](#7-arrays)
    - 7.1 [Row Syntax](#71-row-syntax)
    - 7.2 [Null Elements](#72-null-elements)
    - 7.3 [Row-Size Consistency](#73-row-size-consistency)
    - 7.4 [Element Homogeneity](#74-element-homogeneity)
    - 7.5 [Array Elements with Type Annotations](#75-array-elements-with-type-annotations)
    - 7.6 [Constraints](#76-constraints)
8. [Structs (Scopes)](#8-structs-scopes)
    - 8.1 [Syntax](#81-syntax)
    - 8.2 [Nesting](#82-nesting)
    - 8.3 [Empty Structs](#83-empty-structs)
    - 8.4 [Structs as Array Elements](#84-structs-as-array-elements)
    - 8.5 [Unmatched Braces](#85-unmatched-braces)
9. [Octet Streams (Binary Mode)](#9-octet-streams-binary-mode)
    - 9.1 [Overview](#91-overview)
    - 9.2 [Wire Protocol](#92-wire-protocol)
    - 9.3 [Events](#93-events)
    - 9.4 [Example](#94-example)
    - 9.5 [Constraints](#95-constraints)
10. [Default Type Synthesis](#10-default-type-synthesis)
    - 10.1 [Rules](#101-rules)
    - 10.2 [Event Sequence](#102-event-sequence)
    - 10.3 [Examples](#103-examples)
11. [Units System](#11-units-system)
    - 11.1 [Base Units](#111-base-units)
    - 11.2 [SI Prefixes](#112-si-prefixes)
    - 11.3 [IEC Binary Prefixes](#113-iec-binary-prefixes)
    - 11.4 [Unit Notation](#114-unit-notation)
    - 11.5 [Unit Exponents](#115-unit-exponents)
    - 11.6 [Examples](#116-examples)
    - 11.7 [Compound Unit Constraints](#117-compound-unit-constraints)
    - 11.8 [The `no_unit` Keyword](#118-the-no_unit-keyword)
    - 11.9 [Unit Profiles (spec 1.2)](#119-unit-profiles-spec-12)
12. [Validation & Constraints](#12-validation--constraints)
    - 12.1 [UTF-8 Validation](#121-utf-8-validation)
    - 12.2 [Size Limits](#122-size-limits)
    - 12.3 [Value Validation](#123-value-validation)
    - 12.4 [Array Validation](#124-array-validation)
    - 12.5 [Struct Validation](#125-struct-validation)
    - 12.6 [Identifier Validation](#126-identifier-validation)
    - 12.7 [String Validation](#127-string-validation)
    - 12.8 [Unit Validation](#128-unit-validation)
    - 12.9 [Octet Stream Validation](#129-octet-stream-validation)
13. [Error Handling & Recovery](#13-error-handling--recovery)
    - 13.1 [Error Model](#131-error-model)
    - 13.2 [Recovery Mode](#132-recovery-mode)
    - 13.3 [EOF in Resync](#133-eof-in-resync)
    - 13.4 [The Error Getters Under Recovery](#134-the-error-getters-under-recovery)
14. [Formal EBNF](#14-formal-ebnf)
15. [Complete Examples](#15-complete-examples)
    - 15.1 [Simple Configuration](#151-simple-configuration)
    - 15.2 [Typed Scientific Data](#152-typed-scientific-data)
    - 15.3 [Inline Unit Suffix](#153-inline-unit-suffix)
    - 15.4 [Binary Data with Octet Stream](#154-binary-data-with-octet-stream)
    - 15.5 [Arrays with Mixed Dimensions](#155-arrays-with-mixed-dimensions)
    - 15.6 [Deeply Nested Struct](#156-deeply-nested-struct)
    - 15.7 [References and Symbols](#157-references-and-symbols)
    - 15.8 [Compound Unit Examples](#158-compound-unit-examples)
    - 15.9 [Fixed-Point and Decimal Float Examples](#159-fixed-point-and-decimal-float-examples)
    - 15.10 [Error Examples](#1510-error-examples)
16. [Reference API](#16-reference-api)
    - 16.1 [Core Types](#161-core-types)
    - 16.2 [Type Construction Macros](#162-type-construction-macros)
    - 16.3 [Unit Macros](#163-unit-macros)
    - 16.4 [Reader Setup](#164-reader-setup)
    - 16.5 [Source/Sink Creation](#165-sourcesink-creation)
    - 16.6 [Reading](#166-reading)
    - 16.7 [Error Queries](#167-error-queries)
    - 16.8 [Utility Functions](#168-utility-functions)
    - 16.9 [Typed Write Helpers](#169-typed-write-helpers)
    - 16.10 [Error Codes](#1610-error-codes)
17. [Versioning & Stability](#17-versioning--stability)
18. [Security Considerations](#18-security-considerations)
    - 18.1 [Resource Exhaustion](#181-resource-exhaustion)
    - 18.2 [Recovery Mode Discards Data Silently](#182-recovery-mode-discards-data-silently)
    - 18.3 [Version Leniency](#183-version-leniency)
    - 18.4 [Transport Corruption of Octet Streams](#184-transport-corruption-of-octet-streams)
    - 18.5 [Reference Resolution](#185-reference-resolution)
    - 18.6 [Numeric Interpretation](#186-numeric-interpretation)
    - 18.7 [Units Are Validated, Not Verified](#187-units-are-validated-not-verified)
    - 18.8 [Leap-Second Table Drift](#188-leap-second-table-drift)
    - 18.9 [Confusable and Non-Normalized Keys](#189-confusable-and-non-normalized-keys)
    - 18.10 [No Confidentiality, Integrity, or Authenticity](#1810-no-confidentiality-integrity-or-authenticity)
- [Appendix A: Event Sequence Reference](#appendix-a-event-sequence-reference)
    - A.1 [Simple Assignment (Untyped)](#a1-simple-assignment-untyped)
    - A.2 [Typed Assignment](#a2-typed-assignment)
    - A.3 [Compound Unit Assignment](#a3-compound-unit-assignment)
    - A.4 [Array](#a4-array)
    - A.5 [Struct](#a5-struct)
    - A.6 [Octet Stream](#a6-octet-stream)
- [Appendix B: Implementation Notes](#appendix-b-implementation-notes)
    - B.1 [Keyword State Machine](#b1-keyword-state-machine)
    - B.2 [Special Number Keywords](#b2-special-number-keywords)
    - B.3 [Default Width, Base, and Q](#b3-default-width-base-and-q)
    - B.4 [Type Equality](#b4-type-equality)
    - B.5 [Numeric Type Check](#b5-numeric-type-check)
    - B.6 [Unit Component Access](#b6-unit-component-access)
    - B.7 [Fixed-point and Decimal Float Wire Representations](#b7-fixed-point-and-decimal-float-wire-representations)
- [Appendix C: Limits Summary](#appendix-c-limits-summary)
- [See also](#see-also)

---

## 1. Overview

**Bovnar** (BVNR) is **unit-safe serialization for scientific and industrial systems**: a **typed, self-describing, text-binary hybrid** format that carries a validated physical unit with every value. It combines a human-readable text layer with an efficient binary octet-stream escape mechanism and a rich, unit-aware type annotation system.

### 1.1 Design Goals

- **Unit-safe** – every value may carry a physical unit that is validated against a built-in unit table; a unit that conflicts with the type annotation is a parse error (`error_unit_mismatch`)
- **Self-describing** – values carry explicit type metadata (family, bit-width, base, measurement unit)
- **Human-readable** for common cases (numbers, strings, symbols, nested structures)
- **Binary-friendly** via octet stream escape for opaque payloads
- **Schema-free** but type-safe – the type annotation is per-value, optional, and validated
- **Streamable** – parsed incrementally via a pull-based reader; events emitted to callbacks
- **Error-recoverable** – optional resync mode for parsing through minor corruption

### 1.2 Architecture

The reference implementation is a **two-phase** parser:

| Phase | Callback | Purpose |
|-------|----------|---------|
| **Unverified** (`on_unverified`) | Raw token events before semantic validation | Inspection, logging, partial consumption |
| **Verified** (`on_verified`) | Fully validated events | Actual data consumption |

Both phases receive the same stream of events (`bvnr_event_t`). The validator sits between them.

### 1.3 Stream Model

```
stream_begin →
  (assignment)* →
  EOF
```

Each assignment is:

```
.identifier = value;
```

---

## 2. File Format at a Glance

```bovnar
# This is a comment

.my_first_value = 42;
.my_next_value = 3.14159;
.a_string = "hello world";
.a_typed_value = <uint:32> 1000;
.a_utf8_value = <utf8> "text";
.an_array = [1, 2, 3]/[4, 5, 6];
.a_struct = {
    .nested = true;
};
```

### 2.1 Key Syntax Rules

| Construct | Syntax | Example |
|-----------|--------|---------|
| Comment | `#` … newline | `# this is a comment` |
| Assignment | `.key = value ;` | `.foo = 42;` |
| Type annotation | `<type-spec>` placed after `=`, before the value | `.foo = <uint:32> 42;` |
| Number | `[-]digits[.[digits]][e[+/-]digits]`, or `[-].digits[…]`; `e` or `E` (§4.6) | `42`, `-3.14`, `1e6`, `.5` |
| Special number | `nan`, `inf`, `ninf` | `.x = inf;` |
| Boolean | `true` / `false` / `on` / `off` | `.b = true;`, `.b = off;` |
| String | `"…"` with escapes (`\u{…}`/`\xHH` in spec 1.1) | `.s = "hello\nworld";`, `.s = "caf\u{e9}";` |
| Symbol | bare identifier (no quotes) | `.s = ok;`, `.day = Monday;` |
| Reference | `&.path.to.key`; array indexing `&.path[i][j]` (spec 1.1) | `.ref = &.config.host;`, `.c = &.m[0][1];` |
| Array | `[ … ]` rows separated by `/` | `.a = [1,2,3]/[4,5,6];` |
| Struct | `{ … }` | `.s = {.x = 1; .y = 2;};` |
| Octet stream | `\x00` … binary … `\x00` | binary escape |
| Null value | absent token or `null` keyword | `.x = ;`, `.x = null;`, `[,1,]` |
| Version directive (spec 1.1) | `#!bovnar <major>.<minor>` as the first comment | `#!bovnar 1.1` |
| Datetime (spec 1.1) | `<datetime:width,epoch>` — signed epoch seconds | `.t = <datetime:64,unix> 1750000000;` |
| Unit (type annotation) | inside `<…>` as a type param | `<float:64,m/s>` |
| Unit (inline suffix) | after scalar value, before `;` | `.speed = 9.81 m/s;` |
| Fixed-point type | `<float_fix:[width,][qN][,unit]>` (Q defaults to 0) | `<float_fix:32,q16>`, `<float_fix:q8>` |
| Decimal float type | `<float_dec:[width][,unit]>` | `<float_dec:64,Pa>` |
| Unit (compound) | `unit[*·/unit…]` | `m/s²`, `k~g·m/s²`, `m*s` |

---

## 3. Character Encoding & BOM

### 3.1 UTF-8

All text-layer bytes (outside octet-stream regions) must form **valid UTF-8**. The parser runs a parallel UTF-8 validator (`bvn_utf8_feed`) alongside the state machine. Violations produce `error_invalid_utf8_byte`.

Overlong sequences and surrogate halves (U+D800–U+DFFF) are rejected.

### 3.2 Byte Order Mark (BOM)

The UTF-8 BOM `EF BB BF` is only legal **at byte offset 0** of a stream.

A BOM anywhere else is an error or is ordinary content, depending on whether the first assignment has begun.

**Before the first assignment:**

- A BOM (`EF BB BF`) **inside any comment** — the first comment line, a version directive line, or a later comment line — produces `error_invalid_byte_order_mark`.
- A BOM **after leading whitespace**, with no comment line before it, produces `error_invalid_byte_order_mark`.
- A BOM **at the start of a line that follows a comment line or a version directive** produces `error_unexpected_input_byte`, as does a second BOM directly after the one at offset 0.

**Once the first assignment has begun:**

- `EF BB BF` inside a comment or a string is **valid UTF-8** and is accepted as content.
- `EF BB BF` between assignments produces `error_unexpected_input_byte`.
- `EF BB BF` *inside* an assignment is none of these: it is the three bytes of
  U+FEFF, and U+FEFF is a character a key or a symbol may contain (§4.2, §4.4).
  `.foo<U+FEFF> = 1;` assigns to a four-character key, and `.foo =<U+FEFF>1;`
  assigns the **symbol** `<U+FEFF>1`, not the number 1. Where a key or a symbol
  cannot stand — directly after a number, or before a `{` member's `.` — it is
  `error_unexpected_input_byte`. §18.9 is about exactly this kind of character.

<!-- bovnar-example: illustrative -->
```bovnar
# BOM at byte offset 0: valid
EF BB BF.foo = 1;

# BOM bytes inside a comment before the first assignment → error_invalid_byte_order_mark
# comment text: # EF BB BF text

# BOM bytes after leading whitespace → error_invalid_byte_order_mark
   EF BB BF.foo = 1;

# BOM bytes at the start of the line after a comment line → error_unexpected_input_byte
# comment line
EF BB BF.foo = 1;
```

### 3.3 Byte Classes

| Class | Bytes | Usage |
|-------|-------|-------|
| Whitespace | `0x09` (HT), `0x0A` (LF), `0x0B` (VT), `0x0C` (FF), `0x0D` (CR), `0x20` (SP) | Token separators |
| Control (rejected) | `0x00–0x08`, `0x0E–0x1F`, `0x7F` (DEL) | Hard errors everywhere outside octet streams, strings and comments included (`0x00` where a value is expected opens an octet stream) |
| Safe ASCII | `0x20–0x7E` except reserved punctuation | Identifier/symbol/reference bodies |
| High bytes | `0x80–0xFF` | UTF-8 multi-byte sequences |

### 3.4 Version Directive (spec 1.1)

A document **may** declare the spec version it targets with a directive as its
very first comment — in practice its first line:

```bovnar
#!bovnar 1.1
.x = 42;
```

**An unversioned document is spec 1.0.** A document with no directive is treated
as targeting spec **1.0** — the frozen baseline grammar. The directive only ever
*opts in* to a newer version; its absence is never ambiguous. Consequently, any
**construct** introduced after 1.0 is available only to a document that declares
the version that introduced it, or a newer one. A 1.1 construct in an unversioned
(i.e. 1.0) document is invalid, exactly as it would be to a 1.0 reader, and so
is a 1.2 construct in a document declaring 1.1.

The gate is on constructs, and the constructs are a closed list: the `datetime`
family and its ISO-8601 literals, the `\x`/`\u{}` escapes and reference
indexing (1.1), and the unit-profile notation (1.2). The **unit registry is not
gated**. A unit, an alias, a currency code or an input spelling of a unit added
in a later release — the compact prefix form `kg` among them (§11.4) — is
accepted in every document whatever it declares, because it is only ever
accepted where the parser used to answer `error_unit_illegal` (§17). A
directive therefore says which constructs a document uses; it does not promise
that a reader of that version knows every unit in it.
`bvnr_reader_get_declared_version` returns `false` for an unversioned document
(there is nothing declared), and a consumer that wants a concrete number should
default to 1.0.

**Form.** `#` `!bovnar` followed by one or more spaces/tabs, a `<major>.<minor>`
decimal version, optional trailing spaces/tabs, then end-of-line (LF/CR) or EOF.
Each component is a decimal integer with no leading zero (a bare `0` excepted)
and must fit in 16 bits.

**Backward compatible by construction.** Lexically the directive *is* a comment
(`#…`), so a spec-1.0 reader skips it transparently — declaring a version never
breaks an older parser. This is the one place where a comment carries meaning;
everywhere else comments remain semantically inert.

**Recognition rules.**

- It is recognised only as the **very first comment**, after an optional BOM and
  leading whitespace (blank lines included). A `#!bovnar …` appearing after
  any other comment, or after any content, is an ordinary comment and is
  ignored.
- `#!bovnar` not followed by a space or a tab (e.g. `#!bovnarish`, or the bare
  word at the end of a line) is an ordinary comment, not a directive.
- A directive prefix followed by a malformed version (missing component,
  non-numeric, leading zero, a component that does not fit in 16 bits, or
  trailing junk) is `error_invalid_spec_version`.

**Enforcement.** A 1.1+ reader records the declared version and exposes it
(`bvnr_reader_get_declared_version`). By default a version the reader does not
support — a newer minor, or any other major, `0.x` included — is **accepted
leniently**: recorded, but not rejected up front; the parse only fails if it
later meets syntax the reader does not implement. The constructs available are
those of the declared version as far as the reader knows them, so a document
declaring `0.9` gets the 1.0 grammar and one declaring `2.0` everything this
reader has. A reader
opened with `strict_version` instead rejects any unsupported version
(`error_unsupported_spec_version`), and a malformed directive
(`error_invalid_spec_version`), at the directive. The refusal ends the read
even under `continue_on_error`: recovering from it would read the document
the consumer asked not to read. A reader supports spec
`<major>.<minor>` when `major` equals its own and `minor` is ≤ its own
(`BVNR_SPEC_VERSION_*`).

| Directive line | Outcome |
|---|---|
| `#!bovnar 1.2` | current — accepted |
| `#!bovnar 1.1` | older minor — accepted |
| `#!bovnar 1.0` | older minor — accepted |
| `#!bovnar 1.9` | newer minor — lenient: accepted; strict: error |
| `#!bovnar 2.0` | newer major — lenient: accepted; strict: error |
| `#!bovnar 0.9` | older major — lenient: accepted, with the 1.0 grammar; strict: error |
| `#!bovnar 1.65536` | component over 16 bits — `error_invalid_spec_version` |
| `#!bovnar 1` | malformed (no minor) — `error_invalid_spec_version` |
| `#!bovnar 1.1 # note` | **trailing junk** — `error_invalid_spec_version` |

The outcomes are given in a table rather than as comments on the directives
themselves, because a comment there is precisely the *trailing junk* the rule
above forbids: `#!bovnar 1.1 # current` is not an annotated directive, it is a
rejected one. The directive line carries the version and nothing else. Comments
resume on the next line, where they are ordinary and inert.

---

## 4. Lexical Structure

### 4.1 Whitespace & Comments

Whitespace and comments are freely interleaved between all tokens, with the one restriction on where a comment may *start* that is stated below.

**Whitespace characters:**

```
HT (0x09), LF (0x0A), VT (0x0B), FF (0x0C), CR (0x0D), SP (0x20)
```

**Comments:**

```
# any bytes except 0x00–0x08, 0x0E–0x1F, 0x7F
# terminated by LF or CR (or EOF)
```

```bovnar
# A full-line comment
.foo = 42;  # an inline comment
```

**`#` is not a terminator of a bare token.** A comment may not start immediately after a key, a number, a symbol, a reserved word, a reference or a datetime literal: there `#` is `error_unexpected_input_byte`, and at least one whitespace byte must come first. After a string, `]`, `}`, `>`, `=`, `;` or an inline unit, `#` may follow directly.

<!-- bovnar-example: rejected -->
```bovnar
.ok1 = 42 # whitespace first: valid
;
.ok2 = "text"# directly after a string: valid
;
.bad = 42# directly after a number: error_unexpected_input_byte
;
```

### 4.2 Identifiers (Keys)

Every assignment begins with `.` followed by an identifier (the key).

**Syntax:**

```
id-start  = A–Z | a–z | "_" | UTF-8 leader bytes 0xC3–0xF4
id-body   = id-start | "+" | "-" | DIGIT | UTF-8 continuation bytes
```

**Constraints:**

- At least one `id-start` character is required after `.` (`.=` is `error_empty_identifier`)
- Byte `0xC2` is rejected at identifier start **and body** positions (U+0080–U+00BF are forbidden in identifiers everywhere)
- The following ASCII punctuation characters are **hard errors** inside identifier bodies: `! " # $ % & ' ( ) * , . / : ; < > ? @ [ \ ] ^ ` { | } ~`. (`=` is not among them: it ends the key and begins the value, so `.foo=1;` is valid.)

**Valid identifiers:**

```bovnar
.foo = 1;
.FooBar = 2;
._private = 3;
.my-key = 4;           # hyphen allowed
.my+key = 5;           # plus allowed
.user_defined = 6;
```

**Invalid identifiers:**

<!-- bovnar-example: rejected -->
```bovnar
. = 1;                 # error_empty_identifier
.123 = 2;              # starts with digit
.foo,bar = 3;          # comma inside identifier – error
```

### 4.3 String Literals

**Syntax:**

```
string-literal = '"' { safe-byte | escape-seq } '"'
string         = string-literal { ws string-literal }   # concatenation
```

**Safe bytes:** `0x09–0x0D` (HT, LF, VT, FF, CR), `0x20–0x7E` except `"` (0x22) and `\` (0x5C), and `0x80–0xFF`. `0x7F` (DEL) is rejected even inside strings.

**Control bytes** `0x00–0x08`, `0x0E–0x1F`, and `0x7F` (DEL) are hard errors inside strings. The whitespace bytes HT (0x09), LF (0x0A), VT (0x0B), FF (0x0C), and CR (0x0D) are accepted as raw string content.

#### Escape Sequences

| Escape | Meaning | Byte |
|--------|---------|------|
| `\t` | Horizontal Tab | `0x09` |
| `\n` | Line Feed | `0x0A` |
| `\v` | Vertical Tab | `0x0B` |
| `\f` | Form Feed | `0x0C` |
| `\r` | Carriage Return | `0x0D` |
| `\"` | Double Quote | `0x22` |
| `\\` | Backslash | `0x5C` |

In a **spec 1.1** document (one declaring `#!bovnar 1.1` or newer — see §3.4) two
further escapes are available:

| Escape | Meaning |
|--------|---------|
| `\xHH` | the single byte `HH` (exactly two hex digits) |
| `\u{H…}` | the Unicode scalar value `U+H…` (1–6 hex digits), UTF-8 encoded |

- `\u{…}` rejects surrogates (`U+D800`–`U+DFFF`) and values above `U+10FFFF`
  with `error_invalid_codepoint`; a missing/empty/over-long brace group or a
  non-hex digit is `error_illegal_escape_sequence`.
- `\x` writes a raw byte, but a string's contents must still be valid UTF-8 (the
  `utf8` family guarantee). So `"\xC3\xA9"` is `"é"`, whereas a lone `"\xFF"` is
  `error_invalid_utf8_byte`. Use an octet stream for arbitrary, non-textual bytes.
- A `\xHH` or `\u{…}` escape that *resolves to* an ASCII control byte
  (`0x00`–`0x08`, `0x0E`–`0x1F`, `0x7F` — every control except the whitespace
  controls `0x09`–`0x0D`) is rejected with `error_unexpected_input_byte`, exactly
  as a raw control byte in a string is: the escape is no way around the
  control-byte rule.
- **Gating.** `\x` and `\u` are 1.1-only. In a 1.0 or unversioned document
  (§3.4) the `x`/`u` after a backslash is not a recognised escape and yields
  `error_illegal_escape_sequence`, exactly as a 1.0 reader reports.

Any byte other than `t`, `n`, `v`, `f`, `r`, `"`, `\` (plus `x`, `u` in 1.1)
after `\` causes `error_illegal_escape_sequence`.

**String concatenation:** Two or more adjacent string literals (separated only by whitespace/comments) are concatenated into a single token:

```bovnar
.long = "hello " "world";     # → "hello world"
```

The combined byte length must not exceed `max_string_length` (default 65535).

#### Examples

```bovnar
.simple = "hello";
.with_escapes = "tab:\there\nnewline";
.unicode = "café";            # UTF-8 bytes for é = 0xC3 0xA9
.empty = "";
```

### 4.4 Symbols

A **symbol** is an unquoted bare-word token that appears in value position. It starts with an `id-start` character and continues with `id-body` characters.

**Differences from identifier keys:**

| Context | `,` | `]` | `;` | `=` |
|---------|-----|-----|-----|-----|
| Identifier body | hard error | hard error | hard error | transitions to value |
| Symbol body | terminates → new array element | terminates → close array | terminates → end value | hard error |

```bovnar
.status = ok;              # symbol "ok"
.day = Monday;             # symbol "Monday"
.flags = [red, green, blue];  # symbols as array elements
```

**Reserved keywords.** Eight exact bare words are *not* symbols: `null`, `true`, `false`, `on`, `off`, `nan`, `inf`, and `ninf`. Lexically they are still symbol tokens, but the validator reserves the exact spellings and reclassifies them — `null` to a null value, `true`/`false`/`on`/`off` to a `bool` value (`on` ≡ `true`, `off` ≡ `false`), and the special floats `nan`/`inf`/`ninf` to numeric special values (§6.4). A bare boolean with no annotation synthesises a `<bool>` type (§10); an explicit `<bool>` annotation accepts only the four bool keywords. A longer word that merely begins with a keyword (`ontology`, `nullable`, `truthy`, `infinity`) remains an ordinary symbol.

```bovnar
.enabled = true;           # bool value (vt_bool), not a symbol
.debug   = off;            # bool false
.choice  = <bool> on;      # explicit; on == true
.label   = truthy;         # symbol "truthy" — not a keyword
```

### 4.5 References

A **reference** is a dotted path to another key, introduced by `&`.

**Syntax:**

```
reference     = "&" ref-segment { ref-segment | index }
ref-segment   = "." id-start { ref-body-char }
ref-body-char = same as symbol-body-char
index         = "[" digit { digit } "]"        (* spec 1.1 *)
```

A `.` never extends a segment: it starts the next one, which must begin with an `id-start` character. A trailing or doubled dot (`&.x.`, `&.x..y`) is `error_unexpected_input_byte`.

The stored text includes the leading dot and all intermediate dots:

```bovnar
.host = "localhost";
.port = 8080;

.connection = &.host;             # stored as ".host"
.full = &.config.host;            # stored as ".config.host"
```

**Examples:**

```bovnar
.config = { .host = "example.com"; };  # nested struct
.ref = &.config.host;                  # reference → ".config.host"
```

**Resolution.** A reference is stored **unresolved** — as the path string only.
The parser and library never dereference it, so:

- the target **need not exist** at parse time; a reference to a missing key
  (`&.nope`), a forward reference (`&.x` before `.x` is defined), or a reference
  to a value outside the document is accepted and stored verbatim;
- **cycles are not detected** (`.a = &.b; .b = &.a;` parses), and cannot hang the
  library, which never follows a reference (`bvn_dom_lookup` navigates literal
  structure only, stopping at a reference rather than dereferencing it);
- **resolution is entirely the application's responsibility**, including how to
  treat dangling paths and cycles.

A reference path is bounded by `max_reference_length` (`error_reference_too_long`
otherwise). In an array, references are homogeneous by **kind** — an array of
references is uniform regardless of what its targets resolve to (their target
types are unknown to the parser).

**Array indexing (spec 1.1).** A reference path may address array elements with
`[N]` index suffixes (`&.matrix[0][1]`). Like the rest of the path the index is
captured **verbatim and unresolved** at the byte layer — the library never
dereferences a reference, so `bvn_dom_lookup` on the *reference* node returns its
stored path string, not the target. The index is interpreted only when an
application itself resolves that path string against the tree by calling
`bvn_dom_lookup(doc, ".matrix[0][1]")` directly (this is the same path-walker the
CLI `query` command exposes — `bovnar query .matrix[0][1]` → the element).
The index syntax is a 1.1 feature: in a 1.0/unversioned document a `[` in a
reference is `error_unexpected_input_byte`, exactly as a 1.0 reader reports.

Resolution semantics follow the array model (§7): a flat `/`-row matrix
(`[10,20,30]/[40,50,60]`) is addressed as `[row][col]` — `&.matrix[0][1]` → `20`
— and a 1-D array as `[i]`; genuine nested arrays (`[[1,2],[3,4]]`) descend one
index per level. A partial index of a flat matrix (`&.matrix[0]`), an
out-of-range index, or indexing a non-array does not resolve (the application
sees no node).

```bovnar
#!bovnar 1.1
.matrix  = [10, 20, 30]/[40, 50, 60];
.row0c1  = &.matrix[0][1];   # resolves to 20
```

### 4.6 Numbers

#### Bare Number Literals

```
number = ["-"] ( int-led | dot-led ) [ dec-exponent ]

int-led       = DIGIT { DIGIT } [ "." { DIGIT } ]
dot-led       = "." DIGIT { DIGIT }
dec-exponent  = ("e" | "E") [ "+" | "-" ] DIGIT { DIGIT }
```

- Only `e`/`E` accepted as exponent marker in bare literals (base-16 float values in quoted string literals use `p`/`P` — see §6.3). Under a declared base of 15 or above the `e`/`E` of a bare literal is read as a digit, not as an exponent marker (§5.5)
- Leading zeros are valid (`007` is accepted)
- A trailing dot without fractional digits is valid (`123.`)
- `.` alone is a hard error

```bovnar
.i = 42;
.neg = -17;
.float = 3.14;
.trailing = 123.;
.dot_led = .5;
.sci = 1e6;
.neg_sci = -2.5e-3;
```

#### Special Number Literals

```
special-number = "nan" | "inf" | "ninf"
```

The special IEEE-754 values are **bare reserved keywords** — `nan`, `inf`, and `ninf` (negative infinity) — with no sigil. Like `null`/`true`/`false`/`on`/`off`, they are lexed as symbols and reclassified to numeric special values by the validator; a bare word that is not one of these exact spellings (e.g. `infinity`, `nans`) stays an ordinary symbol. The stored token text is the keyword itself: `nan`, `inf`, `ninf`. A special-number keyword takes **no inline unit suffix** — supply a unit through the type annotation instead (`<float:64,m/s> inf`).

```bovnar
.not_a_number = nan;
.infinite = inf;
.neg_infinite = ninf;
```

### 4.7 Null Values

A null value is the **absence** of a raw-value token, or the reserved keyword `null`. It occurs when:

- At assignment level: `.key = ;` (nothing between `=` and `;`), or `.key = null;`
- In array context: leading/trailing commas, or consecutive commas: `[,1,,2,]`; a bare `null` element is equivalent

```bovnar
.null = ;                    # null value
.also = null;                # the null keyword — identical to the empty slot
.items = [,1, ,2,];          # null, 1, null, 2, null
```

When a type annotation precedes a null value, the null carries the annotated type:

```bovnar
.null_typed = <uint:32> ;
.nulls_in_array = [<uint:32>, <sint:64> , ];
```

---

## 5. Type Annotations

### 5.1 Syntax

```
type-annotation = "<" ws type-spec ws ">"
```

(Whitespace is also allowed beside the `:` and the `,` inside it — §5.3 — which
the summary productions in this section leave out; `doc/12_bovnar.ebnf` carries
every `ws`.)

The type annotation **must** be placed in one of four positions:

1. Immediately after the `=` sign of an assignment, before the value: `.key = <uint:32> 42;`
2. Before the opening `[` of an array — a **whole-array annotation** that is inherited by every element that does not carry its own annotation: `.ports = <uint:16> [80, 443, 8080];`
3. After the opening `[` of an array, before the first element.
4. After a `,` inside an array, before the next element.

In all cases the annotation comes **before** the value it describes.

**A struct takes no annotation.** Its members carry their own, so an annotation
in force where a `{` opens — written before the struct, or inherited from the
array the struct is an element of — is `error_type_value_mismatch`:
`<utf8> {}`, `<uint:8> {.x = 1;}` and `<uint:8> [{.x = 1;}]` are all refused.

**Correct placement:**

```bovnar
.an_int  = <uint:32> 42;
.a_float = <float:64> 3.14;
.arr = [<uint:8> 1, <sint:16> -2];

# Whole-array annotation — applies to all elements that lack their own annotation
.ports = <uint:16> [80, 443, 8080];

# Per-element annotations override the whole-array annotation
.mixed = <uint:8> [1, <sint:8> -1, 255];
```

**Incorrect placement (hard error):**

<!-- bovnar-example: rejected -->
```bovnar
.key<uint:32> = 42;    # ERROR: type annotation must follow '=', not the key
```

Seven type families are recognized in spec 1.0, plus `datetime` in spec 1.1:

| Family | Keyword | Parameter syntax | Default Width |
|--------|---------|-----------------|---------------|
| Unsigned integer | `uint` | `:width,_base,unit` | 64 |
| Signed integer | `sint` | `:width,_base,unit` | 64 |
| Binary floating-point | `float` | `:width,_base,unit` (base `_10` or `_16` only) | 64 |
| Fixed-point (Q-format) | `float_fix` | `:width,qN,unit` | 64 |
| Decimal floating-point (IEEE 754-2008) | `float_dec` | `:width,unit` | 64 |
| UTF-8 string | `utf8` | none (any parameter → `error_illegal_value_type`) | — |
| Boolean | `bool` | none (any parameter → `error_illegal_value_type`) | — |
| Timestamp (spec 1.1) | `datetime` | `:width,epoch` (no numeric base/unit) | 64 |

**The `datetime` family (spec 1.1).** A `datetime` value is a **signed integer
count of seconds** since a named epoch — a timestamp, as distinct from a
*duration* (which is just a number with a time unit, e.g. `<float:64,s>`). The
carrier is validated exactly like `sint` (signed, decimal, range per width;
negative values denote instants before the epoch), and its width parameter takes
what `sint`'s does: `0` (the default, 64) to 32768. Its one family-specific
parameter is the **epoch name**, one of:

`unix` (default), `tai`, `gps`, `mjd`, `ntp`, `galileo`, `glonass`, `y2000`, `beidou`.

A numeric base, `q`, or physical unit parameter is `error_illegal_value_type`; a
fractional or exponent *numeric carrier* (e.g. `1.5` or `1e3` — as opposed to the
sub-second fraction of an ISO-8601 literal, covered below) is
`error_type_value_mismatch`, and so is a quoted string (`<datetime> "123"`) —
unlike `sint`, a datetime has no base to write digits in. As a 1.1 feature it
requires a `#!bovnar 1.1` declaration (§3.4) — in a 1.0/unversioned document
`datetime` is `error_illegal_value_type`. Recover the civil date/time with the
`bvn_datetime.h` helpers (`bvnr_datetime_epoch_mjd()` → the epoch, then
`bvn_dt_epoch_seconds_to_datetime()`).

```bovnar
#!bovnar 1.1
.created = <datetime:64,unix> 1750000000;   # 2025-06-15T...Z
.tai_t   = <datetime:tai>     1400000000;
.before  = <datetime>         -100;          # 100 s before 1970-01-01Z
```

**ISO-8601 literals (spec 1.1).** Instead of a raw integer, a `datetime` value
may be written as an ISO-8601 literal:

```
datetime-literal = YYYY-MM-DD [ "T" HH:MM:SS [ "." fraction ] [ zone ] ]
zone             = "Z" | ( "+" | "-" ) HH:MM
```

* `YYYY-MM-DD` — a calendar date (interpreted at `00:00:00Z`)
* `YYYY-MM-DDTHH:MM:SS` — a date and time (UTC when no zone is given)
* a trailing `Z` (UTC) or a numeric `±HH:MM` time-zone offset
* an optional `.fraction` (one or more digits) after the seconds

The literal is converted at parse time to the integer epoch-seconds carrier;
that integer is what is stored and re-emitted, so a pretty-print round-trip is
idempotent (`2026-06-15` becomes `<datetime:64> 1781481600`). A **bare** literal
with no annotation infers `<datetime:64,unix>`, so `.t = 2026-06-15;` is a
timestamp without any annotation. Fields are strictly validated (month `01`–`12`,
a valid day-of-month, hour `00`–`23`, minute `00`–`59`, second `00`–`60`, offset
`±HH:MM` with two-digit components, its hour `00`–`23` and its minute
`00`–`59`). The year is four digits, `0000`–`9999`. Which of two errors a
malformed literal gets is decided by its **skeleton**: the fields each taken as
a run of one or more digits, with every separator in its place —
`D-D-D`, optionally followed by `TD:D:D`, then optionally `.` and zero or more
digits, then optionally `Z` or `±D:D`, where `D` is one or more digits. A
literal that has the skeleton but is otherwise wrong — an out-of-range field, a
field of the wrong width (`2026-6-15`, `12:00:0Z`, `+2:00`) or an empty
fraction — is `error_invalid_datetime_literal`. One that departs from the
skeleton (a missing seconds field, `+0200` without its colon, a lowercase `t`
or `z`) is a lexical error, `error_unexpected_input_byte`, reported at the
first byte that does not fit. A literal in a document that
declares less than 1.1 is `error_illegal_value_type`. A second of `60` is a **UTC leap second** and
is accepted. What it stores depends on the epoch:

* On the **civil epochs** (`unix`, `mjd`, `ntp`, `y2000`) it normalises onto the
  following second — `2016-12-31T23:59:60Z` and `2017-01-01T00:00:00Z` store the
  same `unix` value. Those scales run a uniform 86 400 s day and have no second
  to spend on the insertion; collapsing is the correct POSIX reading.
* On **`tai`** it does not. TAI is a continuous atomic count, the inserted second
  is a real instant on it, and the two literals above store `1861920036` and
  `1861920037` respectively. Over the insertions the implementation's
  leap-second table records, this makes UTC⇄TAI **injective**: every TAI second
  has exactly one civil spelling and every civil spelling one TAI second, so a
  `tai` value survives any number of write/read cycles unchanged.

A `:60` at an instant the implementation's leap-second table does not record as
an insertion collapses on `tai` too: it stores the value of the following
second, as on a civil epoch, and that is the one case in which two civil
spellings share a `tai` value. The table is a static snapshot, and a build
that predates an IERS announcement must not reject a document spelling a genuine
future leap second.

A **`±HH:MM` offset** shifts the written civil time to true UTC before the
conversion (`12:00:00+02:00` is `10:00:00Z`); for `tai` the offset is applied
*before* the leap-second lookup, so the atomic value stays correct. The `.`,
`Z`, and `±HH:MM` parts are valid only after a full `HH:MM:SS` time.

**Fractional seconds** (`.` then one or more digits — ISO 8601 sets no upper
bound on the digit count) are accepted. The integer carrier is still **whole
seconds**, so the fraction takes no part in the value's arithmetic or
comparison (`<datetime:64> 1781524800` and a literal flooring to that second
compare equal at the carrier). But the verbatim digits are **preserved**:
they are surfaced to consumers as a string (the streaming `bvnr_data_t.frac_data`
/ `.frac_length`, or `bvn_dom_get_datetime_fraction()` on a DOM node) and are
re-emitted so the value **round-trips**. A datetime that carries a fraction is
pretty-printed back as an ISO literal — `2026-06-15T12:00:00.5Z` canonicalises
to `<datetime:64> 2026-06-15T12:00:00.5Z` (always normalised to UTC `Z`, with
the annotation made explicit) and re-printing that is idempotent; a datetime
written as a bare integer carrier still canonicalises to the integer. Because a
literal with a fraction is written back as a literal, its instant must have a
four-digit year **in UTC**: one that an offset carries outside `0000`–`9999`
(`0000-01-01T00:00:00.5+23:59`, which is the last day of the year before) is
`error_invalid_datetime_literal`. Without a fraction the same literal is valid —
it is stored and written back as its integer, which has no year to overflow. The
fraction is informational only — for sub-second values that participate in
computation, use a finer integer carrier (e.g. milliseconds since the epoch).

The UTC→epoch conversion is **leap-second correct**: the civil epochs (`unix`,
`mjd`, `ntp`, `y2000`) use the uniform 86 400 s/day scale (so `unix` is ordinary
POSIX time) and `tai` applies the IERS leap-second table. The atomic GNSS epochs
(`gps`, `galileo`, `glonass`, `beidou`) reject a literal with
`error_datetime_literal_unsupported_epoch` — those scales have no
round-trippable civil⇄seconds inverse in this implementation, so supply an
integer epoch-seconds carrier for them. A literal carries no unit, and an ISO
literal under a non-`datetime` annotation is `error_type_value_mismatch`.

```bovnar
#!bovnar 1.1
.a = 2026-06-15;                                  # bare -> <datetime:64,unix>
.b = 2026-06-15T12:00:00Z;                        # date-time, UTC
.c = <datetime:64,tai> 2017-01-01T00:00:00Z;      # tai: leap-second correct
.f = <datetime:64,tai> 2016-12-31T23:59:60Z;      # the insertion itself, one second earlier
.d = 2026-06-15T12:00:00+02:00;                   # offset -> 10:00:00Z
.e = 2026-06-15T12:00:00.5Z;                      # fraction preserved; round-trips as a literal
```

**Parameter syntax:**

```
type-spec = param-type [ ":" type-param-list ]

param-type = "uint" | "sint" | "float" | "float_fix" | "float_dec" | "utf8"
           | "bool" | "datetime"            (* datetime: spec 1.1 *)

type-param-list = type-param { "," type-param }

type-param = width-param    # decimal digits only, e.g. 32
           | base-param     # "_" followed by digits, e.g. _16
                            #   (forbidden for float_fix and float_dec)
           | q-param        # "q" followed by digits, e.g. q8, q16
                            #   (only valid for float_fix)
           | unit-param     # unit string, e.g. m/s, k~g·m/s²
           | epoch-param    # datetime epoch name, e.g. unix, tai (spec 1.1)
```

> **Lexer note on `float_fix` / `float_dec`:** The lexer keyword state machine
> recognises `float` as the type-family prefix and then accumulates the remaining
> annotation bytes (`_fix`, `_dec`, or the parameter separator `:` / closing `>`)
> via `copy_type_byte` into `type_data`.  `bvn_parse_type_annotation` then
> distinguishes `float`, `float_fix`, and `float_dec` from the fully accumulated
> string.

### 5.2 Parameters

| Parameter | Syntax | Valid Values | Applies To |
|-----------|--------|--------------|------------|
| Width | `N` (decimal digits) | `0`, `16`, or any multiple of `32` up to `32768` for `float`; `0`,`16`,`32`,`64`,`128`,`256` for `float_fix`/`float_dec`; `0` to `BVN_MAX_INT_WIDTH` (32768) for `uint`/`sint`/`datetime` | uint, sint, float, float_fix, float_dec, datetime |
| Base | `_N` (underscore + decimal digits) | `2–62` (uint/sint); `64`, `85` are **uint-only** (their alphabets take the sign characters as digits — Base64 has `+`, Ascii85 has `+` and `-` — so signed values are illegal — `error_illegal_value_type`); every other value (`_0`, `_1`, `_63`, `_99`) is `error_illegal_value_type`; `float` accepts only `10` or `16`; **forbidden** for `float_fix` and `float_dec` | uint, sint, float |
| Q (fractional bits) | `qN` (lowercase `q` + decimal digits) | `0 ≤ N < effective_width`; **omitted → 0** | **float_fix only** |
| Epoch (spec 1.1) | a bare name | `unix` (default), `tai`, `gps`, `mjd`, `ntp`, `galileo`, `glonass`, `y2000`, `beidou` (§5.1) | **datetime only** |
| Unit | unit-string | See [Units System](#11-units-system) — supports compound units | uint, sint, float, float_fix, float_dec |

Width `0` means "default width" — `bvn_effective_width` returns `64`.

**Digit alphabets.** The digits of a base are, in order of value:

| Base | Digits, lowest first | Case |
|------|----------------------|------|
| 2–36 | `0`–`9`, then the letters `a`–`z` (10–35) | insensitive: `F` and `f` are both 15 |
| 37–62 | `0`–`9`, `a`–`z` (10–35), `A`–`Z` (36–61) | sensitive: `a` is 10, `A` is 36 |
| 64 | `A`–`Z` (0–25), `a`–`z` (26–51), `0`–`9` (52–61), `+` (62), `/` (63) — the RFC 4648 alphabet, without padding | sensitive |
| 85 | the 85 characters `!` (0x21, value 0) to `u` (0x75, value 84) — Ascii85 | sensitive |

So `<uint:8,_62> "A"` is 36 and `<uint:8,_36> "A"` is 10. A base-85 value that
needs `"` or `\` writes it with the string escape (`\"`, `\\`, §4.3). A
character outside the alphabet is `error_digit_not_in_base`, an empty string
included.

**An omitted Q is 0.** `<float_fix:32>` is `<float_fix:32,q0>`: a fixed-point
value with no fractional bits.

For `float_fix`, the Q value (stored in `value_type_spec_t.base`) is the number of
fractional bits.  `bvn_effective_q` returns this value; `bvn_effective_base` always
returns `10` for `float_fix` and `float_dec`.

For `float_dec`, the binary encoding the C API reads and writes
(`bvn_float_to_decNN` / `bvn_float_from_decNN`) is parameterised by width. The
32-, 64- and 128-bit widths are the IEEE 754-2008 decimal interchange formats in
their **binary-integer (BID)** encoding; 16 bits is not an IEEE width and is
encoded by the same scheme with the parameters below; 256 bits is an in-house
layout that uses the whole exponent field. None of this reaches a document —
there a `float_dec` is a decimal literal (Appendix B.7):

| Width | exp\_bits | coeff\_bits | bias | max decimal digits |
|-------|-----------|-------------|------|--------------------|
| 16 | 6 | 9 | 24 | 2 |
| 32 | 8 | 23 | 101 | 7 |
| 64 | 10 | 53 | 398 | 16 |
| 128 | 14 | 113 | 6176 | 34 |
| 256 | 20 | 235 | 611867 | 70 |

`coeff_bits` is the coefficient as stored in the common case. In the BID widths
a coefficient too large for it is written with the combination field's `11`
prefix, which implies its top three bits and frees one more — that is how 7
digits fit beside 23 bits and 16 beside 53. *Max decimal digits* is the
format's precision, not what `coeff_bits` alone would hold.

For `float_fix`, the wire representation is a signed Q-format integer stored in
`width` bits; the mathematical value is `raw_integer × 2^(-Q)`.

### 5.3 Parameter Order

Parameters are **identified by class** — each class is recognised by its syntactic form — and at most one of each class may appear. They can appear in any order:

```bovnar
# All of these annotations are equivalent (parameter order is free):
.val = <uint:32,_10,no_unit> 42;
#      <uint:_10,no_unit,32>      — identical
#      <uint:no_unit,_10,32>      — identical
```

Because parameters are class-identified rather than positional, an empty
positional slot is never needed, so the parameter list is parsed strictly: a
comma must introduce a real parameter, and a `:` must be followed by at least
one parameter. Empty, trailing, or doubled components — `<uint:8,>`,
`<uint:8,,>`, `<uint:,_16>`, and the bare `<uint:>` — are `error_illegal_value_type`.
So is a second parameter of one class (`<uint:8,16>`, `<uint:_10,_10>`,
`<float:64,m,s>`), and a parameter of a class the family does not take.

#### Whitespace inside an annotation

Whitespace (and a comment, which is whitespace) is allowed **beside a
separator** and nowhere else. The separators are the `:` that introduces the
parameter list, the `,` between parameters, the `<` / `>` that delimit the
annotation, and the `:` that ends a profile namespace (§11.9) — so
`<float:64,udunits : kg>` is `udunits:kg`, the whitespace dropped as it is
beside any other separator. That is where the namespace ends and the code
begins, and nothing can be misread across it:

```bovnar
.a = <float : 64> 1.0;          # valid — either side of the family colon
.b = <uint:8 , _16> "ff";       # valid — either side of a parameter comma
.c = <float:64,m/s > 1.0;       # valid — before the closing bracket
.d = <float:64, # which unit
      m/s> 1.0;                 # valid — a comment stands in for the space
```

**Inside a parameter it is `error_type_param_whitespace`**, reported at the
first byte after the whitespace:

<!-- bovnar-example: rejected -->
```bovnar
.e = <float:64,k g> 1.0;        # error_type_param_whitespace
.f = <uint:6 4> 1;              # error_type_param_whitespace
.g = <float:64,m / s> 1.0;      # error_type_param_whitespace
```

This is a rule about a *wrong value*, not about tidiness. Whitespace inside a
parameter used to be deleted, so `<float:64,m in>` was accepted as `min` — the
minute — and `<uint:6 4>` as a 64-bit width: a wrong unit and a wrong type, each
produced silently. Two things new in 1.2 would have made the trap much larger
had the deletion stayed. With the compact prefix form, `<float:64,k g>` would
have become `k~g`; and UDUNITS and CF multiply with a space, so
`<float:64,udunits:m s-1>` would have been read as `ms-1`, **reciprocal
milliseconds**, for a value the author wrote as a speed. (Neither of those two
was ever accepted by a released reader: 1.1 has no compact form and no
profiles.) The exception below is what makes the space-multiplied form mean what
it says.

A comma nested inside a UCUM annotation (`{…}`) or a bracketed atom (`[…]`) is
part of the code rather than a separator, so it does not end the parameter:
`ucum:mL{cells,tot}` is one parameter, not two.

#### The one exception: a namespaced parameter

Inside a parameter that carries a **profile namespace** (`udunits:`, `ucum:`, …),
whitespace is neither deleted nor refused — it is **kept verbatim** and handed to
the vocabulary, which decides what it means. This exists for one concrete reason:
UDUNITS multiplies with a space, and `kg m-2 s-1` is the commonest spelling of a
flux in netCDF and CF metadata.

<!-- bovnar-example: rejected -->
```bovnar
#!bovnar 1.2
.flux = <float:64,udunits:kg m-2 s-1> 0.5;   # valid — the same unit as udunits:kg*m-2*s-1
.speed = <float:64,udunits:m s-1> 9.81;      # valid — m/s
.t = <float:64,udunits:ms-1> 1.0;            # valid, and DIFFERENT: m~s⁻¹, a
                                             # reciprocal millisecond. The space
                                             # is what tells the two apart
.count = <float:64,ucum:mL{cells, tot}> 1.0; # valid — an annotation is inert
                                             # text and keeps its spacing
.bad = <float:64,ucum:[in i]> 1.0;           # error_unit_illegal — from UCUM's
                                             # grammar, not from the lexer
.bad = <float:64,qudt:Kilo GM> 1.0;          # error_unit_illegal — a flat
                                             # vocabulary has no operators
```

Four details of that exception:

- **The family `:` is not a namespace.** Only a second `:` at bracket depth 0
  makes a parameter namespaced, so `<uint:6 4>` is still
  `error_type_param_whitespace`.
- **Trailing whitespace is trimmed**, since it sits beside the separator that
  ends the parameter: `<float:64,udunits:m >` is `udunits:m`.
- **A comment cannot follow a namespaced code.** Whitespace after it is
  trimmed, but the code runs to the `,` or `>` that ends the parameter, and `#`
  is not a code byte: `<float:64,ucum:kg # note⏎>` is
  `error_unexpected_input_byte`, where `<float:64,kg # note⏎>` is fine. Put the
  comment before the parameter, or beside its namespace colon.
- **A line break is still an error**, even in a namespaced parameter —
  `error_type_param_whitespace`, as it would be in a native one. No
  vocabulary spells a unit across a line, and admitting one would let a
  malformed code consume a document's layout.

An **inline** unit suffix cannot carry a space and never will — whitespace is
what terminates that token, so `1.0 k g` has always been
`error_unexpected_input_byte`, and the space-separated profile spelling is
available in a type annotation only.

### 5.4 Examples

```bovnar
# Simple types
.a = <uint:32> 1000;
.b = <sint:16> -32768;
.c = <float:64> 3.14159;
.d = <utf8> "text";

# Width only
.g = <uint:8> 255;

# Base (requires quoted string for non-decimal)
.h = <uint:_16> "ff";
.i = <sint:_2> "101010";

# Unit
.j = <uint:32,no_unit> 42;      # explicitly dimensionless
.k = <float:64,m/s> 9.81;       # meters per second (compound)
.l = <uint:64,Ki~B> 1024;       # kibibytes
.m = <float:64,k~g·m/s²> 9.81;  # kilograms · meters per second squared
.n = <float:64,m*s> 1.0;        # meter-seconds (product)

# Type annotation with null value
.o = <uint:32> ;                # null of type uint:32

# Fixed-point (Q-format): 16-bit, 8 fractional bits → resolution 2^-8 ≈ 0.0039
.fx1 = <float_fix:16,q8> 3.14;

# Fixed-point: 32-bit, Q16 (16 fractional bits, 15 integer + 1 sign)
.fx2 = <float_fix:32,q16> -1.5;

# Fixed-point with unit
.fx3 = <float_fix:32,q8,m/s> 9.81;

# Fixed-point: width defaults to 64, Q=0 means pure integer fixed-point
.fx4 = <float_fix:64,q0> 42;

# Decimal float: 32-bit (7 significant decimal digits)
.df1 = <float_dec:32> 3.14;

# Decimal float: 64-bit (16 significant decimal digits) with unit
.df2 = <float_dec:64,Pa> 101325;

# Decimal float: 128-bit (34 significant decimal digits)
.df3 = <float_dec:128> 1.2345678901234567890123456789012345;

# float_fix and float_dec do NOT accept a base parameter:
# .bad = <float_fix:32,q8,_10> 1.0;   # ERROR: base forbidden for float_fix
# .bad = <float_dec:64,_10> 1.0;      # ERROR: base forbidden for float_dec
```

### 5.5 Non-decimal Base with Bare Numbers

A bare number literal (§4.6 — the token is lexed before its base is known, so it must be a number in *that* grammar) under a non-decimal base is valid when every digit of it is a digit of that base, and it is read **in that base**: `<uint:_16> 10` is sixteen, not ten. In base 15 and above `e` and `E` are digits, so a bare `1e3` under `_16` is the integer 0x1E3 (483) and not an exponent form; below base 15 the `e` is still an exponent marker, and under an integer family that is `error_type_value_mismatch`. A value that needs a letter the number lexer does not accept cannot be written bare — `ff` is lexed as a symbol, and the type/value check reports `error_type_value_mismatch`; `1e`, an `e` with no digit after it, is not a number token at all and is `error_unexpected_input_byte` — so it must be a quoted string. Producers SHOULD quote every non-decimal value:

```bovnar
.n = <uint:_16> "ff";     # hex value 255 — the form to write
.m = <uint:_16> 10;       # valid, and sixteen: the bare digits are read in base 16
.k = <uint:_16> 1e3;      # valid, and 483: in base 16 the 'e' is a digit
```

---

## 6. Value Tokens

### 6.1 Type/Value Compatibility

| Type Family | Accepts |
|-------------|---------|
| `vt_plain` (default) | Any value |
| `vt_utf8` | String only |
| `vt_bool` | Boolean keyword only (`true`/`false`/`on`/`off`) |
| `vt_uint` | Number or string (digits) |
| `vt_sint` | Number or string (digits, may be negative) |
| `vt_float` | Number or string (may have `.`, `e`/`E`; base 16 strings use `p`/`P`) — only base 10 or 16 |
| `vt_float_fix` | Number or string (may have `.`, `e`/`E`) — base 10 only |
| `vt_float_dec` | Number or string (may have `.`, `e`/`E`) — base 10 only |
| `vt_datetime` (spec 1.1) | A decimal signed integer (epoch seconds), an ISO-8601 literal (§5.1), or a special number (§6.4); `.`/`e` and string carriers are rejected |

Three value kinds stand outside this table. A **null** takes any annotation and carries its type (§4.7). A **struct** takes none: under any annotation it is `error_type_value_mismatch` (§5.1). An **octet stream** under an annotation is accepted under *every* family: the annotation is carried on the stream's start and end events (its chunks are delivered plain) and is **not checked** against it, so `<uint:8>`, `<utf8>` or `<bool>` in front of an octet stream is not `error_type_value_mismatch`. It says nothing about the payload, and producers SHOULD NOT write one.

### 6.2 Validation Rules per Numeric Type

**uint (unsigned integer):**

<!-- bovnar-example: rejected -->
```bovnar
.valid = <uint:8> 255;
.valid = <uint:8> 0;

.invalid = <uint:8> -1;       # error_value_out_of_range (negative)
.invalid = <uint:8> 256;      # error_value_out_of_range (overflow)
```

**sint (signed integer):**

<!-- bovnar-example: rejected -->
```bovnar
.valid = <sint:8> 127;
.valid = <sint:8> -128;

.invalid = <sint:8> 128;      # error_value_out_of_range
.invalid = <sint:8> -129;     # error_value_out_of_range
```

**float (binary floating-point):**

Valid widths: `0` (default 64), `16`, or any multiple of `32` up to `32768`.
Base `10` (default) or `16` are accepted; all other bases are rejected.

**No range or precision check.** A `float` or `float_dec` literal with more
precision or a larger exponent than the declared width can hold is valid
(`<float:16> 1e30`, `<float_dec:16> 1234`, a bare `1e999`). The excess is the
consumer's to round, and a consumer may deliver ±infinity. For these two
families the width bounds what a producer promises, not what the reader
rejects; only the integer families and `float_fix` are range-checked.

<!-- bovnar-example: rejected -->
```bovnar
.valid = <float:64> 3.14;
.valid = <float:64> 1e100;
.valid = <float:64> nan;
.valid = <float:16> 3.14;      # half-precision
.valid = <float:256> 3.14;     # 256-bit extended precision

.invalid = <float:8> 3.14;     # width 8 → error_illegal_value_type
.invalid = <float:12> 3.14;    # not 16 or a multiple of 32 → error_illegal_value_type
.invalid = <float:64,_2> 3.14; # base 2 → error_illegal_value_type
```

**float_fix (fixed-point Q-format):**

Valid widths: `0` (default 64), `16`, `32`, `64`, `128`, `256`.
The Q parameter (`qN`) specifies fractional bits; `0 ≤ N < effective_width`.
Base parameter (`_N`) is forbidden.
The mathematical value of a fixed-point datum is `raw_integer × 2^(-Q)`.

**Range.** A `float_fix` value must be representable in the declared format:
`raw_integer = round(value × 2^Q)`, rounded once to nearest with ties to even,
must fit a signed `width`-bit field. The representable values are
`[-2^(width-1-Q), 2^(width-1-Q) − 2^(-Q)]`, and the test is made **after**
rounding — so a literal just past either end that rounds back onto it is valid
(`<float_fix:16,q8> 127.998` rounds to the maximum, 127.99609375; `127.999`
rounds to 128 and is out of range). A value whose rounded integer does not fit
is `error_value_out_of_range`, exactly as for `uint`/`sint` overflow
(special numbers `nan`/`inf`/`ninf` are range-exempt, §6.4). The C encoders
`bvn_float_to_fixNN` additionally **saturate** to the representable extreme on
overflow rather than wrapping, so a fixed-point datum can never silently decode
to an unrelated value.

<!-- bovnar-example: rejected -->
```bovnar
.valid   = <float_fix:16,q8> 3.14;    # Q8 in 16-bit: range [-128, 127.99609375]
.valid   = <float_fix:16,q8> 127.99609375;  # max in range
.valid   = <float_fix:16,q8> -128;    # min in range
.valid   = <float_fix:32,q16> -1.5;   # Q16 in 32-bit
.valid   = <float_fix:64,q0> 42;      # Q0 = pure integer, no fractional part
.valid   = <float_fix:32,q8,m/s> 9.81;

.invalid = <float_fix:16,q8> 128;     # exceeds Q8/16-bit range → error_value_out_of_range
.invalid = <float_fix:16,q0> 70000;   # exceeds signed-16-bit range → error_value_out_of_range
.invalid = <float_fix:16,q16> 1.0;    # Q >= width → error_illegal_value_type
.invalid = <float_fix:8> 1.0;         # width 8 not in {0,16,32,64,128,256}
.invalid = <float_fix:32,q8,_10> 1.0; # base param forbidden → error_illegal_value_type
```

**float_dec (IEEE 754-2008 decimal floating-point):**

Valid widths: `0` (default 64), `16`, `32`, `64`, `128`, `256`.
Base parameter (`_N`) is forbidden (decimal base is implicit).
Values are written as ordinary decimal literals or special numbers.

<!-- bovnar-example: rejected -->
```bovnar
.valid   = <float_dec:32> 3.14;
.valid   = <float_dec:64,Pa> 101325;
.valid   = <float_dec:128> nan;

.invalid = <float_dec:8> 1.0;         # width 8 not valid → error_illegal_value_type
.invalid = <float_dec:64,_10> 1.0;    # base param forbidden → error_illegal_value_type
```

### 6.3 Digit Validation

Digits in values are checked against the declared base:

<!-- bovnar-example: rejected -->
```bovnar
.value = <uint:_16> "ff";      # OK: f and f are valid in base 16
.value = <uint:_16> "fg";      # error_digit_not_in_base: 'g' > base 16
.value = <uint:_2> "1010";     # OK: valid binary
.value = <uint:_2> "210";      # error_digit_not_in_base
```

#### Exponent Markers in Quoted String Literals

Bare number literals use `e`/`E` as the exponent separator (§4.6) — in a base below 15; from base 15 up the letter is a digit, bare or quoted (§5.5). Quoted string number literals follow the same rule **except** for base-16 (`_16`) float values, where `p`/`P` must be used instead of `e`/`E`. This is required because `e` and `E` are valid hexadecimal digits: with base 16 you must write `"1.8p+2"`, not `"1.8e+2"` — the `e` would be read as a mantissa digit rather than an exponent marker, leaving the trailing `+2` invalid. Float values support a decimal base (the default) and base-16 (`_16`); the `p`/`P` exponent separator applies to the base-16 form.

```bovnar
.hex_float  = <float:64,_16> "1.8p+2";   # OK: 1.8₁₆ × 2² = 6.0  (p = binary exponent)
.hex_mant   = <float:64,_16> "1.8e";     # OK: 'e' is a hex digit → mantissa 1.8e₁₆
.dec_float  = <float:64> 12.0;           # OK: default decimal base
```

The `p`/`P` exponent value is always interpreted as a decimal integer (the binary exponent bias), matching the C99 hexadecimal floating-point literal convention.

### 6.4 Special Number Semantics

`nan`, `inf`, `ninf` are accepted by any numeric type family (`uint`, `sint`, `float`, `float_fix`, `float_dec`) and in untyped context. `bvn_check_acc_range` explicitly returns `true` when `bvn_is_special_number_string` matches, bypassing all range validation regardless of the declared family. They are rejected under the non-numeric families whose token kind is incompatible with `token_is_number` — `utf8` (string-only) and `bool` (boolean-keyword-only) — both producing `error_type_value_mismatch`. (A `datetime` annotation, by contrast, accepts them: the special-string check short-circuits before the datetime carrier is parsed.)

```bovnar
.okay_f64  = <float:64>  inf;
.okay_f32  = <float:32>  nan;
.okay_u8   = <uint:8>    nan;         # accepted — range check bypassed
.okay_s16  = <sint:16>   ninf;   # accepted — range check bypassed

# Fine in plain/untyped context too
.untyped = inf;       # defaults to float:64
```

### 6.5 Inline Unit Suffix

A **scalar** number or string value may carry an optional unit suffix separated from the literal by **at least one whitespace character**. The suffix uses the same character set as the unit parameter inside a type annotation (`unit-param`).

| Separator form | Example | Valid? |
|---|---|---|
| Space | `.a = 9.81 m/s;` | ✓ |
| No separator | `.b = 9.81m;` | ✗ error |

```bovnar
.distance = 100 m;            # plain integer with inline unit: meter
.speed    = 9.81 m/s;         # plain float with inline compound unit
.mass     = 70.0 k~g;         # with SI prefix
.storage  = 4 Gi~B;           # with IEC prefix
.ratio    = 3.14 no_unit;     # explicitly dimensionless via inline suffix
```

The inline unit suffix is **forbidden inside arrays**; any character that would begin an inline unit — a letter, `_`, `$`, `%`, `(`, or a UTF-8 lead byte — following a value inside `[ … ]` is a lexical error (`error_unexpected_input_byte`).

**Interaction with type annotations:**

| Situation | Outcome |
|-----------|---------|
| No annotation, inline unit present | Inline unit is used as the effective unit |
| Annotation has no unit, inline unit present | Inline unit is used as the effective unit |
| Annotation unit and inline unit **match** | Valid; the common unit is used |
| Annotation unit and inline unit **differ** | `error_unit_mismatch` |

**Match** means the two parse to the same components — the same base unit,
prefix and exponent in each, compared as a set, so order and spelling are free
and algebra is not. `m/s` matches `m*s^-1`, `km` matches `k~m`, `L` matches `l`
and `m*s` matches `s*m`; `N` does not match `k~g·m/s²`, and `m` does not match
`no_unit`. An omitted annotation unit matches anything — the inline unit is then
simply the unit. (This is `bvn_unit_equal`, §16.8; it is also the comparison
§7.4 makes between array elements, except that the two dimensionless forms of
§11.8 count as one unit there.)

<!-- bovnar-example: rejected -->
```bovnar
# Annotation unit with no inline unit — normal
.dist = <float:64,m> 1.5;

# No annotation unit, inline unit — unit from suffix
.dist = <float:64> 1.5 m;

# Both present and identical — valid, redundant but allowed
.dist = <float:64,m> 1.5 m;

# Both present and different — error_unit_mismatch
.dist = <float:64,m> 1.5 s;     # ERROR: annotation says m, inline says s
```

An invalid inline unit string (unrecognised base unit, bad prefix, etc.) produces `error_unit_illegal`. `no_unit` is a valid suffix and is checked like any other.

**An inline unit after a `utf8` string is accepted.** The inline path does not consult the type family: `<utf8> "text" m` and a bare `"text" m` are valid, the unit is carried on the value and is written back on re-serialisation — although the same unit as an annotation parameter (`<utf8,m>`) is `error_illegal_value_type` (§5.1). The suffix exists for quoted *numeric* literals (`<uint:_16> "ff" m`); producers SHOULD NOT attach one to text.

---

## 7. Arrays

### 7.1 Row Syntax

An array consists of one or more bracket-enclosed **rows** separated by `/`:

```bovnar
.array_2d = [1, 2, 3]/[4, 5, 6];
```

This produces the event sequence:

```
ev_array_row_start → 3× ev_data  → ev_array_row_end
ev_array_dim_start → ev_array_row_start  → 3× ev_data  → ev_array_row_end
```

### 7.2 Null Elements

Leading/trailing commas, consecutive commas, and the bare keyword `null` all
produce null elements:

```bovnar
.items = [,1,,2,];   # 5 elements: null, 1, null, 2, null
.one   = [null];     # 1 element: a single null
```

**Empty arrays.** A row with nothing between the brackets is **empty** — zero
elements, no `ev_data` emitted. It is distinct from a one-null row:

```bovnar
.empty = [];         # 0 elements
.null1 = [null];     # 1 element (null)
```

An element may be empty only beside a comma, so whitespace alone (`[ ]`) is
still the empty row. A type annotation alone is an element: `[<uint:8>]` is one
typed null, not an empty row (§7.5).

So `bvn_dom_array_count([])` is `0` and `bvn_dom_array_count([null])` is `1`. The
canonical serialiser writes a null array element as the explicit `null` keyword
(e.g. `[, 1]` round-trips through `[null, 1]`), so an empty slot never collides
with the empty array. Empty rows interact with row-size consistency (§7.3): all
`/`-rows must share one width, and `0` is a valid width — `[]/[]` is two empty
rows, but `[]/[1]` is `error_array_row_size_mismatch`.

### 7.3 Row-Size Consistency

A single array's `/`-separated dimension rows (e.g. `[1,2,3]/[4,5,6]`) must all have the same element count. A mismatch produces `error_array_row_size_mismatch`; the lexer performs this check as each row closes, so an offending row is rejected at the earliest possible byte — the element that overshoots, or the `]` that falls short. This makes one `/`-array a clean rectangular N-dimensional block.

Comma-separated elements are distinct *values*, but since spec 1.0 they are no longer unconstrained: they must be **homogeneous** (§7.4).

### 7.4 Element Homogeneity

Since spec 1.0 the elements of an array must be **homogeneous**. The rule is *"shape uniform, fields free"*, checked over the materialised value (above the lexer; it complements the streaming reader's per-value type and unit checks):

- **Kind.** Every non-null element shares the same kind — number, string, symbol, bool, reference, octet stream, array, or struct. `[1, "two"]` and `[1, {.x=1;}]` are `error_array_element_type_mismatch`.
- **Unit** (bare scalar arrays and matrices). Numeric elements must carry the **same unit**, not merely the same dimension (through spec 1.1 this rule read *the same dimension*; §17 records the correction); the numeric encodings (`uint`, `sint`, `float`, `float_fix`, `float_dec`) may mix freely. `[<float:64,m> 1.0, <float:64,k~g> 2.0]` (length vs mass) is rejected, and so are `[<float:64,m> 1.0, <float:64,ft> 2.0]` (a scale apart), `[<float_dec:64,$USD> 1.0, <float_dec:64,k~$USD> 2.0]` (a prefix apart) and `[<float:64,°C> 1.0, <float:64,K> 2.0]` (an affine **offset** apart). `[1, 2.5, 3]` (all dimensionless) is fine, and so is `[<float:64,m*s> 1.0, <float:64,s*m> 2.0]` — the comparison is order-insensitive, because unit multiplication commutes — as is an explicit `no_unit` beside an omitted unit, which are two spellings of the same thing. Each currency is its own unit, so a bare array may not mix `$USD` and `$EUR` values.

  The rule is the unit and not the dimension because of the sentence that closes this section: *a consumer may treat the elements identically*. Under dimensional homogeneity that sentence was false — a reader that took it at its word and applied one unit to `[1.0 m, 2.0 ft]` was wrong by 0.3048, and on `[1.0 °C, 2.0 K]` wrong by an added 273.15. An offset between two neighbouring cells is not a mistake anything downstream can notice, so it is refused at the parse, where every other wrong unit in this format is refused.
- **`datetime` (spec 1.1).** A `datetime` is its own kind: it does **not** mix with the plain numeric encodings above (`[<datetime:64,unix> 1, <sint:64> 2]` is `error_array_element_type_mismatch`). And, exactly as for currencies, its **epoch is a dimension** — a bare array may not mix epochs (`[<datetime:64,unix> 1, <datetime:64,tai> 2]` is rejected); a homogeneous same-epoch datetime array is fine. (These are materialised-document/DOM-tier rules, like the rest of §7.4.)
- **Rectangular** (nested arrays). Sibling sub-arrays must have the same length and recursively-matching element shape: `[[1,2],[3,4]]` is valid, `[[1,2],[3,4,5]]` is `error_array_row_size_mismatch`.
- **Structs — same keys, fields free.** Sibling structs must share the same keys, in order, with the same per-field *kinds* and nesting. Differing keys are `error_struct_shape_mismatch`; a field that is a number in one record and a string in another is `error_array_element_type_mismatch`. But a scalar field may carry a **different unit** in each record, and a list field a **different length** — so a multi-currency ledger and per-record argument lists are valid.
- **Null is a hole.** `null` / empty array elements match any shape and never establish or break homogeneity, so sparse arrays like `[1, , 3]` remain valid.

<!-- bovnar-example: rejected -->
```bovnar
.ok_scalars = [1, 2.5, 3];                 # valid: same (no) dimension, encodings may mix
.ok_matrix  = [[1, 2], [3, 4]];            # valid: rectangular
.ok_records = [{.cur = USD; .bal = <float_dec:64,$USD> 1.0;},
               {.cur = EUR; .bal = <float_dec:64,$EUR> 2.0;}]; # valid: same keys, fields free
.ok_sparse  = [1, , 3];                    # valid: null hole

.bad_kind   = [1, "two"];                  # error_array_element_type_mismatch
.bad_dim    = [<float:64,m> 1.0, <float:64,k~g> 2.0];  # error_array_element_type_mismatch
.bad_scale  = [<float:64,m> 1.0, <float:64,ft> 2.0];   # same dimension, different unit
.bad_offset = [<float:64,°C> 1.0, <float:64,K> 2.0];   # 273.15 apart
.bad_ragged = [[1, 2], [3, 4, 5]];         # error_array_row_size_mismatch
.bad_keys   = [{.x = 1;}, {.y = 1;}];      # error_struct_shape_mismatch
```

A bare array of measurements is therefore uniform — a consumer may read the whole block under the unit of any one element — while records (structs) describe genuinely different things. **Heterogeneous data is modelled with a struct, not an array.** (This is a deliberate tightening over pre-1.0 drafts, which allowed ragged and mixed-type arrays; it also means a ragged or mixed-type JSON array has no bovnar representation and the `json → bvnr` converter rejects it rather than losing structure.)

### 7.5 Array Elements with Type Annotations

Individual elements may carry type annotations. The annotation appears **before** each element value, **after** the comma or opening bracket:

```bovnar
.mixed = [<uint:8> 1, <sint:8> -1, <float:64> 3.14];
.nulls = [<uint:32> , <sint:64> ];    # null, null with types
```

**An element annotation describes that element and no other.** In
`[<uint:8> 1, 300]` the `300` is a plain integer (`uint:64` by §10), not a
`uint:8` out of range. To type every element, annotate the array (§5.1).

**The one exception is `datetime` (spec 1.1).** After an element annotated
`datetime`, a following **bare integer** is read as a datetime of the same width
and epoch — `[<datetime:64,tai> 1, 2]` is two `tai` instants, and
`[<datetime:8> 1, 300]` is `error_value_out_of_range`. Without that, the second
element would be a `uint` beside a `datetime` and the array could never be
homogeneous (§7.4). It reaches integers only: a bare ISO literal still infers
`<datetime:64,unix>` (§10.1), so `[<datetime:64,tai> 1, 2026-06-15]` mixes
epochs and is `error_array_element_type_mismatch`; annotate the array, or each
literal.

### 7.6 Constraints

| Constraint | Limit |
|------------|-------|
| Array nesting depth | `max_array_nesting` — default 64, hard cap 255 |
| Total array items | `max_array_items` (configurable; 0 → 2 147 483 647 internal default) |

---

## 8. Structs (Scopes)

### 8.1 Syntax

```
struct = "{" ws { assignment } ws "}"
```

Structs group related assignments into a nested scope:

```bovnar
.person = {
    .name = "Alice";
    .age = 30;
    .address = {
        .street = "123 Main St";
        .city = "Springfield";
    };
};
```

**Key uniqueness.** Keys must be unique within a single scope — a struct, and
the top-level document. A repeated key is `error_duplicate_struct_key`, so
lookup, references, and iteration always agree on the value of a key:

<!-- bovnar-example: rejected -->
```bovnar
.bad = {.x = 1; .x = 2;};   # error_duplicate_struct_key
.ok  = {.x = 1; .y = 2;};   # fine
```

The same key in *different* scopes is unrelated and always allowed (e.g.
`.a = {.x = 1;}; .b = {.x = 2;};`). The rule is enforced over the materialised
document, alongside array homogeneity (§7.4).

### 8.2 Nesting

Structs can be nested up to `max_struct_nesting` levels — default 64, hard cap 255 (the limit field is a `uint8_t`; setting it to 0 selects the default of 64). Exceeding the configured limit produces `error_struct_nesting_too_high`.

### 8.3 Empty Structs

```bovnar
.empty = {};
```

### 8.4 Structs as Array Elements

```bovnar
.people = [
    {.name = "Alice"; .age = 30;},
    {.name = "Bob"; .age = 25;}
];
```

### 8.5 Unmatched Braces

A `}` seen with `struct_nesting_level == 0` is `error_illegal_struct_close`.

---

## 9. Octet Streams (Binary Mode)

### 9.1 Overview

A NUL byte (`0x00`) where a value is expected switches from text mode to binary chunk mode. The parallel UTF-8 validator is suspended for the duration.

> **Transport.** Chunks are length-prefixed rather than delimited, so a payload may contain any byte, needs no escaping, and does not expand — and a reader can skip a region without inspecting it. The consequence is that a document containing an octet stream is **not** safe through a transport that rewrites bytes. Line-ending normalisation on a `0x0D` inside a payload desynchronises the length prefixes, and the result is unrecoverable: it reads as a malformed document rather than a mangled one. Store such documents as binary (`*.bvnr binary` in `.gitattributes`), and where a channel must stay text, have the reader enforce it with `text_only` (read/write API §1.13) rather than assume it.

### 9.2 Wire Protocol

```
octet-stream = 0x00 { os-chunk } 0x00
os-chunk     = 0x01 os-length os-data
os-length    = uint16 (little-endian); 0x0000 encodes 65536 bytes
os-data      = exactly os-length bytes
```

| Byte | Meaning |
|------|---------|
| `0x00` | End-of-stream marker (at binary level) |
| `0x01` | Data chunk follows |
| `0x01` + `LL LL` + `data` | Chunk of `L` bytes (0x0000 = 65536) |
| Any other tag byte | `error_octet_stream_out_of_sync` |

### 9.3 Events

```
ev_octet_stream_start  →  (emitted on the leading 0x00)
ev_data                →  (emitted for each binary chunk)
ev_octet_stream_end    →  (emitted on the trailing 0x00)
```

### 9.4 Example

An octet stream carries raw bytes, so it cannot be written literally in a text document. The form below is the conventional *escaped* notation used throughout this specification: each `\xNN` stands for one byte of the file, not for those four characters. Pasted verbatim into a `.bvnr` file it is `error_unexpected_input_byte`; the writer API emits the real bytes.

<!-- bovnar-example: illustrative -->
```bovnar
.data = ;
.binary = \x00\x01\x05\x00hello\x01\x03\x00bye\x00;
```

This binary region encodes two chunks: `"hello"` (5 bytes) and `"bye"` (3 bytes), producing:

```
ev_octet_stream_start
ev_data (type=octet_stream, data="hello",  length=5)
ev_data (type=octet_stream, data="bye",    length=3)
ev_octet_stream_end
```

### 9.5 Constraints

- File size: `max_file_size` (0 → **no total-byte cap**, the default; set to `16777216` for the recommended 16 MiB cap). The text is still bounded by `max_text_bytes` (§12.2)
- Octet stream bytes contribute to the file size limit but not to `max_text_bytes`
- EOF inside an octet stream region is `error_read_complete_chunk_failed`, never `error_got_incomplete_bvnr_stream`, and it is not recovered under `continue_on_error`

---

## 10. Default Type Synthesis

When a number or string value carries **no explicit** type annotation (i.e. the validator's `value_type` is still `vt_plain`), the validator **synthesises** a default type annotation before emitting `ev_data`.

### 10.1 Rules

| Value Form | Synthesised Type |
|------------|-----------------|
| Quoted string | `<utf8>` |
| Boolean keyword (`true`/`false`/`on`/`off`) | `<bool>` |
| Special number (`nan`, `inf`, `ninf`) | `<float:64,_10,no_unit>` |
| Number with `.` or `e`/`E` (float literal) | `<float:64,_10,no_unit>` |
| Negative integer | `<sint:64,_10,no_unit>` |
| Plain integer | `<uint:64,_10,no_unit>` |
| Number with an inline currency unit (`$XXX`) | `<float_dec:64,$XXX>` |
| ISO-8601 datetime literal, no annotation (spec 1.1) | `<datetime:64,unix>` |

**The synthesised type is checked like a written one.** A bare integer above
2⁶⁴ − 1, or below −2⁶³, does not fit the `uint:64` / `sint:64` it is given and is
`error_value_out_of_range`; write it under an annotation wide enough
(`<uint:128> 340282366920938463463374607431768211455`). A bare float is never
range-checked (§6.2).

The currency row takes **precedence** over the float/integer rows: any number
(integer or decimal) that carries an inline currency unit synthesises
`float_dec`, so money is never stored as a binary float — `.price = 5 $USD;`
becomes `<float_dec:64,$USD>`, not `<uint:64>`.

> **A bare integer inside a `datetime` array** inherits the `datetime` type
> (width and epoch) of the array's annotation, or of the annotated element
> before it (§7.5), rather than synthesising `uint`/`sint`, so a datetime array
> as the pretty-printer writes it — annotation on the first element, bare
> integers after — stays homogeneous.

> **`float_fix` is never auto-synthesised**, because it requires a Q parameter
> that cannot be inferred from the value literal; it must be introduced by an
> explicit type annotation. **`float_dec` is auto-synthesised in exactly one
> case** — a number carrying an inline currency unit (the row above) — so that
> monetary values are never held as a binary float; in every other context it
> too requires an explicit annotation.

### 10.2 Event Sequence

The synthesised annotation produces the **same** event sequence as an explicit one — for a scalar value, on every value; inside an array only when the synthesised type changes from one element to the next (Appendix A.4). For numeric types (`uint`, `sint`, `float`) the sequence includes three parameter events; for `utf8` no parameter events are emitted:

```
# numeric (uint / sint / float)
ev_type_annotation_start
ev_type_annotation_type_family  → "uint" / "sint" / "float"
ev_type_annotation_type_family_parameter  → width:64
ev_type_annotation_type_family_parameter  → base:_10
ev_type_annotation_type_family_parameter  → unit:no_unit
ev_type_annotation_end
ev_data

# string
ev_type_annotation_start
ev_type_annotation_type_family  → "utf8"
ev_type_annotation_end
ev_data
```

### 10.3 Examples

```bovnar
# No annotation → synthesised <uint:64,_10,no_unit>
.x = 42;

# No annotation → synthesised <float:64,_10,no_unit>
.y = 3.14;
.z = inf;

# No annotation → synthesised <sint:64,_10,no_unit>
.w = -7;

# No annotation → synthesised <utf8>
.s = "hello";
```

---

## 11. Units System

### 11.1 Base Units

The unit system supports **264 named base units** across SI, IEC-binary, Imperial/US customary, CGS electromagnetic, radiation, electrical-power, and other categories. The table below covers the SI base units, all 22 named SI-derived units (degree Celsius among them — it is listed under *Non-SI units accepted for use with SI* below, where it is conventionally grouped), and the other non-SI units accepted for use with SI. For the complete reference — including Imperial/US customary, CGS, radiation, electrical-power, rotational, textile, surveying, volume, and other unit families — see **[`doc/05_bovnar_unit_system.md`](05_bovnar_unit_system.md)**.

**SI base units and digital units**

| Symbol | Unit | Enum |
|--------|------|------|
| `b` | bit | `bu_bit` |
| `B` | byte | `bu_byte` |
| `s` | second | `bu_second` |
| `m` | meter | `bu_meter` |
| `g` | gram | `bu_gram` |
| `A` | ampere | `bu_ampere` |
| `K` | kelvin | `bu_kelvin` |
| `mol` | mole | `bu_mol` |
| `cd` | candela | `bu_candela` |

**Named SI-derived units**

| Symbol | Unit | Enum |
|--------|------|------|
| `Hz` | hertz | `bu_hertz` |
| `N` | newton | `bu_newton` |
| `Pa` | pascal | `bu_pascal` |
| `J` | joule | `bu_joule` |
| `W` | watt | `bu_watt` |
| `V` | volt | `bu_volt` |
| `Ω` | ohm | `bu_ohm` |
| `F` | farad | `bu_farad` |
| `C` | coulomb | `bu_coulomb` |
| `S` | siemens | `bu_siemens` |
| `Wb` | weber | `bu_weber` |
| `T` | tesla | `bu_tesla` |
| `H` | henry | `bu_henry` |
| `lm` | lumen | `bu_lumen` |
| `lx` | lux | `bu_lux` |
| `Bq` | becquerel | `bu_becquerel` |
| `Gy` | gray | `bu_gray` |
| `Sv` | sievert | `bu_sievert` |
| `kat` | katal | `bu_katal` |
| `rad` | radian | `bu_radian` |
| `sr` | steradian | `bu_steradian` |

**Non-SI units accepted for use with SI**

| Symbol | Unit | Enum |
|--------|------|------|
| `L`, `l` | liter | `bu_liter` |
| `min` | minute | `bu_minute` |
| `h` | hour | `bu_hour` |
| `d` | day | `bu_day` |
| `wk` | week | `bu_week` |
| `yr` | year | `bu_year` |
| `°`, `deg`, `degr`, `degree`, `degrees` | degree (angle) | `bu_degree` |
| `°C`, `degC`, `degrC` | degree Celsius | `bu_celsius` |
| `t` | tonne | `bu_tonne` |
| `bar` | bar | `bu_bar` |
| `eV` | electronvolt | `bu_electronvolt` |
| `Da`, `dalton`, `amu`, `u` | dalton | `bu_dalton` |
| `au` | astronomical unit | `bu_astronomical_unit` |
| `ha` | hectare | `bu_hectare` |

> For Imperial/US customary length (`in`, `ft`, `yd`, `mi`, `nmi`, `Å`, `ly`, `pc`, `fur`, `fath`, `ch`, `rd`, `thou`/`mil`), mass (`lb`, `oz`, `gr`, `st`, `tn_sh`, `tn_l`, `oz_t`, `ct`, `slug`, `dr`, `dwt`), temperature (`°F`, `Ra`), pressure (`atm`, `mmHg`, `Torr`, `psi`, `inHg`, `at`), energy (`cal`, `Btu`, `erg`, `thm`, `ft_lb`), power (`hp`, `PS`/`CV`), force (`lbf`, `dyn`, `kip`, `kgf`), speed/frequency/rotation (`kn`, `rpm`, `rev`), volume (US and UK gallons, pints, fluid ounces, and many more), area (`ac`, `barn`), angle (`arcmin`, `arcsec`, `grad`), CGS (`P`, `St`, `G`, `Mx`, `Oe`, `sb`, `ph`, `Gal`), radiation (`Ci`, `R`, `rem`), logarithmic (`Np`, `dB`), electrical power (`var`, `VA`), acceleration (`gn`), time (`mo`, `fn`), textile linear density (`tex`, `den`), and apothecary/dry volume (`fl_dr`, `minim`, `pk`, `bsh`) — see the [Unit System Reference](05_bovnar_unit_system.md).

### 11.2 SI Prefixes

| Prefix | Symbol | Factor | Enum |
|--------|--------|--------|------|
| quetta | `Q` | 10³⁰ | `si_quetta` |
| ronna | `R` | 10²⁷ | `si_ronna` |
| yotta | `Y` | 10²⁴ | `si_yotta` |
| zetta | `Z` | 10²¹ | `si_zetta` |
| exa | `E` | 10¹⁸ | `si_exa` |
| peta | `P` | 10¹⁵ | `si_peta` |
| tera | `T` | 10¹² | `si_tera` |
| giga | `G` | 10⁹ | `si_giga` |
| mega | `M` | 10⁶ | `si_mega` |
| kilo | `k` | 10³ | `si_kilo` |
| hecto | `h` | 10² | `si_hecto` |
| deca | `da` | 10¹ | `si_deca` |
| deci | `d` | 10⁻¹ | `si_deci` |
| centi | `c` | 10⁻² | `si_centi` |
| milli | `m` | 10⁻³ | `si_milli` |
| micro | `µ` (or `u`) | 10⁻⁶ | `si_micro` |
| nano | `n` | 10⁻⁹ | `si_nano` |
| pico | `p` | 10⁻¹² | `si_pico` |
| femto | `f` | 10⁻¹⁵ | `si_femto` |
| atto | `a` | 10⁻¹⁸ | `si_atto` |
| zepto | `z` | 10⁻²¹ | `si_zepto` |
| yocto | `y` | 10⁻²⁴ | `si_yocto` |
| ronto | `r` | 10⁻²⁷ | `si_ronto` |
| quecto | `q` | 10⁻³⁰ | `si_quecto` |

> **Note:** `µ` = U+00B5 (MICRO SIGN), encoded as `0xC2 0xB5` in UTF-8. ASCII `u` is accepted as an input-only alias for the micro prefix (`u~m` = `µ~m`); `bvn_unit_to_string` and the `bvnr_write_*` helpers always render it as `µ`, though the pretty-print / canonicalize path preserves the input `u` at default `unit_flags`.

### 11.3 IEC Binary Prefixes

| Prefix | Symbol | Factor | Enum |
|--------|--------|--------|------|
| kibi | `Ki` | 2¹⁰ | `iec_kibi` |
| mebi | `Mi` | 2²⁰ | `iec_mebi` |
| gibi | `Gi` | 2³⁰ | `iec_gibi` |
| tebi | `Ti` | 2⁴⁰ | `iec_tebi` |
| pebi | `Pi` | 2⁵⁰ | `iec_pebi` |
| exbi | `Ei` | 2⁶⁰ | `iec_exbi` |
| zebi | `Zi` | 2⁷⁰ | `iec_zebi` |
| yobi | `Yi` | 2⁸⁰ | `iec_yobi` |
| robi † | `Ri` | 2⁹⁰ | `iec_robi` |
| quebi † | `Qi` | 2¹⁰⁰ | `iec_quebi` |

† `Ri`/`Qi` are a forward-looking extension: IEC 80000-13 stops at yobi (`Yi`, 2⁸⁰).

### 11.4 Unit Notation

The unit system supports **compound units** composed of multiple base-unit terms combined with product and division separators.

```
compound-unit  = "no_unit" | unit-expr
unit-expr      = unit-factor { unit-sep unit-factor }
unit-factor    = unit-component | "(" unit-expr ")"

unit-sep       = "*" | "/" | "·"       (* "·" = U+00B7 MIDDLE DOT *)

unit-component = [ prefix [ "~" ] ] base-unit [ unit-exponent ]
```

**Separators:**

| Separator | Code Point | Meaning |
|-----------|-----------|---------|
| `*` | U+002A | Product (multiplication) |
| `·` | U+00B7 | Product (multiplication) — visually preferred |
| `/` | U+002F | Division — subsequent components are in the denominator |

The `·` (middle dot, U+00B7, encoded as `0xC2 0xB7`) and `*` (asterisk) are semantically equivalent; both indicate multiplication of the adjacent unit components.

The `/` separator divides the preceding components by the following ones. The first `/` switches all subsequent components into the denominator; additional `/` separators do not toggle back to the numerator. Every component after the first `/` is always in the denominator.

**Parenthesised grouping.** A `(…)` group is a sub-expression evaluated independently; like any factor it obeys the latching denominator, so a `/` before a group negates the group's net exponents as a whole. Thus `k~g/(m·s²)` parses to `kg·m⁻¹·s⁻²` (identical to `k~g/m·s²`), while `(k~g/m)·s²` is `kg·m⁻¹·s²`. An explicit separator is required before a group (`m·(s)`, not `m(s)`); a group is not followed by its own exponent (`(m·s)²` is rejected); parentheses must balance and nest no deeper than 16. The `bvnr_write_*` helpers and `bvn_unit_to_string`, which build the unit from a parsed `value_unit`, emit the canonical, parenless form (`k~g/(m·s²)` → `k~g/m·s²`); the reader-driven pretty-print / canonicalize path preserves the source spelling verbatim unless a unit flag such as `BVN_UNIT_REDUCE` is set.

**Within each `unit-component`:**

- A bare base unit with no prefix requires no separator.
- When a prefix is present on a **physical** unit, the `~` separator is optional: `k~g` and `kg` are the same unit, as are `Mi~B` and `MiB`. The base unit is matched as the longest alias suffix, so a bare unit always outranks a prefixed reading of the same token (`min` is the minute, not milli-inch) — a compact spelling is therefore only ever accepted where the separated form would have been `error_unit_illegal`, and no existing document decodes differently because of it. Prefixes still cannot be stacked (`kkg`, `k~kg` → `error_unit_illegal`), prefix–unit validity is unchanged, and three compact spellings are refused by name because the token is a well-known annotation for something else: `usb` (the bus, not microstilb — write `u~sb`), `kt` (a standard abbreviation for *two* units this format models, the kilotonne and the knot — write `k~t` or `kn`) and `ppt` (parts per trillion or per thousand in the wild, a picopint by the compact rule — write `pptr`, `‰` or `p~pt`). `pH`, `mph` and `kph` needed an exception until the quantities they name became units in their own right; a bare alias outranks any prefixed reading, so they now resolve without one. See §4.3 of the unit-system reference.
- The same holds for a **currency**: `k~$EUR` and `k$EUR` are the same unit. The `$` sigil already separates the prefix from the code, so the compact form is unambiguous by construction — but the sigil itself stays mandatory, and `kUSD` is `error_unit_illegal` just as bare `USD` is.
- The canonical output form always carries the separator: `bvn_unit_to_string` and the `bvnr_write_*` helpers emit `k~g`. A *re-serialisation* is a different matter — it writes back the spelling the document carried (`unit_source`, §16.1), so `km` stays `km` through `bovnar pretty-print`. `bovnar pretty-print --canonical` does not keep the spelling either way: it writes every unit in reduced form and rescales the value to match (`1.0 km` becomes `1000.0 m`, `2.5 kg` becomes `2500.0 g`). A prefix on a base unit, a currency or a non-SI unit is folded into the value; a unit that is, or reduces to, a single named derived SI unit keeps its prefix, in the separated form (`kPa` → `k~Pa`, `k~N·m` → `k~J`), so `2.5 k~N` and `2500 N` are not unified. An integer that would stop being an integer, or stop fitting its width, fails with `error_value_out_of_range`; nothing is rounded.

```bovnar
# Simple (single-component) units — same as before
.time = <float:64,s> 2.5;               # seconds
.speed = <float:64,k~m> 1.5;            # kilometers (kilo-meter)
.alt = <float:64,km> 10.5;              # same unit, compact spelling

# Compound units
.velocity = <float:64,m/s> 9.81;        # meters per second
.accel = <float:64,m/s²> 9.81;          # meters per second squared
.force = <float:64,k~g·m/s²> 9.81;      # kilogram-meters per second squared
.energy = <float:64,k~g·m²/s²> 1000;    # kilogram-square-meters per second squared
.moment = <float:64,m*s> 1.0;           # meter-seconds
.area_density = <float:64,k~g/m²> 5.0;  # kilograms per square meter
.three_term = <float:64,k~g·m·s⁻²> 9.81;  # equivalent to k~g·m/s²
.pressure = <float:64,k~g/(m·s²)> 101325; # grouped denominator (= k~g/m·s²)

# Explicitly dimensionless
.no_unit_float = <float:64,no_unit> 3.14;
```

### 11.5 Unit Exponents

Exponents can be written in two forms:

| Form | Example | Meaning |
|------|---------|---------|
| Unicode superscript | `m²`, `m⁻³`, `m¹⁰⁰` | `[⁺⁻]? [⁰-⁹]{1,3}` — one to three superscript digits |
| ASCII caret | `m^2`, `m^-3`, `m^+2`, `m^100` | `^[+-]?[0-9]{1,3}` |

An exponent is an **integer in `[BVN_EXPONENT_MIN, BVN_EXPONENT_MAX]` = `[-100, 100]`**, with **zero
reserved**: `m^0` and `m⁰` are not units. The two forms carry the same values; `⁺` and `+` are
accepted and have no effect.

At most **three digits** are scanned, which is exactly what 100 needs, and leading zeros count
among them: `m^007` and `m⁰⁰⁷` are `m⁷`, and an exponent that is zero however it is written (`m^0`,
`m^00`, `m⁰`) is not a unit. A longer run is therefore not an over-large exponent but an
unrecognised token: the exponent of `m^1000` or `m^0001` is not a three-digit one, and what is
left in front of it does not resolve as a base symbol. A value that scans cleanly and lands outside the range —
`m^200` — is refused on the range check. Both are `error_unit_illegal`.

> The named enumerators of `unit_exponent_t` cover only ±1…±9 and are kept for callers that use
> them; they are **not** the range. Do not switch exhaustively over that enum, and do not treat a
> value outside the named nine as invalid — ask `bvn_int_to_exponent`, which returns `exp_invalid`
> for anything genuinely out of range.

**Superscript mapping:**

| Glyph | Code Point | Exponent |
|-------|-----------|----------|
| `¹` | U+00B9 | 1 |
| `²` | U+00B2 | 2 |
| `³` | U+00B3 | 3 |
| `⁴` | U+2074 | 4 |
| `⁵` | U+2075 | 5 |
| `⁶` | U+2076 | 6 |
| `⁷` | U+2077 | 7 |
| `⁸` | U+2078 | 8 |
| `⁹` | U+2079 | 9 |
| `⁰` | U+2070 | 0 — a digit like the others, e.g. `m¹⁰⁰`; an exponent that comes to zero (`m⁰`) is not a unit |
| `⁺` | U+207A | positive sign (no-op) |
| `⁻` | U+207B | negate exponent |

### 11.6 Examples

```bovnar
.distance = <float:64,k~m> 1.5;             # kilometers
.mass = <float:64,g> 500;                   # grams
.velocity = <float:64,m/s> 9.81;            # meters per second
.acceleration = <float:64,m/s²> 9.81;       # meters per second squared
.pressure = <float:64,Pa> 101325;           # pascals (= N/m² = k~g/(m·s²))
.energy = <float:64,k~J> 1000;              # kilojoules
.storage = <uint:64,Ti~B> 2;                # tebibytes
.frequency = <float:64,k~Hz> 2.4;           # kilohertz
.force = <float:64,k~g·m/s²> 9.81;         # kilogram-meters per second squared
.momentum = <float:64,k~g·m/s> 0.5;        # kilogram-meters per second
.density = <float:64,k~g/m³> 7800;          # kilograms per cubic meter
```

### 11.7 Compound Unit Constraints

| Constraint | Limit |
|------------|-------|
| Maximum components per compound unit | 32 (`BVNR_MAX_UNIT_COMPONENTS`) |

If a compound unit string contains more than `BVNR_MAX_UNIT_COMPONENTS` components after parsing, the validator raises `error_unit_illegal`. A **profile** expression (§11.9) that is valid in its vocabulary and translates to more than that is `error_unit_profile_unsupported` instead: the code is not malformed, this unit system cannot hold it.

Empty components between separators (e.g., `m//s`, `m*·s`) produce `error_unit_illegal`.

### 11.8 The `no_unit` Keyword

The literal string `no_unit` in the unit parameter position means "explicitly dimensionless":

```bovnar
.dimensionless = <uint:32,no_unit> 42;
```

Within an explicit annotation, **omitting** the unit parameter yields the same internal representation as an explicit `no_unit`: `BVN_UNIT_NONE` with `num_components == 0`. (`bvn_parse_type_annotation` initialises the unit to `BVN_UNIT_NONE` and only overwrites it when a dimensioned unit parameter is actually present.)

A **fully untyped** value (no annotation at all) instead defaults to dimensionless via default-type synthesis, producing `BVN_UNIT_NO_PREFIX(bu_none)` with `num_components == 1` and `base == bu_none`. All three forms are semantically equivalent — they compare as compatible via `bvn_units_compatible` and both encodings serialize to `"no_unit"` via `bvn_unit_to_string` — but the untyped-default form (`num_components == 1`) is a structurally distinct internal state from the annotated forms (`num_components == 0`).

### 11.9 Unit Profiles (spec 1.2)

A unit parameter that begins with a namespace and a colon — `name:code` — is a **unit profile**: a foreign notation for the same unit slot. The profile translates the code into exactly the value described in §11.1–§11.8, so the result is a unit like any other and every rule in this section applies to it unchanged. A namespace is lowercase letters, digits and `-`; the hyphen may not lead. Anything else before the colon is not a namespace at all, so the parameter is read as a native unit and fails as one: `UCUM:kg` is `error_unit_illegal`, not an unknown profile. Every defined namespace begins with a letter, and a new one must: a parameter that begins with a digit is read as a width, so `<float:64,1u:kg>` is `error_illegal_value_type` and not an unknown profile.

Seven namespaces are defined:

| Namespace | Vocabulary | Code shape |
|-----------|------------|------------|
| `ucum` | UCUM — Unified Code for Units of Measure | an expression over prefixed atoms |
| `unece` | UN/ECE Recommendation 20 and 21 | one whole code, never decomposed |
| `qudt` | QUDT unit local names | one whole code, never decomposed |
| `qudt-qk` | QUDT quantity kinds — translated to the **coherent SI unit** of the kind | one whole code, never decomposed |
| `udunits` | UDUNITS-2, the CF/netCDF units syntax | an expression over prefixed atoms |
| `om` | OM 2 — Ontology of units of Measure | one whole code, never decomposed |
| `cf` | CF standard names — translated to the **`canonical_units`** CF states for the name; **read-only**: no unit is ever translated *into* a `cf:` code, though a spelling a document already carried is kept when it is re-serialised | one whole code, never decomposed |

```bovnar
#!bovnar 1.2
.systolic = <float_dec:64,ucum:mm[Hg]>  120.00;   # the same unit as <...,mmHg>
.mass     = <float:64,unece:KGM>         12.5;    # the same unit as <...,k~g>
.velocity = <float:64,qudt:M-PER-SEC>     9.81;   # the same unit as <...,m/s>
.length   = <float:64,qudt-qk:Length>     3.0;    # the coherent unit of the kind: m
.flux     = <float:64,udunits:kg*m-2*s-1> 0.5;
```

A code that is one whole token is **never decomposed**: `unece:KGM` is the kilogram, not a `k` prefix on a `GM`, and `qudt:MI` is the mile, not a milli-anything. Only an expression namespace strips prefixes and recognises operators.

**Gated on the declared version.** A profile unit requires a `#!bovnar` directive declaring 1.2 or later, exactly as the datetime family and the `\x`/`\u` escapes require 1.1 (§3.4). In a document declaring less — or declaring nothing, which declares less than anything — a profile unit is `error_unit_illegal`, because in that version it is not a unit. A **native** unit is unaffected in every version; the notation is purely additive.

**A unit, an opaque unit, or one of three errors — and nothing else.** A profile expression becomes a unit the parser can compare — a registry unit, or an opaque one that equals only itself — or it becomes an error. There is no state in which a value carries a unit the parser cannot reason about — every guarantee this section makes is a guarantee about units the parser understands. The refusals are distinguished so a producer can act on them:

| Outcome | Result |
|---------|--------|
| Translates onto the unit registry | An ordinary unit; indistinguishable from the native spelling |
| An **opaque** unit — a UCUM *arbitrary* atom (`[IU]`, `[PFU]`, …) or a UNECE package/count code (`XBX`, `C62`, …) | A unit with no native spelling and no dimension: comparable with itself, never convertible to anything else. Two different opaque units are two different units — `error_unit_mismatch` between an annotation and an inline suffix (§6.5), `error_array_element_type_mismatch` in a bare array (§7.4). It serialises back in the notation of the namespace that owns it |
| Not a valid expression in the profile, or an atom it does not define | `error_unit_illegal` |
| Valid in the profile, with no representation in this unit system | `error_unit_profile_unsupported` |
| The namespace is not a profile this implementation supports | `error_unit_profile_unknown` |

**Character set.** A profile unit may use `'`, `[`, `]`, `{` and `}` in addition to the native unit characters (§11.4). A `,` inside `{…}` or `[…]` belongs to the unit rather than ending the type parameter, and inside a type annotation SP and HT within the code belong to it too (§5.3) — UDUNITS multiplies with a space. An inline suffix admits neither, but is otherwise a profile unit like any other (`.p = 120 ucum:mm[Hg];`), under the same version gate. `;`, `#`, `<`, `>` and `"` remain excluded, so an unterminated bracket or annotation cannot consume the remainder of the document.

The complete notation, the transliteration table, and the codes that have no representation are specified in [Unit Profiles](11_bovnar_unit_profiles.md).

---

## 12. Validation & Constraints

### 12.1 UTF-8 Validation

| Constraint | Error |
|------------|-------|
| Non-UTF-8 byte sequence | `error_invalid_utf8_byte` |
| Overlong encoding | `error_invalid_utf8_byte` |
| Surrogate halves (U+D800–U+DFFF) | `error_invalid_utf8_byte` |

### 12.2 Size Limits

| Quantity | Configurable | Default | Overflow Error |
|----------|-------------|---------|----------------|
| Identifier length | Yes (`max_identifier_length`) | 255 | `error_identifier_too_long` |
| String length | Yes (`max_string_length`) | 65535 | `error_string_too_long` |
| Number length | Yes (`max_number_length`) | 65535 | `error_number_too_long` |
| Symbol length | Yes (`max_symbol_length`) | 255 | `error_symbol_too_long` |
| Reference length | Yes (`max_reference_length`) | 65535 | `error_reference_too_long` |
| Array items | Yes (`max_array_items`) | 2 147 483 647 | `error_too_many_array_items` |
| Text bytes | Yes (`max_text_bytes`) | 2 147 483 647 | `error_text_data_too_long` |
| File size | Yes (`max_file_size`) | 0 (→ no total-byte cap; text stays bounded by `max_text_bytes`) | `error_file_too_long` |
| Struct nesting | Yes (`max_struct_nesting`) | 0 (→64 internal, hard cap 255) | `error_struct_nesting_too_high` |
| Array nesting | Yes (`max_array_nesting`) | 0 (→64 internal, hard cap 255) | `error_array_nesting_too_high` |
| Type-annotation body (the bytes between `<` and `>`, whitespace beside separators not counted) | No | 255 | `error_type_too_long` |
| Inline unit suffix | No | 255 | `error_unit_too_long` |
| Unit components | No (`BVNR_MAX_UNIT_COMPONENTS`) | 32 | `error_unit_illegal` (§11.7); a profile expression, `error_unit_profile_unsupported` |
| Unit exponent | No | −100 … 100, zero excluded | `error_unit_illegal` (§11.5) |
| Unit `(…)` group nesting | No | 16 | `error_unit_illegal` (§11.4) |

Setting most fields to `0` in `bvnr_read_flags_t` substitutes an internal default — **64** for both nesting depths, and **2 147 483 647** (2³¹ − 1) for `max_array_items` and `max_text_bytes`. **`max_file_size` is the exception: `0` means no total-byte cap**; set a positive value to cap. That is not the same as endless: the text of a document is still bounded by `max_text_bytes` (default 2³¹ − 1, `error_text_data_too_long`) and each array by `max_array_items`, and only octet-stream payload runs without bound. For an endless text stream raise `max_text_bytes` as well. The writer applies the two nesting defaults and consults no other field: it does not limit array items, text bytes or file size, and it ignores the five `max_*_length` fields — instead it refuses, whatever the flags say, an identifier, string or symbol over 65 535 bytes, the most any reader configuration accepts.

### 12.3 Value Validation

| Check | Error |
|-------|-------|
| Number literal, bare or quoted, contains a digit outside its base | `error_digit_not_in_base` |
| Whitespace inside a type-annotation parameter (§5.3) | `error_type_param_whitespace` |
| Integer value exceeds declared width | `error_value_out_of_range` |
| `float_fix` value outside the declared Q-format range (§6.2) | `error_value_out_of_range` |
| Negative number with `uint` type | `error_value_out_of_range` |
| Mismatched type family for value token | `error_type_value_mismatch` |
| Dot or exponent in integer-typed value (an `e`/`E` is an exponent only in a base below 15, §5.5) | `error_type_value_mismatch` |
| ISO-8601 datetime literal with an out-of-range or wrong-width field, or an empty fraction (spec 1.1); one that breaks off part-way is `error_unexpected_input_byte` | `error_invalid_datetime_literal` |
| ISO-8601 literal for an atomic GNSS epoch — `gps`/`galileo`/`glonass`/`beidou` (spec 1.1) | `error_datetime_literal_unsupported_epoch` |
| Invalid unit string | `error_unit_illegal` |
| Empty parameter component — `<uint:>`, `<uint:8,>`, `<uint:8,,>`, `<uint:,_16>` (§5.3) | `error_illegal_value_type` |
| Wholly empty annotation `<>` / `< >` (no family keyword) | `error_unexpected_input_byte` |
| `utf8` (string) type given a number value | `error_type_value_mismatch` |
| Base `_N` (N≠10, N≠16) for `float` | `error_illegal_value_type` |
| Base param (`_N`) used with `float_fix` or `float_dec` | `error_illegal_value_type` |
| Q param (`qN`) used with family other than `float_fix` | `error_illegal_value_type` |
| Q value ≥ effective width for `float_fix` | `error_illegal_value_type` |
| Invalid float width (not 0/16/multiple-of-32) for `float` | `error_illegal_value_type` |
| Invalid float_fix/float_dec width (not 0/16/32/64/128/256) | `error_illegal_value_type` |

### 12.4 Array Validation

| Check | Error | Tier |
|-------|-------|------|
| Element count mismatch across the `/`-dimension rows of a single array | `error_array_row_size_mismatch` | Streaming |
| Ragged sibling sub-arrays — differing lengths between siblings (§7.4) | `error_array_row_size_mismatch` | DOM |
| Array elements of mixed kind or unit, or a struct field that differs in kind across record elements (§7.4) | `error_array_element_type_mismatch` | DOM |
| Array nesting overflow (exceeds `max_array_nesting`) | `error_array_nesting_too_high` | Streaming |
| Comma outside array context | `error_unexpected_input_byte` | Streaming |

### 12.5 Struct Validation

| Check | Error | Tier |
|-------|-------|------|
| Unmatched `}` | `error_illegal_struct_close` | Streaming |
| A key repeated within one scope (struct or top-level document) (§8.1) | `error_duplicate_struct_key` | DOM |
| Struct array elements with differing key sets (§7.4) | `error_struct_shape_mismatch` | DOM |
| Nesting depth exceeded | `error_struct_nesting_too_high` | Streaming |

> **Validation tier.** *Streaming* checks are raised by the pull-based reader
> (`bvnr_read`) during the `on_verified` event stream. *DOM* checks — the
> spec-1.0 homogeneity (§7.4), struct-shape, and duplicate-key (§8.1) rules —
> are enforced only when a document is **materialised** into a tree
> (`bvn_dom_parse`), because they require comparing sibling elements that the
> streaming reader sees one at a time. A streaming-only consumer therefore
> accepts a heterogeneous array, a ragged sibling sub-array, a shape-mismatched
> record set, or a duplicate key without error; a full spec-1.0 implementation
> must apply these four checks (`error_array_row_size_mismatch` for ragged
> siblings, `error_array_element_type_mismatch`, `error_struct_shape_mismatch`,
> `error_duplicate_struct_key`) in its document/tree API. See the conformance
> tool's "Validation tiers" (§3 of `doc/13_bovnar_conformance.md`).

### 12.6 Identifier Validation

| Check | Error |
|-------|-------|
| Empty key (`.` followed by `=` or by whitespace) | `error_empty_identifier` |
| `.` followed by any other byte that cannot start an identifier (`.1`, `.;`, `.-a`) | `error_unexpected_input_byte` |
| Invalid character in identifier | `error_unexpected_input_byte` |

### 12.7 String Validation

| Check | Error |
|-------|-------|
| Unknown escape sequence (incl. `\x`/`\u` outside spec 1.1) | `error_illegal_escape_sequence` |
| `\u{…}` surrogate or value `> U+10FFFF` (spec 1.1) | `error_invalid_codepoint` |
| `\x` byte(s) that break the string's UTF-8 validity (spec 1.1) | `error_invalid_utf8_byte` |
| Control byte in string | `error_unexpected_input_byte` |
| String length exceeded | `error_string_too_long` |

### 12.8 Unit Validation

| Check | Error |
|-------|-------|
| Invalid unit string | `error_unit_illegal` |
| Compound unit exceeds `BVNR_MAX_UNIT_COMPONENTS` | `error_unit_illegal` (a profile expression: `error_unit_profile_unsupported`, §11.7) |
| Empty component between separators | `error_unit_illegal` |
| Inline unit suffix too long (255-byte lexer buffer) | `error_unit_too_long` |
| Unit written in an annotation, over the 255-byte type-annotation body cap (§12.2) | `error_type_too_long` |
| Inline unit suffix differs from type-annotation unit | `error_unit_mismatch` |
| Inline unit suffix inside an array element | `error_unexpected_input_byte` |

### 12.9 Octet Stream Validation

| Check | Error |
|-------|-------|
| Unknown tag byte in binary mode | `error_octet_stream_out_of_sync` |
| Incomplete chunk read, or EOF anywhere before the closing `0x00` | `error_read_complete_chunk_failed` — never followed by `error_got_incomplete_bvnr_stream`, and it ends the read even under `continue_on_error` (§13.2) |
| An octet stream under `text_only` | `error_octet_stream_forbidden` |

---

## 13. Error Handling & Recovery

### 13.1 Error Model

Errors are reported through the `on_error` callback:

```c
typedef void (*bvnr_on_error_fn)(
    void* userdata, error_code_t err,
    uint64_t line, uint64_t column,
    uint32_t byte, uint64_t offset);
```

### 13.2 Recovery Mode

When `bvnr_read_flags_t.continue_on_error` is `true`, the parser enters **resync mode** after an error in the text — any error but one inside a comment (below). A few errors end the read even so: a fault in an octet stream's framing (`error_octet_stream_out_of_sync`, `error_read_complete_chunk_failed`), `error_text_data_too_long`, `error_file_too_long`, a version directive refused under `strict_version` (§3.4), and a callback that returns `false`. The other limits of §12.2 — the five lengths, `max_array_items`, both nesting depths — are recovered from like any other error.

1. The `on_error` callback is invoked with error details
2. A **resync state machine** (`state_t: resync, resync_dot, resync_string, resync_string_escape, resync_comment`) skips bytes
3. Tracking bracket `[]` and brace `{}` nesting, each with its own counter (table below)
4. Parsing resumes at whichever comes first: a `;` at the saved nesting depth, or the **start of the next assignment** — a `.` at the saved nesting depth followed by a byte that can begin an identifier
5. `recovery_count` (accessible via `bvnr_reader_get_recovery_count`) is incremented immediately when an error triggers entry into resync mode
6. The bytes discarded while recovering — from the byte the error was raised on up to, not including, the boundary recovery resumes at (the `;`, or the `.` that begins the next key) — are added to a running total, readable with `bvnr_reader_get_skipped_bytes`. `recovery_count` says how *often* recovery ran; this says what it *cost*, and it is the only way to learn it — the skipped bytes were never parsed, so no callback mentions them. A non-zero total means the document delivered to the callbacks is not the whole document. The converse does not hold: a value rejected right before its `;` skips zero bytes, so test `recovery_count`, not this, for "something was dropped" (§18.2)

Both boundaries matter, because they cover different errors. When the error is
*inside* a statement, that statement's own `;` is the next one, so the `;` rule
discards exactly the broken statement. When the error is *between* statements —
a stray byte in the whitespace separating two assignments — the next `;` belongs
to the following, perfectly good statement; resuming only there would discard it
whole, however large it is, so a single stray byte in front of a large struct
took the entire struct with it. The assignment boundary stops recovery at the
first point the document plausibly becomes readable again.

The `.` must be followed by a byte that can begin an identifier, so a `.` inside
skipped-over junk (`1.5`, `.5`, a `.` in binary corruption) is just another
skipped byte. A `.` inside a bracket opened *since* recovery began opens no
assignment either, and is skipped. Neither boundary recovers less than the other
alone would: `;` remains a boundary in every case it was one before. The other
side of the rule: a `.` followed by an identifier byte *anywhere* in the rest of
the broken statement is a boundary too — a reference such as `&.c`, or `x.y` —
so parsing restarts there, usually to report a further error, which
`recovery_count` counts.

**What recovery leaves open.** On entry the reader closes — with their
`ev_array_row_end` events — only the arrays opened *inside* the innermost open
struct, the ones the broken statement itself was in. An array that struct is an
*element of* stays open, with its row and element counts, so the struct's own
`}` and the array's `,` or `]` are read normally afterwards and the start/end
events a consumer sees always nest. A `}` that is itself the offending byte
(`.s = {.a = 1 };`, a member missing its `;`) is not discarded: it closes its
struct.

**An error inside a comment does not enter resync.** An invalid UTF-8 sequence
or a control byte in a comment is reported through `on_error` and counted —
`recovery_count` and the skipped-byte total each go up by one — and the comment
continues to its line end. Nothing is unwound and no value is lost, and the rest
of the comment is never read as document text.

**State machine behavior during resync:**

| Byte(s) | Action |
|---------|--------|
| `0x00–0xFF` (most) | Skip (consume and continue) |
| `"` | Enter `resync_string`: skip until matching `"` |
| `#` | Enter `resync_comment`: skip until newline |
| `[`, `{` | Increment its own counter — `resync_array_depth` or `resync_struct_depth` — for brackets opened since recovery began |
| `]`, `}` | If its own counter is non-zero, decrement it and keep skipping. Otherwise it closes a container that was open before the error — an array the innermost struct owns, or that struct — emitting its end event |
| `;` at depth 0 | Reset state, resume normal parsing |
| `.` at depth 0 | Candidate assignment start: resume if the next byte can begin an identifier, else keep skipping |

The two counters are independent: a `}` does not balance a junk `[`, so a
statement that opens a bracket it never closes keeps recovery from resuming
until the bracket is closed or the input ends (§13.3).

### 13.3 EOF in Resync

If EOF is reached while in any resync state, `error_got_incomplete_bvnr_stream` is fired as a **second** `on_error` notification in addition to the original error that triggered resync entry. Both error codes are delivered to the caller's `on_error` callback in order: the original error first, then `error_got_incomplete_bvnr_stream` when EOF is detected.

### 13.4 The Error Getters Under Recovery

The error getters of §16.7 describe the error that **ended** a read, not the
errors recovered from. A recovered error is reported through `on_error` and
nowhere else:

- **`bvnr_read` returned `false`:** the getters hold the error that stopped it.
  Under `continue_on_error` that is one of the errors recovery does not continue
  past (§13.2: `max_text_bytes`, `max_file_size`, an octet-stream framing error, a
  directive refused under `strict_version`, a callback that returned `false`) or the `error_got_incomplete_bvnr_stream` of
  §13.3, positioned at the end of the input.
- **`bvnr_read` returned `true`:** `bvnr_reader_get_error` returns `error_none`
  and the line, column, byte and offset getters return `0`, however many errors
  were recovered. Whether any were, and what they cost, is what
  `bvnr_reader_get_recovery_count` and `bvnr_reader_get_skipped_bytes` report.
  A consumer that needs the recovered errors themselves collects them in
  `on_error`.
- **During the read:** inside `on_error` the getters agree with the callback's
  arguments. After the callback returns they are unspecified until `bvnr_read`
  returns: the position is cleared when recovery begins, and the code may still
  hold the recovered error.

---

## 14. Formal EBNF

The complete grammar is maintained as a standalone file:

**[`doc/12_bovnar.ebnf`](12_bovnar.ebnf)**

The grammar uses ISO/IEC 14977:1996 notation and is derived from and verified against the reference implementation. It covers:

- Top-level stream and assignment structure
- Type annotations (seven core families: `uint`, `sint`, `float`, `float_fix`, `float_dec`, `utf8`, `bool`; plus `datetime` in spec 1.1)
- Value forms: numbers, special numbers, booleans, strings, symbols, references, arrays, structs, octet streams, inline unit suffixes
- Lexical primitives and UTF-8 byte class definitions
- Unit sub-grammar (SI/IEC prefixes, base units, compound units, exponents)
- Constraints not expressible in context-free EBNF (UTF-8 validity, BOM placement, nesting limits, type/value compatibility, error recovery behaviour)

---

## 15. Complete Examples

### 15.1 Simple Configuration

```bovnar
# Application configuration
.app_name = "Bovnar Demo";
.version = 1;
.debug = false;
.max_connections = 100;
.timeout_s = 30;
```

### 15.2 Typed Scientific Data

```bovnar
# Physical measurements with units
.measurements = [
    {.name = "temperature";
     .value = <float:32,°C> 23.5;
     .precision = <float:32,°C> 0.1;},
    {.name = "pressure";
     .value = <float:64,Pa> 101325;
     .precision = <float:64,Pa> 100;},
    {.name = "humidity";
     .value = <float:32> 0.45;
     .precision = <float:32> 0.01;},
    {.name = "wind_speed";
     .value = <float:32,m/s> 5.2;
     .precision = <float:32,m/s> 0.1;}
];

.calibration = <float:64,no_unit> 1.00042;
.density = <float:64,k~g/m³> 7800;
.accel = <float:64,m/s²> 9.81;
```

### 15.3 Inline Unit Suffix

The unit may be written directly after the value literal instead of — or redundantly alongside — the type annotation:

```bovnar
# No type annotation: inline unit supplies both type default and unit
.distance   = 1500 m;           # uint:64, no_unit → unit overridden to m
.speed      = 9.81 m/s;         # float:64, unit = m/s
.mass       = 70.5 k~g;         # float:64, unit = k~g

# Type annotation without unit: inline suffix supplies the unit
.dist       = <float:32> 1.5 k~m;

# Annotation and inline unit match: valid (redundant)
.pressure   = <float:64,Pa> 101325 Pa;

# Annotation and inline unit differ: error_unit_mismatch
# .bad      = <float:64,m> 1.5 s;    # ERROR
```

### 15.4 Binary Data with Octet Stream

<!-- bovnar-example: illustrative -->
```bovnar
# An image file embedded as binary: a 20-byte chunk (length 14 00), a 2-byte
# chunk (length 02 00), then the end marker. The line breaks and indentation
# between them are layout for this page only -- inside an octet stream every
# byte is a tag, a length or payload, and a newline there is out of sync.
.image = \x00
    \x01\x14\x00\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00
    \x01\x02\x00\xFF\xD9
    \x00;

# A checksum alongside
.checksum = <uint:_16> "abcd";
```

### 15.5 Arrays with Mixed Dimensions

```bovnar
# 2D matrix (uniform rows)
.matrix = [1, 2, 3]/[4, 5, 6];

# Uniform nested arrays (all inner arrays same size)
.uniform_nested = [[1,2],[3,4]];

# Multi-dimensional uniform array
.cube = [[1,2],[3,4]]/[[5,6],[7,8]];

# Array with typed nulls
.nullable = [<sint:16> 1, <sint:16> , <sint:16> 3];

# Sibling sub-arrays must match in length (homogeneity, §7.4):
.rect_nested = [[1,2],[3,4]];

# These produce error_array_row_size_mismatch:
# .bad1 = [1,2,3]/[4,5];           # /-row sizes differ: 3 vs 2
# .bad2 = [[1,2]/[3,4,5]];         # inner /-array's own rows differ: 2 vs 3
# .bad3 = [[1,2],[3,4,5]];         # ragged sibling sub-arrays differ: 2 vs 3
```

### 15.6 Deeply Nested Struct

```bovnar
.api_response = {
    .status = ok;
    .code = 200;
    .data = {
        .users = [
            {.id = 1; .name = "Alice"; .roles = ["admin", "user"];},
            {.id = 2; .name = "Bob"; .roles = ["user"];}
        ];
        .pagination = {
            .page = 1;
            .per_page = 50;
            .total = 2;
        };
    };
};
```

### 15.7 References and Symbols

```bovnar
# Configuration with references
.config = {
    .host = "api.example.com";
    .port = 443;
    .tls = true;
};

.endpoint_defaults = {
    .host = &.config.host;
    .port = &.config.port;
    .tls = &.config.tls;
};

# Using symbols as enum-like values
.status = ok;
.mode = readonly;
.direction = north;
```

### 15.8 Compound Unit Examples

```bovnar
# Velocity
.velocity = <float:64,m/s> 9.81;

# Acceleration
.acceleration = <float:64,m/s²> 9.81;

# Force (Newton = kg·m/s²)
.force = <float:64,k~g·m/s²> 9.81;

# Energy (Joule = kg·m²/s²)
.energy = <float:64,k~g·m²/s²> 100;

# Momentum (kg·m/s)
.momentum = <float:64,k~g·m/s> 5.0;

# Pressure (Pa = kg/(m·s²))
.pressure = <float:64,k~g/(m·s²)> 101325;

# Area density (kg/m²)
.area_density = <float:64,k~g/m²> 5.0;

# Electric field (V/m)
.electric_field = <float:64,V/m> 150;

# Magnetic flux density (T = kg/(A·s²))
.mag_flux_density = <float:64,k~g/(A·s²)> 0.5;

# Product form with asterisk
.moment = <float:64,m*s> 1.0;

# Alternative superscript notation for compound units
.force_alt = <float:64,k~g·m·s⁻²> 9.81;
```

### 15.9 Fixed-Point and Decimal Float Examples

```bovnar
# ── float_fix: fixed-point Q-format ────────────────────────────────

# 16-bit Q8: 8 fractional bits, resolution 2^-8 ≈ 0.00390625
# range: [-128.0, +127.99609375]
.adc_reading   = <float_fix:16,q8> 3.14;

# 32-bit Q16: 15 integer bits + sign + 16 fractional bits
.fine_angle    = <float_fix:32,q16> -1.5;

# 64-bit Q0: no fractional bits — pure integer in fixed-point shell
.sample_count  = <float_fix:64,q0> 4096;

# 32-bit Q8 with unit (meters/second at 1/256 resolution)
.velocity_fx   = <float_fix:32,q8,m/s> 9.81;

# null of a fixed-point type
.missing_fx    = <float_fix:16,q8> ;

# inline unit suffix also works with float_fix
.temperature   = <float_fix:32,q8> 23.5 °C;


# ── float_dec: IEEE 754-2008 decimal floating-point ─────────────────

# 32-bit decimal float (7 significant decimal digits)
.price         = <float_dec:32> 12.99;

# 64-bit decimal float with unit (16 significant decimal digits)
.pressure_dec  = <float_dec:64,Pa> 101325.0;

# 128-bit decimal float (34 significant decimal digits)
.pi_dec        = <float_dec:128> 3.14159265358979323846264338327950288;

# 256-bit decimal float (70 significant decimal digits)
.big_constant  = <float_dec:256> 1.4142135623730950488016887242096980785696718753769480731766797;

# Special values are accepted
.nan_dec       = <float_dec:64> nan;
.inf_dec       = <float_dec:32> inf;

# null of a decimal float type
.missing_dec   = <float_dec:64> ;


# ── Contrast with binary float ───────────────────────────────────────

# Binary IEEE float (existing)
.val_bin       = <float:64> 3.14;

# Decimal float — same text representation, different wire encoding
.val_dec       = <float_dec:64> 3.14;

# Fixed-point — same text representation, Q-format wire encoding
.val_fix       = <float_fix:32,q16> 3.14;
```

### 15.10 Error Examples

<!-- bovnar-example: rejected -->
```bovnar
# These will produce parse errors:

# Empty identifier
. = 42;                          # error_empty_identifier

# Type violation (type annotation on the identifier, not the value)
.x<utf8> = 42;                   # error_unexpected_input_byte (annotation must be after '=')

# Correct: .x = <utf8> "text";

# Type violation – value doesn't match type
.x = <utf8> 42;                   # error_type_value_mismatch

# Value out of range
.y = <uint:8> 300;                # error_value_out_of_range

# Negative unsigned
.z = <uint:8> -1;                 # error_value_out_of_range

# Unmatched struct close — a '}' at the top level (struct nesting 0)
.stray_close = 1;}               # error_illegal_struct_close

# Unknown escape
.string = "\x";                  # error_illegal_escape_sequence

# error_array_row_size_mismatch — /-dimension rows of one array, and (since 1.0)
# ragged sibling sub-arrays, must match in length:
# .bad1 = [1,2,3]/[4,5];           # /-row sizes differ: 3 vs 2
# .bad2 = [[1,2]/[3,4,5]];         # inner /-array's own rows differ: 2 vs 3
# .bad3 = [[1,2],[3,4,5]];         # ragged sibling sub-arrays differ: 2 vs 3
# Uniform /-rows and rectangular sibling sub-arrays are valid:
.ok1 = [1,2,3]/[4,5,6];    # valid — both dimension rows have 3 elements
.ok2 = [[1,2],[3,4]];       # valid — rectangular sub-arrays

# Comma outside array
.comma_outside = 42,;            # error_unexpected_input_byte

# Non-decimal base — bare token is parsed as a symbol, causing type mismatch
.hex = <uint:_16> ff;             # error: symbol value for numeric type

# Empty component in compound unit
.x = <float:64,m//s> 1.0;       # error_unit_illegal

# Too many components (> 32 — see §11.7)
.y = <float:64,m*s*g*A*K*mol*cd*b*V*Hz*N*Pa*J*W*Ω*F*C*S*Wb*T*H*lm*lx*Bq*Gy*kat*L*min*h*d*bar*eV*Da> 1.0;
                                            # error_unit_illegal (33 components)

# float_fix: Q >= effective width
.bad_q = <float_fix:16,q16> 1.0;            # Q=16 >= width=16 → error_illegal_value_type

# float_fix: invalid width
.bad_fw = <float_fix:8,q4> 1.0;             # width 8 not in {0,16,32,64,128,256}

# float_fix: base param forbidden
.bad_fb = <float_fix:32,q8,_10> 1.0;        # error_illegal_value_type

# float_dec: invalid width
.bad_dw = <float_dec:24> 1.0;               # width 24 not in {0,16,32,64,128,256}

# float_dec: base param forbidden
.bad_db = <float_dec:64,_10> 1.0;           # error_illegal_value_type

# q-param on non-float_fix type
.bad_qu = <float:64,q8> 1.0;               # q-param only valid for float_fix
```

---

## 16. Reference API

### 16.1 Core Types

```c
typedef enum bvnr_event_e {
    ev_stream_start,
    ev_assignment_start,
    ev_octet_stream_start,
    ev_octet_stream_end,
    ev_struct_start,
    ev_struct_end,
    ev_array_row_start,
    ev_array_row_end,
    ev_array_dim_start,
    ev_data,
    ev_type_annotation_start,
    ev_type_annotation_end,
    ev_type_annotation_type_family,
    ev_type_annotation_type_family_parameter,
    ev_stream_end
} bvnr_event_t;

typedef enum value_type_family_e {
    vt_plain,
    vt_utf8,
    vt_sint,
    vt_uint,
    vt_float,
    vt_float_fix,  /* fixed-point binary, Q-format; Q stored in value_type_spec_t.base */
    vt_float_dec,  /* IEEE 754-2008 decimal floating-point                               */
    vt_bool,       /* boolean (true/false/on/off); see §4.4 and §6.1                     */
    vt_datetime,   /* spec 1.1 — timestamp: signed epoch-seconds; see §5                  */
    vt_illegal
} value_type_family_t;

typedef struct value_type_spec_s {
    value_type_family_t family;
    uint32_t            width; /* bit-width; 0 = default (64)                         */
    uint32_t            base;  /* for uint/sint/float: numeral base; 0 = default (10) */
                               /* for float_fix:        Q (fractional bits)           */
                               /* for float_dec:        unused (always 0)             */
                               /* for datetime:         epoch index (0 = unix)        */
} value_type_spec_t;

#define BVNR_MAX_UNIT_COMPONENTS     32

typedef struct value_unit_prefix_s {
    prefix_system_t system;
    union {
        si_prefix_id_t  si;
        iec_prefix_id_t iec;
    } id;
} value_unit_prefix_t;

typedef struct value_unit_component_s {
    value_base_unit_t   base;
    unit_exponent_t     exponent;
    value_unit_prefix_t prefix;
} value_unit_component_t;

typedef struct value_unit_s {
    uint32_t                num_components;
    value_unit_component_t  components[BVNR_MAX_UNIT_COMPONENTS];
} value_unit_t;

typedef enum token_type_e {
    token_is_identifier,
    token_is_string,
    token_is_number,
    token_is_symbol,
    token_is_reference,
    token_is_array_number,
    token_is_array_string,
    token_is_type,
    token_is_octet_stream,
    token_is_null_value,
    token_is_structure,
    token_is_unit,
    token_is_type_width,
    token_is_type_base,
    token_is_type_q,
    token_is_bool,
    token_is_unknown
} token_type_t;

typedef struct bvnr_data_s {
    token_type_t      type;
    value_type_spec_t value_type;
    value_unit_t      value_unit;
    const void*       data;
    uint32_t          length;
    const void*       frac_data;    /* spec 1.1 — ISO datetime sub-second digits, else NULL */
    uint32_t          frac_length;  /* spec 1.1 — length of frac_data, else 0 */
    const void*       unit_source;        /* library 2.0 — the unit as the document spelled it, else NULL */
    uint32_t          unit_source_length; /* library 2.0 — length of unit_source, else 0 */
    bool              converted;    /* a want_unit read-time conversion was applied (read/write API §1.10) */
    bvnr_converted_t  conv;         /* the exact converted value (unit + text + rational); zeroed when converted is false */
} bvnr_data_t;
```

### 16.2 Type Construction Macros

```c
/* Type-spec convenience constructors (from bovnar.h) */
#define BVN_TYPE_PLAIN          ((value_type_spec_t){ .family = vt_plain, .width = 0,  .base = 0  })
#define BVN_TYPE_UTF8           ((value_type_spec_t){ .family = vt_utf8,  .width = 0,  .base = 0  })
#define BVN_TYPE_BOOL           ((value_type_spec_t){ .family = vt_bool,  .width = 0,  .base = 0  })
#define BVN_TYPE_UINT(w)        ((value_type_spec_t){ .family = vt_uint,      .width = (w) })
#define BVN_TYPE_SINT(w)        ((value_type_spec_t){ .family = vt_sint,      .width = (w) })
#define BVN_TYPE_FLOAT(w)       ((value_type_spec_t){ .family = vt_float,     .width = (w) })
/* float_fix: .base is repurposed to store Q (fractional bits). */
#define BVN_TYPE_FLOAT_FIX(w,q) ((value_type_spec_t){ .family = vt_float_fix, .width = (w), .base = (q) })
/* float_dec: base field is unused (always 0).                   */
#define BVN_TYPE_FLOAT_DEC(w)   ((value_type_spec_t){ .family = vt_float_dec, .width = (w) })
/* With explicit numeral base:                                   */
#define BVN_TYPE_UINT_BASE(w,b) ((value_type_spec_t){ .family = vt_uint, .width = (w), .base = (b) })
#define BVN_TYPE_SINT_BASE(w,b) ((value_type_spec_t){ .family = vt_sint, .width = (w), .base = (b) })
/* float: base 10 or 16 only (read/write API §2.14)                */
#define BVN_TYPE_FLOAT_BASE(w,b) ((value_type_spec_t){ .family = vt_float, .width = (w), .base = (b) })
```

### 16.3 Unit Macros

```c
#define BVN_UNIT_NO_PREFIX(b) \
    ((value_unit_t){ \
        .num_components = 1, \
        .components = {{ \
            .base = (b), .exponent = exp_linear, \
            .prefix.system = prefix_si, .prefix.id.si = si_none \
        }} \
    })

#define BVN_UNIT_SI(b, p) \
    ((value_unit_t){ \
        .num_components = 1, \
        .components = {{ \
            .base = (b), .exponent = exp_linear, \
            .prefix.system = prefix_si, .prefix.id.si = (p) \
        }} \
    })

#define BVN_UNIT_IEC(b, p) \
    ((value_unit_t){ \
        .num_components = 1, \
        .components = {{ \
            .base = (b), .exponent = exp_linear, \
            .prefix.system = prefix_iec, .prefix.id.iec = (p) \
        }} \
    })

#define BVN_UNIT_SI_EXP(b, p, e) \
    ((value_unit_t){ \
        .num_components = 1, \
        .components = {{ \
            .base = (b), .exponent = (e), \
            .prefix.system = prefix_si, .prefix.id.si = (p) \
        }} \
    })

#define BVN_UNIT_NONE \
    ((value_unit_t){ .num_components = 0 })

/* Compound-unit helper: two SI-prefixed components */
#define BVN_UNIT_COMPOUND2(b1, p1, e1, b2, p2, e2) \
    ((value_unit_t){ \
        .num_components = 2, \
        .components = { \
            { .base = (b1), .exponent = (e1), \
              .prefix.system = prefix_si, .prefix.id.si = (p1) }, \
            { .base = (b2), .exponent = (e2), \
              .prefix.system = prefix_si, .prefix.id.si = (p2) } \
        } \
    })
```

### 16.4 Reader Setup

```c
typedef struct bvnr_read_flags_s {
    uint16_t  max_identifier_length;  // 0 → 255 default
    uint16_t  max_string_length;      // 0 → 65535 default
    uint16_t  max_number_length;      // 0 → 65535 default
    uint16_t  max_symbol_length;      // 0 → 255 default
    uint16_t  max_reference_length;   // 0 → 65535 default
    uint64_t  max_array_items;        // 0 → 2 147 483 647 internal default
    uint64_t  max_text_bytes;         // 0 → 2 147 483 647 internal default
    uint64_t  max_file_size;          // 0 → no total-byte cap (default; text still bounded by max_text_bytes); 16 777 216 recommended for a cap
    uint8_t   max_struct_nesting;     // 0 → 64 internal default; hard cap 255
    uint8_t   max_array_nesting;      // 0 → 64 internal default; hard cap 255
    void*     userdata;
    bool    (*on_unverified)(void*, bvnr_event_t, bvnr_data_t*);
    bool    (*on_verified)(void*, bvnr_event_t, bvnr_data_t*);
    bool      continue_on_error;
    bvnr_on_error_fn on_error;
    bool      strict_version;                 // reject a declared spec version newer than this build
    bool      text_only;                      // refuse a document containing an octet stream (§16.10 code 51)
    bool      want_unit_allow_nonterminating; // deliver a non-terminating exact conversion as a rational
    uint32_t  max_conversion_length;          // 0 → 1024; longest want_unit conversion text, in chars
    bool    (*want_unit)(void*, const bvnr_data_t*, value_unit_t*, uint32_t*);  // read-time lossless unit/base conversion hook
    uint64_t  _reserved[2];
} bvnr_read_flags_t;
```

### 16.5 Source/Sink Creation

```c
void bvnr_source_from_fd(bvnr_source_t* s, int fd);
void bvnr_source_from_mem(bvnr_source_t* s, const void* buf, uint64_t len);
void bvnr_sink_to_fd(bvnr_sink_t* s, int fd);
void bvnr_sink_to_mem(bvnr_sink_t* s, void* buf, uint64_t cap);
uint64_t bvnr_sink_bytes_written(const bvnr_sink_t* s);
```

`bvnr_sink_bytes_written` queries the total bytes written to a memory sink created with `bvnr_sink_to_mem` **by the caller**; `bvnr_open_write_sink` copies the sink, so writer output never advances the caller's struct and this stays 0. For writer output use `bvnr_writer_bytes_written` (read/write API §2.4, §2.10).

### 16.6 Reading

```c
bool bvnr_open_read_source(bvnr_reader_t* r, const bvnr_source_t* src,
                        const bvnr_sink_t* src_mirror,
                        bvnr_read_flags_t* options);

bool bvnr_open_read_mem(bvnr_reader_t* r, const void* buf, uint64_t len,
                        void* mirror_buf, uint64_t mirror_cap,
                        bvnr_read_flags_t* options);

bool bvnr_read(bvnr_reader_t* r);

bvnr_reader_t* bvnr_reader_create(void);   /* the reader is opaque */
void           bvnr_reader_destroy(bvnr_reader_t* r);
bool bvnr_reader_get_declared_version(const bvnr_reader_t* r,
                                      uint16_t* major, uint16_t* minor); /* §3.4 */

uint32_t    bvnr_version(void);            /* library version, §17 */
const char* bvnr_version_string(void);
void        bvnr_spec_version(uint16_t* major, uint16_t* minor);
```

### 16.7 Error Queries

```c
error_code_t bvnr_reader_get_error(const bvnr_reader_t* r);
uint64_t     bvnr_reader_get_error_line  (const bvnr_reader_t* r);
uint64_t     bvnr_reader_get_error_column(const bvnr_reader_t* r);
uint32_t     bvnr_reader_get_error_byte  (const bvnr_reader_t* r);
uint64_t     bvnr_reader_get_error_offset(const bvnr_reader_t* r);
uint64_t     bvnr_reader_get_recovery_count(const bvnr_reader_t* r);
uint64_t     bvnr_reader_get_skipped_bytes(const bvnr_reader_t* r);
const char*  bvn_error_to_string(error_code_t code);
```

The getters describe the error that ended the read. After a read that returned
`true` they report `error_none` and position `0`, even if errors were recovered
under `continue_on_error` (§13.4).

### 16.8 Utility Functions

```c
bool bvn_validate_identifier(const char* id);
bool bvn_validate_symbol(const char* surr);
bool bvn_validate_reference(const char* link);
bool bvn_validate_number(const char* s);
bool bvn_validate_string(const uint8_t* data, size_t len);

bool bvn_is_special_number_string(const char* s);
bool bvn_validate_digits_for_base(const char* s, uint32_t base);
bool bvn_validate_number_in_base(const char* s, uint32_t base);

bool bvn_validate_uint_range(const char* s, uint32_t w, uint32_t base);
bool bvn_validate_sint_range(const char* s, uint32_t w, uint32_t base);

uint32_t bvn_char_to_digit(uint32_t c, uint32_t base);
uint32_t bvn_min_digits_for_type(value_type_spec_t vt);

int32_t bvn_format_uint64(char* buf, size_t bufsize,
                           uint64_t value, uint32_t base, uint32_t min_digits);
int32_t bvn_format_int64(char* buf, size_t bufsize,
                          int64_t value, uint32_t base, uint32_t min_digits);
int32_t bvn_format_double(char* buf, size_t bufsize,
                           double value, value_type_spec_t vt);

bool bvn_parse_int64(const char* s, value_type_spec_t vt, int64_t* out);
bool bvn_parse_uint64(const char* s, value_type_spec_t vt, uint64_t* out);
bool bvn_parse_double(const char* s, value_type_spec_t vt, double* out);
bool bvn_parse_double_in_base(const char* s, uint32_t base, double* out);
bool bvn_looks_like_double(const char* s);

/* Parse a NUL-terminated unit string into a value_unit_t.
   Sets *ok to false on error. */
value_unit_t bvn_parse_unit(const uint8_t* unit, bool* ok);

/* Length-bounded variant; does not require a NUL terminator. */
value_unit_t bvn_parse_unit_n(const uint8_t* unit, uint32_t len, bool* ok);

/* Serialize a value_unit_t (possibly compound) back to a string.
   Numerator components are joined by "·", followed by "/" and
   denominator components joined by "·"; when every exponent is
   negative there is no "/" (m⁻¹·s⁻²).  Returns bytes written, or -1
   on buffer overflow, an invalid unit, or an opaque unit with no
   text. */
int32_t bvn_unit_to_string(value_unit_t u, char* buf, size_t bufsize);

/* Extended variant accepting bvn_unit_flags_t:
     BVN_UNIT_FLAGS_NONE  (0)      – Unicode superscript exponents, no reduction
     BVN_UNIT_ASCII_EXP  (1 << 1) – ^N caret exponents and "*" for "·"
     BVN_UNIT_REDUCE     (1 << 0) – reduce compound unit before serialising
     BVN_NUM_CANONICAL   (1 << 2) – see bovnar.h
   Flags may be OR-combined.  Returns bytes written, or -1 on overflow. */
int32_t bvn_unit_to_string_ex(value_unit_t u, char* buf, size_t bufsize,
                               bvn_unit_flags_t flags);

/* Returns true if every component in u has a valid exponent (not
   exp_invalid), a known base unit, and a prefix legal for that base
   unit per bvn_prefix_unit_valid.  Both serialisation functions call
   this predicate internally before writing. */
bool bvn_unit_valid(value_unit_t u);

/* Structural equality of two units: same num_components and the same set
   of components (matching base, exponent, and prefix).  The comparison is
   ORDER-INSENSITIVE — unit multiplication is commutative, so components are
   matched as multisets and "N·m" equals "m·N".  This is the comparison the
   validator uses to match an inline unit suffix against a type-annotation
   unit (error_unit_mismatch on disagreement).  For dimensional equivalence
   — units that measure
   the same physical quantity but differ in spelling or factoring (e.g.
   W vs VA, or the two no_unit forms) — use bvn_units_compatible from
   bovnar_si_units.h (§11.8) instead. */
bool bvn_unit_equal(value_unit_t a, value_unit_t b);

/* Compute the combined prefix factor across all components (ignoring
   base-unit conversion factors).  Each component's prefix factor is
   raised to |exponent| and multiplied together; denominator components
   are inverted. */
double bvn_unit_prefix_factor(value_unit_t u);

/* Compute the combined prefix exponent (sum of prefix_base_exponent ×
   |unit_exponent| across all components, negated for denominator
   components). */
int32_t bvn_unit_prefix_exponent(value_unit_t u);

const uint8_t* bvn_get_escape_repl_table(void);
```

### 16.9 Typed Write Helpers

Convenience functions that emit a type annotation + value in one call.
All functions return `false` on serialisation error.

```c
/* ── Plain scalar writers ──────────────────────────────────────────── */
bool bvnr_write_string(bvnr_writer_t* w, const char* key, const char* value);
bool bvnr_write_plain (bvnr_writer_t* w, const char* key, const char* value);
bool bvnr_write_null  (bvnr_writer_t* w, const char* key);
bool bvnr_write_bool  (bvnr_writer_t* w, const char* key, bool value);

/* ── Integer writers ───────────────────────────────────────────────── */
bool bvnr_write_uint(bvnr_writer_t* w, const char* key,
                     uint32_t width, uint64_t value);
bool bvnr_write_sint(bvnr_writer_t* w, const char* key,
                     uint32_t width, int64_t value);

/* ── Binary float writers ─────────────────────────────────────────── */
bool bvnr_write_float(bvnr_writer_t* w, const char* key,
                      uint32_t width, double value);

/* ── Fixed-point writers (float_fix) ─────────────────────────────── */
/* q = number of fractional bits (Q parameter). */
bool bvnr_write_float_fix(bvnr_writer_t* w, const char* key,
                           uint32_t width, uint32_t q, double value);

/* ── Decimal float writers (float_dec) ───────────────────────────── */
bool bvnr_write_float_dec(bvnr_writer_t* w, const char* key,
                           uint32_t width, double value);

/* ── Writers with explicit unit ───────────────────────────────────── */
bool bvnr_write_uint_unit     (bvnr_writer_t* w, const char* key,
                                uint32_t width, uint64_t value, value_unit_t unit);
bool bvnr_write_sint_unit     (bvnr_writer_t* w, const char* key,
                                uint32_t width, int64_t value, value_unit_t unit);
bool bvnr_write_float_unit    (bvnr_writer_t* w, const char* key,
                                uint32_t width, double value, value_unit_t unit);
bool bvnr_write_float_fix_unit(bvnr_writer_t* w, const char* key,
                                uint32_t width, uint32_t q,
                                double value, value_unit_t unit);
bool bvnr_write_float_dec_unit(bvnr_writer_t* w, const char* key,
                                uint32_t width, double value, value_unit_t unit);

/* ── Wide-precision float writers (bvn_float_t) ─────────────────── */
/* width: the annotation width, any legal float width; 0 = use       */
/* f->_prec, which must then itself be a legal float width.          */
bool bvnr_write_bvnf     (bvnr_writer_t* w, const char* key,
                           const bvn_float_t* f, uint32_t width);
bool bvnr_write_bvnf_unit(bvnr_writer_t* w, const char* key,
                           const bvn_float_t* f, uint32_t width, value_unit_t unit);

/* ── Struct helpers ───────────────────────────────────────────────── */
bool bvnr_write_struct_start(bvnr_writer_t* w, const char* key);
bool bvnr_write_struct_end  (bvnr_writer_t* w);
```

### 16.10 Error Codes

```c
typedef enum error_code_e {
    error_none                          = 0,
    error_unknown_token_type            = 1,
    error_array_row_size_mismatch       = 2,
    error_identifier_too_long           = 3,
    error_empty_identifier              = 4,
    error_struct_nesting_too_high       = 5,
    error_array_nesting_too_high        = 6,
    error_illegal_struct_close          = 7,
    error_string_too_long               = 8,
    error_illegal_escape_sequence       = 9,
    error_number_too_long               = 10,
    error_symbol_too_long               = 11,
    error_reference_too_long            = 12,
    error_read_complete_chunk_failed    = 13,
    error_octet_stream_out_of_sync      = 14,
    error_unexpected_input_byte         = 15,
    error_text_data_too_long            = 16,
    error_reading_from_source_fd        = 17,
    error_got_incomplete_bvnr_stream    = 18,
    error_invalid_utf8_byte             = 19,
    error_invalid_byte_order_mark       = 20,
    error_type_too_long                 = 21,
    error_unit_too_long                 = 22,
    error_expected_string_in_array      = 23,  /* reserved; never set by the library */
    error_expected_number_in_array      = 24,  /* reserved; never set by the library */
    error_illegal_value_type            = 25,
    error_scanner_callback_failed       = 26,
    error_file_too_long                 = 27,
    error_invalid_argument              = 28,
    error_too_many_array_items          = 29,
    error_writing_to_sink               = 30,
    error_sink_buffer_exhausted         = 31,
    error_unit_illegal                  = 32,
    error_base_requires_string_literal  = 33,
    error_type_value_mismatch           = 34,
    error_value_out_of_range            = 35,
    error_digit_not_in_base             = 36,
    error_recovered                     = 37,  /* reserved; never set by the library */
    error_unit_mismatch                 = 38,
    /* Array element homogeneity (spec 1.0): every non-null element of an array
     * must share the same kind, and every element of a BARE array the same UNIT
     * (§7.4 -- not merely the same dimension, so m beside ft and °C beside K are
     * both refused); sibling sub-arrays must match in length and element shape
     * (recursively); sibling structs must share the same keys with
     * recursively-matching fields, whose units stay free. */
    error_array_element_type_mismatch   = 39,
    error_struct_shape_mismatch         = 40,
    /* A struct (or the top-level document) repeats a key. Keys must be unique
     * within one scope so lookup, references and iteration always agree. */
    error_duplicate_struct_key          = 41,
    /* spec 1.1 — a leading "#!bovnar …" directive is present but malformed. */
    error_invalid_spec_version          = 42,
    /* spec 1.1 — the declared version exceeds what the reader supports and
     * strict_version was set. */
    error_unsupported_spec_version      = 43,
    /* spec 1.1 — a \u{…} escape names a non-scalar value (a surrogate, or a
     * code point above U+10FFFF). */
    error_invalid_codepoint             = 44,
    /* spec 1.1 — an ISO-8601 datetime literal is malformed or has an
     * out-of-range field (bad width, separator, month/day/time component). */
    error_invalid_datetime_literal      = 45,
    /* spec 1.1 — an ISO-8601 literal was given for an atomic GNSS epoch
     * (gps/galileo/glonass/beidou), which has no round-trippable inverse. */
    error_datetime_literal_unsupported_epoch = 46,
    /* a lossless read-time unit/base conversion (want_unit) could not be
     * performed: an irrational factor, or a non-terminating expansion. */
    error_unit_inexact                       = 47,
    /* a multiplexed message was still short when its octet stream ended. */
    error_octet_stream_truncated             = 48,
    /* spec 1.2 (§11.9) — the namespace before the ':' is not a unit
     * profile this build supports. Distinct from error_unit_illegal so a
     * producer can tell "no such profile compiled in" from "malformed unit". */
    error_unit_profile_unknown               = 49,
    /* spec 1.2 (§11.9) — valid in its vocabulary, but with no
     * representation in this unit system: a reference level, a scale factor
     * outside the SI prefix decades, an expression wider than
     * BVNR_MAX_UNIT_COMPONENTS, or a construct the unit slot cannot reach at
     * all (UDUNITS reference time). */
    error_unit_profile_unsupported           = 50,
    /* the document contains an octet stream and the reader was opened with
     * text_only. NOT a defect in the document — an octet stream is a
     * first-class part of the format (§9) — but an assertion by the CONSUMER
     * that this particular channel carries text, made at the door rather than
     * discovered as corruption downstream. */
    error_octet_stream_forbidden             = 51,
    /* whitespace split a type-annotation PARAMETER in two (§5.3). Every `ws` the
     * grammar allows inside an annotation sits beside a separator; in the middle
     * of a parameter there is no production for it, and the lexer used to drop it
     * anyway -- so "<float:64,m in>" was accepted as the minute and "<uint:6 4>"
     * as a 64-bit width. Reported at the first byte after the whitespace. A byte
     * that is illegal anywhere in an annotation is still
     * error_unexpected_input_byte. */
    error_type_param_whitespace              = 52
} error_code_t;
```

---

## 17. Versioning & Stability

Bovnar follows semantic versioning of the **format**, independent of any
implementation's version.

**Implementation version.** The reference implementation exposes its own
version, and it is a different number from the format's. Through 1.1.0 the two
moved together; they parted at library **2.0.0**, which broke the ABI and the
Python API while the format it reads grew only additively, to spec **1.2**. The
library's major counts breaks in what a *program* is built against; the format's
major counts breaks in what a *document* means, and has not moved. The C header
defines `BVNR_VERSION_MAJOR`, `BVNR_VERSION_MINOR`, `BVNR_VERSION_PATCH`, the
comparable integer `BVNR_VERSION` (`major*10000 + minor*100 + patch`, so
`#if BVNR_VERSION >= 20000` tests "≥ 2.0.0"), and `BVNR_VERSION_STRING`
(`"2.0.0"`). The Python package mirrors this as `bovnar.__version__`.
Separately, `BVNR_SPEC_VERSION_MAJOR` / `BVNR_SPEC_VERSION_MINOR` name the
highest **spec** version the build understands (`1.2`); `bvnr_version()`, `bvnr_version_string()` and
`bvnr_spec_version()` expose both at runtime.

**Declaring a document's version.** Since 1.1 a document may state which spec
version it targets with a leading `#!bovnar <major>.<minor>` directive (§3.4).
Because the directive is lexically a comment, this is fully backward compatible:
a 1.0 reader ignores it. A 1.1+ reader records it
(`bvnr_reader_get_declared_version`) and, in `strict_version` mode, rejects a
version it does not support.

**What 1.0 freezes.** The grammar is stable. A `.bvnr` document that is valid
under spec 1.0 will remain valid, and will decode to the same values, under every
1.x revision — with the exceptions this section lists by name and no others:
the corrections made in 1.2, below, each of which withdraws something the text
of 1.0 already excluded or contradicted itself over. This covers the lexical structure, the type families and their
annotations, arrays (including the homogeneity rules of §7.4, as corrected in
1.2 — see below), structs, octet
streams, references, and the error-code values in §16.10. Conforming archives
may rely on this for long-term storage.

**What may still grow in 1.x (additive only).** The following may be *extended*
without breaking existing documents, and such extensions ship as minor (1.x)
revisions:

- the **unit and currency tables** — new physical units, prefixes, and ISO 4217 /
  crypto currency codes may be added (a document never depends on a code being
  *absent*);
- the **accepted input spellings** of a unit — new aliases, and the compact
  prefix form (`kg` for `k~g`, §11.4). This is the same
  additive direction as a new unit: a spelling is only ever accepted where the
  parser previously raised `error_unit_illegal`, the base symbol is still
  matched as the longest alias suffix so no existing spelling changes meaning,
  and the **canonical output form is unaffected** — `bvn_unit_to_string` and a
  writer handed no spelling keep emitting the separated `k~g`, and
  `bovnar pretty-print --canonical` writes no compact spelling — a prefix is
  either folded into the value or written with its `~` — so a document can be
  made readable to a reader that predates the spelling, except where an
  integer value cannot take the rescale. (A plain re-serialisation keeps the
  spelling the document was written in.);
- **new constructs gated on the version that introduced them** — available only
  to a document whose `#!bovnar` directive declares that version or a newer
  one, and an error in any other, so no existing document can acquire one by
  accident. This is how both minor revisions so far were delivered: 1.1 added
  the `datetime` family, the `\x`/`\u{}` escapes and reference array indexing;
  1.2 added the unit-profile notation (§11.9);
- **new error codes** appended after the current maximum (existing numeric values
  never change). The five the *grammar* of 1.1 required are
  `error_invalid_spec_version` (42), `error_unsupported_spec_version` (43),
  `error_invalid_codepoint` (44), `error_invalid_datetime_literal` (45), and
  `error_datetime_literal_unsupported_epoch` (46) — those carry a `spec 1.1`
  note in §16.10. Codes appended since: 47 and 48 for facilities beside the
  wire format, 49 and 50 for the spec 1.2 unit profiles, 51 for a reader
  policy, and 52 for a lexical rule the grammar always implied (see the
  corrections below). **§16.10 is the list**, and the current
  maximum is wherever it ends — never a number quoted somewhere else, this
  paragraph included;
- the **optional version directive** (§3.4), added in 1.1: it is an ordinary
  comment to any 1.0 reader, so adding one never invalidates a document;
- new optional reader/writer flags and limits whose defaults preserve current
  behaviour.

A reader from an older 1.x point release may not recognise a unit or currency
added in a newer one; that is the expected direction of forward compatibility and
is not a break of the 1.0 promise.

**Two corrections made in 1.2, stated because they bend the promise in *What 1.0 freezes*.**
Both refuse documents that a 1.1 reader accepted, and both apply to every
document, whatever it declares. They are recorded as corrections of defects
rather than as changes of the format, on these grounds:

- *Whitespace inside a type-annotation parameter* is
  `error_type_param_whitespace` (§5.3). The grammar never derived it — every
  `ws` in an annotation sits beside a separator — but the reference reader
  deleted it, so `<float:64,m in>` was read as `min` and `<uint:6 4>` as 64
  bits. A document relying on that carried a unit or a type its author did not
  write.
- *A bare array is homogeneous in its unit, not merely its dimension* (§7.4).
  The rule as written through 1.1 let `[1 m, 2 ft]`-shaped arrays through
  while the same section promised that a consumer may treat the elements
  identically; a consumer who believed it computed wrong numbers. The promise
  was kept and the rule corrected to make it true.

Neither of the two changes what any accepted document *means*. A producer
affected by either was already emitting data that could not be read correctly.

**Reader defects fixed in the same release.** Beside those two, the reference
reader of library 2.0.0 (which implements 1.2) stopped letting through — or misreading — input the
specification already excluded. They are listed because a document that
depended on one behaves differently. Most of them refuse what was let through.
A few change what an accepted document reads as — the leap second, the zero
mantissa, the element that kept no unit, the path into a nested matrix and the
conversion factors — and each of those entries says so:

- *A `tai` datetime literal spelling a known leap second*
  (`2016-12-31T23:59:60Z`) has its own value, one less than before
  (1861920036, was 1861920037); through 1.1 it collapsed onto the second after
  it. Civil epochs are unchanged.
- *A zero mantissa under an exponent too long to count is zero.*
  `0e999999999999999999999` was delivered as +infinity by the number helpers
  (`bvn_parse_double`, and so `query` and `convert`); it is 0. The document's
  digits are what they were.
- *In an array under a unit annotation, an element annotated without a unit*
  (`<float:64,s>[<float:64> 1, 3]`) no longer strips the unit from the elements
  after it. The `3` is in seconds again — and the document tier then refuses the
  array as mixed.
- *A byte-order mark after leading whitespace* is
  `error_invalid_byte_order_mark` (§3.2 always made offset 0 the only legal
  place; this one was accepted).
- *A version directive ended by end-of-input* is parsed like one ended by a
  newline, so a malformed one is `error_invalid_spec_version`.
- *An array left open at the `;`* (`.a = [1;`) is
  `error_got_incomplete_bvnr_stream`; it was accepted, and absorbed the next
  assignment.
- *A struct inside an array is a scope of its own.* A `,` after a member's
  value (`.f = [{.x = 7, 1;}];`) is `error_unexpected_input_byte`; it was
  accepted, and the member read as `71`. And an array left open at a `;` is
  detected inside a struct as well, after an array nested in it has closed
  (`.p = {.s = [{.a = [1];}; .q = 1;];};`). A `]` is held to the same rule:
  `.a = [{.x = 1];};` closed the array from inside its own element and was
  accepted; it is `error_unexpected_input_byte`.
- *The width of a `/`-row counts each element once* (§7.3). A nested array —
  an element, or an array that is only a member of a struct element — was
  counted twice in the row that holds it. So `[[1]]/[null,null]` (one element
  against two) was accepted and is `error_array_row_size_mismatch`; and
  `[[1],[2]]/[null,null]` and `[{.m = [1];}]/[{.m = null;}]` (equal widths)
  were refused and are valid. This is one of three entries here that also
  *accept* documents a 1.1 reader refused — the others are the nested
  `/`-matrices under homogeneity and the padded version directive. An array in which every row is made of
  arrays, or no row holds one, compared correctly before and is unchanged.
- *`float_fix` and `float_dec` are one keyword each.* Whitespace or a comment
  before the suffix (`<float _fix:32,q8>`) is `error_type_param_whitespace`,
  for the reason given above for whitespace inside a parameter; it was read
  as `float_fix`.
- *After a reference index only `.`, `[` or the end of the reference may
  follow* (§4.5). `&.a[0]b` is `error_unexpected_input_byte`; it was accepted
  as the path `.a[0]b`, which the grammar does not derive and no lookup
  resolves.
- *A comment that ends at end-of-input inside a multi-byte UTF-8 sequence* is
  `error_invalid_utf8_byte` (§3.1), as the same bytes followed by a newline
  always were.
- *Quoted digits are range-checked whatever they spell.* In base 24 and up
  `n`, `a`, `i` and `f` are digits, and a quoted `"nan"`, `"inf"` or `"ninf"`
  under `uint`/`sint` was exempted from the range check as though it were the
  keyword: `<uint:8,_36> "inf"` (24171) is `error_value_out_of_range`. The
  bare keywords keep the exemption of §6.4.
- *Homogeneity (§7.4) no longer depends on element order.* The document tier
  compared every element with the first non-null one, so a hole in that one
  hid whatever stood at the same position in the others:
  `[[null], [1], ["x"]]` and `[{.x = null;}, {.x = 1;}, {.x = "s";}]` were
  accepted — as was a matrix whose rows carry different units behind a leading
  row of nulls — while the same elements in another order were refused. They
  are `error_array_element_type_mismatch` in every order. In the other
  direction, nested `/`-matrices such as `[[[1,1]/[2,2], null], [null, null]]`
  were refused as ragged through a row count shared between array scopes, and
  are valid. The same shared count made a path into such a matrix resolve to
  the wrong element: in `[[1]/[2]] / [[3]/[4]]`, `[1][0][0][0]` answered 1 and
  answers 3.
- *A version directive may be padded with any number of spaces or tabs*
  (§3.4). One with more than 21 between `#!bovnar` and the version was
  `error_invalid_spec_version`, and is a directive.
- *`utf8` and `bool` take no parameter, a width of zero included* (§5.1).
  `<utf8:0>` and `<bool:0>` were accepted, because the check looked at the
  width's value and not at its presence; they are `error_illegal_value_type`,
  as `<utf8:8>` always was.
- *A struct takes no annotation* (§5.1). One before a struct with members was
  always refused — at the first member's key — which left `<utf8> {}`,
  `<uint:8> {}` and `<uint:8> [{}]` accepted. Every struct under an annotation
  is `error_type_value_mismatch`, reported at its `{`.
- *`max_array_items` limits an array of structs.* Every member assignment used
  to reset the count, so such an array was never limited; the budget is per
  outermost array. And `max_file_size` no longer depends on how the source is
  chunked. Both matter only to a reader that sets those limits.
- *A datetime literal with a fraction whose UTC year falls outside 0000–9999*
  (reachable only through a timezone offset) is
  `error_invalid_datetime_literal`.
- *Eight conversion factors were corrected* (`elle`, `klafter`, `rute`,
  `morgen`, `inHg`, `Da`, `slug`, `ft_lb`): a document's digits are unchanged,
  what they convert to is not.

**What requires a 2.0.** Any change that could render a valid 1.x document invalid,
change how it decodes, renumber an error code, or change what an existing
document's bytes mean is a breaking change and is reserved for a major (2.0)
revision of the *format* — which is not the library's version (above). The changes that motivated
the 1.0 freeze — the **mandatory `$` currency sigil** (§10.4 of the unit-system
reference), **array element homogeneity** (§7.4), and **`float_fix` value-range
validation** (§6.2, rejecting a value the declared Q-format cannot represent) —
were exactly such breaks, so they were made *before* 1.0 and cannot be
reconsidered within 1.x. (The correction of §7.4 recorded above narrowed
homogeneity from dimension to unit; it did not reconsider the rule. It and the
other corrections listed in this section are the whole of what 1.2 changed for
an existing document, and each is there because the alternative was to keep a
rule the specification itself contradicted.)

---

## 18. Security Considerations

A `.bvnr` document is passive data. It carries no scripting, no macros, no
external entity references, and no include directive; a reference (§4.5) is an
inert string that the parser never follows. A conforming consumer therefore
performs no I/O beyond reading the bytes it was handed, and parsing a document
cannot by itself cause code execution or a network fetch.

What follows is what remains after that — the places where a consumer of
untrusted input has to do something the format cannot do for it. The
Internet-Draft in `doc/ietf/` carries the same material at greater length, with
normative keywords.

### 18.1 Resource Exhaustion

The defaults of §12.2 are chosen for trusted pipelines, not for hostile input.
**Total input is unbounded by default** — `max_file_size` of 0 sets no total-byte
cap, so octet-stream payload is unlimited and only the text is capped (at 2³¹ − 1
bytes, `max_text_bytes`). That is right for a telemetry stream and wrong for an
untrusted one. A consumer
at a trust boundary sets every limit in §12.2 explicitly.

Four constructs give an attacker a high ratio of work to input bytes: integer
and float widths reach 32768 bits (`BVN_MAX_INT_WIDTH` for `uint`/`sint`; `float` has the same ceiling, §5.2), and validating one
value at that width is arbitrary-precision arithmetic; a non-decimal base is
written as a quoted string (§5.5), so a 32768-bit value in base 2 is a 32768-character
literal well inside the default `max_string_length`, and base conversion is
superlinear in most implementations; array elements default to 2³¹−1 against the
one-byte-per-element syntax `[,,,,,]`; and adjacent string literals accumulate
toward `max_string_length` across the running total, not per literal.

Nesting defaults to 64 with a hard cap of 255, but the depth is only known after
the bytes are read, so a recursive-descent consumer bounds its own stack rather
than relying on the declared depth. A streaming consumer is less exposed than a
materializing one throughout.

### 18.2 Recovery Mode Discards Data Silently

Recovery mode (§13.2) delivers a *subset* of the document, and nothing in the
data says that a subset is what it is. Where the document is an access-control
list, a security policy, or a set of calibration constants, a dropped assignment
is likely to fail open.

Recovery must not be enabled where the parsed document informs a security
decision. Where it is enabled, check the **recovery count** and treat a non-zero
value as a failure of the document rather than a diagnostic. Watching the
skipped-byte total instead is a trap: a rejected value whose `;` follows
immediately skips *zero* bytes, so the total stays at zero while the assignment
arrives stripped of its value.

### 18.3 Version Leniency

By default a reader accepts a declared version newer than it supports and fails
only on a construct it does not implement (§3.4). A document declaring
`#!bovnar 2.0` but using only 1.0 constructs parses without complaint. A 1.0
reader does not see the directive at all — it is lexically a comment — so a
producer cannot rely on the declaration to cause rejection by an older reader.
For untrusted input, enable `strict_version` so an unsupported version is
rejected at the directive rather than possibly not at all.

### 18.4 Transport Corruption of Octet Streams

Octet-stream chunks are length-prefixed (§9.2). A transport that rewrites a byte
inside a payload — line-ending normalisation above all — shifts every subsequent
length field, and the reader begins interpreting attacker-supplied payload as tag
and length fields. The common outcome is a quick `error_octet_stream_out_of_sync`,
but that is not guaranteed, and a chunk length read out of a corrupted field is
an allocation request — at most 65 536 bytes per chunk (§9.2), so the hazard is
an implementation that pre-allocates or accumulates across chunks. Bound the memory committed to a single chunk independently
of its declared length, and carry documents containing octet streams over
transports that do not transform content.

### 18.5 Reference Resolution

References are stored unresolved and never followed (§4.5), so every hazard lands
in the application that resolves them. Cycles are not detected — `.a = &.b; .b = &.a;`
does not terminate under a naive resolver. Dangling paths are valid documents, so
a resolver must define what a missing target means rather than defaulting to
empty. A reference path is not a URI: it has no scheme, no authority, and no
network semantics, and must never be handed to a resolver that might fetch it.
Index syntax is uninterpreted — `&.a[0][1]` is stored verbatim, digits included,
and must be range-checked against the actual array.

### 18.6 Numeric Interpretation

**Special numbers bypass range validation.** `<uint:8> nan` and `<sint:16> ninf`
are valid (§6.4). A successful parse does not mean the value fits the declared
width, so a consumer mapping values onto fixed-width integers must handle a
non-finite value arriving on an integer channel.

**Width is declared, not proven.** The annotation states how wide the value is;
nothing in the document shows the producer honoured it. The reader's range
validation (§6.2) is the only check, and it covers the integer families and
`float_fix` alone: a `float` or `float_dec` literal is never held to its width,
and a special number is held to nothing (above). Bases 64 and 85 are `uint`-only:
their alphabets use sign characters as digits (Base64 `+`, Ascii85 `+` and `-`),
so permitting a signed value there would introduce an ambiguity the format does
not have (§5.2).

### 18.7 Units Are Validated, Not Verified

The unit system rejects a unit that is malformed, unregistered, or contradicted
by an inline suffix on the same value (§6.5, §12.8). It cannot verify that the
value is a plausible measurement in that unit, that two assignments are
dimensionally consistent with each other, or that the producer measured what it
claims. *It parsed* means the dimension is stated and internally consistent, not
that it is correct.

Read-time unit conversion carries a further trap: asking for a value in some
unit is a **request, not a guarantee**. A target *list* (the unit policy's
`targets`, `--unit`, `--si`) leaves a value it cannot convert — different
dimensions, or two different currencies — in its *original* unit and reports no
error; a per-field rule (`--field`, a policy rule by key path) and the
`want_unit` hook fail with `error_unit_mismatch` instead. Only the first needs the check. A caller that
assumes success then holds a number it believes is in one unit and which is in
another, which is exactly the failure this format exists to prevent. Check
whether the conversion occurred; do not infer it from the absence of an error.

### 18.8 Leap-Second Table Drift

The `tai` epoch requires a leap-second table, which is a static snapshot of an
IERS bulletin. Two builds of differing vintage may convert the same civil literal
to different `tai` values for instants after the older table ends. Where exact
agreement matters, transmit an integer carrier rather than a literal, and do not
assume that timestamps converted by different builds compare correctly.

### 18.9 Confusable and Non-Normalized Keys

Keys admit almost any non-ASCII character — everything but U+0080–U+00BF (§4.2), which leaves the zero-width and formatting characters, U+FEFF among them (§3.2), all available — and are compared as byte sequences with
no Unicode normalisation (§3.1, §4.2). Two keys that are canonically equivalent
under Unicode but differ in bytes are distinct keys and do not collide under the
duplicate-key rule; two keys that are visually identical but differ in code
point — Latin and Cyrillic homoglyphs — are distinct keys that a human reviewer
cannot tell apart. Where keys drive authorization or routing, apply a
normalisation and confusable-detection policy before matching, and prefer ASCII
keys where the application permits.

### 18.10 No Confidentiality, Integrity, or Authenticity

The format defines no encryption, no checksum, no signature, and no notion of an
author. All three properties must come from the transport or an enclosing
envelope. Note also that no canonical form *of a document* is defined — the canonical spelling of a unit (§11.4) and `pretty-print --canonical` normalise units and layout, not everything two equal documents can differ in: a signature over the bytes
is a signature over one *spelling* of the data, and a producer that re-emits the
document invalidates it without changing what it means.

Comments are bytes of the document and not part of its data (§4.1). A producer
that copies a document forward carries its comments forward with it, including
any that were never meant to leave the system that wrote them.

---

## Appendix A: Event Sequence Reference

### A.1 Simple Assignment (Untyped)

Input: `.foo = 42;`

```
ev_stream_start
ev_assignment_start            data="foo"
ev_type_annotation_start       (synthesised)
ev_type_annotation_type_family "uint"
ev_type_annotation_type_family_parameter  (width:64)
ev_type_annotation_type_family_parameter  (base:_10)
ev_type_annotation_type_family_parameter  (unit:no_unit)
ev_type_annotation_end
ev_data                        data="42"
ev_stream_end
```

> Every stream is bracketed by `ev_stream_start` … `ev_stream_end`; the reader
> emits `ev_stream_end` once after the final assignment (it is omitted from the
> remaining appendix examples for brevity).

### A.2 Typed Assignment

Input: `.bar = <float:32,m/s> 9.81;`

```
ev_assignment_start            data="bar"
ev_type_annotation_start       data="float:32,m/s"
ev_type_annotation_type_family data="float:32,m/s"   (family = vt_float)
ev_type_annotation_type_family_parameter  (width:32)
ev_type_annotation_type_family_parameter  (unit:m/s)
ev_type_annotation_end         data="float:32,m/s"
ev_data                        data="9.81"
```

The three bracketing events — start, family and end — all carry the same
bytes: the whole annotation body as written, or the bare family keyword when
the annotation was synthesised (`"uint"` in A.1). The family itself is in the
event's `value_type.family`. An explicit annotation emits a parameter event for
each parameter it states and no other — `<float:32,m/s>` states no base, so
there is no base event — where a synthesised one emits all three.

### A.3 Compound Unit Assignment

Input: `.force = <float:64,k~g·m/s²> 9.81;`

```
ev_assignment_start            data="force"
ev_type_annotation_start       data="float:64,k~g·m/s²"
ev_type_annotation_type_family data="float:64,k~g·m/s²"   (family = vt_float)
ev_type_annotation_type_family_parameter  (width:64)
ev_type_annotation_type_family_parameter  (unit:k~g·m/s²)
  → value_unit = {
      num_components = 3,
      components = [
        { base=bu_gram,   exponent=exp_linear,     prefix={prefix_si, si_kilo} },
        { base=bu_meter,  exponent=exp_linear,     prefix={prefix_si, si_none} },
        { base=bu_second, exponent=exp_neg_square, prefix={prefix_si, si_none} }
      ]
    }
ev_type_annotation_end
ev_data                        data="9.81"
```

### A.4 Array

Input: `.arr = [1, 2]/[3, 4];`

```
ev_assignment_start            data="arr"
ev_array_row_start
ev_type_annotation_start       (synthesised for 1)
ev_type_annotation_type_family "uint"
...params...
ev_type_annotation_end
ev_data                        data="1"
ev_data                        data="2"
ev_array_row_end
ev_array_dim_start
ev_array_row_start
ev_data                        data="3"
ev_data                        data="4"
ev_array_row_end
```

Inside an array a synthesised annotation is emitted only when the element's
synthesised type differs from the type currently in force — at the first
element, and again at each change. It is not repeated per element or per row;
an explicit element annotation is always emitted. So `[1, 2.5, 3]` carries
three annotation blocks (`uint`, `float`, `uint`) and `[1, 2]/[3, 4]` carries
one.

### A.5 Struct

Input: `.s = {.x = 1; .y = 2;};`

```
ev_assignment_start            data="s"
ev_struct_start
ev_assignment_start            data="x"
...ev_data for 1...
ev_assignment_start            data="y"
...ev_data for 2...
ev_struct_end
```

### A.6 Octet Stream

Input: `.bin = \x00\x01\x03\x00abc\x00;`

```
ev_assignment_start            data="bin"
ev_octet_stream_start
ev_data (octet_stream)         data="abc", length=3
ev_octet_stream_end
```

---

## Appendix B: Implementation Notes

### B.1 Keyword State Machine

Type family keywords are recognised through a dedicated state machine in the lexer.
The lexer fires `ACT_tf_float_done` after the shared `f→l→o→a→t` path, storing
`"float"` in `type_data` and transitioning to `type_body_outro`.  If the next bytes
are `_fix` or `_dec`, they are accumulated via `copy_type_byte`, so the final string
is `"float_fix"` or `"float_dec"`.  `bvn_parse_type_annotation` then dispatches on
the full accumulated string.

```
u → i → n → t           → keyword "uint"
u → t → f → 8           → keyword "utf8"
s → i → n → t           → keyword "sint"
f → l → o → a → t       → ACT_tf_float_done → type_body_outro
                                              → accumulate "_fix" → "float_fix"
                                              → accumulate "_dec" → "float_dec"
                                              → (nothing)         → "float"
```

### B.2 Special Number Keywords

The special floats are bare reserved keywords — `nan`, `inf`, and `ninf`
(negative infinity) — with no sigil. The lexer reads them as ordinary symbols;
the validator then reclassifies a symbol whose text is exactly one of these
three spellings into a numeric special value (`token_is_number`), the same way
it reclassifies `null`/`true`/`false`/`on`/`off`:

```
symbol "nan"   → reclassify → special number  "nan"   (3 bytes)
symbol "inf"   → reclassify → special number  "inf"   (3 bytes)
symbol "ninf"  → reclassify → special number  "ninf"  (4 bytes)
```

Any other bare word (e.g. `infinity`, `nans`) stays an ordinary symbol. The
stored token text is the keyword itself: `nan`, `inf`, `ninf`. A special-number
keyword takes no inline unit suffix; a unit is supplied through the type
annotation (`<float:64,m/s> inf`).

### B.3 Default Width, Base, and Q

```c
static inline uint32_t bvn_effective_width(value_type_spec_t s) {
    return s.width ? s.width : 64u;
}

/*
 * For float_fix, float_dec and datetime the .base field has a different meaning
 * (Q for float_fix, unused for float_dec, epoch index for datetime); always
 * report base 10 for those — the datetime carrier is decimal epoch-seconds, so
 * decoding it in the epoch index stored in .base would corrupt the value.
 */
static inline uint32_t bvn_effective_base(value_type_spec_t s) {
    if (s.family == vt_float_fix || s.family == vt_float_dec ||
        s.family == vt_datetime)
        return 10u;
    return s.base ? s.base : 10u;
}

/*
 * Returns the Q (fractional bits) for float_fix, 0 for all other families.
 * Q is stored in the .base field of value_type_spec_t.
 */
static inline uint32_t bvn_effective_q(value_type_spec_t s) {
    return (s.family == vt_float_fix) ? s.base : 0u;
}
```

### B.4 Type Equality

```c
static inline bool bvn_type_spec_eq(value_type_spec_t a, value_type_spec_t b) {
    return a.family == b.family && a.width == b.width && a.base == b.base;
}
```

For `float_fix`, `.base` holds Q, so two `float_fix` specs are equal only if they
share the same width **and** the same Q.

### B.5 Numeric Type Check

```c
static inline bool bvn_type_is_numeric(value_type_spec_t s) {
    return s.family == vt_sint      || s.family == vt_uint    ||
           s.family == vt_float     ||
           s.family == vt_float_fix || s.family == vt_float_dec;
}
```

### B.6 Unit Component Access

When iterating compound units, always check `num_components`:

```c
for (uint32_t i = 0; i < u.num_components && i < BVNR_MAX_UNIT_COMPONENTS; i++) {
    value_unit_component_t* c = &u.components[i];
    /* c->base, c->exponent, c->prefix.system, c->prefix.id */
}
```

### B.7 Fixed-point and Decimal Float Wire Representations

`float_fix` and `float_dec` are both serialised as ordinary decimal number literals
in the Bovnar text layer.  The type annotation is the sole indicator of wire encoding.
At the C API level, the conversion path is:

```
float_fix (width ≤ 64):
    text literal → bvn_float_t → bvn_float_to_fixNN(f, frac_bits) → signed integer return value

float_fix (width = 128, 256):
    text literal → bvn_float_t → bvn_float_to_fixNN(f, frac_bits, out) → wire bits in out[]

float_dec:
    text literal → bvn_float_t → bvn_float_to_decNN(f, out) → wire bits in *out / out[]
```

The `bvn_float_t` intermediate representation is MPFR-layout-compatible (see
`bvn_float.h`) and provides exact round-trip fidelity up to the declared precision.

---

## Appendix C: Limits Summary

| Constant | Value | Description |
|----------|-------|-------------|
| reader default struct nesting | 64 | Default applied by the reader when `max_struct_nesting` is 0; hard cap is 255 |
| reader default array nesting | 64 | Default applied by the reader when `max_array_nesting` is 0; hard cap is 255 |
| writer default struct nesting | 64 | Default applied by the writer when `max_struct_nesting` is 0; hard cap is 255 |
| writer default array nesting | 64 | Default applied by the writer when `max_array_nesting` is 0; hard cap is 255 |
| reader default max_identifier_length | 255 | Default applied by the reader when `max_identifier_length` is 0; longer is `error_identifier_too_long` |
| reader default max_string_length | 65 535 | Default applied by the reader when `max_string_length` is 0; longer is `error_string_too_long` |
| reader default max_number_length | 65 535 | Default applied by the reader when `max_number_length` is 0; longer is `error_number_too_long` |
| reader default max_symbol_length | 255 | Default applied by the reader when `max_symbol_length` is 0; longer is `error_symbol_too_long` |
| reader default max_reference_length | 65 535 | Default applied by the reader when `max_reference_length` is 0; longer is `error_reference_too_long` |
| length-limit ceiling | 65 535 | The five `max_*_length` fields are 16-bit, so no setting raises a limit past this. The writer accepts them for symmetry and does not consult them |
| reader default max_array_items | 2 147 483 647 | Default applied by the reader when `max_array_items` is 0 |
| reader default max_text_bytes | 2 147 483 647 | Default applied by the reader when `max_text_bytes` is 0 |
| reader default max_file_size | 0 (no total-byte cap) | A `max_file_size` of 0 imposes no total-byte cap; the text is still bounded by `max_text_bytes`. Set to 16 777 216 (16 MiB) in production |
| recommended file size cap | 16 777 216 | Suggested explicit value for `max_file_size` (16 MiB) |
| `BVNR_MAX_UNIT_COMPONENTS` | 32 | Maximum number of unit components in a compound unit |
| type-annotation body | 255 bytes | The bytes between `<` and `>`, whitespace beside separators not counted (§12.2); longer is `error_type_too_long` |
| inline unit suffix | 255 bytes | Longer is `error_unit_too_long` |
| unit exponent | −100 … 100 | `BVN_EXPONENT_MIN` … `BVN_EXPONENT_MAX`, zero excluded (§11.5) |
| unit `(…)` group nesting | 16 | Deeper is `error_unit_illegal` (§11.4) |
| `BVN_MAX_INT_WIDTH` | 32768 | Maximum bit-width for `uint`, `sint` and `datetime`; `float` has the same ceiling (§5.2). The validator and writer reject any declared width exceeding this value with `error_illegal_value_type`. |

---

## See also

- [Tutorial](01_bovnar_tutorial.md) — the same format taught by example, start here if this is your first Bovnar document
- [Unit & Currency Reference](05_bovnar_unit_system.md) — the full registry behind §11, with dimensions and conversion factors
- [Read & Write API](08_bovnar_readwrite_api.md) — the C reader, writer, and DOM that implement §16
- [EBNF Grammar](12_bovnar.ebnf) — the formal grammar referenced by §14
- [FAQ](02_bovnar_faq.md) — the questions this document answers indirectly
- [Conformance Test Tool](13_bovnar_conformance.md) — how an implementation proves it follows this specification

---

*End of Bovnar — Specification (Bovnar spec 1.2).*
