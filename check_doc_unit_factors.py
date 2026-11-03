#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_doc_unit_factors.py — the Factor column in doc/04 and doc/05, against
src/gendata/units.bvnr.

THE GAP THIS CLOSES. check_doc_unit_tables.py compares roughly 1150 documented
rows against gendata and says so in its own header: "Not checked: the Factor
column. It is prose as much as data -- '10⁻⁴ m²·s⁻¹', '101 325/760 Pa',
'2π rad', '1/9 000 000 kg/m' -- and a parser for it would be guessing at
intent."

That reasoning is right about the guessing and wrong about the conclusion. The
Factor column is the one column a reader USES: it is where somebody looking up
`inHg` learns the number to multiply by, and it is the column that decides
whether the documentation and the library agree about what a unit IS. Leaving it
ungated left a 462-cell surface where the library could be corrected and the
reference could not notice — and it does not need a parser that guesses, because
the cells are not free prose. They are a small grammar with a long tail:

    a scaled unit product     0.0254 m · 10⁻³ m³ · 3.7×10¹⁰ Bq · 1/9 000 000 kg/m
    a bare unit product       kg·m·s⁻² · cd·sr·m⁻² · mol·s⁻¹
    an irrational multiple    π/180 rad · 2π rad · 1000/(4π) A/m
    an affine definition      K = (°F + 459.67) × 5/9
    a temperature interval    5/9 K exactly
    prose                     SI base unit of time · reactive power; same SI dim as W

So the grammar is implemented, every cell that matches it is EVALUATED and
compared against the catalogue in exact rational arithmetic (with π carried
symbolically), and every cell that does not match must be one of the prose forms
in PROSE below. A cell that is neither is a failure — which is what stops the
"unparseable, so skipped" hole from re-opening one cell at a time.

WHAT IS COMPARED. The evaluated cell against `.factor` (and, for the affine
rows, against `.offset` too), plus the DIMENSION the cell's own unit product
implies against `.dims`. That second check is the one that catches a factor that
is numerically right and physically mislabelled — `10⁻⁴ T` for the gauss is only
correct because a tesla is kg·A⁻¹·s⁻², and if the row said `10⁻⁴ Wb` the number
would still be 10⁻⁴.

EXACTNESS IS COMPARED TOO, not just the value. A cell that says "(exact)" for a
unit units.bvnr marks `.exact = false` is a wrong claim about losslessness even
when the digits agree, and vice versa; both directions fail here.

doc/07's own claims — which tokens resolve, which are refused, and which units
share a dimension — are a different question about a different document, and
live in check_doc_ambiguities.py.

