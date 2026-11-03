/*
 * SPDX-License-Identifier: MIT
 * SPDX-FileComment: The test vectors below quote OM 2 local names. Those
 * identifiers are the OM authors' and are not covered by the MIT grant
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
 * The OM 2 unit profile (spec 1.2). Specified in doc/11_bovnar_unit_profiles.md.
 *
 * OM is a flat profile like QUDT, and most of what this file pins is the same
 * shape: a local name is one whole token, case is the publisher's, a code with
 * no bovnar unit is refused by cause rather than approximated.
 *
 * What is particular to OM, and what this file exists to defend, is that the
 * COMPOUND targets were derived from OM's own composition rather than read off
 * the local name. "gramPerPetalitre" is g/P~L because OM states a numerator, a
 * denominator and a prefix, not because the name reads that way in English; a
 * table built by reading names would have got the same answer for most rows and
 * a silently different one for the rest.
 *
 * What this file does NOT check: that OM's factors are right. That is
 * check_profile_factors.py's job, which resolves each local name against
 * om-2.0.rdf itself and compares.
 */

#include <math.h>
#include <stdio.h>
#include <string.h>
#include "bovnar.h"
#include "bovnar_si_units.h"

static int failures = 0;
static int tests    = 0;

#define ASSERT_TRUE(cond, msg) do { \
	tests++; \
	if (!(cond)) { \
		fprintf(stderr, "FAIL line %d: %s\n", __LINE__, (msg)); \
		failures++; \
	} \
} while (0)

static value_unit_t U(const char* s)
{
	bool ok = true;
	return bvn_parse_unit((const uint8_t*)s, &ok);
}

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

static void chk_om(const char* native, const char* want)
{
	value_unit_t u = U(native);
	char buf[BVNR_UNIT_STRING_MAX];
	int32_t n = bvn_unit_to_profile("om", u, buf, sizeof(buf));
	tests++;
	if (n < 0 || strcmp(buf, want) != 0) {
		fprintf(stderr, "FAIL: %s -> om:%s, expected om:%s\n",
		        native, n < 0 ? "(none)" : buf, want);
		failures++;
	}
}

/* ── the atoms ──────────────────────────────────────────────────────────── */

static void test_atoms(void)
{
	printf("  om: singular and prefixed units...\n");

	chk_str("om:metre",       "m");
	chk_str("om:kilogram",    "k~g");
	chk_str("om:second-Time", "s");
	chk_str("om:ampere",      "A");
	chk_str("om:kelvin",      "K");
	chk_str("om:mole",        "mol");
	chk_str("om:candela",     "cd");

	/* The named SI units come out as themselves, not as their definitions. OM
	 * defines the joule AS the newton metre (hasUnit, no factor), and a table
	 * that followed that definition blindly would have spelled it "N·m". */
	chk_str("om:joule",  "J");
	chk_str("om:watt",   "W");
	chk_str("om:newton", "N");
	chk_str("om:pascal", "Pa");
	chk_str("om:hertz",  "Hz");

	/* A PrefixedUnit becomes a native prefix, not a separate unit. */
	chk_str("om:millimetre",  "m~m");
	chk_str("om:kilometre",   "k~m");
	chk_str("om:microgram",   "µ~g");
	chk_factor("om:kilometre", 1e3);
	chk_factor("om:micron",    1e-6);

	/* Non-SI units OM defines with an explicit factor. */
	chk_factor("om:litre",            1e-3);
	chk_factor("om:hectare",          1e4);
	chk_factor("om:hour",             3600.0);
	chk_factor("om:day",              86400.0);
	chk_factor("om:tonne",            1e3);
	chk_factor("om:astronomicalUnit", 1.495978707e11);
	chk_str("om:degreeCelsius", "°C");
}

/* ── the compounds, which are the point ─────────────────────────────────── */

