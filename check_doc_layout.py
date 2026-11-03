#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_doc_layout.py — every document in doc/ has the same shape, and its Table
of Contents is the list of sections it actually contains.

The documentation set is eleven Markdown files written over as many months, and
they had drifted into eleven layouts: four had no Table of Contents at all, the
seven that did called it "Contents" or "Table of Contents" about evenly, the
metadata under the title used five different key sets, and section numbering was
present in six documents and absent in five. A reader who opened two of them in
a row could not tell they belonged to one set.

Layout drift is invisible to every other gate here — a document with no TOC
renders perfectly, publishes perfectly, and every link in it resolves. So the
shape is pinned in one place instead of being restated in eleven:

  * "# Title" on line 1, then a blockquote of exactly
    "**Spec version:**", "**Status:**", "**Scope:**";
  * a "## Table of Contents" section, before the first numbered section;
  * "## N. Title" sections numbered 1..N without gaps (appendices excepted),
    "### N.M Title" subsections numbered within their section, each "##"
    preceded by a "---" rule;
  * a TOC that lists every "##" and "###" heading, in document order, with the
    anchor each heading actually gets — this is the part that rots fastest,
    because adding a section is one edit and remembering the TOC is another;
  * a "## See also" section and the closing "*End of … (Bovnar spec 1.2).*"
    line.

The anchor slugs are computed with gen_html_docs.gh_slug — the same function
that assigns the ids on the published HTML pages — so a TOC that passes here
cannot be a TOC whose links 404 on the site. check_web_links.py then proves the
same for links between documents; this one is about a document's own structure.

Usage:
    python3 check_doc_layout.py            # report and exit 1 on any drift
    python3 check_doc_layout.py --verbose  # also list what was checked
