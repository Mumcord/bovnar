//! SI factors, compatibility, conversion factors, dimension vectors,
//! reduction and value conversion.
//!
//! The `test_return_type_is_frozen` / `test_reduced_is_frozen` cases check
//! that the result is a value: changing a copy cannot change what the
//! library reports.

use std::f64::consts::PI;

use bovnar::units::{
    ReducedUnit, SI_DIM_NAMES, SIConversion, UnitConversion, convert_value, unit_convert_factor,
    unit_dimension_vector, unit_reduce, unit_to_profile, unit_to_si_factor, unit_to_str,
    units_compatible,
};
use bovnar::{BaseUnit, Error, Exponent, SIPrefix, Unit};

/// `math.isclose` with `abs_tol=0`.
fn isclose(a: f64, b: f64, rel_tol: f64) -> bool {
    isclose_abs(a, b, rel_tol, 0.0)
}

fn isclose_abs(a: f64, b: f64, rel_tol: f64, abs_tol: f64) -> bool {
    a == b || (a - b).abs() <= (rel_tol * a.abs().max(b.abs())).max(abs_tol)
}

fn p(s: &str) -> Unit {
    bovnar::units::parse_unit(s).unwrap()
}

fn si(u: Unit) -> SIConversion {
    unit_to_si_factor(&u).unwrap()
}

fn dims(u: Unit) -> [i32; 7] {
    unit_dimension_vector(&u).unwrap()
}

fn compat(a: &str, b: &str) -> bool {
    units_compatible(&p(a), &p(b))
}

fn meter() -> Unit {
    Unit::si(BaseUnit::Meter, SIPrefix::None, Exponent::LINEAR)
}

fn km() -> Unit {
    Unit::si(BaseUnit::Meter, SIPrefix::Kilo, Exponent::LINEAR)
}

fn ms() -> Unit {
    Unit::si(BaseUnit::Second, SIPrefix::Milli, Exponent::LINEAR)
}

fn kelvin() -> Unit {
    Unit::si(BaseUnit::Kelvin, SIPrefix::None, Exponent::LINEAR)
}

fn celsius() -> Unit {
    Unit::si(BaseUnit::Celsius, SIPrefix::None, Exponent::LINEAR)
}

fn gram() -> Unit {
    Unit::si(BaseUnit::Gram, SIPrefix::None, Exponent::LINEAR)
}

fn kg() -> Unit {
    Unit::si(BaseUnit::Gram, SIPrefix::Kilo, Exponent::LINEAR)
}

fn m_per_s() -> Unit {
    p("m/s")
}

fn force() -> Unit {
    p("k~g·m/s²")
}

fn is_argument_error<T: std::fmt::Debug>(r: Result<T, Error>) -> bool {
    matches!(r, Err(Error::Argument { .. }))
}

mod si_dim_names {
    use super::*;

    #[test]
    fn has_seven_entries() {
        assert_eq!(SI_DIM_NAMES.len(), 7);
    }

    #[test]
    fn contains_metre() {
        assert!(SI_DIM_NAMES.contains(&"m"));
    }

    #[test]
    fn contains_kilogram() {
        assert!(SI_DIM_NAMES.contains(&"kg"));
    }

    #[test]
    fn order_si() {
        assert_eq!(SI_DIM_NAMES[0], "m");
        assert_eq!(SI_DIM_NAMES[1], "kg");
        assert_eq!(SI_DIM_NAMES[2], "s");
    }
}

mod unit_to_si_factor_tests {
    use super::*;

    #[test]
    fn meter_factor_one() {
        let conv: SIConversion = si(meter());
        assert!(isclose(conv.factor, 1.0, 1e-9));
        assert!(!conv.is_affine);
    }

    #[test]
    fn kilometer_factor_1000() {
        let conv = si(km());
        assert!(isclose(conv.factor, 1000.0, 1e-9));
        assert!(!conv.is_affine);
    }

    #[test]
    fn millisecond_factor() {
        assert!(isclose(si(ms()).factor, 1e-3, 1e-9));
    }

    #[test]
    fn kelvin_not_affine() {
        let conv = si(kelvin());
        assert!(!conv.is_affine);
        assert!(isclose(conv.factor, 1.0, 1e-9));
    }

    #[test]
    fn celsius_is_affine() {
        let conv = si(celsius());
        assert!(conv.is_affine);
        assert!(isclose(conv.affine_offset, 273.15, 1e-6));
    }

    #[test]
    fn gram_factor() {
        assert!(isclose(si(gram()).factor, 1e-3, 1e-9));
    }

    #[test]
    fn kilogram_factor_one() {
        assert!(isclose(si(kg()).factor, 1.0, 1e-9));
    }

    #[test]
    fn return_type_is_frozen() {
        let conv = si(meter());
        let mut copy = conv;
        copy.factor = 999.0;
        assert_eq!(si(meter()), conv);
        assert_ne!(copy, conv);
    }

    #[test]
    fn dimensionless_factor() {
        assert!(isclose(si(Unit::dimensionless()).factor, 1.0, 1e-9));
    }
}

mod units_compatible_tests {
    use super::*;

    #[test]
    fn meter_and_km_compatible() {
        assert!(units_compatible(&meter(), &km()));
    }

