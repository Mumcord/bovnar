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

import pytest

from conftest import needs_lib
import bovnar
from bovnar import loads, write_array
from bovnar.writer import Writer
from bovnar.enums import Event, ValueTypeFamily, BaseUnit, SIPrefix
from bovnar.structs import (
    make_type_spec, make_unit_si, make_unit_dimensionless, make_unit_none,
)
from bovnar.exceptions import BovnarArgumentError


def _roundtrip(rows, **kw):
    with Writer.to_mem() as w:
        write_array(w, 'data', rows, **kw)
    raw = w.get_output()
    doc = loads(raw)
    return doc.get('data')


@needs_lib
class TestWriteArrayFlat:
    def test_ints_roundtrip(self):
        result = _roundtrip([1, 2, 3])
        assert result == [1, 2, 3]

    def test_single_element(self):
        result = _roundtrip([42])
        assert result == [42]

    def test_floats(self):
        result = _roundtrip([1.5, 2.5, 3.5])
        assert len(result) == 3
        assert abs(result[0] - 1.5) < 1e-9

    def test_strings(self):
        result = _roundtrip(['a', 'b', 'c'])
        assert result == ['a', 'b', 'c']

    def test_negative_ints(self):
        result = _roundtrip([-1, -2, -3])
        assert result == [-1, -2, -3]

    def test_mixed_int_signs(self):
        result = _roundtrip([-5, 0, 5])
        assert result == [-5, 0, 5]

    def test_empty_list(self):
        # An empty array "[]" is a genuinely empty row (zero elements) and is
        # distinct from "[null]" (one null element); it round-trips as [].
        result = _roundtrip([])
        assert result == []

    def test_output_contains_key(self):
        with Writer.to_mem() as w:
            write_array(w, 'mykey', [1, 2])
        out = w.get_output()
        assert b'mykey' in out

    def test_output_contains_values(self):
        with Writer.to_mem() as w:
            write_array(w, 'vals', [10, 20, 30])
        out = w.get_output()
        assert b'10' in out
        assert b'20' in out
        assert b'30' in out


@needs_lib
class TestWriteArrayMultiRow:
    def test_2x2_roundtrip(self):
        result = _roundtrip([[1, 2], [3, 4]])
        assert result == [[1, 2], [3, 4]]

    def test_3x1_roundtrip(self):
        result = _roundtrip([[10], [20], [30]])
        assert result == [[10], [20], [30]]

    def test_output_contains_slash_separator(self):
        with Writer.to_mem() as w:
            write_array(w, 'm', [[1, 2], [3, 4]])
        out = w.get_output()
        assert b'/' in out

    def test_1x3_treated_as_flat(self):
        result = _roundtrip([1, 2, 3])
        assert result == [1, 2, 3]

    def test_3x3_matrix(self):
        rows   = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
        result = _roundtrip(rows)
        assert result == rows

    def test_uniform_rows(self):
        # BVNR arrays are rectangular: all rows must have the same column count.
        rows   = [[1, 0], [2, 3], [4, 5]]
        result = _roundtrip(rows)
        assert result == rows


@needs_lib
class TestWriteArrayDictElements:
    def test_single_dict_element(self):
        result = _roundtrip([{'x': 1}])
        assert isinstance(result, list)
        assert len(result) == 1
        assert result[0]['x'] == 1

    def test_two_dict_elements(self):
        result = _roundtrip([{'a': 1}, {'a': 2}])
        assert result == [{'a': 1}, {'a': 2}]

    def test_dict_with_multiple_fields(self):
        result = _roundtrip([{'x': 10, 'y': 20}])
        assert result[0] == {'x': 10, 'y': 20}

    def test_dict_with_nested_struct(self):
        result = _roundtrip([{'cfg': {'port': 80}}])
        assert result[0]['cfg']['port'] == 80

    def test_dict_with_array_field(self):
        result = _roundtrip([{'vals': [1, 2, 3]}])
        assert result[0]['vals'] == [1, 2, 3]


@needs_lib
class TestWriteArrayNestedElements:
    def test_nested_list_as_element(self):
        result = _roundtrip([[1, 2], [3, 4]])
        assert result == [[1, 2], [3, 4]]

    def test_single_nested_list(self):
        with Writer.to_mem() as w:
            write_array(w, 'd', [1, 2, 3])
        out = w.get_output()
        assert b'd' in out


