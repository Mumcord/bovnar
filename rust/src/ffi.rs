//! Hand-written declarations of the C API, mirroring `python/bovnar/_ffi.py`
//! (functions) and `python/bovnar/structs.py` (by-value struct layouts).
//!
//! This is the only module allowed to use `unsafe`. Besides the raw
//! declarations it holds the small safe handles the other modules build on, so
//! every pointer the library hands out is dereferenced here and nowhere else.

// The declaration list mirrors the whole of _ffi.py; the safe layer above it
// does not reach every function yet.
#![allow(dead_code)]

use std::ffi::{CStr, CString, c_char, c_int, c_void};

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

impl ValueUnitPrefix {
    /// The prefix id, whichever system it belongs to.
    pub fn id_raw(&self) -> c_int {
        // SAFETY: both union fields are a C int, so either reads the same int.
        unsafe { self.id.si }
    }
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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

#[repr(C)]
pub struct BvnCurrencyInfo {
    pub code: [c_char; 5],
    pub numeric_code: u16,
    pub minor_unit: u8,
    pub is_crypto: bool,
    pub name: [c_char; 48],
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
    // Not in _ffi.py: Python keeps its own copy of the currency table.
    pub fn bvn_unit_is_currency(base: c_int) -> bool;
    pub fn bvn_unit_is_fiat(base: c_int) -> bool;
    pub fn bvn_unit_is_crypto(base: c_int) -> bool;
    pub fn bvn_currency_info(base: c_int) -> *const BvnCurrencyInfo;
    pub fn bvn_parse_currency_str(s: *const u8, len: u32) -> c_int;
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
    /// A datetime's sub-second digits, or empty.
    pub frac: &'a [u8],
    /// The unit as the document spelled it, or empty.
    pub unit_source: &'a [u8],
    /// The converted literal when `data.converted`, or empty.
    pub conv_text: &'a [u8],
    /// The exact converted value when `data.converted`.
    pub conv_exact: Option<Rational<'a>>,
}

/// The exact result of a read-time conversion: the reader-owned bignums
/// `conv.num / conv.den`, valid only for the duration of the callback.
#[derive(Clone, Copy)]
pub struct Rational<'a> {
    num: *mut c_void,
    den: *mut c_void,
    _reader: std::marker::PhantomData<&'a BvnrData>,
}

impl std::fmt::Debug for Rational<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Rational")
    }
}

impl<'a> Rational<'a> {
    fn of(d: &'a BvnrData) -> Option<Self> {
        (d.converted && !d.conv.num.is_null() && !d.conv.den.is_null()).then_some(Rational {
            num: d.conv.num,
            den: d.conv.den,
            _reader: std::marker::PhantomData,
        })
    }

    /// The reduced numerator and denominator in decimal.
    pub fn parts(&self) -> Option<(String, String)> {
        Some((int_to_decimal(self.num)?, int_to_decimal(self.den)?))
    }

    /// `bvn_rational_to_str`: the value in `base`; `Err` for a base the
    /// library cannot write, `Ok(None)` when the expansion does not
    /// terminate.
    pub fn in_base(&self, base: u32) -> Result<Option<String>, ()> {
        // SAFETY: num/den are live reader-owned bignums for 'a.
        let size = unsafe { bvn_rational_str_bufsize(self.num, self.den, base) };
        if size == 0 {
            return Err(());
        }
        let mut buf = vec![0u8; size];
        let mut exact = false;
        // SAFETY: `buf` holds `size` bytes, as the library asked for.
        let n = unsafe {
            bvn_rational_to_str(
                self.num,
                self.den,
                base,
                buf.as_mut_ptr().cast(),
                size,
                &mut exact,
            )
        };
        if n < 0 || !exact {
            return Ok(None);
        }
        Ok(CStr::from_bytes_until_nul(&buf)
            .ok()
            .map(|t| t.to_string_lossy().into_owned()))
    }
}

/// `bvn_int_to_str` in base 10.
fn int_to_decimal(n: *mut c_void) -> Option<String> {
    // SAFETY: `n` is a live bignum; the getters only read it.
    let bits = unsafe { bvn_int_bitlen(n) }.max(1) as u32;
    // SAFETY: plain values in.
    let size = unsafe { bvn_int_str_bufsize(bits, 10) };
    let mut buf = vec![0u8; size];
    // SAFETY: `buf` holds `size` bytes, as the library asked for.
    if unsafe { bvn_int_to_str(n, buf.as_mut_ptr().cast(), size, 10) } < 0 {
        return None;
    }
    CStr::from_bytes_until_nul(&buf)
        .ok()
        .map(|t| t.to_string_lossy().into_owned())
}

impl<'a> EventView<'a> {
    /// Borrows everything `d` points at for as long as `d` itself.
    ///
    /// SAFETY: every pointer/length pair in `d` must describe bytes valid for
    /// `'a`, as the library guarantees for the duration of a callback.
    unsafe fn new(event: c_int, d: Option<&'a BvnrData>) -> Self {
        // SAFETY: guaranteed by the caller.
        let view = |p: *const c_void, n: u32| unsafe { bytes(p, n) };
        EventView {
            event,
            data: d,
            raw: d.map_or(&[][..], |d| view(d.data, d.length)),
            frac: d.map_or(&[][..], |d| view(d.frac_data, d.frac_length)),
            unit_source: d.map_or(&[][..], |d| view(d.unit_source, d.unit_source_length)),
            conv_text: d
                .filter(|d| d.converted)
                .map_or(&[][..], |d| view(d.conv.text.cast(), d.conv.length)),
            conv_exact: d.and_then(Rational::of),
        }
    }
}

fn c_buf_to_string(buf: &[u8]) -> Option<String> {
    CStr::from_bytes_until_nul(buf)
        .ok()
        .and_then(|c| c.to_str().ok())
        .map(str::to_owned)
}

