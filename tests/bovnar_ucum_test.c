/*
 * SPDX-License-Identifier: MIT
 * SPDX-FileComment: The test vectors below quote UCUM atom codes. Those
 * identifiers are Regenstrief Institute's and are not covered by the MIT grant
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
 * The UCUM unit profile (spec 1.2). Specified in
 * doc/11_bovnar_unit_profiles.md; this file is what holds the specification to
 * the implementation.
 *
 * The tests are grouped by the claim they defend rather than by the function
 * they call, because most of the claims are about how several pieces agree: a
 * profile unit must be indistinguishable from the native spelling to EVERY
 * downstream consumer, which is a statement about the parser, the formatter, the
 * comparator and the conversion path at once.
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

/* Parse `src` and check the coherent-SI factor. This is the check that catches a
 * fold done in the wrong direction, which a formatting test alone would not:
 * a wrong prefix still formats as a perfectly readable unit. */
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

static value_unit_t U(const char* s)
{
	bool ok = true;
	return bvn_parse_unit((const uint8_t*)s, &ok);
}

/* ── the three outcomes ─────────────────────────────────────────────────── */

static void test_outcomes(void)
{
	printf("  the three outcomes...\n");

	/* Translated. */
	chk_str("ucum:mm[Hg]", "mmHg");
	chk_str("ucum:m",      "m");

	/* Profile-only: parses, and formats back in profile notation because it has
	 * no native spelling. */
	chk_str("ucum:[IU]", "ucum:[IU]");

	/* Refused, split by cause. The split is the point: "you wrote it wrong",
	 * "you wrote it right and we cannot carry it", and "no such profile" call
	 * for three different fixes. */
	chk_error("ucum:metre",  error_unit_illegal);
	chk_error("ucum:B[SPL]", error_unit_profile_unsupported);
	chk_error("zz:m",        error_unit_profile_unknown);

	/* A natively spelled unit is untouched by any of this. */
	chk_error("nosuchunit",  error_unit_illegal);
	tests++;
	if (bvn_unit_error_code((const uint8_t*)"m/s", 3) != error_none) {
		fprintf(stderr, "FAIL: a unit that parses must report error_none\n");
		failures++;
	}
}

/* ── indistinguishable from the native spelling ─────────────────────────── */

static void test_equivalence(void)
{
	printf("  a translated unit equals its native spelling...\n");
	ASSERT_TRUE(bvn_unit_equal(U("ucum:mm[Hg]"), U("mmHg")),
	            "ucum:mm[Hg] == mmHg");
	ASSERT_TRUE(bvn_unit_equal(U("ucum:kg.m/s2"), U("k~g·m·s⁻²")),
	            "ucum:kg.m/s2 == k~g·m·s⁻²");
	ASSERT_TRUE(bvn_unit_equal(U("ucum:Cel"), U("°C")), "ucum:Cel == °C");
	ASSERT_TRUE(bvn_units_compatible(U("ucum:mm[Hg]"), U("k~Pa")),
	            "ucum:mm[Hg] is a pressure");

	/* And a wrong one still is wrong. */
	ASSERT_FALSE(bvn_unit_equal(U("ucum:m"), U("s")), "ucum:m != s");
	/*
	 * UCUM defines the osmole and the equivalent identically -- one mole each --
	 * so the two rows have to agree. They did not: osm was refused as a
	 * "species-dependent relation to the mole", which is equally true of eq
	 * (valence) and is a fact about what an osmolality MEANS rather than about
	 * what UCUM says the unit is worth. qudt.bvnr had meanwhile mapped
	 * MilliOSM-PER-KiloGM onto m~mol/k~g, making the same identification a
	 * third way.
	 */
	ASSERT_TRUE(bvn_unit_equal(U("ucum:osm"), U("mol")), "ucum:osm == mol");
	ASSERT_TRUE(bvn_unit_equal(U("ucum:osm"), U("ucum:eq")), "ucum:osm == ucum:eq");
	ASSERT_TRUE(bvn_unit_equal(U("ucum:mosm"), U("m~mol")), "ucum:mosm == m~mol");
	/*
	 * Both carry .reverse = false, and for osm that is load-bearing in a way it
	 * is not for eq: "osm" and "mol" are both three bytes, so only the flag
	 * stands between every amount of substance and being WRITTEN as an osmole.
	 */
	{
		char buf[64];
		int32_t n = bvn_unit_to_profile("ucum", U("mol"), buf, sizeof buf);
		ASSERT_TRUE(n > 0 && strcmp(buf, "mol") == 0,
			    "a mole writes as ucum:mol, never as osm or eq");
	}
}

/* ── operators ──────────────────────────────────────────────────────────── */

