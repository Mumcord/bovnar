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

#include "bvn_lexer_impl.h"
/*
 * ===========================================================================
 * Lexer transition tables (the "grammar as data")
 * ===========================================================================
 *
 * This file is pure data backing the table-driven lexer in bovnar_lexer.c. It
 * is kept separate from the engine so the grammar can be reviewed and modified
 * as tables without touching the interpreter, and so the large rodata lives in
 * its own translation unit.
 *
 *   bvn_after_state_idx_table[state][byte]  -> ACT_* action index
 *   bvn_action_table[ACT_*]                 -> handler function pointer
 *   bvn_action_target_state[ACT_*]          -> next state for pure transitions
 *   bvn_kw_advance_state[state]             -> next state for keyword matching
 *
 * The BVN_EACH_256 / BVN_WHITESPACE / BVN_UTF8_* / BVN_REJECT_ASCII_CTRL
 * macros are designated-initialiser helpers that fill whole byte ranges of a
 * row at once: e.g. one macro marks every UTF-8 continuation byte, another
 * rejects every disallowed ASCII control byte. This keeps each state's row
 * declarative — you list the bytes that matter and a macro supplies the
 * uniform default for the rest — and guarantees all 256 columns are defined.
 * An action index of 0 (ACT_NONE) means "byte illegal in this state" and is
 * what the engine turns into error_unexpected_input_byte.
 */
#define BVN_EACH_256(a) \
	[0x00]=(a),[0x01]=(a),[0x02]=(a),[0x03]=(a), \
	[0x04]=(a),[0x05]=(a),[0x06]=(a),[0x07]=(a), \
	[0x08]=(a),[0x09]=(a),[0x0a]=(a),[0x0b]=(a), \
	[0x0c]=(a),[0x0d]=(a),[0x0e]=(a),[0x0f]=(a), \
	[0x10]=(a),[0x11]=(a),[0x12]=(a),[0x13]=(a), \
	[0x14]=(a),[0x15]=(a),[0x16]=(a),[0x17]=(a), \
	[0x18]=(a),[0x19]=(a),[0x1a]=(a),[0x1b]=(a), \
	[0x1c]=(a),[0x1d]=(a),[0x1e]=(a),[0x1f]=(a), \
	[0x20]=(a),[0x21]=(a),[0x22]=(a),[0x23]=(a), \
	[0x24]=(a),[0x25]=(a),[0x26]=(a),[0x27]=(a), \
	[0x28]=(a),[0x29]=(a),[0x2a]=(a),[0x2b]=(a), \
	[0x2c]=(a),[0x2d]=(a),[0x2e]=(a),[0x2f]=(a), \
	[0x30]=(a),[0x31]=(a),[0x32]=(a),[0x33]=(a), \
	[0x34]=(a),[0x35]=(a),[0x36]=(a),[0x37]=(a), \
	[0x38]=(a),[0x39]=(a),[0x3a]=(a),[0x3b]=(a), \
	[0x3c]=(a),[0x3d]=(a),[0x3e]=(a),[0x3f]=(a), \
	[0x40]=(a),[0x41]=(a),[0x42]=(a),[0x43]=(a), \
	[0x44]=(a),[0x45]=(a),[0x46]=(a),[0x47]=(a), \
	[0x48]=(a),[0x49]=(a),[0x4a]=(a),[0x4b]=(a), \
	[0x4c]=(a),[0x4d]=(a),[0x4e]=(a),[0x4f]=(a), \
	[0x50]=(a),[0x51]=(a),[0x52]=(a),[0x53]=(a), \
	[0x54]=(a),[0x55]=(a),[0x56]=(a),[0x57]=(a), \
	[0x58]=(a),[0x59]=(a),[0x5a]=(a),[0x5b]=(a), \
	[0x5c]=(a),[0x5d]=(a),[0x5e]=(a),[0x5f]=(a), \
	[0x60]=(a),[0x61]=(a),[0x62]=(a),[0x63]=(a), \
	[0x64]=(a),[0x65]=(a),[0x66]=(a),[0x67]=(a), \
	[0x68]=(a),[0x69]=(a),[0x6a]=(a),[0x6b]=(a), \
	[0x6c]=(a),[0x6d]=(a),[0x6e]=(a),[0x6f]=(a), \
	[0x70]=(a),[0x71]=(a),[0x72]=(a),[0x73]=(a), \
	[0x74]=(a),[0x75]=(a),[0x76]=(a),[0x77]=(a), \
	[0x78]=(a),[0x79]=(a),[0x7a]=(a),[0x7b]=(a), \
	[0x7c]=(a),[0x7d]=(a),[0x7e]=(a),[0x7f]=(a), \
	[0x80]=(a),[0x81]=(a),[0x82]=(a),[0x83]=(a), \
	[0x84]=(a),[0x85]=(a),[0x86]=(a),[0x87]=(a), \
	[0x88]=(a),[0x89]=(a),[0x8a]=(a),[0x8b]=(a), \
	[0x8c]=(a),[0x8d]=(a),[0x8e]=(a),[0x8f]=(a), \
	[0x90]=(a),[0x91]=(a),[0x92]=(a),[0x93]=(a), \
	[0x94]=(a),[0x95]=(a),[0x96]=(a),[0x97]=(a), \
	[0x98]=(a),[0x99]=(a),[0x9a]=(a),[0x9b]=(a), \
	[0x9c]=(a),[0x9d]=(a),[0x9e]=(a),[0x9f]=(a), \
	[0xa0]=(a),[0xa1]=(a),[0xa2]=(a),[0xa3]=(a), \
	[0xa4]=(a),[0xa5]=(a),[0xa6]=(a),[0xa7]=(a), \
	[0xa8]=(a),[0xa9]=(a),[0xaa]=(a),[0xab]=(a), \
	[0xac]=(a),[0xad]=(a),[0xae]=(a),[0xaf]=(a), \
	[0xb0]=(a),[0xb1]=(a),[0xb2]=(a),[0xb3]=(a), \
	[0xb4]=(a),[0xb5]=(a),[0xb6]=(a),[0xb7]=(a), \
	[0xb8]=(a),[0xb9]=(a),[0xba]=(a),[0xbb]=(a), \
	[0xbc]=(a),[0xbd]=(a),[0xbe]=(a),[0xbf]=(a), \
	[0xc0]=(a),[0xc1]=(a),[0xc2]=(a),[0xc3]=(a), \
	[0xc4]=(a),[0xc5]=(a),[0xc6]=(a),[0xc7]=(a), \
	[0xc8]=(a),[0xc9]=(a),[0xca]=(a),[0xcb]=(a), \
	[0xcc]=(a),[0xcd]=(a),[0xce]=(a),[0xcf]=(a), \
	[0xd0]=(a),[0xd1]=(a),[0xd2]=(a),[0xd3]=(a), \
	[0xd4]=(a),[0xd5]=(a),[0xd6]=(a),[0xd7]=(a), \
	[0xd8]=(a),[0xd9]=(a),[0xda]=(a),[0xdb]=(a), \
	[0xdc]=(a),[0xdd]=(a),[0xde]=(a),[0xdf]=(a), \
	[0xe0]=(a),[0xe1]=(a),[0xe2]=(a),[0xe3]=(a), \
	[0xe4]=(a),[0xe5]=(a),[0xe6]=(a),[0xe7]=(a), \
	[0xe8]=(a),[0xe9]=(a),[0xea]=(a),[0xeb]=(a), \
	[0xec]=(a),[0xed]=(a),[0xee]=(a),[0xef]=(a), \
	[0xf0]=(a),[0xf1]=(a),[0xf2]=(a),[0xf3]=(a), \
	[0xf4]=(a),[0xf5]=(a),[0xf6]=(a),[0xf7]=(a), \
	[0xf8]=(a),[0xf9]=(a),[0xfa]=(a),[0xfb]=(a), \
	[0xfc]=(a),[0xfd]=(a),[0xfe]=(a),[0xff]=(a)
