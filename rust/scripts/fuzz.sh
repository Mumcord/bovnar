#!/bin/sh
# Runs the cargo-fuzz targets in fuzz/ one after the other.
#
# Usage: scripts/fuzz.sh <upstream-source-dir> [seconds-per-job] [jobs] [target...]
#
# Needs a nightly toolchain and cargo-fuzz. Seeds are generated from the
# conformance corpus and the upstream examples into fuzz/seeds/; new corpus
# entries go to fuzz/corpus/<target>, crashes to fuzz/artifacts/<target>.
# Copy a fixed crash into fuzz/regressions/<target>/ so `cargo test`
# replays it.
set -eu

if [ $# -lt 1 ]; then
	echo "usage: $0 <upstream-source-dir> [seconds-per-job] [jobs] [target...]" >&2
	exit 2
fi
upstream=$1
seconds=${2:-60}
jobs=${3:-4}
shift $(($# < 3 ? $# : 3))
targets=${*:-reader dom roundtrip writer units json detect}

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root/fuzz"
python3 "$root/scripts/fuzz-seeds.py" "$upstream" seeds
cargo +nightly fuzz build -O

for target in $targets; do
	mkdir -p "corpus/$target" "artifacts/$target"
	echo "== $target: $jobs jobs x ${seconds}s"
	cargo +nightly fuzz run -O "$target" "corpus/$target" "seeds/$target" -- \
		-dict=bovnar.dict -max_len=4096 -rss_limit_mb=2048 -timeout=5 \
		-max_total_time="$seconds" -jobs="$jobs" -workers="$jobs" \
		-artifact_prefix="artifacts/$target/" -print_final_stats=1 \
		>"artifacts/$target.log" 2>&1 || true
	grep -h -E 'stat::number_of_executed_units|^==.*ERROR|panicked' fuzz-*.log 2>/dev/null |
		awk '/number_of_executed_units/ { n += $2; next } { print }
			END { print "executions:", n + 0 }' | tail -n 5
	rm -f fuzz-*.log
	ls "artifacts/$target" | head -n 5
done