static void test_composition(void)
{
	printf("  om: targets built from OM's own structure...\n");

	chk_str("om:metrePerSecond-Time",        "m/s");
	chk_str("om:kilogramPerCubicmetre",      "k~g/m³");
	chk_str("om:newtonMetre",                "N·m");
	chk_str("om:squareMetre",                "m²");
	chk_str("om:cubicMetre",                 "m³");
	chk_str("om:joulePerKelvinKilogram",     "J/K·k~g");
	chk_str("om:kilometrePerHour",           "k~m/h");
	chk_str("om:gramPerPetalitre",           "g/P~L");

	chk_factor("om:kilometrePerHour",     1.0 / 3.6);
	chk_factor("om:kilowattHour",         3.6e6);
	chk_factor("om:squareKilometre",      1e6);
	chk_factor("om:cubicCentimetre",      1e-6);
	chk_factor("om:milligramPerLitre",    1e-3);
	chk_factor("om:kilogramPerCubicmetre", 1.0);

	/*
	 * OM's prefixes multiply the WHOLE unit, so a prefixed compound has the
	 * prefix on one component and the same value either way. attomolar is
	 * atto x molar, and molar is the mole per litre.
	 */
	chk_factor("om:molar",      1e3);       /* mol/L, i.e. 1000 mol/m³ */
	chk_factor("om:attomolar",  1e-15);
}

/* ── flatness ───────────────────────────────────────────────────────────── */

static void test_flatness(void)
{
	printf("  om: a local name is one whole token...\n");

	/* Nothing here decomposes a name. "kilogram" is not kilo + gram to this
	 * parser even though it is to OM, and a name assembled out of parts the
	 * table does carry separately is still a name the table does not have. */
	chk_error("om:kilo",              error_unit_illegal);
	chk_error("om:PerSecond-Time",    error_unit_illegal);
	chk_error("om:metrePerSecond",    error_unit_illegal);
	chk_error("om:kilogram/second",   error_unit_illegal);
	chk_error("om:kilogram.metre",    error_unit_illegal);

	/* Case is OM's own. OM writes "kilogramPerCubicmetre" with a lowercase m in
	 * the denominator and "kilogramPerCubicDecimetre" with a capital one; this
	 * table follows the publisher rather than tidying it. */
	chk_error("om:Metre",                    error_unit_illegal);
	chk_error("om:kilogrampercubicmetre",    error_unit_illegal);
	chk_error("om:kilogramPerCubicMetre",    error_unit_illegal);
	chk_str("om:kilogramPerCubicDecimetre",  "k~g/d~m³");

	/* A full IRI is not a local name (§6 of doc/11, same rule as QUDT). */
	chk_error("om:http://www.ontology-of-units-of-measure.org/resource/om-2/metre",
	          error_unit_illegal);
	chk_error("om:", error_unit_illegal);
}

/* ── refusals, by cause ─────────────────────────────────────────────────── */

