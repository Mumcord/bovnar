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

#include <errno.h>
#include <math.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "bvn_port.h"
#include "bovnar.h"
#include "bovnar_dom.h"
#include "bovnar_si_units.h"
#include "bvn_dom_impl.h"
#include "bvn_float.h"
#ifndef BVN_DOM_FD_MAX_BYTES
#define BVN_DOM_FD_MAX_BYTES   (256u * 1024u * 1024u)
#endif
#ifndef BVN_DOM_OCT_MAX_BYTES
#define BVN_DOM_OCT_MAX_BYTES  (256u * 1024u * 1024u)
#endif
/*
 * ===========================================================================
 * DOM builder (event consumer that materialises the tree)
 * ===========================================================================
 *
 * This bridges the streaming reader and the DOM: it registers on_verified as
 * the reader's callback, and as events arrive it builds the bvn_dom_doc_t tree
 * defined in bovnar_dom.c. The only real state it needs is a scope stack — the
 * chain of containers currently open (document root, struct, or array) — so
 * that each value can be attached to the right parent. builder_attach inspects
 * the top scope to decide between doc_add / struct_add / array_append.
 *
 * Two subtleties:
 *  - Pending key: a struct/doc member arrives as an assignment-start (the key)
 *    followed later by its value event; the key is buffered until the value is
 *    built and attached.
 *  - Deferred array pop: an array row closes (ev_array_row_end) before we know
 *    whether a sibling dimension follows (ev_array_dim_start) — for
 *    multi-dimensional arrays the same array node must stay on the stack across
 *    rows. So the pop is deferred (pending_array_pop) and only applied when the
 *    next non-dimension event confirms the array is really finished.
 */
typedef enum bvn_scope_kind_e {
	BVN_SCOPE_DOC,
	BVN_SCOPE_STRUCT,
	BVN_SCOPE_ARRAY
} bvn_scope_kind_t;
typedef struct bvn_scope_s {
	bvn_scope_kind_t  kind;
	bvn_dom_node_t   *node;
	/* For an array scope: the element count at which the row now being read
	 * began, so ev_array_row_end can size it. It lives HERE, per array, and
	 * used to be one variable for the whole builder -- so an inner array
	 * opening reset it to 0 under the outer one, and the outer row's size was
	 * computed from the inner array's start: `[[1,2,3]/[4,5,6]]` recorded an
	 * outer row of 4294967294 elements, `[[1]/[2]] / [[3]/[4]]` indexed
	 * .a[1][0][0][0] to 1 instead of 3, and a valid `[[[1,1]/[2,2], null],
	 * [null, null]]` was refused as array_row_size_mismatch. */
	uint32_t          row_start_count;
} bvn_scope_t;
typedef struct bvn_builder_s {
	bvn_dom_doc_t  *doc;
	bvn_scope_t    *scopes;
	uint32_t        scope_count;
	uint32_t        scope_cap;
	char           *key;
	uint32_t        key_len;
	uint32_t        key_cap;
	bool            pending_array_pop;
	bool            array_dim_continue;
	uint8_t        *oct_buf;
	uint32_t        oct_len;
	uint32_t        oct_cap;
	bool            in_octet_stream;
	error_code_t    last_error;
	/* The last unit block made (builder_node_unit), then the last unit
	 * spelling judged there, and the
	 * verdict: an array repeats one spelling per element, and formatting the
	 * unit again for each would cost more than the copy it decides about. */
	bvn_dom_unit_t *unit_last;
	bool            usrc_keep;
	uint32_t        usrc_last_len;
	char            usrc_last[BVNR_UNIT_SOURCE_MAX + 1u];
} bvn_builder_t;
/*
 * Give a node its unit, and the unit's spelling when that is worth keeping. A
 * spelling equal to the canonical form of the unit is recoverable from the
 * unit, so it is not kept.
 *
 * The block is SHARED with the node before it wherever the two agree, which is
 * every element of an array under one annotation: the builder holds a reference
 * to the last block it made and hands it out again. One block back is enough --
 * a document alternating two units from value to value pays for a block each,
 * which is still less than every node paid before.
 *
 * `d` is the event the spelling comes from, or NULL for a node that has none to
 * keep: a container, an octet stream, or a value a policy converted, whose unit
 * is one the document never spelled.
 */
static bool builder_node_unit(bvn_builder_t *b, bvn_dom_node_t *nd,
		const value_unit_t *vu, const bvnr_data_t *d)
{
	const char *src = NULL;
	uint32_t    n   = 0u;
	if (d && d->unit_source && d->unit_source_length &&
	    d->unit_source_length <= BVNR_UNIT_SOURCE_MAX) {
		n = d->unit_source_length;
		if (n != b->usrc_last_len ||
		    memcmp(b->usrc_last, d->unit_source, n) != 0) {
			char    canon[BVNR_UNIT_STRING_MAX];
			int32_t cn = bvn_unit_to_string(d->value_unit, canon, sizeof canon);
			b->usrc_keep = !(cn == (int32_t)n &&
					 memcmp(canon, d->unit_source, n) == 0);
			b->usrc_last_len = n;
			memcpy(b->usrc_last, d->unit_source, n);
		}
		if (b->usrc_keep)
			src = (const char *)d->unit_source;
		else
			n = 0u;
	}
	bvn_dom_unit_t *u = NULL;
	if (b->unit_last && bvn_dom_unit_is(b->unit_last, vu, src, n))
		u = bvn_dom_unit_ref(b->unit_last);
	if (!u) {
		u = bvn_dom_unit_get(vu, src, n);
		if (!u)
			return false;
		if (!bvn_dom_unit_static(u)) {  /* worth remembering */
			bvn_dom_unit_release(b->unit_last);
			b->unit_last = bvn_dom_unit_ref(u);
		}
	}
	bvn_dom_node_take_unit(nd, u);
	return true;
}
static bool builder_init(bvn_builder_t *b, bvn_dom_doc_t *doc)
{
	memset(b, 0, sizeof(*b));
	b->doc = doc;
	return true;
}
static void builder_cleanup(bvn_builder_t *b)
{
	free(b->scopes);
	free(b->key);
	free(b->oct_buf);
	bvn_dom_unit_release(b->unit_last);
}
static bvn_scope_t *builder_top(bvn_builder_t *b)
{
	return b->scope_count ? &b->scopes[b->scope_count - 1u] : NULL;
}
static bool builder_push(bvn_builder_t *b, bvn_scope_kind_t kind,
						 bvn_dom_node_t *node)
{
	if (b->scope_count == b->scope_cap) {
		if (b->scope_cap > UINT32_MAX / 2u) return false;
		uint32_t nc = b->scope_cap ? b->scope_cap * 2u : 16u;
		bvn_scope_t *ns = realloc(b->scopes, nc * sizeof(*ns));
		if (!ns) return false;
		b->scopes   = ns;
		b->scope_cap = nc;
	}
	bvn_scope_t *s = &b->scopes[b->scope_count++];
	s->kind = kind;
	s->node = node;
	s->row_start_count = 0;
	return true;
}
static void builder_pop(bvn_builder_t *b)
{
	if (b->scope_count > 0)
		b->scope_count--;
}
static bool builder_set_key(bvn_builder_t *b, const char *data,
							uint32_t len)
{
	if (len == UINT32_MAX) return false;   /* len + 1u would wrap to 0 */
	if (len + 1u > b->key_cap) {
		uint32_t nc = len + 1u;
		char *nk = realloc(b->key, nc);
		if (!nk) return false;
		b->key     = nk;
		b->key_cap = nc;
	}
	memcpy(b->key, data, len);
	b->key[len] = '\0';
	b->key_len  = len;
	return true;
}
static void builder_clear_key(bvn_builder_t *b)
{
	b->key_len = 0;
	if (b->key) b->key[0] = '\0';
}
/*
 * Attach a freshly built value to the currently open container, dispatching on
 * the top scope kind: a document/struct member keyed by the buffered key, or an
 * array element (no key). On failure the value is owned/destroyed by the add
 * helper, so callers must not free it again.
 */
