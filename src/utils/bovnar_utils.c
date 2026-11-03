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

#ifndef _POSIX_C_SOURCE
#  define _POSIX_C_SOURCE 200809L
#endif
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <ctype.h>
#include <math.h>
#include "bovnar.h"
#include "bvn_internal_dims.h"
#include "bvn_float.h"
#include "bvn_datetime.h"
#include "bovnar_si_units.h"
#include "bovnar_currency.h"
#include "bvn_unit_impl.h"
#include "bvn_profile_impl.h"
/*
 * ===========================================================================
 * Shared utilities: number parsing/formatting/validation and unit handling
 * ===========================================================================
 *
 * This is the cross-cutting toolbox used by the reader, writer and DOM. It has
 * three loosely related groups:
 *
 *  1. Digit/number primitives: convert characters to/from digit values in any
 *     base from 2 up to 62, plus the special base-64 and base-85 alphabets;
 *     validate numeric literals; and check that an integer literal fits a given
 *     bit width. The width check is the interesting part — bovnar supports
 *     integers up to BVN_MAX_INT_WIDTH bits, far past uint64, so for wide types
 *     the bound 2^w-1 cannot be held in a register. It is instead materialised
 *     as a decimal *string* (bvn_pow2m1_dec) and compared digit-string against
 *     the (base-converted) literal (bvn_cmp_dec). Cheap length-based prefilters
 *     avoid that expensive path for the common in-range/out-of-range cases.
 *
 *  2. Unit parsing/formatting: turn a textual unit like "kg.m/s^2" into a
 *     structured value_unit_t and back, including SI/IEC prefixes, exponents,
 *     and currency codes.
 *
 *  3. Identifier/symbol/reference validation and scalar parse/format helpers
 *     that wrap the bignum routines for the <=64-bit fast path.
 */

/*
 * Map an ASCII character to its digit value in `base`, or return `base` itself
 * to signal "not a valid digit". Bases 64 and 85 use their own fixed alphabets
 * (standard base64 and Ascii85). For bases up to 62 the ordering is
 * 0-9, a-z, A-Z; note that for base<=36 upper- and lower-case are the SAME
 * digit (case-insensitive hex etc.), while for base>36 they are distinct
 * digits — hence the branch on `base > 36u`.
 */
#define BVN_DEC_SCRATCH_SIZE 10000u
uint32_t bvn_char_to_digit(uint32_t c, uint32_t base)
{
	if (base == 64u) {
		if (c >= 'A' && c <= 'Z') return c - 'A';
		if (c >= 'a' && c <= 'z') return c - 'a' + 26u;
		if (c >= '0' && c <= '9') return c - '0' + 52u;
		if (c == '+')             return 62u;
		if (c == '/')             return 63u;
		return base;
	}
	if (base == 85u) {
		if (c >= '!' && c <= 'u') return c - '!';
		return base;
	}
	uint32_t d;
	if      (c >= '0' && c <= '9') d = c - '0';
	else if (c >= 'a' && c <= 'z') d = 10u + (c - 'a');
	else if (c >= 'A' && c <= 'Z') d = (base > 36u) ? 36u + (c - 'A')
													 : 10u + (c - 'A');
	else return base;
	return d < base ? d : base;
}
static uint32_t bvn_digit_to_char(uint32_t d, uint32_t base)
{
	if (base == 64u) {
		static const char b64[] =
			"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
			"0123456789+/";
		return (d < 64u) ? (uint32_t)(uint8_t)b64[d] : 0u;
	}
	if (base == 85u)
		return (d < 85u) ? ('!' + d) : 0u;
	if (d <  10u) return '0' + d;
	if (d <  36u) return 'a' + (d - 10u);
	if (d <  62u) return 'A' + (d - 36u);
	return 0u;
}
bool bvn_is_special_number_string(const char* s)
{
	if (!s) return false;
	uint8_t c = (uint8_t)s[0];
	if (c != 'n' && c != 'i') return false;
	if (c == 'i')                                   /* inf  */
		return s[1] == 'n' && s[2] == 'f' && s[3] == '\0';
	if (s[1] == 'a')                                /* nan  */
		return s[2] == 'n' && s[3] == '\0';
	return s[1] == 'i' && s[2] == 'n' && s[3] == 'f' && s[4] == '\0';  /* ninf */
}
bool bvn_validate_digits_for_base(const char* s, uint32_t base)
{
	if (!s || !*s) return false;
	uint32_t i = 0;
	/* In bases 64 and 85 '+'/'-' are digit characters, not signs, so the bases
	 * are unsigned-only and no leading sign is consumed. Other bases accept a
	 * leading '-' and only '-': the reader's string-carrier digit loop derives
	 * its sign from '-' alone and rejects every other non-digit byte, so
	 * accepting '+' here let the writer emit an integer literal that came back
	 * as error_digit_not_in_base. (Float carriers DO take a leading '+' -- that
	 * is bvn_validate_number_in_base, which is right to keep it.) */
	if (base != 64u && base != 85u && s[0] == '-') i = 1;
	if (!s[i]) return false;
	for (; s[i]; i++) {
		if (bvn_char_to_digit((uint8_t)s[i], base) >= base)
			return false;
	}
	return true;
}
/*
 * Validate a full numeric literal (optional sign, mantissa with optional dot,
 * optional exponent) in the given base. The exponent marker is base-dependent:
 * decimal-style 'e'/'E' only when the base is small enough (<=14) that 'e'
 * isn't itself a digit, and hex-float-style 'p'/'P' for power-of-two bases up
 * to 16. The exponent itself is always decimal. Requires at least one mantissa
 * digit and, if an exponent marker appears, at least one exponent digit.
 */
bool bvn_validate_number_in_base(const char* s, uint32_t base)
{
	if (!s || !*s) return false;
	/* Bases 64 and 85 are unsigned integer bases with fixed alphabets (standard
	 * Base64, Ascii85) in which '+', '-', '.', and other punctuation are ordinary
	 * digit characters — not sign, decimal-point, or exponent markers. They never
	 * carry float syntax, so validate them as a pure unsigned digit string. This
	 * also keeps this function in lock-step with bvn_validate_digits_for_base,
	 * which the writer uses for the same values. */
	if (base == 64u || base == 85u)
		return bvn_validate_digits_for_base(s, base);
	uint32_t i = 0;
	if (s[0] == '-' || s[0] == '+') i = 1;
	if (!s[i]) return false;
	bool has_dot = false, has_exp = false;
	bool has_mant_digit = false, has_exp_digit = false;
	for (; s[i]; i++) {
		char c = s[i];
		if (c == '.') {
			if (has_dot || has_exp) return false;
			has_dot = true;
			continue;
		}
		if (((c == 'e' || c == 'E') && base <= 14u) ||
		    ((c == 'p' || c == 'P') && (base & (base - 1u)) == 0u && base <= 16u)) {
			if (has_exp || !has_mant_digit) return false;
			has_exp = true;
			if (s[i + 1] == '+' || s[i + 1] == '-') i++;
			continue;
		}
		if (c == '+' || c == '-') return false;
		if (has_exp) {
			if (bvn_char_to_digit((uint8_t)c, 10u) >= 10u)
				return false;
			has_exp_digit = true;
		} else {
			if (bvn_char_to_digit((uint8_t)c, base) >= base)
				return false;
			has_mant_digit = true;
		}
	}
	return has_mant_digit && (!has_exp || has_exp_digit);
}
/*
 * Compute 2^n - 1 as a decimal string — i.e. the largest unsigned value
 * representable in n bits. Used as the upper bound when range-checking integers
 * wider than 64 bits, where the bound itself can't fit in any machine integer.
 * Implemented as schoolbook doubling: maintain the number as an array of
 * little-endian decimal digits, double it n times, then subtract one. O(n^2)
 * in digit count, which is fine because n is a type width (thousands of bits at
 * most) and the result is cached/compared as a string.
 */
static int bvn_pow2m1_dec(uint32_t n, char* buf, size_t cap)
{
	if (!buf || cap < 2u) return -1;
	if (n == 0) {
		buf[0] = '0'; buf[1] = '\0';
		return 1;
	}
	uint8_t *dig = malloc(BVN_DEC_SCRATCH_SIZE);
	if (!dig) return -1;
	uint32_t nd = 1;
	dig[0] = 1;
	for (uint32_t i = 0; i < n; i++) {
		uint16_t carry = 0;
		for (uint32_t j = 0; j < nd; j++) {
			uint16_t v = (uint16_t)((uint16_t)dig[j] * 2u + carry);
			dig[j] = (uint8_t)(v % 10);
			carry  = v / 10;
		}
		while (carry) {
			if (nd >= BVN_DEC_SCRATCH_SIZE) { free(dig); return -1; }
			dig[nd++] = (uint8_t)(carry % 10);
			carry /= 10;
		}
	}
	int borrow = 1;
	for (uint32_t j = 0; j < nd && borrow; j++) {
		int v = (int)dig[j] - borrow;
		if (v < 0) { dig[j] = (uint8_t)(v + 10); borrow = 1; }
		else       { dig[j] = (uint8_t)v;         borrow = 0; }
	}
	while (nd > 1 && dig[nd - 1] == 0) nd--;
	uint32_t len = nd;
	if (len + 1u > cap) { free(dig); return -1; }   /* too long to fit: caller rejects */
	for (uint32_t j = 0; j < len; j++)
		buf[j] = (char)('0' + dig[nd - 1 - j]);
	buf[len] = '\0';
	free(dig);
	return (int)len;
}
/*
 * Compare two non-negative decimal strings numerically. Leading zeros are
 * skipped first so the comparison reduces to "longer string wins, else
 * lexicographic" — valid only because both are pure decimal with no sign.
 */
static int bvn_cmp_dec(const char* a, const char* b)
{
	while (*a == '0' && a[1]) a++;
	while (*b == '0' && b[1]) b++;
	size_t la = strlen(a), lb = strlen(b);
	if (la < lb) return -1;
	if (la > lb) return  1;
	return strcmp(a, b);
}
/*
 * Convert an arbitrary-base digit string to a decimal string (Horner's method:
 * dec = dec*base + digit, with dec kept as little-endian decimal digits). Lets
 * the wide-integer range check normalise any base to decimal once and then do
 * all comparisons in base 10 against bvn_pow2m1_dec's output.
 */
static bool bvn_digits_to_dec(const char* src, uint32_t base,
	char* buf, size_t cap)
{
	uint8_t *dec = malloc(BVN_DEC_SCRATCH_SIZE);
	if (!dec) return false;
	uint32_t nd = 1;
	dec[0] = 0;
	for (; *src; src++) {
		uint32_t d = bvn_char_to_digit((uint8_t)*src, base);
		if (d >= base) { free(dec); return false; }
		uint32_t carry = d;
		for (uint32_t i = 0; i < nd; i++) {
			uint32_t v = (uint32_t)dec[i] * base + carry;
			dec[i] = (uint8_t)(v % 10u);
			carry  = v / 10u;
		}
		while (carry) {
			if (nd >= BVN_DEC_SCRATCH_SIZE) { free(dec); return false; }
			dec[nd++] = (uint8_t)(carry % 10u);
			carry /= 10u;
		}
	}
	if (nd + 1u > cap) { free(dec); return false; }
	for (uint32_t i = 0; i < nd; i++)
		buf[i] = (char)('0' + dec[nd - 1u - i]);
	buf[nd] = '\0';
	free(dec);
	return true;
}
/*
 * Approximate decimal digit count of a w-bit value: w * log10(2), rounded up.
 * Used only to size the cheap length-based prefilter, so a small over-estimate
 * is harmless.
 */
static uint32_t bvn_dec_digits_2pow(uint32_t w)
{
	if (w == 0u) return 1u;
	return (uint32_t)((double)w * 0.30102999566398119521) + 1u;
}
/*
 * Fast accept/reject by digit count before the costly exact comparison:
 * clearly-shorter numbers are in range (1), clearly-longer are out (0), and the
 * ambiguous boundary length returns -1 to mean "must compare exactly".
 */
static int bvn_dec_digit_prefilter(const char* d, uint32_t max_digits)
{
	while (d[0] == '0' && d[1]) d++;
	size_t n = strlen(d);
	if (n + 1u < (size_t)max_digits)     return 1;
	if (n > (size_t)max_digits + 1u)     return 0;
	return -1;
}
/*
 * Does the unsigned literal `s` (in `base`) fit in `w` bits?
 *
 * Two regimes: for w<=64 it parses to a uint64 (strtoull for base 10, manual
 * Horner with overflow guard otherwise) and compares against 2^w-1. For wider
 * types it normalises to decimal, tries the length prefilter, and only on an
 * ambiguous length materialises the exact bound 2^w-1 for a string compare.
 * w==0 means "width unspecified" and always passes; nan/inf pass through.
 * bvn_validate_sint_range mirrors this with the asymmetric signed bounds and a
 * 2^(w-1) magnitude limit.
 */
