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
from enum import IntEnum
from typing import Iterator

from ._ffi import get_library
from .enums import ErrorCode, ValueTypeFamily
from .structs import ValueTypeSpec, ValueUnit, BvnDomEntry, UNIT_STRING_MAX
from .exceptions import BovnarArgumentError, BovnarParseError

class DomType(IntEnum):
    NULL         = 0
    INT          = 1
    FLOAT        = 2
    STRING       = 3
    SYMBOL       = 4
    REFERENCE    = 5
    STRUCT       = 6
    ARRAY        = 7
    OCTET_STREAM = 8
    BOOL         = 9

class DomNode:
    """
    Non-owning view into a bvn_dom_node_t.  The parent DomDoc must stay
    alive for the lifetime of any DomNode derived from it.
    """
    __slots__ = ('_ptr', '_doc')

    def __init__(self, ptr: int, doc: 'DomDoc') -> None:
        self._ptr = ptr
        self._doc = doc

    @property
    def dom_type(self) -> DomType:
        return DomType(get_library().bvn_dom_node_type(self._ptr))

    @property
    def value_type(self) -> ValueTypeSpec:
        return get_library().bvn_dom_get_value_type(self._ptr)

    @property
    def unit(self) -> ValueUnit:
        return get_library().bvn_dom_get_unit(self._ptr)

    @property
    def unit_str(self) -> str:
        """The canonical unit string, or ``''`` when the value carries no unit
        -- the same contract as ``Quantity.unit_str``. (The C formatter spells
        no unit ``no_unit``; that literal used to come through here, so the two
        APIs disagreed on the same document.)"""
        from . import _has_real_unit
        if not _has_real_unit(self.unit):
            return ''
        lib = get_library()
        buf = ctypes.create_string_buffer(UNIT_STRING_MAX)
        n   = lib.bvn_dom_get_unit_string(self._ptr, buf, UNIT_STRING_MAX)
        if n < 0:
            return ''
        return buf.raw[:n].decode('utf-8')

    def is_null(self) -> bool:
        return bool(get_library().bvn_dom_is_null(self._ptr))

    def value_in_base_units(self) -> float:
        return float(get_library().bvn_dom_get_value_in_base_units(self._ptr))

    def as_i64(self) -> int:
        lib = get_library()
        out = ctypes.c_int64()
        if not lib.bvn_dom_get_i64(self._ptr, ctypes.byref(out)):
            raise ValueError("Node does not hold a value representable as int64")
        return int(out.value)

    def as_u64(self) -> int:
        lib = get_library()
        out = ctypes.c_uint64()
        if not lib.bvn_dom_get_u64(self._ptr, ctypes.byref(out)):
            raise ValueError("Node does not hold a value representable as uint64")
        return int(out.value)

    def as_i32(self) -> int:
        lib = get_library()
        out = ctypes.c_int32()
        if not lib.bvn_dom_get_i32(self._ptr, ctypes.byref(out)):
            raise ValueError("Node does not hold a value representable as int32")
        return int(out.value)

    def as_u32(self) -> int:
        lib = get_library()
        out = ctypes.c_uint32()
        if not lib.bvn_dom_get_u32(self._ptr, ctypes.byref(out)):
            raise ValueError("Node does not hold a value representable as uint32")
        return int(out.value)

    def as_i16(self) -> int:
        lib = get_library()
        out = ctypes.c_int16()
        if not lib.bvn_dom_get_i16(self._ptr, ctypes.byref(out)):
            raise ValueError("Node does not hold a value representable as int16")
        return int(out.value)

    def as_u16(self) -> int:
        lib = get_library()
        out = ctypes.c_uint16()
        if not lib.bvn_dom_get_u16(self._ptr, ctypes.byref(out)):
            raise ValueError("Node does not hold a value representable as uint16")
        return int(out.value)

    def as_i8(self) -> int:
        lib = get_library()
        out = ctypes.c_int8()
        if not lib.bvn_dom_get_i8(self._ptr, ctypes.byref(out)):
            raise ValueError("Node does not hold a value representable as int8")
        return int(out.value)

    def as_u8(self) -> int:
        lib = get_library()
        out = ctypes.c_uint8()
        if not lib.bvn_dom_get_u8(self._ptr, ctypes.byref(out)):
            raise ValueError("Node does not hold a value representable as uint8")
        return int(out.value)

    def as_float(self) -> float:
        lib = get_library()
        out = ctypes.c_double()
        if not lib.bvn_dom_get_float(self._ptr, ctypes.byref(out)):
            raise ValueError("Node is not a float")
        return float(out.value)

    def as_bool(self) -> bool:
        lib = get_library()
        out = ctypes.c_bool()
        if not lib.bvn_dom_get_bool(self._ptr, ctypes.byref(out)):
            raise ValueError("Node is not a bool")
        return bool(out.value)

    def as_str(self) -> str:
        """Return string, symbol, or reference value as Python str."""
        lib = get_library()
        dt  = self.dom_type
        out_ptr = ctypes.c_void_p()
        length  = ctypes.c_uint32()
        if dt == DomType.STRING:
            ok = lib.bvn_dom_get_string(self._ptr,
                                         ctypes.byref(out_ptr),
                                         ctypes.byref(length))
        elif dt == DomType.SYMBOL:
            ok = lib.bvn_dom_get_symbol(self._ptr,
                                         ctypes.byref(out_ptr),
                                         ctypes.byref(length))
        elif dt == DomType.REFERENCE:
            ok = lib.bvn_dom_get_reference(self._ptr,
                                            ctypes.byref(out_ptr),
                                            ctypes.byref(length))
        else:
            raise ValueError(f"as_str() not valid for DomType.{dt.name}")
        if not ok or not out_ptr.value:
            return ''
        return ctypes.string_at(out_ptr.value, length.value).decode('utf-8')

    def as_bytes(self) -> bytes:
        """Return octet-stream payload."""
        lib     = get_library()
        out_ptr = ctypes.c_void_p()
        length  = ctypes.c_uint32()
        if not lib.bvn_dom_get_octets(self._ptr,
                                       ctypes.byref(out_ptr),
                                       ctypes.byref(length)):
            raise ValueError("Node is not an octet stream")
        if not out_ptr.value:
            return b''
        return bytes(ctypes.string_at(out_ptr.value, length.value))

    def as_int_str(self, base: int = 10) -> str:
        """
        Return the integer value as a string in the given base.
        Works for both native int64/uint64 and arbitrary-width bigints.
        The returned C string is copied and freed before returning.
        """
        lib     = get_library()
        raw_ptr = lib.bvn_dom_int_to_str(self._ptr, ctypes.c_uint32(base))
        if not raw_ptr:
            raise ValueError("bvn_dom_int_to_str returned NULL")
        try:
            s = ctypes.cast(raw_ptr, ctypes.c_char_p).value.decode('ascii')
        finally:
            lib.bvn_dom_free_string(ctypes.c_void_p(raw_ptr))
        return s

    @property
    def unit_source(self) -> "str | None":
        """
        The node's unit exactly as the document spelled it (library 2.0), where
        that says something ``unit`` cannot -- ``ucum:mL{total}`` is the unit
        ``m~L``, and the annotation survives only here. ``None`` for a node with
        no unit, for one already written in canonical form (``unit_str`` is then
        the spelling), and for one whose value a unit policy converted.
        """
        lib = get_library()
        ptr = lib.bvn_dom_get_unit_source(self._ptr, None)
        if not ptr:
            return None
        return ptr.decode('utf-8', errors='replace')

    @property
    def datetime_fraction(self) -> "str | None":
        """
        The verbatim ISO sub-second digits (without the leading '.') of a
        datetime written as a literal with a fractional part (spec 1.1), or
        ``None`` for a non-datetime node, a datetime given as an integer
        carrier, or a literal with no fraction. The carrier value (whole epoch
        seconds) is unchanged and still read via ``to_python()`` / ``as_int_str``;
        this exposes the fraction the integer carrier cannot hold.
        """
        lib = get_library()
        ptr = lib.bvn_dom_get_datetime_fraction(self._ptr, None)
        if not ptr:
            return None
        return ptr.decode('ascii')

    def __len__(self) -> int:
        lib = get_library()
        dt  = self.dom_type
        if dt == DomType.STRUCT:
            return int(lib.bvn_dom_struct_count(self._ptr))
        if dt == DomType.ARRAY:
            return int(lib.bvn_dom_array_count(self._ptr))
        return 0

    def __getitem__(self, key) -> 'DomNode':
        lib = get_library()
        dt  = self.dom_type
        if dt == DomType.STRUCT:
            if not isinstance(key, str):
                raise TypeError("Struct key must be a str")
            raw = key.encode('utf-8')
            ptr = lib.bvn_dom_struct_get(self._ptr, raw)
            if not ptr:
                raise KeyError(key)
            return DomNode(ptr, self._doc)
        if dt == DomType.ARRAY:
            if not isinstance(key, int):
                raise TypeError("Array index must be an int")
            n = int(lib.bvn_dom_array_count(self._ptr))
            if key < 0:
                key = n + key
            if key < 0 or key >= n:
                raise IndexError(key)
            ptr = lib.bvn_dom_array_at(self._ptr, ctypes.c_uint32(key))
            if not ptr:
                raise IndexError(key)
            return DomNode(ptr, self._doc)
        raise TypeError(f"DomType.{dt.name} does not support indexing")

    def __iter__(self) -> Iterator:
        dt = self.dom_type
        if dt == DomType.STRUCT:
            return iter(self.entries())
        if dt == DomType.ARRAY:
            return (self[i] for i in range(len(self)))
        raise TypeError(f"DomType.{dt.name} is not iterable")

    def __contains__(self, key: str) -> bool:
        # A key is a str; anything else is simply not one of them. (It was an
        # AttributeError from `.encode`, where `in` should just answer.)
        if not isinstance(key, str) or self.dom_type != DomType.STRUCT:
            return False
        lib = get_library()
        return bool(lib.bvn_dom_struct_get(self._ptr, key.encode('utf-8')))

    def entries(self) -> list[tuple[str, 'DomNode']]:
        """Return list of (key, DomNode) pairs for STRUCT nodes."""
        lib   = get_library()
        count = int(lib.bvn_dom_struct_count(self._ptr))
        if count == 0:
            return []
        ep = lib.bvn_dom_struct_entries(self._ptr)
        result = []
        for i in range(count):
            e = ep[i]
            key = e.key.decode('utf-8') if e.key else ''
            result.append((key, DomNode(e.value, self._doc)))
        return result

    def array_dims(self) -> int:
        """Number of /‑separated dimensions in an ARRAY node."""
        return int(get_library().bvn_dom_array_dims(self._ptr))

    def to_python(self):
        """
        Convert the node to a native Python value.  Type and unit info
        is dropped; use the typed accessors when you need them.
        """
        lib = get_library()
        dt  = self.dom_type
        if dt == DomType.NULL:
            return None
        if dt == DomType.INT:
            # get_i64 succeeds for any width<=64 value (the C extractor does an
            # unchecked reinterpret), so for an unsigned family a uint64 >= 2**63
            # would come back as a negative two's-complement number. Read with the
            # accessor matching the declared signedness first; fall back to the
            # arbitrary-width string for values that fit neither 64-bit type.
            # datetime carries a SIGNED epoch-seconds value (pre-1970 timestamps
            # are negative), so it reads as signed like sint — otherwise a
            # negative datetime decodes as a huge unsigned int.
            v64 = ctypes.c_int64()
            u64 = ctypes.c_uint64()
            if self.value_type.family in (ValueTypeFamily.SINT,
                                          ValueTypeFamily.DATETIME):
                if lib.bvn_dom_get_i64(self._ptr, ctypes.byref(v64)):
                    return int(v64.value)
                if lib.bvn_dom_get_u64(self._ptr, ctypes.byref(u64)):
                    return int(u64.value)
            else:
                if lib.bvn_dom_get_u64(self._ptr, ctypes.byref(u64)):
                    return int(u64.value)
                if lib.bvn_dom_get_i64(self._ptr, ctypes.byref(v64)):
                    return int(v64.value)
            # Base 16, not 10: CPython refuses a decimal int() past 4300
            # digits (int_max_str_digits) and a 32768-bit value has ~9865.
            # Power-of-two bases are exempt from that limit.
            return int(self.as_int_str(16), 16)
        if dt == DomType.FLOAT:
            return self.as_float()
        if dt == DomType.BOOL:
            return self.as_bool()
        if dt in (DomType.STRING, DomType.SYMBOL, DomType.REFERENCE):
            return self.as_str()
        if dt == DomType.OCTET_STREAM:
            return self.as_bytes()
        if dt == DomType.STRUCT:
            return {k: v.to_python() for k, v in self.entries()}
        if dt == DomType.ARRAY:
            flat = [self[i].to_python() for i in range(len(self))]
            # The C DOM stores /-separated dimension rows FLAT and records the
            # row count separately, so ignoring array_dims() mapped two different
            # documents onto the same Python value: "[1,2,3]/[4,5,6]" and
            # "[1,2,3,4,5,6]" both came back as [1, 2, 3, 4, 5, 6], while the
            # streaming path (bovnar.loads) correctly gives [[1,2,3],[4,5,6]].
            # Row sizes are equal by construction -- an uneven one is
            # error_array_row_size_mismatch -- so the split is exact.
            #
            # That holds for rows of ZERO elements as well: "[]/[]" is two
            # empty rows, loads() says [[], []], and skipping the split for an
            # empty flat list answered [] here.
            rows = self.array_dims()
            if rows > 1 and len(flat) % rows == 0:
                width = len(flat) // rows
                return [flat[i * width:(i + 1) * width] for i in range(rows)]
            return flat
        return None

    def __repr__(self) -> str:
        return f"DomNode(type={self.dom_type.name}, ptr=0x{self._ptr:x})"


