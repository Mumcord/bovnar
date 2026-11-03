# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
The C writer and the Python writer must agree about every unit-carrying shape.

THE GAP THIS CLOSES. `dumps()` emitted a string's unit as an annotation
parameter -- `<utf8:m> "x"` -- which this library's own reader refuses, so
`loads(typed=True)` -> `dumps` -> `loads` failed on a document it had just read.
The C writer emitted `<utf8> "x" m;` and round-tripped throughout. Two writers,
one shape, and only one of them could read its own output back.

Nothing compared them. The Python suite round-trips through `dumps`, the C suite
round-trips through the C writer, and each was self-consistent. What was missing
is the cross-check: hand the same document to both and require the two outputs
to re-read to the same thing.

Skips, rather than passes, when the CLI is not built -- the point is the
comparison, and a green tick without it would say the opposite of the truth.
"""
import io
import os
import re
import subprocess
import tempfile

import bovnar
import pytest

_REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
_CLI = os.path.join(_REPO, "build", "bovnar")

# One per type family that can carry a unit, plus the utf8 inline suffix that
# was the defect and the quoted non-decimal number that shares its shape.
SHAPES = (
    "<float:64,%s> 1.5",
    "<float:32,%s> 1.5",
    "<uint:64,%s> 7",
    "<sint:64,%s> -7",
    "<float_dec:64,%s> 1.5",
    "<float_fix:32,q8,%s> 1.5",
    '<uint:8,_16,%s> "FF"',
    '<utf8> "x" %s',
)


def _catalogue():
    path = os.path.join(_REPO, "src", "gendata", "units.bvnr")
    with io.open(path, encoding="utf-8") as f:
        return re.findall(r'\.symbol\s*=\s*"([^"]+)"', f.read())


@pytest.mark.needs_lib
def test_the_two_writers_produce_documents_that_read_back_alike():
    if not os.path.exists(_CLI):
        pytest.skip("no built CLI to compare against")
    try:
        symbols = _catalogue()
    except OSError:
        pytest.skip("src/gendata is not present")

    lines = []
    for i, unit in enumerate(symbols):
        for j, shape in enumerate(SHAPES):
            lines.append(".v%d_%d = %s;" % (i, j, shape % unit))

    disagreed, compared = [], 0
    fd, path = tempfile.mkstemp(suffix=".bvnr")
    os.close(fd)
    try:
        for start in range(0, len(lines), 200):
            body = "\n".join(lines[start:start + 200]) + "\n"
            with io.open(path, "w", encoding="utf-8") as f:
                f.write(body)
            run = subprocess.run([_CLI, "pretty-print", path],
                                 capture_output=True, text=True, timeout=60)
            assert run.returncode == 0, run.stderr[:200]
            doc = bovnar.loads(body.encode(), typed=True)
            from_c = bovnar.loads(run.stdout.encode(), typed=True)
            from_py = bovnar.loads(bovnar.dumps(doc), typed=True)
            compared += len(lines[start:start + 200])
            if from_c != from_py:
                differing = [k for k in from_c
                             if from_c.get(k) != from_py.get(k)]
                disagreed.extend(differing[:5])
    finally:
        os.unlink(path)

    assert compared > 1000, "the sweep stopped comparing anything"
    assert not disagreed, ("the C and Python writers disagree on %d shape(s): %r"
                           % (len(disagreed), disagreed[:10]))


@pytest.mark.needs_lib
def test_the_shape_that_broke_is_in_the_sweep():
    """Without this the sweep above could pass by testing nothing that matters."""
    doc = bovnar.loads(b'.s = "x" m;', typed=True)
    assert doc["s"].unit_str == "m"
    assert bovnar.dumps(doc).decode().strip() == '.s = <utf8> "x" m;'
