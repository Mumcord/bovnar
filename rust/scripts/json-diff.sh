#!/bin/sh
# Differential test of bovnar::json against the C CLI's `bovnar convert`.
#
# Usage: scripts/json-diff.sh <upstream-source-dir> <cref-build-dir> [count] [seed] [capture-file]
#
# Builds the C reference (UCUM and CF profiles off, see build-cref.sh) and
# bovnar-json-diff, then converts the upstream JSON fixtures, the fixtures in
# tests/json/inputs, the conformance corpus and the upstream examples, and
# `count` generated and mutated inputs (default 20000) in both directions.
# Exits non-zero on any mismatch. With a capture file, also writes the
# expectations tests/json.rs checks (pass tests/json/expected.json to
# refresh them).
set -eu

if [ $# -lt 2 ]; then
	echo "usage: $0 <upstream-source-dir> <cref-build-dir> [count] [seed] [capture-file]" >&2
	exit 2
fi
upstream=$1
cref=$2
count=${3:-20000}
seed=${4:-1}

root=$(cd "$(dirname "$0")/.." && pwd)
"$root/scripts/build-cref.sh" "$upstream" "$cref" >/dev/null
cargo build --quiet --release --manifest-path "$root/Cargo.toml" -p bovnar-diff
target=$(cargo metadata --format-version 1 --no-deps --manifest-path "$root/Cargo.toml" |
	python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')

set -- --cli "$cref/bovnar" \
	--corpus "$root/tests/conformance/corpus.json" \
	--fixtures "$upstream/tests/json" --fixtures "$root/tests/json/inputs" \
	--examples "$upstream/examples" \
	--count "$count" --seed "$seed" ${5:+--capture "$5"}
"$target/release/bovnar-json-diff" "$@"
