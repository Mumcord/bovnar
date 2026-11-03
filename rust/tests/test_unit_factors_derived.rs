//! Conversion factors, re-derived from each unit's definition and compared
//! against `src/gendata/units.bvnr`, which is read here with the crate's own
//! parser. Exact rationals use a small `i128` fraction type. The generated C
//! tables are read from this crate's build output instead of CMake's
//! `build/generated`.

use std::f64::consts::PI;

use bovnar::{Value, parse};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
const GENERATED: &str = concat!(env!("OUT_DIR"), "/generated");

// --- a minimal exact rational ------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Q {
    num: i128,
    den: i128,
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Q {
    fn new(num: i128, den: i128) -> Q {
        assert!(den != 0, "zero denominator");
        let g = gcd(num, den).max(1);
        let s = if den < 0 { -1 } else { 1 };
        Q {
            num: s * num / g,
            den: s * den / g,
        }
    }

    fn int(n: i128) -> Q {
        Q::new(n, 1)
    }

    /// An exact decimal literal (`"0.45359237"`, `"-1.5e-3"`), like
    /// `Fraction(Decimal(text))`.
    fn dec(text: &str) -> Q {
        let (mant, exp) = match text.find(['e', 'E']) {
            Some(i) => (&text[..i], text[i + 1..].parse::<i32>().expect("exponent")),
            None => (text, 0),
        };
        let neg = mant.starts_with('-');
        let mant = mant.trim_start_matches(['-', '+']);
        let (int, frac) = mant.split_once('.').unwrap_or((mant, ""));
        let digits: i128 = format!("{int}{frac}").parse().expect("decimal digits");
        let scale = exp - frac.len() as i32;
        let pow = |n: i32| 10i128.checked_pow(n as u32).expect("decimal out of range");
        let q = if scale >= 0 {
            Q::int(
                digits
                    .checked_mul(pow(scale))
                    .expect("decimal out of range"),
            )
        } else {
            Q::new(digits, pow(-scale))
        };
        if neg { q.neg() } else { q }
    }

    /// The shortest decimal that round-trips to `x`, as a rational
    /// (`Fraction(Decimal(repr(float(x))))`).
    fn of_f64(x: f64) -> Q {
        Q::dec(&format!("{x:e}"))
    }

    fn neg(self) -> Q {
        Q::new(-self.num, self.den)
    }

    fn add(self, o: Q) -> Q {
        let g = gcd(self.den, o.den);
        let l = self.den / g * o.den;
        Q::new(self.num * (l / self.den) + o.num * (l / o.den), l)
    }

    fn sub(self, o: Q) -> Q {
        self.add(o.neg())
    }

    fn mul(self, o: Q) -> Q {
        let a = Q::new(self.num, o.den);
        let b = Q::new(o.num, self.den);
        Q::new(
            a.num.checked_mul(b.num).expect("overflow"),
            a.den.checked_mul(b.den).expect("overflow"),
        )
    }

    fn div(self, o: Q) -> Q {
        self.mul(Q::new(o.den, o.num))
    }

    fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }
}

// --- the unit document ---------------------------------------------------------

fn field<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Struct(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

fn text(v: &Value, key: &str) -> String {
    match field(v, key) {
        Some(Value::String(s)) => s.clone(),
        other => panic!("{key}: expected a string, got {other:?}"),
    }
}

fn number(v: &Value, key: &str) -> f64 {
    match field(v, key) {
        Some(Value::Float(f)) => *f,
        Some(Value::Int(i)) => *i as f64,
        other => panic!("{key}: expected a number, got {other:?}"),
    }
}

fn int(v: &Value, key: &str) -> Option<i128> {
    match field(v, key) {
        Some(Value::Int(i)) => Some(*i),
        // A value wider than 64 bits is stored as a quoted decimal.
        Some(Value::String(s)) => Some(s.parse().expect("an integer string")),
        None | Some(Value::Null) => None,
        other => panic!("{key}: expected an integer, got {other:?}"),
    }
}

fn flag(v: &Value, key: &str) -> Option<bool> {
    match field(v, key) {
        Some(Value::Bool(b)) => Some(*b),
        None => None,
        other => panic!("{key}: expected a bool, got {other:?}"),
    }
}

fn aliases(v: &Value) -> Vec<String> {
    match field(v, "aliases") {
        Some(Value::Array(items)) => items
            .iter()
            .map(|a| match a {
                Value::String(s) => s.clone(),
                other => panic!("alias: {other:?}"),
            })
            .collect(),
        other => panic!("aliases: {other:?}"),
    }
}

/// `src/gendata/units.bvnr`'s `.units` rows.
fn units() -> Vec<Value> {
    let data = std::fs::read(format!("{ROOT}/src/gendata/units.bvnr")).expect("read units.bvnr");
    let doc = parse(&data).expect("parse units.bvnr");
    match field(&doc, "units") {
        Some(Value::Array(rows)) => rows.clone(),
        other => panic!("units: {other:?}"),
    }
}

/// Alias -> factor for every unit.
fn factors() -> Vec<(String, f64)> {
    units()
        .iter()
        .flat_map(|u| {
            let f = number(u, "factor");
            aliases(u).into_iter().map(move |a| (a, f))
        })
        .collect()
}

fn find<'a>(units: &'a [Value], name: &str) -> Option<&'a Value> {
    units.iter().find(|u| text(u, "name") == name)
}

