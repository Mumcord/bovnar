//! Hand-written declarations of the C API, mirroring `python/bovnar/_ffi.py`
//! (functions) and `python/bovnar/structs.py` (by-value struct layouts).
//!
//! This is the only module allowed to use `unsafe`. Besides the raw
//! declarations it holds the small safe handles the other modules build on, so
//! every pointer the library hands out is dereferenced here and nowhere else.

// The declaration list mirrors the whole of _ffi.py; the safe layer above it
// does not reach every function yet.
#![allow(dead_code)]

use std::ffi::{c_char, c_int, c_void};

/// Size of the opaque `bvnr_source_t` / `bvnr_sink_t` storage.
pub const OPAQUE_BYTES: usize = 256;
/// `BVN_MAX_UNIT_COMPONENTS`.
pub const MAX_UNIT_COMPONENTS: usize = 32;
/// `BVNR_UNIT_STRING_MAX`: the longest unit string the library writes.
pub const UNIT_STRING_MAX: usize = 1088;

/// `bvnr_on_error_fn`.
pub type OnErrorFn = extern "C" fn(*mut c_void, c_int, u64, u64, u32, u64);
/// `bvnr_event_callback`: `bool (*)(void*, bvnr_event_t, const bvnr_data_t*)`.
pub type EventCallbackFn = extern "C" fn(*mut c_void, c_int, *const BvnrData) -> bool;
/// `bvnr_read_flags_t.want_unit`.
pub type WantUnitFn = extern "C" fn(*mut c_void, *const BvnrData, *mut ValueUnit, *mut u32) -> bool;
/// `bvnr_doc_stream_opts_t.on_document`.
pub type OnDocumentFn = extern "C" fn(*mut c_void, u64, bool, c_int) -> bool;
/// `bvnr_mux_on_msg_fn`.
pub type MuxOnMsgFn = extern "C" fn(*mut c_void, u64, *const c_void, u64) -> bool;

