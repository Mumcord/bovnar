#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_doc_refs.py — every "§N.M" a comment cites points at a section that exists.

The code cites the documentation constantly: a test explains what it pins with
"doc/08 §1.11 promises …", a header points at "the spec, §\"Version directive\"".
Those citations are the only thing tying a subtle test back to the sentence that
made it a requirement, and nothing checked them — so they rot in two directions.

Renumbering rots them wholesale. doc/08 numbered its function sections 1-25
straight through, was renumbered to N.M, and every "§7c" in the test suite
quietly began pointing at nothing; the documents' own links were all verified,
which is exactly why it went unnoticed. And they rot one at a time: a comment
cited "spec §501", which was never a section number, and another quoted a doc/05
heading — "An affine unit is valid at exponent 1 only" — that the document does
not contain, in either case describing a rule that is really there under a
number the reader cannot follow.

What a citation may look like, and how the target document is resolved:

    spec §7.4                 -> doc/03          (also "specification")
    doc/08 §1.11               -> doc/08          (a "doc/N" anywhere before it)
    read/write API §1.10      -> doc/08          (a name cue -- see CUES)
    §11 of [Unit Ambiguities] -> 07_bovnar_unit_ambiguities.md  (a cue right after it)
    §13.2                     -> the spec, from source; from inside doc/X, that
                                 same document -- which is what every bare "§"
                                 in the tree means today, and now the rule.
    §"Version directive"      -> a quoted name, matched case-insensitively
                                 against the document's text

A citation to an external standard (IEEE 754 §3.5.2, ISO 8601 §4.2) or to
somebody's licence (UCUM §3(a), CC BY 4.0 §2(a)(1)) is left alone: the skip list
is what tells those apart from a citation into doc/.

THE REPOSITORY ROOT WAS NOT SCANNED, and for a long time nobody noticed, because
the root held little that cited a section. It does now -- README, CONTRIBUTING,
THIRD_PARTY_NOTICES and the two CMakeLists between them carry citations into
doc/, and a deliberately broken §99.4 in any of them passed this gate. The root
is scanned now, non-recursively (its subdirectories are already covered by
SEARCH_DIRS or are build output), with two files skipped whole and a marker for
the individual lines that must not be resolved:

  * CHANGELOG.md cites section numbers AS THEY WERE WHEN THE ENTRY WAS WRITTEN.
    doc/08's renumbering is itself a changelog entry, and "carrying the citations
    over" there would mean rewriting history to match today's numbering --
    check_doc_counts.py skips it for exactly this reason and says so.
  * This file documents bad citations as examples -- §7c and §501 above are the
    two real ones it was built to catch -- so scanning itself reports its own
    documentation as defects.
  * `bovnar:no-section-check` on a line exempts the citations on that line, for
    the two cases a file-level skip is too blunt for: a comment that NARRATES a
    dead citation ("spec §501", never a section number), and a citation into a
    document this checker does not index, such as the IETF draft under doc/ietf.
    It is the same "a marker that looks like a comment is really configuration"
    convention as `bovnar:retired-path`, and it is deliberately per-line: a whole
    file waived is a file that stops being checked.

The same rot reaches a table that quotes source. The spec's §16.10 reprints the
error_code_t enum, and nothing compared it to include/bovnar.h — so it stopped
at 48 while the header had run to 51, and §11.9 named two of the three missing
codes in its own outcome table, leaving the document contradicting itself three
sections apart. The gap had a sharper edge than a stale list: §17 promises new
codes are appended above the current maximum, so a reader implementing from the
spec would have assigned 49 on top of a code that already existed. The enum is
checked here for the same reason the citations are — it is a claim about the
tree that only the tree can settle.

Usage:
    python3 check_doc_refs.py            # report and exit 1 on a dead citation
    python3 check_doc_refs.py --verbose  # also list what resolved, and where
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
DOC_DIR = os.path.join(ROOT, "doc")

SEARCH_DIRS = ["include", "src", "python", "tests", "wasm", "examples",
               "highlighter", "cmake", "doc"]

# Files in the repository ROOT that are scanned in addition to the directories
# above -- everything at the top level with a checked suffix, minus these two.
# See the header for why each is whole-file rather than marker-exempt.
ROOT_SKIP_FILES = {
    "CHANGELOG.md",         # cites section numbers as they were, by design
    "check_doc_refs.py",    # documents bad citations as its own examples
}

