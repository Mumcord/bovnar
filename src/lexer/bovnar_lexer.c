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

#include <stdlib.h>
#include <string.h>
#include "bovnar.h"
#include "bvn_io_impl.h"
#include "bvn_lexer_impl.h"
#include "bvn_val_impl.h"
/*
 * ===========================================================================
 * Lexer / tokenizer
 * ===========================================================================
 *
 * The lexer is a table-driven state machine. For every (state, input byte)
 * pair, bvn_after_state_idx_table (generated in bovnar_state_table.c) yields
 * an action index; bvn_action_table maps that index to a handler function and
 * bvn_action_target_state gives the fall-through next state. This design was
 * chosen over a hand-written switch because the bovnar grammar has ~90 states
 * and the byte-classification logic is dense and regular: encoding it as a
 * flat table keeps the hot loop branch-light (one table lookup + one indirect
 * call per byte) and makes the grammar auditable as data rather than control
 * flow.
 *
 * As tokens complete, the lexer pushes raw tokens into the validator
 * (bvn_val_receive*), which performs the semantic layer. The lexer itself
 * only recognises lexical structure and enforces length/UTF-8 limits.
 *
 * Two cross-cutting concerns shape the rest of this file:
 *
 *  - Performance: a byte-at-a-time indirect-call loop is slow for the common
 *    case of long identifiers/strings/numbers. The "run LUT" tables below let
 *    the inner loop detect a maximal run of bytes that all self-loop in the
 *    current copy-state and bulk-memcpy them in one shot (bvn_try_bulk_run /
 *    bvn_try_bulk_ws), falling back to the per-byte path at run boundaries.
 *
 *  - Error recovery: with continue_on_error set, a lexical error does not
 *    abort the parse. Instead the machine enters the `resync` family of
 *    states (bvn_enter_resync + bvn_action_resync_*), discards bytes up to the
 *    next safe boundary (a top-level ';'), emits the structural close events
 *    the validator needs to stay balanced, and resumes — counting each event
 *    in recovery_count.
 */

/*
 * Run lookup tables: for each "copy byte" state, lut[b] == 1 iff byte b would
 * keep the machine in that same state (i.e. it is a legal continuation that
 * does not terminate the token). Built once from the authoritative transition
 * table so the two can never drift apart. These power the bulk-copy fast path.
 */
static uint8_t bvn_run_lut_string[256];
static uint8_t bvn_run_lut_ident[256];
static uint8_t bvn_run_lut_number[256];
static uint8_t bvn_run_lut_fraction[256];
static uint8_t bvn_run_lut_exp[256];
static uint8_t bvn_run_lut_symbol[256];
static uint8_t bvn_run_lut_inline_unit[256];
/*
 * A unit's spelling travels with the unit. Every site below that copies
 * parsed_unit into or out of an array frame copies its source text the same
 * way and under the same condition, which is the whole of what keeps
 * bvnr_data_t.unit_source describing value_unit and not some earlier one.
 */
static inline void bvn_usrc_save(bvn_array_frame_t* f, const bvnr_validator_t* v)
{
	f->saved_usrc_len = v->unit_src_len;
	memcpy(f->saved_usrc, v->unit_src, v->unit_src_len);
}
static inline void bvn_usrc_restore(bvnr_validator_t* v, const bvn_array_frame_t* f)
{
	v->unit_src_len = f->saved_usrc_len;
	memcpy(v->unit_src, f->saved_usrc, f->saved_usrc_len);
}
static uint8_t bvn_run_lut_reference[256];
static uint8_t bvn_run_lut_type[256];
static uint8_t bvn_run_lut_ws[256];
static uint8_t bvn_ws_accepting_state[dimension_state];
static bool    bvn_run_luts_inited = false;
/*
 * Mark every printable/high byte that the transition table maps to the given
 * "self" action (the action that copies a byte and stays in the same state).
 * Control bytes 0x09/0x0a/0x0d are deliberately excluded so the bulk path
 * never swallows line-affecting whitespace, keeping line/column accounting
 * exact.
 */
static void bvn_build_run_lut(uint8_t lut[256], state_t st, uint8_t self_action)
{
	for (int b = 0x20; b <= 0x7e; ++b) {
		if (b == 0x09 || b == 0x0a || b == 0x0d)
			continue;
		if (bvn_after_state_idx_table[st][b] == self_action)
			lut[b] = 1;
	}
	for (int b = 0x80; b <= 0xff; ++b) {
		if (bvn_after_state_idx_table[st][b] == self_action)
			lut[b] = 1;
	}
}
/*
 * Populate all run LUTs plus the whitespace-accepting-state table.
 *
 * This runs before main() on every supported toolchain — via the constructor
 * attribute on GCC/Clang, and via a .CRT$XCU registration on MSVC (just below
 * the definition) — so the tables are fully built single-threaded, before any
 * thread can be created, before the first lexer is ever used. That pre-main
 * guarantee is what makes the init data-race-free: distinct readers on distinct
 * threads never concurrently run this (a non-atomic `bvn_run_luts_inited` flag
 * would otherwise race on first concurrent use). The guard in bvn_lex_init()
 * is a belt-and-suspenders fallback for the (unreachable) case where neither
 * pre-main hook fired; it must not be the sole initialiser under threads.
 */
#if defined(__GNUC__) || defined(__clang__)
/* noinline is a compiler workaround, not a tuning choice. GCC 15.2's LTO
 * inliner dies copying this constructor's body into bvn_lex_init ("internal
 * compiler error: in copy_bb, at tree-inline.cc:2402") in an executable where
 * the writer reaches the reader. The call it leaves behind costs one
 * predictable branch per reader open. */
#  define BVN_LEX_CONSTRUCTOR __attribute__((constructor, noinline))
#else
#  define BVN_LEX_CONSTRUCTOR
#endif
BVN_LEX_CONSTRUCTOR
static void bvn_init_run_luts(void)
{
	if (bvn_run_luts_inited) return;
	bvn_build_run_lut(bvn_run_lut_string,      copy_string_byte,
	                  ACT_copy_string_byte);
	bvn_build_run_lut(bvn_run_lut_ident,       identifier_body,
	                  ACT_copy_identifier_byte);
	bvn_build_run_lut(bvn_run_lut_number,      copy_number_byte,
	                  ACT_copy_number_byte);
	bvn_build_run_lut(bvn_run_lut_fraction,    copy_fraction_byte,
	                  ACT_copy_fraction_byte);
	bvn_build_run_lut(bvn_run_lut_exp,         copy_exp_byte,
	                  ACT_copy_exp_byte);
	bvn_build_run_lut(bvn_run_lut_symbol,      symbol_body,
	                  ACT_copy_symbol_byte);
	bvn_build_run_lut(bvn_run_lut_inline_unit, inline_unit_body,
	                  ACT_copy_inline_unit_byte);
	bvn_build_run_lut(bvn_run_lut_reference,   reference_segment_body,
	                  ACT_copy_reference_byte);
	bvn_build_run_lut(bvn_run_lut_type,        copy_type_byte,
	                  ACT_copy_type_byte);
	bvn_run_lut_ws[0x09] = 1;
	bvn_run_lut_ws[0x0a] = 1;
	bvn_run_lut_ws[0x0b] = 1;
	bvn_run_lut_ws[0x0c] = 1;
	bvn_run_lut_ws[0x0d] = 1;
	bvn_run_lut_ws[0x20] = 1;
	for (int s = 0; s < (int)dimension_state; ++s)
		bvn_ws_accepting_state[s] = (
			bvn_after_state_idx_table[s][0x20] == ACT_ignore_whitespace &&
			bvn_after_state_idx_table[s][0x0a] == ACT_ignore_whitespace
		) ? 1u : 0u;
	bvn_run_luts_inited = true;
}
#if defined(_MSC_VER)
/*
 * MSVC has no constructor attribute. Place a pointer to the initialiser in the
 * CRT's C-initialiser section (.CRT$XCU); the CRT runs every function pointer in
 * that range before main(), giving the same single-threaded, pre-any-thread
 * guarantee as __attribute__((constructor)) above. The /include linker directive
 * keeps the pointer (and thus the .CRT$XCU entry) from being stripped when the
 * library is linked statically. The build targets x64, whose symbols carry no
 * leading underscore.
 */
#  pragma section(".CRT$XCU", read)
__declspec(allocate(".CRT$XCU"))
void (*bvn_init_run_luts_ctor)(void) = bvn_init_run_luts;
#  pragma comment(linker, "/include:bvn_init_run_luts_ctor")
#endif
static inline void bvn_set_error_pos(bvnr_reader_t* r, uint32_t byte,
	uint64_t offset)
{
	bvnr_lexer_t* l = &r->lex;
	r->val.error_line   = l->line;
	r->val.error_column = l->column;
	r->val.error_byte   = byte;
	r->val.error_offset = offset;
}
/*
 * Offset, in the STREAM, of the text byte the scanner consumed last.
 *
 * text_bytes counts what the text scanner has consumed and nothing else, which
 * is what max_text_bytes needs and is not a position: an octet stream's tags,
 * lengths and payload go past the scanner without being counted. Reporting
 * text_bytes - 1 as the error offset was therefore right up to the first octet
 * stream and short by its whole binary size after it -- in
 * ".a = <8 binary bytes>;\n.c = @;" the '@' is byte 20 and was reported as 13,
 * against a header that says to index the input buffer with this number.
 * octet_bytes is the other half of the count.
 */
static inline uint64_t bvn_last_text_offset(const bvnr_lexer_t* l)
{
	uint64_t consumed = l->text_bytes + l->octet_bytes;
	return consumed > 0 ? consumed - 1 : 0;
}
static inline void bvn_lexer_set_error(bvnr_reader_t* r, error_code_t err)
{
	r->val.last_error = err;
	bvn_set_error_pos(r, (uint32_t)r->lex.byte & 0xffu,
		bvn_last_text_offset(&r->lex));
}
static inline void bvn_notify_error(bvnr_reader_t* p)
{
	bvnr_validator_t* v = &p->val;
	if (v->on_error) {
		v->on_error(v->userdata, v->last_error,
					v->error_line, v->error_column,
					v->error_byte, v->error_offset);
	}
}
/*
 * Begin error recovery (only reached when continue_on_error is set).
 *
 * The tricky part is that the validator downstream tracks open arrays and
 * structs; if we just skipped bytes we would leave it with dangling open
 * scopes and every subsequent token would be misinterpreted. So before
 * switching to the `resync` state we synthesise the array-row-end events for
 * the arrays the broken statement itself opened -- those of the innermost open
 * struct level, see bvn_resync_close_own_arrays -- restoring each parent
 * frame's saved type/unit as we unwind. Struct nesting is left standing and
 * remembered in resync_saved_struct_nesting, and so is every array that
 * struct is an element of: a stray byte inside a struct should not silently
 * discard the whole enclosing object, and the events must keep nesting. For
 * the same reason recovery ends at the start of the next assignment as well as
 * at the next ';' — see bvn_action_resync_resume_identifier. All scratch state
 * (string buffers, UTF-8 progress, accumulator) is reset so the next good
 * token starts clean, and recovery_count is bumped for diagnostics.
 *
 * An error inside a COMMENT never gets here: bvn_recover_in_comment.
 */
/*
 * Close the arrays that were opened INSIDE the innermost open struct (or at top
 * level, when no struct is open) and leave every other one alone.
 *
 * Recovery used to close every open array, whatever it enclosed. An error in a
 * struct that is itself an array element --
 *
 *     .a = [{.x = @; .y = 5;}, {.x = 1;}]; .b = 2;
 *
 * -- therefore emitted ev_array_row_end for the array AROUND the struct, went
 * on to deliver ".y = 5" as a member of that still-open struct, and closed the
 * struct after the array it sits in: start and end events that do not nest,
 * which no consumer building anything from them survives (the canonical
 * observer wrote ".a=[{.x=<uint:8>];.y=..."). The array around the struct is
 * not broken; the member is. So only the arrays this struct level owns are
 * unwound, exactly as a ']' would unwind them, and the enclosing ones keep
 * their row counters, their annotation and their frames for the ',' or ']'
 * that follows the struct.
 *
 * open_struct_level is what tells the two apart, and it is the same test
 * bvn_action_value_outro and bvn_action_new_array_value make.
 */
static bool bvn_resync_close_own_arrays(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	while (l->array_nesting_level > 0 &&
	       l->arr_frames[l->array_nesting_level].open_struct_level ==
	       l->struct_nesting_level) {
		if (!bvn_val_receive_event(p, ev_array_row_end))
			return false;
		--l->array_nesting_level;
		bvn_array_frame_t *f = &l->arr_frames[l->array_nesting_level];
		l->curr_row_size          = f->saved_curr;
		l->array_row_size         = f->saved_row;
		p->val.value_type         = f->saved_vtype;
		p->val.parsed_unit        = f->saved_vunit;
		bvn_usrc_restore(&p->val, f);
		p->val.parsed_unit_serial = f->saved_unit_serial;
	}
	/* Everything above the innermost array still open is scratch now; its own
	 * open_struct_level is not (see bvn_action_value_outro). */
	uint32_t base = l->array_nesting_level;
	uint32_t hwm  = l->arr_used_max;
	if (hwm > base) {
		uint8_t open_level = l->arr_frames[base].open_struct_level;
		memset(l->arr_frames + base, 0,
		       (hwm - base) * sizeof(bvn_array_frame_t));
		l->arr_frames[base].open_struct_level = open_level;
	}
	l->arr_used_max = (uint16_t)base;
	if (!base) {
		l->curr_row_size  = 0;
		l->array_row_size = 0;
		l->array_items    = 0;
	}
	return true;
}
/*
 * An error INSIDE A COMMENT -- a control byte, or a byte that is not UTF-8,
 * which is what a Latin-1 comment in an otherwise clean file looks like.
 *
 * Nothing structural is wrong with the document: a comment carries no data,
 * and it ends at the end of its line whatever it contains. Recovery treated it
 * like any other fault and went to plain resync, which read the REST OF THE
 * COMMENT as code -- ".z = 0; # bad <ff> byte; .q = 9;" delivered ".q = 9" out
 * of the comment text, a '[', '{' or '"' after the bad byte opened a bracket
 * or a string nothing ever closed, so every assignment to the end of the
 * document was skipped and the read failed -- and which closed any array the
 * comment happened to sit in.
 *
 * So the error is reported and counted, the one byte is dropped, and the
 * comment simply goes on: no state is unwound and no value is lost. Returns
 * false when the lexer is not in a comment, and the caller recovers the
 * ordinary way.
 *
 * `ends_line` is for the one case in which the offending byte is the line end
 * itself -- a multi-byte sequence cut short by it. The byte is not looked at
 * again, so the comment is closed here. A first comment that needed this is
 * not a version directive, whatever it began with.
 */
