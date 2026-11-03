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

#define _POSIX_C_SOURCE 200809L
#include <ctype.h>
#include <errno.h>
#include <inttypes.h>
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define BVN_PORT_TIMING   /* pull in bvn_monotonic_sec / bvn_cpu_sec for cmd_bench */
#include "bvn_port.h"
#include "bovnar.h"
#include "bovnar_dom.h"

/*
 * The shortest decimal that reads back as the same double, in plain notation
 * where the value has one.
 *
 * Widening "%.*g" until it round-trips gives full precision, but %g decides the
 * NOTATION by its own rule -- scientific once the decimal exponent reaches the
 * precision -- so the notation fell out of how few digits the value happened to
 * need. 120.0 round-trips at precision 2 and therefore printed as "1.2e+02",
 * while 123456789.12345679 needed all 17 and printed plainly: one command
 * rendered comparable numbers two different ways, and the short round ones came
 * out worst. Precision and notation are separate questions, and more precision
 * never breaks a round trip -- it only moves %g's threshold -- so once the
 * precision is known the notation can be chosen deliberately.
 *
 * Only positive exponents are widened. Below 1e-4, %g's other threshold, there
 * is no short plain form to prefer: 1e-05 would become "0.00001" and 1e-300 an
 * absurdity, and scientific is the conventional rendering there -- Python's
 * repr and every shortest-round-trip printer agree. The upper bound of 17 is
 * where a plain integer form stops being shorter than the scientific one.
 *
 * NaN and the infinities carry no exponent and fall out at the first check.
 */
static void bvn_cli_format_double(char *buf, size_t cap, double v)
{
	int prec = 1;
	for (; prec <= 17; prec++) {
		snprintf(buf, cap, "%.*g", prec, v);
		if (strtod(buf, NULL) == v)
			break;
	}
	const char *e = strchr(buf, 'e');
	if (!e)
		return;
	int e10 = atoi(e + 1);
	if (e10 < 0 || e10 >= 17)
		return;
	int wide = (e10 + 1 > prec) ? (e10 + 1) : prec;
	char alt[64];
	snprintf(alt, sizeof alt, "%.*g", wide, v);
	size_t n = strlen(alt);
	if (strtod(alt, NULL) == v && strchr(alt, 'e') == NULL && n < cap)
		memcpy(buf, alt, n + 1);
}
#include "bovnar_stream.h"
#include "bovnar_si_units.h"
#include "bvn_datetime.h"
#include <stdarg.h>

/*
 * Console-aware UTF-8 output for the human-readable CLI display (the events
 * table, the bench tables, and the box-drawing rules / status glyphs).
 *
 * On Windows the CRT's path to a console mangles UTF-8 even with the console
 * output code page pinned to UTF-8: the bytes reach the console via WriteFile,
 * and that decode is unreliable (Wine renders each multi-byte rune as U+FFFD
 * replacement characters). The robust route is the native wide API: convert the
 * UTF-8 to UTF-16 and call WriteConsoleW. When the stream is redirected to a
 * file or a pipe (GetConsoleMode fails), or on POSIX, we emit the raw UTF-8
 * bytes unchanged — so piped/redirected output and the data subcommands stay
 * byte-exact. These helpers are used only by the decorative/diagnostic display;
 * value/data output keeps using stdio directly.
 */
/* Use the gnu_printf archetype on MinGW so the checker accepts %zu / %lld /
 * PRIu64 — the same ANSI stdio the build links via __USE_MINGW_ANSI_STDIO. The
 * default ms_printf archetype would reject them. */
#if defined(__MINGW32__) || defined(__MINGW64__)
#  define BVN_CLI_FMT(a, b) __attribute__((format(gnu_printf, a, b)))
#elif defined(__GNUC__) || defined(__clang__)
#  define BVN_CLI_FMT(a, b) __attribute__((format(printf, a, b)))
#else
#  define BVN_CLI_FMT(a, b)
#endif
static void cw_vprint(FILE *f, const char *fmt, va_list ap) BVN_CLI_FMT(2, 0);
static void cwf(const char *fmt, ...) BVN_CLI_FMT(1, 2);
static void cwe(const char *fmt, ...) BVN_CLI_FMT(1, 2);
static void cw_write(FILE *f, const char *s, size_t n)
{
#if defined(_WIN32)
	HANDLE h = (f == stderr) ? GetStdHandle(STD_ERROR_HANDLE)
							 : GetStdHandle(STD_OUTPUT_HANDLE);
	DWORD cmode;
	if (h && h != INVALID_HANDLE_VALUE && GetConsoleMode(h, &cmode) && n > 0) {
		/* Flush any buffered stdio first so console writes stay in order. */
		fflush(f);
		int wn = MultiByteToWideChar(CP_UTF8, 0, s, (int)n, NULL, 0);
		if (wn > 0) {
			wchar_t *w = (wchar_t *)malloc((size_t)wn * sizeof(wchar_t));
			if (w) {
				MultiByteToWideChar(CP_UTF8, 0, s, (int)n, w, wn);
				DWORD off = 0;
				while (off < (DWORD)wn) {
					DWORD wrote = 0;
					if (!WriteConsoleW(h, w + off, (DWORD)wn - off,
									   &wrote, NULL) || wrote == 0)
						break;
					off += wrote;
				}
				free(w);
				return;
			}
		}
		/* conversion/alloc failure: fall through to raw bytes */
	}
#endif
	fwrite(s, 1, n, f);
}
static void cw_vprint(FILE *f, const char *fmt, va_list ap)
{
	char stackbuf[1024];
	va_list ap2;
	va_copy(ap2, ap);
	int need = vsnprintf(stackbuf, sizeof(stackbuf), fmt, ap2);
	va_end(ap2);
	if (need < 0)
		return;
	if ((size_t)need < sizeof(stackbuf)) {
		cw_write(f, stackbuf, (size_t)need);
		return;
	}
	char *heap = (char *)malloc((size_t)need + 1u);
	if (!heap) {                       /* emit the truncated prefix rather than nothing */
		cw_write(f, stackbuf, sizeof(stackbuf) - 1u);
		return;
	}
	vsnprintf(heap, (size_t)need + 1u, fmt, ap);
	cw_write(f, heap, (size_t)need);
	free(heap);
}
/* printf to stdout / stderr, console-aware (see cw_write). */
static void cwf(const char *fmt, ...)
{
	va_list ap; va_start(ap, fmt); cw_vprint(stdout, fmt, ap); va_end(ap);
}
static void cwe(const char *fmt, ...)
{
	va_list ap; va_start(ap, fmt); cw_vprint(stderr, fmt, ap); va_end(ap);
}
/* puts() to stdout (appends a newline), console-aware (see cw_write). */
static void cwputs(const char *s)
{
	cw_write(stdout, s, strlen(s));
	cw_write(stdout, "\n", 1u);
}
static void print_indent(uint32_t level, bool pretty)
{
	if (!pretty) return;
	for (uint32_t i = 0; i < level; i++) fputc('\t', stdout);
}
typedef struct {
	bvnr_writer_t *writer;
} pp_callback_ctx_t;
static bool on_verified_write(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	pp_callback_ctx_t *ctx = (pp_callback_ctx_t *)ud;
	return bvnr_write_event(ctx->writer, ev, d);
}
/* spec 1.1: a datetime carrying sub-second digits stores the whole-second epoch
 * count plus the verbatim fraction. Printing the bare carrier would silently
 * drop the fraction — and because the carrier is FLOORED, "carrier.frac" also
 * misreads a pre-epoch value (1969-12-31T23:59:59.5Z stores carrier -1, frac
 * "5"; the instant is -0.5, not -1.5). So reconstruct the faithful ISO literal
 * YYYY-MM-DDTHH:MM:SS.<frac>Z, the same form pretty-print emits. Returns false
 * (caller prints the integer carrier) for a non-fractional datetime, an atomic
 * GNSS epoch, or a carrier whose civil time falls outside the 0000..9999 an ISO
 * literal allows. Mirrors bvn_ser_datetime_to_civil in the writer. */
static bool print_datetime_iso_literal(const bvn_dom_node_t *node)
{
	value_type_spec_t vt = bvn_dom_get_value_type(node);
	if (vt.family != vt_datetime)
		return false;
	uint32_t fraclen = 0;
	const char *frac = bvn_dom_get_datetime_fraction(node, &fraclen);
	if (!frac || fraclen == 0)
		return false;
	int64_t secs = 0;
	if (!bvn_dom_get_i64(node, &secs))
		return false;
	/*
	 * Select on the epoch's IDENTITY, not on its spelling. bvnr_datetime_epoch_mjd
	 * returns the bvn_epoch_t -- the epoch's Modified Julian Day -- and those are
	 * distinct across the table, so comparing them answers the same question the
	 * old chain of strcmp(ep, "gps")... did, without depending on a name that is
	 * really a display string.
	 *
	 * It also drops a -Wanalyzer-deref-before-check that was a false positive
	 * with a real point behind it: GCC models strcmp's nonnull attribute as a
	 * NULL CHECK, so two adjacent strcmp(ep, ...) read as "dereferenced at the
	 * first, checked at the second" however the guard above is written. The code
	 * was correct -- `ep == NULL ||` short-circuits -- but the only way to write
	 * it that satisfies the analyzer is not to ask a string this question.
	 */
	const int32_t epoch = bvnr_datetime_epoch_mjd(vt);
	if (epoch == bvn_epoch_gps     || epoch == bvn_epoch_galileo ||
	    epoch == bvn_epoch_glonass || epoch == bvn_epoch_beidou)
		return false;
	bvn_datetime_t dt;
	memset(&dt, 0, sizeof dt);
	if (epoch == bvn_epoch_tai)
		bvn_dt_tai_seconds_to_utc(&dt, secs);
	else
		bvn_dt_epoch_seconds_to_datetime(&dt, (bvn_epoch_t)epoch, secs);
	/* Returning false here makes the caller print the bare integer carrier, which
	 * has nowhere to put the fraction — so the digits the spec promises will
	 * round-trip would silently disappear. The writer refuses this case outright;
	 * `query` prints the carrier (its output is a value, not a document) but says
	 * on stderr what was left out, so a caller piping stdout is never misled
	 * into thinking it holds the whole instant. */
	if (!(dt.date.month >= 1 && dt.date.month <= 12 &&
	      dt.date.day   >= 1 && dt.date.day   <= 31 &&
	      dt.date.year  >= 0 && dt.date.year  <= 9999)) {
		if (dt.date.month >= 1 && dt.date.month <= 12 &&
		    dt.date.day   >= 1 && dt.date.day   <= 31)
			fprintf(stderr, "warning: sub-second digits '.%.*s' omitted: "
				"UTC year %lld has no ISO literal\n",
				(int)fraclen, frac, (long long)dt.date.year);
		return false;
	}
	printf("%04lld-%02lld-%02lldT%02lld:%02lld:%02lld.%.*sZ",
		(long long)dt.date.year, (long long)dt.date.month,
		(long long)dt.date.day, (long long)dt.hour,
		(long long)dt.minute, (long long)dt.second,
		(int)fraclen, frac);
	return true;
}
static void print_dom_node(const bvn_dom_node_t *node, uint32_t indent)
{
	if (!node) { printf("null"); return; }
	switch (bvn_dom_node_type(node)) {
	case BVN_DOM_NULL:
		printf("null");
		break;
	case BVN_DOM_INT: {
		/* datetime with sub-second digits -> faithful ISO literal, not the
		 * fraction-dropping integer carrier. */
		if (print_datetime_iso_literal(node))
			break;
		value_type_spec_t vt = bvn_dom_get_value_type(node);
		if (vt.width > 64u) {
			char *s = bvn_dom_int_to_str(node, 10u);
			if (s) { fputs(s, stdout); free(s); }
			else   { printf("null"); }
		} else if (vt.family == vt_uint) {
			uint64_t v = 0;
			bvn_dom_get_u64(node, &v);
			printf("%" PRIu64, v);
		} else {
			int64_t v = 0;
			bvn_dom_get_i64(node, &v);
			printf("%" PRId64, v);
		}
		break;
	}
	case BVN_DOM_FLOAT: {
		double v;
		bvn_dom_get_float(node, &v);
		/* "%g" is six significant digits. In a format built on exact numerics
		 * that silently reported the wrong number: an exact-decimal money value
		 * of 12345678.99 printed as "1.23457e+07", losing the cents and the
		 * notation with them, and a float:128 came back at a fraction of its
		 * width. Use the library's own formatter, which emits the shortest
		 * decimal that reads back as the same double.
		 *
		 * That is as exact as this path can be, not as exact as the document:
		 * the DOM carries every float as a C double, so a float_dec or a
		 * float:128 has already been rounded before it reaches here. A consumer
		 * that needs the stored digits verbatim must read the value itself —
		 * the reader hands over the literal untouched.
		 *
		 * Shortest-round-trip rather than bvn_format_double: that one emits the
		 * canonical DOCUMENT form, which is always scientific, and turning "1.5"
		 * into "1.5e+0" for someone reading a value off the command line trades
		 * one wrong answer for an unreadable one. Widening %g until the text
		 * parses back to the same double gives full precision AND the plain
		 * notation for values that have one. */
		char fb[64];
		bvn_cli_format_double(fb, sizeof fb, v);
		fputs(fb, stdout);
		break;
	}
	case BVN_DOM_BOOL: {
		bool b = false;
		bvn_dom_get_bool(node, &b);
		fputs(b ? "true" : "false", stdout);
		break;
	}
	case BVN_DOM_STRING: {
		const char *s; uint32_t l;
		bvn_dom_get_string(node, &s, &l);
		putchar('"');
		for (uint32_t i = 0; i < l; i++) {
			uint8_t c = (uint8_t)s[i];
			switch (c) {
			case '\t': fputs("\\t", stdout); break;
			case '\n': fputs("\\n", stdout); break;
			case '\r': fputs("\\r", stdout); break;
			case '"':  fputs("\\\"", stdout); break;
			case '\\': fputs("\\\\", stdout); break;
			default:   putchar(c);
			}
		}
		putchar('"');
		break;
	}
	case BVN_DOM_SYMBOL:
	case BVN_DOM_REFERENCE: {
		const char *s; uint32_t l;
		if (bvn_dom_get_symbol(node, &s, &l) || bvn_dom_get_reference(node, &s, &l)) {
			fwrite(s, 1, l, stdout);
		}
		break;
	}
	case BVN_DOM_STRUCT: {
		putchar('{');
		if (bvn_dom_struct_count(node)) {
			putchar('\n');
			for (uint32_t i = 0; i < bvn_dom_struct_count(node); i++) {
				const bvn_dom_entry_t *e = bvn_dom_struct_entries(node);
				if (!e) break;
				print_indent(indent + 1, true);
				printf(".%s = ", e[i].key);
				print_dom_node(e[i].value, indent + 1);
				printf(";\n");
			}
			print_indent(indent, true);
		}
		putchar('}');
		break;
	}
	case BVN_DOM_ARRAY: {
		uint32_t cnt  = bvn_dom_array_count(node);
		uint32_t dims = bvn_dom_array_dims(node);
		/* The DOM stores /-separated dimension rows FLAT with the row count
		 * beside them, so printing the elements in order made two DIFFERENT
		 * documents identical: "[1,2,3]/[4,5,6]" and "[1,2,3,4,5,6]" both came
		 * out as [1, 2, 3, 4, 5, 6]. `query` prints a value someone reads or
		 * pipes onward, so the shape has to survive. print_json_node below
		 * already does this; this path did not. Row sizes are equal by
		 * construction (an uneven one is error_array_row_size_mismatch). */
		if (dims > 1 && cnt % dims == 0) {
			uint32_t row_len = cnt / dims;
			putchar('[');
			for (uint32_t r = 0; r < dims; r++) {
				if (r) fputs(", ", stdout);
				putchar('[');
				for (uint32_t c = 0; c < row_len; c++) {
					if (c) fputs(", ", stdout);
					print_dom_node(
						bvn_dom_array_at(node, r * row_len + c),
						indent);
				}
				putchar(']');
			}
			putchar(']');
			break;
		}
		putchar('[');
		for (uint32_t i = 0; i < cnt; i++) {
			if (i) fputs(", ", stdout);
			bvn_dom_node_t *elem = bvn_dom_array_at(node, i);
			print_dom_node(elem, indent);
		}
		putchar(']');
		break;
	}
	case BVN_DOM_OCTET_STREAM: {
		const uint8_t *b; uint32_t l;
		bvn_dom_get_octets(node, &b, &l);
		/* Octet streams are binary-mode and have no round-trippable text
		 * literal; print every byte as \xHH for inspection rather than
		 * silently dropping all but the first. */
		if (l == 0)
			fputs("\\x", stdout);
		else
			for (uint32_t i = 0; i < l; i++)
				printf("\\x%02x", b[i]);
		break;
	}
	}
}
#define EVT_LOG_CAP  8192
#define EVT_COL_WIDTH 80
static const char *evt_tok_str(token_type_t t)
{
	switch (t) {
	case token_is_identifier:   return "identifier";
	case token_is_string:       return "string";
	case token_is_number:       return "number";
	case token_is_symbol:       return "symbol";
	case token_is_reference:    return "reference";
	case token_is_array_number: return "arr_number";
	case token_is_array_string: return "arr_string";
	case token_is_type:         return "type";
	case token_is_octet_stream: return "octets";
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
static const char *evt_event_str(bvnr_event_t e)
{
	switch (e) {
	case ev_stream_start:                     return "stream_start";
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
	default:                                       return "?";
	}
}
static const char *evt_vtf_str(value_type_family_t f)
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
	case vt_illegal:   return "ILLEGAL";
	}
	return "?";
}
static bool evt_unit_has_real_base(value_unit_t u)
{
	if (u.num_components == 0)
		return false;
	for (uint32_t i = 0;
		 i < u.num_components && i < BVNR_MAX_UNIT_COMPONENTS;
		 i++) {
		if (u.components[i].base != bu_none)
			return true;
	}
	return false;
}
typedef struct {
	uint64_t     seq;
	bvnr_event_t event;
	token_type_t type;
	char         text[96];
	char         formatted[512];
} evt_logged_tok_t;
typedef struct {
	uint64_t         unverified_count;
	uint64_t         verified_count;
	uint64_t         next_seq;
	evt_logged_tok_t log[EVT_LOG_CAP];
	uint32_t         log_used;
	bvnr_canon_observer_t *canon;
	bvnr_reader_t        *rd;   /* source of the declared version for the tee */
} evt_ctx_t;
static void evt_format_token(const bvnr_data_t *d, bvnr_event_t e,
					  char *buf, size_t bufsize)
{
#define SCAT(...) \
	do { \
		if ((size_t)pos < bufsize) { \
			pos += snprintf(buf + pos, bufsize - (size_t)pos, __VA_ARGS__); \
			if ((size_t)pos >= bufsize) pos = (int)bufsize - 1; \
		} \
	} while (0)
	int pos = 0;
	SCAT("%-15s %-13s", evt_event_str(e), evt_tok_str(d->type));
	bool skip_data = (e == ev_type_annotation_start  ||
					  e == ev_type_annotation_end     ||
					  e == ev_type_annotation_type_family);
	if (d->type == token_is_type_width) {
		SCAT("%" PRIu32, d->value_type.width);
	} else if (d->type == token_is_type_base) {
		SCAT("%" PRIu32, d->value_type.base);
	} else if (d->type == token_is_unit) {
		if (d->data && d->length) {
			uint32_t show = d->length > 60 ? 60 : d->length;
			SCAT("\"");
			for (uint32_t i = 0;
				 i < show && (size_t)pos < bufsize - 4u;
				 i++) {
				uint8_t c = ((const uint8_t *)d->data)[i];
				if (c >= 0x20 && c < 0x7f)
					buf[pos++] = (char)c;
				else
					SCAT("\\x%02x", c);
			}
			if (show < d->length)
				SCAT("…");
			SCAT("\"");
		}
	} else if (!skip_data && d->data && d->length &&
			   d->type != token_is_octet_stream) {
		uint32_t show = d->length > 60 ? 60 : d->length;
		SCAT("\"");
		for (uint32_t i = 0;
			 i < show && (size_t)pos < bufsize - 4u;
			 i++) {
			uint8_t c = ((const uint8_t *)d->data)[i];
			if (c >= 0x20 && c < 0x7f)
				buf[pos++] = (char)c;
			else
				SCAT("\\x%02x", c);
		}
		if (show < d->length)
			SCAT("…");
		SCAT("\"");
	} else if (d->type == token_is_octet_stream) {
		uint32_t bytes = d->length ? d->length : 65536u;
		SCAT("<%u bytes>", bytes);
	}
	if (!bvn_type_is_plain(d->value_type)  &&
		d->type != token_is_type_width      &&
		d->type != token_is_type_base       &&
		d->type != token_is_unit) {
		value_type_spec_t vt = d->value_type;
		value_unit_t      u  = d->value_unit;
		SCAT(" <%s", evt_vtf_str(vt.family));
		if (bvn_type_is_numeric(vt)) {
			uint32_t ew = bvn_effective_width(vt);
			uint32_t eb = bvn_effective_base(vt);
			SCAT(":%" PRIu32 ",_%" PRIu32, ew, eb);
			if (evt_unit_has_real_base(u)) {
				/* bvn_unit_to_string returns -1 WITHOUT writing a NUL when the
				 * text does not fit, so ignoring the return handed "%s" an
				 * unterminated stack buffer — a stack-buffer-overflow that a
				 * legal document with a long compound unit reached (128 bytes
				 * held only about seven of the eight components a unit may
				 * carry). Size from BVNR_UNIT_STRING_MAX, which gen_units.py
				 * checks against the real table, and still refuse rather than
				 * print whatever happens to be on the stack. */
				char unit_buf[BVNR_UNIT_STRING_MAX];
				if (bvn_unit_to_string(u, unit_buf, sizeof(unit_buf)) < 0)
					SCAT(",<unprintable unit>");
				else
					SCAT(",%s", unit_buf);
			} else {
				SCAT(",no_unit");
			}
		}
		SCAT(">");
	}
	/* A read-time conversion, shown next to the value it belongs to. conv.text is
	 * NULL when the exact result has no terminating expansion in the output base
	 * and the reader was told to hand over the rational anyway; say so rather
	 * than printing nothing, or the value looks unconverted. */
	if (d->converted) {
		char cu[BVNR_UNIT_STRING_MAX];
		if (bvn_unit_to_string(d->conv.unit, cu, sizeof(cu)) < 0)
			cu[0] = '\0';
		if (d->conv.text) {
			uint32_t show = d->conv.length > 40 ? 40 : d->conv.length;
			SCAT(" → \"");
			for (uint32_t i = 0; i < show && (size_t)pos < bufsize - 8u; i++)
				buf[pos++] = d->conv.text[i];
			if (show < d->conv.length)
				SCAT("…");
			SCAT("\"");
		} else {
			SCAT(" → <non-terminating>");
		}
		if (cu[0])
			SCAT(" %s", cu);
		if (d->conv.base)
			SCAT(" _%" PRIu32, d->conv.base);
	}
	buf[pos] = '\0';
#undef SCAT
}
/*
 * Shared parsing for the reader-side unit policy options
 * (bvnr_reader_set_unit_policy), used by both `events` and `validate` so the two
 * cannot drift into meaning different things by the same flag.
 *
 * Consumes the options it recognises from argv[*argi] onwards and leaves *argi
 * on the first argument it does not. Returns 0 on success, 2 on a usage error
 * (already reported), and 1 on a policy the library rejected -- a malformed unit
 * is caught here, before a byte of the document is read, which is the whole
 * point of the setter validating up front.
 */
