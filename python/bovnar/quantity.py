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

from .structs import ValueTypeSpec, ValueUnit, make_type_spec, make_unit_none
from .unit    import Unit
from .enums   import ValueTypeFamily
from .exceptions import BovnarArgumentError

_F = ValueTypeFamily
_FLOATISH = (int(_F.FLOAT), int(_F.FLOAT_DEC), int(_F.FLOAT_FIX))

# IEEE interchange widths that map to a bvn_float bit encoding (binary & decimal).
_IEEE_WIDTHS = (16, 32, 64, 128, 256)


def _validate_float_width(family, width: int) -> None:
    """Reject a width bovnar would refuse, early and with a clear message.

    Binary float accepts width 16 or any multiple of 32 up to 32768 (it stores
    the verbatim literal at that declared precision); decimal and fixed-point
    are limited to the IEEE interchange set 16/32/64/128/256.
    """
    fam, w = int(family), int(width)
    if fam in (int(_F.FLOAT_DEC), int(_F.FLOAT_FIX)):
        if w not in _IEEE_WIDTHS:
            raise BovnarArgumentError(
                f"{_F(fam).name.lower()} width must be one of 16/32/64/128/256, "
                f"not {w}")
    elif fam == int(_F.FLOAT):
        if not (w == 16 or (w >= 32 and w % 32 == 0 and w <= 32768)):
            raise BovnarArgumentError(
                f"binary float width must be 16 or a multiple of 32 "
                f"(up to 32768), not {w}")


def _decimal_to_text(d) -> str:
    """A bovnar-parseable numeric literal for a finite-or-special Decimal."""
    if d.is_nan():
        return 'nan'
    if d.is_infinite():
        return 'ninf' if d.is_signed() else 'inf'
    # str(Decimal) is exact; normalise the exponent marker to bovnar's 'e'.
    return str(d).replace('E', 'e')