@needs_lib
class TestWriteArrayWithType:
    def test_uint32_annotation_in_output(self):
        vt = make_type_spec(ValueTypeFamily.UINT, 32, 10)
        with Writer.to_mem() as w:
            write_array(w, 'ports', [80, 443], vt=vt)
        out = w.get_output()
        assert b'uint' in out
        assert b'32'   in out

    def test_float64_annotation(self):
        vt = make_type_spec(ValueTypeFamily.FLOAT, 64, 0)
        with Writer.to_mem() as w:
            write_array(w, 'vals', [1.0, 2.0], vt=vt)
        out = w.get_output()
        assert b'float' in out

    def test_unit_annotation(self):
        vt = make_type_spec(ValueTypeFamily.FLOAT, 64, 0)
        vu = make_unit_si(BaseUnit.METER, SIPrefix.KILO)
        with Writer.to_mem() as w:
            write_array(w, 'distances', [1.0, 2.0], vt=vt, vu=vu)
        out = w.get_output()
        assert b'k~m' in out or b'km' in out

    def test_no_annotation_still_valid(self):
        result = _roundtrip([5, 10, 15])
        assert result == [5, 10, 15]


@needs_lib
class TestWriteArrayTokenTypes:
    """
    These tests verify the token type fix (issue #2):
    array elements must use TOKEN_IS_ARRAY_NUMBER / TOKEN_IS_ARRAY_STRING,
    not the scalar token types. The fix is validated indirectly by checking
    that the C writer accepts the output without an error (a wrong token type
    would cause bvnr_write_event to return False and raise BovnarWriteError).
    """

    def test_int_array_parses_without_error(self):
        with Writer.to_mem() as w:
            write_array(w, 'x', [1, 2, 3])

    def test_float_array_parses_without_error(self):
        with Writer.to_mem() as w:
            write_array(w, 'x', [1.0, 2.0, 3.0])

    def test_string_array_parses_without_error(self):
        with Writer.to_mem() as w:
            write_array(w, 'x', ['hello', 'world'])

    def test_int_array_roundtrips(self):
        result = _roundtrip([100, 200, 300])
        assert result == [100, 200, 300]

    def test_string_array_roundtrips(self):
        result = _roundtrip(['foo', 'bar', 'baz'])
        assert result == ['foo', 'bar', 'baz']

    def test_multirow_int_parses_without_error(self):
        with Writer.to_mem() as w:
            write_array(w, 'mat', [[1, 2], [3, 4]])

    def test_multirow_roundtrips(self):
        rows   = [[1, 2], [3, 4]]
        result = _roundtrip(rows)
        assert result == rows


@needs_lib
class TestWriteArrayInDumps:
    """write_array is exercised indirectly through dumps() for list values."""

    def test_list_value_in_dumps(self):
        doc = bovnar.loads(bovnar.dumps({'nums': [1, 2, 3]}))
        assert doc['nums'] == [1, 2, 3]

    def test_nested_list_in_dumps(self):
        out = bovnar.dumps({'mat': [[1, 2], [3, 4]]})
        doc = bovnar.loads(out)
        assert doc['mat'] == [[1, 2], [3, 4]]

    def test_list_of_dicts_in_dumps(self):
        data = {'items': [{'k': 1}, {'k': 2}]}
        out  = bovnar.dumps(data)
        doc  = bovnar.loads(out)
        assert doc['items'] == [{'k': 1}, {'k': 2}]

    def test_bool_elements_in_array(self):
        with Writer.to_mem() as w:
            write_array(w, 'flags', [True, False, True])
        out = w.get_output()
        assert b'true'  in out
        assert b'false' in out


@needs_lib
class TestDumpsBigInt:
    """Regression: integers exceeding 64 bits must not be silently truncated.

    Before the fix dumps({'x': 2**100}) emitted '<uint:64> 0' (2**100 mod 2**64),
    and bignums inside arrays raised value_out_of_range.
    """

    @pytest.mark.parametrize('v', [
        2**64, 2**64 - 1, 2**100, -(2**63) - 1, -(2**100),
        2**255, -(2**255), 0, 42, -7,
    ])
    def test_scalar_bigint_roundtrip(self, v):
        assert bovnar.loads(bovnar.dumps({'x': v}))['x'] == v

    def test_scalar_bigint_not_truncated_to_zero(self):
        # 2**100 mod 2**64 == 0 — the exact pre-fix corruption.
        assert bovnar.loads(bovnar.dumps({'x': 2**100}))['x'] == 2**100

    @pytest.mark.parametrize('arr', [
        [1, 2, 2**70],
        [2**100, -(2**99)],
        [-1, -2, 2**70],
        [[1, 2**80], [3, 4]],
        [9223372036854775808, 2**101],
    ])
    def test_array_bigint_roundtrip(self, arr):
        assert bovnar.loads(bovnar.dumps({'a': arr}))['a'] == arr

    def test_plain_int_arrays_unannotated(self):
        # Values that fit 64 bits keep the unannotated form (no regression).
        assert bovnar.dumps({'a': [1, 2, 3]}, pretty=False) == b'.a=[1,2,3];'


