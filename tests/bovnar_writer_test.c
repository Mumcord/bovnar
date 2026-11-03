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
#include "bvn_float.h"
#include "bvn_int.h"

static int failures = 0;
static int tests    = 0;

#define ASSERT_TRUE(cond, msg) do {                                     \
	tests++;                                                            \
	if (!(cond)) {                                                      \
		fprintf(stderr, "FAIL line %d: %s\n", __LINE__, (msg));        \
		failures++;                                                     \
	}                                                                   \
} while (0)

#define ASSERT_EQ_INT(a, b, msg) do {                                   \
	tests++;                                                            \
	int64_t _a = (int64_t)(a);                                         \
	int64_t _b = (int64_t)(b);                                         \
	if (_a != _b) {                                                     \
		fprintf(stderr, "FAIL line %d: %s\n  got %lld, expected %lld\n",\
				__LINE__, (msg), (long long)_a, (long long)_b);        \
		failures++;                                                     \
	}                                                                   \
} while (0)

#define ASSERT_EQ_UINT(a, b, msg) do {                                  \
	tests++;                                                            \
	uint64_t _a = (uint64_t)(a);                                       \
	uint64_t _b = (uint64_t)(b);                                       \
	if (_a != _b) {                                                     \
		fprintf(stderr, "FAIL line %d: %s\n  got %llu, expected %llu\n",\
				__LINE__, (msg),                                        \
				(unsigned long long)_a, (unsigned long long)_b);       \
		failures++;                                                     \
	}                                                                   \
} while (0)

#define ASSERT_NOT_NULL(ptr, msg) do {                                  \
	tests++;                                                            \
	if ((ptr) == NULL) {                                                \
		fprintf(stderr, "FAIL line %d: %s\n  got null pointer\n",      \
				__LINE__, (msg));                                       \
		failures++;                                                     \
	}                                                                   \
} while (0)

#define ASSERT_FALSE(cond, msg) ASSERT_TRUE(!(cond), (msg))

static bvnr_writer_t *make_writer(uint8_t *buf, uint32_t cap, bvnr_sink_t *sink)
{
	bvnr_sink_to_mem(sink, buf, cap);
	bvnr_writer_t *w = bvnr_writer_create();
	if (!w) return NULL;
	if (!bvnr_open_write_sink(w, sink, true, NULL)) {
		bvnr_writer_destroy(w);
		return NULL;
	}
	bvnr_data_t hdr = {0};
	if (!bvnr_write_event(w, ev_stream_start, &hdr)) {
		bvnr_writer_destroy(w);
		return NULL;
	}
	return w;
}

typedef struct {
	char    key[256];
	char    value[256];
	uint32_t value_len;
	value_type_spec_t vt;
	token_type_t value_tok;
	/* The unit the reader resolved for the value, so a test can compare what
	 * came back against what it asked to be written. */
	value_unit_t unit;
} last_event_t;

static bool on_rt(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	last_event_t *le = ud;
	if (ev == ev_assignment_start && d->length < sizeof(le->key)) {
		memcpy(le->key, d->data, d->length);
		le->key[d->length] = '\0';
	}

	if (ev == ev_type_annotation_type_family_parameter)
		le->vt = d->value_type;
	if (ev == ev_data && d->length < sizeof(le->value)) {
		memcpy(le->value, d->data, d->length);
		le->value_len = d->length;
		le->value[d->length] = '\0';
		le->value_tok = d->type;
		le->unit      = d->value_unit;

		if (le->vt.family == vt_plain)
			le->vt = d->value_type;
	}
	return true;
}

static bool roundtrip(const uint8_t *buf, uint64_t len, last_event_t *le)
{
	bvnr_reader_t *r = bvnr_reader_create();
	if (!r) return false;
	bvnr_read_flags_t opts = { .on_verified = on_rt, .userdata = le };
	bool ok = bvnr_open_read_mem(r, buf, len, NULL, 0, &opts)
		   && bvnr_read(r);
	bvnr_reader_destroy(r);
	return ok;
}

static void test_write_simple_strings(void)
{
	printf("  test_write_simple_strings...\n");

	uint8_t output[4096];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	ASSERT_TRUE(bvnr_write_string(w, "name", "Alice"),
				"write name must succeed");
	ASSERT_TRUE(bvnr_write_string(w, "city", "Berlin"),
				"write city must succeed");
	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_TRUE(bvnr_writer_bytes_written(w) > 0, "bytes_written > 0");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
				"round-trip must succeed");
	ASSERT_TRUE(strcmp(le.key, "city") == 0, "last key == city");
	ASSERT_TRUE(strcmp(le.value, "Berlin") == 0, "last value == Berlin");

	bvnr_writer_destroy(w);
}

static void test_write_typed_integers(void)
{
	printf("  test_write_typed_integers...\n");

	uint8_t output[4096];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	ASSERT_TRUE(bvnr_write_uint(w, "age",         8,  42),
				"write uint:8 age must succeed");
	ASSERT_TRUE(bvnr_write_sint(w, "temperature", 16, -15),
				"write sint:16 temperature must succeed");
	ASSERT_TRUE(bvnr_write_uint(w, "flags",       32, 0xDEADBEEFUL),
				"write uint:32 flags must succeed");
	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
				"round-trip must succeed");
	ASSERT_TRUE(le.vt.family == vt_uint, "last type is uint");
	ASSERT_EQ_UINT(le.vt.width, 32, "last width == 32");

	bvnr_writer_destroy(w);
}

static void test_write_float_values(void)
{
	printf("  test_write_float_values...\n");

	uint8_t output[4096];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	ASSERT_TRUE(bvnr_write_float(w, "pi",      64, 3.14159265358979),
				"write float:64 pi must succeed");
	ASSERT_TRUE(bvnr_write_float(w, "epsilon", 32, 1.19209e-7f),
				"write float:32 epsilon must succeed");
	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
				"round-trip must succeed");
	ASSERT_TRUE(le.vt.family == vt_float, "last type is float");

	bvnr_writer_destroy(w);
}

static void test_write_float_width_over_64_rejected(void)
{
	printf("  test_write_float_width_over_64_rejected...\n");

	uint8_t output[512];
	bvnr_sink_t sink;
	bvnr_writer_t *w;

	/* Valid widths 0, 16, 32, 64, 128, 256 must all succeed */
	static const uint32_t valid_widths[] = { 0, 16, 32, 64, 128, 256 };
	for (size_t i = 0; i < sizeof(valid_widths)/sizeof(valid_widths[0]); i++) {
		uint32_t wd = valid_widths[i];
		w = make_writer(output, sizeof(output), &sink);
		ASSERT_NOT_NULL(w, "make_writer must succeed for valid float width");
		if (!w) return;
		ASSERT_TRUE(bvnr_write_float(w, "x", wd, 3.14),
					"bvnr_write_float must succeed for valid width");
		ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none,
					  "no error for valid float width");
		bvnr_writer_destroy(w);

		w = make_writer(output, sizeof(output), &sink);
		ASSERT_NOT_NULL(w, "make_writer must succeed for valid float_fix width");
		if (!w) return;
		ASSERT_TRUE(bvnr_write_float_fix(w, "x", wd, 10, 1.5),
					"bvnr_write_float_fix must succeed for valid width");
		ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none,
					  "no error for valid float_fix width");
		bvnr_writer_destroy(w);

		w = make_writer(output, sizeof(output), &sink);
		ASSERT_NOT_NULL(w, "make_writer must succeed for valid float_dec width");
		if (!w) return;
		ASSERT_TRUE(bvnr_write_float_dec(w, "x", wd, 2.5),
					"bvnr_write_float_dec must succeed for valid width");
		ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none,
					  "no error for valid float_dec width");
		bvnr_writer_destroy(w);
	}

	/* Invalid width (not in {0,16,32,64,128,256}) must be rejected */
	w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;
	ASSERT_FALSE(bvnr_write_float(w, "x", 100, 3.14),
				 "bvnr_write_float with width=100 must fail");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_invalid_argument,
				  "error must be error_invalid_argument for width=100");
	bvnr_writer_destroy(w);

	w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer (fix) must succeed");
	if (!w) return;
	ASSERT_FALSE(bvnr_write_float_fix(w, "x", 100, 10, 1.5),
				 "bvnr_write_float_fix with width=100 must fail");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_invalid_argument,
				  "float_fix width=100 error must be error_invalid_argument");
	bvnr_writer_destroy(w);

	w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer (dec) must succeed");
	if (!w) return;
	ASSERT_FALSE(bvnr_write_float_dec(w, "x", 100, 2.5),
				 "bvnr_write_float_dec with width=100 must fail");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_invalid_argument,
				  "float_dec width=100 error must be error_invalid_argument");
	bvnr_writer_destroy(w);
}

static void test_write_bool_and_null(void)
{
	printf("  test_write_bool_and_null...\n");

	uint8_t output[4096];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	ASSERT_TRUE(bvnr_write_bool(w, "active",   true),  "write true must succeed");
	ASSERT_TRUE(bvnr_write_bool(w, "disabled", false), "write false must succeed");
	ASSERT_TRUE(bvnr_write_null(w, "optional"),        "write null must succeed");
	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	bvnr_writer_destroy(w);
}

static void test_write_with_units(void)
{
	printf("  test_write_with_units...\n");

	uint8_t output[4096];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	value_unit_t km = BVN_UNIT_SI(bu_meter, si_kilo);
	ASSERT_TRUE(bvnr_write_uint_unit(w, "distance", 32, 42, km),
				"write uint with km unit must succeed");

	value_unit_t ms = BVN_UNIT_COMPOUND2(
		bu_meter,  si_none, exp_linear,
		bu_second, si_none, exp_neg_linear);
	ASSERT_TRUE(bvnr_write_float_unit(w, "speed", 64, 9.81, ms),
				"write float with m/s unit must succeed");

	value_unit_t kg = BVN_UNIT_SI(bu_gram, si_kilo);
	ASSERT_TRUE(bvnr_write_float_unit(w, "mass", 32, 70.5, kg),
				"write float with kg unit must succeed");

	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	bvnr_writer_destroy(w);
}

static void test_write_struct(void)
{
	printf("  test_write_struct...\n");

	uint8_t output[4096];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	ASSERT_TRUE(bvnr_write_struct_start(w, "server"),       "struct begin");
	ASSERT_TRUE(bvnr_write_string      (w, "host", "127.0.0.1"), "write host");
	ASSERT_TRUE(bvnr_write_uint        (w, "port", 16, 8080),    "write port");
	ASSERT_TRUE(bvnr_write_bool        (w, "tls",  false),       "write tls");
	ASSERT_TRUE(bvnr_write_struct_end  (w),                 "struct end");

	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
				"round-trip must succeed");

	ASSERT_TRUE(strcmp(le.key, "tls") == 0, "last key == tls");

	bvnr_writer_destroy(w);
}

static void test_write_nested_struct(void)
{
	printf("  test_write_nested_struct...\n");

	uint8_t output[4096];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	ASSERT_TRUE(bvnr_write_struct_start(w, "outer"),    "outer struct begin");
	ASSERT_TRUE(bvnr_write_struct_start(w, "inner"),    "inner struct begin");
	ASSERT_TRUE(bvnr_write_uint        (w, "x", 32, 1), "write x");
	ASSERT_TRUE(bvnr_write_uint        (w, "y", 32, 2), "write y");
	ASSERT_TRUE(bvnr_write_struct_end  (w),             "inner struct end");
	ASSERT_TRUE(bvnr_write_struct_end  (w),             "outer struct end");

	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	bvnr_writer_destroy(w);
}

static void test_write_type_annotation_direct(void)
{
	printf("  test_write_type_annotation_direct...\n");

	uint8_t output[4096];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	const char *key = "count";
	bvnr_data_t d = {
		.type   = token_is_identifier,
		.data   = (const void *)key,
		.length = (uint32_t)strlen(key),
	};
	ASSERT_TRUE(bvnr_write_event(w, ev_assignment_start, &d),
				"assignment_start must succeed");

	value_type_spec_t vt = BVN_TYPE_UINT(32);
	ASSERT_TRUE(bvnr_write_type_annotation(w, vt, BVN_UNIT_NONE),
				"type_annotation must succeed");

	d = (bvnr_data_t){
		.type       = token_is_number,
		.value_type = vt,
		.data       = "99",
		.length     = 2,
	};
	ASSERT_TRUE(bvnr_write_event(w, ev_data, &d), "ev_data must succeed");

	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
				"round-trip must succeed");
	ASSERT_TRUE(strcmp(le.key,   "count") == 0, "key == count");
	ASSERT_TRUE(strcmp(le.value, "99")    == 0, "value == 99");
	ASSERT_TRUE(le.vt.family == vt_uint,        "family == vt_uint");
	ASSERT_EQ_UINT(le.vt.width, 32,             "width == 32");

	bvnr_writer_destroy(w);
}

/*
 * Regression: a symbol (bare plain value) whose text equals a reserved keyword
 * (null/true/false/on/off/nan/inf/ninf) is emitted bare and would be
 * reclassified by the reader into a null/bool/float value, silently losing the
 * symbol type. Symbols have no quoted form, so the writer must reject them.
 */
static void test_write_keyword_symbol_rejected(void)
{
	printf("  test_write_keyword_symbol_rejected...\n");

	const char *keywords[] = { "null", "true", "false", "on",
				   "off", "nan", "inf", "ninf" };
	const char *valid[]    = { "hello", "null_", "true1", "Inf", "onoff" };

	for (size_t i = 0; i < sizeof(keywords) / sizeof(keywords[0]); i++) {
		uint8_t output[256];
		bvnr_sink_t sink;
		bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
		ASSERT_NOT_NULL(w, "make_writer must succeed");
		if (!w) continue;
		ASSERT_FALSE(bvnr_write_plain(w, "k", keywords[i]),
					 "keyword-valued symbol must be rejected");
		bvnr_writer_destroy(w);
	}
	for (size_t i = 0; i < sizeof(valid) / sizeof(valid[0]); i++) {
		uint8_t output[256];
		bvnr_sink_t sink;
		bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
		ASSERT_NOT_NULL(w, "make_writer must succeed");
		if (!w) continue;
		ASSERT_TRUE(bvnr_write_plain(w, "k", valid[i]),
					"non-keyword symbol must be accepted");
		bvnr_writer_destroy(w);
	}
}

/*
 * Regression: utf8 is a parameterless family (spec: any parameter ->
 * error_illegal_value_type). The writer must reject a width on utf8, or it
 * would emit <utf8:N> that its own reader rejects. A bare <utf8> is fine.
 */
static void test_write_utf8_width_rejected(void)
{
	printf("  test_write_utf8_width_rejected...\n");

	uint8_t output[256];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	const char *key = "k";
	bvnr_data_t d = {
		.type   = token_is_identifier,
		.data   = (const void *)key,
		.length = 1u,
	};
	ASSERT_TRUE(bvnr_write_event(w, ev_assignment_start, &d),
				"assignment_start must succeed");

	value_type_spec_t utf8_w = { .family = vt_utf8, .width = 8u, .base = 0u };
	ASSERT_FALSE(bvnr_write_type_annotation(w, utf8_w, BVN_UNIT_NONE),
				 "utf8 with width must be rejected");
	bvnr_writer_destroy(w);

	/* A bare <utf8> annotation must still be accepted. */
	w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;
	ASSERT_TRUE(bvnr_write_event(w, ev_assignment_start, &d),
				"assignment_start must succeed");
	ASSERT_TRUE(bvnr_write_type_annotation(w, BVN_TYPE_UTF8, BVN_UNIT_NONE),
				"bare utf8 annotation must be accepted");
	bvnr_writer_destroy(w);
}

static void test_write_buffer_exhaustion(void)
{
	printf("  test_write_buffer_exhaustion...\n");

	uint8_t output[16];
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, output, sizeof(output));

	bvnr_writer_t *w = bvnr_writer_create();
	ASSERT_NOT_NULL(w, "bvnr_writer_create must succeed");
	if (!w) return;

	ASSERT_TRUE(bvnr_open_write_sink(w, &sink, true, NULL),
				"open_write_sink must succeed");

	bvnr_data_t hdr = {0};
	ASSERT_TRUE(bvnr_write_event(w, ev_stream_start, &hdr),
				"stream_begin must succeed");

	bool result = bvnr_write_string(w,
		"this_is_a_very_long_key_that_will_not_fit", "value");

	if (result) {
		ASSERT_TRUE(!bvnr_write_finish(w),
					"finish must fail on exhaustion");
		ASSERT_EQ_INT(bvnr_writer_get_error(w), error_sink_buffer_exhausted,
					  "error must be buffer_exhausted");
	} else {
		ASSERT_EQ_INT(bvnr_writer_get_error(w), error_sink_buffer_exhausted,
					  "error must be buffer_exhausted");
	}

	bvnr_writer_destroy(w);
}

static void test_write_multiple_assignments(void)
{
	printf("  test_write_multiple_assignments...\n");

	uint8_t output[8192];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	for (int i = 0; i < 20; i++) {
		char key[16];
		snprintf(key, sizeof(key), "item%d", i);
		ASSERT_TRUE(bvnr_write_string(w, key, "value"),
					"write assignment must succeed");
	}

	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_TRUE(bvnr_writer_bytes_written(w) > 0, "bytes_written > 0");

	bvnr_writer_destroy(w);
}

