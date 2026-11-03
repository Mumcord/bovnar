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
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "bovnar_dom.h"
#include "bvn_dom_impl.h"
#include "bovnar_si_units.h"
/*
 * ===========================================================================
 * DOM (in-memory document tree)
 * ===========================================================================
 *
 * The streaming reader is allocation-light but forces the caller to react to
 * events. The DOM is the convenience alternative: parse a whole document into a
 * tree of bvn_dom_node_t (built by the observer in bovnar_dom_builder.c) and
 * then navigate/query it randomly. A document is a bvn_dom_doc_t (a list of
 * top-level key/value entries plus any parse_error); every value is a node
 * tagged with its bvn_dom_type_t, carrying its value_type and unit, and using a
 * union for the payload (inline int/float, heap string/octets, child
 * struct/array). Integers wider than 64 bits are stored as a heap bvn_int_t
 * (val.bigint), narrower ones inline (val.int_val) — the value_type.width is
 * the discriminator everywhere in this file.
 *
 * This file holds construction helpers (used by the builder), the destructor,
 * navigation (lookup by path, struct/array access) and a family of typed
 * accessors that convert a node's value to a requested C type with range
 * checking.
 */

/*
 * Duplicate a length-counted slice into a fresh NUL-terminated heap string.
 * Used for keys and string payloads since DOM nodes own their text (the parse
 * buffers they came from are transient).
 */
char *bvn_dom_strdup(const char *s, uint32_t len)
{
	char *r = malloc(len + 1u);
	if (!r) return NULL;
	memcpy(r, s, len);
	r[len] = '\0';
	return r;
}
bvn_dom_unit_t bvn_dom_unit_default = {
	.refs = 0u,
	.unit = { .num_components = 1,
		  .components = {{ .base = bu_none, .exponent = exp_linear,
				   .prefix.system = prefix_si,
				   .prefix.id.si = si_none }} }
};
bvn_dom_unit_t bvn_dom_unit_none = { .refs = 0u, .unit = { .num_components = 0 } };
bvn_dom_dims_t bvn_dom_dims_1d   = { .num_dims = 1u };
/* Same unit, component for component. Deliberately not bvn_unit_equal: two
 * spellings that mean one unit are still two things bvn_dom_get_unit returns. */
static bool bvn_dom_unit_same(const value_unit_t *a, const value_unit_t *b)
{
	if (a->num_components != b->num_components)
		return false;
	uint32_t n = a->num_components < BVNR_MAX_UNIT_COMPONENTS
		   ? a->num_components : BVNR_MAX_UNIT_COMPONENTS;
	for (uint32_t i = 0; i < n; i++) {
		const value_unit_component_t *x = &a->components[i];
		const value_unit_component_t *y = &b->components[i];
		if (x->base != y->base || x->exponent != y->exponent ||
		    x->prefix.system != y->prefix.system ||
		    (x->prefix.system == prefix_iec
		         ? x->prefix.id.iec != y->prefix.id.iec
		         : x->prefix.id.si  != y->prefix.id.si))
			return false;
	}
	return true;
}
bool bvn_dom_unit_is(const bvn_dom_unit_t *u, const value_unit_t *vu,
		     const char *src, uint32_t src_len)
{
	if (!src)
		src_len = 0u;
	if ((u->src != NULL) != (src != NULL) || u->src_len != src_len)
		return false;
	if (src && memcmp(u->src, src, src_len) != 0)
		return false;
	return bvn_dom_unit_same(&u->unit, vu);
}
bvn_dom_unit_t *bvn_dom_unit_get(const value_unit_t *vu,
				 const char *src, uint32_t src_len)
{
	if (!src) {
		if (bvn_dom_unit_same(vu, &bvn_dom_unit_default.unit))
			return &bvn_dom_unit_default;
		if (vu->num_components == 0u)
			return &bvn_dom_unit_none;
	}
	bvn_dom_unit_t *u = malloc(sizeof(*u));
	if (!u) return NULL;
	u->refs    = 1u;
	u->src     = NULL;
	u->src_len = 0u;
	u->unit    = *vu;
	if (src) {
		u->src = bvn_dom_strdup(src, src_len);
		if (!u->src) {
			free(u);
			return NULL;
		}
		u->src_len = src_len;
	}
	return u;
}
bvn_dom_unit_t *bvn_dom_unit_ref(bvn_dom_unit_t *u)
{
	if (bvn_dom_unit_static(u))
		return u;
	if (u->refs == UINT32_MAX)
		return NULL;                    /* full: the caller makes another */
	u->refs++;
	return u;
}
void bvn_dom_unit_release(bvn_dom_unit_t *u)
{
	if (!u || bvn_dom_unit_static(u))
		return;
	if (--u->refs == 0u) {
		free(u->src);
		free(u);
	}
}
void bvn_dom_node_take_unit(bvn_dom_node_t *n, bvn_dom_unit_t *u)
{
	bvn_dom_unit_release(n->unit);
	n->unit = u;
}
bool bvn_dom_node_set_unit(bvn_dom_node_t *n, const value_unit_t *vu)
{
	bvn_dom_unit_t *u = bvn_dom_unit_get(vu, NULL, 0u);
	if (!u) return false;
	bvn_dom_node_take_unit(n, u);
	return true;
}
bvn_dom_node_t *bvn_dom_node_alloc(bvn_dom_type_t t)
{
	bvn_dom_node_t *n = malloc(sizeof(*n));
	if (!n) return NULL;
	memset(n, 0, sizeof(*n));
	n->type       = t;
	n->value_type = BVN_TYPE_PLAIN;
	n->unit       = &bvn_dom_unit_default;
	return n;
}
static bool bvn_dom_stack_reserve(bvn_dom_node_t ***stack,
								  size_t *cap, size_t need)
{
	if (need <= *cap) return true;
	size_t nc = *cap ? *cap : 16u;
	while (nc < need) {
		if (nc > SIZE_MAX / (2u * sizeof(**stack))) return false;
		nc *= 2u;
	}
	bvn_dom_node_t **ns = realloc(*stack, nc * sizeof(**stack));
	if (!ns) return false;
	*stack = ns;
	*cap   = nc;
	return true;
}
/*
 * Free a node and its entire subtree. Deliberately ITERATIVE, using an explicit
 * heap-allocated work stack rather than recursion: a hostile or simply deep
 * document could nest thousands of levels, and a recursive free would blow the
 * C call stack. Children are pushed and processed in a loop; the node's own
 * payload (bigint/string/octets) is released as it is popped. If the work stack
 * can't grow, it falls back to freeing the current node and bailing — leaking
 * is preferable to crashing under memory pressure.
 */
