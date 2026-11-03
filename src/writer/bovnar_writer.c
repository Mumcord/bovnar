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
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "bovnar.h"
#include "bvn_datetime.h"
#include "bvn_float.h"
#include "bvn_int.h"
#include "bovnar_si_units.h"
#include "bvn_io_impl.h"
#include "bvn_val_impl.h"
#include "bvn_profile_impl.h"
/*
 * ===========================================================================
 * Writer / serializer
 * ===========================================================================
 *
 * The writer is the inverse of the reader: the caller drives it with the same
 * bvnr_event_t stream the reader emits (so reader output can be piped straight
 * into a writer to canonicalise/pretty-print), and it produces bovnar bytes.
 *
 * Each bvnr_write_event goes through two stages, mirroring the reader's
 * lexer/validator split but in reverse:
 *
 *   1. bvn_writer_validate_event — reject anything that would produce an
 *      ill-formed or out-of-spec document (bad identifier, value out of range
 *      for its type, unbalanced struct close, illegal type spec, ...). This is
 *      what makes the writer safe to feed with caller-supplied data: it cannot
 *      be coaxed into emitting a stream the reader would reject.
 *   2. bvn_ser_serialize_event — actually format the bytes.
 *
 * Output buffering: for an fd sink, bytes are accumulated in wbuf and flushed
 * in BVN_SER_WBUF_SIZE chunks to amortise write() syscalls; for a memory sink
 * the wbuf is bypassed and bytes are copied straight in (bvn_ser_push). Errors
 * are sticky — once w->val.last_error is set, every further call short-circuits
 * — so callers can write a whole document and check once at bvnr_write_finish.
 *
 * Separator bookkeeping is the fiddly part: bovnar uses ';' between top-level/
 * struct members and ',' between array elements, and pretty mode adds
 * indentation/newlines. need_semi, the per-depth arr_need_comma[], and
 * struct_depth_at_array_start[] together decide when a separator is due — see
 * bvn_ser_emit_pending_comma / bvn_ser_mark_value_done.
 */

/*
 * Writer = serializer + a validator (reused only for its type/unit state and
 * error fields). Created zeroed and inert; bvnr_open_write_* installs the sink.
 */
bvnr_writer_t* bvnr_writer_create(void)
{
	bvnr_writer_t* w = malloc(sizeof(*w));
	if (!w) return NULL;
	memset(w, 0, sizeof(*w));
	return w;
}
void bvnr_writer_destroy(bvnr_writer_t* w)
{
	if (!w) return;
	free(w->ser.ser_value_text);
	bvnr_reader_destroy(w->usrc_probe);
	free(w);
}
/*
 * Record the first error and the byte offset it occurred at (flushed bytes plus
 * what is still buffered), then return false so callers can `return
 * bvn_writer_set_error(...)` as a one-liner. Because errors are sticky upstream,
 * only the first error's position is kept — the most useful one to report.
 */
bool bvn_writer_set_error(bvnr_writer_t* w, error_code_t err)
{
	w->val.last_error   = err;
	w->val.error_offset = bvnr_sink_bytes_written(&w->ser.sink)
	                    + (uint64_t)w->ser.wbuf_pos;
	return false;
}
bool bvn_ser_flush_wbuf(bvnr_serializer_t* s)
{
	if (!s->wbuf_pos) return true;
	/* A writer whose bvnr_open_write_* failed has no sink, so sink.push is NULL.
	 * Events still "succeed" up to this point because they only fill wbuf, so a
	 * caller checking every return value learns nothing — and the first flush,
	 * usually from bvnr_write_finish, then jumped through the null pointer.
	 * Fail cleanly instead. */
	if (!bvn_sink_impl(&s->sink)->push) {
		s->ser_error = error_writing_to_sink;
		return false;
	}
	if (!bvn_sink_push(&s->sink, s->wbuf, s->wbuf_pos)) return false;
	s->wbuf_pos = 0;
	return true;
}
/*
 * Central output primitive. Memory sinks are written through directly (no
 * point double-buffering a buffer into another buffer), while fd sinks
 * accumulate into wbuf and flush a full block at a time, turning many tiny
 * emits (one per byte/separator) into a few large write() calls. All the
 * push_byte/push_str/indent/newline helpers below funnel through here so the
 * buffering policy lives in exactly one place.
 */
static bool bvn_ser_push(bvnr_serializer_t* s,
	const void* data, uint32_t len)
{
	if (bvn_sink_impl(&s->sink)->is_mem)
		return bvn_sink_push(&s->sink, data, len);
	const uint8_t* src = (const uint8_t*)data;
	uint32_t pos = 0;
	while (pos < len) {
		uint32_t space = BVN_SER_WBUF_SIZE - s->wbuf_pos;
		uint32_t copy  = (len - pos) < space ? (len - pos) : space;
		memcpy(s->wbuf + s->wbuf_pos, src + pos, copy);
		s->wbuf_pos += copy;
		pos         += copy;
		if (s->wbuf_pos == BVN_SER_WBUF_SIZE) {
			if (!bvn_ser_flush_wbuf(s)) return false;
		}
	}
	return true;
}
/* Bytes this document has produced so far: flushed plus still buffered. */
static uint64_t bvn_ser_position(const bvnr_serializer_t* s)
{
	return bvnr_sink_bytes_written(&s->sink) + (uint64_t)s->wbuf_pos;
}
static bool bvn_ser_push_byte(bvnr_serializer_t* s, uint8_t b)
{
	return bvn_ser_push(s, &b, 1);
}
static bool bvn_ser_push_str(bvnr_serializer_t* s, const char* str)
{
	return bvn_ser_push(s, str, (uint32_t)strlen(str));
}
static bool bvn_ser_indent(bvnr_serializer_t* s)
{
	if (!s->pretty || !s->indent) return true;
	uint8_t tabs[64];
	uint32_t fill = s->indent > 64u ? 64u : s->indent;
	memset(tabs, '\t', fill);
	uint32_t remaining = s->indent;
	while (remaining > 0) {
		uint32_t chunk = remaining > 64u ? 64u : remaining;
		if (!bvn_ser_push(s, tabs, chunk)) return false;
		remaining -= chunk;
	}
	return true;
}
static bool bvn_ser_newline(bvnr_serializer_t* s)
{
	if (!s->pretty) return true;
	return bvn_ser_push(s, "\n", 1);
}
static bool bvn_ser_space(bvnr_serializer_t* s)
{
	if (!s->pretty) return true;
	return bvn_ser_push_byte(s, ' ');
}
bool bvn_ser_finish_stream(bvnr_serializer_t* s)
{
	if (!s->need_semi) return bvn_ser_flush_wbuf(s);
	if (!bvn_ser_push_byte(s, ';')) return false;
	if (s->pretty && !bvn_ser_push_byte(s, '\n')) return false;
	s->need_semi = false;
	return bvn_ser_flush_wbuf(s);
}
static bool bvn_validate_string_content(bvnr_writer_t* w,
	const uint8_t* data, uint32_t length)
{
	if (length == 0) return true;
	if (!data) return bvn_writer_set_error(w, error_invalid_argument);
	for (uint32_t i = 0; i < length; i++) {
		uint8_t c = data[i];
		if (c <= 0x08 || (c >= 0x0E && c <= 0x1F) || c == 0x7F) {
			return bvn_writer_set_error(w,
				error_unexpected_input_byte);
		}
	}
	if (!bvn_validate_string(data, (size_t)length)) {
		return bvn_writer_set_error(w, error_invalid_utf8_byte);
	}
	return true;
}
static bool bvn_validate_id_for_writer(bvnr_writer_t* w,
	const void* data, uint32_t length)
{
	if (!data || length == 0) {
		return bvn_writer_set_error(w, error_empty_identifier);
	}
	char  static_buf[256];
	char *buf = static_buf;
	bool  need_free = false;
	if (length >= sizeof(static_buf)) {
		/* (size_t)length + 1u would wrap to 0 only when size_t is 32-bit and
		 * length == UINT32_MAX; reject before allocating so memcpy below can
		 * never run against a 0-byte buffer. Harmless on 64-bit (no wrap). */
		if (length == UINT32_MAX)
			return bvn_writer_set_error(w, error_invalid_argument);
		buf = malloc((size_t)length + 1u);
		if (!buf)
			return bvn_writer_set_error(w, error_invalid_argument);
		need_free = true;
	}
	memcpy(buf, data, length);
	buf[length] = '\0';
	bool ok = true;
	/* max_identifier_length is a uint16_t, so NO reader configuration can
	 * accept more than 65535 bytes; emitting a longer key produced a document
	 * nothing could read back, reported as success. The cap used to be 255, on
	 * the belief that the field was a uint8_t: a key a reader with the limit
	 * raised accepts was refused. 255 is the reader's DEFAULT, not its ceiling
	 * -- the same distinction the string and symbol caps below make. */
	if (length > UINT16_MAX)
		ok = bvn_writer_set_error(w, error_identifier_too_long);
	else if (!bvn_validate_identifier(buf))
		ok = bvn_writer_set_error(w, error_invalid_argument);
	if (need_free) free(buf);
	return ok;
}
/*
 * Validate a numeric literal the caller wants to emit against its declared
 * type: digits legal in the base, sign legal for the family (no '-' for uint),
 * dot/exponent only for float families, and the value within the type's range.
 *
 * The static_buf/malloc pattern recurring through these validators exists
 * because the underlying string validators want a NUL-terminated C string, but
 * event payloads are (ptr,len) and not necessarily terminated. Short values use
 * a stack buffer (the overwhelmingly common case, zero allocation); only an
 * unusually long literal falls back to malloc. The same idiom appears in the
 * id/symbol/reference/string-as-number validators.
 */
static bool bvn_validate_number_for_writer(bvnr_writer_t* w,
	const void* data, uint32_t length, value_type_spec_t vt)
{
	if (!data || length == 0) return true;
	char static_buf[256];
	char *buf = static_buf;
	bool need_free = false;
	if (length >= sizeof(static_buf)) {
		if (length == UINT32_MAX)   /* see bvn_validate_id_for_writer */
			return bvn_writer_set_error(w, error_number_too_long);
		buf = malloc((size_t)length + 1u);
		if (!buf)
			return bvn_writer_set_error(w, error_number_too_long);
		need_free = true;
	}
	memcpy(buf, data, length);
	buf[length] = '\0';
	bool ok = true;
	if (bvn_is_special_number_string(buf)) {
		if (!bvn_type_is_numeric(vt) && !bvn_type_is_plain(vt)) {
			ok = bvn_writer_set_error(w,
				error_type_value_mismatch);
		}
		goto out;
	}
	{
		uint32_t base  = bvn_effective_base(vt);
		uint32_t width = bvn_effective_width(vt);
		/* In bases 64/85 a leading '-' is a digit, not a negative sign. */
		if (vt.family == vt_uint && buf[0] == '-' &&
		    base != 64u && base != 85u) {
			ok = bvn_writer_set_error(w, error_value_out_of_range);
			goto out;
		}
		if (bvn_type_is_numeric(vt) && base != 10) {
			if (!bvn_validate_number_in_base(buf, base)) {
				ok = bvn_writer_set_error(w,
					error_digit_not_in_base);
				goto out;
			}
		} else {
			if (!bvn_validate_number(buf)) {
				ok = bvn_writer_set_error(w, error_type_value_mismatch);
				goto out;
			}
			bool has_dot = false, has_exp = false;
			for (uint32_t i = 0; i < length; i++) {
				if (buf[i] == '.') has_dot = true;
				if (buf[i] == 'e' || buf[i] == 'E') has_exp = true;
			}
			if ((has_dot || has_exp) && vt.family != vt_float &&
				vt.family != vt_float_fix &&
				vt.family != vt_float_dec &&
				vt.family != vt_plain) {
				ok = bvn_writer_set_error(w, error_type_value_mismatch);
				goto out;
			}
		}
		/* `width` is the EFFECTIVE width, so an annotation that names none is
		 * held to the 64 bits the reader reads it at. These two tests used to
		 * be skipped when no width was declared, and `<uint> 2^128` went out
		 * for the reader to refuse. */
		if (vt.family == vt_uint) {
			if (!bvn_validate_uint_range(buf, width, base)) {
				ok = bvn_writer_set_error(w,
					error_value_out_of_range);
				goto out;
			}
		} else if (vt.family == vt_sint || vt.family == vt_datetime) {
			/* datetime carrier is a signed integer (base 10); range-check like sint */
			if (!bvn_validate_sint_range(buf, width, base)) {
				ok = bvn_writer_set_error(w,
					error_value_out_of_range);
				goto out;
			}
		} else if (vt.family == vt_float_fix) {
			/* Refuse to emit a value the declared Q-format can't hold
			 * (would saturate/wrap on decode); matches the reader. */
			if (!bvn_float_str_fits_fix(buf, 10u, width,
									    bvn_effective_q(vt))) {
				ok = bvn_writer_set_error(w,
					error_value_out_of_range);
				goto out;
			}
		}
	}
out:
	if (need_free) free(buf);
	return ok;
}
static bool bvn_validate_string_as_number(bvnr_writer_t* w,
	const void* data, uint32_t length, value_type_spec_t vt)
{
	if (!data || length == 0) return true;
	char static_buf[256];
	char *buf = static_buf;
	bool need_free = false;
	if (length >= sizeof(static_buf)) {
		if (length == UINT32_MAX)   /* see bvn_validate_id_for_writer */
			return bvn_writer_set_error(w, error_string_too_long);
		buf = malloc((size_t)length + 1u);
		if (!buf)
			return bvn_writer_set_error(w, error_string_too_long);
		need_free = true;
	}
	memcpy(buf, data, length);
	buf[length] = '\0';
	bool ok = true;
	{
		uint32_t base  = bvn_effective_base(vt);
		uint32_t width = bvn_effective_width(vt);
		/* In bases 64/85 a leading '-' is a digit, not a negative sign. */
		if (vt.family == vt_uint && buf[0] == '-' &&
		    base != 64u && base != 85u) {
			ok = bvn_writer_set_error(w, error_value_out_of_range);
			goto out;
		}
		if (vt.family == vt_uint || vt.family == vt_sint) {
			if (!bvn_validate_digits_for_base(buf, base)) {
				ok = bvn_writer_set_error(w,
					error_digit_not_in_base);
				goto out;
			}
			/* A QUOTED "nan", "inf" or "ninf" under an integer type is not a
			 * special number, it is DIGITS: in base 36 "nan" is 30191. The
			 * range helpers below exempt those three spellings because they
			 * cannot tell a keyword from a string, and the reader -- which
			 * can -- holds the quoted form to the width. Measure it here. */
			if (bvn_is_special_number_string(buf)) {
				bvn_int_t *n = bvn_int_alloc();
				bool fits = n && bvn_int_from_str(n, buf, base) &&
					(uint32_t)bvn_int_bitlen(n) <=
						(vt.family == vt_uint ? width : width - 1u);
				bvn_int_free(n);
				if (!fits) {
					ok = bvn_writer_set_error(w,
						error_value_out_of_range);
					goto out;
				}
			}
		} else if (vt.family == vt_float ||
			   vt.family == vt_float_fix ||
			   vt.family == vt_float_dec) {
			if (!bvn_validate_number_in_base(buf, base)) {
				ok = bvn_writer_set_error(w,
					error_digit_not_in_base);
				goto out;
			}
		}
		/* `width` is the EFFECTIVE width, so an annotation that names none is
		 * held to the 64 bits the reader reads it at. These two tests used to
		 * be skipped when no width was declared, and `<uint> 2^128` went out
		 * for the reader to refuse. */
		if (vt.family == vt_uint) {
			if (!bvn_validate_uint_range(buf, width, base)) {
				ok = bvn_writer_set_error(w,
					error_value_out_of_range);
				goto out;
			}
		} else if (vt.family == vt_sint || vt.family == vt_datetime) {
			/* datetime carrier is a signed integer (base 10); range-check like sint */
			if (!bvn_validate_sint_range(buf, width, base)) {
				ok = bvn_writer_set_error(w,
					error_value_out_of_range);
				goto out;
			}
		} else if (vt.family == vt_float_fix) {
			if (!bvn_float_str_fits_fix(buf, 10u, width,
									    bvn_effective_q(vt))) {
				ok = bvn_writer_set_error(w,
					error_value_out_of_range);
				goto out;
			}
		}
	}
out:
	if (need_free) free(buf);
	return ok;
}
static bool bvn_validate_symbol_for_writer(bvnr_writer_t* w,
	const void* data, uint32_t length)
{
	if (length == 0 || !data)
		return bvn_writer_set_error(w, error_empty_identifier);
	char  static_buf[256];
	char *buf = static_buf;
	bool  need_free = false;
	if (length >= sizeof(static_buf)) {
		if (length == UINT32_MAX)   /* see bvn_validate_id_for_writer */
			return bvn_writer_set_error(w, error_symbol_too_long);
		buf = malloc((size_t)length + 1u);
		if (!buf)
			return bvn_writer_set_error(w, error_symbol_too_long);
		need_free = true;
	}
	memcpy(buf, data, length);
	buf[length] = '\0';
	bool ok = true;
	if (!bvn_validate_symbol(buf))
		ok = bvn_writer_set_error(w, error_type_value_mismatch);
	/* A symbol whose text is a reserved keyword (null/true/false/on/off/
	 * nan/inf/ninf) is emitted bare and would be reclassified by the reader
	 * into a null/bool/float value, silently losing the symbol type. Symbols
	 * have no quoted form, so the only correct option is to reject it. */
	else if ((length == 4 && memcmp(buf, "null", 4) == 0) ||
	         (length == 4 && memcmp(buf, "true", 4) == 0) ||
	         (length == 2 && memcmp(buf, "on",   2) == 0) ||
	         (length == 5 && memcmp(buf, "false",5) == 0) ||
	         (length == 3 && memcmp(buf, "off",  3) == 0) ||
	         (length == 3 && memcmp(buf, "nan",  3) == 0) ||
	         (length == 3 && memcmp(buf, "inf",  3) == 0) ||
	         (length == 4 && memcmp(buf, "ninf", 4) == 0))
		ok = bvn_writer_set_error(w, error_type_value_mismatch);
	if (need_free) free(buf);
	return ok;
}
static bool bvn_validate_reference_for_writer(bvnr_writer_t* w,
	const void* data, uint32_t length)
{
	if (length == 0 || !data)
		return bvn_writer_set_error(w, error_invalid_argument);
	char static_buf[256];
	char *buf = static_buf;
	bool need_free = false;
	if (length >= sizeof(static_buf)) {
		if (length == UINT32_MAX)   /* see bvn_validate_id_for_writer */
			return bvn_writer_set_error(w, error_reference_too_long);
		buf = malloc((size_t)length + 1u);
		if (!buf)
			return bvn_writer_set_error(w, error_reference_too_long);
		need_free = true;
	}
	memcpy(buf, data, length);
	buf[length] = '\0';
	bool ok = true;
	if (!bvn_validate_reference(buf))
		ok = bvn_writer_set_error(w, error_type_value_mismatch);
	/* The "[N]" index suffix is a spec-1.1 construct and the lexer gates it on a
	 * DECLARED version, exactly as it gates the datetime family. Without the
	 * directive the reference reads back as error_unexpected_input_byte, so
	 * refuse here for the same reason and with the same error the datetime gate
	 * in bvn_validate_type_spec_for_writer uses -- the directive cannot be added
	 * retroactively, it has to precede every value. */
	if (ok && !w->ser.version_emitted && memchr(buf, '[', length) != NULL)
		ok = bvn_writer_set_error(w, error_unsupported_spec_version);
	if (need_free) free(buf);
	return ok;
}
/*
 * Validate a type annotation itself (independent of any value): width and base
 * must be legal for the family. These rules encode the format spec — e.g.
 * float widths are 16 or multiples of 32 up to the max precision; float_fix's
 * fractional-bit count q must be smaller than the total width; float_dec takes
 * no base; an explicit numeric base must be 2-62, 64 or 85 (the supported digit
 * alphabets); bool carries neither width nor base. Catching these here means a
 * malformed <...> can never reach the byte stream.
 */
