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
"""numpy <-> bovnar array bridge.

    to_numpy(src)               bovnar array -> numpy ndarray (+ optional unit)
    to_pint_array(src)          bovnar array -> pint Quantity (ndarray + unit)
    from_numpy(writer, key, a)  ndarray -> bovnar (via a Writer)
    from_pint_array(w, key, q)  pint Quantity -> bovnar (via a Writer)
    array_to_bvnr(key, a)       ndarray -> bovnar bytes (convenience)

*src* for the read side is either a DomNode for an ARRAY (random-access, from
``dom_parse``) or the nested list a typed ``loads(..., typed=True)`` produces.
Both ``/``-rows and bracket nesting collapse to the same ndarray shape.

Design notes:
  * Strict by default — an array that mixes element dtypes or units raises
    (pass an explicit ``dtype=`` to coerce); ragged data raises unless
    ``dtype=object``.
  * The unit is a whole-array property (numpy has one dtype per array): it is
    returned alongside the data (``return_unit``/``to_pint_array``), never baked
    into element values.
  * Prefixes ride in the unit, not the data (see _pint_bridge), so a wrapped
    array is never silently rescaled.

numpy (and, for the pint helpers, pint) are optional dependencies imported
lazily; importing bovnar never requires them.
"""
from __future__ import annotations

from decimal import Decimal
from fractions import Fraction

from .enums import ValueTypeFamily as F
from .dom import DomNode, DomType
from .quantity import Quantity
from .structs import ValueUnit, make_type_spec, make_unit_none
from .enums import PrefixSystem
from .exceptions import BovnarArgumentError
from .unit import Unit

__all__ = ['to_numpy', 'to_pint_array', 'from_numpy', 'from_pint_array',
           'array_to_bvnr']


def _np():
    try:
        import numpy as np
    except ImportError as e:                       # pragma: no cover
        raise BovnarArgumentError(
            "numpy is required for the numpy bridge; `pip install numpy`") from e
    return np


# --------------------------------------------------------------------------- #
#  dtype <-> bovnar (family, width)
# --------------------------------------------------------------------------- #
_INT_DTYPE = {
    (int(F.UINT), 8): 'uint8',  (int(F.UINT), 16): 'uint16',
    (int(F.UINT), 32): 'uint32', (int(F.UINT), 64): 'uint64',
    (int(F.SINT), 8): 'int8',   (int(F.SINT), 16): 'int16',
    (int(F.SINT), 32): 'int32',  (int(F.SINT), 64): 'int64',
}
# numpy's native binary floats only reach float64 here: np.float128 is x86
# 80-bit extended (NOT IEEE binary128), so mapping <float:128> onto it would
# misrepresent the value. bovnar itself stores these widths at full precision
# (the verbatim decimal literal), so the lossless path is loads(typed=True),
# whose Quantity decodes to an exact decimal.Decimal (see _is_exact_decimal_*).
# This native-dtype table is only the lossy/convenience path.
_FLOAT_DTYPE = {16: 'float16', 32: 'float32', 64: 'float64'}

# families that can legitimately carry a physical unit; a leaf of one of these
# with *no* unit is a "bare" numeric value (datetime/bool/utf8 never carry one).
_NUMERIC_FAMILIES = frozenset((
    int(F.UINT), int(F.SINT), int(F.FLOAT), int(F.FLOAT_FIX), int(F.FLOAT_DEC)))