bool bvn_validate_uint_range(const char* s, uint32_t w, uint32_t base)
{
	if (!s) return true;
	if (w == 0) return true;
	/* In bases 64 and 85 '+'/'-' are digits, not signs; do not strip a leading
	 * '+' nor reject a leading '-' (it is digit 12 in Ascii85, an invalid digit
	 * in Base64 — either way the digit loop below decides). */
	const bool uses_sign = (base != 64u && base != 85u);
	uint8_t c0 = (uint8_t)s[0];
	if ((c0 < '0' || c0 > '9') && c0 != '+') {
		if (bvn_is_special_number_string(s)) return true;
		if (uses_sign && c0 == '-') return false;
	}
	const char* p = s;
	if (uses_sign && *p == '+') p++;
	if (w <= 64) {
		uint64_t maxv = (w >= 64) ? UINT64_MAX : (1ULL << w) - 1ULL;
		if (base == 10) {
			char* end;
			errno = 0;
			uint64_t v = strtoull(p, &end, 10);
			if (*end || errno == ERANGE) return false;
			return v <= maxv;
		}
		uint64_t v = 0;
		for (; *p; p++) {
			uint32_t d = bvn_char_to_digit((uint8_t)*p, base);
			if (d >= base) return false;
			if (v > (UINT64_MAX - d) / base) return false;
			v = v * base + d;
		}
		return v <= maxv;
	}
	uint32_t max_digits = bvn_dec_digits_2pow(w);
	if (base == 10) {
		int pf = bvn_dec_digit_prefilter(p, max_digits);
		if (pf >= 0) return pf != 0;
		char *maxs = malloc(BVN_DEC_SCRATCH_SIZE);
		if (!maxs) return false;
		int r = bvn_pow2m1_dec(w, maxs, BVN_DEC_SCRATCH_SIZE);
		bool ok = (r >= 0) && (bvn_cmp_dec(p, maxs) <= 0);
		free(maxs);
		return ok;
	}
	{
		char *dec = malloc(BVN_DEC_SCRATCH_SIZE);
		if (!dec) return false;
		if (!bvn_digits_to_dec(p, base, dec, BVN_DEC_SCRATCH_SIZE)) {
			free(dec);
			return false;
		}
		int pf = bvn_dec_digit_prefilter(dec, max_digits);
		if (pf >= 0) { free(dec); return pf != 0; }
		char *maxs = malloc(BVN_DEC_SCRATCH_SIZE);
		if (!maxs) { free(dec); return false; }
		int r = bvn_pow2m1_dec(w, maxs, BVN_DEC_SCRATCH_SIZE);
		bool ok = (r >= 0) && (bvn_cmp_dec(dec, maxs) <= 0);
		free(dec);
		free(maxs);
		return ok;
	}
}
static bool bvn_dec_increment(char *s)
{
	int len = (int)strlen(s);
	int carry = 1;
	for (int i = len - 1; i >= 0 && carry; i--) {
		int d = (s[i] - '0') + carry;
		s[i] = (char)('0' + (d % 10));
		carry = d / 10;
	}
	if (carry) {
		memmove(s + 1, s, (size_t)len + 1);
		s[0] = '1';
	}
	return true;
}
bool bvn_validate_sint_range(const char* s, uint32_t w, uint32_t base)
{
	if (!s) return true;
	if (w == 0) return true;
	uint8_t c0 = (uint8_t)s[0];
	if ((c0 < '0' || c0 > '9') && c0 != '-' && c0 != '+') {
		if (bvn_is_special_number_string(s)) return true;
	}
	bool neg = (s[0] == '-');
	const char* abs = s;
	if (neg || s[0] == '+') abs++;
	if (w <= 64) {
		if (base == 10) {
			char* end;
			errno = 0;
			int64_t v = strtoll(s, &end, 10);
			if (*end || errno == ERANGE) return false;
			int64_t lo = (w >= 64)
				? INT64_MIN
				: -(int64_t)(1ULL << (w - 1));
			int64_t hi = (w >= 64)
				? INT64_MAX
				: (int64_t)((1ULL << (w - 1)) - 1ULL);
			return v >= lo && v <= hi;
		}
		uint64_t v = 0;
		for (const char* p = abs; *p; p++) {
			uint32_t d = bvn_char_to_digit((uint8_t)*p, base);
			if (d >= base) return false;
			if (v > (UINT64_MAX - d) / base) return false;
			v = v * base + d;
		}
		uint64_t mag = neg
			? ((w >= 64) ? (1ULL << 63) : (1ULL << (w - 1)))
			: ((w >= 64) ? (uint64_t)INT64_MAX : (1ULL << (w - 1)) - 1ULL);
		return v <= mag;
	}
	uint32_t max_digits = bvn_dec_digits_2pow(w - 1);
	if (base == 10) {
		int pf = bvn_dec_digit_prefilter(abs, max_digits);
		if (pf >= 0) return pf != 0;
		char *maxs = malloc(BVN_DEC_SCRATCH_SIZE);
		if (!maxs) return false;
		int r = bvn_pow2m1_dec(w - 1, maxs, BVN_DEC_SCRATCH_SIZE);
		if (r < 0) { free(maxs); return false; }
		if (!neg) {
			bool ok = bvn_cmp_dec(abs, maxs) <= 0;
			free(maxs);
			return ok;
		}
		char *tmp = malloc(BVN_DEC_SCRATCH_SIZE + 1u);
		if (!tmp) { free(maxs); return false; }
		strncpy(tmp, maxs, BVN_DEC_SCRATCH_SIZE - 1u);
		tmp[BVN_DEC_SCRATCH_SIZE - 1u] = '\0';
		bvn_dec_increment(tmp);
		bool ok = bvn_cmp_dec(abs, tmp) <= 0;
		free(maxs);
		free(tmp);
		return ok;
	}
	{
		char *dec  = malloc(BVN_DEC_SCRATCH_SIZE);
		char *maxs = malloc(BVN_DEC_SCRATCH_SIZE);
		if (!dec || !maxs) { free(dec); free(maxs); return false; }
		bool ok = false;
		if (!bvn_digits_to_dec(abs, base, dec, BVN_DEC_SCRATCH_SIZE))
			goto sint_bigint_done;
		{
			int pf = bvn_dec_digit_prefilter(dec, max_digits);
			if (pf >= 0) { ok = (pf != 0); goto sint_bigint_done; }
		}
		int r = bvn_pow2m1_dec(w - 1, maxs, BVN_DEC_SCRATCH_SIZE);
		if (r < 0)
			goto sint_bigint_done;
		if (!neg) {
			ok = bvn_cmp_dec(dec, maxs) <= 0;
			goto sint_bigint_done;
		}
		{
			char *tmp = malloc(BVN_DEC_SCRATCH_SIZE + 1u);
			if (!tmp) goto sint_bigint_done;
			strncpy(tmp, maxs, BVN_DEC_SCRATCH_SIZE - 1u);
			tmp[BVN_DEC_SCRATCH_SIZE - 1u] = '\0';
			bvn_dec_increment(tmp);
			ok = bvn_cmp_dec(dec, tmp) <= 0;
			free(tmp);
		}
sint_bigint_done:
		free(dec);
		free(maxs);
		return ok;
	}
}
/*
 * Map an error code to a short stable identifier string for diagnostics/logs.
 * Returns a static string (never NULL), so it is safe to print unconditionally.
 */
const char* bvn_error_to_string(error_code_t code)
{
	switch (code) {
	case error_none:                      return "none";
	case error_unknown_token_type:        return "unknown_token_type";
	case error_array_row_size_mismatch:   return "array_row_size_mismatch";
	case error_identifier_too_long:       return "identifier_too_long";
	case error_empty_identifier:          return "empty_identifier";
	case error_struct_nesting_too_high:   return "struct_nesting_too_high";
	case error_array_nesting_too_high:    return "array_nesting_too_high";
	case error_illegal_struct_close:      return "illegal_struct_close";
	case error_string_too_long:           return "string_too_long";
	case error_illegal_escape_sequence:   return "illegal_escape_sequence";
	case error_number_too_long:           return "number_too_long";
	case error_symbol_too_long:           return "symbol_too_long";
	case error_reference_too_long:        return "reference_too_long";
	case error_read_complete_chunk_failed:return "read_complete_chunk_failed";
	case error_octet_stream_out_of_sync:  return "octet_stream_out_of_sync";
	case error_unexpected_input_byte:     return "unexpected_input_byte";
	case error_text_data_too_long:        return "text_data_too_long";
	case error_reading_from_source_fd:    return "reading_from_source_fd";
	case error_got_incomplete_bvnr_stream:return "incomplete_bvnr_stream";
	case error_invalid_utf8_byte:         return "invalid_utf8_byte";
	case error_invalid_byte_order_mark:   return "invalid_byte_order_mark";
	case error_type_too_long:             return "type_too_long";
	case error_unit_too_long:             return "unit_too_long";
	case error_expected_string_in_array:  return "expected_string_in_array";
	case error_expected_number_in_array:  return "expected_number_in_array";
	case error_illegal_value_type:        return "illegal_value_type";
	case error_scanner_callback_failed:   return "scanner_callback_failed";
	case error_file_too_long:             return "file_too_long";
	case error_invalid_argument:          return "invalid_argument";
	case error_too_many_array_items:      return "too_many_array_items";
	case error_writing_to_sink:           return "writing_to_sink";
	case error_sink_buffer_exhausted:     return "sink_buffer_exhausted";
	case error_unit_illegal:              return "unit_illegal";
	case error_unit_profile_unknown:      return "unit_profile_unknown";
	case error_unit_profile_unsupported:  return "unit_profile_unsupported";
	case error_octet_stream_forbidden:    return "octet_stream_forbidden";
	case error_type_param_whitespace:     return "type_param_whitespace";
	case error_base_requires_string_literal: return "base_requires_string_literal";
	case error_type_value_mismatch:       return "type_value_mismatch";
	case error_value_out_of_range:        return "value_out_of_range";
	case error_digit_not_in_base:         return "digit_not_in_base";
	case error_recovered:                 return "recovered";
	case error_unit_mismatch:             return "unit_mismatch";
	case error_array_element_type_mismatch: return "array_element_type_mismatch";
	case error_struct_shape_mismatch:     return "struct_shape_mismatch";
	case error_duplicate_struct_key:      return "duplicate_struct_key";
	case error_invalid_spec_version:      return "invalid_spec_version";
	case error_unsupported_spec_version:  return "unsupported_spec_version";
	case error_invalid_codepoint:         return "invalid_codepoint";
	case error_invalid_datetime_literal:  return "invalid_datetime_literal";
	case error_datetime_literal_unsupported_epoch:
	                                      return "datetime_literal_unsupported_epoch";
	case error_unit_inexact:              return "unit_inexact";
	case error_octet_stream_truncated:    return "octet_stream_truncated";
	default:                              return "unknown_error";
	}
}
uint32_t bvnr_version(void)
{
	return (uint32_t)BVNR_VERSION;
}
const char* bvnr_version_string(void)
{
	return BVNR_VERSION_STRING;
}
void bvnr_spec_version(uint16_t* major, uint16_t* minor)
{
	if (major) *major = (uint16_t)BVNR_SPEC_VERSION_MAJOR;
	if (minor) *minor = (uint16_t)BVNR_SPEC_VERSION_MINOR;
}
typedef struct { const char* a; uint32_t len; value_base_unit_t u; } bu_entry_t;
typedef struct { const char* a; uint32_t len; si_prefix_id_t   p; } si_entry_t;
typedef struct { const char* a; uint32_t len; iec_prefix_id_t  p; } iec_entry_t;
#define BU_LEN_INDEX_SIZE 32u
/*
 * Length index over bu_table: bu_first_for_len[L] is the index of the first
 * table entry whose symbol is at most L bytes long, so the suffix matcher can
 * skip straight past the run of entries longer than the input. These values are
 * a pure function of bu_table and are PRECOMPUTED (rather than built lazily on
 * first use) so the lookup touches no mutable global state and is therefore
 * reentrant / thread-safe — concurrent bvn_parse_unit calls are safe without
 * the caller serialising them. bvn_bu_index_selfcheck() recomputes the index
 * from the table and is asserted equal to these literals by the unit tests, so
 * the constants cannot silently drift when the table changes.
 *
 * If the table grows past BU_LEN_INDEX_SIZE-1 (=31) byte symbols, widen the
 * index; the self-check test will flag it.
 */
/* Length index over bu_table (bu_first_for_len / bu_max_len) — generated from
 * src/gendata/units.bvnr by gen_units.py; recomputed by the self-check. */
#include "bovnar_bu_index.gen.inc"
/*
 * Symbol -> base-unit lookup table. CRUCIAL INVARIANT: entries are ordered so
 * that, for any input, a longer symbol that is a suffix of that input is always
 * tested before a shorter one that is also a suffix (in practice: grouped by
 * DESCENDING symbol length). The parser takes the first suffix match and stops,
 * so this ordering is what makes it prefer the longest match — e.g. "mol" must
 * resolve to mole, not to prefix "mo" + "l" (liter). Reordering that lets a
 * shorter suffix precede a longer one is a CORRECTNESS bug, not just a slowdown.
 * When adding a unit, insert it in the correct length group and update
 * bu_first_for_len (the self-check test enforces both). The si_table/iec_table
 * below map the prefix symbols (k, M, Ki, ...) the same way.
 */
static const bu_entry_t bu_table[] = {
	/* Symbol -> base-unit parse table, DESCENDING byte length — generated from
	 * src/gendata/units.bvnr by gen_units.py. NULL terminator follows. */
#include "bovnar_bu_table.gen.inc"
	{NULL, 0, bu_none}
};
static const si_entry_t si_table[] = {
#include "bovnar_si_table.gen.inc"
	{NULL, 0, si_none}
};
static const iec_entry_t iec_table[] = {
#include "bovnar_iec_table.gen.inc"
	{NULL, 0, iec_none}
};
/*
 * Compact spellings that must NOT resolve as prefix+unit even though the tables
 * would let them ("usb" is the bus, not the microstilb). Generated from the
 * .compact_exceptions list in src/gendata/units.bvnr, where each entry carries
 * its reason; the separated form (p~H) is unaffected. Consulted only on the
 * compact path, which is otherwise the parse-error path, so this cannot reject
 * anything that parsed before.
 */
typedef struct { const char* a; uint32_t len; } compact_except_t;
static const compact_except_t compact_except_table[] = {
#include "bovnar_compact_except.gen.inc"
	{NULL, 0}
};
static bool bvn_compact_form_refused(const char* s, uint32_t len)
{
	for (const compact_except_t* e = compact_except_table; e->a; e++) {
		if (e->len == len && memcmp(s, e->a, len) == 0)
			return true;
	}
	return false;
}
/*
 * Recompute the length index from bu_table and verify it equals the precomputed
 * bu_first_for_len / bu_max_len literals above. This is the guard that keeps the
 * baked-in constants honest: it is not used on the parse path (which reads the
 * literals directly, with no init), only by the unit tests. Returns true iff the
 * literals are consistent with the current table; a false return after editing
 * bu_table means the literals must be regenerated.
 */
bool bvn_bu_index_selfcheck(void)
{
	uint32_t count = 0;
	uint32_t maxlen = 0;
	for (const bu_entry_t* e = bu_table; e->a; e++) {
		count++;
		if (e->len > maxlen) maxlen = e->len;
	}
	if (maxlen >= BU_LEN_INDEX_SIZE) maxlen = BU_LEN_INDEX_SIZE - 1u;
	if (bu_max_len != maxlen)
		return false;
	uint32_t idx = 0;
	for (int32_t L = (int32_t)BU_LEN_INDEX_SIZE - 1; L >= 0; L--) {
		while (idx < count && bu_table[idx].len > (uint32_t)L) idx++;
		if (bu_first_for_len[L] != (uint16_t)idx)
			return false;
	}
	return true;
}
/*
 * Strip and decode an exponent suffix from the end of a unit component,
 * returning the number of bytes consumed (0 if none). bovnar accepts three
 * spellings, all handled here: ASCII caret form ("^2", "^-3"), Unicode
 * superscript digits (e.g. "²", "³", and the U+2070 block for 4-9), and a
 * Unicode superscript minus for negative exponents. Defaulting to exp_linear
 * when absent means "m" and "m^1" parse identically.
 */
