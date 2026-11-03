#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_ietf_draft.py — the IETF draft and the documentation set say what the
header says.

THE GAP THIS CLOSES. doc/ietf/draft-sonntag-bovnar-00 is a second, independent
statement of the format, written for a different audience and rendered into four
committed artifacts (.md, .txt, .xml, .html). Nothing compared it to anything.
check_doc_refs.py checks that citations RESOLVE and that the spec's error table
matches include/bovnar.h; check_doc_layout.py checks structure. Neither looks at
the draft at all, and neither checks a numeric CLAIM anywhere.

So the draft drifted, and so did doc/05: both said a compound unit may not exceed
"8 components" long after BVNR_MAX_UNIT_COMPONENTS became 32. doc/03 said 32. A
reader implementing from the draft would have rejected documents the reference
implementation accepts -- an interoperability defect, in the one document whose
entire purpose is interoperability.

WHAT IS CHECKED, AND WHY EACH ONE. Every check here is a fact stated in two
places that only the header can settle:

  * ERROR CODES. Every error_* the draft names must exist in error_code_t, and
    every code in error_code_t must be named by the draft. The first direction
    catches a typo or a renamed code; the second catches a new code the draft was
    never told about, which is how a normative document quietly stops being
    normative.

  * NUMERIC LIMITS. The draft's limits table and doc/05's, against the constants
    they cite. This is the check that would have caught the 8/32 drift on the
    commit that introduced it.

  * EPOCH NAMES and TYPE FAMILIES. Enumerations a consumer must implement
    exactly; a missing one is a document that cannot be implemented from.

  * SPEC VERSION. The draft describes a numbered specification; if the build's
    BVNR_SPEC_VERSION moves past it, the draft needs a new revision rather than
    silently describing an older format.

  * RENDERING AGREEMENT. The .txt, .xml and .html are generated from the .md by
    kramdown-rfc/xml2rfc, which are not run here and not in CI. A fix applied to
    the .md alone would leave three committed artifacts stating the old value, so
    every fact checked above is re-checked in each rendering. This is the check
    that makes "fix the draft" mean all four files.

WHAT IT COMPARES, AND WHY THE SET GREW. It began with the error-code enum and
the component limit -- the two things that had drifted. The EXPONENT RANGE was
added after doc/03 and all three renderings of the draft were found describing
the caret form as "one digit 1-9", the range unit_exponent_t had before it grew
to [-100, 100]: a parser built from either document rejects `m^100`, which this
library emits as the canonical spelling of exponent 100. A specification that
cannot describe its own reference implementation's output is the sharpest form
this failure takes.

The document set grew for the same reason. The tutorial's worked "too many
components" example and the bindings reference's account of the pint bridge were
both stating the old numbers, so the non-normative documents are checked too --
without the requirement to STATE a limit, since only the normative ones have
that duty, but with every figure they do state compared.

Usage:
    python3 check_ietf_draft.py            # report and exit 1 on any drift
    python3 check_ietf_draft.py --verbose  # also list what was checked
