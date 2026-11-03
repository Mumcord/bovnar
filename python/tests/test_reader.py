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



import os
import tempfile
import pytest

from conftest import needs_lib
import bovnar
from bovnar.reader import Reader
from bovnar.enums import Event, ValueTypeFamily, ErrorCode
from bovnar.exceptions import BovnarParseError, BovnarArgumentError

@needs_lib
class TestReaderLifecycle:
    def test_create_and_close(self):
        r = Reader()
        r.close()
        r.close()

    def test_context_manager(self):
        with Reader() as r:
            assert r._ptr is not None
        assert r._ptr is None

    def test_closed_read_raises(self):
        r = Reader()
        r.close()
        with pytest.raises(BovnarArgumentError):
            r.read_mem(b'.x = 1;')

    def test_error_properties_before_read(self):
        with Reader() as r:
            assert r.error_code == ErrorCode.NONE
            assert r.error_line == 0
            assert r.recovery_count == 0

@needs_lib
class TestEventDelivery:
    def _collect(self, data: bytes) -> list[tuple]:
        events = []
        def cb(ev, d):
            events.append((ev, d.raw_bytes() if d else b''))
            return True
        with Reader() as r:
            r.read_mem(data, on_verified=cb)
        return events

    def test_stream_begin_fired(self, simple_scalar_uint):
        events = self._collect(simple_scalar_uint)
        assert events[0][0] == Event.STREAM_START

    def test_assignment_start_key(self, simple_scalar_uint):
        events = self._collect(simple_scalar_uint)
        keys = [raw for ev, raw in events if ev == Event.ASSIGNMENT_START]
        assert b'port' in keys

    def test_data_event_value(self, simple_scalar_uint):
        events = self._collect(simple_scalar_uint)
        values = [raw for ev, raw in events if ev == Event.DATA]
        assert b'8080' in values

    def test_type_annotation_family(self, simple_scalar_uint):
        events = self._collect(simple_scalar_uint)
        families = [raw for ev, raw in events
                    if ev == Event.TYPE_ANNOTATION_TYPE_FAMILY]

        assert any(fam.startswith(b'uint') for fam in families)

    def test_annotation_events_order(self, simple_scalar_float):
        events = [ev for ev, _ in self._collect(simple_scalar_float)]
        ann_start = events.index(Event.TYPE_ANNOTATION_START)
        ann_end   = events.index(Event.TYPE_ANNOTATION_END)
        assert ann_start < ann_end
        data_idx = events.index(Event.DATA)
        assert data_idx > ann_end

    def test_struct_events(self, struct_payload):
        events = [ev for ev, _ in self._collect(struct_payload)]
        assert Event.STRUCT_START in events
        assert Event.STRUCT_END   in events
        assert events.index(Event.STRUCT_START) < events.index(Event.STRUCT_END)

    def test_array_row_events(self, array_1d):
        events = [ev for ev, _ in self._collect(array_1d)]
        assert Event.ARRAY_ROW_START in events
        assert Event.ARRAY_ROW_END in events

    def test_array_2d_events(self, array_2d):
        events = [ev for ev, _ in self._collect(array_2d)]
        assert Event.ARRAY_DIM_START in events

@needs_lib
class TestValueTypeFamily:
    def _data_events(self, data: bytes):
        items = []
        def cb(ev, d):

            if ev in (Event.TYPE_ANNOTATION_TYPE_FAMILY,
                      Event.TYPE_ANNOTATION_TYPE_FAMILY_PARAM) and d:
                items.append(d.value_type)
            return True
        with Reader() as r:
            r.read_mem(data, on_verified=cb)
        return items

    def test_uint16_type(self, simple_scalar_uint):
        specs = self._data_events(simple_scalar_uint)
        families = [ValueTypeFamily(s.family) for s in specs]
        assert ValueTypeFamily.UINT in families

    def test_sint_type(self, simple_scalar_sint):
        specs = self._data_events(simple_scalar_sint)
        families = [ValueTypeFamily(s.family) for s in specs]
        assert ValueTypeFamily.SINT in families

    def test_float_type(self, simple_scalar_float):
        specs = self._data_events(simple_scalar_float)
        families = [ValueTypeFamily(s.family) for s in specs]
        assert ValueTypeFamily.FLOAT in families

    def test_string_type(self):

        typed_string = b'.host = <utf8> "localhost";\n'
        specs = self._data_events(typed_string)
        families = [ValueTypeFamily(s.family) for s in specs]
        assert ValueTypeFamily.UTF8 in families

    def test_untyped_integer_synthesises_uint64(self, untyped_integer):
        specs = self._data_events(untyped_integer)
        assert any(ValueTypeFamily(s.family) == ValueTypeFamily.UINT and s.width == 64
                   for s in specs)

    def test_untyped_float_synthesises_float64(self, untyped_float):
        specs = self._data_events(untyped_float)
        assert any(ValueTypeFamily(s.family) == ValueTypeFamily.FLOAT and s.width == 64
                   for s in specs)

    def test_negative_integer_synthesises_sint64(self, untyped_negative):
        specs = self._data_events(untyped_negative)
        assert any(ValueTypeFamily(s.family) == ValueTypeFamily.SINT
                   for s in specs)

