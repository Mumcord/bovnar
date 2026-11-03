# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
`units_convertible` is a screen, and the gap between it and the conversion is
exactly one shape wide.

doc/05 §12.4 used to claim in one paragraph that `bvn_units_convertible` reports
"exactly the set `bvn_unit_convert_value` and `bvn_unit_convert_rational`
accept", and in the next that it is "a screen, not a guarantee -- `s/°C` passes
here and the conversion entry points still refuse it". The second paragraph was
right, and screening on the first cost three silent policy failures (a target
that claimed `°C/h -> K/h` and then delivered the °C/h value, and two rule modes
that reported OK on a document satisfying neither).

The document says the screen is a SUPERSET whose gap is exactly TWO shapes: an
affine scale outside "alone, at exponent 1", and capacity — a factor needing
more than BVN_INT_MAX_BITS, which is why `Q~m^100 -> q~m^100` screens true and
then declines. That second one is deliberate and documented: separating "too
large to compute" from "these units disagree" is the question the screen exists
to answer.

The sweep below runs over the catalogue and its quotients and squares, a domain
that cannot reach the capacity bound, so within it the affine shape is the only
one permitted. `test_capacity_is_the_other_shape_in_the_gap` pins the second
directly. If a THIRD shape ever opens, a policy can start claiming values it
cannot deliver again, and the sentence in doc/05 quietly becomes false.
"""
import itertools
import os
import random
import re

import bovnar
import pytest

AFFINE = ("°C", "°F", "°De", "°N", "°Re", "°Ro")


# The suite runs with cwd=python/, so anchor on this file rather than on the
# working directory: a relative "src/gendata/..." silently skipped the sweep.
_REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def _catalogue():
    path = os.path.join(_REPO, "src", "gendata", "units.bvnr")
    with open(path, encoding="utf-8") as f:
        return re.findall(r'\.symbol\s*=\s*"([^"]+)"', f.read())


def _affine_misplaced(spelling):
    """True when an affine scale appears anywhere but alone at exponent 1."""
    if spelling in AFFINE:
        return False
    return any(a in spelling for a in AFFINE)


@pytest.mark.needs_lib
def test_convertible_that_cannot_convert_is_always_the_affine_shape():
    try:
        symbols = _catalogue()
    except OSError:                       # running from an installed wheel
        pytest.skip("src/gendata is not present")

    rng = random.Random(20260731)
    spellings = list(symbols)
    for sym in symbols:
        spellings += ["%s/%s" % (sym, other) for other in rng.sample(symbols, 2)]
        spellings.append("%s²" % sym)

    units = []
    for spelling in spellings:
        try:
            units.append((spelling, bovnar.parse_unit(spelling)))
        except Exception:                                     # noqa: BLE001
            continue
    rng.shuffle(units)

    screened = 0
    unexplained = []
    for (sa, a), (sb, b) in itertools.islice(
            itertools.combinations(units, 2), 0, 120_000):
        if not bovnar.units_convertible(a, b):
            continue
        screened += 1
        try:
            bovnar.convert_value(1.0, a, b)
        except Exception:                                     # noqa: BLE001
            if not (_affine_misplaced(sa) or _affine_misplaced(sb)):
                unexplained.append((sa, sb))

    assert screened > 500, "the sweep stopped screening anything"
    assert not unexplained, (
        "units_convertible passed a pair the conversion refused, and it is not "
        "the documented affine-in-a-compound shape: %r" % unexplained[:10])


@pytest.mark.needs_lib
def test_capacity_is_the_other_shape_in_the_gap():
    """The second shape, and the reason the screen answers true for it.

    `Q~m^100 -> q~m^100` is 10^6000: a perfectly well-defined conversion
    between compatible units that no representation can carry. The screen says
    true so that a caller can tell "too large to compute" apart from "these
    units disagree" -- which is what doc/05 12.4 says it is for.
    """
    a, b = bovnar.parse_unit("Q~m^100"), bovnar.parse_unit("q~m^100")
    assert bovnar.units_convertible(a, b) is True
    with pytest.raises(Exception):
        bovnar.convert_value(1.0, a, b)
    # Neither side is affine, so this is genuinely a second shape and not the
    # first one wearing a large prefix.
    assert not _affine_misplaced("Q~m^100")
    assert not _affine_misplaced("q~m^100")
    # The same units at an exponent the factor CAN carry convert normally.
    small_a, small_b = bovnar.parse_unit("Q~m"), bovnar.parse_unit("q~m")
    assert bovnar.convert_value(1.0, small_a, small_b) == pytest.approx(1e60)


@pytest.mark.needs_lib
def test_the_affine_shape_really_is_in_the_gap():
    """The other half: without this the test above passes vacuously."""
    a, b = bovnar.parse_unit("°C/h"), bovnar.parse_unit("K/h")
    assert bovnar.units_convertible(a, b) is True
    with pytest.raises(Exception):
        bovnar.convert_value(1.0, a, b)
    # ...and a lone affine is not in the gap.
    c, k = bovnar.parse_unit("°C"), bovnar.parse_unit("K")
    assert bovnar.units_convertible(c, k) is True
    assert bovnar.convert_value(25.0, c, k) == pytest.approx(298.15)