typedef struct {
	bvnr_unit_target_t targets[BVNR_MAX_UNIT_TARGETS];
	const char        *require[BVNR_MAX_UNIT_TARGETS];
	bvnr_unit_rule_t   rules[BVNR_MAX_UNIT_RULES];
	bvnr_unit_policy_t policy;
	bool               any;
	/* Not part of the unit policy, but the same KIND of thing and reached by
	 * the same commands: an assertion the consumer makes about the document
	 * before reading it. Riding along here is what gives validate, query and
	 * events the flag from one parse site instead of three. */
	bool               text_only;
} cli_unit_policy_t;

/*
 * "<path>=<unit>" for the per-field flags. argv strings are writable and live
 * for the whole run, so the '=' is overwritten in place and both halves point
 * into the original — no allocation, no lifetime question.
 */
static bool cli_split_field(const char *cmd, char *arg,
			    const char **path, const char **unit)
{
	char *eq = strchr(arg, '=');
	if (!eq || eq == arg || !eq[1]) {
		fprintf(stderr, "%s: expected <path>=<unit>, got '%s'\n", cmd, arg);
		return false;
	}
	*eq   = '\0';
	*path = arg;
	*unit = eq + 1;
	return true;
}

static int cli_parse_unit_opts(const char *cmd, int argc, char **argv, int *argi,
			       cli_unit_policy_t *up)
{
	const char *a = argv[*argi];
	if (strcmp(a, "--unit") == 0) {
		if (*argi + 1 >= argc) {
			fprintf(stderr, "%s: --unit needs a unit\n", cmd); return 2;
		}
		if (up->policy.num_targets >= BVNR_MAX_UNIT_TARGETS) {
			fprintf(stderr, "%s: at most %u --unit targets\n", cmd,
				BVNR_MAX_UNIT_TARGETS);
			return 2;
		}
		up->targets[up->policy.num_targets].unit = argv[*argi + 1];
		up->policy.num_targets++;
		up->any = true;
		*argi += 2;
		return 0;
	}
	if (strcmp(a, "--field") == 0 || strcmp(a, "--require-field") == 0) {
		bool require_only = (a[2] == 'r');
		if (*argi + 1 >= argc) {
			fprintf(stderr, "%s: %s needs <path>=<unit>\n", cmd, a);
			return 2;
		}
		if (up->policy.num_rules >= BVNR_MAX_UNIT_RULES) {
			fprintf(stderr, "%s: at most %u per-field rules\n", cmd,
				BVNR_MAX_UNIT_RULES);
			return 2;
		}
		const char *path = NULL, *unit = NULL;
		if (!cli_split_field(cmd, argv[*argi + 1], &path, &unit))
			return 2;
		bvnr_unit_rule_t *r = &up->rules[up->policy.num_rules++];
		r->path = path;
		r->unit = unit;
		r->base = 0u;
		r->mode = require_only ? bvnr_rule_require : bvnr_rule_convert;
		up->any = true;
		*argi += 2;
		return 0;
	}
	if (strcmp(a, "--require-dimension") == 0) {
		if (*argi + 1 >= argc) {
			fprintf(stderr, "%s: --require-dimension needs a unit\n", cmd);
			return 2;
		}
		if (up->policy.num_require_dimension_of >= BVNR_MAX_UNIT_TARGETS) {
			fprintf(stderr, "%s: at most %u --require-dimension units\n", cmd,
				BVNR_MAX_UNIT_TARGETS);
			return 2;
		}
		up->require[up->policy.num_require_dimension_of++] = argv[*argi + 1];
		up->any = true;
		*argi += 2;
		return 0;
	}
	if (strcmp(a, "--base") == 0) {
		if (*argi + 1 >= argc) {
			fprintf(stderr, "%s: --base needs a number\n", cmd); return 2;
		}
		/* Decided HERE, against the library's own rule, and for digits only.
		 * The old test was `b > 85`, which let 1, 63 and 65..84 through to
		 * bvnr_reader_set_unit_policy -- and cli_report_unit_policy, probing
		 * each entry with the bad base stamped on it, then accused the first
		 * --unit it found: "--unit 'm' is not a unit bovnar can parse", for a
		 * unit that parses and a base that was never named. strtoul also took
		 * "" as 0 and "+16" as 16. */
		const char *bs = argv[*argi + 1];
		if (!bs) bs = "";   /* argc says it is there; -fanalyzer wants it said */
		char *end = NULL;
		unsigned long b = (bs[0] >= '0' && bs[0] <= '9')
				? strtoul(bs, &end, 10) : 0ul;
		if (!end || *end || b > 85ul ||
		    !bvn_rational_base_valid((uint32_t)b)) {
			fprintf(stderr, "%s: --base '%s' is not a base bovnar can write "
					"(2..62, 64, 85)\n", cmd, bs);
			return 2;
		}
		/* Recorded, not applied: the targets are stamped with it in
		 * cli_apply_unit_policy, once the whole command line has been seen.
		 * Applying it here made the flag order-dependent — "--base 16 --unit m"
		 * lost the base, because the later --unit wrote its own slot. */
		up->policy.base = (uint32_t)b;
		up->any = true;
		*argi += 2;
		return 0;
	}
	if (strcmp(a, "--si") == 0) {
		up->policy.normalise = bvnr_normalise_si;
		up->any = true;
		(*argi)++;
		return 0;
	}
	if (strcmp(a, "--leave-inexact") == 0) {
		up->policy.on_inexact = bvnr_inexact_leave;
		up->any = true;
		(*argi)++;
		return 0;
	}
	if (strcmp(a, "--require-unit") == 0) {
		up->policy.require_unit = true;
		up->any = true;
		(*argi)++;
		return 0;
	}
	if (strcmp(a, "--text-only") == 0) {
		/* Deliberately does NOT set up->any: that flag gates
		 * bvnr_reader_set_unit_policy, and this is a read flag with no policy
		 * of its own. Setting it would install an empty policy for nothing. */
		up->text_only = true;
		(*argi)++;
		return 0;
	}
	return -1;                       /* not one of ours */
}

/*
 * Ask the library about ONE policy entry, on a scratch reader so the caller's
 * reader is left alone. Using bvnr_reader_set_unit_policy itself as the oracle
 * -- rather than re-deriving its rules here -- is what keeps this from drifting
 * away from bvn_unit_policy_parse the next time that grows a check.
 *
 * A reader it cannot allocate answers "fine", so a probe that never ran cannot
 * accuse an argument; the caller then falls back to the general message.
 */
static bool cli_policy_probe(const bvnr_unit_policy_t *p)
{
	bvnr_reader_t *pr = bvnr_reader_create();
	if (!pr)
		return true;
	bool ok = bvnr_reader_set_unit_policy(pr, p);
	bvnr_reader_destroy(pr);
	return ok;
}

/*
 * Name the argument the caller actually has to fix.
 *
 * The setter answers one bool for the whole policy, and this used to be
 * reported as "check the unit spellings passed to --unit / --require-dimension"
 * whatever had gone wrong. That is right for two of the six things that reach
 * it and wrong for the rest: a rule path over BVNR_MAX_UNIT_PATH bytes, one
 * without a leading '.', and a bare ".*" are all PATH faults reported as unit
 * spellings, and a bad unit in --field / --require-field named neither flag.
 * The count limits (--unit, rules) and --base never get here -- cli_parse_unit_opts
 * catches those itself, with their own messages. (For --base that was only half
 * true until it validated the value with bvn_rational_base_valid: see there.)
 *
 * Each entry is re-offered on its own; the first the library refuses is the one
 * to report. For a rule, the unit is then swapped for one that certainly parses
 * to decide which HALF of "<path>=<unit>" is at fault.
 */
static void cli_report_unit_policy(const char *cmd, const cli_unit_policy_t *up)
{
	bvnr_unit_policy_t probe;

	for (uint32_t i = 0; i < up->policy.num_targets; i++) {
		memset(&probe, 0, sizeof(probe));
		probe.targets     = &up->targets[i];
		probe.num_targets = 1;
		if (!cli_policy_probe(&probe)) {
			fprintf(stderr, "%s: --unit '%s' is not a unit bovnar can "
					"parse\n", cmd, up->targets[i].unit);
			return;
		}
	}
	for (uint32_t i = 0; i < up->policy.num_require_dimension_of; i++) {
		memset(&probe, 0, sizeof(probe));
		probe.require_dimension_of     = &up->require[i];
		probe.num_require_dimension_of = 1;
		if (!cli_policy_probe(&probe)) {
			fprintf(stderr, "%s: --require-dimension '%s' is not a unit "
					"bovnar can parse\n", cmd, up->require[i]);
			return;
		}
	}
	for (uint32_t i = 0; i < up->policy.num_rules; i++) {
		bvnr_unit_rule_t  one  = up->rules[i];
		const char       *flag = one.mode == bvnr_rule_require
				       ? "--require-field" : "--field";

		memset(&probe, 0, sizeof(probe));
		probe.rules     = &one;
		probe.num_rules = 1;
		if (cli_policy_probe(&probe))
			continue;
		/* Path or unit? Re-offer the same rule with a unit that certainly
		 * parses: if it then holds, the unit was the fault; if it still
		 * fails, the path is. */
		one.unit = "m";
		if (cli_policy_probe(&probe))
			fprintf(stderr, "%s: %s '%s=%s' — '%s' is not a unit bovnar "
					"can parse\n", cmd, flag, up->rules[i].path,
				up->rules[i].unit, up->rules[i].unit);
		else
			fprintf(stderr, "%s: %s path '%s' is unusable — a path must "
					"start with '.', be at most %u bytes, and "
					"cannot be just '.*'\n",
				cmd, flag, up->rules[i].path, BVNR_MAX_UNIT_PATH);
		return;
	}
	/* Nothing singled itself out -- a whole-policy refusal, or a probe that
	 * could not run. Say only what is certain. */
	fprintf(stderr, "%s: unusable unit policy — check the arguments passed to "
			"--unit / --require-dimension / --field / --require-field\n",
		cmd);
}

/*
 * Point the policy at the collected arrays and stamp one --base across every
 * place a base can apply, so the flags may appear in any order on the command
 * line. Shared for the same reason cli_parse_unit_opts is: `query` kept its own
 * copy of this, which is how the three commands drift into meaning different
 * things by the same flag.
 */
static void cli_stamp_unit_policy(cli_unit_policy_t *up)
{
	for (uint32_t i = 0; i < up->policy.num_targets; i++)
		up->targets[i].base = up->policy.base;
	for (uint32_t i = 0; i < up->policy.num_rules; i++) {
		if (up->rules[i].mode == bvnr_rule_convert)
			up->rules[i].base = up->policy.base;
	}
	up->policy.targets              = up->targets;
	up->policy.rules                = up->rules;
	up->policy.require_dimension_of = up->require;
}

/* Install what cli_parse_unit_opts collected. A refusal is reported against the
 * argument that caused it -- see cli_report_unit_policy. */
