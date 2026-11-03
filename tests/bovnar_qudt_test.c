/*
 * SPDX-License-Identifier: MIT
 * SPDX-FileComment: The test vectors below quote QUDT local names. Those
 * identifiers are QUDT.org's and are not covered by the MIT grant
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
 * The QUDT unit profile and the QUDT quantity-kind profile (spec 1.2).
 * Specified in doc/11_bovnar_unit_profiles.md.
 *
 * Two namespaces, and the second is the interesting one. "qudt:" spells a UNIT
 * and behaves like any other flat profile. "qudt-qk:" spells a KIND OF QUANTITY,
 * which is not a unit at all, and translates to the COHERENT SI UNIT of the
 * kind. What this file defends about that choice is the part that could go
 * wrong quietly: that the coherent unit is the coherent one and not merely a
 * convenient one, and that a kind with no coherent unit is refused rather than
 * approximated.
 *
 * What this file does NOT check, and cannot: that "KiloGM" is really QUDT's
 * kilogram. See the provenance note at the top of src/gendata/qudt.bvnr.
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

/* ── qudt: the unit namespace ───────────────────────────────────────────── */

/*
 * The closing pass matched a flat QUDT code against a SINGLE native unit by
 * multiplier, so a code worth a native COMPOUND fell through to "no native unit
 * of this dimension is a decade away" -- true of any single unit, and false of
 * the expression. The NUM-PER-* family had already settled the modelling
 * question (a count is no unit, so NUM-PER-X is X^-1, and a dozen siblings map
 * that way); these were the ones whose X is itself a compound.
 */
static void test_compounds_the_closing_pass_could_not_see(void)
{
	printf("  qudt: codes worth a native compound...\n");
	chk_str("qudt:NUM-PER-KiloM2",   "k~m⁻²");
	chk_str("qudt:NUM-PER-M2-DAY",   "m⁻²·d⁻¹");
	chk_str("qudt:NUM-PER-HA-YR",    "ha⁻¹·yr⁻¹");
	/* QUDT states 6.28318531, which is 2*pi to nine figures; native rev is
	 * 2*pi rad, so these agree inside the 7.5e-7 the factor proof allows. */
	chk_str("qudt:REV-PER-SEC2",     "rev/s²");
	chk_str("qudt:REV-PER-MIN-SEC",  "rev/min·s");

	/*
	 * Their neighbours stay refused, and for reasons that are now stated rather
	 * than inherited from the generic decade line. Two are QUDT contradicting
	 * itself: NUM-PER-CentiM-KiloYR's name and multiplier say per CENTIMETRE
	 * while its ucumCode says per centimetre SQUARED, and MicroM-PER-MilliL's
	 * name and multiplier say micrometre per millilitre while its label and
	 * ucumCode say SQUARE micron. Three are substance-specific masses whose
	 * analyte lives in a UCUM annotation, which is inert -- mapping them would
	 * silently drop the carbon, the nitrogen, the "dry". One is the US therm,
	 * which native thm_ec is not.
	 */
	chk_error("qudt:NUM-PER-CentiM-KiloYR", error_unit_profile_unsupported);
	chk_error("qudt:MicroM-PER-MilliL",     error_unit_profile_unsupported);
	chk_error("qudt:GM_Carbon-PER-M2-DAY",  error_unit_profile_unsupported);
	chk_error("qudt:GM_Nitrogen-PER-M2-DAY", error_unit_profile_unsupported);
	chk_error("qudt:M2-PER-GM_DRY",         error_unit_profile_unsupported);
	chk_error("qudt:THM_US-PER-HR",         error_unit_profile_unsupported);
	/* And the currencies, which have no dimension to build a compound on. */
	chk_error("qudt:CHF-PER-KiloGM",        error_unit_profile_unsupported);
	chk_error("qudt:CCY_EUR-PER-M2",        error_unit_profile_unsupported);
}

static void test_units(void)
{
	printf("  qudt: unit local names...\n");

	chk_str("qudt:M",         "m");
	chk_str("qudt:KiloGM",    "k~g");
	chk_str("qudt:M-PER-SEC", "m/s");
	chk_str("qudt:DEG_C",     "°C");
	chk_str("qudt:MebiBYTE",  "Mi~B");

	chk_factor("qudt:KiloGM",  1.0);      /* the SI base unit of mass */
	chk_factor("qudt:GM",      1e-3);
	chk_factor("qudt:KiloM",   1e3);
	chk_factor("qudt:M-PER-SEC", 1.0);

	/* Refused, split by cause. */
	chk_error("qudt:Metre",     error_unit_illegal);
	chk_error("qudt:UNITLESS",  error_unit_profile_unsupported);
	chk_error("qudt:USD",       error_unit_profile_unsupported);
	chk_error("qudt:",          error_unit_illegal);

	/* A full IRI is not a local name. Rejecting it is deliberate: the local
	 * name is the identifying part, and a URI would put a second ':' inside a
	 * namespace separator for no gain. */
	chk_error("qudt:http://qudt.org/vocab/unit/M", error_unit_illegal);
}

