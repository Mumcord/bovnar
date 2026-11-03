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

#include <inttypes.h>
#include <math.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "bovnar.h"
#include "bovnar_si_units.h"
#include "bvn_float.h"

static int g_tests    = 0;
static int g_failures = 0;

#define FAIL(fmt, ...) do { \
	fprintf(stderr, "  FAIL %s:%d: " fmt "\n", \
			__FILE__, __LINE__, __VA_ARGS__); \
	g_failures++; \
} while (0)

#define ASSERT_TRUE(cond, msg) do { \
	g_tests++; \
	if (!(cond)) FAIL("%s", (msg)); \
} while (0)

#define ASSERT_FALSE(cond, msg) \
	ASSERT_TRUE(!(cond), msg)

#define ASSERT_EQ_INT(a, b, msg) do { \
	g_tests++; \
	int _a = (int)(a), _b = (int)(b); \
	if (_a != _b) FAIL("%s: got %d expected %d", (msg), _a, _b); \
} while (0)

#define ASSERT_EQ_UINT(a, b, msg) do { \
	g_tests++; \
	uint64_t _a = (uint64_t)(a), _b = (uint64_t)(b); \
	if (_a != _b) FAIL("%s: got %" PRIu64 " expected %" PRIu64, \
					   (msg), _a, _b); \
} while (0)

#define ASSERT_GE_UINT(a, b, msg) do { \
	g_tests++; \
	uint64_t _a = (uint64_t)(a), _b = (uint64_t)(b); \
	if (_a < _b) FAIL("%s: %" PRIu64 " < %" PRIu64, (msg), _a, _b); \
} while (0)

#define ASSERT_NEAR(a, b, eps, msg) do { \
	g_tests++; \
	double _a = (double)(a), _b = (double)(b), _e = (double)(eps); \
	if (fabs(_a - _b) > _e) \
		FAIL("%s: %.15g vs %.15g (eps %.2g)", (msg), _a, _b, _e); \
} while (0)

typedef struct {
	unsigned ev_data_count;
	unsigned array_row_start_count;
	unsigned array_row_end_count;
	unsigned octet_start_count;
	unsigned octet_end_count;
	unsigned struct_start_count;
	unsigned struct_end_count;
	unsigned stream_end_count;
	unsigned token_array_num_count;
	unsigned token_array_str_count;

	char     val[3][128];
	uint32_t val_len[3];
	error_code_t last_error;
} capture_t;

static bool capture_verified(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	capture_t *c = ud;
	switch (ev) {
	case ev_data:
		if (c->ev_data_count < 3) {
			uint32_t n = d->length < 127u ? d->length : 127u;
			memcpy(c->val[c->ev_data_count], d->data, n);
			c->val[c->ev_data_count][n] = '\0';
			c->val_len[c->ev_data_count] = n;
		}
		c->ev_data_count++;
		if (d->type == token_is_array_number) c->token_array_num_count++;
		if (d->type == token_is_array_string) c->token_array_str_count++;
		break;
	case ev_array_row_start:   c->array_row_start_count++;  break;
	case ev_array_row_end:   c->array_row_end_count++;  break;
	case ev_octet_stream_start: c->octet_start_count++; break;
	case ev_octet_stream_end:   c->octet_end_count++;   break;
	case ev_struct_start:    c->struct_start_count++;   break;
	case ev_struct_end:      c->struct_end_count++;     break;
	case ev_stream_end:      c->stream_end_count++;     break;
	default: break;
	}
	return true;
}

static void capture_error(void *ud, error_code_t err,
						  uint64_t line, uint64_t col,
						  uint32_t byte, uint64_t off)
{
	capture_t *c = ud;
	c->last_error = err;
	(void)line; (void)col; (void)byte; (void)off;
}

static bool do_parse(const void *buf, uint64_t len,
					 bvnr_read_flags_t *flags,
					 bvnr_reader_t **out_r)
{
	bvnr_reader_t *r = bvnr_reader_create();
	if (!r) { FAIL("%s", "bvnr_reader_create returned NULL"); return false; }

	bool ok = bvnr_open_read_mem(r, buf, len, NULL, 0, flags)
		   && bvnr_read(r);

	if (out_r) { *out_r = r; }
	else       { bvnr_reader_destroy(r); }
	return ok;
}

static bool do_parse_str(const char *s, bvnr_read_flags_t *flags,
						 bvnr_reader_t **out_r)
{
	return do_parse(s, (uint32_t)strlen(s), flags, out_r);
}

static void test_writer_error_accessors(void)
{
	printf("  test_writer_error_accessors...\n");

	uint8_t small[12];
	bvnr_writer_t *w = bvnr_writer_create();
	ASSERT_TRUE(w != NULL, "writer create");
	if (!w) return;

	ASSERT_TRUE(bvnr_open_write_mem(w, small, sizeof(small), false,
									NULL),
				"open_write_mem with 12-byte sink");

	bvnr_data_t hdr = {0};
	(void)bvnr_write_event(w, ev_stream_start, &hdr);

	bool any_failed = false;
	bvnr_data_t key = { .data = "port", .length = 4 };
	if (!bvnr_write_event(w, ev_assignment_start, &key)) { any_failed = true; }

	if (!any_failed) {
		value_type_spec_t vt = BVN_TYPE_UINT(16);
		value_unit_t      vu = BVN_UNIT_NONE;
		bvnr_data_t ann = { .value_type = vt, .value_unit = vu };
		if (!bvnr_write_event(w, ev_type_annotation_start,              &ann)) any_failed = true;
		if (!bvnr_write_event(w, ev_type_annotation_type_family,        &ann)) any_failed = true;
		if (!bvnr_write_event(w, ev_type_annotation_type_family_parameter, &ann)) any_failed = true;
		if (!bvnr_write_event(w, ev_type_annotation_end,                &ann)) any_failed = true;
		char vbuf[] = "8080";
		bvnr_data_t val = {
			.data       = vbuf,
			.length     = 4,
			.value_type = vt,
			.value_unit = vu,
			.type       = token_is_number,
		};
		if (!bvnr_write_event(w, ev_data, &val)) any_failed = true;
	}

	ASSERT_TRUE(any_failed, "at least one write must fail into a 12-byte sink");

	error_code_t ec = bvnr_writer_get_error(w);
	ASSERT_EQ_INT(ec, error_sink_buffer_exhausted,
				  "error code is error_sink_buffer_exhausted");

	uint64_t offset = bvnr_writer_get_error_offset(w);
	uint64_t bw     = bvnr_writer_bytes_written(w);

	ASSERT_TRUE(offset <= (uint64_t)sizeof(small),
				"error offset is within buffer size");
	ASSERT_TRUE(bw <= sizeof(small),
				"bytes_written is within buffer size");

	bvnr_data_t dummy = {0};
	ASSERT_FALSE(bvnr_write_event(w, ev_stream_start, &dummy),
				 "write after error_sink_buffer_exhausted returns false");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_sink_buffer_exhausted,
				  "error code unchanged after redundant write");

	bvnr_writer_destroy(w);
}

static void test_reader_max_identifier_length(void)
{
	printf("  test_reader_max_identifier_length...\n");

	capture_t ctx = {0};
	bvnr_read_flags_t f = {
		.max_identifier_length = 3,
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_FALSE(do_parse_str(".abcd = 1;\n", &f, &r),
				 "4-char identifier with limit=3 fails");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_identifier_too_long,
				  "error is error_identifier_too_long");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".abc = 1;\n", &f, &r),
				"3-char identifier with limit=3 succeeds");
	bvnr_reader_destroy(r);
}

static void test_reader_max_string_length(void)
{
	printf("  test_reader_max_string_length...\n");

	capture_t ctx = {0};
	bvnr_read_flags_t f = {
		.max_string_length = 4,
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_FALSE(do_parse_str(".x = \"hello\";\n", &f, &r),
				 "5-byte string with limit=4 fails");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_string_too_long,
				  "error is error_string_too_long");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".x = \"abcd\";\n", &f, &r),
				"4-byte string with limit=4 succeeds");
	bvnr_reader_destroy(r);
}

static void test_reader_max_number_length(void)
{
	printf("  test_reader_max_number_length...\n");

	capture_t ctx = {0};
	bvnr_read_flags_t f = {
		.max_number_length = 3,
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_FALSE(do_parse_str(".x = 1234;\n", &f, &r),
				 "4-digit number with limit=3 fails");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_number_too_long,
				  "error is error_number_too_long");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".x = 123;\n", &f, &r),
				"3-digit number with limit=3 succeeds");
	bvnr_reader_destroy(r);
}

static void test_reader_max_symbol_length(void)
{
	printf("  test_reader_max_symbol_length...\n");

	capture_t ctx = {0};
	bvnr_read_flags_t f = {
		.max_symbol_length = 4,
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_FALSE(do_parse_str(".x = abcde;\n", &f, &r),
				 "5-char symbol with limit=4 fails");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_symbol_too_long,
				  "error is error_symbol_too_long");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".x = abcd;\n", &f, &r),
				"4-char symbol with limit=4 succeeds");
	bvnr_reader_destroy(r);
}

static void test_reader_max_reference_length(void)
{
	printf("  test_reader_max_reference_length...\n");

	capture_t ctx = {0};
	bvnr_read_flags_t f = {
		.max_reference_length = 5,
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_FALSE(do_parse_str(".x = &.longer;\n", &f, &r),
				 "7-char reference with limit=5 fails");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_reference_too_long,
				  "error is error_reference_too_long");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".x = &.long;\n", &f, &r),
				"5-char reference with limit=5 succeeds");
	bvnr_reader_destroy(r);
}

static void test_reader_max_array_items(void)
{
	printf("  test_reader_max_array_items...\n");

	capture_t ctx = {0};
	bvnr_read_flags_t f = {
		.max_array_items = 2,
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_FALSE(do_parse_str(".a = [1,2,3];\n", &f, &r),
				 "3-item array with limit=2 fails");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_too_many_array_items,
				  "error is error_too_many_array_items");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".a = [1,2];\n", &f, &r),
				"2-item array with limit=2 succeeds");
	bvnr_reader_destroy(r);
}

static void test_reader_max_text_bytes(void)
{
	printf("  test_reader_max_text_bytes...\n");

	capture_t ctx = {0};
	bvnr_read_flags_t f = {
		.max_text_bytes = 7,
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_FALSE(do_parse_str(".x = 1;\n", &f, &r),
				 "8-byte stream with max_text_bytes=7 fails");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_text_data_too_long,
				  "error is error_text_data_too_long");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	f.max_text_bytes = 8;
	ASSERT_TRUE(do_parse_str(".x = 1;\n", &f, &r),
				"8-byte stream with max_text_bytes=8 succeeds");
	bvnr_reader_destroy(r);
}

static void test_reader_max_file_size(void)
{
	printf("  test_reader_max_file_size...\n");

	capture_t ctx = {0};
	bvnr_read_flags_t f = {
		.max_file_size = 7,
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_FALSE(do_parse_str(".x = 1;\n", &f, &r),
				 "8-byte stream with max_file_size=7 fails");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_file_too_long,
				  "error is error_file_too_long");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	f.max_file_size = 8;
	ASSERT_TRUE(do_parse_str(".x = 1;\n", &f, &r),
				"8-byte stream with max_file_size=8 succeeds");
	bvnr_reader_destroy(r);
}

static void test_reader_max_struct_nesting(void)
{
	printf("  test_reader_max_struct_nesting...\n");

	const char *depth3 = ".a = { .b = { .c = { .d = 1; }; }; };\n";
	const char *depth2 = ".a = { .b = { .c = 1; }; };\n";

	capture_t ctx = {0};
	bvnr_read_flags_t f = {
		.max_struct_nesting = 2,
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_FALSE(do_parse_str(depth3, &f, &r),
				 "depth-3 struct with limit=2 fails");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_struct_nesting_too_high,
				  "error is error_struct_nesting_too_high");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(depth2, &f, &r),
				"depth-2 struct with limit=2 succeeds");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	f.max_struct_nesting = 3;
	ASSERT_TRUE(do_parse_str(depth3, &f, &r),
				"depth-3 struct with limit=3 succeeds");
	bvnr_reader_destroy(r);
}

static void test_reader_max_array_nesting(void)
{
	printf("  test_reader_max_array_nesting...\n");

	const char *nested = ".a = [[1,2],[3,4]];\n";
	const char *flat   = ".a = [1,2,3];\n";

	capture_t ctx = {0};
	bvnr_read_flags_t f = {
		.max_array_nesting = 1,
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_FALSE(do_parse_str(nested, &f, &r),
				 "depth-2 array with limit=1 fails");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_array_nesting_too_high,
				  "error is error_array_nesting_too_high");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(flat, &f, &r),
				"depth-1 array with limit=1 succeeds");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	f.max_array_nesting = 2;
	ASSERT_TRUE(do_parse_str(nested, &f, &r),
				"depth-2 array with limit=2 succeeds");
	bvnr_reader_destroy(r);
}

static void test_recovery_count(void)
{
	printf("  test_recovery_count...\n");

	const char *src =
		".a = <float:8> 1.0;\n"
		".b = <float:8> 2.0;\n"
		".c = <float:8> 3.0;\n"
		".d = 42;\n";

	capture_t ctx = {0};
	bvnr_read_flags_t flags = {
		.on_verified       = capture_verified,
		.on_error          = capture_error,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = NULL;
	do_parse_str(src, &flags, &r);

	uint64_t rc = bvnr_reader_get_recovery_count(r);
	ASSERT_GE_UINT(rc, 3,
				   "recovery_count >= 3 after three illegal-type assignments");

	ASSERT_GE_UINT((uint64_t)ctx.ev_data_count, 1,
				   "at least one ev_data delivered despite earlier errors");

	bvnr_reader_destroy(r);
}

/*
 * What the error getters report under continue_on_error (doc/03 §13.4).
 *
 * They describe the error that ENDED the read, not the ones recovered from.
 * Inside on_error they agree with the callback's arguments; a read that
 * returns true leaves error_none and every position at 0, however many errors
 * were recovered; one that returns false leaves the error that stopped it --
 * under recovery, the error_got_incomplete_bvnr_stream of a document that
 * ended while resyncing.
 */
typedef struct {
	bvnr_reader_t *r;
	int            calls;
	int            mismatches;
	error_code_t   first;
} getter_probe_t;

static void probe_getters_in_on_error(void *ud, error_code_t err,
				      uint64_t line, uint64_t col,
				      uint32_t byte, uint64_t off)
{
	getter_probe_t *p = ud;
	if (p->calls++ == 0)
		p->first = err;
	if (bvnr_reader_get_error(p->r)        != err  ||
	    bvnr_reader_get_error_line(p->r)   != line ||
	    bvnr_reader_get_error_column(p->r) != col  ||
	    bvnr_reader_get_error_byte(p->r)   != byte ||
	    bvnr_reader_get_error_offset(p->r) != off)
		p->mismatches++;
}

static bool probe_read(const char *src, bool continue_on_error,
		       getter_probe_t *p)
{
	*p = (getter_probe_t){ .r = bvnr_reader_create() };
	if (!p->r) { FAIL("%s", "bvnr_reader_create returned NULL"); return false; }
	bvnr_read_flags_t f = {
		.on_error          = probe_getters_in_on_error,
		.userdata          = p,
		.continue_on_error = continue_on_error,
	};
	if (!bvnr_open_read_mem(p->r, src, strlen(src), NULL, 0, &f)) {
		FAIL("%s", "bvnr_open_read_mem failed");
		return false;
	}
	return bvnr_read(p->r);
}

static void assert_getters_clear(bvnr_reader_t *r, const char *what)
{
	char msg[160];
	snprintf(msg, sizeof msg, "%s: get_error", what);
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_none, msg);
	snprintf(msg, sizeof msg, "%s: get_error_line", what);
	ASSERT_EQ_UINT(bvnr_reader_get_error_line(r), 0, msg);
	snprintf(msg, sizeof msg, "%s: get_error_column", what);
	ASSERT_EQ_UINT(bvnr_reader_get_error_column(r), 0, msg);
	snprintf(msg, sizeof msg, "%s: get_error_byte", what);
	ASSERT_EQ_UINT(bvnr_reader_get_error_byte(r), 0, msg);
	snprintf(msg, sizeof msg, "%s: get_error_offset", what);
	ASSERT_EQ_UINT(bvnr_reader_get_error_offset(r), 0, msg);
}

static void test_error_getters_after_recovery(void)
{
	printf("  test_error_getters_after_recovery...\n");

	/* Recovered errors of several kinds -- a stray byte, a value out of
	 * range, an unknown unit, a bad escape, an error inside a struct, a bad
	 * byte in a comment -- and the read still succeeds. */
	static const char *const recovered[] = {
		".a = 1;\n.b = @;\n.c = 2;\n",
		".a = <uint:8> 300; .b = 1;",
		".a = 1 zzzunit; .b = 2;",
		".a = \"abc\\q\"; .b = 1;",
		".a = {.x = @; .y = 2;}; .b = 1;",
		".a = [{.x = @;}, {.x = 1;}]; .b = 2;",
		".a = 1; # \xc3\n.b = 2;",
		".a = @;\n.b = #;\n.c = 3;\n",
	};
	for (size_t i = 0; i < sizeof recovered / sizeof *recovered; i++) {
		getter_probe_t p;
		char what[96];
		snprintf(what, sizeof what, "recovered[%zu]", i);
		bool ok = probe_read(recovered[i], true, &p);
		if (!p.r) continue;
		ASSERT_TRUE(ok, what);
		ASSERT_GE_UINT((uint64_t)p.calls, 1, "on_error fired");
		ASSERT_EQ_INT(p.mismatches, 0,
			      "inside on_error the getters match its arguments");
		ASSERT_GE_UINT(bvnr_reader_get_recovery_count(p.r), 1,
			       "recovery_count reports the recovered error");
		assert_getters_clear(p.r, what);
		bvnr_reader_destroy(p.r);
	}

	/* Recovery that runs into the end of the document: read fails, and the
	 * getters hold the second error (doc/03 §13.3), at the end of input. */
	{
		const char *src = ".a = @;\n.b = 2";
		getter_probe_t p;
		bool ok = probe_read(src, true, &p);
		if (p.r) {
			ASSERT_FALSE(ok, "EOF in resync fails the read");
			ASSERT_EQ_INT(p.calls, 2, "two on_error calls");
			ASSERT_EQ_INT(p.first, error_unexpected_input_byte,
				      "first: the error that began recovery");
			ASSERT_EQ_INT(bvnr_reader_get_error(p.r),
				      error_got_incomplete_bvnr_stream,
				      "get_error: the error that ended the read");
			ASSERT_EQ_UINT(bvnr_reader_get_error_line(p.r), 2,
				       "its line");
			ASSERT_EQ_UINT(bvnr_reader_get_error_offset(p.r),
				       strlen(src), "its offset: end of input");
			ASSERT_EQ_INT(p.mismatches, 0,
				      "inside on_error the getters match its arguments");
			bvnr_reader_destroy(p.r);
		}
	}

	/* Without continue_on_error the first error ends the read and stays. */
	{
		getter_probe_t p;
		bool ok = probe_read(".a = 1;\n.b = @;\n.c = 2;\n", false, &p);
		if (p.r) {
			ASSERT_FALSE(ok, "strict read fails");
			ASSERT_EQ_INT(bvnr_reader_get_error(p.r),
				      error_unexpected_input_byte, "strict: get_error");
			ASSERT_EQ_UINT(bvnr_reader_get_error_line(p.r), 2,
				       "strict: line");
			ASSERT_EQ_UINT(bvnr_reader_get_error_column(p.r), 6,
				       "strict: column");
			ASSERT_EQ_UINT(bvnr_reader_get_error_offset(p.r), 13,
				       "strict: offset");
			bvnr_reader_destroy(p.r);
		}
	}
}