# A line carrying this marker has its citations left alone. Per line, never per
# file; the header says what the two legitimate uses are.
NO_CHECK = "bovnar:no-section-check"
SUFFIXES = (".c", ".h", ".py", ".sh", ".cmake", ".mjs", ".js", ".md", ".txt")
SKIP_DIRS = {"build", "__pycache__", ".git", ".pytest_cache", "node_modules"}

# Marker on line 1 of a retired path kept as a pointer; see
# gen_html_docs.RETIRED_MARKER for why these files exist. Spelled out rather
# than imported to keep this checker free of third-party dependencies.
RETIRED_MARKER = "bovnar:retired-path"

SPEC = "03_bovnar_spec.md"

# Name cues. Only phrases that can mean nothing but a document: "streaming",
# "conformance" and "FAQ" read as ordinary English in this tree and hijacked
# every bare "§" near them. Matched on word boundaries -- "spec" as a substring
# also lives inside "inspection", which is how a citation two words away from
# "partial inspection" resolved to the specification.
CUES = [
    (r"read\s*[/&]\s*write\s+api", "08_bovnar_readwrite_api.md"),
    (r"unit[- ]system\s+reference", "05_bovnar_unit_system.md"),
    (r"unit\s*&\s*currency\s+reference", "05_bovnar_unit_system.md"),
    (r"unit[_ ]ambiguities", "07_bovnar_unit_ambiguities.md"),
    (r"cheat\s*sheet", "04_bovnar_unit_cheatsheet.md"),
    (r"\bspecifications?\b", SPEC),
    (r"\bspec\b", SPEC),
]
# A markdown link into the doc set, "…](08_bovnar_readwrite_api.md#…)". Any
# .md name matches and every caller then gates on BY_NUMBER membership, so a
# link out of the doc set (README.md, CONTRIBUTING.md) still resolves nothing.
# This used to enumerate the numbered documents plus "datetime_*", which meant
# the design notes were reachable only if their filename happened to start with
# "datetime": a "[v2 §7](bovnar_v2_proposals.md)" named its target and was read
# as a citation into the CITING document, passing whenever the citing document
# happened to have a §7 of its own and failing as a phantom when it did not.
LINK = re.compile(r"\]\((?:doc/)?([\w.-]+\.md)")

# A "§" belonging to somebody else's document. Nothing in doc/ is numbered like
# these, so without the skip they would all read as dead citations into the spec.
EXTERNAL = re.compile(
    r"\b(IEEE|ISO|IEC|RFC|BIPM|SI Brochure|Unicode|POSIX|CommonMark|W3C|ECMA|"
    r"UTS|UAX|ITU|ANSI|IANA|"
    # Licences cite their own sections, and THIRD_PARTY_NOTICES.md quotes them
    # constantly. Without these, "UCUM §3" resolves against the specification --
    # which HAS a §3, so it would not even fail; it would agree, silently, about
    # the wrong document. That file writes external sections out as "section N"
    # for the same reason, but a reader who uses § should not be punished for it.
    r"UCUM|CC BY|Creative Commons|OFL|SIL Open Font|UrhG|UDUNITS|QUDT"
    r")\b", re.I)

DOCN = re.compile(r"doc/(\d+)(?:_[\w.]+)?")
# "§5.3", "§7c", "§A.1", or §"a quoted section name"
REF = re.compile(r"§\s?(?:\"([^\"]{2,60})\"|([0-9]+(?:\.[0-9]+)*[a-z]?|[A-Z]\.[0-9]+))")

BY_NUMBER = {}          # doc filename -> {"5.3", "A.1", …}
TEXT = {}               # doc filename -> lowercased full text
FENCE = re.compile(r"^(```|~~~)")