static void test_write_sint_negative(void)
{
	printf("  test_write_sint_negative...\n");

	uint8_t output[4096];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	ASSERT_TRUE(bvnr_write_sint(w, "delta",  16, -1),
				"write sint:16 -1 must succeed");
	ASSERT_TRUE(bvnr_write_sint(w, "offset", 32, INT32_MIN),
				"write sint:32 INT32_MIN must succeed");
	ASSERT_TRUE(bvnr_write_sint(w, "big",    64, INT64_MIN),
				"write sint:64 INT64_MIN must succeed");

	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	bvnr_writer_destroy(w);
}

static void test_write_bvnf_base10(void)
{
	printf("  test_write_bvnf_base10...\n");

	uint8_t output[32768];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	bvn_float_t *f = bvn_float_alloc(512u);
	ASSERT_NOT_NULL(f, "bvn_float_alloc(512) must succeed");
	if (!f) { bvnr_writer_destroy(w); return; }

	ASSERT_TRUE(bvn_float_from_double(f, 3.14159265358979323846),
				"bvn_float_from_double must succeed");

	ASSERT_TRUE(bvnr_write_bvnf_base(w, "pi_512", f, 512u, 10u),
				"bvnr_write_bvnf_base base10 width512 must succeed");

	ASSERT_TRUE(bvnr_write_bvnf_base(w, "pi_1024", f, 1024u, 10u),
				"bvnr_write_bvnf_base base10 width1024 must succeed");

	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	uint64_t bw = bvnr_writer_bytes_written(w);
	ASSERT_TRUE(bw > 0, "bytes_written > 0");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, bw, &le),
				"bvnf base10 pi document must round-trip");
	ASSERT_TRUE(strstr(le.value, "3.14") != NULL,
				"pi_1024 value must begin with 3.14 (value preserved through write/read)");
	ASSERT_EQ_INT(le.value_tok, token_is_number,
				  "base10 bvnf token must be token_is_number");

	bvn_float_free(f);
	bvnr_writer_destroy(w);
}

static void test_write_bvnf_base16(void)
{
	printf("  test_write_bvnf_base16...\n");

	uint8_t output[32768];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	bvn_float_t *f = bvn_float_alloc(256u);
	ASSERT_NOT_NULL(f, "bvn_float_alloc(256) must succeed");
	if (!f) { bvnr_writer_destroy(w); return; }

	ASSERT_TRUE(bvn_float_from_double(f, 1.5),
				"bvn_float_from_double(1.5) must succeed");

	ASSERT_TRUE(bvnr_write_bvnf_base(w, "hex_256", f, 256u, 16u),
				"bvnr_write_bvnf_base base16 width256 must succeed");

	bvn_float_set_nan(f);
	ASSERT_TRUE(bvnr_write_bvnf_base(w, "nan_256", f, 256u, 16u),
				"bvnr_write_bvnf_base base16 NaN must succeed");

	bvn_float_set_inf(f, false);
	ASSERT_TRUE(bvnr_write_bvnf_base(w, "pos_inf", f, 256u, 16u),
				"bvnr_write_bvnf_base base16 +Inf must succeed");

	bvn_float_set_inf(f, true);
	ASSERT_TRUE(bvnr_write_bvnf_base(w, "neg_inf", f, 256u, 16u),
				"bvnr_write_bvnf_base base16 -Inf must succeed");

	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	uint64_t n = bvnr_writer_bytes_written(w);
	ASSERT_TRUE(n > 0u, "bytes_written > 0");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, n, &le),
				"base16 bvnf document with NaN and Inf must round-trip");

	bvn_float_free(f);
	bvnr_writer_destroy(w);

	{
		uint8_t out2[1024];
		bvnr_sink_t sink2;
		bvnr_writer_t *w2 = make_writer(out2, sizeof(out2), &sink2);
		ASSERT_NOT_NULL(w2, "value-check writer must succeed");
		if (!w2) return;

		bvn_float_t *f2 = bvn_float_alloc(256u);
		ASSERT_NOT_NULL(f2, "value-check bvn_float_alloc must succeed");
		if (!f2) { bvnr_writer_destroy(w2); return; }

		ASSERT_TRUE(bvn_float_from_double(f2, 1.5),
					"bvn_float_from_double(1.5) value-check");
		ASSERT_TRUE(bvnr_write_bvnf_base(w2, "v", f2, 256u, 16u),
					"write 1.5 base16 value-check");
		ASSERT_TRUE(bvnr_write_finish(w2), "finish value-check writer");

		last_event_t le2 = {0};
		ASSERT_TRUE(roundtrip(out2, bvnr_writer_bytes_written(w2), &le2),
					"1.5 base16 bvnf must round-trip");
		ASSERT_TRUE(strstr(le2.value, "1.8") != NULL,
					"base16 repr of 1.5 must contain '1.8' (value preserved)");
		ASSERT_EQ_INT(le2.value_tok, token_is_string,
					  "base16 non-special bvnf token must be token_is_string");

		bvn_float_free(f2);
		bvnr_writer_destroy(w2);
	}
}

static void test_write_bvni_decimal(void)
{
	printf("  test_write_bvni_decimal...\n");

	uint8_t output[32768];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	bvn_int_t *n = bvn_int_alloc();
	ASSERT_NOT_NULL(n, "bvn_int_alloc must succeed");
	if (!n) { bvnr_writer_destroy(w); return; }

	ASSERT_TRUE(bvn_int_from_uint64(n, 18446744073709551615ULL),
				"bvn_int_from_uint64 UINT64_MAX must succeed");
	ASSERT_TRUE(bvnr_write_bvni(w, "u64max", n, 64u, 10u),
				"bvnr_write_bvni uint:64 base10 must succeed");

	ASSERT_TRUE(bvn_int_from_str(n, "340282366920938463463374607431768211455", 10),
				"bvn_int_from_str 128-bit max must succeed");
	ASSERT_TRUE(bvnr_write_bvni(w, "u128max", n, 128u, 10u),
				"bvnr_write_bvni uint:128 base10 must succeed");

	ASSERT_TRUE(bvn_int_from_int64(n, -1LL),
				"bvn_int_from_int64(-1) must succeed");
	ASSERT_TRUE(bvnr_write_bvni(w, "neg_one", n, 64u, 10u),
				"bvnr_write_bvni sint:64 -1 must succeed");

	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
				"bvni decimal roundtrip must succeed");
	ASSERT_TRUE(le.vt.family == vt_sint,
				"last entry (neg_one) must be sint");
	ASSERT_EQ_UINT(le.vt.width, 64,
				   "neg_one width must be 64");
	ASSERT_TRUE(strcmp(le.value, "-1") == 0,
				"neg_one value must round-trip as \"-1\"");
	ASSERT_EQ_INT(le.value_tok, token_is_number,
				  "base10 bvni token must be token_is_number");

	bvn_int_free(n);
	bvnr_writer_destroy(w);
}

/* Width 0 is the default width, 64, and is held to it: 2^64 under a bare <uint>
 * is a document the reader refuses, and bvnr_write_bvni used to write it. One
 * writer per case -- the first refusal is the writer's error from then on. */
static void test_write_bvni_width0_is_64(void)
{
	printf("  test_write_bvni_width0_is_64...\n");

	static const struct { const char *digits; bool ok; } C[] = {
		{ "18446744073709551615",  true  },   /* UINT64_MAX     */
		{ "18446744073709551616",  false },   /* 2^64           */
		{ "-9223372036854775808",  true  },   /* INT64_MIN      */
		{ "-9223372036854775809",  false },   /* INT64_MIN - 1  */
		{ "-18446744073709551615", false },
	};
	for (size_t i = 0; i < sizeof C / sizeof C[0]; i++) {
		uint8_t output[1024];
		bvnr_sink_t sink;
		bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
		bvn_int_t *n = bvn_int_alloc();
		ASSERT_NOT_NULL(w, "make_writer must succeed");
		ASSERT_NOT_NULL(n, "bvn_int_alloc must succeed");
		if (!w || !n) { bvn_int_free(n); bvnr_writer_destroy(w); return; }
		ASSERT_TRUE(bvn_int_from_str(n, C[i].digits, 10), "from_str");
		bool ok = bvnr_write_bvni(w, "v", n, 0u, 10u);
		ASSERT_TRUE(ok == C[i].ok, C[i].digits);
		if (C[i].ok) {
			last_event_t le = {0};
			ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
			ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
						"what width 0 wrote reads back");
		} else {
			ASSERT_EQ_INT(bvnr_writer_get_error(w), error_value_out_of_range,
						  "refused as error_value_out_of_range");
		}
		bvn_int_free(n);
		bvnr_writer_destroy(w);
	}
}

/* ── fourth review round ─────────────────────────────────────────────────── */

/* A writer on `out` with the given unit flags, and a 1.2 directive if asked. */
static bvnr_writer_t *r4_open(uint8_t *out, size_t cap, bvn_unit_flags_t uf,
			      bool version)
{
	memset(out, 0, cap);
	bvnr_writer_t *w = bvnr_writer_create();
	if (!w) return NULL;
	bvnr_write_flags_t wf;
	memset(&wf, 0, sizeof wf);
	wf.unit_flags   = uf;
	wf.emit_version = version;
	if (!bvnr_open_write_mem(w, out, cap - 1u, false, &wf)) {
		bvnr_writer_destroy(w);
		return NULL;
	}
	return w;
}
static bool r4_key(bvnr_writer_t *w, const char *k)
{
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	d.type = token_is_identifier; d.data = k; d.length = (uint32_t)strlen(k);
	return bvnr_write_event(w, ev_assignment_start, &d);
}
/* `<family:width[,unit]>`. `start_unit` goes on the start event; `param` adds a
 * unit-parameter event carrying `text` (NULL: the event has its type only). */
static bool r4_ann(bvnr_writer_t *w, value_type_spec_t vt, value_unit_t start_unit,
		   bool param, value_unit_t param_unit, const char *text)
{
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	d.type = token_is_type; d.value_type = vt; d.value_unit = start_unit;
	if (!bvnr_write_event(w, ev_type_annotation_start, &d)) return false;
	d.value_unit = BVN_UNIT_NONE;
	if (!bvnr_write_event(w, ev_type_annotation_type_family, &d)) return false;
	if (vt.width) {
		d.type = token_is_type_width;
		if (!bvnr_write_event(w, ev_type_annotation_type_family_parameter, &d))
			return false;
	}
	if (param) {
		d.type = token_is_unit; d.value_unit = param_unit;
		d.data = text; d.length = text ? (uint32_t)strlen(text) : 0u;
		if (!bvnr_write_event(w, ev_type_annotation_type_family_parameter, &d))
			return false;
	}
	memset(&d, 0, sizeof d);
	d.type = token_is_type; d.value_type = vt;
	return bvnr_write_event(w, ev_type_annotation_end, &d);
}
static bool r4_val(bvnr_writer_t *w, token_type_t tt, value_type_spec_t vt,
		   value_unit_t vu, const char *text)
{
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	d.type = tt; d.value_type = vt; d.value_unit = vu;
	d.data = text; d.length = text ? (uint32_t)strlen(text) : 0u;
	return bvnr_write_event(w, ev_data, &d);
}
static bool r4_ev(bvnr_writer_t *w, bvnr_event_t ev)
{
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	return bvnr_write_event(w, ev, &d);
}
/* Finish, compare the bytes with `want` (NULL: do not compare) and read them
 * back: whatever the writer reports as written, the reader has to take. */
static void r4_expect(bvnr_writer_t *w, const uint8_t *out, const char *want,
		      const char *what)
{
	last_event_t le = {0};
	ASSERT_TRUE(bvnr_write_finish(w), what);
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, what);
	if (want)
		ASSERT_TRUE(strcmp((const char *)out, want) == 0, what);
	ASSERT_TRUE(roundtrip(out, bvnr_writer_bytes_written(w), &le), what);
	bvnr_writer_destroy(w);
}

/*
 * A value answers to the annotation it stands under, not to the value_type its
 * own event happens to carry.
 *
 * Every check ran against data->value_type. An event typed uint:64 under a
 * `<uint:8>` annotation was range-checked as uint:64; one left zero-initialised
 * is "plain" and skipped the checks altogether. All eight of these were written
 * and reported as success, and the reader refuses every one.
 */
static void test_value_is_held_to_its_annotation(void)
{
	printf("  test_value_is_held_to_its_annotation...\n");
	const value_type_spec_t u8 = BVN_TYPE_UINT(8), u64 = BVN_TYPE_UINT(64);
	static const struct {
		int ann;                  /* 0 uint:8, 1 utf8, 2 bool */
		token_type_t tt; int dvt; /* 0 plain, 1 uint:64 */
		const char *text; error_code_t want;
	} C[] = {
		{ 0, token_is_number,    1, "300",   error_value_out_of_range  },
		{ 0, token_is_number,    0, "300",   error_value_out_of_range  },
		{ 0, token_is_number,    0, "1.5",   error_type_value_mismatch },
		{ 0, token_is_string,    0, "hello", error_digit_not_in_base   },
		{ 0, token_is_symbol,    0, "hello", error_type_value_mismatch },
		{ 0, token_is_bool,      0, "true",  error_type_value_mismatch },
		{ 0, token_is_reference, 0, ".b",    error_type_value_mismatch },
		{ 1, token_is_number,    0, "5",     error_type_value_mismatch },
		{ 2, token_is_string,    0, "x",     error_type_value_mismatch },
	};
	for (size_t i = 0; i < sizeof C / sizeof C[0]; i++) {
		uint8_t out[256];
		bvnr_writer_t *w = r4_open(out, sizeof out, 0, false);
		ASSERT_NOT_NULL(w, "writer");
		if (!w) return;
		value_type_spec_t avt = C[i].ann == 0 ? u8
				      : C[i].ann == 1 ? BVN_TYPE_UTF8 : BVN_TYPE_BOOL;
		ASSERT_TRUE(r4_key(w, "a") &&
			    r4_ann(w, avt, BVN_UNIT_NONE, false, BVN_UNIT_NONE, NULL),
			    "the annotation writes");
		ASSERT_FALSE(r4_val(w, C[i].tt, C[i].dvt ? u64 : BVN_TYPE_PLAIN,
				    BVN_UNIT_NONE, C[i].text), C[i].text);
		ASSERT_EQ_INT(bvnr_writer_get_error(w), C[i].want, C[i].text);
		bvnr_writer_destroy(w);
	}
	/* What the annotation admits still writes, whatever the event's own type
	 * says: plain, the annotation's own, or a wider carrier (the Python
	 * binding types every array element at 64 bits or more). */
	{
		uint8_t out[256];
		bvnr_writer_t *w = r4_open(out, sizeof out, 0, false);
		ASSERT_NOT_NULL(w, "writer");
		if (!w) return;
		ASSERT_TRUE(r4_key(w, "a") &&
			    r4_ann(w, u8, BVN_UNIT_NONE, false, BVN_UNIT_NONE, NULL) &&
			    r4_val(w, token_is_number, BVN_TYPE_PLAIN, BVN_UNIT_NONE, "255") &&
			    r4_key(w, "b") &&
			    r4_ann(w, u8, BVN_UNIT_NONE, false, BVN_UNIT_NONE, NULL) &&
			    r4_val(w, token_is_number, u64, BVN_UNIT_NONE, "255") &&
			    r4_key(w, "c") &&
			    r4_ann(w, u8, BVN_UNIT_NONE, false, BVN_UNIT_NONE, NULL) &&
			    r4_ev(w, ev_array_row_start) &&
			    r4_val(w, token_is_array_number, u64, BVN_UNIT_NONE, "1") &&
			    r4_val(w, token_is_null_value, BVN_TYPE_PLAIN, BVN_UNIT_NONE, NULL) &&
			    r4_ev(w, ev_array_row_end),
			    "values the annotation admits");
		r4_expect(w, out, ".a=<uint:8>255;.b=<uint:8>255;.c=<uint:8>[1,null];",
			  "in-range values under <uint:8>");
	}
	/* ...and inside an annotated array each element is held to it too. */
	{
		uint8_t out[256];
		bvnr_writer_t *w = r4_open(out, sizeof out, 0, false);
		ASSERT_NOT_NULL(w, "writer");
		if (!w) return;
		ASSERT_TRUE(r4_key(w, "c") &&
			    r4_ann(w, u8, BVN_UNIT_NONE, false, BVN_UNIT_NONE, NULL) &&
			    r4_ev(w, ev_array_row_start) &&
			    r4_val(w, token_is_array_number, u64, BVN_UNIT_NONE, "1"),
			    "first element");
		ASSERT_FALSE(r4_val(w, token_is_array_number, u64, BVN_UNIT_NONE, "256"),
			     "256 is not a uint:8, in the second element either");
		ASSERT_EQ_INT(bvnr_writer_get_error(w), error_value_out_of_range, "256");
		bvnr_writer_destroy(w);
	}
}

/*
 * An integer with no declared width is 64 bits wide -- to the reader, and so to
 * the writer. The range tests were skipped when the annotation named no width
 * (and a bare number had none to name), so 2^128 went out under `<uint>`,
 * under `<sint>`, quoted in base 16, and bare.
 */
