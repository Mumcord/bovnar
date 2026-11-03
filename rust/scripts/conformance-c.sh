#!/bin/sh
# Runs the upstream conformance suite (bvnr_conformance --iut) against the
# bovnar-rs IUT and checks the TAP output.
#
# Every case must pass except the ones marked `profile_off_deviation` in
# tests/conformance/corpus.json: those exercise the UCUM and CF unit profiles,
# which bovnar-rs does not ship, so they fail exactly like an upstream build
# with both profiles switched off.
#
# Usage: scripts/conformance-c.sh <cref-build-dir> [iut-binary]
# <cref-build-dir> is the output of scripts/build-cref.sh. Without an explicit
# IUT binary, the release build of bovnar-iut is built and used.
set -eu

if [ $# -lt 1 ]; then
	echo "usage: $0 <cref-build-dir> [iut-binary]" >&2
	exit 2
fi
cref=$1
root=$(cd "$(dirname "$0")/.." && pwd)

if [ $# -ge 2 ]; then
	iut=$2
else
	cargo build --quiet --release --manifest-path "$root/Cargo.toml" -p bovnar-iut
	target=$(cargo metadata --format-version 1 --no-deps --manifest-path "$root/Cargo.toml" |
		python3 -c 'import json, sys; print(json.load(sys.stdin)["target_directory"])')
	iut="$target/release/bovnar-iut"
fi

tap=$(mktemp)
trap 'rm -f "$tap"' EXIT

# The runner exits non-zero whenever any case fails, including the expected
# deviations, so its status is judged from the TAP output below instead.
"$cref/tests/bvnr_conformance" --iut "$iut" >"$tap" || true

python3 -I - "$tap" "$root/tests/conformance/corpus.json" <<'PY'
import json
import re
import sys

tap_path, corpus_path = sys.argv[1], sys.argv[2]
with open(corpus_path, encoding="utf-8") as f:
    cases = json.load(f)["cases"]
all_ids = {c["id"] for c in cases}
allowed = {c["id"] for c in cases if c.get("profile_off_deviation")}

# Individual cases are indented subtest points: "    ok N - [ID] ..." or
# "    not ok N - [ID] ...". Group roll-ups are not indented and are ignored.
point = re.compile(r"^    (not ok|ok) \d+ - \[([^\]]+)\]")
passed, failed = set(), set()
with open(tap_path, encoding="utf-8", errors="replace") as f:
    for line in f:
        m = point.match(line)
        if m:
            (failed if m.group(1) == "not ok" else passed).add(m.group(2))

seen = passed | failed
missing = sorted(all_ids - seen)
unexpected = sorted(failed - allowed)
print(f"conformance: {len(seen)} cases, {len(passed)} ok, {len(failed)} failed "
      f"({len(failed & allowed)} documented UCUM/CF deviations)")
ok = True
if missing:
    print(f"error: {len(missing)} corpus cases missing from the TAP output: "
          + ", ".join(missing[:20]))
    ok = False
if unexpected:
    print(f"error: {len(unexpected)} unexpected failures: " + ", ".join(unexpected))
    ok = False
sys.exit(0 if ok else 1)
PY