@needs_lib
class TestIterMem:
    def test_yields_event_payloads(self, multi_assignment):
        with Reader() as r:
            events = list(r.iter_mem(multi_assignment))
        assert len(events) > 0
        from bovnar.reader import EventPayload
        assert all(isinstance(e, EventPayload) for e in events)

    def test_text_property(self, simple_scalar_string):
        with Reader() as r:
            events = list(r.iter_mem(simple_scalar_string))
        keys = [e.text for e in events if e.event == Event.ASSIGNMENT_START]
        assert 'host' in keys

    def test_assignment_key_found(self, multi_assignment):
        with Reader() as r:
            events = list(r.iter_mem(multi_assignment))
        keys = {e.text for e in events if e.event == Event.ASSIGNMENT_START}
        assert 'name' in keys
        assert 'age'  in keys

@needs_lib
class TestCallbackAbort:
    def test_false_return_raises(self):
        def aborting(ev, d):
            return False

        with pytest.raises(BovnarParseError) as exc_info:
            with Reader() as r:
                r.read_mem(b'.x = 1;', on_verified=aborting)
        assert exc_info.value.code == ErrorCode.SCANNER_CALLBACK_FAILED

    def test_exception_in_callback_raises(self):
        def crashing(ev, d):
            raise RuntimeError("boom")

        with pytest.raises(RuntimeError, match="boom"):
            with Reader() as r:
                r.read_mem(b'.x = 1;', on_verified=crashing)

@needs_lib
class TestReadFile:
    def test_read_file_by_path(self, simple_scalar_uint):
        with tempfile.NamedTemporaryFile(suffix='.bvnr', delete=False) as f:
            f.write(simple_scalar_uint)
            name = f.name
        try:
            keys = []
            def cb(ev, d):
                if ev == Event.ASSIGNMENT_START:
                    keys.append(d.raw_str())
                return True
            with Reader() as r:
                r.read_file(name, on_verified=cb)
            assert 'port' in keys
        finally:
            os.unlink(name)

    def test_read_fd(self, simple_scalar_uint):
        with tempfile.NamedTemporaryFile(suffix='.bvnr', delete=False) as f:
            f.write(simple_scalar_uint)
            name = f.name
        try:
            fd = os.open(name, os.O_RDONLY)
            keys = []
            def cb(ev, d):
                if ev == Event.ASSIGNMENT_START:
                    keys.append(d.raw_str())
                return True
            try:
                with Reader() as r:
                    r.read_fd(fd, on_verified=cb)
                assert 'port' in keys
            finally:
                os.close(fd)
        finally:
            os.unlink(name)

@needs_lib
class TestNestedStructures:
    def _keys(self, data: bytes) -> list[str]:
        keys = []
        def cb(ev, d):
            if ev == Event.ASSIGNMENT_START:
                keys.append(d.raw_str())
            return True
        with Reader() as r:
            r.read_mem(data, on_verified=cb)
        return keys

    def test_nested_struct_keys(self, nested_struct):
        keys = self._keys(nested_struct)
        assert 'server'   in keys
        assert 'host'     in keys
        assert 'limits'   in keys
        assert 'max_conn' in keys
        assert 'timeout'  in keys

    def test_deep_nesting(self, deep_struct):
        events = []
        def cb(ev, d):
            events.append(ev)
            return True
        with Reader() as r:
            r.read_mem(deep_struct, on_verified=cb)
        structs = [e for e in events if e == Event.STRUCT_START]
        assert len(structs) == 4

    def test_reader_nesting_matches_cli_limit(self):
        # Regression: _build_flags left the nesting limit at the reader's bare
        # 64 default, so loads()/read_mem rejected depth-65..255 documents that
        # the CLI, dom_parse() and stream.py (all 255) accept. The Reader must
        # use the same 255 hard cap; 256 is still rejected.
        def depth_ok(builder, d):
            src = builder(d).encode()
            cnt = [0]
            def cb(ev, _d):
                if ev == Event.STRUCT_START or ev == Event.ARRAY_ROW_START:
                    cnt[0] += 1
                return True
            try:
                with Reader() as r:
                    r.read_mem(src, on_verified=cb)
                return True
            except BovnarParseError:
                return False

        struct = lambda d: '.a = ' + '{.b = ' * d + '1' + ';}' * d + ';'
        array  = lambda d: '.a = ' + '[' * d + '1' + ']' * d + ';'
        for builder in (struct, array):
            assert depth_ok(builder, 65)
            assert depth_ok(builder, 255)
            assert not depth_ok(builder, 256)

    def test_array_element_count(self, array_1d):
        data_events = []
        def cb(ev, d):
            if ev == Event.DATA:
                data_events.append(d.raw_bytes())
            return True
        with Reader() as r:
            r.read_mem(array_1d, on_verified=cb)

        assert len(data_events) == 3

