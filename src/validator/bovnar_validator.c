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

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "bovnar.h"
#include "bvn_io_impl.h"
#include "bvn_val_impl.h"
#include "bovnar_currency.h"
#include "bovnar_si_units.h"
#include "bvn_float.h"
#include "bvn_gregorian_date.h"
#include "bvn_datetime.h"
/* bovnar_si_units.c. Declared here rather than pulled in through
 * bvn_unit_impl.h, whose generated prefix tables are static and would be
 * duplicated into this translation unit for one predicate. */
bool bvni_unit_affine_misplaced(value_unit_t u);
#include "bvn_profile_impl.h"
/*
 * ===========================================================================
 * Validator (semantic layer)
 * ===========================================================================
 *
 * The lexer produces lexically valid tokens; the validator turns them into
 * meaning and enforces the type system. It sits behind the lexer and is fed
 * raw tokens (bvn_val_receive) and structural events (bvn_val_receive_event).
 *
 * Dual emission — the central design choice. Each value can be observed at two
 * stages via two independent callbacks:
 *
 *   on_unverified : fired the moment the lexer hands a token over, before any
 *                   semantic check. Lets a consumer see the raw token stream.
 *   on_verified   : fired only after type/range/unit checks pass, carrying the
 *                   resolved value_type and unit.
 *
 * One exception to "before any semantic check": read-time unit conversion. The
 * want_unit hook has to run before either callback — it may abort the parse, and
 * a value both callbacks then observe must not differ between them — so when it
 * is installed, the ev_data an on_unverified consumer sees already carries a
 * populated `converted`/`conv`. Everything else about that event is still the
 * pre-validation view.
 *
 * Tooling (the canonicaliser, the events demo) uses both to show the lexer and
 * validator views side by side; ordinary consumers usually set only
 * on_verified. The bvn_emit_unverified/_verified/_both helpers centralise the
 * "callback returned false => error_scanner_callback_failed" contract.
 *
 * Type inference — bovnar values may omit an explicit type annotation. When a
 * bare value arrives the validator infers a default family (uint/sint/float/
 * utf8/bool) from the literal's shape and *synthesises* the full annotation
 * event sequence (bvn_emit_default_type_annotation), so a downstream writer can
 * round-trip the value with an explicit type even though the input had none.
 *
 * Accumulator — bvn_acc_* incrementally fold a numeric literal into a uint64
 * while it is being parsed, tracking dot/exponent presence. This makes range
 * checking for <=64-bit types allocation-free; wider types fall back to the
 * arbitrary-precision validators in bovnar_utils.c.
 *
 * Caches — type annotations and inline units repeat heavily in real documents
 * (every element of a typed array, say). tcache/iucache are small FIFO caches
 * keyed on the raw annotation bytes so the comparatively expensive parse is
 * done once per distinct annotation rather than once per value.
 */

/*
 * Reader = lexer + validator bundled. Created zeroed; the actual buffers are
 * allocated later by bvnr_open_read_*, so a freshly created reader is cheap and
 * reusable across multiple open/read cycles.
 */
bvnr_reader_t* bvnr_reader_create(void)
{
	bvnr_reader_t* r = malloc(sizeof(*r));
	if (!r) return NULL;
	memset(r, 0, sizeof(*r));
	return r;
}
void bvn_val_free(bvnr_validator_t* v)
{
	if (!v) return;
	bvn_int_free(v->conv_num); v->conv_num = NULL;
	bvn_int_free(v->conv_den); v->conv_den = NULL;
	free(v->conv_text);        v->conv_text = NULL;
	v->conv_text_cap = 0;
}
void bvnr_reader_destroy(bvnr_reader_t* r)
{
	if (!r) return;
	bvn_val_free(&r->val);
	bvn_lex_destroy(&r->lex);
	free(r);
}
void bvn_val_init(bvnr_validator_t* v, bvnr_read_flags_t* opts)
{
	/* A reader is explicitly reusable across documents (bvnr_open_read_source),
	 * and this memset is what re-arms it. Release the heap the previous document
	 * left in the conversion scratch first, or every re-open orphans it.
	 *
	 * The unit policy is the one thing carried across: it is set through its own
	 * call rather than through the flags, describes the CONSUMER rather than the
	 * document, and a caller who set it once and then opened a second document
	 * would have no way to notice it had been silently dropped. Saving it here
	 * is also what lets bvnr_reader_set_unit_policy be called either side of the
	 * open. */
	bvn_unit_policy_state_t saved_policy = v->policy;
	bvn_val_free(v);
	memset(v, 0, sizeof(*v));
	v->policy = saved_policy;
	v->value_type             = BVN_TYPE_PLAIN;
	v->parsed_unit            = BVN_UNIT_NO_PREFIX(bu_none);
	v->inferred_default_vtype = BVN_TYPE_PLAIN;
	if (opts) {
		v->userdata      = opts->userdata;
		v->on_unverified = opts->on_unverified;
		v->on_verified   = opts->on_verified;
		v->want_unit     = opts->want_unit;
		v->want_unit_allow_nonterminating =
			opts->want_unit_allow_nonterminating;
		v->max_conversion_length = opts->max_conversion_length;
		v->on_error      = opts->on_error;
	}
}
/*
 * Parse the caller's unit strings once, up front, so that a typo in a target is
 * a false return from this call rather than an error in the middle of somebody's
 * document. Nothing is written into the reader until every string has parsed:
 * a rejected policy leaves the previous one in force, which is the only
 * behaviour that makes "check the return value" a complete contract.
 */
/* ── key paths ───────────────────────────────────────────────────────────── */

void bvn_path_reset(bvn_key_path_t* p)
{
	p->len = p->depth = p->lost = 0u;
	p->pending_len = 0u;
	p->pending_lost = false;
}
void bvn_path_set_key(bvn_key_path_t* p, const uint8_t* key, uint32_t len)
{
	if (!key || len == 0u || len > BVN_PATH_MAX_KEY) {
		/* No key, or one longer than a path component may be: the position of
		 * this value is unknown rather than empty. */
		p->pending_len  = 0u;
		p->pending_lost = true;
		return;
	}
	memcpy(p->pending, key, len);
	p->pending_len  = (uint8_t)len;
	p->pending_lost = false;
}
void bvn_path_push(bvn_key_path_t* p)
{
	/* Once a push is lost every deeper one is too, whether or not it would fit:
	 * a component under an unknown ancestor is at an unknown path, and keeping
	 * the rule strictly LIFO is what makes bvn_path_pop correct. */
	if (p->lost || p->pending_lost || p->depth >= BVN_PATH_MAX_DEPTH ||
	    (size_t)p->len + 1u + p->pending_len >= BVN_PATH_MAX_BYTES) {
		/* The key is gone with the push, so a SIBLING opening in this same
		 * region has no key either until a fresh assignment supplies one —
		 * pop cannot recover what was never written down. Saying so keeps an
		 * unrepresentable region unknown rather than letting a stale key
		 * describe it. */
		p->pending_lost = true;
		p->lost++;
		return;
	}
	p->mark[p->depth++] = p->len;
	p->buf[p->len++] = '.';
	memcpy(p->buf + p->len, p->pending, p->pending_len);
	p->len += p->pending_len;
	p->pending_len  = 0u;
	p->pending_lost = false;
}
void bvn_path_pop(bvn_key_path_t* p)
{
	if (p->lost) { p->lost--; return; }
	if (p->depth == 0u) { p->len = 0u; return; }
	uint32_t mark = p->mark[--p->depth];
	/*
	 * Restore the key that was in flight when this struct opened.
	 *
	 * An ARRAY OF STRUCTS is why. `.holdings = [ {…}, {…} ]` has one
	 * assignment and one key for every element: the key arrives once, the
	 * first element's push consumes it, and without this the second element
	 * pushed whatever key was last seen INSIDE the first — so
	 * `.holdings.amount` became `.amount.amount` from element two onward, and a
	 * rule naming that field silently stopped applying to all but the first row
	 * of every table in the document.
	 *
	 * Nothing has to be remembered to undo it: the key is the component being
	 * removed, sitting in `buf` between the mark's separator and the end.
	 */
	uint32_t klen = p->len - mark - 1u;
	if (p->len > mark && klen <= BVN_PATH_MAX_KEY) {
		memcpy(p->pending, p->buf + mark + 1u, klen);
		p->pending_len  = (uint8_t)klen;
		p->pending_lost = false;
	} else {
		p->pending_len  = 0u;
		p->pending_lost = true;
	}
	p->len = mark;
}
bool bvn_path_current(const bvn_key_path_t* p, char* out, uint32_t cap,
		      uint32_t* out_len)
{
	if (p->lost || p->pending_lost || p->pending_len == 0u)
		return false;
	uint32_t need = p->len + 1u + p->pending_len;
	if (need + 1u > cap)
		return false;
	memcpy(out, p->buf, p->len);
	out[p->len] = '.';
	memcpy(out + p->len + 1u, p->pending, p->pending_len);
	out[need] = '\0';
	*out_len = need;
	return true;
}
/*
 * Does `pat` name `path`? Either the whole path, or — for a pattern written
 * ".a.b.*", stored as the prefix ".a.b" with `wildcard` set — everything below
 * it at any depth. The separator check is what keeps ".inlet" from matching
 * ".inlet_pump.x": a prefix is only a prefix at a component boundary.
 */
static bool bvn_path_matches(const bvn_unit_rule_state_t* r,
			     const char* path, uint32_t path_len)
{
	if (!r->wildcard)
		return r->path_len == path_len &&
		       memcmp(r->path, path, path_len) == 0;
	if (path_len <= r->path_len)
		return false;
	if (memcmp(r->path, path, r->path_len) != 0)
		return false;
	return path[r->path_len] == '.';
}
int32_t bvn_policy_match_rule(const bvn_unit_policy_state_t* pol,
			      const bvn_key_path_t* path)
{
	if (!pol->num_rules)
		return -1;
	char     cur[BVN_PATH_MAX_BYTES + BVN_PATH_MAX_KEY + 2u];
	uint32_t cur_len = 0u;
	if (!bvn_path_current(path, cur, (uint32_t)sizeof cur, &cur_len))
		return BVN_POLICY_PATH_UNKNOWN;
	for (uint32_t i = 0; i < pol->num_rules; i++) {
		if (bvn_path_matches(&pol->rule[i], cur, cur_len))
			return (int32_t)i;
	}
	return -1;
}
static bool bvn_unit_policy_parse(const bvnr_unit_policy_t* p,
				  bvn_unit_policy_state_t* out)
{
	bvn_unit_policy_state_t np;
	memset(&np, 0, sizeof(np));
	if (p->num_targets > BVNR_MAX_UNIT_TARGETS ||
	    p->num_require_dimension_of > BVNR_MAX_UNIT_TARGETS ||
	    p->num_rules > BVNR_MAX_UNIT_RULES)
		return false;
	if (p->num_targets && !p->targets)
		return false;
	if (p->num_require_dimension_of && !p->require_dimension_of)
		return false;
	if (p->num_rules && !p->rules)
		return false;
	if (p->normalise != bvnr_normalise_none && p->normalise != bvnr_normalise_si)
		return false;
	if (p->on_inexact != bvnr_inexact_error && p->on_inexact != bvnr_inexact_leave)
		return false;
	if (p->base != 0u && !bvn_rational_base_valid(p->base))
		return false;
	for (uint32_t i = 0; i < p->num_targets; i++) {
		const bvnr_unit_target_t* t = &p->targets[i];
		if (!t->unit || !t->unit[0])
			return false;
		if (t->base != 0u && !bvn_rational_base_valid(t->base))
			return false;
		bool ok = false;
		value_unit_t u = bvn_parse_unit((const uint8_t*)t->unit, &ok);
		if (!ok)
			return false;
		np.target[i]      = u;
		np.target_base[i] = t->base;
	}
	for (uint32_t i = 0; i < p->num_require_dimension_of; i++) {
		const char* s = p->require_dimension_of[i];
		if (!s || !s[0])
			return false;
		bool ok = false;
		value_unit_t u = bvn_parse_unit((const uint8_t*)s, &ok);
		if (!ok)
			return false;
		np.require[i] = u;
	}
	for (uint32_t i = 0; i < p->num_rules; i++) {
		const bvnr_unit_rule_t* rl = &p->rules[i];
		if (!rl->path || !rl->path[0] || !rl->unit || !rl->unit[0])
			return false;
		if (rl->mode != bvnr_rule_convert && rl->mode != bvnr_rule_require)
			return false;
		if (rl->base != 0u && !bvn_rational_base_valid(rl->base))
			return false;
		size_t plen = strlen(rl->path);
		if (plen > BVNR_MAX_UNIT_PATH)
			return false;
		/* A path names keys, and every key in a document is written with a
		 * leading dot; a pattern without one could never match anything, so it
		 * is a mistake worth reporting rather than a rule that silently never
		 * fires. */
		if (rl->path[0] != '.')
			return false;
		bvn_unit_rule_state_t* rs = &np.rule[i];
		rs->wildcard = (plen >= 2u && rl->path[plen - 1] == '*' &&
				rl->path[plen - 2] == '.');
		if (rs->wildcard) {
			plen -= 2u;             /* keep ".a.b" from ".a.b.*" */
			if (plen == 0u)
				return false;   /* ".*" would name the whole document;
						 * that is what the target list is for */
		}
		memcpy(rs->path, rl->path, plen);
		rs->path[plen] = '\0';
		rs->path_len   = (uint8_t)plen;
		rs->mode       = (uint8_t)rl->mode;
		rs->base       = rl->base;
		bool ok = false;
		rs->unit = bvn_parse_unit((const uint8_t*)rl->unit, &ok);
		if (!ok)
			return false;
	}
	np.num_rules      = p->num_rules;
	np.num_targets    = p->num_targets;
	np.num_require    = p->num_require_dimension_of;
	np.require_unit   = p->require_unit;
	np.normalise      = (uint8_t)p->normalise;
	np.on_inexact     = (uint8_t)p->on_inexact;
	np.normalise_base = p->base;
	/* An all-default policy is indistinguishable from none, and saying so here
	 * keeps every per-value check behind a single predictable branch. */
	np.active = np.num_targets || np.num_require || np.require_unit ||
		    np.num_rules ||
		    np.normalise != (uint8_t)bvnr_normalise_none;
	*out = np;
	return true;
}
bool bvnr_reader_set_unit_policy(bvnr_reader_t* r, const bvnr_unit_policy_t* p)
{
	if (!r)
		return false;
	bvn_unit_policy_state_t np;
	memset(&np, 0, sizeof(np));
	if (p && !bvn_unit_policy_parse(p, &np))
		return false;
	r->val.policy = np;
	return true;
}
/*
 * The producer's half of the same contract. A reader can only refuse a document
 * whose units a writer already got wrong, so enforcing "every value carries a
 * unit" on the way IN is the half that keeps a bare number from reaching a file
 * at all — which is what the format's promise, that a document carries
 * everything needed to interpret it, actually rests on.
 *
 * Validation only. The conversion fields are REFUSED rather than ignored: the
 * writer already has a value-rewriting mode in BVN_UNIT_REDUCE (unit_flags), and
 * a second one arriving through a different door, with different rules about
 * exactness, is how two features end up disagreeing about what a document says.
 * A caller who sets them here has assumed something untrue and is told so.
 */
