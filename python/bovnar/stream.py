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

"""
Python bindings for the bovnar streaming/framing layer (bovnar_stream.h):

  * Endless streaming   – BVNR_FILESIZE_UNLIMITED (a max_file_size sentinel).
  * Multi-document      – frame_documents / dump_documents / load_documents.
  * Octet multiplexing  – mux_dump / mux_load (cross-chunk reassembly in C).
  * Document-in-document – embed_document / parse_embedded.

These wrap the C functions directly, so behaviour (framing, varint reassembly,
the memory-amplification guards, the strict-vs-resilient document policy) matches
the native library exactly. See doc/10_bovnar_streaming.md for the conventions.
"""

import ctypes
from typing import Iterable

from ._ffi import get_library
from .enums import ErrorCode
from .exceptions import BovnarParseError, BovnarWriteError, BovnarArgumentError
from .structs import (
    BvnrSink, BvnrSource, BvnrReadFlags, BvnrDocStreamOpts,
    EVENT_CALLBACK_FUNC, ON_DOCUMENT_FUNC, MUX_ON_MSG_FUNC, ON_ERROR_FUNC,
)
from .reader import Reader
from .writer import Writer

# Mirrors of the C constants (bovnar.h / bovnar_stream.h).
# 0 == no total-byte cap (the default for a zero-initialised max_file_size);
# the text is still bounded by max_text_bytes.
BVNR_FILESIZE_UNLIMITED = 0
BVNR_DOC_DEFAULT_MAX    = 256 * 1024 * 1024
BVNR_MUX_DEFAULT_MAX    = 64 * 1024 * 1024
BVNR_MUX_MAX_MESSAGE    = 65536 - 20

__all__ = [
    'BVNR_FILESIZE_UNLIMITED', 'BVNR_DOC_DEFAULT_MAX', 'BVNR_MUX_DEFAULT_MAX',
    'BVNR_MUX_MAX_MESSAGE',
    'frame_documents', 'dump_documents', 'load_documents',
    'mux_dump', 'mux_load',
    'embed_document', 'parse_embedded',
]


def _as_bytes(x) -> bytes:
    if isinstance(x, str):
        return x.encode('utf-8')
    return bytes(x)


def _void_copy(data: bytes):
    """Return (c_void_p, keepalive) for `data`, or (None, None) when empty."""
    if not data:
        return None, None
    buf = (ctypes.c_char * len(data)).from_buffer_copy(data)
    return ctypes.cast(buf, ctypes.c_void_p), buf


# ===========================================================================
# Multi-document record framing
# ===========================================================================

def frame_documents(docs: Iterable[bytes]) -> bytes:
    """Wrap each already-serialised document in a length-prefixed frame and
    return the concatenated frame stream (``"BVF1" + u64 len + payload`` each)."""
    lib   = get_library()
    blobs = [_as_bytes(d) for d in docs]
    total = sum(12 + len(b) for b in blobs)
    out   = (ctypes.c_char * (total if total else 1))()
    sink  = BvnrSink()
    lib.bvnr_sink_to_mem(ctypes.byref(sink), ctypes.cast(out, ctypes.c_void_p),
                         total)
    for b in blobs:
        dptr, _keep = _void_copy(b)
        if not lib.bvnr_frame_write(ctypes.byref(sink), dptr, len(b)):
            raise BovnarWriteError(ErrorCode.NONE, message="bvnr_frame_write failed")
    return bytes(out[:total])


def dump_documents(objs: Iterable[dict], *, pretty: bool = True) -> bytes:
    """Serialise each dict (via :func:`bovnar.dumps`) and frame the results."""
    from . import dumps  # deferred: avoids an import cycle with the package root
    return frame_documents(dumps(o, pretty=pretty) for o in objs)


