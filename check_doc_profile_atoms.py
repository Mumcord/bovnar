#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_doc_profile_atoms.py — doc/11 §6.1's UCUM atom tables, against
src/gendata/ucum.bvnr.

THE GAP THIS CLOSES. §6.1 opens with "What follows is the whole mapped list",
and it was not: 188 codes are mapped in `ucum.bvnr` and 155 appeared in the
tables, so a third of the profile was invisible to a reader consulting the one
place that promises completeness. Worse than an omission, the same section
carried a sentence saying the US survey series "is **refused**" while §6.3, a
hundred lines down, correctly listed all nine of them as mapped — the document
contradicting itself about a capability the library has had for some time.

Nothing caught it because the existing gates ask different questions.
check_profile_factors.py's `check_doc_error_claims` keys on an `error_unit_*`
token, and a sentence that says "is refused" in plain English carries none;
`check_doc_profile_spellings` reads what the WRITER emits. Neither asks the
completeness question, which is the one §6.1's own opening sentence makes.

WHAT IS CHECKED, per row of §6.1:

  membership  the code is one `ucum.bvnr` maps (an opaque/arbitrary unit is
              allowed too — §6.1 lists a few beside the mapped ones);
  target      the Bovnar column is what the reference implementation actually
              produces for `ucum:<code>` — not merely a unit that parses;
  factor      the third column is that unit's coherent-SI factor;
  coverage    every mapped code appears in some §6.1 table.

The whole-code table at the end of §6.1 is read the same way, except that its
first column holds a UCUM EXPRESSION rather than an atom, so it is checked by
parsing rather than by table membership.

Needs the built library (it asks the reader, rather than restating its rules),
so it skips rather than fails when there is none — the same rule
check_profile_factors.py follows.

Usage:  python3 check_doc_profile_atoms.py [repo-root]
Exit 0 when §6.1 agrees with src/gendata and the library, 1 with a list when not.
"""
import os
import re
import sys

REPO = os.path.dirname(os.path.abspath(__file__))
DOC = "doc/11_bovnar_unit_profiles.md"
SECTION_START = "### 6.1"
SECTION_END = "### 6.2"

TICK = re.compile(r"`([^`\n]+)`")
SEPARATOR = re.compile(r"^\|[\s:|-]+\|$")


def load_reader(repo):
    """(parse_ucum, unit_to_str, si_factor), or None when there is no library."""
    sys.path.insert(0, os.path.join(repo, "python"))
    try:
        import bovnar
        bovnar._ffi.load_library()
    except Exception:
        return None

    def parse(expr):
        try:
            return bovnar.parse_unit(expr)
        except Exception:
            return None

    def to_str(u):
        return bovnar.unit_to_str(u)

    def factor(u):
        # An opaque (arbitrary) unit has no SI factor at all and the binding
        # raises rather than reporting one, which is the right answer for a
        # row whose factor cell says so in words.
        try:
            return bovnar.unit_to_si_factor(u).factor
        except Exception:
            return None

    return parse, to_str, factor


def rows(path):
    """(line number, cells) for every table row in §6.1."""
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    inside = False
    header = None
    for n, line in enumerate(lines, 1):
        if line.startswith(SECTION_START):
            inside = True
            continue
        if line.startswith(SECTION_END):
            return
        if not inside:
            continue
        if not line.startswith("|"):
            header = None
            continue
        if SEPARATOR.match(line.strip()):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if header is None:
            header = cells
            continue
        if header[:1] != ["UCUM"] or len(cells) < 3:
            continue
        yield n, cells


COLLISION_START = "### 6.2"
COLLISION_END = "### 6.3"


def collision_rows(path):
    """(line number, spelling, native cell, ucum cell) for §6.2's table."""
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    inside, header = False, None
    for n, line in enumerate(lines, 1):
        if line.startswith(COLLISION_START):
            inside = True
            continue
        if line.startswith(COLLISION_END):
            return
        if not inside or not line.startswith("|"):
            continue
        if SEPARATOR.match(line.strip()):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if header is None:
            header = cells
            continue
        if len(cells) >= 3:
            spell = TICK.findall(cells[0])
            if spell:
                yield n, spell[0], cells[1], cells[2]