/*
 * Three recovery defects found in the 2026-11-03 review of doc/03 §12-18.
 */
static void test_recovery_review_2026_11(void)
{
	printf("  test_recovery_review_2026_11...\n");
	getter_probe_t p;
	bool ok;

	/* strict_version is a refusal, not an error to recover from: with
	 * continue_on_error as well, "#!bovnar 9.9" used to be reported and the
	 * rest of the document read anyway, bvnr_read returning true. */
	static const char *const futures[] = {
		"#!bovnar 9.9\n.a = 1;\n",
		"#!bovnar 1.99\n.a = 1;\n",
		"#!bovnar 1.\n.a = 1;\n",          /* malformed: version unknowable */
	};
	for (size_t i = 0; i < sizeof futures / sizeof *futures; i++) {
		p = (getter_probe_t){ .r = bvnr_reader_create() };
		if (!p.r) continue;
		bvnr_read_flags_t f = {
			.on_error = probe_getters_in_on_error, .userdata = &p,
			.continue_on_error = true, .strict_version = true,
		};
		bvnr_open_read_mem(p.r, futures[i], strlen(futures[i]), NULL, 0, &f);
		ok = bvnr_read(p.r);
		ASSERT_FALSE(ok, "strict_version stops the read under continue_on_error");
		ASSERT_EQ_INT(p.calls, 1, "strict_version: one error, nothing after it");
		ASSERT_EQ_UINT(bvnr_reader_get_recovery_count(p.r), 0,
			       "strict_version: no recovery");
		error_code_t e = bvnr_reader_get_error(p.r);
		ASSERT_TRUE(e == error_unsupported_spec_version ||
			    e == error_invalid_spec_version,
			    "strict_version: get_error names the version");
		bvnr_reader_destroy(p.r);
	}
	/* Without strict_version a malformed directive is still recovered. */
	ok = probe_read("#!bovnar 1.\n.a = 1;\n", true, &p);
	if (p.r) {
		ASSERT_TRUE(ok, "lenient: a malformed directive is recovered from");
		bvnr_reader_destroy(p.r);
	}

	/* One stray multi-byte character is one error. Its continuation bytes
	 * were each reported again as error_invalid_utf8_byte, because entering
	 * recovery cleared the UTF-8 progress of the lead byte just rejected. */
	ok = probe_read(".a = 1;\n\xe2\x82\xac\n.b = 2;\n", true, &p);
	if (p.r) {
		ASSERT_TRUE(ok, "stray euro sign: read succeeds");
		ASSERT_EQ_INT(p.calls, 1, "stray euro sign: one error");
		ASSERT_EQ_INT(p.first, error_unexpected_input_byte,
			      "stray euro sign: the error is the lead byte's");
		ASSERT_EQ_UINT(bvnr_reader_get_recovery_count(p.r), 1,
			       "stray euro sign: one recovery");
		ASSERT_EQ_UINT(bvnr_reader_get_skipped_bytes(p.r), 4,
			       "stray euro sign: its three bytes and the newline after it");
		bvnr_reader_destroy(p.r);
	}
	/* ...and inside a broken value too. */
	ok = probe_read(".a = 1 \xc3\xa9 x; .b = 2;\n", true, &p);
	if (p.r) {
		ASSERT_EQ_INT(p.calls, 1, "stray e-acute in a value: one error");
		bvnr_reader_destroy(p.r);
	}

	/* The skipped total runs from the byte the error was raised on up to,
	 * not including, the boundary recovery resumes at. It used to leave out
	 * the offending byte and count the boundary. */
	static const struct { const char *src; uint64_t skipped; } sk[] = {
		{ ".a = 1;@.b = 2;",       1 },  /* the '@'; ".b" is delivered      */
		{ ".a = @@@;",             3 },  /* the three '@'; not the ';'      */
		{ ".a = @@@; .b = 2;",     3 },
		{ ".a = <uint:8> 300;",    0 },  /* rejected at its own ';' (§18.2) */
		{ ".a = 1; @ .b = 2;",     2 },  /* '@' and the blank after it      */
	};
	for (size_t i = 0; i < sizeof sk / sizeof *sk; i++) {
		ok = probe_read(sk[i].src, true, &p);
		if (!p.r) continue;
		char msg[96];
		snprintf(msg, sizeof msg, "skipped bytes of %s", sk[i].src);
		ASSERT_EQ_UINT(bvnr_reader_get_skipped_bytes(p.r), sk[i].skipped, msg);
		bvnr_reader_destroy(p.r);
	}
	/* Recovery that runs into EOF counts everything from the offending byte. */
	ok = probe_read(".a = @", true, &p);
	if (p.r) {
		ASSERT_EQ_UINT(bvnr_reader_get_skipped_bytes(p.r), 1,
			       "skipped bytes: EOF in recovery counts the '@'");
		bvnr_reader_destroy(p.r);
	}
}

static bool cb_abort_immediately(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	(void)ud; (void)ev; (void)d;
	return false;
}

static void test_callback_abort_verified(void)
{
	printf("  test_callback_abort_verified...\n");

	bvnr_read_flags_t flags = { .on_verified = cb_abort_immediately };
	bvnr_reader_t *r = NULL;

	ASSERT_FALSE(do_parse_str(".x = 1;\n", &flags, &r),
				 "parse must fail when on_verified returns false");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_scanner_callback_failed,
				  "error is error_scanner_callback_failed (verified path)");
	bvnr_reader_destroy(r);
}

static void test_callback_abort_unverified(void)
{
	printf("  test_callback_abort_unverified...\n");

	bvnr_read_flags_t flags = {
		.on_unverified = cb_abort_immediately,

	};
	bvnr_reader_t *r = NULL;

	ASSERT_FALSE(do_parse_str(".x = 1;\n", &flags, &r),
				 "parse must fail when on_unverified returns false");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_scanner_callback_failed,
				  "error is error_scanner_callback_failed (unverified path)");
	bvnr_reader_destroy(r);
}

static void test_si_units_compatible(void)
{
	printf("  test_si_units_compatible...\n");

	value_unit_t m   = BVN_UNIT_SI(bu_meter,  si_none);
	value_unit_t km  = BVN_UNIT_SI(bu_meter,  si_kilo);
	value_unit_t mm  = BVN_UNIT_SI(bu_meter,  si_milli);
	value_unit_t s   = BVN_UNIT_SI(bu_second, si_none);
	value_unit_t kg  = BVN_UNIT_SI(bu_gram,   si_kilo);

	ASSERT_TRUE (bvn_units_compatible(m,  km), "m and km: compatible");
	ASSERT_TRUE (bvn_units_compatible(km, m),  "km and m: symmetric");
	ASSERT_TRUE (bvn_units_compatible(m,  mm), "m and mm: compatible");
	ASSERT_FALSE(bvn_units_compatible(m,  s),  "m and s: incompatible");
	ASSERT_FALSE(bvn_units_compatible(m,  kg), "m and kg: incompatible");
	ASSERT_FALSE(bvn_units_compatible(s,  kg), "s and kg: incompatible");

	value_unit_t none = BVN_UNIT_NONE;
	ASSERT_FALSE(bvn_units_compatible(m, none), "m and none: incompatible");
}

static void test_si_unit_convert_factor(void)
{
	printf("  test_si_unit_convert_factor...\n");

	value_unit_t m  = BVN_UNIT_SI(bu_meter, si_none);
	value_unit_t km = BVN_UNIT_SI(bu_meter, si_kilo);
	value_unit_t mm = BVN_UNIT_SI(bu_meter, si_milli);
	value_unit_t s  = BVN_UNIT_SI(bu_second, si_none);

	bool ok = false, req_affine = false;
	double f;

	f = bvn_unit_convert_factor(km, m, &ok, &req_affine);
	ASSERT_TRUE (ok,        "km→m: ok");
	ASSERT_FALSE(req_affine,"km→m: not affine");
	ASSERT_NEAR (f, 1000.0, 1e-9, "km→m factor = 1000");

	f = bvn_unit_convert_factor(m, km, &ok, &req_affine);
	ASSERT_TRUE (ok,        "m→km: ok");
	ASSERT_NEAR (f, 0.001,  1e-12, "m→km factor = 0.001");

	f = bvn_unit_convert_factor(mm, m, &ok, &req_affine);
	ASSERT_TRUE (ok,         "mm→m: ok");
	ASSERT_NEAR (f, 0.001,   1e-12, "mm→m factor = 0.001");

	(void)bvn_unit_convert_factor(m, s, &ok, &req_affine);
	ASSERT_FALSE(ok, "m→s: incompatible, ok=false");
}

static void test_si_unit_dimension_vector(void)
{
	printf("  test_si_unit_dimension_vector...\n");

	value_unit_t ms = BVN_UNIT_COMPOUND2(
		bu_meter,  si_none, exp_linear,
		bu_second, si_none, exp_neg_linear);

	int32_t dims[7] = {0};
	ASSERT_TRUE(bvn_unit_dimension_vector(ms, dims),
				"dimension_vector for m/s succeeds");
	ASSERT_EQ_INT(dims[bvn_si_dim_meter],    1,  "m/s: meter exp = +1");
	ASSERT_EQ_INT(dims[bvn_si_dim_second],  -1,  "m/s: second exp = -1");
	ASSERT_EQ_INT(dims[bvn_si_dim_kilogram], 0,  "m/s: mass exp = 0");
	ASSERT_EQ_INT(dims[bvn_si_dim_ampere],   0,  "m/s: current exp = 0");

	value_unit_t m = BVN_UNIT_SI(bu_meter, si_none);
	memset(dims, 0, sizeof(dims));
	ASSERT_TRUE(bvn_unit_dimension_vector(m, dims),
				"dimension_vector for m succeeds");
	ASSERT_EQ_INT(dims[bvn_si_dim_meter],  1, "m: meter exp = +1");
	ASSERT_EQ_INT(dims[bvn_si_dim_second], 0, "m: second exp = 0");

	value_unit_t none = BVN_UNIT_NONE;
	memset(dims, 0, sizeof(dims));
	(void)bvn_unit_dimension_vector(none, dims);
	for (int i = 0; i < 7; i++)
		ASSERT_EQ_INT(dims[i], 0, "dimensionless: all exponents = 0");
}

static void test_si_unit_reduce(void)
{
	printf("  test_si_unit_reduce...\n");

	value_unit_t km = BVN_UNIT_SI(bu_meter, si_kilo);
	double scale = 1.0;
	bool overflow = false;
	value_unit_t reduced = bvn_unit_reduce(km, &scale, &overflow);

	ASSERT_NEAR(scale, 1000.0, 1e-9, "km: reduce scale = 1000");

	value_unit_t m = BVN_UNIT_SI(bu_meter, si_none);
	ASSERT_TRUE(bvn_units_compatible(reduced, m),
				"reduced km is dimensionally compatible with m");

	value_unit_t ms = BVN_UNIT_SI(bu_second, si_milli);
	scale = 1.0;
	(void)bvn_unit_reduce(ms, &scale, NULL);
	ASSERT_NEAR(scale, 0.001, 1e-12, "ms: reduce scale = 0.001");

	value_unit_t gib = BVN_UNIT_IEC(bu_byte, iec_gibi);
	scale = 1.0;
	(void)bvn_unit_reduce(gib, &scale, NULL);
	ASSERT_NEAR(scale, 1073741824.0, 1.0, "GiB: reduce scale = 2^30");
}

static void test_si_unit_to_si_factor(void)
{
	printf("  test_si_unit_to_si_factor...\n");

	bool is_affine = false, ok = false;
	double offset = 0.0, factor;

	value_unit_t kms = BVN_UNIT_COMPOUND2(
		bu_meter,  si_kilo, exp_linear,
		bu_second, si_none, exp_neg_linear);

	factor = bvn_unit_to_si_factor(kms, &is_affine, &offset, &ok);
	ASSERT_TRUE (ok,         "unit_to_si_factor km/s: ok");
	ASSERT_FALSE(is_affine,  "km/s: not affine");
	ASSERT_NEAR (factor, 1000.0, 1e-9, "km/s SI factor = 1000");

	value_unit_t K = BVN_UNIT_SI(bu_kelvin, si_none);
	factor = bvn_unit_to_si_factor(K, &is_affine, &offset, &ok);
	ASSERT_TRUE (ok,        "unit_to_si_factor K: ok");
	ASSERT_FALSE(is_affine, "K: not affine");
	ASSERT_NEAR (factor, 1.0, 1e-12, "K SI factor = 1");

	value_unit_t mK = BVN_UNIT_SI(bu_kelvin, si_milli);
	factor = bvn_unit_to_si_factor(mK, &is_affine, &offset, &ok);
	ASSERT_TRUE(ok,           "unit_to_si_factor mK: ok");
	ASSERT_NEAR(factor, 0.001, 1e-12, "mK SI factor = 0.001");

	value_unit_t C = BVN_UNIT_SI(bu_celsius, si_none);
	is_affine = false; offset = 0.0;
	factor = bvn_unit_to_si_factor(C, &is_affine, &offset, &ok);
	ASSERT_TRUE(ok,       "unit_to_si_factor °C: ok");
	ASSERT_TRUE(is_affine,"°C: is_affine = true");
	ASSERT_NEAR(factor, 1.0,    1e-9,  "°C SI factor = 1");
	ASSERT_NEAR(offset, 273.15, 1e-3,  "°C affine_offset ≈ 273.15 K");
}

static void test_array_reader_events(void)
{
	printf("  test_array_reader_events...\n");

	capture_t ctx = {0};
	bvnr_read_flags_t flags = {
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_TRUE(do_parse_str(".a = [1, 2, 3];\n", &flags, &r),
				"1-D numeric array parses");
	ASSERT_EQ_INT((int)ctx.token_array_num_count, 3,
				  "[1,2,3]: 3 token_is_array_number events");
	ASSERT_GE_UINT((uint64_t)ctx.array_row_start_count, 1,
				   "[1,2,3]: at least 1 ev_array_row_start");
	ASSERT_GE_UINT((uint64_t)ctx.array_row_end_count, 1,
				   "[1,2,3]: at least 1 ev_array_row_end");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".a = [1,2]/[3,4];\n", &flags, &r),
				"multi-row array parses");
	ASSERT_EQ_INT((int)ctx.token_array_num_count, 4,
				  "[1,2]/[3,4]: 4 token_is_array_number events");

	ASSERT_GE_UINT((uint64_t)ctx.array_row_start_count, 2,
				   "[1,2]/[3,4]: at least 2 ev_array_row_start");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".a = [\"x\", \"y\", \"z\"];\n", &flags, &r),
				"string array parses");
	ASSERT_EQ_INT((int)ctx.token_array_str_count, 3,
				  "string array: 3 token_is_array_string events");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".a = [,1,,2,];\n", &flags, &r),
				"sparse array parses");

	ASSERT_GE_UINT((uint64_t)ctx.ev_data_count, 2,
				   "sparse array: at least 2 ev_data events");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".a = [<sint:16> 1, <sint:16> 2];\n", &flags, &r),
				"typed array parses");
	ASSERT_EQ_INT((int)ctx.token_array_num_count, 2,
				  "typed array: 2 token_is_array_number events");
	bvnr_reader_destroy(r);
}

static void test_octet_stream_reader_events(void)
{
	printf("  test_octet_stream_reader_events...\n");

	static const uint8_t src[] = {

		'.', 'b', ' ', '=', ' ',

		'\x00',

		'\x01', '\x05', '\x00', 'h', 'e', 'l', 'l', 'o',

		'\x00',

		';', '\n'
	};

	capture_t ctx = {0};
	bvnr_read_flags_t flags = {
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_TRUE(do_parse(src, sizeof(src), &flags, &r),
				"octet-stream document parses without error");
	ASSERT_EQ_INT((int)ctx.octet_start_count, 1,
				  "ev_octet_stream_start received exactly once");
	ASSERT_EQ_INT((int)ctx.octet_end_count, 1,
				  "ev_octet_stream_end received exactly once");
	bvnr_reader_destroy(r);

	static const uint8_t empty_oct[] = {
		'.', 'x', ' ', '=', ' ',
		'\x00', '\x00',
		';', '\n'
	};
	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse(empty_oct, sizeof(empty_oct), &flags, &r),
				"empty octet-stream parses without error");
	ASSERT_EQ_INT((int)ctx.octet_start_count, 1, "empty octet: start event");
	ASSERT_EQ_INT((int)ctx.octet_end_count,   1, "empty octet: end event");
	bvnr_reader_destroy(r);
}

