# SPDX-License-Identifier: MIT
#
# Copyright (c) 2026 Janos Sonntag
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.

"""The reader-side unit policy, through the Python binding.

The point of the feature from here is that none of this needs a callback: the
policy is data, so it crosses ctypes without a per-value trampoline. These
tests are the Python-visible half of tests/bovnar_unit_policy_test.c.
"""

import pytest

from conftest import needs_lib
import bovnar
from bovnar import Reader, UnitPolicy, parse_unit, unit_to_str
from bovnar.enums import Event, ErrorCode, ValueTypeFamily
from bovnar.exceptions import BovnarParseError, BovnarArgumentError


def collect(doc: str, policy: UnitPolicy | None):
    """Read *doc* under *policy*, returning one row per numeric value:
    (converted, converted_text, native_unit, converted_unit)."""
    rows = []

    def on_value(ev, d):
        if ev != Event.DATA or d is None:
            return True
        if d.value_type is None or d.value_type.family not in (
                ValueTypeFamily.UINT, ValueTypeFamily.SINT,
                ValueTypeFamily.FLOAT, ValueTypeFamily.FLOAT_FIX,
                ValueTypeFamily.FLOAT_DEC):
            return True
        rows.append((
            bool(d.converted),
            d.converted_str() if d.converted else None,
            unit_to_str(d.value_unit) if d.value_unit is not None else None,
            unit_to_str(d.conv.unit) if d.converted else None,
        ))
        return True

    with Reader() as r:
        r.set_unit_policy(policy)
        r.read_mem(doc.encode('utf-8'), on_verified=on_value)
    return rows


@needs_lib
class TestTargets:

    def test_mixed_document(self):
        rows = collect(
            ".inlet_temp     = <float:64,°F>      212.0;\n"
            ".tank_level     = <float:64,in>      12.0;\n"
            ".duty_cycle     = <float:64,%>       35.0;\n"
            ".spare_capacity = <float:64>         0.25;\n"
            ".unit_price     = <float:64,k~$USD>  5.0;\n",
            UnitPolicy(targets=["°C", "m"]))
        assert len(rows) == 5
        assert rows[0][:2] == (True, "100")
        assert rows[1][:2] == (True, "0.3048")
        assert rows[2][0] is False          # % matches no target
        assert rows[3][0] is False          # a bare number matches no target
        assert rows[4][0] is False          # a currency is neither

    def test_order_is_significant(self):
        assert collect(".d = 5000 m;", UnitPolicy(targets=["m", "k~m"]))[0][0] is False
        rows = collect(".d = 5000 m;", UnitPolicy(targets=["k~m", "m"]))
        assert rows[0][:2] == (True, "5")

    def test_per_target_output_base(self):
        rows = collect(".d = 5 k~m;", UnitPolicy(targets=[("m", 16)]))
        assert rows[0][:2] == (True, "1388")

    def test_unitless_is_fenced_from_the_ratio_units(self):
        # A bare number is dimensionally a percentage. Delivering 0.25 as 25 is
        # exactly the silent rescale the format exists to prevent.
        assert collect(".x = <float:64> 0.25;", UnitPolicy(targets=["%"]))[0][0] is False
        assert collect(".x = <float:64,%> 35.0;",
                       UnitPolicy(targets=["no_unit"]))[0][0] is False
        # ...while a real ratio-to-ratio conversion is untouched by the fence.
        rows = collect(".x = <float:64,%> 35.0;", UnitPolicy(targets=["ppm"]))
        assert rows[0][:2] == (True, "350000")

    def test_currency_prefix_delta(self):
        # units_compatible says no; units_convertible (and so the policy) says yes.
        rows = collect(".p = <float:64,k~$USD> 5.0;", UnitPolicy(targets=["$USD"]))
        assert rows[0][:2] == (True, "5000")


