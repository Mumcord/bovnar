//! Rust data to a [`Value`] tree.

use alloc::collections::BTreeSet;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use ::serde::ser::{self, Serialize};

use super::Error;
use crate::types::{TypeFamily, TypeSpec};
use crate::value::{Array, Document, Kind, Number, Struct, Value};

pub(super) fn to_document<T: ?Sized + Serialize>(value: &T) -> Result<Document, Error> {
    match value.serialize(ValueSerializer)?.into_kind() {
        Kind::Struct(s) => {
            let doc = Document::from_parts(None, s.into_entries());
            // The writer does not enforce the tree rules (homogeneous arrays,
            // matching struct shapes); check them so whatever is written
            // also reads back.
            crate::dom::check_document(&doc)?;
            Ok(doc)
        }
        _ => Err(Error::msg(
            "the top level of a document must serialize as a struct or map",
        )),
    }
}

fn wide(family: TypeFamily, text: String) -> Value {
    Value::from(Number::new(
        TypeSpec {
            family,
            width: 128,
            base: 0,
        },
        text,
    ))
}

fn tagged(variant: &str, value: Value) -> Value {
    Value::from(Struct::from_entries(alloc::vec![(
        variant.to_string(),
        value
    )]))
}

/// Serializes into a [`Value`].
struct ValueSerializer;