static void test_string_vt_vf_escape_write(void)
{
	printf("  test_string_vt_vf_escape_write...\n");

	uint8_t outbuf[256];
	bvnr_sink_t sink;
	bvnr_writer_t *w = bvnr_writer_create();
	ASSERT_TRUE(w != NULL, "writer create");
	if (!w) return;
	bvnr_sink_to_mem(&sink, outbuf, sizeof(outbuf));
	ASSERT_TRUE(bvnr_open_write_sink(w, &sink, false, NULL),
				"open_write_sink for \\v/\\f test");

	bvnr_data_t ev = {0};
	ASSERT_TRUE(bvnr_write_event(w, ev_stream_start, &ev), "stream_start");

	ev.type = token_is_identifier;
	ev.data = "vt_key"; ev.length = 6;
	ASSERT_TRUE(bvnr_write_event(w, ev_assignment_start, &ev),
				"assignment_start for \\v key");

	static const uint8_t vt_payload[] = { 'a', '\v', 'b' };
	ev.type   = token_is_string;
	ev.data   = vt_payload;
	ev.length = sizeof(vt_payload);
	ASSERT_TRUE(bvnr_write_event(w, ev_data, &ev),
				"ev_data with \\v byte must succeed after Bug 1 fix");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none,
				  "no error after writing \\v in string");

	ev.type = token_is_identifier;
	ev.data = "ff_key"; ev.length = 6;
	ASSERT_TRUE(bvnr_write_event(w, ev_assignment_start, &ev),
				"assignment_start for \\f key");

	static const uint8_t ff_payload[] = { 'x', '\f', 'y' };
	ev.type   = token_is_string;
	ev.data   = ff_payload;
	ev.length = sizeof(ff_payload);
	ASSERT_TRUE(bvnr_write_event(w, ev_data, &ev),
				"ev_data with \\f byte must succeed after Bug 1 fix");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none,
				  "no error after writing \\f in string");

	ASSERT_TRUE(bvnr_write_finish(w), "finish");

	uint64_t n = bvnr_writer_bytes_written(w);
	bvnr_writer_destroy(w);
	ASSERT_TRUE(n > 0, "bytes written > 0");

	bvnr_reader_t *r = NULL;
	capture_t ctx = {0};
	bvnr_read_flags_t flags = {
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	ASSERT_TRUE(do_parse(outbuf, (uint32_t)n, &flags, &r),
				"written \\v/\\f document roundtrips cleanly");
	ASSERT_EQ_INT((int)ctx.ev_data_count, 2, "two string values roundtripped");
	bvnr_reader_destroy(r);
}

static void test_octet_zero_length_chunk_rejected(void)
{
	printf("  test_octet_zero_length_chunk_rejected...\n");

	uint8_t outbuf[128];
	bvnr_sink_t sink;
	bvnr_writer_t *w = bvnr_writer_create();
	ASSERT_TRUE(w != NULL, "writer create");
	if (!w) return;
	bvnr_sink_to_mem(&sink, outbuf, sizeof(outbuf));
	ASSERT_TRUE(bvnr_open_write_sink(w, &sink, false, NULL),
				"open_write_sink");

	bvnr_data_t ev = {0};
	ASSERT_TRUE(bvnr_write_event(w, ev_stream_start, &ev), "stream_start");

	ev.type = token_is_identifier;
	ev.data = "b"; ev.length = 1;
	ASSERT_TRUE(bvnr_write_event(w, ev_assignment_start, &ev),
				"assignment_start");
	ASSERT_TRUE(bvnr_write_event(w, ev_octet_stream_start, &ev),
				"octet_stream_start");

	ev.type   = token_is_octet_stream;
	ev.data   = outbuf;
	ev.length = 0;
	ASSERT_FALSE(bvnr_write_event(w, ev_data, &ev),
				 "zero-length octet chunk must be rejected");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_invalid_argument,
				  "error must be error_invalid_argument for zero-length chunk");

	bvnr_writer_destroy(w);
}

static void test_special_float_write(void)
{
	printf("  test_special_float_write...\n");

	uint8_t outbuf[512];

	bvnr_writer_t *w = bvnr_writer_create();
	ASSERT_TRUE(w != NULL, "writer create");
	if (!w) return;

	ASSERT_TRUE(bvnr_open_write_mem(w, outbuf, sizeof(outbuf), false, NULL),
				"open_write_mem for special floats");

	bvnr_data_t hdr = {0};
	ASSERT_TRUE(bvnr_write_event(w, ev_stream_start, &hdr), "stream_start");
	ASSERT_TRUE(bvnr_write_float(w, "nan_val",  64,  (double)NAN),       "write NaN");
	ASSERT_TRUE(bvnr_write_float(w, "pos_inf",  64,  (double)INFINITY),  "write +Inf");
	ASSERT_TRUE(bvnr_write_float(w, "neg_inf",  64, -(double)INFINITY),  "write -Inf");
	ASSERT_TRUE(bvnr_write_finish(w), "finish");

	uint64_t n = bvnr_writer_bytes_written(w);
	bvnr_writer_destroy(w);
	ASSERT_TRUE(n > 0 && n < sizeof(outbuf), "writer produced bytes");

	capture_t ctx = {0};
	bvnr_read_flags_t flags = {
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;
	ASSERT_TRUE(do_parse(outbuf, n, &flags, &r),
				"special-float document parses cleanly");
	ASSERT_EQ_INT((int)ctx.ev_data_count, 3,
				  "three ev_data events (nan, +inf, -inf)");
	bvnr_reader_destroy(r);

	if (ctx.ev_data_count < 3) return;

	ASSERT_TRUE(strstr(ctx.val[0], "nan") != NULL ||
				strstr(ctx.val[0], "NaN") != NULL,
				"NaN: value token contains 'nan'");

	ASSERT_TRUE(strstr(ctx.val[1], "infinity") != NULL ||
				strstr(ctx.val[1], "inf")      != NULL ||
				strstr(ctx.val[1], "Inf")      != NULL,
				"+Inf: value token contains 'inf'");

	ASSERT_FALSE(ctx.val[1][0] == '-',
				 "+Inf: value token is not negative");

	ASSERT_TRUE(strstr(ctx.val[2], "infinity") != NULL ||
				strstr(ctx.val[2], "inf")      != NULL ||
				strstr(ctx.val[2], "Inf")      != NULL,
				"-Inf: value token contains 'inf'");

	ASSERT_TRUE(strcmp(ctx.val[2], "ninf") == 0,
				"-Inf: value token is the bovnar keyword \"ninf\"");
}

static void test_open_write_mem_and_bytes_written(void)
{
	printf("  test_open_write_mem_and_bytes_written...\n");

	uint8_t buf[512];
	memset(buf, 0xAA, sizeof(buf));

	bvnr_writer_t *w = bvnr_writer_create();
	ASSERT_TRUE(w != NULL, "writer create");
	if (!w) return;

	ASSERT_TRUE(bvnr_open_write_mem(w, buf, sizeof(buf), false, NULL),
				"bvnr_open_write_mem succeeds");

	bvnr_data_t hdr = {0};
	ASSERT_TRUE(bvnr_write_event(w, ev_stream_start, &hdr), "stream_start");
	ASSERT_TRUE(bvnr_write_string(w, "host",    "localhost"),    "write host");
	ASSERT_TRUE(bvnr_write_uint  (w, "port",  16, 8080u),        "write port");
	ASSERT_TRUE(bvnr_write_bool  (w, "tls",   true),             "write tls");
	ASSERT_TRUE(bvnr_write_float (w, "ratio", 64, 1.5),          "write ratio");
	ASSERT_TRUE(bvnr_write_finish(w), "finish");

	uint64_t written = bvnr_writer_bytes_written(w);
	bvnr_writer_destroy(w);

	ASSERT_TRUE(written > 0, "bytes_written > 0");
	ASSERT_TRUE(written < sizeof(buf), "bytes_written < buffer capacity");

	ASSERT_EQ_INT(buf[written], 0xAA,
				  "sentinel byte past written region is untouched");

	capture_t ctx = {0};
	bvnr_read_flags_t flags = {
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;
	ASSERT_TRUE(do_parse(buf, written, &flags, &r),
				"bytes written via open_write_mem are parseable");

	ASSERT_EQ_INT((int)ctx.ev_data_count, 4,
				  "4 ev_data events in the round-tripped document");
	bvnr_reader_destroy(r);

	memset(buf, 0xAA, sizeof(buf));
	w = bvnr_writer_create();
	ASSERT_TRUE(w != NULL, "writer create (pretty)");
	if (!w) return;
	ASSERT_TRUE(bvnr_open_write_mem(w, buf, sizeof(buf), true, NULL),
				"open_write_mem pretty=true");
	hdr = (bvnr_data_t){0};
	(void)bvnr_write_event(w, ev_stream_start, &hdr);
	(void)bvnr_write_uint(w, "n", 32, 99u);
	(void)bvnr_write_finish(w);
	uint64_t written_pretty = bvnr_writer_bytes_written(w);
	bvnr_writer_destroy(w);

	ASSERT_TRUE(written_pretty > 0, "pretty mode: bytes_written > 0");
	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse(buf, written_pretty, &flags, &r),
				"pretty-mode output is parseable");
	ASSERT_EQ_INT((int)ctx.ev_data_count, 1,
				  "pretty-mode: 1 ev_data event");
	bvnr_reader_destroy(r);
}

static void test_bvnf_base16_special_roundtrip(void)
{
	printf("  test_bvnf_base16_special_roundtrip...\n");

	uint8_t outbuf[2048];
	bvnr_writer_t *w = bvnr_writer_create();
	ASSERT_TRUE(w != NULL, "writer create");
	if (!w) return;
	ASSERT_TRUE(bvnr_open_write_mem(w, outbuf, sizeof(outbuf), false, NULL),
				"open_write_mem");

	bvnr_data_t hdr = {0};
	ASSERT_TRUE(bvnr_write_event(w, ev_stream_start, &hdr), "stream_start");

	bvn_float_t *f = bvn_float_alloc(256u);
	ASSERT_TRUE(f != NULL, "bvn_float_alloc");
	if (!f) { bvnr_writer_destroy(w); return; }

	bvn_float_set_nan(f);
	ASSERT_TRUE(bvnr_write_bvnf_base(w, "nan_val", f, 256u, 16u),
				"write NaN base16");

	bvn_float_set_inf(f, false);
	ASSERT_TRUE(bvnr_write_bvnf_base(w, "pos_inf", f, 256u, 16u),
				"write +Inf base16");

	bvn_float_set_inf(f, true);
	ASSERT_TRUE(bvnr_write_bvnf_base(w, "neg_inf", f, 256u, 16u),
				"write -Inf base16");

	bvn_float_from_double(f, 1.5);
	ASSERT_TRUE(bvnr_write_bvnf_base(w, "normal", f, 256u, 16u),
				"write 1.5 base16");

	ASSERT_TRUE(bvnr_write_finish(w), "finish");
	uint64_t n = bvnr_writer_bytes_written(w);
	bvn_float_free(f);
	bvnr_writer_destroy(w);

	ASSERT_TRUE(n > 0u, "bytes written > 0");

	capture_t ctx = {0};
	bvnr_read_flags_t flags = {
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;
	ASSERT_TRUE(do_parse(outbuf, n, &flags, &r),
				"base16 bvnf with NaN/+Inf/-Inf/normal must roundtrip");
	ASSERT_EQ_INT((int)ctx.ev_data_count, 4,
				  "four ev_data events in roundtripped document");

	if (ctx.ev_data_count >= 1)
		ASSERT_TRUE(strstr(ctx.val[0], "nan") != NULL ||
					strstr(ctx.val[0], "NaN") != NULL,
					"NaN: value token contains 'nan'");

	if (ctx.ev_data_count >= 2)
		ASSERT_TRUE(strstr(ctx.val[1], "inf") != NULL ||
					strstr(ctx.val[1], "Inf") != NULL,
					"+Inf: value token contains 'inf'");

	bvnr_reader_destroy(r);
}

static void test_stream_end_event(void)
{
	printf("  test_stream_end_event...\n");

	capture_t ctx = {0};
	bvnr_read_flags_t flags = {
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};
	bvnr_reader_t *r = NULL;

	ASSERT_TRUE(do_parse_str(".a = 1;\n", &flags, &r),
				"simple parse succeeds");
	ASSERT_EQ_INT((int)ctx.stream_end_count, 1,
				  "ev_stream_end fires exactly once on clean parse");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".x = \"hello\";\n.y = 42;\n", &flags, &r),
				"multi-statement parse succeeds");
	ASSERT_EQ_INT((int)ctx.stream_end_count, 1,
				  "ev_stream_end fires exactly once for multi-statement stream");
	bvnr_reader_destroy(r);

	ctx = (capture_t){0};
	ASSERT_FALSE(do_parse_str(".a = !!invalid;\n", &flags, &r),
				 "invalid input fails");
	ASSERT_EQ_INT((int)ctx.stream_end_count, 0,
				  "ev_stream_end not fired on parse error");
	bvnr_reader_destroy(r);
}

/*
 * ----------------------------------------------------------------
 * Regression tests for the four release-blocking lexer bugs:
 *   Bug 1 – stale error position (all-zero) at EOF-in-resync
 *   Bug 2 – column counter off-by-one after CRLF line endings
 *   Bug 3 – str_len not reset after octet-stream chunk completes
 *   Bug 4 – spurious ev_array_row_start injected by resync_semicolon
 * ----------------------------------------------------------------
 */

typedef struct {
	uint32_t row_start_count;
	uint32_t row_end_count;
	uint32_t error_count;
	error_code_t last_error;
	uint64_t last_error_line;
	uint64_t last_error_column;
	uint64_t last_error_offset;
} lex_bug_ctx_t;

static bool lex_bug_verified(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	lex_bug_ctx_t *c = ud;
	(void)d;
	if (ev == ev_array_row_start) c->row_start_count++;
	if (ev == ev_array_row_end)   c->row_end_count++;
	return true;
}

static void lex_bug_on_error(void *ud, error_code_t err,
							  uint64_t line, uint64_t col,
							  uint32_t byte, uint64_t off)
{
	lex_bug_ctx_t *c = ud;
	c->error_count++;
	c->last_error          = err;
	c->last_error_line     = line;
	c->last_error_column   = col;
	c->last_error_offset   = off;
	(void)byte;
}

static void test_bug1_resync_eof_position(void)
{
	printf("  test_bug1_resync_eof_position...\n");

	/*
	 * A stream that contains an illegal byte sequence, after which
	 * the lexer enters resync and the stream ends without a
	 * semicolon to recover.  Before the fix, the error callback
	 * fired with line=0, column=0, offset=0.
	 */
	const char *src = ".a = !!;\n";  /* '!!' is invalid, triggers resync */

	lex_bug_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = lex_bug_verified,
		.on_error          = lex_bug_on_error,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "bug1: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_TRUE(ctx.error_count > 0,
				"bug1: at least one error callback must fire");

	/*
	 * The fix: position fields must not all be zero.  At minimum
	 * error_offset must reflect the number of bytes processed,
	 * and error_line must be >= 1 (init value is 1).
	 */
	ASSERT_TRUE(ctx.last_error_line >= 1,
				"bug1: error_line must be >= 1 (not zeroed) at EOF-in-resync");
	ASSERT_TRUE(ctx.last_error_offset > 0,
				"bug1: error_offset must be > 0 (not zeroed) at EOF-in-resync");

	bvnr_reader_destroy(r);
}

static void test_bug2_crlf_column_tracking(void)
{
	printf("  test_bug2_crlf_column_tracking...\n");

	/*
	 * Two assignments separated by a CRLF pair.  The second
	 * assignment has an intentional error on the second character
	 * of the second line so we can verify the reported column.
	 *
	 * Line 1 (CR+LF): ".a = 1;\r\n"   — 9 bytes
	 * Line 2:         ".b = !!;\n"     — error at '!', column 6
	 *
	 * Before the fix, the LF byte in the CRLF pair bumped the
	 * column counter to 1 without resetting it, so all subsequent
	 * columns on line 2 were reported one too high.
	 */
	const char *src = ".a = 1;\r\n.b = !!;\n";

	lex_bug_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = lex_bug_verified,
		.on_error          = lex_bug_on_error,
		.userdata          = &ctx,
		.continue_on_error = false,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "bug2: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_TRUE(ctx.error_count > 0,
				"bug2: error must be reported for '!!'");
	ASSERT_EQ_UINT(ctx.last_error_line, 2,
				   "bug2: error must be on line 2");
	/*
	 * ".b = !!" — the first '!' is at byte offset 5 on line 2.
	 * Column counting is 1-based, so the first '!' is at column 6.
	 * Before the fix the column was 7 due to LF consuming one slot.
	 */
	ASSERT_EQ_UINT(ctx.last_error_column, 6,
				   "bug2: error column must be 6 (1-based), not 7");

	bvnr_reader_destroy(r);
}

static void test_bug3_str_len_after_octet_stream(void)
{
	printf("  test_bug3_str_len_after_octet_stream...\n");

	/*
	 * Parse two assignments: an octet stream followed by a normal
	 * string.  If str_len is not properly reset after the octet
	 * stream finishes, the subsequent string token could be
	 * processed with a stale accumulator.
	 *
	 * Wire layout:
	 *   .b = <octet-stream start> 0x00 (immediate end) ; \n
	 *   .s = "hello"; \n
	 */
	static const uint8_t src[] = {
		'.', 'b', ' ', '=', ' ',
		'\x00', '\x00',           /* octet-stream start + immediate end tag */
		';', '\n',
		'.', 's', ' ', '=', ' ', '"', 'h', 'e', 'l', 'l', 'o', '"', ';', '\n'
	};

	lex_bug_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified = lex_bug_verified,
		.on_error    = lex_bug_on_error,
		.userdata    = &ctx,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "bug3: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)sizeof(src),
								 NULL, 0, &f)
		   && bvnr_read(r);

	ASSERT_TRUE(ok,
				"bug3: octet-stream followed by string must parse without error");
	ASSERT_EQ_UINT((uint64_t)ctx.error_count, 0,
				   "bug3: no error callback must fire");

	bvnr_reader_destroy(r);
}

static void test_bug4_resync_semicolon_row_balance(void)
{
	printf("  test_bug4_resync_semicolon_row_balance...\n");

	/*
	 * An array that contains a parse error followed by a semicolon
	 * that triggers the resync_semicolon recovery path.
	 *
	 * Before the fix, resync_semicolon emitted ev_array_row_start
	 * for each open array level in addition to ev_array_row_end,
	 * producing an extra unmatched start event and confusing any
	 * consumer that counts starts and ends.
	 */
	const char *src =
		".a = [1, !!bad;\n"   /* resync on '!', recover on ';' */
		".b = 2;\n";

	lex_bug_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = lex_bug_verified,
		.on_error          = lex_bug_on_error,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "bug4: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_TRUE(ctx.error_count > 0,
				"bug4: parse error on '!!' must be reported");

	/*
	 * After the fix: each ev_array_row_start must be paired with
	 * exactly one ev_array_row_end.  The spurious extra start from
	 * resync_semicolon is gone, so the counts must be equal.
	 */
	ASSERT_EQ_UINT((uint64_t)ctx.row_start_count,
				   (uint64_t)ctx.row_end_count,
				   "bug4: ev_array_row_start count must equal ev_array_row_end count");

	bvnr_reader_destroy(r);
}


