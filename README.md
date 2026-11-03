# Bovnar (BVNR)

**Unit-safe serialization for scientific and industrial systems — with a C99 reference implementation.**

[![Spec version](https://img.shields.io/badge/spec-1.2-blue)](doc/03_bovnar_spec.md)
[![License: MIT + third-party notices](https://img.shields.io/badge/license-MIT%20%2B%20notices-green)](LICENSE)
[![C standard](https://img.shields.io/badge/C-C99-orange)](CMakeLists.txt)
[![Website](https://img.shields.io/badge/web-bovnar.io-blue)](https://www.bovnar.io)

---

## Table of Contents

- [Links](#links)
- [Overview](#overview)
- [Key Features](#key-features)
- [Format at a Glance](#format-at-a-glance)
- [Where Bovnar Fits](#where-bovnar-fits)
- [Bovnar vs UCUM and CF](#bovnar-vs-ucum-and-cf)
- [Carrying other vocabularies](#carrying-other-vocabularies-unit-profiles-spec-12)
- [Repository Layout](#repository-layout)
- [Building](#building)
- [Command-Line Tool](#command-line-tool)
- [Running the Tests](#running-the-tests)
- [C API — Quick Start](#c-api--quick-start)
- [Python Bindings — Quick Start](#python-bindings--quick-start)
- [Syntax Highlighting](#syntax-highlighting)
- [Web Playground](#web-playground)
- [Use Cases](#use-cases)
- [Documentation](#documentation)
- [Citation](#citation)
- [License](#license)

---

## Links

- **Website:** https://www.bovnar.io
- **Prebuilt downloads:** https://github.com/sothis/bovnar/releases
- **IANA media type (`text/vnd.bovnar`):** https://www.iana.org/assignments/media-types/text/vnd.bovnar
- **DOI — Documentation and Specification (latest):** https://doi.org/10.5281/zenodo.21443295
- **DOI — Source (latest):** https://doi.org/10.5281/zenodo.21443008

---

## Overview

In scientific and industrial systems, the expensive failures are rarely bad syntax — they are unit confusion: a value sent in pounds-force and read as newtons, feet read as meters. The number parses fine; the dimension is wrong.

Bovnar closes that gap. Every value in a `.bvnr` document carries its own type family, bit-width, numeric base, and **physical unit** — inline, in the byte stream, with no external schema. The unit is not a comment or a naming convention; it is part of the value and is validated by the parser. Annotate a measurement as `m/s` and write a mismatched inline unit, and parsing fails with `error_unit_mismatch`. Hand the file to anyone and they have everything required to interpret — and to dimensionally trust — every reading.

<!-- bovnar-example: illustrative -->
```bovnar
#!bovnar 1.1
# The optional spec-version directive above carries the version and nothing
# else; a comment on that line is error_invalid_spec_version. Comments like
# this one, and the trailing ones below, are ordinary and inert.
# A self-describing configuration document
.config = {
    .host      = "api.example.com";
    .port      = <uint:16> 443;
    .limits    = {
        .timeout    = <float:64,s> 2.5;
        .max_packet = <uint:64,Mi~B> 16;
    };
};
.acceleration = 70.5 k~m·s⁻²;
.velocity     = <float:64,m/s> 9.81;
.price        = <float_dec:64,$USD> 19.99;
.btc_balance  = <uint:64,$BTC> 547820000;   # satoshis
.created      = <datetime:64,unix> 1750000000;   # a timestamp (spec 1.1)
.payload      = \x00 … binary stream … \x00;
.matrix       = [1, 2, 3]/[4, 5, 6];
.cell         = &.matrix[0][1];             # reference indexing (spec 1.1) → 2
```

---

## Key Features

- **Strong, optional typing** — seven core families (`uint`, `sint`, `float`, `float_fix`, `float_dec`, `utf8`, `bool`), plus `datetime` (spec 1.1), with explicit bit-width (`8`, `16`, `32`, `64`, …) and numeric base (`_2`, `_16`, `_36`, `_85`, …).
- **First-class physical units** — SI base units, derived SI units, and IEC binary prefixes. Compound units such as `m/s²`, `k~g·m/s²`, and `Gi~B` are written inline; no external schema is needed. A prefix may also be written the way everyone writes it — `kg`, `km`, `MHz`, `MiB`, `k$USD` — with `k~g` remaining the canonical output form.
- **Currency units** — 166 ISO 4217 fiat currencies and 50 cryptocurrencies are first-class units, written with a mandatory `$` sigil (`<float_dec:64,$USD> 19.99`, `<uint:64,$BTC> 547820000`), each carrying minor-unit metadata and prefix-validity rules.
- **Inline unit suffix** — `9.81 m/s` is valid without a full type annotation.
- **Native binary embedding** — Octet streams (`\x00 … \x00`) carry raw bytes without Base64 overhead. Chunks are length-prefixed, so there is no escaping, no expansion, and no byte a payload may not contain — and a reader can skip a binary region without inspecting it. The cost is that such a document is **not** transport-safe through anything that rewrites bytes: line-ending normalisation inside a payload desynchronises the length prefixes and the damage is unrecoverable. A consumer whose pipeline is text-shaped can refuse the binary half outright with `--text-only` (`bvnr_read_flags_t.text_only`), which fails at the stream's opening byte with `error_octet_stream_forbidden`.
- **Multi-dimensional arrays** — Rows separated by `/`; `[1,2,3]/[4,5,6]` is a native 2D structure.
- **Schema-free yet type-safe** — Omit annotations and get well-defined defaults (`uint:64`, `float:64`, …); add them and the parser validates on the fly.
- **Streaming SAX-style reader** — Incremental parsing from memory, a file descriptor, or a socket via a symmetric `on_unverified` / `on_verified` callback pair.
- **Error recovery** — Optional resync mode skips broken assignments and continues parsing — suitable for log streams and unreliable transports.
- **Python bindings** — Pure-`ctypes`, no compiled extension required. Exposes both a high-level `loads`/`dumps` dict-like API and a low-level event-driven streaming API.
- **Command-line tool** — `bovnar` validates, queries values by path, pretty-prints, converts to and from JSON, dumps the lexer/validator event stream, and benchmarks parsing throughput.
- **Browser playground** — the real C reference parser, compiled to WebAssembly (`bovnar_parser_wasm.js` over `bovnar_wasm_core.js`), runs the reference verified event stream (with full type/unit/value validation) in the browser and powers an interactive web playground.
- **Syntax highlighting** — Ready-made grammars for VS Code, Sublime Text, Geany, Vim, and CLion (JetBrains), all sharing one "cyberpunk" colour scheme with depth-cycling brackets.
- **Extensively tested** — Unit tests, socket-pair round-trip tests, a 470-case conformance suite, fuzz harnesses (reader, writer, DOM, utils), and a built-in benchmark mode (`bovnar bench`).

---

## Format at a Glance

| Construct | Syntax | Example |
|---|---|---|
| Assignment | `.key = value ;` | `.x = 42;` |
| Comment | `# … newline` | `# a remark` |
| Type annotation | `<family:width,base,unit>` before value | `<uint:32,_10,k~m> 1000` |
| Integer | `[-]digits` | `42`, `-7` |
| Float | `[-]digits[.digits][e[±]digits]` | `3.14`, `1e-6` |
| Special float | `nan` `inf` `ninf` | `.x = nan;` |
| Boolean | `true` `false` `on` `off` (`<bool>`) | `.b = on;` |
| String | `"…"` with C-style escapes (`\u{…}`/`\xHH` in spec 1.1) | `"hello\nworld"`, `"caf\u{e9}"` |
| Symbol | bare identifier (no quotes) | `ok`, `Monday` |
| Reference | `&.path.to.key`; array indexing `&.path[i][j]` (spec 1.1) | `&.config.host`, `&.matrix[0][1]` |
| Array | `[ … ]` rows separated by `/` | `[1,2]/[3,4]` |
| Struct | `{ .key = val; … }` | `{.x = 1; .y = 2;}` |
| Null | empty slot or `null` keyword | `.x = ;`, `.x = null;` |
| Octet stream | `\x00 … binary … \x00` | raw bytes |
| Version directive (spec 1.1) | `#!bovnar <major>.<minor>` (first comment) | `#!bovnar 1.1` |
| Datetime (spec 1.1) | `<datetime:width,epoch>` signed epoch seconds | `<datetime:64,unix> 1750000000` |
| Datetime literal (spec 1.1) | ISO-8601 (`Z` or `±HH:MM` offset, optional fraction); bare form infers `<datetime:64,unix>` | `2026-06-15T12:00:00+02:00` |

---

## Where Bovnar Fits

Bovnar is built for one thing: data where the **meaning must travel with the value**. Every value is annotated with its type family, bit-width, numeric base, and physical unit — inline, in the byte stream, without any external schema. A `.bvnr` file is self-describing at the individual value level: `<float:64,m/s> 9.81` carries more information than `9.81` can ever carry on its own. The format additionally supports native binary embedding through octet streams, avoiding the size and entropy cost of Base64, and first-class multi-dimensional array syntax that does not reduce to nested lists.

This makes Bovnar suited to contexts where dimensional correctness matters, where the receiving party may not share the sender's schema, or where text readability and binary payloads must coexist in the same document. It is built for the place where a wrong unit is a failure: scientific instrumentation and metrology, industrial telemetry and control, IoT sensor networks, long-term measurement archival, and mixed text-binary log streams.

Reach for Bovnar when unit-safety, precision, and self-description are requirements rather than nice-to-haves — when a value must mean exactly the same thing to its writer, its reader, and an archive opened decades later.

---

## Bovnar vs UCUM and CF

Anyone who has worked with units in earnest arrives with the same question: *why not just use UCUM?* Or, from the earth-science side: *isn't this what CF conventions are for?*

Neither is a rival, because neither sits at this layer. **UCUM** is a code system for the unit *string* — it says how a unit is spelled, and says nothing about where the string lives, what it is attached to, or who checks it. **CF** is a metadata convention layered over netCDF, which is the thing actually doing the serialization. Bovnar is the serialization format, with the unit inside the value grammar.

| | UCUM | CF Conventions | Bovnar |
|---|---|---|---|
| **What it is** | A code syntax for unit *strings* | A metadata convention over a foreign container | A serialization format with units in the value grammar |
| **Serializes data?** | No | No — netCDF does | Yes |
| **The unit binds to** | A string, wherever the application puts it | A variable (an entire array) | A single value |
| **Who validates, and when** | A UCUM library, if the application calls one | A separate checker, after the fact | The parser, while parsing, always |
| **External schema needed?** | n/a | Yes (standard-name table) | None |
| **Character set** | 7-bit ASCII, deliberately | ASCII (UDUNITS) | UTF-8 — `·`, `²`, `Ω`, `°C` |

### The difference is the enforcement point

This is the whole argument, and it is not about vocabulary size:

<!-- bovnar-example: rejected -->
```bovnar
.speed = <float:64,m/s> 9.81 k~m/h;
#                            ^ error_unit_mismatch, at parse time, for every consumer
```

Nothing had to be configured, no checker had to be run afterwards, and no application had to remember to call a units library. The same holds for assertions the *reader* makes without any callback:

```bash
$ bovnar validate --require-field '.a=$EUR' prices.bvnr    # the value is $USD
Validation failed: unit_mismatch at line 2, col 29

$ bovnar events --field '.v=k~m/h' speed.bvnr              # the value is 10.0 m/s
data  number  "10.0" <float:64,_10,m/s> → "36" k~m/h _10
```

With UCUM, validation happens when the application thinks to ask for it. With CF, it happens afterwards, in `cf-checker`, against a variable rather than a value. With Bovnar there is no *forgot to validate* — a unit that does not check out stops the parse.

### Per-value binding, not per-variable

CF attaches one `units` attribute to a whole netCDF variable. That is the right model for a homogeneous array and no model at all for a heterogeneous document: configuration next to measurements next to a binary payload next to timestamps. In Bovnar that is the normal case, and every value carries its own unit — including the elements of an array, which the validator holds to a single element type.

### Where Bovnar leads outright

**Time.** CF encodes time as a UDUNITS string with an embedded reference date (`days since 1970-01-01`), with the calendar in a separate attribute and, as of CF 1.12, a `units_metadata` attribute that *declares* what the producer did about leap seconds. Bovnar makes the epoch part of the type — so it is checked like any other dimension — and implements the IERS leap-second table rather than describing it:

```bovnar
#!bovnar 1.1
.utc_leap = <datetime:64,unix> 2016-12-31T23:59:60Z;   # → 1483228800
.utc_next = <datetime:64,unix> 2017-01-01T00:00:00Z;   # → 1483228800  (POSIX has no slot for it)
.tai_leap = <datetime:64,tai>  2016-12-31T23:59:60Z;   # → 1861920036
.tai_next = <datetime:64,tai>  2017-01-01T00:00:00Z;   # → 1861920037  (TAI does)
```

Nine epochs are supported — `unix`, `tai`, `gps`, `mjd`, `ntp`, `galileo`, `glonass`, `y2000`, `beidou` — and mixing them in one array is `array_element_type_mismatch`, because the epoch is part of the type, not a note about it.

**Currency.** Neither UCUM nor CF has monetary units at all. Bovnar treats 216 denominations as dimensions with the same machinery as physical units: `[<float_dec:64,$USD> 1, <float_dec:64,$EUR> 2]` is rejected, and a cross-currency conversion is never performed — a per-field rule asking for one fails with `error_unit_mismatch`, a target list leaves the amount in the currency it was written in — because the library carries no exchange-rate table and never will.

**Explicit prefix boundaries.** UCUM splits prefix from atom by longest-match and needs brackets to protect the units that lose that fight. Bovnar's `~` makes the boundary explicit, which separates three units that differ by one character and seven orders of dimension:

| Written | Means | Dimension |
|---|---|---|
| `pH` | the acidity scale | dimensionless (its own quantity kind) |
| `p~H` | picohenry | `m²·kg·s⁻²·A⁻²` |
| `ph` | phot | illuminance |

### Where Bovnar does not compete

In earth-system science, CF wins and will keep winning. The entire tool ecosystem — xarray, iris, cdo, cf-python — is bound to netCDF, CF has been developed in the open since 2001 with thousands of standard names, and its known weaknesses are priced in. Bovnar's registry of 264 physical units and 216 currencies is not going to displace that, and this README does not claim it will.

Bovnar's ground is where CF does not reach: heterogeneous documents rather than arrays, text and binary payloads in one file, configuration mixed with measurements, log streams, industrial telemetry, financial data with units. That is the space between JSON (no type, no unit) and netCDF (arrays, external schema, binary container).

### Carrying other vocabularies: unit profiles (spec 1.2)

Because these code systems sit at a different layer, they can be **components rather than alternatives**. Seven foreign notations are accepted in the unit slot, alongside the native one:

```bovnar
#!bovnar 1.2
.systolic = <float_dec:64,ucum:mm[Hg]>    120.00;   # UCUM        → mmHg
.mass     = <float:64,unece:KGM>           12.5;    # UN/ECE Rec 20 → k~g
.velocity = <float:64,qudt:M-PER-SEC>       9.81;   # QUDT        → m/s
.length   = <float:64,qudt-qk:Length>       3.0;    # a QUDT quantity KIND → m
.flux     = <float:64,udunits:kg*m-2*s-1>   0.5;    # UDUNITS / CF
.density  = <float:64,om:kilogramPerCubicmetre> 998.2; # OM 2       → k~g/m³
.temp     = <float:64,cf:air_temperature>  288.15;  # a CF standard name → K
.boxes    = <uint:32,unece:XBX>            12;      # Rec 21: a count of boxes
```

| Namespace | Vocabulary | Where it is spoken |
|---|---|---|
| `ucum:` | UCUM | HL7 FHIR, LOINC, clinical systems |
| `unece:` | UN/ECE Rec 20 / Rec 21 | EDIFACT, UBL, Peppol/EN 16931, ISO 20022, OPC UA |
| `qudt:` | QUDT unit local names | DTDL, Brick, ASHRAE 223P, digital twins |
| `qudt-qk:` | QUDT quantity kinds | the same, where metadata carries a *kind* rather than a unit |
| `udunits:` | UDUNITS-2 | netCDF and the CF conventions |
| `om:` | OM 2 — Ontology of units of Measure | agrifood, food science, LCA, FAIR/ELN data, Wikidata alignment |
| `cf:` | CF standard names (read-only) | netCDF/CF metadata, where a variable carries a standard name and no units string |

The notation is **spec 1.2** and, like every construct added after 1.0, is opt-in per document: it needs the `#!bovnar 1.2` directive, and without one `ucum:mm[Hg]` is simply not a unit. Native units are unaffected in every version.

A profile code is translated at parse time into exactly the same unit a native spelling produces, so nothing downstream can tell which notation was used. That holds *across* vocabularies too — `ucum:kg`, `unece:KGM`, `qudt:KiloGM`, `udunits:kg`, `om:kilogram` and `qudt-qk:Mass` are one and the same `value_unit_t`, and a cross-vocabulary suite of 4789 assertions over 64 concepts checks every spelling against every other one pairwise.

The enforcement point does not move. A profile code either becomes a real unit or becomes an error — there is no passthrough that would let an unchecked string reach a value:

```
ucum:mm[Hg]   →  mmHg                          translated
unece:KGM     →  k~g                           translated, from a different vocabulary
unece:XBX     →  unece:XBX                     a count of boxes: comparable, never convertible
ucum:[IU]     →  ucum:[IU]                     an assay unit: comparable, never convertible
ucum:metre    →  error_unit_illegal            not a UCUM atom
ucum:B[SPL]   →  error_unit_profile_unsupported valid UCUM, no representation here
udunits:days since 1970-01-01
              →  error_unit_profile_unsupported valid UDUNITS; an epoch is a type
                                                parameter here, not a unit
udunits:kg m-2 s-1
              →  k~g/m²·s                      UDUNITS multiplies with a SPACE,
                                                and this is CF's commonest
                                                spelling. Same unit as
                                                udunits:kg*m-2*s-1
udunits:ms-1  →  m~s⁻¹                         a reciprocal millisecond — also
                                                valid UDUNITS, and a different
                                                unit. The space is the difference
<float:64,k g>
              →  error_type_param_whitespace   a NATIVE unit has no space in it
nosuch:m      →  error_unit_profile_unknown    no such profile — and the same
                                                answer for a namespace this
                                                build was compiled without
```

**Each vocabulary is a build option.** `BVNR_WITH_UCUM_PROFILE` and its six siblings are `ON` by
default; turning them off drops that vocabulary's tables and nothing else — the base-unit ids, every
struct layout and every signature are identical in all configurations, so builds with different
switches stay ABI-compatible. The tables are the bulk of the library: with all seven off
`libbvnr.so` goes from 1.96 MB to **524 KB**, which is what makes the unit system usable on a sensor
node that reads no ontology. `bovnar version` prints what a build carries, and
`bvn_unit_profile_count()` / `bvn_unit_profile_name(i)` report it to a program.

Full specification, the transliteration tables, the collisions between namespaces (`st` is the stone natively and the *stere* in UCUM; UCUM's `B` is the bel where QUDT's `BYTE` is the byte), an explicit list of what does *not* map, and the cross-vocabulary suite: [doc/11_bovnar_unit_profiles.md](doc/11_bovnar_unit_profiles.md).

---

## Repository Layout

```
bovnar/
├── include/                 # Public C headers
│   ├── bovnar.h             # Primary API: reader, writer, events, types
│   ├── bovnar_dom.h         # DOM (tree) API
│   ├── bovnar_si_units.h    # SI / IEC unit API
│   ├── bovnar_currency.h    # Fiat + crypto currency API
│   ├── bovnar_stream.h      # Framing, multiplexing & document-in-document
│   ├── bvn_datetime.h       # Datetime / epoch helpers (spec 1.1)
│   ├── bvn_gregorian_date.h # Gregorian calendar conversions (spec 1.1)
│   ├── bvn_float.h          # Arbitrary-precision float (`bvn_float_t`)
│   └── bvn_int.h            # Arbitrary bit-width integer (`bvn_int_t`)
├── src/
│   ├── bovnar.c             # CLI entry point
│   ├── lexer/               # Tokeniser + state table
│   ├── validator/           # Semantic validation layer
│   ├── writer/              # Serialiser + canonicalising observer
│   ├── io/                  # FD / memory source & sink
│   ├── dom/                 # DOM builder and traversal
│   ├── stream/              # Framing / multiplexing on the event API
│   └── utils/               # SI units, currency, datetime, integer, float utilities
├── tests/                   # C unit, integration, conformance, and fuzz tests
├── python/
│   └── bovnar/              # Pure-ctypes Python bindings
│       ├── __init__.py      # loads / dumps / dom_parse / Reader / Writer
│       ├── reader.py
│       ├── writer.py
│       ├── dom.py
│       ├── stream.py        # Framing / multiplexing streaming API
│       ├── units.py
│       ├── currency.py
│       ├── quantity.py
│       ├── structs.py
│       ├── enums.py
│       ├── exceptions.py
│       ├── _ffi.py          # ctypes FFI layer
│       ├── _bvnfloat.py     # float16 / decimal-float helpers
│       └── _numpy.py, _pint_bridge.py, _pint_units.py   # optional NumPy / Pint bridges
├── examples/                # Annotated .bvnr example files
├── highlighter/
│   ├── vscode/              # VS Code TextMate grammar + theme
│   ├── sublime/             # Sublime Text syntax + colour scheme
│   ├── geany/               # Geany filetype definition
│   ├── vim/                 # Vim syntax + filetype plugin
│   └── clion/               # CLion / JetBrains TextMate-bundle installer (reuses the VS Code grammar)
├── web/                     # Browser playground + landing page
│   ├── index.html           # Playground + landing page
│   ├── bovnar_parser_wasm.js # JS wrapper exposing the WASM parser
│   └── bovnar_wasm_core.js   # C reference parser compiled to WASM
├── doc/
│   ├── 01_bovnar_tutorial.md
│   ├── 02_bovnar_faq.md               # Frequently asked questions
│   ├── 03_bovnar_spec.md              # Format specification (v1.2)
│   ├── 04_bovnar_unit_cheatsheet.md   # Units & currencies quick reference
│   ├── 05_bovnar_unit_system.md
│   ├── 06_bovnar_unit_policy.md       # Unit policy: validation & conversion
│   ├── 07_bovnar_unit_ambiguities.md  # Which spelling means what, and why
│   ├── 08_bovnar_readwrite_api.md
│   ├── 09_bovnar_python_bindings.md
│   ├── 10_bovnar_streaming.md         # Streaming, framing & multiplexing
│   ├── 11_bovnar_unit_profiles.md     # UCUM/UNECE/QUDT/UDUNITS/OM/CF codes in a unit slot
│   ├── 12_bovnar.ebnf                 # Formal EBNF grammar
│   ├── 13_bovnar_conformance.md       # Conformance test tool and IUT protocol
│   └── 14_bovnar_rust_bindings.md     # Rust crate: parse/to_string, DOM, Reader/Writer, exact numbers
├── CMakeLists.txt
└── CMakeLists_tests.txt
```

---

## Building

### Requirements

- CMake ≥ 3.21
- A C99-conforming compiler (GCC or Clang on Linux/macOS; **64-bit** MinGW64 or MSVC on Windows)
- No external library dependencies, at build time or runtime, beyond the C standard library (not even `libm`)

### Build the library and CLI tool

```bash
cmake -B build .
cmake --build build
```

This produces:

| Target | Path |
|---|---|
| Static library | `build/libbvnr.a` |
| Shared library | `build/libbvnr.so` |
| CLI binary | `build/bovnar` |

Build types: `Debug` (`-O0 -g3`), `Release` (`-O3 -flto`), `MinSizeRel` (`-Os`), `RelWithDebInfo` (`-O3 -g3 -flto`). `Release` and `RelWithDebInfo` enable link-time optimisation.

```bash
cmake -B build -DCMAKE_BUILD_TYPE=Release .
cmake --build build
```

Every build also regenerates the single-file amalgamation into `build/amalgamate/`
(`bovnar.h` + `bovnar.c`) and packs release archives into the build directory
(`<count>` is the git commit count):

| Archive | Contents |
|---|---|
| `bovnar-<version>-<count>-amalgamate.tar.xz` | the amalgamation (`bovnar.h`, `bovnar.c`), plus `LICENSE`, `THIRD_PARTY_NOTICES.md`, `README.md`, `doc/` and `examples/` |
| `bovnar-linux-<version>-<count>.tar.xz` | Linux libraries, CLI and headers, plus `LICENSE`, `THIRD_PARTY_NOTICES.md`, `README.md`, `doc/`, `examples/` and the editor `highlighter/` grammars |
| `bovnar-windows-msvc-<version>-<count>.zip`, `bovnar-windows-mingw-<version>-<count>.zip` | Windows libraries, CLI and headers, one archive per toolchain (the MinGW one also from the `BVNR_CROSS_MINGW` cross-build on a Linux host); each unpacks to a directory of its own name, plus `LICENSE`, `THIRD_PARTY_NOTICES.md`, `README.md`, `doc/`, `examples/` and the editor `highlighter/` grammars |

Disable with `-DBVNR_PACKAGE=OFF`.

These same archives — built and smoke-tested by CI, plus a `-source.tar.xz`
snapshot of the tag and a `SHA256SUMS` file — are attached to each
[GitHub release](https://github.com/sothis/bovnar/releases) from 1.1.0 onward,
so a build from source is optional. Earlier tags predate the packaging: 1.0.0
was POSIX-only and had no packer at all, and carries no assets.

### Build options

| Option | Default | Effect |
|---|---|---|
| `BVNR_HARDEN` | `ON` | Stack protector (CLI), `_FORTIFY_SOURCE=2`, and `-Wformat` security checks. |
| `BVNR_WERROR` | `OFF` | Promote all warnings to errors (for CI). |
| `BVNR_FUZZ_TEST` | `ON` | Build the self-contained fuzz harnesses and register them as CTest tests. |
| `BVNR_FUZZ_EXTERNAL` | `OFF` | Build libFuzzer / AFL++ targets (requires clang or afl-clang-fast). |
| `BVNR_PACKAGE` | `ON` | On each build, regenerate the amalgamation into `build/amalgamate/` and pack release archives into the build dir. |
| `BVNR_SANITIZE` | `OFF` | ASan + UBSan (`-fno-sanitize-recover=all`) plus `-ftrivial-auto-var-init=pattern` where the compiler has it. GCC/Clang only; a diagnostic build, never a release one. |
| `BVNR_ANALYZER` | `ON` | GCC `-fanalyzer` static analysis. Turn it off for release wheel builds, where newer GCCs in the manylinux image can crash on it. |
| `BVNR_LTO` | `ON` | Link-time optimisation for GCC `Release`/`RelWithDebInfo` builds (forced off under MinGW). |
| `BVNR_BUILD_TESTS` | `ON` | Build and register the C/Python test suite. |
| `BVNR_REGEN_TABLES` | `OFF` | Regenerate the unit, prefix and currency tables from `src/gendata/*.bvnr` unconditionally (they are regenerated when missing anyway). Needs only Python 3. |
| `BVNR_WITH_<NAME>_PROFILE` | `ON` | One per unit-profile vocabulary (`UCUM`, `UNECE`, `QUDT`, `QUDT_QK`, `UDUNITS`, `OM`, `CF`). Off drops that vocabulary's tables; the ABI does not change. |
| `BVNR_CROSS_MINGW` | `OFF` | On a Linux host with the MinGW-w64 toolchain, also build the Windows target into `build/mingw/` as part of `all`. |

```bash
cmake -B build -DBVNR_WERROR=ON .
```

**Running the sanitizer build.** The `bvnr_py_*` tests load `libbvnr.so` through
ctypes into an unsanitized interpreter, which ASan refuses outright, so either
skip them or preload the runtime:

```bash
cmake -S . -B build-asan -DBVNR_SANITIZE=ON -DCMAKE_BUILD_TYPE=Debug
ctest --test-dir build-asan -E '^bvnr_py_'          # C suite

LD_PRELOAD=$(gcc -print-file-name=libasan.so) ASAN_OPTIONS=detect_leaks=0 \
    LIBBOVNAR_PATH=$PWD/build-asan/libbvnr.so python3 -m pytest python
```

`-ftrivial-auto-var-init=pattern` is not a sanitizer and does not report
anything: it fills uninitialized automatic variables with a conspicuous pattern
so a read of one yields a distinctive value instead of whatever the stack held,
which makes a test that depends on such a read change its answer. GCC has no
MemorySanitizer; uninitialized *heap* reads are not covered by any of the above
and need a clang/MSan run.

### Link against the library

```bash
gcc my_app.c -I include -L build -lbvnr -o my_app
```

Both libraries share the base name `bvnr` (`libbvnr.a`, `libbvnr.so`), so `-lbvnr`
links the shared object when both are present. To link the static archive
instead, use `-l:libbvnr.a` (GNU ld) or static-link flags.

### Install (system / distro)

```bash
cmake -B build -DCMAKE_BUILD_TYPE=Release .
cmake --build build
cmake --install build --prefix /usr/local
```

This installs the shared and static libraries (with the versioned `libbvnr.so.2`
soname chain), the `bovnar` CLI, all public headers, a **pkg-config** file, and a
**CMake package**. Downstream projects can then use either:

```bash
# pkg-config
gcc my_app.c $(pkg-config --cflags --libs bvnr) -o my_app
```

```cmake
# CMake
find_package(bovnar REQUIRED)
target_link_libraries(my_app PRIVATE bovnar::bvnr)        # or bovnar::bvnr_static
```

The freedesktop MIME entry is installed too (disable with `-DBVNR_INSTALL_MIME=OFF`);
after install, run `update-mime-database <datadir>/mime` to register the `.bvnr` type.

### Windows (64-bit)

Both 64-bit toolchains are supported with the same `cmake` invocation; each
builds into its own subdirectory so the two can coexist in one checkout:

```bash
# MSVC (x64 Developer environment, or the Visual Studio generator)
cmake -B build/msvc -A x64 .
cmake --build build/msvc --config Release   # -> bvnr.dll + bvnr.lib, bvnr_static.lib, bovnar.exe

# MinGW64
cmake -B build/mingw -G Ninja -DCMAKE_BUILD_TYPE=Release .
cmake --build build/mingw                   # -> libbvnr.dll + libbvnr.dll.a, libbvnr.a, bovnar.exe
```

On MSVC the static archive is `bvnr_static.lib` because the DLL's import library
already takes `bvnr.lib`. On Windows the portable part of the C test suite still
builds and runs (the POSIX-only tests and the Python bindings are left out; a
MinGW cross build runs its tests under Wine), but the native MSVC CI job builds
with `-DBVNR_BUILD_TESTS=OFF`. The `Build & Package` CI workflow builds both
Windows toolchains and a native Linux target, smoke-tests the CLI, and publishes
the artifacts.
Consumers passing their own file descriptors to the fd source/sink on Windows
must open them in binary mode (`_O_BINARY`); the `bovnar` CLI does this itself.

---

## Command-Line Tool

The `bovnar` binary built above wraps the library for everyday use:

| Command | Description |
|---|---|
| `bovnar validate [opts] <file>` | Validate a `.bvnr` file; exit non-zero on the first error. `--require-unit` additionally rejects any numeric value that carries no unit; `--require-dimension <unit>` (repeatable) requires every numeric value to be validly convertible to one of the named units — "this document is lengths, in whatever unit it wrote them". `--require-field <path>=<unit>` (repeatable) asserts one key path rather than the whole document, and a path may end in `.*` to name a subtree. `--text-only` rejects a document containing an octet stream, for a consumer whose transport is not binary-safe. |
| `bovnar query [opts] <path> <file>` | Print a single **value** by dotted path, e.g. `.sensor.temperature` — the number alone, so it pipes into other tools; the unit is deliberately not printed, so read `25` from a `°C` field as 25 °C and not 25 K. Floats print as the shortest decimal that reads back as the same double; a `float_dec` or `float:128` wider than a double is rounded on the way through the DOM, so use the reader or the Python bindings when you need the stored digits verbatim. Takes the unit-policy options of `events` except `--base` (the value is printed in decimal) and `--text-only` (use `bovnar validate --text-only`), so a query can assert what it expects and ask for the unit it wants back: `--field .a.b=m`, `--require-unit`. |
| `bovnar pretty-print [--canonical] <file>` | Re-serialise a document in pretty form, keeping each unit's spelling. `--canonical` writes each unit in reduced form and rescales its value (`km` → `m`, `kg` → `g`; a prefix on a named derived unit such as `kN` stays, as `k~N`), so two spellings of one unit produce the same bytes; it fails with `value_out_of_range` rather than round where an integer cannot be rescaled. |
| `bovnar convert <file>` | Convert between `json` and `bvnr`; direction is auto-detected from the `.json`/`.bvnr` extension. Add `--from <fmt>` or `--to <fmt>` to override. bvnr → json is lossy for most documents — JSON has no units, types, symbols, references, octet streams, non-finite floats or datetimes — so the JSON is still written, what was lost is listed on stderr, and the exit status is 1: exit 0 means the JSON converts back to the same document. A datetime with sub-second digits is refused outright. json → bvnr refuses what Bovnar cannot hold (a number too large for a 64-bit float, a string with a control character, a key that is not an identifier) and names the key. |
| `bovnar events [opts] <file\|->` | Print the lexer (unverified) and validator (verified) event streams side by side. `-c` resync on error (the exit status is still non-zero if any occurred), `-d` debug re-serialisation, `-p` pretty debug output. Pass `-` to read stdin. (`-d` re-serialises the event stream and re-emits the source's `#!bovnar` version directive, so its debug output round-trips even for spec-1.1-gated values.) Unit policy: `--unit <unit>` (repeatable) converts values to the first unit they fit, `--si` converts anything left over to coherent SI base units, `--base <N>` renders conversions in base N, `--leave-inexact` leaves a value the conversion cannot deliver exactly rather than failing the parse. |
| `bovnar frames pack\|list <file…\|->` | Wrap each document in a length-prefixed frame, or list the documents in a frame stream. |
| `bovnar mux pack\|list <chan:file…\|->` | Multiplex files onto channels in one octet stream, or list the channel/message sizes in a multiplexed stream. |
| `bovnar version` | Print the library version and supported spec version. |
| `bovnar bench [options]` | Benchmark parsing throughput. `--profile <list>` picks the payload shape (`scalars,typed,structs,arrays,units,mixed`, or `all`), `--size <list>` the payload sizes in bytes (each payload is whole assignments up to that size). `--iterations <N>` parse rounds per cell (default 100) and `--warmup <N>` warm-up rounds (default 10); `--min-overhead` drops the `on_unverified` (lexer) callback and keeps only `on_verified`. `--verbose` for per-run detail, `--json` for machine-readable output. |

Exit status: `0` success; `1` the document, the input or the output failed; `2` the command line was wrong. Asked-for help (`--help`) goes to stdout.

```bash
bovnar validate examples/units.bvnr
bovnar query .system.host config.bvnr
bovnar convert data.json          # json -> bvnr (direction from extension)
bovnar convert data.bvnr          # bvnr -> json
cat data.bvnr | bovnar events -
bovnar events --si --leave-inexact sensors.bvnr   # show every value in SI
bovnar validate --require-unit sensors.bvnr      # refuse an unannotated number
bovnar query --field .inlet.temp=°C .inlet.temp sensors.bvnr   # one field, in °C
```

---

## Running the Tests

```bash
cmake -B build .
cmake --build build
cd build && ctest --output-on-failure
```

Or use the convenience wrapper at the repository root, which builds first, runs
every registered test, and then sweeps the fuzz harnesses:

```bash
./run_tests.sh                 # --no-fuzz to skip the sweep and the fuzz tests
```

### Test suite

| Binary / test | Coverage |
|---|---|
| `bvnr_reader_test` | Core reader, all token types |
| `bvnr_extended_reader_test` | Edge cases, resync, error recovery |
| `bvnr_writer_test` | Serialiser output |
| `bvnr_write_read_roundtrip_test` | Writer output re-read by the reader, value for value |
| `bvnr_socketpair_roundtrip_test` | Full round-trip over a POSIX socketpair |
| `bvnr_stream_test` | Framing, multiplexing, and document-in-document streaming |
| `bvnr_concurrency_test` | Independent readers and writers driven from several threads |
| `bvnr_dom_test` | DOM builder and traversal |
| `bvnr_si_test` | SI/IEC unit parsing and formatting |
| `bvnr_unit_ext_test` | Extended unit symbols, long-name aliases, prefix enforcement |
| `bvnr_unit_policy_test` | Unit policy: the `require_*` assertions, the target list, SI normalisation |
| `bvnr_want_unit_test` | The reader's `want_unit` conversion hook — factor exactness, output bases, refusal modes |
| `bvnr_ucum_test` | The `ucum:` notation: transliteration, the decade fold, refusals |
| `bvnr_unece_test` | The `unece:` notation: Rec 20 units, Rec 21 packages as incommensurable counts |
| `bvnr_qudt_test` | The `qudt:` and `qudt-qk:` notations: local names, and quantity kinds as coherent SI units |
| `bvnr_udunits_test` | The `udunits:` notation: operators, and the two refusals that carry weight |
| `bvnr_om_test` | The `om:` notation: OM 2 unit individuals, and which name a unit is written back as |
| `bvnr_cf_test` | The `cf:` notation: CF standard names read as their canonical unit; never written |
| `bvnr_unit_source_test` | A unit's spelling as written: delivered by the reader, re-emitted and checked by the writer, kept by the DOM |
| `bvnr_crossvocab_test` | Cross-vocabulary conformance: do the six unit vocabularies agree with each other? |
| `bvnr_currency_test` | Fiat and crypto currency lookup, minor units, prefix rules |
| `bvnr_utils_test` | Utility functions |
| `bvnr_int_test` | Arbitrary-precision integer arithmetic |
| `bvnr_float_test` | Floating-point representation |
| `bvnr_float_fix_dec_test` | Fixed and decimal float modes |
| `bvnr_datetime_test` | Datetime parsing, epochs, ISO-8601 literals, and Gregorian conversions |
| `bvnr_high_severity_test` | Robustness under malformed input |
| `bvnr_conformance` | 470-case conformance suite — self-test plus `--iut` adapter mode |
| `bvnr_fuzz_test --harness reader\|dom\|utils` | Randomised fuzzing of reader, DOM, and utils |
| `bvnr_fuzz_writer_test` | Randomised fuzzing of the serialiser |

CTest additionally runs every file in `examples/` through `bovnar events` and `bovnar validate` as smoke tests. Label filters narrow the run, e.g. `ctest -L unit`, `ctest -L conformance`, `ctest -L fuzz_deep`, or `ctest -L cli`.

### Python tests

Python binding tests are registered automatically when a Python 3 interpreter is found at configure time — no extra flag is required:

```bash
cmake -B build .
cmake --build build
cd build && ctest -L python --output-on-failure
```

Pure-Python tests (`ctest -L python_pure`) run without the shared library; integration tests (`ctest -L python_integration`) need `libbvnr.so`, whose path CTest injects automatically. To run pytest directly instead:

```bash
export LIBBOVNAR_PATH=$(pwd)/build/libbvnr.so
pip install -e ".[dev]"          # from the repository root — pyproject.toml is there
pytest python/tests -v
```

---

## C API — Quick Start

### Reading from memory

Callbacks are registered through `bvnr_read_flags_t`. Both `on_unverified` and `on_verified` receive `(void *userdata, bvnr_event_t ev, bvnr_data_t *d)` and must return `true` to continue parsing.

```c
#include <stdio.h>
#include <string.h>
#include "bovnar.h"

static bool on_event(void *userdata, bvnr_event_t ev, bvnr_data_t *d)
{
    (void)userdata;
    if (ev == ev_data)
        printf("value=%.*s\n", (int)d->length, (const char *)d->data);
    return true;
}

int main(void)
{
    const char *src = ".velocity = <float:64,m/s> 9.81;";
    bvnr_read_flags_t opts = {0};
    opts.on_verified = on_event;
    bvnr_reader_t *r = bvnr_reader_create();
    bvnr_open_read_mem(r, src, (uint64_t)strlen(src), NULL, 0, &opts);
    bvnr_read(r);
    bvnr_reader_destroy(r);
}
```

### Reading from a file descriptor

```c
bvnr_source_t src;
bvnr_source_from_fd(&src, fd);
bvnr_read_flags_t opts = {0};
opts.on_verified = on_event;
bvnr_open_read_source(r, &src, NULL, &opts);
bvnr_read(r);
```

### Writing

The high-level writer helpers accept a key string and a typed value directly:

```c
#include <stdio.h>
#include "bovnar.h"

int main(void)
{
    char buf[256];
    bvnr_writer_t *w = bvnr_writer_create();
    bvnr_sink_t sink;
    bvnr_sink_to_mem(&sink, (uint8_t *)buf, sizeof(buf));
    bvnr_open_write_sink(w, &sink, true, NULL);

    bvnr_write_float(w, "velocity", 64, 9.81);

    bvnr_write_finish(w);
    buf[bvnr_writer_bytes_written(w)] = '\0';   /* NUL-terminate for puts */
    bvnr_writer_destroy(w);
    puts(buf);
}
```

See [doc/08_bovnar_readwrite_api.md](doc/08_bovnar_readwrite_api.md) for the complete API reference, including the low-level `bvnr_write_event` interface and the full set of typed write helpers (`bvnr_write_uint`, `bvnr_write_sint`, `bvnr_write_float_unit`, etc.).

---

## Python Bindings — Quick Start

### Requirements

- **Python ≥ 3.10**
- The shared library `libbvnr.so` — bundled inside the wheel, so only a
  from-source install has to build it

### Installation

```bash
pip install bovnar                # wheels bundle libbvnr; nothing to build

pip install "bovnar[numpy]"       # optional NumPy bridge
pip install "bovnar[pint]"        # optional Pint bridge
pip install "bovnar[all]"         # both
```

From a source checkout instead — `pyproject.toml` lives at the repository root,
so the editable install runs there and not in `python/`:

```bash
cmake -B build . && cmake --build build

export LIBBOVNAR_PATH=$(pwd)/build/libbvnr.so

pip install -e ".[dev]"
```

### High-level API

```python
import bovnar

# Serialise
data = {"sensor_id": 42, "temperature": 36.6, "unit": "celsius"}
raw = bovnar.dumps(data)

# Deserialise
doc = bovnar.loads(raw)
print(doc["sensor_id"])      # 42
print(doc["temperature"])    # 36.6
```

### Streaming event-driven API

```python
import bovnar
from bovnar import Reader, Event

def on_event(ev, data):
    if ev == Event.DATA and data.value_unit.num_components:
        print(data.raw_str(), bovnar.unit_to_str(data.value_unit))

Reader().read_mem(b".velocity = <float:64,m/s> 9.81;", on_verified=on_event)
# → 9.81 m/s
```

Beyond `loads`/`dumps`, the package provides `dom_parse()` for random-access tree traversal, `loads(..., typed=True)` to preserve each value's exact type and unit as `Quantity` objects for lossless round-trips, and a `currency` module mirroring the C currency API.

See [doc/09_bovnar_python_bindings.md](doc/09_bovnar_python_bindings.md) for the full API. The Rust crate in `rust/` mirrors it name for name; see [doc/14_bovnar_rust_bindings.md](doc/14_bovnar_rust_bindings.md).

---

## Syntax Highlighting

### VS Code

```bash
cd highlighter/vscode && ./install.sh
```

### Sublime Text

```bash
cd highlighter/sublime && ./install.sh
```

Then choose **Preferences → Select Color Scheme… → Bovnar Cyberpunk**.

### Geany

```bash
cd highlighter/geany && ./install.sh
```

### Vim

```bash
cd highlighter/vim && ./install.sh
```

### CLion (JetBrains)

Quit CLion first — it rewrites its config on exit and would discard the change —
then run:

```bash
cd highlighter/clion && ./install.sh
```

This registers the shared VS Code grammar (`highlighter/vscode/bovnar-highlight/`)
as a TextMate bundle in every detected CLion config, giving full `source.bovnar`
highlighting. Start CLion and open a `.bvnr` file. The bundle is read live from
that path, so keep it in place. Works in other JetBrains IDEs (IntelliJ IDEA,
PyCharm, GoLand, …) too.

---

## Web Playground

The real C reference parser, compiled to WebAssembly (`web/bovnar_wasm_core.js`, wrapped by `web/bovnar_parser_wasm.js`), runs the reference verified event stream in the browser (in resync mode, so a malformed assignment is skipped rather than fatal) with full type/unit/value validation, and drives the interactive playground (`web/index.html`). It is the reference implementation itself, not a separate approximation. Serve the `web/` directory and open it — no build step required:

```bash
cd web && ./httpd.sh          # python3 -m http.server
# then open http://localhost:8000/
```

---

## Use Cases

- **Scientific and measurement data** — numbers with explicit physical units travel with the payload.
- **Financial and crypto data** — monetary amounts carry their ISO 4217 or cryptocurrency unit and minor-unit precision inline, removing ambiguity between currencies and scales.
- **Embedded systems / IoT** — the streaming parser has a small, allocation-friendly footprint; no heap required for the lexer itself.
- **Hardware and software configuration** — typed values eliminate range ambiguity (e.g., `<uint:16>` for a port number).
- **Mixed text + binary payloads** — log entries with attached raw memory dumps, firmware images, or sensor frames.
- **Multi-dimensional data** — matrices, image frames, time-series batches, any tabular structure where rows are a natural unit.
- **Long-term archival** — self-describing data stays interpretable without an out-of-band schema.

---

## Documentation

| Document | Description |
|---|---|
| [Tutorial](doc/01_bovnar_tutorial.md) | Practical, hands-on introduction to the format. |
| [FAQ](doc/02_bovnar_faq.md) | Frequently asked questions covering the format, type system, units, C API, Python bindings, and limits. |
| [Specification (v1.2)](doc/03_bovnar_spec.md) | Full lexical and syntactic grammar, type system, arrays, structs, octet streams, validation rules, and formal EBNF. |
| [Units & Currencies Cheat Sheet](doc/04_bovnar_unit_cheatsheet.md) | Quick reference for every physical unit, 166 fiat currencies, and 50 cryptocurrencies, with prefix tables and symbol-disambiguation rules. |
| [Unit & Currency Reference](doc/05_bovnar_unit_system.md) | SI and IEC prefixes, base units, compound units, exponents, C API, and validation rules. |
| [Unit Policy](doc/06_bovnar_unit_policy.md) | What a unit policy declares, the order its parts resolve in, what it validates and converts, and the errors it raises — on the reader and the writer. |
| [Unit Ambiguities](doc/07_bovnar_unit_ambiguities.md) | Every token that could mean two things — what the parser reads it as, and how to write the other meaning. Case traps, look-alike characters, same-dimension quantities, and abbreviations that are deliberately not units. |
| [Read & Write API](doc/08_bovnar_readwrite_api.md) | Complete C API for streaming readers and writers with annotated examples. |
| [Python Bindings](doc/09_bovnar_python_bindings.md) | Pure-ctypes Python interface: high-level `loads`/`dumps`, streaming `Reader`/`Writer`, unit helpers. |
| [Streaming, Framing & Multiplexing](doc/10_bovnar_streaming.md) | Endless streams, multi-document framing, octet multiplexing, and document-in-document — applications layered on the event API. |
| [Unit Profiles](doc/11_bovnar_unit_profiles.md) | Writing UCUM, UN/ECE, QUDT, UDUNITS, OM 2 and CF codes in a Bovnar unit slot: the transliteration tables, the collisions between the namespaces, what has no representation, and the cross-vocabulary conformance suite. |
| [Formal EBNF](doc/12_bovnar.ebnf) | Machine-readable grammar. |
| [Conformance Test Tool](doc/13_bovnar_conformance.md) | Conformance suite (470 cases), IUT protocol for verifying third-party implementations, TAP output, and CTest integration. |
| [Rust Bindings](doc/14_bovnar_rust_bindings.md) | The Rust crate: `parse`/`to_string` over a dynamic `Value`, `Reader`/`Writer`, the DOM, exact numbers and JSON conversion. |

See [`CHANGELOG.md`](CHANGELOG.md) for what changed between versions (including the additive specs 1.1 and 1.2, and which spec each library release implements).

---

## Citation

To cite Bovnar in published work, use the concept DOI, which always resolves to the newest archived release:

> Sonntag, J. *Bovnar*. https://doi.org/10.5281/zenodo.21443008

The documentation and specification are archived separately, under their own concept DOI: https://doi.org/10.5281/zenodo.21443295

To pin a particular release instead, use its version DOI — [1.1.0 source](https://doi.org/10.5281/zenodo.21443009), [1.1.0 documentation and specification](https://doi.org/10.5281/zenodo.21443296).

Machine-readable metadata lives in [`CITATION.cff`](CITATION.cff): GitHub's *Cite this repository* button reads it, and `cffconvert` renders it as BibTeX, APA or RIS.

---

## License

| Component | License |
|---|---|
| Source code (C, Python, CMake) | [MIT](LICENSE) |
| Documentation (`doc/`) | [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/) |
| Examples (`examples/`) | [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/) |
| Third-party vocabulary identifiers in the unit profiles | their publishers' — see [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) |
| Other third-party material (fonts, JS libraries, imagery, toolchain output, one derived algorithm) | their authors' — see [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) Part 2 |

Copyright © 2026 Janos Sonntag.

The unit-profile tables carry identifier strings from UCUM, QUDT, OM 2,
UDUNITS-2, the CF standard name table and UN/ECE Recommendations 20 and 21.
Those identifiers belong to their publishers, and the MIT grant above does not
extend to them; [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md) records what
was taken from each, from which published version, under which license, and
which questions are still open. Part 2 of the same file inventories everything
else the project ships that it did not author. Redistributing Bovnar — in source
or in binary form — means carrying that file along with it.
