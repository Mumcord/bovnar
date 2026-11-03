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

#include <bvn_float.h>
#include <bvn_int.h>
#include <inttypes.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <float.h>
#include <limits.h>
/*
 * ===========================================================================
 * Arbitrary-precision binary floating point
 * ===========================================================================
 *
 * bovnar supports floats far wider than IEEE double (up to BVN_FLOAT_MAX_PREC
 * bits of mantissa). A bvn_float_t holds a value as sign · mantissa · 2^exp:
 * _sign, a binary exponent _exp, and a mantissa in the limb array _d of _prec
 * significant bits (normalised so the leading bit is set). Non-finite and zero
 * values are encoded by sentinel exponents (BVN_FLOAT_EXP_NAN/INF/ZERO) rather
 * than mantissa patterns, which keeps the arithmetic paths from special-casing
 * the mantissa.
 *
 * The defining requirement is *correct rounding*: parsing a decimal string and
 * formatting back must round to nearest-even at the target precision, with no
 * accumulated error. That is impossible with hardware floats for arbitrary
 * precision, so decimal<->binary conversion is done exactly using big integers:
 * a decimal literal d×10^e is represented as an exact rational num/den, and
 * bvnf_rational_to_float performs one big-integer division producing the
 * mantissa plus the guard/round/sticky bits needed for round-to-nearest-even.
 * The local bvnf_bn type and the BVN_INT_LOCAL macros (see bvn_float_impl.h)
 * provide stack-allocated big integers so the common precisions need no heap.
 */
typedef struct {
	uint32_t *limbs;
	uint32_t  nlimbs;
	uint32_t  nused;
} bvnf_bn;
static bool bvnf_bn_grow(bvnf_bn *b, uint32_t need)
{
	if (need <= b->nlimbs) return true;
	uint32_t *p = realloc(b->limbs, (size_t)need * sizeof(uint32_t));
	if (!p) return false;
	memset(p + b->nlimbs, 0, (size_t)(need - b->nlimbs) * sizeof(uint32_t));
	b->limbs  = p;
	b->nlimbs = need;
	return true;
}
static bool bvnf_bn_init(bvnf_bn *b, uint32_t cap)
{
	if (!cap) cap = 8;
	b->limbs = calloc(cap, sizeof(uint32_t));
	if (!b->limbs) return false;
	b->nlimbs = cap;
	b->nused  = 0;
	return true;
}
static void bvnf_bn_free(bvnf_bn *b) { free(b->limbs); b->limbs = NULL; }
static void bvnf_bn_norm(bvnf_bn *b)
{
	uint32_t n       = b->nused <= b->nlimbs ? b->nused : b->nlimbs;
	uint32_t *lo     = b->limbs;
	uint32_t *hi     = lo + n;
	while (hi > lo && hi[-1] == 0) hi--;
	b->nused = (uint32_t)(hi - lo);
}
static bool bvnf_bn_is_zero(const bvnf_bn *b) { return b->nused == 0; }
static bool bvnf_bn_from_float_mant(bvnf_bn *b, const bvn_float_t *f)
{
	uint32_t nl = f->_nlimbs;
#if BVN_LIMB_BITS == 64
	if (nl > UINT32_MAX / 2u) return false;
	uint32_t w32 = nl * 2u;
	if (!bvnf_bn_grow(b, w32)) return false;
	for (uint32_t i = 0; i < nl; i++) {
		b->limbs[i * 2]     = (uint32_t)(f->_d[i] & 0xffffffffu);
		b->limbs[i * 2 + 1] = (uint32_t)(f->_d[i] >> 32);
	}
	b->nused = w32;
#else
	if (!bvnf_bn_grow(b, nl)) return false;
	for (uint32_t i = 0; i < nl; i++) b->limbs[i] = f->_d[i];
	b->nused = nl;
#endif
	bvnf_bn_norm(b);
	return true;
}
static bool bvnf_bn_mul_u32(bvnf_bn *b, uint32_t v)
{
	if (!v)         { b->nused = 0; return true; }
	if (v == 1)     return true;
	if (b->nused == 0) return true;
	if (!bvnf_bn_grow(b, b->nused + 1)) return false;
	b->limbs[b->nused] = 0;
	uint64_t carry = 0;
	for (uint32_t i = 0; i < b->nused; i++) {
		uint64_t r = (uint64_t)b->limbs[i] * v + carry;
		b->limbs[i] = (uint32_t)(r & 0xffffffffu);
		carry = r >> 32;
	}
	if (carry) { b->limbs[b->nused] = (uint32_t)carry; b->nused++; }
	return true;
}
static void bvnf_bn_shr(bvnf_bn *b, uint32_t bits)
{
	if (bvnf_bn_is_zero(b) || !bits) return;
	uint32_t wshift = bits / 32u, bshift = bits % 32u;
	if (wshift >= b->nused) { b->nused = 0; return; }
	if (bshift == 0) {
		memmove(b->limbs, b->limbs + wshift,
				(size_t)(b->nused - wshift) * sizeof(uint32_t));
		b->nused -= wshift;
	} else {
		uint32_t rbshift = 32u - bshift;
		for (uint32_t i = 0; i + wshift < b->nused; i++) {
			b->limbs[i] = b->limbs[i + wshift] >> bshift;
			if (i + wshift + 1 < b->nused)
				b->limbs[i] |= b->limbs[i + wshift + 1] << rbshift;
		}
		b->nused -= wshift;
	}
	bvnf_bn_norm(b);
}
static bool bvnf_bn_shl(bvnf_bn *b, uint32_t bits)
{
	if (bvnf_bn_is_zero(b) || !bits) return true;
	uint32_t wshift = bits / 32u, bshift = bits % 32u;
	uint32_t need = b->nused + wshift + (bshift ? 1u : 0u);
	if (!bvnf_bn_grow(b, need)) return false;
	for (uint32_t i = b->nused; i-- > 0;) {
		uint32_t v = b->limbs[i];
		b->limbs[i + wshift]     = bshift ? (v << bshift) : v;
		if (bshift && i + wshift + 1 < need)
			b->limbs[i + wshift + 1] |= v >> (32u - bshift);
		if (wshift) b->limbs[i] = 0;
	}
	b->nused = need;
	bvnf_bn_norm(b);
	return true;
}
static bool bvnf_bn_mul_pow5(bvnf_bn *b, uint32_t k)
{
	static const uint32_t p5_13 = 1220703125u;
	while (k >= 13) {
		if (!bvnf_bn_mul_u32(b, p5_13)) return false;
		k -= 13;
	}
	uint32_t p = 1;
	for (uint32_t i = 0; i < k; i++) p *= 5;
	return bvnf_bn_mul_u32(b, p);
}
static uint32_t bvnf_bn_div_u32(bvnf_bn *b, uint32_t d)
{
	uint64_t rem = 0;
	for (uint32_t i = b->nused; i-- > 0;) {
		uint64_t cur = (rem << 32) | b->limbs[i];
		b->limbs[i] = (uint32_t)(cur / d);
		rem         = cur % d;
	}
	bvnf_bn_norm(b);
	return (uint32_t)rem;
}
/*
 * Growable heap-backed scratch big integer. This replaces the former fixed
 * uint32_t[BVN_INT_MAX_BITS/32] stack arrays, each of which cost ~4 KiB of stack
 * regardless of the working precision — hostile to the small task stacks common
 * on embedded targets (a single bvn_float_from_str frame was ~16 KiB). Starting
 * small and letting the bvn_int_* routines grow on demand (capped internally at
 * BVN_INT_MAX_LIMBS) keeps every frame flat and commits memory proportional to
 * the value actually computed. init_limbs is only a sizing hint to avoid early
 * reallocations; correctness does not depend on it.
 */
static bool bvnf_scratch(bvn_int_t *n, uint32_t init_limbs)
{
	if (init_limbs < 8u)               init_limbs = 8u;
	if (init_limbs > BVN_INT_MAX_LIMBS) init_limbs = BVN_INT_MAX_LIMBS;
	uint32_t *p = calloc(init_limbs, sizeof(uint32_t));
	if (!p) return false;
	n->limbs       = p;
	n->nlimbs      = init_limbs;
	n->nused       = 0;
	n->negative    = false;
	n->heap        = true;
	n->_reserved[0] = 0;
	n->_reserved[1] = 0;
	return true;
}
static void bvnf_scratch_free(bvn_int_t *n) { free(n->limbs); n->limbs = NULL; }
/* Limb hint from a bit precision: ceil(prec/32) plus headroom. */
static uint32_t bvnf_hint_limbs(uint32_t prec)
{
	uint32_t l = prec / 32u + 8u;
	return (l > BVN_INT_MAX_LIMBS) ? BVN_INT_MAX_LIMBS : l;
}
bvn_float_t *bvn_float_alloc(uint32_t prec)
{
	if (!prec || prec > BVN_FLOAT_MAX_PREC) return NULL;
	bvn_float_t *f = malloc(sizeof *f);
	if (!f) return NULL;
	uint32_t nl = BVN_FLOAT_NLIMBS(prec);
	bvn_limb_t *d = calloc(nl, sizeof(bvn_limb_t));
	if (!d) { free(f); return NULL; }
	f->_prec   = prec;
	f->_sign   = 1;
	f->_exp    = BVN_FLOAT_EXP_ZERO;
	f->_d      = d;
	f->_nlimbs = nl;
	f->_heap   = true;
	return f;
}
void bvn_float_free(bvn_float_t *f)
{
	if (!f) return;
	if (f->_heap) free(f->_d);
	free(f);
}
void bvn_float_init_buf(bvn_float_t *f, uint32_t prec,
						bvn_limb_t *buf, uint32_t nlimbs)
{
	f->_prec   = prec;
	f->_sign   = 1;
	f->_exp    = BVN_FLOAT_EXP_ZERO;
	f->_d      = buf;
	f->_nlimbs = nlimbs;
	f->_heap   = false;
	memset(buf, 0, (size_t)nlimbs * sizeof(bvn_limb_t));
}
/*
 * Predicates treat a NULL handle as "none of the above": a NULL float is not a
 * NaN, an infinity, a zero, negative, or regular. Without these guards a NULL
 * dereferences immediately, which is inconsistent with every from_, to_dec and
 * to_fix entry point (all of which already null-check) and turns a caller bug
 * into a crash instead of a benign false.
 */
bool bvn_float_is_nan(const bvn_float_t *f)
	{ return f && f->_exp == BVN_FLOAT_EXP_NAN; }
bool bvn_float_is_inf(const bvn_float_t *f)
	{ return f && f->_exp == BVN_FLOAT_EXP_INF; }
bool bvn_float_is_zero(const bvn_float_t *f)
	{ return f && f->_exp == BVN_FLOAT_EXP_ZERO; }
bool bvn_float_is_neg(const bvn_float_t *f)
	{ return f && f->_sign < 0; }
bool bvn_float_is_regular(const bvn_float_t *f)
{
	return f
		&& f->_exp != BVN_FLOAT_EXP_NAN
		&& f->_exp != BVN_FLOAT_EXP_INF
		&& f->_exp != BVN_FLOAT_EXP_ZERO;
}
void bvn_float_set_nan(bvn_float_t *f)
{
	f->_exp  = BVN_FLOAT_EXP_NAN;
	f->_sign = 1;
}
void bvn_float_set_inf(bvn_float_t *f, bool neg)
{
	f->_exp  = BVN_FLOAT_EXP_INF;
	f->_sign = neg ? -1 : 1;
}
void bvn_float_set_zero(bvn_float_t *f, bool neg)
{
	f->_exp  = BVN_FLOAT_EXP_ZERO;
	f->_sign = neg ? -1 : 1;
	memset(f->_d, 0, (size_t)f->_nlimbs * sizeof(bvn_limb_t));
}
/* Defined below, after the bvn_int repack helpers; needed by bvn_float_copy. */
static void bvnf_store_mant(bvn_float_t *f, const bvn_int_t *Q);
/*
 * Copy src into dst, preserving the represented value (sign * M * 2^(_exp-_prec),
 * with the leading mantissa bit at _prec-1). When the precisions match this is a
 * straight limb copy. When they differ the mantissa must be repositioned so its
 * leading bit lands at dst's _prec-1: widening shifts left (exact), narrowing
 * shifts right with round-to-nearest-even (a carry out of the mantissa bumps the
 * exponent). Non-finite and zero values copy sign+exponent only.
 */
bool bvn_float_copy(bvn_float_t *dst, const bvn_float_t *src)
{
	if (!dst || !src) return false;
	dst->_sign = src->_sign;
	dst->_exp  = src->_exp;
	if (!bvn_float_is_regular(src)) {
		memset(dst->_d, 0, (size_t)dst->_nlimbs * sizeof(bvn_limb_t));
		return true;
	}
	if (dst->_prec == src->_prec) {
		uint32_t nl = (dst->_nlimbs < src->_nlimbs) ? dst->_nlimbs : src->_nlimbs;
		memset(dst->_d, 0, (size_t)dst->_nlimbs * sizeof(bvn_limb_t));
		memcpy(dst->_d, src->_d, (size_t)nl * sizeof(bvn_limb_t));
		return true;
	}
	/*
	 * Differing precision: load the mantissa as a 32-bit big integer, reposition
	 * the leading bit, and round to nearest-even when low bits are dropped.
	 */
	int shift = (int)dst->_prec - (int)src->_prec;
	uint32_t hint = bvnf_hint_limbs(dst->_prec > src->_prec ? dst->_prec : src->_prec);
	bvn_int_t M;
	if (!bvnf_scratch(&M, hint + 2u)) return false;
#if BVN_LIMB_BITS == 64
	for (uint32_t i = 0; i < src->_nlimbs; i++) {
		if (2u * i      < M.nlimbs) M.limbs[2u * i]      = (uint32_t)(src->_d[i] & 0xffffffffu);
		if (2u * i + 1u < M.nlimbs) M.limbs[2u * i + 1u] = (uint32_t)(src->_d[i] >> 32);
	}
	M.nused = (src->_nlimbs * 2u <= M.nlimbs) ? src->_nlimbs * 2u : M.nlimbs;
#else
	for (uint32_t i = 0; i < src->_nlimbs && i < M.nlimbs; i++) M.limbs[i] = src->_d[i];
	M.nused = (src->_nlimbs <= M.nlimbs) ? src->_nlimbs : M.nlimbs;
#endif
	bvn_int_norm(&M);
	if (shift > 0) {
		if (!bvn_int_shl(&M, shift)) { bvnf_scratch_free(&M); return false; }
	} else {
		int  d      = -shift;
		int  guard  = bvn_int_getbit(&M, d - 1);
		bool sticky = false;
		for (int i = 0; i < d - 1 && !sticky; i++)
			if (bvn_int_getbit(&M, i)) sticky = true;
		bvn_int_shr(&M, d);
		if (guard && (sticky || bvn_int_getbit(&M, 0) == 1)) {
			if (!bvn_int_add_u32(&M, 1u)) { bvnf_scratch_free(&M); return false; }
			if (bvn_int_bitlen(&M) > (int)dst->_prec) {
				bvn_int_shr(&M, 1);
				dst->_exp += 1;
			}
		}
	}
	bvnf_store_mant(dst, &M);
	bvnf_scratch_free(&M);
	return true;
}
/*
 * Copy a big-integer mantissa Q into the float's limb array, repacking from the
 * 32-bit bvn_int limbs to the platform's bvn_limb_t (which may be 64-bit). Any
 * unused high limbs are zeroed. Kept separate so the rounding code can work in
 * the uniform 32-bit bvn_int domain and only convert to the storage limb width
 * once, at the end.
 */
