//! Thin Rust binding to the Bovnar (BVNR) C library.
//!
//! The module split mirrors the Python binding in `python/bovnar/`. All FFI
//! lives in the private `ffi` module; every other module is safe Rust on top
//! of it.

#![deny(unsafe_code)]

pub mod dom;
pub mod enums;
pub mod error;
#[allow(unsafe_code)]
mod ffi;
pub mod reader;
pub mod writer;

pub use dom::{DomDoc, DomNode, DomType};
pub use enums::{ErrorCode, Event, TokenType, ValueTypeFamily};
pub use error::{Error, Result};
pub use ffi::ValueTypeSpec;
pub use reader::{EventData, EventPayload, ReadOptions, Reader};
pub use writer::Writer;

/// A plain BVNR value, the Rust counterpart of what Python's `loads` returns
/// and `dumps` takes. Type annotations and units are not carried.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    /// Any integer up to 128 bits (uint64 and sint64 included).
    Int(i128),
    Float(f64),
    /// A string; symbols and references read back as strings too.
    String(String),
    /// An octet stream.
    Bytes(Vec<u8>),
    /// An array; a multi-row array is an array of row arrays.
    Array(Vec<Value>),
    /// A struct (or the whole document): keys in document order.
    Struct(Vec<(String, Value)>),
}

/// Parses a BVNR document into a [`Value::Struct`], like Python's `loads`.
///
/// This is the single-pass streaming parse: it enforces the grammar and each
/// value's own type, unit and range, but not the document-tier rules
/// (duplicate keys, array homogeneity, rectangular matrices). Use
/// [`DomDoc::parse`] for full validation.
pub fn parse(data: &[u8]) -> Result<Value> {
    let mut parser = dict_parser::DictParser::default();
    let mut on_event = |d: &EventData<'_>| {
        parser.on_event(d);
        true
    };
    Reader::new()?.read_mem(data, &ReadOptions::default(), Some(&mut on_event), None)?;
    Ok(parser.result())
}

/// Serialises a [`Value::Struct`] to a BVNR document, like Python's `dumps`.
///
/// Returns bytes rather than `String`: an octet stream is written raw.
/// Arrays and integers wider than 64 bits are not supported yet and are
/// refused with [`Error::Argument`].
pub fn to_string(value: &Value, pretty: bool) -> Result<Vec<u8>> {
    let Value::Struct(entries) = value else {
        return Err(Error::Argument(
            "to_string() requires a struct at the top level".into(),
        ));
    };
    let mut cap = writer::DEFAULT_MEM_CAP;
    loop {
        let mut w = Writer::to_mem(cap, pretty)?;
        match emit_entries(&mut w, entries).and_then(|_| w.finish()) {
            Ok(()) => return Ok(w.get_output().to_vec()),
            Err(e) if e.code() == Some(ErrorCode::SinkBufferExhausted) && cap < 256 << 20 => {
                cap *= 2;
            }
            Err(e) => return Err(e),
        }
    }
}

fn emit_entries(w: &mut Writer, entries: &[(String, Value)]) -> Result<()> {
    entries.iter().try_for_each(|(k, v)| emit_value(w, k, v))
}

fn emit_value(w: &mut Writer, key: &str, value: &Value) -> Result<()> {
    match value {
        Value::Null => w.write_null(key),
        Value::Bool(b) => w.write_bool(key, *b),
        Value::Int(i) => {
            if let Ok(u) = u64::try_from(*i) {
                w.write_uint(key, u, 64, 10)
            } else if let Ok(s) = i64::try_from(*i) {
                w.write_sint(key, s, 64, 10)
            } else {
                Err(Error::Argument(format!(
                    "cannot serialise {key:?}: integers wider than 64 bits are not supported yet"
                )))
            }
        }
        Value::Float(f) => w.write_float(key, *f, 64),
        Value::String(s) => w.write_string(key, s),
        Value::Bytes(b) => w.write_octets(key, b),
        Value::Struct(entries) => {
            w.begin_struct(key)?;
            emit_entries(w, entries)?;
            w.end_struct()
        }
        Value::Array(_) => Err(Error::Argument(format!(
            "cannot serialise {key:?}: arrays are not supported by to_string yet"
        ))),
    }
}

/// The event consumer behind [`parse`], a port of Python's `_DictParser`.
///
/// Arrays are sealed lazily: a row end leaves its array pending, and the next
/// event decides whether a `/` continues it, an enclosing row takes it as an
/// element, or the assignment it belongs to is complete.
mod dict_parser {
    use crate::{Event, EventData, TokenType, Value, ValueTypeFamily, ValueTypeSpec};

    #[derive(Default)]
    struct ArrScope {
        rows: Vec<Vec<Value>>,
        cur_row: Vec<Value>,
        dim_continue: bool,
        pending_seal: bool,
        /// The key the array is assigned to, saved when it opened.
        parent_key: Option<String>,
    }