static bool builder_attach(bvn_builder_t *b, bvn_dom_node_t *val)
{
	bvn_scope_t *top = builder_top(b);
	if (!top) return false;
	if (top->kind == BVN_SCOPE_DOC) {
		return bvn_dom_doc_add_nocheck(b->doc, b->key, b->key_len, val);
	}
	if (top->kind == BVN_SCOPE_STRUCT) {
		bool ok = bvn_dom_struct_add_nocheck(top->node,
									 b->key, b->key_len, val);
		if (ok) builder_clear_key(b);
		return ok;
	}
	return bvn_dom_array_append_nocheck(top->node, val);
}
static bool builder_oct_append(bvn_builder_t *b, const uint8_t *data,
							   uint32_t len)
{
	if (len > UINT32_MAX - b->oct_len) return false;
	uint32_t need = b->oct_len + len;
	if (need > BVN_DOM_OCT_MAX_BYTES) return false;
	if (need > b->oct_cap) {
		uint32_t nc = b->oct_cap ? b->oct_cap : 256u;
		while (nc < need) {
			if (nc > UINT32_MAX / 2u) { nc = need; break; }
			nc *= 2u;
		}
		if (nc > BVN_DOM_OCT_MAX_BYTES) nc = BVN_DOM_OCT_MAX_BYTES;
		if (nc < need) return false;
		uint8_t *nb = realloc(b->oct_buf, nc);
		if (!nb) return false;
		b->oct_buf = nb;
		b->oct_cap = nc;
	}
	memcpy(b->oct_buf + b->oct_len, data, len);
	b->oct_len = need;
	return true;
}
static bvn_dom_node_t *make_null(value_type_spec_t vt)
{
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_NULL);
	if (!n) return NULL;
	n->value_type = vt;
	return n;
}
/*
 * Build an INT node from the literal text. The effective width decides storage:
 * >64 bits parses into a heap bvn_int_t (arbitrary precision), otherwise into
 * the inline int64. Each make_* builder follows the same contract — take the
 * value text plus its resolved type/unit, allocate the typed node, and clean up
 * fully on any failure. The strtoll/strtoull fallback covers plain (untyped)
 * literals the typed parser declines; an untyped value above INT64_MAX is
 * promoted to uint64 so it isn't misread as negative.
 */
static bvn_dom_node_t *make_int(const char *str, uint32_t len,
								value_type_spec_t vt)
{
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_INT);
	if (!n) return NULL;
	n->value_type = vt;
	uint32_t effective_width = vt.width ? vt.width : 64u;
	uint32_t base            = bvn_effective_base(vt);
	if (effective_width > 64u) {
		bvn_int_t *bi = bvn_int_alloc();
		if (!bi) { bvn_dom_node_destroy(n); return NULL; }
		char *wbuf = malloc((size_t)len + 1u);
		if (!wbuf) {
			bvn_int_free(bi);
			bvn_dom_node_destroy(n);
			return NULL;
		}
		memcpy(wbuf, str, (size_t)len);
		wbuf[len] = '\0';
		bool ok = bvn_int_from_str(bi, wbuf, base);
		free(wbuf);
		if (!ok) {
			bvn_int_free(bi);
			bvn_dom_node_destroy(n);
			return NULL;
		}
		n->val.bigint = bi;
		return n;
	}
	char     stkbuf[256];
	char    *buf;
	bool     buf_on_heap = false;
	if ((size_t)len + 1u > sizeof(stkbuf)) {
		buf = malloc((size_t)len + 1u);
		if (!buf) { bvn_dom_node_destroy(n); return NULL; }
		buf_on_heap = true;
	} else {
		buf = stkbuf;
	}
	memcpy(buf, str, (size_t)len);
	buf[len] = '\0';
	bool parse_ok = false;
	if (vt.family == vt_sint || vt.family == vt_datetime ||
	    (vt.family == vt_plain && buf[0] == '-')) {
		int64_t v = 0;
		if (bvn_parse_int64(buf, vt, &v)) {
			n->val.int_val = v;
			parse_ok = true;
		} else {
			char *end = NULL;
			errno = 0;
			long long ll = strtoll(buf, &end, 10);
			/* Defensive: the validator already range-checks a width-typed
			 * literal before it reaches the builder, but never store a value
			 * that does not fit the declared signed width, so the node's tag
			 * and payload can never disagree. (width 0 / >= 64 always fit.) */
			bool fits = !(vt.width >= 1u && vt.width < 64u);
			if (!fits) {
				int64_t lim = (int64_t)1 << (vt.width - 1u);
				fits = ((int64_t)ll >= -lim && (int64_t)ll <= lim - 1);
			}
			if (errno == 0 && end && *end == '\0' && fits) {
				n->val.int_val = (int64_t)ll;
				parse_ok = true;
			}
		}
	} else {
		uint64_t v = 0;
		if (bvn_parse_uint64(buf, vt, &v)) {
			n->val.int_val = (int64_t)v;
			parse_ok = true;
		} else {
			char *end = NULL;
			errno = 0;
			unsigned long long ull = strtoull(buf, &end, 10);
			/* Same defensive width clamp as the signed path above. */
			bool fits = !(vt.width >= 1u && vt.width < 64u) ||
			            ((uint64_t)ull < ((uint64_t)1 << vt.width));
			if (errno == 0 && end && *end == '\0' && fits) {
				v = (uint64_t)ull;
				n->val.int_val = (int64_t)v;
				parse_ok = true;
			}
		}
		if (parse_ok && vt.family == vt_plain && v > (uint64_t)INT64_MAX) {
			n->value_type = BVN_TYPE_UINT_BASE(64u, 0u);
		}
	}
	if (buf_on_heap) free(buf);
	if (!parse_ok) { bvn_dom_node_destroy(n); return NULL; }
	return n;
}
/*
 * A decimal literal as the nearest double, without paying for digits that
 * cannot matter.
 *
 * bvn_parse_double_in_base is exact: it builds the literal as a rational and
 * rounds once. That is what makes it correct, and it is also why its cost
 * follows the MAGNITUDE OF THE EXPONENT rather than the length of the text --
 * "1e-99999" is eight bytes and a hundred-thousand-digit denominator. Each such
 * literal cost milliseconds, so a 180 kB array of them held `bovnar validate`
 * for 84 seconds: the document tier's running time was set by the shape of the
 * input, not its size.
 *
 * Nothing about the answer needs that work. A binary64 spans roughly 1e-324 to
 * 1.8e308, so the position of the leading significant digit settles most
 * literals outright:
 *
 *   - leading digit at 10^309 or above: the value is at least 1e309, past the
 *     largest double by any rounding -- infinity;
 *   - leading digit at 10^-326 or below: the value is under 1e-325, less than
 *     half the smallest subnormal (2.47e-324) -- zero;
 *   - all digits zero: zero, whatever the exponent says.
 *
 * Everything between goes to the exact parser, but NORMALISED first: leading
 * and trailing zeros are folded into the exponent (so 0.000...001e400 and
 * 1000...0e-400 arrive as "1e..." with a small exponent, not as hundreds of
 * digits against a large one), and the significant digits are cut at
 * BVN_DEC_SIG_MAX with one sticky '1' standing for a non-zero remainder. That
 * cut is exact: the boundary between two doubles is a number of at most 767
 * significant decimal digits, so no digit past that can move a value across
 * one -- it can only say "not exactly on it", which is what the sticky digit
 * says. The exact parser then sees an exponent within a few hundred of zero
 * and at most 801 digits, for any input.
 *
 * The result is bit-identical to handing the original text to the exact
 * parser; tests/bovnar_dom_test.c holds it to that at every boundary named
 * here. Returns false -- and the caller falls back to the exact path -- for
 * anything that is not a plain decimal literal.
 */