static void test_operators(void)
{
	printf("  UCUM's '/' binds to one term, bovnar's latches...\n");
	/* The single most dangerous difference between the two grammars. UCUM's '/'
	 * is a binary operator applying to the term that follows it; bovnar's
	 * latches for the rest of the expression. Read "kg/m.s2" with the wrong rule
	 * and s² lands in the denominator. */
	chk_str("ucum:kg.m/s2", "k~g·m/s²");
	chk_str("ucum:kg/m.s2", "k~g·s²/m");
	ASSERT_FALSE(bvn_unit_equal(U("ucum:kg/m.s2"), U("k~g/m·s²")),
	             "ucum:kg/m.s2 is NOT the native k~g/m·s²");

	/* The reciprocal form. Bovnar has no unity atom to divide, so it becomes a
	 * negative exponent. */
	chk_str("ucum:/min", "min⁻¹");
	chk_factor("ucum:/min", 1.0 / 60.0);

	/* Grouping, and the unity. */
	chk_str("ucum:kg/(m.s2)", "k~g/m·s²");
	chk_str("ucum:1", "no_unit");
}

/* ── annotations ────────────────────────────────────────────────────────── */

static void test_annotations(void)
{
	printf("  annotations are inert...\n");
	/* UCUM defines an annotation as carrying no meaning, and a standalone one as
	 * the unity. Implemented faithfully, which means two units a clinician reads
	 * as different compare as the same -- documented in
	 * doc/11_bovnar_unit_profiles.md 6.3 as a trap rather than hidden. */
	chk_str("ucum:mL{total}", "m~L");
	ASSERT_TRUE(bvn_unit_equal(U("ucum:mL{total}"), U("ucum:mL")),
	            "an annotation does not change the unit");
	ASSERT_TRUE(bvn_unit_equal(U("ucum:{RBC}/uL"), U("ucum:{cells}/uL")),
	            "two annotations of the same unit compare equal");
	chk_str("ucum:{RBC}", "no_unit");

	/* An unterminated annotation must not swallow the rest of the string. */
	chk_error("ucum:mL{total", error_unit_illegal);
	/* Nor may it carry a byte that ends a value. */
	chk_error("ucum:mL{a;b}", error_unit_illegal);
}

/* ── the decade fold ────────────────────────────────────────────────────── */

static void test_fold(void)
{
	printf("  scale factors and the decade fold...\n");
	/* The fold divides the residual decade by the component's exponent. In a
	 * denominator that is the difference between the right answer and one
	 * fifteen orders of magnitude out. */
	chk_str("ucum:10*3/uL", "n~L⁻¹");
	chk_factor("ucum:10*3/uL", 1e12);
	chk_str("ucum:10*6/L", "µ~L⁻¹");
	chk_factor("ucum:10*6/L", 1e9);
	chk_str("ucum:/uL", "µ~L⁻¹");
	chk_factor("ucum:/uL", 1e9);

	/* A prefix is a decade on top of the target, scaled by the exponent. */
	chk_str("ucum:km2", "k~m²");
	chk_factor("ucum:km2", 1e6);

	/* The target may already carry a prefix: m[Hg] is a METRE of mercury, so the
	 * milli in mm[Hg] cancels the kilo and leaves plain mmHg. */
	chk_str("ucum:m[Hg]", "k~mmHg");
	chk_factor("ucum:m[Hg]", 133322.387415);
	chk_factor("ucum:mm[Hg]", 133.322387415);

	/* A prefix is discharged into its OWN atom when it fits, so the output reads
	 * like the code that was written. c~g/L is the same quantity and would be
	 * just as correct; m~g/d~L is what a producer recognises. */
	chk_str("ucum:mg/dL", "m~g/d~L");
	chk_factor("ucum:mg/dL", 0.01);

	/* No SI prefix has decade 4, 5, 7 or 8, so these are refused rather than
	 * silently rescaled. */
	chk_error("ucum:10*4/L", error_unit_profile_unsupported);
	chk_error("ucum:10*5.m", error_unit_profile_unsupported);
	/* And a scale with no component to carry it at all. */
	chk_error("ucum:10*3", error_unit_profile_unsupported);
}

/* ── collisions ─────────────────────────────────────────────────────────── */