#[repr(C)]
#[derive(Clone, Copy)]
pub union PrefixId {
    pub si: c_int,
    pub iec: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ValueUnitPrefix {
    pub system: c_int,
    pub id: PrefixId,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ValueUnitComponent {
    pub base: c_int,
    pub exponent: c_int,
    pub prefix: ValueUnitPrefix,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ValueUnit {
    pub num_components: u32,
    pub components: [ValueUnitComponent; MAX_UNIT_COMPONENTS],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ValueTypeSpec {
    pub family: c_int,
    pub width: u32,
    pub base: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BvnrConverted {
    pub unit: ValueUnit,
    pub text: *const c_char,
    pub length: u32,
    pub base: u32,
    pub num: *mut c_void,
    pub den: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BvnrData {
    pub type_: c_int,
    pub value_type: ValueTypeSpec,
    pub value_unit: ValueUnit,
    pub data: *const c_void,
    pub length: u32,
    pub frac_data: *const c_void,
    pub frac_length: u32,
    pub unit_source: *const c_void,
    pub unit_source_length: u32,
    pub converted: bool,
    pub conv: BvnrConverted,
}

#[repr(C)]
pub struct BvnrSource {
    pub opaque: [u64; OPAQUE_BYTES / 8],
}

#[repr(C)]
pub struct BvnrSink {
    pub opaque: [u64; OPAQUE_BYTES / 8],
}

#[repr(C)]
pub struct BvnrReadFlags {
    pub max_identifier_length: u16,
    pub max_string_length: u16,
    pub max_number_length: u16,
    pub max_symbol_length: u16,
    pub max_reference_length: u16,
    pub max_array_items: u64,
    pub max_text_bytes: u64,
    pub max_file_size: u64,
    pub max_struct_nesting: u8,
    pub max_array_nesting: u8,
    pub userdata: *mut c_void,
    pub on_unverified: Option<EventCallbackFn>,
    pub on_verified: Option<EventCallbackFn>,
    pub continue_on_error: bool,
    pub on_error: Option<OnErrorFn>,
    pub strict_version: bool,
    pub text_only: bool,
    pub want_unit_allow_nonterminating: bool,
    pub max_conversion_length: u32,
    pub want_unit: Option<WantUnitFn>,
    pub _reserved: [u64; 2],
}

#[repr(C)]
pub struct BvnrUnitTarget {
    pub unit: *const c_char,
    pub base: u32,
}

#[repr(C)]
pub struct BvnrUnitRule {
    pub path: *const c_char,
    pub unit: *const c_char,
    pub base: u32,
    pub mode: c_int,
}

#[repr(C)]
pub struct BvnrUnitPolicy {
    pub rules: *const BvnrUnitRule,
    pub num_rules: u32,
    pub targets: *const BvnrUnitTarget,
    pub num_targets: u32,
    pub base: u32,
    pub normalise: c_int,
    pub on_inexact: c_int,
    pub require_unit: bool,
    pub require_dimension_of: *const *const c_char,
    pub num_require_dimension_of: u32,
}

#[repr(C)]
pub struct BvnrWriteFlags {
    pub max_identifier_length: u16,
    pub max_string_length: u16,
    pub max_number_length: u16,
    pub max_symbol_length: u16,
    pub max_reference_length: u16,
    pub max_array_items: u64,
    pub max_text_bytes: u64,
    pub max_file_size: u64,
    pub max_struct_nesting: u8,
    pub max_array_nesting: u8,
    pub userdata: *mut c_void,
    pub on_event: Option<EventCallbackFn>,
    pub continue_on_error: bool,
    pub on_error: Option<OnErrorFn>,
    pub unit_flags: u32,
    pub emit_version: bool,
    pub _reserved: [u64; 4],
}

#[repr(C)]
pub struct BvnrDocStreamOpts {
    pub flags: *mut BvnrReadFlags,
    pub userdata: *mut c_void,
    pub on_document: Option<OnDocumentFn>,
    pub continue_past_failed: bool,
    pub max_document_size: u64,
}

#[repr(C)]
pub struct BvnDomEntry {
    pub key: *const c_char,
    pub value: *mut c_void,
}

/// Implements `zeroed()` for the plain-data structs above.
macro_rules! zeroed {
    ($($t:ty),*) => {$(
        impl $t {
            /// The all-zero value, which is what the C API expects as "unset"
            /// (`= {0}` in C): no data, no unit, defaults for every limit.
            pub fn zeroed() -> Self {
                // SAFETY: every field is an integer, bool, raw pointer, array
                // of those, or Option<fn>; all-zero is a valid value of each.
                unsafe { std::mem::zeroed() }
            }
        }
    )*};
}
zeroed!(ValueUnit, BvnrData, BvnrReadFlags, BvnrWriteFlags);

// Raw declarations, in the order of _ffi.py.
unsafe extern "C" {
    pub fn bvnr_reader_create() -> *mut c_void;
    pub fn bvnr_reader_destroy(r: *mut c_void);
    pub fn bvnr_canon_observer_create(sink: *mut BvnrSink, pretty: bool) -> *mut c_void;
    pub fn bvnr_canon_observer_set_version(obs: *mut c_void, major: u16, minor: u16);
    pub fn bvnr_canon_observer_destroy(obs: *mut c_void);
    pub fn bvnr_canon_observer_on_event(obs: *mut c_void, ev: c_int, data: *mut BvnrData) -> bool;
    pub fn bvnr_canon_observer_finish(obs: *mut c_void) -> bool;
    pub fn bvnr_source_from_fd(s: *mut BvnrSource, fd: c_int);
    pub fn bvnr_source_from_mem(s: *mut BvnrSource, buf: *mut c_void, len: u64);
    pub fn bvnr_open_read_source(
        r: *mut c_void,
        src: *mut BvnrSource,
        src_mirror: *mut BvnrSink,
        options: *mut BvnrReadFlags,
    ) -> bool;
    pub fn bvnr_open_read_mem(
        r: *mut c_void,
        buf: *mut c_void,
        len: u64,
        mirror_buf: *mut c_void,
        mirror_cap: u64,
        options: *mut BvnrReadFlags,
    ) -> bool;
    pub fn bvnr_read(r: *mut c_void) -> bool;
    pub fn bvnr_reader_get_error(r: *mut c_void) -> c_int;
    pub fn bvnr_reader_get_error_line(r: *mut c_void) -> u64;
    pub fn bvnr_reader_get_error_column(r: *mut c_void) -> u64;
    pub fn bvnr_reader_get_error_offset(r: *mut c_void) -> u64;
    pub fn bvnr_reader_get_error_byte(r: *mut c_void) -> u32;
    pub fn bvnr_reader_get_recovery_count(r: *mut c_void) -> u64;
    pub fn bvnr_reader_get_skipped_bytes(r: *mut c_void) -> u64;
    pub fn bvnr_reader_get_declared_version(
        r: *mut c_void,
        major: *mut u16,
        minor: *mut u16,
    ) -> bool;
    pub fn bvnr_peek_version(buf: *mut c_void, len: u64, major: *mut u16, minor: *mut u16) -> bool;
    pub fn bvnr_version() -> u32;
    pub fn bvnr_version_string() -> *const c_char;
    pub fn bvnr_spec_version(major: *mut u16, minor: *mut u16);
    pub fn bvnr_write_version(w: *mut c_void, major: u16, minor: u16) -> bool;
    pub fn bvnr_datetime_epoch_name(vt: ValueTypeSpec) -> *const c_char;
    pub fn bvnr_datetime_epoch_mjd(vt: ValueTypeSpec) -> i32;
    pub fn bvnr_writer_create() -> *mut c_void;
    pub fn bvnr_writer_destroy(w: *mut c_void);
    pub fn bvnr_sink_to_fd(s: *mut BvnrSink, fd: c_int);
    pub fn bvnr_sink_to_mem(s: *mut BvnrSink, buf: *mut c_void, cap: u64);
    pub fn bvnr_sink_bytes_written(s: *mut BvnrSink) -> u64;
    pub fn bvnr_open_write_sink(
        w: *mut c_void,
        sink: *mut BvnrSink,
        pretty: bool,
        options: *mut BvnrWriteFlags,
    ) -> bool;
    pub fn bvnr_open_write_mem(
        w: *mut c_void,
        buf: *mut c_void,
        cap: u64,
        pretty: bool,
        options: *mut BvnrWriteFlags,
    ) -> bool;
    pub fn bvnr_write_event(w: *mut c_void, ev: c_int, data: *mut BvnrData) -> bool;
    pub fn bvnr_write_finish(w: *mut c_void) -> bool;
    pub fn bvnr_writer_get_error(w: *mut c_void) -> c_int;
    pub fn bvnr_writer_get_error_offset(w: *mut c_void) -> u64;
    pub fn bvnr_writer_bytes_written(w: *mut c_void) -> u64;
    pub fn bvnr_writer_unit_flags(w: *mut c_void) -> u32;
    pub fn bvn_parse_uint64(s: *const c_char, vt: ValueTypeSpec, out: *mut u64) -> bool;
    pub fn bvn_parse_int64(s: *const c_char, vt: ValueTypeSpec, out: *mut i64) -> bool;
    pub fn bvn_parse_double(s: *const c_char, vt: ValueTypeSpec, out: *mut f64) -> bool;
    pub fn bvn_format_uint64(
        buf: *mut c_char,
        bufsize: usize,
        value: u64,
        base: u32,
        min_digits: u32,
    ) -> i32;
    pub fn bvn_format_int64(
        buf: *mut c_char,
        bufsize: usize,
        value: i64,
        base: u32,
        min_digits: u32,
    ) -> i32;
    pub fn bvn_format_double(
        buf: *mut c_char,
        bufsize: usize,
        value: f64,
        vt: ValueTypeSpec,
    ) -> i32;
    pub fn bvn_parse_unit(unit: *const u8, ok: *mut bool) -> ValueUnit;
    pub fn bvn_parse_unit_n(unit: *const u8, len: u32, ok: *mut bool) -> ValueUnit;
    pub fn bvn_unit_to_string(u: ValueUnit, buf: *mut c_char, bufsize: usize) -> i32;
    pub fn bvn_unit_to_string_ex(u: ValueUnit, buf: *mut c_char, bufsize: usize, flags: u32)
    -> i32;
    pub fn bvn_unit_valid(u: ValueUnit) -> bool;
    pub fn bvn_unit_error_code(unit: *const u8, len: u32) -> c_int;
    pub fn bvn_unit_is_profile_only(u: ValueUnit) -> bool;
    pub fn bvn_unit_profile_count() -> u32;
    pub fn bvn_unit_profile_name(index: u32) -> *const c_char;
    pub fn bvn_unit_to_profile(
        ns: *const c_char,
        u: ValueUnit,
        buf: *mut c_char,
        bufsize: usize,
    ) -> i32;
    pub fn bvn_unit_to_ucum(u: ValueUnit, buf: *mut c_char, bufsize: usize) -> i32;
    pub fn bvn_unit_prefix_factor(u: ValueUnit) -> f64;
    pub fn bvn_unit_prefix_exponent(u: ValueUnit) -> i32;
    pub fn bvn_units_compatible(a: ValueUnit, b: ValueUnit) -> bool;
    pub fn bvn_unit_equal(a: ValueUnit, b: ValueUnit) -> bool;
    pub fn bvn_units_convertible(a: ValueUnit, b: ValueUnit) -> bool;
    pub fn bvn_unit_si_normal_form(u: ValueUnit, out: *mut ValueUnit) -> bool;
    pub fn bvnr_reader_set_unit_policy(r: *mut c_void, policy: *mut BvnrUnitPolicy) -> bool;
    pub fn bvnr_writer_set_unit_policy(w: *mut c_void, policy: *mut BvnrUnitPolicy) -> bool;
    pub fn bvn_unit_convert_factor(
        a: ValueUnit,
        b: ValueUnit,
        ok: *mut bool,
        requires_affine: *mut bool,
    ) -> f64;
    pub fn bvn_unit_convert_value(
        value: f64,
        from: ValueUnit,
        to: ValueUnit,
        out: *mut f64,
    ) -> bool;
    pub fn bvn_unit_to_si_factor(
        u: ValueUnit,
        is_affine: *mut bool,
        affine_offset: *mut f64,
        ok: *mut bool,
    ) -> f64;
    pub fn bvn_rational_str_bufsize(num: *mut c_void, den: *mut c_void, base: u32) -> usize;
    pub fn bvn_rational_to_str(
        num: *mut c_void,
        den: *mut c_void,
        base: u32,
        buf: *mut c_char,
        bufsize: usize,
        exact: *mut bool,
    ) -> i32;
    pub fn bvn_int_bitlen(n: *mut c_void) -> c_int;
    pub fn bvn_int_str_bufsize(bits: u32, base: u32) -> usize;
    pub fn bvn_int_to_str(n: *mut c_void, buf: *mut c_char, bufsize: usize, base: u32) -> i32;
    pub fn bvn_unit_dimension_vector(u: ValueUnit, dims: *mut i32) -> bool;
    pub fn bvn_unit_reduce(u: ValueUnit, scale: *mut f64, overflow: *mut bool) -> ValueUnit;
    pub fn bvn_exponent_to_int(e: c_int) -> i32;
    pub fn bvn_int_to_exponent(n: i32) -> c_int;
    pub fn bvn_prefix_unit_valid(prefix: ValueUnitPrefix, base: c_int) -> bool;
    pub fn bvn_validate_identifier(id: *const c_char) -> bool;
    pub fn bvn_validate_symbol(surr: *const c_char) -> bool;
    pub fn bvn_validate_reference(link: *const c_char) -> bool;
    pub fn bvn_validate_uint_range(s: *const c_char, w: u32, base: u32) -> bool;
    pub fn bvn_validate_sint_range(s: *const c_char, w: u32, base: u32) -> bool;
    pub fn bvn_is_special_number_string(s: *const c_char) -> bool;
    pub fn bvn_error_to_string(code: c_int) -> *const c_char;
    pub fn bvn_dom_doc_create() -> *mut c_void;
    pub fn bvn_dom_doc_destroy(doc: *mut c_void);
    pub fn bvn_dom_parse(data: *mut c_void, len: u32) -> *mut c_void;
    pub fn bvn_dom_parse_policy(
        data: *mut c_void,
        len: u32,
        policy: *mut BvnrUnitPolicy,
    ) -> *mut c_void;
    pub fn bvn_dom_parse_fd_policy(
        fd: c_int,
        max_bytes: u64,
        policy: *mut BvnrUnitPolicy,
    ) -> *mut c_void;
    pub fn bvn_dom_parse_fd(fd: c_int) -> *mut c_void;
    pub fn bvn_dom_doc_get_parse_error(doc: *mut c_void) -> c_int;
    pub fn bvn_dom_lookup(doc: *mut c_void, path: *const c_char) -> *mut c_void;
    pub fn bvn_dom_struct_get(node: *mut c_void, key: *const c_char) -> *mut c_void;
    pub fn bvn_dom_array_at(node: *mut c_void, index: u32) -> *mut c_void;
    pub fn bvn_dom_node_type(node: *mut c_void) -> c_int;
    pub fn bvn_dom_is_null(node: *mut c_void) -> bool;
    pub fn bvn_dom_get_float(node: *mut c_void, out: *mut f64) -> bool;
    pub fn bvn_dom_get_bool(node: *mut c_void, out: *mut bool) -> bool;
    pub fn bvn_dom_get_string(node: *mut c_void, out: *mut *const c_void, len: *mut u32) -> bool;
    pub fn bvn_dom_get_symbol(node: *mut c_void, out: *mut *const c_void, len: *mut u32) -> bool;
    pub fn bvn_dom_get_reference(node: *mut c_void, out: *mut *const c_void, len: *mut u32)
    -> bool;
    pub fn bvn_dom_get_octets(node: *mut c_void, out: *mut *const c_void, len: *mut u32) -> bool;
    pub fn bvn_dom_get_value_type(node: *mut c_void) -> ValueTypeSpec;
    pub fn bvn_dom_get_datetime_fraction(node: *mut c_void, len_out: *mut u32) -> *const c_char;
    pub fn bvn_dom_get_unit_source(node: *mut c_void, len_out: *mut u32) -> *const c_char;
    pub fn bvn_dom_get_unit(node: *mut c_void) -> ValueUnit;
    pub fn bvn_dom_get_unit_string(node: *mut c_void, buf: *mut c_char, bufsize: usize) -> i32;
    pub fn bvn_dom_get_value_in_base_units(node: *mut c_void) -> f64;
    pub fn bvn_dom_struct_count(node: *mut c_void) -> u32;
    pub fn bvn_dom_array_count(node: *mut c_void) -> u32;
    pub fn bvn_dom_array_dims(node: *mut c_void) -> u32;
    pub fn bvn_dom_struct_entries(node: *mut c_void) -> *mut BvnDomEntry;
    pub fn bvn_dom_doc_entries(doc: *mut c_void) -> *mut BvnDomEntry;
    pub fn bvn_dom_doc_count(doc: *mut c_void) -> u32;
    pub fn bvn_dom_get_i64(node: *mut c_void, out: *mut i64) -> bool;
    pub fn bvn_dom_get_u64(node: *mut c_void, out: *mut u64) -> bool;
    pub fn bvn_dom_get_i32(node: *mut c_void, out: *mut i32) -> bool;
    pub fn bvn_dom_get_u32(node: *mut c_void, out: *mut u32) -> bool;
    pub fn bvn_dom_get_i16(node: *mut c_void, out: *mut i16) -> bool;
    pub fn bvn_dom_get_u16(node: *mut c_void, out: *mut u16) -> bool;
    pub fn bvn_dom_get_i8(node: *mut c_void, out: *mut i8) -> bool;
    pub fn bvn_dom_get_u8(node: *mut c_void, out: *mut u8) -> bool;
    pub fn bvn_dom_int_to_str(node: *mut c_void, base: u32) -> *mut c_void;
    pub fn bvn_dom_free_string(s: *mut c_void);
    pub fn bvnr_frame_write(sink: *mut BvnrSink, doc: *mut c_void, doc_len: u64) -> bool;
    pub fn bvnr_doc_stream_read(
        src: *mut BvnrSource,
        opts: *mut BvnrDocStreamOpts,
        out_count: *mut u64,
    ) -> bool;
    pub fn bvnr_mux_begin(w: *mut c_void, key: *const c_char) -> bool;
    pub fn bvnr_mux_send(w: *mut c_void, channel: u64, data: *mut c_void, len: u64) -> bool;
    pub fn bvnr_mux_end(w: *mut c_void) -> bool;
    pub fn bvnr_demux_create(
        on_message: MuxOnMsgFn,
        userdata: *mut c_void,
        max_message: u64,
    ) -> *mut c_void;
    pub fn bvnr_demux_destroy(dm: *mut c_void);
    pub fn bvnr_demux_set_key(dm: *mut c_void, key: *const c_char) -> bool;
    pub fn bvnr_demux_on_event(demux: *mut c_void, ev: c_int, d: *mut BvnrData) -> bool;
    pub fn bvnr_demux_error(dm: *mut c_void) -> c_int;
    pub fn bvnr_embed_document(
        w: *mut c_void,
        key: *const c_char,
        doc: *mut c_void,
        doc_len: u64,
    ) -> bool;
    pub fn bvnr_parse_embedded(doc: *mut c_void, doc_len: u64, flags: *mut BvnrReadFlags) -> bool;
    pub fn bvn_float_alloc(prec: u32) -> *mut c_void;
    pub fn bvn_float_free(f: *mut c_void);
    pub fn bvn_float_from_str(f: *mut c_void, s: *const c_char, base: u32) -> bool;
    pub fn bvn_float_to_str(f: *mut c_void, buf: *mut c_char, bufsize: usize, base: u32) -> i32;
    pub fn bvn_float_str_bufsize(prec: u32, base: u32) -> usize;
    pub fn bvn_float_to_double(f: *mut c_void, out: *mut f64) -> bool;
    pub fn bvn_float_strtoieee_bin(
        s: *const c_char,
        base: u32,
        exp_bits: u32,
        man_bits: u32,
        bias: i32,
        bits: *mut u32,
        bits32: c_int,
    );
    pub fn bvn_float_to_bin16(f: *mut c_void, out: *mut u16);
    pub fn bvn_float_to_bin32(f: *mut c_void, out: *mut u32);
    pub fn bvn_float_to_bin64(f: *mut c_void, out: *mut u64);
    pub fn bvn_float_from_bin16(f: *mut c_void, bits: u16) -> bool;
    pub fn bvn_float_from_bin32(f: *mut c_void, bits: u32) -> bool;
    pub fn bvn_float_from_bin64(f: *mut c_void, bits: u64) -> bool;
    pub fn bvn_float_to_bin128(f: *mut c_void, out: *mut u32);
    pub fn bvn_float_to_bin256(f: *mut c_void, out: *mut u32);
    pub fn bvn_float_from_bin128(f: *mut c_void, bits: *mut u32) -> bool;
    pub fn bvn_float_from_bin256(f: *mut c_void, bits: *mut u32) -> bool;
    pub fn bvn_float_strtodec(s: *const c_char, width: u32, out: *mut u32, bits32: c_int) -> bool;
    pub fn bvn_float_to_dec16(f: *mut c_void, out: *mut u16);
    pub fn bvn_float_to_dec32(f: *mut c_void, out: *mut u32);
    pub fn bvn_float_to_dec64(f: *mut c_void, out: *mut u64);
    pub fn bvn_float_to_dec128(f: *mut c_void, out: *mut u32);
    pub fn bvn_float_to_dec256(f: *mut c_void, out: *mut u32);
    pub fn bvn_float_from_dec16(f: *mut c_void, bits: u16) -> bool;
    pub fn bvn_float_from_dec32(f: *mut c_void, bits: u32) -> bool;
    pub fn bvn_float_from_dec64(f: *mut c_void, bits: u64) -> bool;
    pub fn bvn_float_from_dec128(f: *mut c_void, bits: *mut u32) -> bool;
    pub fn bvn_float_from_dec256(f: *mut c_void, bits: *mut u32) -> bool;
    pub fn bvn_float_to_fix16(f: *mut c_void, frac_bits: u32) -> i16;
    pub fn bvn_float_to_fix32(f: *mut c_void, frac_bits: u32) -> i32;
    pub fn bvn_float_to_fix64(f: *mut c_void, frac_bits: u32) -> i64;
    pub fn bvn_float_to_fix128(f: *mut c_void, frac_bits: u32, out: *mut u32);
    pub fn bvn_float_to_fix256(f: *mut c_void, frac_bits: u32, out: *mut u32);
    pub fn bvn_float_from_fix16(f: *mut c_void, bits: i16, frac_bits: u32) -> bool;
    pub fn bvn_float_from_fix32(f: *mut c_void, bits: i32, frac_bits: u32) -> bool;
    pub fn bvn_float_from_fix64(f: *mut c_void, bits: i64, frac_bits: u32) -> bool;
    pub fn bvn_float_from_fix128(f: *mut c_void, bits: *mut u32, frac_bits: u32) -> bool;
    pub fn bvn_float_from_fix256(f: *mut c_void, bits: *mut u32, frac_bits: u32) -> bool;
    pub fn bvn_float_fix_in_range(f: *mut c_void, total_bits: u32, frac_bits: u32) -> bool;
    pub fn bvn_float_str_fits_fix(
        s: *const c_char,
        base: u32,
        total_bits: u32,
        frac_bits: u32,
    ) -> bool;
    pub fn bvn_float_is_nan(f: *mut c_void) -> bool;
    pub fn bvn_float_is_inf(f: *mut c_void) -> bool;
    pub fn bvn_float_is_zero(f: *mut c_void) -> bool;
    pub fn bvn_float_is_neg(f: *mut c_void) -> bool;
}

/// `bvn_error_to_string`: the C name of an error code, such as
/// `"unexpected_input_byte"`.
pub fn error_name(code: c_int) -> &'static str {
    // SAFETY: the function returns a static NUL-terminated ASCII string and
    // never NULL, for any code (unknown ones included).
    let name = unsafe { std::ffi::CStr::from_ptr(bvn_error_to_string(code)) };
    name.to_str().unwrap_or("unknown")
}

/// Borrows `len` bytes at `ptr` as a slice; NULL or zero length is empty.
///
/// SAFETY: a non-NULL `ptr` must point at `len` readable bytes that stay
/// valid and unchanged for `'a`.
unsafe fn bytes<'a>(ptr: *const c_void, len: u32) -> &'a [u8] {
    if ptr.is_null() || len == 0 {
        &[]
    } else {
        // SAFETY: guaranteed by the caller.
        unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len as usize) }
    }
}

/// One reader event as the callback sees it. Borrowed: valid only for the
/// duration of the callback.
pub struct EventView<'a> {
    pub event: c_int,
    pub data: Option<&'a BvnrData>,
    /// `data.data[..data.length]`, or empty.
    pub raw: &'a [u8],
}

/// A Rust event handler; returning false stops the read.
pub type Handler<'h> = &'h mut dyn FnMut(&EventView<'_>) -> bool;

/// What the read trampolines reach through `userdata`.
struct Handlers<'h> {
    verified: Option<Handler<'h>>,
    unverified: Option<Handler<'h>>,
    panic: Option<Box<dyn std::any::Any + Send>>,
}

extern "C" fn on_verified_trampoline(ud: *mut c_void, ev: c_int, d: *const BvnrData) -> bool {
    dispatch(ud, ev, d, true)
}

extern "C" fn on_unverified_trampoline(ud: *mut c_void, ev: c_int, d: *const BvnrData) -> bool {
    dispatch(ud, ev, d, false)
}

fn dispatch(ud: *mut c_void, ev: c_int, d: *const BvnrData, verified: bool) -> bool {
    // SAFETY: `ud` is the `Handlers` that `ReaderHandle::read_mem` keeps alive
    // and otherwise untouched for the whole bvnr_read call.
    let h = unsafe { &mut *ud.cast::<Handlers>() };
    if h.panic.is_some() {
        return false;
    }
    // SAFETY: the library passes NULL or a bvnr_data_t valid for this call.
    let data = unsafe { d.as_ref() };
    // SAFETY: data/length describe the event's bytes for this call.
    let raw = data.map_or(&[][..], |d| unsafe { bytes(d.data, d.length) });
    let view = EventView {
        event: ev,
        data,
        raw,
    };
    let handler = if verified {
        &mut h.verified
    } else {
        &mut h.unverified
    };
    let Some(f) = handler.as_mut() else {
        return true;
    };
    // A panic must not unwind into C: park it and rethrow after bvnr_read.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&view))) {
        Ok(go_on) => go_on,
        Err(payload) => {
            h.panic = Some(payload);
            false
        }
    }
}

/// The last error a reader recorded.
pub struct ReaderError {
    pub code: c_int,
    pub line: u64,
    pub column: u64,
    pub offset: u64,
    pub byte: u32,
}

/// An owned `bvnr_reader_t`.
pub struct ReaderHandle(std::ptr::NonNull<c_void>);

impl ReaderHandle {
    /// `bvnr_reader_create`; `None` when the library is out of memory.
    pub fn new() -> Option<Self> {
        // SAFETY: no preconditions; the result is NULL or a fresh reader.
        std::ptr::NonNull::new(unsafe { bvnr_reader_create() }).map(Self)
    }

