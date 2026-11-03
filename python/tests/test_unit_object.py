# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
The Unit object and the all-properties accessor shape (1.2).

Both are BREAKING changes and this file is where the new contract is written
down. Before 1.2, Quantity.unit handed back a raw ctypes ValueUnit whose only
useful operations lived elsewhere as module-level functions, and whose `==`
compared by ctypes identity -- so two units that meant the same thing were not
equal. And the value accessors were split between properties (value, epoch_name)
and methods (decimal(), fraction(), fixed_point()) with no rule anyone could
state, so callers guessed and got a TypeError either way.
"""
import pytest

import bovnar
from bovnar import Quantity, Unit
from bovnar.exceptions import BovnarArgumentError


def _q(src: bytes, key: str = "x") -> Quantity:
    return bovnar.loads(src, typed=True)[key]


class TestUnitObject:
    def test_unit_is_a_unit_object(self):
        q = _q(b".x = <float:64,k~m> 5.0;")
        assert isinstance(q.unit, Unit)
        assert str(q.unit) == "k~m"
        assert repr(q.unit) == "Unit('k~m')"

    def test_dimensionless_is_falsey_and_empty(self):
        q = _q(b".x = <float:64> 5.0;")
        assert q.unit.is_dimensionless
        assert not q.unit
        assert str(q.unit) == ""

    def test_percent_is_a_real_unit_not_dimensionless(self):
        """`%` has no dimension but IS a unit; only the absence of a unit is
        dimensionless here. Collapsing the two is how a ratio compares equal to
        a bare count."""
        q = _q(b".x = <float:64,%> 5.0;")
        assert not q.unit.is_dimensionless
        assert bool(q.unit)

    def test_equality_is_by_meaning_not_identity(self):
        """The defect the object exists to fix: ctypes structs compare by
        identity, so two ValueUnits meaning the same thing were never equal."""
        a = _q(b".x = <float:64,m/s> 1.0;").unit
        b = _q(b".x = <float:64,m/s> 2.0;").unit
        assert a == b
        assert hash(a) == hash(b)
        assert a != _q(b".x = <float:64,k~m/s> 1.0;").unit

    def test_equality_accepts_a_string(self):
        q = _q(b".x = <float:64,k~m> 5.0;")
        assert q.unit == "k~m"
        assert q.unit != "m"
        assert q.unit != "not a unit at all"      # unparseable, not an exception

    def test_cross_notation_equality(self):
        """A profile spelling and the native one are the same unit, which is the
        whole promise of doc/11 -- and was not expressible through `==` before."""
        native = Unit.parse("mmHg")
        assert native == Unit.parse("ucum:mm[Hg]")
        assert hash(native) == hash(Unit.parse("ucum:mm[Hg]"))

    def test_factor_and_compatibility(self):
        u = Unit.parse("k~m")
        assert u.factor == 1000.0
        assert u.compatible_with("m")
        assert not u.compatible_with("s")
        assert u.convert_factor("m") == 1000.0

    def test_to_profile(self):
        assert Unit.parse("k~g").to_profile("unece") == "KGM"
        assert Unit.parse("mmHg").to_profile("ucum") == "mm[Hg]"

    def test_profile_only(self):
        u = Unit.parse("ucum:[IU]")
        assert u.is_profile_only
        assert not Unit.parse("m").is_profile_only

    def test_raw_still_reaches_the_struct(self):
        """Nothing that took a ValueUnit is cut off: .raw is the same struct."""
        q = _q(b".x = <float:64,m/s> 1.0;")
        assert bovnar.unit_to_str(q.unit.raw) == "m/s"

    def test_constructor_rejects_nonsense(self):
        with pytest.raises(BovnarArgumentError):
            Unit(42)


class TestAccessorsAreProperties:
    """value, decimal, fraction, fixed_point, stored_value, ieee_bits and
    unit_str are all properties in 2.0. They were split between properties and
    methods with no stateable rule."""

    def test_numeric_accessors(self):
        from decimal import Decimal
        from fractions import Fraction
        q = _q(b".x = <float_dec:64> 3.25;")
        assert q.value == 3.25
        assert q.decimal == Decimal("3.25")
        assert q.fraction == Fraction(13, 4)
        assert q.unit_str == ""

    def test_fixed_point(self):
        q = _q(b".x = <float_fix:32,q8> 3.26953125;")
        mantissa, bits = q.fixed_point
        assert bits == 8
        assert mantissa / (1 << bits) == 3.26953125

    def test_calling_one_is_a_typeerror(self):
        """The point of unifying: there is now exactly one spelling, and the
        other one fails loudly rather than silently returning a bound method."""
        q = _q(b".x = <float:64> 1.0;")
        with pytest.raises(TypeError):
            q.decimal()


class TestQuantityValidation:
    def test_rejects_bad_vtype(self):
        with pytest.raises(BovnarArgumentError, match="ValueTypeSpec"):
            Quantity("1", "nope")

    def test_rejects_bad_raw(self):
        vt = bovnar.make_type_spec(bovnar.ValueTypeFamily.UINT, 64, 0)
        with pytest.raises(BovnarArgumentError, match="raw"):
            Quantity(123, vt)

    def test_rejects_bad_frac(self):
        vt = bovnar.make_type_spec(bovnar.ValueTypeFamily.UINT, 64, 0)
        with pytest.raises(BovnarArgumentError, match="frac"):
            Quantity("1", vt, frac=5)

    def test_accepts_a_unit_string_for_convenience(self):
        vt = bovnar.make_type_spec(bovnar.ValueTypeFamily.FLOAT, 64, 0)
        q = Quantity("1.0", vt, "m/s")
        assert isinstance(q.unit, Unit)
        assert q.unit == "m/s"

    def test_accepts_a_valueunit_and_a_unit(self):
        vt = bovnar.make_type_spec(bovnar.ValueTypeFamily.FLOAT, 64, 0)
        assert Quantity("1.0", vt, bovnar.parse_unit("m")).unit == "m"
        assert Quantity("1.0", vt, Unit.parse("m")).unit == "m"
        assert Quantity("1.0", vt, None).unit.is_dimensionless


class TestRawValueUnitEqualityMatchesUnit:
    """The raw struct compares like the object that wraps it.

    Unit fixed `==` for the type Quantity.unit hands back, but parse_unit --
    the entry point every document and example uses -- still returns a bare
    ValueUnit, and a ctypes Structure defines no __eq__. So the natural Python
    spelling of the natural question silently answered by identity:
    parse_unit("m") == parse_unit("m") was False. Nothing in the repo caught it
    because Quantity.__eq__ compares unit_str and the suite compares formatted
    spellings throughout.
    """

    def test_two_parses_of_the_same_unit_are_equal(self):
        assert bovnar.parse_unit("m") == bovnar.parse_unit("m")
        assert bovnar.parse_unit("k~g·m/s²") == bovnar.parse_unit("k~g·m/s²")

    def test_different_units_are_not_equal(self):
        assert bovnar.parse_unit("m") != bovnar.parse_unit("k~m")
        assert bovnar.parse_unit("N") != bovnar.parse_unit("k~g·m/s²")

    def test_equality_is_the_librarys_not_the_formatters(self):
        # Same unit reached through two vocabularies.
        assert bovnar.parse_unit("mmHg") == bovnar.parse_unit("ucum:mm[Hg]")
        # A UCUM annotation is a comment, never a discriminator (doc/11 3.4).
        assert (bovnar.parse_unit("ucum:{RBC}/uL")
                == bovnar.parse_unit("ucum:{cells}/uL"))

    def test_comparison_with_a_foreign_type_is_false_not_an_error(self):
        assert bovnar.parse_unit("m") != "m"
        assert bovnar.parse_unit("m") != None          # noqa: E711

    def test_units_equal_is_exported_and_agrees(self):
        a, b = bovnar.parse_unit("m"), bovnar.parse_unit("k~m")
        assert bovnar.units_equal(a, a) is True
        assert bovnar.units_equal(a, b) is False
        assert bovnar.units_equal(a, b) == (a == b)

    def test_hash_agrees_with_Unit_so_the_two_share_a_dict(self):
        """Unit.__eq__ accepts a bare ValueUnit and reports True for an equal
        one, so Python's contract requires the two to hash alike."""
        vu, u = bovnar.parse_unit("m"), Unit.parse("m")
        assert u == vu and vu == u
        assert hash(u) == hash(vu)
        assert {u: 1}[vu] == 1
        assert {vu: 2}[u] == 2
        assert len({u, vu}) == 1

    def test_hash_separates_units_that_differ(self):
        assert (hash(bovnar.parse_unit("m"))
                != hash(bovnar.parse_unit("k~m")))
        assert len({bovnar.parse_unit("m"),
                    bovnar.parse_unit("m"),
                    bovnar.parse_unit("k~m")}) == 2