@needs_lib
class TestNullValues:
    def test_null_value_empty_raw(self, null_value):
        data_events = []
        def cb(ev, d):
            if ev == Event.DATA:
                data_events.append(d.raw_bytes() if d else b'__NONE__')
            return True
        with Reader() as r:
            r.read_mem(null_value, on_verified=cb)
        assert b'' in data_events

    def test_typed_null(self, null_typed):
        vts = []
        def cb(ev, d):
            if ev == Event.TYPE_ANNOTATION_TYPE_FAMILY_PARAM and d:
                vts.append(d.value_type)
            return True
        with Reader() as r:
            r.read_mem(null_typed, on_verified=cb)
        families = [ValueTypeFamily(s.family) for s in vts]
        assert ValueTypeFamily.UINT in families

@needs_lib
class TestSpecialFloats:
    def test_nan_infinity_parsed(self, special_floats):
        values = []
        def cb(ev, d):
            if ev == Event.DATA:
                values.append(d.raw_bytes())
            return True
        with Reader() as r:
            r.read_mem(special_floats, on_verified=cb)
        raw_strs = [v.decode('utf-8') for v in values if v]
        assert 'nan'  in raw_strs
        assert 'inf'  in raw_strs
        assert 'ninf' in raw_strs

@needs_lib
class TestComments:
    def test_comments_ignored(self, comment_payload):
        keys = []
        def cb(ev, d):
            if ev == Event.ASSIGNMENT_START:
                keys.append(d.raw_str())
            return True
        with Reader() as r:
            r.read_mem(comment_payload, on_verified=cb)
        assert 'foo' in keys
        assert 'bar' in keys
        assert len(keys) == 2

@needs_lib
class TestStringFeatures:
    def test_string_value(self, simple_scalar_string):
        values = []
        def cb(ev, d):
            if ev == Event.DATA:
                values.append(d.raw_str())
            return True
        with Reader() as r:
            r.read_mem(simple_scalar_string, on_verified=cb)
        assert 'localhost' in values

    def test_string_concat(self, string_concat):
        values = []
        def cb(ev, d):
            if ev == Event.DATA and d:
                values.append(d.raw_str())
            return True
        with Reader() as r:
            r.read_mem(string_concat, on_verified=cb)
        assert 'hello world' in values

@needs_lib
class TestMaxFileSize:
    def test_max_file_size_exceeded(self):

        data = b'.x = 1; .y = 2; .z = 3; .w = 4; .v = 5; .u = 6; .t = 7;\n'
        with pytest.raises(BovnarParseError) as exc_info:
            with Reader() as r:
                r.read_mem(data, max_file_size=10)
        assert exc_info.value.code == ErrorCode.FILE_TOO_LONG

    def test_max_file_size_not_exceeded(self, simple_scalar_uint):

        with Reader() as r:
            r.read_mem(simple_scalar_uint, max_file_size=1024)

@needs_lib
class TestInvalidArguments:
    def test_non_bytes_raises(self):
        with Reader() as r:
            with pytest.raises(BovnarArgumentError):
                r.read_mem("not bytes")

    def test_invalid_fd(self):
        with Reader() as r:
            with pytest.raises(BovnarArgumentError):
                r.read_fd(-1)



class TestFfiDeclarations:
    """Regression test for Bug 1: _declare_functions must not reference
    writer symbols that do not exist in the C library.  A missing symbol
    causes AttributeError inside _declare_functions, which makes every
    candidate path fail and raises BovnarLibraryNotFound, hiding all
    integration tests."""

    def test_library_loads_without_attribute_error(self):
        from bovnar._ffi import _candidate_paths, _declare_functions
        import ctypes, re
        loaded = False
        for p in _candidate_paths():
            try:
                lib = ctypes.CDLL(p, use_errno=True)
                _declare_functions(lib)
                loaded = True
                break
            except AttributeError as e:
                raise AssertionError(
                    f"_declare_functions raised AttributeError: {e}\n"
                    "A non-existent symbol is declared in _ffi.py"
                ) from e
            except OSError:
                continue
        if not loaded:
            import pytest
            pytest.skip("shared library not found on this system")


