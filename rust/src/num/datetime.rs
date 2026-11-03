//! ISO-8601 datetime literals and datetime epochs.
//!
//! Ports `bvn_iso_*` from the C validator plus the parts of `bvn_datetime.c`
//! and `bvn_gregorian_date.c` they use. A literal is converted to whole
//! seconds on the declared epoch's timescale; `tai` additionally applies the
//! leap-second table, and the GNSS epochs refuse ISO literals.

use crate::types::{TypeFamily, TypeSpec};
use crate::ErrorKind;
use alloc::string::{String, ToString};
use core::ops::Range;

/// Epoch names in the order of their index (stored in `TypeSpec::base`),
/// with the Modified Julian Day of each epoch's start (`bvn_epoch_table`).
const EPOCHS: [(&str, i64); 9] = [
    ("unix", 40_587),
    ("tai", 36_204),
    ("gps", 44_244),
    ("mjd", 0),
    ("ntp", 15_020),
    ("galileo", 51_412),
    ("glonass", 50_083),
    ("y2000", 51_544),
    ("beidou", 53_736),
];

const MJD_TAI: i64 = 36_204;
const GNSS_MJDS: [i64; 4] = [44_244, 51_412, 50_083, 53_736];

/// UTC instants (seconds since the TAI epoch 1958-01-01) from which
/// `TAI - UTC` takes the given value (`bvn_dt_leap_table`).
const LEAP_TABLE: [(i64, i64); 28] = [
    (441_763_200, 10),   // 1972-01-01
    (457_488_000, 11),   // 1972-07-01
    (473_385_600, 12),   // 1973-01-01
    (504_921_600, 13),   // 1974-01-01
    (536_457_600, 14),   // 1975-01-01
    (567_993_600, 15),   // 1976-01-01
    (599_616_000, 16),   // 1977-01-01
    (631_152_000, 17),   // 1978-01-01
    (662_688_000, 18),   // 1979-01-01
    (694_224_000, 19),   // 1980-01-01
    (741_484_800, 20),   // 1981-07-01
    (773_020_800, 21),   // 1982-07-01
    (804_556_800, 22),   // 1983-07-01
    (867_715_200, 23),   // 1985-07-01
    (946_684_800, 24),   // 1988-01-01
    (1_009_843_200, 25), // 1990-01-01
    (1_041_379_200, 26), // 1991-01-01
    (1_088_640_000, 27), // 1992-07-01
    (1_120_176_000, 28), // 1993-07-01
    (1_151_712_000, 29), // 1994-07-01
    (1_199_145_600, 30), // 1996-01-01
    (1_246_406_400, 31), // 1997-07-01
    (1_293_840_000, 32), // 1999-01-01
    (1_514_764_800, 33), // 2006-01-01
    (1_609_459_200, 34), // 2009-01-01
    (1_719_792_000, 35), // 2012-07-01
    (1_814_400_000, 36), // 2015-07-01
    (1_861_920_000, 37), // 2017-01-01
];

const FIRST_TAI_MINUS_UTC: i64 = 10;

/// An ISO literal converted to epoch seconds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct IsoInstant {
    /// Whole seconds on the epoch's timescale (the fraction is floored away).
    pub(crate) seconds: i64,
    /// Byte range of the fraction digits in the literal (without the '.');
    /// empty when the literal has no fraction.
    pub(crate) frac: Range<usize>,
}

/// Index of the epoch called `name`, if any (`bvn_epoch_index_of`).
pub(crate) fn epoch_index_of(name: &[u8]) -> Option<u32> {
    EPOCHS
        .iter()
        .position(|(n, _)| n.as_bytes() == name)
        .and_then(|i| u32::try_from(i).ok())
}

/// Name of the epoch with `index`, if any.
pub(crate) fn epoch_name(index: u32) -> Option<&'static str> {
    EPOCHS.get(usize::try_from(index).ok()?).map(|(n, _)| *n)
}

/// MJD of the epoch `vt` selects; anything but a datetime with a known
/// epoch index falls back to unix (`bvnr_datetime_epoch_mjd`).
fn epoch_mjd(vt: TypeSpec) -> i64 {
    let index = match vt.family {
        TypeFamily::Datetime => usize::try_from(vt.base).unwrap_or(usize::MAX),
        _ => 0,
    };
    EPOCHS.get(index).unwrap_or(&EPOCHS[0]).1
}

/// Whether `s` looks like an ISO date: some '-' directly follows a digit,
/// which a number never has (`bvn_str_is_iso_datetime`).
pub(crate) fn str_is_iso_datetime(s: &[u8]) -> bool {
    s.windows(2).any(|w| w[1] == b'-' && w[0].is_ascii_digit())
}

