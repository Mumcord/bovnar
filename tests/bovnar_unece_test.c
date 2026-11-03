/*
 * SPDX-License-Identifier: MIT
 * SPDX-FileComment: The test vectors below quote UN/ECE common codes. Those
 * identifiers are the UNECE's and are not covered by the MIT grant
 * below; see THIRD_PARTY_NOTICES.md.
 *
 * Copyright (c) 2026 Janos Sonntag
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

/*
 * The UNECE unit profile (spec 1.2). Specified in
 * doc/11_bovnar_unit_profiles.md; this file is what holds the specification to
 * the implementation.
 *
 * UNECE is the first FLAT profile, so most of what is defended here is the
 * flatness itself: that a Rec 20 code is one whole token, that nothing in this
 * namespace decomposes into a prefix and a remainder, and that the Rec 21
 * package codes are incommensurable counts rather than dimensionless numbers a
 * conversion could quietly relate.
 *
 * What this file does NOT check, and cannot: that "KGM" is really UNECE's
 * kilogram. Nothing in this repository verifies the UNECE side of the table --
 * see the provenance note at the top of src/gendata/unece.bvnr.
 */

#include <math.h>
#include <stdio.h>
#include <string.h>
#include "bovnar.h"
#include "bovnar_si_units.h"
#include "bvn_internal_dims.h"

static int failures = 0;
static int tests    = 0;

#define ASSERT_TRUE(cond, msg) do { \
	tests++; \
	if (!(cond)) { \
		fprintf(stderr, "FAIL line %d: %s\n", __LINE__, (msg)); \
		failures++; \
	} \
} while (0)

#define ASSERT_FALSE(cond, msg)  ASSERT_TRUE(!(cond), msg)

static value_unit_t U(const char* s)
{
	bool ok = true;
	return bvn_parse_unit((const uint8_t*)s, &ok);
}

/* Parse `src` and check it formats back as `want`. The formatter is the honest
 * witness for what the translation produced: it goes through the same
 * value_unit_t every consumer sees, and its output re-parses. */
static void chk_str(const char* src, const char* want)
{
	bool ok = true;
	value_unit_t u = bvn_parse_unit((const uint8_t*)src, &ok);
	tests++;
	if (!ok) {
		fprintf(stderr, "FAIL: %s did not parse (expected %s)\n", src, want);
		failures++;
		return;
	}
	char buf[BVNR_UNIT_STRING_MAX];
	int32_t n = bvn_unit_to_string(u, buf, sizeof(buf));
	if (n < 0 || strcmp(buf, want) != 0) {
		fprintf(stderr, "FAIL: %s -> %s, expected %s\n",
		        src, n < 0 ? "(unwritable)" : buf, want);
		failures++;
	}
}

static void chk_factor(const char* src, double want)
{
	bool ok = true;
	value_unit_t u = bvn_parse_unit((const uint8_t*)src, &ok);
	tests++;
	if (!ok) {
		fprintf(stderr, "FAIL: %s did not parse\n", src);
		failures++;
		return;
	}
	bool aff = false, fok = true;
	double off = 0.0;
	double f = bvn_unit_to_si_factor(u, &aff, &off, &fok);
	if (!fok || fabs(f - want) > fabs(want) * 1e-9) {
		fprintf(stderr, "FAIL: %s factor %.17g, expected %.17g\n",
		        src, f, want);
		failures++;
	}
}

static void chk_error(const char* src, error_code_t want)
{
	bool ok = true;
	(void)bvn_parse_unit((const uint8_t*)src, &ok);
	tests++;
	if (ok) {
		fprintf(stderr, "FAIL: %s parsed, expected a refusal\n", src);
		failures++;
		return;
	}
	error_code_t got = bvn_unit_error_code((const uint8_t*)src,
	                                       (uint32_t)strlen(src));
	if (got != want) {
		fprintf(stderr, "FAIL: %s -> %s, expected %s\n",
		        src, bvn_error_to_string(got), bvn_error_to_string(want));
		failures++;
	}
}

/* ── the three outcomes ─────────────────────────────────────────────────── */

