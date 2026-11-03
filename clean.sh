#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# DESTRUCTIVE. Resets the working tree to HEAD and removes everything untracked,
# then repacks the git object store and rebuilds the merged bundle.
#
# This is not "clean the build directory". `git reset --hard` discards every
# uncommitted modification and `git clean -xdff` removes every untracked file --
# the second -f reaching into nested git repositories as well. Nothing here is
# recoverable afterwards. It therefore refuses to run without an explicit
# --force, and shows what it is about to remove first.
set -euo pipefail

usage() {
    cat >&2 <<'USAGE'
usage: ./clean.sh [-n|--dry-run] --force

  --force      actually do it (required; there is no undo)
  -n, --dry-run  list what would be discarded and exit

Discards ALL uncommitted changes and ALL untracked files, then runs
git gc/repack/prune and ./merge.sh.
USAGE
}

FORCE=0
DRY=0
for arg in "$@"; do
    case "$arg" in
        --force)        FORCE=1 ;;
        -n|--dry-run)   DRY=1 ;;
        -h|--help)      usage; exit 0 ;;
        *) echo "clean.sh: unknown argument '$arg'" >&2; usage; exit 2 ;;
    esac
done

echo "Modified files that would be DISCARDED:"
git diff --name-only HEAD | sed 's/^/  /' || true
echo "Untracked files that would be REMOVED:"
git clean -xdn | sed 's/^/  /' || true

if [[ $DRY -eq 1 ]]; then
    exit 0
fi
if [[ $FORCE -ne 1 ]]; then
    echo >&2
    echo "clean.sh: refusing to discard the above without --force." >&2
    echo "          Run './clean.sh --dry-run' to review, then add --force." >&2
    exit 2
fi

git reset --hard
git clean -xdff
git gc --aggressive
git repack -Ad
git prune
git gc --aggressive
./merge.sh
