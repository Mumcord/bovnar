//! The document tree: [`Document`], [`Value`] and their parts.
//!
//! A tree built by [`crate::parse`] is lossless: every value keeps the type
//! annotation written in front of it ([`Annotation`]) and numbers keep their
//! literal text, so [`crate::to_string`] writes the document back exactly as
//! the C reference's `bovnar pretty-print` would.

mod number;
mod path;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::mem;

pub use number::Number;

use crate::types::{TypeFamily, TypeSpec};
use crate::SpecVersion;

/// A type annotation as written in front of a value (`<float:64,m>`), or as
/// the reader synthesised it for an untyped value.
///
/// The parameters are `Some` exactly where the annotation spells them out,
/// so writing it back reproduces the original text.
///
/// ```
/// use bovnar::{Annotation, TypeFamily};
///
/// let ann = Annotation::new(TypeFamily::Float).with_width(32).with_unit("m");
/// assert_eq!(ann.unit.as_deref(), Some("m"));
/// ```
#[derive(Debug, Clone, Eq)]
#[non_exhaustive]
pub struct Annotation {
    /// The type family.
    pub family: TypeFamily,
    /// Width in bits, if written.
    pub width: Option<u32>,
    /// Numeric base, if written.
    pub base: Option<u32>,
    /// Fraction bits of a `float_fix`, if written.
    pub q: Option<u32>,
    /// Datetime epoch (`unix`, `tai`, ...), if written.
    pub epoch: Option<&'static str>,
    /// The unit as written, if any.
    pub unit: Option<String>,
    /// Whether the reader synthesised this annotation for an untyped value.
    pub synthesized: bool,
}

/// Annotations are equal when they read the same; whether the reader
/// synthesised one is provenance, not content (writing a synthesised
/// annotation makes it an explicit one).
impl PartialEq for Annotation {
    fn eq(&self, other: &Self) -> bool {
        self.family == other.family
            && self.width == other.width
            && self.base == other.base
            && self.q == other.q
            && self.epoch == other.epoch
            && self.unit == other.unit
    }
}

impl Annotation {
    /// An annotation naming only the family (`<utf8>`).
    pub fn new(family: TypeFamily) -> Self {
        Self {
            family,
            width: None,
            base: None,
            q: None,
            epoch: None,
            unit: None,
            synthesized: false,
        }
    }

    /// The annotation that spells out `spec` and `unit`: every non-default
    /// field of the spec becomes a parameter.
    pub fn for_spec(spec: TypeSpec, unit: Option<&str>) -> Self {
        let mut ann = Self::new(spec.family);
        ann.width = (spec.width != 0).then_some(spec.width);
        match spec.family {
            TypeFamily::FloatFix => ann.q = (spec.base != 0).then_some(spec.base),
            TypeFamily::Datetime => {
                ann.epoch = crate::num::epoch_name(spec.base).filter(|_| spec.base != 0)
            }
            _ => ann.base = (spec.base != 0).then_some(spec.base),
        }
        ann.unit = unit.map(String::from);
        ann
    }

    /// With a width parameter.
    #[must_use]
    pub fn with_width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    /// With a base parameter (`_16`).
    #[must_use]
    pub fn with_base(mut self, base: u32) -> Self {
        self.base = Some(base);
        self
    }

    /// With a unit parameter.
    #[must_use]
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }
}

/// What a [`Value`] holds.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Kind {
    /// `null`, or an empty value (`.a = ;`, `[1, , 3]`).
    Null,
    /// `true` / `false` (`on` / `off` read as these).
    Bool(bool),
    /// Any number or datetime, kept losslessly.
    Number(Number),
    /// A UTF-8 string.
    String(String),
    /// A symbol (a bare word such as `info`).
    Symbol(String),
    /// A reference to another value (`&.a.b[0]`), without the `&`.
    Reference(String),
    /// An octet stream.
    Octets(Vec<u8>),
    /// A struct.
    Struct(Struct),
    /// An array, possibly with `/`-separated rows.
    Array(Array),
}