/// A Rust event handler; returning false stops the read.
pub type Handler<'h> = &'h mut dyn FnMut(&EventView<'_>) -> bool;

/// A `want_unit` hook: the unit (and base) to deliver a value in, or `None`
/// to leave it as written.
pub type WantUnit<'h> = &'h mut dyn FnMut(&EventView<'_>) -> Option<(ValueUnit, u32)>;

/// What the read trampolines reach through `userdata`.
#[derive(Default)]
pub struct Handlers<'h> {
    pub verified: Option<Handler<'h>>,
    pub unverified: Option<Handler<'h>>,
    pub want_unit: Option<WantUnit<'h>>,
    /// Per-document callback of `bvnr_doc_stream_read`: (index, ok, error).
    pub on_document: Option<&'h mut dyn FnMut(u64, bool, c_int) -> bool>,
    /// The first error `on_error` reported: (code, line, column, byte, offset).
    pub first_error: Option<(c_int, u64, u64, u32, u64)>,
    pub panic: Option<Box<dyn std::any::Any + Send>>,
}

impl<'h> Handlers<'h> {
    /// Points `flags` at the trampolines for the handlers that are set, with
    /// `self` as userdata. `self` must stay put while the library uses it.
    fn install(&mut self, flags: &mut BvnrReadFlags) {
        flags.on_verified = self
            .verified
            .is_some()
            .then_some(on_verified_trampoline as EventCallbackFn);
        flags.on_unverified = self
            .unverified
            .is_some()
            .then_some(on_unverified_trampoline as EventCallbackFn);
        flags.want_unit = self
            .want_unit
            .is_some()
            .then_some(want_unit_trampoline as WantUnitFn);
        flags.on_error = Some(on_error_trampoline as OnErrorFn);
        flags.userdata = (self as *mut Self).cast();
    }

    /// Runs `f` on the handlers, parking a panic instead of unwinding into C.
    fn guard<R>(&mut self, fallback: R, f: impl FnOnce(&mut Self) -> R) -> R {
        if self.panic.is_some() {
            return fallback;
        }
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self))) {
            Ok(r) => r,
            Err(payload) => {
                self.panic = Some(payload);
                fallback
            }
        }
    }

    /// Rethrows a panic a handler raised during the last call.
    fn resume(&mut self) {
        if let Some(payload) = self.panic.take() {
            std::panic::resume_unwind(payload);
        }
    }
}

/// SAFETY: `ud` must be the `Handlers` installed for the running call, alive
/// and otherwise untouched for its duration.
unsafe fn handlers<'a>(ud: *mut c_void) -> &'a mut Handlers<'a> {
    // SAFETY: guaranteed by the caller.
    unsafe { &mut *ud.cast::<Handlers>() }
}

extern "C" fn want_unit_trampoline(
    ud: *mut c_void,
    d: *const BvnrData,
    want: *mut ValueUnit,
    want_base: *mut u32,
) -> bool {
    // SAFETY: `ud` is the installed `Handlers`; `d` is NULL or valid for the
    // call, and its pointers describe bytes valid for the call.
    let (h, view) = unsafe { (handlers(ud), EventView::new(0, d.as_ref())) };
    let Some((unit, base)) = h.guard(None, |h| h.want_unit.as_mut().and_then(|f| f(&view))) else {
        return false;
    };
    // SAFETY: `want` is writable; `want_base` is NULL or writable.
    unsafe {
        *want = unit;
        if let Some(b) = want_base.as_mut() {
            *b = base;
        }
    }
    true
}

extern "C" fn on_error_trampoline(
    ud: *mut c_void,
    err: c_int,
    line: u64,
    column: u64,
    byte: u32,
    offset: u64,
) {
    // SAFETY: `ud` is the installed `Handlers`.
    let h = unsafe { handlers(ud) };
    h.first_error
        .get_or_insert((err, line, column, byte, offset));
}

extern "C" fn on_verified_trampoline(ud: *mut c_void, ev: c_int, d: *const BvnrData) -> bool {
    dispatch(ud, ev, d, true)
}

extern "C" fn on_unverified_trampoline(ud: *mut c_void, ev: c_int, d: *const BvnrData) -> bool {
    dispatch(ud, ev, d, false)
}

fn dispatch(ud: *mut c_void, ev: c_int, d: *const BvnrData, verified: bool) -> bool {
    // SAFETY: `ud` is the installed `Handlers`; the library passes NULL or a
    // bvnr_data_t whose pointers are valid for this call.
    let (h, view) = unsafe { (handlers(ud), EventView::new(ev, d.as_ref())) };
    h.guard(false, |h| {
        let handler = if verified {
            &mut h.verified
        } else {
            &mut h.unverified
        };
        handler.as_mut().is_none_or(|f| f(&view))
    })
}

/// A unit policy in Rust terms, marshalled by [`with_policy`].
#[derive(Default)]
pub struct PolicySpec {
    /// (path, unit, base, convert)
    pub rules: Vec<(String, String, u32, bool)>,
    /// (unit, base)
    pub targets: Vec<(String, u32)>,
    pub base: u32,
    pub normalise: bool,
    pub leave_inexact: bool,
    pub require_unit: bool,
    pub require_dimension_of: Vec<String>,
}

