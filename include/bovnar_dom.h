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

#ifndef BOVNAR_DOM_H_
#define BOVNAR_DOM_H_
#include "bovnar.h"
#include "bvn_int.h"
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef enum bvn_dom_type_e {
	BVN_DOM_NULL,
	BVN_DOM_INT,
	BVN_DOM_FLOAT,
	BVN_DOM_STRING,
	BVN_DOM_SYMBOL,
	BVN_DOM_REFERENCE,
	BVN_DOM_STRUCT,
	BVN_DOM_ARRAY,
	BVN_DOM_OCTET_STREAM,
	BVN_DOM_BOOL
} bvn_dom_type_t;
typedef struct bvn_dom_node_s  bvn_dom_node_t;
typedef struct bvn_dom_doc_s   bvn_dom_doc_t;
typedef struct bvn_dom_entry_s {
	char            *key;
	bvn_dom_node_t  *value;
} bvn_dom_entry_t;
BVN_API bvn_dom_doc_t *bvn_dom_doc_create(void);
/* Free a document and its whole node tree. NULL-safe. Not double-free-safe: the
 * pointer is dangling afterwards. (A pathological, very deeply nested tree may
 * leak its lower levels rather than crash if an internal work-stack allocation
 * fails under memory pressure — correctness is never traded for a crash.) */
BVN_API void           bvn_dom_doc_destroy(bvn_dom_doc_t *doc);
/* Every bvn_dom_parse* function reads at the format's hard nesting limit -- 255
 * levels of struct and 255 of array -- not at the reader's default of 64, so a
 * document nested 65..255 deep parses here and is refused by a bvnr_reader_t
 * opened with zeroed flags; set max_struct_nesting / max_array_nesting to 255
 * there to make the two agree.
 *
 * Parse an in-memory document. Returns NULL ONLY on allocation failure. A
 * malformed document still returns a non-NULL doc whose error is retrieved with
 * bvn_dom_doc_get_parse_error() (error_none means it parsed cleanly) — so check
 * that, not the pointer, to detect a parse error. Free with bvn_dom_doc_destroy. */
BVN_API bvn_dom_doc_t *bvn_dom_parse(const void *data, uint32_t len);
/* Read and parse a whole fd. Unlike bvn_dom_parse(), these return NULL on ANY
 * failure (allocation, I/O/read error, or input exceeding the size cap) with no
 * distinguishing code; a non-NULL result may still carry a parse error retrieved
 * via bvn_dom_doc_get_parse_error(). bvn_dom_parse_fd uses the default cap;
 * parse_fd_ex caps the accumulated input at max_bytes, but only DOWNWARD: a
 * max_bytes of 0, or any value above the built-in hard cap (BVN_DOM_FD_MAX_BYTES,
 * 256 MiB), is clamped to that hard cap — there is no unlimited mode. Free with
 * bvn_dom_doc_destroy. */
BVN_API bvn_dom_doc_t *bvn_dom_parse_fd(int fd);
BVN_API bvn_dom_doc_t *bvn_dom_parse_fd_ex(int fd, uint64_t max_bytes);
/* The same two, under a reader unit policy (bvnr_reader_set_unit_policy) — what
 * the document must contain, and what unit its values should be stored in. NULL
 * behaves exactly like the plain forms.
 *
 * Validation failures land in bvn_dom_doc_get_parse_error() as
 * error_unit_mismatch, like any other parse error. A policy the library refuses
 * (a malformed unit, a rule path naming nothing) is error_invalid_argument, so a
 * mistake in the policy is never mistaken for a fault in the document.
 *
 * A value the policy CONVERTED is stored converted: its digits, its unit and its
 * base are the conversion's, since a caller who asked for metres and got the
 * document's feet back would have no way to notice. An integer that converts to
 * a fraction (5 g in kilograms is 0.005) is stored as a float, because that is
 * what it now is. A conversion the reader could not deliver exactly is governed
 * by the policy's on_inexact, exactly as in the streaming reader. */
BVN_API bvn_dom_doc_t *bvn_dom_parse_policy(const void *data, uint32_t len,
					    const bvnr_unit_policy_t *policy);
BVN_API bvn_dom_doc_t *bvn_dom_parse_fd_policy(int fd, uint64_t max_bytes,
					       const bvnr_unit_policy_t *policy);
