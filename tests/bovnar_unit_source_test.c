/*
 * SPDX-License-Identifier: MIT
 * SPDX-FileComment: The test vectors below quote UCUM atom codes. Those
 * identifiers are Regenstrief Institute's and are not covered by the MIT grant
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
 * bvnr_data_t.unit_source (library 2.0): the unit as the producer spelled it.
 * Specified in doc/11_bovnar_unit_profiles.md §5.2 and §7.3.
 *
 * Three things are held here. That the READER hands over the exact bytes and
 * keeps them attached to the right unit through arrays and error recovery.
 * That the WRITER writes them back -- and refuses every spelling that would
 * make the document say something other than value_unit. And that the DOM
 * keeps the spelling exactly where it is not recoverable from the unit.
 */
#include <stdio.h>
#include <string.h>
#include "bovnar.h"
#include "bovnar_dom.h"

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
	bool ok = false;
	value_unit_t u = bvn_parse_unit((const uint8_t*)s, &ok);
	if (!ok) {
		fprintf(stderr, "FAIL: test unit %s did not parse\n", s);
		failures++;
	}
	return u;
}

/* ── reader ─────────────────────────────────────────────────────────── */

#define MAX_SEEN 32
typedef struct {
	int   n;
	char  src[MAX_SEEN][BVNR_UNIT_SOURCE_MAX + 1];
	bool  has[MAX_SEEN];
	bool  hook_saw_source;
	int   params;
	char  param_src[MAX_SEEN][BVNR_UNIT_SOURCE_MAX + 1];
} seen_t;

static bool collect(void* ud, bvnr_event_t ev, bvnr_data_t* d)
{
	seen_t* s = ud;
	if (ev == ev_type_annotation_type_family_parameter &&
	    d->type == token_is_unit && d->unit_source && s->params < MAX_SEEN) {
		memcpy(s->param_src[s->params], d->unit_source, d->unit_source_length);
		s->param_src[s->params][d->unit_source_length] = 0;
		s->params++;
	}
	if (ev != ev_data || s->n >= MAX_SEEN) return true;
	s->has[s->n] = d->unit_source != NULL;
	if (d->unit_source) {
		memcpy(s->src[s->n], d->unit_source, d->unit_source_length);
		s->src[s->n][d->unit_source_length] = 0;
	} else {
		s->src[s->n][0] = 0;
	}
	s->n++;
	return true;
}

static bool read_doc(const char* doc, seen_t* s, bvnr_read_flags_t* f)
{
	bvnr_reader_t* r = bvnr_reader_create();
	if (!r) return false;
	memset(s, 0, sizeof *s);
	f->userdata    = s;
	f->on_verified = collect;
	bool ok = bvnr_open_read_mem(r, doc, (uint64_t)strlen(doc), NULL, 0, f) &&
	          bvnr_read(r);
	bvnr_reader_destroy(r);
	return ok;
}

static void expect_sources(const char* doc, const char* const* want, int n)
{
	seen_t s;
	bvnr_read_flags_t f;
	memset(&f, 0, sizeof f);
	ASSERT_TRUE(read_doc(doc, &s, &f), doc);
	ASSERT_TRUE(s.n == n, "the number of values read");
	for (int i = 0; i < n && i < s.n; i++) {
		tests++;
		bool ok = want[i] ? (s.has[i] && strcmp(s.src[i], want[i]) == 0)
		                  : !s.has[i];
		if (!ok) {
			fprintf(stderr, "FAIL: value %d of %s: got %s, want %s\n", i, doc,
			        s.has[i] ? s.src[i] : "(null)", want[i] ? want[i] : "(null)");
			failures++;
		}
	}
}

static void test_reader_scalars(void)
{
	printf("  the reader hands over the exact spelling...\n");
	static const char doc[] =
		"#!bovnar 1.2\n"
		".a = <float:64,ucum:mL{total}> 1.0;\n"       /* an annotation's text  */
		".b = 2.0 ucum:{RBC}/uL;\n"                   /* an inline suffix      */
		".c = <float:64,mmHg> 1.0 ucum:mm[Hg];\n"     /* both: annotation wins */
		".d = <float:64> 1.0 m;\n"                    /* unitless annotation   */
		".e = <float:64,udunits:kg m-2 s-1> 1.0;\n"   /* interior whitespace   */
		".f = <uint:8,km> 3;\n"                       /* compact, not k~m      */
		".g = \"x\" m;\n"                             /* a string's unit       */
		".h = 7;\n"                                   /* no unit: nothing      */
		".i = <float:64,no_unit> 1.0;\n"
		".j = <datetime:64,tai> 5;\n";                /* an epoch is no unit   */
	static const char* const want[] = {
		"ucum:mL{total}", "ucum:{RBC}/uL", "mmHg", "m", "udunits:kg m-2 s-1",
		"km", "m", NULL, NULL, NULL,
	};
	expect_sources(doc, want, 10);

	/* The annotation's own unit-parameter event carries it too, so a consumer
	 * of annotations and a consumer of values are told the same thing. */
	seen_t s;
	bvnr_read_flags_t f;
	memset(&f, 0, sizeof f);
	ASSERT_TRUE(read_doc("#!bovnar 1.2\n.a = <float:64,ucum:mL{total}> 1.0;",
	                     &s, &f), "annotation document");
	ASSERT_TRUE(s.params == 1 && strcmp(s.param_src[0], "ucum:mL{total}") == 0,
	            "the unit parameter event carries the spelling");
}

static void test_reader_arrays(void)
{
	printf("  ...and keeps it attached to the right unit in arrays...\n");
	/* An annotation inside an array holds for one element; the spelling has to
	 * go back to the array's own with the unit. */
	static const char* const w1[] = { "k~m", "ucum:mm[Hg]", "k~m" };
	expect_sources("#!bovnar 1.2\n"
	               ".a = <float:64,k~m> [1.0, <float:64,ucum:mm[Hg]> 3.0, 4.0];",
	               w1, 3);
	static const char* const w2[] = { "km", "km", "km", "km" };
	expect_sources(".a = <uint:8,km> [[1, 2], [3, 4]];", w2, 4);
	/* Nested: the inner array's annotation ends with the inner array. */
	static const char* const w3[] = { "m", "s", "s", "m" };
	expect_sources(".a = <uint:8,m> [1, <uint:8,s> [2, 3], 4];", w3, 4);
	/* An element annotation WITHOUT a unit holds for one element too: the
	 * array's unit comes back after it. It used not to -- the 3 below was
	 * delivered with no unit at all. */
	static const char* const w5[] = { NULL, "s", "s" };
	expect_sources(".a = <float:64,s> [<float:64> 1, 3, 4];", w5, 3);
	/* The value after an array starts clean. */
	static const char* const w4[] = { "m", NULL, "s" };
	expect_sources(".a = <uint:8,m> [1]; .b = 2; .c = 3 s;", w4, 3);
}

static void test_reader_resync(void)
{
	printf("  ...and through error recovery...\n");
	seen_t s;
	bvnr_read_flags_t f;
	memset(&f, 0, sizeof f);
	f.continue_on_error = true;
	(void)read_doc(".a = <uint:8,m> [1, @@ ;\n.b = 2 s;\n.c = 3;", &s, &f);
	bool b_ok = false, c_ok = false;
	for (int i = 0; i < s.n; i++) {
		if (s.has[i] && strcmp(s.src[i], "s") == 0) b_ok = true;
		if (i == s.n - 1 && !s.has[i])             c_ok = true;
	}
	ASSERT_TRUE(b_ok, "the value after a recovery has its own spelling");
	ASSERT_TRUE(c_ok, "and a unitless one after it has none");
}