static bool bvn_validate_type_spec_for_writer(bvnr_writer_t* w,
	value_type_spec_t vt)
{
	/* datetime is a spec-1.1 construct, and the reader gates the family on a
	 * DECLARED version. Writing one into a document with no "#!bovnar 1.1"
	 * directive produced something the library cannot read back, reported as
	 * success. There is no way to add the directive retroactively — it has to
	 * precede every value — so refuse instead, and point the caller at the flag
	 * that fixes it. */
	if (vt.family == vt_datetime && !w->ser.version_emitted)
		return bvn_writer_set_error(w, error_unsupported_spec_version);
	if (vt.family == vt_uint || vt.family == vt_sint) {
		if (vt.width > BVN_MAX_INT_WIDTH)
			return bvn_writer_set_error(w, error_illegal_value_type);
		/* Bases 64 and 85 use '+'/'-' as digit characters, leaving no sign
		 * character, so they are unsigned-only: reject signed integers. */
		if (vt.family == vt_sint && (vt.base == 64u || vt.base == 85u))
			return bvn_writer_set_error(w, error_illegal_value_type);
	}
	/* datetime carrier width has the same bound as an integer (base holds the
	 * epoch index, not a numeric base, so it is not range-checked here). */
	if (vt.family == vt_datetime && vt.width > BVN_MAX_INT_WIDTH)
		return bvn_writer_set_error(w, error_illegal_value_type);
	if (vt.family == vt_float) {
		if (vt.base != 0 && vt.base != 10 && vt.base != 16) {
			return bvn_writer_set_error(w,
				error_illegal_value_type);
		}
		if (vt.width != 0 && vt.width != 16 &&
		    (vt.width % 32u != 0u || vt.width > BVN_FLOAT_MAX_PREC)) {
			return bvn_writer_set_error(w,
				error_illegal_value_type);
		}
	}
	if (vt.family == vt_float_fix || vt.family == vt_float_dec) {
		if (vt.width != 0  && vt.width != 16  && vt.width != 32 &&
			vt.width != 64 && vt.width != 128 && vt.width != 256) {
			return bvn_writer_set_error(w, error_illegal_value_type);
		}
	}
	if (vt.family == vt_float_fix) {
		uint32_t eff_w = vt.width ? vt.width : 64u;
		if (vt.base >= eff_w)
			return bvn_writer_set_error(w, error_illegal_value_type);
	}
	if (vt.family == vt_float_dec) {
		if (vt.base != 0)
			return bvn_writer_set_error(w, error_illegal_value_type);
	}
	/*
	 * Numeral-base validity applies only to the families whose .base field
	 * is actually a numeral base (uint/sint/float). For float_fix the .base
	 * field holds Q (fractional bits, validated above against the width), and
	 * float_dec leaves it 0 (checked above); neither must be forced into the
	 * {2-62,64,85} numeral-base set, or valid specs like <float_fix:64,q63>
	 * would be wrongly rejected.
	 */
	if ((vt.family == vt_uint || vt.family == vt_sint ||
	     vt.family == vt_float) && vt.base != 0) {
		if (!((vt.base >= 2 && vt.base <= 62) ||
			  vt.base == 64 || vt.base == 85)) {
			return bvn_writer_set_error(w,
				error_illegal_value_type);
		}
	}
	if (vt.family == vt_bool || vt.family == vt_utf8) {
		/* bool and utf8 are parameterless families; the reader rejects a
		 * width or base on them (error_illegal_value_type), so the writer
		 * must too, or it would emit e.g. <utf8:40> that it can't read back. */
		if (vt.width != 0 || vt.base != 0)
			return bvn_writer_set_error(w, error_illegal_value_type);
	}
	return true;
}
static bool bvn_check_type_value_compat(bvnr_writer_t* w,
	token_type_t tt, value_type_spec_t vt)
{
	if (bvn_type_is_plain(vt) || tt == token_is_null_value)
		return true;
	if (vt.family == vt_utf8) {
		if (tt != token_is_string && tt != token_is_array_string) {
			return bvn_writer_set_error(w,
				error_type_value_mismatch);
		}
		return true;
	}
	if (vt.family == vt_bool) {
		if (tt != token_is_bool) {
			return bvn_writer_set_error(w,
				error_type_value_mismatch);
		}
		return true;
	}
	if (vt.family == vt_uint || vt.family == vt_sint ||
		vt.family == vt_float ||
		vt.family == vt_float_fix || vt.family == vt_float_dec) {
		if (tt != token_is_number && tt != token_is_array_number &&
			tt != token_is_string && tt != token_is_array_string) {
			return bvn_writer_set_error(w,
				error_type_value_mismatch);
		}
		return true;
	}
	return true;
}
/*
 * Stage 1 of bvnr_write_event: semantic validation of one event before any
 * bytes are produced. Besides per-kind value checks it enforces structural
 * invariants the serializer assumes — stream_start happens once, struct closes
 * are balanced, nesting depths stay within the configured caps — and snapshots
 * the active value_type/unit from annotation events so subsequent data events
 * can be range-checked against it. Returning false here (via
 * bvn_writer_set_error) aborts the write before the stream is corrupted.
 */
/*
 * Is a value legal right here? Either an assignment is waiting for one, or we
 * are inside an array row (whose elements are keyless), or an octet stream is
 * open. Anywhere else — bare at the top level, or directly inside a struct —
 * the bytes would be syntactically stranded.
 */
static inline bool bvn_ser_is_direct_array_element(
	const bvnr_serializer_t* s, uint32_t idx)
{
	if (idx >= s->max_array_nesting)
		return false;
	return s->struct_depth ==
		   s->struct_depth_at_array_start[idx];
}
static bool bvn_w_value_position(const bvnr_serializer_t* s)
{
	/* Inside an array, a value is only expected at a DIRECT element position;
	 * once a struct has been opened within the array we are in that struct, and
	 * a bare value there needs a key like anywhere else. */
	if (s->array_depth > 0 &&
	    bvn_ser_is_direct_array_element(s, s->array_depth - 1u))
		return true;
	return s->w_awaiting_value || s->in_octet_stream;
}
/* True where a key is legal: at the top level, or inside a struct — including a
 * struct nested inside an array, which is why this is not simply
 * "array_depth == 0". */
static bool bvn_w_key_position(const bvnr_serializer_t* s)
{
	if (s->array_depth == 0)
		return true;
	return !bvn_ser_is_direct_array_element(s, s->array_depth - 1u);
}
/* Has this document declared spec 1.2, the version the profile notation is
 * gated on? The directive cannot be added once a value is out. */
static bool bvn_w_declares_1_2(const bvnr_serializer_t* s)
{
	return s->version_emitted &&
	       (s->version_major > 1u ||
	        (s->version_major == 1u && s->version_minor >= 2u));
}
/*
 * What the probe reader saw: how many assignments and values the probe
 * document turned out to hold, and the unit and spelling of the last value.
 */
typedef struct bvn_usrc_probe_s {
	uint32_t	assignments;
	uint32_t	values;
	value_unit_t	unit;
	uint32_t	src_len;
	uint8_t		src[BVNR_UNIT_SOURCE_MAX + 1u];
} bvn_usrc_probe_t;
static bool bvn_usrc_probe_event(void* ud, bvnr_event_t ev, bvnr_data_t* d)
{
	bvn_usrc_probe_t* pr = ud;
	if (ev == ev_assignment_start) {
		pr->assignments++;
	} else if (ev == ev_data) {
		pr->values++;
		pr->unit    = d->value_unit;
		pr->src_len = 0;
		if (d->unit_source && d->unit_source_length <= BVNR_UNIT_SOURCE_MAX) {
			pr->src_len = d->unit_source_length;
			memcpy(pr->src, d->unit_source, d->unit_source_length);
		}
	}
	return true;
}
/*
 * Would a reader, handed `text` in this unit slot, read back exactly `text` as
 * exactly `unit`? Answered by doing it: a one-value probe document is built
 * around the text and parsed by the real reader.
 *
 * Asking bvn_parse_unit is not enough, and that is why this exists. A unit
 * slot is delimited by the LEXER, and the two slots disagree about where one
 * ends -- "ucum:mL{a,b}" and "udunits:kg m-2 s-1" are one parameter inside an
 * annotation and not a legal inline suffix at all, while "m> 1;.k=<uint:8,m"
 * parses as nothing and yet, pasted into an annotation, is a second assignment.
 * Restating the lexer's byte classes here would be a second copy of rules that
 * have already moved twice; running the lexer cannot drift from it. Every way
 * the text could escape its slot shows up as one of the conditions below.
 */
static bool bvn_writer_probe_unit_source(bvnr_writer_t* w,
	const uint8_t* text, uint32_t len, value_unit_t unit, bool inline_slot)
{
	static const char head_ann[] = "#!bovnar 1.2\n.k=<uint:";
	static const char tail_ann[] = ">1;";
	static const char head_inl[] = "#!bovnar 1.2\n.k=1 ";
	static const char tail_inl[] = ";";
	const char* head = inline_slot ? head_inl : head_ann;
	const char* tail = inline_slot ? tail_inl : tail_ann;
	char   doc[sizeof head_ann + BVNR_UNIT_SOURCE_MAX + sizeof tail_ann];
	size_t hl = strlen(head), tl = strlen(tail);
	memcpy(doc, head, hl);
	memcpy(doc + hl, text, len);
	memcpy(doc + hl + len, tail, tl);
	if (!w->usrc_probe) {
		w->usrc_probe = bvnr_reader_create();
		if (!w->usrc_probe) return false;
	}
	bvn_usrc_probe_t  pr    = {0};
	bvnr_read_flags_t flags = {0};
	flags.max_array_nesting = 1;
	flags.userdata    = &pr;
	flags.on_verified = bvn_usrc_probe_event;
	if (!bvnr_open_read_mem(w->usrc_probe, doc, hl + len + tl, NULL, 0, &flags))
		return false;
	if (!bvnr_read(w->usrc_probe))
		return false;
	if (BVN_UNIT_IS_UNITLESS(unit))
		return pr.assignments == 1u && pr.values == 1u &&
		       pr.src_len == 0u && BVN_UNIT_IS_UNITLESS(pr.unit);
	return pr.assignments == 1u && pr.values == 1u &&
	       pr.src_len == len && memcmp(pr.src, text, len) == 0 &&
	       bvn_unit_equal(pr.unit, unit);
}
/*
 * The text an event is about to put in a unit slot, checked before a byte of it
 * is written. A spelling is only ever a second way of saying value_unit, so it
 * is held to that: see bvnr_data_t.unit_source in bovnar.h for the contract.
 *
 * `inline_slot` says which of the two unit slots the text is headed for.
 */