/// Whether `s` is an optional '-' followed by one or more decimal digits
/// (`bvn_str_is_plain_integer`).
pub(crate) fn str_is_plain_integer(s: &[u8]) -> bool {
    let digits = s.strip_prefix(b"-").unwrap_or(s);
    !digits.is_empty() && digits.iter().all(u8::is_ascii_digit)
}

/// Renders `v` in decimal (`bvn_i64_to_decimal`).
pub(crate) fn i64_to_decimal(v: i64) -> String {
    v.to_string()
}

/// Converts an ISO literal to seconds on `vt`'s epoch
/// (`bvn_iso_to_epoch_seconds`).
///
/// Errors as the reference reports them: `InvalidDatetimeLiteral` for a
/// malformed literal or one with a fraction whose UTC year leaves
/// 0000..9999, `DatetimeLiteralUnsupportedEpoch` for the GNSS epochs and
/// `ValueOutOfRange` on arithmetic overflow.
pub(crate) fn iso_to_epoch_seconds(s: &[u8], vt: TypeSpec) -> Result<IsoInstant, ErrorKind> {
    let f = parse_fields(s).ok_or(ErrorKind::InvalidDatetimeLiteral)?;
    // Fold the zone offset into the minute, not the second, so a written
    // leap second (:60) keeps its meaning under any offset.
    let minute = f.minute - f.offset_seconds / 60;
    let mjd = days_from_civil(f.year, f.month, f.day);
    let epoch = epoch_mjd(vt);
    let seconds = if epoch == MJD_TAI {
        utc_to_tai(mjd, f.hour, minute, f.second)
    } else if GNSS_MJDS.contains(&epoch) {
        return Err(ErrorKind::DatetimeLiteralUnsupportedEpoch);
    } else {
        civil_seconds(mjd, epoch, f.hour, minute, f.second)
    }
    .ok_or(ErrorKind::ValueOutOfRange)?;
    // A fraction only round-trips as an ISO literal, so the UTC year must be
    // one a four-digit ISO year can spell (the writer refuses the rest).
    if !f.frac.is_empty() {
        let back = if epoch == MJD_TAI {
            tai_to_utc_year(seconds)
        } else {
            Some(year_of(seconds, epoch))
        };
        if !back.is_some_and(|y| (0..=9999).contains(&y)) {
            return Err(ErrorKind::InvalidDatetimeLiteral);
        }
    }
    Ok(IsoInstant {
        seconds,
        frac: f.frac,
    })
}

/// Civil UTC fields of an instant: year, month, day, hour, minute, second.
pub(crate) type Civil = [i64; 6];

/// Converts epoch seconds back to the civil UTC time an ISO literal spells
/// (`bvn_dt_epoch_seconds_to_datetime` / `bvn_dt_tai_seconds_to_utc`). `None`
/// for the GNSS epochs, which have no ISO form. An inserted TAI leap second
/// comes out as `23:59:60` of the day before the boundary.
pub(crate) fn epoch_seconds_to_civil(seconds: i64, vt: TypeSpec) -> Option<Civil> {
    let epoch = epoch_mjd(vt);
    if GNSS_MJDS.contains(&epoch) {
        return None;
    }
    let (utc, epoch, leap) = if epoch == MJD_TAI {
        let off = LEAP_TABLE
            .iter()
            .rev()
            .find(|(t, o)| seconds >= t + o)
            .map_or(FIRST_TAI_MINUS_UTC, |e| e.1);
        let inserted = LEAP_TABLE
            .iter()
            .enumerate()
            .any(|(i, (t, o))| is_insertion(i) && seconds == t + o - 1);
        let utc = seconds.checked_sub(off)?;
        if inserted {
            (utc.checked_sub(1)?, MJD_TAI, true)
        } else {
            (utc, MJD_TAI, false)
        }
    } else {
        (seconds, epoch, false)
    };
    let mjd = utc.div_euclid(86_400).checked_add(epoch)?;
    let tod = utc.rem_euclid(86_400);
    let (year, month, day) = civil_from_mjd(mjd);
    let second = tod % 60 + i64::from(leap);
    Some([year, month, day, tod / 3600, tod / 60 % 60, second])
}

/// Proleptic Gregorian date of a Modified Julian Day (civil-from-days).
fn civil_from_mjd(mjd: i64) -> (i64, i64, i64) {
    let z = mjd.saturating_add(678_881);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe.saturating_add(era.saturating_mul(400)) + i64::from(month <= 2);
    (year, month, day)
}