@needs_lib
class TestExactness:

    def test_nonterminating_raises_by_default(self):
        with pytest.raises(BovnarParseError) as e:
            collect(".v = <float:64,k~m/h> 42.0;", UnitPolicy(targets=["m/s"]))
        assert e.value.code == ErrorCode.UNIT_INEXACT

    def test_leave_inexact_delivers_the_native_value(self):
        rows = collect(".v = <float:64,k~m/h> 42.0;\n.w = <float:64,k~m/h> 36.0;\n",
                       UnitPolicy(targets=["m/s"], leave_inexact=True))
        assert rows[0][0] is False and rows[0][2] == "k~m/h"
        assert rows[1][:2] == (True, "10")     # 36 km/h IS 10 m/s exactly

    def test_irrational_factor(self):
        with pytest.raises(BovnarParseError):
            collect(".a = 90 °;", UnitPolicy(targets=["rad"]))
        rows = collect(".a = 90 °;", UnitPolicy(targets=["rad"], leave_inexact=True))
        assert rows[0][0] is False


@needs_lib
class TestNormalise:

    def test_si(self):
        rows = collect(
            ".len  = <float:64,in>  12.0;\n"
            ".temp = <float:64,°F>  212.0;\n"
            ".mass = <float:64,g>   5.0;\n",
            UnitPolicy(normalise_si=True))
        assert rows[0][1] == "0.3048" and rows[0][3] == "m"
        assert rows[1][1] == "373.15" and rows[1][3] == "K"
        assert rows[2][1] == "0.005"  and rows[2][3] == "k~g"

    def test_dimensionless_and_currency_are_left_alone(self):
        rows = collect(
            ".ratio = <float:64,%>    35.0;\n"
            ".gain  = <float:64,dB>   3.0;\n"
            ".acid  = <float:64,pH>   7.2;\n"
            ".angle = <float:64,°>    90.0;\n"
            ".money = <float:64,$USD> 5.0;\n",
            UnitPolicy(normalise_si=True, leave_inexact=True))
        assert [r[0] for r in rows] == [False] * 5

    def test_photometric_units_are_left_alone(self):
        # lm/lx/ph carry the steradian's quantity kind, which no dimension
        # vector can express; rebuilding from dimensions alone would produce cd,
        # which is a different quantity and which the converter refuses.
        rows = collect(".flux = <float:64,lm> 800.0;\n.d = <float:64,k~m> 5.0;\n",
                       UnitPolicy(normalise_si=True))
        assert rows[0][0] is False
        assert rows[1][:2] == (True, "5000")

    def test_targets_outrank_normalise(self):
        rows = collect(".v = <float:64,m/s> 10.0;\n.len = <float:64,in> 12.0;\n",
                       UnitPolicy(targets=["k~m/h"], normalise_si=True))
        assert rows[0][:2] == (True, "36")       # target won
        assert rows[1][:2] == (True, "0.3048")   # fell through to normalise


@needs_lib
class TestAssertions:

    def test_require_unit(self):
        assert len(collect(".a = 5 m;\n.b = 25 °C;\n",
                           UnitPolicy(require_unit=True))) == 2
        for doc in (".a = 5 m;\n.b = 7;\n",
                    ".a = 5 m;\n.b = <uint:32,no_unit> 7;\n"):
            with pytest.raises(BovnarParseError) as e:
                collect(doc, UnitPolicy(require_unit=True))
            assert e.value.code == ErrorCode.UNIT_MISMATCH

    def test_require_dimension(self):
        p = UnitPolicy(require_dimension_of=["m"])
        assert len(collect(".a = 5 k~m;\n.b = 12 in;\n.c = 3 mi;\n", p)) == 3
        with pytest.raises(BovnarParseError):
            collect(".a = 5 k~m;\n.b = 25 °C;\n", p)
        # several requirements: satisfying one is enough
        assert len(collect(".a = 5 k~m;\n.b = 25 °C;\n",
                           UnitPolicy(require_dimension_of=["m", "K"]))) == 2

    def test_require_dimension_fences_unitless(self):
        with pytest.raises(BovnarParseError):
            collect(".x = <float:64> 0.25;", UnitPolicy(require_dimension_of=["%"]))
        assert len(collect(".x = <float:64> 0.25;",
                           UnitPolicy(require_dimension_of=["no_unit"]))) == 1

    def test_assertions_see_the_native_unit(self):
        # Requirement and target are both temperatures; the document is a
        # length. Evaluating after conversion would have made this pass.
        with pytest.raises(BovnarParseError):
            collect(".t = 25 °C;\n.d = 5 m;\n",
                    UnitPolicy(targets=["K"], require_dimension_of=["K"]))