static void test_collisions(void)
{
	printf("  the same spelling, a different unit...\n");
	/*
	 * These are why translation runs on UCUM's resolved atom and never by
	 * handing the UCUM string to the native parser. Each pair below parses in
	 * both namespaces and means something different in each.
	 */
	int32_t dn[bvn_si_dim_count], dp[bvn_si_dim_count];

	/* st: the native stone (mass) against the UCUM stere (volume). The sharpest
	 * one, because both parse and the dimensions differ. */
	ASSERT_TRUE(bvn_unit_dimension_vector(U("st"), dn), "native st has dims");
	ASSERT_TRUE(bvn_unit_dimension_vector(U("ucum:st"), dp), "ucum:st has dims");
	ASSERT_TRUE(dn[bvn_si_dim_kilogram] == 1 && dn[bvn_si_dim_meter] == 0,
	            "native st is a mass");
	ASSERT_TRUE(dp[bvn_si_dim_meter] == 3 && dp[bvn_si_dim_kilogram] == 0,
	            "ucum:st is a volume");

	/* B: the native byte against the UCUM bel. b: the native bit against the
	 * UCUM barn. Gb: the native gigabit against nothing (UCUM's gilbert is not
	 * in the table), which is still not the native reading. */
	ASSERT_TRUE(bvn_unit_equal(U("ucum:By"), U("B")), "ucum:By is the byte");
	ASSERT_TRUE(bvn_unit_equal(U("ucum:B"),  U("da~dB")), "ucum:B is the bel");
	ASSERT_FALSE(bvn_unit_equal(U("ucum:B"), U("B")),
	             "ucum:B is NOT the native byte");
	ASSERT_TRUE(bvn_unit_equal(U("ucum:b"),  U("barn")), "ucum:b is the barn");
	ASSERT_FALSE(bvn_unit_equal(U("ucum:b"), U("b")),
	             "ucum:b is NOT the native bit");

	/* eq: UCUM's generic equivalent is a mole. Bovnar's native val is the
	 * water-analysis equivalent of a divalent species, factor 0.5 -- mapping one
	 * onto the other would be wrong by exactly two on every clinical
	 * electrolyte value. */
	ASSERT_TRUE(bvn_unit_equal(U("ucum:eq"), U("mol")), "ucum:eq is the mole");
	ASSERT_FALSE(bvn_unit_equal(U("ucum:eq"), U("val")),
	             "ucum:eq is NOT the native val");
	chk_factor("ucum:meq/L", 1.0);   /* mmol/L; m~val/L would be 0.5 */

	/* The native eq stays an alias of val, by decision (doc/05, water
	 * analysis): native meq/L is half of ucum:meq/L. */
	ASSERT_TRUE(bvn_unit_equal(U("eq"), U("val")), "native eq is val");
	ASSERT_FALSE(bvn_unit_equal(U("eq"), U("ucum:eq")),
	             "native eq is NOT ucum:eq");
	chk_factor("meq/L", 0.5);        /* the divalent equivalent */
}

/* ── arbitrary units ────────────────────────────────────────────────────── */

static void test_arbitrary(void)
{
	printf("  arbitrary units are commensurable with nothing...\n");
	bool ok = true, aff = false;
	double off = 0.0;

	value_unit_t iu  = U("ucum:[IU]");
	value_unit_t pfu = U("ucum:[PFU]");

	ASSERT_TRUE(bvn_unit_is_profile_only(iu), "[IU] has no native spelling");
	ASSERT_FALSE(bvn_unit_is_profile_only(U("ucum:m")),
	             "a translated unit is not profile-only");

	/* No SI factor, no dimension. This is what makes them incommensurable: the
	 * conversion row in si_conv_table exists only to keep the table well-formed,
	 * and would say [IU] is a plain count if these guards were missing. */
	ok = true;
	(void)bvn_unit_to_si_factor(iu, &aff, &off, &ok);
	ASSERT_FALSE(ok, "[IU] has no SI factor");
	int32_t d[bvn_si_dim_count];
	ASSERT_FALSE(bvn_unit_dimension_vector(iu, d), "[IU] has no dimension");

	/* Identity converts; anything else does not -- including another arbitrary
	 * unit, which is the case a single shared "opaque" base unit would have got
	 * silently wrong. */
	bool cok = true, aff2 = false;
	double f = bvn_unit_convert_factor(iu, iu, &cok, &aff2);
	ASSERT_TRUE(cok && fabs(f - 1.0) < 1e-15, "[IU] -> [IU] is identity");
	cok = true;
	(void)bvn_unit_convert_factor(iu, pfu, &cok, &aff2);
	ASSERT_FALSE(cok, "[IU] does not convert to [PFU]");
	cok = true;
	(void)bvn_unit_convert_factor(iu, U("mol"), &cok, &aff2);
	ASSERT_FALSE(cok, "[IU] does not convert to mol");
	cok = true;
	(void)bvn_unit_convert_factor(iu, BVN_UNIT_NONE, &cok, &aff2);
	ASSERT_FALSE(cok, "[IU] does not convert to a plain count");

	/* But the non-arbitrary part of a compound still converts: [IU]/L is a real
	 * concentration and rescaling its volume is exact. */
	cok = true;
	f = bvn_unit_convert_factor(U("ucum:[IU]/L"), U("ucum:[IU]/mL"),
	                            &cok, &aff2);
	ASSERT_TRUE(cok && fabs(f - 0.001) < 1e-15,
	            "[IU]/L -> [IU]/mL is exactly 1/1000");

	/* UCUM's metric flag is enforced: [IU] takes prefixes, [arb'U] does not. */
	chk_str("ucum:k[IU]", "ucum:k[IU]");
	chk_error("ucum:k[arb'U]", error_unit_illegal);

	/* The one exception UCUM states: [IU] is defined as 1 [iU]. Both spellings
	 * read as one opaque unit, so they compare equal, convert by factor 1 and
	 * write back as [IU]; [iU] keeps [IU]'s prefix rule. Nothing else moves:
	 * [iU] is still not [PFU]. */
	value_unit_t iu_i = U("ucum:[iU]");
	ASSERT_TRUE(bvn_unit_equal(iu_i, iu), "[iU] is [IU]");
	ASSERT_TRUE(bvn_unit_is_profile_only(iu_i), "[iU] has no native spelling");
	ASSERT_FALSE(bvn_unit_equal(iu_i, pfu), "[iU] is NOT [PFU]");
	cok = true;
	f = bvn_unit_convert_factor(iu_i, iu, &cok, &aff2);
	ASSERT_TRUE(cok && fabs(f - 1.0) < 1e-15, "[iU] -> [IU] is identity");
	cok = true;
	f = bvn_unit_convert_factor(U("ucum:[iU]/L"), U("ucum:[IU]/mL"),
	                            &cok, &aff2);
	ASSERT_TRUE(cok && fabs(f - 0.001) < 1e-15,
	            "[iU]/L -> [IU]/mL is exactly 1/1000");
	cok = true;
	(void)bvn_unit_convert_factor(iu_i, pfu, &cok, &aff2);
	ASSERT_FALSE(cok, "[iU] does not convert to [PFU]");
	chk_str("ucum:[iU]", "ucum:[IU]");
	chk_str("ucum:k[iU]", "ucum:k[IU]");
	ASSERT_TRUE(bvn_unit_equal(U("ucum:m[iU]/mL"), U("ucum:m[IU]/mL")),
	            "m[iU]/mL is m[IU]/mL");
}

