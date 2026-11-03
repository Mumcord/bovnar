//! JSON <-> BVNR conversion, mirroring the upstream CLI's `bovnar convert`.
//!
//! [`from_json`] refuses what bovnar cannot represent (invalid keys, integers
//! outside 64 bits, non-finite floats, NUL and control characters, duplicate
//! keys, heterogeneous arrays) instead of writing a lossy document.
//! [`to_json`] renders what JSON cannot carry the way the CLI does (units
//! dropped, octets as hex, datetimes as epoch seconds, symbols and references
//! as strings, integers wider than 64 bits as strings, NaN and infinities as
//! `null`, a float with an integral value as an integer: `1e+6` is
//! `1000000`, which reads back as `uint`, and `-0.0` is `-0`, which reads
//! back as `0`) and only refuses a datetime with
//! sub-second digits. Unlike the CLI it does not report those losses.

use crate::enums::{Event, TokenType, ValueTypeFamily};
use crate::error::{Error, Result};
use crate::writer::{self, Base, EmitSpec, Writer};
use crate::{DomDoc, DomNode, DomType, Unit, ffi};

/// The deepest JSON nesting converted: the writer's and the reader's 255-level
/// struct and array cap, so a document deeper than any reader takes is refused
/// as JSON rather than by the writer half-way through. (The CLI's
/// `JSON_MAX_DEPTH` is 512.)
const MAX_DEPTH: usize = 255;

/// What [`detect`] takes a buffer for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Json,
    Bovnar,
    Unknown,
}

/// Sniffs the format from the first significant byte. The CLI goes by file
/// extension; this is the content-based fallback for data without a name.
pub fn detect(data: &[u8]) -> Format {
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let start = data.iter().position(|c| !c.is_ascii_whitespace());
    let rest = &data[start.unwrap_or(data.len())..];
    match rest.first() {
        Some(b'{' | b'[') => Format::Json,
        Some(b'.') => Format::Bovnar,
        _ if rest.starts_with(b"#!bovnar") => Format::Bovnar,
        _ => Format::Unknown,
    }
}

enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

/// Converts JSON text to a BVNR document. The root must be an object.
pub fn from_json(json: &str) -> Result<Vec<u8>> {
    let mut p = Parser {
        s: json.as_bytes(),
        i: 0,
        key: None,
    };
    let root = p.value(0)?;
    p.skip_ws();
    if p.i != p.s.len() {
        return Err(refuse("trailing content after JSON value".into()));
    }
    let Json::Object(members) = root else {
        return Err(refuse("JSON root must be an object".into()));
    };
    let out = writer::grow_write(json.len() * 2, true, |w| {
        members.iter().try_for_each(|(k, v)| write_value(w, k, v))
    })?;
    // Like the CLI, the library's own document checks decide what has no
    // bovnar form: duplicate keys, mixed or ragged arrays.
    if let Err(e) = DomDoc::parse(&out) {
        let hint = match e.name() {
            Some("duplicate_struct_key") => "a JSON object with a duplicate key",
            Some("struct_shape_mismatch") => "an array of objects with differing key sets",
            _ => "a heterogeneous or ragged array",
        };
        let name = e.name().unwrap_or("parse error");
        return Err(refuse(format!(
            "the JSON has no bovnar representation: {name} ({hint})"
        )));
    }
    Ok(out)
}

fn refuse(msg: String) -> Error {
    Error::argument(format!("convert: {msg}"))
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    /// The object key being parsed, for messages only.
    key: Option<String>,
}

