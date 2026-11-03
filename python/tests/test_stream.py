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
from bovnar import stream
from bovnar.exceptions import BovnarParseError


# ---------------------------------------------------------------------------
# Multi-document record framing
# ---------------------------------------------------------------------------
@needs_lib
class TestMultiDocument:

    def test_roundtrip_dicts(self):
        docs = [{"id": 1, "name": "alpha"},
                {"id": 2, "name": "beta"},
                {"flag": True, "ratio": 3.5}]
        blob = stream.dump_documents(docs)
        assert blob.startswith(b"BVF1")
        assert stream.load_documents(blob) == docs

    def test_empty_stream(self):
        assert stream.load_documents(b"") == []

    def test_single_empty_document(self):
        # A frame with a zero-length payload is a valid empty document.
        blob = stream.frame_documents([b""])
        assert stream.load_documents(blob) == [{}]

    def test_frame_raw_bytes(self):
        good = bovnar.dumps({"x": 7})
        blob = stream.frame_documents([good, good])
        assert stream.load_documents(blob) == [{"x": 7}, {"x": 7}]

    def test_strict_aborts_on_bad_document(self):
        good = bovnar.dumps({"ok": 1})
        blob = stream.frame_documents([good, b".broken = @@@;", good])
        with pytest.raises(BovnarParseError):
            stream.load_documents(blob)            # default: strict

    def test_continue_past_failed(self):
        good = bovnar.dumps({"ok": 1})
        blob = stream.frame_documents([good, b".broken = @@@;", good])
        res = stream.load_documents(blob, continue_past_failed=True)
        assert res == [{"ok": 1}, None, {"ok": 1}]

    def test_truncated_frame_rejected(self):
        blob = stream.frame_documents([bovnar.dumps({"a": 1})])
        with pytest.raises(BovnarParseError):
            stream.load_documents(blob[:-3])       # lop off payload bytes

    def test_oversized_declared_length_rejected(self):
        # "BVF1" + len=100_000_000 (LE) + a few payload bytes only.
        header = b"BVF1" + (100_000_000).to_bytes(8, "little")
        with pytest.raises(BovnarParseError):
            stream.load_documents(header + b".x = ")

    def test_typed_documents(self):
        blob = stream.frame_documents([b".port = <uint:16> 8080;\n"])
        res = stream.load_documents(blob, typed=True)
        assert len(res) == 1
        q = res[0]["port"]
        assert isinstance(q, bovnar.Quantity)


# ---------------------------------------------------------------------------
# Octet multiplexing
# ---------------------------------------------------------------------------
@needs_lib
class TestMultiplexing:

    def test_simple_roundtrip(self):
        msgs = [(1, b"hello"), (42, b"world!!"), (1, b"bye")]
        assert stream.mux_load(stream.mux_dump(msgs)) == msgs

    def test_empty_message(self):
        msgs = [(5, b""), (5, b"x")]
        assert stream.mux_load(stream.mux_dump(msgs)) == msgs

    def test_large_cross_chunk_message(self):
        # Far larger than one 64 KiB octet chunk: exercises split + reassembly.
        big = bytes((i * 7 + 1) & 0xFF for i in range(200000))
        msgs = [(7, big), (9, b"small")]
        back = stream.mux_load(stream.mux_dump(msgs))
        assert back == msgs

    def test_boundary_sizes(self):
        sizes = [0, 1, 65535, 65536, 65537, 131072]
        msgs = [(i % 3, bytes((k + i) & 0xFF for k in range(n)))
                for i, n in enumerate(sizes)]
        assert stream.mux_load(stream.mux_dump(msgs)) == msgs

    def test_interleaved_channels(self):
        msgs = [(1, b"a"), (2, b"bb"), (1, b"ccc"), (3, b"dddd"), (2, b"e")]
        assert stream.mux_load(stream.mux_dump(msgs)) == msgs

    def test_key_scope(self):
        # A mux stream under "data": scoping to the right key round-trips, a
        # non-matching key ignores the stream, and no scope is the default.
        msgs = [(1, b"hi"), (2, b"there")]
        blob = stream.mux_dump(msgs, key="data")
        assert stream.mux_load(blob, key="data") == msgs
        assert stream.mux_load(blob, key="other") == []
        assert stream.mux_load(blob) == msgs


