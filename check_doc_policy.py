#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_doc_policy.py — doc/06's unit-policy claims, put to the library.

THE GAP THIS CLOSES. doc/06 is the unit policy reference and it was the only
unit document with no gate at all. What that cost: doc/06 §2.4 illustrated
`error_unit_inexact` with a document whose heading is in degrees, glossed the
degrees line as "left in degrees" under `--leave-inexact`, and listed "a π-based
angle" first among the factors that take that path. Under `bvnr_normalise_si`
none of that is so — doc/06 §2.3 leaves every dimensionless unit alone, so a degree is
not an inexact conversion but not a conversion at all, and the strict run in
that very example fails on the `k~m/h` two lines below. A reader debugging a
`--si` run would have looked at the wrong value.

WHAT IS CHECKED.

  doc/06 §2.3  The units normalise_si is documented to leave alone, actually left
        alone — and the reverse: the characterisation "every dimensionless unit"
        checked against the whole catalogue, so a new dimensionless unit that
        normalisation *does* rewrite is caught.

  doc/06 §2.4  That inexactness is a property of the RESULT, not of the factor: the
        5/18 of km/h→m/s takes 90 to exactly 25 and refuses only 100.

        The two inexact paths kept apart. A dimensionless unit is never a
        normalisation candidate (strict `normalise_si` accepts it); a
        dimensioned irrational one is (strict `normalise_si` refuses it); and a
        target that names the SI form outright refuses either way.

  doc/06 §7.3  Which subcommands take a policy, and how the two that do not say
        so: `pretty-print` rejects each flag by name and exits 2, while
        `convert` accepts and silently ignores every one of them — including
        the ones taking an argument. "Reports the flags as surplus arguments"
        was the old description of a message that reads `unknown option`, and
        a `bovnar convert --si` that appears to work has normalised nothing.

  doc/06 §2.7  The limit table against the headers, including the three prose bounds
        for the key path — depth, total bytes, and one key — which live in
        src/lexer/bvn_val_impl.h rather than the public header and so are the
        easiest numbers in the document to leave behind.

Usage:  python3 check_doc_policy.py [repo-root]
Exit 0 when doc/06 and the library agree, 1 with a list when they do not.
"""
import os
import re
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.abspath(__file__))
DOC = "doc/06_bovnar_unit_policy.md"


def header_defines(repo):
    """`#define NAME value` from the public header and the validator's own."""
    found = {}
    for rel in ("include/bovnar.h", "src/lexer/bvn_val_impl.h"):
        path = os.path.join(repo, rel)
        if not os.path.exists(path):
            continue
        with open(path, encoding="utf-8") as f:
            for m in re.finditer(r"^#define\s+(\w+)\s+\(?\s*(\d+)", f.read(),
                                 re.M):
                found.setdefault(m.group(1), int(m.group(2)))
    return found


def check_limits(text, defines):
    """doc/06 §2.7's table and its three prose bounds."""
    problems = []
    for name, stated in re.findall(r"\|\s*`(BVNR_MAX_UNIT_\w+)`\s*\|\s*(\d+)\s*\|",
                                   text):
        actual = defines.get(name)
        if actual is None:
            problems.append("%s: doc/06 §2.7 names `%s`, which no header defines"
                            % (DOC, name))
        elif actual != int(stated):
            problems.append("%s: doc/06 §2.7 says %s is %s; the header says %d"
                            % (DOC, name, stated, actual))
    prose = (
        (r"bounded depth \((\d+) nested structs\)", "BVN_PATH_MAX_DEPTH",
         "nested-struct depth"),
        (r"bounded length \((\d+) bytes", "BVN_PATH_MAX_BYTES",
         "key-path bytes"),
        (r"each key at most (\d+)\)", "BVN_PATH_MAX_KEY", "one key's bytes"),
    )
    for pattern, name, what in prose:
        m = re.search(pattern, text)
        if not m:
            problems.append("%s: doc/06 §2.7 no longer states the %s bound; it is %s "
                            "(%s)" % (DOC, what, defines.get(name, "?"), name))
            continue
        actual = defines.get(name)
        if actual is not None and actual != int(m.group(1)):
            problems.append("%s: doc/06 §2.7 says the %s bound is %s; %s is %d"
                            % (DOC, what, m.group(1), name, actual))
    return problems


def documented_left_alone(text):
    """The units doc/06 §2.3 says normalisation leaves alone."""
    m = re.search(r"left alone rather than reinterpreted:\s*(.+?)\.\s*$", text,
                  re.M | re.S)
    if not m:
        return None
    return re.findall(r"`([^`]+)`", m.group(1).split("\n\n")[0])