static bool cli_apply_unit_policy(const char *cmd, bvnr_reader_t *r,
				  cli_unit_policy_t *up)
{
	if (!up->any)
		return true;
	cli_stamp_unit_policy(up);
	if (bvnr_reader_set_unit_policy(r, &up->policy))
		return true;
	cli_report_unit_policy(cmd, up);
	return false;
}
static bool evt_on_unverified(void *ud, bvnr_event_t e, bvnr_data_t *d);
static bool evt_on_unverified_tee(void *ud, bvnr_event_t e, bvnr_data_t *d)
{
	evt_ctx_t *ctx = (evt_ctx_t *)ud;
	/* The reader resolves the document's declared version while parsing the
	 * leading directive (before any value event), so forward it to the
	 * observer here — idempotent and a no-op once output has begun — so the
	 * canonical debug dump carries the directive a 1.1 document needs. */
	uint16_t vmaj = 0, vmin = 0;
	if (ctx->rd && bvnr_reader_get_declared_version(ctx->rd, &vmaj, &vmin))
		bvnr_canon_observer_set_version(ctx->canon, vmaj, vmin);
	if (!bvnr_canon_observer_on_event(ctx->canon, e, d)) return false;
	return evt_on_unverified(ud, e, d);
}
static bool evt_on_unverified(void *ud, bvnr_event_t e, bvnr_data_t *d)
{
	evt_ctx_t *ctx = (evt_ctx_t *)ud;
	ctx->unverified_count++;
	if (ctx->log_used < EVT_LOG_CAP) {
		evt_logged_tok_t *entry = &ctx->log[ctx->log_used++];
		entry->seq   = ctx->next_seq;
		entry->event = e;
		entry->type  = d->type;
		if (d->data && d->length                        &&
			d->type != token_is_octet_stream            &&
			e != ev_type_annotation_start               &&
			e != ev_type_annotation_end                 &&
			e != ev_type_annotation_type_family) {
			uint32_t n = d->length < sizeof(entry->text) - 1
					   ? d->length
					   : (uint32_t)sizeof(entry->text) - 1;
			memcpy(entry->text, d->data, n);
			entry->text[n] = '\0';
		} else {
			entry->text[0] = '\0';
		}
		evt_format_token(d, e, entry->formatted, sizeof(entry->formatted));
	}
	ctx->next_seq++;
	return true;
}
static bool evt_on_verified(void *ud, bvnr_event_t e, bvnr_data_t *d)
{
	evt_ctx_t *ctx = (evt_ctx_t *)ud;
	ctx->verified_count++;
	int match_idx = -1;
	for (uint32_t i = 0; i < ctx->log_used; i++) {
		if (ctx->log[i].event == e && ctx->log[i].type == d->type) {
			match_idx = (int)i;
			break;
		}
	}
	char verified_str[512];
	evt_format_token(d, e, verified_str, sizeof(verified_str));
	if (match_idx >= 0) {
		cwf("  %-*s │ %s\n", EVT_COL_WIDTH,
			   ctx->log[match_idx].formatted, verified_str);
		memmove(&ctx->log[match_idx],
				&ctx->log[match_idx + 1],
				(ctx->log_used - (uint32_t)match_idx - 1u)
				* sizeof(ctx->log[0]));
		ctx->log_used--;
	} else {
		cwf("  %-*s │ %s\n", EVT_COL_WIDTH, "(unmatched)", verified_str);
	}
	return true;
}
static void evt_on_error(void *ud, error_code_t err,
						 uint64_t line, uint64_t column,
						 uint32_t byte, uint64_t offset)
{
	(void)ud;
	cwe(
			"  ⚠ RECOVERY at line %" PRIu64 " col %" PRIu64
			": %s (byte 0x%02" PRIx32 " offset %" PRIu64 ")\n",
			line, column,
			bvn_error_to_string(err),
			byte, offset);
}
static int cmd_events(int argc, char **argv)
{
	bool continue_on_error = false;
	bool enable_debug      = false;
	bool debug_pretty      = false;
	cli_unit_policy_t up;
	memset(&up, 0, sizeof up);
	int argi = 0;
	while (argi < argc && argv[argi][0] == '-') {
		if (strcmp(argv[argi], "-c") == 0) {
			continue_on_error = true; argi++; continue;
		}
		if (strcmp(argv[argi], "-d") == 0) {
			enable_debug = true;      argi++; continue;
		}
		if (strcmp(argv[argi], "-p") == 0) {
			debug_pretty = true;      argi++; continue;
		}
		{
			int rc = cli_parse_unit_opts("events", argc, argv, &argi, &up);
			if (rc == 0) continue;
			if (rc > 0)  return rc;
		}
		if (strcmp(argv[argi], "-") != 0) {
			fprintf(stderr, "events: unknown option: %s\n", argv[argi]);
			return 2;
		}
		break;
	}
	if (argi >= argc) {
		fprintf(stderr, "events: missing file argument\n");
		return 2;
	}
	if (argc - argi > 1) {
		fprintf(stderr, "events: takes exactly one file (or -), got %d\n",
			argc - argi);
		return 2;
	}
	if (debug_pretty && !enable_debug)
		fprintf(stderr, "Warning: -p has no effect without -d\n");
	const char *filename  = argv[argi];
	bool        from_stdin = (strcmp(filename, "-") == 0);
	int fd;
	if (from_stdin) {
		fd = STDIN_FILENO;
	} else {
		fd = open(filename, BVN_O_RDONLY);
		if (fd < 0) { perror(filename); return 1; }
	}
	bvnr_reader_t *rd = bvnr_reader_create();
#if defined(__GNUC__) && !defined(__clang__)
#  pragma GCC diagnostic push
#  pragma GCC diagnostic ignored "-Wanalyzer-fd-leak"
#endif
	if (!rd) {
		fprintf(stderr, "error: failed to allocate reader\n");
		if (!from_stdin) { close(fd); fd = -1; }
		return 1;
	}
#if defined(__GNUC__) && !defined(__clang__)
#  pragma GCC diagnostic pop
#endif
	if (!cli_apply_unit_policy("events", rd, &up)) {
		bvnr_reader_destroy(rd);
		if (!from_stdin) close(fd);
		return 2;
	}
	bvnr_source_t src;
	bvnr_source_from_fd(&src, fd);
	evt_ctx_t *ctx = calloc(1, sizeof(*ctx));
	if (!ctx) {
		fprintf(stderr, "error: failed to allocate event context\n");
		bvnr_reader_destroy(rd);
		if (!from_stdin) close(fd);
		return 1;
	}
	bvnr_read_flags_t flags;
	memset(&flags, 0, sizeof(flags));
	flags.max_file_size      = UINT32_MAX;
	flags.max_array_nesting  = 255;
	flags.max_struct_nesting = 255;
	flags.userdata           = ctx;
	flags.on_unverified      = evt_on_unverified;
	flags.on_verified        = evt_on_verified;
	flags.continue_on_error  = continue_on_error;
	flags.text_only          = up.text_only;
	flags.on_error           = evt_on_error;
	bvnr_canon_observer_t *canon = NULL;
	if (enable_debug) {
		bvnr_sink_t dbg_sink;
		bvnr_sink_to_fd(&dbg_sink, STDERR_FILENO);
		canon = bvnr_canon_observer_create(&dbg_sink, debug_pretty);
		if (!canon) {
			fprintf(stderr, "error: failed to allocate canon observer\n");
			free(ctx);
			bvnr_reader_destroy(rd);
			if (!from_stdin) close(fd);
			return 1;
		}
		ctx->canon = canon;
		ctx->rd    = rd;
		flags.on_unverified = evt_on_unverified_tee;
	}
	if (!bvnr_open_read_source(rd, &src, NULL, &flags)) {
		fprintf(stderr, "error: bvnr_open_read_source failed\n");
		bvnr_canon_observer_destroy(canon);
		free(ctx);
		bvnr_reader_destroy(rd);
		if (!from_stdin) close(fd);
		return 1;
	}
	cwputs("═══════════════════════════════════════════════════════════════════"
		 "════════════════════════════════════════════════════════════════");
	cwf("  Parsing: %s", from_stdin ? "<stdin>" : filename);
	if (enable_debug)
		cwf("  [debug %s]", debug_pretty ? "pretty" : "compact");
	cwf("\n");
	cwputs("═══════════════════════════════════════════════════════════════════"
		 "════════════════════════════════════════════════════════════════");
	cwf("\n");
	cwf("  %-80s │ %s\n", "LEXER (unverified)", "VALIDATOR (verified)");
	cwputs("  ────────────────────────────────────────────────────────────────"
		 "────────────────┼────────────────────────────────────────────────"
		 "────────────────────────────────────");
	bool ok = bvnr_read(rd);
	if (canon) {
		bvnr_canon_observer_finish(canon);
		bvnr_canon_observer_destroy(canon);
		canon = NULL;
		ctx->canon = NULL;
	}
	for (uint32_t i = 0; i < ctx->log_used; i++)
		cwf("  %-*s │ —\n", EVT_COL_WIDTH, ctx->log[i].formatted);
	cwputs("\n───────────────────────────────────────────────────────────────────"
		 "────────────────────────────────────────────────────────────────");
	cwputs("  Summary");
	cwputs("───────────────────────────────────────────────────────────────────"
		 "────────────────────────────────────────────────────────────────");
	cwf("  Lexer tokens     : %" PRIu64 "\n", ctx->unverified_count);
	cwf("  Validated tokens : %" PRIu64 "\n", ctx->verified_count);
	uint64_t recoveries = bvnr_reader_get_recovery_count(rd);
	if (recoveries > 0) {
		cwf("\n  ⚠ %" PRIu64 " error(s) recovered from via resync.\n",
			   recoveries);
		/* How OFTEN recovery ran says nothing about what it cost: one skipped
		 * assignment and a whole discarded struct both report a single
		 * recovery. The bytes are the part a reader of this output needs. */
		cwf("  ⚠ %" PRIu64 " byte(s) of input were discarded and never "
			   "parsed.\n", bvnr_reader_get_skipped_bytes(rd));
	}
	if (!ok) {
		error_code_t err = bvnr_reader_get_error(rd);
		cwf("\n  ✗ PARSE ERROR\n");
		cwf("  ┌──────────────────────────────────────────────────────────"
			   "──────────────────────\n");
		cwf("  │ code   : %d (%s)\n", err, bvn_error_to_string(err));
		cwf("  │ line   : %" PRIu64 "\n", bvnr_reader_get_error_line(rd));
		cwf("  │ column : %" PRIu64 "\n", bvnr_reader_get_error_column(rd));
		cwf("  │ byte   : 0x%02" PRIx32 "\n", bvnr_reader_get_error_byte(rd));
		cwf("  │ offset : %" PRIu64 "\n", bvnr_reader_get_error_offset(rd));
		cwf("  └──────────────────────────────────────────────────────────"
			   "──────────────────────\n");
	}
	uint64_t unmatched = ctx->unverified_count - ctx->verified_count;
	if (unmatched > 0) {
		cwf("\n  ⚠ %" PRIu64 " token(s) emitted by the lexer but never "
			   "validated.\n", unmatched);
		if (ctx->log_used > 0) {
			cwputs("\n  ╔══════════════════════════════════════════════════════"
				 "══════════════════════════╗");
			cwputs("  ║       UNVALIDATED TOKENS                              "
				 "                           ║");
			cwputs("  ╠══════════════════════════════════════════════════════"
				 "══════════════════════════╣");
			for (uint32_t i = 0; i < ctx->log_used; i++) {
				const evt_logged_tok_t *e = &ctx->log[i];
				cwf("  ║  #%-5" PRIu64 " %-15s %-13s",
					   e->seq, evt_event_str(e->event), evt_tok_str(e->type));
				if (e->text[0])
					cwf(" \"%s\"", e->text);
				cwf("\n");
			}
			cwputs("  ╚══════════════════════════════════════════════════════"
				 "══════════════════════════╝");
		}
	}
	bvnr_reader_destroy(rd);

	/* The streaming pass alone is not the whole of validity. Array homogeneity,
	 * struct shape and duplicate keys are DOM-tier rules (spec 12.4/12.5), so
	 * `events` printed "All tokens validated successfully" and exited 0 for a
	 * document `validate` rejects -- and CMakeLists_tests.txt uses that very
	 * string as the per-example validity gate, which made the gate blind to
	 * every whole-array property. Run the same pass when the input is seekable;
	 * say so plainly when it is not, rather than overclaiming. */
	/* A recovered error is still an error. Under -c the read returns true once
	 * it has resynchronised, so a document with a broken assignment reached
	 * the "All tokens validated successfully" line -- printed directly under
	 * "N byte(s) of input were discarded and never parsed" -- and, from stdin,
	 * exit status 0. From a file the same document exited 1, but only because
	 * the document-tier pass below does not resync; the verdict depended on
	 * where the bytes came from. -c decides how far the listing goes, not
	 * whether the document was valid. */
	if (ok && recoveries > 0) {
		cwe("\n  ✗ %" PRIu64 " error(s) were recovered from; the document is "
		    "not valid.\n", recoveries);
		ok = false;
	}
	if (ok && !from_stdin) {
		if (lseek(fd, 0, SEEK_SET) == 0) {
			bvn_dom_doc_t *hdoc = bvn_dom_parse_fd(fd);
			error_code_t herr = hdoc ? bvn_dom_doc_get_parse_error(hdoc)
			                         : error_none;
			if (hdoc) bvn_dom_doc_destroy(hdoc);
			if (herr != error_none) {
				cwe("\n  ✗ Document-tier validation failed: %s\n",
				    bvn_error_to_string(herr));
				ok = false;
			} else {
				cwputs("\n  ✓ All tokens validated successfully.");
			}
		} else {
			cwputs("\n  ✓ All tokens validated successfully "
			       "(streaming tier only; input is not seekable).");
		}
	} else if (ok) {
		cwputs("\n  ✓ All tokens validated successfully "
		       "(streaming tier only; stdin is not seekable).");
	}
	if (!from_stdin) close(fd);
	free(ctx);
	return ok ? 0 : 1;
}
static int cmd_validate(const char *filename, cli_unit_policy_t *up)
{
	int fd = open(filename, BVN_O_RDONLY);
	if (fd < 0) { perror(filename); return 1; }
	bvnr_reader_t *r = bvnr_reader_create();
	if (!r) { close(fd); return 1; }
	if (!cli_apply_unit_policy("validate", r, up)) {
		bvnr_reader_destroy(r); close(fd); return 2;
	}
	bvnr_source_t src;
	bvnr_source_from_fd(&src, fd);
	bvnr_read_flags_t flags = {0};
	flags.max_file_size     = UINT32_MAX;
	flags.max_array_nesting = 255;
	flags.max_struct_nesting = 255;
	flags.text_only         = up->text_only;
	bool ok = bvnr_open_read_source(r, &src, NULL, &flags) && bvnr_read(r);
	error_code_t err = bvnr_reader_get_error(r);
	if (!ok) {
		fprintf(stderr, "Validation failed: %s at line %" PRIu64 ", col %" PRIu64 "\n",
				bvn_error_to_string(err),
				bvnr_reader_get_error_line(r), bvnr_reader_get_error_column(r));
		bvnr_reader_destroy(r);
		close(fd);
		return 1;
	}
	uint16_t vmaj = 0, vmin = 0;
	bool has_version = bvnr_reader_get_declared_version(r, &vmaj, &vmin);
	bvnr_reader_destroy(r);
	/*
	 * The streaming pass above validated per-value syntax, types and units.
	 * Array element homogeneity (spec 1.0) is a cross-element, whole-array
	 * property, enforced over the materialised DOM — so run a second pass
	 * through bvn_dom_parse_fd and surface any homogeneity error. Rewind and
	 * reuse the one descriptor rather than reopening, so both passes see the
	 * same bytes (no reopen TOCTOU) and there is a single fd to manage.
	 */
	if (lseek(fd, 0, SEEK_SET) != 0) {
		/* The homogeneity pass re-reads from the start via the DOM, which only
		 * accepts an fd. A non-seekable input (FIFO/pipe/char device) was
		 * already consumed by the streaming pass and cannot be rewound; report
		 * that clearly rather than leaking a bare "Illegal seek". */
		if (errno == ESPIPE)
			cwe("validate: %s: not a regular file; validate needs a seekable "
			    "file for the array-homogeneity pass\n", filename);
		else
			perror(filename);
		close(fd); return 1;
	}
	bvn_dom_doc_t *doc = bvn_dom_parse_fd(fd);
	close(fd);
	if (!doc) {
		fprintf(stderr, "Validation failed: out of memory\n");
		return 1;
	}
	error_code_t herr = bvn_dom_doc_get_parse_error(doc);
	bvn_dom_doc_destroy(doc);
	if (herr != error_none) {
		fprintf(stderr, "Validation failed: %s\n", bvn_error_to_string(herr));
		return 1;
	}
	if (has_version)
		printf("%s: OK (declares bovnar %u.%u)\n", filename, vmaj, vmin);
	else
		printf("%s: OK\n", filename);
	return 0;
}
static int cmd_query(const char *path, const char *filename,
		     cli_unit_policy_t *up)
{
	/* --text-only is a READ FLAG, and this command goes through the DOM, which
	 * takes a unit policy but not the read flags. Refusing is the only correct
	 * answer: an assertion the caller believes is in force but which is quietly
	 * ignored is worse than no assertion at all, and that is precisely the
	 * failure this flag exists to prevent. */
	if (up && up->text_only) {
		fprintf(stderr, "query: --text-only is not supported here "
			"(the DOM path takes no read flags); use "
			"`bovnar validate --text-only` instead\n");
		return 2;
	}
	/* --base is refused for the same reason, from the other side: it is not
	 * that the option cannot be put in force, it is that this command has no
	 * way to SHOW it. `query` prints a number, in decimal, from a tree that
	 * holds every float as a double -- so "--base 16" printed "2500" exactly
	 * as without it, and (until the DOM stopped stamping the requested base on
	 * types that have none) "--base 2" on a float failed the whole document
	 * with value_out_of_range. An option that is either invisible or fatal is
	 * not one this command takes; `events` renders a conversion in any base. */
	if (up && up->policy.base) {
		fprintf(stderr, "query: --base is not supported here (query prints "
			"values in decimal); use `bovnar events --base %u` to see "
			"a conversion rendered in another base\n",
			(unsigned)up->policy.base);
		return 2;
	}
	/*
	 * Check the policy BEFORE the document is opened, and report it the way
	 * validate and events do.
	 *
	 * This command reaches the setter through the DOM builder, which surfaces a
	 * refused policy as error_invalid_argument on the PARSE -- so every bad
	 * --unit, --require-dimension or --field argument came back as "Parse error
	 * in <file>: invalid_argument". The document was blamed, by name, for a
	 * fault in the command line, and the file was read before anyone noticed.
	 */
	if (up && up->any) {
		cli_stamp_unit_policy(up);
		if (!cli_policy_probe(&up->policy)) {
			cli_report_unit_policy("query", up);
			return 2;
		}
	}
	int fd = open(filename, BVN_O_RDONLY);
	if (fd < 0) { perror(filename); return 1; }
	/* The DOM takes the same policy the streaming reader does, so a query can
	 * assert what it expects to find and ask for the unit it wants back. */
	bvn_dom_doc_t *doc;
	if (up && up->any) {
		doc = bvn_dom_parse_fd_policy(fd, 0, &up->policy);
	} else {
		doc = bvn_dom_parse_fd(fd);
	}
	close(fd);
	if (!doc) {
		fprintf(stderr, "Failed to parse %s\n", filename);
		return 1;
	}
	{
		error_code_t perr = bvn_dom_doc_get_parse_error(doc);
		if (perr != error_none) {
			fprintf(stderr, "Parse error in %s: %s\n",
				filename, bvn_error_to_string(perr));
			bvn_dom_doc_destroy(doc);
			return 1;
		}
	}
	bvn_dom_node_t *node = bvn_dom_lookup(doc, path);
	if (!node) {
		fprintf(stderr, "Path '%s' not found\n", path);
		bvn_dom_doc_destroy(doc);
		return 1;
	}
	print_dom_node(node, 0);
	putchar('\n');
	/* query prints the number alone. Under --leave-inexact a value the policy
	 * would have converted can come back in its own unit, and nothing on
	 * stdout says so; name the unit it was left in. (A per-field rule never
	 * leaves a value: it fails instead.) */
	if (up && up->policy.on_inexact == bvnr_inexact_leave &&
	    (bvn_dom_node_type(node) == BVN_DOM_INT ||
	     bvn_dom_node_type(node) == BVN_DOM_FLOAT)) {
		value_unit_t u = bvn_dom_get_unit(node);
		bool would = false, there = false;
		for (uint32_t i = 0; i < up->policy.num_targets && !there; i++) {
			bool ok = false;
			value_unit_t t = bvn_parse_unit(
				(const uint8_t *)up->policy.targets[i].unit, &ok);
			if (!ok)
				continue;
			if (bvn_unit_equal(u, t))
				there = true;
			else if (bvn_units_convertible(u, t))
				would = true;
		}
		value_unit_t si;
		if (!there && up->policy.normalise == bvnr_normalise_si &&
		    bvn_unit_si_normal_form(u, &si) && !bvn_unit_equal(si, u))
			would = true;
		if (would && !there) {
			char ub[BVNR_UNIT_STRING_MAX];
			if (bvn_unit_to_string(u, ub, sizeof ub) < 0)
				strcpy(ub, "?");
			fprintf(stderr, "query: %s left in %s: the conversion is not "
				"exact (--leave-inexact)\n", path, ub);
		}
	}
	bvn_dom_doc_destroy(doc);
	return 0;
}
/*
 * `canonical` turns on BVN_UNIT_REDUCE for the whole document: every compound
 * unit folds to its reduced form (m·m -> m2, and every prefix out), and the
 * writer rescales each value to match, exactly. That is the difference between
 * a pretty-printer and a canonicaliser -- two spellings of one unit come out
 * as the same bytes, which is what a signature, a content hash or a semantic
 * diff needs and what re-spelling alone does not give. It stops short of
 * unifying every pair of equal quantities: the formatter keeps the prefix of a
 * named derived SI unit (bovnar.h, BVN_UNIT_REDUCE), so 2.5 k~N and 2500 N stay
 * two documents.
 *
 * Everything this needs was already plumbed: bvnr_write_flags_t.unit_flags
 * reaches the serializer, and the serializer applies it on both the annotation
 * and the inline-unit paths. Nothing set it, so the capability existed and was
 * unreachable from the outside.
 *
 * It can FAIL where pretty-print cannot, and that is deliberate: folding a
 * prefix multiplies the value, and where the result has no exact representation
 * in the value's own base (1/100 in base 16) the writer refuses with
 * error_value_out_of_range rather than rounding. A canonical form that quietly
 * rounds is worse than none.
 */
static int cmd_pretty(const char *filename, bool canonical)
{
	int fd = open(filename, BVN_O_RDONLY);
	if (fd < 0) { perror(filename); return 1; }
	off_t sz = lseek(fd, 0, SEEK_END);
	if (sz < 0) {
		cwe("pretty-print: %s: cannot seek — not a regular file?\n",
		        filename);
		close(fd);
		return 1;
	}
	if ((uint64_t)sz > UINT32_MAX) {
		fprintf(stderr, "pretty-print: file exceeds 4 GiB limit\n");
		close(fd);
		return 1;
	}
	if (lseek(fd, 0, SEEK_SET) != 0) { perror(filename); close(fd); return 1; }
	if (sz == 0) { close(fd); return 0; }
	size_t size = (size_t)sz;
	uint8_t *buf = malloc(size);
	if (!buf) { close(fd); return 1; }
	size_t total = 0;
	while (total < size) {
		ssize_t nread = read(fd, buf + total, size - total);
		if (nread < 0) {
			if (errno == EINTR) continue;
			free(buf); close(fd); return 1;
		}
		if (nread == 0) break;
		total += (size_t)nread;
	}
	if (total != size) { free(buf); close(fd); return 1; }
	close(fd);
	bvnr_writer_t *w = bvnr_writer_create();
	if (!w) { free(buf); return 1; }
	bvnr_sink_t sink;
	bvnr_sink_to_fd(&sink, STDOUT_FILENO);
	bvnr_write_flags_t wflags = {0};
	/* The reader below is opened at the format's hard cap of 255 levels, like
	 * every other subcommand. The writer was left at its default of 64, so a
	 * document nested 65..255 deep -- one `validate`, `query`, `convert` and
	 * `events` all accept -- could be read here and not written back:
	 * "scanner_callback_failed", then struct_nesting_too_high, and no output.
	 * The two limits describe the same document and have to agree. */
	wflags.max_struct_nesting = 255;
	wflags.max_array_nesting  = 255;
	if (canonical)
		wflags.unit_flags = BVN_UNIT_REDUCE | BVN_NUM_CANONICAL;
	if (!bvnr_open_write_sink(w, &sink, true, &wflags)) {
		bvnr_writer_destroy(w); free(buf); return 1;
	}
	/* Preserve a leading "#!bovnar M.N" directive across the canonical
	 * re-serialisation (ev_stream_start fires before the directive is parsed,
	 * so peek the raw buffer and re-emit it up front). */
	{
		uint16_t vmaj = 0, vmin = 0;
		if (bvnr_peek_version(buf, (uint64_t)size, &vmaj, &vmin))
			bvnr_write_version(w, vmaj, vmin);
	}
	bvnr_reader_t *r = bvnr_reader_create();
	if (!r) { bvnr_writer_destroy(w); free(buf); return 1; }
	bvnr_source_t src;
	bvnr_source_from_mem(&src, buf, (uint64_t)size);
	pp_callback_ctx_t cb_ctx = { .writer = w };
	bvnr_read_flags_t rflags = {0};
	rflags.userdata      = &cb_ctx;
	rflags.on_verified   = on_verified_write;
	rflags.max_file_size     = UINT32_MAX;
	rflags.max_array_nesting = 255;
	rflags.max_struct_nesting = 255;
	if (!bvnr_open_read_source(r, &src, NULL, &rflags)) {
		bvnr_reader_destroy(r); bvnr_writer_destroy(w); free(buf); return 1;
	}
	bool ok = bvnr_read(r);
	error_code_t err = bvnr_reader_get_error(r);
	if (!ok) {
		/* Say which half failed. The writer runs inside the reader's callback,
		 * so a value it refuses stops the READ with scanner_callback_failed --
		 * and that was all this printed: "Pretty-print parse error:
		 * scanner_callback_failed", blaming the parse of a document that had
		 * parsed, with the real reason one line further down under "Writer
		 * finish failed". Under --canonical that is the documented outcome for
		 * an integer the rescale cannot keep integral, and it read like a
		 * broken file. */
		error_code_t werr = bvnr_writer_get_error(w);
		if (err == error_scanner_callback_failed && werr != error_none) {
			fprintf(stderr, "pretty-print: %s: cannot write the value at line %"
					PRIu64 ", col %" PRIu64 ": %s\n", filename,
					bvnr_reader_get_error_line(r),
					bvnr_reader_get_error_column(r),
					bvn_error_to_string(werr));
			if (canonical && werr == error_value_out_of_range)
				fprintf(stderr, "pretty-print: --canonical rescales a value "
						"with its unit, and this one has no exact "
						"form in its own type and base (an integer "
						"that stops being one, or no longer fits its "
						"width); it is refused rather than rounded\n");
		} else {
			fprintf(stderr, "Pretty-print parse error: %s at line %" PRIu64
					", col %" PRIu64 "\n", bvn_error_to_string(err),
					bvnr_reader_get_error_line(r),
					bvnr_reader_get_error_column(r));
		}
	}
	/* Still called after a failed read -- it flushes what was written -- but its
	 * verdict is only news when the read itself succeeded: after an error it
	 * can only repeat that the stream is incomplete. */
	if (!bvnr_write_finish(w) && ok) {
		fprintf(stderr, "Writer finish failed: %s\n",
				bvn_error_to_string(bvnr_writer_get_error(w)));
		ok = false;
	}
	bvnr_reader_destroy(r);
	bvnr_writer_destroy(w);
	free(buf);
	return ok ? 0 : 1;
}
typedef enum {
	BVN_JSON_NULL, BVN_JSON_BOOL, BVN_JSON_INT, BVN_JSON_UINT, BVN_JSON_FLOAT,
	BVN_JSON_STRING, BVN_JSON_ARRAY, BVN_JSON_OBJECT
} JsonNodeType;
typedef struct JsonNode {
	JsonNodeType type;
	union {
		bool b;
		int64_t i;
		uint64_t u;
		double f;
		char *s;
		struct { struct JsonNode **items; uint32_t count; } arr;
		struct { char **keys; struct JsonNode **values; uint32_t count; } obj;
	} u;
} JsonNode;
static void json_free_node(JsonNode *node)
{
	if (!node) return;
	switch (node->type) {
	case BVN_JSON_STRING:
		free(node->u.s);
		break;
	case BVN_JSON_ARRAY:
		for (uint32_t i = 0; i < node->u.arr.count; i++)
			json_free_node(node->u.arr.items[i]);
		free(node->u.arr.items);
		break;
	case BVN_JSON_OBJECT:
		for (uint32_t i = 0; i < node->u.obj.count; i++) {
			free(node->u.obj.keys[i]);
			json_free_node(node->u.obj.values[i]);
		}
		free(node->u.obj.keys);
		free(node->u.obj.values);
		break;
	default:
		break;
	}
	free(node);
}
static void skip_ws(const char **p)
{
	while (**p && isspace((unsigned char)**p)) (*p)++;
}
static int parse_hex4(const char *s)
{
	uint32_t v = 0;
	for (int i = 0; i < 4; i++) {
		uint32_t c = (uint8_t)s[i];
		uint32_t d;
		if      (c >= '0' && c <= '9') d = c - '0';
		else if (c >= 'a' && c <= 'f') d = c - 'a' + 10u;
		else if (c >= 'A' && c <= 'F') d = c - 'A' + 10u;
		else return -1;
		v = (v << 4) | d;
	}
	return (int)v;
}
static char *encode_utf8(char *out, uint32_t cp)
{
	if (cp <= 0x7fu) {
		*out++ = (char)(uint8_t)cp;
	} else if (cp <= 0x7ffu) {
		*out++ = (char)(uint8_t)(0xc0u | (cp >> 6));
		*out++ = (char)(uint8_t)(0x80u | (cp & 0x3fu));
	} else if (cp <= 0xffffu) {
		*out++ = (char)(uint8_t)(0xe0u | (cp >> 12));
		*out++ = (char)(uint8_t)(0x80u | ((cp >> 6) & 0x3fu));
		*out++ = (char)(uint8_t)(0x80u | (cp & 0x3fu));
	} else if (cp <= 0x10ffffu) {
		*out++ = (char)(uint8_t)(0xf0u | (cp >> 18));
		*out++ = (char)(uint8_t)(0x80u | ((cp >> 12) & 0x3fu));
		*out++ = (char)(uint8_t)(0x80u | ((cp >> 6)  & 0x3fu));
		*out++ = (char)(uint8_t)(0x80u | (cp & 0x3fu));
	}
	return out;
}
static char *parse_json_string(const char **p)
{
	if (**p != '"') return NULL;
	(*p)++;
	const char *start = *p;
	size_t raw_len = 0;
	while (**p && **p != '"') {
		if (**p == '\\') {
			(*p)++;
			if (!**p) return NULL;
			if (**p == 'u') {
				if (!(*p)[1] || !(*p)[2] || !(*p)[3] || !(*p)[4])
					return NULL;
				(*p) += 4;
			}
		}
		(*p)++;
		raw_len++;
	}
	if (**p != '"') return NULL;
	const char *end = *p;
	/* Each input unit decodes to at most 4 UTF-8 bytes; guard the size_t
	 * multiply against overflow on a 32-bit size_t. Always false on 64-bit. */
	if (raw_len > (SIZE_MAX - 1u) / 4u) return NULL;
	char *str = malloc(raw_len * 4u + 1u);
	if (!str) return NULL;
	char *out = str;
	const char *in = start;
	while (in < end) {
		if (*in == '\\') {
			in++;
			if (in >= end) { free(str); return NULL; }
			switch (*in) {
			case '"':  *out++ = '"';  break;
			case '\\': *out++ = '\\'; break;
			case '/':  *out++ = '/';  break;
			case 'b':  *out++ = '\b'; break;
			case 'f':  *out++ = '\f'; break;
			case 'n':  *out++ = '\n'; break;
			case 'r':  *out++ = '\r'; break;
			case 't':  *out++ = '\t'; break;
			case 'u': {
				if (end - in < 5) { free(str); return NULL; }
				int hi = parse_hex4(in + 1);
				if (hi < 0) { free(str); return NULL; }
				in += 4;
				uint32_t cp = (uint32_t)hi;
				if (cp >= 0xd800u && cp <= 0xdbffu) {
					if (end - in < 7 || in[1] != '\\' || in[2] != 'u') {
						free(str); return NULL;
					}
					int lo = parse_hex4(in + 3);
					if (lo < 0 || (uint32_t)lo < 0xdc00u ||
					    (uint32_t)lo > 0xdfffu) {
						free(str); return NULL;
					}
					cp = 0x10000u + (((cp - 0xd800u) << 10) |
					     ((uint32_t)lo - 0xdc00u));
					in += 6;
				} else if (cp >= 0xdc00u && cp <= 0xdfffu) {
					/* A low surrogate with no preceding high
					 * surrogate is not a valid scalar value;
					 * encoding it would emit invalid UTF-8. */
					free(str); return NULL;
				}
				if (cp == 0) {
					/* bovnar forbids the NUL byte in strings.
					 * Encoding it here would also truncate this
					 * NUL-terminated buffer, silently dropping the
					 * rest of the string, so reject it as a hard
					 * error like any other forbidden control byte. */
					fprintf(stderr, "convert: JSON string contains a "
						"NUL (\\u0000), which bovnar cannot "
						"represent in a string\n");
					free(str); return NULL;
				}
				out = encode_utf8(out, cp);
				break;
			}
			default:   free(str); return NULL;
			}
		} else if ((uint8_t)*in < 0x20) {
			/* Unescaped control characters are invalid in JSON strings. */
			free(str); return NULL;
		} else {
			*out++ = *in;
		}
		in++;
	}
	*out = '\0';
	(*p)++;
	return str;
}
#define JSON_MAX_DEPTH 512
/*
 * The object key the converter is currently under, for diagnostics only.
 *
 * A JSON value the converter has to refuse -- a number that overflows, a string
 * holding a byte bovnar has no string for -- is found several calls below the
 * place that knows its key, and "Bovnar writer error: unexpected_input_byte"
 * sent the user looking through the whole input for it. An element of an array
 * is reported under the key of the member that holds the array, which is as
 * close as a key can get. NULL at the top level.
 */
