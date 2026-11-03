#!/bin/sh
# Differential test of bovnar::Writer against the C writer (bvnr_write_event).
#
# Usage: scripts/writer-diff.sh <upstream-source-dir> <cref-build-dir> [count] [seed] [capture-file]
#
# Builds the C reference (UCUM and CF profiles off, see build-cref.sh), the
# replay harness diff/c/writer_replay.c against its static library, and
# bovnar-writer-diff, then replays `count` generated event sequences
# (default 50000) through both writers. Exits non-zero on any divergence
# other than the two fixes bovnar-rs keeps (see the tool's docs). With a
# capture file, also writes the fixture tests/writer_error_parity.rs checks
# (pass tests/writer/events.txt to refresh it).
set -eu

if [ $# -lt 2 ]; then
	echo "usage: $0 <upstream-source-dir> <cref-build-dir> [count] [seed] [capture-file]" >&2
	exit 2
fi
upstream=$1
cref=$2
count=${3:-50000}
seed=${4:-1}

root=$(cd "$(dirname "$0")/.." && pwd)
"$root/scripts/build-cref.sh" "$upstream" "$cref" >/dev/null
${CC:-cc} -O1 -std=c11 -D_GNU_SOURCE -Wall -Wextra \
	-I"$upstream/include" -o "$cref/writer_replay" \
	"$root/diff/c/writer_replay.c" "$cref/libbvnr.a" -lm
cargo build --quiet --release --manifest-path "$root/Cargo.toml" -p bovnar-diff
target=$(cargo metadata --format-version 1 --no-deps --manifest-path "$root/Cargo.toml" |
	python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')

"$target/release/bovnar-writer-diff" --c "$cref/writer_replay" \
	--count "$count" --seed "$seed" ${5:+--capture "$5"}