def _fraction_to_text(fr) -> str:
    """Exact decimal literal for a *terminating* Fraction, else raise.

    A Fraction is exactly a finite decimal iff its reduced denominator factors
    into only 2s and 5s (e.g. the m/2**q of a fixed-point value). 1/3 cannot be
    written as an exact decimal — the caller must round it to a Decimal first.
    """
    from decimal import Decimal
    num, den = fr.numerator, fr.denominator
    a = b = 0
    d = den
    while d % 2 == 0:
        d //= 2; a += 1
    while d % 5 == 0:
        d //= 5; b += 1
    if d != 1:
        raise BovnarArgumentError(
            f"{fr} is not a terminating decimal; round it to a Decimal first")
    k = max(a, b)
    scaled = num * (10 ** k // den)              # exact: den divides 10**k
    return _decimal_to_text(Decimal(scaled).scaleb(-k))


def _exact_number_text(value) -> str:
    """Format a Python number as an exact bovnar numeric literal.

    Decimal/Fraction are exact; int is exact; a float (incl. a numpy float
    scalar) is only as precise as the underlying binary float. numpy scalar
    types are coerced through ``numbers`` so e.g. np.float64 does not stringify
    as ``'np.float64(1.5)'`` (its repr under numpy 2.x).
    """
    import numbers
    from decimal import Decimal
    from fractions import Fraction
    if isinstance(value, bool):
        raise BovnarArgumentError("bool is not a numeric value")
    if isinstance(value, str):
        return value                             # caller-supplied literal, verbatim
    if isinstance(value, Fraction):
        return _fraction_to_text(value)
    if isinstance(value, Decimal):
        return _decimal_to_text(value)
    if isinstance(value, numbers.Integral):      # Python int, numpy ints, ...
        from ._bigint import int_to_decimal      # str() stops at 4300 digits
        return int_to_decimal(int(value))
    if isinstance(value, numbers.Real):          # Python/numpy floats
        f = float(value)
        # repr() spells the non-finite values Python's way, and '-inf' is not a
        # bovnar number: the format writes 'ninf'. from_number(float('-inf'))
        # built a Quantity the writer then refused (type_value_mismatch), and
        # so did a numpy -inf scalar reaching an array through this path.
        if f != f:
            return 'nan'
        if f in (float('inf'), float('-inf')):
            return 'inf' if f > 0 else 'ninf'
        return repr(f)                           # shortest round-tripping decimal
    raise BovnarArgumentError(
        f"cannot format {type(value).__name__} as a bovnar number")


def _check_decimal_fits(text: str, width: int) -> None:
    """Refuse a finite decimal literal that float_dec:*width* cannot hold.

    decimal64 ends just below 1e385, and ``dumps({'a': Decimal('1e400')})``
    wrote ``<float_dec:64> 1e+400`` without a word: a document whose value, the
    moment anything materialises it into the type it declares, is infinity.
    The reader does not range-check a decimal float, so nothing downstream
    said so either. Only OVERFLOW is refused -- a literal with more digits
    than the width keeps is rounded on materialisation, which is what
    ``stored_value`` exists to show, and an underflow to zero is a rounding
    too.

    *text* that is not a plain decimal literal (a verbatim string the caller
    supplied in some other shape) is left to the writer.
    """
    from decimal import Decimal, InvalidOperation
    from ._bvnfloat import DECIMAL_FORMATS, decimal_stored_value
    if int(width) not in DECIMAL_FORMATS:
        return
    try:
        d = Decimal(text)
    except (InvalidOperation, ValueError):
        return
    if not d.is_finite() or d == 0:
        return
    if decimal_stored_value(d, int(width)).is_infinite():
        raise BovnarArgumentError(
            f"{text} is out of range for float_dec:{int(width)} (it would "
            f"materialise to infinity); use a wider float_dec")


class Quantity:
    """
    A typed, unit-annotated scalar value with its original text representation.

    Returned by ``loads(..., typed=True)`` and accepted by ``dumps()`` to
    enable exact round-trips: numeric precision, integer base, and physical
    units are all preserved across a load/dump cycle.
    """
    __slots__ = ('raw', 'vtype', 'unit', '_tok_type', '_frac', '_unit_source')

    def __init__(self,
                 raw:      str | None,
                 vtype:    ValueTypeSpec,
                 unit:     'Unit | ValueUnit | str | None' = None,
                 tok_type: int = 2,
                 frac:     str | None = None,
                 unit_source: str | None = None) -> None:
        # VALIDATED SINCE 2.0. It used to accept anything and store it, so
        # Quantity("1", "not-a-valuetypespec") constructed happily and every
        # consumer of .vtype had to defend itself against a value that could
        # never be right -- one of them by catching AttributeError on .family.
        # A constructor that cannot say no pushes that cost onto every reader of
        # the object, and onto whoever debugs the traceback three frames later.
        if raw is not None and not isinstance(raw, str):
            raise BovnarArgumentError(
                "Quantity raw must be a str (the verbatim literal) or None; "
                "got %s" % type(raw).__name__)
        if not isinstance(vtype, ValueTypeSpec):
            raise BovnarArgumentError(
                "Quantity vtype must be a ValueTypeSpec; got %s. Build one with "
                "bovnar.make_type_spec(family, width, base)."
                % type(vtype).__name__)
        if not isinstance(tok_type, int) or isinstance(tok_type, bool):
            raise BovnarArgumentError(
                "Quantity tok_type must be an int; got %s" % type(tok_type).__name__)
        if frac is not None and not isinstance(frac, str):
            raise BovnarArgumentError(
                "Quantity frac must be the verbatim sub-second digits as a str, "
                "or None; got %s" % type(frac).__name__)
        self.raw       = raw
        self.vtype     = vtype
        self.unit      = unit if isinstance(unit, Unit) else Unit(
            Unit.parse(unit).raw if isinstance(unit, str) else unit)
        self._tok_type = tok_type
        # spec 1.1 — verbatim ISO sub-second digits of a datetime carrier (the
        # whole-second value is `raw`). None for any other value or a datetime
        # with no fraction. Carried through dumps() so a fractional datetime
        # round-trips losslessly via the C writer's ISO-literal reconstruction.
        self._frac     = frac
        # library 2.0 — the unit as the document SPELLED it, where that says more
        # than `unit` can: "ucum:mL{total}" is the unit m~L, and the annotation
        # survives only here. dumps() writes this spelling back; the C writer
        # refuses one that does not parse to `unit`, so a stale or mistyped
        # value fails at the write rather than producing a document that says
        # two things. None writes the canonical form.
        if unit_source is not None and not isinstance(unit_source, str):
            raise BovnarArgumentError(
                "Quantity unit_source must be the unit's verbatim spelling as a "
                "str, or None; got %s" % type(unit_source).__name__)
        self._unit_source = unit_source or None

    @property
    def unit_source(self) -> str | None:
        """The unit exactly as the document spelled it, or None. ``unit`` is
        what it means; this keeps what formatting it would lose -- a profile's
        spelling and, above all, a UCUM annotation."""
        return self._unit_source

    @property
    def datetime_fraction(self) -> str | None:
        """Verbatim ISO sub-second digits of a datetime value, or None."""
        return self._frac

    @classmethod
    def from_number(cls, value, *, family=ValueTypeFamily.FLOAT_DEC,
                    width: int = 64, frac: int | None = None,
                    unit: ValueUnit | None = None) -> 'Quantity':
        """Build a typed float Quantity from an exact Python number.

        *value* may be a :class:`~decimal.Decimal`, :class:`~fractions.Fraction`,
        ``int``, ``str`` (a verbatim literal), or ``float`` (only as precise as
        the double). The value is stored as an exact decimal literal, so writing
        the Quantity with :func:`dumps` is lossless to the format's precision.

        *family* is one of FLOAT, FLOAT_DEC (default), or FLOAT_FIX; *frac* (the
        number of fractional bits) is required for FLOAT_FIX.
        """
        fam = int(family)
        if fam == int(_F.FLOAT_FIX):
            if frac is None:
                raise BovnarArgumentError("float_fix requires frac=<fractional bits>")
            base = int(frac)
        elif fam in (int(_F.FLOAT), int(_F.FLOAT_DEC)):
            base = 0
        else:
            raise BovnarArgumentError(
                "from_number: family must be FLOAT, FLOAT_DEC, or FLOAT_FIX")
        _validate_float_width(family, width)
        text = _exact_number_text(value)
        if fam == int(_F.FLOAT_FIX):
            from ._ffi import get_library
            if not get_library().bvn_float_str_fits_fix(
                    text.encode('ascii'), 10, int(width) or 64, base):
                raise BovnarArgumentError(
                    f"{text} does not fit a signed float_fix:{width},q{base}")
        if fam == int(_F.FLOAT_DEC):
            _check_decimal_fits(text, int(width) or 64)
        vt = make_type_spec(family, int(width), base)
        return cls(text, vt, unit, tok_type=2)

    @property
    def value(self):
        """Decode the stored text to the closest native Python scalar."""
        from . import _decode_value
        raw_bytes = self.raw.encode('utf-8') if self.raw else b''
        return _decode_value(raw_bytes, ValueTypeFamily(self.vtype.family),
                             self.vtype, self._tok_type)

    @property
    def epoch_name(self):
        """For a `datetime` quantity, the epoch name (`"unix"`, `"tai"`, …);
        `None` for any other family. The integer `value` is seconds since this
        epoch."""
        if self.vtype.family != int(ValueTypeFamily.DATETIME):
            return None
        from ._ffi import get_library
        return get_library().bvnr_datetime_epoch_name(self.vtype).decode('ascii')

    @property
    def epoch_mjd(self):
        """For a `datetime` quantity, the epoch's Modified Julian Day (the
        `bvn_epoch_t` value); `None` otherwise."""
        if self.vtype.family != int(ValueTypeFamily.DATETIME):
            return None
        from ._ffi import get_library
        return int(get_library().bvnr_datetime_epoch_mjd(self.vtype))

    # ── lossless numeric access ────────────────────────────────────────────
    # ``value`` decodes through a C double and loses precision for float_dec,
    # float_fix, and the wide binary floats (float:128 and wider). These
    # accessors instead use the verbatim literal text (``raw``) and bovnar's
    # arbitrary-precision float, so the full precision the format carries is
    # reachable from Python.

    def _numeric_base(self) -> int:
        """Numeric radix of the literal (10 for the decimal families)."""
        if int(self.vtype.family) == int(_F.FLOAT):
            return int(self.vtype.base) or 10
        return 10                                # float_dec / float_fix are decimal

    def _nonfinite_decimal(self):
        """Decimal('NaN'/±Infinity') if raw is a non-finite token, else None."""
        from decimal import Decimal
        low = (self.raw or '').strip().lower()
        if low == 'nan':
            return Decimal('NaN')
        if low in ('inf', '+inf', 'infinity', '+infinity'):
            return Decimal('Infinity')
        if low in ('ninf', '-inf', '-infinity'):
            return Decimal('-Infinity')
        return None

    @property
    def decimal(self):
        """Exact value as :class:`decimal.Decimal`, from the verbatim literal.

        Lossless for every numeric family (``uint``/``sint`` and all three float
        families). Raises for non-numeric families (bool/utf8/datetime).
        """
        from decimal import Decimal
        fam = int(self.vtype.family)
        if fam in (int(_F.UINT), int(_F.SINT)):
            v = self.value
            if v is None:
                return None
            return Decimal(v)
        if fam in _FLOATISH:
            nf = self._nonfinite_decimal()
            if nf is not None:
                return nf
            if self.raw is None:
                return None
            base = self._numeric_base()
            if base in (0, 10):
                return Decimal(self.raw)
            from ._bvnfloat import BvnFloat
            return BvnFloat.from_str(self.raw, base).to_decimal()
        raise BovnarArgumentError(
            f"decimal() is not defined for a {_F(fam).name} value")

    @property
    def fraction(self):
        """Exact value as :class:`fractions.Fraction`.

        For a ``float_fix`` value this is the exact Q-format datum
        ``mantissa / 2**frac_bits``; for the others it is the exact literal.
        """
        from fractions import Fraction
        if int(self.vtype.family) == int(_F.FLOAT_FIX):
            m, q = self.fixed_point
            return Fraction(m, 1 << q)
        d = self.decimal
        if d is None:
            return None
        return Fraction(d)

    @property
    def fixed_point(self) -> tuple:
        """``(mantissa, frac_bits)`` for a ``float_fix`` value; the exact value
        is ``mantissa / 2**frac_bits``. Raises for any other family."""
        if int(self.vtype.family) != int(_F.FLOAT_FIX):
            raise BovnarArgumentError("fixed_point() is only valid for float_fix")
        from ._bvnfloat import BvnFloat
        q = int(self.vtype.base)
        width = int(self.vtype.width) or 64
        f = BvnFloat.from_str(self.raw, 10)
        return f.to_fixed_mantissa(width, q), q

    def _parser_saturated(self, f, lit) -> bool:
        """True when bovnar's C parser collapsed a finite, nonzero literal to
        0/inf because its exact rational exceeds the 32768-bit intermediate cap
        (decimal exponent beyond ~1e±9865)."""
        return (lit is not None and lit.is_finite() and lit != 0
                and (f.is_zero or f.is_inf))

    def _decimal_bits(self) -> bytes:
        """IEEE decimal interchange bytes for this (float_dec) value, over the
        full range. Uses bovnar's C encoder normally, falling back to the pure-
        Python encoder only when the C parser saturates an extreme exponent
        (beyond its ~1e9865 intermediate cap)."""
        from ._bvnfloat import BvnFloat, decimal_to_interchange_bits
        width = int(self.vtype.width) or 64
        # nan / inf / ninf have no digits to round, so they go straight to the
        # encoder under one spelling each. strtodec_bits is also where 'ninf'
        # is made to work: the C routine reads 'inf' and 'nan' and answered
        # zeros for the format's own negative infinity.
        nf = self._nonfinite_decimal()
        if nf is not None:
            return BvnFloat.strtodec_bits(
                'nan' if nf.is_nan() else ('ninf' if nf.is_signed() else 'inf'),
                width)
        f = BvnFloat.from_str(self.raw, 10)
        lit = self.decimal
        if self._parser_saturated(f, lit):
            return decimal_to_interchange_bits(lit, width)
        # Straight from the literal: encoding the parsed bvn_float would round
        # the decimal tie through a binary intermediate and land on the wrong
        # side 26 % of the time. See BvnFloat.strtodec_bits.
        return BvnFloat.strtodec_bits(self.raw, width)

    def _decimal_stored(self):
        """The materialised float_dec value as an exact Decimal, full range
        (C encoder normally; pure-Python encoder past the parser's cap)."""
        from ._bvnfloat import BvnFloat, decimal_stored_value
        width = int(self.vtype.width) or 64
        f = BvnFloat.from_str(self.raw, 10)
        lit = self.decimal
        if self._parser_saturated(f, lit):
            return decimal_stored_value(lit, width)
        return BvnFloat.from_decimal_bits(
            width, BvnFloat.strtodec_bits(self.raw, width)).to_decimal()

    def _binary_bits(self) -> bytes:
        """IEEE binary interchange bytes for this (binary float) value, over the
        full representable range. Uses bovnar's C encoder, falling back to the
        pure-Python correctly-rounded encoder when the C parser saturates an
        extreme decimal exponent."""
        from ._bvnfloat import BvnFloat, fraction_to_binary_bits
        width = int(self.vtype.width) or 64
        base = self._numeric_base()
        f = BvnFloat.from_str(self.raw, base)
        lit = self.decimal
        if base in (0, 10) and self._parser_saturated(f, lit):
            from fractions import Fraction
            return fraction_to_binary_bits(Fraction(lit), width)
        return f.to_binary_bits(width)

    @property
    def stored_value(self):
        """The value as actually materialised into the declared IEEE/fixed
        format (round-to-nearest-even), as an exact :class:`decimal.Decimal`.

        This differs from :attr:`decimal` only when the literal carries more
        precision than the format holds (e.g. a 40-digit ``float_dec:64``
        literal rounds to 16 significant digits here). Supported for the IEEE
        binary widths ``float:16/32/64/128/256``, every ``float_dec`` width, and
        every ``float_fix`` width. A binary ``float`` width with no IEEE
        encoding (e.g. ``float:96``/``512``) raises; use :attr:`decimal`.
        """
        from decimal import Decimal
        from ._bvnfloat import BvnFloat
        fam = int(self.vtype.family)
        width = int(self.vtype.width) or 64
        # A typed null (`.a = <float:64,m>;`) has a type and no literal. .value,
        # .decimal and .fraction all answer None for it; this one went on to
        # encode the missing text and raised AttributeError from inside.
        if self.raw is None and fam in _FLOATISH:
            return None
        nf = self._nonfinite_decimal()
        if nf is not None and fam != int(_F.FLOAT_FIX):
            return nf
        if fam == int(_F.FLOAT_FIX):
            m, q = self.fixed_point
            return Decimal(m) / Decimal(1 << q)
        if fam == int(_F.FLOAT) and width not in _IEEE_WIDTHS:
            raise BovnarArgumentError(
                f"stored_value() materialises IEEE 16/32/64/128/256-bit floats; "
                f"float:{width} has no such encoding — use .decimal for the "
                f"exact literal value")
        if fam == int(_F.FLOAT):
            return BvnFloat.from_binary_bits(width, self._binary_bits()).to_decimal()
        if fam == int(_F.FLOAT_DEC):
            return self._decimal_stored()
        raise BovnarArgumentError(
            f"stored_value() is not defined for a {_F(fam).name} value")

    @property
    def ieee_bits(self) -> bytes:
        """The raw IEEE-754 interchange bytes of the materialised value
        (little-endian word order): binary16/32/64/128/256 for ``float``, the
        decimal16…256 interchange encoding for ``float_dec``. A binary ``float``
        width with no IEEE encoding (e.g. ``float:96``/``512``) raises, as does
        any non-``float``/``float_dec`` family."""
        from ._bvnfloat import BvnFloat
        fam = int(self.vtype.family)
        width = int(self.vtype.width) or 64
        if self.raw is None and fam in _FLOATISH:
            raise BovnarArgumentError(
                "ieee_bits() is not defined for a null value: it has a type "
                "and no number to encode")
        if fam == int(_F.FLOAT) and width not in _IEEE_WIDTHS:
            raise BovnarArgumentError(
                f"ieee_bits() materialises IEEE 16/32/64/128/256-bit floats; "
                f"float:{width} has no such encoding — use .decimal instead")
        if fam == int(_F.FLOAT):
            return self._binary_bits()
        if fam == int(_F.FLOAT_DEC):
            return self._decimal_bits()
        raise BovnarArgumentError(
            "ieee_bits() is only valid for float (16..256) and float_dec values")

    @property
    def unit_str(self) -> str:
        """The canonical unit string, or ``''`` when dimensionless. Kept as a
        distinct name from ``str(self.unit)`` -- which it now delegates to --
        because it is what __repr__ and the writer paths ask for."""
        return str(self.unit)

    def __repr__(self) -> str:
        fam = ValueTypeFamily(self.vtype.family).name
        u   = self.unit_str
        u_part = f' [{u}]' if u else ''
        return f'Quantity({self.raw!r}, {fam}{u_part})'

    def __eq__(self, other) -> bool:
        # _frac is part of the value. Without it two datetimes 500 ms apart --
        # same whole-second carrier, different sub-second digits -- compared
        # equal and collapsed to one entry in a set or dict, silently
        # deduplicating distinct instants.
        if not isinstance(other, Quantity):
            return NotImplemented
        return (self.raw        == other.raw
                and self._frac        == other._frac
                and self.vtype.family == other.vtype.family
                and self.vtype.width  == other.vtype.width
                and self.vtype.base   == other.vtype.base
                and self.unit_str   == other.unit_str)

    def __hash__(self) -> int:
        return hash((self.raw,
                     self._frac,
                     self.vtype.family,
                     self.vtype.width,
                     self.vtype.base,
                     self.unit_str))