static const char *json_cur_key = NULL;
static const char *json_key_for_msg(void)
{
	return json_cur_key ? json_cur_key : "(top level)";
}
static JsonNode *json_parse_value(const char **p);
static JsonNode *json_parse_value_depth(const char **p, int depth);
static JsonNode *json_parse_array(const char **p, int depth)
{
	(*p)++;
	JsonNode *node = calloc(1, sizeof(*node));
	if (!node) return NULL;
	node->type = BVN_JSON_ARRAY;
	skip_ws(p);
	if (**p == ']') { (*p)++; return node; }
	uint32_t cap = 8;
	node->u.arr.items = malloc(cap * sizeof(*node->u.arr.items));
	if (!node->u.arr.items) { free(node); return NULL; }
	while (**p) {
		JsonNode *elem = json_parse_value_depth(p, depth);
		if (!elem) { json_free_node(node); return NULL; }
		if (node->u.arr.count == cap) {
			cap *= 2;
			JsonNode **tmp = realloc(node->u.arr.items,
									 cap * sizeof(*node->u.arr.items));
			if (!tmp) { json_free_node(elem); json_free_node(node); return NULL; }
			node->u.arr.items = tmp;
		}
		node->u.arr.items[node->u.arr.count++] = elem;
		skip_ws(p);
		if (**p == ',') { (*p)++; skip_ws(p); continue; }
		if (**p == ']') { (*p)++; break; }
		json_free_node(node); return NULL;
	}
	return node;
}
static JsonNode *json_parse_object(const char **p, int depth)
{
	(*p)++;
	JsonNode *node = calloc(1, sizeof(*node));
	if (!node) return NULL;
	node->type = BVN_JSON_OBJECT;
	skip_ws(p);
	if (**p == '}') { (*p)++; return node; }
	uint32_t cap = 8;
	node->u.obj.keys   = malloc(cap * sizeof(*node->u.obj.keys));
	node->u.obj.values = malloc(cap * sizeof(*node->u.obj.values));
	if (!node->u.obj.keys || !node->u.obj.values) { json_free_node(node); return NULL; }
	while (**p) {
		char *key = parse_json_string(p);
		if (!key) { json_free_node(node); return NULL; }
		skip_ws(p);
		if (**p != ':') { free(key); json_free_node(node); return NULL; }
		(*p)++;
		skip_ws(p);
		const char *outer_key = json_cur_key;
		json_cur_key = key;
		JsonNode *val = json_parse_value_depth(p, depth);
		json_cur_key = outer_key;
		if (!val) { free(key); json_free_node(node); return NULL; }
		if (node->u.obj.count == cap) {
			uint32_t nc = cap * 2;
			char     **tk = malloc(nc * sizeof(*node->u.obj.keys));
			JsonNode **tv = malloc(nc * sizeof(*node->u.obj.values));
			if (!tk || !tv) {
				free(tk); free(tv);
				free(key); json_free_node(val); json_free_node(node); return NULL;
			}
			memcpy(tk, node->u.obj.keys,   cap * sizeof(*node->u.obj.keys));
			memcpy(tv, node->u.obj.values, cap * sizeof(*node->u.obj.values));
			free(node->u.obj.keys);
			free(node->u.obj.values);
			node->u.obj.keys   = tk;
			node->u.obj.values = tv;
			cap = nc;
		}
		node->u.obj.keys[node->u.obj.count]   = key;
		node->u.obj.values[node->u.obj.count] = val;
		node->u.obj.count++;
		skip_ws(p);
		if (**p == ',') { (*p)++; skip_ws(p); continue; }
		if (**p == '}') { (*p)++; break; }
		json_free_node(node); return NULL;
	}
	return node;
}
/*
 * Validate and span a JSON number per the RFC 8259 grammar:
 *   number = [ '-' ] int [ frac ] [ exp ]
 *   int    = '0' | [1-9] digit*       (no leading zeros)
 *   frac   = '.' digit+
 *   exp    = ('e'|'E') ['+'|'-'] digit+
 * On success advances *endp past the number and reports whether it is a
 * floating-point literal. Returns false for malformed numbers (bare '-',
 * leading zeros, missing fraction/exponent digits, etc.).
 */
static bool scan_json_number(const char *s, const char **endp, bool *is_float)
{
	const char *p = s;
	bool isf = false;
	if (*p == '-') p++;
	if (*p == '0') {
		p++;
	} else if (*p >= '1' && *p <= '9') {
		while (isdigit((unsigned char)*p)) p++;
	} else {
		return false;
	}
	if (*p == '.') {
		isf = true;
		p++;
		if (!isdigit((unsigned char)*p)) return false;
		while (isdigit((unsigned char)*p)) p++;
	}
	if (*p == 'e' || *p == 'E') {
		isf = true;
		p++;
		if (*p == '+' || *p == '-') p++;
		if (!isdigit((unsigned char)*p)) return false;
		while (isdigit((unsigned char)*p)) p++;
	}
	*endp = p;
	*is_float = isf;
	return true;
}
static JsonNode *json_parse_value_depth(const char **p, int depth)
{
	skip_ws(p);
	if (!**p) return NULL;
	if (depth > JSON_MAX_DEPTH) {
		fprintf(stderr, "convert: JSON nesting exceeds limit (%d)\n",
			JSON_MAX_DEPTH);
		return NULL;
	}
	if (**p == '"') {
		char *s = parse_json_string(p);
		if (!s) return NULL;
		JsonNode *n = calloc(1, sizeof(*n));
		if (!n) { free(s); return NULL; }
		n->type = BVN_JSON_STRING;
		n->u.s = s;
		return n;
	}
	if (**p == '[') return json_parse_array(p, depth + 1);
	if (**p == '{') return json_parse_object(p, depth + 1);
	if (strncmp(*p, "null",  4) == 0) {
		*p += 4;
		JsonNode *n = calloc(1, sizeof(*n));
		if (!n) return NULL;
		n->type = BVN_JSON_NULL;
		return n;
	}
	if (strncmp(*p, "true",  4) == 0) {
		*p += 4;
		JsonNode *n = calloc(1, sizeof(*n));
		if (!n) return NULL;
		n->type = BVN_JSON_BOOL; n->u.b = true;
		return n;
	}
	if (strncmp(*p, "false", 5) == 0) {
		*p += 5;
		JsonNode *n = calloc(1, sizeof(*n));
		if (!n) return NULL;
		n->type = BVN_JSON_BOOL; n->u.b = false;
		return n;
	}
	if (**p == '-' || isdigit((unsigned char)**p)) {
		const char *num_end;
		bool is_float = false;
		if (!scan_json_number(*p, &num_end, &is_float)) {
			fprintf(stderr, "convert: malformed JSON number\n");
			return NULL;
		}
		JsonNode *n = calloc(1, sizeof(*n));
		if (!n) return NULL;
		char *end;
		errno = 0;
		if (is_float) {
			n->type = BVN_JSON_FLOAT;
			n->u.f = strtod(*p, &end);
			/* JSON has no infinity, so a number token is finite by
			 * definition -- and 1e999 was read by strtod as HUGE_VAL and
			 * written as `inf`, a different value, with exit status 0. (Going
			 * back, that inf becomes JSON null.) The integer branches below
			 * already refuse what does not fit; this one did not check.
			 * Only overflow: a literal too small to tell from zero, and one
			 * with more digits than a binary64 holds, are rounding -- what
			 * reading a decimal into a double always is. */
			if (isinf(n->u.f)) {
				fprintf(stderr, "convert: the JSON number under key '%s' "
					"is too large for a 64-bit float (it would become "
					"infinity)\n", json_key_for_msg());
				free(n);
				return NULL;
			}
		} else if (**p == '-') {
			n->type = BVN_JSON_INT;
			n->u.i = strtoll(*p, &end, 10);
			if (errno == ERANGE) {
				fprintf(stderr, "convert: integer literal out of "
					"range for 64-bit signed\n");
				free(n);
				return NULL;
			}
		} else {
			n->type = BVN_JSON_UINT;
			n->u.u = strtoull(*p, &end, 10);
			if (errno == ERANGE) {
				fprintf(stderr, "convert: integer literal out of "
					"range for 64-bit unsigned\n");
				free(n);
				return NULL;
			}
		}
		*p = num_end;
		return n;
	}
	return NULL;
}
static JsonNode *json_parse_value(const char **p)
{
	return json_parse_value_depth(p, 0);
}
static bool write_bvn_value(bvnr_writer_t *w, const char *key, const JsonNode *node);
/*
 * A bovnar key is a lexer identifier: it starts with a letter, '_', or a UTF-8
 * leader byte (0xC2-0xF4), and continues with letters, digits, '_', '+', '-',
 * or UTF-8 bytes. JSON object keys are arbitrary strings, so reject the ones
 * bovnar cannot represent (spaces, leading digits, punctuation, empty) up front
 * with a clear message rather than letting the writer fail opaquely.
 */
static bool json_key_is_valid_bvnr_id(const char *key)
{
	if (!key || !*key) return false;
	const unsigned char *p = (const unsigned char *)key;
	unsigned char c0 = p[0];
	/* bovnar rejects byte 0xC2 (the UTF-8 leader for U+0080-U+00BF) at both
	 * identifier start and body, so the valid multibyte leaders are 0xC3-0xF4. */
	bool start_ok = (c0 >= 'A' && c0 <= 'Z') || (c0 >= 'a' && c0 <= 'z')
	              || c0 == '_' || (c0 >= 0xC3 && c0 <= 0xF4);
	if (!start_ok) return false;
	for (size_t i = 1; p[i]; i++) {
		unsigned char c = p[i];
		bool ok = (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z')
		        || (c >= '0' && c <= '9') || c == '_' || c == '+'
		        || c == '-' || (c >= 0x80 && c != 0xC2);
		if (!ok) return false;
	}
	return true;
}
/*
 * A JSON array/object maps onto bovnar's general container model: a JSON array
 * becomes a bracket array whose elements are emitted in place (scalars as data
 * events, nested arrays as nested rows, objects as structs), and a JSON object
 * becomes a struct. This is a 1:1 structural mapping — it preserves jagged
 * nesting, arrays of objects, and single-row matrices that the older '/'-based
 * 2-D encoding could not. These three helpers are mutually recursive.
 */
static bool write_bvn_element(bvnr_writer_t *w, const JsonNode *e);
static bool write_bvn_array_body(bvnr_writer_t *w, const JsonNode *node);
static bool write_bvn_struct_body(bvnr_writer_t *w, const JsonNode *node);
/* Emit one scalar array element as an ev_data event. */
/* Does this rendered number text already look like a float to bovnar's default
 * type synthesis? Only a '.' or an exponent marker makes it one. */
static bool json_number_text_is_float(const char *t, uint32_t n)
{
	for (uint32_t i = 0; i < n; i++)
		if (t[i] == '.' || t[i] == 'e' || t[i] == 'E') return true;
	return false;
}
/*
 * A JSON string may hold any code point; a bovnar string may not hold the C0
 * controls outside \t \n \v \f \r, nor DEL (the same set the writer's
 * bvn_validate_string_content refuses). The writer does refuse them, but as
 * "Bovnar writer error: unexpected_input_byte" -- no key, no byte, and nothing
 * to say the JSON was fine and the limit is the target format's. Check first
 * and say all three.
 */
static bool json_string_fits_bvnr(const char *str)
{
	for (const unsigned char *c = (const unsigned char *)str; *c; c++) {
		if (*c <= 0x08 || (*c >= 0x0E && *c <= 0x1F) || *c == 0x7F) {
			fprintf(stderr, "convert: the JSON string under key '%s' "
				"contains the control character U+%04X, which a "
				"bovnar string cannot carry\n",
				json_key_for_msg(), (unsigned)*c);
			return false;
		}
	}
	if (!bvn_validate_string((const uint8_t *)str, strlen(str))) {
		fprintf(stderr, "convert: the JSON string under key '%s' is not "
			"valid UTF-8\n", json_key_for_msg());
		return false;
	}
	return true;
}
static bool write_bvn_array_elem(bvnr_writer_t *w, const JsonNode *e)
{
	bvnr_data_t d = {0};
	char nbuf[64];
	int n;
	if (!e || e->type == BVN_JSON_NULL) {
		d.type = token_is_null_value;
	} else if (e->type == BVN_JSON_BOOL) {
		const char *sym = e->u.b ? "true" : "false";
		d.type   = token_is_bool;
		d.data   = sym;
		d.length = (uint32_t)strlen(sym);
	} else if (e->type == BVN_JSON_UINT) {
		n = snprintf(nbuf, sizeof(nbuf), "%" PRIu64, e->u.u);
		d.type   = token_is_array_number;
		d.data   = nbuf;
		d.length = (n > 0) ? (uint32_t)n : 1u;
	} else if (e->type == BVN_JSON_INT) {
		n = snprintf(nbuf, sizeof(nbuf), "%" PRId64, e->u.i);
		d.type   = token_is_array_number;
		d.data   = nbuf;
		d.length = (n > 0) ? (uint32_t)n : 1u;
	} else if (e->type == BVN_JSON_FLOAT) {
		n = snprintf(nbuf, sizeof(nbuf), "%.17g", e->u.f);
		d.type   = token_is_array_number;
		d.data   = nbuf;
		d.length = (n > 0) ? (uint32_t)n : 1u;
		/* "%.17g" renders 2.0 as "2", which bovnar's default type synthesis then
		 * reads back as a uint -- so the same JSON value came out float at the
		 * top level (bvnr_write_float keeps the family) and uint inside an
		 * array, in one document. An explicit float annotation on the element
		 * keeps the two paths agreeing. */
		d.value_type.family = vt_float;
		if (!json_number_text_is_float(nbuf, (uint32_t)((n > 0) ? n : 1))) {
			bvnr_data_t ann = {0};
			ann.value_type.family = vt_float;
			if (!bvnr_write_event(w, ev_type_annotation_start, &ann))
				return false;
			if (!bvnr_write_event(w, ev_type_annotation_type_family, &ann))
				return false;
			if (!bvnr_write_event(w, ev_type_annotation_end, &ann))
				return false;
		}
	} else if (e->type == BVN_JSON_STRING) {
		if (!json_string_fits_bvnr(e->u.s)) return false;
		d.type   = token_is_array_string;
		d.data   = e->u.s;
		d.length = (uint32_t)strlen(e->u.s);
	} else {
		return false;
	}
	return bvnr_write_event(w, ev_data, &d);
}
/* Emit a JSON object's members as a bovnar struct body (no leading key). */
static bool write_bvn_struct_body(bvnr_writer_t *w, const JsonNode *node)
{
	bvnr_data_t d = {0};
	if (!bvnr_write_event(w, ev_struct_start, &d)) return false;
	for (uint32_t i = 0; i < node->u.obj.count; i++)
		if (!write_bvn_value(w, node->u.obj.keys[i], node->u.obj.values[i]))
			return false;
	bvnr_data_t e = {0};
	return bvnr_write_event(w, ev_struct_end, &e);
}
/* Emit a JSON array's elements as a bovnar bracket-array body (no leading key).
 * The row-type token is only a hint and is ignored for heterogeneous/nested
 * content, so a single number row start is fine; each element carries its own
 * type. */
static bool write_bvn_array_body(bvnr_writer_t *w, const JsonNode *node)
{
	bvnr_data_t d = {0};
	d.type = token_is_array_number;
	if (!bvnr_write_event(w, ev_array_row_start, &d)) return false;
	for (uint32_t i = 0; i < node->u.arr.count; i++)
		if (!write_bvn_element(w, node->u.arr.items[i])) return false;
	bvnr_data_t e = {0};
	return bvnr_write_event(w, ev_array_row_end, &e);
}
/* Emit one array element (no key), dispatching scalars to a data event and
 * containers to a nested array/struct body. */
static bool write_bvn_element(bvnr_writer_t *w, const JsonNode *e)
{
	if (e && e->type == BVN_JSON_ARRAY)  return write_bvn_array_body(w, e);
	if (e && e->type == BVN_JSON_OBJECT) return write_bvn_struct_body(w, e);
	return write_bvn_array_elem(w, e);
}
static bool write_bvn_root(bvnr_writer_t *w, const JsonNode *root)
{
	if (!root || root->type != BVN_JSON_OBJECT) {
		fprintf(stderr, "JSON root must be an object\n");
		return false;
	}
	for (uint32_t i = 0; i < root->u.obj.count; i++) {
		if (!write_bvn_value(w, root->u.obj.keys[i], root->u.obj.values[i]))
			return false;
	}
	return true;
}
static bool write_bvn_value(bvnr_writer_t *w, const char *key, const JsonNode *node)
{
	if (!json_key_is_valid_bvnr_id(key)) {
		fprintf(stderr, "convert: JSON key '%s' is not a valid bovnar "
			"identifier (must start with a letter or '_' and contain "
			"only letters, digits, '_', '+', '-')\n", key);
		return false;
	}
	if (!node) return bvnr_write_null(w, key);
	/* For the diagnostics below this point (see json_cur_key). Restored on the
	 * way out so a sibling written after a nested object is reported under
	 * its own parent again. */
	const char *outer_key = json_cur_key;
	bool ok = false;
	json_cur_key = key;
	switch (node->type) {
	case BVN_JSON_NULL:   ok = bvnr_write_null(w, key); break;
	case BVN_JSON_BOOL:   ok = bvnr_write_bool(w, key, node->u.b); break;
	case BVN_JSON_UINT:   ok = bvnr_write_uint(w, key, 0, node->u.u); break;
	case BVN_JSON_INT:    ok = bvnr_write_sint(w, key, 0, node->u.i); break;
	case BVN_JSON_FLOAT:  ok = bvnr_write_float(w, key, 0, node->u.f); break;
	case BVN_JSON_STRING:
		ok = json_string_fits_bvnr(node->u.s) &&
		     bvnr_write_string(w, key, node->u.s);
		break;
	case BVN_JSON_ARRAY: {
		bvnr_data_t d = {0};
		d.type   = token_is_identifier;
		d.data   = key;
		d.length = (uint32_t)strlen(key);
		ok = bvnr_write_event(w, ev_assignment_start, &d) &&
		     write_bvn_array_body(w, node);
		break;
	}
	case BVN_JSON_OBJECT: {
		ok = bvnr_write_struct_start(w, key);
		for (uint32_t i = 0; ok && i < node->u.obj.count; i++)
			ok = write_bvn_value(w, node->u.obj.keys[i],
					     node->u.obj.values[i]);
		ok = ok && bvnr_write_struct_end(w);
		break;
	}
	}
	json_cur_key = outer_key;
	return ok;
}
static int cmd_convert_json_to_bvnr(const char *file)
{
	int fd = open(file, BVN_O_RDONLY);
	if (fd < 0) { perror(file); return 1; }
	off_t sz = lseek(fd, 0, SEEK_END);
	if (sz < 0) {
		cwe("convert: %s: cannot seek — not a regular file?\n", file);
		close(fd); return 1;
	}
	if (lseek(fd, 0, SEEK_SET) != 0) { perror(file); close(fd); return 1; }
	/* Reject >= UINT32_MAX (not just >) so that size + 1 below cannot wrap to 0
	 * on a 32-bit size_t and hand malloc a zero-byte buffer the read loop then
	 * overflows. A no-op on the 64-bit primary target. */
	if ((uint64_t)sz >= UINT32_MAX) {
		fprintf(stderr, "convert: file exceeds 4 GiB limit\n");
		close(fd); return 1;
	}
	size_t size = (size_t)sz;
	char *buf = malloc(size + 1);
	if (!buf) { close(fd); return 1; }
	size_t total = 0;
	while (total < size) {
		ssize_t nread = read(fd, buf + total, size - total);
		if (nread < 0) {
			if (errno == EINTR) continue;
			free(buf); close(fd); return 1;
		}
		if (nread == 0) break;
		total += (size_t)nread;
	}
	if (total != size) { free(buf); close(fd); return 1; }
	close(fd);
	buf[size] = '\0';
	const char *p = buf;
	JsonNode *root = json_parse_value(&p);
	if (!root) {
		fprintf(stderr, "Failed to parse JSON\n");
		free(buf);
		return 1;
	}
	skip_ws(&p);
	/* Measured against the FILE's length, not against the first NUL. The
	 * buffer is NUL-terminated for the parser's benefit, so `*p != 0` could
	 * not see anything behind a NUL byte in the file itself: `{"a":1}` followed
	 * by a NUL and any amount of anything converted cleanly, and whatever came
	 * after was dropped without a word. */
	if ((size_t)(p - buf) != size) {
		fprintf(stderr, "convert: trailing content after JSON value%s\n",
			*p == '\0' ? " (a NUL byte)" : "");
		json_free_node(root);
		free(buf);
		return 1;
	}
	/*
	 * Render to a memory buffer first so the result can be validated for array
	 * homogeneity (spec 1.0) before it is emitted. JSON permits heterogeneous
	 * and ragged arrays, which have no bovnar representation; emitting them
	 * would produce a .bvnr file that bovnar itself rejects, so we hard-error
	 * instead (no silent lossy output). Re-using bvn_dom_parse keeps the
	 * converter's notion of "valid" identical to the library's.
	 */
	/* Guard the size_t multiply before it reaches malloc: on a 32-bit size_t
	 * a multi-hundred-MB input would wrap to a small allocation while the sink
	 * is handed the untruncated 64-bit cap. Always false on a 64-bit build. */
	if (size > (SIZE_MAX - 65536u) / 8u) {
		fprintf(stderr, "convert: input too large\n");
		json_free_node(root); free(buf); return 1;
	}
	uint64_t out_cap = (uint64_t)size * 8u + 65536u;
	char *out = malloc(out_cap);
	if (!out) { json_free_node(root); free(buf); return 1; }
	bvnr_writer_t *w = bvnr_writer_create();
	if (!w) { free(out); json_free_node(root); free(buf); return 1; }
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, out, out_cap);
	bvnr_write_flags_t wflags = {0};
	/* As in cmd_pretty: the format nests to 255 and bvn_dom_parse, which checks
	 * the result below, reads to 255. At the writer's default of 64 a JSON
	 * document 65 levels deep was refused as array_nesting_too_high although
	 * its bovnar form is perfectly legal. */
	wflags.max_struct_nesting = 255;
	wflags.max_array_nesting  = 255;
	if (!bvnr_open_write_sink(w, &sink, true, &wflags)) {
		bvnr_writer_destroy(w); free(out);
		json_free_node(root); free(buf); return 1;
	}
	bool ok = write_bvn_root(w, root);
	if (ok) ok = bvnr_write_finish(w);
	if (!ok) {
		/* Only surface a writer-level error here; structural problems
		 * (bad keys, unrepresentable nesting) already printed their own
		 * diagnostic and leave the writer error at error_none. */
		error_code_t werr = bvnr_writer_get_error(w);
		if (werr != error_none)
			fprintf(stderr, "Bovnar writer error: %s\n",
				bvn_error_to_string(werr));
		bvnr_writer_destroy(w); free(out);
		json_free_node(root); free(buf); return 1;
	}
	uint64_t out_len = bvnr_writer_bytes_written(w);
	bvnr_writer_destroy(w);
	if (out_len > (uint64_t)UINT32_MAX) {
		/* bvn_dom_parse caps at 4 GiB, so we cannot re-validate a larger
		 * output. Refuse rather than emit a document that skipped the
		 * homogeneity / unique-key check the converter contract promises. */
		fprintf(stderr, "convert: output too large to validate "
			"(%" PRIu64 " bytes exceeds the 4 GiB limit)\n", out_len);
		free(out); json_free_node(root); free(buf); return 1;
	}
	{
		bvn_dom_doc_t *chk = bvn_dom_parse(out, (uint32_t)out_len);
		if (!chk) {
			/* Out of memory re-parsing our own output: do NOT fall through
			 * and emit it, or an unvalidated (possibly non-homogeneous)
			 * document would be written as if it had passed the check. */
			fprintf(stderr, "convert: out of memory validating output\n");
			free(out); json_free_node(root); free(buf); return 1;
		}
		error_code_t herr = bvn_dom_doc_get_parse_error(chk);
		bvn_dom_doc_destroy(chk);
		if (herr != error_none) {
			const char *hint;
			switch (herr) {
			case error_duplicate_struct_key:
				hint = "a JSON object with a duplicate key — bovnar keys "
					"must be unique within a scope";
				break;
			case error_struct_shape_mismatch:
				hint = "an array of objects with differing key sets — "
					"bovnar array elements must share one shape; model "
					"mixed data as a struct";
				break;
			default:
				hint = "a heterogeneous or ragged array — bovnar 1.0 "
					"arrays are homogeneous; model mixed data as a struct";
				break;
			}
			cwe("convert: the JSON has no bovnar "
				"representation: %s (%s)\n",
				bvn_error_to_string(herr), hint);
			free(out); json_free_node(root); free(buf); return 1;
		}
	}
	fwrite(out, 1, (size_t)out_len, stdout);
	free(out);
	json_free_node(root);
	free(buf);
	return 0;
}
static void print_json_node(const bvn_dom_node_t *node, int indent, bool pretty);
static void print_json_indent(int level, bool pretty)
{
	if (!pretty) return;
	for (int i = 0; i < level; i++) fputs("  ", stdout);
}
static void print_json_string_escaped(const char *s, uint32_t len)
{
	putchar('"');
	for (uint32_t i = 0; i < len; i++) {
		uint8_t c = (uint8_t)s[i];
		switch (c) {
		case '"':  fputs("\\\"", stdout); break;
		case '\\': fputs("\\\\", stdout); break;
		case '\b': fputs("\\b",  stdout); break;
		case '\f': fputs("\\f",  stdout); break;
		case '\n': fputs("\\n",  stdout); break;
		case '\r': fputs("\\r",  stdout); break;
		case '\t': fputs("\\t",  stdout); break;
		default:
			if (c < 0x20)
				printf("\\u%04x", c);
			else
				putchar(c);
		}
	}
	putchar('"');
}
/*
 * Emit a double as the shortest decimal string that parses back to the exact
 * same IEEE-754 value, so json -> bvnr -> json stays value-exact without the
 * noise of a fixed 17-significant-digit rendering (0.1 prints as "0.1", not
 * "0.10000000000000001"). We cannot reproduce the original JSON literal's
 * spelling — that is lost when the number is parsed to a double, so "2.0" comes
 * back as "2" and "1e10" as "1e+10" — but the value is preserved exactly. NaN
 * and infinities have no JSON form and are emitted as null.
 */
