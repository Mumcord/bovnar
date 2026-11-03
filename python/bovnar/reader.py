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



import ctypes
from dataclasses import dataclass, field
from typing import Callable, Generator, IO, Sequence

from ._ffi import get_library
from .enums import Event, ErrorCode
from .exceptions import BovnarParseError, BovnarArgumentError
from .structs import (
    BvnrSource, BvnrSink, BvnrReadFlags, BvnrData, ValueUnit,
    build_unit_policy, describe_unit_policy_refusal, MAX_UNIT_TARGETS,
    EVENT_CALLBACK_FUNC, WANT_UNIT_FUNC,
    make_unit_dimensionless,
)
from .unit import _as_vu

MAX_FILESIZE_BYTES = 16 * 1024 * 1024


@dataclass
class UnitRule:
    """A rule about ONE field, named by its key path.

    `path` is the dotted path a value sits at, leading dot included, exactly as
    the document writes its keys: ``".inlet.temperature"``. A path ending in
    ``".*"`` matches everything below that point at any depth. Array elements
    sit at the path of the assignment holding them, so one rule covers a whole
    array.

    Unlike the whole-document `targets` list, a rule is a statement about a
    field the caller NAMED, so a value it cannot be applied to raises
    ``UNIT_MISMATCH`` rather than being passed through — silence would defeat
    the point of naming it. That includes a bare number: ".speed is m/s" is not
    satisfied by a value with no unit.

    With ``convert=False`` the rule asserts only, and the value is delivered
    exactly as the document wrote it.
    """
    path:    str
    unit:    str
    base:    int = 0
    convert: bool = True


@dataclass
class UnitPolicy:
    """What the document must contain, and what unit values arrive in.

    The declarative form of the ``want_unit`` callback: everything is unit
    TEXT, so this is reachable from Python without a per-value trampoline.
    Install it with :meth:`Reader.set_unit_policy`.

    Per-field rules
      ``rules`` is a list of :class:`UnitRule`, matched by key path and
      consulted before everything else — the most specific thing a policy can
      say. First match wins, so order matters: put ``".a.b"`` before ``".a.*"``.

    Conversion
      ``targets`` names the units to convert to. Each entry is either ``"m/s"``
      or ``("m/s", base)``. Each numeric value takes the FIRST target it can
      validly convert to, so order is significant — ``["m", "k~m"]`` never
      selects ``k~m``. A value that matches nothing, or that is already in the
      unit a target names, is delivered untouched; ``data.converted`` is what
      tells the two apart.

      ``normalise_si`` catches whatever the targets did not, delivering it in
      coherent SI base units with prefixes folded out. Currencies and every
      DIMENSIONLESS unit (%, ppm, dB, pH, rad, °) are left as written rather
      than reinterpreted.

      A value with NO unit only ever matches a target that is itself
      ``no_unit``. A bare number is dimensionally compatible with % and ppm, so
      without that fence a policy naming ``"%"`` would deliver 0.25 as 25.

    Exactness
      Nothing approximate is ever delivered. ``leave_inexact`` hands over a
      value this conversion cannot deliver exactly — an irrational factor, a
      non-terminating expansion such as 42 km/h → 35/3 m/s, an exponent past
      the work limit — in its NATIVE unit instead of failing the parse. Without
      it, such a value raises. It applies only to policy-chosen targets; a
      ``want_unit`` hook keeps its strict all-or-nothing contract.

    Validation
      ``require_unit`` rejects a document containing any bare numeric value.
      ``require_dimension_of`` requires every numeric value to be validly
      convertible to at least one of the listed units — "this document is
      lengths, in whatever unit it wrote them". Both are evaluated on the unit
      the DOCUMENT wrote, before any conversion, and both raise
      ``ErrorCode.UNIT_MISMATCH``.
    """
    rules:                Sequence['UnitRule'] = field(default_factory=tuple)
    targets:              Sequence[str | tuple[str, int]] = field(default_factory=tuple)
    normalise_si:         bool = False
    base:                 int = 0
    leave_inexact:        bool = False
    require_unit:         bool = False
    require_dimension_of: Sequence[str] = field(default_factory=tuple)

class _CollectingHandler:


    def __init__(self) -> None:
        self._events: list[tuple[Event, bytes, object, object]] = []

    def __call__(self, ev: Event, data: BvnrData | None) -> bool:
        raw = data.raw_bytes() if data else b''
        vt  = data.value_type if data else None
        vu  = data.value_unit if data else None
        self._events.append((ev, raw, vt, vu))
        return True