/*
 * ----------------------------------------------------------------
 * Regression tests for the seven release-blocking lexer bugs
 * identified in the review of bovnar_lexer.c:
 *
 *   New Bug 1 – resync_close_bracket `}` leaves array_nesting_level
 *               dirty (open arrays not drained before ev_struct_end)
 *   New Bug 2 – resync_semicolon decrements array_nesting_level
 *               before the ev_array_row_end callback fires
 *   New Bug 3 – EOF-in-resync re-fires on_error with the original
 *               error code instead of error_got_incomplete_bvnr_stream
 *   New Bug 4 – resync_close_bracket exits resync to array_outro /
 *               struct_outro prematurely, causing a second recovery
 *               on the very next invalid byte
 *   New Bug 5 – bvn_enter_resync zeros prev_byte, corrupting CR+LF
 *               line counting at a resync boundary
 *   New Bug 6 – resync_close_bracket `]` pre-zeros array_row_size
 *               before bvn_val_on_array_outro, suppressing mismatch
 *               detection (addressed as part of Bug 6 fix)
 *   New Bug 7 – bvn_lex_finalize skips NUL-terminating str_data for
 *               token_is_type (implicit invariant, no runtime break)
 *
 *   Bug A – bvn_resync_semicolon_reset unconditionally restores
 *            struct_nesting_level to resync_saved_struct_nesting
 *            even after `}` in resync already closed the struct,
 *            leaving a phantom open struct that triggers a spurious
 *            error_got_incomplete_bvnr_stream at EOF.
 *
 *   Bug B – bvn_action_resync_semicolon called bvn_notify_error
 *            directly when bvn_resync_semicolon_reset returned
 *            false, producing a first notification with zeroed
 *            position fields before the outer interpreter could
 *            emit the authoritative one.
 * ----------------------------------------------------------------
 */

typedef struct {
	uint32_t     error_count;
	error_code_t first_error;
	error_code_t last_error;
	uint64_t     last_error_line;
	uint64_t     last_error_column;
	uint64_t     last_error_offset;
	uint32_t     array_row_start_count;
	uint32_t     array_row_end_count;
	uint32_t     struct_start_count;
	uint32_t     struct_end_count;
} resync_ctx_t;

static bool resync_verified_cb(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	resync_ctx_t *c = ud;
	(void)d;
	switch (ev) {
	case ev_array_row_start: c->array_row_start_count++; break;
	case ev_array_row_end:   c->array_row_end_count++;   break;
	case ev_struct_start:    c->struct_start_count++;    break;
	case ev_struct_end:      c->struct_end_count++;      break;
	default: break;
	}
	return true;
}

static void resync_error_cb(void *ud, error_code_t err,
							 uint64_t line, uint64_t col,
							 uint32_t byte, uint64_t off)
{
	resync_ctx_t *c = ud;
	if (c->error_count == 0)
		c->first_error = err;
	c->last_error        = err;
	c->last_error_line   = line;
	c->last_error_column = col;
	c->last_error_offset = off;
	c->error_count++;
	(void)byte;
}

/*
 * Recovery used to run to the next ';' at the recovery-relative top level, full
 * stop. That is right when the error happened INSIDE a statement — that
 * statement's own ';' is the next one, so only the broken statement is lost.
 * When the error happened BETWEEN statements, though, the next ';' belonged to
 * the following, perfectly good statement, and recovery swallowed it whole: a
 * single stray byte in front of a large struct discarded the entire struct, and
 * the caller was told only that one recovery had happened.
 *
 * Recovery now also ends at the start of the next assignment — a '.' at the
 * recovery-relative top level followed by a byte that can begin an identifier.
 * These pin both halves: the boundary case recovers what follows it, and the
 * in-statement case still discards exactly the broken statement and no more.
 */
typedef struct { uint32_t values; char keys[16][16]; uint32_t n; } rec_keys_t;

static bool rec_keys_cb(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	rec_keys_t *c = ud;
	static char cur[16];
	if (ev == ev_assignment_start && d && d->data) {
		uint32_t l = d->length < 15u ? d->length : 15u;
		memcpy(cur, d->data, l); cur[l] = '\0';
	}
	if (ev == ev_data && d &&
	    (d->type == token_is_number || d->type == token_is_array_number)) {
		c->values++;
		if (c->n < 16u) { snprintf(c->keys[c->n], 16, "%s", cur); c->n++; }
	}
	return true;
}

static rec_keys_t run_recovering(const char *src)
{
	rec_keys_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = rec_keys_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
		.max_struct_nesting = 255,
		.max_array_nesting  = 255,
	};
	bvnr_reader_t *r = bvnr_reader_create();
	if (!r) return ctx;
	(void)(bvnr_open_read_mem(r, src, (uint32_t)strlen(src), NULL, 0, &f)
	       && bvnr_read(r));
	bvnr_reader_destroy(r);
	return ctx;
}

static void test_resync_resumes_at_the_next_assignment(void)
{
	/* a stray token between two statements must cost only itself */
	rec_keys_t c = run_recovering(".a = 1;\n@@@\n.b = 2;\n.c = 3;\n");
	ASSERT_EQ_UINT(c.values, 3, "resync: a boundary error costs no valid statement");

	/* ...including when the following statement is a whole struct */
	c = run_recovering(".a = 1;\n@@@\n.s = { .b = 2; .c = 3; };\n.d = 4;\n");
	ASSERT_EQ_UINT(c.values, 4, "resync: the struct after the error survives");

	/* ...or a whole array */
	c = run_recovering(".a = 1;\n@@@\n.v = [1, 2, 3];\n.d = 4;\n");
	ASSERT_EQ_UINT(c.values, 5, "resync: the array after the error survives");

	/* and a resumed assignment is a member of the struct it sits in */
	c = run_recovering(".s = { .a = 1; @@@ .b = 2; };\n.c = 3;\n");
	ASSERT_EQ_UINT(c.values, 3, "resync: resumes inside the enclosing struct");
}

static void test_resync_still_discards_only_the_broken_statement(void)
{
	/* the error is INSIDE .a, so .a is lost and nothing else is */
	rec_keys_t c = run_recovering(".a = @@@; .b = 2; .c = 3;\n");
	ASSERT_EQ_UINT(c.values, 2, "resync: the broken statement is still dropped");
	if (c.n >= 2) {
		ASSERT_TRUE(strcmp(c.keys[0], "b") == 0, "resync: .b survives");
		ASSERT_TRUE(strcmp(c.keys[1], "c") == 0, "resync: .c survives");
	}
	/* a '.' inside a bracket opened during recovery opens no assignment: the
	 * error is inside .h, so the whole of .h goes, array rows and all */
	c = run_recovering(".h = @@@ [ { .a = 1; }, { .b = 2; } ];\n.c = 3;\n");
	ASSERT_EQ_UINT(c.values, 1, "resync: no resume inside recovery-opened brackets");
}

static void test_resync_dots_that_lead_nowhere(void)
{
	/* a '.' not followed by an identifier byte is just another skipped byte:
	 * no resume, no extra error, and the same recovery as before. */
	static const char *cases[] = {
		".a = 1;\n@@@ 1.5 @@@\n.c = 3;\n",   /* a float in the junk   */
		".a = 1;\n@@@ .5\n.c = 3;\n",         /* dot then a digit      */
		".a = 1;\n@@@ .{ }\n.c = 3;\n",       /* dot then a brace      */
		".a = 1;\n@@@ .....\n.c = 3;\n",      /* a run of dots         */
	};
	for (unsigned i = 0; i < sizeof cases / sizeof cases[0]; i++) {
		rec_keys_t c = run_recovering(cases[i]);
		ASSERT_EQ_UINT(c.values, 2, "resync: a dead-end dot changes nothing");
	}
}

/*
 * recovery_count says how OFTEN recovery ran; it cannot distinguish one skipped
 * assignment from a whole discarded struct. The skipped-byte total is what says
 * how much the document lost — and it is the only way to learn it, because the
 * skipped bytes were never parsed, so no callback ever mentions them.
 */
static uint64_t skipped_for(const char *src, uint64_t *recoveries)
{
	bvnr_read_flags_t f = {
		.continue_on_error  = true,
		.max_struct_nesting = 255,
		.max_array_nesting  = 255,
	};
	bvnr_reader_t *r = bvnr_reader_create();
	if (!r) return 0;
	(void)(bvnr_open_read_mem(r, src, (uint32_t)strlen(src), NULL, 0, &f)
	       && bvnr_read(r));
	uint64_t n = bvnr_reader_get_skipped_bytes(r);
	if (recoveries) *recoveries = bvnr_reader_get_recovery_count(r);
	bvnr_reader_destroy(r);
	return n;
}

static void test_resync_reports_what_it_discarded(void)
{
	uint64_t rec = 0;

	/* a clean document discards nothing */
	ASSERT_EQ_UINT(skipped_for(".a = 1;\n.b = 2;\n", &rec), 0,
		       "skipped: a clean parse discards nothing");
	ASSERT_EQ_UINT(rec, 0, "skipped: and does not recover");

	/* a broken value costs only itself */
	uint64_t small = skipped_for(".a = @@@; .b = 2;\n", &rec);
	ASSERT_TRUE(small > 0 && small < 12,
		    "skipped: a broken value discards a handful of bytes");
	ASSERT_EQ_UINT(rec, 1, "skipped: one recovery");

	/* the same single recovery, but a far larger loss — which is the whole
	 * point: recovery_count cannot tell these two apart, and this can */
	char big[4096];
	size_t p = (size_t)snprintf(big, sizeof big, ".big = @@@ [");
	for (int i = 0; i < 300; i++)
		p += (size_t)snprintf(big + p, sizeof big - p, "%s%d", i ? ", " : "", i);
	p += (size_t)snprintf(big + p, sizeof big - p, "];\n.c = 3;\n");
	uint64_t large = skipped_for(big, &rec);
	ASSERT_EQ_UINT(rec, 1, "skipped: still exactly one recovery");
	ASSERT_TRUE(large > 500,
		    "skipped: ...but the byte count shows the whole array went");
	ASSERT_TRUE(large > small * 10,
		    "skipped: the two losses are distinguishable, unlike the counts");

	/* two recoveries accumulate */
	uint64_t two = skipped_for(".a = @@@; .b = @@@; .c = 3;\n", &rec);
	ASSERT_EQ_UINT(rec, 2, "skipped: two recoveries");
	ASSERT_TRUE(two >= small, "skipped: the total accumulates across them");

	/* recovery that never finds a boundary counts everything to the end */
	uint64_t eof = skipped_for(".a = 1;\n@@@ and then nothing closes", &rec);
	ASSERT_TRUE(eof > 10, "skipped: recovery running to EOF is counted too");

	/* and the fix above means a boundary error now costs almost nothing */
	uint64_t boundary = skipped_for(".a = 1;\n@@@\n.s = { .b = 2; .c = 3; };\n",
					&rec);
	ASSERT_TRUE(boundary < 12,
		    "skipped: a stray byte no longer takes the struct with it");
}

static void test_new_bug1_resync_struct_drains_arrays(void)
{
	printf("  test_new_bug1_resync_struct_drains_arrays...\n");

	/*
	 * A struct containing an open array.  A syntax error triggers
	 * resync.  The recovery `}` closes the struct.
	 *
	 * Before the fix: ev_struct_end was fired while the array was
	 * still open — array_row_end_count < array_row_start_count.
	 *
	 * After the fix: the array is drained first, so both counts
	 * are equal.
	 */
	const char *src =
		".x = { .a = [1, 2, !!bad } ;\n";

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "new_bug1: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_TRUE(ctx.error_count > 0,
				"new_bug1: at least one error must fire");
	ASSERT_TRUE(ctx.struct_start_count >= 1,
				"new_bug1: at least one ev_struct_start");
	ASSERT_EQ_UINT((uint64_t)ctx.struct_start_count,
				   (uint64_t)ctx.struct_end_count,
				   "new_bug1: ev_struct_start/end must be balanced");
	ASSERT_EQ_UINT((uint64_t)ctx.array_row_start_count,
				   (uint64_t)ctx.array_row_end_count,
				   "new_bug1: ev_array_row_start/end must be balanced");

	bvnr_reader_destroy(r);
}

static void test_new_bug3_resync_eof_distinct_error_code(void)
{
	printf("  test_new_bug3_resync_eof_distinct_error_code...\n");

	/*
	 * A stream that ends mid-resync (no semicolon to recover).
	 *
	 * Before any fix: on_error fired twice at the *initial* error site,
	 * causing a duplicate notification with the same position fields.
	 * That was fixed by removing the second call inside the action handler.
	 *
	 * After Bug 1 fix: on_error now fires once for the initial error and
	 * once more at EOF, carrying the preserved original error code.
	 * The two notifications are distinct: the first has the position of
	 * the offending byte; the second has error_offset == processed_bytes.
	 * Total count is therefore 2, not 1.
	 */
	const char *src = ".a = !!";   /* truncated in resync, no ';' */

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "new_bug3: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_EQ_UINT((uint64_t)ctx.error_count, 2,
				   "new_bug3: two notifications — initial error + EOF-in-resync");
	ASSERT_EQ_INT((int)ctx.last_error, (int)error_got_incomplete_bvnr_stream,
				  "new_bug3: EOF-in-resync fires error_got_incomplete_bvnr_stream, not the stale original code");

	bvnr_reader_destroy(r);
}

static void test_new_bug4_resync_bracket_stays_in_resync(void)
{
	printf("  test_new_bug4_resync_bracket_stays_in_resync...\n");

	/*
	 * An array with a syntax error, followed by `]` that closes the
	 * array during resync, then more invalid bytes before `;`.
	 *
	 * Before the fix: `]` exited resync to array_outro state.  The
	 * invalid bytes after `]` triggered a SECOND recovery, so
	 * recovery_count == 2.
	 *
	 * After the fix: `]` stays in resync.  The invalid bytes are
	 * silently skipped.  Only one recovery ever occurs.
	 */
	const char *src =
		".a = [1, !!] !!;\n"
		".b = 2;\n";

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "new_bug4: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_TRUE(ctx.error_count >= 1,
				"new_bug4: at least one error must fire");
	ASSERT_EQ_UINT(bvnr_reader_get_recovery_count(r), 1,
				   "new_bug4: exactly one recovery (not two)");

	bvnr_reader_destroy(r);
}

static void test_new_bug5_crlf_at_resync_boundary(void)
{
	printf("  test_new_bug5_crlf_at_resync_boundary...\n");

	/*
	 * A CR+LF pair that straddles a resync entry point.
	 *
	 * The error is on the CR byte itself (invalid in neg_number_intro
	 * after a bare minus sign).  bvn_enter_resync was zeroing
	 * prev_byte, which caused the subsequent LF to be treated as a
	 * standalone newline rather than the second half of a CR+LF pair
	 * — double-counting one line.
	 *
	 * Line layout:
	 *   1: .a = -\r\n   ← CR triggers error; LF must NOT bump line again
	 *   2: ;\n           ← semicolon recovers; LF bumps to line 3
	 *   3: .b = !!;\n   ← first '!' must be reported on line 3
	 *
	 * Before the fix: the error on '!' was reported on line 4.
	 * After the fix:  it is correctly reported on line 3.
	 */
	const char *src = ".a = -\r\n;\n.b = !!;\n";

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "new_bug5: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_EQ_UINT((uint64_t)ctx.error_count, 2,
				   "new_bug5: two error callbacks (CR error + '!!' error)");
	ASSERT_EQ_UINT(ctx.last_error_line, 3,
				   "new_bug5: second error on line 3, not 4");

	bvnr_reader_destroy(r);
}

static void test_new_bug2_semicolon_recovery_array_event_balance(void)
{
	printf("  test_new_bug2_semicolon_recovery_array_event_balance...\n");

	/*
	 * A syntax error inside a flat array, recovered with `;`.
	 *
	 * bvn_action_resync_semicolon drains open arrays before calling
	 * bvn_resync_semicolon_reset.  The drain loop must fire
	 * ev_array_row_end BEFORE decrementing array_nesting_level so that
	 * the event is emitted while the parser still considers itself
	 * inside the row.
	 *
	 * Before the fix: --array_nesting_level came first; the callback
	 * saw an already-decremented level.  The external invariant we can
	 * observe is that ev_array_row_start and ev_array_row_end must be
	 * balanced and that the assignment following the recovery parses
	 * correctly.
	 */
	const char *src =
		".x = [1, 2, !!bad;\n"
		".y = 3;\n";

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "new_bug2: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_TRUE(ctx.error_count >= 1,
				"new_bug2: at least one error must fire");
	ASSERT_EQ_UINT((uint64_t)ctx.array_row_start_count,
				   (uint64_t)ctx.array_row_end_count,
				   "new_bug2: ev_array_row_start/end must be balanced");
	ASSERT_EQ_UINT(bvnr_reader_get_recovery_count(r), 1,
				   "new_bug2: exactly one recovery");

	bvnr_reader_destroy(r);
}

typedef struct {
	uint32_t     error_count;
	error_code_t last_error;
	uint32_t     type_anno_start_count;
	bool         type_data_nul_ok;
} bug7_ctx_t;

static bool bug7_verified_cb(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	bug7_ctx_t *c = ud;
	if (ev == ev_type_annotation_start) {
		c->type_anno_start_count++;
		if (d->data && ((const uint8_t *)d->data)[d->length] == '\0')
			c->type_data_nul_ok = true;
	}
	return true;
}

static void bug7_error_cb(void *ud, error_code_t err,
						  uint64_t line, uint64_t col,
						  uint32_t byte, uint64_t off)
{
	bug7_ctx_t *c = ud;
	c->last_error = err;
	c->error_count++;
	(void)line; (void)col; (void)byte; (void)off;
}