def check_collisions(path, parse):
    """doc/11 §6.2 — each spelling parses (or does not) on each side, as stated.

    The table's whole point is which of the two namespaces resolves the same
    bytes, so a row that says one side refuses is the row worth checking. It
    said `ucum:Gb` was refused because UCUM's `b` (the barn) is non-metric and
    `G`+`b` is not a legal prefixed code — all true, and beside the point: `Gb`
    is an atom in its own right, the gilbert. The code resolves, to a
    magnetomotive force, against a native gigabit. That is the dangerous shape
    the table exists to enumerate, and it was listed as the harmless one.
    """
    problems, checked = [], 0
    for lineno, spelling, native_cell, ucum_cell in collision_rows(path):
        for cell, expr, side in ((native_cell, spelling, "natively"),
                                 (ucum_cell, "ucum:" + spelling, "as UCUM")):
            claims_absent = ("not a unit" in cell or "*refused*" in cell
                             or "refused" in cell.lower())
            resolves = parse(expr) is not None
            checked += 1
            if claims_absent and resolves:
                problems.append(
                    "%s:%d: §6.2 says `%s` does not resolve %s; it does"
                    % (DOC, lineno, spelling, side))
            elif not claims_absent and not resolves:
                problems.append(
                    "%s:%d: §6.2 describes `%s` %s; it does not resolve"
                    % (DOC, lineno, spelling, side))
    return problems, checked


# The per-vocabulary tables outside §6.1: "| <code> | <bovnar> | ..." for UNECE
# and, two pairs to a row, for the QUDT quantity kinds.
VOCAB_TABLES = (
    ("### 11.4", "## 12", "unece",   ("Rec 20", "Bovnar")),
    ("### 12.4", "## 13", "qudt-qk", ("QUDT quantity kind", "Bovnar")),
)


def vocab_table_rows(path, start, end, want_header):
    """(line, code, target) for a table whose header begins with want_header.

    Anchored on the HEADER, not just the section, so a future table of another
    shape in the same section is skipped rather than misread as code -> target.
    A row may carry several code/target pairs side by side, as §12.4's does --
    checking only the first pair silently halved that table's coverage.
    """
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    inside, header = False, None
    for n, line in enumerate(lines, 1):
        if line.startswith(start):
            inside = True
            continue
        if inside and line.startswith(end):
            return
        if not inside or not line.startswith("|"):
            continue
        if SEPARATOR.match(line.strip()):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if header is None:
            header = cells
            if tuple(cells[:2]) != want_header:
                header, inside = None, inside   # not our table; keep looking
                header = None
            continue
        for i in range(0, len(cells) - 1, 2):
            code = TICK.findall(cells[i])
            target = TICK.findall(cells[i + 1])
            if len(code) == 1 and len(target) == 1:
                yield n, code[0], target[0]


def check_vocab_tables(path, parse, to_str):
    """doc/11 §11.4 and §12.4, the tables no gate had ever read.

    §6.1 was checked against src/gendata and §13.3 against the parser, while the
    UNECE reading table and the QUDT quantity-kind table -- 49 and 52 rows of
    code-to-unit claims -- were checked by nobody. They were correct when this
    was written; that is a fact about today, not a property of the file.
    """
    problems, checked = [], 0
    for start, end, vocab, head in VOCAB_TABLES:
        for lineno, code, target in vocab_table_rows(path, start, end, head):
            expr = "%s:%s" % (vocab, code)
            got = parse(expr)
            checked += 1
            if got is None:
                problems.append("%s:%d: `%s` is tabulated as `%s`; it is refused"
                                % (DOC, lineno, expr, target))
            elif to_str(got) != target:
                problems.append("%s:%d: `%s` is tabulated as `%s`; it is `%s`"
                                % (DOC, lineno, expr, target, to_str(got)))
    return problems, checked


NEARMISS_START = "### 13.3"
NEARMISS_END = "### 13.4"


def nearmiss_rows(path):
    """(line, [codes], last cell) for §13.3's near-miss table."""
    with open(path, encoding="utf-8") as f:
        lines = f.read().split("\n")
    inside, header = False, None
    for n, line in enumerate(lines, 1):
        if line.startswith(NEARMISS_START):
            inside = True
            continue
        if line.startswith(NEARMISS_END):
            return
        if not inside or not line.startswith("|"):
            continue
        if SEPARATOR.match(line.strip()):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if header is None:
            header = cells
            continue
        if len(cells) >= 5:
            codes = TICK.findall(cells[0])
            if codes:
                yield n, codes, cells[-1]


