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
import ctypes.util
import os
import sys
from pathlib import Path

from .structs import (
    BvnrSource, BvnrSink,
    BvnrReadFlags, BvnrWriteFlags,
    BvnrData, ValueTypeSpec, ValueUnit, ValueUnitPrefix,
    BvnrUnitTarget, BvnrUnitRule, BvnrUnitPolicy,
    BvnDomEntry, BvnrDocStreamOpts,
    EVENT_CALLBACK_FUNC, ON_ERROR_FUNC, ON_DOCUMENT_FUNC, MUX_ON_MSG_FUNC,
)
from .exceptions import BovnarLibraryNotFound

_lib: ctypes.CDLL | None = None

_LIBRARY_BASE = 'bvnr'

def _library_name() -> str:
    """Platform-specific filename of the bvnr_shared CMake target (base name 'bvnr')."""
    if sys.platform == 'darwin':
        return 'libbvnr.dylib'
    if sys.platform in ('win32', 'cygwin'):
        return 'bvnr.dll'
    return 'libbvnr.so'

_LIBRARY_NAME = _library_name()

def _candidate_paths() -> list[str]:
    paths: list[str] = []

    # 1. Explicit override always wins (developer pointing at a custom build).
    env_path = os.environ.get('LIBBOVNAR_PATH')
    if env_path:
        paths.append(env_path)

    env_dir = os.environ.get('LIBBOVNAR_DIR')
    if env_dir:
        paths.append(str(Path(env_dir) / _LIBRARY_NAME))

    # 2. Bundled-in-wheel case: CMake installs the shared library directly into
    #    the package directory, next to this file.  This is the normal path for
    #    a `pip install bovnar`.
    bundled = Path(__file__).parent / _LIBRARY_NAME
    if bundled.exists():
        paths.append(str(bundled))

    # 3. A system-wide install (e.g. distro package).
    found = ctypes.util.find_library(_LIBRARY_BASE)
    if found:
        paths.append(found)

    # 4. Editable/source checkout: fall back to the CMake build tree at the
    #    repo root (python/bovnar/_ffi.py -> ../../build).
    script_dir = Path(__file__).parent
    for rel in ('../../build', '../../build/release', '../..', '.'):
        p = (script_dir / rel / _LIBRARY_NAME).resolve()
        if p.exists():
            paths.append(str(p))

    return paths

def load_library() -> ctypes.CDLL:
    global _lib
    if _lib is not None:
        return _lib

    candidates = _candidate_paths()
    last_error: Exception | None = None

    for path in candidates:
        try:
            candidate = ctypes.CDLL(path, use_errno=True)
            _declare_functions(candidate)
        except (OSError, AttributeError) as exc:
            last_error = exc
            continue
        _lib = candidate
        return _lib

    raise BovnarLibraryNotFound(candidates) from last_error

def get_library() -> ctypes.CDLL:
    return load_library()