#define BVN_WHITESPACE(a) \
	[0x09]=(a),[0x0a]=(a),[0x0b]=(a), \
	[0x0c]=(a),[0x0d]=(a),[0x20]=(a)
#define BVN_DIGITS(a) \
	[0x30]=(a),[0x31]=(a),[0x32]=(a),[0x33]=(a),[0x34]=(a), \
	[0x35]=(a),[0x36]=(a),[0x37]=(a),[0x38]=(a),[0x39]=(a)
#define BVN_ALPHA_LOWER(a) \
	[0x61]=(a),[0x62]=(a),[0x63]=(a),[0x64]=(a),[0x65]=(a), \
	[0x66]=(a),[0x67]=(a),[0x68]=(a),[0x69]=(a),[0x6a]=(a), \
	[0x6b]=(a),[0x6c]=(a),[0x6d]=(a),[0x6e]=(a),[0x6f]=(a), \
	[0x70]=(a),[0x71]=(a),[0x72]=(a),[0x73]=(a),[0x74]=(a), \
	[0x75]=(a),[0x76]=(a),[0x77]=(a),[0x78]=(a),[0x79]=(a), \
	[0x7a]=(a)
#define BVN_ALPHA_UPPER(a) \
	[0x41]=(a),[0x42]=(a),[0x43]=(a),[0x44]=(a),[0x45]=(a), \
	[0x46]=(a),[0x47]=(a),[0x48]=(a),[0x49]=(a),[0x4a]=(a), \
	[0x4b]=(a),[0x4c]=(a),[0x4d]=(a),[0x4e]=(a),[0x4f]=(a), \
	[0x50]=(a),[0x51]=(a),[0x52]=(a),[0x53]=(a),[0x54]=(a), \
	[0x55]=(a),[0x56]=(a),[0x57]=(a),[0x58]=(a),[0x59]=(a), \
	[0x5a]=(a)
#define BVN_UTF8_LEADER(a) \
	[0xc2]=(a),[0xc3]=(a),[0xc4]=(a),[0xc5]=(a),[0xc6]=(a),[0xc7]=(a), \
	[0xc8]=(a),[0xc9]=(a),[0xca]=(a),[0xcb]=(a),[0xcc]=(a),[0xcd]=(a), \
	[0xce]=(a),[0xcf]=(a),[0xd0]=(a),[0xd1]=(a),[0xd2]=(a),[0xd3]=(a), \
	[0xd4]=(a),[0xd5]=(a),[0xd6]=(a),[0xd7]=(a),[0xd8]=(a),[0xd9]=(a), \
	[0xda]=(a),[0xdb]=(a),[0xdc]=(a),[0xdd]=(a),[0xde]=(a),[0xdf]=(a), \
	[0xe0]=(a),[0xe1]=(a),[0xe2]=(a),[0xe3]=(a),[0xe4]=(a),[0xe5]=(a), \
	[0xe6]=(a),[0xe7]=(a),[0xe8]=(a),[0xe9]=(a),[0xea]=(a),[0xeb]=(a), \
	[0xec]=(a),[0xed]=(a),[0xee]=(a),[0xef]=(a),[0xf0]=(a),[0xf1]=(a), \
	[0xf2]=(a),[0xf3]=(a),[0xf4]=(a)
#define BVN_UTF8_CONTINUATION(a) \
	[0x80]=(a),[0x81]=(a),[0x82]=(a),[0x83]=(a), \
	[0x84]=(a),[0x85]=(a),[0x86]=(a),[0x87]=(a), \
	[0x88]=(a),[0x89]=(a),[0x8a]=(a),[0x8b]=(a), \
	[0x8c]=(a),[0x8d]=(a),[0x8e]=(a),[0x8f]=(a), \
	[0x90]=(a),[0x91]=(a),[0x92]=(a),[0x93]=(a), \
	[0x94]=(a),[0x95]=(a),[0x96]=(a),[0x97]=(a), \
	[0x98]=(a),[0x99]=(a),[0x9a]=(a),[0x9b]=(a), \
	[0x9c]=(a),[0x9d]=(a),[0x9e]=(a),[0x9f]=(a), \
	[0xa0]=(a),[0xa1]=(a),[0xa2]=(a),[0xa3]=(a), \
	[0xa4]=(a),[0xa5]=(a),[0xa6]=(a),[0xa7]=(a), \
	[0xa8]=(a),[0xa9]=(a),[0xaa]=(a),[0xab]=(a), \
	[0xac]=(a),[0xad]=(a),[0xae]=(a),[0xaf]=(a), \
	[0xb0]=(a),[0xb1]=(a),[0xb2]=(a),[0xb3]=(a), \
	[0xb4]=(a),[0xb5]=(a),[0xb6]=(a),[0xb7]=(a), \
	[0xb8]=(a),[0xb9]=(a),[0xba]=(a),[0xbb]=(a), \
	[0xbc]=(a),[0xbd]=(a),[0xbe]=(a),[0xbf]=(a)
#define BVN_REJECT_ASCII_CTRL \
	[0x00]=ACT_NONE,[0x01]=ACT_NONE,[0x02]=ACT_NONE,[0x03]=ACT_NONE, \
	[0x04]=ACT_NONE,[0x05]=ACT_NONE,[0x06]=ACT_NONE,[0x07]=ACT_NONE, \
	[0x08]=ACT_NONE,[0x0e]=ACT_NONE,[0x0f]=ACT_NONE,[0x10]=ACT_NONE, \
	[0x11]=ACT_NONE,[0x12]=ACT_NONE,[0x13]=ACT_NONE,[0x14]=ACT_NONE, \
	[0x15]=ACT_NONE,[0x16]=ACT_NONE,[0x17]=ACT_NONE,[0x18]=ACT_NONE, \
	[0x19]=ACT_NONE,[0x1a]=ACT_NONE,[0x1b]=ACT_NONE,[0x1c]=ACT_NONE, \
	[0x1d]=ACT_NONE,[0x1e]=ACT_NONE,[0x1f]=ACT_NONE,[0x7f]=ACT_NONE
/*
 * Every byte a type-annotation PARAMETER may contain, except the two that end
 * one (',' and ':'), the '>' that ends the annotation, the '#' that opens a
 * comment, and whitespace. The three body states below differ only in what they
 * do with this class — copy it, or refuse it because whitespace split the
 * parameter — so it is one macro rather than three hand-kept copies that were
 * already drifting apart in comment text.
 *
 * '$' starts a currency, '%' is percent, '(' and ')' group a unit expression,
 * '~' joins a prefix to its unit, '^' introduces an exponent. spec 1.2 added
 * '[', ']', '{', '}' and '\'' for UCUM (bracketed atoms, annotations, codes like
 * [arb'U]); that widens the class for EVERY unit string, so "<float:64,m[s]>"
 * reaches bvn_parse_unit and fails there as error_unit_illegal instead of
 * failing here as error_unexpected_input_byte -- one error code moves for a
 * family of inputs that were errors before and are errors after. ';', '#', '<',
 * '>' and '"' stay OUT, so an unterminated bracket or annotation cannot consume
 * the rest of the document.
 */