bool bvnr_writer_set_unit_policy(bvnr_writer_t* w, const bvnr_unit_policy_t* p)
{
	if (!w)
		return false;
	bvn_unit_policy_state_t np;
	memset(&np, 0, sizeof(np));
	if (p) {
		if (p->num_targets || p->normalise != bvnr_normalise_none ||
		    p->base || p->on_inexact != bvnr_inexact_error)
			return false;
		/* Per-field rules are welcome — in require mode. A convert rule would
		 * rewrite the value being written, which is the same door
		 * BVN_UNIT_REDUCE already owns. */
		for (uint32_t i = 0; i < p->num_rules; i++) {
			if (p->rules[i].mode != bvnr_rule_require || p->rules[i].base)
				return false;
		}
		if (!bvn_unit_policy_parse(p, &np))
			return false;
	}
	w->val.policy = np;
	return true;
}
/*
 * Whether the policy may convert `native` to `target`.
 *
 * Two questions, and the second one is the one that bites. bvn_units_convertible
 * answers "can the engine do this at all" — dimensionally compatible, or the
 * same unit apart from prefixes, which is what keeps k~$USD -> $USD available.
 * The unitless test in front of it is a fence the engine itself does not have: a
 * value with no unit is dimensionally compatible with % and with ppm, so a
 * policy naming "%" would otherwise convert a bare `0.25` into `25 %`, and one
 * naming no_unit would turn `35 %` into `0.35`. Both are exactly the silent
 * rescale the format exists to make impossible, arrived at through the machinery
 * meant to prevent it. A bare number therefore only ever matches a bare target.
 */
bool bvn_policy_selects(value_unit_t native, value_unit_t target)
{
	if (BVN_UNIT_IS_UNITLESS(native) != BVN_UNIT_IS_UNITLESS(target))
		return false;
	/*
	 * An affine scale outside "alone, at exponent 1" is the second fence the
	 * engine's screen does not have, and for the same reason as the first:
	 * bvn_units_convertible is deliberately a DIMENSIONAL screen, and `°C/h` is
	 * dimensionally `K/h`, so it says yes to a pair the conversion entry points
	 * then refuse (doc/07 §10 -- the offset is a number of kelvin and a product
	 * has nowhere to add it).
	 *
	 * Asking the screen alone made a policy claim a value it could not deliver.
	 * A target selected `°C/h -> K/h`, the conversion failed, and the failure
	 * was swallowed because an unmatched target is allowed to pass a value
	 * through untouched -- so `query --unit K/h` printed the °C/h number as
	 * though it had answered the request. Worse for a RULE, which is an
	 * assertion the caller made by naming the field: `--field .rate=K/h` and
	 * `--require-field .rate=K/h` both reported OK on a document that cannot
	 * satisfy either, which is precisely the silence the header refuses ("a
	 * value that cannot be applied to it is error_unit_mismatch rather than a
	 * value passed through quietly").
	 *
	 * Declining here gets all three right at once, because rules,
	 * require_dimension_of and targets are all decided through this one
	 * function: the rule and the dimension requirement fail loudly, and the
	 * target simply does not match, which leaves the value untouched as an
	 * unmatched target should.
	 */
	if (bvni_unit_affine_misplaced(native) ||
	    bvni_unit_affine_misplaced(target))
		return false;
	return bvn_units_convertible(native, target);
}
/*
 * The target-resolution ladder, most specific first:
 *
 *   want_unit hook  ->  policy targets  ->  policy normalise mode
 *
 * The hook wins because it decides per value and can see everything the policy
 * can; a caller who installs both is asking to normalise the document and
 * hand-handle one field. Returns false when nothing selected a target, which
 * leaves the value in its native unit — that is the normal outcome for a value
 * no rule mentions, not a failure.
 *
 * *from_policy tells the conversion path which contract it is under: the hook
 * promises all-or-nothing exactness, while a blanket policy may be configured to
 * step over a value it cannot deliver exactly.
 */
static bool bvn_resolve_unit_target(bvnr_validator_t* v, const bvnr_data_t* d,
				    value_unit_t* want, uint32_t* want_base,
				    bool* from_policy)
{
	*from_policy = false;
	if (v->want_unit) {
		value_unit_t w = d->value_unit;
		uint32_t     b = 0u;
		if (v->want_unit(v->userdata, d, &w, &b)) {
			*want      = w;
			*want_base = b;
			return true;
		}
	}
	if (!v->policy.active)
		return false;
	/* A rule about this exact field outranks anything said about the document
	 * as a whole. bvn_apply_unit_assertions already matched it and proved the
	 * value can satisfy it. */
	if (v->cur_rule >= 0) {
		const bvn_unit_rule_state_t* rl = &v->policy.rule[v->cur_rule];
		if (rl->mode == (uint8_t)bvnr_rule_require)
			return false;                  /* assert only, deliver as written */
		if (rl->base == 0u && bvn_unit_equal(d->value_unit, rl->unit))
			return false;                  /* already there */
		*want        = rl->unit;
		*want_base   = rl->base;
		*from_policy = true;
		return true;
	}
	for (uint32_t i = 0; i < v->policy.num_targets; i++) {
		if (!bvn_policy_selects(d->value_unit, v->policy.target[i]))
			continue;
		/* The value is already in the unit this target names, and no output
		 * base was asked for: there is nothing to restate. Skipping keeps
		 * `converted` meaning "the policy changed this value" rather than
		 * "the policy looked at it", and saves an exact parse-convert-render
		 * round trip per already-correct value. A target that DOES name a
		 * base is a pure base conversion and still runs. */
		if (v->policy.target_base[i] == 0u &&
		    bvn_unit_equal(d->value_unit, v->policy.target[i]))
			return false;
		*want        = v->policy.target[i];
		*want_base   = v->policy.target_base[i];
		*from_policy = true;
		return true;
	}
	if (v->policy.normalise == (uint8_t)bvnr_normalise_si) {
		value_unit_t si;
		/* Refuses the currencies and every dimensionless unit, which is what
		 * keeps "normalise everything" from reinterpreting a percentage or
		 * demanding the irrational factor between ° and rad. */
		if (bvn_unit_si_normal_form(d->value_unit, &si) &&
		    !bvn_unit_equal(si, d->value_unit)) {
			*want        = si;
			*want_base   = v->policy.normalise_base;
			*from_policy = true;
			return true;
		}
	}
	return false;
}
/*
 * The policy's validation half (bvnr_unit_policy_t.require_*). Runs on the unit
 * the DOCUMENT wrote, before any conversion and whether or not one was
 * requested: validate what you were sent, convert for the consumer. Checking
 * after conversion would be very nearly tautological — the value would be in the
 * unit the caller just asked for, so "must be a length" would pass because the
 * policy said "m", not because the document said anything.
 */
bool bvn_unit_policy_accepts(const bvn_unit_policy_state_t* p, value_unit_t u)
{
	if (!p->active)
		return true;
	if (p->require_unit && BVN_UNIT_IS_UNITLESS(u))
		return false;
	if (p->num_require) {
		for (uint32_t i = 0; i < p->num_require; i++) {
			if (bvn_policy_selects(u, p->require[i]))
				return true;
		}
		return false;
	}
	return true;
}
static bool bvn_apply_unit_assertions(bvnr_reader_t* r, token_type_t tt,
				      const bvnr_data_t* d)
{
	bvnr_validator_t* v = &r->val;
	v->cur_rule = -1;
	if (!v->policy.active)
		return true;
	if (tt != token_is_number && tt != token_is_array_number &&
	    tt != token_is_string && tt != token_is_array_string)
		return true;
	if (!bvn_type_is_numeric(d->value_type))
		return true;

	if (!bvn_unit_policy_accepts(&v->policy, d->value_unit)) {
		v->last_error = error_unit_mismatch;
		return false;
	}
	/* Per-field rules. Decided here, once, and handed to the conversion pass in
	 * cur_rule: both passes need the same answer, and matching a path twice per
	 * value would be the kind of duplication that eventually disagrees.
	 *
	 * A rule is an assertion whichever mode it is in. The caller NAMED this
	 * field; a value that cannot satisfy what they said about it is an error,
	 * not something to pass through quietly the way an unmatched whole-document
	 * target is. */
	if (v->policy.num_rules) {
		int32_t idx = bvn_policy_match_rule(&v->policy, &v->path);
		/*
		 * The position could not be established -- the document nested deeper
		 * than BVN_PATH_MAX_DEPTH, or a key path outgrew BVN_PATH_MAX_BYTES.
		 * The path machinery is careful never to report a WRONG position (see
		 * bvn_key_path_t), which leaves exactly two honest answers here, and
		 * this used to take the quieter one: no rule matched, carry on.
		 *
		 * That is the silence this whole mechanism refuses everywhere else. A
		 * per-field rule is an ASSERTION the caller made by naming a field, and
		 * the header says so: a value a rule cannot be applied to is
		 * error_unit_mismatch rather than a value left alone, "silence would
		 * defeat the point of naming it". A rule that stops asserting because
		 * the document got deep is that defeat with no way to notice it --
		 * `.reactor.…35 levels….pressure is a pressure` simply stopped being
		 * checked, and the parse still said yes.
		 *
		 * So it refuses. error_unit_mismatch is the policy's own code and this
		 * is a policy requirement that could not be met; a document this deep
		 * can still be read with whole-document `targets`, which need no path.
		 */
		if (idx == BVN_POLICY_PATH_UNKNOWN) {
			v->last_error = error_unit_mismatch;
			return false;
		}
		if (idx >= 0) {
			const bvn_unit_rule_state_t* rl = &v->policy.rule[idx];
			if (!bvn_policy_selects(d->value_unit, rl->unit)) {
				v->last_error = error_unit_mismatch;
				return false;
			}
			v->cur_rule = idx;
		}
	}
	return true;
}
/*
 * Callbacks receive a non-NULL data pointer even for empty payloads: consumers
 * can blindly read/printf the pointer without a NULL guard. This sentinel is
 * the shared "" they see in that case.
 */
static const uint8_t bvn_empty_sentinel[1] = { 0 };
static inline void bvn_normalize_data_ptr(bvnr_data_t* d)
{
	if (d && d->data == NULL)
		d->data = bvn_empty_sentinel;
}
/* Remember the text parsed_unit is about to be (or was just) read from. */
static inline void bvn_val_set_unit_src(bvnr_validator_t* v,
					const uint8_t* s, uint32_t n)
{
	if (n > BVNR_UNIT_SOURCE_MAX) n = BVNR_UNIT_SOURCE_MAX;
	v->unit_src_len = (uint8_t)n;
	memcpy(v->unit_src, s, n);
}
/* bvnr_data_t.unit_source: the spelling of the unit this event carries. Only a
 * real unit has one, and d->value_unit is always parsed_unit when it is real,
 * so the validator's copy is the right text by its own invariant. */
