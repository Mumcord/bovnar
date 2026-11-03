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
from types import SimpleNamespace
from .enums import (
    Event, ValueTypeFamily, PrefixSystem, SIPrefix, IECPrefix,
    BaseUnit, Exponent, ErrorCode,
)

# Reserved size for the opaque pass-through structs (BvnrSource / BvnrSink).
# These are black boxes the C side owns entirely: Python only allocates them and
# hands &struct to C (bvnr_source_from_fd, bvnr_open_read_source, ...). C uses
# BVNR_SOURCE_RESERVED_SIZE / BVNR_SINK_RESERVED_SIZE bytes (64 each as of this
# writing). We deliberately over-allocate (256) as a forward-compatibility
# margin: a binding may be LARGER than the C struct (C writes within its own
# bounds) but must never be SMALLER. test_abi.py enforces `python_size >= c_size`
# against the live C layout, so if the C reserve ever grows past 256 the build
# fails loudly here instead of under-allocating and corrupting memory.
OPAQUE_BYTES = 256
# Use 64-bit words so the ctypes struct's alignment matches the C
# struct's (pointer-aligned).  c_uint8 arrays have alignment 1.
_OPAQUE_WORDS = OPAQUE_BYTES // 8

# Must match BVNR_MAX_UNIT_COMPONENTS in include/bovnar.h -- value_unit_t
# embeds the array inline, so a mismatch silently misreads every struct that
# contains a unit. tests/test_abi.py compares the two sizes on every run.
MAX_UNIT_COMPONENTS = 32

# Must match BVNR_UNIT_STRING_MAX in include/bovnar.h: the longest unit string
# the library will produce, and therefore the smallest buffer that can always
# receive one.
#
# Every unit formatter here used to allocate 256 bytes, which is not a bound on
# anything -- the C side allows 1088, and a legal 32-component unit reaches 597:
#
#     "·".join(["da~ton_ref^-100", "da~cal_IT^100",
#               "da~Btu_th^-100",  "da~fath^100"] * 8)
#
# C formats that; Python raised "output buffer overflow" on a unit its own
# library had just written. A binding narrower than the library it binds is a
# defect a caller cannot work around, so the size comes from the header's
# constant rather than from a round number.
UNIT_STRING_MAX = 1088

ON_ERROR_FUNC = ctypes.CFUNCTYPE(
    None,
    ctypes.c_void_p,
    ctypes.c_int,
    ctypes.c_uint64,
    ctypes.c_uint64,
    ctypes.c_uint32,
    ctypes.c_uint64,
)

class _PrefixId(ctypes.Union):
    _fields_ = [
        ('si',  ctypes.c_int),
        ('iec', ctypes.c_int),
    ]

class ValueUnitPrefix(ctypes.Structure):
    _fields_ = [
        ('system', ctypes.c_int),
        ('id',     _PrefixId),
    ]

    @classmethod
    def make_si(cls, prefix: SIPrefix) -> 'ValueUnitPrefix':
        p = cls()
        p.system = int(PrefixSystem.SI)
        p.id.si  = int(prefix)
        return p

    @classmethod
    def make_iec(cls, prefix: IECPrefix) -> 'ValueUnitPrefix':
        p = cls()
        p.system = int(PrefixSystem.IEC)
        p.id.iec = int(prefix)
        return p

    @property
    def prefix_system(self) -> PrefixSystem:
        return PrefixSystem(self.system)

    @property
    def si_prefix(self) -> SIPrefix:
        return SIPrefix(self.id.si)

    @property
    def iec_prefix(self) -> IECPrefix:
        return IECPrefix(self.id.iec)

    def __repr__(self) -> str:
        if self.prefix_system == PrefixSystem.SI:
            return f"ValueUnitPrefix(si={self.si_prefix.name})"
        return f"ValueUnitPrefix(iec={self.iec_prefix.name})"

    def _key(self):
        # id is a union, so id.si covers the IEC member's storage too.
        #
        # WITHOUT A PREFIX THE SYSTEM IS NOT PART OF THE IDENTITY. "no SI
        # prefix" and "no IEC prefix" are the same statement, and
        # bvn_unit_equal treats them so: parse_unit("B") is SI/none,
        # make_unit_iec(BaseUnit.BYTE) is IEC/none, and the two compare equal.
        # This key fed ValueUnit.__hash__ with the system in it, so one unit
        # hashed two ways and `{Unit(a): 1}[Unit(b)]` was a KeyError for a key
        # the dict held -- the contract ValueUnit.__hash__ exists to keep.
        pid = int(self.id.si)
        return (int(self.system), pid) if pid else (0, 0)

    def __eq__(self, other) -> bool:
        if not isinstance(other, ValueUnitPrefix):
            return NotImplemented
        return self._key() == other._key()

    def __hash__(self) -> int:
        return hash(self._key())


