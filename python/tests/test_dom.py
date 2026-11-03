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

import math
import os
import tempfile
import pytest

from conftest import needs_lib

import bovnar
from bovnar.dom import DomDoc, DomNode, DomType
from bovnar.enums import BaseUnit, ErrorCode, SIPrefix, ValueTypeFamily
from bovnar.exceptions import BovnarParseError


_SCALAR_INT    = b".x = 42;\n"
_SCALAR_UINT32 = b".x = <uint:32> 42;\n"
_SCALAR_INT8   = b".x = <sint:8> -5;\n"
_SCALAR_FLOAT  = b".v = <float:64,m/s> 9.81;\n"
_SCALAR_STRING = b'.s = "hello";\n'
_NULL          = b".n = ;\n"
_SYMBOL        = b".flag = ok;\n"
_STRUCT        = b".obj = { .a = 1; .b = 2; };\n"
_ARRAY_1D      = b".arr = [1, 2, 3];\n"
_ARRAY_2D      = b".mat = [1, 2]/[3, 4];\n"
_NESTED_STRUCT = b".outer = { .inner = { .val = 99; }; };\n"
_MULTI_KEY     = b".x = 1;\n.y = 2;\n.z = 3;\n"
_STRUCT_ARRAY  = b".recs = [{.a = 10; .b = 20;}];\n"
_TYPED_UNIT    = b".temp = <float:64,K> 300.0;\n"


@needs_lib
class TestDomDocParse:
    def test_parse_bytes(self):
        doc = DomDoc.parse(_SCALAR_INT)
        assert isinstance(doc, DomDoc)

    def test_parse_bytearray(self):
        doc = DomDoc.parse(bytearray(_SCALAR_INT))
        assert len(doc) == 1

    def test_parse_file(self):
        with tempfile.NamedTemporaryFile(suffix='.bvnr', delete=False) as f:
            f.write(_SCALAR_INT)
            name = f.name
        try:
            doc = DomDoc.parse_file(name)
            assert len(doc) == 1
        finally:
            os.unlink(name)

    def test_parse_fd(self):
        with tempfile.NamedTemporaryFile(suffix='.bvnr', delete=False) as f:
            f.write(_MULTI_KEY)
            name = f.name
        try:
            fd = os.open(name, os.O_RDONLY)
            try:
                doc = DomDoc.parse_fd(fd)
                assert len(doc) == 3
            finally:
                os.close(fd)
        finally:
            os.unlink(name)

    def test_invalid_bvnr_raises(self):
        with pytest.raises(BovnarParseError):
            DomDoc.parse(b"!!! this is not valid BVNR !!!")

    @pytest.mark.parametrize("src, code", [
        (b'.a = [1, "two"];',                ErrorCode.ARRAY_ELEMENT_TYPE_MISMATCH),
        (b'.a = [<float:64,m> 1.0, <float:64,k~g> 2.0];',
                                             ErrorCode.ARRAY_ELEMENT_TYPE_MISMATCH),
        (b'.a = [[1, 2], [3, 4, 5]];',       ErrorCode.ARRAY_ROW_SIZE_MISMATCH),
        (b'.a = [{.x = 1;}, {.y = 1;}];',    ErrorCode.STRUCT_SHAPE_MISMATCH),
        (b'.x = 1; .x = 2;',                 ErrorCode.DUPLICATE_STRUCT_KEY),
        (b'.s = {.x = 1; .x = 2;};',         ErrorCode.DUPLICATE_STRUCT_KEY),
    ])
    def test_spec_1_0_homogeneity_raises(self, src, code):
        # The materialised-document rules (spec 1.0 §7.4 / §8.1) carry frozen
        # error codes 39/40/41; the DOM parser must surface them as a proper
        # BovnarParseError, not crash on an unmapped ErrorCode.
        with pytest.raises(BovnarParseError) as exc:
            DomDoc.parse(src)
        assert exc.value.code == code

    def test_empty_doc(self):
        doc = DomDoc.parse(b"")
        assert len(doc) == 0

    def test_repr_contains_ptr(self):
        doc = DomDoc.parse(_SCALAR_INT)
        assert 'DomDoc' in repr(doc)


