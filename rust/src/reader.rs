//! The streaming reader, mirroring `python/bovnar/reader.py`.

use crate::enums::{ErrorCode, Event, TokenType};
use crate::error::{Error, Result};
use crate::ffi::{self, BvnrReadFlags, ValueTypeSpec};
use crate::unit::Unit;

/// Options for [`Reader::read_mem`], the keyword arguments of Python's
/// `Reader.read_mem`. The default reads with the library's limits.
#[derive(Clone, Debug, Default)]
pub struct ReadOptions {
    /// Largest accepted input in bytes; 0 is the library default.
    pub max_file_size: u64,
    /// Recover from errors and keep reading instead of stopping.
    pub continue_on_error: bool,
    /// Refuse a document whose `#!bovnar` directive names a newer version.
    pub strict_version: bool,
    /// Refuse a document containing an octet stream.
    pub text_only: bool,
    /// Let a `want_unit` conversion deliver a non-terminating expansion as
    /// a rounded literal (its exact `num/den` stays available in C).
    pub want_unit_allow_nonterminating: bool,
    /// Longest converted literal; 0 is the library default.
    pub max_conversion_length: u32,
}

/// A rule about one field, named by its dotted key path (`".a.b"`, or
/// `".a.*"` for everything below). With `convert` false it only asserts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitRule {
    pub path: String,
    pub unit: String,
    pub base: u32,
    pub convert: bool,
}

impl UnitRule {
    /// A converting rule in base 10.
    pub fn new(path: &str, unit: &str) -> Self {
        UnitRule {
            path: path.into(),
            unit: unit.into(),
            base: 0,
            convert: true,
        }
    }
}

/// What a document must contain, and what unit values arrive in: the
/// declarative form of `want_unit`. See Python's `UnitPolicy` for the full
/// rules; everything is unit text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitPolicy {
    /// Per-field rules, consulted first; the first match wins.
    pub rules: Vec<UnitRule>,
    /// Units to convert to, as (unit, base); the first valid one wins.
    pub targets: Vec<(String, u32)>,
    /// Deliver what no target took in coherent SI base units.
    pub normalise_si: bool,
    /// Output base of converted values; 0 is decimal.
    pub base: u32,
    /// Deliver a value that cannot convert exactly in its own unit instead
    /// of failing.
    pub leave_inexact: bool,
    /// Reject any bare numeric value.
    pub require_unit: bool,
    /// Require every numeric value to convert to one of these units.
    pub require_dimension_of: Vec<String>,
}

/// `BVNR_MAX_UNIT_TARGETS`: the most targets, and required dimensions.
pub const MAX_UNIT_TARGETS: usize = 8;
/// `BVNR_MAX_UNIT_RULES`.
pub const MAX_UNIT_RULES: usize = 8;
/// `BVNR_MAX_UNIT_PATH`: the longest rule path in bytes.
pub const MAX_UNIT_PATH: usize = 96;

/// What a refused policy is reported as when no single entry is to blame.
const UNIT_POLICY_REFUSED_GENERAL: &str = "unusable unit policy — check the units, bases and \
     rule paths in targets / require_dimension_of / rules";

impl UnitPolicy {
    /// The counts the C side would refuse anyway, reported by name.
    pub(crate) fn check_counts(&self) -> Result<()> {
        let too_many = |what: &str, n: usize, max: usize| {
            Err(Error::Argument(format!("at most {max} {what}, got {n}")))
        };
        if self.targets.len() > MAX_UNIT_TARGETS {
            return too_many("unit targets", self.targets.len(), MAX_UNIT_TARGETS);
        }
        if self.require_dimension_of.len() > MAX_UNIT_TARGETS {
            return too_many(
                "required dimensions",
                self.require_dimension_of.len(),
                MAX_UNIT_TARGETS,
            );
        }
        if self.rules.len() > MAX_UNIT_RULES {
            return too_many("per-field rules", self.rules.len(), MAX_UNIT_RULES);
        }
        Ok(())
    }