class ValueUnitComponent(ctypes.Structure):
    _fields_ = [
        ('base',     ctypes.c_int),
        ('exponent', ctypes.c_int),
        ('prefix',   ValueUnitPrefix),
    ]

    @property
    def base_unit(self) -> BaseUnit:
        return BaseUnit(self.base)

    @property
    def exp(self) -> Exponent:
        return Exponent(self.exponent)

    @property
    def prefix_system(self) -> PrefixSystem:
        return PrefixSystem(self.prefix.system)

    @property
    def si_prefix(self) -> SIPrefix:
        return SIPrefix(self.prefix.id.si)

    @property
    def iec_prefix(self) -> IECPrefix:
        return IECPrefix(self.prefix.id.iec)

    def __repr__(self) -> str:
        # A repr must not raise, and this one did: BaseUnit has no member for a
        # profile's opaque atoms (ucum:[IU] is base 200000, see doc/09 §18), so
        # going through the enum made print(parse_unit("ucum:[IU]")) a
        # ValueError -- in a debugger, a log line, or pytest's own failure
        # report. Anything the enums cannot name is shown as its number.
        def _name(enum, value):
            try:
                return enum(value).name
            except ValueError:
                return str(int(value))

        if int(self.prefix.system) == int(PrefixSystem.IEC):
            pfx = f"iec={_name(IECPrefix, self.prefix.id.iec)}"
        else:
            pfx = f"si={_name(SIPrefix, self.prefix.id.si)}"
        return (f"ValueUnitComponent(base={_name(BaseUnit, self.base)}, "
                f"exp={_name(Exponent, self.exponent)}, {pfx})")

    def _key(self):
        return (int(self.base), int(self.exponent), self.prefix._key())

    def __eq__(self, other) -> bool:
        """By value, so a component survives a round trip through a set.

        `ValueUnit.__eq__` asks the library; this is the piecewise question a
        caller iterating `active_components()` would otherwise have to spell
        out field by field.
        """
        if not isinstance(other, ValueUnitComponent):
            return NotImplemented
        return self._key() == other._key()

    def __hash__(self) -> int:
        return hash(self._key())

class ValueUnit(ctypes.Structure):
    _fields_ = [
        ('num_components', ctypes.c_uint32),
        ('components',     ValueUnitComponent * MAX_UNIT_COMPONENTS),
    ]

    def active_components(self) -> list:
        return list(self.components[:self.num_components])

    @property
    def is_dimensionless(self) -> bool:
        n = self.num_components
        if n == 0:
            return True
        if n == 1 and self.components[0].base == BaseUnit.NONE:
            return True
        return False

    def __repr__(self) -> str:
        return (f"ValueUnit(n={self.num_components}, "
                f"components={self.active_components()!r})")

    def __eq__(self, other) -> bool:
        """Unit equality, as `bvn_unit_equal` defines it.

        A ctypes Structure defines no `__eq__`, so `==` fell back to identity
        and `parse_unit("m") == parse_unit("m")` was **False** — the natural
        Python spelling of the question silently gave the wrong answer, with no
        `units_equal` exported to reach for instead. The C API has had
        `bvn_unit_equal` throughout; only the binding was missing.

        `Quantity.__eq__` never had the bug because it compares `unit_str`, and
        the Python suite compares formatted spellings everywhere, so nothing in
        the repo exercised this.
        """
        if not isinstance(other, ValueUnit):
            return NotImplemented
        from ._ffi import get_library
        return bool(get_library().bvn_unit_equal(self, other))

    def __hash__(self) -> int:
        """Consistent with `__eq__`, and with `Unit.__hash__`.

        The same multiset of (base, exponent, prefix) triples that `Unit` uses,
        deliberately and not coincidentally: `Unit.__eq__` accepts a bare
        `ValueUnit` and reports True for an equal one, so Python's contract
        requires the two to hash alike. Hashing this struct any other way would
        make `{Unit.parse("m"): 1}[parse_unit("m")]` a KeyError for a key the
        dict considers present.

        Order-independent, because `bvn_unit_equal` is.
        """
        return hash(frozenset(c._key()
                              for c in self.components[:self.num_components]))

