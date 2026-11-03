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

#ifndef BOVNAR_H_
#define BOVNAR_H_
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "bvn_float.h"
#include "bvn_int.h"
#ifndef BVN_API
#  if defined(_WIN32) || defined(__CYGWIN__)
#    define BVN_API  /* DLL symbols exported via CMake WINDOWS_EXPORT_ALL_SYMBOLS */
#  elif defined(__GNUC__) || defined(__clang__)
#    define BVN_API __attribute__((visibility("default")))
#  else
#    define BVN_API
#  endif
#endif
#ifdef __cplusplus
extern "C" {
#endif
/*
 * LIBRARY version. The reference implementation versions in lockstep with the
 * Python package (pyproject.toml, bovnar.__version__). BVNR_VERSION is an int
 * that increases monotonically and is safe for #if comparison, e.g.
 * #if BVNR_VERSION >= 20000.
 *
 * NOT the format version -- that is BVNR_SPEC_VERSION_* below, and since 2.0.0
 * the two are different numbers on purpose. The library's major counts breaks
 * in what a PROGRAM is built against: 2.0.0 broke the ABI and the Python API
 * while the format it reads grew only additively, to spec 1.2. A document is
 * versioned by its "#!bovnar" directive and never by this.
 */
#define BVNR_VERSION_MAJOR		2
#define BVNR_VERSION_MINOR		0
#define BVNR_VERSION_PATCH		0
#define BVNR_VERSION			((BVNR_VERSION_MAJOR) * 10000 + \
					 (BVNR_VERSION_MINOR) * 100 + \
					 (BVNR_VERSION_PATCH))
#define BVNR_VERSION_STRING		"2.0.0"
/*
 * Pre-release marker for a build made from a tree that is AHEAD of the released
 * version above. "-dev" while CHANGELOG.md still has an [Unreleased] section;
 * "" as part of cutting a release, in the same commit that closes that section.
 *
 * It exists because the bare version is a claim a build from an unreleased tree
 * cannot honestly make: between 1.1.0 and 2.0.0 the tree carried an ABI break,
 * new error codes and a unit notation that 1.1.0 did not have, so a
 * `bovnar version` reading "1.1.0" sent a bug report against a release whose
 * behaviour was not what the reporter ran. Set it back to "-dev" in the first
 * commit after a release that opens a new [Unreleased] section.
 *
 * DELIBERATELY NOT PART OF BVNR_VERSION_STRING, and not a new API symbol. Those
 * feed places that must stay a bare numeric triple -- the CMake project version
 * (the release workflow greps it with a [0-9]+\.[0-9]+\.[0-9]+ regex),
 * pyproject.toml's PEP 440 version, and the wheel smoke test that asserts
 * bovnar.version() equals it. The suffix is applied where the mistake actually
 * happens: the `bovnar version` banner a human reads.
 */
#define BVNR_VERSION_SUFFIX		""
#define BVNR_VERSION_STRING_FULL	BVNR_VERSION_STRING BVNR_VERSION_SUFFIX
/*
 * Highest bovnar *spec* version this build understands. A document may declare
 * the spec version it targets with a leading "#!bovnar <major>.<minor>"
 * directive (see the spec, §"Version directive"); the reader parses it, exposes
 * it via bvnr_reader_get_declared_version(), and — when bvnr_read_flags_t
 * .strict_version is set — refuses a document whose declared version exceeds
 * what this build supports. The spec version tracks the language; it is bumped
 * only when the grammar/type system grows, independently of the release patch
 * level above.
 */
#define BVNR_SPEC_VERSION_MAJOR		1
#define BVNR_SPEC_VERSION_MINOR		2
/*
 * Components in one compound unit. value_unit_t embeds the array inline and is
 * passed BY VALUE across this API, so this number is the struct's size: at 32 a
 * value_unit_t is 516 bytes against the 132 it was at 8. That is deliberate —
 * the old limit turned real UCUM and UDUNITS expressions into
 * error_unit_too_many_components — but it is an ABI break, and any caller
 * compiled against the old header must be rebuilt rather than relinked.
 */
#define BVNR_MAX_UNIT_COMPONENTS		32
/*
 * Bytes a buffer needs to hold ANY unit bvn_unit_to_string can emit, NUL
 * included. Size unit buffers from this rather than by eye.
 *
 * The shape of the worst case is BVNR_MAX_UNIT_COMPONENTS components, each the
 * longest prefixable canonical symbol with a two-byte prefix, a '~', and the
 * eleven-byte "⁻¹⁰⁰" — negative exponents render at full width only when EVERY
 * component is negative, because a mixed unit moves them into the denominator
 * and renders them positive — joined by two-byte '·' separators. A profile
 * spelling is bounded separately and lands higher, because "udunits:" and its
 * long spelled-out atoms cost more than the native symbols do: the worst case
 * there is 32 components of UDUNITS' longest code, each with a signed exponent
 * and a separator.
 *
 * THE NUMBERS ARE NOT WRITTEN HERE, deliberately. They move with the catalogue:
 * the native worst case was 798 bytes when it was "fl_oz_uk" (8 bytes) and is
 * 862 now that `footlambert` (11) has overtaken it, and a comment stating the
 * old figure outlived two catalogue growths while the macro below stayed
 * correct. gen_units.py and gen_profiles.py both print the live figure against
 * this cap on every build, and test_longest_unit_fits_the_declared_bound sweeps
 * the table for the extreme rather than naming a symbol that can be overtaken.
 *
 * gen_units.py and gen_profiles.py recompute that bound from src/gendata on
 * every build and fail if a symbol or a code outgrows this, because the margin
 * is data-driven and the failure is not graceful: bvn_unit_to_string returns -1
 * WITHOUT writing a NUL, so a caller that ignores the return and hands the
 * buffer to "%s" reads off the end of it. That was a live stack-buffer-overflow
 * in `bovnar events -d`, reachable from a legal document with a long compound
 * unit.
 */
#define BVNR_UNIT_STRING_MAX			1088u
/* Longest unit spelling a document can carry in one unit slot, and therefore
 * the longest bvnr_data_t.unit_source the reader delivers or the writer takes.
 * It is reachable as an inline suffix. A type annotation is capped at 255 bytes
 * AS A WHOLE, so there a spelling shares the room with the family and every
 * parameter before it, and the writer refuses one that would not fit. */
#define BVNR_UNIT_SOURCE_MAX			255u
#define BVN_MAX_INT_WIDTH			32768u
/*
 * Sentinel for bvnr_read_flags_t.max_file_size and bvnr_write_flags_t.max_file_size:
 * a value of 0 removes the byte-count cap entirely, enabling unbounded ("endless")
 * streaming through the lexer/validator. This is the default for a zero-initialised
 * flags struct, so endless streams accumulate no byte-count limit unless a cap is
 * requested explicitly. Every byte/offset/line/column counter in the core is already
 * 64-bit, so a single never-ending document (or octet stream) can run to 2^64-1 bytes;
 * the only limit is this policy knob. To impose a cap, set max_file_size to a positive
 * byte count (e.g. 16 MiB for production); once the processed byte count exceeds it the
 * reader raises error_file_too_long.
 */