    pub(crate) fn spec(&self) -> ffi::PolicySpec {
        ffi::PolicySpec {
            rules: self
                .rules
                .iter()
                .map(|r| (r.path.clone(), r.unit.clone(), r.base, r.convert))
                .collect(),
            targets: self.targets.clone(),
            base: self.base,
            normalise: self.normalise_si,
            leave_inexact: self.leave_inexact,
            require_unit: self.require_unit,
            require_dimension_of: self.require_dimension_of.clone(),
        }
    }
}

/// Names the entry the C setter refused, by offering each one on its own
/// (Python's `describe_unit_policy_refusal`). The setter answers one bool for
/// a whole policy; the setter itself is the oracle, so this cannot drift from
/// the library's own rules.
pub(crate) fn describe_unit_policy_refusal(policy: &UnitPolicy) -> String {
    let Some(mut probe) = ffi::ReaderHandle::new() else {
        return UNIT_POLICY_REFUSED_GENERAL.into();
    };
    let mut refuses = |p: UnitPolicy| !probe.set_unit_policy(Some(&p.spec()));
    let alone = |f: &dyn Fn(&mut UnitPolicy)| {
        let mut p = UnitPolicy::default();
        f(&mut p);
        p
    };

    for (unit, tbase) in &policy.targets {
        let t = (unit.clone(), *tbase);
        if !refuses(alone(&|p| p.targets = vec![t.clone()])) {
            continue;
        }
        if *tbase != 0 && !refuses(alone(&|p| p.targets = vec![(unit.clone(), 0)])) {
            return format!(
                "unusable unit policy: targets entry {unit:?} asks for base {tbase}, \
                 which bovnar cannot write"
            );
        }
        return format!(
            "unusable unit policy: targets entry {unit:?} is not a unit bovnar can parse"
        );
    }

    for u in &policy.require_dimension_of {
        if refuses(alone(&|p| p.require_dimension_of = vec![u.clone()])) {
            return format!(
                "unusable unit policy: require_dimension_of entry {u:?} is not a unit bovnar can parse"
            );
        }
    }

    for r in &policy.rules {
        if !refuses(alone(&|p| p.rules = vec![r.clone()])) {
            continue;
        }
        // Which part? Hold the others at values that certainly parse and vary
        // one at a time: the path first, since it is the only one left when a
        // unit that parses and no base still will not go in.
        let plain = UnitRule {
            unit: "m".into(),
            base: 0,
            ..r.clone()
        };
        if refuses(alone(&|p| p.rules = vec![plain.clone()])) {
            return format!(
                "unusable unit policy: rule path {:?} is unusable — a path must start with '.', \
                 be at most {MAX_UNIT_PATH} bytes, and cannot be just '.*'",
                r.path
            );
        }
        let unbased = UnitRule {
            base: 0,
            ..r.clone()
        };
        if refuses(alone(&|p| p.rules = vec![unbased.clone()])) {
            return format!(
                "unusable unit policy: rule {:?} names unit {:?}, which is not a unit bovnar can parse",
                r.path, r.unit
            );
        }
        return format!(
            "unusable unit policy: rule {:?} asks for base {}, which bovnar cannot write",
            r.path, r.base
        );
    }

    // Nothing singled itself out: a whole-policy field or a combination.
    if policy.base != 0
        && !refuses(UnitPolicy {
            base: 0,
            ..policy.clone()
        })
    {
        return format!(
            "unusable unit policy: base {} is not a base bovnar can write",
            policy.base
        );
    }
    UNIT_POLICY_REFUSED_GENERAL.into()
}

/// One event as a handler sees it, borrowed for the duration of the call
/// (Python's `(Event, BvnrData)` callback arguments).
#[derive(Clone, Copy, Debug)]
pub struct EventData<'a> {
    pub event: Event,
    /// The token type of the payload (`BvnrData.type`), when it has one.
    pub token_type: Option<TokenType>,
    pub value_type: ValueTypeSpec,
    /// The value's unit; dimensionless when it has none.
    pub value_unit: Unit,
    /// The payload bytes: a key, a value literal or an octet chunk.
    pub raw: &'a [u8],
    /// A datetime's sub-second digits, or empty.
    pub frac: &'a [u8],
    /// The unit as the document spelled it, or empty.
    pub unit_source: &'a [u8],
    /// True when a unit policy or `want_unit` converted the value.
    pub converted: bool,
    /// The converted literal, or empty.
    pub converted_text: &'a [u8],
    /// The base of `converted_text`.
    pub converted_base: u32,
    /// The unit of `converted_text`.
    pub converted_unit: Unit,
    /// The exact converted value, when `converted`.
    conv_exact: Option<ffi::Rational<'a>>,
}