class ValueTypeSpec(ctypes.Structure):
    _fields_ = [
        ('family', ctypes.c_int),
        ('width',  ctypes.c_uint32),
        ('base',   ctypes.c_uint32),
    ]

    @property
    def type_family(self) -> ValueTypeFamily:
        return ValueTypeFamily(self.family)

    def __repr__(self) -> str:
        f = self.type_family
        return f"ValueTypeSpec(family={f.name}, width={self.width}, base={self.base})"

    def _key(self):
        return (int(self.family), int(self.width), int(self.base))

    def __eq__(self, other) -> bool:
        """By value — the three fields, which is all this struct is.

        Same reason as `ValueUnit`: a ctypes Structure has no `__eq__`, so
        `make_type_spec(FLOAT, 64, 0) == make_type_spec(FLOAT, 64, 0)` was
        False, and `node.value_type == other.value_type` — the obvious way to
        ask whether two values share a type — silently answered "no".
        `Quantity.__eq__` compares `family`, `width` and `base` one at a time,
        which is the workaround this removes the need for.
        """
        if not isinstance(other, ValueTypeSpec):
            return NotImplemented
        return self._key() == other._key()

    def __hash__(self) -> int:
        return hash(self._key())

class BvnrConverted(ctypes.Structure):
    # Mirrors C `bvnr_converted_t`: the exact result of a lossless read-time
    # unit/base conversion. `text` is the value's full positional expansion in
    # `base` (reader-owned, callback lifetime); num/den are opaque bvn_int_t*.
    # `text` is NULL when the exact result has no terminating expansion in `base`
    # and the reader was opened with want_unit_allow_nonterminating — num/den are
    # still exact there. See BvnrData.converted_str().
    _fields_ = [
        ('unit',   ValueUnit),
        ('text',   ctypes.c_char_p),
        ('length', ctypes.c_uint32),
        ('base',   ctypes.c_uint32),
        ('num',    ctypes.c_void_p),
        ('den',    ctypes.c_void_p),
    ]

class _OwnedBigInt:
    """A heap ``bvn_int_t`` this process owns: a copy of a reader-owned bignum
    (``conv.num``/``conv.den``), freed when the last reference goes. See
    ``BvnrData._own_buffers``."""

    __slots__ = ('ptr', '_free')

    def __init__(self, src: int) -> None:
        from ._ffi import get_library
        lib = get_library()
        self._free = lib.bvn_int_free
        self.ptr = lib.bvn_int_alloc()
        if not self.ptr:
            raise MemoryError("bvn_int_alloc() returned NULL")
        if not lib.bvn_int_copy(self.ptr, src):
            lib.bvn_int_free(self.ptr)
            self.ptr = None
            raise MemoryError("bvn_int_copy() failed on a conversion result")

    def __del__(self) -> None:
        if self.ptr:
            self._free(self.ptr)
            self.ptr = None