/// Builds the C `bvnr_unit_policy_t` for `spec` and runs `f` on it (NULL for
/// `None`). False when a string holds a NUL byte or a count overflows.
fn with_policy(spec: Option<&PolicySpec>, f: impl FnOnce(*mut BvnrUnitPolicy) -> bool) -> bool {
    let Some(spec) = spec else {
        return f(std::ptr::null_mut());
    };
    let c = |t: &str| CString::new(t).ok();
    let Some(rule_strs) = spec
        .rules
        .iter()
        .map(|(p, u, _, _)| Some((c(p)?, c(u)?)))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let Some(target_strs) = spec
        .targets
        .iter()
        .map(|(u, _)| c(u))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let Some(dim_strs) = spec
        .require_dimension_of
        .iter()
        .map(|u| c(u))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let rules: Vec<BvnrUnitRule> = spec
        .rules
        .iter()
        .zip(&rule_strs)
        .map(|((_, _, base, convert), (p, u))| BvnrUnitRule {
            path: p.as_ptr(),
            unit: u.as_ptr(),
            base: *base,
            mode: c_int::from(!convert),
        })
        .collect();
    let targets: Vec<BvnrUnitTarget> = spec
        .targets
        .iter()
        .zip(&target_strs)
        .map(|((_, base), u)| BvnrUnitTarget {
            unit: u.as_ptr(),
            base: *base,
        })
        .collect();
    let dims: Vec<*const c_char> = dim_strs.iter().map(|d| d.as_ptr()).collect();
    let (Ok(num_rules), Ok(num_targets), Ok(num_dims)) = (
        u32::try_from(rules.len()),
        u32::try_from(targets.len()),
        u32::try_from(dims.len()),
    ) else {
        return false;
    };
    let mut policy = BvnrUnitPolicy {
        rules: rules.as_ptr(),
        num_rules,
        targets: targets.as_ptr(),
        num_targets,
        base: spec.base,
        normalise: c_int::from(spec.normalise),
        on_inexact: c_int::from(spec.leave_inexact),
        require_unit: spec.require_unit,
        require_dimension_of: dims.as_ptr(),
        num_require_dimension_of: num_dims,
    };
    // Every pointer in `policy` refers into the vectors above, alive until
    // `f` returns.
    f(&mut policy)
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
    pub fn read_mem(
        &mut self,
        data: &[u8],
        mut flags: BvnrReadFlags,
        h: &mut Handlers<'_>,
    ) -> bool {
        h.install(&mut flags);
        // SAFETY: the reader is live; `data`, `flags` and `h` outlive both
        // calls, and nothing else touches `h` meanwhile.
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
        h.resume();
        ok
    }

    /// `bvnr_reader_set_unit_policy`; `None` clears it.
    pub fn set_unit_policy(&mut self, policy: Option<&PolicySpec>) -> bool {
        let r = self.0.as_ptr();
        // SAFETY: the reader is live; the policy is only read during the call.
        with_policy(policy, |p| unsafe { bvnr_reader_set_unit_policy(r, p) })
    }

    /// `bvnr_reader_get_recovery_count` and `bvnr_reader_get_skipped_bytes`.
    pub fn recovery(&self) -> (u64, u64) {
        let r = self.0.as_ptr();
        // SAFETY: the reader is live; the getters only read it.
        unsafe {
            (
                bvnr_reader_get_recovery_count(r),
                bvnr_reader_get_skipped_bytes(r),
            )
        }
    }

    /// `bvnr_reader_get_declared_version`: the `#!bovnar M.N` directive of
    /// the last read, if it had one.
    pub fn declared_version(&self) -> Option<(u16, u16)> {
        let (mut major, mut minor) = (0u16, 0u16);
        // SAFETY: the reader is live; the getter writes two u16 out-params.
        unsafe { bvnr_reader_get_declared_version(self.0.as_ptr(), &mut major, &mut minor) }
            .then_some((major, minor))
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

/// An owned `bvn_dom_doc_t`.
pub struct DocHandle(std::ptr::NonNull<c_void>);

/// A node borrowed from a [`DocHandle`]; valid as long as the document.
#[derive(Clone, Copy)]
pub struct Node<'a>(
    std::ptr::NonNull<c_void>,
    std::marker::PhantomData<&'a DocHandle>,
);

/// Turns a borrowed `(key, node)` C entry array into Rust pairs.
///
/// SAFETY: `entries` must be NULL or point at `count` entries owned by a
/// document that outlives `'a`.
unsafe fn entries<'a>(entries: *const BvnDomEntry, count: u32) -> Vec<(&'a [u8], Node<'a>)> {
    if entries.is_null() {
        return Vec::new();
    }
    // SAFETY: guaranteed by the caller.
    let list = unsafe { std::slice::from_raw_parts(entries, count as usize) };
    list.iter()
        .filter_map(|e| {
            let node = std::ptr::NonNull::new(e.value)?;
            let key = if e.key.is_null() {
                &[][..]
            } else {
                // SAFETY: a non-NULL key is a NUL-terminated string the
                // document owns.
                unsafe { std::ffi::CStr::from_ptr(e.key) }.to_bytes()
            };
            Some((key, Node(node, std::marker::PhantomData)))
        })
        .collect()
}

impl DocHandle {
    /// `bvn_dom_parse`. `None` only on allocation failure (or input over
    /// 4 GiB, which the C length cannot express); a malformed document
    /// returns a handle whose `parse_error` is set.
    pub fn parse(data: &[u8]) -> Option<Self> {
        let len = u32::try_from(data.len()).ok()?;
        // SAFETY: `data` is valid for `len` bytes and only read during the call.
        let doc = unsafe { bvn_dom_parse(data.as_ptr().cast_mut().cast(), len) };
        std::ptr::NonNull::new(doc).map(Self)
    }

    /// `bvn_dom_parse_policy`: `None` also when the policy cannot be built.
    pub fn parse_policy(data: &[u8], policy: &PolicySpec) -> Option<Self> {
        let len = u32::try_from(data.len()).ok()?;
        let mut doc = std::ptr::null_mut();
        with_policy(Some(policy), |p| {
            // SAFETY: `data` is valid for `len` bytes and `p` for the call;
            // both are only read.
            doc = unsafe { bvn_dom_parse_policy(data.as_ptr().cast_mut().cast(), len, p) };
            true
        });
        std::ptr::NonNull::new(doc).map(Self)
    }

    /// `bvn_dom_doc_get_parse_error`.
    pub fn parse_error(&self) -> c_int {
        // SAFETY: the document is live.
        unsafe { bvn_dom_doc_get_parse_error(self.0.as_ptr()) }
    }

    /// `bvn_dom_lookup`: a dotted path such as `"a.b"`.
    pub fn lookup(&self, path: &str) -> Option<Node<'_>> {
        let path = std::ffi::CString::new(path).ok()?;
        // SAFETY: the document is live; `path` is NUL-terminated.
        let node = unsafe { bvn_dom_lookup(self.0.as_ptr(), path.as_ptr()) };
        std::ptr::NonNull::new(node).map(|n| Node(n, std::marker::PhantomData))
    }

    /// The top-level `(key, node)` pairs, in document order.
    pub fn entries(&self) -> Vec<(&[u8], Node<'_>)> {
        let doc = self.0.as_ptr();
        // SAFETY: the document is live and owns the entry array.
        unsafe { entries(bvn_dom_doc_entries(doc), bvn_dom_doc_count(doc)) }
    }
}

impl Drop for DocHandle {
    fn drop(&mut self) {
        // SAFETY: the document is live and dropped exactly once; no Node can
        // outlive it (they borrow it).
        unsafe { bvn_dom_doc_destroy(self.0.as_ptr()) }
    }
}

impl<'a> Node<'a> {
    fn ptr(self) -> *mut c_void {
        self.0.as_ptr()
    }

    /// `bvn_dom_node_type`.
    pub fn node_type(self) -> c_int {
        // SAFETY: the node is live (borrowed from its document).
        unsafe { bvn_dom_node_type(self.ptr()) }
    }

    /// `bvn_dom_get_value_type`.
    pub fn value_type(self) -> ValueTypeSpec {
        // SAFETY: as above.
        unsafe { bvn_dom_get_value_type(self.ptr()) }
    }

    /// `bvn_dom_get_unit_string`: the canonical unit, empty when unitless.
    pub fn unit_string(self) -> String {
        let mut buf = vec![0u8; UNIT_STRING_MAX];
        // SAFETY: the node is live; `buf` has UNIT_STRING_MAX writable bytes.
        let n = unsafe { bvn_dom_get_unit_string(self.ptr(), buf.as_mut_ptr().cast(), buf.len()) };
        buf.truncate(usize::try_from(n).unwrap_or(0).min(UNIT_STRING_MAX));
        String::from_utf8_lossy(&buf).into_owned()
    }

    /// `bvn_dom_get_i64`.
    pub fn get_i64(self) -> Option<i64> {
        let mut v = 0;
        // SAFETY: the node is live; `v` is a valid out-param.
        unsafe { bvn_dom_get_i64(self.ptr(), &mut v) }.then_some(v)
    }

    /// `bvn_dom_get_u64`.
    pub fn get_u64(self) -> Option<u64> {
        let mut v = 0;
        // SAFETY: as above.
        unsafe { bvn_dom_get_u64(self.ptr(), &mut v) }.then_some(v)
    }