#define BVN_DEC_SIG_MAX 800u
static bool builder_decimal_to_double(const char *s, double *out)
{
	const char *p   = s;
	bool        neg = false;
	if (*p == '-') { neg = true; p++; }
	const char *mant = p;
	int64_t ndig = 0, point = -1, first_nz = -1, last_nz = -1;
	for (;; p++) {
		if (*p >= '0' && *p <= '9') {
			if (*p != '0') {
				if (first_nz < 0) first_nz = ndig;
				last_nz = ndig;
			}
			ndig++;
		} else if (*p == '.' && point < 0) {
			point = ndig;
		} else {
			break;
		}
	}
	if (ndig == 0) return false;
	if (point < 0) point = ndig;
	int64_t e10 = 0;
	if (*p == 'e' || *p == 'E') {
		bool eneg = false;
		p++;
		if (*p == '+' || *p == '-') { eneg = (*p == '-'); p++; }
		if (*p < '0' || *p > '9') return false;
		for (; *p >= '0' && *p <= '9'; p++) {
			/* Saturate: past this the verdict cannot change, and the digits
			 * of an exponent must not be able to overflow the counter. */
			if (e10 < (int64_t)1000000000000)
				e10 = e10 * 10 + (*p - '0');
		}
		if (eneg) e10 = -e10;
	}
	if (*p != '\0') return false;
	if (first_nz < 0) {
		*out = neg ? -0.0 : 0.0;
		return true;
	}
	/* The power of ten of the leading significant digit. */
	int64_t lead = point - 1 - first_nz + e10;
	if (lead >= 309) {
		*out = neg ? -INFINITY : INFINITY;
		return true;
	}
	if (lead <= -326) {
		*out = neg ? -0.0 : 0.0;
		return true;
	}
	char     norm[BVN_DEC_SIG_MAX + 48u];
	size_t   n = 0;
	int64_t  idx = 0, kept = 0;
	bool     sticky = false;
	if (neg) norm[n++] = '-';
	for (const char *q = mant; idx <= last_nz; q++) {
		if (*q == '.') continue;
		if (idx >= first_nz) {
			if (kept < (int64_t)BVN_DEC_SIG_MAX) {
				norm[n++] = *q;
				kept++;
			} else if (*q != '0') {
				sticky = true;
			}
		}
		idx++;
	}
	if (sticky) { norm[n++] = '1'; kept++; }
	/* The digits now stand as one integer, so the exponent is the leading
	 * digit's power less the digits after it. */
	snprintf(norm + n, sizeof(norm) - n, "e%lld",
		 (long long)(lead - (kept - 1)));
	return bvn_parse_double_in_base(norm, 10u, out);
}
static bvn_dom_node_t *make_float(const char *str, uint32_t len,
								  value_type_spec_t vt)
{
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_FLOAT);
	if (!n) return NULL;
	n->value_type = vt;
	char  stkbuf[256];
	char *buf;
	bool  buf_on_heap = false;
	if ((size_t)len + 1u > sizeof(stkbuf)) {
		buf = malloc((size_t)len + 1u);
		if (!buf) { bvn_dom_node_destroy(n); return NULL; }
		buf_on_heap = true;
	} else {
		buf = stkbuf;
	}
	memcpy(buf, str, (size_t)len);
	buf[len] = '\0';
	if (bvn_is_special_number_string(buf)) {
		if      (strcmp(buf, "nan")  == 0) n->val.float_val = NAN;
		else if (strcmp(buf, "inf")  == 0) n->val.float_val = INFINITY;
		else if (strcmp(buf, "ninf") == 0) n->val.float_val = -INFINITY;
	} else {
		uint32_t base = bvn_effective_base(vt);
		double v = 0.0;
		/*
		 * Convert through the canonical decimal/base parser. For base 10 this
		 * takes the exact-rational single-rounding path, which is correctly
		 * rounded across the whole binary64 range including subnormals. The
		 * former local route -- parse into a finite-precision bvn_float, then
		 * bvn_float_to_double -- rounded twice and was 1 ULP off for subnormal
		 * results (the stored value is a double regardless of the parse width,
		 * so nothing is lost by delegating). For non-decimal bases this still
		 * rejects a literal the parser cannot consume.
		 */
		if (!(base == 10u && builder_decimal_to_double(buf, &v)) &&
		    !bvn_parse_double_in_base(buf, base, &v)) {
			if (buf_on_heap) free(buf);
			bvn_dom_node_destroy(n);
			return NULL;
		}
		n->val.float_val = v;
	}
	if (buf_on_heap) free(buf);
	return n;
}
/*
 * num/den as the nearest double.
 *
 * For a value a unit policy converted into a base its type has no literal in
 * (see the ev_data handler): the digits in conv.text are then unreadable as a
 * float, but the exact rational travels beside them. The quotient is taken to
 * 64 significant bits with a sticky low bit, so the one rounding that happens
 * -- uint64 to double, to nearest-even -- sees everything it needs and the
 * result is correctly rounded across the normal range.
 */
static bool builder_rational_to_double(const bvn_int_t *num,
				       const bvn_int_t *den, double *out)
{
	if (!num || !den || bvn_int_is_zero(den)) return false;
	if (bvn_int_is_zero(num)) { *out = 0.0; return true; }
	bool       ok = false;
	bvn_int_t *n = bvn_int_alloc(), *q = bvn_int_alloc(), *r = bvn_int_alloc();
	bvn_int_t *dd = bvn_int_alloc();
	if (!n || !q || !r || !dd || !bvn_int_copy(n, num) || !bvn_int_copy(dd, den))
		goto done;
	bool neg = (num->negative != den->negative);
	n->negative  = false;
	dd->negative = false;
	int shift = 66 + bvn_int_bitlen(dd) - bvn_int_bitlen(n);
	if (shift < 0) shift = 0;
	if (shift && !bvn_int_shl(n, shift)) goto done;
	if (!bvn_int_divrem(q, r, n, dd)) goto done;
	bool sticky = !bvn_int_is_zero(r);
	int  ql     = bvn_int_bitlen(q);
	int  drop   = ql > 64 ? ql - 64 : 0;
	for (int i = 0; i < drop && !sticky; i++)
		if (bvn_int_getbit(q, i)) sticky = true;
	if (drop) bvn_int_shr(q, drop);
	uint64_t m = 0;
	if (!bvn_int_to_uint64(q, &m)) goto done;
	if (sticky) m |= 1u;
	double v = ldexp((double)m, drop - shift);
	*out = neg ? -v : v;
	ok = true;
done:
	bvn_int_free(n); bvn_int_free(q); bvn_int_free(r); bvn_int_free(dd);
	return ok;
}
static bvn_dom_node_t *make_float_from_rational(const bvn_int_t *num,
						const bvn_int_t *den,
						value_type_spec_t vt)
{
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_FLOAT);
	if (!n) return NULL;
	n->value_type = vt;
	if (!builder_rational_to_double(num, den, &n->val.float_val)) {
		bvn_dom_node_destroy(n);
		return NULL;
	}
	return n;
}
/*
 * The type a converted integer is stored under: the declared one, widened to
 * the smallest power-of-two width (8 at least) that holds `num` when the
 * declared width no longer does. A uint that came out negative becomes a sint,
 * for the same reason an integer that came out fractional becomes a float --
 * the node has to describe the value it now holds.
 */