/*
 * One superscript digit ENDING at s[end-1]. Returns 0-9 and sets *blen to its
 * byte length, or -1 when the bytes there are not a superscript digit.
 *
 * The widths are not uniform and that is Unicode's doing, not ours: ¹ ² ³ live
 * in the Latin-1 supplement at two bytes (C2 B9/B2/B3) while ⁰ and ⁴-⁹ live in
 * the superscripts block at three (E2 81 B0/B4-B9). A multi-digit exponent
 * therefore mixes widths -- "¹⁰⁰" is 2+3+3 -- so the scan has to consume one
 * digit at a time rather than assume a stride.
 */
static int bvni_sup_digit_back(const char* s, uint32_t end, uint32_t* blen)
{
	if (end >= 3 && (uint8_t)s[end - 3] == 0xE2 &&
	                (uint8_t)s[end - 2] == 0x81) {
		uint8_t b = (uint8_t)s[end - 1];
		if (b == 0xB0)              { *blen = 3; return 0; }
		if (b >= 0xB4 && b <= 0xB9) { *blen = 3; return 4 + (b - 0xB4); }
	}
	if (end >= 2 && (uint8_t)s[end - 2] == 0xC2) {
		switch ((uint8_t)s[end - 1]) {
		case 0xB9: *blen = 2; return 1;
		case 0xB2: *blen = 2; return 2;
		case 0xB3: *blen = 2; return 3;
		default:   break;
		}
	}
	return -1;
}
/*
 * Both forms scan BACKWARDS from the end of the token, because this is a suffix
 * parser: the caller has the whole component and needs to know how many bytes
 * to strip before resolving the rest as prefix+base.
 *
 * At most three digits are consumed. That is not an arbitrary cap but exactly
 * what BVN_EXPONENT_MAX (100) needs, and it is what makes an out-of-range
 * exponent fail: "m^1000" leaves a '0' where the scan expects a '^', so no
 * suffix is recognised, "m^1000" is resolved as a base symbol, and that fails.
 * A value that scans cleanly but lands outside the range -- "m^200" -- is
 * refused by bvn_int_to_exponent returning exp_invalid.
 */
static uint32_t parse_unit_exponent_suffix(
	const char* s, uint32_t len, unit_exponent_t* exp)
{
	/* ── ASCII caret form: "^2", "^-3", "^100", "^-100" ──────────────── */
	{
		uint32_t p = len, nd = 0;
		while (p > 0 && nd < 3 && s[p - 1] >= '0' && s[p - 1] <= '9') {
			p--; nd++;
		}
		if (nd > 0) {
			uint32_t q   = p;
			bool     neg = false;
			if (q > 0 && (s[q - 1] == '-' || s[q - 1] == '+')) {
				neg = (s[q - 1] == '-');
				q--;
			}
			if (q > 0 && s[q - 1] == '^') {
				int32_t v = 0;
				for (uint32_t i = p; i < len; i++)
					v = v * 10 + (s[i] - '0');
				unit_exponent_t e = bvn_int_to_exponent(neg ? -v : v);
				if (e == exp_invalid)
					return 0;
				*exp = e;
				return len - (q - 1);
			}
		}
	}
	/* ── Unicode superscript form: "²", "⁻³", "¹⁰⁰", "⁻¹⁰⁰" ──────────── */
	{
		uint32_t p = len, nd = 0;
		int      digs[3];
		while (nd < 3) {
			uint32_t bl = 0;
			int      d  = bvni_sup_digit_back(s, p, &bl);
			if (d < 0)
				break;
			digs[nd++] = d;
			p -= bl;
		}
		if (nd == 0)
			return 0;
		bool negative = false;
		if (p >= 3 && (uint8_t)s[p - 3] == 0xE2 &&
		              (uint8_t)s[p - 2] == 0x81) {
			uint8_t b = (uint8_t)s[p - 1];
			if      (b == 0xBB) { negative = true;  p -= 3; }
			else if (b == 0xBA) { negative = false; p -= 3; }
		}
		int32_t v = 0;
		for (int32_t i = (int32_t)nd - 1; i >= 0; i--)
			v = v * 10 + digs[i];
		unit_exponent_t e = bvn_int_to_exponent(negative ? -v : v);
		if (e == exp_invalid)
			return 0;
		*exp = e;
		return len - p;
	}
}
static unit_exponent_t bvn_negate_exponent(unit_exponent_t e)
{
	int32_t n = bvn_exponent_to_int(e);
	return (n == 0) ? e : bvn_int_to_exponent(-n);
}
/*
 * Parse one unit component "[prefix[~]]base[^exp]" into its structured form.
 *
 * Order of operations matters: the exponent suffix is removed first, then the
 * remainder is resolved as base ± prefix. Currencies are checked before the SI
 * table because a currency code may itself look like a prefixed unit; their '$'
 * sigil is what dispatches them, and it separates an (SI/IEC) prefix from the
 * code on its own, so the '~' between the two is optional. For
 * physical units the base symbol is matched as the longest suffix (via the
 * length index) and whatever precedes it must be a known prefix, written either
 * with the '~' separator ("k~g") or compactly ("kg" — see the branch below).
 * bvn_prefix_unit_valid rejects nonsensical prefix/unit pairings (e.g. a
 * binary IEC prefix on a non-information unit). *ok reports validity.
 */
static value_unit_component_t bvn_parse_single_unit_component(
	const char* s, uint32_t len, bool* ok)
{
	value_unit_component_t r = {
		.base = bu_none,
		.exponent = exp_linear,
		.prefix.system = prefix_si,
		.prefix.id.si  = si_none
	};
	*ok = true;
	if (!s || !len) { *ok = false; return r; }
	uint32_t suf = parse_unit_exponent_suffix(s, len, &r.exponent);
	len -= suf;
	if (len == 0) { *ok = false; return r; }
	{
		/* Currencies carry a mandatory '$' sigil (spec 1.0): standalone "$USD",
		 * or prefixed "<prefix>[~]$EUR".  A bare code is no longer a currency,
		 * so the CUP/cup namespace collision can no longer arise. */
		if (s[0] == '$') {
			int cid = bvn_parse_currency_str((const uint8_t*)s + 1, len - 1);
			if (cid > 0) {
				r.base = (value_base_unit_t)cid;
				return r;
			}
			*ok = false;   /* '$' introduces a currency and nothing else */
			return r;
		}
		/* A '$' anywhere past the start means everything before it is the
		 * prefix, written with the separator ("k~$EUR") or compactly
		 * ("k$EUR"). The sigil itself already separates the two, so the
		 * compact form is unambiguous by construction — no currency code and
		 * no prefix symbol can contain a '$'. A component with no '$' at all
		 * is a physical unit and falls through below. */
		const char* sig = (const char*)memchr(s, '$', len);
		if (sig) {
			uint32_t sig_off = (uint32_t)(sig - s);
			uint32_t pfx_len = (s[sig_off - 1] == '~') ? sig_off - 1 : sig_off;
			uint32_t code_off = sig_off + 1;
			uint32_t code_len = len - code_off;
			int cid = pfx_len ? bvn_parse_currency_str(
				(const uint8_t*)s + code_off, code_len) : 0;
			if (cid <= 0) {
				/* Unknown code, an empty prefix ("~$EUR"), or a prefix that is
				 * itself sigil-led: '$' introduces a currency and nothing else. */
				*ok = false;
				return r;
			}
			r.base = (value_base_unit_t)cid;
			for (const iec_entry_t* e = iec_table; e->a; e++) {
				if (e->len == pfx_len && memcmp(s, e->a, pfx_len) == 0) {
					r.prefix.system = prefix_iec;
					r.prefix.id.iec = e->p;
					if (!bvn_prefix_unit_valid(r.prefix, r.base))
						*ok = false;
					return r;
				}
			}
			for (const si_entry_t* e = si_table; e->a; e++) {
				if (e->len == pfx_len && memcmp(s, e->a, pfx_len) == 0) {
					r.prefix.system = prefix_si;
					r.prefix.id.si  = e->p;
					if (!bvn_prefix_unit_valid(r.prefix, r.base))
						*ok = false;
					return r;
				}
			}
			*ok = false;   /* not a known prefix before the sigil */
			return r;
		}
	}
	uint32_t bu_skip_for_input;
	{
		uint32_t lkup = len > bu_max_len ? bu_max_len : len;
		bu_skip_for_input = bu_first_for_len[lkup];
	}
	const bu_entry_t* best = NULL;
	for (const bu_entry_t* e = &bu_table[bu_skip_for_input]; e->a; e++) {
		if (e->len > len) continue;
		if (memcmp(s + len - e->len, e->a, e->len) == 0) {
			best = e;
			break;
		}
	}
	if (!best) { *ok = false; return r; }
	r.base = best->u;
	uint32_t plen = len - best->len;
	if (plen == 0) {
		r.prefix.system = prefix_si;
		r.prefix.id.si  = si_none;
		return r;
	}
	/* Whatever precedes the base symbol is the prefix, written either with the
	 * '~' separator ("k~g") or compactly ("kg"). The compact form is reachable
	 * only here, where a missing separator used to end the parse, so widening
	 * it cannot change how any existing document decodes: the base symbol is
	 * still the LONGEST matching alias suffix, which keeps a bare unit ahead of
	 * any prefixed reading ("min" is the minute, not milli-inch). A remainder
	 * that still contains a '~' is a malformed separated form, not a compact
	 * one, and stays an error — as does a spelling that is compact but refused
	 * by name (see compact_except_table). */
	uint32_t plen2;
	if (plen >= 2 && s[plen - 1] == '~') {
		plen2 = plen - 1;
	} else if (memchr(s, '~', plen) != NULL ||
	           bvn_compact_form_refused(s, len)) {
		*ok = false;
		return r;
	} else {
		plen2 = plen;
	}
	for (const iec_entry_t* e = iec_table; e->a; e++) {
		if (e->len == plen2 &&
			memcmp(s, e->a, plen2) == 0) {
			r.prefix.system = prefix_iec;
			r.prefix.id.iec = e->p;
			if (!bvn_prefix_unit_valid(r.prefix, r.base)) {
				*ok = false;
			}
			return r;
		}
	}
	for (const si_entry_t* e = si_table; e->a; e++) {
		if (e->len == plen2 &&
			memcmp(s, e->a, plen2) == 0) {
			r.prefix.system = prefix_si;
			r.prefix.id.si  = e->p;
			if (!bvn_prefix_unit_valid(r.prefix, r.base)) {
				*ok = false;
			}
			return r;
		}
	}
	*ok = false;
	return r;
}
/*
 * Parse a complete unit expression into a value_unit_t (up to
 * BVNR_MAX_UNIT_COMPONENTS components). The literal "no_unit" maps to the empty
 * unit. A component-separated expression uses '*' / '·' for multiplication and
 * '/' to switch into the denominator, where component exponents are negated so
 * "m/s^2" becomes [m^1, s^-2]. With no separator the whole string is a single
 * component, handled by the fast path. bvn_parse_unit is the convenience
 * NUL-terminated wrapper around this length-counted form.
 */
/*
 * Parenthesised grouping in unit expressions (spec 1.0). A "(...)" group is a
 * sub-expression parsed independently; like any factor it obeys the sticky
 * denominator, so a "/" before a group negates the group's net component
 * exponents. This makes the readable forms work and compose correctly:
 *   k~g/(m·s²)   -> kg·m⁻¹·s⁻²     (pressure)
 *   (k~g/m)·s²   -> kg·m⁻¹·s²
 *   a/(b/c)      -> a·c·b⁻¹
 * Parenless expressions are unaffected (identical to the pre-1.0 flat parser).
 * An explicit separator is required between a factor and a group ("m·(s)", not
 * "m(s)"); a group is not yet followed by its own exponent. Nesting is bounded.
 */
