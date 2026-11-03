//! The DOM, mirroring `python/bovnar/dom.py`: a parsed, random-access,
//! type-preserving tree with full document-tier validation.

use crate::Value;
use crate::enums::{ErrorCode, ValueTypeFamily};
use crate::error::{Error, Result};
use crate::ffi::{self, ValueTypeSpec};

/// Node kinds (`bvn_dom_type_t`).
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DomType {
    Null = 0,
    Int = 1,
    Float = 2,
    String = 3,
    Symbol = 4,
    Reference = 5,
    Struct = 6,
    Array = 7,
    OctetStream = 8,
    Bool = 9,
}

impl DomType {
    /// The variant for a raw C value, or `None` for a value this binding
    /// does not know.
    pub fn from_raw(raw: i32) -> Option<Self> {
        Some(match raw {
            0 => Self::Null,
            1 => Self::Int,
            2 => Self::Float,
            3 => Self::String,
            4 => Self::Symbol,
            5 => Self::Reference,
            6 => Self::Struct,
            7 => Self::Array,
            8 => Self::OctetStream,
            9 => Self::Bool,
            _ => return None,
        })
    }
}

/// A parsed document. Nodes borrow it.
pub struct DomDoc {
    handle: ffi::DocHandle,
}

impl DomDoc {
    /// Parses BVNR bytes into a DOM tree; a malformed document is an
    /// [`Error::Parse`] carrying the C error code.
    ///
    /// The error has no position: the C DOM reports only the code
    /// (`bvn_dom_doc_get_parse_error`), so `line`, `column`, `offset` and
    /// `byte` are always 0. Read the document with a [`crate::Reader`] to
    /// locate the error.
    pub fn parse(data: &[u8]) -> Result<Self> {
        Self::checked(data, ffi::DocHandle::parse(data))
    }

    /// [`DomDoc::parse`] under a unit policy: values arrive converted, and a
    /// document breaking the policy fails with `unit_mismatch`. A policy the
    /// library refuses is an [`Error::Argument`] naming the bad entry, the
    /// same error [`crate::Reader::set_unit_policy`] gives, before anything is
    /// parsed.
    pub fn parse_with_policy(data: &[u8], policy: &crate::reader::UnitPolicy) -> Result<Self> {
        policy.check_counts()?;
        // bovnar_dom.h: a policy the library refuses is error_invalid_argument,
        // which no fault in the document produces. Only then is the reason
        // worked out, by the same probe Reader::set_unit_policy uses.
        Self::checked(data, ffi::DocHandle::parse_policy(data, &policy.spec())).map_err(|e| {
            if e.code() == Some(ErrorCode::InvalidArgument) {
                Error::Argument(crate::reader::describe_unit_policy_refusal(policy))
            } else {
                e
            }
        })
    }

    /// `handle` is `None` when the input is over the C API's 4 GiB (a
    /// `uint32_t` length) or the library could not allocate.
    fn checked(data: &[u8], handle: Option<ffi::DocHandle>) -> Result<Self> {
        if u32::try_from(data.len()).is_err() {
            return Err(Error::Argument(format!(
                "a DOM document is at most 4 GiB, got {} bytes",
                data.len()
            )));
        }
        let handle = handle.ok_or(Error::OutOfMemory("bvn_dom_parse()"))?;
        match handle.parse_error() {
            0 => Ok(DomDoc { handle }),
            code => Err(Error::Parse {
                code,
                line: 0,
                column: 0,
                offset: 0,
                byte: 0,
            }),
        }
    }

    /// The node at a dotted path such as `"a.b"`.
    pub fn lookup(&self, path: &str) -> Option<DomNode<'_>> {
        self.handle.lookup(path).map(DomNode)
    }

    /// The top-level value assigned to `key`.
    pub fn get(&self, key: &str) -> Option<DomNode<'_>> {
        self.entries()
            .into_iter()
            .find(|(k, _)| k == key)
            .map(|(_, n)| n)
    }

    /// The number of top-level assignments.
    pub fn len(&self) -> usize {
        self.handle.entries().len()
    }

    /// Whether the document assigns nothing.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The top-level `(key, node)` pairs, in document order.
    pub fn entries(&self) -> Vec<(String, DomNode<'_>)> {
        to_entries(self.handle.entries())
    }

    /// The whole document as a [`Value::Struct`] (Python's `to_dict`). Fails,
    /// as Python raises, where a node cannot be read as its kind; see
    /// [`DomNode::to_value`].
    pub fn to_value(&self) -> Result<Value> {
        strict_entries(self.handle.entries()).map(Value::Struct)
    }
}

