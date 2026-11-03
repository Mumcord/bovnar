# bovnar (Rust)

Rust bindings for **Bovnar (BVNR)** — a typed, unit-aware serialisation format
with a C99 reference implementation.

This crate is a thin wrapper around the upstream C library. The C sources are
compiled by `build.rs`; there is no system library to install.

The API stays close to the C library: `parse`/`to_string` over a dynamic
`Value`, a DOM, `Reader` callbacks and `Writer` events, rather than serde
integration, builders or typed deserialisation. The C enums are Rust enums,
and errors carry the library's error kind.

## Versioning

The crate carries the version of the C library it wraps, currently **2.0.0**:
`version` in `Cargo.toml` is the `project(bovnar VERSION x.y.z)` of the
top-level `CMakeLists.txt`, and CI fails when the two differ. A release tag
`vX.Y.Z` names that same version. The minimum supported Rust version is
**1.88** (`rust-version`), which CI builds and tests.

## Building

Requirements: Rust 1.88 or later, a C compiler, and Python 3 at build time (the
build runs the upstream table generators, exactly as CMake does).

The crate depends on `num-bigint`, `num-rational`, `num-integer` and
`num-traits` for exact numbers: wide integers, exact fractions and the
coefficient of `Decimal`.

```bash
cargo build
cargo test
```

### Building for WebAssembly (wasm32-wasip2)

