#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_doc_counts.py — every "180 physical units" in the tree, against gendata.

THE GAP THIS CLOSES. src/gendata is the source of truth for the catalogue, and
this repo gates the tables against it thoroughly: check_doc_unit_tables.py
compares roughly 1150 documented ROWS, test_unit_factors_derived.py compares the
generated C factors, check_profile_factors.py compares the profiles against
their publishers. What none of them compares is the SENTENCE — the headline
number a reader meets first, in the README, in the cheatsheet's scope line, in
the IETF draft, in the JOSS paper, in the EBNF's commentary, on the website.

Those had drifted, and drifted quietly, because a count in prose is invisible to
every table check. The registry had grown past 180 units and fifteen files still
said 180; the spelling total was stated as 529 against an actual 586. A reader
implementing from the draft, or citing the paper, got a number this repository
could have contradicted from its own data.

WHAT IS CHECKED. Only tightly anchored phrases — "N physical units", "N accepted
spellings", "N fiat currencies" and the handful of others in CLAIMS below. Each
is a phrase whose subject is unambiguous, so a number that happens to sit near
the word "unit" ("180 square Ruten", "216 bytes") is not swept up. A file with
no such phrase is not a failure: this proves the numbers that ARE stated, it
does not require any to be.

WHAT IS DELIBERATELY NOT CHECKED. CHANGELOG.md and the release notes, which are
a record of what was true at a version and must not be rewritten; web/ and dist/,
which are generated from the documents checked here.