static value_type_spec_t builder_widen_converted_int(value_type_spec_t vt,
						     const bvn_int_t *num)
{
	if (!num) return vt;
	if (vt.family == vt_uint && num->negative && !bvn_int_is_zero(num))
		vt.family = vt_sint;
	int      bl   = bvn_int_bitlen(num);
	uint32_t need = (uint32_t)bl + (vt.family == vt_sint ? 1u : 0u);
	/* -2^(w-1) is the one signed value that fits in its own bit length. */
	if (vt.family == vt_sint && num->negative && bl > 0) {
		bool pow2 = true;
		for (int i = 0; i < bl - 1 && pow2; i++)
			if (bvn_int_getbit(num, i)) pow2 = false;
		if (pow2) need = (uint32_t)bl;
	}
	if (need <= bvn_effective_width(vt)) return vt;
	uint32_t w = 8u;
	while (w < need && w < BVN_MAX_INT_WIDTH) w *= 2u;
	vt.width = w;
	return vt;
}
static bvn_dom_node_t *make_string(const char *str, uint32_t len,
								   value_type_spec_t vt)
{
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_STRING);
	if (!n) return NULL;
	n->value_type = vt;
	n->val.str.data = bvn_dom_strdup(str, len);
	n->val.str.len  = len;
	if (!n->val.str.data) { bvn_dom_node_destroy(n); return NULL; }
	return n;
}
static bvn_dom_node_t *make_bool(const char *str, uint32_t len,
								 value_type_spec_t vt)
{
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_BOOL);
	if (!n) return NULL;
	n->value_type = vt;
	/* The validator collapses on/off to the canonical true/false text
	 * before emitting token_is_bool, so the only spellings that reach
	 * the DOM builder are "true" and "false". */
	n->val.int_val = (str && len == 4 && memcmp(str, "true", 4) == 0)
		? 1 : 0;
	return n;
}
static bvn_dom_node_t *make_symbol(const char *str, uint32_t len,
								   value_type_spec_t vt)
{
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_SYMBOL);
	if (!n) return NULL;
	n->value_type = vt;
	n->val.str.data = bvn_dom_strdup(str, len);
	n->val.str.len  = len;
	if (!n->val.str.data) { bvn_dom_node_destroy(n); return NULL; }
	return n;
}
static bvn_dom_node_t *make_reference(const char *str, uint32_t len,
									  value_type_spec_t vt)
{
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_REFERENCE);
	if (!n) return NULL;
	n->value_type = vt;
	n->val.str.data = bvn_dom_strdup(str, len);
	n->val.str.len  = len;
	if (!n->val.str.data) { bvn_dom_node_destroy(n); return NULL; }
	return n;
}
static bvn_dom_node_t *make_octets(const uint8_t *data, uint32_t len,
								   value_type_spec_t vt)
{
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_OCTET_STREAM);
	if (!n) return NULL;
	n->value_type = vt;
	if (len > 0) {
		n->val.octets.data = malloc(len);
		if (!n->val.octets.data) { bvn_dom_node_destroy(n); return NULL; }
		memcpy(n->val.octets.data, data, len);
	}
	n->val.octets.len = len;
	return n;
}
static bvn_dom_node_t *make_struct(value_type_spec_t vt)
{
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_STRUCT);
	if (!n) return NULL;
	n->value_type = vt;
	return n;
}
static bvn_dom_node_t *make_array(value_type_spec_t vt)
{
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_ARRAY);
	if (!n) return NULL;
	n->value_type = vt;
	n->ext.dims = &bvn_dom_dims_1d;
	return n;
}
/*
 * Decide whether a numeric token should become a FLOAT or INT node. An explicit
 * float family forces float; an explicit int family forces int; otherwise
 * (untyped) it sniffs the text for a dot/exponent or a special value (nan/inf).
 * This is what lets a bare "3.14" land as a float and "42" as an int without an
 * annotation.
 */
/* Is a CONVERTED value a whole number? Its digits may be fractional even
 * though the source family was an integer one. The answer comes from the exact
 * denominator the reader hands over: the digits cannot tell, because 'e' and
 * 'p' ARE digits from base 15 and 26 up -- `<uint:64,m> 14` converted to m in
 * base 16 is "e", and was stored as a float. The text test is only the
 * fallback for a conversion that carries no rational. */
static bool bvn_is_integer_text(const char *s, uint32_t len)
{
	for (uint32_t i = 0; i < len; i++) {
		if (s[i] == '.' || s[i] == 'e' || s[i] == 'E' ||
		    s[i] == 'p' || s[i] == 'P')
			return false;
	}
	return true;
}
static bool bvn_conv_is_integer(const bvnr_converted_t *c,
				const char *s, uint32_t len)
{
	uint64_t den;
	if (c->den)
		return bvn_int_to_uint64(c->den, &den) && den == 1u;
	return bvn_is_integer_text(s, len);
}
/*
 * `quoted` is whether the literal came as a string token. It matters for one
 * thing: nan / inf / ninf are special numbers only as BARE words. Quoted under
 * a uint or sint they are digits -- "nan" in base 36 is 30191 -- and the test
 * below used to run before the family was looked at, so `<uint:64,_36> "nan"`
 * was stored as a float NaN and `<uint:8,_36> "inf"` (24171, which does not fit
 * eight bits) was accepted as infinity without ever reaching the range check.
 */
static bool number_is_float(const char *s, uint32_t len,
							value_type_spec_t vt, bool quoted)
{
	if (vt.family == vt_float ||
		vt.family == vt_float_fix ||
		vt.family == vt_float_dec) return true;
	if (quoted && (vt.family == vt_uint || vt.family == vt_sint))
		return false;
	if (len <= 9) {
		char tmp[10];
		memcpy(tmp, s, len);
		tmp[len] = '\0';
		if (bvn_is_special_number_string(tmp))
			return true;
	}
	if (vt.family == vt_uint || vt.family == vt_sint) return false;
	for (uint32_t i = 0; i < len; i++) {
		if (s[i] == '.' || s[i] == 'e' || s[i] == 'E')
			return true;
	}
	return false;
}
static void builder_do_deferred_pop(bvn_builder_t *b)
{
	if (!b->pending_array_pop) return;
	b->pending_array_pop = false;
	bvn_scope_t *top = b->scope_count > 0 ? builder_top(b) : NULL;
	if (top && top->kind == BVN_SCOPE_ARRAY) {
		builder_pop(b);
	}
}
/*
 * The reader's verified-event callback: the state machine that turns the event
 * stream into tree mutations. assignment_start buffers the key; struct/array
 * starts allocate a container, attach it, and push a scope; the matching ends
 * pop; ev_data builds the right leaf node via the make_* helpers and attaches
 * it; octet-stream events accumulate binary chunks into oct_buf and emit one
 * octet node at the end. Array dimension/row events drive the deferred-pop and
 * per-dimension row-size bookkeeping described at the top of the file. Returning
 * false aborts the parse — the reader surfaces it as
 * error_scanner_callback_failed, which bvn_dom_parse maps back to b->last_error.
 */