class BvnrData(ctypes.Structure):
    # Must mirror the C `bvnr_data_t` layout exactly: the writer path allocates
    # this and passes &d to bvnr_write_event(), and the reader path receives a
    # C-allocated pointer. The spec-1.1 frac_* fields and the conversion carrier
    # are appended last, matching the C struct — omitting them makes the struct
    # short, so C would read out of bounds.
    _fields_ = [
        ('type',        ctypes.c_int),
        ('value_type',  ValueTypeSpec),
        ('value_unit',  ValueUnit),
        ('data',        ctypes.c_void_p),
        ('length',      ctypes.c_uint32),
        ('frac_data',   ctypes.c_void_p),    # spec 1.1 — ISO datetime sub-second digits, else NULL
        ('frac_length', ctypes.c_uint32),    # spec 1.1 — length of frac_data, else 0
        ('unit_source', ctypes.c_void_p),    # library 2.0 — the unit as the producer spelled it, else NULL
        ('unit_source_length', ctypes.c_uint32),  # library 2.0 — length of unit_source, else 0
        ('converted',   ctypes.c_bool),      # lossless read-time conversion applied?
        ('conv',        BvnrConverted),      # exact converted value when converted
    ]

    def frac_str(self) -> "str | None":
        """The verbatim ISO sub-second digits (spec 1.1), or None when absent."""
        if not self.frac_data or self.frac_length == 0:
            return None
        return (ctypes.c_char * self.frac_length).from_address(
            self.frac_data).raw.decode('ascii')

    def unit_source_str(self) -> "str | None":
        """The unit exactly as the document spelled it (library 2.0), or None when
        the value carries no unit.

        ``value_unit`` is what the unit means; this is what was written. They
        differ where formatting a unit is lossy: ``ucum:mL{total}`` parses to
        ``m~L`` and only this keeps the annotation. Like ``raw_bytes()``, it
        reads a copy the callback's ``BvnrData`` owns."""
        if not self.unit_source or self.unit_source_length == 0:
            return None
        return (ctypes.c_char * self.unit_source_length).from_address(
            self.unit_source).raw.decode('utf-8', errors='replace')

    def converted_str(self) -> "str | None":
        """The exact converted value in the requested base (lossless read-time
        unit/base conversion), or None when no conversion was applied.

        Also None — with `converted` True — when the exact result has no
        terminating expansion in the requested base and the reader was opened
        with want_unit_allow_nonterminating: there is no finite digit string to
        return, only the exact rational in conv.num/conv.den. Check `converted`
        rather than this method to tell the two cases apart."""
        if not self.converted or not self.conv.text:
            return None
        return self.conv.text.decode('ascii')

    def converted_rational(self) -> "tuple[int, int] | None":
        """The exact converted value as a reduced ``(numerator, denominator)``
        pair, or None when no conversion was applied.

        This is the only carrier of the value when ``converted_str()`` returns
        None — the result was exact but had no terminating expansion in the
        requested base (``want_unit_allow_nonterminating``). ``Fraction(*pair)``
        gives you the exact number.

        On a ``BvnrData`` a reader callback received this stays valid after
        the callback and after the reader is gone: the snapshot owns copies of
        the two bignums.
        """
        if not self.converted or not self.conv.num or not self.conv.den:
            return None
        from ._ffi import get_library
        lib = get_library()

        def _to_int(handle: int) -> int:
            # Base 16, not 10: a power-of-two base is exempt from CPython's
            # int_max_str_digits limit (4300 decimal digits by default), and a
            # conversion result can be as wide as the format's 32768 bits.
            bits = lib.bvn_int_bitlen(handle) or 1
            size = lib.bvn_int_str_bufsize(bits, 16)
            buf  = ctypes.create_string_buffer(size)
            if lib.bvn_int_to_str(handle, buf, size, 16) < 0:
                raise ValueError("bvn_int_to_str failed on a conversion result")
            return int(buf.value.decode('ascii'), 16)

        return (_to_int(self.conv.num), _to_int(self.conv.den))

    def converted_in_base(self, base: int) -> "str | None":
        """Render the exact converted value in ``base`` (2..62, 64 or 85).

        Returns None when no conversion was applied, or when the exact value has
        no terminating expansion in that base — use converted_rational() there.
        Lets a caller re-render in a base other than the one requested at read
        time. Like converted_rational(), it keeps working on a callback's
        ``BvnrData`` after the reader is gone.
        """
        if not self.converted or not self.conv.num or not self.conv.den:
            return None
        from ._ffi import get_library
        lib   = get_library()
        size  = lib.bvn_rational_str_bufsize(self.conv.num, self.conv.den, base)
        if not size:
            raise ValueError(f"base {base} is not one bovnar can write")
        buf   = ctypes.create_string_buffer(size)
        exact = ctypes.c_bool(False)
        n = lib.bvn_rational_to_str(self.conv.num, self.conv.den, base,
                                    buf, size, ctypes.byref(exact))
        if n < 0 or not exact.value:
            return None                    # non-terminating, or unrepresentable
        return buf.value.decode('ascii')

    def _own_buffers(self) -> None:
        """Replace every pointer into reader memory with a copy this object
        owns, so each accessor works after the callback returns and after the
        reader is destroyed.

        A reader hands its callbacks a view whose ``data``, ``frac_data``,
        ``unit_source`` and ``conv.num``/``conv.den`` point at storage the
        reader reuses and frees; a ``BvnrData`` stashed past the callback then
        read freed memory -- raw_bytes() returned garbage and
        converted_rational() walked a freed bignum. ``conv.text`` is a
        ``c_char_p`` and was already copied by ctypes on assignment.
        """
        # Copied as ctypes arrays (from_buffer_copy of a view on the source):
        # about a microsecond per field, which matters at one call per event.
        keep = []
        _arr = ctypes.c_char
        ptr, n = self.data, self.length
        if ptr:
            if n:
                b = (_arr * n).from_buffer_copy((_arr * n).from_address(ptr))
                keep.append(b)
                self.data = ctypes.addressof(b)
            else:
                self.data = None
        ptr, n = self.frac_data, self.frac_length
        if ptr:
            if n:
                b = (_arr * n).from_buffer_copy((_arr * n).from_address(ptr))
                keep.append(b)
                self.frac_data = ctypes.addressof(b)
            else:
                self.frac_data = None
        ptr, n = self.unit_source, self.unit_source_length
        if ptr:
            if n:
                b = (_arr * n).from_buffer_copy((_arr * n).from_address(ptr))
                keep.append(b)
                self.unit_source = ctypes.addressof(b)
            else:
                self.unit_source = None
        if self.converted:
            conv = self.conv
            if conv.num and conv.den:
                num, den = _OwnedBigInt(conv.num), _OwnedBigInt(conv.den)
                keep += (num, den)
                conv.num, conv.den = num.ptr, den.ptr
            else:
                conv.num = conv.den = None
        # Without a conversion conv is not populated (Reader._snapshot copies
        # it only when `converted` is set), so there is no pointer to replace.
        self._owned = keep

    def raw_bytes(self) -> bytes:
        if not self.data or self.length == 0:
            return b''
        return (ctypes.c_char * self.length).from_address(self.data).raw

    def raw_str(self, encoding: str = 'utf-8') -> str:
        return self.raw_bytes().decode(encoding)

    def __repr__(self) -> str:
        return (f"BvnrData(type={self.type}, "
                f"vt={self.value_type!r}, "
                f"len={self.length}, "
                f"raw={self.raw_bytes()!r})")

