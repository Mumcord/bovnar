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

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "bovnar.h"
#include "bvn_float.h"
#include "bvn_int.h"
#include "bvn_val_impl.h"
/*
 * ===========================================================================
 * High-level write convenience API
 * ===========================================================================
 *
 * bvnr_write_event is the low-level interface: powerful but verbose, requiring
 * the caller to emit the assignment-start, the type-annotation event burst, and
 * the data event by hand. The bvnr_write_* helpers here wrap that for the
 * common "write one typed key = value" case so application code reads like
 * bvnr_write_uint(w, "age", 8, 42) instead of a dozen event calls.
 *
 * The shared shape of every helper: emit_key (the `.key =` part), then for
 * numeric values bvnr_write_type_annotation (the `<...>` part), then the data
 * event. The numbers are formatted to a text buffer first because the byte
 * stream is textual; widths/bases are validated here so a bad call fails before
 * touching the sink. The bvnf/bvni variants accept the library's
 * arbitrary-precision float/int types for values that don't fit in double/u64.
 */

/*
 * Emit the `.key =` assignment header common to every typed-value helper.
 */
static bool emit_key(bvnr_writer_t *w, const char *key)
{
	if (!key)
		return bvn_writer_set_error(w, error_invalid_argument);
	bvnr_data_t d = {
		.type   = token_is_identifier,
		.data   = (const void *)key,
		.length = (uint32_t)strlen(key),
	};
	return bvnr_write_event(w, ev_assignment_start, &d);
}
/*
 * Expand a (type spec, unit) pair into the full `<family:width,_base,unit>`
 * event sequence. Only the parameters that are actually meaningful for the
 * family are emitted — e.g. width only for numeric/utf8 types, an explicit base
 * only when it differs from the default 10, the q parameter only for float_fix,
 * and the unit only when present. Suppressing redundant parameters is what
 * keeps annotations compact (`<uint>` rather than `<uint:64,_10,no_unit>`).
 * The unit is rendered to text honouring the writer's unit_flags (reduce /
 * ASCII-exponent options).
 */
/*
 * The one-shot spelling for the next unit (see bovnar.h). Kept in the writer's
 * validator, whose unit_src is otherwise unused on this side.
 */