def load_documents(data,
                   *,
                   typed: bool = False,
                   continue_past_failed: bool = False,
                   max_document_size: int = 0) -> list:
    """Parse a frame stream into a list of documents (one dict per frame).

    With ``continue_past_failed=False`` (default) the first malformed document
    raises :class:`BovnarParseError`. With ``continue_past_failed=True`` every
    frame is processed and a failed document is represented by ``None`` in the
    returned list. ``typed=True`` wraps scalars in :class:`bovnar.Quantity`.
    """
    from . import _DictParser, _TypedDictParser  # deferred (import cycle)
    lib  = get_library()
    data = _as_bytes(data)

    def _new_parser():
        return _TypedDictParser() if typed else _DictParser()

    results: list = []
    holder = {'parser': _new_parser()}
    fail   = {'index': None, 'err': None}
    state  = {'exc': None}

    def on_event(ev, d):
        return holder['parser'].on_event(ev, d)

    def on_document(index, ok, err):
        results.append(holder['parser'].result() if ok else None)
        if not ok and fail['index'] is None:
            fail['index'], fail['err'] = index, err
        holder['parser'] = _new_parser()
        return True

    cb_event = Reader._wrap_callback(on_event)

    def _c_on_doc(_ud, index, ok, err):
        try:
            return bool(on_document(int(index), bool(ok), int(err)))
        except BaseException as exc:               # noqa: BLE001 (propagated below)
            if state['exc'] is None:
                state['exc'] = exc
            return False
    cb_doc = ON_DOCUMENT_FUNC(_c_on_doc)

    flags = BvnrReadFlags()
    flags.max_file_size      = BVNR_FILESIZE_UNLIMITED
    flags.max_array_nesting  = 255
    flags.max_struct_nesting = 255
    flags.on_verified        = cb_event
    flags.continue_on_error  = False               # strict per-document parsing

    opts = BvnrDocStreamOpts()
    opts.flags                = ctypes.pointer(flags)
    opts.on_document          = cb_doc
    opts.continue_past_failed = continue_past_failed
    opts.max_document_size    = max_document_size

    src = BvnrSource()
    dptr, _keep = _void_copy(data)
    lib.bvnr_source_from_mem(ctypes.byref(src), dptr, len(data))

    out_count = ctypes.c_uint64(0)
    ok = lib.bvnr_doc_stream_read(ctypes.byref(src), ctypes.byref(opts),
                                  ctypes.byref(out_count))

    cb_state = getattr(cb_event, '_bvnr_state', None)
    if cb_state is not None and cb_state.get('exc') is not None:
        raise cb_state['exc']
    if state['exc'] is not None:
        raise state['exc']
    if not ok:
        if fail['err'] is not None:
            raise BovnarParseError(
                ErrorCode(fail['err']),
                message=f"document {fail['index']} failed to parse")
        raise BovnarParseError(
            ErrorCode.GOT_INCOMPLETE_BVNR_STREAM,
            message="frame stream truncated or malformed")
    return results


# ===========================================================================
# Octet multiplexing
# ===========================================================================

def mux_dump(messages: Iterable[tuple], *, key: str = 'mux') -> bytes:
    """Serialise ``(channel:int, payload:bytes)`` messages as one multiplexed
    octet-stream document under ``key``. Channels may repeat and interleave;
    messages of any size are split across octet chunks and reassembled by
    :func:`mux_load`."""
    lib  = get_library()
    msgs = [(int(ch), _as_bytes(p)) for ch, p in messages]
    # The channel crosses the FFI as a uint64_t, and ctypes wraps an
    # out-of-range int without a word: channel -1 was written as 2**64-1, and
    # the reader then reported a channel the producer never used.
    for ch, _ in msgs:
        if not 0 <= ch < (1 << 64):
            raise BovnarArgumentError(
                f"mux channel {ch} is outside the unsigned 64-bit range")
    total = sum(len(p) for _, p in msgs)
    kb    = key.encode('utf-8')
    # Output is the payload plus octet-chunk framing (a few bytes per 64 KiB
    # chunk); start with a tight over-estimate and grow on exhaustion so large
    # or many-chunk messages cannot overflow a fixed guess.
    cap     = total + total // 4000 + len(msgs) * 64 + len(kb) + 8192
    cap_max = max(256 * 1024 * 1024, total * 2 + 1024 * 1024)

    while True:
        w = Writer.to_mem(cap=cap, pretty=False)
        try:
            if not lib.bvnr_mux_begin(w._ptr, kb):
                w._raise_error()
            for ch, payload in msgs:
                dptr, _keep = _void_copy(payload)
                if not lib.bvnr_mux_send(w._ptr, ch, dptr, len(payload)):
                    w._raise_error()
            if not lib.bvnr_mux_end(w._ptr):
                w._raise_error()
            w.finish()
            return w.get_output()
        except BovnarWriteError as exc:
            if exc.code == ErrorCode.SINK_BUFFER_EXHAUSTED and cap < cap_max:
                cap = min(cap * 2, cap_max)
                continue
            raise
        finally:
            w.destroy()