/* ── round trip ─────────────────────────────────────────────────────────── */

static void test_round_trip(void)
{
	printf("  profile-only units round-trip through the formatter...\n");
	static const char* const cases[] = {
		"ucum:[IU]", "ucum:[IU]/L", "ucum:k[IU]/L", "ucum:[PFU]/mL",
		"ucum:[arb'U]", "ucum:[CFU]/mL", NULL
	};
	for (const char* const* c = cases; *c; c++) {
		bool ok = true;
		value_unit_t u = bvn_parse_unit((const uint8_t*)*c, &ok);
		tests++;
		if (!ok) {
			fprintf(stderr, "FAIL: %s did not parse\n", *c);
			failures++;
			continue;
		}
		char buf[BVNR_UNIT_STRING_MAX];
		if (bvn_unit_to_string(u, buf, sizeof(buf)) < 0) {
			fprintf(stderr, "FAIL: %s has no written form\n", *c);
			failures++;
			continue;
		}
		bool ok2 = true;
		value_unit_t back = bvn_parse_unit((const uint8_t*)buf, &ok2);
		if (!ok2 || !bvn_unit_equal(u, back)) {
			fprintf(stderr, "FAIL: %s -> %s did not re-parse to itself\n",
			        *c, buf);
			failures++;
		}
	}
}

/*
 * doc/11 §5.3 quotes a sweep of the whole native registry: how many spellings
 * survive native -> UCUM -> native unchanged, how many have no UCUM code, and
 * -- the one that is a correctness claim rather than a statistic -- that none
 * lands on a DIFFERENT unit. Nothing pinned any of the three, and the first had
 * already drifted by two as the registry grew.
 *
 * The prefix list and the physical-unit id ranges are part of the measurement:
 * change either and the counts change, which is why they are spelled out here
 * and in the document rather than left to be inferred.
 */
static void test_sweep_round_trip(void)
{
	static const si_prefix_id_t PREFIXES[] = {
		si_none, si_deca, si_hecto, si_kilo, si_mega, si_giga,
		si_tera, si_deci, si_centi, si_milli, si_micro, si_nano
	};
	long same = 0, no_code = 0, differ = 0, rejected = 0;

	printf("  sweeping the registry for native -> UCUM -> native...\n");
	/* Block 10 only: the native units, one contiguous run. Currencies
	 * (block 90) and the profiles' opaque units (blocks 20+) have no native
	 * spelling to sweep and are covered by their own tests. */
	for (int b = BVN_UNIT_NATIVE_FIRST; b <= BVN_UNIT_NATIVE_LAST; b++) {
		for (size_t p = 0; p < sizeof(PREFIXES) / sizeof(PREFIXES[0]); p++) {
			value_unit_prefix_t pf;
			memset(&pf, 0, sizeof(pf));
			pf.system = prefix_si;
			pf.id.si  = PREFIXES[p];
			if (!bvn_prefix_unit_valid(pf, (value_base_unit_t)b)) {
				rejected++;
				continue;
			}
			value_unit_t u;
			memset(&u, 0, sizeof(u));
			u.num_components         = 1;
			u.components[0].base     = (value_base_unit_t)b;
			u.components[0].exponent = exp_linear;
			u.components[0].prefix   = pf;

			char native[BVNR_UNIT_STRING_MAX], code[BVNR_UNIT_STRING_MAX];
			if (bvn_unit_to_string(u, native, sizeof(native)) < 0) {
				rejected++;
				continue;
			}
			if (bvn_unit_to_ucum(u, code, sizeof(code)) < 0) {
				no_code++;
				continue;
			}
			char qualified[BVNR_UNIT_STRING_MAX + 8];
			snprintf(qualified, sizeof(qualified), "ucum:%s", code);
			bool ok = true;
			value_unit_t back = bvn_parse_unit((const uint8_t*)qualified, &ok);
			if (!ok || !bvn_unit_equal(u, back)) {
				differ++;
				fprintf(stderr, "FAIL: %s -> ucum:%s did not return to itself\n",
				        native, code);
				continue;
			}
			same++;
		}
	}

	tests++;
	if (differ != 0) {
		fprintf(stderr, "FAIL: %ld unit(s) round-tripped to a different unit\n",
		        differ);
		failures++;
	}
	/* The figures doc/11 §5.3 quotes. Kept as named constants so the failure
	 * message cannot disagree with what is actually compared. */
	enum { DOC_SAME = 865, DOC_NO_CODE = 1926, DOC_REJECTED = 377 };
	tests++;
	if (same != DOC_SAME || no_code != DOC_NO_CODE || rejected != DOC_REJECTED) {
		fprintf(stderr,
		        "FAIL: sweep is %ld same / %ld without a code / %ld rejected; "
		        "doc/11 §5.3 says %d / %d / %d. Update the document with the "
		        "measured figures if the registry grew.\n",
		        same, no_code, rejected,
		        DOC_SAME, DOC_NO_CODE, DOC_REJECTED);
		failures++;
	}
}