    #[test]
    fn meter_and_gram_incompatible() {
        assert!(!units_compatible(&meter(), &gram()));
    }

    #[test]
    fn meter_and_meter_compatible() {
        assert!(units_compatible(&meter(), &meter()));
    }

    #[test]
    fn kelvin_and_celsius_compatible() {
        assert!(units_compatible(&kelvin(), &celsius()));
    }

    #[test]
    fn kg_and_gram_compatible() {
        assert!(units_compatible(&kg(), &gram()));
    }

    #[test]
    fn m_per_s_and_km_per_h_compatible() {
        assert!(units_compatible(&p("m/s"), &p("k~m/h")));
    }

    #[test]
    fn force_and_meter_incompatible() {
        assert!(!units_compatible(&force(), &meter()));
    }
}

mod unit_convert_factor_tests {
    use super::*;

    #[test]
    fn km_to_m() {
        let conv: UnitConversion = unit_convert_factor(&km(), &meter()).unwrap();
        assert!(isclose(conv.factor, 1000.0, 1e-9));
    }

    #[test]
    fn m_to_km() {
        let conv = unit_convert_factor(&meter(), &km()).unwrap();
        assert!(isclose(conv.factor, 1e-3, 1e-9));
    }

    #[test]
    fn same_unit_factor_one() {
        let conv = unit_convert_factor(&meter(), &meter()).unwrap();
        assert!(isclose(conv.factor, 1.0, 1e-9));
    }

    #[test]
    fn gram_to_kg() {
        let conv = unit_convert_factor(&gram(), &kg()).unwrap();
        assert!(isclose(conv.factor, 1e-3, 1e-9));
    }

    #[test]
    fn kg_to_gram() {
        let conv = unit_convert_factor(&kg(), &gram()).unwrap();
        assert!(isclose(conv.factor, 1000.0, 1e-9));
    }

    #[test]
    fn celsius_to_kelvin_requires_affine() {
        let conv = unit_convert_factor(&celsius(), &kelvin()).unwrap();
        assert!(conv.requires_affine);
    }

    #[test]
    fn incompatible_raises() {
        assert!(is_argument_error(unit_convert_factor(&meter(), &kg())));
    }

    #[test]
    fn return_type_is_frozen() {
        let conv = unit_convert_factor(&meter(), &km()).unwrap();
        let mut copy = conv;
        copy.factor = 0.0;
        assert_eq!(unit_convert_factor(&meter(), &km()).unwrap(), conv);
        assert_ne!(copy, conv);
    }
}

mod unit_dimension_vector_tests {
    use super::*;

    #[test]
    fn meter_is_length() {
        let v = dims(meter());
        assert_eq!(v.len(), 7);
        assert_eq!(v[0], 1);
        assert!(v[1..].iter().all(|&x| x == 0));
    }

    #[test]
    fn gram_is_mass() {
        let v = dims(gram());
        assert_eq!(v[1], 1);
        assert_eq!(v[0], 0);
    }

    #[test]
    fn second_is_time() {
        let v = dims(p("s"));
        assert_eq!(v[2], 1);
        assert_eq!(v[0], 0);
        assert_eq!(v[1], 0);
    }

    #[test]
    fn m_per_s_velocity() {
        let v = dims(m_per_s());
        assert_eq!(v[0], 1);
        assert_eq!(v[2], -1);
    }

    #[test]
    fn force_kg_m_s2() {
        let v = dims(force());
        assert_eq!(v[0], 1);
        assert_eq!(v[1], 1);
        assert_eq!(v[2], -2);
    }

    #[test]
    fn dimensionless_all_zero() {
        assert!(dims(Unit::dimensionless()).iter().all(|&x| x == 0));
    }

    #[test]
    fn km_same_dims_as_m() {
        assert_eq!(dims(meter()), dims(km()));
    }

    #[test]
    fn incompatible_dim_vectors_differ() {
        assert_ne!(dims(meter()), dims(gram()));
    }
}

mod unit_reduce_tests {
    use super::*;

    #[test]
    fn meter_reduces_to_itself() {
        let r: ReducedUnit = unit_reduce(&meter()).unwrap();
        assert!(isclose(r.scale, 1.0, 1e-9));
    }

    #[test]
    fn km_scale_1000() {
        assert!(isclose(unit_reduce(&km()).unwrap().scale, 1000.0, 1e-9));
    }

    #[test]
    fn gram_scale() {
        // bvn_unit_reduce strips explicit SI prefixes only; a bare gram has
        // none, so it reduces to itself with scale 1.
        assert!(isclose(unit_reduce(&gram()).unwrap().scale, 1.0, 1e-9));
    }

    #[test]
    fn reduced_is_frozen() {
        let r = unit_reduce(&meter()).unwrap();
        let mut copy = r;
        copy.scale = 999.0;
        assert_eq!(unit_reduce(&meter()).unwrap(), r);
        assert_ne!(copy, r);
    }

    #[test]
    fn result_has_unit_and_scale() {
        let r = unit_reduce(&km()).unwrap();
        let _: Unit = r.unit;
        let _: f64 = r.scale;
    }
}

mod convert_value_tests {
    use super::*;