@needs_lib
class TestDomDocAccess:
    def test_len(self):
        doc = DomDoc.parse(_MULTI_KEY)
        assert len(doc) == 3

    def test_contains_existing_key(self):
        doc = DomDoc.parse(_SCALAR_INT)
        assert 'x' in doc

    def test_contains_missing_key(self):
        doc = DomDoc.parse(_SCALAR_INT)
        assert 'missing' not in doc

    def test_getitem_returns_node(self):
        doc = DomDoc.parse(_SCALAR_INT)
        node = doc['x']
        assert isinstance(node, DomNode)

    def test_getitem_missing_raises_keyerror(self):
        doc = DomDoc.parse(_SCALAR_INT)
        with pytest.raises(KeyError):
            _ = doc['no_such_key']

    def test_entries_returns_list_of_tuples(self):
        doc = DomDoc.parse(_MULTI_KEY)
        entries = doc.entries()
        assert len(entries) == 3
        keys = [k for k, _ in entries]
        assert 'x' in keys
        assert 'y' in keys
        assert 'z' in keys

    def test_iter_yields_tuples(self):
        doc = DomDoc.parse(_MULTI_KEY)
        pairs = list(doc)
        assert len(pairs) == 3
        for k, v in pairs:
            assert isinstance(k, str)
            assert isinstance(v, DomNode)

    def test_to_dict_scalar(self):
        doc = DomDoc.parse(_SCALAR_INT)
        d   = doc.to_dict()
        assert isinstance(d, dict)
        assert d['x'] == 42

    def test_to_dict_multi(self):
        doc = DomDoc.parse(_MULTI_KEY)
        d   = doc.to_dict()
        assert d == {'x': 1, 'y': 2, 'z': 3}


@needs_lib
class TestDomDocLookup:
    def test_lookup_top_level(self):
        doc  = DomDoc.parse(_SCALAR_INT)
        node = doc.lookup('x')
        assert node is not None
        assert isinstance(node, DomNode)

    def test_lookup_missing_returns_none(self):
        doc = DomDoc.parse(_SCALAR_INT)
        assert doc.lookup('no_such') is None

    def test_lookup_nested_dot_path(self):
        doc  = DomDoc.parse(_NESTED_STRUCT)
        node = doc.lookup('outer.inner.val')
        assert node is not None
        assert node.as_i64() == 99

    def test_lookup_one_level_deep(self):
        doc  = DomDoc.parse(_NESTED_STRUCT)
        node = doc.lookup('outer.inner')
        assert node is not None
        assert node.dom_type == DomType.STRUCT

    def test_lookup_array_index(self):
        doc = DomDoc.parse(b'#!bovnar 1.1\n.matrix = [10, 20, 30]/[40, 50, 60];\n')
        assert doc.lookup('matrix[0][1]').as_i64() == 20
        assert doc.lookup('matrix[1][0]').as_i64() == 40
        assert doc.lookup('matrix[0]') is None       # partial index of a matrix
        assert doc.lookup('matrix[0][9]') is None     # out of range

    def test_lookup_1d_and_nested_index(self):
        doc = DomDoc.parse(b'#!bovnar 1.1\n.a = [1, 2, 3];\n.n = [[1, 2], [3, 4]];\n')
        assert doc.lookup('a[2]').as_i64() == 3
        assert doc.lookup('n[0][1]').as_i64() == 2

    def test_lookup_index_then_field(self):
        doc = DomDoc.parse(
            b'#!bovnar 1.1\n.rows = [{.name = "a";}, {.name = "b";}];\n')
        assert doc.lookup('rows[1].name').as_str() == 'b'


