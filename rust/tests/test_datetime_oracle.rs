//! Port of `python/tests/test_datetime_oracle.py`: ISO-8601 datetime literals
//! checked against an independent reference written in this file (proleptic
//! Gregorian civil-day arithmetic plus the IERS leap-second table).

use bovnar::{Value, parse};

/// Epoch day zero as a Modified Julian Date. mjd/ntp/y2000 are the civil
/// (UTC-labelled) scale; tai is the only literal-supported atomic epoch.
fn epoch_mjd(epoch: &str) -> i64 {
    match epoch {
        "unix" => 40587,
        "mjd" => 0,
        "ntp" => 15020,
        "y2000" => 51544,
        "tai" => 36204,
        _ => panic!("unknown epoch {epoch}"),
    }
}

/// IERS TAI-UTC table: thresholds in TAI-epoch uniform seconds -> (TAI - UTC).
/// Verbatim from src/utils/bvn_datetime.c; pre-1972 instants use the first
/// entry.
const LEAP: [(i64, i64); 28] = [
    (441763200, 10),
    (457488000, 11),
    (473385600, 12),
    (504921600, 13),
    (536457600, 14),
    (567993600, 15),
    (599616000, 16),
    (631152000, 17),
    (662688000, 18),
    (694224000, 19),
    (741484800, 20),
    (773020800, 21),
    (804556800, 22),
    (867715200, 23),
    (946684800, 24),
    (1009843200, 25),
    (1041379200, 26),
    (1088640000, 27),
    (1120176000, 28),
    (1151712000, 29),
    (1199145600, 30),
    (1246406400, 31),
    (1293840000, 32),
    (1514764800, 33),
    (1609459200, 34),
    (1719792000, 35),
    (1814400000, 36),
    (1861920000, 37),
];

const TAI_SHIFT: i64 = (40587 - 36204) * 86400;

fn leap_at(uniform_tai: i64) -> i64 {
    let mut off = LEAP[0].1;
    for (thr, val) in LEAP {
        if uniform_tai >= thr {
            off = val;
        }
    }
    off
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// The civil date of a day count since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (
        if m <= 2 {
            yoe + era * 400 + 1
        } else {
            yoe + era * 400
        },
        m,
        d,
    )
}

/// Python's `calendar.timegm`: linear in every field, so second 60 is
/// simply the next second.
fn timegm(y: i64, mo: i64, d: i64, h: i64, mi: i64, s: i64) -> i64 {
    days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + s
}

/// Python's `time.gmtime`: (year, month, day, hour, minute, second).
fn gmtime(t: i64) -> (i64, i64, i64, i64, i64, i64) {
    let (y, mo, d) = civil_from_days(t.div_euclid(86400));
    let s = t.rem_euclid(86400);
    (y, mo, d, s / 3600, s % 3600 / 60, s % 60)
}

#[allow(clippy::too_many_arguments)]
fn oracle(y: i64, mo: i64, d: i64, h: i64, mi: i64, s: i64, epoch: &str, offset_s: i64) -> i64 {
    // fold the tz offset to true UTC: UTC = civil - offset
    let unix_s = timegm(y, mo, d, h, mi, s) - offset_s;
    if epoch == "tai" {
        let uniform_tai = unix_s + TAI_SHIFT;
        return uniform_tai + leap_at(uniform_tai);
    }
    unix_s + (40587 - epoch_mjd(epoch)) * 86400
}

fn dim(y: i64, m: i64) -> i64 {
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ][(m - 1) as usize]
}

/// Parses `doc` and returns its `.t` as an integer.
fn load_t(doc: &str) -> i64 {
    let v = parse(doc.as_bytes()).unwrap_or_else(|e| panic!("{doc:?}: {e}"));
    let Value::Struct(entries) = v else {
        unreachable!()
    };
    match entries.iter().find(|(k, _)| k == "t") {
        Some((_, Value::Int(i))) => i64::try_from(*i).unwrap(),
        other => panic!("{doc:?}: unexpected {other:?}"),
    }
}