impl<'a> EventData<'a> {
    /// A plain, unconverted event, so the dict parser can be driven by
    /// hand-built event sequences (like Python's `_DictParser` tests).
    #[doc(hidden)]
    pub fn plain(event: Event, value_type: ValueTypeSpec, raw: &'a [u8]) -> Self {
        Self {
            event,
            token_type: None,
            value_type,
            value_unit: Unit::default(),
            raw,
            frac: b"",
            unit_source: b"",
            converted: false,
            converted_text: b"",
            converted_base: 0,
            converted_unit: Unit::default(),
            conv_exact: None,
        }
    }

    /// The public view of a raw event; `None` for an event this binding does
    /// not know.
    pub(crate) fn from_view(view: &ffi::EventView<'a>) -> Option<Self> {
        let event = Event::from_raw(view.event)?;
        let d = view.data;
        Some(EventData {
            event,
            token_type: d.and_then(|d| TokenType::from_raw(d.type_)),
            value_type: d.map_or(ValueTypeSpec::default(), |d| d.value_type),
            value_unit: d.map_or(Unit::default(), |d| Unit(d.value_unit)),
            raw: view.raw,
            frac: view.frac,
            unit_source: view.unit_source,
            converted: d.is_some_and(|d| d.converted),
            converted_text: view.conv_text,
            converted_base: d.filter(|d| d.converted).map_or(0, |d| d.conv.base),
            converted_unit: d
                .filter(|d| d.converted)
                .map_or(Unit::default(), |d| Unit(d.conv.unit)),
            conv_exact: view.conv_exact,
        })
    }

    /// The exact converted value as a reduced `(numerator, denominator)`
    /// pair in decimal (it may exceed any machine integer), or `None` when no
    /// conversion was applied (Python's `converted_rational`). The only
    /// carrier of the value when the expansion does not terminate in the
    /// requested base (`want_unit_allow_nonterminating`).
    pub fn converted_rational(&self) -> Option<(String, String)> {
        self.conv_exact?.parts()
    }

    /// The exact converted value rendered in `base` (2..62, 64 or 85), or
    /// `None` when no conversion was applied or the value has no terminating
    /// expansion in `base` (Python's `converted_in_base`). Fails for a base
    /// the library cannot write.
    pub fn converted_in_base(&self, base: u32) -> Result<Option<String>> {
        let Some(exact) = self.conv_exact else {
            return Ok(None);
        };
        exact
            .in_base(base)
            .map_err(|()| Error::Argument(format!("base {base} is not one bovnar can write")))
    }

    /// The payload as text, with invalid UTF-8 replaced.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(self.raw).into_owned()
    }
}

/// An owned copy of one event, as collected by [`Reader::iter_mem`]
/// (Python's `EventPayload`).
#[derive(Clone, Debug, PartialEq)]
pub struct EventPayload {
    pub event: Event,
    pub token_type: Option<TokenType>,
    pub value_type: ValueTypeSpec,
    pub value_unit: Unit,
    pub raw: Vec<u8>,
    pub converted: bool,
    /// The converted literal, when `converted`.
    pub converted_text: Option<String>,
    pub converted_base: u32,
}

impl EventPayload {
    /// The payload as text, with invalid UTF-8 replaced.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.raw).into_owned()
    }
}

impl From<&EventData<'_>> for EventPayload {
    fn from(d: &EventData<'_>) -> Self {
        EventPayload {
            event: d.event,
            token_type: d.token_type,
            value_type: d.value_type,
            value_unit: d.value_unit,
            raw: d.raw.to_vec(),
            converted: d.converted,
            converted_text: d
                .converted
                .then(|| String::from_utf8_lossy(d.converted_text).into_owned()),
            converted_base: d.converted_base,
        }
    }
}