static void print_json_double(double v)
{
	if (!isfinite(v)) { fputs("null", stdout); return; }
	char buf[64];
	bvn_cli_format_double(buf, sizeof buf, v);
	fputs(buf, stdout);
}
static void print_json_node(const bvn_dom_node_t *node, int indent, bool pretty)
{
	if (!node) { fputs("null", stdout); return; }
	switch (bvn_dom_node_type(node)) {
	case BVN_DOM_NULL:
		fputs("null", stdout);
		break;
	case BVN_DOM_INT: {
		value_type_spec_t vt = bvn_dom_get_value_type(node);
		if (vt.width > 64u) {
			char *s = bvn_dom_int_to_str(node, 10u);
			if (s) {
				putchar('"');
				fputs(s, stdout);
				putchar('"');
				free(s);
			} else {
				fputs("null", stdout);
			}
		} else if (vt.family == vt_uint) {
			uint64_t v = 0;
			bvn_dom_get_u64(node, &v);
			printf("%" PRIu64, v);
		} else {
			int64_t v = 0;
			bvn_dom_get_i64(node, &v);
			printf("%" PRId64, v);
		}
		break;
	}
	case BVN_DOM_FLOAT: {
		double v;
		bvn_dom_get_float(node, &v);
		print_json_double(v);
		break;
	}
	case BVN_DOM_BOOL: {
		bool b = false;
		bvn_dom_get_bool(node, &b);
		fputs(b ? "true" : "false", stdout);
		break;
	}
	case BVN_DOM_STRING: {
		const char *s; uint32_t l;
		bvn_dom_get_string(node, &s, &l);
		print_json_string_escaped(s, l);
		break;
	}
	case BVN_DOM_SYMBOL:
	case BVN_DOM_REFERENCE: {
		const char *s = NULL; uint32_t l = 0;
		bvn_dom_get_symbol(node, &s, &l);
		if (!s) bvn_dom_get_reference(node, &s, &l);
		if (s) print_json_string_escaped(s, l);
		else   fputs("null", stdout);
		break;
	}
	case BVN_DOM_OCTET_STREAM: {
		const uint8_t *b; uint32_t l;
		bvn_dom_get_octets(node, &b, &l);
		putchar('"');
		for (uint32_t i = 0; i < l; i++) printf("%02x", b[i]);
		putchar('"');
		break;
	}
	case BVN_DOM_STRUCT: {
		uint32_t cnt = bvn_dom_struct_count(node);
		const bvn_dom_entry_t *e = bvn_dom_struct_entries(node);
		if (!e) cnt = 0;   /* defensive: match print_dom_node's NULL-entries guard */
		putchar('{');
		if (cnt && pretty) putchar('\n');
		for (uint32_t i = 0; i < cnt; i++) {
			print_json_indent(indent + 1, pretty);
			print_json_string_escaped(e[i].key, (uint32_t)strlen(e[i].key));
			putchar(':');
			if (pretty) putchar(' ');
			print_json_node(e[i].value, indent + 1, pretty);
			if (i + 1 < cnt) putchar(',');
			if (pretty) putchar('\n');
		}
		if (cnt && pretty) print_json_indent(indent, pretty);
		putchar('}');
		break;
	}
	case BVN_DOM_ARRAY: {
		uint32_t cnt  = bvn_dom_array_count(node);
		uint32_t dims = bvn_dom_array_dims(node);
		if (dims > 1 && cnt % dims == 0) {
			/* Multi-row bovnar array -> nested JSON arrays (one inner
			 * array per row) so the dimensionality survives. */
			uint32_t row_len = cnt / dims;
			putchar('[');
			for (uint32_t r = 0; r < dims; r++) {
				if (r) { putchar(','); if (pretty) putchar(' '); }
				putchar('[');
				for (uint32_t c = 0; c < row_len; c++) {
					if (c) { putchar(','); if (pretty) putchar(' '); }
					print_json_node(
						bvn_dom_array_at(node, r * row_len + c),
						indent, pretty);
				}
				putchar(']');
			}
			putchar(']');
			break;
		}
		putchar('[');
		for (uint32_t i = 0; i < cnt; i++) {
			if (i) { putchar(','); if (pretty) putchar(' '); }
			print_json_node(bvn_dom_array_at(node, i), indent, pretty);
		}
		putchar(']');
		break;
	}
	}
}
/* spec 1.1: a datetime written with sub-second digits (e.g. ...:00.123Z) stores
 * the whole-second epoch count as its carrier plus the verbatim fraction. JSON
 * has only the integer carrier to offer, so emitting it would silently drop the
 * fraction — a value change, not just metadata loss. Per the converter's
 * "anything it cannot represent is a hard error" contract, scan for any such
 * value up front and refuse before writing partial output. Returns the first
 * offending node, or NULL if the subtree has none. */
static const bvn_dom_node_t *find_fractional_datetime(const bvn_dom_node_t *node)
{
	if (!node) return NULL;
	switch (bvn_dom_node_type(node)) {
	case BVN_DOM_INT:
		return bvn_dom_get_datetime_fraction(node, NULL) ? node : NULL;
	case BVN_DOM_STRUCT: {
		uint32_t cnt = bvn_dom_struct_count(node);
		const bvn_dom_entry_t *e = bvn_dom_struct_entries(node);
		for (uint32_t i = 0; i < cnt; i++) {
			const bvn_dom_node_t *hit = find_fractional_datetime(e[i].value);
			if (hit) return hit;
		}
		return NULL;
	}
	case BVN_DOM_ARRAY: {
		uint32_t cnt = bvn_dom_array_count(node);
		for (uint32_t i = 0; i < cnt; i++) {
			const bvn_dom_node_t *hit =
				find_fractional_datetime(bvn_dom_array_at(node, i));
			if (hit) return hit;
		}
		return NULL;
	}
	default:
		return NULL;
	}
}

/*
 * JSON cannot carry most of what makes a bovnar document what it is. The
 * converter used to drop all of it silently and exit 0 — while going out of its
 * way to hard-error on a sub-second datetime, which is the same class of loss.
 * Rather than refuse (that would make `convert` useless for any document with a
 * unit, the format's whole point) it now reports precisely what it dropped and
 * exits non-zero, so a script cannot mistake the output for a round-trip.
 */
typedef struct {
	uint32_t n_unit, n_symbol, n_reference, n_octet, n_wide, n_typed, n_dims;
	/* Three the first survey missed, each of which exited 0 over a value that
	 * does not come back: a non-finite float (JSON has none, so it is written
	 * as null and returns as a null), a float declared wider or more exact
	 * than the binary64 a JSON number is read into, and a datetime, which
	 * leaves as its bare epoch-second count and returns as an integer with no
	 * epoch and no type. */
	uint32_t n_nonfinite, n_float, n_datetime;
	char     ex_unit[160], ex_symbol[128], ex_reference[128];
	char     ex_octet[128], ex_wide[128], ex_typed[160], ex_dims[160];
	char     ex_nonfinite[128], ex_float[160], ex_datetime[160];
} json_loss_t;

static void json_loss_note(char *dst, size_t cap, const char *path,
			   const char *detail)
{
	if (dst[0]) return;                     /* keep the first example only */
	if (detail) snprintf(dst, cap, "%s (%s)", path, detail);
	else        snprintf(dst, cap, "%s", path);
}

static void json_loss_scan(const bvn_dom_node_t *node, const char *path,
			   json_loss_t *L)
{
	if (!node) return;
	value_unit_t vu = bvn_dom_get_unit(node);
	/* BVN_UNIT_IS_NO_UNIT lives in an internal header; spell the same test out.
	 * A single bu_none component is the dimensionless unit, not a real one. */
	bool has_unit = vu.num_components > 0 &&
			!(vu.num_components == 1u &&
			  vu.components[0].base == bu_none);
	if (has_unit) {
		/* Sized from BVNR_UNIT_STRING_MAX so a long-but-legal unit is NAMED in
		 * the loss report rather than degraded to "a unit". The -1 guard stays:
		 * the function writes no NUL on failure. */
		char ub[BVNR_UNIT_STRING_MAX];
		if (bvn_unit_to_string(vu, ub, sizeof ub) < 0) ub[0] = '\0';
		L->n_unit++;
		json_loss_note(L->ex_unit, sizeof L->ex_unit, path, ub[0] ? ub : "a unit");
	}
	switch (bvn_dom_node_type(node)) {
	case BVN_DOM_SYMBOL:
		L->n_symbol++;
		json_loss_note(L->ex_symbol, sizeof L->ex_symbol, path, NULL);
		break;
	case BVN_DOM_REFERENCE:
		L->n_reference++;
		json_loss_note(L->ex_reference, sizeof L->ex_reference, path, NULL);
		break;
	case BVN_DOM_OCTET_STREAM:
		L->n_octet++;
		json_loss_note(L->ex_octet, sizeof L->ex_octet, path, NULL);
		break;
	case BVN_DOM_FLOAT: {
		value_type_spec_t vt = bvn_dom_get_value_type(node);
		double v = 0.0;
		bvn_dom_get_float(node, &v);
		if (!isfinite(v)) {
			L->n_nonfinite++;
			json_loss_note(L->ex_nonfinite, sizeof L->ex_nonfinite, path,
				       isnan(v) ? "nan" : v > 0 ? "inf" : "ninf");
		} else if ((vt.family == vt_float && vt.width > 64u) ||
			   ((vt.family == vt_float_dec ||
			     vt.family == vt_float_fix) &&
			    bvn_effective_width(vt) >= 64u)) {
			/* What the TYPE promises and a binary64 cannot keep: more than
			 * 53 significand bits (a float:128, a 64-bit fixed-point), or
			 * 16+ exact decimal digits (float_dec:64 and up). A narrower
			 * one survives the trip through a double digit for digit, and
			 * a plain float:64 is a double already. */
			char d[48];
			snprintf(d, sizeof d, "<%s:%u>",
				 vt.family == vt_float ? "float" :
				 vt.family == vt_float_dec ? "float_dec" : "float_fix",
				 bvn_effective_width(vt));
			L->n_float++;
			json_loss_note(L->ex_float, sizeof L->ex_float, path, d);
		}
		break;
	}
	case BVN_DOM_INT: {
		value_type_spec_t vt = bvn_dom_get_value_type(node);
		if (vt.family == vt_datetime) {
			const char *ep = bvnr_datetime_epoch_name(vt);
			char d[48];
			snprintf(d, sizeof d, "seconds since the %s epoch",
				 ep ? ep : "unix");
			L->n_datetime++;
			json_loss_note(L->ex_datetime, sizeof L->ex_datetime, path, d);
		} else if (vt.width > 64u) {
			L->n_wide++;
			json_loss_note(L->ex_wide, sizeof L->ex_wide, path,
				       "emitted as a JSON string");
		} else if (bvn_effective_base(vt) != 10u) {
			L->n_typed++;
			json_loss_note(L->ex_typed, sizeof L->ex_typed, path,
				       "written in a non-decimal base");
		}
		break;
	}
	case BVN_DOM_STRUCT: {
		uint32_t cnt = bvn_dom_struct_count(node);
		const bvn_dom_entry_t *e = bvn_dom_struct_entries(node);
		for (uint32_t i = 0; e && i < cnt; i++) {
			char sub[256];
			snprintf(sub, sizeof sub, "%s.%s", path, e[i].key);
			json_loss_scan(e[i].value, sub, L);
		}
		break;
	}
	case BVN_DOM_ARRAY: {
		uint32_t cnt  = bvn_dom_array_count(node);
		/*
		 * A "/"-separated multi-dimensional array and a nested array of the same
		 * shape produce BYTE-IDENTICAL JSON: [1,2,3]/[4,5,6] and
		 * [[1,2,3],[4,5,6]] both emit [[1,2,3],[4,5,6]]. In the DOM they are not
		 * the same thing at all -- the first is one array of 6 elements with
		 * num_dims 2, the second is an array of 2 arrays of 3 -- and only the
		 * second is what reading that JSON back gives you. So the dimension
		 * structure is lost, exactly like a unit is, and it used to be lost
		 * SILENTLY: convert exited 0 and said nothing, which is the one outcome
		 * this report exists to prevent. Rows of length 1 are not a special case
		 * worth excluding; if the document wrote "/", it meant a dimension.
		 */
		if (bvn_dom_array_dims(node) > 1u) {
			char d[64];
			snprintf(d, sizeof d, "%u-dimensional, %u element(s)",
				 bvn_dom_array_dims(node), cnt);
			L->n_dims++;
			json_loss_note(L->ex_dims, sizeof L->ex_dims, path, d);
		}
		for (uint32_t i = 0; i < cnt; i++) {
			char sub[256];
			snprintf(sub, sizeof sub, "%s[%u]", path, i);
			json_loss_scan(bvn_dom_array_at(node, i), sub, L);
		}
		break;
	}
	default:
		break;
	}
}

