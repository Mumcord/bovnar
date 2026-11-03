//! The unit profiles (spec 1.2, doc/11; `bovnar_profiles.c`): `ns:code` is an
//! alternative spelling of a unit, translated into an ordinary [`Unit`].
//!
//! A profile expression either becomes a real unit or an error, never a unit
//! the rest of the library cannot reason about. The refusals split by cause:
//! `UnitIllegal` (not a code of the vocabulary, or malformed),
//! `UnitProfileUnsupported` (a valid code Bovnar cannot carry) and
//! `UnitProfileUnknown` (no such profile in this build).
//!
//! Each shipped profile sits behind its cargo feature. UCUM and CF are never
//! registered, so their namespaces are refused with `UnitProfileUnknown`
//! exactly as a C build with those profiles switched off refuses them; the
//! UCUM-only syntax (annotations, binary prefixes) is therefore not carried.
//!
//! Translation is decade-based: a profile prefix, a `10^n` factor, and a
//! scale the target cannot carry accumulate as one power of ten that is
//! finally folded into a single component's SI prefix.

// A build with only some of the profiles leaves parts of the shared
// machinery (the expression grammar, opaque units) without a user.
#![cfg_attr(
    not(all(
        feature = "profile-unece",
        feature = "profile-qudt",
        feature = "profile-qudt-qk",
        feature = "profile-udunits",
        feature = "profile-om"
    )),
    allow(dead_code, unused_macros)
)]

use alloc::string::String;

use super::{
    data, parse, BaseUnit, Component, Prefix, PrefixSystem, Unit, EXPONENT_MAX, EXPONENT_MIN,
};
use crate::ErrorKind;

/// Deepest group nesting of an expression profile (`BVN_PROFILE_MAX_DEPTH`).
const MAX_DEPTH: usize = 16;

/// Longest `<prefix><code>` the writer assembles (`BVN_PROF_WRITE_MAX`).
const WRITE_MAX: usize = 256;

/// A prefix spelling of an expression profile and its power of ten.
pub(crate) struct ProfilePrefix {
    pub(crate) code: &'static str,
    pub(crate) decade: i32,
}

/// What a code translates to.
pub(crate) enum AtomTarget {
    /// A native unit expression, parsed by the native grammar at use.
    Native(&'static str),
    /// An opaque unit of this profile, by base unit id.
    Opaque(u32),
}

/// One code of a profile's vocabulary.
pub(crate) struct Atom {
    pub(crate) code: &'static str,
    pub(crate) target: AtomTarget,
    /// Whether a profile prefix may attach (always false in a flat profile).
    pub(crate) metric: bool,
}

/// One row of the writer's table: the code that spells `base` and the
/// decade (and binary prefix, 0 for none) that code already carries.
pub(crate) struct Reverse {
    pub(crate) base: u32,
    pub(crate) code: &'static str,
    pub(crate) decade: i32,
    pub(crate) metric: bool,
    pub(crate) iec: u8,
}

/// How a code is cut into atoms.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Grammar {
    /// Operators, exponents and grouping over prefixed atoms (UDUNITS).
    Expr,
    /// One whole token, looked up entire (UNECE, QUDT, OM).
    Flat,
}

/// One unit profile.
pub(crate) struct Profile {
    /// The namespace before the `:`, e.g. `"udunits"`.
    pub(crate) namespace: &'static str,
    grammar: Grammar,
    prefixes: &'static [ProfilePrefix],
    /// Sorted by bytes.
    atoms: &'static [Atom],
    /// Sorted by bytes.
    unsupported: &'static [&'static str],
    reverse: &'static [Reverse],
    opaque_first: u32,
    /// (C enumerator suffix, code) per opaque unit, in id order.
    opaque: &'static [(&'static str, &'static str)],
    /// Multiplication bytes beside `.`.
    mul: &'static [u8],
    /// Whether `m^2` is an exponent as well as `m2`.
    caret_exp: bool,
    /// A substring that refuses a code outright as unsupported (UDUNITS'
    /// reference time, "since").
    refuse: Option<&'static [u8]>,
}