EVENT_CALLBACK_FUNC = ctypes.CFUNCTYPE(
    ctypes.c_bool,
    ctypes.c_void_p,
    ctypes.c_int,
    ctypes.POINTER(BvnrData),
)

# bvnr_read_flags_t.want_unit:
#   bool (*want_unit)(void* ud, const bvnr_data_t* data,
#                     value_unit_t* want, uint32_t* want_base)
WANT_UNIT_FUNC = ctypes.CFUNCTYPE(
    ctypes.c_bool,
    ctypes.c_void_p,
    ctypes.POINTER(BvnrData),
    ctypes.POINTER(ValueUnit),
    ctypes.POINTER(ctypes.c_uint32),
)

# bvnr_stream.h callback signatures.
#   bool (*on_document)(void* ud, uint64_t index, bool ok, error_code_t err)
ON_DOCUMENT_FUNC = ctypes.CFUNCTYPE(
    ctypes.c_bool,
    ctypes.c_void_p,
    ctypes.c_uint64,
    ctypes.c_bool,
    ctypes.c_int,
)
#   bool (*on_message)(void* ud, uint64_t channel, const uint8_t* data, uint64_t len)
MUX_ON_MSG_FUNC = ctypes.CFUNCTYPE(
    ctypes.c_bool,
    ctypes.c_void_p,
    ctypes.c_uint64,
    ctypes.c_void_p,
    ctypes.c_uint64,
)

class BvnrSource(ctypes.Structure):
    _fields_ = [('_opaque', ctypes.c_uint64 * _OPAQUE_WORDS)]

class BvnrSink(ctypes.Structure):
    _fields_ = [('_opaque', ctypes.c_uint64 * _OPAQUE_WORDS)]

class BvnrReadFlags(ctypes.Structure):
    _fields_ = [
        ('max_identifier_length', ctypes.c_uint16),
        ('max_string_length',     ctypes.c_uint16),
        ('max_number_length',     ctypes.c_uint16),
        ('max_symbol_length',     ctypes.c_uint16),
        ('max_reference_length',  ctypes.c_uint16),
        ('max_array_items',       ctypes.c_uint64),
        ('max_text_bytes',        ctypes.c_uint64),
        ('max_file_size',         ctypes.c_uint64),
        ('max_struct_nesting',    ctypes.c_uint8),
        ('max_array_nesting',     ctypes.c_uint8),
        ('userdata',              ctypes.c_void_p),
        ('on_unverified',         EVENT_CALLBACK_FUNC),
        ('on_verified',           EVENT_CALLBACK_FUNC),
        ('continue_on_error',     ctypes.c_bool),
        ('on_error',              ON_ERROR_FUNC),
        ('strict_version',        ctypes.c_bool),
        # spec 1.2 -- refuse a document containing an octet stream. Must sit
        # exactly where the C struct puts it: this is a by-value mirror, and a
        # field in the wrong place silently misreads every field after it.
        ('text_only',             ctypes.c_bool),
        ('want_unit_allow_nonterminating', ctypes.c_bool),
        ('max_conversion_length', ctypes.c_uint32),
        ('want_unit',             WANT_UNIT_FUNC),
        ('_reserved',             ctypes.c_uint64 * 2),
    ]

class BvnrUnitTarget(ctypes.Structure):
    """One conversion target: the unit, and the base to render it in."""
    _fields_ = [
        ('unit', ctypes.c_char_p),
        ('base', ctypes.c_uint32),
    ]

class BvnrUnitRule(ctypes.Structure):
    """One per-field rule: which key path, which unit, and what to do."""
    _fields_ = [
        ('path', ctypes.c_char_p),
        ('unit', ctypes.c_char_p),
        ('base', ctypes.c_uint32),
        ('mode', ctypes.c_int),
    ]