impl Parser<'_> {
    fn err(&self, what: &str) -> Error {
        let key = self.key.as_deref().unwrap_or("(top level)");
        refuse(format!("{what} under key '{key}' at byte {}", self.i))
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    /// C `isspace`, as the CLI skips it.
    fn skip_ws(&mut self) {
        while matches!(
            self.peek(),
            Some(b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
        ) {
            self.i += 1;
        }
    }

    fn eat(&mut self, lit: &[u8]) -> bool {
        let hit = self.s[self.i..].starts_with(lit);
        if hit {
            self.i += lit.len();
        }
        hit
    }

    fn value(&mut self, depth: usize) -> Result<Json> {
        self.skip_ws();
        // `depth` counts the containers around this value, the root object
        // not among them (it is the document, not a struct): a container here
        // nests `depth` deep, a scalar nests nothing.
        if depth > MAX_DEPTH && matches!(self.peek(), Some(b'[' | b'{')) {
            return Err(self.err(&format!("JSON nesting exceeds limit ({MAX_DEPTH})")));
        }
        match self.peek() {
            Some(b'"') => self.string().map(Json::Str),
            Some(b'[') => self.array(depth + 1),
            Some(b'{') => self.object(depth + 1),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ if self.eat(b"null") => Ok(Json::Null),
            _ if self.eat(b"true") => Ok(Json::Bool(true)),
            _ if self.eat(b"false") => Ok(Json::Bool(false)),
            _ => Err(self.err("malformed JSON")),
        }
    }

    /// After the opening bracket or brace: `,`-separated items up to `close`.
    fn items(&mut self, close: u8, mut item: impl FnMut(&mut Self) -> Result<()>) -> Result<()> {
        self.i += 1;
        self.skip_ws();
        if self.eat(&[close]) {
            return Ok(());
        }
        loop {
            item(self)?;
            self.skip_ws();
            if self.eat(b",") {
                continue;
            }
            if self.eat(&[close]) {
                return Ok(());
            }
            return Err(self.err("malformed JSON container"));
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json> {
        let mut out = Vec::new();
        self.items(b']', |p| p.value(depth).map(|v| out.push(v)))?;
        Ok(Json::Array(out))
    }

    fn object(&mut self, depth: usize) -> Result<Json> {
        let mut out = Vec::new();
        self.items(b'}', |p| {
            p.skip_ws();
            if p.peek() != Some(b'"') {
                return Err(p.err("expected a JSON object key"));
            }
            let key = p.string()?;
            p.skip_ws();
            if !p.eat(b":") {
                return Err(p.err("expected ':' after a JSON object key"));
            }
            let outer = p.key.replace(key.clone());
            let v = p.value(depth);
            p.key = outer;
            out.push((key, v?));
            Ok(())
        })?;
        Ok(Json::Object(out))
    }

    fn hex4(&mut self) -> Result<u32> {
        let digits = self
            .s
            .get(self.i..self.i + 4)
            .and_then(|h| std::str::from_utf8(h).ok());
        let v = digits.and_then(|h| u32::from_str_radix(h, 16).ok());
        self.i += 4;
        v.filter(|_| digits.is_some_and(|h| h.bytes().all(|c| c.is_ascii_hexdigit())))
            .ok_or_else(|| self.err("bad \\u escape in JSON string"))
    }

    fn string(&mut self) -> Result<String> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let start = self.i;
            while !matches!(self.peek(), None | Some(b'"' | b'\\' | 0..=0x1F)) {
                self.i += 1;
            }
            // The input is a &str and the stop bytes are ASCII: a valid slice.
            out.push_str(std::str::from_utf8(&self.s[start..self.i]).unwrap_or_default());
            let Some(c) = self.peek() else {
                return Err(self.err("unterminated JSON string"));
            };
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {}
                _ => return Err(self.err("unescaped control character in JSON string")),
            }
            let esc = self.peek();
            self.i += 1;
            out.push(match esc {
                Some(b'"') => '"',
                Some(b'\\') => '\\',
                Some(b'/') => '/',
                Some(b'b') => '\u{8}',
                Some(b'f') => '\u{c}',
                Some(b'n') => '\n',
                Some(b'r') => '\r',
                Some(b't') => '\t',
                Some(b'u') => self.code_point()?,
                _ => return Err(self.err("bad escape in JSON string")),
            });
        }
    }

    fn code_point(&mut self) -> Result<char> {
        let mut cp = self.hex4()?;
        if (0xD800..0xDC00).contains(&cp) {
            let lo = if self.eat(b"\\u") { self.hex4()? } else { 0 };
            if !(0xDC00..0xE000).contains(&lo) {
                return Err(self.err("unpaired surrogate in JSON string"));
            }
            cp = 0x10000 + ((cp - 0xD800) << 10 | (lo - 0xDC00));
        }
        if cp == 0 {
            return Err(
                self.err("JSON string contains a NUL (\\u0000), which bovnar cannot represent")
            );
        }
        char::from_u32(cp).ok_or_else(|| self.err("unpaired surrogate in JSON string"))
    }

    /// RFC 8259: `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE] [+-]? [0-9]+)?`.
    fn number(&mut self) -> Result<Json> {
        let start = self.i;
        let digits = |p: &mut Self| {
            let from = p.i;
            while p.peek().is_some_and(|c| c.is_ascii_digit()) {
                p.i += 1;
            }
            p.i > from
        };
        self.eat(b"-");
        let mut ok = self.eat(b"0") || digits(self);
        let mut float = false;
        if self.eat(b".") {
            float = true;
            ok &= digits(self);
        }
        if self.eat(b"e") || self.eat(b"E") {
            float = true;
            let _ = self.eat(b"+") || self.eat(b"-");
            ok &= digits(self);
        }
        if !ok {
            return Err(self.err("malformed JSON number"));
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).unwrap_or_default();
        if float {
            // Only overflow is refused: underflow and excess digits are the
            // rounding any decimal-to-double read does.
            return text
                .parse::<f64>()
                .ok()
                .filter(|f| f.is_finite())
                .map(Json::Float)
                .ok_or_else(|| self.err("JSON number too large for a 64-bit float"));
        }
        if text.starts_with('-') {
            text.parse()
                .map(Json::Int)
                .map_err(|_| self.err("integer out of range for 64-bit signed"))
        } else {
            text.parse()
                .map(Json::Uint)
                .map_err(|_| self.err("integer out of range for 64-bit unsigned"))
        }
    }
}

