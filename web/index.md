# Bovnar (BVNR)

**Unit-safe serialization for scientific and industrial systems — with a C99 reference implementation.**

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

## Documentation

- [Tutorial](https://www.bovnar.io/doc/01_bovnar_tutorial.md) — A guided introduction to writing and reading Bovnar documents.
- [FAQ](https://www.bovnar.io/doc/02_bovnar_faq.md) — Common questions on types, units, limits, encoding, and the API.
- [Specification](https://www.bovnar.io/doc/03_bovnar_spec.md) — The complete format specification (spec 1.2): grammar, types, units, limits.
- [Unit & currency cheatsheet](https://www.bovnar.io/doc/04_bovnar_unit_cheatsheet.md) — Every unit symbol, prefix, and currency code in one reference table.
- [Unit system](https://www.bovnar.io/doc/05_bovnar_unit_system.md) — The 264 physical units, SI/IEC prefixes, and 216 currencies, with dimensions and factors.
- [Unit policy](https://www.bovnar.io/doc/06_bovnar_unit_policy.md) — What a policy validates and converts, the order it resolves in, and the errors it raises.
- [Unit ambiguities](https://www.bovnar.io/doc/07_bovnar_unit_ambiguities.md) — Which reading wins when a token could mean two things, and how to write the other.
- [Read & Write C API](https://www.bovnar.io/doc/08_bovnar_readwrite_api.md) — The C reader/writer/DOM API reference (bovnar.h, bovnar_dom.h).
- [Python bindings](https://www.bovnar.io/doc/09_bovnar_python_bindings.md) — The pure-ctypes Python package: loads/dumps, streaming, DOM, Quantity, NumPy/Pint bridges.
- [Streaming & framing](https://www.bovnar.io/doc/10_bovnar_streaming.md) — Octet streams, frames, multiplexing/demultiplexing, and embedded documents.
- [Unit profiles](https://www.bovnar.io/doc/11_bovnar_unit_profiles.md) — The ucum:, unece:, qudt:, qudt-qk:, udunits:, om: and cf: notations, their transliteration tables, what they refuse, and how they are checked against each other.
- [EBNF grammar](https://www.bovnar.io/doc/12_bovnar.ebnf) — The formal grammar, annotated against the reference lexer/validator.
- [Conformance](https://www.bovnar.io/doc/13_bovnar_conformance.md) — The 470-case conformance suite and the IUT protocol for third-party implementations.
- [Rust bindings](https://www.bovnar.io/doc/14_bovnar_rust_bindings.md) — The Rust crate: parse/to_string, streaming reader/writer, DOM, Quantity and exact numbers, JSON.

Browse the documentation as HTML at [https://www.bovnar.io/docs/](https://www.bovnar.io/docs/). A single-file copy of the whole set is at [https://www.bovnar.io/llms-full.txt](https://www.bovnar.io/llms-full.txt).