void bvn_dom_node_destroy(bvn_dom_node_t *n)
{
	if (!n) return;
	bvn_dom_node_t **stack = NULL;
	size_t sc  = 0;
	size_t cap = 0;
	if (!bvn_dom_stack_reserve(&stack, &cap, 1u)) {
		free(n);
		return;
	}
	stack[sc++] = n;
	while (sc > 0u) {
		bvn_dom_node_t *cur = stack[--sc];
		if (!cur) continue;
		/* The unit block is shared; the extra pointer is one allocation
		 * whichever of its two meanings the tag gives it, apart from the
		 * static shape every one-row array points at. */
		bvn_dom_unit_release(cur->unit);
		if (cur->ext.dims != &bvn_dom_dims_1d)
			free(cur->ext.dt_frac);
		switch (cur->type) {
		case BVN_DOM_INT:
			if (cur->value_type.width > 64u && cur->val.bigint)
				bvn_int_free(cur->val.bigint);
			break;
		case BVN_DOM_STRING:
		case BVN_DOM_SYMBOL:
		case BVN_DOM_REFERENCE:
			free(cur->val.str.data);
			break;
		case BVN_DOM_OCTET_STREAM:
			free(cur->val.octets.data);
			break;
		case BVN_DOM_STRUCT:
			if (cur->val.members.entries) {
				uint32_t mc = cur->val.members.count;
				if (!bvn_dom_stack_reserve(&stack, &cap, sc + mc)) {
					free(cur);
					free(stack);
					return;
				}
				for (uint32_t i = 0; i < mc; i++) {
					free(cur->val.members.entries[i].key);
					stack[sc++] = cur->val.members.entries[i].value;
				}
				free(cur->val.members.entries);
			}
			break;
		case BVN_DOM_ARRAY:
			if (cur->val.arr.items) {
				uint32_t ac = cur->val.arr.count;
				if (!bvn_dom_stack_reserve(&stack, &cap, sc + ac)) {
					free(cur);
					free(stack);
					return;
				}
				for (uint32_t i = 0; i < ac; i++)
					stack[sc++] = cur->val.arr.items[i];
				free(cur->val.arr.items);
			}
			break;
		default:
			break;
		}
		free(cur);
	}
	free(stack);
}
/*
 * The three attach functions are public, and were written as if only the
 * builder called them: no check that the container IS one, none for NULL, and
 * none for a key the scope already has. bvn_dom_array_append on a struct node
 * returned true and stored the element where nothing reads it -- array_count
 * stayed 0, the caller was told it had succeeded -- and a hand-built struct
 * could carry the duplicate key a parsed one is refused for
 * (error_duplicate_struct_key).
 *
 * The contract in doc/08 4.5 already said what a refusal looks like: the value
 * is destroyed and the call returns false. The checks below make the refusals
 * exist.
 *
 * The duplicate test is a linear scan, which is right for a tree built by hand
 * and wrong for the parser: a document with a 100 000-member struct would pay
 * for it quadratically, and the parser has to REPORT a duplicate as a
 * document-tier error after the tree is complete rather than refuse the member
 * that carries it. So the builder attaches through the *_nocheck forms
 * (bvn_dom_impl.h), which are these functions as they used to be.
 */
