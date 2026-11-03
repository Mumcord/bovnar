//! Document-tier structural rules (spec §7.4 homogeneity, §8.1 unique keys),
//! a port of `bvn_dom_check_homogeneous` and `bvn_dom_shape_merge` from the
//! C reference with an explicit work stack instead of recursion.
//!
//! An array's elements are held to a *shape* they build between them: one
//! slot per position (a scalar, the element of an array, each field of a
//! struct), empty until some element has a non-null value there. A null is a
//! hole that neither fills a slot nor is tested against one, so the verdict
//! does not depend on the order of the elements.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::units::{self, Unit};
use crate::value::{Document, Kind, Value};
use crate::{ErrorKind, SpecVersion};

/// Only the first rows of a matrix take part in shape comparison, as in the
/// C reference (`rows_per_dim[8]`).
const RECORDED_ROWS: usize = 8;

/// One slot of a shape.
#[derive(Clone, Copy, Default)]
struct Slot<'a> {
    /// The first non-null value seen at this position.
    rep: Option<&'a Value>,
    /// Index of the first sub-slot in the arena.
    sub: usize,
    /// Number of sub-slots (1 for an array, one per field for a struct).
    nsub: usize,
}

/// A pending piece of work, executed in the order the recursive C code
/// would perform it.
enum Task<'a> {
    /// `bvn_dom_check_homogeneous`.
    Check(&'a Value),
    /// `bvn_dom_shape_merge`.
    Merge {
        slot: usize,
        value: &'a Value,
        check_dim: bool,
        internal: bool,
    },
}

/// Applies the rules to a whole document: unique top-level keys, then every
/// value in order.
pub(super) fn document(doc: &Document, version: SpecVersion) -> Result<(), ErrorKind> {
    keys_unique(doc.entries().iter().map(|(k, _)| k.as_str()))?;
    let mut checker = Checker {
        slots: Vec::new(),
        tasks: Vec::new(),
        units: UnitCache::new(version),
    };
    for (_, value) in doc.entries() {
        checker.tasks.push(Task::Check(value));
        checker.run()?;
    }
    Ok(())
}

struct Checker<'a> {
    slots: Vec<Slot<'a>>,
    tasks: Vec<Task<'a>>,
    units: UnitCache,
}

impl<'a> Checker<'a> {
    fn run(&mut self) -> Result<(), ErrorKind> {
        while let Some(task) = self.tasks.pop() {
            match task {
                Task::Check(value) => self.check(value)?,
                Task::Merge {
                    slot,
                    value,
                    check_dim,
                    internal,
                } => self.merge(slot, value, check_dim, internal)?,
            }
        }
        // Shapes live only as long as one top-level check.
        self.slots.clear();
        Ok(())
    }