#define BVN_UNIT_GROUP_MAX_DEPTH 16u
static value_unit_t bvn_parse_unit_expr(
	const char* s, uint32_t slen, uint32_t depth, bool* ok)
{
	value_unit_t result = { .num_components = 0 };
	*ok = true;
	if (!s || !slen)                    { *ok = false; return result; }
	if (depth > BVN_UNIT_GROUP_MAX_DEPTH) { *ok = false; return result; }
	if (slen == 7 && memcmp(s, "no_unit", 7) == 0)
		return result;                  /* the empty (dimensionless) unit */

	bool     in_denominator = false;
	uint32_t i              = 0;
	while (i < slen) {
		if (s[i] == '(') {
			/* Factor is a parenthesised group: find the matching ')'. */
			uint32_t pd = 1, j = i + 1;
			for (; j < slen && pd; j++) {
				if (s[j] == '(')      pd++;
				else if (s[j] == ')') pd--;
				if (pd == 0) break;
			}
			if (pd != 0) { *ok = false; return result; }  /* unmatched '(' */
			bool gok;
			value_unit_t grp = bvn_parse_unit_expr(
				s + i + 1, j - (i + 1), depth + 1, &gok);
			if (!gok) { *ok = false; return result; }
			uint32_t gn = grp.num_components < BVNR_MAX_UNIT_COMPONENTS
			            ? grp.num_components : BVNR_MAX_UNIT_COMPONENTS;
			for (uint32_t k = 0; k < gn; k++) {
				if (result.num_components >= BVNR_MAX_UNIT_COMPONENTS) {
					*ok = false; return result;
				}
				value_unit_component_t c = grp.components[k];
				if (in_denominator)
					c.exponent = bvn_negate_exponent(c.exponent);
				result.components[result.num_components++] = c;
			}
			i = j + 1;                  /* advance past ')' */
		} else {
			/* Factor is a plain component: scan to the next top-level
			 * separator or '('. */
			uint32_t start = i;
			while (i < slen) {
				uint8_t b = (uint8_t)s[i];
				if (b == '*' || b == '/' || b == '(')
					break;
				if (b == 0xC2 && i + 1 < slen &&
					(uint8_t)s[i + 1] == 0xB7)
					break;
				i++;
			}
			if (i == start) { *ok = false; return result; } /* empty */
			bool comp_ok;
			value_unit_component_t comp = bvn_parse_single_unit_component(
				s + start, i - start, &comp_ok);
			if (!comp_ok) { *ok = false; return result; }
			if (in_denominator)
				comp.exponent = bvn_negate_exponent(comp.exponent);
			if (result.num_components >= BVNR_MAX_UNIT_COMPONENTS) {
				*ok = false; return result;
			}
			result.components[result.num_components++] = comp;
		}
		/* Consume the separator between factors (or stop at end). */
		if (i < slen) {
			uint8_t b = (uint8_t)s[i];
			if (b == '/')      { in_denominator = true; i++; }
			else if (b == '*') { i++; }
			else if (b == 0xC2 && i + 1 < slen &&
					 (uint8_t)s[i + 1] == 0xB7) { i += 2; }
			else { *ok = false; return result; } /* '(' (implicit mult) or
			                                       * an exponent after ')' */
			if (i >= slen) { *ok = false; return result; } /* trailing sep */
		}
	}
	if (result.num_components == 0)
		*ok = false;
	return result;
}
value_unit_t bvn_parse_unit_n(const uint8_t* unit, uint32_t len, bool* ok)
{
	/* `ok` is optional: a caller that only wants the unit can read
	 * num_components, since a refused parse returns BVN_UNIT_NONE. The input
	 * pointer was already guarded here and this one was not, which is the kind
	 * of asymmetry that only shows up in somebody else's crash report. */
	bool ok_scratch = false;
	if (!ok)
		ok = &ok_scratch;
	*ok = true;
	if (!unit || !len) {
		*ok = false;
		return (value_unit_t){ .num_components = 0 };
	}
	/*
	 * spec 1.2 — a "name:" namespace hands the string to the unit profile
	 * (doc/11_bovnar_unit_profiles.md). This is the SINGLE door: every path that
	 * reaches a unit goes through bvn_parse_unit, so a profile unit cannot arrive by a
	 * route that skips a check, and the policy strings of bvnr_unit_policy_t
	 * accept the notation with no change at all.
	 *
	 * A ':' in a unit was already an error before this, and no native alias or
	 * currency code contains one, so nothing that parsed yesterday changes
	 * meaning.
	 */
	if (bvni_unit_has_profile((const char*)unit, len)) {
		bvni_profile_status_t st = bvni_profile_ok;
		value_unit_t u = bvni_parse_profile_unit((const char*)unit, len, &st);
		*ok = (st == bvni_profile_ok);
		return u;
	}
	return bvn_parse_unit_expr((const char*)unit, len, 0, ok);
}
/*
 * Which error a rejected unit string deserves. Re-parses, deliberately: this is
 * the error path, where one more pass over at most 255 bytes costs nothing, and
 * the alternative — threading a status out of bvn_parse_unit — would change the
 * signature of a public function every caller in the tree already uses.
 */
error_code_t bvn_unit_error_code(const uint8_t* unit, uint32_t len)
{
	if (!unit || !len)
		return error_unit_illegal;
	if (!bvni_unit_has_profile((const char*)unit, len)) {
		bool ok = true;
		(void)bvn_parse_unit_expr((const char*)unit, len, 0, &ok);
		return ok ? error_none : error_unit_illegal;
	}
	bvni_profile_status_t st = bvni_profile_ok;
	(void)bvni_parse_profile_unit((const char*)unit, len, &st);
	switch (st) {
	case bvni_profile_ok:              return error_none;
	case bvni_profile_unsupported:     return error_unit_profile_unsupported;
	case bvni_profile_unknown_ns: return error_unit_profile_unknown;
	default:                        return error_unit_illegal;
	}
}
bool bvn_unit_is_profile_only(value_unit_t u)
{
	return bvni_unit_has_opaque(u);
}
int32_t bvn_unit_to_profile(const char* ns, value_unit_t u, char* buf,
                            size_t bufsize)
{
	if (!ns)
		return -1;
	return bvni_unit_to_profile(ns, (uint32_t)strlen(ns), u, buf, bufsize);
}
int32_t bvn_unit_to_ucum(value_unit_t u, char* buf, size_t bufsize)
{
	return bvni_unit_to_ucum(u, buf, bufsize);
}
uint32_t bvn_unit_profile_count(void)
{
	return bvni_profile_count();
}
const char* bvn_unit_profile_name(uint32_t index)
{
	return bvni_profile_name(index);
}
value_unit_t bvn_parse_unit(const uint8_t* unit, bool* ok)
{
	if (!unit) {
		if (ok) *ok = false;
		return (value_unit_t){ .num_components = 0 };
	}
	return bvn_parse_unit_n(unit, (uint32_t)strlen((const char*)unit), ok);
}
static const char* si_prefix_str(si_prefix_id_t p)
{
	switch (p) {
#include "bovnar_si_prefix_str.gen.inc"
	default:       return "";
	}
}
static const char* iec_prefix_str(iec_prefix_id_t p)
{
	switch (p) {
#include "bovnar_iec_prefix_str.gen.inc"
	default:       return "";
	}
}
static const char* base_unit_str(value_base_unit_t b)
{
	switch (b) {
	/* Physical-unit canonical symbols — generated from src/gendata/units.bvnr by
	 * gen_units.py. The default: case below handles currencies. */
#include "bovnar_base_unit_str.gen.inc"
	/* UCUM arbitrary units — generated from src/gendata/ucum.bvnr by
	 * gen_profiles.py. Their canonical spelling IS the UCUM code, because they have
	 * no native one; bvn_unit_to_string wraps the whole expression in "ucum:"
	 * when one of these is present. */
#include "bovnar_profiles_str.gen.inc"
	default:
		if (bvn_unit_is_currency((int)b)) {
			const bvn_currency_info_t *info = bvn_currency_info((int)b);
			if (info) return info->code;
		}
		return "";
	}
}
/*
 * Render an exponent as a Unicode superscript suffix (² ³ ⁻² ⁻¹⁰⁰). This is the
 * default, pretty form; bvn_write_exponent_suffix_ascii produces the plain
 * "^2" / "^-2" form selected by BVN_UNIT_ASCII_EXP for ASCII-only consumers.
 * exp_linear writes nothing (the implied exponent 1 is never spelled out).
 *
 * Both were exhaustive switches over ±1..±9. With the range now up to ±100 that
 * is no longer enumerable, so both convert the magnitude to decimal digits and
 * emit them most-significant first. The superscript digits are not one width --
 * ¹ ² ³ are two bytes and ⁰ ⁴-⁹ are three -- so the writer indexes a table of
 * (bytes, length) rather than computing an offset from the digit.
 */
static const struct { const char* utf8; uint8_t len; } bvni_sup_digit[10] = {
	{ "\xe2\x81\xb0", 3 },  /* ⁰ */
	{ "\xc2\xb9",     2 },  /* ¹ */
	{ "\xc2\xb2",     2 },  /* ² */
	{ "\xc2\xb3",     2 },  /* ³ */
	{ "\xe2\x81\xb4", 3 },  /* ⁴ */
	{ "\xe2\x81\xb5", 3 },  /* ⁵ */
	{ "\xe2\x81\xb6", 3 },  /* ⁶ */
	{ "\xe2\x81\xb7", 3 },  /* ⁷ */
	{ "\xe2\x81\xb8", 3 },  /* ⁸ */
	{ "\xe2\x81\xb9", 3 },  /* ⁹ */
};
static int32_t bvn_write_exponent_suffix(
	char* buf, size_t bufsize, unit_exponent_t e)
{
	int32_t n = bvn_exponent_to_int(e);
	if (n == 0)
		return -1;                     /* exp_invalid has no spelling */
	if (n == 1) {                          /* implied, never written */
		if (bufsize < 1) return -1;
		buf[0] = '\0';
		return 0;
	}
	int32_t pos = 0;
	if (n < 0) {
		if (pos + 3 >= (int32_t)bufsize) return -1;
		buf[pos++] = (char)0xe2;       /* ⁻ U+207B */
		buf[pos++] = (char)0x81;
		buf[pos++] = (char)0xbb;
		n = -n;
	}
	int32_t digs[3], nd = 0;
	while (n > 0 && nd < 3) { digs[nd++] = n % 10; n /= 10; }
	for (int32_t i = nd - 1; i >= 0; i--) {
		const char* d   = bvni_sup_digit[digs[i]].utf8;
		int32_t     dl  = bvni_sup_digit[digs[i]].len;
		if (pos + dl >= (int32_t)bufsize) return -1;
		memcpy(buf + pos, d, (size_t)dl);
		pos += dl;
	}
	buf[pos] = '\0';
	return pos;
}
static int32_t bvn_write_exponent_suffix_ascii(
	char* buf, size_t bufsize, unit_exponent_t e)
{
	int32_t n = bvn_exponent_to_int(e);
	if (n == 0)
		return -1;
	if (n == 1) {
		if (bufsize < 1) return -1;
		buf[0] = '\0';
		return 0;
	}
	int32_t pos = 0;
	if (pos + 1 >= (int32_t)bufsize) return -1;
	buf[pos++] = '^';
	if (n < 0) {
		if (pos + 1 >= (int32_t)bufsize) return -1;
		buf[pos++] = '-';
		n = -n;
	}
	int32_t digs[3], nd = 0;
	while (n > 0 && nd < 3) { digs[nd++] = n % 10; n /= 10; }
	for (int32_t i = nd - 1; i >= 0; i--) {
		if (pos + 1 >= (int32_t)bufsize) return -1;
		buf[pos++] = (char)('0' + digs[i]);
	}
	buf[pos] = '\0';
	return pos;
}
/*
 * Format one component as "[prefix~]base[exp]" — the inverse of
 * bvn_parse_single_unit_component. _ex is the same but routes the exponent
 * through the ASCII or Unicode writer per flags. Every write is bounds-checked
 * against bufsize and returns -1 on overflow so the caller can surface
 * error_unit_too_long rather than truncate.
 */
static int32_t bvn_write_unit_component(
	char* buf, size_t bufsize, const value_unit_component_t* c)
{
	int32_t pos = 0;
	bool has_prefix = false;
	if (c->prefix.system == prefix_iec && c->prefix.id.iec != iec_none) {
		const char* p = iec_prefix_str(c->prefix.id.iec);
		size_t plen = strlen(p);
		if (pos + (int32_t)plen >= (int32_t)bufsize) return -1;
		memcpy(buf + pos, p, plen);
		pos += (int32_t)plen;
		has_prefix = true;
	} else if (c->prefix.system == prefix_si && c->prefix.id.si != si_none) {
		const char* p = si_prefix_str(c->prefix.id.si);
		size_t plen = strlen(p);
		if (pos + (int32_t)plen >= (int32_t)bufsize) return -1;
		memcpy(buf + pos, p, plen);
		pos += (int32_t)plen;
		has_prefix = true;
	}
	if (has_prefix) {
		if (pos + 1 >= (int32_t)bufsize) return -1;
		buf[pos++] = '~';
	}
	if (bvn_unit_is_currency((int)c->base)) {
		/* Mandatory currency sigil (spec 1.0) so output round-trips: "$USD",
		 * and "<prefix>~$EUR" once the prefix/'~' above is already written. */
		if (pos + 1 >= (int32_t)bufsize) return -1;
		buf[pos++] = '$';
	}
	{
		const char* bu = base_unit_str(c->base);
		size_t bulen = strlen(bu);
		if (pos + (int32_t)bulen >= (int32_t)bufsize) return -1;
		memcpy(buf + pos, bu, bulen);
		pos += (int32_t)bulen;
	}
	int32_t w = bvn_write_exponent_suffix(
		buf + pos, bufsize - (size_t)pos,
		c->exponent);
	if (w < 0) return -1;
	pos += w;
	return pos;
}
static int32_t bvn_write_unit_component_ex(
	char* buf, size_t bufsize,
	const value_unit_component_t* c,
	bvn_unit_flags_t flags)
{
	int32_t pos = 0;
	bool has_prefix = false;
	if (c->prefix.system == prefix_iec && c->prefix.id.iec != iec_none) {
		const char* p = iec_prefix_str(c->prefix.id.iec);
		size_t plen = strlen(p);
		if (pos + (int32_t)plen >= (int32_t)bufsize) return -1;
		memcpy(buf + pos, p, plen);
		pos += (int32_t)plen;
		has_prefix = true;
	} else if (c->prefix.system == prefix_si && c->prefix.id.si != si_none) {
		const char* p = si_prefix_str(c->prefix.id.si);
		size_t plen = strlen(p);
		if (pos + (int32_t)plen >= (int32_t)bufsize) return -1;
		memcpy(buf + pos, p, plen);
		pos += (int32_t)plen;
		has_prefix = true;
	}
	if (has_prefix) {
		if (pos + 1 >= (int32_t)bufsize) return -1;
		buf[pos++] = '~';
	}
	if (bvn_unit_is_currency((int)c->base)) {
		/* Mandatory currency sigil (spec 1.0) so output round-trips: "$USD",
		 * and "<prefix>~$EUR" once the prefix/'~' above is already written. */
		if (pos + 1 >= (int32_t)bufsize) return -1;
		buf[pos++] = '$';
	}
	{
		const char* bu = base_unit_str(c->base);
		size_t bulen = strlen(bu);
		if (pos + (int32_t)bulen >= (int32_t)bufsize) return -1;
		memcpy(buf + pos, bu, bulen);
		pos += (int32_t)bulen;
	}
	int32_t w;
	if (flags & BVN_UNIT_ASCII_EXP)
		w = bvn_write_exponent_suffix_ascii(buf + pos,
		                                    bufsize - (size_t)pos,
		                                    c->exponent);
	else
		w = bvn_write_exponent_suffix(buf + pos,
		                               bufsize - (size_t)pos,
		                               c->exponent);
	if (w < 0) return -1;
	pos += w;
	return pos;
}
/*
 * Exact identity of a single unit component (base, exponent, prefix). Helper
 * for the order-insensitive bvn_unit_equal below.
 */