Usage:  python3 check_doc_unit_factors.py [repo-root]
Exit 0 when every checked cell agrees with src/gendata, 1 with a list when not.
"""
import os
import re
import sys
from fractions import Fraction

REPO = os.path.dirname(os.path.abspath(__file__))

DOCS = ("doc/04_bovnar_unit_cheatsheet.md", "doc/05_bovnar_unit_system.md")

# Column labels that hold a factor/definition. "Notes" is in the list because
# §3.3's Notes column carries "10⁻³ m³" and "60 s"; §3.1's carries prose, and
# the prose classifier below is what tells the two apart.
FACTOR_COLS = ("Factor", "SI Definition", "SI definition", "Conversion",
               "SI equivalent", "Factor / notes", "Notes")

# ── the value type: a rational times an integer power of π ────────────────
# Every factor in the catalogue is either exactly rational or a rational
# multiple of a power of π (the angles, and the oersted's 1000/4π). Carrying the
# π exponent symbolically is what lets "π/180 rad" be compared to the catalogue
# EXACTLY instead of through a float, so a doc that said π/181 could not hide
# inside a tolerance.


class Val(object):
    __slots__ = ("q", "pi")

    def __init__(self, q, pi=0):
        self.q = Fraction(q)
        self.pi = pi

    def __mul__(self, o):
        return Val(self.q * o.q, self.pi + o.pi)

    def __truediv__(self, o):
        if o.q == 0:
            raise ValueError("division by zero")
        return Val(self.q / o.q, self.pi - o.pi)

    def __pow__(self, n):
        return Val(self.q ** n, self.pi * n)

    def __eq__(self, o):
        return self.q == o.q and self.pi == o.pi

    def __repr__(self):
        if self.pi == 0:
            return str(self.q)
        return "%s·π%s" % (self.q, "" if self.pi == 1 else "^%d" % self.pi)

    def as_float(self):
        import math
        return float(self.q) * (math.pi ** self.pi)


ONE = Val(1)
PI = Val(1, 1)

# ── normalisation ─────────────────────────────────────────────────────────
SUPERSCRIPT = {
    "⁰": "0", "¹": "1", "²": "2", "³": "3", "⁴": "4",
    "⁵": "5", "⁶": "6", "⁷": "7", "⁸": "8", "⁹": "9",
    "⁻": "-", "⁺": "+",
}
# Digit-group separators the documents use inside numbers: a thin space
# (U+202F), a non-breaking space (U+00A0) and, in the older tables, a plain
# space. Only removed BETWEEN digits, so "1852 m" keeps its separator.
GROUP_SEP = "[   ]"


def normalise(cell):
    """Fold a documented cell into the ASCII shape the grammar below parses.

    Superscript runs become `^…`, the multiplication and middle dots become
    `*`, the Unicode minus becomes `-`, and digit grouping inside a number is
    removed. Everything else is left alone so the prose classifier still sees
    what was written."""
    out = []
    i = 0
    while i < len(cell):
        ch = cell[i]
        if ch in SUPERSCRIPT:
            run = ""
            while i < len(cell) and cell[i] in SUPERSCRIPT:
                run += SUPERSCRIPT[cell[i]]
                i += 1
            out.append("^(" + run + ")")
            continue
        out.append(ch)
        i += 1
    s = "".join(out)
    s = s.replace("**", "")        # markdown bold
    s = s.replace("×", "*")        # ×
    s = s.replace("·", "*")        # ·
    s = s.replace("−", "-")        # −
    s = s.replace("⁄", "/")        # ⁄
    s = s.replace("–", "-")        # –
    s = s.replace("`", "")
    # digit grouping, repeatedly (three-digit groups chain)
    while True:
        t = re.sub(r"(?<=\d)" + GROUP_SEP + r"(?=\d)", "", s)
        if t == s:
            break
        s = t
    return s


# ── the unit-expression evaluator ─────────────────────────────────────────
# A cell's tail names a unit product. Its value is the product of each factor's
# own SI factor, and its dimension the sum of their dimension vectors, so a cell
# can be checked for BOTH at once.

DIM_NAMES = ["length", "mass", "time", "current",
             "temperature", "amount", "luminosity"]


class Catalogue(object):
    def __init__(self, repo):
        sys.path.insert(0, repo)
        import bvnr_data
        with open(os.path.join(repo, "src", "gendata", "units.bvnr"), "rb") as f:
            self.units = bvnr_data.load(f.read())["units"]
        with open(os.path.join(repo, "src", "gendata", "prefixes.bvnr"), "rb") as f:
            pfx = bvnr_data.load(f.read())
        self.by_spelling = {}
        for u in self.units:
            for a in u["aliases"]:
                self.by_spelling[a] = u
        self.prefixes = {}
        for p in pfx["si_prefixes"]:
            for a in p["aliases"]:
                self.prefixes[a] = ("si", p["exp"])
        for p in pfx["iec_prefixes"]:
            for a in p["aliases"]:
                self.prefixes[a] = ("iec", p["exp"])

    def exact_factor(self, u, key="factor"):
        """The catalogue's own exact value for a unit's factor or offset."""
        n, d = u.get(key + "_num"), u.get(key + "_den")
        if n is not None and d is not None:
            return Val(Fraction(int(n), int(d)))
        from decimal import Decimal
        return Val(Fraction(Decimal(repr(u[key]))))

    def dims(self, u):
        return tuple(u["dims"].get(n, 0) for n in DIM_NAMES)


# The π-based units: the catalogue stores a rounded double and marks
# `.exact = false`, so the true value has to be stated here to compare a
# documented "π/180 rad" against anything but that rounding. Each is the
# unit's DEFINITION, which is what the document states as well.
IRRATIONAL = {
    "degree":    Val(Fraction(1, 180), 1),
    "arcminute": Val(Fraction(1, 10800), 1),
    "arcsecond": Val(Fraction(1, 648000), 1),
    "grad":      Val(Fraction(1, 200), 1),
    "revolution": Val(2, 1),
    "oersted":   Val(Fraction(1000, 4), -1),
    "parsec":    Val(Fraction(648000 * 149597870700), -1),
}


class Unparseable(Exception):
    pass


TOKEN = re.compile(r"""
      (?P<num>\d+(?:\.\d*)?(?:[eE][-+]?\d+)?|\.\d+)
    | (?P<pi>pi|π)
    | (?P<op>[*/^()])
    | (?P<sym>[^\s*/^()=]+)
    | (?P<ws>\s+)