impl ser::Serializer for ValueSerializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = SeqSerializer;
    type SerializeTuple = SeqSerializer;
    type SerializeTupleStruct = SeqSerializer;
    type SerializeTupleVariant = SeqSerializer;
    type SerializeMap = MapSerializer;
    type SerializeStruct = MapSerializer;
    type SerializeStructVariant = MapSerializer;

    fn serialize_bool(self, v: bool) -> Result<Value, Error> {
        Ok(Value::from(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Value, Error> {
        Ok(Value::from(Number::from(v)))
    }

    fn serialize_i16(self, v: i16) -> Result<Value, Error> {
        Ok(Value::from(Number::from(v)))
    }

    fn serialize_i32(self, v: i32) -> Result<Value, Error> {
        Ok(Value::from(Number::from(v)))
    }

    fn serialize_i64(self, v: i64) -> Result<Value, Error> {
        Ok(Value::from(Number::from(v)))
    }

    fn serialize_i128(self, v: i128) -> Result<Value, Error> {
        Ok(wide(TypeFamily::Sint, v.to_string()))
    }

    fn serialize_u8(self, v: u8) -> Result<Value, Error> {
        Ok(Value::from(Number::from(v)))
    }

    fn serialize_u16(self, v: u16) -> Result<Value, Error> {
        Ok(Value::from(Number::from(v)))
    }

    fn serialize_u32(self, v: u32) -> Result<Value, Error> {
        Ok(Value::from(Number::from(v)))
    }

    fn serialize_u64(self, v: u64) -> Result<Value, Error> {
        Ok(Value::from(Number::from(v)))
    }

    fn serialize_u128(self, v: u128) -> Result<Value, Error> {
        Ok(wide(TypeFamily::Uint, v.to_string()))
    }

    fn serialize_f32(self, v: f32) -> Result<Value, Error> {
        Ok(Value::from(Number::from(v)))
    }

    fn serialize_f64(self, v: f64) -> Result<Value, Error> {
        Ok(Value::from(Number::from(v)))
    }

    fn serialize_char(self, v: char) -> Result<Value, Error> {
        Ok(Value::from(v.to_string()))
    }

    fn serialize_str(self, v: &str) -> Result<Value, Error> {
        Ok(Value::from(v))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Value, Error> {
        Ok(Value::from(v))
    }

    fn serialize_none(self) -> Result<Value, Error> {
        Ok(Value::null())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Value, Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Value, Error> {
        Ok(Value::null())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Error> {
        Ok(Value::null())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<Value, Error> {
        Ok(Value::symbol(variant))
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Value, Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, Error> {
        Ok(tagged(variant, value.serialize(self)?))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<SeqSerializer, Error> {
        Ok(SeqSerializer::new(None, len))
    }

    fn serialize_tuple(self, len: usize) -> Result<SeqSerializer, Error> {
        Ok(SeqSerializer::new(None, Some(len)))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<SeqSerializer, Error> {
        Ok(SeqSerializer::new(None, Some(len)))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<SeqSerializer, Error> {
        Ok(SeqSerializer::new(Some(variant), Some(len)))
    }

    fn serialize_map(self, len: Option<usize>) -> Result<MapSerializer, Error> {
        Ok(MapSerializer::new(None, len))
    }

    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<MapSerializer, Error> {
        Ok(MapSerializer::new(None, Some(len)))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<MapSerializer, Error> {
        Ok(MapSerializer::new(Some(variant), Some(len)))
    }
}

/// Collects array elements; `variant` wraps the array in a one-key struct.
struct SeqSerializer {
    variant: Option<&'static str>,
    items: Vec<Value>,
}

impl SeqSerializer {
    fn new(variant: Option<&'static str>, len: Option<usize>) -> Self {
        // The hint comes from the caller's data, but cap it anyway so a
        // lying `Serialize` impl cannot force a huge allocation.
        let capacity = len.unwrap_or(0).min(4096);
        Self {
            variant,
            items: Vec::with_capacity(capacity),
        }
    }

    fn push<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.items.push(value.serialize(ValueSerializer)?);
        Ok(())
    }

    fn finish(self) -> Value {
        let array = Value::from(self.items.into_iter().collect::<Array>());
        match self.variant {
            Some(variant) => tagged(variant, array),
            None => array,
        }
    }
}

impl ser::SerializeSeq for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }

    fn end(self) -> Result<Value, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeTuple for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }

    fn end(self) -> Result<Value, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeTupleStruct for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }

    fn end(self) -> Result<Value, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeTupleVariant for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        self.push(value)
    }

    fn end(self) -> Result<Value, Error> {
        Ok(self.finish())
    }
}

/// Collects struct fields or map entries; `variant` wraps the struct in a
/// one-key struct.
struct MapSerializer {
    variant: Option<&'static str>,
    entries: Vec<(String, Value)>,
    keys: BTreeSet<String>,
    pending_key: Option<String>,
}

impl MapSerializer {
    fn new(variant: Option<&'static str>, len: Option<usize>) -> Self {
        Self {
            variant,
            entries: Vec::with_capacity(len.unwrap_or(0).min(4096)),
            keys: BTreeSet::new(),
            pending_key: None,
        }
    }

    fn insert<T: ?Sized + Serialize>(&mut self, key: String, value: &T) -> Result<(), Error> {
        let value = value.serialize(ValueSerializer)?;
        if !self.keys.insert(key.clone()) {
            return Err(Error::msg(format_args!("duplicate key `{key}`")));
        }
        self.entries.push((key, value));
        Ok(())
    }

    fn finish(self) -> Value {
        let s = Value::from(Struct::from_entries(self.entries));
        match self.variant {
            Some(variant) => tagged(variant, s),
            None => s,
        }
    }
}

impl ser::SerializeMap for MapSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Error> {
        // Keys become identifiers, so only text-like keys (strings, chars,
        // unit variants) are accepted.
        let key = match key.serialize(ValueSerializer)?.into_kind() {
            Kind::String(s) | Kind::Symbol(s) => s,
            _ => return Err(Error::msg("map keys must serialize as strings")),
        };
        self.pending_key = Some(key);
        Ok(())
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Error> {
        let key = self
            .pending_key
            .take()
            .ok_or_else(|| Error::msg("map value without a key"))?;
        self.insert(key, value)
    }

    fn end(self) -> Result<Value, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeStruct for MapSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.insert(key.to_string(), value)
    }

    fn end(self) -> Result<Value, Error> {
        Ok(self.finish())
    }
}

impl ser::SerializeStructVariant for MapSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.insert(key.to_string(), value)
    }

    fn end(self) -> Result<Value, Error> {
        Ok(self.finish())
    }
}