/* Returns true if anything was lost (and reported). */
static bool json_loss_report(const json_loss_t *L, const char *file)
{
	uint32_t total = L->n_unit + L->n_symbol + L->n_reference +
			 L->n_octet + L->n_wide + L->n_typed + L->n_dims +
			 L->n_nonfinite + L->n_float + L->n_datetime;
	if (!total) return false;
	fprintf(stderr, "convert: %s: JSON cannot carry everything in this "
			"document; the output above is LOSSY:\n", file);
	if (L->n_unit)
		fprintf(stderr, "  %u value(s) lose their unit entirely, e.g. %s\n",
			L->n_unit, L->ex_unit);
	if (L->n_wide)
		fprintf(stderr, "  %u integer(s) wider than 64 bits become JSON "
				"strings, e.g. %s\n", L->n_wide, L->ex_wide);
	if (L->n_typed)
		fprintf(stderr, "  %u integer(s) lose their numeric base, e.g. %s\n",
			L->n_typed, L->ex_typed);
	if (L->n_nonfinite)
		fprintf(stderr, "  %u non-finite float(s) become null, e.g. %s\n",
			L->n_nonfinite, L->ex_nonfinite);
	if (L->n_float)
		fprintf(stderr, "  %u float(s) wider or more exact than a 64-bit "
				"binary float are rounded to one, e.g. %s\n",
			L->n_float, L->ex_float);
	if (L->n_datetime)
		fprintf(stderr, "  %u datetime(s) become bare integers, e.g. %s\n",
			L->n_datetime, L->ex_datetime);
	if (L->n_symbol)
		fprintf(stderr, "  %u symbol(s) become plain strings, e.g. %s\n",
			L->n_symbol, L->ex_symbol);
	if (L->n_reference)
		fprintf(stderr, "  %u reference(s) become plain strings and stop "
				"resolving, e.g. %s\n", L->n_reference, L->ex_reference);
	if (L->n_octet)
		fprintf(stderr, "  %u octet stream(s) become hex strings, e.g. %s\n",
			L->n_octet, L->ex_octet);
	if (L->n_dims)
		fprintf(stderr, "  %u \"/\"-separated multi-dimensional array(s) become "
				"nested JSON arrays and read back as nested arrays, "
				"e.g. %s\n", L->n_dims, L->ex_dims);
	fprintf(stderr, "convert: exiting 1: this JSON does not convert back to "
			"the document it came from.\n");
	return true;
}
static int cmd_convert_bvnr_to_json(const char *file)
{
	int fd = open(file, BVN_O_RDONLY);
	if (fd < 0) { perror(file); return 1; }
	bvn_dom_doc_t *doc = bvn_dom_parse_fd(fd);
	close(fd);
	if (!doc) {
		fprintf(stderr, "Failed to parse %s\n", file);
		return 1;
	}
	{
		error_code_t perr = bvn_dom_doc_get_parse_error(doc);
		if (perr != error_none) {
			fprintf(stderr, "Parse error in %s: %s\n",
				file, bvn_error_to_string(perr));
			bvn_dom_doc_destroy(doc);
			return 1;
		}
	}
	uint32_t cnt = bvn_dom_doc_count(doc);
	const bvn_dom_entry_t *entries = bvn_dom_doc_entries(doc);
	/* Refuse before emitting anything if a datetime carries a sub-second
	 * fraction JSON cannot represent (see find_fractional_datetime). */
	for (uint32_t i = 0; i < cnt; i++) {
		if (find_fractional_datetime(entries[i].value)) {
			fprintf(stderr,
				"convert: %s: a datetime with sub-second digits cannot be "
				"represented in JSON without dropping the fraction; refusing "
				"rather than silently truncating (under top-level key '%s')\n",
				file, entries[i].key);
			bvn_dom_doc_destroy(doc);
			return 1;
		}
	}
	/* Survey the loss before emitting, so the report is complete even if stdout
	 * is a pipe the reader closes early. */
	json_loss_t loss;
	memset(&loss, 0, sizeof loss);
	for (uint32_t i = 0; i < cnt; i++) {
		char path[256];
		snprintf(path, sizeof path, ".%s", entries[i].key);
		json_loss_scan(entries[i].value, path, &loss);
	}
	putchar('{');
	if (cnt) putchar('\n');
	for (uint32_t i = 0; i < cnt; i++) {
		fputs("  ", stdout);
		print_json_string_escaped(entries[i].key, (uint32_t)strlen(entries[i].key));
		fputs(": ", stdout);
		print_json_node(entries[i].value, 1, true);
		if (i + 1 < cnt) putchar(',');
		putchar('\n');
	}
	puts("}");
	bvn_dom_doc_destroy(doc);
	fflush(stdout);
	/* The JSON is still written — it is usually what the caller wanted — but the
	 * exit status says it is not a faithful copy. */
	return json_loss_report(&loss, file) ? 1 : 0;
}
/* Map a file name to its format ("json" or "bvnr") by extension, or NULL if
 * the extension is unknown. Used to auto-detect the conversion direction so
 * the caller need not spell out --from/--to. */
static const char *convert_format_from_ext(const char *file)
{
	const char *dot = strrchr(file, '.');
	if (!dot) return NULL;
	if (strcmp(dot, ".json") == 0) return "json";
	if (strcmp(dot, ".bvnr") == 0) return "bvnr";
	return NULL;
}

static int cmd_convert(const char *from, const char *to, const char *file)
{
	/* There are two formats, so naming EITHER end names the direction: with
	 * --from json the target is bvnr, with --to json the source is. The
	 * extension is consulted only when neither was given.
	 *
	 * It used to be consulted whenever one was missing, and the missing end
	 * was then derived from the EXTENSION rather than from the end that had
	 * been given -- so `convert --from json data.bvnr` reported "Unsupported
	 * conversion: json -> json", a direction nobody asked for, and
	 * `convert --from json data.txt` said to "pass --from/--to explicitly" to
	 * someone who just had. */
	static const char *const fmts[] = { "json", "bvnr" };
	for (int k = 0; k < 2; k++) {
		const char *given = k == 0 ? from : to;
		if (given && strcmp(given, fmts[0]) != 0 &&
			     strcmp(given, fmts[1]) != 0) {
			fprintf(stderr, "convert: %s '%s' is not a format "
				"(json, bvnr)\n", k == 0 ? "--from" : "--to", given);
			return 2;
		}
	}
	if (!from && !to) {
		const char *fmt = convert_format_from_ext(file);
		if (!fmt) {
			fprintf(stderr,
					"convert: cannot detect format of '%s' from its "
					"extension; pass --from <fmt> or --to <fmt>\n", file);
			return 2;
		}
		from = fmt;
	}
	if (!from) from = (strcmp(to,   "json") == 0) ? "bvnr" : "json";
	if (!to)   to   = (strcmp(from, "json") == 0) ? "bvnr" : "json";
	if (strcmp(from, "json") == 0 && strcmp(to, "bvnr") == 0)
		return cmd_convert_json_to_bvnr(file);
	if (strcmp(from, "bvnr") == 0 && strcmp(to, "json") == 0)
		return cmd_convert_bvnr_to_json(file);
	fprintf(stderr,
			"Unsupported conversion: %s -> %s\n"
			"Supported: json -> bvnr,  bvnr -> json\n",
			from, to);
	return 2;
}
typedef enum {
	BMARK_PROFILE_SCALARS,
	BMARK_PROFILE_TYPED,
	BMARK_PROFILE_STRUCTS,
	BMARK_PROFILE_ARRAYS,
	BMARK_PROFILE_UNITS,
	BMARK_PROFILE_MIXED,
	BMARK_PROFILE_COUNT
} bmark_profile_t;
static const char *bmark_profile_names[BMARK_PROFILE_COUNT] = {
	"scalars", "typed", "structs", "arrays", "units", "mixed"
};
typedef struct {
	bmark_profile_t profile;
	size_t          payload_size;
	size_t          num_assignments;
	size_t          num_events;
	double          elapsed_sec;
	double          cpu_sec;
} bmark_result_t;
typedef struct {
	bool     profiles[BMARK_PROFILE_COUNT];
	size_t   sizes[64];
	uint32_t num_sizes;
	uint32_t iterations;
	uint32_t warmup;
	bool     verbose;
	bool     json;
	bool     min_overhead;
} bmark_cfg_t;
static bmark_cfg_t bmark_cfg = {
	.profiles   = {true, true, true, true, true, true},
	.sizes      = {1024, 4096, 16384, 65536},
	.num_sizes  = 4,
	.iterations = 100,
	.warmup     = 10,
	.verbose    = false,
	.json       = false,
	.min_overhead = false,
};
/* Monotonic wall clock as a double (seconds); the platform source is in
 * bvn_port.h (clock_gettime on POSIX, QueryPerformanceCounter on Windows). */
typedef double bmark_wall_clock_t;
static bmark_wall_clock_t bmark_timer_now(void)
{
	return bvn_monotonic_sec();
}
static double bmark_timer_sec(const bmark_wall_clock_t *start,
                              const bmark_wall_clock_t *end)
{
	return *end - *start;
}
static double bmark_cpu_sec(void)
{
	return bvn_cpu_sec();
}
typedef struct {
	size_t      event_count;
	size_t      assign_count;
	const bool *min_mode;
} bmark_counter_ctx_t;
static bool bmark_event_counter(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	bmark_counter_ctx_t *ctx = ud;
	ctx->event_count++;
	if (ev == ev_assignment_start)
		ctx->assign_count++;
	(void)d;
	return true;
}
#if defined(__GNUC__) || defined(__clang__)
#  pragma GCC diagnostic push
#  pragma GCC diagnostic ignored "-Wformat-nonliteral"
#endif
static size_t bmark_gen_scalars(uint8_t *buf, size_t cap, size_t *out_assignments)
{
	static const char *templates[] = {
		".k%u=%lld;\n",
		".k%u=%lld;\n",
		".k%u=\"%s\";\n",
		".k%u=true;\n",
	};
	static const int64_t values[] = {0, 42, -17, 1000000, 255, 9999};
	size_t pos   = 0;
	size_t count = 0;
	for (size_t i = 0; ; i++) {
		int     tmpl_idx = (int)(i % 4);
		int64_t val      = values[i % 6];
		char    buf32[64];
		int     n;
		if (tmpl_idx == 2) {
			static const char *strs[] = {"hello","world","test","value","key","data"};
			n = snprintf(buf32, sizeof(buf32), ".k%u=\"%s\";\n",
			             (unsigned)i, strs[i % 6]);
		} else {
			n = snprintf(buf32, sizeof(buf32), templates[tmpl_idx],
			             (unsigned)i, (long long)val);
		}
		if (n < 0 || (size_t)n >= cap - pos - 1)
			break;
		memcpy(buf + pos, buf32, (size_t)n);
		pos   += (size_t)n;
		count++;
	}
	*out_assignments = count;
	return pos;
}
static size_t bmark_gen_typed(uint8_t *buf, size_t cap, size_t *out_assignments)
{
	static const char *tmpls[] = {
		".k%u=<uint:8>%u;\n",
		".k%u=<uint:16>%u;\n",
		".k%u=<uint:32>%u;\n",
		".k%u=<sint:16>%d;\n",
		".k%u=<float:32>%.17g;\n",
		".k%u=<float:64>%.17g;\n",
		".k%u=<utf8>\"%s\";\n",
	};
	static const unsigned  u8v[]  = {0, 1,  42, 127,       200,       255};
	static const unsigned  u16v[] = {0, 1, 100, 1000,    32767,     65535};
	static const unsigned  u32v[] = {0, 1, 999, 65536, 2147483648u, 4294967295u};
	static const int       s16v[] = {-32768, -100, -1, 0,   100, 32767};
	static const double    fltv[] = {3.14, -1.5, 2.71828, 1e10,  0.5,  -0.5};
	static const char     *strv[] = {"text","data","value","name","label","x"};
	size_t pos   = 0;
	size_t count = 0;
	for (size_t i = 0; ; i++) {
		int    tmpl = (int)(i % 7);
		size_t vi   = (i / 7) % 6;
		char   tmp[128];
		int    n;
		switch (tmpl) {
		case 0: n = snprintf(tmp, sizeof(tmp), tmpls[0], (unsigned)i, u8v[vi]);  break;
		case 1: n = snprintf(tmp, sizeof(tmp), tmpls[1], (unsigned)i, u16v[vi]); break;
		case 2: n = snprintf(tmp, sizeof(tmp), tmpls[2], (unsigned)i, u32v[vi]); break;
		case 3: n = snprintf(tmp, sizeof(tmp), tmpls[3], (unsigned)i, s16v[vi]); break;
		case 4: n = snprintf(tmp, sizeof(tmp), tmpls[4], (unsigned)i, fltv[vi]); break;
		case 5: n = snprintf(tmp, sizeof(tmp), tmpls[5], (unsigned)i, fltv[vi]); break;
		case 6: n = snprintf(tmp, sizeof(tmp), tmpls[6], (unsigned)i, strv[vi]); break;
		default: n = -1; break;
		}
		if (n < 0 || (size_t)n >= cap - pos - 1)
			break;
		memcpy(buf + pos, tmp, (size_t)n);
		pos   += (size_t)n;
		count++;
	}
	*out_assignments = count;
	return pos;
}
static size_t bmark_gen_structs(uint8_t *buf, size_t cap, size_t *out_assignments)
{
	static const size_t MAX_DEPTH = 16;
	size_t pos    = 0;
	size_t count  = 0;
	size_t serial = 0;
	bool   done   = false;
	while (!done) {
		size_t depth = 0;
		while (depth < MAX_DEPTH) {
			char tmp[64];
			int n = snprintf(tmp, sizeof(tmp),
			                 ".s%zu={.a=%zu;.b=%zu;.c=%zu;",
			                 serial, serial, serial + 1, serial + 2);
			/* Guard truncation as well as error: a truncated snprintf returns
			 * the would-be length (>= sizeof tmp), and the memcpy below would
			 * then over-read tmp. Unreachable with the cap-bounded serial here,
			 * but cheap to make safe. */
			if (n < 0 || (size_t)n >= sizeof(tmp)) { done = true; break; }
			size_t close_needed = (depth + 1) * 3;
			if (pos + (size_t)n + close_needed > cap) { done = true; break; }
			memcpy(buf + pos, tmp, (size_t)n);
			pos    += (size_t)n;
			count  += 4;
			serial++;
			depth++;
		}
		for (size_t i = 0; i < depth && pos + 3 <= cap; i++) {
			memcpy(buf + pos, "};\n", 3);
			pos += 3;
		}
		if (depth == 0) break;
	}
	*out_assignments = count;
	return pos;
}
static size_t bmark_gen_arrays(uint8_t *buf, size_t cap, size_t *out_assignments)
{
	static const int ROW_LEN       = 5;
	static const int ROWS_PER_ASGN = 8;
	static const int int_vals[] = {42, 0, 100, 999, 7};
	static const int NVALS = (int)(sizeof(int_vals) / sizeof(int_vals[0]));
	size_t pos  = 0;
	size_t asgn = 0;
	size_t elem = 0;
	for (;;) {
		char hdr[32];
		int  hn = snprintf(hdr, sizeof(hdr), ".a%zu=[", asgn);
		if (hn < 0) break;
		size_t max_per_asgn = (size_t)hn
			+ (size_t)ROWS_PER_ASGN * ((size_t)ROW_LEN * 5u + 4u)
			+ 8u;
		if (pos + max_per_asgn > cap) break;
		memcpy(buf + pos, hdr, (size_t)hn);
		pos += (size_t)hn;
		for (int row = 0; row < ROWS_PER_ASGN; row++) {
			if (row > 0) {
				memcpy(buf + pos, "]/[", 3);
				pos += 3;
			}
			for (int col = 0; col < ROW_LEN; col++) {
				if (col > 0) buf[pos++] = ',';
				char val[16];
				int n = snprintf(val, sizeof(val), "%d",
				                 int_vals[elem % (size_t)NVALS]);
				size_t vl = (n > 0) ? (size_t)n : 1u;
				memcpy(buf + pos, val, vl);
				pos += vl;
				elem++;
			}
		}
		memcpy(buf + pos, "];\n", 3);
		pos += 3;
		asgn++;
	}
	*out_assignments = asgn;
	return pos;
}
static size_t bmark_gen_units(uint8_t *buf, size_t cap, size_t *out_assignments)
{
	static const char *unit_templates[] = {
		".k%u=<float:64,m/s>%.17g;\n",
		".k%u=<float:64,k~g\xc2\xb7m/s\xc2\xb2>%.17g;\n",
		".k%u=<float:64,k~J>%.17g;\n",
		".k%u=<uint:64,Gi~B>%llu;\n",
		".k%u=<float:64,K>%.17g;\n",
		".k%u=<uint:64,Mi~b>%llu;\n",
		".k%u=<float:64,m/s\xc2\xb2>%.17g;\n",
		".k%u=<float:64,k~Pa>%.17g;\n",
		".k%u=<float:64,k~g/m\xc2\xb3>%.17g;\n",
		".k%u=<float:64,m*s>%.17g;\n",
		".k%u=<uint:32,no_unit>%llu;\n",
		".k%u=<float:64,V/m>%.17g;\n",
	};
	static const double flt_vals[] = {
		9.81, 9.81, 5400.0, 0, 300.0, 0, 9.81, 101.325, 7800.0, 9.81, 0, 150.0
	};
	static const uint64_t uint_vals[] = {0, 8, 256, 0, 0, 0, 0, 0, 0, 0, 42, 0};
	size_t ntemplates = sizeof(unit_templates) / sizeof(unit_templates[0]);
	size_t pos   = 0;
	size_t count = 0;
	for (size_t i = 0; ; i++) {
		int  t_idx = (int)(i % ntemplates);
		char buf128[256];
		int  n;
		if (t_idx == 3 || t_idx == 5 || t_idx == 10)
			n = snprintf(buf128, sizeof(buf128), unit_templates[t_idx],
			             (unsigned)i, (unsigned long long)uint_vals[i % 12]);
		else
			n = snprintf(buf128, sizeof(buf128), unit_templates[t_idx],
			             (unsigned)i, flt_vals[i % 12]);
		if (n < 0 || (size_t)n >= cap - pos - 1)
			break;
		memcpy(buf + pos, buf128, (size_t)n);
		pos   += (size_t)n;
		count++;
	}
	*out_assignments = count;
	return pos;
}
static size_t bmark_gen_mixed(uint8_t *buf, size_t cap, size_t *out_assignments)
{
	static const char block1[] =
		".system={\n"
		".host=\"localhost\";\n"
		".port=<uint:16>8080;\n"
		".limits={\n"
		".timeout=<float:64,s>30;\n"
		".max_payload=<uint:64,Mi~B>16;\n"
		"};\n"
		"};\n"
		".sensors=[\n"
		"{.name=\"temp\";.value=<float:64,\xc2\xb0\x43>23.5;.precision=<float:32>0.1;},\n"
		"{.name=\"pressure\";.value=<float:64,k~Pa>101.3;}\n"
		"];\n";
	static const char block2[] =
		".matrix=[1,2,3,4]/[5,6,7,8]/[9,10,11,12];\n"
		".count=<uint:32>42;\n"
		".name=<utf8>\"test object\";\n"
		".ratio=0.95;\n"
		".flags=[true,false,true,false];\n";
	size_t pos = 0;
	size_t count = 0;
	size_t block1_size    = sizeof(block1) - 1;
	size_t block2_size    = sizeof(block2) - 1;
	size_t block1_assigns = 8;
	size_t block2_assigns = 5;
	while (pos < cap) {
		if (pos + block1_size < cap) {
			memcpy(buf + pos, block1, block1_size);
			pos   += block1_size;
			count += block1_assigns;
			if (pos + block2_size < cap) {
				memcpy(buf + pos, block2, block2_size);
				pos   += block2_size;
				count += block2_assigns;
			} else {
				break;
			}
		} else {
			break;
		}
	}
	*out_assignments = count;
	return pos;
}
typedef struct {
	size_t (*gen)(uint8_t *, size_t, size_t *);
	const char *name;
} bmark_builder_t;
static const bmark_builder_t bmark_builders[BMARK_PROFILE_COUNT] = {
	{ bmark_gen_scalars, "scalars" },
	{ bmark_gen_typed,   "typed"   },
	{ bmark_gen_structs, "structs" },
	{ bmark_gen_arrays,  "arrays"  },
	{ bmark_gen_units,   "units"   },
	{ bmark_gen_mixed,   "mixed"   },
};
#if defined(__GNUC__) || defined(__clang__)
#  pragma GCC diagnostic pop
#endif
static bmark_result_t bmark_run(bmark_profile_t profile,
                                size_t target_size,
                                uint32_t iterations)
{
	bmark_result_t result;
	memset(&result, 0, sizeof(result));
	result.profile = profile;
	uint8_t *buf = malloc(target_size + 1024 + 256);
	if (!buf) {
		fprintf(stderr, "  ERROR: malloc(%zu) failed\n", target_size + 1280);
		return result;
	}
	/* The generator is given --size itself as its limit. It used to be given
	 * size + 1024, and since each generator fills its limit with whole
	 * assignments, "--size 1024" benchmarked a 2040-byte payload and
	 * "--size 1" a 1015-byte one -- the Bytes column said so, and the option
	 * that asked for it did not mean what its help said. The slack stays in
	 * the ALLOCATION only. A size too small for one whole assignment of the
	 * profile now generates nothing, which the caller reports as a failed
	 * cell rather than quietly measuring a kilobyte instead. */
	size_t assign_count = 0;
	size_t actual_len = bmark_builders[profile].gen(buf, target_size,
	                                                &assign_count);
	if (actual_len == 0) {
		free(buf);
		return result;
	}
	result.num_assignments = assign_count;
	bmark_counter_ctx_t count_ctx = {0, 0, &bmark_cfg.min_overhead};
	bvnr_read_flags_t count_flags;
	memset(&count_flags, 0, sizeof(count_flags));
	count_flags.userdata      = &count_ctx;
	count_flags.on_verified   = bmark_event_counter;
	count_flags.on_unverified = bmark_cfg.min_overhead ? NULL : bmark_event_counter;
	bvnr_reader_t *r_count = bvnr_reader_create();
	if (!r_count) { free(buf); return result; }
	bool ok = bvnr_open_read_mem(r_count, buf, (uint64_t)actual_len,
	                             NULL, 0, &count_flags)
	       && bvnr_read(r_count);
	if (ok)
		result.num_events = count_ctx.event_count;
	bvnr_reader_destroy(r_count);
	if (!ok) {
		fprintf(stderr, "  ERROR: count-parse failed for profile=%s size=%zu\n",
		        bmark_profile_names[profile], target_size);
		/* payload_size stays 0: that is what tells cmd_bench the cell
		 * failed. Setting it here printed a row of zeros and infinities
		 * under the error and let the run exit 0. */
		free(buf);
		return result;
	}
	for (uint32_t w = 0; w < bmark_cfg.warmup; w++) {
		bmark_counter_ctx_t warm_ctx = {0, 0, &bmark_cfg.min_overhead};
		bvnr_read_flags_t warm_flags;
		memset(&warm_flags, 0, sizeof(warm_flags));
		warm_flags.userdata      = &warm_ctx;
		warm_flags.on_verified   = bmark_event_counter;
		warm_flags.on_unverified = bmark_cfg.min_overhead ? NULL : bmark_event_counter;
		bvnr_reader_t *r_warm = bvnr_reader_create();
		if (r_warm) {
			bvnr_open_read_mem(r_warm, buf, (uint64_t)actual_len,
			                   NULL, 0, &warm_flags);
			bvnr_read(r_warm);
			bvnr_reader_destroy(r_warm);
		}
	}
	double            cpu_start  = bmark_cpu_sec();
	bmark_wall_clock_t wall_start = bmark_timer_now();
	for (uint32_t i = 0; i < iterations; i++) {
		bmark_counter_ctx_t run_ctx = {0, 0, &bmark_cfg.min_overhead};
		bvnr_read_flags_t run_flags;
		memset(&run_flags, 0, sizeof(run_flags));
		run_flags.userdata      = &run_ctx;
		run_flags.on_verified   = bmark_event_counter;
		run_flags.on_unverified = bmark_cfg.min_overhead ? NULL : bmark_event_counter;
		bvnr_reader_t *r_run = bvnr_reader_create();
		if (!r_run) { free(buf); return result; }
		if (!bvnr_open_read_mem(r_run, buf, (uint64_t)actual_len,
		                        NULL, 0, &run_flags)) {
			bvnr_reader_destroy(r_run);
			free(buf);
			return result;
		}
		bvnr_read(r_run);
		bvnr_reader_destroy(r_run);
	}
	bmark_wall_clock_t wall_end = bmark_timer_now();
	double             cpu_end  = bmark_cpu_sec();
	result.elapsed_sec  = bmark_timer_sec(&wall_start, &wall_end);
	result.cpu_sec      = cpu_end - cpu_start;
	result.payload_size = actual_len;
	free(buf);
	return result;
}
static void bmark_print_header(void)
{
	if (bmark_cfg.json) return;
	printf("%-10s %8s %8s %12s %12s %12s %10s %10s\n",
	       "Profile", "Bytes", "Assigns", "Wall (ms)", "CPU (ms)",
	       "MB/s", "Ass/s", "Ev/s");
	cwf("────────── ──────── ──────── ──────────── "
	       "──────────── ──────────── ────────── ──────────\n");
}
static void bmark_print_result(const bmark_result_t *r, uint32_t iterations)
{
	double wall_ms          = r->elapsed_sec * 1000.0;
	double cpu_ms           = r->cpu_sec * 1000.0;
	double wall_per_iter_ms = wall_ms / (double)iterations;
	double mb_per_sec       = (double)r->payload_size * (double)iterations
	                        / (1024.0 * 1024.0) / r->elapsed_sec;
	double assign_per_sec   = (double)r->num_assignments * (double)iterations
	                        / r->elapsed_sec;
	double events_per_sec   = (double)r->num_events * (double)iterations
	                        / r->elapsed_sec;
	if (bmark_cfg.json) {
		printf("{\"profile\":\"%s\",\"bytes\":%zu,\"assignments\":%zu,"
		       "\"events\":%zu,\"iterations\":%u,\"wall_ms\":%.3f,"
		       "\"cpu_ms\":%.3f,\"mb_per_sec\":%.3f,\"ass_per_sec\":%.0f,"
		       "\"ev_per_sec\":%.0f,\"wall_per_iter_us\":%.1f}\n",
		       bmark_profile_names[r->profile],
		       r->payload_size, r->num_assignments, r->num_events,
		       iterations,
		       wall_ms, cpu_ms,
		       mb_per_sec, assign_per_sec, events_per_sec,
		       wall_per_iter_ms * 1000.0);
	} else {
		printf("%-10s %8zu %8zu %12.3f %12.3f %12.2f %10.0f %10.0f\n",
		       bmark_profile_names[r->profile],
		       r->payload_size, r->num_assignments,
		       wall_ms, cpu_ms,
		       mb_per_sec, assign_per_sec, events_per_sec);
	}
}
static bool bmark_any_profile(void)
{
	for (int i = 0; i < BMARK_PROFILE_COUNT; i++)
		if (bmark_cfg.profiles[i]) return true;
	return false;
}
/*
 * Both list parsers return false, having said why, on the first entry they do
 * not understand. They used to skip it: "--profile scalars,bogus" ran scalars
 * and never mentioned bogus, "--size 12abc" benchmarked 12 bytes, and an
 * out-of-range size was "ignored" on stderr while the run carried on with the
 * rest -- three ways to get a table for a command line the user did not write.
 */
