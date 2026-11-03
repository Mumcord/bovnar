# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
The exact-rational renderer, checked against arithmetic rather than against
itself.

`converted_in_base(b)` renders an exact conversion in base b, or returns None
when it has no terminating expansion there. Both halves are decidable without
consulting the implementation:

  * a rational p/q in lowest terms terminates in base b exactly when every
    prime factor of q also divides b; and
  * when it does terminate, decoding the digits must give back p/q exactly.

The suite spot-checked one value in bases 2 and 16. This checks the rule itself
across every base the format writes, so a renderer that started truncating, or
started claiming a terminating expansion it could not produce, fails here rather
than in a document.
"""
from fractions import Fraction

import bovnar
import pytest
from bovnar import Event, Reader

_DIGITS = "0123456789abcdefghijklmnopqrstuvwxyz" \
          "ABCDEFGHIJKLMNOPQRSTUVWXYZ"

# (source unit, hook target). Chosen so the exact factors between them cover a
# range of denominators: a pure power of ten, one with a 3 in it, the inch's
# 254/10000, the foot's 10000/3048 and the pound's 45359237/100000000.
CASES = (
    ("m", "k~m"), ("in", "m"), ("m", "ft"), ("lb", "k~g"),
    ("k~m", "m"), ("h", "s"), ("k~m/h", "m/s"),
)


def _prime_factors(n):
    out, d = set(), 2
    while d * d <= n:
        while n % d == 0:
            out.add(d)
            n //= d
        d += 1
    if n > 1:
        out.add(n)
    return out


def _terminates(frac, base):
    """p/q terminates in `base` iff every prime factor of q divides base."""
    if frac.denominator == 1:
        return True
    return _prime_factors(frac.denominator) <= _prime_factors(base)


def _decode(text, base):
    negative = text.startswith("-")
    if negative:
        text = text[1:]
    integral, _, fractional = text.partition(".")
    value = Fraction(0)
    for ch in integral:
        value = value * base + _DIGITS.index(ch)
    scale = Fraction(1)
    for ch in fractional:
        scale /= base
        value += _DIGITS.index(ch) * scale
    return -value if negative else value


def _render(unit, target):
    """{'rational': (p, q), base: text-or-None} for one conversion."""
    out = {}

    def on_event(ev, d):
        if ev != Event.DATA or d is None or not d.converted:
            return True
        out["rational"] = d.converted_rational()
        for base in range(2, 63):
            try:
                out[base] = d.converted_in_base(base)
            except Exception:                                 # noqa: BLE001
                out[base] = None
        return True

    Reader().read_mem((".v = <float:64,%s> 1.0;" % unit).encode(),
                      want_unit=lambda _d: bovnar.parse_unit(target),
                      want_unit_allow_nonterminating=True,
                      on_verified=on_event)
    return out


@pytest.mark.needs_lib
def test_it_terminates_exactly_when_the_arithmetic_says_it_should():
    checked, wrong = 0, []
    for unit, target in CASES:
        rendered = _render(unit, target)
        assert "rational" in rendered, "%s -> %s produced no conversion" % (unit, target)
        exact = Fraction(*rendered["rational"])
        for base in range(2, 63):
            checked += 1
            produced = rendered[base] is not None
            if produced != _terminates(exact, base):
                wrong.append((unit, target, base, produced,
                              _terminates(exact, base)))
    assert checked > 300
    assert not wrong, ("the renderer disagrees with the terminating rule: %r"
                       % wrong[:8])


@pytest.mark.needs_lib
def test_every_terminating_rendering_decodes_back_to_the_exact_rational():
    checked, wrong = 0, []
    for unit, target in CASES:
        rendered = _render(unit, target)
        exact = Fraction(*rendered["rational"])
        for base in range(2, 63):
            text = rendered[base]
            if text is None:
                continue
            checked += 1
            if _decode(text, base) != exact:
                wrong.append((unit, target, base, text,
                              str(_decode(text, base)), str(exact)))
    assert checked > 100
    assert not wrong, "renderings that do not decode back: %r" % wrong[:8]


@pytest.mark.needs_lib
def test_the_non_terminating_case_is_really_in_the_set():
    """Without this both tests above could pass over a set that always terminates."""
    rendered = _render("k~m/h", "m/s")
    # 1 km/h in m/s is 5/18 = 5/(2 * 3**2): it needs a base carrying both a 2
    # and a 3, so base 6 renders it and base 10 cannot.
    assert Fraction(*rendered["rational"]) == Fraction(5, 18)
    assert rendered[10] is None
    assert rendered[3] is None           # a 3 alone is not enough
    assert rendered[6] == "0.14"         # 1/6 + 4/36 = 5/18
