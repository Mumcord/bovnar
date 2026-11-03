/*
 * SPDX-License-Identifier: MIT
 *
 * Copyright (c) 2026 Janos Sonntag
 *
 * bvnr_wasm.c — WebAssembly entry points around the Bovnar C reference parser.
 *
 * This wraps the amalgamated single-file library (build/amalgamate/bovnar.{c,h})
 * with a handful of stable, string-in / JSON-string-out functions suitable for
 * calling across the JS<->WASM boundary. These surfaces are exposed:
 *
 *   bvnr_wasm_validate(ptr,len) -> JSON  { ok, error, error_name, tier, line,
 *                                          column, byte, offset, recovery_count,
 *                                          declared_version }
 *   bvnr_wasm_to_json (ptr,len) -> JSON  { ok, error, error_name, value }
 *                                          where `value` is the DOM projected to
 *                                          plain JSON (units dropped — see notes)
 *   bvnr_wasm_events  (ptr,len) -> JSON  { ok, error, error_name, events:[...] }
 *                                          the reference event stream (verified)
 *   bvnr_wasm_events_convert(ptr,len,unit,base,allow_nonterminating)
 *                               -> the same stream with the reader's lossless
 *                                  unit/base conversion armed, so events carry
 *                                  converted / converted_base / converted_unit
 *   bvnr_wasm_parse   (ptr,len) -> JSON  { ok, error, error_name, events:[...],
 *                                          errors:[...], declared_version }
 *                                          events + errors from ONE reader pass;
 *                                          what a caller wanting both should use
 *   bvnr_wasm_errors  (ptr,len) -> JSON  { ok, errors:[...], declared_version }
 *                                          every error, in resync mode
 *   bvnr_wasm_doc     (ptr,len) -> JSON  { ok, error, error_name, tree:[...] }
 *                                          the document tree with type fields
 *   bvnr_wasm_version()         -> the library version string
 *
 * The caller passes a pointer+length (NOT a NUL-terminated string), because a
 * .bvnr document may carry octet streams with embedded NULs. Every returning
 * function hands back a malloc'd, NUL-terminated UTF-8 JSON string that the JS
 * side must release with bvnr_wasm_free().
 *
 * Notes / current limitations (documented, not hidden):
 *   - to_json is a *projection* of the validated DOM to plain JSON values; it
 *     drops unit/type metadata (use events for that). Big integers are emitted
 *     as JSON number tokens with their exact decimal digits (lossless at the
 *     JSON-text level, but a naive JS JSON.parse will round them to double);
 *     integers wider than 64 bits are emitted as JSON strings to stay lossless.
 *     Non-finite floats (inf/-inf/nan) are emitted as JSON null (JSON has no
 *     native infinity/NaN). Octet streams are emitted as a lowercase hex
 *     string. Symbols and references are emitted as their text.
 */
#include "bovnar.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <stdarg.h>
#include <math.h>

#ifdef __EMSCRIPTEN__
#  include <emscripten.h>
#  define WASM_EXPORT EMSCRIPTEN_KEEPALIVE
#else
#  define WASM_EXPORT
#endif

/* ------------------------------------------------------------------ */
/* growable byte buffer                                               */
/* ------------------------------------------------------------------ */
typedef struct {
	char  *p;
	size_t len;
	size_t cap;
	int    oom;
} sb_t;

static void sb_init(sb_t *b) { b->p = NULL; b->len = 0; b->cap = 0; b->oom = 0; }

static int sb_reserve(sb_t *b, size_t extra)
{
	if (b->oom) return 0;
	if (b->len + extra + 1 <= b->cap) return 1;
	size_t ncap = b->cap ? b->cap * 2 : 256;
	while (ncap < b->len + extra + 1) ncap *= 2;
	char *np = (char *)realloc(b->p, ncap);
	if (!np) { b->oom = 1; return 0; }
	b->p = np;
	b->cap = ncap;
	return 1;
}

static void sb_putc(sb_t *b, char c)
{
	if (!sb_reserve(b, 1)) return;
	b->p[b->len++] = c;
}

static void sb_put(sb_t *b, const char *s, size_t n)
{
	if (!sb_reserve(b, n)) return;
	memcpy(b->p + b->len, s, n);
	b->len += n;
}

static void sb_puts(sb_t *b, const char *s) { sb_put(b, s, strlen(s)); }

static void sb_printf(sb_t *b, const char *fmt, ...)
{
	char tmp[160];
	va_list ap;
	va_start(ap, fmt);
	int n = vsnprintf(tmp, sizeof(tmp), fmt, ap);
	va_end(ap);
	if (n < 0) return;
	if ((size_t)n < sizeof(tmp)) { sb_put(b, tmp, (size_t)n); return; }
	/* rare: needs a bigger scratch */
	if (!sb_reserve(b, (size_t)n)) return;
	va_start(ap, fmt);
	vsnprintf(b->p + b->len, (size_t)n + 1, fmt, ap);
	va_end(ap);
	b->len += (size_t)n;
}

/* Append `n` bytes of `s` as the *contents* of a JSON string (no surrounding
 * quotes), escaping per RFC 8259. Bytes are assumed UTF-8; raw control bytes
 * and the JSON metacharacters are escaped, everything else passes through. */
static void sb_json_str_body(sb_t *b, const char *s, size_t n)
{
	for (size_t i = 0; i < n; i++) {
		unsigned char c = (unsigned char)s[i];
		switch (c) {
		case '"':  sb_puts(b, "\\\""); break;
		case '\\': sb_puts(b, "\\\\"); break;
		case '\b': sb_puts(b, "\\b");  break;
		case '\f': sb_puts(b, "\\f");  break;
		case '\n': sb_puts(b, "\\n");  break;
		case '\r': sb_puts(b, "\\r");  break;
		case '\t': sb_puts(b, "\\t");  break;
		default:
			if (c < 0x20) sb_printf(b, "\\u%04x", c);
			else          sb_putc(b, (char)c);
		}
	}
}

static void sb_json_cstr(sb_t *b, const char *s)
{
	sb_putc(b, '"');
	if (s) sb_json_str_body(b, s, strlen(s));
	sb_putc(b, '"');
}

/* Return ownership of the buffer's NUL-terminated string to the caller. On OOM
 * frees what it has and returns a small JSON error (heap-allocated so the
 * caller's free() is always valid).
 *
 * That error has the SAME shape as every other result: ok, a numeric error and
 * its error_name. It used to be {"ok":false,"error":"out_of_memory"} -- `error`
 * a string where every other payload has a number, and no error_name at all, so
 * `r.error_name` was undefined on the one path a caller is least likely to have
 * tested. The library has no code of its own for exhaustion and reports it as
 * error_invalid_argument (see bvnr_read_flags_t.want_unit); `detail` says which
 * it was. */