/*
 * Sixteen codes were refused as "no native unit is a decade away from what
 * QUDT's cross-reference makes this worth" -- true when written, and false the
 * moment the registry gained the unit. Each now matches its QUDT claimant
 * EXACTLY rather than within a decade, which is the standard this vocabulary is
 * held to: it is reached through a secondary source, so an exact factor is the
 * evidence and a near miss is not.
 */
static void test_codes_unblocked_by_the_new_units(void)
{
	printf("  unece: refusals the registry outgrew...\n");
	chk_str("unece:208", "miUS²");        /* QUDT MI_US2,   [mi_us]2 */
	chk_str("unece:BUI", "bsh_uk");       /* QUDT BU_UK,    [bu_br]  */
	chk_str("unece:D42", "yr_trop");      /* QUDT YR_TROPICAL, a_t   */
	chk_str("unece:E86", "Ti~b/m³");
	chk_str("unece:E87", "Ti~b/m²");
	chk_str("unece:F80", "hp_W");
	chk_str("unece:J83", "clo");
	chk_str("unece:K42", "hp_B");
	chk_str("unece:L95", "yr_com");
	chk_str("unece:L96", "yr_sid");
	chk_str("unece:M47", "cml");
	chk_str("unece:M85", "AT");
	chk_str("unece:N94", "statC");
	chk_str("unece:P29", "footlambert");
	chk_str("unece:P53", "unit_pole");

	/*
	 * Three neighbours in the same sweep stay refused, and the reason is the
	 * cross-reference rather than the registry. M48's only claimant, QUDT's
	 * MI2, is labelled "Square Mile" with ucumCode [mi_i]2 -- the
	 * INTERNATIONAL mile -- while stating the SURVEY multiplier; MI2 claims
	 * MIK too, so the same contradiction leaves both open; and P30's
	 * claimant LA is labelled "Lambert" while stating 0.3183098862, which is
	 * the APOSTILB. The lambert is 1e4/pi, four decades from that.
	 *
	 * 208 is the one square mile in the set whose claimant is coherent, which
	 * is why it maps and its two look-alikes do not.
	 */
	chk_error("unece:M48", error_unit_profile_unsupported);
	chk_error("unece:MIK", error_unit_profile_unsupported);
	chk_error("unece:P30", error_unit_profile_unsupported);

	/* The 2026-11-03 coverage review, each via the QUDT row that carries the
	 * code. B32 was refused as "kilogram per metre", which is KL; QUDT's
	 * claimant is KiloGM-M2 (kg·m², ucumCode kg.m2), between B31 and B33. */
	chk_str("unece:B32", "k~g·m²");
	chk_factor("unece:B32", 1.0);
	chk_factor("unece:E97", 1e-3);   /* mm/(°C·m) */
	chk_factor("unece:H73", 1e-3);   /* %/daK */
}

static void test_outcomes(void)
{
	printf("  the three outcomes...\n");

	/* Translated onto the native registry, and formatting back natively is the
	 * proof: nothing downstream can tell the document said "KGM". */
	chk_str("unece:KGM", "k~g");
	chk_str("unece:MTR", "m");
	chk_str("unece:LTR", "L");

	/* Profile-only: an opaque count has no native spelling, so it formats back
	 * in profile notation, in the namespace that owns it. */
	chk_str("unece:XBX", "unece:XBX");
	chk_str("unece:C62", "unece:C62");

	/* Refused, split by cause. */
	chk_error("unece:KILOGRAM", error_unit_illegal);
	chk_error("unece:DZN",      error_unit_profile_unsupported);
	chk_error("unece:",         error_unit_illegal);
	chk_error("nosuch:KGM",     error_unit_profile_unknown);
}

/* ── flatness ───────────────────────────────────────────────────────────── */