"""
import collections
import os
import re
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
DOC_DIR = os.path.join(ROOT, "doc")

FENCE = re.compile(r"^(```|~~~)")
# A retired path kept as a pointer to the renamed document (the IANA
# registration cites five documents by their pre-renumbering path; see
# gen_html_docs.RETIRED_MARKER). It is three sentences and a link, so it has no
# TOC, no numbered sections and no footer, and the layout below does not apply.
# Spelled out here rather than imported for the reason given at gh_slug: this
# checker takes no third-party dependency, and importing gen_html_docs would
# give it markdown and gen_csp.
RETIRED_MARKER = "bovnar:retired-path"
NO_NUMBER = ("Table of Contents", "See also")
FOOTER = re.compile(r"^\*End of .+ \(Bovnar spec 1\.2\)\.\*$")
META_KEYS = ["**Spec version:**", "**Status:**", "**Scope:**"]


def is_retired(path):
    """True if this file is a pointer to a renamed document, not a document."""
    try:
        with open(path, encoding="utf-8") as fh:
            return RETIRED_MARKER in fh.readline()
    except OSError:
        return False


def check_retired(name):
    """A pointer names the document it points at, and that document exists.

    The pointers are skipped by the layout check above, so without this they
    are the one thing in doc/ nothing looks at — and their whole job is to hold
    a link that outlives a rename. A pointer to a file the next renumbering
    moved again is worse than no pointer: it answers the question wrongly.
    """
    with open(os.path.join(DOC_DIR, name), encoding="utf-8") as fh:
        first, body = fh.readline(), fh.read()
    m = re.search(RETIRED_MARKER + r"\s*->\s*([\w.]+)", first)
    if not m:
        return [f"line 1 carries {RETIRED_MARKER} but names no target; "
                f"expected '{RETIRED_MARKER} -> <current-filename>'"]
    target = m.group(1)
    problems = []
    if not os.path.exists(os.path.join(DOC_DIR, target)):
        problems.append(f"points at doc/{target}, which does not exist")
    if target == name:
        problems.append("points at itself")
    # Against the body only. Matching the whole file would match the marker on
    # line 1 and pass every time -- which is what it did.
    if target not in body:
        problems.append(f"line 1 names doc/{target} but the body never says so; "
                        f"a reader who lands here sees no way on")
    return problems


def gh_slug(text):
    """GitHub-style heading slug — the one gen_html_docs.py stamps on the
    published pages. Kept byte-identical to that function on purpose: a TOC
    checked against a different slugifier is not checked at all."""
    s = text.strip().lower()
    s = s.replace("`", "").replace("*", "")
    s = re.sub(r"<[^>]+>", "", s)
    s = re.sub(r"[^\w\s-]", "", s)
    return s.strip().replace(" ", "-")


def parse(text):
    """Return (lines, headings) where headings is [(lineno, level, title,
    anchor)] for every heading outside a fenced code block. Fences matter:
    doc/01 alone has twenty '#' comment lines inside ```bovnar blocks."""
    lines = text.split("\n")
    headings = []
    seen = collections.Counter()
    fence = None
    for i, line in enumerate(lines, 1):
        stripped = line.strip()
        m = FENCE.match(stripped)
        if m:
            if fence is None:
                fence = m.group(1)
            elif stripped.startswith(fence):
                fence = None
            continue
        if fence is not None:
            continue
        h = re.match(r"^(#{1,3}) +(.*?)\s*$", line)
        if not h:
            continue
        title = h.group(2)
        base = gh_slug(title)
        n = seen[base]
        seen[base] += 1
        anchor = base if n == 0 else f"{base}-{n}"
        headings.append((i, len(h.group(1)), title, anchor))
    return lines, headings


def expected_toc(headings):
    """The TOC rows the document's own headings imply, in order.

    Two pieces of whitespace here are load-bearing, both learned from what the
    published pages did with the hand-written lists this replaced. A subsection
    is indented four spaces, not three: python-markdown nests on four, and at
    three every sub-entry rendered as literal "- 3.1 SI Base Units" text glued
    onto the end of its parent line. And a bullet row that follows the numbered
    list (an appendix, "See also") gets a blank line before it, or it is
    swallowed into the last numbered item the same way."""
    rows = []

    def top(row):
        if row.startswith("- ") and rows and not rows[-1].startswith(("- ", " ")):
            rows.append("")
        rows.append(row)

    for _ln, lvl, title, anchor in headings:
        if lvl == 1:
            continue
        if title in NO_NUMBER:
            if title == "See also":
                top(f"- [See also](#{anchor})")
            continue
        m = re.match(r"^((?:\d+\.|\d+(?:\.\d+)+|[A-Z]\.\d+))\s+(.*)$", title)
        if lvl == 2:
            if title.startswith("Appendix ") or not m:
                top(f"- [{title}](#{anchor})")
            else:
                rows.append(f"{m.group(1)} [{m.group(2)}](#{anchor})")
        else:
            label = m.group(2) if m else title
            num = (m.group(1) + " ") if m else ""
            rows.append(f"    - {num}[{label}](#{anchor})")
    return rows


def actual_toc(lines):
    """The rows written under '## Table of Contents', up to its closing rule."""
    try:
        i = next(k for k, l in enumerate(lines)
                 if l.strip() == "## Table of Contents")
    except StopIteration:
        return None
    j = next(k for k in range(i + 1, len(lines)) if lines[k].strip() == "---")
    rows = lines[i + 1:j]
    while rows and not rows[0].strip():
        rows.pop(0)
    while rows and not rows[-1].strip():
        rows.pop()
    return rows


def check(path, verbose=False):
    rel = os.path.relpath(path, ROOT)
    text = open(path, encoding="utf-8").read()
    lines, headings = parse(text)
    problems = []

    # ── title and metadata block ────────────────────────────────────────────
    if not lines or not lines[0].startswith("# "):
        problems.append("no '# Title' on line 1")
    meta = [l for l in lines[:8] if l.startswith("> ")]
    keys = [l[2:].split(" ", 2)[0] + " " + l[2:].split(" ", 2)[1]
            if len(l[2:].split(" ", 2)) > 1 else l[2:] for l in meta]
    have = [k for k in META_KEYS
            if any(l.startswith("> " + k) for l in meta)]
    if have != META_KEYS:
        problems.append(
            "metadata blockquote must be exactly "
            f"{', '.join(META_KEYS)} — found {meta and keys or 'nothing'}")

    # ── section numbering ───────────────────────────────────────────────────
    section = 0
    label = None
    sub = 0
    for ln, lvl, title, _anchor in headings:
        if lvl == 2:
            sub = 0
            if title in NO_NUMBER:
                label = None
                continue
            ap = re.match(r"^Appendix ([A-Z]): ", title)
            if ap:
                label = ap.group(1)
                continue
            m = re.match(r"^(\d+)\. \S", title)
            if not m:
                problems.append(f"line {ln}: section is not numbered: '{title}'")
                label = None
                continue
            section += 1
            if int(m.group(1)) != section:
                problems.append(
                    f"line {ln}: section numbered {m.group(1)}, expected "
                    f"{section}: '{title}'")
            label = m.group(1)
            if ln >= 3 and lines[ln - 3].strip() != "---":
                problems.append(f"line {ln}: no '---' rule before '{title}'")
        elif lvl == 3:
            if label is None:
                continue
            sub += 1
            m = re.match(r"^(%s)\.(\d+) \S" % re.escape(label), title)
            if not m:
                problems.append(
                    f"line {ln}: subsection is not numbered {label}.{sub}: "
                    f"'{title}'")
            elif int(m.group(2)) != sub:
                problems.append(
                    f"line {ln}: subsection numbered {m.group(1)}.{m.group(2)},"
                    f" expected {label}.{sub}: '{title}'")

    # ── table of contents ───────────────────────────────────────────────────
    toc = actual_toc(lines)
    if toc is None:
        problems.append("no '## Table of Contents' section")
    else:
        first = next((ln for ln, lvl, t, _ in headings
                      if lvl == 2 and t not in NO_NUMBER), None)
        toc_ln = next(ln for ln, lvl, t, _ in headings
                      if lvl == 2 and t == "Table of Contents")
        if first is not None and toc_ln > first:
            problems.append("the Table of Contents is not at the top")
        want = expected_toc(headings)
        for a, b in zip(toc, want):
            if a.rstrip() != b:
                problems.append(f"TOC row differs\n     have: {a.rstrip()}\n     want: {b}")
        if len(toc) != len(want):
            extra = toc[len(want):] or want[len(toc):]
            problems.append(
                f"TOC lists {len(toc)} rows, the document has {len(want)} "
                f"headings — first unmatched: {extra[0].strip()}")

    # ── see also and closing line ───────────────────────────────────────────
    if not any(lvl == 2 and t == "See also" for _ln, lvl, t, _a in headings):
        problems.append("no '## See also' section")
    tail = [l for l in lines if l.strip()]
    title = lines[0][2:].strip() if lines and lines[0].startswith("# ") else ""
    want = f"*End of {title} (Bovnar spec 1.2).*"
    if not tail:
        problems.append("empty document")
    elif not FOOTER.match(tail[-1].strip()):
        problems.append(
            f"last line must be {want!r} — found {tail[-1].strip()[:60]!r}")
    elif tail[-1].strip() != want:
        # Naming the document twice is how the closing line ends up describing
        # a document that was renamed at the top and nowhere else.
        problems.append(
            f"closing line names {tail[-1].strip()[:60]!r}, the title is "
            f"{title!r}")

    if verbose and not problems:
        n2 = sum(1 for _l, lvl, t, _a in headings
                 if lvl == 2 and t not in NO_NUMBER)
        n3 = sum(1 for _l, lvl, _t, _a in headings if lvl == 3)
        print(f"  ok  {rel}: {n2} sections, {n3} subsections, TOC matches")
    return problems


def main(argv):
    verbose = "--verbose" in argv
    everything = sorted(f for f in os.listdir(DOC_DIR)
                        if f.endswith((".md", ".ebnf")))
    retired = [f for f in everything
               if is_retired(os.path.join(DOC_DIR, f))]
    docs = [f for f in everything if f.endswith(".md") and f not in retired]
    if not docs:
        print("check_doc_layout: no documents found in doc/", file=sys.stderr)
        return 1
    failed = 0
    for name in docs:
        problems = check(os.path.join(DOC_DIR, name), verbose)
        if problems:
            failed += 1
            print(f"doc/{name}:")
            for p in problems:
                print(f"   - {p}")
    for name in retired:
        problems = check_retired(name)
        if problems:
            failed += 1
            print(f"doc/{name}:")
            for p in problems:
                print(f"   - {p}")
        elif verbose:
            print(f"  ok  doc/{name}: pointer to a renamed document")
    if failed:
        print(f"\n{failed} of {len(docs) + len(retired)} documents do not match "
              f"the layout.")
        print("The layout is described at the top of check_doc_layout.py.")
        return 1
    print(f"check_doc_layout: {len(docs)} documents share one layout"
          + (f", and {len(retired)} retired paths point into them." if retired
             else "."))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