static bool bvn_writer_check_unit_text(bvnr_writer_t* w,
	const void* src, uint32_t len, value_unit_t unit, bool inline_slot)
{
	if (len > BVNR_UNIT_SOURCE_MAX)
		return bvn_writer_set_error(w, error_unit_too_long);
	/* An annotation is capped as a whole, so there the spelling shares the
	 * reader's buffer with the family and every parameter before it.
	 *
	 * One document can be read and not written back for this reason: a bare
	 * value with an inline unit of some 243 bytes or more. The reader folds
	 * the unit into a synthesised `<float:64,_10,...>`, which is a dozen
	 * bytes longer than the slot it came from. Leaving such a unit to the
	 * value's inline slot instead was tried and withdrawn: at this point
	 * nothing knows whether the value CAN carry one (an array cannot, nor
	 * can nan/inf), and the wrong guess is an unreadable document. Refusing
	 * is the honest answer for a unit that long. */
	if (!inline_slot &&
	    bvn_ser_position(&w->ser) - w->ser.ann_start + 1u + len
	        > BVN_TYPE_BUF_CAP - 1u)
		return bvn_writer_set_error(w, error_unit_too_long);
	if (w->usrc_ok_len == len && w->usrc_ok_inline == inline_slot &&
	    memcmp(w->usrc_ok, src, len) == 0 &&
	    bvn_unit_equal(w->usrc_ok_unit, unit) &&
	    (!bvni_unit_has_profile((const char*)src, len) ||
	     bvn_w_declares_1_2(&w->ser)))
		return true;
	uint8_t text[BVNR_UNIT_SOURCE_MAX + 1u];
	memcpy(text, src, len);
	text[len] = 0;
	/* An embedded NUL would end the text early for every check below. */
	if (memchr(text, 0, len))
		return bvn_writer_set_error(w, error_unit_illegal);
	error_code_t ec = bvn_unit_error_code(text, len);
	if (ec != error_none)
		return bvn_writer_set_error(w, ec);
	bool         ok = false;
	value_unit_t pu = bvn_parse_unit(text, &ok);
	if (!ok)
		return bvn_writer_set_error(w, error_unit_illegal);
	if (BVN_UNIT_IS_UNITLESS(pu)) {
		/* "No unit" has two struct shapes (BVN_UNIT_IS_UNITLESS), one
		 * spelling, and no unit_source on the way back -- so it is compared
		 * by kind and by text, and there is nothing for the probe to do. */
		if (!BVN_UNIT_IS_UNITLESS(unit))
			return bvn_writer_set_error(w, error_unit_mismatch);
		if (!inline_slot && len == 7u && memcmp(text, "no_unit", 7u) == 0)
			return true;
		if (inline_slot)
			return bvn_writer_set_error(w, error_unit_illegal);
		if (bvni_unit_has_profile((const char*)text, len) &&
		    !bvn_w_declares_1_2(&w->ser))
			return bvn_writer_set_error(w, error_unsupported_spec_version);
		if (!bvn_writer_probe_unit_source(w, text, len, unit, false))
			return bvn_writer_set_error(w, error_unit_illegal);
		return true;
	}
	if (!bvn_unit_equal(pu, unit))
		return bvn_writer_set_error(w, error_unit_mismatch);
	if (bvni_unit_has_profile((const char*)text, len) &&
	    !bvn_w_declares_1_2(&w->ser))
		return bvn_writer_set_error(w, error_unsupported_spec_version);
	if (!bvn_writer_probe_unit_source(w, text, len, unit, inline_slot))
		return bvn_writer_set_error(w, error_unit_illegal);
	w->usrc_ok_inline = inline_slot;
	w->usrc_ok_len    = (uint8_t)len;
	memcpy(w->usrc_ok, text, len);
	w->usrc_ok_unit   = unit;
	return true;
}
/*
 * The unit parameter of a type annotation. Two fields can carry its text --
 * `data`, which is what the reader fills and every caller before 2.0 used, and
 * `unit_source` -- and BOTH are checked, because the serialiser writes whichever
 * it is given. Checking only the new field left the old one a way to write
 * "kg>1;.k=<uint:8,m" into an annotation whose value_unit said metres.
 */
static bool bvn_writer_check_unit_param(bvnr_writer_t* w, const bvnr_data_t* d)
{
	const void* text = d->data;
	uint32_t    len  = text ? d->length : 0u;
	bool        flags = w->ser.unit_flags != BVN_UNIT_FLAGS_NONE;
	/* The unit this text has to name: the event's own, or the one the
	 * annotation's start event gave when this event carries none. */
	value_unit_t unit = BVN_UNIT_IS_REAL(d->value_unit) ? d->value_unit
	                                                    : w->ann_unit;
	/* A datetime's parameter is its epoch, not a unit: the one text allowed is
	 * the epoch's own name. */
	if (d->value_type.family == vt_datetime) {
		const char* ep = bvnr_datetime_epoch_name(d->value_type);
		if (!ep || !len || len != strlen(ep) || memcmp(text, ep, len) != 0)
			return bvn_writer_set_error(w, error_illegal_value_type);
		return true;
	}
	/* unit_source is ignored under a unit flag and so is not looked at; with
	 * no flag it is what gets written, in preference to `data`. */
	if (!flags && d->unit_source && d->unit_source_length) {
		if (!bvn_writer_check_unit_text(w, d->unit_source,
				d->unit_source_length, unit, false))
			return false;
		/* `data` is then not written -- but if present it must not say
		 * something else, or the event describes two units. Only its
		 * MEANING is held to that: it never reaches the document, so the
		 * slot's length and lexical limits are not its concern. */
		if (!len || (len == d->unit_source_length &&
		             memcmp(text, d->unit_source, len) == 0))
			return true;
		{
			char tmp[BVNR_UNIT_STRING_MAX];
			if (len >= sizeof tmp)
				return bvn_writer_set_error(w, error_unit_too_long);
			memcpy(tmp, text, len);
			tmp[len] = 0;
			if (memchr(tmp, 0, len))
				return bvn_writer_set_error(w, error_unit_illegal);
			bool ok2 = false;
			value_unit_t pu2 = bvn_parse_unit((const uint8_t*)tmp, &ok2);
			if (!ok2)
				return bvn_writer_set_error(w, error_unit_illegal);
			if (BVN_UNIT_IS_UNITLESS(pu2) != BVN_UNIT_IS_UNITLESS(unit) ||
			    (BVN_UNIT_IS_REAL(pu2) && !bvn_unit_equal(pu2, unit)))
				return bvn_writer_set_error(w, error_unit_mismatch);
			return true;
		}
	}
	if (!len)
		return true;
	if (flags) {
		/* Under a unit flag the serialiser does not write `data`: it parses
		 * it and writes that unit's flagged form, and a caller who formatted
		 * the text with the same flags (bvnr_write_type_annotation does)
		 * hands over "m" for a value_unit of k~m. So the two are compared
		 * where they are supposed to meet -- in the flagged form -- and what
		 * goes out is the formatter's own text, which needs no probe. */
		char    tmp[BVNR_UNIT_STRING_MAX], a[BVNR_UNIT_STRING_MAX],
		        b[BVNR_UNIT_STRING_MAX];
		if (len >= sizeof tmp)
			return bvn_writer_set_error(w, error_unit_too_long);
		memcpy(tmp, text, len);
		tmp[len] = 0;
		if (memchr(tmp, 0, len))
			return bvn_writer_set_error(w, error_unit_illegal);
		bool         ok = false;
		value_unit_t pu = bvn_parse_unit((const uint8_t*)tmp, &ok);
		if (!ok) {
			error_code_t ec = bvn_unit_error_code((const uint8_t*)tmp, len);
			return bvn_writer_set_error(w,
				ec != error_none ? ec : error_unit_illegal);
		}
		if (BVN_UNIT_IS_UNITLESS(pu) || BVN_UNIT_IS_UNITLESS(unit))
			return BVN_UNIT_IS_UNITLESS(pu) &&
			       BVN_UNIT_IS_UNITLESS(unit)
				? true : bvn_writer_set_error(w, error_unit_mismatch);
		int32_t an = bvn_unit_to_string_ex(pu, a, sizeof a, w->ser.unit_flags);
		int32_t bn = bvn_unit_to_string_ex(unit, b, sizeof b,
						   w->ser.unit_flags);
		if (an < 0 || bn < 0)
			return bvn_writer_set_error(w, error_unit_illegal);
		if (an != bn || memcmp(a, b, (size_t)an) != 0)
			return bvn_writer_set_error(w, error_unit_mismatch);
		return true;
	}
	return bvn_writer_check_unit_text(w, text, len, unit, false);
}
/* An inline suffix: only unit_source can spell it, and only without a flag. */
static bool bvn_writer_check_inline_unit(bvnr_writer_t* w, const bvnr_data_t* d)
{
	if (!d->unit_source || !d->unit_source_length)   return true;
	if (w->ser.unit_flags != BVN_UNIT_FLAGS_NONE)    return true;
	if (!BVN_UNIT_IS_REAL(d->value_unit))            return true;
	return bvn_writer_check_unit_text(w, d->unit_source,
			d->unit_source_length, d->value_unit, true);
}
static bool bvn_w_family_takes_unit(value_type_family_t f)
{
	return f == vt_sint || f == vt_uint || f == vt_float ||
	       f == vt_float_fix || f == vt_float_dec;
}
/* A value (scalar, struct, octet stream) starts at a direct element position
 * of the innermost open row: count it. */
static void bvn_w_element_begins(bvnr_writer_t* w)
{
	const bvnr_serializer_t* s = &w->ser;
	if (s->array_depth > 0u &&
	    bvn_ser_is_direct_array_element(s, s->array_depth - 1u))
		w->arr_count[s->array_depth - 1u]++;
}
/* ...and it is complete: the next sibling is governed by the row's annotation
 * again (bvn_ser_mark_value_done does the same to emitted_unit). `struct_depth`
 * is the depth once the event has been applied. */
static void bvn_w_element_done(bvnr_writer_t* w, uint32_t struct_depth)
{
	const bvnr_serializer_t* s = &w->ser;
	if (s->array_depth > 0u && s->array_depth <= s->max_array_nesting &&
	    struct_depth == s->struct_depth_at_array_start[s->array_depth - 1u]) {
		w->gov_unit  = w->arr_gov_unit[s->array_depth - 1u];
		w->gov_typed = w->arr_gov_typed[s->array_depth - 1u];
		w->gov_type  = w->arr_gov_type[s->array_depth - 1u];
	} else if (s->array_depth == 0u) {
		w->gov_unit  = BVN_UNIT_NONE;
		w->gov_typed = false;
	}
}
/*
 * The type a number token with no annotation over it will be READ as: the
 * reader synthesises `<uint:64>`, `<sint:64>` or `<float:64>` from the text
 * (spec §10), whatever the event that wrote it claimed. `text` is the token,
 * NUL-terminated or not.
 */
static value_type_spec_t bvn_w_bare_number_type(const void* text, uint32_t len)
{
	const char* p = (const char*)text;
	bool is_float = false;
	char sb[8] = {0};
	if (p && len && len < sizeof sb) {
		memcpy(sb, p, len);
		if (bvn_is_special_number_string(sb))
			is_float = true;
	}
	for (uint32_t i = 0; p && i < len; i++)
		if (p[i] == '.' || p[i] == 'e' || p[i] == 'E')
			is_float = true;
	if (is_float)
		return BVN_TYPE_FLOAT(64);
	if (p && len && p[0] == '-')
		return BVN_TYPE_SINT(64);
	return BVN_TYPE_UINT(64);
}
static bool bvn_w_is_special_number(const void* text, uint32_t len)
{
	char sb[8] = {0};
	if (!text || !len || len >= sizeof sb)
		return false;
	memcpy(sb, text, len);
	return bvn_is_special_number_string(sb);
}
/*
 * The type a value is validated and serialised as: the governing annotation's
 * where there is one, and otherwise what the reader will infer. `typed` and
 * `gtype` are the governing state as it stood BEFORE the event was validated.
 */