    /// `bvnr_open_read_mem` + `bvnr_read` over `data` with the limits in
    /// `flags` (its userdata and callbacks are set here). Returns false when
    /// the library reports an error; a handler panic is resumed afterwards.
    pub fn read_mem<'h>(
        &mut self,
        data: &[u8],
        mut flags: BvnrReadFlags,
        verified: Option<Handler<'h>>,
        unverified: Option<Handler<'h>>,
    ) -> bool {
        let mut handlers = Handlers {
            verified,
            unverified,
            panic: None,
        };
        flags.on_verified = handlers
            .verified
            .is_some()
            .then_some(on_verified_trampoline as EventCallbackFn);
        flags.on_unverified = handlers
            .unverified
            .is_some()
            .then_some(on_unverified_trampoline as EventCallbackFn);
        flags.userdata = (&raw mut handlers).cast();
        // SAFETY: the reader is live; `data`, `flags` and `handlers` outlive
        // both calls, and nothing else touches `handlers` meanwhile.
        let ok = unsafe {
            bvnr_open_read_mem(
                self.0.as_ptr(),
                data.as_ptr().cast_mut().cast(),
                data.len() as u64,
                std::ptr::null_mut(),
                0,
                &mut flags,
            ) && bvnr_read(self.0.as_ptr())
        };
        if let Some(payload) = handlers.panic.take() {
            std::panic::resume_unwind(payload);
        }
        ok
    }