static bool bvn_dom_entries_have_key(const bvn_dom_entry_t *e, uint32_t n,
				     const char *key, uint32_t klen)
{
	for (uint32_t i = 0; i < n; i++) {
		if (e[i].key && strlen(e[i].key) == (size_t)klen &&
		    memcmp(e[i].key, key, klen) == 0)
			return true;
	}
	return false;
}
bool bvn_dom_struct_add(bvn_dom_node_t *s,
						const char *key, uint32_t klen,
						bvn_dom_node_t *val)
{
	if (!s || s->type != BVN_DOM_STRUCT || !val || val == s ||
	    (!key && klen) ||
	    bvn_dom_entries_have_key(s->val.members.entries, s->val.members.count,
				     key, klen)) {
		bvn_dom_node_destroy(val);
		return false;
	}
	return bvn_dom_struct_add_nocheck(s, key, klen, val);
}
bool bvn_dom_struct_add_nocheck(bvn_dom_node_t *s,
						const char *key, uint32_t klen,
						bvn_dom_node_t *val)
{
	if (s->val.members.count == s->val.members.cap) {
		if (s->val.members.cap > UINT32_MAX / 2u) {
			bvn_dom_node_destroy(val); return false;
		}
		uint32_t nc = s->val.members.cap ? s->val.members.cap * 2u : 8u;
#if SIZE_MAX <= 0xFFFFFFFFu
		/* nc fits uint32_t (guard above), but nc * sizeof(entry) is computed in
		 * size_t and would wrap on a 32-bit size_t; reject as bvn_dom_stack_reserve
		 * does. Compiled out on 64-bit, where the product cannot overflow. */
		if ((size_t)nc > SIZE_MAX / sizeof(bvn_dom_entry_t)) {
			bvn_dom_node_destroy(val); return false;
		}
#endif
		bvn_dom_entry_t *ne = realloc(s->val.members.entries,
									  nc * sizeof(*ne));
		if (!ne) { bvn_dom_node_destroy(val); return false; }
		s->val.members.entries = ne;
		s->val.members.cap     = nc;
	}
	char *ks = bvn_dom_strdup(key, klen);
	if (!ks) { bvn_dom_node_destroy(val); return false; }
	s->val.members.entries[s->val.members.count].key   = ks;
	s->val.members.entries[s->val.members.count].value = val;
	s->val.members.count++;
	return true;
}
bool bvn_dom_doc_add(bvn_dom_doc_t *d,
					 const char *key, uint32_t klen,
					 bvn_dom_node_t *val)
{
	if (!d || !val || (!key && klen) ||
	    bvn_dom_entries_have_key(d->entries, d->count, key, klen)) {
		bvn_dom_node_destroy(val);
		return false;
	}
	return bvn_dom_doc_add_nocheck(d, key, klen, val);
}
bool bvn_dom_doc_add_nocheck(bvn_dom_doc_t *d,
					 const char *key, uint32_t klen,
					 bvn_dom_node_t *val)
{
	if (d->count == d->cap) {
		if (d->cap > UINT32_MAX / 2u) {
			bvn_dom_node_destroy(val); return false;
		}
		uint32_t nc = d->cap ? d->cap * 2u : 8u;
#if SIZE_MAX <= 0xFFFFFFFFu
		if ((size_t)nc > SIZE_MAX / sizeof(bvn_dom_entry_t)) {   /* 32-bit wrap guard */
			bvn_dom_node_destroy(val); return false;
		}
#endif
		bvn_dom_entry_t *ne = realloc(d->entries, nc * sizeof(*ne));
		if (!ne) { bvn_dom_node_destroy(val); return false; }
		d->entries = ne;
		d->cap     = nc;
	}
	char *ks = bvn_dom_strdup(key, klen);
	if (!ks) { bvn_dom_node_destroy(val); return false; }
	d->entries[d->count].key   = ks;
	d->entries[d->count].value = val;
	d->count++;
	return true;
}
bool bvn_dom_array_append(bvn_dom_node_t *a, bvn_dom_node_t *elem)
{
	if (!a || a->type != BVN_DOM_ARRAY || !elem || elem == a) {
		bvn_dom_node_destroy(elem);
		return false;
	}
	return bvn_dom_array_append_nocheck(a, elem);
}
bool bvn_dom_array_append_nocheck(bvn_dom_node_t *a, bvn_dom_node_t *elem)
{
	if (a->val.arr.count == a->val.arr.cap) {
		if (a->val.arr.cap > UINT32_MAX / 2u) {
			bvn_dom_node_destroy(elem); return false;
		}
		uint32_t nc = a->val.arr.cap ? a->val.arr.cap * 2u : 8u;
#if SIZE_MAX <= 0xFFFFFFFFu
		if ((size_t)nc > SIZE_MAX / sizeof(bvn_dom_node_t *)) {   /* 32-bit wrap guard */
			bvn_dom_node_destroy(elem); return false;
		}
#endif
		bvn_dom_node_t **ni = realloc(a->val.arr.items,
									  nc * sizeof(*ni));
		if (!ni) { bvn_dom_node_destroy(elem); return false; }
		a->val.arr.items = ni;
		a->val.arr.cap   = nc;
	}
	a->val.arr.items[a->val.arr.count++] = elem;
	return true;
}
bvn_dom_doc_t *bvn_dom_doc_create(void)
{
	bvn_dom_doc_t *d = malloc(sizeof(*d));
	if (!d) return NULL;
	memset(d, 0, sizeof(*d));
	return d;
}
void bvn_dom_doc_destroy(bvn_dom_doc_t *doc)
{
	if (!doc) return;
	if (doc->entries) {
		for (uint32_t i = 0; i < doc->count; i++) {
			free(doc->entries[i].key);
			bvn_dom_node_destroy(doc->entries[i].value);
		}
		free(doc->entries);
	}
	free(doc);
}
bvn_dom_node_t *bvn_dom_struct_get(const bvn_dom_node_t *node,
								   const char *key)
{
	if (!node || !key) return NULL;
	if (node->type != BVN_DOM_STRUCT) return NULL;
	for (uint32_t i = 0; i < node->val.members.count; i++) {
		if (strcmp(node->val.members.entries[i].key, key) == 0)
			return node->val.members.entries[i].value;
	}
	return NULL;
}
/*
 * Descend an array node by a run of [N] indices (spec 1.1). A flat /-row matrix
 * stores its cells row-major with arr.num_dims rows whose individual widths are
 * arr.rows_per_dim[] (the first 8 rows), so it consumes TWO indices
 * ([row][col] -> items[sum(widths before row) + col]); for a rectangular matrix
 * this reduces to items[row*cols + col]. A 1-D array (num_dims == 1) consumes
 * ONE ([i] -> items[i]). Genuine nested
 * arrays (the [[..],[..]] form) are 1-D at each level, so they descend one index
 * per level. A partial index of a matrix (only [row]) has no node to return and
 * yields NULL, as does any out-of-range index. Returns the addressed node, or
 * NULL; *cur is advanced as indices are consumed.
 */