static void bvnf_store_mant(bvn_float_t *f, const bvn_int_t *Q)
{
	uint32_t nl = f->_nlimbs;
	memset(f->_d, 0, (size_t)nl * sizeof(bvn_limb_t));
#if BVN_LIMB_BITS == 64
	for (uint32_t i = 0; i < nl; i++) {
		uint32_t lo_idx = i * 2;
		uint32_t hi_idx = i * 2 + 1;
		uint64_t lo = (lo_idx < Q->nused) ? Q->limbs[lo_idx] : 0u;
		uint64_t hi = (hi_idx < Q->nused) ? Q->limbs[hi_idx] : 0u;
		f->_d[i] = lo | (hi << 32);
	}
#else
	for (uint32_t i = 0; i < nl; i++)
		f->_d[i] = (i < Q->nused) ? Q->limbs[i] : 0u;
#endif
}
/*
 * Long division of (num << shift) / den, one bit at a time, emitting only the
 * top `want` quotient bits and folding everything below into a single sticky
 * bit. This is the fallback used when the scaled numerator would exceed the
 * fixed big-int bit budget (BVN_INT_MAX_BITS): instead of materialising the
 * huge shifted value, it shifts a running remainder R left one bit per
 * iteration and pulls in the next numerator bit — classic restoring division —
 * so memory stays bounded by the divisor size. The sticky bit (any discarded
 * low bit or nonzero final remainder) is exactly what round-to-nearest-even
 * needs below.
 */
static bool bvnf_bitdiv(const bvn_int_t *num, int shift,
						 const bvn_int_t *den,
						 uint32_t *Q, uint32_t q_words,
						 int want, bool *sticky_out)
{
	int db = bvn_int_bitlen(den);
	int nb = bvn_int_bitlen(num);
	int total = nb + (shift > 0 ? shift : 0);
	if (total <= 0) { *sticky_out = false; return true; }
	uint32_t r_cap = (uint32_t)(db / 32) + 4u;
	bvn_int_t R, H;
	if (!bvnf_scratch(&R, r_cap)) return false;
	/* H = den >> 1 and dlow = den & 1. The obvious loop body -- shift R left,
	 * pull in the next bit, then compare against den -- transiently needs
	 * bitlen(den)+1 bits, which for a denominator of exactly BVN_INT_MAX_BITS
	 * (10^9864 has bitlen 32768) the shift cannot produce: it failed and aborted
	 * the whole division. Deciding against H instead never materialises that
	 * extra bit:
	 *
	 *     2R + b >= den  <=>  R > H, or R == H and b >= dlow
	 *
	 * and the updated remainder is rewritten so the value being doubled always
	 * stays below den:
	 *
	 *     2R + b - den  ==  2(R - H) + (b - dlow)
	 *
	 * which for b < dlow (only b=0, dlow=1) is taken as 2(R - H - 1) + 1 so the
	 * intermediate never goes negative. Both branches therefore double a value
	 * whose result is < den, i.e. at most bitlen(den) bits. */
	if (!bvnf_scratch(&H, r_cap)) { bvnf_scratch_free(&R); return false; }
	if (!bvn_int_copy(&H, den)) {
		bvnf_scratch_free(&R); bvnf_scratch_free(&H); return false;
	}
	bvn_int_shr(&H, 1);
	const int dlow = bvn_int_getbit(den, 0);
	uint32_t one_limb[1] = { 1u };
	bvn_int_t ONE = { one_limb, 1u, 1u, false, false, { 0, 0 } };
	memset(Q, 0, (size_t)q_words * sizeof(uint32_t));
	bool sticky = false;
	bool fail = false;
	for (int i = total - 1; i >= 0; i--) {
		int src = i - shift;
		int b = (src >= 0 && bvn_int_getbit(num, src)) ? 1 : 0;
		int c = bvn_int_cmp(&R, &H);
		if (c > 0 || (c == 0 && b >= dlow)) {
			if (!bvn_int_sub_inplace(&R, &H)) { fail = true; break; }
			if (b < dlow && !bvn_int_sub_inplace(&R, &ONE)) { fail = true; break; }
			if (!bvn_int_shl(&R, 1)) { fail = true; break; }
			if (b != dlow && !bvn_int_add_u32(&R, 1u)) { fail = true; break; }
			if (i < want) {
				uint32_t wi = (uint32_t)i / 32u;
				if (wi < q_words) Q[wi] |= 1u << ((uint32_t)i % 32u);
			} else {
				sticky = true;
			}
		} else {
			if (!bvn_int_shl(&R, 1)) { fail = true; break; }
			if (b && !bvn_int_add_u32(&R, 1u)) { fail = true; break; }
		}
	}
	if (fail) { bvnf_scratch_free(&R); bvnf_scratch_free(&H); return false; }
	if (!bvn_int_is_zero(&R)) sticky = true;
	bvnf_scratch_free(&R);
	bvnf_scratch_free(&H);
	*sticky_out = sticky;
	return true;
}
/*
 * The heart of correct decimal->binary conversion: turn the exact rational
 * num/den into a correctly-rounded bvn_float of precision f->_prec.
 *
 * Steps: estimate the binary exponent E from the bit lengths (refined by one
 * comparison so the leading mantissa bit lands right), then divide to obtain
 * prec+2 quotient bits — prec mantissa bits plus the guard and round bits —
 * while tracking a sticky bit for everything beyond. Depending on the sign of
 * the required shift and whether the operands fit the big-int budget, the
 * division is done either exactly via bvn_int_divrem (remainder gives sticky)
 * or via the bounded-memory bvnf_bitdiv. The classic round-to-nearest-even
 * decision uses guard/round/sticky and the mantissa LSB; a carry out of the
 * mantissa on round-up bumps the exponent. The result is stored normalised by
 * bvnf_store_mant.
 */
static bool bvnf_rational_to_float(bvn_float_t *f, bool neg,
									bvn_int_t *num, bvn_int_t *den)
{
	if (bvn_int_is_zero(num)) { bvn_float_set_zero(f, neg); return true; }
	int prec = (int)f->_prec;
	int nb   = bvn_int_bitlen(num);
	int db   = bvn_int_bitlen(den);
	int E = nb - db;
	{
		uint32_t hint = bvnf_hint_limbs((uint32_t)((nb > db ? nb : db) + (E < 0 ? -E : E)));
		bvn_int_t tmp;
		if (!bvnf_scratch(&tmp, hint)) return false;
		if (E >= 0) {
			if (bvn_int_copy(&tmp, den) && bvn_int_shl(&tmp, E))
				if (bvn_int_cmp(&tmp, num) > 0) E--;
		} else {
			if (bvn_int_copy(&tmp, num) && bvn_int_shl(&tmp, -E))
				if (bvn_int_cmp(den, &tmp) > 0) E--;
		}
		bvnf_scratch_free(&tmp);
	}
	/* _sign/_exp are committed here, before the division that can still fail.
	 * Every failure path below must therefore leave a VALID float behind: the
	 * mantissa is untouched on failure, so returning early used to leave `f`
	 * reporting is_regular() with an all-zero mantissa -- a state the rest of
	 * the library treats as impossible, and which bvn_float_to_str() rendered
	 * as the illegal "0e-9884" in base 10 and as "1p-32768" in base 16. Zero is
	 * the honest fallback: the caller was told false, and reading `f` anyway now
	 * yields a signed zero rather than nonsense. */
	f->_sign = neg ? -1 : 1;
	f->_exp  = (int64_t)E + 1;
	int shift = prec + 1 - E;
	int want  = prec + 2;
	uint32_t q_words = (uint32_t)(want + 31) / 32u + 4u;
	uint32_t *_Qb = calloc(q_words, sizeof(uint32_t));
	if (!_Qb) { bvn_float_set_zero(f, neg); return false; }
#define RTF_RET(v) do { \
		if (!(v)) bvn_float_set_zero(f, neg); \
		free(_Qb); \
		return (v); \
	} while (0)
	bool sticky = false, ok = true;
	if (shift >= 0 && nb + shift > (int)BVN_INT_MAX_BITS - 32) {
		ok = bvnf_bitdiv(num, shift, den, _Qb, q_words, want, &sticky);
	} else if (shift >= 0) {
		bvn_int_t Q_int = { _Qb, q_words, 0, false, false, { 0, 0 } };
		bvn_int_t scaled, R_int;
		if (!bvnf_scratch(&scaled, bvnf_hint_limbs((uint32_t)(nb + shift)))) {
			ok = false;
		} else if (!bvnf_scratch(&R_int, bvnf_hint_limbs((uint32_t)db))) {
			bvnf_scratch_free(&scaled);
			ok = false;
		} else {
			ok = bvn_int_copy(&scaled, num) && bvn_int_shl(&scaled, shift)
				 && bvn_int_divrem(&Q_int, &R_int, &scaled, den);
			if (ok) sticky = !bvn_int_is_zero(&R_int);
			bvnf_scratch_free(&scaled);
			bvnf_scratch_free(&R_int);
		}
	} else {
		bvn_int_t Q_int = { _Qb, q_words, 0, false, false, { 0, 0 } };
		bvn_int_t dscaled, R_int;
		if (!bvnf_scratch(&dscaled, bvnf_hint_limbs((uint32_t)(db - shift)))) {
			ok = false;
		} else if (!bvnf_scratch(&R_int, bvnf_hint_limbs((uint32_t)(db - shift)))) {
			bvnf_scratch_free(&dscaled);
			ok = false;
		} else {
			ok = bvn_int_copy(&dscaled, den) && bvn_int_shl(&dscaled, -shift)
				 && bvn_int_divrem(&Q_int, &R_int, num, &dscaled);
			if (ok) sticky = !bvn_int_is_zero(&R_int);
			bvnf_scratch_free(&dscaled);
			bvnf_scratch_free(&R_int);
		}
	}
	if (!ok) RTF_RET(false);
	bvn_int_t Q_view = { _Qb, q_words, q_words, false, false, { 0, 0 } };
	while (Q_view.nused > 0 && _Qb[Q_view.nused - 1] == 0) Q_view.nused--;
	bool rbit     = (bool)bvn_int_getbit(&Q_view, 0);
	bool gbit     = (bool)bvn_int_getbit(&Q_view, 1);
	bool lsbit    = (prec > 0) ? (bool)bvn_int_getbit(&Q_view, 2) : false;
	bool round_up = gbit && (rbit || sticky || lsbit);
	bvn_int_shr(&Q_view, 2);
	if (round_up) {
		if (!bvn_int_add_u32(&Q_view, 1u)) RTF_RET(false);
		if (bvn_int_bitlen(&Q_view) > prec) {
			bvn_int_shr(&Q_view, 1);
			f->_exp++;
		}
	}
	bvnf_store_mant(f, &Q_view);
#undef RTF_RET
	free(_Qb);
	return true;
}
/*
 * Round the exact value (-1)^neg * num/den (num > 0, den > 0) DIRECTLY to an
 * IEEE-754 binary interchange format (man_bits fraction bits, exp_bits, bias)
 * with a single round-to-nearest-even, and pack it into bits[] using the same
 * field layout as bvn_float_to_ieee_bin. This is the correct decimal/hex-float
 * -> binary conversion: it rounds the exact rational once, at the position the
 * target format actually rounds at, so it is correct for normals, subnormals,
 * overflow (-> Inf) and underflow (-> 0) alike, with no intermediate fixed-width
 * bvn_float to double-round through.
 *
 * Method (the textbook big-integer comparison, identical in spirit to
 * bvnf_rational_to_float but clamped to the format): estimate E with
 * 2^E <= V < 2^{E+1}; pick the unit-in-last-place exponent F (F = E - man_bits
 * for a normal result, F = e_min - man_bits for the subnormal/underflow region,
 * where e_min = 1 - bias); form Q = round(V * 2^-F) by dividing num*2^(2-F) by
 * den, keeping two extra low quotient bits (round + guard) plus an exact sticky
 * bit from the remainder, then applying ties-to-even; finally normalise Q into
 * the biased exponent and fraction, propagating a mantissa carry into the
 * exponent and saturating to Infinity on overflow.
 */