class TestValueTypeSpecEquality:
    """The same ctypes gap, on the other struct a caller compares.

    make_type_spec is exported and doc/09 prints a ValueTypeSpec, so
    `node.value_type == other.value_type` is the obvious way to ask whether two
    values share a type. It answered by identity. Quantity.__eq__ compares
    family, width and base one at a time, which is the workaround.
    """

    def test_equal_specs_compare_equal(self):
        f = bovnar.ValueTypeFamily.FLOAT
        assert bovnar.make_type_spec(f, 64, 0) == bovnar.make_type_spec(f, 64, 0)

    def test_differing_specs_do_not(self):
        f = bovnar.ValueTypeFamily.FLOAT
        assert bovnar.make_type_spec(f, 64, 0) != bovnar.make_type_spec(f, 32, 0)
        assert bovnar.make_type_spec(f, 64, 0) != bovnar.make_type_spec(f, 64, 16)
        assert (bovnar.make_type_spec(f, 64, 0)
                != bovnar.make_type_spec(bovnar.ValueTypeFamily.UINT, 64, 0))

    def test_hashable_and_foreign_types_are_unequal(self):
        f = bovnar.ValueTypeFamily.FLOAT
        assert (hash(bovnar.make_type_spec(f, 64, 0))
                == hash(bovnar.make_type_spec(f, 64, 0)))
        assert len({bovnar.make_type_spec(f, 64, 0),
                    bovnar.make_type_spec(f, 64, 0),
                    bovnar.make_type_spec(f, 32, 0)}) == 2
        assert bovnar.make_type_spec(f, 64, 0) != "float64"

    def test_components_compare_by_value(self):
        a = bovnar.parse_unit("k~m").components[0]
        b = bovnar.parse_unit("k~m").components[0]
        c = bovnar.parse_unit("m").components[0]
        assert a == b and hash(a) == hash(b)
        assert a != c
        assert a.prefix == b.prefix and a.prefix != c.prefix


