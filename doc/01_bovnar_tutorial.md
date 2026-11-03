# Bovnar — Tutorial

> **Spec version:** 1.2
> **Status:** Non-normative — a guided, example-driven introduction
> **Scope:** Writing and reading Bovnar documents by hand: types, units, arrays, structs, references, and the event model.

---

## Table of Contents

1. [Why Bovnar Exists](#1-why-bovnar-exists)
2. [The Absolute Minimum: A Valid Bovnar File](#2-the-absolute-minimum-a-valid-bovnar-file)
3. [Keys: Identifiers](#3-keys-identifiers)
4. [Value Tokens: What Can Follow the `=` Sign](#4-value-tokens-what-can-follow-the--sign)
5. [Numbers and the Type Annotation](#5-numbers-and-the-type-annotation)
    - 5.1 [Bare Literals](#51-bare-literals)
    - 5.2 [Type Annotation Syntax](#52-type-annotation-syntax)
    - 5.3 [Type Families](#53-type-families)
    - 5.4 [Parameters](#54-parameters)
6. [Units](#6-units)
    - 6.1 [Simple Units](#61-simple-units)
    - 6.2 [SI Prefixes](#62-si-prefixes)
    - 6.3 [Compound Units](#63-compound-units)
    - 6.4 [Explicitly Dimensionless: `no_unit`](#64-explicitly-dimensionless-no_unit)
    - 6.5 [Inline Unit Suffix](#65-inline-unit-suffix)
    - 6.6 [Units From Another Vocabulary (spec 1.2)](#66-units-from-another-vocabulary-spec-12)
7. [Fixed-Point Numbers: `float_fix`](#7-fixed-point-numbers-float_fix)
8. [Decimal Floating-Point: `float_dec`](#8-decimal-floating-point-float_dec)
9. [Strings](#9-strings)
    - 9.1 [Escape Sequences](#91-escape-sequences)
    - 9.2 [String Concatenation](#92-string-concatenation)
10. [Symbols](#10-symbols)
11. [Booleans](#11-booleans)
12. [Null Values](#12-null-values)
13. [References](#13-references)
14. [Arrays](#14-arrays)
    - 14.1 [Multi-Dimensional Rows](#141-multi-dimensional-rows)
    - 14.2 [Per-Element Type Annotations](#142-per-element-type-annotations)
    - 14.3 [Structs as Array Elements](#143-structs-as-array-elements)
15. [Structs](#15-structs)
16. [Octet Streams: Inline Binary](#16-octet-streams-inline-binary)
17. [Putting It Together: Realistic Examples](#17-putting-it-together-realistic-examples)
    - 17.1 [Hardware Configuration](#171-hardware-configuration)
    - 17.2 [Scientific Measurement Batch](#172-scientific-measurement-batch)
    - 17.3 [Multi-Dimensional Matrix](#173-multi-dimensional-matrix)
    - 17.4 [Mixed Binary and Text](#174-mixed-binary-and-text)
18. [Common Mistakes and the Errors They Produce](#18-common-mistakes-and-the-errors-they-produce)
19. [Error Recovery Mode](#19-error-recovery-mode)
20. [Size Limits](#20-size-limits)
21. [Event Model (C API Overview)](#21-event-model-c-api-overview)
22. [Quick-Reference: Syntax Cheat Sheet](#22-quick-reference-syntax-cheat-sheet)

- [See also](#see-also)

---

## 1. Why Bovnar Exists

Bovnar is **unit-safe serialization for scientific and industrial systems**. In those domains, the expensive failures are rarely bad syntax — they are unit confusion. A thrust value sent in pounds-force and read as newtons. An altitude in feet read as meters. The number parsed perfectly; the dimension was wrong, and nothing in the data said otherwise.

A bare `9.81` carries no dimension on its own: it could be meters per second, volts, or a dimensionless ratio, and the receiving application has no way to know without an external schema, a naming convention, or documentation. The unit lives outside the data, and that gap is where the expensive failures hide.

Bovnar makes a different choice: every value in a Bovnar document is **self-describing and unit-safe**. The type family (signed integer, float, string…), the bit-width, the numeric base, and the physical unit all travel *with* the value, in the same byte stream, without any external schema — and the unit is validated by the parser rather than assumed. Annotate a value as `m/s` and write a mismatched inline unit, and parsing fails. Hand a `.bvnr` file to anyone, and they have everything needed to interpret *and dimensionally trust* it: a 64-bit signed integer measured in kilopascals looks different from a 32-bit unsigned integer measured in mebibytes, and the format makes that difference explicit.

The format also supports raw binary embedding, multi-dimensional arrays with a clean row-separator syntax, and incremental streaming parsing — all in a text layer that is pleasant to read and write by hand.

---

## 2. The Absolute Minimum: A Valid Bovnar File

A Bovnar file is a sequence of **assignments**. Every assignment looks like this:

```bovnar
.key = value;
```

Three things to notice immediately:

1. The key always starts with a dot.
2. The value comes after `=`, optionally preceded by a type annotation.
3. Every assignment ends with a semicolon.

The simplest possible complete file:

```bovnar
.greeting = "hello";
.count    = 42;
.ratio    = 3.14;
```

Comments begin with `#` and run to end-of-line:

```bovnar
# This is a config file
.host    = "localhost";     # the server address
.port    = 5432;            # PostgreSQL default
```

The format is UTF-8 throughout. A UTF-8 BOM is accepted only at the very first byte of the stream.

---

## 3. Keys: Identifiers

A key is the bare word after the leading dot. Identifiers follow these rules:

- Must begin with a letter (`A–Z`, `a–z`), an underscore, or a UTF-8 leader byte in the range 0xC3–0xF4 (i.e., most non-ASCII letters).
- The body may also contain `+`, `-`, and digits.
- Byte `0xC2` (the UTF-8 leader for U+0080–U+00BF) is rejected at both the start and body of an identifier.
- Most ASCII punctuation is forbidden — specifically `! " # $ % & ' ( ) * , . / : ; < = > ? @ [ \ ] ^ ` { | } ~`. If you hit a parse error on what looks like a valid identifier, one of these characters is likely hiding inside it.
- An empty key (`.=`) is a hard error.

```bovnar
.simple       = 1;
.with_under   = 2;
.with-hyphen  = 3;
.with+plus    = 4;
.camelCase    = 5;
.ALL_CAPS     = 6;
.v2           = 7;     # digit in body is fine; not at start
```

This would fail:

<!-- bovnar-example: rejected -->
```bovnar
.123invalid = 1;       # error: starts with a digit
.bad,key    = 1;       # error: comma is forbidden in identifiers
```

---

## 4. Value Tokens: What Can Follow the `=` Sign

These are the categories of raw values you will write by hand (two more — the ISO-8601 datetime literal of spec 1.1, §5.3, and the octet stream, §16 — complete the list):

| Category | Example |
|---|---|
| Integer literal | `42`, `-7`, `"FF"` (base-16, quoted) |
| Float literal | `3.14`, `1e6`, `-.5` |
| Special number | `nan`, `inf`, `ninf` |
| Boolean | `true`, `false`, `on`, `off` |
| Quoted string | `"hello world"` |
| Symbol (bare word) | `ok`, `Monday`, `read_only` |
| Reference | `&.other.key` |
| Array | `[1, 2, 3]` |
| Struct | `{.x = 1; .y = 2;}` |
| Null | `;` (empty) or the `null` keyword |

Each is covered in depth below, starting with numbers because that is where Bovnar's type system is most visible.

---

## 5. Numbers and the Type Annotation

### 5.1 Bare Literals

Numbers are written as plain text. Positive integers default to `uint:64`, negative integers to `sint:64`, and anything with a `.` or `e` to `float:64`:

```bovnar
.a = 42;            # → uint:64
.b = -7;            # → sint:64
.c = 3.14;          # → float:64
.d = 1e6;           # → float:64 (1 000 000)
.e = -.5;           # → float:64 (leading-dot form, -0.5)
.f = 123.;          # → float:64 (trailing dot, valid)
.g = 007;           # → uint:64 (leading zeros are legal; value is 7)
```

This automatic type assignment is called **default type synthesis**. It is convenient for quick files and config work. For anything where precision matters, you add an explicit type annotation.

### 5.2 Type Annotation Syntax

The annotation is enclosed in angle brackets and placed **between `=` and the value** — not on the key, not after the value:

```bovnar
.port   = <uint:16> 443;
.temp   = <float:32> 23.5;
.offset = <sint:64> -2147483648;
```

Placing the annotation anywhere else is a hard error:

<!-- bovnar-example: rejected -->
```bovnar
.port<uint:16> = 443;      # WRONG: annotation must come after '='
```

### 5.3 Type Families

There are seven core type families (spec 1.0), plus `datetime` added in spec 1.1:

| Keyword | Description |
|---|---|
| `uint` | Unsigned integer |
| `sint` | Signed integer |
| `float` | IEEE 754 binary floating-point |
| `float_fix` | Q-format signed fixed-point |
| `float_dec` | IEEE 754-2008 decimal floating-point |
| `utf8` | UTF-8 string |
| `bool` | Boolean (`true`/`false`); takes no parameters |
| `datetime` | Timestamp: signed epoch-seconds, or an ISO-8601 literal like `2026-06-15T12:00:00Z` (spec 1.1; needs a `#!bovnar 1.1` directive) |

### 5.4 Parameters

After the family keyword you may add parameters separated by commas after a `:`. The three parameter classes are **width**, **base**, and **unit**. They are identified by their syntactic form and may appear in any order:

```bovnar
.a = <uint:32>           1000;   # width only
.b = <uint:32,_16>       "FF";   # width + base (hex)
.c = <uint:32,m>         500;    # width + unit (meters)
.d = <uint:32,_16,m>     "1F4";  # all three — 500 meters in hex
.e = <uint:_16,m,32>     "1F4";  # same thing; order doesn't matter
```

#### Width

Width is a plain decimal number of bits. `0` means "use the default," which is always 64:

```bovnar
.a8  = <uint:8>   255;
.a16 = <uint:16>  65535;
.a32 = <uint:32>  4294967295;
.a64 = <uint:64>  18446744073709551615;
.def = <uint:0>   99;           # same as <uint:64>
```

For `float`, valid widths are `0`, `16`, and any multiple of `32` up to `32768`. Width `8` or a non-multiple-of-32 (other than 16) is a hard error. For `float_fix` and `float_dec`, valid widths are `0`, `16`, `32`, `64`, `128`, and `256`.

```bovnar
.half   = <float:16>  3.14;    # half-precision
.single = <float:32>  3.14;    # single-precision
.double = <float:64>  3.14;    # double-precision
.quad   = <float:128> 3.14;    # quad-precision
```

For `sint` and `uint`, any width from 1 up to 32768 bits (`BVN_MAX_INT_WIDTH`) is accepted — `<uint:12>` is perfectly valid for a 12-bit ADC sample.

#### Base

The base parameter starts with `_` followed by a decimal number. For `uint` and `sint`, every base from `_2` through `_62` (consecutive) is allowed. Bases `_64` (standard Base64) and `_85` (Ascii85) are also available, but **only for `uint`**: their alphabets claim the sign glyphs as digits — Base64 uses `+` (digit 62) and `/`, Ascii85 uses both `+` and `-` — so no unambiguous sign character remains and signed values are illegal (`error_illegal_value_type`). The base selects the numeral system used to interpret the value.

This is the biggest surprise for newcomers: **non-decimal values should be provided as quoted strings**. A bare `ff` in value position is lexed as a *symbol*, not a number, and will produce a type mismatch error. A bare literal made only of digits of the base does parse — and is read in that base, so `<uint:8,_16> 10` is sixteen, and `<uint:_16> 1e3` is 483 because `e` is a hex digit — which is one more reason to always quote non-decimal digits:

<!-- bovnar-example: rejected -->
```bovnar
.hex = <uint:8,_16>  "FF";       # correct — 255
.hex = <uint:8,_16>  FF;         # WRONG — FF is a symbol
.bin = <uint:8,_2>   "11010110"; # correct — 214
.oct = <uint:16,_8>  "755";      # correct — Unix permission rwxr-xr-x
```

Signed values use a minus prefix inside the string:

```bovnar
.neg_hex = <sint:32,_16> "-7FFFFFFF";
.neg_bin = <sint:8,_2>   "-10000000";   # -128 in binary
```

`float` accepts only `_10` (the default) and `_16`. `float_fix` and `float_dec` **do not accept a base parameter at all** — specifying one is a hard error.

#### Special Number Literals

Three special values bypass normal numeric parsing. They are **bare reserved keywords** — no sigil — and, like `null`/`true`/`false`, are reclassified out of the symbol space by the validator:

```bovnar
.nan = nan;
.inf = inf;
.neg = ninf;     # negative infinity
```

Only these exact spellings are reserved: any other bare word (e.g. `infinity`, `nans`) is an ordinary symbol. They may be combined with any float type annotation and are valid in untyped context too. A special-number keyword takes no inline unit suffix — give its unit via the annotation (`<float:64,m/s> inf`).

---

## 6. Units

This is the feature most unique to Bovnar. Physical units are a first-class part of the type annotation, not a comment or a string field you hope someone reads.

### 6.1 Simple Units

The unit goes inside the angle brackets, after the other parameters:

```bovnar
.timeout  = <float:64,s>    2.5;      # seconds
.distance = <uint:64,m>     384400000; # meters (Earth-Moon distance)
.voltage  = <float:32,V>    3.3;      # volts
.temp     = <float:32,°C>   23.5;     # degrees Celsius (UTF-8 is fine)
```

### 6.2 SI Prefixes

SI prefixes are written before the base unit symbol, either with the `~` separator or compactly without it:

```bovnar
.k_ohm  = <float:32,k~Ω>   4.7;     # kilo-ohm
.milli  = <float:32,m~V>   330.0;   # millivolt
.micro  = <float:32,µ~s>   50.0;    # microsecond (µ = U+00B5)
.giga   = <float:64,G~Hz>  2.4;     # gigahertz
.kibi   = <uint:64,Ki~B>   1024;    # kibibytes (IEC binary prefix)
.mebi   = <uint:64,Mi~B>   4096;    # mebibytes
```

`<float:32,kΩ>` means exactly the same thing as `<float:32,k~Ω>` — the `~` is optional on a physical unit, and `kg`, `km`, `MHz`, `MiB` all parse as you would expect. Two things to know about the compact spelling. A token that is *itself* a unit always wins (`min` is the minute, so milli-inch has to be written `m~in`), and the canonical spelling is the `~` form: that is what the library writes for a unit it is handed without a spelling. (Re-serialising a document keeps the spelling it was written in; `bovnar pretty-print --canonical` goes the other way: it reduces each unit, folding the prefix of a base unit into the value and writing any prefix that remains in the `~` form.) A prefixed **currency** works the same way (`k$EUR` = `k~$EUR`), but its `$` sigil is never optional.

IEC binary prefixes (`Ki`, `Mi`, `Gi`, `Ti`, `Pi`, `Ei`, `Zi`, `Yi`, and the forward-looking `Ri`, `Qi` — IEC 80000-13 itself stops at `Yi`) follow the same rule:

```bovnar
.ram  = <uint:64,Gi~B>  8;     # 8 gibibytes
.disk = <uint:64,TiB>   2;     # 2 tebibytes (compact spelling of Ti~B)
```

### 6.3 Compound Units

Compound units are built from multiple components joined by `*` (or the middle dot `·`, U+00B7) for multiplication, and `/` for division:

```bovnar
.velocity     = <float:64,m/s>         9.81;   # meters per second
.acceleration = <float:64,m/s²>        9.81;   # m·s⁻²
.force        = <float:64,k~g·m/s²>    9.81;   # Newton (kilogram × meter / second²)
.energy       = <float:64,k~g·m²/s²>   1000;   # Joule
.pressure     = <float:64,k~g/(m·s²)>  101325; # Pascal
.momentum     = <float:64,k~g·m/s>     0.5;
.field        = <float:64,V/m>         150.0;
.moment       = <float:64,m*s>         1.0;    # asterisk = middle dot
```

The `/` separator is a **one-way switch**: once you write `/`, every subsequent component is in the denominator. Writing another `/` does not switch back to the numerator. If you need a component back in the numerator after a divisor, use a negative exponent:

```bovnar
.force_alt = <float:64,k~g·m·s⁻²> 9.81;   # same as k~g·m/s²
```

Exponents can be written as Unicode superscripts (`²`, `³`, `⁻¹`) or with an ASCII caret (`^2`, `^-2`). Both are valid:

```bovnar
.area1 = <float:64,m²>   100.0;
.area2 = <float:64,m^2>  100.0;   # same thing
.inv1  = <float:64,s⁻¹>  50.0;
.inv2  = <float:64,s^-1> 50.0;    # same thing
```

The maximum number of unit components in a compound unit is 32 (`BVNR_MAX_UNIT_COMPONENTS`). More than that is `error_unit_illegal`.

### 6.4 Explicitly Dimensionless: `no_unit`

When you want to be explicit that a value carries no physical dimension, use the keyword `no_unit`:

```bovnar
.ratio = <float:64,no_unit> 1.4142;
.count = <uint:32,no_unit>  42;
```

There are three ways to be dimensionless, and only one of them is a *different* internal state:

| Written | Internal | |
|---|---|---|
| `<float:64,no_unit> 1.0` | `BVN_UNIT_NONE`, `num_components == 0` | you said so |
| `<float:64> 1.0` | `BVN_UNIT_NONE`, `num_components == 0` | annotated, unit parameter omitted — the *same* state |
| `1.0` | `BVN_UNIT_NO_PREFIX(bu_none)`, `num_components == 1` | no annotation at all; the default type is synthesised |

Omitting the unit parameter *inside an annotation* is the same thing as writing `no_unit` — `bvn_parse_type_annotation` initialises the unit to `BVN_UNIT_NONE` and only overwrites it when a dimensioned unit parameter is actually present. It is a **fully untyped** value that gets the one-component form. All three compare as compatible via `bvn_units_compatible` and all three serialise to `"no_unit"` via `bvn_unit_to_string`. The explicit `no_unit` is useful for documentation: it tells a reader that the author actively chose dimensionless, not that they forgot.

### 6.5 Inline Unit Suffix

For untyped or partially-typed values, you can append a unit suffix directly after the value literal, separated by at least one whitespace character:

```bovnar
.speed  = 9.81 m/s;       # valid
.mass   = 70.0 k~g;       # result: float:64, unit = k~g
.heap   = 65536 B;         # result: uint:64, unit = B (bytes)
```

This is forbidden:

<!-- bovnar-example: rejected -->
```bovnar
.bad = 9.81m;             # ERROR: no separator at all
```

If a type annotation also specifies a unit and you additionally write an inline suffix, they must match exactly. A mismatch is a hard error:

<!-- bovnar-example: rejected -->
```bovnar
.ok  = <float:64,m/s> 9.81 m/s;   # redundant but valid: both say m/s
.bad = <float:64,m>   9.81 s;     # ERROR: annotation says m, inline says s
```

**The inline unit suffix is completely forbidden inside array elements.** This is a lexer-level restriction, not a semantic one — any letter or underscore following a value token inside `[…]` is an immediate hard error.

### 6.6 Units From Another Vocabulary (spec 1.2)

> **Spec 1.2.** A document reaches this notation only by declaring `#!bovnar 1.2`; under 1.1, or with no directive, every unit below is `error_unit_illegal`.

Data rarely starts out in Bovnar. A lab system hands you UCUM codes, an invoice carries UN/ECE codes, a netCDF file has UDUNITS strings. You do not have to translate them by hand: write `namespace:code` in the unit slot and the parser translates it.

```bovnar
#!bovnar 1.2
.systolic = <float:64,ucum:mm[Hg]>           120.0;   # UCUM
.weight   = <float_dec:64,unece:KGM>          12.5;   # UN/ECE Rec 20: kilogram
.speed    = <float:64,qudt:M-PER-SEC>          9.81;  # QUDT
.flux     = <float:64,udunits:kg m-2 s-1>      0.5;   # UDUNITS: a space multiplies
.glucose  = 5.4 ucum:mmol/L;                          # inline works too
```

Seven namespaces exist: `ucum`, `unece`, `qudt`, `qudt-qk`, `udunits`, `om` and `cf`. Three things are worth knowing before you use one.

**It is a spelling, not a second kind of unit.** `ucum:mm[Hg]` and `mmHg` are the same unit after parsing — they compare equal, convert to each other with a factor of exactly one, and satisfy the same policies. That is also why the match rule of §6.5 is about the unit and not the text:

```bovnar
#!bovnar 1.2
.ok = <float:64,mmHg> 120.0 ucum:mm[Hg];   # valid: one unit, written two ways
```

**A code either becomes a real unit or it is an error.** There is no "unknown unit, passed through". A code the vocabulary does not define is `error_unit_illegal`; one it defines but Bovnar cannot represent — a decibel referenced to a fixed level, say — is `error_unit_profile_unsupported`; a namespace this build does not have is `error_unit_profile_unknown`.

<!-- bovnar-example: rejected -->
```bovnar
#!bovnar 1.2
.bad = <float:64,ucum:furlongs> 1.0;       # ERROR: not a UCUM code
```

**Your spelling is kept.** Re-serialising a document writes `ucum:mm[Hg]` back as `ucum:mm[Hg]`, including a UCUM annotation such as `ucum:mL{total}` whose `{total}` is not part of the unit at all. The reader also hands the spelling to your code beside the parsed unit, so you can tell what was written from what it means.

The full rules, the tables and the codes that are refused are in [Unit Profiles](11_bovnar_unit_profiles.md).

---

## 7. Fixed-Point Numbers: `float_fix`

Fixed-point (Q-format) numbers store a value as a signed integer, with the binary point shifted by a declared number of fractional bits. The wire representation is an integer; the mathematical value is `raw_integer × 2⁻Q`.

The annotation requires a `qN` parameter specifying fractional bits, in addition to the width:

```bovnar
.adc = <float_fix:16,q8>  3.14;    # 16-bit, 8 fractional bits
                                    # range: [-128, 127.996], resolution ≈ 0.0039
.pid = <float_fix:32,q16> -1.5;    # 32-bit Q16: 15 integer bits + sign + 16 fractional
.vel = <float_fix:32,q8,m/s> 9.81; # with a unit
.cnt = <float_fix:64,q0>  4096;    # Q0 = no fractional bits (pure integer shell)
```

Constraints:

- `Q` must be in the range `0 ≤ Q < effective_width`. `<float_fix:16,q16>` is illegal because Q must be strictly less than the width.
- The base parameter (`_N`) is forbidden. `<float_fix:32,q8,_10>` is an error.
- Valid widths: `0` (→64), `16`, `32`, `64`, `128`, `256`. Width `8` is not valid.

---

## 8. Decimal Floating-Point: `float_dec`

`float_dec` is IEEE 754-2008 decimal floating-point — useful in financial and metrological contexts where exact decimal representation matters (binary floats cannot represent `0.1` exactly; decimal floats can):

```bovnar
.price   = <float_dec:32>     12.99;    # 7 significant decimal digits
.voltage = <float_dec:64,Pa>  101325.0; # 16 significant decimal digits
.pi_big  = <float_dec:128>    3.14159265358979323846264338327950288; # 34 digits
```

Rules are similar to `float_fix`: no base parameter, same set of valid widths.

---

## 9. Strings

Strings are enclosed in double quotes. The `<utf8>` annotation is optional — a bare quoted literal is automatically synthesised as `utf8`:

```bovnar
.greeting = "hello";           # → <utf8>
.typed    = <utf8> "world";    # explicit, same result
.empty    = "";
```

### 9.1 Escape Sequences

Seven escape sequences are defined in spec 1.0:

| Escape | Meaning |
|---|---|
| `\t` | Horizontal tab |
| `\n` | Line feed |
| `\v` | Vertical tab |
| `\f` | Form feed |
| `\r` | Carriage return |
| `\"` | Double quote |
| `\\` | Backslash |

In spec 1.0 any other character after a backslash is a hard error. A document that declares `#!bovnar 1.1` additionally gets `\xHH` (one byte) and `\u{H…}` (a Unicode scalar, UTF-8 encoded) — but a control byte (other than the whitespace ones above) is still rejected, so for arbitrary binary use an octet stream (see below). In a 1.0/unversioned document `\x`/`\u` remain hard errors.

Raw whitespace bytes (HT, LF, VT, FF, CR) are accepted unescaped inside string literals, so a multi-line string is valid:

```bovnar
.poem = "roses are red
violets are blue";
```

### 9.2 String Concatenation

Adjacent string literals separated only by whitespace or comments are concatenated at lex time:

```bovnar
.url  = "https://" "api.example.com" "/v1";
.long = "first part "
        "second part "    # a comment is fine here
        "third part";
```

This is the idiomatic way to write a long string across multiple lines or to construct a non-decimal value that happens to be split for readability:

```bovnar
.guid = <uint:128,_16> "deadbeef"
                       "cafebabe"
                       "01234567"
                       "89abcdef";
```

---

## 10. Symbols

A symbol is a bare word in value position — no quotes. It is its own token type, distinct from strings:

```bovnar
.status  = ok;
.mode    = read_only;
.day     = Monday;
```

Symbol bodies follow the same character rules as identifier bodies: letters, digits, `+`, `-`, `_`, and valid UTF-8 high bytes. The key distinction from an identifier is context: identifiers appear as keys (after the opening dot), symbols appear as values.

Inside an array, `]` and `,` terminate a symbol. At assignment level, `;` terminates it.

Eight bare words are **reserved keywords** and are *not* symbols: `null`, `true`, `false`, `on`, `off`, and the special floats `nan`, `inf`, `ninf` (see the special-number section). A longer word that merely starts with one — `ontology`, `nullable`, `truthy`, `infinity` — is still an ordinary symbol.

---

## 11. Booleans

`true`, `false`, `on`, and `off` are reserved keywords carrying the `bool` type family. `on` is an alias for `true` and `off` for `false`; all four collapse to a boolean value and serialize canonically as `true` / `false`:

```bovnar
.enabled = true;
.debug   = off;          # same value as false
.typed   = <bool> on;    # explicit annotation; on == true
```

A bare boolean synthesises a `<bool>` annotation automatically. The `<bool>` family takes no parameters (`<bool:8>` is a hard error), and only the four keywords are valid `<bool>` values.

---

## 12. Null Values

A null is the explicit absence of a value. It appears as an empty slot — nothing between `=` and `;` — or, equivalently, the reserved keyword `null`; extra commas inside an array also produce nulls:

```bovnar
.nothing    = ;              # null, no type
.also_null  = null;          # the null keyword — same as the empty slot
.typed_null = <uint:32> ;    # null with an explicit type annotation
```

Inside arrays, null elements arise from leading, trailing, or consecutive commas:

```bovnar
.sparse = [, 1, , 2, ];      # null, 1, null, 2, null — five elements
.holes  = [<sint:16> , <sint:16> 0, <sint:16> ];  # typed nulls
```

---

## 13. References

A reference is a dotted path to another key in the document, introduced by `&`. The parser stores the path as a string token; it does not resolve it — resolution is entirely up to the application:

```bovnar
.host       = "db.internal";
.port       = 5432;

.conn_host  = &.host;          # stored as ".host"
.conn_port  = &.port;          # stored as ".port"
```

Multi-segment paths:

```bovnar
.server = {
    .address = "10.0.0.1";
    .tls     = { .cert = "/etc/ssl/server.pem"; };
};

.proxy_addr = &.server.address;     # ".server.address"
.cert_path  = &.server.tls.cert;    # ".server.tls.cert"
```

References are valid in arrays and structs:

```bovnar
.nodes   = [&.master, &.replica-a, &.replica-b];

.worker  = {
    .retries = &.defaults.retries;
    .delay   = &.defaults.delay_s;
};
```

A reference is not a null. If a field is optional and currently has no referent, represent the absence with an explicit null:

```bovnar
.override = ;             # null — no reference, use internal default
# .override = &.config.x; # or, non-null — follow this reference
```

**Array indexing (spec 1.1).** In a `#!bovnar 1.1` document a reference may end
with one or more `[N]` index suffixes to address an array element. For a flat
`/`-row matrix the two indices are `[row][col]`:

```bovnar
#!bovnar 1.1
.matrix = [10, 20, 30]/[40, 50, 60];
.cell   = &.matrix[1][2];   # the element at row 1, column 2 → 60
.first  = &.vector[0];      # a 1-D array takes a single [i]
```

The path (including the indices) is stored verbatim; the DOM layer
(`bvn_dom_lookup` / the CLI `query`) is what resolves it to a value. Index
suffixes are rejected in a 1.0 or unversioned document.

---

## 14. Arrays

An array is enclosed in square brackets. A single row is a comma-separated list of values:

```bovnar
.primes = [2, 3, 5, 7, 11, 13];
.names  = ["Alice", "Bob", "Carol"];
```

### 14.1 Multi-Dimensional Rows

Multiple rows are separated by `/`. Each bracketed group is one row:

```bovnar
.matrix = [1, 2, 3]/[4, 5, 6];    # 2 rows × 3 columns
```

The event stream reflects this: `ev_array_row_start`, elements, `ev_array_row_end`, then `ev_array_dim_start` before the second `ev_array_row_start`. Row-separator `/` and array nesting are independent concepts — do not confuse them. The `/` between `]` and `[` creates a new dimension in the *same* array, not a nested one.

The `/`-separated dimension rows of a single array must have a **consistent element count**, and (since spec 1.0) array elements are **homogeneous**: sibling sub-arrays must also match in length, so bare arrays and matrices are rectangular:

<!-- bovnar-example: rejected -->
```bovnar
.ok1 = [1, 2, 3]/[4, 5, 6];   # valid: both /-rows of one array have 3 elements
.ok2 = [[1, 2], [3, 4]];       # valid: rectangular sub-arrays
# .ok3 = [[1, 2], [3, 4, 5]];  # error_array_row_size_mismatch: sibling sub-arrays differ (2 vs 3)

.bad1 = [1, 2, 3]/[4, 5];      # error_array_row_size_mismatch: 3 vs 2 (/-rows of one array)
.bad2 = [[1, 2]/[3, 4, 5]];    # error_array_row_size_mismatch: 2 vs 3 (one /-array's own rows)
```

### 14.2 Per-Element Type Annotations

Each element may carry its own annotation, placed immediately after the comma (or the opening bracket for the first element):

```bovnar
.mixed = [<uint:8> 255, <sint:8> -128, <float:64> 3.14];
```

A single annotation before `[` applies to all elements — it is a whole-array annotation:

```bovnar
.ports = <uint:16> [80, 443, 8080, 8443];
.temps = <float:32,°C> [23.5, 24.1, 22.8];
```

Per-element annotations can still appear inside a whole-array-annotated array.

### 14.3 Structs as Array Elements

Arrays of structs are the standard idiom for a list of records:

```bovnar
.users = [
    {.id = <uint:32> 1; .name = "Alice"; .role = admin;},
    {.id = <uint:32> 2; .name = "Bob";   .role = user;}
];
```

---

## 15. Structs

A struct is a nested scope, grouping zero or more assignments inside `{…}`. Every field inside is a complete assignment with `.key = value;` syntax. The struct value at the parent level ends with `};`:

```bovnar
.person = {
    .name   = "Alice";
    .age    = 30;
    .active = true;
};
```

Structs nest deeply — the limit is `max_struct_nesting`, which defaults to 64 and has a hard cap of 255:

```bovnar
.config = {
    .database = {
        .primary = {
            .host = "db1.internal";
            .port = <uint:16> 5432;
            .tls  = {
                .enabled = true;
                .cert    = "/etc/ssl/db.pem";
            };
        };
    };
};
```

An empty struct is valid:

```bovnar
.placeholder = {};
```

An unmatched closing brace — a `}` with no matching `{` — is `error_illegal_struct_close`.

---

## 16. Octet Streams: Inline Binary

When you need to embed raw binary data without Base64 overhead, a NUL byte (`0x00`) in value position switches the parser into binary chunk mode.

The wire protocol for an octet stream is:

```
0x00                     ← stream open
  0x01 <LL> <LL> <data>  ← one chunk: tag 0x01, length as little-endian uint16, then data
  0x01 <LL> <LL> <data>  ← another chunk
0x00                     ← stream close
```

A length value of `0x0000` means exactly 65536 bytes. Any tag byte other than `0x01` (chunk) or `0x00` (close) inside a stream is `error_octet_stream_out_of_sync`.

In practice, you will not hand-author octet streams — the writer API does this for you. In a `.bvnr` file shown for documentation purposes it appears as an escaped binary sequence:

<!-- bovnar-example: illustrative -->
```bovnar
.binary = \x00\x01\x05\x00hello\x01\x03\x00bye\x00;
```

This produces two data events with payloads `"hello"` (5 bytes) and `"bye"` (3 bytes). The UTF-8 validator is suspended for the duration of a binary region.

---

## 17. Putting It Together: Realistic Examples

### 17.1 Hardware Configuration

```bovnar
# Sensor node configuration
.node_id        = <uint:8>  7;
.firmware_ver   = "2.4.1";

.radio = {
    .frequency  = <float:64,M~Hz>  868.1;
    .tx_power   = <sint:8,dB>     14;          # dB (power level); 'dBm' is not a bovnar unit
    .bandwidth  = <float:32,k~Hz>  125.0;
    .sf         = <uint:8>         7;          # spreading factor
};

.sampling = {
    .interval   = <uint:32,s>      60;
    .burst_size = <uint:16>        8;
    .adc_bits   = <uint:8>         12;
};

.calibration = {
    .offset     = <float_fix:32,q8,°C>  -0.5;
    .gain       = <float_fix:32,q16>     1.002;
};
```

### 17.2 Scientific Measurement Batch

```bovnar
.experiment = "thermal_runaway_001";
.timestamp  = <uint:64,s> 1715000000;

.measurements = [
    {.channel = <uint:8> 0;
     .value   = <float:32,°C>  27.3;
     .valid   = true;},
    {.channel = <uint:8> 1;
     .value   = <float:32,°C>  31.7;
     .valid   = true;},
    {.channel = <uint:8> 2;
     .value   = <float:32,°C> 112.4;
     .valid   = true;}
];

.threshold = <float:32,°C>  80.0;
.alarm     = triggered;
```

### 17.3 Multi-Dimensional Matrix

```bovnar
# 3×3 rotation matrix in float64
.R = <float:64> [1.0, 0.0, 0.0]
                /[0.0, 0.866, -0.5]
                /[0.0, 0.5,   0.866];
```

### 17.4 Mixed Binary and Text

<!-- bovnar-example: illustrative -->
```bovnar
.packet = {
    .type      = data_frame;
    .sequence  = <uint:32>  42;
    .timestamp = <uint:64,µ~s> 1715000000123456;
    .payload   = \x00\x01\x08\x00\xDE\xAD\xBE\xEF\xCA\xFE\xBA\xBE\x00;
    .crc32     = <uint:32,_16> "A1B2C3D4";
};
```

---

## 18. Common Mistakes and the Errors They Produce

Understanding the error codes is essential for debugging. The parser reports line, column, the offending byte, and a byte-offset into the stream.

**Annotation on the wrong side of `=`:**
<!-- bovnar-example: rejected -->
```bovnar
.x<uint:32> = 42;    # error_unexpected_input_byte (< after identifier)
```

**Non-decimal value not quoted:**
<!-- bovnar-example: rejected -->
```bovnar
.x = <uint:_16> FF;  # FF is parsed as a symbol → error_type_value_mismatch
```

**Signed value in unsigned type:**
<!-- bovnar-example: rejected -->
```bovnar
.x = <uint:8> -1;    # error_value_out_of_range
```

**Overflow:**
<!-- bovnar-example: rejected -->
```bovnar
.x = <uint:8> 256;   # error_value_out_of_range (max is 255)
```

**Float with an unsupported width:**
<!-- bovnar-example: rejected -->
```bovnar
.x = <float:8>  1.0; # error_illegal_value_type
.x = <float:12> 1.0; # error_illegal_value_type (not 16 or multiple of 32)
```

**Base parameter on `float_fix` or `float_dec`:**
<!-- bovnar-example: rejected -->
```bovnar
.x = <float_fix:32,q8,_10> 1.0;  # error_illegal_value_type
.x = <float_dec:64,_10>    1.0;  # error_illegal_value_type
```

**Q value too large for `float_fix`:**
<!-- bovnar-example: rejected -->
```bovnar
.x = <float_fix:16,q16> 1.0;  # error_illegal_value_type (Q must be < width)
```

**Empty unit component:**
<!-- bovnar-example: rejected -->
```bovnar
.x = <float:64,m//s> 1.0;    # error_unit_illegal (empty component between //)
```

**Too many unit components:** a compound unit may carry at most
`BVNR_MAX_UNIT_COMPONENTS` = **32**; a thirty-third is `error_unit_illegal`. Nine is fine:

```bovnar
.ok = <float:64,m*s*k~g*A*K*mol*cd*b*V> 1.0;  # nine components — valid
```

**Inline unit suffix inside array:**
<!-- bovnar-example: rejected -->
```bovnar
.x = [9.81 m/s, 3.14 m];  # error_unexpected_input_byte (units forbidden in arrays)
```

**Comma outside array:**
<!-- bovnar-example: rejected -->
```bovnar
.x = 42,;   # error_unexpected_input_byte
```

**Unknown escape sequence:**
<!-- bovnar-example: rejected -->
```bovnar
.x = "\q";   # error_illegal_escape_sequence (\q is not a defined escape)
```
(`\xHH` and `\u{…}` *are* defined in spec 1.1 — see the escapes section above;
they are rejected only in a 1.0 or unversioned document.)

**Unmatched struct close:**
<!-- bovnar-example: rejected -->
```bovnar
.x = 1;}   # error_illegal_struct_close (a stray '}' at the top level)
```

**A `/`-array's dimension rows must match:**
<!-- bovnar-example: rejected -->
```bovnar
.ok  = [[1, 2]/[3, 4]];          # valid — the /-array's two rows both have 2 elements
.bad = [[1, 2]/[3, 4, 5]];       # error_array_row_size_mismatch — 2 vs 3 (/-rows)
# note: [[1,2],[3,4,5]] is also rejected — sibling sub-arrays must match in length (1.0 homogeneity)
```

---

## 19. Error Recovery Mode

The parser supports an optional `continue_on_error` mode. When enabled, a parse error invokes the `on_error` callback and then enters a resync state machine that skips bytes while tracking bracket and brace nesting. The `recovery_count` counter (accessible via `bvnr_reader_get_recovery_count`) is incremented immediately when an error triggers entry into resync mode — not when the resync eventually completes. Normal parsing resumes at whichever comes first: a `;` at the original nesting depth, or the start of the next assignment — a `.` at that depth followed by a byte that can begin an identifier.

This mode is intended for log streams and unreliable transports — situations where one malformed assignment should not discard the rest of the file. For configuration and data serialization, disable it: fail fast and reject corrupt input entirely.

---

## 20. Size Limits

All limits are configurable via `bvnr_read_flags_t`. The defaults are intentionally permissive — 2 147 483 647 for array items and text bytes, and **unlimited** (`0`) for file size. Production deployments should set explicit caps:

| Field | Default | Suggested cap |
|---|---|---|
| `max_identifier_length` | 255 | 255 |
| `max_string_length` | 65535 | application-defined |
| `max_number_length` | 65535 | 65535 |
| `max_symbol_length` | 255 | 255 |
| `max_reference_length` | 65535 | 65535 |
| `max_array_items` | 2 147 483 647 | application-defined |
| `max_text_bytes` | 2 147 483 647 | application-defined |
| `max_file_size` | 0 (no total-byte cap) | `16777216` (16 MiB) |
| `max_array_nesting` | 0 (→64 internal) | 32 or less for most applications |
| `max_struct_nesting` | 0 (→64 internal) | 32 or less for most applications |

Setting most fields to `0` in `bvnr_read_flags_t` causes the reader to substitute an internal default — **64** for both nesting depths, and **2 147 483 647** for `max_array_items` and `max_text_bytes`. **`max_file_size` is the exception: `0` means no total-byte cap**. That is not endless: the text is still bounded by `max_text_bytes`, so an endless text stream raises that as well. Explicitly setting `max_file_size = 16777216` (16 MiB) is the recommended practice for production deployments.

---

## 21. Event Model (C API Overview)

The C reader is a two-phase, callback-driven SAX-style parser. You supply two optional callbacks: `on_unverified` receives events before semantic validation, `on_verified` receives them after. For normal consumption, `on_verified` is sufficient.

The event sequence for `.port = <uint:16> 443;` is:

```
ev_stream_start
ev_assignment_start            → key = "port"
ev_type_annotation_start       → raw = "uint:16"
ev_type_annotation_type_family → "uint"
ev_type_annotation_type_family_parameter → width: 16
ev_type_annotation_end
ev_data                        → value = "443", type = {uint, 16}
```

The `ev_type_annotation_type_family_parameter` event fires once per **present** parameter. Because the explicit annotation `<uint:16>` carries neither a base nor a unit parameter, only the width event is emitted. Contrast this with a bare `42;` (no annotation), where the validator **synthesises** `<uint:64,_10,no_unit>` and therefore emits three parameter events:

```
ev_stream_start
ev_assignment_start            → key = "count"
ev_type_annotation_start       → (synthesised)
ev_type_annotation_type_family → "uint"
ev_type_annotation_type_family_parameter → width: 64
ev_type_annotation_type_family_parameter → base: _10
ev_type_annotation_type_family_parameter → unit: no_unit
ev_type_annotation_end
ev_data                        → value = "42", type = {uint, 64}
```

The synthesised annotation always produces all three parameter events; explicit annotations only produce events for the parameters that are actually present.

**`ev_type_annotation_start` asymmetry:** This event is emitted to `on_unverified` first (with raw annotation bytes in `data`/`length` but no `value_type` or `value_unit` populated), and then separately to `on_verified` (with `value_type` and `value_unit` filled in). All other type-annotation events and `ev_data` are emitted to both callbacks simultaneously via the same call.

The `bvnr_data_t` structure passed with `ev_data` carries:

- `type` — the token type (`token_is_number`, `token_is_string`, `token_is_symbol`, `token_is_reference`, `token_is_array_number`, `token_is_array_string`, `token_is_bool`, `token_is_null_value`, `token_is_octet_stream`)
- `value_type` — the `value_type_spec_t` (family, width, base/Q)
- `value_unit` — the `value_unit_t` (up to `BVNR_MAX_UNIT_COMPONENTS` components)
- `data` — pointer to the raw value bytes
- `length` — byte count

For an octet stream, `ev_octet_stream_start` and `ev_octet_stream_end` bracket one or more `ev_data` events with `type == token_is_octet_stream`.

For arrays, `ev_array_row_start` opens each row, `ev_array_row_end` closes it, and `ev_array_dim_start` separates dimensions (the `/` rows).

For structs, `ev_struct_start` and `ev_struct_end` bracket the nested assignments. No `ev_data` event is emitted for the struct value itself.

---

## 22. Quick-Reference: Syntax Cheat Sheet

<!-- bovnar-example: illustrative -->
```bovnar
# ── Assignments ────────────────────────────────────────────────────
.key = value;
.key = <type-family:width,_base,unit> value;

# ── Type families ──────────────────────────────────────────────────
<uint:32>              # unsigned 32-bit integer
<sint:64>              # signed 64-bit integer
<float:32>             # IEEE 754 single-precision
<float:128>            # IEEE 754 quad-precision (or any multiple of 32)
<float_fix:32,q16>     # Q-format fixed-point, 32-bit, 16 fractional bits
<float_dec:64>         # IEEE 754-2008 decimal, 64-bit
<utf8>                 # UTF-8 string
<bool>                 # boolean (true/false); no parameters

# ── Units ──────────────────────────────────────────────────────────
<float:64,s>           # seconds
<float:64,k~m>         # kilometers (SI prefix~unit; `km` is the same unit)
<float:64,m/s>         # meters per second
<float:64,m/s²>        # meters per second squared
<float:64,k~g·m/s²>    # kilogram-meters per second squared
<uint:64,Ki~B>         # kibibytes
<float:64,no_unit>     # explicitly dimensionless

# ── Special values ─────────────────────────────────────────────────
nan  inf  ninf

# ── Booleans and null (reserved keywords) ──────────────────────────
.b = true;   .b = false;   .b = on;   .b = off;   # on==true, off==false
.n = null;   .n = ;                                # null keyword == empty

# ── Inline unit suffix (scalar context only) ───────────────────────
.x = 9.81 m/s;

# ── Non-decimal (must be a quoted string) ──────────────────────────
.x = <uint:_16> "DEADBEEF";
.x = <sint:_2>  "-10000000";

# ── Arrays ─────────────────────────────────────────────────────────
.a = [1, 2, 3];
.a = [1, 2, 3]/[4, 5, 6];     # 2D: two rows
.a = <float:32> [1.0, 2.0];   # whole-array annotation
.a = [<uint:8> 1, <sint:8> -1]; # per-element annotations
.a = [, 1, , 2, ];            # null, 1, null, 2, null

# ── Structs ────────────────────────────────────────────────────────
.s = {.x = 1; .y = 2;};
.s = {};

# ── Symbols ────────────────────────────────────────────────────────
.state = running;
.ok    = true;

# ── References ─────────────────────────────────────────────────────
.ref  = &.other_key;
.deep = &.config.db.host;

# ── Null ───────────────────────────────────────────────────────────
.x = ;
.x = <uint:32> ;

# ── Strings ────────────────────────────────────────────────────────
.s = "hello\nworld";
.s = "part one " "part two";   # concatenation
```

---

## See also

- [Specification](03_bovnar_spec.md) — the normative rules behind everything shown here
- [Unit & Currency Reference](05_bovnar_unit_system.md) — every unit and currency the parser accepts
- [Unit & Currency Cheat Sheet](04_bovnar_unit_cheatsheet.md) — the symbol tables, for looking one up quickly
- [Read & Write API](08_bovnar_readwrite_api.md) — the C API sketched in the event-model section
- [Python Bindings](09_bovnar_python_bindings.md) — the same document set, read and written from Python
- [Rust Bindings](14_bovnar_rust_bindings.md) — the same document set, read and written from Rust
- [FAQ](02_bovnar_faq.md) — the questions this tutorial raises next

---

*End of Bovnar — Tutorial (Bovnar spec 1.2).*