#define BVNR_FILESIZE_UNLIMITED			0u
typedef enum bvnr_event_e {
	ev_stream_start,
	ev_assignment_start,
	ev_octet_stream_start,
	ev_octet_stream_end,
	ev_struct_start,
	ev_struct_end,
	ev_array_row_start,
	ev_array_row_end,
	ev_array_dim_start,
	ev_data,
	ev_type_annotation_start,
	ev_type_annotation_end,
	ev_type_annotation_type_family,
	ev_type_annotation_type_family_parameter,
	ev_stream_end
} bvnr_event_t;
typedef enum value_type_family_e {
	vt_plain,
	vt_utf8,
	vt_sint,
	vt_uint,
	vt_float,
	vt_float_fix,
	vt_float_dec,
	vt_bool,
	/* Timestamp (spec 1.1). Carried as a signed integer count of seconds since
	 * a named epoch; the epoch is selected by an annotation parameter and stored
	 * as a small dense index in value_type_spec_t.base (0 = unix, the default).
	 * Use bvnr_datetime_epoch_name() / bvnr_datetime_epoch_mjd() to recover the
	 * epoch, then the bvn_dt_* helpers in bvn_datetime.h to convert to civil time. */
	vt_datetime,
	vt_illegal
} value_type_family_t;
typedef enum token_type_e {
	token_is_identifier,
	token_is_string,
	token_is_number,
	token_is_symbol,
	token_is_reference,
	token_is_array_number,
	token_is_array_string,
	token_is_type,
	token_is_octet_stream,
	token_is_null_value,
	token_is_structure,
	token_is_unit,
	token_is_type_width,
	token_is_type_base,
	token_is_type_q,
	token_is_bool,
	token_is_unknown
} token_type_t;
typedef enum error_code_e {
	error_none                          = 0,
	error_unknown_token_type            = 1,
	error_array_row_size_mismatch       = 2,
	error_identifier_too_long           = 3,
	error_empty_identifier              = 4,
	error_struct_nesting_too_high       = 5,
	error_array_nesting_too_high        = 6,
	error_illegal_struct_close          = 7,
	error_string_too_long               = 8,
	error_illegal_escape_sequence       = 9,
	error_number_too_long               = 10,
	error_symbol_too_long               = 11,
	error_reference_too_long            = 12,
	error_read_complete_chunk_failed    = 13,
	error_octet_stream_out_of_sync      = 14,
	error_unexpected_input_byte         = 15,
	error_text_data_too_long            = 16,
	error_reading_from_source_fd        = 17,
	error_got_incomplete_bvnr_stream    = 18,
	error_invalid_utf8_byte             = 19,
	error_invalid_byte_order_mark       = 20,
	error_type_too_long                 = 21,
	error_unit_too_long                 = 22,
	error_expected_string_in_array      = 23,  /* reserved; never set by the library */
	error_expected_number_in_array      = 24,  /* reserved; never set by the library */
	error_illegal_value_type            = 25,
	error_scanner_callback_failed       = 26,
	error_file_too_long                 = 27,
	error_invalid_argument              = 28,
	error_too_many_array_items          = 29,
	error_writing_to_sink               = 30,
	error_sink_buffer_exhausted         = 31,
	error_unit_illegal                  = 32,
	error_base_requires_string_literal  = 33,
	error_type_value_mismatch           = 34,
	error_value_out_of_range            = 35,
	error_digit_not_in_base             = 36,
	error_recovered                     = 37,  /* reserved; never set by the library */
	error_unit_mismatch                 = 38,
	/* Array element homogeneity (spec 1.0): every non-null element of an array
	 * must share the same kind and, for numbers, the same UNIT (spec 1.2: not
	 * merely the same physical dimension); sibling sub-arrays must
	 * match in length and element shape (recursively); sibling structs must
	 * share the same keys with recursively-matching fields. */
	error_array_element_type_mismatch   = 39,
	error_struct_shape_mismatch         = 40,
	/* A struct (or the top-level document) repeats a key. Keys must be unique
	 * within one scope so lookup, references and iteration always agree. */
	error_duplicate_struct_key          = 41,
	/* A leading "#!bovnar …" version directive is present but malformed — the
	 * "!bovnar" prefix is followed by something that is not a "<major>.<minor>"
	 * decimal version (bad digits, missing dot, trailing junk, overlong). */
	error_invalid_spec_version          = 42,
	/* The document declares a spec version newer than this build supports, and
	 * bvnr_read_flags_t.strict_version was set. Without strict_version the
	 * declared version is stored but not enforced (the parse continues and only
	 * fails if it actually meets syntax the build does not implement). */
	error_unsupported_spec_version      = 43,
	/* spec 1.1 — a \u{…} escape names a value that is not a Unicode scalar
	 * (a surrogate U+D800–U+DFFF, or a code point above U+10FFFF). Malformed
	 * \x / \u *structure* (bad hex, missing brace, empty or over-long braces)
	 * stays error_illegal_escape_sequence; a \u{…} or \xHH that decodes to a
	 * disallowed raw ASCII control byte is rejected by the control-byte rule as
	 * error_unexpected_input_byte, not here. */
	error_invalid_codepoint             = 44,
	/* spec 1.1 — an ISO-8601 datetime literal (e.g. 2026-06-15T12:00:00Z) is
	 * malformed: a field has the wrong width, a separator is misplaced, or a
	 * component is out of range (month 1–12, a valid day-of-month, hour 0–23,
	 * minute 0–59, second 0–60 — 60 is a UTC leap second). Accepted shapes are
	 * YYYY-MM-DD, or YYYY-MM-DDTHH:MM:SS with an optional fractional second and an
	 * optional trailing `Z` (UTC) or numeric `±HH:MM` offset; the fraction and
	 * zone attach only to the time-bearing form (a bare date takes neither). */
	error_invalid_datetime_literal      = 45,
	/* spec 1.1 — an ISO-8601 datetime literal was given for an atomic GNSS
	 * epoch (gps, galileo, glonass, beidou). Those scales have no
	 * round-trippable civil⇄seconds inverse in this build, so a literal there
	 * is rejected; supply an integer epoch-seconds carrier instead. Civil
	 * epochs (unix, mjd, ntp, y2000) and tai accept literals. */
	error_datetime_literal_unsupported_epoch = 46,
	/* A read-time unit/base conversion (bvnr_read_flags_t.want_unit) could not be
	 * performed losslessly: the true conversion factor is irrational (a π-based
	 * angle), or the exact result has no terminating expansion in the requested
	 * output base. The reader refuses to silently round — request a representable
	 * target unit/base, or read the value in its native form. */
	error_unit_inexact                  = 47,
	/* An octet stream ended while a multiplexed message was still being
	 * reassembled: the declared length exceeded the bytes actually present.
	 * Distinct from error_octet_stream_out_of_sync because the octet FRAMING is
	 * well-formed -- the reader is right to succeed -- and only the message
	 * layer is short. bvnr_demux_on_event() latches this and keeps going rather
	 * than aborting the read, so the partial message is dropped (never
	 * delivered, never pre-allocated) but the loss is reportable instead of
	 * indistinguishable from an empty stream. */
	error_octet_stream_truncated        = 48,
	/* The unit profiles (spec 1.2). The split matters to a producer: one of
	 * these says "you wrote it wrong", the other says "you wrote it right and this build cannot carry it", and the
	 * fixes are different. A malformed code, and a code over something the
	 * vocabulary does not define, stay error_unit_illegal. */
	/* The namespace before the ':' is not a profile this build supports. A
	 * consumer reads this as "no such profile compiled in", which a plain
	 * error_unit_illegal could not be distinguished from. */
	error_unit_profile_unknown          = 49,
	/* Valid in its vocabulary, with no representation in the unit model: a
	 * special unit carrying a reference level, a scale factor outside the SI
	 * prefix decades, an expression wider than BVNR_MAX_UNIT_COMPONENTS, or a
	 * construct the unit slot cannot reach at all (UDUNITS reference time). */
	error_unit_profile_unsupported      = 50,
	/* The document contains an octet stream and the reader was opened with
	 * bvnr_read_flags_t.text_only. Not a defect in the document -- an octet
	 * stream is a first-class part of the format -- but an assertion by the
	 * CONSUMER that this particular channel carries text only. The format is a
	 * text/binary hybrid by design, and a consumer whose pipeline is not
	 * (anything that normalises line endings, a log aggregator, a git checkout
	 * without `-text`) can say so at the door rather than discover it as
	 * corruption downstream. */
	error_octet_stream_forbidden        = 51,
	/* Whitespace split a type-annotation parameter in two. The grammar puts
	 * every `ws` inside an annotation beside a separator -- after the family
	 * keyword, either side of the family ':' or of a ',' between parameters, and
	 * before the closing '>' -- and has no production for whitespace in the
	 * middle of a parameter. The lexer used to drop it anyway, which is how
	 * `<float:64,m in>` became the minute, `<uint:6 4>` became a 64-bit width,
	 * and the UDUNITS spelling `udunits:m s-1` would have become a reciprocal
	 * millisecond instead of a speed: the one place in the format where a wrong unit was
	 * produced silently rather than refused. Reported at the first byte after
	 * the whitespace, with its own code because the fix is specific -- delete
	 * the space, or write the multiplication the notation actually has
	 * (`m*s-1`). A byte that is illegal anywhere in an annotation remains
	 * error_unexpected_input_byte. */
	error_type_param_whitespace         = 52
} error_code_t;
typedef enum prefix_system_e {
	prefix_si,
	prefix_iec
} prefix_system_t;
typedef enum si_prefix_id_e {
	si_none = 0,
	/* SI prefixes (si_quecto..si_quetta) — generated from src/gendata/prefixes.bvnr. */
#include "bovnar_si_prefix.gen.h"
} si_prefix_id_t;
typedef enum iec_prefix_id_e {
	iec_none = 0,
	/* IEC prefixes (iec_kibi..iec_quebi) — generated from src/gendata/prefixes.bvnr. */
#include "bovnar_iec_prefix.gen.h"
} iec_prefix_id_t;
/*
 * ===========================================================================
 * THE BASE-UNIT ID SPACE
 * ===========================================================================
 *
 * A base unit's id is not a running counter. It is BLOCKED: the leading two
 * decimal digits name the vocabulary the unit comes from, and the four digits
 * after them are its position within that vocabulary.
 *
 *      block  ids              vocabulary            source
 *      -----  ---------------  --------------------  ----------------------
 *         10  100000..109999   native bovnar units   src/gendata/units.bvnr
 *         20  200000..209999   UCUM                  src/gendata/ucum.bvnr
 *         30  300000..309999   UN/ECE Rec 20/21      src/gendata/unece.bvnr
 *         40  400000..409999   QUDT units            src/gendata/qudt.bvnr
 *         50  500000..509999   QUDT quantity kinds   src/gendata/qudt-qk.bvnr
 *         60  600000..609999   UDUNITS-2             src/gendata/udunits.bvnr
 *         70  700000..709999   OM 2                  src/gendata/om.bvnr
 *         80  800000..809999   CF standard names     src/gendata/cf.bvnr
 *         90  900000..909999   currencies            src/gendata/currencies.bvnr
 *
 * Every tag between the native units and the currencies is now taken; a further
 * vocabulary needs a tag outside 10..90, and the layout above is the ABI it
 * would have to fit into. Only a vocabulary that
 * contributes units of its OWN takes a block: a profile is a spelling for the
 * unit slot, so most of its codes translate to native units and carry native
 * ids. The ones that get a block id are the opaque units — codes with no native
 * equivalent and no dimension (UCUM's assay-defined [IU], UN/ECE's package
 * types), which need an identity of their own precisely because nothing else
 * can stand in for them.
 *
 * WHY BLOCKS. One flat counter makes every vocabulary's ids a function of every
 * other vocabulary's size, and that is not hypothetical: this space previously
 * had physical units at 1..133 and again at 348..396 because the currencies had
 * been dropped in between, two currencies stranded at 378..379 outside their own
 * range because that range had been frozen, and the profile units pinned above
 * all of it. Adding a currency shifted units. Blocks end that — a vocabulary
 * grows into its own 10000 ids and nothing outside it moves — and they make an
 * id self-describing: 200017 is UCUM's, whatever else the build contains.
 *
 * The price is a sparse space, so DO NOT index an array by a base unit id. The
 * library's own tables are dense and one row per defined unit; see
 * BVN_UNIT_SLOT_COUNT and the BVN_SLOT_* macros in the generated headers.
 * bu_none is 0 and belongs to no block.
 *
 * Currencies get ids but no enumerators, unlike every other block: a currency
 * is written by its ISO 4217 code behind a '$' sigil, resolved by
 * bvn_parse_currency_str, and carried as the numeric base value. See
 * bovnar_currency.h.
 */
#define BVN_UNIT_BLOCK_SIZE      10000
#define BVN_UNIT_BLOCK_FIRST(tag)  ((int)(tag) * BVN_UNIT_BLOCK_SIZE)
/* The first id of the first block, i.e. the smallest id any real unit can
 * have — everything below it is bu_none or nothing at all. */
#define BVN_UNIT_ID_FIRST        BVN_UNIT_BLOCK_FIRST(10)
typedef enum value_base_unit_e {
	bu_none = 0,
	/*
	 * The native units, block 10 — generated from src/gendata/units.bvnr by
	 * gen_units.py, which also emits this block's bounds and BVN_SLOT_NATIVE.
	 */
#include "bovnar_units.gen.h"
	/*
	 * The unit profiles' opaque units (spec 1.2), one block each — generated
	 * from the profile data files by gen_profiles.py, which also emits every
	 * block's bounds, its BVN_SLOT_* macro and BVN_UNIT_SLOT_COUNT.
	 *
	 * Assay-defined quantities and package counts that are commensurable with
	 * nothing, not even with each other, so each needs its own id: collapsing
	 * them onto one would make [IU] and [PFU] compare equal. They have no
	 * native spelling and are reachable only through the profile notation.
	 */
#include "bovnar_profiles.gen.h"
} value_base_unit_t;
/*
 * A component's exponent, as an integer in [BVN_EXPONENT_MIN,
 * BVN_EXPONENT_MAX] with ZERO RESERVED for "not an exponent".
 *
 * The named enumerators below cover ±1..±9 and are kept because they are what
 * existing source says; they are NOT the whole domain. Any integer in the range
 * is a valid exponent and bvn_int_to_exponent will produce it, so a switch over
 * this type must have a default and code must not assume it can enumerate the
 * values. exp_invalid is zero: a component with exponent 0 is malformed, not a
 * dimensionless one, and every consumer already tests for it.
 *
 * The bounds are enumerators too, which is what forces the underlying type wide
 * enough to hold them on every conforming compiler. The field has been four
 * bytes since before the range grew, so widening it cost no ABI change.
 */