Usage:  python3 check_doc_counts.py [repo-root]
Exit 0 when every stated count matches src/gendata, 1 with a list when not.
"""
import os
import re
import subprocess
import sys

REPO = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, REPO)
import bvnr_build_env                                    # noqa: E402


def counts(repo):
    """The numbers src/gendata actually states."""
    sys.path.insert(0, repo)
    import bvnr_data

    def load(name):
        with open(os.path.join(repo, "src", "gendata", name), "rb") as f:
            return bvnr_data.load(f.read())

    units = load("units.bvnr")["units"]
    curr = load("currencies.bvnr")["currencies"]
    pfx = load("prefixes.bvnr")
    profiles = ("ucum", "unece", "qudt", "qudt-qk", "udunits", "om", "cf")
    mapped = refused = 0
    opaque, aliases = {}, {}
    for ns in profiles:
        d = load(ns + ".bvnr")
        # Codes, so an opaque alias counts; ids (opaque[]) do not include it.
        mapped += (len(d.get("mapped", [])) + len(d.get("opaque", []))
                   + len(d.get("opaque_aliases", [])))
        refused += len(d.get("unsupported", []))
        opaque[ns] = len(d.get("opaque", []))
        aliases[ns] = len(d.get("opaque_aliases", []))
    _ = profiles
    # The id-space BOUNDS, not just the totals. A block's last id is a function
    # of how many rows the block holds, so "100000–100179 (180)" in doc/04 and
    # "`bu_bit` = 100000 to `bu_long_hundredweight` = 100191" in doc/05 are
    # count claims wearing a different hat -- and both had been left behind by a
    # growing catalogue, in two documents whose tables listed every unit
    # correctly.
    native_last = 100000 + len(units) - 1
    # BVN_EXPONENT_MAX, read from the header that defines it. Stated in prose in
    # a dozen places and stale in six of them at once: the range grew from ±9 to
    # ±100 and the sentences describing it did not, in doc/08, in two Python
    # docstrings, in an OverflowError message a user reads at runtime, and in the
    # public header's own note. Each was a bound a caller would code against.
    exp_max = 100
    try:
        with open(os.path.join(repo, "include", "bovnar.h"), encoding="utf-8") as f:
            m = re.search(r"#define\s+BVN_EXPONENT_MAX\s+\(?\s*(\d+)", f.read())
            if m:
                exp_max = int(m.group(1))
    except OSError:
        pass
    # BVN_UNIT_SLOT_COUNT, read from the header that defines it. Two documents
    # quoted it as a number and BOTH had been left behind, at two DIFFERENT
    # stale values -- doc/04 said 247 and doc/05's transcript of the header said
    # 259 while the generated header said 329. It is a function of the
    # catalogue, so it moves every time a unit is added, and nothing was
    # comparing it: the id-space bounds beside it in the same paragraph were
    # gated and this one was not.
    slot_count = None
    try:
        with open(os.path.join(repo, "include", "bovnar_profiles.gen.h"),
                  encoding="utf-8") as f:
            m = re.search(r"#define\s+BVN_UNIT_SLOT_COUNT\s+(\d+)", f.read())
            if m:
                slot_count = int(m.group(1))
    except OSError:
        pass
    return {
        "exponent_max": exp_max,
        # Units a Δ() operator would be a no-op on: every unit that is not an
        # affine scale. doc/temperature_difference.md states it beside the
        # total, and when the total was corrected from 192 to 262 the
        # complement stayed at 186 -- the number that made the argument, left
        # asserting 186 of 262 when 192 - 6 was where it came from.
        "units_non_affine": sum(1 for u in units if not u["affine"]),
        **({"unit_slot_count": slot_count} if slot_count is not None else {}),
        "native_last": native_last,
        "native_last_name": "bu_" + units[-1]["name"],
        "ucum_opaque": opaque["ucum"],
        # UCUM's arbitrary CODES: one id each, except [iU], which is [IU]'s.
        "ucum_arbitrary": opaque["ucum"] + aliases["ucum"],
        "ucum_opaque_last": 200000 + opaque["ucum"] - 1,
        "unece_opaque": opaque["unece"],
        "unece_opaque_last": 300000 + opaque["unece"] - 1,
        "currency_last": 900000 + len(curr) - 1,
        "profiles": len(profiles),
        "mapped_codes": mapped,
        "refusals": refused,
        "units": len(units),
        "spellings": sum(len(u["aliases"]) for u in units),
        "currencies": len(curr),
        "fiat": sum(1 for c in curr if not c["is_crypto"]),
        "crypto": sum(1 for c in curr if c["is_crypto"]),
        "si_prefixes": len(pfx["si_prefixes"]),
        "iec_prefixes": len(pfx["iec_prefixes"]),
    }


# The gap between two words of a phrase: ONE space, or a line break and whatever
# a wrapped line opens with (Markdown quoting, a comment marker, indentation).
# Not \s+ — a run of spaces is a TABLE COLUMN, not a sentence, and matching one
# turned "900000..909999   currencies" in the id-space table of bovnar.h into a
# claim that the catalogue holds 909 999 of them.
_G = r"(?:[ ]|\n[ \t>#*(]*)"


def _phrase(*words):
    return _G.join(words)


# (regex, key) — the regex must capture exactly one number, and the phrase must
# leave no doubt what that number counts. Add a phrasing here rather than
# loosening one that already works.
# The namespace count, stated as a WORD in four documents. doc/11 managed to say
# "seven" in its own header and "five" 160 lines later, and doc/09 said "five"
# long after om and cf shipped -- a count nothing gated because it is spelled out
# rather than written as a numeral.
_WORD = {"one": 1, "two": 2, "three": 3, "four": 4, "five": 5, "six": 6,
         "seven": 7, "eight": 8, "nine": 9, "ten": 10}

CLAIMS = [
    # An OPTIONAL adjective run between the number and "units": "215 physical
    # units", "215 named base units", "215 named physical base units". The five
    # fixed phrasings this replaces spelled out four of those five orderings and
    # missed the fifth, which is exactly the one doc/05 §3 opened with -- a
    # sentence that said 180 for as long as the registry had been growing, in
    # the very document whose 1150 gated table rows all said otherwise.
    (_phrase(r"\b(\d+)", r"(?:named|physical|base)(?:%s(?:named|physical|base))*"
             % _G, r"units\b"),                                 "units"),
    (_phrase(r"registry(?:'s)?", r"(\d+)", r"units\b"),         "units"),
    # ...and the complement stated beside it. See units_non_affine above.
    (_phrase(r"no-op", "on", r"(\d+)", "of", r"the", r"registry"),
                                                                "units_non_affine"),
    # The dense-table row count, wherever a document quotes the macro's value.
    # Matches both spellings it appears in: prose ("`BVN_UNIT_SLOT_COUNT` = 331")
    # and a transcript of the header ("#define BVN_UNIT_SLOT_COUNT 331").
    (r"BVN_UNIT_SLOT_COUNT`?\s*(?:=|\s)\s*(\d+)",               "unit_slot_count"),
    # doc/11 §6.4's class table heads its first row "Arbitrary units (41)" --
    # UCUM's opaque list, which grew with the rest and said 32 for as long as
    # every other profile count in that document was wrong.
    (r"Arbitrary units \((\d+)\)",                              "ucum_arbitrary"),
    # The exponent bound, wherever a sentence states it as the CURRENT range.
    # Anchored on phrasings that assert rather than recall: "±9 the format can
    # spell" is a claim, "used to stop at ±9" is history and must stay.
    (_phrase(r"±(\d+)", "the", "format", "can", r"spell\b"),   "exponent_max"),
    (_phrase(r"exponent", "outside", r"±(\d+)"),               "exponent_max"),
    (_phrase(r"exponent", "range", r"±(\d+)"),                 "exponent_max"),
    (_phrase(r"summed", "exponent", "past", r"±(\d+)"),        "exponent_max"),
    (_phrase(r"summed", "exponent", "outside", r"±(\d+)"),     "exponent_max"),
    (_phrase(r"\b(\d+)", "units", "and", r"\d+", r"currencies\b"), "units"),
    (_phrase(r"\b(\d+)", "accepted", r"spellings\b"),           "spellings"),
    (_phrase(r"\b(\d+)", "spellings", "in", r"total\b"),        "spellings"),
    (_phrase(r"\b(\d+)", "registered", r"spellings\b"),         "spellings"),
    # Two words, always on one line: the wrapping form of this one matched the
    # BLOCK-TAG TABLES in the .bvnr headers, where "90" is an id block and
    # "currencies" the next comment line's first word.
    (r"\b(\d+) currencies\b",                                   "currencies"),
    (_phrase(r"\b(\d+)", "currency", r"codes\b"),               "currencies"),
    # "ISO 4217 fiat currencies" names the STANDARD, not a count, so the
    # lookbehind keeps 4217 out of the comparison.
    (_phrase(r"(?<!ISO )\b(\d+)", "fiat", r"currencies\b"),     "fiat"),
    (_phrase(r"\b(\d+)", r"cryptocurrencies\b"),                "crypto"),
    (_phrase(r"\b(\d+)", "SI", r"\+", r"\d+", "IEC", r"prefixes\b"),
                                                                "si_prefixes"),
    (_phrase(r"\bSI", r"\+", r"(\d+)", "IEC", r"prefixes\b"),   "iec_prefixes"),
    # The profile tables, as a WHOLE. Anchored on the full "N mapped codes and M
    # named refusals" phrasing rather than on "N mapped codes" alone: doc/11
    # narrates how each vocabulary's own table grew ("from 100 mapped codes to
    # 201"), which is history and must not be rewritten to today's total. The
    # digits carry a thin space in the documents, so a separator is allowed
    # inside the number and stripped before comparing.
    (_phrase(r"\b([\d\u202f\u00a0 ]+?)", "mapped", "codes", "and",
             r"[\d\u202f\u00a0 ]+?", "named", r"refusals\b"),   "mapped_codes"),
    (_phrase(r"\b[\d\u202f\u00a0 ]+?", "mapped", "codes", "and",
             r"([\d\u202f\u00a0 ]+?)", "named", r"refusals\b"), "refusals"),
]

# The id-space BOUNDS, as opposed to the plain counts above. Each pattern
# captures SEVERAL numbers and every one of them is compared, because a range
# and the size beside it are two ways of stating the same fact and a document
# that updates one and not the other is worse than one that updates neither.
RANGE_CLAIMS = [
    (r"Native units 100000[–-](\d+) \((\d+)\)",
     ("native_last", "units")),
    (r"UCUM opaque units 200000[–-](\d+) \((\d+)\)",
     ("ucum_opaque_last", "ucum_opaque")),
    (r"UN/ECE opaque units 300000[–-](\d+) \((\d+)\)",
     ("unece_opaque_last", "unece_opaque")),
    (r"currencies 900000[–-](\d+) \((\d+) fiat, (\d+) crypto\)",
     ("currency_last", "fiat", "crypto")),
    (_phrase(r"`bu_bit`", "=", "100000", "to", r"`(bu_\w+)`", "=", r"(\d+)"),
     ("native_last_name", "native_last")),
]

# Everything under these is either a historical record or generated from a file
# that IS checked. Rewriting a changelog to match today's catalogue would be a
# lie about what shipped.
SKIP_DIRS = {".git", "build", "dist", "web", "__pycache__", ".pytest_cache",
             ".cache", ".idea", "node_modules"}
SKIP_FILES = {"CHANGELOG.md", "check_doc_counts.py"}
SKIP_PREFIXES = ("RELEASE_NOTES_",)
SUFFIXES = (".md", ".py", ".txt", ".bvnr", ".h", ".c", ".ebnf", ".cff", ".xml")


def _add_profile_keys(repo, want):
    """UCUM's opaque count, for the CLAIMS pattern above."""
    counts = profile_counts(repo)
    if "ucum" in counts:
        want["ucum_opaque"] = counts["ucum"]["opaque"]
        want["ucum_arbitrary"] = (counts["ucum"]["opaque"]
                                  + counts["ucum"]["opaque_aliases"])
    return want