static void bvnf_rational_to_ieee_bin(bool neg, bvn_int_t *num, bvn_int_t *den,
		uint32_t exp_bits, uint32_t man_bits, int32_t bias,
		uint32_t *bits, int bits32)
{
	int  total = 1 + (int)exp_bits + (int)man_bits;
	long eall  = (long)((1u << exp_bits) - 1u);   /* reserved all-ones exponent */
	long emin  = 1L - (long)bias;                 /* min normal unbiased exponent */
	int  i;
	for (i = 0; i < bits32; i++) bits[i] = 0;
	if (neg) bits[(total - 1) / 32] |= 1u << ((total - 1) % 32);
	if (bvn_int_is_zero(num)) return;             /* +/-0 (sign already set) */

	/* --- estimate E such that 2^E <= num/den < 2^{E+1} --- */
	int  nb = bvn_int_bitlen(num);
	int  db = bvn_int_bitlen(den);
	long E  = (long)nb - (long)db;
	{
		uint32_t hint = bvnf_hint_limbs((uint32_t)((nb > db ? nb : db)
									   + (E < 0 ? -(int)E : (int)E) + 4));
		bvn_int_t tmp;
		if (bvnf_scratch(&tmp, hint)) {
			if (E >= 0) {
				if (bvn_int_copy(&tmp, den) && bvn_int_shl(&tmp, (int)E))
					if (bvn_int_cmp(&tmp, num) > 0) E--;
			} else {
				if (bvn_int_copy(&tmp, num) && bvn_int_shl(&tmp, (int)(-E)))
					if (bvn_int_cmp(den, &tmp) > 0) E--;
			}
			bvnf_scratch_free(&tmp);
		}
	}

	/* early overflow: V >= 2^{e_max+1} can only round to Infinity */
	if (E + (long)bias >= eall) {
		for (i = 0; i < (int)exp_bits; i++)
			bits[(man_bits + (uint32_t)i) / 32] |= 1u << ((man_bits + (uint32_t)i) % 32);
		return;
	}

	/* --- choose the ulp exponent F and the provisional biased exponent --- */
	long be, F;
	if (E + (long)bias >= 1L) { F = E - (long)man_bits;   be = E + (long)bias; }
	else                      { F = emin - (long)man_bits; be = 0;             }

	/* --- Q_ext = floor(num/den * 2^(2-F)); two low bits kept as round+guard --- */
	long     shift  = 2L - F;
	int      want   = (int)man_bits + 8;          /* >= significand+guard bit count */
	uint32_t q_words = (uint32_t)(want + 31) / 32u + 4u;
	uint32_t *Qb = calloc(q_words, sizeof(uint32_t));
	if (!Qb) return;                              /* OOM: leave +/-0 */
	bool sticky = false, ok = true;

	if (shift >= 0) {
		if (shift > (long)INT_MAX) {
			ok = false;
		} else if (nb + (int)shift > (int)BVN_INT_MAX_BITS - 32) {
			ok = bvnf_bitdiv(num, (int)shift, den, Qb, q_words, want, &sticky);
		} else {
			bvn_int_t Q_int = { Qb, q_words, 0, false, false, { 0, 0 } };
			bvn_int_t scaled, R_int;
			if (!bvnf_scratch(&scaled, bvnf_hint_limbs((uint32_t)(nb + (int)shift + 4)))) {
				ok = false;
			} else if (!bvnf_scratch(&R_int, bvnf_hint_limbs((uint32_t)(db + 4)))) {
				bvnf_scratch_free(&scaled); ok = false;
			} else {
				ok = bvn_int_copy(&scaled, num) && bvn_int_shl(&scaled, (int)shift)
					 && bvn_int_divrem(&Q_int, &R_int, &scaled, den);
				if (ok) sticky = !bvn_int_is_zero(&R_int);
				bvnf_scratch_free(&scaled);
				bvnf_scratch_free(&R_int);
			}
		}
	} else {
		long ns = -shift;
		if (ns > (long)INT_MAX) {
			ok = false;
		} else {
			bvn_int_t Q_int = { Qb, q_words, 0, false, false, { 0, 0 } };
			bvn_int_t dscaled, R_int;
			if (!bvnf_scratch(&dscaled, bvnf_hint_limbs((uint32_t)(db + (int)ns + 4)))) {
				ok = false;
			} else if (!bvnf_scratch(&R_int, bvnf_hint_limbs((uint32_t)(db + (int)ns + 4)))) {
				bvnf_scratch_free(&dscaled); ok = false;
			} else {
				ok = bvn_int_copy(&dscaled, den) && bvn_int_shl(&dscaled, (int)ns)
					 && bvn_int_divrem(&Q_int, &R_int, num, &dscaled);
				if (ok) sticky = !bvn_int_is_zero(&R_int);
				bvnf_scratch_free(&dscaled);
				bvnf_scratch_free(&R_int);
			}
		}
	}
	if (!ok) {
		/* The exact division would exceed the big-int budget -- only reachable
		 * at astronomically large/small exponents that cannot be constructed in
		 * BVN_INT_MAX_BITS to begin with. Saturate by magnitude so we never emit
		 * a wrong finite value: large -> Inf, tiny -> 0. */
		free(Qb);
		if (E >= 0)
			for (i = 0; i < (int)exp_bits; i++)
				bits[(man_bits + (uint32_t)i) / 32] |= 1u << ((man_bits + (uint32_t)i) % 32);
		return;
	}

	bvn_int_t Q = { Qb, q_words, q_words, false, false, { 0, 0 } };
	while (Q.nused > 0 && Qb[Q.nused - 1] == 0) Q.nused--;

	/* round-to-nearest-even: bit1 is the guard (rounding) bit, bit0 plus the
	 * division sticky is the tail below it, bit2 is the kept significand's LSB. */
	bool rbit = (bool)bvn_int_getbit(&Q, 0);
	bool gbit = (bool)bvn_int_getbit(&Q, 1);
	bool lsb  = (bool)bvn_int_getbit(&Q, 2);
	bool round_up = gbit && (rbit || sticky || lsb);
	bvn_int_shr(&Q, 2);                            /* drop guard bits -> units of 2^F */
	if (round_up) {
		if (!bvn_int_add_u32(&Q, 1u)) { free(Qb); return; }
	}

	/* --- normalise into (be, fraction), carrying into the exponent --- */
	if (be > 0) {                                  /* normal */
		if (bvn_int_bitlen(&Q) > (int)man_bits + 1) {  /* 1.11..1 + 1 -> 10.0 */
			bvn_int_shr(&Q, 1);
			be++;
		}
		if (be >= eall) {                          /* overflow -> Infinity */
			free(Qb);
			for (i = 0; i < (int)exp_bits; i++)
				bits[(man_bits + (uint32_t)i) / 32] |= 1u << ((man_bits + (uint32_t)i) % 32);
			return;
		}
	}
	/* a subnormal that rounded up to 2^man_bits is exactly the smallest normal */
	if (be == 0 && bvn_int_getbit(&Q, (int)man_bits) == 1) be = 1;

	/* pack the biased exponent and the fraction (implicit leading bit dropped) */
	if (be > 0) {
		int wi = (int)man_bits / 32, b2 = (int)man_bits % 32;
		if ((uint32_t)wi < Q.nlimbs) Qb[wi] &= ~(1u << b2);
		while (Q.nused > 0 && Qb[Q.nused - 1] == 0) Q.nused--;
		bvn_int_t eb;
		if (bvnf_scratch(&eb, bvnf_hint_limbs(man_bits + 64u))) {
			bvn_int_from_uint64(&eb, (uint64_t)be);
			bvn_int_shl(&eb, (int)man_bits);
			for (i = 0; (uint32_t)i < eb.nused && i < bits32; i++)
				bits[i] |= eb.limbs[i];
			bvnf_scratch_free(&eb);
		}
	}
	for (i = 0; (uint32_t)i < Q.nused && i < bits32; i++)
		bits[i] |= Qb[i];
	free(Qb);
}
static uint32_t bvnf_digit(uint8_t c, uint32_t base)
{
	uint32_t d;
	if      (c >= '0' && c <= '9') d = (uint32_t)(c - '0');
	else if (c >= 'a' && c <= 'z') d = 10u + (uint32_t)(c - 'a');
	else if (c >= 'A' && c <= 'Z') d = 10u + (uint32_t)(c - 'A');
	else return base;
	return (d < base) ? d : base;
}
static bool bvnf_pow2_base(uint32_t base, uint32_t *log2base)
{
	if (!base || (base & (base - 1u))) return false;
	uint32_t k = 0;
	while ((1u << k) < base) k++;
	*log2base = k;
	return true;
}
/*
 * Shared exact-rational parser for base 10 / base 16 (hex-float) literals. Reads
 * the mantissa digits and the (decimal 'e' or binary 'p') exponent into an exact
 * rational num/den -- fractional digits and negative exponents go into the
 * denominator, integer scaling into the numerator -- and reports what the
 * literal denotes via the return value. num and den are caller-owned scratch
 * ints (the caller allocates and frees them); on BVNF_RK_OK they hold the exact
 * value's numerator and denominator (both > 0) and *neg_out carries the sign.
 * NAN/INF/ZERO are returned with *neg_out set so the caller can pack the right
 * signed pattern; FAIL means the text is not a number. Both bvn_float_from_str
 * (which then rounds to a bvn_float) and bvn_float_strtoieee_bin (which rounds
 * straight to an IEEE binary format) build on this single parser.
 *
 * UNDERFLOW is ZERO's counterpart for a literal that is NOT zero but whose exact
 * rational will not fit the big-int budget — 1.5e-20000, or a 2000-digit mantissa
 * at e-9000. A rounding caller treats it exactly like ZERO (that is what it
 * rounds to). A caller that wants the EXACT value must not: reporting such a
 * literal as "exactly 0" is wrong by every digit, which is why it is a kind of
 * its own rather than folded into ZERO. It mirrors INF on the overflow side.
 */
typedef enum {
	BVNF_RK_OK = 0,
	BVNF_RK_NAN,
	BVNF_RK_INF,
	BVNF_RK_ZERO,
	BVNF_RK_UNDERFLOW,
	BVNF_RK_FAIL
} bvnf_rkind;
static bvnf_rkind bvnf_parse_rational(const char *s, uint32_t base,
									  bool *neg_out, bvn_int_t *num, bvn_int_t *den)
{
	*neg_out = false;
	if (!s || (base != 10u && base != 16u)) return BVNF_RK_FAIL;
	while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
	bool neg = false;
	if      (*s == '+') s++;
	else if (*s == '-') { neg = true; s++; }
	*neg_out = neg;
	/*
	 * Accept the conventional C99 hex-float prefix. Only skip "0x"/"0X" when a
	 * hex digit or radix point follows, so a bare "0" (the value zero) is left
	 * for the digit scanner. Without this, "0x1.8p+1" parsed the leading '0'
	 * and stopped at 'x', silently yielding 0.0.
	 */
	if (base == 16u && s[0] == '0' && (s[1] == 'x' || s[1] == 'X') &&
		(bvnf_digit((uint8_t)s[2], 16u) < 16u || s[2] == '.'))
		s += 2;
	{
		const char *p = s;
		uint8_t c0 = (uint8_t)(*p | 0x20);
		/* The NUL terminator check matters: without it "nanana" was a NaN and
		 * "infinity" an infinity, and both p[1]/p[2] reads stay in bounds
		 * because a mismatch short-circuits before the next index. */
		if (c0 == 'n' && ((uint8_t)(p[1]|0x20)) == 'a' &&
		    ((uint8_t)(p[2]|0x20)) == 'n' && p[3] == '\0')
			return BVNF_RK_NAN;
		if (c0 == 'i' && ((uint8_t)(p[1]|0x20)) == 'n' &&
		    ((uint8_t)(p[2]|0x20)) == 'f' && p[3] == '\0')
			return BVNF_RK_INF;
		/* The format's own spelling of negative infinity, which is what an
		 * ev_data token carries and what bvn_float_to_str writes. */
		if (!neg && p[0] == 'n' && p[1] == 'i' && p[2] == 'n' &&
		    p[3] == 'f' && p[4] == '\0') {
			*neg_out = true;
			return BVNF_RK_INF;
		}
	}
	bvn_int_zero(num);
	bvn_int_from_uint64(den, 1u);
	uint32_t acc = 0, acc_d = 0;
	bool overflow = false;
	int frac_digits = 0;
	bool has_int  = false;
	bool has_frac = false;
#define FLUSH_CHUNK() \
	do { \
		if (acc_d > 0 && !overflow) { \
			uint32_t _mul = 1; \
			for (uint32_t _k = 0; _k < acc_d; _k++) _mul *= base; \
			if (!bvn_int_mul_u32(num, _mul)) { overflow = true; break; } \
			if (!bvn_int_add_u32(num, acc))  { overflow = true; break; } \
			acc = 0; acc_d = 0; \
		} \
	} while (0)
	while (!overflow) {
		uint32_t d = bvnf_digit((uint8_t)*s, base);
		if (d >= base) break;
		has_int = true;
		acc = acc * base + d; acc_d++;
		uint32_t cap = (base <= 10) ? 9u :
					   (base <= 16) ? 7u :
					   (base <= 32) ? 6u : 5u;
		if (acc_d >= cap) FLUSH_CHUNK();
		s++;
	}
	FLUSH_CHUNK();
	if (*s == '.') {
		s++;
		while (!overflow) {
			uint32_t d = bvnf_digit((uint8_t)*s, base);
			if (d >= base) break;
			has_frac = true;
			acc = acc * base + d; acc_d++;
			frac_digits++;
			uint32_t cap = (base <= 10) ? 9u :
						   (base <= 16) ? 7u :
						   (base <= 32) ? 6u : 5u;
			if (acc_d >= cap) FLUSH_CHUNK();
			s++;
		}
		FLUSH_CHUNK();
	}
	if (!has_int && !has_frac) return BVNF_RK_FAIL;
	long exp_val = 0;
	bool use_bin_exp = false;
	bool eneg = false;
	bool exp_overflow = false;
	/*
	 * Exponent marker is base-specific: 'e'/'E' (decimal exponent) for base 10,
	 * 'p'/'P' (binary exponent) for base 16. In base 16 'e'/'E' are digits and
	 * were already consumed above, so they can never reach here; we must also not
	 * accept 'p' in base 10 (it is not a valid base-10 token).
	 */
	if ((base == 10u && (*s == 'e' || *s == 'E')) ||
		(base == 16u && (*s == 'p' || *s == 'P'))) {
		use_bin_exp = (base == 16u);
		s++;
		if      (*s == '+') s++;
		else if (*s == '-') { eneg = true; s++; }
		long eabs = 0; bool has_e = false;
		while (*s >= '0' && *s <= '9') {
			if (!exp_overflow) {
				long d = (long)(*s - '0');
				if (eabs > (LONG_MAX - d) / 10L) exp_overflow = true;
				else eabs = eabs * 10L + d;
			}
			has_e = true; s++;
		}
		if (!has_e) return BVNF_RK_FAIL;
		if (exp_overflow) {
			/* An exponent too long to COUNT says nothing about a mantissa
			 * of zero: zero times any power is zero, and
			 * "0.000e999999999999999999999" came back as +inf because the
			 * shortcut answered before looking. The sign is the caller's
			 * (*neg_out), so "-0e..." stays -0. The literal still has to
			 * end here to be one. */
			if (*s != '\0') return BVNF_RK_FAIL;
			if (!overflow && bvn_int_is_zero(num)) return BVNF_RK_ZERO;
			return eneg ? BVNF_RK_UNDERFLOW : BVNF_RK_INF;
		}
		exp_val = eneg ? -eabs : eabs;
	}
	/* Everything above stops at the first byte it does not recognise, so without
	 * this the parser happily returned a value for a prefix and ignored the rest:
	 * "1.2.3" was 1.2, "12 34" was 12, "1e2e3" was 100, "0x1.8p1zzz" was 3. The
	 * header promises false for a malformed literal, and bvn_int_from_str already
	 * rejects the analogous "12 3", so the two string APIs disagreed. */
	if (*s != '\0') return BVNF_RK_FAIL;
	if (overflow) return (exp_val < 0) ? BVNF_RK_UNDERFLOW : BVNF_RK_INF;
	uint32_t log2b = 0;
	bool b_is_pow2 = bvnf_pow2_base(base, &log2b);
	if (bvn_int_is_zero(num)) return BVNF_RK_ZERO;
	if (use_bin_exp || b_is_pow2) {
		long net_shift;
		if (b_is_pow2) {
			net_shift = exp_val - (long)frac_digits * (long)log2b;
		} else {
			net_shift = exp_val;
			goto rational_path;
		}
		if (net_shift >= 0) {
			if (net_shift > (long)INT_MAX || !bvn_int_shl(num, (int)net_shift))
				return BVNF_RK_INF;
		} else {
			bvn_int_from_uint64(den, 1u);
			if (-net_shift > (long)INT_MAX || !bvn_int_shl(den, (int)(-net_shift)))
				return BVNF_RK_UNDERFLOW;
		}
	} else {
rational_path:;
		long net_b_exp = exp_val - (long)frac_digits;
		if (!use_bin_exp) {
			if (net_b_exp >= 0) {
				for (long i = 0; i < net_b_exp; i++)
					if (!bvn_int_mul_u32(num, base)) return BVNF_RK_INF;
			} else {
				for (long i = 0; i < -net_b_exp; i++)
					if (!bvn_int_mul_u32(den, base)) return BVNF_RK_UNDERFLOW;
			}
		} else {
			for (int i = 0; i < frac_digits; i++)
				if (!bvn_int_mul_u32(den, base)) return BVNF_RK_UNDERFLOW;
			if (exp_val >= 0) {
				if (exp_val > (long)INT_MAX || !bvn_int_shl(num, (int)exp_val))
					return BVNF_RK_INF;
			} else {
				if (-exp_val > (long)INT_MAX || !bvn_int_shl(den, (int)(-exp_val)))
					return BVNF_RK_UNDERFLOW;
			}
		}
	}
	if (bvn_int_is_zero(num)) return BVNF_RK_ZERO;
	return BVNF_RK_OK;
#undef FLUSH_CHUNK
}
/*
 * Public exact-rational parse: expose the internal rational builder for lossless
 * unit conversion. Yields value = num/den exactly, with the sign folded into
 * num. Only finite values succeed; nan/inf and malformed input return false.
 */
bool bvn_float_parse_rational(const char *s, uint32_t base,
                              bvn_int_t *num, bvn_int_t *den)
{
	if (!s || !num || !den || (base != 10u && base != 16u)) return false;
	bool neg = false;
	bvnf_rkind k = bvnf_parse_rational(s, base, &neg, num, den);
	if (k == BVNF_RK_ZERO) {               /* the literal really is zero */
		bvn_int_zero(num);
		return bvn_int_from_uint64(den, 1u);
	}
	/* nan / inf / malformed, and UNDERFLOW: a literal like 1.5e-20000 is not
	 * zero, it is a value too small to build a rational for. Handing back an
	 * exact zero here would be wrong by every digit, and silently so — the
	 * symmetric case, INF, has always been refused. */
	if (k != BVNF_RK_OK) return false;
	num->negative = neg && !bvn_int_is_zero(num);
	return true;
}
/*
 * Parse a floating literal in base 10 or 16 into a correctly-rounded bvn_float
 * of precision f->_prec. The exact rational is built by bvnf_parse_rational and
 * rounded once by bvnf_rational_to_float. Routing all parsing through the exact
 * rational is what makes round-tripping lossless, unlike strtod which only gives
 * double precision. (To convert directly to a fixed binary format such as
 * double, use bvn_float_strtoieee_bin / bvn_float_strtod instead -- they round
 * the exact rational straight to the target and never double-round.)
 */