def load_docs():
    for name in sorted(os.listdir(DOC_DIR)):
        if not name.endswith(".md"):
            continue
        # A retired path kept as a pointer to the renamed document (see
        # gen_html_docs.RETIRED_MARKER). It cites nothing and defines no
        # section, so registering it here would only add an old filename to
        # the set of names a citation can resolve against -- exactly the
        # ambiguity the rename removed.
        if RETIRED_MARKER in open(
                os.path.join(DOC_DIR, name), encoding="utf-8").readline():
            continue
        text = open(os.path.join(DOC_DIR, name), encoding="utf-8").read()
        TEXT[name] = text.lower()
        nums, fence = set(), None
        for line in text.split("\n"):
            s = line.strip()
            m = FENCE.match(s)
            if m:
                fence = None if fence and s.startswith(fence) else (
                    fence or m.group(1))
                continue
            if fence:
                continue
            h = re.match(r"^#{2,4} +((?:[0-9]+(?:\.[0-9]+)*[a-z]?|[A-Z]\.[0-9]+))[. ]",
                         line)
            if h:
                nums.add(h.group(1).rstrip("."))
            ap = re.match(r"^## +Appendix ([A-Z])\b", line)
            if ap:
                nums.add(ap.group(1))
        BY_NUMBER[name] = nums
    if SPEC not in BY_NUMBER:
        raise SystemExit("check_doc_refs.py: doc/%s not found" % SPEC)


def _cue_doc(window):
    """The document a text window names, and how far into it the name sits."""
    best, pos = None, -1
    for pattern, doc in CUES:
        for m in re.finditer(pattern, window, re.I):
            if m.start() > pos:
                best, pos = doc, m.start()
    for m in LINK.finditer(window):
        if m.start() > pos and m.group(1) in BY_NUMBER:
            best, pos = m.group(1), m.start()
    for m in DOCN.finditer(window):
        if m.start() > pos:
            for name in BY_NUMBER:
                if name.startswith(m.group(1) + "_"):
                    best, pos = name, m.start()
    return best, pos


def resolve_doc(text, start, end, default):
    """Which document a citation at [start:end) is talking about.

    In order: the link it is written inside ("[FAQ §13 — …](02_bovnar_faq.md)"),
    the document named right after it ("§11 of the unit-system reference"), the
    nearest document named before it, and otherwise the default -- the spec from
    source, the containing document from inside doc/."""
    line_start = text.rfind("\n", 0, start) + 1
    line_end = text.find("\n", end)
    line = text[line_start:line_end if line_end != -1 else len(text)]
    off = start - line_start
    open_b = line.rfind("[", 0, off)
    close_b = line.find("](", off)
    if open_b != -1 and close_b != -1 and "]" not in line[open_b + 1:off]:
        m = LINK.match(line[close_b:])
        if m and m.group(1) in BY_NUMBER:
            return m.group(1)

    # "§3.26-3.27 of the unit-system reference": the range has to be stepped
    # over before the name that resolves both of its ends comes into view.
    after = re.sub(r"\s+", " ", text[end:end + 72])
    m = re.match(r"(?:[-\u2013\u2014]\s?[0-9.]+[a-z]?)? ?(?:of|in|,) (?:the )?[\[(`\"']*(.*)",
                 after)
    if m:
        doc, pos = _cue_doc(m.group(1)[:44])
        if doc is not None and pos <= 2:
            return doc

    # A short window on purpose. At seventy characters a document named in the
    # PREVIOUS clause -- "(§10.4 of the unit-system reference), array element
    # homogeneity (§7.4)" -- captured the next citation, which is the spec's.
    before = re.sub(r"\s+", " ", text[max(0, start - 34):start])
    doc, _pos = _cue_doc(before)
    return doc if doc is not None else default


def check_file(path, rel, verbose):
    text = open(path, encoding="utf-8", errors="replace").read()
    if "§" not in text:
        return []
    inside_doc = rel.startswith("doc/") and rel.endswith(".md")
    default = os.path.basename(rel) if inside_doc else SPEC
    if default not in BY_NUMBER:
        default = SPEC
    problems = []
    for m in REF.finditer(text):
        before = text[max(0, m.start() - 34):m.start()]
        if EXTERNAL.search(before):
            continue
        # The line's own opt-out. Read from the whole line rather than the
        # 34-character window the cues use, so the marker can sit at the end of
        # the sentence it exempts instead of having to crowd the citation.
        ls = text.rfind("\n", 0, m.start()) + 1
        le = text.find("\n", m.end())
        if NO_CHECK in text[ls:le if le != -1 else len(text)]:
            continue
        doc = resolve_doc(text, m.start(), m.end(), default)
        line = text.count("\n", 0, m.start()) + 1
        quoted, number = m.group(1), m.group(2)
        if quoted:
            if quoted.lower() not in TEXT[doc]:
                problems.append((line, f'§"{quoted}" is not text in doc/{doc}'))
            elif verbose:
                print(f'  ok  {rel}:{line}: §"{quoted}" -> doc/{doc}')
            continue
        if number not in BY_NUMBER[doc]:
            problems.append((line, f"§{number} is not a section of doc/{doc}"))
        elif verbose:
            print(f"  ok  {rel}:{line}: §{number} -> doc/{doc}")
    return problems