static bvn_dom_node_t *bvn_dom_apply_indices(
	bvn_dom_node_t *cur, const uint32_t *idx, uint32_t nidx)
{
	uint32_t k = 0;
	while (k < nidx) {
		if (!cur || cur->type != BVN_DOM_ARRAY) return NULL;
		uint32_t nd = bvn_dom_arr_ndims(cur);
		if (nd >= 2u) {
			if (k + 2u > nidx) return NULL;       /* matrix needs [row][col] */
			uint32_t r = idx[k], c = idx[k + 1u];
			if (r >= nd) return NULL;
			/* rows_per_dim records the width of each of the first 8 rows.
			 * For r < 8 the row base is the exact sum of the preceding row
			 * widths (this is correct for jagged rows, and equals r*cols for
			 * a rectangular matrix); the row's own width is rows_per_dim[r].
			 * For r >= 8 the per-row widths are no longer recorded, so we can
			 * only resolve a matrix whose recorded rows are all uniform, in
			 * which case it is rectangular and base = r*cols. */
			uint64_t base, cols;
			if (r < 8u) {
				cols = bvn_dom_arr_row(cur, r);
				base = 0;
				for (uint32_t i = 0; i < r; i++)
					base += bvn_dom_arr_row(cur, i);
			} else {
				uint32_t w0 = bvn_dom_arr_row(cur, 0);
				bool uniform = (w0 != 0u);
				for (uint32_t i = 1; i < 8u; i++)
					if (bvn_dom_arr_row(cur, i) != w0) { uniform = false; break; }
				if (!uniform) return NULL;
				cols = w0;
				base = (uint64_t)r * w0;
			}
			if (cols == 0u || (uint64_t)c >= cols) return NULL;
			uint64_t off = base + c;
			if (off >= cur->val.arr.count) return NULL;
			cur = cur->val.arr.items[off];
			k += 2u;
		} else {
			if (idx[k] >= cur->val.arr.count) return NULL;
			cur = cur->val.arr.items[idx[k]];
			k += 1u;
		}
	}
	return cur;
}
/*
 * Resolve a path like ".a.b.c" — or ".matrix[0][1]" / ".rows[2].name" with array
 * indexing (spec 1.1) — to a node, walking struct members and array elements
 * from the document root. A leading dot is optional. Descending into a struct
 * member requires a struct; a [N] run requires an array (see
 * bvn_dom_apply_indices). Any missing key, non-array index, out-of-range index,
 * or malformed [N] returns NULL. This is also what the CLI `query` command and
 * application-driven reference resolution use, so indexing works there too.
 */