static bool bvn_recover_in_comment(bvnr_reader_t* p, bool ends_line)
{
	bvnr_lexer_t* l = &p->lex;
	bool first;
	switch (l->next_state) {
	case comment_intro:
	case ignore_comment_byte:
		first = false;
		break;
	case first_comment_intro:
	case first_comment_after_ef:
	case first_comment_after_ef_bb:
		first = true;
		break;
	default:
		return false;
	}
	l->utf8_need = 0;
	l->utf8_lo   = 0;
	l->utf8_hi   = 0;
	p->val.error_line   = 0;
	p->val.error_column = 0;
	p->val.error_byte   = 0;
	p->val.error_offset = 0;
	++l->recovery_count;
	++l->skipped_bytes;
	if (first) {
		l->ver_checked = true;
		l->next_state  = ends_line ? first_bom : first_comment_intro;
	} else if (ends_line) {
		(void)bvn_action_comment_outro(p);
	}
	return true;
}
static bool bvn_enter_resync(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	l->resync_saved_struct_nesting = l->struct_nesting_level;
	if (!bvn_resync_close_own_arrays(p)) {
		p->val.value_type  = BVN_TYPE_PLAIN;
		p->val.parsed_unit = BVN_UNIT_NO_PREFIX(bu_none);
		return false;
	}
	l->utf8_need = 0;
	l->utf8_lo = 0;
	l->utf8_hi = 0;
	l->bom_len    = 0;
	l->str_len    = 0;
	l->type_len   = 0;
	l->inline_unit_len = 0;
	l->token_type = token_is_unknown;
	bvn_acc_reset(&p->val);
	p->val.value_type          = BVN_TYPE_PLAIN;
	p->val.parsed_unit         = BVN_UNIT_NO_PREFIX(bu_none);
	p->val.has_annotation_unit = false;
	p->val.error_line          = 0;
	p->val.error_column        = 0;
	p->val.error_byte          = 0;
	p->val.error_offset        = 0;
	/* The unit just went back to "none" under an array frame that may still be
	 * open; move the serial so that frame puts its own unit back. */
	p->val.parsed_unit_serial++;
	l->resync_array_depth  = 0;
	l->resync_struct_depth = 0;
	/*
	 * If the error fired while inside a string body or an escape sequence
	 * (incl. the spec-1.1 \x / \u escapes), the broken string's closing '"'
	 * is still ahead in the stream. Skip in the resync_string sub-machine so
	 * that '"' is consumed as the close — entering plain resync instead would
	 * treat it as the OPEN of a new string-to-skip and swallow every following
	 * good assignment up to the next quote (or EOF).
	 *
	 * With one exception, which this used to deny: a \x or \u{...} escape cut
	 * short BY the closing quote. In "\x4" and "\u{41" the byte the escape
	 * rejects is the quote that ends the string, so there is no closing quote
	 * ahead and waiting for one swallowed the rest of the document --
	 * ".a = \"\x4\"; .b = 2; .c = \"z\"; .d = 4;" lost .b, .c and .d and ended
	 * in error_got_incomplete_bvnr_stream. Those four states go to plain resync
	 * when the offending byte is the quote. (escape_from_copy is not among
	 * them: there a '"' is the escaped quote, never an error for being one.)
	 *
	 * string_intro is a string-body state too — it is where the FIRST body byte
	 * is read, and its transition row is byte-for-byte identical to
	 * copy_string_byte's. Leaving it out made the fix above depend on the string
	 * having at least one good character: ".a = \"<bad>\"; .b = 2;" fell to the
	 * default branch, entered plain resync, and lost .b and everything after it,
	 * while ".a = \"x<bad>\"; .b = 2;" recovered cleanly. Reachable from any
	 * string whose first byte is malformed UTF-8 or a rejected control byte —
	 * including the first byte of a continuation string ("a" "<bad>") and of an
	 * array element.
	 */
	switch (l->next_state) {
	case string_intro:
	case copy_string_byte:
	case escape_from_copy:
		l->next_state = resync_string;
		break;
	case esc_x1:
	case esc_x2:
	case esc_u_open:
	case esc_u_hex:
		l->next_state = (l->byte == '"') ? resync : resync_string;
		break;
	default:
		/* The offending byte may itself be a '"' in a state where a quote is
		 * illegal (after a number, inside an identifier, ...). That quote has
		 * just been consumed and OPENS a string, so plain resync would read its
		 * closing quote as another opening one — inverting the parity and
		 * swallowing every following assignment to the next quote or EOF.
		 * ".a = 1 \"x\"; .b = 2;" lost .b and everything after it.
		 *
		 * Likewise a '#' that is itself the offending byte ("42# note", the
		 * slip doc/03 §4.1 names) has opened a comment: plain resync read the
		 * comment's body as document text and delivered ".a = 41" out of
		 * "42# was: .a = 41;" (§13.2: the rest of a comment is never read). */
		l->next_state = (l->byte == '"') ? resync_string
			      : (l->byte == '#') ? resync_comment : resync;
		break;
	}
	++l->recovery_count;
	/* The byte the error was raised on is the first one discarded: it has
	 * already been counted in text_bytes, so the skipped region starts one
	 * earlier. bvn_resync_reset ends it before the boundary it resumes at. */
	l->resync_start_bytes  = l->text_bytes ? l->text_bytes - 1u : 0u;
	l->in_array_element    = false;
	return true;
}
/*
 * Escape decode table. The high bit (ESC_VALID) flags a legal escape letter;
 * the low byte holds the replacement character. A single table lookup replaces
 * a switch and lets bvn_decode_escape stay branch-free on the hot path. Only
 * the bovnar-approved escapes are present; everything else decodes as invalid.
 */
#define ESC_VALID 0x100u
static const uint16_t bvn_escape_lut[256] = {
	['t']  = ESC_VALID | '\t',  ['n']  = ESC_VALID | '\n',
	['v']  = ESC_VALID | '\v',  ['f']  = ESC_VALID | '\f',
	['r']  = ESC_VALID | '\r',  ['"']  = ESC_VALID | '"',
	['\\'] = ESC_VALID | '\\',
};
static inline bool bvn_decode_escape(bvnr_reader_t* p, uint8_t* out)
{
	uint16_t e = bvn_escape_lut[(uint8_t)p->lex.byte];
	if (!(e & ESC_VALID)) {
		bvn_lexer_set_error(p, error_illegal_escape_sequence);
		return false;
	}
	*out = (uint8_t)(e & 0xffu);
	return true;
}
/*
 * Append the current byte to a length-bounded buffer, raising `overflow` if
 * the per-token limit is hit. Every variable-length token (identifier, string,
 * number, symbol, reference) funnels through this so the configurable size
 * caps are enforced uniformly in one place — the defence against a hostile
 * stream trying to exhaust memory with one unbounded token. The thin
 * bvn_push_str/_number/_string/_reference wrappers just bind the right limit
 * and error code.
 */
static inline bool bvn_push_bounded(
	bvnr_reader_t* p, uint8_t* dst, uint32_t len, uint32_t max,
	error_code_t overflow, state_t after)
{
	if (len == max) {
		bvn_lexer_set_error(p, overflow);
		return false;
	}
	dst[len] = (uint8_t)p->lex.byte;
	p->lex.next_state = after;
	return true;
}
static inline bool bvn_push_str(bvnr_reader_t* p, uint32_t max,
	error_code_t overflow, state_t after)
{
	if (!bvn_push_bounded(p, p->lex.str_data, p->lex.str_len,
						  max, overflow, after))
		return false;
	++p->lex.str_len;
	return true;
}
static inline bool bvn_push_number_byte(bvnr_reader_t* p, state_t after)
{
	return bvn_push_str(p, p->lex.max_number_length,
		error_number_too_long, after);
}
static inline bool bvn_push_string_byte(bvnr_reader_t* p, state_t after)
{
	return bvn_push_str(p, p->lex.max_string_length,
		error_string_too_long, after);
}
static inline bool bvn_push_reference_byte(bvnr_reader_t* p, state_t after)
{
	return bvn_push_str(p, p->lex.max_reference_length,
		error_reference_too_long, after);
}
static bool bvn_replace_escaped_byte_impl(
	bvnr_reader_t* p, state_t after)
{
	uint8_t rep;
	if (p->lex.str_len == p->lex.max_string_length) {
		bvn_lexer_set_error(p, error_string_too_long);
		return false;
	}
	if (!bvn_decode_escape(p, &rep))
		return false;
	p->lex.str_data[p->lex.str_len++] = rep;
	p->lex.next_state = after;
	return true;
}
/*
 * Richer string escapes (spec 1.1): \xHH (one byte) and \u{1-6 hex} (a Unicode
 * scalar, UTF-8 encoded). Both are gated on the document declaring spec >= 1.1;
 * in a 1.0 (or unversioned) document the 'x'/'u' after a backslash is not a
 * recognised escape, so it stays error_illegal_escape_sequence exactly as a 1.0
 * reader would report. \x preserves the "strings are valid UTF-8" guarantee by
 * flagging the string for a whole-string UTF-8 check at finalize (see
 * bvn_lex_finalize): \xC3\xA9 spells "é", but \xFF alone is rejected there.
 */
#define bvn_doc_supports_1_1(l) bvn_lex_supports_1_1(l)
static inline int bvn_hexval(int32_t c)
{
	if (c >= '0' && c <= '9') return c - '0';
	if (c >= 'a' && c <= 'f') return c - 'a' + 10;
	if (c >= 'A' && c <= 'F') return c - 'A' + 10;
	return -1;
}
static inline bool bvn_string_push_byte(bvnr_reader_t* p, uint8_t b)
{
	if (p->lex.str_len == p->lex.max_string_length) {
		bvn_lexer_set_error(p, error_string_too_long);
		return false;
	}
	p->lex.str_data[p->lex.str_len++] = b;
	return true;
}
bool bvn_action_escape_x_intro(bvnr_reader_t* p)
{
	if (!bvn_doc_supports_1_1(&p->lex)) {
		bvn_lexer_set_error(p, error_illegal_escape_sequence);
		return false;
	}
	p->lex.next_state = esc_x1;
	return true;
}
bool bvn_action_escape_u_intro(bvnr_reader_t* p)
{
	if (!bvn_doc_supports_1_1(&p->lex)) {
		bvn_lexer_set_error(p, error_illegal_escape_sequence);
		return false;
	}
	p->lex.next_state = esc_u_open;
	return true;
}
/*
 * A `\x`/`\u{}` escape may not smuggle an ASCII control byte into a string that
 * a raw byte could not — the lexer rejects raw 0x00–0x08, 0x0E–0x1F and 0x7F in
 * strings (BVN_REJECT_ASCII_CTRL), and the canonical writer cannot emit them, so
 * accepting them via an escape would be an un-serialisable round-trip break.
 * The whitespace controls 0x09–0x0D stay allowed (they are exactly what the
 * named escapes \t \n \v \f \r produce). For arbitrary bytes, use an octet stream.
 */
static inline bool bvn_is_string_ctrl_reject(uint32_t b)
{
	return b <= 0x08u || (b >= 0x0Eu && b <= 0x1Fu) || b == 0x7Fu;
}
bool bvn_action_escape_x_d1(bvnr_reader_t* p)
{
	int v = bvn_hexval(p->lex.byte);
	if (v < 0) {
		bvn_lexer_set_error(p, error_illegal_escape_sequence);
		return false;
	}
	p->lex.esc_x_hi = (uint8_t)v;
	p->lex.next_state = esc_x2;
	return true;
}
bool bvn_action_escape_x_d2(bvnr_reader_t* p)
{
	int v = bvn_hexval(p->lex.byte);
	if (v < 0) {
		bvn_lexer_set_error(p, error_illegal_escape_sequence);
		return false;
	}
	uint8_t b = (uint8_t)((p->lex.esc_x_hi << 4) | v);
	if (bvn_is_string_ctrl_reject(b)) {
		bvn_lexer_set_error(p, error_unexpected_input_byte);
		return false;
	}
	if (!bvn_string_push_byte(p, b))
		return false;
	p->lex.str_has_raw_escape = true;
	p->lex.next_state = copy_string_byte;
	return true;
}
bool bvn_action_escape_u_open(bvnr_reader_t* p)
{
	if (p->lex.byte != '{') {
		bvn_lexer_set_error(p, error_illegal_escape_sequence);
		return false;
	}
	p->lex.esc_u_acc    = 0;
	p->lex.esc_u_digits = 0;
	p->lex.next_state   = esc_u_hex;
	return true;
}
bool bvn_action_escape_u_hex(bvnr_reader_t* p)
{
	if (p->lex.byte == '}') {
		if (p->lex.esc_u_digits == 0) {       /* empty \u{} */
			bvn_lexer_set_error(p, error_illegal_escape_sequence);
			return false;
		}
		uint32_t cp = p->lex.esc_u_acc;
		if (cp > 0x10FFFFu || (cp >= 0xD800u && cp <= 0xDFFFu)) {
			bvn_lexer_set_error(p, error_invalid_codepoint);
			return false;
		}
		/* an ASCII control scalar (other than the whitespace controls) is no
		 * more legal in a string via \u than as a raw byte */
		if (bvn_is_string_ctrl_reject(cp)) {
			bvn_lexer_set_error(p, error_unexpected_input_byte);
			return false;
		}
		uint8_t buf[4];
		uint32_t n;
		if (cp < 0x80u) {
			buf[0] = (uint8_t)cp; n = 1;
		} else if (cp < 0x800u) {
			buf[0] = (uint8_t)(0xC0u | (cp >> 6));
			buf[1] = (uint8_t)(0x80u | (cp & 0x3Fu)); n = 2;
		} else if (cp < 0x10000u) {
			buf[0] = (uint8_t)(0xE0u | (cp >> 12));
			buf[1] = (uint8_t)(0x80u | ((cp >> 6) & 0x3Fu));
			buf[2] = (uint8_t)(0x80u | (cp & 0x3Fu)); n = 3;
		} else {
			buf[0] = (uint8_t)(0xF0u | (cp >> 18));
			buf[1] = (uint8_t)(0x80u | ((cp >> 12) & 0x3Fu));
			buf[2] = (uint8_t)(0x80u | ((cp >> 6) & 0x3Fu));
			buf[3] = (uint8_t)(0x80u | (cp & 0x3Fu)); n = 4;
		}
		for (uint32_t i = 0; i < n; i++)
			if (!bvn_string_push_byte(p, buf[i]))
				return false;
		p->lex.next_state = copy_string_byte;
		return true;
	}
	int v = bvn_hexval(p->lex.byte);
	if (v < 0 || p->lex.esc_u_digits >= 6u) {
		bvn_lexer_set_error(p, error_illegal_escape_sequence);
		return false;
	}
	p->lex.esc_u_acc = (p->lex.esc_u_acc << 4) | (uint32_t)v;
	p->lex.esc_u_digits++;
	p->lex.next_state = esc_u_hex;
	return true;
}
static inline bool bvn_lex_number_begin(bvnr_reader_t* p, bool is_array)
{
	if (p->lex.str_len)
		return true;
	if (is_array)
		p->lex.token_type = token_is_array_number;
	else
		p->lex.token_type = token_is_number;
	return true;
}
/*
 * Emit the token that has just finished accumulating.
 *
 * Called at every token boundary (whitespace, separators, closing bytes). It
 * NUL-terminates the three scratch buffers — the validator and unit/type
 * parsers expect C strings even though lengths are also passed — packages them
 * into a bvnr_raw_token_t, and hands them to the sink (normally bvn_val_receive).
 * Structural tokens carry no payload and are filtered out early; an identifier
 * maps to an assignment-start event, everything else to a data event. On
 * success all scratch lengths are zeroed so the next token starts fresh.
 */
static bool bvn_lex_finalize(bvnr_reader_t* p, bvn_lex_sink_fn sink)
{
	bvnr_lexer_t* l = &p->lex;
	token_type_t  tt = l->token_type;
	if (tt == token_is_structure) {
		l->token_type = token_is_unknown;
		return true;
	}
	if (tt == token_is_unknown)
		return true;
	l->str_data[l->str_len] = 0;
	l->type_data[l->type_len] = 0;
	l->inline_unit_data[l->inline_unit_len] = 0;
	if (tt == token_is_identifier && !l->str_len) {
		bvn_lexer_set_error(p, error_empty_identifier);
		return false;
	}
	/* A \x escape (spec 1.1) can emit an arbitrary byte; the text-layer UTF-8
	 * check in the main loop only sees the raw input, not decoded escape output,
	 * so re-validate the assembled string here. This keeps the utf8-family
	 * guarantee intact: \xC3\xA9 is "é", but a lone \xFF is error_invalid_utf8_byte. */
	if (l->str_has_raw_escape &&
	    (tt == token_is_string || tt == token_is_array_string) &&
	    !bvn_validate_string(l->str_data, l->str_len)) {
		bvn_lexer_set_error(p, error_invalid_utf8_byte);
		return false;
	}
	bvnr_raw_token_t raw = {
		.type            = tt,
		.event           = (tt == token_is_identifier)
						   ? ev_assignment_start : ev_data,
		.str_data        = l->str_data,
		.str_len         = l->str_len,
		.type_data       = l->type_data,
		.type_len        = l->type_len,
		.inline_unit_data = l->inline_unit_data,
		.inline_unit_len  = l->inline_unit_len,
	};
	if (!sink(p, &raw))
		return false;
	l->type_len          = 0;
	l->str_len           = 0;
	l->inline_unit_len   = 0;
	l->str_has_raw_escape = false;
	l->token_type        = token_is_unknown;
	return true;
}
/*
 * ---------------------------------------------------------------------------
 * Action handlers
 * ---------------------------------------------------------------------------
 * One handler per ACT_* index. Each is invoked with the just-read byte already
 * in p->lex.byte and is responsible for (a) mutating accumulator/scratch state
 * and (b) setting p->lex.next_state. They return false (after recording an
 * error) to abort or trigger recovery. Most are tiny state-setters; the few
 * with real logic (arrays, structs, octet streams, resync) are commented
 * individually below.
 *
 * bvn_action_set_state is the degenerate handler for pure transitions that do
 * no work; the dispatch loop special-cases it to skip the indirect call.
 */
