//! Properties checked by the fuzz targets.
//!
//! This file is shared: the fuzz targets call it through the `bovnar-fuzz`
//! crate, and `tests/fuzz_regressions.rs` includes it with `#[path]` to
//! replay `fuzz/regressions/` under plain `cargo test`. It therefore only
//! depends on `bovnar`, `std` and `serde_json` (a dev-dependency of the
//! crate).
//!
//! Every check panics when a property fails, which is what makes libFuzzer
//! report the input.

use bovnar::units::{parse_unit, AsciiExponents};
use bovnar::{
    parse, parse_with, to_vec, to_vec_compact, Document, Error, ErrorKind, Event, Limits, Reader,
    SpecVersion, TokenKind, TypeAnnotation, TypeFamily, TypeSpec, Writer,
};

/// Splits off the first byte of the input as a control byte.
fn split_control(data: &[u8]) -> (u8, &[u8]) {
    match data.split_first() {
        Some((&first, rest)) => (first, rest),
        None => (0, data),
    }
}

/// Small limits derived from control bytes, so the fuzzer reaches every
/// limit error quickly. A zero byte keeps the default for that limit.
fn tight_limits(control: &[u8]) -> Limits {
    let byte = |i: usize| control.get(i).copied().unwrap_or(0);
    let pick16 = |b: u8, default: u16| {
        if b == 0 {
            default
        } else {
            u16::from(b % 64) + 1
        }
    };
    let pick8 = |b: u8, default: u8| if b == 0 { default } else { b % 16 + 1 };
    let mut limits = Limits::default();
    limits.max_identifier_length = pick16(byte(0), limits.max_identifier_length);
    limits.max_string_length = pick16(byte(1), limits.max_string_length);
    limits.max_number_length = pick16(byte(2), limits.max_number_length);
    limits.max_symbol_length = pick16(byte(3), limits.max_symbol_length);
    limits.max_reference_length = pick16(byte(4), limits.max_reference_length);
    limits.max_struct_nesting = pick8(byte(5), limits.max_struct_nesting);
    limits.max_array_nesting = pick8(byte(6), limits.max_array_nesting);
    if byte(7) != 0 {
        limits.max_array_items = u64::from(byte(7) % 32) + 1;
    }
    if byte(8) != 0 {
        limits.max_input_bytes = u64::from(byte(8)) * 16;
    }
    if byte(9) != 0 {
        limits.max_dom_nodes = u64::from(byte(9) % 64) + 1;
    }
    limits
}

/// An owned, comparable copy of one event.
fn describe(event: &Event<'_>) -> String {
    format!("{event:?}")
}

/// Runs the reader over `input` cut at the given chunk lengths (cycled) and
/// returns the event log plus the final result.
fn run_reader(input: &[u8], limits: &Limits, cuts: &[u8]) -> (Vec<String>, Result<(), Error>) {
    let mut reader = Reader::new(limits);
    let mut log = Vec::new();
    let mut rest = input;
    let mut i = 0;
    while !rest.is_empty() {
        let len = match cuts.get(i % cuts.len().max(1)) {
            Some(&c) if c != 0 => usize::from(c),
            _ => rest.len(),
        };
        i += 1;
        let (chunk, tail) = rest.split_at(len.min(rest.len()));
        rest = tail;
        let result = reader.feed(chunk);
        while let Some(event) = reader.next_event() {
            log.push(describe(&event));
        }
        if result.is_err() {
            return (log, result);
        }
    }
    let result = reader.finish();
    while let Some(event) = reader.next_event() {
        log.push(describe(&event));
    }
    // A failed reader must keep failing with the same error.
    if let Err(err) = &result {
        assert_eq!(
            reader.finish().err().as_ref(),
            Some(err),
            "error is not sticky"
        );
    }
    (log, result)
}

/// The streaming reader: any chunking yields the same events and the same
/// result as feeding the input in one piece.
///
/// Input layout: 10 limit bytes, 1 length byte `n`, `n` chunk lengths, then
/// the document.
pub fn reader(data: &[u8]) {
    let (limit_bytes, rest) = data.split_at(data.len().min(10));
    let limits = tight_limits(limit_bytes);
    let (n, rest) = split_control(rest);
    let (cuts, input) = rest.split_at(rest.len().min(usize::from(n % 16)));

    let whole = run_reader(input, &limits, &[]);
    let chunked = run_reader(input, &limits, cuts);
    assert_eq!(whole.1, chunked.1, "result depends on chunking");
    assert_eq!(whole.0, chunked.0, "events depend on chunking");

    let bytewise = run_reader(input, &limits, &[1]);
    assert_eq!(whole.1, bytewise.1, "result differs when fed byte by byte");
    assert_eq!(whole.0, bytewise.0, "events differ when fed byte by byte");
}