static void test_flatness(void)
{
	printf("  flatness: a code is one whole token...\n");

	/*
	 * The claim that makes this profile safe. "KGM" is the kilogram; it is NOT
	 * a "k" prefix on a "GM", and there is no rule in this namespace by which it
	 * could become one. If a flat profile ever grew prefix handling, "MTS"
	 * (metre per second) would become a mega-TS and "MTK" (square metre) a
	 * milli-TK -- units UNECE never defined, invented by a parser being clever.
	 */
	chk_str("unece:MTS", "m/s");
	chk_str("unece:MTK", "m²");
	chk_str("unece:KMH", "k~m/h");

	/* No operator is recognised: a flat code cannot be built out of others. */
	chk_error("unece:KGM/MTR", error_unit_illegal);
	chk_error("unece:KGM.MTR", error_unit_illegal);
	chk_error("unece:MTR2",    error_unit_illegal);
	chk_error("unece:kMTR",    error_unit_illegal);

	/* Case is significant and is UNECE's own: the codes are upper case. */
	chk_error("unece:kgm", error_unit_illegal);
	chk_error("unece:Kgm", error_unit_illegal);
}

/* ── the translation is the native unit, exactly ────────────────────────── */

static void test_equivalence(void)
{
	printf("  a translated code IS the native unit...\n");

	ASSERT_TRUE(bvn_unit_equal(U("unece:KGM"), U("k~g")),
	            "unece:KGM should equal k~g");
	ASSERT_TRUE(bvn_unit_equal(U("unece:MTS"), U("m/s")),
	            "unece:MTS should equal m/s");
	ASSERT_TRUE(bvn_unit_equal(U("unece:HAR"), U("ha")),
	            "unece:HAR should equal ha");

	/* And across vocabularies, which is the whole point of a shared unit model:
	 * two different code systems naming one quantity land on one value_unit_t. */
	ASSERT_TRUE(bvn_unit_equal(U("unece:KGM"), U("ucum:kg")),
	            "unece:KGM should equal ucum:kg");
	ASSERT_TRUE(bvn_unit_equal(U("unece:CEL"), U("ucum:Cel")),
	            "unece:CEL should equal ucum:Cel");
	ASSERT_TRUE(bvn_unit_equal(U("unece:LTR"), U("ucum:L")),
	            "unece:LTR should equal ucum:L");

	/* Factors come out of the native registry unchanged. */
	chk_factor("unece:KGM", 1.0);          /* the SI base unit of mass */
	chk_factor("unece:GRM", 1e-3);
	chk_factor("unece:TNE", 1e3);
	chk_factor("unece:KMT", 1e3);
	chk_factor("unece:MTS", 1.0);
}

/* ── opaque counts ──────────────────────────────────────────────────────── */

static void test_opaque_counts(void)
{
	printf("  Rec 21 packages are incommensurable counts...\n");

	value_unit_t box    = U("unece:XBX");
	value_unit_t pallet = U("unece:XPX");
	value_unit_t one    = U("unece:C62");

	ASSERT_TRUE(bvn_unit_is_profile_only(box),
	            "a package code has no native spelling");
	ASSERT_TRUE(bvn_unit_equal(box, U("unece:XBX")),
	            "a box is a box");
	ASSERT_FALSE(bvn_unit_equal(box, pallet),
	             "a box is not a pallet");

	/*
	 * Incommensurable, through the mechanism currencies already use: no SI
	 * conversion row, so there is no dimension to fall back on and nothing to
	 * compare against. A format whose claim is that a wrong unit fails loudly
	 * must not invent a box-to-pallet factor, and nothing in the code list
	 * supplies one.
	 */
	ASSERT_FALSE(bvn_units_compatible(box, pallet),
	             "boxes and pallets must not be compatible");
	ASSERT_FALSE(bvn_units_compatible(box, one),
	             "a box is not a bare count");
	ASSERT_FALSE(bvn_units_compatible(box, U("k~g")),
	             "a box has no mass");
	ASSERT_FALSE(bvn_units_compatible(box, U("%")),
	             "a box is not dimensionless — it has no dimension at all");

	/* Same base apart from prefixes still converts, exactly as $USD->$USD does.
	 * Note the shape of the answer: bvn_units_compatible says FALSE for a box
	 * against itself, while the factor is 1. That reads oddly until you see
	 * compatibility is a statement about DIMENSION, and neither a currency nor a
	 * package has one. */
	bool cok = true, aff = false;
	double f = bvn_unit_convert_factor(box, box, &cok, &aff);
	ASSERT_TRUE(cok && fabs(f - 1.0) < 1e-12,
	            "a box converts to a box with factor 1");
	cok = true;
	(void)bvn_unit_convert_factor(box, pallet, &cok, &aff);
	ASSERT_FALSE(cok, "a box must not convert to a pallet");

	/* An opaque unit takes no prefix: it is not metric and never was. */
	chk_error("unece:kXBX", error_unit_illegal);
}