/// `(key, value)` pairs for a [`Value::Struct`]: a key that is not UTF-8 is an
/// error, not replaced, so two distinct keys cannot collapse into one.
fn strict_entries(raw: Vec<(&[u8], ffi::Node<'_>)>) -> Result<Vec<(String, Value)>> {
    raw.into_iter()
        .map(|(k, n)| {
            let key = std::str::from_utf8(k)
                .map_err(|_| Error::Argument(format!("DOM key {k:?} is not valid UTF-8")))?;
            Ok((key.to_owned(), DomNode(n).to_value()?))
        })
        .collect()
}

/// The error for a node [`DomNode::to_value`] cannot read as its kind.
fn unreadable(what: &str) -> Error {
    Error::Argument(format!("DOM node cannot be read: {what}"))
}

fn to_entries<'a>(raw: Vec<(&'a [u8], ffi::Node<'a>)>) -> Vec<(String, DomNode<'a>)> {
    raw.into_iter()
        .map(|(k, n)| (String::from_utf8_lossy(k).into_owned(), DomNode(n)))
        .collect()
}

/// One node of a [`DomDoc`].
#[derive(Clone, Copy)]
pub struct DomNode<'a>(ffi::Node<'a>);

impl<'a> DomNode<'a> {
    /// The node's kind; `None` for a kind this binding does not know.
    pub fn dom_type(self) -> Option<DomType> {
        DomType::from_raw(self.0.node_type())
    }

    /// The value's type annotation (family, width, base).
    pub fn value_type(self) -> ValueTypeSpec {
        self.0.value_type()
    }

    /// The canonical unit string, empty when the value has none, the same
    /// contract as [`crate::Quantity::unit_str`]. (The C formatter spells no
    /// unit `no_unit`; that literal must not come through here.)
    pub fn unit_str(self) -> String {
        if self.unit().is_dimensionless() {
            return String::new();
        }
        self.0.unit_string()
    }

    /// The value's unit; dimensionless when it has none.
    pub fn unit(self) -> crate::Unit {
        crate::Unit(self.0.unit())
    }

