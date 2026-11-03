#!/bin/sh
# Differential oracle for the native unit system.
#
# Builds a tiny driver against the C reference library and feeds it the lines
# of stdin. Each line is one query, tab-separated:
#
#   u <unit>         -> u <unit> <code> <canonical>
#   p <unit> <unit>  -> p <a> <b> <equal 0|1> <convertible 0|1>
#   w <ns> <unit>    -> w <ns> <unit> <code>
#
# <code> of a "w" query is bvn_unit_to_profile(ns, unit), or "-" when it
# returns -1 (or the unit does not parse).
# <code> is "ok" or the error name bvn_error_to_string prints. <canonical> is
# bvn_unit_to_string of the parsed unit ("-" when the parse failed).
#
# Usage: scripts/unit-oracle.sh <upstream-src> <c-reference-build> < queries
# The C reference must be built with the UCUM and CF profiles switched off.
#
#        scripts/unit-oracle.sh --profile-queries <upstream-src> [seed] [--sweep|--full]
# prints queries for the shipped unit profiles instead: every code of their
# tables (read from the upstream data files; ucum.bvnr and cf.bvnr are never
# opened), exponent and operator forms, seeded random mutations, prefixed
# native units and write queries. The default size is the checked-in
# src/units/testdata/profile-oracle.tsv (seed 1); --sweep is about 650k
# queries and --full (every UDUNITS prefix on every code, every native unit
# under every prefix, a write per namespace for everything) about 1.8M.
set -eu
if [ "${1:-}" = "--profile-queries" ]; then
	upstream=${2:?upstream source directory}
	exec python3 -I - "$upstream" "${3:-1}" "${4:-}" <<'PY'
import os, random, sys
upstream, seed, mode = sys.argv[1], int(sys.argv[2]), sys.argv[3]
full = mode == "--full"
small = mode not in ("--sweep", "--full")
sys.dont_write_bytecode = True
sys.path.insert(0, upstream)
import gen_profiles as gp

SHIPPED = ("unece", "qudt", "qudt-qk", "udunits", "om")
WRITE_NS = SHIPPED + ("ucum", "cf", "nope")
rng = random.Random(seed)
out = []
seen = set()

def u(s):
    if "\t" in s or "\n" in s or s in seen:
        return
    seen.add(s)
    out.append("u\t" + s)

codes = {}
for p in gp.PROFILES:
    if p.ns not in SHIPPED:
        continue
    p.doc = gp.load(os.path.join(upstream, "src", "gendata", p.data))
    cs = [r["code"] for k in ("mapped", "opaque", "unsupported") for r in p.list(k)]
    codes[p.ns] = (cs, [r["code"] for r in p.list("prefixes")])

alphabet = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-^.*/() []{}'+:~$µ°"
def mutate(c):
    c = list(c)
    for _ in range(rng.randint(1, 3)):
        op = rng.randrange(3)
        i = rng.randrange(len(c) + 1)
        if op == 0 or not c:
            c.insert(i, rng.choice(alphabet))
        elif op == 1:
            del c[min(i, len(c) - 1)]
        else:
            c[min(i, len(c) - 1)] = rng.choice(alphabet)
    return "".join(c)

for ns, (cs, pfx) in codes.items():
    for c in cs:
        u(ns + ":" + c)
        u(ns + ":" + mutate(c))
        if not small:
            u(ns + ":" + c.lower())
            u(ns + ":" + c.upper())
    for c in rng.sample(cs, min(len(cs), 12 if small else 300)):
        for form in ("%s2", "%s-1", "%s^2", "%s^-3", "%s0", "%s^0", "%s101",
                     "%s^100", "%s^-100", "%s1000", "/%s", "(%s)", "%s.%s",
                     "%s/%s", "%s*%s", "%s %s", "%s  %s", "10^3.%s", "10^-4.%s",
                     "1000.%s", "%s/10^3", "(%s/%s)2", "%ssince", "%s.", "1/%s"):
            u(ns + ":" + form.replace("%s", c))
        u(ns + ":" + c + "." + rng.choice(cs))
        u(ns + ":" + rng.choice(cs) + "/" + c + "^2")
    for f in pfx:
        for c in (cs if full else rng.sample(cs, min(len(cs), 3 if small else 40))):
            u(ns + ":" + f + c)
            u(ns + ":" + f + c + "2")
            u(ns + ":" + c + "/" + f + rng.choice(cs))