The crate compiles its C sources itself, so a WebAssembly build needs a C
compiler for WASI: [wasi-sdk](https://github.com/WebAssembly/wasi-sdk). Point
`WASI_SDK_PATH` at it and build for `wasm32-wasip2`:

```sh
rustup target add wasm32-wasip2
export WASI_SDK_PATH=/opt/wasi-sdk        # where wasi-sdk is unpacked
cargo build --release --target wasm32-wasip2
```

The same applies to a crate that depends on this one and is built for
`wasm32-wasip2`: set `WASI_SDK_PATH` for its build.

## Usage

The examples below are compiled and run as doctests (`cargo test --doc`).

### Read

```rust
use bovnar::{DomDoc, Event, EventData, ReadOptions, Reader, Value, parse};

fn main() -> Result<(), bovnar::Error> {
    // A plain Value tree.
    let doc = parse(b".host = \"api.example.com\"; .port = <uint:16> 443;")?;
    let Value::Struct(fields) = doc else { unreachable!() };
    assert_eq!(fields[1], ("port".to_string(), Value::Int(443)));

    // The DOM: look values up, with their units.
    let dom = DomDoc::parse(b".link = { .length = 1.5 k~m; };")?;
    let length = dom.lookup("link.length").expect("present");
    assert_eq!(length.as_float(), Some(1.5));
    assert_eq!(length.unit(), bovnar::Unit::parse("k~m")?);

    // Streaming: one callback per event.
    let mut keys = Vec::new();
    let mut on_event = |d: &EventData<'_>| {
        if d.event == Event::AssignmentStart {
            keys.push(String::from_utf8_lossy(d.raw).into_owned());
        }
        true
    };
    Reader::new()?.read_mem(
        b".a = 1; .b = 2;",
        &ReadOptions::default(),
        Some(&mut on_event),
        None,
    )?;
    assert_eq!(keys, ["a", "b"]);
    Ok(())
}
```

### Write

```rust
use bovnar::{Base, Value, Writer, to_string};

fn main() -> Result<(), bovnar::Error> {
    // A Value tree to a document.
    let out = to_string(&Value::Struct(vec![("port".into(), Value::Int(443))]), false)?;
    assert_eq!(bovnar::parse(&out)?, bovnar::parse(b".port = 443;")?);

    // The streaming writer: typed scalars, structs, arrays.
    let mut w = Writer::to_mem(4096, false)?;
    w.write_uint("flags", 0xff, 8, Base::HEX)?;
    w.begin_struct("server")?;
    w.write_string("host", "example.com")?;
    w.end_struct()?;
    w.finish()?;
    let back = bovnar::parse(w.get_output())?;
    let Value::Struct(fields) = back else { unreachable!() };
    assert_eq!(fields[0], ("flags".to_string(), Value::Int(255)));
    Ok(())
}
```

### Files and I/O streams

The I/O entry points take any `std::io::Read` or `std::io::Write`: a `File`,
a pipe, a socket, a `Cursor`.

```rust
use std::io::Cursor;
use bovnar::{Base, DomDoc, ReadOptions, Reader, Writer};

fn main() -> Result<(), bovnar::Error> {
    let path = std::env::temp_dir().join(format!("bovnar-readme-{}.bvnr", std::process::id()));
    let mut w = Writer::to_writer(std::fs::File::create(&path).unwrap(), false)?;
    w.write_uint("port", 443, 16, Base::DECIMAL)?;
    w.finish()?; // the document reaches the file here
    drop(w);

    let dom = DomDoc::parse_reader(std::fs::File::open(&path).unwrap())?;
    assert_eq!(dom.lookup("port").and_then(|n| n.as_u64()), Some(443));
    std::fs::remove_file(&path).unwrap();

    Reader::new()?.read_reader(Cursor::new(b".a = 1;"), &ReadOptions::default(), None, None)?;

    let value = bovnar::parse_reader(Cursor::new(b".a = 1;"))?;
    let mut out = Vec::new();
    bovnar::write_to(&value, false, &mut out)?;
    assert_eq!(bovnar::parse(&out)?, value);

    let mut framed = Vec::new();
    bovnar::stream::dump_documents_to(&[value.clone(), value.clone()], false, &mut framed)?;
    let docs = bovnar::stream::load_documents_reader(Cursor::new(framed), false, false, 0)?;
    assert_eq!(docs, vec![Some(value.clone()), Some(value)]);
    Ok(())
}
```

### JSON

```rust
fn main() -> Result<(), bovnar::Error> {
    let bvnr = bovnar::json::from_json(r#"{"name": "example", "ports": [8080, 443]}"#)?;
    let json = bovnar::json::to_json(&bvnr)?;
    assert_eq!(bovnar::json::from_json(&json)?, bvnr);
    Ok(())
}
```

### Exact numbers

`Quantity::decimal`, `fraction`, `fixed_point`, `stored_value` and
`ieee_bits` read a float at the precision its format carries. Exact fractions
are `num_rational::BigRational`; exact decimals are `bovnar::Decimal` (sign,
coefficient, exponent, or infinity or NaN), which compares numerically and
prints with the to-scientific-string conversion of General Decimal
Arithmetic.

```rust
use bovnar::{Decimal, Quantity, Value, parse_typed};

fn main() -> Result<(), bovnar::Error> {
    let doc = parse_typed(b".p = <float_dec:64> 3.141592653589793238462643383279503;")?;
    let Value::Struct(fields) = doc else { unreachable!() };
    let Value::Quantity(p) = &fields[0].1 else { unreachable!() };
    // The literal, every digit of it...
    let literal: Decimal = "3.141592653589793238462643383279503".parse()?;
    assert_eq!(p.decimal()?, Some(literal));
    // ...and the value decimal64 stores: 16 digits, rounded half to even.
    assert_eq!(p.stored_value()?.map(|d| d.to_string()), Some("3.141592653589793".into()));
    assert_eq!(p.ieee_bits()?.len(), 8);

    // Decimal and Fraction values are written exactly, as float_dec:64.
    let half = num_rational::BigRational::new(1.into(), 2.into());
    let out = bovnar::to_string(&Value::Struct(vec![("h".into(), Value::Fraction(half))]), false)?;
    assert_eq!(out, b".h=<float_dec:64>0.5;");
    Ok(())
}
```

### Enums

The C enums are Rust enums whose variants are the C enumerators in
CamelCase, without their prefix (`bu_meter` is `BaseUnit::Meter`). Raw C values convert with
`TryFrom<i32>`, which refuses an unknown value, and back with `i32::from`.
`Exponent` and `Base` are open ranges, so they are checked newtypes with the
common values named.

```rust
use bovnar::{BaseUnit, Exponent, SIPrefix, Unit};

fn main() -> Result<(), bovnar::Error> {
    let km = Unit::si(BaseUnit::Meter, SIPrefix::Kilo, Exponent::LINEAR);
    assert_eq!(km.to_string(), "k~m");
    assert_eq!(Unit::parse("k~m")?, km);
    let c = km.components()[0];
    assert_eq!((c.base, c.prefix), (BaseUnit::Meter, SIPrefix::Kilo.into()));
    assert_eq!(BaseUnit::try_from(i32::from(BaseUnit::Meter)), Ok(BaseUnit::Meter));
    assert_eq!(BaseUnit::Meter.name(), "METER");
    Ok(())
}
```

### Errors

`Error` has a variant per kind of failure (`Parse`, `Write`, `Argument`,
`OutOfMemory`, `Io`). `kind()` gives the library's error
name as an `ErrorKind` wherever it reported one, `position()` where the
input was refused.

```rust
use bovnar::{Error, ErrorKind, Unit};

let err = bovnar::parse(b".a = 1;\n.b = @;").unwrap_err();
assert_eq!(err.kind(), Some(ErrorKind::UnexpectedInputByte));
assert_eq!(err.position().map(|p| p.line), Some(2));

match Unit::parse("furlong per fortnight") {
    Err(Error::Argument { code, message }) => {
        assert!(code.is_some(), "the library said why: {message}");
    }
    other => panic!("expected an argument error, got {other:?}"),
}
```

## API overview

- **Plain values:** `parse`, `parse_typed` (keeping types and units in
  `Quantity`), `to_string` and `write_array` over the dynamic `Value`.
- **DOM:** `DomDoc::parse` / `parse_with_policy`, then `get`, `lookup`
  (dotted paths), `entries` and `DomNode` accessors; `to_value` fails rather
  than inventing a value for a node it cannot read.
- **Streaming:** `Reader` (`read_mem`, `read_mem_want_unit`, `iter_mem`, a
  `UnitPolicy`) and `Writer` (typed `write_*` calls, structs, arrays and raw
  `emit`). The writer's arguments are typed: `Base` (the bases bovnar has an
  alphabet for, 2..62, 64, 85; `Base::DECIMAL`, `Base::HEX`), `Signedness`
  (`Signed`, `Unsigned`, `Auto`) and `EmitSpec` (the options of `emit`, with
  a table in its docs).