@needs_lib
class TestWriteArraySpecialFloats:
    """Regression tests for Bug 2: float inf/nan must be encoded as bovnar
    special-number keywords (inf, ninf, nan), not as Python
    repr() strings ('inf', '-inf', 'nan')."""

    def test_nan_in_array_does_not_raise(self):
        with Writer.to_mem() as w:
            write_array(w, 'v', [float('nan')])

    def test_pos_inf_in_array_does_not_raise(self):
        with Writer.to_mem() as w:
            write_array(w, 'v', [float('inf')])

    def test_neg_inf_in_array_does_not_raise(self):
        with Writer.to_mem() as w:
            write_array(w, 'v', [float('-inf')])

    def test_nan_encoded_as_special_literal(self):
        with Writer.to_mem() as w:
            write_array(w, 'v', [float('nan')])
        out = w.get_output()
        assert b'nan' in out

    def test_pos_inf_encoded_as_special_literal(self):
        with Writer.to_mem() as w:
            write_array(w, 'v', [float('inf')])
        out = w.get_output()
        assert b'inf' in out

    def test_neg_inf_encoded_as_special_literal(self):
        with Writer.to_mem() as w:
            write_array(w, 'v', [float('-inf')])
        out = w.get_output()
        assert b'ninf' in out

    def test_mixed_specials_and_normal_floats(self):
        with Writer.to_mem() as w:
            write_array(w, 'v', [float('nan'), float('inf'), float('-inf'), 1.5])
        out = w.get_output()
        assert b'nan' in out
        assert b'inf' in out
        assert b'ninf' in out
        assert b'1.5' in out


@needs_lib
class TestBoolRoundtrip:
    """Regression tests for the bool roundtrip.

    BVNR booleans are written as the bare keywords ``true`` / ``false``.
    The parser delivers them as ``token_is_bool`` (type field == 15) with
    ``value_type.family == vt_bool``.  _decode_value must inspect the token
    type so that these keywords are decoded back to Python ``True`` /
    ``False`` rather than the strings ``'true'`` / ``'false'``.
    """

    def test_true_roundtrips_to_python_bool(self):
        result = bovnar.loads(bovnar.dumps({'flag': True}))
        assert result['flag'] is True

    def test_false_roundtrips_to_python_bool(self):
        result = bovnar.loads(bovnar.dumps({'flag': False}))
        assert result['flag'] is False

    def test_bool_type_not_string(self):
        result = bovnar.loads(bovnar.dumps({'flag': True}))
        assert isinstance(result['flag'], bool), (
            f"Expected bool, got {type(result['flag']).__name__}")

    def test_bool_in_nested_struct(self):
        result = bovnar.loads(bovnar.dumps({'cfg': {'enabled': True, 'debug': False}}))
        assert result['cfg']['enabled'] is True
        assert result['cfg']['debug'] is False

    def test_bool_with_sibling_int(self):
        result = bovnar.loads(bovnar.dumps({'active': True, 'count': 5}))
        assert result['active'] is True
        assert result['count'] == 5

    def test_bool_array_elements_roundtrip(self):
        result = _roundtrip([True, False, True])
        assert result == [True, False, True]

    def test_bool_array_element_type(self):
        result = _roundtrip([True, False])
        assert isinstance(result[0], bool)
        assert isinstance(result[1], bool)

    def test_bool_in_struct_in_array(self):
        result = _roundtrip([{'enabled': True}, {'enabled': False}])
        assert result[0]['enabled'] is True
        assert result[1]['enabled'] is False

    def test_bool_via_write_array_helper(self):
        with Writer.to_mem() as w:
            write_array(w, 'flags', [True, False, True])
        out = w.get_output()
        result = bovnar.loads(out)
        assert result['flags'] == [True, False, True]

    def test_bool_dumps_loads_cycle(self):
        doc = {'x': True, 'y': False, 'z': 42, 's': 'hello'}
        result = bovnar.loads(bovnar.dumps(doc))
        assert result['x'] is True
        assert result['y'] is False
        assert result['z'] == 42
        assert result['s'] == 'hello'

    def test_raw_bvnr_true_symbol(self):
        result = bovnar.loads(b'.flag = true;\n')
        assert result['flag'] is True

    def test_raw_bvnr_false_symbol(self):
        result = bovnar.loads(b'.flag = false;\n')
        assert result['flag'] is False

    def test_other_symbols_remain_strings(self):
        result = bovnar.loads(b'.status = ok;\n')
        assert result['status'] == 'ok'
        assert not isinstance(result['status'], bool)