    #[test]
    fn km_to_m() {
        assert!(isclose(
            convert_value(5.0, &km(), &meter()).unwrap(),
            5000.0,
            1e-9
        ));
    }

    #[test]
    fn m_to_km() {
        assert!(isclose(
            convert_value(3000.0, &meter(), &km()).unwrap(),
            3.0,
            1e-9
        ));
    }

    #[test]
    fn identity() {
        assert!(isclose(
            convert_value(42.0, &meter(), &meter()).unwrap(),
            42.0,
            1e-9
        ));
    }

    #[test]
    fn celsius_to_kelvin() {
        assert!(isclose(
            convert_value(0.0, &celsius(), &kelvin()).unwrap(),
            273.15,
            1e-6
        ));
    }

    #[test]
    fn celsius_100_to_kelvin() {
        assert!(isclose(
            convert_value(100.0, &celsius(), &kelvin()).unwrap(),
            373.15,
            1e-6
        ));
    }

    #[test]
    fn kelvin_to_celsius() {
        let v = convert_value(273.15, &kelvin(), &celsius()).unwrap();
        assert!(isclose_abs(v, 0.0, 1e-9, 1e-9));
    }

    #[test]
    fn incompatible_raises() {
        assert!(is_argument_error(convert_value(1.0, &meter(), &kg())));
    }

    #[test]
    fn gram_to_kg() {
        assert!(isclose(
            convert_value(1000.0, &gram(), &kg()).unwrap(),
            1.0,
            1e-9
        ));
    }
}

mod new_unit_si_factors {
    use super::*;

    #[test]
    fn kgf_factor() {
        assert!(isclose(si(p("kgf")).factor, 9.80665, 1e-9));
    }

    #[test]
    fn inhg_factor() {
        assert!(isclose(si(p("inHg")).factor, 3386.388645, 1e-7));
    }

    #[test]
    fn rpm_factor() {
        assert!(isclose(si(p("rpm")).factor, 1.0 / 60.0, 1e-12));
    }

    #[test]
    fn ft_lb_factor() {
        assert!(isclose(si(p("ft_lb")).factor, 1.3558179483, 1e-9));
    }

    #[test]
    fn dr_factor() {
        assert!(isclose(si(p("dr")).factor, 1.7718451953125e-3, 1e-12));
    }

    #[test]
    fn dwt_factor() {
        assert!(isclose(si(p("dwt")).factor, 1.55517384e-3, 1e-9));
    }

    #[test]
    fn chain_factor() {
        assert!(isclose(si(p("ch")).factor, 20.1168, 1e-9));
    }

    #[test]
    fn rod_factor() {
        assert!(isclose(si(p("rd")).factor, 5.0292, 1e-9));
    }

    #[test]
    fn gill_factor() {
        assert!(isclose(si(p("gi")).factor, 1.18294118250e-4, 1e-9));
    }

    #[test]
    fn gill_uk_factor() {
        assert!(isclose(si(p("gi_uk")).factor, 1.420653125e-4, 1e-9));
    }

    #[test]
    fn kgf_not_affine() {
        let conv = si(p("kgf"));
        assert!(!conv.is_affine);
    }

    #[test]
    fn rpm_not_affine() {
        let conv = si(p("rpm"));
        assert!(!conv.is_affine);
    }
}

mod new_unit_dimension_vectors {
    use super::*;

    #[test]
    fn kgf_dims() {
        assert_eq!(dims(p("kgf")), [1, 1, -2, 0, 0, 0, 0]);
    }

    #[test]
    fn inhg_dims() {
        assert_eq!(dims(p("inHg")), [-1, 1, -2, 0, 0, 0, 0]);
    }

    #[test]
    fn rpm_dims() {
        assert_eq!(dims(p("rpm")), [0, 0, -1, 0, 0, 0, 0]);
    }

    #[test]
    fn ft_lb_dims() {
        assert_eq!(dims(p("ft_lb")), [2, 1, -2, 0, 0, 0, 0]);
    }