class BvnrUnitPolicy(ctypes.Structure):
    """Mirror of bvnr_unit_policy_t (bvnr_reader_set_unit_policy).

    Everything is unit TEXT rather than a built ValueUnit, which is the whole
    reason this is reachable from Python without a callback: the C side parses
    the strings when the policy is set. Keep any Python objects holding those
    strings alive for the duration of the set call -- ctypes does not own them.
    """
    _fields_ = [
        ('rules',                    ctypes.POINTER(BvnrUnitRule)),
        ('num_rules',                ctypes.c_uint32),
        ('targets',                  ctypes.POINTER(BvnrUnitTarget)),
        ('num_targets',              ctypes.c_uint32),
        ('base',                     ctypes.c_uint32),
        ('normalise',                ctypes.c_int),
        ('on_inexact',               ctypes.c_int),
        ('require_unit',             ctypes.c_bool),
        ('require_dimension_of',     ctypes.POINTER(ctypes.c_char_p)),
        ('num_require_dimension_of', ctypes.c_uint32),
    ]

MAX_UNIT_TARGETS = 8          # BVNR_MAX_UNIT_TARGETS
MAX_UNIT_RULES   = 8          # BVNR_MAX_UNIT_RULES
MAX_UNIT_PATH    = 96         # BVNR_MAX_UNIT_PATH


def build_unit_policy(targets=(), base=0, normalise_si=False,
                      leave_inexact=False, require_unit=False,
                      require_dimension_of=(), rules=()):
    """Marshal a UnitPolicy into (BvnrUnitPolicy, keepalive).

    Shared by the reader and the writer so the two cannot disagree about what a
    policy means. The caller MUST keep `keepalive` referenced until the C call
    returns: it holds the encoded unit strings the struct's pointers refer to.

    Raises ValueError on a count the C side would reject anyway; a malformed
    unit string is left to the C parser, which reports it against the same
    table the documents are read with.
    """
    if len(targets) > MAX_UNIT_TARGETS:
        raise ValueError(
            f"at most {MAX_UNIT_TARGETS} unit targets, got {len(targets)}")
    if len(require_dimension_of) > MAX_UNIT_TARGETS:
        raise ValueError(
            f"at most {MAX_UNIT_TARGETS} required dimensions, "
            f"got {len(require_dimension_of)}")
    if len(rules) > MAX_UNIT_RULES:
        raise ValueError(
            f"at most {MAX_UNIT_RULES} per-field rules, got {len(rules)}")

    keepalive = []
    c_rules = (BvnrUnitRule * max(len(rules), 1))()
    for i, r in enumerate(rules):
        pth = r.path.encode('utf-8')
        unt = r.unit.encode('utf-8')
        keepalive.extend((pth, unt))
        c_rules[i].path = pth
        c_rules[i].unit = unt
        c_rules[i].base = int(r.base)
        c_rules[i].mode = 0 if r.convert else 1
    c_targets = (BvnrUnitTarget * max(len(targets), 1))()
    for i, t in enumerate(targets):
        unit, tbase = (t, 0) if isinstance(t, str) else (t[0], t[1])
        enc = unit.encode('utf-8')
        keepalive.append(enc)
        c_targets[i].unit = enc
        c_targets[i].base = int(tbase)

    c_require = (ctypes.c_char_p * max(len(require_dimension_of), 1))()
    for i, u in enumerate(require_dimension_of):
        enc = u.encode('utf-8')
        keepalive.append(enc)
        c_require[i] = enc

    keepalive.extend((c_targets, c_require, c_rules))
    cp = BvnrUnitPolicy()
    cp.rules                    = c_rules
    cp.num_rules                = len(rules)
    cp.targets                  = c_targets
    cp.num_targets              = len(targets)
    cp.base                     = int(base)
    cp.normalise                = 1 if normalise_si else 0
    cp.on_inexact               = 1 if leave_inexact else 0
    cp.require_unit             = bool(require_unit)
    cp.require_dimension_of     = c_require
    cp.num_require_dimension_of = len(require_dimension_of)
    return cp, keepalive


UNIT_POLICY_REFUSED_GENERAL = (
    "unusable unit policy — check the units, bases and rule paths in "
    "targets / require_dimension_of / rules")