static value_type_spec_t bvn_w_effective_type(const bvnr_data_t* d,
	bool typed, value_type_spec_t gtype)
{
	if (typed)
		return gtype;
	if (d->type == token_is_number || d->type == token_is_array_number)
		return bvn_type_is_plain(d->value_type)
			? bvn_w_bare_number_type(d->data, d->length)
			: d->value_type;
	return d->value_type;
}
static bool bvn_writer_validate_event(bvnr_writer_t* w,
	bvnr_event_t ev, bvnr_data_t* data)
{
	/*
	 * Event-ordering gate. Everything below assumes the stream has been opened,
	 * that annotation parameters arrive inside an annotation, and that a value
	 * appears only where one belongs. Without it the writer emitted "/" for a
	 * dimension outside an array, ".k=.k=1;" for two assignments in a row and
	 * ".k={1;};" for a value bare inside a struct — reporting success each time
	 * for output its own reader rejects.
	 */
	{
		bvnr_serializer_t* s = &w->ser;
		bool ok = true;
		switch (ev) {
		case ev_stream_start:
			break;                            /* checked below: once only */
		case ev_type_annotation_type_family:
		case ev_type_annotation_type_family_parameter:
		case ev_type_annotation_end:
			ok = s->w_in_annotation;
			break;
		case ev_type_annotation_start:
			ok = !s->w_in_annotation && bvn_w_value_position(s) &&
			     !s->in_octet_stream;
			break;
		case ev_assignment_start:
			/* A direct array element has no key, and an assignment cannot start
			 * while one is already waiting for its value. */
			ok = !s->w_in_annotation && !s->w_awaiting_value &&
			     bvn_w_key_position(s) && !s->in_octet_stream;
			break;
		case ev_data:
			ok = !s->w_in_annotation && bvn_w_value_position(s);
			/* An octet chunk is only meaningful inside an open stream; on its
			 * own the serializer still emitted a chunk header and the bytes,
			 * which the reader cannot parse. Conversely a normal value cannot
			 * appear in the middle of one. */
			if (ok) {
				bool is_oct = data && data->type == token_is_octet_stream;
				ok = (is_oct == s->in_octet_stream);
			}
			break;
		case ev_array_row_start:
			/* A row may open where a value is expected, or straight after a "/"
			 * that continues a multi-row array: [1,2]/[3,4]. */
			ok = !s->w_in_annotation && !s->in_octet_stream &&
			     (bvn_w_value_position(s) || s->w_after_dim);
			break;
		case ev_struct_start:
		case ev_octet_stream_start:
			ok = !s->w_in_annotation && bvn_w_value_position(s) &&
			     !s->in_octet_stream;
			break;
		case ev_array_row_end:
			/* The row can only close where one of its elements could stand:
			 * with a struct opened inside it still open this wrote "[{]". */
			ok = !s->w_in_annotation && !s->in_octet_stream &&
			     s->array_depth > 0 &&
			     bvn_ser_is_direct_array_element(s, s->array_depth - 1u);
			break;
		case ev_array_dim_start:
			/* The row separator sits BETWEEN two closed rows — [1,2]/[3,4] — so
			 * what it needs is a row that just closed, at any depth. Keying off
			 * array_depth instead let "[//]" through. */
			ok = !s->w_in_annotation && s->w_after_row_end;
			break;
		case ev_octet_stream_end:
			ok = s->in_octet_stream;
			break;
		case ev_struct_end:
			/* A struct may legitimately close inside an array element. */
			ok = !s->w_in_annotation && !s->w_awaiting_value &&
			     s->struct_depth > 0 && !s->in_octet_stream;
			/* ...but not across an array opened inside that struct. */
			if (ok && s->array_depth > 0 &&
			    bvn_ser_is_direct_array_element(s, s->array_depth - 1u))
				ok = false;
			break;
		case ev_stream_end:
			ok = !s->w_in_annotation && !s->w_awaiting_value &&
			     s->array_depth == 0 && s->struct_depth == 0 &&
			     !s->in_octet_stream;
			break;
		default:
			ok = false;
			break;
		}
		/* Once the stream has been closed nothing more may be written; the
		 * serializer would happily append past the end. */
		if (s->w_stream_ended)
			ok = false;
		/* A closed annotation is waiting for the value it types, and only a
		 * value may follow it: "[<uint:8>]" with no element behind it, or
		 * "[1]<uint:8>/[2]", is an annotation left dangling. */
		if (s->had_type_annotation && !s->w_in_annotation &&
		    ev != ev_data && ev != ev_struct_start &&
		    ev != ev_array_row_start && ev != ev_octet_stream_start)
			ok = false;
		/* A "/" promises another ROW, not a scalar: "[[]/null]" is not a thing. */
		if (s->w_after_dim && ev != ev_array_row_start)
			ok = false;
		/* No blanket "stream must be open" rule: ev_stream_start is optional —
		 * the bvnr_write_* helpers never send it, and an assignment opens the
		 * stream by itself. The per-event conditions above already reject the
		 * events that genuinely cannot come first. */
		if (!ok)
			return bvn_writer_set_error(w, error_unknown_token_type);
	}
	switch (ev) {
	case ev_stream_start:
		if (w->ser.stream_begun)
			return bvn_writer_set_error(w, error_invalid_argument);
		w->ser.stream_begun = true;
		return true;
	case ev_stream_end:
		w->ser.w_stream_ended = true;
		return true;
	case ev_assignment_start:
		w->ser.stream_begun = true;
		w->gov_unit  = BVN_UNIT_NONE;
		w->gov_typed = false;
		w->gov_type  = BVN_TYPE_PLAIN;
		/* A member of a struct that stands under a type annotation is what
		 * the reader refuses (error_type_value_mismatch). */
		if (w->ser.struct_depth > 0u &&
		    w->ser.struct_depth <= UINT8_MAX + 1u &&
		    w->struct_typed[w->ser.struct_depth - 1u])
			return bvn_writer_set_error(w, error_type_value_mismatch);
		if (!bvn_validate_id_for_writer(w, data->data, data->length))
			return false;
		/* A spelling set by bvnr_writer_set_unit_source is for the NEXT value.
		 * A second assignment starting with it still unused means that value
		 * took no unit (a string, a bool, a struct) -- the same caller mistake
		 * as setting one for a unitless number, and reported the same way,
		 * rather than left to land on whichever later value carries a unit.
		 * Counted here and not in the helpers, so it holds however the
		 * assignments in between were written. */
		if (w->val.unit_src_len && ++w->usrc_keys > 1u) {
			w->val.unit_src_len = 0;
			w->usrc_keys        = 0;
			return bvn_writer_set_error(w, error_unit_mismatch);
		}
		/* The key of the value about to be written — all a per-field rule needs
		 * from the event stream on this side, exactly as on the reader's. */
		if (w->val.policy.num_rules)
			bvn_path_set_key(&w->val.path,
					 (const uint8_t*)data->data, data->length);
		w->ser.w_awaiting_value = true;
		return true;
	case ev_type_annotation_start:
		w->ser.stream_begun     = true;
		w->ser.w_in_annotation  = true;
		w->val.value_type  = data->value_type;
		w->val.parsed_unit = data->value_unit;
		w->ann_unit        = data->value_unit;
		w->ann_family      = data->value_type.family;
		/* The type as WRITTEN is assembled from the events that write it:
		 * the family now (the family event confirms it), and the width,
		 * base and Q as their parameter events arrive. */
		w->ann_type.family = data->value_type.family;
		w->ann_type.width  = 0;
		w->ann_type.base   = 0;
		w->gov_unit        = BVN_UNIT_NONE;
		w->gov_typed       = false;
		w->gov_type        = BVN_TYPE_PLAIN;
		/*
		 * A unit with no native spelling — one carrying a UCUM arbitrary atom —
		 * can only be written in the spec-1.2 profile notation, which the reader
		 * gates on a DECLARED version. Emitting it under "#!bovnar 1.1", or
		 * under no directive at all, produces a document this library then
		 * refuses to read, reported to the caller as success. Same failure the
		 * datetime and reference-index guards above exist to prevent, and the
		 * same answer: refuse, because the directive cannot be added
		 * retroactively.
		 *
		 * A TRANSLATED unit needs no gate: ucum:mm[Hg] comes back out as the
		 * native mmHg, which every version accepts. Only the profile-only units
		 * force the notation.
		 */
		if (bvni_unit_has_opaque(data->value_unit) &&
			!bvn_w_declares_1_2(&w->ser))
			return bvn_writer_set_error(w, error_unsupported_spec_version);
		return bvn_validate_type_spec_for_writer(w,
			data->value_type);
	case ev_type_annotation_type_family_parameter:
		if (data->type == token_is_unit) {
			/* The annotation's unit may be stated on its start event, on
			 * this one, or both. A caller who sets it only at the start --
			 * the parameter events need nothing but their `type` -- is
			 * checked against that. */
			/* One unit (or epoch) per annotation: a second one wrote
			 * "<float:64,m,k~g>", which nothing reads. */
			if (w->ser.emitted_unit)
				return bvn_writer_set_error(w, error_illegal_value_type);
			if (BVN_UNIT_IS_REAL(data->value_unit)) {
				if (BVN_UNIT_IS_UNITLESS(w->ann_unit))
					w->ann_unit = data->value_unit;
				else if (!bvn_unit_equal(data->value_unit, w->ann_unit))
					/* The start event said one unit and this says
					 * another; the value is then held to the first
					 * while the second is what gets written. */
					return bvn_writer_set_error(w, error_unit_mismatch);
			}
			/* Only a numeric family takes a unit (a datetime's parameter
			 * is its epoch and is handled by the check below). */
			if (BVN_UNIT_IS_REAL(w->ann_unit) &&
			    !bvn_w_family_takes_unit(w->ann_family))
				return bvn_writer_set_error(w, error_illegal_value_type);
			/* The start event is not the only place the unit can first
			 * appear, so the spec-1.2 gate is repeated where it does. */
			if (bvni_unit_has_opaque(w->ann_unit) &&
			    !bvn_w_declares_1_2(&w->ser))
				return bvn_writer_set_error(w,
					error_unsupported_spec_version);
			if (!bvn_writer_check_unit_param(w, data))
				return false;
			/* A datetime's parameter is its epoch, checked just above to
			 * be this event's own. */
			if (data->value_type.family == vt_datetime)
				w->ann_type.base = data->value_type.base;
		} else if (data->type == token_is_type_width) {
			/* Mirrors the serialiser, which writes the EFFECTIVE value. */
			w->ann_type.width = bvn_effective_width(data->value_type);
		} else if (data->type == token_is_type_base) {
			/* For float_fix and datetime the field is Q or the epoch, and
			 * bvn_effective_base answers 10 for them by design. */
			if (w->ann_type.family == vt_uint ||
			    w->ann_type.family == vt_sint ||
			    w->ann_type.family == vt_float)
				w->ann_type.base = bvn_effective_base(data->value_type);
		} else if (data->type == token_is_type_q) {
			w->ann_type.base = bvn_effective_q(data->value_type);
		}
		/* fallthrough */
	case ev_type_annotation_type_family:
		if (ev == ev_type_annotation_type_family) {
			w->ann_family      = data->value_type.family;
			w->ann_type.family = data->value_type.family;
		}
		w->val.value_type  = data->value_type;
		w->val.parsed_unit = data->value_unit;
		break;
	case ev_type_annotation_end:
		w->ser.w_in_annotation = false;
		/* A unit named on the start event that no parameter wrote is written
		 * by bvnr_write_event before the ">" -- for a numeric family. Any
		 * other family has no unit parameter (`<utf8> "x" m` carries its unit
		 * inline, and the Python binding puts `m` on that start event), so
		 * there the annotation governs no unit at all. */
		w->gov_unit = (w->ser.emitted_unit ||
		               bvn_w_family_takes_unit(w->ann_family))
		              ? w->ann_unit : BVN_UNIT_NONE;
		w->gov_typed           = true;
		w->gov_type            = w->ann_type;
		/* The start event's type was checked when it arrived; what the
		 * parameter events then wrote is checked here, since they are free
		 * to say something else -- a width event carrying 7 under a start
		 * event that said float:64 wrote `<float:7>`. */
		if (!bvn_validate_type_spec_for_writer(w, w->ann_type))
			return false;
		break;
	case ev_data: {
		w->ser.stream_begun = true;
		/* The value the assignment was waiting for has arrived. An octet stream
		 * spans several data events, so it clears the flag at its end instead. */
		if (!w->ser.in_octet_stream)
			w->ser.w_awaiting_value = false;
		token_type_t tt = data->type;
		/*
		 * WHAT TYPE THIS VALUE HAS. Not the one its event claims: the one the
		 * document will say it has, which is the governing annotation's where
		 * there is one and otherwise what the reader infers from the token.
		 *
		 * Every check below used to run against data->value_type, and the
		 * annotation just written was never consulted. So `<uint:8>` followed
		 * by a data event typed uint:64 -- or left zero-initialised, which is
		 * plain and skips nearly everything -- wrote `<uint:8>300`,
		 * `<uint:8>1.5`, `<uint:8>"hello"`, `<utf8>5` and `<bool>"x"`, each
		 * reported as success and each refused by the reader. The unit of a
		 * value was made to answer to its annotation in an earlier pass; this
		 * is the same rule for its type.
		 *
		 * So under an annotation the event's own value_type is not consulted
		 * at all. It is not REFUSED for disagreeing either: nothing of it
		 * reaches the document, and existing producers state a carrier type
		 * there that is deliberately wider than the annotation (the Python
		 * binding types every array element at 64 bits or more). What the
		 * value must satisfy is the annotation, and that is now checked.
		 * An octet chunk is not a typed value and keeps what it was given.
		 */
		const bool typed = w->gov_typed && tt != token_is_octet_stream;
		value_type_spec_t vt = data->value_type;
		if (typed) {
			vt = w->gov_type;
			/* Under an annotation the reader takes a number, a string, a
			 * bool or a null -- never a symbol or a reference, whatever the
			 * family. bvn_check_type_value_compat lets them through for a
			 * plain type because that is what an UNannotated value has. */
			if (tt == token_is_symbol || tt == token_is_reference)
				return bvn_writer_set_error(w, error_type_value_mismatch);
		} else {
			vt = bvn_w_effective_type(data, false, vt);
		}
		if (!bvn_check_type_value_compat(w, tt, vt))
			return false;
		bool carries = tt == token_is_number || tt == token_is_array_number ||
		               tt == token_is_string || tt == token_is_array_string;
		/* Where the governing annotation wrote a unit parameter (a unit,
		 * `no_unit` or an epoch) the serialiser never looks at the value's
		 * unit. So the two must agree -- under its own annotation, as an
		 * element of an annotated array, or under `no_unit` -- or five metres
		 * go out as five kilograms. */
		if (carries && w->ser.emitted_unit &&
		    BVN_UNIT_IS_REAL(data->value_unit) &&
		    (BVN_UNIT_IS_UNITLESS(w->gov_unit) ||
		     !bvn_unit_equal(data->value_unit, w->gov_unit)))
			return bvn_writer_set_error(w, error_unit_mismatch);
		/* A unit the annotation did not carry is written as an inline suffix.
		 * That exists for a scalar only: inside "[...]" the reader takes no
		 * inline unit at all, so one there is a document nobody can read. */
		if (!w->ser.emitted_unit && BVN_UNIT_IS_REAL(data->value_unit) &&
		    carries) {
			if (w->ser.array_depth > 0u &&
			    bvn_ser_is_direct_array_element(&w->ser,
					w->ser.array_depth - 1u))
				return bvn_writer_set_error(w, error_unit_illegal);
			/* The inline slot is the other place a profile-only unit can
			 * reach the document; the annotation's gate does not see it. */
			if (bvni_unit_has_opaque(data->value_unit) &&
			    !bvn_w_declares_1_2(&w->ser))
				return bvn_writer_set_error(w,
					error_unsupported_spec_version);
			/* Nor does a special number take one (spec §6.4: "supply a
			 * unit through the type annotation instead"), nor a datetime,
			 * whose only parameter is its epoch. Both went out --
			 * `<float:64>nan k~m`, `<datetime:64>5 k~m` -- and neither
			 * reads back. */
			if ((tt == token_is_number || tt == token_is_array_number) &&
			    bvn_w_is_special_number(data->data, data->length))
				return bvn_writer_set_error(w, error_unit_illegal);
			if (vt.family == vt_datetime)
				return bvn_writer_set_error(w, error_unit_illegal);
			if (!bvn_writer_check_inline_unit(w, data))
				return false;
		}
		/*
		 * The producer's half of the unit policy (bvnr_writer_set_unit_policy).
		 * Refuse to EMIT what a reader under the same policy would refuse to
		 * accept — a numeric value carrying no unit, or one of the wrong
		 * quantity. This is the half the format's promise rests on: a reader can
		 * only reject a document a writer has already produced.
		 *
		 * The unit a value will carry is whichever of the two places it can come
		 * from actually has it. Normally that is data->value_unit; when the unit
		 * was given in the type annotation instead, the annotation's own event
		 * carried it and left it in val.parsed_unit, with ser.emitted_unit
		 * recording that it did. That flag is what makes the fallback safe:
		 * parsed_unit persists past the value it belonged to, and reading it
		 * unguarded would let one annotated value vouch for the next bare one.
		 *
		 * The fallback is gov_unit, the unit the serialiser is about to write
		 * this value under. It was val.parsed_unit, which is only "whatever
		 * the last annotation event carried" and was wrong three ways: an
		 * element annotation overwrote it for every later element of the
		 * array (`<float:64,m>[<float:64,s> 1, 2]` had its 2 checked as
		 * seconds), a unit named on the annotation's start event alone was
		 * forgotten by the events after it and the value refused as bare, and
		 * -- `vt` being the event's own type then -- a number whose event
		 * left the type plain was not looked at at all, which let `.a=5;`
		 * through require_unit.
		 */
		if (w->val.policy.active && bvn_type_is_numeric(vt) &&
		    (tt == token_is_number || tt == token_is_array_number ||
		     tt == token_is_string || tt == token_is_array_string)) {
			value_unit_t u = data->value_unit;
			if (BVN_UNIT_IS_UNITLESS(u) && w->ser.emitted_unit)
				u = w->gov_unit;
			if (!bvn_unit_policy_accepts(&w->val.policy, u))
				return bvn_writer_set_error(w, error_unit_mismatch);
			/* ...and what the policy says about THIS field specifically. The
			 * writer takes rules in require mode only (a convert rule is
			 * refused when the policy is set), so this is purely an
			 * assertion — no value is ever rewritten here. */
			int32_t ri = bvn_policy_match_rule(&w->val.policy, &w->val.path);
			/* The producing side carries the promise (doc/06 §5.2), so it
			 * refuses an undecidable rule for the same reason the reader does:
			 * a document nested past BVN_PATH_MAX_DEPTH leaves the position
			 * unknown, and an assertion that cannot be evaluated has not been
			 * met. Writing the document anyway would put the silence into the
			 * artefact rather than into one parse of it. */
			if (ri == BVN_POLICY_PATH_UNKNOWN)
				return bvn_writer_set_error(w, error_unit_mismatch);
			if (ri >= 0 &&
			    !bvn_policy_selects(u, w->val.policy.rule[ri].unit))
				return bvn_writer_set_error(w, error_unit_mismatch);
		}
		if (tt == token_is_null_value) {
		} else if (tt == token_is_number ||
				   tt == token_is_array_number) {
			/* A bare literal can only be read back in a base whose digits the
			 * lexer accepts unquoted. Above base 10 the digits are letters, which
			 * the grammar requires in a string literal — the reader even has
			 * error_base_requires_string_literal for it, but never got the chance
			 * because the writer emitted the bare token happily. */
			if (bvn_type_is_numeric(vt) && bvn_effective_base(vt) > 10u &&
			    data->data) {
				/* The bare-number lexer only accepts [0-9.+-eE]; a letter used
				 * as a digit in a base above ten has to arrive in a string
				 * literal. The writer used to emit "<uint:64,_16> 18F" bare and
				 * report success for a document the reader answers with
				 * unexpected_input_byte. "5" and "1e3" stay bare — they lex
				 * fine — so this rejects only what is genuinely unreadable.
				 * nan/inf/ninf are keywords, not digits, and are exempt. */
				char sb[8] = {0};
				bool special = data->length < sizeof sb && data->data &&
					(memcpy(sb, data->data, data->length),
					 bvn_is_special_number_string(sb));
				if (!special) {
					const char *p = (const char *)data->data;
					for (uint32_t i = 0; i < data->length; i++) {
						char ch = p[i];
						if ((ch >= '0' && ch <= '9') || ch == '.' ||
						    ch == '+' || ch == '-' ||
						    ch == 'e' || ch == 'E')
							continue;
						return bvn_writer_set_error(w,
							error_base_requires_string_literal);
					}
					/* ...and those bytes have to form a number TOKEN. 'e' is a
					 * digit above base 14, so "3e" and "310EE" passed the
					 * loop above and went out bare. */
					char  nbuf[64];
					char *nb = nbuf;
					if (data->length >= sizeof nbuf)
						nb = malloc((size_t)data->length + 1u);
					if (!nb)
						return bvn_writer_set_error(w, error_number_too_long);
					memcpy(nb, data->data, data->length);
					nb[data->length] = '\0';
					bool lexes = bvn_validate_number(nb);
					if (nb != nbuf) free(nb);
					if (!lexes)
						return bvn_writer_set_error(w,
							error_base_requires_string_literal);
				}
			}
			if (!bvn_validate_number_for_writer(w,
					data->data, data->length, vt))
				return false;
			/* With no annotation over it the token is read as the type its
			 * TEXT implies, at 64 bits, whatever this event says: a bare
			 * 2^128 is out of range to the reader even from an event typed
			 * uint:128. */
			if (!typed && !bvn_type_is_plain(data->value_type) &&
			    !bvn_validate_number_for_writer(w, data->data, data->length,
					bvn_w_bare_number_type(data->data, data->length)))
				return false;
		} else if (tt == token_is_string ||
				   tt == token_is_array_string) {
			/* The reader's max_string_length is a uint16_t, so no reader
			 * configuration can accept more than 65535 bytes. Writing a longer
			 * one produced a document nothing could read back, reported as
			 * success. Same reasoning as the identifier cap. */
			if (data->length > UINT16_MAX)
				return bvn_writer_set_error(w, error_string_too_long);
			if (!bvn_validate_string_content(w,
					(const uint8_t*)data->data, data->length))
				return false;
			if (bvn_type_is_numeric(vt)) {
				if (!bvn_validate_string_as_number(w,
						data->data, data->length, vt))
					return false;
			}
		} else if (tt == token_is_bool) {
			const char* bs = (const char*)data->data;
			uint32_t    bn = data->length;
			bool ok = bs && (
				(bn == 4 && memcmp(bs, "true",  4) == 0) ||
				(bn == 5 && memcmp(bs, "false", 5) == 0) ||
				(bn == 2 && memcmp(bs, "on",    2) == 0) ||
				(bn == 3 && memcmp(bs, "off",   3) == 0));
			if (!ok)
				return bvn_writer_set_error(w,
					error_type_value_mismatch);
		} else if (tt == token_is_symbol) {
			if (data->length > UINT16_MAX)
				return bvn_writer_set_error(w, error_symbol_too_long);
			if (!bvn_validate_symbol_for_writer(w,
					data->data, data->length))
				return false;
		} else if (tt == token_is_reference) {
			if (!bvn_validate_reference_for_writer(w,
					data->data, data->length))
				return false;
		} else if (tt == token_is_octet_stream) {
			if (data->length == 0 || data->length > 65536u)
				return bvn_writer_set_error(w,
					error_invalid_argument);
		}
		if (tt != token_is_octet_stream) {
			bvn_w_element_begins(w);
			bvn_w_element_done(w, w->ser.struct_depth);
		}
		break;
	}
	case ev_struct_end:
		w->ser.stream_begun = true;
		if (w->ser.struct_depth == 0)
			return bvn_writer_set_error(w, error_illegal_struct_close);
		if (w->val.policy.num_rules)
			bvn_path_pop(&w->val.path);
		bvn_w_element_done(w, w->ser.struct_depth - 1u);
		break;
	case ev_octet_stream_end:
		w->ser.stream_begun     = true;
		w->ser.w_awaiting_value = false;   /* the stream WAS the value */
		bvn_w_element_done(w, w->ser.struct_depth);
		break;
	case ev_octet_stream_start:
		w->ser.stream_begun = true;
		bvn_w_element_begins(w);
		break;
	case ev_struct_start:
		w->ser.stream_begun = true;
		if (w->ser.struct_depth >= (uint32_t)w->ser.max_struct_nesting)
			return bvn_writer_set_error(w, error_struct_nesting_too_high);
		bvn_w_element_begins(w);
		w->struct_typed[w->ser.struct_depth] = w->gov_typed;
		w->gov_unit  = BVN_UNIT_NONE;
		w->gov_typed = false;
		w->gov_type  = BVN_TYPE_PLAIN;
		if (w->val.policy.num_rules)
			bvn_path_push(&w->val.path);
		w->ser.w_awaiting_value = false;   /* the struct IS the value */
		break;
	case ev_array_row_start: {
		w->ser.stream_begun = true;
		if (w->ser.array_depth >= w->ser.max_array_nesting)
			return bvn_writer_set_error(w, error_array_nesting_too_high);
		/* A row's width as the READER measures it (bvn_action_array_intro /
		 * _outro): one per element position, and one more per nested row,
		 * a "/"-continued one included. The two sides have to count alike or
		 * this refuses what the reader takes. */
		uint32_t d = w->ser.array_depth;
		if (d > 0u && bvn_ser_is_direct_array_element(&w->ser, d - 1u))
			w->arr_count[d - 1u] += w->ser.w_after_dim ? 1u : 2u;
		w->arr_gov_unit[d]  = w->gov_unit;
		w->arr_gov_typed[d] = w->gov_typed;
		w->arr_gov_type[d]  = w->gov_type;
		w->arr_count[d]     = 0;
		if (!w->ser.w_after_dim)
			w->arr_dim_set[d] = false;
		w->ser.w_awaiting_value = false;   /* the array IS the value */
		w->ser.w_after_row_end  = false;
		w->ser.w_after_dim      = false;
		break;
	}
	case ev_array_row_end: {
		uint32_t d = w->ser.array_depth - 1u;   /* > 0: the gate checked */
		/* Rows joined by "/" have one width, and the first to close sets it. */
		if (w->arr_dim_set[d] && w->arr_dim_count[d] != w->arr_count[d])
			return bvn_writer_set_error(w, error_array_row_size_mismatch);
		w->arr_dim_set[d]   = true;
		w->arr_dim_count[d] = w->arr_count[d];
		w->ser.stream_begun    = true;
		w->ser.w_after_row_end = true;
		/* What the serialiser does to emitted_unit, done to the unit. */
		w->gov_unit  = w->arr_gov_unit[d];
		w->gov_typed = w->arr_gov_typed[d];
		w->gov_type  = w->arr_gov_type[d];
		if (d > 0u && bvn_ser_is_direct_array_element(&w->ser, d - 1u)) {
			w->gov_unit  = w->arr_gov_unit[d - 1u];
			w->gov_typed = w->arr_gov_typed[d - 1u];
			w->gov_type  = w->arr_gov_type[d - 1u];
		}
		break;
	}
	case ev_array_dim_start:
		w->ser.stream_begun    = true;
		w->ser.w_after_row_end = false;
		w->ser.w_after_dim     = true;
		if (w->ser.array_depth < w->ser.max_array_nesting) {
			w->gov_unit  = w->arr_gov_unit[w->ser.array_depth];
			w->gov_typed = w->arr_gov_typed[w->ser.array_depth];
			w->gov_type  = w->arr_gov_type[w->ser.array_depth];
		}
		break;
	default:
		w->ser.stream_begun = true;
		break;
	}
	return true;
}
/*
 * Emit a quoted string, escaping only the characters that must be escaped. The
 * run_start cursor batches every maximal span of literal (non-escaped) bytes
 * into a single push, so a clean string is written essentially as one memcpy
 * plus the surrounding quotes rather than byte-by-byte. UTF-8 multibyte
 * sequences pass through verbatim (the validate stage already proved they are
 * well-formed), which is why no per-byte UTF-8 handling is needed here.
 */