def value_after(bovnar, doc_bytes, policy):
    """(value, unit) a policy delivers, or the error name it raised."""
    try:
        node = bovnar.dom_parse(doc_bytes, policy)["d"]
        return (node.as_float(), node.unit_str or "no_unit")
    except Exception as exc:                                  # noqa: BLE001
        return type(exc).__name__ + ": " + str(exc)


def check_behaviour(repo, text):
    sys.path.insert(0, os.path.join(repo, "python"))
    try:
        import bovnar
        from bovnar import UnitPolicy
        bovnar._ffi.load_library()
    except Exception:                                         # noqa: BLE001
        print("check_doc_policy: no built library; behaviour not checked")
        return [], 0

    problems, checked = [], 0
    strict = UnitPolicy(normalise_si=True)

    # doc/06 §2.3 — every unit the document names is left alone, unconverted.
    named = documented_left_alone(text)
    if named is None:
        problems.append("%s: doc/06 §2.3 no longer lists the units normalisation "
                        "leaves alone; nothing could be checked against it"
                        % DOC)
        named = []
    for unit in named:
        doc_bytes = (".d=<float:64,%s> 7.0;" % unit).encode("utf-8")
        got = value_after(bovnar, doc_bytes, strict)
        checked += 1
        if got != (7.0, unit):
            problems.append("%s: doc/06 §2.3 says normalise_si leaves `%s` alone; "
                            "under it the value becomes %r" % (DOC, unit, got))

    # doc/06 §2.3, the other direction — the CHARACTERISATION, over the catalogue. A
    # dimensionless unit normalisation rewrites would make the list wrong
    # without making any single entry on it wrong.
    for unit in catalogue_symbols(repo):
        try:
            vu = bovnar.parse_unit(unit)
        except Exception:                                     # noqa: BLE001
            continue
        if bovnar.unit_si_normal_form(vu) is not None:
            continue                                # has an SI form; not ours
        doc_bytes = (".d=<float:64,%s> 7.0;" % unit).encode("utf-8")
        got = value_after(bovnar, doc_bytes, strict)
        checked += 1
        if got != (7.0, unit):
            problems.append("%s: doc/06 §2.3 says normalisation leaves every unit "
                            "with no coherent SI form alone; `%s` becomes %r"
                            % (DOC, unit, got))

    # doc/06 §2.4 — the two inexact paths, kept apart.
    # The value matters, not just the unit: the 5/18 of km/h→m/s takes 90 to
    # exactly 25, and only 100 to the non-terminating 250/9.
    cases = (
        ("°", 90.0, strict, True,
         "a dimensionless unit is not a normalisation candidate, so strict "
         "normalise_si accepts it"),
        ("pc", 1.0, strict, False,
         "a DIMENSIONED irrational factor is a candidate, so strict "
         "normalise_si refuses it"),
        ("k~m/h", 100.0, strict, False,
         "a non-terminating rational is a candidate, so strict normalise_si "
         "refuses it"),
        ("°", 90.0, UnitPolicy(targets=["rad"]), False,
         "a target naming the SI form outright refuses the π-based angle"),
        ("°", 90.0, UnitPolicy(targets=["rad"], leave_inexact=True), True,
         "and leaves it untouched under bvnr_inexact_leave"),
    )
    for unit, value, policy, should_pass, why in cases:
        doc_bytes = (".d=<float:64,%s> %r;" % (unit, value)).encode("utf-8")
        got = value_after(bovnar, doc_bytes, policy)
        passed = not isinstance(got, str)
        checked += 1
        if passed != should_pass:
            problems.append("%s: doc/06 §2.4 — %s; `%s` at %r gave %r"
                            % (DOC, why, unit, value, got))
    return problems, checked


def catalogue_symbols(repo):
    path = os.path.join(repo, "src", "gendata", "units.bvnr")
    if not os.path.exists(path):
        return []
    with open(path, encoding="utf-8") as f:
        return re.findall(r'\.symbol\s*=\s*"([^"]+)"', f.read())


def cli_flags(text):
    """The policy flags doc/06 §7.3 tabulates, with any argument they take."""
    flags = []
    for m in re.finditer(r"^\|\s*`(--[a-z-]+)([^`]*)`\s*\|", text, re.M):
        arg = m.group(2).strip()
        flags.append((m.group(1),
                      {"<unit>": "m", "<path>=<unit>": ".d=m", "<N>": "10"}
                      .get(arg)))
    return flags