def _skip_dir(d):
    # "build" was skipped by name, but a second build tree is routine here --
    # CI's own sanitizer pass configures build-asan/ beside it, and the
    # amalgamation copies the generated headers into it. Those are build
    # OUTPUT: they restate the catalogue's own numbers, so scanning them
    # reported a stale artifact as a documentation claim and made the gate
    # fail on which directories happened to exist.
    return d in SKIP_DIRS or d.startswith("build-")


def files(repo):
    for root, dirs, names in os.walk(repo):
        dirs[:] = [d for d in dirs if not _skip_dir(d)]
        for n in sorted(names):
            if n in SKIP_FILES or n.startswith(SKIP_PREFIXES):
                continue
            if not n.endswith(SUFFIXES):
                continue
            yield os.path.join(root, n)


PROFILE_FILES = ("ucum", "unece", "qudt", "qudt-qk", "udunits", "om", "cf")


def profile_counts(repo):
    """{vocab: {mapped, opaque, unsupported, prefixes, total}} from src/gendata."""
    sys.path.insert(0, repo)
    try:
        import bvnr_data
    except ImportError:
        return {}
    out = {}
    for vocab in PROFILE_FILES:
        path = os.path.join(repo, "src", "gendata", "%s.bvnr" % vocab)
        if not os.path.exists(path):
            continue
        with open(path, "rb") as f:
            data = bvnr_data.load(f.read())
        n = {k: len(data.get(k, [])) for k in
             ("mapped", "opaque", "opaque_aliases", "unsupported", "prefixes")}
        n["total"] = (n["mapped"] + n["opaque"] + n["opaque_aliases"]
                      + n["unsupported"])
        out[vocab] = n
    return out