static char *sb_finish(sb_t *b)
{
	if (b->oom) {
		free(b->p);
		b->p = NULL; b->len = 0; b->cap = 0;
		char oom[160];
		int n = snprintf(oom, sizeof oom,
			"{\"ok\":false,\"error\":%d,\"error_name\":\"%s\","
			"\"detail\":\"out of memory\"}",
			(int)error_invalid_argument,
			bvn_error_to_string(error_invalid_argument));
		if (n < 0 || (size_t)n >= sizeof oom) return NULL;
		char *e = (char *)malloc((size_t)n + 1u);
		if (e) memcpy(e, oom, (size_t)n + 1u);
		return e;
	}
	if (!b->p) { /* empty -> "" */
		char *e = (char *)malloc(1);
		if (e) e[0] = '\0';
		return e;
	}
	b->p[b->len] = '\0';
	return b->p;
}

/* Splice a finished sub-buffer (the events or errors array body) into `into`,
 * and release it.
 *
 * NOT by way of sb_finish: on exhaustion that returns the JSON error OBJECT,
 * and the callers used to paste whatever it returned between "events":[ and ]
 * -- a truncated listing became {"ok":true,...,"events":[{"ok":false,...}]},
 * a well-formed success wrapped around the report of its own failure. An
 * exhausted part makes the whole result the out-of-memory result. */
static void sb_splice(sb_t *into, sb_t *part)
{
	if (part->oom) into->oom = 1;
	else if (part->p) sb_put(into, part->p, part->len);
	free(part->p);
	part->p = NULL; part->len = 0; part->cap = 0;
}

/* ------------------------------------------------------------------ */
/* DOM -> JSON projection                                             */
/* ------------------------------------------------------------------ */
static void json_emit_node(sb_t *b, const bvn_dom_node_t *node);

static void json_emit_entries(sb_t *b, const bvn_dom_entry_t *entries, uint32_t n)
{
	sb_putc(b, '{');
	for (uint32_t i = 0; i < n; i++) {
		if (i) sb_putc(b, ',');
		sb_json_cstr(b, entries[i].key);
		sb_putc(b, ':');
		json_emit_node(b, entries[i].value);
	}
	sb_putc(b, '}');
}

static const char hexd[] = "0123456789abcdef";

static void json_emit_node(sb_t *b, const bvn_dom_node_t *node)
{
	if (!node) { sb_puts(b, "null"); return; }
	switch (bvn_dom_node_type(node)) {
	case BVN_DOM_NULL:
		sb_puts(b, "null");
		break;
	case BVN_DOM_BOOL: {
		bool v = false;
		bvn_dom_get_bool(node, &v);
		sb_puts(b, v ? "true" : "false");
		break;
	}
	case BVN_DOM_INT: {
		char *s = bvn_dom_int_to_str(node, 10);
		if (s) {
			/* Integers wider than 64 bits cannot survive JSON's double-backed
			 * number type, so emit them as a JSON string (matching the CLI's
			 * `convert`); narrow ints stay as number tokens. */
			if (bvn_dom_get_bigint(node)) {
				sb_putc(b, '"'); sb_puts(b, s); sb_putc(b, '"');
			} else {
				sb_puts(b, s);
			}
			bvn_dom_free_string(s);
		} else {
			sb_puts(b, "null");
		}
		break;
	}
	case BVN_DOM_FLOAT: {
		double v = 0.0;
		bvn_dom_get_float(node, &v);
		/* JSON has no infinity/NaN; emit null for non-finite, as `bovnar
		 * convert` does. */
		if (isnan(v) || isinf(v)) sb_puts(b, "null");
		else                      sb_printf(b, "%.17g", v);
		break;
	}
	case BVN_DOM_STRING: {
		const char *s = NULL; uint32_t n = 0;
		bvn_dom_get_string(node, &s, &n);
		sb_putc(b, '"');
		if (s) sb_json_str_body(b, s, n);
		sb_putc(b, '"');
		break;
	}
	case BVN_DOM_SYMBOL: {
		const char *s = NULL; uint32_t n = 0;
		bvn_dom_get_symbol(node, &s, &n);
		sb_putc(b, '"');
		if (s) sb_json_str_body(b, s, n);
		sb_putc(b, '"');
		break;
	}
	case BVN_DOM_REFERENCE: {
		const char *s = NULL; uint32_t n = 0;
		bvn_dom_get_reference(node, &s, &n);
		sb_putc(b, '"');
		if (s) sb_json_str_body(b, s, n);
		sb_putc(b, '"');
		break;
	}
	case BVN_DOM_OCTET_STREAM: {
		const uint8_t *s = NULL; uint32_t n = 0;
		bvn_dom_get_octets(node, &s, &n);
		sb_putc(b, '"');
		for (uint32_t i = 0; i < n; i++) {
			sb_putc(b, hexd[s[i] >> 4]);
			sb_putc(b, hexd[s[i] & 0xf]);
		}
		sb_putc(b, '"');
		break;
	}
	case BVN_DOM_STRUCT:
		json_emit_entries(b, bvn_dom_struct_entries(node),
		                  bvn_dom_struct_count(node));
		break;
	case BVN_DOM_ARRAY: {
		uint32_t cnt  = bvn_dom_array_count(node);
		uint32_t dims = bvn_dom_array_dims(node);
		if (dims > 1 && cnt % dims == 0) {
			/* Multi-row bovnar array -> nested JSON arrays (one inner array per
			 * row) so the dimensionality survives, matching `bovnar convert`. */
			uint32_t row_len = cnt / dims;
			sb_putc(b, '[');
			for (uint32_t r = 0; r < dims; r++) {
				if (r) sb_putc(b, ',');
				sb_putc(b, '[');
				for (uint32_t c = 0; c < row_len; c++) {
					if (c) sb_putc(b, ',');
					json_emit_node(b, bvn_dom_array_at(node, r * row_len + c));
				}
				sb_putc(b, ']');
			}
			sb_putc(b, ']');
		} else {
			sb_putc(b, '[');
			for (uint32_t i = 0; i < cnt; i++) {
				if (i) sb_putc(b, ',');
				json_emit_node(b, bvn_dom_array_at(node, i));
			}
			sb_putc(b, ']');
		}
		break;
	}
	default:
		sb_puts(b, "null");
		break;
	}
}

/* ------------------------------------------------------------------ */
/* shared: emit the {ok,error,error_name,...} preamble fields         */
/* ------------------------------------------------------------------ */
static void emit_status(sb_t *b, error_code_t err)
{
	sb_printf(b, "\"ok\":%s,", err == error_none ? "true" : "false");
	sb_printf(b, "\"error\":%d,", (int)err);
	sb_puts(b, "\"error_name\":");
	sb_json_cstr(b, bvn_error_to_string(err));
}