static bool bvn_unit_component_identical(const value_unit_component_t *ca,
                                         const value_unit_component_t *cb)
{
	if (ca->base     != cb->base)     return false;
	if (ca->exponent != cb->exponent) return false;
	/*
	 * "No prefix" has two spellings — (prefix_si, si_none) and (prefix_iec,
	 * iec_none) — and they are indistinguishable everywhere else: both render
	 * to the same text, both score a factor of 1. The parser only ever emits
	 * the SI one, but a caller building an unprefixed bit/byte unit by hand
	 * reaches for prefix_iec naturally, and comparing the system first called
	 * that unequal to the very text it prints — against this function's own
	 * contract that equivalent notations match. Settle the absent-prefix case
	 * before the system is allowed to matter.
	 */
	bool a_none = (ca->prefix.system == prefix_iec)
	            ? ca->prefix.id.iec == iec_none : ca->prefix.id.si == si_none;
	bool b_none = (cb->prefix.system == prefix_iec)
	            ? cb->prefix.id.iec == iec_none : cb->prefix.id.si == si_none;
	if (a_none || b_none) return a_none && b_none;
	if (ca->prefix.system != cb->prefix.system) return false;
	if (ca->prefix.system == prefix_si)
		return ca->prefix.id.si == cb->prefix.id.si;
	return ca->prefix.id.iec == cb->prefix.id.iec;
}
/*
 * Structural equality of two units. This is exact component identity (base,
 * exponent, prefix) — not dimensional equivalence — but it is ORDER-INSENSITIVE:
 * unit multiplication is commutative, so "s³·m⁻⁵" and "m⁻⁵·s³" denote the same
 * unit and must compare equal. The parser preserves source order in the
 * value_unit_t component array, so a positional comparison would wrongly reject
 * two spellings of the same unit (e.g. an annotation unit vs an inline unit in a
 * different order), violating the spec rule that logically equivalent notations
 * compare as equal. We therefore match components as multisets: every component
 * of a must pair with a distinct, identical component of b.
 */
/*
 * ΔK and K, where the temperature-interval kind does not count.
 *
 * The interval kind is scoped to A LONE COMPONENT AT EXPONENT 1, which is
 * exactly where the hazard lives: bvn_unit_to_si_factor applies an affine offset
 * only there, so that is the only place a difference could ever be read as a
 * scale. Everywhere else an affine scale cannot appear at all, so a `K` in a
 * compound was ALREADY an interval and there is nothing to separate --
 * `W/(m²·ΔK)` and `W/(m²·K)` are one unit, as are `ΔK/k~m` and `K/k~m`, and
 * `ΔK⁻¹` and `K⁻¹`.
 *
 * bvni_kind_exponents applies that scoping, so bvn_units_convertible already
 * agreed: the pairs above convert with factor 1. Equality did not, because it
 * compares bases structurally and bu_delta_kelvin is not bu_kelvin -- so the two
 * spellings the design calls one unit were separated one layer up, and a
 * document annotated `W/(m²·ΔK)` with an inline `W/(m²·K)` suffix was refused
 * as error_unit_mismatch. That is precisely the U-value the scoping rule exists
 * to keep working.
 *
 * Only ΔK folds. The other five interval units are not spellings of a kelvin:
 * Δ°F is 5/9 K, and its scale counterpart °F is affine and unusable in a
 * compound anyway, so there is no pair to merge.
 */
static value_base_unit_t bvni_equality_base(value_unit_t u, uint32_t i)
{
	const value_unit_component_t *c = &u.components[i];
	if (c->base != bu_delta_kelvin)
		return c->base;
	bool lone = (u.num_components == 1u);
	return (lone && bvn_exponent_to_int(c->exponent) == 1)
	     ? c->base                       /* the hazard: ΔK is not K */
	     : bu_kelvin;
}
bool bvn_unit_equal(value_unit_t a, value_unit_t b)
{
	if (a.num_components != b.num_components)
		return false;
	uint32_t n = a.num_components < BVNR_MAX_UNIT_COMPONENTS
	           ? a.num_components : BVNR_MAX_UNIT_COMPONENTS;
	bool matched[BVNR_MAX_UNIT_COMPONENTS] = { false };
	for (uint32_t i = 0; i < n; i++) {
		bool found = false;
		value_unit_component_t ca = a.components[i];
		ca.base = bvni_equality_base(a, i);
		for (uint32_t j = 0; j < n; j++) {
			if (matched[j])
				continue;
			value_unit_component_t cb = b.components[j];
			cb.base = bvni_equality_base(b, j);
			if (bvn_unit_component_identical(&ca, &cb)) {
				matched[j] = true;
				found = true;
				break;
			}
		}
		if (!found)
			return false;
	}
	return true;
}
bool bvn_unit_valid(value_unit_t u)
{
	if (u.num_components > BVNR_MAX_UNIT_COMPONENTS)
		return false;
	for (uint32_t i = 0; i < u.num_components; i++) {
		const value_unit_component_t *c = &u.components[i];
		/* bvn_exponent_to_int returns 0 both for exp_invalid and for a value
		 * outside unit_exponent_t. Checking only for exp_invalid let an
		 * exponent of, say, 10 through here and into arithmetic that cannot
		 * represent it — the formatter dropped it silently and the factor
		 * helpers had no defined answer. */
		if (bvn_exponent_to_int(c->exponent) == 0)
			return false;
		if (!bvni_base_defined(c->base))
			return false;
		if (!bvn_prefix_unit_valid(c->prefix, c->base))
			return false;
	}
	return true;
}
static const value_base_unit_t bvni_si_named_derived[] = {
	bu_newton, bu_pascal, bu_joule, bu_watt,
	bu_volt, bu_ohm, bu_farad, bu_coulomb, bu_siemens,
	bu_weber, bu_tesla, bu_henry, bu_lux, bu_lumen,
	bu_hertz, bu_becquerel, bu_gray, bu_sievert, bu_katal,
};
#define BVNI_SI_NAMED_DERIVED_COUNT \
	((uint32_t)(sizeof(bvni_si_named_derived)/sizeof(bvni_si_named_derived[0])))
static int32_t bvni_pexp_to_si_prefix_id(int32_t pexp)
{
	for (uint32_t i = 0; i < BVN_SI_PREFIX_COUNT; i++) {
		if (bvni_si_pfx_table[i].exp == pexp)
			return (int32_t)i;
	}
	return -1;
}
/* The SI prefix for `v` where it is a power of ten that has one, else -1.
 * Matched against the exact power-of-ten ladder rather than by a logarithm. */
static int32_t bvni_p10_si_prefix_id(double v)
{
	for (int32_t e = -30; e <= 30; e++) {
		double p = bvni_pow10(e);
		if (fabs(v - p) <= 1e-6 * p)
			return bvni_pexp_to_si_prefix_id(e);
	}
	return -1;
}
/*
 * Try to collapse a reduced unit back into a single named SI derived unit with
 * the right prefix (e.g. kg·m/s² -> N, or 1000 of those -> kN). It computes the
 * unit's SI dimension vector and net scale factor, finds a named derived unit
 * (bvni_si_named_derived) with the same dimensions, and — if the leftover scale
 * is an exact power of ten that maps to an SI prefix — emits that prefixed
 * named unit. Only invoked under BVN_UNIT_REDUCE; affine units (those with an
 * offset, like °C) are left alone because they can't be rescaled by a factor.
 */
static value_unit_t bvni_reduce_to_named_si(value_unit_t u, double scale,
					    bool scale_known)
{
	bool aff, ok;
	double off;
	double base_si = bvn_unit_to_si_factor(u, &aff, &off, &ok);
	if (!ok || aff)
		return u;
	/* A folded scale that no double holds has no prefix either, so it goes
	 * straight to the second lookup below -- the prefix of the reduced unit's
	 * own factor -- and stays in the value. It must not skip the collapse
	 * altogether: q~kg¹¹·m/kg¹⁰·s² reduces to kg·m/s², and the next pass, with
	 * nothing left to fold, would write N. */
	double net_si = scale_known ? scale * base_si : 0.0;
	if (scale_known && (net_si <= 0.0 || !isfinite(net_si)))
		return u;
	int32_t dim_r[bvn_si_dim_count];
	if (!bvn_unit_dimension_vector(u, dim_r))
		return u;
	for (uint32_t n = 0; n < BVNI_SI_NAMED_DERIVED_COUNT; n++) {
		value_base_unit_t nd = bvni_si_named_derived[n];
		value_unit_t probe;
		memset(&probe, 0, sizeof(probe));
		probe.num_components = 1;
		probe.components[0].base      = nd;
		probe.components[0].exponent  = exp_linear;
		probe.components[0].prefix.system   = prefix_si;
		probe.components[0].prefix.id.si    = si_none;
		int32_t dim_nd[bvn_si_dim_count];
		if (!bvn_unit_dimension_vector(probe, dim_nd))
			continue;
		bool dim_match = true;
		for (int d = 0; d < bvn_si_dim_count; d++) {
			if (dim_r[d] != dim_nd[d]) {
				dim_match = false;
				break;
			}
		}
		if (!dim_match)
			continue;
		/* The SI dimension vector alone is not enough: it is [0,...,0] for every
		 * dimensionless quantity kind, so "B/s" matched "Hz" on dimension and
		 * collapsed onto it — turning a data rate into a frequency, a unit the
		 * library itself would call incompatible with the one it replaced. Same
		 * for rad/s against Hz. Require the kind vectors to agree too. */
		if (!bvni_kinds_match(u, probe))
			continue;
		/*
		 * Never SUBSTITUTE one named unit for another. This function exists to
		 * fold a compound back into the named unit it spells out — kg·m/s² -> N,
		 * A·s -> C — and to restore the prefix that bvn_unit_reduce folded into
		 * the scale (k~N reduces to N with scale 1000, and comes back k~N, same
		 * base). What it must not do is rewrite a unit that ALREADY names its
		 * quantity: Sv and Gy share a dimension vector and carry no kind, so the
		 * first match in the table won and BVN_UNIT_REDUCE turned an equivalent
		 * dose into an absorbed dose — in the document, not merely in a
		 * conversion. Same for Bq -> Hz, Bd -> Hz, var -> W, rem -> c~Gy. Those
		 * conversions stay available through bvn_unit_convert_factor, which is a
		 * caller asking for one; a serialisation is not.
		 *
		 * The test is deliberately narrow: a lone base at exponent ONE is a unit
		 * that already names its quantity, and swapping it is the defect above.
		 * A lone base at any other exponent is a compound in all but spelling —
		 * s⁻¹ IS the thing hertz names — and collapsing it is the whole point of
		 * this function. Guarding on the component count alone conflated the two
		 * and cost more than it saved: s⁻¹ stopped becoming Hz, and m~s⁻¹ (which
		 * reduces to s⁻¹ with a scale of 1000, and used to come back as k~Hz)
		 * lost that prefix with the scale still folded out.
		 */
		if (u.num_components == 1 &&
		    u.components[0].exponent == exp_linear &&
		    u.components[0].base != nd)
			continue;
		/*
		 * Only an (almost-)exact power of ten maps onto a single SI
		 * prefix. Find that integer power by matching net_si against the
		 * exact power-of-ten ladder spanning the SI prefix range, rather
		 * than taking a logarithm — net_si is a product of exact prefix
		 * factors, so a genuine match is within a few ulp while anything
		 * else is off by orders of magnitude. The 1e-6 relative window
		 * mirrors the old fabs(log10(net_si) - pexp) > 1e-6 gate.
		 */
		int32_t pfx_id = scale_known ? bvni_p10_si_prefix_id(net_si) : -1;
		/* No prefix spells the whole factor (N/c~m² is 10^4 Pa; d~J·ppm is
		 * 10^-7 J). This used to leave the compound as it was -- and the NEXT
		 * pass, with the folded scale now gone from the unit, collapsed it
		 * after all: the reduced form was not a fixed point. Take the prefix
		 * the next pass would find, the one for the reduced unit's own SI
		 * factor; the folded scale then stays in the value, which the caller
		 * moves by the factor between the unit it had and the unit it is given
		 * back (`scale` is not 1 here, or the first lookup would have matched,
		 * so bvn_ser_reduced_number's scale==1 shortcut is not taken). */
		if (pfx_id < 0)
			pfx_id = bvni_p10_si_prefix_id(base_si);
		if (pfx_id < 0)
			continue;
		value_unit_t result;
		memset(&result, 0, sizeof(result));
		result.num_components = 1;
		result.components[0].base           = nd;
		result.components[0].exponent       = exp_linear;
		result.components[0].prefix.system  = prefix_si;
		result.components[0].prefix.id.si   = (si_prefix_id_t)pfx_id;
		return result;
	}
	return u;
}
/*
 * Format a whole unit to text. The empty unit prints as "no_unit". A
 * multi-component unit is grouped into numerator (positive exponents) and
 * denominator (negative exponents): if both are present it prints
 * "a·b/c·d" with denominator exponents negated back to positive; otherwise it
 * prints a flat "·"-joined product. The separator is '·' (U+00B7) normally or
 * '*' under BVN_UNIT_ASCII_EXP. With BVN_UNIT_REDUCE it first reduces to SI base
 * units and attempts the named-SI collapse above. bvn_unit_to_string is the
 * plain wrapper with default (Unicode, non-reducing) flags.
 */