bvn_dom_node_t *bvn_dom_lookup(const bvn_dom_doc_t *doc,
							   const char *path)
{
	if (!doc || !path) return NULL;
	const char *p = path;
	if (*p == '.') p++;
	if (!*p || *p == '[') return NULL;     /* must start with a key */
	bvn_dom_node_t *cur = NULL;
	bool first = true;
	while (*p) {
		/* a key segment runs up to the next '.', '[', or end */
		const char *start = p;
		while (*p && *p != '.' && *p != '[') p++;
		uint32_t seg_len = (uint32_t)(p - start);
		if (!seg_len) return NULL;
		char segment[256];
		if (seg_len >= sizeof(segment)) return NULL;
		memcpy(segment, start, seg_len);
		segment[seg_len] = '\0';
		if (first) {
			cur = NULL;
			for (uint32_t i = 0; i < doc->count; i++) {
				if (strcmp(doc->entries[i].key, segment) == 0) {
					cur = doc->entries[i].value;
					break;
				}
			}
			first = false;
		} else {
			if (cur->type != BVN_DOM_STRUCT) return NULL;
			cur = bvn_dom_struct_get(cur, segment);
		}
		if (!cur) return NULL;
		/* a run of [N] indices addressing the segment's array value */
		if (*p == '[') {
			uint32_t idx[32] = {0};
			uint32_t nidx = 0;
			while (*p == '[') {
				p++;
				if (*p < '0' || *p > '9') return NULL;
				uint64_t v = 0;
				while (*p >= '0' && *p <= '9') {
					v = v * 10u + (uint64_t)(*p - '0');
					if (v > UINT32_MAX) return NULL;
					p++;
				}
				if (*p != ']') return NULL;
				p++;
				if (nidx >= 32u) return NULL;
				idx[nidx++] = (uint32_t)v;
			}
			cur = bvn_dom_apply_indices(cur, idx, nidx);
			if (!cur) return NULL;
		}
		if (*p == '.') { p++; if (!*p) return NULL; continue; }
		if (*p == '\0') break;
		return NULL;                       /* unexpected trailing byte */
	}
	return cur;
}
bvn_dom_node_t *bvn_dom_array_at(const bvn_dom_node_t *node,
								 uint32_t index)
{
	if (!node || node->type != BVN_DOM_ARRAY) return NULL;
	if (index >= node->val.arr.count) return NULL;
	return node->val.arr.items[index];
}
bvn_dom_type_t bvn_dom_node_type(const bvn_dom_node_t *node)
{
	return node ? node->type : BVN_DOM_NULL;
}
bool bvn_dom_is_null(const bvn_dom_node_t *node)
{
	return node && node->type == BVN_DOM_NULL;
}
bool bvn_dom_get_bool(const bvn_dom_node_t *node, bool *out)
{
	if (!node || !out || node->type != BVN_DOM_BOOL) return false;
	*out = node->val.int_val != 0;
	return true;
}
/* True for a family whose narrow carrier is a SIGNED int64 (so a negative value
 * is genuinely negative rather than a large unsigned one). datetime counts:
 * epoch seconds run before 1970. */