static bool on_verified(void *userdata, bvnr_event_t ev, bvnr_data_t *d)
{
	bvn_builder_t *b = (bvn_builder_t *)userdata;
	switch (ev) {
	case ev_stream_start:
		break;
	case ev_stream_end:
		break;
	case ev_assignment_start: {
		builder_do_deferred_pop(b);
		if (d->data && d->length) {
			if (!builder_set_key(b, (const char *)d->data, d->length))
				return false;
		}
		break;
	}
	case ev_type_annotation_start:
	case ev_type_annotation_end:
	case ev_type_annotation_type_family:
	case ev_type_annotation_type_family_parameter:
		break;
	case ev_struct_start: {
		builder_do_deferred_pop(b);
		bvn_dom_node_t *s = make_struct(d->value_type);
		if (!s) return false;
		if (!builder_node_unit(b, s, &d->value_unit, NULL)) {
			bvn_dom_node_destroy(s);
			return false;
		}
		if (!builder_attach(b, s))
			return false;
		if (!builder_push(b, BVN_SCOPE_STRUCT, s)) return false;
		builder_clear_key(b);
		break;
	}
	case ev_struct_end:
		builder_do_deferred_pop(b);
		builder_pop(b);
		break;
	case ev_array_row_start: {
		if (b->array_dim_continue) {
			b->array_dim_continue = false;
			bvn_scope_t *top = builder_top(b);
			if (top && top->kind == BVN_SCOPE_ARRAY && top->node)
				top->row_start_count = top->node->val.arr.count;
			break;
		}
		builder_do_deferred_pop(b);
		bvn_dom_node_t *a = make_array(d->value_type);
		if (!a) return false;
		if (!builder_node_unit(b, a, &d->value_unit, NULL)) {
			bvn_dom_node_destroy(a);
			return false;
		}
		if (!builder_attach(b, a))
			return false;
		if (!builder_push(b, BVN_SCOPE_ARRAY, a)) return false;
		builder_clear_key(b);
		break;
	}
	case ev_array_row_end: {
		/*
		 * A nested array element (the [[...],[...]] form) closes its own
		 * row before the parent's row closes, producing back-to-back
		 * row_end events. The inner array's pop is deferred (pending), so
		 * apply it now — otherwise this parent row_end would mis-operate
		 * on the still-open child scope, leaving the parent array un-popped
		 * and silently absorbing every following sibling. A pending pop is
		 * only ever cancelled by ev_array_dim_start (which clears the flag
		 * to continue the same array across '/' dimensions), so if it is
		 * still set here the child array is genuinely finished.
		 */
		builder_do_deferred_pop(b);
		bvn_scope_t *top = builder_top(b);
		if (top && top->kind == BVN_SCOPE_ARRAY) {
			/* A one-row array has no shape of its own to record: its only
			 * row is the array (bvn_dom_arr_row). */
			bvn_dom_dims_t *dm = top->node->ext.dims;
			if (dm && dm != &bvn_dom_dims_1d) {
				uint32_t row_size = top->node->val.arr.count -
						    top->row_start_count;
				uint32_t dim = dm->num_dims - 1u;
				if (dim < 8)
					dm->rows_per_dim[dim] = row_size;
			}
		}
		b->pending_array_pop = true;
		break;
	}
	case ev_array_dim_start: {
		b->pending_array_pop = false;
		b->array_dim_continue = true;
		bvn_scope_t *top = builder_top(b);
		if (top && top->kind == BVN_SCOPE_ARRAY) {
			/* num_dims is the array's row count (one per '/' separator) and is
			 * read back by bvn_dom_array_dims to reconstruct the matrix shape
			 * (JSON convert, numpy bridge). It must track the true row count, so
			 * it is NOT clamped to the rows_per_dim[8] bound; saturate only to
			 * avoid a wrap to 0 (which would divide-by-zero in the shape emitter)
			 * on a pathologically large number of rows. */
			bvn_dom_dims_t *dm = top->node->ext.dims;
			if (!dm || dm == &bvn_dom_dims_1d) {
				/* The second row: this is a matrix after all, and the
				 * row that just closed was all of the array so far. */
				dm = calloc(1, sizeof(*dm));
				if (!dm) return false;
				dm->num_dims        = 1u;
				dm->rows_per_dim[0] = top->node->val.arr.count;
				top->node->ext.dims = dm;
			}
			if (dm->num_dims < UINT32_MAX)
				dm->num_dims++;
		}
		break;
	}
	case ev_octet_stream_start:
		builder_do_deferred_pop(b);
		b->in_octet_stream = true;
		b->oct_len = 0;
		break;
	case ev_octet_stream_end: {
		bvn_dom_node_t *o = make_octets(b->oct_buf, b->oct_len,
										d->value_type);
		if (!o) return false;
		if (!builder_node_unit(b, o, &d->value_unit, NULL)) {
			bvn_dom_node_destroy(o);
			return false;
		}
		if (!builder_attach(b, o))
			return false;
		b->in_octet_stream = false;
		b->oct_len = 0;
		break;
	}
	case ev_data: {
		if (b->in_octet_stream) {
			if (d->type == token_is_octet_stream && d->data && d->length) {
				if (!builder_oct_append(b,
						(const uint8_t *)d->data, d->length))
					return false;
			}
			break;
		}
		builder_do_deferred_pop(b);
		const char *str = (const char *)d->data;
		uint32_t   len = d->length;
		value_type_spec_t vt = d->value_type;
		value_unit_t      vu = d->value_unit;
		bool              conv_from_rational = false;
		/*
		 * A unit policy that converts is asking for the CONVERTED value, so that
		 * is what the tree stores — `bovnar query .x --unit m` would otherwise
		 * report the document's own digits and the caller would never know the
		 * conversion had been ignored.
		 *
		 * Two things travel with the digits. The base, because a conversion may
		 * have been asked to render in another one and a node carrying base 10
		 * over hex digits is unreadable. And the FAMILY, because an integer can
		 * convert to a fraction — 5 g in kilograms is 0.005 — and number_is_float
		 * answers from the declared family for uint/sint, so leaving it would
		 * hand a fractional string to the integer builder. Promoting to float is
		 * the honest description of what the value now is.
		 *
		 * conv.text is NULL exactly when the exact result does not terminate in
		 * the output base and the reader was told to hand over the rational
		 * anyway; there are no digits to store then, so the original stands.
		 */
		if (d->converted && d->conv.text) {
			str = d->conv.text;
			len = d->conv.length;
			vu  = d->conv.unit;
			/* Only when the conversion actually MOVED base. Writing the
			 * base back unconditionally turns a value_type's unset 0 into
			 * an explicit 10, which is a different thing to the float
			 * formatter: `100` came back out as `1e+02`. */
			if (d->conv.base &&
			    d->conv.base != bvn_effective_base(d->value_type)) {
				/* ...and only where the type HAS a base to move. A uint/sint
				 * is written in any base bovnar has, so its digits and its
				 * base travel together. A float has a literal in base 10 and
				 * 16 and no other; float_fix keeps its Q in this very field
				 * and float_dec leaves it unused, both always decimal. Stamping
				 * conv.base on those produced a type no document can declare
				 * (`<float:64,_2>`), digits no float parser reads, and for
				 * float_fix a silently different Q -- and the failure surfaced
				 * as error_value_out_of_range against the document. There the
				 * VALUE is taken from the exact rational instead (it is a
				 * double either way) and the type keeps its own base. */
				if (vt.family == vt_uint || vt.family == vt_sint ||
				    (vt.family == vt_float &&
				     (d->conv.base == 10u || d->conv.base == 16u)))
					vt.base = d->conv.base;
				else if (bvn_type_is_numeric(vt))
					conv_from_rational = true;
			}
			if ((vt.family == vt_uint || vt.family == vt_sint) &&
			    !bvn_conv_is_integer(&d->conv, str, len)) {
				vt.family = vt_float;
				/* The WIDTH has to follow the family. `<uint:8,g> 5` in
				 * kilograms came back as a float of width 8, which is not a
				 * float any reader accepts (16, or a multiple of 32) -- a
				 * value_type the node's own document could not be written
				 * with. The payload is a double whatever the integer's width
				 * was, so 64 is the honest one; the base goes with it, since a
				 * float is written in 10 or 16 and the digits about to be
				 * parsed are in conv.base (see builder_float_from_rational
				 * for the bases a float has no literal in). */
				vt.width = 64u;
				if (vt.base != 0u && vt.base != 10u && vt.base != 16u) {
					vt.base = 0u;
					conv_from_rational = true;
				}
			} else if (vt.family == vt_uint || vt.family == vt_sint) {
				/* ...and an integer that stays one can still outgrow the width
				 * it was declared with: `<uint:8,k~m> 2` is 2000 m. make_int
				 * range-checks against the declared width, so that one value
				 * failed the WHOLE document with error_value_out_of_range --
				 * `bovnar query --unit m .other f` blamed the file for a
				 * conversion `validate` and `events` deliver without comment.
				 * Widen to the smallest power-of-two width that holds the
				 * result, counted from the exact numerator the reader hands
				 * over beside the digits. */
				vt = builder_widen_converted_int(vt, d->conv.num);
			}
		}
		bvn_dom_node_t *nd = NULL;
		switch (d->type) {
		case token_is_null_value:
			nd = make_null(vt);
			break;
		case token_is_number:
		case token_is_array_number:
			if (conv_from_rational)
				nd = make_float_from_rational(d->conv.num, d->conv.den,
							      vt);
			else if (number_is_float(str, len, vt, false))
				nd = make_float(str, len, vt);
			else
				nd = make_int(str, len, vt);
			/* spec 1.1 — keep a datetime literal's verbatim sub-second
			 * digits so DOM consumers can read them and the value can
			 * round-trip as an ISO literal. */
			if (nd && vt.family == vt_datetime &&
			    d->frac_data && d->frac_length) {
				nd->ext.dt_frac = bvn_dom_strdup(
					(const char *)d->frac_data, d->frac_length);
				if (!nd->ext.dt_frac) {
					bvn_dom_node_destroy(nd);
					return false;
				}
			}
			if (!nd && b->last_error == error_none)
				b->last_error = error_value_out_of_range;
			break;
		case token_is_string:
		case token_is_array_string:
			/* A quoted literal is not necessarily a string. Spec 6.1 lets
			 * uint/sint/float take "Number OR string", and the quoted form is
			 * the ONLY way to write a non-decimal base whose digits are letters
			 * and the canonical way to write a wide integer — examples/
			 * integers.bvnr ships twelve of them, and pretty-print re-emits them
			 * quoted. Dispatching on the token alone made every one of those a
			 * BVN_DOM_STRING carrying a numeric value_type, so get_u64,
			 * get_bigint and int_to_str all failed and the value was
			 * unreachable. The declared family decides what the node is; the
			 * token only says how it was spelled. */
			if (bvn_type_is_numeric(vt)) {
				if (conv_from_rational)
					nd = make_float_from_rational(d->conv.num,
								      d->conv.den, vt);
				else if (number_is_float(str, len, vt, true))
					nd = make_float(str, len, vt);
				else
					nd = make_int(str, len, vt);
				if (!nd && b->last_error == error_none)
					b->last_error = error_value_out_of_range;
			} else {
				nd = make_string(str, len, vt);
			}
			break;
		case token_is_bool:
			nd = make_bool(str, len, vt);
			break;
		case token_is_symbol:
			nd = make_symbol(str, len, vt);
			break;
		case token_is_reference:
			nd = make_reference(str, len, vt);
			break;
		case token_is_octet_stream:
			break;
		default:
			nd = make_null(vt);
			break;
		}
		if (!nd) return false;
		/* library 2.0 — the unit's own spelling. Not when a policy converted the
		 * value: the node then carries conv.unit, which the document never
		 * spelled. */
		if (!builder_node_unit(b, nd, &vu,
				       (d->converted && d->conv.text) ? NULL : d)) {
			bvn_dom_node_destroy(nd);
			return false;
		}
		if (!builder_attach(b, nd))
			return false;
		break;
	}
	default:
		break;
	}
	return true;
}
/*
 * Public one-shot parse: build a complete DOM from an in-memory buffer. It
 * wires a builder (with the document scope pushed) as the reader's on_verified
 * consumer, runs the reader to completion, and returns the document. On a parse
 * failure the document is still returned but carries parse_error — callers
 * check bvn_dom_doc_get_parse_error rather than relying on a NULL return, so
 * partial structure can be inspected. bvn_dom_parse_fd[_ex] slurp a file
 * descriptor into a bounded buffer first, then delegate here.
 */