/* ================================================================== */
/* exported entry points                                              */
/* ================================================================== */

/*
 * The document tier's verdict on a buffer the streaming reader has accepted.
 *
 * Validity has two tiers (spec 12.4/12.5). The reader checks every value as it
 * passes; that an array's elements agree with one another, that the records of
 * an array share one shape and that no scope repeats a key are properties of
 * the whole document, and are checked over the materialised tree. `bovnar
 * validate` runs both. validate() and errors() here ran the first only -- so
 * `.a = 1; .a = 2;` and `.a = [1, "x"];` came back {"ok":true} from the module
 * whose README says it reports what the CLI reports, while toJSON() and doc()
 * in the SAME module refused them.
 */
static error_code_t wasm_document_tier(const char *buf, int len)
{
	bvn_dom_doc_t *doc = bvn_dom_parse(buf, (uint32_t)len);
	error_code_t err = doc ? bvn_dom_doc_get_parse_error(doc)
	                       : error_invalid_argument;
	if (doc) bvn_dom_doc_destroy(doc);
	return err;
}

/* The first node in the subtree that is a datetime written with sub-second
 * digits, or NULL. Mirrors find_fractional_datetime in src/bovnar.c. */
static const bvn_dom_node_t *json_fractional_datetime(const bvn_dom_node_t *node)
{
	if (!node) return NULL;
	switch (bvn_dom_node_type(node)) {
	case BVN_DOM_INT:
		return bvn_dom_get_datetime_fraction(node, NULL) ? node : NULL;
	case BVN_DOM_STRUCT: {
		uint32_t cnt = bvn_dom_struct_count(node);
		const bvn_dom_entry_t *e = bvn_dom_struct_entries(node);
		for (uint32_t i = 0; e && i < cnt; i++) {
			const bvn_dom_node_t *hit = json_fractional_datetime(e[i].value);
			if (hit) return hit;
		}
		return NULL;
	}
	case BVN_DOM_ARRAY: {
		uint32_t cnt = bvn_dom_array_count(node);
		for (uint32_t i = 0; i < cnt; i++) {
			const bvn_dom_node_t *hit =
				json_fractional_datetime(bvn_dom_array_at(node, i));
			if (hit) return hit;
		}
		return NULL;
	}
	default:
		return NULL;
	}
}

WASM_EXPORT
char *bvnr_wasm_to_json(const char *buf, int len)
{
	sb_t b; sb_init(&b);
	if (len < 0) len = 0;
	bvn_dom_doc_t *doc = bvn_dom_parse(buf, (uint32_t)len);
	error_code_t err = doc ? bvn_dom_doc_get_parse_error(doc) : error_invalid_argument;

	/* A datetime written with a fractional second has only its whole-second
	 * carrier to offer JSON, so projecting it CHANGES the value: half past
	 * midnight came out as midnight, with ok:true. `bovnar convert` refuses
	 * that document rather than truncate it, and the README says this
	 * projection matches convert; it refuses too. There is no error code for
	 * "valid, but not expressible here", so it is value_out_of_range with a
	 * `detail` that says what and where. events() still delivers the digits
	 * (`frac`). */
	const char *frac_key = NULL;
	if (doc && err == error_none) {
		const bvn_dom_entry_t *e = bvn_dom_doc_entries(doc);
		uint32_t cnt = bvn_dom_doc_count(doc);
		for (uint32_t i = 0; e && i < cnt && !frac_key; i++)
			if (json_fractional_datetime(e[i].value))
				frac_key = e[i].key ? e[i].key : "";
		if (frac_key) err = error_value_out_of_range;
	}

	sb_putc(&b, '{');
	emit_status(&b, err);
	if (frac_key) {
		sb_puts(&b, ",\"detail\":\"a datetime with sub-second digits cannot "
		            "be represented in JSON without dropping the fraction "
		            "(under top-level key '");
		sb_json_str_body(&b, frac_key, strlen(frac_key));
		sb_puts(&b, "')\"");
	}
	sb_puts(&b, ",\"value\":");
	if (doc && err == error_none)
		json_emit_entries(&b, bvn_dom_doc_entries(doc), bvn_dom_doc_count(doc));
	else
		sb_puts(&b, "null");
	sb_putc(&b, '}');

	if (doc) bvn_dom_doc_destroy(doc);
	return sb_finish(&b);
}

WASM_EXPORT
char *bvnr_wasm_validate(const char *buf, int len)
{
	sb_t b; sb_init(&b);
	if (len < 0) len = 0;

	bvnr_reader_t *r = bvnr_reader_create();
	bvnr_read_flags_t flags;
	memset(&flags, 0, sizeof(flags));
	flags.continue_on_error = false;
	/* Match every other front end: the CLI (src/bovnar.c), the Python bindings
	 * and the DOM builder all raise BOTH of these to 255. Left at the library
	 * default of 64, the browser rejected a document the same bvnr_read()
	 * accepts natively -- and this very binary's DOM path already allows 255, so
	 * the playground disagreed with itself as well as with the CLI. The page
	 * states that the playground runs the reference reader and delivers the same
	 * events; that has to hold at the limits too.
	 *
	 * The array half of that was missed when the struct half was fixed, and it
	 * behaved exactly as described above: 65 nested arrays parsed natively and
	 * raised error_array_nesting_too_high in the browser, while doc()/toJSON()
	 * -- which reach the reader through bvn_dom_parse, where the DOM builder
	 * sets its own 255 -- accepted the same document in the same module. */
	flags.max_struct_nesting = 255;
	flags.max_array_nesting  = 255;

	bool opened = r && bvnr_open_read_mem(r, buf, (uint64_t)len, NULL, 0, &flags);
	if (opened) bvnr_read(r);

	error_code_t err = r ? bvnr_reader_get_error(r)
	                     : error_invalid_argument;
	if (!opened && err == error_none) err = error_invalid_argument;

	/* The second tier, exactly where cmd_validate runs it: only once the
	 * streaming pass is clean. Its errors are properties of the whole document
	 * and have no position -- line, column, byte and offset stay 0 -- so `tier`
	 * says which kind this is; a caller placing a caret must not put one at
	 * 0:0. (See wasm_document_tier.) */
	const char *tier = (err == error_none) ? NULL : "stream";
	if (opened && err == error_none) {
		err = wasm_document_tier(buf, len);
		if (err != error_none) tier = "document";
	}

	sb_putc(&b, '{');
	emit_status(&b, err);
	sb_puts(&b, ",\"tier\":");
	if (tier) sb_json_cstr(&b, tier);
	else      sb_puts(&b, "null");
	if (r) {
		sb_printf(&b, ",\"line\":%llu",
		          (unsigned long long)bvnr_reader_get_error_line(r));
		sb_printf(&b, ",\"column\":%llu",
		          (unsigned long long)bvnr_reader_get_error_column(r));
		sb_printf(&b, ",\"byte\":%u", bvnr_reader_get_error_byte(r));
		sb_printf(&b, ",\"offset\":%llu",
		          (unsigned long long)bvnr_reader_get_error_offset(r));
		sb_printf(&b, ",\"recovery_count\":%llu",
		          (unsigned long long)bvnr_reader_get_recovery_count(r));
		uint16_t maj = 0, min = 0;
		if (bvnr_reader_get_declared_version(r, &maj, &min))
			sb_printf(&b, ",\"declared_version\":\"%u.%u\"", maj, min);
		else
			sb_puts(&b, ",\"declared_version\":null");
	}
	sb_putc(&b, '}');

	if (r) bvnr_reader_destroy(r);
	return sb_finish(&b);
}

