//! Port of `python/tests/test_unit_object.py`: the `Unit` object, its
//! equality and hashing, and the unit functions taking it.
//!
//! Not ported:
//! - `test_constructor_rejects_nonsense`: `Unit(42)` cannot be written in
//!   Rust; the type system refuses it.
//! - `test_numeric_accessors`, `test_fixed_point`: the `decimal`, `fraction`
//!   and `fixed_point` accessors are not mirrored (see the README).
//! - `test_calling_one_is_a_typeerror`: a property-versus-method mistake has
//!   no Rust counterpart.
//! - `test_rejects_bad_vtype`, `test_rejects_bad_raw`, `test_rejects_bad_frac`:
//!   `Quantity::new` takes a typed `&str`, `ValueTypeSpec` and `Unit`, so
//!   these arguments cannot be of the wrong type.
//! - `test_accepts_a_unit_string_for_convenience`: `Quantity::new` takes a
//!   `Unit`, never a string.
//! - `test_comparison_with_a_foreign_type_is_false_not_an_error`: Rust has no
//!   separate raw `ValueUnit`; `parse_unit` returns a `Unit`, and comparing
//!   one with any other type does not compile.
//!
//! The raw `ValueUnit` and the `Unit` object are one type in Rust, so the
//! tests that check the two agree compare a parsed unit with `Unit::parse`.

use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use bovnar::units::{
    convert_value, parse_unit, unit_convert_factor, unit_dimension_vector, unit_is_profile_only,
    unit_prefix_exponent, unit_prefix_factor, unit_reduce, unit_si_normal_form, unit_to_profile,
    unit_to_si_factor, unit_to_str, unit_to_str_ex, unit_to_ucum, unit_valid, units_compatible,
    units_convertible, units_equal,
};
use bovnar::{
    BaseUnit, Error, Event, Exponent, IECPrefix, Quantity, ReadOptions, Reader, Unit, Value,
    ValueTypeFamily, ValueTypeSpec, parse_typed,
};

fn q(src: &[u8]) -> Quantity {
    let Value::Struct(fields) = parse_typed(src).unwrap() else {
        panic!("not a struct");
    };
    match fields.into_iter().find(|(k, _)| k == "x").unwrap().1 {
        Value::Quantity(q) => *q,
        other => panic!("not a quantity: {other:?}"),
    }
}

fn u(s: &str) -> Unit {
    Unit::parse(s).unwrap()
}

fn h<T: Hash>(v: &T) -> u64 {
    let mut s = DefaultHasher::new();
    v.hash(&mut s);
    s.finish()
}

fn spec(family: ValueTypeFamily, width: u32, base: u32) -> ValueTypeSpec {
    ValueTypeSpec {
        family: family as i32,
        width,
        base,
    }
}

fn arg_err_contains<T: std::fmt::Debug>(r: bovnar::Result<T>, needle: &str) {
    match r {
        Err(Error::Argument(msg)) => assert!(msg.contains(needle), "{msg}"),
        other => panic!("expected an argument error, got {other:?}"),
    }
}

// TestUnitObject

#[test]
fn test_unit_is_a_unit_object() {
    let q = q(b".x = <float:64,k~m> 5.0;");
    assert_eq!(q.unit.to_string(), "k~m");
    assert_eq!(format!("{:?}", q.unit), "Unit(\"k~m\")");
}

#[test]
fn test_dimensionless_is_falsey_and_empty() {
    let q = q(b".x = <float:64> 5.0;");
    assert!(q.unit.is_dimensionless());
    assert_eq!(q.unit.to_string(), "");
}

#[test]
fn test_percent_is_a_real_unit_not_dimensionless() {
    let q = q(b".x = <float:64,%> 5.0;");
    assert!(!q.unit.is_dimensionless());
}