for ns in ("ucum", "cf", "nope", "unece", "x-y", "1a"):
    for c in ("m", "KGM", "", "kg", ":"):
        u(ns + ":" + c)
for s in ("udunits:" + "(" * 16 + "m" + ")" * 16, "udunits:" + "(" * 17 + "m" + ")" * 17,
          "udunits:m999999999999999999", "udunits:10^401.m", "udunits:10^400.m",
          "udunits:" + ".".join(["m"] * 33), "udunits:" + ".".join(["m"] * 32),
          "udunits:(" + ".".join(["m"] * 20) + ").(" + ".".join(["s"] * 13) + ")",
          "udunits:1", "udunits:10", "udunits:10^", "udunits:[m", "udunits:m]",
          "unece:", "udunits:", ":m", "-x:m", "A:m", "m[s]", "udunits:m s-1",
          "udunits:kg m-2 s-1", "udunits:ms-1"):
    u(s)
# Native units, prefixed, so the writer's read-back check meets the traps
# ("k~t" would be written as the knot).
units, si, _allpfx, iec = gp.native_registry()
names = sorted(set(units))
for name in (names if full else rng.sample(names, 60 if small else 120)):
    u(name)
    for f in (list(si) + list(iec)) if full else rng.sample(list(si) + list(iec), 3 if small else 6):
        u(f + "~" + name)
for s in ("k~t", "p~H", "f~t", "p~t", "n~t", "a~t", "Mi~B", "Ki~bit", "k~g", "m~g/d~L"):
    u(s)
reads = list(out)
for line in reads:
    unit = line[2:]
    if rng.randrange(8) == 0:
        other = rng.choice(reads)[2:]
        out.append("p\t%s\t%s" % (unit, other))
    if small and rng.randrange(10) != 0:
        continue
    for ns in WRITE_NS:
        out.append("w\t%s\t%s" % (ns, unit))
print("\n".join(out))
PY
fi
upstream=${1:?upstream source directory}
cref=${2:?C reference build directory}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cat > "$work/oracle.c" <<'C'
#include <stdio.h>
#include <string.h>
#include "bovnar.h"
#include "bovnar_si_units.h"

static value_unit_t parse(const char* s, bool* ok)
{
	return bvn_parse_unit_n((const uint8_t*)s, (uint32_t)strlen(s), ok);
}

int main(void)
{
	static char line[4096];
	while (fgets(line, sizeof line, stdin)) {
		line[strcspn(line, "\n")] = '\0';
		char* a = strchr(line, '\t');
		if (!a)
			continue;
		*a++ = '\0';
		if (line[0] == 'u') {
			bool ok = false;
			value_unit_t u = parse(a, &ok);
			char buf[BVNR_UNIT_STRING_MAX];
			const char* code = "ok";
			if (!ok)
				code = bvn_error_to_string(bvn_unit_error_code(
					(const uint8_t*)a, (uint32_t)strlen(a)));
			if (!ok || bvn_unit_to_string(u, buf, sizeof buf) < 0)
				strcpy(buf, "-");
			printf("u\t%s\t%s\t%s\n", a, code, buf);
		} else if (line[0] == 'p') {
			char* b = strchr(a, '\t');
			if (!b)
				continue;
			*b++ = '\0';
			bool oka = false, okb = false;
			value_unit_t ua = parse(a, &oka), ub = parse(b, &okb);
			if (!oka || !okb)
				continue;
			printf("p\t%s\t%s\t%d\t%d\n", a, b, bvn_unit_equal(ua, ub),
			       bvn_units_convertible(ua, ub));
		} else if (line[0] == 'w') {
			char* b = strchr(a, '\t');
			if (!b)
				continue;
			*b++ = '\0';
			bool ok = false;
			value_unit_t u = parse(b, &ok);
			char buf[BVNR_UNIT_STRING_MAX];
			if (!ok || bvn_unit_to_profile(a, u, buf, sizeof buf) < 0)
				strcpy(buf, "-");
			printf("w\t%s\t%s\t%s\n", a, b, buf);
		}
	}
	return 0;
}
C
cc -O1 -I"$upstream/include" -I"$cref/generated" -o "$work/oracle" \
	"$work/oracle.c" "$cref/libbvnr.a" -lm
"$work/oracle"