# ---------------------------------------------------------------------------
# What dumps() does with a list that is not a rectangle of plain numbers
# ---------------------------------------------------------------------------

@needs_lib
class TestArrayShapesSurviveDumps:
    """dumps() used to pick the array's text from the OUTER list alone."""

    @pytest.mark.parametrize('rows', [
        [[1], [2, 3]],              # was `[1]/[2,3]`: error_array_row_size_mismatch
        [[1, 2, 3], [4]],
        [[[1]]],                    # was `[[1]]`: one dimension short
        [[[1.5]]],
        [[[[7]]]],
        [[0, 1, 2, 3]],             # one row: nested, or it reads as shape (4,)
        [[1, 2], [3, 4]],
        [[[1, 2], [3, 4]], [[5, 6], [7, 8]]],
        [[[1], [2, 3]], [[4]]],
        [[], [1]],
    ])
    def test_nested_lists_round_trip(self, rows):
        out = bovnar.dumps({'a': rows}, pretty=False)
        assert bovnar.loads(out) == {'a': rows}, out

    def test_a_rectangle_is_still_written_as_rows(self):
        assert bovnar.dumps({'a': [[1, 2], [3, 4]]},
                            pretty=False) == b'.a=[1,2]/[3,4];'

    def test_ragged_rows_are_nested_not_slash_separated(self):
        assert bovnar.dumps({'a': [[1], [2, 3]]},
                            pretty=False) == b'.a=[[1],[2,3]];'


@needs_lib
class TestTypedArraysSurviveDumps:
    """loads(typed=True) -> dumps() on arrays the format takes."""

    @staticmethod
    def _again(src):
        doc = bovnar.loads(src, typed=True)
        out = bovnar.dumps(doc, pretty=False)
        back = bovnar.loads(out, typed=True)
        return doc, out, back

    def test_one_unit_several_types_keeps_each_annotation(self):
        # Homogeneity is about the unit. This raised "elements carry units but
        # do not agree" although every element is in metres.
        src = b'.a=[<uint:8,m>1,<float:64,m>2.5];'
        doc, out, back = self._again(src)
        assert out == src
        assert back == doc

    def test_one_type_under_two_spellings_is_one_annotation(self):
        # <uint:64> and <uint:64,_10> are one type; m*s and s*m one unit.
        _, out, back = self._again(b'.a=[<uint:64,m>1,<uint:64,_10,m>2];')
        assert out == b'.a=<uint:64,m>[1,2];'
        assert [q.value for q in back['a']] == [1, 2]
        _, out, back = self._again(b'.a=[<float:64,m*s>1.5,<float:64,s*m>2.5];')
        assert out.count(b'<float:64') == 1
        assert back['a'][0].unit == back['a'][1].unit

    def test_two_units_are_still_refused(self):
        doc = bovnar.loads(b'.a=[<float:64,s>1,<float:64,m>3];', typed=True)
        with pytest.raises(bovnar.BovnarArgumentError):
            bovnar.dumps(doc)

    def test_a_reference_stays_a_reference(self):
        # It was written as the string "&.k0".
        src = b'.a=[&.k0,&.k1];.k0=1;.k1=2;'
        doc, out, back = self._again(src)
        assert out == src
        assert back == doc

    def test_a_symbol_stays_a_symbol(self):
        src = b'.a=[sym,abc];'
        _, out, _ = self._again(src)
        assert out == src

    def test_a_quoted_literal_stays_quoted(self):
        # Every base above ten requires the quotes; ff unquoted is a symbol.
        src = b'.a=<uint:64,_16>["ff","10"];'
        doc, out, back = self._again(src)
        assert out == src
        assert [q.value for q in back['a']] == [255, 16]