macro_rules! flat_profile {
    ($ns:literal, $table:ident) => {
        Profile {
            namespace: $ns,
            grammar: Grammar::Flat,
            prefixes: &super::gen::$table::PREFIXES,
            atoms: &super::gen::$table::ATOMS,
            unsupported: &super::gen::$table::UNSUPPORTED,
            reverse: &super::gen::$table::REVERSE,
            opaque_first: super::gen::$table::OPAQUE_FIRST,
            opaque: &super::gen::$table::OPAQUE,
            mul: b"",
            caret_exp: false,
            refuse: None,
        }
    };
}

/// The profiles this build carries, in the C registry order.
pub(crate) static PROFILES: &[Profile] = &[
    #[cfg(feature = "profile-unece")]
    flat_profile!("unece", profile_unece),
    #[cfg(feature = "profile-qudt")]
    flat_profile!("qudt", profile_qudt),
    #[cfg(feature = "profile-qudt-qk")]
    flat_profile!("qudt-qk", profile_qudt_qk),
    // UDUNITS multiplies with '*' and a space as well as '.'; a space only
    // reaches the parser from inside a namespaced type parameter.
    #[cfg(feature = "profile-udunits")]
    Profile {
        namespace: "udunits",
        grammar: Grammar::Expr,
        prefixes: &super::gen::profile_udunits::PREFIXES,
        atoms: &super::gen::profile_udunits::ATOMS,
        unsupported: &super::gen::profile_udunits::UNSUPPORTED,
        reverse: &super::gen::profile_udunits::REVERSE,
        opaque_first: super::gen::profile_udunits::OPAQUE_FIRST,
        opaque: &super::gen::profile_udunits::OPAQUE,
        mul: b"* ",
        caret_exp: true,
        refuse: Some(b"since"),
    },
    #[cfg(feature = "profile-om")]
    flat_profile!("om", profile_om),
];

impl Profile {
    fn find_atom(&self, code: &[u8]) -> Option<&'static Atom> {
        let i = self
            .atoms
            .binary_search_by(|a| a.code.as_bytes().cmp(code))
            .ok()?;
        self.atoms.get(i)
    }

    fn is_unsupported(&self, code: &[u8]) -> bool {
        self.unsupported
            .binary_search_by(|c| c.as_bytes().cmp(code))
            .is_ok()
    }

    fn owns_opaque(&self, id: u32) -> bool {
        self.opaque_row(id).is_some()
    }

    fn opaque_row(&self, id: u32) -> Option<&'static (&'static str, &'static str)> {
        let slot = id.checked_sub(self.opaque_first)?;
        self.opaque.get(usize::try_from(slot).ok()?)
    }

    fn is_mul(&self, b: u8) -> bool {
        b == b'.' || self.mul.contains(&b)
    }

    /// The prefixes that head `s` with something left over, longest first.
    fn heads<'a>(
        &'a self,
        s: &'a [u8],
    ) -> impl Iterator<Item = (&'static ProfilePrefix, &'a [u8])> {
        self.prefixes.iter().filter_map(move |f| {
            let rest = s.strip_prefix(f.code.as_bytes())?;
            (!rest.is_empty()).then_some((f, rest))
        })
    }

    /// `resolve_prefixed`: prefix + metric atom, trying every prefix that
    /// heads `s` (prefixes are not suffix-free).
    fn resolve_prefixed(&self, s: &[u8]) -> Option<(&'static Atom, i32)> {
        self.heads(s).find_map(|(f, rest)| {
            let atom = self.find_atom(rest)?;
            atom.metric.then_some((atom, f.decade))
        })
    }

    fn resolve_prefixed_unsupported(&self, s: &[u8]) -> bool {
        self.heads(s).any(|(_, rest)| self.is_unsupported(rest))
    }
}

/// The owning profile and row of an opaque unit id.
pub(crate) fn opaque_row(id: u32) -> Option<(&'static Profile, &'static str, &'static str)> {
    PROFILES
        .iter()
        .find_map(|p| p.opaque_row(id).map(|&(name, code)| (p, name, code)))
}

/// Whether a base unit id is an opaque profile unit (`bvni_is_opaque`).
pub(crate) fn is_opaque(id: u32) -> bool {
    opaque_row(id).is_some()
}

/// The namespaces this build carries, in registry order.
pub(crate) fn namespaces() -> impl Iterator<Item = &'static str> {
    PROFILES.iter().map(|p| p.namespace)
}