- **Files and I/O streams:** `Reader::read_reader` and
  `read_reader_want_unit`, `DomDoc::parse_reader` and
  `parse_reader_with_policy`, `Writer::to_writer` and `to_writer_with`,
  `parse_reader`, `parse_typed_reader` and `write_to` (the twins of `parse`,
  `parse_typed` and `to_string`), and in `stream` `frame_documents_to`,
  `dump_documents_to`, `load_documents_reader`, `mux_dump_to`,
  `mux_load_reader`, `embed_document_to` and `parse_embedded_reader` take
  any `std::io::Read` / `std::io::Write`; a path is read or written through
  an opened `std::fs::File`. The C library takes no callback source or
  sink, so they go through memory: the input is read in full (to one byte
  past `max_file_size` when it is set, and at most 256 MiB for the DOM, the
  cap of `bvn_dom_parse_fd`; `parse_reader` and the `stream` readers read
  with no `max_file_size`, and the stream readers then apply
  `max_document_size` and `max_message` as their in-memory twins do), and
  the writer's document is written out by `finish`, from a memory sink of
  `DEFAULT_MEM_CAP` bytes unless `to_writer_with` sets another; the other
  writer twins build their output in memory and then write it out. A failing reader or writer is an
  `Error::Io` with the code the C fd source or sink gives
  (`reading_from_source_fd`, `writing_to_sink`). No file descriptor is
  involved, so they work the same on Windows.
- **Exact numbers:** the `Quantity` accessors `decimal`, `fraction`,
  `fixed_point`, `stored_value` and `ieee_bits`; `Decimal` and
  `BigRational`, also as the `Value` variants `Decimal` and `Fraction`,
  which `to_string` writes as `float_dec:64`; `Quantity::from_number` taking an
  `ExactNumber` (a literal, `Decimal`, `BigRational`, `BigInt` or `f64`).
  `BvnFloat` and the `bvnfloat` module reach the library's
  arbitrary-precision float: the IEEE binary and decimal interchange
  encodings and the fixed-point datum, plus the exact materialisers used
  past the C parser's exponent cap. The `stored_value` of a `float_fix` is
  exact, however wide its datum (as is the literal of a terminating
  fraction).
- **Units:** `Unit`, `UnitComponent` (a `BaseUnit`, an `Exponent` and a
  `Prefix`), the `units` and `currency` modules.
- **Enums:** `Event`, `TokenType`, `ValueTypeFamily`, `ErrorCode`,
  `BaseUnit`, `PrefixSystem`, `SIPrefix`, `IECPrefix`, with the C
  enumerators in CamelCase, `TryFrom<i32>` (an unknown value is an `UnknownValue` error) and
  `From<_> for i32`. `BaseUnit` also covers the unit profiles' opaque ids
  (`UcumIu` for `ucum:[IU]`).
  `Exponent` and `Base` are open ranges, so they are checked newtypes.