static bool hook_km(void* ud, const bvnr_data_t* d, value_unit_t* want,
                    uint32_t* base)
{
	seen_t* s = ud;
	if (d->unit_source && d->unit_source_length == 1 &&
	    memcmp(d->unit_source, "m", 1) == 0)
		s->hook_saw_source = true;
	*want = U("m~m");
	*base = 0;
	return true;
}

static void test_reader_conversion(void)
{
	printf("  ...and a conversion leaves it alone...\n");
	seen_t s;
	bvnr_read_flags_t f;
	memset(&f, 0, sizeof f);
	f.want_unit = hook_km;
	ASSERT_TRUE(read_doc(".a = <float:64,m> 1.5;", &s, &f), "converted document");
	ASSERT_TRUE(s.hook_saw_source, "the want_unit hook sees the spelling");
	ASSERT_TRUE(s.n == 1 && s.has[0] && strcmp(s.src[0], "m") == 0,
	            "it stays the spelling of value_unit, not of conv.unit");
}

/* ── writer ─────────────────────────────────────────────────────────── */

static char wbuf[8192];

static bvnr_writer_t* open_writer(bool v12, bvn_unit_flags_t flags)
{
	bvnr_writer_t* w = bvnr_writer_create();
	if (!w) return NULL;
	bvnr_write_flags_t wf;
	memset(&wf, 0, sizeof wf);
	wf.unit_flags = flags;
	memset(wbuf, 0, sizeof wbuf);
	if (!bvnr_open_write_mem(w, wbuf, sizeof wbuf - 1, false, &wf) ||
	    (v12 && !bvnr_write_version(w, 1, 2))) {
		bvnr_writer_destroy(w);
		return NULL;
	}
	return w;
}

static void test_writer_one_shot(void)
{
	printf("  the helpers write the spelling back, once...\n");
	bvnr_writer_t* w = open_writer(true, BVN_UNIT_FLAGS_NONE);
	ASSERT_TRUE(w != NULL, "writer");
	if (!w) return;
	ASSERT_TRUE(bvnr_writer_set_unit_source(w, "ucum:mL{total}", 14),
	            "set_unit_source");
	ASSERT_TRUE(bvnr_write_float_unit(w, "a", 64, 1.5, U("m~L")), "annotated value");
	/* One shot: the next value gets the canonical form again. */
	ASSERT_TRUE(bvnr_write_float_unit(w, "b", 64, 2.5, U("m~L")), "next value");
	ASSERT_TRUE(bvnr_writer_set_unit_source(w, "km", 2), "set_unit_source");
	ASSERT_TRUE(bvnr_write_uint_unit(w, "c", 8, 3, U("k~m")), "integer helper");
	/* NULL clears a spelling that was set and not used. */
	ASSERT_TRUE(bvnr_writer_set_unit_source(w, "ucum:mL{total}", 14), "set");
	ASSERT_TRUE(bvnr_writer_set_unit_source(w, NULL, 0), "clear");
	ASSERT_TRUE(bvnr_write_float_unit(w, "d", 64, 4.5, U("m~L")), "after clear");
	ASSERT_TRUE(bvnr_write_finish(w), "finish");
	ASSERT_TRUE(strstr(wbuf, ".a=<float:64,ucum:mL{total}>1.5e+0;") != NULL, wbuf);
	ASSERT_TRUE(strstr(wbuf, ".b=<float:64,m~L>2.5e+0;") != NULL, wbuf);
	ASSERT_TRUE(strstr(wbuf, ".c=<uint:8,km>3;") != NULL, wbuf);
	ASSERT_TRUE(strstr(wbuf, ".d=<float:64,m~L>4.5e+0;") != NULL, wbuf);
	bvnr_writer_destroy(w);

	/* What was written reads back as the unit it was given with. */
	seen_t s;
	bvnr_read_flags_t f;
	memset(&f, 0, sizeof f);
	char copy[sizeof wbuf];
	memcpy(copy, wbuf, sizeof copy);
	ASSERT_TRUE(read_doc(copy, &s, &f), "the written document reads back");
	ASSERT_TRUE(s.n == 4 && strcmp(s.src[0], "ucum:mL{total}") == 0 &&
	            strcmp(s.src[1], "m~L") == 0 && strcmp(s.src[2], "km") == 0,
	            "and the reader returns the same spellings");

	/* Re-opening drops a pending spelling: it belonged to the old document. */
	w = open_writer(true, BVN_UNIT_FLAGS_NONE);
	if (!w) { failures++; return; }
	bvnr_writer_set_unit_source(w, "ucum:mL{total}", 14);
	bvnr_write_flags_t wf;
	memset(&wf, 0, sizeof wf);
	memset(wbuf, 0, sizeof wbuf);
	ASSERT_TRUE(bvnr_open_write_mem(w, wbuf, sizeof wbuf - 1, false, &wf), "re-open");
	ASSERT_TRUE(bvnr_write_float_unit(w, "a", 64, 1.5, U("m~L")) &&
	            bvnr_write_finish(w), "write after re-open");
	ASSERT_TRUE(strstr(wbuf, "<float:64,m~L>") != NULL, wbuf);
	bvnr_writer_destroy(w);
}

/* One helper write under a one-shot spelling; returns the writer's error. */
static error_code_t one_shot_error(bool v12, bvn_unit_flags_t flags,
                                   const char* src, const char* unit)
{
	bvnr_writer_t* w = open_writer(v12, flags);
	if (!w) return error_invalid_argument;
	error_code_t e = error_none;
	if (!bvnr_writer_set_unit_source(w, src, (uint32_t)strlen(src)) ||
	    !bvnr_write_float_unit(w, "a", 64, 1.5, U(unit)) ||
	    !bvnr_write_finish(w))
		e = bvnr_writer_get_error(w);
	bvnr_writer_destroy(w);
	return e;
}