static void test_refusals(void)
{
	printf("  om: what is refused, and why...\n");

	/*
	 * OM's year is the GREGORIAN year, 31556952 s. Native yr is the Julian
	 * 31557600 s: 21 ppm apart and dimensionally identical, which is exactly
	 * the disagreement that would never show up in a test that does not check
	 * the number. It is refused, and so is everything OM builds on it.
	 */
	/* om:year became a MAPPING when yr_greg arrived; the distinction it was
	 * refused to protect is asserted directly instead. Everything OM builds on
	 * the year stays refused -- those are compounds, not the unit itself. */
	chk_str("om:year", "yr_greg");
	chk_factor("om:year", 31556952.0);
	/* And everything OM builds ON the year followed it, once yr_greg existed to
	 * build them from. Each is still a different unit from anything on native
	 * yr, which is what the refusal was protecting. */
	chk_str("om:gigayear",           "G~yr_greg");
	chk_str("om:reciprocalYear",     "yr_greg⁻¹");
	chk_str("om:cubicMetrePerYear",  "m³/yr_greg");
	/*
	 * These three kept the Gregorian-year refusal after om:year stopped
	 * carrying it -- the reason outlived the unit it was about, in the same
	 * file that maps the year two lines above. OM composes each from terms that
	 * all map: gigayear x cubicKiloparsec, gigayear x cubicParsec, ppm / year.
	 */
	chk_str("om:gigayearCubicKiloparsec", "G~yr_greg·k~pc³");
	chk_str("om:gigayearCubicParsec",     "G~yr_greg·pc³");
	chk_str("om:partsPerMillionPerYear",  "ppm/yr_greg");
	/*
	 * NOT a fourth. OM's identifier says PerYear and its definition does not:
	 * label "reciprocal parts per million", symbol ppm-1, hasBase
	 * partsPerMillion, hasExponent -1. The mapping follows the definition.
	 */
	chk_str("om:reciprocalPartsPerMillionPerYear", "ppm⁻¹");
	/*
	 * The two that stay refused do so for the reason the neighbouring solarMass
	 * rows always gave -- bovnar has no solar mass -- and not for the year.
	 */
	chk_error("om:solarMass",                        error_unit_profile_unsupported);
	chk_error("om:solarMassPerGigayearCubicParsec",  error_unit_profile_unsupported);
	chk_error("om:solarMassPerGigayearCubicKiloparsec", error_unit_profile_unsupported);

	/* An arbitrary unit is carried by the vocabulary that gives it an identity.
	 * UCUM's [IU] is an opaque base unit here; a second, incommensurable OM one
	 * would mean om:InternationalUnit and ucum:[IU] could not be compared. */
	chk_error("om:InternationalUnit",  error_unit_profile_unsupported);
	chk_error("om:colonyFormingUnit",  error_unit_profile_unsupported);

	/* Money is not a physical quantity; bovnar carries currencies elsewhere. */
	chk_error("om:UnitedStatesDollar", error_unit_profile_unsupported);
	chk_error("om:euro",               error_unit_profile_unsupported);

	/* Dimension one is spelled by omitting the unit, so a code that means
	 * "the number one" has no translation that would not make a count equal a
	 * percentage. */
	chk_error("om:one",   error_unit_profile_unsupported);
	chk_error("om:dozen", error_unit_profile_unsupported);

	/* A ratio of two of the SAME unit is not that code. It says what it is a
	 * ratio OF, and bovnar can spell it, so it maps to the compound rather than
	 * being collapsed onto the generic ratio unit -- which is the distinction
	 * unece.bvnr already draws (3H is k~g/k~g; the mg/kg-shaped codes worth
	 * exactly ppm stay out). This used to be refused alongside om:one. */
	chk_str("om:metrePerMetre",             "m/m");
	chk_str("om:kilogramPerKilogram",       "k~g/k~g");
	chk_str("om:squareMetrePerSquareMetre", "m²/m²");

	/* An affine unit means nothing away from exponent 1 or beside another
	 * component (doc/11 §3.8) -- which is exactly why a degree Celsius appearing
	 * in a compound is the INTERVAL, and the interval is the kelvin. qudt.bvnr
	 * has always read DEG_C-PER-SEC as K/s; these agree with it now instead of
	 * refusing the same construct in the other vocabulary. */
	chk_str("om:degreeCelsiusPerHour",    "K/h");
	chk_str("om:reciprocalDegreeCelsius", "K⁻¹");
	chk_str("om:degreeCelsiusDay",        "K·d");

	/* A unit OM defines without stating a magnitude cannot be carried: OM gives
	 * calorie-15C a dimension and no factor. */
	chk_error("om:calorie-15C", error_unit_profile_unsupported);

	/* And a namespace this build does not have is a THIRD outcome, distinct
	 * from both of the above. */
	chk_error("omm:metre", error_unit_profile_unknown);
}

/* ── writing a unit back as an OM code ──────────────────────────────────── */

static void test_to_om(void)
{
	printf("  om: the reverse direction...\n");

	chk_om("k~g", "kilogram");
	chk_om("m",   "metre");
	chk_om("s",   "second-Time");
	chk_om("L",   "litre");
	chk_om("ha",  "hectare");

	/*
	 * Several OM names are worth the same native unit, and which one is written
	 * back is decided by SHAPE before length. 10 A is OM's abampere, its biot
	 * and its decaampere; the prefixed native target takes the name OM builds
	 * the same way, so the answer is decaampere and not the shorter biot.
	 */
	chk_om("da~A", "decaampere");

	/*
	 * And a composed name never wins a slot: OM's cubicMetrePerSquareMetre is
	 * worth a metre, and writing a metre back as one would say something about
	 * the quantity that the unit does not know.
	 */
	{
		value_unit_t u = U("m");
		char buf[BVNR_UNIT_STRING_MAX];
		int32_t n = bvn_unit_to_profile("om", u, buf, sizeof(buf));
		ASSERT_TRUE(n > 0 && strcmp(buf, "cubicMetrePerSquareMetre") != 0,
		            "the metre must not be written as a composed OM name");
	}

	/* A flat vocabulary spells one whole code, so a compound unit has no OM
	 * form at all -- the same answer QUDT gives, and for the same reason. */
	{
		value_unit_t u = U("m/s^2");
		char buf[BVNR_UNIT_STRING_MAX];
		ASSERT_TRUE(bvn_unit_to_profile("om", u, buf, sizeof(buf)) < 0,
		            "a compound unit has no single OM code");
	}
}