#[test]
fn test_equality_is_by_meaning_not_identity() {
    let a = q(b".x = <float:64,m/s> 1.0;").unit;
    let b = q(b".x = <float:64,m/s> 2.0;").unit;
    assert_eq!(a, b);
    assert_eq!(h(&a), h(&b));
    assert_ne!(a, q(b".x = <float:64,k~m/s> 1.0;").unit);
}

#[test]
fn test_equality_accepts_a_string() {
    let q = q(b".x = <float:64,k~m> 5.0;");
    assert!(q.unit == "k~m");
    assert!(q.unit != "m");
    assert!(q.unit != "not a unit at all");
}

#[test]
fn test_cross_notation_equality() {
    let native = u("mmHg");
    assert_eq!(native, u("ucum:mm[Hg]"));
    assert_eq!(h(&native), h(&u("ucum:mm[Hg]")));
}

#[test]
fn test_factor_and_compatibility() {
    let k = u("k~m");
    assert_eq!(k.factor().unwrap(), 1000.0);
    assert!(k.compatible_with(&u("m")));
    assert!(!k.compatible_with(&u("s")));
    assert_eq!(k.convert_factor(&u("m")).unwrap(), 1000.0);
}

#[test]
fn test_to_profile() {
    assert_eq!(u("k~g").to_profile("unece").unwrap(), "KGM");
    assert_eq!(u("mmHg").to_profile("ucum").unwrap(), "mm[Hg]");
}

#[test]
fn test_profile_only() {
    assert!(u("ucum:[IU]").is_profile_only());
    assert!(!u("m").is_profile_only());
}

#[test]
fn test_raw_still_reaches_the_struct() {
    let q = q(b".x = <float:64,m/s> 1.0;");
    assert_eq!(unit_to_str(&q.unit).unwrap(), "m/s");
}

// TestQuantityValidation

#[test]
fn test_accepts_a_valueunit_and_a_unit() {
    let vt = spec(ValueTypeFamily::Float, 64, 0);
    assert!(Quantity::new("1.0", vt, parse_unit("m").unwrap()).unit == "m");
    assert!(Quantity::new("1.0", vt, u("m")).unit == "m");
    assert!(
        Quantity::new("1.0", vt, Unit::default())
            .unit
            .is_dimensionless()
    );
}

// TestRawValueUnitEqualityMatchesUnit

#[test]
fn test_two_parses_of_the_same_unit_are_equal() {
    assert_eq!(parse_unit("m").unwrap(), parse_unit("m").unwrap());
    assert_eq!(
        parse_unit("k~g·m/s²").unwrap(),
        parse_unit("k~g·m/s²").unwrap()
    );
}

#[test]
fn test_different_units_are_not_equal() {
    assert_ne!(parse_unit("m").unwrap(), parse_unit("k~m").unwrap());
    assert_ne!(parse_unit("N").unwrap(), parse_unit("k~g·m/s²").unwrap());
}

#[test]
fn test_equality_is_the_librarys_not_the_formatters() {
    assert_eq!(
        parse_unit("mmHg").unwrap(),
        parse_unit("ucum:mm[Hg]").unwrap()
    );
    // A UCUM annotation is a comment, never a discriminator (doc/11 3.4).
    assert_eq!(
        parse_unit("ucum:{RBC}/uL").unwrap(),
        parse_unit("ucum:{cells}/uL").unwrap()
    );
}

#[test]
fn test_units_equal_is_exported_and_agrees() {
    let (a, b) = (parse_unit("m").unwrap(), parse_unit("k~m").unwrap());
    assert!(units_equal(&a, &a));
    assert!(!units_equal(&a, &b));
    assert_eq!(units_equal(&a, &b), a == b);
}

#[test]
fn test_hash_agrees_with_unit_so_the_two_share_a_dict() {
    let (vu, un) = (parse_unit("m").unwrap(), u("m"));
    assert!(un == vu && vu == un);
    assert_eq!(h(&un), h(&vu));
    assert_eq!(HashMap::from([(un, 1)])[&vu], 1);
    assert_eq!(HashMap::from([(vu, 2)])[&un], 2);
    assert_eq!(HashSet::from([un, vu]).len(), 1);
}