    /// The reader's last error and where it happened.
    pub fn error(&self) -> ReaderError {
        let r = self.0.as_ptr();
        // SAFETY: the reader is live; the getters only read it.
        unsafe {
            ReaderError {
                code: bvnr_reader_get_error(r),
                line: bvnr_reader_get_error_line(r),
                column: bvnr_reader_get_error_column(r),
                offset: bvnr_reader_get_error_offset(r),
                byte: bvnr_reader_get_error_byte(r),
            }
        }
    }
}

impl Drop for ReaderHandle {
    fn drop(&mut self) {
        // SAFETY: the reader is live and dropped exactly once.
        unsafe { bvnr_reader_destroy(self.0.as_ptr()) }
    }
}

/// Checks every mirrored struct against the compiler's own layout, as printed
/// by the upstream ABI manifest generator (tests/bvnr_abi_dump.c).
#[cfg(all(test, not(target_family = "wasm")))]
mod abi_tests {
    use super::*;
    use std::collections::HashMap;
    use std::mem::{offset_of, size_of};
    use std::path::Path;
    use std::process::Command;

    fn size_of_field<T, F>(_: fn(&T) -> &F) -> usize {
        size_of::<F>()
    }

    macro_rules! layout {
        ($out:ident, $t:ty, $c:literal { $($f:ident),* $(,)? }) => {
            $out.push(($c.to_string(), String::new(), 0, size_of::<$t>()));
            $(
                $out.push((
                    $c.to_string(),
                    // `type_` mirrors C's `type`, a Rust keyword.
                    stringify!($f).trim_end_matches('_').to_string(),
                    offset_of!($t, $f),
                    size_of_field(|s: &$t| &s.$f),
                ));
            )*
        };
    }