BVN_API error_code_t   bvn_dom_doc_get_parse_error(const bvn_dom_doc_t *doc);
BVN_API bvn_dom_node_t *bvn_dom_lookup(const bvn_dom_doc_t *doc,
							   const char *path);
BVN_API bvn_dom_node_t *bvn_dom_struct_get(const bvn_dom_node_t *node,
								   const char *key);
BVN_API bvn_dom_node_t *bvn_dom_array_at(const bvn_dom_node_t *node,
								 uint32_t index);
BVN_API bvn_dom_type_t  bvn_dom_node_type(const bvn_dom_node_t *node);
BVN_API bool            bvn_dom_is_null(const bvn_dom_node_t *node);
BVN_API bool bvn_dom_get_float(const bvn_dom_node_t *node, double *out);
BVN_API bool bvn_dom_get_bool(const bvn_dom_node_t *node, bool *out);
/* Accessors: return false (leaving the out-params unchanged) when the node is NULL or
 * not of the requested kind. The returned pointer is BORROWED — it points into
 * the node and is valid only until the owning document is destroyed; do not free
 * it. The string/symbol/reference data is NUL-terminated; *len excludes the NUL.
 * Octet-stream data (bvn_dom_get_octets) is raw bytes and is NOT NUL-terminated;
 * use *len exactly. */
BVN_API bool bvn_dom_get_string(const bvn_dom_node_t *node,
						const char **out, uint32_t *len);
BVN_API bool bvn_dom_get_symbol(const bvn_dom_node_t *node,
						const char **out, uint32_t *len);
BVN_API bool bvn_dom_get_reference(const bvn_dom_node_t *node,
						   const char **out, uint32_t *len);
BVN_API bool bvn_dom_get_octets(const bvn_dom_node_t *node,
						const uint8_t **out, uint32_t *len);
BVN_API value_type_spec_t bvn_dom_get_value_type(const bvn_dom_node_t *node);
BVN_API value_unit_t      bvn_dom_get_unit(const bvn_dom_node_t *node);
BVN_API int32_t bvn_dom_get_unit_string(const bvn_dom_node_t *node,
								char *buf, size_t bufsize);
BVN_API double bvn_dom_get_value_in_base_units(const bvn_dom_node_t *node);
BVN_API uint32_t bvn_dom_struct_count(const bvn_dom_node_t *node);
BVN_API uint32_t bvn_dom_array_count(const bvn_dom_node_t *node);
BVN_API uint32_t bvn_dom_array_dims(const bvn_dom_node_t *node);
BVN_API const bvn_dom_entry_t *bvn_dom_struct_entries(const bvn_dom_node_t *node);
BVN_API const bvn_dom_entry_t *bvn_dom_doc_entries(const bvn_dom_doc_t *doc);
BVN_API uint32_t               bvn_dom_doc_count(const bvn_dom_doc_t *doc);
BVN_API bvn_dom_node_t *bvn_dom_node_alloc(bvn_dom_type_t t);
/* Free a node and its subtree. NULL-safe; not double-free-safe. See the note on
 * bvn_dom_doc_destroy about deep trees under memory pressure. */
BVN_API void            bvn_dom_node_destroy(bvn_dom_node_t *n);
/* Add a value to a struct/doc/array. OWNERSHIP: these ALWAYS take ownership of
 * `val`/`elem` — on success it is owned by the container, and on EVERY failure
 * path (return false) it is destroyed internally. So never destroy it yourself
 * after the call; in particular do NOT write
 *     if (!bvn_dom_struct_add(s, k, n, v)) bvn_dom_node_destroy(v); // DOUBLE FREE
 * The container pointer (s/d/a) must be non-NULL. The key is copied. */
BVN_API bool bvn_dom_struct_add(bvn_dom_node_t *s,
						const char *key, uint32_t klen,
						bvn_dom_node_t *val);
BVN_API bool bvn_dom_doc_add(bvn_dom_doc_t *d,
					 const char *key, uint32_t klen,
					 bvn_dom_node_t *val);
BVN_API bool bvn_dom_array_append(bvn_dom_node_t *a, bvn_dom_node_t *elem);
BVN_API char *bvn_dom_strdup(const char *s, uint32_t len);
/* Borrowed bigint view, owned by the node (do not free). Returns NULL unless the
 * node is an integer WIDER than 64 bits; narrow integers are stored inline, so
 * read those with bvn_dom_get_i64/u64 (etc.), not this. */