def _family_width_dtype(family: int, width: int, base: int = 0):
    """bovnar (family,width[,base]) -> numpy dtype name, or None to infer (PLAIN).

    ``base`` is the datetime epoch index (0 = unix); it is ignored for every
    other family.
    """
    family = int(family)
    width = int(width) or 64                    # bovnar default width is 64
    if family == int(F.DATETIME):
        # numpy datetime64 counts seconds from the unix epoch. The unix-epoch
        # carrier maps directly; any other epoch (tai, gps, …) is on a
        # different origin/timescale, so mapping it to datetime64 would
        # silently mis-date every value — refuse and point at the int escape.
        if int(base) == 0:
            return 'datetime64[s]'
        raise BovnarArgumentError(
            "datetime epoch index {} is not the unix epoch; numpy datetime64 is "
            "unix-based, so this would mis-date the values — pass dtype='int64' "
            "for the raw epoch seconds, or convert to unix first".format(int(base)))
    if family == int(F.BOOL):
        return 'bool'
    if family == int(F.UTF8):
        return 'str'
    if family in (int(F.UINT), int(F.SINT)):
        dt = _INT_DTYPE.get((family, width))
        if dt is None:
            raise BovnarArgumentError(
                f"integer width {width} has no numpy dtype (bigints need dtype=object)")
        return dt
    if family == int(F.FLOAT):
        dt = _FLOAT_DTYPE.get(width)
        if dt is None:
            raise BovnarArgumentError(
                f"float:{width} has no lossless numpy dtype; read it with "
                f"loads(typed=True) for exact Decimals, or pass dtype='float64'")
        return dt
    if family in (int(F.FLOAT_FIX), int(F.FLOAT_DEC)):
        # decimal/fixed-point semantics have no exact binary-float dtype. On the
        # typed path these decode to exact Decimals (handled in _leaf before this
        # is reached); this branch is only the DOM/native path, where the value
        # is already a lossy C double — so require the explicit float64 opt-in.
        raise BovnarArgumentError(
            f"{F(family).name.lower()} has no lossless numpy dtype; read it with "
            f"loads(typed=True) for exact Decimals, or pass dtype='float64'")
    if family == int(F.PLAIN):
        return None                              # infer from the value / DOM node type
    raise BovnarArgumentError(f"value family {family} cannot map to a numpy dtype")


def _dtype_to_family_width(dt):
    """numpy dtype -> (ValueTypeFamily, width-in-bits)."""
    kind, bits = dt.kind, dt.itemsize * 8
    if kind == 'u':
        return F.UINT, bits
    if kind == 'i':
        return F.SINT, bits
    if kind == 'f':
        if bits > 64:
            # np.float128 is x86 80-bit extended, NOT IEEE binary128, and its
            # value is ambiguous to map; np.asarray(...).tolist() also yields
            # longdouble objects the element writer does not accept. For an exact
            # wide/decimal float, convert to Decimal and pass
            # float_format=('float',128) (or 'float_dec'/'float_fix').
            raise BovnarArgumentError(
                f"a {dt.name} array has no faithful bovnar mapping; convert to "
                f"Decimal and use float_format=('float',128), or cast to float64")
        return F.FLOAT, bits
    if kind == 'b':
        return F.BOOL, 0
    if kind == 'S':
        # A bytes array was mapped to utf8 here and then failed element by
        # element, as "Cannot serialise array element of type 'bytes'". Which
        # text the bytes are is the caller's knowledge, not something to guess
        # at the first element; say so before anything is written.
        raise BovnarArgumentError(
            f"a bytes array ({dt.name}) has no bovnar mapping: decode it to "
            f"text first, e.g. arr.astype('U') or np.char.decode(arr, 'utf-8')")
    if kind == 'U':
        return F.UTF8, 0
    if kind == 'M':                               # datetime64[*] -> unix epoch
        return F.DATETIME, 64
    raise BovnarArgumentError(
        f"numpy dtype {dt!r} (kind {kind!r}) has no bovnar equivalent")


# --------------------------------------------------------------------------- #
#  read side: bovnar -> ndarray
# --------------------------------------------------------------------------- #
def _dom_rows(node: DomNode):
    """Children of an ARRAY DomNode as a nested list, undoing /-row flattening."""
    kids = [node[i] for i in range(len(node))]
    if not kids:
        return []
    dims = node.array_dims()
    if dims > 1:                                  # /-multirow: split flat children
        rl = len(kids) // dims
        return [kids[r * rl:(r + 1) * rl] for r in range(dims)]
    return kids


