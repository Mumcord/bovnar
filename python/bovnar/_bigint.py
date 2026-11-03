# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""Integer <-> digit text with no ``sys.int_max_str_digits`` limit.

The format holds integers up to 32768 bits -- about 9865 decimal digits -- but
CPython (3.11+) refuses ``int(text)`` and ``str(n)`` past 4300 digits in any
base that is not a power of two (CVE-2020-10735). A document this library wrote
itself, ``dumps({'a': 10**4300})``, then could not be read back as a number.

The limit is process-global and belongs to the application, so it is never
changed here. Instead the text is cut into pieces each well below the smallest
limit CPython accepts (640), and the pieces are combined arithmetically. That
is still sub-quadratic enough for 10 000 digits.
"""

from __future__ import annotations

__all__ = ['int_from_text', 'int_to_decimal', 'int_repr']

# Digits per piece: below 640, the lowest non-zero int_max_str_digits allowed.
_CHUNK = 512


def int_from_text(text: str, base: int = 10) -> int:
    """``int(text, base)`` for bases 2..36 without the digit-count limit.

    Accepts what ``int()`` accepts for the short case; a long text must be an
    optional sign followed by digits only (all a bovnar integer literal is).
    Raises ValueError on a malformed literal, like ``int()``.
    """
    if len(text) <= _CHUNK or base & (base - 1) == 0:
        return int(text, base)          # short, or a power-of-two base: exempt
    s = text.strip()
    neg = s[:1] == '-'
    if s[:1] in ('+', '-'):
        s = s[1:]
    if not s or not s.isascii() or not s.isalnum():
        raise ValueError(f"invalid literal for int() with base {base}: "
                         f"{text[:40]!r}...")
    step = base ** _CHUNK
    head = len(s) % _CHUNK or _CHUNK
    val = int(s[:head], base)
    for i in range(head, len(s), _CHUNK):
        val = val * step + int(s[i:i + _CHUNK], base)
    return -val if neg else val


def int_to_decimal(n: int) -> str:
    """``str(n)`` without the digit-count limit."""
    n = int(n)
    mag = -n if n < 0 else n
    # 1700 bits is ~512 decimal digits: comfortably under every limit.
    if mag.bit_length() <= 1700:
        return str(n)
    step = 10 ** _CHUNK
    pieces: list[str] = []
    while mag >= step:
        mag, r = divmod(mag, step)
        pieces.append(str(r).zfill(_CHUNK))
    pieces.append(str(mag))
    s = ''.join(reversed(pieces))
    return '-' + s if n < 0 else s


def int_repr(n: int, limit: int = 60) -> str:
    """A short, never-raising rendering of *n* for an error message: the full
    number when it is small, otherwise its leading digits and its size."""
    s = int_to_decimal(n)
    if len(s) <= limit:
        return s
    digits = len(s) - (s[0] == '-')
    return f"{s[:limit // 2]}...({digits} digits)"