/* ── the opaque block is shared, and namespaced ─────────────────────────── */

static void test_opaque_block(void)
{
	printf("  the opaque block is shared across profiles...\n");

	value_unit_t iu  = U("ucum:[IU]");
	value_unit_t box = U("unece:XBX");

	/* Both are opaque, both are profile-only, and each writes back in the
	 * namespace that OWNS it -- which is what the per-profile sub-ranges in
	 * bovnar_profiles.gen.h exist for. A single "ucum:" prefix hardcoded in the
	 * writer would have spelled the box "ucum:XBX", and that re-parses as
	 * nothing at all. */
	ASSERT_TRUE(bvn_unit_is_profile_only(iu),  "[IU] is profile-only");
	ASSERT_TRUE(bvn_unit_is_profile_only(box), "XBX is profile-only");
	chk_str("ucum:[IU]",  "ucum:[IU]");
	chk_str("unece:XBX",  "unece:XBX");

	/* Two profiles' opaque units are as incommensurable as any other pair. */
	ASSERT_FALSE(bvn_units_compatible(iu, box),
	             "an international unit is not a box");
	ASSERT_FALSE(bvn_unit_equal(iu, box), "[IU] is not XBX");

	/* The two profiles own separate blocks of the id space, which is what makes
	 * the ownership test a comparison rather than a search — and what lets
	 * either profile grow without renumbering the other. */
	ASSERT_TRUE(BVN_PROFILE_UCUM_OPAQUE_LAST < BVN_PROFILE_UNECE_OPAQUE_FIRST,
	            "the ucum and unece opaque ranges must not overlap");
	ASSERT_TRUE(BVN_PROFILE_UNECE_OPAQUE_FIRST - BVN_PROFILE_UCUM_OPAQUE_FIRST
	            == 10 * BVN_UNIT_BLOCK_SIZE,
	            "ucum and unece are one block tag apart");
	ASSERT_TRUE(BVN_PROFILE_UNECE_OPAQUE_LAST
	            < BVN_PROFILE_UNECE_OPAQUE_FIRST + BVN_UNIT_BLOCK_SIZE,
	            "unece's opaque units fit inside its own block");
}

/*
 * Every opaque unit of every profile, through text and back.
 *
 * Spot checks covered a handful of codes. This walks the blocks: each unit must
 * be valid, report itself profile-only, serialise into ITS OWN namespace, and
 * re-parse to exactly what it was. The namespace check is the one that matters
 * most — a single hardcoded "ucum:" in the writer would print a UN/ECE package
 * code as `ucum:XBX`, which re-parses as nothing at all.
 */
static void test_every_opaque_unit_round_trips(void)
{
	printf("  every opaque unit, text and back...\n");
	static const struct { int first, last; const char *ns; } BLOCKS[] = {
		{ BVN_PROFILE_UCUM_OPAQUE_FIRST,  BVN_PROFILE_UCUM_OPAQUE_LAST,  "ucum:"  },
		{ BVN_PROFILE_UNECE_OPAQUE_FIRST, BVN_PROFILE_UNECE_OPAQUE_LAST, "unece:" },
	};
	int seen = 0;
	for (size_t k = 0; k < sizeof BLOCKS / sizeof BLOCKS[0]; k++) {
		for (int b = BLOCKS[k].first; b <= BLOCKS[k].last; b++) {
			value_unit_t u = BVN_UNIT_NO_PREFIX((value_base_unit_t)b);
			seen++;
			ASSERT_TRUE(bvn_unit_valid(u), "an opaque unit is valid");
			ASSERT_TRUE(bvn_unit_is_profile_only(u),
			            "an opaque unit is profile-only");
			char s[BVNR_UNIT_STRING_MAX];
			if (bvn_unit_to_string(u, s, sizeof s) <= 0) {
				ASSERT_TRUE(0, "an opaque unit has a spelling");
				continue;
			}
			ASSERT_TRUE(strncmp(s, BLOCKS[k].ns, strlen(BLOCKS[k].ns)) == 0,
			            "and it is written in its OWN namespace");
			bool ok = false;
			value_unit_t back = bvn_parse_unit((const uint8_t *)s, &ok);
			ASSERT_TRUE(ok && bvn_unit_equal(u, back),
			            "and re-parses to the unit it came from");
			/* Commensurable with nothing, including a plain count. */
			ASSERT_FALSE(bvn_units_compatible(u, BVN_UNIT_NO_PREFIX(bu_meter)),
			             "an opaque unit is not a length");
			double out = 0.0;
			ASSERT_TRUE(bvn_unit_convert_value(1.0, u, u, &out) && out == 1.0,
			            "but it does convert to itself");
		}
	}
	ASSERT_TRUE(seen == BVN_PROFILE_OPAQUE_COUNT,
	            "the sweep saw every opaque unit the build defines");
}