static void test_flatness(void)
{
	printf("  qudt: a local name is one whole token...\n");

	/*
	 * QUDT's naming is regular enough to tempt a parser into decomposing
	 * "KiloGM" into Kilo + GM, and irregular enough that the parser would be
	 * wrong: "MI" is the mile, not a milli-anything. Nothing in this namespace
	 * decomposes.
	 */
	chk_str("qudt:MI", "mi");
	chk_str("qudt:IN", "in");

	/* The parsec is "PARSEC". QUDT has no "PC" at all, and its "PCA" -- whose
	 * SYMBOL is "pc" -- is the PICA, a typographic length: reading the symbol
	 * rather than the local name is how a length becomes 7e18 times too small.
	 */
	chk_str("qudt:PARSEC", "pc");
	chk_error("qudt:PC", error_unit_illegal);

	/* QUDT's MO is the SYNODAL month (29.53 days); native mo is a twelfth of
	 * the Julian year. Three per cent apart and dimensionally identical. */
	/* MO is QUDT's SYNODAL (lunar) month, 29.53 d; native mo is a twelfth of
	 * the Julian year. It was refused while bovnar had only the second; it
	 * now maps to mo_syn, and the distinction is asserted instead. */
	chk_str("qudt:MO", "mo_syn");
	chk_factor("qudt:MO", 2551442.976);

	/* REV-PER-MIN is an ANGULAR VELOCITY in QUDT -- 2π/60, a revolution being
	 * 2π radians -- not the revolution-counting rpm. */
	chk_str("qudt:REV-PER-MIN", "rev/min");
	chk_factor("qudt:REV-PER-MIN", 6.283185307179586 / 60.0);

	chk_error("qudt:Kilo",        error_unit_illegal);
	chk_error("qudt:KiloM-PER",   error_unit_illegal);
	chk_error("qudt:M/SEC",       error_unit_illegal);
	chk_error("qudt:M.SEC",       error_unit_illegal);
	/* A local name built out of parts this table does carry separately is still
	 * one whole token, and one this table does not have. M2-PER-SEC used to
	 * stand here; it is a row now, so the assertion moved to a name QUDT itself
	 * does not define — the point is the FLAT lookup, not which name it is. */
	chk_error("qudt:M2-PER-SEC3", error_unit_illegal);

	/* Case is QUDT's own, and significant. */
	chk_error("qudt:kilogm", error_unit_illegal);
	chk_error("qudt:m",      error_unit_illegal);
}

/* ── qudt-qk: the quantity-kind namespace ───────────────────────────────── */