/// The fields of an ISO literal exactly as written.
struct Fields {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    offset_seconds: i64,
    frac: Range<usize>,
}

fn two_digits(s: &[u8], at: usize) -> Option<i64> {
    match s.get(at..at + 2)? {
        [a @ b'0'..=b'9', b @ b'0'..=b'9'] => Some(i64::from((a - b'0') * 10 + (b - b'0'))),
        _ => None,
    }
}

fn days_in_month(year: i64, month: i64) -> i64 {
    const DIM: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    if month == 2 && leap {
        return 29;
    }
    usize::try_from(month - 1)
        .ok()
        .and_then(|i| DIM.get(i))
        .copied()
        .unwrap_or(0)
}

/// Strict parse of `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM:SS[.frac][Z|±HH:MM]`
/// (`bvn_iso_parse_fields`). Second 60 (a leap second) is accepted.
fn parse_fields(s: &[u8]) -> Option<Fields> {
    let n = s.len();
    if n < 10 {
        return None;
    }
    let year = s.get(..4)?.iter().try_fold(0i64, |v, c| {
        c.is_ascii_digit().then(|| v * 10 + i64::from(c - b'0'))
    })?;
    if s.get(4) != Some(&b'-') || s.get(7) != Some(&b'-') {
        return None;
    }
    let month = two_digits(s, 5)?;
    let day = two_digits(s, 8)?;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    let mut f = Fields {
        year,
        month,
        day,
        hour: 0,
        minute: 0,
        second: 0,
        offset_seconds: 0,
        frac: 0..0,
    };
    let mut p = 10;
    if p < n {
        if n < 19
            || s.get(10) != Some(&b'T')
            || s.get(13) != Some(&b':')
            || s.get(16) != Some(&b':')
        {
            return None;
        }
        f.hour = two_digits(s, 11)?;
        f.minute = two_digits(s, 14)?;
        f.second = two_digits(s, 17)?;
        if f.hour > 23 || f.minute > 59 || f.second > 60 {
            return None;
        }
        p = 19;
        if s.get(p) == Some(&b'.') {
            p += 1;
            let start = p;
            p += s
                .get(p..)?
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .count();
            if p == start {
                return None;
            }
            f.frac = start..p;
        }
        match s.get(p) {
            None => {}
            Some(b'Z') => p += 1,
            Some(&sign @ (b'+' | b'-')) => {
                p += 1;
                if n - p != 5 || s.get(p + 2) != Some(&b':') {
                    return None;
                }
                let oh = two_digits(s, p)?;
                let om = two_digits(s, p + 3)?;
                if oh > 23 || om > 59 {
                    return None;
                }
                let off = oh * 3600 + om * 60;
                f.offset_seconds = if sign == b'-' { -off } else { off };
                p = n;
            }
            Some(_) => return None,
        }
    }
    (p == n).then_some(f)
}

/// Modified Julian Day of a proleptic Gregorian date (days-from-civil).
/// Inputs come from a validated literal: year 0..9999, valid month and day.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    // 678881 days separate 0000-03-01 (day 0 of the algorithm) from MJD 0.
    era * 146_097 + doe - 678_881
}

/// Gregorian year of a Modified Julian Day (civil-from-days).
fn year_from_mjd(mjd: i64) -> i64 {
    let z = mjd + 678_881;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    // Months are counted from March, so January and February (mp 10, 11)
    // belong to the next calendar year.
    yoe + era * 400 + i64::from(mp >= 10)
}

/// Year of the instant `seconds` on the civil epoch `epoch` (floor days).
fn year_of(seconds: i64, epoch: i64) -> i64 {
    year_from_mjd(seconds.div_euclid(86_400) + epoch)
}

/// Uniform 86400-second-day conversion (`bvn_dt_datetime_to_epoch_seconds`);
/// out-of-range time fields roll over, so second 60 becomes the next minute.
fn civil_seconds(mjd: i64, epoch: i64, hour: i64, minute: i64, second: i64) -> Option<i64> {
    (mjd - epoch)
        .checked_mul(86_400)?
        .checked_add(hour * 3600)?
        .checked_add(minute.checked_mul(60)?)?
        .checked_add(second)
        .filter(|&r| r != i64::MIN)
}

/// `TAI - UTC` in force at the UTC instant (before 1972 the first entry).
fn tai_minus_utc_at(utc: i64) -> i64 {
    LEAP_TABLE
        .iter()
        .rev()
        .find(|(t, _)| utc >= *t)
        .map_or(FIRST_TAI_MINUS_UTC, |e| e.1)
}

