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

#ifndef BVN_VAL_IMPL_H_
#define BVN_VAL_IMPL_H_
#include "bvn_lexer_impl.h"
#include "bvn_int.h"
#define BVN_SER_WBUF_SIZE 65536u
#define BOVN_READ_BUFFER_SIZE	65536u
typedef enum bvn_limit_defaults_e {
	max_identifier_length = UINT8_MAX,
	max_number_length     = UINT16_MAX,
	max_string_length     = UINT16_MAX,
	max_symbol_length     = UINT8_MAX,
	max_reference_length  = UINT16_MAX,
	max_struct_nesting    = 64,
	max_array_nesting     = 64,
	max_array_items       = 2147483647,
	max_text_bytes        = 2147483647,
	/* No max_file_size default: 0 == BVNR_FILESIZE_UNLIMITED (endless). */
} bvn_limit_defaults_t;
typedef struct bvnr_serializer_s {
	bvnr_sink_t		sink;
	bool			pretty;
	uint32_t		indent;
	bool			need_semi;
	bool			finished;
	bool			stream_begun;
	bool			version_emitted;
	/* The minor component actually declared, so a guard can require a
	 * SPECIFIC version rather than merely "some directive was written".
	 * version_emitted alone cannot tell 1.0 from 1.2, and the spec-1.2 unit
	 * profile has to know the difference: emitting "ucum:..." under a "#!bovnar
	 * 1.1" directive would produce a document this library then refuses to
	 * read. Only ever read alongside version_emitted. */
	uint16_t		version_major;
	uint16_t		version_minor;
	uint32_t		array_depth;
	uint8_t			max_array_nesting;
	uint8_t			max_struct_nesting;
	bool			arr_need_comma[UINT8_MAX+1];
	/* emitted_unit as it stood when each array row opened: whether the
	 * annotation that governs the ROW carried the unit. An element's own
	 * annotation overrides it for that element only; every later sibling
	 * goes back to this. Without it an inner "<uint:8>" left emitted_unit
	 * false for the rest of the outer array, whose elements then each
	 * gained an inline unit -- which no reader accepts inside "[...]". */
	bool			arr_emitted_unit[UINT8_MAX+1];
	/* Bytes written when the current annotation's "<" went out, so its
	 * length can be held to the reader's cap (BVN_TYPE_BUF_CAP - 1). */
	uint64_t		ann_start;
	bool			in_octet_stream;
	uint32_t		struct_depth;
	uint32_t		struct_depth_at_array_start[UINT8_MAX+1];
	/* Event-grammar state for bvn_writer_validate_event. The serializer assumes a
	 * well-ordered event stream — it happily writes ">" for a lone annotation_end
	 * or ".k=.k=" for two assignments in a row — so the ordering has to be
	 * enforced before any bytes are produced, not discovered by the reader later. */
	bool			w_awaiting_value;  /* assignment_start, value not yet written */
	bool			w_in_annotation;   /* between annotation_start and _end */
	bool			w_after_row_end;   /* a row just closed; "/" may follow */
	bool			w_after_dim;       /* a "/" was written; a row may follow */
	bool			w_stream_ended;    /* ev_stream_end seen; nothing may follow */
	bool			had_type_annotation;
	bool			emitted_type_param;
	bool			emitted_unit;	/* a unit was emitted in the annotation,
						 * so an inline unit on the value need
						 * not (and must not) be re-appended */
	void*			event_userdata;
	bool			(*on_event)(void* userdata, bvnr_event_t e, bvnr_data_t* data);
	bvn_unit_flags_t	unit_flags;
	/* Set when serialisation fails for a reason of its own rather than because
	 * the sink filled up — a BVN_UNIT_REDUCE rescale that cannot be written
	 * exactly, say. bvnr_write_event prefers this over the generic sink error, so
	 * the caller is told what actually went wrong. error_none means "no reason of
	 * my own; assume the sink". */
	error_code_t		ser_error;
	/* When a BVN_UNIT_REDUCE rescale replaced the value's digits, the text that
	 * actually reached the sink, so bvnr_write_event can show the observer what it
	 * wrote rather than what the caller passed. Owned; freed and cleared per
	 * event. */
	char*			ser_value_text;
	uint32_t		ser_value_len;
	value_unit_t		ser_value_unit;   /* the unit that went with it */
	value_unit_t		ser_reduced_unit; /* scratch: the unit being emitted */
	uint8_t			wbuf[BVN_SER_WBUF_SIZE];
	uint32_t		wbuf_pos;
} bvnr_serializer_t;
#define BVN_UNIT_IS_NO_UNIT(u) \
	((u).num_components == 1u && (u).components[0].base == bu_none)