def describe_unit_policy_refusal(lib, targets=(), base=0, normalise_si=False,
                                 leave_inexact=False, require_unit=False,
                                 require_dimension_of=(), rules=()):
    """Name the entry the C setter refused, by offering each one on its own.

    ``bvnr_reader_set_unit_policy`` answers one bool for a whole policy, so both
    bindings reported every refusal as "check the unit spellings in targets /
    require_dimension_of". That never mentioned ``rules`` at all, and described a
    bad rule PATH — over ``MAX_UNIT_PATH`` bytes, missing its leading '.', or a
    bare '.*' — and an invalid base as unit spellings.

    Shared by the reader and the writer for the same reason
    :func:`build_unit_policy` is. The setter itself is the oracle rather than a
    re-derivation of its rules here, so this cannot drift away from
    ``bvn_unit_policy_parse`` the next time that grows a check.

    Returns the message for the first entry the library refuses, or
    :data:`UNIT_POLICY_REFUSED_GENERAL` if nothing singles itself out.
    """
    rd = lib.bvnr_reader_create()
    if not rd:
        return UNIT_POLICY_REFUSED_GENERAL
    try:
        def refuses(**kw):
            cp, keepalive = build_unit_policy(**kw)
            ok = lib.bvnr_reader_set_unit_policy(rd, ctypes.byref(cp))
            del keepalive
            return not ok

        for t in targets:
            if not refuses(targets=[t]):
                continue
            unit, tbase = (t, 0) if isinstance(t, str) else (t[0], t[1])
            if tbase and not refuses(targets=[unit]):
                return (f"unusable unit policy: targets entry {unit!r} asks for "
                        f"base {tbase}, which bovnar cannot write")
            return (f"unusable unit policy: targets entry {unit!r} is not a unit "
                    f"bovnar can parse")

        for u in require_dimension_of:
            if refuses(require_dimension_of=[u]):
                return (f"unusable unit policy: require_dimension_of entry {u!r} "
                        f"is not a unit bovnar can parse")

        for r in rules:
            if not refuses(rules=[r]):
                continue
            # Which part? Hold the others at values that certainly parse and
            # vary one at a time: path first, since it is the only one left
            # when a unit that parses and no base still will not go in.
            plain = SimpleNamespace(path=r.path, unit="m", base=0,
                                    convert=r.convert)
            if refuses(rules=[plain]):
                return (f"unusable unit policy: rule path {r.path!r} is unusable "
                        f"— a path must start with '.', be at most "
                        f"{MAX_UNIT_PATH} bytes, and cannot be just '.*'")
            unbased = SimpleNamespace(path=r.path, unit=r.unit, base=0,
                                      convert=r.convert)
            if refuses(rules=[unbased]):
                return (f"unusable unit policy: rule {r.path!r} names unit "
                        f"{r.unit!r}, which is not a unit bovnar can parse")
            return (f"unusable unit policy: rule {r.path!r} asks for base "
                    f"{r.base}, which bovnar cannot write")

        # Nothing singled itself out: a whole-policy field (base, normalise,
        # on_inexact) or a combination.
        if base and not refuses(targets=list(targets), normalise_si=normalise_si,
                                leave_inexact=leave_inexact,
                                require_unit=require_unit,
                                require_dimension_of=list(require_dimension_of),
                                rules=list(rules)):
            return (f"unusable unit policy: base {base} is not a base bovnar can "
                    f"write")
        return UNIT_POLICY_REFUSED_GENERAL
    finally:
        lib.bvnr_reader_destroy(rd)


class BvnrWriteFlags(ctypes.Structure):
    _fields_ = [
        ('max_identifier_length', ctypes.c_uint16),
        ('max_string_length',     ctypes.c_uint16),
        ('max_number_length',     ctypes.c_uint16),
        ('max_symbol_length',     ctypes.c_uint16),
        ('max_reference_length',  ctypes.c_uint16),
        ('max_array_items',       ctypes.c_uint64),
        ('max_text_bytes',        ctypes.c_uint64),
        ('max_file_size',         ctypes.c_uint64),
        ('max_struct_nesting',    ctypes.c_uint8),
        ('max_array_nesting',     ctypes.c_uint8),
        ('userdata',              ctypes.c_void_p),
        ('on_event',              EVENT_CALLBACK_FUNC),
        ('continue_on_error',     ctypes.c_bool),
        ('on_error',              ON_ERROR_FUNC),
        ('unit_flags',            ctypes.c_uint32),
        ('emit_version',          ctypes.c_bool),
        ('_reserved',             ctypes.c_uint64 * 4),
    ]

class BvnrDocStreamOpts(ctypes.Structure):
    """Mirror of bvnr_doc_stream_opts_t (bovnar_stream.h)."""
    _fields_ = [
        ('flags',                ctypes.POINTER(BvnrReadFlags)),
        ('userdata',             ctypes.c_void_p),
        ('on_document',          ON_DOCUMENT_FUNC),
        ('continue_past_failed', ctypes.c_bool),
        ('max_document_size',    ctypes.c_uint64),
    ]

class BvnDomEntry(ctypes.Structure):
    _fields_ = [
        ('key',   ctypes.c_char_p),
        ('value', ctypes.c_void_p),
    ]