@needs_lib
class TestPolicyLifecycle:

    def test_survives_reuse_and_clears(self):
        with Reader() as r:
            r.set_unit_policy(UnitPolicy(targets=["m"]))
            for _ in range(2):
                rows = []
                r.read_mem(b".d = 5 k~m;",
                           on_verified=lambda ev, d: (
                               rows.append(d.converted_str())
                               if ev == Event.DATA and d and d.converted else None,
                               True)[1])
                assert rows == ["5000"]
            r.set_unit_policy(None)
            seen = []
            r.read_mem(b".d = 5 k~m;",
                       on_verified=lambda ev, d: (
                           seen.append(bool(d.converted))
                           if ev == Event.DATA and d and d.value_type
                           and d.value_type.family == ValueTypeFamily.UINT else None,
                           True)[1])
            assert seen == [False]

    def test_bad_unit_raises_before_the_read(self):
        with Reader() as r:
            with pytest.raises(BovnarArgumentError):
                r.set_unit_policy(UnitPolicy(targets=["not_a_unit_at_all"]))
            with pytest.raises(BovnarArgumentError):
                r.set_unit_policy(UnitPolicy(require_dimension_of=["nonsense"]))
            with pytest.raises(BovnarArgumentError):
                r.set_unit_policy(UnitPolicy(targets=[("m", 63)]))
            with pytest.raises(BovnarArgumentError):
                r.set_unit_policy(UnitPolicy(targets=["m"] * 9))

    def test_a_rejected_policy_leaves_the_previous_one(self):
        with Reader() as r:
            r.set_unit_policy(UnitPolicy(targets=["m"]))
            with pytest.raises(BovnarArgumentError):
                r.set_unit_policy(UnitPolicy(targets=["still_not_a_unit"]))
            rows = []
            r.read_mem(b".d = 5 k~m;",
                       on_verified=lambda ev, d: (
                           rows.append(d.converted_str())
                           if ev == Event.DATA and d and d.converted else None,
                           True)[1])
            assert rows == ["5000"]


NESTED = (".inlet = {\n"
          "  .temperature = <float:64,°F> 212.0;\n"
          "  .flow        = <float:64,in> 12.0;\n"
          "};\n"
          ".outlet = {\n"
          "  .temperature = <float:64,°F> 32.0;\n"
          "};\n")