    #[test]
    fn dr_dims() {
        assert_eq!(dims(p("dr")), [0, 1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn dwt_dims() {
        assert_eq!(dims(p("dwt")), [0, 1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn chain_dims() {
        assert_eq!(dims(p("ch")), [1, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn rod_dims() {
        assert_eq!(dims(p("rd")), [1, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn gill_dims() {
        assert_eq!(dims(p("gi")), [3, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn gill_uk_dims() {
        assert_eq!(dims(p("gi_uk")), [3, 0, 0, 0, 0, 0, 0]);
    }
}

mod new_units_dimensions {
    use super::*;

    #[test]
    fn gn_dims_acceleration() {
        assert_eq!(dims(p("gn")), [1, 0, -2, 0, 0, 0, 0]);
    }

    #[test]
    fn ps_dims_power() {
        assert_eq!(dims(p("PS")), [2, 1, -3, 0, 0, 0, 0]);
    }

    #[test]
    fn rev_dims_dimensionless() {
        assert_eq!(dims(p("rev")), [0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn mo_dims_time() {
        assert_eq!(dims(p("mo")), [0, 0, 1, 0, 0, 0, 0]);
    }

    #[test]
    fn fn_dims_time() {
        assert_eq!(dims(p("fn")), [0, 0, 1, 0, 0, 0, 0]);
    }

    #[test]
    fn at_dims_pressure() {
        assert_eq!(dims(p("at")), [-1, 1, -2, 0, 0, 0, 0]);
    }

    #[test]
    fn tex_dims_linear_density() {
        assert_eq!(dims(p("tex")), [-1, 1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn den_dims_linear_density() {
        assert_eq!(dims(p("den")), [-1, 1, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn fl_dr_dims_volume() {
        assert_eq!(dims(p("fl_dr")), [3, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn minim_dims_volume() {
        assert_eq!(dims(p("minim")), [3, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn pk_dims_volume() {
        assert_eq!(dims(p("pk")), [3, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn bsh_dims_volume() {
        assert_eq!(dims(p("bsh")), [3, 0, 0, 0, 0, 0, 0]);
    }
}

mod new_unit_conversions {
    use super::*;

    #[test]
    fn kgf_to_newton() {
        let v = convert_value(1.0, &p("kgf"), &p("N")).unwrap();
        assert!(isclose(v, 9.80665, 1e-9));
    }

    #[test]
    fn newton_to_kgf() {
        let v = convert_value(9.80665, &p("N"), &p("kgf")).unwrap();
        assert!(isclose(v, 1.0, 1e-9));
    }

    #[test]
    fn kgf_to_lbf() {
        let v = convert_value(1.0, &p("kgf"), &p("lbf")).unwrap();
        assert!(isclose(v, 2.20462262185, 1e-6));
    }

    #[test]
    fn inhg_to_pa() {
        let v = convert_value(1.0, &p("inHg"), &p("Pa")).unwrap();
        assert!(isclose(v, 3386.388645, 1e-6));
    }

    #[test]
    fn standard_atm_in_inhg() {
        let v = convert_value(101325.0, &p("Pa"), &p("inHg")).unwrap();
        assert!(isclose(v, 29.9212, 1e-4));
    }

    #[test]
    fn inhg_to_mmhg() {
        let v = convert_value(1.0, &p("inHg"), &p("mmHg")).unwrap();
        assert!(isclose(v, 25.4, 1e-6));
    }

    #[test]
    fn rpm_to_hz() {
        let v = convert_value(60.0, &p("rpm"), &p("Hz")).unwrap();
        assert!(isclose(v, 1.0, 1e-12));
    }

    #[test]
    fn rpm_3600_to_60hz() {
        let v = convert_value(3600.0, &p("rpm"), &p("Hz")).unwrap();
        assert!(isclose(v, 60.0, 1e-12));
    }

    #[test]
    fn hz_to_rpm() {
        let v = convert_value(50.0, &p("Hz"), &p("rpm")).unwrap();
        assert!(isclose(v, 3000.0, 1e-12));
    }

    #[test]
    fn ft_lb_to_joule() {
        let v = convert_value(1.0, &p("ft_lb"), &p("J")).unwrap();
        assert!(isclose(v, 1.3558179483, 1e-9));
    }

    #[test]
    fn joule_to_ft_lb() {
        let v = convert_value(1.3558179483, &p("J"), &p("ft_lb")).unwrap();
        assert!(isclose(v, 1.0, 1e-9));
    }

    #[test]
    fn ft_lb_to_cal() {
        let v = convert_value(1.0, &p("ft_lb"), &p("cal")).unwrap();
        assert!(isclose(v, 1.3558179483 / 4.184, 1e-9));
    }

    #[test]
    fn test_16_dr_equals_1_oz() {
        let v = convert_value(16.0, &p("dr"), &p("oz")).unwrap();
        assert!(isclose(v, 1.0, 1e-10));
    }

    #[test]
    fn dr_to_gram() {
        let v = convert_value(1.0, &p("dr"), &p("g")).unwrap();
        assert!(isclose(v, 1.7718451953125, 1e-10));
    }

    #[test]
    fn test_20_dwt_equals_1_oz_t() {
        let v = convert_value(20.0, &p("dwt"), &p("oz_t")).unwrap();
        assert!(isclose(v, 1.0, 1e-10));
    }

    #[test]
    fn dwt_to_gram() {
        let v = convert_value(1.0, &p("dwt"), &p("g")).unwrap();
        assert!(isclose(v, 1.55517384, 1e-9));
    }

    #[test]
    fn test_80_chains_equals_1_mile() {
        let v = convert_value(80.0, &p("ch"), &p("mi")).unwrap();
        assert!(isclose(v, 1.0, 1e-9));
    }

    #[test]
    fn chain_to_feet() {
        let v = convert_value(1.0, &p("ch"), &p("ft")).unwrap();
        assert!(isclose(v, 66.0, 1e-9));
    }

    #[test]
    fn test_4_rods_equals_1_chain() {
        let v = convert_value(4.0, &p("rd"), &p("ch")).unwrap();
        assert!(isclose(v, 1.0, 1e-9));
    }

    #[test]
    fn rod_to_feet() {
        let v = convert_value(1.0, &p("rd"), &p("ft")).unwrap();
        assert!(isclose(v, 16.5, 1e-9));
    }

    #[test]
    fn test_4_gill_equals_1_pint() {
        let v = convert_value(4.0, &p("gi"), &p("pt")).unwrap();
        assert!(isclose(v, 1.0, 1e-7));
    }

    #[test]
    fn gill_to_fl_oz() {
        let v = convert_value(1.0, &p("gi"), &p("fl_oz")).unwrap();
        assert!(isclose(v, 4.0, 1e-9));
    }

    #[test]
    fn test_4_gill_uk_equals_1_pt_uk() {
        let v = convert_value(4.0, &p("gi_uk"), &p("pt_uk")).unwrap();
        assert!(isclose(v, 1.0, 1e-9));
    }

    #[test]
    fn test_32_gill_uk_equals_1_gal_uk() {
        let v = convert_value(32.0, &p("gi_uk"), &p("gal_uk")).unwrap();
        assert!(isclose(v, 1.0, 1e-9));
    }

    #[test]
    fn gill_to_ml() {
        let v = convert_value(1.0, &p("gi"), &p("m~L")).unwrap();
        assert!(isclose(v, 118.294118750, 1e-7));
    }

    #[test]
    fn gill_uk_to_ml() {
        let v = convert_value(1.0, &p("gi_uk"), &p("m~L")).unwrap();
        assert!(isclose(v, 142.0653125, 1e-7));
    }
}

mod new_units_convert_value {
    use super::*;

    #[test]
    fn gn_to_m_per_s2() {
        let v = convert_value(1.0, &p("gn"), &p("m/s²")).unwrap();
        assert!(isclose(v, 9.80665, 1e-10));
    }

    #[test]
    fn gn_to_gal() {
        let v = convert_value(1.0, &p("gn"), &p("Gal")).unwrap();
        assert!(isclose(v, 980.665, 1e-9));
    }

    #[test]
    fn ps_to_watt() {
        let v = convert_value(1.0, &p("PS"), &p("W")).unwrap();
        assert!(isclose(v, 735.49875, 1e-10));
    }

    #[test]
    fn ps_to_hp() {
        let v = convert_value(1.0, &p("PS"), &p("hp")).unwrap();
        assert!(isclose(v, 735.49875 / 745.69987158227, 1e-9));
    }

    #[test]
    fn hp_to_ps() {
        let v = convert_value(1.0, &p("hp"), &p("PS")).unwrap();
        assert!(isclose(v, 745.69987158227 / 735.49875, 1e-9));
    }

    #[test]
    fn test_1_rev_is_360_degrees() {
        let v = convert_value(1.0, &p("rev"), &p("deg")).unwrap();
        assert!(isclose(v, 360.0, 1e-10));
    }

    #[test]
    fn test_1_rev_is_2pi_radians() {
        let v = convert_value(1.0, &p("rev"), &p("rad")).unwrap();
        assert!(isclose(v, 2.0 * PI, 1e-12));
    }

    #[test]
    fn test_1_rev_is_400_grad() {
        let v = convert_value(1.0, &p("rev"), &p("grad")).unwrap();
        assert!(isclose(v, 400.0, 1e-10));
    }

    #[test]
    fn test_12_months_equals_1_julian_year() {
        let v = convert_value(12.0, &p("mo"), &p("yr")).unwrap();
        assert!(isclose(v, 1.0, 1e-12));
    }

    #[test]
    fn test_2_fortnights_equals_1_month_approx() {
        let v = convert_value(2.0, &p("fn"), &p("mo")).unwrap();
        assert!(isclose(v, 2419200.0 / 2629800.0, 1e-10));
    }

    #[test]
    fn fortnight_to_days() {
        let v = convert_value(1.0, &p("fn"), &p("d")).unwrap();
        assert!(isclose(v, 14.0, 1e-12));
    }

    #[test]
    fn test_2_weeks_equals_1_fortnight() {
        let v = convert_value(2.0, &p("wk"), &p("fn")).unwrap();
        assert!(isclose(v, 1.0, 1e-12));
    }

    #[test]
    fn at_to_pa() {
        let v = convert_value(1.0, &p("at"), &p("Pa")).unwrap();
        assert!(isclose(v, 98066.5, 1e-10));
    }

    #[test]
    fn at_vs_atm() {
        let v = convert_value(1.0, &p("at"), &p("atm")).unwrap();
        assert!(isclose(v, 98066.5 / 101325.0, 1e-10));
    }

    #[test]
    fn test_9_den_equals_1_tex() {
        let v = convert_value(9.0, &p("den"), &p("tex")).unwrap();
        assert!(isclose(v, 1.0, 1e-9));
    }

    #[test]
    fn tex_to_den() {
        let v = convert_value(1.0, &p("tex"), &p("den")).unwrap();
        assert!(isclose(v, 9.0, 1e-9));
    }

    #[test]
    fn test_8_fl_dr_equals_1_fl_oz() {
        let v = convert_value(8.0, &p("fl_dr"), &p("fl_oz")).unwrap();
        assert!(isclose(v, 1.0, 1e-12));
    }

    #[test]
    fn fl_dr_to_ml() {
        let v = convert_value(1.0, &p("fl_dr"), &p("m~L")).unwrap();
        assert!(isclose(v, 3.6966911953125, 1e-10));
    }

    #[test]
    fn test_60_minims_equals_1_fl_dr() {
        let v = convert_value(60.0, &p("minim"), &p("fl_dr")).unwrap();
        assert!(isclose(v, 1.0, 1e-12));
    }

    #[test]
    fn test_480_minims_equals_1_fl_oz() {
        let v = convert_value(480.0, &p("minim"), &p("fl_oz")).unwrap();
        assert!(isclose(v, 1.0, 1e-10));
    }

    #[test]
    fn minim_to_ul() {
        let v = convert_value(1.0, &p("minim"), &p("µ~L")).unwrap();
        assert!(isclose(v, 61.6115199218750, 1e-9));
    }

    #[test]
    fn test_4_pecks_equals_1_bushel() {
        let v = convert_value(4.0, &p("pk"), &p("bsh")).unwrap();
        assert!(isclose(v, 1.0, 1e-10));
    }

    #[test]
    fn peck_to_liter() {
        let v = convert_value(1.0, &p("pk"), &p("L")).unwrap();
        assert!(isclose(v, 8.80976754172, 1e-9));
    }

    #[test]
    fn bushel_to_liter() {
        let v = convert_value(1.0, &p("bsh"), &p("L")).unwrap();
        assert!(isclose(v, 35.23907016688, 1e-9));
    }

    #[test]
    fn bushel_to_peck() {
        let v = convert_value(1.0, &p("bsh"), &p("pk")).unwrap();
        assert!(isclose(v, 4.0, 1e-10));
    }

    #[test]
    fn kps_to_kw() {
        let v = convert_value(1.0, &p("k~PS"), &p("k~W")).unwrap();
        assert!(isclose(v, 735.49875, 1e-9));
    }

    #[test]
    fn prefixed_gn_milli() {
        let conv = si(p("m~gn"));
        assert!(isclose(conv.factor, 9.80665e-3, 1e-10));
    }
}

mod new_units_parsing_roundtrip {
    use super::*;

    #[test]
    fn parse_gn() {
        let vu = p("gn");
        assert_eq!(vu.components()[0].base, BaseUnit::StandardGravity);
    }

    #[test]
    fn parse_standard_gravity() {
        let vu = p("standard_gravity");
        assert_eq!(vu.components()[0].base, BaseUnit::StandardGravity);
    }

    #[test]
    fn parse_ps() {
        let vu = p("PS");
        assert_eq!(vu.components()[0].base, BaseUnit::MetricHorsepower);
    }

    #[test]
    fn parse_cv() {
        let vu = p("CV");
        assert_eq!(vu.components()[0].base, BaseUnit::MetricHorsepower);
    }

    #[test]
    fn parse_metric_horsepower() {
        let vu = p("metric_horsepower");
        assert_eq!(vu.components()[0].base, BaseUnit::MetricHorsepower);
    }

    #[test]
    fn parse_rev() {
        let vu = p("rev");
        assert_eq!(vu.components()[0].base, BaseUnit::Revolution);
    }

    #[test]
    fn parse_turn() {
        let vu = p("turn");
        assert_eq!(vu.components()[0].base, BaseUnit::Revolution);
    }

    #[test]
    fn parse_revolution() {
        let vu = p("revolution");
        assert_eq!(vu.components()[0].base, BaseUnit::Revolution);
    }

    #[test]
    fn parse_revolutions() {
        let vu = p("revolutions");
        assert_eq!(vu.components()[0].base, BaseUnit::Revolution);
    }

    #[test]
    fn parse_turns() {
        let vu = p("turns");
        assert_eq!(vu.components()[0].base, BaseUnit::Revolution);
    }

    #[test]
    fn parse_mo() {
        let vu = p("mo");
        assert_eq!(vu.components()[0].base, BaseUnit::Month);
    }

    #[test]
    fn parse_month() {
        let vu = p("month");
        assert_eq!(vu.components()[0].base, BaseUnit::Month);
    }

    #[test]
    fn parse_months() {
        let vu = p("months");
        assert_eq!(vu.components()[0].base, BaseUnit::Month);
    }

    #[test]
    fn parse_fn() {
        let vu = p("fn");
        assert_eq!(vu.components()[0].base, BaseUnit::Fortnight);
    }

    #[test]
    fn parse_fortnight() {
        let vu = p("fortnight");
        assert_eq!(vu.components()[0].base, BaseUnit::Fortnight);
    }

    #[test]
    fn parse_fortnights() {
        let vu = p("fortnights");
        assert_eq!(vu.components()[0].base, BaseUnit::Fortnight);
    }

    #[test]
    fn parse_at() {
        let vu = p("at");
        assert_eq!(vu.components()[0].base, BaseUnit::AtmosphereTechnical);
    }

    #[test]
    fn parse_atmosphere_technical() {
        let vu = p("atmosphere_technical");
        assert_eq!(vu.components()[0].base, BaseUnit::AtmosphereTechnical);
    }

    #[test]
    fn parse_tex() {
        let vu = p("tex");
        assert_eq!(vu.components()[0].base, BaseUnit::Tex);
    }

    #[test]
    fn parse_den() {
        let vu = p("den");
        assert_eq!(vu.components()[0].base, BaseUnit::Denier);
    }

    #[test]
    fn parse_denier() {
        let vu = p("denier");
        assert_eq!(vu.components()[0].base, BaseUnit::Denier);
    }

    #[test]
    fn parse_deniers() {
        let vu = p("deniers");
        assert_eq!(vu.components()[0].base, BaseUnit::Denier);
    }

    #[test]
    fn parse_fl_dr() {
        let vu = p("fl_dr");
        assert_eq!(vu.components()[0].base, BaseUnit::FluidDram);
    }

    #[test]
    fn parse_fluid_dram() {
        let vu = p("fluid_dram");
        assert_eq!(vu.components()[0].base, BaseUnit::FluidDram);
    }

    #[test]
    fn parse_fluid_drams() {
        let vu = p("fluid_drams");
        assert_eq!(vu.components()[0].base, BaseUnit::FluidDram);
    }

    #[test]
    fn parse_minim() {
        let vu = p("minim");
        assert_eq!(vu.components()[0].base, BaseUnit::Minim);
    }

    #[test]
    fn parse_minims() {
        let vu = p("minims");
        assert_eq!(vu.components()[0].base, BaseUnit::Minim);
    }

    #[test]
    fn parse_pk() {
        let vu = p("pk");
        assert_eq!(vu.components()[0].base, BaseUnit::Peck);
    }

    #[test]
    fn parse_peck() {
        let vu = p("peck");
        assert_eq!(vu.components()[0].base, BaseUnit::Peck);
    }

    #[test]
    fn parse_pecks() {
        let vu = p("pecks");
        assert_eq!(vu.components()[0].base, BaseUnit::Peck);
    }

    #[test]
    fn parse_bsh() {
        let vu = p("bsh");
        assert_eq!(vu.components()[0].base, BaseUnit::Bushel);
    }

    #[test]
    fn parse_bushel() {
        let vu = p("bushel");
        assert_eq!(vu.components()[0].base, BaseUnit::Bushel);
    }

    #[test]
    fn parse_bushels() {
        let vu = p("bushels");
        assert_eq!(vu.components()[0].base, BaseUnit::Bushel);
    }

    #[test]
    fn roundtrip_gn() {
        assert_eq!(unit_to_str(&p("gn")).unwrap(), "gn");
    }

    #[test]
    fn roundtrip_ps() {
        assert_eq!(unit_to_str(&p("PS")).unwrap(), "PS");
    }

    #[test]
    fn roundtrip_rev() {
        assert_eq!(unit_to_str(&p("rev")).unwrap(), "rev");
    }

    #[test]
    fn roundtrip_mo() {
        assert_eq!(unit_to_str(&p("mo")).unwrap(), "mo");
    }

    #[test]
    fn roundtrip_fn() {
        assert_eq!(unit_to_str(&p("fn")).unwrap(), "fn");
    }

    #[test]
    fn roundtrip_at() {
        assert_eq!(unit_to_str(&p("at")).unwrap(), "at");
    }

    #[test]
    fn roundtrip_tex() {
        assert_eq!(unit_to_str(&p("tex")).unwrap(), "tex");
    }

    #[test]
    fn roundtrip_den() {
        assert_eq!(unit_to_str(&p("den")).unwrap(), "den");
    }

    #[test]
    fn roundtrip_fl_dr() {
        assert_eq!(unit_to_str(&p("fl_dr")).unwrap(), "fl_dr");
    }

    #[test]
    fn roundtrip_minim() {
        assert_eq!(unit_to_str(&p("minim")).unwrap(), "minim");
    }

    #[test]
    fn roundtrip_pk() {
        assert_eq!(unit_to_str(&p("pk")).unwrap(), "pk");
    }

    #[test]
    fn roundtrip_bsh() {
        assert_eq!(unit_to_str(&p("bsh")).unwrap(), "bsh");
    }
}

mod new_units_si_factor {
    use super::*;

    #[test]
    fn gn_factor() {
        let conv = si(p("gn"));
        assert!(isclose(conv.factor, 9.80665, 1e-10));
        assert!(!conv.is_affine);
    }

    #[test]
    fn ps_factor() {
        let conv = si(p("PS"));
        assert!(isclose(conv.factor, 735.49875, 1e-10));
        assert!(!conv.is_affine);
    }

    #[test]
    fn rev_factor_is_two_pi() {
        let conv = si(p("rev"));
        assert!(isclose(conv.factor, 2.0 * PI, 1e-12));
        assert!(!conv.is_affine);
    }

    #[test]
    fn mo_factor() {
        let conv = si(p("mo"));
        assert!(isclose(conv.factor, 2629800.0, 1e-12));
        assert!(!conv.is_affine);
    }

    #[test]
    fn fn_factor() {
        let conv = si(p("fn"));
        assert!(isclose(conv.factor, 1209600.0, 1e-12));
        assert!(!conv.is_affine);
    }

    #[test]
    fn at_factor() {
        let conv = si(p("at"));
        assert!(isclose(conv.factor, 98066.5, 1e-10));
        assert!(!conv.is_affine);
    }

    #[test]
    fn tex_factor() {
        let conv = si(p("tex"));
        assert!(isclose(conv.factor, 1e-6, 1e-12));
        assert!(!conv.is_affine);
    }

    #[test]
    fn den_factor() {
        let conv = si(p("den"));
        assert!(isclose(conv.factor, 1.0 / 9000000.0, 1e-10));
        assert!(!conv.is_affine);
    }

    #[test]
    fn fl_dr_factor() {
        let conv = si(p("fl_dr"));
        assert!(isclose(conv.factor, 3.6966911953125e-6, 1e-12));
        assert!(!conv.is_affine);
    }

    #[test]
    fn minim_factor() {
        let conv = si(p("minim"));
        assert!(isclose(conv.factor, 6.16115199218750e-8, 1e-12));
        assert!(!conv.is_affine);
    }

    #[test]
    fn pk_factor() {
        let conv = si(p("pk"));
        assert!(isclose(conv.factor, 8.80976754172e-3, 1e-10));
        assert!(!conv.is_affine);
    }

    #[test]
    fn bsh_factor() {
        let conv = si(p("bsh"));
        assert!(isclose(conv.factor, 3.523907016688e-2, 1e-10));
        assert!(!conv.is_affine);
    }
}

mod new_units_compatibility {
    use super::*;

    #[test]
    fn gn_compatible_with_m_per_s2() {
        assert!(compat("gn", "m/s²"));
    }

    #[test]
    fn gn_compatible_with_gal() {
        assert!(compat("gn", "Gal"));
    }

    #[test]
    fn ps_compatible_with_watt() {
        assert!(compat("PS", "W"));
    }

    #[test]
    fn ps_compatible_with_hp() {
        assert!(compat("PS", "hp"));
    }

    #[test]
    fn rev_compatible_with_rad() {
        assert!(compat("rev", "rad"));
    }

    #[test]
    fn rev_compatible_with_deg() {
        assert!(compat("rev", "deg"));
    }

    #[test]
    fn mo_compatible_with_second() {
        assert!(compat("mo", "s"));
    }

    #[test]
    fn mo_compatible_with_week() {
        assert!(compat("mo", "wk"));
    }

    #[test]
    fn mo_compatible_with_year() {
        assert!(compat("mo", "yr"));
    }

    #[test]
    fn fn_compatible_with_second() {
        assert!(compat("fn", "s"));
    }

    #[test]
    fn fn_compatible_with_week() {
        assert!(compat("fn", "wk"));
    }

    #[test]
    fn fn_compatible_with_month() {
        assert!(compat("fn", "mo"));
    }

    #[test]
    fn at_compatible_with_pa() {
        assert!(compat("at", "Pa"));
    }

    #[test]
    fn at_compatible_with_atm() {
        assert!(compat("at", "atm"));
    }

    #[test]
    fn at_compatible_with_psi() {
        assert!(compat("at", "psi"));
    }

    #[test]
    fn tex_compatible_with_denier() {
        assert!(compat("tex", "den"));
    }

    #[test]
    fn fl_dr_compatible_with_fl_oz() {
        assert!(compat("fl_dr", "fl_oz"));
    }

    #[test]
    fn fl_dr_compatible_with_liter() {
        assert!(compat("fl_dr", "L"));
    }

    #[test]
    fn minim_compatible_with_fl_dr() {
        assert!(compat("minim", "fl_dr"));
    }

    #[test]
    fn minim_compatible_with_liter() {
        assert!(compat("minim", "L"));
    }

    #[test]
    fn pk_compatible_with_liter() {
        assert!(compat("pk", "L"));
    }

    #[test]
    fn pk_compatible_with_bsh() {
        assert!(compat("pk", "bsh"));
    }

    #[test]
    fn pk_compatible_with_gallon() {
        assert!(compat("pk", "gal"));
    }

    #[test]
    fn bsh_compatible_with_liter() {
        assert!(compat("bsh", "L"));
    }

    #[test]
    fn bsh_compatible_with_barrel() {
        assert!(compat("bsh", "bbl"));
    }

    #[test]
    fn gn_incompatible_with_meter() {
        assert!(!compat("gn", "m"));
    }

    #[test]
    fn ps_incompatible_with_joule() {
        assert!(!compat("PS", "J"));
    }

    #[test]
    fn tex_incompatible_with_gram() {
        assert!(!compat("tex", "g"));
    }
}

/// The mean Gregorian month (UCUM `mo_g`, QUDT `MO_MeanGREGORIAN`) is its own
/// unit, 54 s shorter than the Julian `mo`.
#[test]
fn the_mean_gregorian_month() {
    let greg = p("mo_greg");
    assert_eq!(convert_value(1.0, &greg, &p("s")).unwrap(), 2629746.0);
    assert_eq!(
        convert_value(1.0, &p("yr_greg"), &p("s")).unwrap() / 12.0,
        2629746.0
    );
    assert_eq!(p("ucum:mo_g"), greg);
    assert_eq!(p("qudt:MO_MeanGREGORIAN"), greg);
    assert_ne!(p("mo"), greg);
    assert_eq!(convert_value(1.0, &p("mo"), &p("s")).unwrap(), 2629800.0);
    assert_eq!(unit_to_profile("ucum", &greg).unwrap(), "mo_g");
    assert_eq!(unit_to_profile("qudt", &greg).unwrap(), "MO_MeanGREGORIAN");
}
