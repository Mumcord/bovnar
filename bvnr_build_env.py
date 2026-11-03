# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""Where the documentation gates find the build they are checking.

The check_doc_*.py gates put claims in the documents to the reference
implementation: they load libbvnr through the Python package, run the CLI, run
the per-profile test binaries. Each used to look for those under <repo>/build
and, finding nothing, print "no built library; skipped" and exit 0 -- which is
right for somebody running a script by hand in a fresh clone, and wrong under
CTest, where a build directory by any other name (build-asan, out/, a packager's
obj-x86_64) made six registered tests pass without having checked anything, or
check the binaries of a different build.

CTest now says where the build is (LIBBOVNAR_PATH, which the Python package
reads by itself; BOVNAR_CLI; BVNR_TESTS_DIR) and sets BVNR_REQUIRE_BUILD, under
which a missing piece is a failure instead of a skip.
"""

import os
import sys


def cli(repo):
    """Path of the bovnar executable to drive (it may not exist)."""
    return os.environ.get("BOVNAR_CLI") or os.path.join(repo, "build", "bovnar")


def tests_dir(repo):
    """Directory holding the built test executables (it may not exist)."""
    return (os.environ.get("BVNR_TESTS_DIR")
            or os.path.join(repo, "build", "tests"))


def require_build(repo, who, need=("library",)):
    """Under BVNR_REQUIRE_BUILD, exit 1 unless every part in `need` is there.

    `need` names any of "library", "cli", "tests". Without the variable this
    does nothing and the caller's own skip-with-a-message paths apply."""
    if not os.environ.get("BVNR_REQUIRE_BUILD"):
        return
    missing = []
    if "library" in need:
        sys.path.insert(0, os.path.join(repo, "python"))
        # The package tries LIBBOVNAR_PATH first and then goes on looking, so a
        # path that names nothing would be answered by an installed libbvnr or
        # by <repo>/build -- some library, not the one under test.
        pinned = os.environ.get("LIBBOVNAR_PATH")
        if pinned and not os.path.exists(pinned):
            missing.append("LIBBOVNAR_PATH names no file: %s" % pinned)
        try:
            import bovnar
            bovnar._ffi.load_library()
        except Exception as exc:                              # noqa: BLE001
            missing.append("libbvnr is not loadable (LIBBOVNAR_PATH=%s): %s"
                           % (os.environ.get("LIBBOVNAR_PATH", "<unset>"), exc))
    if "cli" in need and not os.path.exists(cli(repo)):
        missing.append("no bovnar executable at %s" % cli(repo))
    if "tests" in need and not os.path.isdir(tests_dir(repo)):
        missing.append("no test executables at %s" % tests_dir(repo))
    if missing:
        for line in missing:
            print("%s: %s" % (who, line), file=sys.stderr)
        print("%s: BVNR_REQUIRE_BUILD is set, so this is a failure and not a "
              "skip" % who, file=sys.stderr)
        sys.exit(1)