bool bvn_float_from_str(bvn_float_t *f, const char *s, uint32_t base)
{
	if (!f || !s || (base != 10u && base != 16u)) return false;
	bvn_int_t num, den;
	uint32_t hint = bvnf_hint_limbs(f->_prec);
	if (!bvnf_scratch(&num, hint)) return false;
	if (!bvnf_scratch(&den, hint)) { bvnf_scratch_free(&num); return false; }
	bool neg = false;
	bvnf_rkind k = bvnf_parse_rational(s, base, &neg, &num, &den);
	bool rc;
	switch (k) {
		case BVNF_RK_OK:   rc = bvnf_rational_to_float(f, neg, &num, &den); break;
		case BVNF_RK_NAN:  bvn_float_set_nan(f);        rc = true;  break;
		case BVNF_RK_INF:  bvn_float_set_inf(f, neg);   rc = true;  break;
		case BVNF_RK_UNDERFLOW:   /* not representable exactly; rounds to +-0 */
		case BVNF_RK_ZERO: bvn_float_set_zero(f, neg);  rc = true;  break;
		case BVNF_RK_FAIL:
		default:           rc = false; break;
	}
	bvnf_scratch_free(&num);
	bvnf_scratch_free(&den);
	return rc;
}
/*
 * Public, externally-linked entry point: a base-10/16 string straight to a
 * correctly-rounded IEEE-754 binary interchange value, in a single rounding.
 * See the header for the contract. nan/inf/zero are packed with the requested
 * field widths exactly as bvn_float_to_ieee_bin would; finite values are rounded
 * by bvnf_rational_to_ieee_bin from the exact rational. Because nothing here
 * builds an intermediate fixed-width bvn_float, it cannot double-round, at any
 * input length, in either the normal or subnormal range -- which is the bug the
 * old parse-into-128-bits-then-narrow path had.
 */
/*
 * Validate a caller-supplied IEEE binary field geometry against the word
 * buffer it must fit in. Guards two hazards on the public {to,from,strto}ieee_bin
 * entry points, whose exp_bits/man_bits/bits32 are caller-controlled: a shift
 * 1u << exp_bits with exp_bits >= 32 is undefined behaviour, and a field whose
 * total bit count exceeds bits32*32 would index bits[] out of bounds. The
 * bundled bin16..bin256 wrappers always pass conforming geometries, so this only
 * rejects misuse of the arbitrary-width API.
 */
static bool bvnf_ieee_geom_ok(uint32_t exp_bits, uint32_t man_bits, int bits32)
{
	return exp_bits < 32u && bits32 > 0 &&
		   1L + (long)exp_bits + (long)man_bits <= (long)bits32 * 32L;
}
void bvn_float_strtoieee_bin(const char *s, uint32_t base,
		uint32_t exp_bits, uint32_t man_bits, int32_t bias,
		uint32_t *bits, int bits32)
{
	if (!bits) return;
	for (int i = 0; i < bits32; i++) bits[i] = 0;
	if (!s || (base != 10u && base != 16u)) return;
	if (!bvnf_ieee_geom_ok(exp_bits, man_bits, bits32)) return;
	int total = 1 + (int)exp_bits + (int)man_bits;
	bvn_int_t num, den;
	uint32_t hint = bvnf_hint_limbs(man_bits + 64u);
	if (!bvnf_scratch(&num, hint)) return;
	if (!bvnf_scratch(&den, hint)) { bvnf_scratch_free(&num); return; }
	bool neg = false;
	bvnf_rkind k = bvnf_parse_rational(s, base, &neg, &num, &den);
	switch (k) {
	case BVNF_RK_OK:
		bvnf_rational_to_ieee_bin(neg, &num, &den, exp_bits, man_bits, bias, bits, bits32);
		break;
	case BVNF_RK_NAN:
		for (uint32_t i = 0; i < exp_bits; i++)
			bits[(man_bits + i) / 32] |= 1u << ((man_bits + i) % 32);
		if (man_bits > 0) bits[(man_bits - 1) / 32] |= 1u << ((man_bits - 1) % 32);
		if (neg) bits[(total - 1) / 32] |= 1u << ((total - 1) % 32);
		break;
	case BVNF_RK_INF:
		for (uint32_t i = 0; i < exp_bits; i++)
			bits[(man_bits + i) / 32] |= 1u << ((man_bits + i) % 32);
		if (neg) bits[(total - 1) / 32] |= 1u << ((total - 1) % 32);
		break;
	case BVNF_RK_UNDERFLOW:       /* not representable exactly; rounds to +-0 */
	case BVNF_RK_ZERO:
		if (neg) bits[(total - 1) / 32] |= 1u << ((total - 1) % 32);
		break;
	case BVNF_RK_FAIL:
	default:
		break;                                  /* +0.0 */
	}
	bvnf_scratch_free(&num);
	bvnf_scratch_free(&den);
}
size_t bvn_float_str_bufsize(uint32_t prec, uint32_t base)
{
	size_t digits;
	/* Base 0 and 1 make the log2b_floor loop below yield 0 and the division
	 * trap with SIGFPE. This is BVN_API with no documented precondition, and the
	 * sibling bvn_int_str_bufsize() already screens base < 2 the same way. */
	if (base < 2u) return 64u;
	if (base == 10u) {
		digits = (size_t)prec * 302u / 1000u + 24u;
	} else {
		uint32_t log2b_floor = 1u;
		uint32_t b = base;
		while (b >= 2u) { log2b_floor++; b >>= 1u; }
		log2b_floor--;
		digits = (size_t)prec / log2b_floor + 24u;
	}
	return digits + 48u;
}
static int32_t bvnf_to_str_pow2(const bvn_float_t *f, char *buf,
								  size_t bufsize, uint32_t log2b)
{
	static const char DIGS[] = "0123456789abcdefghijklmnopqrstuvwxyz";
	long prec = (long)f->_prec;
	long frac_bits = prec - 1L;
	long nfrac = (frac_bits + (long)log2b - 1L) / (long)log2b;
	char *fdigs = calloc((size_t)(nfrac + 1), 1u);
	if (!fdigs) return -1;
	for (long d = 0; d < nfrac; d++) {
		long bit_pos = prec - 2L - d * (long)log2b;
		uint32_t digit = 0;
		for (uint32_t b = 0; b < log2b; b++) {
			digit <<= 1;
			long bp = bit_pos - (long)b;
			if (bp >= 0) {
#if BVN_LIMB_BITS == 64
				uint32_t li = (uint32_t)bp / 64u;
				uint32_t lb = (uint32_t)bp % 64u;
				digit |= (li < f->_nlimbs)
				       ? (uint32_t)((f->_d[li] >> lb) & 1u) : 0u;
#else
				uint32_t li = (uint32_t)bp / 32u;
				uint32_t lb = (uint32_t)bp % 32u;
				digit |= (li < f->_nlimbs)
				       ? ((f->_d[li] >> lb) & 1u) : 0u;
#endif
			}
		}
		fdigs[d] = DIGS[digit < 37u ? digit : 0u];
	}
	long last_nz = nfrac - 1L;
	while (last_nz >= 0 && fdigs[last_nz] == '0') last_nz--;
	size_t pos = 0;
	int32_t ret = -1;
#define PUTC(c) do { if (pos + 1 >= bufsize) goto p2_done; buf[pos++] = (char)(c); } while (0)
	if (f->_sign < 0) PUTC('-');
	PUTC('1');
	if (last_nz >= 0) {
		PUTC('.');
		for (long d = 0; d <= last_nz; d++) PUTC(fdigs[d]);
	}
	{
		int64_t bin_exp = f->_exp - 1;        /* _exp is int64; long would truncate on LLP64 */
		PUTC('p');
		if (bin_exp >= 0) {
			PUTC('+');
		} else {
			PUTC('-');
			bin_exp = -bin_exp;
		}
		char ebuf[32]; int en = 0;
		if (bin_exp == 0) {
			ebuf[en++] = '0';
		} else {
			int64_t ev = bin_exp;
			while (ev > 0) { ebuf[en++] = (char)('0' + ev % 10); ev /= 10; }
			for (int i = 0, j = en - 1; i < j; i++, j--) {
				char t = ebuf[i]; ebuf[i] = ebuf[j]; ebuf[j] = t;
			}
		}
		for (int i = 0; i < en; i++) PUTC(ebuf[i]);
	}
	buf[pos] = '\0';
	ret = (int32_t)pos;
p2_done:
#undef PUTC
	free(fdigs);
	return ret;
}
/*
 * Exact equality of two finite, same-precision bvn_floats. The representation is
 * canonical (normalised leading mantissa bit), so identical sign, exponent and
 * mantissa limbs imply identical real values. Used to test whether a shortened
 * decimal still parses back to the original float.
 */
static bool bvnf_regular_eq(const bvn_float_t *a, const bvn_float_t *b)
{
	if (a->_sign != b->_sign || a->_exp != b->_exp) return false;
	uint32_t nl = (a->_nlimbs < b->_nlimbs) ? a->_nlimbs : b->_nlimbs;
	if (memcmp(a->_d, b->_d, (size_t)nl * sizeof(bvn_limb_t)) != 0) return false;
	for (uint32_t i = nl; i < a->_nlimbs; i++) if (a->_d[i]) return false;
	for (uint32_t i = nl; i < b->_nlimbs; i++) if (b->_d[i]) return false;
	return true;
}
/*
 * Take the first k significant digits of a digit string, either truncated
 * (up == false) or rounded up by one unit in the last place (up == true). These
 * two candidates bracket the value, so the nearest k-digit decimal is always one
 * of them. A carry out of the leading digit (…9 -> 10) collapses to "1" and
 * reports *eshift = 1 so the caller bumps the decimal exponent. Trailing zeros
 * are trimmed. The output buffer doubles as scratch for the digit values.
 */
static void bvnf_take_k(const char *digs, long n, uint32_t base,
						long k, bool up, char *out, long *out_n, long *eshift)
{
	static const char D[] = "0123456789abcdefghijklmnopqrstuvwxyz";
	uint8_t *v = (uint8_t *)out;
	(void)n;
	for (long i = 0; i < k; i++) v[i] = (uint8_t)bvnf_digit((uint8_t)digs[i], base);
	*eshift = 0;
	if (up) {
		long j = k - 1;
		for (;;) {
			v[j]++;
			if ((uint32_t)v[j] < base) break;
			v[j] = 0;
			if (j == 0) { v[0] = 1; *eshift = 1; break; }
			j--;
		}
	}
	long m = k;
	while (m > 1 && v[m - 1] == 0) m--;
	for (long i = 0; i < m; i++) out[i] = D[v[i]];
	*out_n = m;
}
/*
 * Build the decimal text for one candidate (sign, leading digit, fraction, 'e',
 * exponent), parse it back at the float's own precision and base, and report
 * whether it reproduces f exactly. This is the round-trip oracle that drives the
 * shortening search.
 */
static bool bvnf_rt_candidate(const bvn_float_t *f, uint32_t base,
							  const char *d, long rn, long E,
							  char *cnd, bvn_float_t *t)
{
	char *c = cnd;
	if (f->_sign < 0) *c++ = '-';
	*c++ = d[0];
	if (rn > 1) { *c++ = '.'; for (long i = 1; i < rn; i++) *c++ = d[i]; }
	*c++ = 'e';
	long ev = E;
	if (ev < 0) { *c++ = '-'; ev = -ev; } else *c++ = '+';
	char eb[24]; int en = 0;
	if (ev == 0) eb[en++] = '0';
	else while (ev > 0) { eb[en++] = (char)('0' + ev % 10); ev /= 10; }
	while (en > 0) *c++ = eb[--en];
	*c = '\0';
	return bvn_float_from_str(t, cnd, base) && bvnf_regular_eq(t, f);
}
/*
 * Reduce a faithful, correctly-rounded digit string to the fewest digits that
 * still parse back to exactly f at its own precision — true shortest round-trip.
 * Feasibility is monotonic in the digit count k (more digits is a strictly closer
 * approximation that stays inside the round-trip interval), so a binary search
 * suffices; at each k we test both the truncated and the rounded-up candidate,
 * since the nearest k-digit decimal is one of the two. The final length picks the
 * candidate nearest the true value (ties to even). On any allocation or parse
 * failure the input is left unchanged — still correct, just not minimal.
 */
static void bvnf_shorten_digits(const bvn_float_t *f, uint32_t base,
								char *digs, long *pn, long *pE)
{
	long n = *pn;
	if (n <= 1) return;
	bvn_float_t *t   = bvn_float_alloc(f->_prec);
	char        *cnd = malloc((size_t)n + 32u);
	char        *df  = malloc((size_t)n + 2u);
	char        *dc  = malloc((size_t)n + 2u);
	if (!t || !cnd || !df || !dc) {
		free(cnd); free(df); free(dc); bvn_float_free(t); return;
	}
	long lo = 1, hi = n;
	while (lo < hi) {
		long k = lo + (hi - lo) / 2;
		long fn, fe, cn, ce;
		bvnf_take_k(digs, n, base, k, false, df, &fn, &fe);
		bvnf_take_k(digs, n, base, k, true,  dc, &cn, &ce);
		bool feasible = bvnf_rt_candidate(f, base, df, fn, *pE + fe, cnd, t)
					 || bvnf_rt_candidate(f, base, dc, cn, *pE + ce, cnd, t);
		if (feasible) hi = k; else lo = k + 1;
	}
	{
		long k = lo, fn, fe, cn, ce;
		bvnf_take_k(digs, n, base, k, false, df, &fn, &fe);
		bvnf_take_k(digs, n, base, k, true,  dc, &cn, &ce);
		uint32_t nd = (k < n) ? bvnf_digit((uint8_t)digs[k], base) : 0u;
		bool sticky = false;
		for (long i = k + 1; i < n; i++)
			if (bvnf_digit((uint8_t)digs[i], base) != 0u) { sticky = true; break; }
		uint32_t last = bvnf_digit((uint8_t)digs[k - 1], base);
		bool near_is_ceil = (2u * nd > base) ||
			(2u * nd == base && (sticky || (last & 1u)));
		bool near_ok = near_is_ceil
			? bvnf_rt_candidate(f, base, dc, cn, *pE + ce, cnd, t)
			: bvnf_rt_candidate(f, base, df, fn, *pE + fe, cnd, t);
		bool use_ceil = near_is_ceil ? near_ok : !near_ok;
		if (use_ceil) { memcpy(digs, dc, (size_t)cn); *pn = cn; *pE += ce; }
		else          { memcpy(digs, df, (size_t)fn); *pn = fn; *pE += fe; }
	}
	free(cnd); free(df); free(dc); bvn_float_free(t);
}
static int32_t bvnf_to_str_dec(const bvn_float_t *f, char *buf, size_t bufsize)
{
	if (f->_prec > BVN_FLOAT_MAX_PREC) return -1;
	if (f->_exp  >  1000000000L || f->_exp < -1000000000L) return -1;
	/* Explicit casts: long is 64-bit on LP64 (Linux) but 32-bit on Windows
	 * (LLP64). _prec (<= BVN_FLOAT_MAX_PREC) and _exp (guarded to +-1e9 above)
	 * both fit a 32-bit long, so the value is identical on every target; the
	 * casts only document the intended (bounded) narrowing for -Wconversion. */
	long prec = (long)f->_prec;
	long exp2 = (long)f->_exp;
	long e2   = exp2 - prec;
	long E10;
	{
		/*
		 * Estimate E10 ~= (exp2-1)*log10(2). The old 302/1000 (=0.302)
		 * approximation overshot log10(2) by ~1e-3 per unit, so past ~ndigits*1000
		 * of binary exponent it placed the whole ndigits-wide extraction window
		 * ABOVE the value's true leading digit: every extracted digit came out
		 * zero, and the leading-zero strip below — which rotates a '0' into the
		 * tail without shrinking the digit count — then spun forever (a hang on
		 * e.g. to_str(1e4900) at 16-bit precision). 30103/100000 (=0.30103) is
		 * within 5e-9 of log10(2), matching bvnf_dec_render_roundodd; the strip
		 * loop is additionally bounded so a residual 1-ulp estimate error can
		 * never re-introduce the spin.
		 */
		long ep = exp2 - 1L;
		if (ep >= 0)  E10 =  (long)(((int64_t)ep * 30103 + 99999) / 100000);
		else          E10 = -(long)(((int64_t)(-ep) * 30103) / 100000);
	}
	long ndigits = (prec * 302L + 999L) / 1000L + 4L;
	long k = ndigits - 1L - E10;
	if (f->_nlimbs > (UINT32_MAX - 4u) / 2u) return -1;
	bvnf_bn M = {0}; if (!bvnf_bn_init(&M, f->_nlimbs * 2u)) return -1;
	if (!bvnf_bn_from_float_mant(&M, f)) { bvnf_bn_free(&M); return -1; }
	long net2 = e2 + k;
	if (k >= 0) {
		bvnf_bn_mul_pow5(&M, (uint32_t)k);
		if      (net2 > 0) bvnf_bn_shl(&M, (uint32_t)net2);
		else if (net2 < 0) bvnf_bn_shr(&M, (uint32_t)(-net2));
	} else {
		long neg_k = -k;
		if      (net2 > 0) bvnf_bn_shl(&M, (uint32_t)net2);
		else if (net2 < 0) bvnf_bn_shr(&M, (uint32_t)(-net2));
		for (long i = 0; i < neg_k; i++)
			bvnf_bn_div_u32(&M, 5u);
	}
	if (ndigits <= 0) { bvnf_bn_free(&M); return -1; }
	char *digs = calloc((size_t)(ndigits + 4), 1u);
	if (!digs) { bvnf_bn_free(&M); return -1; }
	for (long i = 0; i < ndigits; i++) {
		uint32_t rem = bvnf_bn_div_u32(&M, 10u);
		digs[i] = (char)('0' + rem);
	}
	bvnf_bn_free(&M);
	for (long i = 0, j = ndigits - 1; i < j; i++, j--) {
		char t = digs[i]; digs[i] = digs[j]; digs[j] = t;
	}
	{
		/*
		 * Strip leading zeros, lowering E10 to track the leading digit. The guard
		 * `lead < ndigits` is essential: if the exponent estimate overshot far
		 * enough that EVERY extracted digit is zero, digs[0] never becomes
		 * non-zero and the unbounded form of this loop never terminates. Bounding
		 * it by the digit count makes a degenerate all-zero window collapse to the
		 * single digit "0" instead of hanging.
		 */
		long lead = 0;
		while (lead < ndigits - 1 && digs[lead] == '0') lead++;
		if (lead > 0) {
			memmove(digs, digs + lead, (size_t)(ndigits - lead));
			for (long i = ndigits - lead; i < ndigits; i++) digs[i] = '0';
			E10 -= lead;
		}
	}
	long last = ndigits - 1;
	while (last > 0 && digs[last] == '0') last--;
	ndigits = last + 1;
	bvnf_shorten_digits(f, 10u, digs, &ndigits, &E10);
	size_t pos = 0;
	int32_t ret = -1;
#define PUTC(c) do { if (pos + 1 >= bufsize) goto overflow; \
					 buf[pos++] = (char)(c); } while(0)
	if (f->_sign < 0) PUTC('-');
	PUTC(digs[0]);
	if (ndigits > 1) {
		PUTC('.');
		for (long i = 1; i < ndigits; i++) PUTC(digs[i]);
	}
	PUTC('e');
	if (E10 >= 0) PUTC('+'); else { PUTC('-'); E10 = -E10; }
	{ char ebuf[32]; int en = 0;
	  if (E10 == 0) { ebuf[en++] = '0'; }
	  else {
		  long ev = E10;
		  while (ev > 0) { ebuf[en++] = (char)('0' + ev % 10); ev /= 10; }
		  for (int i = 0, j = en - 1; i < j; i++, j--) {
			  char t = ebuf[i]; ebuf[i] = ebuf[j]; ebuf[j] = t;
		  }
	  }
	  for (int i = 0; i < en; i++) PUTC(ebuf[i]);
	}
	buf[pos] = '\0';
	ret = (int32_t)pos;