fn annotation(epoch: &str) -> String {
    if epoch == "unix" {
        String::new()
    } else {
        format!("<datetime:64,{epoch}> ")
    }
}

// Years: leap-second boundaries, epoch starts, pre-epoch, DST-ish, far future.
const YEARS: [i64; 20] = [
    1, 100, 1583, 1700, 1900, 1969, 1970, 1971, 1972, 1999, 2000, 2016, 2017, 2024, 2025, 2026,
    2038, 2100, 2400, 9999,
];
const TIMES: [(i64, i64, i64); 4] = [(0, 0, 0), (12, 0, 0), (23, 59, 59), (6, 30, 15)];
const EPOCHS: [&str; 5] = ["unix", "mjd", "ntp", "y2000", "tai"];

#[test]
fn test_iso_conversion_matches_independent_reference() {
    let mut mism = Vec::new();
    let mut n = 0;
    for y in YEARS {
        for mo in [1, 2, 6, 12] {
            for d in [1, 15, 28.min(dim(y, mo)), dim(y, mo)] {
                for (h, mi, s) in TIMES {
                    for ep in EPOCHS {
                        n += 1;
                        let iso = format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z");
                        let doc = format!("#!bovnar 1.1\n.t = {}{iso};\n", annotation(ep));
                        let got = load_t(&doc);
                        let exp = oracle(y, mo, d, h, mi, s, ep, 0);
                        if got != exp {
                            mism.push((iso, ep, got, exp));
                        }
                    }
                }
            }
        }
    }
    assert!(
        mism.is_empty(),
        "{}/{n} mismatches, first: {:?}",
        mism.len(),
        &mism[..mism.len().min(5)]
    );
    assert!(n > 5000, "corpus unexpectedly small ({n})");
}

#[test]
fn test_date_only_literal_is_midnight_utc() {
    for y in [1970, 2000, 2026, 9999] {
        for mo in [1, 6, 12] {
            for d in [1, dim(y, mo)] {
                let doc = format!("#!bovnar 1.1\n.t = {y:04}-{mo:02}-{d:02};\n");
                assert_eq!(load_t(&doc), oracle(y, mo, d, 0, 0, 0, "unix", 0));
            }
        }
    }
}

#[test]
fn test_fractional_seconds_floor_to_whole_second_carrier() {
    // the carrier is whole seconds, so the value floors to the written second
    // (the fraction digits are preserved separately; the carrier is unaffected)
    let base = load_t("#!bovnar 1.1\n.t = 2026-06-15T12:00:00Z;");
    for frac in [".0", ".5", ".999", ".000000123", ".123456789"] {
        let doc = format!("#!bovnar 1.1\n.t = 2026-06-15T12:00:00{frac}Z;\n");
        assert_eq!(load_t(&doc), base);
    }
}

#[test]
fn test_tz_offset_folds_to_utc() {
    // +-HH:MM offsets, incl. half-hour (India), -12, +14, and tai
    // (leap-correct because the offset is folded before the leap-second
    // lookup). Verified against the reference with the offset folded to UTC.
    let offs = [
        (0, "+00:00"),
        (7200, "+02:00"),
        (-18000, "-05:00"),
        (19800, "+05:30"),
        (-43200, "-12:00"),
        (50400, "+14:00"),
    ];
    let mut n = 0;
    for y in [1972, 2000, 2017, 2026] {
        for (h, mi, s) in [(0, 0, 0), (12, 30, 15), (23, 59, 59)] {
            for (off_s, zone) in offs {
                for ep in EPOCHS {
                    n += 1;
                    let doc = format!(
                        "#!bovnar 1.1\n.t = {}{y:04}-06-15T{h:02}:{mi:02}:{s:02}{zone};\n",
                        annotation(ep)
                    );
                    assert_eq!(load_t(&doc), oracle(y, 6, 15, h, mi, s, ep, off_s));
                }
            }
        }
    }
    assert!(n > 200);
}