    /// `bvn_dom_get_i32`.
    pub fn get_i32(self) -> Option<i32> {
        let mut v = 0;
        // SAFETY: as above.
        unsafe { bvn_dom_get_i32(self.ptr(), &mut v) }.then_some(v)
    }

    /// `bvn_dom_get_u32`.
    pub fn get_u32(self) -> Option<u32> {
        let mut v = 0;
        // SAFETY: as above.
        unsafe { bvn_dom_get_u32(self.ptr(), &mut v) }.then_some(v)
    }

    /// `bvn_dom_get_i16`.
    pub fn get_i16(self) -> Option<i16> {
        let mut v = 0;
        // SAFETY: as above.
        unsafe { bvn_dom_get_i16(self.ptr(), &mut v) }.then_some(v)
    }

    /// `bvn_dom_get_u16`.
    pub fn get_u16(self) -> Option<u16> {
        let mut v = 0;
        // SAFETY: as above.
        unsafe { bvn_dom_get_u16(self.ptr(), &mut v) }.then_some(v)
    }

    /// `bvn_dom_get_i8`.
    pub fn get_i8(self) -> Option<i8> {
        let mut v = 0;
        // SAFETY: as above.
        unsafe { bvn_dom_get_i8(self.ptr(), &mut v) }.then_some(v)
    }

    /// `bvn_dom_get_u8`.
    pub fn get_u8(self) -> Option<u8> {
        let mut v = 0;
        // SAFETY: as above.
        unsafe { bvn_dom_get_u8(self.ptr(), &mut v) }.then_some(v)
    }

    /// `bvn_dom_get_float`.
    pub fn get_float(self) -> Option<f64> {
        let mut v = 0.0;
        // SAFETY: as above.
        unsafe { bvn_dom_get_float(self.ptr(), &mut v) }.then_some(v)
    }

    /// `bvn_dom_get_bool`.
    pub fn get_bool(self) -> Option<bool> {
        let mut v = false;
        // SAFETY: as above.
        unsafe { bvn_dom_get_bool(self.ptr(), &mut v) }.then_some(v)
    }

    /// The bytes of a string, symbol, reference or octet-stream node,
    /// chosen by `getter`.
    fn get_bytes(
        self,
        getter: unsafe extern "C" fn(*mut c_void, *mut *const c_void, *mut u32) -> bool,
    ) -> Option<&'a [u8]> {
        let mut ptr = std::ptr::null();
        let mut len = 0;
        // SAFETY: the node is live; on success ptr/len describe bytes the
        // node owns, so they live as long as the document.
        unsafe { getter(self.ptr(), &mut ptr, &mut len).then(|| bytes(ptr, len)) }
    }

    /// `bvn_dom_get_string`.
    pub fn get_string(self) -> Option<&'a [u8]> {
        self.get_bytes(bvn_dom_get_string)
    }

    /// `bvn_dom_get_symbol`.
    pub fn get_symbol(self) -> Option<&'a [u8]> {
        self.get_bytes(bvn_dom_get_symbol)
    }

    /// `bvn_dom_get_unit`.
    pub fn unit(self) -> ValueUnit {
        // SAFETY: the node is live.
        unsafe { bvn_dom_get_unit(self.ptr()) }
    }

    /// `bvn_dom_get_value_in_base_units`.
    pub fn value_in_base_units(self) -> f64 {
        // SAFETY: the node is live.
        unsafe { bvn_dom_get_value_in_base_units(self.ptr()) }
    }

    /// `bvn_dom_get_unit_source` and `bvn_dom_get_datetime_fraction`: the
    /// unit's spelling and a datetime's sub-second digits, when present.
    pub fn unit_source_and_fraction(self) -> (Option<&'a [u8]>, Option<&'a [u8]>) {
        let (mut ul, mut fl) = (0u32, 0u32);
        // SAFETY: the node is live; the results are NULL or `len` bytes owned
        // by the document, which outlives 'a.
        unsafe {
            let u = bvn_dom_get_unit_source(self.ptr(), &mut ul);
            let f = bvn_dom_get_datetime_fraction(self.ptr(), &mut fl);
            (
                (!u.is_null()).then(|| bytes(u.cast(), ul)),
                (!f.is_null()).then(|| bytes(f.cast(), fl)),
            )
        }
    }

    /// `bvn_dom_get_reference`.
    pub fn get_reference(self) -> Option<&'a [u8]> {
        self.get_bytes(bvn_dom_get_reference)
    }

    /// `bvn_dom_get_octets`.
    pub fn get_octets(self) -> Option<&'a [u8]> {
        self.get_bytes(bvn_dom_get_octets)
    }

    /// `bvn_dom_int_to_str`: any integer node, however wide, in `base`.
    pub fn int_to_str(self, base: u32) -> Option<String> {
        // SAFETY: the node is live.
        let s = unsafe { bvn_dom_int_to_str(self.ptr(), base) };
        if s.is_null() {
            return None;
        }
        // SAFETY: a non-NULL result is a fresh NUL-terminated string that we
        // own and release exactly once, after copying it.
        unsafe {
            let text = std::ffi::CStr::from_ptr(s.cast())
                .to_string_lossy()
                .into_owned();
            bvn_dom_free_string(s);
            Some(text)
        }
    }

    /// `bvn_dom_struct_count`.
    pub fn struct_count(self) -> u32 {
        // SAFETY: the node is live.
        unsafe { bvn_dom_struct_count(self.ptr()) }
    }

    /// `bvn_dom_array_count`.
    pub fn array_count(self) -> u32 {
        // SAFETY: the node is live.
        unsafe { bvn_dom_array_count(self.ptr()) }
    }

    /// `bvn_dom_array_dims`.
    pub fn array_dims(self) -> u32 {
        // SAFETY: the node is live.
        unsafe { bvn_dom_array_dims(self.ptr()) }
    }

    /// `bvn_dom_struct_get`.
    pub fn struct_get(self, key: &str) -> Option<Node<'a>> {
        let key = std::ffi::CString::new(key).ok()?;
        // SAFETY: the node is live; `key` is NUL-terminated.
        let n = unsafe { bvn_dom_struct_get(self.ptr(), key.as_ptr()) };
        std::ptr::NonNull::new(n).map(|n| Node(n, std::marker::PhantomData))
    }

    /// `bvn_dom_array_at`.
    pub fn array_at(self, index: u32) -> Option<Node<'a>> {
        // SAFETY: the node is live; the library bounds-checks `index`.
        let n = unsafe { bvn_dom_array_at(self.ptr(), index) };
        std::ptr::NonNull::new(n).map(|n| Node(n, std::marker::PhantomData))
    }

    /// The `(key, node)` pairs of a struct node, in document order.
    pub fn struct_entries(self) -> Vec<(&'a [u8], Node<'a>)> {
        // SAFETY: the node is live and owns its entry array.
        unsafe { entries(bvn_dom_struct_entries(self.ptr()), self.struct_count()) }
    }
}