static bool bvn_ser_serialize_string(bvnr_serializer_t* s,
	const uint8_t* data, uint32_t len)
{
	if (!bvn_ser_push_byte(s, '"')) return false;
	uint32_t run_start = 0;
	for (uint32_t i = 0; i < len; i++) {
		uint8_t c = data[i];
		const char* esc = NULL;
		switch (c) {
		case '\t': esc = "\\t";  break;
		case '\n': esc = "\\n";  break;
		case '\v': esc = "\\v";  break;
		case '\f': esc = "\\f";  break;
		case '\r': esc = "\\r";  break;
		case '"':  esc = "\\\""; break;
		case '\\': esc = "\\\\"; break;
		default:   continue;
		}
		if (i > run_start) {
			if (!bvn_ser_push(s, data + run_start, i - run_start))
				return false;
		}
		if (!bvn_ser_push(s, esc, 2)) return false;
		run_start = i + 1u;
	}
	if (len > run_start) {
		if (!bvn_ser_push(s, data + run_start, len - run_start))
			return false;
	}
	return bvn_ser_push_byte(s, '"');
}
/*
 * Comma placement in arrays is subtle: a comma separates *array elements*, but
 * a struct or nested array opened as an element contains its own values that
 * must NOT get element-commas. We disambiguate by remembering the struct depth
 * at which each array level began (struct_depth_at_array_start). A value is a
 * "direct" element of array level idx only if the current struct depth equals
 * the depth recorded when that array opened; values produced while deeper
 * inside a struct are not direct elements and so don't arm the comma flag.
 *
 * bvn_ser_emit_pending_comma writes the comma due before the next element;
 * bvn_ser_mark_value_done arms the flag after a complete value is emitted.
 */
static bool bvn_ser_emit_pending_comma(bvnr_serializer_t* s)
{
	if (s->array_depth == 0)
		return true;
	uint32_t idx = s->array_depth - 1u;
	if (idx >= s->max_array_nesting)
		return true;
	if (!bvn_ser_is_direct_array_element(s, idx))
		return true;
	if (!s->arr_need_comma[idx])
		return true;
	if (!bvn_ser_push_byte(s, ','))
		return false;
	if (!bvn_ser_space(s))
		return false;
	s->arr_need_comma[idx] = false;
	return true;
}
static void bvn_ser_mark_value_done(bvnr_serializer_t* s)
{
	if (s->array_depth == 0)
		return;
	uint32_t idx = s->array_depth - 1u;
	if (idx >= s->max_array_nesting)
		return;
	if (!bvn_ser_is_direct_array_element(s, idx))
		return;
	s->arr_need_comma[idx] = true;
	/* The next sibling is governed by the row's annotation again, not by
	 * whatever annotation this element brought with it. */
	s->emitted_unit = s->arr_emitted_unit[idx];
}
/*
 * Stage 2: turn one (already-validated) event into bytes.
 *
 * This is the formatter that knows the concrete bovnar syntax: `.key = ` for
 * assignments, `<...>` type annotations with the `:`-then-`,` parameter
 * punctuation, `{}` structs, `[]` arrays with `/` dimension separators,
 * `nan`/`inf`/`ninf` bare keywords for special floats, `&` reference prefix, and the 0x00/0x01
 * binary octet-stream framing. Interleaved through every case is the separator
 * state machine (need_semi, pending commas, indentation) so output is
 * syntactically correct in both compact and pretty modes. had_type_annotation
 * suppresses the pending comma between a value's annotation and the value
 * itself, since they form one element.
 */
/*
 * spec 1.1 — decide whether a datetime value carrying captured ISO sub-second
 * digits should be re-emitted as an ISO literal (so the fraction, which the
 * whole-second integer carrier cannot hold, survives the round-trip), and if so
 * compute the civil UTC fields. Pure: produces no output, so the path is chosen
 * before the serializer writes any bytes.
 *
 * Returns false (caller emits the plain integer carrier) when the value is not a
 * datetime, has no fraction, the carrier text can't be parsed, or the civil year
 * falls outside the 0000..9999 an ISO literal's 4-digit year allows — the last
 * guard matters because the public writer API could be handed a fraction with an
 * out-of-literal-range carrier. The atomic GNSS epochs never reach here: they
 * reject ISO literals at read time and so never carry a fraction.
 */
/*
 * BVN_UNIT_REDUCE rescales the VALUE, not just the unit.
 *
 * bvn_unit_reduce folds every prefix into a scalar — km becomes m with scale
 * 1000 — and the formatter writes only the reduced unit. Emitting the value
 * unchanged beside it therefore wrote "5 km" as "5 m": the annotation said one
 * thing and the digits another, in a format whose whole premise is that a unit
 * confusion is the expensive failure. The value has to move with the unit.
 *
 * The rescale goes through the exact-rational conversion rather than the double
 * `scale`, so a wide value keeps every digit — the same guarantee the reader's
 * want_unit hook gives. Returns:
 *    1  text/len hold the rescaled value; write that instead of d->data
 *    0  nothing to do; write d->data verbatim
 *   -1  the reduced form cannot be written exactly; the caller must fail rather
 *       than emit a rounded number under an exact annotation
 *
 * The scale is always a power of ten or two (prefixes are all this touches), so
 * the digit growth is bounded by the prefix exponent and there is no unbounded
 * work here.
 */
/*
 * An inline unit ("<float:64> 9.81 m/s") lives on the VALUE, not the annotation,
 * so it was not emitted as a type parameter and has to be appended after the
 * literal. A unit given in the annotation set emitted_unit (skip, else it would
 * double); datetime carries no unit, so this is skipped for it.
 *
 * A HELPER rather than inline code, because it was inline in the token_is_number
 * branch and nowhere else -- so a QUOTED literal carrying an inline unit lost it
 * outright:
 *
 *     .a = <uint:64,_16> "7" k~m;   ->   .a = <uint:64,_16> "7";
 *
 * Seven kilometres rewritten as a bare, dimensionless seven, on an ordinary
 * pretty-print with no flags at all. A value losing its unit is the one outcome
 * this format exists to make impossible, and a quoted literal is not an exotic
 * shape: the spec REQUIRES it for every non-decimal base.
 */
static bool bvn_ser_append_inline_unit(bvnr_serializer_t *s,
				       const bvnr_data_t *d)
{
	if (s->emitted_unit)                              return true;
	if (d->value_unit.num_components == 0u)           return true;
	if (BVN_UNIT_IS_NO_UNIT(d->value_unit))           return true;
	/* The format has no inline unit inside "[...]". The validating writer
	 * refuses one before it gets here; the canon observer drives this
	 * function with no validation at all, so the rule is held here too. */
	if (s->array_depth > 0u &&
	    bvn_ser_is_direct_array_element(s, s->array_depth - 1u)) {
		s->ser_error = error_unit_illegal;
		return false;
	}
	/* The producer's own spelling, checked against value_unit already. */
	if (s->unit_flags == BVN_UNIT_FLAGS_NONE &&
	    d->unit_source && d->unit_source_length) {
		if (!bvn_ser_push_byte(s, ' ')) return false;
		return bvn_ser_push(s, d->unit_source, d->unit_source_length);
	}
	char ubuf[BVNR_UNIT_STRING_MAX];
	/* Honour the writer's unit_flags (reduce / ASCII-exponent) so a value-side
	 * inline unit canonicalises identically to one given in the annotation,
	 * which uses bvn_unit_to_string_ex too. A unit that overflows ubuf is a
	 * hard error, not a silent drop, or the value would lose its unit (and thus
	 * its meaning) on round-trip. */
	int32_t un = bvn_unit_to_string_ex(d->value_unit, ubuf, sizeof ubuf,
					   s->unit_flags);
	if (un > (int32_t)BVNR_UNIT_SOURCE_MAX) {
		s->ser_error = error_unit_too_long;
		return false;
	}
	if (un <= 0) {
		/* Say WHICH failure. Without this the refusal this comment describes
		 * was reported as whatever ser_error happened to hold. */
		s->ser_error = error_unit_illegal;
		return false;
	}
	/* BVN_UNIT_REDUCE cancelled the unit altogether (m/m, eV/eV). "No unit"
	 * as a suffix is "4 no_unit", which the next pass rewrites to "4": not a
	 * fixed point. Dimensionless is written as nothing. */
	if (un == 7 && memcmp(ubuf, "no_unit", 7) == 0)
		return true;
	if (!bvn_ser_push_byte(s, ' ')) return false;
	return bvn_ser_push(s, ubuf, (uint32_t)un);
}
/*
 * Does this literal overflow the IEEE binary format a `<float:width>` names --
 * i.e. does it round to an infinity there? Answered for the five interchange
 * widths; any other width has no fixed exponent range to leave.
 */
