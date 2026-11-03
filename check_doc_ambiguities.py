#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_doc_ambiguities.py — every claim doc/07 makes, put to the reference parser.

doc/07 is the document a reader consults when a token could mean two things, and
its header promises exactly this:

    Every row here was checked against the reference parser; where a token is
    refused, it really is `error_unit_illegal`.

That was true when each table was written and nothing re-asked it, which is the
one property a table of REFUSALS cannot keep on its own: a refusal rots the
moment the catalogue grows. Adding a unit spelled `usb`, or an alias that made
`kWh` resolve, would leave §4 and §13 asserting an error the library no longer
raises — and in the direction no test notices, because nothing fails when a
document merely lies about a capability.

WHAT IS CHECKED, section by section. Each is the table's own content, not a
restatement of it:

  §2  a bare unit outranks a prefixed reading — the Token parses, and the
      "Write that as" spelling parses to a DIFFERENT unit (which is the whole
      claim: the two readings exist and are not the same one).
  §3  two prefixed readings compete — the same shape.
  §4  refused outright — the Token is `error_unit_illegal`, and every
      replacement offered beside it parses.
  §5  prefix symbol vs base unit symbol — the row's own claim, in two halves:
      the bare symbol parses as a unit, and `<symbol>~s` parses as a DIFFERENT
      unit, which is what "before another unit they are always the prefix"
      means. Everything the example cell offers in backticks must parse, and a
      compact/separated pair in it must agree; that last part is best-effort,
      since the cell is prose and may pair its spellings however it reads best.
  §9  same dimension, different quantity — the accept-list rows are mutually
      convertible and carry the dimension stated beside them; the refuse-list
      rows really are refused; and the five worked conversions return what the
      table says. The "families, in full" sentence is checked for CLOSURE too,
      since a new native unit of one of those dimensions joins the family
      whether or not anybody updates the sentence.
  §13 looks like a unit, is not one — every "Written" token is refused and every
      "Write instead" spelling parses; and the two tokens listed as accepted but
      surprising really are accepted.

Needs the built library, because the question is what the reader does; it skips
rather than fails when there is none, the same rule check_profile_factors.py
follows.