bool bvn_action_set_state(bvnr_reader_t* p)
{
	uint8_t idx = bvn_after_state_idx_table[p->lex.next_state][p->lex.byte];
	p->lex.next_state = bvn_action_target_state[idx];
	return true;
}
bool bvn_action_ignore_whitespace(bvnr_reader_t* p) { (void)p; return true; }
bool bvn_action_comment_intro(bvnr_reader_t* p)
{
	p->lex.last_state = p->lex.next_state;
	p->lex.next_state = comment_intro;
	return true;
}
bool bvn_action_comment_outro(bvnr_reader_t* p)
{
	state_t s = p->lex.last_state;
	/* number_outro_nosp is vestigial (unreachable since nan/inf/ninf
	 * became bare keywords); the first branch is dead but harmless. */
	if      (s == number_outro_nosp) s = number_outro;
	else if (s == string_outro_nosp) s = string_outro;
	p->lex.next_state = s;
	return true;
}
bool bvn_action_copy_utf8bom_byte(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	if (l->bom_len < 2) {
		if (!l->bom_len)
			l->last_state = l->next_state;
		l->bom[l->bom_len++] = (uint8_t)l->byte;
		l->next_state = utf8bom_intro;
		return true;
	}
	/* Spec 3.2: the BOM is legal only at byte offset 0. Testing last_state ==
	 * undefined was not enough — the [undefined] row self-loops on whitespace, so
	 * a BOM after a space or a newline slipped through. text_bytes counts what
	 * has been consumed, and the BOM's own three bytes are already in it. */
	if (l->bom[0] != 0xef || l->bom[1] != 0xbb || l->byte != 0xbf
		|| l->last_state != undefined || l->text_bytes != 3u) {
		bvn_lexer_set_error(p, error_invalid_byte_order_mark);
		return false;
	}
	l->bom_len    = 0;
	l->next_state = first_bom;
	return true;
}
bool bvn_action_identifier_intro(bvnr_reader_t* p)
{
	p->lex.str_len    = 0;
	p->lex.token_type = token_is_identifier;
	p->lex.next_state = identifier_intro;
	return true;
}
bool bvn_action_copy_identifier_byte(bvnr_reader_t* p)
{
	return bvn_push_str(p, p->lex.max_identifier_length,
		error_identifier_too_long, identifier_body);
}
bool bvn_action_value_intro(bvnr_reader_t* p)
{
	if (!bvn_lex_finalize(p, bvn_val_receive))
		return false;
	if (!bvn_val_on_value_intro(p))
		return false;
	p->lex.str_len          = 0;
	/* max_array_items is a budget for everything under one OUTERMOST '['. It
	 * was reset here for every assignment, and a struct member is an
	 * assignment: "[{.a=1;},{.a=1;},...]" restarted the count at each element
	 * and was never limited at all. Reset only where no array is open. */
	if (!p->lex.array_nesting_level)
		p->lex.array_items = 0;
	p->lex.token_type       = token_is_null_value;
	p->lex.in_array_element = false;
	bvn_acc_reset(&p->val);
	p->lex.next_state  = value_intro;
	return true;
}
bool bvn_action_value_outro(bvnr_reader_t* p)
{
	/* A ';' (or '}') here ends the whole value, so every '[' opened for it must
	 * already have been closed by a ']'. Accepting an unterminated array meant no
	 * ev_array_row_end was ever emitted and the level was silently zeroed, so the
	 * NEXT assignment's value was absorbed into the array and its key vanished:
	 * ".a = [1;  .b = 2;" validated as OK and converted to {"a":[1,2]} with no
	 * ".b" at all. Inside a struct the level was not even reset, so a top-level
	 * key afterwards migrated into the struct. The EBNF makes ']' mandatory. */
	if (p->lex.array_nesting_level > 0 &&
	    p->lex.arr_frames[p->lex.array_nesting_level].open_struct_level ==
	    p->lex.struct_nesting_level) {
		p->val.last_error = error_got_incomplete_bvnr_stream;
		return false;
	}
	if (!bvn_lex_finalize(p, bvn_val_receive))
		return false;
	if (!bvn_val_on_value_outro(p))
		return false;
	uint32_t base = p->lex.array_nesting_level;
	uint32_t hwm  = p->lex.arr_used_max;
	if (hwm > base) {
		/* arr_frames[base] is two things: scratch for an array nested inside
		 * the innermost open one (dead now, hence the wipe) and that open
		 * array's OWN open_struct_level, which is very much alive. Wiping it
		 * disarmed the unterminated-array check above for the rest of the
		 * array -- but only inside a struct, where the level is not 0 anyway:
		 * ".p = {.s = [{.a = [1];}; .q = {.a = [1];}];};" validated, and .q
		 * became the array's second element. */
		uint8_t open_level = p->lex.arr_frames[base].open_struct_level;
		memset(p->lex.arr_frames + base, 0,
		       (hwm - base) * sizeof(bvn_array_frame_t));
		p->lex.arr_frames[base].open_struct_level = open_level;
	}
	p->lex.arr_used_max = (uint16_t)base;
	if (!p->lex.struct_nesting_level) {
		p->lex.in_array_element    = false;
		p->lex.array_nesting_level = 0;
		p->lex.curr_row_size       = 0;
		p->lex.array_row_size      = 0;
	}
	p->lex.next_state = value_outro;
	return true;
}
bool bvn_action_type_intro(bvnr_reader_t* p)
{
	p->lex.token_type = token_is_type;
	/* A fresh body: no family ':' yet, and the parameter that has not started
	 * yet has no namespace. Both are per-annotation state. */
	p->lex.type_family_colon = false;
	p->lex.type_param_ns     = false;
	p->lex.next_state = type_intro;
	return true;
}
/*
 * Drop whitespace that ended up at the end of a parameter.
 *
 * Only a NAMESPACED parameter can accumulate whitespace at all (see
 * bvn_action_type_ws_byte), and there the space is a code byte -- so
 * "<float:64,udunits:m >" would otherwise reach the profile as "m " and fail on a
 * dangling multiplication, while "<float:64,m/s >" has always been legal. The
 * trailing run is whitespace beside the separator that closes the parameter,
 * which is exactly where the grammar allows it; the interior run is the part that
 * means something and is left alone. Called at the two bytes that end a
 * parameter: a depth-0 ',' and the '>'.
 */
static void bvn_type_trim_trailing_ws(bvnr_lexer_t* l)
{
	while (l->type_len > 0u
	       && (l->type_data[l->type_len - 1u] == 0x20u
	           || l->type_data[l->type_len - 1u] == 0x09u))
		--l->type_len;
}
bool bvn_action_type_outro(bvnr_reader_t* p)
{
	bvn_type_trim_trailing_ws(&p->lex);
	if (!bvn_lex_finalize(p, bvn_val_receive))
		return false;
	p->lex.next_state = type_outro;
	return true;
}
bool bvn_action_copy_type_byte(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	/*
	 * "float_fix" and "float_dec" are ONE keyword each. The keyword machine
	 * stops at "float" and the "_fix" / "_dec" arrive here as ordinary body
	 * bytes -- from type_body_outro, where whitespace and comments are
	 * ignorable, so "<float _fix:32,q8>" and "<float # c <LF> _dec:64>" were
	 * read as float_fix and float_dec: whitespace in the middle of a token,
	 * which is the thing error_type_param_whitespace exists to refuse. The
	 * suffix has to be the very next byte after the 't'.
	 */
	if (l->byte == '_' && l->type_len == 5u &&
	    memcmp(l->type_data, "float", 5u) == 0 &&
	    l->text_bytes != l->type_kw_end + 1u) {
		bvn_lexer_set_error(p, error_type_param_whitespace);
		return false;
	}
	if (l->type_len == BVN_TYPE_BUF_CAP - 1u) {
		bvn_lexer_set_error(p, error_type_too_long);
		return false;
	}
	l->type_data[l->type_len++] = (uint8_t)l->byte;
	l->next_state = copy_type_byte;
	return true;
}
/*
 * Brace/bracket depth of the parameter bytes accumulated so far.
 *
 * A ',' inside a UCUM annotation or a bracketed atom is part of the code, not a
 * parameter separator (doc/11 §2.4) — "ucum:mL{cells,tot}" is ONE parameter —
 * and the same goes for a ':' appearing in annotation text. Only a separator at
 * depth zero may open the whitespace window below, or "{cells, tot}" would have
 * its space deleted and §3.4's promise to preserve annotation text verbatim
 * would stop being true.
 *
 * The depth is recomputed from type_data rather than counted as the bytes go by.
 * That is deliberate: the bulk-copy fast path memcpy's a run of parameter bytes
 * without ever entering an action, so a running counter would miss every brace
 * it copied, and taking '{' and '}' out of the run class to fix that would slow
 * the hot path to serve a byte that arrives at most a few times per annotation.
 * A separator is rare; a re-scan of at most 255 bytes when one arrives is not
 * worth avoiding, and it cannot fall out of step with the buffer it reads.
 */
static uint32_t bvn_type_param_depth(const uint8_t* d, uint32_t len)
{
	uint32_t depth = 0;
	for (uint32_t i = 0; i < len; i++) {
		if (d[i] == '{' || d[i] == '[')
			++depth;
		else if ((d[i] == '}' || d[i] == ']') && depth)
			--depth;
	}
	return depth;
}
/*
 * A ',' or ':' at depth zero: copied like any other parameter byte, but it
 * leaves the machine at type_body_outro, where whitespace is ignorable, instead
 * of copy_type_byte, where it is not. Nested, it is an ordinary code byte and
 * behaves exactly like bvn_action_copy_type_byte.
 */
bool bvn_action_copy_type_sep_byte(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	bool nested = bvn_type_param_depth(l->type_data, l->type_len) != 0u;
	if (!nested) {
		/* This byte ends the parameter, so whitespace that ran up to it was
		 * beside a separator after all. */
		if (l->byte == ',')
			bvn_type_trim_trailing_ws(l);
		/* Which colon is this? The first at depth 0 separates the type family
		 * from its parameter list; a later one opens a profile namespace, and
		 * that is what decides whether whitespace inside THIS parameter is a
		 * code byte or an error. A ',' starts a fresh parameter, which has no
		 * namespace until a ':' gives it one. */
		if (l->byte == ':') {
			if (l->type_family_colon)
				l->type_param_ns = true;
			else
				l->type_family_colon = true;
		} else {
			l->type_param_ns = false;
		}
	}
	if (l->type_len == BVN_TYPE_BUF_CAP - 1u) {
		bvn_lexer_set_error(p, error_type_too_long);
		return false;
	}
	l->type_data[l->type_len++] = (uint8_t)l->byte;
	l->next_state = nested ? copy_type_byte : type_body_outro;
	return true;
}
/*
 * Whitespace arriving in the MIDDLE of a parameter. Two outcomes, and which one
 * depends on whether the parameter is namespaced.
 *
 * NATIVE PARAMETER -> error_type_param_whitespace, via type_body_ws. A space
 * cannot be part of a native unit or a width: "<float:64,k g>" was accepted as
 * k~g and "<uint:6 4>" as a 64-bit width, which is a wrong unit and a wrong type
 * produced silently, and doc/12 never derived either of them.
 *
 * PROFILE PARAMETER -> the byte is COPIED, and the vocabulary decides. This is
 * the case that was previously indistinguishable from the one above and is the
 * reason the whole space was deleted for so long. UDUNITS multiplies with a
 * space, and CF's commonest spelling of a flux is "kg m-2 s-1"; that expression
 * now reaches the UDUNITS parser as itself instead of as the "kg m-2 s-1" with
 * the spaces deleted -- which was "kgm-2s-1", a different unit, or in the
 * two-token case "ms-1", a reciprocal millisecond where a speed was meant. A
 * UCUM annotation keeps its spacing, which is what doc/11 §3.4 has always
 * promised. Every other vocabulary refuses the space through its own grammar as
 * error_unit_illegal, which is the right layer for it: "no such code" rather
 * than "no whitespace here".
 *
 * ONLY SPACE AND TAB. LF/CR/VT/FF keep going to type_body_ws even in a profile
 * parameter: no vocabulary spells a unit across a line, so admitting one would
 * buy nothing and would let a malformed code swallow a document's layout the way
 * an unterminated bracket would.
 */
bool bvn_action_type_ws_byte(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	if (!l->type_param_ns || (l->byte != 0x20 && l->byte != 0x09)) {
		l->next_state = type_body_ws;
		return true;
	}
	if (l->type_len == BVN_TYPE_BUF_CAP - 1u) {
		bvn_lexer_set_error(p, error_type_too_long);
		return false;
	}
	l->type_data[l->type_len++] = (uint8_t)l->byte;
	l->next_state = copy_type_byte;
	return true;
}
/*
 * A byte that would have been a legal parameter byte, arriving after whitespace
 * that split a parameter in two. Reported as its own code rather than as
 * error_unexpected_input_byte because the byte itself is not the problem and the
 * fix is specific: delete the space. "<float:64,k g>" and "<uint:6 4>" both land
 * here, and so does the space-multiplied UDUNITS spelling "udunits:m s-1" that
 * doc/11 §13.2 warns about — for which the answer is "m*s-1", not a narrower
 * unit table. A byte that is illegal anywhere in an annotation still fails as
 * error_unexpected_input_byte; only bytes that WERE accepted here before reach
 * this action.
 */
bool bvn_action_type_param_whitespace(bvnr_reader_t* p)
{
	bvn_lexer_set_error(p, error_type_param_whitespace);
	return false;
}
bool bvn_action_neg_number_intro(bvnr_reader_t* p)
{
	bool is_arr = p->lex.in_array_element;
	if (!bvn_lex_number_begin(p, is_arr)) return false;
	return bvn_push_number_byte(p, neg_number_intro);
}
bool bvn_action_copy_number_byte(bvnr_reader_t* p)
{
	bool is_arr = p->lex.in_array_element;
	if (!bvn_lex_number_begin(p, is_arr)) return false;
	return bvn_push_number_byte(p, copy_number_byte);
}
bool bvn_action_zero_intro(bvnr_reader_t* p)
{
	bool is_arr = p->lex.in_array_element;
	if (!bvn_lex_number_begin(p, is_arr)) return false;
	return bvn_push_number_byte(p, zero_intro);
}
bool bvn_action_fraction_intro(bvnr_reader_t* p)
{
	bool is_arr = p->lex.in_array_element;
	if (!bvn_lex_number_begin(p, is_arr)) return false;
	p->val.acc_has_dot = true;
	return bvn_push_number_byte(p, fraction_intro);
}
bool bvn_action_fraction_no_int(bvnr_reader_t* p)
{
	bool is_arr = p->lex.in_array_element;
	if (!bvn_lex_number_begin(p, is_arr)) return false;
	p->val.acc_has_dot = true;
	return bvn_push_number_byte(p, fraction_no_int);
}
bool bvn_action_copy_fraction_byte(bvnr_reader_t* p)
{
	return bvn_push_number_byte(p, copy_fraction_byte);
}
/*
 * '[' — open an array level.
 *
 * bovnar arrays are rectangular/multi-dimensional, so each nesting level needs
 * its own bookkeeping: the per-row element count and the established row size
 * to check ragged rows against. We push the parent level's counters AND its
 * value-type/unit onto arr_frames[level] (a manually managed stack sized at
 * init to max_array_nesting+1) so they can be restored on the matching ']'.
 * Saving type/unit per frame is what lets an inner array inherit, then later
 * not clobber, the element type annotation of its parent. in_dim_seq carries
 * the expected row width for subsequent sibling rows of a multi-dim array.
 */