static void test_unstated_integer_width_is_64(void)
{
	printf("  test_unstated_integer_width_is_64...\n");
	const char *big = "340282366920938463463374607431768211456";
	for (int k = 0; k < 4; k++) {
		uint8_t out[256];
		bvnr_writer_t *w = r4_open(out, sizeof out, 0, false);
		ASSERT_NOT_NULL(w, "writer");
		if (!w) return;
		bool ok = r4_key(w, "a");
		if (k == 0) {
			ok = ok && r4_ann(w, BVN_TYPE_UINT(0), BVN_UNIT_NONE, false,
					  BVN_UNIT_NONE, NULL) &&
			     r4_val(w, token_is_number, BVN_TYPE_UINT(0), BVN_UNIT_NONE, big);
		} else if (k == 1) {
			ok = ok && r4_ann(w, BVN_TYPE_SINT(0), BVN_UNIT_NONE, false,
					  BVN_UNIT_NONE, NULL) &&
			     r4_val(w, token_is_number, BVN_TYPE_SINT(0), BVN_UNIT_NONE,
				    "-340282366920938463463374607431768211456");
		} else if (k == 2) {
			/* bare, the event leaving its type plain */
			ok = ok && r4_val(w, token_is_number, BVN_TYPE_PLAIN,
					  BVN_UNIT_NONE, big);
		} else {
			/* bare, the event CLAIMING 128 bits nothing in the document says */
			ok = ok && r4_val(w, token_is_number, BVN_TYPE_UINT(128),
					  BVN_UNIT_NONE, big);
		}
		ASSERT_FALSE(ok, "2^128 with no width declared is refused");
		ASSERT_EQ_INT(bvnr_writer_get_error(w), error_value_out_of_range,
			      "...as out of range");
		bvnr_writer_destroy(w);
	}
	uint8_t out[256];
	bvnr_writer_t *w = r4_open(out, sizeof out, 0, false);
	ASSERT_NOT_NULL(w, "writer");
	if (!w) return;
	ASSERT_TRUE(r4_key(w, "a") &&
		    r4_val(w, token_is_number, BVN_TYPE_PLAIN, BVN_UNIT_NONE,
			   "18446744073709551615") &&
		    r4_key(w, "b") &&
		    r4_val(w, token_is_number, BVN_TYPE_PLAIN, BVN_UNIT_NONE,
			   "-9223372036854775808") &&
		    r4_key(w, "c") &&
		    r4_val(w, token_is_number, BVN_TYPE_PLAIN, BVN_UNIT_NONE, "1e400"),
		    "the 64-bit extremes and a float still write bare");
	r4_expect(w, out, NULL, "bare extremes read back");
}

/*
 * BVN_UNIT_REDUCE moves the VALUE with the unit whatever the event's type says.
 *
 * The serialiser took "is this a number" from data->value_type, so an event that
 * left it plain -- the natural way to write a bare `5 k~m` -- had its unit
 * reduced and its digits left alone: `5 m`. Five kilometres, written as five
 * metres, reported as success.
 */
static void test_reduce_rescales_whatever_the_event_type(void)
{
	printf("  test_reduce_rescales_whatever_the_event_type...\n");
	const value_unit_t km = BVN_UNIT_SI(bu_meter, si_kilo);
	uint8_t out[256];
	bvnr_writer_t *w = r4_open(out, sizeof out, BVN_UNIT_REDUCE, false);
	ASSERT_NOT_NULL(w, "writer");
	if (!w) return;
	ASSERT_TRUE(r4_key(w, "a") &&
		    r4_val(w, token_is_number, BVN_TYPE_PLAIN, km, "5") &&
		    r4_key(w, "b") &&
		    r4_ann(w, BVN_TYPE_FLOAT(64), km, true, km, "k~m") &&
		    r4_val(w, token_is_number, BVN_TYPE_PLAIN, km, "5") &&
		    r4_key(w, "c") &&
		    r4_ann(w, BVN_TYPE_FLOAT(64), km, true, km, "k~m") &&
		    r4_val(w, token_is_number, BVN_TYPE_PLAIN, BVN_UNIT_NONE, "2.5"),
		    "plain-typed values with a prefixed unit");
	r4_expect(w, out, ".a=5000 m;.b=<float:64,m>5000;.c=<float:64,m>2500;",
		  "the digits moved with the unit");
}

/*
 * The rescale keeps a float a float: its sign of zero, and its range.
 */
static void test_reduce_float_edges(void)
{
	printf("  test_reduce_float_edges...\n");
	const value_unit_t km = BVN_UNIT_SI(bu_meter, si_kilo);
	uint8_t out[512];
	bvnr_writer_t *w = r4_open(out, sizeof out, BVN_UNIT_REDUCE, false);
	ASSERT_NOT_NULL(w, "writer");
	if (!w) return;
	/* Zero through a rational has no sign: -0.0 k~m came out as "0 m". */
	ASSERT_TRUE(bvnr_write_float_unit(w, "z", 64, -0.0, km), "-0.0 writes");
	r4_expect(w, out, ".z=<float:64,m>-0;", "negative zero keeps its sign");

	/* 3e38 fits a binary32; the 3e41 it becomes in metres is an infinity to
	 * anything decoding at the declared width. Refused, like an integer the
	 * rescale pushes past its width. */
	w = r4_open(out, sizeof out, BVN_UNIT_REDUCE, false);
	ASSERT_NOT_NULL(w, "writer");
	if (!w) return;
	ASSERT_FALSE(bvnr_write_float_unit(w, "e", 32, 3e38, km),
		     "a rescale past FLT_MAX is refused");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_value_out_of_range,
		      "...as out of range");
	bvnr_writer_destroy(w);

	/* The same value in a float:64 has room. */
	w = r4_open(out, sizeof out, BVN_UNIT_REDUCE, false);
	ASSERT_NOT_NULL(w, "writer");
	if (!w) return;
	ASSERT_TRUE(bvnr_write_float_unit(w, "e", 64, 3e38, km), "float:64 has room");
	r4_expect(w, out, NULL, "3e41 under float:64");
}

/*
 * A prefix is a prefix whatever it is attached to. BVN_UNIT_REDUCE asks for the
 * conversion `k~pc -> pc`, and that went through the parsec's SI factor -- which
 * is irrational, so the write failed as "value_out_of_range". The same for a
 * milliarcsecond, and for a prefixed °C inside a compound, where an affine
 * scale has no SI factor at all.
 */
static void test_reduce_prefix_on_any_base(void)
{
	printf("  test_reduce_prefix_on_any_base...\n");
	static const struct { const char *unit; double v; const char *want; } C[] = {
		{ "k~pc",     2.5,   ".v=<float:64,pc>2500;"      },
		{ "m~arcsec", 150.0, ".v=<float:64,arcsec>0.15;"  },
		{ "k~rev",    2.0,   ".v=<float:64,rev>2000;"     },
		{ "m~°C/s",   7.5,   ".v=<float:64,°C/s>0.0075;"  },
	};
	for (size_t i = 0; i < sizeof C / sizeof C[0]; i++) {
		uint8_t out[256];
		bool ok = false;
		value_unit_t u = bvn_parse_unit((const uint8_t *)C[i].unit, &ok);
		ASSERT_TRUE(ok, C[i].unit);
		bvnr_writer_t *w = r4_open(out, sizeof out, BVN_UNIT_REDUCE, false);
		ASSERT_NOT_NULL(w, "writer");
		if (!w) return;
		ASSERT_TRUE(bvnr_write_float_unit(w, "v", 64, C[i].v, u), C[i].unit);
		r4_expect(w, out, C[i].want, C[i].unit);
	}
}

/*
 * The format has no inline unit after nan/inf/ninf (spec section 6.4) or after
 * a datetime. Both were written -- `<float:64>nan k~m`, `<datetime:64>5 k~m` --
 * and neither reads back. With the unit on the annotation they are fine.
 */
static void test_no_inline_unit_on_special_or_datetime(void)
{
	printf("  test_no_inline_unit_on_special_or_datetime...\n");
	const value_unit_t km = BVN_UNIT_SI(bu_meter, si_kilo);
	const value_type_spec_t f64 = BVN_TYPE_FLOAT(64);
	const value_type_spec_t dt  = { .family = vt_datetime, .width = 64 };
	static const char *const sp[] = { "nan", "inf", "ninf" };
	for (size_t i = 0; i < 3; i++) {
		uint8_t out[128];
		bvnr_writer_t *w = r4_open(out, sizeof out, 0, false);
		ASSERT_NOT_NULL(w, "writer");
		if (!w) return;
		ASSERT_TRUE(r4_key(w, "a") &&
			    r4_ann(w, f64, BVN_UNIT_NONE, false, BVN_UNIT_NONE, NULL),
			    "annotation");
		ASSERT_FALSE(r4_val(w, token_is_number, f64, km, sp[i]), sp[i]);
		ASSERT_EQ_INT(bvnr_writer_get_error(w), error_unit_illegal, sp[i]);
		bvnr_writer_destroy(w);
	}
	uint8_t out[128];
	bvnr_writer_t *w = r4_open(out, sizeof out, 0, false);
	ASSERT_NOT_NULL(w, "writer");
	if (!w) return;
	ASSERT_FALSE(r4_key(w, "a") &&
		     r4_val(w, token_is_number, BVN_TYPE_PLAIN, km, "nan"),
		     "bare nan with an inline unit");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_unit_illegal, "bare nan");
	bvnr_writer_destroy(w);

	w = r4_open(out, sizeof out, 0, true);
	ASSERT_NOT_NULL(w, "writer");
	if (!w) return;
	ASSERT_TRUE(r4_key(w, "a") &&
		    r4_ann(w, dt, BVN_UNIT_NONE, false, BVN_UNIT_NONE, NULL),
		    "datetime annotation");
	ASSERT_FALSE(r4_val(w, token_is_number, dt, km, "5"),
		     "a datetime takes no inline unit");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_unit_illegal, "datetime");
	bvnr_writer_destroy(w);

	w = r4_open(out, sizeof out, 0, false);
	ASSERT_NOT_NULL(w, "writer");
	if (!w) return;
	volatile double zero_d = 0.0;        /* a NaN without <math.h> */
	ASSERT_TRUE(bvnr_write_float_unit(w, "a", 64, 0.0 / zero_d, km),
		    "nan with the unit on the annotation");
	r4_expect(w, out, ".a=<float:64,k~m>nan;", "nan under <float:64,k~m>");
}

/*
 * The writer's unit policy checks the unit the value is WRITTEN under.
 *
 * It read val.parsed_unit -- whatever the last annotation event carried -- and
 * decided "is this a number" from the event's own type. Three ways wrong.
 */
static void test_writer_policy_checks_the_written_unit(void)
{
	printf("  test_writer_policy_checks_the_written_unit...\n");
	const value_unit_t m = BVN_UNIT_NO_PREFIX(bu_meter);
	const value_unit_t s = BVN_UNIT_NO_PREFIX(bu_second);
	const value_type_spec_t f64 = BVN_TYPE_FLOAT(64);
	uint8_t out[256];
	bvnr_unit_policy_t req;
	memset(&req, 0, sizeof req);
	req.require_unit = true;

	/* (a) A bare number whose event left its type plain was not looked at. */
	bvnr_writer_t *w = r4_open(out, sizeof out, 0, false);
	ASSERT_NOT_NULL(w, "writer");
	if (!w) return;
	ASSERT_TRUE(bvnr_writer_set_unit_policy(w, &req), "policy");
	ASSERT_FALSE(r4_key(w, "a") &&
		     r4_val(w, token_is_number, BVN_TYPE_PLAIN, BVN_UNIT_NONE, "5"),
		     "require_unit refuses a bare number, however it is typed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_unit_mismatch, "bare 5");
	bvnr_writer_destroy(w);

	/* (b) An element annotation overwrote the unit for the rest of the array:
	 * the 2.0 here is in METRES, and passed a policy asking for seconds. */
	static const char *const dims_s[] = { "s" };
	bvnr_unit_policy_t dim;
	memset(&dim, 0, sizeof dim);
	dim.require_dimension_of     = dims_s;
	dim.num_require_dimension_of = 1;
	w = r4_open(out, sizeof out, 0, false);
	ASSERT_NOT_NULL(w, "writer");
	if (!w) return;
	ASSERT_TRUE(bvnr_writer_set_unit_policy(w, &dim), "policy");
	/* the outer annotation is metres, so the array is refused at the first
	 * element NOT under its own seconds annotation */
	ASSERT_TRUE(r4_key(w, "a") && r4_ann(w, f64, m, true, m, "m") &&
		    r4_ev(w, ev_array_row_start) &&
		    r4_ann(w, f64, s, true, s, "s") &&
		    r4_val(w, token_is_array_number, f64, BVN_UNIT_NONE, "1.0"),
		    "the element in seconds passes");
	ASSERT_FALSE(r4_val(w, token_is_array_number, f64, BVN_UNIT_NONE, "2.0"),
		     "the next element is in metres again, and is refused");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_unit_mismatch, "metres");
	bvnr_writer_destroy(w);

	/* (c) A unit named on the annotation's start event alone is the value's
	 * unit (bovnar.h says so, and it is what gets written); the events after
	 * it carry none, and the value was refused as bare. */
	for (int with_param = 0; with_param < 2; with_param++) {
		w = r4_open(out, sizeof out, 0, false);
		ASSERT_NOT_NULL(w, "writer");
		if (!w) return;
		ASSERT_TRUE(bvnr_writer_set_unit_policy(w, &req), "policy");
		ASSERT_TRUE(r4_key(w, "a") &&
			    r4_ann(w, f64, m, with_param != 0, BVN_UNIT_NONE, NULL) &&
			    r4_val(w, token_is_number, f64, BVN_UNIT_NONE, "1.5"),
			    "a unit given on the start event satisfies require_unit");
		r4_expect(w, out, ".a=<float:64,m>1.5;", "start-event unit");
	}
}

/*
 * bvnr_write_float_dec / _fix do not narrow through a binary32. A decimal32 has
 * seven decimal digits and a Q16.16 thirty-two bits; a float has neither.
 */
static void test_dec_and_fix_are_not_narrowed_through_float(void)
{
	printf("  test_dec_and_fix_are_not_narrowed_through_float...\n");
	uint8_t out[512];
	bvnr_writer_t *w = r4_open(out, sizeof out, 0, false);
	ASSERT_NOT_NULL(w, "writer");
	if (!w) return;
	/* 8589973000 is a seven-digit decimal no binary32 holds: it used to be
	 * written 8.589973504e+9, which decimal32 rounds UP to 8.589974e9. */
	ASSERT_TRUE(bvnr_write_float_dec(w, "d", 32, 8.589973e9), "decimal32");
	/* Q16.16 resolves 1.5e-5; through a float this was 7.8e-5 off. */
	ASSERT_TRUE(bvnr_write_float_fix(w, "f", 32, 16, 12345.67), "Q16.16");
	/* INT32_MAX in a q0 field rounded to 2^31 as a float and was refused. */
	ASSERT_TRUE(bvnr_write_float_fix(w, "g", 32, 0, 2147483647.0), "Q32.0 max");
	ASSERT_TRUE(bvnr_write_finish(w), "finish");
	const char *o = (const char *)out;
	ASSERT_TRUE(strstr(o, ".d=<float_dec:32>8.589973e+9;") != NULL,
		    "the seven digits written are the seven given");
	ASSERT_TRUE(strstr(o, ".f=<float_fix:32,q16>1.234567e+4;") != NULL,
		    "12345.67 keeps its digits");
	ASSERT_TRUE(strstr(o, ".g=<float_fix:32>2.147483647e+9;") != NULL,
		    "2147483647 is written, and exactly");
	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(out, bvnr_writer_bytes_written(w), &le), "reads back");
	bvnr_writer_destroy(w);
}

/*
 * Two places where the writer's text checks were laxer than the reader's.
 */