""", re.VERBOSE)

ZERO_DIM = (0,) * len(DIM_NAMES)


def dim_mul(a, b):
    return tuple(x + y for x, y in zip(a, b))


def dim_div(a, b):
    return tuple(x - y for x, y in zip(a, b))


def dim_pow(a, n):
    return tuple(x * n for x in a)


def tokenise(s):
    toks = []
    i = 0
    while i < len(s):
        m = TOKEN.match(s, i)
        if not m:
            raise Unparseable("stray %r" % s[i])
        i = m.end()
        if m.lastgroup == "ws":
            continue
        toks.append((m.lastgroup, m.group(0)))
    return toks


class Evaluator(object):
    """Evaluate a normalised cell as `value * unit-product`.

    One grammar covers both halves, because they are the same shape: a product
    of factors, each a number, a pi, or a unit symbol with an optional prefix
    and an optional exponent. Every level returns (Val, dimension vector)
    together, so a unit under a division sign has its dimension subtracted
    rather than added -- which is the whole point of checking the dimension at
    all: `1852/3600 m/s` and `1852/3600 m*s` carry the same number.
    """

    def __init__(self, cat):
        self.cat = cat

    def eval(self, text):
        self.toks = tokenise(text)
        self.i = 0
        self.saw_unit = False
        v, d = self.product()
        if self.i != len(self.toks):
            raise Unparseable("trailing %r" % (self.toks[self.i],))
        return v, d

    def peek(self):
        return self.toks[self.i] if self.i < len(self.toks) else (None, None)

    def product(self):
        v, d = self.power()
        while True:
            kind, tok = self.peek()
            if kind == "op" and tok in "*/":
                self.i += 1
                rv, rd = self.power()
                if tok == "*":
                    v, d = v * rv, dim_mul(d, rd)
                else:
                    v, d = v / rv, dim_div(d, rd)
            elif kind in ("num", "pi", "sym") or (kind == "op" and tok == "("):
                # juxtaposition is multiplication: "3.7*10^(10) Bq", "2 pi"
                rv, rd = self.power()
                v, d = v * rv, dim_mul(d, rd)
            else:
                return v, d

    def power(self):
        v, d = self.atom()
        kind, tok = self.peek()
        if kind == "op" and tok == "^":
            self.i += 1
            e = self.exponent()
            v, d = v ** e, dim_pow(d, e)
        return v, d

    def exponent(self):
        kind, tok = self.peek()
        if kind == "op" and tok == "(":
            self.i += 1
            n = self.signed_int()
            if self.peek() != ("op", ")"):
                raise Unparseable("exponent )")
            self.i += 1
            return n
        return self.signed_int()

    def signed_int(self):
        kind, tok = self.peek()
        # A leading minus rides in the `sym` token, since `-` is not an operator
        # in this grammar (nothing in these cells subtracts).
        if kind == "sym" and re.match(r"^-\d+$", tok):
            self.i += 1
            return int(tok)
        if kind == "num":
            self.i += 1
            return int(tok)
        raise Unparseable("exponent")

    def atom(self):
        kind, tok = self.peek()
        if kind == "op" and tok == "(":
            self.i += 1
            v, d = self.product()
            if self.peek() != ("op", ")"):
                raise Unparseable("unbalanced (")
            self.i += 1
            return v, d
        if kind == "num":
            self.i += 1
            return Val(Fraction(tok)), ZERO_DIM
        if kind == "pi":
            self.i += 1
            return PI, ZERO_DIM
        if kind == "sym":
            self.i += 1
            return self.unit(tok)
        raise Unparseable("atom %r" % (tok,))

    def unit(self, tok):
        """A unit symbol, with an optional SI/IEC prefix and an optional
        trailing digit exponent written without a caret ("m2", "in3")."""
        exp = 1
        m = re.match(r"^(.*?)(-?\d+)$", tok)
        if m and m.group(1) and self.lookup(m.group(1)) is not None:
            tok, exp = m.group(1), int(m.group(2))
        found = self.lookup(tok)
        if found is None:
            raise Unparseable("unknown unit %r" % tok)
        scale, u = found
        self.saw_unit = True
        base = IRRATIONAL.get(u["name"]) or self.cat.exact_factor(u)
        d = self.cat.dims(u)
        return (scale * base) ** exp, dim_pow(d, exp)

    def lookup(self, tok):
        """(prefix scale, unit record) for a spelling, or None. `kg` resolves
        through the prefix table exactly as the parser does."""
        u = self.cat.by_spelling.get(tok)
        if u is not None:
            return ONE, u
        for plen in (2, 1):
            if len(tok) > plen and tok[:plen] in self.cat.prefixes:
                rest = self.cat.by_spelling.get(tok[plen:])
                if rest is not None:
                    system, e = self.cat.prefixes[tok[:plen]]
                    scale = Val(Fraction(10) ** e if system == "si"
                                else Fraction(2) ** e)
                    return scale, rest
        return None


# ── cell classification ───────────────────────────────────────────────────
# Cells that are genuinely prose, and what each is allowed to say. A cell is
# matched against these ONLY after the grammar has declined it, and a cell that
# matches neither is a failure -- so a new unparseable phrasing has to be
# either fixed or added here deliberately.
PROSE = (
    r"^SI base unit of .*$",
    r"^SI base unit of mass is kg; g carries the prefix$",
    r"^dimensionless.*$",
    r"^reactive power; same SI dim(ension)? as W$",
    r"^apparent power; same SI dim(ension)? as W$",
    r"^1 \(dimensionless.*\)$",
)

# The trailing commentary a cell may carry after its value, stripped before
# evaluation. Parentheticals are handled by balanced matching rather than by a
# regex: the content nests ("(= 1 kgf/cm²)" normalises to "(= 1 kgf/cm^(2))"),
# and a `[^)]*` rule both stops at the inner paren and — worse — matched the
# `(-1)` of every negative exponent, which silently turned "s^(-1)" into "s^".
TRAILERS = (
    # An em/en dash or a "--" run, SURROUNDED BY SPACE: "— U+00B0",
    # "-- also §3.2". The spacing requirement is load-bearing -- a bare `-`
    # rule swallowed the minus of every negative exponent.
    r"\s+(?:[—–]|--)\s.*$",
    r"\s*;.*$",                          # "; does not convert to dB"
    r"\s*≈.*$",                          # "≈ 0.30480061 m"
    r"\.\s+[A-Z\u0394`].*$",              # ". `Δ°C` is this unit"
)

# What a trailing parenthetical is allowed to open with. Anything else stays,
# and the cell then fails to parse rather than being quietly trimmed.
PAREN_OPENERS = (
    "exact", "conventional", "linear", "affine", "=", "ISO ", "CODATA ",
    "Julian ", "CGPM ", "ITU-T ", "plane angle", "solid angle", "Delisle ",
    "the ", "dimensionless", "U+", "or ", "no prefix",
)


def strip_paren(s):
    """Remove one trailing balanced parenthetical whose content is commentary."""
    s = s.rstrip()
    if not s.endswith(")"):
        return s, False
    depth = 0
    for i in range(len(s) - 1, -1, -1):
        if s[i] == ")":
            depth += 1
        elif s[i] == "(":
            depth -= 1
            if depth == 0:
                inner = s[i + 1:-1].strip()
                if any(inner.startswith(o) for o in PAREN_OPENERS):
                    return s[:i].rstrip(), True
                return s, False
    return s, False


def strip_trailers(s):
    changed = True
    while changed:
        # Parentheticals first. The ";" rule below would otherwise cut
        # "(exact; 25½ Zoll)" in half and leave an unbalanced "(exact".
        s, changed = strip_paren(s)
        for pat in TRAILERS:
            t = re.sub(pat, "", s)
            if t != s:
                s, changed = t, True
    return s.strip()


AFFINE = re.compile(
    r"^K\s*=\s*(?:\(\s*(?P<sym1>[^\s)+-]+)\s*(?P<sign>[-+])\s*(?P<off1>[\d.]+)\s*\)"
    r"|(?P<sym2>[^\s*=+-]+))"
    r"\s*(?:[*x]\s*(?P<slope>[\d./]+))?"
    r"\s*(?:(?P<osign>[-+])\s*(?P<off2>[\d.]+))?\s*$")
# "K = 373.15 - °De × 2/3" runs the other way round.
AFFINE_REV = re.compile(
    r"^K\s*=\s*(?P<base>[\d.]+)\s*-\s*(?P<sym>[^\s*]+)\s*[*x]\s*(?P<slope>[\d./]+)\s*$")

INTERVAL = re.compile(r"^(?P<val>-?[\d./]+)\s*K exactly\b")


def rat(s):
    if "/" in s:
        n, d = s.split("/", 1)
        return Fraction(Fraction(n), Fraction(d))
    from decimal import Decimal
    return Fraction(Decimal(s))


# ── the tables ────────────────────────────────────────────────────────────
SEPARATOR = re.compile(r"^\|[\s:|-]+\|$")
TICK = re.compile(r"`([^`]*)`")


def rows(path):
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    header = None
    for n, line in enumerate(lines, 1):
        if not line.startswith("|"):
            header = None
            continue
        if SEPARATOR.match(line):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if header is None:
            header = cells
            continue
        if "Symbol" not in header:
            continue
        si = header.index("Symbol")
        fi = None
        for j, label in enumerate(header):
            if label in FACTOR_COLS:
                fi = j
        if fi is None or fi >= len(cells) or si >= len(cells):
            continue
        # The row must name a `bu_*` enumerator. Without this the SI/IEC prefix
        # tables come through as unit rows -- their Symbol column holds `R`,
        # `P`, `T`, `G`, `h`, `d`, `m`, which are all real unit spellings, and
        # their Factor column ("10²⁷") would then be compared against the
        # roentgen.
        if not any("bu_" in c for c in cells):
            continue
        yield n, cells[si], cells[fi]


# Every unit error code a document names must be REACHABLE, and be what the
# reader actually raises for the case the row describes.
#
# The codes are a small closed set and three documents tabulate them, which is
# exactly the shape that rots: doc/05 §8 and §14, doc/03 §12.8 and doc/08 §5 all
# said an over-long unit string raises `error_unit_too_long`, and an over-long
# unit written in an ANNOTATION raises `error_type_too_long` — the type body has
# its own 255-byte cap and it counts the family name and the commas too, so a
# unit parameter can never be the only thing over the line. doc/11 §2.5 had it
# right and the other three did not.
#
# Only the codes are checked, against a document that provokes each. What a row
# SAYS is prose; that the code it names is the one the reader raises is not.
ERROR_CASES = [
    ("error_unit_illegal",             ".a = <float:64,notaunit> 1.0;"),
    ("error_unit_illegal",             ".a = <float:64,m^0> 1.0;"),
    ("error_unit_illegal",             ".a = <float:64,m^200> 1.0;"),
    ("error_unit_illegal",             ".a = <float:64,kt> 1.0;"),
    ("error_unit_illegal",             ".a = <float_dec:64,USD> 1.0;"),
    ("error_unit_illegal",             ".a = <float:64,%s> 1.0;" % "·".join(["m"] * 33)),
    ("error_unit_mismatch",            ".a = <float:64,m/s> 1.0 k~g;"),
    ("error_unit_profile_unknown",     "#!bovnar 1.2\n.a = <float:64,nosuchns:m> 1.0;"),
    ("error_unit_profile_unsupported", "#!bovnar 1.2\n.a = <float:64,ucum:[Btu_39]> 1.0;"),
    # The pair the documents had backwards. Both are provoked, so neither can be
    # restated as the other without this failing.
    ("error_unit_too_long",            ".a = <float:64> 1.0 %s;" % "·".join(["m"] * 400)),
    ("error_type_too_long",            ".a = <float:64,%s> 1.0;" % "·".join(["m"] * 400)),
]

# doc/08 §1.10's table, which is about the want_unit HOOK rather than the plain
# parse — a different code path, and one none of the cases above reaches. Each
# row names the error the hook raises for a condition, and the whole point of
# the table is "the value either arrives converted or the parse STOPS", so a row
# that quietly stopped being true would be the silent-skip the section promises
# cannot happen. (source unit, hook target, documented error).
HOOK_ERROR_CASES = [
    # dimensionally incompatible — seconds for a length, and one currency for
    # another, which the row names as the same outcome for different reasons.
    ("m",     "s",    "error_unit_mismatch"),
    ("$USD",  "$EUR", "error_unit_mismatch"),
    # irrational factor: a π-based angle.
    ("°",     "rad",  "error_unit_inexact"),
    # an affine scale inside a compound has nowhere to put its offset.
    ("°C/h",  "K/h",  "error_unit_mismatch"),
    # exact, but no terminating expansion in the output base, with
    # want_unit_allow_nonterminating off.
    ("m",     "mi",   "error_unit_inexact"),
]


def check_hook_error_codes(repo):
    """-> (problems, cases checked). doc/08 §1.10's want_unit error table."""
    sys.path.insert(0, os.path.join(repo, "python"))
    try:
        import bovnar
        from bovnar.enums import ErrorCode
        bovnar._ffi.load_library()
    except Exception:                                         # noqa: BLE001
        return [], 0
    problems, checked = [], 0
    for unit, target, want in HOOK_ERROR_CASES:
        src = (".a = <float:64,%s> 1.0;" % unit).encode("utf-8")
        got = None
        try:
            with bovnar.Reader() as r:
                r.read_mem(src,
                           want_unit=lambda data, t=target: bovnar.parse_unit(t))
        except Exception as exc:                              # noqa: BLE001
            code = getattr(exc, "code", None)
            got = ErrorCode(code).name.lower() if code is not None else None
        checked += 1
        if got is None:
            problems.append("doc/08 §1.10 says a want_unit hook asking for %r on "
                            "a %r value is %s; the parse did not stop"
                            % (target, unit, want))
        elif ("error_" + got) != want:
            problems.append("doc/08 §1.10 says a want_unit hook asking for %r on "
                            "a %r value is %s; the reader raises error_%s"
                            % (target, unit, want, got))
    # ...and the other half of the same promise: nan/inf are handed over
    # UNTOUCHED rather than refused, with converted == false.
    for literal in ("nan", "inf", "ninf"):
        seen = []
        try:
            with bovnar.Reader() as r:
                r.read_mem((".a = <float:64,m> %s;" % literal).encode(),
                           want_unit=lambda data: bovnar.parse_unit("k~m"),
                           on_verified=lambda ev, d: seen.append(d.converted)
                           if int(ev) == 9 else None)
        except Exception as exc:                              # noqa: BLE001
            problems.append("doc/08 §1.10 says %r is handed over untouched; the "
                            "parse raised %s" % (literal, type(exc).__name__))
            continue
        checked += 1
        if seen and seen[0]:
            problems.append("doc/08 §1.10 says %r arrives with converted == "
                            "false; it arrived converted" % literal)
    return problems, checked