/// C (and so the reader) accepts an annotation whose unit nests more than
/// 16 groups without checking the rest of the annotation, so the tree can
/// hold a type the writer refuses (UPSTREAM_NOTES.md). Such an input has at
/// least 17 brackets or braces.
fn c_deep_unit_quirk(input: &[u8], err: &Error) -> bool {
    let count = |c: u8| input.iter().filter(|&&b| b == c).count();
    err.kind() == ErrorKind::IllegalValueType && (count(b'[') > 16 || count(b'{') > 16)
}

/// The document tree under tight limits: never panics, and a document the
/// tree accepts is one the streaming reader accepts too, under the same
/// limits.
pub fn dom(data: &[u8]) {
    let (limit_bytes, input) = data.split_at(data.len().min(10));
    let limits = tight_limits(limit_bytes);
    let Ok(doc) = parse_with(input, &limits) else {
        return;
    };
    let (_, result) = run_reader(input, &limits, &[]);
    assert_eq!(result, Ok(()), "tree accepted what the reader rejects");
    // Writing a tree read under any limits must succeed.
    if let Err(e) = to_vec(&doc) {
        assert!(
            c_deep_unit_quirk(input, &e),
            "writing a parsed tree failed: {e}"
        );
    }
}

/// parse → write → parse yields an equal tree, pretty and compact, and
/// writing is a fixed point after one round.
pub fn roundtrip(input: &[u8]) {
    let Ok(doc) = parse(input) else {
        return;
    };
    // `to_vec`, not `to_string`: octet streams make the output binary.
    type WriteFn = fn(&Document) -> Result<Vec<u8>, Error>;
    for (name, write) in [("pretty", to_vec as WriteFn), ("compact", to_vec_compact)] {
        let text = match write(&doc) {
            Ok(text) => text,
            Err(e) if c_deep_unit_quirk(input, &e) => return,
            Err(e) => panic!("{name}: writing failed: {e}"),
        };
        let shown = String::from_utf8_lossy(&text);
        let again = parse(&text).unwrap_or_else(|e| panic!("{name}: reparse failed: {e}\n{shown}"));
        assert_eq!(doc, again, "{name}: tree changed\n{shown}");
        let text2 = write(&again).unwrap_or_else(|e| panic!("{name}: rewrite failed: {e}"));
        assert_eq!(text, text2, "{name}: output is not a fixed point");
    }
}

/// Cursor over the fuzz input that hands out small values and slices.
struct Bytes<'a>(&'a [u8]);

impl<'a> Bytes<'a> {
    fn byte(&mut self) -> Option<u8> {
        let (&b, rest) = self.0.split_first()?;
        self.0 = rest;
        Some(b)
    }