int32_t bvn_unit_to_string_ex(value_unit_t u, char* buf, size_t bufsize,
                               bvn_unit_flags_t flags)
{
	if (!buf || bufsize < 1)
		return -1;
	/*
	 * spec 1.2 — a unit carrying a UCUM arbitrary atom has NO native spelling,
	 * so the profile prefix is part of its canonical form and the whole
	 * expression is written in profile notation. Re-parsing that output yields
	 * the same value_unit_t, which is what closes the round trip without any
	 * state living outside the struct.
	 *
	 * This runs before the flag handling below on purpose: BVN_UNIT_REDUCE folds
	 * a unit towards named SI, and an arbitrary unit has no SI form to fold
	 * towards — its conversion row exists only to keep the table well-formed.
	 */
	if (bvni_unit_has_opaque(u)) {
		if (!bvn_unit_valid(u))
			return -1;
		return bvni_unit_profile_spelling(u, buf, bufsize);
	}
	if (flags & BVN_UNIT_REDUCE) {
		double   scale;
		bool     dim_lost, scale_lost;
		if (!bvn_unit_valid(u))
			return -1;
		u = bvni_unit_reduce_ex(u, &scale, &dim_lost, &scale_lost);
		/*
		 * A reduction that DROPS a component does not produce a shorter spelling
		 * of the same unit — it produces a DIFFERENT one. That happens when a
		 * summed exponent leaves the ±100 the format can spell, or when more
		 * bases survive than a unit may carry: m⁵⁰·m⁵¹ (which is m¹⁰¹) came back
		 * with no components at all and formatted as "no_unit", and m⁵⁰·m⁵¹·s as
		 * "s". Dimensionless, and not compatible with what it replaced — the
		 * wrong-unit-is-worse-than-a-refused-one failure this format exists to
		 * prevent, handed to the caller as a success.
		 *
		 * The flag was already computed and was consulted only to skip the
		 * cosmetic named-SI collapse. Refuse instead: -1 is this function's
		 * existing answer for a unit it cannot write (see the bu_none guard
		 * below), it is what bvnr_writer already demanded of this path, and it
		 * is what the Python binding's unit_reduce() already raises on.
		 *
		 * A folded scale too large or too small for a double is NOT that case
		 * and is not refused: the unit is intact, and this function returns a
		 * unit, not a scale. See bvni_unit_reduce_ex.
		 */
		if (dim_lost)
			return -1;
		u = bvni_reduce_to_named_si(u, scale, !scale_lost);
	} else {
		if (!bvn_unit_valid(u))
			return -1;
	}
	uint32_t nc = u.num_components < BVNR_MAX_UNIT_COMPONENTS
	            ? u.num_components : BVNR_MAX_UNIT_COMPONENTS;
	if (nc == 0) {
		if (bufsize < 8)
			return -1;
		memcpy(buf, "no_unit", 7);
		buf[7] = '\0';
		return 7;
	}
	if (nc == 1
		&& u.components[0].base == bu_none
		&& u.components[0].prefix.system == prefix_si
		&& u.components[0].prefix.id.si == si_none
		&& u.components[0].exponent == exp_linear) {
		if (bufsize < 8)
			return -1;
		memcpy(buf, "no_unit", 7);
		buf[7] = '\0';
		return 7;
	}
	/* Past the "no_unit" shapes above, a bu_none component has no spelling: the
	 * base contributes no symbol, so a prefixed or exponentiated one formats as
	 * a bare "k~" or "²" and a compound as "m·k~" — text this library's own
	 * parser rejects, which the writer would then embed in a document. Refuse
	 * instead of emitting something unreadable. */
	for (uint32_t i = 0; i < nc; i++) {
		if (u.components[i].base == bu_none)
			return -1;
	}
	bool ascii = (flags & BVN_UNIT_ASCII_EXP) != 0;
	if (nc == 1) {
		return bvn_write_unit_component_ex(
			buf, bufsize, &u.components[0], flags);
	}
	uint32_t num_num = 0, num_den = 0;
	uint32_t num_idx[BVNR_MAX_UNIT_COMPONENTS];
	uint32_t den_idx[BVNR_MAX_UNIT_COMPONENTS];
	for (uint32_t i = 0; i < nc; i++) {
		if (bvni_is_neg_exp(u.components[i].exponent))
			den_idx[num_den++] = i;
		else
			num_idx[num_num++] = i;
	}
	int32_t pos = 0;
	if (num_num > 0 && num_den > 0) {
		for (uint32_t n = 0; n < num_num; n++) {
			if (n > 0) {
				if (ascii) {
					if (pos + 1 >= (int32_t)bufsize) return -1;
					buf[pos++] = '*';
				} else {
					if (pos + 2 >= (int32_t)bufsize) return -1;
					buf[pos++] = (char)0xc2;
					buf[pos++] = (char)0xb7;
				}
			}
			int32_t w = bvn_write_unit_component_ex(
				buf + pos, bufsize - (size_t)pos,
				&u.components[num_idx[n]], flags);
			if (w < 0) return -1;
			pos += w;
		}
		if (pos + 1 >= (int32_t)bufsize) return -1;
		buf[pos++] = '/';
		for (uint32_t d = 0; d < num_den; d++) {
			if (d > 0) {
				if (ascii) {
					if (pos + 1 >= (int32_t)bufsize) return -1;
					buf[pos++] = '*';
				} else {
					if (pos + 2 >= (int32_t)bufsize) return -1;
					buf[pos++] = (char)0xc2;
					buf[pos++] = (char)0xb7;
				}
			}
			value_unit_component_t tmp = u.components[den_idx[d]];
			tmp.exponent = bvn_negate_exponent(tmp.exponent);
			int32_t w = bvn_write_unit_component_ex(
				buf + pos, bufsize - (size_t)pos,
				&tmp, flags);
			if (w < 0) return -1;
			pos += w;
		}
	} else {
		for (uint32_t i = 0; i < nc; i++) {
			if (i > 0) {
				if (ascii) {
					if (pos + 1 >= (int32_t)bufsize) return -1;
					buf[pos++] = '*';
				} else {
					if (pos + 2 >= (int32_t)bufsize) return -1;
					buf[pos++] = (char)0xc2;
					buf[pos++] = (char)0xb7;
				}
			}
			int32_t w = bvn_write_unit_component_ex(
				buf + pos, bufsize - (size_t)pos,
				&u.components[i], flags);
			if (w < 0) return -1;
			pos += w;
		}
	}
	buf[pos] = '\0';
	return pos;
}
int32_t bvn_unit_to_string(value_unit_t u, char* buf, size_t bufsize)
{
	if (!buf || bufsize < 1)
		return -1;
	if (!bvn_unit_valid(u))
		return -1;
	/* spec 1.2 — a unit carrying an opaque base unit has no native spelling, so
	 * its canonical form is the profile one, in the namespace that owns it. Same
	 * rule as in bvn_unit_to_string_ex; the two formatters are separate
	 * implementations and a unit that printed differently through them would be
	 * a round-trip hole. */
	if (bvni_unit_has_opaque(u))
		return bvni_unit_profile_spelling(u, buf, bufsize);
	uint32_t nc = u.num_components < BVNR_MAX_UNIT_COMPONENTS
	            ? u.num_components : BVNR_MAX_UNIT_COMPONENTS;
	if (nc == 0) {
		if (bufsize < 8)
			return -1;
		memcpy(buf, "no_unit", 7);
		buf[7] = '\0';
		return 7;
	}
	if (nc == 1
		&& u.components[0].base == bu_none
		&& u.components[0].prefix.system == prefix_si
		&& u.components[0].prefix.id.si == si_none
		&& u.components[0].exponent == exp_linear) {
		if (bufsize < 8)
			return -1;
		memcpy(buf, "no_unit", 7);
		buf[7] = '\0';
		return 7;
	}
	/* Same guard as bvn_unit_to_string_ex: a bu_none component past the
	 * "no_unit" shapes above has no spelling, and emitting "m·k~" would put text
	 * into a document that this library's own parser rejects. */
	for (uint32_t i = 0; i < nc; i++) {
		if (u.components[i].base == bu_none)
			return -1;
	}
	if (nc == 1) {
		return bvn_write_unit_component(
			buf, bufsize, &u.components[0]);
	}
	uint32_t num_num = 0, num_den = 0;
	uint32_t num_idx[BVNR_MAX_UNIT_COMPONENTS];
	uint32_t den_idx[BVNR_MAX_UNIT_COMPONENTS];
	for (uint32_t i = 0; i < nc; i++) {
		if (bvni_is_neg_exp(u.components[i].exponent)) {
			den_idx[num_den++] = i;
		} else {
			num_idx[num_num++] = i;
		}
	}
	int32_t pos = 0;
	if (num_num > 0 && num_den > 0) {
		for (uint32_t n = 0; n < num_num; n++) {
			if (n > 0) {
				if (pos + 2 >= (int32_t)bufsize) return -1;
				buf[pos++] = (char)0xc2;
				buf[pos++] = (char)0xb7;
			}
			int32_t w = bvn_write_unit_component(
				buf + pos, bufsize - (size_t)pos,
				&u.components[num_idx[n]]);
			if (w < 0) return -1;
			pos += w;
		}
		if (pos + 1 >= (int32_t)bufsize) return -1;
		buf[pos++] = '/';
		for (uint32_t d = 0; d < num_den; d++) {
			if (d > 0) {
				if (pos + 2 >= (int32_t)bufsize) return -1;
				buf[pos++] = (char)0xc2;
				buf[pos++] = (char)0xb7;
			}
			value_unit_component_t tmp = u.components[den_idx[d]];
			tmp.exponent = bvn_negate_exponent(tmp.exponent);
			int32_t w = bvn_write_unit_component(
				buf + pos, bufsize - (size_t)pos,
				&tmp);
			if (w < 0) return -1;
			pos += w;
		}
	} else {
		for (uint32_t i = 0; i < nc; i++) {
			if (i > 0) {
				if (pos + 2 >= (int32_t)bufsize) return -1;
				buf[pos++] = (char)0xc2;
				buf[pos++] = (char)0xb7;
			}
			int32_t w = bvn_write_unit_component(
				buf + pos, bufsize - (size_t)pos,
				&u.components[i]);
			if (w < 0) return -1;
			pos += w;
		}
	}
	buf[pos] = '\0';
	return pos;
}
static double bvn_single_component_factor(value_unit_component_t c)
{
	double pf      = bvni_prefix_factor(c);
	int32_t abs_exp = bvni_exp_abs(c.exponent);
	double result  = bvni_ipow(pf, abs_exp);
	if (bvni_is_neg_exp(c.exponent))
		result = 1.0 / result;
	return result;
}
/*
 * The multiplicative scale a unit's prefixes contribute (e.g. "km" -> 1000),
 * folding each component's prefix factor raised to its exponent. Lets a
 * consumer convert a stored value to base-unit magnitude.
 * bvn_unit_prefix_exponent is the base-10 log of the same, for callers that
 * prefer an integer power of ten over a floating factor.
 */
double bvn_unit_prefix_factor(value_unit_t u)
{
	double f = 1.0;
	for (uint32_t i = 0; i < u.num_components && i < BVNR_MAX_UNIT_COMPONENTS; i++) {
		if (bvn_exponent_to_int(u.components[i].exponent) == 0)
			continue;                  /* exp_invalid, or outside the enum */
		f *= bvn_single_component_factor(u.components[i]);
	}
	return f;
}
int32_t bvn_unit_prefix_exponent(value_unit_t u)
{
	int32_t total = 0;
	for (uint32_t i = 0; i < u.num_components && i < BVNR_MAX_UNIT_COMPONENTS; i++) {
		if (bvn_exponent_to_int(u.components[i].exponent) == 0)
			continue;                  /* exp_invalid, or outside the enum */
		total += bvni_prefix_exp_int(u.components[i]);
	}
	return total;
}
/*
 * Parse the body of a type annotation (the text inside `<...>`, e.g.
 * "uint:32,_16,kg") into a value_type_spec_t plus its unit.
 *
 * Grammar: a family keyword, then optional `:`-introduced, `,`-separated
 * parameters in any order — a bare number is the bit width, `_N` is the numeric
 * base (rejected for the decimal float families), `qN` is the fixed-point
 * fraction-bit count, and anything else is the unit string. The four out-flags
 * separate the kinds of failure the caller must distinguish: an unparseable
 * type (type_ok) is fatal, whereas a bad/oversized unit (unit_ok /
 * unit_too_long) may be tolerated for utf8 where a trailing token isn't a unit.
 * Returning these separately is what lets the validator produce precise
 * error_unit_illegal vs error_unit_too_long vs error_illegal_value_type codes.
 */
/*
 * Datetime epoch table (spec 1.1). For a vt_datetime spec the selected epoch is
 * stored as this array index in value_type_spec_t.base; index 0 (unix) is the
 * default. `mjd` is the bvn_epoch_t value — the Modified Julian Day of the
 * epoch's instant — handed to the bvn_dt_* converters.
 */