@needs_lib
class TestDomNodeType:
    def test_int_type(self):
        doc = DomDoc.parse(_SCALAR_INT)
        assert doc['x'].dom_type == DomType.INT

    def test_float_type(self):
        doc = DomDoc.parse(_SCALAR_FLOAT)
        assert doc['v'].dom_type == DomType.FLOAT

    def test_string_type(self):
        doc = DomDoc.parse(_SCALAR_STRING)
        assert doc['s'].dom_type == DomType.STRING

    def test_null_type(self):
        doc = DomDoc.parse(_NULL)
        assert doc['n'].dom_type == DomType.NULL

    def test_symbol_type(self):
        doc = DomDoc.parse(_SYMBOL)
        assert doc['flag'].dom_type == DomType.SYMBOL

    def test_struct_type(self):
        doc = DomDoc.parse(_STRUCT)
        assert doc['obj'].dom_type == DomType.STRUCT

    def test_array_type(self):
        doc = DomDoc.parse(_ARRAY_1D)
        assert doc['arr'].dom_type == DomType.ARRAY

    def test_slash_matrix_row_count_uncapped(self):
        # Regression: the DOM used to clamp the recorded /-row count (num_dims)
        # at 8, so a matrix with more than 8 rows reported the wrong shape and
        # convert/numpy silently flattened or mis-grouped it. array_dims() must
        # report the true number of rows.
        rows = '/'.join('[%d,%d]' % (i, i + 1) for i in range(10))
        doc = DomDoc.parse(('.m=<uint:16>' + rows + ';').encode())
        node = doc['m']
        assert node.dom_type == DomType.ARRAY
        assert node.array_dims() == 10          # 10 rows, not capped to 8
        assert len(node) == 20                  # 10 rows x 2 columns, flat

    def test_repr_contains_type(self):
        doc  = DomDoc.parse(_SCALAR_INT)
        node = doc['x']
        assert 'DomNode' in repr(node)


@needs_lib
class TestDomNodeIntAccessors:
    def test_as_i64(self):
        doc = DomDoc.parse(_SCALAR_INT)
        assert doc['x'].as_i64() == 42

    def test_as_u64_positive(self):
        doc = DomDoc.parse(_SCALAR_INT)
        assert doc['x'].as_u64() == 42

    def test_as_i32(self):
        doc = DomDoc.parse(_SCALAR_UINT32)
        assert doc['x'].as_i32() == 42

    def test_as_u32(self):
        doc = DomDoc.parse(_SCALAR_UINT32)
        assert doc['x'].as_u32() == 42

    def test_as_i8_negative(self):
        doc = DomDoc.parse(_SCALAR_INT8)
        assert doc['x'].as_i8() == -5

    def test_as_u8_small(self):
        doc = DomDoc.parse(b".x = <uint:8> 200;\n")
        assert doc['x'].as_u8() == 200

    def test_as_i16(self):
        doc = DomDoc.parse(b".x = <sint:16> -1000;\n")
        assert doc['x'].as_i16() == -1000

    def test_as_u16(self):
        doc = DomDoc.parse(b".x = <uint:16> 8080;\n")
        assert doc['x'].as_u16() == 8080

    def test_wrong_type_raises(self):
        doc = DomDoc.parse(_SCALAR_STRING)
        with pytest.raises(ValueError):
            doc['s'].as_i64()

    def test_as_int_str_base10(self):
        doc = DomDoc.parse(_SCALAR_INT)
        s   = doc['x'].as_int_str(10)
        assert s == '42'

    def test_as_int_str_base16(self):
        doc = DomDoc.parse(b".x = <uint:8> 255;\n")
        s   = doc['x'].as_int_str(16)
        assert s.lower() in ('ff', '0xff')

    def test_to_python_uint64_high_bit(self):
        # Regression: to_python() called get_i64 first, which succeeds (unchecked)
        # for a width<=64 value, so a uint64 >= 2**63 came back as a negative
        # two's-complement number. It must read by declared signedness.
        cases = {
            18446744073709551615: b".x = <uint:64> 18446744073709551615;\n",  # 2**64-1
            9223372036854775808:  b".x = <uint:64> 9223372036854775808;\n",   # 2**63
            9223372036854775809:  b".x = <uint:64> 9223372036854775809;\n",   # 2**63+1
        }
        for want, src in cases.items():
            doc = DomDoc.parse(src)
            assert doc['x'].to_python() == want, f"to_python {src!r}"
            assert doc.to_dict()['x'] == want, f"to_dict {src!r}"

    def test_to_python_sint_and_bignum(self):
        # Signed values and >64-bit bignums must still round-trip through to_python.
        assert DomDoc.parse(b".x = <sint:64> -5;\n")['x'].to_python() == -5
        big = DomDoc.parse(bovnar.dumps({'x': 2**100}))['x'].to_python()
        assert big == 2**100
        neg = DomDoc.parse(bovnar.dumps({'x': -(2**70)}))['x'].to_python()
        assert neg == -(2**70)

    def test_to_python_datetime_is_signed(self):
        # Regression: datetime is a SIGNED epoch-seconds carrier, but to_python()
        # routed non-sint families through the unsigned accessor, so a pre-1970
        # (negative) datetime decoded as a huge uint64 instead of a negative int.
        doc = DomDoc.parse(b"#!bovnar 1.1\n.t = 1960-01-01;\n")
        assert doc['t'].to_python() == -315619200
        # positive epoch and the matching loads() path agree
        doc2 = DomDoc.parse(b"#!bovnar 1.1\n.t = <datetime:64,unix> 1750000000;\n")
        assert doc2['t'].to_python() == 1750000000


