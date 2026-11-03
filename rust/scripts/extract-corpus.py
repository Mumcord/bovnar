#!/usr/bin/env python3
"""Extract the Bovnar conformance corpus into tests/conformance/corpus.json.

The 470-odd cases live in a C array (g_cases in upstream
tests/bvnr_conformance.c) built from macros, adjacent string literals and
binary byte arrays. Re-parsing C literals in Python would be fragile, so this
script lets the C compiler do it: it compiles a tiny shim that #includes the
upstream file and dumps every case, then runs the C reference IUT adapter on
each streaming case to capture its exact stdout and exit code.

The C reference must be built with scripts/build-cref.sh (UCUM and CF
profiles off). Cases whose reference result differs from the upstream corpus
expectation only because those profiles are off are flagged with
"profile_off_deviation": true.

Usage (run isolated, from anywhere):
    python3 -I scripts/extract-corpus.py <upstream-dir> <cref-build-dir> <out.json>
"""

import base64
import json
import os
import shutil
import subprocess
import sys
import tempfile

SHIM = r"""
#define main bvnr_conformance_upstream_main
#include "tests/bvnr_conformance.c"
#undef main

static void hex(const uint8_t *p, size_t n)
{
	if (!p) { fputs("-", stdout); return; }
	fputs("=", stdout);
	for (size_t i = 0; i < n; i++) printf("%02x", p[i]);
}

static void hexs(const char *s)
{
	hex((const uint8_t *)s, s ? strlen(s) : 0);
}

int main(void)
{
	size_t n = sizeof(g_cases) / sizeof(g_cases[0]);
	for (size_t i = 0; i < n; i++) {
		const cf_case_t *c = &g_cases[i];
		hexs(c->id); putchar('\t');
		hexs(c->group); putchar('\t');
		hexs(c->description); putchar('\t');
		if (c->input_bin) hex(c->input_bin, c->input_bin_len);
		else hexs(c->input);
		printf("\t%s\t%s\t%d\t%u\t%u\t%u\t%u\t%u\t%u\t%u\t%llu\t%d\t",
		       c->expect == CF_VALID ? "valid" : "error",
		       bvn_error_to_string(c->expected_error),
		       c->continue_on_error ? 1 : 0,
		       (unsigned)c->max_id_len, (unsigned)c->max_str_len,
		       (unsigned)c->max_num_len, (unsigned)c->max_sym_len,
		       (unsigned)c->max_ref_len,
		       (unsigned)c->max_struct_nesting,
		       (unsigned)c->max_array_nesting,
		       (unsigned long long)c->max_array_items,
		       c->dom_validate ? 1 : 0);
		hexs(c->expect_key); putchar('\t');
		hexs(c->expect_data); putchar('\t');
		hexs(c->iut_skip_reason); putchar('\n');
	}
	return 0;
}
"""

# Field name in the corpus -> IUT command-line option (doc/13 section 6.4).
LIMIT_OPTIONS = [
    ("max_id", "--max-id"),
    ("max_str", "--max-str"),
    ("max_num", "--max-num"),
    ("max_sym", "--max-sym"),
    ("max_ref", "--max-ref"),
    ("max_snest", "--max-snest"),
    ("max_anest", "--max-anest"),
    ("max_aitems", "--max-aitems"),
]


def unhex(field):
    if field == "-":
        return None
    return bytes.fromhex(field[1:])


def text(field):
    raw = unhex(field)
    return None if raw is None else raw.decode("utf-8")


def dump_cases(upstream, cref):
    cc = os.environ.get("CC") or shutil.which("cc") or "cc"
    with tempfile.TemporaryDirectory() as tmp:
        shim = os.path.join(tmp, "dump_corpus.c")
        exe = os.path.join(tmp, "dump_corpus")
        with open(shim, "w", encoding="utf-8") as f:
            f.write(SHIM)
        # The static library is built with LTO, so the shim must be too.
        subprocess.run(
            [cc, "-std=c99", "-O1", "-flto=auto", "-w",
             "-I", upstream, "-I", os.path.join(upstream, "include"),
             shim, os.path.join(cref, "libbvnr.a"), "-o", exe],
            check=True,
        )
        out = subprocess.run([exe], check=True, capture_output=True).stdout
    cases = []
    for line in out.decode("ascii").splitlines():
        f = line.split("\t")
        case = {
            "id": text(f[0]),
            "group": text(f[1]),
            "description": text(f[2]),
            "input": base64.b64encode(unhex(f[3])).decode("ascii"),
            "expect": f[4],
            "expected_error": None if f[5] == "none" else f[5],
            "continue_on_error": f[6] == "1",
        }
        limits = {}
        for (name, _), value in zip(LIMIT_OPTIONS, f[7:15]):
            if int(value):
                limits[name] = int(value)
        case["limits"] = limits
        case["dom_validate"] = f[15] == "1"
        case["expect_key"] = text(f[16])
        case["expect_data"] = text(f[17])
        case["iut_skip_reason"] = text(f[18])
        cases.append(case)
    return cases


def run_iut(iut, case):
    args = [iut]
    for name, option in LIMIT_OPTIONS:
        if name in case["limits"]:
            args += [option, str(case["limits"][name])]
    proc = subprocess.run(args, input=base64.b64decode(case["input"]),
                          capture_output=True, timeout=60)
    return proc.stdout, proc.returncode


def expectation_met(case, stdout, code):
    if case["expect"] == "valid":
        return code == 0
    if code == 0:
        return False
    if case["expected_error"] is None:
        return True
    return stdout.startswith(f"ERROR {case['expected_error']}\n".encode())


def main():
    if len(sys.argv) != 4:
        sys.exit(f"usage: {sys.argv[0]} <upstream-dir> <cref-build-dir> <out.json>")
    upstream, cref, out_path = (os.path.abspath(a) for a in sys.argv[1:])
    iut = os.path.join(cref, "tests", "bvnr_conformance_iut")

    cases = dump_cases(upstream, cref)
    deviations = []
    for case in cases:
        case["profile_off_deviation"] = False
        if case["dom_validate"] or case["iut_skip_reason"]:
            continue
        stdout, code = run_iut(iut, case)
        case["reference"] = {
            "stdout": base64.b64encode(stdout).decode("ascii"),
            "exit_code": code,
        }
        if not expectation_met(case, stdout, code):
            case["profile_off_deviation"] = True
            deviations.append(case["id"])

    with open(out_path, "w", encoding="utf-8") as f:
        json.dump({
            "source": "https://github.com/sothis/bovnar tests/bvnr_conformance.c",
            "reference_build": "UCUM and CF unit profiles off",
            "cases": cases,
        }, f, indent=1, ensure_ascii=False)
        f.write("\n")

    print(f"{len(cases)} cases, "
          f"{sum(c['dom_validate'] for c in cases)} DOM-tier, "
          f"{len(deviations)} profile-off deviations")
    if deviations:
        print("deviations: " + " ".join(deviations))


if __name__ == "__main__":
    main()