typedef struct { const char* name; uint8_t len; int32_t mjd; } bvn_epoch_entry_t;
static const bvn_epoch_entry_t bvn_epoch_table[] = {
	{ "unix",    4, bvn_epoch_unix    },
	{ "tai",     3, bvn_epoch_tai     },
	{ "gps",     3, bvn_epoch_gps     },
	{ "mjd",     3, bvn_epoch_mjd     },
	{ "ntp",     3, bvn_epoch_ntp     },
	{ "galileo", 7, bvn_epoch_galileo },
	{ "glonass", 7, bvn_epoch_glonass },
	{ "y2000",   5, bvn_epoch_2000    },
	{ "beidou",  6, bvn_epoch_beidou  },
};
#define BVN_EPOCH_COUNT (sizeof(bvn_epoch_table) / sizeof(bvn_epoch_table[0]))
static int bvn_epoch_index_of(const uint8_t* s, uint32_t n)
{
	for (uint32_t i = 0; i < BVN_EPOCH_COUNT; i++)
		if (bvn_epoch_table[i].len == n &&
		    memcmp(s, bvn_epoch_table[i].name, n) == 0)
			return (int)i;
	return -1;
}
const char* bvnr_datetime_epoch_name(value_type_spec_t vt)
{
	uint32_t i = (vt.family == vt_datetime && vt.base < BVN_EPOCH_COUNT)
		? vt.base : 0u;
	return bvn_epoch_table[i].name;
}
int32_t bvnr_datetime_epoch_mjd(value_type_spec_t vt)
{
	uint32_t i = (vt.family == vt_datetime && vt.base < BVN_EPOCH_COUNT)
		? vt.base : 0u;
	return bvn_epoch_table[i].mjd;
}
int32_t bvnr_datetime_epoch_index(const char* name)
{
	if (!name || !*name)
		return 0;                /* default epoch: unix */
	return bvn_epoch_index_of((const uint8_t*)name, (uint32_t)strlen(name));
}
value_type_spec_t bvn_parse_type_annotation(
	const uint8_t* str, uint32_t len,
	bool* type_ok, bool* unit_ok, bool* unit_too_long,
	value_unit_t* out_unit,
	uint8_t* unit_buf, uint8_t* unit_buf_len)
{
	value_type_spec_t r = BVN_TYPE_PLAIN;
	*type_ok      = true;
	*unit_ok      = true;
	*unit_too_long = false;
	*unit_buf_len = 0;
	*out_unit     = BVN_UNIT_NONE;
	if (!str || !len) { *type_ok = false; return r; }
	uint32_t pos = 0;
	if (pos + 9 <= len && memcmp(str + pos, "float_fix", 9) == 0) {
		r.family = vt_float_fix; pos += 9;
	} else if (pos + 9 <= len && memcmp(str + pos, "float_dec", 9) == 0) {
		r.family = vt_float_dec; pos += 9;
	} else if (pos + 5 <= len && memcmp(str + pos, "float", 5) == 0) {
		r.family = vt_float; pos += 5;
	} else if (pos + 4 <= len && memcmp(str + pos, "uint", 4) == 0) {
		r.family = vt_uint; pos += 4;
	} else if (pos + 4 <= len && memcmp(str + pos, "utf8", 4) == 0) {
		r.family = vt_utf8; pos += 4;
	} else if (pos + 4 <= len && memcmp(str + pos, "bool", 4) == 0) {
		r.family = vt_bool; pos += 4;
	} else if (pos + 4 <= len && memcmp(str + pos, "sint", 4) == 0) {
		r.family = vt_sint; pos += 4;
	} else if (pos + 8 <= len && memcmp(str + pos, "datetime", 8) == 0) {
		/* base carries the epoch index (0 = unix, the default). */
		r.family = vt_datetime; pos += 8;
	} else {
		*type_ok = false;
		return r;
	}
	if (pos < len) {
		if (str[pos] != ':') {
			*type_ok = false;
			return r;
		}
		pos++;
		while (pos < len && str[pos] == ' ') pos++;
		/* A ":" must introduce at least one parameter; "<uint:>" with an
		 * empty list is rejected (param-type = family [":" type-param-list],
		 * the list being mandatory once the colon is present). */
		if (pos >= len) {
			*type_ok = false;
			return r;
		}
	}
	/* Each parameter class (width, base, q-fraction, unit) may appear at most
	 * once; a repeated class is error_illegal_value_type. Without these guards
	 * a later occurrence silently overwrites the earlier one (last-wins), which
	 * makes acceptance order-dependent and can mask a real violation — e.g.
	 * "<uint:8,16> 300" would otherwise be accepted with width 16, hiding that
	 * 300 does not fit uint:8. */
	bool have_width = false, have_base = false, have_q = false, have_unit = false;
	bool need_comma = false;
	while (pos < len) {
		if (need_comma) {
			if (str[pos] != ',') {
				*type_ok = false;
				return r;
			}
			pos++;
			while (pos < len && str[pos] == ' ') pos++;
		}
		need_comma = true;
		/* A comma must be followed by a real parameter. Empty or
		 * trailing components (<uint:8,>, <uint:8,,>, <uint:,_16>) are
		 * rejected to match the strict type-param-list grammar
		 * (type-param-list = type-param , {ws "," ws type-param}); since
		 * parameters are class-identified and order-independent, empty
		 * positional slots are never required. */
		if (pos >= len || str[pos] == ',') {
			*type_ok = false;
			return r;
		}
		uint8_t c = str[pos];
		if (c >= '0' && c <= '9') {
			if (have_width) { *type_ok = false; return r; }
			have_width = true;
			uint64_t w = 0;
			while (pos < len && str[pos] >= '0' && str[pos] <= '9') {
				uint32_t d = (uint32_t)(str[pos] - '0');
				if (w > ((uint64_t)UINT32_MAX - d) / 10u) {
					*type_ok = false;
					return r;
				}
				w = w * 10u + d;
				pos++;
			}
			if (pos < len && str[pos] != ',') {
				*type_ok = false;
				return r;
			}
			r.width = (uint32_t)w;
		} else if (c == '_') {
			if (have_base) { *type_ok = false; return r; }
			/* datetime carries a decimal epoch-seconds value; a numeric base
			 * parameter is meaningless (the only parameter is the epoch name). */
			if (r.family == vt_datetime) { *type_ok = false; return r; }
			have_base = true;
			pos++;
			if (pos >= len || str[pos] < '0' || str[pos] > '9') {
				*type_ok = false;
				return r;
			}
			uint32_t b = 0;
			while (pos < len && str[pos] >= '0' && str[pos] <= '9') {
				b = b * 10u + (uint32_t)(str[pos] - '0');
				if (b > 10000u) {
					*type_ok = false;
					return r;
				}
				pos++;
			}
			if (pos < len && str[pos] != ',') {
				*type_ok = false;
				return r;
			}
			if (r.family == vt_float_fix || r.family == vt_float_dec) {
				*type_ok = false;
				return r;
			}
			if (!((b >= 2u && b <= 62u) || b == 64u || b == 85u)) {
				*type_ok = false;
				return r;
			}
			r.base = b;
		} else if (c == 'q' && pos + 1 < len &&
				   str[pos + 1] >= '0' && str[pos + 1] <= '9') {
			if (r.family != vt_float_fix) {
				*type_ok = false;
				return r;
			}
			if (have_q) { *type_ok = false; return r; }
			have_q = true;
			pos++;
			uint32_t q = 0;
			while (pos < len && str[pos] >= '0' && str[pos] <= '9') {
				q = q * 10u + (uint32_t)(str[pos] - '0');
				if (q > 256u) {
					*type_ok = false;
					return r;
				}
				pos++;
			}
			if (pos < len && str[pos] != ',') {
				*type_ok = false;
				return r;
			}
			r.base = q;
		} else if (r.family == vt_datetime) {
			/* The non-width parameter of a datetime is its epoch name. */
			if (have_unit) { *type_ok = false; return r; }
			have_unit = true;
			uint32_t estart = pos;
			while (pos < len && str[pos] != ',')
				pos++;
			int ei = bvn_epoch_index_of(str + estart, pos - estart);
			if (ei < 0) { *type_ok = false; return r; }
			r.base = (uint32_t)ei;
		} else {
			if (have_unit) { *type_ok = false; return r; }
			have_unit = true;
			uint32_t ustart = pos;
			/*
			 * spec 1.2 — a UCUM annotation may contain a comma ("{cells,total}"),
			 * and a type-parameter list is comma-separated, so the scan tracks
			 * brace depth: a ',' at depth 0 ends the parameter, one at depth >= 1
			 * belongs to it. Bracket depth is tracked too, not because a UCUM
			 * atom can contain a comma inside brackets — none does — but so that
			 * an unbalanced '[' is diagnosed as a malformed unit rather than as a
			 * malformed parameter list.
			 *
			 * The depth bound matches the native parenthesis parser's. UCUM
			 * annotations do not nest, so any depth above 1 is already an error;
			 * the bound is here so a hostile document cannot drive the SCANNER
			 * deep before the parser gets to say so.
			 */
			uint32_t bdepth = 0, kdepth = 0;
			while (pos < len) {
				uint8_t uc = str[pos];
				if (uc == ',' && bdepth == 0 && kdepth == 0)
					break;
				if (uc == '{')      bdepth++;
				else if (uc == '}') { if (bdepth) bdepth--; }
				else if (uc == '[') kdepth++;
				else if (uc == ']') { if (kdepth) kdepth--; }
				if (bdepth > BVN_UNIT_GROUP_MAX_DEPTH ||
					kdepth > BVN_UNIT_GROUP_MAX_DEPTH) {
					*unit_ok = false;
					return r;
				}
				pos++;
			}
			uint32_t ulen = pos - ustart;
			if (ulen > UINT8_MAX) {
				*unit_ok      = false;
				*unit_too_long = true;
				return r;
			}
			memcpy(unit_buf, str + ustart, ulen);
			unit_buf[ulen] = '\0';
			*unit_buf_len  = (uint8_t)ulen;
			if (ulen == 7u && memcmp(unit_buf, "no_unit", 7) == 0) {
				*out_unit = BVN_UNIT_NONE;
			} else {
				bool uok = true;
				*out_unit = bvn_parse_unit_n(unit_buf, ulen, &uok);
				if (!uok)
					*unit_ok = false;
			}
		}
	}
	if (r.family == vt_sint || r.family == vt_uint) {
		if (r.width > BVN_MAX_INT_WIDTH) {
			*type_ok = false;
			return r;
		}
		/* Bases 64 and 85 use '+'/'-' as digit characters, leaving no sign
		 * character available, so they are unsigned-only: signed integers in
		 * those bases are illegal. */
		if (r.family == vt_sint && (r.base == 64u || r.base == 85u)) {
			*type_ok = false;
			return r;
		}
	}
	if (r.family == vt_datetime && r.width > BVN_MAX_INT_WIDTH) {
		*type_ok = false;
		return r;
	}
	if (r.family == vt_float) {
		if (r.base != 0u && r.base != 10u && r.base != 16u) {
			*type_ok = false;
			return r;
		}
		if (r.width != 0u && r.width != 16u &&
			(r.width % 32u != 0u || r.width > BVN_FLOAT_MAX_PREC)) {
			*type_ok = false;
			return r;
		}
	}
	if (r.family == vt_float_fix || r.family == vt_float_dec) {
		if (r.width != 0u  && r.width != 16u  && r.width != 32u &&
			r.width != 64u && r.width != 128u && r.width != 256u) {
			*type_ok = false;
			return r;
		}
	}
	if (r.family == vt_float_fix) {
		uint32_t eff_w = r.width ? r.width : 64u;
		if (r.base >= eff_w) {
			*type_ok = false;
			return r;
		}
	}
	if (r.family == vt_bool || r.family == vt_utf8) {
		/* bool and utf8 are parameterless families: a width, base, q, or
		 * unit parameter is error_illegal_value_type. (q is already
		 * rejected above for any non-float_fix family.) The test is on a
		 * parameter being PRESENT, not on its value: <utf8:0> and <bool:0>
		 * state a width and are refused like <utf8:8>. */
		if (have_width || have_base || r.width != 0u || r.base != 0u ||
		    *unit_buf_len != 0u) {
			*type_ok = false;
			return r;
		}
	}
	return r;
}
/*
 * Identifier well-formedness: must start with a letter, '_' or a UTF-8
 * multibyte lead, and may not contain whitespace, controls, or any of the
 * characters the grammar reserves for punctuation (the explicit reject set).
 * The trailing bvn_validate_string call also guarantees the whole thing is
 * valid UTF-8. bvn_validate_symbol reuses the same rules; bvn_validate_reference
 * applies them to each dot-separated segment of a `.a.b.c` path.
 */
bool bvn_validate_identifier(const char* id)
{
	if (!id || !*id) return false;
	if (!((*id >= 'a' && *id <= 'z') || (*id >= 'A' && *id <= 'Z') ||
		  *id == '_' ||
		  ((uint8_t)*id >= 0xc3 && (uint8_t)*id <= 0xf4)))
		return false;
	for (const char* p = id + 1; *p; p++) {
		unsigned char c = (unsigned char)*p;
		if (c <= 0x20 || c == 0x7f || c == 0xc2) return false;
		if (c == '"' || c == '#' || c == ',' || c == '.' ||
			c == '/' || c == ';' || c == '<' || c == '=' ||
			c == '>' || c == '[' || c == ']' || c == '{' ||
			c == '}' || c == '!' || c == '$' || c == '%' ||
			c == '&' || c == '\''|| c == '(' || c == ')' ||
			c == '*' || c == ':' || c == '?' || c == '@' ||
			c == '\\'|| c == '^' || c == '`' || c == '|' ||
			c == '~')
			return false;
	}
	return bvn_validate_string((const uint8_t*)id, strlen(id));
}
bool bvn_validate_symbol(const char* surr)
{
	if (!surr || !*surr) return false;
	return bvn_validate_identifier(surr);
}
bool bvn_validate_reference(const char* link)
{
	if (!link || !*link) return false;
	const char* p = link;
	if (*p != '.') return false;
	do {
		p++;
		if (!((*p >= 'a' && *p <= 'z') || (*p >= 'A' && *p <= 'Z') ||
			  *p == '_' ||
			  ((uint8_t)*p >= 0xc3 && (uint8_t)*p <= 0xf4)))
			return false;
		p++;
		while (*p && *p != '.') {
			unsigned char c = (unsigned char)*p;
			/* Array index suffix (spec 1.1): "[" digit+ "]" captured into the
			 * path verbatim. A bare "]" (not closing an index) never appears in
			 * a stored path — at the lexer it closes an enclosing array — so it
			 * stays forbidden below. */
			if (c == '[') {
				p++;
				if (*p < '0' || *p > '9') return false;
				while (*p >= '0' && *p <= '9') p++;
				if (*p != ']') return false;
				p++;
				/* After an index the path continues with a member, another
				 * index, or ends. The reader refuses anything else
				 * (`&.a[0]b` is error_unexpected_input_byte), and this
				 * accepted it, so the writer could emit a reference its
				 * own reader would not take. */
				if (*p && *p != '.' && *p != '[') return false;
				continue;
			}
			if (c <= 0x20 || c == 0x7f || c == 0xc2) return false;
			if (c == '"' || c == '#' || c == ',' || c == '/' ||
				c == ';' || c == '<' || c == '=' || c == '>' ||
				c == ']' || c == '{' || c == '}' ||
				c == '!' || c == '$' || c == '%' || c == '&' ||
				c == '\''|| c == '(' || c == ')' || c == '*' ||
				c == ':' || c == '?' || c == '@' || c == '\\' ||
				c == '^' || c == '`' || c == '|' || c == '~')
				return false;
			p++;
		}
	} while (*p == '.');
	/* The per-byte reject set above screens punctuation, not encoding: every
	 * byte >= 0x80 other than 0xc2 walks straight through it. Without this the
	 * writer accepted a lone continuation byte, a UTF-16 surrogate, or an
	 * overlong sequence in a path and emitted a document its own reader then
	 * refused with error_invalid_utf8_byte. The overlong form is the worse one
	 * in principle: "\xc0\xae" is an overlong '.', so a lenient third-party
	 * decoder would read it as the path separator the reject set is there to
	 * forbid. bvn_validate_identifier ends with the same call. */
	return bvn_validate_string((const uint8_t*)link, strlen(link));
}
bool bvn_validate_number(const char* s)
{
	if (!s || !*s) return false;
	const char* p = s;
	if (*p == '-') p++;
	if (!*p) return false;
	bool has_dot = false, has_exp = false;
	bool has_mant_dig = false, has_exp_dig = false;
	for (; *p; p++) {
		if (*p >= '0' && *p <= '9') {
			if (has_exp) has_exp_dig = true;
			else         has_mant_dig = true;
			continue;
		}
		if (*p == '.') { if (has_dot || has_exp) return false; has_dot = true; continue; }
		if (*p == 'e' || *p == 'E') {
			if (has_exp || !has_mant_dig) return false;
			has_exp = true;
			if (p[1] == '+' || p[1] == '-') p++;
			continue;
		}
		return false;
	}
	return has_mant_dig && (!has_exp || has_exp_dig);
}
/*
 * Format an unsigned integer in any supported base, left-padded with leading
 * zeros to min_digits. Digits are produced least-significant-first into a
 * stack scratch buffer and then copied out in order — the standard trick that
 * avoids needing to know the length up front. Returns -1 if the base is
 * unsupported or the output buffer is too small (never truncates silently).
 * bvn_format_int64 prepends '-' and formats the magnitude, taking care with
 * INT64_MIN whose magnitude doesn't fit in int64.
 */