    /// (struct, field or "" for the struct itself, offset, size)
    fn rust_layout() -> Vec<(String, String, usize, usize)> {
        let mut v = Vec::new();
        layout!(v, ValueTypeSpec, "value_type_spec_t" { family, width, base });
        layout!(v, ValueUnitPrefix, "value_unit_prefix_t" { system, id });
        layout!(v, ValueUnitComponent, "value_unit_component_t" { base, exponent, prefix });
        layout!(v, ValueUnit, "value_unit_t" { num_components, components });
        layout!(v, BvnrData, "bvnr_data_t" {
            type_, value_type, value_unit, data, length, frac_data, frac_length,
            unit_source, unit_source_length, converted, conv,
        });
        layout!(v, BvnrConverted, "bvnr_converted_t" { unit, text, length, base, num, den });
        layout!(v, BvnrUnitTarget, "bvnr_unit_target_t" { unit, base });
        layout!(v, BvnrUnitRule, "bvnr_unit_rule_t" { path, unit, base, mode });
        layout!(v, BvnrUnitPolicy, "bvnr_unit_policy_t" {
            rules, num_rules, targets, num_targets, base, normalise, on_inexact,
            require_unit, require_dimension_of, num_require_dimension_of,
        });
        layout!(v, BvnrReadFlags, "bvnr_read_flags_t" {
            max_identifier_length, max_string_length, max_number_length,
            max_symbol_length, max_reference_length, max_array_items, max_text_bytes,
            max_file_size, max_struct_nesting, max_array_nesting, userdata,
            on_unverified, on_verified, continue_on_error, on_error, strict_version,
            text_only, want_unit_allow_nonterminating, max_conversion_length,
            want_unit, _reserved,
        });
        layout!(v, BvnrWriteFlags, "bvnr_write_flags_t" {
            max_identifier_length, max_string_length, max_number_length,
            max_symbol_length, max_reference_length, max_array_items, max_text_bytes,
            max_file_size, max_struct_nesting, max_array_nesting, userdata, on_event,
            continue_on_error, on_error, unit_flags, emit_version, _reserved,
        });
        layout!(v, BvnrDocStreamOpts, "bvnr_doc_stream_opts_t" {
            flags, userdata, on_document, continue_past_failed, max_document_size,
        });
        layout!(v, BvnDomEntry, "bvn_dom_entry_t" { key, value });
        v
    }

