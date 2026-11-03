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
(`Error::name()`, e.g. `"unexpected_input_byte"`).

## Layout

| Rust            | Python          |
|-----------------|-----------------|
| `src/ffi.rs`    | `_ffi.py`, `structs.py` (the only `unsafe` code) |
| `src/reader.rs` | `reader.py`     |
| `src/writer.rs` | `writer.py`     |
| `src/dom.rs`    | `dom.py`        |
| `src/enums.rs`  | `enums.py`      |
| `src/error.rs`  | `exceptions.py` |
| `src/lib.rs`    | `__init__.py` (`parse`/`to_string` = `loads`/`dumps`) |

## Not yet bound

Units and quantities, currencies, streams, unit policies, typed loads, and
arrays / integers wider than 64 bits in `to_string`.

## License

MIT, like the C library.