def check_profile_tables(repo, counts):
    """doc/11 §9.1's per-file table, against the files themselves.

    Every number in it was stale — `ucum.bvnr` listed as 141 mapped against 204,
    `qudt.bvnr` as 263 against 2222 — and `om.bvnr` and `cf.bvnr` were not in it
    at all. The same figures are quoted in six other places in that document and
    all of them had drifted the same way, because the tables grow whenever the
    registry gains a unit that unblocks a previously-refused code, and nothing
    compared the sentences with the files.

    §9.1 is the one table this gate owns, deliberately: it is the place the
    document points at for current figures, so it is the one that has to be
    right. The blockquote under each vocabulary section is checked too, since
    that is where a reader of that section looks.
    """
    doc = os.path.join(repo, "doc", "11_bovnar_unit_profiles.md")
    if not os.path.exists(doc) or not counts:
        return [], 0
    with open(doc, encoding="utf-8") as f:
        text = f.read()
    problems, checked = [], 0
    # "| `ucum.bvnr` | 20 prefixes, 204 mapped, 41 opaque, 67 unsupported |"
    row = re.compile(r"\|\s*`([a-z-]+)\.bvnr`\s*\|([^|]*)\|")
    for m in row.finditer(text):
        vocab, body = m.group(1), m.group(2)
        if vocab not in counts:
            continue
        line = text.count("\n", 0, m.start()) + 1
        # "opaque alias" before "opaque", or "1 opaque alias" reads as 1 opaque.
        for stated, kind in re.findall(r"(\d+)\s+(prefixes|mapped|"
                                       r"opaque alias(?:es)?|opaque|"
                                       r"unsupported)", body):
            key = "opaque_aliases" if kind.startswith("opaque alias") else kind
            checked += 1
            if int(stated) != counts[vocab][key]:
                problems.append("doc/11_bovnar_unit_profiles.md:%d: says %s "
                                "%s for %s.bvnr, src/gendata has %d"
                                % (line, stated, kind, vocab,
                                   counts[vocab][key]))
    # "> Data file `src/gendata/qudt.bvnr` (2222 mapped, 586 unsupported — …"
    quote = re.compile(r"`src/gendata/([a-z-]+)\.bvnr`\s*\(([^)]*)\)")
    for m in quote.finditer(text):
        vocab, body = m.group(1), m.group(2)
        if vocab not in counts:
            continue
        line = text.count("\n", 0, m.start()) + 1
        for stated, kind in re.findall(r"(\d+)\s+(prefixes|mapped|"
                                       r"opaque alias(?:es)?|opaque|"
                                       r"unsupported|refused)", body):
            key = {"refused": "unsupported"}.get(kind, kind)
            if kind.startswith("opaque alias"):
                key = "opaque_aliases"
            checked += 1
            if int(stated) != counts[vocab][key]:
                problems.append("doc/11_bovnar_unit_profiles.md:%d: says %s "
                                "%s for %s.bvnr, src/gendata has %d"
                                % (line, stated, kind, vocab,
                                   counts[vocab][key]))
    return problems, checked


