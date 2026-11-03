/*
 * SPDX-License-Identifier: MIT
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

#include <stdio.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "bovnar.h"
#include "bovnar_si_units.h"
#include "bovnar_currency.h"
#include "bovnar_dom.h"
#include "bvn_int.h"

static int failures = 0;
static int tests = 0;

#define ASSERT_TRUE(cond, msg) do {                                       \
	tests++;                                                              \
	if (!(cond)) {                                                        \
		fprintf(stderr, "FAIL line %d: %s\n", __LINE__, (msg));           \
		failures++;                                                       \
	}                                                                     \
} while (0)

#define ASSERT_EQ_INT(a, b, msg) do {                                     \
	tests++;                                                              \
	int64_t _a = (int64_t)(a);                                            \
	int64_t _b = (int64_t)(b);                                            \
	if (_a != _b) {                                                       \
		fprintf(stderr, "FAIL line %d: %s\n  got %lld, expected %lld\n",  \
				__LINE__, (msg), (long long)_a, (long long)_b);           \
		failures++;                                                       \
	}                                                                     \
} while (0)

#define ASSERT_STR(a, b, msg) do {                                        \
	tests++;                                                              \
	if (strcmp((a), (b)) != 0) {                                          \
		fprintf(stderr, "FAIL line %d: %s\n  got \"%s\", expected \"%s\"\n", \
				__LINE__, (msg), (a), (b));                               \
		failures++;                                                       \
	}                                                                     \
} while (0)

/*
 * Suite for the reader-side unit policy (bvnr_reader_set_unit_policy): the
 * declarative half of what bvnr_read_flags_t.want_unit does through a callback.
 * Three features share one struct and one code path, so they share one binary:
 * the assertions (require_unit / require_dimension_of), the opaque target list,
 * and the SI normalisation mode.
 *
 * The interesting cases here are the ones where "convert everything that fits"
 * fits too much — a bare number is dimensionally a percentage, a currency is
 * compatible with nothing including itself — and the ones where an ordinary
 * document contains a value with no exact decimal expansion in the unit the
 * caller asked for.
 */

#define POL_MAX_VALUES 48

typedef struct {
	int      n;
	bool     converted[POL_MAX_VALUES];
	char     text[POL_MAX_VALUES][64];    /* conv.text, "" when absent */
	char     unit[POL_MAX_VALUES][64];    /* conv.unit, or the native unit */
	char     native[POL_MAX_VALUES][64];  /* always the document's own unit */
	uint32_t base[POL_MAX_VALUES];
} pol_ctx_t;

static void pol_unit_str(value_unit_t u, char *buf, size_t cap)
{
	if (bvn_unit_to_string(u, buf, cap) < 0)
		snprintf(buf, cap, "<unprintable>");
}

static bool pol_on_verified(void *userdata, bvnr_event_t ev, bvnr_data_t *d)
{
	pol_ctx_t *c = userdata;
	if (ev != ev_data || !d)
		return true;
	if (d->type != token_is_number && d->type != token_is_string &&
	    d->type != token_is_array_number && d->type != token_is_array_string)
		return true;
	if (!bvn_type_is_numeric(d->value_type))
		return true;
	if (c->n >= POL_MAX_VALUES)
		return true;
	int i = c->n++;
	c->converted[i] = d->converted;
	c->base[i]      = d->converted ? d->conv.base : 0u;
	pol_unit_str(d->value_unit, c->native[i], sizeof c->native[i]);
	pol_unit_str(d->converted ? d->conv.unit : d->value_unit,
		     c->unit[i], sizeof c->unit[i]);
	c->text[i][0] = '\0';
	if (d->converted && d->conv.text) {
		size_t n = d->conv.length < sizeof c->text[i] - 1
			 ? d->conv.length : sizeof c->text[i] - 1;
		memcpy(c->text[i], d->conv.text, n);
		c->text[i][n] = '\0';
	}
	return true;
}

/* Drive one document through a reader carrying `policy`, collecting every
 * numeric value the verified callback sees. */
static void run_policy(const char *payload, const bvnr_unit_policy_t *policy,
		       pol_ctx_t *ctx, bool expect_ok, error_code_t expect_err)
{
	bvnr_read_flags_t flags;
	memset(&flags, 0, sizeof(flags));
	flags.userdata    = ctx;
	flags.on_verified = pol_on_verified;

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "policy: reader_create");
	if (!r) return;
	ASSERT_TRUE(bvnr_reader_set_unit_policy(r, policy), "policy: set_unit_policy");

	bool opened = bvnr_open_read_mem(r, payload, (uint32_t)strlen(payload),
					 NULL, 0, &flags);
	ASSERT_TRUE(opened, "policy: open_read_mem");
	bool ok = opened && bvnr_read(r);
	error_code_t err = bvnr_reader_get_error(r);
	ASSERT_EQ_INT(ok, expect_ok, "policy: read result matches expectation");
	ASSERT_EQ_INT(err, expect_err, "policy: error code matches expectation");
	bvnr_reader_destroy(r);
}

/* The same, but through the want_unit HOOK rather than a policy. The two paths
 * are deliberately different where a target cannot be applied -- a policy names
 * a preference and delivers the value untouched, a hook names a demand and the
 * parse stops -- so the hook's error codes need their own driver. */
static const char *g_hook_want;

static bool pol_want_unit(void *ud, const bvnr_data_t *d,
			  value_unit_t *want, uint32_t *want_base)
{
	(void)ud; (void)d;
	bool ok = false;
	*want = bvn_parse_unit((const uint8_t *)g_hook_want, &ok);
	if (!ok) return false;
	*want_base = 0u;
	return true;
}

static void run_hook(const char *payload, const char *want, pol_ctx_t *ctx,
		     bool expect_ok, error_code_t expect_err)
{
	g_hook_want = want;
	bvnr_read_flags_t flags;
	memset(&flags, 0, sizeof(flags));
	flags.userdata    = ctx;
	flags.on_verified = pol_on_verified;
	flags.want_unit   = pol_want_unit;

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "hook: reader_create");
	if (!r) return;
	bool opened = bvnr_open_read_mem(r, payload, (uint32_t)strlen(payload),
					 NULL, 0, &flags);
	ASSERT_TRUE(opened, "hook: open_read_mem");
	bool ok = opened && bvnr_read(r);
	error_code_t err = bvnr_reader_get_error(r);
	ASSERT_EQ_INT(ok, expect_ok, "hook: read result matches expectation");
	ASSERT_EQ_INT(err, expect_err, "hook: error code matches expectation");
	bvnr_reader_destroy(r);
}

/* ── option 1: the opaque target list ────────────────────────────────────── */

/*
 * The reference's running document, deliberately awkward: mixed dimensions,
 * an affine scale, a bare ratio, a prefixed currency. Every value that a target
 * covers is converted; every value that none covers is delivered untouched,
 * with data->converted as the only signal telling the two apart.
 */
static void test_targets_mixed_document(void)
{
	static const bvnr_unit_target_t targets[] = {
		{ "°C", 0 }, { "m", 0 },
	};
	bvnr_unit_policy_t p = {0};
	p.targets     = targets;
	p.num_targets = 2;

	pol_ctx_t c = {0};
	run_policy(
		".inlet_temp     = <float:64,°F>      212.0;\n"
		".tank_level     = <float:64,in>      12.0;\n"
		".duty_cycle     = <float:64,%>       35.0;\n"
		".spare_capacity = <float:64>         0.25;\n"
		".unit_price     = <float:64,k~$USD>  5.0;\n",
		&p, &c, true, error_none);

	ASSERT_EQ_INT(c.n, 5, "targets: five numeric values seen");
	if (c.n != 5) return;
	ASSERT_TRUE(c.converted[0], "targets: °F matched the °C target");
	ASSERT_STR(c.text[0], "100", "targets: 212 °F -> 100 °C exact");
	ASSERT_TRUE(c.converted[1], "targets: in matched the m target");
	ASSERT_STR(c.text[1], "0.3048", "targets: 12 in -> 0.3048 m exact");
	ASSERT_TRUE(!c.converted[2], "targets: % matches no target");
	ASSERT_TRUE(!c.converted[3], "targets: a bare number matches no target");
	ASSERT_TRUE(!c.converted[4], "targets: a currency matches no length/temp");
}

/* "First compatible wins" is the whole selection algorithm, so the order of the
 * list is semantics, not presentation. */