/// One value of a document: its content plus the type annotation written in
/// front of it, if any.
///
/// Accessors never panic; they return `None` for a value of another kind.
///
/// ```
/// use bovnar::Value;
///
/// let v = Value::from(42u64);
/// assert_eq!(v.as_u64(), Some(42));
/// assert_eq!(v.as_str(), None);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Value {
    kind: Kind,
    annotation: Option<Box<Annotation>>,
}

impl Value {
    /// A value of `kind` with no annotation.
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            annotation: None,
        }
    }

    /// `null`.
    pub fn null() -> Self {
        Self::new(Kind::Null)
    }

    /// A symbol (`'name` in a document, written bare).
    pub fn symbol(name: impl Into<String>) -> Self {
        Self::new(Kind::Symbol(name.into()))
    }

    /// A reference such as `.a.b[0]` (written as `&.a.b[0]`).
    pub fn reference(path: impl Into<String>) -> Self {
        Self::new(Kind::Reference(path.into()))
    }

    /// The same value with `annotation` written in front of it.
    ///
    /// ```
    /// use bovnar::{Annotation, TypeFamily, Value};
    ///
    /// let v = Value::from(1.5).with_annotation(Annotation::new(TypeFamily::Float).with_unit("m"));
    /// assert_eq!(v.annotation().and_then(|a| a.unit.as_deref()), Some("m"));
    /// ```
    #[must_use]
    pub fn with_annotation(mut self, annotation: Annotation) -> Self {
        self.annotation = Some(Box::new(annotation));
        self
    }

    pub(crate) fn from_parts(kind: Kind, annotation: Option<Box<Annotation>>) -> Self {
        Self { kind, annotation }
    }

    pub(crate) fn kind_mut(&mut self) -> &mut Kind {
        &mut self.kind
    }

    /// What the value holds.
    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    /// Takes the value apart.
    pub fn into_kind(self) -> Kind {
        self.kind
    }

    /// The annotation written in front of the value, if any.
    pub fn annotation(&self) -> Option<&Annotation> {
        self.annotation.as_deref()
    }

    /// Whether this is `null`.
    pub fn is_null(&self) -> bool {
        matches!(self.kind, Kind::Null)
    }

    /// The boolean, if this is one.
    pub fn as_bool(&self) -> Option<bool> {
        match self.kind {
            Kind::Bool(b) => Some(b),
            _ => None,
        }
    }

    /// The number, if this is one.
    pub fn as_number(&self) -> Option<&Number> {
        match &self.kind {
            Kind::Number(n) => Some(n),
            _ => None,
        }
    }

    /// The number as `u64`, if it is an integer that fits.
    pub fn as_u64(&self) -> Option<u64> {
        self.as_number()?.as_u64()
    }

    /// The number as `i64`, if it is an integer that fits.
    pub fn as_i64(&self) -> Option<i64> {
        self.as_number()?.as_i64()
    }

    /// The number as `f64`.
    pub fn as_f64(&self) -> Option<f64> {
        self.as_number()?.as_f64()
    }

    /// The string, if this is a string.
    pub fn as_str(&self) -> Option<&str> {
        match &self.kind {
            Kind::String(s) => Some(s),
            _ => None,
        }
    }

    /// The symbol name, if this is a symbol.
    pub fn as_symbol(&self) -> Option<&str> {
        match &self.kind {
            Kind::Symbol(s) => Some(s),
            _ => None,
        }
    }

    /// The reference path, if this is a reference.
    pub fn as_reference(&self) -> Option<&str> {
        match &self.kind {
            Kind::Reference(s) => Some(s),
            _ => None,
        }
    }

    /// The bytes, if this is an octet stream.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match &self.kind {
            Kind::Octets(b) => Some(b),
            _ => None,
        }
    }

    /// The struct, if this is one.
    pub fn as_struct(&self) -> Option<&Struct> {
        match &self.kind {
            Kind::Struct(s) => Some(s),
            _ => None,
        }
    }

    /// The struct, mutably, if this is one.
    pub fn as_struct_mut(&mut self) -> Option<&mut Struct> {
        match &mut self.kind {
            Kind::Struct(s) => Some(s),
            _ => None,
        }
    }

    /// The array, if this is one.
    pub fn as_array(&self) -> Option<&Array> {
        match &self.kind {
            Kind::Array(a) => Some(a),
            _ => None,
        }
    }

    /// The array, mutably, if this is one.
    pub fn as_array_mut(&mut self) -> Option<&mut Array> {
        match &mut self.kind {
            Kind::Array(a) => Some(a),
            _ => None,
        }
    }

    /// The struct field `key`, if this is a struct that has it.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_struct()?.get(key)
    }

    /// Element `index` of an array (counted across all rows).
    pub fn at(&self, index: usize) -> Option<&Value> {
        self.as_array()?.get(index)
    }

    /// The value at `path` below this one: `.key` steps into a struct field,
    /// `[n]` into an array element (`.servers[0].port`).
    ///
    /// ```
    /// let doc = bovnar::parse(b".s = {.list = [1, 2, 3];};")?;
    /// let s = doc.get("s").unwrap();
    /// assert_eq!(s.lookup(".list[2]").and_then(|v| v.as_u64()), Some(3));
    /// # Ok::<(), bovnar::Error>(())
    /// ```
    pub fn lookup(&self, path: &str) -> Option<&Value> {
        let mut steps = path::Steps::new(path);
        let mut current = self;
        while let Some(step) = steps.next()? {
            current = match step {
                path::Step::Key(key) => current.get(key)?,
                path::Step::Index(i) => current.at(i)?,
            };
        }
        Some(current)
    }

    /// The effective type, for a number.
    pub fn type_spec(&self) -> Option<TypeSpec> {
        Some(self.as_number()?.spec())
    }

    /// The unit as spelled in the document, for a number that has one.
    pub fn unit(&self) -> Option<&str> {
        self.as_number()?.unit()
    }
}

