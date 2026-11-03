//! Row types of the generated tables and the lookups over them.

use super::gen::{currencies, prefixes, units};

/// Which prefixes a base unit accepts (`.prefix` in `units.bvnr`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PrefixPolicy {
    /// Any SI prefix, no IEC prefix.
    Default,
    /// IEC prefixes and SI prefixes from kilo up (bit, byte).
    Info,
    /// Historical German units: no prefix.
    German,
    /// Ratios such as `%` and `ppm`: no prefix.
    Ratio,
    /// No prefix.
    None,
}

/// One native base unit.
#[derive(Debug)]
pub(crate) struct UnitRow {
    pub(crate) name: &'static str,
    pub(crate) symbol: &'static str,
    pub(crate) factor: f64,
    pub(crate) dims: [i8; 7],
    pub(crate) affine: bool,
    pub(crate) offset: f64,
    pub(crate) policy: PrefixPolicy,
    pub(crate) factor_num: &'static str,
    pub(crate) factor_den: &'static str,
    pub(crate) offset_num: &'static str,
    pub(crate) offset_den: &'static str,
    pub(crate) exact: bool,
    pub(crate) pi: Option<(&'static str, &'static str, i32)>,
}

pub(crate) use super::PrefixSystem;

/// One SI or IEC prefix.
#[derive(Debug)]
pub(crate) struct PrefixRow {
    pub(crate) name: &'static str,
    pub(crate) symbol: &'static str,
    pub(crate) exp: i32,
}

/// The native unit row of a base unit id, if it is one.
pub(crate) fn native_row(id: u32) -> Option<&'static UnitRow> {
    let slot = id.checked_sub(units::NATIVE_FIRST)?;
    units::UNITS.get(usize::try_from(slot).ok()?)
}

/// The ISO code of a currency id, if it is one.
pub(crate) fn currency_code(id: u32) -> Option<&'static str> {
    let slot = id.checked_sub(currencies::CURRENCY_FIRST)?;
    currencies::CURRENCIES
        .get(usize::try_from(slot).ok()?)
        .copied()
}

/// The unit a spelling names exactly.
pub(crate) fn alias_lookup(spelling: &[u8]) -> Option<u32> {
    units::ALIASES
        .binary_search_by(|(a, _)| a.as_bytes().cmp(spelling))
        .ok()
        .and_then(|i| units::ALIASES.get(i))
        .map(|&(_, id)| id)
}

/// The prefix a spelling names exactly, as (system, id).
pub(crate) fn prefix_lookup(spelling: &[u8]) -> Option<(PrefixSystem, u8)> {
    prefixes::PREFIX_ALIASES
        .binary_search_by(|(a, _, _)| a.as_bytes().cmp(spelling))
        .ok()
        .and_then(|i| prefixes::PREFIX_ALIASES.get(i))
        .map(|&(_, system, id)| (system, id))
}

/// The currency id of an ISO code, matched like `bvn_parse_currency_str`:
/// three or four ASCII capitals naming a catalogued currency.
pub(crate) fn currency_lookup(code: &[u8]) -> Option<u32> {
    if !(3..=4).contains(&code.len()) || !code.iter().all(u8::is_ascii_uppercase) {
        return None;
    }
    let i = currencies::CURRENCY_INDEX
        .binary_search_by(|(c, _)| c.as_bytes().cmp(code))
        .ok()?;
    let &(_, slot) = currencies::CURRENCY_INDEX.get(i)?;
    Some(currencies::CURRENCY_FIRST + u32::from(slot))
}

/// Whether a compact prefixed spelling is refused by name ("usb", "kt").
pub(crate) fn compact_refused(spelling: &[u8]) -> bool {
    units::COMPACT_EXCEPTIONS
        .binary_search_by(|e| e.as_bytes().cmp(spelling))
        .is_ok()
}

/// The prefix row of (system, id); id 0 means "no prefix" and has none.
pub(crate) fn prefix_row(system: PrefixSystem, id: u8) -> Option<&'static PrefixRow> {
    let table: &[PrefixRow] = match system {
        PrefixSystem::Si => &prefixes::SI_PREFIXES,
        PrefixSystem::Iec => &prefixes::IEC_PREFIXES,
    };
    table.get(usize::from(id).checked_sub(1)?)
}