/// Whether a unit string uses the profile notation (`bvni_unit_has_profile`):
/// a non-empty run of lowercase letters, digits and non-leading `-` followed
/// by `:`. No native spelling contains a `:`, so this never shadows one.
pub(crate) fn has_namespace(s: &[u8]) -> bool {
    for (i, &b) in s.iter().enumerate() {
        match b {
            b':' => return i > 0,
            b'a'..=b'z' | b'0'..=b'9' => {}
            b'-' if i > 0 => {}
            _ => return false,
        }
    }
    false
}

/// Why a profile code was refused (`bvni_profile_status_t`).
#[derive(Clone, Copy)]
enum Refusal {
    Illegal,
    Unsupported,
}

impl From<Refusal> for ErrorKind {
    fn from(r: Refusal) -> Self {
        match r {
            Refusal::Illegal => ErrorKind::UnitIllegal,
            Refusal::Unsupported => ErrorKind::UnitProfileUnsupported,
        }
    }
}

/// A partially built unit and the decade nothing has absorbed yet.
#[derive(Clone)]
struct Acc {
    unit: Unit,
    decade: i32,
}

impl Acc {
    fn new() -> Self {
        Self {
            unit: Unit::no_unit(),
            decade: 0,
        }
    }

    fn push(&mut self, c: Component) -> Result<(), Refusal> {
        if self.unit.push(c) {
            Ok(())
        } else {
            Err(Refusal::Unsupported)
        }
    }

    fn add_decade(&mut self, d: i32) {
        self.decade = self.decade.saturating_add(d);
    }
}

/// Parses a unit in profile notation (`bvni_parse_profile_unit`).
pub(crate) fn parse(s: &[u8]) -> Result<Unit, ErrorKind> {
    let colon = s
        .iter()
        .position(|&b| b == b':')
        .ok_or(ErrorKind::UnitIllegal)?;
    let (Some(namespace), Some(code)) = (s.get(..colon), s.get(colon + 1..)) else {
        return Err(ErrorKind::UnitIllegal);
    };
    let profile = PROFILES
        .iter()
        .find(|p| p.namespace.as_bytes() == namespace)
        .ok_or(ErrorKind::UnitProfileUnknown)?;
    if code.is_empty() {
        return Err(ErrorKind::UnitIllegal);
    }
    if let Some(needle) = profile.refuse {
        if code.windows(needle.len()).any(|w| w == needle) {
            return Err(ErrorKind::UnitProfileUnsupported);
        }
    }
    let mut acc = match profile.grammar {
        Grammar::Flat => parse_flat(profile, code)?,
        Grammar::Expr => parse_expr(profile, code)?,
    };
    // A residual scale with no prefix to put it in (there is none for 10^4,
    // and none at all without a component).
    let mut decade = acc.decade;
    let len = acc.unit.components().len();
    if !fold_decade(&mut acc.unit, &mut decade, 0, len) {
        return Err(ErrorKind::UnitProfileUnsupported);
    }
    // The translated unit must satisfy every rule a natively written one does.
    if !acc
        .unit
        .components()
        .iter()
        .all(|c| parse::prefix_allowed(c.prefix, c.base))
    {
        return Err(ErrorKind::UnitProfileUnsupported);
    }
    Ok(acc.unit)
}

/// A flat profile's whole code, matched entire: no prefix is stripped and no
/// operator recognised, so "KGM" never reads as a kilo-GM.
fn parse_flat(p: &Profile, code: &[u8]) -> Result<Acc, Refusal> {
    let mut acc = Acc::new();
    match p.find_atom(code) {
        Some(atom) => materialise(atom, 1, 0, &mut acc)?,
        None if p.is_unsupported(code) => return Err(Refusal::Unsupported),
        None => return Err(Refusal::Illegal),
    }
    Ok(acc)
}

/// One nesting level of [`parse_expr`]: the C recursion, flattened.
#[derive(Clone)]
struct Level {
    /// End of this level's text (exclusive).
    end: usize,
    /// Where the current term starts / the scan stands.
    i: usize,
    /// +1 in front of a multiplication, -1 after a `/`.
    sign: i32,
    acc: Acc,
}