/// Every table entry after the first is a positive leap second of exactly
/// one second, i.e. a real insertion UTC spells 23:59:60. Entry 0 establishes
/// the initial 1972 offset and inserts nothing.
fn insertions() -> Vec<(i64, i64)> {
    (1..LEAP.len())
        .filter(|&i| LEAP[i].1 == LEAP[i - 1].1 + 1)
        .map(|i| LEAP[i])
        .collect()
}

/// Civil fields of the UTC second one below `uniform_utc` (TAI-epoch scale).
fn utc_civil_before(uniform_utc: i64) -> (i64, i64, i64, i64, i64, i64) {
    gmtime(uniform_utc - TAI_SHIFT - 1)
}

fn oracle_iso(uniform_utc: i64) -> String {
    let (y, mo, d, h, mi, s) = gmtime(uniform_utc - TAI_SHIFT);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

#[test]
fn test_leap_second_has_its_own_tai_value() {
    // The inserted second is a real instant on the continuous atomic scale,
    // so on tai it must NOT collapse onto the following second: it sits
    // exactly one TAI second below the boundary. This is what makes UTC<->TAI
    // injective.
    let ins = insertions();
    assert_eq!(ins.len(), 27, "expected 27 insertions, got {}", ins.len());
    for (thr, off) in ins {
        let (y, mo, d, h, mi, _) = utc_civil_before(thr);
        let leap = format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:60Z");
        let got = load_t(&format!("#!bovnar 1.1\n.t = <datetime:64,tai> {leap};\n"));
        assert_eq!(got, thr + off - 1, "{leap}");
        // ... and the second after it is a distinct value, one higher.
        let nxt = load_t(&format!(
            "#!bovnar 1.1\n.t = <datetime:64,tai> {};\n",
            oracle_iso(thr)
        ));
        assert_eq!(nxt, got + 1, "{leap}: next second");
    }
}

#[test]
fn test_leap_second_collapses_on_civil_epochs() {
    // POSIX time runs a uniform 86400 s day and has no second to spend on the
    // insertion, so there :60 must still fold onto the following second.
    for (thr, _) in insertions() {
        let (y, mo, d, h, mi, _) = utc_civil_before(thr);
        let leap = format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:60Z");
        for ep in ["unix", "mjd", "ntp", "y2000"] {
            let got = load_t(&format!("#!bovnar 1.1\n.t = {}{leap};\n", annotation(ep)));
            assert_eq!(got, oracle(y, mo, d, h, mi, 60, ep, 0), "{leap} on {ep}");
        }
    }
}

#[test]
fn test_leap_second_survives_a_tz_offset() {
    // An ISO offset is whole minutes, so it moves the wall clock but not the
    // second-of-minute: the :60 spelling must survive the fold to UTC.
    let z = load_t("#!bovnar 1.1\n.t = <datetime:64,tai> 2016-12-31T23:59:60Z;\n");
    let tz = load_t("#!bovnar 1.1\n.t = <datetime:64,tai> 2017-01-01T00:59:60+01:00;\n");
    assert_eq!(z, tz);
    assert_eq!(tz, 1861920036);
}

#[test]
fn test_unrecorded_leap_second_still_collapses() {
    // The table is a static snapshot. A build predating an IERS announcement
    // must not reject a document spelling a genuine future leap second, so a
    // :60 the table does not record keeps the historical collapse.
    let a = load_t("#!bovnar 1.1\n.t = <datetime:64,tai> 2020-06-30T23:59:60Z;\n");
    let b = load_t("#!bovnar 1.1\n.t = <datetime:64,tai> 2020-07-01T00:00:00Z;\n");
    assert_eq!(a, b);
}
