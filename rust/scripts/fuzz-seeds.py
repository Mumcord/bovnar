#!/usr/bin/env python3
"""Writes seed corpora for the cargo-fuzz targets in fuzz/.

Usage: scripts/fuzz-seeds.py <upstream-source-dir> <out-dir>

Seeds come from tests/conformance/corpus.json and the upstream
examples/*.bvnr files. Each target gets its own directory under <out-dir>,
with the control bytes its input layout expects (see fuzz/src/checks.rs).
The seeds are generated, not committed.
"""

import base64
import hashlib
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MAX_LEN = 4096

# Event encodings for the writer target (see `next_event` in checks.rs):
# 0 stream start, 1 assignment + key, 3 data + kind byte + text, 6/7 struct,
# 8/9 row, 10 dim, 11..13 octets. Pool indices pick ready-made tokens.
WRITER_SEEDS = [
    bytes([0, 0, 1, 0, 3, 0, 0]),
    bytes([1, 0, 1, 1, 3, 0, 3, 3, 1, 18]),
    bytes([0, 0, 1, 0, 6, 1, 1, 3, 5, 13, 7]),
    bytes([0, 0, 1, 0, 8, 3, 0, 1, 3, 0, 2, 9, 10, 8, 3, 0, 3, 9]),
    bytes([0, 0, 1, 0, 11, 12, 3, 97, 98, 99, 13]),
    bytes([0, 0, 1, 0, 2, 0x82, 0, 0xC0, 0x80, 0, 0, 3, 0, 1]),
    bytes([0, 0, 1, 0, 3, 0x40, 3, 0]),
]


def write(dir_: pathlib.Path, data: bytes) -> None:
    if len(data) > MAX_LEN:
        return
    dir_.mkdir(parents=True, exist_ok=True)
    (dir_ / hashlib.sha1(data).hexdigest()).write_bytes(data)


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    upstream, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    docs = []
    corpus = json.loads((ROOT / "tests/conformance/corpus.json").read_text())
    for case in corpus["cases"]:
        docs.append(base64.b64decode(case["input"]))
    for path in sorted((upstream / "examples").glob("*.bvnr")):
        docs.append(path.read_bytes())

    units = set()
    for doc in docs:
        write(out / "reader", bytes(11) + doc)
        write(out / "dom", bytes(10) + doc)
        write(out / "roundtrip", doc)
        text = doc.decode("utf-8", "replace")
        for ann in re.findall(r"<([^<>\n]*)>", text):
            for param in ann.split(",")[1:]:
                units.add(param.strip())
        for unit in re.findall(r"\d ([A-Za-z~%µ°Δ][^\s;,\]]*)", text):
            units.add(unit)
    for unit in sorted(units):
        for version in (0, 2):
            write(out / "units", bytes([version]) + unit.encode())
    jsons = []
    for d in (ROOT / "tests/json/inputs", upstream / "tests/json"):
        jsons += [p.read_bytes() for p in sorted(d.glob("*.json"))]
    expected = json.loads((ROOT / "tests/json/expected.json").read_text())
    for case in expected["cases"]:
        if "input" in case:
            jsons.append(base64.b64decode(case["input"]))
    for data in jsons:
        write(out / "json", data)
        write(out / "detect", data)
    for doc in docs:
        write(out / "json", doc)
        write(out / "detect", doc)
    for seed in WRITER_SEEDS:
        for control in (0, 1, 13):
            write(out / "writer", bytes([control]) + seed)
    return 0


if __name__ == "__main__":
    sys.exit(main())