int32_t bvn_format_uint64(char* buf, size_t bufsize, uint64_t value,
						  uint32_t base, uint32_t min_digits)
{
	bool valid_base = (base >= 2u && base <= 62u) ||
					  base == 64u || base == 85u;
	if (!valid_base || !buf || bufsize < 2) return -1;
	if (min_digits > 64u) min_digits = 64u;
	char     tmp[65];
	int      pos      = 64;
	uint32_t zero_ch  = bvn_digit_to_char(0u, base);
	tmp[pos] = '\0';
	if (value == 0) {
		tmp[--pos] = (char)zero_ch;
	} else {
		while (value) {
			uint32_t d  = (uint32_t)(value % base);
			uint32_t ch = bvn_digit_to_char(d, base);
			if (!ch) return -1;
			tmp[--pos] = (char)ch;
			value /= base;
		}
	}
	while ((64 - pos) < (int)min_digits) tmp[--pos] = (char)zero_ch;
	int len = 64 - pos;
	if ((size_t)(len + 1) > bufsize) return -1;
	memcpy(buf, tmp + pos, (size_t)len + 1);
	return len;
}
int32_t bvn_format_int64(char* buf, size_t bufsize, int64_t value,
						 uint32_t base, uint32_t min_digits)
{
	bool neg = (value < 0);
	/* Bases 64 and 85 use '+'/'-' as digits and so have no sign character; a
	 * negative value cannot be represented (callers gate this via uint-only
	 * type validation, but guard here too rather than emit a misreadable '-'). */
	if (neg && (base == 64u || base == 85u)) return -1;
	if (neg) {
		if (bufsize < 2) return -1;
		buf[0] = '-';
		uint64_t abs_val = (uint64_t)0 - (uint64_t)value;
		int32_t r = bvn_format_uint64(buf + 1, bufsize - 1,
									  abs_val, base, min_digits);
		return r < 0 ? r : r + 1;
	}
	return bvn_format_uint64(buf, bufsize, (uint64_t)value, base, min_digits);
}
uint32_t bvn_min_digits_for_type(value_type_spec_t vt)
{
	if (!bvn_type_is_numeric(vt) || !vt.width) return 0;
	uint32_t w = vt.width;
	uint32_t base = bvn_effective_base(vt);
	uint32_t digits = 1;
	if (w > 64u) return 0u;
	uint64_t maxv = (vt.family == vt_sint)
		? ((uint64_t)1u << (w - 1u)) - 1u
		: (w >= 64u) ? UINT64_MAX : ((uint64_t)1u << w) - 1u;
	uint64_t v = maxv;
	while (v >= base) { v /= base; digits++; }
	return digits;
}
/*
 * Parse a signed integer in the type's effective base into int64. Bases 2-36
 * defer to the libc strtoll (fast, locale-free here); larger bases (up to 62,
 * plus 64/85) are parsed by hand via bvn_char_to_digit because strtoll doesn't
 * know those alphabets. Overflow is checked before it happens, and INT64_MIN is
 * handled specially since its magnitude is one past INT64_MAX. The uint64 and
 * double parsers follow the same base-split structure.
 */
/* The declared width bounds the value, which is the whole reason these helpers
 * take a value_type_spec_t: doc/08 states "the vt argument supplies the base and
 * bit-width for range checking" and "returns false if the string is not
 * representable in the declared type". Only the base was ever consulted, so
 * bvn_parse_uint64("256", <uint:8>, &u) returned true with 256 and, worse,
 * "-1" returned true with 18446744073709551615 -- strtoull wraps a sign the
 * format does not allow on an unsigned carrier. A width above 64 needs no check:
 * the out parameter is the binding limit there. */
static bool bvn_fits_uint_width(uint64_t v, uint32_t width)
{
	if (width == 0u || width >= 64u) return true;
	return v <= (~(uint64_t)0 >> (64u - width));
}

static bool bvn_fits_sint_width(int64_t v, uint32_t width)
{
	if (width == 0u || width >= 64u) return true;
	int64_t lim = (int64_t)1 << (width - 1u);
	return v >= -lim && v <= lim - 1;
}

/* strtoll/strtoull accept leading whitespace and an empty string (yielding 0),
 * neither of which is a value token this format can produce. Screen them so the
 * helpers reject what the reader would. */
static bool bvn_parse_prefix_ok(const char* s, bool allow_sign)
{
	if (!s || !*s) return false;
	if (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r' ||
	    *s == '\f' || *s == '\v')
		return false;
	if (*s == '+') return false;              /* the format never writes one */
	if (*s == '-') return allow_sign && s[1] != '\0';
	return true;
}

bool bvn_parse_int64(const char* s, value_type_spec_t vt, int64_t* out)
{
	if (!s || !out) return false;
	if (!bvn_parse_prefix_ok(s, true)) return false;
	uint32_t base = bvn_effective_base(vt);
	if (base >= 2u && base <= 36u) {
		char* end;
		errno = 0;
		long long v = strtoll(s, &end, (int)base);
		if (*end || errno == ERANGE) return false;
		if (!bvn_fits_sint_width((int64_t)v, bvn_effective_width(vt)))
			return false;
		*out = (int64_t)v;
		return true;
	}
	bool neg = (s[0] == '-');
	const char* p = s + (neg ? 1u : 0u);
	if (!*p) return false;
	uint64_t acc = 0;
	while (*p) {
		uint32_t d = bvn_char_to_digit((uint8_t)*p, base);
		if (d >= base) return false;
		if (acc > (UINT64_MAX - d) / base) return false;
		acc = acc * base + d;
		p++;
	}
	if (neg) {
		if (acc > (uint64_t)INT64_MAX + 1u) return false;
		if (!bvn_fits_sint_width(
			(acc == (uint64_t)INT64_MAX + 1u) ? INT64_MIN : -(int64_t)acc,
			bvn_effective_width(vt)))
			return false;
		*out = (acc == (uint64_t)INT64_MAX + 1u)
		       ? INT64_MIN
		       : -(int64_t)acc;
	} else {
		if (acc > (uint64_t)INT64_MAX) return false;
		if (!bvn_fits_sint_width((int64_t)acc, bvn_effective_width(vt)))
			return false;
		*out = (int64_t)acc;
	}
	return true;
}
bool bvn_parse_uint64(const char* s, value_type_spec_t vt, uint64_t* out)
{
	if (!s || !out) return false;
	uint32_t base = bvn_effective_base(vt);
	/* No sign at all: an unsigned carrier has none, and strtoull silently wraps
	 * a leading '-' into a huge positive value rather than failing.
	 *
	 * That is a rule about bases up to 36, where '-' and '+' can only be a
	 * sign. In Base64 '+' is the digit 62, and in Ascii85 '+' and '-' are the
	 * digits 10 and 12 -- so `<uint:64,_85> "-."` is the number 1033, which the
	 * streaming reader delivered and this refused, and with it every document
	 * tier built on this function (bovnar validate, the DOM, query). There the
	 * digit loop below is the whole test: a byte that is not a digit of the
	 * base fails it. */
	if (base <= 36u) {
		if (!bvn_parse_prefix_ok(s, false)) return false;
	} else if (!*s) {
		return false;
	}
	if (base >= 2u && base <= 36u) {
		char* end;
		errno = 0;
		unsigned long long v = strtoull(s, &end, (int)base);
		if (*end || errno == ERANGE) return false;
		if (!bvn_fits_uint_width((uint64_t)v, bvn_effective_width(vt)))
			return false;
		*out = (uint64_t)v;
		return true;
	}
	const char* p = s;
	if (!*p) return false;
	uint64_t acc = 0;
	while (*p) {
		uint32_t d = bvn_char_to_digit((uint8_t)*p, base);
		if (d >= base) return false;
		if (acc > (UINT64_MAX - d) / base) return false;
		acc = acc * base + d;
		p++;
	}
	if (!bvn_fits_uint_width(acc, bvn_effective_width(vt)))
		return false;
	*out = acc;
	return true;
}
/*
 * Parse a floating literal in an arbitrary base into a double.
 *
 * Base 10 and base 16 both go through bvn_float_strtoieee_bin, which forms the
 * literal as an exact rational num/den and rounds it ONCE to binary64 (correct
 * rounding across the whole range: normals, subnormals, overflow to inf,
 * underflow to 0, at any input length). It does NOT build an intermediate
 * fixed-width bvn_float and then narrow -- that sequence rounds twice and is off
 * by 1 ULP for inputs near a rounding midpoint, and no fixed intermediate width
 * removes it (the previous base-10 route landed 2.2250738585072011e-308 on the
 * smallest normal instead of the largest subnormal; the previous base-16 route
 * mis-rounded hex-floats with more than 64 significant bits the same way).
 * bvn_float_strtoieee_bin understands both decimal mantissas and hex-float
 * p-exponents, so a single exact path now serves both bases.
 *
 * bvn_format_double is the inverse, going double -> bvn_float -> text so the
 * rendered precision matches the declared width.
 */
/* What a floating value token can look like, and nothing else. The rounding
 * engine below returns no verdict -- text it cannot read comes back as zero --
 * so without this every string was a number: "", "abc" and "1x" parsed as 0.0
 * and reported success, where the integer helpers beside this one refuse. */
static bool bvn_float_text_ok(const char* s, uint32_t base)
{
	if (strcmp(s, "inf") == 0 || strcmp(s, "ninf") == 0 ||
	    strcmp(s, "nan") == 0)
		return true;
	/* Either sign. The reader takes a QUOTED float literal with a leading '+'
	 * (bvn_validate_number_in_base does, in every base), so `<float:64> "+5"`
	 * reaches the DOM builder through here and has to keep parsing as 5. */
	if (*s == '-' || *s == '+') s++;
	if (strcmp(s, "inf") == 0 || strcmp(s, "nan") == 0)
		return true;
	bool hex = (base == 16u);
	if (hex && s[0] == '0' && (s[1] == 'x' || s[1] == 'X')) s += 2;
	size_t digits = 0;
	for (; *s; s++) {
		if (hex ? isxdigit((unsigned char)*s) : (*s >= '0' && *s <= '9'))
			digits++;
		else
			break;
	}
	if (*s == '.') {
		for (s++; *s; s++) {
			if (hex ? isxdigit((unsigned char)*s)
			        : (*s >= '0' && *s <= '9'))
				digits++;
			else
				break;
		}
	}
	if (digits == 0) return false;
	if (hex ? (*s == 'p' || *s == 'P') : (*s == 'e' || *s == 'E')) {
		s++;
		if (*s == '+' || *s == '-') s++;
		if (!(*s >= '0' && *s <= '9')) return false;
		while (*s >= '0' && *s <= '9') s++;
	}
	return *s == '\0';
}
bool bvn_parse_double_in_base(const char* s, uint32_t base, double* out)
{
	if (!s || !out) return false;
	if (base == 10u || base == 16u) {
		if (!bvn_float_text_ok(s, base)) return false;
		uint32_t b[2] = { 0u, 0u };
		bvn_float_strtoieee_bin(s, base, 11u, 52u, 1023, b, 2);
		uint64_t u = (uint64_t)b[0] | ((uint64_t)b[1] << 32);
		memcpy(out, &u, sizeof *out);
		return true;
	}
	/* No other base denotes a floating literal in this format. */
	return false;
}
bool bvn_looks_like_double(const char* s)
{
	if (!s) return false;
	for (; *s; s++)
		if (*s == '.' || *s == 'e' || *s == 'E' || *s == 'p' || *s == 'P')
			return true;
	return false;
}
int32_t bvn_format_double(char* buf, size_t bufsize, double value,
						  value_type_spec_t vt)
{
	if (!buf || bufsize < 2) return -1;
	bvn_limb_t _dlimbs[BVN_FLOAT_NLIMBS(64u)];
	bvn_float_t f;
	uint32_t prec = bvn_effective_width(vt);
	if (!prec || prec > 64u) prec = 64u;
	/* `value` is a C double, which carries at most 53 significand bits. Loading
	 * it into a wider float pads the mantissa with zero bits the value never
	 * actually had; the shortest-round-trip search in bvn_float_to_str then has
	 * to pin down that wider float and emits ~20 noise digits
	 * (0.1 -> 1.0000000000000000555e-1). Capping the format precision at the
	 * double's true 53 bits yields the canonical shortest decimal (0.1) that
	 * still round-trips the double exactly, while a narrower declared width
	 * (float:16/float:32) is preserved so its rounding intent is honoured. */
	if (prec > 53u) prec = 53u;
	/* For float_fix and float_dec the width is the FIELD (a Q-format integer,
	 * a decimal interchange format), not a binary significand. Rounding the
	 * double to `width` bits first and leaving the reader to round again to
	 * the field's grid was a double rounding: 0.5019531251 under q8 went out
	 * as 0.50195 and was read as 128/256 instead of 129/256. Write the
	 * double's own value, and let the one rounding happen in the reader. */
	if (vt.family == vt_float_fix || vt.family == vt_float_dec) prec = 53u;
	bvn_float_init_buf(&f, prec, _dlimbs, BVN_FLOAT_NLIMBS(64u));
	if (!bvn_float_from_double(&f, value)) return -1;
	uint32_t base = bvn_effective_base(vt);
	if (!base) base = 10u;
	if (base > 36u) return -1;
	return bvn_float_to_str(&f, buf, bufsize, base);
}
bool bvn_parse_double(const char* s, value_type_spec_t vt, double* out)
{
	return bvn_parse_double_in_base(s, bvn_effective_base(vt), out);
}
/*
 * Expose the escape-replacement table (escape letter -> literal byte) so other
 * components decode string escapes identically to the lexer. A non-zero entry
 * is a recognised escape; zero means "not a valid escape character".
 */
const uint8_t* bvn_get_escape_repl_table(void)
{
	static const uint8_t table[256] = {
		['t']  = '\t',  ['n']  = '\n', ['v']  = '\v',
		['f']  = '\f',  ['r']  = '\r', ['"']  = '"',
		['\\'] = '\\',
	};
	return table;
}