/* ------- event stream ------- */
static const char *evt_name(bvnr_event_t e)
{
	switch (e) {
	case ev_stream_start:                          return "stream_start";
	case ev_assignment_start:                      return "assign_start";
	case ev_octet_stream_start:                    return "octet_start";
	case ev_octet_stream_end:                      return "octet_end";
	case ev_struct_start:                          return "struct_open";
	case ev_struct_end:                            return "struct_close";
	case ev_array_row_start:                       return "array_row_start";
	case ev_array_row_end:                         return "array_row_end";
	case ev_array_dim_start:                       return "array_dim_start";
	case ev_data:                                  return "data";
	case ev_type_annotation_start:                 return "type_start";
	case ev_type_annotation_end:                   return "type_end";
	case ev_type_annotation_type_family:           return "type_family";
	case ev_type_annotation_type_family_parameter: return "type_param";
	case ev_stream_end:                            return "stream_end";
	}
	return "?";
}

static const char *evt_tok(token_type_t t)
{
	switch (t) {
	case token_is_identifier:   return "identifier";
	case token_is_string:       return "string";
	case token_is_number:       return "number";
	case token_is_symbol:       return "symbol";
	case token_is_reference:    return "reference";
	case token_is_array_number: return "array_number";
	case token_is_array_string: return "array_string";
	case token_is_type:         return "type";
	case token_is_octet_stream: return "octet_stream";
	case token_is_null_value:   return "null";
	case token_is_structure:    return "structure";
	case token_is_unit:         return "unit";
	case token_is_type_width:   return "type_width";
	case token_is_type_base:    return "type_base";
	case token_is_type_q:       return "type_q";
	case token_is_bool:         return "bool";
	case token_is_unknown:      return "unknown";
	}
	return "?";
}

static const char *evt_family(value_type_family_t f)
{
	switch (f) {
	case vt_plain:     return "plain";
	case vt_utf8:      return "utf8";
	case vt_sint:      return "sint";
	case vt_uint:      return "uint";
	case vt_float:     return "float";
	case vt_float_fix: return "float_fix";
	case vt_float_dec: return "float_dec";
	case vt_bool:      return "bool";
	case vt_datetime:  return "datetime";
	case vt_illegal:   return "illegal";
	}
	return "?";
}

typedef struct {
	sb_t    *b;
	uint64_t seq;
	int      count;
} evt_ctx_t;

/*
 * on_verified and want_unit share the single `userdata` slot, so they share one
 * context: `ev` for the event serialiser, `req` for the conversion target. A
 * NULL `req.unit` means "keep each value's own unit", which together with a base
 * makes the request a pure base conversion.
 */
typedef struct {
	evt_ctx_t           ev;
	const value_unit_t *unit;
	uint32_t            base;
	/* Recovered errors. events_impl runs in resync mode, and a document that
	 * resyncs and then reaches EOF cleanly has the reader's final code CLEARED
	 * -- so deriving `ok` from that code alone reported success for a document
	 * whose failing assignment had been skipped. bvnr_wasm_parse hit exactly
	 * this and computes `ok` from an error COUNT instead; the comment there
	 * spells out why. This is the same fix for the events path, which is where
	 * it bites hardest: eventsConvert('.v = <float:64,k~m/h> 100.0;', 'm/s')
	 * reported {"ok":true} with the data event simply ABSENT, because 250/9
	 * does not terminate and the value was dropped. A well-formed stream
	 * carrying a key and no value, and nothing saying so. */
	uint32_t            err_count;
	error_code_t        first_err;
} events_ud_t;

static void events_err_cb(void *ud, error_code_t err,
                          uint64_t line, uint64_t column,
                          uint32_t byte, uint64_t offset)
{
	events_ud_t *u = (events_ud_t *)ud;
	(void)line; (void)column; (void)byte; (void)offset;
	if (u->err_count == 0u)
		u->first_err = err;
	u->err_count++;
}


static bool evt_cb(void *ud, bvnr_event_t e, bvnr_data_t *d)
{
	evt_ctx_t *ctx = &((events_ud_t *)ud)->ev;
	sb_t *b = ctx->b;
	if (ctx->count++) sb_putc(b, ',');
	sb_putc(b, '{');
	sb_printf(b, "\"seq\":%llu", (unsigned long long)ctx->seq++);
	sb_puts(b, ",\"ev\":");
	sb_json_cstr(b, evt_name(e));
	sb_puts(b, ",\"tok\":");
	sb_json_cstr(b, evt_tok(d->type));

	/* textual payload (skip raw octet bytes; report their length instead) */
	if (d->type == token_is_octet_stream) {
		sb_printf(b, ",\"bytes\":%u", d->length);
	} else if (d->data && d->length) {
		sb_puts(b, ",\"text\":\"");
		sb_json_str_body(b, (const char *)d->data, d->length);
		sb_putc(b, '"');
	}

	/* type/unit metadata, where meaningful */
	if (d->value_type.family != vt_plain &&
	    d->type != token_is_type_width &&
	    d->type != token_is_type_base &&
	    d->type != token_is_unit) {
		sb_puts(b, ",\"family\":");
		sb_json_cstr(b, evt_family(d->value_type.family));
		sb_printf(b, ",\"width\":%u", d->value_type.width);
		sb_printf(b, ",\"base\":%u", d->value_type.base);
		char unit_buf[BVNR_UNIT_STRING_MAX];
		if (bvn_unit_to_string(d->value_unit, unit_buf, sizeof(unit_buf)) >= 0 &&
		    unit_buf[0] && strcmp(unit_buf, "no_unit") != 0) {
			sb_puts(b, ",\"unit\":");
			sb_json_cstr(b, unit_buf);
		}
		if (d->value_type.family == vt_datetime) {
			const char *ep = bvnr_datetime_epoch_name(d->value_type);
			if (ep) { sb_puts(b, ",\"epoch\":"); sb_json_cstr(b, ep); }
		}
	}
	/* numeric value of a type-annotation width/base parameter (carried in
	 * value_type, not in d->data) so the shim can rebuild the annotation. */
	if (d->type == token_is_type_width)
		sb_printf(b, ",\"width\":%u", d->value_type.width);
	else if (d->type == token_is_type_base)
		sb_printf(b, ",\"base\":%u", d->value_type.base);
	if (d->frac_data && d->frac_length) {
		sb_puts(b, ",\"frac\":\"");
		sb_json_str_body(b, (const char *)d->frac_data, d->frac_length);
		sb_putc(b, '"');
	}
	/* library 2.0 — the unit as the document spelled it, beside the canonical
	 * "unit" above: "ucum:mL{total}" where "unit" says "m~L". */
	if (d->unit_source && d->unit_source_length) {
		sb_puts(b, ",\"unit_source\":\"");
		sb_json_str_body(b, (const char *)d->unit_source,
				 d->unit_source_length);
		sb_putc(b, '"');
	}
	/* lossless read-time unit/base conversion (bvnr_read_flags_t.want_unit).
	 * conv.text is NULL when the exact result has no terminating expansion in
	 * the output base (want_unit_allow_nonterminating): report the conversion
	 * with a null value rather than dropping the event's conversion fields. */
	if (d->converted) {
		if (d->conv.text) {
			sb_puts(b, ",\"converted\":\"");
			sb_json_str_body(b, d->conv.text, d->conv.length);
			sb_putc(b, '"');
		} else {
			sb_puts(b, ",\"converted\":null");
		}
		sb_printf(b, ",\"converted_base\":%u", d->conv.base);
		char ub[BVNR_UNIT_STRING_MAX];
		if (bvn_unit_to_string(d->conv.unit, ub, sizeof ub) >= 0 && ub[0]) {
			sb_puts(b, ",\"converted_unit\":");
			sb_json_cstr(b, ub);
		}
	}
	sb_putc(b, '}');
	return true;
}