/// A bovnar identifier: a letter, `_` or UTF-8 lead byte first, then letters,
/// digits, `_`, `+`, `-` or UTF-8 bytes; the lead byte 0xC2 never.
fn key_is_valid(key: &str) -> bool {
    let b = key.as_bytes();
    let start_ok = |c: u8| c.is_ascii_alphabetic() || c == b'_' || (0xC3..=0xF4).contains(&c);
    let body_ok = |c: u8| {
        c.is_ascii_alphanumeric() || matches!(c, b'_' | b'+' | b'-') || (c >= 0x80 && c != 0xC2)
    };
    b.first().is_some_and(|&c| start_ok(c)) && b[1..].iter().all(|&c| body_ok(c))
}

/// Refuses the C0 controls a bovnar string cannot carry (all but `\t` to
/// `\r`) and DEL.
fn check_string(s: &str) -> Result<()> {
    match s
        .bytes()
        .find(|&c| c <= 0x08 || (0x0E..=0x1F).contains(&c) || c == 0x7F)
    {
        Some(c) => Err(refuse(format!(
            "the JSON string contains the control character U+{c:04X}, which a bovnar string cannot carry"
        ))),
        None => Ok(()),
    }
}

fn write_value(w: &mut Writer, key: &str, v: &Json) -> Result<()> {
    if !key_is_valid(key) {
        return Err(refuse(format!(
            "JSON key '{key}' is not a valid bovnar identifier (must start with a letter or '_' \
             and contain only letters, digits, '_', '+', '-')"
        )));
    }
    match v {
        Json::Null => w.write_null(key),
        Json::Bool(b) => w.write_bool(key, *b),
        Json::Uint(u) => w.write_uint(key, *u, 0, Base::DECIMAL),
        Json::Int(i) => w.write_sint(key, *i, 0, Base::DECIMAL),
        Json::Float(f) => w.write_float(key, *f, 0),
        Json::Str(s) => check_string(s).and_then(|_| w.write_string(key, s)),
        Json::Array(items) => {
            w.assignment(key)?;
            write_array_body(w, items)
        }
        Json::Object(members) => {
            w.begin_struct(key)?;
            members.iter().try_for_each(|(k, v)| write_value(w, k, v))?;
            w.end_struct()
        }
    }
}