static bool bvn_ser_float_overflows(const char *text, uint32_t base,
				    uint32_t width)
{
	uint32_t exp_bits, man_bits, bits[8] = {0};
	int32_t  bias;
	int      n;
	switch (width) {
	case 16:  exp_bits = 5;  man_bits = 10;  bias = 15;     n = 1; break;
	case 32:  exp_bits = 8;  man_bits = 23;  bias = 127;    n = 1; break;
	case 64:  exp_bits = 11; man_bits = 52;  bias = 1023;   n = 2; break;
	case 128: exp_bits = 15; man_bits = 112; bias = 16383;  n = 4; break;
	case 256: exp_bits = 19; man_bits = 236; bias = 262143; n = 8; break;
	default:  return false;
	}
	bvn_float_strtoieee_bin(text, base == 16u ? 16u : 10u,
				exp_bits, man_bits, bias, bits, n);
	/* Sign, then the exponent field, at the top of the highest word; for
	 * binary16 the value sits in the low half of the one word there is. */
	uint32_t top   = bits[n - 1];
	uint32_t shift = (width == 16u ? 16u : 32u) - 1u - exp_bits;
	uint32_t mask  = (1u << exp_bits) - 1u;
	return ((top >> shift) & mask) == mask;
}
static int bvn_ser_reduced_number(bvnr_serializer_t *s, const bvnr_data_t *d,
				  char **text, int32_t *len)
{
	if (!(s->unit_flags & BVN_UNIT_REDUCE))                   return 0;
	if (d->value_unit.num_components == 0u)                   return 0;
	if (BVN_UNIT_IS_NO_UNIT(d->value_unit))                   return 0;
	if (!bvn_type_is_numeric(d->value_type))                  return 0;
	if (!d->data || !d->length)                               return 0;

	/* Cheap way out first. bvn_unit_reduce reports the scalar every prefix folds
	 * into; when that is 1 the value cannot move, whatever the formatter then
	 * does with the unit — the named-SI collapse only fires when there is a scale
	 * to absorb, and combining m·m into m² does not rescale anything. Most
	 * documents use unprefixed units, so this skips the format-and-reparse below
	 * for nearly every value. The overflow flag still has to fall through: an
	 * exponent folded past the representable range leaves scale at 1 while the
	 * unit has silently lost a dimension, and that must be refused. */
	{
		double scale = 1.0;
		bool   ovf   = false;
		(void)bvn_unit_reduce(d->value_unit, &scale, &ovf);
		if (scale == 1.0 && !ovf) return 0;
	}

	/* Convert to the unit that will actually be EMITTED, not to what
	 * bvn_unit_reduce returns. The formatter reduces and then re-attaches a
	 * prefix when the result lands on a named SI unit, so kN stays kN with no
	 * rescale at all — using bvn_unit_reduce's raw scale there would multiply by
	 * 1000 twice over. Round-tripping the emitted text is what keeps the digits
	 * and the annotation in step whatever the formatter decides. */
	char    ubuf[BVNR_UNIT_STRING_MAX];
	int32_t ulen = bvn_unit_to_string_ex(d->value_unit, ubuf, sizeof ubuf,
					     s->unit_flags);
	if (ulen < 0) { s->ser_error = error_unit_illegal; return -1; }
	bool         pok     = false;
	value_unit_t emitted = bvn_parse_unit((const uint8_t *)ubuf, &pok);
	if (!pok) { s->ser_error = error_unit_illegal; return -1; }
	if (bvn_unit_equal(emitted, d->value_unit)) return 0;   /* nothing moved */
	s->ser_reduced_unit = emitted;


	char  small[128];
	char *nb  = small;
	uint32_t n = d->length;
	if ((size_t)n + 1u > sizeof small) {
		nb = malloc((size_t)n + 1u);
		if (!nb) return -1;
	}
	memcpy(nb, d->data, n);
	nb[n] = '\0';
	/* nan/inf carry no finite value; a scale does not apply to them. */
	if (bvn_is_special_number_string(nb)) {
		if (nb != small) free(nb);
		return 0;
	}

	int rc = -1;
	/* Everything from here that fails means the reduced form cannot be written
	 * exactly. Say so specifically; the caller would otherwise report a sink
	 * problem, which is not what happened. */
	s->ser_error = error_value_out_of_range;
	uint32_t base = bvn_effective_base(d->value_type);
	bvn_int_t *vn = bvn_int_alloc(), *vd = bvn_int_alloc();
	bvn_int_t *on = bvn_int_alloc(), *od = bvn_int_alloc();
	char *out = NULL;
	if (!vn || !vd || !on || !od) goto done;
	if (d->value_type.family == vt_uint || d->value_type.family == vt_sint) {
		if (!bvn_int_from_str(vn, nb, base) || !bvn_int_from_uint64(vd, 1u))
			goto done;
	} else if (!bvn_float_parse_rational(nb, base == 16u ? 16u : 10u, vn, vd)) {
		goto done;
	}
	{
		bool exact = false;
		/* Incompatible here means the reduction lost something it should not
		 * have — an exponent folded past the representable range, say. Refuse
		 * rather than write digits under an annotation that no longer matches. */
		if (!bvn_unit_convert_rational(vn, vd, d->value_unit, emitted,
					       on, od, &exact) || !exact)
			goto done;
		size_t need = bvn_rational_str_bufsize(on, od, base);
		if (!need || need > (1u << 20)) goto done;
		out = malloc(need);
		if (!out) goto done;
		bool rexact = false;
		int32_t l = bvn_rational_to_str(on, od, base, out, need, &rexact);
		/* A scale that does not terminate in the value's own base — 1/100 written
		 * in base 16, say — cannot be expressed. Refuse; rounding here would be
		 * the silent loss this whole path exists to prevent. */
		if (l < 0 || !rexact) goto done;
		/* The rescaled value has to satisfy the SAME type it was declared with.
		 * bvn_writer_validate_event already approved the ORIGINAL text and runs
		 * before this, so without re-checking here the writer happily emitted
		 * "<uint:32,s> 0.02" (20 ms reduced to seconds is not an integer) and
		 * values orders of magnitude past the declared width — reporting success
		 * for a document its own reader rejects. Refusing is the only honest
		 * answer: the caller asked for a unit the value cannot be expressed in. */
		if (d->value_type.family == vt_uint || d->value_type.family == vt_sint) {
			for (int32_t i = 0; i < l; i++)
				if (out[i] == '.') goto done;      /* no longer an integer */
			uint32_t width = bvn_effective_width(d->value_type);
			bool fits = (d->value_type.family == vt_uint)
				  ? bvn_validate_uint_range(out, width, base)
				  : bvn_validate_sint_range(out, width, base);
			if (!fits) goto done;
		} else if (d->value_type.family == vt_float_fix) {
			/* float_fix has a declared RANGE too, and it is the tightest of the
			 * families: q16 in 32 bits leaves 15 integer bits. The check above
			 * covered only uint/sint, so a rescale that overflowed the Q format
			 * sailed through — <float_fix:32,q16,k~m> 30000 was written out as
			 * <float_fix:32,q16,m> 30000000, reported as success, and rejected by
			 * this library's own reader with error_value_out_of_range. Same
			 * predicate the validator applies on the way in, so the writer cannot
			 * emit what the reader will refuse. float/float_dec need no such
			 * test: neither pins a magnitude the rescale could leave. */
			if (!bvn_is_special_number_string(out) &&
			    !bvn_float_str_fits_fix(out, 10u,
						    bvn_effective_width(d->value_type),
						    bvn_effective_q(d->value_type)))
				goto done;
		} else if (d->value_type.family == vt_float) {
			/* "float needs no such test" was true of the reader, which does
			 * not range-check a float literal, and not of the VALUE: 3e38
			 * in a `<float:32,k~m>` is finite, and the 3e41 the rescale
			 * writes under `<float:32,m>` is an infinity to anything that
			 * decodes it at the declared width. A literal that was already
			 * past the range is left to the caller, as it always was; one
			 * the rescale pushed there is refused, like the integer above. */
			uint32_t fw = bvn_effective_width(d->value_type);
			if (bvn_ser_float_overflows(out, base, fw) &&
			    !bvn_ser_float_overflows(nb, base, fw))
				goto done;
		}
		/* Zero times any scale is zero, and the rational it went through has
		 * no sign to give back: `-0.0 k~m` came out as `0 m`. A float's
		 * negative zero is a value of its own, so put the sign back. */
		if (bvn_int_is_zero(on) && nb[0] == '-' &&
		    (d->value_type.family == vt_float ||
		     d->value_type.family == vt_float_dec ||
		     d->value_type.family == vt_float_fix)) {
			char *wider = realloc(out, (size_t)l + 2u);
			if (!wider) goto done;
			out = wider;
			memmove(out + 1, out, (size_t)l + 1u);
			out[0] = '-';
			l++;
		}
		/*
		 * BVN_NUM_CANONICAL: keep a rescaled float looking like a float.
		 * bvn_rational_to_str renders 5000/1 as "5000", so a reduced
		 * "<float:64,k~m> 5.0" came out as "5000" while a document already in
		 * metres kept its "5000.0" -- two spellings of one value, which is
		 * exactly what the canonical mode exists to prevent. Off by default, so
		 * plain BVN_UNIT_REDUCE keeps writing the bare exact value it always did.
		 *
		 * Base 10 only: a base-16 float marks its exponent with p/P and a bare
		 * integer mantissa is already its canonical spelling there.
		 */
		if ((s->unit_flags & BVN_NUM_CANONICAL) &&
		    (d->value_type.family == vt_float ||
		     d->value_type.family == vt_float_dec ||
		     d->value_type.family == vt_float_fix) &&
		    base == 10u && !bvn_is_special_number_string(out)) {
			bool has_marker = false;
			for (int32_t i = 0; i < l; i++)
				if (out[i] == '.' || out[i] == 'e' || out[i] == 'E') {
					has_marker = true;
					break;
				}
			if (!has_marker) {
				char *wider = realloc(out, (size_t)l + 3u);
				if (!wider) goto done;
				out = wider;
				out[l++] = '.';
				out[l++] = '0';
				out[l]   = '\0';
			}
		}
		*text = out;
		*len  = l;
		out   = NULL;
		rc    = 1;
		s->ser_error = error_none;
	}
done:
	free(out);
	if (nb != small) free(nb);
	bvn_int_free(vn); bvn_int_free(vd); bvn_int_free(on); bvn_int_free(od);
	return rc;
}
static bool bvn_ser_datetime_to_civil(const bvnr_data_t* d, bvn_datetime_t* dt,
                                      bool* year_out_of_range)
{
	if (year_out_of_range) *year_out_of_range = false;
	if (d->value_type.family != vt_datetime ||
	    d->frac_data == NULL || d->frac_length == 0 || d->data == NULL)
		return false;
	/* The fraction is spliced verbatim into the reconstructed ISO literal, so it
	 * must be pure ASCII digits. The reader only ever captures digits, but the
	 * public bvnr_write_event() API could hand us anything; reject (and fall back
	 * to the integer carrier) rather than emit a literal that will not re-parse. */
	const uint8_t* fd = (const uint8_t*)d->frac_data;
	for (uint32_t i = 0; i < d->frac_length; i++)
		if (fd[i] < '0' || fd[i] > '9')
			return false;
	char numbuf[24];
	if (d->length == 0 || d->length >= sizeof numbuf)
		return false;
	memcpy(numbuf, d->data, d->length);
	numbuf[d->length] = '\0';
	int64_t secs;
	if (!bvn_parse_int64(numbuf, d->value_type, &secs))
		return false;
	/* Only the epochs whose ISO literal the reader accepts may be re-emitted as a
	 * literal. The atomic GNSS epochs reject literals at read time, so they never
	 * carry a reader-captured fraction; one supplied via the writer API falls back
	 * to the integer carrier rather than producing a literal the reader rejects.
	 *
	 * Selected on the epoch's IDENTITY: bvnr_datetime_epoch_mjd returns the
	 * bvn_epoch_t, and those are distinct across the table. The name is a display
	 * string and was never the right key for this. */
	const int32_t epoch = bvnr_datetime_epoch_mjd(d->value_type);
	if (epoch == bvn_epoch_gps     || epoch == bvn_epoch_galileo ||
	    epoch == bvn_epoch_glonass || epoch == bvn_epoch_beidou)
		return false;
	/* Zero-init so the validity guard is deterministic even when a converter
	 * leaves *dt untouched: bvn_dt_tai_seconds_to_utc() does so on the tai
	 * underflow near INT64_MIN, and bvn_dt_epoch_seconds_to_datetime() leaves
	 * dt->date untouched when the MJD is outside the gregorian range. The fields
	 * then stay zero (month 0) and the guard below rejects them. */
	memset(dt, 0, sizeof *dt);
	if (epoch == bvn_epoch_tai)
		bvn_dt_tai_seconds_to_utc(dt, secs);
	else
		bvn_dt_epoch_seconds_to_datetime(dt, (bvn_epoch_t)epoch, secs);
	/* A well-formed civil time the reader can round-trip has month 1..12, a valid
	 * day, and a 4-digit year; reject anything else (including the untouched-zero
	 * case above) and emit the plain integer carrier instead.
	 *
	 * The year check is singled out because the fraction has nowhere to go if it
	 * fails: everything above (a non-digit fraction, a GNSS epoch, an unparseable
	 * carrier) describes a value the reader could never have produced, so the
	 * caller supplied it and the integer fallback is the right answer, whereas
	 * dropping sub-second digits the spec promises to round-trip is not.
	 *
	 * The reader now refuses such a literal too (bvn_iso_to_epoch_seconds), so a
	 * parsed document can no longer reach this — it is left in place for the
	 * public bvnr_write_event() path, where a caller can still hand us a fraction
	 * against an out-of-range carrier directly. */
	bool civil_ok = dt->date.month >= 1 && dt->date.month <= 12 &&
			dt->date.day   >= 1 && dt->date.day   <= 31;
	if (year_out_of_range)
		*year_out_of_range = civil_ok &&
				     (dt->date.year < 0 || dt->date.year > 9999);
	return civil_ok && dt->date.year >= 0 && dt->date.year <= 9999;
}
bool bvn_ser_serialize_event(bvnr_serializer_t* s,
	bvnr_event_t ev, bvnr_data_t* d)
{
	switch (ev) {
	case ev_stream_start:
		break;
	case ev_stream_end:
		break;
	case ev_assignment_start:
		if (s->need_semi) {
			if (!bvn_ser_push_byte(s, ';')) return false;
			if (!bvn_ser_newline(s)) return false;
		}
		if (!bvn_ser_indent(s)) return false;
		if (!bvn_ser_push_byte(s, '.')) return false;
		if (d->data && d->length) {
			if (!bvn_ser_push(s, d->data, d->length))
				return false;
		}
		if (!bvn_ser_space(s)) return false;
		if (!bvn_ser_push_byte(s, '=')) return false;
		if (!bvn_ser_space(s)) return false;
		s->need_semi = false;
		s->had_type_annotation = false;
		s->emitted_type_param = false;
		s->emitted_unit = false;
		break;
	case ev_type_annotation_start:
		if (!bvn_ser_emit_pending_comma(s))
			return false;
		s->had_type_annotation = true;
		s->emitted_type_param = false;
		s->emitted_unit = false;
		if (!bvn_ser_push_byte(s, '<')) return false;
		s->ann_start = bvn_ser_position(s);
		break;
	case ev_type_annotation_type_family: {
		const char* fname;
		switch (d->value_type.family) {
		case vt_uint:      fname = "uint";      break;
		case vt_sint:      fname = "sint";      break;
		case vt_float:     fname = "float";     break;
		case vt_float_fix: fname = "float_fix"; break;
		case vt_float_dec: fname = "float_dec"; break;
		case vt_utf8:      fname = "utf8";      break;
		case vt_bool:      fname = "bool";      break;
		case vt_datetime:  fname = "datetime";  break;
		default:           fname = "plain";     break;
		}
		if (!bvn_ser_push_str(s, fname))
			return false;
		break;
	}
	case ev_type_annotation_type_family_parameter: {
		bool use_colon = !s->emitted_type_param;
		if (d->type == token_is_type_width) {
			char buf[12];
			if (!bvn_ser_push_byte(s, ':')) return false;
			int32_t n = bvn_format_uint64(buf, sizeof(buf),
				bvn_effective_width(d->value_type), 10u, 0u);
			if (n < 0) return false;
			if (!bvn_ser_push(s, buf, (uint32_t)n))
				return false;
			s->emitted_type_param = true;
		} else if (d->type == token_is_type_base) {
			char buf[12];
			if (!bvn_ser_push_byte(s, use_colon ? ':' : ',')) return false;
			if (!bvn_ser_push_byte(s, '_')) return false;
			int32_t n = bvn_format_uint64(buf, sizeof(buf),
				bvn_effective_base(d->value_type), 10u, 0u);
			if (n < 0) return false;
			if (!bvn_ser_push(s, buf, (uint32_t)n))
				return false;
			s->emitted_type_param = true;
		} else if (d->type == token_is_type_q) {
			char buf[12];
			if (!bvn_ser_push_byte(s, use_colon ? ':' : ',')) return false;
			if (!bvn_ser_push_byte(s, 'q')) return false;
			int32_t n = bvn_format_uint64(buf, sizeof(buf),
				bvn_effective_q(d->value_type), 10u, 0u);
			if (n < 0) return false;
			if (!bvn_ser_push(s, buf, (uint32_t)n))
				return false;
			s->emitted_type_param = true;
		} else if (d->type == token_is_unit) {
			/* The unit's text: what the event carries in `data`, or -- for a
			 * caller that set only bvnr_data_t.unit_source -- that. Under a
			 * unit flag the spelling is ignored by contract and unchecked, so
			 * there the text is taken from value_unit instead. */
			const void *utext = d->data;
			uint32_t    utlen = d->length;
			char        cbuf[BVNR_UNIT_STRING_MAX];
			if (!(utext && utlen)) {
				if (s->unit_flags == BVN_UNIT_FLAGS_NONE &&
				    d->unit_source && d->unit_source_length) {
					utext = d->unit_source;
					utlen = d->unit_source_length;
				} else if (BVN_UNIT_IS_REAL(d->value_unit)) {
					/* No text at all, only the unit: write its canonical
					 * form. This used to emit the separator and nothing
					 * after it -- "<float:64,>" -- and report success. */
					int32_t cn = bvn_unit_to_string_ex(
						d->value_unit, cbuf, sizeof cbuf, s->unit_flags);
					if (cn < 0) {
						s->ser_error = error_unit_illegal;
						return false;
					}
					utext = cbuf;
					utlen = (uint32_t)cn;
				}
			}
			if (utext && utlen) {
				/* Apply unit_flags here too. On the reader-driven path
				 * (pretty-print, canonicalise) this token carries the unit
				 * TEXT the reader saw, and pushing it verbatim left the
				 * annotation unreduced while bvn_ser_reduced_number scaled
				 * the value against the reduced unit — writing "5 km" as
				 * "5000 km", the very confusion the rescale exists to
				 * prevent, and compounding on every further pass. The
				 * annotation and the inline-unit path must agree, so both
				 * go through bvn_unit_to_string_ex. Text that will not
				 * parse back (a unit this build does not know) is passed
				 * through unchanged rather than dropped. */
				const uint8_t *ut = (const uint8_t *)utext;
				char           ubuf[BVNR_UNIT_STRING_MAX];
				const char    *emit = (const char *)utext;
				uint32_t       elen = utlen;
				/* library 2.0 — the producer's own spelling, where one was
				 * handed over (bvnr_data_t.unit_source) and no unit flag
				 * asks for a different one. It has been checked against
				 * value_unit by the time it gets here. */
				if (s->unit_flags == BVN_UNIT_FLAGS_NONE &&
				    d->unit_source && d->unit_source_length) {
					emit = (const char *)d->unit_source;
					elen = d->unit_source_length;
				}
				if (s->unit_flags != BVN_UNIT_FLAGS_NONE &&
				    utlen < sizeof ubuf) {
					/* Sized by the guard above. This was char[512] under
					 * a guard of 1088, so a unit text of 512..1087 bytes
					 * ran off the end of it. */
					char tmp[BVNR_UNIT_STRING_MAX];
					memcpy(tmp, ut, utlen);
					tmp[utlen] = '\0';
					bool pok = false;
					value_unit_t pu =
						bvn_parse_unit((const uint8_t *)tmp, &pok);
					if (pok) {
						int32_t un = bvn_unit_to_string_ex(
							pu, ubuf, sizeof ubuf, s->unit_flags);
						/* Returning false without SETTING an error leaves the
						 * writer reporting whatever code was already in
						 * ser_error -- a unit that cannot be written surfaced
						 * as "sink_buffer_exhausted", sending the caller after
						 * a buffer that was never involved. */
						if (un < 0) {
							s->ser_error = error_unit_illegal;
							return false;
						}
						emit = ubuf;
						elen = (uint32_t)un;
					}
				}
				if (!bvn_ser_push_byte(s, use_colon ? ':' : ','))
					return false;
				if (!bvn_ser_push(s, emit, elen))
					return false;
			} else {
				/* Neither a text nor a unit: there is no parameter to
				 * write, and a bare separator is not an annotation. */
				break;
			}
			s->emitted_type_param = true;
			s->emitted_unit = true;
		}
		break;
	}
	case ev_type_annotation_end:
		/* The reader holds an annotation's body in a fixed buffer. Longer
		 * than that and this document cannot be read back, by anyone. */
		if (bvn_ser_position(s) - s->ann_start > BVN_TYPE_BUF_CAP - 1u) {
			s->ser_error = error_type_too_long;
			return false;
		}
		if (!bvn_ser_push_byte(s, '>')) return false;
		if (!bvn_ser_space(s)) return false;
		break;
	case ev_struct_start:
		if (!s->had_type_annotation) {
			if (!bvn_ser_emit_pending_comma(s))
				return false;
		}
		s->had_type_annotation = false;
		if (!bvn_ser_push_byte(s, '{')) return false;
		if (!bvn_ser_newline(s)) return false;
		s->indent++;
		s->need_semi = false;
		s->struct_depth++;
		break;
	case ev_struct_end:
		/* Self-protect against an unbalanced struct-end, mirroring the array-depth
		 * guard below: the canon-observer path drives this serializer with no
		 * validation, so without a matching struct-start indent/struct_depth would
		 * underflow and the next bvn_ser_indent would emit ~4 billion tab bytes. */
		if (s->struct_depth == 0)
			return false;
		s->indent--;
		if (s->need_semi) {
			if (!bvn_ser_push_byte(s, ';')) return false;
			if (!bvn_ser_newline(s)) return false;
		}
		if (!bvn_ser_indent(s)) return false;
		if (!bvn_ser_push_byte(s, '}')) return false;
		s->need_semi = true;
		s->struct_depth--;
		bvn_ser_mark_value_done(s);
		break;
	case ev_array_row_start: {
		/* Self-protect the fixed-size depth arrays. The validating writer path
		 * already rejects over-deep nesting (bvn_writer_validate_event), but the
		 * canon-observer path drives this serializer directly with no validation,
		 * so the bound is enforced here too rather than trusting the event source. */
		if (s->array_depth >= s->max_array_nesting)
			return false;
		if (!s->had_type_annotation) {
			if (!bvn_ser_emit_pending_comma(s))
				return false;
		}
		s->had_type_annotation = false;
		s->struct_depth_at_array_start[s->array_depth] = s->struct_depth;
		s->arr_need_comma[s->array_depth] = false;
		s->arr_emitted_unit[s->array_depth] = s->emitted_unit;
		if (!bvn_ser_push_byte(s, '[')) return false;
		s->array_depth++;
		break;
	}
	case ev_array_row_end:
		if (s->array_depth > 0)
			s->array_depth--;
		/* Back to what governed the row, so a "/"-continued row and the
		 * enclosing array's next element both see the right annotation. */
		if (s->array_depth < s->max_array_nesting)
			s->emitted_unit = s->arr_emitted_unit[s->array_depth];
		if (!bvn_ser_push_byte(s, ']')) return false;
		s->need_semi = true;
		bvn_ser_mark_value_done(s);
		break;
	case ev_array_dim_start:
		if (s->pretty) {
			if (!bvn_ser_push_str(s, " / ")) return false;
		} else {
			if (!bvn_ser_push_str(s, "/")) return false;
		}
		if (s->array_depth > 0) {
			uint32_t idx = s->array_depth - 1u;
			if (idx < s->max_array_nesting &&
				bvn_ser_is_direct_array_element(s, idx)) {
				s->arr_need_comma[idx] = false;
			}
		}
		if (s->array_depth < s->max_array_nesting)
			s->emitted_unit = s->arr_emitted_unit[s->array_depth];
		break;
	case ev_octet_stream_start:
		if (!s->had_type_annotation) {
			if (!bvn_ser_emit_pending_comma(s))
				return false;
		}
		s->had_type_annotation = false;
		if (!bvn_ser_push_byte(s, 0x00)) return false;
		s->in_octet_stream = true;
		break;
	case ev_octet_stream_end:
		if (!bvn_ser_push_byte(s, 0x00)) return false;
		s->in_octet_stream = false;
		s->need_semi = true;
		bvn_ser_mark_value_done(s);
		break;
	case ev_data:
		/* A chunk is PART of the element its octet stream is, not an element.
		 * ev_octet_stream_start wrote the separator the stream is owed and
		 * ev_octet_stream_end marks it done; treating each chunk as a value
		 * of its own armed the comma after the first one, so a stream of two
		 * chunks inside an array went out as 0x00 chunk "," chunk 0x00 --
		 * a byte the framing does not have, and octet_stream_out_of_sync to
		 * the reader. Outside an array nothing arms a comma, which is why the
		 * same events wrote correctly at the top level and in a struct. */
		if (d->type == token_is_octet_stream) {
			if (d->data && d->length) {
				if (d->length > 65536u)
					return false;
				uint8_t hdr[3];
				uint32_t clen = d->length;
				hdr[0] = 0x01;
				if (clen == 65536u) clen = 0;
				hdr[1] = (uint8_t)(clen & 0xFFu);
				hdr[2] = (uint8_t)((clen >> 8) & 0xFFu);
				if (!bvn_ser_push(s, hdr, 3))
					return false;
				if (!bvn_ser_push(s, d->data, d->length))
					return false;
			}
			break;
		}
		if (!s->had_type_annotation) {
			if (!bvn_ser_emit_pending_comma(s))
				return false;
		}
		s->had_type_annotation = false;
		if (d->type == token_is_null_value) {
			/* A null *direct array element* is written as the explicit "null"
			 * keyword. A null can no longer be an empty slot: an empty slot with
			 * no delimiting comma is now an empty array "[]" (zero elements), so
			 * "[null]" (one null) must stay distinct from "[]". Emitting "null"
			 * uniformly keeps every position unambiguous and round-trip-safe.
			 * Top-level nulls (".x = ;") and struct-field nulls are unaffected. */
			if (s->array_depth > 0 &&
				bvn_ser_is_direct_array_element(s, s->array_depth - 1u)) {
				if (!bvn_ser_push_str(s, "null"))
					return false;
			}
		} else if (d->type == token_is_number ||
				   d->type == token_is_array_number) {
			bvn_datetime_t dt;
			/* A datetime that carries sub-second digits but whose UTC civil
			 * year falls outside 0000-9999 cannot be re-emitted as an ISO
			 * literal — and the integer-carrier fallback below has nowhere to
			 * put the fraction, so it used to vanish. The spec promises those
			 * digits round-trip; dropping them silently is the one thing that
			 * must not happen. Reachable when a tz offset pushes the UTC year
			 * past either end: "0000-01-01T00:00:00.5+23:59" parses (its LOCAL
			 * year is in range) but its UTC year is -1. */
			bool dt_year_oor = false;
			if (bvn_ser_datetime_to_civil(d, &dt, &dt_year_oor)) {
				/* spec 1.1 — re-emit as an ISO literal so the captured
				 * sub-second digits round-trip:
				 * YYYY-MM-DDTHH:MM:SS.<frac>Z (always UTC). */
				char head[24];
				int hn = snprintf(head, sizeof head,
					"%04lld-%02lld-%02lldT%02lld:%02lld:%02lld",
					(long long)dt.date.year, (long long)dt.date.month,
					(long long)dt.date.day, (long long)dt.hour,
					(long long)dt.minute, (long long)dt.second);
				if (hn < 0 || (size_t)hn >= sizeof head)
					return false;
				if (!bvn_ser_push(s, head, (uint32_t)hn)) return false;
				if (!bvn_ser_push_byte(s, '.')) return false;
				if (!bvn_ser_push(s, d->frac_data, d->frac_length))
					return false;
				if (!bvn_ser_push_byte(s, 'Z')) return false;
			} else if (dt_year_oor) {
				/* The value is fine; its UTC year simply has no ISO
				 * spelling, and the integer carrier below cannot carry
				 * the fraction. Refuse instead of losing it.
				 *
				 * The reader refuses such a literal too now, so a parsed
				 * document no longer reaches this — only a caller driving
				 * bvnr_write_event() directly can. */
				s->ser_error = error_invalid_datetime_literal;
				return false;
			} else if (d->data && d->length) {
				/* Special floats (nan/inf/ninf) are emitted as the
				 * bare keyword, exactly like any other number token —
				 * no sigil. */
				char   *rtext = NULL;
				int32_t rlen  = 0;
				int     rr    = bvn_ser_reduced_number(s, d, &rtext, &rlen);
				if (rr < 0)
					return false;
				if (rr > 0) {
					/* The rescaled digits are new text, and above base ten
					 * they can be letters where the original had none:
					 * "<uint:64,_16,k~m> 3" is bb8 metres, and "bb8" bare is
					 * not a number token. Quote it, as §4.6 requires. */
					bool bare = bvn_effective_base(d->value_type) <= 10u ||
						    bvn_is_special_number_string(rtext) ||
						    bvn_validate_number(rtext);
					bool okp = bare
						? bvn_ser_push(s, rtext, (uint32_t)rlen)
						: bvn_ser_serialize_string(s,
							(const uint8_t*)rtext, (uint32_t)rlen);
					/* Hand it to bvnr_write_event rather than dropping it:
					 * an observer told the pre-rescale digits is out of
					 * sync with the bytes it is meant to mirror. */
					free(s->ser_value_text);
					s->ser_value_text = rtext;
					s->ser_value_len  = (uint32_t)rlen;
					s->ser_value_unit = s->ser_reduced_unit;
					if (!okp) return false;
				} else if (!bvn_ser_push(s, d->data, d->length)) {
					return false;
				}
			}
			if (!bvn_ser_append_inline_unit(s, d)) return false;
		} else if (d->type == token_is_string ||
				   d->type == token_is_array_string) {
			/*
			 * A QUOTED NUMBER LITERAL is a string token carrying a numeric
			 * annotation, and it has to be rescaled by BVN_UNIT_REDUCE exactly
			 * like a bare one. The rescale used to live only under
			 * token_is_number, while the ANNOTATION is written by the common
			 * code above and reduced there regardless — so the unit lost its
			 * prefix and the digits stayed put:
			 *
			 *     <uint:64,_10,k~m> "7"      ->  <uint:64,_10,m> "7"
			 *     <float:64,_16,k~m> "1p0"   ->  <float:64,_16,m> "1p0"
			 *
			 * Seven kilometres written back as seven metres, silently, exit 0.
			 * That is the unit confusion this format exists to prevent, and it
			 * was reachable from `pretty-print --canonical` on any prefixed
			 * unit. It bites the NON-DECIMAL bases hardest because they have no
			 * choice: §4.6 requires the quoted form for a base-16 float, so
			 * every such value with a prefix was exposed.
			 *
			 * bvn_ser_reduced_number answers 0 for anything that is not a
			 * numeric value with a scale to apply, so a genuine utf8 string --
			 * which cannot carry a unit at all (doc/05 §2.3) -- passes straight
			 * through here.
			 */
			char   *rtext = NULL;
			int32_t rlen  = 0;
			int     rr    = bvn_ser_reduced_number(s, d, &rtext, &rlen);
			if (rr < 0)
				return false;
			if (rr > 0) {
				bool okp = bvn_ser_serialize_string(
					s, (const uint8_t*)rtext, (uint32_t)rlen);
				free(s->ser_value_text);
				s->ser_value_text = rtext;
				s->ser_value_len  = (uint32_t)rlen;
				s->ser_value_unit = s->ser_reduced_unit;
				if (!okp) return false;
			} else if (!bvn_ser_serialize_string(s,
					(const uint8_t*)d->data, d->length)) {
				return false;
			}
			/* ...and the inline unit, for the same reason the number branch
			 * appends it: it is not in the annotation, so nothing else will. */
			if (!bvn_ser_append_inline_unit(s, d)) return false;
		} else if (d->type == token_is_bool) {
			const char* bs = (const char*)d->data;
			uint32_t    bn = d->length;
			bool tv = bs && ((bn == 4 && memcmp(bs, "true", 4) == 0) ||
			                 (bn == 2 && memcmp(bs, "on", 2) == 0));
			if (!bvn_ser_push_str(s, tv ? "true" : "false"))
				return false;
		} else if (d->type == token_is_symbol) {
			if (d->data && d->length) {
				if (!bvn_ser_push(s, d->data, d->length))
					return false;
			}
		} else if (d->type == token_is_reference) {
			if (!bvn_ser_push_byte(s, '&')) return false;
			if (d->data && d->length) {
				if (!bvn_ser_push(s, d->data, d->length))
					return false;
			}
		}
		/*
		 * NB: emitted_unit is intentionally NOT reset here. A single annotation
		 * applies to every element of an array (<float:64,m/s> [1, 2, 3]), and
		 * each element is its own ev_data — the flag must persist across them so
		 * the unit is emitted once (in the annotation) and not re-appended inline
		 * to element 2+. It is correctly re-scoped by ev_assignment_start /
		 * ev_type_annotation_start when the next value context begins.
		 */
		s->need_semi = true;
		bvn_ser_mark_value_done(s);
		break;
	default:
		break;
	}
	return true;
}
static void bvn_writer_init(bvnr_writer_t* w, bvnr_write_flags_t* opts)
{
	/* Carried across the re-arming memset for the same reason the reader carries
	 * it: the policy is set through its own call, describes what this PRODUCER
	 * emits rather than what one document contains, and a caller who set it
	 * before opening would have no way to notice it had been dropped. */
	bvn_unit_policy_state_t saved_policy = w->val.policy;
	bvnr_reader_t* saved_probe = w->usrc_probe;
	memset(w, 0, sizeof(*w));
	w->val.policy = saved_policy;
	w->usrc_probe = saved_probe;
	w->ser.pretty = false;
	w->ser.indent = 0;
	w->ser.need_semi = false;
	w->ser.finished = false;
	w->ser.array_depth = 0;
	w->ser.in_octet_stream = false;
	w->ser.struct_depth = 0;
	w->ser.had_type_annotation = false;
	w->ser.emitted_type_param = false;
	w->ser.wbuf_pos = 0;
	memset(w->ser.arr_need_comma, 0, sizeof(w->ser.arr_need_comma));
	memset(w->ser.struct_depth_at_array_start, 0,
		   sizeof(w->ser.struct_depth_at_array_start));
	w->val.value_type  = BVN_TYPE_PLAIN;
	w->val.parsed_unit = BVN_UNIT_NO_PREFIX(bu_none);
	if (opts) {
		w->ser.event_userdata    = opts->userdata;
		w->ser.on_event          = opts->on_event;
		w->val.on_error          = opts->on_error;
		w->ser.max_array_nesting = opts->max_array_nesting;
		w->ser.max_struct_nesting = opts->max_struct_nesting;
		w->ser.unit_flags        = opts->unit_flags;
	}
	if (!w->ser.max_array_nesting)
		w->ser.max_array_nesting = max_array_nesting;
	if (!w->ser.max_struct_nesting)
		w->ser.max_struct_nesting = max_struct_nesting;
}
bool bvnr_open_write_sink(
	bvnr_writer_t* w, const bvnr_sink_t* sink,
	bool pretty, bvnr_write_flags_t* options)
{
	if (!w || !sink || !bvn_sink_impl_c(sink)->push) return false;
	bvn_writer_init(w, options);
	w->ser.sink   = *sink;
	w->ser.pretty = pretty;
	if (options && options->emit_version)
		return bvnr_write_version(w,
			(uint16_t)BVNR_SPEC_VERSION_MAJOR,
			(uint16_t)BVNR_SPEC_VERSION_MINOR);
	return true;
}
bool bvnr_open_write_mem(
	bvnr_writer_t* w, void* buf, uint64_t cap,
	bool pretty, bvnr_write_flags_t* options)
{
	if (!w || !buf) return false;
	bvnr_sink_t sink;
	bvnr_sink_to_mem(&sink, buf, cap);
	return bvnr_open_write_sink(w, &sink, pretty, options);
}
/*
 * Public event entry point: validate, serialize, then notify the optional
 * on_event observer. Bails immediately if a prior error is latched or the
 * stream is already finished, so a caller can write a whole document without
 * checking after every call. A serialize failure is mapped to the right sink
 * error (buffer exhausted vs. write failed) for accurate diagnostics.
 */