/*
 * Array element homogeneity (spec 1.0). Heterogeneous arrays are a parse error:
 * every non-null element of an array must share the same kind and physical
 * dimension. The check is recursive and runs over the materialised DOM (the
 * streaming reader validates per-value types/units; cross-element structure is
 * a whole-array property, so it is enforced where the structure exists).
 *
 * The rules, decided for 1.0:
 *   - scalars: same physical dimension; the numeric encodings (int/float) may
 *     mix freely, bool/utf8/symbol/reference/octet may not mix with numbers or
 *     each other;
 *   - sub-arrays: same length and recursively-matching element shape;
 *   - structs: same keys (same order) with recursively-matching field shapes;
 *   - null / empty: a hole that matches any element and never establishes or
 *     breaks the shape (so sparse arrays like [1,,3] stay valid).
 */
/*
 * THE SAME UNIT, for the homogeneity of a bare array.
 *
 * This used to be "the same physical dimension" — bvn_units_compatible, with a
 * currency fallback — and §7.4 said in the same breath that "a bare array of
 * measurements is therefore uniform: a consumer may treat its elements
 * identically". Those two statements were not compatible. Dimensional
 * homogeneity admits
 *
 *      [<float:64,m> 1.0, <float:64,ft> 2.0]          scale differs by 0.3048
 *      [<float_dec:64,$USD> 1, <float_dec:64,k~$USD> 2]   by 1000
 *      [<float:64,°C> 1.0, <float:64,K> 2.0]          by an OFFSET of 273.15
 *
 * and in each of them a consumer that took §7.4 at its word and read the block
 * with one unit computed a wrong number. The last is the one that settles it: an
 * affine offset between two neighbouring cells is not a scale error a reader can
 * notice downstream, and refusing a wrong unit at the parse is the whole claim
 * the format makes.
 *
 * So a bare array is homogeneous in its UNIT, not merely in its dimension, and
 * §7.4's promise is now true as written. Struct fields are untouched — "shape
 * uniform, fields free" still lets a multi-currency ledger carry $USD in one
 * record and $EUR in the next, because a record's fields are named and a
 * consumer reads them one at a time.
 *
 * bvn_unit_equal is the library's own comparator, so this agrees with equality
 * everywhere else, and it is order-insensitive: "$USD·$EUR" and "$EUR·$USD" are
 * the same unit and stay in one array. What it cannot see is that an omitted
 * unit and an explicit "no_unit" are two spellings of dimensionless with
 * different component counts (0 against 1 × bu_none), which would have made
 * "[<float:64,no_unit> 1.0, <float:64> 2.0]" heterogeneous; that is settled
 * first, below, before the structural comparison is allowed to matter.
 */
static bool bvn_dom_unit_dimensionless(value_unit_t u)
{
	for (uint32_t i = 0; i < u.num_components
	                  && i < BVNR_MAX_UNIT_COMPONENTS; i++) {
		if (u.components[i].base != bu_none)
			return false;
	}
	return true;
}
static bool bvn_dom_same_unit(value_unit_t a, value_unit_t b)
{
	if (bvn_dom_unit_dimensionless(a) && bvn_dom_unit_dimensionless(b))
		return true;
	return bvn_unit_equal(a, b);
}
/*
 * Hard recursion-depth guard for the post-parse structural validators below.
 * The reader caps array and struct nesting at 255 *each*, so a materialised tree
 * reaching these checks is at most ~510 levels deep; this ceiling sits well above
 * that yet stops any pathological tree (or a future caller that bypasses the
 * reader caps) from exhausting the C stack. The destructor is iterative for the
 * same reason (see bovnar_dom.c); these checks fail closed instead.
 */
#define BVN_DOM_CHECK_MAX_DEPTH 4096u
/*
 * THE SHAPE AN ARRAY'S ELEMENTS SHARE, accumulated as the elements are read.
 *
 * Homogeneity used to be a pairwise test of each element against the FIRST
 * non-null one. A null is a hole that matches anything, and "matches anything"
 * is not transitive: where the first element had a hole, it said nothing about
 * that position, and the elements after it were compared with IT and never with
 * each other. So
 *
 *      [[null, null], [<float:64,m> 1, null], [<float:64,s> 2, null]]
 *
 * put metres and seconds in one matrix column and validated, and
 * `[{.x = null;}, {.x = 1;}, {.x = "s";}]` validated while the same three
 * records in the order 1, null, "s" were array_element_type_mismatch. The
 * verdict depended on where the holes happened to be.
 *
 * The reference is therefore not an element but a shape: one slot per position
 * (a scalar, the element of an array, each field of a struct), EMPTY until
 * some sibling has a non-null value there, and from then on what every later
 * sibling is held to. A hole neither fills a slot nor is tested against one,
 * which is all that "matches anything" was ever meant to say. The result no
 * longer depends on the order of the elements.
 *
 * `rep` is the first node seen at the position: for a scalar its kind and unit,
 * for an array its row geometry, for a struct its keys. `sub` is the one
 * element slot of an array, or one slot per field of a struct.
 */
typedef struct bvn_dom_shape_s {
	const bvn_dom_node_t   *rep;
	struct bvn_dom_shape_s *sub;
	uint32_t                nsub;
} bvn_dom_shape_t;
static void bvn_dom_shape_free(bvn_dom_shape_t *s, unsigned depth)
{
	/* No depth guard of its own: the merge never builds deeper than its guard
	 * allows, so this recursion is bounded by the same number -- and a guard
	 * here could only ever return early, which would be a leak. */
	(void)depth;
	if (!s->sub) return;
	for (uint32_t i = 0; i < s->nsub; i++)
		bvn_dom_shape_free(&s->sub[i], depth + 1u);
	free(s->sub);
	s->sub  = NULL;
	s->nsub = 0;
}
static error_code_t bvn_dom_check_homogeneous(
	const bvn_dom_node_t *node, unsigned depth);
static error_code_t bvn_dom_keys_unique(
	const bvn_dom_entry_t *ent, uint32_t n);