/// `parse_expr`: `[/] term { op term }` where `/` inverts only the term
/// after it and a term is a component or a parenthesised group. Groups are
/// walked with an explicit bounded stack; each level keeps its own
/// accumulator, so errors surface in the same order as in C.
fn parse_expr(p: &Profile, s: &[u8]) -> Result<Acc, Refusal> {
    let mut levels: [Level; MAX_DEPTH + 1] = core::array::from_fn(|_| Level {
        end: 0,
        i: 0,
        sign: 1,
        acc: Acc::new(),
    });
    let mut depth = 0usize;
    open_level(levels.get_mut(0).ok_or(Refusal::Illegal)?, s, 0, s.len())?;
    loop {
        // At the start of a term.
        let level = levels.get_mut(depth).ok_or(Refusal::Illegal)?;
        let i = level.i;
        if i >= level.end {
            return Err(Refusal::Illegal); // trailing operator
        }
        if s.get(i) == Some(&b'(') {
            let close = matching_paren(s, i, level.end).ok_or(Refusal::Illegal)?;
            if depth + 1 > MAX_DEPTH {
                return Err(Refusal::Illegal);
            }
            depth += 1;
            let inner = levels.get_mut(depth).ok_or(Refusal::Illegal)?;
            open_level(inner, s, i + 1, close)?;
            continue;
        }
        let start = i;
        let mut j = i;
        let mut brackets = 0usize;
        while j < level.end {
            let Some(&b) = s.get(j) else { break };
            match b {
                b'[' => brackets += 1,
                b']' if brackets == 0 => break,
                b']' => brackets -= 1,
                _ if brackets == 0 && (p.is_mul(b) || b == b'/' || b == b'(') => break,
                _ => {}
            }
            j += 1;
        }
        if brackets != 0 {
            return Err(Refusal::Illegal); // unbalanced '['
        }
        let term = s.get(start..j).ok_or(Refusal::Illegal)?;
        parse_component(p, term, level.sign, &mut level.acc)?;
        level.i = j;
        // After a term: close every level that ends here, then expect an
        // operator.
        loop {
            let level = levels.get_mut(depth).ok_or(Refusal::Illegal)?;
            if level.i < level.end {
                let op = s.get(level.i).copied().ok_or(Refusal::Illegal)?;
                level.sign = if p.is_mul(op) {
                    1
                } else if op == b'/' {
                    -1
                } else {
                    return Err(Refusal::Illegal); // e.g. implicit multiplication
                };
                level.i += 1;
                break;
            }
            if depth == 0 {
                return Ok(level.acc.clone());
            }
            let (done, close) = (level.acc.clone(), level.end);
            depth -= 1;
            let parent = levels.get_mut(depth).ok_or(Refusal::Illegal)?;
            for c in done.unit.components() {
                let mut c = *c;
                if parent.sign < 0 {
                    c.exponent = -c.exponent;
                }
                parent.acc.push(c)?;
            }
            parent
                .acc
                .add_decade(parent.sign.saturating_mul(done.decade));
            parent.i = close + 1;
        }
    }
}

/// Starts a level over `s[start..end]`, consuming a leading reciprocal `/`.
fn open_level(level: &mut Level, s: &[u8], start: usize, end: usize) -> Result<(), Refusal> {
    if start >= end {
        return Err(Refusal::Illegal);
    }
    *level = Level {
        end,
        i: start,
        sign: 1,
        acc: Acc::new(),
    };
    if s.get(start) == Some(&b'/') {
        level.sign = -1;
        level.i += 1;
        if level.i >= end {
            return Err(Refusal::Illegal);
        }
    }
    Ok(())
}

/// The `)` closing the `(` at `open`, before `end`.
fn matching_paren(s: &[u8], open: usize, end: usize) -> Option<usize> {
    let mut depth = 1usize;
    for (offset, &b) in s.get(open + 1..end)?.iter().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return Some(open + 1 + offset);
        }
    }
    None
}

/// Reads the digits of `s` into a value, refusing one past three digits'
/// worth (the C bound that keeps the accumulator from overflowing).
fn bounded_exponent(digits: &[u8]) -> Result<i32, Refusal> {
    let mut v = 0i32;
    for &d in digits {
        if v > 99 {
            return Err(Refusal::Unsupported);
        }
        v = v * 10 + i32::from(d - b'0');
    }
    Ok(v)
}

fn in_exponent_range(e: i32) -> bool {
    e != 0 && (EXPONENT_MIN..=EXPONENT_MAX).contains(&e)
}