bool bvn_action_array_intro(bvnr_reader_t* p)
{
	if (p->lex.max_array_items &&
		p->lex.array_items == p->lex.max_array_items) {
		bvn_lexer_set_error(p, error_too_many_array_items);
		return false;
	}
	uint64_t level = p->lex.array_nesting_level;
	if (level == p->lex.max_array_nesting) {
		bvn_lexer_set_error(p, error_array_nesting_too_high);
		return false;
	}
	++p->lex.array_items;
	bvn_array_frame_t *f = &p->lex.arr_frames[level];
	f->saved_curr  = p->lex.curr_row_size;
	f->saved_row   = p->lex.array_row_size;
	f->saved_vtype = p->val.value_type;
	f->saved_vunit = p->val.parsed_unit;
	bvn_usrc_save(f, &p->val);
	f->saved_unit_serial = p->val.parsed_unit_serial;
	p->lex.curr_row_size  = 0;
	/* UINT64_MAX = "row width not yet established" (an empty row has width 0,
	 * a real value, so 0 can no longer double as the unset sentinel). */
	p->lex.array_row_size = f->in_dim_seq ? f->dim_row_size : UINT64_MAX;
	++p->lex.curr_row_size;
	/*
	 * The array we are about to open tracks its OWN '/'-dimension sequence in
	 * the child frame (arr_frames[level + 1]). Start that frame clean: a row
	 * width established by a *previous, unrelated* array instance at this depth
	 * (a sibling branch reached via the parent's own '/' or ',') must never
	 * leak in and falsely constrain this fresh array's nested rows. Only a '/'
	 * occurring inside this very array may later set in_dim_seq here. Without
	 * this reset, "[[1,2]/[3,4]]/[[5,6,7]/[8,9,10]]" wrongly rejects the second
	 * branch's inner rows against the first branch's width. The index is in
	 * range: the nesting guard above rejected level == max_array_nesting, and
	 * arr_frames is sized max_array_nesting + 1.
	 */
	bvn_array_frame_t *child = &p->lex.arr_frames[level + 1u];
	child->in_dim_seq   = false;
	child->dim_row_size = 0;
	child->open_struct_level = p->lex.struct_nesting_level;
	p->lex.token_type       = token_is_null_value;
	p->lex.in_array_element = true;
	bvn_acc_reset(&p->val);
	if (!bvn_val_on_array_intro(p))
		return false;
	++p->lex.array_nesting_level;
	if (p->lex.array_nesting_level > p->lex.arr_used_max)
		p->lex.arr_used_max = p->lex.array_nesting_level;
	p->lex.next_state = array_intro;
	return true;
}
/*
 * ']' — close the current array level. Finalises any pending element, asks the
 * validator to verify this row's width against the established row size, then
 * pops the saved frame: the just-closed row's width becomes the parent frame's
 * dim_row_size (so the parent can enforce that sibling rows match), and the
 * parent's counters/type are restored. The unit_serial guard avoids
 * needlessly overwriting a unit that the inner scope never changed.
 */
/*
 * A ']' closes the innermost '[' -- and only where no struct has been opened
 * inside it since. The ',' and the ';' were given this test in the round
 * before; the ']' itself was not, so ".a = [{.x = 1];};" validated: the ']'
 * closed the array from inside its own element, ev_array_row_end arrived
 * before the ev_struct_end of the struct the array contains, and the
 * mismatched brackets were a document. Same test as bvn_action_new_array_value.
 */
static inline bool bvn_array_close_allowed(bvnr_reader_t* p)
{
	if (!p->lex.array_nesting_level ||
	    p->lex.arr_frames[p->lex.array_nesting_level].open_struct_level !=
	    p->lex.struct_nesting_level) {
		bvn_lexer_set_error(p, error_unexpected_input_byte);
		return false;
	}
	return true;
}
bool bvn_action_array_outro(bvnr_reader_t* p)
{
	if (!bvn_array_close_allowed(p))
		return false;
	if (!bvn_lex_finalize(p, bvn_val_receive))
		return false;
	if (!bvn_val_on_array_outro(p, p->lex.curr_row_size,
								&p->lex.array_row_size))
		return false;
	--p->lex.array_nesting_level;
	uint64_t level = p->lex.array_nesting_level;
	bvn_array_frame_t *f = &p->lex.arr_frames[level];
	f->dim_row_size       = p->lex.array_row_size;
	p->lex.curr_row_size  = f->saved_curr;
	p->lex.array_row_size = f->saved_row;
	p->val.value_type     = f->saved_vtype;
	if (p->val.parsed_unit_serial != f->saved_unit_serial) {
		p->val.parsed_unit         = f->saved_vunit;
		bvn_usrc_restore(&p->val, f);
		p->val.parsed_unit_serial  = f->saved_unit_serial;
	}
	p->lex.next_state = array_outro;
	return true;
}
/*
 * ']' reached directly from the array_intro state — only whitespace or comments
 * appeared between '[' and ']'. This is an EMPTY array row (zero elements).
 * array_intro pre-seeds the first element position with a null placeholder
 * (curr_row_size == 1, token_is_null_value); an empty row must NOT finalise it,
 * so — unlike bvn_action_array_outro — this emits no ev_data, reports a row
 * width of 0, and then pops the frame identically. "[]" is therefore distinct
 * from "[null]" (one null element). A width of 0 is a real width: an empty
 * '/'-row may only sit beside other empty rows ([]/[] is valid; []/[1] is a
 * row-size mismatch).
 */
bool bvn_action_array_outro_empty(bvnr_reader_t* p)
{
	if (!bvn_array_close_allowed(p))
		return false;
	/* Clear the null placeholder array_intro pre-seeded. We skip
	 * bvn_lex_finalize (which would emit it as an element AND reset the token),
	 * so reset the pending-token state here — otherwise the lingering
	 * token_is_null_value would be emitted as a phantom value when the
	 * enclosing assignment/element is finalised. */
	p->lex.token_type = token_is_unknown;
	p->lex.str_len    = 0u;
	p->lex.curr_row_size = 0u;
	if (!bvn_val_on_array_outro(p, 0u, &p->lex.array_row_size))
		return false;
	--p->lex.array_nesting_level;
	uint64_t level = p->lex.array_nesting_level;
	bvn_array_frame_t *f = &p->lex.arr_frames[level];
	f->dim_row_size       = p->lex.array_row_size;
	p->lex.curr_row_size  = f->saved_curr;
	p->lex.array_row_size = f->saved_row;
	p->val.value_type     = f->saved_vtype;
	if (p->val.parsed_unit_serial != f->saved_unit_serial) {
		p->val.parsed_unit         = f->saved_vunit;
		bvn_usrc_restore(&p->val, f);
		p->val.parsed_unit_serial  = f->saved_unit_serial;
	}
	p->lex.next_state = array_outro;
	return true;
}
bool bvn_action_new_array_value(bvnr_reader_t* p)
{
	if (!p->lex.array_nesting_level) {
		bvn_lexer_set_error(p, error_unexpected_input_byte);
		return false;
	}
	/* A ',' separates the elements of the innermost '[' -- and only where no
	 * struct has been opened inside it since. "[{.x = 7, 1;}]" used to read as
	 * two values for the one key. Same test bvn_action_value_outro makes. */
	if (p->lex.arr_frames[p->lex.array_nesting_level].open_struct_level !=
	    p->lex.struct_nesting_level) {
		bvn_lexer_set_error(p, error_unexpected_input_byte);
		return false;
	}
	if (!bvn_lex_finalize(p, bvn_val_receive))
		return false;
	if (p->lex.max_array_items &&
		p->lex.array_items == p->lex.max_array_items) {
		bvn_lexer_set_error(p, error_too_many_array_items);
		return false;
	}
	++p->lex.array_items;
	p->lex.str_len          = 0;
	p->lex.in_array_element = true;
	bvn_acc_reset(&p->val);
	if (!bvn_val_on_new_array_value(p, p->lex.curr_row_size,
									p->lex.array_row_size))
		return false;
	p->lex.arr_frames[p->lex.array_nesting_level].in_dim_seq = false;
	bvn_array_frame_t *par = &p->lex.arr_frames[p->lex.array_nesting_level - 1u];
	p->val.value_type  = par->saved_vtype;
	if (p->val.parsed_unit_serial != par->saved_unit_serial) {
		p->val.parsed_unit         = par->saved_vunit;
		bvn_usrc_restore(&p->val, par);
		p->val.parsed_unit_serial  = par->saved_unit_serial;
	}
	++p->lex.curr_row_size;
	p->lex.token_type = token_is_null_value;
	p->lex.next_state = new_array_value;
	return true;
}
/*
 * Dimension separator between rows of a multi-dimensional array. Emits
 * ev_array_dim_start and flags the current frame as a dimension sequence so
 * the next opened row inherits the sibling row width to validate against.
 */
bool bvn_action_array_dim_sep(bvnr_reader_t* p)
{
	if (!bvn_val_receive_event(p, ev_array_dim_start))
		return false;
	p->lex.arr_frames[p->lex.array_nesting_level].in_dim_seq = true;
	p->lex.next_state = array_dim_sep;
	return true;
}
bool bvn_action_exp_intro(bvnr_reader_t* p)
{
	bool is_arr = p->lex.in_array_element;
	if (!bvn_lex_number_begin(p, is_arr)) return false;
	p->val.acc_has_exp = true;
	return bvn_push_number_byte(p, exp_intro);
}
bool bvn_action_exp_sign_intro(bvnr_reader_t* p)
{
	return bvn_push_number_byte(p, exp_sign_intro);
}
bool bvn_action_copy_exp_byte(bvnr_reader_t* p)
{
	return bvn_push_number_byte(p, copy_exp_byte);
}
/*
 * ISO-8601 datetime literal accumulation (spec 1.1). Shared by every dtlit_*
 * transition: append the current byte to str_data (the number scratch buffer,
 * so the byte/length cap is the number cap) and advance to the per-cell target
 * state recorded in bvn_action_target_state — exactly the bvn_action_set_state
 * idiom, but copying the byte. The token keeps the number/array-number type set
 * by copy_number_byte on the year digits; the validator recognises the ISO
 * shape in str_data and converts it to epoch seconds.
 */
bool bvn_action_dtlit_byte(bvnr_reader_t* p)
{
	uint8_t idx = bvn_after_state_idx_table[p->lex.next_state][p->lex.byte];
	return bvn_push_number_byte(p, bvn_action_target_state[idx]);
}
bool bvn_action_kw_advance(bvnr_reader_t* p)
{
	p->lex.next_state = bvn_kw_advance_state[p->lex.next_state];
	return true;
}
/*
 * The grammar recognises the type-family keywords (uint/sint/float/utf8/bool)
 * letter-by-letter via the tf_* states. Once matched, bvn_tf_family_done writes
 * the canonical spelling into type_data so downstream parsing sees a normal
 * type token. (The special floats nan/inf/ninf are no longer lexed here: they
 * are bare reserved keywords reclassified from symbols by the validator, like
 * null/true/false.)
 */
static bool bvn_tf_family_done(bvnr_reader_t* p,
	const char* name, uint32_t len, state_t after)
{
	memcpy(p->lex.type_data, name, len);
	p->lex.type_len   = (uint8_t)len;
	p->lex.next_state = after;
	return true;
}
bool bvn_action_tf_uint_done(bvnr_reader_t* p)
	{ return bvn_tf_family_done(p, "uint",      4, type_body_outro); }
bool bvn_action_tf_sint_done(bvnr_reader_t* p)
	{ return bvn_tf_family_done(p, "sint",      4, type_body_outro); }
bool bvn_action_tf_float_done(bvnr_reader_t* p)
{
	/* Where the keyword ended, for bvn_action_copy_type_byte. */
	p->lex.type_kw_end = p->lex.text_bytes;
	return bvn_tf_family_done(p, "float",     5, type_body_outro);
}
bool bvn_action_tf_utf8_done(bvnr_reader_t* p)
	{ return bvn_tf_family_done(p, "utf8",      4, type_body_outro); }
bool bvn_action_tf_bool_done(bvnr_reader_t* p)
	{ return bvn_tf_family_done(p, "bool",      4, type_body_outro); }
bool bvn_action_tf_datetime_done(bvnr_reader_t* p)
	{ return bvn_tf_family_done(p, "datetime",  8, type_body_outro); }
bool bvn_action_string_intro(bvnr_reader_t* p)
{
	p->lex.token_type = p->lex.in_array_element
		? token_is_array_string : token_is_string;
	p->lex.next_state = string_intro;
	return true;
}
bool bvn_action_copy_string_byte(bvnr_reader_t* p)
{
	return bvn_push_string_byte(p, copy_string_byte);
}
bool bvn_action_replace_escaped_byte(bvnr_reader_t* p)
{
	return bvn_replace_escaped_byte_impl(p, copy_string_byte);
}
bool bvn_action_arr_string_intro(bvnr_reader_t* p)
{
	p->lex.token_type = token_is_array_string;
	p->lex.next_state = string_intro;
	return true;
}
bool bvn_action_symbol_intro(bvnr_reader_t* p)
{
	p->lex.str_len    = 0;
	p->lex.token_type = token_is_symbol;
	return bvn_push_str(p, p->lex.max_symbol_length,
		error_symbol_too_long, symbol_body);
}
bool bvn_action_copy_symbol_byte(bvnr_reader_t* p)
{
	return bvn_push_str(p, p->lex.max_symbol_length,
		error_symbol_too_long, symbol_body);
}
bool bvn_action_reference_intro(bvnr_reader_t* p)
{
	p->lex.str_len    = 0;
	p->lex.token_type = token_is_reference;
	p->lex.next_state = reference_intro;
	return true;
}
bool bvn_action_copy_reference_dot(bvnr_reader_t* p)
{
	return bvn_push_reference_byte(p, reference_segment_intro);
}
bool bvn_action_copy_reference_byte(bvnr_reader_t* p)
{
	/* After an index only '.', '[' or the end of the reference may follow
	 * (reference = "&" ref-segment {ref-segment | ref-index}). An index close
	 * returns to reference_segment_body, whose row takes body bytes, so
	 * "&.a[0]b" was read as the path ".a[0]b" -- which the grammar does not
	 * derive and bvn_dom_lookup cannot resolve. A ']' is in the stored path
	 * only as the close of an index, so the byte before says where we are.
	 * (bvn_try_bulk_run makes the same test before copying a run.) */
	if (p->lex.str_len > 0u &&
	    p->lex.str_data[p->lex.str_len - 1u] == ']') {
		bvn_lexer_set_error(p, error_unexpected_input_byte);
		return false;
	}
	return bvn_push_reference_byte(p, reference_segment_body);
}
/*
 * Array indexing inside a reference path (spec 1.1): &.matrix[0][1]. The bytes
 * are captured verbatim into the path string — references are stored unresolved,
 * so the index is only interpreted later by bvn_dom_lookup at the DOM layer. A
 * '[' is gated on a declared spec >= 1.1; in a 1.0/unversioned document it stays
 * the error_unexpected_input_byte a 1.0 reader reports. The reference_index
 * sub-state is what lets a ']' that closes an index be told apart from a ']'
 * that closes an enclosing array (which still ends the reference). */