# ── §16.10's reprint of error_code_t vs the header it reprints ──────────────
HEADER = os.path.join(ROOT, "include", "bovnar.h")
ENUM_SECTION = "### 16.10 Error Codes"
ENUMERATOR = re.compile(r"\b(error_[a-z_0-9]+)\s*=\s*(\d+)")


def read_enum(text):
    """name -> value for every explicitly numbered error_code_t enumerator."""
    return {n: int(v) for n, v in ENUMERATOR.findall(text)}


def check_error_enum(verbose=False):
    """The spec's §16.10 listing must match include/bovnar.h name for name."""
    if not os.path.exists(HEADER):
        return []
    spec_path = os.path.join(DOC_DIR, SPEC)
    spec_text = open(spec_path, encoding="utf-8").read()
    if ENUM_SECTION not in spec_text:
        return [f"doc/{SPEC}: no \"{ENUM_SECTION}\" section to check the enum against"]

    header = read_enum(open(HEADER, encoding="utf-8").read())
    listed = read_enum(spec_text.split(ENUM_SECTION, 1)[1])
    problems = []
    for name in sorted(set(header) - set(listed), key=lambda n: header[n]):
        problems.append(
            f"doc/{SPEC} §16.10: {name} = {header[name]} is in include/bovnar.h "
            f"but not in the listing")
    for name in sorted(set(listed) - set(header), key=lambda n: listed[n]):
        problems.append(
            f"doc/{SPEC} §16.10: {name} = {listed[name]} is listed but not in "
            f"include/bovnar.h")
    for name in sorted(set(listed) & set(header), key=lambda n: header[n]):
        if listed[name] != header[name]:
            problems.append(
                f"doc/{SPEC} §16.10: {name} is {listed[name]} in the listing "
                f"and {header[name]} in include/bovnar.h")
    if verbose and not problems:
        print(f"  ok  doc/{SPEC} §16.10: {len(listed)} error codes match "
              f"include/bovnar.h")
    return problems


# ── a document's `typedef struct … } name_t;` vs the header it restates ─────
#
# The same rot as §16.10's enum, one layer over: a document reprints a public
# struct, the header grows a field, and the listing silently becomes a different
# type from the one it claims to show. Three had drifted at once --
# bvnr_unit_policy_t was missing `rules`/`num_rules`, the two fields the example
# directly beneath it assigns; bvnr_read_flags_t in the spec was missing
# `text_only`, which §16.10 already names as what raises code 51; and
# bvnr_data_t in doc/05 was missing `converted`/`conv`, the two fields a unit
# conversion reports through, in the document about units.
#
# A listing may be curated on purpose: doc/08 shows "the most important fields"
# of bvnr_read_flags_t, and regroups bvnr_write_flags_t into what the writer
# enforces and what is there for symmetry with the reader -- both clearer than
# declaration order, and both signposted. An ellipsis comment inside the braces
# is how a listing declares itself curated, and is what exempts it from the
# field-for-field and ordering comparisons. What it does list must still exist:
# a curated listing may omit and reorder, never invent.
INCLUDE_DIR = os.path.join(ROOT, "include")
STRUCT = re.compile(r"typedef struct\s+\w*\s*\{(.*?)\}\s*(\w+_t)\s*;", re.S)
ELLIPSIS = re.compile(r"/\*[^*]*\.\.\.")


def _struct_fields(body):
    body = re.sub(r"/\*.*?\*/", "", body, flags=re.S)
    body = re.sub(r"//[^\n]*", "", body)
    out = []
    for stmt in body.split(";"):
        stmt = stmt.strip()
        if not stmt:
            continue
        m = re.search(r"([A-Za-z_]\w*)\s*(\[[^\]]*\])?$", stmt)
        if m:
            out.append(m.group(1))
    return out