def make_data_key(key: str) -> tuple:
    raw = key.encode('utf-8')
    d = BvnrData()
    d.data = ctypes.cast(ctypes.c_char_p(raw), ctypes.c_void_p)
    d.length = len(raw)
    return d, raw

def make_data_value(value_str: str,
                    vt: ValueTypeSpec,
                    vu: ValueUnit) -> tuple:
    raw = value_str.encode('utf-8')
    d = BvnrData()
    d.value_type = vt
    d.value_unit = vu
    d.data = ctypes.cast(ctypes.c_char_p(raw), ctypes.c_void_p)
    d.length = len(raw)
    return d, raw

def make_data_typed(vt: ValueTypeSpec, vu: ValueUnit) -> BvnrData:
    d = BvnrData()
    d.value_type = vt
    d.value_unit = vu
    return d

def make_type_spec(family: ValueTypeFamily,
                   width: int = 0,
                   base: int = 0) -> ValueTypeSpec:
    vt = ValueTypeSpec()
    vt.family = int(family)
    vt.width  = width
    vt.base   = base
    return vt

def make_unit_dimensionless() -> ValueUnit:
    vu = ValueUnit()
    vu.num_components = 1
    vu.components[0].base              = int(BaseUnit.NONE)
    vu.components[0].exponent          = int(Exponent.LINEAR)
    vu.components[0].prefix.system     = int(PrefixSystem.SI)
    vu.components[0].prefix.id.si      = int(SIPrefix.NONE)
    return vu

def make_unit_none() -> ValueUnit:
    return ValueUnit()

def make_unit_si(base: BaseUnit,
                 prefix: SIPrefix = SIPrefix.NONE,
                 exp: Exponent = Exponent.LINEAR) -> ValueUnit:
    vu = ValueUnit()
    vu.num_components = 1
    c = vu.components[0]
    c.base             = int(base)
    c.exponent         = int(exp)
    c.prefix.system    = int(PrefixSystem.SI)
    c.prefix.id.si     = int(prefix)
    return vu

def make_unit_iec(base: BaseUnit,
                  prefix: IECPrefix = IECPrefix.NONE,
                  exp: Exponent = Exponent.LINEAR) -> ValueUnit:
    vu = ValueUnit()
    vu.num_components = 1
    c = vu.components[0]
    c.base             = int(base)
    c.exponent         = int(exp)
    c.prefix.system    = int(PrefixSystem.IEC)
    c.prefix.id.iec    = int(prefix)
    return vu

def make_unit_compound(components: list[dict]) -> ValueUnit:
    """
    Build a ValueUnit with multiple components (e.g. kg·m/s²).

    Each dict in *components* must contain:
      'base'       : BaseUnit  (required)
      'exp'        : Exponent  (default LINEAR)
      'si_prefix'  : SIPrefix  (optional, default NONE, mutually exclusive with iec_prefix)
      'iec_prefix' : IECPrefix (optional, mutually exclusive with si_prefix)

    Example – Newton (kg·m·s⁻²):
      make_unit_compound([
          {'base': BaseUnit.GRAM,   'exp': Exponent.LINEAR,     'si_prefix': SIPrefix.KILO},
          {'base': BaseUnit.METER,  'exp': Exponent.LINEAR},
          {'base': BaseUnit.SECOND, 'exp': Exponent.NEG_SQUARE},
      ])
    """
    if len(components) > MAX_UNIT_COMPONENTS:
        raise ValueError(
            f"Too many unit components: {len(components)} > {MAX_UNIT_COMPONENTS}")
    vu = ValueUnit()
    vu.num_components = len(components)
    _KEYS = {'base', 'exp', 'si_prefix', 'iec_prefix'}
    for i, comp in enumerate(components):
        # An unrecognised key used to be dropped in silence, which is how
        # {'base': ..., 'prefix': SIPrefix.KILO} built an UNPREFIXED unit and
        # every downstream check agreed it was fine. The keys differ by one word
        # from the obvious guess, so the typo is the likely case, not the rare one.
        unknown = set(comp) - _KEYS
        if unknown:
            raise ValueError(
                f"component {i}: unknown key(s) {sorted(unknown)}; "
                f"expected any of {sorted(_KEYS)}")
        c = vu.components[i]
        c.base     = int(comp['base'])
        c.exponent = int(comp.get('exp', Exponent.LINEAR))
        if 'iec_prefix' in comp:
            c.prefix.system  = int(PrefixSystem.IEC)
            c.prefix.id.iec  = int(comp['iec_prefix'])
        else:
            c.prefix.system  = int(PrefixSystem.SI)
            c.prefix.id.si   = int(comp.get('si_prefix', SIPrefix.NONE))
    return vu