@needs_lib
class TestFieldRules:
    """Per-field rules: the thing a whole-document target list cannot say."""

    def test_exact_path(self):
        from bovnar import UnitRule
        rows = collect(NESTED, UnitPolicy(rules=[UnitRule(".inlet.temperature", "°C")]))
        assert len(rows) == 3
        assert rows[0][:2] == (True, "100")   # named
        assert rows[1][0] is False            # its sibling
        assert rows[2][0] is False            # same key, different parent

    def test_wildcard_and_component_boundary(self):
        from bovnar import UnitRule
        rows = collect(".in    = { .a = <float:64,in> 12.0; };\n"
                       ".inlet = { .a = <float:64,in> 12.0; };\n",
                       UnitPolicy(rules=[UnitRule(".in.*", "m")]))
        assert rows[0][0] is True     # .in.a
        assert rows[1][0] is False    # .inlet.a is NOT under .in

    def test_a_rule_is_an_assertion(self):
        from bovnar import UnitRule
        p = UnitPolicy(rules=[UnitRule(".speed", "m/s", convert=False)])
        rows = collect(".speed = <float:64,k~m/h> 42.0;", p)
        assert rows[0][0] is False and rows[0][2] == "k~m/h"
        for bad in (".speed = <float:64,°C> 42.0;", ".speed = <float:64> 42.0;"):
            with pytest.raises(BovnarParseError) as e:
                collect(bad, p)
            assert e.value.code == ErrorCode.UNIT_MISMATCH

    def test_rules_outrank_targets(self):
        from bovnar import UnitRule
        rows = collect(".a = <float:64,in> 12000.0;\n.b = <float:64,in> 12.0;\n",
                       UnitPolicy(rules=[UnitRule(".a", "k~m")], targets=["m"]))
        assert rows[0][3] == "k~m"    # the rule
        assert rows[1][3] == "m"      # the target list

    def test_across_an_array_of_structs(self):
        """One assignment, one key, many rows: the rule has to reach every row.
        It used to reach only the first, because the first row's push consumed
        the key and later rows pushed whatever key that row had ended on."""
        from bovnar import UnitRule
        doc = (".holdings = [\n"
               "  { .amount = <float:64,in>  12.0; },\n"
               "  { .amount = <float:64,k~m>  1.0; },\n"
               "  { .amount = <float:64,in>  24.0; }\n"
               "];\n")
        rows = collect(doc, UnitPolicy(rules=[UnitRule(".holdings.amount", "m")]))
        assert [r[1] for r in rows] == ["0.3048", "1000", "0.6096"]

        # and the assertion half reaches every row too
        bad = (".holdings = [ { .amount = <float:64,in> 12.0; },\n"
               "              { .amount = <float:64,°C>  5.0; } ];\n")
        with pytest.raises(BovnarParseError) as e:
            collect(bad, UnitPolicy(rules=[UnitRule(".holdings.amount", "m")]))
        assert e.value.code == ErrorCode.UNIT_MISMATCH

    def test_bad_rule_paths_are_refused(self):
        from bovnar import Reader, UnitRule
        with Reader() as r:
            for bad in (UnitRule("a.b", "m"),        # no leading dot
                        UnitRule(".*", "m"),         # names the whole document
                        UnitRule(".a", "not_a_unit")):
                with pytest.raises(BovnarArgumentError):
                    r.set_unit_policy(UnitPolicy(rules=[bad]))


@needs_lib
class TestDomPolicy:
    """The DOM takes the same policy, so a random-access consumer can assert
    what it expects and store what it asked for."""

    def test_conversion_is_stored(self):
        from bovnar import dom_parse, UnitRule
        doc = dom_parse(NESTED, UnitPolicy(rules=[UnitRule(".inlet.temperature", "°C")]))
        n = doc.lookup(".inlet.temperature")
        assert abs(n.as_float() - 100.0) < 1e-9
        assert n.unit_str == "°C"
        # the sibling is as the document wrote it
        assert abs(doc.lookup(".inlet.flow").as_float() - 12.0) < 1e-9

    def test_validation(self):
        from bovnar import dom_parse
        with pytest.raises(BovnarParseError) as e:
            dom_parse(".a = 5 m;\n.b = 7;", UnitPolicy(require_unit=True))
        assert e.value.code == ErrorCode.UNIT_MISMATCH

    def test_a_bad_policy_is_not_a_parse_error(self):
        # The DOM path validates the policy as Reader.set_unit_policy does and
        # raises the same BovnarArgumentError, naming the bad entry. It used to
        # surface as BovnarParseError(INVALID_ARGUMENT) with no reason at all,
        # which is what doc/09 §5.6 said it did NOT do.
        from bovnar import dom_parse, UnitRule
        bad = UnitPolicy(targets=["not_a_unit_at_all"])
        with pytest.raises(BovnarArgumentError) as dom_err:
            dom_parse(".a = 5 m;", bad)
        with pytest.raises(BovnarArgumentError) as reader_err:
            Reader().set_unit_policy(bad)
        assert "not_a_unit_at_all" in str(dom_err.value)
        assert str(dom_err.value) == str(reader_err.value)
        with pytest.raises(BovnarArgumentError, match="'zz'"):
            dom_parse(".a = 5 m;", UnitPolicy(rules=[UnitRule(".a", "zz")]))

    def test_a_bad_policy_is_refused_by_parse_fd_too(self, tmp_path):
        from bovnar import DomDoc
        p = tmp_path / "d.bvnr"
        p.write_bytes(b".a = 5 m;")
        with pytest.raises(BovnarArgumentError, match="nosuch"):
            DomDoc.parse_file(str(p), UnitPolicy(targets=["nosuch"]))

    def test_across_an_array_of_structs(self):
        from bovnar import dom_parse, UnitRule
        doc = dom_parse(".h = [ { .a = <float:64,in> 12.0; },\n"
                        "       { .a = <float:64,in> 24.0; } ];\n",
                        UnitPolicy(rules=[UnitRule(".h.a", "m")]))
        rows = doc.lookup(".h")
        assert abs(rows[0]["a"].as_float() - 0.3048) < 1e-12
        assert abs(rows[1]["a"].as_float() - 0.6096) < 1e-12

    def test_integer_converting_to_a_fraction_becomes_a_float(self):
        from bovnar import dom_parse, UnitRule
        doc = dom_parse(".m = <uint:32,g> 5;",
                        UnitPolicy(rules=[UnitRule(".m", "k~g")]))
        assert abs(doc.lookup(".m").as_float() - 0.005) < 1e-12


