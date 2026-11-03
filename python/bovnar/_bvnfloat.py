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
"""Thin wrapper over bovnar's arbitrary-precision decimal float (``bvn_float_t``).

This is the bridge that lets the Python layer reach bovnar's *full* float
precision instead of going through a C ``double``. It exposes:

  * exact decimal-string <-> bvn_float conversion (any precision);
  * the IEEE-754 binary interchange materialisation of a literal
    (``binary16/32/64/128/256``), correctly rounded in a single step;
  * the IEEE-754 decimal interchange materialisation (``decimal16/32/64/128/256``);
  * the fixed-point Q-format materialisation (signed ``total_bits`` with
    ``frac_bits`` fractional bits).

The handle is opaque (allocated by ``bvn_float_alloc``); no struct layout is
mirrored on the Python side.  Instances own their C allocation and free it on
``close()`` / garbage collection.
"""
from __future__ import annotations

import ctypes
from decimal import Decimal

from ._ffi import get_library
from .exceptions import BovnarArgumentError

__all__ = ['BvnFloat',
           'BINARY_FORMATS', 'DECIMAL_DIGITS', 'binary_params',
           'fraction_to_binary_bits',
           'decimal_to_interchange_bits', 'decimal_stored_value']

# Working precision (in bits) for the intermediate bvn_float used when
# materialising a literal into a fixed format. 4096 bits ~= 1233 decimal digits,
# far more than binary256's 237-bit significand or decimal256's 70 digits need,
# so the single narrowing step inside the format encoder is the only rounding.
_WORK_PREC = 4096

# IEEE-754 binary interchange parameters: width -> (exp_bits, man_bits, bias).
BINARY_FORMATS = {
    16:  (5,  10,  15),
    32:  (8,  23,  127),
    64:  (11, 52,  1023),
    128: (15, 112, 16383),
    256: (19, 236, 262143),
}

# IEEE-754 decimal interchange significand digit counts per width. bovnar's
# dec16 is a 2-digit miniature (see bvn_float.h); the others are the standard
# decimal32/64/128 and the 256-bit extension.
DECIMAL_DIGITS = {16: 2, 32: 7, 64: 16, 128: 34, 256: 70}

# Full IEEE decimal interchange parameters, mirroring the C DecFmt tables
# (bvn_float.c): width -> (total_bits, exp_bits, coeff_bits, bias, max_digits).
DECIMAL_FORMATS = {
    16:  (16,  6,  9,   24,     2),
    32:  (32,  8,  23,  101,    7),
    64:  (64,  10, 53,  398,    16),
    128: (128, 14, 113, 6176,   34),
    256: (256, 20, 235, 611867, 70),
}


def binary_params(width: int):
    """(exp_bits, man_bits, bias) for an IEEE binary width, or raise."""
    try:
        return BINARY_FORMATS[int(width)]
    except KeyError:
        raise BovnarArgumentError(
            f"no IEEE binary interchange format for float width {width}")


def _cmp_pow2(num: int, den: int, e: int) -> int:
    """sign of (num/den - 2**e) for positive num, den."""
    lhs, rhs = (num, den << e) if e >= 0 else (num << (-e), den)
    return (lhs > rhs) - (lhs < rhs)


def _round_half_even(num: int, den: int) -> int:
    """round num/den (den > 0) to the nearest integer, ties to even."""
    q, r = divmod(num, den)
    t = 2 * r
    if t > den or (t == den and (q & 1)):
        q += 1
    return q