def check_struct_listings(verbose=False):
    """Every struct a document reprints must match the header declaring it."""
    if not os.path.isdir(INCLUDE_DIR):
        return []
    header_structs = {}
    for fn in sorted(os.listdir(INCLUDE_DIR)):
        if not fn.endswith(".h"):
            continue
        text = open(os.path.join(INCLUDE_DIR, fn), encoding="utf-8").read()
        for body, name in STRUCT.findall(text):
            header_structs[name] = (_struct_fields(body), fn)

    problems = []
    checked = 0
    for fn in sorted(os.listdir(DOC_DIR)):
        if not fn.endswith(".md"):
            continue
        rel = f"doc/{fn}"
        text = open(os.path.join(DOC_DIR, fn), encoding="utf-8").read()
        for m in STRUCT.finditer(text):
            body, name = m.group(1), m.group(2)
            if name not in header_structs:
                continue
            line = text[:m.start()].count("\n") + 1
            listed = _struct_fields(body)
            actual, where = header_structs[name]
            checked += 1
            curated = bool(ELLIPSIS.search(body))
            missing = [] if curated else [f for f in actual if f not in listed]
            invented = [f for f in listed if f not in actual]
            order = [f for f in listed if f in actual]
            if curated:
                order = [f for f in actual if f in listed]
            if missing:
                problems.append(
                    f"{rel}:{line}: the {name} listing is missing "
                    f"{', '.join(missing)} — in include/{where}. Add the "
                    f"field, or, if the listing is a curated excerpt, say so "
                    f"with a '/* ... */' comment inside the braces.")
            if invented:
                problems.append(
                    f"{rel}:{line}: the {name} listing has "
                    f"{', '.join(invented)}, which include/{where} does not "
                    f"declare.")
            if order != [f for f in actual if f in listed]:
                problems.append(
                    f"{rel}:{line}: the {name} listing orders its fields "
                    f"differently from include/{where}.")
    if verbose and not problems:
        print(f"  ok  {checked} struct listing(s) match include/")
    return problems


def main(argv):
    verbose = "--verbose" in argv
    load_docs()
    failed = total = 0
    # The repository root, non-recursively: os.walk would descend into build/
    # and into the directories SEARCH_DIRS already covers, and would scan each of
    # those twice.
    scan = [(os.path.join(ROOT, fn), fn)
            for fn in sorted(os.listdir(ROOT))
            if fn.endswith(SUFFIXES)
            and fn not in ROOT_SKIP_FILES
            and os.path.isfile(os.path.join(ROOT, fn))]
    for d in SEARCH_DIRS:
        base = os.path.join(ROOT, d)
        if not os.path.isdir(base):
            continue
        for dirpath, dirnames, filenames in os.walk(base):
            dirnames[:] = [x for x in dirnames if x not in SKIP_DIRS]
            for fn in sorted(filenames):
                if not fn.endswith(SUFFIXES):
                    continue
                path = os.path.join(dirpath, fn)
                rel = os.path.relpath(path, ROOT).replace(os.sep, "/")
                scan.append((path, rel))
    for path, rel in scan:
        problems = check_file(path, rel, verbose)
        total += 1
        for line, msg in problems:
            failed += 1
            print(f"{rel}:{line}: {msg}")
    if failed:
        print(f"\n{failed} citation(s) point at a section that does not exist.")
        print("Renumbering a document means carrying its citations over with "
              "it — see the header of check_doc_refs.py.")
        return 1

    enum_problems = check_error_enum(verbose)
    for msg in enum_problems:
        print(msg)
    if enum_problems:
        print(f"\n{len(enum_problems)} error code(s) disagree between "
              f"doc/{SPEC} §16.10 and include/bovnar.h.")
        print("The header is the source; §16.10 reprints it. Adding a code "
              "means adding it there too — see the header of "
              "check_doc_refs.py.")
        return 1

    struct_problems = check_struct_listings(verbose)
    for msg in struct_problems:
        print(msg)
    if struct_problems:
        print(f"\n{len(struct_problems)} struct listing(s) disagree with the "
              f"headers they reprint.")
        print("The header is the source; a document restating a struct has to "
              "carry its fields over — see the header of check_doc_refs.py.")
        return 1

    print(f"check_doc_refs: every section citation across {total} files resolves, "
          f"§16.10 matches include/bovnar.h, and every struct listing matches "
          f"its header.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