/* A value carrying no unit at all, in either of the two shapes that means:
 * `no_unit` written out (num_components == 0) and no unit parameter at all
 * (one bu_none component). Both must be fenced off from unit-bearing targets,
 * because a bare number IS dimensionally compatible with % and ppm — see
 * bvnr_unit_policy_t. */
#define BVN_UNIT_IS_UNITLESS(u) \
	((u).num_components == 0u || BVN_UNIT_IS_NO_UNIT(u))
/* The opposite: a value that does carry a unit, and so has a spelling. */
#define BVN_UNIT_IS_REAL(u) (!BVN_UNIT_IS_UNITLESS(u))
/* Parsed form of bvnr_unit_policy_t. Held as one struct so bvn_val_init can
 * carry it across the memset that re-arms the validator for a new document: a
 * policy belongs to the consumer, not to the document it was set for. */
/* Parsed form of one bvnr_unit_rule_t. The path is COPIED, because the public
 * contract is that the caller's strings need not outlive the setter call. */
typedef struct bvn_unit_rule_state_s {
	uint8_t		path_len;
	bool		wildcard;   /* the path ended in ".*"; path holds the prefix */
	uint8_t		mode;       /* bvnr_unit_rule_mode_t */
	uint32_t	base;
	value_unit_t	unit;
	char		path[BVNR_MAX_UNIT_PATH + 1];
} bvn_unit_rule_state_t;
typedef struct bvn_unit_policy_state_s {
	bool		active;
	bool		require_unit;
	uint8_t		normalise;    /* bvnr_unit_normalise_t */
	uint8_t		on_inexact;   /* bvnr_unit_inexact_policy_t */
	uint32_t	normalise_base;
	uint32_t	num_targets;
	uint32_t	num_require;
	uint32_t	num_rules;
	value_unit_t	target[BVNR_MAX_UNIT_TARGETS];
	uint32_t	target_base[BVNR_MAX_UNIT_TARGETS];
	value_unit_t	require[BVNR_MAX_UNIT_TARGETS];
	bvn_unit_rule_state_t rule[BVNR_MAX_UNIT_RULES];
} bvn_unit_policy_state_t;
/*
 * Where the value being read or written sits, as a dotted key path.
 *
 * The streaming tiers had no key context at all before per-field rules needed
 * one — a value arrived knowing its type and unit and nothing about where it
 * came from. This is the smallest thing that supplies it: a flat text buffer
 * holding the enclosing struct prefix (".a.b"), a mark per open struct so a
 * close can truncate back, and the key of the assignment currently in flight.
 *
 * `lost` is the honest answer to a path that will not fit — too deep, or a
 * component longer than the buffer allows. Once one push is lost every deeper
 * push is lost too, whether or not it would have fitted, so pops stay balanced
 * and the path can never be silently WRONG: while lost > 0 the position is
 * unknown and no rule may match it. Matching the wrong field is the one failure
 * mode a per-field rule must not have.
 */
#define BVN_PATH_MAX_DEPTH  32u
#define BVN_PATH_MAX_BYTES  256u
#define BVN_PATH_MAX_KEY    64u
typedef struct bvn_key_path_s {
	uint32_t	len;                        /* bytes of `buf` in use */
	uint32_t	depth;                      /* structs pushed into `mark` */
	uint32_t	lost;                       /* unrepresentable pushes */
	uint8_t		pending_len;                /* key of the current assignment */
	bool		pending_lost;
	uint32_t	mark[BVN_PATH_MAX_DEPTH];
	char		buf[BVN_PATH_MAX_BYTES];
	char		pending[BVN_PATH_MAX_KEY];
} bvn_key_path_t;
void bvn_path_reset(bvn_key_path_t* p);
void bvn_path_set_key(bvn_key_path_t* p, const uint8_t* key, uint32_t len);
void bvn_path_push(bvn_key_path_t* p);
void bvn_path_pop(bvn_key_path_t* p);
/* Full path of the value in flight (prefix + pending key) into `out`. Returns
 * false when the position is unknown — see `lost` above — in which case no rule
 * may be applied to this value. */
bool bvn_path_current(const bvn_key_path_t* p, char* out, uint32_t cap,
		      uint32_t* out_len);
/* bvn_policy_match_rule could not tell WHERE this value sits, so it cannot say
 * whether a rule names it. Distinct from -1 ("no rule matches"), and the caller
 * must not treat the two alike: a rule is an assertion the caller asked for by
 * naming a field, and "I could not check it" is not "it holds". See the note at
 * the call site in bvn_apply_unit_assertions. */
#define BVN_POLICY_PATH_UNKNOWN (-2)
/* Index of the first rule whose path matches, -1 if none does, or
 * BVN_POLICY_PATH_UNKNOWN if the position could not be established. */
int32_t bvn_policy_match_rule(const bvn_unit_policy_state_t* pol,
			      const bvn_key_path_t* path);