static inline void bvn_attach_unit_src(const bvnr_validator_t* v, bvnr_data_t* d)
{
	if (BVN_UNIT_IS_REAL(d->value_unit) && v->unit_src_len) {
		d->unit_source        = v->unit_src;
		d->unit_source_length = v->unit_src_len;
	}
}
static inline bool bvn_emit_unverified(bvnr_reader_t* r,
									   bvnr_event_t ev, bvnr_data_t* d)
{
	bvnr_validator_t* v = &r->val;
	if (!v->on_unverified) return true;
	bvn_normalize_data_ptr(d);
	bvn_attach_unit_src(v, d);
	if (!v->on_unverified(v->userdata, ev, d)) {
		v->last_error = error_scanner_callback_failed;
		return false;
	}
	return true;
}
static inline bool bvn_emit_verified(bvnr_reader_t* r,
									 bvnr_event_t ev, bvnr_data_t* d)
{
	bvnr_validator_t* v = &r->val;
	if (!v->on_verified) return true;
	bvn_normalize_data_ptr(d);
	bvn_attach_unit_src(v, d);
	if (!v->on_verified(v->userdata, ev, d)) {
		v->last_error = error_scanner_callback_failed;
		return false;
	}
	return true;
}
static inline bool bvn_emit_both(bvnr_reader_t* r,
								 bvnr_event_t ev, bvnr_data_t* d)
{
	bvnr_validator_t* v = &r->val;
	if (!v->on_unverified && !v->on_verified) return true;
	if (!bvn_emit_unverified(r, ev, d)) return false;
	if (!bvn_emit_verified(r, ev, d))   return false;
	return true;
}
/*
 * Read-time LOSSLESS unit/base conversion. Just before a numeric value is
 * delivered — ahead of BOTH callbacks, since this can abort the parse and the
 * two views of one value must agree — ask the caller's want_unit hook (if any)
 * which unit and output base it wants. When it names a target, the value's exact text is parsed into an
 * arbitrary-precision rational, converted with bvn_unit_convert_rational (exact
 * for any width/base, affine-aware), and rendered in the requested base. The
 * exact result — string and rational — is attached to `d->conv`; the original
 * `data`/`length`/`value_unit` are left untouched.
 *
 * Fires for every numeric family (never datetime), with or without a unit, so a
 * caller can also request a pure base conversion (same unit, new base).
 *
 * Nothing approximate is ever delivered, and nothing is ever silently skipped:
 * once the hook has asked for a conversion, the value either arrives converted
 * or the parse stops. An incompatible target is error_unit_mismatch; an
 * irrational factor is error_unit_inexact; an unusable output base is
 * error_invalid_argument; a finite literal too extreme to build a rational from
 * is error_value_out_of_range. A result that is exact as a rational but has no
 * terminating expansion in the output base is error_unit_inexact too, unless the
 * caller set want_unit_allow_nonterminating — then it arrives with conv.text
 * NULL and conv.num/den exact. Only nan/inf pass through unconverted: they hold
 * no finite value, so no conversion was possible or promised.
 */
static bool bvn_apply_want_unit(bvnr_reader_t* r, token_type_t tt, bvnr_data_t* d)
{
	bvnr_validator_t* v = &r->val;
	if (!v->want_unit && !v->policy.active)
		return true;
	if (tt != token_is_number && tt != token_is_array_number &&
	    tt != token_is_string && tt != token_is_array_string)
		return true;
	if (!bvn_type_is_numeric(d->value_type))
		return true;

	value_unit_t want        = d->value_unit;
	uint32_t     want_base   = 0u;
	bool         from_policy = false;
	if (!bvn_resolve_unit_target(v, d, &want, &want_base, &from_policy))
		return true;                 /* nothing named a target for this value */
	/*
	 * "Step over a value this conversion cannot deliver, rather than rejecting
	 * the document over it" — bvnr_inexact_leave, and only ever for a target the
	 * POLICY chose. A blanket mode makes every value a candidate, so one
	 * irrational factor (a parsec, a water-hardness scale), one non-terminating
	 * expansion (km/h -> m/s) or one absurd exponent would otherwise reject a
	 * document nobody had a complaint about.
	 *
	 * It covers exactness and the work limit — the two ways a conversion can be
	 * refused for a property of the VALUE. It deliberately does not cover an
	 * unusable output base or an allocation failure: those are the caller's
	 * configuration and the machine's memory, and swallowing either would hide a
	 * fault instead of stepping over a value. The want_unit hook never takes this
	 * path at all: a caller who named one target for one value gets the strict
	 * all-or-nothing contract that hook was documented with.
	 */
	/* A per-field RULE is an assertion (doc/06 §2.1): the caller named this
	 * field, and a value it cannot be delivered in is an error, never the
	 * native value passed through quietly. So neither the inexact-leave mode
	 * nor the "not a match after all" pass-through below applies to one --
	 * `--field .a=m/s --leave-inexact` used to deliver 100 k~m/h as written.
	 * bvn_resolve_unit_target sets from_policy for a rule only when the hook
	 * did not choose, so cur_rule >= 0 here means the rule chose. */
	const bool from_rule = from_policy && v->cur_rule >= 0;
	const bool leave_on_refusal = from_policy && !from_rule &&
		v->policy.on_inexact == (uint8_t)bvnr_inexact_leave;

	/* Settle the output base before doing any work: 0 keeps the value's own, and
	 * anything else must be a base bvnr can actually write (2..62, 64, 85).
	 * Substituting a fallback here would hand back digits in a base the caller
	 * never asked for, which is the same class of silent misread as a wrong
	 * unit — so an unusable base is an error. */
	uint32_t wire_base = bvn_effective_base(d->value_type);
	uint32_t obase     = want_base ? want_base : wire_base;
	if (!bvn_rational_base_valid(obase)) {
		v->last_error = error_invalid_argument;
		return false;
	}

	/* d->data is not NUL-terminated; copy the exact digits out. */
	char  small[128];
	char* nb  = small;
	uint32_t len = d->length;
	if ((size_t)len + 1u > sizeof small) {
		nb = malloc((size_t)len + 1u);
		if (!nb) { v->last_error = error_invalid_argument; return false; }
	}
	if (len) memcpy(nb, d->data, len);
	nb[len] = '\0';

	/* nan/inf are floats wearing a numeric token: there is no finite value to
	 * convert, so they are handed over untouched. This is the ONLY pass-through —
	 * every other parse failure below is reported, never swallowed.
	 *
	 * A NUMBER token, that is: the reclassified keyword. A quoted string that
	 * got this far under uint/sint is digits in its base, and in base 24 and up
	 * "nan", "inf" and "ninf" are digits -- <uint:64,_36> "nan" is 30191, and
	 * was passed through unconverted because it is spelled like the keyword. */
	if ((tt == token_is_number || tt == token_is_array_number) &&
	    bvn_is_special_number_string(nb)) {
		if (nb != small) free(nb);
		return true;
	}

	/* Refuse an out-of-reach exponent BEFORE building the rational. For a literal
	 * M·base^E the exact value needs at least |E| digits either side of the point,
	 * so |E| > the output cap means the result cannot be delivered whatever else
	 * happens — and checking here is what stops the cost, because building the
	 * 10^9800 denominator is itself a tenth of a second. Rejecting only at render
	 * time left the whole attack intact under continue_on_error, where each value
	 * paid the parse and then got skipped. The bound never rejects a value that
	 * would have been accepted. */
	{
		uint32_t cap = v->max_conversion_length
			     ? v->max_conversion_length
			     : BVNR_DEFAULT_MAX_CONVERSION_LENGTH;
		const char* e = NULL;
		for (const char* p = nb; *p; p++) {
			char c = *p;
			if (wire_base == 16u ? (c == 'p' || c == 'P')
					     : (c == 'e' || c == 'E')) { e = p + 1; break; }
		}
		if (e) {
			if (*e == '+' || *e == '-') e++;
			uint64_t mag = 0;
			for (; *e >= '0' && *e <= '9'; e++) {
				mag = mag * 10u + (uint64_t)(*e - '0');
				if (mag > (uint64_t)cap) break;
			}
			if (mag > (uint64_t)cap) {
				if (nb != small) free(nb);
				if (leave_on_refusal) return true;
				v->last_error = error_value_out_of_range;
				return false;
			}
		}
	}

	/* Parse the value into an exact rational vn/vd. */
	bvn_int_t* vn = bvn_int_alloc();
	bvn_int_t* vd = bvn_int_alloc();
	bool alloc_ok = vn && vd;
	bool parsed   = alloc_ok;
	if (parsed) {
		if (d->value_type.family == vt_uint || d->value_type.family == vt_sint)
			parsed = bvn_int_from_str(vn, nb, wire_base) &&
				 bvn_int_from_uint64(vd, 1u);
		else
			parsed = bvn_float_parse_rational(
				nb, wire_base == 16u ? 16u : 10u, vn, vd);
	}
	if (nb != small) free(nb);
	if (!parsed) {
		/* A finite literal the rational builder could not represent — an
		 * exponent so large the numerator would not fit in memory, say. The
		 * caller asked for a conversion and must not be left holding the
		 * original unit's digits while believing they were converted. */
		bvn_int_free(vn); bvn_int_free(vd);
		if (alloc_ok && leave_on_refusal) return true;
		v->last_error = alloc_ok ? error_value_out_of_range
					 : error_invalid_argument;
		return false;
	}

	/* Convert exactly into the reader-owned scratch rational. */
	if (!v->conv_num) v->conv_num = bvn_int_alloc();
	if (!v->conv_den) v->conv_den = bvn_int_alloc();
	if (!v->conv_num || !v->conv_den) {
		bvn_int_free(vn); bvn_int_free(vd);
		v->last_error = error_invalid_argument;
		return false;
	}
	bool exact = false;
	bool conv_ok = bvn_unit_convert_rational(vn, vd, d->value_unit, want,
						 v->conv_num, v->conv_den, &exact);
	bvn_int_free(vn); bvn_int_free(vd);
	if (!conv_ok) {
		/*
		 * The engine will not perform this conversion at all. For a target the
		 * POLICY chose that is not an error, it is a non-match discovered one
		 * step late: a policy names a preference, and a value it cannot be
		 * applied to is delivered in its own unit exactly like a value no
		 * target mentioned. Aborting instead would reject a valid document over
		 * a pair the caller never singled out.
		 *
		 * The screen in bvn_policy_selects catches almost all of these, but it
		 * cannot catch all: `s/°C` is dimensionally compatible with `s/K` and
		 * still unconvertible, because an affine scale has no meaning at an
		 * exponent other than 1. Rather than restate the engine's rules in the
		 * selector and drift from them, the selector stays approximate and this
		 * is the backstop. Nothing is misrepresented by it — a value left in
		 * its native unit still carries that unit, and `converted` says it was
		 * not touched.
		 *
		 * A target the HOOK named is different: the caller chose it for this
		 * value, and a mismatch there is the error it was documented as.
		 *
		 * ...but only when the units really do not match. The engine also
		 * refuses a pair it AGREES about when the exact factor outgrows
		 * bvn_int_t: `Q~m¹⁰⁰·Q~g¹⁰⁰` to `q~m¹⁰⁰·q~g¹⁰⁰` is the same bases at
		 * the same exponents and needs 10^12000, past BVN_INT_MAX_BITS. Calling
		 * that error_unit_mismatch tells the caller their units are
		 * incompatible, which is a false statement about their document; the
		 * conversion is defined and merely too large to represent, which is
		 * what error_value_out_of_range already says for a literal too extreme
		 * to build a rational from. Ask the cheap question the engine's own
		 * screen answers rather than restating its capacity rule here.
		 */
		if (from_policy && !from_rule) return true;
		/*
		 * ...and the capacity question is asked STRUCTURALLY, not through the
		 * screen. bvn_units_convertible passes an affine scale in a compound by
		 * design -- it is a screen, and `s/°C` is dimensionally `s/K` -- so
		 * asking it alone reported `°C/h -> K/h` as error_value_out_of_range,
		 * which says the VALUE is extreme about a document whose value is 1.0,
		 * and sends the caller looking at their number instead of their unit.
		 * doc/05 §12.4 and doc/08 §1.10 both promise error_unit_mismatch there.
		 * bvni_unit_affine_misplaced asks only where the affine component sits,
		 * so it separates the two without restating the engine's rules.
		 */
		if (bvni_unit_affine_misplaced(d->value_unit) ||
		    bvni_unit_affine_misplaced(want))
			v->last_error = error_unit_mismatch;
		else
			v->last_error = bvn_units_convertible(d->value_unit, want)
				      ? error_value_out_of_range
				      : error_unit_mismatch;
		return false;
	}
	if (!exact) {
		/* The true factor is irrational — a parsec, a π-based angle, a water
		 * hardness scale. No exact rational exists, so unlike the
		 * non-terminating case below there is nothing to hand over even to a
		 * caller who can take one; the only alternatives are the native value
		 * or a refusal. */
		if (leave_on_refusal) return true;
		v->last_error = error_unit_inexact;
		return false;
	}

	/* Render in the output base. bvn_rational_to_str refuses a short buffer
	 * rather than truncating, so the buffer is sized from its own bound. */
	size_t need = bvn_rational_str_bufsize(v->conv_num, v->conv_den, obase);
	if (need == 0u) { v->last_error = error_invalid_argument; return false; }
	/* Bound the work. Rendering is quadratic in the digit count, and the count
	 * follows the magnitude of the value's exponent rather than its length, so a
	 * seven-byte literal like 1e-9800 costs a tenth of a second and a kilobyte of
	 * them costs minutes. Capping the buffer makes bvn_rational_to_str refuse
	 * before it generates anything, which is the only place the cost can be
	 * stopped without changing what an accepted conversion means. */
	uint32_t cap = v->max_conversion_length ? v->max_conversion_length
					        : BVNR_DEFAULT_MAX_CONVERSION_LENGTH;
	if (need > (size_t)cap + 1u)
		need = (size_t)cap + 1u;
	if ((size_t)v->conv_text_cap < need) {
		char* nt = realloc(v->conv_text, need);
		if (!nt) { v->last_error = error_invalid_argument; return false; }
		v->conv_text = nt;
		v->conv_text_cap = (uint32_t)need;
	}
	bool rexact = false;
	int32_t tlen = bvn_rational_to_str(v->conv_num, v->conv_den, obase,
					   v->conv_text, v->conv_text_cap, &rexact);
	if (tlen == BVN_RATIONAL_NONTERMINATING) {
		/* The rational is exact; only its digit string in this base is
		 * infinite (5/18 m/s in base 10, 1/1000 km in base 2). Rounding it
		 * would be the silent precision loss this hook exists to prevent, so
		 * by default the parse stops. A caller that can consume a rational
		 * opts in and gets num/den with no text. */
		if (!v->want_unit_allow_nonterminating) {
			if (leave_on_refusal) return true;
			v->last_error = error_unit_inexact;
			return false;
		}
		tlen = 0;
	} else if (tlen == BVN_RATIONAL_TOO_LONG) {
		/* Longer than max_conversion_length allows. The value is fine; it is
		 * the cost of writing it out that the reader declines. */
		if (leave_on_refusal) return true;
		v->last_error = error_value_out_of_range;
		return false;
	} else if (tlen < 0 || !rexact) {
		/* Out of memory, or a negative value in sign-less base 64/85. */
		v->last_error = error_invalid_argument;
		return false;
	}
	d->converted    = true;
	d->conv.unit    = want;
	d->conv.text    = rexact ? v->conv_text : NULL;
	d->conv.length  = rexact ? (uint32_t)tlen : 0u;
	d->conv.base    = obase;
	d->conv.num     = (const struct bvn_int_s*)v->conv_num;
	d->conv.den     = (const struct bvn_int_s*)v->conv_den;
	return true;
}
void bvn_acc_reset(bvnr_validator_t* v)
{
	v->acc_value     = 0;
	v->acc_overflow  = false;
	v->acc_has_dot   = false;
	v->acc_has_exp   = false;
	v->acc_exp_state = 0;
}
/*
 * Fold one more digit into the running integer accumulator, in arbitrary base.
 * Stops accumulating once a dot is seen (the integer value is then meaningless)
 * or once overflow is detected — the overflow flag is sticky and is what
 * bvn_check_acc_range turns into error_value_out_of_range for <=64-bit types.
 * The pre-multiply comparison avoids ever actually overflowing uint64.
 */