impl From<Number> for Value {
    /// A number value. It gets an annotation spelling out its type and unit
    /// unless the reader would synthesise exactly that type for its text.
    fn from(n: Number) -> Self {
        let annotation = (!n.is_default_typed())
            .then(|| Box::new(Annotation::for_spec(n.spec, n.unit.as_deref())));
        Self::from_parts(Kind::Number(n), annotation)
    }
}

macro_rules! value_from_number {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            fn from(v: $t) -> Self {
                Value::from(Number::from(v))
            }
        }
    )*};
}

value_from_number!(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::new(Kind::Bool(v))
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::new(Kind::String(v.into()))
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::new(Kind::String(v))
    }
}

impl From<Vec<u8>> for Value {
    /// An octet stream.
    fn from(v: Vec<u8>) -> Self {
        Self::new(Kind::Octets(v))
    }
}

impl From<&[u8]> for Value {
    /// An octet stream.
    fn from(v: &[u8]) -> Self {
        Self::new(Kind::Octets(v.to_vec()))
    }
}

impl From<Struct> for Value {
    fn from(v: Struct) -> Self {
        Self::new(Kind::Struct(v))
    }
}

impl From<Array> for Value {
    fn from(v: Array) -> Self {
        Self::new(Kind::Array(v))
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    /// `None` becomes `null`.
    fn from(v: Option<T>) -> Self {
        v.map_or_else(Value::null, Into::into)
    }
}

/// Moves every container below `values` onto one flat list so that dropping
/// a deep tree never recurses.
fn flatten_for_drop(mut stack: Vec<Value>) {
    while let Some(mut value) = stack.pop() {
        match value.kind_mut() {
            Kind::Struct(s) => stack.extend(mem::take(&mut s.entries).into_iter().map(|(_, v)| v)),
            Kind::Array(a) => stack.append(&mut a.items),
            _ => {}
        }
    }
}

/// The ordered key/value pairs of a struct.
///
/// Keys are unique in a parsed document; [`Struct::insert`] keeps them so by
/// replacing an existing key in place.
///
/// ```
/// use bovnar::{Struct, Value};
///
/// let mut s = Struct::new();
/// s.insert("host", "localhost");
/// s.insert("port", 5432u64);
/// assert_eq!(s.get("port").and_then(Value::as_u64), Some(5432));
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Struct {
    entries: Vec<(String, Value)>,
}

impl Struct {
    /// An empty struct.
    pub fn new() -> Self {
        Self::default()
    }