@needs_lib
class TestDomNodeFloatAccessor:
    def test_as_float(self):
        doc = DomDoc.parse(_SCALAR_FLOAT)
        v   = doc['v'].as_float()
        assert math.isclose(v, 9.81, rel_tol=1e-9)

    def test_wrong_type_raises(self):
        doc = DomDoc.parse(_SCALAR_STRING)
        with pytest.raises(ValueError):
            doc['s'].as_float()


@needs_lib
class TestDomNodeStringAccessors:
    def test_as_str_string(self):
        doc = DomDoc.parse(_SCALAR_STRING)
        assert doc['s'].as_str() == 'hello'

    def test_as_str_symbol(self):
        doc = DomDoc.parse(_SYMBOL)
        assert doc['flag'].as_str() == 'ok'

    def test_as_str_wrong_type_raises(self):
        doc = DomDoc.parse(_SCALAR_INT)
        with pytest.raises(ValueError):
            doc['x'].as_str()


@needs_lib
class TestDomNodeNullIsNull:
    def test_is_null_true(self):
        doc = DomDoc.parse(_NULL)
        assert doc['n'].is_null()

    def test_is_null_false(self):
        doc = DomDoc.parse(_SCALAR_INT)
        assert not doc['x'].is_null()


@needs_lib
class TestDomNodeUnit:
    def test_unit_str_kelvin(self):
        doc = DomDoc.parse(_TYPED_UNIT)
        s   = doc['temp'].unit_str
        assert 'K' in s

    def test_value_type_family_float(self):
        doc = DomDoc.parse(_TYPED_UNIT)
        vt  = doc['temp'].value_type
        assert ValueTypeFamily(vt.family) == ValueTypeFamily.FLOAT

    def test_value_in_base_units_kelvin(self):
        doc = DomDoc.parse(_TYPED_UNIT)
        v   = doc['temp'].value_in_base_units()
        assert math.isclose(v, 300.0, rel_tol=1e-6)

    def test_value_in_base_units_km(self):
        doc = DomDoc.parse(b".d = <uint:32,k~m> 5;\n")
        v   = doc['d'].value_in_base_units()
        assert math.isclose(v, 5000.0, rel_tol=1e-6)

    def test_no_unit_str_empty(self):
        doc = DomDoc.parse(_SCALAR_INT)
        s   = doc['x'].unit_str
        assert isinstance(s, str)
        assert s == ''

    def test_unit_str_agrees_with_quantity(self):
        # doc/09 §13.3 promises '' for a value with no unit, as Quantity.unit_str
        # gives; the DOM returned the C formatter's literal 'no_unit' instead.
        src = (b'.a = 1; .b = <uint:32,no_unit> 7; .s = "x"; .m = 5 m;'
               b' .p = 5 %; .t = { .q = 1; };')
        doc = DomDoc.parse(src)
        typed = bovnar.loads(src, typed=True)
        for k in ('a', 'b', 's', 'm', 'p'):
            assert doc[k].unit_str == typed[k].unit_str, k
        assert doc['a'].unit_str == '' and doc['b'].unit_str == ''
        assert doc['m'].unit_str == 'm' and doc['p'].unit_str == '%'
        assert doc['t'].unit_str == ''