#define BVN_TYPE_PARAM_CLASS(a) \
	[0x24]=(a),[0x25]=(a),[0x27]=(a),[0x28]=(a),[0x29]=(a), \
	[0x2a]=(a),[0x2b]=(a),[0x2d]=(a),[0x2e]=(a),[0x2f]=(a), \
	[0x5b]=(a),[0x5d]=(a),[0x5e]=(a),[0x5f]=(a), \
	[0x7b]=(a),[0x7d]=(a),[0x7e]=(a), \
	BVN_DIGITS(a), BVN_ALPHA_UPPER(a), BVN_ALPHA_LOWER(a), \
	BVN_UTF8_CONTINUATION(a), BVN_UTF8_LEADER(a)
/*
 * THE ROWS BELOW OVERRIDE INITIALISERS ON PURPOSE, and that is what the macros
 * above are for: one of them supplies the uniform default for all 256 columns,
 * and the explicit [byte] entries that follow replace it. Listing the bytes that
 * matter and letting a macro fill the rest is what guarantees every column is
 * defined -- an undefined column would be a silent zero, i.e. ACT_NONE, i.e. a
 * byte rejected by accident rather than by decision. So -Woverride-init (GCC) /
 * -Winitializer-overrides (clang) fires 546 times here, on correct code, and on
 * nothing else in the library.
 *
 * SUPPRESSED HERE RATHER THAN WITH A -Wno- FLAG IN THE BUILD, because a flag
 * only protects THIS build. dist/bovnar.c carries this file verbatim, and an
 * integrator who compiles the amalgamation under their own -Wall -Wextra
 * -Werror -- which is the first thing anyone does with a single-file library --
 * met 546 warnings on their first build with no way to tell deliberate from
 * accidental. A pragma travels with the code it is about; a build flag does not.
 * cmake/amalgam_smoke.cmake compiles the amalgamation with no -Wno- of its own,
 * so that first build is now what the smoke test already proved.
 *
 * MSVC has no equivalent warning and needs no arm here.
 */