static void test_reference_and_quoted_keyword_digits(void)
{
	printf("  test_reference_and_quoted_keyword_digits...\n");
	/* After an index a reference continues with '.', '[' or ends. */
	static const struct { const char *ref; bool ok; } R[] = {
		{ ".a[0]",    true  }, { ".a[0].b", true }, { ".a[0][1]", true },
		{ ".a[0]b",   false }, { ".a[0]1",  false }, { ".a[0]_", false },
	};
	for (size_t i = 0; i < sizeof R / sizeof R[0]; i++) {
		uint8_t out[128];
		bvnr_writer_t *w = r4_open(out, sizeof out, 0, true);
		ASSERT_NOT_NULL(w, "writer");
		if (!w) return;
		bool ok = r4_key(w, "r") &&
			  r4_val(w, token_is_reference, BVN_TYPE_PLAIN, BVN_UNIT_NONE,
				 R[i].ref);
		ASSERT_TRUE(ok == R[i].ok, R[i].ref);
		if (!R[i].ok)
			ASSERT_EQ_INT(bvnr_writer_get_error(w),
				      error_type_value_mismatch, R[i].ref);
		bvnr_writer_destroy(w);
	}
	/* `<uint:8,_36> "nan"` is the number 30191, not a NaN: out of range. */
	const value_type_spec_t u8b36  = BVN_TYPE_UINT_BASE(8, 36);
	const value_type_spec_t u16b36 = BVN_TYPE_UINT_BASE(16, 36);
	for (int wide = 0; wide < 2; wide++) {
		uint8_t out[128];
		value_type_spec_t vt = wide ? u16b36 : u8b36;
		bvnr_writer_t *w = r4_open(out, sizeof out, 0, false);
		ASSERT_NOT_NULL(w, "writer");
		if (!w) return;
		bvnr_data_t d;
		memset(&d, 0, sizeof d);
		d.type = token_is_type; d.value_type = vt;
		bool ok = r4_key(w, "a") &&
			  bvnr_write_event(w, ev_type_annotation_start, &d) &&
			  bvnr_write_event(w, ev_type_annotation_type_family, &d);
		d.type = token_is_type_width;
		ok = ok && bvnr_write_event(w, ev_type_annotation_type_family_parameter, &d);
		d.type = token_is_type_base;
		ok = ok && bvnr_write_event(w, ev_type_annotation_type_family_parameter, &d);
		d.type = token_is_type;
		ok = ok && bvnr_write_event(w, ev_type_annotation_end, &d);
		ASSERT_TRUE(ok, "annotation");
		ok = r4_val(w, token_is_string, vt, BVN_UNIT_NONE, "nan");
		if (wide) {
			ASSERT_TRUE(ok, "30191 fits sixteen bits");
			r4_expect(w, out, ".a=<uint:16,_36>\"nan\";", "quoted nan, base 36");
		} else {
			ASSERT_FALSE(ok, "30191 does not fit eight bits");
			ASSERT_EQ_INT(bvnr_writer_get_error(w), error_value_out_of_range,
				      "quoted nan under uint:8");
			bvnr_writer_destroy(w);
		}
	}
}

/* Counts the octet payload a document delivers, and its values. */
typedef struct { uint64_t octets; uint32_t streams; uint32_t numbers; } r4_oct_t;
static bool r4_oct_cb(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
	r4_oct_t *o = ud;
	if (ev == ev_octet_stream_start) o->streams++;
	if (ev == ev_data && d->type == token_is_octet_stream) o->octets += d->length;
	if (ev == ev_data && (d->type == token_is_number ||
			      d->type == token_is_array_number)) o->numbers++;
	return true;
}
/*
 * An octet stream is ONE array element however many chunks it is written in.
 *
 * Each chunk was treated as a value of its own, so the element separator was
 * armed after the first and written before the second: 0x00, chunk, ",", chunk,
 * 0x00. The framing has no such byte and the reader answered
 * octet_stream_out_of_sync. Outside an array nothing arms a comma, which is why
 * the same events wrote correctly at the top level.
 */
static void test_multi_chunk_octet_stream_in_array(void)
{
	printf("  test_multi_chunk_octet_stream_in_array...\n");
	enum { BIG = 65536, SMALL = 4464 };
	static uint8_t payload[BIG];
	static uint8_t out[4 * (BIG + SMALL) + 1024];
	/* ',' is the byte that went astray; a payload full of it proves the
	 * check below is about the framing and not about the data. */
	memset(payload, ',', sizeof payload);

	/* shape 0: [stream]   1: [1, stream, 2]   2: [stream]/[stream] */
	for (int shape = 0; shape < 3; shape++) {
		bvnr_writer_t *w = r4_open(out, sizeof out, 0, false);
		ASSERT_NOT_NULL(w, "writer");
		if (!w) return;
		bool ok = r4_key(w, "a") && r4_ev(w, ev_array_row_start);
		int rows = shape == 2 ? 2 : 1;
		for (int r = 0; ok && r < rows; r++) {
			if (r > 0)
				ok = r4_ev(w, ev_array_dim_start) &&
				     r4_ev(w, ev_array_row_start);
			if (ok && shape == 1)
				ok = r4_val(w, token_is_array_number, BVN_TYPE_PLAIN,
					    BVN_UNIT_NONE, "1");
			bvnr_data_t d;
			memset(&d, 0, sizeof d);
			ok = ok && bvnr_write_event(w, ev_octet_stream_start, &d);
			d.type = token_is_octet_stream; d.data = payload;
			d.length = BIG;
			ok = ok && bvnr_write_event(w, ev_data, &d);
			d.length = SMALL;
			ok = ok && bvnr_write_event(w, ev_data, &d);
			memset(&d, 0, sizeof d);
			ok = ok && bvnr_write_event(w, ev_octet_stream_end, &d);
			if (ok && shape == 1)
				ok = r4_val(w, token_is_array_number, BVN_TYPE_PLAIN,
					    BVN_UNIT_NONE, "2");
			ok = ok && r4_ev(w, ev_array_row_end);
		}
		ASSERT_TRUE(ok, "the events are accepted");
		ASSERT_TRUE(bvnr_write_finish(w), "finish");
		uint64_t n = bvnr_writer_bytes_written(w);
		bvnr_writer_destroy(w);

		r4_oct_t seen = {0};
		bvnr_reader_t *r = bvnr_reader_create();
		ASSERT_NOT_NULL(r, "reader");
		if (!r) return;
		bvnr_read_flags_t rf;
		memset(&rf, 0, sizeof rf);
		rf.userdata = &seen; rf.on_verified = r4_oct_cb;
		ASSERT_TRUE(bvnr_open_read_mem(r, out, n, NULL, 0, &rf), "open");
		ASSERT_TRUE(bvnr_read(r), "a multi-chunk element reads back");
		ASSERT_EQ_INT(bvnr_reader_get_error(r), error_none, "no error");
		ASSERT_EQ_UINT(seen.streams, (uint32_t)rows, "one stream per row");
		ASSERT_EQ_UINT(seen.octets, (uint64_t)rows * (BIG + SMALL),
			       "every payload byte, and no more");
		ASSERT_EQ_UINT(seen.numbers, shape == 1 ? 2u : 0u,
			       "the neighbouring elements");
		bvnr_reader_destroy(r);
	}
}

static void test_write_bvni_hex(void)
{
	printf("  test_write_bvni_hex...\n");

	uint8_t output[32768];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	bvn_int_t *n = bvn_int_alloc();
	ASSERT_NOT_NULL(n, "bvn_int_alloc must succeed");
	if (!n) { bvnr_writer_destroy(w); return; }

	ASSERT_TRUE(bvn_int_from_str(n, "deadbeef", 16),
				"bvn_int_from_str hex must succeed");
	ASSERT_TRUE(bvnr_write_bvni(w, "hex_val", n, 32u, 16u),
				"bvnr_write_bvni uint:32 base16 must succeed");

	ASSERT_TRUE(bvn_int_from_str(n, "-7fffffff", 16),
				"bvn_int_from_str negative hex must succeed");
	ASSERT_TRUE(bvnr_write_bvni(w, "neg_hex", n, 32u, 16u),
				"bvnr_write_bvni sint:32 base16 must succeed");

	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
				"bvni hex roundtrip must succeed");
	ASSERT_TRUE(le.vt.family == vt_sint,
				"last entry (neg_hex) must be sint");
	ASSERT_EQ_UINT(le.vt.width, 32,
				   "neg_hex width must be 32");
	ASSERT_EQ_UINT(le.vt.base, 16,
				   "neg_hex base must be 16");
	ASSERT_TRUE(strcmp(le.value, "-7fffffff") == 0,
				"neg_hex value must round-trip as \"-7fffffff\"");
	ASSERT_EQ_INT(le.value_tok, token_is_string,
				  "base16 bvni token must be token_is_string");

	bvn_int_free(n);
	bvnr_writer_destroy(w);
}

/*
 * Bases 64 and 85 use '+'/'-' as digit characters (Base64 '+'=62; Ascii85
 * '+'=10, '-'=12). They carry no sign and are unsigned-only. Verify the full
 * digit alphabet round-trips through writer->reader (a value whose digits
 * include '+'/'-' is written and read back unchanged), and that a signed
 * integer in these bases is rejected as an illegal type.
 */
static void test_write_bvni_base64_signchar(void)
{
	printf("  test_write_bvni_base64_signchar...\n");

	bvn_int_t *n = bvn_int_alloc();
	ASSERT_NOT_NULL(n, "bvn_int_alloc must succeed");
	if (!n) return;

	/* 3326 -> "z+" (digit 62 = '+'): writer emits it and the reader accepts. */
	{
		uint8_t output[4096];
		bvnr_sink_t sink;
		bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
		ASSERT_NOT_NULL(w, "make_writer (b64) must succeed");
		if (w) {
			ASSERT_TRUE(bvn_int_from_str(n, "3326", 10), "parse 3326");
			ASSERT_TRUE(bvnr_write_bvni(w, "v", n, 128u, 64u),
						"uint base-64 with a '+' digit must write");
			ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
			last_event_t le = {0};
			ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
						"writer output must read back (no writer/reader skew)");
			ASSERT_TRUE(strcmp(le.value, "z+") == 0,
						"3326 must round-trip as base-64 \"z+\"");
			bvnr_writer_destroy(w);
		}
	}

	/* 12 -> "-" in Ascii85 (digit 12 = '-'): full alphabet round-trips. */
	{
		uint8_t output[4096];
		bvnr_sink_t sink;
		bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
		ASSERT_NOT_NULL(w, "make_writer (b85) must succeed");
		if (w) {
			ASSERT_TRUE(bvn_int_from_str(n, "12", 10), "parse 12");
			ASSERT_TRUE(bvnr_write_bvni(w, "v", n, 128u, 85u),
						"uint base-85 with a '-' digit must write");
			ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
			last_event_t le = {0};
			ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
						"writer output must read back (no writer/reader skew)");
			ASSERT_TRUE(strcmp(le.value, "-") == 0,
						"12 must round-trip as base-85 \"-\"");
			bvnr_writer_destroy(w);
		}
	}

	/* A negative value forces family sint; sint is illegal for base 64/85. */
	{
		uint8_t output[4096];
		bvnr_sink_t sink;
		bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
		ASSERT_NOT_NULL(w, "make_writer (sint) must succeed");
		if (w) {
			ASSERT_TRUE(bvn_int_from_str(n, "-5", 10), "parse -5");
			ASSERT_TRUE(!bvnr_write_bvni(w, "bad", n, 128u, 64u),
						"signed integer in base 64 must be rejected");
			ASSERT_EQ_INT(bvnr_writer_get_error(w), error_illegal_value_type,
						  "rejection must report illegal_value_type");
			bvnr_writer_destroy(w);
		}
	}

	bvn_int_free(n);
}

static void test_write_streaming_flush(void)
{
	printf("  test_write_streaming_flush...\n");

	uint32_t cap = 2u * 1024u * 1024u;
	uint8_t *output = malloc(cap);
	ASSERT_NOT_NULL(output, "malloc 2MiB must succeed");
	if (!output) return;

	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, output, cap);

	bvnr_writer_t *w = bvnr_writer_create();
	ASSERT_NOT_NULL(w, "bvnr_writer_create must succeed");
	if (!w) { free(output); return; }

	ASSERT_TRUE(bvnr_open_write_sink(w, &sink, false, NULL),
				"open_write_sink must succeed");

	bvnr_data_t hdr = {0};
	ASSERT_TRUE(bvnr_write_event(w, ev_stream_start, &hdr),
				"stream_start must succeed");

	const uint32_t n_writes = 4000u;
	for (uint32_t i = 0; i < n_writes; i++) {
		char key[32];
		snprintf(key, sizeof(key), "k%u", i);
		ASSERT_TRUE(bvnr_write_uint(w, key, 64u, (uint64_t)i),
					"bvnr_write_uint must succeed in streaming loop");
	}

	uint64_t mid_count = bvnr_writer_bytes_written(w);
	ASSERT_TRUE(mid_count > 65536u,
				"bytes_written must exceed 64KiB (buffer flushed at least once)");

	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	uint64_t final_count = bvnr_writer_bytes_written(w);
	ASSERT_TRUE(final_count >= mid_count,
				"bytes_written after finish >= bytes_written before finish");

	bvnr_writer_destroy(w);
	free(output);
}

static void test_write_float_fix_roundtrip(void)
{
	printf("  test_write_float_fix_roundtrip...\n");

	uint8_t output[256];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	ASSERT_TRUE(bvnr_write_float_fix(w, "ff", 32, 16, 1.5),
				"write float_fix:32,q16 must succeed");
	ASSERT_TRUE(bvnr_write_float_fix(w, "neg", 64, 8, -0.25),
				"write float_fix:64,q8 negative must succeed");
	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
				"float_fix roundtrip must parse cleanly");
	ASSERT_TRUE(le.vt.family == vt_float_fix, "last type family = float_fix");
	ASSERT_EQ_UINT(le.vt.width, 64, "last width = 64");
	ASSERT_EQ_UINT(le.vt.base, 8, "last Q = 8 (stored in base)");
	ASSERT_TRUE(le.value_tok == token_is_number,
				"float_fix data token is token_is_number");

	bvnr_writer_destroy(w);
}

/*
 * Regression: float_fix stores Q in the .base field, but Q is NOT a numeral
 * base. Q values outside the {2-62,64,85} numeral-base set (e.g. 1, 63, 100)
 * must still be writable as long as Q < width — the reader accepts them, so the
 * writer must round-trip them. Q == width (or greater) must still be rejected.
 */
static void test_write_float_fix_q_edges(void)
{
	printf("  test_write_float_fix_q_edges...\n");

	const struct { uint32_t w, q; double v; } good[] = {
		{ 32, 1,   0.5    },
		{ 64, 63,  0.125  },
		{ 128, 100, 0.0625 },
		{ 256, 200, 0.03125 },
	};
	for (size_t i = 0; i < sizeof(good) / sizeof(good[0]); i++) {
		uint8_t output[256];
		bvnr_sink_t sink;
		bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
		ASSERT_NOT_NULL(w, "make_writer must succeed");
		if (!w) continue;
		ASSERT_TRUE(bvnr_write_float_fix(w, "x", good[i].w, good[i].q, good[i].v),
					"write float_fix with valid Q outside numeral-base set");
		ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
		ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no write error");
		last_event_t le = {0};
		ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
					"float_fix q-edge roundtrip must parse cleanly");
		ASSERT_EQ_UINT(le.vt.base, good[i].q, "Q preserved through roundtrip");
		bvnr_writer_destroy(w);
	}

	/* Q == width must be rejected. */
	uint8_t output[64];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (w) {
		ASSERT_FALSE(bvnr_write_float_fix(w, "x", 32, 32, 1.0),
					 "float_fix with Q == width must fail");
		bvnr_writer_destroy(w);
	}

	/* Value outside the Q-format's representable range must be rejected,
	 * not silently saturated/wrapped on a later decode. */
	const struct { uint32_t w, q; double v; } oor[] = {
		{ 16, 8, 1000000.0 },   /* range [-128, 127.996] */
		{ 16, 8, 128.0 },
		{ 16, 8, -200.0 },
		{ 32, 16, 1e9 },
		{ 16, 0, 70000.0 },     /* q0: signed 16-bit int range */
	};
	for (size_t i = 0; i < sizeof(oor) / sizeof(oor[0]); i++) {
		uint8_t obuf[64];
		bvnr_sink_t sk;
		bvnr_writer_t *ww = make_writer(obuf, sizeof(obuf), &sk);
		ASSERT_NOT_NULL(ww, "make_writer must succeed");
		if (!ww) continue;
		ASSERT_FALSE(bvnr_write_float_fix(ww, "x", oor[i].w, oor[i].q, oor[i].v),
					 "out-of-range float_fix value must be rejected");
		ASSERT_EQ_INT(bvnr_writer_get_error(ww), error_value_out_of_range,
					  "rejection must be error_value_out_of_range");
		bvnr_writer_destroy(ww);
	}
}

static void test_write_float_dec_roundtrip(void)
{
	printf("  test_write_float_dec_roundtrip...\n");

	uint8_t output[256];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	ASSERT_TRUE(bvnr_write_float_dec(w, "fd", 64, 3.14),
				"write float_dec:64 must succeed");
	ASSERT_TRUE(bvnr_write_float_dec(w, "small", 32, -1.5e-3f),
				"write float_dec:32 negative must succeed");
	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no error");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
				"float_dec roundtrip must parse cleanly");
	ASSERT_TRUE(le.vt.family == vt_float_dec, "last type family = float_dec");
	ASSERT_EQ_UINT(le.vt.width, 32, "last width = 32");
	ASSERT_TRUE(le.value_tok == token_is_number,
				"float_dec data token is token_is_number");

	bvnr_writer_destroy(w);
}

static void test_validate_reference(void)
{
	printf("  test_validate_reference...\n");

	ASSERT_TRUE(bvn_validate_reference(".host"),
				"'.host' is a valid reference");
	ASSERT_TRUE(bvn_validate_reference(".config.host"),
				"'.config.host' is a valid multi-segment reference");
	ASSERT_TRUE(bvn_validate_reference(".a.b.c"),
				"'.a.b.c' is a valid three-segment reference");
	ASSERT_TRUE(bvn_validate_reference("._under_score"),
				"'._under_score' is valid");

	ASSERT_FALSE(bvn_validate_reference("host"),
				 "'host' without leading dot must be rejected");
	ASSERT_FALSE(bvn_validate_reference(""),
				 "empty string must be rejected");
	ASSERT_FALSE(bvn_validate_reference(NULL),
				 "NULL must be rejected");
	ASSERT_FALSE(bvn_validate_reference("."),
				 "'.' (dot only, no id-start) must be rejected");
	ASSERT_FALSE(bvn_validate_reference("..host"),
				 "'..host' (double dot at start) must be rejected");
	ASSERT_FALSE(bvn_validate_reference(".host."),
				 "'.host.' (trailing dot, no id-start) must be rejected");
}