/*
 * Hold node `n` to the shape `s`, filling in whatever `s` does not know yet.
 * Returns error_none when n fits, otherwise the specific mismatch code.
 *
 * check_dim distinguishes the two scopes of the spec-1.0 rule ("shape uniform,
 * fields free"). In a bare array / matrix context (check_dim = true) elements
 * must share the same UNIT (bvn_dom_same_unit, above) AND sub-arrays must be
 * rectangular (equal length). Once the path descends into a struct, its fields
 * need only match in KIND and nesting: a scalar field may carry a different unit
 * in each record (e.g. a multi-currency ledger) and a list field may have a
 * different length in each record (e.g. per-record argument lists). So
 * struct-field recursion passes check_dim = false, freeing both unit and array
 * length.
 *
 * `internal` says that n has not been checked on its OWN account yet: that the
 * arrays below it are each homogeneous and no struct below it repeats a key.
 * For a sub-array under check_dim that needs no second walk -- holding all of
 * its elements to the one shared element slot is exactly its own homogeneity --
 * so the flag is simply passed down. A struct's fields are the place the two
 * diverge: across records they are compared loosely (fields free), while a list
 * inside ONE record is still a bare array with one unit, so each field value
 * gets its own bvn_dom_check_homogeneous there.
 */
static error_code_t bvn_dom_shape_merge(
	bvn_dom_shape_t *s, const bvn_dom_node_t *n, bool check_dim,
	bool internal, unsigned depth)
{
	if (depth > BVN_DOM_CHECK_MAX_DEPTH)
		return error_array_nesting_too_high;
	if (!n)
		return error_none;
	bvn_dom_type_t tn = bvn_dom_node_type(n);
	if (tn == BVN_DOM_NULL)
		return error_none;              /* a hole: says nothing, breaks nothing */
	if (!s->rep) {
		uint32_t want = tn == BVN_DOM_ARRAY  ? 1u
			      : tn == BVN_DOM_STRUCT ? bvn_dom_struct_count(n) : 0u;
		if (want) {
			s->sub = calloc(want, sizeof(*s->sub));
			/* Fail closed: a check that could not run has not passed. */
			if (!s->sub)
				return error_invalid_argument;
			s->nsub = want;
		}
		s->rep = n;
	} else {
		const bvn_dom_node_t *a = s->rep;
		bvn_dom_type_t ta = bvn_dom_node_type(a);
		bool a_num = (ta == BVN_DOM_INT || ta == BVN_DOM_FLOAT);
		bool b_num = (tn == BVN_DOM_INT || tn == BVN_DOM_FLOAT);
		if (a_num && b_num) {
			/* Both numeric: kinds match (encodings may mix). Dimension only
			 * when comparing array/matrix elements, not struct fields. */
			/* datetime (spec 1.1) is its own kind: a timestamp does not mix
			 * with a plain count/measurement (spec §7.4 lists the mixable
			 * encodings; datetime is not among them), and — like a currency —
			 * its epoch is a dimension, so two datetimes on different epochs
			 * (time scales) may not share an array. */
			bool a_dt = (a->value_type.family == vt_datetime);
			bool b_dt = (n->value_type.family == vt_datetime);
			if (a_dt != b_dt)
				return error_array_element_type_mismatch;
			if (a_dt && b_dt && check_dim &&
				a->value_type.base != n->value_type.base)
				return error_array_element_type_mismatch;
			if (check_dim &&
				!bvn_dom_same_unit(bvn_dom_get_unit(a), bvn_dom_get_unit(n)))
				return error_array_element_type_mismatch;
			return error_none;
		}
		if (ta != tn)
			return error_array_element_type_mismatch;
		if (tn == BVN_DOM_ARRAY && check_dim) {
			/* rectangular only in the bare array / matrix context; list
			 * fields of records may vary in length (check_dim == false). */
			if (bvn_dom_array_count(a) != bvn_dom_array_count(n))
				return error_array_row_size_mismatch;
			/* A '/'-row matrix stores its cells flat, with the row geometry in
			 * num_dims + rows_per_dim[]. Equal total cell count is NOT enough to
			 * mean equal shape: a 2x3 block and a 3x2 block (or a flat 4-element
			 * array and a 2x2 matrix) both hold the same number of cells but are
			 * different shapes, so the row geometry must match too. */
			if (bvn_dom_arr_ndims(a) != bvn_dom_arr_ndims(n))
				return error_array_row_size_mismatch;
			uint32_t nd = bvn_dom_arr_ndims(a);
			if (nd > 8u) nd = 8u;   /* rows_per_dim records only the first 8 rows */
			for (uint32_t i = 0; i < nd; i++)
				if (bvn_dom_arr_row(a, i) != bvn_dom_arr_row(n, i))
					return error_array_row_size_mismatch;
		}
		if (tn == BVN_DOM_STRUCT) {
			uint32_t na = bvn_dom_struct_count(a);
			if (na != bvn_dom_struct_count(n))
				return error_struct_shape_mismatch;
			const bvn_dom_entry_t *ea = bvn_dom_struct_entries(a);
			const bvn_dom_entry_t *eb = bvn_dom_struct_entries(n);
			for (uint32_t i = 0; i < na; i++)
				if (!ea[i].key || !eb[i].key ||
					strcmp(ea[i].key, eb[i].key) != 0)
					return error_struct_shape_mismatch;
		}
	}
	switch (tn) {
	case BVN_DOM_ARRAY: {
		uint32_t cnt = bvn_dom_array_count(n);
		for (uint32_t i = 0; i < cnt; i++) {
			/* matrices stay uniform: every cell of every sibling is held to
			 * the one element slot. */
			error_code_t e = bvn_dom_shape_merge(&s->sub[0],
				bvn_dom_array_at(n, i), check_dim, internal, depth + 1u);
			if (e != error_none)
				return e;
		}
		return error_none;
	}
	case BVN_DOM_STRUCT: {
		uint32_t cnt = bvn_dom_struct_count(n);
		const bvn_dom_entry_t *ent = bvn_dom_struct_entries(n);
		if (internal) {
			error_code_t dup = bvn_dom_keys_unique(ent, cnt);
			if (dup != error_none)
				return dup;
		}
		for (uint32_t i = 0; i < cnt && i < s->nsub; i++) {
			if (internal) {
				error_code_t own =
					bvn_dom_check_homogeneous(ent[i].value, depth + 1u);
				if (own != error_none)
					return own;
			}
			/* fields free: compare kind/shape, not dimension. */
			error_code_t e = bvn_dom_shape_merge(&s->sub[i], ent[i].value,
				false, false, depth + 1u);
			if (e != error_none)
				return e;
		}
		return error_none;
	}
	default:
		/* string / symbol / bool / reference / octet: same kind suffices. */
		return error_none;
	}
}
/*
 * Recursively verify that every array in the subtree is homogeneous. Descends
 * into struct fields and array elements so a violation at any depth is found.
 */
/*
 * Verify no key is repeated within one scope — a struct, or the top-level
 * document (spec 1.0). Unique keys keep lookup, references and iteration in
 * agreement on the value of a key. Sorting a copy of the key pointers keeps
 * this O(n log n), so a scope with very many members cannot turn quadratic.
 */