static void test_writer_refusals(void)
{
	printf("  the writer refuses a spelling that says something else...\n");
	ASSERT_TRUE(one_shot_error(true, 0, "ucum:mL{total}", "m~L") == error_none,
	            "the control: a matching spelling is written");
	ASSERT_TRUE(one_shot_error(true, 0, "ucum:s", "m~L") == error_unit_mismatch,
	            "a different unit is error_unit_mismatch");
	ASSERT_TRUE(one_shot_error(true, 0, "k~L", "m~L") == error_unit_mismatch,
	            "a different prefix is a different unit");
	ASSERT_TRUE(one_shot_error(true, 0, "ucum:nonsense", "m~L")
	            == bvn_unit_error_code((const uint8_t*)"ucum:nonsense", 13),
	            "an unparseable spelling reports the parser's own code");
	ASSERT_TRUE(one_shot_error(true, 0, "nosuch:m", "m")
	            == error_unit_profile_unknown, "an unknown namespace says so");
	ASSERT_TRUE(one_shot_error(false, 0, "ucum:mL{total}", "m~L")
	            == error_unsupported_spec_version,
	            "profile notation needs the 1.2 directive");
	ASSERT_TRUE(one_shot_error(false, 0, "km", "k~m") == error_none,
	            "a native spelling needs no directive");

	/* Text that would escape the unit slot. None of these is a unit, but each
	 * is a document if pasted in unchecked -- the second is a whole extra
	 * assignment. */
	static const char* const escapes[] = {
		"m>1;.k=<uint:8,m", "m,_16", " m", "m ", "m;", "m#", "m\"", "m\n",
	};
	for (size_t i = 0; i < sizeof escapes / sizeof escapes[0]; i++)
		ASSERT_TRUE(one_shot_error(true, 0, escapes[i], "m") != error_none,
		            escapes[i]);

	/* Too long to be read back: refused where it is handed over. */
	char longsrc[BVNR_UNIT_SOURCE_MAX + 2];
	memset(longsrc, 'm', sizeof longsrc);
	bvnr_writer_t* w = open_writer(true, 0);
	if (!w) { failures++; return; }
	ASSERT_FALSE(bvnr_writer_set_unit_source(w, longsrc, sizeof longsrc),
	             "an over-long spelling is refused");
	ASSERT_TRUE(bvnr_writer_get_error(w) == error_unit_too_long,
	            "as error_unit_too_long");
	bvnr_writer_destroy(w);

	/* One shot means the NEXT value. A helper that writes no annotation does
	 * not quietly pass the spelling on to whichever later value has a unit. */
	w = open_writer(true, 0);
	if (!w) { failures++; return; }
	bvnr_writer_set_unit_source(w, "ucum:mL{total}", 14);
	ASSERT_TRUE(bvnr_write_string(w, "s", "x"), "a string takes no unit");
	ASSERT_FALSE(bvnr_write_float_unit(w, "v", 64, 1.5, U("m~L")),
	             "and the spelling does not travel on to the next value");
	ASSERT_TRUE(bvnr_writer_get_error(w) == error_unit_mismatch,
	            "it is reported as set for a value with no unit");
	bvnr_writer_destroy(w);

	/* ...nor is it dropped in silence when that value was the last one. */
	w = open_writer(true, 0);
	if (!w) { failures++; return; }
	bvnr_writer_set_unit_source(w, "km", 2);
	ASSERT_TRUE(bvnr_write_string(w, "s", "x"), "a string takes no unit");
	ASSERT_FALSE(bvnr_write_finish(w), "finishing with the spelling unused");
	ASSERT_TRUE(bvnr_writer_get_error(w) == error_unit_mismatch, "is the same mistake");
	bvnr_writer_destroy(w);

	/* The FIRST error is the one a caller finds. A bad key used to be
	 * overwritten by the one-shot's own complaint on the write after it. */
	w = open_writer(true, 0);
	if (!w) { failures++; return; }
	bvnr_writer_set_unit_source(w, "km", 2);
	ASSERT_FALSE(bvnr_write_float_unit(w, "bad key", 64, 1.5, U("k~m")), "bad key");
	ASSERT_FALSE(bvnr_write_float_unit(w, "a", 64, 1.5, U("k~m")), "sticky");
	ASSERT_TRUE(bvnr_writer_get_error(w) != error_unit_mismatch,
	            "the first error stands");
	bvnr_writer_destroy(w);

	/* A spelling for a value that carries no unit names the wrong value. */
	w = open_writer(true, 0);
	if (!w) { failures++; return; }
	bvnr_writer_set_unit_source(w, "m", 1);
	ASSERT_FALSE(bvnr_write_float(w, "a", 64, 1.5), "spelling without a unit");
	ASSERT_TRUE(bvnr_writer_get_error(w) == error_unit_mismatch,
	            "is error_unit_mismatch");
	bvnr_writer_destroy(w);
}

static void test_writer_flags(void)
{
	printf("  a unit flag asks for its own spelling and wins...\n");
	bvnr_writer_t* w = open_writer(false, BVN_UNIT_ASCII_EXP);
	if (!w) { failures++; return; }
	/* Profile notation with no directive would be refused -- were it used. */
	ASSERT_TRUE(bvnr_writer_set_unit_source(w, "ucum:m/s2", 9), "set");
	ASSERT_TRUE(bvnr_write_float_unit(w, "a", 64, 1.5, U("m/s²")) &&
	            bvnr_write_finish(w), "written under BVN_UNIT_ASCII_EXP");
	ASSERT_TRUE(strstr(wbuf, "<float:64,m/s^2>") != NULL, wbuf);
	bvnr_writer_destroy(w);
}

/* Drive bvnr_write_event directly: `.k = <float:64,UNIT> 1;` or `.k = 1 UNIT;` */
static error_code_t event_write_ex(bool annotated, const char* src,
                                   const char* unit, const char* data_text);
static error_code_t event_write(bool annotated, const char* src,
                                const char* unit, bool with_data_text)
{
	return event_write_ex(annotated, src, unit, with_data_text ? unit : NULL);
}
/* `src` may be NULL (no unit_source); `data_text` is the unit parameter's
 * `data`, which is what every caller before 2.0 filled. */
static error_code_t event_write_ex(bool annotated, const char* src,
                                   const char* unit, const char* data_text)
{
	bvnr_writer_t* w = open_writer(true, 0);
	if (!w) return error_invalid_argument;
	value_unit_t vu = U(unit);
	bvnr_data_t d;
	bool ok = true;
	memset(&d, 0, sizeof d);
	d.type = token_is_identifier; d.data = "k"; d.length = 1;
	ok = ok && bvnr_write_event(w, ev_assignment_start, &d);
	if (annotated) {
		memset(&d, 0, sizeof d);
		d.type = token_is_type;
		d.value_type = BVN_TYPE_FLOAT(64);
		d.value_unit = vu;
		ok = ok && bvnr_write_event(w, ev_type_annotation_start, &d);
		ok = ok && bvnr_write_event(w, ev_type_annotation_type_family, &d);
		d.type = token_is_type_width;
		ok = ok && bvnr_write_event(w, ev_type_annotation_type_family_parameter, &d);
		d.type = token_is_unit;
		if (data_text) { d.data = data_text; d.length = (uint32_t)strlen(data_text); }
		if (src) {
			d.unit_source = src;
			d.unit_source_length = (uint32_t)strlen(src);
		}
		ok = ok && bvnr_write_event(w, ev_type_annotation_type_family_parameter, &d);
		memset(&d, 0, sizeof d);
		d.type = token_is_type;
		ok = ok && bvnr_write_event(w, ev_type_annotation_end, &d);
	}
	memset(&d, 0, sizeof d);
	d.type = token_is_number;
	d.value_type = annotated ? BVN_TYPE_FLOAT(64) : BVN_TYPE_PLAIN;
	d.value_unit = vu;
	d.data = "1"; d.length = 1;
	if (src) {
		d.unit_source = src;
		d.unit_source_length = (uint32_t)strlen(src);
	}
	ok = ok && bvnr_write_event(w, ev_data, &d);
	ok = ok && bvnr_write_finish(w);
	error_code_t e = ok ? error_none : bvnr_writer_get_error(w);
	bvnr_writer_destroy(w);
	return e;
}