static bool bmark_parse_profile_list(const char *list)
{
	for (int i = 0; i < BMARK_PROFILE_COUNT; i++)
		bmark_cfg.profiles[i] = false;
	if (strcmp(list, "all") == 0) {
		for (int i = 0; i < BMARK_PROFILE_COUNT; i++)
			bmark_cfg.profiles[i] = true;
		return true;
	}
	char *copy = strdup(list);
	if (!copy) return false;
	bool ok = true;
	char *token = strtok(copy, ",");
	while (token && ok) {
		int i = 0;
		for (; i < BMARK_PROFILE_COUNT; i++) {
			if (strcmp(token, bmark_profile_names[i]) == 0) {
				bmark_cfg.profiles[i] = true;
				break;
			}
		}
		if (i == BMARK_PROFILE_COUNT) {
			fprintf(stderr, "bench: --profile '%s' is not a known profile "
				"(all, scalars, typed, structs, arrays, units, mixed)\n",
				token);
			ok = false;
		}
		token = strtok(NULL, ",");
	}
	free(copy);
	return ok;
}
static bool bmark_parse_size_list(const char *list)
{
	bmark_cfg.num_sizes = 0;
	char *copy = strdup(list);
	if (!copy) return false;
	bool ok = true;
	char *token = strtok(copy, ",");
	while (token && ok) {
		char *end = NULL;
		errno = 0;
		unsigned long long v = (token[0] >= '0' && token[0] <= '9')
				     ? strtoull(token, &end, 10) : 0ull;
		/* bmark_run allocates size + 1280; reject 0 and any value that would
		 * wrap that addition (or overflow size_t on a 32-bit host) so the
		 * generator can never be handed a buffer smaller than it writes. */
		if (!end || *end != '\0' || errno != 0 || v == 0 ||
		    v > (unsigned long long)(SIZE_MAX - 4096u)) {
			fprintf(stderr, "bench: --size '%s' is not a payload size "
				"(want a plain positive number of bytes)\n", token);
			ok = false;
		} else if (bmark_cfg.num_sizes >= 64) {
			fprintf(stderr, "bench: at most 64 --size values\n");
			ok = false;
		} else {
			bmark_cfg.sizes[bmark_cfg.num_sizes++] = (size_t)v;
		}
		token = strtok(NULL, ",");
	}
	free(copy);
	return ok;
}
/* Parse a bounded count for a bench option. strtoul wraps a leading '-' into a
 * huge positive value, and neither errno nor the end pointer reports it, so
 * "--iterations -1" became 4294967295 rounds and the CLI hung for hours; the
 * "< 1" clamp below it could never fire. "--iterations abc" silently became the
 * default with no diagnostic. Same class as the guard on mux pack's channel id.
 * Returns false and complains on anything that is not a plain decimal count. */
static bool bench_parse_count(const char *opt, const char *arg,
			      unsigned long max, uint32_t *out)
{
	if (!arg || !*arg || arg[0] == '-' || arg[0] == '+' ||
	    arg[0] == ' ' || arg[0] == '\t') {
		fprintf(stderr, "bench: %s wants a plain non-negative number, got '%s'\n",
			opt, arg ? arg : "");
		return false;
	}
	char *end = NULL;
	errno = 0;
	unsigned long v = strtoul(arg, &end, 10);
	if (end == arg || *end != '\0' || errno != 0) {
		fprintf(stderr, "bench: %s wants a plain non-negative number, got '%s'\n",
			opt, arg);
		return false;
	}
	if (v > max) {
		fprintf(stderr, "bench: %s %lu exceeds the maximum of %lu\n",
			opt, v, max);
		return false;
	}
	*out = (uint32_t)v;
	return true;
}
static void bench_usage(FILE *f)
{
	fprintf(f,
	    "Usage: bovnar bench [options]\n"
	    "\n"
	    "Options:\n"
	    "  --profile <list>   Comma-separated profile names:\n"
	    "                     all,scalars,typed,structs,arrays,units,mixed\n"
	    "                     (default: all)\n"
	    "  --size <list>      Comma-separated payload sizes in bytes; each\n"
	    "                     payload is whole assignments up to that size\n"
	    "                     (default: 1024,4096,16384,65536)\n"
	    "  --iterations <N>   Parse rounds per size×profile (default: 100)\n"
	    "  --warmup <N>       Warm-up iterations (default: 10)\n"
	    "  --verbose          Print per-run details\n"
	    "  --json             Machine-readable JSON output\n"
	    "  --min-overhead     Drop the on_unverified (lexer) callback and keep\n"
	    "                     only on_verified, halving the callback work\n"
	    "  -h, --help         Show this help and exit\n"
	    "\n"
	    "Examples:\n"
	    "  bovnar bench --profile scalars --size 4096\n"
	    "  bovnar bench --profile all --size 1024,65536 --iterations 200 --json\n"
	    "  bovnar bench --min-overhead --profile scalars,units --size 4096\n");
}
static int cmd_bench(int argc, char **argv)
{
	for (int i = 0; i < argc; i++) {
		/* An option that takes a value, given last on the line, used to fall
		 * through every `&& i + 1 < argc` below to the final else and be
		 * reported as "unknown option: --profile" -- for an option the help
		 * lists. Name what is actually missing. */
		if ((strcmp(argv[i], "--profile") == 0 ||
		     strcmp(argv[i], "--size") == 0 ||
		     strcmp(argv[i], "--iterations") == 0 ||
		     strcmp(argv[i], "--warmup") == 0) && i + 1 >= argc) {
			fprintf(stderr, "bench: %s needs a value\n", argv[i]);
			return 2;
		}
		if (strcmp(argv[i], "--profile") == 0) {
			if (!bmark_parse_profile_list(argv[++i]))
				return 2;
			/* The list parser clears the defaults before parsing, so a
			 * value that names nothing ("--profile ,") left an empty
			 * selection and the benchmark printed an empty table. */
			if (!bmark_any_profile()) {
				fprintf(stderr, "bench: --profile '%s' selects no "
					"known profile\n", argv[i]);
				return 2;
			}
		} else if (strcmp(argv[i], "--size") == 0) {
			if (!bmark_parse_size_list(argv[++i]))
				return 2;
			if (bmark_cfg.num_sizes == 0) {
				fprintf(stderr, "bench: --size '%s' selects no "
					"payload size\n", argv[i]);
				return 2;
			}
		} else if (strcmp(argv[i], "--iterations") == 0) {
			if (!bench_parse_count("--iterations", argv[++i],
					       10000000ul, &bmark_cfg.iterations))
				return 2;
			if (bmark_cfg.iterations < 1) bmark_cfg.iterations = 1;
		} else if (strcmp(argv[i], "--warmup") == 0) {
			if (!bench_parse_count("--warmup", argv[++i],
					       10000000ul, &bmark_cfg.warmup))
				return 2;
		} else if (strcmp(argv[i], "--verbose") == 0) {
			bmark_cfg.verbose = true;
		} else if (strcmp(argv[i], "--json") == 0) {
			bmark_cfg.json = true;
		} else if (strcmp(argv[i], "--min-overhead") == 0) {
			bmark_cfg.min_overhead = true;
		} else if (strcmp(argv[i], "-h") == 0 ||
		           strcmp(argv[i], "--help") == 0) {
			/* Asked-for help is output, not a diagnostic: stdout, so that
			 * `bovnar bench --help | less` shows something. */
			bench_usage(stdout);
			return 0;
		} else {
			fprintf(stderr, "bench: unknown option: %s\n", argv[i]);
			return 2;
		}
	}
	if (bmark_cfg.json) {
		printf("{\n\"config\":{\"iterations\":%u,\"warmup\":%u,"
		       "\"min_overhead\":%s},\n\"results\":[\n",
		       bmark_cfg.iterations, bmark_cfg.warmup,
		       bmark_cfg.min_overhead ? "true" : "false");
	} else {
		cwf("═════════════════════════════════════════════════════════════\n");
		printf("  Bovnar Parsing Throughput Benchmark\n");
		printf("  Iterations: %u   Warmup: %u   Min-overhead: %s\n",
		       bmark_cfg.iterations, bmark_cfg.warmup,
		       bmark_cfg.min_overhead ? "yes" : "no");
		cwf("═════════════════════════════════════════════════════════════\n");
		bmark_print_header();
	}
	bool first = true;
	/* A cell that could not be measured is a failed run. It was a line on
	 * stderr and exit status 0, so `bench --size 99999999999999999` -- every
	 * cell a malloc failure -- printed an empty table and reported success. */
	uint32_t failed = 0;
	for (int p = 0; p < BMARK_PROFILE_COUNT; p++) {
		if (!bmark_cfg.profiles[p]) continue;
		for (uint32_t s = 0; s < bmark_cfg.num_sizes; s++) {
			bmark_result_t r = bmark_run((bmark_profile_t)p,
			                             bmark_cfg.sizes[s],
			                             bmark_cfg.iterations);
			if (r.payload_size == 0) {
				fprintf(stderr, "  FAILED: %s/%zu (the size holds no whole "
					"assignment of this profile, or generation or "
					"the parse failed)\n",
				        bmark_profile_names[p], bmark_cfg.sizes[s]);
				failed++;
				continue;
			}
			if (bmark_cfg.json && !first)
				printf(",\n");
			first = false;
			bmark_print_result(&r, bmark_cfg.iterations);
			if (bmark_cfg.verbose && !bmark_cfg.json) {
				cwf("  ── detail: profile=%s, size=%zu bytes, "
				       "assignments=%zu, events=%zu, "
				       "avg_cost=%.3f us/assign\n",
				       bmark_profile_names[p], r.payload_size,
				       r.num_assignments, r.num_events,
				       (r.elapsed_sec * 1e6) /
				           (double)(r.num_assignments * bmark_cfg.iterations));
			}
		}
	}
	if (bmark_cfg.json)
		printf("\n]}\n");
	printf("\n");
	if (failed) {
		fprintf(stderr, "bench: %u cell(s) could not be measured\n", failed);
		return 1;
	}
	return 0;
}
/*
 * ---------------------------------------------------------------------------
 * Streaming/framing subcommands (frames, mux)
 * ---------------------------------------------------------------------------
 * Thin CLI drivers over the bovnar_stream layer so the multi-document framing
 * and octet-multiplexing conventions can be produced and consumed from the
 * shell. See doc/10_bovnar_streaming.md for the wire conventions.
 */

/* Read everything available on `fd` into a freshly malloc'd buffer. Does not
 * close fd — the caller owns its lifetime. Keeping the fd ownership out of this
 * function (slurp opens/closes unconditionally) is also what keeps -fanalyzer
 * from a false "leak of fd" report on the conditional stdin-vs-file path. */
static int read_all_fd(int fd, uint8_t **out, uint64_t *outlen)
{
	size_t cap = 65536, len = 0;
	uint8_t *buf = malloc(cap);
	if (!buf) return -1;
	for (;;) {
		if (len == cap) {
			size_t ncap = cap * 2u;
			uint8_t *nb = realloc(buf, ncap);
			if (!nb) { free(buf); return -1; }
			buf = nb; cap = ncap;
		}
		ssize_t n = read(fd, buf + len, cap - len);
		if (n < 0) {
			if (errno == EINTR) continue;
			free(buf); return -1;
		}
		if (n == 0) break;
		len += (size_t)n;
	}
	*out = buf;
	*outlen = (uint64_t)len;
	return 0;
}

/* Read an entire file (or stdin for "-") into a freshly malloc'd buffer. */
static int slurp(const char *path, uint8_t **out, uint64_t *outlen)
{
	if (strcmp(path, "-") == 0)
		return read_all_fd(STDIN_FILENO, out, outlen);
	int fd = open(path, BVN_O_RDONLY);
	if (fd < 0) { perror(path); return -1; }
	int rc = read_all_fd(fd, out, outlen);
	/* open() succeeding does not mean read() will: a directory opens fine and
	 * then fails with EISDIR. Without this the command exited 1 with nothing on
	 * stderr at all. */
	if (rc != 0) perror(path);
	close(fd);
	return rc;
}

static int cmd_frames_pack(int argc, char **argv)
{
	if (argc < 1) {
		fprintf(stderr, "frames pack: need at least one file\n");
		return 2;
	}
	bvnr_sink_t sink;
	bvnr_sink_to_fd(&sink, STDOUT_FILENO);
	for (int i = 0; i < argc; i++) {
		uint8_t *buf = NULL; uint64_t len = 0;
		if (slurp(argv[i], &buf, &len) != 0) return 1;
		bool ok = bvnr_frame_write(&sink, buf, len);
		free(buf);
		if (!ok) {
			fprintf(stderr, "frames pack: write failed on %s\n", argv[i]);
			return 1;
		}
	}
	return 0;
}

static bool frames_on_document(void *ud, uint64_t index, bool ok, error_code_t err)
{
	uint64_t *bad = (uint64_t *)ud;
	if (ok) {
		printf("frame %" PRIu64 ": OK\n", index);
	} else {
		printf("frame %" PRIu64 ": ERROR %s\n", index, bvn_error_to_string(err));
		/* Counted, not just printed: the exit status has to carry it. See
		 * cmd_frames_list. */
		if (bad) (*bad)++;
	}
	return true;   /* keep listing even past a bad frame */
}

static int cmd_frames_list(const char *path)
{
	bool is_stdin = (strcmp(path, "-") == 0);
	int fd = is_stdin ? STDIN_FILENO : open(path, BVN_O_RDONLY);
	if (fd < 0) { perror(path); return 1; }
	bvnr_source_t src;
	bvnr_source_from_fd(&src, fd);
	bvnr_read_flags_t fl = {0};
	fl.max_file_size      = BVNR_FILESIZE_UNLIMITED;
	fl.max_array_nesting  = 255;
	fl.max_struct_nesting = 255;
	/* Strict per-document parsing so each frame's status is reported accurately
	 * (no resync masking errors), but keep listing every frame regardless. */
	fl.continue_on_error  = false;
	uint64_t bad = 0;
	bvnr_doc_stream_opts_t opts = {0};
	opts.flags                = &fl;
	opts.userdata             = &bad;
	opts.on_document          = frames_on_document;
	opts.continue_past_failed = true;
	/* List every frame regardless of size: lift the per-frame guard with an
	 * explicit huge cap (max_document_size==0 now selects BVNR_DOC_DEFAULT_MAX). */
	opts.max_document_size    = UINT64_MAX;
	uint64_t count = 0;
	bool ok = bvnr_doc_stream_read(&src, &opts, &count);
	if (!is_stdin) close(fd);
	printf("%" PRIu64 " document(s)\n", count);
	if (!ok) {
		fprintf(stderr, "frames list: framing/IO error after %" PRIu64
				" document(s)\n", count);
		return 1;
	}
	/*
	 * A frame whose payload the reader rejected is a failure of the thing this
	 * command exists to report, so it has to reach the exit status.
	 *
	 * continue_past_failed is on so the listing is complete, and the per-frame
	 * verdict was PRINTED and then dropped: `bovnar frames list log.bvf`
	 * exited 0 over a log with a corrupt record in it, so `frames list x || alert`
	 * never fired and a caller had to grep stdout for "ERROR" to learn what the
	 * command had just told it. The sibling `mux list` already fails on a
	 * message-layer error, and `bovnar convert` was fixed for exactly this
	 * shape -- a loss reported on stdout and contradicted by a 0 exit.
	 *
	 * The framing error above keeps its own message and its own return: that one
	 * says the STREAM could not be walked, this one that a document in a stream
	 * that walked fine did not parse.
	 */
	if (bad) {
		fprintf(stderr, "frames list: %" PRIu64 " of %" PRIu64
				" document(s) failed to parse\n", bad, count);
		return 1;
	}
	return 0;
}

static int cmd_frames(int argc, char **argv)
{
	if (argc < 1) {
		fprintf(stderr, "Usage: frames pack <file>... | frames list <file|->\n");
		return 2;
	}
	if (strcmp(argv[0], "pack") == 0)
		return cmd_frames_pack(argc - 1, argv + 1);
	if (strcmp(argv[0], "list") == 0) {
		if (argc < 2) { fprintf(stderr, "frames list: need a file or -\n"); return 2; }
		if (argc > 2) {
			fprintf(stderr, "frames list: takes exactly one file (or -), "
					"got %d\n", argc - 1);
			return 2;
		}
		return cmd_frames_list(argv[1]);
	}
	fprintf(stderr, "frames: unknown action '%s' (use pack|list)\n", argv[0]);
	return 2;
}