#define BVN_EXPONENT_MIN	(-100)
#define BVN_EXPONENT_MAX	( 100)
typedef enum unit_exponent_e {
	exp_range_min  = BVN_EXPONENT_MIN,
	exp_range_max  = BVN_EXPONENT_MAX,
	exp_invalid    =   0,
	exp_linear     =   1,
	exp_square     =   2,
	exp_cubic      =   3,
	exp_quartic    =   4,
	exp_quintic    =   5,
	exp_sextic     =   6,
	exp_septic     =   7,
	exp_octic      =   8,
	exp_nonic      =   9,
	exp_neg_linear =  -1,
	exp_neg_square =  -2,
	exp_neg_cubic  =  -3,
	exp_neg_quartic=  -4,
	exp_neg_quintic=  -5,
	exp_neg_sextic =  -6,
	exp_neg_septic =  -7,
	exp_neg_octic  =  -8,
	exp_neg_nonic  =  -9
} unit_exponent_t;
typedef struct value_type_spec_s {
	value_type_family_t	family;
	uint32_t		width;
	uint32_t		base;
} value_type_spec_t;
typedef struct value_unit_prefix_s {
	prefix_system_t system;
	union {
		si_prefix_id_t  si;
		iec_prefix_id_t iec;
	} id;
} value_unit_prefix_t;
typedef struct value_unit_component_s {
	value_base_unit_t   base;
	unit_exponent_t     exponent;
	value_unit_prefix_t prefix;
} value_unit_component_t;
typedef struct value_unit_s {
	uint32_t			num_components;
	value_unit_component_t		components[BVNR_MAX_UNIT_COMPONENTS];
} value_unit_t;
/* Result of a lossless read-time unit/base conversion (bvnr_read_flags_t.want_unit).
 * The value is delivered EXACTLY: `text` is its full positional expansion in
 * `base` (the requested output base, or the value's own if none was requested),
 * and num/den are the exact reduced rational (den > 0). Every field references
 * reader-owned storage valid only for the duration of the callback — copy `text`
 * (or num/den, via bvn_int_copy) if you need to retain it. `text`/num/den never
 * lose precision, whatever the value's width or base; a conversion that could not
 * be exact (an irrational factor) never reaches you — it aborts the parse with
 * error_unit_inexact.
 *
 * `text` is NULL (and `length` 0) in exactly one case: the exact result has no
 * terminating expansion in `base` — 5/18 m/s in base 10, 1/1000 km in base 2 —
 * AND the reader was told to deliver it anyway via
 * bvnr_read_flags_t.want_unit_allow_nonterminating. num/den are still exact
 * there; without that flag such a value aborts the parse instead. Always check
 * `text` for NULL before printing it. */
struct bvn_int_s;   /* bvn_int.h — arbitrary-precision integer */
typedef struct bvnr_converted_s {
	value_unit_t		unit;     /* the target unit */
	const char*		text;     /* exact value in `base`, NUL-terminated;
					   * NULL if it does not terminate in `base` */
	uint32_t		length;   /* strlen(text), 0 when text is NULL */
	uint32_t		base;     /* base text is rendered in (2..62, 64, 85) */
	const struct bvn_int_s*	num;      /* exact numerator (signed) */
	const struct bvn_int_s*	den;      /* exact denominator (> 0) */
} bvnr_converted_t;
typedef struct bvnr_data_s {
	token_type_t		type;
	value_type_spec_t	value_type;
	value_unit_t		value_unit;
	const void*		data;
	uint32_t		length;
	/* spec 1.1 — sub-second digits of an ISO-8601 datetime literal, captured
	 * verbatim (no leading "0.", no trailing-zero trimming) so consumers can
	 * see fractional seconds the integer carrier cannot hold. NULL/0 for every
	 * value except a vt_datetime that was written as a literal with a `.frac`
	 * part. The carrier in `data` is still the whole-second epoch integer; the
	 * fraction is informational (not used in arithmetic) and survives a
	 * pretty-print round-trip via re-emission of the ISO literal. */
	const void*		frac_data;
	uint32_t		frac_length;
	/* library 2.0 — the unit exactly as the producer SPELLED it, captured verbatim.
	 * `value_unit` above is what the unit means; this is what was written, and
	 * the two differ in the one way formatting a value_unit_t is lossy: a unit
	 * profile's spelling ("ucum:mm[Hg]" for mmHg) and above all a UCUM
	 * annotation ("ucum:mL{total}" parses to m~L and "{total}" has nowhere to
	 * live in the struct).
	 *
	 * READER: set on ev_data and on the type-annotation events whenever
	 * value_unit is a real unit, NULL/0 otherwise (no unit, `no_unit`, a
	 * datetime's epoch). It is the annotation's unit parameter, or the inline
	 * suffix when the annotation carried no unit; where a value has both, the
	 * annotation's spelling. Reader-owned, valid for the callback only, NOT
	 * NUL-terminated. A want_unit conversion leaves it alone: it is the
	 * spelling of value_unit, never of conv.unit.
	 *
	 * WRITER: when non-NULL, bvnr_write_event emits it in place of the
	 * canonical spelling of value_unit -- in the annotation's unit parameter
	 * and in an inline suffix alike. It is checked first, because a spelling
	 * that disagrees with the unit is a document that says two things:
	 *   error_unit_mismatch            it parses to a different unit
	 *   error_unit_illegal / error_unit_profile_*   it does not parse
	 *   error_unit_too_long            over BVNR_UNIT_SOURCE_MAX bytes, or too
	 *                                  long for what is left of an
	 *                                  annotation's 255 -- either way more
	 *                                  than a reader could take back
	 *   error_unit_illegal             as an inline suffix on an array
	 *                                  element: the format has none there
	 *   error_unsupported_spec_version it is profile notation and no
	 *                                  "#!bovnar 1.2" directive was emitted
	 * The unit parameter's `data` text -- what the reader fills there and every
	 * caller before 2.0 used -- is held to the same contract; where both are
	 * set they must agree. "The unit" is the event's own value_unit, or, where
	 * a parameter event carries none, the one given on the annotation's start
	 * event -- which is written as the annotation's unit parameter if no
	 * parameter event follows, and which a parameter event may not contradict.
	 * And a VALUE under an annotation that names a unit must carry that unit
	 * or none: the serialiser writes the annotation's, so a value saying
	 * otherwise is error_unit_mismatch. That holds for every element of an
	 * annotated array, in every row of it, and for `no_unit` as for a unit.
	 *
	 * One document can be read and not written back: a bare value with an
	 * inline unit of some 243 bytes or more. The reader hands it over inside
	 * a synthesised annotation, where it no longer fits.
	 * With bvnr_write_flags_t.unit_flags set it is IGNORED and the canonical
	 * form is written: BVN_UNIT_REDUCE and BVN_UNIT_ASCII_EXP ask for a
	 * particular spelling, and that request wins. Leave it NULL (zero-init) to
	 * get the canonical form, which is what every caller got before. */
	const void*		unit_source;
	uint32_t		unit_source_length;
	/* Read-time unit/base conversion (see bvnr_read_flags_t.want_unit). When the
	 * reader was asked to convert this numeric value and could do so LOSSLESSLY,
	 * `converted` is true and `conv` carries the exact result — the value in the
	 * requested unit and base, as both a full positional string (conv.text) and
	 * an exact rational (conv.num/conv.den). The `data`/`length` text above is
	 * left untouched (the original digits in the original unit and base). When no
	 * conversion was requested or applied, `converted` is false and `conv` is
	 * zeroed. Every numeric family is eligible — uint/sint of any width and any
	 * base (incl. a multiprecision integer or a non-decimal base written as a
	 * string literal), float, float_dec, float_fix — but never a datetime.
	 *
	 * The conversion is EXACT regardless of the value's width or base: a 1056-bit
	 * float or a 512-bit integer converts with no loss beyond the library's own
	 * declared factor. A conversion that cannot be represented exactly — an
	 * irrational factor (π-based angle) — is never delivered here; it aborts the
	 * parse with error_unit_inexact. A result with no terminating expansion in the
	 * requested base likewise aborts, unless
	 * bvnr_read_flags_t.want_unit_allow_nonterminating is set, in which case it
	 * arrives with conv.text == NULL and the exact conv.num/conv.den. */
	bool			converted;
	bvnr_converted_t	conv;
} bvnr_data_t;
typedef void (*bvnr_on_error_fn)(
	void* userdata, error_code_t err,
	uint64_t line, uint64_t column,
	uint32_t byte, uint64_t offset);