/// `gen_units.exact_rational`: the `<key>_num/<key>_den` override, else the
/// stored double's shortest decimal.
fn exact_rational(u: &Value, key: &str) -> Q {
    match (int(u, &format!("{key}_num")), int(u, &format!("{key}_den"))) {
        (Some(n), Some(d)) => Q::new(n, d),
        _ => Q::of_f64(number(u, key)),
    }
}

// --- the defining constants -------------------------------------------------------

const G0: f64 = 9.80665;
const LB: f64 = 0.45359237;
const FT: f64 = 0.3048;
const IN: f64 = FT / 12.0;
const GAL: f64 = 231.0 * IN * IN * IN;
const GAL_UK: f64 = 4.54609e-3;
const FL_OZ: f64 = GAL / 128.0;
const OZ_T: f64 = 31.1034768e-3;
const NMI: f64 = 1852.0;
const BUSHEL: f64 = 2150.42 * IN * IN * IN;
const PR_FUSS: f64 = 0.313853;
const C: f64 = 299792458.0;
const BTU_IT: f64 = 1055.05585262;

fn delisle_slope() -> Q {
    Q::new(-100, 150)
}
fn newton_slope() -> Q {
    Q::new(100, 33)
}
fn reaumur_slope() -> Q {
    Q::new(100, 80)
}
fn romer_slope() -> Q {
    Q::int(100).div(Q::int(60).sub(Q::new(15, 2)))
}

const DEFAULT_TOL: f64 = 1e-9;