def _declare_functions(lib: ctypes.CDLL) -> None:
    c_bool    = ctypes.c_bool
    c_int     = ctypes.c_int
    c_int8    = ctypes.c_int8
    c_uint8   = ctypes.c_uint8
    c_int16   = ctypes.c_int16
    c_uint16  = ctypes.c_uint16
    c_int32   = ctypes.c_int32
    c_uint32  = ctypes.c_uint32
    c_int64   = ctypes.c_int64
    c_uint64  = ctypes.c_uint64
    c_double  = ctypes.c_double
    c_size_t  = ctypes.c_size_t
    c_char_p  = ctypes.c_char_p
    c_void_p  = ctypes.c_void_p
    c_uint8_p = ctypes.POINTER(ctypes.c_uint8)

    P = ctypes.POINTER

    lib.bvnr_reader_create.restype  = c_void_p
    lib.bvnr_reader_create.argtypes = []

    lib.bvnr_reader_destroy.restype  = None
    lib.bvnr_reader_destroy.argtypes = [c_void_p]

    lib.bvnr_canon_observer_create.restype  = c_void_p
    lib.bvnr_canon_observer_create.argtypes = [P(BvnrSink), c_bool]

    lib.bvnr_canon_observer_set_version.restype  = None
    lib.bvnr_canon_observer_set_version.argtypes = [c_void_p, c_uint16, c_uint16]

    lib.bvnr_canon_observer_destroy.restype  = None
    lib.bvnr_canon_observer_destroy.argtypes = [c_void_p]

    lib.bvnr_canon_observer_on_event.restype  = c_bool
    lib.bvnr_canon_observer_on_event.argtypes = [c_void_p, c_int, P(BvnrData)]

    lib.bvnr_canon_observer_finish.restype  = c_bool
    lib.bvnr_canon_observer_finish.argtypes = [c_void_p]

    lib.bvnr_source_from_fd.restype  = None
    lib.bvnr_source_from_fd.argtypes = [P(BvnrSource), c_int]

    lib.bvnr_source_from_mem.restype  = None
    lib.bvnr_source_from_mem.argtypes = [P(BvnrSource), c_void_p, c_uint64]

    lib.bvnr_open_read_source.restype  = c_bool
    lib.bvnr_open_read_source.argtypes = [
        c_void_p, P(BvnrSource), P(BvnrSink), P(BvnrReadFlags),
    ]

    lib.bvnr_open_read_mem.restype  = c_bool
    lib.bvnr_open_read_mem.argtypes = [
        c_void_p, c_void_p, c_uint64, c_void_p, c_uint64, P(BvnrReadFlags),
    ]

    lib.bvnr_read.restype  = c_bool
    lib.bvnr_read.argtypes = [c_void_p]

    lib.bvnr_reader_get_error.restype  = c_int
    lib.bvnr_reader_get_error.argtypes = [c_void_p]

    lib.bvnr_reader_get_error_line.restype  = c_uint64
    lib.bvnr_reader_get_error_line.argtypes = [c_void_p]

    lib.bvnr_reader_get_error_column.restype  = c_uint64
    lib.bvnr_reader_get_error_column.argtypes = [c_void_p]

    lib.bvnr_reader_get_error_offset.restype  = c_uint64
    lib.bvnr_reader_get_error_offset.argtypes = [c_void_p]

    lib.bvnr_reader_get_error_byte.restype  = c_uint32
    lib.bvnr_reader_get_error_byte.argtypes = [c_void_p]

    lib.bvnr_reader_get_recovery_count.restype  = c_uint64
    lib.bvnr_reader_get_recovery_count.argtypes = [c_void_p]

    lib.bvnr_reader_get_skipped_bytes.restype  = c_uint64
    lib.bvnr_reader_get_skipped_bytes.argtypes = [c_void_p]

    lib.bvnr_reader_get_declared_version.restype  = c_bool
    lib.bvnr_reader_get_declared_version.argtypes = [
        c_void_p, P(c_uint16), P(c_uint16),
    ]

    lib.bvnr_peek_version.restype  = c_bool
    lib.bvnr_peek_version.argtypes = [
        c_void_p, c_uint64, P(c_uint16), P(c_uint16),
    ]

    lib.bvnr_version.restype  = c_uint32
    lib.bvnr_version.argtypes = []

    lib.bvnr_version_string.restype  = c_char_p
    lib.bvnr_version_string.argtypes = []

    lib.bvnr_spec_version.restype  = None
    lib.bvnr_spec_version.argtypes = [P(c_uint16), P(c_uint16)]

    lib.bvnr_write_version.restype  = c_bool
    lib.bvnr_write_version.argtypes = [c_void_p, c_uint16, c_uint16]

    lib.bvnr_datetime_epoch_name.restype  = c_char_p
    lib.bvnr_datetime_epoch_name.argtypes = [ValueTypeSpec]

    lib.bvnr_datetime_epoch_mjd.restype  = c_int32
    lib.bvnr_datetime_epoch_mjd.argtypes = [ValueTypeSpec]

    lib.bvnr_writer_create.restype  = c_void_p
    lib.bvnr_writer_create.argtypes = []

    lib.bvnr_writer_destroy.restype  = None
    lib.bvnr_writer_destroy.argtypes = [c_void_p]

    lib.bvnr_sink_to_fd.restype  = None
    lib.bvnr_sink_to_fd.argtypes = [P(BvnrSink), c_int]

    lib.bvnr_sink_to_mem.restype  = None
    lib.bvnr_sink_to_mem.argtypes = [P(BvnrSink), c_void_p, c_uint64]

    lib.bvnr_sink_bytes_written.restype  = c_uint64
    lib.bvnr_sink_bytes_written.argtypes = [P(BvnrSink)]

    lib.bvnr_open_write_sink.restype  = c_bool
    lib.bvnr_open_write_sink.argtypes = [
        c_void_p, P(BvnrSink), c_bool, P(BvnrWriteFlags),
    ]

    lib.bvnr_open_write_mem.restype  = c_bool
    lib.bvnr_open_write_mem.argtypes = [
        c_void_p, c_void_p, c_uint64, c_bool, P(BvnrWriteFlags),
    ]

    lib.bvnr_write_event.restype  = c_bool
    lib.bvnr_write_event.argtypes = [c_void_p, c_int, P(BvnrData)]

    lib.bvnr_write_finish.restype  = c_bool
    lib.bvnr_write_finish.argtypes = [c_void_p]

    lib.bvnr_writer_get_error.restype  = c_int
    lib.bvnr_writer_get_error.argtypes = [c_void_p]

    lib.bvnr_writer_get_error_offset.restype  = c_uint64
    lib.bvnr_writer_get_error_offset.argtypes = [c_void_p]

    lib.bvnr_writer_bytes_written.restype  = c_uint64
    lib.bvnr_writer_bytes_written.argtypes = [c_void_p]

    lib.bvnr_writer_unit_flags.restype  = c_uint32
    lib.bvnr_writer_unit_flags.argtypes = [c_void_p]

    lib.bvn_parse_uint64.restype  = c_bool
    lib.bvn_parse_uint64.argtypes = [c_char_p, ValueTypeSpec, P(c_uint64)]

    lib.bvn_parse_int64.restype  = c_bool
    lib.bvn_parse_int64.argtypes = [c_char_p, ValueTypeSpec, P(c_int64)]

    lib.bvn_parse_double.restype  = c_bool
    lib.bvn_parse_double.argtypes = [c_char_p, ValueTypeSpec, P(c_double)]

    lib.bvn_format_uint64.restype  = c_int32
    lib.bvn_format_uint64.argtypes = [c_char_p, c_size_t, c_uint64, c_uint32, c_uint32]

    lib.bvn_format_int64.restype  = c_int32
    lib.bvn_format_int64.argtypes = [c_char_p, c_size_t, c_int64, c_uint32, c_uint32]

    lib.bvn_format_double.restype  = c_int32
    lib.bvn_format_double.argtypes = [c_char_p, c_size_t, c_double, ValueTypeSpec]

    lib.bvn_parse_unit.restype  = ValueUnit
    lib.bvn_parse_unit.argtypes = [c_uint8_p, P(c_bool)]

    lib.bvn_parse_unit_n.restype  = ValueUnit
    lib.bvn_parse_unit_n.argtypes = [c_uint8_p, c_uint32, P(c_bool)]

    lib.bvn_unit_to_string.restype  = c_int32
    lib.bvn_unit_to_string.argtypes = [ValueUnit, c_char_p, c_size_t]

    lib.bvn_unit_to_string_ex.restype  = c_int32
    lib.bvn_unit_to_string_ex.argtypes = [ValueUnit, c_char_p, c_size_t, c_uint32]

    lib.bvn_unit_valid.restype  = c_bool
    lib.bvn_unit_valid.argtypes = [ValueUnit]

    # spec 1.2 -- the UCUM unit profile (doc/11_bovnar_unit_profiles.md).
    lib.bvn_unit_error_code.restype  = c_int
    lib.bvn_unit_error_code.argtypes = [c_uint8_p, c_uint32]

    lib.bvn_unit_is_profile_only.restype  = c_bool
    lib.bvn_unit_is_profile_only.argtypes = [ValueUnit]

    # The reporting pair. Each unit profile is a COMPILE-TIME switch, so which
    # ones a given libbvnr carries is a property of that build -- and a Python
    # caller who installed a wheel has no other way to find out.
    lib.bvn_unit_profile_count.restype  = c_uint32
    lib.bvn_unit_profile_count.argtypes = []
    lib.bvn_unit_profile_name.restype   = c_char_p
    lib.bvn_unit_profile_name.argtypes  = [c_uint32]

    lib.bvn_unit_to_profile.restype  = c_int32
    lib.bvn_unit_to_profile.argtypes = [c_char_p, ValueUnit, c_char_p, c_size_t]

    lib.bvn_unit_to_ucum.restype  = c_int32
    lib.bvn_unit_to_ucum.argtypes = [ValueUnit, c_char_p, c_size_t]

    lib.bvn_unit_prefix_factor.restype  = c_double
    lib.bvn_unit_prefix_factor.argtypes = [ValueUnit]

    lib.bvn_unit_prefix_exponent.restype  = c_int32
    lib.bvn_unit_prefix_exponent.argtypes = [ValueUnit]

    lib.bvn_units_compatible.restype  = c_bool
    lib.bvn_units_compatible.argtypes = [ValueUnit, ValueUnit]

    # Unit.__eq__ needs this. It is NOT units_compatible: compatible is a
    # statement about dimension (m and km are compatible), equal is a statement
    # about the multiset of (base, exponent, prefix) triples. Only the second
    # can back a __hash__.
    lib.bvn_unit_equal.restype  = c_bool
    lib.bvn_unit_equal.argtypes = [ValueUnit, ValueUnit]

    lib.bvn_units_convertible.restype  = c_bool
    lib.bvn_units_convertible.argtypes = [ValueUnit, ValueUnit]

    lib.bvn_unit_si_normal_form.restype  = c_bool
    lib.bvn_unit_si_normal_form.argtypes = [ValueUnit, P(ValueUnit)]

    lib.bvnr_reader_set_unit_policy.restype  = c_bool
    lib.bvnr_reader_set_unit_policy.argtypes = [c_void_p, P(BvnrUnitPolicy)]

    lib.bvnr_writer_set_unit_policy.restype  = c_bool
    lib.bvnr_writer_set_unit_policy.argtypes = [c_void_p, P(BvnrUnitPolicy)]

    lib.bvn_unit_convert_factor.restype  = c_double
    lib.bvn_unit_convert_factor.argtypes = [
        ValueUnit, ValueUnit, P(c_bool), P(c_bool),
    ]

    lib.bvn_unit_convert_value.restype  = c_bool
    lib.bvn_unit_convert_value.argtypes = [
        c_double, ValueUnit, ValueUnit, P(c_double),
    ]

    lib.bvn_unit_to_si_factor.restype  = c_double
    lib.bvn_unit_to_si_factor.argtypes = [
        ValueUnit, P(c_bool), P(c_double), P(c_bool),
    ]

    # Exact-rational conversion (bovnar_si_units.h). bvn_int_t is opaque here —
    # it is only ever a pointer we received from C (BvnrData.conv.num/den) and
    # hand straight back, so c_void_p is the whole binding it needs. Without
    # these the exact rational a want_unit conversion produces is unreachable
    # from Python, which is the only thing carrying the value when the result
    # has no terminating expansion in the output base.
    lib.bvn_rational_str_bufsize.restype  = c_size_t
    lib.bvn_rational_str_bufsize.argtypes = [c_void_p, c_void_p, c_uint32]

    lib.bvn_rational_to_str.restype  = c_int32
    lib.bvn_rational_to_str.argtypes = [
        c_void_p, c_void_p, c_uint32, c_char_p, c_size_t, P(c_bool),
    ]

    lib.bvn_int_bitlen.restype  = c_int
    lib.bvn_int_bitlen.argtypes = [c_void_p]

    lib.bvn_int_str_bufsize.restype  = c_size_t
    lib.bvn_int_str_bufsize.argtypes = [c_uint32, c_uint32]

    lib.bvn_int_to_str.restype  = c_int32
    lib.bvn_int_to_str.argtypes = [c_void_p, c_char_p, c_size_t, c_uint32]

    # bvn_int.h -- owned copies. A snapshot taken inside a reader callback
    # copies conv.num/conv.den into bignums IT owns, so converted_rational()
    # and converted_in_base() still work once the reader has freed its own.
    lib.bvn_int_alloc.restype  = c_void_p
    lib.bvn_int_alloc.argtypes = []

    lib.bvn_int_free.restype  = None
    lib.bvn_int_free.argtypes = [c_void_p]

    lib.bvn_int_copy.restype  = c_bool
    lib.bvn_int_copy.argtypes = [c_void_p, c_void_p]

    lib.bvn_unit_dimension_vector.restype  = c_bool
    lib.bvn_unit_dimension_vector.argtypes = [ValueUnit, P(c_int32)]

    lib.bvn_unit_reduce.restype  = ValueUnit
    lib.bvn_unit_reduce.argtypes = [ValueUnit, P(c_double), P(c_bool)]

    lib.bvn_exponent_to_int.restype  = c_int32
    lib.bvn_exponent_to_int.argtypes = [c_int]

    lib.bvn_int_to_exponent.restype  = c_int
    lib.bvn_int_to_exponent.argtypes = [c_int32]

    lib.bvn_prefix_unit_valid.restype  = c_bool
    lib.bvn_prefix_unit_valid.argtypes = [ValueUnitPrefix, c_int]

    lib.bvn_validate_identifier.restype  = c_bool
    lib.bvn_validate_identifier.argtypes = [c_char_p]

    lib.bvn_validate_symbol.restype  = c_bool
    lib.bvn_validate_symbol.argtypes = [c_char_p]

    lib.bvn_validate_reference.restype  = c_bool
    lib.bvn_validate_reference.argtypes = [c_char_p]

    lib.bvn_validate_uint_range.restype  = c_bool
    lib.bvn_validate_uint_range.argtypes = [c_char_p, c_uint32, c_uint32]

    lib.bvn_validate_sint_range.restype  = c_bool
    lib.bvn_validate_sint_range.argtypes = [c_char_p, c_uint32, c_uint32]

    lib.bvn_is_special_number_string.restype  = c_bool
    lib.bvn_is_special_number_string.argtypes = [c_char_p]

    lib.bvn_error_to_string.restype  = c_char_p
    lib.bvn_error_to_string.argtypes = [c_int]

    lib.bvn_dom_doc_create.restype  = c_void_p
    lib.bvn_dom_doc_create.argtypes = []

    lib.bvn_dom_doc_destroy.restype  = None
    lib.bvn_dom_doc_destroy.argtypes = [c_void_p]

    lib.bvn_dom_parse.restype  = c_void_p
    lib.bvn_dom_parse.argtypes = [c_void_p, c_uint32]

    lib.bvn_dom_parse_policy.restype  = c_void_p
    lib.bvn_dom_parse_policy.argtypes = [c_void_p, c_uint32, P(BvnrUnitPolicy)]

    lib.bvn_dom_parse_fd_policy.restype  = c_void_p
    lib.bvn_dom_parse_fd_policy.argtypes = [c_int, c_uint64, P(BvnrUnitPolicy)]

    lib.bvn_dom_parse_fd.restype  = c_void_p
    lib.bvn_dom_parse_fd.argtypes = [c_int]

    lib.bvn_dom_doc_get_parse_error.restype  = c_int
    lib.bvn_dom_doc_get_parse_error.argtypes = [c_void_p]

    lib.bvn_dom_lookup.restype  = c_void_p
    lib.bvn_dom_lookup.argtypes = [c_void_p, c_char_p]

    lib.bvn_dom_struct_get.restype  = c_void_p
    lib.bvn_dom_struct_get.argtypes = [c_void_p, c_char_p]

    lib.bvn_dom_array_at.restype  = c_void_p
    lib.bvn_dom_array_at.argtypes = [c_void_p, c_uint32]

    lib.bvn_dom_node_type.restype  = c_int
    lib.bvn_dom_node_type.argtypes = [c_void_p]

    lib.bvn_dom_is_null.restype  = c_bool
    lib.bvn_dom_is_null.argtypes = [c_void_p]

    lib.bvn_dom_get_float.restype  = c_bool
    lib.bvn_dom_get_float.argtypes = [c_void_p, P(c_double)]

    lib.bvn_dom_get_bool.restype  = c_bool
    lib.bvn_dom_get_bool.argtypes = [c_void_p, P(c_bool)]

    lib.bvn_dom_get_string.restype  = c_bool
    lib.bvn_dom_get_string.argtypes = [c_void_p, P(c_void_p), P(c_uint32)]

    lib.bvn_dom_get_symbol.restype  = c_bool
    lib.bvn_dom_get_symbol.argtypes = [c_void_p, P(c_void_p), P(c_uint32)]

    lib.bvn_dom_get_reference.restype  = c_bool
    lib.bvn_dom_get_reference.argtypes = [c_void_p, P(c_void_p), P(c_uint32)]

    lib.bvn_dom_get_octets.restype  = c_bool
    lib.bvn_dom_get_octets.argtypes = [c_void_p, P(c_void_p), P(c_uint32)]

    lib.bvn_dom_get_value_type.restype  = ValueTypeSpec
    lib.bvn_dom_get_value_type.argtypes = [c_void_p]

    # spec 1.1 — verbatim ISO sub-second digits of a datetime literal (NULL when
    # the value has none); the node owns the storage, so the result is read-only.
    lib.bvn_dom_get_datetime_fraction.restype  = c_char_p
    lib.bvn_dom_get_datetime_fraction.argtypes = [c_void_p, P(c_uint32)]
    lib.bvn_dom_get_unit_source.restype  = c_char_p
    lib.bvn_dom_get_unit_source.argtypes = [c_void_p, P(c_uint32)]

    lib.bvn_dom_get_unit.restype  = ValueUnit
    lib.bvn_dom_get_unit.argtypes = [c_void_p]

    lib.bvn_dom_get_unit_string.restype  = c_int32
    lib.bvn_dom_get_unit_string.argtypes = [c_void_p, c_char_p, c_size_t]

    lib.bvn_dom_get_value_in_base_units.restype  = c_double
    lib.bvn_dom_get_value_in_base_units.argtypes = [c_void_p]

    lib.bvn_dom_struct_count.restype  = c_uint32
    lib.bvn_dom_struct_count.argtypes = [c_void_p]

    lib.bvn_dom_array_count.restype  = c_uint32
    lib.bvn_dom_array_count.argtypes = [c_void_p]

    lib.bvn_dom_array_dims.restype  = c_uint32
    lib.bvn_dom_array_dims.argtypes = [c_void_p]

    lib.bvn_dom_struct_entries.restype  = P(BvnDomEntry)
    lib.bvn_dom_struct_entries.argtypes = [c_void_p]

    lib.bvn_dom_doc_entries.restype  = P(BvnDomEntry)
    lib.bvn_dom_doc_entries.argtypes = [c_void_p]

    lib.bvn_dom_doc_count.restype  = c_uint32
    lib.bvn_dom_doc_count.argtypes = [c_void_p]

    lib.bvn_dom_get_i64.restype  = c_bool
    lib.bvn_dom_get_i64.argtypes = [c_void_p, P(c_int64)]

    lib.bvn_dom_get_u64.restype  = c_bool
    lib.bvn_dom_get_u64.argtypes = [c_void_p, P(c_uint64)]

    lib.bvn_dom_get_i32.restype  = c_bool
    lib.bvn_dom_get_i32.argtypes = [c_void_p, P(c_int32)]

    lib.bvn_dom_get_u32.restype  = c_bool
    lib.bvn_dom_get_u32.argtypes = [c_void_p, P(c_uint32)]

    lib.bvn_dom_get_i16.restype  = c_bool
    lib.bvn_dom_get_i16.argtypes = [c_void_p, P(c_int16)]

    lib.bvn_dom_get_u16.restype  = c_bool
    lib.bvn_dom_get_u16.argtypes = [c_void_p, P(c_uint16)]

    lib.bvn_dom_get_i8.restype   = c_bool
    lib.bvn_dom_get_i8.argtypes  = [c_void_p, P(c_int8)]

    lib.bvn_dom_get_u8.restype   = c_bool
    lib.bvn_dom_get_u8.argtypes  = [c_void_p, P(c_uint8)]

    lib.bvn_dom_int_to_str.restype  = c_void_p
    lib.bvn_dom_int_to_str.argtypes = [c_void_p, c_uint32]

    lib.bvn_dom_free_string.restype  = None
    lib.bvn_dom_free_string.argtypes = [c_void_p]

    # ── bovnar_stream.h: framing, multiplexing, document-in-document ──────────
    lib.bvnr_frame_write.restype  = c_bool
    lib.bvnr_frame_write.argtypes = [P(BvnrSink), c_void_p, c_uint64]

    lib.bvnr_doc_stream_read.restype  = c_bool
    lib.bvnr_doc_stream_read.argtypes = [
        P(BvnrSource), P(BvnrDocStreamOpts), P(c_uint64),
    ]

    lib.bvnr_mux_begin.restype  = c_bool
    lib.bvnr_mux_begin.argtypes = [c_void_p, c_char_p]

    lib.bvnr_mux_send.restype  = c_bool
    lib.bvnr_mux_send.argtypes = [c_void_p, c_uint64, c_void_p, c_uint64]

    lib.bvnr_mux_end.restype  = c_bool
    lib.bvnr_mux_end.argtypes = [c_void_p]

    lib.bvnr_demux_create.restype  = c_void_p
    lib.bvnr_demux_create.argtypes = [MUX_ON_MSG_FUNC, c_void_p, c_uint64]

    lib.bvnr_demux_destroy.restype  = None
    lib.bvnr_demux_destroy.argtypes = [c_void_p]

    lib.bvnr_demux_set_key.restype  = c_bool
    lib.bvnr_demux_set_key.argtypes = [c_void_p, c_char_p]

    lib.bvnr_demux_on_event.restype  = c_bool
    lib.bvnr_demux_on_event.argtypes = [c_void_p, c_int, P(BvnrData)]

    lib.bvnr_demux_error.restype  = c_int
    lib.bvnr_demux_error.argtypes = [c_void_p]

    lib.bvnr_embed_document.restype  = c_bool
    lib.bvnr_embed_document.argtypes = [c_void_p, c_char_p, c_void_p, c_uint64]

    lib.bvnr_parse_embedded.restype  = c_bool
    lib.bvnr_parse_embedded.argtypes = [c_void_p, c_uint64, P(BvnrReadFlags)]

    # ── bvn_float.h: arbitrary-precision decimal float + IEEE/fixed encoders ──
    # The bvn_float_t* handle is treated as an opaque c_void_p (allocated by
    # bvn_float_alloc, released by bvn_float_free); no struct layout is mirrored.
    c_u32_4 = c_uint32 * 4
    c_u32_8 = c_uint32 * 8
    lib.bvn_float_alloc.restype  = c_void_p
    lib.bvn_float_alloc.argtypes = [c_uint32]
    lib.bvn_float_free.restype  = None
    lib.bvn_float_free.argtypes = [c_void_p]
    lib.bvn_float_from_str.restype  = c_bool
    lib.bvn_float_from_str.argtypes = [c_void_p, c_char_p, c_uint32]
    lib.bvn_float_to_str.restype  = c_int32
    lib.bvn_float_to_str.argtypes = [c_void_p, c_char_p, c_size_t, c_uint32]
    lib.bvn_float_str_bufsize.restype  = c_size_t
    lib.bvn_float_str_bufsize.argtypes = [c_uint32, c_uint32]
    lib.bvn_float_to_double.restype  = c_bool
    lib.bvn_float_to_double.argtypes = [c_void_p, P(c_double)]
    for _p in ('nan', 'inf', 'zero', 'neg'):
        fn = getattr(lib, 'bvn_float_is_' + _p)
        fn.restype = c_bool
        fn.argtypes = [c_void_p]
    # string -> IEEE-754 binary interchange bits, single correctly-rounded step
    lib.bvn_float_strtoieee_bin.restype  = None
    lib.bvn_float_strtoieee_bin.argtypes = [
        c_char_p, c_uint32, c_uint32, c_uint32, c_int32, P(c_uint32), c_int]
    # IEEE binary16/32/64 (scalar) and binary128/256 (4 / 8 uint32 words)
    lib.bvn_float_to_bin16.restype  = None
    lib.bvn_float_to_bin16.argtypes = [c_void_p, P(c_uint16)]
    lib.bvn_float_to_bin32.restype  = None
    lib.bvn_float_to_bin32.argtypes = [c_void_p, P(c_uint32)]
    lib.bvn_float_to_bin64.restype  = None
    lib.bvn_float_to_bin64.argtypes = [c_void_p, P(c_uint64)]
    lib.bvn_float_from_bin16.restype  = c_bool
    lib.bvn_float_from_bin16.argtypes = [c_void_p, c_uint16]
    lib.bvn_float_from_bin32.restype  = c_bool
    lib.bvn_float_from_bin32.argtypes = [c_void_p, c_uint32]
    lib.bvn_float_from_bin64.restype  = c_bool
    lib.bvn_float_from_bin64.argtypes = [c_void_p, c_uint64]
    lib.bvn_float_to_bin128.restype  = None
    lib.bvn_float_to_bin128.argtypes = [c_void_p, c_u32_4]
    lib.bvn_float_to_bin256.restype  = None
    lib.bvn_float_to_bin256.argtypes = [c_void_p, c_u32_8]
    lib.bvn_float_from_bin128.restype  = c_bool
    lib.bvn_float_from_bin128.argtypes = [c_void_p, c_u32_4]
    lib.bvn_float_from_bin256.restype  = c_bool
    lib.bvn_float_from_bin256.argtypes = [c_void_p, c_u32_8]
    # IEEE decimal interchange dec16/32/64 (scalar) and dec128/256 (word arrays)
    lib.bvn_float_strtodec.restype  = c_bool
    lib.bvn_float_strtodec.argtypes = [c_char_p, c_uint32, P(c_uint32), c_int]
    lib.bvn_float_to_dec16.restype  = None
    lib.bvn_float_to_dec16.argtypes = [c_void_p, P(c_uint16)]
    lib.bvn_float_to_dec32.restype  = None
    lib.bvn_float_to_dec32.argtypes = [c_void_p, P(c_uint32)]
    lib.bvn_float_to_dec64.restype  = None
    lib.bvn_float_to_dec64.argtypes = [c_void_p, P(c_uint64)]
    lib.bvn_float_to_dec128.restype  = None
    lib.bvn_float_to_dec128.argtypes = [c_void_p, c_u32_4]
    lib.bvn_float_to_dec256.restype  = None
    lib.bvn_float_to_dec256.argtypes = [c_void_p, c_u32_8]
    lib.bvn_float_from_dec16.restype  = c_bool
    lib.bvn_float_from_dec16.argtypes = [c_void_p, c_uint16]
    lib.bvn_float_from_dec32.restype  = c_bool
    lib.bvn_float_from_dec32.argtypes = [c_void_p, c_uint32]
    lib.bvn_float_from_dec64.restype  = c_bool
    lib.bvn_float_from_dec64.argtypes = [c_void_p, c_uint64]
    lib.bvn_float_from_dec128.restype  = c_bool
    lib.bvn_float_from_dec128.argtypes = [c_void_p, c_u32_4]
    lib.bvn_float_from_dec256.restype  = c_bool
    lib.bvn_float_from_dec256.argtypes = [c_void_p, c_u32_8]
    # fixed-point Q-format fix16/32/64 (scalar) and fix128/256 (word arrays)
    lib.bvn_float_to_fix16.restype  = c_int16
    lib.bvn_float_to_fix16.argtypes = [c_void_p, c_uint32]
    lib.bvn_float_to_fix32.restype  = c_int32
    lib.bvn_float_to_fix32.argtypes = [c_void_p, c_uint32]
    lib.bvn_float_to_fix64.restype  = c_int64
    lib.bvn_float_to_fix64.argtypes = [c_void_p, c_uint32]
    lib.bvn_float_to_fix128.restype  = None
    lib.bvn_float_to_fix128.argtypes = [c_void_p, c_uint32, c_u32_4]
    lib.bvn_float_to_fix256.restype  = None
    lib.bvn_float_to_fix256.argtypes = [c_void_p, c_uint32, c_u32_8]
    lib.bvn_float_from_fix16.restype  = c_bool
    lib.bvn_float_from_fix16.argtypes = [c_void_p, c_int16, c_uint32]
    lib.bvn_float_from_fix32.restype  = c_bool
    lib.bvn_float_from_fix32.argtypes = [c_void_p, c_int32, c_uint32]
    lib.bvn_float_from_fix64.restype  = c_bool
    lib.bvn_float_from_fix64.argtypes = [c_void_p, c_int64, c_uint32]
    lib.bvn_float_from_fix128.restype  = c_bool
    lib.bvn_float_from_fix128.argtypes = [c_void_p, c_u32_4, c_uint32]
    lib.bvn_float_from_fix256.restype  = c_bool
    lib.bvn_float_from_fix256.argtypes = [c_void_p, c_u32_8, c_uint32]
    lib.bvn_float_fix_in_range.restype  = c_bool
    lib.bvn_float_fix_in_range.argtypes = [c_void_p, c_uint32, c_uint32]
    lib.bvn_float_str_fits_fix.restype  = c_bool
    lib.bvn_float_str_fits_fix.argtypes = [c_char_p, c_uint32, c_uint32, c_uint32]