def _walk(obj, acc):
    """Recurse the source, recording leaf dtypes/units in *acc*, return values."""
    if isinstance(obj, (list, tuple)):
        return [_walk(x, acc) for x in obj]
    if isinstance(obj, DomNode) and obj.dom_type == DomType.ARRAY:
        return _walk(_dom_rows(obj), acc)
    return _leaf(obj, acc)


def _gather_dtype(acc, family, width, base=0):
    """Best-effort dtype hint for inference (dtype=None).

    A width with no native numpy dtype — a >64-bit integer (bigint) or a float
    width other than 16/32/64 — is *recorded* as unmappable rather than
    raised here, so an explicit ``dtype=`` (e.g. ``object`` for bigints, or
    ``float``) can still coerce the array. Strict inference reports it later in
    ``_extract`` only when no dtype was supplied. A non-unix datetime epoch is
    likewise recorded (it maps only with an explicit dtype='int64').
    """
    try:
        return _family_width_dtype(family, width, base)
    except BovnarArgumentError as e:
        acc['unmappable'] = True
        acc.setdefault('unmappable_reason', str(e))
        return None


# Families whose value cannot be held in a native numpy float without loss:
# decimal floats, fixed-point, and the wide binary floats (float:128 and wider).
# On the typed-load path (Quantity, which keeps the verbatim literal) these
# decode to exact decimal.Decimal in an object array; pass dtype='float64' to
# opt into the lossy native conversion instead.
def _is_exact_decimal_family(family: int, width: int) -> bool:
    if family in (int(F.FLOAT_DEC), int(F.FLOAT_FIX)):
        return True
    # any binary float wider than float64 (128/256/512/...) — numpy has no
    # lossless native dtype, so it decodes to an exact Decimal from the literal.
    return family == int(F.FLOAT) and int(width or 64) not in (16, 32, 64)


def _is_exact_decimal_quantity(q) -> bool:
    return _is_exact_decimal_family(int(q.vtype.family), int(q.vtype.width or 64))


def _unit_key(unit):
    """Order-insensitive identity of a unit, for deciding array uniformity.

    Unit multiplication commutes, so ``m·s`` and ``s·m`` are ONE unit — that is
    why ``bvn_unit_equal`` matches components as a multiset, and the spec
    requires logically equivalent notations to compare equal (unit-system
    reference §2.2). The canonical STRING does not carry that property: the
    formatter splits numerator from denominator but preserves source order
    within each group, so ``m·s`` and ``s·m`` serialise differently. Keying
    uniqueness on the string therefore rejected an array that merely spelled one
    unit two ways, with "array mixes units ['m·s', 's·m']".

    Mirrors bvn_unit_component_identical: base, exponent, prefix system, and the
    prefix id from whichever arm of the union that system selects.
    """
    n = min(int(unit.num_components), len(unit.components))
    out = []
    for i in range(n):
        c = unit.components[i]
        sysid = int(c.prefix.system)
        pid = int(c.prefix.id.iec) if sysid == int(PrefixSystem.IEC) \
            else int(c.prefix.id.si)
        if not pid:
            sysid = 0       # no prefix is no prefix in either system
        out.append((int(c.base), int(c.exponent), sysid, pid))
    return tuple(sorted(out))


def _record_unit(acc, unit, display):
    """Remember one unit under its order-insensitive key, keeping the FIRST
    spelling seen as the one to report."""
    acc['units'].setdefault(_unit_key(unit), (unit, display))