#if defined(__clang__)
#	pragma clang diagnostic push
#	pragma clang diagnostic ignored "-Winitializer-overrides"
#elif defined(__GNUC__)
#	pragma GCC diagnostic push
#	pragma GCC diagnostic ignored "-Woverride-init"
#endif
const uint8_t bvn_after_state_idx_table[dimension_state][256] = {
	/* A '}' before the first statement is an unmatched close like any other
	 * (doc/03 §8.5: error_illegal_struct_close at nesting level 0); it was
	 * error_unexpected_input_byte here and illegal_struct_close after ".a=1;". */
	[undefined] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_first_comment_intro,
		[0x2e] = ACT_identifier_intro,
		[0x7d] = ACT_struct_outro,
		[0xef] = ACT_copy_utf8bom_byte,
	},
	[first_bom] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_first_comment_intro,
		[0x2e] = ACT_identifier_intro,
		[0x7d] = ACT_struct_outro,
	},
	[utf8bom_intro] = {
		BVN_EACH_256(ACT_copy_utf8bom_byte),
	},
	[comment_intro] = {
		BVN_EACH_256(ACT_ignore_comment_byte),
		BVN_REJECT_ASCII_CTRL,
		[0x0a] = ACT_comment_outro,
		[0x0d] = ACT_comment_outro,
	},
	[ignore_comment_byte] = {
		BVN_EACH_256(ACT_ignore_comment_byte),
		BVN_REJECT_ASCII_CTRL,
		[0x0a] = ACT_comment_outro,
		[0x0d] = ACT_comment_outro,
	},
	[comment_outro] = { 0 },
	[first_comment_intro] = {
		BVN_EACH_256(ACT_first_comment_byte),
		BVN_REJECT_ASCII_CTRL,
		[0x0a] = ACT_first_comment_outro,
		[0x0d] = ACT_first_comment_outro,
	},
	[first_comment_after_ef] = {
		BVN_EACH_256(ACT_first_comment_after_ef),
		BVN_REJECT_ASCII_CTRL,
		[0x0a] = ACT_first_comment_outro,
		[0x0d] = ACT_first_comment_outro,
	},
	[first_comment_after_ef_bb] = {
		BVN_EACH_256(ACT_first_comment_after_ef_bb),
		BVN_REJECT_ASCII_CTRL,
		[0x0a] = ACT_first_comment_outro,
		[0x0d] = ACT_first_comment_outro,
	},
	[identifier_intro] = {
		BVN_ALPHA_UPPER(ACT_copy_identifier_byte),
		BVN_ALPHA_LOWER(ACT_copy_identifier_byte),
		[0x5f] = ACT_copy_identifier_byte,
		BVN_UTF8_LEADER(ACT_copy_identifier_byte),
		[0xc2] = ACT_NONE,
		/* A '.' followed immediately by whitespace or '=' is an EMPTY key. Mirror
		 * identifier_body's transitions so the (zero-length) identifier token gets
		 * finalized via value_intro/identifier_outro; bvn_lex_finalize then reports
		 * error_empty_identifier, as the spec (§4.2, §12.6) documents for ".=" and
		 * ". =". Other non-identifier-start bytes (digits, punctuation) stay at the
		 * ACT_NONE default and remain error_unexpected_input_byte. */
		BVN_WHITESPACE(ACT_to_identifier_outro),
		[0x3d] = ACT_value_intro,
	},
	[identifier_body] = {
		BVN_EACH_256(ACT_copy_identifier_byte),
		BVN_REJECT_ASCII_CTRL,
		BVN_WHITESPACE(ACT_to_identifier_outro),
		[0x22] = ACT_NONE, [0x23] = ACT_NONE, [0x2c] = ACT_NONE,
		[0x2e] = ACT_NONE, [0x2f] = ACT_NONE, [0x3b] = ACT_NONE,
		[0x3c] = ACT_NONE, [0x3d] = ACT_value_intro,
		[0x3e] = ACT_NONE, [0x5b] = ACT_NONE, [0x5d] = ACT_NONE,
		[0x7b] = ACT_NONE, [0x7d] = ACT_NONE, [0x21] = ACT_NONE,
		[0x24] = ACT_NONE, [0x25] = ACT_NONE, [0x26] = ACT_NONE,
		[0x27] = ACT_NONE, [0x28] = ACT_NONE, [0x29] = ACT_NONE,
		[0x2a] = ACT_NONE, [0x2b] = ACT_copy_identifier_byte,
		[0x2d] = ACT_copy_identifier_byte, [0x3a] = ACT_NONE,
		[0x3f] = ACT_NONE, [0x40] = ACT_NONE, [0x5c] = ACT_NONE,
		[0x5e] = ACT_NONE, [0x60] = ACT_NONE, [0x7c] = ACT_NONE,
		[0x7e] = ACT_NONE, [0xc2] = ACT_NONE,
	},
	[identifier_outro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		[0x3d] = ACT_value_intro,
	},
	[type_intro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		['u'] = ACT_tf_to_u,
		['s'] = ACT_tf_to_s,
		['f'] = ACT_tf_to_f,
		['b'] = ACT_tf_to_b,
		['d'] = ACT_tf_to_d,
	},
	[type_noparams] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		[0x3e] = ACT_type_outro,
	},
	[tf_u] = { ['i'] = ACT_tf_u_to_ui, ['t'] = ACT_tf_u_to_ut },
	[tf_ui]  = { ['n'] = ACT_kw_advance },
	[tf_uin] = { ['t'] = ACT_tf_uint_done },
	[tf_ut]  = { ['f'] = ACT_kw_advance },
	[tf_utf] = { ['8'] = ACT_tf_utf8_done },
	[tf_s]   = { ['i'] = ACT_kw_advance },
	[tf_si]  = { ['n'] = ACT_kw_advance },
	[tf_sin] = { ['t'] = ACT_tf_sint_done },
	[tf_f]    = { ['l'] = ACT_kw_advance },
	[tf_fl]   = { ['o'] = ACT_kw_advance },
	[tf_flo]  = { ['a'] = ACT_kw_advance },
	[tf_floa] = { ['t'] = ACT_tf_float_done },
	[tf_b]    = { ['o'] = ACT_kw_advance },
	[tf_bo]   = { ['o'] = ACT_kw_advance },
	[tf_boo]  = { ['l'] = ACT_tf_bool_done },
	[tf_d]       = { ['a'] = ACT_kw_advance },
	[tf_da]      = { ['t'] = ACT_kw_advance },
	[tf_dat]     = { ['e'] = ACT_kw_advance },
	[tf_date]    = { ['t'] = ACT_kw_advance },
	[tf_datet]   = { ['i'] = ACT_kw_advance },
	[tf_dateti]  = { ['m'] = ACT_kw_advance },
	[tf_datetim] = { ['e'] = ACT_tf_datetime_done },
	/*
	 * The three annotation-body states. They accept the same bytes and differ
	 * only in where whitespace may fall:
	 *
	 *   copy_type_byte   mid-parameter. Whitespace here has no production in
	 *                    doc/12 (`type-param-list = type-param , {ws , "," ,
	 *                    ws , type-param}` puts every `ws` beside a separator),
	 *                    so it does not resume the parameter -- it goes to
	 *                    type_body_ws for judgement.
	 *   type_body_outro  at a separator boundary: after the family keyword,
	 *                    after a ',' or after the family ':'. Whitespace and
	 *                    comments are ignorable here, which is what keeps
	 *                    "<uint:8, 16>" and "<float : 64>" legal.
	 *   type_body_ws     mid-parameter, whitespace already consumed. Only a
	 *                    separator, a '>' or a comment may follow; a parameter
	 *                    byte is error_type_param_whitespace.
	 *
	 * Before this split the whole body was two states and whitespace was simply
	 * dropped, which made "<float:64,k g>" a kilogram and the UDUNITS spelling
	 * "udunits:m s-1" a reciprocal millisecond -- the one place in the format
	 * where a wrong unit was produced silently instead of refused.
	 */
	[copy_type_byte] = {
		BVN_WHITESPACE(ACT_type_ws_byte),
		BVN_TYPE_PARAM_CLASS(ACT_copy_type_byte),
		[0x2c] = ACT_copy_type_sep_byte,
		[0x3a] = ACT_copy_type_sep_byte,
		[0x3e] = ACT_type_outro,
	},
	[type_body_outro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		BVN_TYPE_PARAM_CLASS(ACT_copy_type_byte),
		[0x2c] = ACT_copy_type_sep_byte,
		[0x3a] = ACT_copy_type_sep_byte,
		[0x3e] = ACT_type_outro,
	},
	[type_body_ws] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		BVN_TYPE_PARAM_CLASS(ACT_type_param_whitespace),
		[0x2c] = ACT_copy_type_sep_byte,
		[0x3a] = ACT_copy_type_sep_byte,
		/* Whitespace before the closing '>' is the outermost `ws` of
		 * `type-annotation = "<" , ws , param-type , ws , ">"`, and deleting it
		 * cannot change which unit was written. */
		[0x3e] = ACT_type_outro,
	},
	[type_outro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x00] = ACT_octet_stream_intro,
		[0x22] = ACT_string_intro,
		[0x23] = ACT_comment_intro,
		[0x26] = ACT_reference_intro,
		[0x2c] = ACT_type_null_then_new_array_value,
		[0x2d] = ACT_neg_number_intro,
		[0x2e] = ACT_fraction_no_int,
		BVN_DIGITS(ACT_copy_number_byte),
		[0x30] = ACT_zero_intro,
		[0x3b] = ACT_type_null_then_value_outro,
		[0x5b] = ACT_array_intro,
		[0x5d] = ACT_type_null_then_array_outro,
		[0x5f] = ACT_symbol_intro,
		[0x7b] = ACT_struct_intro,
		BVN_ALPHA_UPPER(ACT_symbol_intro),
		BVN_ALPHA_LOWER(ACT_symbol_intro),
		BVN_UTF8_LEADER(ACT_symbol_intro),
		[0xc2] = ACT_NONE,
	},
	[value_intro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x00] = ACT_octet_stream_intro,
		[0x22] = ACT_string_intro,
		[0x23] = ACT_comment_intro,
		[0x26] = ACT_reference_intro,
		[0x2d] = ACT_neg_number_intro,
		[0x2e] = ACT_fraction_no_int,
		BVN_DIGITS(ACT_copy_number_byte),
		[0x30] = ACT_zero_intro,
		[0x3b] = ACT_value_outro,
		[0x3c] = ACT_type_intro,
		BVN_ALPHA_UPPER(ACT_symbol_intro),
		[0x5b] = ACT_array_intro,
		[0x5f] = ACT_symbol_intro,
		BVN_ALPHA_LOWER(ACT_symbol_intro),
		[0x7b] = ACT_struct_intro,
		BVN_UTF8_LEADER(ACT_symbol_intro),
		[0xc2] = ACT_NONE,
	},
	[array_intro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x00] = ACT_octet_stream_intro,
		[0x22] = ACT_arr_string_intro,
		[0x23] = ACT_comment_intro,
		[0x26] = ACT_reference_intro,
		[0x2c] = ACT_new_array_value,
		[0x2d] = ACT_neg_number_intro,
		[0x2e] = ACT_fraction_no_int,
		BVN_DIGITS(ACT_copy_number_byte),
		[0x30] = ACT_zero_intro,
		[0x3c] = ACT_type_intro,
		[0x5b] = ACT_array_intro,
		/* "]" straight after "[" (only ws/comments between) = empty array. */
		[0x5d] = ACT_array_outro_empty,
		[0x5f] = ACT_symbol_intro,
		[0x7b] = ACT_struct_intro,
		BVN_ALPHA_UPPER(ACT_symbol_intro),
		BVN_ALPHA_LOWER(ACT_symbol_intro),
		BVN_UTF8_LEADER(ACT_symbol_intro),
		[0xc2] = ACT_NONE,
	},
	[new_array_value] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x00] = ACT_octet_stream_intro,
		[0x22] = ACT_arr_string_intro,
		[0x23] = ACT_comment_intro,
		[0x26] = ACT_reference_intro,
		[0x2c] = ACT_new_array_value,
		[0x2d] = ACT_neg_number_intro,
		[0x2e] = ACT_fraction_no_int,
		BVN_DIGITS(ACT_copy_number_byte),
		[0x30] = ACT_zero_intro,
		[0x3c] = ACT_type_intro,
		[0x5b] = ACT_array_intro,
		[0x5d] = ACT_array_outro,
		[0x5f] = ACT_symbol_intro,
		[0x7b] = ACT_struct_intro,
		BVN_ALPHA_UPPER(ACT_symbol_intro),
		BVN_ALPHA_LOWER(ACT_symbol_intro),
		BVN_UTF8_LEADER(ACT_symbol_intro),
		[0xc2] = ACT_NONE,
	},
	[array_outro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		[0x2c] = ACT_new_array_value,
		[0x2f] = ACT_array_dim_sep,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	[array_dim_sep] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		[0x5b] = ACT_array_intro,
	},
	[neg_number_intro] = {
		[0x2e] = ACT_fraction_no_int,
		BVN_DIGITS(ACT_copy_number_byte),
		[0x30] = ACT_zero_intro,
	},
	[zero_intro] = {
		BVN_WHITESPACE(ACT_to_number_outro),
		[0x2e] = ACT_fraction_intro,
		BVN_DIGITS(ACT_copy_number_byte),
		[0x2c] = ACT_new_array_value,
		[0x2d] = ACT_dtlit_after_y,   /* '-' begins an ISO-8601 literal (1.1) */
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
		[0x45] = ACT_exp_intro,
		[0x65] = ACT_exp_intro,
	},
	[copy_number_byte] = {
		BVN_WHITESPACE(ACT_to_number_outro),
		[0x2e] = ACT_fraction_intro,
		BVN_DIGITS(ACT_copy_number_byte),
		[0x2c] = ACT_new_array_value,
		[0x2d] = ACT_dtlit_after_y,   /* '-' begins an ISO-8601 literal (1.1) */
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
		[0x45] = ACT_exp_intro,
		[0x65] = ACT_exp_intro,
	},
	[fraction_intro] = {
		BVN_WHITESPACE(ACT_to_number_outro),
		BVN_DIGITS(ACT_copy_fraction_byte),
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
		[0x45] = ACT_exp_intro,
		[0x65] = ACT_exp_intro,
	},
	[fraction_no_int] = {
		BVN_DIGITS(ACT_copy_fraction_byte),
	},
	[copy_fraction_byte] = {
		BVN_WHITESPACE(ACT_to_number_outro),
		BVN_DIGITS(ACT_copy_fraction_byte),
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
		[0x45] = ACT_exp_intro,
		[0x65] = ACT_exp_intro,
	},
	[number_outro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
		[0x24] = ACT_inline_unit_intro,
		[0x25] = ACT_inline_unit_intro,
		[0x28] = ACT_inline_unit_intro,
		BVN_ALPHA_UPPER(ACT_inline_unit_intro),
		BVN_ALPHA_LOWER(ACT_inline_unit_intro),
		[0x5f] = ACT_inline_unit_intro,
		BVN_UTF8_LEADER(ACT_inline_unit_intro),
	},
	/* Vestigial row: number_outro_nosp is unreachable since nan/inf/ninf
	 * became bare keywords (lexed as symbols). Kept to avoid renumbering
	 * state_t before the 1.0 freeze. See bvn_lexer_impl.h. */
	[number_outro_nosp] = {
		BVN_WHITESPACE(ACT_to_number_outro),
		[0x23] = ACT_comment_intro,
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	[exp_intro] = {
		[0x2b] = ACT_exp_sign_intro,
		[0x2d] = ACT_exp_sign_intro,
		BVN_DIGITS(ACT_copy_exp_byte),
	},
	[exp_sign_intro] = {
		BVN_DIGITS(ACT_copy_exp_byte),
	},
	[copy_exp_byte] = {
		BVN_WHITESPACE(ACT_to_number_outro),
		BVN_DIGITS(ACT_copy_exp_byte),
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	/* ISO-8601 datetime literal states (spec 1.1). Bytes accumulate into
	 * str_data via ACT_dtlit_* (-> bvn_action_dtlit_byte); dtlit_day/_sec/
	 * _zulu are complete-value states that accept the number terminators
	 * (whitespace -> number_outro, ',' ';' ']'), the rest require the next
	 * structural byte ('-', 'T', ':', 'Z') or a digit. The lexer is
	 * deliberately permissive about digit counts — the validator strictly
	 * range-checks each field and the YYYY-MM-DD widths. */
	[dtlit_after_y] = {
		BVN_DIGITS(ACT_dtlit_month),
	},
	[dtlit_month] = {
		BVN_DIGITS(ACT_dtlit_month),
		[0x2d] = ACT_dtlit_after_m,
	},
	[dtlit_after_m] = {
		BVN_DIGITS(ACT_dtlit_day),
	},
	[dtlit_day] = {
		BVN_WHITESPACE(ACT_to_number_outro),
		BVN_DIGITS(ACT_dtlit_day),
		[0x54] = ACT_dtlit_after_T,   /* 'T' date/time separator */
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	[dtlit_after_T] = {
		BVN_DIGITS(ACT_dtlit_hour),
	},
	[dtlit_hour] = {
		BVN_DIGITS(ACT_dtlit_hour),
		[0x3a] = ACT_dtlit_after_h,   /* ':' */
	},
	[dtlit_after_h] = {
		BVN_DIGITS(ACT_dtlit_min),
	},
	[dtlit_min] = {
		BVN_DIGITS(ACT_dtlit_min),
		[0x3a] = ACT_dtlit_after_mi,  /* ':' */
	},
	[dtlit_after_mi] = {
		BVN_DIGITS(ACT_dtlit_sec),
	},
	[dtlit_sec] = {
		BVN_WHITESPACE(ACT_to_number_outro),
		BVN_DIGITS(ACT_dtlit_sec),
		[0x2e] = ACT_dtlit_frac,      /* '.' fractional seconds (spec 1.1) */
		[0x5a] = ACT_dtlit_zulu,      /* 'Z' UTC designator */
		[0x2b] = ACT_dtlit_off_sign,  /* '+' tz offset (spec 1.1) */
		[0x2d] = ACT_dtlit_off_sign,  /* '-' tz offset (spec 1.1) */
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	[dtlit_zulu] = {
		BVN_WHITESPACE(ACT_to_number_outro),
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	/* Fractional seconds (spec 1.1): digits after '.', then an optional 'Z' or
	 * ±HH:MM tz offset. The carrier stays whole seconds, but the validator keeps
	 * the fraction digits verbatim (surfaced as bvnr_data_t.frac_data and
	 * re-emitted on round-trip) — it does not discard them. */
	[dtlit_frac] = {
		BVN_WHITESPACE(ACT_to_number_outro),
		BVN_DIGITS(ACT_dtlit_frac),
		[0x5a] = ACT_dtlit_zulu,      /* 'Z' */
		[0x2b] = ACT_dtlit_off_sign,  /* '+' */
		[0x2d] = ACT_dtlit_off_sign,  /* '-' */
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	/* tz offset (spec 1.1): ±HH:MM. The validator strictly checks the widths
	 * and ranges and folds the offset to true UTC before converting. */
	[dtlit_off_sign] = {
		BVN_DIGITS(ACT_dtlit_off_hour),
	},
	[dtlit_off_hour] = {
		BVN_DIGITS(ACT_dtlit_off_hour),
		[0x3a] = ACT_dtlit_off_colon,  /* ':' */
	},
	[dtlit_off_colon] = {
		BVN_DIGITS(ACT_dtlit_off_min),
	},
	[dtlit_off_min] = {
		BVN_WHITESPACE(ACT_to_number_outro),
		BVN_DIGITS(ACT_dtlit_off_min),
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	[symbol_body] = {
		BVN_EACH_256(ACT_copy_symbol_byte),
		BVN_REJECT_ASCII_CTRL,
		BVN_WHITESPACE(ACT_to_symbol_outro),
		[0x22] = ACT_NONE, [0x23] = ACT_NONE, [0x2c] = ACT_new_array_value,
		[0x2e] = ACT_NONE, [0x2f] = ACT_NONE, [0x3b] = ACT_value_outro,
		[0x3c] = ACT_NONE, [0x3d] = ACT_NONE, [0x3e] = ACT_NONE,
		[0x5b] = ACT_NONE, [0x5d] = ACT_array_outro, [0x7b] = ACT_NONE,
		[0x7d] = ACT_NONE, [0x21] = ACT_NONE, [0x24] = ACT_NONE,
		[0x25] = ACT_NONE, [0x26] = ACT_NONE, [0x27] = ACT_NONE,
		[0x28] = ACT_NONE, [0x29] = ACT_NONE, [0x2a] = ACT_NONE,
		[0x2b] = ACT_copy_symbol_byte, [0x2d] = ACT_copy_symbol_byte,
		[0x3a] = ACT_NONE, [0x3f] = ACT_NONE, [0x40] = ACT_NONE,
		[0x5c] = ACT_NONE, [0x5e] = ACT_NONE, [0x60] = ACT_NONE,
		[0x7c] = ACT_NONE, [0x7e] = ACT_NONE, [0xc2] = ACT_NONE,
	},
	[symbol_outro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	[reference_intro] = {
		[0x2e] = ACT_copy_reference_dot,
	},
	[reference_segment_intro] = {
		BVN_ALPHA_UPPER(ACT_copy_reference_byte),
		BVN_ALPHA_LOWER(ACT_copy_reference_byte),
		[0x5f] = ACT_copy_reference_byte,
		BVN_UTF8_LEADER(ACT_copy_reference_byte),
		[0xc2] = ACT_NONE,
	},
	[reference_segment_body] = {
		BVN_EACH_256(ACT_copy_reference_byte),
		BVN_REJECT_ASCII_CTRL,
		BVN_WHITESPACE(ACT_to_reference_outro),
		[0x22] = ACT_NONE, [0x23] = ACT_NONE, [0x2c] = ACT_new_array_value,
		[0x2e] = ACT_copy_reference_dot, [0x2f] = ACT_NONE,
		[0x3b] = ACT_value_outro, [0x3c] = ACT_NONE,
		[0x3d] = ACT_NONE, [0x3e] = ACT_NONE,
		[0x5b] = ACT_reference_index_open, [0x5d] = ACT_array_outro,
		[0x7b] = ACT_NONE, [0x7d] = ACT_NONE,
		[0x21] = ACT_NONE, [0x24] = ACT_NONE,
		[0x25] = ACT_NONE, [0x26] = ACT_NONE,
		[0x27] = ACT_NONE, [0x28] = ACT_NONE, [0x29] = ACT_NONE,
		[0x2a] = ACT_NONE,
		[0x2b] = ACT_copy_reference_byte,
		[0x2d] = ACT_copy_reference_byte,
		[0x3a] = ACT_NONE, [0x3f] = ACT_NONE,
		[0x40] = ACT_NONE, [0x5c] = ACT_NONE,
		[0x5e] = ACT_NONE, [0x60] = ACT_NONE,
		[0x7c] = ACT_NONE, [0x7e] = ACT_NONE,
		[0xc2] = ACT_NONE,
	},
	[reference_index_first] = {
		/* the first byte after '[' MUST be a digit (no ']' here), so an empty
		 * "[]" is rejected; a digit advances to reference_index. */
		BVN_DIGITS(ACT_reference_index_byte),
	},
	[reference_index] = {
		/* inside &.path[...]: digits accumulate the index; ']' closes it and
		 * returns to the segment body (where another '[' or '.' may follow).
		 * Everything else is illegal — the index is [digits]. */
		BVN_DIGITS(ACT_reference_index_byte),
		[0x5d] = ACT_reference_index_close,
	},
	[reference_outro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	[copy_string_byte] = {
		BVN_EACH_256(ACT_copy_string_byte),
		BVN_REJECT_ASCII_CTRL,
		[0x22] = ACT_string_outro,
		[0x5c] = ACT_escape_from_copy,
	},
	[escape_from_copy] = {
		/* Every byte after a backslash is handed to bvn_decode_escape,
		 * which accepts the seven bovnar escapes (\" \\ \f \n \r \t \v)
		 * and rejects anything else as error_illegal_escape_sequence.
		 * Routing all 256 columns here (rather than leaving non-escape
		 * bytes at ACT_NONE) is what makes that error reachable instead
		 * of the generic error_unexpected_input_byte. */
		BVN_EACH_256(ACT_replace_escaped_byte),
		/* spec 1.1: \xHH and \u{...}. The intro actions gate on a declared
		 * spec >= 1.1 — in a 1.0/unversioned document they fall back to
		 * error_illegal_escape_sequence, so 'x'/'u' stay non-escapes there. */
		[0x78] = ACT_escape_x_intro,   /* x */
		[0x75] = ACT_escape_u_intro,   /* u */
	},
	[esc_x1] = {
		BVN_EACH_256(ACT_escape_x_d1),
	},
	[esc_x2] = {
		BVN_EACH_256(ACT_escape_x_d2),
	},
	[esc_u_open] = {
		BVN_EACH_256(ACT_escape_u_open),
	},
	[esc_u_hex] = {
		BVN_EACH_256(ACT_escape_u_hex),
	},
	[string_intro] = {
		BVN_EACH_256(ACT_copy_string_byte),
		BVN_REJECT_ASCII_CTRL,
		[0x22] = ACT_string_outro,
		[0x5c] = ACT_escape_from_copy,
	},
	[string_outro_nosp] = {
		BVN_WHITESPACE(ACT_to_string_outro),
		[0x22] = ACT_string_intro,
		[0x23] = ACT_comment_intro,
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	[string_outro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x22] = ACT_string_intro,
		[0x23] = ACT_comment_intro,
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
		[0x24] = ACT_inline_unit_intro,
		[0x25] = ACT_inline_unit_intro,
		[0x28] = ACT_inline_unit_intro,
		BVN_ALPHA_UPPER(ACT_inline_unit_intro),
		BVN_ALPHA_LOWER(ACT_inline_unit_intro),
		[0x5f] = ACT_inline_unit_intro,
		BVN_UTF8_LEADER(ACT_inline_unit_intro),
	},
	[octet_stream_intro] = { 0 },
	[inline_unit_body] = {
		BVN_ALPHA_UPPER(ACT_copy_inline_unit_byte),
		BVN_ALPHA_LOWER(ACT_copy_inline_unit_byte),
		BVN_DIGITS(ACT_copy_inline_unit_byte),
		[0x2a] = ACT_copy_inline_unit_byte,
		[0x2b] = ACT_copy_inline_unit_byte,
		[0x2d] = ACT_copy_inline_unit_byte,
		[0x2e] = ACT_copy_inline_unit_byte,
		[0x2f] = ACT_copy_inline_unit_byte,
		[0x3a] = ACT_copy_inline_unit_byte,
		[0x5e] = ACT_copy_inline_unit_byte,
		[0x5f] = ACT_copy_inline_unit_byte,
		[0x7e] = ACT_copy_inline_unit_byte,
		/* spec 1.2 — the UCUM unit profile needs '[', ']', '{', '}' and '\''
		 * (bracketed atoms, annotations, codes like [arb'U]). This widens the
		 * class for EVERY unit string, so "<float:64,m[s]>" now reaches
		 * bvn_parse_unit and fails there as error_unit_illegal instead of
		 * failing here as error_unexpected_input_byte: one error code moves for
		 * a family of inputs that were errors before and are errors after.
		 * ';', '#', '<', '>' and '"' stay OUT, so an unterminated bracket or
		 * annotation cannot consume the rest of the document. */
		[0x27] = ACT_copy_inline_unit_byte,
		[0x5b] = ACT_copy_inline_unit_byte,
		[0x5d] = ACT_copy_inline_unit_byte,
		[0x7b] = ACT_copy_inline_unit_byte,
		[0x7d] = ACT_copy_inline_unit_byte,
		[0x24] = ACT_copy_inline_unit_byte,
		[0x25] = ACT_copy_inline_unit_byte,
		[0x28] = ACT_copy_inline_unit_byte,
		[0x29] = ACT_copy_inline_unit_byte,
		BVN_UTF8_CONTINUATION(ACT_copy_inline_unit_byte),
		BVN_UTF8_LEADER(ACT_copy_inline_unit_byte),
		BVN_WHITESPACE(ACT_to_inline_unit_outro),
		[0x23] = ACT_comment_intro,
		[0x3b] = ACT_value_outro,
	},
	[inline_unit_outro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		[0x3b] = ACT_value_outro,
	},
	[octet_stream_outro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	[struct_intro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		[0x2e] = ACT_identifier_intro,
		[0x7d] = ACT_struct_outro,
	},
	[struct_outro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		[0x2c] = ACT_new_array_value,
		[0x3b] = ACT_value_outro,
		[0x5d] = ACT_array_outro,
	},
	[value_outro] = {
		BVN_WHITESPACE(ACT_ignore_whitespace),
		[0x23] = ACT_comment_intro,
		[0x2e] = ACT_identifier_intro,
		[0x7d] = ACT_struct_outro,
	},
	[resync] = {
		BVN_EACH_256(ACT_resync_skip),
		[0x22] = ACT_resync_string_intro,
		[0x23] = ACT_resync_comment_intro,
		/* Every assignment begins with '.', so a '.' at the recovery-relative
		 * top level is the earliest point recovery could plausibly end. It is
		 * only a candidate: resync_dot decides on the byte that follows. */
		[0x2e] = ACT_resync_dot,
		[0x3b] = ACT_resync_semicolon,
		[0x5b] = ACT_resync_open_bracket,
		[0x5d] = ACT_resync_close_bracket,
		[0x7b] = ACT_resync_open_bracket,
		[0x7d] = ACT_resync_close_bracket,
	},
	/* Identical to `resync` except that a byte which can START AN IDENTIFIER
	 * ends recovery here — the '.' just skipped opened the next assignment.
	 * Every other byte behaves exactly as it would have in `resync`, so a '.'
	 * that leads nowhere (".5", ".;", a '.' inside binary junk) costs nothing
	 * but one extra state hop. 0xc2 is excluded for the same reason
	 * identifier_intro excludes it. */
	[resync_dot] = {
		BVN_EACH_256(ACT_resync_skip),
		BVN_ALPHA_UPPER(ACT_resync_resume_identifier),
		BVN_ALPHA_LOWER(ACT_resync_resume_identifier),
		[0x5f] = ACT_resync_resume_identifier,
		BVN_UTF8_LEADER(ACT_resync_resume_identifier),
		[0xc2] = ACT_resync_skip,
		[0x22] = ACT_resync_string_intro,
		[0x23] = ACT_resync_comment_intro,
		[0x2e] = ACT_resync_dot,
		[0x3b] = ACT_resync_semicolon,
		[0x5b] = ACT_resync_open_bracket,
		[0x5d] = ACT_resync_close_bracket,
		[0x7b] = ACT_resync_open_bracket,
		[0x7d] = ACT_resync_close_bracket,
	},
	[resync_string] = {
		BVN_EACH_256(ACT_resync_string_byte),
		[0x22] = ACT_resync_string_outro,
		[0x5c] = ACT_resync_string_escape,
	},
	[resync_string_escape] = {
		BVN_EACH_256(ACT_resync_string_escape_byte),
	},
	[resync_comment] = {
		BVN_EACH_256(ACT_resync_comment_byte),
		[0x0a] = ACT_resync_comment_outro,
		[0x0d] = ACT_resync_comment_outro,
	},
};
#if defined(__clang__)
#	pragma clang diagnostic pop
#elif defined(__GNUC__)
#	pragma GCC diagnostic pop
#endif
#undef BVN_EACH_256
#undef BVN_WHITESPACE
#undef BVN_DIGITS
#undef BVN_ALPHA_LOWER
#undef BVN_ALPHA_UPPER
#undef BVN_UTF8_LEADER
#undef BVN_UTF8_CONTINUATION
#undef BVN_REJECT_ASCII_CTRL
const action_t bvn_action_table[ACT__count] = {
	[ACT_NONE]                       = NULL,
	[ACT_ignore_whitespace]          = bvn_action_ignore_whitespace,
	[ACT_comment_intro]              = bvn_action_comment_intro,
	[ACT_ignore_comment_byte]        = bvn_action_set_state,
	[ACT_comment_outro]              = bvn_action_comment_outro,
	[ACT_copy_utf8bom_byte]          = bvn_action_copy_utf8bom_byte,
	[ACT_identifier_intro]           = bvn_action_identifier_intro,
	[ACT_copy_identifier_byte]       = bvn_action_copy_identifier_byte,
	[ACT_to_identifier_outro]        = bvn_action_set_state,
	[ACT_value_intro]                = bvn_action_value_intro,
	[ACT_value_outro]                = bvn_action_value_outro,
	[ACT_type_intro]                 = bvn_action_type_intro,
	[ACT_type_outro]                 = bvn_action_type_outro,
	[ACT_copy_type_byte]             = bvn_action_copy_type_byte,
	[ACT_copy_type_sep_byte]         = bvn_action_copy_type_sep_byte,
	[ACT_type_param_whitespace]      = bvn_action_type_param_whitespace,
	[ACT_type_ws_byte]               = bvn_action_type_ws_byte,
	[ACT_neg_number_intro]           = bvn_action_neg_number_intro,
	[ACT_copy_number_byte]           = bvn_action_copy_number_byte,
	[ACT_zero_intro]                 = bvn_action_zero_intro,
	[ACT_fraction_intro]             = bvn_action_fraction_intro,
	[ACT_fraction_no_int]            = bvn_action_fraction_no_int,
	[ACT_copy_fraction_byte]         = bvn_action_copy_fraction_byte,
	[ACT_to_number_outro]            = bvn_action_set_state,
	[ACT_kw_advance]                 = bvn_action_kw_advance,
	[ACT_tf_to_u]                    = bvn_action_set_state,
	[ACT_tf_to_s]                    = bvn_action_set_state,
	[ACT_tf_to_f]                    = bvn_action_set_state,
	[ACT_tf_to_b]                    = bvn_action_set_state,
	[ACT_tf_to_d]                    = bvn_action_set_state,
	[ACT_tf_datetime_done]           = bvn_action_tf_datetime_done,
	[ACT_tf_u_to_ui]                 = bvn_action_set_state,
	[ACT_tf_u_to_ut]                 = bvn_action_set_state,
	[ACT_tf_uint_done]               = bvn_action_tf_uint_done,
	[ACT_tf_sint_done]               = bvn_action_tf_sint_done,
	[ACT_tf_float_done]              = bvn_action_tf_float_done,
	[ACT_tf_utf8_done]               = bvn_action_tf_utf8_done,
	[ACT_tf_bool_done]               = bvn_action_tf_bool_done,
	[ACT_array_intro]                = bvn_action_array_intro,
	[ACT_array_outro]                = bvn_action_array_outro,
	[ACT_array_outro_empty]          = bvn_action_array_outro_empty,
	[ACT_new_array_value]            = bvn_action_new_array_value,
	[ACT_array_dim_sep]              = bvn_action_array_dim_sep,
	[ACT_exp_intro]                  = bvn_action_exp_intro,
	[ACT_exp_sign_intro]             = bvn_action_exp_sign_intro,
	[ACT_copy_exp_byte]              = bvn_action_copy_exp_byte,
	[ACT_string_intro]               = bvn_action_string_intro,
	[ACT_string_outro]               = bvn_action_set_state,
	[ACT_copy_string_byte]           = bvn_action_copy_string_byte,
	[ACT_replace_escaped_byte]       = bvn_action_replace_escaped_byte,
	[ACT_escape_from_copy]           = bvn_action_set_state,
	[ACT_escape_x_intro]             = bvn_action_escape_x_intro,
	[ACT_escape_u_intro]             = bvn_action_escape_u_intro,
	[ACT_escape_x_d1]                = bvn_action_escape_x_d1,
	[ACT_escape_x_d2]                = bvn_action_escape_x_d2,
	[ACT_escape_u_open]              = bvn_action_escape_u_open,
	[ACT_escape_u_hex]               = bvn_action_escape_u_hex,
	[ACT_arr_string_intro]           = bvn_action_arr_string_intro,
	[ACT_symbol_intro]               = bvn_action_symbol_intro,
	[ACT_copy_symbol_byte]           = bvn_action_copy_symbol_byte,
	[ACT_to_symbol_outro]            = bvn_action_set_state,
	[ACT_reference_intro]            = bvn_action_reference_intro,
	[ACT_copy_reference_dot]         = bvn_action_copy_reference_dot,
	[ACT_copy_reference_byte]        = bvn_action_copy_reference_byte,
	[ACT_reference_index_open]       = bvn_action_reference_index_open,
	[ACT_reference_index_byte]       = bvn_action_reference_index_byte,
	[ACT_reference_index_close]      = bvn_action_reference_index_close,
	[ACT_to_reference_outro]         = bvn_action_set_state,
	[ACT_octet_stream_intro]         = bvn_action_octet_stream_intro,
	[ACT_struct_intro]               = bvn_action_struct_intro,
	[ACT_struct_outro]               = bvn_action_struct_outro,
	[ACT_first_comment_intro]        = bvn_action_first_comment_intro,
	[ACT_first_comment_byte]         = bvn_action_first_comment_byte,
	[ACT_first_comment_after_ef]     = bvn_action_first_comment_after_ef,
	[ACT_first_comment_after_ef_bb]  = bvn_action_first_comment_after_ef_bb,
	[ACT_first_comment_outro]        = bvn_action_first_comment_outro,
	[ACT_type_null_then_value_outro]     = bvn_action_type_null_then_value_outro,
	[ACT_type_null_then_new_array_value] = bvn_action_type_null_then_new_array_value,
	[ACT_type_null_then_array_outro]     = bvn_action_type_null_then_array_outro,
	[ACT_resync_skip]               = bvn_action_resync_skip,
	[ACT_resync_open_bracket]       = bvn_action_resync_open_bracket,
	[ACT_resync_close_bracket]      = bvn_action_resync_close_bracket,
	[ACT_resync_semicolon]          = bvn_action_resync_semicolon,
	[ACT_resync_string_intro]       = bvn_action_resync_string_intro,
	[ACT_resync_string_byte]        = bvn_action_resync_string_byte,
	[ACT_resync_string_escape]      = bvn_action_resync_string_escape,
	[ACT_resync_string_escape_byte] = bvn_action_resync_string_escape_byte,
	[ACT_resync_string_outro]       = bvn_action_resync_string_outro,
	[ACT_resync_comment_intro]      = bvn_action_resync_comment_intro,
	[ACT_resync_comment_byte]       = bvn_action_resync_comment_byte,
	[ACT_resync_comment_outro]      = bvn_action_resync_comment_outro,
	[ACT_resync_dot]                = bvn_action_resync_dot,
	[ACT_resync_resume_identifier]  = bvn_action_resync_resume_identifier,
	[ACT_inline_unit_intro]         = bvn_action_inline_unit_intro,
	[ACT_copy_inline_unit_byte]     = bvn_action_copy_inline_unit_byte,
	[ACT_to_inline_unit_outro]      = bvn_action_to_inline_unit_outro,
	[ACT_to_string_outro]           = bvn_action_set_state,
	[ACT_dtlit_after_y]             = bvn_action_dtlit_byte,
	[ACT_dtlit_month]               = bvn_action_dtlit_byte,
	[ACT_dtlit_after_m]             = bvn_action_dtlit_byte,
	[ACT_dtlit_day]                 = bvn_action_dtlit_byte,
	[ACT_dtlit_after_T]             = bvn_action_dtlit_byte,
	[ACT_dtlit_hour]                = bvn_action_dtlit_byte,
	[ACT_dtlit_after_h]             = bvn_action_dtlit_byte,
	[ACT_dtlit_min]                 = bvn_action_dtlit_byte,
	[ACT_dtlit_after_mi]            = bvn_action_dtlit_byte,
	[ACT_dtlit_sec]                 = bvn_action_dtlit_byte,
	[ACT_dtlit_zulu]                = bvn_action_dtlit_byte,
	[ACT_dtlit_frac]                = bvn_action_dtlit_byte,
	[ACT_dtlit_off_sign]            = bvn_action_dtlit_byte,
	[ACT_dtlit_off_hour]            = bvn_action_dtlit_byte,
	[ACT_dtlit_off_colon]           = bvn_action_dtlit_byte,
	[ACT_dtlit_off_min]             = bvn_action_dtlit_byte,
};
const state_t bvn_action_target_state[ACT__count] = {
	[ACT_ignore_comment_byte]       = ignore_comment_byte,
	[ACT_to_identifier_outro]       = identifier_outro,
	[ACT_to_number_outro]           = number_outro,
	[ACT_string_outro]              = string_outro_nosp,
	[ACT_to_string_outro]           = string_outro,
	[ACT_escape_from_copy]          = escape_from_copy,
	[ACT_to_symbol_outro]           = symbol_outro,
	[ACT_to_reference_outro]        = reference_outro,
	[ACT_tf_to_u]                   = tf_u,
	[ACT_tf_to_s]                   = tf_s,
	[ACT_tf_to_f]                   = tf_f,
	[ACT_tf_to_b]                   = tf_b,
	[ACT_tf_to_d]                   = tf_d,
	[ACT_tf_u_to_ui]                = tf_ui,
	[ACT_tf_u_to_ut]                = tf_ut,
	[ACT_dtlit_after_y]             = dtlit_after_y,
	[ACT_dtlit_month]               = dtlit_month,
	[ACT_dtlit_after_m]             = dtlit_after_m,
	[ACT_dtlit_day]                 = dtlit_day,
	[ACT_dtlit_after_T]             = dtlit_after_T,
	[ACT_dtlit_hour]                = dtlit_hour,
	[ACT_dtlit_after_h]             = dtlit_after_h,
	[ACT_dtlit_min]                 = dtlit_min,
	[ACT_dtlit_after_mi]            = dtlit_after_mi,
	[ACT_dtlit_sec]                 = dtlit_sec,
	[ACT_dtlit_zulu]                = dtlit_zulu,
	[ACT_dtlit_frac]                = dtlit_frac,
	[ACT_dtlit_off_sign]            = dtlit_off_sign,
	[ACT_dtlit_off_hour]            = dtlit_off_hour,
	[ACT_dtlit_off_colon]           = dtlit_off_colon,
	[ACT_dtlit_off_min]             = dtlit_off_min,
};
const state_t bvn_kw_advance_state[dimension_state] = {
	[tf_ui]      = tf_uin,
	[tf_ut]      = tf_utf,
	[tf_s]       = tf_si,   [tf_si]      = tf_sin,
	[tf_f]       = tf_fl,   [tf_fl]      = tf_flo,  [tf_flo]     = tf_floa,
	[tf_b]       = tf_bo,   [tf_bo]      = tf_boo,
	[tf_d]       = tf_da,   [tf_da]      = tf_dat,  [tf_dat]     = tf_date,
	[tf_date]    = tf_datet,[tf_datet]   = tf_dateti,
	[tf_dateti]  = tf_datetim,
};
