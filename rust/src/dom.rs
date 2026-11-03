//! The DOM, mirroring `python/bovnar/dom.py`: a parsed, random-access,
//! type-preserving tree with full document-tier validation.

use crate::Value;
use crate::enums::ValueTypeFamily;
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
    pub fn parse(data: &[u8]) -> Result<Self> {
        Self::checked(ffi::DocHandle::parse(data))
    }

    /// [`DomDoc::parse`] under a unit policy: values arrive converted, and a
    /// document breaking the policy fails with `unit_mismatch`.
    pub fn parse_with_policy(data: &[u8], policy: &crate::reader::UnitPolicy) -> Result<Self> {
        Self::checked(ffi::DocHandle::parse_policy(data, &policy.spec()))
    }

    fn checked(handle: Option<ffi::DocHandle>) -> Result<Self> {
        let handle = handle.ok_or_else(|| {
            Error::Argument(
                "bvn_dom_parse() returned NULL (out of memory or input over 4 GiB)".into(),
            )
        })?;
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

    /// The whole document as a [`Value::Struct`] (Python's `to_dict`).
    pub fn to_value(&self) -> Value {
        Value::Struct(
            self.entries()
                .into_iter()
                .map(|(k, n)| (k, n.to_value()))
                .collect(),
        )
    }
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

    /// The canonical unit string, empty when the value has none.
    pub fn unit_str(self) -> String {
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
    /// `to_python`).
    pub fn to_value(self) -> Value {
        match self.dom_type() {
            None | Some(DomType::Null) => Value::Null,
            Some(DomType::Int) => self.int_value(),
            Some(DomType::Float) => self.as_float().map_or(Value::Null, Value::Float),
            Some(DomType::Bool) => self.as_bool().map_or(Value::Null, Value::Bool),
            Some(DomType::String | DomType::Symbol | DomType::Reference) => {
                Value::String(self.as_str().unwrap_or_default().to_owned())
            }
            Some(DomType::OctetStream) => {
                Value::Bytes(self.as_bytes().unwrap_or_default().to_vec())
            }
            Some(DomType::Struct) => Value::Struct(
                self.entries()
                    .into_iter()
                    .map(|(k, n)| (k, n.to_value()))
                    .collect(),
            ),
            Some(DomType::Array) => {
                let flat: Vec<Value> = (0..self.len())
                    .filter_map(|i| self.at(i))
                    .map(|n| n.to_value())
                    .collect();
                let rows = self.array_dims();
                if rows > 1 && flat.len().is_multiple_of(rows) {
                    let width = flat.len() / rows;
                    Value::Array(
                        flat.chunks(width)
                            .map(|r| Value::Array(r.to_vec()))
                            .collect(),
                    )
                } else {
                    Value::Array(flat)
                }
            }
        }
    }

    fn int_value(self) -> Value {
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
            return Value::Int(v);
        }
        // Wider than 64 bits: go through the digits. Past i128 the digits are
        // all this binding can carry so far.
        let digits = self.as_int_str(10).unwrap_or_default();
        digits.parse().map_or(Value::String(digits), Value::Int)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