typedef struct bvnr_read_flags_s {
	uint16_t	max_identifier_length;
	uint16_t	max_string_length;
	uint16_t	max_number_length;
	uint16_t	max_symbol_length;
	uint16_t	max_reference_length;
	uint64_t	max_array_items;
	/* Bytes consumed by the TEXT scanner. An octet stream's binary payload does
	 * NOT count towards it -- a 212-byte document whose body is a 200-byte octet
	 * stream is accepted at max_text_bytes = 8. So this does not bound total
	 * input, and using it as a memory cap leaves a large binary payload
	 * unbounded; max_file_size is the one that counts every byte. Both are exact
	 * at their boundary (a document needing N is accepted at N, refused at N-1).
	 * 0 selects the internal default of 2147483647. */
	uint64_t	max_text_bytes;
	/* Bytes of INPUT, octet-stream payloads included -- the total-size cap.
	 * 0 means unlimited (BVNR_FILESIZE_UNLIMITED), and is the default; set it
	 * explicitly in production. It is exact, and independent of how the bytes
	 * arrive: a document of N bytes is read in full at a cap of N, and at a
	 * smaller cap everything up to it is read and delivered before
	 * error_file_too_long is raised at the first byte over it.
	 *
	 * Unlimited is not the same as endless. max_text_bytes above has a finite
	 * default, so with both left at 0 an octet stream may run without bound
	 * but the TEXT of a document stops at 2147483647 bytes
	 * (error_text_data_too_long); raise max_text_bytes as well -- UINT64_MAX
	 * if need be -- for a text stream longer than that. max_array_items has
	 * the same default and bounds one array the same way. */
	uint64_t	max_file_size;
	uint8_t		max_struct_nesting;
	uint8_t		max_array_nesting;
	void*		userdata;
	bool		(*on_unverified)
			(void* userdata, bvnr_event_t e, bvnr_data_t* data);
	bool		(*on_verified)
			(void* userdata, bvnr_event_t e, bvnr_data_t* data);
	bool		continue_on_error;
	bvnr_on_error_fn	on_error;
	/* When true, a leading "#!bovnar <major>.<minor>" directive declaring a
	 * spec version this build does not support -- a newer minor, or any other
	 * major (BVNR_SPEC_VERSION_*) -- is a hard error_unsupported_spec_version, and a
 * malformed directive a hard error_invalid_spec_version: both end the read even
 * under continue_on_error. When false (the zero-init default)
	 * the declared version is recorded — query it with
	 * bvnr_reader_get_declared_version() — but not enforced. Production
	 * consumers that must not silently misread a future document should set it. */
	bool		strict_version;
	/* Refuse a document that contains an octet stream, with
	 * error_octet_stream_forbidden at the stream's opening 0x00.
	 *
	 * The binary region is the one part of a Bovnar document that is not text,
	 * and it is the one part a text-shaped pipeline destroys silently: LF
	 * normalisation rewrites 0x0D inside a chunk payload, and the length prefix
	 * that made the region skippable then points at the wrong byte. The damage
	 * is unrecoverable and looks like a malformed document rather than a
	 * mangled one.
	 *
	 * This is the same move as the unit policy's assertions: the format permits
	 * something, and a consumer that cannot accept it says so at the parse,
	 * where it is checkable, instead of hoping. A producer can use it the same
	 * way to guarantee that a channel stays transport-safe. */
	bool		text_only;
	/* Relaxes the want_unit hook's one unavoidably common refusal. Plenty of
	 * everyday conversions are exact as a rational but have no finite positional
	 * expansion in the output base — km/h to m/s is 5/18, m to km in base 2 is
	 * 1/1000 — and by default those abort the parse with error_unit_inexact
	 * rather than round. Set this when your consumer can take the rational: the
	 * value is then delivered with data->conv.num/den exact and data->conv.text
	 * NULL. A genuinely irrational factor (π-based angle) still aborts, because
	 * there no exact rational exists to hand over. Leave false (the zero-init
	 * default) to keep the strict all-or-nothing behaviour. */
	bool		want_unit_allow_nonterminating;
	/* Longest conv.text a want_unit conversion may produce, in characters; 0
	 * selects BVNR_DEFAULT_MAX_CONVERSION_LENGTH. A conversion whose exact result
	 * would be longer aborts with error_value_out_of_range.
	 *
	 * This is a WORK limit, not a correctness one. Rendering an exact expansion is
	 * quadratic in its digit count, and the count follows the magnitude of the
	 * value's exponent rather than its length: `1e-9800` is seven characters and
	 * expands to 9800 digits, so a kilobyte of such values costs minutes of CPU
	 * while the document itself looks trivial. Like max_number_length and
	 * max_array_items, this is here so a consumer of untrusted input is not at the
	 * mercy of the input's shape. Raise it if you genuinely need thousands of
	 * exact digits; no physical measurement does. */
	uint32_t	max_conversion_length;
	/* Optional read-time LOSSLESS unit/base conversion hook. When non-NULL, the
	 * reader calls it for every numeric value (with or without a unit) after
	 * validation and before EITHER value callback — it can abort the parse, and
	 * the two views of one value must not disagree. An on_unverified consumer
	 * therefore also sees a populated `converted`/`conv` on ev_data, even though
	 * everything else about that event is the pre-validation view. To request a
	 * conversion, fill *want with
	 * the target unit and *want_base with the output base and return true; return
	 * false (or leave this NULL) to receive the value untouched. Inspect
	 * data->value_unit (native unit), data->value_type, and data->data to decide;
	 * track the current key via userdata if the choice is key-specific.
	 *
	 * *want_base accepts any base bvnr can write — 2..62 plus 64 (Base64) and 85
	 * (Ascii85) — or 0 to keep the value's own. Anything else is rejected with
	 * error_invalid_argument rather than quietly substituted; note that 64 and 85
	 * have no sign character, so a negative result in those is also rejected.
	 *
	 * The conversion is performed in exact arbitrary-precision arithmetic, so it
	 * is lossless for any value width and base (a 1056-bit float, a 512-bit
	 * integer). The result arrives with data->converted set and data->conv
	 * carrying the exact value (text + rational) in the requested unit and base;
	 * data->data keeps the original text. Requesting `want` == the native unit
	 * with a different base performs a pure base conversion.
	 *
	 * The parse aborts rather than deliver anything approximate:
	 *   error_unit_mismatch     — `want` is dimensionally incompatible.
	 *   error_unit_inexact      — the true factor is irrational (π-based angle),
	 *                             or the result does not terminate in the output
	 *                             base and want_unit_allow_nonterminating is off.
	 *   error_value_out_of_range— the literal is finite but too extreme to build
	 *                             an exact rational from (e.g. 1e1000000), OR the
	 *                             units agree and the exact FACTOR between them
	 *                             does not fit (`Q~m¹⁰⁰·Q~g¹⁰⁰` to
	 *                             `q~m¹⁰⁰·q~g¹⁰⁰` needs 10^12000). Both are "too
	 *                             large to represent" and neither is a mismatch:
	 *                             a `want` this reports is one the value really
	 *                             could have been converted to. The value is NOT
	 *                             silently passed through unconverted.
	 *   error_invalid_argument  — unusable *want_base, or out of memory.
	 * Only nan/inf are delivered untouched (converted == false, no error): they
	 * carry no finite value to convert. Never consulted for datetime. */
	bool		(*want_unit)
			(void* userdata, const bvnr_data_t* data,
			 value_unit_t* want, uint32_t* want_base);
	uint64_t	_reserved[2];
} bvnr_read_flags_t;
typedef uint32_t bvn_unit_flags_t;
/* Default for bvnr_read_flags_t.max_conversion_length: generous next to any real
 * measurement, small enough that the quadratic render cost stays proportionate
 * to the document. */
#define BVNR_DEFAULT_MAX_CONVERSION_LENGTH 1024u
#define BVN_UNIT_FLAGS_NONE ((bvn_unit_flags_t)0u)
/* Reduce a compound unit to canonical form before writing it: repeated bases
 * combine (m·m -> m²), and every prefix is folded out (km -> m).
 *
 * Folding a prefix out CHANGES THE UNIT, so the writer scales the value to
 * match: `5` with unit `k~m` is written as `5000 m`, not `5 m`. The rescale runs
 * in exact rational arithmetic, so a value far wider than a double keeps every
 * digit. Where the scaled value cannot be written exactly in the value's own
 * base — 1/100 in base 16 needs a factor of five — the write fails with
 * error_value_out_of_range rather than rounding.
 *
 * Note for direct callers of bvn_unit_to_string_ex: that function returns only
 * the reduced unit, so if you format a unit yourself you must move your value
 * with it. The writer does that for you; nothing else does.
 *
 * DO NOT take the scale from bvn_unit_reduce to do it. That is the scale to the
 * FULLY reduced unit, and the formatter does not always emit the fully reduced
 * unit: where the reduction lands on a named SI unit it re-attaches the prefix,
 * so k~N comes back "k~N" with nothing to rescale while bvn_unit_reduce still
 * reports 1000, and k~g comes back "g" where the 1000 must be applied. The two
 * are indistinguishable from outside. Take the scale from the unit actually
 * emitted -- format, parse the result back, convert from the original to that --
 * which is what bvn_ser_reduced_number does; doc/05 §12.2 spells it out. */
#define BVN_UNIT_REDUCE     ((bvn_unit_flags_t)(1u << 0))
#define BVN_UNIT_ASCII_EXP  ((bvn_unit_flags_t)(1u << 1))
/* Render a rescaled FLOAT in float shape: append ".0" when reducing leaves the
 * value an integer string, so "<float:64,k~m> 5.0" reduced to metres comes out
 * as "5000.0" rather than "5000".
 *
 * Separate from BVN_UNIT_REDUCE on purpose. Reduction's own contract is that it
 * writes the exact rescaled value and nothing more -- "2.5 k~g" becomes "2500 g"
 * -- and callers depend on that. This flag is for the CANONICAL document mode,
 * where the goal is different: two documents that mean the same thing must
 * produce the same bytes, and a value that took the rescale path must therefore
 * land on the same spelling as one that did not.
 *
 * It normalises the RESCALED literal only. A literal the producer wrote and that
 * no rescale touched is passed through untouched, so this does not make every
 * float in a document converge -- writing "5000" where another document wrote
 * "5000.0" still differs. Full number-shape canonicalisation is a larger change
 * and is not this flag. */
#define BVN_NUM_CANONICAL   ((bvn_unit_flags_t)(1u << 2))
/*
 * Write options. The fields mirror bvnr_read_flags_t for symmetry, but the
 * writer consults only: max_array_nesting and max_struct_nesting (enforced),
 * userdata + on_event (the post-serialize observer callback), unit_flags, and
 * emit_version. The byte/length/count caps (the max_*_length fields,
 * max_array_items, max_text_bytes, max_file_size), continue_on_error, and
 * on_error are accepted for struct symmetry but are NOT consulted on the write
 * path: the writer emits already-validated values and latches any failure into
 * the writer (query it with bvnr_writer_get_error). Those input caps and the
 * resync/error-callback behaviour apply to the reader (bvnr_read_flags_t).
 */
typedef struct bvnr_write_flags_s {
	uint16_t	max_identifier_length;
	uint16_t	max_string_length;
	uint16_t	max_number_length;
	uint16_t	max_symbol_length;
	uint16_t	max_reference_length;
	uint64_t	max_array_items;
	uint64_t	max_text_bytes;
	uint64_t	max_file_size;
	uint8_t		max_struct_nesting;
	uint8_t		max_array_nesting;
	void*		userdata;
	bool		(*on_event)
			(void* userdata, bvnr_event_t e, bvnr_data_t* data);
	bool		continue_on_error;
	bvnr_on_error_fn	on_error;
	bvn_unit_flags_t	unit_flags;
	/* When true, bvnr_open_write_* emits a leading
	 * "#!bovnar <BVNR_SPEC_VERSION_MAJOR>.<BVNR_SPEC_VERSION_MINOR>" directive
	 * before any value, so the produced document self-declares its spec
	 * version. Equivalent to calling bvnr_write_version() right after open.
	 * Off by default (zero-init). */
	bool		emit_version;
	uint64_t	_reserved[4];
} bvnr_write_flags_t;
typedef struct bvnr_source_s bvnr_source_t;
typedef struct bvnr_sink_s   bvnr_sink_t;
typedef bool (*bvnr_pull_fn)
	(bvnr_source_t* s, void* buf, uint32_t want, uint32_t* got);