    /// The unit as the document spelled it, when that differs from the
    /// canonical form or a policy converted the value.
    pub fn unit_source(self) -> Option<&'a str> {
        let (u, _) = self.0.unit_source_and_fraction();
        u.and_then(|b| std::str::from_utf8(b).ok())
    }

    /// A datetime's verbatim sub-second digits.
    pub fn datetime_fraction(self) -> Option<&'a str> {
        let (_, f) = self.0.unit_source_and_fraction();
        f.and_then(|b| std::str::from_utf8(b).ok())
    }

    /// The numeric value in coherent SI base units.
    pub fn value_in_base_units(self) -> f64 {
        self.0.value_in_base_units()
    }

    pub fn is_null(self) -> bool {
        self.dom_type() == Some(DomType::Null)
    }

    pub fn as_i64(self) -> Option<i64> {
        self.0.get_i64()
    }

    pub fn as_u64(self) -> Option<u64> {
        self.0.get_u64()
    }

    pub fn as_i32(self) -> Option<i32> {
        self.0.get_i32()
    }

    pub fn as_u32(self) -> Option<u32> {
        self.0.get_u32()
    }

    pub fn as_i16(self) -> Option<i16> {
        self.0.get_i16()
    }

    pub fn as_u16(self) -> Option<u16> {
        self.0.get_u16()
    }

    pub fn as_i8(self) -> Option<i8> {
        self.0.get_i8()
    }

    pub fn as_u8(self) -> Option<u8> {
        self.0.get_u8()
    }

    pub fn as_float(self) -> Option<f64> {
        self.0.get_float()
    }

    pub fn as_bool(self) -> Option<bool> {
        self.0.get_bool()
    }

    /// A string, symbol or reference node as text.
    pub fn as_str(self) -> Option<&'a str> {
        let raw = match self.dom_type()? {
            DomType::String => self.0.get_string(),
            DomType::Symbol => self.0.get_symbol(),
            DomType::Reference => self.0.get_reference(),
            _ => None,
        }?;
        std::str::from_utf8(raw).ok()
    }

    /// An octet-stream node's payload.
    pub fn as_bytes(self) -> Option<&'a [u8]> {
        self.0.get_octets()
    }

    /// Any integer node, however wide, as digits in `base`.
    pub fn as_int_str(self, base: u32) -> Option<String> {
        self.0.int_to_str(base)
    }

    /// Entries of a struct, elements of an array, 0 otherwise.
    pub fn len(self) -> usize {
        match self.dom_type() {
            Some(DomType::Struct) => self.0.struct_count() as usize,
            Some(DomType::Array) => self.0.array_count() as usize,
            _ => 0,
        }
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// A struct member by key.
    pub fn get(self, key: &str) -> Option<DomNode<'a>> {
        self.0.struct_get(key).map(DomNode)
    }

    /// An array element by flat index.
    pub fn at(self, index: usize) -> Option<DomNode<'a>> {
        self.0.array_at(u32::try_from(index).ok()?).map(DomNode)
    }

    /// The `(key, node)` pairs of a struct, in document order.
    pub fn entries(self) -> Vec<(String, DomNode<'a>)> {
        to_entries(self.0.struct_entries())
    }

    /// The number of `/`-separated rows of an array.
    pub fn array_dims(self) -> usize {
        self.0.array_dims() as usize
    }

    /// The node as a plain [`Value`]; type and unit are dropped (Python's
    /// `to_python`). Where Python raises, this fails instead of defaulting:
    /// text or a key that is not UTF-8, a float or bool node without its
    /// value, an unknown node kind.
    pub fn to_value(self) -> Result<Value> {
        let raw = self.0.node_type();
        Ok(match DomType::from_raw(raw) {
            None => return Err(unreadable(&format!("unknown node type {raw}"))),
            Some(DomType::Null) => Value::Null,
            Some(DomType::Int) => self.int_value()?,
            Some(DomType::Float) => Value::Float(
                self.as_float()
                    .ok_or_else(|| unreadable("a float node without a float value"))?,
            ),
            Some(DomType::Bool) => Value::Bool(
                self.as_bool()
                    .ok_or_else(|| unreadable("a bool node without a bool value"))?,
            ),
            Some(DomType::String | DomType::Symbol | DomType::Reference) => Value::String(
                self.as_str()
                    .ok_or_else(|| unreadable("text that is not valid UTF-8"))?
                    .to_owned(),
            ),
            Some(DomType::OctetStream) => Value::Bytes(
                self.as_bytes()
                    .ok_or_else(|| unreadable("an octet stream without a payload"))?
                    .to_vec(),
            ),
            Some(DomType::Struct) => Value::Struct(strict_entries(self.0.struct_entries())?),
            Some(DomType::Array) => {
                let flat = (0..self.len())
                    .map(|i| {
                        self.at(i)
                            .ok_or_else(|| unreadable(&format!("array element {i} is missing")))?
                            .to_value()
                    })
                    .collect::<Result<Vec<Value>>>()?;
                let rows = self.array_dims();
                if rows > 1 && flat.len().is_multiple_of(rows) {
                    // Index by row rather than chunking: rows of zero
                    // elements ("[]/[]") still count, as loads() keeps them.
                    let width = flat.len() / rows;
                    Value::Array(
                        (0..rows)
                            .map(|i| Value::Array(flat[i * width..(i + 1) * width].to_vec()))
                            .collect(),
                    )
                } else {
                    Value::Array(flat)
                }
            }
        })
    }

    fn int_value(self) -> Result<Value> {
        let family = ValueTypeFamily::from_raw(self.value_type().family);
        let signed_first = matches!(
            family,
            Some(ValueTypeFamily::Sint | ValueTypeFamily::Datetime)
        );
        let native = if signed_first {
            self.as_i64()
                .map(i128::from)
                .or_else(|| self.as_u64().map(i128::from))
        } else {
            self.as_u64()
                .map(i128::from)
                .or_else(|| self.as_i64().map(i128::from))
        };
        if let Some(v) = native {
            return Ok(Value::Int(v));
        }
        // Wider than 64 bits: go through the digits. Past i128 the digits are
        // all this binding can carry so far.
        let digits = self
            .as_int_str(10)
            .ok_or_else(|| unreadable("an integer node without digits"))?;
        Ok(digits.parse().map_or(Value::String(digits), Value::Int))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_that_is_not_utf8_is_an_error_not_replaced() {
        // The reader refuses such keys, so no document carries one; the
        // check is fed a real node under a forged key instead.
        let doc = DomDoc::parse(b".a = 1;").unwrap();
        let (_, node) = doc.handle.entries()[0];
        let err = strict_entries(vec![(b"\xffa", node)]).unwrap_err();
        assert!(err.to_string().contains("not valid UTF-8"), "{err}");
        assert_eq!(
            strict_entries(vec![(b"a", node)]).unwrap(),
            vec![("a".to_owned(), Value::Int(1))]
        );
    }

    #[test]
    fn policy_parse_converts_and_exposes_units() {
        let doc = b".d = <float:64,k~m> 1.5;";
        let plain = DomDoc::parse(doc).unwrap();
        let d = plain.get("d").unwrap();
        assert_eq!(d.unit().to_string(), "k~m");
        assert_eq!(d.value_in_base_units(), 1500.0);
        let policy = crate::UnitPolicy {
            targets: vec![("m".into(), 0)],
            ..Default::default()
        };
        let conv = DomDoc::parse_with_policy(doc, &policy).unwrap();
        let d = conv.get("d").unwrap();
        assert_eq!((d.as_float(), d.unit_str()), (Some(1500.0), "m".into()));
    }

    #[test]
    fn parse_and_walk() {
        let doc = DomDoc::parse(b".a = 1; .s = { .t = \"x\"; };").unwrap();
        assert_eq!(doc.len(), 2);
        assert_eq!(doc.get("a").and_then(|n| n.as_u64()), Some(1));
        assert_eq!(doc.lookup("s.t").and_then(|n| n.as_str()), Some("x"));
        assert_eq!(
            doc.get("s").map(|n| n.dom_type()),
            Some(Some(DomType::Struct))
        );
    }

    #[test]
    fn duplicate_key_is_a_document_error() {
        let err = DomDoc::parse(b".a = 1; .a = 2;").err().unwrap();
        assert_eq!(err.name(), Some("duplicate_struct_key"));
    }
}