static bool dom_family_is_signed(const bvn_dom_node_t *node)
{
	return node->value_type.family == vt_sint ||
	       node->value_type.family == vt_datetime;
}
bool bvn_dom_get_float(const bvn_dom_node_t *node, double *out)
{
	if (!node || !out) return false;
	if (node->type == BVN_DOM_FLOAT) {
		*out = node->val.float_val;
		return true;
	}
	if (node->type == BVN_DOM_INT) {
		if (node->value_type.width > 64u) {
			/* A bignum beyond int64 is still a perfectly good double. Try the
			 * cheap integer paths, then fall back to its decimal text: funnelling
			 * everything through bvn_int_to_int64 made a uint:128 of 1e20 fail,
			 * and bvn_dom_get_value_in_base_units turned that failure into an
			 * exact 0.0 — indistinguishable from its "no SI mapping" signal. */
			int64_t v;
			if (node->val.bigint && bvn_int_to_int64(node->val.bigint, &v)) {
				*out = (double)v;
				return true;
			}
			uint64_t uv;
			if (node->val.bigint && bvn_int_to_uint64(node->val.bigint, &uv)) {
				*out = (double)uv;
				return true;
			}
			if (!node->val.bigint) return false;
			{
				int bits = bvn_int_bitlen(node->val.bigint);
				size_t sz = bvn_int_str_bufsize(
					(uint32_t)(bits > 0 ? bits : 1), 10u);
				char *txt = malloc(sz);
				if (!txt) return false;
				bool ok = bvn_int_to_str(node->val.bigint, txt, sz, 10u) >= 0;
				if (ok) *out = bvn_float_strtod(txt);
				free(txt);
				return ok;
			}
		}
		/* An unsigned width-64 value above INT64_MAX must not be read back as a
		 * negative double. */
		if (!dom_family_is_signed(node) && node->val.int_val < 0) {
			*out = (double)(uint64_t)node->val.int_val;
			return true;
		}
		*out = (double)node->val.int_val;
		return true;
	}
	return false;
}
bool bvn_dom_get_string(const bvn_dom_node_t *node,
						const char **out, uint32_t *len)
{
	if (!node || node->type != BVN_DOM_STRING || !out) return false;
	*out = node->val.str.data;
	if (len) *len = node->val.str.len;
	return true;
}
bool bvn_dom_get_symbol(const bvn_dom_node_t *node,
						const char **out, uint32_t *len)
{
	if (!node || node->type != BVN_DOM_SYMBOL || !out) return false;
	*out = node->val.str.data;
	if (len) *len = node->val.str.len;
	return true;
}
bool bvn_dom_get_reference(const bvn_dom_node_t *node,
						   const char **out, uint32_t *len)
{
	if (!node || node->type != BVN_DOM_REFERENCE || !out) return false;
	*out = node->val.str.data;
	if (len) *len = node->val.str.len;
	return true;
}
bool bvn_dom_get_octets(const bvn_dom_node_t *node,
						const uint8_t **out, uint32_t *len)
{
	if (!node || node->type != BVN_DOM_OCTET_STREAM || !out) return false;
	*out = node->val.octets.data;
	if (len) *len = node->val.octets.len;
	return true;
}
value_type_spec_t bvn_dom_get_value_type(const bvn_dom_node_t *node)
{
	return node ? node->value_type : BVN_TYPE_PLAIN;
}
value_unit_t bvn_dom_get_unit(const bvn_dom_node_t *node)
{
	return node ? node->unit->unit : BVN_UNIT_NO_PREFIX(bu_none);
}
int32_t bvn_dom_get_unit_string(const bvn_dom_node_t *node,
								char *buf, size_t bufsize)
{
	if (!node) return -1;
	return bvn_unit_to_string(node->unit->unit, buf, bufsize);
}
/*
 * Convert a numeric node to its value expressed in SI base units, applying the
 * unit's scale factor (and offset for affine units such as °C/°F, where the
 * conversion is value*factor + offset rather than a pure scaling). This is the
 * payoff of carrying units in the type system: e.g. a node "5 km" returns 5000.
 * Non-numeric nodes or units with no SI mapping return 0.0.
 */
double bvn_dom_get_value_in_base_units(const bvn_dom_node_t *node)
{
	if (!node) return 0.0;
	double raw;
	if (node->type == BVN_DOM_FLOAT) {
		raw = node->val.float_val;
	} else if (node->type == BVN_DOM_INT) {
		/* Reuse the accessor rather than repeating the carrier logic: this used
		 * to funnel a bignum through bvn_int_to_int64 and return 0.0 when it did
		 * not fit — the same value this function uses to signal "no SI mapping",
		 * so a caller could not tell 1e23 metres from an unconvertible unit. */
		if (!bvn_dom_get_float(node, &raw))
			return 0.0;
	} else {
		return 0.0;
	}
	bool is_affine      = false;
	double affine_offset = 0.0;
	bool si_ok           = true;
	double factor = bvn_unit_to_si_factor(node->unit->unit,
										   &is_affine,
										   &affine_offset,
										   &si_ok);
	if (!si_ok)
		return 0.0;
	if (is_affine)
		return raw * factor + affine_offset;
	return raw * factor;
}
uint32_t bvn_dom_struct_count(const bvn_dom_node_t *node)
{
	return (node && node->type == BVN_DOM_STRUCT)
		   ? node->val.members.count : 0;
}
uint32_t bvn_dom_array_count(const bvn_dom_node_t *node)
{
	return (node && node->type == BVN_DOM_ARRAY)
		   ? node->val.arr.count : 0;
}
uint32_t bvn_dom_array_dims(const bvn_dom_node_t *node)
{
	return (node && node->type == BVN_DOM_ARRAY)
		   ? bvn_dom_arr_ndims(node) : 0;
}
const bvn_dom_entry_t *bvn_dom_struct_entries(const bvn_dom_node_t *node)
{
	return (node && node->type == BVN_DOM_STRUCT)
		   ? node->val.members.entries : NULL;
}
const bvn_dom_entry_t *bvn_dom_doc_entries(const bvn_dom_doc_t *doc)
{
	return doc ? doc->entries : NULL;
}
uint32_t bvn_dom_doc_count(const bvn_dom_doc_t *doc)
{
	return doc ? doc->count : 0;
}
error_code_t bvn_dom_doc_get_parse_error(const bvn_dom_doc_t *doc)
{
	return doc ? doc->parse_error : error_none;
}
const bvn_int_t *bvn_dom_get_bigint(const bvn_dom_node_t *node)
{
	if (!node || node->type != BVN_DOM_INT) return NULL;
	if (node->value_type.width <= 64u)      return NULL;
	return node->val.bigint;
}
/*
 * Render an integer node to a freshly allocated string in any base, hiding the
 * narrow-vs-bignum split: wide values format via bvn_int_to_str, narrow ones
 * via the int/uint formatters chosen by signedness. The caller owns the result
 * (bvn_dom_free_string). This is the only way to read a >64-bit DOM integer as
 * a value, since it can't fit any C integer type.
 */