def _leaf(obj, acc):
    if obj is None:
        acc['null'] = True
        return None
    if isinstance(obj, Quantity):
        if _is_exact_decimal_quantity(obj):
            # lossless: an object array of exact Decimals (see note above).
            acc['dtypes'].add('object')
            us = obj.unit_str
            if us:
                _record_unit(acc, obj.unit.raw, us)
            else:
                acc['bare_numeric'] = True
            val = obj.decimal                   # exact Decimal, or None for a null
            if val is None:
                acc['null'] = True
            return val
        dt = _gather_dtype(acc, obj.vtype.family, obj.vtype.width, obj.vtype.base)
        if dt:
            acc['dtypes'].add(dt)
        us = obj.unit_str
        if us:
            _record_unit(acc, obj.unit.raw, us)
        elif int(obj.vtype.family) in _NUMERIC_FAMILIES:
            acc['bare_numeric'] = True
        val = obj.value                           # a typed null decodes to None
        if val is None:
            acc['null'] = True
        elif type(val) is int and val < 0:
            acc['has_negative'] = True
        return val
    if isinstance(obj, DomNode):                  # a non-array leaf node
        if int(obj.value_type.family) == int(F.PLAIN):
            dt = {DomType.INT: 'int64', DomType.FLOAT: 'float64',
                  DomType.BOOL: 'bool', DomType.STRING: 'str'}.get(obj.dom_type)
        else:
            if _is_exact_decimal_family(int(obj.value_type.family),
                                        int(obj.value_type.width or 64)):
                # The DOM stores every float as a C double, so a decimal / wide
                # binary float has ALREADY lost precision here — unlike the typed
                # (Quantity) path, which keeps the verbatim literal. Record it so
                # an exactness-implying dtype=object errors (below) instead of
                # silently handing back lossy doubles in an object array.
                acc['dom_inexact_float'] = (
                    f"{F(int(obj.value_type.family)).name.lower()} read from a "
                    "DOM node is a lossy C double, not an exact value; read it "
                    "with loads(typed=True) for exact Decimals, or pass "
                    "dtype='float64' for the lossy double")
            dt = _gather_dtype(acc, obj.value_type.family, obj.value_type.width,
                               obj.value_type.base)
        if dt:
            acc['dtypes'].add(dt)
        # DomNode.unit_str is '' for a value with no unit, like
        # Quantity.unit_str (it used to return the literal 'no_unit').
        us = obj.unit_str
        if us:
            # DomNode.unit is still a bare ValueUnit; only Quantity.unit became a
            # Unit object. _record_unit wants the struct, so this one is already it.
            _record_unit(acc, obj.unit, us)
        elif obj.dom_type in (DomType.INT, DomType.FLOAT):
            acc['bare_numeric'] = True
        val = obj.to_python()
        if val is None:
            acc['null'] = True
        elif type(val) is int and val < 0:
            acc['has_negative'] = True
        return val
    if isinstance(obj, bool):
        acc['dtypes'].add('bool'); return obj
    if isinstance(obj, int):
        # A raw Python int is unbounded: pick the narrowest native 64-bit dtype
        # that holds it (preferring int64, then uint64 for the 2**63..2**64-1
        # band, mirroring bovnar's uint64 default), and route a true bigint to
        # the unmappable/object path — exactly as a >64-bit *typed* element is
        # handled. Tagging every int 'int64' made numpy raise a bare
        # OverflowError for any value past 2**63-1 instead of a clean message.
        acc['bare_numeric'] = True            # a raw Python number is dimensionless
        if obj < 0:
            acc['has_negative'] = True
        if -(2 ** 63) <= obj < 2 ** 63:
            acc['dtypes'].add('int64')
        elif 0 <= obj < 2 ** 64:
            acc['dtypes'].add('uint64')
        else:
            acc['unmappable'] = True
            acc.setdefault('unmappable_reason',
                           f"integer {obj} exceeds 64-bit range; pass dtype=object")
        return obj
    if isinstance(obj, float):
        acc['dtypes'].add('float64'); acc['bare_numeric'] = True; return obj
    if isinstance(obj, str):
        acc['dtypes'].add('str'); return obj
    raise BovnarArgumentError(f"unsupported array element type {type(obj).__name__}")