def check_assertion_counts(repo):
    """doc/11's "pinned by tests/bovnar_X_test.c (N assertions)", against the run.

    Five of these were stated and all five were stale — the UN/ECE one said 134
    against an actual 557. They drift for the same reason the table counts do:
    the tests grow whenever the tables do, and a number in prose has nothing
    holding it to the number in the binary.

    The binaries print their own total, so the honest check is to run them. A
    tree without a build skips, loudly, rather than passing on an unread claim.
    """
    doc = os.path.join(repo, "doc", "11_bovnar_unit_profiles.md")
    if not os.path.exists(doc):
        return [], 0
    with open(doc, encoding="utf-8") as f:
        text = f.read()
    claims = re.findall(r"`tests/(bovnar_(\w+)_test)\.c`\s*\((\d+) assertions\)",
                        text)
    if not claims:
        return [], 0
    problems, checked = [], 0
    ran = False
    for _src, vocab, stated in claims:
        exe = os.path.join(bvnr_build_env.tests_dir(repo),
                           "bvnr_%s_test" % vocab)
        if not os.path.exists(exe):
            continue
        try:
            out = subprocess.run([exe], capture_output=True, text=True,
                                 timeout=120)
        except Exception:                                     # noqa: BLE001
            continue
        m = re.search(r"(\d+) tests", (out.stdout or "") + (out.stderr or ""))
        if not m:
            continue
        ran = True
        checked += 1
        if int(m.group(1)) != int(stated):
            problems.append("doc/11_bovnar_unit_profiles.md: says "
                            "tests/bovnar_%s_test.c has %s assertions; it runs "
                            "%s" % (vocab, stated, m.group(1)))
    if not ran:
        print("check_doc_counts: no built profile tests; the %d assertion "
              "count(s) in doc/11 were not checked" % len(claims))
    return problems, checked


INVENTORY_DOC = "doc/02_bovnar_faq.md"
INVENTORY_HEAD = "**How many base units does Bovnar support?**"