bool bvn_action_reference_index_open(bvnr_reader_t* p)
{
	if (!bvn_lex_supports_1_1(&p->lex)) {
		bvn_lexer_set_error(p, error_unexpected_input_byte);
		return false;
	}
	/* require at least one digit before ']' (reference_index_first has no ']'
	 * transition), so an empty "[]" is rejected. With the test in
	 * bvn_action_copy_reference_byte -- nothing but '.', '[' or the end after an
	 * index -- the lexer accepts exactly the paths bvn_dom_lookup can walk.
	 * bvn_validate_reference on the write side is still wider by that one
	 * case: it lets ".a[0]b" through. */
	return bvn_push_reference_byte(p, reference_index_first);
}
bool bvn_action_reference_index_byte(bvnr_reader_t* p)
{
	return bvn_push_reference_byte(p, reference_index);
}
bool bvn_action_reference_index_close(bvnr_reader_t* p)
{
	return bvn_push_reference_byte(p, reference_segment_body);
}
/*
 * Entry to a binary octet stream. This only flips the state and signals the
 * validator; the actual length-prefixed binary framing is read out-of-band by
 * bvn_read_octet_stream because that data is not text and must bypass the
 * UTF-8/whitespace machinery entirely (see the main loop in bvn_lex_run).
 */
bool bvn_action_octet_stream_intro(bvnr_reader_t* p)
{
	/*
	 * spec 1.2 -- a consumer can refuse the binary half of the format outright.
	 * The check belongs here, at the opening 0x00, and not after the region has
	 * been read: the point of the flag is that this document must not have
	 * reached a text-shaped pipeline at all, so the earliest possible refusal is
	 * the useful one, and it costs nothing to reject before consuming a payload
	 * that may be megabytes.
	 */
	if (p->lex.text_only) {
		bvn_lexer_set_error(p, error_octet_stream_forbidden);
		return false;
	}
	p->lex.token_type = token_is_octet_stream;
	p->lex.next_state = octet_stream_intro;
	return bvn_val_receive_event(p, ev_octet_stream_start);
}
bool bvn_action_struct_intro(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	if (l->struct_nesting_level == l->max_struct_nesting) {
		bvn_lexer_set_error(p, error_struct_nesting_too_high);
		return false;
	}
	/* A struct takes no type annotation -- its members carry their own. An
	 * annotation in force here (written before the struct, or inherited from
	 * the array the struct is an element of) used to survive until the first
	 * member's key and fail there, which let it through before an EMPTY
	 * struct: <utf8> {} was accepted and <utf8> {.x = "s";} was not. */
	if (p->val.value_type.family != vt_plain) {
		bvn_lexer_set_error(p, error_type_value_mismatch);
		return false;
	}
	if (!bvn_val_receive_event(p, ev_struct_start))
		return false;
	++l->struct_nesting_level;
	l->token_type = token_is_structure;
	l->next_state = struct_intro;
	return true;
}
bool bvn_action_struct_outro(bvnr_reader_t* p)
{
	if (!p->lex.struct_nesting_level) {
		bvn_lexer_set_error(p, error_illegal_struct_close);
		return false;
	}
	--p->lex.struct_nesting_level;
	if (!bvn_val_receive_event(p, ev_struct_end))
		return false;
	p->lex.next_state = struct_outro;
	return true;
}
/*
 * Parse the content of a leading version directive — the bytes *after* the
 * leading '#', e.g. "!bovnar 1.1". Returns:
 *   BVN_VERDIR_NONE    — no "!bovnar" prefix; this is an ordinary comment.
 *   BVN_VERDIR_INVALID — the prefix matched but the version is malformed.
 *   BVN_VERDIR_OK      — well-formed; major/minor are filled.
 * The grammar is: "!bovnar" ws+ uint "." uint ws* (end). Each uint is decimal
 * with no leading zero (except a bare "0") and must fit in uint16.
 */
bvn_verdir_t bvn_parse_version_directive(
	const uint8_t* s, uint32_t len, uint16_t* major, uint16_t* minor)
{
	static const char pfx[] = "!bovnar";
	const uint32_t pfx_len = 7u;
	if (len < pfx_len || memcmp(s, pfx, pfx_len) != 0)
		return BVN_VERDIR_NONE;
	uint32_t i = pfx_len;
	/* the prefix must be followed by whitespace, else it is a different word
	 * (e.g. "#!bovnarish") and not our directive at all */
	if (i >= len || (s[i] != ' ' && s[i] != '\t'))
		return BVN_VERDIR_NONE;
	while (i < len && (s[i] == ' ' || s[i] == '\t')) i++;
	uint16_t out[2] = { 0, 0 };
	for (int part = 0; part < 2; part++) {
		if (i >= len || s[i] < '0' || s[i] > '9')
			return BVN_VERDIR_INVALID;
		bool leading_zero = (s[i] == '0');
		uint32_t acc = 0, digits = 0;
		while (i < len && s[i] >= '0' && s[i] <= '9') {
			acc = acc * 10u + (uint32_t)(s[i] - '0');
			if (acc > 0xffffu) return BVN_VERDIR_INVALID;
			i++; digits++;
		}
		if (leading_zero && digits > 1) return BVN_VERDIR_INVALID;
		out[part] = (uint16_t)acc;
		if (part == 0) {
			if (i >= len || s[i] != '.') return BVN_VERDIR_INVALID;
			i++;
		}
	}
	while (i < len && (s[i] == ' ' || s[i] == '\t')) i++;
	if (i != len) return BVN_VERDIR_INVALID;   /* trailing junk */
	if (major) *major = out[0];
	if (minor) *minor = out[1];
	return BVN_VERDIR_OK;
}
static inline void bvn_first_comment_capture(bvnr_lexer_t* l)
{
	if (l->ver_checked) return;
	/*
	 * A run of spaces and tabs is captured as its first byte only.
	 *
	 * The directive's grammar is "!bovnar" hspace+ version hspace* and puts no
	 * bound on either run, while the capture buffer holds 32 bytes -- so
	 * "#!bovnar 1.1" followed by twenty-two spaces overflowed it and was
	 * refused as error_invalid_spec_version, a document failing on trailing
	 * blanks its own grammar allows (and bvnr_peek_version, which reads the
	 * line where it lies, accepted). bvn_parse_version_directive only ever
	 * asks whether a run is there, so one byte of it says everything; the
	 * longest well-formed directive is then 20 bytes and overflow means what
	 * it was meant to mean, a line that is not a version.
	 */
	if ((l->byte == ' ' || l->byte == '\t') && l->ver_len > 0u &&
	    !l->ver_overflow &&
	    (l->ver_buf[l->ver_len - 1u] == ' ' ||
	     l->ver_buf[l->ver_len - 1u] == '\t'))
		return;
	if (l->ver_len < (uint8_t)sizeof(l->ver_buf))
		l->ver_buf[l->ver_len++] = (uint8_t)l->byte;
	else
		l->ver_overflow = true;
}
bool bvn_action_first_comment_intro(bvnr_reader_t* p)
{
	p->lex.last_state = p->lex.next_state;
	p->lex.next_state = first_comment_intro;
	if (!p->lex.ver_checked) {
		p->lex.ver_len      = 0;
		p->lex.ver_overflow = false;
	}
	return true;
}
bool bvn_action_first_comment_byte(bvnr_reader_t* p)
{
	bvn_first_comment_capture(&p->lex);
	p->lex.next_state = (p->lex.byte == 0xefu)
		? first_comment_after_ef : first_comment_intro;
	return true;
}
bool bvn_action_first_comment_after_ef(bvnr_reader_t* p)
{
	bvn_first_comment_capture(&p->lex);
	if (p->lex.byte == 0xbbu)      p->lex.next_state = first_comment_after_ef_bb;
	else if (p->lex.byte == 0xefu) p->lex.next_state = first_comment_after_ef;
	else                           p->lex.next_state = first_comment_intro;
	return true;
}
bool bvn_action_first_comment_after_ef_bb(bvnr_reader_t* p)
{
	if (p->lex.byte == 0xbfu) {
		bvn_lexer_set_error(p, error_invalid_byte_order_mark);
		return false;
	}
	bvn_first_comment_capture(&p->lex);
	p->lex.next_state = (p->lex.byte == 0xefu)
		? first_comment_after_ef : first_comment_intro;
	return true;
}
bool bvn_action_first_comment_outro(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	p->lex.next_state = first_bom;
	if (l->ver_checked)
		return true;
	l->ver_checked = true;
	uint16_t major = 0, minor = 0;
	bvn_verdir_t r = bvn_parse_version_directive(
		l->ver_buf, l->ver_len, &major, &minor);
	/* An overflowed capture that still begins with the directive prefix is an
	 * over-long (therefore malformed) directive; one that does not is just a
	 * long ordinary comment. */
	if (l->ver_overflow) {
		if (r == BVN_VERDIR_NONE)
			return true;
		bvn_lexer_set_error(p, error_invalid_spec_version);
		return false;
	}
	if (r == BVN_VERDIR_NONE)
		return true;
	if (r == BVN_VERDIR_INVALID) {
		bvn_lexer_set_error(p, error_invalid_spec_version);
		return false;
	}
	l->has_declared_version = true;
	l->declared_major       = major;
	l->declared_minor       = minor;
	bool supported = (major == BVNR_SPEC_VERSION_MAJOR &&
	                  minor <= BVNR_SPEC_VERSION_MINOR);
	if (l->strict_version && !supported) {
		bvn_lexer_set_error(p, error_unsupported_spec_version);
		return false;
	}
	return true;
}
bool bvnr_peek_version(
	const void* buf, uint64_t len, uint16_t* major, uint16_t* minor)
{
	const uint8_t* b = (const uint8_t*)buf;
	if (!b) return false;
	uint64_t i = 0;
	if (len >= 3 && b[0] == 0xefu && b[1] == 0xbbu && b[2] == 0xbfu)
		i = 3;                                   /* skip UTF-8 BOM */
	/* The six bytes the reader skips before the first comment (the [undefined]
	 * row of the state table). VT and FF were missing, so a document the
	 * reader took as 1.1 peeked as undeclared. */
	while (i < len && (b[i] == ' ' || b[i] == '\t' ||
	                   b[i] == '\n' || b[i] == '\r' ||
	                   b[i] == 0x0b || b[i] == 0x0c))
		i++;
	if (i >= len || b[i] != '#')
		return false;
	i++;                                         /* consume '#' */
	uint64_t start = i;
	while (i < len && b[i] != '\n' && b[i] != '\r')
		i++;
	uint64_t clen = i - start;
	if (clen > UINT32_MAX) clen = UINT32_MAX;
	return bvn_parse_version_directive(
		b + start, (uint32_t)clen, major, minor) == BVN_VERDIR_OK;
}
bool bvn_action_type_null_then_value_outro(bvnr_reader_t* p)
{
	p->lex.token_type = token_is_null_value;
	p->lex.str_len    = 0;
	if (!bvn_lex_finalize(p, bvn_val_receive))
		return false;
	return bvn_action_value_outro(p);
}
bool bvn_action_type_null_then_new_array_value(bvnr_reader_t* p)
{
	p->lex.token_type = token_is_null_value;
	p->lex.str_len    = 0;
	if (!bvn_lex_finalize(p, bvn_val_receive))
		return false;
	return bvn_action_new_array_value(p);
}
bool bvn_action_type_null_then_array_outro(bvnr_reader_t* p)
{
	p->lex.token_type = token_is_null_value;
	p->lex.str_len    = 0;
	if (!bvn_lex_finalize(p, bvn_val_receive))
		return false;
	return bvn_action_array_outro(p);
}
/*
 * ---------------------------------------------------------------------------
 * Resync (error-recovery) handlers
 * ---------------------------------------------------------------------------
 * Active only after bvn_enter_resync. The goal is to reach the next safe
 * top-level boundary (a ';' that is not inside a bracket pair or string) and
 * resume normal parsing there. While skipping we still have to track bracket
 * and string/comment nesting so a ';' inside skipped-over junk is not mistaken
 * for the boundary — resync_array_depth / resync_struct_depth count brackets
 * opened *after* recovery began. resync_string/_comment states swallow string
 * and comment bodies wholesale so their contents can't trigger false resyncs.
 */
bool bvn_action_resync_skip(bvnr_reader_t* p)
{
	p->lex.next_state = resync;
	return true;
}
bool bvn_action_resync_open_bracket(bvnr_reader_t* p)
{
	if (p->lex.byte == '[')
		++p->lex.resync_array_depth;
	else
		++p->lex.resync_struct_depth;
	p->lex.next_state = resync;
	return true;
}
/*
 * A closing bracket seen during resync. If it merely balances a bracket opened
 * since recovery started, just decrement the resync depth. Otherwise it closes
 * a real array/struct that was open before the error, so we must emit the
 * corresponding close events to the validator (unwinding array frames, or
 * closing all arrays then the struct for '}') to keep its scope stack
 * balanced. This is what lets recovery rejoin the stream cleanly at a point
 * inside a still-open container rather than only at the very top level.
 */
bool bvn_action_resync_close_bracket(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	if (l->byte == ']') {
		if (l->resync_array_depth > 0) {
			--l->resync_array_depth;
			l->next_state = resync;
			return true;
		}
		/* Only an array this struct level owns: a ']' cannot close an array
		 * across the struct that is open inside it (bvn_resync_close_own_arrays). */
		if (l->array_nesting_level > 0 &&
		    l->arr_frames[l->array_nesting_level].open_struct_level ==
		    l->struct_nesting_level) {
			if (!bvn_val_receive_event(p, ev_array_row_end))
				return false;
			--l->array_nesting_level;
			uint64_t level = l->array_nesting_level;
			bvn_array_frame_t *f = &l->arr_frames[level];
			f->dim_row_size      = l->curr_row_size;
			l->curr_row_size     = f->saved_curr;
			l->array_row_size    = f->saved_row;
			p->val.value_type    = f->saved_vtype;
			p->val.parsed_unit   = f->saved_vunit;
			bvn_usrc_restore(&p->val, f);
			p->val.parsed_unit_serial = f->saved_unit_serial;
			l->in_array_element  = (l->array_nesting_level > 0);
			l->next_state        = resync;
		} else {
			l->next_state = resync;
		}
	} else if (l->byte == '}') {
		if (l->resync_struct_depth > 0) {
			--l->resync_struct_depth;
			l->next_state = resync;
			return true;
		}
		if (l->struct_nesting_level > 0) {
			/* The arrays opened inside this struct go with it; one the
			 * struct is an element OF stays open. */
			if (!bvn_resync_close_own_arrays(p))
				return false;
			l->in_array_element = false;
			--l->struct_nesting_level;
			if (!bvn_val_receive_event(p, ev_struct_end))
				return false;
			l->next_state = resync;
		} else {
			l->next_state = resync;
		}
	} else {
		l->next_state = resync;
	}
	return true;
}
/*
 * Reached a ';' boundary during recovery: collapse everything back to a clean
 * value_outro state. Any structs opened since recovery began are closed (down
 * to the nesting level captured when recovery started), all array/scratch
 * state is cleared, and the value type/unit are reset to plain. After this the
 * machine is indistinguishable from having just finished a well-formed
 * assignment, so the next byte resumes normal parsing.
 */