void bvn_acc_digit(bvnr_validator_t* v, uint32_t dv, uint32_t base)
{
	if (v->acc_overflow || v->acc_has_dot) return;
	if (base < 2u) return;
	uint64_t b = (uint64_t)base;
	uint64_t d = (uint64_t)dv;
	if (v->acc_value > (UINT64_MAX - d) / b)
		v->acc_overflow = true;
	else
		v->acc_value = v->acc_value * b + d;
}
static inline uint32_t bvn_effective_base_or_10(value_type_spec_t vt)
{
	/* float_fix/dec are always decimal; datetime's base field holds an epoch
	 * index (not a numeric base) and its carrier is decimal epoch-seconds. */
	if (vt.family == vt_float_fix || vt.family == vt_float_dec ||
	    vt.family == vt_datetime)
		return 10u;
	return vt.base ? vt.base : 10u;
}
/*
 * Infer and emit a default type annotation for an untyped value.
 *
 * The family is chosen from the literal's shape: strings -> utf8; numbers with
 * a dot/exponent -> float; numbers with a leading '-' -> sint, else uint;
 * nan/inf -> float; a number carrying a currency unit -> float_dec (money must
 * not be binary float); bools -> bool. The inferred 64-bit type then becomes
 * v->value_type so the value is validated as if it had been explicitly typed.
 *
 * inferred_default_vtype is an optimisation: consecutive values of the same
 * inferred type (e.g. a long array of plain ints) are extremely common, so if
 * the new inference matches the previous one we update state but skip
 * re-emitting the identical annotation event burst. The full event sequence
 * (start, family, width param, base param, unit param, end) is only emitted
 * when the inferred type actually changes.
 *
 * The comparison is by *effective* family/width/base (bvn_default_type_equiv),
 * not raw fields, and an explicit annotation primes inferred_default_vtype just
 * like a synthesised default. This keeps the canonical serialiser idempotent:
 * an explicit "<uint:64,_10,no_unit>" (base == 10) and the synthesised uint
 * default (base == 0, effective 10) are the same effective type, so a following
 * bare element is suppressed identically whether the run was opened by an
 * explicit annotation or an inferred one.
 */
static inline bool bvn_default_type_equiv(
	value_type_spec_t a, value_type_spec_t b)
{
	/* A datetime's `base` field is its EPOCH, and bvn_effective_base reports 10
	 * for every one of them -- so without this a bare ISO literal (unix) after
	 * a `<datetime:64,tai>` element was "the same type", its annotation was
	 * suppressed, and a re-serialisation put it under the tai annotation. */
	if (a.family == vt_datetime && b.family == vt_datetime && a.base != b.base)
		return false;
	return a.family == b.family &&
	       bvn_effective_width(a) == bvn_effective_width(b) &&
	       bvn_effective_base(a)  == bvn_effective_base(b)  &&
	       bvn_effective_q(a)     == bvn_effective_q(b);
}
/*
 * ISO-8601 datetime literal support (spec 1.1).
 *
 * The lexer accumulates a literal such as 2026-06-15T12:00:00Z into the number
 * scratch buffer (see the dtlit_* states); these helpers recognise that shape,
 * strictly validate the calendar fields, and convert the UTC civil instant to a
 * signed integer count of seconds on the declared epoch's timescale. The result
 * is rendered as a decimal string and fed back through the ordinary
 * datetime-integer path, so range checking and emission are shared verbatim.
 */
static inline bool bvn_str_is_iso_datetime(const uint8_t* s, uint32_t n)
{
	/* A '-' that immediately follows a digit can only be the date separator
	 * of an ISO literal: a number never contains it (a negative sign is at
	 * index 0, an exponent sign follows 'e'/'E'). */
	for (uint32_t i = 1; i < n; i++)
		if (s[i] == '-' && s[i - 1] >= '0' && s[i - 1] <= '9')
			return true;
	return false;
}
/* A plain signed/unsigned decimal integer: optional '-' then all digits, with
 * no '.', exponent or other character. Used to decide whether a bare value in a
 * datetime array context can inherit the established datetime type. */
static inline bool bvn_str_is_plain_integer(const uint8_t* s, uint32_t n)
{
	uint32_t i = (n > 0 && s[0] == '-') ? 1u : 0u;
	if (i >= n)
		return false;
	for (; i < n; i++)
		if (s[i] < '0' || s[i] > '9')
			return false;
	return true;
}
static bool bvn_iso_two_digit(const uint8_t* s, int* out)
{
	if (s[0] < '0' || s[0] > '9' || s[1] < '0' || s[1] > '9')
		return false;
	*out = (s[0] - '0') * 10 + (s[1] - '0');
	return true;
}
static int bvn_iso_days_in_month(int64_t y, int m)
{
	static const int dim[] = {31,28,31,30,31,30,31,31,30,31,30,31};
	if (m == 2 && ((y % 4 == 0 && y % 100 != 0) || y % 400 == 0))
		return 29;
	return dim[m - 1];
}
/* Strict parse of YYYY-MM-DD, YYYY-MM-DDTHH:MM:SS or …Z into *dt (UTC civil).
 * Returns false on a wrong-width field, a misplaced separator or an
 * out-of-range component. */
/* Strict parse of an ISO-8601 literal into *dt (the civil time exactly as
 * written) and *offset_seconds (the tz offset in seconds; 0 for `Z` or no
 * zone). Accepted forms:
 *   YYYY-MM-DD
 *   YYYY-MM-DDTHH:MM:SS[.frac][Z | ±HH:MM]
 * A fractional part (`.` then 1+ digits) is accepted: the whole-second carrier
 * floors to the written second, but the verbatim fraction digits are reported
 * via *frac / *frac_len (the digits only, without the leading '.') so the
 * caller can surface them to consumers and round-trip them. When no fraction is
 * present *frac is NULL and *frac_len is 0. Returns false on a wrong-width
 * field, a misplaced separator, an out-of-range component, a malformed offset,
 * or any trailing junk. */
static bool bvn_iso_parse_fields(const uint8_t* s, uint32_t n, bvn_datetime_t* dt,
	int64_t* offset_seconds, const uint8_t** frac, uint32_t* frac_len)
{
	*offset_seconds = 0;
	*frac     = NULL;
	*frac_len = 0;
	if (n < 10)
		return false;
	for (int i = 0; i < 4; i++)
		if (s[i] < '0' || s[i] > '9')
			return false;
	int64_t year = (s[0]-'0')*1000 + (s[1]-'0')*100 + (s[2]-'0')*10 + (s[3]-'0');
	int month, day, hour = 0, minute = 0, second = 0;
	if (s[4] != '-' || s[7] != '-')
		return false;
	if (!bvn_iso_two_digit(s + 5, &month) || !bvn_iso_two_digit(s + 8, &day))
		return false;
	if (month < 1 || month > 12)
		return false;
	if (day < 1 || day > bvn_iso_days_in_month(year, month))
		return false;

	uint32_t p = 10;
	if (p < n) {
		/* a time part must follow: 'T' HH:MM:SS */
		if (n < 19 || s[10] != 'T' || s[13] != ':' || s[16] != ':')
			return false;
		if (!bvn_iso_two_digit(s + 11, &hour) ||
		    !bvn_iso_two_digit(s + 14, &minute) ||
		    !bvn_iso_two_digit(s + 17, &second))
			return false;
		/* second == 60 is a UTC leap second. It is accepted; because the
		 * carrier is whole epoch-seconds it normalises onto the following
		 * second (the leap-second-collapse convention shared with
		 * bvn_datetime.c), which is the correct POSIX value for the civil
		 * epochs and stays consistent for tai. :61 and beyond are rejected. */
		if (hour > 23 || minute > 59 || second > 60)
			return false;
		p = 19;
		/* optional fractional seconds: '.' then 1+ digits. The carrier stays
		 * whole seconds, but the digits are captured verbatim for the caller. */
		if (p < n && s[p] == '.') {
			uint32_t f0 = ++p;
			while (p < n && s[p] >= '0' && s[p] <= '9')
				p++;
			if (p == f0)
				return false;            /* '.' with no digit */
			*frac     = s + f0;
			*frac_len = p - f0;
		}
		/* optional zone: 'Z', or ±HH:MM */
		if (p < n) {
			if (s[p] == 'Z') {
				p++;
			} else if (s[p] == '+' || s[p] == '-') {
				int sign = (s[p] == '-') ? -1 : 1;
				p++;
				int oh, om;
				if (n - p != 5u || s[p + 2] != ':')
					return false;        /* require exactly HH:MM */
				if (!bvn_iso_two_digit(s + p, &oh) ||
				    !bvn_iso_two_digit(s + p + 3, &om))
					return false;
				if (oh > 23 || om > 59)
					return false;
				*offset_seconds = sign *
					((int64_t)oh * 3600 + (int64_t)om * 60);
				p = n;
			} else {
				return false;
			}
		}
	}
	if (p != n)
		return false;                            /* trailing junk */

	memset(dt, 0, sizeof *dt);
	dt->date.year = year; dt->date.month = month; dt->date.day = day;
	dt->hour = hour; dt->minute = minute; dt->second = second;
	return true;
}
/* Convert an ISO literal to epoch seconds on vt's declared epoch. Civil epochs
 * (unix/mjd/ntp/y2000) use the uniform conversion; tai applies the leap-second
 * table; the atomic GNSS epochs are rejected (no round-trippable inverse). */
