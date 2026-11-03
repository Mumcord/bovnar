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

## Building

Requirements: a C compiler and `python3` (the build runs the upstream table
generators, exactly as CMake does).

```bash
cargo build
cargo test
```

For `wasm32-wasip2`, point `WASI_SDK_PATH` at a wasi-sdk:

```bash
WASI_SDK_PATH=/opt/wasi-sdk cargo build --target wasm32-wasip2
```

## Versioning

The crate carries the version of the C library it wraps: `version` in
`Cargo.toml` is the `project(bovnar VERSION x.y.z)` of the top-level
`CMakeLists.txt`, and CI fails when the two differ. A release tag `vX.Y.Z`
names that same version.

## Publishing

The crate is not on crates.io and `Cargo.toml` keeps `publish = false`: as
it stands it cannot be packaged. `cargo publish` packs only `rust/`, while
`build.rs` compiles `../src` and `../include` and runs the `../gen_*.py`
generators with Python. Publishing would take:

- the C sources, headers and generated tables inside the package (vendored
  into `rust/` at package time, or the generated `.gen.h`/`.gen.c` files
  committed so the build needs no Python);
- `build.rs` reading them from inside the package;
- a release step, under the same tag == version gate as the GitHub release,
  with a crates.io token, and `publish = false` removed.

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
    let bvnr = bovnar::json::from_json(r#"{"name": "mumcord", "ports": [64738, 443]}"#)?;
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

### In a Mumcord plugin (wasm32-wasip2)

The crate compiles its C sources itself, so a WebAssembly build needs a C
compiler for WASI: [wasi-sdk](https://github.com/WebAssembly/wasi-sdk). Point
`WASI_SDK_PATH` at it and build for `wasm32-wasip2`:

```sh
rustup target add wasm32-wasip2
export WASI_SDK_PATH=/opt/wasi-sdk        # where wasi-sdk is unpacked
cargo build --release --target wasm32-wasip2
```

In the plugin's `Cargo.toml`, depend on a fixed commit of this branch, as
Mumcord does:

```toml
[dependencies]
bovnar = { git = "https://github.com/Mumcord/bovnar", rev = "<commit>" }
```

`python3` must be on `PATH` at build time: `build.rs` runs the upstream
table generators.

Lower level: `Reader` (streaming events) and `Writer` (streaming output into
memory). Errors carry the library's error code and its C name
(`Error::name()`, e.g. `"unexpected_input_byte"`; `Error::kind()` as an
`ErrorKind`). A `DomDoc::parse` error has no position (`Error::position()` is
`None`): the C DOM reports only the code. Read the document with `Reader` to
locate it.

`Reader`, `Writer` and `DomDoc` are `Send` but not `Sync`: as `bovnar.h`
(THREADING) allows, each can be moved to another thread, and distinct ones
can be used on distinct threads, but one object must not be shared.

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

The pint and NumPy bridges (Python-only libraries), `BvnFloat`
and the `Quantity` accessors built on it or on `Decimal`/`Fraction`
(`decimal`, `fraction`, `fixed_point`, `stored_value`, `ieee_bits`), and the
file-descriptor entry points (`to_fd`, `read_fd`, ...): read the file with
`std::fs` and use the in-memory functions.

Python test files with nothing to port, for that reason:
`test_decimal_tie_rounding.py` (both tests read the materialised decimal64
through `Quantity.stored_value`, which is built on `BvnFloat`). Single tests
that are not ported are listed at the top of each `tests/*.rs` port.

## License

MIT, like the C library.