/// An owned `bvnr_writer_t` writing into a buffer it owns.
pub struct WriterHandle {
    ptr: std::ptr::NonNull<c_void>,
    /// The memory sink. Allocated once in `open_mem` and never resized, so the
    /// address the library holds stays valid until the writer is destroyed.
    buf: Vec<u8>,
}

impl WriterHandle {
    /// `bvnr_writer_create`; `None` when the library is out of memory.
    pub fn new() -> Option<Self> {
        // SAFETY: no preconditions; the result is NULL or a fresh writer.
        let ptr = std::ptr::NonNull::new(unsafe { bvnr_writer_create() })?;
        Some(WriterHandle {
            ptr,
            buf: Vec::new(),
        })
    }

    /// `bvnr_open_write_mem` into a fresh buffer of `cap` bytes.
    pub fn open_mem(&mut self, cap: usize, pretty: bool, mut flags: BvnrWriteFlags) -> bool {
        self.buf = vec![0; cap];
        // SAFETY: the writer is live and `buf` (cap bytes) lives, unmoved and
        // unresized, as long as the writer; `flags` is only read during the call.
        unsafe {
            bvnr_open_write_mem(
                self.ptr.as_ptr(),
                self.buf.as_mut_ptr().cast(),
                cap as u64,
                pretty,
                &mut flags,
            )
        }
    }

    /// `bvnr_write_event` with a payload of `data` (key, literal or octet
    /// chunk). False when the library refuses it or `data` exceeds 4 GiB.
    pub fn write_event(
        &mut self,
        event: c_int,
        token_type: c_int,
        value_type: ValueTypeSpec,
        data: &[u8],
    ) -> bool {
        let Ok(length) = u32::try_from(data.len()) else {
            return false;
        };
        let mut d = BvnrData::zeroed();
        d.type_ = token_type;
        d.value_type = value_type;
        d.data = data.as_ptr().cast();
        d.length = length;
        // SAFETY: the writer is live; `d` and the bytes it points at are
        // valid for the call, and the library copies what it keeps.
        unsafe { bvnr_write_event(self.ptr.as_ptr(), event, &mut d) }
    }

    /// `bvnr_write_event` with every payload field: the literal `data`, the
    /// value's unit, a datetime's sub-second digits `frac` and the unit's
    /// spelling `unit_source` (empty for none).
    pub fn write_event_full(
        &mut self,
        event: c_int,
        token_type: c_int,
        value_type: ValueTypeSpec,
        unit: &ValueUnit,
        [data, frac, unit_source]: [&[u8]; 3],
    ) -> bool {
        let (Ok(length), Ok(frac_length), Ok(unit_source_length)) = (
            u32::try_from(data.len()),
            u32::try_from(frac.len()),
            u32::try_from(unit_source.len()),
        ) else {
            return false;
        };
        let mut d = BvnrData::zeroed();
        d.type_ = token_type;
        d.value_type = value_type;
        d.value_unit = *unit;
        d.data = data.as_ptr().cast();
        d.length = length;
        if !frac.is_empty() {
            d.frac_data = frac.as_ptr().cast();
            d.frac_length = frac_length;
        }
        if !unit_source.is_empty() {
            d.unit_source = unit_source.as_ptr().cast();
            d.unit_source_length = unit_source_length;
        }
        // SAFETY: the writer is live; `d` and the bytes it points at are
        // valid for the call, and the library copies what it keeps.
        unsafe { bvnr_write_event(self.ptr.as_ptr(), event, &mut d) }
    }

    /// `bvnr_writer_set_unit_policy`; `None` clears it.
    pub fn set_unit_policy(&mut self, policy: Option<&PolicySpec>) -> bool {
        let w = self.ptr.as_ptr();
        // SAFETY: the writer is live; the policy is only read during the call.
        with_policy(policy, |p| unsafe { bvnr_writer_set_unit_policy(w, p) })
    }

    /// `bvnr_mux_begin`, then `bvnr_mux_send` for each message, then
    /// `bvnr_mux_end`: a multiplexed stream under `key`.
    pub fn mux(&mut self, key: &str, messages: &[(u64, &[u8])]) -> bool {
        let Ok(key) = CString::new(key) else {
            return false;
        };
        let w = self.ptr.as_ptr();
        // SAFETY: the writer is live; `key` and every payload are valid for
        // their call and only read.
        unsafe {
            bvnr_mux_begin(w, key.as_ptr())
                && messages.iter().all(|(ch, data)| {
                    bvnr_mux_send(w, *ch, data.as_ptr().cast_mut().cast(), data.len() as u64)
                })
                && bvnr_mux_end(w)
        }
    }

    /// `bvnr_embed_document`: `doc` as a canonical embedded document under `key`.
    pub fn embed_document(&mut self, key: &str, doc: &[u8]) -> bool {
        let Ok(key) = CString::new(key) else {
            return false;
        };
        // SAFETY: the writer is live; `key` and `doc` are only read during
        // the call.
        unsafe {
            bvnr_embed_document(
                self.ptr.as_ptr(),
                key.as_ptr(),
                doc.as_ptr().cast_mut().cast(),
                doc.len() as u64,
            )
        }
    }

    /// `bvnr_writer_unit_flags`.
    pub fn unit_flags(&self) -> u32 {
        // SAFETY: the writer is live.
        unsafe { bvnr_writer_unit_flags(self.ptr.as_ptr()) }
    }

    /// `bvnr_write_version`.
    pub fn write_version(&mut self, major: u16, minor: u16) -> bool {
        // SAFETY: the writer is live.
        unsafe { bvnr_write_version(self.ptr.as_ptr(), major, minor) }
    }

    /// `bvnr_write_finish`.
    pub fn finish(&mut self) -> bool {
        // SAFETY: the writer is live.
        unsafe { bvnr_write_finish(self.ptr.as_ptr()) }
    }

    /// `bvnr_writer_get_error` and `bvnr_writer_get_error_offset`.
    pub fn error(&self) -> (c_int, u64) {
        let w = self.ptr.as_ptr();
        // SAFETY: the writer is live; the getters only read it.
        unsafe { (bvnr_writer_get_error(w), bvnr_writer_get_error_offset(w)) }
    }

    /// `bvnr_writer_bytes_written`.
    pub fn bytes_written(&self) -> u64 {
        // SAFETY: the writer is live.
        unsafe { bvnr_writer_bytes_written(self.ptr.as_ptr()) }
    }

    /// What the writer has put into its buffer so far.
    pub fn output(&self) -> &[u8] {
        let n = usize::try_from(self.bytes_written()).unwrap_or(usize::MAX);
        &self.buf[..n.min(self.buf.len())]
    }
}