def check_error_codes(repo):
    """-> (problems, cases checked). Needs the built library; skips without it."""
    sys.path.insert(0, os.path.join(repo, "python"))
    try:
        import bovnar
        from bovnar.enums import ErrorCode
        bovnar._ffi.load_library()
    except Exception:
        return [], 0
    problems = []
    for want, src in ERROR_CASES:
        try:
            bovnar.loads(src)
            problems.append("%r is documented as %s and the reader accepts it"
                            % (src[:44], want))
            continue
        except Exception as exc:
            code = getattr(exc, "code", None)
            got = ErrorCode(code).name.lower() if code is not None else None
        if got is None:
            problems.append("%r raised no bovnar error code" % src[:44])
        elif ("error_" + got) != want:
            problems.append("%r is documented as %s; the reader raises error_%s"
                            % (src[:44], want, got))
    return problems, len(ERROR_CASES)


def agrees_as_double(got, want, unit):
    """True when a documented value differs from the catalogue's only by
    rounding to the same IEEE-754 double.

    The documents are allowed to state a decimal where the catalogue keeps an
    exact rational -- `psi` is 8 896 443 230 521/1 290 320 000 and no decimal
    spells it -- but only when the decimal is the one a reader would compute:
    the same double. A value truncated a few digits early (`hp` as
    745.69987158227, which is a DIFFERENT double from 745.699871582270 25) is
    not that, and is reported."""
    if got.pi != want.pi:
        # A unit whose true factor is irrational (the π-based angles, the
        # oersted, the parsec, the luminance and circular-mil rows) cannot be
        # written as a decimal at all, so a document that states one is stating
        # the same rounding units.bvnr stores. That is allowed; stating a
        # DIFFERENT rounding is not.
        #
        # A FEW ULP of slack, not bit-equality. There is no exact value to be
        # bit-equal to: `π/4 thou²` evaluated as (rational × π) and the same
        # quantity evaluated as a chain of doubles land one ulp apart, and
        # neither is more correct than the other. The bound is still four orders
        # tighter than any real disagreement -- a documented value that is wrong
        # rather than differently rounded is out by digits, not by an ulp.
        if unit.get("exact", True) is not False:
            return False
        f = unit["factor"]
        return f != 0.0 and abs(got.as_float() - f) <= 1e-15 * abs(f)
    return float(got.q) == float(want.q)