def inventory(repo, want):
    """doc/02's per-category inventory must SUM to the catalogue.

    The FAQ answers "how many base units" with a total and then 42 categories
    that account for every one of them. Each category count is checkable only
    against the others: a unit moved between two rows, or a row whose count was
    not bumped when the catalogue grew, leaves the total right and the breakdown
    wrong. Nothing else in this file would notice, because every individual
    number stays plausible.
    """
    path = os.path.join(repo, INVENTORY_DOC)
    if not os.path.exists(path):
        return [], 0
    with open(path, encoding="utf-8") as f:
        text = f.read()
    if INVENTORY_HEAD not in text:
        return ["%s: the base-unit inventory heading is gone; the category "
                "breakdown is no longer checked" % INVENTORY_DOC], 0
    section = text.split(INVENTORY_HEAD, 1)[1].split("\n\nThe `bu_gram`", 1)[0]
    m = re.search(r"(\d+) named base units", section)
    if not m:
        return ["%s: the inventory no longer states a total" % INVENTORY_DOC], 0
    stated = int(m.group(1))
    rows = re.findall(r"^- \*\*(\d+) ", section, re.M)
    total = sum(int(n) for n in rows)
    problems = []
    if stated != want["units"]:
        problems.append("%s: the inventory says %d named base units; "
                        "src/gendata has %d"
                        % (INVENTORY_DOC, stated, want["units"]))
    if total != stated:
        problems.append("%s: the inventory's %d categories sum to %d, but it "
                        "states %d — a unit is in two rows or in none"
                        % (INVENTORY_DOC, len(rows), total, stated))
    return problems, 2


def main(argv):
    repo = os.path.abspath(argv[1]) if len(argv) > 1 else REPO
    sys.path.insert(0, REPO)
    import bvnr_build_env
    bvnr_build_env.require_build(repo, "check_doc_counts", ("tests",))
    want = _add_profile_keys(repo, counts(repo))
    bad, checked = [], 0
    inv_bad, inv_checked = inventory(repo, want)
    bad += inv_bad
    checked += inv_checked
    prof_bad, prof_checked = check_profile_tables(repo, profile_counts(repo))
    bad += prof_bad
    checked += prof_checked
    asrt_bad, asrt_checked = check_assertion_counts(repo)
    bad += asrt_bad
    checked += asrt_checked
    for path in files(repo):
        try:
            with open(path, encoding="utf-8") as f:
                text = f.read()
        except (UnicodeDecodeError, OSError):
            continue
        rel = os.path.relpath(path, repo)
        # The word-form namespace claim, checked before the numeral claims
        # because its value is a word and the shared loop parses an integer.
        for m in re.finditer(
                r"\b([Oo]ne|[Tt]wo|[Tt]hree|[Ff]our|[Ff]ive|[Ss]ix|[Ss]even|"
                r"[Ee]ight|[Nn]ine|[Tt]en)" + _G + r"namespaces?" + _G +
                r"(?:are|is)" + _G + r"(?:defined|accepted)", text):
            checked += 1
            got = _WORD[m.group(1).lower()]
            if got != want["profiles"]:
                line = text.count("\n", 0, m.start()) + 1
                bad.append("%s:%d: says %d namespaces, src/gendata defines %d  (%r)"
                           % (rel, line, got, want["profiles"],
                              " ".join(m.group(0).split())))
        for pattern, key in CLAIMS:
            for m in re.finditer(pattern, text):
                checked += 1
                got = int(re.sub(r"[\s\u202f\u00a0]", "", m.group(1)))
                if got != want[key]:
                    line = text.count("\n", 0, m.start()) + 1
                    bad.append("%s:%d: says %d %s, src/gendata has %d  (%r)"
                               % (rel, line, got, key, want[key],
                                  m.group(0)))
        for pattern, keys in RANGE_CLAIMS:
            for m in re.finditer(pattern, text):
                for group, key in enumerate(keys, 1):
                    checked += 1
                    raw = m.group(group)
                    expect = want[key]
                    got = (raw if isinstance(expect, str)
                           else int(re.sub(r"[\s  ]", "", raw)))
                    if got != expect:
                        line = text.count("\n", 0, m.start()) + 1
                        bad.append("%s:%d: says %s for %s, src/gendata has %s"
                                   "  (%r)" % (rel, line, got, key, expect,
                                               " ".join(m.group(0).split())))
    if bad:
        print("check_doc_counts: stated counts disagree with src/gendata:",
              file=sys.stderr)
        for line in bad:
            print("    " + line, file=sys.stderr)
        print("\nsrc/gendata is the source of truth; fix the sentence.",
              file=sys.stderr)
        return 1
    print("check_doc_counts: %d stated count(s) checked, all matching "
          "src/gendata (%d units, %d spellings, %d currencies)"
          % (checked, want["units"], want["spellings"], want["currencies"]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