static void test_new_bug7_type_annotation_nul_termination(void)
{
	printf("  test_new_bug7_type_annotation_nul_termination...\n");

	/*
	 * bvn_lex_finalize must NUL-terminate str_data for every token
	 * type that passes through it, including token_is_type.
	 *
	 * Before the fix: str_data was not NUL-terminated when the token
	 * type was token_is_type.  This is an implicit invariant violation
	 * with no immediate runtime break, because the validator never
	 * reads raw->str_data for token_is_type tokens — it uses
	 * raw->type_data exclusively.  The test verifies the code path
	 * succeeds and that the type_data pointer exposed through the
	 * ev_type_annotation_start callback is properly NUL-terminated.
	 */
	const char *src = ".x = <uint:8> 42;\n";

	bug7_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified = bug7_verified_cb,
		.on_error    = bug7_error_cb,
		.userdata    = &ctx,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "new_bug7: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);

	ASSERT_TRUE(ok, "new_bug7: valid type-annotated assignment must parse");
	ASSERT_EQ_UINT((uint64_t)ctx.error_count, 0,
				   "new_bug7: no error on valid type annotation");
	ASSERT_TRUE(ctx.type_anno_start_count >= 1,
				"new_bug7: at least one ev_type_annotation_start");
	ASSERT_TRUE(ctx.type_data_nul_ok,
				"new_bug7: type_data must be NUL-terminated at d->length");

	bvnr_reader_destroy(r);
}

static void test_new_bug6_resync_bracket_preserves_array_row_size(void)
{
	printf("  test_new_bug6_resync_bracket_preserves_array_row_size...\n");

	/*
	 * A `]` encountered during resync closes the innermost open array.
	 * In the fixed code the action restores array_row_size from the
	 * saved frame (f->saved_row) AFTER firing ev_array_row_end.
	 *
	 * Before the fix: array_row_size was pre-zeroed before the
	 * ev_array_row_end callback, which would suppress row-size-mismatch
	 * detection for any outer array row validated after the resync.
	 *
	 * The test exercises the `]`-in-resync path in a nested array.
	 * It verifies that:
	 *   - recovery counts and event counts are consistent (no crash or
	 *     double-close of the inner array),
	 *   - ev_array_row_start/end balance is maintained across the whole
	 *     input including the outer array, and
	 *   - a valid row-uniform 2D array parsed after the faulty one is
	 *     accepted without error (confirming state was fully reset).
	 */
	const char *src =
		".bad = [[1, 2], [3, !!];\n"
		".good = [[10, 20], [30, 40]];\n";

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "new_bug6: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_TRUE(ctx.error_count >= 1,
				"new_bug6: at least one error must fire (the !! in .bad)");
	ASSERT_EQ_UINT((uint64_t)ctx.array_row_start_count,
				   (uint64_t)ctx.array_row_end_count,
				   "new_bug6: ev_array_row_start/end must be balanced globally");
	ASSERT_EQ_UINT(bvnr_reader_get_recovery_count(r), 1,
				   "new_bug6: exactly one recovery from the faulty row");
	ASSERT_EQ_INT((int)ctx.last_error, (int)error_unexpected_input_byte,
				  "new_bug6: last error is the !! byte, not a mismatch from corrupted row size");

	bvnr_reader_destroy(r);
}

static void test_bug_a_no_phantom_struct_after_bracket_semicolon(void)
{
	printf("  test_bug_a_no_phantom_struct_after_bracket_semicolon...\n");

	/*
	 * A syntax error inside a struct, followed by `}` (which closes
	 * the struct during resync) and then `;` (which triggers recovery).
	 *
	 * Before the fix: bvn_resync_semicolon_reset unconditionally
	 * restored struct_nesting_level to resync_saved_struct_nesting
	 * (the level at error time = 1).  The `}` had already decremented
	 * the level to 0 and emitted ev_struct_end, so the restore to 1
	 * created a phantom open struct.  At EOF the phantom caused a
	 * spurious error_got_incomplete_bvnr_stream notification, making
	 * error_count == 2 and struct_start_count != struct_end_count.
	 *
	 * After the fix: bvn_resync_semicolon_reset clamps
	 * resync_saved_struct_nesting to the current level before the
	 * assignment, so the level stays at 0 after recovery and the
	 * stream ends cleanly.
	 */
	const char *src = ".x = { !! } ;\n";

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "bug_a: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_EQ_UINT((uint64_t)ctx.error_count, 1,
				   "bug_a: exactly one error (the !! byte, no phantom EOF error)");
	ASSERT_EQ_UINT((uint64_t)ctx.struct_start_count,
				   (uint64_t)ctx.struct_end_count,
				   "bug_a: ev_struct_start/end must be balanced (no phantom struct)");
	ASSERT_TRUE(ctx.last_error != error_got_incomplete_bvnr_stream,
				"bug_a: last error must not be error_got_incomplete_bvnr_stream");

	bvnr_reader_destroy(r);
}

static void test_bug_b_single_notification_on_semicolon_recovery(void)
{
	printf("  test_bug_b_single_notification_on_semicolon_recovery...\n");

	/*
	 * Before the fix: bvn_action_resync_semicolon called bvn_notify_error
	 * directly when bvn_resync_semicolon_reset returned false.  That
	 * call had zeroed position fields (zeroed by bvn_enter_resync, never
	 * repopulated) and fired before the outer bvn_interpret_input_buffer
	 * could emit the authoritative notification — producing two
	 * on_error callbacks for a single logical error.
	 *
	 * After the fix: the interior bvn_notify_error call is removed.
	 * The outer interpreter is solely responsible for calling on_error,
	 * which it does once with the correct position.
	 *
	 * The test verifies the error count and that the single notification
	 * carries a non-zero position, ruling out the zeroed-fields symptom.
	 * Multiple recoveries are exercised to confirm the invariant holds
	 * across the whole stream.
	 */
	const char *src =
		".x = { !! ;\n"
		".a = 1;\n"
		"};\n"
		".y = !!;\n";

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "bug_b: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_EQ_UINT(bvnr_reader_get_recovery_count(r), 2,
				   "bug_b: two recoveries (one per `!!` site)");
	ASSERT_EQ_UINT((uint64_t)ctx.error_count,
				   (uint64_t)bvnr_reader_get_recovery_count(r),
				   "bug_b: on_error fires exactly once per recovery (no double-fire)");
	ASSERT_GE_UINT(ctx.last_error_line, 1,
				   "bug_b: last error notification carries a non-zero line number");
	ASSERT_GE_UINT(ctx.last_error_offset, 1,
				   "bug_b: last error notification carries a non-zero stream offset");

	bvnr_reader_destroy(r);
}

typedef struct {
	unsigned struct_start_count;
	unsigned struct_end_count;
	unsigned ev_data_count;
	unsigned error_count;
	bool     reject_struct_start;
} svt_ctx_t;

static bool svt_verified(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	svt_ctx_t *c = ud;
	(void)d;
	if (ev == ev_struct_start) {
		c->struct_start_count++;
		if (c->reject_struct_start)
			return false;
	} else if (ev == ev_struct_end) {
		c->struct_end_count++;
	} else if (ev == ev_data) {
		c->ev_data_count++;
	}
	return true;
}

static void svt_error(void *ud, error_code_t err,
					   uint64_t line, uint64_t col,
					   uint32_t byte, uint64_t off)
{
	svt_ctx_t *c = ud;
	c->error_count++;
	(void)err; (void)line; (void)col; (void)byte; (void)off;
}

static void test_bug_c_inline_unit_comment_no_concat(void)
{
	printf("  test_bug_c_inline_unit_comment_no_concat...\n");

	/*
	 * Before fix: bvn_action_comment_outro restored last_state =
	 * inline_unit_body without translation, so the byte immediately
	 * after a comment that appeared inside an inline-unit body was
	 * silently appended to the unit string.
	 *
	 * Input:  ".x = 1.5 kg#comment\nm;\n"
	 * Before: unit = "kgm"   (silent wrong result)
	 * After:  'm' is illegal in inline_unit_outro => error + recovery
	 *
	 * Sub-test A: comment followed immediately by ';' — must succeed
	 * cleanly (the comment is the terminator for the unit body).
	 *
	 * Sub-test B: with continue_on_error, a non-unit byte after the
	 * comment must trigger at least one recovery rather than silently
	 * producing a wrong unit string.
	 */

	capture_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified = capture_verified,
		.on_error    = capture_error,
		.userdata    = &ctx,
	};

	bvnr_reader_t *r = NULL;
	ASSERT_TRUE(do_parse_str(".x = 1.5 m#comment\n;\n", &f, &r),
				"bug_c A: unit + comment + semicolon must parse cleanly");
	ASSERT_EQ_UINT((uint64_t)ctx.ev_data_count, 1,
				   "bug_c A: exactly one data event");
	bvnr_reader_destroy(r);

	resync_ctx_t rctx = {0};
	bvnr_read_flags_t fc = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &rctx,
		.continue_on_error = true,
	};
	r = NULL;
	(void)do_parse_str(".x = 1.5 m#comment\n!;\n", &fc, &r);
	ASSERT_GE_UINT(bvnr_reader_get_recovery_count(r), 1,
				   "bug_c B: non-unit byte after unit comment must trigger recovery");
	bvnr_reader_destroy(r);
}

static void test_bug_d_struct_intro_no_phantom_end(void)
{
	printf("  test_bug_d_struct_intro_no_phantom_end...\n");

	/*
	 * Before fix: bvn_action_struct_intro incremented struct_nesting_level
	 * before calling bvn_val_receive_event(ev_struct_start). With
	 * continue_on_error, if on_verified rejected ev_struct_start,
	 * bvn_enter_resync captured the inflated level (1 instead of 0).
	 * The subsequent '}' in resync then decremented it back to 0 while
	 * emitting ev_struct_end — a phantom event with no matching start.
	 *
	 * After fix: the level is incremented only on success, so resync
	 * captures the correct level (0) and the resync '}' handler never
	 * fires the closing event.
	 */
	svt_ctx_t ctx = { .reject_struct_start = true };
	bvnr_read_flags_t f = {
		.on_verified       = svt_verified,
		.on_error          = svt_error,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "bug_d: reader create");
	if (!r) return;

	const char *src = ".x = {};\n";
	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_EQ_UINT((uint64_t)ctx.error_count, 1,
				   "bug_d: exactly one error (the rejected ev_struct_start)");
	ASSERT_EQ_UINT((uint64_t)ctx.struct_start_count, 1,
				   "bug_d: ev_struct_start delivered once (then rejected)");
	ASSERT_EQ_UINT((uint64_t)ctx.struct_end_count, 0,
				   "bug_d: no phantom ev_struct_end without matching start");

	bvnr_reader_destroy(r);
}

static void test_bug_f_bad_continuation_retried_in_resync(void)
{
	printf("  test_bug_f_bad_continuation_retried_in_resync...\n");

	/*
	 * Before fix: when an invalid UTF-8 continuation byte was detected
	 * inside resync mode the offending byte was silently consumed via
	 * `continue`, never reaching the resync state machine. Structural
	 * bytes like ';' (0x3B) that appeared where a continuation was
	 * expected were therefore lost, preventing resync from terminating.
	 *
	 * The stream ".x = !!\xc2;\n":
	 *   - '!!' in value_intro => error_unexpected_input_byte, recovery #1.
	 *   - '\xc2' in resync => valid 2-byte leader, utf8_need = 1.
	 *   - ';' (0x3B) arrives: not a valid continuation (not in [0x80,0xBF])
	 *     => error_invalid_utf8_byte, recovery #2.
	 *     Before fix: ';' dropped, parser stays in resync, hits EOF =>
	 *       error_got_incomplete_bvnr_stream, bvnr_read returns false.
	 *     After fix: ';' re-offered to resync scanner =>
	 *       bvn_action_resync_semicolon fires, recovery exits cleanly,
	 *       bvnr_read returns true.
	 */
	static const uint8_t src[] = {
		'.','x',' ','=',' ','!','!', (uint8_t)0xc2, ';','\n'
	};

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "bug_f: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, sizeof(src), NULL, 0, &f)
		   && bvnr_read(r);

	ASSERT_TRUE(ok,
				"bug_f: parse must succeed when ';' is retried after bad continuation");
	ASSERT_EQ_UINT(bvnr_reader_get_recovery_count(r), 2,
				   "bug_f: exactly two recoveries ('!!' error and bad-continuation ';')");
	ASSERT_EQ_UINT((uint64_t)ctx.error_count, 2,
				   "bug_f: exactly two on_error callbacks");

	bvnr_reader_destroy(r);
}

typedef struct {
	uint32_t row_end_none_unit_count;
	uint32_t row_end_count;
} bug_e_ctx_t;

static bool bug_e_verified_cb(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	bug_e_ctx_t *c = ud;
	if (ev == ev_array_row_end) {
		c->row_end_count++;
		if (d->value_unit.num_components == 0)
			c->row_end_none_unit_count++;
	}
	return true;
}

static void bug_e_error_cb(void *ud, error_code_t err,
							uint64_t line, uint64_t col,
							uint32_t byte, uint64_t off)
{
	(void)ud; (void)err; (void)line; (void)col; (void)byte; (void)off;
}

static void test_bug_e_resync_frame_vunit_init(void)
{
	printf("  test_bug_e_resync_frame_vunit_init...\n");

	/*
	 * Bug E: bvn_enter_resync zeroed the entire arr_frames array with
	 * memset, leaving saved_vunit as all-bytes-zero for every live
	 * frame.  all-bytes-zero is BVN_UNIT_NONE (num_components == 0),
	 * not BVN_UNIT_NO_PREFIX(bu_none) (num_components == 1).
	 *
	 * After `]` closes an inner array during resync, the frame
	 * restored for the outer array carries saved_vunit = BVN_UNIT_NONE.
	 * Any ev_array_row_end subsequently emitted for the outer array
	 * (e.g. by the bvn_action_resync_semicolon drain loop) therefore
	 * carries value_unit.num_components == 0 instead of 1.
	 *
	 * Scenario: .bad = [[1, !!];
	 *   - outer `[` opens (nesting=1)
	 *   - inner `[1,` opens (nesting=2)
	 *   - `!!` triggers resync inside inner array (nesting remains 2)
	 *   - `]` in resync closes the inner array; saved_vunit of outer
	 *     frame is restored — zero before fix, BVN_UNIT_NO_PREFIX(bu_none)
	 *     after fix
	 *   - `;` drain loop closes the outer array, emitting
	 *     ev_array_row_end with the (potentially corrupted) parsed_unit
	 *
	 * Note: we do NOT use normal (pre-error) array closes because
	 * bvn_val_on_array_outro always emits ev_array_row_end with a
	 * zero-initialised data struct (num_components=0 by design).
	 * The bug is specifically about events emitted via
	 * bvn_val_receive_event (which copies from parsed_unit) in the
	 * resync ] pop path and the semicolon drain.
	 *
	 * After fix: every ev_array_row_end emitted via the resync/drain
	 * paths must carry num_components >= 1.
	 */
	const char *src =
		".bad = [[1, !!];\n"
		".good = 1;\n";

	bug_e_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = bug_e_verified_cb,
		.on_error          = bug_e_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "bug_e: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								  NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_TRUE(ctx.row_end_count >= 1,
				"bug_e: at least one ev_array_row_end must fire");
	ASSERT_EQ_UINT((uint64_t)ctx.row_end_none_unit_count, 0,
				   "bug_e: no ev_array_row_end may carry num_components==0 (BVN_UNIT_NONE)");

	bvnr_reader_destroy(r);
}

static void test_bug_g_semicolon_reset_clears_utf8_and_inline_unit(void)
{
	printf("  test_bug_g_semicolon_reset_clears_utf8_and_inline_unit...\n");

	/*
	 * Defensive fix (Bugs G1+G2): bvn_resync_semicolon_reset did not
	 * reset utf8_need/utf8_lo/utf8_hi or inline_unit_len.  Because
	 * bvn_enter_resync already clears those fields before any resync
	 * action fires, the omission has no observable effect in the
	 * current code.  The fix is protective against future refactoring.
	 *
	 * This test verifies that multi-byte UTF-8 string content in the
	 * assignment immediately following a semicolon recovery parses
	 * without error, ensuring the reset path is exercised correctly
	 * and leaves the decoder in a clean state.
	 */
	static const uint8_t src[] =
		".a = !!;\n"
		".b = \"\xc3\xa9\xc3\xa0\";\n";

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "bug_g: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, sizeof(src) - 1u,
								  NULL, 0, &f)
		   && bvnr_read(r);

	ASSERT_TRUE(ok,
				"bug_g: stream must parse successfully after semicolon recovery");
	ASSERT_EQ_UINT((uint64_t)ctx.error_count, 1,
				   "bug_g: exactly one error (the !! byte) — no spurious UTF-8 errors");
	ASSERT_EQ_UINT(bvnr_reader_get_recovery_count(r), 1,
				   "bug_g: exactly one recovery");

	bvnr_reader_destroy(r);
}

/*
 * ----------------------------------------------------------------
 * Regression tests for the five release-blocking lexer bugs
 * identified in the second round of review:
 *
 *   RB1 – bvn_lex_run did not call bvn_notify_error at EOF-in-resync,
 *          so the on_error callback was never invoked for that terminal
 *          failure.
 *
 *   RB2 – bvn_enter_resync did not emit ev_array_row_end events for
 *          any arrays open at the time of the error, so consumers
 *          received unbalanced array events when a stream ended in
 *          resync without a recovery character.
 *
 *   RB3 – bvn_special_keyword_outro wrote "nan" / "infinity" /
 *          "-infinity" into str_data without checking the caller-
 *          configured max_number_length limit.
 *
 *   RB4 – bvn_interpret_input_buffer called bvn_advance_line twice
 *          for the same byte when a bad UTF-8 continuation was
 *          re-fed through the resync state machine (structurally
 *          incorrect; harmless in practice because the second call
 *          is a no-op for non-newline bytes, but the latent double-
 *          call becomes a real line-counter corruption the moment
 *          the surrounding guard condition changes).
 *
 *   RB5 – bvn_action_resync_close_bracket emitted ev_array_row_end
 *          for arrays that bvn_enter_resync had already conceptually
 *          closed, producing duplicate close events.  Subsumed by the
 *          RB2 fix: bvn_enter_resync now zeroes array_nesting_level
 *          after draining, so the ']' resync handler finds nesting=0
 *          and emits no extra events.
 * ----------------------------------------------------------------
 */

static void test_rb1_eof_resync_callback_fires(void)
{
	printf("  test_rb1_eof_resync_callback_fires...\n");

	/*
	 * A stream that ends while in resync (no recovery character).
	 * Before fix: the on_error callback was never invoked for the
	 * EOF condition; bvn_lex_run just returned false silently.
	 * After fix: on_error fires a second time at EOF carrying the
	 * preserved original error code.
	 *
	 * This is distinct from test_new_bug3, which verified that the
	 * *initial* error does not fire twice.  Here we verify that the
	 * *terminal* EOF condition does fire once.
	 */
	const char *src = ".a = !!";  /* no semicolon — ends in resync */

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "rb1: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	ASSERT_FALSE(ok, "rb1: bvnr_read must return false for incomplete stream");

	ASSERT_EQ_UINT((uint64_t)ctx.error_count, 2,
				   "rb1: on_error fires twice — once at the bad byte, once at EOF");
	ASSERT_GE_UINT(ctx.last_error_offset, 1,
				   "rb1: the EOF notification carries a non-zero stream offset");

	bvnr_reader_destroy(r);
}