@needs_lib
class TestWriterPolicy:
    """The producer's half. A reader can only refuse a document somebody
    already wrote; this is what stops the bare number reaching the file."""

    def test_require_unit(self):
        from bovnar import Writer
        from bovnar.exceptions import BovnarWriteError

        w = Writer.to_mem()
        w.set_unit_policy(UnitPolicy(require_unit=True))
        w.write_float("speed", 9.81, unit_str="m/s")     # accepted
        with pytest.raises(BovnarWriteError) as e:
            w.write_float("bare", 0.25)
        assert e.value.code == ErrorCode.UNIT_MISMATCH

    def test_require_dimension(self):
        from bovnar import Writer
        from bovnar.exceptions import BovnarWriteError

        w = Writer.to_mem()
        w.set_unit_policy(UnitPolicy(require_dimension_of=["m"]))
        w.write_float("a", 12.0, unit_str="in")          # a length, accepted
        with pytest.raises(BovnarWriteError) as e:
            w.write_float("b", 25.0, unit_str="°C")
        assert e.value.code == ErrorCode.UNIT_MISMATCH

    def test_non_numeric_values_are_not_subject_to_it(self):
        from bovnar import Writer

        w = Writer.to_mem()
        w.set_unit_policy(UnitPolicy(require_unit=True))
        # No exception is the assertion: none of these carries a unit, and none
        # of them is the policy's business.
        w.write_string("name", "sensor-1")
        w.write_bool("ok", True)
        w.write_null("spare")
        w.finish()

    def test_conversion_fields_are_refused(self):
        from bovnar import Writer

        w = Writer.to_mem()
        for bad in (UnitPolicy(targets=["m"]),
                    UnitPolicy(normalise_si=True),
                    UnitPolicy(base=16),
                    UnitPolicy(leave_inexact=True)):
            with pytest.raises(BovnarArgumentError):
                w.set_unit_policy(bad)
        with pytest.raises(BovnarArgumentError):
            w.set_unit_policy(UnitPolicy(require_dimension_of=["not_a_unit"]))

    def test_what_the_writer_emits_the_reader_accepts(self):
        """The property that makes a producer-side check worth having: it is the
        same check, so a document that passed on the way out passes on the way
        back in."""
        from bovnar import Writer

        policy = UnitPolicy(require_unit=True, require_dimension_of=["m"])
        w = Writer.to_mem()
        w.set_unit_policy(policy)
        w.write_float("a", 12.0, unit_str="in")
        w.write_float("b", 5.0, unit_str="k~m")
        w.finish()
        doc = w.get_output().decode("utf-8")

        assert len(collect(doc, policy)) == 2


