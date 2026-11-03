//! Rust data from a borrowed [`Value`] tree.

use alloc::string::String;

use ::serde::de::{
    self, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, Unexpected,
    VariantAccess, Visitor,
};
use ::serde::forward_to_deserialize_any;

use super::Error;
use crate::types::TypeFamily;
use crate::value::{Document, Kind, Number, Value};

/// The document's assignments, read as a map.
pub(super) struct DocumentDeserializer<'de> {
    entries: &'de [(String, Value)],
}

impl<'de> DocumentDeserializer<'de> {
    pub(super) fn new(doc: &'de Document) -> Self {
        Self {
            entries: doc.entries(),
        }
    }
}

impl<'de> de::Deserializer<'de> for DocumentDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_map(Entries::new(self.entries))
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct enum
        identifier ignored_any
    }
}

/// One value.
struct ValueDeserializer<'de>(&'de Value);

fn number<'de, V: Visitor<'de>>(n: &'de Number, visitor: V) -> Result<V::Value, Error> {
    if n.is_integer() {
        if let Some(v) = n.as_u64() {
            return visitor.visit_u64(v);
        }
        if let Some(v) = n.as_i64() {
            return visitor.visit_i64(v);
        }
        if let Some(v) = wide_integer(n) {
            return match v {
                Wide::Unsigned(v) => visitor.visit_u128(v),
                Wide::Signed(v) => visitor.visit_i128(v),
            };
        }
    } else if let Some(v) = n.as_f64() {
        return visitor.visit_f64(v);
    }
    // Datetimes and integers beyond 128 bits have no serde number type.
    visitor.visit_borrowed_str(n.text())
}

enum Wide {
    Unsigned(u128),
    Signed(i128),
}

/// An integer beyond 64 bits, read in its declared base.
fn wide_integer(n: &Number) -> Option<Wide> {
    if !matches!(n.spec().family, TypeFamily::Uint | TypeFamily::Sint) {
        return None;
    }
    let radix = n.spec().effective_base();
    if !(2..=36).contains(&radix) {
        return None;
    }
    let text = n.text();
    if let Ok(v) = u128::from_str_radix(text, radix) {
        return Some(Wide::Unsigned(v));
    }
    i128::from_str_radix(text, radix).ok().map(Wide::Signed)
}

impl<'de> de::Deserializer<'de> for ValueDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.0.kind() {
            Kind::Null => visitor.visit_unit(),
            Kind::Bool(b) => visitor.visit_bool(*b),
            Kind::Number(n) => number(n, visitor),
            Kind::String(s) | Kind::Symbol(s) | Kind::Reference(s) => visitor.visit_borrowed_str(s),
            Kind::Octets(b) => visitor.visit_borrowed_bytes(b),
            Kind::Struct(s) => visitor.visit_map(Entries::new(s.entries())),
            Kind::Array(a) if a.row_count() > 1 => visitor.visit_seq(Rows {
                rows: a.rows(),
                len: a.row_count(),
            }),
            Kind::Array(a) => visitor.visit_seq(Items(a.items().iter())),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        if self.0.is_null() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        match self.0.kind() {
            Kind::Symbol(s) | Kind::String(s) => visitor.visit_enum(s.as_str().into_deserializer()),
            Kind::Struct(s) => match s.entries() {
                [(variant, value)] => visitor.visit_enum(Variant { variant, value }),
                _ => Err(de::Error::invalid_length(
                    s.len(),
                    &"a struct with exactly one key naming the variant",
                )),
            },
            _ => Err(de::Error::invalid_type(
                unexpected(self.0),
                &"a symbol or a one-key struct",
            )),
        }
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct
        identifier
    }
}

fn unexpected(v: &Value) -> Unexpected<'_> {
    match v.kind() {
        Kind::Null => Unexpected::Unit,
        Kind::Bool(b) => Unexpected::Bool(*b),
        Kind::Number(n) => Unexpected::Other(n.text()),
        Kind::String(s) | Kind::Symbol(s) | Kind::Reference(s) => Unexpected::Str(s),
        Kind::Octets(b) => Unexpected::Bytes(b),
        Kind::Struct(_) => Unexpected::Map,
        Kind::Array(_) => Unexpected::Seq,
    }
}

/// Struct fields or document assignments as a serde map.
struct Entries<'de> {
    iter: core::slice::Iter<'de, (String, Value)>,
    value: Option<&'de Value>,
}

impl<'de> Entries<'de> {
    fn new(entries: &'de [(String, Value)]) -> Self {
        Self {
            iter: entries.iter(),
            value: None,
        }
    }
}

impl<'de> MapAccess<'de> for Entries<'de> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        let Some((key, value)) = self.iter.next() else {
            return Ok(None);
        };
        self.value = Some(value);
        seed.deserialize(de::value::BorrowedStrDeserializer::new(key))
            .map(Some)
    }

    fn next_value_seed<S: DeserializeSeed<'de>>(&mut self, seed: S) -> Result<S::Value, Error> {
        let value = self
            .value
            .take()
            .ok_or_else(|| Error::msg("map value requested before its key"))?;
        seed.deserialize(ValueDeserializer(value))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

/// The elements of one array row.
struct Items<'de>(core::slice::Iter<'de, Value>);

impl<'de> SeqAccess<'de> for Items<'de> {
    type Error = Error;

    fn next_element_seed<S: DeserializeSeed<'de>>(
        &mut self,
        seed: S,
    ) -> Result<Option<S::Value>, Error> {
        self.0
            .next()
            .map(|v| seed.deserialize(ValueDeserializer(v)))
            .transpose()
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.0.len())
    }
}

/// The rows of a matrix array, each a sequence.
struct Rows<I> {
    rows: I,
    len: usize,
}

impl<'de, I: Iterator<Item = &'de [Value]>> SeqAccess<'de> for Rows<I> {
    type Error = Error;

    fn next_element_seed<S: DeserializeSeed<'de>>(
        &mut self,
        seed: S,
    ) -> Result<Option<S::Value>, Error> {
        let Some(row) = self.rows.next() else {
            return Ok(None);
        };
        self.len = self.len.saturating_sub(1);
        seed.deserialize(RowDeserializer(row)).map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }
}

/// One matrix row as a sequence.
struct RowDeserializer<'de>(&'de [Value]);

impl<'de> de::Deserializer<'de> for RowDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_seq(Items(self.0.iter()))
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

/// A one-key struct naming an enum variant.
struct Variant<'de> {
    variant: &'de str,
    value: &'de Value,
}

impl<'de> EnumAccess<'de> for Variant<'de> {
    type Error = Error;
    type Variant = ValueDeserializer<'de>;

    fn variant_seed<S: DeserializeSeed<'de>>(
        self,
        seed: S,
    ) -> Result<(S::Value, Self::Variant), Error> {
        let variant = seed.deserialize(de::value::BorrowedStrDeserializer::<Error>::new(
            self.variant,
        ))?;
        Ok((variant, ValueDeserializer(self.value)))
    }
}

impl<'de> VariantAccess<'de> for ValueDeserializer<'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        if self.0.is_null() {
            Ok(())
        } else {
            Err(de::Error::invalid_type(
                unexpected(self.0),
                &"null for a unit variant",
            ))
        }
    }

    fn newtype_variant_seed<S: DeserializeSeed<'de>>(self, seed: S) -> Result<S::Value, Error> {
        seed.deserialize(self)
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_seq(self, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_map(self, visitor)
    }
}