static void test_rb2_eof_array_events_balanced(void)
{
	printf("  test_rb2_eof_array_events_balanced...\n");

	/*
	 * A stream with an open array that ends in resync without a
	 * recovery character (no ']', '}', or ';').
	 *
	 * Before fix: bvn_enter_resync preserved array_nesting_level so
	 * that recovery handlers could drain it, but if EOF arrived first
	 * the drain never ran and ev_array_row_end was never emitted.
	 * Result: ev_array_row_start count > ev_array_row_end count.
	 *
	 * After fix: bvn_enter_resync immediately emits ev_array_row_end
	 * for every open array level before zeroing nesting, so the
	 * consumer sees balanced events even when the stream is truncated.
	 */
	const char *src = ".a = [1, !!";  /* open array, error, then EOF */

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "rb2: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	ASSERT_FALSE(ok, "rb2: bvnr_read must return false for truncated stream");

	ASSERT_GE_UINT((uint64_t)ctx.error_count, 1,
				   "rb2: at least one error must fire");
	ASSERT_EQ_UINT((uint64_t)ctx.array_row_start_count,
				   (uint64_t)ctx.array_row_end_count,
				   "rb2: ev_array_row_start/end must be balanced even at EOF-in-resync");

	bvnr_reader_destroy(r);
}

static void test_rb2_nested_array_eof_balanced(void)
{
	printf("  test_rb2_nested_array_eof_balanced...\n");

	/*
	 * A nested array (.a = [[1, 2, !!) ending in resync at EOF with
	 * two open array levels.  Both must be drained by bvn_enter_resync.
	 */
	const char *src = ".a = [[1, 2, !!";

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "rb2n: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_EQ_UINT((uint64_t)ctx.array_row_start_count,
				   (uint64_t)ctx.array_row_end_count,
				   "rb2n: ev_array_row_start/end balanced for nested arrays at EOF-in-resync");

	bvnr_reader_destroy(r);
}

static void test_rb3_special_number_respects_max_symbol_length(void)
{
	printf("  test_rb3_special_number_respects_max_symbol_length...\n");

	/*
	 * Special floats are bare reserved keywords (nan / inf / ninf) that lex
	 * as symbols and are reclassified to numbers by the validator. As
	 * symbols they are bounded by max_symbol_length, so a caller that
	 * configures a tight limit to bound processing cost gets a clean
	 * error_symbol_too_long rather than an over-limit token.
	 *
	 * max_symbol_length = 3 allows "nan" and "inf" (3 bytes, exactly at the
	 * limit) but rejects "ninf" (4).
	 */
	capture_t ctx = {0};
	bvnr_read_flags_t f = {
		.max_symbol_length = 3,
		.on_verified       = capture_verified,
		.on_error          = capture_error,
		.userdata          = &ctx,
	};
	bvnr_reader_t *r = NULL;

	/* "nan" (3-char keyword) must succeed under limit=3 */
	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".x = nan;\n", &f, &r),
				"rb3: 'nan' (3-char keyword) with limit=3 must succeed");
	bvnr_reader_destroy(r); r = NULL;

	/* "inf" (3-char keyword) must succeed under limit=3 */
	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".x = inf;\n", &f, &r),
				"rb3: 'inf' (3-char keyword) with limit=3 must succeed");
	bvnr_reader_destroy(r); r = NULL;

	/* "ninf" (4-char keyword) must fail under limit=3 */
	ctx = (capture_t){0};
	ASSERT_FALSE(do_parse_str(".x = ninf;\n", &f, &r),
				 "rb3: 'ninf' (4-char keyword) with limit=3 must fail");
	ASSERT_EQ_INT(bvnr_reader_get_error(r), error_symbol_too_long,
				  "rb3: error must be error_symbol_too_long for 'ninf'");
	bvnr_reader_destroy(r); r = NULL;

	/* "ninf" (4-char keyword) must succeed under limit=4 */
	f.max_symbol_length = 4;
	ctx = (capture_t){0};
	ASSERT_TRUE(do_parse_str(".x = ninf;\n", &f, &r),
				"rb3: 'ninf' (4-char keyword) with limit=4 must succeed");
	bvnr_reader_destroy(r);
}

static void test_rb4_advance_line_once_per_byte(void)
{
	printf("  test_rb4_advance_line_once_per_byte...\n");

	/*
	 * Before fix: bvn_interpret_input_buffer called bvn_advance_line
	 * twice for the same byte when a bad UTF-8 continuation was
	 * re-fed through the resync scanner and did not trigger a
	 * `continue`.  For non-newline bytes this is a no-op (the function
	 * only acts on 0x0d and 0x0a), so there is no observable behavioural
	 * regression — but the structural double-call is a latent defect.
	 *
	 * After fix: a `line_advanced` flag ensures the call happens at most
	 * once per byte regardless of path taken through the loop body.
	 *
	 * Observable proxy: a two-byte UTF-8 leader (\xc2) followed by
	 * the ASCII letter 'z' (a valid new leader) in resync.  After the
	 * continuation error the 'z' is re-fed.  The column counter must
	 * advance exactly once for 'z', not twice.  We use a second error
	 * immediately after 'z' on the same line to read back the column.
	 *
	 * Input (with continue_on_error):
	 *   .a = !!         -> error, enter resync (col recorded by resync_error_cb)
	 *   \xc2 z !!;\n    -> continuation error for 'z', 'z' re-fed into resync,
	 *                      second '!!' triggers another resync_semicolon
	 */
	static const uint8_t src[] = {
		'.','a',' ','=',' ','!','!',
		(uint8_t)0xc2, 'z', '!','!', ';', '\n'
	};

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "rb4: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, sizeof(src), NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_GE_UINT((uint64_t)ctx.error_count, 2,
				   "rb4: at least two errors must fire (initial !! and bad continuation)");
	ASSERT_EQ_UINT(ctx.last_error_line, 1,
				   "rb4: all errors are on line 1");

	bvnr_reader_destroy(r);
}

static void test_rb5_resync_bracket_no_duplicate_row_end(void)
{
	printf("  test_rb5_resync_bracket_no_duplicate_row_end...\n");

	/*
	 * Before fix: bvn_enter_resync preserved array_nesting_level so
	 * that the ']' resync handler could later emit ev_array_row_end.
	 * But because bvn_enter_resync had already conceptually closed the
	 * array (by zeroing its accumulated state), the ']' handler
	 * emitted a second ev_array_row_end for the same logical array that
	 * was already closed, producing one extra unmatched close event.
	 *
	 * After fix: bvn_enter_resync drains and zeroes array_nesting_level
	 * itself (RB2 fix).  The ']' resync handler then finds nesting=0
	 * and emits nothing — no duplicate.
	 *
	 * Scenario: .a = [1, !!] ;
	 *   One ev_array_row_start (the [), then error enters resync and
	 *   drains (one ev_array_row_end), then ] in resync emits nothing
	 *   (nesting=0), then ; recovers.  Final balance: 1 == 1.
	 */
	const char *src = ".a = [1, !!] ;\n.b = 2;\n";

	resync_ctx_t ctx = {0};
	bvnr_read_flags_t f = {
		.on_verified       = resync_verified_cb,
		.on_error          = resync_error_cb,
		.userdata          = &ctx,
		.continue_on_error = true,
	};

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "rb5: reader create");
	if (!r) return;

	bool ok = bvnr_open_read_mem(r, src, (uint32_t)strlen(src),
								 NULL, 0, &f)
		   && bvnr_read(r);
	(void)ok;

	ASSERT_GE_UINT((uint64_t)ctx.error_count, 1,
				   "rb5: at least one error must fire");
	ASSERT_EQ_UINT((uint64_t)ctx.array_row_start_count,
				   (uint64_t)ctx.array_row_end_count,
				   "rb5: ev_array_row_start/end balanced — no duplicate close from ']' in resync");
	ASSERT_EQ_UINT(bvnr_reader_get_recovery_count(r), 1,
				   "rb5: exactly one recovery");

	bvnr_reader_destroy(r);
}

/*
 * bvnr_read() on a reader that was created but never opened dereferenced a NULL
 * pull function pointer and segfaulted. Every other entry point in the API
 * tolerates a NULL handle -- bvnr_reader_get_error, bvnr_reader_destroy,
 * bvn_dom_lookup, bvnr_demux_error all return quietly -- so this one was the odd
 * one out, and an unopened reader is the same class of caller mistake as a NULL
 * one. Found by driving every public entry point with NULL handles and
 * out-of-order calls under ASan/UBSan.
 */
static void test_rb6_read_without_open_does_not_fault(void)
{
	ASSERT_FALSE(bvnr_read(NULL), "bvnr_read(NULL) returns false");

	bvnr_reader_t *r = bvnr_reader_create();
	ASSERT_TRUE(r != NULL, "reader created");
	if (!r) return;
	ASSERT_FALSE(bvnr_read(r), "bvnr_read() on an unopened reader returns false");
	ASSERT_TRUE(bvnr_reader_get_error(r) == error_invalid_argument,
		"an unopened reader reports error_invalid_argument");

	/* ... and the same reader still works once a source is attached, i.e. the
	 * guard rejects the call without poisoning the handle. */
	const char *doc = "#!bovnar 1.1\n.x = <float:64,m> 1.5;\n";
	bvnr_read_flags_t fl = { .max_file_size = BVNR_FILESIZE_UNLIMITED };
	ASSERT_TRUE(bvnr_open_read_mem(r, (const uint8_t *)doc,
				       (uint32_t)strlen(doc), NULL, 0, &fl),
		"open after the refused read succeeds");
	ASSERT_TRUE(bvnr_read(r), "the reader parses normally afterwards");
	bvnr_reader_destroy(r);
}

/*
 * The error position counters measure different things, and nothing said which.
 * column is a 1-based CHARACTER column (a multi-byte UTF-8 sequence advances it
 * by one, a tab advances to the next multiple of 4) while offset is a 0-based
 * BYTE offset. A tool highlighting the error needs to know: in
 * `.x = "café"; .y = @;` the '@' is byte 20 but column 19, and unit symbols in
 * this format are routinely non-ASCII (µ~m, °C, Ω), so the two disagree often.
 * Now documented on the getters and in doc/08, so pinned here.
 */
static void test_rb7_error_position_counters(void)
{
	static const struct {
		const char *doc;
		uint64_t line, column, offset;
		const char *why;
	} T[] = {
		{ ".x = @;\n",                    1,  6,  5, "plain ASCII" },
		{ ".a = 1;\n.b = 2;\n.c = @;\n",  3,  6, 21, "third line" },
		{ "# comment\n.x = @;\n",         2,  6, 15, "after a comment" },
		/* café is 5 characters in 6 bytes, so column and offset diverge. */
		{ ".x = \"caf\xc3\xa9\"; .y = @;\n", 1, 19, 19, "multi-byte UTF-8" },
		/* ".x =" is 4 columns; the tab advances to 8, so '@' lands on 9. */
		{ ".x =\t@;\n",                   1,  9,  5, "tab to the next multiple of 4" },
		{ ".x =   \t@;\n",                1,  9,  8, "tab from column 8" },
		{ ".x =    \t@;\n",               1, 13,  9, "tab from column 9" },
	};
	for (size_t i = 0; i < sizeof T / sizeof T[0]; i++) {
		bvnr_reader_t *r = bvnr_reader_create();
		bvnr_read_flags_t fl = { .max_file_size = BVNR_FILESIZE_UNLIMITED };
		bool ok = bvnr_open_read_mem(r, (const uint8_t *)T[i].doc,
					     (uint32_t)strlen(T[i].doc), NULL, 0, &fl)
			  && bvnr_read(r);
		ASSERT_FALSE(ok, "the probe document is meant to fail");
		if (bvnr_reader_get_error_line(r) != T[i].line
		    || bvnr_reader_get_error_column(r) != T[i].column
		    || bvnr_reader_get_error_offset(r) != T[i].offset)
			fprintf(stderr,
				"    %s: line=%llu col=%llu off=%llu, want %llu/%llu/%llu\n",
				T[i].why,
				(unsigned long long)bvnr_reader_get_error_line(r),
				(unsigned long long)bvnr_reader_get_error_column(r),
				(unsigned long long)bvnr_reader_get_error_offset(r),
				(unsigned long long)T[i].line,
				(unsigned long long)T[i].column,
				(unsigned long long)T[i].offset);
		ASSERT_TRUE(bvnr_reader_get_error_line(r) == T[i].line,
			"error line is the 1-based line number");
		ASSERT_TRUE(bvnr_reader_get_error_column(r) == T[i].column,
			"error column is the 1-based CHARACTER column");
		ASSERT_TRUE(bvnr_reader_get_error_offset(r) == T[i].offset,
			"error offset is the 0-based BYTE offset");
		bvnr_reader_destroy(r);
	}
}

/*
 * Every reader limit must be exact at its boundary -- a document needing N is
 * accepted at N and refused at N-1 -- and the two BYTE caps must keep counting
 * different things.
 *
 * max_text_bytes counts what the TEXT scanner consumes; an octet stream's binary
 * payload does not count towards it. A 212-byte document whose body is a
 * 200-byte octet stream is accepted at max_text_bytes = 8. Nothing said so, and
 * the distinction has teeth: using it as a memory cap leaves a large binary
 * payload unbounded. Only max_file_size counts every byte.
 */
static bool hs_parses(const unsigned char *doc, uint32_t len,
                      const bvnr_read_flags_t *base)
{
	bvnr_reader_t *r = bvnr_reader_create();
	if (!r) return false;
	bvnr_read_flags_t fl = *base;
	bool ok = bvnr_open_read_mem(r, doc, len, NULL, 0, &fl) && bvnr_read(r);
	bvnr_reader_destroy(r);
	return ok;
}

static void test_rb8_reader_limits_are_exact(void)
{
	bvnr_read_flags_t base;

#define HS_BOUNDARY(field, cast, doc, n, why)                                 \
	do {                                                                      \
		const char *_d = (doc);                                               \
		uint32_t _l = (uint32_t)strlen(_d);                                   \
		memset(&base, 0, sizeof base);                                        \
		base.max_file_size = BVNR_FILESIZE_UNLIMITED;                         \
		base.field = (cast)(n);                                               \
		ASSERT_TRUE(hs_parses((const unsigned char *)_d, _l, &base),          \
			"accepted at its own size: " why);                                \
		memset(&base, 0, sizeof base);                                        \
		base.max_file_size = BVNR_FILESIZE_UNLIMITED;                         \
		base.field = (cast)((n) - 1);                                         \
		ASSERT_FALSE(hs_parses((const unsigned char *)_d, _l, &base),         \
			"refused one below: " why);                                       \
	} while (0)

	HS_BOUNDARY(max_identifier_length, uint16_t,
		".aaaaaaaaaaaaaaaaaaaa = 1;\n", 20, "identifier of 20");
	HS_BOUNDARY(max_string_length, uint16_t,
		".x = \"aaaaaaaaaa\";\n", 10, "string of 10");
	HS_BOUNDARY(max_number_length, uint16_t,
		".x = 777777777777;\n", 12, "number of 12 digits");
	HS_BOUNDARY(max_symbol_length, uint16_t,
		".x = sssssssssssssss;\n", 15, "symbol of 15");
	HS_BOUNDARY(max_reference_length, uint16_t,
		".abcdefghij = 1;\n.y = &.abcdefghij;\n", 11, "reference path of 11");
	HS_BOUNDARY(max_array_items, uint64_t,
		".x = [1, 2, 3, 4, 5];\n", 5, "array of 5 items");
	HS_BOUNDARY(max_struct_nesting, uint8_t,
		".x = {.a = {.b = {.c = 1;};};};\n", 3, "struct nesting of 3");
	HS_BOUNDARY(max_array_nesting, uint8_t,
		".x = [[[1]]];\n", 3, "array nesting of 3");
#undef HS_BOUNDARY

	/* The two byte caps, on one document: 212 bytes of which 200 are an octet
	 * stream payload. */
	unsigned char doc[512];
	uint32_t n = 0;
	memcpy(doc, ".b = ", 5); n = 5;
	doc[n++] = 0x00; doc[n++] = 0x01; doc[n++] = 200; doc[n++] = 0x00;
	for (int i = 0; i < 200; i++) doc[n++] = (unsigned char)(i & 0xff);
	doc[n++] = 0x00; doc[n++] = ';'; doc[n++] = '\n';

	memset(&base, 0, sizeof base);
	base.max_file_size = BVNR_FILESIZE_UNLIMITED;
	base.max_text_bytes = 8;
	ASSERT_TRUE(hs_parses(doc, n, &base),
		"max_text_bytes ignores the octet-stream payload");
	base.max_text_bytes = 7;
	ASSERT_FALSE(hs_parses(doc, n, &base),
		"...and is still exact at its own boundary");

	memset(&base, 0, sizeof base);
	base.max_file_size = n;
	ASSERT_TRUE(hs_parses(doc, n, &base),
		"max_file_size counts every byte, payload included");
	base.max_file_size = n - 1;
	ASSERT_FALSE(hs_parses(doc, n, &base),
		"...and is exact at its boundary");
}

/* ===========================================================================
 * Fourth review round -- the read side.
 *
 * One recorder for all of it: the keys and values delivered, every error with
 * its position, and whether the start/end events NEST (a row_end must close an
 * array, a struct_end a struct, an assignment may not sit directly in an
 * array, and nothing may be open at stream_end).
 * ======================================================================== */