#[test]
fn test_hash_separates_units_that_differ() {
    assert_ne!(h(&parse_unit("m").unwrap()), h(&parse_unit("k~m").unwrap()));
    let set = HashSet::from([
        parse_unit("m").unwrap(),
        parse_unit("m").unwrap(),
        parse_unit("k~m").unwrap(),
    ]);
    assert_eq!(set.len(), 2);
}

// TestValueTypeSpecEquality

#[test]
fn test_equal_specs_compare_equal() {
    let f = ValueTypeFamily::Float;
    assert_eq!(spec(f, 64, 0), spec(f, 64, 0));
}

#[test]
fn test_differing_specs_do_not() {
    let f = ValueTypeFamily::Float;
    assert_ne!(spec(f, 64, 0), spec(f, 32, 0));
    assert_ne!(spec(f, 64, 0), spec(f, 64, 16));
    assert_ne!(spec(f, 64, 0), spec(ValueTypeFamily::Uint, 64, 0));
}

#[test]
fn test_hashable_and_foreign_types_are_unequal() {
    // The comparison with a string ("float64") does not compile in Rust.
    let f = ValueTypeFamily::Float;
    assert_eq!(h(&spec(f, 64, 0)), h(&spec(f, 64, 0)));
    let set = HashSet::from([spec(f, 64, 0), spec(f, 64, 0), spec(f, 32, 0)]);
    assert_eq!(set.len(), 2);
}

#[test]
fn test_components_compare_by_value() {
    let a = parse_unit("k~m").unwrap().components()[0];
    let b = parse_unit("k~m").unwrap().components()[0];
    let c = parse_unit("m").unwrap().components()[0];
    assert!(a == b && h(&a) == h(&b));
    assert_ne!(a, c);
    assert!(a.prefix_key() == b.prefix_key() && a.prefix_key() != c.prefix_key());
}

#[test]
fn test_unit_and_valueunit_hash_alike_across_the_catalogue() {
    for spelling in [
        "m",
        "k~m",
        "k~g·m/s²",
        "µ~L⁻¹",
        "ucum:mm[Hg]",
        "°C",
        "Ki~B/s",
        "no_unit",
    ] {
        let vu = parse_unit(spelling).unwrap();
        let un = u(spelling);
        assert!(un == vu && vu == un);
        assert_eq!(h(&un), h(&vu), "{spelling}");
        assert_eq!(HashMap::from([(un, 1)])[&vu], 1);
    }
}

// TestAffineIsNotAFactor

#[test]
fn test_convert_factor_raises_for_an_affine_pair() {
    arg_err_contains(u("°C").convert_factor(&u("K")), "convert_value");
}

#[test]
fn test_factor_raises_for_an_affine_unit() {
    arg_err_contains(u("°C").factor(), "unit_to_si_factor");
}

#[test]
fn test_multiplicative_ones_still_answer() {
    assert_eq!(u("k~m").convert_factor(&u("m")).unwrap(), 1000.0);
    assert_eq!(u("in").factor().unwrap(), 0.0254);
    assert_eq!(u("K").factor().unwrap(), 1.0);
}

// TestHashFollowsEquality

#[test]
fn test_prefix_system_is_not_identity_without_a_prefix() {
    let a = parse_unit("B").unwrap();
    let b = Unit::iec(BaseUnit::Byte, IECPrefix::None, Exponent::LINEAR);
    assert_eq!(a, b);
    assert_eq!(h(&a), h(&b));
    assert_eq!(HashSet::from([a, b]).len(), 1);
    assert_eq!(HashMap::from([(a, 1)])[&b], 1);
    assert_eq!(
        a.components()[0].prefix_key(),
        b.components()[0].prefix_key()
    );
    assert_eq!(h(&a.components()[0]), h(&b.components()[0]));
}