/// Like the CLI, a JSON array is one bracket row with nested arrays as nested
/// rows (never `/`-separated), so a single-row matrix keeps its nesting.
fn write_array_body(w: &mut Writer, items: &[Json]) -> Result<()> {
    w.begin_array_row()?;
    items.iter().try_for_each(|e| write_element(w, e))?;
    w.end_array_row()
}

fn data(w: &mut Writer, token: TokenType, vt: ffi::ValueTypeSpec, text: &str) -> Result<()> {
    w.data_event(
        Event::Data,
        token,
        vt,
        &Unit::default(),
        [text.as_bytes(), b"", b""],
    )
}

fn write_element(w: &mut Writer, e: &Json) -> Result<()> {
    let plain = ffi::ValueTypeSpec::default();
    match e {
        Json::Null => data(w, TokenType::NullValue, plain, ""),
        Json::Bool(b) => data(w, TokenType::Bool, plain, &b.to_string()),
        Json::Uint(u) => data(w, TokenType::ArrayNumber, plain, &u.to_string()),
        Json::Int(i) => data(w, TokenType::ArrayNumber, plain, &i.to_string()),
        Json::Float(f) => {
            let vt = writer::type_spec(ValueTypeFamily::Float, 64, 0);
            let text = ffi::format_f64(*f, vt)
                .ok_or_else(|| Error::argument(format!("cannot format {f}")))?;
            // "2" would read back as a uint inside an array, unlike at the
            // top level: annotate it as a float.
            if !text.contains(['.', 'e', 'E']) {
                w.annotation(vt, &Unit::default(), None)?;
            }
            data(w, TokenType::ArrayNumber, vt, &text)
        }
        Json::Str(s) => check_string(s).and_then(|_| data(w, TokenType::ArrayString, plain, s)),
        Json::Array(items) => write_array_body(w, items),
        Json::Object(members) => {
            w.emit(Event::StructStart, EmitSpec::NONE)?;
            members.iter().try_for_each(|(k, v)| write_value(w, k, v))?;
            w.emit(Event::StructEnd, EmitSpec::NONE)
        }
    }
}

/// Converts a BVNR document to pretty-printed JSON, as `bovnar convert` prints it.
pub fn to_json(data: &[u8]) -> Result<String> {
    let doc = DomDoc::parse(data)?;
    let entries = doc.entries();
    for (key, node) in &entries {
        if has_fractional_datetime(*node) {
            return Err(refuse(format!(
                "a datetime with sub-second digits cannot be represented in JSON without \
                 dropping the fraction (under top-level key '{key}')"
            )));
        }
    }
    let mut out = String::new();
    push_struct(&mut out, &entries, 0);
    out.push('\n');
    Ok(out)
}

fn has_fractional_datetime(n: DomNode<'_>) -> bool {
    match n.dom_type() {
        Some(DomType::Int) => n.datetime_fraction().is_some(),
        Some(DomType::Struct) => n
            .entries()
            .into_iter()
            .any(|(_, v)| has_fractional_datetime(v)),
        Some(DomType::Array) => (0..n.len())
            .filter_map(|i| n.at(i))
            .any(has_fractional_datetime),
        _ => false,
    }
}