    fn slice(&mut self) -> &'a [u8] {
        let len = usize::from(self.byte().unwrap_or(0) % 24).min(self.0.len());
        let (head, rest) = self.0.split_at(len);
        self.0 = rest;
        head
    }

    fn opt_u32(&mut self) -> Option<u32> {
        match self.byte()? {
            0..=127 => None,
            b => Some(u32::from(b % 70)),
        }
    }

    /// A slice, often replaced by a token the writer is likely to accept.
    fn text(&mut self, pool: &[&'static [u8]]) -> &'a [u8] {
        match self.byte() {
            Some(b) if usize::from(b) < pool.len() * 4 => pool[usize::from(b) % pool.len()],
            _ => self.slice(),
        }
    }
}

const TEXTS: &[&[u8]] = &[
    b"0",
    b"1",
    b"-1",
    b"42",
    b"1.5",
    b"-0.0",
    b"1e3",
    b"0x1F",
    b"0b101",
    b"0o17",
    b"inf",
    b"-inf",
    b"nan",
    b"true",
    b"false",
    b"null",
    b"abc",
    b"",
    b"a\"b\\c\n",
    b"18446744073709551615",
    b"-9223372036854775808",
    b"2026-06-15T12:00:00Z",
    b"1750000000",
    b"sym_x",
    b"a.b[0]",
    b"\xc3\xa9",
    b"\x00",
    b"1/3",
];
const KEYS: &[&[u8]] = &[b"a", b"key", b"_x", b"k1", b"", b"1a", b"a-b", b"\xff"];
const UNITS: &[&[u8]] = &[
    b"m",
    b"k~g",
    b"m/s^2",
    b"no_unit",
    b"unece:KGM",
    b"qudt:M",
    b"om:metre",
    b"ucum:m",
    b"cf:m",
    b"%",
    b"",
    b"bogus",
];
const KINDS: [TokenKind; 9] = [
    TokenKind::Number,
    TokenKind::String,
    TokenKind::Symbol,
    TokenKind::Reference,
    TokenKind::Null,
    TokenKind::Bool,
    TokenKind::ArrayNumber,
    TokenKind::Identifier,
    TokenKind::Unknown,
];
const FAMILIES: [TypeFamily; 9] = [
    TypeFamily::Plain,
    TypeFamily::Sint,
    TypeFamily::Uint,
    TypeFamily::Float,
    TypeFamily::FloatFix,
    TypeFamily::FloatDec,
    TypeFamily::Bool,
    TypeFamily::Utf8,
    TypeFamily::Datetime,
];
const EPOCHS: [Option<&str>; 4] = [None, Some("unix"), Some("tai"), Some("gps")];

fn next_event<'a>(bytes: &mut Bytes<'a>) -> Option<Event<'a>> {
    let op = bytes.byte()?;
    Some(match op % 14 {
        0 => Event::StreamStart,
        1 => Event::AssignmentStart {
            key: bytes.text(KEYS),
        },
        2 => {
            let b = bytes.byte().unwrap_or(0);
            Event::TypeAnnotation(TypeAnnotation {
                text: bytes.slice(),
                family: FAMILIES[usize::from(b) % FAMILIES.len()],
                width: bytes.opt_u32(),
                base: bytes.opt_u32(),
                q: bytes.opt_u32(),
                unit: (b & 0x80 != 0).then(|| bytes.text(UNITS)),
                epoch: EPOCHS[usize::from(b >> 4) % EPOCHS.len()],
                synthesized: b & 0x40 != 0,
            })
        }
        3..=5 => {
            let b = bytes.byte().unwrap_or(0);
            Event::Data {
                kind: KINDS[usize::from(b) % KINDS.len()],
                text: bytes.text(TEXTS),
                frac: if b & 0x80 != 0 { bytes.slice() } else { b"" },
                value_type: TypeSpec::PLAIN,
                unit: b"",
                inline_unit: if b & 0x40 != 0 {
                    bytes.text(UNITS)
                } else {
                    b""
                },
            }
        }
        6 => Event::StructStart,
        7 => Event::StructEnd,
        8 => Event::ArrayRowStart,
        9 => Event::ArrayRowEnd,
        10 => Event::ArrayDimStart,
        11 => Event::OctetStreamStart,
        12 => Event::OctetStreamChunk(bytes.slice()),
        _ => Event::OctetStreamEnd,
    })
}

/// Arbitrary event sequences through the writer never panic, and any
/// document the writer finishes reads back.
///
/// Input layout: 1 control byte (pretty, version, limits), then events.
pub fn writer(data: &[u8]) {
    let (control, rest) = split_control(data);
    let pretty = control & 1 != 0;
    let mut out = Vec::new();
    let mut writer = if control & 2 != 0 {
        Writer::with_limits(
            &mut out,
            pretty,
            &tight_limits(rest.get(..10).unwrap_or(&[])),
        )
    } else {
        Writer::new(&mut out, pretty)
    };
    let version = [
        None,
        Some(SpecVersion::V1_0),
        Some(SpecVersion::V1_1),
        Some(SpecVersion::V1_2),
    ][usize::from(control >> 2) % 4];
    if let Some(version) = version {
        if writer.write_version(version).is_err() {
            return;
        }
    }
    let mut bytes = Bytes(rest);
    let mut ok = true;
    while let Some(event) = next_event(&mut bytes) {
        if writer.write_event(&event).is_err() {
            ok = false;
            break;
        }
    }
    if !ok || writer.finish().is_err() {
        return;
    }
    drop(writer);
    let shown = String::from_utf8_lossy(&out);
    let (_, result) = run_reader(&out, &Limits::default(), &[]);
    assert_eq!(result, Ok(()), "writer output does not read back\n{shown}");
    // The writer streams: like the C writer it leaves the document-tier
    // rules (duplicate keys, homogeneity, struct shape) to the tree, so only
    // a document the tree accepts must also survive the tree round trip.
    if let Ok(doc) = parse(&out) {
        let text = match to_vec(&doc) {
            Ok(text) => text,
            Err(e) if c_deep_unit_quirk(&out, &e) => return,
            Err(e) => panic!("writing the reparsed tree failed: {e}\n{shown}"),
        };
        assert_eq!(
            parse(&text).ok().as_ref(),
            Some(&doc),
            "round trip changed the tree\n{shown}"
        );
    }
}