    /// The fields, moved out.
    #[cfg(feature = "serde")]
    pub(crate) fn into_entries(mut self) -> Vec<(String, Value)> {
        mem::take(&mut self.entries)
    }

    pub(crate) fn from_entries(entries: Vec<(String, Value)>) -> Self {
        Self { entries }
    }

    /// The field `key`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// The field `key`, mutably.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Sets field `key`, replacing an existing one in place, else appending.
    /// Returns the previous value.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Value>) -> Option<Value> {
        insert_entry(&mut self.entries, key.into(), value.into())
    }

    /// Removes field `key`.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let at = self.entries.iter().position(|(k, _)| k == key)?;
        Some(self.entries.remove(at).1)
    }

    /// The fields in document order.
    pub fn entries(&self) -> &[(String, Value)] {
        &self.entries
    }

    /// Iterates over the fields in document order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Number of fields.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the struct has no fields.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Drop for Struct {
    fn drop(&mut self) {
        if self.entries.iter().any(|(_, v)| is_container(v)) {
            flatten_for_drop(
                mem::take(&mut self.entries)
                    .into_iter()
                    .map(|(_, v)| v)
                    .collect(),
            );
        }
    }
}

impl<K: Into<String>, V: Into<Value>> FromIterator<(K, V)> for Struct {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut s = Self::new();
        for (k, v) in iter {
            s.insert(k, v);
        }
        s
    }
}

fn is_container(v: &Value) -> bool {
    matches!(v.kind, Kind::Struct(_) | Kind::Array(_))
}

fn insert_entry(entries: &mut Vec<(String, Value)>, key: String, value: Value) -> Option<Value> {
    match entries.iter_mut().find(|(k, _)| *k == key) {
        Some((_, slot)) => Some(mem::replace(slot, value)),
        None => {
            entries.push((key, value));
            None
        }
    }
}

/// An array: its elements in order, and for a `/`-separated matrix
/// (`[1, 2] / [3, 4]`) the lengths of its rows.
///
/// ```
/// use bovnar::{Array, Value};
///
/// let mut a = Array::new();
/// a.push(1u64);
/// a.push(2u64);
/// a.new_row();
/// a.push(3u64);
/// a.push(4u64);
/// assert_eq!(a.len(), 4);
/// assert_eq!(a.row_count(), 2);
/// assert_eq!(a.rows().nth(1).map(|r| r.len()), Some(2));
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Array {
    items: Vec<Value>,
    /// Row lengths of a matrix; empty for a single-row array.
    rows: Vec<usize>,
}

impl Array {
    /// An empty array.
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn from_parts(items: Vec<Value>, rows: Vec<usize>) -> Self {
        Self { items, rows }
    }

    /// Appends an element to the last row.
    pub fn push(&mut self, value: impl Into<Value>) {
        self.items.push(value.into());
        if let Some(last) = self.rows.last_mut() {
            *last += 1;
        }
    }

    /// Starts a new `/`-separated row.
    pub fn new_row(&mut self) {
        if self.rows.is_empty() {
            self.rows.push(self.items.len());
        }
        self.rows.push(0);
    }

    /// Element `index`, counted across all rows.
    pub fn get(&self, index: usize) -> Option<&Value> {
        self.items.get(index)
    }

    /// Element `index`, mutably.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut Value> {
        self.items.get_mut(index)
    }

    /// All elements, row after row.
    pub fn items(&self) -> &[Value] {
        &self.items
    }

    /// Iterates over all elements.
    pub fn iter(&self) -> core::slice::Iter<'_, Value> {
        self.items.iter()
    }

    /// Number of elements over all rows.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the array has no elements.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Number of rows: 1 unless the array is a `/`-separated matrix.
    pub fn row_count(&self) -> usize {
        self.rows.len().max(1)
    }

    /// The rows.
    pub fn rows(&self) -> impl Iterator<Item = &[Value]> {
        let single = [self.items.len()];
        let lens: &[usize] = if self.rows.is_empty() {
            &[]
        } else {
            &self.rows
        };
        let mut start = 0usize;
        let mut slices = Vec::with_capacity(self.row_count());
        for &len in lens.iter().chain(single.iter().filter(|_| lens.is_empty())) {
            let end = start.saturating_add(len).min(self.items.len());
            slices.push(self.items.get(start..end).unwrap_or_default());
            start = end;
        }
        slices.into_iter()
    }

    /// Row lengths of a matrix; empty for a single-row array.
    pub(crate) fn row_lengths(&self) -> &[usize] {
        &self.rows
    }
}