class DomDoc:
    """
    Owning wrapper around a bvn_dom_doc_t.  Destruction of this object
    frees the entire document tree; all DomNodes derived from it become
    invalid after that.
    """
    __slots__ = ('_ptr',)

    def __init__(self, ptr: int) -> None:
        # Assigned before the check: __del__ runs on a half-built object too,
        # and with __slots__ an unset `_ptr` is an AttributeError there --
        # reported as "Exception ignored in DomDoc.__del__" right after the
        # MemoryError the caller was actually owed.
        self._ptr = 0
        if not ptr:
            raise MemoryError("DomDoc received a NULL pointer")
        self._ptr = ptr

    def __del__(self) -> None:
        if getattr(self, '_ptr', 0):
            get_library().bvn_dom_doc_destroy(self._ptr)
            self._ptr = 0

    @staticmethod
    def _c_policy(policy):
        """Marshal a UnitPolicy for the DOM entry points, or (None, None)."""
        if policy is None:
            return None, None
        # Validate exactly as Reader.set_unit_policy does, on a throwaway
        # reader: a policy the library refuses raises BovnarArgumentError
        # naming the bad entry. Handed straight to bvn_dom_parse_policy it
        # surfaced as a BovnarParseError(INVALID_ARGUMENT) with no reason --
        # a mistake in the policy reported as a fault in the document.
        from .reader import Reader
        with Reader() as r:
            r.set_unit_policy(policy)
        from .structs import build_unit_policy
        try:
            cp, keepalive = build_unit_policy(
                policy.targets, policy.base, policy.normalise_si,
                policy.leave_inexact, policy.require_unit,
                policy.require_dimension_of, policy.rules)
        except ValueError as e:
            raise BovnarArgumentError(str(e)) from None
        return cp, keepalive

    @classmethod
    def parse(cls, data: bytes | bytearray | memoryview,
              policy=None) -> 'DomDoc':
        """Parse BVNR bytes into a DOM tree.

        With a :class:`~bovnar.UnitPolicy`, the same rules the streaming reader
        applies: a validation failure raises, and a value the policy converted
        is STORED converted — digits, unit and base. A policy the library
        refuses raises :class:`~bovnar.BovnarArgumentError` before anything is
        parsed -- the same error, with the same reason, as
        ``Reader.set_unit_policy`` -- so a mistake in the policy is never
        mistaken for a fault in the document.
        """
        if isinstance(data, memoryview):
            data = bytes(data)
        raw  = (ctypes.c_uint8 * len(data)).from_buffer_copy(data)
        cp, _keep = cls._c_policy(policy)
        if cp is None:
            ptr = get_library().bvn_dom_parse(raw, ctypes.c_uint32(len(data)))
        else:
            ptr = get_library().bvn_dom_parse_policy(
                raw, ctypes.c_uint32(len(data)), ctypes.byref(cp))
        doc  = cls(ptr)
        err  = doc.parse_error
        if err != ErrorCode.NONE:
            raise BovnarParseError(err)
        return doc

    @classmethod
    def parse_fd(cls, fd: int, policy=None) -> 'DomDoc':
        """Parse BVNR from an open file descriptor into a DOM tree."""
        cp, _keep = cls._c_policy(policy)
        if cp is None:
            ptr = get_library().bvn_dom_parse_fd(ctypes.c_int(fd))
        else:
            ptr = get_library().bvn_dom_parse_fd_policy(
                ctypes.c_int(fd), ctypes.c_uint64(0), ctypes.byref(cp))
        doc = cls(ptr)
        err = doc.parse_error
        if err != ErrorCode.NONE:
            raise BovnarParseError(err)
        return doc

    @classmethod
    def parse_file(cls, path: str, policy=None) -> 'DomDoc':
        """Parse a BVNR file by path."""
        import os
        fd = os.open(path, os.O_RDONLY)
        try:
            return cls.parse_fd(fd, policy)
        finally:
            os.close(fd)

    @property
    def parse_error(self) -> ErrorCode:
        return ErrorCode(get_library().bvn_dom_doc_get_parse_error(self._ptr))

    def lookup(self, path: str) -> DomNode | None:
        """
        Look up a node by dot-separated key path, e.g. 'sensor.temperature'.
        Returns None if the path does not exist.
        """
        ptr = get_library().bvn_dom_lookup(self._ptr, path.encode('utf-8'))
        return DomNode(ptr, self) if ptr else None

    def __getitem__(self, key: str) -> DomNode:
        """Return an entry by key; raises KeyError if absent.

        *key* is handed to the path lookup, so a dotted or indexed path
        reaches below the top level -- ``doc["server.tls"]`` is
        ``doc["server"]["tls"]`` and ``doc["matrix[1][0]"]`` an element. A key
        cannot itself contain '.' or '[', so the two readings never collide.
        ``in`` follows the same rule."""
        ptr = get_library().bvn_dom_lookup(
            self._ptr, ('.' + key).encode('utf-8'))
        if not ptr:
            raise KeyError(key)
        return DomNode(ptr, self)

    def __contains__(self, key: str) -> bool:
        if not isinstance(key, str):
            return False
        return bool(get_library().bvn_dom_lookup(
            self._ptr, ('.' + key).encode('utf-8')))

    def __len__(self) -> int:
        return int(get_library().bvn_dom_doc_count(self._ptr))

    def __iter__(self) -> Iterator[tuple[str, DomNode]]:
        return iter(self.entries())

    def entries(self) -> list[tuple[str, DomNode]]:
        """Return all top-level (key, DomNode) pairs."""
        lib   = get_library()
        count = int(lib.bvn_dom_doc_count(self._ptr))
        if count == 0:
            return []
        ep     = lib.bvn_dom_doc_entries(self._ptr)
        result = []
        for i in range(count):
            e   = ep[i]
            key = e.key.decode('utf-8') if e.key else ''
            result.append((key, DomNode(e.value, self)))
        return result

    def to_dict(self) -> dict:
        """Convert the entire document to a plain Python dict, dropping type/unit info."""
        return {k: v.to_python() for k, v in self.entries()}

    def __repr__(self) -> str:
        return f"DomDoc(entries={len(self)}, ptr=0x{self._ptr:x})"