overflow:
#undef PUTC
	free(digs);
	return ret;
}
/*
 * Format a bvn_float as text. Only base 10 and base 16 are supported. After
 * handling nan/inf/±0 it dispatches by base: base 16 (bvnf_to_str_pow2) reads
 * mantissa bits directly into hex digit groups with a binary 'p' exponent —
 * exact and cheap; base 10 (bvnf_to_str_dec) does the harder exact
 * binary->decimal conversion and then reduces the result to the genuinely
 * shortest digit string that still round-trips to this float at its own
 * precision. Returns -1 on a too-small buffer rather than truncating. Size the
 * buffer with bvn_float_str_bufsize.
 */
int32_t bvn_float_to_str(const bvn_float_t *f, char *buf, size_t bufsize,
						  uint32_t base)
{
	if (!f || !buf || bufsize < 2 || (base != 10u && base != 16u)) return -1;
	if (bvn_float_is_nan(f)) {
		if (bufsize < 4) return -1;
		memcpy(buf, "nan", 4);
		return 3;
	}
	if (bvn_float_is_inf(f)) {
		bool neg = f->_sign < 0;
		/* Negative infinity is "ninf" in bovnar (a bare reserved keyword);
		 * a leading "-" would not re-parse as a special-number keyword. */
		if (neg) { if (bufsize < 5) return -1; memcpy(buf, "ninf", 5); return 4; }
		else     { if (bufsize < 4) return -1; memcpy(buf, "inf", 4);  return 3; }
	}
	if (bvn_float_is_zero(f)) {
		if (bufsize < 4) return -1;
		if (f->_sign < 0) { if (bufsize < 5) return -1; memcpy(buf, "-0.0", 5); return 4; }
		memcpy(buf, "0.0", 4);
		return 3;
	}
	if (base == 16u) {
		uint32_t log2b = 0;
		bvnf_pow2_base(base, &log2b);
		return bvnf_to_str_pow2(f, buf, bufsize, log2b);
	}
	return bvnf_to_str_dec(f, buf, bufsize);
}
/*
 * Load an IEEE-754 double into a bvn_float exactly (a double is always
 * representable when prec>=53). It decomposes the bit pattern into sign, 11-bit
 * exponent and 52-bit mantissa, reconstructs the implicit leading 1 (or handles
 * subnormals), normalises so the leading bit is at the top, and places the
 * 53-bit significand into the mantissa limbs at the right offset for the target
 * precision. The negative-zero case is preserved via the 1.0/v sign test.
 * bvn_float_to_double is the inverse with round-to-nearest-even back to 53 bits.
 */
bool bvn_float_from_double(bvn_float_t *f, double v)
{
	if (!f) return false;
	if (isnan(v))  { bvn_float_set_nan(f);  return true; }
	if (isinf(v))  { bvn_float_set_inf(f, v < 0.0); return true; }
	if (v == 0.0) {
		bvn_float_set_zero(f, signbit(v) != 0);
		return true;
	}
	uint64_t bits;
	memcpy(&bits, &v, 8);
	bool neg = (bits >> 63) != 0;
	int  exp11 = (int)((bits >> 52) & 0x7ffu);
	uint64_t man = bits & ((1ull << 52) - 1ull);
	long exp2;
	uint64_t full_man;
	if (exp11 == 0) {
		full_man = man;
		exp2     = -1022L + 1L;
	} else {
		full_man = man | (1ull << 52);
		exp2     = (long)exp11 - 1023L + 1L;
	}
	int lz = 0;
	uint64_t tmp = full_man;
	while (tmp < (1ull << 52)) { tmp <<= 1; lz++; }
	full_man <<= lz;
	exp2 -= lz;
	f->_sign = neg ? -1 : 1;
	f->_exp  = exp2;
	memset(f->_d, 0, (size_t)f->_nlimbs * sizeof(bvn_limb_t));
	long prec = (long)f->_prec;   /* bounded; see bvnf_to_str_dec cast note */
	int shift = (int)prec - 53;
#if BVN_LIMB_BITS == 64
	if (shift >= 0) {
		uint32_t lo_limb = (uint32_t)shift / 64u;
		uint32_t lo_off  = (uint32_t)shift % 64u;
		if (lo_limb < f->_nlimbs)
			f->_d[lo_limb] = full_man << lo_off;
		if (lo_off > 0u && lo_limb + 1u < f->_nlimbs)
			f->_d[lo_limb + 1u] = full_man >> (64u - lo_off);
	} else {
		unsigned dnum   = (unsigned)(-shift);          /* low bits dropped */
		uint64_t kept   = full_man >> dnum;
		uint64_t guard  = (full_man >> (dnum - 1u)) & 1u;
		/* Both arms are uint64_t: a bare `!= 0u` yields int, which the usual
		 * arithmetic conversions against `0u` would turn into an unsigned
		 * conversion (-Wsign-conversion). The value is only ever 0 or 1. */
		uint64_t sticky = (dnum >= 2u)
			? (uint64_t)((full_man & (((uint64_t)1u << (dnum - 1u)) - 1u)) != 0u)
			: (uint64_t)0u;
		if (guard && (sticky || (kept & 1u))) {
			kept++;
			if (kept >> (unsigned)prec) { kept >>= 1; f->_exp += 1; }
		}
		if (f->_nlimbs > 0u) f->_d[0] = kept;
	}
#else
	if (shift >= 0) {
		uint32_t lo_limb = (uint32_t)shift / 32u;
		uint32_t lo_off  = (uint32_t)shift % 32u;
		uint64_t shifted = full_man << lo_off;
		if (lo_limb     < f->_nlimbs) f->_d[lo_limb]     = (uint32_t)(shifted & 0xffffffffu);
		if (lo_limb + 1 < f->_nlimbs) f->_d[lo_limb + 1] = (uint32_t)(shifted >> 32);
		if (lo_off > 0u && lo_limb + 2 < f->_nlimbs)
			f->_d[lo_limb + 2] = (uint32_t)(full_man >> (64u - lo_off));
	} else {
		unsigned dnum   = (unsigned)(-shift);
		uint64_t kept   = full_man >> dnum;
		uint64_t guard  = (full_man >> (dnum - 1u)) & 1u;
		/* Both arms are uint64_t: a bare `!= 0u` yields int, which the usual
		 * arithmetic conversions against `0u` would turn into an unsigned
		 * conversion (-Wsign-conversion). The value is only ever 0 or 1. */
		uint64_t sticky = (dnum >= 2u)
			? (uint64_t)((full_man & (((uint64_t)1u << (dnum - 1u)) - 1u)) != 0u)
			: (uint64_t)0u;
		if (guard && (sticky || (kept & 1u))) {
			kept++;
			if (kept >> (unsigned)prec) { kept >>= 1; f->_exp += 1; }
		}
		if (f->_nlimbs > 0) f->_d[0] = (uint32_t)(kept & 0xffffffffu);
		if (f->_nlimbs > 1) f->_d[1] = (uint32_t)(kept >> 32);
	}
#endif
	return true;
}
bool bvn_float_to_double(const bvn_float_t *f, double *out)
{
	if (!f || !out) return false;
	uint64_t bits = 0;
	bvn_float_to_bin64(f, &bits);
	memcpy(out, &bits, sizeof(double));
	return true;
}
bool bvn_float_from_float(bvn_float_t *f, float v)
{
	return bvn_float_from_double(f, (double)v);
}
bool bvn_float_to_float(const bvn_float_t *f, float *out)
{
	if (!out) return false;
	double d;
	if (!bvn_float_to_double(f, &d)) return false;
	*out = (float)d;
	return true;
}
#include "bvn_float_impl.h"
/*
 * Correctly-rounded conversion of a base-10 floating literal to double, correct
 * across the whole range -- normals, subnormals, overflow to +/-inf, underflow
 * to 0 -- and at any input length.
 *
 * It rounds the exact rational num/den straight to binary64 in a SINGLE step via
 * bvn_float_strtoieee_bin. It deliberately does NOT parse into a fixed-width
 * bvn_float and then narrow with bvn_float_to_double: that is two roundings
 * (decimal -> p bits, then p -> 53 bits) and double-rounds by 1 ULP for inputs
 * near a rounding midpoint. No fixed intermediate width p fixes this -- the
 * "2p+2" rule concerns the exact product/sum of p-bit operands, not the rounding
 * of an arbitrary-length decimal string -- so the only correct route is to round
 * the exact value once, which is what this does. NULL yields 0.0; a parse or
 * allocation failure also yields 0.0 (there is no error channel in the
 * double-returning signature). Public and externally linked so callers in other
 * translation units (e.g. the value layer) need not include bvn_float_impl.h.
 */
double bvn_float_strtod(const char *s)
{
	uint32_t b[2] = { 0u, 0u };
	if (!s) return 0.0;
	bvn_float_strtoieee_bin(s, 10u, 11u, 52u, 1023u, b, 2);
	uint64_t u = (uint64_t)b[0] | ((uint64_t)b[1] << 32);
	double out;
	memcpy(&out, &u, sizeof out);
	return out;
}
/*
 * Encode a bvn_float into an arbitrary IEEE-754-style binary interchange format
 * described by (exp_bits, man_bits, bias) — the same machinery serves binary16
 * through binary256 and the decimal-interchange helpers, just with different
 * field sizes. nan/inf are written as their reserved all-ones-exponent
 * patterns. For finite values it round-trips through the decimal string and the
 * shared to_ieee_binary packer (in bvn_float_impl.h), which handles the
 * subnormal/rounding/overflow rules for the requested field widths.
 * bvn_float_from_ieee_bin is the inverse decode. The fixed-width bin16/32/64/...
 * wrappers below just call these with the standard parameter sets.
 */
/*
 * Repack a finite, non-zero bvn_float straight into an IEEE-754 binary field set,
 * without the decimal-string detour the old path used. The value is
 * significand * 2^(_exp - _prec) with the leading mantissa bit at _prec-1, so in
 * 1.fff form its unbiased exponent is _exp-1 and the biased exponent is
 * _exp-1+bias. We keep man_bits+1 significant bits and round the discarded low
 * bits to nearest, ties to even, using one guard bit plus an exact sticky bit
 * (computed by masking off the low drop-1 bits and comparing). Subnormals fold
 * the extra right shift into the cut and clamp the biased exponent to 0; a
 * round-up that carries out of the mantissa, or an exponent that reaches the
 * all-ones code, yields Infinity. Single-rounded by construction.
 */