static void test_write_reference_roundtrip(void)
{
	printf("  test_write_reference_roundtrip...\n");

	uint8_t output[4096];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	bvnr_data_t ka = {
		.type   = token_is_identifier,
		.data   = "x",
		.length = 1,
	};
	ASSERT_TRUE(bvnr_write_event(w, ev_assignment_start, &ka),
				"assignment_start must succeed");

	bvnr_data_t kd = {
		.type   = token_is_reference,
		.data   = ".host",
		.length = 5,
	};
	ASSERT_TRUE(bvnr_write_event(w, ev_data, &kd),
				"writing valid reference '.host' must succeed");

	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");

	uint64_t n = bvnr_writer_bytes_written(w);
	ASSERT_TRUE(n > 0, "bytes_written > 0");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, n, &le),
				"reference roundtrip must parse cleanly");
	ASSERT_TRUE(strcmp(le.key, "x") == 0, "last key == x");
	ASSERT_TRUE(strcmp(le.value, ".host") == 0,
				"last value == .host (with leading dot)");
	ASSERT_EQ_INT(le.value_tok, token_is_reference,
				  "token type must be token_is_reference");

	bvnr_writer_destroy(w);
}

static void test_write_reference_without_dot_rejected(void)
{
	printf("  test_write_reference_without_dot_rejected...\n");

	uint8_t output[256];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	bvnr_data_t ka = {
		.type   = token_is_identifier,
		.data   = "x",
		.length = 1,
	};
	bvnr_write_event(w, ev_assignment_start, &ka);

	bvnr_data_t kd = {
		.type   = token_is_reference,
		.data   = "host",
		.length = 4,
	};
	ASSERT_FALSE(bvnr_write_event(w, ev_data, &kd),
				 "writing reference without leading dot must be rejected");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_type_value_mismatch,
				  "error must be type_value_mismatch");

	bvnr_writer_destroy(w);
}

static void test_write_reference_dot_only_rejected(void)
{
	printf("  test_write_reference_dot_only_rejected...\n");

	uint8_t output[256];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	bvnr_data_t ka = {
		.type   = token_is_identifier,
		.data   = "x",
		.length = 1,
	};
	bvnr_write_event(w, ev_assignment_start, &ka);

	bvnr_data_t kd = {
		.type   = token_is_reference,
		.data   = ".",
		.length = 1,
	};
	ASSERT_FALSE(bvnr_write_event(w, ev_data, &kd),
				 "writing reference '.' (dot only) must be rejected");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_type_value_mismatch,
				  "error must be type_value_mismatch");

	bvnr_writer_destroy(w);
}

static void test_write_string_vf_escapes(void)
{
	printf("  test_write_string_vf_escapes...\n");

	uint8_t output[4096];
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
	ASSERT_NOT_NULL(w, "make_writer must succeed");
	if (!w) return;

	char vf_str[] = {'v', 't', '\x0b', 'f', 'f', '\x0c', '\0'};
	ASSERT_TRUE(bvnr_write_string(w, "s", vf_str),
				"write string with \\v and \\f must succeed");
	ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");

	uint64_t n = bvnr_writer_bytes_written(w);
	ASSERT_TRUE(n > 0, "bytes_written > 0");

	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(output, n, &le),
				"string with \\v and \\f must roundtrip cleanly");
	ASSERT_EQ_INT(le.value_len, (int64_t)strlen(vf_str),
				  "roundtripped string length matches");
	ASSERT_TRUE(memcmp(le.value, vf_str, le.value_len) == 0,
				"roundtripped string content matches (\\v and \\f preserved)");

	bvnr_writer_destroy(w);
}

static void test_write_version_directive(void)
{
	printf("  test_write_version_directive...\n");

	/* explicit bvnr_write_version before any value, round-tripped through the reader */
	uint8_t out[4096];
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, out, sizeof(out));
	bvnr_writer_t *w = bvnr_writer_create();
	ASSERT_NOT_NULL(w, "writer create");
	if (!w) return;
	ASSERT_TRUE(bvnr_open_write_sink(w, &sink, true, NULL), "open");
	ASSERT_TRUE(bvnr_write_version(w, 1, 1), "write_version before any value");
	ASSERT_TRUE(bvnr_write_uint(w, "x", 8, 7), "write a value after the directive");
	ASSERT_TRUE(bvnr_write_finish(w), "finish");
	uint64_t n = bvnr_writer_bytes_written(w);
	ASSERT_TRUE(n > 13 && memcmp(out, "#!bovnar 1.1\n", 13) == 0,
		"output begins with the version directive");
	uint16_t maj = 0, min = 0;
	ASSERT_TRUE(bvnr_peek_version(out, n, &maj, &min) && maj == 1 && min == 1,
		"emitted directive is re-readable");
	bvnr_writer_destroy(w);

	/* calling bvnr_write_version once output has begun is rejected */
	bvnr_sink_to_mem(&sink, out, sizeof(out));
	w = bvnr_writer_create();
	ASSERT_NOT_NULL(w, "writer create 2");
	if (!w) return;
	ASSERT_TRUE(bvnr_open_write_sink(w, &sink, true, NULL), "open 2");
	ASSERT_TRUE(bvnr_write_uint(w, "x", 8, 1), "write a value first");
	ASSERT_FALSE(bvnr_write_version(w, 1, 1), "write_version after output is rejected");
	ASSERT_EQ_INT(bvnr_writer_get_error(w), error_invalid_argument,
		"late write_version sets error_invalid_argument");
	bvnr_writer_destroy(w);

	/* emit_version write flag stamps the current spec version on open */
	bvnr_sink_to_mem(&sink, out, sizeof(out));
	w = bvnr_writer_create();
	ASSERT_NOT_NULL(w, "writer create 3");
	if (!w) return;
	bvnr_write_flags_t wf = {0};
	wf.emit_version = true;
	ASSERT_TRUE(bvnr_open_write_sink(w, &sink, true, &wf), "open with emit_version");
	ASSERT_TRUE(bvnr_write_uint(w, "x", 8, 1), "write a value");
	ASSERT_TRUE(bvnr_write_finish(w), "finish 3");
	maj = min = 0;
	ASSERT_TRUE(bvnr_peek_version(out, bvnr_writer_bytes_written(w), &maj, &min) &&
		maj == BVNR_SPEC_VERSION_MAJOR && min == BVNR_SPEC_VERSION_MINOR,
		"emit_version stamps the current spec version");
	bvnr_writer_destroy(w);
}

static void test_write_datetime(void)
{
	printf("  test_write_datetime...\n");
	uint8_t out[512];
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, out, sizeof(out));
	bvnr_writer_t *w = bvnr_writer_create();
	ASSERT_NOT_NULL(w, "writer create");
	if (!w) return;
	ASSERT_TRUE(bvnr_open_write_sink(w, &sink, false, NULL), "open");
	ASSERT_TRUE(bvnr_write_version(w, 1, 1), "version directive");
	/* the epoch must survive the direct write API (regression: it was dropped) */
	ASSERT_TRUE(bvnr_write_datetime(w, "a", 64, "gps", 1750000000),
		"write datetime gps");
	ASSERT_TRUE(bvnr_write_datetime(w, "b", 0, NULL, -100),
		"write datetime unix default, negative");
	ASSERT_TRUE(bvnr_write_finish(w), "finish");
	uint64_t n = bvnr_writer_bytes_written(w);
	out[n < sizeof(out) ? n : sizeof(out) - 1] = '\0';
	ASSERT_TRUE(strstr((char *)out, "<datetime:64,gps>") != NULL,
		"epoch 'gps' is emitted in the annotation");
	ASSERT_TRUE(strstr((char *)out, "<datetime>") != NULL,
		"unix default epoch stays implicit");
	/* the produced document re-parses (annotation + directive are consistent) */
	last_event_t le = {0};
	ASSERT_TRUE(roundtrip(out, n, &le), "datetime document round-trips");
	bvnr_writer_destroy(w);

	/* an unknown epoch name is rejected */
	bvnr_sink_to_mem(&sink, out, sizeof(out));
	w = bvnr_writer_create();
	if (w) {
		bvnr_open_write_sink(w, &sink, false, NULL);
		ASSERT_FALSE(bvnr_write_datetime(w, "x", 64, "nope", 1),
			"unknown epoch name is rejected");
		bvnr_writer_destroy(w);
	}
	ASSERT_EQ_INT(bvnr_datetime_epoch_index("tai"), 1, "epoch index tai == 1");
	ASSERT_EQ_INT(bvnr_datetime_epoch_index(NULL), 0, "NULL epoch == unix (0)");
	ASSERT_EQ_INT(bvnr_datetime_epoch_index("bogus"), -1, "unknown epoch == -1");

	/* the writer must reject a datetime the reader would reject (no
	 * writer/reader asymmetry): out-of-range value and over-wide carrier. */
	{
		uint8_t b2[128]; bvnr_sink_t s2;
		struct { uint32_t w; const char *ep; int64_t v; const char *why; } bad[] = {
			{ 8,  NULL,   300,   "datetime:8 300 over signed range" },
			{ 8,  "unix", -200,  "datetime:8 -200 under signed range" },
			{ 16, NULL,   32768, "datetime:16 32768 over signed range" },
			{ 33000, NULL, 1,    "datetime:33000 width over BVN_MAX_INT_WIDTH" },
		};
		for (size_t i = 0; i < sizeof(bad)/sizeof(bad[0]); i++) {
			bvnr_sink_to_mem(&s2, b2, sizeof(b2));
			bvnr_writer_t *w2 = bvnr_writer_create();
			if (!w2) continue;
			bvnr_open_write_sink(w2, &s2, false, NULL);
			ASSERT_FALSE(bvnr_write_datetime(w2, "x", bad[i].w, bad[i].ep, bad[i].v),
				bad[i].why);
			bvnr_writer_destroy(w2);
		}
		/* in-range still accepted — but datetime is a spec-1.1 construct, so the
		 * document has to declare 1.1 or the reader will not accept it back.
		 * This used to pass with default flags, silently producing exactly such
		 * an unreadable document. */
		bvnr_write_flags_t vf;
		memset(&vf, 0, sizeof vf);
		vf.emit_version = true;
		bvnr_sink_to_mem(&s2, b2, sizeof(b2));
		bvnr_writer_t *w3 = bvnr_writer_create();
		if (w3) {
			bvnr_open_write_sink(w3, &s2, false, &vf);
			ASSERT_TRUE(bvnr_write_datetime(w3, "x", 8, NULL, 127),
				"datetime:8 127 is in range");
			bvnr_writer_destroy(w3);
		}
		/* ...and without the directive it is refused rather than written. */
		bvnr_sink_to_mem(&s2, b2, sizeof(b2));
		bvnr_writer_t *w4 = bvnr_writer_create();
		if (w4) {
			bvnr_open_write_sink(w4, &s2, false, NULL);
			ASSERT_FALSE(bvnr_write_datetime(w4, "x", 8, NULL, 127),
				"a datetime without a 1.1 directive is refused");
			ASSERT_EQ_INT((int)bvnr_writer_get_error(w4),
				(int)error_unsupported_spec_version,
				"...naming the spec version as the reason");
			bvnr_writer_destroy(w4);
		}
	}
}

/*
 * spec 1.1 — the writer re-emits a datetime carrying captured ISO sub-second
 * digits (bvnr_data_t.frac_data) as an ISO literal so it round-trips. These
 * cases drive the raw event API directly to cover the hardening guards that the
 * reader path can never reach: a fraction with non-digit bytes, an atomic GNSS
 * epoch (which rejects literals), and a near-INT64_MIN tai carrier whose civil
 * conversion overflows (the converter leaves *dt untouched). All three must
 * fall back to the plain integer carrier — never a corrupt literal or a crash.
 *
 * The fourth case is the opposite ruling, and the only one a real document can
 * reach: a tz offset can push the UTC year outside 0000-9999 while the literal
 * itself parses. There the value is fine and only its ISO spelling is missing,
 * so the integer fallback would silently drop digits the spec promises to keep
 * — the writer must refuse instead. Returns the bvnr_write_finish() result.
 */
static bool emit_dt_value(uint8_t *out, uint32_t cap, uint32_t *outlen,
	uint32_t epoch_base, const char *carrier,
	const char *frac, uint32_t frac_len)
{
	bvnr_sink_t sink;
	bvnr_writer_t *w = make_writer(out, cap, &sink);
	if (!w) { *outlen = 0; return false; }
	const char *key = "t";
	bvnr_data_t d = { .type = token_is_identifier,
	                  .data = key, .length = 1 };
	bvnr_write_event(w, ev_assignment_start, &d);
	value_type_spec_t vt = { .family = vt_datetime, .width = 64,
	                         .base = epoch_base };
	d = (bvnr_data_t){ .type = token_is_number, .value_type = vt,
	                   .data = carrier, .length = (uint32_t)strlen(carrier),
	                   .frac_data = frac, .frac_length = frac_len };
	bvnr_write_event(w, ev_data, &d);
	bool ok = bvnr_write_finish(w);
	*outlen = (uint32_t)bvnr_writer_bytes_written(w);
	if (*outlen < cap) out[*outlen] = '\0';
	bvnr_writer_destroy(w);
	return ok;
}
static void test_write_datetime_fraction_guards(void)
{
	printf("  test_write_datetime_fraction_guards...\n");
	uint8_t out[4096];
	uint32_t n;

	/* 1. valid unix fractional datetime -> re-emitted as an ISO literal */
	emit_dt_value(out, sizeof out, &n, 0, "1781524800", "5", 1);
	ASSERT_TRUE(strstr((char *)out, "2026-06-15T12:00:00.5Z") != NULL,
		"valid fraction re-emits as an ISO literal");

	/* 2. a fraction with non-digit bytes must NOT produce a corrupt literal;
	 *    fall back to the integer carrier. */
	emit_dt_value(out, sizeof out, &n, 0, "1781524800", "5Z", 2);
	ASSERT_TRUE(strstr((char *)out, "1781524800") != NULL &&
	            strstr((char *)out, "2026-") == NULL,
		"non-digit fraction falls back to the integer carrier");

	/* 3. an atomic GNSS epoch (gps=2) rejects literals on read, so a fraction
	 *    must fall back to the integer carrier rather than emit one. */
	emit_dt_value(out, sizeof out, &n, 2, "1781524800", "5", 1);
	ASSERT_TRUE(strstr((char *)out, "1781524800") != NULL &&
	            strstr((char *)out, "2026-") == NULL,
		"GNSS-epoch fraction falls back to the integer carrier");

	/* 4. a near-INT64_MIN tai carrier whose civil conversion overflows must not
	 *    read uninitialised civil fields; it falls back to the integer carrier. */
	emit_dt_value(out, sizeof out, &n, 1, "-9223372036854775808", "5", 1);
	ASSERT_TRUE(strstr((char *)out, "-9223372036854775808") != NULL,
		"tai overflow falls back to the integer carrier (no UB)");

	/* 5. the inverse: a UTC year outside 0000-9999 has no ISO spelling, but the
	 *    integer carrier cannot hold the fraction either. Unlike 2-4 this IS
	 *    reachable from a document — "0000-01-01T00:00:00.5+23:59" parses, and
	 *    its UTC year is -1 — so the writer must refuse rather than drop the
	 *    digits. Without a fraction the same carrier falls back normally. */
	ASSERT_TRUE(!emit_dt_value(out, sizeof out, &n, 0, "-62167305540", "5", 1),
		"out-of-range UTC year with a fraction is refused, not truncated");
	ASSERT_TRUE(emit_dt_value(out, sizeof out, &n, 0, "-62167305540", NULL, 0) &&
	            strstr((char *)out, "-62167305540") != NULL,
		"the same carrier without a fraction still falls back");
}
/*
 * The canon-observer path drives the serializer with no validation (unlike the
 * bvnr_write_* API, which validates first). An unbalanced ev_struct_end — no
 * matching ev_struct_start, so struct_depth is 0 — must be self-rejected: without
 * the guard in bvn_ser_serialize_event, indent/struct_depth would underflow and
 * the next bvn_ser_indent would emit ~4 billion tab bytes. This is the only path
 * that reaches that guard, so it is exercised here directly. Pretty mode is on so
 * indentation is actually emitted: without the guard the run would flood the
 * sink, so bytes_written==0 (not the on_event return alone) is what pins it.
 */