- **Errors:** `Error` has the variants `Parse`, `Write`, `Argument` and
  `OutOfMemory`, plus `Io` for a failing
  `std::io::Read` / `Write`. `kind()` is the library's
  error name as an `ErrorKind` wherever it reported one (an unparsable unit
  or a refused policy included), `name()` its C spelling, and `position()`
  the line, column and offset where the input was refused. A `DomDoc::parse`
  error has no position: the C DOM reports only the code, so read the
  document with `Reader` to locate it.
- **JSON:** `json::from_json`, `json::to_json` and `json::detect`, matching
  the CLI's `bovnar convert`. JSON has one number type, so two values change
  on the way back, as with the CLI: a float with an integral value becomes an
  integer (`1e+6` is written `1000000` and reads back as `uint`), and `-0.0`
  is written `-0`, which reads back as `0`. From the second trip on the JSON
  is stable.
- **Streams:** `stream` frames several documents, multiplexes octet
  channels and embeds documents in documents.
- **Threads:** `Reader`, `Writer` and `DomDoc` are `Send` but not `Sync`: as
  `bovnar.h` (THREADING) allows, each can be moved to another thread, and
  distinct ones can be used on distinct threads, but one object must not be
  shared.

## Tests

- `cargo test` runs the unit tests in `src/` and the integration tests in
  `tests/`, one file per area (reader, writer, DOM, units, currencies, exact
  floats, datetimes, streams, round trips and more). The ABI checks, which
  compare the private FFI structs with the C compiler's layout, are unit
  tests in `src/ffi.rs`. Integers wider than `i128` are tested through
  `Quantity`: the exact literal under a `uint` / `sint` annotation, read back
  exactly with `parse_typed` or `DomNode::as_int_str`.
- `test_writers_agree` compares `to_string` with the C CLI's `pretty-print`,
  so it is `#[ignore]`d unless the CLI is built at `<repo>/build/bovnar`:
  `cmake -B build . && cmake --build build --target bovnar`, then
  `cargo test --test test_writers_agree -- --ignored`.
- The README's examples run as doctests (`cargo test --doc`).
- The C library's own suite runs unchanged through CMake:
  `cmake -B build . && cmake --build build && ctest --test-dir build`.

## Fuzzing

`fuzz/` holds [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets
(nightly toolchain): `parse` (the streaming parsers and a write-back round
trip), `dom` (parse and walk every node) and `json_roundtrip` (`from_json`
and `to_json` until the JSON is stable). Instrument the C library too by
building it with clang:

```sh
cd fuzz
CC=clang CFLAGS="-fsanitize=fuzzer-no-link,address" \
    cargo +nightly fuzz run -O parse -- -max_total_time=600
```

## CI and releases

The Rust jobs run in `.github/workflows/build-and-package.yml`, next to the
C library's: a version check (crate version = `CMakeLists.txt` version),
fmt, clippy `-D warnings` and the tests on Linux x64 (with the CLI for
`test_writers_agree`), Linux aarch64, macOS, Windows MSVC x64 and Windows
MinGW64, a build and test on Rust 1.88, and a `wasm32-wasip2` build with
wasi-sdk. The release job does not wait for them, so the C release archives
never depend on the Rust jobs.

The crate is **not published to crates.io**, and `Cargo.toml` keeps
`publish = false`: as it stands it cannot be packaged. `cargo publish` packs
only `rust/`, while `build.rs` compiles `../src` and `../include` and runs the
`../gen_*.py` generators. Publishing would take:

- the C sources, headers and generated tables inside the package (vendored
  into `rust/` at package time, or the generated files committed so the build
  needs no table generators);
- `build.rs` reading them from inside the package;
- a release step, under the same tag == version gate as the GitHub release,
  with a crates.io token, and `publish = false` removed.

## License

MIT — see [LICENSE](https://github.com/sothis/bovnar/blob/main/LICENSE).

The `libbvnr` this crate compiles from the repository's C sources embeds
Bovnar's unit-profile tables, which carry identifier strings from UCUM, QUDT,
OM 2, UDUNITS-2, the CF standard name table and UN/ECE Recommendations 20 and
21. Those identifiers belong to their publishers and the MIT grant does not
extend to them; the notices are at
[`THIRD_PARTY_NOTICES.md`](https://github.com/sothis/bovnar/blob/main/THIRD_PARTY_NOTICES.md).