static bool bvn_iso_to_epoch_seconds(const uint8_t* s, uint32_t n,
	value_type_spec_t vt, int64_t* out, error_code_t* err,
	const uint8_t** frac, uint32_t* frac_len)
{
	bvn_datetime_t dt;
	int64_t offset_seconds;
	if (!bvn_iso_parse_fields(s, n, &dt, &offset_seconds, frac, frac_len)) {
		*err = error_invalid_datetime_literal;
		return false;
	}
	/* Fold a tz offset to true UTC: the written civil time is at UTC+offset, so
	 * UTC = civil - offset. Done before the conversion so tai's leap-second
	 * lookup is evaluated at the true UTC instant; the conversion formula
	 * absorbs the now-out-of-range field (mktime-style).
	 *
	 * The shift goes into MINUTE, not second, even though the totals are equal:
	 * an ISO offset is always a whole number of minutes, and bvn_dt has exactly
	 * one value of `second` that means something other than its arithmetic
	 * value -- 60, the inserted leap second. Subtracting the offset from
	 * `second` would erase that spelling for every offset but Z, so
	 * "2017-01-01T00:59:60+01:00" would silently become the following second
	 * instead of the insertion it names. */
	dt.minute -= offset_seconds / 60;
	/* The bvn_epoch_t IS the epoch's MJD, and they are distinct across the table,
	 * so it identifies the epoch on its own -- the name is a display string and
	 * selecting on it made this the third copy of the same chain of strcmp. */
	const int32_t epoch = bvnr_datetime_epoch_mjd(vt);   /* vt.base = epoch index */
	int64_t secs;
	if (epoch == bvn_epoch_tai) {
		secs = bvn_dt_utc_to_tai_seconds(&dt);
	} else if (epoch == bvn_epoch_gps     || epoch == bvn_epoch_galileo ||
	           epoch == bvn_epoch_glonass || epoch == bvn_epoch_beidou) {
		*err = error_datetime_literal_unsupported_epoch;
		return false;
	} else {
		/* civil epoch; the bvn_epoch_t enum value is the epoch's MJD. */
		secs = bvn_dt_datetime_to_epoch_seconds(&dt, (bvn_epoch_t)epoch);
	}
	if (secs == BVN_GDATE_OVF) {
		*err = error_value_out_of_range;
		return false;
	}
	/*
	 * A fraction only survives a round-trip as an ISO literal — the whole-second
	 * integer carrier has nowhere to put it — so a literal whose UTC civil year
	 * falls outside the 0000..9999 a 4-digit ISO year can spell is one this
	 * library can read and never write back. The writer already refuses it
	 * (bvn_ser_serialize_event, error_invalid_datetime_literal), which left
	 * "0000-01-01T00:00:00.5+23:59" validating fine and then failing every
	 * pretty-print, canonicalise and convert of the document that contained it.
	 *
	 * Refuse it here instead, so anything the reader accepts the writer can emit.
	 * The check is exactly the writer's, and it is asked only when a fraction is
	 * actually present: without one the integer carrier round-trips perfectly at
	 * any year, and rejecting those would refuse legal documents.
	 *
	 * The offset is what makes this reachable — the LOCAL year is in range and
	 * only the fold to UTC pushes it out, at either end.
	 */
	if (*frac_len > 0u) {
		bvn_datetime_t back;
		memset(&back, 0, sizeof back);   /* the converters leave *dt untouched
						  * on an out-of-range/overflow input */
		if (epoch == bvn_epoch_tai)
			bvn_dt_tai_seconds_to_utc(&back, secs);
		else
			bvn_dt_epoch_seconds_to_datetime(&back,
				(bvn_epoch_t)epoch, secs);
		if (back.date.month < 1 || back.date.month > 12 ||
		    back.date.day   < 1 || back.date.day   > 31 ||
		    back.date.year  < 0 || back.date.year  > 9999) {
			*err = error_invalid_datetime_literal;
			return false;
		}
	}
	*out = secs;
	return true;
}
/* Render an int64 as a decimal string (NUL-terminated); returns the length.
 * INT64_MIN-safe (magnitude computed in unsigned). */
static uint32_t bvn_i64_to_decimal(uint8_t* buf, int64_t value)
{
	uint8_t  tmp[24];
	uint32_t ti = 0;
	uint64_t mag = (value < 0) ? (uint64_t)(-(value + 1)) + 1u : (uint64_t)value;
	do { tmp[ti++] = (uint8_t)('0' + mag % 10u); mag /= 10u; } while (mag);
	uint32_t bi = 0;
	if (value < 0)
		buf[bi++] = '-';
	while (ti)
		buf[bi++] = tmp[--ti];
	buf[bi] = 0;
	return bi;
}
static bool bvn_emit_default_type_annotation(bvnr_reader_t* r,
	token_type_t tt, const uint8_t* str, uint32_t str_len,
	bool is_currency_unit,
	const uint8_t* inline_unit_str, uint32_t inline_unit_len,
	value_unit_t inline_unit_val)
{
	bvnr_validator_t* v = &r->val;
	value_type_spec_t default_type;
	const value_unit_t default_unit = BVN_UNIT_NO_PREFIX(bu_none);
	const char*       family_name;
	uint32_t          family_name_len;
	bool              emit_width = false;
	bool              emit_base  = false;
	bool              emit_unit  = false;
	static const char no_unit_str[] = "no_unit";
	if (tt == token_is_string || tt == token_is_array_string) {
		if (!bvn_type_is_plain(v->inferred_default_vtype) &&
		    bvn_default_type_equiv(BVN_TYPE_UTF8, v->inferred_default_vtype)) {
			v->value_type    = BVN_TYPE_UTF8;
			v->parsed_unit   = BVN_UNIT_NO_PREFIX(bu_none);
			v->unit_data_len = 0;
			return true;
		}
		default_type    = BVN_TYPE_UTF8;
		family_name     = "utf8";
		family_name_len = 4;
	} else if ((tt == token_is_number || tt == token_is_array_number) &&
	           bvn_str_is_iso_datetime(str, str_len)) {
		/* Bare ISO-8601 literal (spec 1.1) infers <datetime:64,unix>; the
		 * unix epoch is index 0 and stays implicit (no epoch param), so the
		 * synthesised annotation round-trips as <datetime:64>. */
		default_type.family = vt_datetime;
		default_type.width  = 64;
		default_type.base   = 0;
		family_name     = "datetime";
		family_name_len = 8;
		emit_width = true;
		emit_base  = false;
		emit_unit  = false;
	} else if ((tt == token_is_number || tt == token_is_array_number) &&
	           v->inferred_default_vtype.family == vt_datetime &&
	           bvn_str_is_plain_integer(str, str_len)) {
		/* A bare integer inside a datetime array (the established element
		 * type is datetime) inherits that datetime type — width and epoch —
		 * rather than inferring uint/sint. Without this the canonical form of
		 * a datetime array (annotation on the first element, bare integers
		 * after) would re-read as a datetime/uint mix and the DOM tier would
		 * reject it as non-homogeneous. inferred_default_vtype resets per
		 * assignment, so this only ever fires within one array. */
		default_type    = v->inferred_default_vtype;
		family_name     = "datetime";
		family_name_len = 8;
		emit_width = true;
		emit_base  = false;
		emit_unit  = false;
	} else if (tt == token_is_number || tt == token_is_array_number) {
		bool has_dot = v->acc_has_dot;
		bool has_exp = v->acc_has_exp;
		bool is_neg  = (str_len > 0 && str[0] == '-');
		if (!is_currency_unit && (has_dot || has_exp)) {
			value_type_spec_t cand = {.family=vt_float,.width=64,.base=0};
			if (!bvn_type_is_plain(v->inferred_default_vtype) &&
			    bvn_default_type_equiv(cand, v->inferred_default_vtype)) {
				v->value_type    = cand;
				v->parsed_unit   = BVN_UNIT_NO_PREFIX(bu_none);
				v->unit_data_len = 7u;
				return true;
			}
			default_type.family = vt_float;
			default_type.width  = 64;
			default_type.base   = 0;
			family_name     = "float";
			family_name_len = 5;
			emit_width = emit_base = emit_unit = true;
		} else if (!is_currency_unit
		           && str_len > (uint32_t)is_neg
		           && (uint8_t)(str[(uint32_t)is_neg] - '0') <= 9u) {
			value_type_spec_t cand;
			cand.family = is_neg ? vt_sint : vt_uint;
			cand.width  = 64;
			cand.base   = 0;
			if (!bvn_type_is_plain(v->inferred_default_vtype) &&
			    bvn_default_type_equiv(cand, v->inferred_default_vtype)) {
				v->value_type    = cand;
				v->parsed_unit   = BVN_UNIT_NO_PREFIX(bu_none);
				v->unit_data_len = 7u;
				return true;
			}
			default_type    = cand;
			family_name     = is_neg ? "sint" : "uint";
			family_name_len = 4;
			emit_width = emit_base = emit_unit = true;
		} else {
			bool is_special = bvn_is_special_number_string(
				(const char*)str);
			if (is_currency_unit) {
				default_type.family = vt_float_dec;
				default_type.width  = 64;
				default_type.base   = 0;
				family_name     = "float_dec";
				family_name_len = 9;
				/*
				 * float_dec has no numeric-base parameter; the
				 * parser rejects "<float_dec:...,_10>", so the
				 * canonical default must not emit a base param or
				 * its own output would fail to re-parse.
				 */
				emit_width = emit_unit = true;
				emit_base  = false;
			} else if (is_special) {
				default_type.family = vt_float;
				default_type.width  = 64;
				default_type.base   = 0;
				family_name     = "float";
				family_name_len = 5;
				emit_width = emit_base = emit_unit = true;
			} else if (is_neg) {
				default_type.family = vt_sint;
				default_type.width  = 64;
				default_type.base   = 0;
				family_name     = "sint";
				family_name_len = 4;
				emit_width = emit_base = emit_unit = true;
			} else {
				default_type.family = vt_uint;
				default_type.width  = 64;
				default_type.base   = 0;
				family_name     = "uint";
				family_name_len = 4;
				emit_width = emit_base = emit_unit = true;
			}
		}
	} else if (tt == token_is_bool) {
		default_type    = BVN_TYPE_BOOL;
		family_name     = "bool";
		family_name_len = 4;
	} else {
		return true;
	}
	if (!bvn_type_is_plain(v->inferred_default_vtype) &&
	    bvn_default_type_equiv(default_type, v->inferred_default_vtype)) {
		v->value_type    = default_type;
		v->parsed_unit   = default_unit;
		v->unit_data_len = emit_unit ? 7u : 0u;
		return true;
	}
	v->inferred_default_vtype = default_type;
	v->value_type    = default_type;
	/*
	 * Fold an inline unit on an otherwise un-annotated value into the
	 * synthesised annotation (e.g. "9.81 m/s" -> "<float:64,m/s> 9.81").
	 * The serialiser renders the unit from the annotation's unit param, not
	 * from the data event, so without this the inline unit would be dropped.
	 * Only numeric defaults carry a unit param (emit_unit); utf8 ignores
	 * units by spec, so a unit on a string is intentionally not folded.
	 */
	bool have_inline_unit =
		emit_unit && inline_unit_str != NULL && inline_unit_len > 0;
	v->parsed_unit   = have_inline_unit ? inline_unit_val : default_unit;
	if (have_inline_unit)
		bvn_val_set_unit_src(v, inline_unit_str, inline_unit_len);
	v->unit_data_len = have_inline_unit ? (uint8_t)inline_unit_len
	                 : (emit_unit ? 7u : 0u);
	bvnr_data_t d = {0};
	d.type       = token_is_type;
	d.value_type = BVN_TYPE_PLAIN;
	d.value_unit = BVN_UNIT_NO_PREFIX(bu_none);
	d.data       = family_name;
	d.length     = family_name_len;
	if (!bvn_emit_unverified(r, ev_type_annotation_start, &d))
		return false;
	d.value_type = v->value_type;
	d.value_unit = v->parsed_unit;
	if (!bvn_emit_verified(r, ev_type_annotation_start, &d))
		return false;
	if (!bvn_emit_both(r, ev_type_annotation_type_family, &d))
		return false;
	if (emit_width) {
		d.type   = token_is_type_width;
		d.data   = NULL;
		d.length = 0;
		if (!bvn_emit_both(r,
				ev_type_annotation_type_family_parameter,
				&d))
			return false;
	}
	if (emit_base) {
		d.type   = token_is_type_base;
		d.data   = NULL;
		d.length = 0;
		if (!bvn_emit_both(r,
				ev_type_annotation_type_family_parameter,
				&d))
			return false;
	}
	if (emit_unit) {
		d.type   = token_is_unit;
		if (have_inline_unit) {
			d.data   = inline_unit_str;
			d.length = inline_unit_len;
		} else {
			d.data   = no_unit_str;
			d.length = 7;
		}
		if (!bvn_emit_both(r,
				ev_type_annotation_type_family_parameter,
				&d))
			return false;
	}
	d.type   = token_is_type;
	d.data   = family_name;
	d.length = family_name_len;
	if (!bvn_emit_both(r, ev_type_annotation_end, &d))
		return false;
	return true;
}
/*
 * Re-scan a complete numeric literal to (a) populate the integer accumulator
 * for range checking and (b) set acc_has_dot / acc_has_exp. The lexer already
 * proved the literal is well-formed, so this is about extracting semantics, not
 * re-validating syntax — except for digit-in-base checking when the literal
 * came in as a string that must be interpreted in a non-decimal base.
 *
 * The base-10 path is split out and hand-tuned (inline overflow guard against
 * the UINT64 threshold 1.8e19) because decimal is by far the hot case; other
 * bases defer to bvn_char_to_digit + bvn_acc_digit. The acc_exp_state tracks
 * sign/digits of the exponent so exponent digits aren't folded into the value.
 */
