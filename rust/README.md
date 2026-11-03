# bovnar (Rust)

Rust bindings for **Bovnar (BVNR)** — a typed, unit-aware serialisation format
with a C99 reference implementation.

This crate is a thin wrapper around the upstream C library, modelled 1:1 on the
Python binding in `../python`. The C sources are compiled by `build.rs`; there
is no system library to install.

**It mirrors the Python binding; it is not an idiomatic Rust API.** That is a
deliberate choice: the same names, the same entry points and the same
behaviour as `../python`, so the two bindings stay as simple as each other and
a change upstream maps onto both the same way. Expect Python's shapes
(`parse`/`to_string` over a dynamic `Value`, `Reader` callbacks, `Writer`
events) rather than serde integration, builders or typed deserialisation.
Where Rust's types can say more than Python's at no cost to that mirror, they
do: the C enums are Rust enums, and errors carry the library's error kind.

## Versioning

The crate carries the version of the C library it wraps, currently **2.0.0**:
`version` in `Cargo.toml` is the `project(bovnar VERSION x.y.z)` of the
top-level `CMakeLists.txt`, and CI fails when the two differ. A release tag
`vX.Y.Z` names that same version. The minimum supported Rust version is
**1.88** (`rust-version`), which CI builds and tests.

## Building

Requirements: Rust 1.88 or later, a C compiler, and `python3` on `PATH` (the
build runs the upstream table generators, exactly as CMake does).

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
    // Python's loads(): a plain Value tree.
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
    // Python's dumps(): a Value tree to a document.
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

### JSON

```rust
fn main() -> Result<(), bovnar::Error> {
    let bvnr = bovnar::json::from_json(r#"{"name": "example", "ports": [8080, 443]}"#)?;
    let json = bovnar::json::to_json(&bvnr)?;
    assert_eq!(bovnar::json::from_json(&json)?, bvnr);
    Ok(())
}
```

### Enums

The C enums are Rust enums named like Python's `enums.py` members, in
CamelCase (`BaseUnit.METER` is `BaseUnit::Meter`). Raw C values convert with
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

`Error` mirrors Python's exception classes (`Parse`, `Write`, `Argument`,
and `OutOfMemory` for `MemoryError`). `kind()` gives the library's error
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

- **Plain values:** `parse` (Python's `loads`), `parse_typed`
  (`loads(typed=True)`, keeping types and units in `Quantity`), `to_string`
  (`dumps`) and `write_array` over the dynamic `Value`.
- **DOM:** `DomDoc::parse` / `parse_with_policy`, then `get`, `lookup`
  (dotted paths), `entries` and `DomNode` accessors; `to_value` fails rather
  than inventing a value for a node it cannot read.
- **Streaming:** `Reader` (`read_mem`, `read_mem_want_unit`, `iter_mem`, a
  `UnitPolicy`) and `Writer` (typed `write_*` calls, structs, arrays and raw
  `emit`). The writer's arguments are typed: `Base` (the bases bovnar has an
  alphabet for, 2..62, 64, 85; `Base::DECIMAL`, `Base::HEX`), `Signedness`
  (`Signed`, `Unsigned`, `Auto` for Python's `signed=True/False/None`) and
  `EmitSpec` (Python's `emit` keyword arguments, with a table in its docs).
- **Units:** `Unit`, `UnitComponent` (a `BaseUnit`, an `Exponent` and a
  `Prefix`), the `units` and `currency` modules.
- **Enums:** `Event`, `TokenType`, `ValueTypeFamily`, `ErrorCode`,
  `BaseUnit`, `PrefixSystem`, `SIPrefix`, `IECPrefix`, named like Python's
  members in CamelCase, with `TryFrom<i32>` (an unknown value is an
  `UnknownValue` error) and `From<_> for i32`. `BaseUnit` also covers the unit
  profiles' opaque ids (`UcumIu` for `ucum:[IU]`), which Python's leaves out.
  `Exponent` and `Base` are open ranges, so they are checked newtypes.
- **Errors:** `Error` mirrors Python's exception classes: `Parse`, `Write`,
  `Argument` and `OutOfMemory` (`MemoryError`). `kind()` is the library's
  error name as an `ErrorKind` wherever it reported one (an unparsable unit
  or a refused policy included), `name()` its C spelling, and `position()`
  the line, column and offset where the input was refused. A `DomDoc::parse`
  error has no position: the C DOM reports only the code, so read the
  document with `Reader` to locate it.
- **JSON:** `json::from_json`, `json::to_json` and `json::detect`, mirroring
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

- `cargo test` runs the Rust tests: 32 of Python's 38 test files are ported
  one to one, one `tests/*.rs` file each with the Python test names; the ABI
  check (`test_abi.py`) is a unit test in `src/ffi.rs`, because the mirrored
  structs are private. Tests that could not be ported are listed at the top
  of each port, with the reason.
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
wasi-sdk. The release job does not wait for them, so the C archives and the
PyPI release never depend on the Rust jobs.

The crate is **not published to crates.io**, and `Cargo.toml` keeps
`publish = false`: as it stands it cannot be packaged. `cargo publish` packs
only `rust/`, while `build.rs` compiles `../src` and `../include` and runs the
`../gen_*.py` generators with Python. Publishing would take:

- the C sources, headers and generated tables inside the package (vendored
  into `rust/` at package time, or the generated files committed so the build
  needs no Python);
- `build.rs` reading them from inside the package;
- a release step, under the same tag == version gate as the GitHub release,
  with a crates.io token, and `publish = false` removed.

## Layout

| Rust            | Python          |
|-----------------|-----------------|
| `src/ffi.rs`    | `_ffi.py`, `structs.py` (the only `unsafe` code) |
| `src/reader.rs` | `reader.py`     |
| `src/writer.rs` | `writer.py`     |
| `src/dom.rs`    | `dom.py`        |
| `src/enums.rs`  | `enums.py`      |
| `src/error.rs`  | `exceptions.py` |
| `src/unit.rs`   | `unit.py`       |
| `src/units.rs`  | `units.py` and the unit functions of `__init__.py` |
| `src/quantity.rs` | `quantity.py` |
| `src/currency.rs` | `currency.py` (read from the C catalogue) |
| `src/stream.rs` | `stream.py`     |
| `src/json.rs`   | none (mirrors the CLI's `bovnar convert` JSON <-> BVNR) |
| `src/lib.rs`    | `__init__.py` (`parse`/`parse_typed`/`to_string` = `loads`/`loads(typed=True)`/`dumps`, `write_array`) |

## Not mirrored

The pint and NumPy bridges (Python-only libraries, so `test_numpy_bridge`,
`test_pint_bridge`, `test_pint_factor_parity` and
`test_example_numpy_roundtrip` have no port), `BvnFloat` and the `Quantity`
accessors built on it or on `Decimal`/`Fraction` (`decimal`, `fraction`,
`fixed_point`, `stored_value`, `ieee_bits`; so `test_decimal_tie_rounding`,
whose two tests read `stored_value`, has none either), and the
file-descriptor entry points (`to_fd`, `read_fd`, ...): read the file with
`std::fs` and use the in-memory functions.

## License

MIT, like the C library.