/* ── writing UCUM back out ──────────────────────────────────────────────── */

static void test_to_ucum(void)
{
	printf("  emitting UCUM from a unit...\n");
	char buf[BVNR_UNIT_STRING_MAX];

	tests++;
	if (bvn_unit_to_ucum(U("m/s"), buf, sizeof(buf)) < 0 ||
		strcmp(buf, "m.s-1") != 0) {
		fprintf(stderr, "FAIL: m/s -> UCUM gave %s, expected m.s-1\n", buf);
		failures++;
	}
	tests++;
	if (bvn_unit_to_ucum(U("k~m"), buf, sizeof(buf)) < 0 ||
		strcmp(buf, "km") != 0) {
		fprintf(stderr, "FAIL: k~m -> UCUM gave %s, expected km\n", buf);
		failures++;
	}

	/* Partial by construction. A native unit outside the transliteration table
	 * has nowhere to go, and saying so beats inventing a code. */
	ASSERT_TRUE(bvn_unit_to_ucum(U("$USD"), buf, sizeof(buf)) < 0,
	            "a currency has no UCUM code");
	ASSERT_TRUE(bvn_unit_to_ucum(U("°dH"), buf, sizeof(buf)) < 0,
	            "German water hardness has no UCUM code");
	ASSERT_TRUE(bvn_unit_to_ucum(U("NTU"), buf, sizeof(buf)) < 0,
	            "turbidity has no UCUM code");
	/* A too-small buffer is a refusal, not a truncation. */
	ASSERT_TRUE(bvn_unit_to_ucum(U("m/s"), buf, 3) < 0,
	            "a short buffer is refused");

	/*
	 * Exponents past 9 are spelled, not smeared into one byte.
	 *
	 * The emitter wrote `(char)('0' + v)`, which was right only while
	 * unit_exponent_t stopped at ±9. Past that it emitted whatever byte the
	 * arithmetic produced: m^10 came out as "m:" — a colon, which is the
	 * profile namespace separator — and m^100 as "m" plus a raw 0x94. The
	 * profile PARSER had accepted two- and three-digit exponents all along, so
	 * these were units the library could read and could not write back.
	 *
	 * Each case is checked by ROUND TRIP as well as by text, because the way
	 * this failed was producing something that still looked like a code.
	 */
	static const struct { const char *unit; const char *code; } exps[] = {
		{ "m^9",       "m9"       },
		{ "m^10",      "m10"      },
		{ "m^100",     "m100"     },
		{ "m^-10",     "m-10"     },
		{ "m^-100",    "m-100"    },
		{ "k~m^10",    "km10"     },
		{ "m^10/s^10", "m10.s-10" },
	};
	for (size_t i = 0; i < sizeof exps / sizeof exps[0]; i++) {
		value_unit_t u = U(exps[i].unit);
		tests++;
		if (bvn_unit_to_ucum(u, buf, sizeof(buf)) < 0 ||
		    strcmp(buf, exps[i].code) != 0) {
			fprintf(stderr, "FAIL: %s -> UCUM gave %s, expected %s\n",
			        exps[i].unit, buf, exps[i].code);
			failures++;
			continue;
		}
		char full[BVNR_UNIT_STRING_MAX + 8];
		snprintf(full, sizeof full, "ucum:%s", buf);
		bool ok = false;
		value_unit_t back = bvn_parse_unit((const uint8_t *)full, &ok);
		ASSERT_TRUE(ok, "the emitted UCUM code re-parses");
		ASSERT_TRUE(ok && bvn_unit_equal(u, back),
		            "and re-parses to the unit it came from");
	}
}

/* ── regressions ────────────────────────────────────────────────────────── */