/// One term of an expression (`parse_component`): a scale factor (`10^3`,
/// a bare power of ten) or an atom, optionally prefixed and raised to an
/// exponent. `sign` is -1 right after a `/`.
fn parse_component(p: &Profile, s: &[u8], sign: i32, acc: &mut Acc) -> Result<(), Refusal> {
    let Some(&first) = s.first() else {
        return Err(Refusal::Illegal);
    };
    // "10*n" / "10^n": a decade and no component.
    if let [b'1', b'0', b'*' | b'^', rest @ ..] = s {
        let (neg, digits) = match rest {
            [b'-', d @ ..] => (true, d),
            [b'+', d @ ..] => (false, d),
            d => (false, d),
        };
        if digits.is_empty() {
            // Known, and Bovnar has no component for a numeric factor.
            return Err(Refusal::Unsupported);
        }
        let mut v = 0i32;
        for &d in digits {
            if !d.is_ascii_digit() {
                return Err(Refusal::Illegal);
            }
            v = v * 10 + i32::from(d - b'0');
            if v > 400 {
                return Err(Refusal::Unsupported);
            }
        }
        acc.add_decade(sign * if neg { -v } else { v });
        return Ok(());
    }
    if first.is_ascii_digit() {
        // Only a power of ten has a representation.
        let (b'1', zeros) = (first, s.get(1..).unwrap_or_default()) else {
            return Err(Refusal::Unsupported);
        };
        if !zeros.iter().all(|&b| b == b'0') {
            return Err(Refusal::Unsupported);
        }
        acc.add_decade(sign.saturating_mul(i32::try_from(zeros.len()).unwrap_or(i32::MAX)));
        return Ok(());
    }

    let len = s.len();
    let digit_run_start = |end: usize| {
        let mut k = end;
        while k > 0 && s.get(k - 1).is_some_and(u8::is_ascii_digit) {
            k -= 1;
        }
        k
    };
    let is_sign = |k: usize| matches!(s.get(k), Some(b'-' | b'+'));
    let mut atom_len = len;
    let mut exp = 1i32;
    let mut caret = false;
    if p.caret_exp {
        let mut k = digit_run_start(len);
        if k > 0 && k < len && is_sign(k - 1) {
            k -= 1;
        }
        if k > 1 && k < len && s.get(k - 1) == Some(&b'^') {
            let (neg, j) = match s.get(k) {
                Some(b'-') => (true, k + 1),
                Some(b'+') => (false, k + 1),
                _ => (false, k),
            };
            if j >= len {
                return Err(Refusal::Illegal);
            }
            let v = bounded_exponent(s.get(j..).unwrap_or_default())?;
            exp = if neg { -v } else { v };
            atom_len = k - 1;
            if !in_exponent_range(exp) {
                return Err(Refusal::Unsupported);
            }
            caret = true;
        }
    }
    if !caret {
        let d = digit_run_start(len);
        if d < len {
            let mut ds = d;
            if ds > 0 && is_sign(ds - 1) {
                ds -= 1;
            }
            if ds == 0 {
                return Err(Refusal::Illegal);
            }
            let v = bounded_exponent(s.get(d..).unwrap_or_default())?;
            exp = if s.get(ds) == Some(&b'-') { -v } else { v };
            atom_len = ds;
        }
        if !in_exponent_range(exp) {
            return Err(Refusal::Unsupported);
        }
    }

    // The whole atom first: a prefix must never win over a complete atom
    // ("min" is the minute, not milli-inch).
    let name = s.get(..atom_len).unwrap_or_default();
    let (atom, pdec) = match p.find_atom(name) {
        Some(atom) => (atom, 0),
        None => match p.resolve_prefixed(name) {
            Some(found) => found,
            None if p.is_unsupported(name) || p.resolve_prefixed_unsupported(name) => {
                return Err(Refusal::Unsupported)
            }
            None => return Err(Refusal::Illegal),
        },
    };
    materialise(atom, sign * exp, pdec, acc)
}