class EventPayload:


    __slots__ = ('event', 'raw', 'value_type', 'value_unit',
                 'converted', 'converted_text', 'converted_base')

    def __init__(self,
                 event: Event,
                 raw: bytes,
                 value_type,
                 value_unit,
                 converted: bool = False,
                 converted_text: "str | None" = None,
                 converted_base: int = 0) -> None:
        self.event      = event
        self.raw        = raw
        self.value_type = value_type
        self.value_unit = value_unit
        # Lossless read-time unit/base conversion (see Reader want_unit). When
        # True, converted_text is the EXACT value in the requested unit and base
        # (converted_base); `raw` keeps the original text. See BvnrData.conv.
        # converted_text is None even with converted True when the result has no
        # terminating expansion in that base and the reader allowed it through
        # (want_unit_allow_nonterminating).
        self.converted      = converted
        self.converted_text = converted_text
        self.converted_base = converted_base

    @property
    def text(self) -> str:
        return self.raw.decode('utf-8', errors='replace') if self.raw else ''

    def __repr__(self) -> str:
        return f"EventPayload(event={self.event.name}, text={self.text!r})"

class Reader:


    def __init__(self) -> None:
        self._lib  = get_library()
        self._ptr  = None
        self._open()

    def _open(self) -> None:
        ptr = self._lib.bvnr_reader_create()
        if not ptr:
            raise MemoryError("bvnr_reader_create() returned NULL")
        self._ptr = ptr

    def close(self) -> None:

        if self._ptr is not None:
            self._lib.bvnr_reader_destroy(self._ptr)
            self._ptr = None

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def __del__(self):
        self.close()

    def _check_open(self) -> None:
        if self._ptr is None:
            raise BovnarArgumentError("Reader has been closed")

    def _build_flags(self,
                     on_verified,
                     on_unverified,
                     max_file_size: int,
                     continue_on_error: bool,
                     strict_version: bool = False,
                     text_only: bool = False,
                     want_unit=None,
                     want_unit_allow_nonterminating: bool = False,
                     max_conversion_length: int = 0) -> tuple:

        flags    = BvnrReadFlags()
        cb_refs  = []

        flags.max_file_size = max_file_size
        # Match the nesting limit used by the CLI, the DOM builder and stream.py
        # (the uint8_t hard cap, 255) rather than the reader's bare 64 default,
        # so loads()/read_mem accept the same documents those paths do. Leaving
        # these at 0 would default to 64 and surprise users with a rejection at
        # depth 65 for a document the CLI and dom_parse() accept.
        flags.max_struct_nesting = 255
        flags.max_array_nesting  = 255

        if on_verified is not None:
            cb = self._wrap_callback(on_verified)
            cb_refs.append(cb)
            flags.on_verified = cb

        if on_unverified is not None:
            cb = self._wrap_callback(on_unverified)
            cb_refs.append(cb)
            flags.on_unverified = cb

        if want_unit is not None:
            cb = self._wrap_want_unit(want_unit)
            cb_refs.append(cb)
            flags.want_unit = cb

        flags.continue_on_error = continue_on_error
        flags.strict_version    = strict_version
        flags.text_only         = text_only
        flags.want_unit_allow_nonterminating = want_unit_allow_nonterminating
        # 0 selects the C default (BVNR_DEFAULT_MAX_CONVERSION_LENGTH). This is a
        # work limit: rendering an exact expansion is quadratic in its digit
        # count, and the count follows the value's exponent, not its length.
        flags.max_conversion_length = max_conversion_length
        return flags, cb_refs

    @staticmethod
    def _snapshot(src: BvnrData) -> BvnrData:
        # Copy a C-owned BvnrData into a Python-owned one.
        #
        # `data_ptr.contents` is a VIEW over the reader's memory, not a copy:
        # every field read through it dereferences C storage that dies with the
        # reader, so an object a callback stashes away segfaults later. Every
        # callback must therefore hand Python a snapshot, never the view.
        #
        # The by-value fields below become genuinely Python-owned, and so do
        # the pointer fields (`data`, `frac_data`, `unit_source`, `conv.num`,
        # `conv.den`): _own_buffers() replaces each with a copy the snapshot
        # owns. They used to be carried over as raw addresses, and a BvnrData
        # kept past the callback read freed reader memory.
        unit_size = ctypes.sizeof(ValueUnit)
        snap = BvnrData()
        snap.type              = src.type
        snap.value_type.family = src.value_type.family
        snap.value_type.width  = src.value_type.width
        snap.value_type.base   = src.value_type.base
        ctypes.memmove(ctypes.addressof(snap.value_unit),
                       ctypes.addressof(src.value_unit),
                       unit_size)
        snap.length = src.length
        snap.data   = src.data
        # spec 1.1 — an ISO datetime literal's sub-second digits.
        snap.frac_data   = src.frac_data
        snap.frac_length = src.frac_length
        # library 2.0 — the unit's own spelling; reader-owned like the two above.
        snap.unit_source        = src.unit_source
        snap.unit_source_length = src.unit_source_length
        # Lossless read-time conversion result (see want_unit). The exact `text`
        # lives in reader-owned memory valid only during this call, so copy it
        # into the snapshot's own carrier now.
        snap.converted = src.converted
        if src.converted:
            snap.conv.base   = src.conv.base
            snap.conv.length = src.conv.length
            ctypes.memmove(ctypes.addressof(snap.conv.unit),
                           ctypes.addressof(src.conv.unit),
                           unit_size)
            # reading a c_char_p yields a fresh bytes copy; assigning it back
            # lets ctypes keep it alive past the callback. None when the result
            # does not terminate in the output base
            # (want_unit_allow_nonterminating) — the exact value then lives in
            # conv.num/conv.den, copied below.
            snap.conv.text = src.conv.text
            snap.conv.num  = src.conv.num
            snap.conv.den  = src.conv.den
        snap._own_buffers()
        return snap

    @staticmethod
    def _wrap_callback(py_fn: Callable):

        state = {'exc': None}
        # Each event gets a fresh BvnrData that owns everything it points at
        # (see _snapshot), so a callback may keep it -- or a sub-struct view
        # like d.value_type -- and use every accessor after the reader is gone.

        def _c_cb(userdata_void, event_int: int, data_ptr) -> bool:
            try:
                ev = Event(event_int)
                data = Reader._snapshot(data_ptr.contents) if data_ptr else None
                result = py_fn(ev, data)
                return bool(result) if result is not None else True
            except BaseException as exc:
                if state['exc'] is None:
                    state['exc'] = exc
                return False

        cb = EVENT_CALLBACK_FUNC(_c_cb)
        cb._bvnr_state = state
        return cb

    @staticmethod
    def _wrap_want_unit(py_fn: Callable):
        # Wrap a Python want_unit(data) into the C want_unit contract
        #   bool(*)(void*, const bvnr_data_t*, value_unit_t*, uint32_t*).
        # The Python callback returns one of:
        #   None                       -> decline (value delivered untouched)
        #   ValueUnit                  -> convert to that unit, keep native base
        #   (ValueUnit, base:int)      -> convert to that unit and output base
        #                                 (0 keeps native; otherwise any base bvnr
        #                                  writes: 2..62, 64 or 85)
        # The conversion is lossless. An incompatible unit becomes
        # error_unit_mismatch, an irrational factor error_unit_inexact, an
        # unusable base error_invalid_argument — each aborts the parse. So does a
        # result that does not terminate in the output base, unless the reader was
        # given want_unit_allow_nonterminating=True.
        state = {'exc': None}
        unit_size = ctypes.sizeof(ValueUnit)

        def _c_cb(userdata_void, data_ptr, want_ptr, want_base_ptr) -> bool:
            try:
                # Snapshot, never the raw view: a callback that stashes the
                # object away would otherwise be holding freed reader memory.
                data = Reader._snapshot(data_ptr.contents) if data_ptr else None
                res = py_fn(data)
                if res is None:
                    return False
                if isinstance(res, tuple):
                    want, base = res
                else:
                    want, base = res, 0
                # A Unit is a unit: Quantity.unit and Unit.parse() hand those
                # out, and refusing one here sent callers hunting for `.raw`.
                want = _as_vu(want)
                if not isinstance(want, ValueUnit):
                    raise BovnarArgumentError(
                        "want_unit callback must return a ValueUnit or Unit, "
                        "(unit, base), or None")
                ctypes.memmove(ctypes.addressof(want_ptr.contents),
                               ctypes.addressof(want), unit_size)
                if want_base_ptr:
                    want_base_ptr.contents.value = int(base)
                return True
            except BaseException as exc:
                if state['exc'] is None:
                    state['exc'] = exc
                return False

        cb = WANT_UNIT_FUNC(_c_cb)
        cb._bvnr_state = state
        return cb

    def _raise_error(self) -> None:

        code   = ErrorCode(self._lib.bvnr_reader_get_error(self._ptr))
        line   = self._lib.bvnr_reader_get_error_line(self._ptr)
        column = self._lib.bvnr_reader_get_error_column(self._ptr)
        offset = self._lib.bvnr_reader_get_error_offset(self._ptr)
        byte   = self._lib.bvnr_reader_get_error_byte(self._ptr)
        msg_b  = self._lib.bvn_error_to_string(int(code))
        msg    = msg_b.decode('utf-8') if msg_b else ''
        raise BovnarParseError(code, line, column, offset, byte, msg)

    def read_mem(self,
                 data: bytes | bytearray | memoryview,
                 *,
                 on_verified: Callable | None = None,
                 on_unverified: Callable | None = None,
                 max_file_size: int = 0,
                 continue_on_error: bool = False,
                 strict_version: bool = False,
                 text_only: bool = False,
                 want_unit: Callable | None = None,
                 want_unit_allow_nonterminating: bool = False,
                 max_conversion_length: int = 0) -> None:

        self._check_open()
        if isinstance(data, memoryview):
            data = bytes(data)
        if not isinstance(data, (bytes, bytearray)):
            raise BovnarArgumentError("data must be bytes, bytearray, or memoryview")

        flags, cb_refs = self._build_flags(
            on_verified, on_unverified, max_file_size, continue_on_error,
            strict_version, text_only, want_unit,
            want_unit_allow_nonterminating, max_conversion_length
        )

        buf = (ctypes.c_char * len(data)).from_buffer_copy(data)
        ok  = self._lib.bvnr_open_read_mem(
            self._ptr,
            buf, len(data),
            None, 0,
            ctypes.byref(flags),
        )
        if not ok:
            self._raise_error()

        ok = self._lib.bvnr_read(self._ptr)

        cb_exc = None
        for cb in cb_refs:
            st = getattr(cb, '_bvnr_state', None)
            if st is not None and st.get('exc') is not None:
                cb_exc = st['exc']
                break

        del cb_refs
        if cb_exc is not None:
            raise cb_exc
        if not ok:
            self._raise_error()

    def read_fd(self,
                fd: int,
                *,
                on_verified: Callable | None = None,
                on_unverified: Callable | None = None,
                max_file_size: int = 0,
                continue_on_error: bool = False,
                strict_version: bool = False,
                text_only: bool = False,
                want_unit: Callable | None = None,
                want_unit_allow_nonterminating: bool = False,
                max_conversion_length: int = 0) -> None:

        self._check_open()
        if not isinstance(fd, int) or fd < 0:
            raise BovnarArgumentError(f"Invalid file descriptor: {fd!r}")

        flags, cb_refs = self._build_flags(
            on_verified, on_unverified, max_file_size, continue_on_error,
            strict_version, text_only, want_unit,
            want_unit_allow_nonterminating, max_conversion_length
        )

        src = BvnrSource()
        self._lib.bvnr_source_from_fd(ctypes.byref(src), fd)

        ok = self._lib.bvnr_open_read_source(
            self._ptr,
            ctypes.byref(src),
            None,
            ctypes.byref(flags),
        )
        if not ok:
            self._raise_error()

        ok = self._lib.bvnr_read(self._ptr)

        cb_exc = None
        for cb in cb_refs:
            st = getattr(cb, '_bvnr_state', None)
            if st is not None and st.get('exc') is not None:
                cb_exc = st['exc']
                break

        del cb_refs
        if cb_exc is not None:
            raise cb_exc
        if not ok:
            self._raise_error()

    def read_file(self,
                  path: str,
                  *,
                  on_verified: Callable | None = None,
                  on_unverified: Callable | None = None,
                  max_file_size: int = MAX_FILESIZE_BYTES,
                  continue_on_error: bool = False,
                  strict_version: bool = False,
                  text_only: bool = False,
                  want_unit: Callable | None = None,
                  want_unit_allow_nonterminating: bool = False,
                  max_conversion_length: int = 0) -> None:
        # Forwards every read_fd option; dropping any of them here would make a
        # conversion or strict-version read impossible from a path.
        import os
        fd = os.open(path, os.O_RDONLY)
        try:
            self.read_fd(
                fd,
                on_verified=on_verified,
                on_unverified=on_unverified,
                max_file_size=max_file_size,
                continue_on_error=continue_on_error,
                strict_version=strict_version,
                text_only=text_only,
                want_unit=want_unit,
                want_unit_allow_nonterminating=want_unit_allow_nonterminating,
                max_conversion_length=max_conversion_length,
            )
        finally:
            os.close(fd)

    def iter_mem(self,
                 data: bytes | bytearray | memoryview,
                 *,
                 verified_only: bool = True,
                 max_file_size: int = 0) -> Generator[EventPayload, None, None]:

        events: list[EventPayload] = []

        def collector(ev: Event, d):
            raw = d.raw_bytes() if d else b''
            vt  = d.value_type  if d else None
            vu  = d.value_unit  if d else None
            cvd = bool(d.converted) if d else False
            ctxt = d.converted_str() if (d and cvd) else None
            cbase = int(d.conv.base) if (d and cvd) else 0
            events.append(EventPayload(ev, raw, vt, vu, cvd, ctxt, cbase))
            return True

        kwargs = dict(max_file_size=max_file_size)
        if verified_only:
            self.read_mem(data, on_verified=collector, **kwargs)
        else:
            self.read_mem(data,
                          on_verified=collector,
                          on_unverified=collector,
                          **kwargs)

        yield from events

    @property
    def error_code(self) -> ErrorCode:
        self._check_open()
        return ErrorCode(self._lib.bvnr_reader_get_error(self._ptr))

    @property
    def error_line(self) -> int:
        self._check_open()
        return self._lib.bvnr_reader_get_error_line(self._ptr)

    @property
    def error_column(self) -> int:
        self._check_open()
        return self._lib.bvnr_reader_get_error_column(self._ptr)

    @property
    def error_offset(self) -> int:
        self._check_open()
        return self._lib.bvnr_reader_get_error_offset(self._ptr)

    @property
    def recovery_count(self) -> int:
        self._check_open()
        return self._lib.bvnr_reader_get_recovery_count(self._ptr)

    def set_unit_policy(self, policy: UnitPolicy | None) -> None:
        """Install (or, with ``None``, clear) this reader's unit policy.

        Every unit string is parsed here, so a typo raises before a byte of the
        document is read; a rejected policy leaves the previous one in force.
        The policy may be set before or after a read and survives re-reading
        the same reader on another document — it describes the consumer, not
        the document.

            r = Reader()
            r.set_unit_policy(UnitPolicy(targets=["m/s", "°C"],
                                         normalise_si=True,
                                         leave_inexact=True))
            r.read_file("sensors.bvnr", on_verified=handler)
        """
        self._check_open()
        if policy is None:
            if not self._lib.bvnr_reader_set_unit_policy(self._ptr, None):
                raise BovnarArgumentError("bvnr_reader_set_unit_policy failed")
            return
        try:
            cp, keepalive = build_unit_policy(
                policy.targets, policy.base, policy.normalise_si,
                policy.leave_inexact, policy.require_unit,
                policy.require_dimension_of, policy.rules)
        except ValueError as e:
            raise BovnarArgumentError(str(e)) from None
        # `keepalive` holds the encoded unit strings the struct points at; it
        # must stay referenced until the call returns, which it does by being a
        # live local here.
        if not self._lib.bvnr_reader_set_unit_policy(self._ptr, ctypes.byref(cp)):
            raise BovnarArgumentError(describe_unit_policy_refusal(
                self._lib, policy.targets, policy.base, policy.normalise_si,
                policy.leave_inexact, policy.require_unit,
                policy.require_dimension_of, policy.rules))
        del keepalive

    @property
    def skipped_bytes(self) -> int:
        """Bytes the reader consumed and DISCARDED while recovering.

        ``recovery_count`` says how often the parser had to recover; this says
        what it cost, and it is the only way to learn that: the skipped bytes
        were never parsed, so no callback ever mentions them. A non-zero value
        means the document your callbacks saw is not the whole document.
        """
        self._check_open()
        return self._lib.bvnr_reader_get_skipped_bytes(self._ptr)

    @property
    def declared_version(self):
        """The (major, minor) spec version declared by a leading
        ``#!bovnar M.N`` directive, or ``None`` if the document carried none.
        Valid after a read."""
        import ctypes
        self._check_open()
        maj, mn = ctypes.c_uint16(0), ctypes.c_uint16(0)
        if self._lib.bvnr_reader_get_declared_version(
                self._ptr, ctypes.byref(maj), ctypes.byref(mn)):
            return (maj.value, mn.value)
        return None