static bool conv_want_unit(void *ud, const bvnr_data_t *d,
			   value_unit_t *want, uint32_t *want_base)
{
	const events_ud_t *u = (const events_ud_t *)ud;
	*want      = u->unit ? *u->unit : d->value_unit;
	*want_base = u->base;
	return true;
}

static char *events_impl(const char *buf, int len,
			 const value_unit_t *target, uint32_t base,
			 bool convert, bool allow_nonterminating)
{
	sb_t b; sb_init(&b);
	if (len < 0) len = 0;

	sb_t evbuf; sb_init(&evbuf);
	events_ud_t ud = { { &evbuf, 0, 0 }, target, base, 0u, error_none };

	bvnr_reader_t *r = bvnr_reader_create();
	bvnr_read_flags_t flags;
	memset(&flags, 0, sizeof(flags));
	flags.max_struct_nesting = 255;   /* see bvnr_wasm_validate */
	flags.max_array_nesting  = 255;
	flags.userdata = &ud;
	flags.on_verified = evt_cb;
	flags.on_error = events_err_cb;
	/* resync: emit verified events for every well-formed assignment, skipping
	 * (not aborting at) a broken one — so the playground tree and the demos'
	 * resync mode show all recoverable structure past an error. */
	flags.continue_on_error = true;
	if (convert) {
		flags.want_unit = conv_want_unit;
		flags.want_unit_allow_nonterminating = allow_nonterminating;
	}

	bool opened = r && bvnr_open_read_mem(r, buf, (uint64_t)len, NULL, 0, &flags);
	if (opened) bvnr_read(r);

	error_code_t err = r ? bvnr_reader_get_error(r) : error_invalid_argument;
	if (!opened && err == error_none) err = error_invalid_argument;

	/* `ok` from the error COUNT, not the final code -- see events_ud_t. The
	 * reported code is the FIRST error whenever on_error saw one, so a caller
	 * learns what went wrong first rather than reading "none" beside a missing
	 * value.
	 *
	 * Whenever, not only "when the final code has been cleared": that was the
	 * condition, and it reported the LAST error for any document whose final
	 * one was fatal. A string holding a bad byte and never closed came back as
	 * incomplete_bvnr_stream here while errors()[0] and validate() both said
	 * invalid_utf8_byte -- three surfaces documented to name the same, first,
	 * error. */
	if (ud.err_count > 0u)
		err = ud.first_err;
	sb_putc(&b, '{');
	sb_printf(&b, "\"ok\":%s,",
	          (err == error_none && ud.err_count == 0u) ? "true" : "false");
	sb_printf(&b, "\"error\":%d,", (int)err);
	sb_puts(&b, "\"error_name\":");
	sb_json_cstr(&b, bvn_error_to_string(err));
	sb_puts(&b, ",\"events\":[");
	sb_splice(&b, &evbuf);
	sb_puts(&b, "]}");

	if (r) bvnr_reader_destroy(r);
	return sb_finish(&b);
}

WASM_EXPORT
char *bvnr_wasm_events(const char *buf, int len)
{
	return events_impl(buf, len, NULL, 0u, false, false);
}

/*
 * Same event stream, but with the reader's lossless read-time unit/base
 * conversion armed — the only way to reach the `converted` / `converted_base` /
 * `converted_unit` fields evt_cb emits.
 *
 * `unit` is a NUL-terminated unit string ("m", "k~m/s", ""/NULL to keep each
 * value's own unit — which with a `base` is a pure base conversion). `base` is
 * the output base (0 keeps the value's own; otherwise 2..62, 64 or 85).
 * `allow_nonterminating` non-zero delivers a result that is exact as a rational
 * but has no finite expansion in `base` with `"converted":null` instead of
 * failing the parse.
 *
 * An unparseable `unit` is reported as error_unit_illegal rather than silently
 * falling back to no conversion.
 *
 * Note the interaction with this surface's resync policy: because
 * continue_on_error is on, a value the conversion rejects — dimensionally
 * incompatible with `unit`, or inexact — is skipped along with its assignment
 * rather than aborting, so it does not appear in `events`. The status DOES say
 * so: `ok` is false and `error`/`error_name` carry the rejection
 * (unit_mismatch, unit_inexact), because events_impl counts recovered errors
 * rather than reading the reader's cleared final code. Reporting `ok` over a
 * value it had dropped is what that count replaced.
 */
WASM_EXPORT
char *bvnr_wasm_events_convert(const char *buf, int len, const char *unit,
			       int base, int allow_nonterminating)
{
	value_unit_t        target;
	const value_unit_t *tp = NULL;

	/* 0 keeps each value's own base; a negative one is no base at all, and
	 * used to be read as 0 -- a silent "no conversion" reported as ok. */
	if (base < 0) {
		sb_t b; sb_init(&b);
		sb_putc(&b, '{');
		emit_status(&b, error_invalid_argument);
		sb_puts(&b, ",\"events\":[]}");
		return sb_finish(&b);
	}
	if (unit && unit[0]) {
		bool ok = false;
		target = bvn_parse_unit((const uint8_t *)unit, &ok);
		if (!ok) {
			sb_t b; sb_init(&b);
			sb_putc(&b, '{');
			emit_status(&b, error_unit_illegal);
			sb_puts(&b, ",\"events\":[]}");
			return sb_finish(&b);
		}
		tp = &target;
	}
	return events_impl(buf, len, tp, (uint32_t)base,
			   true, allow_nonterminating != 0);
}