static void test_canon_observer_unbalanced_struct_end_rejected(void)
{
	uint8_t buf[256];
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, buf, sizeof buf);

	bvnr_canon_observer_t *obs = bvnr_canon_observer_create(&sink, true);
	ASSERT_NOT_NULL(obs, "canon observer create");
	if (!obs) return;

	bvnr_data_t data;
	memset(&data, 0, sizeof data);
	ASSERT_FALSE(bvnr_canon_observer_on_event(obs, ev_struct_end, &data),
		"unbalanced struct_end must be rejected, not underflow");
	ASSERT_TRUE(bvnr_sink_bytes_written(&sink) == 0,
		"rejected struct_end must not emit any bytes");

	bvnr_canon_observer_destroy(obs);
}
/*
 * A canonicalising observer must be able to re-emit a document's version
 * directive, or a spec-1.1 document (datetime, the new escapes, …)
 * canonicalises to output a reader then rejects as 1.0. The realistic timing
 * (mirrored here) is that the driver learns the declared version only AFTER the
 * leading ev_stream_start event — a reader resolves it while parsing the
 * directive line — but still before any value bytes; the observer emits the
 * directive lazily ahead of the first event that follows.
 */
static void test_canon_observer_emits_version(void)
{
	printf("  test_canon_observer_emits_version...\n");
	uint8_t out[256];
	memset(out, 0, sizeof out);
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, out, sizeof out);
	bvnr_canon_observer_t *obs = bvnr_canon_observer_create(&sink, false);
	ASSERT_NOT_NULL(obs, "observer create");
	if (!obs) return;

	bvnr_data_t data;
	memset(&data, 0, sizeof data);

	/* The observer copies the sink, so its byte counter lives on its private
	 * copy; read the shared output buffer directly instead. ev_stream_start
	 * emits no value bytes, so the buffer holds only the directive (ASCII, no
	 * embedded NUL), which makes strlen a valid length here. */

	/* stream-start arrives before the version is known: nothing is emitted */
	ASSERT_TRUE(bvnr_canon_observer_on_event(obs, ev_stream_start, &data),
		"stream-start accepted");
	ASSERT_TRUE(out[0] == 0, "no output before the version is known");

	/* the driver now learns the declared version (still ahead of any value) */
	bvnr_canon_observer_set_version(obs, 1, 1);
	ASSERT_TRUE(bvnr_canon_observer_on_event(obs, ev_stream_start, &data),
		"next event accepted");
	ASSERT_TRUE(memcmp(out, "#!bovnar 1.1\n", 13) == 0,
		"observer prepends the version directive ahead of the first event");
	size_t n = strlen((const char *)out);
	ASSERT_TRUE(n == 13, "directive is exactly the one line");
	uint16_t maj = 0, min = 0;
	ASSERT_TRUE(bvnr_peek_version(out, (uint64_t)n, &maj, &min) && maj == 1 && min == 1,
		"emitted directive re-reads as 1.1");

	/* the directive is emitted exactly once even across further events */
	ASSERT_TRUE(bvnr_canon_observer_on_event(obs, ev_stream_start, &data),
		"further event accepted");
	ASSERT_TRUE(strlen((const char *)out) == 13, "directive emitted exactly once");

	/* a late set_version (after the directive went out) is a harmless no-op */
	bvnr_canon_observer_set_version(obs, 2, 0);
	ASSERT_TRUE(strlen((const char *)out) == 13,
		"set_version after emission does not re-emit");

	bvnr_canon_observer_destroy(obs);
}

/*
 * BVN_UNIT_REDUCE must move the VALUE with the unit. Reducing km to m and
 * leaving the digits alone wrote "5 km" as "5 m" — the annotation saying one
 * thing and the number another, in a format whose premise is that a unit
 * confusion is the expensive failure.
 */
static int reduce_emit(char *out, size_t cap, value_unit_t u,
		       value_type_spec_t vt, const char *lit)
{
	memset(out, 0, cap);
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, out, cap);
	bvnr_writer_t *w = bvnr_writer_create();
	if (!w) return -1;
	bvnr_write_flags_t wf;
	memset(&wf, 0, sizeof wf);
	wf.unit_flags = BVN_UNIT_REDUCE;
	if (!bvnr_open_write_sink(w, &sink, false, &wf)) {
		bvnr_writer_destroy(w); return -1;
	}
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	bvnr_write_event(w, ev_stream_start, &d);
	d.type = token_is_identifier; d.data = "d"; d.length = 1;
	bvnr_write_event(w, ev_assignment_start, &d);
	memset(&d, 0, sizeof d);
	d.type       = token_is_number;
	d.value_type = vt;
	d.value_unit = u;
	d.data       = lit;
	d.length     = (uint32_t)strlen(lit);
	bool ok = bvnr_write_event(w, ev_data, &d);
	error_code_t err = bvnr_writer_get_error(w);
	memset(&d, 0, sizeof d);
	bvnr_write_event(w, ev_stream_end, &d);
	bvnr_writer_destroy(w);
	return ok ? 0 : (int)err;
}

/* The same value under `<type,unit>`: the unit on the annotation, the event's
 * own unit left to it. */
static int reduce_emit_annotated(char *out, size_t cap, value_unit_t u,
				 value_type_spec_t vt, const char *lit)
{
	memset(out, 0, cap);
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, out, cap);
	bvnr_writer_t *w = bvnr_writer_create();
	if (!w) return -1;
	bvnr_write_flags_t wf;
	memset(&wf, 0, sizeof wf);
	wf.unit_flags = BVN_UNIT_REDUCE;
	if (!bvnr_open_write_sink(w, &sink, false, &wf)) {
		bvnr_writer_destroy(w); return -1;
	}
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	d.type = token_is_identifier; d.data = "d"; d.length = 1;
	bvnr_write_event(w, ev_assignment_start, &d);
	bool ok = bvnr_write_type_annotation(w, vt, u);
	memset(&d, 0, sizeof d);
	d.type       = token_is_number;
	d.value_type = vt;
	d.value_unit = u;
	d.data       = lit;
	d.length     = (uint32_t)strlen(lit);
	ok = ok && bvnr_write_event(w, ev_data, &d);
	error_code_t err = bvnr_writer_get_error(w);
	bvnr_writer_destroy(w);
	return ok ? 0 : (int)err;
}

/*
 * The same, for a QUOTED number literal.
 *
 * A quoted numeric is a `token_is_string` carrying a numeric annotation, and the
 * rescale used to live only under `token_is_number`. The ANNOTATION is written
 * by the common path and was reduced regardless, so the unit lost its prefix
 * while the digits stayed put:
 *
 *     <uint:64,_10,k~m> "7"     ->  <uint:64,_10,m> "7"
 *     <float:64,_16,k~m> "1p0"  ->  <float:64,_16,m> "1p0"
 *
 * Seven kilometres written back as seven metres, silently, with a successful
 * return. It hit the NON-DECIMAL bases hardest because they have no alternative
 * -- the spec requires the quoted form for a base-16 float -- so every such
 * value with a prefixed unit was exposed to `pretty-print --canonical`.
 *
 * Driven through the same writer the bare-number case uses, with only the token
 * type and the base changed, so a regression in either shape shows up here.
 */
static int reduce_emit_quoted(char *out, size_t cap, value_unit_t u,
			      value_type_spec_t vt, const char *lit)
{
	memset(out, 0, cap);
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, out, cap);
	bvnr_writer_t *w = bvnr_writer_create();
	if (!w) return -1;
	bvnr_write_flags_t wf;
	memset(&wf, 0, sizeof wf);
	wf.unit_flags = BVN_UNIT_REDUCE;
	if (!bvnr_open_write_sink(w, &sink, false, &wf)) {
		bvnr_writer_destroy(w); return -1;
	}
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	bvnr_write_event(w, ev_stream_start, &d);
	d.type = token_is_identifier; d.data = "d"; d.length = 1;
	bvnr_write_event(w, ev_assignment_start, &d);
	memset(&d, 0, sizeof d);
	d.type       = token_is_string;      /* the quoted form */
	d.value_type = vt;
	d.value_unit = u;
	d.data       = lit;
	d.length     = (uint32_t)strlen(lit);
	bool ok = bvnr_write_event(w, ev_data, &d);
	error_code_t err = bvnr_writer_get_error(w);
	memset(&d, 0, sizeof d);
	bvnr_write_event(w, ev_stream_end, &d);
	bvnr_writer_destroy(w);
	return ok ? 0 : (int)err;
}

static void test_unit_reduce_rescales_a_quoted_literal(void)
{
	char out[512];
	value_unit_t km = BVN_UNIT_SI(bu_meter, si_kilo);
	static const struct { uint8_t base; value_type_family_t fam;
			      const char *lit; const char *want; const char *what; }
	cases[] = {
		{ 10, vt_uint,  "7",   ".d=\"7000\" m",          "base 10, uint"  },
		{ 16, vt_uint,  "7",   ".d=\"1b58\" m",          "base 16, uint"  },
		{  2, vt_uint,  "111", ".d=\"1101101011000\" m", "base 2, uint"   },
		{ 36, vt_uint,  "z",   ".d=\"r08\" m",           "base 36, uint"  },
		{ 16, vt_float, "1p0", ".d=\"3e8\" m",           "base 16, float" },
		{ 10, vt_float, "1.0", ".d=\"1000\" m",          "base 10, float" },
	};
	for (size_t i = 0; i < sizeof cases / sizeof cases[0]; i++) {
		value_type_spec_t vt;
		memset(&vt, 0, sizeof vt);
		vt.family = cases[i].fam;
		vt.width  = 64;
		vt.base   = cases[i].base;
		int rc = reduce_emit_quoted(out, sizeof out, km, vt, cases[i].lit);
		ASSERT_EQ_INT(rc, 0, cases[i].what);
		ASSERT_TRUE(strstr(out, cases[i].want) != NULL, cases[i].what);
		if (!strstr(out, cases[i].want))
			fprintf(stderr, "  %s: wanted %s in %s\n",
				cases[i].what, cases[i].want, out);
	}

	/*
	 * THE INLINE UNIT SURVIVES, which is a separate failure from the rescale
	 * above and a worse one. The append lived in the token_is_number branch and
	 * nowhere else, so a quoted literal carrying a value-side unit lost it
	 * outright -- with NO flags set at all, on an ordinary write:
	 *
	 *     <uint:64,_16> "7" k~m   ->   <uint:64,_16> "7"
	 *
	 * Seven kilometres rewritten as a bare dimensionless seven. A value losing
	 * its unit is the one outcome this format exists to make impossible, and the
	 * quoted form is not exotic -- the spec requires it for every non-decimal
	 * base, so it was the ONLY way to write a base-16 value at all.
	 *
	 * Asserted with the unit on the VALUE (no annotation carrying it), because
	 * an annotated unit takes the other path and never showed the bug.
	 */
	{
		char nf[512];
		memset(nf, 0, sizeof nf);
		bvnr_sink_t sink;
		bvnr_sink_to_mem(&sink, nf, sizeof nf);
		bvnr_writer_t *w = bvnr_writer_create();
		ASSERT_TRUE(w != NULL, "writer for the inline-unit case");
		bvnr_write_flags_t wf;
		memset(&wf, 0, sizeof wf);      /* no unit_flags: plain write */
		ASSERT_TRUE(bvnr_open_write_sink(w, &sink, false, &wf), "open");
		bvnr_data_t d;
		memset(&d, 0, sizeof d);
		bvnr_write_event(w, ev_stream_start, &d);
		d.type = token_is_identifier; d.data = "d"; d.length = 1;
		bvnr_write_event(w, ev_assignment_start, &d);
		memset(&d, 0, sizeof d);
		d.type              = token_is_string;
		d.value_type.family = vt_uint;
		d.value_type.width  = 64;
		d.value_type.base   = 16;
		d.value_unit        = km;
		d.data              = "7";
		d.length            = 1;
		ASSERT_TRUE(bvnr_write_event(w, ev_data, &d),
			    "a quoted literal with an inline unit writes");
		memset(&d, 0, sizeof d);
		bvnr_write_event(w, ev_stream_end, &d);
		bvnr_writer_destroy(w);
		ASSERT_TRUE(strstr(nf, "k~m") != NULL,
			    "...and KEEPS its inline unit — losing it makes 7 km a bare 7");
		if (!strstr(nf, "k~m"))
			fprintf(stderr, "  inline unit dropped, wrote: %s\n", nf);
	}

	/* A genuine string carries no unit and must pass through untouched --
	 * bvn_ser_reduced_number answers 0 for a non-numeric type, and this is what
	 * proves the new call site did not start rewriting text. */
	value_type_spec_t utf8;
	memset(&utf8, 0, sizeof utf8);
	utf8.family = vt_utf8;
	int rc = reduce_emit_quoted(out, sizeof out, BVN_UNIT_NONE, utf8, "hello");
	ASSERT_EQ_INT(rc, 0, "a utf8 string still writes");
	ASSERT_TRUE(strstr(out, "\"hello\"") != NULL,
		    "...and its text is untouched by the reduce path");
}

static void test_unit_reduce_rescales_the_value(void)
{
	char out[512];
	value_type_spec_t f64 = { .family = vt_float, .width = 64 };
	static const struct { const char *lbl; const char *lit; const char *want; }
	cases[] = {
		{ "k~m",  "5",   ".d=5000 m"   },
		{ "c~m",  "5",   ".d=0.05 m"   },
		{ "k~g",  "2.5", ".d=2500 g"   },
		{ "n~s",  "7",   ".d=0.000000007 s" },
	};
	value_unit_t units[] = {
		BVN_UNIT_SI(bu_meter,  si_kilo),
		BVN_UNIT_SI(bu_meter,  si_centi),
		BVN_UNIT_SI(bu_gram,   si_kilo),
		BVN_UNIT_SI(bu_second, si_nano),
	};
	for (size_t i = 0; i < sizeof cases / sizeof cases[0]; i++) {
		ASSERT_EQ_INT(reduce_emit(out, sizeof out, units[i], f64, cases[i].lit),
			      0, "REDUCE writes the value");
		ASSERT_TRUE(strcmp(out, cases[i].want) == 0,
			    "REDUCE scales the value along with the unit");
	}

	/* A named SI derived unit keeps its prefix, so the value must NOT move.
	 * Rescaling by bvn_unit_reduce's raw scale here would multiply twice over. */
	ASSERT_EQ_INT(reduce_emit(out, sizeof out, BVN_UNIT_SI(bu_newton, si_kilo),
				  f64, "1"), 0, "kN writes");
	ASSERT_TRUE(strcmp(out, ".d=1 k~N") == 0,
		    "a named SI unit keeps its prefix and its value");

	/* Every digit survives: the rescale goes through exact rational arithmetic,
	 * not the double scale. */
	/* Under its annotation: a bare number is 64 bits to the reader whatever
	 * the event claims, so this used to assert a document nothing reads. */
	value_type_spec_t u128 = { .family = vt_uint, .width = 128 };
	ASSERT_EQ_INT(reduce_emit_annotated(out, sizeof out,
				  BVN_UNIT_SI(bu_meter, si_kilo),
				  u128, "340282366920938463463374607431768211"),
		      0, "a 128-bit value writes");
	ASSERT_TRUE(strcmp(out,
		".d=<uint:128,m>340282366920938463463374607431768211000") == 0,
		    "the rescale is exact for a value far wider than a double");

	/* ...but the rescaled value still has to fit the type it was declared with.
	 * The rescale happens after the writer validated the ORIGINAL text, so
	 * without re-checking it emitted values orders of magnitude past the declared
	 * width and reported success for a document its own reader rejects. */
	ASSERT_EQ_INT(reduce_emit(out, sizeof out, BVN_UNIT_SI(bu_meter, si_kilo),
				  u128, "340282366920938463463374607431768211455"),
		      (int)error_value_out_of_range,
		      "uint128 max scaled by 1000 no longer fits and is refused");
	value_type_spec_t u32 = { .family = vt_uint, .width = 32 };
	ASSERT_EQ_INT(reduce_emit(out, sizeof out, BVN_UNIT_IEC(bu_byte, iec_zebi),
				  u32, "20"),
		      (int)error_value_out_of_range,
		      "20 ZiB in bytes overflows uint32 and is refused");
	/* An integer count that stops being an integer is refused too: 20 ms is not
	 * a whole number of seconds. */
	ASSERT_EQ_INT(reduce_emit(out, sizeof out, BVN_UNIT_SI(bu_second, si_milli),
				  u32, "20"),
		      (int)error_value_out_of_range,
		      "20 ms is not an integral number of seconds and is refused");
	/* The same value under a float type is fine. */
	ASSERT_EQ_INT(reduce_emit(out, sizeof out, BVN_UNIT_SI(bu_second, si_milli),
				  f64, "20"), 0, "20 ms as a float writes");
	ASSERT_TRUE(strcmp(out, ".d=0.02 s") == 0, "...as 0.02 s");

	/* float_fix has a declared RANGE too, and the tightest of any family: q16 in
	 * 32 bits leaves 15 integer bits. The re-check above covered only uint/sint,
	 * so a rescale that overflowed the Q format was written out and reported as
	 * success -- <float_fix:32,q16,k~m> 30000 became <float_fix:32,q16,m>
	 * 30000000, which this library's own reader rejects. */
	{
		value_type_spec_t q16 = { .family = vt_float_fix, .width = 32,
					  .base = 16 };
		ASSERT_EQ_INT(reduce_emit(out, sizeof out,
					  BVN_UNIT_SI(bu_meter, si_kilo), q16, "30000"),
			      (int)error_value_out_of_range,
			      "30000 km in metres overflows q16/32 and is refused");
		value_type_spec_t q30 = { .family = vt_float_fix, .width = 32,
					  .base = 30 };
		ASSERT_EQ_INT(reduce_emit(out, sizeof out,
					  BVN_UNIT_SI(bu_meter, si_kilo), q30, "1"),
			      (int)error_value_out_of_range,
			      "1 km in metres needs 10 integer bits; q30/32 has 1");
		/* one that does fit still writes, rescaled */
		ASSERT_EQ_INT(reduce_emit(out, sizeof out,
					  BVN_UNIT_SI(bu_meter, si_kilo), q16, "1.5"),
			      0, "1.5 km fits q16/32 as 1500 m");
		ASSERT_TRUE(strcmp(out, ".d=1500 m") == 0, "...and is rescaled");
	}

	/* nan/inf carry no finite value, so no scale applies. */
	/* The unit goes on the annotation: a special number takes no inline
	 * suffix (spec section 6.4), and ".d=nan m" was never readable. */
	ASSERT_EQ_INT(reduce_emit_annotated(out, sizeof out,
				  BVN_UNIT_SI(bu_meter, si_kilo),
				  f64, "nan"), 0, "nan writes");
	ASSERT_TRUE(strcmp(out, ".d=<float:64,m>nan") == 0, "nan is not scaled");

	/* A scale that does not terminate in the value's OWN base cannot be written
	 * exactly — 1/100 in base 16 needs a factor of 5. Refuse rather than round. */
	value_type_spec_t f16 = { .family = vt_float, .width = 64, .base = 16 };
	ASSERT_EQ_INT(reduce_emit(out, sizeof out, BVN_UNIT_SI(bu_meter, si_centi),
				  f16, "5"),
		      (int)error_value_out_of_range,
		      "a non-terminating rescale is refused, not rounded");
	/* ...while one that does terminate goes through: 5 km = 5000 = 0x1388. */
	ASSERT_EQ_INT(reduce_emit(out, sizeof out, BVN_UNIT_SI(bu_meter, si_kilo),
				  f16, "5"), 0, "a terminating rescale writes in base 16");
	ASSERT_TRUE(strcmp(out, ".d=1388 m") == 0, "and in the value's own base");

	/* Without the flag nothing moves at all. */
	memset(out, 0, sizeof out);
	{
		bvnr_sink_t sink;
		bvnr_sink_to_mem(&sink, out, sizeof out);
		bvnr_writer_t *w = bvnr_writer_create();
		bvnr_write_flags_t wf;
		memset(&wf, 0, sizeof wf);
		ASSERT_TRUE(bvnr_open_write_sink(w, &sink, false, &wf), "plain writer opens");
		bvnr_data_t d;
		memset(&d, 0, sizeof d);
		bvnr_write_event(w, ev_stream_start, &d);
		d.type = token_is_identifier; d.data = "d"; d.length = 1;
		bvnr_write_event(w, ev_assignment_start, &d);
		memset(&d, 0, sizeof d);
		d.type = token_is_number; d.value_type = f64;
		d.value_unit = BVN_UNIT_SI(bu_meter, si_kilo);
		d.data = "5"; d.length = 1;
		bvnr_write_event(w, ev_data, &d);
		memset(&d, 0, sizeof d);
		bvnr_write_event(w, ev_stream_end, &d);
		bvnr_writer_destroy(w);
		ASSERT_TRUE(strcmp(out, ".d=5 k~m") == 0,
			    "without BVN_UNIT_REDUCE the value and unit are verbatim");
	}
}