static void test_writer_events(void)
{
	printf("  bvnr_write_event honours the field in both unit slots...\n");
	ASSERT_TRUE(event_write(true, "ucum:mL{a,b}", "m~L", true) == error_none,
	            "annotation slot");
	ASSERT_TRUE(strstr(wbuf, ".k=<float:64,ucum:mL{a,b}>1;") != NULL, wbuf);
	/* unit_source alone, with no text in `data`, is enough. */
	ASSERT_TRUE(event_write(true, "ucum:mm[Hg]", "mmHg", false) == error_none,
	            "annotation slot, no data text");
	ASSERT_TRUE(strstr(wbuf, ".k=<float:64,ucum:mm[Hg]>1;") != NULL, wbuf);
	ASSERT_TRUE(event_write(false, "ucum:mm[Hg]", "mmHg", false) == error_none,
	            "inline slot");
	ASSERT_TRUE(strstr(wbuf, ".k=1 ucum:mm[Hg];") != NULL, wbuf);
	ASSERT_TRUE(event_write(false, "ucum:s", "mmHg", false) == error_unit_mismatch,
	            "inline slot, wrong unit");
	/* The two slots end in different places. A comma or a space is one
	 * parameter inside an annotation and no inline suffix at all, so the same
	 * spelling is written in one and refused in the other. */
	ASSERT_TRUE(event_write(false, "ucum:mL{a,b}", "m~L", false) == error_unit_illegal,
	            "a comma cannot be written inline");
	ASSERT_TRUE(event_write(true, "udunits:kg m-2 s-1", "k~g/m²/s", true) == error_none,
	            "a space-multiplied code is one annotation parameter");
	ASSERT_TRUE(event_write(false, "udunits:kg m-2 s-1", "k~g/m²/s", false)
	            == error_unit_illegal, "and not an inline suffix");
}

static bool replay_cb(void* ud, bvnr_event_t ev, bvnr_data_t* d);

static void test_writer_data_text(void)
{
	printf("  ...and the parameter's older `data` text is held to the same rule...\n");
	/* The serialiser writes `data` when there is no unit_source, so checking
	 * only the new field left the old one a way round every refusal above. */
	ASSERT_TRUE(event_write_ex(true, NULL, "m", "m") == error_none,
	            "the control: data naming the unit");
	ASSERT_TRUE(event_write_ex(true, NULL, "k~m", "km") == error_none,
	            "data may spell the unit another way");
	ASSERT_TRUE(strstr(wbuf, ".k=<float:64,km>1;") != NULL, wbuf);
	ASSERT_TRUE(event_write_ex(true, NULL, "m", "kg") == error_unit_mismatch,
	            "data naming a different unit");
	ASSERT_TRUE(event_write_ex(true, NULL, "m", "kg>1;.k=<uint:8,m") != error_none,
	            "data that would write a second assignment");
	ASSERT_TRUE(strstr(wbuf, ".k=<uint:8") == NULL, wbuf);
	ASSERT_TRUE(event_write_ex(true, "m", "m", "s") == error_unit_mismatch,
	            "data and unit_source disagreeing");
}

/* A 32-component unit whose compact spelling (km^-9*ms^-9*...) is some 210
 * bytes and whose canonical form (k~m⁻⁹·m~s⁻⁹·...) is some 370. */
static value_unit_t long_unit(char* spelling, size_t cap)
{
	static const char* const b[] = {
		"km","ms","kg","mA","mK","mmol","mcd","kN","kPa","kJ","kW","kV","kHz",
		"mL","mbar","keV","mlm","mlx","mrad","kWb","mT","mH","mF","mC","mS",
		"kBq","mGy","mSv","mkat","mW","mm","cm","dm","kL","MeV","kcal","MPa",
	};
	value_unit_t u = {0};
	int n = 0;
	spelling[0] = 0;
	for (size_t i = 0; i < sizeof b / sizeof b[0] && n < 32; i++) {
		char t[900];
		snprintf(t, sizeof t, "%s%s%s^-9", spelling, n ? "*" : "", b[i]);
		bool ok = false;
		value_unit_t v = bvn_parse_unit((const uint8_t*)t, &ok);
		if (ok && v.num_components == (uint32_t)n + 1u && strlen(t) < cap) {
			strcpy(spelling, t);
			u = v;
			n++;
		}
	}
	return u;
}

static void test_writer_long_units(void)
{
	printf("  ...a long unit is judged by what is WRITTEN, not by its other spelling...\n");
	char src[900], canon[BVNR_UNIT_STRING_MAX];
	value_unit_t u = long_unit(src, sizeof src);
	int32_t cn = bvn_unit_to_string(u, canon, sizeof canon);
	ASSERT_TRUE(u.num_components == 32u && strlen(src) < 240u && cn > 255,
	            "the test unit: 32 components, short spelling, long canonical form");

	/* The helper passes the canonical text in `data` beside the spelling. It
	 * is not written, so its length is not the document's concern. */
	bvnr_writer_t* w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(bvnr_writer_set_unit_source(w, src, (uint32_t)strlen(src)) &&
	            bvnr_write_float_unit(w, "a", 64, 1.5, u) && bvnr_write_finish(w),
	            "a spelling that fits is written though the canonical form would not");
	bvnr_writer_destroy(w);
	seen_t s;
	bvnr_read_flags_t f;
	memset(&f, 0, sizeof f);
	char copy[sizeof wbuf];
	memcpy(copy, wbuf, sizeof copy);
	ASSERT_TRUE(read_doc(copy, &s, &f) && s.n == 1 && strcmp(s.src[0], src) == 0,
	            "and reads back as written");

	/* Without the spelling the canonical form is what would go out, and it
	 * does not fit: refused, in the annotation and as an inline suffix. */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_FALSE(bvnr_write_float_unit(w, "a", 64, 1.5, u), "canonical form too long");
	ASSERT_TRUE(bvnr_writer_get_error(w) == error_unit_too_long ||
	            bvnr_writer_get_error(w) == error_type_too_long, "refused for its length");
	bvnr_writer_destroy(w);
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	d.type = token_is_identifier; d.data = "a"; d.length = 1;
	bool ok = bvnr_write_event(w, ev_assignment_start, &d);
	memset(&d, 0, sizeof d);
	d.type = token_is_number;
	d.value_unit = u;
	d.data = "1"; d.length = 1;
	ASSERT_TRUE(ok && !bvnr_write_event(w, ev_data, &d) &&
	            bvnr_writer_get_error(w) == error_unit_too_long,
	            "an inline suffix over the reader's cap is refused, not written");
	bvnr_writer_destroy(w);

	/* The probe must not be stricter than the document: <uint:...> leaves
	 * 250 bytes for the unit, and the probe used to allow 246. */
	char doc[400], unit[300];
	memcpy(unit, "ucum:mL{", 8);
	memset(unit + 8, 'x', 240);
	strcpy(unit + 248, "}");
	snprintf(doc, sizeof doc, "#!bovnar 1.2\n.a=<uint:%s>1;", unit);
	w = open_writer(true, 0);
	bvnr_reader_t* r = bvnr_reader_create();
	if (!w || !r) { failures++; return; }
	memset(&f, 0, sizeof f);
	f.userdata = w;
	f.on_verified = replay_cb;
	ASSERT_TRUE(bvnr_open_read_mem(r, doc, strlen(doc), NULL, 0, &f) &&
	            bvnr_read(r) && bvnr_write_finish(w),
	            "an annotation that fills the reader's buffer replays");
	ASSERT_TRUE(strcmp(wbuf, doc) == 0, "byte for byte");
	bvnr_reader_destroy(r);
	bvnr_writer_destroy(w);
}