typedef bool (*bvnr_push_fn)
	(bvnr_sink_t* s, const void* buf, uint32_t len);
typedef bool (*bvnr_flush_fn)
	(bvnr_sink_t* s);
#define BVNR_SOURCE_RESERVED_SIZE 64
#define BVNR_SINK_RESERVED_SIZE   64
struct bvnr_source_s {
	union {
		void*    _ptr;
		uint64_t _u64;
		uint8_t  _opaque[BVNR_SOURCE_RESERVED_SIZE];
	} _reserved;
};
struct bvnr_sink_s {
	union {
		void*    _ptr;
		uint64_t _u64;
		uint8_t  _opaque[BVNR_SINK_RESERVED_SIZE];
	} _reserved;
};
typedef struct bvnr_reader_s bvnr_reader_t;
typedef struct bvnr_writer_s bvnr_writer_t;
/*
 * The BVN_TYPE_* and BVN_UNIT_* constructors below are C99 compound literals
 * with designated initialisers. The headers are usable from C++ (every
 * declaration is extern "C"), but these macros are not: g++ and clang++ take
 * the plain ones as an extension, with a -pedantic warning, and refuse the
 * compound-unit ones outright. From C++, fill in a value_type_spec_t or a
 * value_unit_t member by member, or call bvn_unit_parse.
 */
#define BVN_TYPE_PLAIN \
	((value_type_spec_t){ .family = vt_plain, .width = 0,  .base = 0  })
#define BVN_TYPE_UTF8 \
	((value_type_spec_t){ .family = vt_utf8,  .width = 0,  .base = 0  })
#define BVN_TYPE_BOOL \
	((value_type_spec_t){ .family = vt_bool,  .width = 0,  .base = 0  })
#define BVN_TYPE_SINT(w) \
	((value_type_spec_t){ .family = vt_sint,  .width = (w), .base = 0  })
#define BVN_TYPE_UINT(w) \
	((value_type_spec_t){ .family = vt_uint,  .width = (w), .base = 0  })
#define BVN_TYPE_FLOAT(w) \
	((value_type_spec_t){ .family = vt_float, .width = (w), .base = 0  })
#define BVN_TYPE_FLOAT_FIX(w, q) \
	((value_type_spec_t){ .family = vt_float_fix, .width = (w), .base = (q) })
#define BVN_TYPE_FLOAT_DEC(w) \
	((value_type_spec_t){ .family = vt_float_dec, .width = (w), .base = 0  })
#define BVN_TYPE_FLOAT_BASE(w, b) \
	((value_type_spec_t){ .family = vt_float, .width = (w), .base = (b) })
#define BVN_TYPE_UINT_BASE(w,b) \
	((value_type_spec_t){ .family = vt_uint,  .width = (w), .base = (b) })
#define BVN_TYPE_SINT_BASE(w,b) \
	((value_type_spec_t){ .family = vt_sint,  .width = (w), .base = (b) })
#define BVN_UNIT_NO_PREFIX(b) \
	((value_unit_t){ \
		.num_components = 1, \
		.components = {{ \
			.base = (b), .exponent = exp_linear, \
			.prefix.system = prefix_si, .prefix.id.si = si_none \
		}} \
	})
#define BVN_UNIT_SI(b, p) \
	((value_unit_t){ \
		.num_components = 1, \
		.components = {{ \
			.base = (b), .exponent = exp_linear, \
			.prefix.system = prefix_si, .prefix.id.si = (p) \
		}} \
	})
#define BVN_UNIT_IEC(b, p) \
	((value_unit_t){ \
		.num_components = 1, \
		.components = {{ \
			.base = (b), .exponent = exp_linear, \
			.prefix.system = prefix_iec, .prefix.id.iec = (p) \
		}} \
	})
#define BVN_UNIT_SI_EXP(b, p, e) \
	((value_unit_t){ \
		.num_components = 1, \
		.components = {{ \
			.base = (b), .exponent = (e), \
			.prefix.system = prefix_si, .prefix.id.si = (p) \
		}} \
	})
#define BVN_UNIT_NONE  ((value_unit_t){ .num_components = 0 })
#define BVN_UNIT_COMPOUND2(b1, p1, e1, b2, p2, e2) \
	((value_unit_t){ \
		.num_components = 2, \
		.components = { \
			{ .base = (b1), .exponent = (e1), \
			  .prefix.system = prefix_si, .prefix.id.si = (p1) }, \
			{ .base = (b2), .exponent = (e2), \
			  .prefix.system = prefix_si, .prefix.id.si = (p2) } \
		} \
	})
static inline bool bvn_type_spec_eq(
	value_type_spec_t a, value_type_spec_t b)
{
	return a.family == b.family && a.width == b.width && a.base == b.base;
}
static inline uint32_t bvn_effective_width(value_type_spec_t s)
{
	return s.width ? s.width : 64u;
}
static inline uint32_t bvn_effective_base(value_type_spec_t s)
{
	/* float_fix/dec are always decimal. datetime's base field holds an epoch
	 * index, NOT a numeric base, and its carrier is decimal epoch-seconds — so
	 * it must report base 10, or DOM/parse paths (bvn_parse_int64, the DOM
	 * builder) would decode the seconds in the bogus "base" and corrupt it. */
	if (s.family == vt_float_fix || s.family == vt_float_dec ||
	    s.family == vt_datetime)
		return 10u;
	return s.base ? s.base : 10u;
}
static inline uint32_t bvn_effective_q(value_type_spec_t s)
{
	return (s.family == vt_float_fix) ? s.base : 0u;
}
static inline bool bvn_type_is_numeric(value_type_spec_t s)
{
	return s.family == vt_sint      || s.family == vt_uint    ||
		   s.family == vt_float     ||
		   s.family == vt_float_fix || s.family == vt_float_dec;
}
static inline bool bvn_type_is_plain(value_type_spec_t s)
{
	return s.family == vt_plain;
}
BVN_API void bvnr_source_from_fd(bvnr_source_t* s, int fd);
BVN_API void bvnr_source_from_mem(bvnr_source_t* s, const void* buf, uint64_t len);
BVN_API void bvnr_sink_to_fd(bvnr_sink_t* s, int fd);
BVN_API void bvnr_sink_to_mem(bvnr_sink_t* s, void* buf, uint64_t cap);
/* Bytes pushed into a bvnr_sink_to_mem sink. Counts only what the CALLER pushed:
 * bvnr_open_write_sink() takes a copy of the sink, so writer output advances the
 * copy and this stays 0 for the writer's whole lifetime. For writer output use
 * bvnr_writer_bytes_written(), and read it before bvnr_writer_destroy(). */
BVN_API uint64_t bvnr_sink_bytes_written(const bvnr_sink_t* s);
/* THREADING. A single reader or writer object holds the whole parse/serialise
 * state and must not be used from two threads at once. DISTINCT objects on
 * distinct threads need no synchronisation: the library writes no shared state
 * after startup (the lexer's run tables are built from a pre-main constructor
 * for exactly this reason). See tests/bovnar_concurrency_test.c. */
BVN_API bvnr_reader_t* bvnr_reader_create(void);
BVN_API void           bvnr_reader_destroy(bvnr_reader_t* r);
BVN_API bool bvnr_open_read_source(
	bvnr_reader_t* r, const bvnr_source_t* src,
	const bvnr_sink_t* src_mirror, bvnr_read_flags_t* options);
BVN_API bool bvnr_open_read_mem(
	bvnr_reader_t* r, const void* buf, uint64_t len,
	void* mirror_buf, uint64_t mirror_cap,
	bvnr_read_flags_t* options);
/* Run the parse. Requires a reader that one of the bvnr_open_read_* calls above
 * has attached a source to; a NULL handle, or one that was created but never
 * opened, returns false (the latter setting error_invalid_argument) rather than
 * faulting. Returns false on the first error unless continue_on_error is set. */
BVN_API bool bvnr_read(bvnr_reader_t* r);
typedef struct bvnr_canon_observer_s bvnr_canon_observer_t;
BVN_API bvnr_canon_observer_t* bvnr_canon_observer_create(
	const bvnr_sink_t* sink, bool pretty);
/* Prepend a "#!bovnar <major>.<minor>" version directive to the canonical
 * output (emitted lazily before the first event). Call it before any event is
 * fed — e.g. from a reader's on_verified callback once
 * bvnr_reader_get_declared_version() resolves — so canonicalising a spec-1.1
 * document keeps the directive its constructs require to re-read. A version of
 * 0.0, or a call after output has begun, is ignored. */
BVN_API void bvnr_canon_observer_set_version(
	bvnr_canon_observer_t* obs, uint16_t major, uint16_t minor);
BVN_API void bvnr_canon_observer_destroy(bvnr_canon_observer_t* obs);
BVN_API bool bvnr_canon_observer_on_event(
	void* obs, bvnr_event_t ev, bvnr_data_t* data);
BVN_API bool bvnr_canon_observer_finish(bvnr_canon_observer_t* obs);
/* Where the error is. The three position counters measure DIFFERENT things, and
 * a tool placing a caret needs the right one:
 *
 *   line   1-based line number.
 *   column 1-based CHARACTER column, not a byte column -- a multi-byte UTF-8
 *          sequence advances it by one. In `.x = "çafé"; .y = @;` the '@' is
 *          at column 19 and offset 20: two multi-byte characters before it,
 *          a count from 1 against a count from 0. A tab advances to the next
 *          multiple of 4.
 *          This matters here more than in most formats: unit symbols are
 *          routinely non-ASCII (µ~m, °C, Ω), so the two disagree often.
 *   offset  0-based BYTE offset into the stream -- use this one to index the
 *          input buffer.
 *
 * byte is the offending input byte itself, or 0 where the error is not
 * attributable to a single byte.
 *
 * These describe the error that ENDED the read: meaningful after bvnr_read()
 * returned false, and inside on_error, where they match its arguments. An
 * error recovered under continue_on_error is reported through on_error only --
 * after a read that returned true they give error_none and 0, however many
 * errors were recovered; see bvnr_reader_get_recovery_count (doc/03 §13.4). */