def _extract(src, dtype):
    """Core: src -> (ndarray, ValueUnit, unit_str)."""
    np = _np()
    if not isinstance(src, (DomNode, list, tuple)):
        raise BovnarArgumentError(
            "to_numpy expects a DomNode array or the nested list from "
            "loads(..., typed=True)")
    acc = {'dtypes': set(), 'units': {}, 'null': False, 'unmappable': False,
           'has_negative': False, 'bare_numeric': False}
    values = _walk(src, acc)

    if dtype is None:
        if acc['unmappable']:
            # A bigint / unsupported-width element with no native numpy dtype.
            # Strict by default: report it, but the message points at the
            # explicit-dtype escape hatch (which now works — see _gather_dtype).
            raise BovnarArgumentError(acc.get(
                'unmappable_reason',
                "array has an element with no native numpy dtype; "
                "pass dtype=object"))
        dts = acc['dtypes']
        if not dts:
            resolved = None
        elif len(dts) == 1:
            resolved = next(iter(dts))
        elif dts == {'int64', 'uint64'} and not acc['has_negative']:
            # int64 (small/signed leaves) mixed with uint64 (2**63..2**64-1
            # leaves) but nothing is negative, so every value fits uint64 — the
            # same widening bovnar applies to a bare non-negative integer.
            resolved = 'uint64'
        else:
            raise BovnarArgumentError(
                f"array mixes element dtypes {sorted(dts)}; pass dtype= to coerce")
    else:
        resolved = dtype

    np_dtype = None
    if resolved is not None:
        try:
            np_dtype = np.dtype(resolved)
        except TypeError as e:
            raise BovnarArgumentError(f"unknown dtype {resolved!r}: {e}") from e

    # dtype=object on a DOM decimal / wide-binary float would silently fill the
    # object array with lossy C doubles (the DOM keeps no exact value), defeating
    # the point of an object array. Match the dtype=None path and refuse, pointing
    # at the lossless typed route. (dtype='float64' is the explicit lossy opt-in
    # and still works; a bigint object array is exact and never sets this flag.)
    if (np_dtype is not None and np_dtype.kind == 'O'
            and acc.get('dom_inexact_float')):
        raise BovnarArgumentError(acc['dom_inexact_float'])

    # A null element survives only in a dtype that has a null/NaN slot: float
    # (NaN), complex, datetime64/timedelta64 (NaT) or object (None). For any
    # other dtype numpy coerces silently and corrupts the value — None becomes
    # False in a bool array and the literal string 'None' in a str array — so
    # reject it rather than emit garbage (strict by default; the int message is
    # kept since float/object is the usual remedy for a numeric column).
    if (acc['null'] and np_dtype is not None
            and np_dtype.kind not in ('f', 'c', 'M', 'm', 'O')):
        kind_word = ('an integer' if np_dtype.kind in ('i', 'u')
                     else f"a {np_dtype.name}")
        raise BovnarArgumentError(
            f"null element(s) cannot fill {kind_word} array "
            f"(it has no null representation); pass dtype=float or dtype=object")

    try:
        arr = np.array(values, dtype=np_dtype)
    except ValueError as e:                       # ragged / inhomogeneous
        raise BovnarArgumentError(
            f"cannot build a rectangular array (ragged data?): {e}; "
            f"pass dtype=object to keep it ragged") from e
    except TypeError as e:                         # e.g. None into an int array
        raise BovnarArgumentError(
            f"cannot build a {np_dtype} array from these values "
            f"(null element?): {e}") from e
    except OverflowError as e:                      # value outside the dtype range
        raise BovnarArgumentError(
            f"a value is out of range for a {np_dtype} array: {e}; "
            f"pass dtype=object or a wider dtype") from e

    # Keyed order-insensitively (see _unit_key), so two spellings of one unit
    # are one entry; the display strings are what the messages below show.
    units = [display for _unit, display in acc['units'].values()]
    if len(units) > 1:
        raise BovnarArgumentError(
            f"array mixes units {units}; a numpy array carries a single unit")
    if units and acc['bare_numeric']:
        # Some numeric elements carry a unit and others are dimensionless; a
        # numpy array holds one whole-array unit, so silently labelling the bare
        # values with {units[0]} would invent a dimension they do not have.
        raise BovnarArgumentError(
            f"array mixes a unit ({units[0]}) with dimensionless numeric "
            f"value(s); give every element the same unit, or drop it")
    if units:
        unit, display = next(iter(acc['units'].values()))
        return arr, unit, display
    return arr, make_unit_none(), ''