    impl ArrScope {
        /// One row unwraps to a flat array; several become an array of rows.
        fn seal(mut self) -> Value {
            if self.rows.len() == 1 {
                Value::Array(self.rows.pop().unwrap_or_default())
            } else {
                Value::Array(self.rows.into_iter().map(Value::Array).collect())
            }
        }
    }

    enum Scope {
        Doc(Vec<(String, Value)>),
        Struct(Vec<(String, Value)>, Option<String>),
        Array(ArrScope),
    }

    pub struct DictParser {
        stack: Vec<Scope>,
        current_key: Option<String>,
        in_octet: bool,
        octets: Vec<u8>,
    }

    impl Default for DictParser {
        fn default() -> Self {
            DictParser {
                stack: vec![Scope::Doc(Vec::new())],
                current_key: None,
                in_octet: false,
                octets: Vec::new(),
            }
        }
    }

    /// Dict assignment: a repeated key keeps its first position, last value.
    fn insert(entries: &mut Vec<(String, Value)>, key: String, value: Value) {
        match entries.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => entries.push((key, value)),
        }
    }

    impl DictParser {
        pub fn on_event(&mut self, d: &EventData<'_>) {
            match d.event {
                Event::AssignmentStart => {
                    self.maybe_seal_array();
                    self.current_key = Some(String::from_utf8_lossy(d.raw).into_owned());
                }
                Event::StructStart => {
                    let key = self.current_key.take();
                    self.stack.push(Scope::Struct(Vec::new(), key));
                }
                Event::StructEnd => {
                    self.maybe_seal_array();
                    if self.stack.len() > 1
                        && matches!(self.stack.last(), Some(Scope::Struct(..)))
                        && let Some(Scope::Struct(entries, key)) = self.stack.pop()
                    {
                        self.commit(key, Value::Struct(entries));
                    }
                }
                Event::ArrayDimStart => {
                    if let Some(Scope::Array(a)) = self.stack.last_mut() {
                        a.pending_seal = false;
                        a.dim_continue = true;
                    }
                }
                Event::ArrayRowStart => match self.stack.last_mut() {
                    Some(Scope::Array(a)) if a.dim_continue => {
                        a.cur_row.clear();
                        a.dim_continue = false;
                    }
                    _ => {
                        if matches!(self.stack.last(), Some(Scope::Array(a)) if a.pending_seal) {
                            self.seal_top();
                        }
                        let parent_key = self.current_key.take();
                        self.stack.push(Scope::Array(ArrScope {
                            parent_key,
                            ..ArrScope::default()
                        }));
                    }
                },
                Event::ArrayRowEnd => {
                    self.flush_pending_inner_arrays();
                    if let Some(Scope::Array(a)) = self.stack.last_mut() {
                        let row = std::mem::take(&mut a.cur_row);
                        a.rows.push(row);
                        a.pending_seal = true;
                    }
                }
                Event::OctetStreamStart => {
                    self.in_octet = true;
                    self.octets.clear();
                }
                Event::OctetStreamEnd => {
                    self.in_octet = false;
                    let bytes = std::mem::take(&mut self.octets);
                    self.push_value(Value::Bytes(bytes));
                }
                Event::Data if self.in_octet => self.octets.extend_from_slice(d.raw),
                Event::Data => {
                    let value = decode_value(d.raw, d.value_type, d.token_type);
                    self.push_value(value);
                }
                _ => {}
            }
        }

        pub fn result(mut self) -> Value {
            self.maybe_seal_array();
            while self.stack.len() > 1 {
                // An unterminated scope (continue_on_error) still lands.
                match self.stack.pop() {
                    Some(Scope::Struct(entries, key)) => self.commit(key, Value::Struct(entries)),
                    Some(Scope::Array(a)) => {
                        let key = a.parent_key.clone();
                        self.commit(key, a.seal());
                    }
                    _ => {}
                }
            }
            match self.stack.pop() {
                Some(Scope::Doc(entries)) => Value::Struct(entries),
                _ => Value::Struct(Vec::new()),
            }
        }

        /// A scalar or octet stream: into the open row, or under the
        /// current key.
        fn push_value(&mut self, value: Value) {
            match self.stack.last_mut() {
                Some(Scope::Doc(entries) | Scope::Struct(entries, _)) => {
                    if let Some(key) = self.current_key.take() {
                        insert(entries, key, value);
                    }
                }
                Some(Scope::Array(a)) => a.cur_row.push(value),
                None => {}
            }
        }

        /// A finished struct or array: into the open row, or under the key
        /// saved when it opened.
        fn commit(&mut self, key: Option<String>, value: Value) {
            match self.stack.last_mut() {
                Some(Scope::Doc(entries) | Scope::Struct(entries, _)) => {
                    if let Some(key) = key {
                        insert(entries, key, value);
                    }
                }
                Some(Scope::Array(a)) => a.cur_row.push(value),
                None => {}
            }
        }

        fn seal_top(&mut self) {
            if let Some(Scope::Array(a)) = self.stack.pop() {
                let key = a.parent_key.clone();
                self.commit(key, a.seal());
            }
        }

        fn maybe_seal_array(&mut self) {
            while matches!(self.stack.last(), Some(Scope::Array(a)) if a.pending_seal) {
                self.seal_top();
            }
        }

        /// Before a row ends, completed inner arrays become its elements.
        fn flush_pending_inner_arrays(&mut self) {
            while self.stack.len() >= 2
                && matches!(self.stack.last(), Some(Scope::Array(a)) if a.pending_seal)
            {
                self.seal_top();
            }
        }
    }

    /// Python's `_char_to_digit`: the digit value of `c` in `base`, or `None`.
    /// Bases 64 and 85 use the Base64 and Ascii85 alphabets; up to 36 letters
    /// are case-insensitive, 37..62 case-sensitive.
    fn char_to_digit(c: u8, base: u32) -> Option<u32> {
        let d = match (base, c) {
            (64, b'A'..=b'Z') => u32::from(c - b'A'),
            (64, b'a'..=b'z') => u32::from(c - b'a') + 26,
            (64, b'0'..=b'9') => u32::from(c - b'0') + 52,
            (64, b'+') => 62,
            (64, b'/') => 63,
            (64, _) => return None,
            (85, b'!'..=b'u') => u32::from(c - b'!'),
            (85, _) => return None,
            (_, b'0'..=b'9') => u32::from(c - b'0'),
            (_, b'a'..=b'z') => 10 + u32::from(c - b'a'),
            (_, b'A'..=b'Z') if base > 36 => 36 + u32::from(c - b'A'),
            (_, b'A'..=b'Z') => 10 + u32::from(c - b'A'),
            _ => return None,
        };
        (d < base).then_some(d)
    }

    /// Python's `_int_from_base`: an integer in any bovnar base (2..62, 64,
    /// 85). Bases 64 and 85 are unsigned: their '+' and '-' are digits.
    fn int_from_base(text: &str, base: u32) -> Option<i128> {
        let bytes = text.as_bytes();
        let (neg, digits) = match bytes.first() {
            Some(b'+') if base != 64 && base != 85 => (false, &bytes[1..]),
            Some(b'-') if base != 64 && base != 85 => (true, &bytes[1..]),
            _ => (false, bytes),
        };
        if digits.is_empty() {
            return None;
        }
        let mut val: i128 = 0;
        for &c in digits {
            let d = char_to_digit(c, base)?;
            val = val
                .checked_mul(i128::from(base))?
                .checked_add(i128::from(d))?;
        }
        Some(if neg { -val } else { val })
    }

    /// Python's `_decode_value`: one literal as a plain value. What cannot be
    /// decoded stays its text, as in Python.
    fn decode_value(raw: &[u8], vt: ValueTypeSpec, token: Option<TokenType>) -> Value {
        if raw.is_empty() {
            return match token {
                Some(TokenType::String | TokenType::ArrayString) => Value::String(String::new()),
                _ => Value::Null,
            };
        }
        let text = String::from_utf8_lossy(raw).into_owned();
        match token {
            // The validator normalises on/off to true/false.
            Some(TokenType::Bool) => return Value::Bool(text == "true"),
            Some(TokenType::Symbol) => return Value::String(text),
            _ => {}
        }
        let base = if vt.base > 1 { vt.base } else { 10 };
        match ValueTypeFamily::from_raw(vt.family) {
            // Epoch seconds; `base` holds the epoch index, not a radix.
            Some(ValueTypeFamily::Datetime) => int_from_base(&text, 10).map(Value::Int),
            Some(ValueTypeFamily::Uint | ValueTypeFamily::Sint) => {
                int_from_base(&text, base).map(Value::Int)
            }
            Some(
                family @ (ValueTypeFamily::Float
                | ValueTypeFamily::FloatFix
                | ValueTypeFamily::FloatDec),
            ) => {
                match text.as_str() {
                    "nan" => Some(Value::Float(f64::NAN)),
                    "inf" => Some(Value::Float(f64::INFINITY)),
                    "ninf" => Some(Value::Float(f64::NEG_INFINITY)),
                    // A <float:W,_16> literal is a hex float; decoding it is
                    // not ported yet, so it stays text. float_fix keeps Q and
                    // float_dec nothing in `base`: both are always decimal.
                    _ if family == ValueTypeFamily::Float && base == 16 => None,
                    _ => text.parse().ok().map(Value::Float),
                }
            }
            _ => None,
        }
        .unwrap_or(Value::String(text))
    }
}