def test_unit_and_valueunit_hash_alike_across_the_catalogue():
    """One hash implementation, not two that agree today.

    Unit.__hash__ and ValueUnit.__hash__ were separate copies of the same
    multiset formula; nesting the prefix key in one of them was enough to make
    {Unit.parse("m"): 1}[parse_unit("m")] a KeyError. Unit now delegates.
    """
    for spelling in ("m", "k~m", "k~g·m/s²", "µ~L⁻¹", "ucum:mm[Hg]", "°C",
                     "Ki~B/s", "no_unit"):
        vu = bovnar.parse_unit(spelling)
        u = Unit.parse(spelling)
        assert u == vu and vu == u
        assert hash(u) == hash(vu), spelling
        assert {u: 1}[vu] == 1


class TestAffineIsNotAFactor:
    """°C -> K has no multiplicative factor. The module-level function says so
    in `requires_affine`; these two shortcuts returned the bare number and so
    handed back 0.0 and 1.0 as if they were answers."""

    def test_convert_factor_raises_for_an_affine_pair(self):
        with pytest.raises(BovnarArgumentError, match="convert_value"):
            Unit.parse("°C").convert_factor("K")

    def test_factor_raises_for_an_affine_unit(self):
        with pytest.raises(BovnarArgumentError, match="unit_to_si_factor"):
            Unit.parse("°C").factor

    def test_multiplicative_ones_still_answer(self):
        assert Unit.parse("k~m").convert_factor("m") == 1000.0
        assert Unit.parse("in").factor == 0.0254
        assert Unit.parse("K").factor == 1.0