# ---------------------------------------------------------------------------
# Document-in-document
# ---------------------------------------------------------------------------
@needs_lib
class TestDocumentInDocument:

    def test_embed_and_parse(self):
        inner = bovnar.dumps({"nested": 99, "label": "inner"})
        outer = stream.embed_document(inner, key="payload")
        top = bovnar.loads(outer)               # octet stream surfaces as bytes
        assert isinstance(top["payload"], (bytes, bytearray))
        assert stream.parse_embedded(top["payload"]) == {"nested": 99, "label": "inner"}

    def test_embed_empty_document(self):
        outer = stream.embed_document(b"", key="payload")
        top = bovnar.loads(outer)
        assert stream.parse_embedded(top["payload"]) == {}

    def test_large_embed(self):
        # An inner document larger than one 64 KiB octet chunk: exercises the
        # writer's buffer growth and multi-chunk embedding.
        inner = bovnar.dumps({"data": list(range(20000))})
        assert len(inner) > 65536
        outer = stream.embed_document(inner, key="payload")
        rec = stream.parse_embedded(bovnar.loads(outer)["payload"])
        assert rec["data"] == list(range(20000))

    def test_nested_two_levels(self):
        deepest = bovnar.dumps({"v": 1})
        mid     = stream.embed_document(deepest, key="inner")
        outer   = stream.embed_document(mid, key="outer")
        lvl1    = bovnar.loads(outer)
        lvl2    = stream.parse_embedded(lvl1["outer"])   # parses the mid doc...
        # ...whose "inner" octet payload is the deepest doc.
        assert stream.parse_embedded(bovnar.loads(mid)["inner"]) == {"v": 1}
        assert "inner" in lvl2


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
def test_constants_present():
    assert stream.BVNR_FILESIZE_UNLIMITED == 0
    assert stream.BVNR_DOC_DEFAULT_MAX == 256 * 1024 * 1024
    assert stream.BVNR_MUX_DEFAULT_MAX == 64 * 1024 * 1024


@needs_lib
def test_mux_channel_out_of_range_is_refused():
    # -1 crossed the FFI as a uint64_t and was written as channel 2**64-1.
    from bovnar.exceptions import BovnarArgumentError
    with pytest.raises(BovnarArgumentError):
        stream.mux_dump([(-1, b'x')])
    with pytest.raises(BovnarArgumentError):
        stream.mux_dump([(2 ** 64, b'x')])
    assert stream.mux_load(stream.mux_dump([(2 ** 64 - 1, b'x')])) == \
        [(2 ** 64 - 1, b'x')]


@needs_lib
def test_parse_embedded_reports_the_real_error():
    # Every failure was GOT_INCOMPLETE_BVNR_STREAM, whatever went wrong.
    from bovnar.enums import ErrorCode
    with pytest.raises(BovnarParseError) as ei:
        stream.parse_embedded(b'.a=@;')
    assert ei.value.code == ErrorCode.UNEXPECTED_INPUT_BYTE
    assert (ei.value.line, ei.value.offset) == (1, 3)
    with pytest.raises(BovnarParseError) as ei:
        stream.parse_embedded(b'.a={.b=1;')
    assert ei.value.code == ErrorCode.GOT_INCOMPLETE_BVNR_STREAM


@needs_lib
def test_mux_load_names_the_demux_error():
    # Every demux error used to read "demux desync", including a stream that
    # ends part-way through a message, which bovnar_stream.h keeps distinct
    # from a framing desync.
    from bovnar.enums import ErrorCode
    with pytest.raises(BovnarParseError) as ei:
        stream.mux_load(b'.mux=\x00\x01\x04\x00\x02\x05ab\x00;')   # 5 declared, 2 sent
    assert ei.value.code == ErrorCode.OCTET_STREAM_TRUNCATED
    assert 'desync' not in str(ei.value)
    assert 'octet_stream_truncated' in str(ei.value)

    with pytest.raises(BovnarParseError) as ei:
        stream.mux_load(stream.mux_dump([(1, b'abcdef')]), max_message=3)
    assert ei.value.code == ErrorCode.VALUE_OUT_OF_RANGE
    assert 'desync' not in str(ei.value)

    # an overlong message-length varint is a real desync, and still says so
    with pytest.raises(BovnarParseError) as ei:
        stream.mux_load(b'.mux=\x00\x01\x0c\x00\x01' + b'\xff' * 11 + b'\x00;')
    assert ei.value.code == ErrorCode.OCTET_STREAM_OUT_OF_SYNC
    assert 'desync' in str(ei.value)