/// The CLI's `print_json_node` with `pretty`: structs indented, arrays inline.
fn push_node(out: &mut String, n: DomNode<'_>, indent: usize) {
    match n.dom_type() {
        Some(DomType::Int) => {
            let vt = n.value_type();
            if vt.width > 64 {
                match n.as_int_str(10) {
                    Some(s) => out.push_str(&format!("\"{s}\"")),
                    None => out.push_str("null"),
                }
            } else if ValueTypeFamily::from_raw(vt.family) == Some(ValueTypeFamily::Uint) {
                out.push_str(&n.as_u64().unwrap_or_default().to_string());
            } else {
                out.push_str(&n.as_i64().unwrap_or_default().to_string());
            }
        }
        Some(DomType::Float) => out.push_str(&format_double(n.as_float().unwrap_or_default())),
        Some(DomType::Bool) => out.push_str(&n.as_bool().unwrap_or_default().to_string()),
        Some(DomType::String | DomType::Symbol | DomType::Reference) => match n.as_str() {
            Some(s) => push_escaped(out, s),
            None => out.push_str("null"),
        },
        Some(DomType::OctetStream) => {
            out.push('"');
            for b in n.as_bytes().unwrap_or_default() {
                out.push_str(&format!("{b:02x}"));
            }
            out.push('"');
        }
        Some(DomType::Struct) => push_struct(out, &n.entries(), indent),
        Some(DomType::Array) => {
            let (cnt, dims) = (n.len(), n.array_dims());
            let elems = |out: &mut String, range: std::ops::Range<usize>| {
                out.push('[');
                for i in range.clone() {
                    if i > range.start {
                        out.push_str(", ");
                    }
                    match n.at(i) {
                        Some(e) => push_node(out, e, indent),
                        None => out.push_str("null"),
                    }
                }
                out.push(']');
            };
            // A "/"-separated array becomes one JSON array per row.
            if dims > 1 && cnt.is_multiple_of(dims) {
                let row = cnt / dims;
                out.push('[');
                for r in 0..dims {
                    if r > 0 {
                        out.push_str(", ");
                    }
                    elems(out, r * row..(r + 1) * row);
                }
                out.push(']');
            } else {
                elems(out, 0..cnt);
            }
        }
        Some(DomType::Null) | None => out.push_str("null"),
    }
}