/* ------- all errors (resync mode) ------- */
typedef struct {
	sb_t        *b;
	int          count;
	/* First recovered code, for the same reason events_ud_t carries one: under
	 * continue_on_error a document that resyncs and then reaches EOF cleanly
	 * has the reader's final code CLEARED, so a surface reporting that code
	 * alone says "none" beside its own non-empty errors array. */
	error_code_t first;
} err_ctx_t;

static void err_collect(void *ud, error_code_t err,
                        uint64_t line, uint64_t column,
                        uint32_t byte, uint64_t offset)
{
	err_ctx_t *ctx = (err_ctx_t *)ud;
	sb_t *b = ctx->b;
	if (ctx->count == 0) ctx->first = err;
	if (ctx->count++) sb_putc(b, ',');
	sb_putc(b, '{');
	sb_printf(b, "\"error\":%d,", (int)err);
	sb_puts(b, "\"error_name\":");
	sb_json_cstr(b, bvn_error_to_string(err));
	sb_printf(b, ",\"line\":%llu", (unsigned long long)line);
	sb_printf(b, ",\"column\":%llu", (unsigned long long)column);
	sb_printf(b, ",\"byte\":%u", byte);
	sb_printf(b, ",\"offset\":%llu", (unsigned long long)offset);
	sb_puts(b, ",\"tier\":\"stream\"");
	sb_putc(b, '}');
}

/* Validate in resync mode, collecting EVERY error (not just the first) the way
 * the playground's on_error panel expects. Returns
 * { ok, errors:[{error,error_name,line,column,byte,offset,tier}],
 *   declared_version }. `tier` is "stream" for an error the reader raised at a
 * position, "document" for a whole-document rule (no position; see below). */
WASM_EXPORT
char *bvnr_wasm_errors(const char *buf, int len)
{
	sb_t b; sb_init(&b);
	if (len < 0) len = 0;

	sb_t ebuf; sb_init(&ebuf);
	err_ctx_t ctx = { &ebuf, 0, error_none };

	bvnr_reader_t *r = bvnr_reader_create();
	bvnr_read_flags_t flags;
	memset(&flags, 0, sizeof(flags));
	flags.max_struct_nesting = 255;   /* see bvnr_wasm_validate */
	flags.max_array_nesting  = 255;
	flags.userdata          = &ctx;
	flags.on_error          = err_collect;
	flags.continue_on_error = true;   /* resync: report all errors, keep going */

	bool opened = r && bvnr_open_read_mem(r, buf, (uint64_t)len, NULL, 0, &flags);
	if (opened) bvnr_read(r);

	/* "EVERY error" includes the one the streaming pass cannot see. When that
	 * pass found nothing, ask the document tier, and report what it says as
	 * one more entry -- positionless, and marked as such. (Only then: the DOM
	 * does not resync, so over a document with stream errors it could only
	 * repeat the first of them.) Without this, errors() said {"ok":true,
	 * "errors":[]} for a duplicate key. */
	if (opened && ctx.count == 0) {
		error_code_t derr = wasm_document_tier(buf, len);
		if (derr != error_none) {
			sb_printf(&ebuf, "{\"error\":%d,\"error_name\":", (int)derr);
			sb_json_cstr(&ebuf, bvn_error_to_string(derr));
			sb_puts(&ebuf, ",\"line\":0,\"column\":0,\"byte\":0,"
			               "\"offset\":0,\"tier\":\"document\"}");
			ctx.count++;
		}
	}

	sb_putc(&b, '{');
	sb_printf(&b, "\"ok\":%s", ctx.count == 0 ? "true" : "false");
	sb_puts(&b, ",\"errors\":[");
	sb_splice(&b, &ebuf);
	sb_puts(&b, "]");
	if (r) {
		uint16_t maj = 0, min = 0;
		if (bvnr_reader_get_declared_version(r, &maj, &min))
			sb_printf(&b, ",\"declared_version\":\"%u.%u\"", maj, min);
		else
			sb_puts(&b, ",\"declared_version\":null");
	}
	sb_putc(&b, '}');

	if (r) bvnr_reader_destroy(r);
	return sb_finish(&b);
}

/* ------- combined single-pass parse (events + errors) ------- */
/*
 * The playground needs BOTH the verified event stream and the full error list
 * for the same document. Doing that with bvnr_wasm_events + bvnr_wasm_errors
 * meant constructing two readers, walking the document twice, and serialising
 * then re-parsing two JSON payloads in JS -- for a 1000-assignment document the
 * event payload alone is hundreds of KB, on every debounce tick.
 *
 * Both of those surfaces already run with continue_on_error = true, so a single
 * reader carrying both callbacks produces exactly the same two streams in one
 * pass. The only thing in the way was bvnr_read_flags_t::userdata being a single
 * pointer; the shim struct below lets both callbacks keep their own context.
 *
 * Returns { ok, error, error_name, events:[...], errors:[...], declared_version }
 * -- the union of what the two separate exports return. They are kept for
 * compatibility (and because errors alone is genuinely cheaper when the caller
 * only validates).
 */
typedef struct {
	events_ud_t ev;
	err_ctx_t   er;
} both_ud_t;

static bool both_evt_cb(void *ud, bvnr_event_t e, bvnr_data_t *d)
{
	return evt_cb(&((both_ud_t *)ud)->ev, e, d);
}

static void both_err_cb(void *ud, error_code_t err,
                        uint64_t line, uint64_t column,
                        uint32_t byte, uint64_t offset)
{
	err_collect(&((both_ud_t *)ud)->er, err, line, column, byte, offset);
}

