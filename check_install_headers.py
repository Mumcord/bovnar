#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_install_headers.py — every header a consumer can reach is installed.

The public headers include each other, and three of them are GENERATED
(bovnar_units.gen.h, bovnar_profiles.gen.h, bovnar_currency.gen.h). The install
list in CMakeLists.txt names each header explicitly, which is the right thing —
a glob would ship whatever happened to be in include/ — but it is also a second
list, maintained by hand, of something the source already states.

It went wrong exactly the way a second list does: bovnar_profiles.gen.h was
added to bovnar.h and not to the install list, so `make install` produced an
include directory whose bovnar.h could not even preprocess. Nothing caught it.
The library built from the source tree, every test ran against the source tree,
and the only way to see the break was to compile something against an install.

So: start at the headers the install list ships, follow every #include "..."
transitively, and require the closure to be a subset of what is installed.

Usage:  python3 check_install_headers.py [repo-root]
Exit 0 when the list is complete, 1 (with the missing names) when it is not.
"""
import os
import re
import sys

INCLUDE_RX = re.compile(r'^\s*#\s*include\s+"([^"]+)"', re.M)
# The install(FILES ...) block that ships the headers: located by the bovnar.h
# entry, which is the one line that block is guaranteed to have.
INSTALL_RX = re.compile(r'install\(FILES\s*(.*?)\)', re.S)


def installed_headers(cmakelists):
    with open(cmakelists, encoding="utf-8") as f:
        text = f.read()
    for body in INSTALL_RX.findall(text):
        names = re.findall(r'include/([\w.]+\.h)\b', body)
        if "bovnar.h" in names:
            return set(names)
    raise SystemExit("check_install_headers: no install(FILES ...) block "
                     "shipping include/bovnar.h — has the block moved?")


def closure(root, seeds):
    """Every header reachable from `seeds` by quoted #include, restricted to
    include/. A quoted include naming a file that is not there is somebody
    else's problem (the .inc snippets live in the build dir) and is skipped —
    this check is about what a CONSUMER needs, and a consumer only ever sees
    include/."""
    seen, queue = set(), list(seeds)
    while queue:
        name = queue.pop()
        if name in seen:
            continue
        path = os.path.join(root, "include", name)
        if not os.path.exists(path):
            continue
        seen.add(name)
        with open(path, encoding="utf-8") as f:
            queue.extend(INCLUDE_RX.findall(f.read()))
    return seen


def main(argv):
    root = argv[1] if len(argv) > 1 else os.path.dirname(os.path.abspath(__file__))
    cmakelists = os.path.join(root, "CMakeLists.txt")
    shipped = installed_headers(cmakelists)
    needed = closure(root, shipped)
    missing = sorted(needed - shipped)
    if missing:
        print("check_install_headers: these headers are reachable from the "
              "installed set but are not installed:", file=sys.stderr)
        for name in missing:
            print("    include/%s" % name, file=sys.stderr)
        print("\nAdd them to the install(FILES ...) block in CMakeLists.txt. "
              "An installed bovnar.h that #includes a header the install did "
              "not ship cannot be compiled against.", file=sys.stderr)
        return 1
    print("check_install_headers: %d headers installed, closure complete"
          % len(shipped))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