/// A `want_unit` hook: the unit (and output base) to deliver a numeric value
/// in, or `None` to deliver it as written.
pub type WantUnit<'h> = &'h mut dyn FnMut(&EventData<'_>) -> Option<(Unit, u32)>;

/// A handler for reader events; returning false stops the read.
pub type EventHandler<'h> = &'h mut dyn FnMut(&EventData<'_>) -> bool;

/// A streaming BVNR reader.
pub struct Reader {
    handle: ffi::ReaderHandle,
}

impl Reader {
    /// Creates a reader.
    pub fn new() -> Result<Self> {
        let handle = ffi::ReaderHandle::new()
            .ok_or_else(|| Error::Argument("bvnr_reader_create() returned NULL".into()))?;
        Ok(Reader { handle })
    }

    /// Reads `data`, calling `on_verified` for every event the validator
    /// accepted and `on_unverified` for every raw lexer event.
    pub fn read_mem<'h>(
        &mut self,
        data: &[u8],
        options: &ReadOptions,
        on_verified: Option<EventHandler<'h>>,
        on_unverified: Option<EventHandler<'h>>,
    ) -> Result<()> {
        self.read(data, options, on_verified, on_unverified, None)
    }

    /// [`Reader::read_mem`] with a `want_unit` hook, which picks the unit
    /// (and base) each numeric value is delivered in; the conversion is
    /// exact or the read fails.
    pub fn read_mem_want_unit<'h>(
        &mut self,
        data: &[u8],
        options: &ReadOptions,
        want_unit: WantUnit<'h>,
        on_verified: Option<EventHandler<'h>>,
    ) -> Result<()> {
        self.read(data, options, on_verified, None, Some(want_unit))
    }

    /// Installs a unit policy for every following read; `None` clears it.
    pub fn set_unit_policy(&mut self, policy: Option<&UnitPolicy>) -> Result<()> {
        if let Some(p) = policy {
            p.check_counts()?;
        }
        if self
            .handle
            .set_unit_policy(policy.map(UnitPolicy::spec).as_ref())
        {
            Ok(())
        } else {
            Err(Error::Argument(match policy {
                Some(p) => describe_unit_policy_refusal(p),
                None => "bvnr_reader_set_unit_policy failed".into(),
            }))
        }
    }

    pub fn recovery_count(&self) -> u64 {
        self.handle.recovery().0
    }

    /// The `#!bovnar M.N` directive of the last read, or `None` when the
    /// document carried none (Python's `declared_version`).
    pub fn declared_version(&self) -> Option<(u16, u16)> {
        self.handle.declared_version()
    }

    /// How many input bytes a `continue_on_error` read skipped.
    pub fn skipped_bytes(&self) -> u64 {
        self.handle.recovery().1
    }

    fn read<'h>(
        &mut self,
        data: &[u8],
        options: &ReadOptions,
        on_verified: Option<EventHandler<'h>>,
        on_unverified: Option<EventHandler<'h>>,
        want_unit: Option<WantUnit<'h>>,
    ) -> Result<()> {
        let mut flags = BvnrReadFlags::zeroed();
        flags.max_file_size = options.max_file_size;
        // Reader.read_mem opens at the 255-level cap in both directions, like
        // the CLI and the DOM; zero would select the C default of 64.
        flags.max_struct_nesting = 255;
        flags.max_array_nesting = 255;
        flags.continue_on_error = options.continue_on_error;
        flags.strict_version = options.strict_version;
        flags.text_only = options.text_only;
        flags.want_unit_allow_nonterminating = options.want_unit_allow_nonterminating;
        flags.max_conversion_length = options.max_conversion_length;

        let mut verified = on_verified.map(wrap);
        let mut unverified = on_unverified.map(wrap);
        let mut want = want_unit.map(|f| {
            move |view: &ffi::EventView<'_>| {
                let d = EventData::from_view(view)?;
                f(&d).map(|(u, base)| (u.0, base))
            }
        });
        let mut handlers = ffi::Handlers {
            verified: verified.as_mut().map(|f| f as ffi::Handler<'_>),
            unverified: unverified.as_mut().map(|f| f as ffi::Handler<'_>),
            want_unit: want.as_mut().map(|f| f as ffi::WantUnit<'_>),
            ..ffi::Handlers::default()
        };
        let ok = self.handle.read_mem(data, flags, &mut handlers);
        if ok { Ok(()) } else { Err(self.error()) }
    }

    /// Reads `data` and returns its events, verified ones only unless
    /// `verified_only` is false.
    pub fn iter_mem(
        &mut self,
        data: &[u8],
        verified_only: bool,
        max_file_size: u64,
    ) -> Result<Vec<EventPayload>> {
        let events = std::cell::RefCell::new(Vec::new());
        let mut collect = |d: &EventData<'_>| {
            events.borrow_mut().push(EventPayload::from(d));
            true
        };
        let mut collect_raw = |d: &EventData<'_>| {
            events.borrow_mut().push(EventPayload::from(d));
            true
        };
        let options = ReadOptions {
            max_file_size,
            ..ReadOptions::default()
        };
        let unverified: Option<EventHandler<'_>> = if verified_only {
            None
        } else {
            Some(&mut collect_raw)
        };
        self.read_mem(data, &options, Some(&mut collect), unverified)?;
        Ok(events.into_inner())
    }

    /// The code of the last error, `ErrorCode::None` after a clean read.
    pub fn error_code(&self) -> Option<ErrorCode> {
        ErrorCode::from_raw(self.handle.error().code)
    }

    /// The line of the last error, 0 after a clean read.
    pub fn error_line(&self) -> u64 {
        self.handle.error().line
    }

    fn error(&self) -> Error {
        let e = self.handle.error();
        Error::Parse {
            code: e.code,
            line: e.line,
            column: e.column,
            offset: e.offset,
            byte: e.byte,
        }
    }
}