#define BVN_TYPE_CACHE_KEY_CAP  64u
#define BVN_IU_CACHE_KEY_CAP    32u
#define BVN_TYPE_CACHE_UBUF_CAP 64u
#define BVN_TYPE_CACHE_SLOTS    16u
#define BVN_IU_CACHE_SLOTS      16u
typedef struct bvn_type_cache_slot_s {
	uint16_t		key_len;
	uint8_t			ubuf_len;
	bool			type_ok;
	bool			unit_ok;
	bool			unit_too_long;
	value_type_spec_t	vtype;
	value_unit_t		unit;
	uint8_t			key[BVN_TYPE_CACHE_KEY_CAP];
	uint8_t			ubuf[BVN_TYPE_CACHE_UBUF_CAP];
} bvn_type_cache_slot_t;
typedef struct bvn_iu_cache_slot_s {
	uint8_t			key_len;
	bool			ok;
	value_unit_t		unit;
	uint8_t			key[BVN_IU_CACHE_KEY_CAP];
} bvn_iu_cache_slot_t;
typedef struct bvnr_validator_s {
	value_type_spec_t	value_type;
	value_unit_t		parsed_unit;
	value_type_spec_t	inferred_default_vtype;
	/* 64 bits: it moves once per annotation, and a frame restores its unit
	 * when the serial has MOVED -- a 32-bit one comes back to where it was
	 * after 2^32 annotations inside one array element. */
	uint64_t		parsed_unit_serial;
	uint64_t		acc_value;
	bool			acc_overflow;
	bool			acc_has_dot;
	bool			acc_has_exp;
	uint8_t			acc_exp_state;
	uint8_t			unit_data_len;
	bool			has_annotation_unit;
	/* The spelling parsed_unit was read from (bvnr_data_t.unit_source). The
	 * invariant is one-directional: WHENEVER parsed_unit is a real unit, this
	 * is its source text. It is not cleared when parsed_unit goes back to "no
	 * unit" -- nothing reads it then -- so it must be set at every site that
	 * gives parsed_unit a real unit and saved/restored wherever parsed_unit
	 * is (the array frames), under the same condition. */
	uint8_t			unit_src_len;
	uint8_t			unit_src[BVNR_UNIT_SOURCE_MAX + 1u];
	void*			userdata;
	error_code_t		last_error;
	uint64_t		error_line;
	uint64_t		error_column;
	uint32_t		error_byte;
	uint64_t		error_offset;
	bool			(*on_unverified)
			(void* userdata, bvnr_event_t e, bvnr_data_t* data);
	bool			(*on_verified)
			(void* userdata, bvnr_event_t e, bvnr_data_t* data);
	bool			(*want_unit)
			(void* userdata, const bvnr_data_t* data,
			 value_unit_t* want, uint32_t* want_base);
	bool			want_unit_allow_nonterminating;
	uint32_t		max_conversion_length;
	bvn_unit_policy_state_t	policy;
	bvn_key_path_t		path;
	/* Rule matched for the value in flight, decided once by the assertion pass
	 * and reused by the conversion pass; -1 when no rule names this value. */
	int32_t			cur_rule;
	/* Scratch for lossless want_unit conversion, allocated lazily and reused per
	 * value; freed in bvn_val_free. num/den hold the exact converted rational,
	 * conv_text the rendered string handed to the callback. */
	bvn_int_t*		conv_num;
	bvn_int_t*		conv_den;
	char*			conv_text;
	uint32_t		conv_text_cap;
	bvnr_on_error_fn	on_error;
	uint8_t			tcache_count;
	uint8_t			tcache_next;
	uint8_t			iucache_count;
	uint8_t			iucache_next;
	bvn_type_cache_slot_t	tcache[BVN_TYPE_CACHE_SLOTS];
	bvn_iu_cache_slot_t	iucache[BVN_IU_CACHE_SLOTS];
} bvnr_validator_t;
struct bvnr_reader_s {
	bvnr_lexer_t		lex;
	bvnr_validator_t	val;
};
struct bvnr_writer_s {
	bvnr_serializer_t	ser;
	/* val.unit_src doubles as the one-shot spelling handed over by
	 * bvnr_writer_set_unit_source: the writer has no parsed_unit history of its
	 * own to keep there. */
	bvnr_validator_t	val;
	/* bvnr_data_t.unit_source is checked by reading it back (see
	 * bvn_writer_check_unit_source). The reader that does it is created on
	 * first use and kept; the last spelling that passed is remembered so a
	 * document repeating one unit pays for one check, not one per value. */
	bvnr_reader_t*		usrc_probe;
	/* Keys started since bvnr_writer_set_unit_source without the spelling
	 * being consumed: the second one means the value it was set for took
	 * no unit. */
	uint8_t			usrc_keys;
	/* The unit the annotation in flight (or just closed) names, taken from
	 * its start event or, failing that, its unit parameter -- and whether
	 * that annotation is still waiting for the value it governs. This is
	 * what a value is held against: the serialiser writes the annotation's
	 * unit and never looks at the value's, so `<float:64,k~g>` followed by a
	 * value saying metres used to be written as kilograms. */
	value_unit_t		ann_unit;
	/* The unit of the annotation that GOVERNS the next value -- ann_unit once
	 * its annotation has closed, carried through "[...]" exactly the way the
	 * serialiser carries emitted_unit (saved per row, restored after every
	 * element), and "none" where no annotation with a unit is in force. */
	value_unit_t		gov_unit;
	/* The family that annotation declares. */
	value_type_family_t	ann_family;
	value_unit_t		arr_gov_unit[UINT8_MAX+1];
	/* Whether ANY annotation governs the next value, scoped the same way:
	 * the reader refuses a struct under one at its "{" (`<uint:8> {}`). */
	bool			gov_typed;
	bool			arr_gov_typed[UINT8_MAX+1];
	/* The type of the last annotation written in this assignment, which is
	 * what the reader's datetime inheritance (doc/03 §7.5) keys on: a bare
	 * integer after a datetime-annotated element is read as that datetime. */
	value_type_spec_t	last_ann_type;
	/* The TYPE that annotation declares, as it was actually written (the
	 * family from the family event, the width, base and Q from whichever
	 * parameter events came), and scoped exactly like gov_unit. A value is
	 * validated against this and not against its own event's value_type:
	 * `<uint:8>` followed by a data event saying uint:64, or saying nothing,
	 * used to be range-checked as what the event claimed and written as 300
	 * under the annotation. Meaningful only while gov_typed is set. */
	value_type_spec_t	ann_type;
	value_type_spec_t	gov_type;
	value_type_spec_t	arr_gov_type[UINT8_MAX+1];
	/* Elements written into the row open at each depth, and the width the
	 * "/"-sequence that row belongs to has established. */
	uint64_t		arr_count[UINT8_MAX+1];
	uint64_t		arr_dim_count[UINT8_MAX+1];
	bool			arr_dim_set[UINT8_MAX+1];
	bool			usrc_ok_inline;
	uint8_t			usrc_ok_len;
	uint8_t			usrc_ok[BVNR_UNIT_SOURCE_MAX + 1u];
	value_unit_t		usrc_ok_unit;
};
/* Does `u` satisfy the policy's require_unit / require_dimension_of? Shared by
 * the reader and the writer so the two cannot drift into meaning different
 * things by the same policy — the point of a producer-side check is that it is
 * the SAME check the consumer will apply. */
