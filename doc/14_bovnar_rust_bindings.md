# Bovnar — Rust Bindings

> **Spec version:** 1.2
> **Status:** Reference — the Rust crate as implemented
> **Scope:** Installation, the high-level and streaming APIs, the DOM, `Quantity` and the exact numbers, JSON conversion, and the thread rules.

Rust bindings for the **Bovnar (BVNR)** typed serialisation library (library
2.0.0, format spec 1.2).

The crate is a thin wrapper around the C library: `build.rs` compiles the C
sources into the crate, so there is no system library to install and nothing is
looked up at run time. The API stays close to the C library's own shapes:
`parse` / `to_string` over a dynamic `Value`, `Reader` callbacks, `Writer` calls
and a random-access DOM. There is no serde integration, no builder and no typed
deserialisation. Where Rust's types can say more at no cost, they do: the C
enums are Rust enums, and an error carries the library's error kind.

---

## Table of Contents

1. [Requirements](#1-requirements)
2. [Installation](#2-installation)
3. [Building the C library](#3-building-the-c-library)
    - 3.1 [WebAssembly (`wasm32-wasip2`)](#31-webassembly-wasm32-wasip2)
4. [Quick-start](#4-quick-start)
    - 4.1 [High-level API (`parse` / `to_string`)](#41-high-level-api-parse--to_string)
    - 4.2 [SAX-style streaming reader](#42-sax-style-streaming-reader)
    - 4.3 [Collecting events](#43-collecting-events)
    - 4.4 [DOM (random-access) API](#44-dom-random-access-api)
    - 4.5 [Low-level writer](#45-low-level-writer)
    - 4.6 [Streaming / framing (`bovnar::stream`)](#46-streaming--framing-bovnarstream)
    - 4.7 [Files and I/O streams (`std::io::Read` / `Write`)](#47-files-and-io-streams-stdioread--write)
5. [Unit helpers](#5-unit-helpers)
    - 5.1 [The unit-profile notations (spec 1.2)](#51-the-unit-profile-notations-spec-12)
    - 5.2 [Extended unit functions](#52-extended-unit-functions)
    - 5.3 [`UnitFlags`](#53-unitflags)
    - 5.4 [`Prefix`](#54-prefix)
    - 5.5 [Inline unit suffix](#55-inline-unit-suffix)
    - 5.6 [`UnitPolicy` — validation and conversion without a callback](#56-unitpolicy--validation-and-conversion-without-a-callback)
    - 5.7 [Building a `Unit`](#57-building-a-unit)
6. [`Quantity`](#6-quantity)
    - 6.1 [Construction](#61-construction)
    - 6.2 [Fields and methods](#62-fields-and-methods)
    - 6.3 [Exact numbers (`Decimal`, `BigRational`, `BvnFloat`)](#63-exact-numbers-decimal-bigrational-bvnfloat)
    - 6.4 [`to_string()` integration](#64-to_string-integration)
7. [`write_array` helper](#7-write_array-helper)
8. [JSON conversion](#8-json-conversion)
9. [Currency helpers](#9-currency-helpers)
10. [`Reader` reference](#10-reader-reference)
    - 10.1 [Construction](#101-construction)
    - 10.2 [`read_mem`](#102-read_mem)
    - 10.3 [`ReadOptions`](#103-readoptions)
    - 10.4 [`EventData` and `EventPayload`](#104-eventdata-and-eventpayload)
    - 10.5 [`iter_mem`](#105-iter_mem)
    - 10.6 [Error-state accessors](#106-error-state-accessors)
11. [`Writer` reference](#11-writer-reference)
    - 11.1 [Construction](#111-construction)
    - 11.2 [Output retrieval](#112-output-retrieval)
    - 11.3 [Scalar write methods](#113-scalar-write-methods)
    - 11.4 [Extended integer writers](#114-extended-integer-writers)
    - 11.5 [Struct helpers](#115-struct-helpers)
    - 11.6 [Version directive](#116-version-directive)
    - 11.7 [Array helpers](#117-array-helpers)
    - 11.8 [Low-level `emit`](#118-low-level-emit)
12. [DOM API](#12-dom-api)
    - 12.1 [`DomDoc::parse`](#121-domdocparse)
    - 12.2 [`DomDoc`](#122-domdoc)
    - 12.3 [`DomNode`](#123-domnode)
    - 12.4 [`DomType`](#124-domtype)
13. [Error handling](#13-error-handling)
14. [Threads](#14-threads)
15. [Running the test suite](#15-running-the-test-suite)
16. [Crate layout](#16-crate-layout)
17. [Enums and `BaseUnit`](#17-enums-and-baseunit)
18. [Spec 1.1 additions](#18-spec-11-additions)
19. [License](#19-license)

- [See also](#see-also)

---

## 1. Requirements

| Requirement | Notes |
|---|---|
| Rust ≥ 1.88 | The crate's `rust-version`; CI builds and tests on it |
| A C compiler | `build.rs` compiles the C library through the `cc` crate |
| Python 3 on `PATH` (build time only) | The build runs the upstream table generator scripts, exactly as CMake does. Set the `PYTHON` environment variable to name a different interpreter. Nothing needs it at run time |
| `num-bigint`, `num-rational`, `num-integer`, `num-traits` | Pulled in by Cargo; they carry the exact numbers (arbitrary-width integers, rational numbers and the coefficient of a `Decimal`) |
| [wasi-sdk](https://github.com/WebAssembly/wasi-sdk) | **Only** for the `wasm32-wasip2` target — see [§3](#3-building-the-c-library) |

There are no optional features and no optional dependencies, and no integration
with third-party unit or array libraries; see [§16](#16-crate-layout).

---

## 2. Installation

The crate carries the version of the C library it wraps (**2.0.0**: `version` in
`Cargo.toml` is the `project(bovnar VERSION x.y.z)` of the top-level
`CMakeLists.txt`, and CI fails when the two differ). It is **not published to
crates.io** — `Cargo.toml` keeps `publish = false`, because `build.rs` compiles
sources outside `rust/` and runs the generators in the repository root, which a
packaged crate would not contain. Depend on it from a checkout of the
repository.

As a path dependency:

```toml
[dependencies]
bovnar = { path = "../bovnar/rust" }
```

or straight from Git, where Cargo finds the crate inside the repository:

```toml
[dependencies]
bovnar = { git = "https://github.com/sothis/bovnar" }
```

To work on the crate itself:

```bash
cd rust
cargo build
cargo test
```

---

## 3. Building the C library

The crate has no shared library to find: `build.rs` generates the unit tables
with the repository's table generators and compiles `../src` and `../include`
into the crate. A downstream crate therefore needs a C compiler and the
generators' interpreter ([§1](#1-requirements)) at build time and nothing at run
time.

`build.rs` re-runs when the C sources, the generators, the interpreter override
or `WASI_SDK_PATH` change.

### 3.1 WebAssembly (`wasm32-wasip2`)

A WebAssembly build needs a C compiler for WASI, [wasi-sdk](https://github.com/WebAssembly/wasi-sdk).
Point `WASI_SDK_PATH` at it and build for `wasm32-wasip2`:

```bash
rustup target add wasm32-wasip2
export WASI_SDK_PATH=/opt/wasi-sdk        # where wasi-sdk is unpacked
cargo build --release --target wasm32-wasip2
```

The same applies to a crate that depends on this one and is built for
`wasm32-wasip2`: set `WASI_SDK_PATH` for its build.

---

## 4. Quick-start

### 4.1 High-level API (`parse` / `to_string`)

The dynamic document type is `Value`. `parse` reads BVNR bytes into a `Value`
and `to_string` writes a `Value` out as BVNR bytes:

```rust
use bovnar::{Value, parse, to_string};

fn main() -> Result<(), bovnar::Error> {
    // Serialise a document to BVNR bytes
    let data = Value::Struct(vec![
        ("sensor_id".into(), Value::Int(42)),
        ("temperature".into(), Value::Float(-3.7)),
        ("label".into(), Value::String("outdoor".into())),
        ("active".into(), Value::Bool(true)),
        ("payload".into(), Value::Null),
    ]);
    let bvnr_bytes = to_string(&data, true)?;

    // Deserialise it back
    let recovered = parse(&bvnr_bytes)?;
    assert_eq!(recovered, data);
    let Value::Struct(fields) = recovered else { unreachable!() };
    assert_eq!(fields[0], ("sensor_id".to_string(), Value::Int(42)));
    Ok(())
}
```

`Value` is what `parse` returns and `to_string` takes:

| Variant | Notes |
|---|---|
| `Null` | |
| `Bool(bool)` | |
| `Int(i128)` | Any integer up to 128 bits, `uint64` and `sint64` included |
| `Float(f64)` | |
| `String(String)` | Symbols and references read back as strings too |
| `Bytes(Vec<u8>)` | An octet stream |
| `Array(Vec<Value>)` | A multi-row array is an array of row arrays |
| `Struct(Vec<(String, Value)>)` | Keys in document order; a struct is a `Vec` of pairs, not a map |
| `Quantity(Box<Quantity>)` | A typed scalar with its literal, type and unit |
| `Decimal(Decimal)` | Written as an exact `float_dec:64` literal |
| `Fraction(BigRational)` | Written as an exact `float_dec:64` literal when it terminates |

`parse_typed` is `parse` with typed values kept: they come back as
`Value::Quantity` instead of being decoded to native scalars. That preserves the
original text representation, exact type width, numeral base and physical unit
for lossless round trips:

```rust
use bovnar::{Value, parse_typed, to_string};

fn main() -> Result<(), bovnar::Error> {
    let bvnr = b".pressure = <float:32,Pa> 101325.0;";
    let doc = parse_typed(bvnr)?;
    let Value::Struct(fields) = &doc else { unreachable!() };
    let Value::Quantity(q) = &fields[0].1 else { unreachable!() };
    assert_eq!(q.raw.as_deref(), Some("101325.0"));
    assert_eq!(q.unit_str(), "Pa");

    // to_string() accepts Quantity values — annotation and raw text are re-emitted as-is
    let out = to_string(&doc, true)?;
    assert_eq!(parse_typed(&out)?, doc);
    Ok(())
}
```

`to_string(value, pretty)` takes a `Value::Struct` — any other top-level value is
an `Error::Argument`. Its write buffer starts at 4 MiB and doubles on overflow
up to 256 MiB; there is no `cap` argument. It returns `Vec<u8>` rather than
`String`, because an octet stream is written raw. `write_to(value, pretty, out)`
does the same into any `std::io::Write`, and `parse_reader(input)` /
`parse_typed_reader(input)` read from any `std::io::Read`
([§4.7](#47-files-and-io-streams-stdioread--write)).

> **`to_string()` writes what `parse()` reads — the streaming tier, not the
> document tier.** `parse()` checks the grammar and each value's own type, unit
> and range and stops there; `to_string()` is its inverse and stops in the same
> place, so `to_string(&parse(x)?)` works for every `x` that `parse()` accepts.
> Three shapes are therefore written without complaint and then **refused by the
> document tier** (`DomDoc::parse`, `bovnar validate`), where an array is
> homogeneous and rectangular:
>
> | Rust value | Written as | Document tier says |
> |---|---|---|
> | `Array(vec![Int(1), String("a".into())])` — an array mixing kinds | `[1, "a"]` | `array_element_type_mismatch` |
> | a ragged array of arrays — rows of 1 and 2 elements | `[[1], [2, 3]]` | `array_row_size_mismatch` |
> | structs that do not share their keys | `[{.a = <uint:64> 1;}, {.b = <uint:64> 2;}]` | `struct_shape_mismatch` |
>
> An array mixing `Int` and `Float`, or holding `Null` beside values, is fine at
> both tiers. If the output has to pass `bovnar validate`, run it through
> `DomDoc::parse` before shipping it.
>
> What `to_string()` does refuse, with `Error::Argument`, is what *neither* tier
> reads: an array whose elements carry different units, an integer wider than 64
> bits in an array that cannot take the width annotation it needs
> (`[Int(1 << 70), Float(1.5)]`), and a `Decimal` past the range of
> `float_dec:64`. A key that is not a string is not on the list: a struct key is
> a `String`, so the type system has already refused it.

### 4.2 SAX-style streaming reader

The callback receives exactly **one** argument, the `EventData`. There is no
userdata/context argument — capture external state in the closure instead. The
closure returns `true` to go on and `false` to stop the read (which then ends
in an `Error::Parse`).

```rust
use bovnar::{Event, EventData, ReadOptions, Reader};

fn main() -> Result<(), bovnar::Error> {
    let bvnr_bytes = b".a = 1; .b = \"two\";";
    let mut keys = Vec::new();
    let mut values = Vec::new();
    let mut on_event = |d: &EventData<'_>| {
        if d.event == Event::AssignmentStart {
            keys.push(d.text());
        } else if d.event == Event::Data {
            values.push(d.raw.to_vec());
        }
        true // returning false stops parsing (an Error::Parse)
    };

    let mut r = Reader::new()?;
    r.read_mem(bvnr_bytes, &ReadOptions::default(), Some(&mut on_event), None)?;
    assert_eq!(keys, ["a", "b"]);
    assert_eq!(values[0], b"1");
    Ok(())
}
```

`EventData` is the payload passed to a callback:

| Field / method | Type | Description |
|---|---|---|
| `d.event` | `Event` | The event code |
| `d.token_type` | `Option<TokenType>` | How the token was written: number, string, symbol, ... |
| `d.value_type` | `ValueTypeSpec` | The type annotation (family, width, base) |
| `d.value_unit` | `Unit` | The unit the value carries (dimensionless when it has none) |
| `d.raw` | `&[u8]` | The raw token bytes |
| `text()` | `String` | The token bytes as text, invalid UTF-8 replaced |
| `d.frac` | `&[u8]` | Verbatim ISO sub-second digits of a `datetime` literal (spec 1.1); empty when there are none |
| `d.unit_source` | `&[u8]` | The unit exactly as the document spelled it (library 2.0); empty when the value carries no unit |
| `d.converted` | `bool` | `true` when a `want_unit` conversion or a unit policy was applied |
| `d.converted_text` | `&[u8]` | The exact converted value as a positional string; empty if the conversion does not terminate in its base |
| `d.converted_base` | `u32` | The base `converted_text` is rendered in |
| `d.converted_unit` | `Unit` | The unit the converted value is in |
| `d.converted_rational()` | `Option<(String, String)>` | The exact conversion as `(numerator, denominator)` decimal strings — they may exceed any machine integer |
| `d.converted_in_base(base)` | `Result<Option<String>>` | The exact conversion re-rendered in `base` |

**The borrow ends with the callback.** `EventData<'a>` borrows the reader's
memory, so a field cannot be kept past the call; copy what you need (`text()`,
`d.raw.to_vec()`). The owned copy that can outlive its callback is
[`EventPayload`](#104-eventdata-and-eventpayload), which `iter_mem` collects for
you.

### 4.3 Collecting events

`Reader::iter_mem` returns the events as a `Vec` of owned `EventPayload` values
(the whole stream, read to the end):

```rust
use bovnar::Reader;

fn main() -> Result<(), bovnar::Error> {
    let bvnr_bytes = b".a = 1; .b = 2;";
    let mut r = Reader::new()?;
    for payload in r.iter_mem(bvnr_bytes, true, 0)? {
        println!("{:?} {}", payload.event, payload.text());
    }
    Ok(())
}
```

### 4.4 DOM (random-access) API

```rust
use bovnar::{DomDoc, DomType};

fn main() -> Result<(), bovnar::Error> {
    let bvnr_bytes = br#"
        .sensor_id = 42;
        .config = { .host = "example.com"; };
        .values = [1.5, 2.5, 3.5];
    "#;
    let doc = DomDoc::parse(bvnr_bytes)?;

    // Top-level key lookup
    let node = doc.get("sensor_id").expect("present");
    assert_eq!(node.as_i64(), Some(42));
    assert_eq!(node.value_type().width, 64);

    // Struct traversal — by member, or by dotted path
    let cfg = doc.get("config").expect("present");
    assert_eq!(cfg.get("host").and_then(|n| n.as_str()), Some("example.com"));
    assert_eq!(doc.lookup("config.host").and_then(|n| n.as_str()), Some("example.com"));

    // Array access
    let arr = doc.get("values").expect("present");
    assert_eq!(arr.dom_type(), Some(DomType::Array));
    let sum: f64 = (0..arr.len()).filter_map(|i| arr.at(i)?.as_float()).sum();
    assert_eq!(sum, 7.5);

    // Convert the entire document to a plain Value (drops type/unit info)
    let value = doc.to_value()?;
    assert!(matches!(value, bovnar::Value::Struct(_)));
    Ok(())
}
```

A `DomNode` is a `Copy` view that borrows its `DomDoc`, so the compiler — not a
convention — keeps the document alive for as long as a node is in use.

### 4.5 Low-level writer

```rust
use bovnar::{Base, Writer};

fn main() -> Result<(), bovnar::Error> {
    let mut w = Writer::to_mem(4096, true)?;
    w.write_float("velocity", 9.81, 64)?;
    w.write_uint("count", 1024, 32, Base::DECIMAL)?;
    w.finish()?;

    let output: &[u8] = w.get_output();
    assert_eq!(bovnar::parse(output)?, bovnar::parse(b".velocity = 9.81; .count = <uint:32> 1024;")?);
    Ok(())
}
```

The scalar writers take **no unit arguments**: write a value with
a unit through [`write_quantity`](#113-scalar-write-methods) or
[`write_array`](#7-write_array-helper), which take a `Unit`.

### 4.6 Streaming / framing (`bovnar::stream`)

Bindings for the streaming layer (see
[Streaming, Framing & Multiplexing](10_bovnar_streaming.md) for the full
treatment). The functions work on in-memory buffers:

```rust
use bovnar::{Value, parse, stream, to_string};

fn main() -> Result<(), bovnar::Error> {
    let one = Value::Struct(vec![("id".into(), Value::Int(1))]);
    let two = Value::Struct(vec![("id".into(), Value::Int(2))]);

    // Multi-document record framing
    let blob = stream::dump_documents(&[one.clone(), two.clone()], true)?;
    let docs = stream::load_documents(&blob, false, false, 0)?;
    assert_eq!(docs, [Some(one.clone()), Some(two)]);
    // With continue_past_failed = true a document that fails is None and the rest are read
    let docs = stream::load_documents(&blob, false, true, 0)?;
    assert_eq!(docs.len(), 2);

    // Octet multiplexing: (channel, payload) of any size, interleaved & reassembled
    let multiplexed = stream::mux_dump(&[(1, b"hello".as_slice()), (42, b"world".as_slice())], "mux")?;
    let messages = stream::mux_load(&multiplexed, 0, None)?;
    assert_eq!(messages, [(1, b"hello".to_vec()), (42, b"world".to_vec())]);

    // Document-in-document
    let outer = stream::embed_document(&to_string(&one, true)?, "payload", true)?;
    let Value::Struct(fields) = parse(&outer)? else { unreachable!() };
    let Value::Bytes(inner) = &fields[0].1 else { unreachable!() };
    assert_eq!(stream::parse_embedded(inner, false)?, one);
    Ok(())
}
```

All arguments are positional:

| Function | Arguments |
|---|---|
| `frame_documents(docs)` | already-serialised documents, `&[impl AsRef<[u8]>]` |
| `dump_documents(values, pretty)` | `&[Value]` |
| `load_documents(data, typed, continue_past_failed, max_document_size)` | `max_document_size` 0 is the default cap (`BVNR_DOC_DEFAULT_MAX`, 256 MiB) |
| `mux_dump(messages, key)` | `(u64, payload)` pairs; the conventional key is `"mux"` |
| `mux_load(data, max_message, key)` | `max_message` 0 is the default cap (`BVNR_MUX_DEFAULT_MAX`, 64 MiB); `key` `None` accepts any |
| `embed_document(inner, key, pretty)` | the conventional key is `"payload"` |
| `parse_embedded(inner, typed)` | |

`parse_embedded` returns an `Error::Parse` carrying the inner document's own
error code and, where the library reported one, its position. `mux_dump`
channels are `u64`, so the channel range is checked by the type.

Each function has a twin for `std::io::Read` or `std::io::Write`
([§4.7](#47-files-and-io-streams-stdioread--write)).

The `Reader` and `Writer` types of [§10](#10-reader-reference) and
[§11](#11-writer-reference) are the building blocks underneath: use them
directly when a document is produced or consumed event by event rather than as
one buffer.

### 4.7 Files and I/O streams (`std::io::Read` / `Write`)

Every entry point that reads or writes a whole document has a twin that takes
any `std::io::Read` or `std::io::Write`: a `File`, a pipe, a socket, a `Cursor`.
There is no separate path-based or file-descriptor form; open the `File` and
pass it in.

```rust
use std::io::Cursor;
use bovnar::{Base, DomDoc, ReadOptions, Reader, Writer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!("bovnar-doc-{}.bvnr", std::process::id()));

    // Write: the document reaches the file when finish() is called
    let mut w = Writer::to_writer(std::fs::File::create(&path)?, false)?;
    w.write_uint("port", 443, 16, Base::DECIMAL)?;
    w.finish()?;
    drop(w);

    // Random access over the file
    let dom = DomDoc::parse_reader(std::fs::File::open(&path)?)?;
    assert_eq!(dom.lookup("port").and_then(|n| n.as_u64()), Some(443));
    std::fs::remove_file(&path)?;

    // Streaming over any reader
    let mut keys = Vec::new();
    let mut on_event = |d: &bovnar::EventData<'_>| {
        if d.event == bovnar::Event::AssignmentStart {
            keys.push(String::from_utf8_lossy(d.raw).into_owned());
        }
        true
    };
    Reader::new()?.read_reader(Cursor::new(b".a = 1;"), &ReadOptions::default(), Some(&mut on_event), None)?;
    assert_eq!(keys, ["a"]);
    Ok(())
}
```

The plain-value and `stream` functions have twins too:

```rust
use std::io::Cursor;
use bovnar::{parse_reader, stream, write_to};

fn main() -> Result<(), bovnar::Error> {
    let value = parse_reader(Cursor::new(b".a = 1;"))?;

    // Serialise into any Write; here a Vec<u8>
    let mut out = Vec::new();
    write_to(&value, false, &mut out)?;
    assert_eq!(bovnar::parse(&out)?, value);

    // Framed documents round-trip through a Write and a Read
    let mut framed = Vec::new();
    stream::dump_documents_to(&[value.clone(), value.clone()], false, &mut framed)?;
    let docs = stream::load_documents_reader(Cursor::new(framed), false, false, 0)?;
    assert_eq!(docs, [Some(value.clone()), Some(value)]);
    Ok(())
}
```

| In-memory entry point | Reader or writer twin |
|---|---|
| `parse(data)` | `parse_reader(input)` |
| `parse_typed(data)` | `parse_typed_reader(input)` |
| `to_string(value, pretty)` | `write_to(value, pretty, out)` |
| `Reader::read_mem(data, …)` | `Reader::read_reader(input, &options, on_verified, on_unverified)` |
| `Reader::read_mem_want_unit(data, …)` | `Reader::read_reader_want_unit(input, &options, want_unit, on_verified)` |
| `DomDoc::parse(data)` | `DomDoc::parse_reader(input)` |
| `DomDoc::parse_with_policy(data, &policy)` | `DomDoc::parse_reader_with_policy(input, &policy)` |
| `Writer::to_mem(cap, pretty)` | `Writer::to_writer(out, pretty)` |
| `Writer::to_mem_with(cap, pretty, unit_flags, emit_version)` | `Writer::to_writer_with(out, cap, pretty, unit_flags, emit_version)` |
| `stream::frame_documents(docs)` | `stream::frame_documents_to(docs, out)` |
| `stream::dump_documents(values, pretty)` | `stream::dump_documents_to(values, pretty, out)` |
| `stream::load_documents(data, typed, continue_past_failed, max_document_size)` | `stream::load_documents_reader(input, typed, continue_past_failed, max_document_size)` |
| `stream::mux_dump(messages, key)` | `stream::mux_dump_to(messages, key, out)` |
| `stream::mux_load(data, max_message, key)` | `stream::mux_load_reader(input, max_message, key)` |
| `stream::embed_document(inner, key, pretty)` | `stream::embed_document_to(inner, key, pretty, out)` |
| `stream::parse_embedded(inner, typed)` | `stream::parse_embedded_reader(input, typed)` |

**They go through memory.** The C library takes no callback source or sink, so
nothing is streamed incrementally. A reader's input is read in full first —
to one byte past `ReadOptions::max_file_size` when that is set, so a document over
the cap fails with `file_too_long` at the same offset as `read_mem` reports, and
to at most `DOM_READ_MAX_BYTES` (256 MiB, the cap of `bvn_dom_parse_fd`) for the
DOM — and then parsed as `read_mem` parses it, so the events are the same. The
plain-value twins (`parse_reader`, `parse_typed_reader`) and the `stream` readers
read their input to its end with no cap; `load_documents_reader` and
`mux_load_reader` then apply `max_document_size` and `max_message` in the parse,
as their in-memory twins do. The `stream` writers and `write_to` serialise first,
so nothing reaches `out` when serialising fails; then they write once and flush. A writer puts its
document together in a memory sink of `DEFAULT_MEM_CAP` (4 MiB) bytes, or the
`cap` of `to_writer_with`, and **`finish()` writes it to `out` once and flushes
it**: nothing reaches `out` before then. Writing past the cap fails with
`sink_buffer_exhausted`, as with `Writer::to_mem`. `out` must be
`Send + 'static`; the writer stays `Send` ([§14](#14-threads)).

A failing `input` or `out` is an `Error::Io` ([§13](#13-error-handling)) with the
code the C library gives an unreadable fd source or unwritable fd sink
(`reading_from_source_fd`, `writing_to_sink`); no event is delivered for a reader
that failed, because nothing is parsed before the input has been read. No file
descriptor is involved, so these work the same on Windows.

---

## 5. Unit helpers

The unit functions live in the `bovnar::units` module; `Unit` itself is at the
crate root.

```rust
use bovnar::Unit;
use bovnar::units::{unit_factor, unit_prefix_factor, unit_to_str};

fn main() -> Result<(), bovnar::Error> {
    // Parse a unit string into a Unit
    let u = Unit::parse("k~g·m/s²")?;

    // Convert a Unit back to its canonical string
    assert_eq!(unit_to_str(&u)?, "k~g·m/s²");
    assert_eq!(u.to_string(), "k~g·m/s²"); // Display is the canonical spelling too

    // Scalar SI/IEC PREFIX factor of a unit — the prefix only, never the
    // base unit's own factor.
    assert_eq!(unit_prefix_factor(&Unit::parse("M~Hz")?), 1_000_000.0);
    assert_eq!(unit_prefix_factor(&Unit::parse("in")?), 1.0); // NOT 0.0254 — the inch has no prefix
    assert_eq!(unit_prefix_factor(&Unit::parse("h")?), 1.0);  // NOT 3600.0

    // unit_factor() takes the unit as text and is the same prefix factor
    assert_eq!(unit_factor("M~Hz")?, 1_000_000.0);
    assert_eq!(unit_factor("in")?, 1.0);
    assert_eq!(unit_factor("h")?, 1.0);

    // The FULL factor to coherent SI is Unit::factor
    assert_eq!(Unit::parse("in")?.factor()?, 0.0254);
    assert_eq!(Unit::parse("h")?.factor()?, 3600.0);
    Ok(())
}
```

**`unit_factor` is the prefix scale, not the full factor.** It is
`unit_prefix_factor` on a unit given as text, and reports only what the prefixes
contribute: `1000000.0` for `M~Hz`, `1.0` for `in` and `h`. The full factor to
coherent SI is `Unit::factor` on the parsed unit (or `unit_to_si_factor`); it
fails for an affine unit (`°C`) or one with no SI factor.

`Unit` is the one unit type; every function that takes a unit takes a `&Unit`.

### 5.1 The unit-profile notations (spec 1.2)

**These notations are spec 1.2**: in a document they need a `#!bovnar 1.2`
directive, while the API entry points below have no document and take them
unconditionally. `Unit::parse` takes a profile notation as readily as the
native one, and returns the same `Unit` either way — so everything else in this
chapter works on the result unchanged. **Seven** namespaces are accepted —
`ucum:`, `unece:`, `qudt:`, `qudt-qk:`, `udunits:`, `om:` and `cf:` — but each
is a *compile-time* switch of the C library, so which of them a given build
carries is a question to ask rather than assume; see `available_profiles()`
below.

```rust
use bovnar::{ErrorCode, Unit};
use bovnar::units::{
    available_profiles, units_compatible, unit_error_code, unit_is_profile_only, unit_to_profile,
    unit_to_str, unit_to_ucum,
};

fn main() -> Result<(), bovnar::Error> {
    let vu = Unit::parse("ucum:mm[Hg]")?;
    assert_eq!(unit_to_str(&vu)?, "mmHg");               // the native canonical form
    assert_eq!(unit_to_ucum(&vu)?, "mm[Hg]");            // back to a UCUM code
    assert!(units_compatible(&vu, &Unit::parse("k~Pa")?));

    // Which profiles THIS build carries. Unit::parse fails with the same
    // Error::Argument for an absent namespace as for a bad unit;
    // unit_error_code() is what tells the two apart (49 against 32).
    assert_eq!(available_profiles(), ["ucum", "unece", "qudt", "qudt-qk", "udunits", "om", "cf"]);

    // The same unit in other vocabularies — all one Unit
    let kg = Unit::parse("k~g")?;
    assert_eq!(unit_to_profile("ucum", &kg)?, "kg");
    assert_eq!(unit_to_profile("unece", &kg)?, "KGM");
    assert_eq!(unit_to_profile("qudt", &kg)?, "KiloGM");
    assert_eq!(unit_to_profile("udunits", &kg)?, "kg");
    assert_eq!(kg.to_profile("unece")?, "KGM");           // the method form

    let iu = Unit::parse("ucum:[IU]/mL")?;
    assert!(unit_is_profile_only(&iu));                   // no native spelling exists
    assert_eq!(unit_to_str(&iu)?, "ucum:[IU].mL-1");

    let packaging = Unit::parse("unece:XBX")?;
    assert!(packaging.is_profile_only());              // a countable package
    assert_eq!(unit_to_str(&packaging)?, "unece:XBX");    // in its OWN namespace

    assert_eq!(unit_error_code("ucum:B[SPL]"), ErrorCode::UnitProfileUnsupported as i32); // 50
    assert_eq!(unit_error_code("udunits:days since 1970-01-01"), 50); // valid UDUNITS, not carryable
    assert_eq!(unit_error_code("nosuch:m"), ErrorCode::UnitProfileUnknown as i32);        // 49
    assert_eq!(unit_error_code("m/s"), 0);                // it parses
    Ok(())
}
```

`unit_to_profile` and `unit_to_ucum` fail rather than inventing a code, in three
situations. A native unit outside that vocabulary's transliteration table has no
form in it — the Old German units, the water-hardness degrees, the turbidity
kinds and every currency. An opaque unit belonging to a *different* profile has
none either: `[IU]` is UCUM's, so `unit_to_profile("unece", …)` refuses it. And a
**flat** vocabulary (`unece`, `qudt`, `qudt-qk`, `om`) can spell only a single
component — no product, quotient or exponent, though a prefixed one is fine
(`k~g` is `unece:KGM`) — because a flat code is one whole token with no operator
to build it out of — so `k~m/h` has no UNECE form even though `unece:KMH` parses
to exactly it.

In a **document** the notation additionally requires a `#!bovnar 1.2`
directive; without it `parse` fails with `unit_illegal`. `Unit::parse` has no
document and therefore no declared version, so it accepts the notation
unconditionally.

See [Unit Profiles](11_bovnar_unit_profiles.md) for the transliteration tables
and the codes that have no representation.

### 5.2 Extended unit functions

The following functions operate on `Unit` values and live in `bovnar::units`.

```rust
use bovnar::{BaseUnit, Exponent, IECPrefix, Prefix, Unit};
use bovnar::units::{
    convert_value, exponent_to_int, int_to_exponent, prefix_unit_valid, unit_convert_factor,
    unit_dimension_vector, unit_prefix_exponent, unit_prefix_factor, unit_reduce,
    unit_to_si_factor, unit_to_str_ex, unit_valid, units_compatible, UnitFlags,
};

fn main() -> Result<(), bovnar::Error> {
    let km = Unit::parse("k~m")?;
    let gib = Unit::parse("Gi~B")?;

    // Validate a Unit
    assert!(unit_valid(&km));

    // Prefix scale factor (SI or IEC) for a Unit
    assert_eq!(unit_prefix_factor(&km), 1000.0);
    assert_eq!(unit_prefix_factor(&gib), 2f64.powi(30));

    // Prefix exponent (base-10 for SI, base-2 for IEC)
    assert_eq!(unit_prefix_exponent(&km), 3);
    assert_eq!(unit_prefix_exponent(&gib), 30);

    // Validate a prefix for a base unit (IEC prefixes are only valid on bit/byte)
    let p = Prefix::Iec(IECPrefix::Gibi);
    assert!(prefix_unit_valid(p, BaseUnit::Byte));
    assert!(!prefix_unit_valid(p, BaseUnit::Meter));

    // Full SI conversion including affine terms (e.g. Celsius → Kelvin)
    let c = unit_to_si_factor(&Unit::parse("°C")?)?;
    assert!(c.is_affine);
    assert_eq!(c.affine_offset, 273.15);

    // Check convertibility: same SI dimension vector AND same quantity kinds
    assert!(units_compatible(&km, &Unit::parse("m")?));
    assert!(!units_compatible(&Unit::parse("b")?, &Unit::parse("B")?)); // the dimensions agree, the kinds do not

    // Conversion factor between two compatible units
    let conv = unit_convert_factor(&km, &Unit::parse("m")?)?;
    assert_eq!((conv.factor, conv.requires_affine), (1000.0, false));

    // 7-element SI dimension exponent vector [m, kg, s, A, K, mol, cd]
    assert_eq!(unit_dimension_vector(&Unit::parse("m/s")?)?, [1, 0, -1, 0, 0, 0, 0]);

    // Fold every prefix out and combine repeated bases. NOT the named-SI collapse:
    // the reduced unit of k~g·m/s² is "m·g/s²", and it is
    // unit_to_str_ex(.., UnitFlags::REDUCE) that gives "N". See the warning in §5.3
    // before applying r.scale to a value you are also going to serialise.
    let r = unit_reduce(&Unit::parse("k~g·m/s²")?)?;
    assert_eq!(r.unit.to_string(), "m·g/s²");
    assert_eq!(r.scale, 1000.0);

    // Convert a scalar value between units (handles affine conversions)
    let kelvin = convert_value(25.0, &Unit::parse("°C")?, &Unit::parse("K")?)?;
    assert!((kelvin - 298.15).abs() < 1e-9);

    // Serialise with formatting options (see UnitFlags below)
    let force = Unit::parse("k~g·m/s²")?;
    assert_eq!(unit_to_str_ex(&Unit::parse("m/s²")?, UnitFlags::ASCII_EXP)?, "m/s^2"); // ^N instead of superscripts
    assert_eq!(unit_to_str_ex(&force, UnitFlags::REDUCE)?, "N");                      // reduce first — but see the warning below
    assert_eq!(unit_to_str_ex(&force, UnitFlags::REDUCE | UnitFlags::ASCII_EXP)?, "N");

    // Exponent ↔ integer conversions
    assert_eq!(exponent_to_int(Exponent::NEG_SQUARE), -2);
    assert_eq!(int_to_exponent(-2), Exponent::NEG_SQUARE);
    Ok(())
}
```

`SI_DIM_NAMES` is the array `["m", "kg", "s", "A", "K", "mol", "cd"]` — the index
positions used by `unit_dimension_vector`. Both `SIConversion` (`factor`,
`is_affine`, `affine_offset`) and `UnitConversion` (`factor`,
`requires_affine`) are small `Copy` structs, and `ReducedUnit` is
`{ unit, scale }`.

### 5.3 `UnitFlags`

```rust
use bovnar::units::UnitFlags;

fn main() {
    assert_eq!(UnitFlags::NONE, 0);           // no special formatting
    assert_eq!(UnitFlags::REDUCE, 1);         // reduce to a canonical named SI unit before serialising
    assert_eq!(UnitFlags::ASCII_EXP, 2);      // use ^N exponent notation instead of Unicode superscripts
    assert_eq!(UnitFlags::NUM_CANONICAL, 4);  // writer only: a float REDUCE rescaled keeps float shape
}
```

`UnitFlags` is a namespace of `u32` constants, not an `IntFlag` type: OR them
together and pass the `u32`. The same flags configure a **writer**:
`Writer::to_mem_with(cap, pretty, UnitFlags::REDUCE, false)` rewrites each unit
as it is written *and* rescales the value with it — `2.5 k~g` goes out as
`<float:64,g> 2500`. `NUM_CANONICAL` is meaningful only there: it appends `.0` to
a rescaled float that came out as an integer string, so the canonical document
mode lands on one spelling.

> **`UnitFlags::REDUCE` returns the reduced unit, not a rescaled value.**
> Reduction folds every prefix out, so `unit_to_str_ex(&k~g, UnitFlags::REDUCE)`
> is `"g"` — a string denoting a quantity 1000× smaller than the unit you
> passed. Moving the value with it is your job. (The writer does it for you;
> nothing else does.)
>
> **Do not use `unit_reduce(&u)?.scale` for that.** It is the scale to the *fully
> reduced* unit, and this function does not always emit the fully reduced unit:
> where the reduction lands on a named SI unit the formatter re-attaches the
> prefix. `k~N` comes back `"k~N"` with nothing to rescale while `unit_reduce`
> still reports `1000.0`, and `k~g` comes back `"g"` where the `1000.0` must be
> applied. The two are indistinguishable from outside — both a lone unit
> carrying a kilo prefix — so take the scale from the unit that is actually
> emitted:
>
> ```rust
> use bovnar::Unit;
> use bovnar::units::{convert_value, unit_to_str_ex, UnitFlags};
>
> fn main() -> Result<(), bovnar::Error> {
>     let vu = Unit::parse("k~g")?;
>     let mut value = 2.5;
>     let emitted = Unit::parse(&unit_to_str_ex(&vu, UnitFlags::REDUCE)?)?;
>     value *= convert_value(1.0, &vu, &emitted)?; // 1.0 for k~N, 1000.0 for k~g
>     assert_eq!((emitted.to_string().as_str(), value), ("g", 2500.0));
>     Ok(())
> }
> ```
>
> The collapse never substitutes one named unit for another, so a `Sv` stays a
> `Sv` rather than becoming a `Gy`. A lone base at an exponent other than 1 does
> still collapse (`s⁻¹` → `"Hz"`, `m~s⁻¹` → `"k~Hz"`), and a reduction whose
> summed exponent leaves the `±100` range (`EXPONENT_MIN` / `EXPONENT_MAX`)
> fails with `Error::Argument` instead of returning a unit that lost a
> dimension — `m⁵⁰·m⁵⁰` reduces to `m¹⁰⁰`, `m⁵⁰·m⁵¹` fails.

### 5.4 `Prefix`

`Prefix` is one value for what the C struct `value_unit_prefix_t` splits into a
system and an id: an enum with an SI and an IEC variant. It is what `UnitComponent::prefix` holds:

```rust
use bovnar::{IECPrefix, Prefix, SIPrefix, Unit};

fn main() -> Result<(), bovnar::Error> {
    let p_si = Prefix::Si(SIPrefix::Kilo);
    let p_iec = Prefix::Iec(IECPrefix::Gibi);
    assert!(!p_si.is_none() && !p_iec.is_none());
    assert_eq!(Prefix::default(), Prefix::Si(SIPrefix::None));

    let vu = Unit::parse("Gi~B")?;
    let comp = vu.components()[0];
    assert_eq!(comp.prefix, p_iec);             // the Prefix of a component
    assert_eq!(comp.prefix, IECPrefix::Gibi);   // compares equal to the bare IEC prefix as well
    Ok(())
}
```

`Prefix::default()` is no prefix (`SIPrefix::None`), and an unprefixed component
equals whatever its prefix system.

### 5.5 Inline unit suffix

In addition to the unit embedded in a type annotation (`<float:64,m/s>`), the
Bovnar format supports an **inline unit suffix** placed directly after a scalar
value, before the terminating `;`:

```bovnar
.speed = 9.81 m/s;           # inline suffix, no annotation
.mass  = <float:64> 70.5 k~g;# annotation without unit, inline adopted
.dist  = <float:64,m> 1.5 m; # annotation and inline both say 'm' — valid
```

From the Rust layer, inline and annotation units are **identical**: both reach
the application as `d.value_unit` in the `Event::Data` payload. No extra code is
required to consume inline units.

The validator fails with `ErrorCode::UnitMismatch` (38, an `Error::Parse`) when
an inline suffix is present and a type-annotation unit is also present but the
two do not resolve to the same unit. Inline unit suffixes inside array elements
always fail with `ErrorCode::UnexpectedInputByte`.

### 5.6 `UnitPolicy` — validation and conversion without a callback

`Reader::set_unit_policy` states what the document must contain and what unit
values should arrive in. Everything is unit **text**, so unlike `want_unit` it
costs no per-value trampoline across the FFI boundary — the C side parses the
strings once, when the policy is set.

```rust
use bovnar::{EventData, ReadOptions, Reader, UnitPolicy};

fn main() -> Result<(), bovnar::Error> {
    let mut r = Reader::new()?;
    r.set_unit_policy(Some(&UnitPolicy {
        targets: vec![("m/s".into(), 0), ("°C".into(), 0)], // convert to the first one that fits
        normalise_si: true,    // ...and everything else to SI base units
        leave_inexact: true,   // step over what cannot be exact
        require_unit: true,    // reject any bare numeric value
        ..UnitPolicy::default()
    }))?;

    let mut delivered = Vec::new();
    let mut handler = |d: &EventData<'_>| {
        if d.converted {
            delivered.push(String::from_utf8_lossy(d.converted_text).into_owned());
        }
        true
    };
    r.read_mem(b".v = <float:64,k~m/h> 36.0;", &ReadOptions::default(), Some(&mut handler), None)?;
    assert_eq!(delivered, ["10"]);
    Ok(())
}
```

A policy survives re-reading the same reader on another document — it describes
the consumer, not the document — and `set_unit_policy(None)` clears it. Every
unit string is parsed by the call, so a typo fails with `Error::Argument` before
a byte is read, and a rejected policy leaves the previous one in force. The
struct is plain data: `UnitPolicy { rules, targets, normalise_si, base,
leave_inexact, require_unit, require_dimension_of }`, so
`..UnitPolicy::default()` fills what a policy leaves out. At most
`MAX_UNIT_TARGETS` (8) targets, `MAX_UNIT_RULES` (8) rules, and
`MAX_UNIT_PATH` (96) bytes of path are accepted.

**Conversion.** Each numeric value takes the first target it can validly convert
to, so order is significant: `["m", "k~m"]` never selects `k~m`. A value that
matches nothing, or that is already in the unit a target names, arrives
untouched — `d.converted` is what tells the two apart. Each target carries its
own output base as the second tuple element: `("m".into(), 16)`; `0` is decimal.

`normalise_si` catches whatever the targets did not, delivering it in coherent
SI base units with prefixes folded out. Currencies and every **dimensionless**
unit (`%`, `ppm`, `dB`, `pH`, `rad`, `°`) are left as written rather than
reinterpreted.

**A value with no unit only ever matches a target that is itself `no_unit`.** A
bare number is dimensionally compatible with `%` and `ppm`, so without that
fence a policy naming `"%"` would deliver `0.25` as `25`.

**Exactness.** Nothing approximate is ever delivered. `leave_inexact: true`
hands over a value the conversion cannot deliver exactly — an irrational
factor, a non-terminating expansion (`42 km/h` is `35/3 m/s`), an exponent past
the work limit — in its native unit instead of failing. It applies only to
policy-chosen targets; a `want_unit` hook keeps its strict all-or-nothing
contract.

**Validation.** `require_unit` rejects a document containing any bare numeric
value; `require_dimension_of: vec!["m".into()]` requires every numeric value to
be validly convertible to at least one listed unit. Both are evaluated on the
unit the **document** wrote, before any conversion, and both fail with
`ErrorCode::UnitMismatch`.

**Per-field rules** name one field by its key path, and are consulted before
everything else:

```rust
use bovnar::{UnitPolicy, UnitRule};

fn main() {
    let policy = UnitPolicy {
        rules: vec![
            UnitRule::new(".inlet.temperature", "°C"), // convert this field
            UnitRule { convert: false, ..UnitRule::new(".inlet.*", "m") }, // assert the subtree
        ],
        ..UnitPolicy::default()
    };
    assert_eq!(policy.rules.len(), 2);
}
```

A path ending in `".*"` names a subtree at any depth, and a prefix only
matches at a component boundary (`".in.*"` does not claim `".inlet.a"`). Unlike
a whole-document target, a rule is an **assertion**: a value it cannot be
applied to fails with `UnitMismatch`, because silence would defeat the point of
naming the field. `convert: false` asserts without converting.

**The DOM takes the same policy**, so a random-access consumer gets both halves:

```rust
use bovnar::{DomDoc, UnitPolicy, UnitRule};

fn main() -> Result<(), bovnar::Error> {
    let text = ".inlet = { .temperature = <float:64,°F> 212.0; };".as_bytes();
    let policy = UnitPolicy {
        rules: vec![UnitRule::new(".inlet.temperature", "°C")],
        ..UnitPolicy::default()
    };
    let doc = DomDoc::parse_with_policy(text, &policy)?;
    let t = doc.lookup("inlet.temperature").expect("present");
    assert_eq!(t.as_float(), Some(100.0)); // from a document in °F
    Ok(())
}
```

A value the policy converted is STORED converted — digits, unit and base — and
an integer that converts to a fraction (5 g in kilograms is 0.005) is stored as
a float. A policy the library refuses is an `Error::Argument` — the same error,
naming the same entry, as `Reader::set_unit_policy` — before anything is
parsed, rather than a parse error, so a mistake in the policy is never mistaken
for a fault in the document.

**The writer takes the same policy, validation half only.** A reader can only
reject a document somebody already wrote; `Writer::set_unit_policy` is what
stops a bare number reaching the file:

```rust
use bovnar::{BaseUnit, Exponent, Quantity, SIPrefix, Unit, UnitPolicy, ValueTypeFamily, ValueTypeSpec, Writer};

fn main() -> Result<(), bovnar::Error> {
    let mut w = Writer::to_mem(4096, true)?;
    w.set_unit_policy(Some(&UnitPolicy {
        require_unit: true,
        require_dimension_of: vec!["m".into()],
        ..UnitPolicy::default()
    }))?;

    let float64 = ValueTypeSpec { family: ValueTypeFamily::Float as i32, width: 64, base: 0 };
    let inches = Unit::parse("in")?;
    w.write_quantity("tank_level", &Quantity::new("12.0", float64, inches))?; // a length — written
    let err = w.write_quantity("spare", &Quantity::new("0.25", float64, Unit::default())).unwrap_err();
    assert!(matches!(err, bovnar::Error::Write { .. })); // code UnitMismatch
    let _ = (BaseUnit::Meter, Exponent::LINEAR, SIPrefix::Kilo); // see §5.7 for building units
    Ok(())
}
```

A policy carrying `targets`, `normalise_si`, `base` or `leave_inexact` fails with
`Error::Argument` there rather than being half-honoured: write-side unit
rewriting is `UnitFlags::REDUCE`'s job — open the writer with
`Writer::to_mem_with(cap, pretty, UnitFlags::REDUCE, false)` — and two rewriting
modes with different rules about exactness is how a document ends up meaning
two things.

Module-level helpers back the same decisions outside the reader:

```rust
use bovnar::Unit;
use bovnar::units::{unit_si_normal_form, units_compatible, units_convertible};

fn main() -> Result<(), bovnar::Error> {
    let (kusd, usd) = (Unit::parse("k~$USD")?, Unit::parse("$USD")?);
    // units_compatible is the wrong screen for a conversion target: a currency
    // carries no dimension, so it is reported incompatible with itself.
    assert!(!units_compatible(&kusd, &usd));
    assert!(units_convertible(&kusd, &usd));

    assert_eq!(unit_si_normal_form(&Unit::parse("in")?), Some(Unit::parse("m")?));
    assert_eq!(unit_si_normal_form(&Unit::parse("g")?), Some(Unit::parse("k~g")?)); // the SI base unit for mass
    assert_eq!(unit_si_normal_form(&Unit::parse("%")?), None);                      // dimensionless
    assert_eq!(unit_si_normal_form(&Unit::parse("lm")?), None);                     // carries the steradian's kind
    Ok(())
}
```

**Three questions, in decreasing strictness.** `units_equal` asks whether the
two are the *same* unit, `units_compatible` whether their dimensions and
quantity kinds line up, `units_convertible` whether a conversion can actually be
performed. Only the last is the right screen for a conversion target.

```rust
use bovnar::Unit;
use bovnar::units::units_equal;

fn main() -> Result<(), bovnar::Error> {
    let p = Unit::parse;
    assert!(units_equal(&p("m")?, &p("m")?));
    assert!(!units_equal(&p("m")?, &p("k~m")?));             // convertible, not equal
    assert!(!units_equal(&p("N")?, &p("k~g·m/s²")?));        // same SI form
    assert!(units_equal(&p("mmHg")?, &p("ucum:mm[Hg]")?));
    Ok(())
}
```

`units_equal` is also what `==` means between two `Unit`s, and `Unit` is `Eq`
and `Hash` with the two agreeing — equal units are one `HashMap` key regardless
of the order their components were written in. The hash follows the equality in
the one place the two could part: a component with **no prefix** is the same
component whether its prefix system says SI or IEC, so `Unit::parse("B")` and
`Unit::iec(BaseUnit::Byte, IECPrefix::None, Exponent::LINEAR)` are equal *and*
one key.

A `Unit` also compares equal to a **string** that parses to it
(`unit == "k~m"`, via `PartialEq<str>` and `PartialEq<&str>`); a string that does
not parse is simply unequal. That is a convenience for comparison only and is
**not hash-compatible** — `"km"`, `"k~m"` and `"ucum:km"` are three strings with
three hashes and one unit. Key a map or set on `Unit` values (or on
`unit.to_string()`), never on a mix of the two.

`Unit::convert_factor(&other)` and `Unit::factor()` return a bare number, so
they **fail** with `Error::Argument` where there is no such number: an affine
pair (`°C` → `K`) has no multiplicative factor and an affine unit no SI factor.
Use `convert_value()` there, or `unit_to_si_factor()` for the scale and the
offset.

### 5.7 Building a `Unit`

`Unit::parse` builds a unit from text. To build one programmatically — without
going through the notation — use the constructors on `Unit`:

```rust
use bovnar::{BaseUnit, Exponent, IECPrefix, SIPrefix, Unit, UnitComponent};

fn main() -> Result<(), bovnar::Error> {
    let kg = Unit::si(BaseUnit::Gram, SIPrefix::Kilo, Exponent::LINEAR);
    assert_eq!(kg.to_string(), "k~g");
    let m2 = Unit::si(BaseUnit::Meter, SIPrefix::None, Exponent::SQUARE);
    assert_eq!(m2.to_string(), "m²");
    let gib = Unit::iec(BaseUnit::Byte, IECPrefix::Gibi, Exponent::LINEAR);
    assert_eq!(gib.to_string(), "Gi~B");

    let newton = Unit::compound(&[
        UnitComponent::si(BaseUnit::Gram, SIPrefix::Kilo, Exponent::LINEAR),
        UnitComponent::si(BaseUnit::Meter, SIPrefix::None, Exponent::LINEAR),
        UnitComponent::si(BaseUnit::Second, SIPrefix::None, Exponent::NEG_SQUARE),
    ])?;
    assert_eq!(newton.to_string(), "k~g·m/s²");
    Ok(())
}
```

The C constructors `make_unit_si`, `make_unit_iec` and `make_unit_compound` are
`Unit::si`, `Unit::iec` and `Unit::compound`; the prefix and exponent are always
passed (`SIPrefix::None`, `Exponent::LINEAR` for a plain base unit), and
`Unit::compound` fails with `Error::Argument` beyond `MAX_UNIT_COMPONENTS` (32)
components.

**`Unit::default()` and `Unit::dimensionless()` are not the same thing**, even
though both render as `no_unit`. The first (the C `make_unit_none()`) is a
unit with no components at all — a value carrying no unit. The second (the C
`make_unit_dimensionless()`) is one component whose base is `BaseUnit::None` — a
value carrying `no_unit` explicitly. That is exactly the pair
`UnitPolicy { require_unit: true, .. }` refuses together (§5.6):

```rust
use bovnar::Unit;
use bovnar::units::unit_to_str;

fn main() -> Result<(), bovnar::Error> {
    let (none, dimensionless) = (Unit::default(), Unit::dimensionless());
    assert_eq!(unit_to_str(&none)?, "no_unit");
    assert_eq!(unit_to_str(&dimensionless)?, "no_unit");
    assert_eq!((none.num_components(), dimensionless.num_components()), (0, 1));
    assert!(Unit::is_dimensionless(&none) && Unit::is_dimensionless(&dimensionless));
    // Display, unlike unit_to_str, spells a unit-less value as nothing at all
    assert_eq!(none.to_string(), "");
    Ok(())
}
```

`Unit::is_dimensionless()` is true for both of them and for any unit made only of
`none` components; `%` and `ppm` are real units and answer false.

---

## 6. `Quantity`

`Quantity` is a typed, unit-annotated scalar value that preserves the original
text representation, type width, numeral base, and physical unit across a
`parse_typed` / `to_string` round trip.

### 6.1 Construction

`Quantity` is normally produced by `parse_typed` rather than constructed by
hand, but direct construction is supported. Its fields are public. The type is a
`ValueTypeSpec` — a plain struct of the family (a `ValueTypeFamily` as its C
integer), the bit width and the numeral base:

```rust
use bovnar::{Quantity, Unit, ValueTypeFamily, ValueTypeSpec};

fn main() -> Result<(), bovnar::Error> {
    let vt = ValueTypeSpec { family: ValueTypeFamily::Float as i32, width: 32, base: 0 };
    let q = Quantity::new("101325.0", vt, Unit::default());       // dimensionless
    let q2 = Quantity::new("9.81", vt, Unit::parse("m/s²")?);     // with a unit
    assert_eq!(q.unit_str(), "");
    assert_eq!(q2.unit_str(), "m/s²");
    Ok(())
}
```

For exact numeric input there is also `Quantity::from_number`, which stores the
value as an exact decimal literal (so writing it with `to_string` is lossless to
the format's precision):

```rust
use bovnar::{Decimal, Quantity, Unit, ValueTypeFamily};
use num_rational::BigRational;

fn main() -> Result<(), bovnar::Error> {
    let price: Decimal = "19.99".parse()?;
    Quantity::from_number(price, ValueTypeFamily::FloatDec, 64, None, Unit::default())?;

    let one_point_one: Decimal = "1.1".parse()?;
    Quantity::from_number(one_point_one, ValueTypeFamily::Float, 128, None, Unit::default())?;

    // A float_fix takes its fractional bits: 837/256 is 3.26953125
    let ratio = BigRational::new(837.into(), 256.into());
    Quantity::from_number(ratio, ValueTypeFamily::FloatFix, 32, Some(8), Unit::default())?;
    Ok(())
}
```

It takes an `impl Into<ExactNumber>`: a literal (`&str` or `String`, used
verbatim), a `Decimal`, a terminating `BigRational`, a `BigInt`, or an `f64`
(only as precise as the double). It validates the width for the chosen family.
For `float_dec` it also refuses a finite value past the width's range — `1e400`
does not fit `float_dec:64` and would materialise to infinity — and a
non-finite `f64` is stored under the format's own spelling (`inf`, `ninf`,
`nan`). A `float_fix` needs its `frac` (`Some(q)`).

### 6.2 Fields and methods

`Quantity` is a plain struct: the properties that merely hold a value are public
fields and the others are methods:

| Name | Type | Description |
|---|---|---|
| `q.raw` | `Option<String>` | Original text token as it appeared in the BVNR stream; `None` for a typed null |
| `q.vtype` | `ValueTypeSpec` | Type family, bit width, and numeral base |
| `q.unit` | `Unit` | Physical unit; `q.unit.to_string()` is its canonical text |
| `q.token_type` | `TokenType` | How the literal was written: number, quoted string, symbol, ... |
| `q.frac` | `Option<String>` | For a `datetime` written as a literal with a fractional second, the verbatim sub-second digits (spec 1.1) |
| `q.unit_source` | `Option<String>` | The unit exactly as the document spelled it (library 2.0) — `ucum:mL{total}` where `q.unit` is `m~L`. Set by `parse_typed`; `to_string()` writes it back, and refuses one that does not parse to `q.unit` — including a spelling on a `Quantity` that has no unit at all. `None` writes the canonical form |
| `value()` | `Value` | Decode `raw` to the closest plain value (`Int`, `Float`, `String`, `Bool`) — **lossy** for `float_dec` / `float_fix` / `float:128`+ (goes through a C `double`) |
| `q.family()` | `Option<ValueTypeFamily>` | The type family, when it is one this binding knows |
| `unit_str()` | `String` | Canonical unit string (e.g. `"m/s²"`), or `""` when dimensionless |
| `decimal()` | `Result<Option<Decimal>>` | **Exact** value as a `Decimal` from the verbatim literal — lossless at any width; an error for non-numeric families, `None` for a typed null |
| `fraction()` | `Result<Option<BigRational>>` | Exact value as a rational (for `float_fix`, the exact `mantissa / 2**frac`) |
| `fixed_point()` | `Result<(BigInt, u32)>` | `(mantissa, frac_bits)` of a `float_fix` value; the value is `mantissa / 2**frac_bits` |
| `stored_value()` | `Result<Option<Decimal>>` | The value materialised into the declared IEEE/fixed format (round-to-nearest-even) — differs from `decimal` only when the literal carries more precision than the format holds. `None` for a typed null, as `decimal` is |
| `ieee_bits()` | `Result<Vec<u8>>` | IEEE-754 interchange bytes (binary16…256 for `float`, decimal16…256 for `float_dec`), little-endian word order |
| `epoch_name()` | `Option<String>` | For a `datetime`, the epoch name (`"unix"`, `"tai"`, …); `None` otherwise |
| `epoch_mjd()` | `Option<i32>` | For a `datetime`, the epoch's Modified Julian Day; `None` otherwise |

### 6.3 Exact numbers (`Decimal`, `BigRational`, `BvnFloat`)

`value()` decodes through a C `double`, which loses precision for the
decimal-float, fixed-point, and wide binary-float families. The accessors above
instead use the verbatim literal text (and bovnar's arbitrary-precision
`bvn_float`), so the full precision the format carries is reachable from Rust.

An integer is `i128` in a `Value` and `num_bigint::BigInt` where a number has no
width limit, a rational is `num_rational::BigRational`, and a decimal is
**`bovnar::Decimal`**: an exact decimal — sign, coefficient (`BigUint`) and
exponent, or an infinity or a NaN — that compares numerically and prints in the
General Decimal Arithmetic scientific-string form (`1.50` stays `1.50`). `Decimal` is the exact value
only; it does no arithmetic.

```rust
use bovnar::{Decimal, Value, parse_typed};

fn main() -> Result<(), bovnar::Error> {
    let doc = parse_typed(b".p=<float_dec:64> 3.141592653589793238462643383279503;")?;
    let Value::Struct(fields) = doc else { unreachable!() };
    let Value::Quantity(p) = &fields[0].1 else { unreachable!() };

    // Quantity::value is lossy: a C double
    assert_eq!(p.value(), Value::Float(3.141592653589793));
    // the literal, every digit of it ...
    let literal: Decimal = "3.141592653589793238462643383279503".parse()?;
    assert_eq!(p.decimal()?, Some(literal));
    // ... and the value decimal64 stores: 16 digits, rounded half to even
    assert_eq!(p.stored_value()?.map(|d| d.to_string()), Some("3.141592653589793".into()));
    assert_eq!(p.ieee_bits()?.len(), 8);

    let doc = parse_typed(b".x=<float_fix:32,q8> 3.27;")?;
    let Value::Struct(fields) = doc else { unreachable!() };
    let Value::Quantity(f) = &fields[0].1 else { unreachable!() };
    assert_eq!(f.fraction()?, Some(num_rational::BigRational::new(837.into(), 256.into())));
    assert_eq!(f.fixed_point()?, (837.into(), 8));
    Ok(())
}
```

These materialise over the **full** representable range, including exponents
the C parser cannot otherwise reach (beyond ~1e9865). `decimal` / `fraction`
work at every binary width bovnar allows (16, or a multiple of 32 up to 32768);
the bit-exact `stored_value` / `ieee_bits` apply to the IEEE encodings
(`float:16/32/64/128/256`), which are also exposed directly as `bovnar::BvnFloat`.

**`BvnFloat` and the `bvnfloat` module** reach the
library's arbitrary-precision float: the IEEE binary and decimal interchange
encodings and the fixed-point datum, plus the exact materialisers used past the
C parser's exponent cap.

```rust
use bovnar::BvnFloat;

fn main() -> Result<(), bovnar::Error> {
    let f = BvnFloat::from_str("0.1", 10, 256)?;     // text, base, precision in bits
    assert!(!f.is_nan() && !f.is_inf() && !f.is_zero());
    assert_eq!(f.to_double(), 0.1);
    let bits = f.to_binary_bits(32)?;                // binary32, little-endian
    assert_eq!(bits.len(), 4);
    assert_eq!(BvnFloat::from_binary_bits(32, &bits)?.to_double(), 0.1f32 as f64);
    Ok(())
}
```

**Exactness.** The `stored_value` of a `float_fix` is exact, never rounded to a
working precision, however wide the datum; the same goes for the literal of a
terminating `Fraction`.

### 6.4 `to_string()` integration

`to_string` dispatches on `Value::Quantity` before the plain `Int` / `Float`
path, so any value that came from `parse_typed` can be passed directly to
`to_string` and will produce identical output:

```rust
use bovnar::{parse_typed, to_string};

fn main() -> Result<(), bovnar::Error> {
    let bvnr = b".speed = <float:32,m/s> 9.81;";
    let doc = parse_typed(bvnr)?;
    let out = to_string(&doc, false)?;
    assert_eq!(out, b".speed=<float:32,m/s>9.81;");
    assert_eq!(parse_typed(&out)?, doc);
    Ok(())
}
```

The annotation is left out where it says nothing the literal does not — i.e.
when the type is `float:64` with no unit and no non-decimal base (matching the C
library's own default-annotation omission rules).

`to_string` also accepts a bare `Value::Decimal` (written as an exact
`float_dec:64` literal) and a terminating `Value::Fraction` — a non-terminating
fraction (e.g. `1/3`) fails with `Error::Argument`, as does a `Decimal` outside
the range of `float_dec:64`. An array whose elements are all `Decimal` /
`Fraction` is written under one `<float_dec:64>` annotation, so the elements
mean what the scalars mean:

```rust
use bovnar::{Value, to_string};
use num_rational::BigRational;

fn main() -> Result<(), bovnar::Error> {
    let half = BigRational::new(1.into(), 2.into());
    let doc = Value::Struct(vec![("h".into(), Value::Fraction(half))]);
    assert_eq!(to_string(&doc, false)?, b".h=<float_dec:64>0.5;");

    let third = BigRational::new(1.into(), 3.into());
    let bad = Value::Struct(vec![("t".into(), Value::Fraction(third))]);
    assert!(matches!(to_string(&bad, false), Err(bovnar::Error::Argument { .. })));
    Ok(())
}
```

**Arrays of `Quantity`.** An array has one annotation. Elements that agree on
type and unit are written under it. Elements that agree on the unit but not
the type — or that carry no unit and differ in type, `[<uint:8> 1, <float:64>
2.5]` — are each written under their own annotation, as they were read.
Elements that carry *different* units have no valid form and fail.

**Octet streams.** `parse()` returns an octet stream as `Value::Bytes`, and
`to_string()` writes `Value::Bytes` back as one, so a document holding one
round-trips.

**What "can be passed directly to `to_string()`" does not promise** is that the
result passes the *document* tier: `to_string()` writes what `parse()` reads,
and `parse()` reads heterogeneous and ragged arrays that `DomDoc::parse` and
`bovnar validate` refuse. See the note in [§4.1](#41-high-level-api-parse--to_string).

---

## 7. `write_array` helper

`write_array` is a free function at the crate root. It handles both flat and
multi-dimensional arrays, and takes the `Writer` to write into:

```rust
use bovnar::{Value, ValueTypeFamily, ValueTypeSpec, Writer, write_array};

fn main() -> Result<(), bovnar::Error> {
    let ints = |xs: &[i128]| xs.iter().map(|&x| Value::Int(x)).collect::<Vec<_>>();
    let mut w = Writer::to_mem(4096, true)?;

    // Single-row array
    write_array(&mut w, "primes", &ints(&[2, 3, 5, 7]), None, None, None)?;

    // Multi-row array (rows separated by /)
    let matrix = [Value::Array(ints(&[1, 2, 3])), Value::Array(ints(&[4, 5, 6]))];
    write_array(&mut w, "matrix", &matrix, None, None, None)?;

    // Typed array: whole-array type annotation
    let vt = ValueTypeSpec { family: ValueTypeFamily::Uint as i32, width: 16, base: 0 };
    write_array(&mut w, "ports", &ints(&[80, 443, 8080]), Some(vt), None, None)?;

    w.finish()?;
    let Value::Struct(fields) = bovnar::parse(w.get_output())? else { unreachable!() };
    assert_eq!(fields.len(), 3);
    Ok(())
}
```

The signature is `write_array(w, key, rows, vt, unit, unit_source)`: `vt` is the
whole-array annotation (`Option<ValueTypeSpec>`), `unit` an `Option<&Unit>`, and
`unit_source` the unit's spelling if it is to be kept.

Element types accepted per element: `Int`, `Float`, `String`, `Bool`, `Null`,
`Bytes` (an octet stream), `Struct` (written as a struct), `Quantity`, or a
nested `Array` (written as a nested array). When *rows* is a flat slice it is
treated as a single-row array; when all top-level elements are themselves
`Array`s it is treated as a multi-row array.

**With `vt`, every element is held to it.** An `Int` under an integer annotation
is checked against the width and written in the annotation's base — quoted where
the base requires it. An element the annotation cannot describe — `80800` under
`<uint:16>`, a `Float` or a `String` under an integer type, a `Struct` under
any — fails with `Error::Argument` instead of producing a document nothing
reads. `Null` is a null hole and is always allowed; a `Quantity` element carries
its own literal and is written as it is.

---

## 8. JSON conversion

The `bovnar::json` module mirrors the CLI's `bovnar convert` in both directions:

| Function | Description |
|---|---|
| `json::from_json(&str) -> Result<Vec<u8>>` | JSON text to a BVNR document. The root must be an object |
| `json::to_json(&[u8]) -> Result<String>` | A BVNR document to pretty-printed JSON, as `bovnar convert` prints it |
| `json::detect(&[u8]) -> Format` | Sniffs the format from the first significant byte: `Format::Json`, `Format::Bovnar` or `Format::Unknown`. The CLI goes by file extension; this is the content-based fallback for data without a name |

```rust
use bovnar::json::{self, Format};

fn main() -> Result<(), bovnar::Error> {
    let text = r#"{"name": "example", "ports": [8080, 443]}"#;
    assert_eq!(json::detect(text.as_bytes()), Format::Json);

    let bvnr = json::from_json(text)?;
    assert_eq!(json::detect(&bvnr), Format::Bovnar);
    let back = json::to_json(&bvnr)?;
    assert_eq!(json::from_json(&back)?, bvnr);
    Ok(())
}
```

JSON arrays become a single bracket row with nested arrays as nested rows (never
`/`-separated), as in the CLI. Nesting is limited to 255 levels — the writer's
and the reader's limit — and a document deeper than any reader takes is refused
as JSON rather than by the writer half-way through. NaN and the infinities have
no JSON spelling and are written as `null`.

**Two documented round-trip edge cases.** JSON has one number type, so two
values change on the way back, exactly as with the CLI:

- a float with an integral value becomes an integer — `1e+6` is written
  `1000000` and reads back as `uint`;
- `-0.0` is written `-0`, which reads back as `0`.

```rust
use bovnar::json::{from_json, to_json};

fn main() -> Result<(), bovnar::Error> {
    let first = to_json(&bovnar::to_string(
        &bovnar::Value::Struct(vec![("big".into(), bovnar::Value::Float(1e6))]),
        true,
    )?)?;
    assert!(first.contains("1000000"));            // not 1e+6
    let back = from_json(&first)?;                 // ... and it reads back as an integer
    assert_eq!(to_json(&back)?, first);            // from here on the JSON is stable
    Ok(())
}
```

From the second trip on, the JSON is stable: the first `to_json`/`from_json` pair
normalises, and nothing after it changes.

---

## 9. Currency helpers

The `bovnar::currency` module provides metadata for the ISO 4217 fiat and
cryptocurrency `BaseUnit` members. The catalogue is read from the C library, so
there is no table to keep in step.

```rust
use bovnar::BaseUnit;
use bovnar::currency::{
    all_crypto, all_fiat, currency_code, currency_info, from_code, is_crypto, is_currency, is_fiat,
    minor_unit,
};

fn main() -> Result<(), bovnar::Error> {
    assert!(is_currency(BaseUnit::Usd));
    assert!(is_fiat(BaseUnit::Usd));
    assert!(is_crypto(BaseUnit::Btc));

    let info = currency_info(BaseUnit::Usd)?;
    assert_eq!(info.code, "USD");
    assert_eq!(info.numeric_code, 840);   // ISO 4217 numeric; 0 for crypto
    assert_eq!(info.minor_unit, 2);       // decimal places: 1 major = 10^minor minor units
    assert_eq!(info.name, "US Dollar");

    assert_eq!(minor_unit(BaseUnit::Jpy)?, 0);
    assert_eq!(currency_code(BaseUnit::Eur)?, "EUR");
    assert_eq!(from_code("gbp")?, BaseUnit::Gbp);   // surrounding space and case are ignored

    assert!(all_fiat().any(|ci| ci.code == "CHF")); // all_crypto() / all_currencies() also exist
    assert!(all_crypto().all(|ci| ci.numeric_code == 0));
    Ok(())
}
```

`CurrencyInfo` is a plain struct (`base`, `code`, `numeric_code`, `minor_unit`,
`name`) with an `is_fiat()` method; its `Display` is the code. `currency_info`, `minor_unit`, `currency_name`, `currency_code` and
`from_code` fail with `Error::Argument` for non-currency bases or unknown codes.
`CURRENCY_FIRST`, `CURRENCY_LAST` and `CURRENCY_COUNT` are re-exported from the
module.

---

## 10. `Reader` reference

### 10.1 Construction

```rust
use bovnar::Reader;

fn main() -> Result<(), bovnar::Error> {
    let r = Reader::new()?;
    drop(r); // the C object is released when the Reader is dropped
    Ok(())
}
```

`Reader::new` calls `bvnr_reader_create` immediately. There is no context
manager and no `close()`: the C object is released when the `Reader` is dropped,
and a dropped reader cannot be used again because the compiler has moved it.

### 10.2 `read_mem`

```rust,ignore
read_mem(data: &[u8], options: &ReadOptions,
         on_verified: Option<&mut dyn FnMut(&EventData) -> bool>,
         on_unverified: Option<&mut dyn FnMut(&EventData) -> bool>) -> Result<()>

read_mem_want_unit(data, options, want_unit: &mut dyn FnMut(&EventData) -> Option<(Unit, u32)>,
                   on_verified) -> Result<()>
```

Parse BVNR from a byte slice. The options are the `ReadOptions` struct, and the
`want_unit` hook is the separate method `read_mem_want_unit`.

| Parameter | Type | Description |
|---|---|---|
| `data` | `&[u8]` | Input buffer |
| `options` | `&ReadOptions` | See [§10.3](#103-readoptions) |
| `on_verified` | `Option<&mut dyn FnMut(&EventData) -> bool>` | Callback for validated events |
| `on_unverified` | `Option<&mut dyn FnMut(&EventData) -> bool>` | Callback for raw pre-validation events |
| `want_unit` | `&mut dyn FnMut(&EventData) -> Option<(Unit, u32)>` | Read-time lossless unit/base conversion hook. Called per numeric value; return `Some((unit, base))` to request a conversion (the exact result arrives on the payload as `converted` and `converted_text`, with `converted_rational()` / `converted_in_base()` beside it — see §4.2), or `None` to leave the value untouched. An inexact conversion aborts the read (`error_unit_inexact`) |

A reader opens at the 255-level struct and array nesting cap, in both
directions, like the CLI and the DOM.

The file-descriptor entry points `read_fd` and `read_file` are `read_reader` and
`read_reader_want_unit`, over any `std::io::Read` — see
[§4.7](#47-files-and-io-streams-stdioread--write).

### 10.3 `ReadOptions`

`ReadOptions` is plain data with a `Default`; fill what you need and leave the
rest with `..ReadOptions::default()`.

| Field | Type | Default | Description |
|---|---|---|---|
| `max_file_size` | `u64` | `0` (unlimited) | Hard limit on bytes consumed; past it the read fails with `file_too_long` |
| `continue_on_error` | `bool` | `false` | Enable resync mode |
| `strict_version` | `bool` | `false` | Reject a declared spec version newer than this build (`error_unsupported_spec_version`) |
| `text_only` | `bool` | `false` | Refuse a document that contains an octet stream (`error_octet_stream_forbidden`), for a channel that is not byte-transparent |
| `want_unit_allow_nonterminating` | `bool` | `false` | Deliver an exact-rational-but-non-terminating conversion (e.g. km/h→m/s) as a rational — `converted_rational()` returns the exact pair — instead of aborting with `error_unit_inexact` |
| `max_conversion_length` | `u32` | `0` (default 1024) | Longest converted text a conversion may produce, in characters; a longer exact result aborts with `error_value_out_of_range` |

`max_file_size` is 0 by default, which means no cap; set it where a cap matters.

### 10.4 `EventData` and `EventPayload`

`EventData<'a>` — the table in [§4.2](#42-sax-style-streaming-reader) — is the
borrowed view a callback receives. `EventPayload` is the owned copy: the same
fields without the borrow, with `raw` a `Vec<u8>`, `converted_text` an
`Option<String>` (`None` when the conversion does not terminate in
`converted_base`), and a `text()` method.

| Field | Type | Description |
|---|---|---|
| `event` | `Event` | The event code |
| `token_type` | `Option<TokenType>` | How the token was written |
| `value_type` | `ValueTypeSpec` | Type annotation if present |
| `value_unit` | `Unit` | Unit if present |
| `raw` | `Vec<u8>` | Raw token bytes |
| `text()` | `String` | `raw` decoded as UTF-8, invalid bytes replaced |
| `converted` | `bool` | `true` when a `want_unit` conversion was applied to this value |
| `converted_text` | `Option<String>` | Exact converted value as a positional string; `None` if the conversion does not terminate in `converted_base` |
| `converted_base` | `u32` | Base the converted text is rendered in |

### 10.5 `iter_mem`

`iter_mem(data, verified_only, max_file_size) -> Result<Vec<EventPayload>>`
reads `data` and returns all its events, in order. It is a `Vec`, not a lazy
iterator: the read has finished when it returns.

| Parameter | Description |
|---|---|
| `verified_only` | When `false`, both callbacks fire and events may appear twice |
| `max_file_size` | Forwarded to `read_mem`; `0` is unlimited |

### 10.6 Error-state accessors

These query the C reader object after a failed parse. They describe the error
that **ended** the read; the `Error::Parse` that `read_mem` returned carries the
same code and position. An error recovered under `continue_on_error: true` does
not appear here: after a read that succeeded, `error_code()` is
`Some(ErrorCode::None)` and the line is `0`, however many errors were recovered,
and `recovery_count()` is how to tell that any were (spec §13.4).

| Method | Type | Description |
|---|---|---|
| `error_code()` | `Option<ErrorCode>` | The error that ended the read; `ErrorCode::None` after a successful one |
| `error_line()` | `u64` | 1-based line number of the error |
| `recovery_count()` | `u64` | Number of times resync was entered (incremented at error entry, not at resync completion) |
| `skipped_bytes()` | `u64` | How many input bytes a `continue_on_error` read skipped |
| `declared_version()` | `Option<(u16, u16)>` | The `#!bovnar M.N` directive of the last read, or `None` when the document carried none |

The column and the byte offset of the error are on the `Error` itself — see
[§13](#13-error-handling) — as `position()`.

---

## 11. `Writer` reference

### 11.1 Construction

| Method | Description |
|---|---|
| `Writer::to_mem(cap, pretty)` | Write to an in-process buffer of `cap` bytes; `pretty` indents the output. Writing past `cap` fails with `sink_buffer_exhausted` (`DEFAULT_MEM_CAP` is 4 MiB) |
| `Writer::to_mem_with(cap, pretty, unit_flags, emit_version)` | `to_mem` with two more settings: `unit_flags` ([`UnitFlags`](#53-unitflags)) rewrite units on output, `emit_version` leads the document with `#!bovnar <major>.<minor>` |
| `Writer::to_writer(out, pretty)` | Write to any `std::io::Write`: the document is put together in a memory sink of `DEFAULT_MEM_CAP` bytes and `finish()` writes it to `out` and flushes it |
| `Writer::to_writer_with(out, cap, pretty, unit_flags, emit_version)` | `to_writer` with a sink of `cap` bytes, plus `unit_flags` and `emit_version` as in `to_mem_with` |

See [§4.7](#47-files-and-io-streams-stdioread--write) for the I/O writer. Every writer accepts structs and arrays nested up to **255** deep,
the same limit `Reader` and `parse()` read at. `unit_flags` takes
[`UnitFlags`](#53-unitflags) — `REDUCE` rewrites each unit and rescales its
value, `ASCII_EXP` writes `^N` exponents, `NUM_CANONICAL` keeps a rescaled float
in float shape. `emit_version: true` writes a leading `#!bovnar M.N` naming the
spec version this build implements, as `write_version()` would.

The high-level `to_string` needs no buffer size: it retries on a writer twice
the size until the output fits (up to 256 MiB).

### 11.2 Output retrieval

| Method | Description |
|---|---|
| `w.get_output() -> &[u8]` | The document written so far |
| `Writer::bytes_written -> u64` | Number of bytes written |
| `w.finish()` | Flush and seal the output. Fails with `Error::Write` if any struct is still open. For a `to_writer` writer it also writes the document to its `std::io::Write`, once |

There is no `destroy()`: dropping the `Writer` releases the C object.

### 11.3 Scalar write methods

Every scalar writer takes the key first and returns `Result<()>`. They take no
unit parameters: a value that carries a unit is written with `write_quantity` (or inside [`write_array`](#7-write_array-helper)),
which take a `Unit`.

#### `write_uint(key, value: u64, width: u32, base: Base)`

Write an unsigned integer. `base` selects the numeral system (`Base::DECIMAL` or
`Base::HEX` for inline values; any Bovnar-supported base for `write_bvni`).

#### `write_sint(key, value: i64, width: u32, base: Base)`

Write a signed integer. `base` 64 and 85 are **unsigned** — `+` and `-` are
digits in those alphabets — and fail with `Error::Argument` here and for a
signed `write_bvni`.

#### `write_float(key, value: f64, width: u32)`

Write an IEEE 754 binary float.

#### `write_float_fix(key, value: f64, width: u32, q: u32)`

Write a Q-format fixed-point value. `q` is the number of fractional bits
(`0 ≤ q < width`).

#### `write_float_dec(key, value: f64, width: u32)`

Write an IEEE 754-2008 decimal float.

#### `write_string(key, value: &str)`

Write a bare quoted UTF-8 string with no type annotation:

```bovnar
.host = "localhost";
```

To emit an explicit `<utf8>` annotation, use the low-level `emit` API with
`Event::TypeAnnotationStart` / `TypeAnnotationEnd` before `Event::Data`.

#### `write_bool(key, value: bool)`

Write a `bool` value — serialized as the bare keyword `true` or `false`. On
read-back it decodes to `Value::Bool`.

#### `write_null(key)`

Write a null value (empty slot).

#### `write_octets(key, value: &[u8])`

Write an octet stream, chunked. It reads back as `Value::Bytes`.

#### `write_quantity(key, q: &Quantity)`

Write a typed scalar as `parse_typed` read it: type, unit, unit spelling and
datetime fraction are kept. The annotation is left out where it says nothing
the literal does not.

```rust
use bovnar::{Quantity, Unit, ValueTypeFamily, ValueTypeSpec, Writer};

fn main() -> Result<(), bovnar::Error> {
    let float64 = ValueTypeSpec { family: ValueTypeFamily::Float as i32, width: 64, base: 0 };
    let mut w = Writer::to_mem(4096, false)?;
    w.write_quantity("velocity", &Quantity::new("9.81", float64, Unit::parse("m/s")?))?;
    w.finish()?;
    assert_eq!(w.get_output(), b".velocity=<float:64,m/s>9.81;");
    Ok(())
}
```

### 11.4 Extended integer writers

#### `write_bvni(key, value: i128, width: u32, base: Base, signed: Signedness)`

Arbitrary-width integer writer that supports all Bovnar numeral bases (**2–62,
64, and 85**), for an integer of any width up to 128 bits. Non-decimal values
are formatted with bovnar's own big-integer arithmetic and emitted as quoted
strings. `signed` is a
`Signedness`: `Signed`, `Unsigned`, or `Auto` — `sint` for a negative value,
`uint` otherwise (the default). `format_bigint(value, base)` renders an integer in
any base on its own.

#### `write_bvnf_base(key, literal: &str, width: u32, base: Base)`

Write a float from a pre-formatted string in base 10 or 16. `base` must be
`Base::DECIMAL` or `Base::HEX`; any other value fails with `Error::Argument`.

#### `Base`

`Base` is a checked newtype over the numeral bases bovnar has an alphabet for —
2 to 62, 64 and 85 — rather than an enum of 63 variants. The common ones are
named: `BINARY`, `OCTAL`, `DECIMAL`, `HEX`, `BASE36`, `BASE62`, `BASE64`,
`BASE85`. `Base::new(n)` is `Some` for a base bovnar has an alphabet for and
`None` otherwise; `get()` returns the number.

```rust
use bovnar::{Base, Signedness, Writer};

fn main() -> Result<(), bovnar::Error> {
    assert_eq!(Base::new(16), Some(Base::HEX));
    assert_eq!(Base::new(63), None);

    let mut w = Writer::to_mem(4096, false)?;
    w.write_bvni("big", 1i128 << 100, 128, Base::DECIMAL, Signedness::Auto)?;
    w.write_bvni("neg", -255, 16, Base::HEX, Signedness::Auto)?;
    w.finish()?;
    assert_eq!(bovnar::parse(w.get_output())?, bovnar::parse(
        b".big = <uint:128> 1267650600228229401496703205376; .neg = <sint:16,_16> \"-ff\";",
    )?);
    Ok(())
}
```

### 11.5 Struct helpers

```rust
use bovnar::Writer;

fn main() -> Result<(), bovnar::Error> {
    let mut w = Writer::to_mem(4096, true)?;
    w.begin_struct("server")?;        // emit AssignmentStart + StructStart, increment depth
    w.write_string("host", "example.com")?;
    w.end_struct()?;                  // emit StructEnd, decrement depth
    w.finish()?;
    Ok(())
}
```

`finish()` verifies that the struct depth is zero; an unclosed struct fails with
`Error::Write` (`GotIncompleteBvnrStream`).

### 11.6 Version directive

```rust
use bovnar::Writer;

fn main() -> Result<(), bovnar::Error> {
    let mut w = Writer::to_mem(4096, true)?;
    w.write_version(1, 1)?;           // emit a leading "#!bovnar 1.1" directive
    w.write_uint("n", 1, 8, bovnar::Base::DECIMAL)?;
    w.finish()?;
    assert!(w.get_output().starts_with(b"#!bovnar 1.1"));
    Ok(())
}
```

Emits a leading `#!bovnar <major>.<minor>` directive. Must be called before any
value is written; calling it after output has begun fails with `Error::Write`
(`InvalidArgument`). Use it to self-declare the spec version a document targets
(spec-1.1 constructs such as datetime literals need it to re-read).

### 11.7 Array helpers

```rust
use bovnar::{EmitSpec, Event, ValueTypeSpec, Writer};

fn main() -> Result<(), bovnar::Error> {
    let plain = ValueTypeSpec::default();
    let mut w = Writer::to_mem(4096, false)?;
    w.emit(Event::AssignmentStart, EmitSpec::key("m"))?;
    for (i, row) in [["1", "2"], ["3", "4"]].iter().enumerate() {
        if i > 0 {
            w.new_array_dim()?;     // emit ArrayDimStart (the / separator between rows)
        }
        w.begin_array_row()?;       // emit ArrayRowStart
        for n in row {
            w.emit(Event::Data, EmitSpec::value(n, plain))?;
        }
        w.end_array_row()?;         // emit ArrayRowEnd
    }
    w.finish()?;
    assert_eq!(bovnar::parse(w.get_output())?, bovnar::parse(b".m = [1, 2] / [3, 4];")?);
    Ok(())
}
```

These are the raw events; [`write_array`](#7-write_array-helper) is the helper
that drives them for you.

### 11.8 Low-level `emit`

```rust,ignore
w.emit(event: Event, spec: EmitSpec<'_>) -> Result<()>
```

Send an arbitrary event to the C writer. What goes with it — a key, a value and a type
annotation — is an `EmitSpec`; it carries no unit. When both a `vt` and a `value` are supplied the token type is
inferred: a string token for `ValueTypeFamily::Utf8`, a number otherwise.

| Purpose | Call |
|---|---|
| Start an assignment with a key | `w.emit(Event::AssignmentStart, EmitSpec::key("x"))` |
| Write a typed value | `w.emit(Event::Data, EmitSpec::value("42", spec))` |
| End a struct (nothing attached) | `w.emit(Event::StructEnd, EmitSpec::NONE)` |

```rust
use bovnar::{EmitSpec, Event, ValueTypeFamily, ValueTypeSpec, Writer};

fn main() -> Result<(), bovnar::Error> {
    let spec = ValueTypeSpec { family: ValueTypeFamily::Uint as i32, width: 16, base: 0 };
    let mut w = Writer::to_mem(4096, false)?;
    w.emit(Event::AssignmentStart, EmitSpec::key("x"))?;
    w.emit(Event::Data, EmitSpec::value("42", spec))?;
    w.finish()?;
    assert_eq!(bovnar::parse(w.get_output())?, bovnar::parse(b".x = <uint:16> 42;")?);
    Ok(())
}
```

---

## 12. DOM API

The DOM API parses a complete BVNR document into an in-memory tree for
random-access queries without writing a SAX callback.

### 12.1 `DomDoc::parse`

```rust
use bovnar::DomDoc;

fn main() -> Result<(), bovnar::Error> {
    let doc = DomDoc::parse(b".a = 1;")?;
    assert_eq!(doc.len(), 1);
    Ok(())
}
```

A malformed document is an `Error::Parse` carrying the C error code. **It has no
position:** the C DOM reports only the code (`bvn_dom_doc_get_parse_error`), so
`Error::position()` is `None`. To locate the fault, read the same document with
a `Reader`, which does report where.

### 12.2 `DomDoc`

Owning wrapper around `bvn_dom_doc_t`. Dropping it frees the entire tree; the
`DomNode`s derived from it borrow it, so none can outlive it.

| Method | Description |
|---|---|
| `DomDoc::parse(data)` | Parse a byte slice |
| `DomDoc::parse_reader(input)` | `parse` over any `std::io::Read`; see the note below |
| `DomDoc::parse_reader_with_policy(input, &policy)` | `parse_reader` under a policy |
| `DomDoc::parse_with_policy(data, &policy)` | Parse under a [`UnitPolicy`](#56-unitpolicy--validation-and-conversion-without-a-callback): values arrive converted, and a document breaking the policy fails with `unit_mismatch` |
| `doc.get(key)` | The top-level value assigned to `key`, as `Option<DomNode>` |
| `doc.lookup(path)` | Dot-separated path lookup, e.g. `"server.tls.cert"`, with indexed elements (`"matrix[1][0]"`). `None` when absent |
| `doc.len()` / `doc.is_empty()` | Number of top-level entries |
| `doc.entries()` | All top-level `(String, DomNode)` pairs, in document order |
| `doc.to_value()` | The whole document as a `Value::Struct`, dropping type and unit info. Fails where a node cannot be read as its kind — see `node.to_value()` |

> **File-size limits:** `DomDoc::parse_reader` and `parse_reader_with_policy` take
> any `std::io::Read` and apply the same ceiling, `DOM_READ_MAX_BYTES`
> (256 MiB, `BVN_DOM_FD_MAX_BYTES`). A longer input is an `Error::Parse` with
> `file_too_long`, where the C function returns no document. Input over 4 GiB is refused as well (the C API's length is a
> `uint32_t`).

Membership is `get(key).is_some()`, and iteration is `entries()`.

### 12.3 `DomNode`

A `Copy` view into a `bvn_dom_node_t` that borrows its `DomDoc`.

| Method | Description |
|---|---|
| `dom_type()` | `Option<DomType>` — `None` for a kind this binding does not know |
| `node.value_type()` | `ValueTypeSpec` |
| `node.unit()` | `Unit`; dimensionless when the value has none |
| `unit_str()` | Unit as a canonical string via `bvn_dom_get_unit_string`, or `""` when the value carries no unit — the same contract as `unit_str()` (the C formatter's `no_unit` spelling is never returned) |
| `node.is_null()` | `true` for null values |
| `node.value_in_base_units()` | Numeric value scaled to SI base units (`f64`); an affine unit gets its offset applied (`25 °C` → `298.15`). Returns `0.0` for *any* failure — non-numeric node, currency, affine-in-compound — which is indistinguishable from a genuine zero, and it rounds to `double`. Use `units::unit_to_si_factor()` (which reports the failure) or the `want_unit` hook when either matters |
| `node.as_i64()` / `as_u64()` | Signed / unsigned 64-bit integer, as an `Option` |
| `node.as_i32()` / `as_u32()` | Signed / unsigned 32-bit integer |
| `node.as_i16()` / `as_u16()` | Signed / unsigned 16-bit integer |
| `node.as_i8()` / `as_u8()` | Signed / unsigned 8-bit integer |
| `node.as_float()` | `Option<f64>` |
| `node.as_bool()` | `Option<bool>` for `Bool` nodes |
| `node.as_str()` | `Option<&str>` for `String`, `Symbol`, or `Reference` nodes |
| `node.as_bytes()` | `Option<&[u8]>` for `OctetStream` nodes |
| `node.as_int_str(base)` | Any integer node, however wide, as digits in the given base |
| `datetime_fraction()` | `Option<&str>` of the verbatim ISO sub-second digits for a `datetime` written as a literal with a fraction (spec 1.1) |
| `node.unit_source()` | `Option<&str>` of the unit exactly as the document spelled it (library 2.0), where that differs from the canonical form — `ucum:mL{total}` for a unit of `m~L`; `None` for a node with no unit, one already written canonically, or one a unit policy converted |
| `node.get(key)` | Child `DomNode` by string key (struct) |
| `node.at(index)` | Child `DomNode` by flat index (array) |
| `node.len()` / `is_empty()` | Element count for struct or array nodes, `0` otherwise |
| `node.entries()` | `(String, DomNode)` pairs (struct nodes) |
| `node.array_dims()` | Number of `/`-separated dimensions (array nodes only) |
| `node.to_value()` | Recursively convert to a plain `Value` (drops type/unit info). A multi-row array comes back as an array of rows, empty rows included (`[]/[]` → `[[], []]`), as `parse()` gives it |

**The `as_*` accessors return `Option`.** A node that is not that kind gives `None`
rather than a zero or a panic, and `as_i64` on a
`float` node is `None` rather than a truncated number.

**`to_value` fails rather than inventing a value.** Text or a key that is not
UTF-8, a float or bool node without its value, or an unknown node kind is an
`Error`, rather than an invented value: two distinct keys must not collapse
into one.

### 12.4 `DomType`

```text
Null=0  Int=1  Float=2  String=3  Symbol=4
Reference=5  Struct=6  Array=7  OctetStream=8  Bool=9
```

`DomType::from_raw(i32)` converts a raw C value.

---

## 13. Error handling

Every fallible call returns `bovnar::Result<T>`, whose error is `bovnar::Error`.
`Error` has one variant per failure class:

| Variant | When raised |
|---|---|
| `Error::Parse { code, position }` | Parse error in `Reader` or `DomDoc` (carries the code and, where the library reported one, the `Position`) |
| `Error::Write { code, offset }` | Write error in `Writer` |
| `Error::Argument { code, message }` | Invalid argument passed to a helper (e.g. bad unit string, refused unit policy) |
| `Error::OutOfMemory(call)` | The library could not allocate; names the call that failed |
| `Error::Io { code, io_kind, message }` | The `std::io::Read` or `Write` behind `read_reader`, `parse_reader`, `write_to`, `to_writer` or a `stream` twin failed. `code` is what the C library reports for an unreadable fd source or unwritable fd sink (`reading_from_source_fd`, `writing_to_sink`); `io_kind` and `message` keep what the I/O error said |

The library is part of the crate, so there is no library-not-found error. `Error`
implements `std::error::Error` and `Display`, so it works with `?` into
`Box<dyn Error>` and `anyhow`.

`kind()` gives the library's error name as an `ErrorKind` (an alias of
`ErrorCode`) wherever the library reported one — an unparsable unit or a refused
policy included — `name()` its C spelling, `code_raw()` the raw integer, and
`position()` the line, column and byte offset where the input was refused:

```rust
use bovnar::{Error, ErrorKind, Unit};

fn main() {
    let err = bovnar::parse(b".a = 1;\n.b = @;").unwrap_err();
    assert_eq!(err.kind(), Some(ErrorKind::UnexpectedInputByte));
    assert_eq!(err.name(), Some("unexpected_input_byte"));
    let pos = err.position().expect("a parse error from the reader has a position");
    assert_eq!(pos.line, 2);
    assert!(pos.column > 0 && pos.offset > 0);
    assert_eq!(pos.byte, b'@' as u32);              // the byte the error was raised on

    match Unit::parse("furlong per fortnight") {
        Err(Error::Argument { code, message }) => {
            assert!(code.is_some(), "the library said why: {message}");
        }
        other => panic!("expected an argument error, got {other:?}"),
    }
}
```

`position()` is `None` for errors the library locates no better than by code: the
DOM parser, and the stream helpers' framing errors. `kind()` is `None` for an
argument error caught before the library and for an allocation failure.

**`unit_convert_factor` error semantics.** The C function uses the
`(ok, requires_affine)` pair to signal four distinct outcomes:

| ok | requires_affine | Meaning |
|---|---|---|
| `true` | `false` | Multiplicative conversion; `factor` is ready to use |
| `true` | `true` | Both sides are affine and share their zero (e.g. °C ↔ °Re, Réaumur), so `factor` alone is correct |
| `false` | `true` | Affine conversion the offsets do not cancel (e.g. °C ↔ K). **`factor` is `0.0`** — not a usable multiplier |
| `false` | `false` | Units are dimensionally incompatible → `Error::Argument` |

`unit_convert_factor` fails with `Error::Argument` **only** when both `ok=false`
and `requires_affine=false`; the result does not carry `ok`, so check
`requires_affine` before multiplying by `factor`. When it is set, call
`convert_value`, which handles the two-step affine path, or call
`unit_to_si_factor` on both units and convert by hand. The `Unit` method
`convert_factor()` returns the bare number and therefore fails for the `0.0`
row instead of returning it.

**Callbacks returning `false`.** When an `on_verified` or `on_unverified`
callback returns `false`, the C parser stops and `read_mem` returns
`Error::Parse` with whatever error code the reader recorded.

**Callbacks that panic.** If the callback panics, the panic is caught at the FFI
boundary, the C callback returns `false` to stop the parse, and the original
panic is resumed from `read_mem` after the C call returns — a Rust panic never
unwinds through C frames.

---

## 14. Threads

`Reader`, `Writer` and `DomDoc` are `Send` but not `Sync`. As `bovnar.h`
(THREADING) allows, each can be moved to another thread, and distinct ones can
be used on distinct threads, but one object must not be shared: a single reader
or writer holds the whole parse or serialise state of one stream.

```rust
use bovnar::{DomDoc, Writer};

fn main() -> Result<(), bovnar::Error> {
    let doc = DomDoc::parse(b".n = 7;")?;
    // A DomDoc moves to another thread; a DomNode borrows it, so it stays with its document.
    let n = std::thread::spawn(move || doc.get("n").and_then(|n| n.as_i64())).join().unwrap();
    assert_eq!(n, Some(7));

    // Distinct writers on distinct threads need no locking
    let outputs: Vec<Vec<u8>> = (0..4u64)
        .map(|i| {
            std::thread::spawn(move || {
                let mut w = Writer::to_mem(4096, false).unwrap();
                w.write_uint("i", i, 8, bovnar::Base::DECIMAL).unwrap();
                w.finish().unwrap();
                w.get_output().to_vec()
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    assert_eq!(outputs.len(), 4);
    Ok(())
}
```

`parse`, `parse_typed`, `to_string`, their `_reader` / `write_to` twins and the `units`, `currency` and `json`
functions are free functions that build their own reader, writer or DOM for each
call, so no two calls share an object and they can run on several threads at
once.

---

## 15. Running the test suite

```bash
cd rust

# Run everything
cargo test

# One test file
cargo test --test test_units

# The documentation examples of rust/README.md
cargo test --doc
```

`cargo test` runs the unit tests in `src/`, the integration tests in `tests/`
(one file per topic — `test_units.rs`, `test_dom.rs`, `test_stream.rs` and so
on — plus `smoke.rs`) and the documentation examples. The ABI check is three
unit tests in `src/ffi.rs`, because the C structs it compares against are
private.

`test_writers_agree` compares `to_string` with the C CLI's `pretty-print`, so it
is `#[ignore]`d unless the CLI is built at `<repo>/build/bovnar`:

```bash
cmake -B build . && cmake --build build --target bovnar
cargo test --test test_writers_agree -- --ignored
```

The C library's own suite runs unchanged through CMake: `cmake -B build . &&
cmake --build build && ctest --test-dir build`. That suite also parses every
`bovnar` code block of this document (`ctest -R bvnr_doc`).

`rust/fuzz/` holds [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets
(nightly toolchain): `parse` (the streaming parsers and a write-back round
trip), `dom` (parse and walk every node) and `json_roundtrip` (`from_json` and
`to_json` until the JSON is stable). Instrument the C library too by building it
with clang:

```bash
cd rust/fuzz
CC=clang CFLAGS="-fsanitize=fuzzer-no-link,address" \
    cargo +nightly fuzz run -O parse -- -max_total_time=600
```

---

## 16. Crate layout

```text
rust/
├── Cargo.toml        # version = the C library's version; publish = false
├── build.rs          # generates the unit tables, compiles ../src and ../include
├── src/
│   ├── lib.rs        # parse() / parse_typed() / to_string() / write_array() — public API
│   ├── ffi.rs        # the C declarations; the only `unsafe` code
│   ├── bvnfloat.rs   # BvnFloat: arbitrary-precision float + IEEE binary/decimal/fixed encoders
│   ├── currency.rs   # ISO 4217 / crypto currency metadata (read from the C catalogue)
│   ├── decimal.rs    # Decimal: an exact decimal number (sign, coefficient, exponent)
│   ├── dom.rs        # DomDoc, DomNode, DomType — random-access DOM
│   ├── enums.rs      # Rust enums mirroring the C enums
│   ├── error.rs      # Error, ErrorKind, Position
│   ├── io.rs         # reads a caller's std::io::Read for read_reader / parse_reader
│   ├── json.rs       # from_json / to_json / detect (the CLI's `bovnar convert`)
│   ├── quantity.rs   # Quantity — typed, unit-annotated scalar for lossless round-trips
│   ├── reader.rs     # Reader, ReadOptions, EventData, EventPayload, UnitPolicy, UnitRule
│   ├── stream.rs     # framing, octet multiplexing, document-in-document
│   ├── unit.rs       # Unit, UnitComponent — a unit with equality, hashing and its operations
│   ├── units.rs      # unit_to_si_factor, unit_convert_factor, etc.
│   └── writer.rs     # Writer, Base, Signedness, EmitSpec
├── tests/            # one integration-test file per topic, plus smoke.rs
└── fuzz/             # cargo-fuzz targets
```

| Module | Contents |
|---|---|
| `src/ffi.rs` | the C declarations and the mirrored C structs |
| `src/reader.rs` | `Reader`, `ReadOptions`, `EventData`, `EventPayload`, `UnitPolicy`, `UnitRule` |
| `src/writer.rs` | `Writer`, `Base`, `Signedness`, `EmitSpec` |
| `src/dom.rs` | `DomDoc`, `DomNode`, `DomType` |
| `src/enums.rs` | the Rust enums behind the C enums, `Exponent` and `Base` |
| `src/error.rs` | `Error`, `ErrorKind`, `Position` |
| `src/unit.rs` | `Unit`, `UnitComponent` |
| `src/units.rs` | the unit functions: conversion, dimension vectors, profile notations |
| `src/quantity.rs` | `Quantity` |
| `src/bvnfloat.rs` | `BvnFloat` and the IEEE / fixed-point encoders |
| `src/decimal.rs` | `Decimal`, the exact decimal value |
| `src/currency.rs` | currency metadata |
| `src/stream.rs` | framing, multiplexing and document-in-document, with their `Read` / `Write` twins |
| `src/io.rs` | the `std::io::Read` and `Write` helpers behind the `_reader` and `_to` entry points |
| `src/json.rs` | JSON ⇄ BVNR, as the CLI's `bovnar convert` does it |
| `src/lib.rs` | `parse`, `parse_typed`, `to_string`, the reader and writer twins, `write_array` and the crate-root re-exports |

**Out of scope.** The crate has no bridges to third-party unit or array
libraries. It takes no raw file descriptors or paths either: the entry points
that read or write a whole document are `std::io::Read` / `std::io::Write` ones
([§4.7](#47-files-and-io-streams-stdioread--write)), and a file is an opened
`std::fs::File` passed to them.

---

## 17. Enums and `BaseUnit`

The C enums are Rust enums with CamelCase variants (the C member `METER` is
`BaseUnit::Meter`): `Event`, `TokenType`,
`ValueTypeFamily`, `ErrorCode` (also `ErrorKind`), `BaseUnit`, `PrefixSystem`,
`SIPrefix` and `IECPrefix`. Raw C values convert with `TryFrom<i32>`, which
refuses an unknown value with an `UnknownValue` error, and back with
`i32::from` (or `as i32`). Each also has `from_raw`, which returns an `Option`.

`Exponent` and `Base` are open ranges, so they are checked newtypes with the
common values named (`Exponent::LINEAR`, `Exponent::NEG_SQUARE`, ...).
`Exponent` runs from `EXPONENT_MIN` (−100) to `EXPONENT_MAX` (100).

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

The `BaseUnit` enum mirrors the C `value_base_unit_e`. See doc/05 §12.1 for the
block layout — an id's leading two digits name the vocabulary it comes from. The
native units are 100000–100263 (`UNIT_NATIVE_FIRST` … `UNIT_NATIVE_LAST`) and the
currencies 900000–900215 (`CURRENCY_FIRST` … `CURRENCY_LAST`), with the
CamelCase spelling of each member (`POUND_FORCE` is `PoundForce`, `GERMAN_MILE`
is `GermanMile`):

| Range | Members |
|---|---|
| 0 | `None` |
| 100000–100001 | `Bit`, `Byte` |
| 100002–100008 | SI base units (`Second` … `Candela`) |
| 100009–100027 | Named SI derived units (`Hertz` … `Katal`) |
| 100028–100043 | Non-SI accepted units (`Liter` … `Year`) |
| 100044–100053 | Imperial/US length (`Inch` … `Fathom`) |
| 100054–100061 | Imperial/US mass (`Pound` … `Carat`) |
| 100062 | `Fahrenheit` |
| 100063–100066 | Pressure (`Atmosphere` … `Psi`) |
| 100067–100070 | Energy (`Calorie` … `Therm`) |
| 100071 | `Horsepower` |
| 100072–100074 | Force (`PoundForce`, `Dyne`, `Kip`) |
| 100075 | `Knot` |
| 100076–100084 | US volume (`Gallon` … `Barrel`) |
| 100085–100086 | Area (`Acre`, `Barn`) |
| 100087–100089 | Angle (`Arcminute`, `Arcsecond`, `Grad`) |
| 100090–100097 | CGS units (`Poise` … `Galileo`) |
| 100098–100100 | Radiation (`Curie`, `Roentgen`, `Rem`) |
| 100101–100102 | Logarithmic (`Neper`, `Decibel`) |
| 100103 | `Rankine` |
| 100104 | `Slug` |
| 100105 | `Thou` |
| 100106–100108 | UK imperial volume (`PintUk`, `FluidOunceUk`, `QuartUk`) |
| 100109–100110 | Electrical power (`Var`, `VoltAmpere`) |
| 100111 | `KilogramForce` |
| 100112 | `InchHg` |
| 100113 | `Rpm` |
| 100114 | `FootPound` |
| 100115–100116 | Mass additional (`Dram`, `Pennyweight`) |
| 100117–100118 | Length additional (`Chain`, `Rod`) |
| 100119–100120 | Volume additional (`Gill`, `GillUk`) |
| 100121 | `StandardGravity` |
| 100122 | `MetricHorsepower` |
| 100123 | `Revolution` |
| 100124–100125 | Time additional (`Month`, `Fortnight`) |
| 100126 | `AtmosphereTechnical` |
| 100127–100128 | Textile linear density (`Tex`, `Denier`) |
| 100129–100132 | Apothecary/dry volume (`FluidDram`, `Minim`, `Peck`, `Bushel`) |
| 100133–100145 | Historical German units (`Pfund`, `Zentner`, `Doppelzentner`, `Lot`, Prussian line/zoll/fuss/elle/rute, `Klafter`, `GermanMile`, `Morgen`, `Scheffel`) |
| 100146–100152 | Additional physical units (`SurveyFoot`, `League`, `Cable`, `Hand`, `Quintal`, `Scruple`, `Baud`) |
| 100153–100156 | Temperature scales (`Delisle`, `NewtonTemp`, `Reaumur`, `Romer`) |
| 100157–100162 | Ratio/proportion units (`Percent`, `PerMille`, `PerMyriad`, `PerCentMille`, `Ppm`, `Ppb`) |
| 100163 | `PhScale` |
| 100164–100165 | Named speed units (`MilePerHour`, `KilometerPerHour`) |
| 100166–100170 | Water hardness (`GermanHardness`, `EnglishHardness`, `FrenchHardness`, `RussianHardness`, `AmericanHardness`) |
| 100171–100172 | Concentration (`Val`, `GrainsPerGallon`) |
| 100173–100179 | Turbidity, salinity and conductivity (`TurbidityNtu`, `TurbidityFnu`, `PracticalSalinity`, `ConductivityFactor`, `TurbidityFtu`, `TurbidityFau`, `TurbidityJtu`) |
| 100180–100185 | Temperature *differences* (`DeltaKelvin`, `DeltaFahrenheit`, `DeltaDelisle`, `DeltaNewtonTemp`, `DeltaReaumur`, `DeltaRomer`) — `Δ°C` shares `DeltaKelvin` and `Δ°Ra` shares `DeltaFahrenheit` |
| 100186–100191 | The six units the profiles needed (`MeterWater`, `CalorieIt`, `BtuTh`, `TroyPound`, `ApothecaryDram`, `LongHundredweight`) |
| 100192–100200 | US survey lengths (`SurveyInch` … `SurveyAcre`) |
| 100201–100203 | Typographic lengths (`Point`, `Pica`, `Line`) |
| 100204–100209 | US dry volumes and the trade measures (`DryGallon`, `DryQuart`, `DryPint`, `BoardFoot`, `Cord`, `SurveyAcreFoot`) |
| 100210–100214 | `Darcy`, `ThermEc`, `RefrigerationTon`, `Dobson`, `Shake` |
| 100215–100216 | The ratios below ppb (`Ppt`, `Ppq`) — spelled `pptr` and `ppq` in documents |
| 100217–100225 | Nine units two or three vocabularies define (`PrintersPoint`, `PrintersPica`, `HorsepowerElectric`, `HorsepowerBoiler`, `Abvolt`, `AssayTon`, `BushelUk`, `Clo`, `Debye`) |
| 100226–100236 | Calendar and sidereal time (`YearTropical`, `YearGregorian`, `YearSidereal`, `YearCommon`, `MonthSynodal`, `MonthTropical`, `MonthSidereal`, `DaySidereal`, `HourSidereal`, `MinuteSidereal`, `SecondSidereal`) |
| 100237–100243 | CGS electrostatic (`Statvolt`, `Statampere`, `Statcoulomb`, `Statfarad`, `Statohm`, `Stathenry`, `Statsiemens`) |
| 100244–100246 | Luminance (`Lambert`, `Apostilb`, `Footlambert`) |
| 100247–100249 | Water-vapour permeance (`Perm0c`, `Perm23c`, `PermMetric`) |
| 100250–100255 | Heat conventions (`CalorieMean`, `Calorie15c`, `Calorie20c`, `Btu59f`, `Btu60f`, `BtuMean`) |
| 100256–100261 | `ElementaryCharge`, `Spere`, `CircularMil`, `UnitPole`, `HorsepowerWater`, `MonthGregorian` |
| 100262–100263 | CGS electrostatic magnetics (`Statweber`, `Stattesla`) |
| 900000–900165 | ISO 4217 fiat currencies (`Aed` … `Zwl`), block 90 — starts at `CURRENCY_FIRST` |
| 900166–900215 | Cryptocurrencies (`Btc` … `Rune`), the rest of block 90 — ends at `CURRENCY_LAST` |

There is **no sentinel member** one past the highest id: the id space is blocked
and sparse, so such a number would only invite a bounds check it cannot support.
Use `UNIT_NATIVE_FIRST` / `UNIT_NATIVE_LAST` / `CURRENCY_FIRST` / `CURRENCY_LAST`,
or ask `currency::is_currency`.

> **The profiles' opaque units are in this enum.** The C `value_base_unit_t` has
> a block per profile for the profile-only units, and as shipped two of them have
> members: UCUM's arbitrary atoms (200000–200039, `UcumIu` for `ucum:[IU]`) and
> UN/ECE's package and count codes (300000–300024, `UneceOne` … `UneceVial`; see
> doc/11 §7.1). They have no native spelling, so a unit carrying one renders in
> its profile notation: `unit_to_str` returns `"unece:XBX"`. `BaseUnit::try_from`
> accepts the raw base of a profile-only unit.

> **Note on `Cup_`:** the Cuban-Peso currency is exposed as **`BaseUnit::Cup_`**
> (trailing underscore), not `BaseUnit::Cup`. The plain name `Cup` is the US-cup
> volume unit (enum value 100080); since enum variant names must be unique, the
> currency at value 900033 takes the suffixed name. This affects the variant
> name only — the `.bvnr` wire token is still the bare uppercase `CUP`,
> and case stays load-bearing (`cup` = volume, `CUP` = currency).

---

## 18. Spec 1.1 additions

These bindings target the **Bovnar spec 1.2**; spec 1.0 remains the frozen
baseline, and the crate's own version (2.0.0) is a separate number
(`bovnar::version()` against `bovnar::spec_version()`). Spec 1.2's one addition,
the unit-profile notation, is covered in [§5.1](#51-the-unit-profile-notations-spec-12):
it is gated on a `#!bovnar 1.2` directive, which `to_string()` prepends when a
kept spelling or a profile-only unit needs it. The 1.1 features (all gated on a
`#!bovnar 1.1` directive — an unversioned document is treated as 1.0) are
exposed as:

- **Version:** `bovnar::version()` (library version string),
  `bovnar::spec_version()` → `(major, minor)` of the highest spec understood, and
  `bovnar::peek_version(data)` → the `(major, minor)` a document declares, or
  `None`. `Reader::declared_version()` gives the same after a read;
  `ReadOptions { strict_version: true, .. }` rejects a too-new version
  (`ErrorCode::UnsupportedSpecVersion`). `parse` takes no options; use a
  `Reader` when you need it.
- **Richer escapes:** `\u{…}` and `\xHH` in string literals are decoded by the C
  reader, so `parse` returns the resulting text transparently. A surrogate /
  out-of-range `\u` is `ErrorCode::InvalidCodepoint`.
- **Datetime family:** `parse` decodes a `<datetime:width,epoch>` value to a
  plain `Value::Int` (signed epoch seconds). With `parse_typed` it is a
  `Quantity` whose `epoch_name()` (`"unix"`, `"tai"`, …) and `epoch_mjd()`
  recover the epoch. `to_string()` emits the `<datetime:…>` annotation and
  prepends `#!bovnar 1.1` automatically when the value contains a datetime. With
  `parse_typed` the sub-second fraction of an ISO-8601 literal (spec 1.1) is
  preserved on the `Quantity` (`q.frac`) and re-emitted, so a
  `parse_typed` → `to_string()` round trip is lossless (`…T12:00:00.5Z` survives
  verbatim). The plain (non-typed) value is only the whole-second integer
  carrier, so a non-typed `parse` → `to_string()` drops the fraction; to read it
  explicitly use the DOM tier (`datetime_fraction()`) or the streaming
  reader (`d.frac` in a callback). `ValueTypeFamily::Datetime` is the
  family enum member.
- **Reference array indexing:** `&.matrix[0][1]` paths are stored verbatim and
  resolved by `DomDoc::lookup("matrix[0][1]")` at the DOM layer.

```rust
use bovnar::{Value, parse_typed, peek_version, spec_version, to_string};

fn main() -> Result<(), bovnar::Error> {
    assert_eq!(spec_version(), (1, 2));
    let bvnr = b"#!bovnar 1.1\n.t = <datetime:64,unix> 2026-01-01T12:00:00.5Z;";
    assert_eq!(peek_version(bvnr), Some((1, 1)));

    let doc = parse_typed(bvnr)?;
    let Value::Struct(fields) = &doc else { unreachable!() };
    let Value::Quantity(q) = &fields[0].1 else { unreachable!() };
    assert_eq!(q.epoch_name().as_deref(), Some("unix"));
    assert_eq!(q.frac.as_deref(), Some("5"));
    // the fraction survives a typed round trip
    assert_eq!(parse_typed(&to_string(&doc, true)?)?, doc);
    Ok(())
}
```

`ErrorCode` adds `InvalidSpecVersion` (42), `UnsupportedSpecVersion` (43),
`InvalidCodepoint` (44), `InvalidDatetimeLiteral` (45), and
`DatetimeLiteralUnsupportedEpoch` (46).

---

## 19. License

MIT, like the C library — see
[LICENSE](https://github.com/sothis/bovnar/blob/main/LICENSE).

The crate compiles `libbvnr` into your binary, and that embeds Bovnar's
unit-profile tables, which carry identifier strings from UCUM, QUDT, OM 2,
UDUNITS-2, the CF standard name table and UN/ECE Recommendations 20 and 21.
Those identifiers belong to their publishers and the MIT grant does not extend
to them; the notices are at
[`THIRD_PARTY_NOTICES.md`](https://github.com/sothis/bovnar/blob/main/THIRD_PARTY_NOTICES.md).

---

## See also

- [Read & Write API](08_bovnar_readwrite_api.md) — the C reader, writer, and DOM these bindings wrap
- [Streaming, Framing & Multiplexing](10_bovnar_streaming.md) — the framing, multiplexing and embedding behind `bovnar::stream`
- [Specification](03_bovnar_spec.md) — the format the crate reads and writes
- [Unit & Currency Reference](05_bovnar_unit_system.md) — the unit model behind `Unit` and `Quantity`
- [Tutorial](01_bovnar_tutorial.md) — the document syntax, by example

---

*End of Bovnar — Rust Bindings (Bovnar spec 1.2).*