/// symbol -> (value in coherent SI, how it is defined, tolerance)
fn derived() -> Vec<(&'static str, f64, &'static str, f64)> {
    let t = DEFAULT_TOL;
    let ca_co3 = 40.078 + 12.011 + 3.0 * 15.999;
    vec![
        ("in", IN, "foot / 12", t),
        ("yd", 3.0 * FT, "3 ft", t),
        ("mi", 5280.0 * FT, "5280 ft", t),
        ("fath", 6.0 * FT, "6 ft", t),
        ("ch", 66.0 * FT, "66 ft (Gunter's chain)", t),
        ("rd", 16.5 * FT, "16.5 ft", t),
        ("fur", 660.0 * FT, "660 ft = 1/8 mile", t),
        ("lea", 3.0 * 5280.0 * FT, "3 miles", t),
        ("nmi", NMI, "1852 m exactly", t),
        ("cbl", NMI / 10.0, "1/10 nautical mile", t),
        ("thou", IN / 1000.0, "inch / 1000", t),
        ("hand", 4.0 * IN, "4 in", t),
        ("ftUS", 1200.0 / 3937.0, "US survey foot", t),
        ("Å", 1e-10, "10⁻¹⁰ m", t),
        ("au", 149597870700.0, "IAU 2012 (exact)", t),
        ("ly", 9460730472580800.0, "c × Julian year (exact)", t),
        ("pc", 149597870700.0 * 648000.0 / PI, "au × 648000/π", t),
        ("ac", 43560.0 * FT * FT, "43560 ft²", t),
        ("ha", 1e4, "10⁴ m²", t),
        ("gal", GAL, "231 in³", t),
        ("qt", GAL / 4.0, "gallon / 4", t),
        ("pt", GAL / 8.0, "gallon / 8", t),
        ("cup", GAL / 16.0, "gallon / 16", t),
        ("fl_oz", FL_OZ, "gallon / 128", t),
        ("tbsp", FL_OZ / 2.0, "fl oz / 2", t),
        ("tsp", FL_OZ / 6.0, "fl oz / 6", t),
        ("fl_dr", FL_OZ / 8.0, "fl oz / 8", t),
        ("minim", FL_OZ / 480.0, "fl oz / 480", t),
        ("bbl", 42.0 * GAL, "42 US gallons (petroleum)", t),
        ("gal_uk", GAL_UK, "4.54609 L exactly", t),
        ("qt_uk", GAL_UK / 4.0, "imperial gallon / 4", t),
        ("pt_uk", GAL_UK / 8.0, "imperial gallon / 8", t),
        ("fl_oz_uk", GAL_UK / 160.0, "imperial gallon / 160", t),
        ("gi_uk", GAL_UK / 32.0, "imperial gallon / 32", t),
        ("bsh", BUSHEL, "2150.42 in³", t),
        ("pk", BUSHEL / 4.0, "bushel / 4", t),
        ("lb", LB, "0.45359237 kg exactly", t),
        ("oz", LB / 16.0, "pound / 16", t),
        ("dr", LB / 256.0, "pound / 256", t),
        ("gr", LB / 7000.0, "pound / 7000", t),
        ("st", 14.0 * LB, "14 lb", t),
        ("tn_sh", 2000.0 * LB, "2000 lb", t),
        ("tn_l", 2240.0 * LB, "2240 lb", t),
        ("oz_t", OZ_T, "troy ounce", t),
        ("dwt", OZ_T / 20.0, "troy ounce / 20", t),
        ("ct", 0.2e-3, "200 mg exactly", t),
        ("qntl", 100.0, "100 kg", t),
        ("t", 1000.0, "1000 kg", t),
        ("lbf", LB * G0, "pound × g₀", t),
        ("kip", 1000.0 * LB * G0, "1000 lbf", t),
        ("kgf", G0, "kilogram × g₀", t),
        ("gn", G0, "standard gravity", t),
        ("dyn", 1e-5, "10⁻⁵ N", t),
        ("psi", LB * G0 / (IN * IN), "lbf / in²", t),
        ("atm", 101325.0, "101325 Pa exactly", t),
        ("at", 98066.5, "1 kgf/cm²", t),
        ("bar", 1e5, "10⁵ Pa", t),
        ("Torr", 101325.0 / 760.0, "atm / 760", t),
        ("mmHg", 133.322387415, "conventional mmHg", t),
        ("cal", 4.184, "thermochemical calorie", t),
        ("ft_lb", FT * LB * G0, "foot × lbf", t),
        ("slug", LB * G0 / FT, "lbf·s²/ft", t),
        ("inHg", 25.4 * 133.322387415, "25.4 conventional mmHg", t),
        ("erg", 1e-7, "10⁻⁷ J", t),
        (
            "eV",
            1.602176634e-19,
            "elementary charge × volt (exact, SI 2019)",
            t,
        ),
        ("thm", 1e5 * 1054.804, "10⁵ BTU(59 °F) — US therm", t),
        ("thm_ec", 1e5 * BTU_IT, "10⁵ BTU(IT) — EC therm", t),
        ("hp", 550.0 * FT * LB * G0, "550 ft·lbf/s", t),
        ("PS", 75.0 * G0, "75 kgf·m/s", t),
        ("kn", NMI / 3600.0, "nautical mile per hour", t),
        ("mph", 5280.0 * FT / 3600.0, "mile per hour", t),
        ("kph", 1000.0 / 3600.0, "km per hour", t),
        (
            "rpm",
            1.0 / 60.0,
            "revolution per minute (as a cycle rate)",
            t,
        ),
        ("Gal", 0.01, "cm/s²", t),
        ("G", 1e-4, "10⁻⁴ T", t),
        ("Mx", 1e-8, "10⁻⁸ Wb", t),
        ("ph", 1e4, "10⁴ lx", t),
        ("sb", 1e4, "10⁴ cd/m²", t),
        ("Oe", 1000.0 / (4.0 * PI), "1000/4π A/m", t),
        (
            "statWb",
            1e-8 * 100.0 * C,
            "statV·s; statV = 10⁻⁸ c in cm/s",
            t,
        ),
        ("statT", 1e-8 * 100.0 * C * 1e4, "statWb / cm²", t),
        ("Ci", 3.7e10, "3.7×10¹⁰ Bq exactly", t),
        ("R", 2.58e-4, "2.58×10⁻⁴ C/kg exactly", t),
        ("min", 60.0, "60 s", t),
        ("h", 3600.0, "3600 s", t),
        ("d", 86400.0, "86400 s", t),
        ("wk", 7.0 * 86400.0, "7 d", t),
        ("fn", 14.0 * 86400.0, "14 d", t),
        ("yr", 365.25 * 86400.0, "Julian year", t),
        ("mo", 365.25 * 86400.0 / 12.0, "Julian year / 12", t),
        ("°", PI / 180.0, "π/180 rad", t),
        ("grad", PI / 200.0, "π/200 rad", t),
        ("rev", 2.0 * PI, "2π rad", t),
        ("arcmin", PI / 10800.0, "degree / 60", t),
        ("arcsec", PI / 648000.0, "degree / 3600", t),
        ("%", 0.01, "10⁻²", t),
        ("‰", 1e-3, "10⁻³", t),
        ("‱", 1e-4, "10⁻⁴", t),
        ("pcm", 1e-5, "10⁻⁵", t),
        ("ppm", 1e-6, "10⁻⁶", t),
        ("ppb", 1e-9, "10⁻⁹", t),
        ("L", 1e-3, "10⁻³ m³", t),
        (
            "Da",
            1.66053906892e-27,
            "unified atomic mass unit (CODATA 2022)",
            t,
        ),
        ("Pfd", 0.5, "Zollverein Pfund = 500 g", t),
        ("Ztr", 50.0, "100 Pfund", t),
        ("dz", 100.0, "200 Pfund", t),
        ("lot", 0.5 / 32.0, "Pfund / 32", t),
        ("prf", PR_FUSS, "Prussian foot, 1816", t),
        ("prz", PR_FUSS / 12.0, "Fuß / 12", t),
        ("prln", PR_FUSS / 144.0, "Zoll / 12", t),
        ("rute", 12.0 * PR_FUSS, "12 Fuß", t),
        ("klafter", 6.0 * PR_FUSS, "6 Fuß", t),
        ("elle", 25.5 * PR_FUSS / 12.0, "25½ Zoll", t),
        (
            "morgen",
            180.0 * (12.0 * PR_FUSS) * (12.0 * PR_FUSS),
            "180 square Ruten",
            t,
        ),
        (
            "schffl",
            0.0549615,
            "published Prussian Scheffel, 54.9615 L",
            5e-5,
        ),
        (
            "dt_mi",
            7420.4386,
            "geographische Meile = 1/15 of an equatorial degree",
            5e-6,
        ),
        ("°dH", 10.0 / (40.078 + 15.999), "10 mg CaO/L ÷ M(CaO)", t),
        ("°fH", 10.0 / ca_co3, "10 mg CaCO₃/L ÷ M(CaCO₃)", t),
        ("°aH", 1.0 / ca_co3, "1 mg CaCO₃/L ÷ M(CaCO₃)", t),
        ("°rH", 1.0 / 40.078, "1 mg Ca/L ÷ M(Ca)", t),
        (
            "°e",
            (64.79891e-3 * 1000.0 / 4.54609) / ca_co3,
            "grain CaCO₃ per imperial gallon",
            t,
        ),
        (
            "gpg",
            (64.79891e-3 * 1000.0 / 3.785411784) / ca_co3,
            "grain CaCO₃ per US gallon",
            t,
        ),
        ("val", 0.5, "equivalent of a divalent ion = ½ mol", t),
        ("CF", 0.01, "EC in mS/cm × 10 = 0.1 mS/cm", t),
        (
            "°De",
            delisle_slope().to_f64(),
            "0 °De = 100 °C and 150 °De = 0 °C",
            t,
        ),
        (
            "°N",
            newton_slope().to_f64(),
            "0 °N = 0 °C and 33 °N = 100 °C",
            t,
        ),
        (
            "°Re",
            reaumur_slope().to_f64(),
            "0 °Re = 0 °C and 80 °Re = 100 °C",
            t,
        ),
        (
            "°Ro",
            romer_slope().to_f64(),
            "7.5 °Ro = 0 °C and 60 °Ro = 100 °C",
            t,
        ),
        ("ΔK", 1.0, "the kelvin; Δ°C is the same interval", t),
        (
            "Δ°De",
            delisle_slope().to_f64(),
            "the Delisle degree as an interval",
            t,
        ),
        (
            "Δ°N",
            newton_slope().to_f64(),
            "the Newton degree as an interval",
            t,
        ),
        (
            "Δ°Re",
            reaumur_slope().to_f64(),
            "the Réaumur degree as an interval",
            t,
        ),
        (
            "Δ°Ro",
            romer_slope().to_f64(),
            "the Rømer degree as an interval",
            t,
        ),
        (
            "DU",
            2.687e20 / 6.02214076e23,
            "2.687×10²⁰ molecules/m² ÷ N_A = 0.4462 mmol/m²",
            2e-4,
        ),
    ]
}