def to_numpy(src, *, dtype=None, return_unit: bool = False):
    """Convert a bovnar array (DomNode or typed nested list) to an ndarray.

    With return_unit=True, returns (ndarray, unit_str) where unit_str is the
    bovnar canonical unit ('' if dimensionless).
    """
    arr, _vu, unit_str = _extract(src, dtype)
    return (arr, unit_str) if return_unit else arr


def to_pint_array(src, *, dtype=None, ureg=None):
    """Convert a bovnar array to a pint Quantity (ndarray data + its unit)."""
    arr, vu, _us = _extract(src, dtype)
    # pint has no notion of an absolute timestamp, so wrapping a datetime64
    # array would silently present it as a *dimensionless* quantity — a
    # meaningless, misleading result (the write side likewise refuses a unit on
    # a datetime). Use to_numpy for the datetime64 array, or pass dtype='int64'
    # here to get the raw epoch-seconds as an explicit dimensionless count.
    if arr.dtype.kind in ('M', 'm'):
        raise BovnarArgumentError(
            "a datetime array has no pint unit; use to_numpy() for the "
            "datetime64 array, or pass dtype='int64' for the raw epoch seconds")
    from ._pint_bridge import to_pint
    return to_pint(arr, vu, ureg=ureg)


# --------------------------------------------------------------------------- #
#  write side: ndarray -> bovnar
# --------------------------------------------------------------------------- #
def _resolve_unit_arg(unit):
    """unit arg (None|str|ValueUnit|pint Unit/Quantity) -> ValueUnit or None."""
    if unit is None:
        return None
    if isinstance(unit, Unit):                    # what Quantity.unit hands out
        return unit.raw
    if isinstance(unit, ValueUnit):
        return unit
    if isinstance(unit, str):
        from . import parse_unit
        return parse_unit(unit)
    # otherwise it must be a pint Unit (has .dimensionality) or Quantity (.units).
    # Guard explicitly so a wrong type gives a clear message instead of a cryptic
    # AttributeError from deep inside the pint bridge.
    if not (hasattr(unit, 'dimensionality') or hasattr(unit, 'units')):
        raise BovnarArgumentError(
            f"unit must be a unit string, Unit, ValueUnit, or pint "
            f"Unit/Quantity, not {type(unit).__name__}")
    from ._pint_bridge import from_pint_unit
    return from_pint_unit(unit)


_FLOAT_FAMILY_NAMES = {'float': F.FLOAT, 'float_dec': F.FLOAT_DEC,
                       'float_fix': F.FLOAT_FIX}


def _resolve_float_format(float_format):
    """A float_format spec -> (ValueTypeFamily, width, base). Default float_dec:64."""
    if float_format is None:
        return F.FLOAT_DEC, 64, 0
    if not isinstance(float_format, (tuple, list)) or len(float_format) < 2:
        raise BovnarArgumentError(
            "float_format must be (family, width) or (family, width, frac)")
    fam, width = float_format[0], float_format[1]
    frac = float_format[2] if len(float_format) > 2 else None
    if isinstance(fam, str):
        if fam not in _FLOAT_FAMILY_NAMES:
            raise BovnarArgumentError(f"unknown float family {fam!r}")
        fam = _FLOAT_FAMILY_NAMES[fam]
    fam = F(int(fam))
    if fam not in (F.FLOAT, F.FLOAT_DEC, F.FLOAT_FIX):
        raise BovnarArgumentError(
            "float_format family must be float, float_dec, or float_fix")
    from .quantity import _validate_float_width
    _validate_float_width(fam, width)
    if fam == F.FLOAT_FIX:
        if frac is None:
            raise BovnarArgumentError(
                "float_fix float_format needs frac: ('float_fix', width, frac)")
        return fam, int(width), int(frac)
    return fam, int(width), 0