static bool bvn_resync_reset(bvnr_reader_t* p, uint32_t boundary_bytes)
{
	bvnr_lexer_t* l = &p->lex;
	bool ok = true;
	/* Everything consumed since recovery began was discarded, from the byte
	 * the error was raised on up to the boundary recovery resumes at. Both
	 * exits — the ';' boundary and the next-assignment boundary — come through
	 * here, so this is the one place the cost has to be counted.
	 * boundary_bytes is how much of what was just consumed IS that boundary
	 * and was not thrown away: the ';' (1), or the '.' and the first byte of
	 * the next key (2). The skipped total used to include the boundary and
	 * leave out the offending byte -- ".a = 1;@.b = 2;" reported the ".b" that
	 * was delivered and not the "@" that was not. */
	uint64_t end = l->text_bytes >= boundary_bytes
		     ? l->text_bytes - boundary_bytes : 0u;
	if (end > l->resync_start_bytes)
		l->skipped_bytes += end - l->resync_start_bytes;
	while (l->struct_nesting_level > l->resync_saved_struct_nesting) {
		--l->struct_nesting_level;
		if (!bvn_val_receive_event(p, ev_struct_end)) {
			ok = false;
			break;
		}
	}
	if (l->struct_nesting_level < l->resync_saved_struct_nesting)
		l->resync_saved_struct_nesting = l->struct_nesting_level;
	l->token_type           = token_is_unknown;
	l->str_len              = 0;
	l->type_len             = 0;
	l->inline_unit_len      = 0;
	l->utf8_need            = 0;
	l->utf8_lo              = 0;
	l->utf8_hi              = 0;
	l->in_array_element     = false;
	l->struct_nesting_level = l->resync_saved_struct_nesting;
	l->resync_array_depth   = 0;
	l->resync_struct_depth  = 0;
	/* Parsing resumes at an assignment boundary of THIS struct level, so the
	 * arrays it owns are over; an array this struct is an element of is not,
	 * and keeps its state for the rest of its row. */
	if (!bvn_resync_close_own_arrays(p))
		ok = false;
	p->val.value_type          = BVN_TYPE_PLAIN;
	p->val.parsed_unit         = BVN_UNIT_NO_PREFIX(bu_none);
	p->val.parsed_unit_serial++;
	p->val.has_annotation_unit = false;
	bvn_acc_reset(&p->val);
	return ok;
}
static bool bvn_resync_semicolon_reset(bvnr_reader_t* p)
{
	bool ok = bvn_resync_reset(p, 1u);
	p->lex.next_state = value_outro;
	return ok;
}
/*
 * A '.' skipped at the recovery-relative top level. Every assignment starts
 * with one, so this is the earliest point recovery could end — but a '.' also
 * turns up inside junk, so nothing is decided yet: resync_dot looks at the byte
 * after it and only resumes if that byte could begin an identifier. Inside a
 * bracket opened since recovery started there is no assignment to begin, so the
 * '.' is skipped like any other byte.
 */
bool bvn_action_resync_dot(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	l->next_state = (l->resync_array_depth > 0 || l->resync_struct_depth > 0)
		      ? resync : resync_dot;
	return true;
}
/*
 * The byte after that '.' can start an identifier, so recovery ends HERE rather
 * than at the next ';'.
 *
 * That difference is the whole point. Recovery used to run to the next ';' at
 * the recovery-relative top level, which is correct when the error happened
 * inside a statement — that statement's own ';' is the next one, and only the
 * broken statement is lost. When the error happened BETWEEN statements, though,
 * the next ';' belongs to the following, perfectly good statement, and it was
 * swallowed whole: a single stray byte in front of a two-hundred-member struct
 * discarded the entire struct, and the caller was told only that one recovery
 * had occurred.
 *
 * Resuming here cannot recover LESS than the old rule did, because ';' is still
 * a boundary and still ends recovery. What it can do that the old rule could
 * not is stop early, at the point where the document plausibly becomes readable
 * again. It does not make it newly possible to read an assignment out of
 * corruption — a ';' inside a corrupt region already ended recovery and resumed
 * parsing inside it — it changes which corrupt inputs that happens for.
 *
 * The state is reset exactly as the ';' path resets it, and then this byte is
 * consumed as the first character of the identifier, which is where the normal
 * machine would be one byte into an assignment.
 */
bool bvn_action_resync_resume_identifier(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	if (!bvn_resync_reset(p, 2u))
		return false;
	l->str_len    = 0;
	l->token_type = token_is_identifier;
	return bvn_push_str(p, l->max_identifier_length,
			    error_identifier_too_long, identifier_body);
}
bool bvn_action_resync_semicolon(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	if (l->resync_array_depth > 0 || l->resync_struct_depth > 0) {
		l->next_state = resync;
		return true;
	}
	/* bvn_resync_reset closes the arrays this struct level owns. */
	if (!bvn_resync_semicolon_reset(p))
		return false;
	return true;
}
bool bvn_action_resync_string_intro(bvnr_reader_t* p)
{
	p->lex.next_state = resync_string;
	return true;
}
bool bvn_action_resync_string_byte(bvnr_reader_t* p)
{
	p->lex.next_state = resync_string;
	return true;
}
bool bvn_action_resync_string_escape(bvnr_reader_t* p)
{
	p->lex.next_state = resync_string_escape;
	return true;
}
bool bvn_action_resync_string_escape_byte(bvnr_reader_t* p)
{
	p->lex.next_state = resync_string;
	return true;
}
bool bvn_action_resync_string_outro(bvnr_reader_t* p)
{
	p->lex.next_state = resync;
	return true;
}
bool bvn_action_resync_comment_intro(bvnr_reader_t* p)
{
	p->lex.next_state = resync_comment;
	return true;
}
bool bvn_action_resync_comment_byte(bvnr_reader_t* p)
{
	p->lex.next_state = resync_comment;
	return true;
}
bool bvn_action_resync_comment_outro(bvnr_reader_t* p)
{
	p->lex.next_state = resync;
	return true;
}
/*
 * Inline unit suffix (e.g. the `kg` in `42 kg`). Disallowed inside array
 * elements — array element types/units come from the array's annotation, not
 * from each element — hence the early rejection. The bytes are accumulated in
 * a dedicated small buffer and later parsed by the unit machinery.
 */
bool bvn_action_inline_unit_intro(bvnr_reader_t* p)
{
	if (p->lex.in_array_element) {
		bvn_lexer_set_error(p, error_unexpected_input_byte);
		return false;
	}
	bvnr_lexer_t* l = &p->lex;
	l->inline_unit_len = 0;
	l->inline_unit_data[l->inline_unit_len++] = (uint8_t)l->byte;
	l->next_state = inline_unit_body;
	return true;
}
bool bvn_action_copy_inline_unit_byte(bvnr_reader_t* p)
{
	bvnr_lexer_t* l = &p->lex;
	if (l->inline_unit_len == BVN_INLINE_UNIT_BUF_CAP - 1u) {
		bvn_lexer_set_error(p, error_unit_too_long);
		return false;
	}
	l->inline_unit_data[l->inline_unit_len++] = (uint8_t)l->byte;
	l->next_state = inline_unit_body;
	return true;
}
bool bvn_action_to_inline_unit_outro(bvnr_reader_t* p)
{
	p->lex.next_state = inline_unit_outro;
	return true;
}
typedef struct {
	bvnr_reader_t* p;
	const uint8_t*  resid;
	uint32_t        resid_left;
} octet_source_t;
/* processed_bytes counts bytes PULLED, and the text loop adds a whole buffer
 * before lexing any of it, so the residual bytes handed to the octet reader are
 * already counted but not yet consumed. Reporting processed_bytes alone made the
 * error offset depend on the feed chunking -- the same desynced byte was blamed
 * on offset 4022 when the document arrived in one buffer and on 32 when it
 * trickled in 8 bytes at a time. Subtracting what is still unconsumed gives the
 * position the octet reader has actually reached, which is the same number
 * whatever the chunking. */
static inline void bvn_capture_os_error(bvnr_reader_t* p, error_code_t err,
					uint32_t resid_left)
{
	bvnr_lexer_t* l = &p->lex;
	p->val.last_error   = err;
	p->val.error_line   = l->line;
	p->val.error_column = l->column;
	p->val.error_byte   = 0;
	p->val.error_offset = l->processed_bytes - (uint64_t)resid_left;
}
/* A consumer callback that returned false while the octet stream was being
 * read: an abort, reported through on_error like every other one (doc/03
 * §13.1) -- these sites used to return without it, at offset 0. */
static inline bool bvn_os_abort(bvnr_reader_t* p, uint32_t resid_left)
{
	bvn_capture_os_error(p, p->val.last_error, resid_left);
	bvn_notify_error(p);
	return false;
}
/*
 * ---------------------------------------------------------------------------
 * Octet-stream reading
 * ---------------------------------------------------------------------------
 * An octet stream is raw binary framed as repeated [0x01][len_lo][len_hi]
 * [len bytes] chunks terminated by a 0x00 tag. Because the framing is binary,
 * it is read directly from the source here rather than through the text state
 * machine. When the text loop hands control over, some bytes of the next chunk
 * may already sit in the text buffer; those "residual" bytes are consumed
 * first (src->resid) before pulling more from the underlying source, and any
 * bytes left over after the stream ends are returned so the text loop can
 * resume exactly where the binary data stopped.
 */

/*
 * Fill `output` with exactly `length` bytes, drawing first from the residual
 * buffer then from the source, looping over short reads. Also mirrors bytes to
 * the debug sink and enforces the file-size cap, so the binary path honours the
 * same limits and tee behaviour as the text path.
 */
static bool bvn_os_read_exact(
	octet_source_t* src, void* output, uint32_t length)
{
	bvnr_reader_t* p   = src->p;
	uint8_t*        buf = (uint8_t*)output;
	uint32_t from_resid = (src->resid_left < length)
		? src->resid_left : length;
	if (from_resid) {
		const uint8_t* taken = src->resid;
		memcpy(buf, src->resid, from_resid);
		/* Account for the bytes before the sink push, so an error captured
		 * inside it reports the position we have actually reached. */
		src->resid      += from_resid;
		src->resid_left -= from_resid;
		if (p->lex.use_dbg) {
			if (!bvn_sink_push(&p->lex.src_dbg, taken, from_resid)) {
				bvn_capture_os_error(p, error_writing_to_sink,
						     src->resid_left);
				bvn_notify_error(p);
				return false;
			}
		}
	}
	uint32_t need = length - from_resid;
	if (!need) {
		p->lex.octet_bytes += (uint64_t)length;
		return true;
	}
	uint32_t total = 0, got;
	while (total != need) {
		if (p->lex.file_cap_hit) {
			/* bvn_lex_run already cut this document at max_file_size. */
			bvn_capture_os_error(p, error_file_too_long, src->resid_left);
			p->val.error_offset = p->lex.max_file_size;
			bvn_notify_error(p);
			return false;
		}
		bool pulled = bvn_source_pull(&p->lex.src,
				buf + from_resid + total, need - total, &got);
		if (!pulled || !got) {
			/* A source that FAILED is an I/O error, as it is outside a
			 * stream; only end of input is a truncated stream. */
			bvn_capture_os_error(p, pulled
				? error_read_complete_chunk_failed
				: error_reading_from_source_fd,
				src->resid_left);
			bvn_notify_error(p);
			return false;
		}
		p->lex.processed_bytes += (uint64_t)got;
		if (p->lex.max_file_size &&
			p->lex.processed_bytes > p->lex.max_file_size) {
			bvn_capture_os_error(p, error_file_too_long,
					     src->resid_left);
			/* The first byte over the cap, whatever the size of the pull
			 * that crossed it -- see bvn_lex_run. */
			p->val.error_offset = p->lex.max_file_size;
			bvn_notify_error(p);
			return false;
		}
		if (p->lex.use_dbg) {
			if (!bvn_sink_push(&p->lex.src_dbg,
							  buf + from_resid + total, got)) {
				bvn_capture_os_error(p, error_writing_to_sink,
						     src->resid_left);
				bvn_notify_error(p);
				return false;
			}
		}
		total += got;
	}
	p->lex.octet_bytes += (uint64_t)length;
	return true;
}
/*
 * Drive the chunk loop: read a tag, and on 0x01 read the 16-bit little-endian
 * length (where 0 means the maximal 65536, since 0-length chunks are useless)
 * and forward that many bytes to the validator; on 0x00 emit the stream-end
 * event and stop. Any other tag is a framing desync. *out_leftover reports how
 * many residual bytes were not consumed so the caller can rewind the text loop.
 */
static bool bvn_read_octet_stream(
	bvnr_reader_t* p, const uint8_t* resid, uint32_t resid_len,
	uint32_t* out_leftover)
{
	octet_source_t src = { .p = p, .resid = resid, .resid_left = resid_len };
	/* Zero-init the framing scratch: bvn_os_read_exact fills these on every
	 * success path for the library's own sources, but pull is a caller-supplied
	 * callback (bvnr_open_read_source) — a buggy one returning true without
	 * writing must not leave us reading an indeterminate tag/length. */
	uint8_t  tag = 0, lenbuf[2] = { 0, 0 };
	uint32_t chunklen;
	for (;;) {
		if (!bvn_os_read_exact(&src, &tag, 1))
			return false;
		if (tag == 0x00) {
			p->lex.token_type = token_is_unknown;
			p->lex.str_len    = 0;
			if (!bvn_val_receive_event(p, ev_octet_stream_end))
				return bvn_os_abort(p, src.resid_left);
			p->lex.next_state = octet_stream_outro;
			*out_leftover = src.resid_left;
			return true;
		}
		if (tag != 0x01) {
			bvn_capture_os_error(p, error_octet_stream_out_of_sync,
					     src.resid_left);
			/* The tag has been consumed, so the position reached is one past
			 * it; the error is the tag itself, and it is a single byte, so it
			 * is reported the way every text error is -- its own offset, and
			 * the byte. (It used to be offset + 1 and byte 0.) */
			if (p->val.error_offset > 0)
				p->val.error_offset -= 1u;
			p->val.error_byte = (uint32_t)tag;
			bvn_notify_error(p);
			return false;
		}
		if (!bvn_os_read_exact(&src, lenbuf, 2))
			return false;
		chunklen = (uint32_t)lenbuf[0]
				 | ((uint32_t)lenbuf[1] << 8);
		if (!chunklen)
			chunklen = 65536u;
		if (!bvn_os_read_exact(&src, p->lex.str_data, chunklen))
			return false;
		if (!bvn_val_receive_octet_chunk(p, p->lex.str_data, chunklen))
			return bvn_os_abort(p, src.resid_left);
	}
}
/*
 * Classify a UTF-8 leading byte: how many continuation bytes follow (`need`)
 * and the valid range [lo,hi] of the *first* continuation. The per-leader lo/hi
 * ranges are the standard UTF-8 well-formedness constraints from RFC 3629 /
 * Unicode Table 3-7 that reject overlong encodings, surrogates (0xED) and
 * out-of-range code points (0xF4), so validation is exact rather than the
 * naive "0x80-0xBF for every trailing byte". Returns false for bytes that can
 * never begin a sequence (0x80-0xC1, 0xF5-0xFF).
 */
bool bvn_utf8_classify_leader(
	uint32_t b, uint8_t* need, uint8_t* lo, uint8_t* hi)
{
	if (b <= 0x7f) { *need = 0; *lo = 0;    *hi = 0;    return true;  }
	if (b <= 0xc1) return false;
	if (b <= 0xdf) { *need = 1; *lo = 0x80; *hi = 0xbf; return true;  }
	if (b == 0xe0) { *need = 2; *lo = 0xa0; *hi = 0xbf; return true;  }
	if (b == 0xed) { *need = 2; *lo = 0x80; *hi = 0x9f; return true;  }
	if (b <= 0xef) { *need = 2; *lo = 0x80; *hi = 0xbf; return true;  }
	if (b == 0xf0) { *need = 3; *lo = 0x90; *hi = 0xbf; return true;  }
	if (b <= 0xf3) { *need = 3; *lo = 0x80; *hi = 0xbf; return true;  }
	if (b == 0xf4) { *need = 3; *lo = 0x80; *hi = 0x8f; return true;  }
	return false;
}
typedef struct {
	uint8_t need, lo, hi;
} utf8_state_t;
/*
 * Incremental UTF-8 validator: feed one byte, return false on malformation.
 * Holding the decode state in a tiny struct (need/lo/hi) lets the same logic
 * serve both the streaming lexer (state stored across buffer boundaries in
 * bvn_utf8_feed) and the one-shot bvn_validate_string. After the first
 * continuation byte the range relaxes to the generic 0x80-0xBF.
 */
static inline bool bvn_utf8_feed_st(utf8_state_t* st, uint32_t b)
{
	if (st->need) {
		if (b < st->lo || b > st->hi) {
			*st = (utf8_state_t){0};
			return false;
		}
		--st->need;
		st->lo = 0x80;
		st->hi = 0xbf;
		return true;
	}
	return bvn_utf8_classify_leader(b, &st->need, &st->lo, &st->hi);
}
static bool bvn_utf8_feed(bvnr_lexer_t* l, uint32_t b)
{
	utf8_state_t st = { l->utf8_need, l->utf8_lo, l->utf8_hi };
	bool ok = bvn_utf8_feed_st(&st, b);
	l->utf8_need = st.need;
	l->utf8_lo   = st.lo;
	l->utf8_hi   = st.hi;
	return ok;
}
/*
 * Public helper: is the whole buffer well-formed UTF-8? Runs the incremental
 * validator to completion and requires no pending continuation bytes at the
 * end (a truncated multibyte sequence at EOF is invalid).
 */
