# bovnar-rs

A pure-Rust reader, writer and serde integration for
[Bovnar](https://github.com/sothis/bovnar), a typed, unit-aware text data
format with an optional binary half (octet streams). It implements
specification version 1.2 and is built for parsing untrusted network input.

The library crate is called `bovnar-rs`; its library name is `bovnar`.

## Status

- Functionally complete for the reading, writing and streaming parts of the
  specification listed below.
- Behaviour is pinned to the C reference implementation at upstream commit
  `1bcf03b`: the upstream conformance suite passes byte-identically (see
  [Conformance](#conformance)), and a differential tester compares both
  implementations on generated and mutated input in CI.
- Version 0.1.0, not released. The API may still change.
- Not published on crates.io; use it as a git dependency (see
  [Installation](#installation)).

## Installation

Pin a commit with `rev`:

```toml
[dependencies]
bovnar-rs = { git = "https://github.com/Mumcord/bovnar", rev = "<commit>" }
```

For `no_std` targets, or to pick unit profiles:

```toml
[dependencies]
bovnar-rs = { git = "https://github.com/Mumcord/bovnar", rev = "<commit>", default-features = false, features = ["serde", "profile-unece"] }
```

## Quick start

Parse a whole document into a tree and query it:

```rust
let doc = bovnar::parse(b".server = { .host = \"localhost\"; .port = <uint:16> 64738; };")?;
assert_eq!(doc.lookup(".server.port").and_then(|v| v.as_u64()), Some(64738));
```

Build a document in code and write it:

```rust
use bovnar::{Document, Struct};

let mut server = Struct::new();
server.insert("host", "localhost");
server.insert("port", 64738u16);
let mut doc = Document::new();
doc.insert("server", server);
assert_eq!(
    bovnar::to_string(&doc)?,
    ".server = {\n\t.host = \"localhost\";\n\t.port = <uint:16> 64738;\n};\n"
);
```

Stream events in constant memory, feeding input in chunks of any size. With
`std`, `stream::ReadAdapter` pulls from any `std::io::Read`:

```rust
use bovnar::{Event, ReadOptions, Reader};

let mut reader = Reader::with_options(&ReadOptions::new().strict_version(true));
let mut keys = 0;
for chunk in [&b".a = 1; .b"[..], &b" = 2;"[..]] {
    reader.feed(chunk)?;
    while let Some(event) = reader.next_event() {
        if let Event::AssignmentStart { .. } = event {
            keys += 1;
        }
    }
}
reader.finish()?;
assert_eq!(keys, 2);
```

Map Rust data with serde (feature `serde`):

```rust
#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
struct Peer {
    name: String,
    volume: f32,
}

let peer = Peer { name: "alice".into(), volume: 0.75 };
let text = bovnar::serde::to_string(&peer)?;
assert_eq!(bovnar::serde::from_str::<Peer>(&text)?, peer);
```

Several documents travel over one connection with the length-prefixed record
framing in `bovnar::stream` (`write_frame`, `FrameDecoder`, and with `std`
`FrameReader` and `read_doc_stream`).

Convert JSON to Bovnar and back, exactly as the C CLI's `bovnar convert`
does, and sniff which of the two a buffer holds:

```rust
use bovnar::json::{self, LossKind};
use bovnar::{detect, Format, Limits};

let input = br#"{"name": "probe", "temps": [20.5, 21.0]}"#;
assert_eq!(detect(input), Format::Json);
let text = json::to_bovnar(input, &Limits::default())?;
assert_eq!(text, ".name = \"probe\";\n.temps = [20.5, <float> 21];\n");

let out = json::from_bovnar(b".speed = 3 m/s;\n", &Limits::default())?;
assert_eq!(out.json, "{\n  \"speed\": 3\n}\n");
assert_eq!(out.loss.count(LossKind::Unit), 1); // JSON has no units
```

## Feature flags

| Feature | Default | Adds |
|---------|---------|------|
| `std` | yes | `std::io` adapters: `IoSink`, `stream::ReadAdapter`, `stream::FrameReader`, `stream::read_doc_stream` |
| `serde` | no | the `bovnar::serde` module, and `Serialize`/`Deserialize` for `Limits` |
| `profile-unece` | yes | UN/ECE Recommendation 20 unit profile (`unece:`) |
| `profile-qudt` | yes | QUDT unit profile (`qudt:`) |
| `profile-qudt-qk` | yes | QUDT quantity-kind profile (`qudt-qk:`) |
| `profile-udunits` | yes | UDUNITS-2 unit profile (`udunits:`) |
| `profile-om` | yes | Ontology of units of Measure profile (`om:`) |

A unit in the namespace of a profile that is not compiled in is refused with
`unit_profile_unknown`, exactly as the C reference does for a profile switched
off at build time.

## Specification coverage

Section references are to the upstream documentation at the pinned commit
(`doc/03_bovnar_spec.md` and the documents next to it).

| Area | Supported | Notes |
|------|-----------|-------|
| Encoding: UTF-8, BOM handling | yes | |
| Version directive (`#!bovnar 1.2`) | yes | `strict_version` read option refuses unsupported versions |
| Keys and key paths | yes | |
| Strings and escapes | yes | |
| Symbols | yes | |
| References | yes | |
| Numbers (lossless literal text) | yes | |
| Type families (all 7), widths and bases | yes | |
| Datetime: epochs and ISO-8601 literals | yes | |
| Arrays, rows and homogeneity | yes | |
| Structs | yes | |
| Octet streams | yes | chunk boundaries are not kept in the tree (see below) |
| Default type synthesis | yes | |
| Units: SI and IEC prefixes, compound units, currencies | yes | 264 base units, 216 currencies; `bovnar::units` |
| Profile `unece` | yes | feature `profile-unece` |
| Profile `qudt` | yes | feature `profile-qudt` |
| Profile `qudt-qk` | yes | feature `profile-qudt-qk` |
| Profile `udunits` | yes | feature `profile-udunits` |
| Profile `om` | yes | feature `profile-om` |
| Profile `ucum` | no | licence of the source data unclear |
| Profile `cf` | no | licence of the source data unclear |
| Read options (`strict_version`, `text_only`, `recovery`) | yes | |
| Error recovery | yes | streaming `Reader` only |
| Record framing (doc/10 §2.1) | yes | `bovnar::stream` |
| Document-in-document embedding | yes | `stream::embed_document` |
| Octet multiplexing (doc/10 §3) | no | |
| Unit policy (doc/06) | no | |
| `want_unit` conversion on read | no | units can be compared and checked for convertibility |
| Canonicalising observer | no | |
| `unit_source` round trip | no | |

| Tool feature | Supported | Notes |
|--------------|-----------|-------|
| JSON to Bovnar (`bovnar convert --from json`) | yes | `bovnar::json::to_bovnar`, `to_document`; byte-identical output and the same refusals |
| Bovnar to JSON (`bovnar convert --to json`) | yes | `bovnar::json::from_document`, `from_bovnar`; byte-identical output, loss report as `json::Loss` |
| Format detection | yes | `bovnar::detect` (not in the C CLI, which goes by file extension) |

## JSON conversion

`bovnar::json` reproduces `bovnar convert` of the C CLI:

```rust
pub fn to_bovnar(json: &[u8], limits: &Limits) -> Result<String, JsonError>;
pub fn to_document(json: &[u8], limits: &Limits) -> Result<Document, JsonError>;
pub fn from_document(doc: &Document) -> Result<Json, JsonError>; // Json { json, loss }
pub fn from_bovnar(input: &[u8], limits: &Limits) -> Result<Json, JsonError>;
```

- **JSON to Bovnar.** The root must be an object. Objects become structs,
  arrays become bracket arrays (nested arrays stay nested), integers
  `<uint>` or `<sint>` (64-bit), numbers with a fraction or exponent
  `<float>`, plus strings, booleans and null. Refused, with the CLI's
  diagnostics: keys that are not Bovnar identifiers, control characters and
  `\u0000` in strings, integers beyond 64 bits, floats that overflow,
  trailing content, more than 512 nested containers, and anything the
  Bovnar reader would refuse (duplicate keys, mixed or ragged arrays,
  objects of differing shape in one array) as `JsonError::Unrepresentable`.
- **Bovnar to JSON** is lossy: units, numeric types and bases, symbols and
  references (strings), octet streams (hex strings), non-finite floats
  (`null`), datetimes (epoch seconds), integers wider than 64 bits (decimal
  strings) and `/` rows (nested arrays) do not survive. `Json::loss` counts
  each kind with a first example; the CLI exits 1 exactly when it is not
  empty. A datetime with sub-second digits is refused.
- **Round trip.** JSON that converts comes back with the same keys in the same
  order and the same values; float spellings are normalised (`2.0` comes
  back as `2`).
- **Limits.** The parser is iterative; input size follows
  `Limits::max_input_bytes`, output is capped at eight times the input plus
  64 KiB (the CLI's buffer), and the writer and the checking reader use the
  nesting limits of `limits`. The CLI itself allows 255 levels of nesting,
  the default `Limits` 64.

`bovnar::detect(&[u8]) -> Format` (`Json`, `Bovnar`, `Unknown`) looks at the
first significant byte after a BOM, whitespace and `#` comments, within a
64 KiB prefix: `{` or `[` is JSON, `#!bovnar`, a comment or `.key` is Bovnar.
It is a cheap guess, not a validation.

## Conformance

The upstream suite has 470 cases. Run against the C reference's own
conformance runner (`bvnr_conformance --iut`) with the `bovnar-iut` adapter:

- 444 pass byte-identically.
- 26 deviate only because the UCUM and CF profiles are absent. The C
  reference built without those profiles (`-DBVNR_WITH_UCUM_PROFILE=OFF
  -DBVNR_WITH_CF_PROFILE=OFF`) produces exactly the same output for them.
  Their ids are marked `profile_off_deviation` in
  `tests/conformance/corpus.json`.

CI checks this with `scripts/conformance-c.sh` and runs the differential
tester (`scripts/diff.sh`, 20 000 inputs) against the same C build.

JSON conversion is compared with the C CLI's `bovnar convert` by
`scripts/json-diff.sh`: the upstream JSON fixtures, the edge cases in
`tests/json/inputs`, every corpus input and upstream example, and generated
and mutated documents in both directions agree byte for byte on output,
exit status, error class and loss counts. `tests/json.rs` checks the same
against expectations captured from the CLI (`tests/json/expected.json`).
Places where the specification and the C reference disagree are listed in
[UPSTREAM_NOTES.md](UPSTREAM_NOTES.md); bovnar-rs follows the C reference.

## Differences from the C implementation

- **No UCUM or CF profiles.** `ucum:` and `cf:` units fail with
  `unit_profile_unknown`, as in a C build with both profiles off.
- **Input size and tree size caps.** `Limits::max_input_bytes` (16 MiB by
  default, `0` for unlimited streams) bounds the input, and
  `Limits::max_dom_nodes` bounds the number of values in a parsed tree. Both
  report the error `file_too_long`.
- **Stricter writer.** The writer refuses events that would produce a
  document its own reader rejects, in some places where the C writer accepts
  them. In those cases the error code can differ from what the C writer
  reports, or the C writer reports none. Examples: an empty struct under a
  type annotation is refused at the struct start with `type_value_mismatch`;
  an untyped integer after a datetime is checked against the inherited
  datetime type.
- **Array row counting in the writer fixed.** The C writer counts a nested
  row as two elements and therefore refuses some valid documents and writes
  some invalid ones; bovnar-rs counts rows the way both readers do.
- **Octet chunking is not preserved in the tree.** A parsed octet stream is a
  single byte vector; the chunk boundaries of the input are not kept and are
  not reproduced when writing the tree.
- **`parse_with_options` refuses recovery.** A tree built from a document
  with skipped parts would silently miss values, so `ReadOptions::recovery`
  is rejected with `invalid_argument` there and offered on the streaming
  `Reader` only.
- **No `error_none`.** Errors are returned as `Result`; `ErrorKind` mirrors
  the C error codes in order, without `error_none`. `ErrorKind::as_str()`
  returns the same names as the C `bvn_error_to_string`.

## Safety

- No `unsafe` code (`#![forbid(unsafe_code)]`).
- No panics on any input, and no recursion on input depth in the reader, the
  tree builder, the writer or when dropping a tree. (Serde deserialization
  follows serde's recursive visitor model; its depth is bounded by the
  nesting limit.)
- Every resource is bounded by `Limits`: input size, lengths of strings,
  numbers, identifiers and symbols, nesting depth (64 by default), array
  items and tree nodes. Frame payloads are capped (256 MiB by default) and
  buffers grow with the bytes that arrive, never with a declared length.
- Fuzzing: `cargo fuzz` targets for the reader, the tree, the writer, round
  trips, units, JSON conversion and format detection live in `fuzz/`
  (`scripts/fuzz.sh`).
- Differential testing against the C reference: `diff/` and
  `scripts/diff.sh`.

## `no_std` and WebAssembly

The crate is `#![no_std]` and needs only `alloc`. Disable default features
for targets without `std`, such as `wasm32-unknown-unknown`, and re-enable
the unit profiles you need. Everything except the `std::io` adapters stays
available, serde support included. CI builds `wasm32-unknown-unknown`
(without `std`, with and without `serde`) and `wasm32-wasip2` (default and all
features).

## Minimum supported Rust version

Rust 1.81, as declared by `rust-version` in `Cargo.toml` and checked in CI.

## Development

| Script | Purpose |
|--------|---------|
| `scripts/build-cref.sh <upstream> <build>` | build the C reference with UCUM and CF off |
| `scripts/conformance-c.sh <build>` | run the upstream conformance suite against `bovnar-iut` |
| `scripts/diff.sh <upstream> <build> [count] [seed]` | differential test against the C reference |
| `scripts/json-diff.sh <upstream> <build> [count] [seed] [capture]` | differential test of JSON conversion against `bovnar convert` |
| `python3 -I scripts/gen-tables.py <upstream> [--check]` | regenerate or check the unit tables in `src/units/gen/` |
| `scripts/fuzz.sh <upstream> [seconds] [jobs] [target...]` | run the fuzz targets (nightly, cargo-fuzz) |

`<upstream>` is the root of this repository (`..` from `rust/`): the C reference,
its conformance corpus and the unit data live there. The fixtures were captured at
upstream commit `1bcf03b`.

## Licence and attribution

bovnar-rs is licensed under the [MIT licence](LICENSE).

Bovnar, its specification, conformance corpus and unit data are the work of
Janos Sonntag ([sothis/bovnar](https://github.com/sothis/bovnar), MIT; see
[vendor/upstream/LICENSE](vendor/upstream/LICENSE)). The unit vocabularies
compiled into the profiles come from third parties; their notices are in
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