static void test_writer_value_agrees(void)
{
	printf("  ...and a value agrees with the annotation it is written under...\n");
	/* The serialiser writes the annotation's unit and never looks at the
	 * value's: five metres under <float:64,k~g> went out as five kilograms. */
	bvnr_writer_t* w = open_writer(false, 0);
	if (!w) { failures++; return; }
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	d.type = token_is_identifier; d.data = "a"; d.length = 1;
	bool ok = bvnr_write_event(w, ev_assignment_start, &d);
	ok = ok && bvnr_write_type_annotation(w, BVN_TYPE_FLOAT(64), U("k~g"));
	memset(&d, 0, sizeof d);
	d.type = token_is_number;
	d.value_type = BVN_TYPE_FLOAT(64);
	d.value_unit = U("m");
	d.data = "5"; d.length = 1;
	ASSERT_TRUE(ok && !bvnr_write_event(w, ev_data, &d) &&
	            bvnr_writer_get_error(w) == error_unit_mismatch,
	            "a value in metres under an annotation in kilograms");
	bvnr_writer_destroy(w);

	/* The annotation's unit may be given on its START event alone; the
	 * parameter events need nothing but their type. */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	memset(&d, 0, sizeof d);
	d.type = token_is_identifier; d.data = "a"; d.length = 1;
	ok = bvnr_write_event(w, ev_assignment_start, &d);
	memset(&d, 0, sizeof d);
	d.type = token_is_type;
	d.value_type = BVN_TYPE_FLOAT(64);
	d.value_unit = U("m");
	ok = ok && bvnr_write_event(w, ev_type_annotation_start, &d);
	d.value_unit = (value_unit_t){0};
	ok = ok && bvnr_write_event(w, ev_type_annotation_type_family, &d);
	d.type = token_is_unit; d.data = "m"; d.length = 1;
	ok = ok && bvnr_write_event(w, ev_type_annotation_type_family_parameter, &d);
	memset(&d, 0, sizeof d);
	d.type = token_is_type;
	ok = ok && bvnr_write_event(w, ev_type_annotation_end, &d);
	memset(&d, 0, sizeof d);
	d.type = token_is_number;
	d.value_type = BVN_TYPE_FLOAT(64);
	d.value_unit = U("m");
	d.data = "5"; d.length = 1;
	ok = ok && bvnr_write_event(w, ev_data, &d) && bvnr_write_finish(w);
	ASSERT_TRUE(ok && strstr(wbuf, ".a=<float:m>5;") != NULL, wbuf);
	bvnr_writer_destroy(w);

	/* A unit parameter event that carries the unit and no text at all is
	 * written in canonical form. It used to write "<float:64,>". */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	memset(&d, 0, sizeof d);
	d.type = token_is_identifier; d.data = "a"; d.length = 1;
	ok = bvnr_write_event(w, ev_assignment_start, &d);
	memset(&d, 0, sizeof d);
	d.type = token_is_type;
	d.value_type = BVN_TYPE_FLOAT(64);
	d.value_unit = U("k~m");
	ok = ok && bvnr_write_event(w, ev_type_annotation_start, &d);
	ok = ok && bvnr_write_event(w, ev_type_annotation_type_family, &d);
	d.type = token_is_unit;
	ok = ok && bvnr_write_event(w, ev_type_annotation_type_family_parameter, &d);
	d.type = token_is_type;
	ok = ok && bvnr_write_event(w, ev_type_annotation_end, &d);
	d.type = token_is_number; d.data = "5"; d.length = 1;
	ok = ok && bvnr_write_event(w, ev_data, &d) && bvnr_write_finish(w);
	ASSERT_TRUE(ok && strstr(wbuf, ".a=<float:k~m>5;") != NULL, wbuf);
	bvnr_writer_destroy(w);

	/* A datetime's parameter is its epoch, and only its epoch's own name. */
	w = open_writer(true, 0);
	if (!w) { failures++; return; }
	memset(&d, 0, sizeof d);
	d.type = token_is_identifier; d.data = "t"; d.length = 1;
	ok = bvnr_write_event(w, ev_assignment_start, &d);
	memset(&d, 0, sizeof d);
	d.type = token_is_type;
	d.value_type.family = vt_datetime;
	d.value_type.width  = 64;
	d.value_type.base   = (uint32_t)bvnr_datetime_epoch_index("tai");
	ok = ok && bvnr_write_event(w, ev_type_annotation_start, &d);
	ok = ok && bvnr_write_event(w, ev_type_annotation_type_family, &d);
	d.type = token_is_unit; d.data = "tai>1;.k=<uint:8"; d.length = 16;
	ASSERT_TRUE(ok && !bvnr_write_event(w, ev_type_annotation_type_family_parameter, &d),
	            "an epoch parameter that is not the epoch's name is refused");
	bvnr_writer_destroy(w);
}

static void test_writer_annotation_cap(void)
{
	printf("  ...and a spelling may not push an annotation past the reader's cap...\n");
	/* The reader holds an annotation's body in 255 bytes, family and all. A
	 * spelling that fits on its own can still be the one that overflows it,
	 * and how much room is left depends on what came before. */
	char src[BVNR_UNIT_SOURCE_MAX + 1];
	for (uint32_t L = 200; L <= BVNR_UNIT_SOURCE_MAX; L++) {
		memcpy(src, "ucum:m{", 7);
		memset(src + 7, 'a', L - 8);
		src[L - 1] = '}';
		src[L] = 0;
		bvnr_writer_t* w = open_writer(true, 0);
		if (!w) { failures++; return; }
		bool ok = bvnr_writer_set_unit_source(w, src, L) &&
		          bvnr_write_float_fix_unit(w, "a", 128, 64, 1.5, U("m")) &&
		          bvnr_write_finish(w);
		error_code_t e = bvnr_writer_get_error(w);
		bvnr_writer_destroy(w);
		seen_t s;
		bvnr_read_flags_t f;
		memset(&f, 0, sizeof f);
		char copy[sizeof wbuf];
		memcpy(copy, wbuf, sizeof copy);
		tests++;
		if (ok ? !read_doc(copy, &s, &f) : e != error_unit_too_long) {
			fprintf(stderr, "FAIL: spelling of %u bytes: %s\n", L,
			        ok ? "written, and unreadable" : "refused with the wrong code");
			failures++;
		}
	}
}