static int bvn_dom_keycmp(const void *a, const void *b)
{
	const char *ka = *(const char *const *)a;
	const char *kb = *(const char *const *)b;
	return strcmp(ka ? ka : "", kb ? kb : "");
}
static error_code_t bvn_dom_keys_unique(
	const bvn_dom_entry_t *ent, uint32_t n)
{
	if (n < 2u)
		return error_none;
	const char **keys = malloc((size_t)n * sizeof(*keys));
	if (!keys) {
		/* OOM: fall back to an in-place O(n^2) compare rather than skipping
		 * the check, which would let a document with duplicate keys pass
		 * validation under memory pressure. Slow for a huge scope, but a
		 * scope that large is unlikely to coexist with a failed small malloc,
		 * and correctness must not depend on available memory. */
		for (uint32_t i = 0; i < n; i++) {
			if (!(ent[i].key && ent[i].key[0]))
				continue;
			for (uint32_t j = i + 1u; j < n; j++)
				if (ent[j].key && ent[j].key[0] &&
				    strcmp(ent[i].key, ent[j].key) == 0)
					return error_duplicate_struct_key;
		}
		return error_none;
	}
	uint32_t m = 0;
	for (uint32_t i = 0; i < n; i++)
		if (ent[i].key && ent[i].key[0])   /* only real, non-empty keys */
			keys[m++] = ent[i].key;
	qsort(keys, (size_t)m, sizeof(*keys), bvn_dom_keycmp);
	error_code_t r = error_none;
	for (uint32_t i = 1; i < m; i++) {
		if (strcmp(keys[i], keys[i - 1]) == 0) {
			r = error_duplicate_struct_key;
			break;
		}
	}
	free(keys);
	return r;
}
/* -fanalyzer reports shape.sub as leaked at the end of this function. It is
 * freed on every path: bvn_dom_shape_free walks the same tree the merge built.
 * The analyzer gives up following the two mutually recursive walks a few levels
 * down and reports what it could not finish proving. LeakSanitizer, which sees
 * what actually happens, is clean over the DOM tests and the fuzz corpus. */
#if defined(__GNUC__) && !defined(__clang__)
#  pragma GCC diagnostic push
#  pragma GCC diagnostic ignored "-Wanalyzer-malloc-leak"
#endif
static error_code_t bvn_dom_check_homogeneous(
	const bvn_dom_node_t *node, unsigned depth)
{
	if (!node)
		return error_none;
	if (depth > BVN_DOM_CHECK_MAX_DEPTH)
		return bvn_dom_node_type(node) == BVN_DOM_STRUCT
			? error_struct_nesting_too_high
			: error_array_nesting_too_high;
	switch (bvn_dom_node_type(node)) {
	case BVN_DOM_ARRAY: {
		uint32_t n = bvn_dom_array_count(node);
		/* array elements: dimension is enforced (bare arrays and matrices
		 * are uniform), against the shape the elements build between them --
		 * see bvn_dom_shape_t for why not against the first one. */
		bvn_dom_shape_t shape = { NULL, NULL, 0u };
		error_code_t r = error_none;
		for (uint32_t i = 0; r == error_none && i < n; i++)
			r = bvn_dom_shape_merge(&shape, bvn_dom_array_at(node, i),
						true, true, depth + 1u);
		bvn_dom_shape_free(&shape, 0u);
		return r;
	}
	case BVN_DOM_STRUCT: {
		uint32_t n = bvn_dom_struct_count(node);
		const bvn_dom_entry_t *ent = bvn_dom_struct_entries(node);
		error_code_t dup = bvn_dom_keys_unique(ent, n);
		if (dup != error_none)
			return dup;
		for (uint32_t i = 0; i < n; i++) {
			error_code_t sub =
				bvn_dom_check_homogeneous(ent[i].value, depth + 1u);
			if (sub != error_none)
				return sub;
		}
		return error_none;
	}
	default:
		return error_none;
	}
}
#if defined(__GNUC__) && !defined(__clang__)
#  pragma GCC diagnostic pop
#endif
bvn_dom_doc_t *bvn_dom_parse_policy(const void *data, uint32_t len,
				    const bvnr_unit_policy_t *policy)
{
	bvn_dom_doc_t *doc = bvn_dom_doc_create();
	if (!doc) return NULL;
	bvn_builder_t b;
	if (!builder_init(&b, doc)) {
		bvn_dom_doc_destroy(doc);
		return NULL;
	}
	if (!builder_push(&b, BVN_SCOPE_DOC, NULL)) {
		builder_cleanup(&b);
		bvn_dom_doc_destroy(doc);
		return NULL;
	}
	bvnr_read_flags_t flags;
	memset(&flags, 0, sizeof(flags));
	flags.max_file_size = UINT32_MAX;
	flags.max_array_nesting = 255;
	flags.max_struct_nesting = 255;
	flags.userdata      = &b;
	flags.on_verified   = on_verified;
	bvnr_reader_t *rd = bvnr_reader_create();
	if (!rd) {
		builder_cleanup(&b);
		bvn_dom_doc_destroy(doc);
		return NULL;
	}
	/* The policy is a property of the reader, not of the flags, and it is
	 * carried across the open — so it may be installed either side of it. A
	 * policy the parser refuses (a malformed unit, a path that names nothing)
	 * is reported here rather than as a parse error against the document. */
	if (policy && !bvnr_reader_set_unit_policy(rd, policy)) {
		bvnr_reader_destroy(rd);
		builder_cleanup(&b);
		doc->parse_error = error_invalid_argument;
		return doc;
	}
	if (!bvnr_open_read_mem(rd, data, len, NULL, 0, &flags)) {
		bvnr_reader_destroy(rd);
		builder_cleanup(&b);
		bvn_dom_doc_destroy(doc);
		return NULL;
	}
	bool ok = bvnr_read(rd);
	if (!ok) {
		error_code_t reader_err = bvnr_reader_get_error(rd);
		if (reader_err == error_scanner_callback_failed
			&& b.last_error != error_none)
			doc->parse_error = b.last_error;
		else
			doc->parse_error = reader_err;
	} else {
		/* Per-value syntax/type/unit all passed; now enforce the cross-element
		 * structural rules over the materialised tree (spec 1.0): unique keys
		 * within every scope (including the top-level document) and array
		 * element homogeneity. */
		error_code_t herr = bvn_dom_keys_unique(doc->entries, doc->count);
		for (uint32_t i = 0; herr == error_none && i < doc->count; i++)
			herr = bvn_dom_check_homogeneous(doc->entries[i].value, 0u);
		if (herr != error_none)
			doc->parse_error = herr;
	}
	bvnr_reader_destroy(rd);
	builder_do_deferred_pop(&b);
	builder_cleanup(&b);
	return doc;
}
/*
 * Read an entire fd into a growable buffer (capped at max_bytes to bound memory
 * against an endless stream) and parse it. EINTR is retried; the buffer doubles
 * until the cap. Because the DOM needs the whole document in memory anyway,
 * slurping first is simpler than driving the reader off the fd directly.
 */
bvn_dom_doc_t *bvn_dom_parse(const void *data, uint32_t len)
{
	return bvn_dom_parse_policy(data, len, NULL);
}
bvn_dom_doc_t *bvn_dom_parse_fd_policy(int fd, uint64_t max_bytes,
				       const bvnr_unit_policy_t *policy)
{
	if (!max_bytes || max_bytes > (uint64_t)BVN_DOM_FD_MAX_BYTES)
		max_bytes = BVN_DOM_FD_MAX_BYTES;
	size_t cap = 65536;
	size_t len = 0;
	uint8_t *buf = malloc(cap);
	if (!buf) return NULL;
	for (;;) {
		ssize_t n;
		do {
			n = read(fd, buf + len, cap - len);
		} while (n < 0 && errno == EINTR);
		if (n < 0) {
			free(buf);
			return NULL;
		}
		if (n == 0) break;
		len += (size_t)n;
		if ((uint64_t)len > max_bytes) {
			free(buf);
			return NULL;
		}
		if (len == cap) {
			size_t ncap;
			if (cap > SIZE_MAX / 2u) ncap = SIZE_MAX;
			else                     ncap = cap * 2u;
			if ((uint64_t)ncap > max_bytes) ncap = (size_t)max_bytes + 1u;
			if (ncap <= cap) { free(buf); return NULL; }
			uint8_t *nb = realloc(buf, ncap);
			if (!nb) { free(buf); return NULL; }
			buf = nb;
			cap = ncap;
		}
	}
	if (len > (size_t)UINT32_MAX) { free(buf); return NULL; }
	bvn_dom_doc_t *doc = bvn_dom_parse_policy(buf, (uint32_t)len, policy);
	free(buf);
	return doc;
}
bvn_dom_doc_t *bvn_dom_parse_fd_ex(int fd, uint64_t max_bytes)
{
	return bvn_dom_parse_fd_policy(fd, max_bytes, NULL);
}
bvn_dom_doc_t *bvn_dom_parse_fd(int fd)
{
	return bvn_dom_parse_fd_ex(fd, BVN_DOM_FD_MAX_BYTES);
}