BVN_API error_code_t bvnr_reader_get_error(const bvnr_reader_t* r);
BVN_API uint64_t     bvnr_reader_get_error_line  (const bvnr_reader_t* r);
BVN_API uint64_t     bvnr_reader_get_error_column(const bvnr_reader_t* r);
BVN_API uint32_t     bvnr_reader_get_error_byte  (const bvnr_reader_t* r);
BVN_API uint64_t     bvnr_reader_get_error_offset(const bvnr_reader_t* r);
BVN_API uint64_t     bvnr_reader_get_recovery_count(const bvnr_reader_t* r);
/* Bytes the reader consumed and DISCARDED while recovering, summed over the
 * document. Like bvnr_reader_get_recovery_count, this is meaningful even when
 * bvnr_read() returned true — and it is the one that says what recovery cost.
 *
 * recovery_count says how often the parser had to recover; on its own it cannot
 * distinguish a single skipped assignment from a whole discarded struct, and a
 * consumer reading a document under continue_on_error has no other way to learn
 * that values went missing: the skipped bytes were never parsed, so no callback
 * ever mentions them. A non-zero value here means the document handed to the
 * callbacks is not the whole document, and this is how much of it is absent.
 *
 * Counted in bytes of TEXT (an octet-stream payload is not scanned as text, so
 * it does not appear here), from the byte the error was raised on up to, not
 * including, the boundary recovery resumed at -- the ';' or the '.' that begins
 * the next key. A value rejected at its own ';' therefore counts 0. Recovery
 * that runs to end-of-input counts everything to the end. */
BVN_API uint64_t     bvnr_reader_get_skipped_bytes(const bvnr_reader_t* r);
/*
 * Spec version a document declared via a leading "#!bovnar <major>.<minor>"
 * directive. Returns true and fills major/minor when the document carried a
 * (well-formed) directive; returns false (leaving the outputs untouched) when
 * it did not. Valid only after bvnr_read(). Either out pointer may be NULL.
 */
BVN_API bool bvnr_reader_get_declared_version(
	const bvnr_reader_t* r, uint16_t* major, uint16_t* minor);
/* ── Reader-side unit policy ────────────────────────────────────────────────
 *
 * What bvnr_read_flags_t.want_unit does through a callback, stated as data: what
 * the document MUST contain, and what unit the consumer wants values delivered
 * in. Set with bvnr_reader_set_unit_policy; everything is expressed as unit
 * TEXT, so a binding can drive it without a function pointer.
 *
 * Nothing here weakens the exactness contract. The policy only chooses targets;
 * the conversion itself is the same exact arbitrary-precision path want_unit
 * uses, and a value that cannot be delivered exactly is still never rounded.
 */
#define BVNR_MAX_UNIT_TARGETS 8u
/* Per-field rules: how many, and the longest key path one may name (NUL
 * excluded). A path longer than this is rejected when the policy is set. */
#define BVNR_MAX_UNIT_RULES   8u
#define BVNR_MAX_UNIT_PATH    96u
/* One conversion target: the unit to convert to, and the base to render the
 * result in (0 keeps the value's own base; anything else must be a base bvnr can
 * write -- 2..62, 64, 85). */
typedef struct bvnr_unit_target_s {
	const char*	unit;
	uint32_t	base;
} bvnr_unit_target_t;
typedef enum bvnr_unit_rule_mode_e {
	/* The value must be validly convertible to the rule's unit, and is
	 * delivered converted to it. */
	bvnr_rule_convert = 0,
	/* The value must be validly convertible to the rule's unit, and is
	 * delivered exactly as the document wrote it. "This field is a length,
	 * in whatever length unit it chose." */
	bvnr_rule_require = 1
} bvnr_unit_rule_mode_t;
/* A rule about ONE field, named by its key path.
 *
 * `path` is the dotted path a value sits at, leading dot included, exactly as
 * the document writes its keys: ".inlet.temperature". A path ending in ".*"
 * matches every value BELOW that point at any depth (".inlet.*" matches
 * ".inlet.temperature" and ".inlet.pump.rpm", but not ".inlet" itself). Array
 * elements sit at the path of the assignment that holds them, so one rule covers
 * every element of an array.
 *
 * Unlike the whole-document `targets` list, a rule is a statement about a field
 * the caller has NAMED, so a value it cannot be applied to is
 * error_unit_mismatch rather than a value left alone: silence would defeat the
 * point of naming it. That includes a value with no unit at all -- ".speed is
 * m/s" is not satisfied by a bare number.
 *
 * It also includes a value the reader cannot LOCATE. A key path is recorded to a
 * bounded depth and length; a document that outruns either leaves the position
 * of everything below it unknown, and a rule that cannot be evaluated is refused
 * with error_unit_mismatch rather than skipped -- "I could not check it" is not
 * "it holds". `targets`, `normalise`, `require_unit` and `require_dimension_of`
 * ask about the value rather than about where it sits and are unaffected, so a
 * document of any depth reads normally under those. See doc/06 section 2.7.
 *
 * A rule that MATCHES NOTHING is satisfied, and this is the opposite of the case
 * above -- read the two together. The assertion is about values the rule reaches,
 * so a path no value sits at makes no claim and the document passes. The
 * distinction is between a rule that found nothing to check (fine) and one that
 * could not check what it found (an error).
 *
 * The consequence runs against what the name suggests: a rule does NOT require
 * its field to exist. A typo in `path`, or a rename in the document the policy
 * was not updated for, turns that rule off silently and the document still
 * validates. Nothing detects it here; a caller whose policy is load-bearing
 * should assert that the paths it names are the ones the document uses. */
typedef struct bvnr_unit_rule_s {
	const char*		path;
	const char*		unit;
	uint32_t		base;   /* output base for bvnr_rule_convert, 0 = own */
	bvnr_unit_rule_mode_t	mode;
} bvnr_unit_rule_t;
typedef enum bvnr_unit_normalise_e {
	/* Leave whatever the targets did not match in its native unit. */
	bvnr_normalise_none = 0,
	/* Deliver anything the targets did not match in coherent SI base units,
	 * prefixes folded out (see bvn_unit_si_normal_form). Values with no
	 * coherent SI form -- currencies, and every DIMENSIONLESS unit (%, ppm,
	 * dB, pH, rad, °) -- are left alone rather than reinterpreted. */
	bvnr_normalise_si   = 1
} bvnr_unit_normalise_t;
typedef enum bvnr_unit_inexact_policy_e {
	/* Abort the parse with error_unit_inexact -- the want_unit default, and
	 * the right one when the caller named a target deliberately. */
	bvnr_inexact_error  = 0,
	/* Deliver the value untouched (converted == false) instead of aborting.
	 * Meant for bvnr_normalise_si, where EVERY value is a conversion candidate
	 * and one 5/18 factor would otherwise reject an ordinary document. The
	 * consumer must read `converted` to know which values this happened to.
	 *
	 * Applies to EVERY result the conversion cannot deliver exactly, including
	 * a genuinely irrational factor (a π-based angle, a parsec, a water
	 * hardness scale). The tempting distinction -- that an irrational factor is
	 * special because there is no rational to hand over -- belongs to
	 * want_unit_allow_nonterminating, whose fallback IS the rational. This
	 * flag's fallback is the NATIVE VALUE, and that works for an irrational
	 * factor exactly as well as for a non-terminating one. It has to: a
	 * document carrying a heading in degrees is entirely ordinary, and under
	 * bvnr_normalise_si the strict reading would reject it, which is the case
	 * this mode exists for.
	 *
	 * Only a POLICY-chosen target takes this path. A target named by the
	 * want_unit hook keeps the strict all-or-nothing contract that hook was
	 * documented with, whatever this field says. */
	bvnr_inexact_leave  = 1
} bvnr_unit_inexact_policy_t;
typedef struct bvnr_unit_policy_s {
	/* PER-FIELD RULES, matched by key path and consulted before everything
	 * below -- the most specific thing a policy can say. First match wins, so
	 * order matters: put ".a.b" before ".a.*". At most BVNR_MAX_UNIT_RULES
	 * entries, each path at most BVNR_MAX_UNIT_PATH characters.
	 *
	 * A rule is an ASSERTION as well as a target: a value at a path a rule
	 * names must be validly convertible to that rule's unit, or the parse
	 * fails with error_unit_mismatch. */
	const bvnr_unit_rule_t*		rules;
	uint32_t			num_rules;
	/* CONVERSION. Each numeric value is converted to the first target it can
	 * validly convert to (bvn_units_convertible), so order is significant: a
	 * list of {"m", "km"} never selects "km". A value that matches nothing is
	 * delivered untouched unless `normalise` catches it. At most
	 * BVNR_MAX_UNIT_TARGETS entries.
	 *
	 * A value ALREADY in the unit a target names is left untouched too (unless
	 * that target asks for an output base, which makes it a pure base
	 * conversion): `converted` then means "the policy restated this value",
	 * not "the policy looked at it".
	 *
	 * The unitless fence runs BOTH WAYS: a value with no unit only ever
	 * matches a target that is itself `no_unit`, AND a `no_unit` target only
	 * ever matches a value that has no unit. A bare number is dimensionally
	 * compatible with % and with ppm, so without the fence a policy naming "%"
	 * would deliver `0.25` as `25`, and one naming `no_unit` would deliver
	 * `35 %` as `0.35` -- the same silent rescale from either side, arrived at
	 * through the machinery meant to prevent it.
	 *
	 * The second half is the one that surprises: `no_unit` as a target does
	 * NOT strip units off dimensionless values. There is no policy setting
	 * that turns "35 %" into a bare number, by design. What the fence does not
	 * touch is a conversion between two units -- `ppm` to `%` is real and
	 * wanted -- so this is specifically about values carrying no unit at all. */
	const bvnr_unit_target_t*	targets;
	uint32_t			num_targets;
	uint32_t			base;      /* output base for `normalise`,
						    * 0 = the value's own */
	bvnr_unit_normalise_t		normalise;
	bvnr_unit_inexact_policy_t	on_inexact;
	/* VALIDATION. These reject a document; they never change a value. They are
	 * evaluated on the unit the DOCUMENT wrote, before any conversion --
	 * validate what you were sent, convert for the consumer -- so they mean the
	 * same thing whether or not a conversion was also requested. A failure is
	 * error_unit_mismatch.
	 *
	 * require_unit: every numeric value must carry a unit. A bare number fails,
	 * whether it was written without a unit parameter or with an explicit
	 * `no_unit`. */
	bool				require_unit;
	/* Every numeric value must be validly convertible to at least one of these
	 * units -- "this document is lengths and temperatures, in whatever unit it
	 * chose to write them". A value with NO unit satisfies this only if one of
	 * the listed units is itself `no_unit`: a bare number is dimensionally
	 * compatible with % and ppm, and quietly accepting it there is the silent
	 * factor-of-a-hundred this whole mechanism exists to prevent. At most
	 * BVNR_MAX_UNIT_TARGETS entries. */
	const char* const*		require_dimension_of;
	uint32_t			num_require_dimension_of;
} bvnr_unit_policy_t;
/*
 * Install (or, with policy == NULL, clear) the reader's unit policy. Every unit
 * string is parsed HERE, so a malformed unit, an out-of-range count or an
 * unusable base is reported by a false return before the parse begins rather
 * than as a mid-document error; the previous policy is left untouched when that
 * happens. The strings themselves need not outlive the call.
 *
 * Callable before or after bvnr_open_read_*, and it survives re-opening the
 * reader on another document -- a policy is a property of the consumer, not of
 * the document it was first set for.
 *
 * Where this sits relative to bvnr_read_flags_t.want_unit: the hook is more
 * specific and wins. For each value the reader asks the hook first, falls back
 * to `targets`, and falls back again to `normalise`. Setting both is a
 * legitimate combination -- normalise the document, hand-handle the one field
 * that needs something else.
 */