static int cmd_mux_pack(int argc, char **argv)
{
	if (argc < 1) {
		fprintf(stderr, "mux pack: need at least one channel:file argument\n");
		return 2;
	}
	/* The command line is checked as a whole BEFORE the first byte is written.
	 * Each argument used to be examined as its turn came, so `mux pack 1:a.bvnr
	 * oops` had already put the stream header and channel 1 on stdout when it
	 * discovered the second argument was not one -- half a stream for a typo,
	 * and exit status 1, the code for a document that failed, where every
	 * other malformed command line exits 2. */
	for (int i = 0; i < argc; i++) {
		const char *colon = strchr(argv[i], ':');
		if (!colon) {
			fprintf(stderr, "mux pack: expected channel:file, got '%s'\n", argv[i]);
			return 2;
		}
		size_t clen = (size_t)(colon - argv[i]);
		/* strtoull wraps a leading '-' into a huge positive value and skips
		 * leading whitespace, neither of which endp or errno reports: "-1"
		 * packed cleanly onto channel 18446744073709551615. Digits only, so
		 * a typo is an error, not a surprise channel. */
		bool digits = clen > 0;
		for (size_t k = 0; k < clen; k++)
			if (argv[i][k] < '0' || argv[i][k] > '9') digits = false;
		errno = 0;
		if (digits) (void)strtoull(argv[i], NULL, 10);
		if (!digits || errno != 0) {
			fprintf(stderr, "mux pack: invalid channel '%.*s' (want a "
					"plain non-negative number below 2^64)\n",
				(int)clen, argv[i]);
			return 2;
		}
		if (!colon[1]) {
			fprintf(stderr, "mux pack: no file after '%s'\n", argv[i]);
			return 2;
		}
	}
	bvnr_writer_t *w = bvnr_writer_create();
	if (!w) return 1;
	bvnr_sink_t sink;
	bvnr_sink_to_fd(&sink, STDOUT_FILENO);
	bvnr_data_t hdr = {0};
	int rc = 1;
	if (!bvnr_open_write_sink(w, &sink, false, NULL)) goto done;
	if (!bvnr_write_event(w, ev_stream_start, &hdr))   goto done;
	if (!bvnr_mux_begin(w, "mux"))                     goto done;
	for (int i = 0; i < argc; i++) {
		const char *colon = strchr(argv[i], ':');
		uint64_t channel = strtoull(argv[i], NULL, 10);
		const char *path = colon + 1;
		uint8_t *buf = NULL; uint64_t len = 0;
		if (slurp(path, &buf, &len) != 0) goto done;
		bool ok = bvnr_mux_send(w, channel, buf, len);
		free(buf);
		if (!ok) {
			fprintf(stderr, "mux pack: send failed on channel %" PRIu64 "\n", channel);
			goto done;
		}
	}
	if (!bvnr_mux_end(w))        goto done;
	if (!bvnr_write_finish(w))   goto done;
	rc = 0;
done:
	/* Only report a writer-layer failure; CLI argument errors already printed
	 * their own message and leave the writer's error code clean. */
	if (rc != 0) {
		error_code_t werr = bvnr_writer_get_error(w);
		if (werr != error_none)
			fprintf(stderr, "mux pack: writer error %s\n",
					bvn_error_to_string(werr));
	}
	bvnr_writer_destroy(w);
	return rc;
}

static bool mux_print_msg(void *ud, uint64_t channel, const uint8_t *data, uint64_t len)
{
	(void)ud; (void)data;
	printf("channel %" PRIu64 ": %" PRIu64 " bytes\n", channel, len);
	return true;
}

static int cmd_mux_list(const char *path)
{
	bool is_stdin = (strcmp(path, "-") == 0);
	int fd = is_stdin ? STDIN_FILENO : open(path, BVN_O_RDONLY);
	if (fd < 0) { perror(path); return 1; }
	bvnr_demux_t *dm = bvnr_demux_create(mux_print_msg, NULL, 0);
	/* `mux pack` writes under the key "mux" and doc/10_bovnar_streaming.md tells
	 * callers to set the same filter. Without it every octet stream in the
	 * document is demuxed, so an ordinary binary payload is misread as framing
	 * and printed as garbage channels. */
	if (dm) bvnr_demux_set_key(dm, "mux");
	bvnr_reader_t *r = bvnr_reader_create();
	if (!dm || !r) {
		bvnr_demux_destroy(dm); bvnr_reader_destroy(r);
		if (!is_stdin) close(fd);
		return 1;
	}
	bvnr_source_t src;
	bvnr_source_from_fd(&src, fd);
	bvnr_read_flags_t fl = {0};
	fl.max_file_size      = BVNR_FILESIZE_UNLIMITED;
	fl.max_array_nesting  = 255;
	fl.max_struct_nesting = 255;
	fl.on_verified        = bvnr_demux_on_event;
	fl.userdata           = dm;
	bool ok = bvnr_open_read_source(r, &src, NULL, &fl) && bvnr_read(r);
	error_code_t derr = bvnr_demux_error(dm);
	/* Both, not one or the other: when the demux aborts the read, the reader
	 * reports the generic scanner_callback_failed and the specific reason lives
	 * only in derr -- an `else if` here hid exactly the case worth seeing. */
	if (!ok)
		fprintf(stderr, "mux list: parse error %s\n",
				bvn_error_to_string(bvnr_reader_get_error(r)));
	if (derr != error_none)
		fprintf(stderr, "mux list: demux error %s\n", bvn_error_to_string(derr));
	bvnr_reader_destroy(r);
	bvnr_demux_destroy(dm);
	if (!is_stdin) close(fd);
	return (ok && derr == error_none) ? 0 : 1;
}

static int cmd_mux(int argc, char **argv)
{
	if (argc < 1) {
		fprintf(stderr, "Usage: mux pack <channel:file>... | mux list <file|->\n");
		return 2;
	}
	if (strcmp(argv[0], "pack") == 0)
		return cmd_mux_pack(argc - 1, argv + 1);
	if (strcmp(argv[0], "list") == 0) {
		if (argc < 2) { fprintf(stderr, "mux list: need a file or -\n"); return 2; }
		if (argc > 2) {
			fprintf(stderr, "mux list: takes exactly one file (or -), "
					"got %d\n", argc - 1);
			return 2;
		}
		return cmd_mux_list(argv[1]);
	}
	fprintf(stderr, "mux: unknown action '%s' (use pack|list)\n", argv[0]);
	return 2;
}

static void usage(FILE *out, const char *prog)
{
	fprintf(out,
		"Usage: %s <command> [options] [file]\n"
		"Commands:\n"
		"  validate [opts] <file>\n"
		"                  Validate a .bvnr file. Unit-policy options:\n"
		"                    --text-only                refuse a document that\n"
		"                                               contains an octet stream\n"
		"                    --require-unit             every numeric value must\n"
		"                                               carry a unit\n"
		"                    --require-dimension <unit> every value must be\n"
		"                                               convertible to this unit\n"
		"                                               (repeatable)\n"
		"                    --require-field <path>=<unit>\n"
		"                                               the value at this key path\n"
		"                                               must be convertible to it;\n"
		"                                               a path may end in '.*'\n"
		"                                               (repeatable)\n"
		"                    --unit, --field, --si, --base, --leave-inexact\n"
		"                                               as for events, below. A\n"
		"                                               conversion that cannot be\n"
		"                                               delivered exactly FAILS the\n"
		"                                               validation, so --si asks\n"
		"                                               'would this document\n"
		"                                               survive normalisation?';\n"
		"                                               --leave-inexact relaxes it\n"
		"  query [opts] <path> <file>\n"
		"                  Query a value by path (e.g. .sensor.temperature).\n"
		"                  Prints the VALUE only -- no unit -- so it pipes cleanly.\n"
		"                  Takes the unit-policy options of events, so a query can\n"
		"                  assert what it expects and ask for the unit it wants\n"
		"                  back: --field .a.b=m --require-unit ... Two it does\n"
		"                  not take: --base (the value is printed in decimal) and\n"
		"                  --text-only (use validate).\n"
		"  pretty-print [--canonical] <file>\n"
		"                Pretty-print a .bvnr file. Comments are not part of a\n"
		"                document's data and are NOT preserved. --canonical: also\n"
		"                reduce every unit to canonical form, rescaling values to\n"
		"                match, so two spellings of one unit produce equal bytes\n"
		"  convert <file>  Convert json<->bvnr (direction from .json/.bvnr ext)\n"
		"                  Override with --from <fmt> and/or --to <fmt>\n"
		"                  Supported: json -> bvnr,  bvnr -> json\n"
		"                  bvnr -> json is LOSSY for most documents: JSON has no\n"
		"                  units, types, symbols, references or octet streams.\n"
		"                  The JSON is still written; what was lost is listed on\n"
		"                  stderr and the exit status is 1, so 0 means the JSON\n"
		"                  converts back to the same document.\n"
		"                  The unit-policy options are accepted and\n"
		"                  ignored here: JSON carries no units.\n",
		prog);
	/* Split here: one string literal for the whole of usage() grew past the
	 * 4095 characters C99 requires a compiler to support, which -Wpedantic
	 * reports. Separate calls cost nothing and keep the text strictly
	 * conforming. */
	fprintf(out,
		"  events [opts] <file|->\n"
		"                  Print lexer and validator events side by side.\n"
		"                  Pass '-' to read from stdin.\n"
		"                  Options:\n"
		"                    -c  Continue parsing on errors (resync mode); the\n"
		"                        exit status is still non-zero if any occurred\n"
		"                    -d  Enable debug re-serialisation output to stderr\n"
		"                    -p  Pretty-print debug output (requires -d)\n"
		"                    --unit <unit>       convert values to this unit,\n"
		"                                        first match wins (repeatable)\n"
		"                    --si                convert anything left over to\n"
		"                                        coherent SI base units\n"
		"                    --base <N>          render conversions in base N\n"
		"                    --leave-inexact     leave a value the conversion\n"
		"                                        cannot deliver exactly, rather\n"
		"                                        than failing the parse\n"
		"                    --field <path>=<unit>\n"
		"                                        convert the value at this key\n"
		"                                        path; a path may end in '.*' to\n"
		"                                        name a subtree (repeatable)\n"
		"                    --text-only         refuse a document that contains\n"
		"                                        an octet stream\n"
		"                    --require-unit, --require-dimension <unit>,\n"
		"                    --require-field <path>=<unit>\n"
		"                                        as for validate, above\n");
	fprintf(out,
		"  frames pack <file>...\n"
		"                  Wrap each document in a length-prefixed frame (stdout).\n"
		"  frames list <file|->\n"
		"                  List the documents in a frame stream.\n"
		"  mux pack <channel:file>...\n"
		"                  Multiplex files onto channels in one octet stream (stdout).\n"
		"  mux list <file|->\n"
		"                  List the channel/message sizes in a multiplexed stream.\n"
		"  version       Print library and supported spec version\n"
		"  bench [opts]\n"
		"                  Run parsing throughput benchmark.\n"
		"                  Options:\n"
		"                    --profile <list>   scalars,typed,structs,arrays,units,mixed\n"
		"                    --size <list>      payload sizes in bytes\n"
		"                    --iterations <N>   parse rounds per cell (default: 100)\n"
		"                    --warmup <N>       warm-up rounds (default: 10)\n"
		"                    --verbose          per-run detail\n"
		"                    --json             machine-readable JSON output\n"
		"                    --min-overhead     drop the on_unverified callback\n"
		"\n"
		"Exit status: 0 success; 1 the document, the input or the output failed;\n"
		"2 the command line was wrong.\n"
		"\n"
		"Examples:\n"
		"  %s validate config.bvnr\n"
		"  %s query .system.host config.bvnr\n"
		"  %s pretty-print data.bvnr\n"
		"  %s pretty-print --canonical data.bvnr\n"
		"  %s convert data.json\n"
		"  %s convert data.bvnr\n"
		"  %s events data.bvnr\n"
		"  %s events -c -d data.bvnr\n"
		"  cat data.bvnr | %s events -\n"
		"  %s frames pack a.bvnr b.bvnr > docs.bvf\n"
		"  %s frames list docs.bvf\n"
		"  %s mux pack 1:a.bvnr 2:b.bvnr | %s mux list -\n"
		"  %s bench --profile scalars --size 4096\n"
		"  %s bench --profile all --size 1024,65536 --iterations 200 --json\n",
		prog, prog, prog, prog, prog, prog, prog, prog,
		prog, prog, prog, prog, prog, prog, prog);
}
/*
 * The dispatcher. Its exit status is one of three, and which one is decided by
 * WHO was wrong:
 *
 *   0  it worked;
 *   1  the document, the input or the output failed -- a parse error, a file
 *      that cannot be read, a lossy conversion, a full disk;
 *   2  the command line was wrong -- nothing was read, and the same arguments
 *      will fail the same way on any file.
 *
 * That split existed, but not consistently: `validate` with no file exited 1
 * and with an unknown option 2, `query .a` 1 and `query .a f x` 2, a `bench`
 * typo 1 and a `bench` bad number 2, and no arguments at all 1. A script cannot
 * tell "this document is bad" from "I called it wrong" unless every usage error
 * says 2.
 */
static int cli_main(int argc, char **argv)
{
	if (argc < 2) { usage(stderr, argv[0]); return 2; }
	const char *cmd = argv[1];
	/* Extra positional arguments were silently ignored, so a typo or a shell
	 * glob that matched more than intended ("validate *.bvnr") checked only the
	 * first file and still exited 0. Refuse instead of half-doing the job. That
	 * holds for EVERY subcommand now; it was written for validate, query and
	 * pretty-print, and `events a b`, `frames list a b`, `mux list a b`,
	 * `convert a.json b.json` (which converted b) and `version x` all took
	 * the stray argument without a word. */
	if (strcmp(cmd, "validate") == 0) {
		cli_unit_policy_t up;
		memset(&up, 0, sizeof up);
		int argi = 2;
		while (argi < argc && argv[argi][0] == '-' && argv[argi][1] != '\0') {
			int rc = cli_parse_unit_opts("validate", argc, argv, &argi, &up);
			if (rc == 0) continue;
			if (rc > 0)  return rc;
			fprintf(stderr, "validate: unknown option: %s\n", argv[argi]);
			return 2;
		}
		if (argi >= argc) { fprintf(stderr, "Usage: %s validate [opts] <file>\n",
					argv[0]); return 2; }
		if (argc - argi > 1) { fprintf(stderr, "validate: takes exactly one file, "
					"got %d\n", argc - argi); return 2; }
		return cmd_validate(argv[argi], &up);
	} else if (strcmp(cmd, "query") == 0) {
		cli_unit_policy_t up;
		memset(&up, 0, sizeof up);
		int argi = 2;
		while (argi < argc && argv[argi][0] == '-' && argv[argi][1] != '\0') {
			int rc = cli_parse_unit_opts("query", argc, argv, &argi, &up);
			if (rc == 0) continue;
			if (rc > 0)  return rc;
			fprintf(stderr, "query: unknown option: %s\n", argv[argi]);
			return 2;
		}
		if (argc - argi != 2) {
			fprintf(stderr, "Usage: %s query [opts] <path> <file>\n", argv[0]);
			return 2;
		}
		return cmd_query(argv[argi], argv[argi + 1], &up);
	} else if (strcmp(cmd, "pretty-print") == 0) {
		bool canonical = false;
		const char *pp_file = NULL;
		for (int i = 2; i < argc; i++) {
			if (strcmp(argv[i], "--canonical") == 0) {
				canonical = true;
			} else if (argv[i][0] == '-' && argv[i][1] != '\0') {
				fprintf(stderr, "pretty-print: unknown option %s\n", argv[i]);
				return 2;
			} else if (pp_file) {
				fprintf(stderr, "pretty-print: takes exactly one file\n");
				return 2;
			} else {
				pp_file = argv[i];
			}
		}
		if (!pp_file) {
			fprintf(stderr, "Usage: %s pretty-print [--canonical] <file>\n", argv[0]);
			return 2;
		}
		return cmd_pretty(pp_file, canonical);
	} else if (strcmp(cmd, "convert") == 0) {
		const char *from = NULL, *to = NULL, *file = NULL;
		/* The unit-policy flags are accepted here and IGNORED, which is what
		 * doc/06 7.3 records and check_doc_policy.py pins: JSON has no units to
		 * apply a policy to. They are parsed with the same routine as
		 * everywhere else so that a flag's ARGUMENT is consumed as one --
		 * "convert --unit m x.json" used to treat `m` as a file name that the
		 * next argument happened to overwrite. Anything else beginning with
		 * '-' is refused: "every argument that is not --from/--to is the
		 * file" meant a typo was silently a file name, and two files were
		 * silently one. */
		cli_unit_policy_t ignored;
		memset(&ignored, 0, sizeof ignored);
		for (int i = 2; i < argc; ) {
			if (strcmp(argv[i], "--from") == 0 ||
			    strcmp(argv[i], "--to") == 0) {
				if (i + 1 >= argc) {
					fprintf(stderr, "convert: %s needs a value\n", argv[i]);
					return 2;
				}
				if (argv[i][2] == 'f') from = argv[i + 1];
				else                   to   = argv[i + 1];
				i += 2;
			} else if (argv[i][0] == '-' && argv[i][1] != '\0') {
				int rc = cli_parse_unit_opts("convert", argc, argv, &i,
							     &ignored);
				if (rc == 0) continue;
				if (rc > 0)  return rc;
				fprintf(stderr, "convert: unknown option: %s\n", argv[i]);
				return 2;
			} else if (file) {
				fprintf(stderr, "convert: takes exactly one file\n");
				return 2;
			} else {
				file = argv[i++];
			}
		}
		if (!file) {
			fprintf(stderr,
					"Usage: %s convert <file>            (direction from extension)\n"
					"       %s convert --from <fmt> --to <fmt> <file>\n",
					argv[0], argv[0]);
			return 2;
		}
		return cmd_convert(from, to, file);
	} else if (strcmp(cmd, "events") == 0) {
		return cmd_events(argc - 2, argv + 2);
	} else if (strcmp(cmd, "frames") == 0) {
		return cmd_frames(argc - 2, argv + 2);
	} else if (strcmp(cmd, "mux") == 0) {
		return cmd_mux(argc - 2, argv + 2);
	} else if (strcmp(cmd, "bench") == 0) {
		return cmd_bench(argc - 2, argv + 2);
	} else if (strcmp(cmd, "version") == 0 ||
	           strcmp(cmd, "--version") == 0) {
		if (argc > 2) {
			fprintf(stderr, "%s: takes no arguments\n", cmd);
			return 2;
		}
		uint16_t smaj = 0, smin = 0;
		bvnr_spec_version(&smaj, &smin);
		/* BVNR_VERSION_STRING_FULL, not bvnr_version_string(): the CLI is
		 * compiled against this header, so it can say "-dev" for a build made
		 * ahead of the release without adding a library symbol or moving the
		 * string every packaging surface compares against. See bovnar.h. */
		printf("bovnar %s (spec %u.%u)\n",
			BVNR_VERSION_STRING_FULL, smaj, smin);
		/* Which unit profiles this build carries. The vocabularies are
		 * compiled per-namespace (BVNR_WITH_*_PROFILE), so "ucum:m is not a
		 * unit" and "this build has no ucum" are the same error code from the
		 * outside; a human debugging that needs one place to look. Printed even
		 * when the list is empty, because "none" is the answer that explains
		 * the error, and a missing line explains nothing. */
		uint32_t np = bvn_unit_profile_count();
		printf("unit profiles: ");
		if (!np) {
			printf("none\n");
		} else {
			for (uint32_t i = 0; i < np; i++)
				printf("%s%s", i ? " " : "", bvn_unit_profile_name(i));
			printf("\n");
		}
		return 0;
	} else if (strcmp(cmd, "-h") == 0 || strcmp(cmd, "--help") == 0) {
		/* Help that was asked for is the command's output, so it goes to
		 * stdout: `bovnar --help | less` showed an empty pager, and
		 * `bovnar --help > help.txt` an empty file, because all of it went to
		 * stderr. Usage printed because the command line was WRONG still goes
		 * to stderr, with status 2. */
		usage(stdout, argv[0]);
		return 0;
	} else {
		fprintf(stderr, "Unknown command: %s\n", cmd);
		usage(stderr, argv[0]);
		return 2;
	}
}
int main(int argc, char **argv)
{
	/* A binary format must not be mangled by the Windows CRT's CRLF text
	 * translation on stdin/stdout/stderr; no-op on POSIX. */
	bvn_set_binary_stdio();
	int rc = cli_main(argc, argv);
	/*
	 * The one place stdout is checked.
	 *
	 * Everything the stdio subcommands print -- a query's value, validate's
	 * "OK", the JSON, the event table, a frame listing -- sits in a buffer
	 * until exit, and nothing looked at whether it ever left: `bovnar query
	 * .port cfg > /dev/full` and the same with stdout closed both exited 0
	 * having written nothing, so `port=$(bovnar query ...) || fail` carried on
	 * with an empty string. Only the commands that write through an fd sink
	 * (pretty-print, frames pack, mux pack) noticed, because the writer checks
	 * its own pushes.
	 *
	 * fflush reports the failure of THIS flush; ferror also catches one that
	 * happened mid-run, when a full buffer was pushed out early.
	 */
	errno = 0;
	if (fflush(stdout) != 0 || ferror(stdout)) {
		int e = errno;
		fprintf(stderr, "bovnar: error writing to standard output%s%s\n",
			e ? ": " : "", e ? strerror(e) : "");
		if (rc == 0) rc = 1;
	}
	return rc;
}