    fn check(&mut self, value: &'a Value) -> Result<(), ErrorKind> {
        match value.kind() {
            Kind::Array(array) => {
                // Elements of a bare array: units are enforced, against a
                // fresh shape.
                let slot = self.new_slots(1);
                for item in array.items().iter().rev() {
                    self.tasks.push(Task::Merge {
                        slot,
                        value: item,
                        check_dim: true,
                        internal: true,
                    });
                }
            }
            Kind::Struct(s) => {
                keys_unique(s.entries().iter().map(|(k, _)| k.as_str()))?;
                for (_, field) in s.entries().iter().rev() {
                    self.tasks.push(Task::Check(field));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Holds `value` to the shape in `slot`, filling in what the slot does
    /// not know yet. `check_dim` is false below a struct field ("shape
    /// uniform, fields free": a field may change unit and list length from
    /// record to record). `internal` says the value still needs its own
    /// checks (unique keys, homogeneous lists inside one record).
    fn merge(
        &mut self,
        slot: usize,
        value: &'a Value,
        check_dim: bool,
        internal: bool,
    ) -> Result<(), ErrorKind> {
        if value.is_null() {
            return Ok(());
        }
        let current = self.slots.get(slot).copied().unwrap_or_default();
        let (sub, nsub) = match current.rep {
            None => {
                let want = match value.kind() {
                    Kind::Array(_) => 1,
                    Kind::Struct(s) => s.len(),
                    _ => 0,
                };
                let sub = self.new_slots(want);
                if let Some(s) = self.slots.get_mut(slot) {
                    *s = Slot {
                        rep: Some(value),
                        sub,
                        nsub: want,
                    };
                }
                (sub, want)
            }
            Some(rep) => {
                self.compare(rep, value, check_dim)?;
                (current.sub, current.nsub)
            }
        };
        match value.kind() {
            Kind::Array(array) => {
                for item in array.items().iter().rev() {
                    self.tasks.push(Task::Merge {
                        slot: sub,
                        value: item,
                        check_dim,
                        internal,
                    });
                }
            }
            Kind::Struct(s) => {
                if internal {
                    keys_unique(s.entries().iter().map(|(k, _)| k.as_str()))?;
                }
                let fields = s.entries().iter().take(nsub).enumerate().rev();
                for (i, (_, field)) in fields {
                    self.tasks.push(Task::Merge {
                        slot: sub + i,
                        value: field,
                        check_dim: false,
                        internal: false,
                    });
                    if internal {
                        self.tasks.push(Task::Check(field));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Compares `value` with the slot's representative `rep` (both non-null).
    fn compare(&mut self, rep: &Value, value: &Value, check_dim: bool) -> Result<(), ErrorKind> {
        match (rep.kind(), value.kind()) {
            (Kind::Number(a), Kind::Number(b)) => {
                // Numeric encodings mix freely; a datetime is its own kind,
                // and its epoch is a dimension like a unit.
                let (a_dt, b_dt) = (a.is_datetime(), b.is_datetime());
                if a_dt != b_dt {
                    return Err(ErrorKind::ArrayElementTypeMismatch);
                }
                if check_dim {
                    if a_dt && a.spec().base != b.spec().base {
                        return Err(ErrorKind::ArrayElementTypeMismatch);
                    }
                    if !self.units.same(a.unit(), b.unit())? {
                        return Err(ErrorKind::ArrayElementTypeMismatch);
                    }
                }
                Ok(())
            }
            (Kind::Array(a), Kind::Array(b)) => {
                if check_dim {
                    // Rectangular only in the bare array / matrix context;
                    // the row geometry must match too, not just the count.
                    if a.len() != b.len()
                        || dims(a.row_lengths(), a.len()) != dims(b.row_lengths(), b.len())
                    {
                        return Err(ErrorKind::ArrayRowSizeMismatch);
                    }
                }
                Ok(())
            }
            (Kind::Struct(a), Kind::Struct(b)) => {
                let same = a.len() == b.len()
                    && a.entries().iter().zip(b.entries()).all(|(x, y)| x.0 == y.0);
                if same {
                    Ok(())
                } else {
                    Err(ErrorKind::StructShapeMismatch)
                }
            }
            (a, b) if core::mem::discriminant(a) == core::mem::discriminant(b) => Ok(()),
            _ => Err(ErrorKind::ArrayElementTypeMismatch),
        }
    }

    fn new_slots(&mut self, n: usize) -> usize {
        let at = self.slots.len();
        self.slots
            .extend(core::iter::repeat(Slot::default()).take(n));
        at
    }
}

/// The recorded geometry of an array: its row count and the widths of its
/// first rows, as the C reference keeps them.
fn dims(rows: &[usize], len: usize) -> (usize, [usize; RECORDED_ROWS]) {
    let mut widths = [0; RECORDED_ROWS];
    if rows.is_empty() {
        widths[0] = len;
        return (1, widths);
    }
    for (w, r) in widths.iter_mut().zip(rows) {
        *w = *r;
    }
    (rows.len(), widths)
}

/// No key may repeat within one scope; empty keys are not compared.
fn keys_unique<'k>(keys: impl Iterator<Item = &'k str>) -> Result<(), ErrorKind> {
    let mut seen = BTreeSet::new();
    for key in keys.filter(|k| !k.is_empty()) {
        if !seen.insert(key) {
            return Err(ErrorKind::DuplicateStructKey);
        }
    }
    Ok(())
}

/// Parses unit spellings for comparison, remembering the last two.
struct UnitCache {
    version: SpecVersion,
    last: Option<(alloc::string::String, Unit)>,
}

impl UnitCache {
    fn new(version: SpecVersion) -> Self {
        Self {
            version,
            last: None,
        }
    }

    fn parse(&mut self, text: Option<&str>) -> Result<Unit, ErrorKind> {
        let Some(text) = text else {
            return Ok(Unit::no_unit());
        };
        if let Some((t, u)) = &self.last {
            if t == text {
                return Ok(u.clone());
            }
        }
        let unit = units::parse_unit(text.as_bytes(), self.version)?;
        self.last = Some((text.into(), unit.clone()));
        Ok(unit)
    }

    /// The same unit for a bare array: equal as units, or both
    /// dimensionless (an omitted unit and an explicit `no_unit`).
    fn same(&mut self, a: Option<&str>, b: Option<&str>) -> Result<bool, ErrorKind> {
        if a == b {
            return Ok(true);
        }
        let (ua, ub) = (self.parse(a)?, self.parse(b)?);
        Ok((ua.is_no_unit() && ub.is_no_unit()) || ua == ub)
    }
}
