//! Dimensional analysis: `bvn_units_compatible` and `bvn_units_convertible`.

use super::gen::units::*;
use super::{data, Unit, MAX_COMPONENTS};

/// Quantity kinds: units the SI dimension vector cannot tell apart from a
/// pure number (or from each other) and which still must not convert into
/// one another. Each is tracked by net exponent, like a dimension.
#[derive(Clone, Copy)]
enum Kind {
    InfoBit,
    InfoByte,
    /// One shared kind, so ° converts to rad; the steradian and the
    /// photometric units built on it count twice.
    Angle,
    LogNeper,
    LogDecibel,
    LogPh,
    TurbidityNtu,
    TurbidityFnu,
    TurbidityFtu,
    TurbidityFau,
    TurbidityJtu,
    SalinityPsu,
    /// A temperature difference; only counted for a lone unit at exponent 1,
    /// the one place an affine offset could otherwise be applied.
    TempInterval,
}

const KIND_COUNT: usize = Kind::TempInterval as usize + 1;

/// `bvni_kind_table`: (base unit, kind, weight).
const KINDS: [(u32, Kind, i32); 28] = [
    (BU_BIT, Kind::InfoBit, 1),
    (BU_BYTE, Kind::InfoByte, 1),
    (BU_RADIAN, Kind::Angle, 1),
    (BU_DEGREE, Kind::Angle, 1),
    (BU_ARCMINUTE, Kind::Angle, 1),
    (BU_ARCSECOND, Kind::Angle, 1),
    (BU_GRAD, Kind::Angle, 1),
    (BU_REVOLUTION, Kind::Angle, 1),
    (BU_STERADIAN, Kind::Angle, 2),
    (BU_SPERE, Kind::Angle, 2),
    (BU_LUMEN, Kind::Angle, 2),
    (BU_LUX, Kind::Angle, 2),
    (BU_PHOT, Kind::Angle, 2),
    (BU_NEPER, Kind::LogNeper, 1),
    (BU_DECIBEL, Kind::LogDecibel, 1),
    (BU_PH_SCALE, Kind::LogPh, 1),
    (BU_TURBIDITY_NTU, Kind::TurbidityNtu, 1),
    (BU_TURBIDITY_FNU, Kind::TurbidityFnu, 1),
    (BU_TURBIDITY_FTU, Kind::TurbidityFtu, 1),
    (BU_TURBIDITY_FAU, Kind::TurbidityFau, 1),
    (BU_TURBIDITY_JTU, Kind::TurbidityJtu, 1),
    (BU_PRACTICAL_SALINITY, Kind::SalinityPsu, 1),
    (BU_DELTA_KELVIN, Kind::TempInterval, 1),
    (BU_DELTA_FAHRENHEIT, Kind::TempInterval, 1),
    (BU_DELTA_DELISLE, Kind::TempInterval, 1),
    (BU_DELTA_NEWTON_TEMP, Kind::TempInterval, 1),
    (BU_DELTA_REAUMUR, Kind::TempInterval, 1),
    (BU_DELTA_ROMER, Kind::TempInterval, 1),
];

/// Net exponent of every quantity kind (`bvni_kind_exponents`). Repeated
/// bases are folded first, so "lone" means the unit's shape after
/// cancellation: `ΔK·m/m` is a lone ΔK.
fn kind_exponents(u: &Unit) -> [i32; KIND_COUNT] {
    let mut bases = [(0u32, 0i32); MAX_COMPONENTS];
    let mut n = 0;
    for c in u.components() {
        let e = c.exponent();
        match bases
            .get_mut(..n)
            .and_then(|b| b.iter_mut().find(|(id, _)| *id == c.base.0))
        {
            Some((_, net)) => *net += e,
            None => {
                if let Some(slot) = bases.get_mut(n) {
                    *slot = (c.base.0, e);
                    n += 1;
                }
            }
        }
    }
    let live = bases.get(..n).unwrap_or_default();
    let lone = live.iter().filter(|(_, net)| *net != 0).count() == 1;
    let mut out = [0i32; KIND_COUNT];
    for &(id, net) in live.iter().filter(|(_, net)| *net != 0) {
        let Some(&(_, kind, weight)) = KINDS.iter().find(|(base, _, _)| *base == id) else {
            continue;
        };
        if matches!(kind, Kind::TempInterval) && !(lone && net == 1) {
            continue; // inside a compound K already is an interval
        }
        if let Some(slot) = out.get_mut(kind as usize) {
            *slot += weight * net;
        }
    }
    out
}

/// The SI dimension vector; `None` for a unit without one (a currency).
fn dimension_vector(u: &Unit) -> Option<[i32; 7]> {
    let mut dims = [0i32; 7];
    for c in u.components() {
        let row = data::native_row(c.base.0)?;
        for (d, &unit_dim) in dims.iter_mut().zip(row.dims.iter()) {
            *d += i32::from(unit_dim) * c.exponent();
        }
    }
    Some(dims)
}

/// `bvn_units_compatible`: same dimensions and same quantity kinds.
fn compatible(a: &Unit, b: &Unit) -> bool {
    if kind_exponents(a) != kind_exponents(b) {
        return false;
    }
    match (dimension_vector(a), dimension_vector(b)) {
        (Some(da), Some(db)) => da == db,
        _ => false,
    }
}

/// `bvn_unit_prefix_only_delta`: the same (base, exponent) multiset, so the
/// units differ at most in their prefixes.
fn same_but_prefixes(a: &Unit, b: &Unit) -> bool {
    if a.len != b.len {
        return false;
    }
    let mut used = [false; MAX_COMPONENTS];
    a.components().iter().all(|ca| {
        match b
            .components()
            .iter()
            .zip(used.iter_mut())
            .find(|(cb, used)| !**used && cb.base == ca.base && cb.exponent == ca.exponent)
        {
            Some((_, used)) => {
                *used = true;
                true
            }
            None => false,
        }
    })
}

pub(crate) fn convertible(a: &Unit, b: &Unit) -> bool {
    compatible(a, b) || same_but_prefixes(a, b)
}