/// Adapts a public handler to the raw event view the FFI layer delivers.
/// An event this binding does not know yet is skipped and the read goes on.
pub(crate) fn wrap<'h>(f: EventHandler<'h>) -> impl FnMut(&ffi::EventView<'_>) -> bool + 'h {
    move |view| EventData::from_view(view).is_none_or(|d| f(&d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iter_mem_yields_key_and_value() {
        let events = Reader::new()
            .unwrap()
            .iter_mem(b".a = 1;", true, 0)
            .unwrap();
        let texts: Vec<(Event, String)> = events.iter().map(|e| (e.event, e.text())).collect();
        assert!(
            texts.contains(&(Event::AssignmentStart, "a".into())),
            "{texts:?}"
        );
        assert!(texts.contains(&(Event::Data, "1".into())), "{texts:?}");
    }

    #[test]
    fn policy_converts_and_want_unit_converts() {
        let doc = b".d = <uint:64,k~m> 3;";
        let mut r = Reader::new().unwrap();
        let policy = UnitPolicy {
            targets: vec![("m".into(), 0)],
            ..UnitPolicy::default()
        };
        r.set_unit_policy(Some(&policy)).unwrap();
        let events = r.iter_mem(doc, true, 0).unwrap();
        let data = events.iter().find(|e| e.event == Event::Data).unwrap();
        assert!(data.converted, "{data:?}");
        assert_eq!(data.converted_text.as_deref(), Some("3000"));
        assert_eq!(data.value_unit.to_string(), "k~m");

        r.set_unit_policy(None).unwrap();
        let mut seen = None;
        let mut on = |d: &EventData<'_>| {
            if d.event == Event::Data {
                seen = Some((d.converted, d.text(), d.converted_text.to_vec()));
            }
            true
        };
        let mut want = |_: &EventData<'_>| Some((Unit::parse("m").unwrap(), 16));
        r.read_mem_want_unit(doc, &ReadOptions::default(), &mut want, Some(&mut on))
            .unwrap();
        assert_eq!(seen, Some((true, "3".into(), b"bb8".to_vec())));

        let strict = UnitPolicy {
            require_unit: true,
            ..UnitPolicy::default()
        };
        r.set_unit_policy(Some(&strict)).unwrap();
        assert!(r.iter_mem(b".n = 1;", true, 0).is_err());
    }

    #[test]
    fn handler_returning_false_stops_the_read() {
        let mut stop = |_: &EventData<'_>| false;
        let err = Reader::new()
            .unwrap()
            .read_mem(b".a = 1;", &ReadOptions::default(), Some(&mut stop), None)
            .unwrap_err();
        assert!(matches!(err, Error::Parse { .. }), "{err:?}");
    }
}