static void test_writer_arrays(void)
{
	printf("  ...and no inline unit is ever written inside an array...\n");
	/* An element's own annotation used to switch the "unit already written"
	 * state off for every LATER element of the enclosing array, which then
	 * each gained an inline unit -- and the reader takes none inside [...].
	 * A valid document replayed into one its own reader refused. */
	static const char* const docs[] = {
		".a=<float:64,s>[<float:64,m>[<uint:8>null,2],3];",
		/* A "/"-continued row is governed by the same annotation as the row
		 * before it. The first fix for the case above restored the state at
		 * each row's END and then let the enclosing array overwrite it, so
		 * the second row of an annotated block nested in a plain array lost
		 * its annotation's unit and gained inline ones. */
		".a=[<sint:16,s>[46,96]/[2707,44]];",
		".a=[[<uint:8,s>[1]/[2]]];",
		/* A profile spelling can mean "no unit": a bare UCUM annotation, the
		 * UCUM unity. The writer's check knew only the literal no_unit. */
		"#!bovnar 1.2\n.a=<float:64,ucum:{cells}>5;.b=<uint:64,_10,ucum:1>5;",
		".a=<float:64,s>[<uint:8>1,3]/[4,<float:64>5];",
		("#!bovnar 1.2\n.a=<float:64,udunits:kg m-2 s-1>"
			"[<float:64,ucum:mL{a,b}>[<uint:8>null,\"7\"],3];"),
	};
	for (size_t i = 0; i < sizeof docs / sizeof docs[0]; i++) {
		/* The directive is not an event; a replay re-declares it itself. */
		bvnr_writer_t* w = open_writer(strncmp(docs[i], "#!", 2) == 0, 0);
		bvnr_reader_t* r = bvnr_reader_create();
		if (!w || !r) { failures++; return; }
		bvnr_read_flags_t f;
		memset(&f, 0, sizeof f);
		f.userdata = w;
		f.on_verified = replay_cb;
		ASSERT_TRUE(bvnr_open_read_mem(r, docs[i], strlen(docs[i]), NULL, 0, &f) &&
		            bvnr_read(r) && bvnr_write_finish(w), docs[i]);
		bvnr_reader_destroy(r);
		bvnr_writer_destroy(w);
		seen_t s1, s2;
		char copy[sizeof wbuf];
		memcpy(copy, wbuf, sizeof copy);
		memset(&f, 0, sizeof f);
		bool ok1 = read_doc(docs[i], &s1, &f);
		memset(&f, 0, sizeof f);
		bool ok2 = read_doc(copy, &s2, &f);
		ASSERT_TRUE(ok1 && ok2, copy);
		ASSERT_TRUE(s1.n == s2.n && memcmp(s1.src, s2.src, sizeof s1.src) == 0 &&
		            memcmp(s1.has, s2.has, sizeof s1.has) == 0,
		            "the replay carries the same units on the same values");
	}

	/* And a caller building the events by hand is told, not obliged. */
	bvnr_writer_t* w = open_writer(false, 0);
	if (!w) { failures++; return; }
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	d.type = token_is_identifier; d.data = "a"; d.length = 1;
	bool ok = bvnr_write_event(w, ev_assignment_start, &d);
	memset(&d, 0, sizeof d);
	ok = ok && bvnr_write_event(w, ev_array_row_start, &d);
	d.type = token_is_array_number;
	d.value_unit = U("m");
	d.data = "1"; d.length = 1;
	ASSERT_TRUE(ok && !bvnr_write_event(w, ev_data, &d) &&
	            bvnr_writer_get_error(w) == error_unit_illegal,
	            "an inline unit on an array element is refused");
	bvnr_writer_destroy(w);
}

/* Replay a document reader -> writer; the events now carry unit_source on
 * every unit-bearing value, and the output must not move for it. */
static bool replay_cb(void* ud, bvnr_event_t ev, bvnr_data_t* d)
{
	return bvnr_write_event((bvnr_writer_t*)ud, ev, d);
}

static void test_replay(void)
{
	printf("  a reader-to-writer replay is unchanged...\n");
	static const char doc[] =
		"#!bovnar 1.2\n"
		".a=<float:64,ucum:mL{total}>1.0;"
		".b=<float:64,_10,ucum:{RBC}/uL>2.0;"
		".c=<float:64,k~m>[1.0,<float:64,ucum:mm[Hg]>3.0,4.0];"
		".d=<uint:8,km>[[1,2],[3,4]];"
		".e=<utf8>\"x\" m;";
	bvnr_writer_t* w = open_writer(true, 0);
	bvnr_reader_t* r = bvnr_reader_create();
	if (!w || !r) { failures++; return; }
	bvnr_read_flags_t f;
	memset(&f, 0, sizeof f);
	f.userdata = w;
	f.on_verified = replay_cb;
	ASSERT_TRUE(bvnr_open_read_mem(r, doc, sizeof doc - 1, NULL, 0, &f) &&
	            bvnr_read(r) && bvnr_write_finish(w), "replay");
	ASSERT_TRUE(strcmp(wbuf, doc) == 0, wbuf);
	bvnr_reader_destroy(r);
	bvnr_writer_destroy(w);
}

static void test_long_unit_text_under_flags(void)
{
	printf("  a long unit text under a unit flag stays inside its buffer...\n");
	/* The annotation path copied the text into char[512] under a guard of
	 * BVNR_UNIT_STRING_MAX. Nothing here is a unit, and the writer now refuses
	 * it before the serialiser sees it -- so what this pins is that 1000
	 * bytes of it are refused cleanly, with every copy on the way in bounds.
	 * Run under ASan. */
	char text[1000];
	memset(text, 'm', sizeof text);
	bvnr_writer_t* w = open_writer(false, BVN_UNIT_REDUCE);
	if (!w) { failures++; return; }
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	d.type = token_is_identifier; d.data = "k"; d.length = 1;
	bool ok = bvnr_write_event(w, ev_assignment_start, &d);
	memset(&d, 0, sizeof d);
	d.type = token_is_type;
	d.value_type = BVN_TYPE_FLOAT(64);
	ok = ok && bvnr_write_event(w, ev_type_annotation_start, &d);
	ok = ok && bvnr_write_event(w, ev_type_annotation_type_family, &d);
	d.type = token_is_unit;
	d.data = text; d.length = sizeof text;
	(void)(ok && bvnr_write_event(w, ev_type_annotation_type_family_parameter, &d));
	ASSERT_TRUE(ok, "reached the unit parameter");
	bvnr_writer_destroy(w);
}

/* ── DOM ────────────────────────────────────────────────────────────── */