class TestHashFollowsEquality:
    def test_prefix_system_is_not_identity_without_a_prefix(self):
        """`B` from the parser is SI-system/no-prefix, `make_unit_iec(BYTE)` is
        IEC-system/no-prefix; bvn_unit_equal calls them equal and the hash
        keyed on the system, so one unit was two dict keys."""
        from bovnar import make_unit_iec, parse_unit, BaseUnit
        a, b = parse_unit("B"), make_unit_iec(BaseUnit.BYTE)
        assert a == b
        assert hash(a) == hash(b)
        assert len({Unit(a), Unit(b)}) == 1
        assert {Unit(a): 1}[Unit(b)] == 1
        assert a.components[0].prefix == b.components[0].prefix
        assert hash(a.components[0]) == hash(b.components[0])

    def test_a_real_iec_prefix_still_differs(self):
        from bovnar import parse_unit
        assert parse_unit("Ki~B") != parse_unit("k~B")
        assert hash(parse_unit("Ki~B")) != hash(parse_unit("k~B"))


class TestUnitIsAcceptedWhereAValueUnitIs:
    """Quantity.unit became a Unit in 2.0 and nothing that takes a unit took
    one: unit_to_str(q.unit) was a ctypes.ArgumentError."""

    def test_module_level_functions(self):
        q = _q(b".x = <float:64,k~m> 5.0;")
        m = Unit.parse("m")
        assert bovnar.unit_to_str(q.unit) == "k~m"
        assert bovnar.unit_to_str_ex(q.unit) == "k~m"
        assert bovnar.unit_valid(q.unit)
        assert bovnar.units_compatible(q.unit, m)
        assert bovnar.units_convertible(q.unit, m)
        assert not bovnar.units_equal(q.unit, m)
        assert bovnar.convert_value(1.0, q.unit, m) == 1000.0
        assert bovnar.unit_convert_factor(q.unit, m).factor == 1000.0
        assert bovnar.unit_to_si_factor(q.unit).factor == 1000.0
        assert bovnar.unit_prefix_factor(q.unit) == 1000.0
        assert bovnar.unit_prefix_exponent(q.unit) == 3
        assert bovnar.unit_dimension_vector(q.unit)[0] == 1
        assert bovnar.unit_reduce(q.unit).scale == 1000.0
        assert bovnar.unit_to_str(bovnar.unit_si_normal_form(q.unit)) == "m"
        assert bovnar.unit_to_profile("ucum", q.unit) == "km"
        assert bovnar.unit_to_ucum(q.unit) == "km"
        assert not bovnar.unit_is_profile_only(q.unit)

    def test_want_unit_may_return_one(self):
        out = []
        bovnar.Reader().read_mem(
            b".x = <uint:8,k~m> 5;",
            on_verified=lambda ev, d: out.append(d.converted_str())
            if ev == bovnar.Event.DATA else None,
            want_unit=lambda d: Unit.parse("m"))
        assert out == ["5000"]


def test_repr_of_a_profile_only_unit_does_not_raise():
    """BaseUnit has no member for a profile's opaque atoms, and __repr__ went
    through BaseUnit(...): printing such a unit was a ValueError."""
    vu = bovnar.parse_unit("ucum:[IU]/mL")
    assert "200000" in repr(vu)
    assert "LITER" in repr(vu)
    assert "200000" in repr(vu.active_components()[0])