def check(repo):
    cat = Catalogue(repo)
    ev = Evaluator(cat)
    problems = []
    checked = prose = 0
    for rel in DOCS:
        path = os.path.join(repo, rel)
        if not os.path.exists(path):
            continue
        for lineno, symcell, factcell in rows(path):
            spellings = TICK.findall(symcell)
            unit = None
            for s in spellings:
                if s in cat.by_spelling:
                    unit = cat.by_spelling[s]
                    break
            if unit is None:
                continue          # a prefix table row, not a unit row
            where = "%s:%d  %-10s" % (rel, lineno, unit["name"])
            raw = normalise(factcell)
            claims_exact = "exact" in raw
            body = strip_trailers(raw)

            # 1. an affine or interval temperature definition
            m = INTERVAL.match(body)
            if m:
                got = Val(rat(m.group("val")))
                want = cat.exact_factor(unit)
                checked += 1
                if got != want:
                    problems.append("%s interval %s, catalogue %s"
                                    % (where, got, want))
                continue
            m = AFFINE_REV.match(body) or AFFINE.match(body)
            if m and unit["affine"] or (m and body.startswith("K =")):
                g = m.groupdict()
                if "base" in g:                      # K = 373.15 - °De × 2/3
                    slope = -rat(g["slope"])
                    offset = Fraction(rat(g["base"]))
                else:
                    slope = rat(g["slope"]) if g.get("slope") else Fraction(1)
                    offset = Fraction(0)
                    if g.get("off1"):                # K = (°F + 459.67) × 5/9
                        inner = rat(g["off1"])
                        if g["sign"] == "-":
                            inner = -inner
                        offset = inner * slope
                    if g.get("off2"):                # K = °N × 100/33 + 273.15
                        o = rat(g["off2"])
                        offset += o if g["osign"] == "+" else -o
                checked += 1
                wf, wo = cat.exact_factor(unit), cat.exact_factor(unit, "offset")
                if Val(slope) != wf:
                    problems.append("%s slope %s, catalogue %s"
                                    % (where, slope, wf))
                if Val(offset) != wo:
                    problems.append("%s offset %s, catalogue %s"
                                    % (where, offset, wo))
                continue

            # 2. a scaled unit product. A cell may state the same value twice
            #    with an `=` between the two spellings ("268.8025 in³ =
            #    4.40488377086×10⁻³ m³", "12 000 `Btu`/h = 52752792631/15000000
            #    W"). Both sides are evaluated and BOTH are compared, so the
            #    restatement is checked rather than skipped.
            sides, unparsed = [], []
            for part in [p.strip() for p in body.split("=")]:
                if not part:
                    continue
                try:
                    sides.append(ev.eval(part) + (ev.saw_unit,))
                except (Unparseable, ValueError, ZeroDivisionError, IndexError):
                    unparsed.append(part)
            if not sides or (unparsed and len(sides) < 1):
                if any(re.match(p, factcell) for p in PROSE):
                    prose += 1
                    continue
                problems.append("%s cell is neither a factor nor a known prose "
                                "form: %r" % (where, factcell))
                continue
            # A side that does not parse is only tolerated when another does and
            # the unparsed text is a bare word ("1 symbol/s"): a NUMBER that
            # fails to parse is a defect, not commentary.
            for part in unparsed:
                if re.search(r"[\d.]", re.sub(r"^\d+\s+", "", part)):
                    problems.append("%s side %r of an `=` cell does not parse"
                                    % (where, part))

            want = IRRATIONAL.get(unit["name"]) or cat.exact_factor(unit)
            wantdim = cat.dims(unit)
            for got, dim, saw_unit in sides:
                checked += 1
                if got != want and not agrees_as_double(got, want, unit):
                    problems.append(
                        "%s factor %s (= %.17g), catalogue %s (= %.17g)"
                        % (where, got, got.as_float(), want, want.as_float()))
                elif got != want and claims_exact:
                    # The digits round to the same double, so the VALUE is not
                    # wrong -- but calling a rounded decimal exact is.
                    problems.append("%s says (exact) and states %s, which is a "
                                    "rounding of the catalogue's %s"
                                    % (where, got, want))
                if saw_unit and dim != wantdim:
                    problems.append("%s dimension %s, catalogue %s"
                                    % (where, dim, wantdim))
            # "(exact)" is a claim about losslessness, so it is checked too.
            if claims_exact and not unit.get("exact", True):
                problems.append("%s says (exact); units.bvnr marks it "
                                ".exact = false" % where)
    return problems, checked, prose