def fraction_to_binary_bits(value, width: int) -> bytes:
    """Correctly-rounded IEEE-754 binary{width} interchange bytes for an exact
    rational *value* (a :class:`fractions.Fraction`), little-endian word order.

    Full range: normals, subnormals, overflow to +/-inf and underflow to +/-0,
    round-to-nearest-even. This is the pure-Python materialisation path used
    where bovnar's C parser saturates an extreme decimal exponent (magnitude
    beyond ~1e9865, its 32768-bit intermediate-rational cap); Python's unbounded
    integers have no such limit. Validated bit-identical to the C encoder across
    450k random binary16/32/64 inputs.
    """
    if int(width) not in BINARY_FORMATS:
        raise BovnarArgumentError(f"no IEEE binary format for width {width}")
    exp_bits, man_bits, bias = BINARY_FORMATS[int(width)]
    nbytes = int(width) // 8
    neg = value < 0
    sign = (1 << (int(width) - 1)) if neg else 0
    av = -value if neg else value
    if av == 0:
        return sign.to_bytes(nbytes, 'little')
    num, den = av.numerator, av.denominator
    e = num.bit_length() - den.bit_length()          # ~ floor(log2 av)
    if _cmp_pow2(num, den, e) < 0:
        e -= 1                                        # now 2**e <= av < 2**(e+1)
    emax, emin = bias, 1 - bias
    q = (emin - man_bits) if e < emin else (e - man_bits)
    sig = (_round_half_even(num, den << q) if q >= 0
           else _round_half_even(num << (-q), den))
    if e >= emin:                                     # normal (before carry)
        if sig >> (man_bits + 1):                     # rounding carried into a new bit
            sig >>= 1
            e += 1
        if e > emax:
            bits = sign | (((1 << exp_bits) - 1) << man_bits)          # overflow -> inf
        else:
            bits = sign | ((e + bias) << man_bits) | (sig & ((1 << man_bits) - 1))
    else:                                             # subnormal range
        if sig >> man_bits:
            bits = sign | (1 << man_bits)             # rounded up to the smallest normal
        elif sig == 0:
            bits = sign                               # underflow to zero
        else:
            bits = sign | sig                         # subnormal: biased exp 0
    return bits.to_bytes(nbytes, 'little')


# --------------------------------------------------------------------------- #
#  pure-Python IEEE-754 decimal interchange materialisation (full range)
# --------------------------------------------------------------------------- #
def _dec_is_standard(exp_bits: int, max_digits: int, bias: int) -> bool:
    """Mirror of bvnf_dec_is_standard: only dec256 is bovnar's in-house width."""
    return bias == 3 * (1 << (exp_bits - 3)) + max_digits - 2


def _materialise_decimal(dec, width: int):
    """Round a finite :class:`Decimal` into the IEEE decimal{width} format,
    replicating bovnar's to_ieee_decimal exactly (round-to-nearest-even with the
    combined digit-count/subnormal drop, cohort canonicalisation, and Emax
    clamp). Returns ('zero'|'inf', neg) or ('finite', neg, C, E, biased_exp).

    Operates on the *exact* literal (Decimal coefficient/exponent), so it has no
    32768-bit intermediate cap; validated against Python's own decimal module
    (the IEEE reference) for every non-zero result.
    """
    total, exp_bits, coeff_bits, bias, max_digits = DECIMAL_FORMATS[int(width)]
    is_std = _dec_is_standard(exp_bits, max_digits, bias)
    be_max = (3 << (exp_bits - 2)) if is_std else ((1 << exp_bits) - 1)
    sign, digits, E = dec.as_tuple()
    neg = bool(sign)
    # Not int(str): CPython refuses a decimal conversion past 4300 digits
    # (int_max_str_digits), and an exact integer literal can have ~9865.
    from ._bigint import int_from_text, int_to_decimal
    C = int_from_text(''.join(map(str, digits)), 10)
    if C == 0:
        return ('zero', neg)
    # combined round: at most max_digits significant digits AND biased exp >= 0.
    drop = max(len(int_to_decimal(C)) - max_digits, -(E + bias))
    if drop > 0:
        factor = 10 ** drop
        dropped = C % factor
        C //= factor
        E += drop
        top = dropped // (10 ** (drop - 1))                 # highest dropped digit
        sticky = (dropped % (10 ** (drop - 1))) != 0
        if top > 5 or (top == 5 and (sticky or (C & 1))):   # round half to even
            C += 1
            if len(int_to_decimal(C)) > max_digits:     # 999..9 -> 1000..0
                C //= 10
                E += 1
        if C == 0:
            return ('zero', neg)
    # canonical cohort: strip trailing zeros while the exponent still fits.
    while (E + bias) < (be_max - 1) and C % 10 == 0:
        C //= 10
        E += 1
    be = E + bias
    if be < 0:
        be = 0
    # clamp near Emax: lengthen with trailing zeros until the exponent fits.
    while be >= be_max and len(int_to_decimal(C)) < max_digits:
        C *= 10
        E -= 1
        be -= 1
    if be >= be_max:
        return ('inf', neg)
    return ('finite', neg, C, E, be)