/// Unit expressions: a parsed unit prints, both canonically and with ASCII
/// exponents, to text that parses back to an equal unit and prints the same.
///
/// Input layout: 1 version byte, then the unit text.
pub fn units(data: &[u8]) {
    let (control, text) = split_control(data);
    let version =
        [SpecVersion::V1_0, SpecVersion::V1_1, SpecVersion::V1_2][usize::from(control) % 3];
    let Ok(unit) = parse_unit(text, version) else {
        return;
    };
    // Profile codes print in the native spelling, which needs spec 1.2 only
    // for its namespace; native spellings parse under every version.
    for printed in [unit.to_string(), AsciiExponents(&unit).to_string()] {
        let again = parse_unit(printed.as_bytes(), SpecVersion::V1_2).unwrap_or_else(|e| {
            panic!(
                "{:?} printed as {printed:?}, which does not parse: {e}",
                String::from_utf8_lossy(text)
            )
        });
        assert!(
            again == unit,
            "{:?} printed as {printed:?} reparses to {again}",
            String::from_utf8_lossy(text)
        );
        assert_eq!(
            again.to_string(),
            unit.to_string(),
            "printing is not stable"
        );
    }
}

/// The limits `bovnar convert` runs with (the default 16 MiB input cap is
/// kept; fuzz inputs are far smaller).
fn convert_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_struct_nesting = 255;
    limits.max_array_nesting = 255;
    limits
}

/// Two JSON values are the same per `bovnar::json`'s round-trip rules:
/// same keys in the same order, same strings, booleans and nulls, integers
/// equal, other numbers equal as 64-bit floats.
fn same_json(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    use serde_json::Value as J;
    let mut stack = vec![(a, b)];
    while let Some((a, b)) = stack.pop() {
        match (a, b) {
            (J::Object(x), J::Object(y)) => {
                if x.len() != y.len() || !x.keys().eq(y.keys()) {
                    return false;
                }
                stack.extend(x.values().zip(y.values()));
            }
            (J::Array(x), J::Array(y)) => {
                if x.len() != y.len() {
                    return false;
                }
                stack.extend(x.iter().zip(y.iter()));
            }
            (J::Number(x), J::Number(y)) => {
                let ints = x.as_i64().zip(y.as_i64()).map(|(p, q)| p == q);
                let uints = x.as_u64().zip(y.as_u64()).map(|(p, q)| p == q);
                let same = ints.or(uints).unwrap_or_else(|| x.as_f64() == y.as_f64());
                if !same {
                    return false;
                }
            }
            (x, y) => {
                if x != y {
                    return false;
                }
            }
        }
    }
    true
}

/// JSON → Bovnar → JSON: never panics; whatever converts reads back as a
/// tree, converts to JSON without loss, and that JSON has the input's value.
/// Any Bovnar input that converts to JSON yields valid JSON.
pub fn json(data: &[u8]) {
    use bovnar::json::{from_bovnar, from_document, to_bovnar, to_document};

    let limits = convert_limits();
    if let Ok(out) = from_bovnar(data, &limits) {
        if let Err(e) = serde_json::from_str::<serde_json::Value>(&out.json) {
            assert!(
                e.to_string().contains("recursion limit"),
                "bvnr -> json printed invalid JSON: {e}"
            );
        }
    }

    let text = to_bovnar(data, &limits);
    let doc = to_document(data, &limits);
    assert_eq!(
        text.is_ok(),
        doc.is_ok(),
        "to_bovnar and to_document disagree"
    );
    let (Ok(text), Ok(doc)) = (text, doc) else {
        return;
    };
    assert_eq!(
        parse_with(text.as_bytes(), &limits).as_ref(),
        Ok(&doc),
        "text and tree differ"
    );
    let out = from_document(&doc).expect("a tree from JSON converts back");
    assert!(out.loss.is_empty(), "JSON-born tree lost {:?}", out.loss);
    let back: serde_json::Value = match serde_json::from_str(&out.json) {
        Ok(v) => v,
        Err(e) if e.to_string().contains("recursion limit") => return,
        Err(e) => panic!("json -> bvnr -> json is not JSON: {e}"),
    };
    // The reference's reader accepts a few inputs a strict JSON reader does
    // not (truncated containers, VT and FF as whitespace); compare the rest.
    if let Ok(input) = serde_json::from_slice::<serde_json::Value>(data) {
        assert!(
            same_json(&input, &back),
            "round trip changed the value: {input} -> {back}"
        );
    }
}

/// Format sniffing never panics and never contradicts a parser: input that
/// converts from JSON is detected as JSON, a Bovnar document with content
/// as Bovnar.
pub fn detect(data: &[u8]) {
    use bovnar::{detect, Format};

    let format = detect(data);
    if bovnar::json::to_bovnar(data, &convert_limits()).is_ok() {
        assert_eq!(format, Format::Json, "converting JSON not detected as JSON");
    }
    let blank = data
        .strip_prefix(b"\xEF\xBB\xBF")
        .unwrap_or(data)
        .iter()
        .all(|b| b" \t\n\r\x0b\x0c".contains(b));
    if !blank && parse(data).is_ok() {
        assert_eq!(
            format,
            Format::Bovnar,
            "Bovnar document not detected as Bovnar"
        );
    }
}