/// One member per line; the document itself is the struct at indent 0.
fn push_struct(out: &mut String, entries: &[(String, DomNode<'_>)], indent: usize) {
    out.push('{');
    for (i, (key, v)) in entries.iter().enumerate() {
        out.push_str(if i > 0 { ",\n" } else { "\n" });
        out.push_str(&"  ".repeat(indent + 1));
        push_escaped(out, key);
        out.push_str(": ");
        push_node(out, *v, indent + 1);
    }
    if !entries.is_empty() {
        out.push('\n');
        out.push_str(&"  ".repeat(indent));
    }
    out.push('}');
}

fn push_escaped(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0'..'\u{20}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// The CLI's `bvn_cli_format_double`: shortest round-trip digits, plain
/// notation for decimal exponents -4..17, `%g`-style scientific otherwise;
/// NaN and infinities are `null`.
fn format_double(v: f64) -> String {
    if !v.is_finite() {
        return "null".into();
    }
    let sci = format!("{v:e}");
    let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    if (-4..17).contains(&exp) {
        format!("{v}")
    } else {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{mantissa}e{sign}{:02}", exp.abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! fixture {
        ($name:literal) => {
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/json/",
                $name
            ))
        };
    }

    fn strip(s: &str) -> String {
        s.chars()
            .filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n'))
            .collect()
    }

    #[test]
    fn roundtrip_fixture_is_byte_identical() {
        let input = fixture!("roundtrip.json");
        let bvnr = from_json(input).unwrap();
        assert_eq!(strip(&to_json(&bvnr).unwrap()), strip(input));
    }

    #[test]
    fn bad_fixtures_are_refused() {
        for bad in [
            fixture!("bad_diffkeys.json"),
            fixture!("bad_dupkey.json"),
            fixture!("bad_key.json"),
            fixture!("bad_nul.json"),
            fixture!("bad_overflow.json"),
            fixture!("bad_trailing.json"),
        ] {
            assert!(
                matches!(from_json(bad), Err(Error::Argument { .. })),
                "{bad}"
            );
        }
    }

    #[test]
    fn edge_cases() {
        let rt = |j: &str| strip(&to_json(&from_json(j).unwrap()).unwrap());
        // A float stays a float inside an array.
        assert_eq!(rt(r#"{"f":[2.0,0.5]}"#), r#"{"f":[2,0.5]}"#);
        assert_eq!(
            rt(r#"{"e":1e300,"s":1e-7,"x":"a\"é\t"}"#),
            r#"{"e":1e+300,"s":1e-07,"x":"a\"é\t"}"#
        );
        for bad in [
            r#"[1]"#,
            r#"{"a":1e999}"#,
            r#"{"a":-9223372036854775809}"#,
            r#"{"a":"\u0001"}"#,
            r#"{"1a":1}"#,
            r#"{"a":[1"#,
            r#"{"a":01}"#,
            r#"{"j":[[1],[2,3]]}"#,
            r#"{"m":[1,"x"]}"#,
        ] {
            assert!(from_json(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn renders_what_json_cannot_carry() {
        let mut doc = crate::to_string(
            &crate::Value::Struct(vec![
                ("o".into(), crate::Value::Bytes(vec![1, 0xab])),
                ("n".into(), crate::Value::Float(f64::NAN)),
                ("w".into(), crate::Value::Int(i128::MAX)),
            ]),
            false,
        )
        .unwrap();
        doc.extend_from_slice(b".u = <float:64,m> 1.5;");
        let j = to_json(&doc).unwrap();
        assert_eq!(
            strip(&j),
            r#"{"o":"01ab","n":null,"w":"170141183460469231731687303715884105727","u":1.5}"#
        );
    }

    #[test]
    fn detects_format() {
        assert_eq!(detect(b"\xEF\xBB\xBF  {\"a\":1}"), Format::Json);
        assert_eq!(detect(b"\n[1]"), Format::Json);
        assert_eq!(detect(b"#!bovnar 1.1\n.a = 1;"), Format::Bovnar);
        assert_eq!(detect(b" .a = 1;"), Format::Bovnar);
        assert_eq!(detect(b"hello"), Format::Unknown);
        assert_eq!(detect(b""), Format::Unknown);
    }

    /// `n` JSON objects nested under the root object.
    fn nested(n: usize) -> String {
        format!(
            "{{{}1{}}}",
            "{\"a\":".repeat(n + 1).replacen('{', "", 1),
            "}".repeat(n)
        )
    }

    #[test]
    fn deep_output_outgrows_the_first_buffer() {
        // Pretty-printed, each level indents every level below it: the output
        // is many times the input and only fits after the sink has grown.
        let json = nested(200);
        let out = from_json(&json).unwrap();
        assert!(out.len() > json.len() * 2 + 8192, "{} bytes", out.len());
        assert_eq!(strip(&to_json(&out).unwrap()), strip(&json));
    }

    #[test]
    fn nesting_stops_where_the_reader_does() {
        let deepest = (1..400)
            .take_while(|&n| from_json(&nested(n)).is_ok())
            .last()
            .unwrap();
        assert_eq!(deepest, 255);
        assert!(DomDoc::parse(&from_json(&nested(255)).unwrap()).is_ok());
        let err = from_json(&nested(deepest + 1)).unwrap_err().to_string();
        assert!(err.contains("JSON nesting exceeds limit (255)"), "{err}");
    }

    /// Found by fuzzing (fuzz/fuzz_targets/json_roundtrip.rs): JSON has one
    /// number type, so a float with an integral value comes back as an
    /// integer, as from the CLI. The second trip must change nothing.
    #[test]
    fn an_integral_float_becomes_an_integer_once() {
        let doc = from_json(r#"{"n":999999.99999999999}"#).unwrap();
        let json = to_json(&doc).unwrap();
        assert_eq!(strip(&json), r#"{"n":1000000}"#);
        let json2 = to_json(&from_json(&json).unwrap()).unwrap();
        assert_eq!(json2, json);
        // -0.0 is written "-0", an integer, which has no sign: stable from
        // the second trip on.
        let json = to_json(&from_json(r#"{"n":-0.0}"#).unwrap()).unwrap();
        let json2 = to_json(&from_json(&json).unwrap()).unwrap();
        assert_eq!(strip(&json2), r#"{"n":0}"#);
        assert_eq!(to_json(&from_json(&json2).unwrap()).unwrap(), json2);
    }
}