@needs_lib
class TestDomNodeStructAccess:
    def test_len_struct(self):
        doc = DomDoc.parse(_STRUCT)
        assert len(doc['obj']) == 2

    def test_getitem_str_key(self):
        doc = DomDoc.parse(_STRUCT)
        a   = doc['obj']['a']
        assert isinstance(a, DomNode)
        assert a.as_i64() == 1

    def test_getitem_missing_struct_key_raises(self):
        doc = DomDoc.parse(_STRUCT)
        with pytest.raises(KeyError):
            _ = doc['obj']['no_such']

    def test_contains_struct_key(self):
        doc = DomDoc.parse(_STRUCT)
        assert 'a' in doc['obj']
        assert 'z' not in doc['obj']

    def test_entries_struct(self):
        doc = DomDoc.parse(_STRUCT)
        entries = doc['obj'].entries()
        assert len(entries) == 2
        keys = {k for k, _ in entries}
        assert keys == {'a', 'b'}

    def test_iter_struct(self):
        doc   = DomDoc.parse(_STRUCT)
        pairs = list(doc['obj'])
        assert len(pairs) == 2
        for k, v in pairs:
            assert isinstance(k, str)
            assert isinstance(v, DomNode)

    def test_getitem_wrong_type_raises(self):
        doc = DomDoc.parse(_SCALAR_INT)
        with pytest.raises(TypeError):
            _ = doc['x']['key']


@needs_lib
class TestDomNodeArrayAccess:
    def test_len_array(self):
        doc = DomDoc.parse(_ARRAY_1D)
        assert len(doc['arr']) == 3

    def test_getitem_int_index(self):
        doc  = DomDoc.parse(_ARRAY_1D)
        node = doc['arr'][0]
        assert isinstance(node, DomNode)
        assert node.as_i64() == 1

    def test_getitem_last_element(self):
        doc = DomDoc.parse(_ARRAY_1D)
        assert doc['arr'][2].as_i64() == 3

    def test_getitem_negative_index(self):
        doc = DomDoc.parse(_ARRAY_1D)
        assert doc['arr'][-1].as_i64() == 3

    def test_getitem_out_of_range_raises(self):
        doc = DomDoc.parse(_ARRAY_1D)
        with pytest.raises(IndexError):
            _ = doc['arr'][99]

    def test_iter_array(self):
        doc   = DomDoc.parse(_ARRAY_1D)
        nodes = list(doc['arr'])
        assert len(nodes) == 3
        assert all(isinstance(n, DomNode) for n in nodes)

    def test_array_values_correct(self):
        doc    = DomDoc.parse(_ARRAY_1D)
        values = [doc['arr'][i].as_i64() for i in range(3)]
        assert values == [1, 2, 3]

    def test_2d_array_count(self):
        doc = DomDoc.parse(_ARRAY_2D)
        assert len(doc['mat']) == 4

    def test_array_dims_1d(self):
        doc = DomDoc.parse(_ARRAY_1D)
        assert doc['arr'].array_dims() == 1

    def test_array_dims_2d(self):
        doc = DomDoc.parse(_ARRAY_2D)
        assert doc['mat'].array_dims() == 2

    def test_getitem_wrong_type_raises(self):
        doc = DomDoc.parse(_SCALAR_INT)
        with pytest.raises(TypeError):
            _ = doc['x'][0]

    def test_getitem_str_key_on_array_raises(self):
        doc = DomDoc.parse(_ARRAY_1D)
        with pytest.raises(TypeError):
            _ = doc['arr']['bad']