bool bvnr_write_event(
	bvnr_writer_t* w, bvnr_event_t ev, bvnr_data_t* data)
{
	if (!w) return false;
	if (!data) return bvn_writer_set_error(w, error_invalid_argument);
	if (w->val.last_error != error_none)
		return false;
	if (w->ser.finished)
		return bvn_writer_set_error(w, error_invalid_argument);
	/* The unit in force for a value, before validation moves it on. */
	const bool         governed = w->ser.emitted_unit;
	const value_unit_t gov      = w->gov_unit;
	/* ...and the type in force, for the same reason. */
	const bool              typed = w->gov_typed;
	const value_type_spec_t gtype = w->gov_type;
	if (!bvn_writer_validate_event(w, ev, data))
		return false;
	w->ser.ser_error = error_none;
	free(w->ser.ser_value_text);
	w->ser.ser_value_text = NULL;
	w->ser.ser_value_len  = 0;
	bvnr_data_t local;
	if (ev == ev_type_annotation_end && BVN_UNIT_IS_REAL(w->ann_unit) &&
	    !w->ser.emitted_unit && bvn_w_family_takes_unit(w->ann_family)) {
		/* The unit was named on the start event and no parameter event wrote
		 * it (none came, or it carried neither text nor unit). The contract
		 * allows that, and without this the annotation went out bare and the
		 * unit with it. */
		local = *data;
		local.type               = token_is_unit;
		local.value_unit         = w->ann_unit;
		local.data               = NULL;
		local.length             = 0;
		local.unit_source        = NULL;
		local.unit_source_length = 0;
		if (!bvn_ser_serialize_event(&w->ser,
				ev_type_annotation_type_family_parameter, &local))
			return bvn_writer_set_error(w,
				w->ser.ser_error != error_none ? w->ser.ser_error
				: bvn_sink_impl(&w->ser.sink)->is_mem
					? error_sink_buffer_exhausted
					: error_writing_to_sink);
	}
	if (ev == ev_data && governed && BVN_UNIT_IS_REAL(gov) &&
	    BVN_UNIT_IS_UNITLESS(data->value_unit) &&
	    (data->type == token_is_number || data->type == token_is_array_number ||
	     data->type == token_is_string || data->type == token_is_array_string)) {
		/* "That unit or none": a value that leaves it to the annotation still
		 * HAS the annotation's unit, and BVN_UNIT_REDUCE has to rescale it. */
		local = *data;
		local.value_unit = gov;
		data = &local;
	}
	if (ev == ev_data &&
	    (data->type == token_is_number || data->type == token_is_array_number ||
	     data->type == token_is_string || data->type == token_is_array_string)) {
		/* The serialiser decides whether a value is a NUMBER -- and so whether
		 * BVN_UNIT_REDUCE has digits to rescale -- from the event's value_type.
		 * A caller who left that plain, which is the natural way to write a
		 * bare `5 k~m`, got the unit reduced and the digits left alone: `5 m`,
		 * a factor of a thousand, reported as success. Hand over the type the
		 * validator just held the value to. */
		value_type_spec_t et = bvn_w_effective_type(data, typed, gtype);
		if (!bvn_type_spec_eq(et, data->value_type)) {
			if (data != &local)
				local = *data;
			local.value_type = et;
			data = &local;
		}
	}
	if (!bvn_ser_serialize_event(&w->ser, ev, data)) {
		if (w->ser.ser_error != error_none)
			return bvn_writer_set_error(w, w->ser.ser_error);
		return bvn_writer_set_error(w, bvn_sink_impl(&w->ser.sink)->is_mem
			? error_sink_buffer_exhausted
			: error_writing_to_sink);
	}
	if (w->ser.on_event) {
		/* Show the observer what reached the sink. A BVN_UNIT_REDUCE rescale
		 * replaces the digits, and passing the caller's struct through unchanged
		 * left anyone mirroring or checksumming the stream describing a value
		 * that was never written. The caller's struct is not modified. */
		if (w->ser.ser_value_text) {
			bvnr_data_t shown = *data;
			shown.data       = w->ser.ser_value_text;
			shown.length     = w->ser.ser_value_len;
			shown.value_unit = w->ser.ser_value_unit;
			if (!w->ser.on_event(w->ser.event_userdata, ev, &shown))
				return bvn_writer_set_error(w, error_scanner_callback_failed);
		} else if (!w->ser.on_event(w->ser.event_userdata, ev, data)) {
			return bvn_writer_set_error(w, error_scanner_callback_failed);
		}
	}
	return true;
}
/*
 * Emit a leading "#!bovnar <major>.<minor>" version directive. Legal only before
 * any output has begun (right after open); afterwards it is error_invalid_argument.
 * The directive is a single comment line, so it never affects separator
 * bookkeeping — it is written straight through ahead of the value stream.
 */