bool bvnr_writer_set_unit_source(bvnr_writer_t *w,
				const char *text, uint32_t length)
{
	if (!w) return false;
	w->usrc_keys = 0;
	if (!text || !length) {
		w->val.unit_src_len = 0;
		return true;
	}
	if (length > BVNR_UNIT_SOURCE_MAX) {
		w->val.unit_src_len = 0;
		return bvn_writer_set_error(w, error_unit_too_long);
	}
	memcpy(w->val.unit_src, text, length);
	w->val.unit_src_len = (uint8_t)length;
	return true;
}
bool bvnr_write_type_annotation(bvnr_writer_t *w,
				value_type_spec_t vt,
				value_unit_t vu)
{
	if (vu.num_components == 1u && vu.components[0].base == bu_none)
		vu = BVN_UNIT_NONE;
	/* Take the one-shot spelling now and forget it, whatever happens below: a
	 * spelling that outlived a failed write would land on an unrelated value.
	 * The copy is what the unit-parameter event points at. */
	uint8_t  usrc[BVNR_UNIT_SOURCE_MAX + 1u];
	uint32_t usrc_len = w ? w->val.unit_src_len : 0u;
	if (usrc_len) {
		memcpy(usrc, w->val.unit_src, usrc_len);
		w->val.unit_src_len = 0;
		w->usrc_keys        = 0;
	}
	bvnr_data_t d = {
		.type       = token_is_type,
		.value_type = vt,
		.value_unit = vu,
	};
	if (!bvnr_write_event(w, ev_type_annotation_start,       &d)) return false;
	if (!bvnr_write_event(w, ev_type_annotation_type_family, &d)) return false;
	/* Only numeric families and datetime carry a width; utf8/bool are
	 * parameterless and a width on them is rejected by the type-spec validator. */
	if ((bvn_type_is_numeric(vt) || vt.family == vt_datetime) && vt.width != 0) {
		d.type   = token_is_type_width;
		d.data   = NULL;
		d.length = 0;
		if (!bvnr_write_event(w,
				ev_type_annotation_type_family_parameter, &d))
			return false;
	}
	/* datetime's epoch is carried as a named parameter (base holds the epoch
	 * index; 0 = unix is the default and stays implicit). */
	if (vt.family == vt_datetime && vt.base != 0u) {
		const char *ep = bvnr_datetime_epoch_name(vt);
		d.type   = token_is_unit;
		d.data   = (const void *)ep;
		d.length = (uint32_t)strlen(ep);
		if (!bvnr_write_event(w,
				ev_type_annotation_type_family_parameter, &d))
			return false;
	}
	if (vt.family == vt_float && vt.base != 0u && vt.base != 10u) {
		d.type   = token_is_type_base;
		d.data   = NULL;
		d.length = 0;
		if (!bvnr_write_event(w,
				ev_type_annotation_type_family_parameter, &d))
			return false;
	}
	if (vt.family == vt_sint || vt.family == vt_uint) {
		if (vt.base != 0u && vt.base != 10u) {
			d.type   = token_is_type_base;
			d.data   = NULL;
			d.length = 0;
			if (!bvnr_write_event(w,
					ev_type_annotation_type_family_parameter, &d))
				return false;
		}
	}
	if (vt.family == vt_float_fix && vt.base != 0u) {
		d.type   = token_is_type_q;
		d.data   = NULL;
		d.length = 0;
		if (!bvnr_write_event(w,
				ev_type_annotation_type_family_parameter, &d))
			return false;
	}
	if (vu.num_components > 0) {
		char ubuf[BVNR_UNIT_STRING_MAX];
		bvn_unit_flags_t uflags = bvnr_writer_unit_flags(w);
		int32_t ulen = bvn_unit_to_string_ex(vu, ubuf, sizeof(ubuf), uflags);
		if (ulen < 0) {
			/* -1 is not only "the text did not fit". It also means the unit has
			 * no spelling (a bu_none component) and, under BVN_UNIT_REDUCE, that
			 * the reduction drops a component and so cannot express this unit at
			 * all. Reporting those as error_unit_too_long sends the caller after
			 * a buffer that was never the problem. Ask again with a buffer
			 * nothing could overflow — a unit is capped at 8 components and an
			 * alias at 31 bytes, so this cannot fail for length — and if it still
			 * refuses, the length was not the cause. */
			char probe[1024];
			int32_t again = bvn_unit_to_string_ex(vu, probe, sizeof probe, uflags);
			return bvn_writer_set_error(
				w, again < 0 ? error_unit_illegal : error_unit_too_long);
		}
		d.type   = token_is_unit;
		d.data   = (const void *)ubuf;
		d.length = (uint32_t)ulen;
		if (usrc_len) {
			d.unit_source        = usrc;
			d.unit_source_length = usrc_len;
		}
		if (!bvnr_write_event(w,
				ev_type_annotation_type_family_parameter, &d))
			return false;
		d.unit_source        = NULL;
		d.unit_source_length = 0;
	} else if (usrc_len) {
		/* A spelling for a value that turned out to carry no unit is a caller
		 * mistake, and dropping it silently would hide which value it was
		 * meant for. */
		return bvn_writer_set_error(w, error_unit_mismatch);
	}
	d.type   = token_is_type;
	d.data   = NULL;
	d.length = 0;
	return bvnr_write_event(w, ev_type_annotation_end, &d);
}
/*
 * Common tail for every numeric helper: write the annotation, then the value
 * event. The token type is a parameter because a non-decimal-base number is
 * carried as a string token (the digits use a non-numeric alphabet), whereas
 * base-10 uses the number token; callers pick accordingly.
 */
