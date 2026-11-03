#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_conformance_doc.py — doc/13's worked examples must be what the adapter emits.

doc/13 is the specification a third-party implementor works from: it states that
an implementation is conformant when its IUT adapter produces output
"byte-for-byte identical to the reference for every test case". The worked
examples are therefore normative in practice — they are what somebody copies.

Nothing checked them, and the suite structurally could not: bvnr_conformance_iut
compares the adapter's log against the reference's INTERNAL capture, and the two
formatters are line-for-line duplicates of each other. Both agreeing says nothing
about either matching the document. Four of the six examples were wrong:

  * TYPE_ANN_START / TYPE_FAMILY / TYPE_ANN_END were shown carrying the family
    keyword ("float"); they carry the whole annotation body ("float:64,m/s").
    True only for a SYNTHESISED annotation, which is what the other examples use.
  * TYPE_PARAM_BASE was shown for <float:64,m/s>, which names no base and emits
    no such line.
  * the array example repeated the whole annotation block for every element; it
    is emitted once per row.
  * the datetime example (added with this checker) needs the spec-1.1 directive
    to parse at all.

Every "**Input:** `<source>`" followed by a fenced block is replayed through the
adapter and compared line by line. A block containing an ERROR line is checked
the same way, so the error examples are covered too.

Usage:
    python3 check_conformance_doc.py <path-to-bvnr_conformance_iut>
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
DOC = os.path.join(ROOT, "doc", "13_bovnar_conformance.md")

# "**Input:** `<src>`" + optional prose to end of line, blank line, fenced block.
_EXAMPLE_RE = re.compile(
    r'\*\*Input:\*\*\s*(.+?)\n\n```\n(.*?)```', re.S)
# The source is written as one or more `backticked` fragments joined by prose
# ("`#!bovnar 1.1` + `.t = ...;`"); take the fragments in order, one per line.
_FRAGMENT_RE = re.compile(r'`([^`]+)`')


def examples(text):
    for m in _EXAMPLE_RE.finditer(text):
        frags = _FRAGMENT_RE.findall(m.group(1))
        if not frags:
            continue
        yield "\n".join(frags) + "\n", m.group(2), m.group(1).strip()


def main():
    if len(sys.argv) < 2:
        print("usage: check_conformance_doc.py <bvnr_conformance_iut>",
              file=sys.stderr)
        return 2
    iut = sys.argv[1]
    if not os.path.isfile(iut):
        print("check_conformance_doc.py: no adapter at %s" % iut, file=sys.stderr)
        return 2
    with open(DOC, encoding="utf-8") as f:
        text = f.read()
    cases = list(examples(text))
    if not cases:
        print("check_conformance_doc.py: no worked examples found in doc/13 — the "
              "format changed and this check would pass vacuously", file=sys.stderr)
        return 1
    bad = 0
    for src, want, label in cases:
        r = subprocess.run([iut], input=src.encode("utf-8"),
                           stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        got_lines = [l for l in r.stdout.decode("utf-8", "replace").splitlines() if l]
        want_lines = [l for l in want.splitlines() if l]
        if got_lines == want_lines:
            continue
        bad += 1
        print("check_conformance_doc.py: doc/13 example does not match the adapter"
              "\n  input: %s" % label, file=sys.stderr)
        for i in range(max(len(want_lines), len(got_lines))):
            a = want_lines[i] if i < len(want_lines) else "(absent)"
            b = got_lines[i] if i < len(got_lines) else "(absent)"
            if a != b:
                print("      doc: %-34s adapter: %s" % (a, b), file=sys.stderr)
    if bad:
        print("check_conformance_doc.py: %d of %d worked example(s) wrong"
              % (bad, len(cases)), file=sys.stderr)
        return 1
    print("check_conformance_doc.py: all %d worked examples in doc/13 match the "
          "reference adapter byte for byte" % len(cases))
    return 0


if __name__ == "__main__":
    sys.exit(main())