#[test]
fn test_a_real_iec_prefix_still_differs() {
    assert_ne!(parse_unit("Ki~B").unwrap(), parse_unit("k~B").unwrap());
    assert_ne!(
        h(&parse_unit("Ki~B").unwrap()),
        h(&parse_unit("k~B").unwrap())
    );
}

// TestUnitIsAcceptedWhereAValueUnitIs

#[test]
fn test_module_level_functions() {
    let q = q(b".x = <float:64,k~m> 5.0;");
    let m = u("m");
    assert_eq!(unit_to_str(&q.unit).unwrap(), "k~m");
    assert_eq!(unit_to_str_ex(&q.unit, 0).unwrap(), "k~m");
    assert!(unit_valid(&q.unit));
    assert!(units_compatible(&q.unit, &m));
    assert!(units_convertible(&q.unit, &m));
    assert!(!units_equal(&q.unit, &m));
    assert_eq!(convert_value(1.0, &q.unit, &m).unwrap(), 1000.0);
    assert_eq!(unit_convert_factor(&q.unit, &m).unwrap().factor, 1000.0);
    assert_eq!(unit_to_si_factor(&q.unit).unwrap().factor, 1000.0);
    assert_eq!(unit_prefix_factor(&q.unit), 1000.0);
    assert_eq!(unit_prefix_exponent(&q.unit), 3);
    assert_eq!(unit_dimension_vector(&q.unit).unwrap()[0], 1);
    assert_eq!(unit_reduce(&q.unit).unwrap().scale, 1000.0);
    assert_eq!(
        unit_to_str(&unit_si_normal_form(&q.unit).unwrap()).unwrap(),
        "m"
    );
    assert_eq!(unit_to_profile("ucum", &q.unit).unwrap(), "km");
    assert_eq!(unit_to_ucum(&q.unit).unwrap(), "km");
    assert!(!unit_is_profile_only(&q.unit));
}

#[test]
fn test_want_unit_may_return_one() {
    let mut out = Vec::new();
    let mut want = |_: &bovnar::EventData<'_>| Some((u("m"), 0));
    let mut on_event = |d: &bovnar::EventData<'_>| {
        if d.event == Event::Data {
            out.push(String::from_utf8_lossy(d.converted_text).into_owned());
        }
        true
    };
    Reader::new()
        .unwrap()
        .read_mem_want_unit(
            b".x = <uint:8,k~m> 5;",
            &ReadOptions::default(),
            &mut want,
            Some(&mut on_event),
        )
        .unwrap();
    assert_eq!(out, ["5000"]);
}

#[test]
fn test_repr_of_a_profile_only_unit_does_not_raise() {
    // Python checks the repr names the opaque atom id and LITER; Rust's
    // components carry the BaseUnit, and BaseUnit::name gives the member name.
    let vu = parse_unit("ucum:[IU]/mL").unwrap();
    let comps = vu.components();
    assert!(comps.iter().any(|c| c.base as i32 == 200000));
    assert!(comps.iter().any(|c| c.base.name() == "LITER"));
    assert_eq!(comps[0].base as i32, 200000);
    let atom = format!("{:?}", comps[0].base);
    assert!(format!("{:?}", comps[0]).contains(&atom), "{atom}");
}

/// Upstream 2.0.0: `ucum:[iU]` is a second spelling of `ucum:[IU]` (UCUM
/// defines `[IU]` = `1 [iU]`), resolved by `gen_profiles.py` from the
/// `.opaque_aliases` list. Both spellings are one unit, written back as
/// `ucum:[IU]`, and a prefix carries over.
#[test]
fn test_ucum_iu_spellings_are_one_unit() {
    assert_eq!(u("ucum:[iU]"), u("ucum:[IU]"));
    assert_eq!(u("ucum:[iU]").to_string(), "ucum:[IU]");
    assert_eq!(u("ucum:k[iU]"), u("ucum:k[IU]"));
    assert_ne!(u("ucum:[IU]"), u("ucum:[arb'U]"));
}