static void test_dom(void)
{
	printf("  the DOM keeps the spelling where the unit cannot give it back...\n");
	static const char doc[] =
		"#!bovnar 1.2\n"
		".a = <float:64,ucum:mL{total}> 1.0;\n"
		".b = <float:64,m~L> 1.0;\n"          /* canonical: recoverable */
		".c = <uint:8,km> [1, 2];\n"          /* compact: not canonical */
		".d = 7;\n";
	bvn_dom_doc_t* dd = bvn_dom_parse(doc, (uint32_t)(sizeof doc - 1));
	ASSERT_TRUE(dd && bvn_dom_doc_get_parse_error(dd) == error_none, "dom parse");
	if (!dd) return;
	uint32_t n = 99;
	const char* s = bvn_dom_get_unit_source(bvn_dom_lookup(dd, ".a"), &n);
	ASSERT_TRUE(s && n == 14 && strcmp(s, "ucum:mL{total}") == 0,
	            "an annotated unit keeps its spelling");
	n = 99;
	ASSERT_TRUE(bvn_dom_get_unit_source(bvn_dom_lookup(dd, ".b"), &n) == NULL &&
	            n == 0, "a canonical spelling is not stored");
	const bvn_dom_node_t* c = bvn_dom_lookup(dd, ".c");
	s = bvn_dom_get_unit_source(bvn_dom_array_at(c, 1), &n);
	ASSERT_TRUE(s && strcmp(s, "km") == 0, "array elements carry it too");
	ASSERT_TRUE(bvn_dom_get_unit_source(bvn_dom_lookup(dd, ".d"), NULL) == NULL,
	            "a unitless value has none");
	ASSERT_TRUE(bvn_dom_get_unit_source(NULL, &n) == NULL && n == 0, "NULL node");
	bvn_dom_doc_destroy(dd);

	/* value_unit and unit_source come from ONE parse. `s·m` and `m·s` are
	 * equal, so a redundant inline suffix in the other order used to replace
	 * the struct while the annotation kept the text -- and the verdict on
	 * whether the text was worth keeping then depended on document order. */
	static const char doc3[] =
		".a = <float:64,s\xc2\xb7m> 2;\n"
		".b = <float:64,s\xc2\xb7m> 1 m\xc2\xb7s;\n"
		".c = <float:64,s\xc2\xb7m> 3;\n";
	dd = bvn_dom_parse(doc3, (uint32_t)(sizeof doc3 - 1));
	ASSERT_TRUE(dd && bvn_dom_doc_get_parse_error(dd) == error_none, "dom parse");
	if (!dd) return;
	const char* sa = bvn_dom_get_unit_source(bvn_dom_lookup(dd, ".a"), NULL);
	const char* sb = bvn_dom_get_unit_source(bvn_dom_lookup(dd, ".b"), NULL);
	const char* sc = bvn_dom_get_unit_source(bvn_dom_lookup(dd, ".c"), NULL);
	ASSERT_TRUE((sa == NULL) == (sb == NULL) && (sb == NULL) == (sc == NULL),
	            "three values written alike are kept alike");
	char ua[64], ub[64];
	ASSERT_TRUE(bvn_dom_get_unit_string(bvn_dom_lookup(dd, ".a"), ua, sizeof ua) > 0 &&
	            bvn_dom_get_unit_string(bvn_dom_lookup(dd, ".b"), ub, sizeof ub) > 0 &&
	            strcmp(ua, ub) == 0,
	            "and the unit is the annotation's parse, whatever the suffix's order");
	bvn_dom_doc_destroy(dd);

	/* A converting policy stores conv.unit, which the document never spelled. */
	bvnr_unit_policy_t pol;
	memset(&pol, 0, sizeof pol);
	bvnr_unit_target_t tgt;
	memset(&tgt, 0, sizeof tgt);
	tgt.unit = "m";
	pol.targets = &tgt;
	pol.num_targets = 1;
	static const char doc2[] = ".a = <float:64,km> 1.5;";
	dd = bvn_dom_parse_policy(doc2, (uint32_t)(sizeof doc2 - 1), &pol);
	ASSERT_TRUE(dd && bvn_dom_doc_get_parse_error(dd) == error_none, "policy parse");
	if (!dd) return;
	ASSERT_TRUE(bvn_dom_get_unit_source(bvn_dom_lookup(dd, ".a"), NULL) == NULL,
	            "a converted node has no document spelling");
	bvn_dom_doc_destroy(dd);
}

/* ── the event stream as a whole ────────────────────────────────────── */

static bool ev(bvnr_writer_t* w, bvnr_event_t e, token_type_t t,
               value_type_spec_t vt, const char* unit, const char* text)
{
	bvnr_data_t d;
	memset(&d, 0, sizeof d);
	d.type = t;
	d.value_type = vt;
	if (unit) d.value_unit = U(unit);
	if (text) { d.data = text; d.length = (uint32_t)strlen(text); }
	return bvnr_write_event(w, e, &d);
}
#define KEY(w)        ev(w, ev_assignment_start, token_is_identifier, BVN_TYPE_PLAIN, NULL, "k")
#define ROW(w)        ev(w, ev_array_row_start, token_is_number, BVN_TYPE_PLAIN, NULL, NULL)
#define ROW_END(w)    ev(w, ev_array_row_end, token_is_number, BVN_TYPE_PLAIN, NULL, NULL)
#define DIM(w)        ev(w, ev_array_dim_start, token_is_number, BVN_TYPE_PLAIN, NULL, NULL)
#define ELEM(w, u, t) ev(w, ev_data, token_is_array_number, BVN_TYPE_FLOAT(64), u, t)

/* `<float:64[,PARAM...]>` with START as the unit the start event names. */
static bool ann(bvnr_writer_t* w, const char* start, const char* p1,
                const char* p2)
{
	value_type_spec_t f = BVN_TYPE_FLOAT(64);
	bool ok = ev(w, ev_type_annotation_start, token_is_type, f, start, NULL) &&
	          ev(w, ev_type_annotation_type_family, token_is_type, f, start, NULL) &&
	          ev(w, ev_type_annotation_type_family_parameter,
	             token_is_type_width, f, start, NULL);
	if (ok && p1)
		ok = ev(w, ev_type_annotation_type_family_parameter, token_is_unit,
		        f, p1, p1);
	if (ok && p2)
		ok = ev(w, ev_type_annotation_type_family_parameter, token_is_unit,
		        f, p2, p2);
	return ok && ev(w, ev_type_annotation_end, token_is_type, f, NULL, NULL);
}

static bool rereads(void)
{
	seen_t s;
	bvnr_read_flags_t f;
	memset(&f, 0, sizeof f);
	char copy[sizeof wbuf];
	memcpy(copy, wbuf, sizeof copy);
	return read_doc(copy, &s, &f);
}

static void test_writer_annotation_governs(void)
{
	printf("  an annotation's unit governs what follows it, and is written...\n");
	bvnr_writer_t* w;

	/* The start event names a unit and no parameter event follows: the
	 * annotation was written as <float:64>, the value under it was then held
	 * to "the annotation's unit", and 1 m came out as a bare 1. */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(KEY(w) && ann(w, "m", NULL, NULL) &&
	            ev(w, ev_data, token_is_number, BVN_TYPE_FLOAT(64), "m", "1") &&
	            bvnr_write_finish(w), "a unit named only by the start event writes");
	ASSERT_TRUE(strstr(wbuf, "<float:64,m>") != NULL, wbuf);
	bvnr_writer_destroy(w);

	/* ...and one the parameter contradicts is refused, as is a second one:
	 * <float:64,m,s> is not an annotation any reader takes. */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(KEY(w) && !ann(w, "m", "s", NULL) &&
	            bvnr_writer_get_error(w) == error_unit_mismatch,
	            "the start event and the parameter must name one unit");
	bvnr_writer_destroy(w);
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(KEY(w) && !ann(w, "m", "m", "m"),
	            "a second unit parameter is refused");
	bvnr_writer_destroy(w);

	/* An array element is held to the annotation over the array exactly as a
	 * scalar is to its own: .k=<float:64,m>[1,2] with the 2 in seconds used to
	 * be written, silently, as two metres. */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(KEY(w) && ann(w, "m", "m", NULL) && ROW(w) && ELEM(w, "m", "1"),
	            "an element in the array's unit is taken");
	ASSERT_TRUE(!ELEM(w, "s", "2") &&
	            bvnr_writer_get_error(w) == error_unit_mismatch,
	            "an element in another unit is refused");
	bvnr_writer_destroy(w);
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(KEY(w) && ann(w, "m", "m", NULL) && ROW(w) && ELEM(w, "m", "1") &&
	            ROW_END(w) && DIM(w) && ROW(w) && !ELEM(w, "s", "2"),
	            "...in a continuation row as well");
	bvnr_writer_destroy(w);
	/* The same under an annotation that says no_unit. */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(KEY(w) && ann(w, "no_unit", "no_unit", NULL) &&
	            !ev(w, ev_data, token_is_number, BVN_TYPE_FLOAT(64), "m", "1") &&
	            bvnr_writer_get_error(w) == error_unit_mismatch,
	            "a value in metres under <float:64,no_unit> is refused");
	bvnr_writer_destroy(w);
	/* A unit-less element takes the array's unit and is fine. */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(KEY(w) && ann(w, "m", "m", NULL) && ROW(w) && ELEM(w, NULL, "1") &&
	            ELEM(w, "m", "2") && ROW_END(w) && bvnr_write_finish(w) &&
	            strstr(wbuf, ".k=<float:64,m>[1,2];") != NULL && rereads(), wbuf);
	bvnr_writer_destroy(w);
}