/// Whether table entry `i` adds a second to its predecessor.
fn is_insertion(i: usize) -> bool {
    i > 0
        && matches!(
            (LEAP_TABLE.get(i), LEAP_TABLE.get(i - 1)),
            (Some(a), Some(b)) if a.1 == b.1 + 1
        )
}

/// Whether a leap second was inserted right before the UTC instant.
fn utc_boundary_has_insertion(utc: i64) -> bool {
    LEAP_TABLE
        .iter()
        .position(|(t, _)| *t == utc)
        .is_some_and(is_insertion)
}

/// `bvn_dt_utc_to_tai_seconds`: a written :60 on an insertion boundary gets
/// the TAI second just below the boundary instead of folding onto it.
fn utc_to_tai(mjd: i64, hour: i64, minute: i64, second: i64) -> Option<i64> {
    let utc = civil_seconds(mjd, MJD_TAI, hour, minute, second)?;
    let mut tai = utc
        .checked_add(tai_minus_utc_at(utc))
        .filter(|&t| t != i64::MIN)?;
    if second == 60 && utc_boundary_has_insertion(utc) {
        tai = tai.checked_sub(1).filter(|&t| t != i64::MIN)?;
    }
    Some(tai)
}

/// UTC calendar year of a TAI instant (`bvn_dt_tai_seconds_to_utc`); `None`
/// where the reference leaves its output untouched (overflow).
fn tai_to_utc_year(tai: i64) -> Option<i64> {
    let off = LEAP_TABLE
        .iter()
        .rev()
        .find(|(t, o)| tai >= t + o)
        .map_or(FIRST_TAI_MINUS_UTC, |e| e.1);
    let utc = tai.checked_sub(off)?;
    let inserted = LEAP_TABLE
        .iter()
        .enumerate()
        .any(|(i, (t, o))| is_insertion(i) && tai == t + o - 1);
    // An inserted second renders as 23:59:60 of the day before the boundary.
    Some(year_of(if inserted { utc - 1 } else { utc }, MJD_TAI))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(epoch: &str) -> TypeSpec {
        TypeSpec {
            family: TypeFamily::Datetime,
            width: 64,
            base: epoch_index_of(epoch.as_bytes()).unwrap(),
        }
    }

    fn secs(s: &str, epoch: &str) -> Result<i64, ErrorKind> {
        iso_to_epoch_seconds(s.as_bytes(), dt(epoch)).map(|i| i.seconds)
    }

    #[test]
    fn epochs() {
        assert_eq!(epoch_index_of(b"unix"), Some(0));
        assert_eq!(epoch_index_of(b"beidou"), Some(8));
        assert_eq!(epoch_index_of(b""), None);
        assert_eq!(epoch_index_of(b"UNIX"), None);
        assert_eq!(epoch_name(1), Some("tai"));
        assert_eq!(epoch_name(9), None);
    }

    #[test]
    fn shape_checks() {
        assert!(str_is_iso_datetime(b"2026-06-15"));
        assert!(!str_is_iso_datetime(b"-5"));
        assert!(!str_is_iso_datetime(b"1e-5"));
        assert!(str_is_plain_integer(b"-12") && str_is_plain_integer(b"0"));
        assert!(!str_is_plain_integer(b"-") && !str_is_plain_integer(b""));
        assert!(!str_is_plain_integer(b"+1") && !str_is_plain_integer(b"1.0"));
        assert_eq!(i64_to_decimal(i64::MIN), "-9223372036854775808");
        assert_eq!(i64_to_decimal(0), "0");
    }

    #[test]
    fn civil_epochs() {
        assert_eq!(secs("1970-01-01T00:00:00Z", "unix"), Ok(0));
        assert_eq!(secs("1970-01-01", "unix"), Ok(0));
        assert_eq!(secs("2026-06-15T12:00:00Z", "unix"), Ok(1_781_524_800));
        assert_eq!(secs("1969-12-31T23:59:59Z", "unix"), Ok(-1));
        assert_eq!(secs("0000-01-01T00:00:00Z", "unix"), Ok(-62_167_219_200));
        assert_eq!(secs("9999-12-31T23:59:59Z", "unix"), Ok(253_402_300_799));
        assert_eq!(secs("2000-01-01T00:00:00Z", "y2000"), Ok(0));
        assert_eq!(secs("1900-01-01T00:00:00Z", "ntp"), Ok(0));
        assert_eq!(secs("1858-11-17T00:00:00Z", "mjd"), Ok(0));
        // Offsets fold to UTC.
        assert_eq!(secs("1970-01-01T01:00:00+01:00", "unix"), Ok(0));
        assert_eq!(secs("1969-12-31T23:30:00-00:30", "unix"), Ok(0));
        // A leap second folds onto the next second on civil epochs.
        assert_eq!(secs("2016-12-31T23:59:60Z", "unix"), Ok(1_483_228_800));
    }

    #[test]
    fn tai_leap_seconds() {
        assert_eq!(secs("2017-01-01T00:00:00Z", "tai"), Ok(1_861_920_037));
        assert_eq!(secs("2016-12-31T23:59:60Z", "tai"), Ok(1_861_920_036));
        assert_eq!(secs("2016-12-31T23:59:59Z", "tai"), Ok(1_861_920_035));
        assert_eq!(secs("2017-01-01T00:59:60+01:00", "tai"), Ok(1_861_920_036));
        // Not an insertion boundary: :60 just folds.
        assert_eq!(secs("2016-06-30T23:59:60Z", "tai"), Ok(1_846_022_436));
        assert_eq!(secs("2016-07-01T00:00:00Z", "tai"), Ok(1_846_022_436));
        // Before 1972 the first offset (10 s) applies.
        assert_eq!(secs("1958-01-01T00:00:00Z", "tai"), Ok(10));
        // 1972-01-01 is the first entry, not an insertion.
        assert_eq!(secs("1971-12-31T23:59:60Z", "tai"), Ok(441_763_210));
    }

    #[test]
    fn invalid_literals() {
        let bad = ErrorKind::InvalidDatetimeLiteral;
        for s in [
            "2026-02-29",
            "2026-13-01",
            "2026-00-10",
            "2026-04-31",
            "2026-06-15T24:00:00Z",
            "2026-06-15T12:60:00Z",
            "2026-06-15T12:00:61Z",
            "2026-06-15T12:00:00.Z",
            "2026-06-15T12:00:00+1:00",
            "2026-06-15T12:00:00+24:00",
            "2026-06-15T12:00:00+01:60",
            "2026-06-15T12:00:00+0100",
            "2026-06-15T12:00:00ZZ",
            "2026-06-15T12:00",
            "2026-06-15 12:00:00",
            "2026-6-15",
            "+2026-06-15",
            "2026-06-15T",
        ] {
            assert_eq!(secs(s, "unix"), Err(bad), "{s}");
        }
        assert!(secs("2024-02-29", "unix").is_ok());
        assert!(secs("2000-02-29", "unix").is_ok());
        assert_eq!(secs("1900-02-29", "unix"), Err(bad));
    }

    #[test]
    fn gnss_epochs_refuse() {
        for e in ["gps", "galileo", "glonass", "beidou"] {
            assert_eq!(
                secs("2020-01-01", e),
                Err(ErrorKind::DatetimeLiteralUnsupportedEpoch)
            );
        }
        // Malformed beats unsupported.
        assert_eq!(
            secs("2020-13-01", "gps"),
            Err(ErrorKind::InvalidDatetimeLiteral)
        );
    }

    #[test]
    fn fractions() {
        let s = b"2026-06-15T12:00:00.250Z";
        let i = iso_to_epoch_seconds(s, dt("unix")).unwrap();
        assert_eq!(i.seconds, 1_781_524_800);
        assert_eq!(&s[i.frac], b"250");
        let i = iso_to_epoch_seconds(b"2026-06-15", dt("unix")).unwrap();
        assert!(i.frac.is_empty());
        // The fold to UTC may push the year out of 0000..9999; only refused
        // when a fraction is present.
        let bad = Err(ErrorKind::InvalidDatetimeLiteral);
        assert_eq!(secs("0000-01-01T00:00:00.5+23:59", "unix"), bad);
        assert!(secs("0000-01-01T00:00:00+23:59", "unix").is_ok());
        assert_eq!(secs("9999-12-31T23:59:59.5-00:01", "unix"), bad);
        assert_eq!(secs("9999-12-31T23:59:59.5-00:01", "tai"), bad);
        assert_eq!(secs("9999-12-31T23:59:59.5Z", "tai"), Ok(253_780_992_036));
        assert_eq!(secs("0000-01-01T00:00:00.5Z", "tai"), Ok(-61_788_527_990));
        assert_eq!(
            secs("0000-01-01T00:00:00+23:59", "unix"),
            Ok(-62_167_305_540)
        );
        assert!(secs("2016-12-31T23:59:60.5Z", "tai").is_ok());
    }

    #[test]
    fn non_datetime_spec_uses_unix() {
        let i = iso_to_epoch_seconds(b"1970-01-02", TypeSpec::PLAIN).unwrap();
        assert_eq!(i.seconds, 86_400);
    }
}