BVN_API bool bvnr_reader_set_unit_policy(
	bvnr_reader_t* r, const bvnr_unit_policy_t* policy);
/*
 * The same contract on the producing side: refuse to EMIT a value a reader
 * under this policy would refuse to accept. bvnr_write_event fails with
 * error_unit_mismatch, and the writer latches it like any other write error
 * (query it with bvnr_writer_get_error).
 *
 * This is the half the format's promise rests on. A reader can only reject a
 * document somebody already wrote; only the writer can stop a bare number
 * reaching a file in the first place, which is what "hand the file to anyone and
 * they have everything required to interpret it" actually depends on.
 *
 * VALIDATION ONLY. `require_unit`, `require_dimension_of` and per-field rules
 * in bvnr_rule_require mode are accepted; a policy carrying `targets`,
 * `normalise`, `base`, `on_inexact` or a bvnr_rule_convert rule is rejected
 * (false) rather than half-honoured. The writer already has a value-rewriting
 * mode — BVN_UNIT_REDUCE in bvnr_write_flags_t.unit_flags, which folds prefixes
 * out and rescales the value exactly — and a second one arriving through a
 * different door with different rules about exactness is how two features end up
 * disagreeing about what a document says.
 *
 * The unit checked is the one the value will carry in the output, whether it was
 * given inline on the value or as a parameter of its type annotation. Like the
 * reader's, this policy may be set before or after bvnr_open_write_*, survives
 * re-opening the writer on another document, and is cleared by passing NULL.
 */
BVN_API bool bvnr_writer_set_unit_policy(
	bvnr_writer_t* w, const bvnr_unit_policy_t* policy);
/*
 * Scan a buffer for a leading "#!bovnar <major>.<minor>" directive without a
 * full parse (an optional UTF-8 BOM and leading whitespace are skipped).
 * Returns true and fills major/minor on a well-formed directive, false
 * otherwise. Either out pointer may be NULL.
 */
BVN_API bool bvnr_peek_version(
	const void* buf, uint64_t len, uint16_t* major, uint16_t* minor);
/* Runtime library version (BVNR_VERSION / BVNR_VERSION_STRING). */
BVN_API uint32_t    bvnr_version(void);
BVN_API const char* bvnr_version_string(void);
/* Highest bovnar spec version this build understands (BVNR_SPEC_VERSION_*). */
BVN_API void        bvnr_spec_version(uint16_t* major, uint16_t* minor);
BVN_API bvnr_writer_t* bvnr_writer_create(void);
BVN_API void           bvnr_writer_destroy(bvnr_writer_t* w);
BVN_API bool bvnr_open_write_sink(
	bvnr_writer_t* w, const bvnr_sink_t* sink,
	bool pretty, bvnr_write_flags_t* options);
BVN_API bool bvnr_open_write_mem(
	bvnr_writer_t* w, void* buf, uint64_t cap,
	bool pretty, bvnr_write_flags_t* options);
BVN_API bool bvnr_write_event(
	bvnr_writer_t* w, bvnr_event_t ev, bvnr_data_t* data);
/*
 * Emit a leading "#!bovnar <major>.<minor>" version directive. Must be called
 * immediately after bvnr_open_write_* and before any value is written; doing so
 * later (once output has begun) is error_invalid_argument. Use it to round-trip
 * a directive read from a source document, or pass BVNR_SPEC_VERSION_MAJOR /
 * BVNR_SPEC_VERSION_MINOR to stamp the current spec version.
 */
BVN_API bool bvnr_write_version(
	bvnr_writer_t* w, uint16_t major, uint16_t minor);
BVN_API bool bvnr_write_finish(bvnr_writer_t* w);
BVN_API error_code_t bvnr_writer_get_error(const bvnr_writer_t* w);
BVN_API uint64_t     bvnr_writer_get_error_offset(const bvnr_writer_t* w);
BVN_API uint64_t     bvnr_writer_bytes_written(const bvnr_writer_t* w);
BVN_API const char*  bvn_error_to_string(error_code_t code);
/*
 * Parse a unit string. `ok` is OPTIONAL — a caller that does not want it can
 * read the result's num_components, since a refused parse returns
 * BVN_UNIT_NONE. Every out-parameter of every unit function is optional the same
 * way; see the note at the top of bovnar_si_units.h.
 *
 * bvn_unit_error_code says WHY a string was refused, which `ok` cannot.
 */
BVN_API value_unit_t bvn_parse_unit(const uint8_t* unit, bool* ok);
BVN_API value_unit_t bvn_parse_unit_n(const uint8_t* unit, uint32_t len, bool* ok);
/*
 * Write `u` as text, NUL-terminated. Returns the length written, or -1.
 *
 * A short buffer is a REFUSAL, not a truncation: on -1 nothing is written at
 * all, not even the NUL, because a truncated unit is a different unit.
 * BVNR_UNIT_STRING_MAX is large enough for anything these functions can emit.
 *
 * -1 also covers a valid unit with no TEXT. That is not a contradiction — see
 * bvn_unit_valid below — and there are three ways to reach it, all of them
 * involving an opaque unit (one of a profile's units with no native spelling;
 * bvn_unit_is_profile_only reports them):
 *   - opaque bases from two different profiles in one unit: no namespace owns
 *     it, and writing it in whichever came first would re-parse as something
 *     else;
 *   - an opaque unit from a FLAT profile (unece, qudt, qudt-qk, om, cf) at
 *     any exponent but 1, or beside another component: a flat vocabulary is
 *     one whole code looked up entire, so it can spell exactly one unprefixed
 *     component;
 *   - an opaque unit beside a native one, for the same reason.
 * Such a unit is still perfectly usable in memory — it compares, and it
 * converts to itself — it simply cannot be serialised, and the writer refuses
 * it (error_unit_illegal) rather than emitting something that reads back wrong.
 */
BVN_API int32_t      bvn_unit_to_string(value_unit_t u, char* buf, size_t bufsize);
BVN_API int32_t      bvn_unit_to_string_ex(value_unit_t u, char* buf, size_t bufsize,
                                    bvn_unit_flags_t flags);
/*
 * STRUCTURAL validity: no more than BVNR_MAX_UNIT_COMPONENTS components, every
 * exponent inside [BVN_EXPONENT_MIN, BVN_EXPONENT_MAX] and non-zero, every base
 * one this build defines, and every prefix one its base accepts
 * (bvn_prefix_unit_valid). It is the precondition the conversion and formatting
 * entry points share, and what a caller that filled in a value_unit_t by hand
 * should check.
 *
 * It is NOT "this unit can be written": see bvn_unit_to_string above for the
 * units that have no text. Nor is it "this unit converts" — a currency and an
 * opaque unit are both valid and neither has an SI factor. Those are separate
 * questions with separate functions, deliberately: a unit with no spelling is
 * still a unit, and refusing to compare or convert it would be wrong.
 */
BVN_API bool         bvn_unit_valid(value_unit_t u);
/* The unit profiles (spec 1.2, doc/11_bovnar_unit_profiles.md).
 *
 * bvn_unit_error_code says WHY a unit string bvn_parse_unit rejected is not a
 * unit: error_unit_illegal for malformed input, error_unit_profile_unknown for
 * an unrecognised "name:" namespace, error_unit_profile_unsupported for a code
 * that is valid in its vocabulary but which this build cannot represent. It
 * re-parses, so it is for the error path only. Passing a string that DOES parse
 * returns error_none.
 *
 * bvn_unit_is_profile_only is true when a unit has no native spelling -- it
 * contains an OPAQUE base unit, such as a UCUM arbitrary atom or a UNECE package
 * code -- so bvn_unit_to_string emits it in the profile notation of whichever
 * namespace owns it.
 *
 * bvn_unit_to_profile writes a unit as a code in the named vocabulary (without
 * the "<ns>:" prefix); bvn_unit_to_ucum is the "ucum" case, kept for callers
 * that predate the others. Both are partial by construction and return -1 for a
 * native unit outside that vocabulary's transliteration table -- which includes
 * the Old German units, the water-hardness degrees, the turbidity kinds and
 * every currency -- for an opaque unit belonging to a DIFFERENT profile, and,
 * in a flat vocabulary such as UNECE, for any compound or exponentiated unit,
 * since a flat code is one whole token with nothing to build it out of.
 *
 * ONE NAMESPACE ANSWERS FOR NOTHING AT ALL. "cf" is read-only: a CF standard
 * name states a QUANTITY, dozens of them state the same unit, and choosing one
 * would make this function assert what a quantity is from a unit that does not
 * know it. It reads like any other namespace and returns -1 here for every
 * unit, by having no reverse table rather than by a special case. */
BVN_API error_code_t bvn_unit_error_code(const uint8_t* unit, uint32_t len);
BVN_API bool         bvn_unit_is_profile_only(value_unit_t u);
BVN_API int32_t      bvn_unit_to_profile(const char* ns, value_unit_t u,
                                         char* buf, size_t bufsize);