char *bvn_dom_int_to_str(const bvn_dom_node_t *node, uint32_t base)
{
	if (!node || node->type != BVN_DOM_INT) return NULL;
	if (node->value_type.width > 64u) {
		if (!node->val.bigint) return NULL;
		size_t bufsz = bvn_int_str_bufsize(node->value_type.width, base);
		char  *buf   = malloc(bufsz);
		if (!buf) return NULL;
		if (bvn_int_to_str(node->val.bigint, buf, bufsz, base) < 0) {
			free(buf);
			return NULL;
		}
		return buf;
	}
	char    tmp[128];
	int32_t slen;
	if (node->value_type.family == vt_sint ||
	    node->value_type.family == vt_datetime)   /* datetime carrier is signed */
		slen = bvn_format_int64(tmp, sizeof(tmp),
								node->val.int_val, base, 0u);
	else
		slen = bvn_format_uint64(tmp, sizeof(tmp),
								 (uint64_t)node->val.int_val, base, 0u);
	if (slen < 0) return NULL;
	char *out = malloc((size_t)slen + 1u);
	if (!out) return NULL;
	memcpy(out, tmp, (size_t)slen + 1u);
	return out;
}
void bvn_dom_free_string(char *s)
{
	free(s);
}
const char *bvn_dom_get_datetime_fraction(const bvn_dom_node_t *node,
		uint32_t *len_out)
{
	if (len_out) *len_out = 0;
	if (!node || node->type != BVN_DOM_INT ||
	    node->value_type.family != vt_datetime || !node->ext.dt_frac)
		return NULL;
	if (len_out) *len_out = (uint32_t)strlen(node->ext.dt_frac);
	return node->ext.dt_frac;
}
const char *bvn_dom_get_unit_source(const bvn_dom_node_t *node,
		uint32_t *len_out)
{
	if (len_out) *len_out = 0;
	if (!node || !node->unit->src)
		return NULL;
	if (len_out) *len_out = node->unit->src_len;
	return node->unit->src;
}
/*
 * The two raw integer extractors behind every bvn_dom_get_iN / get_uN
 * accessor. They
 * normalise the narrow/bignum storage split into a single int64/uint64, failing
 * if a wide value doesn't fit 64 bits or a negative value is requested as
 * unsigned. The public fixed-width getters (get_i32, get_u8, ...) layer a range
 * check on top, so each returns false rather than silently truncating when the
 * value is out of range for the requested C type. make_narrow_int and the
 * bvn_dom_node_from_* constructors are the symmetric builders.
 */