static void test_regressions(void)
{
	printf("  regressions...\n");

	/*
	 * A long digit run in an exponent overflowed a signed int — undefined
	 * behaviour, and in practice "m" followed by twenty nines wrapped to a
	 * plausible value and parsed as m⁻¹ instead of being refused. A unit
	 * parameter may be 255 bytes, so it was reachable from a document.
	 */
	chk_error("ucum:m99999999999999999999", error_unit_profile_unsupported);
	chk_error("ucum:m-99999999999999999999", error_unit_profile_unsupported);
	/* 10 used to be past the limit. The range is now
	 * BVN_EXPONENT_MIN..BVN_EXPONENT_MAX, so the first refusal is at 101. */
	chk_str("ucum:m10",    "m¹⁰");
	chk_str("ucum:m100",   "m¹⁰⁰");
	chk_str("ucum:m-100",  "m⁻¹⁰⁰");
	chk_error("ucum:m101",  error_unit_profile_unsupported);
	chk_error("ucum:m-101", error_unit_profile_unsupported);
	chk_error("ucum:m0",    error_unit_profile_unsupported);
	chk_error("ucum:m0",  error_unit_profile_unsupported);

	/*
	 * Prefix resolution tried only the LONGEST prefix that headed the code.
	 * UCUM's prefixes are not suffix-free — "d" is a head of "da" — so "dar",
	 * the deciare, matched "da", found no atom "r", and was refused although it
	 * is legal UCUM. Every viable split has to be tried.
	 */
	chk_str("ucum:dar", "m~ha");       /* deci + are  = 10 m²   */
	chk_str("ucum:dam", "da~m");       /* deca + metre          */
	chk_str("ucum:dB",  "dB");         /* deci + bel            */
	chk_str("ucum:har", "ha");         /* hecto + are           */
	/* A whole atom still outranks any prefixed reading of it. */
	chk_str("ucum:mho", "S");
	chk_str("ucum:cd",  "cd");
	chk_str("ucum:cal", "cal");
	/* And a prefix on a non-metric atom stays illegal however it is split. */
	chk_error("ucum:datm", error_unit_illegal);

	/*
	 * bvn_unit_to_ucum picked whichever row the length-sorted parse table
	 * reached first, so siemens came out as "mho" and the calorie as "cal_th".
	 * The generated reverse table picks the shortest code, ties alphabetically
	 * — an expression profile's competing rows are spellings of one atom, and
	 * the vocabulary's own abbreviation is the short one.
	 *
	 * Except where the short one means something else. `.reverse = false` in
	 * ucum.bvnr takes those rows out of the running: "eq" is two bytes and is
	 * the EQUIVALENT, so shortest-wins wrote every mole out as an equivalent;
	 * "[oz_ap]" is a byte shorter than "[oz_tr]" and says apothecary about a
	 * document that never did.
	 */
	char buf[BVNR_UNIT_STRING_MAX];
#define CHK_UCUM(nat, want) do { \
	tests++; \
	if (bvn_unit_to_ucum(U(nat), buf, sizeof(buf)) < 0 || \
		strcmp(buf, (want)) != 0) { \
		fprintf(stderr, "FAIL: %s -> UCUM %s, expected %s\n", \
		        (nat), buf, (want)); \
		failures++; \
	} \
} while (0)
	CHK_UCUM("S",   "S");
	CHK_UCUM("cal", "cal");
	CHK_UCUM("mo",  "mo");
	CHK_UCUM("yr",  "a");
	CHK_UCUM("L",   "L");
	/* The two `.reverse = false` rows. Both spellings still PARSE — the flag
	 * governs the writer only — so each assertion is a pair. */
	CHK_UCUM("mol",  "mol");
	CHK_UCUM("oz_t", "[oz_tr]");
	chk_str("ucum:eq",       "mol");
	chk_str("ucum:[oz_ap]",  "oz_t");
	/*
	 * And a base whose UCUM atom is itself prefixed had no UCUM form at all.
	 * "m[Hg]" is a METRE of mercury — bovnar's mmHg times 10³ — so writing plain
	 * mmHg means emitting the prefix for (0 - 3). Same shape for the hectare,
	 * whose atom is "ar".
	 */
	CHK_UCUM("mmHg", "mm[Hg]");
	CHK_UCUM("ha",   "har");