static void bvnf_to_ieee_bin_direct(const bvn_float_t *f,
		uint32_t exp_bits, uint32_t man_bits, int32_t bias,
		uint32_t *bits, int bits32)
{
	int eall  = (int)((1u << exp_bits) - 1u);
	int total = 1 + (int)exp_bits + (int)man_bits;
	for (int i = 0; i < bits32; i++) bits[i] = 0;
	if (f->_sign < 0)
		bits[(total - 1) / 32] |= 1u << ((total - 1) % 32);
	long be   = (long)f->_exp - 1L + (long)bias;
	long drop = (long)f->_prec - ((long)man_bits + 1L);
	if (be <= 0) { drop += (1L - be); be = 0; }
	uint32_t shint = bvnf_hint_limbs(f->_prec > (man_bits + 1u) ? f->_prec : (man_bits + 1u));
	bvn_int_t S;
	if (!bvnf_scratch(&S, shint)) return;
#if BVN_LIMB_BITS == 64
	for (uint32_t i = 0; i < f->_nlimbs; i++) {
		if (2u * i      < S.nlimbs) S.limbs[2u * i]      = (uint32_t)(f->_d[i] & 0xffffffffu);
		if (2u * i + 1u < S.nlimbs) S.limbs[2u * i + 1u] = (uint32_t)(f->_d[i] >> 32);
	}
	S.nused = (f->_nlimbs * 2u <= S.nlimbs) ? f->_nlimbs * 2u : S.nlimbs;
#else
	for (uint32_t i = 0; i < f->_nlimbs && i < S.nlimbs; i++) S.limbs[i] = f->_d[i];
	S.nused = (f->_nlimbs <= S.nlimbs) ? f->_nlimbs : S.nlimbs;
#endif
	bvn_int_norm(&S);
	bool gbit = false, sticky = false;
	if (drop > 0) {
		gbit = bvn_int_getbit(&S, (int)drop - 1) == 1;
		if (drop >= 2) {
			bvn_int_t T;
			if (bvnf_scratch(&T, shint)) {
				bvn_int_copy(&T, &S);
				bvn_int_shr(&T, (int)drop - 1);
				bvn_int_shl(&T, (int)drop - 1);
				sticky = bvn_int_cmp(&T, &S) != 0;
				bvnf_scratch_free(&T);
			}
		}
		bvn_int_shr(&S, (int)drop);
	} else if (drop < 0) {
		bvn_int_shl(&S, (int)(-drop));
	}
	if (gbit && (sticky || bvn_int_getbit(&S, 0) == 1)) {
		bvn_int_add_u32(&S, 1u);
		if (bvn_int_bitlen(&S) > (int)man_bits + 1) {
			bvn_int_shr(&S, 1);
			be++;
		}
	}
	if (be >= eall) {
		for (uint32_t i = 0; i < exp_bits; i++)
			bits[(man_bits + i) / 32] |= 1u << ((man_bits + i) % 32);
		bvnf_scratch_free(&S);
		return;
	}
	if (be == 0 && bvn_int_getbit(&S, (int)man_bits) == 1)
		be = 1;
	if (be > 0) {
		int wi = (int)man_bits / 32, b2 = (int)man_bits % 32;
		if ((uint32_t)wi < S.nlimbs) S.limbs[wi] &= ~(1u << b2);
		bvn_int_norm(&S);
		bvn_int_t eb;
		if (bvnf_scratch(&eb, bvnf_hint_limbs(man_bits + 64u))) {
			bvn_int_from_uint64(&eb, (uint64_t)be);
			bvn_int_shl(&eb, (int)man_bits);
			for (uint32_t i = 0; i < eb.nused && (int)i < bits32; i++)
				bits[i] |= eb.limbs[i];
			bvnf_scratch_free(&eb);
		}
	}
	for (uint32_t i = 0; i < S.nused && (int)i < bits32; i++)
		bits[i] |= S.limbs[i];
	bvnf_scratch_free(&S);
}
void bvn_float_to_ieee_bin(const bvn_float_t *f,
							uint32_t exp_bits, uint32_t man_bits, int32_t bias,
							uint32_t *bits, int bits32)
{
	if (!bits) return;
	for (int i = 0; i < bits32; i++) bits[i] = 0;
	if (!f) return;                                /* NULL handle -> all-zero field */
	if (!bvnf_ieee_geom_ok(exp_bits, man_bits, bits32)) return;
	if (bvn_float_is_nan(f)) {
		for (uint32_t i = 0; i < exp_bits; i++)
			bits[(man_bits + i) / 32] |= 1u << ((man_bits + i) % 32);
		if (man_bits > 0) bits[(man_bits - 1) / 32] |= 1u << ((man_bits - 1) % 32);
		if (f->_sign < 0) {
			uint32_t sp = man_bits + exp_bits;
			bits[sp / 32] |= 1u << (sp % 32);
		}
		return;
	}
	if (bvn_float_is_inf(f)) {
		for (uint32_t i = 0; i < exp_bits; i++)
			bits[(man_bits + i) / 32] |= 1u << ((man_bits + i) % 32);
		if (f->_sign < 0) {
			uint32_t sp = man_bits + exp_bits;
			bits[sp / 32] |= 1u << (sp % 32);
		}
		return;
	}
	/* Signed zero is the only finite case the direct repack does not cover. */
	if (bvn_float_is_zero(f)) {
		if (f->_sign < 0) {
			int total = 1 + (int)exp_bits + (int)man_bits;
			bits[(total - 1) / 32] |= 1u << ((total - 1) % 32);
		}
		return;
	}
	bvnf_to_ieee_bin_direct(f, exp_bits, man_bits, bias, bits, bits32);
}
bool bvn_float_from_ieee_bin(bvn_float_t *f,
							  uint32_t exp_bits, uint32_t man_bits, int32_t bias,
							  const uint32_t *bits, int bits32)
{
	if (!f || !bits) return false;
	if (!bvnf_ieee_geom_ok(exp_bits, man_bits, bits32)) return false;
	int total     = 1 + (int)exp_bits + (int)man_bits;
	uint32_t eall = (1u << exp_bits) - 1u;
	uint32_t raw_exp = 0;
	for (uint32_t i = 0; i < exp_bits; i++) {
		uint32_t bit_idx = man_bits + i;
		if ((bits[bit_idx / 32] >> (bit_idx % 32)) & 1u)
			raw_exp |= 1u << i;
	}
	uint32_t sign_idx = (uint32_t)(total - 1);
	bool neg = ((bits[sign_idx / 32] >> (sign_idx % 32)) & 1u) != 0u;
	if (raw_exp == eall) {
		bool mant_zero = true;
		for (uint32_t i = 0; i < man_bits && mant_zero; i++) {
			if ((bits[i / 32] >> (i % 32)) & 1u) mant_zero = false;
		}
		if (mant_zero) bvn_float_set_inf(f, neg);
		else           bvn_float_set_nan(f);
		return true;
	}
	if (raw_exp == 0) {
		bool mant_zero = true;
		for (uint32_t i = 0; i < man_bits && mant_zero; i++) {
			if ((bits[i / 32] >> (i % 32)) & 1u) mant_zero = false;
		}
		if (mant_zero) { bvn_float_set_zero(f, neg); return true; }
	}
	f->_sign = neg ? -1 : 1;
	long unbiased = (long)raw_exp - (long)bias;
	bvn_int_t mant;
	if (!bvnf_scratch(&mant, bvnf_hint_limbs(man_bits + 2u))) return false;
	if (raw_exp != 0) {
		bvn_int_from_uint64(&mant, 1u);
		if (!bvn_int_shl(&mant, (int)man_bits)) { bvnf_scratch_free(&mant); return false; }
	}
	for (uint32_t i = 0; i < man_bits; i++) {
		if ((bits[i / 32] >> (i % 32)) & 1u)
			if (!bvn_int_setbit(&mant, (int)i)) { bvnf_scratch_free(&mant); return false; }
	}
	if (bvn_int_is_zero(&mant)) { bvnf_scratch_free(&mant); bvn_float_set_zero(f, neg); return true; }
	int leading = bvn_int_bitlen(&mant) - 1;
	long mpfr_exp = (raw_exp != 0) ?
					(unbiased + 1L) :
					/* subnormal: exponent base is e_min = 1 - bias, not -bias */
					(unbiased - (long)man_bits + (long)leading + 2L);
	int target_top = (int)f->_prec - 1;
	int cur_top    = leading;
	int shift      = target_top - cur_top;
	uint32_t qhint = bvnf_hint_limbs(f->_prec > (man_bits + 1u) ? f->_prec : (man_bits + 1u));
	bvn_int_t Q;
	if (!bvnf_scratch(&Q, qhint)) { bvnf_scratch_free(&mant); return false; }
	bvn_int_copy(&Q, &mant);
	bvnf_scratch_free(&mant);
	if (shift > 0) {
		if (!bvn_int_shl(&Q, shift)) { bvnf_scratch_free(&Q); return false; }
	} else if (shift < 0) {
		/*
		 * Target precision is narrower than the source significand: round the
		 * dropped low bits to nearest, ties to even (mirrors bvn_float_copy).
		 * A plain truncating shift here would lose a unit in the last place.
		 */
		int  d      = -shift;
		int  guard  = bvn_int_getbit(&Q, d - 1);
		bool sticky = false;
		for (int i = 0; i < d - 1 && !sticky; i++)
			if (bvn_int_getbit(&Q, i)) sticky = true;
		bvn_int_shr(&Q, d);
		if (guard && (sticky || bvn_int_getbit(&Q, 0) == 1)) {
			if (!bvn_int_add_u32(&Q, 1u)) { bvnf_scratch_free(&Q); return false; }
			if (bvn_int_bitlen(&Q) > (int)f->_prec) {
				bvn_int_shr(&Q, 1);
				mpfr_exp += 1L;
			}
		}
	}
	f->_exp = mpfr_exp;
	bvnf_store_mant(f, &Q);
	bvnf_scratch_free(&Q);
	return true;
}
void bvn_float_to_bin16(const bvn_float_t *f, uint16_t *out)
{
	uint32_t b[1] = {0};
	if (!out) return;
	bvn_float_to_ieee_bin(f, 5, 10, 15, b, 1);
	*out = (uint16_t)b[0];
}
void bvn_float_to_bin32(const bvn_float_t *f, uint32_t *out)
{
	if (!out) return;
	bvn_float_to_ieee_bin(f, 8, 23, 127, out, 1);
}
void bvn_float_to_bin64(const bvn_float_t *f, uint64_t *out)
{
	uint32_t b[2] = {0};
	if (!out) return;
	bvn_float_to_ieee_bin(f, 11, 52, 1023, b, 2);
	*out = (uint64_t)b[0] | ((uint64_t)b[1] << 32);
}
void bvn_float_to_bin128(const bvn_float_t *f, uint32_t out[4])
{
	if (!out) return;
	bvn_float_to_ieee_bin(f, 15, 112, 16383, out, 4);
}
void bvn_float_to_bin256(const bvn_float_t *f, uint32_t out[8])
{
	if (!out) return;
	bvn_float_to_ieee_bin(f, 19, 236, 262143, out, 8);
}
bool bvn_float_from_bin16(bvn_float_t *f, uint16_t bits)
{
	uint32_t b = bits;
	return bvn_float_from_ieee_bin(f, 5, 10, 15, &b, 1);
}
bool bvn_float_from_bin32(bvn_float_t *f, uint32_t bits)
{
	return bvn_float_from_ieee_bin(f, 8, 23, 127, &bits, 1);
}
bool bvn_float_from_bin64(bvn_float_t *f, uint64_t bits)
{
	uint32_t b[2] = { (uint32_t)(bits & 0xffffffffu), (uint32_t)(bits >> 32) };
	return bvn_float_from_ieee_bin(f, 11, 52, 1023, b, 2);
}
bool bvn_float_from_bin128(bvn_float_t *f, const uint32_t bits[4])
{
	return bvn_float_from_ieee_bin(f, 15, 112, 16383, bits, 4);
}
bool bvn_float_from_bin256(bvn_float_t *f, const uint32_t bits[8])
{
	return bvn_float_from_ieee_bin(f, 19, 236, 262143, bits, 8);
}
static bool from_ieee_decimal(bvn_float_t *f,
							   int total_bits, int exp_bits, int coeff_bits,
							   int bias, int max_coeff_digs, const uint32_t *bits, int bits32)
{
	if (!f || !bits) return false;
	int sign_pos = total_bits - 1;
	bool neg = ((bits[sign_pos / 32] >> (sign_pos % 32)) & 1u) != 0u;
	bool is_std = bvnf_dec_is_standard(exp_bits, max_coeff_digs, bias);
	uint32_t raw_exp = 0;
	for (int i = 0; i < exp_bits && i < 32; i++) {
		int pos = coeff_bits + i;
		if (pos >= bits32 * 32) break;
		if ((bits[pos / 32] >> (pos % 32)) & 1u)
			raw_exp |= (1u << i);
	}
	/*
	 * Special-value detection mirrors the encoder. Standard formats use the BID
	 * combination field: top four exponent-field bits 1111, fifth bit selecting
	 * Infinity (0) or NaN (1). The in-house widths flag specials with the all-ones
	 * exponent field, telling Infinity from NaN by whether the coefficient is zero.
	 */
	if (is_std) {
		if ((raw_exp >> (exp_bits - 4)) == 0xFu) {
			if ((raw_exp >> (exp_bits - 5)) & 1u) bvn_float_set_nan(f);
			else                                  bvn_float_set_inf(f, neg);
			return true;
		}
	} else {
		uint32_t exp_max = (exp_bits < 32) ? ((1u << exp_bits) - 1u) : 0xffffffffu;
		if (raw_exp == exp_max) {
			bool coeff_zero = true;
			for (int i = 0; i < coeff_bits && coeff_zero; i++) {
				if (i >= bits32 * 32) break;
				if ((bits[i / 32] >> (i % 32)) & 1u) coeff_zero = false;
			}
			if (coeff_zero) bvn_float_set_inf(f, neg);
			else            bvn_float_set_nan(f);
			return true;
		}
	}
	int cb_words = (coeff_bits + 31) / 32;
	if (cb_words > 8) cb_words = 8;
	uint32_t cb[8] = {0};
	int64_t unbiased;
	/*
	 * CASE B (standard formats only): a "11" combination prefix that is not the
	 * 1111x special. The exponent then lives in the exp_bits field at bit
	 * tL = total-3-exp_bits, and the coefficient is the low tL bits with the
	 * implicit 2^coeff_bits top bit restored. CASE A reads the exponent from the
	 * trailing field's top and the coefficient straight from the low bits.
	 */
	if (is_std && raw_exp >= (3u << (exp_bits - 2))) {
		int tL = total_bits - 3 - exp_bits;
		uint32_t be_field = 0;
		for (int i = 0; i < exp_bits && i < 32; i++) {
			int pos = tL + i;
			if (pos >= bits32 * 32) break;
			if ((bits[pos / 32] >> (pos % 32)) & 1u)
				be_field |= (1u << i);
		}
		unbiased = (int64_t)be_field - (int64_t)bias;
		for (int i = 0; i < tL && i < 256; i++) {
			if (i >= bits32 * 32) break;
			if ((bits[i / 32] >> (i % 32)) & 1u)
				cb[i / 32] |= (1u << (i % 32));
		}
		cb[coeff_bits / 32] |= (1u << (coeff_bits % 32));
	} else {
		unbiased = (int64_t)raw_exp - (int64_t)bias;
		for (int i = 0; i < coeff_bits && i < 256; i++) {
			if (i >= bits32 * 32) break;
			if ((bits[i / 32] >> (i % 32)) & 1u)
				cb[i / 32] |= (1u << (i % 32));
		}
	}
	bool coeff_zero = true;
	for (int i = 0; i < cb_words && coeff_zero; i++)
		if (cb[i]) coeff_zero = false;
	if (coeff_zero) { bvn_float_set_zero(f, neg); return true; }
	uint32_t tmp[8];
	for (int i = 0; i < cb_words; i++) tmp[i] = cb[i];
	bvn_int_t T;
	T.limbs    = tmp;
	T.nlimbs   = (uint32_t)cb_words;
	T.negative = false;
	T.heap     = false;
	T.nused    = 0;
	for (int i = cb_words - 1; i >= 0; i--) {
		if (tmp[i]) { T.nused = (uint32_t)(i + 1); break; }
	}
	char rev[256];
	int  rlen = 0;
	while (!bvn_int_is_zero(&T) && rlen < 255) {
		uint32_t rem = bvn_int_div_u32(&T, 10u);
		rev[rlen++]  = (char)('0' + rem);
	}
	/*
	 * Reject non-canonical significands. A coefficient with more than
	 * max_coeff_digs digits cannot occur in a well-formed encoding (the encoder
	 * never emits one), but a coefficient field wider than the digit limit can
	 * carry it in malformed input — notably decimal16, whose 9-bit field holds
	 * 0..511 while only 0..99 is canonical. IEEE 754 (§3.5.2) interprets such an
	 * encoding as zero; doing so also keeps from_dec* a left inverse of to_dec*
	 * on every value to_dec* can produce.
	 */
	if (rlen > max_coeff_digs) { bvn_float_set_zero(f, neg); return true; }
	char final_str[320];
	int  flen = 0;
	if (neg) final_str[flen++] = '-';
	for (int i = rlen - 1; i >= 0; i--)
		final_str[flen++] = rev[i];
	int n = snprintf(final_str + flen, sizeof(final_str) - (size_t)flen,
					 "e%" PRId64, unbiased);
	if (n < 0 || (size_t)n >= sizeof(final_str) - (size_t)flen) return false;
	flen += n;
	final_str[flen] = '\0';
	return bvn_float_from_str(f, final_str, 10);
}
/*
 * Load a regular float's mantissa limbs into a 32-bit bvn_int (repacking from the
 * platform limb width). The result is the bare integer significand M; the value
 * of the float is M * 2^(_exp - _prec) with M's top bit at position _prec-1.
 */