static void test_quantity_kinds(void)
{
	printf("  qudt-qk: a kind translates to its COHERENT SI unit...\n");

	/*
	 * The claim: the target is the coherent SI unit of the kind, never a
	 * convenient one. Mass is the KILOGRAM, not the gram -- the coherent SI unit
	 * of mass is the only SI base unit with a prefix in its name, and a table
	 * that "tidied" it to "g" would silently rescale every mass by 1000.
	 */
	chk_str("qudt-qk:Length",                  "m");
	chk_str("qudt-qk:Mass",                    "k~g");
	chk_str("qudt-qk:Time",                    "s");
	chk_str("qudt-qk:ThermodynamicTemperature", "K");
	chk_str("qudt-qk:Velocity",                "m/s");
	chk_str("qudt-qk:Force",                   "N");
	chk_str("qudt-qk:Pressure",                "Pa");
	chk_str("qudt-qk:Power",                   "W");

	chk_factor("qudt-qk:Mass",   1.0);
	chk_factor("qudt-qk:Length", 1.0);
	chk_factor("qudt-qk:Energy", 1.0);
	chk_factor("qudt-qk:Power",  1.0);

	/* Every coherent unit is coherent: its factor to SI is exactly 1. That is
	 * what "coherent" MEANS, and it is the one property this whole namespace
	 * rests on, so it is checked as a property rather than case by case. */
	static const char* kinds[] = {
		"qudt-qk:Length", "qudt-qk:Mass", "qudt-qk:Time",
		"qudt-qk:ElectricCurrent", "qudt-qk:ThermodynamicTemperature",
		"qudt-qk:AmountOfSubstance", "qudt-qk:LuminousIntensity",
		"qudt-qk:Area", "qudt-qk:Volume", "qudt-qk:Velocity",
		"qudt-qk:Acceleration", "qudt-qk:Frequency", "qudt-qk:Force",
		"qudt-qk:Pressure", "qudt-qk:Energy", "qudt-qk:Power",
		"qudt-qk:ElectricCharge", "qudt-qk:Voltage", "qudt-qk:Resistance",
		"qudt-qk:Capacitance", "qudt-qk:Inductance", "qudt-qk:Conductance",
		"qudt-qk:MagneticFlux", "qudt-qk:MagneticFluxDensity",
		"qudt-qk:LuminousFlux", "qudt-qk:Illuminance", "qudt-qk:Activity",
		"qudt-qk:AbsorbedDose", "qudt-qk:DoseEquivalent",
		"qudt-qk:CatalyticActivity", "qudt-qk:Density", "qudt-qk:Torque",
		"qudt-qk:MassFlowRate", "qudt-qk:VolumeFlowRate",
		"qudt-qk:DynamicViscosity", "qudt-qk:KinematicViscosity",
		"qudt-qk:HeatCapacity", "qudt-qk:SpecificHeatCapacity",
		"qudt-qk:ThermalConductivity", "qudt-qk:AmountOfSubstanceConcentration",
	};
	for (size_t i = 0; i < sizeof(kinds) / sizeof(kinds[0]); i++)
		chk_factor(kinds[i], 1.0);

	/* The 2026-11-03 coverage review: refused as "no native unit of this
	 * dimension", though the registry spells each one. The bit densities are
	 * the bit because QUDT lists only bit units for them; the rest follow
	 * their mapped siblings (ParticleFluenceRate, PhotonRadiance,
	 * NumberDensity, InverseAmountOfSubstance, LinearExpansionCoefficient). */
	chk_str("qudt-qk:AreaBitDensity",         "b/m²");
	chk_str("qudt-qk:VolumetricBitDensity",   "b/m³");
	chk_str("qudt-qk:ParticleCurrentDensity", "m⁻²·s⁻¹");
	chk_str("qudt-qk:PhotonLuminance",        "s⁻¹·m⁻²·sr⁻¹");
	chk_str("qudt-qk:VolumetricEntityDensity", "m⁻³");
	chk_str("qudt-qk:MolarDensity",           "mol⁻¹");
	chk_str("qudt-qk:ThermalCoefficientOfLinearExpansion", "K⁻¹");
	chk_factor("qudt-qk:ThermalCoefficientOfLinearExpansion", 1.0);

	/* And one QUDT unit: a percent per decakelvin is 0.001 K^-1. */
	chk_factor("qudt:PERCENT-PER-DecaK", 1e-3);
}

static void test_kind_agrees_with_unit(void)
{
	printf("  qudt-qk: a kind and its unit are the same value_unit_t...\n");

	/* The kind namespace is not a parallel unit model: it lands on exactly the
	 * unit the other namespace would have produced. */
	ASSERT_TRUE(bvn_unit_equal(U("qudt-qk:Mass"),   U("qudt:KiloGM")),
	            "the coherent unit of Mass is the kilogram");
	ASSERT_TRUE(bvn_unit_equal(U("qudt-qk:Length"), U("qudt:M")),
	            "the coherent unit of Length is the metre");
	ASSERT_TRUE(bvn_unit_equal(U("qudt-qk:Velocity"), U("qudt:M-PER-SEC")),
	            "the coherent unit of Velocity is the metre per second");
	ASSERT_TRUE(bvn_unit_equal(U("qudt-qk:Mass"),   U("unece:KGM")),
	            "and it agrees with UNECE too");
	ASSERT_TRUE(bvn_unit_equal(U("qudt-qk:Mass"),   U("ucum:kg")),
	            "and with UCUM");

	/* Kinds that share a dimension share a unit, which is the honest
	 * consequence of translating to a unit: bovnar's dimension vector cannot
	 * tell Energy from Work, and neither can ISO 80000. */
	ASSERT_TRUE(bvn_unit_equal(U("qudt-qk:Energy"), U("qudt-qk:Work")),
	            "energy and work share a coherent unit");
	ASSERT_TRUE(bvn_unit_equal(U("qudt-qk:Speed"), U("qudt-qk:Velocity")),
	            "speed and velocity share a coherent unit");
	ASSERT_TRUE(bvn_units_compatible(U("qudt-qk:Pressure"), U("qudt-qk:Stress")),
	            "pressure and stress are compatible");
}