def mux_load(data, *, max_message: int = 0, key: str | None = None) -> list:
    """Demultiplex an octet-stream document, returning a list of
    ``(channel:int, payload:bytes)`` in arrival order.

    By default every octet stream in the document is treated as multiplexed.
    Pass ``key`` to scope the demux to streams opened under that key only, so a
    document mixing a mux stream with ordinary binary octet payloads ignores the
    latter (mirrors ``bvnr_demux_set_key``)."""
    lib  = get_library()
    data = _as_bytes(data)

    results: list = []
    state = {'exc': None}

    def _on_msg(_ud, channel, data_ptr, length):
        try:
            payload = ctypes.string_at(data_ptr, length) if (data_ptr and length) else b''
            results.append((int(channel), payload))
            return True
        except BaseException as exc:               # noqa: BLE001 (propagated below)
            if state['exc'] is None:
                state['exc'] = exc
            return False
    cb_msg = MUX_ON_MSG_FUNC(_on_msg)

    dm = lib.bvnr_demux_create(cb_msg, None, max_message)
    if not dm:
        raise MemoryError("bvnr_demux_create() returned NULL")
    if key is not None and not lib.bvnr_demux_set_key(dm, key.encode('utf-8')):
        lib.bvnr_demux_destroy(dm)
        raise MemoryError("bvnr_demux_set_key() failed")
    rptr = lib.bvnr_reader_create()
    if not rptr:
        lib.bvnr_demux_destroy(dm)
        raise MemoryError("bvnr_reader_create() returned NULL")
    try:
        # Route every event straight into the C demultiplexer; only the
        # per-message callback above is Python.
        def _trampoline(_ud, ev, dptr):
            return lib.bvnr_demux_on_event(dm, ev, dptr)
        cb_ev = EVENT_CALLBACK_FUNC(_trampoline)

        flags = BvnrReadFlags()
        flags.max_file_size      = BVNR_FILESIZE_UNLIMITED
        flags.max_array_nesting  = 255
        flags.max_struct_nesting = 255
        flags.on_verified        = cb_ev

        dptr, _keep = _void_copy(data)
        ok = lib.bvnr_open_read_mem(rptr, dptr, len(data), None, 0,
                                    ctypes.byref(flags)) and lib.bvnr_read(rptr)

        if state['exc'] is not None:
            raise state['exc']
        derr = lib.bvnr_demux_error(dm)
        if derr != int(ErrorCode.NONE):
            raise BovnarParseError(ErrorCode(derr), message="demux desync")
        if not ok:
            raise BovnarParseError(
                ErrorCode(lib.bvnr_reader_get_error(rptr)),
                message="multiplexed stream parse error")
        return results
    finally:
        lib.bvnr_reader_destroy(rptr)
        lib.bvnr_demux_destroy(dm)


# ===========================================================================
# Document-in-document
# ===========================================================================

def embed_document(inner, *, key: str = 'payload', pretty: bool = True) -> bytes:
    """Return a complete document that embeds ``inner`` (raw document bytes) as
    the octet-stream payload of ``key``."""
    lib   = get_library()
    inner = _as_bytes(inner)
    kb    = key.encode('utf-8')
    # As in mux_dump: payload + per-chunk octet framing; grow on exhaustion.
    cap     = len(inner) + len(inner) // 4000 + len(kb) + 8192
    cap_max = max(256 * 1024 * 1024, len(inner) * 2 + 1024 * 1024)

    while True:
        w = Writer.to_mem(cap=cap, pretty=pretty)
        try:
            dptr, _keep = _void_copy(inner)
            if not lib.bvnr_embed_document(w._ptr, kb, dptr, len(inner)):
                w._raise_error()
            w.finish()
            return w.get_output()
        except BovnarWriteError as exc:
            if exc.code == ErrorCode.SINK_BUFFER_EXHAUSTED and cap < cap_max:
                cap = min(cap * 2, cap_max)
                continue
            raise
        finally:
            w.destroy()


def parse_embedded(inner, *, typed: bool = False) -> dict:
    """Parse an embedded document's bytes (as extracted from an octet stream)
    into a dict — a thin wrapper over the C ``bvnr_parse_embedded``."""
    from . import _DictParser, _TypedDictParser  # deferred (import cycle)
    lib    = get_library()
    inner  = _as_bytes(inner)
    parser = _TypedDictParser() if typed else _DictParser()

    cb = Reader._wrap_callback(parser.on_event)

    # bvnr_parse_embedded owns its reader and answers one bool, so the error it
    # hit is gone by the time it returns -- and every failure used to be
    # reported as GOT_INCOMPLETE_BVNR_STREAM, an unexpected byte included. The
    # reader does tell its on_error hook what went wrong and where; that is the
    # only place the code survives, so it is caught there. The first report is
    # the one that stopped the parse.
    seen: list = []

    def _on_error(_ud, err, line, column, byte, offset):
        if not seen:
            seen.append((int(err), int(line), int(column), int(offset),
                         int(byte)))
    cb_err = ON_ERROR_FUNC(_on_error)

    flags = BvnrReadFlags()
    flags.max_file_size      = BVNR_FILESIZE_UNLIMITED
    flags.max_array_nesting  = 255
    flags.max_struct_nesting = 255
    flags.on_verified        = cb
    flags.on_error           = cb_err

    dptr, _keep = _void_copy(inner)
    ok = lib.bvnr_parse_embedded(dptr, len(inner), ctypes.byref(flags))

    cb_state = getattr(cb, '_bvnr_state', None)
    if cb_state is not None and cb_state.get('exc') is not None:
        raise cb_state['exc']
    if not ok:
        if seen:
            code, line, column, offset, byte = seen[0]
            msg_b = lib.bvn_error_to_string(code)
            raise BovnarParseError(
                ErrorCode(code), line, column, offset, byte,
                "embedded document: " + (msg_b.decode('utf-8') if msg_b else ''))
        raise BovnarParseError(ErrorCode.GOT_INCOMPLETE_BVNR_STREAM,
                               message="embedded document parse error")
    return parser.result()