#undef CHK_UCUM

	/*
	 * The near misses. Each of these pairs is dimensionally identical and
	 * numerically different, which is the failure no later check would catch,
	 * and each used to be half refused: bovnar had the IT BTU and not the
	 * thermochemical one, the thermochemical calorie and not the IT one, the
	 * avoirdupois dram and not the apothecary one, so [Btu], cal_IT and [dr_ap]
	 * could only be turned away.
	 *
	 * Both halves of each pair are now units, so what is pinned here is that
	 * they stay APART: the two spellings land on two different bases, at the
	 * two different values, and the unqualified UCUM code lands on the one UCUM
	 * says it means. A regression that collapsed a pair would be exactly the
	 * silent 0.07 % (or 2.2x) error the refusals were protecting against.
	 */
	chk_str("ucum:[Btu_IT]", "Btu");
	chk_str("ucum:[Btu_th]", "Btu_th");
	chk_str("ucum:[Btu]",    "Btu_th");   /* UCUM defines [Btu] AS [Btu_th] */
	chk_factor("ucum:[Btu_IT]", 1055.05585262);
	chk_factor("ucum:[Btu_th]", 1054.3502644888889);
	chk_str("ucum:cal_th",  "cal");
	chk_str("ucum:cal",     "cal");       /* UCUM's bare cal is thermochemical */
	chk_str("ucum:cal_IT",  "cal_IT");
	chk_factor("ucum:cal",    4.184);
	chk_factor("ucum:cal_IT", 4.1868);
	chk_str("ucum:[dr_av]",  "dr");
	chk_str("ucum:[dr_ap]",  "dr_ap");
	chk_factor("ucum:[dr_av]", 0.0017718451953125);
	chk_factor("ucum:[dr_ap]", 0.0038879346);
	chk_factor("ucum:[Cal]", 4184.0);
	/* The apothecary pound IS the troy pound — twelve apothecary ounces, and an
	 * apothecary ounce is a troy ounce — so these two DO land together. */
	chk_str("ucum:[lb_tr]", "lb_t");
	chk_str("ucum:[lb_ap]", "lb_t");
	/* The water column, and the inch built on it the way UCUM builds it. */
	chk_factor("ucum:m[H2O]",  9806.65);
	chk_factor("ucum:cm[H2O]",   98.0665);
	chk_str("ucum:m[H2O]", "mH2O");
	/* The short hundredweight is exactly a hecto-pound and needs no unit of its
	 * own; the long one is 112 lb and does. */
	chk_str("ucum:[scwt_av]", "h~lb");
	chk_str("ucum:[lcwt_av]", "cwt_l");

	/* An unknown lowercase namespace reports the profile error, not a generic
	 * one — a consumer needs to tell "no such profile" from "not a unit". */
	chk_error("nosuch:m", error_unit_profile_unknown);
}

/* ── the spec-1.2 gate ──────────────────────────────────────────────────── */

/*
 * UCUM's own metric flag, and its binary prefixes.
 *
 * Twenty rows of ucum.bvnr carried `.metric = false` for atoms ucum-essence.xml
 * marks isMetric="yes" -- the whole CGS table, gf, erg, dyn, [g], [e], g%, Lmb
 * and three calories. A prefix is only tried on a metric atom, so the prefixed
 * codes fell through to "not a UCUM atom": cP, cSt and kgf, three of the
 * commonest codes in the vocabulary, were error_unit_illegal. And the four
 * binary prefixes were described by doc/11 section 3.2 and read by nothing.
 */
static void test_metric_flags_and_binary_prefixes(void)
{
	printf("  metric flags and binary prefixes...\n");
	chk_str("ucum:cP",   "c~P");
	chk_factor("ucum:cP", 0.001);                 /* 1 cP = 1 mPa.s */
	chk_str("ucum:cSt",  "c~St");
	chk_factor("ucum:cSt", 1e-6);
	chk_str("ucum:mGal", "m~Gal");
	chk_factor("ucum:mGal", 1e-5);
	chk_str("ucum:mG",   "m~G");
	chk_factor("ucum:kgf", 9.80665);
	chk_factor("ucum:kdyn", 1e-2);
	chk_factor("ucum:kcal_m", 4190.02);
	chk_factor("ucum:m[g]", 9.80665e-3);
	/* Still an error where UCUM says non-metric: the fix is the flag, not a
	 * prefix on everything. */
	chk_error("ucum:kAo",  error_unit_illegal);       /* angstrom */
	chk_error("ucum:matm", error_unit_illegal);
	chk_error("ucum:k[arb'U]", error_unit_illegal);

	chk_str("ucum:KiBy",  "Ki~B");
	chk_str("ucum:MiBy",  "Mi~B");
	chk_str("ucum:Gibit", "Gi~b");
	chk_str("ucum:TiBy",  "Ti~B");
	chk_str("ucum:KiBy/s", "Ki~B/s");
	chk_str("ucum:MiBy2", "Mi~B²");
	chk_factor("ucum:KiBy", 1024.0);
	/* A kibimetre is legal UCUM and bovnar has no such unit: known, and not
	 * carryable -- as natively, where Ki~m is refused. */
	chk_error("ucum:Kim", error_unit_profile_unsupported);
	chk_error("ucum:Kig", error_unit_profile_unsupported);
	/* The decimal reading is untouched, and a whole atom still wins. */
	chk_str("ucum:kBy", "k~B");
	chk_str("ucum:Mm",  "M~m");
	/* Binary prefixes are UCUM's; UDUNITS has none. */
	chk_error("udunits:Kibyte", error_unit_illegal);
}