/*
 * The annotation's unit must be reduced too. On the reader-driven path
 * (pretty-print, canonicalise) the unit arrives as a TOKEN carrying the text the
 * reader saw, and pushing it verbatim left the annotation unreduced while the
 * value was scaled against the reduced unit — writing "5 km" as "5000 km", and
 * multiplying again on every further pass.
 */
static bool reduce_canon_ev(void *u, bvnr_event_t e, bvnr_data_t *d)
{
	return bvnr_write_event((bvnr_writer_t *)u, e, d);
}

static int reduce_canon(const char *in, char *out, size_t cap)
{
	memset(out, 0, cap);
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, out, cap);
	bvnr_writer_t *w = bvnr_writer_create();
	if (!w) return -1;
	bvnr_write_flags_t wf;
	memset(&wf, 0, sizeof wf);
	wf.unit_flags = BVN_UNIT_REDUCE;
	if (!bvnr_open_write_sink(w, &sink, false, &wf)) {
		bvnr_writer_destroy(w); return -1;
	}
	bvnr_reader_t *r = bvnr_reader_create();
	bvnr_read_flags_t rf;
	memset(&rf, 0, sizeof rf);
	rf.userdata = w;
	rf.on_verified = reduce_canon_ev;
	bvnr_open_read_mem(r, in, (uint32_t)strlen(in), NULL, 0, &rf);
	bool ok = bvnr_read(r) && bvnr_write_finish(w);
	bvnr_reader_destroy(r);
	bvnr_writer_destroy(w);
	return ok ? 0 : -1;
}

static void test_unwritable_unit_reports_a_unit_error(void)
{
	printf("  a unit with no spelling reports a UNIT error, not a stale one...\n");
	/* Both value-side sites returned false WITHOUT setting ser_error, so the
	 * writer reported whatever code was already there — a unit that cannot be
	 * written surfaced as "sink_buffer_exhausted", sending the caller after a
	 * buffer that was never involved. A bu_none component past the bare
	 * "no_unit" shapes has no spelling, which is the reachable trigger. */
	char out[512];
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, out, sizeof out);
	bvnr_writer_t *w = bvnr_writer_create();
	bvnr_write_flags_t wf;
	memset(&wf, 0, sizeof wf);
	bvnr_open_write_sink(w, &sink, false, &wf);
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	bvnr_write_event(w, ev_stream_start, &d);
	d.type = token_is_identifier; d.data = "d"; d.length = 1;
	bvnr_write_event(w, ev_assignment_start, &d);
	memset(&d, 0, sizeof d);
	d.type = token_is_number;
	d.value_type.family = vt_float;
	d.value_type.width  = 64;
	d.value_unit.num_components = 2;
	d.value_unit.components[0].base          = bu_meter;
	d.value_unit.components[0].exponent      = exp_linear;
	d.value_unit.components[0].prefix.system = prefix_si;
	d.value_unit.components[0].prefix.id.si  = si_none;
	d.value_unit.components[1].base          = bu_none;   /* no spelling */
	d.value_unit.components[1].exponent      = exp_linear;
	d.value_unit.components[1].prefix.system = prefix_si;
	d.value_unit.components[1].prefix.id.si  = si_kilo;
	d.data = "1.5"; d.length = 3;
	bvnr_write_event(w, ev_data, &d);
	ASSERT_EQ_INT((int)bvnr_writer_get_error(w), (int)error_unit_illegal,
		      "the writer names the unit as the problem");
	bvnr_writer_destroy(w);
}

static void test_write_type_annotation_names_the_real_failure(void)
{
	printf("  a unit REDUCE cannot express is illegal, not too long...\n");
	/* bvn_unit_to_string_ex returns -1 for several reasons, and this helper
	 * reported every one of them as error_unit_too_long -- sending the caller
	 * after a buffer that was never the problem. Under BVN_UNIT_REDUCE, a
	 * product whose summed exponent leaves [BVN_EXPONENT_MIN, BVN_EXPONENT_MAX]
	 * cannot be reduced without dropping a component, so it is refused; the
	 * diagnostic has to say which refusal it was.
	 *
	 * m^100*m^100 is m^200, past the range. It used to be m^9*m^2 -- the cap
	 * was 9 long after the exponent range had grown to 100, so the case that
	 * exercised this diagnostic was one the library should have handled. */
	struct { const char *unit; bvn_unit_flags_t flags; error_code_t want; } cases[] = {
		{ "m^100" "\xc2\xb7" "m^100", BVN_UNIT_REDUCE,     error_unit_illegal },
		{ "m^100" "\xc2\xb7" "m^100", BVN_UNIT_FLAGS_NONE, error_none },
		{ "m\xe2\x81\xb9" "\xc2\xb7" "m\xc2\xb2", BVN_UNIT_REDUCE, error_none },
		{ "m/s",                                     BVN_UNIT_REDUCE, error_none },
	};
	for (size_t i = 0; i < sizeof cases / sizeof cases[0]; i++) {
		char out[512];
		bvnr_sink_t sink;
		bvnr_sink_to_mem(&sink, out, sizeof out);
		bvnr_writer_t *w = bvnr_writer_create();
		bvnr_write_flags_t wf;
		memset(&wf, 0, sizeof wf);
		wf.unit_flags = cases[i].flags;
		bvnr_open_write_sink(w, &sink, false, &wf);
		bvnr_data_t d;
		memset(&d, 0, sizeof d);
		bvnr_write_event(w, ev_stream_start, &d);
		d.type = token_is_identifier; d.data = "k"; d.length = 1;
		bvnr_write_event(w, ev_assignment_start, &d);
		bool pok = false;
		value_unit_t vu = bvn_parse_unit((const uint8_t *)cases[i].unit, &pok);
		ASSERT_TRUE(pok, "the test unit parses");
		value_type_spec_t vt;
		memset(&vt, 0, sizeof vt);
		vt.family = vt_float; vt.width = 64;
		bvnr_write_type_annotation(w, vt, vu);
		ASSERT_EQ_INT((int)bvnr_writer_get_error(w), (int)cases[i].want,
			      "the annotation writer names the real failure");
		bvnr_writer_destroy(w);
	}
}

static void test_unit_reduce_annotation_matches_the_value(void)
{
	char p1[512], p2[512], p3[512];
	/* annotation-carried unit */
	ASSERT_EQ_INT(reduce_canon(".d = <float:64,k~m> 5;", p1, sizeof p1), 0,
		      "canonicalising with REDUCE succeeds");
	ASSERT_TRUE(strcmp(p1, ".d=<float:64,m>5000;") == 0,
		    "the annotation is reduced along with the value");
	/* and it must be stable: a second pass changed the value again before */
	ASSERT_EQ_INT(reduce_canon(p1, p2, sizeof p2), 0, "second pass succeeds");
	ASSERT_TRUE(strcmp(p1, p2) == 0, "REDUCE canonicalisation is idempotent");
	ASSERT_EQ_INT(reduce_canon(p2, p3, sizeof p3), 0, "third pass succeeds");
	ASSERT_TRUE(strcmp(p2, p3) == 0, "...and stays idempotent");

	/* the inline-unit spelling must agree with the annotation one */
	ASSERT_EQ_INT(reduce_canon(".d = <float:64> 5 k~m;", p1, sizeof p1), 0,
		      "inline-unit canonicalisation succeeds");
	ASSERT_TRUE(strcmp(p1, ".d=<float:64>5000 m;") == 0,
		    "an inline unit reduces and scales the same way");
	ASSERT_EQ_INT(reduce_canon(p1, p2, sizeof p2), 0, "second pass succeeds");
	ASSERT_TRUE(strcmp(p1, p2) == 0, "...and is idempotent too");
}


/*
 * Two writer defects from the 2026-11-03 review.
 *
 * bvnr_write_bvnf with width 0 takes the annotation width from f->_prec; a
 * bvn_float_t of 53 bits is no float width, and the failed call left ".c=" in
 * the output because the key went out before the width was checked.
 *
 * A key longer than 255 bytes was refused although a reader with
 * max_identifier_length raised reads it back: 255 is the default, 65535 the
 * ceiling.
 */
static void test_writer_review_2026_11(void)
{
	char out[200000];
	bvn_float_t *f = bvn_float_alloc(53);
	ASSERT_TRUE(f != NULL, "bvn_float_alloc");
	if (!f) return;
	bvn_float_from_str(f, "1.5", 10);

	bvnr_writer_t *w = bvnr_writer_create();
	bvnr_open_write_mem(w, out, sizeof out, false, NULL);
	ASSERT_TRUE(bvnr_write_bvnf(w, "a", f, 64), "width 64 is written");
	uint64_t before = bvnr_writer_bytes_written(w);
	ASSERT_FALSE(bvnr_write_bvnf(w, "c", f, 0),
		     "width 0 with a 53-bit float is refused");
	ASSERT_EQ_INT((int)bvnr_writer_get_error(w), (int)error_illegal_value_type,
		      "...as error_illegal_value_type");
	ASSERT_EQ_UINT(bvnr_writer_bytes_written(w), before,
		       "...and the refused call wrote nothing, not even its key");
	bvnr_writer_destroy(w);
	bvn_float_free(f);

	/* A 65535-byte key: written, and read back by a reader that allows it. */
	static char key[65536];
	memset(key, 'k', 65535);
	key[65535] = '\0';
	w = bvnr_writer_create();
	bvnr_open_write_mem(w, out, sizeof out, false, NULL);
	ASSERT_TRUE(bvnr_write_uint(w, key, 0, 7), "a 65535-byte key is written");
	ASSERT_TRUE(bvnr_write_finish(w), "and the document finishes");
	uint64_t n = bvnr_writer_bytes_written(w);
	bvnr_writer_destroy(w);

	bvnr_read_flags_t rf;
	memset(&rf, 0, sizeof rf);
	rf.max_identifier_length = 65535;
	bvnr_reader_t *r = bvnr_reader_create();
	bvnr_open_read_mem(r, out, n, NULL, 0, &rf);
	ASSERT_TRUE(bvnr_read(r), "a reader with the limit raised reads it back");
	bvnr_reader_destroy(r);
}

static void test_writer_refuses_unreadable_output(void)
{
	/* The writer's contract is that it "cannot be coaxed into emitting a stream
	 * the reader would reject". These three ways of doing exactly that reported
	 * success instead. */
	char out[200000];
	static char big[70000];
	memset(big, 'a', sizeof big);

	/* Payloads past the reader's FIXED caps: max_identifier_length,
	 * max_string_length and max_symbol_length are uint16_t, so no reader
	 * configuration can accept more than 65535 bytes. (255 is the identifier
	 * DEFAULT; a reader with the limit raised reads a longer key.) */
	struct { const char *lbl; token_type_t tt; uint32_t len; int want; } lim[] = {
		{ "identifier 256", token_is_identifier, 256,   0 },
		{ "identifier 65535", token_is_identifier, 65535, 0 },
		{ "identifier 65536", token_is_identifier, 65536, (int)error_identifier_too_long },
		{ "string 65535",   token_is_string,     65535, 0 },
		{ "string 65536",   token_is_string,     65536, (int)error_string_too_long },
		{ "symbol 65536",   token_is_symbol,     65536, (int)error_symbol_too_long },
	};
	for (size_t i = 0; i < sizeof lim / sizeof lim[0]; i++) {
		bvnr_sink_t sink;
		bvnr_sink_to_mem(&sink, out, sizeof out);
		bvnr_writer_t *w = bvnr_writer_create();
		bvnr_open_write_sink(w, &sink, false, NULL);
		bvnr_data_t d;
		memset(&d, 0, sizeof d);
		bvnr_write_event(w, ev_stream_start, &d);
		if (lim[i].tt == token_is_identifier) {
			d.type = lim[i].tt; d.data = big; d.length = lim[i].len;
			bvnr_write_event(w, ev_assignment_start, &d);
		} else {
			d.type = token_is_identifier; d.data = "k"; d.length = 1;
			bvnr_write_event(w, ev_assignment_start, &d);
			memset(&d, 0, sizeof d);
			d.type = lim[i].tt; d.data = big; d.length = lim[i].len;
			if (lim[i].tt == token_is_string)
				d.value_type.family = vt_utf8;
			bvnr_write_event(w, ev_data, &d);
		}
		ASSERT_EQ_INT((int)bvnr_writer_get_error(w), lim[i].want,
			      "a payload past the reader's fixed cap is refused");
		bvnr_writer_destroy(w);
	}

	/* A bare number whose digits the lexer cannot read. "5" and "1e3" lex fine
	 * in any base, so only genuinely unreadable literals are rejected. */
	static const struct { const char *lit; uint32_t base; int want; } num[] = {
		{ "18F",  16, (int)error_base_requires_string_literal },
		{ "18F",  36, (int)error_base_requires_string_literal },
		{ "1.8p+2", 16, (int)error_base_requires_string_literal },
		{ "5",    16, 0 },
		{ "1e3",  16, 0 },
		{ "255",  10, 0 },
		{ "inf",  16, 0 },
	};
	for (size_t i = 0; i < sizeof num / sizeof num[0]; i++) {
		bvnr_sink_t sink;
		bvnr_sink_to_mem(&sink, out, sizeof out);
		bvnr_writer_t *w = bvnr_writer_create();
		bvnr_open_write_sink(w, &sink, false, NULL);
		bvnr_data_t d;
		memset(&d, 0, sizeof d);
		bvnr_write_event(w, ev_stream_start, &d);
		d.type = token_is_identifier; d.data = "k"; d.length = 1;
		bvnr_write_event(w, ev_assignment_start, &d);
		memset(&d, 0, sizeof d);
		d.type = token_is_number;
		d.value_type = (value_type_spec_t){ .family = vt_float, .width = 64,
						    .base = num[i].base };
		if (num[i].base != 16 && num[i].base != 10)
			d.value_type.family = vt_uint;
		d.data = num[i].lit; d.length = (uint32_t)strlen(num[i].lit);
		bvnr_write_event(w, ev_data, &d);
		ASSERT_EQ_INT((int)bvnr_writer_get_error(w), num[i].want,
			      "a bare literal is refused iff the lexer cannot read it");
		bvnr_writer_destroy(w);
	}

	/* A writer whose open failed has no sink. Events still "succeed" because
	 * they only fill the internal buffer, so checking their return value does
	 * not save the caller — the first flush used to jump through a NULL
	 * function pointer. */
	{
		bvnr_writer_t *w = bvnr_writer_create();
		ASSERT_FALSE(bvnr_open_write_mem(w, NULL, 0, false, NULL),
			     "opening with no buffer fails");
		bvnr_data_t d;
		memset(&d, 0, sizeof d);
		bvnr_write_event(w, ev_stream_start, &d);
		d.type = token_is_identifier; d.data = "key"; d.length = 3;
		bvnr_write_event(w, ev_assignment_start, &d);
		memset(&d, 0, sizeof d);
		d.type = token_is_number;
		d.value_type = (value_type_spec_t){ .family = vt_uint, .width = 64 };
		d.data = "42"; d.length = 2;
		bvnr_write_event(w, ev_data, &d);
		ASSERT_FALSE(bvnr_write_finish(w), "finishing a sinkless writer fails");
		ASSERT_EQ_INT((int)bvnr_writer_get_error(w), (int)error_writing_to_sink,
			      "...and says why, rather than crashing");
		bvnr_writer_destroy(w);
	}
}