/*
 * The three shapes with no spelling at all.
 *
 * A FLAT vocabulary is one whole code looked up entire, so it can write exactly
 * one unprefixed component at exponent 1. An expr vocabulary (UCUM) has an
 * exponent notation and can write the square. Both refusals are structural, and
 * neither makes the unit invalid: a unit with no text is still a unit, which is
 * what bvn_unit_valid answers and bvn_unit_to_string does not.
 */
static void test_units_with_no_spelling(void)
{
	printf("  a valid unit that cannot be written...\n");
	char s[BVNR_UNIT_STRING_MAX];

	value_unit_t sq = BVN_UNIT_SI_EXP(bu_unece_box, si_none, exp_square);
	ASSERT_TRUE(bvn_unit_valid(sq), "a squared box is structurally valid");
	ASSERT_TRUE(bvn_unit_to_string(sq, s, sizeof s) < 0,
	            "but a flat vocabulary has no exponent notation");

	value_unit_t inv = BVN_UNIT_SI_EXP(bu_unece_box, si_none, exp_neg_linear);
	ASSERT_TRUE(bvn_unit_to_string(inv, s, sizeof s) < 0,
	            "nor a reciprocal one");

	value_unit_t mixed = { .num_components = 2 };
	mixed.components[0] = BVN_UNIT_NO_PREFIX(bu_unece_box).components[0];
	mixed.components[1] = BVN_UNIT_NO_PREFIX(bu_meter).components[0];
	ASSERT_TRUE(bvn_unit_valid(mixed), "boxes per metre is a real quantity");
	ASSERT_TRUE(bvn_unit_to_string(mixed, s, sizeof s) < 0,
	            "but a flat vocabulary cannot write a product");

	/* UCUM is an expr profile, so the same shape IS spellable there — which is
	 * what makes the refusals above a property of the vocabulary rather than of
	 * opaque units in general. */
	value_unit_t iu2 = BVN_UNIT_SI_EXP(bu_ucum_iu, si_none, exp_square);
	int32_t n = bvn_unit_to_string(iu2, s, sizeof s);
	ASSERT_TRUE(n > 0, "UCUM can write a squared arbitrary unit");
	bool ok = false;
	ASSERT_TRUE(n > 0 && bvn_unit_equal(bvn_parse_unit((const uint8_t *)s, &ok), iu2)
	            && ok, "and it round-trips");
}

/* ── round trip ─────────────────────────────────────────────────────────── */