static void test_kind_refusals(void)
{
	printf("  qudt-qk: a kind with no coherent unit is refused...\n");

	/*
	 * Refusing costs an error message; approximating costs a number. Every
	 * refusal here is a kind whose coherent unit bovnar cannot state:
	 * dimensionless kinds (which are the ABSENCE of a unit, not the unity),
	 * affine scales, and logarithms with no reference level.
	 */
	chk_error("qudt-qk:Dimensionless",      error_unit_profile_unsupported);
	chk_error("qudt-qk:Count",              error_unit_profile_unsupported);
	chk_error("qudt-qk:CelsiusTemperature", error_unit_profile_unsupported);
	chk_error("qudt-qk:LogarithmicRatio",   error_unit_profile_unsupported);
	chk_error("qudt-qk:Currency",           error_unit_profile_unsupported);

	/* A cost is a currency compound, and every applicable unit QUDT lists for
	 * these four is one (CCY_EUR-PER-KiloW-HR, ...). The coherent SI unit of
	 * the bare dimension would read 0.30 EUR/kWh as 0.30 J^-1. */
	chk_error("qudt-qk:CostPerEnergy", error_unit_profile_unsupported);
	chk_error("qudt-qk:CostPerPower",  error_unit_profile_unsupported);
	chk_error("qudt-qk:CostPerArea",   error_unit_profile_unsupported);
	chk_error("qudt-qk:CostPerMass",   error_unit_profile_unsupported);
	/* A data volume in no stated unit: bit or octet is not settled. */
	chk_error("qudt-qk:AreicDataVolume",     error_unit_profile_unsupported);
	chk_error("qudt-qk:VolumicDataQuantity", error_unit_profile_unsupported);

	/* Not a kind at all. */
	chk_error("qudt-qk:Lenght",  error_unit_illegal);   /* a plausible typo */
	chk_error("qudt-qk:KiloGM",  error_unit_illegal);   /* that is a UNIT */
	chk_error("qudt-qk:",        error_unit_illegal);

	/* And the two namespaces do not leak into each other. */
	chk_error("qudt:Length",     error_unit_illegal);
}

/* ── the hyphenated namespace ───────────────────────────────────────────── */

static void test_namespace_syntax(void)
{
	printf("  a hyphen is legal inside a namespace...\n");

	/* "qudt-qk" is the reason bvni_unit_has_profile admits '-'. A hyphen may not
	 * LEAD, so nothing that used to parse changes meaning. */
	ASSERT_TRUE(bvn_unit_valid(U("qudt-qk:Mass")),
	            "a hyphenated namespace parses");
	chk_error("-qk:Mass",  error_unit_illegal);
	chk_error("qudt-:Mass", error_unit_profile_unknown);
	chk_error("QUDT:M",    error_unit_illegal);   /* namespaces are lower case */

	/* An unknown namespace is its own outcome, distinct from a bad code: a
	 * consumer reads it as "this build has no such profile". */
	chk_error("qudt-xx:Mass", error_unit_profile_unknown);
	chk_error("nosuch:M",     error_unit_profile_unknown);
}

/* ── writing back out ───────────────────────────────────────────────────── */

static void test_to_qudt(void)
{
	printf("  emitting a QUDT local name from a native unit...\n");

	char buf[BVNR_UNIT_STRING_MAX];

	ASSERT_TRUE(bvn_unit_to_profile("qudt", U("k~g"), buf, sizeof(buf)) > 0 &&
	            strcmp(buf, "KiloGM") == 0,
	            "k~g should emit as KiloGM");
	ASSERT_TRUE(bvn_unit_to_profile("qudt", U("m"), buf, sizeof(buf)) > 0 &&
	            strcmp(buf, "M") == 0,
	            "m should emit as M");
	ASSERT_TRUE(bvn_unit_to_profile("qudt", U("Mi~B"), buf, sizeof(buf)) > 0 &&
	            strcmp(buf, "MebiBYTE") == 0,
	            "a binary prefix has a QUDT name even though it has no decade");

	/* Flat, so a compound has no spelling -- it has a native one instead. */
	ASSERT_TRUE(bvn_unit_to_profile("qudt", U("m/s"), buf, sizeof(buf)) < 0,
	            "a compound has no flat spelling");

	/*
	 * The canonical local name where QUDT gives a unit several. Shortest-wins
	 * chose the bare, ambiguous ones -- "TON" for the SHORT ton, "PINT" for the
	 * imperial pint, "Gs" for the gauss -- each of which is a QUDT unit and
	 * none of which is the name that says which one it is. A flat profile takes
	 * the first name qudt.bvnr lists instead.
	 */
#define CHK_QUDT(nat, want) do { \
	tests++; \
	if (bvn_unit_to_profile("qudt", U(nat), buf, sizeof(buf)) < 0 || \
		strcmp(buf, (want)) != 0) { \
		fprintf(stderr, "FAIL: %s -> QUDT %s, expected %s\n", \
		        (nat), buf, (want)); \
		failures++; \
	} \
} while (0)
	CHK_QUDT("tn_sh", "TON_SHORT");   /* not the bare TON        */
	CHK_QUDT("tn_l",  "TON_LONG");    /* not TON_UK              */
	CHK_QUDT("pt_uk", "PINT_UK");     /* not the bare PINT       */
	CHK_QUDT("t",     "TON_Metric");  /* not TONNE or MegaGM     */
	CHK_QUDT("G",     "GAUSS");       /* not Gs                  */
	CHK_QUDT("grad",  "GRAD");        /* not GON                 */
	CHK_QUDT("c~Gy",  "CentiGRAY");   /* not RAD_R, which is the rad */