@needs_lib
class TestWriteArrayHeldToAnnotation:
    """With `vt` given, every element is held to it.

    The elements used to be range-checked against a type synthesised per
    element -- uint:64 for any non-negative int -- and the annotation was only
    ever written. So `write_array(..., vt=<uint:16>)` wrote 80800 under
    <uint:16>, a document nothing reads, and under a base-16 annotation it
    wrote the DECIMAL digits of each int: 255 went out as `255` and came back
    as 0x255."""

    @staticmethod
    def _write(rows, vt, **kw):
        w = Writer.to_mem(pretty=False)
        write_array(w, 'p', rows, vt=vt, **kw)
        w.finish()
        return w.get_output()

    def test_out_of_range_element_is_refused(self):
        with pytest.raises(BovnarArgumentError):
            self._write([80, 443, 80800], make_type_spec(ValueTypeFamily.UINT, 16))

    def test_negative_under_uint_is_refused(self):
        with pytest.raises(BovnarArgumentError):
            self._write([-1, 1], make_type_spec(ValueTypeFamily.UINT, 8))

    def test_float_under_integer_annotation_is_refused(self):
        with pytest.raises(BovnarArgumentError):
            self._write([1.5], make_type_spec(ValueTypeFamily.UINT, 32))

    def test_string_under_numeric_annotation_is_refused(self):
        with pytest.raises(BovnarArgumentError):
            self._write(["a"], make_type_spec(ValueTypeFamily.UINT, 32))

    def test_bool_under_numeric_annotation_is_refused(self):
        with pytest.raises(BovnarArgumentError):
            self._write([True], make_type_spec(ValueTypeFamily.UINT, 8))

    def test_in_range_elements_still_write(self):
        out = self._write([[-128, 127], [0, None]],
                          make_type_spec(ValueTypeFamily.SINT, 8))
        assert loads(out)['p'] == [[-128, 127], [0, None]]

    def test_non_decimal_base_writes_the_value(self):
        out = self._write([255, 16], make_type_spec(ValueTypeFamily.UINT, 16, 16))
        assert out == b'.p=<uint:16,_16>["ff","10"];'
        assert loads(out)['p'] == [255, 16]

    def test_ints_under_a_float_annotation_are_floats(self):
        out = self._write([1, 2], make_type_spec(ValueTypeFamily.FLOAT, 32))
        assert loads(out)['p'] == [1.0, 2.0]

    def test_generator_rows_keep_their_first_element(self):
        # The "is this a list of rows" probe ran all() over the rows and so ate
        # the head of a generator before a single element was written.
        w = Writer.to_mem(pretty=False)
        write_array(w, 'p', (x for x in [1, 2, 3]))
        w.finish()
        assert loads(w.get_output())['p'] == [1, 2, 3]

    def test_a_unit_object_is_a_unit(self):
        out = self._write([1.0], make_type_spec(ValueTypeFamily.FLOAT, 64),
                          vu=bovnar.Unit.parse('m'))
        assert out == b'.p=<float:64,m>[1.0];'

    @pytest.mark.parametrize("rows, vt", [
        ([80, 443, 80800], make_type_spec(ValueTypeFamily.UINT, 16)),
        ([[1, 2], [3, 1.5]], make_type_spec(ValueTypeFamily.UINT, 16)),
        ([1, 2, {'a': 1}], make_type_spec(ValueTypeFamily.UINT, 16)),
        ([1, 2, 10 ** 5000], make_type_spec(ValueTypeFamily.UINT, 16)),
        (["a", 1], make_type_spec(ValueTypeFamily.UTF8, 0)),
        ([1, 2, object()], None),
    ])
    def test_a_refusal_writes_nothing(self, rows, vt):
        # The refusal came at the bad element, after `.p = <uint:16> [80, 443,`
        # was already out: the writer was left inside an open array and every
        # later write failed with it. Now the whole array is checked first.
        w = Writer.to_mem(pretty=False)
        w.write_string('before', 'x')
        mark = w.get_output()
        with pytest.raises(BovnarArgumentError):
            write_array(w, 'p', rows, vt=vt)
        assert w.get_output() == mark
        write_array(w, 'q', [1, 2], vt=make_type_spec(ValueTypeFamily.UINT, 16))
        w.write_string('after', 'y')
        w.finish()
        assert loads(w.get_output()) == {'before': 'x', 'q': [1, 2],
                                         'after': 'y'}