static void bvnf_load_mant_bvint(const bvn_float_t *f, bvn_int_t *M)
{
#if BVN_LIMB_BITS == 64
	for (uint32_t i = 0; i < f->_nlimbs; i++) {
		if (2u * i      < M->nlimbs) M->limbs[2u * i]      = (uint32_t)(f->_d[i] & 0xffffffffu);
		if (2u * i + 1u < M->nlimbs) M->limbs[2u * i + 1u] = (uint32_t)(f->_d[i] >> 32);
	}
	M->nused = (f->_nlimbs * 2u <= M->nlimbs) ? f->_nlimbs * 2u : M->nlimbs;
#else
	for (uint32_t i = 0; i < f->_nlimbs && i < M->nlimbs; i++) M->limbs[i] = f->_d[i];
	M->nused = (f->_nlimbs <= M->nlimbs) ? f->_nlimbs : M->nlimbs;
#endif
	bvn_int_norm(M);
}
/*
 * Fixed-point (Qm.n) conversion done directly from the binary mantissa with a
 * single round-to-nearest-even — no decimal round-trip. The value is
 * M * 2^(_exp - _prec); scaling by 2^frac_bits gives M * 2^(_exp - _prec +
 * frac_bits). A non-negative shift is exact (no rounding); a negative shift
 * drops |sh| low bits, rounded to nearest with ties to even using one guard bit
 * and an exact sticky. The result is two's-complemented into total_bits and the
 * top word masked/sign-extended to the field width, matching the prior decoder's
 * overflow-wrap semantics. Routing this straight off the mantissa avoids the
 * double rounding that a float -> shortest-decimal-string -> round path incurs.
 */
/*
 * Core conversion. Computes M = round(|value| * 2^frac_bits) and writes the
 * signed two's-complement fixed-point datum into bits[]. A value outside the
 * representable signed range [-2^(total_bits-1), 2^(total_bits-1)-1] is an
 * overflow:
 *   - reported through *overflow_out (when non-NULL), and
 *   - if `saturate`, clamped to the nearest representable extreme (+max for a
 *     positive overflow, -2^(total_bits-1) for a negative one) instead of the
 *     historical low-bits wrap, so a too-large datum can never silently decode
 *     to an unrelated value. When `saturate` is false the function is a pure
 *     range probe: it sets *overflow_out and leaves bits[] zeroed.
 */
static void bvnf_to_fix_core(const bvn_float_t *f, int total_bits,
							  uint32_t frac_bits, uint32_t *bits, int bits32,
							  bool saturate, bool *overflow_out)
{
	if (overflow_out) *overflow_out = false;
	for (int i = 0; i < bits32; i++) bits[i] = 0;
	if (!bvn_float_is_regular(f)) return;          /* nan/inf/zero -> 0 */
	bool neg = f->_sign < 0;
	uint32_t qhint = bvnf_hint_limbs(f->_prec + frac_bits + 64u);
	bvn_int_t M;
	if (!bvnf_scratch(&M, qhint)) return;
	bvnf_load_mant_bvint(f, &M);
	long sh = (long)f->_exp - (long)f->_prec + (long)frac_bits;
	/* A magnitude past the field is an overflow whatever its low bits are,
	 * and is decided before the shift: a shift that large can fail (or would
	 * need megabytes), and failing it used to return 0 -- neither saturated
	 * nor reported, so 1e9862 under q8 passed the range probe. */
	bool too_big = (sh >= 0) &&
		((long)bvn_int_bitlen(&M) + sh > (long)total_bits);
	if (too_big) {
		bvn_int_zero(&M);
	} else if (sh >= 0) {
		if (sh > (long)INT_MAX || !bvn_int_shl(&M, (int)sh)) {
			bvnf_scratch_free(&M); return;
		}
	} else {
		long d   = -sh;
		int  mlen = bvn_int_bitlen(&M);
		if (d > (long)mlen) {                      /* |value*2^frac| < 1/2 */
			bvn_int_zero(&M);
		} else {
			int  di     = (int)d;
			int  guard  = bvn_int_getbit(&M, di - 1);
			bool sticky = false;
			for (int i = 0; i < di - 1 && !sticky; i++)
				if (bvn_int_getbit(&M, i)) sticky = true;
			bvn_int_shr(&M, di);
			if (guard && (sticky || bvn_int_getbit(&M, 0) == 1)) {
				if (!bvn_int_add_u32(&M, 1u)) { bvnf_scratch_free(&M); return; }
			}
		}
	}
	/*
	 * Overflow check against the signed range. Positive magnitude is
	 * representable up to 2^(total_bits-1)-1 (bitlen <= total_bits-1);
	 * negative up to 2^(total_bits-1) (bitlen == total_bits with only the
	 * top bit set).
	 */
	{
		int  bl = bvn_int_bitlen(&M);
		bool ov;
		if (too_big) {
			ov = true;
		} else if (!neg) {
			ov = (bl >= total_bits);
		} else if (bl < total_bits) {
			ov = false;
		} else if (bl > total_bits) {
			ov = true;
		} else {
			ov = false;
			for (int i = 0; i < total_bits - 1; i++)
				if (bvn_int_getbit(&M, i)) { ov = true; break; }
		}
		if (ov) {
			if (overflow_out) *overflow_out = true;
			if (!saturate) { bvnf_scratch_free(&M); return; }
			bvn_int_zero(&M);
			if (!neg) {
				for (int i = 0; i < total_bits - 1; i++)
					if (!bvn_int_setbit(&M, i)) {
						bvnf_scratch_free(&M); return;
					}
			} else {
				if (!bvn_int_setbit(&M, total_bits - 1)) {
					bvnf_scratch_free(&M); return;
				}
			}
		} else if (!saturate) {
			/* probe only: no overflow, nothing to emit */
			bvnf_scratch_free(&M); return;
		}
	}
	int w = (total_bits + 31) / 32;
	if (w > bits32) w = bits32;
	for (int i = 0; i < w; i++)
		bits[i] = (i < (int)M.nused) ? M.limbs[i] : 0u;
	if (neg) {
		for (int i = 0; i < w; i++) bits[i] = ~bits[i];
		uint64_t carry = 1;
		for (int i = 0; i < w && carry; i++) {
			uint64_t s = (uint64_t)bits[i] + carry;
			bits[i] = (uint32_t)s;
			carry   = s >> 32;
		}
	}
	if (total_bits % 32 != 0 && w > 0) {
		uint32_t mask = ((uint32_t)1 << (total_bits % 32)) - 1u;
		if (neg && (bits[w - 1] & ((uint32_t)1 << ((total_bits % 32) - 1))))
			bits[w - 1] |= ~mask;
		else
			bits[w - 1] &= mask;
	}
	bvnf_scratch_free(&M);
}
static void bvnf_to_fix_direct(const bvn_float_t *f, int total_bits,
								uint32_t frac_bits, uint32_t *bits, int bits32)
{
	bvnf_to_fix_core(f, total_bits, frac_bits, bits, bits32,
					 true /*saturate*/, NULL);
}
/*
 * Range probe: does `value * 2^frac_bits` round to a datum that fits the signed
 * `total_bits`-bit Q-format without overflow? nan/inf/zero map to 0 and so
 * always "fit" (special numbers are range-exempt by spec §6.4). Used by the
 * reader and writer to reject an out-of-range fixed-point value rather than let
 * it saturate or wrap.
 */
bool bvn_float_fix_in_range(const bvn_float_t *f,
							uint32_t total_bits, uint32_t frac_bits)
{
	if (!f || !bvn_float_is_regular(f)) return true;
	uint32_t tmp[8];
	bool ov = false;
	bvnf_to_fix_core(f, (int)total_bits, frac_bits, tmp,
					 (int)(sizeof tmp / sizeof tmp[0]), false /*probe*/, &ov);
	return !ov;
}
/*
 * Convenience: parse a NUL-terminated numeric literal in `base` and report
 * whether it fits the signed total_bits/frac_bits Q-format. Allocation failure
 * and parse failure both return true (don't reject a value the caller already
 * validated syntactically). total_bits is one of 16/32/64/128/256.
 */
/*
 * Decimal magnitude of a base-10 literal without parsing it: *dexp = D such
 * that |v| lies in [10^(D-1), 10^D). Returns 0 for a zero, 1 when D is set,
 * -1 when the text is not a plain decimal literal (the caller then parses).
 * The exponent saturates far beyond any field, so "1e-99999" costs a scan.
 */
static int bvnf_dec_magnitude(const char *s, long *dexp)
{
	const char *p = s;
	if (*p == '+' || *p == '-') p++;
	long ndig_int = 0, first_nz = -1, pos = 0;
	bool any = false, dot = false;
	for (;; p++) {
		if (*p >= '0' && *p <= '9') {
			any = true;
			if (*p != '0' && first_nz < 0) first_nz = pos;
			pos++;
			if (!dot) ndig_int++;
			if (pos > 1000000L) return -1;
		} else if (*p == '.' && !dot) {
			dot = true;
		} else {
			break;
		}
	}
	if (!any) return -1;
	long e = 0;
	if (*p == 'e' || *p == 'E') {
		p++;
		bool eneg = false;
		if (*p == '+' || *p == '-') eneg = (*p++ == '-');
		if (*p < '0' || *p > '9') return -1;
		for (; *p >= '0' && *p <= '9'; p++)
			if (e < 100000000L) e = e * 10 + (*p - '0');
		if (eneg) e = -e;
	}
	if (*p != '\0') return -1;
	if (first_nz < 0) return 0;
	*dexp = ndig_int - first_nz + e;
	return 1;
}
bool bvn_float_str_fits_fix(const char *s, uint32_t base,
							uint32_t total_bits, uint32_t frac_bits)
{
	if (!s) return true;
	/* Decide the clear cases from the digits alone. Parsing "1e-9000" to a
	 * float of this precision costs milliseconds, and a document of such
	 * values was a denial of service; a value that far below 2^-frac_bits
	 * rounds to 0 and fits, one far above 2^total_bits cannot. The margins
	 * (log2 10 < 3.33, and four bits either way) leave every borderline
	 * case to the exact path below. */
	if (base == 10u) {
		long D;
		int m = bvnf_dec_magnitude(s, &D);
		if (m == 0) return true;
		if (m == 1) {
			if (D < 0 && -(D * 3) > (long)frac_bits + 4L) return true;
			if (D > 1 && (D - 1) * 3 > (long)total_bits + 4L) return false;
		}
	}
	uint32_t prec = total_bits + frac_bits + 64u;
	if (prec > BVN_FLOAT_MAX_PREC) prec = BVN_FLOAT_MAX_PREC;
	bvn_float_t *f = bvn_float_alloc(prec);
	if (!f) return true;
	bool fits = true;
	if (bvn_float_from_str(f, s, base)) {
		/* The callers pass no special number, so an infinity here is a
		 * finite literal past the float's exponent range: out of range,
		 * not the range-exempt inf of §6.4. */
		fits = !bvn_float_is_inf(f) &&
		       bvn_float_fix_in_range(f, total_bits, frac_bits);
	}
	bvn_float_free(f);
	return fits;
}
/* Number of decimal digits in a (small) big integer. -1 on allocation failure. */
static int bvnf_dec_digit_count(const bvn_int_t *b)
{
	if (bvn_int_is_zero(b)) return 0;
	bvn_int_t t;
	if (!bvnf_scratch(&t, b->nused + 2u)) return -1;
	if (!bvn_int_copy(&t, b)) { bvnf_scratch_free(&t); return -1; }
	int d = 0;
	while (!bvn_int_is_zero(&t)) { bvn_int_div_u32(&t, 10u); d++; }
	bvnf_scratch_free(&t);
	return d;
}
/*
 * Render a regular, non-zero float to exactly D significant decimal digits using
 * round-to-ODD (von Neumann / sticky rounding). The exact value M * 2^e2 is
 * formed as an exact rational num/den, scaled by 10^s so the quotient carries a
 * few more than D digits, divided once (the remainder is an exact inexact flag),
 * trimmed to D digits, and — when inexact — its last digit forced odd. Feeding a
 * round-to-odd D-digit value into to_ieee_decimal's final round-to-nearest-even
 * is provably free of double-rounding error, so the packed result is the
 * correctly-rounded narrowing of the *exact* float, not of a shortest string.
 * Writes "[-]digits e exponent" into out. false on overflow/alloc failure, which
 * makes the caller fall back to the (non-correctly-rounded but safe) string path.
 */
static bool bvnf_dec_render_roundodd(const bvn_float_t *f, int D,
									  char *out, size_t outsz)
{
	long e2  = (long)f->_exp - (long)f->_prec;
	long ep  = (long)f->_exp - 1L;
	/*
	 * Estimate the decimal exponent E10 ~= ep*log10(2). The scale s below offsets
	 * by it so the quotient carries a few more than D digits; if E10est OVER-
	 * estimates the true exponent the quotient comes out SHORT and digits are
	 * silently lost. The old 302/1000 (=0.302) approximation of log10(2) overshot
	 * by ~1e-3 per unit, which past ~2000 binary digits of exponent exceeded the
	 * +2 safety margin and truncated full-coefficient values near a format's Emax
	 * (e.g. decimal128's 34-nine maximum rendered to ~20 digits). 30103/100000
	 * (=0.30103) is within 5e-9 of log10(2), keeping E10est within 1 ulp of the
	 * truth across the whole exponent range. The multiply is done in 64-bit to
	 * stay exact for the largest representable exponents.
	 */
	long E10est = (ep >= 0)
		? (long)(((int64_t)ep * 30103 + 99999) / 100000)
		: -(long)(((int64_t)(-ep) * 30103) / 100000);
	long s = (long)D + 2L - E10est;          /* guarantees quotient has > D digits */
	long mag = (long)f->_prec + (e2 < 0 ? -e2 : e2) + (s < 0 ? -s : s) * 4L + 64L;
	if (mag < 64L) mag = 64L;
	uint32_t hint = bvnf_hint_limbs((uint32_t)mag);
	bvn_int_t num, den, Q, R;
	if (!bvnf_scratch(&num, hint)) return false;
	if (!bvnf_scratch(&den, hint)) { bvnf_scratch_free(&num); return false; }
	bvnf_load_mant_bvint(f, &num);
	bvn_int_from_uint64(&den, 1u);
	bool ok = true;
	if      (e2 > 0) ok = (e2 <= (long)INT_MAX) && bvn_int_shl(&num, (int)e2);
	else if (e2 < 0) ok = (-e2 <= (long)INT_MAX) && bvn_int_shl(&den, (int)(-e2));
	if (ok) {
		if      (s > 0) ok = (s <= (long)INT_MAX) && bvn_int_mul_pow10(&num, (int)s);
		else if (s < 0) ok = (-s <= (long)INT_MAX) && bvn_int_mul_pow10(&den, (int)(-s));
	}
#define RRO_FAIL() do { bvnf_scratch_free(&num); bvnf_scratch_free(&den); return false; } while (0)
	if (!ok) RRO_FAIL();
	if (!bvnf_scratch(&Q, hint)) RRO_FAIL();
	if (!bvnf_scratch(&R, hint)) { bvnf_scratch_free(&Q); RRO_FAIL(); }
	ok = bvn_int_divrem(&Q, &R, &num, &den);
	bool inexact = ok && !bvn_int_is_zero(&R);
	bvnf_scratch_free(&num);
	bvnf_scratch_free(&den);
	if (!ok) { bvnf_scratch_free(&Q); bvnf_scratch_free(&R); return false; }
	bvnf_scratch_free(&R);
#undef RRO_FAIL
	int g0 = bvnf_dec_digit_count(&Q);
	if (g0 < 0) { bvnf_scratch_free(&Q); return false; }
	long E10 = (long)g0 - 1L - s;            /* decimal exponent of the leading digit */
	int  g = g0;
	while (g > D) {
		uint32_t r = bvn_int_div_u32(&Q, 10u);
		if (r) inexact = true;
		g--;
	}
	if (inexact) {                            /* force last digit odd */
		bvn_int_t c;
		if (!bvnf_scratch(&c, Q.nused + 2u)) { bvnf_scratch_free(&Q); return false; }
		if (!bvn_int_copy(&c, &Q)) { bvnf_scratch_free(&c); bvnf_scratch_free(&Q); return false; }
		uint32_t last = bvn_int_div_u32(&c, 10u);
		bvnf_scratch_free(&c);
		if ((last & 1u) == 0u) {
			if (!bvn_int_add_u32(&Q, 1u)) { bvnf_scratch_free(&Q); return false; }
		}
	}
	char tmp[96];
	int  tn = 0;
	if (bvn_int_is_zero(&Q)) {
		tmp[tn++] = '0';
	} else {
		bvn_int_t c;
		if (!bvnf_scratch(&c, Q.nused + 2u)) { bvnf_scratch_free(&Q); return false; }
		if (!bvn_int_copy(&c, &Q)) { bvnf_scratch_free(&c); bvnf_scratch_free(&Q); return false; }
		while (!bvn_int_is_zero(&c) && tn < (int)sizeof(tmp))
			tmp[tn++] = (char)('0' + bvn_int_div_u32(&c, 10u));
		bvnf_scratch_free(&c);
	}
	bvnf_scratch_free(&Q);
	size_t pos = 0;
	if (f->_sign < 0) { if (pos + 1 >= outsz) return false; out[pos++] = '-'; }
	for (int i = tn - 1; i >= 0; i--) { if (pos + 1 >= outsz) return false; out[pos++] = tmp[i]; }
	if (pos + 1 >= outsz) return false;
	out[pos++] = 'e';
	long expo = E10 - (long)(tn - 1);        /* value = (tn-digit integer) * 10^expo */
	long ev   = expo;
	if (ev < 0) { if (pos + 1 >= outsz) return false; out[pos++] = '-'; ev = -ev; }
	else        { if (pos + 1 >= outsz) return false; out[pos++] = '+'; }
	char eb[24];
	int  en = 0;
	if (ev == 0) eb[en++] = '0';
	else while (ev > 0) { eb[en++] = (char)('0' + ev % 10); ev /= 10; }
	for (int i = en - 1; i >= 0; i--) { if (pos + 1 >= outsz) return false; out[pos++] = eb[i]; }
	out[pos] = '\0';
	return true;
}
/*
 * Encode a float into an IEEE-754 decimal interchange field set. Regular values
 * are rendered to max_coeff_digs+4 significant digits with round-to-odd (see
 * bvnf_dec_render_roundodd) and packed by to_ieee_decimal, which performs the
 * single, format-aware round-to-nearest-even at the target width. nan/inf/zero
 * are handed to the packer directly. On any overflow/allocation failure of the
 * exact-render path it falls back to the shortest-string rendering.
 */
