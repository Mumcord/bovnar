#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
Generate a single-file amalgamation of the Bovnar library into build/amalgamate/:

    build/amalgamate/bovnar.h  -- the complete public API (all include/*.h)
    build/amalgamate/bovnar.c  -- the complete implementation (all src/**/*.{c,h}
                                  except the CLI src/bovnar.c), #include "bovnar.h"

A consumer then needs only those two files:

    cc -c bovnar.c -o bovnar.o          # build the library object
    #include "bovnar.h"                  # use the API

Project `#include "..."` directives are resolved and each project header is
inlined exactly once; system `#include <...>` directives are hoisted and
deduplicated. The build under build/ remains the authoritative one; this is a
convenience distribution form. Run tests/amalgam_smoke.c (CTest target
`bvnr_amalgam`) to verify the generated files compile and work.
"""
import os, re, sys

ROOT = os.path.dirname(os.path.abspath(__file__))

# Directories searched to resolve a project `#include "name"`. The generated
# *.gen.inc snippets are build artifacts (not committed); they live in
# BVNR_GENERATED_DIR (passed by the build, default build/generated), which is
# searched first so the amalgamation can inline them.
SEARCH_DIRS = [
    os.environ.get("BVNR_GENERATED_DIR", os.path.join(ROOT, "build", "generated")),
    "include",
    "src", "src/dom", "src/io", "src/lexer",
    "src/stream", "src/utils", "src/validator", "src/writer",
]

# The generated directory, resolved once, as the banner normalisation in
# expand() identifies a build artifact by comparing against it.
GEN_DIR = os.path.abspath(SEARCH_DIRS[0])

# Public headers, in dependency order (recursion handles transitive includes).
PUBLIC_HEADERS = [
    "bvn_int.h", "bvn_float.h", "bovnar.h",
    "bovnar_si_units.h", "bovnar_currency.h", "bovnar_dom.h",
    "bovnar_stream.h",
    "bvn_gregorian_date.h", "bvn_datetime.h",
]

# Implementation translation units (the CLI src/bovnar.c is intentionally
# excluded: it carries main() and is an application, not part of libbovnar).
IMPL_SOURCES = [
    "src/utils/bvn_int.c",
    "src/utils/bvn_float.c",
    "src/utils/bvn_gregorian_date.c",
    "src/utils/bvn_datetime.c",
    "src/utils/bovnar_si_units.c",
    "src/utils/bovnar_currency.c",
    "src/utils/bovnar_profiles.c",
    "src/utils/bovnar_utils.c",
    "src/io/bovnar_io.c",
    "src/lexer/bovnar_state_table.c",
    "src/lexer/bovnar_lexer.c",
    "src/validator/bovnar_validator.c",
    "src/writer/bovnar_writer.c",
    "src/writer/bovnar_write_utils.c",
    "src/writer/bovnar_canon_observer.c",
    "src/dom/bovnar_dom.c",
    "src/dom/bovnar_dom_builder.c",
    "src/stream/bovnar_stream.c",
]

# Capture any trailing content after the directive (e.g. a comment that opens a
# multi-line block) so dropping the include never orphans a "/*".
INC_SYS = re.compile(r'^\s*#\s*include\s*<([^>]+)>(.*)$')
INC_PRJ = re.compile(r'^\s*#\s*include\s*"([^"]+)"(.*)$')

# Preprocessor conditional tracking. A system include sitting inside a
# platform-conditional block (e.g. `#if defined(_WIN32)` in bvn_port.h) must NOT
# be hoisted to the top of the amalgamation — doing so strips its guard and the
# header (<io.h>, <windows.h>, …) then gets included on the wrong platform. Such
# includes are kept inline so their #if/#else/#endif structure is preserved;
# only unconditional and plain include-guard'd system includes are hoisted.
COND_OPEN  = re.compile(r'^\s*#\s*(?:if|ifdef|ifndef)\b(.*)$')
COND_CLOSE = re.compile(r'^\s*#\s*endif\b')
PLATFORM_MACRO = re.compile(r'_WIN32|_WIN64|__CYGWIN__|_MSC_VER|__MINGW|\bWIN32\b')


def resolve(name):
    base = os.path.basename(name)
    for d in SEARCH_DIRS:
        p = os.path.join(ROOT, d, base)
        if os.path.isfile(p):
            return p
    return None


def expand(path, inlined, sys_inc, out, in_platform_cond=False):
    """Append `path`'s content to out, inlining project headers once and
    collecting unconditional system includes into sys_inc (ordered, deduped).
    System includes guarded by a platform conditional are kept inline so their
    guard survives. `in_platform_cond` carries that context across inlining when
    a header is itself included from inside a platform conditional."""
    # Path RELATIVE TO THE ROOT leaks the build directory for generated
    # fragments (they live under <build>/generated), which made the amalgamation
    # non-reproducible: two runs over identical sources differed in exactly these
    # banner comments, so any hash of bovnar.c depended on where it was built.
    # That also leaks absolute paths -- usernames, temp dirs -- into a shipped
    # release artifact. A fragment is identified by its own name and the
    # directory it sits in, both of which are stable.
    #
    # Recognised by WHERE THE FILE LIVES, not by what the build directory is
    # called. Matching the literal name "build" covered only two of the three
    # cases -- a build tree outside the source tree, and one named exactly
    # "build" -- and left an in-tree directory under any OTHER name falling
    # through with its name intact. `cmake -B build-asan` is precisely that, so
    # the sanitizer build produced a different amalgamation from the ordinary one
    # over byte-identical sources, and bvnr_wasm_freshness (which hashes it)
    # failed there while passing in build/. Comparing against the generated
    # directory itself has no such blind spot: every build tree, wherever it is
    # and whatever it is called, yields "generated/<fragment>".
    apath = os.path.abspath(path)
    rel = os.path.relpath(apath, ROOT)
    if (os.path.dirname(apath) == GEN_DIR
            or os.path.isabs(rel) or rel.startswith(os.pardir + os.sep)):
        rel = os.path.join(os.path.basename(os.path.dirname(apath)),
                           os.path.basename(apath))
    out.append(f"\n/* ==================== {rel} ==================== */\n")
    # Stack of bools: does this open #if level reference a platform macro?
    cond_platform = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            co = COND_OPEN.match(line)
            if co:
                cond_platform.append(bool(PLATFORM_MACRO.search(co.group(1))))
                out.append(line)
                continue
            if COND_CLOSE.match(line):
                if cond_platform:
                    cond_platform.pop()
                out.append(line)
                continue
            ms, mp = INC_SYS.match(line), INC_PRJ.match(line)
            if ms or mp:
                m = ms or mp
                name, trailing = m.group(1), m.group(2)
                platform_ctx = in_platform_cond or any(cond_platform)
                # An include that resolves to a project file (quote form, or the
                # <...> form bvn_float.c uses for <bvn_float.h>) is inlined once;
                # an unconditional <system> header is hoisted; a platform-guarded
                # <system> header is kept inline (its #if guard must survive); an
                # unresolved quote include is dropped. Any trailing content (e.g.
                # a comment opening a multi-line block) is always preserved.
                tgt = resolve(name)
                if tgt:
                    if tgt not in inlined:
                        inlined.add(tgt)
                        expand(tgt, inlined, sys_inc, out, platform_ctx)
                elif ms:
                    if platform_ctx:
                        out.append(line)
                    elif name not in sys_inc:
                        sys_inc.append(name)
                if trailing.strip():
                    out.append(trailing + "\n")
                continue
            out.append(line)


def emit_sys_block(sys_inc):
    return "".join(f"#include <{h}>\n" for h in sys_inc)


def build_header():
    inlined, sys_inc, body = set(), [], []
    for h in PUBLIC_HEADERS:
        p = resolve(h)
        if p and p not in inlined:
            inlined.add(p)
            expand(p, inlined, sys_inc, body)
    text = (
        "/* Bovnar — single-file public API (generated by amalgamate.py;\n"
        " * do not edit. Source of truth: include/ and src/). */\n"
        "#ifndef BOVNAR_AMALGAMATION_H\n#define BOVNAR_AMALGAMATION_H\n\n"
        + emit_sys_block(sys_inc) + "\n"
        + "".join(body)
        + "\n#endif /* BOVNAR_AMALGAMATION_H */\n"
    )
    return text, inlined


def build_impl(header_inlined):
    # The public headers already live in bovnar.h; do not re-inline them.
    inlined = set(header_inlined)
    sys_inc, body = [], []
    for c in IMPL_SOURCES:
        p = os.path.join(ROOT, c)
        if not os.path.isfile(p):
            sys.exit(f"missing source: {c}")
        expand(p, inlined, sys_inc, body)
    text = (
        '/* Bovnar — single-file implementation (generated by amalgamate.py;\n'
        ' * do not edit. Source of truth: include/ and src/). */\n'
        '#include "bovnar.h"\n\n'
        + emit_sys_block(sys_inc) + "\n"
        + "".join(body)
    )
    return text


USAGE = (
    "usage: amalgamate.py [OUT_DIR]\n"
    "\n"
    "Generate the single-file amalgamation (bovnar.h + bovnar.c).\n"
    "OUT_DIR defaults to the repository's build/amalgamate/ directory.\n"
)


def main():
    args = sys.argv[1:]
    if args and args[0] in ("-h", "--help"):
        sys.stdout.write(USAGE)
        return
    if len(args) > 1:
        sys.stderr.write("error: too many arguments\n\n" + USAGE)
        sys.exit(2)
    out_dir = args[0] if args else os.path.join(ROOT, "build", "amalgamate")
    os.makedirs(out_dir, exist_ok=True)
    hdr, header_inlined = build_header()
    impl = build_impl(header_inlined)
    with open(os.path.join(out_dir, "bovnar.h"), "w", encoding="utf-8") as f:
        f.write(hdr)
    with open(os.path.join(out_dir, "bovnar.c"), "w", encoding="utf-8") as f:
        f.write(impl)
    print("wrote %s/bovnar.h (%d bytes) and %s/bovnar.c (%d bytes)"
          % (out_dir, len(hdr), out_dir, len(impl)))


if __name__ == "__main__":
    main()