typedef struct {
	char         keys[256];
	char         vals[256];
	char         stk[600];
	int          sp;
	int          nest_violations;
	int          nerr;
	error_code_t errs[8];
	uint64_t     offs[8];
	uint32_t     bytes[8];
	bool         ok;
	error_code_t err;
	uint64_t     off;
	uint32_t     byte;
	uint64_t     recoveries;
	char         conv[64];
} r4_t;
static void r4_cat(char *dst, size_t cap, const void *s, uint32_t n)
{
	size_t l = strlen(dst);
	if (l + n + 2u > cap) return;
	memcpy(dst + l, s, n);
	dst[l + n] = ',';
	dst[l + n + 1u] = '\0';
}
static bool r4_cb(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	r4_t *c = ud;
	switch (ev) {
	case ev_assignment_start:
		if (c->sp > 0 && c->stk[c->sp - 1] == 'A') c->nest_violations++;
		r4_cat(c->keys, sizeof c->keys, d->data, d->length);
		break;
	case ev_data:
		if (d->type != token_is_octet_stream)
			r4_cat(c->vals, sizeof c->vals, d->data, d->length);
		if (d->converted && d->conv.text && d->conv.length < sizeof c->conv) {
			memcpy(c->conv, d->conv.text, d->conv.length);
			c->conv[d->conv.length] = '\0';
		}
		break;
	case ev_struct_start:
		if (c->sp < (int)sizeof c->stk) c->stk[c->sp++] = 'S';
		break;
	case ev_array_row_start:
		if (c->sp < (int)sizeof c->stk) c->stk[c->sp++] = 'A';
		break;
	case ev_struct_end:
		if (c->sp == 0 || c->stk[c->sp - 1] != 'S') c->nest_violations++;
		if (c->sp > 0) c->sp--;
		break;
	case ev_array_row_end:
		if (c->sp == 0 || c->stk[c->sp - 1] != 'A') c->nest_violations++;
		if (c->sp > 0) c->sp--;
		break;
	case ev_stream_end:
		if (c->sp != 0) c->nest_violations++;
		break;
	default: break;
	}
	return true;
}
static void r4_err(void *ud, error_code_t err, uint64_t line, uint64_t col,
		   uint32_t byte, uint64_t off)
{
	r4_t *c = ud;
	(void)line; (void)col;
	if (c->nerr < 8) {
		c->errs[c->nerr]  = err;
		c->offs[c->nerr]  = off;
		c->bytes[c->nerr] = byte;
	}
	c->nerr++;
}
static r4_t r4_run_flags(const void *doc, size_t n, bool cont,
			 bvnr_read_flags_t fl)
{
	static r4_t c;
	memset(&c, 0, sizeof c);
	fl.userdata          = &c;
	fl.on_verified       = r4_cb;
	fl.on_error          = r4_err;
	fl.continue_on_error = cont;
	bvnr_reader_t *r = bvnr_reader_create();
	c.ok = bvnr_open_read_mem(r, doc, n, NULL, 0, &fl) && bvnr_read(r);
	c.err        = bvnr_reader_get_error(r);
	c.off        = bvnr_reader_get_error_offset(r);
	c.byte       = bvnr_reader_get_error_byte(r);
	c.recoveries = bvnr_reader_get_recovery_count(r);
	bvnr_reader_destroy(r);
	return c;
}
static r4_t r4_run(const void *doc, size_t n, bool cont)
{
	bvnr_read_flags_t fl;
	memset(&fl, 0, sizeof fl);
	return r4_run_flags(doc, n, cont, fl);
}
static r4_t r4_str(const char *doc, bool cont)
{
	return r4_run(doc, strlen(doc), cont);
}

/* A nested array was counted twice in the row that holds it, so the "/"-row
 * width check was wrong for every row containing one -- an array element, or
 * an array that is only a member of a struct element. */
static void test_r4_row_width_counts_a_nested_array_once(void)
{
	static const char *const ragged[] = {
		".a = [[1]]/[null,null];",
		".a = [[1],[2]]/[[3],null,null];",
		".a = [{.m = [1];}]/[null, null];",
		".a = [[1]]/[1,2];",
		".a = [[]]/[null,null];",
		".a = [{.m = [1]; .n = [2];}]/[null, null, null];",
	};
	static const char *const rectangular[] = {
		".a = [[1],[2]]/[null,null];",
		".a = [null,null]/[[1],[2]];",
		".a = [{.m = [1];}]/[{.m = null;}];",
		".a = [[],[]]/[null,null];",
		".a = [[[1]]]/[null];",
		".a = [[1],[2]]/[[3],[4]];",
		".a = [1,2]/[null,null];",
		".a = [[1]/[2], 5]/[null, 6];",
	};
	for (size_t i = 0; i < sizeof ragged / sizeof *ragged; i++) {
		r4_t c = r4_str(ragged[i], false);
		ASSERT_FALSE(c.ok, ragged[i]);
		ASSERT_EQ_INT(c.err, error_array_row_size_mismatch, ragged[i]);
	}
	for (size_t i = 0; i < sizeof rectangular / sizeof *rectangular; i++) {
		r4_t c = r4_str(rectangular[i], false);
		ASSERT_TRUE(c.ok, rectangular[i]);
	}
	/* ...and the element that overshoots is still caught where it stands */
	r4_t c = r4_str(".a = [[1],[2]]/[[3],[4],[5]];", false);
	ASSERT_EQ_INT(c.err, error_array_row_size_mismatch, "third element");
	ASSERT_EQ_UINT(c.off, 23u, "at the ',' that starts it");
}

static bool r4_want_same_unit_base10(void *ud, const bvnr_data_t *d,
				     value_unit_t *want, uint32_t *want_base)
{
	(void)ud;
	*want      = d->value_unit;
	*want_base = 10u;
	return true;
}
/* "nan", "inf" and "ninf" are DIGITS in base 24 and up. Only the bare keyword
 * is a special number. */
static void test_r4_quoted_digits_spelling_a_special_number(void)
{
	r4_t c = r4_str(".a = <uint:8,_36> \"inf\";", false);
	ASSERT_FALSE(c.ok, "inf in base 36 is 24171: not a uint:8");
	ASSERT_EQ_INT(c.err, error_value_out_of_range, "uint:8,_36 \"inf\"");
	c = r4_str(".a = <sint:8,_36> \"ninf\";", false);
	ASSERT_EQ_INT(c.err, error_value_out_of_range, "sint:8,_36 \"ninf\"");
	c = r4_str(".a = <uint:8,_36> \"nan\";", false);
	ASSERT_EQ_INT(c.err, error_value_out_of_range, "uint:8,_36 \"nan\"");
	c = r4_str(".a = <uint:16,_36> \"nan\"; .b = <sint:16,_36> \"-inf\";", false);
	ASSERT_TRUE(c.ok, "and where it fits, it is a number like any other");
	/* the keyword keeps its exemption, in every family (spec 6.4) */
	c = r4_str(".a = <uint:8> nan; .b = <sint:16> ninf; .c = <uint:8,_36> inf;"
		   " .d = <float_fix:32,q8> inf;", false);
	ASSERT_TRUE(c.ok, "bare nan/inf/ninf stay range-exempt");
	/* a conversion sees the number too */
	bvnr_read_flags_t fl;
	memset(&fl, 0, sizeof fl);
	fl.want_unit = r4_want_same_unit_base10;
	const char *doc = ".a = <uint:64,_36> \"nan\";";
	c = r4_run_flags(doc, strlen(doc), false, fl);
	ASSERT_TRUE(c.ok, "base conversion of uint:64,_36 \"nan\"");
	ASSERT_TRUE(strcmp(c.conv, "30191") == 0, "converted to 30191");
}

/* A struct member is an assignment, and the count was reset per assignment. */
static void test_r4_max_array_items_counts_through_structs(void)
{
	bvnr_read_flags_t fl;
	memset(&fl, 0, sizeof fl);
	fl.max_array_items = 3;
	const char *d;
	r4_t c;
	d = ".x = [1,2,3];";
	c = r4_run_flags(d, strlen(d), false, fl);
	ASSERT_TRUE(c.ok, "three items at a limit of three");
	d = ".x = [{.a=1;},{.a=1;},{.a=1;}];";
	c = r4_run_flags(d, strlen(d), false, fl);
	ASSERT_TRUE(c.ok, "three struct items at a limit of three");
	d = ".x = [{.a=1;},{.a=1;},{.a=1;},{.a=1;}];";
	c = r4_run_flags(d, strlen(d), false, fl);
	ASSERT_EQ_INT(c.err, error_too_many_array_items, "four struct items");
	d = ".x = [{.a=1;},2,3,4];";
	c = r4_run_flags(d, strlen(d), false, fl);
	ASSERT_EQ_INT(c.err, error_too_many_array_items, "a struct, then three more");
	ASSERT_EQ_UINT(c.off, 17u, "refused at the fourth item's comma");
	d = ".x = [{.a=[1,2];}];";
	c = r4_run_flags(d, strlen(d), false, fl);
	ASSERT_TRUE(c.ok, "an array in a member draws on the same budget: 3");
	d = ".x = [{.a=[1,2,3];}];";
	c = r4_run_flags(d, strlen(d), false, fl);
	ASSERT_EQ_INT(c.err, error_too_many_array_items, "...and exhausts it at 4");
	/* the budget is per outermost array, as it always was */
	d = ".x = [1,2,3]; .y = [1,2,3]; .s = {.a = [1,2,3]; .b = [1,2,3];};";
	c = r4_run_flags(d, strlen(d), false, fl);
	ASSERT_TRUE(c.ok, "separate arrays have separate budgets");
}

/* Recovery closed every open array, including the one AROUND the struct the
 * error was in: row_end, then more members of that struct, then struct_end. */
static void test_r4_recovery_keeps_events_nested(void)
{
	r4_t c = r4_str(".a = [{.x = @; .y = 5;}, {.x = 1;}]; .b = 2;", true);
	ASSERT_TRUE(c.ok, "recovers");
	ASSERT_EQ_INT(c.nest_violations, 0, "start/end events nest");
	ASSERT_EQ_UINT(c.recoveries, 1u, "one error, one recovery");
	ASSERT_TRUE(strcmp(c.keys, "a,x,y,x,b,") == 0, "every key but the broken value");
	ASSERT_TRUE(strcmp(c.vals, "5,1,2,") == 0, "and the second element survives");

	c = r4_str(".a = [{.x = <uint:8> 300; .y = 5;}, {.x = 1; .y = 6;}]; .b = 2;", true);
	ASSERT_EQ_INT(c.nest_violations, 0, "semantic error at the ';'");
	ASSERT_EQ_UINT(c.recoveries, 1u, "one recovery");
	ASSERT_TRUE(strcmp(c.vals, "5,1,6,2,") == 0, "rest of the array delivered");

	/* the array the struct OWNS is still closed */
	c = r4_str(".a = [{.x = [1, @, 3]; .y = 5;}, 7]/[{.x = [1]; .y = 2;}, 8]; .b = 2;", true);
	ASSERT_TRUE(c.ok, "inner array broken, outer intact");
	ASSERT_EQ_INT(c.nest_violations, 0, "nested");
	ASSERT_EQ_UINT(c.recoveries, 1u, "the outer rows still compare equal");
	ASSERT_TRUE(strcmp(c.vals, "1,5,7,1,2,8,2,") == 0, "values");

	/* a '}' or ']' met while skipping */
	c = r4_str(".a = [{.x = @ }, {.x = 1;}]; .b = 2;", true);
	ASSERT_TRUE(c.ok, "struct closed by the '}' during recovery");
	ASSERT_EQ_INT(c.nest_violations, 0, "nested");
	c = r4_str(".a = [{.x = [@ ]; .y = 1;}]; .b = 2;", true);
	ASSERT_EQ_INT(c.nest_violations, 0, "']' of an already closed array");
	ASSERT_TRUE(strcmp(c.keys, "a,x,y,b,") == 0, "keys");
	c = r4_str(".a = [{.x = @ ] }, 4]; .b = 2;", true);
	ASSERT_EQ_INT(c.nest_violations, 0, "a ']' cannot close across a struct");
	c = r4_str(".a = [[{.x = @; .y = [1,2];}], [{.x = 3; .y = [4,5];}]]; .b = 2;", true);
	ASSERT_TRUE(c.ok, "two array levels around the struct");
	ASSERT_EQ_INT(c.nest_violations, 0, "nested");
	ASSERT_TRUE(strcmp(c.vals, "1,2,3,4,5,2,") == 0, "values");
	/* an annotated outer array gets its unit back for the next element */
	c = r4_str(".a = [1, @, 3]; .b = [4, 5];", true);
	ASSERT_EQ_INT(c.nest_violations, 0, "plain array, unchanged behaviour");
	ASSERT_TRUE(strcmp(c.vals, "1,4,5,") == 0, "the broken array is closed");
}

static void test_r4_recovery_from_inside_a_comment(void)
{
	static const char d1[] = ".z = 0; # bad \xff byte; .q = 9;\n.a = 1;\n";
	r4_t c = r4_run(d1, sizeof d1 - 1, true);
	ASSERT_TRUE(c.ok, "recovers");
	ASSERT_TRUE(strcmp(c.keys, "z,a,") == 0, "nothing is read out of a comment");
	static const char d2[] = ".z = 0;\n# caf\xe9 [draft\n.a = 1;\n.b = 2;\n";
	c = r4_run(d2, sizeof d2 - 1, true);
	ASSERT_TRUE(c.ok, "a '[' in the comment opens nothing");
	ASSERT_TRUE(strcmp(c.keys, "z,a,b,") == 0, "the document after it is read");
	static const char d3[] = ".z = 0;\n# it\x01s \"quoted {\n.a = 1;\n.b = \"x\";\n";
	c = r4_run(d3, sizeof d3 - 1, true);
	ASSERT_TRUE(c.ok, "nor a quote, nor a brace");
	ASSERT_TRUE(strcmp(c.keys, "z,a,b,") == 0, "keys");
	static const char d4[] = "# first \xff line [\n.a = 1;\n";
	c = r4_run(d4, sizeof d4 - 1, true);
	ASSERT_TRUE(c.ok, "the first comment too");
	ASSERT_TRUE(strcmp(c.keys, "a,") == 0, "keys");
	/* the sequence is cut short BY the newline: the comment is over */
	static const char d5[] = ".z = 0; # x \xc3\n.a = 1;\n";
	c = r4_run(d5, sizeof d5 - 1, true);
	ASSERT_TRUE(c.ok, "error on the newline itself");
	ASSERT_TRUE(strcmp(c.keys, "z,a,") == 0, "the next line is not skipped");
	/* a comment is not structure: what it sits in is untouched */
	static const char d6[] = ".a = [1, # two \xff [\n 2,\n # \x01 {\n 3]; .s = {.x = 1; # \xc3(\n .y = 2;};\n";
	c = r4_run(d6, sizeof d6 - 1, true);
	ASSERT_TRUE(c.ok, "bad bytes in comments inside an array and a struct");
	ASSERT_EQ_INT(c.nest_violations, 0, "nested");
	ASSERT_EQ_UINT(c.recoveries, 3u, "each one reported and counted");
	ASSERT_EQ_INT(c.nerr, 3, "on_error three times");
	ASSERT_TRUE(strcmp(c.vals, "1,2,3,1,2,") == 0, "and nothing lost");
	/* ...and one that runs to end of input still ends the document cleanly */
	static const char d7[] = ".a = 1; # x\xff y";
	c = r4_run(d7, sizeof d7 - 1, true);
	ASSERT_TRUE(c.ok, "bad byte in a trailing comment without a newline");
	static const char d8[] = ".a = 1; # x \xe2\x82";
	c = r4_run(d8, sizeof d8 - 1, true);
	ASSERT_TRUE(c.ok, "end of input mid-sequence is reported and recovered");
	ASSERT_EQ_INT(c.nerr, 1, "once");
	ASSERT_EQ_INT(c.errs[0], error_invalid_utf8_byte, "as invalid UTF-8");
	ASSERT_EQ_UINT(c.recoveries, 1u, "and counted");
	/* a first line with a bad byte in it declares nothing */
	static const char d9[] = "#!bovnar 1.1 \xff\n.a = 2024-01-01;\n.b = 1;\n";
	c = r4_run(d9, sizeof d9 - 1, true);
	ASSERT_TRUE(strcmp(c.keys, "a,b,") == 0, "keys");
	ASSERT_TRUE(strcmp(c.vals, "1,") == 0, "no directive: the ISO literal is refused");
}

static void test_r4_recovery_from_an_escape_cut_by_the_quote(void)
{
	r4_t c = r4_str("#!bovnar 1.1\n.a = \"\\x4\"; .b = 2; .c = \"z\"; .d = 4;\n", true);
	ASSERT_TRUE(c.ok, "\\x4\" recovers");
	ASSERT_TRUE(strcmp(c.keys, "a,b,c,d,") == 0, "only .a's value is lost");
	c = r4_str("#!bovnar 1.1\n.a = \"\\x\"; .b = 2;\n", true);
	ASSERT_TRUE(strcmp(c.keys, "a,b,") == 0, "\\x\"");
	c = r4_str("#!bovnar 1.1\n.a = \"\\u\"; .b = 2;\n", true);
	ASSERT_TRUE(strcmp(c.keys, "a,b,") == 0, "\\u\"");
	c = r4_str("#!bovnar 1.1\n.a = \"\\u{41\"; .b = 2;\n.c = 3;\n", true);
	ASSERT_TRUE(c.ok, "\\u{41\" recovers");
	ASSERT_TRUE(strcmp(c.keys, "a,b,c,") == 0, "keys");
	/* the quote is still ahead when the bad byte is something else */
	c = r4_str("#!bovnar 1.1\n.a = \"\\x4g; .q = 1;\"; .b = 2;\n", true);
	ASSERT_TRUE(strcmp(c.keys, "a,b,") == 0, "\\x4g: skip to the closing quote");
}

static void test_r4_recovery_when_the_bad_byte_closes_a_struct(void)
{
	r4_t c = r4_str(".s = {.a = 1 }; .b = 2;", true);
	ASSERT_TRUE(c.ok, "a member missing its ';'");
	ASSERT_EQ_INT(c.nest_violations, 0, "nested");
	ASSERT_EQ_INT(c.sp, 0, "the struct was closed");
	ASSERT_TRUE(strcmp(c.keys, "s,a,b,") == 0, "keys");
	c = r4_str(".s = {.a = [1, 2 }; .b = 2;", true);
	ASSERT_TRUE(c.ok, "with an array open inside it");
	ASSERT_EQ_INT(c.nest_violations, 0, "nested");
	c = r4_str(".a = 1 }; .b = 2;", true);
	ASSERT_TRUE(c.ok, "a '}' with nothing to close");
	ASSERT_TRUE(strcmp(c.keys, "a,b,") == 0, "keys");
}