/// Units whose defining relation is an exact rational, written from their
/// definitions independently of the table.
fn exact_definitions() -> Vec<(&'static str, Q, &'static str)> {
    let g0 = Q::new(980665, 100000);
    let lb = Q::dec("0.45359237");
    let ft = Q::dec("0.3048");
    let prf = Q::dec("0.313853");
    let mmhg = Q::dec("133.322387415");
    let btu_it = Q::dec("1055.05585262");
    let n = Q::int;
    let rute = n(12).mul(prf);
    let inch = ft.div(n(12));
    vec![
        ("prussian_zoll", prf.div(n(12)), "Fuß / 12"),
        ("prussian_line", prf.div(n(144)), "Zoll / 12"),
        (
            "prussian_elle",
            Q::new(51, 2).mul(prf).div(n(12)),
            "25½ Zoll",
        ),
        ("klafter", n(6).mul(prf), "6 Fuß"),
        ("prussian_rute", rute, "12 Fuß"),
        ("morgen", n(180).mul(rute).mul(rute), "180 square Ruten"),
        ("pound_force", lb.mul(g0), "lb × g₀"),
        ("kip", n(1000).mul(lb).mul(g0), "1000 lbf"),
        ("foot_pound", ft.mul(lb).mul(g0), "ft × lbf"),
        ("slug", lb.mul(g0).div(ft), "lbf·s²/ft"),
        ("psi", lb.mul(g0).div(inch.mul(inch)), "lbf / in²"),
        ("horsepower", n(550).mul(ft).mul(lb).mul(g0), "550 ft·lbf/s"),
        ("metric_horsepower", n(75).mul(g0), "75 kgf·m/s"),
        ("inch_hg", Q::new(254, 10).mul(mmhg), "25.4 mmHg"),
        ("torr", n(101325).div(n(760)), "atm / 760"),
        ("therm_ec", n(100000).mul(btu_it), "10⁵ BTU(IT)"),
        ("delisle", delisle_slope(), "0 °De = 100 °C, 150 °De = 0 °C"),
        (
            "delta_delisle",
            delisle_slope(),
            "the Delisle degree as an interval",
        ),
        ("newton_temp", newton_slope(), "0 °N = 0 °C, 33 °N = 100 °C"),
        (
            "delta_newton_temp",
            newton_slope(),
            "the Newton degree as an interval",
        ),
        ("reaumur", reaumur_slope(), "0 °Re = 0 °C, 80 °Re = 100 °C"),
        (
            "delta_reaumur",
            reaumur_slope(),
            "the Réaumur degree as an interval",
        ),
        ("romer", romer_slope(), "7.5 °Ro = 0 °C, 60 °Ro = 100 °C"),
        (
            "delta_romer",
            romer_slope(),
            "the Rømer degree as an interval",
        ),
    ]
}