impl Drop for WriterHandle {
    fn drop(&mut self) {
        // SAFETY: the writer is live and destroyed exactly once, before `buf`.
        unsafe { bvnr_writer_destroy(self.ptr.as_ptr()) }
    }
}

/// Runs one of the `bvn_format_*` functions on a 128-byte buffer.
fn format(f: impl FnOnce(*mut c_char, usize) -> i32) -> Option<String> {
    let mut buf = [0u8; 128];
    let n = usize::try_from(f(buf.as_mut_ptr().cast(), buf.len())).ok()?;
    String::from_utf8(buf.get(..n)?.to_vec()).ok()
}

/// `bvn_format_uint64`: `value` as digits in `base`.
pub fn format_u64(value: u64, base: u32) -> Option<String> {
    // SAFETY: `buf` has `len` writable bytes for the call.
    format(|buf, len| unsafe { bvn_format_uint64(buf, len, value, base, 0) })
}

/// `bvn_format_int64`: `value` as digits in `base`.
pub fn format_i64(value: i64, base: u32) -> Option<String> {
    // SAFETY: as above.
    format(|buf, len| unsafe { bvn_format_int64(buf, len, value, base, 0) })
}

/// `bvn_format_double`: `value` as the literal for type `vt`.
pub fn format_f64(value: f64, vt: ValueTypeSpec) -> Option<String> {
    // SAFETY: as above.
    format(|buf, len| unsafe { bvn_format_double(buf, len, value, vt) })
}

// ---------------------------------------------------------------------------
// Units: value-in, value-out helpers over `value_unit_t`.

/// Runs a `bvn_unit_to_*` formatter on a `UNIT_STRING_MAX` buffer: the text,
/// or the negative return value the library refused with.
fn unit_text(f: impl FnOnce(*mut c_char, usize) -> i32) -> std::result::Result<String, i32> {
    let mut buf = vec![0u8; UNIT_STRING_MAX];
    let n = f(buf.as_mut_ptr().cast(), buf.len());
    let len = usize::try_from(n).map_err(|_| n)?;
    buf.truncate(len.min(UNIT_STRING_MAX));
    String::from_utf8(buf).map_err(|_| -1)
}

/// `bvn_parse_unit_n`: `None` for text that is not a unit.
pub fn parse_unit(text: &str) -> Option<ValueUnit> {
    let len = u32::try_from(text.len()).ok()?;
    let mut ok = false;
    // SAFETY: `text` has `len` readable bytes for the call; `ok` is writable.
    let u = unsafe { bvn_parse_unit_n(text.as_ptr(), len, &mut ok) };
    ok.then_some(u)
}

/// `bvn_unit_error_code`: why `text` is not a unit (0 when it is one).
pub fn unit_error_code(text: &str) -> c_int {
    let len = u32::try_from(text.len()).unwrap_or(u32::MAX);
    // SAFETY: `text` has at least `len` readable bytes for the call.
    unsafe { bvn_unit_error_code(text.as_ptr(), len) }
}

/// `bvn_unit_to_string_ex` with `flags` (0 is `bvn_unit_to_string`).
pub fn unit_to_string_ex(u: &ValueUnit, flags: u32) -> std::result::Result<String, i32> {
    // SAFETY: the buffer has `len` writable bytes for the call.
    unit_text(|buf, len| unsafe { bvn_unit_to_string_ex(*u, buf, len, flags) })
}

/// `bvn_unit_to_profile`: `u` as a code in vocabulary `ns`.
pub fn unit_to_profile(ns: &str, u: &ValueUnit) -> Option<String> {
    let ns = CString::new(ns).ok()?;
    // SAFETY: `ns` is NUL-terminated; the buffer has `len` writable bytes.
    unit_text(|buf, len| unsafe { bvn_unit_to_profile(ns.as_ptr(), *u, buf, len) }).ok()
}

/// `bvn_unit_to_ucum`.
pub fn unit_to_ucum(u: &ValueUnit) -> Option<String> {
    // SAFETY: the buffer has `len` writable bytes for the call.
    unit_text(|buf, len| unsafe { bvn_unit_to_ucum(*u, buf, len) }).ok()
}

/// `bvn_unit_profile_name` for every index below `bvn_unit_profile_count`.
pub fn unit_profiles() -> Vec<String> {
    // SAFETY: no preconditions.
    let count = unsafe { bvn_unit_profile_count() };
    (0..count)
        .filter_map(|i| {
            // SAFETY: `i` is in range; the result is NULL or a static string.
            let p = unsafe { bvn_unit_profile_name(i) };
            // SAFETY: a non-NULL result is a static NUL-terminated string.
            (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned())
        })
        .collect()
}

/// The `value_unit_t` predicates and scalars that take units by value and
/// touch nothing else; each is safe for any bit pattern of its arguments.
pub mod unit_fns {
    use super::*;

    macro_rules! by_value {
        ($($name:ident => $c:ident($($a:ident: $t:ty),*) -> $r:ty;)*) => {$(
            #[doc = concat!("`", stringify!($c), "`.")]
            pub fn $name($($a: $t),*) -> $r {
                // SAFETY: plain values in, a plain value out; the library
                // validates the unit structs it is handed.
                unsafe { super::$c($($a),*) }
            }
        )*};
    }

    by_value! {
        valid => bvn_unit_valid(u: ValueUnit) -> bool;
        is_profile_only => bvn_unit_is_profile_only(u: ValueUnit) -> bool;
        prefix_factor => bvn_unit_prefix_factor(u: ValueUnit) -> f64;
        prefix_exponent => bvn_unit_prefix_exponent(u: ValueUnit) -> i32;
        compatible => bvn_units_compatible(a: ValueUnit, b: ValueUnit) -> bool;
        equal => bvn_unit_equal(a: ValueUnit, b: ValueUnit) -> bool;
        convertible => bvn_units_convertible(a: ValueUnit, b: ValueUnit) -> bool;
        exponent_to_int => bvn_exponent_to_int(e: c_int) -> i32;
        int_to_exponent => bvn_int_to_exponent(n: i32) -> c_int;
        prefix_unit_valid => bvn_prefix_unit_valid(p: ValueUnitPrefix, base: c_int) -> bool;
    }

    /// `bvn_unit_si_normal_form`.
    pub fn si_normal_form(u: ValueUnit) -> Option<ValueUnit> {
        let mut out = ValueUnit::zeroed();
        // SAFETY: `out` is writable for the call.
        unsafe { bvn_unit_si_normal_form(u, &mut out) }.then_some(out)
    }

    /// `bvn_unit_convert_factor`: (factor, ok, requires_affine).
    pub fn convert_factor(a: ValueUnit, b: ValueUnit) -> (f64, bool, bool) {
        let (mut ok, mut affine) = (false, false);
        // SAFETY: both flags are writable for the call.
        let f = unsafe { bvn_unit_convert_factor(a, b, &mut ok, &mut affine) };
        (f, ok, affine)
    }