static bool bvn_acc_parse_number(bvnr_validator_t* v,
	const uint8_t* str, uint32_t len, token_type_t tt)
{
	value_type_spec_t vt   = v->value_type;
	uint32_t          base = bvn_effective_base_or_10(vt);
	/* In bases 64 and 85 '+'/'-' are digit characters (those bases are
	 * unsigned-only), so a leading '-' is a digit to be accumulated, never a
	 * negative sign. */
	bool is_neg = (base != 64u && base != 85u && len > 0 && str[0] == '-');
	if (vt.family == vt_uint && is_neg) {
		v->last_error = error_value_out_of_range;
		return false;
	}
	if (tt == token_is_string || tt == token_is_array_string) {
		bool needs_parse =
			(vt.family == vt_uint || vt.family == vt_sint);
		if (!needs_parse) return true;
		uint32_t start = is_neg ? 1u : 0u;
		for (uint32_t i = start; i < len; i++) {
			uint8_t b = str[i];
			uint32_t dv = bvn_char_to_digit((uint32_t)b, base);
			if (dv >= base) {
				v->last_error = error_digit_not_in_base;
				return false;
			}
			bvn_acc_digit(v, dv, base);
		}
		return true;
	}
	if (tt != token_is_number && tt != token_is_array_number)
		return true;
	if (bvn_is_special_number_string((const char*)str))
		return true;
	uint32_t start = is_neg ? 1u : 0u;
	if (base == 10u) {
		for (uint32_t i = start; i < len; i++) {
			uint8_t b = str[i];
			if (b == '.') { v->acc_has_dot = true; continue; }
			if (b == 'e' || b == 'E') {
				v->acc_has_exp = true;
				v->acc_exp_state = 1;
				continue;
			}
			if (v->acc_exp_state > 0) {
				/* Inside the exponent: a sign or digit, never folded
				 * into the mantissa accumulator. */
				v->acc_exp_state = 2;
				continue;
			}
			uint32_t dv = (uint32_t)(b - (uint8_t)'0');
			if (dv > 9u) {
				v->last_error = error_digit_not_in_base;
				return false;
			}
			if (!v->acc_overflow && !v->acc_has_dot) {
				if (v->acc_value > (uint64_t)1844674407370955161ULL ||
				    (v->acc_value == (uint64_t)1844674407370955161ULL && dv > 5u))
					v->acc_overflow = true;
				else
					v->acc_value = v->acc_value * 10u + (uint64_t)dv;
			}
		}
		return true;
	}
	for (uint32_t i = start; i < len; i++) {
		uint8_t b = str[i];
		if (b == '.' ) { v->acc_has_dot = true; continue; }
		if ((b == 'e' || b == 'E') && base <= 14u) {
			v->acc_has_exp = true;
			v->acc_exp_state = 1;
			continue;
		}
		if (v->acc_exp_state > 0) {
			/* Inside the exponent: a sign or digit, never folded
			 * into the mantissa accumulator. */
			v->acc_exp_state = 2;
			continue;
		}
		uint32_t dv = bvn_char_to_digit((uint32_t)b, base);
		if (dv >= base) {
			v->last_error = error_digit_not_in_base;
			return false;
		}
		bvn_acc_digit(v, dv, base);
	}
	return true;
}
/*
 * Verify an integer literal fits the declared width/signedness.
 *
 * For widths > 64 bits the accumulator is useless, so it delegates to the
 * string-based arbitrary-precision range checks. For <=64 bits it computes the
 * exact bound from the width — note signed negatives allow one more magnitude
 * than positives (e.g. INT8 reaches -128 but only +127), handled by the is_neg
 * branch — and compares the accumulated magnitude. nan/inf pass through (they
 * are floats wearing a numeric token). A sticky overflow flag is an immediate
 * out-of-range.
 */
bool bvn_check_acc_range(bvnr_validator_t* v,
	const uint8_t* str, uint32_t str_len, token_type_t tt)
{
	value_type_spec_t vt   = v->value_type;
	/* datetime carries a decimal epoch-seconds value (base holds the epoch
	 * index, not a numeric base), validated as a signed integer. */
	uint32_t          base = (vt.family == vt_datetime)
		? 10u : bvn_effective_base_or_10(vt);
	/* Bases 64/85 are unsigned-only and use '+'/'-' as digits, so a leading '-'
	 * is never a negative sign there. */
	bool is_neg = (base != 64u && base != 85u && str_len > 0 && str[0] == '-');
	if (vt.family != vt_uint && vt.family != vt_sint && vt.family != vt_datetime)
		return true;
	/* The special-number keywords are range-exempt (spec 6.4) -- the KEYWORDS,
	 * which arrive as a number token. A quoted string under uint/sint is digits
	 * in the declared base, and from base 24 up 'n', 'a', 'i' and 'f' are all
	 * digits: <uint:8,_36> "inf" is 24171. Testing the spelling alone exempted
	 * it from the range check, so it was accepted as a uint:8. */
	if (str_len > 0 &&
	    (tt == token_is_number || tt == token_is_array_number)) {
		uint8_t c0 = str[0];
		if ((c0 == 'n' || c0 == 'i') &&
		    bvn_is_special_number_string((const char*)str))
			return true;
	}
	uint32_t w = bvn_effective_width(vt);
	if (w > 64u) {
		bool in_range = (vt.family == vt_uint)
			? bvn_validate_uint_range((const char*)str, w, base)
			: bvn_validate_sint_range((const char*)str, w, base);
		if (!in_range)
			v->last_error = error_value_out_of_range;
		return in_range;
	}
	if (v->acc_overflow) {
		v->last_error = error_value_out_of_range;
		return false;
	}
	uint64_t acc = v->acc_value;
	if (vt.family == vt_uint) {
		uint64_t max_val = (w >= 64u) ? UINT64_MAX : (1ULL << w) - 1ULL;
		if (acc > max_val) {
			v->last_error = error_value_out_of_range;
			return false;
		}
	} else {
		uint64_t max_mag;
		if (is_neg)
			max_mag = (w >= 64u) ? (1ULL << 63u) : (1ULL << (w - 1));
		else
			max_mag = (w >= 64u) ? (uint64_t)INT64_MAX : (1ULL << (w - 1)) - 1ULL;
		if (acc > max_mag) {
			v->last_error = error_value_out_of_range;
			return false;
		}
	}
	return true;
}
/*
 * Gatekeeper: does the actual token kind agree with the declared type family?
 *
 * Enforces the core type rules — utf8 demands a string, bool demands a bool
 * token, numeric types demand a number (or a string that parses as a number in
 * the declared base), and dot/exponent are only allowed for the float
 * families. plain (untyped) and null always pass. When the checks pass for an
 * integer type it tail-calls the range check, so this one function is the
 * single "is this value legal for its type" decision point.
 */
bool bvn_validate_type_value_compat(bvnr_reader_t* r,
	token_type_t tt, const uint8_t* str, uint32_t str_len)
{
	bvnr_validator_t* v  = &r->val;
	value_type_spec_t vt = v->value_type;
	if (bvn_type_is_plain(vt) || tt == token_is_null_value)
		return true;
	if (vt.family == vt_utf8) {
		if (tt != token_is_string && tt != token_is_array_string) {
			v->last_error = error_type_value_mismatch;
			return false;
		}
		return true;
	}
	if (vt.family == vt_bool) {
		if (tt != token_is_bool) {
			v->last_error = error_type_value_mismatch;
			return false;
		}
		return true;
	}
	if (!bvn_type_is_numeric(vt) && vt.family != vt_datetime)
		return true;
	if (tt != token_is_number    && tt != token_is_array_number &&
		tt != token_is_string    && tt != token_is_array_string) {
		v->last_error = error_type_value_mismatch;
		return false;
	}
	/* A datetime is a bare decimal epoch-seconds integer; a string carrier
	 * (the base-in-a-string form, e.g. <uint:_16> "ff") does not apply. */
	if (vt.family == vt_datetime &&
	    (tt == token_is_string || tt == token_is_array_string)) {
		v->last_error = error_type_value_mismatch;
		return false;
	}
	if (tt == token_is_string || tt == token_is_array_string) {
		const char* s = (const char*)str;
		uint32_t base = bvn_effective_base(vt);
		if (!bvn_validate_number_in_base(s, base)) {
			v->last_error = error_digit_not_in_base;
			return false;
		}
	}
	if (v->acc_has_exp && vt.family != vt_float &&
		vt.family != vt_float_fix && vt.family != vt_float_dec) {
		v->last_error = error_type_value_mismatch;
		return false;
	}
	if (v->acc_has_dot && vt.family != vt_float &&
		vt.family != vt_float_fix && vt.family != vt_float_dec) {
		v->last_error = error_type_value_mismatch;
		return false;
	}
	/*
	 * Fixed-point range: a value outside the declared Q-format's signed range
	 * (e.g. 1000000 in float_fix:16,q8, whose range is [-128, 127.996]) is
	 * error_value_out_of_range, mirroring the uint/sint overflow check. Without
	 * this the literal would be accepted and then silently saturate/wrap when an
	 * application converts it to the fixed-point wire form. Special numbers
	 * (nan/inf/ninf) are range-exempt (§6.4) and skipped.
	 */
	if (vt.family == vt_float_fix && str_len > 0) {
		uint8_t c0 = str[0];
		bool special = (c0 == 'n' || c0 == 'i') &&
					   bvn_is_special_number_string((const char*)str);
		if (!special &&
			!bvn_float_str_fits_fix((const char*)str, 10u,
									bvn_effective_width(vt),
									bvn_effective_q(vt))) {
			v->last_error = error_value_out_of_range;
			return false;
		}
	}
	return bvn_check_acc_range(v, str, str_len, tt);
}
/*
 * The main token entry point — the validator's dispatch core.
 *
 * Handles each raw token kind in turn:
 *  - type annotation: parse (via cache) into value_type + unit, then replay the
 *    annotation as the structured event burst the writer expects; a unit bumps
 *    parsed_unit_serial so array frames can tell whether the unit changed.
 *  - symbol: recognise the reserved keywords null/true/false/on/off and rewrite
 *    them into null/bool tokens (bovnar has no separate keyword token kind).
 *  - inline unit: parse (via cache) the trailing unit and reconcile it with any
 *    annotation unit — a mismatch is error_unit_mismatch, the rule that "42 m"
 *    with a <,kg> annotation is illegal.
 *  - bare value: infer a default type, fold the accumulator, run the
 *    type/value/range checks, then emit the verified data event.
 * Every path ends by emitting the value (or its rewrite) through bvn_emit_both.
 */
/*
 * The token_is_type arm of bvn_val_receive, split out so that IT carries the big
 * stack frame and the hot path does not.
 *
 * The buffers here are unavoidable -- a unit parameter is up to 255 bytes and
 * bvn_parse_type_annotation writes into a caller buffer -- but they were being
 * paid for by every OTHER token as well: the compiler allocates one frame at
 * function entry, so bvn_val_receive opened with "sub $0xcf8,%rsp" (3320 bytes)
 * plus a stack-protector canary on all 7.5 million calls the array benchmark
 * makes, of which zero are type annotations. Bare array elements are the case
 * this hurts most, because they are the case with the most tokens per byte.
 *
 * noinline is the point of the split and not a hint: without it the compiler is
 * free to fold the frame straight back into the caller.
 */