static void test_target_order_is_significant(void)
{
	static const bvnr_unit_target_t m_first[]  = { { "m", 0 },  { "k~m", 0 } };
	static const bvnr_unit_target_t km_first[] = { { "k~m", 0 }, { "m", 0 }  };
	bvnr_unit_policy_t p = {0};
	pol_ctx_t c = {0};

	p.targets = m_first; p.num_targets = 2;
	run_policy(".d = 5000 m;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "order: one value");
	ASSERT_TRUE(c.n == 1 && !c.converted[0],
		    "order: m target selected first, value already in m");

	memset(&c, 0, sizeof c);
	p.targets = km_first; p.num_targets = 2;
	run_policy(".d = 5000 m;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "order: one value");
	if (c.n == 1) {
		ASSERT_TRUE(c.converted[0], "order: k~m target selected first");
		ASSERT_STR(c.text[0], "5", "order: 5000 m -> 5 km");
	}
}

/* A target carries its own output base, so one policy can do the unit and the
 * base in a single step. */
static void test_target_output_base(void)
{
	static const bvnr_unit_target_t targets[] = { { "m", 16 } };
	bvnr_unit_policy_t p = {0};
	p.targets = targets; p.num_targets = 1;

	pol_ctx_t c = {0};
	run_policy(".d = 5 k~m;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "base: one value");
	if (c.n != 1) return;
	ASSERT_TRUE(c.converted[0], "base: converted");
	ASSERT_STR(c.text[0], "1388", "base: 5000 m rendered in base 16");
	ASSERT_EQ_INT(c.base[0], 16, "base: reported output base");
}

/*
 * The hazard that decides the whole selector design. A value with no unit is
 * dimensionally compatible with % and with ppm — bvn_units_compatible says so —
 * so a policy naming "%" would convert a bare 0.25 into 25, and one naming
 * no_unit would turn 35 % into 0.35. Both directions are fenced off.
 */
static void test_unitless_is_fenced_from_the_ratio_units(void)
{
	static const bvnr_unit_target_t pct[]  = { { "%", 0 } };
	static const bvnr_unit_target_t bare[] = { { "no_unit", 0 } };
	bvnr_unit_policy_t p = {0};
	pol_ctx_t c = {0};

	p.targets = pct; p.num_targets = 1;
	run_policy(".spare = <float:64> 0.25;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "fence: one value");
	ASSERT_TRUE(c.n == 1 && !c.converted[0],
		    "fence: a bare 0.25 is NOT delivered as 25 %");

	memset(&c, 0, sizeof c);
	p.targets = bare; p.num_targets = 1;
	run_policy(".duty = <float:64,%> 35.0;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "fence: one value");
	ASSERT_TRUE(c.n == 1 && !c.converted[0],
		    "fence: 35 % is NOT delivered as a bare 0.35");

	/* ...while a genuine ratio-to-ratio conversion still works: both carry a
	 * unit, so the fence never comes into it. */
	static const bvnr_unit_target_t ppm[] = { { "ppm", 0 } };
	memset(&c, 0, sizeof c);
	p.targets = ppm; p.num_targets = 1;
	run_policy(".duty = <float:64,%> 35.0;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "fence: one value");
	if (c.n == 1) {
		ASSERT_TRUE(c.converted[0], "fence: % -> ppm is a real conversion");
		ASSERT_STR(c.text[0], "350000", "fence: 35 % -> 350000 ppm");
	}
}

/*
 * A currency carries no dimension by design, so bvn_units_compatible calls it
 * incompatible with itself and a selector built on that predicate would decline
 * a conversion the engine performs correctly. The policy screens with
 * bvn_units_convertible instead, which is what keeps this working.
 */
static void test_currency_prefix_delta_is_selectable(void)
{
	static const bvnr_unit_target_t usd[] = { { "$USD", 0 } };
	bvnr_unit_policy_t p = {0};
	p.targets = usd; p.num_targets = 1;

	pol_ctx_t c = {0};
	run_policy(".price = <float:64,k~$USD> 5.0;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "currency: one value");
	if (c.n != 1) return;
	ASSERT_TRUE(c.converted[0], "currency: k~$USD -> $USD selected");
	ASSERT_STR(c.text[0], "5000", "currency: 5 k$USD -> 5000 $USD");
}

/* A target the value cannot reach at all leaves it alone rather than aborting:
 * the policy names what it wants, not what the document must be. */
static void test_unmatched_target_is_not_an_error(void)
{
	static const bvnr_unit_target_t sec[] = { { "s", 0 } };
	bvnr_unit_policy_t p = {0};
	p.targets = sec; p.num_targets = 1;

	pol_ctx_t c = {0};
	run_policy(".d = 5 k~m;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "unmatched: one value");
	ASSERT_TRUE(c.n == 1 && !c.converted[0], "unmatched: delivered untouched");
}

/* ── exactness ───────────────────────────────────────────────────────────── */

/*
 * 42 km/h is 35/3 m/s: exact as a rational, with no terminating expansion in
 * base 10. The default is the want_unit contract — refuse rather than round.
 */
static void test_nonterminating_default_is_error(void)
{
	static const bvnr_unit_target_t ms[] = { { "m/s", 0 } };
	bvnr_unit_policy_t p = {0};
	p.targets = ms; p.num_targets = 1;

	pol_ctx_t c = {0};
	run_policy(".v = <float:64,km/h> 42.0;", &p, &c, false, error_unit_inexact);
}

/* bvnr_inexact_leave hands that value over in its native unit instead. It is
 * the mode a blanket policy needs, and `converted` is how a consumer tells. */
static void test_nonterminating_leave_delivers_native(void)
{
	static const bvnr_unit_target_t ms[] = { { "m/s", 0 } };
	bvnr_unit_policy_t p = {0};
	p.targets    = ms;
	p.num_targets = 1;
	p.on_inexact = bvnr_inexact_leave;

	pol_ctx_t c = {0};
	run_policy(".v = <float:64,km/h> 42.0;\n.w = <float:64,km/h> 36.0;\n",
		   &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 2, "inexact_leave: two values");
	if (c.n != 2) return;
	ASSERT_TRUE(!c.converted[0], "inexact_leave: 42 km/h left native");
	ASSERT_STR(c.native[0], "k~m/h", "inexact_leave: native unit intact");
	/* 36 km/h IS 10 m/s exactly — the mode steps over the value it cannot
	 * deliver, not over the target. */
	ASSERT_TRUE(c.converted[1], "inexact_leave: 36 km/h still converts");
	ASSERT_STR(c.text[1], "10", "inexact_leave: 36 km/h -> 10 m/s");
}

/* An irrational factor (° -> rad is π-based) aborts by default... */
static void test_irrational_default_is_error(void)
{
	static const bvnr_unit_target_t rad[] = { { "rad", 0 } };
	bvnr_unit_policy_t p = {0};
	p.targets     = rad;
	p.num_targets = 1;

	pol_ctx_t c = {0};
	run_policy(".a = 90 °;", &p, &c, false, error_unit_inexact);
}

/*
 * ...and is stepped over under bvnr_inexact_leave, like every other value the
 * conversion cannot deliver exactly. The alternative reading — that an
 * irrational factor is special because there is no rational to hand over —
 * belongs to want_unit_allow_nonterminating, whose fallback IS the rational.
 * This flag's fallback is the native value, and that works for an irrational
 * factor exactly as well as for a non-terminating one.
 */
static void test_irrational_is_left_under_leave(void)
{
	static const bvnr_unit_target_t rad[] = { { "rad", 0 } };
	bvnr_unit_policy_t p = {0};
	p.targets     = rad;
	p.num_targets = 1;
	p.on_inexact  = bvnr_inexact_leave;

	pol_ctx_t c = {0};
	run_policy(".a = 90 °;\n.b = 2 rad;\n", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 2, "irrational: two values");
	if (c.n != 2) return;
	ASSERT_TRUE(!c.converted[0], "irrational: 90 ° left in degrees");
	ASSERT_STR(c.native[0], "°", "irrational: native unit intact");
	ASSERT_TRUE(!c.converted[1], "irrational: 2 rad already in the target unit");
}

/*
 * The work limit is a property of the value, not of the caller's configuration,
 * so bvnr_inexact_leave steps over it too: `1e-9800` is seven characters that
 * expand to 9800 digits, and refusing to spend that is not a reason to reject
 * the document around it.
 */
static void test_work_limit_is_left_under_leave(void)
{
	bvnr_unit_policy_t p = {0};
	p.normalise  = bvnr_normalise_si;
	p.on_inexact = bvnr_inexact_leave;

	pol_ctx_t c = {0};
	run_policy(".a = <float:64,in> 1e-9800;\n.b = <float:64,in> 12.0;\n",
		   &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 2, "work limit: two values");
	if (c.n != 2) return;
	ASSERT_TRUE(!c.converted[0], "work limit: absurd exponent left alone");
	ASSERT_TRUE(c.converted[1], "work limit: the next value still converts");
	ASSERT_STR(c.text[1], "0.3048", "work limit: 12 in -> 0.3048 m");
}

/* ...and aborts by default, which is the behaviour that keeps an untrusted
 * document from choosing how much CPU the reader spends. */
static void test_work_limit_default_is_error(void)
{
	bvnr_unit_policy_t p = {0};
	p.normalise = bvnr_normalise_si;

	pol_ctx_t c = {0};
	run_policy(".a = <float:64,in> 1e-9800;", &p, &c, false,
		   error_value_out_of_range);
}

/*
 * bvnr_inexact_leave belongs to the policy, not to the reader: a want_unit hook
 * names one target for one value deliberately, and keeps the strict
 * all-or-nothing contract it was documented with even when a policy sharing the
 * reader is set to step over refusals.
 */
static bool strict_hook(void *userdata, const bvnr_data_t *data,
			value_unit_t *want, uint32_t *want_base)
{
	(void)userdata; (void)data;
	bool ok = false;
	*want      = bvn_parse_unit((const uint8_t *)"m/s", &ok);
	*want_base = 0u;
	return ok;
}

static void test_hook_keeps_the_strict_contract(void)
{
	bvnr_unit_policy_t p = {0};
	p.normalise  = bvnr_normalise_si;
	p.on_inexact = bvnr_inexact_leave;

	pol_ctx_t c = {0};
	bvnr_read_flags_t flags;
	memset(&flags, 0, sizeof(flags));
	flags.userdata  = &c;
	flags.on_verified = pol_on_verified;
	flags.want_unit = strict_hook;

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "strict hook: reader_create");
	if (!r) return;
	ASSERT_TRUE(bvnr_reader_set_unit_policy(r, &p), "strict hook: set policy");
	const char *doc = ".v = <float:64,k~m/h> 42.0;";
	bool ok = bvnr_open_read_mem(r, doc, (uint32_t)strlen(doc), NULL, 0, &flags)
		  && bvnr_read(r);
	ASSERT_TRUE(!ok, "strict hook: the hook's conversion still aborts");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_unit_inexact,
		      "strict hook: error_unit_inexact");
	bvnr_reader_destroy(r);
}

/* ── option 3: SI normalisation ──────────────────────────────────────────── */

static void test_normalise_si(void)
{
	bvnr_unit_policy_t p = {0};
	p.normalise = bvnr_normalise_si;

	pol_ctx_t c = {0};
	run_policy(
		".len   = <float:64,in>   12.0;\n"
		".temp  = <float:64,°F>   212.0;\n"
		".dist  = <float:64,k~m>  5.0;\n"
		".mass  = <float:64,g>    5.0;\n",
		&p, &c, true, error_none);

	ASSERT_EQ_INT(c.n, 4, "normalise: four values");
	if (c.n != 4) return;
	ASSERT_TRUE(c.converted[0], "normalise: in converted");
	ASSERT_STR(c.text[0], "0.3048", "normalise: 12 in -> 0.3048 m");
	ASSERT_STR(c.unit[0], "m", "normalise: target unit is m");
	ASSERT_TRUE(c.converted[1], "normalise: °F converted");
	ASSERT_STR(c.text[1], "373.15", "normalise: 212 °F -> 373.15 K");
	ASSERT_STR(c.unit[1], "K", "normalise: target unit is K");
	ASSERT_TRUE(c.converted[2], "normalise: k~m converted");
	ASSERT_STR(c.text[2], "5000", "normalise: 5 km -> 5000 m");
	/* Mass normalises to the kilogram, the SI base unit — not to the gram the
	 * symbol table is built around. */
	ASSERT_TRUE(c.converted[3], "normalise: g converted");
	ASSERT_STR(c.text[3], "0.005", "normalise: 5 g -> 0.005 kg");
	ASSERT_STR(c.unit[3], "k~g", "normalise: target unit is k~g");
}

/*
 * Everything dimensionless is left exactly as written. Normalising a ratio would
 * silently restate 35 % as 0.35, and an angle would need the irrational factor
 * between ° and rad — so bvn_unit_si_normal_form refuses the lot, and the mode
 * never has to carry a hand-maintained skip list.
 */
static void test_normalise_leaves_dimensionless_alone(void)
{
	bvnr_unit_policy_t p = {0};
	p.normalise  = bvnr_normalise_si;
	p.on_inexact = bvnr_inexact_leave;

	pol_ctx_t c = {0};
	run_policy(
		".ratio = <float:64,%>     35.0;\n"
		".gain  = <float:64,dB>    3.0;\n"
		".acid  = <float:64,pH>    7.2;\n"
		".angle = <float:64,°>     90.0;\n"
		".money = <float:64,$USD>  5.0;\n"
		".bare  = <float:64>       0.25;\n",
		&p, &c, true, error_none);

	ASSERT_EQ_INT(c.n, 6, "normalise: six dimensionless values");
	if (c.n != 6) return;
	for (int i = 0; i < 6; i++)
		ASSERT_TRUE(!c.converted[i],
			    "normalise: dimensionless value left as written");
}

/*
 * The photometric units are why bvn_unit_si_normal_form checks its own answer.
 * lm, lx and ph each carry the steradian's quantity kind, which no SI dimension
 * vector can express — so a normal form rebuilt from dimensions alone comes back
 * as cd and cd/m², which is luminous INTENSITY where the value was luminous
 * FLUX. bvn_unit_convert_rational refuses those pairs, correctly, and before the
 * self-check a document containing a lumen aborted the moment normalisation was
 * switched on.
 */
static void test_normalise_refuses_a_form_it_cannot_convert(void)
{
	bvnr_unit_policy_t p = {0};
	p.normalise = bvnr_normalise_si;

	pol_ctx_t c = {0};
	run_policy(
		".flux  = <float:64,lm> 800.0;\n"
		".illum = <float:64,lx> 500.0;\n"
		".phot  = <float:64,ph> 2.0;\n"
		".dist  = <float:64,k~m> 5.0;\n",
		&p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 4, "photometric: four values, none rejected");
	if (c.n != 4) return;
	ASSERT_TRUE(!c.converted[0], "photometric: lm left alone, not made cd");
	ASSERT_TRUE(!c.converted[1], "photometric: lx left alone");
	ASSERT_TRUE(!c.converted[2], "photometric: ph left alone");
	ASSERT_TRUE(c.converted[3], "photometric: an ordinary length still converts");
}

/*
 * A high exponent is not a reason to have no SI form.
 *
 * Two places kept a ±9 bound long after unit_exponent_t's range grew to ±100:
 * bvn_unit_si_normal_form refused any dimension exponent past 9, and
 * bvn_unit_reduce dropped the component instead of carrying it. Neither
 * reported an error — the first said "this unit has no SI form", the second
 * handed back a DIMENSIONLESS unit — so a normalising policy quietly left every
 * such value as written, and a formatter under BVN_UNIT_REDUCE serialised a
 * volume as a plain number.
 *
 * k~m^10 is the case in one line: its SI form is m^10, its factor is exactly
 * 10^30, and every part of that is representable.
 */
static void test_normalise_high_exponents(void)
{
	bvnr_unit_policy_t p = {0};
	p.normalise  = bvnr_normalise_si;
	p.on_inexact = bvnr_inexact_leave;

	pol_ctx_t c = {0};
	run_policy(
		".a = <float:64,k~m^10> 1.0;\n"
		".b = <float:64,m^10> 1.0;\n"
		".c = <float:64,k~m^100> 1.0;\n",
		&p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 3, "high exponents: three values, none rejected");
	if (c.n != 3) return;
	ASSERT_TRUE(c.converted[0], "k~m^10 normalises");
	ASSERT_STR(c.unit[0], "m\xc2\xb9\xe2\x81\xb0", "k~m^10 -> m^10");
	ASSERT_TRUE(!c.converted[1], "m^10 is already its own SI form");
	ASSERT_TRUE(c.converted[2], "k~m^100 normalises too — 100 is the bound");
}

/*
 * An affine scale inside a compound is dimensionally impeccable and completely
 * unconvertible: `s/°C` has the dimensions of `s/K`, but °C means nothing at an
 * exponent other than 1, so the engine refuses the pair — correctly. Both halves
 * of the policy have to survive that. Normalisation must not offer a form it
 * cannot reach, and a TARGET the caller named explicitly must not abort the
 * document either: the value is simply delivered in its own unit, like any value
 * no rule could be applied to.
 */
static void test_affine_in_compound_never_aborts(void)
{
	bvnr_unit_policy_t p = {0};
	p.normalise = bvnr_normalise_si;

	pol_ctx_t c = {0};
	run_policy(".rate = <float:64,s/°C> 5.0;\n.d = <float:64,k~m> 5.0;\n",
		   &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 2, "affine compound: two values, none rejected");
	if (c.n == 2) {
		ASSERT_TRUE(!c.converted[0], "affine compound: s/°C left alone");
		ASSERT_TRUE(c.converted[1], "affine compound: the length still converts");
	}

	/* ...and through the target list, where the screen cannot see it coming. */
	static const bvnr_unit_target_t sk[] = { { "s/K", 0 } };
	memset(&c, 0, sizeof c);
	memset(&p, 0, sizeof p);
	p.targets = sk; p.num_targets = 1;
	run_policy(".rate = <float:64,s/°C> 5.0;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "affine compound: target list does not abort");
	if (c.n == 1)
		ASSERT_TRUE(!c.converted[0], "affine compound: delivered untouched");

	/*
	 * THE HOOK IS THE OTHER HALF, and it must fail with the code the documents
	 * promise. A target the CALLER named is not a preference the way a policy's
	 * is, so it aborts -- and doc/05 §12.4 and doc/08 §1.10 both say the abort
	 * is error_unit_mismatch.
	 *
	 * It was error_value_out_of_range. The reader told the two failures that
	 * pass bvn_units_convertible and fail the conversion apart by asking that
	 * screen again, and the screen passes an affine compound BY DESIGN -- so
	 * `°C/h -> K/h` was reported as the value being out of range, about a
	 * document whose value is 1.0. bvni_unit_affine_misplaced asks where the
	 * affine component sits instead, which magnitude cannot confuse.
	 */
	static const struct { const char *doc; const char *want; error_code_t err; }
	hook_cases[] = {
		{ ".a = <float:64,°C/h> 1.0;",  "K/h",  error_unit_mismatch },
		{ ".a = <float:64,°C·m> 1.0;",  "K·m",  error_unit_mismatch },
		{ ".a = <float:64,°C²> 1.0;",   "K²",   error_unit_mismatch },
		{ ".a = <float:64,s/°C> 1.0;",  "s/K",  error_unit_mismatch },
		/* the affine unit in the TARGET, not the value */
		{ ".a = <float:64,K/h> 1.0;",   "°C/h", error_unit_mismatch },
		/* a lone affine at exponent 1 is the shape that DOES convert */
		{ ".a = <float:64,°C> 20.0;",   "K",    error_none },
		/* and the capacity refusal keeps its own code: the units agree and the
		 * factor needs 10^12000, which is not a statement about the units */
		{ ".a = <float:64,Q~m¹⁰⁰·Q~g¹⁰⁰> 1.0;", "q~m¹⁰⁰·q~g¹⁰⁰",
		  error_value_out_of_range },
	};
	for (size_t i = 0; i < sizeof hook_cases / sizeof hook_cases[0]; i++) {
		pol_ctx_t hc = {0};
		run_hook(hook_cases[i].doc, hook_cases[i].want, &hc,
			 hook_cases[i].err == error_none, hook_cases[i].err);
	}
}

/*
 * Some units have an SI form but only reach it through an irrational factor —
 * the parsec, the oersted, the water-hardness scales. Under the default they
 * abort like any other inexact conversion; under bvnr_inexact_leave the mode
 * steps over them, which is what makes "normalise this document" usable on a
 * document the caller has not audited unit by unit.
 */
static void test_normalise_irrational_factors(void)
{
	bvnr_unit_policy_t p = {0};
	p.normalise = bvnr_normalise_si;

	pol_ctx_t c = {0};
	run_policy(".d = <float:64,pc> 1.0;", &p, &c, false, error_unit_inexact);

	memset(&c, 0, sizeof c);
	p.on_inexact = bvnr_inexact_leave;
	run_policy(".d = <float:64,pc> 1.0;\n.h = <float:64,°dH> 8.0;\n"
		   ".m = <float:64,k~m> 5.0;\n", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 3, "irrational normalise: three values");
	if (c.n != 3) return;
	ASSERT_TRUE(!c.converted[0], "irrational normalise: parsec left alone");
	ASSERT_TRUE(!c.converted[1], "irrational normalise: °dH left alone");
	ASSERT_TRUE(c.converted[2], "irrational normalise: the length still converts");
}

/* A value already in its SI normal form is not converted to itself: the mode
 * reports work it did, and doing none is a valid outcome. */
static void test_normalise_leaves_si_values_alone(void)
{
	bvnr_unit_policy_t p = {0};
	p.normalise = bvnr_normalise_si;

	pol_ctx_t c = {0};
	run_policy(".d = 5 m;\n.t = 25 s;\n", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 2, "normalise: two values");
	if (c.n != 2) return;
	ASSERT_TRUE(!c.converted[0], "normalise: 5 m already normal");
	ASSERT_TRUE(!c.converted[1], "normalise: 25 s already normal");
}

/* Targets are consulted before the normalisation fallback, so one policy can
 * say "speeds in km/h, everything else in SI". */
static void test_targets_outrank_normalise(void)
{
	static const bvnr_unit_target_t kmh[] = { { "k~m/h", 0 } };
	bvnr_unit_policy_t p = {0};
	p.targets     = kmh;
	p.num_targets = 1;
	p.normalise   = bvnr_normalise_si;

	pol_ctx_t c = {0};
	run_policy(".v = <float:64,m/s> 10.0;\n.len = <float:64,in> 12.0;\n",
		   &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 2, "ladder: two values");
	if (c.n != 2) return;
	ASSERT_TRUE(c.converted[0], "ladder: speed took the target, not SI");
	ASSERT_STR(c.text[0], "36", "ladder: 10 m/s -> 36 km/h");
	ASSERT_TRUE(c.converted[1], "ladder: length fell through to normalise");
	ASSERT_STR(c.text[1], "0.3048", "ladder: 12 in -> 0.3048 m");
}

/* The normalisation fallback has its own output base. */
static void test_normalise_output_base(void)
{
	bvnr_unit_policy_t p = {0};
	p.normalise = bvnr_normalise_si;
	p.base      = 16;

	pol_ctx_t c = {0};
	run_policy(".d = 5 k~m;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "normalise base: one value");
	if (c.n != 1) return;
	ASSERT_STR(c.text[0], "1388", "normalise base: 5000 m in base 16");
	ASSERT_EQ_INT(c.base[0], 16, "normalise base: reported");
}

/* ── option 4: the assertions ────────────────────────────────────────────── */

static void test_require_unit(void)
{
	bvnr_unit_policy_t p = {0};
	p.require_unit = true;

	pol_ctx_t c = {0};
	run_policy(".a = 5 m;\n.b = 25 °C;\n", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 2, "require_unit: a fully annotated document passes");

	/* A value with no unit parameter at all... */
	memset(&c, 0, sizeof c);
	run_policy(".a = 5 m;\n.b = 7;\n", &p, &c, false, error_unit_mismatch);

	/* ...and one that wrote no_unit explicitly. Both are bare numbers; the
	 * requirement is about what the value carries, not how it was spelled. */
	memset(&c, 0, sizeof c);
	run_policy(".a = 5 m;\n.b = <uint:32,no_unit> 7;\n",
		   &p, &c, false, error_unit_mismatch);
}

static void test_require_dimension(void)
{
	static const char *lengths[] = { "m" };
	bvnr_unit_policy_t p = {0};
	p.require_dimension_of     = lengths;
	p.num_require_dimension_of = 1;

	/* Any length in any unit satisfies it — that is the point of asking about
	 * the dimension rather than the unit. */
	pol_ctx_t c = {0};
	run_policy(".a = 5 k~m;\n.b = 12 in;\n.c = 3 mi;\n",
		   &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 3, "require_dimension: three lengths pass");

	memset(&c, 0, sizeof c);
	run_policy(".a = 5 k~m;\n.b = 25 °C;\n", &p, &c, false, error_unit_mismatch);

	/* Several requirements: a value must satisfy at least one. */
	static const char *len_or_temp[] = { "m", "K" };
	memset(&c, 0, sizeof c);
	p.require_dimension_of     = len_or_temp;
	p.num_require_dimension_of = 2;
	run_policy(".a = 5 k~m;\n.b = 25 °C;\n", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 2, "require_dimension: length or temperature passes");
}

/*
 * The same fence as the conversion side, for the same reason: a bare number is
 * dimensionally a percentage, so a requirement of "%" would quietly accept an
 * unannotated value as a ratio. It has to name no_unit to allow one.
 */
static void test_require_dimension_fences_unitless(void)
{
	static const char *pct[]  = { "%" };
	static const char *bare[] = { "no_unit" };
	bvnr_unit_policy_t p = {0};
	p.require_dimension_of     = pct;
	p.num_require_dimension_of = 1;

	pol_ctx_t c = {0};
	run_policy(".x = <float:64> 0.25;", &p, &c, false, error_unit_mismatch);

	memset(&c, 0, sizeof c);
	p.require_dimension_of = bare;
	run_policy(".x = <float:64> 0.25;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "require_dimension: no_unit admits a bare number");
}

/*
 * Assertions are evaluated on the unit the DOCUMENT wrote, before conversion.
 * Were it the other way round the check would be near-tautological: the value
 * would be in the unit the policy just asked for. Here the requirement is a
 * temperature and the target is a temperature, but the document is a length —
 * so it must fail, and it must fail on the length.
 */
static void test_assertions_see_the_native_unit(void)
{
	static const bvnr_unit_target_t temp[] = { { "K", 0 } };
	static const char *temps[] = { "K" };
	bvnr_unit_policy_t p = {0};
	p.targets                  = temp;
	p.num_targets              = 1;
	p.require_dimension_of     = temps;
	p.num_require_dimension_of = 1;

	pol_ctx_t c = {0};
	run_policy(".t = 25 °C;\n.d = 5 m;\n", &p, &c, false, error_unit_mismatch);
	/* The temperature was seen and converted before the length was rejected. */
	ASSERT_EQ_INT(c.n, 1, "assertions: the temperature came through first");
	if (c.n == 1)
		ASSERT_STR(c.text[0], "298.15", "assertions: 25 °C -> 298.15 K");
}

/* ── the ladder, reader lifecycle, and argument checking ─────────────────── */

static bool ladder_hook(void *userdata, const bvnr_data_t *data,
			value_unit_t *want, uint32_t *want_base)
{
	(void)userdata;
	/* Claim temperatures only; everything else falls through to the policy. */
	bool ok = false;
	value_unit_t kelvin = bvn_parse_unit((const uint8_t *)"K", &ok);
	if (!ok || !bvn_units_compatible(data->value_unit, kelvin))
		return false;
	*want      = kelvin;
	*want_base = 0u;
	return true;
}

/*
 * want_unit is the most specific selector and wins; the policy catches what the
 * hook declined. That combination is the point of having a ladder rather than
 * making the two mutually exclusive.
 */
static void test_hook_outranks_policy(void)
{
	static const bvnr_unit_target_t targets[] = { { "°C", 0 }, { "m", 0 } };
	bvnr_unit_policy_t p = {0};
	p.targets     = targets;
	p.num_targets = 2;

	pol_ctx_t c = {0};
	bvnr_read_flags_t flags;
	memset(&flags, 0, sizeof(flags));
	flags.userdata    = &c;
	flags.on_verified = pol_on_verified;
	flags.want_unit   = ladder_hook;

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "ladder: reader_create");
	if (!r) return;
	ASSERT_TRUE(bvnr_reader_set_unit_policy(r, &p), "ladder: set policy");
	const char *doc = ".t = 25 °C;\n.d = 12 in;\n";
	bool ok = bvnr_open_read_mem(r, doc, (uint32_t)strlen(doc), NULL, 0, &flags)
		  && bvnr_read(r);
	ASSERT_TRUE(ok, "ladder: read succeeded");
	ASSERT_EQ_INT(c.n, 2, "ladder: two values");
	if (c.n == 2) {
		/* The hook claimed the temperature and asked for K, overriding the
		 * policy's °C target. */
		ASSERT_STR(c.unit[0], "K", "ladder: hook won the temperature");
		ASSERT_STR(c.text[0], "298.15", "ladder: 25 °C -> 298.15 K");
		ASSERT_STR(c.unit[1], "m", "ladder: policy took the length");
		ASSERT_STR(c.text[1], "0.3048", "ladder: 12 in -> 0.3048 m");
	}
	bvnr_reader_destroy(r);
}

/*
 * A policy describes the consumer, not the document, so it survives re-opening
 * the reader — and it can be installed before the first open, which is the
 * order most callers will reach for.
 */
static void test_policy_survives_reader_reuse(void)
{
	static const bvnr_unit_target_t targets[] = { { "m", 0 } };
	bvnr_unit_policy_t p = {0};
	p.targets = targets; p.num_targets = 1;

	pol_ctx_t c = {0};
	bvnr_read_flags_t flags;
	memset(&flags, 0, sizeof(flags));
	flags.userdata    = &c;
	flags.on_verified = pol_on_verified;

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "reuse: reader_create");
	if (!r) return;
	/* Set BEFORE the first open: bvn_val_init memsets the validator, and the
	 * policy is deliberately carried across that. */
	ASSERT_TRUE(bvnr_reader_set_unit_policy(r, &p), "reuse: set policy");

	for (int pass = 0; pass < 2; pass++) {
		memset(&c, 0, sizeof c);
		const char *doc = ".d = 5 k~m;";
		bool ok = bvnr_open_read_mem(r, doc, (uint32_t)strlen(doc),
					     NULL, 0, &flags) && bvnr_read(r);
		ASSERT_TRUE(ok, "reuse: read succeeded");
		ASSERT_EQ_INT(c.n, 1, "reuse: one value");
		if (c.n == 1) {
			ASSERT_TRUE(c.converted[0], "reuse: policy still in force");
			ASSERT_STR(c.text[0], "5000", "reuse: 5 km -> 5000 m");
		}
	}
	bvnr_reader_destroy(r);
}

/* Every unit string is parsed at set time, so a bad policy is a false return
 * before the parse rather than an error inside somebody's document — and the
 * policy already in force is left untouched when one is rejected. */
static void test_set_policy_argument_checking(void)
{
	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "args: reader_create");
	if (!r) return;

	static const bvnr_unit_target_t good[] = { { "m", 0 } };
	bvnr_unit_policy_t p = {0};
	p.targets = good; p.num_targets = 1;
	ASSERT_TRUE(bvnr_reader_set_unit_policy(r, &p), "args: a good policy");

	static const bvnr_unit_target_t bad_unit[] = { { "not_a_unit_at_all", 0 } };
	bvnr_unit_policy_t q = {0};
	q.targets = bad_unit; q.num_targets = 1;
	ASSERT_TRUE(!bvnr_reader_set_unit_policy(r, &q), "args: bad unit rejected");

	static const bvnr_unit_target_t bad_base[] = { { "m", 63 } };
	q.targets = bad_base; q.num_targets = 1;
	ASSERT_TRUE(!bvnr_reader_set_unit_policy(r, &q), "args: bad base rejected");

	q.targets = good; q.num_targets = BVNR_MAX_UNIT_TARGETS + 1u;
	ASSERT_TRUE(!bvnr_reader_set_unit_policy(r, &q), "args: too many targets");

	q.targets = NULL; q.num_targets = 1;
	ASSERT_TRUE(!bvnr_reader_set_unit_policy(r, &q), "args: NULL target array");

	memset(&q, 0, sizeof q);
	q.normalise = (bvnr_unit_normalise_t)99;
	ASSERT_TRUE(!bvnr_reader_set_unit_policy(r, &q), "args: bad normalise mode");

	ASSERT_TRUE(!bvnr_reader_set_unit_policy(NULL, &p), "args: NULL reader");

	/* The rejected policies changed nothing: the good one is still installed. */
	pol_ctx_t c = {0};
	bvnr_read_flags_t flags;
	memset(&flags, 0, sizeof(flags));
	flags.userdata    = &c;
	flags.on_verified = pol_on_verified;
	const char *doc = ".d = 5 k~m;";
	bool ok = bvnr_open_read_mem(r, doc, (uint32_t)strlen(doc), NULL, 0, &flags)
		  && bvnr_read(r);
	ASSERT_TRUE(ok, "args: read succeeded");
	ASSERT_TRUE(c.n == 1 && c.converted[0] && strcmp(c.text[0], "5000") == 0,
		    "args: the rejected policies left the good one in force");

	/* NULL clears. */
	ASSERT_TRUE(bvnr_reader_set_unit_policy(r, NULL), "args: NULL clears");
	memset(&c, 0, sizeof c);
	ok = bvnr_open_read_mem(r, doc, (uint32_t)strlen(doc), NULL, 0, &flags)
	     && bvnr_read(r);
	ASSERT_TRUE(ok, "args: read succeeded after clear");
	ASSERT_TRUE(c.n == 1 && !c.converted[0], "args: cleared policy converts nothing");

	bvnr_reader_destroy(r);
}

/* Array elements are values too, and go through the same path. */
static void test_policy_applies_to_array_elements(void)
{
	static const bvnr_unit_target_t targets[] = { { "m", 0 } };
	bvnr_unit_policy_t p = {0};
	p.targets = targets; p.num_targets = 1;

	pol_ctx_t c = {0};
	run_policy(".d = <float:64,k~m> [1.0, 2.0, 3.0];", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 3, "array: three elements");
	if (c.n != 3) return;
	ASSERT_STR(c.text[0], "1000", "array: element 0 converted");
	ASSERT_STR(c.text[1], "2000", "array: element 1 converted");
	ASSERT_STR(c.text[2], "3000", "array: element 2 converted");
}

/* ── per-field rules ─────────────────────────────────────────────────────── */

static const char *NESTED_DOC =
	".inlet = {\n"
	"  .temperature = <float:64,°F> 212.0;\n"
	"  .flow        = <float:64,in> 12.0;\n"
	"};\n"
	".outlet = {\n"
	"  .temperature = <float:64,°F> 32.0;\n"
	"};\n";

/* A rule names ONE field. Its siblings, and the same key under a different
 * parent, are untouched — which is the whole difference from a target list. */
static void test_rule_exact_path(void)
{
	static const bvnr_unit_rule_t rules[] = {
		{ ".inlet.temperature", "°C", 0, bvnr_rule_convert },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = rules; p.num_rules = 1;

	pol_ctx_t c = {0};
	run_policy(NESTED_DOC, &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 3, "rule: three values");
	if (c.n != 3) return;
	ASSERT_TRUE(c.converted[0], "rule: .inlet.temperature converted");
	ASSERT_STR(c.text[0], "100", "rule: 212 °F -> 100 °C");
	ASSERT_TRUE(!c.converted[1], "rule: its sibling is untouched");
	ASSERT_TRUE(!c.converted[2], "rule: the same key elsewhere is untouched");
}

/* A trailing ".*" names a subtree. Everything under it must satisfy the rule —
 * so a subtree of mixed dimensions is a mismatch, not a partial conversion. */
static void test_rule_wildcard(void)
{
	static const bvnr_unit_rule_t lengths[] = {
		{ ".inlet.*", "m", 0, bvnr_rule_convert },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = lengths; p.num_rules = 1;
	pol_ctx_t c = {0};
	run_policy(NESTED_DOC, &p, &c, false, error_unit_mismatch);

	/* On a subtree that IS all lengths, every value converts. */
	static const bvnr_unit_rule_t all[] = { { ".d.*", "m", 0, bvnr_rule_convert } };
	memset(&c, 0, sizeof c);
	p.rules = all;
	run_policy(".d = {\n .a = <float:64,in> 12.0;\n .b = <float:64,k~m> 1.0;\n};\n",
		   &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 2, "wildcard: two values");
	if (c.n != 2) return;
	ASSERT_STR(c.text[0], "0.3048", "wildcard: 12 in -> m");
	ASSERT_STR(c.text[1], "1000", "wildcard: 1 km -> m");
}

/*
 * A prefix is only a prefix at a component boundary. Without that check
 * ".in.*" would also claim ".inlet.a" — a rule quietly applying to a field
 * nobody named is the one failure mode per-field rules must not have.
 */
static void test_rule_wildcard_respects_component_boundary(void)
{
	static const bvnr_unit_rule_t rules[] = {
		{ ".in.*", "m", 0, bvnr_rule_convert },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = rules; p.num_rules = 1;

	pol_ctx_t c = {0};
	run_policy(".in    = { .a = <float:64,in> 12.0; };\n"
		   ".inlet = { .a = <float:64,in> 12.0; };\n",
		   &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 2, "boundary: two values");
	if (c.n != 2) return;
	ASSERT_TRUE(c.converted[0], "boundary: .in.a matched");
	ASSERT_TRUE(!c.converted[1], "boundary: .inlet.a did NOT match");
}

/*
 * A rule is an assertion as well as a target. The caller named this field; a
 * value that cannot satisfy what they said about it is an error, unlike a
 * whole-document target that simply finds nothing to apply to.
 */
static void test_rule_is_an_assertion(void)
{
	static const bvnr_unit_rule_t rules[] = {
		{ ".speed", "m/s", 0, bvnr_rule_require },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = rules; p.num_rules = 1;

	/* require mode: any speed satisfies it, and the value is delivered as the
	 * document wrote it. */
	pol_ctx_t c = {0};
	run_policy(".speed = <float:64,k~m/h> 42.0;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "assertion: one value");
	if (c.n == 1) {
		ASSERT_TRUE(!c.converted[0], "assertion: require does not convert");
		ASSERT_STR(c.native[0], "k~m/h", "assertion: delivered as written");
	}
	/* the wrong quantity, and a bare number, are both mismatches */
	memset(&c, 0, sizeof c);
	run_policy(".speed = <float:64,°C> 42.0;", &p, &c, false, error_unit_mismatch);
	memset(&c, 0, sizeof c);
	run_policy(".speed = <float:64> 42.0;", &p, &c, false, error_unit_mismatch);
}

/*
 * An affine scale outside "alone, at exponent 1" has no SI value, so no policy
 * may claim it -- and the three ways a policy can claim one must each say so in
 * the way that mode says things.
 *
 * bvn_policy_selects screened on bvn_units_convertible alone, which is a
 * DIMENSIONAL screen and passes `°C/h -> K/h` by design. So a target selected
 * the pair, the conversion refused, and the refusal was swallowed because an
 * unmatched target may pass a value through: `query --unit K/h` printed the
 * °C/h number as though it had answered. A RULE was worse -- `--field
 * .rate=K/h` and `--require-field .rate=K/h` both reported OK on a document
 * that satisfies neither, which is exactly the silence a rule exists to refuse.
 */
static void test_affine_in_a_compound_is_claimed_by_no_policy(void)
{
	const char* doc = ".rate = <float:64,°C/h> 1.0;";

	/* A rule is an assertion in either mode: both refuse. */
	static const bvnr_unit_rule_t conv[] = {
		{ ".rate", "K/h", 0, bvnr_rule_convert },
	};
	static const bvnr_unit_rule_t req[] = {
		{ ".rate", "K/h", 0, bvnr_rule_require },
	};
	bvnr_unit_policy_t p = {0};
	pol_ctx_t c = {0};

	p.rules = conv; p.num_rules = 1;
	run_policy(doc, &p, &c, false, error_unit_mismatch);

	memset(&p, 0, sizeof p); memset(&c, 0, sizeof c);
	p.rules = req; p.num_rules = 1;
	run_policy(doc, &p, &c, false, error_unit_mismatch);

	/* require_dimension_of is an assertion too. */
	static const char* const dims[] = { "K/h" };
	memset(&p, 0, sizeof p); memset(&c, 0, sizeof c);
	p.require_dimension_of = dims; p.num_require_dimension_of = 1;
	run_policy(doc, &p, &c, false, error_unit_mismatch);

	/* A whole-document target is not an assertion: it simply does not match,
	 * and an unmatched target delivers the value exactly as written. */
	static const bvnr_unit_target_t tgts[] = { { "K/h", 0 } };
	memset(&p, 0, sizeof p); memset(&c, 0, sizeof c);
	p.targets = tgts; p.num_targets = 1;
	run_policy(doc, &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "affine compound: one value");
	if (c.n == 1) {
		ASSERT_TRUE(!c.converted[0],
			    "affine compound: target must not claim it");
		ASSERT_STR(c.native[0], "°C/h",
			   "affine compound: delivered as written");
	}

	/* None of this may touch a LONE affine, which converts normally... */
	memset(&p, 0, sizeof p); memset(&c, 0, sizeof c);
	static const bvnr_unit_target_t kelvin[] = { { "K", 0 } };
	p.targets = kelvin; p.num_targets = 1;
	run_policy(".rate = <float:64,°C> 25.0;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "lone affine: one value");
	if (c.n == 1)
		ASSERT_TRUE(c.converted[0], "lone affine: still converts");

	/* ...nor an ordinary compound with no affine component in it. */
	memset(&c, 0, sizeof c);
	static const bvnr_unit_target_t mps[] = { { "m/s", 0 } };
	memset(&p, 0, sizeof p);
	p.targets = mps; p.num_targets = 1;
	run_policy(".rate = <float:64,k~m/h> 90.0;", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 1, "ordinary compound: one value");
	if (c.n == 1)
		ASSERT_TRUE(c.converted[0], "ordinary compound: still converts");
}

/* Rules are the most specific thing a policy can say, so they outrank the
 * whole-document target list and the normalisation fallback alike. */
static void test_rule_outranks_targets_and_normalise(void)
{
	static const bvnr_unit_rule_t rules[]   = { { ".a", "k~m", 0, bvnr_rule_convert } };
	static const bvnr_unit_target_t tgts[] = { { "m", 0 } };
	bvnr_unit_policy_t p = {0};
	p.rules = rules; p.num_rules = 1;
	p.targets = tgts; p.num_targets = 1;
	p.normalise = bvnr_normalise_si;

	pol_ctx_t c = {0};
	run_policy(".a = <float:64,in> 12000.0;\n.b = <float:64,in> 12.0;\n",
		   &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 2, "precedence: two values");
	if (c.n != 2) return;
	ASSERT_STR(c.unit[0], "k~m", "precedence: the rule won for .a");
	ASSERT_STR(c.unit[1], "m", "precedence: .b fell through to the target list");
}

/* One assignment holds a whole array, so one rule covers every element. */
static void test_rule_covers_array_elements(void)
{
	static const bvnr_unit_rule_t rules[] = {
		{ ".v", "m", 0, bvnr_rule_convert },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = rules; p.num_rules = 1;

	pol_ctx_t c = {0};
	run_policy(".v = <float:64,k~m> [1.0, 2.0, 3.0];", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 3, "array rule: three elements");
	if (c.n != 3) return;
	ASSERT_STR(c.text[0], "1000", "array rule: element 0");
	ASSERT_STR(c.text[2], "3000", "array rule: element 2");
}

/* A path is only usable if it is known. These are the ways it can be refused
 * at set time, before a document is touched. */
static void test_rule_path_argument_checking(void)
{
	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "rule args: reader_create");
	if (!r) return;
	bvnr_unit_policy_t p = {0};

	static const bvnr_unit_rule_t no_dot[] = { { "a.b", "m", 0, bvnr_rule_convert } };
	p.rules = no_dot; p.num_rules = 1;
	ASSERT_TRUE(!bvnr_reader_set_unit_policy(r, &p),
		    "rule args: a path must start with '.' — keys always do");

	static const bvnr_unit_rule_t bare_star[] = { { ".*", "m", 0, bvnr_rule_convert } };
	p.rules = bare_star;
	ASSERT_TRUE(!bvnr_reader_set_unit_policy(r, &p),
		    "rule args: '.*' names the whole document — use targets");

	static const bvnr_unit_rule_t bad_unit[] = { { ".a", "nope", 0, bvnr_rule_convert } };
	p.rules = bad_unit;
	ASSERT_TRUE(!bvnr_reader_set_unit_policy(r, &p), "rule args: bad unit");

	static const bvnr_unit_rule_t long_path[] = {
		{ ".aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		  ".aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
		  ".aaaaaaaaaaaaaaaaaaaaaa", "m", 0, bvnr_rule_convert }
	};
	p.rules = long_path;
	ASSERT_TRUE(!bvnr_reader_set_unit_policy(r, &p), "rule args: path too long");

	p.rules = no_dot; p.num_rules = BVNR_MAX_UNIT_RULES + 1u;
	ASSERT_TRUE(!bvnr_reader_set_unit_policy(r, &p), "rule args: too many rules");
	bvnr_writer_destroy(NULL);
	bvnr_reader_destroy(r);
}

/*
 * An array of structs has ONE assignment and one key for every element, so the
 * key arrives once and the first element's push consumes it. Restoring it on
 * the close is what keeps element two onward at the same path: without it
 * `.holdings.amount` became `.amount.amount` from the second row on, and a rule
 * naming that field silently stopped applying to all but the first row of every
 * table in the document — a rule quietly not firing on a field it names, which
 * is the failure mode per-field rules exist to not have.
 */
static void test_rule_across_array_of_structs(void)
{
	static const bvnr_unit_rule_t rules[] = {
		{ ".holdings.amount", "m", 0, bvnr_rule_convert },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = rules; p.num_rules = 1;

	pol_ctx_t c = {0};
	run_policy(".holdings = [\n"
		   "  { .amount = <float:64,in>  12.0; },\n"
		   "  { .amount = <float:64,k~m>  1.0; },\n"
		   "  { .amount = <float:64,in>  24.0; }\n"
		   "];\n", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 3, "aos: three rows");
	if (c.n != 3) return;
	ASSERT_STR(c.text[0], "0.3048", "aos: row 0");
	ASSERT_STR(c.text[1], "1000",   "aos: row 1 — not just the first row");
	ASSERT_STR(c.text[2], "0.6096", "aos: row 2");

	/* ...and the assertion half reaches every row too: a wrong quantity in row
	 * two must be caught, not skipped. */
	memset(&c, 0, sizeof c);
	run_policy(".holdings = [\n"
		   "  { .amount = <float:64,in> 12.0; },\n"
		   "  { .amount = <float:64,°C>  5.0; }\n"
		   "];\n", &p, &c, false, error_unit_mismatch);
}

/* The same, with a second key per row: the key in flight when a struct opens is
 * the ARRAY's, never whichever key the previous row happened to end on. */
static void test_rule_array_of_structs_two_keys(void)
{
	static const bvnr_unit_rule_t rules[] = {
		{ ".h.b", "m", 0, bvnr_rule_convert },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = rules; p.num_rules = 1;

	pol_ctx_t c = {0};
	run_policy(".h = [\n"
		   "  { .a = <float:64,°C> 1.0; .b = <float:64,in> 12.0; },\n"
		   "  { .a = <float:64,°C> 2.0; .b = <float:64,in> 24.0; }\n"
		   "];\n", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 4, "aos keys: four values");
	if (c.n != 4) return;
	ASSERT_TRUE(!c.converted[0], "aos keys: .h.a row 0 untouched");
	ASSERT_STR(c.text[1], "0.3048", "aos keys: .h.b row 0");
	ASSERT_TRUE(!c.converted[2], "aos keys: .h.a row 1 untouched");
	ASSERT_STR(c.text[3], "0.6096", "aos keys: .h.b row 1");
}

/* A struct nested inside each row, and a sibling assignment after the array:
 * the prefix has to unwind to exactly where it started. */
static void test_rule_paths_unwind(void)
{
	static const bvnr_unit_rule_t rules[] = {
		{ ".h.i.v", "m", 0, bvnr_rule_convert },
		{ ".after",  "m", 0, bvnr_rule_convert },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = rules; p.num_rules = 2;

	pol_ctx_t c = {0};
	run_policy(".h = [ { .i = { .v = <float:64,in> 12.0; }; },\n"
		   "       { .i = { .v = <float:64,in> 24.0; }; } ];\n"
		   ".after = <float:64,k~m> 1.0;\n", &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 3, "unwind: three values");
	if (c.n != 3) return;
	ASSERT_STR(c.text[0], "0.3048", "unwind: row 0");
	ASSERT_STR(c.text[1], "0.6096", "unwind: row 1");
	ASSERT_STR(c.text[2], "1000",   "unwind: the sibling after the array");
}

/*
 * Deeper than the path buffer can describe, the position is UNKNOWN — and a
 * rule whose applicability cannot be decided is REFUSED, not skipped.
 *
 * There are three things this could do and only one of them is honest. Matching
 * anyway would be matching the wrong field, which a per-field rule must never
 * do (see bvn_key_path_t). Matching nothing was the answer here until it was
 * noticed that it is the silence this mechanism exists to refuse: a rule is an
 * assertion the caller made by NAMING a field, the header says a value a rule
 * cannot be applied to is error_unit_mismatch because "silence would defeat the
 * point of naming it", and a rule that quietly stops asserting once a document
 * gets deep is that defeat with nothing to notice it by. So it refuses, and a
 * document this deep can still be read with whole-document `targets`, which
 * need no path at all — which is what the second half of this test pins.
 */
static void test_rule_refuses_unrepresentable_paths(void)
{
	static const bvnr_unit_rule_t rules[] = {
		{ ".a.b", "m", 0, bvnr_rule_convert },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = rules; p.num_rules = 1;

	/* 40 levels of nesting — past BVN_PATH_MAX_DEPTH (32). */
	char doc[4096];
	size_t pos = 0;
	for (int i = 0; i < 40; i++)
		pos += (size_t)snprintf(doc + pos, sizeof doc - pos, ".s%d = {", i);
	pos += (size_t)snprintf(doc + pos, sizeof doc - pos,
				" .leaf = <float:64,in> 12.0; ");
	for (int i = 0; i < 40; i++)
		pos += (size_t)snprintf(doc + pos, sizeof doc - pos, "};");

	pol_ctx_t c = {0};
	run_policy(doc, &p, &c, false, error_unit_mismatch);

	/* The same document under a whole-document target: no path is consulted, so
	 * nothing is undecidable and the value converts. The refusal above is about
	 * an assertion that could not be evaluated, not about the document. */
	static const bvnr_unit_target_t targets[] = { { "m", 0 } };
	bvnr_unit_policy_t tp = {0};
	tp.targets = targets; tp.num_targets = 1;
	pol_ctx_t tc = {0};
	run_policy(doc, &tp, &tc, true, error_none);
	ASSERT_EQ_INT(tc.n, 1, "deep+targets: the value still arrives");
	if (tc.n == 1)
		ASSERT_STR(tc.text[0], "0.3048", "deep+targets: and converts");
}

/*
 * Randomised document SHAPES.
 *
 * The array-of-structs defect got past a hand-written table of cases because
 * every document in it had one struct per key — which is the shape you write
 * when you are testing units, not when you are testing paths. So this generates
 * documents instead: nested structs, arrays of scalars, arrays of structs,
 * structs inside rows, to a random depth, recording each value's true path as
 * it builds them. Then for every distinct path it installs a rule naming that
 * path and asserts the rule fires on exactly the values that live there — no
 * more (a rule must never claim a field it does not name) and no fewer (it must
 * never quietly stop applying, which is how the defect showed).
 *
 * Deterministic seed: a failure here is reproducible, and the shape that caused
 * it is printed.
 */
#define SHAPE_MAX_VALUES 40
#define SHAPE_ITERATIONS 300

typedef struct {
	char     doc[8192];
	size_t   dpos;
	char     cur[128];      /* path under construction */
	size_t   cpos;
	char     vpath[SHAPE_MAX_VALUES][128];
	int      nval;
	uint64_t rs;
} shape_t;

static uint32_t shape_rnd(shape_t *s, uint32_t n)
{
	s->rs = s->rs * 6364136223846793005ull + 1442695040888963407ull;
	return (uint32_t)((s->rs >> 33) % n);
}
static void shape_emit_value(shape_t *s)
{
	if (s->nval >= SHAPE_MAX_VALUES) return;
	snprintf(s->vpath[s->nval], sizeof s->vpath[0], "%s", s->cur);
	s->nval++;
	s->dpos += (size_t)snprintf(s->doc + s->dpos, sizeof s->doc - s->dpos,
				    "<float:64,in> 12.0");
}
static void shape_gen_value(shape_t *s, int depth);
static void shape_gen_struct(shape_t *s, int depth)
{
	s->dpos += (size_t)snprintf(s->doc + s->dpos, sizeof s->doc - s->dpos, "{");
	uint32_t members = 1u + shape_rnd(s, 3u);
	for (uint32_t i = 0; i < members; i++) {
		size_t save = s->cpos;
		char key = (char)('a' + shape_rnd(s, 6u));
		s->cpos += (size_t)snprintf(s->cur + s->cpos, sizeof s->cur - s->cpos,
					    ".%c", key);
		s->dpos += (size_t)snprintf(s->doc + s->dpos, sizeof s->doc - s->dpos,
					    ".%c = ", key);
		shape_gen_value(s, depth + 1);
		s->dpos += (size_t)snprintf(s->doc + s->dpos, sizeof s->doc - s->dpos, "; ");
		s->cur[save] = '\0'; s->cpos = save;
	}
	s->dpos += (size_t)snprintf(s->doc + s->dpos, sizeof s->doc - s->dpos, "}");
}
static void shape_gen_array(shape_t *s, int depth)
{
	uint32_t rows = 1u + shape_rnd(s, 3u);
	bool of_structs = shape_rnd(s, 2u) != 0u && depth < 5;
	s->dpos += (size_t)snprintf(s->doc + s->dpos, sizeof s->doc - s->dpos, "[");
	for (uint32_t i = 0; i < rows; i++) {
		if (i) s->dpos += (size_t)snprintf(s->doc + s->dpos,
						   sizeof s->doc - s->dpos, ", ");
		if (of_structs) shape_gen_struct(s, depth + 1);
		else            shape_emit_value(s);
	}
	s->dpos += (size_t)snprintf(s->doc + s->dpos, sizeof s->doc - s->dpos, "]");
}
static void shape_gen_value(shape_t *s, int depth)
{
	uint32_t pick = (depth >= 5) ? 0u : shape_rnd(s, 4u);
	if      (pick == 1u) shape_gen_struct(s, depth);
	else if (pick == 2u) shape_gen_array(s, depth);
	else                 shape_emit_value(s);
}

static void test_rule_paths_over_random_shapes(void)
{
	shape_t s;
	s.rs = 0x243F6A8885A308D3ull;      /* fixed: a failure must be reproducible */
	uint32_t mismatches = 0, assertions = 0;

	for (uint32_t it = 0; it < SHAPE_ITERATIONS; it++) {
		s.dpos = 0; s.cpos = 0; s.cur[0] = '\0'; s.nval = 0; s.doc[0] = '\0';
		uint32_t top = 1u + shape_rnd(&s, 3u);
		for (uint32_t i = 0; i < top; i++) {
			size_t save = s.cpos;
			char key = (char)('m' + shape_rnd(&s, 6u));
			s.cpos += (size_t)snprintf(s.cur + s.cpos, sizeof s.cur - s.cpos,
						   ".%c", key);
			s.dpos += (size_t)snprintf(s.doc + s.dpos, sizeof s.doc - s.dpos,
						   ".%c = ", key);
			shape_gen_value(&s, 0);
			s.dpos += (size_t)snprintf(s.doc + s.dpos, sizeof s.doc - s.dpos, ";\n");
			s.cur[save] = '\0'; s.cpos = save;
		}
		if (s.nval == 0 || s.nval >= SHAPE_MAX_VALUES) continue;

		for (int v = 0; v < s.nval; v++) {
			bool dup = false;
			for (int u = 0; u < v; u++)
				if (strcmp(s.vpath[u], s.vpath[v]) == 0) { dup = true; break; }
			if (dup) continue;

			static bvnr_unit_rule_t rl;
			rl.path = s.vpath[v]; rl.unit = "m"; rl.base = 0;
			rl.mode = bvnr_rule_convert;
			bvnr_unit_policy_t p = {0};
			p.rules = &rl; p.num_rules = 1;

			pol_ctx_t c = {0};
			run_policy(s.doc, &p, &c, true, error_none);
			assertions++;

			bool ok = (c.n == s.nval);
			for (int u = 0; ok && u < s.nval && u < POL_MAX_VALUES; u++) {
				bool want = strcmp(s.vpath[u], s.vpath[v]) == 0;
				if (c.converted[u] != want) ok = false;
			}
			if (!ok && mismatches < 3)
				fprintf(stderr, "  shape mismatch: rule %s over %s\n",
					s.vpath[v], s.doc);
			if (!ok) mismatches++;
		}
	}
	ASSERT_EQ_INT(mismatches, 0, "shapes: every rule fired on exactly its own path");
	ASSERT_TRUE(assertions > 1000u, "shapes: the generator produced real coverage");
}

/* ── the writer's half ───────────────────────────────────────────────────── */

/* Open a writer over `buf` carrying `policy`, set BEFORE the open so the
 * carry-across-init is exercised on every one of these. */
static bvnr_writer_t *open_writer(char *buf, size_t cap,
				  const bvnr_unit_policy_t *p, bool *set_ok)
{
	bvnr_writer_t *w = bvnr_writer_create();
	if (!w) return NULL;
	*set_ok = bvnr_writer_set_unit_policy(w, p);
	if (!*set_ok) return w;
	if (!bvnr_open_write_mem(w, buf, (uint64_t)cap, false, NULL)) {
		bvnr_writer_destroy(w);
		return NULL;
	}
	return w;
}

/*
 * A reader can only reject a document somebody already wrote. require_unit on
 * the writer is what stops the bare number reaching the file — the half the
 * format's promise actually rests on.
 */
static void test_writer_require_unit(void)
{
	static const bvnr_unit_target_t none[1] = { { NULL, 0 } };
	(void)none;
	bvnr_unit_policy_t p = {0};
	p.require_unit = true;

	char buf[512];
	bool set_ok = false;
	bvnr_writer_t *w = open_writer(buf, sizeof buf, &p, &set_ok);
	ASSERT_TRUE(w != NULL && set_ok, "writer: policy set");
	if (!w) return;
	bool ok = false;
	/* A unit given inline on the value satisfies it... */
	ok = bvnr_write_float_unit(w, "speed", 64, 9.81,
				   bvn_parse_unit((const uint8_t *)"m/s", &ok));
	ASSERT_TRUE(ok, "writer: an annotated value is accepted");
	/* ...a bare one does not, and the writer latches the reason. */
	ASSERT_TRUE(!bvnr_write_float(w, "bare", 64, 0.25),
		    "writer: a bare numeric value is refused");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_unit_mismatch,
		      "writer: refused with error_unit_mismatch");
	bvnr_writer_destroy(w);
}

/*
 * The unit can arrive from either of two places — inline on the value, or as a
 * parameter of the type annotation, in which case the value event carries no
 * unit at all. The check has to see both, and must NOT let an annotated value
 * vouch for a later bare one (val.parsed_unit outlives the value it came from).
 */
static void test_writer_sees_the_annotation_unit(void)
{
	bvnr_unit_policy_t p = {0};
	p.require_unit = true;

	char buf[512];
	bool set_ok = false, ok = false;
	bvnr_writer_t *w = open_writer(buf, sizeof buf, &p, &set_ok);
	ASSERT_TRUE(w != NULL && set_ok, "writer: policy set");
	if (!w) return;

	value_unit_t m = bvn_parse_unit((const uint8_t *)"m", &ok);
	ASSERT_TRUE(ok, "writer: parse m");

	/* Unit in the ANNOTATION, value event has none of its own. */
	bvnr_data_t d = {0};
	d.type = token_is_identifier; d.data = "len"; d.length = 3;
	ASSERT_TRUE(bvnr_write_event(w, ev_assignment_start, &d),
		    "writer: assignment_start");
	ASSERT_TRUE(bvnr_write_type_annotation(w, BVN_TYPE_UINT(32), m),
		    "writer: annotation carrying the unit");
	bvnr_data_t v = {0};
	v.type = token_is_number; v.value_type = BVN_TYPE_UINT(32);
	v.value_unit = BVN_UNIT_NO_PREFIX(bu_none);   /* nothing on the value */
	v.data = "5"; v.length = 1;
	ASSERT_TRUE(bvnr_write_event(w, ev_data, &v),
		    "writer: the annotation's unit satisfies require_unit");

	/* The next value has no annotation and no inline unit. The previous
	 * annotation must not carry it. */
	ASSERT_TRUE(!bvnr_write_float(w, "bare", 64, 1.0),
		    "writer: a later bare value is still refused");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_unit_mismatch,
		      "writer: refused with error_unit_mismatch");
	bvnr_writer_destroy(w);
}

static void test_writer_require_dimension(void)
{
	static const char *lengths[] = { "m" };
	bvnr_unit_policy_t p = {0};
	p.require_dimension_of     = lengths;
	p.num_require_dimension_of = 1;

	char buf[512];
	bool set_ok = false, ok = false;
	bvnr_writer_t *w = open_writer(buf, sizeof buf, &p, &set_ok);
	ASSERT_TRUE(w != NULL && set_ok, "writer: policy set");
	if (!w) return;
	ASSERT_TRUE(bvnr_write_float_unit(w, "a", 64, 12.0,
			bvn_parse_unit((const uint8_t *)"in", &ok)),
		    "writer: a length in any unit is accepted");
	ASSERT_TRUE(!bvnr_write_float_unit(w, "b", 64, 25.0,
			bvn_parse_unit((const uint8_t *)"°C", &ok)),
		    "writer: a temperature is refused");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_unit_mismatch,
		      "writer: refused with error_unit_mismatch");
	bvnr_writer_destroy(w);
}

/* Non-numeric values carry no unit and are none of the policy's business. */
static void test_writer_ignores_non_numeric(void)
{
	bvnr_unit_policy_t p = {0};
	p.require_unit = true;

	char buf[512];
	bool set_ok = false;
	bvnr_writer_t *w = open_writer(buf, sizeof buf, &p, &set_ok);
	ASSERT_TRUE(w != NULL && set_ok, "writer: policy set");
	if (!w) return;
	ASSERT_TRUE(bvnr_write_string(w, "name", "sensor-1"),
		    "writer: a string is not subject to a unit requirement");
	ASSERT_TRUE(bvnr_write_bool(w, "ok", true),
		    "writer: a bool is not either");
	ASSERT_TRUE(bvnr_write_null(w, "spare"),
		    "writer: nor is null");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none,
		      "writer: nothing latched");
	bvnr_writer_destroy(w);
}

/*
 * The conversion half is refused rather than half-honoured: the writer already
 * rewrites values under BVN_UNIT_REDUCE, and a second rewriting mode with
 * different exactness rules is how two features end up disagreeing about what a
 * document says.
 */
static void test_writer_refuses_the_conversion_fields(void)
{
	static const bvnr_unit_target_t targets[] = { { "m", 0 } };
	bvnr_writer_t *w = bvnr_writer_create();
	ASSERT_TRUE(w != NULL, "writer: create");
	if (!w) return;

	bvnr_unit_policy_t p = {0};
	p.targets = targets; p.num_targets = 1;
	ASSERT_TRUE(!bvnr_writer_set_unit_policy(w, &p), "writer: targets refused");

	memset(&p, 0, sizeof p);
	p.normalise = bvnr_normalise_si;
	ASSERT_TRUE(!bvnr_writer_set_unit_policy(w, &p), "writer: normalise refused");

	memset(&p, 0, sizeof p);
	p.on_inexact = bvnr_inexact_leave;
	ASSERT_TRUE(!bvnr_writer_set_unit_policy(w, &p), "writer: on_inexact refused");

	memset(&p, 0, sizeof p);
	p.base = 16;
	ASSERT_TRUE(!bvnr_writer_set_unit_policy(w, &p), "writer: base refused");

	/* ...and a malformed unit is still caught up front, as on the reader. */
	static const char *bad[] = { "not_a_unit_at_all" };
	memset(&p, 0, sizeof p);
	p.require_dimension_of = bad; p.num_require_dimension_of = 1;
	ASSERT_TRUE(!bvnr_writer_set_unit_policy(w, &p), "writer: bad unit refused");

	ASSERT_TRUE(!bvnr_writer_set_unit_policy(NULL, &p), "writer: NULL writer");
	bvnr_writer_destroy(w);
}

/* One annotation covers every element of an array; each element is its own
 * value event, and none of them may be refused for lacking a unit of its own. */
static void test_writer_array_under_one_annotation(void)
{
	bvnr_unit_policy_t p = {0};
	p.require_unit = true;

	char buf[512];
	bool set_ok = false, ok = false;
	bvnr_writer_t *w = open_writer(buf, sizeof buf, &p, &set_ok);
	ASSERT_TRUE(w != NULL && set_ok, "writer: policy set");
	if (!w) return;
	value_unit_t ms = bvn_parse_unit((const uint8_t *)"m/s", &ok);

	bvnr_data_t d = {0};
	d.type = token_is_identifier; d.data = "v"; d.length = 1;
	ASSERT_TRUE(bvnr_write_event(w, ev_assignment_start, &d), "writer: assign");
	ASSERT_TRUE(bvnr_write_type_annotation(w, BVN_TYPE_FLOAT(64), ms),
		    "writer: annotation");
	bvnr_data_t rs = {0};
	rs.value_type = BVN_TYPE_FLOAT(64);
	ASSERT_TRUE(bvnr_write_event(w, ev_array_row_start, &rs), "writer: row");
	for (int i = 0; i < 3; i++) {
		bvnr_data_t e = {0};
		e.type = token_is_array_number;
		e.value_type = BVN_TYPE_FLOAT(64);
		e.value_unit = BVN_UNIT_NO_PREFIX(bu_none);
		e.data = "1.5"; e.length = 3;
		ASSERT_TRUE(bvnr_write_event(w, ev_data, &e),
			    "writer: array element covered by the annotation");
	}
	ASSERT_TRUE(bvnr_write_event(w, ev_array_row_end, &rs), "writer: row end");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none,
		      "writer: nothing latched for the array");
	bvnr_writer_destroy(w);
}

/*
 * What the writer refuses, a reader under the same policy would have refused —
 * which is the property that makes the producer-side check worth having. Write
 * a document that satisfies the policy, then read it back under it.
 */
static void test_writer_and_reader_agree(void)
{
	static const char *lengths[] = { "m" };
	bvnr_unit_policy_t p = {0};
	p.require_unit             = true;
	p.require_dimension_of     = lengths;
	p.num_require_dimension_of = 1;

	char buf[512];
	memset(buf, 0, sizeof buf);
	bool set_ok = false, ok = false;
	bvnr_writer_t *w = open_writer(buf, sizeof buf, &p, &set_ok);
	ASSERT_TRUE(w != NULL && set_ok, "roundtrip: policy set on the writer");
	if (!w) return;
	ASSERT_TRUE(bvnr_write_float_unit(w, "a", 64, 12.0,
			bvn_parse_unit((const uint8_t *)"in", &ok)), "roundtrip: write in");
	ASSERT_TRUE(bvnr_write_float_unit(w, "b", 64, 5.0,
			bvn_parse_unit((const uint8_t *)"k~m", &ok)), "roundtrip: write km");
	ASSERT_TRUE(bvnr_write_finish(w), "roundtrip: finish");
	bvnr_writer_destroy(w);

	pol_ctx_t c = {0};
	run_policy(buf, &p, &c, true, error_none);
	ASSERT_EQ_INT(c.n, 2, "roundtrip: the reader accepts what the writer emitted");
}

/*
 * A rule that MATCHES NOTHING is satisfied — and the neighbouring case, a value
 * the reader cannot LOCATE, is not. The two behave oppositely and are easy to
 * confuse, so they are asserted together.
 *
 * The distinction: a rule that found nothing to check is fine; a rule that could
 * not check what it found is an error. What that means in practice runs against
 * what the name suggests — `--require-field` / bvnr_rule_require does NOT
 * require the field to exist. A typo in the path, or a rename the policy was not
 * updated for, turns the rule off silently and the document still validates.
 *
 * Pinned here because it is a SILENT behaviour: nothing fails when a rule stops
 * applying, so if it ever changed in either direction no other test would say
 * so. doc/06 §2.1 states it, and this is what keeps that statement true.
 */
static void test_a_rule_matching_nothing_is_satisfied(void)
{
	static const char doc[] =
		"#!bovnar 1.2\n.inlet = {\n  .speed = <float:64,m/s> 9.81;\n};\n";
	pol_ctx_t c;

	/* The path that exists: asserted for real, both ways round. */
	{
		static const bvnr_unit_rule_t hit[] = {
			{ ".inlet.speed", "m/s", 0, bvnr_rule_require } };
		bvnr_unit_policy_t p = {0};
		p.rules = hit; p.num_rules = 1;
		memset(&c, 0, sizeof c);
		run_policy(doc, &p, &c, true, error_none);
	}
	{
		static const bvnr_unit_rule_t wrong[] = {
			{ ".inlet.speed", "s", 0, bvnr_rule_require } };
		bvnr_unit_policy_t p = {0};
		p.rules = wrong; p.num_rules = 1;
		memset(&c, 0, sizeof c);
		run_policy(doc, &p, &c, false, error_unit_mismatch);
	}

	/* ...and every way of naming a path no value sits at: a typo, an absent
	 * top-level key, a path deeper than any value, and an absent subtree. Each
	 * one silently checks nothing. */
	static const char *nowhere[] = {
		".inlet.sped",            /* a typo in the leaf */
		".nope",                  /* an absent top-level key */
		".inlet.speed.deeper",    /* deeper than any value goes */
		".nope.*",                /* an absent subtree */
	};
	for (size_t i = 0; i < sizeof nowhere / sizeof nowhere[0]; i++) {
		bvnr_unit_rule_t miss[1];
		miss[0].path = nowhere[i];
		miss[0].unit = "m/s";
		miss[0].base = 0;
		miss[0].mode = bvnr_rule_require;
		bvnr_unit_policy_t p = {0};
		p.rules = miss; p.num_rules = 1;
		memset(&c, 0, sizeof c);
		/* Deliberately a unit the document's one value would FAIL, so a pass
		 * here can only mean the rule never fired -- not that it fired and
		 * happened to agree. */
		miss[0].unit = "s";
		run_policy(doc, &p, &c, true, error_none);
	}
}

/* The writer takes rules too — in require mode, since a convert rule would
 * rewrite the value being written and that door belongs to BVN_UNIT_REDUCE. */
static void test_writer_rules(void)
{
	static const bvnr_unit_rule_t req[] = {
		{ ".speed", "m/s", 0, bvnr_rule_require },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = req; p.num_rules = 1;

	char buf[512];
	bool set_ok = false, ok = false;
	bvnr_writer_t *w = open_writer(buf, sizeof buf, &p, &set_ok);
	ASSERT_TRUE(w != NULL && set_ok, "writer rules: policy set");
	if (!w) return;
	ASSERT_TRUE(bvnr_write_float_unit(w, "speed", 64, 42.0,
			bvn_parse_unit((const uint8_t *)"k~m/h", &ok)),
		    "writer rules: a speed at .speed is accepted");
	ASSERT_TRUE(bvnr_write_float_unit(w, "other", 64, 25.0,
			bvn_parse_unit((const uint8_t *)"°C", &ok)),
		    "writer rules: a field no rule names is not constrained");
	ASSERT_TRUE(!bvnr_write_float_unit(w, "speed", 64, 25.0,
			bvn_parse_unit((const uint8_t *)"°C", &ok)),
		    "writer rules: a temperature at .speed is refused");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_unit_mismatch,
		      "writer rules: error_unit_mismatch");
	bvnr_writer_destroy(w);

	/* a convert rule is refused outright */
	static const bvnr_unit_rule_t conv[] = {
		{ ".speed", "m/s", 0, bvnr_rule_convert },
	};
	bvnr_writer_t *w2 = bvnr_writer_create();
	ASSERT_TRUE(w2 != NULL, "writer rules: create");
	if (!w2) return;
	bvnr_unit_policy_t q = {0};
	q.rules = conv; q.num_rules = 1;
	ASSERT_TRUE(!bvnr_writer_set_unit_policy(w2, &q),
		    "writer rules: a convert rule is refused");
	bvnr_writer_destroy(w2);
}

/* The writer tracks struct depth for rules the same way the reader does. */
static void test_writer_rules_are_path_scoped(void)
{
	static const bvnr_unit_rule_t req[] = {
		{ ".inlet.temperature", "K", 0, bvnr_rule_require },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = req; p.num_rules = 1;

	char buf[512];
	bool set_ok = false, ok = false;
	bvnr_writer_t *w = open_writer(buf, sizeof buf, &p, &set_ok);
	ASSERT_TRUE(w != NULL && set_ok, "writer paths: policy set");
	if (!w) return;
	value_unit_t metre = bvn_parse_unit((const uint8_t *)"m", &ok);

	/* .temperature at TOP level is not .inlet.temperature */
	ASSERT_TRUE(bvnr_write_float_unit(w, "temperature", 64, 5.0, metre),
		    "writer paths: top-level .temperature is unconstrained");

	bvnr_data_t d = {0};
	d.type = token_is_identifier; d.data = "inlet"; d.length = 5;
	ASSERT_TRUE(bvnr_write_event(w, ev_assignment_start, &d),
		    "writer paths: open .inlet");
	ASSERT_TRUE(bvnr_write_event(w, ev_struct_start, &d),
		    "writer paths: struct start");
	ASSERT_TRUE(!bvnr_write_float_unit(w, "temperature", 64, 5.0, metre),
		    "writer paths: a length at .inlet.temperature is refused");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_unit_mismatch,
		      "writer paths: error_unit_mismatch");
	bvnr_writer_destroy(w);
}

/* The writer shares the path tracker, so it shares the array-of-structs case:
 * every row must be held to the rule, not just the first. */
static void test_writer_rules_across_array_of_structs(void)
{
	static const bvnr_unit_rule_t req[] = {
		{ ".h.amount", "m", 0, bvnr_rule_require },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = req; p.num_rules = 1;

	char buf[1024];
	bool set_ok = false, ok = false;
	bvnr_writer_t *w = open_writer(buf, sizeof buf, &p, &set_ok);
	ASSERT_TRUE(w != NULL && set_ok, "writer aos: policy set");
	if (!w) return;
	value_unit_t inch = bvn_parse_unit((const uint8_t *)"in", &ok);
	value_unit_t degc = bvn_parse_unit((const uint8_t *)"°C", &ok);

	bvnr_data_t d = {0};
	d.type = token_is_identifier; d.data = "h"; d.length = 1;
	ASSERT_TRUE(bvnr_write_event(w, ev_assignment_start, &d), "writer aos: assign");
	bvnr_data_t rs = {0};
	ASSERT_TRUE(bvnr_write_event(w, ev_array_row_start, &rs), "writer aos: row");

	/* row 0: a length, accepted */
	ASSERT_TRUE(bvnr_write_event(w, ev_struct_start, &rs), "writer aos: row 0 open");
	ASSERT_TRUE(bvnr_write_float_unit(w, "amount", 64, 12.0, inch),
		    "writer aos: row 0 accepted");
	ASSERT_TRUE(bvnr_write_event(w, ev_struct_end, &rs), "writer aos: row 0 close");

	/* row 1: a temperature at the same path — must be refused */
	ASSERT_TRUE(bvnr_write_event(w, ev_struct_start, &rs), "writer aos: row 1 open");
	ASSERT_TRUE(!bvnr_write_float_unit(w, "amount", 64, 25.0, degc),
		    "writer aos: row 1 is held to the rule too");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_unit_mismatch,
		      "writer aos: error_unit_mismatch");
	bvnr_writer_destroy(w);
}

/* ── the DOM tier ────────────────────────────────────────────────────────── */

/*
 * The DOM takes the same policy, so a random-access consumer can assert what it
 * expects and store what it asked for. A converted value is stored CONVERTED —
 * a caller who asked for metres and got the document's feet back would have no
 * way to notice.
 */
static void test_dom_policy(void)
{
	static const bvnr_unit_rule_t rules[] = {
		{ ".inlet.temperature", "°C", 0, bvnr_rule_convert },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = rules; p.num_rules = 1;

	bvn_dom_doc_t *doc = bvn_dom_parse_policy(NESTED_DOC,
						  (uint32_t)strlen(NESTED_DOC), &p);
	ASSERT_TRUE(doc != NULL, "dom: parsed");
	if (!doc) return;
	ASSERT_EQ_INT(bvn_dom_doc_get_parse_error(doc), error_none, "dom: no error");
	bvn_dom_node_t *n = bvn_dom_lookup(doc, ".inlet.temperature");
	ASSERT_TRUE(n != NULL, "dom: found the node");
	if (n) {
		double v = 0.0;
		ASSERT_TRUE(bvn_dom_get_float(n, &v), "dom: read as float");
		ASSERT_TRUE(v > 99.999 && v < 100.001,
			    "dom: 212 °F stored as 100 °C, not as 212");
		char ub[BVNR_UNIT_STRING_MAX];
		value_unit_t u = bvn_dom_get_unit(n);
		if (bvn_unit_to_string(u, ub, sizeof ub) >= 0)
			ASSERT_STR(ub, "°C", "dom: the stored unit is the converted one");
	}
	/* the sibling is untouched */
	bvn_dom_node_t *f = bvn_dom_lookup(doc, ".inlet.flow");
	if (f) {
		double v = 0.0;
		bvn_dom_get_float(f, &v);
		ASSERT_TRUE(v > 11.99 && v < 12.01, "dom: the sibling is as written");
	}
	bvn_dom_doc_destroy(doc);
}

/* A validation failure is a parse error like any other... */
static void test_dom_policy_validation(void)
{
	bvnr_unit_policy_t p = {0};
	p.require_unit = true;

	const char *doc_s = ".a = 5 m;\n.b = 7;\n";
	bvn_dom_doc_t *doc = bvn_dom_parse_policy(doc_s, (uint32_t)strlen(doc_s), &p);
	ASSERT_TRUE(doc != NULL, "dom validate: parsed");
	if (!doc) return;
	ASSERT_EQ_INT(bvn_dom_doc_get_parse_error(doc), error_unit_mismatch,
		      "dom validate: the bare value is rejected");
	bvn_dom_doc_destroy(doc);
}

/* ...and a policy the library refuses is error_invalid_argument, so a mistake
 * in the POLICY is never mistaken for a fault in the document. */
static void test_dom_policy_rejects_a_bad_policy(void)
{
	static const bvnr_unit_target_t bad[] = { { "not_a_unit_at_all", 0 } };
	bvnr_unit_policy_t p = {0};
	p.targets = bad; p.num_targets = 1;

	const char *doc_s = ".a = 5 m;";
	bvn_dom_doc_t *doc = bvn_dom_parse_policy(doc_s, (uint32_t)strlen(doc_s), &p);
	ASSERT_TRUE(doc != NULL, "dom bad policy: a doc is still returned");
	if (!doc) return;
	ASSERT_EQ_INT(bvn_dom_doc_get_parse_error(doc), error_invalid_argument,
		      "dom bad policy: error_invalid_argument, not a parse error");
	bvn_dom_doc_destroy(doc);
}

/* An integer that converts to a fraction is stored as a float — because that is
 * what it now is. Storing 0.005 through the integer builder would lose it. */
static void test_dom_integer_converting_to_a_fraction(void)
{
	static const bvnr_unit_rule_t rules[] = {
		{ ".m", "k~g", 0, bvnr_rule_convert },
	};
	bvnr_unit_policy_t p = {0};
	p.rules = rules; p.num_rules = 1;

	const char *doc_s = ".m = <uint:32,g> 5;";
	bvn_dom_doc_t *doc = bvn_dom_parse_policy(doc_s, (uint32_t)strlen(doc_s), &p);
	ASSERT_TRUE(doc != NULL, "dom frac: parsed");
	if (!doc) return;
	ASSERT_EQ_INT(bvn_dom_doc_get_parse_error(doc), error_none, "dom frac: no error");
	bvn_dom_node_t *n = bvn_dom_lookup(doc, ".m");
	ASSERT_TRUE(n != NULL, "dom frac: found");
	if (n) {
		double v = 0.0;
		ASSERT_TRUE(bvn_dom_get_float(n, &v), "dom frac: reads as a float");
		ASSERT_TRUE(v > 0.00499 && v < 0.00501, "dom frac: 5 g is 0.005 kg");
	}
	bvn_dom_doc_destroy(doc);
}

/*
 * Writer/reader agreement, over generated documents.
 *
 * The two sides read the same policy through different code: the writer checks
 * a unit the caller hands it, at whichever of two places the unit came from,
 * before serialising; the reader checks a unit it parsed back out of the text.
 * If those ever diverge, a producer emits a file its own consumer rejects — the
 * exact failure a producer-side check exists to prevent, arrived at through the
 * check itself. So: whatever the writer ACCEPTS under a policy must be a
 * document the reader accepts under that same policy.
 */
static const char *WRFUZZ_UNITS[] = { "m", "in", "k~m", "°C", "K", "m/s", "k~m/h" };

static bool wrfuzz_gen(bvnr_writer_t *w, shape_t *s, int depth)
{
	uint32_t members = 1u + shape_rnd(s, 3u);
	for (uint32_t i = 0; i < members; i++) {
		char key[2] = { (char)('a' + shape_rnd(s, 6u)), '\0' };
		uint32_t pick = (depth >= 3) ? 0u : shape_rnd(s, 4u);
		if (pick == 1u) {
			bvnr_data_t d = {0};
			d.type = token_is_identifier; d.data = key; d.length = 1;
			if (!bvnr_write_event(w, ev_assignment_start, &d)) return false;
			if (!bvnr_write_event(w, ev_struct_start, &d))     return false;
			if (!wrfuzz_gen(w, s, depth + 1))                  return false;
			if (!bvnr_write_event(w, ev_struct_end, &d))       return false;
		} else if (shape_rnd(s, 8u) == 0u) {
			/* sometimes a bare value, which some policies must refuse */
			if (!bvnr_write_float(w, key, 64, 1.5)) return false;
		} else {
			bool ok = false;
			value_unit_t vu = bvn_parse_unit(
				(const uint8_t *)WRFUZZ_UNITS[shape_rnd(s, 7u)], &ok);
			if (!ok) return false;
			if (!bvnr_write_float_unit(w, key, 64, 1.5, vu)) return false;
		}
	}
	return true;
}

static void test_writer_and_reader_agree_over_shapes(void)
{
	static const char *dims[]  = { "m", "K", "m/s" };
	static const bvnr_unit_rule_t rl[] = { { ".a", "m", 0, bvnr_rule_require } };
	shape_t s;
	s.rs = 0xB5026F5AA96619E9ull;
	uint32_t accepted = 0, disagreements = 0;
	char buf[8192];

	for (uint32_t it = 0; it < 200u; it++) {
		bvnr_unit_policy_t p = {0};
		switch (it % 3u) {
		case 0: p.require_unit = true; break;
		case 1: p.require_dimension_of = dims; p.num_require_dimension_of = 3; break;
		default: p.require_unit = true; p.rules = rl; p.num_rules = 1; break;
		}
		bvnr_writer_t *w = bvnr_writer_create();
		if (!w) return;
		if (!bvnr_writer_set_unit_policy(w, &p)) { bvnr_writer_destroy(w); continue; }
		memset(buf, 0, sizeof buf);
		if (!bvnr_open_write_mem(w, buf, sizeof buf, false, NULL)) {
			bvnr_writer_destroy(w); continue;
		}
		bool wok = wrfuzz_gen(w, &s, 0) && bvnr_write_finish(w);
		error_code_t werr = bvnr_writer_get_error(w);
		bvnr_writer_destroy(w);
		if (!wok) {
			/* the only refusals this generator can provoke */
			if (werr != error_unit_mismatch && werr != error_sink_buffer_exhausted)
				disagreements++;
			continue;
		}
		accepted++;

		bvnr_read_flags_t f;
		memset(&f, 0, sizeof f);
		f.max_struct_nesting = 255; f.max_array_nesting = 255;
		bvnr_reader_t *r = bvnr_reader_create();
		if (!r) return;
		bvnr_reader_set_unit_policy(r, &p);
		bool rok = bvnr_open_read_mem(r, buf, (uint32_t)strlen(buf), NULL, 0, &f)
			   && bvnr_read(r);
		if (!rok && disagreements < 3)
			fprintf(stderr, "  writer accepted, reader refused (%s): %s\n",
				bvn_error_to_string(bvnr_reader_get_error(r)), buf);
		if (!rok) disagreements++;
		bvnr_reader_destroy(r);
	}
	ASSERT_EQ_INT(disagreements, 0,
		      "agreement: the reader accepts everything the writer emitted");
	ASSERT_TRUE(accepted > 50u, "agreement: the generator produced real documents");
}

/*
 * DOM/reader agreement. The DOM stores what the streaming reader delivered, so
 * for the same document under the same policy the two must report the same
 * value and the same unit for every value — and must agree about whether the
 * document is acceptable at all.
 */
static int    dom_diff_n;
static double dom_diff_val[POL_MAX_VALUES];
static char   dom_diff_unit[POL_MAX_VALUES][64];

static void dom_diff_walk(bvn_dom_node_t *n)
{
	if (!n || dom_diff_n >= POL_MAX_VALUES) return;
	switch (bvn_dom_node_type(n)) {
	case BVN_DOM_STRUCT: {
		uint32_t c = bvn_dom_struct_count(n);
		const bvn_dom_entry_t *e = bvn_dom_struct_entries(n);
		for (uint32_t i = 0; i < c; i++) dom_diff_walk(e[i].value);
		break; }
	case BVN_DOM_ARRAY: {
		uint32_t c = bvn_dom_array_count(n);
		for (uint32_t i = 0; i < c; i++) dom_diff_walk(bvn_dom_array_at(n, i));
		break; }
	case BVN_DOM_INT:
	case BVN_DOM_FLOAT: {
		double v = 0.0;
		bvn_dom_get_float(n, &v);
		dom_diff_val[dom_diff_n] = v;
		bvn_dom_get_unit_string(n, dom_diff_unit[dom_diff_n],
					sizeof dom_diff_unit[0]);
		dom_diff_n++;
		break; }
	default: break;
	}
}

static void test_dom_and_reader_agree(void)
{
	static const char *DOCS[] = {
		".a = <float:64,in> 12.0;\n.b = <float:64,k~m> 1.0;\n",
		".s = { .t = <float:64,°F> 212.0; .u = <float:64,in> 12.0; };\n",
		".h = [ { .a = <float:64,in> 12.0; }, { .a = <float:64,k~m> 1.0; } ];\n",
		".v = <float:64,in> [12.0, 24.0, 36.0];\n",
		".m = <uint:32,g> 5;\n.n = <uint:64,k~m> 3;\n",
		(".mix = <float:64,%> 35.0;\n.bare = <float:64> 0.25;\n"
		 ".cur = <float:64,$USD> 5.0;\n"),
	};
	static const bvnr_unit_target_t tgt[] = { { "m", 0 } };
	uint32_t compared = 0;

	for (unsigned di = 0; di < sizeof DOCS / sizeof DOCS[0]; di++) {
		for (int mode = 0; mode < 3; mode++) {
			bvnr_unit_policy_t p = {0};
			p.on_inexact = bvnr_inexact_leave;
			if (mode != 1) p.normalise = bvnr_normalise_si;
			if (mode != 0) { p.targets = tgt; p.num_targets = 1; }

			pol_ctx_t c = {0};
			bvnr_read_flags_t f;
			memset(&f, 0, sizeof f);
			f.userdata = &c; f.on_verified = pol_on_verified;
			f.max_struct_nesting = 255; f.max_array_nesting = 255;
			bvnr_reader_t *r = bvnr_reader_create();
			if (!r) return;
			bvnr_reader_set_unit_policy(r, &p);
			bool rok = bvnr_open_read_mem(r, DOCS[di], (uint32_t)strlen(DOCS[di]),
						      NULL, 0, &f) && bvnr_read(r);
			bvnr_reader_destroy(r);

			bvn_dom_doc_t *doc = bvn_dom_parse_policy(
				DOCS[di], (uint32_t)strlen(DOCS[di]), &p);
			bool dok = doc && bvn_dom_doc_get_parse_error(doc) == error_none;
			ASSERT_EQ_INT(dok, rok, "dom/reader: same verdict on the document");
			if (rok && dok) {
				dom_diff_n = 0;
				uint32_t n = bvn_dom_doc_count(doc);
				const bvn_dom_entry_t *e = bvn_dom_doc_entries(doc);
				for (uint32_t i = 0; i < n; i++) dom_diff_walk(e[i].value);
				ASSERT_EQ_INT(dom_diff_n, c.n, "dom/reader: same value count");
				for (int i = 0; i < c.n && i < dom_diff_n; i++) {
					const char *txt = c.converted[i] ? c.text[i] : NULL;
					if (txt && txt[0]) {
						double want = strtod(txt, NULL);
						double got  = dom_diff_val[i];
						double tol  = (want < 0 ? -want : want) * 1e-12 + 1e-15;
						double dd   = want - got;
						if (dd < 0) dd = -dd;
						ASSERT_TRUE(dd <= tol,
							    "dom/reader: same value");
					}
					ASSERT_STR(dom_diff_unit[i], c.unit[i],
						   "dom/reader: same unit");
					compared++;
				}
			}
			if (doc) bvn_dom_doc_destroy(doc);
		}
	}
	ASSERT_TRUE(compared > 30u, "dom/reader: real coverage");
}

/*
 * Every id this build defines, for the two exhaustive sweeps below.
 *
 * The id space is blocked and sparse (see the layout note in bovnar.h), so a
 * sweep is a walk over the blocks rather than a count from 1. Currencies are in
 * it because they are base units for every purpose these tests exercise, and a
 * profile block that contributes nothing has FIRST > LAST, so its loop body
 * never runs and the table needs no editing when one grows its first entry.
 */
static const struct { int first, last; } BU_BLOCKS[] = {
	{ BVN_UNIT_NATIVE_FIRST,             BVN_UNIT_NATIVE_LAST             },
	{ BVN_PROFILE_UCUM_OPAQUE_FIRST,     BVN_PROFILE_UCUM_OPAQUE_LAST     },
	{ BVN_PROFILE_UNECE_OPAQUE_FIRST,    BVN_PROFILE_UNECE_OPAQUE_LAST    },
	{ BVN_PROFILE_QUDT_OPAQUE_FIRST,     BVN_PROFILE_QUDT_OPAQUE_LAST     },
	{ BVN_PROFILE_QUDT_QK_OPAQUE_FIRST,  BVN_PROFILE_QUDT_QK_OPAQUE_LAST  },
	{ BVN_PROFILE_UDUNITS_OPAQUE_FIRST,  BVN_PROFILE_UDUNITS_OPAQUE_LAST  },
	{ BVN_CURRENCY_FIRST,                BVN_CURRENCY_LAST                },
};
#define BU_BLOCK_COUNT (sizeof BU_BLOCKS / sizeof BU_BLOCKS[0])
/* Both sweeps below are `BU_SWEEP { ... }` over `bu`. */
#define BU_SWEEP \
	for (size_t blk_ = 0; blk_ < BU_BLOCK_COUNT; blk_++) \
		for (int bu = BU_BLOCKS[blk_].first; bu <= BU_BLOCKS[blk_].last; bu++)

/*
 * Every unit in the registry, driven through a real PARSE.
 *
 * The conversion engine had been swept exhaustively by calling it directly,
 * which proves what the engine does and nothing about what a document does. A
 * unit reaches the policy by being written into an annotation, lexed back out
 * and matched — three steps the direct sweep skips entirely. So take each of
 * the registry's units, write a document in it, and check the parse against
 * what the engine says in isolation:
 *
 *   - the canonical spelling of every unit must be re-readable in a document;
 *   - a value the parse converted must carry exactly the value and unit the
 *     engine produces for it;
 *   - a value the parse left alone must be one the engine genuinely cannot
 *     deliver — no SI form, already normal, an irrational factor, or no
 *     terminating expansion. "Left alone" is not allowed to be a shrug.
 */
static void test_every_unit_through_a_parse(void)
{
	uint32_t units = 0, converted = 0, left = 0, mismatches = 0;
	/* doc is sized FROM ubuf rather than picked round: it holds
	 * ".v = <float:64,<unit>> 1.0;\n", so anything smaller than
	 * BVNR_UNIT_STRING_MAX plus that fixed text can truncate. It used to be 512,
	 * which was provably enough while BVNR_UNIT_STRING_MAX was 192 and became a
	 * -Wformat-truncation the moment that constant was raised to 1024. Deriving
	 * the size means the next raise cannot reintroduce it. */
	char ubuf[BVNR_UNIT_STRING_MAX], doc[BVNR_UNIT_STRING_MAX + 48];
	char want[256], wunit[128];

	BU_SWEEP {
		value_unit_t u;
		memset(&u, 0, sizeof u);
		u.num_components = 1;
		u.components[0].base     = (value_base_unit_t)bu;
		u.components[0].exponent = exp_linear;
		u.components[0].prefix.system = prefix_si;
		u.components[0].prefix.id.si  = si_none;
		if (!bvn_unit_valid(u)) continue;
		if (bvn_unit_to_string(u, ubuf, sizeof ubuf) < 0) continue;
		if (!ubuf[0] || strcmp(ubuf, "no_unit") == 0) continue;
		units++;

		/* Declared 1.2 because the sweep reaches the profiles' opaque
		 * units, whose only spelling is the profile notation — and that
		 * notation is gated on the spec version. Every native unit and
		 * every currency reads the same under 1.2. */
		snprintf(doc, sizeof doc,
		         "#!bovnar 1.2\n.v = <float:64,%s> 1.0;\n", ubuf);
		bvnr_unit_policy_t p = {0};
		p.normalise  = bvnr_normalise_si;
		p.on_inexact = bvnr_inexact_leave;

		pol_ctx_t c = {0};
		run_policy(doc, &p, &c, true, error_none);
		if (c.n != 1) {
			mismatches++;
			fprintf(stderr, "  unit %s did not survive a document\n", ubuf);
			continue;
		}

		/* what the engine says, called directly */
		value_unit_t si;
		bool has_si = bvn_unit_si_normal_form(u, &si);
		bool deliverable = false;
		want[0] = wunit[0] = '\0';
		if (has_si && !bvn_unit_equal(si, u)) {
			bvn_unit_to_string(si, wunit, sizeof wunit);
			bvn_int_t *vn = bvn_int_alloc(), *vd = bvn_int_alloc();
			bvn_int_t *on = bvn_int_alloc(), *od = bvn_int_alloc();
			if (vn && vd && on && od) {
				bvn_int_from_uint64(vn, 1u);
				bvn_int_from_uint64(vd, 1u);
				bool exact = false;
				if (bvn_unit_convert_rational(vn, vd, u, si, on, od, &exact)
				    && exact) {
					bool rex = false;
					int32_t l = bvn_rational_to_str(on, od, 10u, want,
								        sizeof want, &rex);
					deliverable = (l >= 0 && rex);
				}
			}
			bvn_int_free(vn); bvn_int_free(vd);
			bvn_int_free(on); bvn_int_free(od);
		}

		if (c.converted[0]) {
			converted++;
			if (!deliverable || strcmp(c.unit[0], wunit) != 0 ||
			    strcmp(c.text[0], want) != 0) {
				mismatches++;
				fprintf(stderr, "  %s: parse gave [%s %s], engine [%s %s]\n",
					ubuf, c.text[0], c.unit[0], want, wunit);
			}
		} else {
			left++;
			if (deliverable) {
				mismatches++;
				fprintf(stderr, "  %s: left alone, but the engine delivers "
					"[%s %s]\n", ubuf, want, wunit);
			}
		}
	}
	ASSERT_TRUE(units > 380u, "registry: the sweep saw the whole table");
	ASSERT_TRUE(converted > 50u, "registry: a real share of it normalises");
	ASSERT_TRUE(left > 50u, "registry: and a real share of it cannot");
	ASSERT_EQ_INT(mismatches, 0,
		      "registry: every unit's parse matches the engine in isolation");
}

/*
 * The same table, through the other two tiers and the other policy shapes: the
 * DOM must reach the reader's answer for every unit, the writer must accept
 * every unit-bearing value under require_unit and under a rule naming that very
 * unit, and a target naming a value's OWN unit must be a no-op rather than a
 * pointless rewrite.
 */
static void test_every_unit_through_the_other_tiers(void)
{
	uint32_t units = 0, bad = 0;
	/* doc is sized FROM ubuf rather than picked round: it holds
	 * ".v = <float:64,<unit>> 1.0;\n", so anything smaller than
	 * BVNR_UNIT_STRING_MAX plus that fixed text can truncate. It used to be 512,
	 * which was provably enough while BVNR_UNIT_STRING_MAX was 192 and became a
	 * -Wformat-truncation the moment that constant was raised to 1024. Deriving
	 * the size means the next raise cannot reintroduce it. */
	char ubuf[BVNR_UNIT_STRING_MAX], doc[BVNR_UNIT_STRING_MAX + 48];
	char wbuf[256];

	BU_SWEEP {
		value_unit_t u;
		memset(&u, 0, sizeof u);
		u.num_components = 1;
		u.components[0].base     = (value_base_unit_t)bu;
		u.components[0].exponent = exp_linear;
		u.components[0].prefix.system = prefix_si;
		u.components[0].prefix.id.si  = si_none;
		if (!bvn_unit_valid(u)) continue;
		if (bvn_unit_to_string(u, ubuf, sizeof ubuf) < 0) continue;
		if (!ubuf[0] || strcmp(ubuf, "no_unit") == 0) continue;
		units++;
		/* Declared 1.2 because the sweep reaches the profiles' opaque
		 * units, whose only spelling is the profile notation — and that
		 * notation is gated on the spec version. Every native unit and
		 * every currency reads the same under 1.2. */
		snprintf(doc, sizeof doc,
		         "#!bovnar 1.2\n.v = <float:64,%s> 1.0;\n", ubuf);

		bvnr_unit_policy_t norm = {0};
		norm.normalise  = bvnr_normalise_si;
		norm.on_inexact = bvnr_inexact_leave;

		/* the DOM's answer must be the reader's */
		pol_ctx_t c = {0};
		run_policy(doc, &norm, &c, true, error_none);
		bvn_dom_doc_t *dd = bvn_dom_parse_policy(doc, (uint32_t)strlen(doc), &norm);
		if (!dd || bvn_dom_doc_get_parse_error(dd) != error_none) {
			bad++;
		} else if (c.n == 1) {
			bvn_dom_node_t *n = bvn_dom_lookup(dd, ".v");
			char du[128] = {0};
			double dv = 0.0;
			if (n) { bvn_dom_get_unit_string(n, du, sizeof du);
				 bvn_dom_get_float(n, &dv); }
			const char *wu = c.converted[0] ? c.unit[0] : ubuf;
			double wv = (c.converted[0] && c.text[0][0])
				  ? strtod(c.text[0], NULL) : 1.0;
			double dd_abs = dv - wv; if (dd_abs < 0) dd_abs = -dd_abs;
			double tol = (wv < 0 ? -wv : wv) * 1e-12 + 1e-15;
			if (!n || strcmp(du, wu) != 0 || dd_abs > tol) {
				bad++;
				fprintf(stderr, "  %s: dom [%.17g %s] vs reader [%s %s]\n",
					ubuf, dv, du, c.converted[0] ? c.text[0] : "1.0", wu);
			}
		}
		if (dd) bvn_dom_doc_destroy(dd);

		/* the writer accepts it under require_unit, and under a rule naming it */
		bvnr_unit_policy_t req = {0};
		req.require_unit = true;
		bvnr_writer_t *w = bvnr_writer_create();
		if (w) {
			bvnr_writer_set_unit_policy(w, &req);
			bvnr_open_write_mem(w, wbuf, sizeof wbuf, false, NULL);
			/* Same reason the read documents declare 1.2: an opaque unit
			 * has only the profile spelling, and the writer refuses to
			 * emit one into a document whose declared version the reader
			 * would then reject it under. */
			bvnr_write_version(w, 1, 2);
			if (!bvnr_write_float_unit(w, "v", 64, 1.0, u)) {
				bad++;
				fprintf(stderr, "  writer refused a value in %s\n", ubuf);
			}
			bvnr_writer_destroy(w);
		}
		bvnr_unit_rule_t rl = { ".v", ubuf, 0, bvnr_rule_require };
		bvnr_unit_policy_t byrule = {0};
		byrule.rules = &rl; byrule.num_rules = 1;
		w = bvnr_writer_create();
		if (w) {
			if (!bvnr_writer_set_unit_policy(w, &byrule)) {
				bad++;
			} else {
				bvnr_open_write_mem(w, wbuf, sizeof wbuf, false, NULL);
				bvnr_write_version(w, 1, 2);
				if (!bvnr_write_float_unit(w, "v", 64, 1.0, u)) {
					bad++;
					fprintf(stderr, "  %s fails a rule naming itself\n",
						ubuf);
				}
			}
			bvnr_writer_destroy(w);
		}

		/* a target naming the value's own unit must do nothing at all */
		bvnr_unit_target_t t = { ubuf, 0 };
		bvnr_unit_policy_t self = {0};
		self.targets = &t; self.num_targets = 1;
		pol_ctx_t sc = {0};
		run_policy(doc, &self, &sc, true, error_none);
		if (sc.n != 1 || sc.converted[0]) {
			bad++;
			fprintf(stderr, "  %s converted to itself\n", ubuf);
		}
	}
	ASSERT_TRUE(units > 380u, "tiers: the sweep saw the whole table");
	ASSERT_EQ_INT(bad, 0, "tiers: dom, writer and self-target agree for every unit");
}

/* The 2026-11-03 review. A per-field rule is an assertion: neither
 * bvnr_inexact_leave nor the "not a match after all" pass-through applies to
 * it, so `--field .a=m/s --leave-inexact` no longer delivers 100 k~m/h as
 * written. And six profile codes refused for want of a native unit map. */
static void test_review_2026_11_rules_and_mappings(void)
{
	static const bvnr_unit_rule_t ms[] = { { ".a", "m/s", 0, bvnr_rule_convert } };
	bvnr_unit_policy_t p = {0};
	p.rules = ms; p.num_rules = 1;
	p.on_inexact = bvnr_inexact_leave;
	pol_ctx_t c = {0};
	run_policy(".a = 100 k~m/h;\n", &p, &c, false, error_unit_inexact);

	static const bvnr_unit_rule_t big[] = {
		{ ".a", "q~m¹⁰⁰·q~g¹⁰⁰", 0, bvnr_rule_convert } };
	bvnr_unit_policy_t q = {0};
	q.rules = big; q.num_rules = 1;
	pol_ctx_t d = {0};
	run_policy(".a = 1 Q~m¹⁰⁰·Q~g¹⁰⁰;\n", &q, &d, false,
		   error_value_out_of_range);

	static const struct { const char *code, *native; } map[] = {
		{ "udunits:DU",       "DU"    },
		{ "udunits:dobsons",  "DU"    },
		{ "qudt:REV-PER-SEC", "rev/s" },
		{ "qudt:REV-PER-HR",  "rev/h" },
		{ "unece:RPS",        "rev/s" },
		{ "qudt:DEG_C-PER-YR","K/yr"  },
		{ "qudt:DEG_C-WK",    "K·wk"  },
		{ "qudt:DEG_F-HR",    "Δ°F·h" },
	};
	for (size_t i = 0; i < sizeof map / sizeof *map; i++) {
		bool ok1 = false, ok2 = false;
		value_unit_t a = bvn_parse_unit((const uint8_t *)map[i].code, &ok1);
		value_unit_t b = bvn_parse_unit((const uint8_t *)map[i].native, &ok2);
		ASSERT_TRUE(ok1 && ok2 && bvn_unit_equal(a, b), map[i].code);
	}
}

int main(void)
{
	test_targets_mixed_document();
	test_review_2026_11_rules_and_mappings();
	test_target_order_is_significant();
	test_target_output_base();
	test_unitless_is_fenced_from_the_ratio_units();
	test_currency_prefix_delta_is_selectable();
	test_unmatched_target_is_not_an_error();

	test_nonterminating_default_is_error();
	test_nonterminating_leave_delivers_native();
	test_irrational_default_is_error();
	test_irrational_is_left_under_leave();
	test_work_limit_is_left_under_leave();
	test_work_limit_default_is_error();
	test_hook_keeps_the_strict_contract();

	test_normalise_si();
	test_normalise_refuses_a_form_it_cannot_convert();
	test_normalise_high_exponents();
	test_affine_in_compound_never_aborts();
	test_normalise_irrational_factors();
	test_normalise_leaves_dimensionless_alone();
	test_normalise_leaves_si_values_alone();
	test_targets_outrank_normalise();
	test_normalise_output_base();

	test_require_unit();
	test_require_dimension();
	test_require_dimension_fences_unitless();
	test_assertions_see_the_native_unit();

	test_hook_outranks_policy();
	test_policy_survives_reader_reuse();
	test_set_policy_argument_checking();
	test_policy_applies_to_array_elements();

	test_rule_exact_path();
	test_rule_wildcard();
	test_rule_wildcard_respects_component_boundary();
	test_rule_is_an_assertion();
	test_affine_in_a_compound_is_claimed_by_no_policy();
	test_rule_outranks_targets_and_normalise();
	test_rule_covers_array_elements();
	test_rule_path_argument_checking();
	test_rule_across_array_of_structs();
	test_rule_array_of_structs_two_keys();
	test_rule_paths_unwind();
	test_rule_refuses_unrepresentable_paths();
	test_rule_paths_over_random_shapes();

	test_writer_require_unit();
	test_writer_sees_the_annotation_unit();
	test_writer_require_dimension();
	test_writer_ignores_non_numeric();
	test_writer_refuses_the_conversion_fields();
	test_writer_array_under_one_annotation();
	test_writer_and_reader_agree();
	test_a_rule_matching_nothing_is_satisfied();
	test_writer_rules();
	test_writer_rules_are_path_scoped();
	test_writer_rules_across_array_of_structs();

	test_dom_policy();
	test_dom_policy_validation();
	test_dom_policy_rejects_a_bad_policy();
	test_dom_integer_converting_to_a_fraction();
	test_dom_and_reader_agree();
	test_every_unit_through_a_parse();
	test_every_unit_through_the_other_tiers();
	test_writer_and_reader_agree_over_shapes();

	if (failures == 0) {
		printf("PASSED %d tests\n", tests);
		return 0;
	}
	fprintf(stderr, "FAILED %d of %d tests\n", failures, tests);
	return 1;
}