bool bvn_validate_string(const uint8_t* data, size_t len)
{
	utf8_state_t st = {0};
	for (size_t i = 0; i < len; i++) {
		if (!bvn_utf8_feed_st(&st, data[i]))
			return false;
	}
	return st.need == 0;
}
/*
 * The core per-byte step: look up the action for (current state, byte), then
 * dispatch. Whitespace is the hottest case so it short-circuits before the
 * indirect call; a zero index means the byte is illegal in this state. Pure
 * state transitions (bvn_action_set_state) are inlined here too, so an actual
 * function-pointer call only happens for handlers that do real work.
 */
static inline bool bvn_call_action_for_new_byte(bvnr_reader_t* p)
{
	uint8_t idx = bvn_after_state_idx_table[p->lex.next_state][p->lex.byte];
	if (idx == ACT_ignore_whitespace)
		return true;
	if (!idx) {
		bvn_lexer_set_error(p, error_unexpected_input_byte);
		return false;
	}
	action_t fn = bvn_action_table[idx];
	if (fn == bvn_action_set_state) {
		p->lex.next_state = bvn_action_target_state[idx];
		return true;
	}
	return fn(p);
}
static inline void bvn_advance_line(bvnr_lexer_t* l, uint8_t prev)
{
	if (l->byte == 0x0d) {
		++l->line;
		l->column = 0;
	} else if (l->byte == 0x0a) {
		if (prev != 0x0d)
			++l->line;
		l->column = 0;
	}
}
/*
 * Scan ahead to find the end of a maximal run of string-body bytes starting at
 * `start`: stops at a control byte, a quote, a backslash (escape), or any
 * malformed/incomplete UTF-8. Returns the index one past the last byte that is
 * safe to bulk-copy — crucially it never returns mid-multibyte-sequence
 * (last_complete tracks the last code-point boundary), so a UTF-8 character
 * split across the run boundary is left for the per-byte path to handle.
 */
static inline uint32_t bvn_string_ext_end(
	const uint8_t* data, uint32_t start, uint32_t len)
{
	uint8_t  need = 0, lo = 0, hi = 0;
	uint32_t last_complete = start;
	uint32_t i = start;
	while (i < len) {
		uint8_t b = data[i];
		/* 0x00-0x1F and 0x7F (DEL) are rejected control bytes in string
		 * bodies (BVN_REJECT_ASCII_CTRL); break so the per-byte path
		 * raises the error rather than the fast path swallowing them.
		 * Without the explicit 0x7F check the bulk scanner would drift
		 * from the authoritative transition table and accept DEL. */
		if (b < 0x20u || b == 0x7fu || b == 0x22u || b == 0x5cu)
			break;
		if (b < 0x80u) {
			if (need) return last_complete;
			last_complete = i + 1u;
		} else if (need) {
			if (b < lo || b > hi) return last_complete;
			--need;
			lo = 0x80u; hi = 0xbfu;
			if (!need) last_complete = i + 1u;
		} else {
			if (!bvn_utf8_classify_leader(b, &need, &lo, &hi))
				return last_complete;
		}
		++i;
	}
	return need ? last_complete : i;
}
/*
 * Count UTF-8 code-point starts (non-continuation bytes) in data[0..n).
 * The bulk paths advance l->column by this, not by the raw byte count, so a
 * run containing multibyte characters advances the column exactly as the
 * per-byte path does (which only ++column on bytes with saved_need == 0).
 * Bulk runs are only entered with l->utf8_need == 0 (see bvn_lex_run), so the
 * run always starts on a code-point boundary; a clamp that truncates the run
 * mid-sequence still yields the correct count because column tracks code-point
 * starts, exactly the bytes counted here.
 */
static inline uint32_t bvn_utf8_count_starts(const uint8_t* data, uint32_t n)
{
	uint32_t cols = 0;
	for (uint32_t i = 0; i < n; ++i)
		cols += ((data[i] & 0xc0u) != 0x80u);
	return cols;
}
/*
 * A capacity/byte-budget clamp can cut a run length `n` short of the UTF-8
 * code-point boundary the scanner stopped on, landing `start + n` on a
 * continuation byte. The per-byte path would then resume with utf8_need == 0 on
 * that continuation byte and raise error_invalid_utf8_byte instead of the
 * correct *_too_long cap error. Walk `n` back to the previous code-point
 * boundary so the per-byte path resumes on a leader byte and the real cap error
 * surfaces. Only steps when a clamp truncated the run: an unclamped `n` already
 * ends on a boundary (and the `start + n < len` guard keeps the read in range).
 */
static inline uint32_t bvn_clamp_to_utf8_boundary(
	const uint8_t* data, uint32_t start, uint32_t n, uint32_t len)
{
	while (n > 0u && (start + n) < len && (data[start + n] & 0xc0u) == 0x80u)
		--n;
	return n;
}
/*
 * Bulk fast path. If the machine is sitting in one of the "copy byte" states,
 * find the longest run of bytes from `start` that all stay in that state and
 * memcpy them into the target buffer in a single call, returning how many
 * bytes were consumed (0 = fall back to the per-byte loop). This is the main
 * throughput win: long strings/identifiers/numbers no longer pay one indirect
 * call per byte. All the limits the per-byte path enforces (buffer capacity,
 * max_string_length, max_text_bytes) are re-applied to the run length so the
 * fast path can never overrun a bound. Strings get a dedicated branch because
 * their terminator set differs; the rest share a run-LUT-driven scanner that
 * also walks complete UTF-8 sequences. l->byte/prev_byte are updated to the
 * last copied byte so the per-byte path resumes consistently.
 */
static inline uint32_t bvn_try_bulk_run(
	bvnr_lexer_t* l, const uint8_t* data, uint32_t start, uint32_t len)
{
	state_t st = l->next_state;
	if (st == copy_string_byte) {
		uint32_t end = bvn_string_ext_end(data, start, len);
		uint32_t n   = end - start;
		if (!n) return 0;
		uint32_t avail = (uint32_t)l->max_string_length - (uint32_t)l->str_len;
		if (!avail) return 0;
		if (n > avail) n = avail;
		if (l->max_text_bytes) {
			uint64_t tb = l->max_text_bytes - l->text_bytes;
			if (!tb) return 0;
			if ((uint64_t)n > tb) n = (uint32_t)tb;
		}
		n = bvn_clamp_to_utf8_boundary(data, start, n, len);
		if (!n) return 0;
		memcpy(l->str_data + l->str_len, data + start, n);
		l->str_len     = (uint16_t)(l->str_len + n);
		l->text_bytes += n;
		l->column     += bvn_utf8_count_starts(data + start, n);
		l->byte        = data[start + n - 1u];
		l->prev_byte   = data[start + n - 1u];
		return n;
	}
	const uint8_t* lut;
	uint8_t* dst;
	uint32_t dst_len;
	uint32_t dst_cap;
	bool is_str_buf = true;
	switch (st) {
	case identifier_body:
		lut = bvn_run_lut_ident;
		dst_cap = l->max_identifier_length;
		break;
	case copy_number_byte:
		lut = bvn_run_lut_number;
		dst_cap = l->max_number_length;
		break;
	case copy_fraction_byte:
		lut = bvn_run_lut_fraction;
		dst_cap = l->max_number_length;
		break;
	case copy_exp_byte:
		lut = bvn_run_lut_exp;
		dst_cap = l->max_number_length;
		break;
	case symbol_body:
		lut = bvn_run_lut_symbol;
		dst_cap = l->max_symbol_length;
		break;
	case reference_segment_body:
		/* Directly after an index: the per-byte path decides (see
		 * bvn_action_copy_reference_byte). */
		if (l->str_len > 0u && l->str_data[l->str_len - 1u] == ']')
			return 0;
		lut = bvn_run_lut_reference;
		dst_cap = l->max_reference_length;
		break;
	case inline_unit_body:
		lut = bvn_run_lut_inline_unit;
		dst = l->inline_unit_data;
		dst_len = l->inline_unit_len;
		dst_cap = BVN_INLINE_UNIT_BUF_CAP - 1u;
		is_str_buf = false;
		break;
	case copy_type_byte:
		lut = bvn_run_lut_type;
		dst = l->type_data;
		dst_len = l->type_len;
		dst_cap = BVN_TYPE_BUF_CAP - 1u;
		is_str_buf = false;
		break;
	default:
		return 0;
	}
	if (is_str_buf) {
		dst     = l->str_data;
		dst_len = l->str_len;
	}
	if (!lut[data[start]])
		return 0;
	uint32_t end = start;
	/* Initialised to `start` so the degenerate start>=len case (which the
	 * caller never produces — data[start] is read above) is well-defined and
	 * returns 0 below, rather than reading an uninitialised value. The optimiser
	 * cannot prove start<len once this is inlined into bvn_lex_run, so an
	 * uninitialised declaration trips -Wmaybe-uninitialized at -O2. */
	uint32_t last_complete = start;
	while (end < len) {
		uint32_t a = end;
		while (a < len && data[a] < 0x80u && lut[data[a]]) ++a;
		last_complete = a;
		end = a;
		if (end >= len) break;
		uint8_t b = data[end];
		if (b < 0x80u) break;
		if (!lut[b]) break;
		uint8_t need = 0, lo = 0, hi = 0;
		if (!bvn_utf8_classify_leader(b, &need, &lo, &hi)) break;
		++end;
		bool seq_ok = true;
		while (need) {
			if (end >= len) { seq_ok = false; break; }
			uint8_t cb = data[end];
			if (cb < lo || cb > hi) { seq_ok = false; break; }
			--need;
			lo = 0x80u; hi = 0xbfu;
			++end;
		}
		if (!seq_ok) break;
		last_complete = end;
	}
	end = last_complete;
	if (end == start) return 0;
	uint32_t n = end - start;
	uint32_t avail = dst_cap - dst_len;
	if (!avail) return 0;
	if (n > avail) n = avail;
	if (l->max_text_bytes) {
		uint64_t tb = l->max_text_bytes - l->text_bytes;
		if (!tb) return 0;
		if ((uint64_t)n > tb) n = (uint32_t)tb;
	}
	n = bvn_clamp_to_utf8_boundary(data, start, n, len);
	if (!n) return 0;
	memcpy(dst + dst_len, data + start, n);
	if (is_str_buf) {
		l->str_len = (uint16_t)(dst_len + n);
	} else if (st == copy_type_byte) {
		l->type_len = (uint8_t)(dst_len + n);
	} else {
		l->inline_unit_len = (uint8_t)(dst_len + n);
	}
	l->text_bytes += n;
	l->column     += bvn_utf8_count_starts(data + start, n);
	l->byte        = data[start + n - 1u];
	l->prev_byte   = data[start + n - 1u];
	return n;
}
/*
 * Companion fast path for whitespace runs, used only in states that accept
 * whitespace as a no-op. It collapses a span of spaces/tabs/newlines in one
 * pass while keeping line and column accounting exact — including tab stops
 * (rounded up to multiples of 4) and CR/LF handling (a CRLF pair counts as one
 * line). This matters because indentation in pretty-printed bovnar is common.
 */
static inline uint32_t bvn_try_bulk_ws(
	bvnr_lexer_t* l, const uint8_t* data, uint32_t start, uint32_t len)
{
	state_t st = l->next_state;
	if (!bvn_ws_accepting_state[st])
		return 0;
	uint64_t col  = l->column;
	uint64_t line = l->line;
	uint8_t  prev = l->prev_byte;
	uint32_t end  = start;
	while (end < len) {
		uint8_t b = data[end];
		if (b == 0x20)      { ++col; }
		else if (b == 0x09) { col = ((col >> 2u) + 1u) << 2u; }
		else if (b == 0x0a) {
			if (prev != 0x0d) ++line;
			col = 0;
		}
		else if (b == 0x0d) { ++line; col = 0; }
		else if (b == 0x0b || b == 0x0c) { ++col; }
		else break;
		prev = b;
		++end;
	}
	uint32_t n = end - start;
	if (!n) return 0;
	if (l->max_text_bytes &&
	    l->text_bytes + (uint64_t)n > l->max_text_bytes)
		return 0;
	l->text_bytes += n;
	l->line        = line;
	l->column      = col;
	l->byte        = data[end - 1u];
	l->prev_byte   = prev;
	return n;
}
/*
 * Run the state machine over one contiguous input buffer.
 *
 * For each position it first tries the bulk fast paths (only when not mid
 * UTF-8 sequence), then falls back to the exact per-byte path: enforce the
 * text-byte cap, validate UTF-8 incrementally, update line/column, and invoke
 * the byte's action. On a per-byte error it either aborts or, under
 * continue_on_error, enters resync and keeps going. The one place it returns
 * early mid-buffer is when an octet-stream intro is hit: *consumed marks where
 * binary framing takes over so the caller can switch to bvn_read_octet_stream.
 */