/// Puts a resolved atom, raised to `total` and scaled by its stripped
/// prefix decade `pdec`, onto the accumulator (`materialise_atom`).
fn materialise(atom: &Atom, total: i32, pdec: i32, acc: &mut Acc) -> Result<(), Refusal> {
    let base = match atom.target {
        // A target that does not parse is a data defect, reported as C does.
        AtomTarget::Native(target) => {
            parse::parse_expression(target.as_bytes()).ok_or(Refusal::Unsupported)?
        }
        AtomTarget::Opaque(id) => {
            let mut u = Unit::no_unit();
            u.push(Component {
                base: BaseUnit(id),
                prefix: None,
                exponent: 1,
            });
            u
        }
    };
    let first = acc.unit.components().len();
    for c in base.components() {
        let e = c.exponent().saturating_mul(total);
        if !in_exponent_range(e) {
            return Err(Refusal::Unsupported);
        }
        acc.push(Component {
            exponent: i8::try_from(e).map_err(|_| Refusal::Unsupported)?,
            ..*c
        })?;
    }
    // Discharge the prefix into this atom's own components when it fits, so
    // the result reads like the code; otherwise leave it to the final fold.
    let mut local = pdec * total;
    let end = acc.unit.components().len();
    if local != 0 && !fold_decade(&mut acc.unit, &mut local, first, end) {
        acc.add_decade(local);
    }
    Ok(())
}

/// The SI prefix with decade `d`; `Ok(None)` is decade 0 (no prefix).
fn si_prefix_for_decade(d: i32) -> Option<Option<Prefix>> {
    if d == 0 {
        return Some(None);
    }
    (1..=u8::MAX)
        .map(|id| Prefix {
            system: PrefixSystem::Si,
            id,
        })
        .take_while(|p| data::prefix_row(PrefixSystem::Si, p.id).is_some())
        .find(|p| p.exponent() == d)
        .map(Some)
}

/// The decade of a component's SI prefix (0 without one).
fn si_decade_of(prefix: Option<Prefix>) -> i32 {
    prefix.map_or(0, Prefix::exponent)
}

/// `fold_decade_range`: moves `decade` into the SI prefix of the leftmost
/// component in `from..to` that can carry it. A component with exponent `e`
/// absorbs `D` by moving its prefix by `D / e`, which is what keeps a
/// denominator right (`10*3/uL` is n~L⁻¹).
fn fold_decade(unit: &mut Unit, decade: &mut i32, from: usize, to: usize) -> bool {
    if *decade == 0 {
        return true;
    }
    let Some(components) = unit.components.get_mut(from..to) else {
        return false;
    };
    for c in components {
        // A decimal decade has no business in a binary prefix.
        if c.prefix.is_some_and(|p| p.system == PrefixSystem::Iec) {
            continue;
        }
        let e = i32::from(c.exponent);
        if decade.checked_rem(e) != Some(0) {
            continue;
        }
        let Some(shift) = decade.checked_div(e) else {
            continue;
        };
        let Some(prefix) = si_prefix_for_decade(si_decade_of(c.prefix).saturating_add(shift))
        else {
            continue;
        };
        if !parse::prefix_allowed(prefix, c.base) {
            continue;
        }
        c.prefix = prefix;
        *decade = 0;
        return true;
    }
    false
}

/// The profile spelling of a unit with an opaque component, `<ns>:<code>`
/// (`bvni_unit_profile_spelling`); `None` when it has none.
pub(crate) fn opaque_spelling(u: &Unit) -> Option<String> {
    let mut owner: Option<&Profile> = None;
    for c in u.components() {
        if let Some((p, _, _)) = opaque_row(c.base.0) {
            if owner.is_some_and(|o| !core::ptr::eq(o, p)) {
                return None;
            }
            owner = Some(p);
        }
    }
    let owner = owner?;
    let mut out = String::from(owner.namespace);
    out.push(':');
    out.push_str(&to_code(owner, u)?);
    Some(out)
}

/// Writes `u` as a code of the profile `namespace` (`bvn_unit_to_profile`).
pub(crate) fn to_profile(namespace: &str, u: &Unit) -> Option<String> {
    let p = PROFILES.iter().find(|p| p.namespace == namespace)?;
    to_code(p, u)
}