bool bvn_unit_policy_accepts(const bvn_unit_policy_state_t* p, value_unit_t u);
/* May a value in `native` be delivered as `target`? Dimensionally convertible,
 * and never bridging a unit-bearing value and a bare one in either direction. */
bool bvn_policy_selects(value_unit_t native, value_unit_t target);
void bvn_val_init(bvnr_validator_t* v, bvnr_read_flags_t* opts);
void bvn_val_free(bvnr_validator_t* v);
bool bvn_val_receive(bvnr_reader_t* r, const bvnr_raw_token_t* raw);
bool bvn_val_receive_event(bvnr_reader_t* r, bvnr_event_t ev);
bool bvn_val_receive_octet_chunk(
	bvnr_reader_t* r, const uint8_t* data, uint32_t len);
bool bvn_val_on_value_intro(bvnr_reader_t* r);
bool bvn_val_on_value_outro(bvnr_reader_t* r);
bool bvn_val_on_array_intro(bvnr_reader_t* r);
bool bvn_val_on_array_outro(bvnr_reader_t* r,
	uint64_t curr_row_size, uint64_t* array_row_size);
bool bvn_val_on_new_array_value(bvnr_reader_t* r,
	uint64_t curr_row_size, uint64_t array_row_size);
bool bvn_validate_type_value_compat(bvnr_reader_t* r,
	token_type_t tt, const uint8_t* str, uint32_t str_len);
bool bvn_check_acc_range(bvnr_validator_t* v,
	const uint8_t* str, uint32_t str_len,
	token_type_t tt);
void bvn_acc_reset(bvnr_validator_t* v);
void bvn_acc_digit(bvnr_validator_t* v, uint32_t dv, uint32_t base);
bool bvn_ser_serialize_event(
	bvnr_serializer_t* s, bvnr_event_t ev, bvnr_data_t* d);
bool bvn_ser_flush_wbuf(bvnr_serializer_t* s);
bool bvn_ser_finish_stream(bvnr_serializer_t* s);
bool bvn_ser_emit_version(bvnr_serializer_t* s, uint16_t major, uint16_t minor);
bool bvn_writer_set_error(bvnr_writer_t* w, error_code_t err);
#endif