static bool emit_numeric_tt(bvnr_writer_t *w,
			     value_type_spec_t vt, value_unit_t vu,
			     const char *valbuf, token_type_t tt)
{
	if (!bvnr_write_type_annotation(w, vt, vu)) return false;
	bvnr_data_t d = {
		.type       = tt,
		.value_type = vt,
		.value_unit = vu,
		.data       = (const void *)valbuf,
		.length     = (uint32_t)strlen(valbuf),
	};
	return bvnr_write_event(w, ev_data, &d);
}
static bool emit_numeric(bvnr_writer_t *w,
			 value_type_spec_t vt, value_unit_t vu,
			 const char *valbuf)
{
	return emit_numeric_tt(w, vt, vu, valbuf, token_is_number);
}
bool bvnr_write_string(bvnr_writer_t *w, const char *key, const char *value)
{
	if (!emit_key(w, key)) return false;
	if (!value) return bvn_writer_set_error(w, error_invalid_argument);
	bvnr_data_t d = {
		.type   = token_is_string,
		.data   = (const void *)value,
		.length = (uint32_t)strlen(value),
	};
	return bvnr_write_event(w, ev_data, &d);
}
bool bvnr_write_plain(bvnr_writer_t *w, const char *key, const char *value)
{
	if (!emit_key(w, key)) return false;
	if (!value) return bvn_writer_set_error(w, error_invalid_argument);
	bvnr_data_t d = {
		.type   = token_is_symbol,
		.data   = (const void *)value,
		.length = (uint32_t)strlen(value),
	};
	return bvnr_write_event(w, ev_data, &d);
}
bool bvnr_write_null(bvnr_writer_t *w, const char *key)
{
	if (!emit_key(w, key)) return false;
	bvnr_data_t d = { .type = token_is_null_value };
	return bvnr_write_event(w, ev_data, &d);
}
bool bvnr_write_bool(bvnr_writer_t *w, const char *key, bool value)
{
	if (!emit_key(w, key)) return false;
	const char *sym = value ? "true" : "false";
	bvnr_data_t d = {
		.type   = token_is_bool,
		.data   = (const void *)sym,
		.length = (uint32_t)strlen(sym),
	};
	return bvnr_write_event(w, ev_data, &d);
}
/*
 * The scalar helpers below all follow one pattern, with a no-unit wrapper
 * delegating to a _unit variant: validate width, format the value to text, then
 * emit_numeric. A width of 0 means "unspecified" (the reader infers a default).
 * The BINARY float helper narrows the value through float when width<=32 so the
 * emitted text matches the precision the type promises rather than the full
 * double. The fixed-point and decimal helpers do not: a binary32 is not what
 * either of those types promises (see the notes in them).
 */