#if defined(__GNUC__) || defined(__clang__)
__attribute__((noinline))
#elif defined(_MSC_VER)
__declspec(noinline)
#endif
static bool bvn_val_receive_type(bvnr_reader_t* r,
                                 const bvnr_raw_token_t* raw)
{
	bvnr_validator_t* v = &r->val;
	bool type_ok = true, unit_ok = true, unit_too_long = false;
	value_unit_t parsed_unit = BVN_UNIT_NO_PREFIX(bu_none);
	uint8_t unit_buf[UINT8_MAX + 1];
	uint8_t ulen = 0;
	bvnr_data_t d = {0};
	d.type       = token_is_type;
	d.value_type = BVN_TYPE_PLAIN;
	d.value_unit = BVN_UNIT_NO_PREFIX(bu_none);
	d.data       = raw->type_data;
	d.length     = raw->type_len;
	if (!bvn_emit_unverified(r, ev_type_annotation_start, &d))
		return false;
	uint32_t tcache_hit_idx = (uint32_t)-1;
	for (uint32_t i = 0; i < v->tcache_count; i++) {
		const bvn_type_cache_slot_t* sl = &v->tcache[i];
		if (sl->key_len == raw->type_len &&
		    memcmp(sl->key, raw->type_data, raw->type_len) == 0) {
			tcache_hit_idx = i;
			break;
		}
	}
	if (tcache_hit_idx != (uint32_t)-1) {
		const bvn_type_cache_slot_t* sl =
			&v->tcache[tcache_hit_idx];
		v->value_type = sl->vtype;
		parsed_unit   = sl->unit;
		ulen          = sl->ubuf_len;
		if (ulen)
			memcpy(unit_buf, sl->ubuf, ulen);
		type_ok       = sl->type_ok;
		unit_ok       = sl->unit_ok;
		unit_too_long = sl->unit_too_long;
	} else {
		v->value_type = bvn_parse_type_annotation(
			raw->type_data, raw->type_len,
			&type_ok, &unit_ok, &unit_too_long, &parsed_unit,
			unit_buf, &ulen);
		if (raw->type_len <= BVN_TYPE_CACHE_KEY_CAP &&
		    ulen <= BVN_TYPE_CACHE_UBUF_CAP) {
			uint32_t idx;
			if (v->tcache_count < BVN_TYPE_CACHE_SLOTS) {
				idx = v->tcache_count;
				v->tcache_count = (uint8_t)(idx + 1u);
			} else {
				idx = v->tcache_next;
				v->tcache_next = (uint8_t)((idx + 1u)
					% BVN_TYPE_CACHE_SLOTS);
			}
			bvn_type_cache_slot_t* sl = &v->tcache[idx];
			sl->key_len       = (uint16_t)raw->type_len;
			memcpy(sl->key, raw->type_data, raw->type_len);
			sl->vtype         = v->value_type;
			sl->unit          = parsed_unit;
			sl->ubuf_len      = ulen;
			if (ulen)
				memcpy(sl->ubuf, unit_buf, ulen);
			sl->type_ok       = type_ok;
			sl->unit_ok       = unit_ok;
			sl->unit_too_long = unit_too_long;
		}
	}
	if (!type_ok) {
		v->last_error = error_illegal_value_type;
		return false;
	}
	/* The datetime family is spec 1.1; in a 1.0/unversioned document it is
	 * not a recognised type (error_illegal_value_type), preserving the
	 * unversioned==1.0 contract. */
	if (v->value_type.family == vt_datetime &&
	    !bvn_lex_supports_1_1(&r->lex)) {
		v->last_error = error_illegal_value_type;
		return false;
	}
	/*
	 * Prime the default-annotation suppression state so a following
	 * bare array element of the same effective type is suppressed
	 * identically to a synthesised-default run — without this the
	 * canonical serialiser is not idempotent on heterogeneous arrays.
	 */
	v->inferred_default_vtype = v->value_type;
	v->parsed_unit   = parsed_unit;
	/*
	 * Bump the serial only for a genuinely dimensioned unit. "No unit" has
	 * two encodings here: bvn_parse_type_annotation yields BVN_UNIT_NONE
	 * (num_components == 0) for an absent/no_unit annotation, while
	 * BVN_UNIT_IS_NO_UNIT matches the single-component bu_none shape. Test
	 * both so a unitless annotation never spuriously advances the serial.
	 */
	if (parsed_unit.num_components != 0u && !BVN_UNIT_IS_NO_UNIT(parsed_unit))
		bvn_val_set_unit_src(v, unit_buf, ulen);
	/*
	 * The serial moves for EVERY annotation, with a unit or without. It used to
	 * move only for a dimensioned one, on the theory that "no unit" changes
	 * nothing -- but inside an array it does: `<float:64,s>[<float:64> 1, 3]`
	 * sets parsed_unit to "none" for the first element, and with the serial
	 * unmoved the array frame saw no reason to put `s` back. The 3 was then
	 * delivered with no unit at all, a value losing its unit in silence.
	 */
	v->parsed_unit_serial++;
	v->unit_data_len = ulen;
	if (ulen > 0)
		v->has_annotation_unit = true;
	d.value_type = v->value_type;
	d.value_unit = v->parsed_unit;
	if (!bvn_emit_verified(r, ev_type_annotation_start, &d))
		return false;
	if (!bvn_emit_both(r, ev_type_annotation_type_family, &d))
		return false;
	if (v->value_type.width != 0) {
		d.type   = token_is_type_width;
		d.data   = NULL;
		d.length = 0;
		if (!bvn_emit_both(r,
				ev_type_annotation_type_family_parameter,
				&d))
			return false;
	}
	if (v->value_type.family == vt_datetime) {
		/* base holds the epoch index; emit it as a named parameter
		 * (e.g. "tai") rather than a numeric base. Index 0 (unix) is the
		 * default and is left implicit so <datetime> round-trips. */
		if (v->value_type.base != 0) {
			const char* ep =
				bvnr_datetime_epoch_name(v->value_type);
			d.type   = token_is_unit;
			d.data   = ep;
			d.length = (uint32_t)strlen(ep);
			if (!bvn_emit_both(r,
					ev_type_annotation_type_family_parameter,
					&d))
				return false;
		}
	} else if (v->value_type.base != 0) {
		d.type = (v->value_type.family == vt_float_fix)
			? token_is_type_q
			: token_is_type_base;
		d.data   = NULL;
		d.length = 0;
		if (!bvn_emit_both(r,
				ev_type_annotation_type_family_parameter,
				&d))
			return false;
	}
	if (ulen > 0) {
		/*
		 * spec 1.2 — the unit PROFILE notation ("ucum:<code>") is gated on
		 * the declared version, exactly as the datetime family and the
		 * \x/\u escapes are gated on 1.1. Without this a document
		 * declaring 1.0 or 1.1 would carry a unit that every conforming
		 * reader of that version must reject, which is the interoperability
		 * hazard the version directive exists to prevent.
		 *
		 * The error is error_unit_illegal, not one of the profile codes: in
		 * a pre-1.2 document "ucum:mm[Hg]" is simply not a unit, the same
		 * way <datetime> in a 1.0 document is simply not a value type. A
		 * document with no directive declares nothing and gets neither
		 * surface.
		 */
		if (v->value_type.family != vt_utf8 &&
			bvni_unit_has_profile((const char*)unit_buf, ulen) &&
			!bvn_lex_supports_1_2(&r->lex)) {
			v->last_error = error_unit_illegal;
			return false;
		}
		if (v->value_type.family != vt_utf8 && !unit_ok) {
			/* spec 1.2 — a profile unit distinguishes "that is not a unit"
			 * from "valid UCUM this build cannot carry" and from "no such
			 * profile"; bvn_unit_error_code re-parses to tell them apart, on
			 * the error path only. A natively spelled unit still yields
			 * error_unit_illegal, unchanged. */
			v->last_error = unit_too_long
							? error_unit_too_long
							: bvn_unit_error_code(unit_buf, ulen);
			return false;
		}
		d.type   = token_is_unit;
		d.data   = unit_buf;
		d.length = ulen;
		if (!bvn_emit_both(r,
				ev_type_annotation_type_family_parameter,
				&d))
			return false;
	}
	d.type   = token_is_type;
	d.data   = raw->type_data;
	d.length = raw->type_len;
	if (!bvn_emit_both(r, ev_type_annotation_end, &d))
		return false;
	return true;
}

bool bvn_val_receive(bvnr_reader_t* r, const bvnr_raw_token_t* raw)
{
	bvnr_validator_t* v = &r->val;
	token_type_t      tt = raw->type;
	/* Every assignment key passes through here as an identifier; remembering it
	 * is the whole of what per-field rules need from the token stream. Gated on
	 * there being rules at all, so a reader without them pays one test. */
	if (tt == token_is_identifier && v->policy.num_rules)
		bvn_path_set_key(&v->path, raw->str_data, raw->str_len);
	if (tt == token_is_type)
		return bvn_val_receive_type(r, raw);
	if (tt == token_is_symbol) {
		const uint8_t* s = raw->str_data;
		uint32_t       n = raw->str_len;
		bool           kw_bool = false, kw_true = false;
		if (n == 4 && memcmp(s, "null", 4) == 0) {
			tt = token_is_null_value;
		} else if ((n == 4 && memcmp(s, "true", 4) == 0) ||
		           (n == 2 && memcmp(s, "on", 2) == 0)) {
			kw_bool = true; kw_true = true;
		} else if ((n == 5 && memcmp(s, "false", 5) == 0) ||
		           (n == 3 && memcmp(s, "off", 3) == 0)) {
			kw_bool = true; kw_true = false;
		} else if ((n == 3 && (memcmp(s, "nan", 3) == 0 ||
		                       memcmp(s, "inf", 3) == 0)) ||
		           (n == 4 && memcmp(s, "ninf", 4) == 0)) {
			/* Special floats are bare reserved keywords (like null/
			 * true/false). Reclassify to a number token and fall
			 * through to the bare-value path, which synthesises the
			 * default <float:64> and applies special-number handling
			 * via bvn_is_special_number_string on raw->str_data.
			 * Array context is carried structurally by the array-row
			 * events, so (as for null/bool) a single token type
			 * suffices regardless of nesting. */
			tt = token_is_number;
		}
		if (kw_bool) {
			if (bvn_type_is_plain(v->value_type)) {
				if (!bvn_emit_default_type_annotation(r,
						token_is_bool, s, n, false,
						NULL, 0, BVN_UNIT_NO_PREFIX(bu_none)))
					return false;
			} else if (v->value_type.family != vt_bool) {
				v->last_error = error_type_value_mismatch;
				return false;
			}
			bvnr_data_t d = {0};
			d.type       = token_is_bool;
			d.value_type = v->value_type;
			d.value_unit = v->parsed_unit;
			d.data       = kw_true ? "true" : "false";
			d.length     = kw_true ? 4u : 5u;
			return bvn_emit_both(r, ev_data, &d);
		}
	}
	if (tt == token_is_null_value) {
		bvnr_data_t d = {0};
		d.type       = tt;
		d.value_type = v->value_type;
		d.value_unit = v->parsed_unit;
		d.data       = NULL;
		d.length     = 0;
		return bvn_emit_both(r, raw->event, &d);
	}
	bool         iu_have = false;
	bool         iu_ok   = true;
	value_unit_t iu_unit = BVN_UNIT_NO_PREFIX(bu_none);
	if (raw->inline_unit_len > 0) {
		uint32_t iu_hit_idx = (uint32_t)-1;
		for (uint32_t i = 0; i < v->iucache_count; i++) {
			const bvn_iu_cache_slot_t* sl = &v->iucache[i];
			if (sl->key_len == raw->inline_unit_len &&
			    memcmp(sl->key, raw->inline_unit_data,
			           raw->inline_unit_len) == 0) {
				iu_hit_idx = i;
				break;
			}
		}
		if (iu_hit_idx != (uint32_t)-1) {
			iu_unit = v->iucache[iu_hit_idx].unit;
			iu_ok   = v->iucache[iu_hit_idx].ok;
		} else {
			iu_unit = bvn_parse_unit(raw->inline_unit_data, &iu_ok);
			if (raw->inline_unit_len <= BVN_IU_CACHE_KEY_CAP) {
				uint32_t idx;
				if (v->iucache_count < BVN_IU_CACHE_SLOTS) {
					idx = v->iucache_count;
					v->iucache_count = (uint8_t)(idx + 1u);
				} else {
					idx = v->iucache_next;
					v->iucache_next = (uint8_t)((idx + 1u)
						% BVN_IU_CACHE_SLOTS);
				}
				bvn_iu_cache_slot_t* sl = &v->iucache[idx];
				sl->key_len = (uint8_t)raw->inline_unit_len;
				memcpy(sl->key, raw->inline_unit_data,
				       raw->inline_unit_len);
				sl->unit = iu_unit;
				sl->ok   = iu_ok;
			}
		}
		iu_have = true;
	}
	const uint8_t* val_str = raw->str_data;
	uint32_t       val_len = raw->str_len;
	uint8_t        dt_numbuf[24];
	const uint8_t* dt_frac     = NULL;   /* ISO sub-second digits (spec 1.1) */
	uint32_t       dt_frac_len = 0;
	bool           is_iso_dt =
		(tt == token_is_number || tt == token_is_array_number) &&
		bvn_str_is_iso_datetime(raw->str_data, raw->str_len);
	if (is_iso_dt) {
		/* spec 1.1: an ISO-8601 literal is not a recognised value in a
		 * 1.0/unversioned document — mirror the datetime-family gating. */
		if (!bvn_lex_supports_1_1(&r->lex)) {
			v->last_error = error_illegal_value_type;
			return false;
		}
		/* a timestamp carries no unit. */
		if (iu_have) {
			v->last_error = error_unit_illegal;
			return false;
		}
	}
	if (bvn_type_is_plain(v->value_type) &&
		(tt == token_is_number || tt == token_is_array_number ||
		 tt == token_is_string || tt == token_is_array_string)) {
		bool is_currency_unit = false;
		if (iu_have &&
		    (tt == token_is_number || tt == token_is_array_number)) {
			if (iu_ok && iu_unit.num_components == 1 &&
			    bvn_unit_is_currency((int)iu_unit.components[0].base))
				is_currency_unit = true;
		}
		const uint8_t* fold_unit_str = NULL;
		uint32_t       fold_unit_len = 0;
		value_unit_t   fold_unit_val = BVN_UNIT_NO_PREFIX(bu_none);
		if (iu_have && iu_ok &&
		    (tt == token_is_number || tt == token_is_array_number)) {
			fold_unit_str = raw->inline_unit_data;
			fold_unit_len = raw->inline_unit_len;
			fold_unit_val = iu_unit;
		}
		/* The synthesised annotation is a VERIFIED event, so the inline unit
		 * it may carry has to have passed first: `.o = 100 zzz;` delivered a
		 * verified <uint:64,_10,no_unit> before its unit_illegal, and an
		 * ungated `ucum:km/h` reached on_verified as a unit (and a writer fed
		 * from it then failed instead of the reader). The same two checks
		 * run again below for the annotated path. */
		if (iu_have &&
		    bvni_unit_has_profile((const char*)raw->inline_unit_data,
		                          raw->inline_unit_len) &&
		    !bvn_lex_supports_1_2(&r->lex)) {
			v->last_error = error_unit_illegal;
			return false;
		}
		if (iu_have && !iu_ok) {
			v->last_error = bvn_unit_error_code(
				raw->inline_unit_data, raw->inline_unit_len);
			return false;
		}
		if (!bvn_emit_default_type_annotation(r, tt,
				raw->str_data, raw->str_len, is_currency_unit,
				fold_unit_str, fold_unit_len, fold_unit_val))
			return false;
	}
	if (is_iso_dt) {
		/* The annotation — explicit, or the <datetime:64,unix> just
		 * inferred above — must be datetime; convert the UTC civil literal
		 * to epoch seconds and substitute the decimal integer so the range
		 * check and emission below run the ordinary datetime-integer path. */
		if (v->value_type.family != vt_datetime) {
			v->last_error = error_type_value_mismatch;
			return false;
		}
		int64_t secs;
		if (!bvn_iso_to_epoch_seconds(raw->str_data, raw->str_len,
				v->value_type, &secs, &v->last_error,
				&dt_frac, &dt_frac_len))
			return false;
		val_len = bvn_i64_to_decimal(dt_numbuf, secs);
		val_str = dt_numbuf;
	}
	if (tt == token_is_number || tt == token_is_array_number ||
		tt == token_is_string || tt == token_is_array_string) {
		bvn_acc_reset(v);
		if (!bvn_acc_parse_number(v, val_str, val_len, tt))
			return false;
	}
	if (!bvn_validate_type_value_compat(r, tt, val_str, val_len))
		return false;
	if (iu_have) {
		/* No datetime carries a unit: a timestamp is a count of seconds since an
		 * epoch, not a dimensioned quantity. The ISO-literal form is rejected
		 * above; this also rejects the plain integer carrier, e.g.
		 * <datetime:64> 100 m, which would otherwise be accepted (and the unit
		 * then silently dropped on emit). */
		if (v->value_type.family == vt_datetime) {
			v->last_error = error_unit_illegal;
			return false;
		}
		/* spec 1.2 — the profile notation is gated here too; an inline unit is
		 * the same unit slot reached by another route. */
		if (bvni_unit_has_profile((const char*)raw->inline_unit_data,
		                          raw->inline_unit_len) &&
			!bvn_lex_supports_1_2(&r->lex)) {
			v->last_error = error_unit_illegal;
			return false;
		}
		if (!iu_ok) {
			v->last_error = bvn_unit_error_code(
				raw->inline_unit_data, raw->inline_unit_len);
			return false;
		}
		if (v->has_annotation_unit &&
			!bvn_unit_equal(v->parsed_unit, iu_unit)) {
			v->last_error = error_unit_mismatch;
			return false;
		}
		/* Where the annotation named the unit too, its spelling stands: that
		 * is the one a re-serialisation writes, the inline suffix being
		 * dropped as redundant once the annotation carries the unit.
		 *
		 * The UNIT stands with it. The two compared equal just above, but equal
		 * is order-insensitive (`s·m` is `m·s`), so taking the suffix's struct
		 * while keeping the annotation's text would hand out a value_unit and
		 * a unit_source from two different parses of one unit. */
		if (!v->has_annotation_unit) {
			bvn_val_set_unit_src(v, raw->inline_unit_data,
					     raw->inline_unit_len);
			v->parsed_unit = iu_unit;
		}
		if (!BVN_UNIT_IS_NO_UNIT(iu_unit))
			v->parsed_unit_serial++;
	}
	bvnr_data_t d = {0};
	d.type       = tt;
	d.value_type = v->value_type;
	d.value_unit = v->parsed_unit;
	if (tt != token_is_null_value) {
		d.data   = val_str;
		d.length = val_len;
	} else {
		d.data   = NULL;
		d.length = 0;
	}
	/* spec 1.1 — carry the verbatim ISO sub-second digits (NULL/0 unless this
	 * is a datetime literal that had a `.frac` part). */
	d.frac_data   = dt_frac;
	d.frac_length = dt_frac_len;
	/* library 2.0 — and the unit's own spelling. Attached here rather than left to
	 * the emit below so the assertion pass and the want_unit hook see the same
	 * event the callbacks will. */
	bvn_attach_unit_src(v, &d);
	/* The policy's validation half, on the native unit — before any conversion
	 * can rewrite the unit the document actually wrote. */
	if (!bvn_apply_unit_assertions(r, tt, &d))
		return false;
	/* Optional read-time unit conversion (bvnr_read_flags_t.want_unit, or the
	 * unit policy). Runs after all type/unit validation, so it sees the value in
	 * its native unit and can rewrite it (or abort with error_unit_mismatch). */
	if (!bvn_apply_want_unit(r, tt, &d))
		return false;
	return bvn_emit_both(r, raw->event, &d);
}
/*
 * Forward a payload-less structural event (struct/array open/close, stream
 * start/end, etc.). Short-circuits entirely when no callbacks are installed —
 * a validate-only run (no consumer) then does zero per-event work. The current
 * type/unit are attached so observers see the active context on every event.
 */