static void test_round_trip(void)
{
	printf("  round trip...\n");

	/* Every code in the table must parse, format, and re-parse to the same
	 * unit. The formatter is allowed to choose the NATIVE spelling -- that is
	 * the canonical form -- so this checks the unit, not the text. */
	static const char* codes[] = {
		"unece:MTR", "unece:KMT", "unece:CMT", "unece:MMT", "unece:INH",
		"unece:FOT", "unece:YRD", "unece:SMI", "unece:NMI", "unece:MTK",
		"unece:MTQ", "unece:LTR", "unece:MLT", "unece:HLT", "unece:GLL",
		"unece:KGM", "unece:GRM", "unece:MGM", "unece:TNE", "unece:LBR",
		"unece:ONZ", "unece:SEC", "unece:MIN", "unece:HUR", "unece:DAY",
		"unece:WEE", "unece:MON", "unece:ANN", "unece:KEL", "unece:CEL",
		"unece:FAH", "unece:MOL", "unece:CDL", "unece:NEW", "unece:PAL",
		"unece:KPA", "unece:BAR", "unece:JOU", "unece:WTT", "unece:KWT",
		"unece:KWH", "unece:AMP", "unece:VLT", "unece:OHM", "unece:HTZ",
		"unece:MHZ", "unece:BQL", "unece:GRY", "unece:MTS", "unece:KMH",
		"unece:KNT", "unece:DD",  "unece:P1",  "unece:XBX", "unece:XPX",
		"unece:C62", "unece:H87", "unece:NAR",
	};
	for (size_t i = 0; i < sizeof(codes) / sizeof(codes[0]); i++) {
		bool ok = true;
		value_unit_t u = bvn_parse_unit((const uint8_t*)codes[i], &ok);
		tests++;
		if (!ok) {
			fprintf(stderr, "FAIL: %s did not parse\n", codes[i]);
			failures++;
			continue;
		}
		char buf[BVNR_UNIT_STRING_MAX];
		int32_t n = bvn_unit_to_string(u, buf, sizeof(buf));
		if (n < 0) {
			fprintf(stderr, "FAIL: %s has no spelling at all\n", codes[i]);
			failures++;
			continue;
		}
		bool ok2 = true;
		value_unit_t v = bvn_parse_unit((const uint8_t*)buf, &ok2);
		if (!ok2 || !bvn_unit_equal(u, v)) {
			fprintf(stderr, "FAIL: %s -> %s did not re-parse to itself\n",
			        codes[i], buf);
			failures++;
		}
	}
}

/* ── writing UNECE back out ─────────────────────────────────────────────── */

static void test_to_unece(void)
{
	printf("  emitting a UNECE code from a native unit...\n");

	char buf[BVNR_UNIT_STRING_MAX];

	/* A flat profile can spell a single unprefixed component and nothing else.
	 * That is not a round-trip hole: k~m/h has a perfectly good native spelling
	 * and only an OPAQUE unit is obliged to come back out in profile notation. */
	ASSERT_TRUE(bvn_unit_to_profile("unece", U("k~g"), buf, sizeof(buf)) > 0 &&
	            strcmp(buf, "KGM") == 0,
	            "k~g should emit as KGM");
	ASSERT_TRUE(bvn_unit_to_profile("unece", U("m"), buf, sizeof(buf)) > 0 &&
	            strcmp(buf, "MTR") == 0,
	            "m should emit as MTR");
	ASSERT_TRUE(bvn_unit_to_profile("unece", U("k~m/h"), buf, sizeof(buf)) < 0,
	            "a compound has no flat spelling");

	/*
	 * THE CANONICAL CODE, NOT MERELY A CORRECT ONE. Rec 20 gives several codes
	 * one value: JOU and J55 (watt second) are both the joule, PAL and C55
	 * (newton per square metre) both the pascal, MOL and C34 both the mole. The
	 * writer used to take the shortest code and break ties alphabetically,
	 * which for a code list whose every code is two or three bytes is just
	 * "alphabetically first" — so a joule came out as "J55" and a mole as
	 * "C34". Both are worth the right number; neither is what a Rec 20 reader
	 * expects to receive. A flat profile now writes the FIRST code its data
	 * file lists for a unit, which is the canonical one.
	 *
	 * Nothing caught this when the alias codes were added, because every
	 * assertion here was about a unit with only one code. These are the ones
	 * with more than one.
	 */
#define CHK_UNECE(nat, want) do { \
	tests++; \
	if (bvn_unit_to_profile("unece", U(nat), buf, sizeof(buf)) < 0 || \
		strcmp(buf, (want)) != 0) { \
		fprintf(stderr, "FAIL: %s -> UNECE %s, expected %s\n", \
		        (nat), buf, (want)); \
		failures++; \
	} \
} while (0)
	CHK_UNECE("J",   "JOU");   /* not J55, the watt second               */
	CHK_UNECE("Pa",  "PAL");   /* not C55, the newton per square metre   */
	CHK_UNECE("N",   "NEW");   /* not B12/M77                            */
	CHK_UNECE("W",   "WTT");   /* not P14, the joule per second          */
	CHK_UNECE("C",   "COU");   /* not A8, the ampere second              */
	CHK_UNECE("H",   "HEN");   /* not 81                                 */
	CHK_UNECE("Wb",  "WEB");   /* not F90                                */
	CHK_UNECE("lx",  "LUX");   /* not B60, the lumen per square metre    */
	CHK_UNECE("kat", "KAT");   /* not E95, the mole per second           */
	CHK_UNECE("mol", "MOL");   /* not C34                                */
	CHK_UNECE("S",   "SIE");   /* not NQ, the mho                        */
	CHK_UNECE("Gy",  "GRY");   /* not A95                                */
	CHK_UNECE("t",   "TNE");   /* not 2U, the megagram                   */
	CHK_UNECE("tex", "D34");   /* not C12, the milligram per metre       */
	/* A prefixed decade is its own code, and picks its own canonical one. */
	CHK_UNECE("k~g", "KGM");
	CHK_UNECE("k~W", "KWT");
	CHK_UNECE("k~Pa", "KPA");