#undef CHK_QUDT

	/* Neither QUDT namespace owns an opaque unit, so neither can spell one. */
	ASSERT_TRUE(bvn_unit_to_profile("qudt", U("ucum:[IU]"), buf,
	                                sizeof(buf)) < 0,
	            "qudt must not spell a UCUM arbitrary atom");
	ASSERT_TRUE(bvn_unit_to_profile("qudt", U("unece:XBX"), buf,
	                                sizeof(buf)) < 0,
	            "qudt must not spell a UNECE package code");
	ASSERT_TRUE(BVN_PROFILE_QUDT_OPAQUE_FIRST > BVN_PROFILE_QUDT_OPAQUE_LAST,
	            "the qudt profile owns no opaque range");
	ASSERT_TRUE(BVN_PROFILE_QUDT_QK_OPAQUE_FIRST >
	            BVN_PROFILE_QUDT_QK_OPAQUE_LAST,
	            "the qudt-qk profile owns no opaque range");
}

/* ── round trip ─────────────────────────────────────────────────────────── */

static void test_round_trip(void)
{
	printf("  round trip...\n");

	static const char* codes[] = {
		"qudt:M", "qudt:KiloM", "qudt:CentiM", "qudt:MilliM", "qudt:MicroM",
		"qudt:M2", "qudt:M3", "qudt:L", "qudt:MilliL", "qudt:HA",
		"qudt:GM", "qudt:KiloGM", "qudt:MilliGM", "qudt:TON_Metric",
		"qudt:SEC", "qudt:MIN", "qudt:HR", "qudt:DAY", "qudt:YR",
		"qudt:K", "qudt:DEG_C", "qudt:DEG_F", "qudt:MOL", "qudt:CD",
		"qudt:HZ", "qudt:KiloHZ", "qudt:N", "qudt:PA", "qudt:KiloPA",
		"qudt:J", "qudt:W", "qudt:KiloW", "qudt:V", "qudt:A", "qudt:OHM",
		"qudt:FARAD", "qudt:WB", "qudt:T", "qudt:H", "qudt:LM", "qudt:LUX",
		"qudt:BQ", "qudt:GRAY", "qudt:SV", "qudt:BAR", "qudt:PSI",
		"qudt:M-PER-SEC", "qudt:M-PER-SEC2", "qudt:KiloM-PER-HR",
		"qudt:N-M", "qudt:MOL-PER-L", "qudt:RAD", "qudt:DEG", "qudt:PERCENT",
		"qudt:BIT", "qudt:BYTE", "qudt:KiloBYTE", "qudt:KibiBYTE",
		"qudt-qk:Length", "qudt-qk:Mass", "qudt-qk:Time", "qudt-qk:Force",
		"qudt-qk:Energy", "qudt-qk:Power", "qudt-qk:Pressure",
		"qudt-qk:Density", "qudt-qk:Torque", "qudt-qk:SpecificHeatCapacity",
		"qudt-qk:ThermalConductivity",
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

int main(void)
{
	printf("QUDT unit and quantity-kind profiles\n");

	test_units();
	test_compounds_the_closing_pass_could_not_see();
	test_flatness();
	test_quantity_kinds();
	test_kind_agrees_with_unit();
	test_kind_refusals();
	test_namespace_syntax();
	test_to_qudt();
	test_round_trip();

	printf("\n%d tests, %d failures\n", tests, failures);
	if (failures == 0)
		printf("ALL TESTS PASSED\n");
	return failures == 0 ? 0 : 1;
}