def _from_exact_float_array(writer, key, arr, unit, float_format) -> None:
    """Write an object array of Decimal/Fraction (or any numbers) as an exact
    decimal/fixed/binary float array of the chosen *float_format*."""
    fam, width, base = _resolve_float_format(float_format)
    vu = _resolve_unit_arg(unit)
    rows = arr.tolist()
    if fam == F.FLOAT_FIX:                          # validate every element fits
        from ._ffi import get_library
        from .quantity import _exact_number_text
        lib = get_library()

        def _check(x):
            if isinstance(x, (list, tuple)):
                for y in x:
                    _check(y)
            elif x is not None:
                txt = _exact_number_text(x).encode('ascii')
                if not lib.bvn_float_str_fits_fix(txt, 10, width, base):
                    raise BovnarArgumentError(
                        f"{x} does not fit a signed float_fix:{width},q{base}")

        _check(rows)
    from . import write_array
    write_array(writer, key, rows, vt=make_type_spec(fam, width, base), vu=vu)


def from_numpy(writer, key: str, arr, *, unit=None, float_format=None) -> None:
    """Write *arr* (an ndarray) as a bovnar array into *writer* under *key*.

    *unit* may be a bovnar unit string, a ValueUnit, or a pint Unit/Quantity.
    Units apply to numeric arrays only.

    *float_format* — ``(family, width[, frac])`` with family ``'float'``,
    ``'float_dec'`` (the default for an object array of Decimals), or
    ``'float_fix'`` — writes the values as an exact decimal/fixed/wide-binary
    float, bypassing the lossy native float path. Required to disambiguate an
    object array that is not purely Decimal/Fraction.
    """
    np = _np()
    # np.asarray on a masked array silently drops the mask and exposes the raw
    # data under the masked cells, so a missing entry would serialize as whatever
    # value happens to sit there. Refuse it (mirrors the NaT and null guards)
    # rather than emit a fabricated value; the caller fills or drops the mask.
    if isinstance(arr, np.ma.MaskedArray) and np.ma.getmaskarray(arr).any():
        raise BovnarArgumentError(
            "a masked array would serialize its masked entries as their "
            "underlying values; fill it first (e.g. arr.filled(...)) or drop the mask")
    arr = np.asarray(arr)
    if arr.ndim == 0:
        raise BovnarArgumentError(
            "from_numpy needs a 1-D+ array; write a 0-D value with the scalar API")

    # Exact decimal/fixed/wide-binary path: explicit float_format always wins;
    # an object array of pure Decimal/Fraction defaults to float_dec:64. Any
    # other object array (bigints, mixed) is ambiguous and must say float_format
    # rather than be silently coerced to a lossy float_dec.
    if float_format is not None:
        _from_exact_float_array(writer, key, arr, unit, float_format)
        return
    if arr.dtype == object:
        flat = arr.ravel().tolist()
        if all(x is None or isinstance(x, (Decimal, Fraction)) for x in flat):
            _from_exact_float_array(writer, key, arr, unit, None)
            return
        if any(x is not None for x in flat) and all(
                x is None or (isinstance(x, int) and not isinstance(x, bool))
                for x in flat):
            # An object array of Python ints is how to_numpy hands out
            # integers wider than 64 bits, and there was no way to write one
            # back: this fell through to the message below, whose only advice
            # was to store them as floats. The width is the narrowest that
            # holds them all and never under 64 -- the same rule dumps()
            # applies to a list of ints.
            from . import write_array, _array_int_annotation
            rows = arr.tolist()
            vt = _array_int_annotation(rows)
            if vt is None:
                vt = make_type_spec(
                    F.SINT if any(x is not None and x < 0 for x in flat)
                    else F.UINT, 64, 0)
            write_array(writer, key, rows, vt=vt, vu=_resolve_unit_arg(unit))
            return
        raise BovnarArgumentError(
            "object array is neither all int nor all Decimal/Fraction; pass "
            "float_format=(family,width[,frac]) to choose the target float type")

    family, width = _dtype_to_family_width(arr.dtype)
    vu = _resolve_unit_arg(unit)

    from . import write_array
    if family == F.DATETIME:
        # datetime64[*] -> the unix-epoch integer-seconds carrier. A unit makes
        # no sense on a timestamp. astype('datetime64[s]') truncates sub-second
        # resolution; NaT has no bovnar representation, so reject it rather than
        # emit the INT64_MIN sentinel as a bogus instant. The caller's writer is
        # responsible for the #!bovnar 1.1 directive (datetime is spec 1.1);
        # array_to_bvnr emits it automatically.
        if vu is not None:
            raise BovnarArgumentError("a unit is not applicable to a datetime array")
        secs = arr.astype('datetime64[s]')
        if np.isnat(secs).any():
            raise BovnarArgumentError(
                "NaT (not-a-time) has no bovnar datetime value; mask or fill it "
                "before converting")
        write_array(writer, key, secs.astype('int64').tolist(),
                    vt=make_type_spec(F.DATETIME, 64, 0), vu=None)
        return

    rows = arr.tolist()
    if family in (F.BOOL, F.UTF8):
        if vu is not None:
            raise BovnarArgumentError(
                f"a unit is not applicable to a {family.name.lower()} array")
        vt = make_type_spec(F.UTF8, 0, 0) if family == F.UTF8 else None
        write_array(writer, key, rows, vt=vt, vu=None)
    else:
        write_array(writer, key, rows, vt=make_type_spec(family, width, 0), vu=vu)


