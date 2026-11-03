#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_citation.py — CITATION.cff is a schema-valid CFF 1.2 file.

THE GAP THIS CLOSES. CITATION.cff is read by machines and by nobody else: GitHub's
"Cite this repository" button, Zenodo's deposit form, cffconvert rendering BibTeX
or APA. When it is wrong they do not report an error to the repository — they
degrade, or fall back, or drop a field, and the repository looks fine from the
inside. That is exactly what happened here: one line, `abbreviation: BVNR`, sat at
the TOP LEVEL of the file, where CFF 1.2 sets `additionalProperties: false`. The
key is real and legal — inside a `references:` entry. At the top level it made the
whole document fail validation, silently, for as long as it was there.

It matters more now than it did, because that file no longer only says who to cite:
its `references:` block carries the machine-readable attribution for the six unit
vocabularies whose identifiers this project ships (THIRD_PARTY_NOTICES.md, doc/11
§18). Attribution that fails to parse is attribution nobody receives.

WHY THIS DOES NOT USE A SCHEMA VALIDATOR. cffconvert and jsonschema would check
far more than this does, and neither can be a test dependency: every gate in this
repository runs on a clean checkout with Python 3 and a C compiler, no network and
no pip. So this checks the one axis that actually broke, and checks it exactly:
the set of keys at the top level, against the CFF 1.2 allow-list, plus the three
keys every `references:` entry is required to carry. It is a spelling checker, not
a schema validator, and it says so rather than implying a completeness it has not
got. A full validation stays a manual step:

    pip install cffconvert && cffconvert --validate -i CITATION.cff

WHY NOT PyYAML. It is not a dependency of anything else here, and the structure
this needs is trivially visible without it: a top-level key is a key at column 0,
and a reference entry is a `- ` item under `references:`. Parsing YAML properly to
find out whether a line starts at column 0 would add a dependency to answer a
question the bytes already answer.

Usage:
    python3 check_citation.py [path/to/CITATION.cff]
"""
import os
import re
import sys

REPO = os.path.dirname(os.path.abspath(__file__))

# CFF 1.2.0 top-level keys, from the published schema
# (https://citation-file-format.github.io/1.2.0/schema.json). The schema sets
# additionalProperties: false, so anything outside this set is a hard failure --
# not a warning, and not a key that "will be ignored".
TOP_LEVEL = {
    "abstract", "authors", "cff-version", "commit", "contact", "date-released",
    "doi", "identifiers", "keywords", "license", "license-url", "message",
    "preferred-citation", "references", "repository", "repository-artifact",
    "repository-code", "title", "type", "url", "version",
}

# Required at the top level by the same schema.
REQUIRED = {"authors", "cff-version", "message", "title"}

# Required in every entry of `references:`.
REF_REQUIRED = {"authors", "title", "type"}


def top_level_keys(lines):
    """Keys at column 0. A CFF document is a mapping, so that is exactly the
    top-level key set -- no YAML parser needed to see it."""
    out = []
    for n, line in enumerate(lines, 1):
        m = re.match(r"^([A-Za-z][A-Za-z0-9_-]*):", line)
        if m:
            out.append((m.group(1), n))
    return out


def reference_entries(lines):
    """Each `- ` item under `references:`, as (start_line, {keys}).

    Ends at the next column-0 key, which is where the block ends in any valid
    CFF file.

    THE ONE THING THIS HAS TO GET RIGHT IS NESTING, and the first version did
    not: it treated every `- ` line as a new entry, so the `- name:` items inside
    an entry's own `authors:` list each became an entry of their own -- and every
    one of those "entries" was of course missing type, title and authors. The
    file was valid and this said it was broken nine times over.

    So the item indent is fixed by the FIRST `- ` seen under `references:` and
    only that indent starts an entry; anything deeper belongs to the entry above.
    Keys of an entry are those two columns further in, which is where a mapping
    under `- ` sits."""
    entries, in_refs = [], False
    item_indent = None
    for n, line in enumerate(lines, 1):
        if re.match(r"^references:", line):
            in_refs = True
            continue
        if in_refs and re.match(r"^[A-Za-z]", line):
            break                                   # next top-level key
        if not in_refs or not line.strip() or line.lstrip().startswith("#"):
            continue
        m = re.match(r"^(\s*)-\s+([A-Za-z][A-Za-z0-9_-]*):", line)
        if m:
            depth = len(m.group(1))
            if item_indent is None:
                item_indent = depth
            if depth == item_indent:
                entries.append([n, {m.group(2)}])
                continue
            # deeper: an item of a list nested inside the current entry
            continue
        if entries and item_indent is not None:
            m = re.match(r"^(\s*)([A-Za-z][A-Za-z0-9_-]*):", line)
            if m and len(m.group(1)) == item_indent + 2:
                entries[-1][1].add(m.group(2))
    return entries


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(REPO, "CITATION.cff")
    if not os.path.isfile(path):
        sys.exit(f"check_citation: no such file: {path}")
    lines = open(path, encoding="utf-8").read().split("\n")

    errors = []
    keys = top_level_keys(lines)
    seen = {k for k, _ in keys}

    for k, n in keys:
        if k not in TOP_LEVEL:
            hint = ""
            if k == "abbreviation":
                hint = ("  (legal inside a `references:` entry, never at the "
                        "top level)")
            errors.append(f"{path}:{n}: '{k}' is not a CFF 1.2 top-level "
                          f"key{hint}")

    for k in sorted(REQUIRED - seen):
        errors.append(f"{path}: required top-level key '{k}' is missing")

    dupes = sorted({k for k, _ in keys if [x for x, _ in keys].count(k) > 1})
    for k in dupes:
        errors.append(f"{path}: top-level key '{k}' appears more than once")

    refs = reference_entries(lines)
    for start, kset in refs:
        for k in sorted(REF_REQUIRED - kset):
            errors.append(f"{path}:{start}: reference entry is missing the "
                          f"required key '{k}'")

    if errors:
        for e in errors:
            print(e, file=sys.stderr)
        print(f"\ncheck_citation: {len(errors)} problem(s). A CFF file that "
              f"fails validation is not reported by GitHub or Zenodo -- they "
              f"degrade quietly, which is why this is a build gate.",
              file=sys.stderr)
        return 1

    print(f"check_citation: {len(seen)} top-level key(s) and {len(refs)} "
          f"reference(s) in CITATION.cff, all named by CFF 1.2 "
          f"(key spelling only; run cffconvert --validate for the full schema)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