/*
 * The writer's stated contract is that it "cannot be coaxed into emitting a
 * stream the reader would reject". Before the event-ordering gate it enforced
 * only struct balance and the depth caps, so a value bare at the top level, two
 * assignments in a row, a lone annotation_end and a dimension separator outside
 * an array all produced bytes and reported success.
 *
 * The property test below is the real check: fuzz event sequences and require
 * that everything the writer ACCEPTS re-parses. The named cases here document
 * the specific shapes and, unlike the fuzz, name what each one used to emit.
 */
static uint64_t gram_seq(const bvnr_event_t *evs, const token_type_t *toks,
			 uint32_t n, char *out, size_t cap, bool *accepted)
{
	memset(out, 0, cap);
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, out, cap);
	bvnr_writer_t *w = bvnr_writer_create();
	if (!w) { *accepted = false; return 0; }
	bvnr_write_flags_t wf;
	memset(&wf, 0, sizeof wf);
	bvnr_open_write_sink(w, &sink, false, &wf);
	bool ok = true;
	for (uint32_t i = 0; i < n && ok; i++) {
		bvnr_data_t d;
		memset(&d, 0, sizeof d);
		d.type = toks[i];
		d.value_type.family = vt_uint;
		d.value_type.width  = 64;
		d.data = "1"; d.length = 1;
		if (toks[i] == token_is_identifier) { d.data = "k"; d.length = 1; }
		ok = bvnr_write_event(w, evs[i], &d);
	}
	if (ok) ok = bvnr_write_finish(w);
	uint64_t nbytes = bvnr_writer_bytes_written(w);
	bvnr_writer_destroy(w);
	*accepted = ok;
	return nbytes;
}

/*
 * The six unit-carrying writers nothing called.
 *
 * bvnr_write_uint_unit and bvnr_write_float_unit were exercised here and in
 * three other files. The other six -- signed integers, fixed point, decimal
 * floats, and the two arbitrary-precision families -- were public API with NO
 * caller anywhere in the tree: not a test, not the Python bindings, not the
 * fuzz harnesses. They happen to work; nothing said so, and nothing would have
 * said otherwise. These are precisely the families this format exists for, so
 * "it compiles" was the whole of the guarantee on the path that attaches a unit
 * to a 128-bit integer.
 *
 * Each is written, read back, and checked on the two things that can go wrong
 * independently: the VALUE survives, and the UNIT survives as the same
 * value_unit_t rather than as text that merely looks similar.
 */
static void test_every_unit_writer_round_trips(void)
{
	printf("  test_every_unit_writer_round_trips...\n");

	/* One case per entry point: a key, the unit to attach, and the unit as the
	 * reader should hand it back. */
	struct {
		const char *key;
		const char *unit;
	} want[] = {
		{ "si",  "m/s"  },   /* bvnr_write_sint_unit       */
		{ "fx",  "k~g"  },   /* bvnr_write_float_fix_unit  */
		{ "dc",  "\xc2\xb0" "C" },  /* bvnr_write_float_dec_unit  (°C) */
		{ "bf",  "Pa"   },   /* bvnr_write_bvnf_unit       */
		{ "bfb", "Pa"   },   /* bvnr_write_bvnf_base_unit  */
		{ "bi",  "$USD" },   /* bvnr_write_bvni_unit       */
	};

	for (size_t i = 0; i < sizeof want / sizeof want[0]; i++) {
		uint8_t output[4096];
		bvnr_sink_t sink;
		bvnr_writer_t *w = make_writer(output, sizeof(output), &sink);
		ASSERT_NOT_NULL(w, "make_writer must succeed");
		if (!w) return;

		bool uok = true;
		value_unit_t u = bvn_parse_unit((const uint8_t *)want[i].unit, &uok);
		ASSERT_TRUE(uok, "the test's own unit string must parse");

		bool wrote = false;
		bvn_float_t *f = NULL;
		bvn_int_t   *n = NULL;
		switch (i) {
		case 0:
			wrote = bvnr_write_sint_unit(w, want[i].key, 32, -5, u);
			break;
		case 1:
			wrote = bvnr_write_float_fix_unit(w, want[i].key, 32, 16, 1.5, u);
			break;
		case 2:
			wrote = bvnr_write_float_dec_unit(w, want[i].key, 64, 2.5, u);
			break;
		case 3:
			f = bvn_float_alloc(64);
			wrote = f && bvn_float_from_double(f, 3.25) &&
			        bvnr_write_bvnf_unit(w, want[i].key, f, 64, u);
			break;
		case 4:
			f = bvn_float_alloc(64);
			wrote = f && bvn_float_from_double(f, 3.25) &&
			        bvnr_write_bvnf_base_unit(w, want[i].key, f, 64, 16, u);
			break;
		case 5:
			n = bvn_int_alloc();
			/* Wider than any machine integer, which is the point of the
			 * entry point: a currency amount in minor units at 128 bits. */
			wrote = n &&
			        bvn_int_from_str(n, "123456789012345678901234567890", 10) &&
			        bvnr_write_bvni_unit(w, want[i].key, n, 128, 10, u);
			break;
		default:
			break;
		}
		bvn_float_free(f);
		bvn_int_free(n);

		ASSERT_TRUE(wrote, "the unit-carrying writer must succeed");
		ASSERT_TRUE(bvnr_write_finish(w), "finish must succeed");
		ASSERT_EQ_INT(bvnr_writer_get_error(w), error_none, "no writer error");

		last_event_t le = {0};
		ASSERT_TRUE(roundtrip(output, bvnr_writer_bytes_written(w), &le),
		            "what it wrote must read back");
		ASSERT_TRUE(strcmp(le.key, want[i].key) == 0, "the key survives");
		ASSERT_TRUE(le.value_len > 0, "the value survives");
		/* The unit is compared as a UNIT, not as text: a writer that emitted a
		 * different spelling of the same unit is fine, and one that emitted a
		 * similar spelling of a different unit is not. */
		ASSERT_TRUE(bvn_unit_equal(le.unit, u),
		            "the unit survives as the same unit");
		bvnr_writer_destroy(w);
	}
}

static void test_writer_event_grammar(void)
{
	char out[4096];
	bool accepted;

	/* Each of these produced output and reported success. */
	{
		bvnr_event_t e1[] = { ev_data };
		token_type_t t1[] = { token_is_number };
		gram_seq(e1, t1, 1, out, sizeof out, &accepted);
		ASSERT_FALSE(accepted, "a value with no assignment is refused (was \"1;\")");
	}
	{
		bvnr_event_t e[] = { ev_array_row_end };
		token_type_t t[] = { token_is_number };
		gram_seq(e, t, 1, out, sizeof out, &accepted);
		ASSERT_FALSE(accepted, "closing a row that never opened is refused");
	}
	{
		bvnr_event_t e[] = { ev_type_annotation_end };
		token_type_t t[] = { token_is_type };
		gram_seq(e, t, 1, out, sizeof out, &accepted);
		ASSERT_FALSE(accepted, "a lone annotation end is refused (was \">\")");
	}
	{
		bvnr_event_t e[] = { ev_assignment_start, ev_assignment_start };
		token_type_t t[] = { token_is_identifier, token_is_identifier };
		gram_seq(e, t, 2, out, sizeof out, &accepted);
		ASSERT_FALSE(accepted, "two assignments in a row are refused (was \".k=.k=\")");
	}
	{
		bvnr_event_t e[] = { ev_array_dim_start };
		token_type_t t[] = { token_is_number };
		gram_seq(e, t, 1, out, sizeof out, &accepted);
		ASSERT_FALSE(accepted, "a dimension outside an array is refused (was \"/\")");
	}
	{
		bvnr_event_t e[] = { ev_assignment_start, ev_struct_start, ev_data };
		token_type_t t[] = { token_is_identifier, token_is_type, token_is_number };
		gram_seq(e, t, 3, out, sizeof out, &accepted);
		ASSERT_FALSE(accepted, "a keyless value inside a struct is refused");
	}
	{   /* an assignment left dangling is as incomplete as an unclosed struct */
		bvnr_event_t e[] = { ev_assignment_start };
		token_type_t t[] = { token_is_identifier };
		gram_seq(e, t, 1, out, sizeof out, &accepted);
		ASSERT_FALSE(accepted, "finishing with a dangling assignment is refused");
	}

	/* ...while every legal shape still writes. A null value is a value: the
	 * caller sends it explicitly rather than leaving the assignment open. */
	{
		bvnr_event_t e[] = { ev_assignment_start, ev_data };
		token_type_t t[] = { token_is_identifier, token_is_null_value };
		gram_seq(e, t, 2, out, sizeof out, &accepted);
		ASSERT_TRUE(accepted, "an explicit null value still writes");
	}
	{   /* .k = [1]/[1]; -- the row separator between two closed rows */
		bvnr_event_t e[] = { ev_assignment_start, ev_array_row_start, ev_data,
				     ev_array_row_end, ev_array_dim_start,
				     ev_array_row_start, ev_data, ev_array_row_end };
		token_type_t t[] = { token_is_identifier, token_is_number,
				     token_is_array_number, token_is_number,
				     token_is_number, token_is_number,
				     token_is_array_number, token_is_number };
		gram_seq(e, t, 8, out, sizeof out, &accepted);
		ASSERT_TRUE(accepted, "a multi-row array still writes");
	}
	{   /* .k = {.k = 1;}; -- a key inside a struct inside nothing */
		bvnr_event_t e[] = { ev_assignment_start, ev_struct_start,
				     ev_assignment_start, ev_data, ev_struct_end };
		token_type_t t[] = { token_is_identifier, token_is_type,
				     token_is_identifier, token_is_number, token_is_type };
		gram_seq(e, t, 5, out, sizeof out, &accepted);
		ASSERT_TRUE(accepted, "a nested struct still writes");
	}
}

/*
 * The property the named cases above only sample: anything the writer accepts
 * in full must re-parse. This is what makes the contract testable rather than
 * aspirational -- it fails on any hole, not just the ones someone listed.
 */
static void test_writer_accepts_only_readable_streams(void)
{
	static const bvnr_event_t EVS[] = {
		ev_stream_start, ev_assignment_start, ev_octet_stream_start,
		ev_octet_stream_end, ev_struct_start, ev_struct_end,
		ev_array_row_start, ev_array_row_end, ev_array_dim_start, ev_data,
		ev_type_annotation_start, ev_type_annotation_end,
		ev_type_annotation_type_family,
		ev_type_annotation_type_family_parameter, ev_stream_end,
	};
	static const token_type_t TOKS[] = {
		token_is_identifier, token_is_string, token_is_number, token_is_symbol,
		token_is_reference, token_is_array_number, token_is_array_string,
		token_is_type, token_is_octet_stream, token_is_null_value,
		token_is_unit, token_is_type_width, token_is_type_base, token_is_bool,
	};
	static const char *LITS[] = { "1", "k", "42", "true", "abc", "1.5" };
	uint64_t rs = 0x9E3779B97F4A7C15ull;
	char out[8192];
	uint32_t accepted = 0, unreadable = 0;

	for (uint32_t it = 0; it < 40000u; it++) {
#define GR_RND(n) (rs = rs * 6364136223846793005ull + 1442695040888963407ull, \
		   (uint32_t)((rs >> 33) % (n)))
		bvnr_sink_t sink;
		memset(out, 0, sizeof out);
		bvnr_sink_to_mem(&sink, out, sizeof out);
		bvnr_writer_t *w = bvnr_writer_create();
		if (!w) continue;
		bvnr_write_flags_t wf;
		memset(&wf, 0, sizeof wf);
		if (!bvnr_open_write_sink(w, &sink, false, &wf)) {
			bvnr_writer_destroy(w); continue;
		}
		uint32_t n = 1u + GR_RND(9u);
		bool ok = true;
		for (uint32_t i = 0; i < n && ok; i++) {
			bvnr_data_t d;
			memset(&d, 0, sizeof d);
			d.type = TOKS[GR_RND(sizeof TOKS / sizeof TOKS[0])];
			d.value_type.family = (value_type_family_t)GR_RND(9u);
			d.value_type.width  = 64u;
			const char *lit = LITS[GR_RND(6u)];
			d.data = lit; d.length = (uint32_t)strlen(lit);
			ok = bvnr_write_event(w, EVS[GR_RND(sizeof EVS / sizeof EVS[0])], &d);
		}
		if (ok) ok = bvnr_write_finish(w);
		/* the real byte count: an octet stream embeds NULs, so strlen lies */
		uint64_t nbytes = bvnr_writer_bytes_written(w);
		bvnr_writer_destroy(w);
		if (!ok) continue;
		accepted++;
		bvnr_reader_t *r = bvnr_reader_create();
		bvnr_read_flags_t rf;
		memset(&rf, 0, sizeof rf);
		bvnr_open_read_mem(r, out, nbytes, NULL, 0, &rf);
		if (!bvnr_read(r)) unreadable++;
		bvnr_reader_destroy(r);
#undef GR_RND
	}
	ASSERT_TRUE(accepted > 200,
		    "the fuzz actually accepted a meaningful number of streams");
	ASSERT_EQ_INT((int64_t)unreadable, 0,
		      "every stream the writer accepts is one the reader can read");
}

int main(void)
{
	test_unit_reduce_rescales_the_value();
	test_unit_reduce_rescales_a_quoted_literal();
	test_unit_reduce_annotation_matches_the_value();
	test_write_type_annotation_names_the_real_failure();
	test_unwritable_unit_reports_a_unit_error();
	test_writer_refuses_unreadable_output();
	test_writer_review_2026_11();
	test_every_unit_writer_round_trips();
	test_writer_event_grammar();
	test_writer_accepts_only_readable_streams();
	printf("Running bovnar_writer_test regression suite...\n");

	test_write_version_directive();
	test_write_datetime();
	test_write_datetime_fraction_guards();

	test_write_simple_strings();
	test_write_typed_integers();
	test_write_float_values();
	test_write_float_width_over_64_rejected();
	test_write_float_fix_roundtrip();
	test_write_float_fix_q_edges();
	test_write_float_dec_roundtrip();
	test_write_bool_and_null();
	test_write_with_units();
	test_write_struct();
	test_write_nested_struct();
	test_write_type_annotation_direct();
	test_write_keyword_symbol_rejected();
	test_write_utf8_width_rejected();
	test_write_buffer_exhaustion();
	test_write_multiple_assignments();
	test_write_sint_negative();
	test_write_bvnf_base10();
	test_write_bvnf_base16();
	test_write_bvni_decimal();
	test_write_bvni_width0_is_64();
	test_value_is_held_to_its_annotation();
	test_unstated_integer_width_is_64();
	test_reduce_rescales_whatever_the_event_type();
	test_reduce_float_edges();
	test_reduce_prefix_on_any_base();
	test_no_inline_unit_on_special_or_datetime();
	test_writer_policy_checks_the_written_unit();
	test_dec_and_fix_are_not_narrowed_through_float();
	test_multi_chunk_octet_stream_in_array();
	test_reference_and_quoted_keyword_digits();
	test_write_bvni_hex();
	test_write_bvni_base64_signchar();
	test_write_streaming_flush();
	test_validate_reference();
	test_write_reference_roundtrip();
	test_write_reference_without_dot_rejected();
	test_write_reference_dot_only_rejected();
	test_write_string_vf_escapes();
	test_canon_observer_unbalanced_struct_end_rejected();
	test_canon_observer_emits_version();

	if (failures == 0) {
		printf("PASSED %d tests\n", tests);
		return 0;
	}

	fprintf(stderr, "FAILED %d of %d tests\n", failures, tests);
	return 1;
}