# Every function `bovnar_si_units.h` exports must be NAMED in doc/05, which is
# the unit system's own reference. Four were not: bvn_units_convertible and
# bvn_unit_si_normal_form (the two the reader's unit policy is built on, and the
# two whose header comments say every hand-written want_unit hook gets the
# screening wrong without them) and the two rational-rendering helpers, which
# live in doc/08 and only needed a pointer from here.
#
# Naming is all this asks. Whether the description is right is a reading, not a
# gate -- but a function the reference never mentions cannot be found at all,
# and that is checkable.
API_HEADER = "include/bovnar_si_units.h"
API_DOC = "doc/05_bovnar_unit_system.md"
BVN_API = re.compile(r"BVN_API\s+[\w \*]*?\b(\w+)\s*\(")


def check_api_named(repo):
    header = os.path.join(repo, API_HEADER)
    doc = os.path.join(repo, API_DOC)
    if not (os.path.exists(header) and os.path.exists(doc)):
        return [], 0
    with open(header, encoding="utf-8") as f:
        exported = BVN_API.findall(f.read())
    with open(doc, encoding="utf-8") as f:
        text = f.read()
    missing = [fn for fn in exported if fn not in text]
    return (["%s exports %s and %s never names it"
             % (API_HEADER, fn, API_DOC) for fn in missing], len(exported))


def main(argv):
    repo = argv[1] if len(argv) > 1 else REPO
    sys.path.insert(0, REPO)
    import bvnr_build_env
    bvnr_build_env.require_build(repo, "check_doc_unit_factors", ("library",))
    problems, checked, prose = check(repo)
    api_problems, api_count = check_api_named(repo)
    problems += api_problems
    err_problems, err_count = check_error_codes(repo)
    hook_problems, hook_count = check_hook_error_codes(repo)
    err_problems = err_problems + hook_problems
    err_count += hook_count
    problems += err_problems
    if problems:
        sys.stderr.write(
            "check_doc_unit_factors: %d documented factor(s) disagree with "
            "src/gendata/units.bvnr:\n\n" % len(problems))
        for p in problems:
            sys.stderr.write("  %s\n" % p)
        sys.stderr.write("\n%d cell(s) checked, %d prose.\n" % (checked, prose))
        return 1
    print("check_doc_unit_factors: %d factor cell(s) checked against "
          "src/gendata, %d prose, all matching; %d bovnar_si_units.h "
          "export(s) named in %s; %d unit error code(s) provoked."
          % (checked, prose, api_count, API_DOC, err_count))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