bool bvnr_write_uint(bvnr_writer_t *w, const char *key,
			 uint32_t width, uint64_t value)
{
	return bvnr_write_uint_unit(w, key, width, value, BVN_UNIT_NONE);
}
bool bvnr_write_uint_unit(bvnr_writer_t *w, const char *key,
			   uint32_t width, uint64_t value, value_unit_t unit)
{
	if (!emit_key(w, key)) return false;
	value_type_spec_t vt = BVN_TYPE_UINT(width);
	char buf[32];
	if (bvn_format_uint64(buf, sizeof(buf), value, 10, 0) < 0)
	{
		w->val.last_error   = error_number_too_long;
		w->val.error_offset = bvnr_sink_bytes_written(&w->ser.sink)
		                    + (uint64_t)w->ser.wbuf_pos;
		return false;
	}
	return emit_numeric(w, vt, unit, buf);
}
bool bvnr_write_sint(bvnr_writer_t *w, const char *key,
			 uint32_t width, int64_t value)
{
	return bvnr_write_sint_unit(w, key, width, value, BVN_UNIT_NONE);
}
bool bvnr_write_sint_unit(bvnr_writer_t *w, const char *key,
			   uint32_t width, int64_t value, value_unit_t unit)
{
	if (!emit_key(w, key)) return false;
	value_type_spec_t vt = BVN_TYPE_SINT(width);
	char buf[32];
	if (bvn_format_int64(buf, sizeof(buf), value, 10, 0) < 0)
	{
		w->val.last_error   = error_number_too_long;
		w->val.error_offset = bvnr_sink_bytes_written(&w->ser.sink)
		                    + (uint64_t)w->ser.wbuf_pos;
		return false;
	}
	return emit_numeric(w, vt, unit, buf);
}
bool bvnr_write_datetime(bvnr_writer_t *w, const char *key,
			 uint32_t width, const char *epoch, int64_t value)
{
	if (!emit_key(w, key)) return false;
	int32_t ei = bvnr_datetime_epoch_index(epoch);
	if (ei < 0) return bvn_writer_set_error(w, error_invalid_argument);
	value_type_spec_t vt = { .family = vt_datetime,
				 .width = width, .base = (uint32_t)ei };
	char buf[32];
	if (bvn_format_int64(buf, sizeof(buf), value, 10, 0) < 0)
	{
		w->val.last_error   = error_number_too_long;
		w->val.error_offset = bvnr_sink_bytes_written(&w->ser.sink)
		                    + (uint64_t)w->ser.wbuf_pos;
		return false;
	}
	return emit_numeric(w, vt, BVN_UNIT_NONE, buf);
}
bool bvnr_write_float(bvnr_writer_t *w, const char *key,
			  uint32_t width, double value)
{
	return bvnr_write_float_unit(w, key, width, value, BVN_UNIT_NONE);
}
bool bvnr_write_float_unit(bvnr_writer_t *w, const char *key,
			    uint32_t width, double value, value_unit_t unit)
{
	if (!emit_key(w, key)) return false;
	if (width != 0u && width != 16u && width != 32u && width != 64u
	 && width != 128u && width != 256u)
		return bvn_writer_set_error(w, error_invalid_argument);
	value_type_spec_t vt = BVN_TYPE_FLOAT(width);
	if (width != 0u && width <= 32u) value = (double)(float)value;
	char buf[64];
	if (bvn_format_double(buf, sizeof(buf), value, vt) < 0)
	{
		w->val.last_error   = error_number_too_long;
		w->val.error_offset = bvnr_sink_bytes_written(&w->ser.sink)
		                    + (uint64_t)w->ser.wbuf_pos;
		return false;
	}
	return emit_numeric(w, vt, unit, buf);
}
bool bvnr_write_float_fix(bvnr_writer_t *w, const char *key,
			   uint32_t width, uint32_t q, double value)
{
	return bvnr_write_float_fix_unit(w, key, width, q, value, BVN_UNIT_NONE);
}
bool bvnr_write_float_fix_unit(bvnr_writer_t *w, const char *key,
			        uint32_t width, uint32_t q,
			        double value, value_unit_t unit)
{
	if (!emit_key(w, key)) return false;
	if (width != 0u && width != 16u && width != 32u && width != 64u
	 && width != 128u && width != 256u)
		return bvn_writer_set_error(w, error_invalid_argument);
	value_type_spec_t vt = BVN_TYPE_FLOAT_FIX(width, q);
	/* No narrowing through a binary32 here, unlike bvnr_write_float_unit. A
	 * Q16.16 value has 32 significant bits and a float has 24, so the "matching"
	 * narrowing threw away a quarter of what the type holds: 12345.67 was
	 * written 7.8e-5 off where the format resolves 1.5e-5, and 2147483647 in a
	 * q0 field rounded up to 2^31 and was then refused as out of range. */
	char buf[64];
	if (bvn_format_double(buf, sizeof(buf), value, vt) < 0)
	{
		w->val.last_error   = error_number_too_long;
		w->val.error_offset = bvnr_sink_bytes_written(&w->ser.sink)
		                    + (uint64_t)w->ser.wbuf_pos;
		return false;
	}
	return emit_numeric(w, vt, unit, buf);
}
bool bvnr_write_float_dec(bvnr_writer_t *w, const char *key,
			   uint32_t width, double value)
{
	return bvnr_write_float_dec_unit(w, key, width, value, BVN_UNIT_NONE);
}
bool bvnr_write_float_dec_unit(bvnr_writer_t *w, const char *key,
			        uint32_t width,
			        double value, value_unit_t unit)
{
	if (!emit_key(w, key)) return false;
	if (width != 0u && width != 16u && width != 32u && width != 64u
	 && width != 128u && width != 256u)
		return bvn_writer_set_error(w, error_invalid_argument);
	value_type_spec_t vt = BVN_TYPE_FLOAT_DEC(width);
	/* No narrowing through a binary32 here, unlike bvnr_write_float_unit. A
	 * decimal32 holds seven DECIMAL digits, and not every seven-digit decimal
	 * survives a binary32: 8.589973e9 went out as 8.589973504e+9, which a
	 * decimal32 reader rounds to 8.589974e9 -- a value the type represents
	 * exactly, written as its neighbour. */
	char buf[64];
	if (bvn_format_double(buf, sizeof(buf), value, vt) < 0)
	{
		w->val.last_error   = error_number_too_long;
		w->val.error_offset = bvnr_sink_bytes_written(&w->ser.sink)
		                    + (uint64_t)w->ser.wbuf_pos;
		return false;
	}
	return emit_numeric(w, vt, unit, buf);
}
bool bvnr_write_struct_start(bvnr_writer_t *w, const char *key)
{
	if (!emit_key(w, key)) return false;
	bvnr_data_t d = {0};
	return bvnr_write_event(w, ev_struct_start, &d);
}
bool bvnr_write_struct_end(bvnr_writer_t *w)
{
	bvnr_data_t d = {0};
	return bvnr_write_event(w, ev_struct_end, &d);
}
bool bvnr_write_bvnf_unit(bvnr_writer_t *w, const char *key,
			   const bvn_float_t *f,
			   uint32_t width, value_unit_t unit)
{
	return bvnr_write_bvnf_base_unit(w, key, f, width, 10u, unit);
}
bool bvnr_write_bvnf(bvnr_writer_t *w, const char *key,
			 const bvn_float_t *f, uint32_t width)
{
	return bvnr_write_bvnf_base_unit(w, key, f, width, 10u, BVN_UNIT_NONE);
}
/*
 * Emit an arbitrary-precision float (bvn_float_t) for values beyond double's
 * range/precision. Only base 10 and 16 are meaningful for floats. The text
 * length is unbounded in principle, so unlike the scalar helpers it formats
 * into a malloc'd buffer sized by bvn_float_str_bufsize. A base-16 value is
 * carried as a string token (hex digits aren't a number-token alphabet) unless
 * it is a special value like inf/nan. The bvni variant is the integer analogue.
 */