impl Drop for Array {
    fn drop(&mut self) {
        if self.items.iter().any(is_container) {
            flatten_for_drop(mem::take(&mut self.items));
        }
    }
}

impl<V: Into<Value>> FromIterator<V> for Array {
    fn from_iter<I: IntoIterator<Item = V>>(iter: I) -> Self {
        Self {
            items: iter.into_iter().map(Into::into).collect(),
            rows: Vec::new(),
        }
    }
}

impl<V: Into<Value>> From<Vec<V>> for Array {
    fn from(v: Vec<V>) -> Self {
        v.into_iter().collect()
    }
}

/// A whole document: its top-level assignments in order, and the version it
/// declares (`#!bovnar 1.1`), if any.
///
/// ```
/// use bovnar::{Document, Struct};
///
/// let mut doc = Document::new();
/// doc.insert("name", "Alice");
/// doc.insert("age", 30u64);
/// let text = bovnar::to_string(&doc)?;
/// assert_eq!(text, ".name = \"Alice\";\n.age = 30;\n");
/// assert_eq!(bovnar::parse(text.as_bytes())?.get("age").and_then(|v| v.as_u64()), Some(30));
/// # Ok::<(), bovnar::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Document {
    version: Option<SpecVersion>,
    entries: Vec<(String, Value)>,
}

impl Document {
    /// An empty document without a version directive.
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn from_parts(version: Option<SpecVersion>, entries: Vec<(String, Value)>) -> Self {
        Self { version, entries }
    }

    /// The declared version.
    pub fn version(&self) -> Option<SpecVersion> {
        self.version
    }

    /// Sets or clears the version directive written in front of the document.
    pub fn set_version(&mut self, version: Option<SpecVersion>) {
        self.version = version;
    }

    /// The top-level value `key`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// The top-level value `key`, mutably.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Sets top-level `key`, replacing an existing one in place, else
    /// appending. Returns the previous value.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<Value>) -> Option<Value> {
        insert_entry(&mut self.entries, key.into(), value.into())
    }

    /// Removes top-level `key`.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let at = self.entries.iter().position(|(k, _)| k == key)?;
        Some(self.entries.remove(at).1)
    }

    /// The value at `path` (`.servers[0].port`; the leading `.` is optional),
    /// as the C reference's `bvn_dom_lookup` resolves it.
    ///
    /// ```
    /// let doc = bovnar::parse(b".a = {.b = [10, 20];};")?;
    /// assert_eq!(doc.lookup(".a.b[1]").and_then(|v| v.as_u64()), Some(20));
    /// assert!(doc.lookup(".a.c").is_none());
    /// # Ok::<(), bovnar::Error>(())
    /// ```
    pub fn lookup(&self, path: &str) -> Option<&Value> {
        let mut steps = path::Steps::new(path);
        let first = match steps.next()?? {
            path::Step::Key(key) => self.get(key)?,
            path::Step::Index(_) => return None,
        };
        let rest = steps.rest();
        if rest.is_empty() {
            Some(first)
        } else {
            first.lookup(rest)
        }
    }

    /// The top-level assignments in order.
    pub fn entries(&self) -> &[(String, Value)] {
        &self.entries
    }

    /// Iterates over the top-level assignments in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Number of top-level assignments.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the document has no assignments.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Drop for Document {
    fn drop(&mut self) {
        if self.entries.iter().any(|(_, v)| is_container(v)) {
            flatten_for_drop(
                mem::take(&mut self.entries)
                    .into_iter()
                    .map(|(_, v)| v)
                    .collect(),
            );
        }
    }
}

impl<K: Into<String>, V: Into<Value>> FromIterator<(K, V)> for Document {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut d = Self::new();
        for (k, v) in iter {
            d.insert(k, v);
        }
        d
    }
}