static void test_writer_stream_shape(void)
{
	printf("  ...and a stream no reader would produce is refused...\n");
	bvnr_writer_t* w;

	/* [1,2]/[3] -- the reader's error_array_row_size_mismatch. */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(KEY(w) && ROW(w) && ELEM(w, NULL, "1") && ELEM(w, NULL, "2") &&
	            ROW_END(w) && DIM(w) && ROW(w) && ELEM(w, NULL, "3"),
	            "a shorter second row is taken element by element");
	ASSERT_TRUE(!ROW_END(w) &&
	            bvnr_writer_get_error(w) == error_array_row_size_mismatch,
	            "...and refused where it closes short");
	bvnr_writer_destroy(w);
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(KEY(w) && ROW(w) && ELEM(w, NULL, "1") && ROW_END(w) && DIM(w) &&
	            ROW(w) && ELEM(w, NULL, "2") && ELEM(w, NULL, "3") && !ROW_END(w) &&
	            bvnr_writer_get_error(w) == error_array_row_size_mismatch,
	            "a longer second row is refused the same way");
	bvnr_writer_destroy(w);

	/* A struct closed across an array that is still open: .k={.k=[1}; */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(KEY(w) &&
	            ev(w, ev_struct_start, token_is_type, BVN_TYPE_PLAIN, NULL, NULL) &&
	            KEY(w) && ROW(w) && ELEM(w, NULL, "1") &&
	            !ev(w, ev_struct_end, token_is_type, BVN_TYPE_PLAIN, NULL, NULL),
	            "a struct does not close across an open array");
	bvnr_writer_destroy(w);

	/* An annotation with nothing after it: .k=<float:64,m>; */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(KEY(w) && ann(w, "m", "m", NULL) && !bvnr_write_finish(w),
	            "an annotation left without its value is not a finished stream");
	bvnr_writer_destroy(w);

	/* A row separator with no row after it: .k=[1]/; */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	ASSERT_TRUE(KEY(w) && ROW(w) && ELEM(w, NULL, "1") && ROW_END(w) && DIM(w) &&
	            !bvnr_write_finish(w),
	            "nor is a row separator with no row behind it");
	bvnr_writer_destroy(w);

	/* A struct is not a number: .k=<float:64,m>{.k=1;} */
	w = open_writer(false, 0);
	if (!w) { failures++; return; }
	bool opened = KEY(w) && ann(w, "m", "m", NULL) &&
	              ev(w, ev_struct_start, token_is_type, BVN_TYPE_PLAIN, NULL, NULL);
	ASSERT_TRUE(!opened || !KEY(w),
	            "an annotated struct takes no members");
	bvnr_writer_destroy(w);
}

static void test_reduce_is_a_fixed_point(void)
{
	printf("  a reduced document reduces to itself...\n");
	/* 4.0 N/c~m² reduced to 40000 N/m² and stopped there; fed back, N/m²
	 * became Pa. One normalisation, two runs, two documents -- so the first
	 * was never the normal form. */
	static const char* const docs[] = {
		".a=<float:64,N/c~m\xc2\xb2>4.0;",
		".a=<float:64,k~N/m\xc2\xb2>4.0;",
		".a=<float:64,k~g*m/s\xc2\xb2>2.5;",
		".a=<float:64,J/m~s>[1.0,2.0];",
		".a=4.0 N/c~m\xc2\xb2;",
	};
	for (size_t i = 0; i < sizeof docs / sizeof docs[0]; i++) {
		char pass[2][sizeof wbuf];
		const char* in = docs[i];
		bool ok = true;
		for (int k = 0; k < 2 && ok; k++) {
			bvnr_writer_t* w = open_writer(false, BVN_UNIT_REDUCE);
			bvnr_reader_t* r = bvnr_reader_create();
			if (!w || !r) { failures++; return; }
			bvnr_read_flags_t f;
			memset(&f, 0, sizeof f);
			f.userdata = w;
			f.on_verified = replay_cb;
			ok = bvnr_open_read_mem(r, in, strlen(in), NULL, 0, &f) &&
			     bvnr_read(r) && bvnr_write_finish(w);
			bvnr_reader_destroy(r);
			bvnr_writer_destroy(w);
			memcpy(pass[k], wbuf, sizeof wbuf);
			in = pass[k];
		}
		ASSERT_TRUE(ok, docs[i]);
		tests++;
		if (ok && strcmp(pass[0], pass[1]) != 0) {
			fprintf(stderr, "FAIL: %s reduces to %s and that to %s\n",
			        docs[i], pass[0], pass[1]);
			failures++;
		}
	}
}

static void test_replay_keeps_each_epoch(void)
{
	printf("  a datetime element keeps its own epoch...\n");
	/* `base` is a datetime's epoch, and the "same type as the element before"
	 * test read it as a number base -- 10 for every one. So the bare ISO
	 * literal (unix) was replayed with no annotation of its own, under tai. */
	static const char doc[] =
		"#!bovnar 1.1\n"
		".a=[<datetime:64,tai>2020-01-01T00:00:00,2020-01-01T00:00:00];";
	bvnr_writer_t* w = open_writer(false, 0);
	bvnr_reader_t* r = bvnr_reader_create();
	if (!w || !r) { failures++; return; }
	bvnr_read_flags_t f;
	memset(&f, 0, sizeof f);
	f.userdata = w;
	f.on_verified = replay_cb;
	ASSERT_TRUE(bvnr_write_version(w, 1, 1) &&
	            bvnr_open_read_mem(r, doc, sizeof doc - 1, NULL, 0, &f) &&
	            bvnr_read(r) && bvnr_write_finish(w), "replay");
	ASSERT_TRUE(strstr(wbuf, "<datetime:64,tai>") != NULL &&
	            strstr(wbuf, ",<datetime:64>") != NULL, wbuf);
	bvnr_reader_destroy(r);
	bvnr_writer_destroy(w);
}

int main(void)
{
	printf("══════════════════════════════════════\n");
	printf("  Bovnar Unit Source Test Suite\n");
	printf("══════════════════════════════════════\n\n");

	test_reader_scalars();
	test_reader_arrays();
	test_reader_resync();
	test_reader_conversion();
	test_writer_one_shot();
	test_writer_refusals();
	test_writer_flags();
	test_writer_events();
	test_writer_data_text();
	test_writer_annotation_cap();
	test_writer_long_units();
	test_writer_value_agrees();
	test_replay();
	test_writer_arrays();
	test_long_unit_text_under_flags();
	test_writer_annotation_governs();
	test_writer_stream_shape();
	test_reduce_is_a_fixed_point();
	test_replay_keeps_each_epoch();
	test_dom();

	printf("\n%d tests, %d failures\n", tests, failures);
	if (failures == 0)
		printf("ALL TESTS PASSED\n");
	return failures == 0 ? 0 : 1;
}