WASM_EXPORT
char *bvnr_wasm_parse(const char *buf, int len)
{
	sb_t b; sb_init(&b);
	if (len < 0) len = 0;

	sb_t evbuf; sb_init(&evbuf);
	sb_t ebuf;  sb_init(&ebuf);
	both_ud_t ud;
	memset(&ud, 0, sizeof(ud));
	ud.ev.ev.b = &evbuf;
	ud.ev.unit = NULL;
	ud.ev.base = 0u;
	ud.er.b     = &ebuf;
	ud.er.count = 0;
	ud.er.first = error_none;

	bvnr_reader_t *r = bvnr_reader_create();
	bvnr_read_flags_t flags;
	memset(&flags, 0, sizeof(flags));
	flags.max_struct_nesting = 255;   /* see bvnr_wasm_validate */
	flags.max_array_nesting  = 255;
	flags.userdata          = &ud;
	flags.on_verified       = both_evt_cb;
	flags.on_error          = both_err_cb;
	flags.continue_on_error = true;   /* resync: all events AND all errors */

	bool opened = r && bvnr_open_read_mem(r, buf, (uint64_t)len, NULL, 0, &flags);
	if (opened) bvnr_read(r);

	error_code_t err = r ? bvnr_reader_get_error(r) : error_invalid_argument;
	if (!opened && err == error_none) err = error_invalid_argument;

	/* `ok` here means "nothing went wrong", which for a payload that also
	 * carries an errors array must account for RECOVERED errors. emit_status
	 * derives ok from the reader's final error code, and under
	 * continue_on_error a document that resyncs and then reaches EOF cleanly
	 * has that code cleared (bovnar_lexer.c clears last_error on the clean-EOF
	 * break; tests/bovnar_reader_test.c asserts it). Using emit_status verbatim
	 * therefore produced {"ok":true, ..., "errors":[...3 entries...]} -- self-
	 * contradictory, and disagreeing with bvnr_wasm_errors, which computes ok
	 * from its error COUNT. Match that instead.
	 *
	 * "error"/"error_name" need the same treatment, and getting only the `ok`
	 * half left this export saying {"ok":false,"error":0,"error_name":"none"}
	 * -- a caller doing `if (!r.ok) show(r.error_name)` was told "none" went
	 * wrong. events_impl reports the FIRST recovered code when the final one
	 * has been cleared; this is the export index.mjs tells callers to PREFER
	 * over events()+errors(), and the one the playground shim actually uses, so
	 * it disagreed with the surface it replaced on the same document: events()
	 * said unexpected_input_byte, parse() said none. Same fallback here. */
	/* ...and, as there, the first error whenever there was one -- not only
	 * when the final code happens to have been cleared. */
	if (ud.er.count > 0)
		err = ud.er.first;
	sb_putc(&b, '{');
	sb_printf(&b, "\"ok\":%s,",
	          (err == error_none && ud.er.count == 0) ? "true" : "false");
	sb_printf(&b, "\"error\":%d,", (int)err);
	sb_puts(&b, "\"error_name\":");
	sb_json_cstr(&b, bvn_error_to_string(err));
	sb_puts(&b, ",\"events\":[");
	sb_splice(&b, &evbuf);
	sb_puts(&b, "],\"errors\":[");
	sb_splice(&b, &ebuf);
	sb_putc(&b, ']');
	if (r) {
		uint16_t maj = 0, min = 0;
		if (bvnr_reader_get_declared_version(r, &maj, &min))
			sb_printf(&b, ",\"declared_version\":\"%u.%u\"", maj, min);
		else
			sb_puts(&b, ",\"declared_version\":null");
	}
	sb_putc(&b, '}');

	if (r) bvnr_reader_destroy(r);
	return sb_finish(&b);
}

/* ================================================================== */
/* rich document tree (drives the playground's WASM-backed parser shim)*/
/* ================================================================== */
/* Shortest round-tripping %g representation of a finite double (so the tree
 * shows "9.81", not "9.8100000000000005"). */
static void wdoc_float_text(sb_t *b, double v)
{
	char tmp[64];
	for (int p = 1; p <= 17; p++) {
		snprintf(tmp, sizeof tmp, "%.*g", p, v);
		if (strtod(tmp, NULL) == v) break;
	}
	sb_json_str_body(b, tmp, strlen(tmp));
}
static void wdoc_float_number(sb_t *b, double v)
{
	char tmp[64];
	for (int p = 1; p <= 17; p++) {
		snprintf(tmp, sizeof tmp, "%.*g", p, v);
		if (strtod(tmp, NULL) == v) break;
	}
	sb_puts(b, tmp);
}

/* Resolved type fields shared by ann + valueType on the JS side:
 * familyName, width, base (numeric only), unit (string|null), epoch (dt|null). */
static void wdoc_type_fields(sb_t *b, const bvn_dom_node_t *node)
{
	value_type_spec_t vt = bvn_dom_get_value_type(node);
	sb_puts(b, ",\"familyName\":");
	sb_json_cstr(b, evt_family(vt.family));
	if (bvn_type_is_numeric(vt)) {
		sb_printf(b, ",\"width\":%u", bvn_effective_width(vt));
		sb_printf(b, ",\"base\":%u", bvn_effective_base(vt));
	} else {
		sb_puts(b, ",\"width\":null,\"base\":null");
	}
	char ub[BVNR_UNIT_STRING_MAX];
	int32_t ul = bvn_dom_get_unit_string(node, ub, sizeof ub);
	if (ul > 0 && strcmp(ub, "no_unit") != 0) {
		sb_puts(b, ",\"unit\":");
		sb_json_cstr(b, ub);
	} else {
		sb_puts(b, ",\"unit\":null");
	}
	if (vt.family == vt_datetime) {
		const char *ep = bvnr_datetime_epoch_name(vt);
		if (ep) { sb_puts(b, ",\"epoch\":"); sb_json_cstr(b, ep); }
		else      sb_puts(b, ",\"epoch\":null");
	} else {
		sb_puts(b, ",\"epoch\":null");
	}
}

static void wdoc_emit_value(sb_t *b, const bvn_dom_node_t *node);

static void wdoc_emit_entry(sb_t *b, const char *key, const bvn_dom_node_t *node)
{
	sb_puts(b, "{\"key\":");
	sb_json_cstr(b, key);
	sb_puts(b, ",\"value\":");
	wdoc_emit_value(b, node);
	sb_putc(b, '}');
}