bool bvnr_write_bvnf_base_unit(bvnr_writer_t *w, const char *key,
				const bvn_float_t *f,
				uint32_t width, uint32_t base,
				value_unit_t unit)
{
	if (!f) return bvn_writer_set_error(w, error_invalid_argument);
	uint32_t effective_base = base ? base : 10u;
	if (effective_base != 10u && effective_base != 16u)
		return bvn_writer_set_error(w, error_invalid_argument);
	uint32_t prec = width ? width : (uint32_t)f->_prec;
	/* Refuse an illegal width BEFORE the key goes out. The annotation check
	 * that would catch it runs after emit_key, so the failed call left a
	 * dangling ".c=" in the output -- with width 0 whenever f->_prec is not
	 * itself a float width (a bvn_float_t of 53 bits is the common case). */
	if (prec != 16u && (prec == 0u || prec % 32u != 0u ||
			    prec > BVN_FLOAT_MAX_PREC))
		return bvn_writer_set_error(w, error_illegal_value_type);
	if (!emit_key(w, key)) return false;
	value_type_spec_t vt = BVN_TYPE_FLOAT_BASE(prec,
		effective_base == 10u ? 0u : 16u);
	size_t bufsz = bvn_float_str_bufsize(prec, effective_base);
	char *buf = malloc(bufsz);
	if (!buf) return bvn_writer_set_error(w, error_invalid_argument);
	int32_t n = bvn_float_to_str(f, buf, bufsz, effective_base);
	bool ok = false;
	if (n > 0) {
		token_type_t tt;
		if (effective_base == 10u || bvn_is_special_number_string(buf))
			tt = token_is_number;
		else
			tt = token_is_string;
		ok = emit_numeric_tt(w, vt, unit, buf, tt);
	} else {
		bvn_writer_set_error(w, error_value_out_of_range);
	}
	free(buf);
	return ok;
}
bool bvnr_write_bvnf_base(bvnr_writer_t *w, const char *key,
			   const bvn_float_t *f,
			   uint32_t width, uint32_t base)
{
	return bvnr_write_bvnf_base_unit(w, key, f, width, base, BVN_UNIT_NONE);
}
bool bvnr_write_bvni_unit(bvnr_writer_t *w, const char *key,
			   const bvn_int_t *n,
			   uint32_t width, uint32_t base,
			   value_unit_t unit)
{
	if (!n) return bvn_writer_set_error(w, error_invalid_argument);
	uint32_t effective_base = base ? base : 10u;
	/* Bases 64 and 85 are unsigned-only (they use '+'/'-' as digit characters,
	 * leaving no sign character). A negative value would require a signed type,
	 * which is illegal in those bases — reject it before emitting anything. */
	if (n->negative && (effective_base == 64u || effective_base == 85u))
		return bvn_writer_set_error(w, error_illegal_value_type);
	/* Width 0 is "the default", and the default is 64 -- to the reader as well,
	 * which holds `<uint> 18446744073709551616` to 64 bits and refuses it. The
	 * range check further down runs off the declared width, so with none
	 * declared there was no check, and this wrote a document that the library
	 * could not read back. A sint:64 reaches -2^63, whose magnitude alone has
	 * the 64 bits; anything else negative has at most 63. */
	if (width == 0u) {
		int bl = bvn_int_bitlen(n);
		bool min64 = false;
		if (n->negative && bl == 64) {
			min64 = true;
			for (int i = 0; i < 63 && min64; i++)
				if (bvn_int_getbit(n, i)) min64 = false;
		}
		if (bl > (n->negative ? 63 : 64) && !min64)
			return bvn_writer_set_error(w, error_value_out_of_range);
	}
	if (!emit_key(w, key)) return false;
	value_type_family_t fam = n->negative ? vt_sint : vt_uint;
	value_type_spec_t vt;
	if (fam == vt_sint)
		vt = BVN_TYPE_SINT_BASE(width, effective_base == 10u ? 0u : effective_base);
	else
		vt = BVN_TYPE_UINT_BASE(width, effective_base == 10u ? 0u : effective_base);
	uint32_t actual_bits = n->nused * 32u;
	uint32_t bits = width ? width : (actual_bits ? actual_bits : 64u);
	if (actual_bits > bits)
		bits = actual_bits;
	size_t bufsz = bvn_int_str_bufsize(bits, effective_base);
	char *buf = malloc(bufsz);
	if (!buf) return bvn_writer_set_error(w, error_invalid_argument);
	int32_t nr = bvn_int_to_str(n, buf, bufsz, effective_base);
	bool ok = false;
	if (nr > 0) {
		token_type_t tt = (effective_base == 10u)
			? token_is_number : token_is_string;
		ok = emit_numeric_tt(w, vt, unit, buf, tt);
	} else {
		bvn_writer_set_error(w, error_value_out_of_range);
	}
	free(buf);
	return ok;
}
bool bvnr_write_bvni(bvnr_writer_t *w, const char *key,
		     const bvn_int_t *n,
		     uint32_t width, uint32_t base)
{
	return bvnr_write_bvni_unit(w, key, n, width, base, BVN_UNIT_NONE);
}