BVN_API const bvn_int_t *bvn_dom_get_bigint(const bvn_dom_node_t *node);
/* Render an integer node in `base`. Returns a heap string the CALLER OWNS and
 * must release with bvn_dom_free_string() (NULL on a non-integer node or alloc
 * failure). */
BVN_API char *bvn_dom_int_to_str(const bvn_dom_node_t *node, uint32_t base);
BVN_API void  bvn_dom_free_string(char *s);
/* library 2.0 — the unit of a value node exactly as the document spelled it
 * (NUL-terminated, owned by the node — do NOT free), where that spelling says
 * something bvn_dom_get_unit() cannot: "ucum:mL{total}" is the unit m~L, and
 * the annotation survives only here. Returns NULL — and the canonical
 * bvn_unit_to_string() of the node's unit IS the spelling — for a node with no
 * unit, for one written in canonical form already, and for one whose value a
 * unit policy converted (the document's spelling describes a unit the node no
 * longer has). *len_out (when non-NULL) receives the length, 0 with NULL. */
BVN_API const char *bvn_dom_get_unit_source(const bvn_dom_node_t *node,
		uint32_t *len_out);
/* spec 1.1 — verbatim ISO sub-second digits of a datetime node written as a
 * literal with a `.frac` part (the digits only, NUL-terminated, owned by the
 * node — do NOT free). Returns NULL for a non-datetime node, a datetime given
 * as an integer carrier, or a literal with no fraction; *len_out (when non-NULL)
 * receives the digit count (0 when NULL is returned). The node's integer value
 * is still the whole-second epoch count read via bvn_dom_get_i64(). */
BVN_API const char *bvn_dom_get_datetime_fraction(const bvn_dom_node_t *node,
		uint32_t *len_out);
/* Fetch an integer node as a fixed-width type. Return false (leaving *out
 * UNCHANGED — no clamping or truncation) when the node is not an integer or its
 * value does not fit the target type. Always check the return value. */
BVN_API bool bvn_dom_get_i64(const bvn_dom_node_t *node, int64_t  *out);
BVN_API bool bvn_dom_get_u64(const bvn_dom_node_t *node, uint64_t *out);
BVN_API bool bvn_dom_get_i32(const bvn_dom_node_t *node, int32_t  *out);
BVN_API bool bvn_dom_get_u32(const bvn_dom_node_t *node, uint32_t *out);
BVN_API bool bvn_dom_get_i16(const bvn_dom_node_t *node, int16_t  *out);
BVN_API bool bvn_dom_get_u16(const bvn_dom_node_t *node, uint16_t *out);
BVN_API bool bvn_dom_get_i8 (const bvn_dom_node_t *node, int8_t   *out);
BVN_API bool bvn_dom_get_u8 (const bvn_dom_node_t *node, uint8_t  *out);
BVN_API bvn_dom_node_t *bvn_dom_node_from_i64(int64_t  v);
BVN_API bvn_dom_node_t *bvn_dom_node_from_u64(uint64_t v);
BVN_API bvn_dom_node_t *bvn_dom_node_from_i32(int32_t  v);
BVN_API bvn_dom_node_t *bvn_dom_node_from_u32(uint32_t v);
BVN_API bvn_dom_node_t *bvn_dom_node_from_i16(int16_t  v);
BVN_API bvn_dom_node_t *bvn_dom_node_from_u16(uint16_t v);
BVN_API bvn_dom_node_t *bvn_dom_node_from_i8 (int8_t   v);
BVN_API bvn_dom_node_t *bvn_dom_node_from_u8 (uint8_t  v);
/* Build an integer node from a bvn_int_t. `vt` must be uint or sint wider than
 * 64 bits and the value must fit it (NULL otherwise). OWNERSHIP is ASYMMETRIC:
 * on success the returned node takes ownership of `bigint`; on failure (returns
 * NULL) it does NOT — the caller still owns `bigint` and must free it with
 * bvn_int_free. */
BVN_API bvn_dom_node_t *bvn_dom_node_from_bigint(bvn_int_t     *bigint,
										  value_type_spec_t vt,
										  value_unit_t      vu);
#ifdef __cplusplus
}
#endif
#endif
