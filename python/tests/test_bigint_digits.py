# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""Integers past CPython's 4300-digit int<->str limit.

The format holds integers up to 32768 bits, about 9865 decimal digits, but
CPython refuses a decimal ``int(text)`` or ``str(n)`` past
``sys.get_int_max_str_digits()`` (4300 by default). Every place the bindings
crossed between an int and its decimal text raised there -- or worse, in
loads(), caught the ValueError and returned the TEXT, so a document this library
wrote itself came back with a str where the int had been.

The process-wide limit belongs to the application: none of these may change it.
"""

import sys
from decimal import Decimal
from fractions import Fraction

import pytest

from conftest import needs_lib
import bovnar
from bovnar import Quantity
from bovnar.enums import Event, BaseUnit, ValueTypeFamily
from bovnar.reader import Reader
from bovnar.structs import make_unit_si, make_type_spec
from bovnar.writer import Writer
from bovnar.exceptions import BovnarArgumentError, BovnarWriteError
from bovnar._bigint import int_from_text, int_to_decimal

_LIMIT = getattr(sys, 'get_int_max_str_digits', lambda: 0)()

# 4301 digits: one past the default limit; ~9000: near the format's ceiling.
BIG = [10 ** 4300 + 12345, -(10 ** 9000) + 1, 2 ** 32767 - 1]
# pytest's own ids would str() the values, and hit the very limit under test
BIG_IDS = ['1e4300+k', '-1e9000+1', '2^32767-1']


@pytest.fixture(autouse=True)
def _limit_untouched():
    yield
    if _LIMIT:
        assert sys.get_int_max_str_digits() == _LIMIT


def test_helpers_round_trip_past_the_limit():
    for n in BIG + [0, -1, 10 ** 511, 10 ** 512, -(10 ** 513)]:
        s = int_to_decimal(n)
        assert int_from_text(s, 10) == n
        if abs(n) < 10 ** 4000:
            assert s == str(n)
    assert int_from_text('7' * 5000, 8) == int('7' * 5000, 8)
    assert int_from_text('z' * 5000, 36) == sum(
        35 * 36 ** i for i in range(5000))
    with pytest.raises(ValueError):
        int_from_text('1' * 5000 + 'x', 10)


@needs_lib
@pytest.mark.parametrize("n", BIG, ids=BIG_IDS)
def test_loads_dumps_round_trip(n):
    doc = bovnar.dumps({'a': n})
    back = bovnar.loads(doc)['a']
    assert isinstance(back, int) and back == n
    assert bovnar.loads(doc, typed=True)['a'].decimal == Decimal(n)


@needs_lib
@pytest.mark.parametrize("n", BIG, ids=BIG_IDS)
def test_array_round_trip(n):
    back = bovnar.loads(bovnar.dumps({'a': [n, 1]}))['a']
    assert back == [n, 1] and isinstance(back[0], int)


@needs_lib
def test_loads_a_hand_written_wide_literal():
    got = bovnar.loads(b'.a = <uint:32768> ' + b'9' * 5000 + b';')['a']
    assert isinstance(got, int) and got == 10 ** 5000 - 1


@needs_lib
@pytest.mark.parametrize("n", BIG, ids=BIG_IDS)
def test_dom_to_python(n):
    doc = bovnar.dom_parse(bovnar.dumps({'a': n, 's': {'b': n}}))
    assert doc['a'].to_python() == n
    assert doc.to_dict() == {'a': n, 's': {'b': n}}


@needs_lib
def test_write_array_under_a_wide_annotation():
    n = 10 ** 9000
    w = Writer.to_mem(pretty=False)
    bovnar.write_array(w, 'a', [n, 2],
                       vt=make_type_spec(ValueTypeFamily.UINT, 32768))
    w.finish()
    assert bovnar.loads(w.get_output())['a'] == [n, 2]


@needs_lib
def test_write_uint_refusal_message_does_not_raise_valueerror():
    w = Writer.to_mem()
    with pytest.raises(BovnarWriteError, match=r'digits'):
        w.write_uint('a', 10 ** 5000)


@needs_lib
@pytest.mark.parametrize("n", BIG, ids=BIG_IDS)
def test_quantity_from_number(n):
    q = Quantity.from_number(n, width=256)
    assert q.decimal == Decimal(n)
    back = bovnar.loads(bovnar.dumps({'q': q}), typed=True)['q']
    assert back.decimal == Decimal(n)
    # decimal64 cannot hold it: a range refusal, not a ValueError
    with pytest.raises(BovnarArgumentError):
        Quantity.from_number(n)


@needs_lib
def test_converted_rational_of_a_wide_result():
    n = 10 ** 5000 - 1                      # km -> m multiplies by 1000; 5000 9s
    seen = []

    def want(d):
        return (make_unit_si(BaseUnit.METER), 10)

    def on_event(ev, d):
        if ev == Event.DATA and d is not None and d.converted:
            seen.append(d)
        return True

    Reader().read_mem(b'.d = <uint:32768,k~m> ' + b'9' * 5000 + b';',
                      on_verified=on_event, want_unit=want,
                      max_conversion_length=20000)
    num, den = seen[0].converted_rational()
    assert Fraction(num, den) == n * 1000
    assert int_from_text(seen[0].converted_str(), 10) == n * 1000


@needs_lib
def test_numpy_object_array():
    np = pytest.importorskip('numpy')
    n = 10 ** 4300 + 1
    out = bovnar.array_to_bvnr('a', np.array([n, 1], dtype=object))
    assert bovnar.loads(out)['a'] == [n, 1]
    for src in (bovnar.loads(out, typed=True)['a'],
                bovnar.dom_parse(out)['a']):
        arr = bovnar.to_numpy(src, dtype=object)
        assert list(arr) == [n, 1]