BVN_API int32_t      bvn_unit_to_ucum(value_unit_t u, char* buf, size_t bufsize);
/*
 * WHICH PROFILE NAMESPACES THIS BUILD CARRIES.
 *
 * The profiles are compiled per vocabulary (BVNR_WITH_UCUM_PROFILE and its six
 * siblings, all on by default). Turning one off drops that vocabulary's tables
 * -- the seven together are about half of the library's data -- and nothing
 * else: the base-unit ids, the dense unit tables, every struct layout and every
 * signature here are the same in all configurations, so builds with different
 * switches stay ABI-compatible and differ only in which namespaces they
 * translate.
 *
 * A namespace that is not compiled in is error_unit_profile_unknown, the same
 * answer as a namespace that never existed, and it is unwritable as well as
 * unreadable: bvn_unit_to_profile and bvn_unit_to_ucum return -1 for it.
 *
 * This pair is how a consumer finds out WITHOUT parsing a probe document.
 * bvn_unit_profile_count returns how many namespaces are present;
 * bvn_unit_profile_name returns the i-th name (lowercase, as it is spelled
 * before the ':'), or NULL when i is out of range. The order is the library's
 * and is not a contract; the set is. Both are always available -- a build with
 * every profile off reports a count of zero rather than dropping the symbols,
 * so a caller does not need a second way to ask.
 */
BVN_API uint32_t     bvn_unit_profile_count(void);
BVN_API const char*  bvn_unit_profile_name(uint32_t index);
BVN_API bool         bvn_unit_equal(value_unit_t a, value_unit_t b);
BVN_API double       bvn_unit_prefix_factor(value_unit_t u);
BVN_API int32_t      bvn_unit_prefix_exponent(value_unit_t u);
BVN_API bvn_unit_flags_t bvnr_writer_unit_flags(const bvnr_writer_t* w);
/*
 * Datetime epoch helpers (spec 1.1). For a vt_datetime spec, return the epoch's
 * lowercase name ("unix", "tai", "gps", "mjd", "ntp", "galileo", "glonass",
 * "y2000", "beidou") and its epoch day number (the bvn_epoch_t value from
 * bvn_datetime.h, i.e. the MJD of the epoch's instant). For a non-datetime spec
 * the name is "unix" and the day number is the unix epoch, by convention.
 */
BVN_API const char* bvnr_datetime_epoch_name(value_type_spec_t vt);
BVN_API int32_t     bvnr_datetime_epoch_mjd(value_type_spec_t vt);
/* Dense epoch index for an epoch name ("unix", "tai", …) — the value stored in
 * value_type_spec_t.base for a vt_datetime spec. NULL/empty → 0 (unix); an
 * unknown name → -1. Inverse of bvnr_datetime_epoch_name(). */
BVN_API int32_t     bvnr_datetime_epoch_index(const char* name);
/*
 * Parse the body of a type annotation — the text BETWEEN the angle brackets,
 * e.g. "uint:32,_16,k~g" — into a type spec plus its unit.
 *
 * unit_buf receives the unit parameter's raw text, NUL-terminated, and MUST be at
 * least UINT8_MAX + 1 (256) bytes: a unit parameter may be up to 255 bytes, and
 * there is no length argument to bound the write. *unit_buf_len returns its
 * length excluding the NUL (0 when the annotation carries no unit parameter).
 *
 * The three flags separate the failure kinds a caller has to tell apart:
 * type_ok false means the annotation itself is unparseable (fatal); unit_ok false
 * with unit_too_long true means the unit parameter exceeded 255 bytes; unit_ok
 * false alone means it was present but not a unit this build knows.
 */
BVN_API value_type_spec_t bvn_parse_type_annotation(
	const uint8_t* str, uint32_t len,
	bool* type_ok, bool* unit_ok, bool* unit_too_long,
	value_unit_t* out_unit,
	uint8_t* unit_buf, uint8_t* unit_buf_len);
BVN_API bool bvn_validate_identifier(const char* id);
BVN_API bool bvn_validate_symbol(const char* surr);
BVN_API bool bvn_validate_reference(const char* link);
BVN_API bool bvn_validate_number(const char* s);
BVN_API bool bvn_is_special_number_string(const char* s);
BVN_API bool bvn_validate_digits_for_base(const char* s, uint32_t base);
BVN_API bool bvn_validate_number_in_base(const char* s, uint32_t base);
BVN_API bool bvn_validate_uint_range(const char* s, uint32_t w, uint32_t base);
BVN_API bool bvn_validate_sint_range(const char* s, uint32_t w, uint32_t base);
BVN_API bool bvn_validate_string(const uint8_t* data, size_t len);
BVN_API uint32_t bvn_char_to_digit(uint32_t c, uint32_t base);
BVN_API uint32_t bvn_min_digits_for_type(value_type_spec_t vt);
BVN_API int32_t bvn_format_uint64(
	char* buf, size_t bufsize, uint64_t value,
	uint32_t base, uint32_t min_digits);
BVN_API int32_t bvn_format_int64(
	char* buf, size_t bufsize, int64_t value,
	uint32_t base, uint32_t min_digits);
BVN_API int32_t bvn_format_double(
	char* buf, size_t bufsize, double value, value_type_spec_t vt);
BVN_API bool bvn_parse_int64(const char* s, value_type_spec_t vt, int64_t* out);
BVN_API bool bvn_parse_uint64(const char* s, value_type_spec_t vt, uint64_t* out);
BVN_API bool bvn_parse_double(const char* s, value_type_spec_t vt, double* out);
BVN_API bool bvn_parse_double_in_base(const char* s, uint32_t base, double* out);
BVN_API bool bvn_looks_like_double(const char* s);
BVN_API const uint8_t* bvn_get_escape_repl_table(void);
/*
 * Spell the unit of the NEXT value written through the helpers below.
 *
 * Every bvnr_write_*_unit helper takes a value_unit_t, and a value_unit_t has
 * nowhere to hold a spelling, so a producer holding "ucum:mL{total}" could only
 * write the canonical "m~L". This hands the spelling over separately: the next
 * bvnr_write_type_annotation -- which every one of those helpers calls --
 * emits `text` for its unit and then forgets it. One shot, and for the NEXT
 * value only: if that value takes no unit (a string, a bool, a struct, a bare
 * number) the write that follows it fails with error_unit_mismatch, rather than
 * the spelling travelling on to whichever later value happens to carry a unit.
 * (If it was the last value, bvnr_write_finish reports it.) It is also dropped
 * when a write fails, when the writer is re-opened, and by passing NULL or a
 * zero length.
 *
 * The text is copied, and it is held to bvnr_data_t.unit_source's contract (see
 * there): it must parse to the very unit the helper is then given. That check
 * happens at the write, not here, because only the write knows the unit; this
 * call fails (false, error_unit_too_long latched) only for a text longer than
 * BVNR_UNIT_SOURCE_MAX. A caller driving bvnr_write_event directly does not
 * need this -- set bvnr_data_t.unit_source on the events instead.
 */
BVN_API bool bvnr_writer_set_unit_source(bvnr_writer_t* w,
				const char* text, uint32_t length);
BVN_API bool bvnr_write_type_annotation(bvnr_writer_t* w,
				value_type_spec_t vt,
				value_unit_t vu);
BVN_API bool bvnr_write_string(bvnr_writer_t* w, const char* key, const char* value);
BVN_API bool bvnr_write_plain (bvnr_writer_t* w, const char* key, const char* value);
BVN_API bool bvnr_write_null  (bvnr_writer_t* w, const char* key);
BVN_API bool bvnr_write_bool  (bvnr_writer_t* w, const char* key, bool value);
BVN_API bool bvnr_write_uint(bvnr_writer_t* w, const char* key,
			 uint32_t width, uint64_t value);
BVN_API bool bvnr_write_sint(bvnr_writer_t* w, const char* key,
			 uint32_t width, int64_t value);
BVN_API bool bvnr_write_float(bvnr_writer_t* w, const char* key,
			  uint32_t width, double value);
/* Write a timestamp (spec 1.1). `epoch` is an epoch name ("unix", "tai", "gps",
 * "mjd", "ntp", "galileo", "glonass", "y2000", "beidou") or NULL for unix; an
 * unknown name is error_invalid_argument. `value` is signed seconds since the
 * epoch. Emits a <datetime:width,epoch> annotation (the epoch is implicit when
 * it is the default unix). A #!bovnar 1.1 directive must ALREADY have been emitted
 * (set bvnr_write_flags_t.emit_version, or call bvnr_write_version right after
 * open); a datetime written without it FAILS with error_unsupported_spec_version
 * before its annotation or value is written, since the directive cannot be
 * added retroactively. (The key is already in the sink by then, and the writer
 * is in its error state: the document is not usable.) That directive is also
 * what lets the document be re-read. */
BVN_API bool bvnr_write_datetime(bvnr_writer_t* w, const char* key,
			  uint32_t width, const char* epoch, int64_t value);
BVN_API bool bvnr_write_float_fix(bvnr_writer_t* w, const char* key,
			   uint32_t width, uint32_t q, double value);
BVN_API bool bvnr_write_float_dec(bvnr_writer_t* w, const char* key,
			   uint32_t width, double value);
BVN_API bool bvnr_write_uint_unit(bvnr_writer_t* w, const char* key,
			   uint32_t width, uint64_t value, value_unit_t unit);
BVN_API bool bvnr_write_sint_unit(bvnr_writer_t* w, const char* key,
			   uint32_t width, int64_t value, value_unit_t unit);
BVN_API bool bvnr_write_float_unit(bvnr_writer_t* w, const char* key,
				uint32_t width, double value, value_unit_t unit);
BVN_API bool bvnr_write_float_fix_unit(bvnr_writer_t* w, const char* key,
				uint32_t width, uint32_t q,
				double value, value_unit_t unit);
BVN_API bool bvnr_write_float_dec_unit(bvnr_writer_t* w, const char* key,
				uint32_t width,
				double value, value_unit_t unit);
BVN_API bool bvnr_write_bvnf(bvnr_writer_t* w, const char* key,
			 const bvn_float_t* f, uint32_t width);
BVN_API bool bvnr_write_bvnf_unit(bvnr_writer_t* w, const char* key,
			   const bvn_float_t* f,
			   uint32_t width, value_unit_t unit);
BVN_API bool bvnr_write_bvnf_base(bvnr_writer_t* w, const char* key,
			   const bvn_float_t* f,
			   uint32_t width, uint32_t base);
BVN_API bool bvnr_write_bvnf_base_unit(bvnr_writer_t* w, const char* key,
				const bvn_float_t* f,
				uint32_t width, uint32_t base,
				value_unit_t unit);
BVN_API bool bvnr_write_bvni(bvnr_writer_t* w, const char* key,
		     const bvn_int_t* n,
		     uint32_t width, uint32_t base);
BVN_API bool bvnr_write_bvni_unit(bvnr_writer_t* w, const char* key,
			   const bvn_int_t* n,
			   uint32_t width, uint32_t base,
			   value_unit_t unit);
BVN_API bool bvnr_write_struct_start(bvnr_writer_t* w, const char* key);
BVN_API bool bvnr_write_struct_end  (bvnr_writer_t* w);
#ifdef __cplusplus
}
#endif
#endif