    #[test]
    fn struct_layouts_match_c() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let exe = Path::new(env!("OUT_DIR")).join("bvnr_abi_dump");
        let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
        let status = Command::new(cc)
            .arg("-std=c99")
            .arg("-I")
            .arg(repo.join("include"))
            .arg(repo.join("tests/bvnr_abi_dump.c"))
            .arg("-o")
            .arg(&exe)
            .status()
            .expect("run the C compiler");
        assert!(status.success(), "compiling bvnr_abi_dump.c failed");
        let out = Command::new(&exe).output().expect("run bvnr_abi_dump");
        let text = String::from_utf8(out.stdout).unwrap();

        let mut c: HashMap<(String, String), (usize, usize)> = HashMap::new();
        for line in text.lines() {
            let p: Vec<&str> = line.split_whitespace().collect();
            match p.as_slice() {
                ["STRUCT", s, size] => {
                    c.insert((s.to_string(), String::new()), (0, size.parse().unwrap()));
                }
                ["FIELD", s, f, off, size] => {
                    c.insert(
                        (s.to_string(), f.to_string()),
                        (off.parse().unwrap(), size.parse().unwrap()),
                    );
                }
                _ => panic!("unexpected ABI line: {line}"),
            }
        }

        let rust = rust_layout();
        for (s, f, off, size) in &rust {
            let want = c.get(&(s.clone(), f.clone()));
            assert_eq!(want, Some(&(*off, *size)), "{s}.{f}: Rust (offset, size)");
        }
        // Every field the manifest lists is mirrored.
        assert_eq!(c.len(), rust.len() + 2, "fields mirrored vs. C manifest");
        // The opaque blobs may be larger than C's, never smaller.
        for (s, rust_size) in [
            ("bvnr_source_t", size_of::<BvnrSource>()),
            ("bvnr_sink_t", size_of::<BvnrSink>()),
        ] {
            assert!(
                rust_size >= c[&(s.to_string(), String::new())].1,
                "{s} too small"
            );
        }
    }
}
