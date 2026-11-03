# bovnar (Rust)

Rust bindings for **Bovnar (BVNR)** — a typed, unit-aware serialisation format
with a C99 reference implementation.

This crate is a thin wrapper around the upstream C library, modelled 1:1 on the
Python binding in `../python`. The C sources are compiled by `build.rs`; there
is no system library to install.

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

```rust
use bovnar::{parse, to_string, DomDoc, Value};

let doc = parse(b".host = \"api.example.com\"; .matrix = [1, 2]/[3, 4];")?;

let out = to_string(
    &Value::Struct(vec![("port".into(), Value::Int(443))]),
    true,
)?;

let dom = DomDoc::parse(&out)?;
assert_eq!(dom.get("port").and_then(|n| n.as_u64()), Some(443));
```

Lower level: `Reader` (streaming events) and `Writer` (streaming output into
memory). Errors carry the library's error code and its C name
(`Error::name()`, e.g. `"unexpected_input_byte"`). A `DomDoc::parse` error has
no position (line, column and offset are 0): the C DOM reports only the code.
Read the document with `Reader` to locate it.

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