Usage:  python3 check_doc_ambiguities.py [repo-root]
Exit 0 when doc/07 and the library agree, 1 with a list when they do not.
"""
import os
import re
import sys

REPO = os.path.dirname(os.path.abspath(__file__))
DOC = "doc/07_bovnar_unit_ambiguities.md"

TICK = re.compile(r"`([^`\n]+)`")
SEPARATOR = re.compile(r"^\|[\s:|-]+\|$")

# Tables are found BY THEIR HEADER, so a table that is reshaped stops being
# checked loudly (the count in the summary drops) rather than being misread.
OUTRANKS_HEAD = ("Token", "Bovnar reads it as", "Might be mistaken for",
                 "Write that as")
COMPETES_HEAD = ("Token", "Bovnar reads it as", "The other split",
                 "Write that as")
REFUSED_HEAD = ("Token", "Would have resolved as", "Actually means, in the wild",
                "Write")
PREFIX_HEAD = ("Symbol", "As a bare unit", "As a prefix", "Example of each")
NOTAUNIT_HEAD = ("Written", "Intended", "Write instead")
SURPRISING_HEAD = ("Written", "Bovnar reads", "If you meant…")
ACCEPTS_HEAD = ("Units", "Shared dimension")
REFUSES_HEAD = ("Unit", "Kind", "Refused against")
HAZARD_HEAD = ("Written", "Read as", "Bovnar returns", "What just happened")

FAMILY_LINE = re.compile(r"The families, in full, are:(.*?)\(kg·m²·s⁻²\)", re.S)
FAMILY_GROUP = re.compile(r"((?:`[^`]+`\s*)+)\(([^)]+)\)")

# A cell may offer a spelling with commentary around it ("`k~t` (mass) or `kn`
# (speed)", "*(impossible: an SI prefix below kilo is illegal on `B`)*"). Every
# backticked token in such a cell is being offered to a reader as writable, so
# every one of them must parse -- except where the cell says it cannot be
# written at all, which is what this marks.
IMPOSSIBLE = "impossible"


def load_reader(repo):
    sys.path.insert(0, os.path.join(repo, "python"))
    try:
        import bovnar
        bovnar._ffi.load_library()
        return bovnar
    except Exception:
        return None


def rows(path):
    """(line number, header tuple, cells) for every markdown table row."""
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    header = None
    for n, line in enumerate(lines, 1):
        if not line.startswith("|"):
            header = None
            continue
        if SEPARATOR.match(line.strip()):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if header is None:
            header = tuple(cells)
            continue
        yield n, header, cells


class Checker(object):
    def __init__(self, bovnar):
        self.b = bovnar
        self.problems = []
        self.checked = 0

    def fail(self, n, msg):
        self.problems.append("%s:%d: %s" % (DOC, n, msg))

    def parse(self, expr):
        """(unit, None) or (None, reason)."""
        try:
            return self.b.parse_unit(expr), None
        except Exception as exc:
            return None, type(exc).__name__

    def key(self, u):
        return self.b.unit_to_str(u)

    def must_parse(self, n, expr, why):
        u, err = self.parse(expr)
        if u is None:
            self.fail(n, "`%s` %s, and the parser refuses it (%s)"
                         % (expr, why, err))
        return u

    def must_refuse(self, n, expr, why):
        u, _ = self.parse(expr)
        if u is not None:
            self.fail(n, "`%s` %s, and the parser accepts it as `%s`"
                         % (expr, why, self.key(u)))


def check(repo):
    path = os.path.join(repo, DOC)
    if not os.path.exists(path):
        print("check_doc_ambiguities: %s is absent; nothing to check" % DOC)
        return [], 0
    bovnar = load_reader(repo)
    if bovnar is None:
        print("check_doc_ambiguities: no built library; skipped")
        return [], 0

    sys.path.insert(0, repo)
    import bvnr_data
    with open(os.path.join(repo, "src", "gendata", "units.bvnr"), "rb") as f:
        catalogue = bvnr_data.load(f.read())["units"]

    c = Checker(bovnar)
    text = open(path, encoding="utf-8").read()

    for n, header, cells in rows(path):
        h = tuple(header)
        check_profile_claims(c, n, cells)

        # §2 and §3 have the same shape and the same claim: the token resolves
        # one way, and the OTHER reading is reachable only by spelling it out.
        if h in (OUTRANKS_HEAD, COMPETES_HEAD) and len(cells) >= 4:
            tok = TICK.findall(cells[0])
            if not tok:
                continue
            c.checked += 1
            u = c.must_parse(n, tok[0], "is listed as a unit in its own right")
            if IMPOSSIBLE in cells[3]:
                continue
            for alt in TICK.findall(cells[3]):
                v = c.must_parse(n, alt, "is offered as the other spelling")
                if u is not None and v is not None and c.key(u) == c.key(v):
                    c.fail(n, "`%s` and `%s` are listed as two readings and "
                              "are the same unit" % (tok[0], alt))

        elif h == REFUSED_HEAD and len(cells) >= 4:
            tok = TICK.findall(cells[0])
            if not tok:
                continue
            c.checked += 1
            c.must_refuse(n, tok[0], "is listed as refused outright")
            for alt in TICK.findall(cells[3]):
                c.must_parse(n, alt, "is offered as what to write instead")

        elif h == PREFIX_HEAD and len(cells) >= 4:
            sym = TICK.findall(cells[0])
            if not sym:
                continue
            c.checked += 1
            # The row's claim, in two halves: "bare, they are always the
            # unit; before another unit they are always the prefix." Both are
            # asserted directly rather than inferred from the example cell,
            # which is prose and may pair its spellings however it reads best.
            bare = c.must_parse(n, sym[0], "is listed as a bare unit")
            asprefix = c.must_parse(n, sym[0] + "~s",
                                    "is listed as a prefix, so `%s~s` must be "
                                    "one" % sym[0])
            if bare is not None and asprefix is not None:
                if c.key(bare) == c.key(asprefix):
                    c.fail(n, "`%s` and `%s~s` are the same unit, so the symbol "
                              "is not both a unit and a prefix"
                           % (sym[0], sym[0]))
            # Everything the example cell offers in backticks is being shown to
            # a reader as writable, and a compact/separated pair must agree.
            spellings = TICK.findall(cells[3])
            for s in spellings:
                c.must_parse(n, s, "is offered as an example spelling")
            for sep in [s for s in spellings if "~" in s]:
                flat = sep.replace("~", "")
                if flat not in spellings:
                    continue
                a, _ = c.parse(sep)
                b, _ = c.parse(flat)
                if a is not None and b is not None and c.key(a) != c.key(b):
                    c.fail(n, "`%s` and `%s` are offered as one unit and parse "
                              "to `%s` and `%s`"
                           % (flat, sep, c.key(b), c.key(a)))

        elif h == NOTAUNIT_HEAD and len(cells) >= 3:
            tok = TICK.findall(cells[0])
            if not tok:
                continue
            c.checked += 1
            for t in tok:                      # "`KB`, `Kg`" is one row
                c.must_refuse(n, t, "is listed as error_unit_illegal")
            for alt in TICK.findall(cells[2]):
                if alt.startswith("§"):
                    continue
                c.must_parse(n, alt, "is offered as what to write instead")

        elif h == SURPRISING_HEAD and len(cells) >= 2:
            tok = TICK.findall(cells[0])
            if not tok:
                continue
            c.checked += 1
            c.must_parse(n, tok[0], "is listed as accepted but surprising")

        elif h[:2] == ACCEPTS_HEAD and len(cells) >= 2:
            c.checked += 1
            check_accepts_row(c, n, cells, bovnar)

        elif h[:3] == REFUSES_HEAD and len(cells) >= 3:
            c.checked += 1
            check_refuses_row(c, n, cells)

        elif h[:4] == HAZARD_HEAD and len(cells) >= 3:
            c.checked += 1
            check_hazard_row(c, n, cells)

    check_families(c, text, catalogue)
    return c.problems, c.checked


def dimension_of(bovnar, expr):
    return tuple(bovnar.unit_dimension_vector(bovnar.parse_unit(expr)))


def check_accepts_row(c, n, cells, bovnar):
    """§9's accept-list: the row's units convert to each other, and the
    dimension stated beside them is the one they have. The second half is what
    prose cannot keep -- `10⁻⁴ T` for the gauss is only right because a tesla is
    kg·A⁻¹·s⁻²."""
    parsed = []
    for m in TICK.findall(cells[0]):
        u, err = c.parse(m)
        if u is None:
            c.fail(n, "`%s` does not parse (%s)" % (m, err))
        else:
            parsed.append((m, u))
    for i in range(len(parsed)):
        for j in range(i + 1, len(parsed)):
            if not c.b.units_convertible(parsed[i][1], parsed[j][1]):
                c.fail(n, "the row groups `%s` with `%s`, and they do not "
                          "convert" % (parsed[i][0], parsed[j][0]))
    try:
        want = dimension_of(bovnar, cells[1])
    except Exception:
        c.fail(n, "%r is not a dimension this gate can read" % cells[1])
        return
    for name, u in parsed:
        got = tuple(c.b.unit_dimension_vector(u))
        if got != want:
            c.fail(n, "`%s` is %s, not the %s the row states"
                      % (name, got, want))


def check_refuses_row(c, n, cells):
    """§9's refuse-list: nothing in the first column converts to anything in the
    third. A cell that opens with an italic parenthetical is a NOTE rather than
    a refusal list -- the ratio row's "(freely interconvertible)" says the
    opposite of every other row, so it is checked as the opposite."""
    subjects = []
    for s in TICK.findall(cells[0]):
        u, err = c.parse(s)
        if u is None:
            c.fail(n, "`%s` does not parse (%s)" % (s, err))
        else:
            subjects.append((s, u))
    if cells[2].startswith("*("):
        for i in range(len(subjects)):
            for j in range(i + 1, len(subjects)):
                if not c.b.units_convertible(subjects[i][1], subjects[j][1]):
                    c.fail(n, "the row calls `%s` and `%s` freely "
                              "interconvertible, and they are not"
                           % (subjects[i][0], subjects[j][0]))
        return
    for tname in TICK.findall(cells[2]):
        tu, err = c.parse(tname)
        if tu is None:
            c.fail(n, "`%s` does not parse (%s)" % (tname, err))
            continue
        for sname, su in subjects:
            if c.b.units_convertible(su, tu):
                c.fail(n, "the row says `%s` is refused against `%s`, and the "
                          "library converts them" % (sname, tname))


VOCABS = ("ucum", "unece", "qudt", "qudt-qk", "udunits", "om", "cf")
# "`ucum:st` → `m³`" — a mapping the row states outright.
MAPS_TO = re.compile(r"`((?:%s):[^`]+)`\s*(?:→|->)\s*`([^`]+)`" % "|".join(VOCABS))
PROFILE_CODE = re.compile(r"`((?:%s):[^`]+)`" % "|".join(VOCABS))


def check_profile_claims(c, n, cells):
    """Every `<vocab>:<code>` a row names, put to the parser.

    The tables in this document are checked column by column, and the prose
    column — "the trap", the whole point of the row — was not checked at all.
    It said of the letter `a`: "in `udunits:` the same letter is the prefix
    atto, and there is no bare `a` atom, so `udunits:a` is refused". There is a
    bare `a` atom, an alias of `are`, so `udunits:a` is `c~ha` — which makes
    that letter the one spelling in the table that means a DIFFERENT unit in
    each profile, and the row called it a refusal. In the document a reader
    consults precisely when a token could mean two things.

    Two claim shapes are checkable wherever they appear in a row:
    "`ucum:st` → `m³`" must map that way, and a code named in the same cell as
    the word "refused" must actually be refused.
    """
    for cell in cells:
        for code, target in MAPS_TO.findall(cell):
            c.checked += 1
            try:
                got = c.b.unit_to_str(c.b.parse_unit(code))
            except Exception:
                c.fail(n, "`%s` is stated to be `%s`; it does not resolve"
                          % (code, target))
                continue
            if got != target:
                c.fail(n, "`%s` is stated to be `%s`; it is `%s`"
                          % (code, target, got))
        if not re.search(r"\brefus", cell, re.I):
            continue
        for code in PROFILE_CODE.findall(cell):
            if MAPS_TO.search(cell) and code in [m[0] for m in MAPS_TO.findall(cell)]:
                continue
            c.checked += 1
            try:
                got = c.b.unit_to_str(c.b.parse_unit(code))
            except Exception:
                continue
            c.fail(n, "`%s` is described as refused; it resolves to `%s`"
                      % (code, got))


def check_hazard_row(c, n, cells):
    """§9's worked conversions: `1 Ci` in `Hz` really is 3.7e10."""
    written = TICK.findall(cells[0])
    target = TICK.findall(cells[1])
    if not written or not target:
        return
    m = re.match(r"^([\d.]+)\s+(.+)$", written[0])
    if not m:
        c.fail(n, "%r is not a `<value> <unit>` cell" % written[0])
        return
    value, from_u, to_u = float(m.group(1)), m.group(2), target[0]
    try:
        got = c.b.convert_value(value, c.b.parse_unit(from_u),
                                c.b.parse_unit(to_u))
    except Exception as exc:
        c.fail(n, "%s -> %s does not convert (%s) -- the row claims it does"
                  % (from_u, to_u, type(exc).__name__))
        return
    want = read_number(cells[2])
    if want is None:
        c.fail(n, "%r is not a number this gate can read" % cells[2])
    elif want == 0.0 or abs(got - want) > 1e-12 * abs(want):
        c.fail(n, "%g %s in %s is %.17g; the table says %s"
                  % (value, from_u, to_u, got, cells[2]))