@needs_lib
class TestDomNodeToPython:
    def test_int_scalar(self):
        doc = DomDoc.parse(_SCALAR_INT)
        assert doc['x'].to_python() == 42

    def test_float_scalar(self):
        doc = DomDoc.parse(_SCALAR_FLOAT)
        v   = doc['v'].to_python()
        assert math.isclose(v, 9.81, rel_tol=1e-9)

    def test_string_scalar(self):
        doc = DomDoc.parse(_SCALAR_STRING)
        assert doc['s'].to_python() == 'hello'

    def test_null(self):
        doc = DomDoc.parse(_NULL)
        assert doc['n'].to_python() is None

    def test_struct(self):
        doc = DomDoc.parse(_STRUCT)
        d   = doc['obj'].to_python()
        assert isinstance(d, dict)
        assert d['a'] == 1
        assert d['b'] == 2

    def test_array(self):
        doc = DomDoc.parse(_ARRAY_1D)
        lst = doc['arr'].to_python()
        assert isinstance(lst, list)
        assert lst == [1, 2, 3]

    def test_nested_struct(self):
        doc = DomDoc.parse(_NESTED_STRUCT)
        d   = doc['outer'].to_python()
        assert d == {'inner': {'val': 99}}

    def test_struct_in_array(self):
        doc = DomDoc.parse(_STRUCT_ARRAY)
        lst = doc['recs'].to_python()
        assert isinstance(lst, list)
        assert len(lst) == 1
        assert lst[0] == {'a': 10, 'b': 20}


@needs_lib
class TestDomDocGcSafety:
    def test_node_survives_doc_in_scope(self):
        doc  = DomDoc.parse(_SCALAR_INT)
        node = doc['x']
        assert node.as_i64() == 42
        assert node._doc is doc

    def test_multiple_nodes_from_same_doc(self):
        doc = DomDoc.parse(_MULTI_KEY)
        nodes = [doc[k] for k in ('x', 'y', 'z')]
        values = [n.as_i64() for n in nodes]
        assert values == [1, 2, 3]


@needs_lib
class TestDomSmallEdges:
    def test_null_pointer_leaves_nothing_for_del_to_trip_on(self):
        import gc
        import sys
        seen = []
        old = sys.unraisablehook
        sys.unraisablehook = lambda u: seen.append(u)
        try:
            with pytest.raises(MemoryError):
                DomDoc(0)
            gc.collect()
        finally:
            sys.unraisablehook = old
        assert seen == []

    def test_non_str_key_is_simply_absent(self):
        doc = DomDoc.parse(b'.s={.a=1;};')
        assert 5 not in doc['s']
        assert 5 not in doc
        assert 'a' in doc['s']

    def test_empty_rows_keep_their_count(self):
        # The streaming tier gives [[], []]; the split was skipped for an empty
        # flat list and the DOM gave [].
        src = b'.a=[]/[];.b=[];.c=[]/[]/[];'
        assert DomDoc.parse(src).to_dict() == bovnar.loads(src)
        assert bovnar.loads(src) == {'a': [[], []], 'b': [], 'c': [[], [], []]}