#[test]
fn test_exactly_defined_factors_are_exactly_right() {
    let units = units();
    let mut wrong = Vec::new();
    for (name, want, how) in exact_definitions() {
        let u =
            find(&units, name).unwrap_or_else(|| panic!("{name} is no longer in the unit table"));
        let got = exact_rational(u, "factor");
        if got != want {
            let rel = (got.sub(want).div(want).to_f64()).abs();
            wrong.push(format!(
                "  {name:<18} table {}/{}, but {how} is {}/{} (off by {rel:.2e} relative)",
                got.num, got.den, want.num, want.den
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} factor(s) are not exactly their own definition:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}

#[test]
fn test_affine_offsets_follow_from_their_fixed_points() {
    let zero_c = Q::new(27315, 100);
    let want = [
        ("celsius", zero_c, "0 °C = 273.15 K"),
        (
            "fahrenheit",
            zero_c.sub(Q::int(32).mul(Q::new(5, 9))),
            "0 °F is 32 °F below the ice point",
        ),
        (
            "delisle",
            zero_c.add(Q::int(100)),
            "0 °De = 100 °C (the scale is inverted)",
        ),
        ("newton_temp", zero_c, "0 °N = 0 °C"),
        ("reaumur", zero_c, "0 °Re = 0 °C"),
        (
            "romer",
            zero_c.sub(Q::new(15, 2).mul(romer_slope())),
            "7.5 °Ro = 0 °C",
        ),
    ];
    let units = units();
    let mut affine: Vec<String> = units
        .iter()
        .filter(|u| flag(u, "affine") == Some(true))
        .map(|u| text(u, "name"))
        .collect();
    affine.sort();
    let mut want_names: Vec<String> = want.iter().map(|(n, _, _)| n.to_string()).collect();
    want_names.sort();
    assert_eq!(
        affine, want_names,
        "the set of affine scales has changed; derive the new one's offset here \
         rather than letting it ship unchecked"
    );
    let mut wrong = Vec::new();
    for (name, expected, how) in want {
        let got = exact_rational(find(&units, name).unwrap(), "offset");
        if got != expected {
            wrong.push(format!(
                "  {name:<14} table {}/{}, but {how} gives {}/{}",
                got.num, got.den, expected.num, expected.den
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} affine offset(s) are not their own definition:\n{}",
        wrong.len(),
        wrong.join("\n")
    );

    let mut stray: Vec<String> = units
        .iter()
        .map(|u| (text(u, "name"), u))
        .filter(|(n, u)| n.starts_with("delta_") && exact_rational(u, "offset") != Q::int(0))
        .map(|(n, _)| n)
        .collect();
    stray.sort();
    assert!(
        stray.is_empty(),
        "interval units must carry no offset: {stray:?}"
    );
}

/// Splits `text` at top-level commas (outside braces and quotes).
fn split_fields(text: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut quoted) = (Vec::new(), String::new(), 0, false);
    for ch in text.chars() {
        match ch {
            '"' => quoted = !quoted,
            '{' if !quoted => depth += 1,
            '}' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    out.push(cur.trim().to_string());
    out
}

#[test]
fn test_generated_tables_match_the_source() {
    let conv = std::fs::read_to_string(format!("{GENERATED}/bovnar_si_conv_table.gen.inc"))
        .expect("the build generates the conversion table");
    let units = units();
    let dim_names = [
        "length",
        "mass",
        "time",
        "current",
        "temperature",
        "amount",
        "luminosity",
    ];
    let mut rows: Vec<(String, Vec<String>)> = Vec::new();
    for line in conv.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("[BVN_SLOT_NATIVE(") else {
            continue;
        };
        let (key, rest) = rest.split_once(')').unwrap();
        let body = rest.split_once('{').unwrap().1;
        let body = body.rsplit_once('}').unwrap().0;
        let fields = split_fields(body);
        rows.push((key.to_string(), fields[1..10].to_vec()));
    }
    assert_eq!(
        rows.len(),
        units.len(),
        "si_conv_table has {} rows for {} units",
        rows.len(),
        units.len()
    );
    for u in &units {
        let key = format!("bu_{}", text(u, "name"));
        let row = &rows.iter().find(|(k, _)| *k == key).expect(&key).1;
        let [factor, dims, affine, offset, fnum, fden, _on, _od, exact] = [
            &row[0], &row[1], &row[2], &row[3], &row[4], &row[5], &row[6], &row[7], &row[8],
        ];
        assert_eq!(factor.parse::<f64>().unwrap(), number(u, "factor"), "{key}");
        let mut want = [0i128; 7];
        if let Some(Value::Struct(d)) = field(u, "dims") {
            for (name, value) in d {
                let Value::Int(v) = value else {
                    panic!("{key} dims")
                };
                want[dim_names.iter().position(|n| n == name).unwrap()] = *v;
            }
        }
        let got: Vec<i128> = dims
            .trim_matches(['{', '}'])
            .split(',')
            .map(|x| x.trim().parse().unwrap())
            .collect();
        assert_eq!(got, want, "{key} dims");
        assert_eq!(
            affine == "true",
            flag(u, "affine").unwrap_or(false),
            "{key} affine"
        );
        assert_eq!(
            offset.parse::<f64>().unwrap(),
            number(u, "offset"),
            "{key} offset"
        );
        assert_eq!(
            exact == "true",
            flag(u, "exact").unwrap_or(true),
            "{key} exact"
        );
        let stated = match (int(u, "factor_num"), int(u, "factor_den")) {
            (Some(n), Some(d)) => Q::new(n, d),
            _ => Q::of_f64(number(u, "factor")),
        };
        let table = Q::new(
            fnum.trim_matches('"').parse().unwrap(),
            fden.trim_matches('"').parse().unwrap(),
        );
        assert_eq!(table, stated, "{key} rational");
    }
}

#[test]
fn test_the_parse_table_is_ordered_longest_first() {
    let table = std::fs::read_to_string(format!("{GENERATED}/bovnar_bu_table.gen.inc"))
        .expect("the build generates the parse table");
    let lengths: Vec<u32> = table
        .lines()
        .filter_map(|l| {
            let l = l.trim().strip_prefix('{')?.trim_end().strip_suffix("},")?;
            let fields = split_fields(l);
            if fields.len() != 3 || !fields[2].starts_with("bu_") {
                return None;
            }
            fields[1].parse().ok()
        })
        .collect();
    assert!(
        !lengths.is_empty(),
        "no rows parsed out of bovnar_bu_table.gen.inc"
    );
    let mut sorted = lengths.clone();
    sorted.sort_by(|a, b| b.cmp(a));
    assert_eq!(lengths, sorted, "bu_table is not sorted longest-first");
    assert!(
        *lengths.iter().max().unwrap() < 32,
        "an alias exceeds BU_LEN_INDEX_SIZE"
    );
}

#[test]
fn test_factor_matches_its_definition() {
    let factors = factors();
    let mut table = derived();
    table.sort_by(|a, b| a.0.cmp(b.0));
    for (symbol, want, how, tol) in table {
        let got = factors
            .iter()
            .find(|(a, _)| a == symbol)
            .unwrap_or_else(|| panic!("{symbol} is no longer in the unit table"))
            .1;
        // math.isclose(got, want, rel_tol=tol)
        let close = (got - want).abs() <= tol * got.abs().max(want.abs());
        assert!(
            close,
            "{symbol}: table says {got:?}, but {how} gives {want:?} \
             (relative difference {:.3e}, tolerance {tol})",
            ((got - want) / want).abs()
        );
    }
}

#[test]
fn test_the_faq_category_breakdown_adds_up() {
    let total = units().len();
    // A Windows checkout may turn the FAQ's line endings into CRLF.
    let faq = std::fs::read_to_string(format!("{ROOT}/doc/02_bovnar_faq.md"))
        .expect("read FAQ")
        .replace("\r\n", "\n");
    let marker = " named base units across the following categories:\n\n";
    let at = faq
        .find(marker)
        .expect("the FAQ's unit-count answer has moved; update this test");
    let head = &faq[..at];
    let digits_start = head
        .rfind(|c: char| !c.is_ascii_digit())
        .map_or(0, |i| i + 1);
    let headline: usize = head[digits_start..]
        .parse()
        .expect("the FAQ's unit-count answer has moved; update this test");
    let counts: Vec<usize> = faq[at + marker.len()..]
        .lines()
        .take_while(|l| l.starts_with("- **"))
        .filter_map(|l| {
            let rest = &l["- **".len()..];
            let n: String = rest.chars().take_while(char::is_ascii_digit).collect();
            (!n.is_empty() && rest[n.len()..].starts_with(' ')).then(|| n.parse().unwrap())
        })
        .collect();
    assert!(
        !counts.is_empty(),
        "the FAQ's unit-count answer has moved; update this test"
    );
    assert_eq!(
        headline, total,
        "doc/02 says {headline} base units, units.bvnr has {total}"
    );
    let sum: usize = counts.iter().sum();
    assert_eq!(
        sum, total,
        "doc/02's category list sums to {sum}, but there are {total} units — a \
         category is missing or miscounted"
    );
}