/// `unit_to_profile_code`: a flat profile writes one unprefixed-or-coded
/// component at exponent 1; an expression profile joins components with `.`
/// and bare exponents. The dimensionless unit is `1` in an expression
/// profile that can write at all.
fn to_code(p: &Profile, u: &Unit) -> Option<String> {
    if !u
        .components()
        .iter()
        .all(|c| parse::prefix_allowed(c.prefix, c.base))
    {
        return None;
    }
    if u.is_no_unit() {
        let unity = p.grammar == Grammar::Expr && !p.reverse.is_empty();
        return unity.then(|| String::from("1"));
    }
    if p.grammar == Grammar::Flat && u.components().len() != 1 {
        return None;
    }
    let mut out = String::new();
    for (i, c) in u.components().iter().enumerate() {
        let (pfx, code) = write_atom(p, c.base.0, c.prefix)?;
        let e = c.exponent();
        if p.grammar == Grammar::Flat && e != 1 {
            return None;
        }
        if i > 0 {
            out.push('.');
        }
        out.push_str(pfx);
        out.push_str(code);
        if e != 1 {
            out.push_str(itoa(e).as_str());
        }
    }
    Some(out)
}

fn itoa(e: i32) -> String {
    use core::fmt::Write;
    let mut s = String::new();
    let _ = write!(s, "{e}");
    s
}

/// `code_reads_back`: whether `<pfx><code>` resolves, by the parser's own
/// order, to exactly `want` at decade `want_decade`. A concatenation that is
/// itself another atom ("kt", the knot) must not be written.
fn reads_back(p: &Profile, pfx: &str, code: &str, want: &Atom, want_decade: i32) -> bool {
    if pfx.len() + code.len() > WRITE_MAX {
        return false;
    }
    let mut joined = [0u8; WRITE_MAX];
    let Some(dest) = joined.get_mut(..pfx.len() + code.len()) else {
        return false;
    };
    let (a, b) = dest.split_at_mut(pfx.len());
    a.copy_from_slice(pfx.as_bytes());
    b.copy_from_slice(code.as_bytes());
    let got = match p.find_atom(dest) {
        Some(atom) => Some((atom, 0)),
        None => p.resolve_prefixed(dest),
    };
    got.is_some_and(|(atom, d)| core::ptr::eq(atom, want) && d == want_decade)
}

/// `prefix_that_reads_back`: the shortest prefix spelling of decade `d`
/// (ties by bytes) whose concatenation with `code` reads back as `want`.
/// A spelling that would resolve to another atom is skipped, not emitted.
fn prefix_that_reads_back(p: &Profile, d: i32, code: &str, want: &Atom) -> Option<&'static str> {
    if d == 0 {
        return reads_back(p, "", code, want, 0).then_some("");
    }
    let order = |f: &ProfilePrefix| (f.code.len(), f.code.as_bytes());
    let mut tried: Option<&ProfilePrefix> = None;
    loop {
        // The next spelling of this decade after the one already tried.
        let next = p
            .prefixes
            .iter()
            .filter(|f| f.decade == d && tried.map_or(true, |t| order(f) > order(t)))
            .min_by(|a, b| order(a).cmp(&order(b)))?;
        if reads_back(p, next.code, code, want, d) {
            return Some(next.code);
        }
        tried = Some(next);
    }
}

/// `write_atom`: the prefix and code that spell one component.
fn write_atom(
    p: &Profile,
    base: u32,
    prefix: Option<Prefix>,
) -> Option<(&'static str, &'static str)> {
    // A binary prefix is spelled only by a code that names it ("MebiBYTE").
    if let Some(Prefix {
        system: PrefixSystem::Iec,
        id,
    }) = prefix
    {
        if is_opaque(base) {
            return None;
        }
        return p
            .reverse
            .iter()
            .find(|e| e.base == base && e.iec == id && e.decade == 0)
            .map(|e| ("", e.code));
    }
    let decade = si_decade_of(prefix);
    if is_opaque(base) {
        // An opaque unit is spelled only by the profile that owns it.
        if !p.owns_opaque(base) {
            return None;
        }
        let atom = p
            .atoms
            .iter()
            .find(|a| matches!(a.target, AtomTarget::Opaque(id) if id == base))?;
        if decade != 0 && !atom.metric {
            return None;
        }
        let pfx = prefix_that_reads_back(p, decade, atom.code, atom)?;
        return Some((pfx, atom.code));
    }
    // A base may have several rows (one per decade in a flat profile).
    p.reverse
        .iter()
        .filter(|e| e.base == base && e.iec == 0)
        .find_map(|e| {
            let want = decade - e.decade;
            if want != 0 && !e.metric {
                return None;
            }
            let atom = p.find_atom(e.code.as_bytes())?;
            let pfx = prefix_that_reads_back(p, want, e.code, atom)?;
            Some((pfx, e.code))
        })
}