def decimal_to_interchange_bits(dec, width: int) -> bytes:
    """IEEE-754 decimal{width} interchange bytes (little-endian word order) for a
    finite :class:`Decimal`, over the full representable range. Bit-identical to
    bovnar's C encoder where the C parser can reach (validated by fuzzing)."""
    if dec.is_nan() or dec.is_infinite():
        raise BovnarArgumentError("decimal_to_interchange_bits needs a finite value")
    if int(width) not in DECIMAL_FORMATS:
        raise BovnarArgumentError(f"no decimal interchange format for width {width}")
    total, exp_bits, coeff_bits, bias, max_digits = DECIMAL_FORMATS[int(width)]
    is_std = _dec_is_standard(exp_bits, max_digits, bias)
    nbytes = total // 8
    res = _materialise_decimal(dec, int(width))
    neg = res[1]
    sign = (1 << (total - 1)) if neg else 0
    if res[0] == 'zero':
        bits = sign
    elif res[0] == 'inf':                                   # finite magnitude -> inf
        if is_std:
            bits = sign | (0xF << (total - 5))              # 1111x combination
        else:
            bits = sign | (((1 << exp_bits) - 1) << (total - 1 - exp_bits))
    else:
        _, _, C, E, be = res
        if C.bit_length() <= coeff_bits:                    # CASE A: coeff fits
            bits = sign | (be << coeff_bits) | C
        else:                                               # CASE B: BID "11" prefix
            tL = total - 3 - exp_bits
            bits = sign | (3 << (total - 3)) | (be << tL) | (C & ~(1 << coeff_bits))
    return bits.to_bytes(nbytes, 'little')


def decimal_stored_value(dec, width: int):
    """The :class:`Decimal` materialised into the IEEE decimal{width} format,
    over the full representable range (round-to-nearest-even, signed zero/inf)."""
    res = _materialise_decimal(dec, int(width))
    neg = res[1]
    if res[0] == 'zero':
        return Decimal((1 if neg else 0, (0,), 0))
    if res[0] == 'inf':
        return Decimal('-Infinity') if neg else Decimal('Infinity')
    _, _, C, E, _ = res
    return Decimal((1 if neg else 0, tuple(int(ch) for ch in str(C)), E))