static void bvnf_encode_dec(const bvn_float_t *f, const DecFmt *fmt,
							uint32_t *bits, int bits32)
{
	bvn_float_ctx_t ctx; bvn_float_ctx_init(&ctx);
	PNum p; bvn_float_pnum_init(&p);
	char sbuf[160];
	bool built = false;
	if (bvn_float_is_nan(f)) {
		p.nan = true;  p.neg = (f->_sign < 0); built = true;
	} else if (bvn_float_is_inf(f)) {
		p.inf = true;  p.neg = (f->_sign < 0); built = true;
	} else if (bvn_float_is_zero(f)) {
		p.neg = (f->_sign < 0); built = true;          /* coeff stays zero */
	} else if (bvnf_dec_render_roundodd(f, fmt->max_coeff_digs + 4, sbuf, sizeof sbuf)) {
		bvn_float_parse(&ctx, sbuf, &p);
		built = true;
	}
	if (!built) {
		size_t bsz = bvn_float_str_bufsize((uint32_t)f->_prec, 10u);
		char  *buf = malloc(bsz);
		if (buf) {
			int n = bvn_float_to_str(f, buf, bsz, 10);
			if (n > 0) bvn_float_parse(&ctx, buf, &p);
			free(buf);
		}
	}
	to_ieee_decimal(&ctx, &p, fmt, bits, bits32);
}
/*
 * IEEE-754 *decimal* interchange encoders (decimal16/32/64/128/256). The source
 * float is rounded once, directly from its exact binary value, to the format's
 * decimal coefficient/exponent by bvnf_encode_dec + to_ieee_decimal. (An earlier
 * implementation rendered the shortest round-tripping decimal first and then
 * rounded that to the coefficient width, which double-rounded and could be off
 * by a unit in the last place; bvnf_encode_dec's round-to-odd render removes it.)
 */
void bvn_float_to_dec16(const bvn_float_t *f, uint16_t *out)
{
	if (!f || !out) return;
	uint32_t b[1] = {0};
	DecFmt fmt = {16, 6, 9, 24, 2};
	bvnf_encode_dec(f, &fmt, b, 1);
	*out = (uint16_t)b[0];
}
void bvn_float_to_dec32(const bvn_float_t *f, uint32_t *out)
{
	if (!f || !out) return;
	DecFmt fmt = {32, 8, 23, 101, 7};
	bvnf_encode_dec(f, &fmt, out, 1);
}
/*
 * Decimal string -> IEEE-754 DECIMAL interchange, in a single rounding.
 *
 * The counterpart to bvn_float_strtoieee_bin, which the public API had for the
 * binary formats but not for the decimal ones. Without it a caller had to go
 * bvn_float_from_str() -> bvn_float_to_dec*(), i.e. through a BINARY
 * intermediate -- and a decimal tie is in general not exactly representable in
 * binary, so it lands just above or below and the second rounding follows that
 * instead of round-half-even. Measured over 4000 exact decimal64 ties: the
 * two-step path disagrees with IEEE roundTiesToEven on 26.4 % of them (and not
 * consistently up or down -- it is whichever side the binary landed on), while
 * this direct path is exact on all 4000.
 *
 * width is 16, 32, 64, 128 or 256; bits32 is the number of uint32_t words in
 * out (1, 1, 2, 4, 8 respectively). Returns false for an unsupported width, a
 * short buffer or text that is not a number, leaving *out untouched.
 */
bool bvn_float_strtodec(const char *s, uint32_t width, uint32_t *out, int bits32)
{
	if (!s || !out || bits32 < 1)
		return false;
	/* The encoders below pack whatever the parser left and do not look at its
	 * verdict, so text that is not a number came back as true with all-zero
	 * bits: "abc" and "" were +0, and so was "ninf" -- the format's own
	 * spelling of negative infinity -- until bvn_float_parse_ro learned it.
	 * Ask once here, where there is a result to return. */
	{
		bvn_float_ctx_t ctx; bvn_float_ctx_init(&ctx);
		PNum p; bvn_float_pnum_init(&p);
		if (!bvn_float_parse_ro(&ctx, s, &p))
			return false;
	}
	switch (width) {
	case 16:  if (bits32 < 1) return false;
		  out[0] = bvn_float_parse_dec16(s);  return true;
	case 32:  if (bits32 < 1) return false;
		  out[0] = bvn_float_parse_dec32(s);  return true;
	case 64: {
		if (bits32 < 2) return false;
		uint64_t v = bvn_float_parse_dec64(s);
		out[0] = (uint32_t)(v & 0xffffffffu);
		out[1] = (uint32_t)(v >> 32);
		return true;
	}
	case 128: if (bits32 < 4) return false;
		  bvn_float_parse_dec128(s, out); return true;
	case 256: if (bits32 < 8) return false;
		  bvn_float_parse_dec256(s, out); return true;
	default:  return false;
	}
}
void bvn_float_to_dec64(const bvn_float_t *f, uint64_t *out)
{
	if (!f || !out) return;
	uint32_t b[2] = {0, 0};
	DecFmt fmt = {64, 10, 53, 398, 16};
	bvnf_encode_dec(f, &fmt, b, 2);
	*out = (uint64_t)b[0] | ((uint64_t)b[1] << 32);
}
void bvn_float_to_dec128(const bvn_float_t *f, uint32_t out[4])
{
	if (!f || !out) return;
	DecFmt fmt = {128, 14, 113, 6176, 34};
	bvnf_encode_dec(f, &fmt, out, 4);
}
void bvn_float_to_dec256(const bvn_float_t *f, uint32_t out[8])
{
	if (!f || !out) return;
	DecFmt fmt = {256, 20, 235, 611867, 70};
	bvnf_encode_dec(f, &fmt, out, 8);
}
bool bvn_float_from_dec16(bvn_float_t *f, uint16_t bits)
{
	uint32_t b = bits;
	return from_ieee_decimal(f, 16, 6, 9, 24, 2, &b, 1);
}
bool bvn_float_from_dec32(bvn_float_t *f, uint32_t bits)
{
	return from_ieee_decimal(f, 32, 8, 23, 101, 7, &bits, 1);
}
bool bvn_float_from_dec64(bvn_float_t *f, uint64_t bits)
{
	uint32_t b[2] = { (uint32_t)(bits & 0xffffffffu), (uint32_t)(bits >> 32) };
	return from_ieee_decimal(f, 64, 10, 53, 398, 16, b, 2);
}
bool bvn_float_from_dec128(bvn_float_t *f, const uint32_t bits[4])
{
	return from_ieee_decimal(f, 128, 14, 113, 6176, 34, bits, 4);
}
bool bvn_float_from_dec256(bvn_float_t *f, const uint32_t bits[8])
{
	return from_ieee_decimal(f, 256, 20, 235, 611867, 70, bits, 8);
}
/*
 * Fixed-point (Qm.n) encoders/decoders: represent the value as an integer
 * scaled by 2^frac_bits. Like the IEEE helpers they round-trip through the
 * decimal string so the scaling and rounding are done by one shared, exact
 * routine rather than re-implemented per width. Used for the float_fix family.
 */
int16_t bvn_float_to_fix16(const bvn_float_t *f, uint32_t frac_bits)
{
	if (!f) return 0;
	uint32_t b[1] = {0};
	bvnf_to_fix_direct(f, 16, frac_bits, b, 1);
	return (int16_t)b[0];
}
int32_t bvn_float_to_fix32(const bvn_float_t *f, uint32_t frac_bits)
{
	if (!f) return 0;
	uint32_t b[1] = {0};
	bvnf_to_fix_direct(f, 32, frac_bits, b, 1);
	return (int32_t)b[0];
}
int64_t bvn_float_to_fix64(const bvn_float_t *f, uint32_t frac_bits)
{
	if (!f) return 0;
	uint32_t b[2] = {0, 0};
	bvnf_to_fix_direct(f, 64, frac_bits, b, 2);
	return (int64_t)((uint64_t)b[0] | ((uint64_t)b[1] << 32));
}
void bvn_float_to_fix128(const bvn_float_t *f, uint32_t frac_bits, uint32_t out[4])
{
	if (!f || !out) return;
	bvnf_to_fix_direct(f, 128, frac_bits, out, 4);
}
void bvn_float_to_fix256(const bvn_float_t *f, uint32_t frac_bits, uint32_t out[8])
{
	if (!f || !out) return;
	bvnf_to_fix_direct(f, 256, frac_bits, out, 8);
}
static bool from_fix_common(bvn_float_t *f, bool neg,
							 const uint32_t *abs_words, int nwords,
							 uint32_t frac_bits)
{
	if (!f) return false;
	if (nwords < 1 || nwords > 8) return false;
	bool is_zero = true;
	for (int i = 0; i < nwords && is_zero; i++)
		if (abs_words[i]) is_zero = false;
	if (is_zero) { bvn_float_set_zero(f, neg); return true; }
	uint32_t _mb[8];
	for (int i = 0; i < 8; i++) _mb[i] = (i < nwords) ? abs_words[i] : 0u;
	bvn_int_t M;
	M.limbs    = _mb;
	M.nlimbs   = 8;
	M.negative = false;
	M.heap     = false;
	M.nused    = 0;
	for (int i = 7; i >= 0; i--) {
		if (_mb[i]) { M.nused = (uint32_t)(i + 1); break; }
	}
	int leading = bvn_int_bitlen(&M) - 1;
	long mpfr_exp = (long)leading + 1L - (long)frac_bits;
	uint32_t qhint = bvnf_hint_limbs(f->_prec > 257u ? f->_prec : 257u);
	bvn_int_t Q;
	if (!bvnf_scratch(&Q, qhint)) return false;
	bvn_int_copy(&Q, &M);
	int shift = (int)f->_prec - 1 - leading;
	if (shift > 0) {
		if (!bvn_int_shl(&Q, shift)) { bvnf_scratch_free(&Q); return false; }
	} else if (shift < 0) {
		/* Narrowing: round dropped low bits to nearest, ties to even. */
		int  d      = -shift;
		int  guard  = bvn_int_getbit(&Q, d - 1);
		bool sticky = false;
		for (int i = 0; i < d - 1 && !sticky; i++)
			if (bvn_int_getbit(&Q, i)) sticky = true;
		bvn_int_shr(&Q, d);
		if (guard && (sticky || bvn_int_getbit(&Q, 0) == 1)) {
			if (!bvn_int_add_u32(&Q, 1u)) { bvnf_scratch_free(&Q); return false; }
			if (bvn_int_bitlen(&Q) > (int)f->_prec) {
				bvn_int_shr(&Q, 1);
				mpfr_exp += 1L;
			}
		}
	}
	f->_sign = neg ? -1 : 1;
	f->_exp  = mpfr_exp;
	bvnf_store_mant(f, &Q);
	bvnf_scratch_free(&Q);
	return true;
}
bool bvn_float_from_fix16(bvn_float_t *f, int16_t bits, uint32_t frac_bits)
{
	if (!f) return false;
	bool neg = (bits < 0);
	uint32_t abs_v = neg ? ((uint32_t)(-(int32_t)bits)) : (uint32_t)bits;
	return from_fix_common(f, neg, &abs_v, 1, frac_bits);
}
bool bvn_float_from_fix32(bvn_float_t *f, int32_t bits, uint32_t frac_bits)
{
	if (!f) return false;
	bool neg = (bits < 0);
	uint32_t abs_v = neg ? ((uint32_t)(-(int64_t)bits)) : (uint32_t)bits;
	return from_fix_common(f, neg, &abs_v, 1, frac_bits);
}
bool bvn_float_from_fix64(bvn_float_t *f, int64_t bits, uint32_t frac_bits)
{
	if (!f) return false;
	bool neg = (bits < 0);
	uint64_t abs_v;
	if (bits == INT64_MIN) abs_v = (uint64_t)INT64_MAX + 1ULL;
	else abs_v = (uint64_t)(neg ? -bits : bits);
	uint32_t w[2] = { (uint32_t)(abs_v & 0xffffffffu), (uint32_t)(abs_v >> 32) };
	return from_fix_common(f, neg, w, 2, frac_bits);
}
bool bvn_float_from_fix128(bvn_float_t *f, const uint32_t bits[4],
							uint32_t frac_bits)
{
	if (!f || !bits) return false;
	bool neg = ((bits[3] >> 31) & 1u) != 0u;
	uint32_t w[4];
	if (neg) {
		uint64_t carry = 1;
		for (int i = 0; i < 4; i++) {
			uint64_t v = (uint64_t)(~bits[i]) + carry;
			w[i] = (uint32_t)v; carry = v >> 32;
		}
	} else {
		for (int i = 0; i < 4; i++) w[i] = bits[i];
	}
	return from_fix_common(f, neg, w, 4, frac_bits);
}
bool bvn_float_from_fix256(bvn_float_t *f, const uint32_t bits[8],
							uint32_t frac_bits)
{
	if (!f || !bits) return false;
	bool neg = ((bits[7] >> 31) & 1u) != 0u;
	uint32_t w[8];
	if (neg) {
		uint64_t carry = 1;
		for (int i = 0; i < 8; i++) {
			uint64_t v = (uint64_t)(~bits[i]) + carry;
			w[i] = (uint32_t)v; carry = v >> 32;
		}
	} else {
		for (int i = 0; i < 8; i++) w[i] = bits[i];
	}
	return from_fix_common(f, neg, w, 8, frac_bits);
}

