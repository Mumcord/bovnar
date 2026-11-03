#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_doc_unit_tables.py — the unit and currency tables in doc/ against gendata.

doc/04 (the cheatsheet) and doc/05 (the unit system) between them carry every one
of the 264 physical units and all 216 currencies, twice over, as hand-written
markdown: a canonical symbol, the accepted long forms, the bu_* enumerator, and
for money the ISO numeric code and minor-unit count. Roughly 1150 rows.

Nothing checked any of it. The C tables, the parse tables and the Python
catalogue are all generated from or gated against src/gendata/*.bvnr — this
repo's whole convention — and these two documents were the copy nobody compared.
Rename a unit, add an alias, correct a minor-unit digit, and the build stays
green while the reference a reader trusts says something else. For money that is
not cosmetic: `minor_unit` is the field an application formats amounts with.

What is checked, per row that names one:

  units       the Symbol column's first spelling IS the unit's canonical symbol;
              every other backticked spelling in the Symbol and Long-forms
              columns is a spelling the parser actually accepts; the enumerator
              exists.
  currencies  the ISO numeric code and the minor-unit count match, and the Name
              column starts with the catalogue's name (documents append
              annotations like "*(historical; retired 2023-01-01)*").
  coverage    every unit and every currency appears in each document, and no row
              names something gendata does not define.

Also checked, in a document that has no `bu_*` column to key on: doc/09 §18
summarises the whole BaseUnit enum as a table of id RANGES, so the coverage rule
above cannot see it. Contiguity is checked instead -- the ranges must tile block
10 and block 90 end to end.

Not checked here: the Factor column. It is a grammar rather than free prose and
it has its own gate, check_doc_unit_factors.py, which evaluates every cell in
exact rational arithmetic and compares the value, the dimension and the
"(exact)" claim. The factors are gated on the other side too:
test_unit_factors_derived.py checks the generated C table against units.bvnr,
and check_profile_factors.py checks the profiles against their publishers' own
files.

Usage:  python3 check_doc_unit_tables.py [repo-root]
Exit 0 when the documents agree with gendata, 1 with a list when they do not.
"""
import os
import re
import sys

REPO = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, REPO)
import bvnr_data  # noqa: E402

DOCS = ("doc/04_bovnar_unit_cheatsheet.md", "doc/05_bovnar_unit_system.md")

# A row is checked only when its table's header says what its columns mean, so a
# table with a different shape (the "Written in BVNR" walkthrough of the CUP
# case, say) is skipped rather than misread. Columns are found BY LABEL, not by
# position: the unit tables come in a dozen shapes -- "Long form" and "Long
# forms", with and without a Name column, and a last column that is variously
# Factor, Notes, Conversion, SI equivalent or Method -- and pinning positions
# would silently skip the ones that did not match.
SYMBOL_COL = ("Symbol",)
LONGFORM_COL = ("Long form", "Long forms")
ENUM_COL = ("Enum", "Enum value")
FIAT_HEAD = ("Code", "Num", "Min", "Name")
CRYPTO_HEAD = ("Code", "Min", "Subunit", "Name")


def column(header, names):
    """Index of the first column whose label is one of `names`, or None."""
    for i, label in enumerate(header):
        if label in names:
            return i
    return None

TICK = re.compile(r"`([^`]*)`")
SEPARATOR = re.compile(r"^\|[\s:|-]+\|$")


def cells(line):
    return [c.strip() for c in line.strip().strip("|").split("|")]


def tables(path):
    """Yield (line_number, header_tuple, row_cells) for every markdown table row
    under a header, so a check can dispatch on what the columns claim to be."""
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    header = None
    for i, line in enumerate(lines):
        if not line.startswith("|"):
            header = None
            continue
        if i + 1 < len(lines) and SEPARATOR.match(lines[i + 1].strip()):
            header = tuple(cells(line))
            continue
        if header is None or SEPARATOR.match(line.strip()):
            continue
        yield i + 1, header, cells(line)


def load():
    def read(name, key):
        with open(os.path.join(REPO, "src", "gendata", name), "rb") as f:
            return bvnr_data.load(f.read())[key]
    units = {u["name"]: u for u in read("units.bvnr", "units")}
    curr = {c["code"]: c for c in read("currencies.bvnr", "currencies")}
    return units, curr


def check_document(path, units, curr, bad):
    def fail(ln, msg):
        bad.append("%s:%d: %s" % (path, ln, msg))

    seen_units, seen_curr = set(), set()
    for ln, header, row in tables(os.path.join(REPO, path)):
        sym_i = column(header, SYMBOL_COL)
        long_i = column(header, LONGFORM_COL)
        enum_i = column(header, ENUM_COL)
        if sym_i is not None and enum_i is not None and len(row) > max(sym_i, enum_i):
            m = re.fullmatch(r"`(bu_\w+)`", row[enum_i])
            if not m:
                continue
            name = m.group(1)[3:]
            u = units.get(name)
            if u is None:
                fail(ln, "bu_%s is not a unit in units.bvnr" % name)
                continue
            seen_units.add(name)
            spellings = TICK.findall(row[sym_i])
            if not spellings:
                fail(ln, "bu_%s: the Symbol column has no `symbol`" % name)
            elif spellings[0] != u["symbol"]:
                fail(ln, "bu_%s: documented symbol %r, units.bvnr says %r"
                         % (name, spellings[0], u["symbol"]))
            # Anything else in quotes across the two spelling columns is being
            # offered to a reader as writable, so the parser must accept it.
            extras = spellings[1:]
            if long_i is not None and len(row) > long_i:
                extras += TICK.findall(row[long_i])
            for extra in extras:
                if extra and extra not in u["aliases"]:
                    fail(ln, "bu_%s: %r is documented as a spelling but is not "
                             "an alias in units.bvnr" % (name, extra))
        elif header[:4] in (FIAT_HEAD, CRYPTO_HEAD) and len(row) >= 4:
            m = re.fullmatch(r"`([A-Z]{3,4})`", row[0])
            if not m:
                continue
            code = m.group(1)
            c = curr.get(code)
            if c is None:
                fail(ln, "%s is not a currency in currencies.bvnr" % code)
                continue
            seen_curr.add(code)
            fiat = header[:4] == FIAT_HEAD
            fields = [("minor_unit", row[2 if fiat else 1])]
            if fiat:
                fields.append(("numeric_code", row[1]))
            for field, cell in fields:
                text = cell.strip("* ")
                if not text.isdigit():
                    fail(ln, "%s: %s column %r is not a number" % (code, field, cell))
                elif int(text) != c[field]:
                    fail(ln, "%s: documented %s %s, currencies.bvnr says %d"
                             % (code, field, text, c[field]))
            # Documents append annotations ("*(historical; retired …)*"), so the
            # catalogue name has to be a prefix rather than the whole cell.
            if not row[3].startswith(c["name"]):
                fail(ln, "%s: documented name %r does not start with the "
                         "catalogue's %r" % (code, row[3], c["name"]))

    for missing in sorted(set(units) - seen_units):
        fail(0, "bu_%s appears in no unit table of this document" % missing)
    for missing in sorted(set(curr) - seen_curr):
        fail(0, "%s appears in no currency table of this document" % missing)
    return len(seen_units), len(seen_curr)


# doc/09 §18 summarises the whole BaseUnit enum as a table of id RANGES rather
# than a row per member, so the unit-coverage check above cannot see it: there
# is no `bu_*` cell to key on. It drifted exactly the way that invisibility
# predicts -- the table stopped at 100179 and the 35 members from 100180 up (the
# six temperature differences, the US survey lengths, the typographic lengths,
# the dry volumes and the rest) were simply absent, with the build green. What
# is checkable without parsing prose is CONTIGUITY: the ranges must tile block
# 10 and block 90 end to end, with no gap and no overlap.
RANGE_DOC = "doc/09_bovnar_python_bindings.md"
RANGE_ROW = re.compile(r"^(\d+)(?:[–-](\d+))?$")


def check_id_range_table(units, curr, bad):
    path = os.path.join(REPO, RANGE_DOC)
    if not os.path.exists(path):
        return 0
    spans = []
    for ln, header, row in tables(path):
        if header[:2] != ("Range", "Members") or len(row) < 2:
            continue
        m = RANGE_ROW.match(row[0].strip())
        if not m:
            bad.append("%s:%d: %r is not an id or id range"
                       % (RANGE_DOC, ln, row[0]))
            continue
        lo = int(m.group(1))
        hi = int(m.group(2)) if m.group(2) else lo
        if hi < lo:
            bad.append("%s:%d: range %s runs backwards" % (RANGE_DOC, ln, row[0]))
        spans.append((lo, hi, ln))
    if not spans:
        bad.append("%s: the BaseUnit id-range table is gone" % RANGE_DOC)
        return 0
    want = [(0, 0),
            (100000, 100000 + len(units) - 1),
            (900000, 900000 + len(curr) - 1)]
    spans.sort()
    cursor = None
    for lo, hi, ln in spans:
        if cursor is None or lo != cursor:
            # a new block must open exactly where src/gendata says it does
            if not any(lo == blo for blo, _ in want):
                bad.append("%s:%d: the range table jumps to %d, which opens no "
                           "block src/gendata defines" % (RANGE_DOC, ln, lo))
        cursor = hi + 1
    for blo, bhi in want:
        covered = set()
        for lo, hi, _ in spans:
            if lo >= blo and hi <= bhi:
                covered.update(range(lo, hi + 1))
        gap = sorted(set(range(blo, bhi + 1)) - covered)
        if gap:
            bad.append("%s: the BaseUnit range table does not cover %d–%d: %d "
                       "id(s) missing, first %d, last %d"
                       % (RANGE_DOC, blo, bhi, len(gap), gap[0], gap[-1]))
    return len(spans)


def main(argv):
    global REPO
    if len(argv) > 1:
        REPO = os.path.abspath(argv[1])
    units, curr = load()
    bad = []
    totals = []
    for path in DOCS:
        totals.append((path,) + check_document(path, units, curr, bad))
    nranges = check_id_range_table(units, curr, bad)
    if bad:
        print("check_doc_unit_tables: the documents disagree with src/gendata:",
              file=sys.stderr)
        for line in bad:
            print("    " + line, file=sys.stderr)
        print("\nsrc/gendata is the source of truth; fix the document (or the "
              "data, if the document is right).", file=sys.stderr)
        return 1
    for path, nu, nc in totals:
        print("check_doc_unit_tables: %s — %d units, %d currencies, all "
              "matching src/gendata" % (path, nu, nc))
    print("check_doc_unit_tables: %s — %d id range(s), covering every "
          "BaseUnit member" % (RANGE_DOC, nranges))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