@needs_lib
class TestUnitPredicates:

    def test_units_convertible_covers_what_compatible_misses(self):
        usd, kusd = parse_unit("$USD"), parse_unit("k~$USD")
        assert bovnar.units_compatible(kusd, usd) is False
        assert bovnar.units_convertible(kusd, usd) is True
        # and it still refuses genuinely unrelated units
        assert bovnar.units_convertible(parse_unit("m"), parse_unit("s")) is False

    def test_si_normal_form(self):
        assert unit_to_str(bovnar.unit_si_normal_form(parse_unit("in"))) == "m"
        assert unit_to_str(bovnar.unit_si_normal_form(parse_unit("g"))) == "k~g"
        for none_case in ("%", "ppm", "dB", "pH", "rad", "$USD", "lm", "s/°C"):
            assert bovnar.unit_si_normal_form(parse_unit(none_case)) is None, none_case


@needs_lib
class TestPolicyRefusalNamesTheEntry:
    """A refused policy must say WHICH entry, and which part of it.

    ``bvnr_reader_set_unit_policy`` answers one bool for a whole policy, and
    both bindings reported every refusal as "check the unit spellings in
    targets / require_dimension_of". That never mentioned ``rules`` at all, and
    described a bad rule PATH — over MAX_UNIT_PATH bytes, missing its leading
    '.', or a bare '.*' — and an invalid base as unit spellings. The tests above
    assert only that *something* raised, which is what let the wording drift
    away from the causes.
    """

    BAD_UNIT = "not_a_unit_at_all"

    def _msg(self, **kw):
        from bovnar import Reader, UnitPolicy
        with Reader() as r:
            with pytest.raises(bovnar.BovnarArgumentError) as e:
                r.set_unit_policy(UnitPolicy(**kw))
        return str(e.value)

    def test_targets_entry_is_named(self):
        m = self._msg(targets=[self.BAD_UNIT])
        assert "targets" in m and self.BAD_UNIT in m

    def test_require_dimension_entry_is_named(self):
        m = self._msg(require_dimension_of=[self.BAD_UNIT])
        assert "require_dimension_of" in m and self.BAD_UNIT in m

    def test_a_bad_rule_unit_names_the_rule_and_the_unit(self):
        from bovnar import UnitRule
        m = self._msg(rules=[UnitRule(".inlet.speed", self.BAD_UNIT)])
        assert ".inlet.speed" in m and self.BAD_UNIT in m
        # ...and calls it a unit, not a path.
        assert "path" not in m

    @pytest.mark.parametrize("path", ["a.b", ".*", "." + "a" * 97])
    def test_a_bad_rule_path_is_called_a_path(self, path):
        from bovnar import UnitRule
        m = self._msg(rules=[UnitRule(path, "m")])
        assert "path" in m
        # The old message blamed unit spellings for every one of these.
        assert "unit spellings" not in m

    def test_a_bad_base_is_called_a_base(self):
        from bovnar import UnitRule
        assert "base" in self._msg(targets=[("m", 63)])
        assert "base" in self._msg(rules=[UnitRule(".a", "m", base=63)])
        assert "base" in self._msg(targets=["m"], base=63)

    def test_the_writer_names_rules_too(self):
        """The writer's message listed only require_dimension_of, and its
        setter takes rules (in convert=False form)."""
        from bovnar import Writer, UnitPolicy, UnitRule
        w = Writer.to_mem()
        with pytest.raises(bovnar.BovnarArgumentError) as e:
            w.set_unit_policy(UnitPolicy(
                rules=[UnitRule(".a", self.BAD_UNIT, convert=False)]))
        assert ".a" in str(e.value) and self.BAD_UNIT in str(e.value)

    def test_the_first_bad_entry_is_the_one_reported(self):
        """Good entries before the bad one must not be accused."""
        m = self._msg(targets=["m", "k~m/s", self.BAD_UNIT])
        assert self.BAD_UNIT in m and "k~m/s" not in m

    def test_a_valid_policy_is_still_accepted(self):
        """The probe must not reject anything the setter takes."""
        from bovnar import Reader, UnitPolicy, UnitRule
        with Reader() as r:
            r.set_unit_policy(UnitPolicy(
                targets=["m", ("k~m", 16)], base=10, normalise_si=True,
                leave_inexact=True, require_unit=False,
                require_dimension_of=["s"],
                rules=[UnitRule(".a.b", "m"), UnitRule(".c.*", "s",
                                                       convert=False)]))