static void wdoc_emit_value(sb_t *b, const bvn_dom_node_t *node)
{
	if (!node) { sb_puts(b, "{\"type\":\"null\",\"text\":\"\"}"); return; }
	switch (bvn_dom_node_type(node)) {
	case BVN_DOM_NULL:
		sb_puts(b, "{\"type\":\"null\",\"text\":\"\"");
		wdoc_type_fields(b, node);
		sb_putc(b, '}');
		break;
	case BVN_DOM_BOOL: {
		bool v = false; bvn_dom_get_bool(node, &v);
		sb_puts(b, "{\"type\":\"scalar\",\"kind\":\"bool\",\"text\":");
		sb_json_cstr(b, v ? "true" : "false");
		sb_printf(b, ",\"value\":%s", v ? "true" : "false");
		wdoc_type_fields(b, node);
		sb_putc(b, '}');
		break;
	}
	case BVN_DOM_INT: {
		char *s = bvn_dom_int_to_str(node, 10);
		bool big = bvn_dom_get_bigint(node) != NULL;
		value_type_spec_t vt = bvn_dom_get_value_type(node);
		sb_puts(b, "{\"type\":\"scalar\",\"kind\":");
		sb_json_cstr(b, vt.family == vt_datetime ? "datetime" : "number");
		sb_puts(b, ",\"text\":");
		sb_json_cstr(b, s ? s : "0");
		sb_puts(b, ",\"value\":");
		if (!s)        sb_puts(b, "null");
		else if (big) { sb_putc(b, '"'); sb_puts(b, s); sb_putc(b, '"'); }
		else           sb_puts(b, s);
		if (s) bvn_dom_free_string(s);
		wdoc_type_fields(b, node);
		sb_putc(b, '}');
		break;
	}
	case BVN_DOM_FLOAT: {
		double v = 0.0; bvn_dom_get_float(node, &v);
		value_type_spec_t vt = bvn_dom_get_value_type(node);
		bool fin = !(isnan(v) || isinf(v));
		sb_puts(b, "{\"type\":\"scalar\",\"kind\":");
		sb_json_cstr(b, vt.family == vt_datetime ? "datetime"
		                : (fin ? "number" : "special"));
		sb_puts(b, ",\"text\":\"");
		if (isnan(v))      sb_puts(b, "nan");
		else if (isinf(v)) sb_puts(b, v < 0 ? "-inf" : "inf");
		else               wdoc_float_text(b, v);
		sb_puts(b, "\",\"value\":");
		if (fin) wdoc_float_number(b, v);
		else     sb_puts(b, "null");
		wdoc_type_fields(b, node);
		sb_putc(b, '}');
		break;
	}
	case BVN_DOM_STRING: {
		const char *s = NULL; uint32_t n = 0; bvn_dom_get_string(node, &s, &n);
		sb_puts(b, "{\"type\":\"scalar\",\"kind\":\"string\",\"text\":\"");
		if (s) sb_json_str_body(b, s, n);
		sb_puts(b, "\",\"value\":\"");
		if (s) sb_json_str_body(b, s, n);
		sb_putc(b, '"');
		wdoc_type_fields(b, node);
		sb_putc(b, '}');
		break;
	}
	case BVN_DOM_SYMBOL: {
		const char *s = NULL; uint32_t n = 0; bvn_dom_get_symbol(node, &s, &n);
		sb_puts(b, "{\"type\":\"scalar\",\"kind\":\"symbol\",\"text\":\"");
		if (s) sb_json_str_body(b, s, n);
		sb_puts(b, "\",\"value\":\"");
		if (s) sb_json_str_body(b, s, n);
		sb_putc(b, '"');
		wdoc_type_fields(b, node);
		sb_putc(b, '}');
		break;
	}
	case BVN_DOM_REFERENCE: {
		const char *s = NULL; uint32_t n = 0; bvn_dom_get_reference(node, &s, &n);
		sb_puts(b, "{\"type\":\"reference\",\"kind\":\"reference\",\"text\":\"&");
		if (s) sb_json_str_body(b, s, n);
		sb_puts(b, "\",\"value\":\"");
		if (s) sb_json_str_body(b, s, n);
		sb_putc(b, '"');
		wdoc_type_fields(b, node);
		sb_putc(b, '}');
		break;
	}
	case BVN_DOM_OCTET_STREAM: {
		const uint8_t *s = NULL; uint32_t n = 0; bvn_dom_get_octets(node, &s, &n);
		sb_printf(b, "{\"type\":\"octet\",\"total\":%u,\"chunks\":[{\"length\":%u}]}",
		          n, n);
		break;
	}
	case BVN_DOM_STRUCT: {
		sb_puts(b, "{\"type\":\"struct\",\"children\":[");
		const bvn_dom_entry_t *e = bvn_dom_struct_entries(node);
		uint32_t cnt = bvn_dom_struct_count(node);
		for (uint32_t i = 0; i < cnt; i++) {
			if (i) sb_putc(b, ',');
			wdoc_emit_entry(b, e[i].key, e[i].value);
		}
		sb_puts(b, "]}");
		break;
	}
	case BVN_DOM_ARRAY: {
		uint32_t cnt = bvn_dom_array_count(node);
		uint32_t dims = bvn_dom_array_dims(node);
		sb_printf(b, "{\"type\":\"array\",\"dims\":%u,\"rows\":[", dims ? dims : 1);
		if (dims > 1 && cnt % dims == 0) {
			uint32_t rl = cnt / dims;
			for (uint32_t r = 0; r < dims; r++) {
				if (r) sb_putc(b, ',');
				sb_puts(b, "{\"elements\":[");
				for (uint32_t c = 0; c < rl; c++) {
					if (c) sb_putc(b, ',');
					wdoc_emit_value(b, bvn_dom_array_at(node, r * rl + c));
				}
				sb_puts(b, "]}");
			}
		} else {
			sb_puts(b, "{\"elements\":[");
			for (uint32_t i = 0; i < cnt; i++) {
				if (i) sb_putc(b, ',');
				wdoc_emit_value(b, bvn_dom_array_at(node, i));
			}
			sb_puts(b, "]}");
		}
		sb_puts(b, "]");
		wdoc_type_fields(b, node);
		sb_putc(b, '}');
		break;
	}
	default:
		sb_puts(b, "{\"type\":\"null\",\"text\":\"\"}");
		break;
	}
}

/* Full document tree in document order, each value carrying its resolved type
 * fields. The shim turns this into parseFaithful's tree and replays it as
 * parseBovnar's event stream. Returns
 * { ok, error, error_name, tree:[{key,value}...] }. */
WASM_EXPORT
char *bvnr_wasm_doc(const char *buf, int len)
{
	sb_t b; sb_init(&b);
	if (len < 0) len = 0;
	bvn_dom_doc_t *doc = bvn_dom_parse(buf, (uint32_t)len);
	error_code_t err = doc ? bvn_dom_doc_get_parse_error(doc)
	                       : error_invalid_argument;
	/* The same {ok, error, error_name} preamble as every other result. It was
	 * a bare "ok": a caller told false had no way to learn why, short of
	 * parsing the document again through another entry point. */
	sb_putc(&b, '{');
	emit_status(&b, err);
	sb_puts(&b, ",\"tree\":[");
	if (doc) {
		const bvn_dom_entry_t *e = bvn_dom_doc_entries(doc);
		uint32_t cnt = bvn_dom_doc_count(doc);
		for (uint32_t i = 0; i < cnt; i++) {
			if (i) sb_putc(&b, ',');
			wdoc_emit_entry(&b, e[i].key, e[i].value);
		}
	}
	sb_puts(&b, "]}");
	if (doc) bvn_dom_doc_destroy(doc);
	return sb_finish(&b);
}

WASM_EXPORT
const char *bvnr_wasm_version(void)
{
	return bvnr_version_string();
}

WASM_EXPORT
void bvnr_wasm_free(char *p)
{
	free(p);
}