static bool bvn_interpret_input_buffer(
	bvnr_reader_t* p, const uint8_t* data, uint32_t len,
	uint32_t* consumed)
{
	bvnr_lexer_t* l = &p->lex;
	for (uint32_t idx = 0; idx < len; ++idx) {
		if (l->utf8_need == 0u) {
			uint32_t n = bvn_try_bulk_run(l, data, idx, len);
			if (n) {
				idx += n - 1u;
				continue;
			}
			if (bvn_run_lut_ws[data[idx]]) {
				n = bvn_try_bulk_ws(l, data, idx, len);
				if (n) {
					idx += n - 1u;
					continue;
				}
			}
		}
		if (l->max_text_bytes &&
			l->text_bytes == l->max_text_bytes) {
			uint8_t offending = data[idx];
			uint64_t col;
			if (offending == 0x09u)
				col = ((l->column >> 2u) + 1u) << 2u;
			else if (l->utf8_need == 0)
				col = l->column + 1u;
			else
				col = l->column;
			p->val.last_error   = error_text_data_too_long;
			p->val.error_line   = l->line;
			p->val.error_column = col;
			p->val.error_byte   = (uint32_t)offending & 0xffu;
			p->val.error_offset = l->text_bytes + l->octet_bytes;
			bvn_notify_error(p);
			return false;
		}
		++l->text_bytes;
		uint8_t b = data[idx];
		l->byte = b;
		uint8_t prev = l->prev_byte;
		l->prev_byte = b;
		uint8_t saved_need = l->utf8_need;
		bool line_advanced = false;
		if (b < 0x80u && saved_need == 0u) {
		} else if (!bvn_utf8_feed(l, (uint32_t)b)) {
			if (l->byte == 0x09)
				l->column = ((l->column >> 2u) + 1u) << 2u;
			else if (saved_need == 0)
				++l->column;
			p->val.last_error = error_invalid_utf8_byte;
			bvn_set_error_pos(p, (uint32_t)l->byte & 0xffu,
				bvn_last_text_offset(l));
			bvn_notify_error(p);
			bvn_advance_line(l, prev);
			line_advanced = true;
			if (l->continue_on_error) {
				bool was_continuation = (saved_need > 0);
				if (bvn_recover_in_comment(p,
					l->byte == 0x0a || l->byte == 0x0d)) {
					if (!was_continuation || l->byte == 0x0a ||
						l->byte == 0x0d)
						continue;
					/* the byte that cut the sequence short starts a new
					 * character of the comment; fall through to it */
				} else if (!bvn_enter_resync(p)) {
					return false;
				}
				if (!was_continuation || l->byte == 0x09 ||
					l->byte == 0x0a || l->byte == 0x0d) {
					continue;
				}
				if (!bvn_utf8_feed(l, (uint32_t)l->byte)) {
					++l->column;
					continue;
				}
				++l->column;
			} else {
				return false;
			}
		}
		if (l->byte == 0x09)
			l->column = ((l->column >> 2u) + 1u) << 2u;
		else if (saved_need == 0)
			++l->column;
		if (!bvn_call_action_for_new_byte(p)) {
			bvn_set_error_pos(p, (uint32_t)l->byte & 0xffu,
				bvn_last_text_offset(l));
			bvn_notify_error(p);
			if (!line_advanced)
				bvn_advance_line(l, prev);
			/* A consumer returning false is not a document error: the API
			 * documents it as "abort". Feeding it the rest of the file because
			 * continue_on_error happens to be set re-enters a consumer that has
			 * already said stop, and inflates recovery_count with aborts that
			 * were never parse failures. */
			if (p->val.last_error == error_scanner_callback_failed)
				return false;
			/* strict_version is the consumer saying "do not read a document
			 * whose version I cannot vouch for". Recovering from it went on
			 * to read "#!bovnar 9.9" whole and return true. Under
			 * strict_version a malformed directive is the same refusal:
			 * the version cannot be known. */
			if (p->val.last_error == error_unsupported_spec_version ||
			    (l->strict_version &&
			     p->val.last_error == error_invalid_spec_version))
				return false;
			/* text_only refuses the document, not one value in it: the
			 * payload after the refused 0x00 is binary, and recovery,
			 * which cannot read its framing, scanned it as text and
			 * delivered assignments out of it. */
			if (p->val.last_error == error_octet_stream_forbidden)
				return false;
			if (l->continue_on_error) {
				if (bvn_recover_in_comment(p, false))
					continue;
				/* The byte just rejected may be the lead byte of a valid
				 * multi-byte character (a stray "€" between statements).
				 * Entering resync clears the UTF-8 progress, so its
				 * continuation bytes were then each reported as
				 * error_invalid_utf8_byte: one stray character, three
				 * errors and three recoveries. Keep the progress, so the
				 * character is skipped whole. */
				uint8_t keep_need = l->utf8_need;
				uint8_t keep_lo   = l->utf8_lo;
				uint8_t keep_hi   = l->utf8_hi;
				if (!bvn_enter_resync(p))
					return false;
				l->utf8_need = keep_need;
				l->utf8_lo   = keep_lo;
				l->utf8_hi   = keep_hi;
				/*
				 * A semantic error (e.g. value-out-of-range) is only
				 * detected once the value's terminating ';' has been
				 * consumed, so the stream is already at a clean
				 * assignment boundary. Skipping forward to the *next*
				 * ';' would swallow the following — possibly valid —
				 * assignment. Per §13.2 ("resume on ';' at the saved
				 * nesting depth") treat the just-consumed ';' as that
				 * boundary and resume normal parsing immediately.
				 */
				if (l->next_state == resync && l->byte == (uint8_t)';')
					bvn_resync_semicolon_reset(p);
				/*
				 * The same goes for a '}' that was itself the offending
				 * byte -- ".s = {.a = 1 }; .b = 2;", a member missing its
				 * ';'. It was consumed as the error and then forgotten, so
				 * the struct it closes stayed open: ".b" was delivered as a
				 * member of ".s" and the document ended incomplete. Give it
				 * to the recovery machine, which closes the struct exactly
				 * as it does for a '}' it meets while skipping. (A ']' needs
				 * nothing: the arrays it could close were closed on entry.)
				 */
				else if (l->next_state == resync &&
					 l->byte == (uint8_t)'}' &&
					 !bvn_action_resync_close_bracket(p))
					return false;
				/*
				 * And the converse: a '{' or '[' that was the offending byte
				 * opened nothing (struct_intro / array_intro refuse before
				 * counting a level), but its closing bracket is still ahead.
				 * Recovery counts it as a bracket it skipped, or that close
				 * shut the ENCLOSING struct: in
				 * ".s = {.x = <utf8> {.a = 1;}; .y = 2;};" the inner '}'
				 * closed .s, .a was delivered into .s and .y at top level.
				 */
				else if (l->next_state == resync &&
					 l->byte == (uint8_t)'{')
					l->resync_struct_depth = 1;
				else if (l->next_state == resync &&
					 l->byte == (uint8_t)'[')
					l->resync_array_depth = 1;
			} else {
				return false;
			}
		} else {
			if (!line_advanced)
				bvn_advance_line(l, prev);
		}
		if (l->next_state == octet_stream_intro) {
			*consumed = idx + 1;
			return true;
		}
	}
	*consumed = len;
	return true;
}
static inline void bvn_set_eof_error(bvnr_reader_t* p, error_code_t err)
{
	p->val.last_error = err;
	bvn_set_error_pos(p, 0, p->lex.processed_bytes);
}
/*
 * Free lexer-owned heap. Note str_data/type_data/inline_unit_data are a single
 * allocation (see bvn_lex_init) carved into three regions, so only str_data is
 * freed; the other two pointers just alias into it and are nulled.
 */
void bvn_lex_destroy(bvnr_lexer_t* l)
{
	if (!l)
		return;
	free(l->arr_frames);
	l->arr_frames = NULL;
	free(l->str_data);
	l->str_data         = NULL;
	l->type_data        = NULL;
	l->inline_unit_data = NULL;
}
/*
 * Initialise a lexer: copy caller limits, falling back to the compiled-in
 * defaults for any left at zero, then allocate the array-frame stack and the
 * scratch buffers. The three scratch buffers are deliberately carved from one
 * malloc to save two allocations and keep them in one cache-friendly block;
 * bvn_lex_destroy relies on this layout. An optional debug sink tees the raw
 * bytes consumed, used by the canonicaliser/observer tooling.
 */
bool bvn_lex_init(bvnr_lexer_t* l, const bvnr_source_t* src,
	const bvnr_sink_t* dbg_sink, bvnr_read_flags_t* opts)
{
	bvn_init_run_luts();
	memset(l, 0, sizeof(*l));
	l->next_state  = undefined;
	l->line        = 1;
	if (src)
		l->src = *src;
	if (dbg_sink && bvn_sink_impl_c(dbg_sink)->push) {
		l->src_dbg = *dbg_sink;
		l->use_dbg = true;
	}
	if (opts) {
		l->max_identifier_length = opts->max_identifier_length;
		l->max_string_length     = opts->max_string_length;
		l->max_number_length     = opts->max_number_length;
		l->max_symbol_length     = opts->max_symbol_length;
		l->max_reference_length  = opts->max_reference_length;
		l->max_array_items       = opts->max_array_items;
		l->max_text_bytes        = opts->max_text_bytes;
		l->max_file_size         = opts->max_file_size;
		l->max_struct_nesting    = opts->max_struct_nesting;
		l->max_array_nesting     = opts->max_array_nesting;
		l->continue_on_error     = opts->continue_on_error;
		l->strict_version        = opts->strict_version;
		l->text_only             = opts->text_only;
	}
	if (!l->max_identifier_length)	l->max_identifier_length	= max_identifier_length;
	if (!l->max_number_length)		l->max_number_length		= max_number_length;
	if (!l->max_string_length)		l->max_string_length		= max_string_length;
	if (!l->max_symbol_length)		l->max_symbol_length		= max_symbol_length;
	if (!l->max_reference_length)	l->max_reference_length		= max_reference_length;
	if (!l->max_struct_nesting)		l->max_struct_nesting		= max_struct_nesting;
	if (!l->max_array_nesting)		l->max_array_nesting		= max_array_nesting;
	if (!l->max_array_items)		l->max_array_items			= max_array_items;
	if (!l->max_text_bytes)			l->max_text_bytes			= max_text_bytes;
	/* max_file_size is deliberately NOT defaulted: 0 means "unlimited / endless"
	 * (BVNR_FILESIZE_UNLIMITED). The cap checks below are guarded by a nonzero
	 * max_file_size, so a 0 here accumulates no byte-count limit. */
	l->arr_frames = calloc(l->max_array_nesting + 1u, sizeof(bvn_array_frame_t));
	if (!l->arr_frames)
		return false;
	uint8_t* bufs = (uint8_t*)malloc(BVN_STR_BUF_CAP + BVN_TYPE_BUF_CAP +
	                                 BVN_INLINE_UNIT_BUF_CAP);
	if (!bufs) {
		free(l->arr_frames);
		l->arr_frames = NULL;
		return false;
	}
	l->str_data         = bufs;
	l->type_data        = bufs + BVN_STR_BUF_CAP;
	l->inline_unit_data = bufs + BVN_STR_BUF_CAP + BVN_TYPE_BUF_CAP;
	return true;
}
/*
 * Top-level read loop: pull buffer, lex it, repeat until EOF.
 *
 * Two source modes are handled. For an in-memory source the lexer walks the
 * caller's buffer directly in <=1 MiB windows (no copy into stack_data) — the
 * zero-copy fast path that motivated exposing bvn_pull_mem. For an fd source
 * it reads into a stack buffer. At EOF the next_state is inspected to decide
 * whether the stream ended cleanly (only legal at a top-level value boundary
 * with no open structs) or is truncated/incomplete; mid-resync EOF is also a
 * truncation. The octet-stream intro is the one place the text loop yields to
 * the binary reader and then resumes at the returned offset. stream_start /
 * stream_end events bracket the whole run for the validator.
 */
#define BVN_MEM_CHUNK_MAX (1u << 20)
bool bvn_lex_run(bvnr_reader_t* r)
{
	bvnr_lexer_t* l = &r->lex;
	uint8_t stack_data[BOVN_READ_BUFFER_SIZE];
	const uint8_t* data;
	uint32_t nb, off, consumed;
	uint32_t os_leftover;
	bvn_source_impl_t* src_impl = bvn_source_impl(&l->src);
	bool is_mem_src = (src_impl->pull == bvn_pull_mem);
	/* An abort from the consumer at the first or the last event is reported
	 * like any other (doc/03 §13.1); it used to skip on_error. */
	if (!bvn_val_receive_event(r, ev_stream_start)) {
		bvn_set_error_pos(r, 0, 0);
		bvn_notify_error(r);
		return false;
	}
	for (;;) {
		if (is_mem_src) {
			uint64_t avail = src_impl->mem_left;
			if (!avail) {
				nb = 0;
				data = NULL;
			} else {
				nb = (avail > (uint64_t)BVN_MEM_CHUNK_MAX)
					? BVN_MEM_CHUNK_MAX : (uint32_t)avail;
				data = src_impl->mem_ptr;
				src_impl->mem_ptr  += nb;
				src_impl->mem_left -= nb;
			}
		} else if (!bvn_source_pull(&l->src, stack_data,
		                            BOVN_READ_BUFFER_SIZE, &nb)) {
			bvn_set_eof_error(r, error_reading_from_source_fd);
			bvn_notify_error(r);
			return false;
		} else {
			data = stack_data;
		}
		if (!nb) {
			/*
			 * End of input inside a multi-byte sequence. Every other place
			 * such a sequence can be cut short is refused already, because
			 * the byte after it is not a continuation byte -- but a comment
			 * may run to end of input, and there nothing came after it:
			 * ".a = 1; #<c3>" was accepted, where the same bytes followed by
			 * a newline are error_invalid_utf8_byte. Asked only where the
			 * document would otherwise be accepted; elsewhere a truncated
			 * document stays error_got_incomplete_bvnr_stream.
			 */
			bool eof_in_first_comment =
				l->next_state == first_comment_intro ||
				l->next_state == first_comment_after_ef ||
				l->next_state == first_comment_after_ef_bb;
			bool eof_acceptable =
				((l->next_state == value_outro ||
				  l->next_state == undefined   ||
				  l->next_state == first_bom   ||
				  eof_in_first_comment) ||
				 ((l->next_state == comment_intro ||
				   l->next_state == ignore_comment_byte) &&
				  l->last_state == value_outro)) &&
				!l->struct_nesting_level;
			if (eof_acceptable && l->utf8_need != 0u) {
				bvn_set_eof_error(r, error_invalid_utf8_byte);
				bvn_notify_error(r);
				if (!l->continue_on_error)
					return false;
				/* Nothing is left to resynchronise on: the comment, and the
				 * document, end here. Reported and counted like any other
				 * recovered error. */
				l->utf8_need = 0;
				++l->recovery_count;
				if (eof_in_first_comment)
					l->next_state = first_comment_intro;
			}
			if ((l->next_state == value_outro ||
				 l->next_state == undefined   ||
				 l->next_state == first_bom   ||
				 l->next_state == first_comment_intro) &&
				!l->struct_nesting_level) {
				/* first_comment_intro: a document that is nothing but a
				 * leading comment terminated by EOF (no trailing newline)
				 * is a valid empty document — the EBNF comment production
				 * ends on CR | LF | EOF. Without this a comment-only file
				 * missing its final newline wrongly reported
				 * error_got_incomplete_bvnr_stream.
				 *
				 * That comment may be the VERSION DIRECTIVE, though, and
				 * bvn_action_first_comment_outro is the only place it gets
				 * parsed. Accepting straight from here skipped it, so
				 * "#!bovnar 99.9" with no trailing newline passed even under
				 * strict_version. Close the comment properly first. */
				if (l->next_state == first_comment_intro &&
				    !bvn_action_first_comment_outro(r)) {
					bvn_set_eof_error(r, r->val.last_error != error_none
						? r->val.last_error
						: error_invalid_spec_version);
					bvn_notify_error(r);
					return false;
				}
				r->val.last_error = error_none;
				break;
			}
			if ((l->next_state == comment_intro ||
				 l->next_state == ignore_comment_byte) &&
				l->last_state == value_outro &&
				!l->struct_nesting_level) {
				r->val.last_error = error_none;
				break;
			}
			if (l->next_state == resync ||
				l->next_state == resync_dot ||
				l->next_state == resync_string ||
				l->next_state == resync_string_escape ||
				l->next_state == resync_comment) {
				/* Recovery that never found a boundary: everything from the
				 * error to the end of the document was discarded, and it is
				 * counted like any other skip. */
				if (l->text_bytes > l->resync_start_bytes)
					l->skipped_bytes +=
						l->text_bytes - l->resync_start_bytes;
				bvn_set_eof_error(r, error_got_incomplete_bvnr_stream);
				bvn_notify_error(r);
				return false;
			}
			bvn_set_eof_error(r, error_got_incomplete_bvnr_stream);
			bvn_notify_error(r);
			return false;
		}
		/*
		 * max_file_size. The cap used to be tested against whole pulled
		 * buffers, before any byte of the buffer that crossed it was read --
		 * so where a too-long document stopped, what had been delivered by
		 * then, and the offset reported all depended on how the bytes
		 * happened to arrive: 150 bytes under a cap of 70 gave no events and
		 * offset 150 from a memory buffer, and seventy parsed bytes and
		 * offset 71 from a source handing over one byte at a time (which
		 * could also meet an earlier syntax error first, and report that
		 * instead). Read exactly what the cap allows, then refuse at the
		 * first byte over it: the same events and the same position whatever
		 * the chunking.
		 */
		bool over_cap = false;
		if (l->max_file_size &&
			l->processed_bytes + (uint64_t)nb > l->max_file_size) {
			nb = (l->processed_bytes < l->max_file_size)
				? (uint32_t)(l->max_file_size - l->processed_bytes) : 0u;
			over_cap = true;
			/* The octet reader pulls for itself once its residual runs
			 * out, and the bytes cut off here are gone from the source. */
			l->file_cap_hit = true;
		}
		l->processed_bytes += (uint64_t)nb;
		off = 0;
		while (off < nb) {
			uint32_t text_start = off;
			if (!bvn_interpret_input_buffer(r, data + off,
											nb - off, &consumed))
				return false;
			off += consumed;
			if (l->use_dbg && consumed) {
				if (!bvn_sink_push(&l->src_dbg,
								  data + text_start,
								  consumed)) {
					bvn_set_eof_error(r,
						error_writing_to_sink);
					bvn_notify_error(r);
					return false;
				}
			}
			if (l->next_state == octet_stream_intro) {
				if (!bvn_read_octet_stream(r,
					data + off, nb - off, &os_leftover))
					return false;
				off = nb - os_leftover;
			}
		}
		if (over_cap) {
			bvn_set_eof_error(r, error_file_too_long);
			bvn_notify_error(r);
			return false;
		}
	}
	if (!bvn_val_receive_event(r, ev_stream_end)) {
		bvn_set_error_pos(r, 0, l->processed_bytes);
		bvn_notify_error(r);
		return false;
	}
	return true;
}