def check_cli(repo, text):
    """doc/06 §7.3 — which subcommands take a policy, and how the others refuse."""
    import bvnr_build_env
    exe = bvnr_build_env.cli(repo)
    if not os.path.exists(exe):
        print("check_doc_policy: no built CLI; doc/06 §7.3 not checked")
        return [], 0
    flags = cli_flags(text)
    if not flags:
        return ["%s: doc/06 §7.3 no longer tabulates the policy flags" % DOC], 0

    fd, path = tempfile.mkstemp(suffix=".bvnr")
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        f.write(".d = <float:64,k~m> 5.0;\n")
    problems, checked = [], 0
    try:
        for flag, arg in flags:
            argv = [flag] + ([arg] if arg else [])
            # FIRST that the flag is real. Without this the "pretty-print
            # rejects it by name" test passes for a flag that does not exist —
            # pretty-print says `unknown option --nosuch` just as readily — so
            # a typo or an invented row in the table would gate green.
            for sub, expect in (("validate", "accept"),
                                ("events", "accept"),
                                ("query", "accept"),
                                ("pretty-print", "reject"),
                                ("convert", "ignore")):
                cmd = [exe, sub] + argv + ([".d"] if sub == "query" else []) \
                    + [path]
                r = subprocess.run(cmd, capture_output=True, text=True,
                                   timeout=30)
                checked += 1
                err = (r.stderr or "") + (r.stdout or "")
                if expect == "accept":
                    if "unknown option" in err:
                        problems.append(
                            "%s: doc/06 §7.3 tabulates `%s`, but %s does not know "
                            "it: %r" % (DOC, flag, sub, err.strip()[:70]))
                elif expect == "reject":
                    if "unknown option %s" % flag not in err:
                        problems.append(
                            "%s: doc/06 §7.3 says pretty-print rejects `%s` by name; "
                            "it said %r" % (DOC, flag, err.strip()[:70]))
                    elif r.returncode != 2:
                        problems.append(
                            "%s: doc/06 §7.3 says pretty-print exits 2 on `%s`; it "
                            "exited %d" % (DOC, flag, r.returncode))
                else:
                    # convert warns that JSON is lossy for this document; the
                    # claim is only that it never mentions the FLAG.
                    if "unknown option" in err or "unrecognised" in err:
                        problems.append(
                            "%s: doc/06 §7.3 says convert silently ignores `%s`; "
                            "it said %r" % (DOC, flag, err.strip()[:70]))
        # doc/06 §7.3 — a conversion flag on `validate` changes the VERDICT.
        # Easy to read as a no-op, since validate produces no output.
        fd2, inexact = tempfile.mkstemp(suffix=".bvnr")
        with os.fdopen(fd2, "w", encoding="utf-8") as f:
            f.write(".speed = <float:64,k~m/h> 100.0;\n")
        try:
            plain = subprocess.run([exe, "validate", inexact],
                                   capture_output=True, text=True, timeout=30)
            si = subprocess.run([exe, "validate", "--si", inexact],
                                capture_output=True, text=True, timeout=30)
            leave = subprocess.run([exe, "validate", "--si", "--leave-inexact",
                                    inexact], capture_output=True, text=True,
                                   timeout=30)
            checked += 3
            if plain.returncode != 0:
                problems.append("%s: doc/06 §7.3 — bare `validate` should accept "
                                "a k~m/h document; it exited %d"
                                % (DOC, plain.returncode))
            if si.returncode == 0:
                problems.append("%s: doc/06 §7.3 says a conversion flag on "
                                "`validate` changes the verdict; `--si` still "
                                "reported OK" % DOC)
            if leave.returncode != 0:
                problems.append("%s: doc/06 §7.3 says `--leave-inexact` relaxes "
                                "it; the run still failed" % DOC)
        finally:
            os.unlink(inexact)
    finally:
        os.unlink(path)
    return problems, checked


def main(argv):
    repo = os.path.abspath(argv[1]) if len(argv) > 1 else REPO
    sys.path.insert(0, REPO)
    import bvnr_build_env
    bvnr_build_env.require_build(repo, "check_doc_policy", ("library", "cli"))
    path = os.path.join(repo, DOC)
    if not os.path.exists(path):
        print("check_doc_policy: %s is absent" % DOC)
        return 0
    with open(path, encoding="utf-8") as f:
        text = f.read()

    problems = check_limits(text, header_defines(repo))
    behaviour, checked = check_behaviour(repo, text)
    problems += behaviour
    cli, cli_checked = check_cli(repo, text)
    problems += cli
    checked += cli_checked

    if problems:
        sys.stderr.write("check_doc_policy: %s disagrees with the library:\n"
                         % DOC)
        for p in problems:
            sys.stderr.write("    %s\n" % p)
        return 1
    print("check_doc_policy: %s — limits and %d policy behaviour(s) match the "
          "library" % (DOC, checked))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