static void test_version_gate(void)
{
	printf("  the notation is gated on a declared 1.2...\n");
	/*
	 * The notation changes which documents are VALID, so it is gated on the
	 * declared version exactly as the datetime family and the \x/\u escapes are
	 * gated on 1.1. Without the gate a document could carry a unit that every
	 * conforming reader of its own declared version has to reject.
	 *
	 * bvn_parse_unit itself is NOT gated and must not be: it is an API entry
	 * point with no document and therefore no declared version. The gate lives
	 * where a version exists -- the validator, for both unit slots, and the
	 * writer, for the units that can only be spelled in the notation.
	 */
	static const struct { const char* doc; error_code_t want; } cases[] = {
		{ "#!bovnar 1.2\n.x = <float:64,ucum:m> 1.0;", error_none },
		{ "#!bovnar 1.2\n.x = 1.0 ucum:m;",            error_none },
		{ "#!bovnar 2.0\n.x = <float:64,ucum:m> 1.0;", error_none },
		{ "#!bovnar 1.1\n.x = <float:64,ucum:m> 1.0;", error_unit_illegal },
		{ "#!bovnar 1.0\n.x = <float:64,ucum:m> 1.0;", error_unit_illegal },
		{ ".x = <float:64,ucum:m> 1.0;",               error_unit_illegal },
		{ "#!bovnar 1.1\n.x = 1.0 ucum:m;",            error_unit_illegal },
		/* A native unit is untouched in every version -- the bump is additive. */
		{ "#!bovnar 1.0\n.x = <float:64,mmHg> 1.0;",   error_none },
		{ ".x = <float:64,mmHg> 1.0;",                 error_none },
	};
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		bvnr_reader_t* r = bvnr_reader_create();
		tests++;
		if (!r) { failures++; continue; }
		bvnr_read_flags_t f;
		memset(&f, 0, sizeof(f));
		error_code_t got = error_none;
		if (bvnr_open_read_mem(r, cases[i].doc,
		                       (uint64_t)strlen(cases[i].doc), NULL, 0, &f)) {
			if (!bvnr_read(r))
				got = bvnr_reader_get_error(r);
		}
		if (got != cases[i].want) {
			fprintf(stderr, "FAIL: %s -> %s, expected %s\n",
			        cases[i].doc, bvn_error_to_string(got),
			        bvn_error_to_string(cases[i].want));
			failures++;
		}
		bvnr_reader_destroy(r);
	}

	/*
	 * The writer's half. A unit with no native spelling can only be emitted in
	 * the notation, so writing one without a 1.2 directive would produce a
	 * document this library refuses to read -- reported to the caller as
	 * success. A TRANSLATED unit needs no guard: it is written natively.
	 */
	struct { bool emit; uint16_t maj, min; const char* unit; error_code_t want; }
	wc[] = {
		{ false, 0, 0, "ucum:[IU]", error_unsupported_spec_version },
		{ true,  1, 1, "ucum:[IU]", error_unsupported_spec_version },
		{ true,  1, 2, "ucum:[IU]", error_none },
		/* translated: native spelling, so no directive is needed */
		{ false, 0, 0, "ucum:mm[Hg]", error_none },
		{ true,  1, 0, "ucum:mm[Hg]", error_none },
	};
	for (size_t i = 0; i < sizeof(wc) / sizeof(wc[0]); i++) {
		char buf[512];
		bvnr_writer_t* w = bvnr_writer_create();
		tests++;
		if (!w) { failures++; continue; }
		error_code_t got = error_none;
		if (bvnr_open_write_mem(w, buf, sizeof buf, false, NULL)) {
			if (wc[i].emit)
				bvnr_write_version(w, wc[i].maj, wc[i].min);
			bvnr_write_float_unit(w, "t", 64, 12.5, U(wc[i].unit));
			got = bvnr_writer_get_error(w);
		}
		if (got != wc[i].want) {
			fprintf(stderr, "FAIL: writing %s under %s -> %s, expected %s\n",
			        wc[i].unit, wc[i].emit ? "a directive" : "no directive",
			        bvn_error_to_string(got), bvn_error_to_string(wc[i].want));
			failures++;
		}
		bvnr_writer_destroy(w);
	}
}

/* ── the policy engine takes the notation unchanged ─────────────────────── */

static void test_policy_strings(void)
{
	printf("  a profile string works wherever a unit string does...\n");
	/* bvnr_unit_policy_t parses its targets with bvn_parse_unit, so the notation
	 * arrives for free -- and a rule naming ucum:mm[Hg] is satisfied by a
	 * document that writes mmHg, because the comparison is on the parsed unit
	 * and neither spelling survives into it. */
	ASSERT_TRUE(bvn_units_compatible(U("ucum:mm[Hg]"), U("mmHg")),
	            "rule target and document unit agree");
	ASSERT_TRUE(bvn_units_compatible(U("ucum:mmol/L"), U("m~mol/L")),
	            "ucum:mmol/L matches the native spelling");
}

int main(void)
{
	printf("══════════════════════════════════════\n");
	printf("  Bovnar UCUM Profile Test Suite\n");
	printf("══════════════════════════════════════\n\n");

	test_outcomes();
	test_equivalence();
	test_operators();
	test_annotations();
	test_fold();
	test_collisions();
	test_arbitrary();
	test_round_trip();
	test_sweep_round_trip();
	test_to_ucum();
	test_regressions();
	test_metric_flags_and_binary_prefixes();
	test_version_gate();
	test_policy_strings();

	printf("\n%d tests, %d failures\n", tests, failures);
	if (failures == 0)
		printf("ALL TESTS PASSED\n");
	return failures == 0 ? 0 : 1;
}