def from_pint_array(writer, key: str, qty) -> None:
    """Write a pint Quantity (magnitude + unit) as a bovnar array into *writer*."""
    from ._pint_bridge import from_pint
    mag, vu = from_pint(qty)
    from_numpy(writer, key, mag, unit=vu)


def array_to_bvnr(key: str, arr, *, unit=None, pretty: bool = True,
                  float_format=None) -> bytes:
    """Serialize a single ndarray to bovnar bytes under *key* (convenience).

    *float_format* forwards to :func:`from_numpy` for exact decimal/fixed/
    wide-binary float arrays (see that function).
    """
    from .writer import Writer
    from .exceptions import BovnarWriteError
    from .enums import ErrorCode
    np = _np()
    # The directive names the LOWEST version that admits what is written, and it
    # must lead the document. datetime64 is spec 1.1. A unit with no native
    # spelling (ucum:[IU], a UN/ECE package code) can only be written in the
    # profile notation, which is spec 1.2 -- without the directive the C writer
    # refuses it, and this function used to fail on such a unit for that reason.
    version = None
    if np.asarray(arr).dtype.kind == 'M':
        version = (1, 1)
    if unit is not None:
        from . import unit_is_profile_only
        vu = _resolve_unit_arg(unit)
        if vu is not None and vu.num_components and unit_is_profile_only(vu):
            version = (1, 2)
    cap = 4 * 1024 * 1024
    while True:
        try:
            with Writer.to_mem(cap=cap, pretty=pretty) as w:
                if version:
                    w.write_version(*version)       # directive must lead the doc
                from_numpy(w, key, arr, unit=unit, float_format=float_format)
            return w.get_output()
        except BovnarWriteError as e:
            if e.code == ErrorCode.SINK_BUFFER_EXHAUSTED and cap < 256 * 1024 * 1024:
                cap *= 2
            else:
                raise