bool bvn_val_receive_event(bvnr_reader_t* r, bvnr_event_t ev)
{
	bvnr_validator_t* v = &r->val;
	/* Struct depth is what turns a key into a path, and it has to be tracked
	 * BEFORE the no-callbacks short-circuit below: a validate-only run with
	 * per-field rules installs no callbacks at all, and would otherwise see
	 * every value at top level. */
	if (v->policy.num_rules) {
		if (ev == ev_struct_start)    bvn_path_push(&v->path);
		else if (ev == ev_struct_end) bvn_path_pop(&v->path);
	}
	if (!v->on_unverified && !v->on_verified) return true;
	bvnr_data_t d = {0};
	d.value_type = v->value_type;
	d.value_unit = v->parsed_unit;
	return bvn_emit_both(r, ev, &d);
}
bool bvn_val_receive_octet_chunk(
	bvnr_reader_t* r, const uint8_t* data, uint32_t len)
{
	bvnr_data_t d = {0};
	d.type       = token_is_octet_stream;
	d.value_type = BVN_TYPE_PLAIN;
	d.data       = data;
	d.length     = len;
	return bvn_emit_both(r, ev_data, &d);
}
/*
 * Reset per-value state at the start (and end) of each assignment so a type
 * annotation, unit, or accumulator left over from the previous value can never
 * leak into the next one. Both intro and outro reset identically; having both
 * makes the invariant hold regardless of which boundary the grammar hits first.
 */
bool bvn_val_on_value_intro(bvnr_reader_t* r)
{
	bvnr_validator_t* v = &r->val;
	v->value_type             = BVN_TYPE_PLAIN;
	v->parsed_unit            = BVN_UNIT_NO_PREFIX(bu_none);
	v->inferred_default_vtype = BVN_TYPE_PLAIN;
	v->has_annotation_unit    = false;
	bvn_acc_reset(v);
	return true;
}
bool bvn_val_on_value_outro(bvnr_reader_t* r)
{
	bvnr_validator_t* v = &r->val;
	v->value_type             = BVN_TYPE_PLAIN;
	v->parsed_unit            = BVN_UNIT_NO_PREFIX(bu_none);
	v->inferred_default_vtype = BVN_TYPE_PLAIN;
	v->has_annotation_unit    = false;
	bvn_acc_reset(v);
	return true;
}
bool bvn_val_on_array_intro(bvnr_reader_t* r)
{
	bvnr_data_t d = {0};
	d.value_type = r->val.value_type;
	d.value_unit = r->val.parsed_unit;
	bvn_acc_reset(&r->val);
	r->val.has_annotation_unit = false;
	return bvn_emit_unverified(r, ev_array_row_start, &d) &&
		   bvn_emit_verified(r, ev_array_row_start, &d);
}
/*
 * Closing a row enforces rectangularity: the first row to close establishes the
 * expected width (*array_row_size); every later sibling row must match it or it
 * is error_array_row_size_mismatch. bvn_val_on_new_array_value applies the same
 * bound element-by-element so an over-long row is caught as early as possible.
 */
bool bvn_val_on_array_outro(bvnr_reader_t* r,
	uint64_t curr_row_size, uint64_t* array_row_size)
{
	bvnr_validator_t* v = &r->val;
	bvnr_data_t       d = {0};
	/* UINT64_MAX is the "row width not yet established" sentinel: a width of 0
	 * is a real, valid width (an empty row, "[]"), so it must be distinguishable
	 * from "unset". The first row to close fixes the width; siblings must match. */
	if (*array_row_size == UINT64_MAX)
		*array_row_size = curr_row_size;
	else if (*array_row_size != curr_row_size) {
		v->last_error = error_array_row_size_mismatch;
		return false;
	}
	return bvn_emit_both(r, ev_array_row_end, &d);
}
bool bvn_val_on_new_array_value(bvnr_reader_t* r,
	uint64_t curr_row_size, uint64_t array_row_size)
{
	bvnr_validator_t* v = &r->val;
	if (array_row_size != UINT64_MAX && curr_row_size >= array_row_size) {
		v->last_error = error_array_row_size_mismatch;
		return false;
	}
	bvn_acc_reset(&r->val);
	v->has_annotation_unit = false;
	return true;
}
bool bvnr_read(bvnr_reader_t* r)
{
	/* Every other entry point tolerates a NULL handle, and an unopened reader is
	 * the same class of caller mistake -- but this one dereferenced straight
	 * through to a NULL pull function pointer and segfaulted. Both open paths
	 * (bvnr_open_read_source and bvnr_open_read_mem) install a pull, so a NULL
	 * one means no source was ever attached. */
	if (!r)
		return false;
	if (bvn_source_impl(&r->lex.src)->pull == NULL) {
		r->val.last_error = error_invalid_argument;
		return false;
	}
	return bvn_lex_run(r);
}
error_code_t bvnr_reader_get_error(const bvnr_reader_t* r)
{
	return r ? r->val.last_error : error_none;
}
uint64_t bvnr_reader_get_error_line(const bvnr_reader_t* r)
{
	return r ? r->val.error_line : 0;
}
uint64_t bvnr_reader_get_error_column(const bvnr_reader_t* r)
{
	return r ? r->val.error_column : 0;
}
uint32_t bvnr_reader_get_error_byte(const bvnr_reader_t* r)
{
	return r ? r->val.error_byte : 0;
}
uint64_t bvnr_reader_get_error_offset(const bvnr_reader_t* r)
{
	return r ? r->val.error_offset : 0;
}
uint64_t bvnr_reader_get_skipped_bytes(const bvnr_reader_t* r)
{
	return r ? r->lex.skipped_bytes : 0;
}
uint64_t bvnr_reader_get_recovery_count(const bvnr_reader_t* r)
{
	return r ? r->lex.recovery_count : 0;
}
bool bvnr_reader_get_declared_version(
	const bvnr_reader_t* r, uint16_t* major, uint16_t* minor)
{
	if (!r || !r->lex.has_declared_version)
		return false;
	if (major) *major = r->lex.declared_major;
	if (minor) *minor = r->lex.declared_minor;
	return true;
}
/*
 * Bind a reader to a source and arm it for bvnr_read. Tears down any previous
 * lexer state first so a single reader can be reused for many documents.
 * Requires a source with a real pull function (a zeroed/uninitialised source is
 * rejected rather than crashing). The optional dbg_sink tees raw input for the
 * canonicaliser. bvnr_open_read_mem is a thin convenience wrapper that builds a
 * memory source (and optional memory debug sink) and forwards here.
 */
bool bvnr_open_read_source(
	bvnr_reader_t* r, const bvnr_source_t* src,
	const bvnr_sink_t* dbg_sink, bvnr_read_flags_t* options)
{
	if (!r)
		return false;
	bvn_lex_destroy(&r->lex);
	memset(&r->lex, 0, sizeof(r->lex));
	if (!src || !bvn_source_impl_c(src)->pull)
		return false;
	if (!bvn_lex_init(&r->lex, src, dbg_sink, options))
		return false;
	bvn_val_init(&r->val, options);
	return true;
}
bool bvnr_open_read_mem(
	bvnr_reader_t* r, const void* buf, uint64_t len,
	void* dbg_buf, uint64_t dbg_cap,
	bvnr_read_flags_t* options)
{
	bvnr_source_t src;
	bvnr_sink_t   dbg;
	/* A NULL pointer with a nonzero length used to open successfully and then
	 * dereference the null in the lexer. buf and len arrive independently from
	 * the caller (the WASM entry points take them as separate arguments), so the
	 * pairing has to be checked here rather than assumed. A NULL buffer with
	 * length 0 stays valid — that is an empty document. */
	if (!buf && len > 0)
		return false;
	bvnr_source_from_mem(&src, buf, len);
	if (dbg_buf && dbg_cap)
		bvnr_sink_to_mem(&dbg, dbg_buf, dbg_cap);
	return bvnr_open_read_source(r, &src,
		(dbg_buf && dbg_cap) ? &dbg : NULL, options);
}