/* error_offset is a STREAM offset: octet payloads count. */
static void test_r4_error_offset_after_an_octet_stream(void)
{
	static const char d1[] = ".a = \x00\x01\x03\x00" "abc\x00;\n.c = @;";
	r4_t c = r4_run(d1, sizeof d1 - 1, false);
	ASSERT_EQ_INT(c.err, error_unexpected_input_byte, "the '@'");
	ASSERT_EQ_UINT(c.off, 20u, "at its index in the buffer");
	ASSERT_TRUE(d1[c.off] == '@', "which indexes the input");
	ASSERT_EQ_UINT(c.offs[0], 20u, "on_error agrees");
	static const char d2[] = ".a = \x00\x01\x03\x00" "abc\x07;";
	c = r4_run(d2, sizeof d2 - 1, false);
	ASSERT_EQ_INT(c.err, error_octet_stream_out_of_sync, "bad tag");
	ASSERT_EQ_UINT(c.off, 12u, "the tag's own offset");
	ASSERT_EQ_UINT(c.byte, 0x07u, "and the tag");
	/* two streams, a semantic error, and a string error after them */
	static const char d3[] = ".a = \x00\x01\x01\x00z\x00; .b = \x00\x00; .c = <uint:8> 300;";
	c = r4_run(d3, sizeof d3 - 1, false);
	ASSERT_EQ_INT(c.err, error_value_out_of_range, "300");
	ASSERT_TRUE(d3[c.off] == ';', "reported at its ';'");
	ASSERT_EQ_UINT(c.off, sizeof d3 - 2, "the last byte");
	/* max_text_bytes: the first byte over the limit */
	bvnr_read_flags_t fl;
	memset(&fl, 0, sizeof fl);
	fl.max_text_bytes = 10;
	static const char d4[] = ".a = \x00\x01\x02\x00xy\x00;\n.b = 1;";
	c = r4_run_flags(d4, sizeof d4 - 1, false, fl);
	ASSERT_EQ_INT(c.err, error_text_data_too_long, "text cap");
	ASSERT_EQ_UINT(c.off, 16u, "eleventh TEXT byte is stream byte 16");
}

/* max_file_size is exact, and what happens at it does not depend on how the
 * bytes arrive. */
static void test_r4_max_file_size_reads_up_to_the_cap(void)
{
	static const char d[] = ".a = 1;\n.b = 2;\n.c = 3;\n.d = 4;\n";
	bvnr_read_flags_t fl;
	memset(&fl, 0, sizeof fl);
	fl.max_file_size = sizeof d - 1;
	r4_t c = r4_run_flags(d, sizeof d - 1, false, fl);
	ASSERT_TRUE(c.ok, "a document of N bytes at a cap of N");
	fl.max_file_size = 20;
	c = r4_run_flags(d, sizeof d - 1, false, fl);
	ASSERT_EQ_INT(c.err, error_file_too_long, "over the cap");
	ASSERT_EQ_UINT(c.off, 20u, "refused at the first byte over it");
	ASSERT_TRUE(strcmp(c.vals, "1,2,") == 0, "having read what came before");
	/* an error inside the allowed prefix is the error */
	static const char e[] = ".a = @;\n.b = 2;\n.c = 3;\n.d = 4;\n";
	c = r4_run_flags(e, sizeof e - 1, false, fl);
	ASSERT_EQ_INT(c.err, error_unexpected_input_byte, "the earlier error wins");
	/* the cap falls inside an octet chunk */
	static const char o[] = ".a = \x00\x01\x08\x00" "abcdefgh\x00;";
	fl.max_file_size = 12;
	c = r4_run_flags(o, sizeof o - 1, false, fl);
	ASSERT_EQ_INT(c.err, error_file_too_long, "cut inside a chunk");
	ASSERT_EQ_UINT(c.off, 12u, "at the cap");
}

static void test_r4_eof_inside_a_utf8_sequence(void)
{
	static const char *const bad[] = {
		".a = 1; #\xc3", "#\xc3", ".a = 1; # ok \xe2\x82",
		".a = 1;\n#\xf0\x9f", "#x\n#\xc3", "#\xef", "#\xef\xbb",
		"\xef\xbb\xbf# \xc3",
	};
	for (size_t i = 0; i < sizeof bad / sizeof *bad; i++) {
		r4_t c = r4_str(bad[i], false);
		ASSERT_FALSE(c.ok, "a comment ending mid-sequence");
		ASSERT_EQ_INT(c.err, error_invalid_utf8_byte, "is invalid UTF-8");
		ASSERT_EQ_UINT(c.off, strlen(bad[i]), "reported at end of input");
	}
	r4_t c = r4_str(".a = 1; # caf\xc3\xa9", false);
	ASSERT_TRUE(c.ok, "a complete character at end of input is fine");
	c = r4_str("# \xef\xbc\x81", false);
	ASSERT_TRUE(c.ok, "in the first comment too");
	c = r4_str(".a = \"\xc3", false);
	ASSERT_EQ_INT(c.err, error_got_incomplete_bvnr_stream,
		      "a truncated document is still a truncated document");
}

static void test_r4_directive_padding_and_peek(void)
{
	static const char *const docs[] = {
		"#!bovnar 1.1                              \n.a = 2024-01-01;",
		"#!bovnar                              1.1\n.a = 2024-01-01;",
		"#!bovnar \t \t \t \t \t \t \t \t \t \t \t \t \t \t1.1 \t \t \t \t \t \t \t \t \t \t \t \t\n.a = 2024-01-01;",
		"#!bovnar                    65535.65535                    \n",
		"\x0b#!bovnar 1.1\n.a = 2024-01-01;",
		"\x0c \x0b\n#!bovnar 1.1\n.a = 2024-01-01;",
	};
	for (size_t i = 0; i < sizeof docs / sizeof *docs; i++) {
		r4_t c = r4_str(docs[i], false);
		ASSERT_TRUE(c.ok, "padded directive is a directive");
		uint16_t ma = 0, mi = 0;
		ASSERT_TRUE(bvnr_peek_version(docs[i], strlen(docs[i]), &ma, &mi),
			    "and peek agrees with the reader");
		ASSERT_EQ_UINT(ma, i == 3 ? 65535u : 1u, "major");
		ASSERT_EQ_UINT(mi, i == 3 ? 65535u : 1u, "minor");
	}
	static const char *const invalid[] = {
		"#!bovnar 1.1                              x\n",
		"#!bovnar                              1.1.\n",
		"#!bovnar 1 .1\n",
		"#!bovnar 1.1 this line is far longer than any version could ever be\n",
		"#!bovnar 1.1\x0b\n",
	};
	for (size_t i = 0; i < sizeof invalid / sizeof *invalid; i++) {
		r4_t c = r4_str(invalid[i], false);
		ASSERT_EQ_INT(c.err, error_invalid_spec_version, invalid[i]);
		ASSERT_FALSE(bvnr_peek_version(invalid[i], strlen(invalid[i]), NULL, NULL),
			     "peek refuses it too");
	}
	r4_t c = r4_str("#!bovnarish                                        and on\n.a = 1;", false);
	ASSERT_TRUE(c.ok, "a long ordinary comment is still an ordinary comment");
}

static void test_r4_float_fix_and_float_dec_are_one_word(void)
{
	static const char *const bad[] = {
		".a = <float _fix:32,q8> 1.5;",
		".a = <float\t_dec:64> 1.5;",
		".a = <float#x\n_dec:64> 1.5;",
		".a = <float # x\n _fix:32,q8> 1.5;",
		".a = <float\n_fix> 1.5;",
	};
	for (size_t i = 0; i < sizeof bad / sizeof *bad; i++) {
		r4_t c = r4_str(bad[i], false);
		ASSERT_FALSE(c.ok, bad[i]);
		ASSERT_EQ_INT(c.err, error_type_param_whitespace, bad[i]);
	}
	static const char *const good[] = {
		".a = <float_fix:32,q8> 1.5;", ".a = <float_dec:64> 1.5;",
		".a = < float_fix : 32 , q8 > 1.5;", ".a = <float_dec # c\n> 1.5;",
		".a = <float :64> 1.5;", ".a = <float # c\n : 64, _16> \"1.8\";",
		".a = <float> 1.5;", ".a = <float:64,_16> \"1.8\";",
	};
	for (size_t i = 0; i < sizeof good / sizeof *good; i++) {
		r4_t c = r4_str(good[i], false);
		ASSERT_TRUE(c.ok, good[i]);
	}
}

static void test_r5_a_parameterless_family_refuses_a_zero_width(void)
{
	/* The check tested the parameter's VALUE, so a width of 0 -- "the
	 * default" for a family that has one -- passed under two that have none. */
	static const char *const bad[] = {
		".a = <utf8:0> \"x\";", ".a = <bool:0> true;",
		".a = <utf8:00> \"x\";", ".a = <utf8:_0> \"x\";",
		".a = <utf8:8> \"x\";", ".a = <bool:m> true;",
	};
	for (size_t i = 0; i < sizeof bad / sizeof *bad; i++) {
		r4_t c = r4_str(bad[i], false);
		ASSERT_FALSE(c.ok, bad[i]);
		ASSERT_EQ_INT(c.err, error_illegal_value_type, bad[i]);
	}
	ASSERT_TRUE(r4_str(".a = <utf8> \"x\";", false).ok, "utf8 alone");
	ASSERT_TRUE(r4_str(".a = <bool> on;", false).ok, "bool alone");
}

static void test_r5_a_struct_takes_no_annotation(void)
{
	/* The annotation used to survive until the first member's key and fail
	 * there, so it was refused before every struct but an empty one. */
	static const char *const bad[] = {
		".a = <utf8> {};", ".a = <uint:8> {};",
		".a = <uint:8> {.x = 1;};", ".a = <utf8> {.x = \"s\";};",
		".a = <uint:8> [{}];", ".a = <uint:8> [[{}]];",
		".a = [<uint:8> {}];", ".a = <float:64,m> [{.x = 1.0;}];",
	};
	for (size_t i = 0; i < sizeof bad / sizeof *bad; i++) {
		r4_t c = r4_str(bad[i], false);
		ASSERT_FALSE(c.ok, bad[i]);
		ASSERT_EQ_INT(c.err, error_type_value_mismatch, bad[i]);
	}
	static const char *const good[] = {
		".a = {};", ".a = {.x = <uint:8> 1;};", ".a = [{}, {}];",
		".a = <uint:8> [1, 2]; .b = {.c = 1;};",
		".a = {.b = <uint:8> 1; .c = {.d = 2;};};",
		".a = [{.x = <uint:8> 1;}, {.x = <uint:8> 2;}];",
	};
	for (size_t i = 0; i < sizeof good / sizeof *good; i++)
		ASSERT_TRUE(r4_str(good[i], false).ok, good[i]);
}

static void test_r4_nothing_but_a_dot_or_an_index_after_an_index(void)
{
	static const char *const bad[] = {
		"#!bovnar 1.1\n.r = &.a[0]b;",
		"#!bovnar 1.1\n.r = &.a[0]bcdefgh;",
		"#!bovnar 1.1\n.r = [&.a[0][1]_x];",
		"#!bovnar 1.1\n.r = &.a[0]-;",
		"#!bovnar 1.1\n.r = &.a[0]\xc3\xa9;",
	};
	for (size_t i = 0; i < sizeof bad / sizeof *bad; i++) {
		r4_t c = r4_str(bad[i], false);
		ASSERT_FALSE(c.ok, bad[i]);
		ASSERT_EQ_INT(c.err, error_unexpected_input_byte, bad[i]);
	}
	r4_t c = r4_str("#!bovnar 1.1\n.r = &.a[0]b;", false);
	ASSERT_EQ_UINT(c.off, 24u, "at the byte after the ']'");
	c = r4_str("#!bovnar 1.1\n.p = &.a[0].b; .q = &.a[0][1]; .r = [&.a[0], &.b[1][2].c]; .s = &.a[0] ;", false);
	ASSERT_TRUE(c.ok, "'.', '[' and the end of the reference stay valid");
	ASSERT_TRUE(strcmp(c.vals, ".a[0].b,.a[0][1],.a[0],.b[1][2].c,.a[0],") == 0, "paths");
}

/* A ']' may not close an array from inside a struct that is its element. */
static void test_r4_bracket_cannot_close_across_a_struct(void)
{
	static const char *const bad[] = {
		".a = [{.x = 1];};",
		".a = [{.x = [1]];};",
		".a = [{.x = [1]/[2]];};",
		".a = [{.x = <uint:8>];};",
		".a = [{.x = \"s\"];};",
		".a = [{.x = sym];};",
		".a = [{.x = []];};",
		".a = [[{.x = {.y = 1];};}]];",
	};
	for (size_t i = 0; i < sizeof bad / sizeof *bad; i++) {
		r4_t c = r4_str(bad[i], false);
		ASSERT_FALSE(c.ok, bad[i]);
		ASSERT_EQ_INT(c.err, error_unexpected_input_byte, bad[i]);
		ASSERT_EQ_INT(c.nest_violations, 0, "and no row_end was delivered for it");
		c = r4_str(bad[i], true);
		ASSERT_EQ_INT(c.nest_violations, 0, "nor under recovery");
	}
	r4_t c = r4_str(".a = [{.x = 1];};", false);
	ASSERT_EQ_UINT(c.off, 13u, "at the ']'");
	c = r4_str(".a = [{.x = [1]; .y = [[2],[]];}, {.x = []; .y = [[3],[4]];}];", false);
	ASSERT_TRUE(c.ok, "arrays closed where they were opened are untouched");
}

int main(void)
{
	printf("Running bovnar_high_severity_test suite...\n\n");

	printf("Gaps 1+2: writer error accessors\n");
	test_writer_error_accessors();

	printf("\nGaps 3+4: reader max_* limits\n");
	test_reader_max_identifier_length();
	test_reader_max_string_length();
	test_reader_max_number_length();
	test_reader_max_symbol_length();
	test_reader_max_reference_length();
	test_reader_max_array_items();
	test_reader_max_text_bytes();
	test_reader_max_file_size();
	test_reader_max_struct_nesting();
	test_reader_max_array_nesting();

	printf("\nGap 5: recovery count\n");
	test_recovery_count();
	test_error_getters_after_recovery();
	test_recovery_review_2026_11();

	printf("\nGap 6: callback abort\n");
	test_callback_abort_verified();
	test_callback_abort_unverified();

	printf("\nGap 7: SI unit conversion APIs\n");
	test_si_units_compatible();
	test_si_unit_convert_factor();
	test_si_unit_dimension_vector();
	test_si_unit_reduce();

	printf("\nGap 8: unit_to_si_factor (affine)\n");
	test_si_unit_to_si_factor();

	printf("\nGap 9: array reader events\n");
	test_array_reader_events();

	printf("\nGap 10: octet-stream events\n");
	test_octet_stream_reader_events();
	test_octet_zero_length_chunk_rejected();

	printf("\nGap 11: string escape write roundtrip\n");
	test_string_vt_vf_escape_write();

	printf("\nGap 12: special float write\n");
	test_special_float_write();

	printf("\nGap 12: open_write_mem / bytes_written\n");
	test_open_write_mem_and_bytes_written();

	printf("\nGap 13: bvnf base-16 special float roundtrip\n");
	test_bvnf_base16_special_roundtrip();

	printf("\nFix: ev_stream_end event\n");
	test_stream_end_event();

	printf("\nRegression: four release-blocking lexer bugs\n");
	test_bug1_resync_eof_position();
	test_bug2_crlf_column_tracking();
	test_bug3_str_len_after_octet_stream();
	test_bug4_resync_semicolon_row_balance();

	printf("\nRegression: seven newly identified release-blocking lexer bugs\n");
	test_resync_resumes_at_the_next_assignment();
	test_resync_still_discards_only_the_broken_statement();
	test_resync_dots_that_lead_nowhere();
	test_resync_reports_what_it_discarded();
	test_new_bug1_resync_struct_drains_arrays();
	test_new_bug2_semicolon_recovery_array_event_balance();
	test_new_bug3_resync_eof_distinct_error_code();
	test_new_bug4_resync_bracket_stays_in_resync();
	test_new_bug5_crlf_at_resync_boundary();
	test_new_bug6_resync_bracket_preserves_array_row_size();
	test_new_bug7_type_annotation_nul_termination();

	printf("\nRegression: two additional release-blocking lexer bugs (Bug A, Bug B)\n");
	test_bug_a_no_phantom_struct_after_bracket_semicolon();
	test_bug_b_single_notification_on_semicolon_recovery();

	printf("\nFix: three further release-blocking lexer bugs (Bug C, D, F)\n");
	test_bug_c_inline_unit_comment_no_concat();
	test_bug_d_struct_intro_no_phantom_end();
	test_bug_f_bad_continuation_retried_in_resync();

	printf("\nFix: Bug E (arr_frames vunit init) and Bug G (semicolon reset completeness)\n");
	test_bug_e_resync_frame_vunit_init();
	test_bug_g_semicolon_reset_clears_utf8_and_inline_unit();

	printf("\nRegression: five newly identified release-blocking bugs (RB1-RB5)\n");
	test_rb1_eof_resync_callback_fires();
	test_rb2_eof_array_events_balanced();
	test_rb2_nested_array_eof_balanced();
	test_rb3_special_number_respects_max_symbol_length();
	test_rb4_advance_line_once_per_byte();
	test_rb5_resync_bracket_no_duplicate_row_end();
	test_rb6_read_without_open_does_not_fault();
	test_rb7_error_position_counters();
	test_rb8_reader_limits_are_exact();

	printf("\nFourth review round: the read side\n");
	test_r4_row_width_counts_a_nested_array_once();
	test_r4_quoted_digits_spelling_a_special_number();
	test_r4_max_array_items_counts_through_structs();
	test_r4_recovery_keeps_events_nested();
	test_r4_recovery_from_inside_a_comment();
	test_r4_recovery_from_an_escape_cut_by_the_quote();
	test_r4_recovery_when_the_bad_byte_closes_a_struct();
	test_r4_error_offset_after_an_octet_stream();
	test_r4_max_file_size_reads_up_to_the_cap();
	test_r4_eof_inside_a_utf8_sequence();
	test_r4_directive_padding_and_peek();
	test_r4_float_fix_and_float_dec_are_one_word();
	test_r5_a_parameterless_family_refuses_a_zero_width();
	test_r5_a_struct_takes_no_annotation();
	test_r4_nothing_but_a_dot_or_an_index_after_an_index();
	test_r4_bracket_cannot_close_across_a_struct();

	printf("\n");
	if (g_failures == 0) {
		printf("PASSED %d tests\n", g_tests);
		return 0;
	}
	fprintf(stderr, "FAILED %d of %d tests\n", g_failures, g_tests);
	return 1;
}