#undef CHK_UNECE
	ASSERT_TRUE(bvn_unit_to_profile("unece", U("m²"), buf, sizeof(buf)) < 0,
	            "an exponent has no flat spelling");

	/* A unit outside the table has nowhere to go, and says so. */
	ASSERT_TRUE(bvn_unit_to_profile("unece", U("prln"), buf, sizeof(buf)) < 0,
	            "the prussian line has no UNECE code");
	ASSERT_TRUE(bvn_unit_to_profile("unece", U("$USD"), buf, sizeof(buf)) < 0,
	            "a currency is not a UNECE unit");

	/* An opaque unit is spelled by the profile that owns it, and by no other. */
	ASSERT_TRUE(bvn_unit_to_profile("unece", U("unece:XBX"), buf,
	                                sizeof(buf)) > 0 &&
	            strcmp(buf, "XBX") == 0,
	            "unece owns XBX and can spell it");
	ASSERT_TRUE(bvn_unit_to_profile("ucum", U("unece:XBX"), buf,
	                                sizeof(buf)) < 0,
	            "ucum must not spell another profile's opaque unit");
	ASSERT_TRUE(bvn_unit_to_profile("unece", U("ucum:[IU]"), buf,
	                                sizeof(buf)) < 0,
	            "unece must not spell a UCUM arbitrary atom");

	/* An unknown namespace is a refusal, not a fallback. */
	ASSERT_TRUE(bvn_unit_to_profile("nosuch", U("m"), buf, sizeof(buf)) < 0,
	            "an unknown namespace has no spelling");
}

/* ── policy strings ─────────────────────────────────────────────────────── */

static void test_policy_strings(void)
{
	printf("  the notation works wherever a unit string does...\n");

	/* A reader/writer unit policy takes the same notation with no change: the
	 * profile lives inside bvn_parse_unit, which every path goes through. */
	bool ok = true;
	value_unit_t want = bvn_parse_unit((const uint8_t*)"unece:KGM", &ok);
	ASSERT_TRUE(ok, "a policy string may be written in profile notation");
	ASSERT_TRUE(bvn_units_compatible(want, U("g")),
	            "and compares against a natively spelled unit");
}

int main(void)
{
	printf("UNECE unit profile\n");

	test_outcomes();
	test_codes_unblocked_by_the_new_units();
	test_flatness();
	test_equivalence();
	test_opaque_counts();
	test_opaque_block();
	test_every_opaque_unit_round_trips();
	test_units_with_no_spelling();
	test_round_trip();
	test_to_unece();
	test_policy_strings();

	printf("\n%d tests, %d failures\n", tests, failures);
	if (failures == 0)
		printf("ALL TESTS PASSED\n");
	return failures == 0 ? 0 : 1;
}