static bool dom_raw_i64(const bvn_dom_node_t *node, int64_t *out)
{
	if (!node || node->type != BVN_DOM_INT || !out) return false;
	if (node->value_type.width > 64u)
		return node->val.bigint && bvn_int_to_int64(node->val.bigint, out);
	/* A width-64 uint above INT64_MAX does not fit an int64. The narrow carrier
	 * holds its bit pattern, and handing that back reinterpreted as signed
	 * returned -1 for 18446744073709551615 — with a true return, against this
	 * header's explicit promise of "no clamping or truncation". The fixed-width
	 * wrappers then range-check the already-wrong number, so get_i8 "succeeded"
	 * too. */
	if (!dom_family_is_signed(node) && node->val.int_val < 0)
		return false;
	*out = node->val.int_val;
	return true;
}
static bool dom_raw_u64(const bvn_dom_node_t *node, uint64_t *out)
{
	if (!node || node->type != BVN_DOM_INT || !out) return false;
	if (node->value_type.width > 64u)
		return node->val.bigint && bvn_int_to_uint64(node->val.bigint, out);
	/* Guarding only vt_sint let a pre-epoch datetime through: its carrier is
	 * signed too, so -315619200 came back as 18446744073393932416. */
	if (dom_family_is_signed(node) && node->val.int_val < 0)
		return false;
	*out = (uint64_t)node->val.int_val;
	return true;
}
bool bvn_dom_get_i64(const bvn_dom_node_t *node, int64_t *out)
{ return dom_raw_i64(node, out); }
bool bvn_dom_get_u64(const bvn_dom_node_t *node, uint64_t *out)
{ return dom_raw_u64(node, out); }
bool bvn_dom_get_i32(const bvn_dom_node_t *node, int32_t *out)
{
	int64_t v;
	if (!dom_raw_i64(node, &v)) return false;
	if (v < (int64_t)INT32_MIN || v > (int64_t)INT32_MAX) return false;
	*out = (int32_t)v;
	return true;
}
bool bvn_dom_get_u32(const bvn_dom_node_t *node, uint32_t *out)
{
	uint64_t v;
	if (!dom_raw_u64(node, &v)) return false;
	if (v > (uint64_t)UINT32_MAX) return false;
	*out = (uint32_t)v;
	return true;
}
bool bvn_dom_get_i16(const bvn_dom_node_t *node, int16_t *out)
{
	int64_t v;
	if (!dom_raw_i64(node, &v)) return false;
	if (v < (int64_t)INT16_MIN || v > (int64_t)INT16_MAX) return false;
	*out = (int16_t)v;
	return true;
}
bool bvn_dom_get_u16(const bvn_dom_node_t *node, uint16_t *out)
{
	uint64_t v;
	if (!dom_raw_u64(node, &v)) return false;
	if (v > (uint64_t)UINT16_MAX) return false;
	*out = (uint16_t)v;
	return true;
}
bool bvn_dom_get_i8(const bvn_dom_node_t *node, int8_t *out)
{
	int64_t v;
	if (!dom_raw_i64(node, &v)) return false;
	if (v < (int64_t)INT8_MIN || v > (int64_t)INT8_MAX) return false;
	*out = (int8_t)v;
	return true;
}
bool bvn_dom_get_u8(const bvn_dom_node_t *node, uint8_t *out)
{
	uint64_t v;
	if (!dom_raw_u64(node, &v)) return false;
	if (v > (uint64_t)UINT8_MAX) return false;
	*out = (uint8_t)v;
	return true;
}
static bvn_dom_node_t *make_narrow_int(int64_t            raw,
										value_type_family_t fam,
										uint32_t            width)
{
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_INT);
	if (!n) return NULL;
	n->value_type = (value_type_spec_t){ .family = fam,
										 .width  = width,
										 .base   = 0u };
	n->unit = &bvn_dom_unit_none;
	n->val.int_val    = raw;
	return n;
}
bvn_dom_node_t *bvn_dom_node_from_i64(int64_t v)
{ return make_narrow_int(v,                     vt_sint, 64u); }
bvn_dom_node_t *bvn_dom_node_from_u64(uint64_t v)
{ return make_narrow_int((int64_t)v,            vt_uint, 64u); }
bvn_dom_node_t *bvn_dom_node_from_i32(int32_t v)
{ return make_narrow_int((int64_t)v,            vt_sint, 32u); }
bvn_dom_node_t *bvn_dom_node_from_u32(uint32_t v)
{ return make_narrow_int((int64_t)(uint64_t)v,  vt_uint, 32u); }
bvn_dom_node_t *bvn_dom_node_from_i16(int16_t v)
{ return make_narrow_int((int64_t)v,            vt_sint, 16u); }
bvn_dom_node_t *bvn_dom_node_from_u16(uint16_t v)
{ return make_narrow_int((int64_t)(uint64_t)v,  vt_uint, 16u); }
bvn_dom_node_t *bvn_dom_node_from_i8(int8_t v)
{ return make_narrow_int((int64_t)v,            vt_sint,  8u); }
bvn_dom_node_t *bvn_dom_node_from_u8(uint8_t v)
{ return make_narrow_int((int64_t)(uint64_t)v,  vt_uint,  8u); }
bvn_dom_node_t *bvn_dom_node_from_bigint(bvn_int_t     *bigint,
										  value_type_spec_t vt,
										  value_unit_t      vu)
{
	if (!bigint) return NULL;
	if (vt.width <= 64u) return NULL;
	if (vt.family != vt_sint && vt.family != vt_uint) return NULL;
	/* The value has to fit the type it is declared as, as it does for every
	 * other way into an integer node: a negative uint:128 answered -5 from
	 * bvn_dom_get_i64, and a 4000-bit value under uint:128 made
	 * bvn_dom_int_to_str (which sizes its buffer from the width) fail. */
	{
		int  bl  = bvn_int_bitlen(bigint);
		bool neg = bigint->negative && !bvn_int_is_zero(bigint);
		if (bl < 0) return NULL;
		if (vt.family == vt_uint) {
			if (neg || (uint32_t)bl > vt.width) return NULL;
		} else if (!neg) {
			if ((uint32_t)bl > vt.width - 1u) return NULL;
		} else if ((uint32_t)bl > vt.width) {
			return NULL;
		} else if ((uint32_t)bl == vt.width) {
			/* only -2^(width-1) has a magnitude of `width` bits */
			uint32_t ones = 0;
			for (uint32_t i = 0; i < bigint->nused; i++)
				for (uint32_t x = bigint->limbs[i]; x; x &= x - 1u)
					ones++;
			if (ones != 1u) return NULL;
		}
	}
	bvn_dom_node_t *n = bvn_dom_node_alloc(BVN_DOM_INT);
	if (!n) return NULL;
	n->value_type = vt;
	if (!bvn_dom_node_set_unit(n, &vu)) {
		free(n);
		return NULL;
	}
	n->val.bigint     = bigint;
	return n;
}