/*
 * Format and push a leading "#!bovnar <major>.<minor>" directive straight to
 * the serializer's sink, bypassing the event/separator bookkeeping (the
 * directive is a lexical comment). Shared by bvnr_write_version (the writer
 * entry point) and the canonicalising observer. Returns false if a stream or
 * version was already begun, or the sink push fails; the caller maps that to
 * its own error space.
 */
bool bvn_ser_emit_version(bvnr_serializer_t* s, uint16_t major, uint16_t minor)
{
	if (s->finished || s->stream_begun || s->version_emitted)
		return false;
	char line[48];
	int n = snprintf(line, sizeof(line), "#!bovnar %u.%u\n",
		(unsigned)major, (unsigned)minor);
	if (n <= 0 || (size_t)n >= sizeof(line))
		return false;
	if (!bvn_ser_push(s, line, (uint32_t)n))
		return false;
	s->version_emitted = true;
	s->version_major   = major;
	s->version_minor   = minor;
	return true;
}
bool bvnr_write_version(bvnr_writer_t* w, uint16_t major, uint16_t minor)
{
	if (!w) return false;
	if (w->val.last_error != error_none)
		return false;
	if (w->ser.finished || w->ser.stream_begun || w->ser.version_emitted)
		return bvn_writer_set_error(w, error_invalid_argument);
	if (!bvn_ser_emit_version(&w->ser, major, minor))
		return bvn_writer_set_error(w, bvn_sink_impl(&w->ser.sink)->is_mem
			? error_sink_buffer_exhausted
			: error_writing_to_sink);
	return true;
}
/*
 * Close out the document: refuse if any struct/array is still open (that would
 * be a truncated stream), emit the trailing ';' if one is pending, flush the
 * write buffer and the sink, and latch finished so no further events are
 * accepted. Must be called for output to be complete — the final separator and
 * any buffered bytes are only guaranteed on disk after this returns true.
 */
bool bvnr_write_finish(bvnr_writer_t* w)
{
	if (!w) return false;
	if (w->val.last_error != error_none)
		return false;
	/* A spelling handed over and never used: the value it was meant for took
	 * no unit, and it was the last one. */
	if (w->val.unit_src_len) {
		w->val.unit_src_len = 0;
		return bvn_writer_set_error(w, error_unit_mismatch);
	}
	/* An assignment still waiting for its value, or an unclosed annotation, is
	 * just as incomplete as an unclosed struct — it leaves ".k=" or ".k=<" in the
	 * document with no terminating ';'. A caller who wants a null value sends
	 * ev_data with token_is_null_value; leaving the assignment dangling is not
	 * the same thing. */
	if (w->ser.struct_depth > 0 || w->ser.array_depth > 0 ||
	    w->ser.w_awaiting_value || w->ser.w_in_annotation ||
	    w->ser.in_octet_stream || w->ser.w_after_dim)
		return bvn_writer_set_error(w, error_got_incomplete_bvnr_stream);
	w->ser.ser_error = error_none;
	if (!bvn_ser_finish_stream(&w->ser)) {
		if (w->ser.ser_error != error_none)
			return bvn_writer_set_error(w, w->ser.ser_error);
		return bvn_writer_set_error(w, bvn_sink_impl(&w->ser.sink)->is_mem
			? error_sink_buffer_exhausted
			: error_writing_to_sink);
	}
	{
		bvn_sink_impl_t* si = bvn_sink_impl(&w->ser.sink);
		if (si->flush) {
			if (!si->flush(&w->ser.sink))
				return bvn_writer_set_error(w, si->is_mem
					? error_sink_buffer_exhausted
					: error_writing_to_sink);
		}
	}
	w->ser.finished = true;
	return true;
}
error_code_t bvnr_writer_get_error(const bvnr_writer_t* w)
{
	if (!w) return error_invalid_argument;
	return w->val.last_error;
}
uint64_t bvnr_writer_get_error_offset(const bvnr_writer_t* w)
{
	if (!w) return 0;
	return w->val.error_offset;
}
uint64_t bvnr_writer_bytes_written(const bvnr_writer_t* w)
{
	if (!w) return 0;
	return bvnr_sink_bytes_written(&w->ser.sink) + w->ser.wbuf_pos;
}
bvn_unit_flags_t bvnr_writer_unit_flags(const bvnr_writer_t* w)
{
	if (!w) return BVN_UNIT_FLAGS_NONE;
	return w->ser.unit_flags;
}