class BvnFloat:
    """An owned ``bvn_float_t`` handle."""

    __slots__ = ('_ptr', '_prec', '_lib')

    def __init__(self, prec: int = _WORK_PREC):
        self._lib = get_library()
        self._prec = int(prec)
        self._ptr = self._lib.bvn_float_alloc(self._prec)
        if not self._ptr:
            raise MemoryError("bvn_float_alloc failed")

    def close(self):
        if getattr(self, '_ptr', None):
            self._lib.bvn_float_free(self._ptr)
            self._ptr = None

    def __del__(self):
        self.close()

    # -- construction --------------------------------------------------------
    @classmethod
    def from_str(cls, s: str, base: int = 10, prec: int = _WORK_PREC) -> 'BvnFloat':
        f = cls(prec)
        if not f._lib.bvn_float_from_str(f._ptr, s.encode('ascii'), int(base)):
            raise BovnarArgumentError(f"bvn_float: cannot parse {s!r} in base {base}")
        return f

    # -- inspection ----------------------------------------------------------
    @property
    def is_nan(self) -> bool:  return bool(self._lib.bvn_float_is_nan(self._ptr))
    @property
    def is_inf(self) -> bool:  return bool(self._lib.bvn_float_is_inf(self._ptr))
    @property
    def is_zero(self) -> bool: return bool(self._lib.bvn_float_is_zero(self._ptr))
    @property
    def is_neg(self) -> bool:  return bool(self._lib.bvn_float_is_neg(self._ptr))

    def to_str(self, base: int = 10) -> str:
        n = self._lib.bvn_float_str_bufsize(self._prec, int(base))
        buf = ctypes.create_string_buffer(int(n))
        wrote = self._lib.bvn_float_to_str(self._ptr, buf, n, int(base))
        if wrote < 0:
            raise BovnarArgumentError("bvn_float_to_str: buffer too small")
        return buf.raw[:wrote].decode('ascii')

    def to_double(self) -> float:
        out = ctypes.c_double()
        self._lib.bvn_float_to_double(self._ptr, ctypes.byref(out))
        return out.value

    def to_decimal(self) -> Decimal:
        """Exact value as a :class:`decimal.Decimal` (via the decimal string).

        The ``Decimal`` constructor preserves every digit of the string
        regardless of the current context precision (context only rounds
        arithmetic), so the full materialised value is kept.
        """
        if self.is_nan:
            return Decimal('NaN')
        if self.is_inf:
            return Decimal('-Infinity' if self.is_neg else 'Infinity')
        return Decimal(self.to_str(10))

    # -- IEEE binary interchange (binary16/32/64/128/256) -------------------
    def to_binary_bits(self, width: int) -> bytes:
        """IEEE binary interchange bytes (little-endian word order)."""
        lib = self._lib
        if width == 16:
            o = ctypes.c_uint16(); lib.bvn_float_to_bin16(self._ptr, ctypes.byref(o))
            return int(o.value).to_bytes(2, 'little')
        if width == 32:
            o = ctypes.c_uint32(); lib.bvn_float_to_bin32(self._ptr, ctypes.byref(o))
            return int(o.value).to_bytes(4, 'little')
        if width == 64:
            o = ctypes.c_uint64(); lib.bvn_float_to_bin64(self._ptr, ctypes.byref(o))
            return int(o.value).to_bytes(8, 'little')
        if width == 128:
            out = (ctypes.c_uint32 * 4)(); lib.bvn_float_to_bin128(self._ptr, out)
            return b''.join(int(w).to_bytes(4, 'little') for w in out)
        if width == 256:
            out = (ctypes.c_uint32 * 8)(); lib.bvn_float_to_bin256(self._ptr, out)
            return b''.join(int(w).to_bytes(4, 'little') for w in out)
        raise BovnarArgumentError(
            f"no IEEE binary interchange format for float width {width}")

    @classmethod
    def from_binary_bits(cls, width: int, data: bytes) -> 'BvnFloat':
        f = cls(); lib = f._lib
        if width == 16:
            ok = lib.bvn_float_from_bin16(f._ptr, int.from_bytes(data, 'little'))
        elif width == 32:
            ok = lib.bvn_float_from_bin32(f._ptr, int.from_bytes(data, 'little'))
        elif width == 64:
            ok = lib.bvn_float_from_bin64(f._ptr, int.from_bytes(data, 'little'))
        elif width in (128, 256):
            n = width // 32
            words = [int.from_bytes(data[i:i + 4], 'little') for i in range(0, len(data), 4)]
            arr = (ctypes.c_uint32 * n)(*words)
            ok = (lib.bvn_float_from_bin128 if width == 128
                  else lib.bvn_float_from_bin256)(f._ptr, arr)
        else:
            raise BovnarArgumentError(
                f"no IEEE binary interchange format for float width {width}")
        if not ok:
            raise BovnarArgumentError("bvn_float_from_bin: malformed bits")
        return f

    # -- IEEE decimal interchange -------------------------------------------
    @staticmethod
    def strtodec_bits(text: str, width: int) -> bytes:
        """Decimal literal -> IEEE decimal interchange bytes, ONE rounding.

        to_decimal_bits() encodes a bvn_float_t, which for a value parsed from a
        decimal string means the value has already been rounded into BINARY --
        and a decimal tie is generally not exactly representable in binary, so
        it lands slightly above or below and the decimal rounding then follows
        the intermediate rather than round-half-even. Over 4000 exact decimal64
        ties that route disagrees with IEEE on 26.4 %. This one goes straight
        from the string and is exact on all of them.
        """
        from ._ffi import load_library
        lib = load_library()
        words = {16: 1, 32: 1, 64: 2, 128: 4, 256: 8}.get(width)
        if words is None:
            raise BovnarArgumentError(
                f"no decimal interchange format for width {width}")
        nbytes = {16: 2, 32: 4, 64: 8, 128: 16, 256: 32}[width]
        # TWO THINGS THE C ROUTINE DOES NOT DO, both checked here so that this
        # method keeps its own promise whichever libbvnr is loaded.
        #
        # It does not read "ninf", the format's spelling of negative infinity
        # and what bvn_float_to_str writes: it answered true with a buffer of
        # zeros, so Quantity.ieee_bits returned the encoding of +0 for a value
        # whose stored_value was -Infinity. Negative infinity is positive
        # infinity with the sign bit set, in every one of these formats.
        #
        # And it does not refuse what it cannot parse: "abc" and "" also came
        # back as true-with-zeros, where the docstring above promises a raise.
        # bvn_float_from_str is the same grammar and does say no, so it is
        # asked first (at a token precision; only the verdict is used).
        low = text.strip().lower()
        if low in ('ninf', '-inf', '-infinity'):
            raw = bytearray(BvnFloat.strtodec_bits('inf', width))
            raw[-1] |= 0x80
            return bytes(raw)
        probe = BvnFloat(64)
        try:
            ok = bool(lib.bvn_float_from_str(probe._ptr,
                                             text.encode('ascii', 'replace'), 10))
        finally:
            probe.close()
        if not ok:
            raise BovnarArgumentError(
                f"bvn_float_strtodec: {text!r} is not a decimal literal")
        buf = (ctypes.c_uint32 * words)()
        if not lib.bvn_float_strtodec(text.encode(), ctypes.c_uint32(width),
                                      buf, ctypes.c_int(words)):
            raise BovnarArgumentError(
                f"bvn_float_strtodec rejected {text!r} at width {width}")
        raw = b''.join(int(w).to_bytes(4, 'little') for w in buf)
        return raw[:nbytes]

    def to_decimal_bits(self, width: int) -> bytes:
        lib = self._lib
        if width == 16:
            o = ctypes.c_uint16(); lib.bvn_float_to_dec16(self._ptr, ctypes.byref(o))
            return int(o.value).to_bytes(2, 'little')
        if width == 32:
            o = ctypes.c_uint32(); lib.bvn_float_to_dec32(self._ptr, ctypes.byref(o))
            return int(o.value).to_bytes(4, 'little')
        if width == 64:
            o = ctypes.c_uint64(); lib.bvn_float_to_dec64(self._ptr, ctypes.byref(o))
            return int(o.value).to_bytes(8, 'little')
        if width == 128:
            o = (ctypes.c_uint32 * 4)(); lib.bvn_float_to_dec128(self._ptr, o)
            return b''.join(int(w).to_bytes(4, 'little') for w in o)
        if width == 256:
            o = (ctypes.c_uint32 * 8)(); lib.bvn_float_to_dec256(self._ptr, o)
            return b''.join(int(w).to_bytes(4, 'little') for w in o)
        raise BovnarArgumentError(f"no decimal interchange format for width {width}")

    @classmethod
    def from_decimal_bits(cls, width: int, data: bytes) -> 'BvnFloat':
        f = cls(); lib = f._lib
        if width == 16:
            ok = lib.bvn_float_from_dec16(f._ptr, int.from_bytes(data, 'little'))
        elif width == 32:
            ok = lib.bvn_float_from_dec32(f._ptr, int.from_bytes(data, 'little'))
        elif width == 64:
            ok = lib.bvn_float_from_dec64(f._ptr, int.from_bytes(data, 'little'))
        elif width in (128, 256):
            n = width // 32
            words = [int.from_bytes(data[i:i + 4], 'little') for i in range(0, len(data), 4)]
            arr = (ctypes.c_uint32 * n)(*words)
            ok = (lib.bvn_float_from_dec128 if width == 128
                  else lib.bvn_float_from_dec256)(f._ptr, arr)
        else:
            raise BovnarArgumentError(f"no decimal interchange format for width {width}")
        if not ok:
            raise BovnarArgumentError("bvn_float_from_dec: malformed bits")
        return f

    # -- fixed-point Q-format ------------------------------------------------
    def to_fixed_mantissa(self, total_bits: int, frac_bits: int) -> int:
        """Signed integer datum m such that the value == m / 2**frac_bits."""
        lib = self._lib
        fb = int(frac_bits)
        if total_bits == 16:
            return int(lib.bvn_float_to_fix16(self._ptr, fb))
        if total_bits == 32:
            return int(lib.bvn_float_to_fix32(self._ptr, fb))
        if total_bits == 64:
            return int(lib.bvn_float_to_fix64(self._ptr, fb))
        if total_bits in (128, 256):
            n = total_bits // 32
            out = (ctypes.c_uint32 * n)()
            (lib.bvn_float_to_fix128 if total_bits == 128
             else lib.bvn_float_to_fix256)(self._ptr, fb, out)
            mag = int.from_bytes(b''.join(int(w).to_bytes(4, 'little') for w in out),
                                 'little', signed=True)
            return mag
        raise BovnarArgumentError(f"no fixed-point format for width {total_bits}")