"""
import os
import re
import string
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
HDR = os.path.join(ROOT, "include", "bovnar.h")
IETF = os.path.join(ROOT, "doc", "ietf")
DRAFT_STEM = "draft-sonntag-bovnar-00"
DOC = os.path.join(ROOT, "doc")

# doc/05's own limits table restates the same constants for the unit half of the
# format. It is here rather than in check_doc_refs.py because it is the same
# question -- a number stated twice -- and answering it in two tools is how the
# two answers start to differ.
UNIT_DOC = os.path.join(DOC, "05_bovnar_unit_system.md")
SPEC_DOC = os.path.join(DOC, "03_bovnar_spec.md")


def die(problems):
    for p in problems:
        print("check_ietf_draft: " + p, file=sys.stderr)
    print("\n%d inconsistency(ies). The header is the authority: fix the "
          "document, or the header if the document is right." % len(problems),
          file=sys.stderr)
    return 1


def header_facts():
    """The authoritative values, read from include/bovnar.h."""
    h = open(HDR, encoding="utf-8").read()
    f = {}
    f["errors"] = set(re.findall(r"^\s*(error_\w+)\s*=\s*\d+", h, re.M))
    for name in ("BVNR_MAX_UNIT_COMPONENTS", "BVNR_UNIT_STRING_MAX",
                 "BVNR_SPEC_VERSION_MAJOR", "BVNR_SPEC_VERSION_MINOR"):
        m = re.search(r"#\s*define\s+%s\s+(\d+)" % name, h)
        if not m:
            raise SystemExit("check_ietf_draft: %s not in include/bovnar.h" % name)
        f[name] = int(m.group(1))
    # The exponent bounds are SIGNED and parenthesised -- "(-100)" / "( 100)" --
    # so they need their own pattern rather than the unsigned one above.
    for name in ("BVN_EXPONENT_MIN", "BVN_EXPONENT_MAX"):
        m = re.search(r"#\s*define\s+%s\s+\(\s*(-?\d+)\s*\)" % name, h)
        if not m:
            raise SystemExit("check_ietf_draft: %s not in include/bovnar.h" % name)
        f[name] = int(m.group(1))
    # Type families: the vt_* enumerators, minus the two that are not families a
    # document can write (vt_plain is "no annotation", vt_illegal is the error).
    f["families"] = set(re.findall(r"\bvt_(\w+)", h)) - {"plain", "illegal"}
    return f


def epoch_names():
    """The epoch table lives in the validator, not the header."""
    src = open(os.path.join(ROOT, "src", "utils", "bovnar_utils.c"),
               encoding="utf-8").read()
    m = re.search(r"bvn_epoch_table\[\]\s*=\s*\{(.*?)\n\};", src, re.S)
    if not m:
        return None
    return set(re.findall(r'"(\w+)"', m.group(1)))


def check_errors(text, where, facts, problems, verbose):
    cited = set(re.findall(r"error_[a-z0-9_]+", text))
    unknown = sorted(cited - facts["errors"])
    if unknown:
        problems.append("%s names %d error code(s) that are not in "
                        "error_code_t: %s" % (where, len(unknown),
                                              ", ".join(unknown)))
    missing = sorted(facts["errors"] - cited)
    if missing:
        problems.append("%s never mentions %d error code(s) the header "
                        "defines: %s" % (where, len(missing),
                                         ", ".join(missing)))
    if verbose and not unknown and not missing:
        print("  ok  %s: all %d error codes agree" % (where, len(cited)))


def check_component_limit(text, where, facts, problems, verbose,
                          required=True):
    """The claim that drifted. Matched on the sentence, not on a bare number, so
    an unrelated 8 elsewhere in the document cannot satisfy or break it.

    `required` is False for a document that has no duty to STATE the limit — a
    tutorial or a bindings reference may mention it or not — while any figure it
    does state is still checked. Only the normative documents must say it."""
    want = facts["BVNR_MAX_UNIT_COMPONENTS"]
    hits = re.findall(r"exceed[^.\n]*?(\d+)[^.\n]*?components?", text)
    hits += re.findall(r"components? per compound unit\D*?(\d+)", text)
    # "Too many components (9 > 8)" and "(9 > 8 max)" -- the worked EXAMPLES,
    # which the two patterns above never saw because they state the limit as
    # the right-hand side of a comparison rather than as a sentence. Three
    # documents carried a nine-component unit annotated as an error while the
    # limit had been 32 for some time, so a reader copying the example got a
    # valid document the comment called invalid.
    hits += re.findall(r"components?[^.\n]*?\d+\s*>\s*(\d+)", text)
    if not hits:
        if required:
            problems.append("%s states no compound-unit component limit; the "
                            "header says %d and a consumer implementing from "
                            "this document cannot know it" % (where, want))
        return
    bad = [h for h in hits if int(h) != want]
    if bad:
        problems.append("%s says a compound unit may not exceed %s components; "
                        "BVNR_MAX_UNIT_COMPONENTS is %d"
                        % (where, "/".join(sorted(set(bad))), want))
    elif verbose:
        print("  ok  %s: component limit %d" % (where, want))


def check_exponent_range(text, where, facts, problems, verbose):
    """The other claim that drifted, and drifted further than the first.

    doc/03 and all three renderings of the draft stated the caret exponent form
    as "^[+-]?[1-9]", one digit, and omitted U+2070 from the superscript table
    -- the range unit_exponent_t had before it grew to [-100, 100]. So an
    implementer building a conforming parser from either document rejected
    `m^100` and `m¹⁰⁰`, which this library not only accepts but EMITS: exponent
    100 is written back as `m¹⁰⁰`. A specification that cannot describe its own
    reference implementation's output is the sharpest form this failure takes,
    and nothing compared the two, which is how it survived the widening.

    Matched on the range as a PAIR, so a document that states one bound and not
    the other is caught too, and on the "one digit"/"[1-9]" phrasings that were
    the actual wording of the mistake."""
    lo = facts["BVN_EXPONENT_MIN"]
    hi = facts["BVN_EXPONENT_MAX"]

    # The old wording, in every spelling it appeared in -- INCLUDING ABNF.
    #
    # The three prose spellings were what this check began with, and they were
    # not enough: the draft's Appendix A said the same thing in ABNF, as
    # `%x31-39` and a `sup-digit` set with U+2070 missing, and no prose pattern
    # matches that. So the fix landed in the prose (Table 8 correctly says "one
    # to three digits" and `^[+-]?[0-9]{1,3}`) while the formal grammar an
    # implementer actually builds from kept the single-digit rule -- the
    # document disagreeing with itself, with the machine-readable half wrong.
    for pat, what in ((r"[Oo]ne digit 1-9", "\"one digit 1-9\""),
                      (r"\^\[\+-\]\?\[1-9\]", "`^[+-]?[1-9]`"),
                      (r"single digit", "\"single digit\""),
                      (r'caret-exp\s*=\s*"\^"\s*\[\s*"\+"\s*/\s*"-"\s*\]\s*%x31-39',
                       "the ABNF `caret-exp = \"^\" [ \"+\" / \"-\" ] %x31-39`"),
                      (r'\[\s*"\+"\s*/\s*"-"\s*\]\s*%x31-39\s*\)',
                       "the inline ABNF `[ \"+\" / \"-\" ] %x31-39`"),
                      (r"sup-exp\s*=\s*\[\s*sup-sign\s*\]\s*sup-digit\b",
                       "the ABNF `sup-exp = [ sup-sign ] sup-digit` "
                       "(one superscript digit)")):
        if re.search(pat, text):
            problems.append(
                "%s describes the exponent form as %s; the range is [%d, %d] "
                "and up to three digits are scanned, so a parser built from "
                "this document rejects `m^100` -- which the library writes"
                % (where, what, lo, hi))

    # And the range itself, wherever it is stated as a pair.
    hits = re.findall(r"\[\s*(-?\d+)\s*,\s*(-?\d+)\s*\]", text)
    ranges = [(int(a), int(b)) for a, b in hits
              if abs(int(a)) in (9, 100) and abs(int(b)) in (9, 100)]
    bad = [r for r in ranges if r != (lo, hi)]
    if bad:
        problems.append(
            "%s states an exponent range of %s; BVN_EXPONENT_MIN/MAX are "
            "[%d, %d]" % (where, "/".join("[%d, %d]" % r for r in sorted(set(bad))),
                          lo, hi))
    elif verbose and ranges:
        print("  ok  %s: exponent range [%d, %d]" % (where, lo, hi))


def check_base_unit_char(text, where, problems, verbose):
    """The draft's `base-unit-char` must admit every character a symbol uses.

    The draft is the ONLY document that enumerates the characters a unit symbol
    may be built from. doc/12's EBNF deliberately does not -- it says "a symbol
    looked up in the fixed unit table", deferring to the registry -- which is
    exactly why doc/12 did not go stale when the registry grew and the draft
    did. An enumeration is a copy, and a copy of a growing thing rots.

    It rotted twice, silently:
      * U+0394 GREEK CAPITAL DELTA, absent, which excluded all eight
        temperature-INTERVAL spellings (ΔK, Δ°C, Δ°F, Δ°De, Δ°N, Δ°Ra);
      * DIGIT, absent altogether, which excluded mH2O.
    A parser built from the draft rejected nine spellings this library accepts
    and emits.

    Checked against src/gendata/units.bvnr rather than against a list here, so
    the next symbol with an unusual character fails this instead of shipping."""
    m = re.search(r"base-unit-char\s*=(.*?)(?:\ncurrency-code|\n\s*\n)",
                  text, re.S)
    if not m:
        problems.append("%s has no base-unit-char production to check" % where)
        return
    block = m.group(1)

    allowed = set()
    if re.search(r"\bALPHA\b", block):
        allowed |= set(string.ascii_letters)
    if re.search(r"\bDIGIT\b", block):
        allowed |= set(string.digits)
    for lit in re.findall(r'"(.)"', block):
        allowed.add(lit)
    # %xC2.B0 and friends: UTF-8 byte sequences, decoded back to the character.
    for hexrun in re.findall(r"%x([0-9A-Fa-f]{2}(?:\.[0-9A-Fa-f]{2})*)", block):
        try:
            allowed.add(bytes(int(b, 16) for b in hexrun.split(".")).decode("utf-8"))
        except (UnicodeDecodeError, ValueError):
            pass
    # ...and the codepoints the comments name, which is how the ohm and the
    # angstrom are given (two encodings, one character each).
    for cp in re.findall(r"U\+([0-9A-Fa-f]{4,6})", block):
        allowed.add(chr(int(cp, 16)))

    spellings = registry_spellings()
    if spellings is None:
        if verbose:
            print("  ..  %s: base-unit-char not checked (no src/gendata)" % where)
        return
    missing = {}
    for text_ in spellings:
        for ch in text_:
            if ch not in allowed:
                missing.setdefault(ch, set()).add(text_)
    if missing:
        detail = "; ".join(
            "U+%04X (in %s)" % (ord(ch), ", ".join(sorted(ex)[:4]))
            for ch, ex in sorted(missing.items(), key=lambda kv: ord(kv[0])))
        problems.append(
            "%s: base-unit-char does not admit %d character(s) that real unit "
            "symbols use — %s. A parser built from this grammar rejects them, "
            "and the library accepts and emits them"
            % (where, len(missing), detail))
    elif verbose:
        print("  ok  %s: base-unit-char covers all %d registry spellings"
              % (where, len(spellings)))


def registry_spellings():
    """Every accepted input spelling in src/gendata/units.bvnr, or None."""
    repo = os.path.dirname(os.path.abspath(__file__))
    path = os.path.join(repo, "src", "gendata", "units.bvnr")
    if not os.path.exists(path):
        return None
    sys.path.insert(0, repo)
    try:
        import bvnr_data
        doc = bvnr_data.load(open(path, encoding="utf-8").read())
    except Exception:
        return None
    out = set()
    for u in doc.get("units", []):
        out.update(u.get("aliases", []))
        if u.get("symbol"):
            out.add(u["symbol"])
    return out


def check_macro_values(text, where, facts, problems, verbose):
    """Any '#define NAME <n>' or '| `NAME` | <n> |' restating a header macro."""
    for name in ("BVNR_MAX_UNIT_COMPONENTS", "BVNR_UNIT_STRING_MAX"):
        want = facts[name]
        for m in re.finditer(r"#\s*define\s+%s\s+(\d+)" % name, text):
            if int(m.group(1)) != want:
                problems.append("%s reprints `#define %s %s`; the header says %d"
                                % (where, name, m.group(1), want))
        for m in re.finditer(r"`%s`\s*\|\s*(\d+)" % name, text):
            if int(m.group(1)) != want:
                problems.append("%s tabulates %s as %s; the header says %d"
                                % (where, name, m.group(1), want))
        for m in re.finditer(r"\|\s*(\d+)\s*\(`%s`\)" % name, text):
            if int(m.group(1)) != want:
                problems.append("%s tabulates %s as %s; the header says %d"
                                % (where, name, m.group(1), want))
    if verbose:
        print("  ok  %s: macro restatements agree" % where)


def check_spec_version(text, where, facts, problems, verbose):
    want = "%d.%d" % (facts["BVNR_SPEC_VERSION_MAJOR"],
                      facts["BVNR_SPEC_VERSION_MINOR"])
    seen = set(re.findall(r"#!bovnar\s+(\d+\.\d+)", text))
    # A draft may legitimately show an OLDER version in an example (a 1.0
    # document is still valid), but it must describe the current one somewhere.
    if want not in seen:
        problems.append("%s never shows a `#!bovnar %s` document, but this build "
                        "implements spec %s -- the draft describes an older "
                        "format than the one that ships" % (where, want, want))
    ahead = sorted(v for v in seen
                   if tuple(int(x) for x in v.split(".")) >
                      (facts["BVNR_SPEC_VERSION_MAJOR"],
                       facts["BVNR_SPEC_VERSION_MINOR"]))
    if ahead and where.endswith(".md"):
        # Only a note: the unit-profile notation is deliberately written against
        # an unreleased version, and the draft may reference it.
        if verbose:
            print("  note %s: mentions unreleased spec version(s) %s"
                  % (where, ", ".join(ahead)))
    if verbose:
        print("  ok  %s: describes spec %s" % (where, want))


def check_epochs(text, where, problems, verbose):
    eps = epoch_names()
    if not eps:
        return
    missing = sorted(e for e in eps if not re.search(r"\b%s\b" % e, text))
    if missing:
        problems.append("%s does not name %d datetime epoch(s) the "
                        "implementation accepts: %s"
                        % (where, len(missing), ", ".join(missing)))
    elif verbose:
        print("  ok  %s: all %d epochs named" % (where, len(eps)))


def check_families(text, where, facts, problems, verbose):
    missing = sorted(f for f in facts["families"]
                     if not re.search(r"\b%s\b" % re.escape(f), text))
    if missing:
        problems.append("%s does not name %d type family(ies): %s"
                        % (where, len(missing), ", ".join(missing)))
    elif verbose:
        print("  ok  %s: all %d type families named" % (where, len(facts["families"])))


def main(argv):
    verbose = "--verbose" in argv
    facts = header_facts()
    problems = []

    md = os.path.join(IETF, DRAFT_STEM + ".md")
    if not os.path.exists(md):
        raise SystemExit("check_ietf_draft: %s not found" % md)
    text = open(md, encoding="utf-8").read()
    where = "doc/ietf/%s.md" % DRAFT_STEM
    check_errors(text, where, facts, problems, verbose)
    check_component_limit(text, where, facts, problems, verbose)
    check_exponent_range(text, where, facts, problems, verbose)
    check_base_unit_char(text, where, problems, verbose)
    check_macro_values(text, where, facts, problems, verbose)
    check_spec_version(text, where, facts, problems, verbose)
    check_epochs(text, where, problems, verbose)
    check_families(text, where, facts, problems, verbose)

    # The three generated renderings must state the same facts as the source.
    for ext in ("txt", "xml", "html"):
        p = os.path.join(IETF, "%s.%s" % (DRAFT_STEM, ext))
        if not os.path.exists(p):
            continue
        t = open(p, encoding="utf-8", errors="replace").read()
        w = "doc/ietf/%s.%s" % (DRAFT_STEM, ext)
        check_component_limit(t, w, facts, problems, verbose)
        check_exponent_range(t, w, facts, problems, verbose)
        # The .xml/.html wrap identifiers in markup, so only the component
        # claim and the macro restatements survive a plain-text comparison
        # reliably. That is the claim that drifted; the rest is covered by
        # the .md check above plus the fact that all four are one generation.
        check_macro_values(t, w, facts, problems, verbose)

    # The documentation set restates the same constants.
    for p, w in ((UNIT_DOC, "doc/05_bovnar_unit_system.md"),
                 (SPEC_DOC, "doc/03_bovnar_spec.md")):
        t = open(p, encoding="utf-8").read()
        check_component_limit(t, w, facts, problems, verbose)
        check_exponent_range(t, w, facts, problems, verbose)
        check_macro_values(t, w, facts, problems, verbose)

    # And the documents that are not normative but restate the limits anyway.
    # The tutorial's worked "too many components" example and the bindings
    # reference's account of what from_pint refuses were both wrong for as long
    # as the constants had been growing -- the tutorial showed a nine-component
    # unit annotated as an error, and the bindings doc described the pint
    # bridge's own stale bounds as bovnar's. Neither has to STATE a limit, so
    # the presence requirement is off; what they do state is checked.
    for name in ("01_bovnar_tutorial.md", "09_bovnar_python_bindings.md",
                 "04_bovnar_unit_cheatsheet.md", "06_bovnar_unit_policy.md",
                 "11_bovnar_unit_profiles.md"):
        p = os.path.join(ROOT, "doc", name)
        if not os.path.exists(p):
            continue
        t = open(p, encoding="utf-8").read()
        w = "doc/" + name
        check_component_limit(t, w, facts, problems, verbose, required=False)
        check_exponent_range(t, w, facts, problems, verbose)
        check_macro_values(t, w, facts, problems, verbose)

    if problems:
        return die(problems)
    print("check_ietf_draft: the IETF draft, its three renderings and six "
          "documents agree with include/bovnar.h on %d error codes, the "
          "component limit (%d), the exponent range ([%d, %d]), %d type "
          "families and the spec version."
          % (len(facts["errors"]), facts["BVNR_MAX_UNIT_COMPONENTS"],
             facts["BVN_EXPONENT_MIN"], facts["BVN_EXPONENT_MAX"],
             len(facts["families"])))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