SUPER = {"⁰": "0", "¹": "1", "²": "2", "³": "3", "⁴": "4", "⁵": "5",
         "⁶": "6", "⁷": "7", "⁸": "8", "⁹": "9", "⁻": "-", "⁺": "+"}


def read_number(cell):
    """The documents write a number as `3.7×10¹⁰`, `1/60` or `0.01`."""
    s = "".join(SUPER.get(ch, ch) for ch in cell.replace("`", "").strip())
    s = s.replace("×10", "e").replace("·10", "e").replace(" ", "")
    try:
        if "/" in s:
            num, den = s.split("/", 1)
            return float(num) / float(den)
        return float(s)
    except ValueError:
        return None


def check_families(c, text, catalogue):
    """§9's "the families, in full" sentence: each group is mutually
    convertible AND CLOSED. Closure is the half that rots on its own — a new
    native unit of one of those dimensions joins the family whether or not
    anybody updates the sentence."""
    m = FAMILY_LINE.search(text)
    if not m:
        c.problems.append("%s: §9's \"families, in full\" sentence is gone"
                          % DOC)
        return
    for members, _label in FAMILY_GROUP.findall(m.group(0)):
        names = TICK.findall(members)
        if len(names) < 2:
            continue
        c.checked += 1
        rep, err = c.parse(names[0])
        if rep is None:
            c.problems.append("%s: §9 names `%s`, which does not parse (%s)"
                              % (DOC, names[0], err))
            continue
        for other in names[1:]:
            u, _ = c.parse(other)
            if u is None or not c.b.units_convertible(u, rep):
                c.problems.append("%s: §9 lists `%s` in the same family as "
                                  "`%s`, and they do not convert"
                                  % (DOC, other, names[0]))
        listed = set(names)
        for u in catalogue:
            if u["symbol"] in listed:
                continue
            cand, _ = c.parse(u["symbol"])
            if cand is not None and c.b.units_convertible(cand, rep):
                c.problems.append(
                    "%s: §9's `%s` family omits `%s`, which converts to it — "
                    "the sentence says \"in full\""
                    % (DOC, names[0], u["symbol"]))


def main(argv):
    repo = os.path.abspath(argv[1]) if len(argv) > 1 else REPO
    sys.path.insert(0, REPO)
    import bvnr_build_env
    bvnr_build_env.require_build(repo, "check_doc_ambiguities", ("library",))
    problems, checked = check(repo)
    if problems:
        sys.stderr.write("check_doc_ambiguities: %s disagrees with the "
                         "reference parser:\n" % DOC)
        for p in problems:
            sys.stderr.write("    %s\n" % p)
        sys.stderr.write("\nThe parser is the source; %s says it was checked "
                         "against it.\n" % DOC)
        return 1
    print("check_doc_ambiguities: %s — %d table row(s) put to the reference "
          "parser, all as documented" % (DOC, checked))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