    /// `bvn_unit_convert_value`.
    pub fn convert_value(value: f64, from: ValueUnit, to: ValueUnit) -> Option<f64> {
        let mut out = 0.0;
        // SAFETY: `out` is writable for the call.
        unsafe { bvn_unit_convert_value(value, from, to, &mut out) }.then_some(out)
    }

    /// `bvn_unit_to_si_factor`: (factor, is_affine, affine_offset), or
    /// `None` when the unit has no SI conversion.
    pub fn to_si_factor(u: ValueUnit) -> Option<(f64, bool, f64)> {
        let (mut affine, mut offset, mut ok) = (false, 0.0, false);
        // SAFETY: the three outputs are writable for the call.
        let f = unsafe { bvn_unit_to_si_factor(u, &mut affine, &mut offset, &mut ok) };
        ok.then_some((f, affine, offset))
    }

    /// `bvn_unit_dimension_vector`: the exponents of the seven SI base
    /// dimensions.
    pub fn dimension_vector(u: ValueUnit) -> Option<[i32; 7]> {
        let mut dims = [0i32; 7];
        // SAFETY: `dims` has the seven slots the function fills.
        unsafe { bvn_unit_dimension_vector(u, dims.as_mut_ptr()) }.then_some(dims)
    }

    /// `bvn_unit_reduce`: (unit, scale), or `None` when the scale overflows.
    pub fn reduce(u: ValueUnit) -> Option<(ValueUnit, f64)> {
        let (mut scale, mut overflow) = (1.0, false);
        // SAFETY: both outputs are writable for the call.
        let r = unsafe { bvn_unit_reduce(u, &mut scale, &mut overflow) };
        (!overflow).then_some((r, scale))
    }
}

// ---------------------------------------------------------------------------
// Currencies.

/// `bvn_unit_is_currency`, `bvn_unit_is_fiat`, `bvn_unit_is_crypto`.
pub fn currency_kind(base: c_int) -> (bool, bool, bool) {
    // SAFETY: plain value in, plain value out, for any id.
    unsafe {
        (
            bvn_unit_is_currency(base),
            bvn_unit_is_fiat(base),
            bvn_unit_is_crypto(base),
        )
    }
}

/// `bvn_currency_info`: (code, numeric code, minor unit, is crypto, name).
pub fn currency_info(base: c_int) -> Option<(String, u16, u8, bool, String)> {
    // SAFETY: the result is NULL or a row of the static catalogue.
    let row = unsafe { bvn_currency_info(base).as_ref() }?;
    let text = |chars: &[c_char]| {
        let bytes: Vec<u8> = chars
            .iter()
            .take_while(|&&c| c != 0)
            // c_char is i8 on x86 and u8 on aarch64 Linux: take the byte as is.
            .map(|&c| c.to_ne_bytes()[0])
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    };
    Some((
        text(&row.code),
        row.numeric_code,
        row.minor_unit,
        row.is_crypto,
        text(&row.name),
    ))
}

/// `bvn_parse_currency_str`: the base-unit id of a code, 0 when unknown.
pub fn parse_currency(code: &str) -> c_int {
    let len = u32::try_from(code.len()).unwrap_or(u32::MAX);
    // SAFETY: `code` has at least `len` readable bytes for the call.
    unsafe { bvn_parse_currency_str(code.as_ptr(), len) }
}

// ---------------------------------------------------------------------------
// Versions.

/// `bvnr_version_string`: the library version, e.g. `"2.0.0"`.
pub fn version_string() -> String {
    // SAFETY: returns a static NUL-terminated string.
    unsafe { CStr::from_ptr(bvnr_version_string()) }
        .to_string_lossy()
        .into_owned()
}

/// `bvnr_spec_version`: the newest format version the library implements.
pub fn spec_version() -> (u16, u16) {
    let (mut major, mut minor) = (0u16, 0u16);
    // SAFETY: writes two u16 out-params.
    unsafe { bvnr_spec_version(&mut major, &mut minor) };
    (major, minor)
}

/// `bvnr_peek_version`: the `#!bovnar M.N` directive leading `data`, if any.
pub fn peek_version(data: &[u8]) -> Option<(u16, u16)> {
    let (mut major, mut minor) = (0u16, 0u16);
    // SAFETY: the C side only reads `len` bytes of the buffer.
    unsafe {
        bvnr_peek_version(
            data.as_ptr() as *mut c_void,
            data.len() as u64,
            &mut major,
            &mut minor,
        )
    }
    .then_some((major, minor))
}

// Numbers and datetimes.

/// `bvnr_datetime_epoch_name` and `bvnr_datetime_epoch_mjd` of a datetime type.
pub fn datetime_epoch(vt: ValueTypeSpec) -> (Option<String>, i32) {
    // SAFETY: plain value in; the name is NULL or a static string.
    let (name, mjd) = unsafe { (bvnr_datetime_epoch_name(vt), bvnr_datetime_epoch_mjd(vt)) };
    // SAFETY: a non-NULL name is a static NUL-terminated string.
    let name = (!name.is_null()).then(|| {
        unsafe { CStr::from_ptr(name) }
            .to_string_lossy()
            .into_owned()
    });
    (name, mjd)
}

/// `bvn_float_str_fits_fix`: whether decimal `text` fits a signed fixed
/// point of `total_bits` with `frac_bits` fractional bits.
pub fn float_str_fits_fix(text: &str, total_bits: u32, frac_bits: u32) -> bool {
    let Ok(t) = CString::new(text) else {
        return false;
    };
    // SAFETY: `t` is NUL-terminated and only read.
    unsafe { bvn_float_str_fits_fix(t.as_ptr(), 10, total_bits, frac_bits) }
}

/// `bvn_float_strtodec`: decimal `text` as the IEEE decimal interchange
/// encoding of `width` bits (little-endian bytes), rounded once straight from
/// the string. `None` for a width with no decimal format or a refused literal.
pub fn float_strtodec(text: &str, width: u32) -> Option<Vec<u8>> {
    let words: c_int = match width {
        16 | 32 => 1,
        64 => 2,
        128 => 4,
        256 => 8,
        _ => return None,
    };
    let t = CString::new(text).ok()?;
    let mut buf = [0u32; 8];
    // SAFETY: `t` is NUL-terminated and only read; `buf` holds at least
    // `words` words.
    let ok = unsafe { bvn_float_strtodec(t.as_ptr(), width, buf.as_mut_ptr(), words) };
    ok.then(|| {
        buf.iter()
            .flat_map(|w| w.to_le_bytes())
            .take(width as usize / 8)
            .collect()
    })
}