@needs_lib
class TestTypedRoundtripFidelity:
    """High-level loads(typed=True)/dumps fidelity and the streaming-vs-DOM
    validation boundary."""

    def test_symbol_and_reference_keep_kind(self):
        # A symbol and a (plain) reference must survive a typed round-trip as a
        # symbol / &.path, not be downgraded to a quoted string.
        src = '.target = 1;\n.ref = &.target;\n.sym = tcp;\n'
        out = bovnar.dumps(bovnar.loads(src, typed=True)).decode()
        assert '.ref = &.target;' in out
        assert '.sym = tcp;' in out
        assert '"' not in out                      # nothing got quoted
        # stable / idempotent
        assert bovnar.dumps(bovnar.loads(out, typed=True)).decode() == out

    def test_reference_indexing_roundtrips_with_directive(self):
        # &.m[0][1] is spec 1.1; dumps must prepend #!bovnar 1.1 so it re-parses.
        src = '#!bovnar 1.1\n.m = [1,2,3]/[4,5,6];\n.cell = &.m[0][1];\n'
        out = bovnar.dumps(bovnar.loads(src, typed=True)).decode()
        assert out.startswith('#!bovnar 1.1')
        assert '&.m[0][1]' in out
        assert bovnar.dumps(bovnar.loads(out, typed=True)).decode() == out

    def test_untyped_symbol_reference_stay_str(self):
        # the dict model maps a symbol/reference to a plain str (unchanged)
        d = bovnar.loads('.sym = tcp;\n.ref = &.sym;\n')
        assert d == {'sym': 'tcp', 'ref': '.sym'}

    @pytest.mark.parametrize("src", [
        '.a = 1;\n.a = 2;\n',            # duplicate top-level key
        '.m = [[1, 2], [3, 4, 5]];\n',   # non-rectangular matrix
        '.a = [1, "two"];\n',            # heterogeneous array
    ])
    def test_loads_is_streaming_not_dom_validation(self, src):
        # loads() is a single-pass streaming parse: DOM-tier checks (duplicate
        # keys, homogeneity, rectangularity) are NOT enforced — dom_parse() is.
        bovnar.loads(src)                                  # accepted (no raise)
        with pytest.raises(BovnarParseError):
            bovnar.dom_parse(src.encode())                 # rejected by full validation


@needs_lib
def test_a_kept_payload_outlives_the_reader():
    # The snapshot a callback receives used to copy only its by-value fields:
    # data, frac_data, unit_source and conv.num/conv.den stayed pointers into
    # the reader, so a BvnrData kept past the callback read freed memory once
    # the reader was gone -- raw_bytes() returned garbage and
    # converted_rational() walked a freed bignum. Every accessor must now give
    # the answer it gave inside the callback.
    import gc
    from bovnar.enums import BaseUnit
    from bovnar.structs import make_unit_si

    def accessors(d):
        return (d.raw_bytes(), d.raw_str(), d.frac_str(), d.unit_source_str(),
                bool(d.converted), d.converted_str(), d.converted_rational(),
                d.converted_in_base(16) if d.converted else None)

    def want(d):
        if d.unit_source_str() == 'm':                 # 1 m in miles: no
            return (make_unit_si(BaseUnit.MILE), 10)   # finite expansion
        if d.unit_source_str() == 'k~m':
            return (make_unit_si(BaseUnit.METER), 10)
        return None

    kept, inside, wanted = [], [], []

    def on_event(ev, d):
        if ev == Event.DATA and d is not None:
            kept.append(d)
            inside.append(accessors(d))
        return True

    def want_keep(d):
        wanted.append((d, d.raw_bytes(), d.unit_source_str()))
        return want(d)

    r = Reader()
    r.read_mem(b'#!bovnar 1.2\n.d = 1 m;\n.k = 5 k~m;\n'
               b'.t = 2024-01-02T03:04:05.123456Z;\n.u = 5 ucum:mL{total};\n'
               b'.s = "' + b'x' * 300 + b'";\n',
               on_verified=on_event, want_unit=want_keep,
               want_unit_allow_nonterminating=True)
    r.close()
    del r
    # reuse the freed memory before looking again
    churn = [bytearray(b'Z' * 4096) for _ in range(2000)]
    gc.collect()
    del churn

    after = [accessors(d) for d in kept]
    assert after == inside
    assert after[0][6] == (125, 201168)                # 1 m / 1609.344 m
    assert after[0][5] is None
    assert after[1][5] == '5000' and after[1][7] == '1388'
    assert after[2][2] == '123456'
    assert after[3][3] == 'ucum:mL{total}'
    assert after[4][0] == b'x' * 300
    assert [(d.raw_bytes(), d.unit_source_str()) for d, _, _ in wanted] == \
        [(raw, src) for _, raw, src in wanted]
