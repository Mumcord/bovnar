#!/bin/sh
# Differential test of bovnar-rs against the C reference.
#
# Usage: scripts/diff.sh <upstream-source-dir> <cref-build-dir> [count] [seed]
#
# Builds the C reference IUT (UCUM and CF profiles off, see build-cref.sh),
# the bovnar-rs IUT and bovnar-diff, then runs `count` inputs (default
# 20000): every conformance corpus case, then generated documents and
# mutated corpus inputs. Exits non-zero on any stdout or exit-code mismatch.
set -eu

if [ $# -lt 2 ]; then
	echo "usage: $0 <upstream-source-dir> <cref-build-dir> [count] [seed]" >&2
	exit 2
fi
upstream=$1
cref=$2
count=${3:-20000}
seed=${4:-1}

root=$(cd "$(dirname "$0")/.." && pwd)
"$root/scripts/build-cref.sh" "$upstream" "$cref" >/dev/null
cargo build --quiet --release --manifest-path "$root/Cargo.toml" -p bovnar-iut -p bovnar-diff
target=$(cargo metadata --format-version 1 --no-deps --manifest-path "$root/Cargo.toml" |
	python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')

"$target/release/bovnar-diff" \
	--c "$cref/tests/bvnr_conformance_iut" \
	--rust "$target/release/bovnar-iut" \
	--corpus "$root/tests/conformance/corpus.json" \
	--examples "$upstream/examples" \
	--count "$count" --seed "$seed"