/// A float literal in `base` (16 for a hex float) as the nearest `f64`, via
/// the library's arbitrary-precision float at Python's working precision.
pub fn parse_float_base(text: &str, base: u32) -> Option<f64> {
    let t = CString::new(text).ok()?;
    // SAFETY: the float is allocated, used and freed here; `t` is
    // NUL-terminated and `out` writable.
    unsafe {
        let f = bvn_float_alloc(4096);
        if f.is_null() {
            return None;
        }
        let mut out = 0.0;
        let ok = bvn_float_from_str(f, t.as_ptr(), base) && bvn_float_to_double(f, &mut out);
        bvn_float_free(f);
        ok.then_some(out)
    }
}

// ---------------------------------------------------------------------------
// Streams: framing, multiplexing, embedding.

/// `bvnr_sink_to_mem` + `bvnr_frame_write` for each document: the framed
/// concatenation, 12 header bytes per document.
pub fn frame_documents(docs: &[&[u8]]) -> Option<Vec<u8>> {
    let total: usize = docs.iter().map(|d| 12 + d.len()).sum();
    let mut out = vec![0u8; total.max(1)];
    let mut sink = BvnrSink {
        opaque: [0; OPAQUE_BYTES / 8],
    };
    // SAFETY: `out` has `total` writable bytes and outlives `sink`; every
    // document is only read during its call.
    let ok = unsafe {
        bvnr_sink_to_mem(&mut sink, out.as_mut_ptr().cast(), total as u64);
        docs.iter()
            .all(|d| bvnr_frame_write(&mut sink, d.as_ptr().cast_mut().cast(), d.len() as u64))
    };
    out.truncate(total);
    ok.then_some(out)
}

/// `bvnr_doc_stream_read` over `data`: `h.verified` sees each document's
/// events and `h.on_document` each document's end. Returns (ok, count).
pub fn doc_stream_read(
    data: &[u8],
    mut flags: BvnrReadFlags,
    continue_past_failed: bool,
    max_document_size: u64,
    h: &mut Handlers<'_>,
) -> (bool, u64) {
    h.install(&mut flags);
    let mut opts = BvnrDocStreamOpts {
        flags: &mut flags,
        userdata: flags.userdata,
        on_document: Some(on_document_trampoline as OnDocumentFn),
        continue_past_failed,
        max_document_size,
    };
    let mut src = BvnrSource {
        opaque: [0; OPAQUE_BYTES / 8],
    };
    let mut count = 0;
    // SAFETY: `data`, `flags`, `opts` and `h` outlive both calls, and nothing
    // else touches `h` meanwhile; the source only reads `data`.
    let ok = unsafe {
        bvnr_source_from_mem(&mut src, data.as_ptr().cast_mut().cast(), data.len() as u64);
        bvnr_doc_stream_read(&mut src, &mut opts, &mut count)
    };
    h.resume();
    (ok, count)
}

extern "C" fn on_document_trampoline(ud: *mut c_void, index: u64, ok: bool, err: c_int) -> bool {
    // SAFETY: `ud` is the installed `Handlers`.
    let h = unsafe { handlers(ud) };
    h.guard(false, |h| {
        h.on_document.as_mut().is_none_or(|f| f(index, ok, err))
    })
}

/// `bvnr_parse_embedded`: `h.verified` sees the inner document's events and
/// `h.first_error` records why it failed.
pub fn parse_embedded(inner: &[u8], mut flags: BvnrReadFlags, h: &mut Handlers<'_>) -> bool {
    h.install(&mut flags);
    // SAFETY: `inner`, `flags` and `h` outlive the call; `inner` is only read.
    let ok = unsafe {
        bvnr_parse_embedded(
            inner.as_ptr().cast_mut().cast(),
            inner.len() as u64,
            &mut flags,
        )
    };
    h.resume();
    ok
}

/// Why [`demux_read`] failed.
pub enum DemuxError {
    /// The demultiplexer lost sync (`bvnr_demux_error`).
    Demux(c_int),
    /// The reader refused the stream.
    Read(ReaderError),
    OutOfMemory,
}

extern "C" fn on_message_trampoline(
    ud: *mut c_void,
    ch: u64,
    data: *const c_void,
    len: u64,
) -> bool {
    // SAFETY: `ud` is the message vector `demux_read` keeps alive, and
    // `data` holds `len` bytes for the call.
    let (out, payload) = unsafe {
        let len = usize::try_from(len).unwrap_or(0);
        let payload = if data.is_null() || len == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(data.cast::<u8>(), len).to_vec()
        };
        (&mut *ud.cast::<Vec<(u64, Vec<u8>)>>(), payload)
    };
    out.push((ch, payload));
    true
}

/// A reader feeding `bvnr_demux_on_event`: the messages of a multiplexed
/// stream, in order.
pub fn demux_read(
    data: &[u8],
    mut flags: BvnrReadFlags,
    max_message: u64,
    key: Option<&str>,
) -> Result<Vec<(u64, Vec<u8>)>, DemuxError> {
    let key = key
        .map(CString::new)
        .transpose()
        .map_err(|_| DemuxError::OutOfMemory)?;
    let mut messages: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut reader = ReaderHandle::new().ok_or(DemuxError::OutOfMemory)?;
    // SAFETY: `messages` outlives the demux, which is destroyed below.
    let dm = unsafe {
        bvnr_demux_create(
            on_message_trampoline,
            (&raw mut messages).cast(),
            max_message,
        )
    };
    if dm.is_null() {
        return Err(DemuxError::OutOfMemory);
    }
    // SAFETY: `dm` is live; `key` is NUL-terminated.
    if key.is_some_and(|k| !unsafe { bvnr_demux_set_key(dm, k.as_ptr()) }) {
        // SAFETY: `dm` is live and destroyed once.
        unsafe { bvnr_demux_destroy(dm) };
        return Err(DemuxError::OutOfMemory);
    }
    let mut forward = |v: &EventView<'_>| {
        let d = v
            .data
            .map_or(std::ptr::null_mut(), |d| (d as *const BvnrData).cast_mut());
        // SAFETY: `dm` is live; `d` is the event's data, valid for the call
        // and only read by the demux.
        unsafe { bvnr_demux_on_event(dm, v.event, d) }
    };
    let mut h = Handlers {
        verified: Some(&mut forward),
        ..Handlers::default()
    };
    flags.max_file_size = 0;
    let ok = reader.read_mem(data, flags, &mut h);
    // SAFETY: `dm` is live; it is destroyed exactly once, after its last use.
    let derr = unsafe {
        let e = bvnr_demux_error(dm);
        bvnr_demux_destroy(dm);
        e
    };
    if derr != 0 {
        Err(DemuxError::Demux(derr))
    } else if !ok {
        Err(DemuxError::Read(reader.error()))
    } else {
        Ok(messages)
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