/* ── agreement with the other vocabularies ──────────────────────────────── */

static void test_cross_vocabulary(void)
{
	printf("  om: agrees with the vocabularies already here...\n");

	ASSERT_TRUE(bvn_unit_equal(U("om:kilogram"), U("k~g")),
	            "om:kilogram is the kilogram");
	ASSERT_TRUE(bvn_unit_equal(U("om:kilogram"), U("ucum:kg")),
	            "om:kilogram == ucum:kg");
	ASSERT_TRUE(bvn_unit_equal(U("om:kilogram"), U("qudt:KiloGM")),
	            "om:kilogram == qudt:KiloGM");
	ASSERT_TRUE(bvn_unit_equal(U("om:kilogram"), U("unece:KGM")),
	            "om:kilogram == unece:KGM");
	ASSERT_TRUE(bvn_unit_equal(U("om:metrePerSecond-Time"), U("udunits:m/s")),
	            "om:metrePerSecond-Time == udunits:m/s");
	ASSERT_TRUE(bvn_unit_equal(U("om:pascal"), U("qudt-qk:Pressure")),
	            "om:pascal == the coherent unit of qudt-qk:Pressure");

	/* Convertibility, not just equality: an OM unit is an ordinary unit once
	 * translated, so it converts against a native one. */
	{
		bool ok = true, aff = false;
		double f = bvn_unit_convert_factor(U("om:kilometrePerHour"), U("m/s"),
		                                   &ok, &aff);
		tests++;
		if (!ok || fabs(f - 1.0 / 3.6) > 1e-15) {
			fprintf(stderr, "FAIL: om:kilometrePerHour -> m/s factor %.17g\n", f);
			failures++;
		}
	}
}

/* ── round trip ─────────────────────────────────────────────────────────── */

static void test_round_trip(void)
{
	printf("  om: every spelling re-parses to itself...\n");

	static const char* codes[] = {
		"om:metre", "om:kilogram", "om:hectare", "om:litre", "om:micron",
		"om:kilogramPerCubicmetre", "om:newtonMetre", "om:kilowattHour",
		"om:millimetreOfMercury", "om:degreeCelsius", "om:attomolar",
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

/* The 2026-11-03 review: OM's measurement scales were mapped to K (and the
 * Reaumur scale to the temperature DIFFERENCE), so om:CelsiusScale 20 read as
 * 20 K. A scale is related to its unit by a factor and an offset; it is not a
 * unit to read a value in. */
static void test_scales_are_not_units(void)
{
	chk_error("om:CelsiusScale",    error_unit_profile_unsupported);
	chk_error("om:FahrenheitScale", error_unit_profile_unsupported);
	chk_error("om:KelvinScale",     error_unit_profile_unsupported);
	chk_error("om:RankineScale",    error_unit_profile_unsupported);
	chk_error("om:ReaumurScale",    error_unit_profile_unsupported);
	/* ...and the units of those scales are still the units. */
	chk_str("om:degreeCelsius", "°C");
	char buf[64];
	ASSERT_TRUE(bvn_unit_to_profile("om", U("Δ°Re"), buf, sizeof buf) < 0,
		    "Δ°Re is not written back as a scale");
}

int main(void)
{
	printf("OM 2 unit profile\n");

	test_atoms();
	test_composition();
	test_flatness();
	test_scales_are_not_units();
	test_refusals();
	test_to_om();
	test_cross_vocabulary();
	test_round_trip();

	printf("\n%d tests, %d failures\n", tests, failures);
	if (failures == 0)
		printf("ALL TESTS PASSED\n");
	return failures == 0 ? 0 : 1;
}