def check_nearmisses(path, parse, to_str):
    """doc/11 §13.3 — the UDUNITS codes that borrow a native unit's name.

    This table said every row was refused, and eleven of its twelve rows had
    started mapping: `udunits:year` to `yr_trop`, `calorie` to `cal_IT`, the
    chain/rod/furlong/fathom/acre series to the US survey units, `shake` to
    `shake`. The registry gaining those units is exactly what unblocked them —
    §6.3 recorded the same change for UCUM's survey series and this section did
    not. A reader was told a code is refused that resolves.

    The last column names what each code maps to now, or says it is refused.
    """
    problems, checked = [], 0
    for lineno, codes, verdict in nearmiss_rows(path):
        refused = "refus" in verdict.lower()
        targets = TICK.findall(verdict)
        for i, code in enumerate(codes):
            got = parse("udunits:" + code)
            checked += 1
            if refused:
                if got is not None:
                    problems.append(
                        "%s:%d: §13.3 says `udunits:%s` is refused; it maps to %r"
                        % (DOC, lineno, code, to_str(got)))
                continue
            if got is None:
                problems.append("%s:%d: §13.3 says `udunits:%s` maps; it is "
                                "refused" % (DOC, lineno, code))
                continue
            if not targets:
                continue
            want = targets[i] if len(targets) == len(codes) else targets[0]
            if to_str(got) != want:
                problems.append(
                    "%s:%d: §13.3 says `udunits:%s` maps to `%s`; it maps to `%s`"
                    % (DOC, lineno, code, want, to_str(got)))
    return problems, checked


def main(argv):
    repo = os.path.abspath(argv[1]) if len(argv) > 1 else REPO
    sys.path.insert(0, REPO)
    import bvnr_build_env
    bvnr_build_env.require_build(repo, "check_doc_profile_atoms", ("library",))
    sys.path.insert(0, repo)
    import bvnr_data

    path = os.path.join(repo, DOC)
    if not os.path.exists(path):
        print("check_doc_profile_atoms: %s is absent; nothing to check" % DOC)
        return 0

    reader = load_reader(repo)
    if reader is None:
        print("check_doc_profile_atoms: no built library; skipped")
        return 0
    parse, to_str, factor = reader

    with open(os.path.join(repo, "src", "gendata", "ucum.bvnr"), "rb") as f:
        data = bvnr_data.load(f.read())
    mapped = {m["code"] for m in data.get("mapped", [])}
    opaque = {o["code"] for o in data.get("opaque", [])}

    bad, checked = check_collisions(path, parse)
    nm_bad, nm_checked = check_nearmisses(path, parse, to_str)
    bad += nm_bad
    checked += nm_checked
    vt_bad, vt_checked = check_vocab_tables(path, parse, to_str)
    bad += vt_bad
    checked += vt_checked
    seen = set()
    for lineno, cells in rows(path):
        codes = TICK.findall(cells[0])
        if not codes:
            continue
        code = codes[0]
        want_target = TICK.findall(cells[1])
        want_factor = TICK.findall(cells[2])
        where = "%s:%d  %-14s" % (DOC, lineno, code)

        # An ATOM row names a code the table declares; a WHOLE-CODE row names an
        # expression built from several. Both must read through the profile, so
        # both are parsed -- the distinction only decides whether membership in
        # `mapped` is required.
        is_atom = code in mapped or code in opaque
        if is_atom:
            seen.add(code)
        u = parse("ucum:" + code)
        if u is None:
            bad.append("%s does not parse through the ucum profile" % where)
            continue
        checked += 1
        if want_target:
            got = to_str(u)
            if got != want_target[0]:
                bad.append("%s documented target %r, the library writes %r"
                           % (where, want_target[0], got))
        if want_factor:
            got_f = factor(u)
            try:
                doc_f = float(want_factor[0])
            except ValueError:
                continue          # "no SI factor", an opaque unit's row
            if got_f is None:
                bad.append("%s documents a factor %s; the unit has none"
                           % (where, want_factor[0]))
            elif got_f == 0.0 or abs(got_f - doc_f) > 1e-12 * abs(got_f):
                bad.append("%s documented factor %s, the library computes %.17g"
                           % (where, want_factor[0], got_f))

    missing = sorted(mapped - seen)
    if missing:
        bad.append("%s §6.1 says it is \"the whole mapped list\" and omits %d "
                   "of the %d mapped codes: %s"
                   % (DOC, len(missing), len(mapped), ", ".join(missing)))

    if bad:
        sys.stderr.write("check_doc_profile_atoms: doc/11 disagrees with "
                         "src/gendata/ucum.bvnr:\n")
        for line in bad:
            sys.stderr.write("    %s\n" % line)
        sys.stderr.write("\nsrc/gendata is the source of truth; fix the "
                         "document.\n")
        return 1
    print("check_doc_profile_atoms: %s §6.1, §6.2, §11.4, §12.4 and §13.3 — "
          "%d row(s) checked "
          "against the reader, all %d mapped UCUM codes present"
          % (DOC, checked, len(mapped)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
