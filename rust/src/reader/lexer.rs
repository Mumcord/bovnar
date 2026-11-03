//! The table-driven lexer (`src/lexer/bovnar_lexer.c`).
//!
//! A faithful port of the C reference lexer: the same states, the same
//! transition table ([`table`]), the same action handlers and the same calls
//! into the [`Validator`] in the same order, so a document is accepted or
//! refused at exactly the byte where the reference decides, with the same
//! error code and position.
//!
//! Two things differ in shape, not in behaviour:
//!
//! - The C lexer pulls its input from a source; this one is pushed bytes
//!   through [`Lexer::feed`] and must give identical results for any split of
//!   the input. The text scanner already works byte by byte, so the only part
//!   that needed a new form is the octet-stream reader, which in C reads its
//!   binary framing straight from the source. Here it is a small resumable
//!   state machine ([`OctetPhase`]) that buffers one chunk and hands it to the
//!   validator only once it is complete, as C does.
//! - The C bulk-copy fast paths (`bvn_try_bulk_run`, `bvn_try_bulk_ws`) are
//!   left out. They are designed to be indistinguishable from the per-byte
//!   path (same buffers, limits, line and column accounting), so only the
//!   per-byte path is ported.
//!
//! Action handlers keep the C convention on purpose: they return `false`
//! after storing the error in [`Validator::last_error`], and the byte loop
//! turns that into an error position or into recovery. That keeps every
//! handler comparable line by line with its C original.

mod table;

use alloc::vec::Vec;

use self::table::{Act, St};
use super::validator::{Ev, RawToken, Validator};
use crate::event::TokenKind;
use crate::types::{TypeFamily, TypeSpec};
use crate::units::Unit;
use crate::{Error, ErrorKind, Limits, SpecVersion};

/// `max_text_bytes`: the C reader's cap on text (non-octet) bytes. The
/// conformance protocol has no flag for it, so it stays at the C default.
const MAX_TEXT_BYTES: u64 = 2_147_483_647;
/// `BVN_TYPE_BUF_CAP - 1`: longest type annotation body.
const TYPE_CAP: usize = 255;
/// `BVN_INLINE_UNIT_BUF_CAP - 1`: longest inline unit.
const INLINE_UNIT_CAP: usize = 255;
/// Size of the version directive capture buffer (`ver_buf`).
const VER_BUF_CAP: usize = 32;

/// Where the octet-stream reader stands inside its binary framing
/// (`[0x01][len_lo][len_hi][payload]` chunks, ended by a `0x00` tag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OctetPhase {
    Tag,
    LenLo,
    LenHi(u8),
    /// Payload of the given length is being buffered.
    Payload(usize),
}

/// One saved array level (`bvn_array_frame_t`).
#[derive(Debug, Clone)]
struct Frame {
    saved_curr: u64,
    saved_row: u64,
    saved_vtype: TypeSpec,
    saved_vunit: Unit,
    saved_unit_serial: u64,
    /// `saved_vunit`'s spelling; it travels with the unit.
    saved_usrc: Vec<u8>,
    dim_row_size: u64,
    in_dim_seq: bool,
    /// Struct nesting level when this `[` was opened: a `;`, `,` or `]` only
    /// belongs to this array if no struct has been opened inside it since.
    open_struct_level: u8,
}

impl Frame {
    /// The zeroed frame C gets from `calloc` / `memset`.
    fn empty() -> Self {
        Self {
            saved_curr: 0,
            saved_row: 0,
            saved_vtype: TypeSpec::PLAIN,
            saved_vunit: Unit::no_unit(),
            saved_unit_serial: 0,
            saved_usrc: Vec::new(),
            dim_row_size: 0,
            in_dim_seq: false,
            open_struct_level: 0,
        }
    }
}

/// Outcome of parsing the first comment as a version directive.
enum VerDir {
    None,
    Invalid,
    Ok(u16, u16),
}

/// The lexer (`bvnr_lexer_t`).
pub(crate) struct Lexer {
    byte: u8,
    prev_byte: u8,
    token: TokenKind,
    state: St,
    last_state: St,
    struct_nesting_level: u8,
    array_items: u64,
    array_row_size: u64,
    curr_row_size: u64,
    in_array_element: bool,
    array_nesting_level: u8,
    arr_used_max: u16,
    frames: Vec<Frame>,
    /// Bytes accepted from `feed` so far (`processed_bytes`).
    processed_bytes: u64,
    text_bytes: u64,
    /// Bytes consumed by the octet-stream reader; `text_bytes + octet_bytes`
    /// is the stream offset reached.
    octet_bytes: u64,
    octet_phase: OctetPhase,
    octet_buf: Vec<u8>,
    utf8_need: u8,
    utf8_lo: u8,
    utf8_hi: u8,
    bom_len: u8,
    bom: [u8; 2],
    type_family_colon: bool,
    type_param_ns: bool,
    type_kw_end: u64,
    str_data: Vec<u8>,
    type_data: Vec<u8>,
    inline_unit: Vec<u8>,
    line: u64,
    column: u64,
    max_identifier_length: u16,
    max_string_length: u16,
    max_number_length: u16,
    max_symbol_length: u16,
    max_reference_length: u16,
    max_array_items: u64,
    /// `max_file_size`; 0 means unlimited, as in C.
    max_file_size: u64,
    max_struct_nesting: u8,
    max_array_nesting: u8,
    continue_on_error: bool,
    /// `strict_version` and `text_only` of `bvnr_read_flags_t`; off by
    /// default as in C, set through [`Lexer::set_policy`].
    strict_version: bool,
    text_only: bool,
    resync_array_depth: u32,
    resync_struct_depth: u32,
    resync_saved_struct_nesting: u8,
    recovery_count: u64,
    skipped_bytes: u64,
    resync_start_bytes: u64,
    ver_checked: bool,
    ver_overflow: bool,
    ver_len: u8,
    ver_buf: [u8; VER_BUF_CAP],
    esc_x_hi: u8,
    esc_u_digits: u8,
    str_has_raw_escape: bool,
    esc_u_acc: u32,
    /// `ev_stream_start` has been handed to the validator.
    started: bool,
    /// The error that ended the document; every later call repeats it.
    failed: Option<Error>,
}

/// Picks a C limit: 0 means "use the compiled-in default".
fn or_default<T: PartialEq + Default>(value: T, default: T) -> T {
    if value == T::default() {
        default
    } else {
        value
    }
}

/// The next tab stop (multiples of 4), as C advances the column on `\t`.
fn tab_stop(column: u64) -> u64 {
    ((column >> 2).saturating_add(1)) << 2
}

/// `bvn_utf8_classify_leader`: continuation count and the valid range of the
/// first continuation byte, or `None` for bytes that cannot start a sequence.
/// The ranges reject overlong forms, surrogates and code points past U+10FFFF.
fn utf8_classify_leader(b: u8) -> Option<(u8, u8, u8)> {
    match b {
        0x00..=0x7f => Some((0, 0, 0)),
        0x80..=0xc1 => None,
        0xc2..=0xdf => Some((1, 0x80, 0xbf)),
        0xe0 => Some((2, 0xa0, 0xbf)),
        0xed => Some((2, 0x80, 0x9f)),
        0xe1..=0xef => Some((2, 0x80, 0xbf)),
        0xf0 => Some((3, 0x90, 0xbf)),
        0xf1..=0xf3 => Some((3, 0x80, 0xbf)),
        0xf4 => Some((3, 0x80, 0x8f)),
        0xf5..=0xff => None,
    }
}

/// `bvn_utf8_feed_st`: one byte of incremental UTF-8 validation.
fn utf8_feed_st(st: &mut (u8, u8, u8), b: u8) -> bool {
    let (need, lo, hi) = *st;
    if need != 0 {
        if b < lo || b > hi {
            *st = (0, 0, 0);
            return false;
        }
        *st = (need - 1, 0x80, 0xbf);
        return true;
    }
    match utf8_classify_leader(b) {
        Some(s) => {
            *st = s;
            true
        }
        None => false,
    }
}

/// `bvn_validate_string`: the whole buffer is well-formed UTF-8.
fn validate_utf8(data: &[u8]) -> bool {
    let mut st = (0, 0, 0);
    data.iter().all(|&b| utf8_feed_st(&mut st, b)) && st.0 == 0
}

/// `bvn_hexval`.
fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// `bvn_is_string_ctrl_reject`: control bytes a string may not hold, whether
/// raw or produced by an escape (the whitespace controls stay allowed).
fn is_string_ctrl_reject(b: u32) -> bool {
    b <= 0x08 || (0x0e..=0x1f).contains(&b) || b == 0x7f
}

/// `bvn_escape_lut`: the named escapes.
fn decode_named_escape(b: u8) -> Option<u8> {
    match b {
        b't' => Some(b'\t'),
        b'n' => Some(b'\n'),
        b'v' => Some(0x0b),
        b'f' => Some(0x0c),
        b'r' => Some(b'\r'),
        b'"' => Some(b'"'),
        b'\\' => Some(b'\\'),
        _ => None,
    }
}

/// `bvn_type_param_depth`: brace/bracket depth of the annotation bytes so far.
/// A separator inside `{...}` or `[...]` is part of a profile code.
fn type_param_depth(d: &[u8]) -> u32 {
    let mut depth: u32 = 0;
    for &c in d {
        if c == b'{' || c == b'[' {
            depth = depth.saturating_add(1);
        } else if (c == b'}' || c == b']') && depth != 0 {
            depth -= 1;
        }
    }
    depth
}

/// `bvn_parse_version_directive`: the bytes after the leading `#`, e.g.
/// `!bovnar 1.1`. Grammar: `"!bovnar" ws+ uint "." uint ws*`, each uint
/// decimal without leading zero and fitting in 16 bits.
fn parse_version_directive(s: &[u8]) -> VerDir {
    let Some(rest) = s.strip_prefix(b"!bovnar") else {
        return VerDir::None;
    };
    let is_ws = |c: &u8| *c == b' ' || *c == b'\t';
    // The prefix must be followed by whitespace, else it is another word.
    if !rest.first().is_some_and(is_ws) {
        return VerDir::None;
    }
    let mut it = rest.iter().skip_while(|c| is_ws(c)).peekable();
    let mut out = [0u16; 2];
    for (part, slot) in out.iter_mut().enumerate() {
        let Some(&first) = it.peek() else {
            return VerDir::Invalid;
        };
        if !first.is_ascii_digit() {
            return VerDir::Invalid;
        }
        let leading_zero = *first == b'0';
        let mut acc: u32 = 0;
        let mut digits = 0u32;
        while let Some(&c) = it.peek().filter(|c| c.is_ascii_digit()) {
            acc = acc * 10 + u32::from(c - b'0');
            if acc > 0xffff {
                return VerDir::Invalid;
            }
            digits += 1;
            it.next();
        }
        if leading_zero && digits > 1 {
            return VerDir::Invalid;
        }
        *slot = u16::try_from(acc).unwrap_or(u16::MAX);
        if part == 0 && it.next() != Some(&b'.') {
            return VerDir::Invalid;
        }
    }
    if it.any(|c| !is_ws(c)) {
        return VerDir::Invalid;
    }
    VerDir::Ok(out[0], out[1])
}

impl Lexer {
    /// `bvn_lex_init`: limits left at 0 fall back to the C defaults, except
    /// `max_input_bytes`, where 0 means unlimited like C's `max_file_size`.
    pub(crate) fn new(limits: &Limits, continue_on_error: bool) -> Self {
        let max_array_nesting = or_default(limits.max_array_nesting, 64);
        let mut frames = Vec::new();
        frames.resize_with(usize::from(max_array_nesting) + 1, Frame::empty);
        Self {
            byte: 0,
            prev_byte: 0,
            token: TokenKind::Unknown,
            state: St::Undefined,
            last_state: St::Undefined,
            struct_nesting_level: 0,
            array_items: 0,
            array_row_size: 0,
            curr_row_size: 0,
            in_array_element: false,
            array_nesting_level: 0,
            arr_used_max: 0,
            frames,
            processed_bytes: 0,
            text_bytes: 0,
            octet_bytes: 0,
            octet_phase: OctetPhase::Tag,
            octet_buf: Vec::new(),
            utf8_need: 0,
            utf8_lo: 0,
            utf8_hi: 0,
            bom_len: 0,
            bom: [0; 2],
            type_family_colon: false,
            type_param_ns: false,
            type_kw_end: 0,
            str_data: Vec::new(),
            type_data: Vec::new(),
            inline_unit: Vec::new(),
            line: 1,
            column: 0,
            max_identifier_length: or_default(limits.max_identifier_length, 255),
            max_string_length: or_default(limits.max_string_length, 65_535),
            max_number_length: or_default(limits.max_number_length, 65_535),
            max_symbol_length: or_default(limits.max_symbol_length, 255),
            max_reference_length: or_default(limits.max_reference_length, 65_535),
            max_array_items: or_default(limits.max_array_items, 2_147_483_647),
            max_file_size: limits.max_input_bytes,
            max_struct_nesting: or_default(limits.max_struct_nesting, 64),
            max_array_nesting,
            continue_on_error,
            strict_version: false,
            text_only: false,
            resync_array_depth: 0,
            resync_struct_depth: 0,
            resync_saved_struct_nesting: 0,
            recovery_count: 0,
            skipped_bytes: 0,
            resync_start_bytes: 0,
            ver_checked: false,
            ver_overflow: false,
            ver_len: 0,
            ver_buf: [0; VER_BUF_CAP],
            esc_x_hi: 0,
            esc_u_digits: 0,
            str_has_raw_escape: false,
            esc_u_acc: 0,
            started: false,
            failed: None,
        }
    }

    /// Errors recovered from so far (`bvnr_reader_get_recovery_count`).
    pub(crate) fn recovery_count(&self) -> u64 {
        self.recovery_count
    }

    /// Text bytes discarded while recovering (`bvnr_reader_get_skipped_bytes`).
    pub(crate) fn skipped_bytes(&self) -> u64 {
        self.skipped_bytes
    }

    /// Sets the C reader's `strict_version` and `text_only` flags.
    pub(crate) fn set_policy(&mut self, strict_version: bool, text_only: bool) {
        self.strict_version = strict_version;
        self.text_only = text_only;
    }

    /// Lexes the next bytes of the document (whole slice); emits
    /// `ev_stream_start` before the first byte.
    pub(crate) fn feed(&mut self, val: &mut Validator, input: &[u8]) -> Result<(), Error> {
        self.start(val)?;
        for &b in input {
            // max_file_size: everything up to the cap is read, the first byte
            // over it is refused, whatever the chunking (see bvn_lex_run).
            if self.max_file_size != 0 && self.processed_bytes == self.max_file_size {
                self.set_eof_error(val, ErrorKind::FileTooLong);
                return Err(self.fail(val));
            }
            self.processed_bytes += 1;
            if self.state == St::OctetStreamIntro {
                self.octet_byte(val, b)?;
            } else {
                self.text_byte(val, b)?;
                if self.state == St::OctetStreamIntro {
                    // The binary framing takes over from the next byte on.
                    self.octet_phase = OctetPhase::Tag;
                    self.octet_buf.clear();
                }
            }
        }
        Ok(())
    }

    /// End of input: C's EOF handling, including `ev_stream_end`.
    pub(crate) fn finish(&mut self, val: &mut Validator) -> Result<(), Error> {
        self.start(val)?;
        if self.state == St::OctetStreamIntro {
            // The octet reader's pull came back empty.
            self.capture_os_error(
                val,
                ErrorKind::ReadCompleteChunkFailed,
                self.processed_bytes,
            );
            return Err(self.fail(val));
        }
        let eof_in_first_comment = matches!(
            self.state,
            St::FirstCommentIntro | St::FirstCommentAfterEf | St::FirstCommentAfterEfBb
        );
        let in_comment_after_value = matches!(self.state, St::CommentIntro | St::IgnoreCommentByte)
            && self.last_state == St::ValueOutro;
        let eof_acceptable = (matches!(self.state, St::ValueOutro | St::Undefined | St::FirstBom)
            || eof_in_first_comment
            || in_comment_after_value)
            && self.struct_nesting_level == 0;
        // A comment may run to the end of input; a multi-byte sequence cut
        // short there is refused like one cut short by a newline.
        if eof_acceptable && self.utf8_need != 0 {
            self.set_eof_error(val, ErrorKind::InvalidUtf8Byte);
            if !self.continue_on_error {
                return Err(self.fail(val));
            }
            val.notify_recovered();
            self.utf8_need = 0;
            self.recovery_count = self.recovery_count.saturating_add(1);
            if eof_in_first_comment {
                self.state = St::FirstCommentIntro;
            }
        }
        if matches!(
            self.state,
            St::ValueOutro | St::Undefined | St::FirstBom | St::FirstCommentIntro
        ) && self.struct_nesting_level == 0
        {
            // A comment-only document ending without a newline may still be
            // the version directive, which only comment_outro parses.
            if self.state == St::FirstCommentIntro && !self.first_comment_outro(val) {
                let err = val.last_error.unwrap_or(ErrorKind::InvalidSpecVersion);
                self.set_eof_error(val, err);
                return Err(self.fail(val));
            }
            val.last_error = None;
        } else if matches!(self.state, St::CommentIntro | St::IgnoreCommentByte)
            && self.last_state == St::ValueOutro
            && self.struct_nesting_level == 0
        {
            val.last_error = None;
        } else {
            if matches!(
                self.state,
                St::Resync
                    | St::ResyncDot
                    | St::ResyncString
                    | St::ResyncStringEscape
                    | St::ResyncComment
            ) {
                // Recovery that never found a boundary skipped everything
                // from the error to the end.
                self.count_skipped();
            }
            self.set_eof_error(val, ErrorKind::GotIncompleteBvnrStream);
            return Err(self.fail(val));
        }
        if !val.receive_event(Ev::StreamEnd) {
            return Err(self.fail(val));
        }
        Ok(())
    }

    /// Emits `ev_stream_start` once, and repeats an earlier fatal error.
    fn start(&mut self, val: &mut Validator) -> Result<(), Error> {
        if let Some(e) = self.failed {
            return Err(e);
        }
        if !self.started {
            self.started = true;
            if !val.receive_event(Ev::StreamStart) {
                return Err(self.fail(val));
            }
        }
        Ok(())
    }

    /// Turns the validator's error state into the returned error and makes it
    /// sticky.
    fn fail(&mut self, val: &Validator) -> Error {
        // Every failing path stores a code first; the fallback only guards
        // against a validator method that returns false without one.
        let kind = val.last_error.unwrap_or(ErrorKind::ScannerCallbackFailed);
        let e = Error::new(kind, val.error_line, val.error_column, val.error_offset);
        self.failed = Some(e);
        e
    }

    // ---------------------------------------------------------------------
    // Error positions
    // ---------------------------------------------------------------------

    /// `bvn_set_error_pos`.
    fn set_error_pos(&self, val: &mut Validator, byte: u8, offset: u64) {
        val.error_line = self.line;
        val.error_column = self.column;
        val.error_byte = u32::from(byte);
        val.error_offset = offset;
    }

    /// `bvn_last_text_offset`: stream offset of the last text byte consumed.
    fn last_text_offset(&self) -> u64 {
        (self.text_bytes + self.octet_bytes).saturating_sub(1)
    }

    /// `bvn_lexer_set_error`.
    fn set_error(&self, val: &mut Validator, err: ErrorKind) {
        val.last_error = Some(err);
        self.set_error_pos(val, self.byte, self.last_text_offset());
    }

    /// `bvn_set_eof_error`.
    fn set_eof_error(&self, val: &mut Validator, err: ErrorKind) {
        val.last_error = Some(err);
        self.set_error_pos(val, 0, self.processed_bytes);
    }

    /// `bvn_capture_os_error`; `offset` is the position the octet reader
    /// has reached.
    fn capture_os_error(&self, val: &mut Validator, err: ErrorKind, offset: u64) {
        val.last_error = Some(err);
        self.set_error_pos(val, 0, offset);
    }

    fn count_skipped(&mut self) {
        if self.text_bytes > self.resync_start_bytes {
            self.skipped_bytes = self
                .skipped_bytes
                .saturating_add(self.text_bytes - self.resync_start_bytes);
        }
    }

    // ---------------------------------------------------------------------
    // The byte loop (bvn_interpret_input_buffer, per-byte path)
    // ---------------------------------------------------------------------

    fn utf8_feed(&mut self, b: u8) -> bool {
        let mut st = (self.utf8_need, self.utf8_lo, self.utf8_hi);
        let ok = utf8_feed_st(&mut st, b);
        (self.utf8_need, self.utf8_lo, self.utf8_hi) = st;
        ok
    }

    /// `bvn_advance_line`: CR, LF and CRLF each end one line.
    fn advance_line(&mut self, prev: u8) {
        if self.byte == 0x0d {
            self.line = self.line.saturating_add(1);
            self.column = 0;
        } else if self.byte == 0x0a {
            if prev != 0x0d {
                self.line = self.line.saturating_add(1);
            }
            self.column = 0;
        }
    }

    fn advance_column(&mut self, saved_need: u8) {
        if self.byte == 0x09 {
            self.column = tab_stop(self.column);
        } else if saved_need == 0 {
            self.column = self.column.saturating_add(1);
        }
    }

    /// One text byte: byte cap, UTF-8, line/column, then the byte's action,
    /// with recovery when `continue_on_error` is set.
    fn text_byte(&mut self, val: &mut Validator, b: u8) -> Result<(), Error> {
        if self.text_bytes == MAX_TEXT_BYTES {
            let col = if b == 0x09 {
                tab_stop(self.column)
            } else if self.utf8_need == 0 {
                self.column.saturating_add(1)
            } else {
                self.column
            };
            val.last_error = Some(ErrorKind::TextDataTooLong);
            val.error_line = self.line;
            val.error_column = col;
            val.error_byte = u32::from(b);
            val.error_offset = self.text_bytes + self.octet_bytes;
            return Err(self.fail(val));
        }
        self.text_bytes += 1;
        self.byte = b;
        let prev = self.prev_byte;
        self.prev_byte = b;
        let saved_need = self.utf8_need;
        let mut line_advanced = false;
        if !(b < 0x80 && saved_need == 0) && !self.utf8_feed(b) {
            self.advance_column(saved_need);
            val.last_error = Some(ErrorKind::InvalidUtf8Byte);
            self.set_error_pos(val, b, self.last_text_offset());
            self.advance_line(prev);
            line_advanced = true;
            if !self.continue_on_error {
                return Err(self.fail(val));
            }
            val.notify_recovered();
            let was_continuation = saved_need > 0;
            if self.recover_in_comment(val, b == 0x0a || b == 0x0d) {
                if !was_continuation || b == 0x0a || b == 0x0d {
                    return Ok(());
                }
                // The byte that cut the sequence short starts a new
                // character of the comment; fall through to it.
            } else if !self.enter_resync(val) {
                return Err(self.fail(val));
            }
            if !was_continuation || b == 0x09 || b == 0x0a || b == 0x0d {
                return Ok(());
            }
            if !self.utf8_feed(b) {
                self.column = self.column.saturating_add(1);
                return Ok(());
            }
            self.column = self.column.saturating_add(1);
        }
        self.advance_column(saved_need);
        if self.call_action(val) {
            if !line_advanced {
                self.advance_line(prev);
            }
            return Ok(());
        }
        self.set_error_pos(val, b, self.last_text_offset());
        if !line_advanced {
            self.advance_line(prev);
        }
        // A consumer that returned false asked to abort; that is not a
        // document error to recover from.
        if val.last_error == Some(ErrorKind::ScannerCallbackFailed) || !self.continue_on_error {
            return Err(self.fail(val));
        }
        val.notify_recovered();
        if self.recover_in_comment(val, false) {
            return Ok(());
        }
        if !self.enter_resync(val) {
            return Err(self.fail(val));
        }
        if self.state == St::Resync && b == b';' {
            // A semantic error found at the value's own ';' leaves the
            // stream at an assignment boundary already: resume right here
            // instead of swallowing the next assignment. C ignores the
            // result of this reset, and so does the port.
            let _ = self.resync_semicolon_reset(val);
        } else if self.state == St::Resync && b == b'}' && !self.resync_close_bracket(val) {
            // The offending '}' still closes its struct.
            return Err(self.fail(val));
        }
        Ok(())
    }

    /// `bvn_call_action_for_new_byte`: look up the action for (state, byte)
    /// and run its handler (`bvn_action_table`).
    fn call_action(&mut self, val: &mut Validator) -> bool {
        let act = table::action(self.state, self.byte);
        match act {
            Act::IgnoreWhitespace => true,
            Act::None => {
                self.set_error(val, ErrorKind::UnexpectedInputByte);
                false
            }
            // Pure transitions (bvn_action_set_state).
            Act::IgnoreCommentByte
            | Act::ToIdentifierOutro
            | Act::ToNumberOutro
            | Act::TfToU
            | Act::TfToS
            | Act::TfToF
            | Act::TfToB
            | Act::TfToD
            | Act::TfUToUi
            | Act::TfUToUt
            | Act::StringOutro
            | Act::EscapeFromCopy
            | Act::ToSymbolOutro
            | Act::ToReferenceOutro
            | Act::ToStringOutro => {
                self.state = table::target(act);
                true
            }
            Act::CommentIntro => {
                self.last_state = self.state;
                self.state = St::CommentIntro;
                true
            }
            Act::CommentOutro => {
                self.comment_outro();
                true
            }
            Act::CopyUtf8bomByte => self.copy_utf8bom_byte(val),
            Act::IdentifierIntro => {
                self.str_data.clear();
                self.token = TokenKind::Identifier;
                self.state = St::IdentifierIntro;
                true
            }
            Act::CopyIdentifierByte => self.push_str(
                val,
                self.max_identifier_length,
                ErrorKind::IdentifierTooLong,
                St::IdentifierBody,
            ),
            Act::ValueIntro => self.value_intro(val),
            Act::ValueOutro => self.value_outro(val),
            Act::TypeIntro => {
                self.token = TokenKind::Type;
                self.type_family_colon = false;
                self.type_param_ns = false;
                self.state = St::TypeIntro;
                true
            }
            Act::TypeOutro => {
                self.type_trim_trailing_ws();
                if !self.finalize(val) {
                    return false;
                }
                self.state = St::TypeOutro;
                true
            }
            Act::CopyTypeByte => self.copy_type_byte(val),
            Act::CopyTypeSepByte => self.copy_type_sep_byte(val),
            Act::TypeWsByte => self.type_ws_byte(val),
            Act::TypeParamWhitespace => {
                self.set_error(val, ErrorKind::TypeParamWhitespace);
                false
            }
            Act::NegNumberIntro => {
                self.number_begin();
                self.push_number_byte(val, St::NegNumberIntro)
            }
            Act::CopyNumberByte => {
                self.number_begin();
                self.push_number_byte(val, St::CopyNumberByte)
            }
            Act::ZeroIntro => {
                self.number_begin();
                self.push_number_byte(val, St::ZeroIntro)
            }
            Act::FractionIntro => {
                self.number_begin();
                val.acc.has_dot = true;
                self.push_number_byte(val, St::FractionIntro)
            }
            Act::FractionNoInt => {
                self.number_begin();
                val.acc.has_dot = true;
                self.push_number_byte(val, St::FractionNoInt)
            }
            Act::CopyFractionByte => self.push_number_byte(val, St::CopyFractionByte),
            Act::KwAdvance => {
                self.state = table::kw_advance(self.state);
                true
            }
            Act::TfUintDone => self.tf_family_done(b"uint"),
            Act::TfSintDone => self.tf_family_done(b"sint"),
            Act::TfFloatDone => {
                // Where the keyword ended: "_fix" / "_dec" must follow it
                // directly (see copy_type_byte).
                self.type_kw_end = self.text_bytes;
                self.tf_family_done(b"float")
            }
            Act::TfUtf8Done => self.tf_family_done(b"utf8"),
            Act::TfBoolDone => self.tf_family_done(b"bool"),
            Act::TfDatetimeDone => self.tf_family_done(b"datetime"),
            Act::ArrayIntro => self.array_intro(val),
            Act::ArrayOutro => self.array_outro(val),
            Act::ArrayOutroEmpty => self.array_outro_empty(val),
            Act::NewArrayValue => self.new_array_value(val),
            Act::ArrayDimSep => {
                if !val.receive_event(Ev::ArrayDimStart) {
                    return false;
                }
                let level = usize::from(self.array_nesting_level);
                self.frame(level).in_dim_seq = true;
                self.state = St::ArrayDimSep;
                true
            }
            Act::ExpIntro => {
                self.number_begin();
                val.acc.has_exp = true;
                self.push_number_byte(val, St::ExpIntro)
            }
            Act::ExpSignIntro => self.push_number_byte(val, St::ExpSignIntro),
            Act::CopyExpByte => self.push_number_byte(val, St::CopyExpByte),
            Act::DtlitAfterY
            | Act::DtlitMonth
            | Act::DtlitAfterM
            | Act::DtlitDay
            | Act::DtlitAfterT
            | Act::DtlitHour
            | Act::DtlitAfterH
            | Act::DtlitMin
            | Act::DtlitAfterMi
            | Act::DtlitSec
            | Act::DtlitZulu
            | Act::DtlitFrac
            | Act::DtlitOffSign
            | Act::DtlitOffHour
            | Act::DtlitOffColon
            | Act::DtlitOffMin => {
                // A datetime literal accumulates like a number (same buffer,
                // same cap); the validator recognises the ISO shape.
                self.push_number_byte(val, table::target(act))
            }
            Act::StringIntro => {
                self.token = if self.in_array_element {
                    TokenKind::ArrayString
                } else {
                    TokenKind::String
                };
                self.state = St::StringIntro;
                true
            }
            Act::CopyStringByte => self.push_str(
                val,
                self.max_string_length,
                ErrorKind::StringTooLong,
                St::CopyStringByte,
            ),
            Act::ReplaceEscapedByte => self.replace_escaped_byte(val),
            Act::EscapeXIntro => self.escape_intro(val, St::EscX1),
            Act::EscapeUIntro => self.escape_intro(val, St::EscUOpen),
            Act::EscapeXD1 => self.escape_x_d1(val),
            Act::EscapeXD2 => self.escape_x_d2(val),
            Act::EscapeUOpen => {
                if self.byte != b'{' {
                    self.set_error(val, ErrorKind::IllegalEscapeSequence);
                    return false;
                }
                self.esc_u_acc = 0;
                self.esc_u_digits = 0;
                self.state = St::EscUHex;
                true
            }
            Act::EscapeUHex => self.escape_u_hex(val),
            Act::ArrStringIntro => {
                self.token = TokenKind::ArrayString;
                self.state = St::StringIntro;
                true
            }
            Act::SymbolIntro => {
                self.str_data.clear();
                self.token = TokenKind::Symbol;
                self.push_str(
                    val,
                    self.max_symbol_length,
                    ErrorKind::SymbolTooLong,
                    St::SymbolBody,
                )
            }
            Act::CopySymbolByte => self.push_str(
                val,
                self.max_symbol_length,
                ErrorKind::SymbolTooLong,
                St::SymbolBody,
            ),
            Act::ReferenceIntro => {
                self.str_data.clear();
                self.token = TokenKind::Reference;
                self.state = St::ReferenceIntro;
                true
            }
            Act::CopyReferenceDot => self.push_reference_byte(val, St::ReferenceSegmentIntro),
            Act::CopyReferenceByte => {
                // After an index only '.', '[' or the end may follow; a ']'
                // in the stored path is always an index close.
                if self.str_data.last() == Some(&b']') {
                    self.set_error(val, ErrorKind::UnexpectedInputByte);
                    return false;
                }
                self.push_reference_byte(val, St::ReferenceSegmentBody)
            }
            Act::ReferenceIndexOpen => {
                // Indexing in a reference is spec 1.1 syntax.
                if !val.supports_1_1() {
                    self.set_error(val, ErrorKind::UnexpectedInputByte);
                    return false;
                }
                self.push_reference_byte(val, St::ReferenceIndexFirst)
            }
            Act::ReferenceIndexByte => self.push_reference_byte(val, St::ReferenceIndex),
            Act::ReferenceIndexClose => self.push_reference_byte(val, St::ReferenceSegmentBody),
            Act::OctetStreamIntro => {
                // Refused at the opening 0x00, before any payload is read.
                if self.text_only {
                    self.set_error(val, ErrorKind::OctetStreamForbidden);
                    return false;
                }
                self.token = TokenKind::OctetStream;
                self.state = St::OctetStreamIntro;
                val.receive_event(Ev::OctetStreamStart)
            }
            Act::StructIntro => self.struct_intro(val),
            Act::StructOutro => self.struct_outro(val),
            Act::FirstCommentIntro => {
                self.last_state = self.state;
                self.state = St::FirstCommentIntro;
                if !self.ver_checked {
                    self.ver_len = 0;
                    self.ver_overflow = false;
                }
                true
            }
            Act::FirstCommentByte => {
                self.first_comment_capture();
                self.state = if self.byte == 0xef {
                    St::FirstCommentAfterEf
                } else {
                    St::FirstCommentIntro
                };
                true
            }
            Act::FirstCommentAfterEf => {
                self.first_comment_capture();
                self.state = match self.byte {
                    0xbb => St::FirstCommentAfterEfBb,
                    0xef => St::FirstCommentAfterEf,
                    _ => St::FirstCommentIntro,
                };
                true
            }
            Act::FirstCommentAfterEfBb => {
                // A BOM is legal only at offset 0, not inside the comment.
                if self.byte == 0xbf {
                    self.set_error(val, ErrorKind::InvalidByteOrderMark);
                    return false;
                }
                self.first_comment_capture();
                self.state = if self.byte == 0xef {
                    St::FirstCommentAfterEf
                } else {
                    St::FirstCommentIntro
                };
                true
            }
            Act::FirstCommentOutro => self.first_comment_outro(val),
            Act::TypeNullThenValueOutro => self.null_then(val) && self.value_outro(val),
            Act::TypeNullThenNewArrayValue => self.null_then(val) && self.new_array_value(val),
            Act::TypeNullThenArrayOutro => self.null_then(val) && self.array_outro(val),
            Act::ResyncSkip => {
                self.state = St::Resync;
                true
            }
            Act::ResyncOpenBracket => {
                if self.byte == b'[' {
                    self.resync_array_depth = self.resync_array_depth.saturating_add(1);
                } else {
                    self.resync_struct_depth = self.resync_struct_depth.saturating_add(1);
                }
                self.state = St::Resync;
                true
            }
            Act::ResyncCloseBracket => self.resync_close_bracket(val),
            Act::ResyncSemicolon => {
                if self.resync_array_depth > 0 || self.resync_struct_depth > 0 {
                    self.state = St::Resync;
                    return true;
                }
                self.resync_semicolon_reset(val)
            }
            Act::ResyncStringIntro | Act::ResyncStringByte | Act::ResyncStringEscapeByte => {
                self.state = St::ResyncString;
                true
            }
            Act::ResyncStringEscape => {
                self.state = St::ResyncStringEscape;
                true
            }
            Act::ResyncStringOutro | Act::ResyncCommentOutro => {
                self.state = St::Resync;
                true
            }
            Act::ResyncCommentIntro | Act::ResyncCommentByte => {
                self.state = St::ResyncComment;
                true
            }
            Act::ResyncDot => {
                // Only at the recovery-relative top level can a '.' begin
                // the next assignment.
                self.state = if self.resync_array_depth > 0 || self.resync_struct_depth > 0 {
                    St::Resync
                } else {
                    St::ResyncDot
                };
                true
            }
            Act::ResyncResumeIdentifier => {
                // Recovery ends at the start of the next assignment; this
                // byte is the identifier's first.
                if !self.resync_reset(val) {
                    return false;
                }
                self.str_data.clear();
                self.token = TokenKind::Identifier;
                self.push_str(
                    val,
                    self.max_identifier_length,
                    ErrorKind::IdentifierTooLong,
                    St::IdentifierBody,
                )
            }
            Act::InlineUnitIntro => {
                // Array elements take their unit from the annotation only.
                if self.in_array_element {
                    self.set_error(val, ErrorKind::UnexpectedInputByte);
                    return false;
                }
                self.inline_unit.clear();
                self.inline_unit.push(self.byte);
                self.state = St::InlineUnitBody;
                true
            }
            Act::CopyInlineUnitByte => {
                if self.inline_unit.len() == INLINE_UNIT_CAP {
                    self.set_error(val, ErrorKind::UnitTooLong);
                    return false;
                }
                self.inline_unit.push(self.byte);
                self.state = St::InlineUnitBody;
                true
            }
            Act::ToInlineUnitOutro => {
                self.state = St::InlineUnitOutro;
                true
            }
        }
    }

    // ---------------------------------------------------------------------
    // Token buffers
    // ---------------------------------------------------------------------

    /// `bvn_push_str`: append the current byte to the token buffer, bounded
    /// by the per-token limit.
    fn push_str(&mut self, val: &mut Validator, max: u16, overflow: ErrorKind, after: St) -> bool {
        if self.str_data.len() == usize::from(max) {
            self.set_error(val, overflow);
            return false;
        }
        self.str_data.push(self.byte);
        self.state = after;
        true
    }

    fn push_number_byte(&mut self, val: &mut Validator, after: St) -> bool {
        self.push_str(val, self.max_number_length, ErrorKind::NumberTooLong, after)
    }

    fn push_reference_byte(&mut self, val: &mut Validator, after: St) -> bool {
        self.push_str(
            val,
            self.max_reference_length,
            ErrorKind::ReferenceTooLong,
            after,
        )
    }

    /// `bvn_string_push_byte`: a byte produced by an escape.
    fn string_push_byte(&mut self, val: &mut Validator, b: u8) -> bool {
        if self.str_data.len() == usize::from(self.max_string_length) {
            self.set_error(val, ErrorKind::StringTooLong);
            return false;
        }
        self.str_data.push(b);
        true
    }

    /// `bvn_lex_number_begin`: the first number byte fixes the token type.
    fn number_begin(&mut self) {
        if self.str_data.is_empty() {
            self.token = if self.in_array_element {
                TokenKind::ArrayNumber
            } else {
                TokenKind::Number
            };
        }
    }

    /// `bvn_lex_finalize`: hand the finished token to the validator.
    fn finalize(&mut self, val: &mut Validator) -> bool {
        let tt = self.token;
        if tt == TokenKind::Structure {
            self.token = TokenKind::Unknown;
            return true;
        }
        if tt == TokenKind::Unknown {
            return true;
        }
        if tt == TokenKind::Identifier && self.str_data.is_empty() {
            self.set_error(val, ErrorKind::EmptyIdentifier);
            return false;
        }
        // A \x escape can emit any byte; the raw-input UTF-8 check never saw
        // it, so the assembled string is checked once more.
        if self.str_has_raw_escape
            && (tt == TokenKind::String || tt == TokenKind::ArrayString)
            && !validate_utf8(&self.str_data)
        {
            self.set_error(val, ErrorKind::InvalidUtf8Byte);
            return false;
        }
        let raw = RawToken {
            kind: tt,
            event: if tt == TokenKind::Identifier {
                Ev::AssignmentStart
            } else {
                Ev::Data
            },
            data: &self.str_data,
            type_data: &self.type_data,
            inline_unit: &self.inline_unit,
        };
        if !val.receive(&raw) {
            return false;
        }
        self.type_data.clear();
        self.str_data.clear();
        self.inline_unit.clear();
        self.str_has_raw_escape = false;
        self.token = TokenKind::Unknown;
        true
    }

    /// The `type_null_then_*` prefix: a pending annotation without a value
    /// is a null value.
    fn null_then(&mut self, val: &mut Validator) -> bool {
        self.token = TokenKind::Null;
        self.str_data.clear();
        self.finalize(val)
    }

    // ---------------------------------------------------------------------
    // Comments, BOM, version directive
    // ---------------------------------------------------------------------

    fn comment_outro(&mut self) {
        // number_outro_nosp is vestigial in C; the mapping is kept as is.
        self.state = match self.last_state {
            St::NumberOutroNosp => St::NumberOutro,
            St::StringOutroNosp => St::StringOutro,
            s => s,
        };
    }

    fn copy_utf8bom_byte(&mut self, val: &mut Validator) -> bool {
        if let Some(slot) = self.bom.get_mut(usize::from(self.bom_len)) {
            *slot = self.byte;
            if self.bom_len == 0 {
                self.last_state = self.state;
            }
            self.bom_len += 1;
            self.state = St::Utf8bomIntro;
            return true;
        }
        // Spec 3.2: the BOM is legal only at byte offset 0; its own three
        // bytes are already counted in text_bytes.
        if self.bom != [0xef, 0xbb]
            || self.byte != 0xbf
            || self.last_state != St::Undefined
            || self.text_bytes != 3
        {
            self.set_error(val, ErrorKind::InvalidByteOrderMark);
            return false;
        }
        self.bom_len = 0;
        self.state = St::FirstBom;
        true
    }

    /// `bvn_first_comment_capture`: keep the first comment's bytes for the
    /// version directive. A run of blanks is kept as its first byte only, so
    /// trailing blanks the grammar allows cannot overflow the buffer.
    fn first_comment_capture(&mut self) {
        if self.ver_checked {
            return;
        }
        let is_blank = |c: u8| c == b' ' || c == b'\t';
        if is_blank(self.byte) && !self.ver_overflow {
            let last = usize::from(self.ver_len)
                .checked_sub(1)
                .and_then(|i| self.ver_buf.get(i));
            if last.is_some_and(|&c| is_blank(c)) {
                return;
            }
        }
        if let Some(slot) = self.ver_buf.get_mut(usize::from(self.ver_len)) {
            *slot = self.byte;
            self.ver_len += 1;
        } else {
            self.ver_overflow = true;
        }
    }

    fn first_comment_outro(&mut self, val: &mut Validator) -> bool {
        self.state = St::FirstBom;
        if self.ver_checked {
            return true;
        }
        self.ver_checked = true;
        let captured = self.ver_buf.get(..usize::from(self.ver_len)).unwrap_or(&[]);
        let r = parse_version_directive(captured);
        // An overflowed capture that still starts with the directive prefix
        // is an over-long directive; otherwise it is a long comment.
        let (major, minor) = match r {
            VerDir::None => return true,
            _ if self.ver_overflow => {
                self.set_error(val, ErrorKind::InvalidSpecVersion);
                return false;
            }
            VerDir::Invalid => {
                self.set_error(val, ErrorKind::InvalidSpecVersion);
                return false;
            }
            VerDir::Ok(major, minor) => (major, minor),
        };
        val.declared_version = Some(SpecVersion { major, minor });
        let supported = major == 1 && minor <= 2;
        if self.strict_version && !supported {
            self.set_error(val, ErrorKind::UnsupportedSpecVersion);
            return false;
        }
        true
    }

    // ---------------------------------------------------------------------
    // Values, types and strings
    // ---------------------------------------------------------------------

    fn value_intro(&mut self, val: &mut Validator) -> bool {
        if !self.finalize(val) || !val.on_value_intro() {
            return false;
        }
        self.str_data.clear();
        // max_array_items budgets everything under one outermost '[', so it
        // is only reset where no array is open.
        if self.array_nesting_level == 0 {
            self.array_items = 0;
        }
        self.token = TokenKind::Null;
        self.in_array_element = false;
        val.acc_reset();
        self.state = St::ValueIntro;
        true
    }

    /// Resets the frames above `base` (scratch for arrays closed since),
    /// keeping `base`'s own `open_struct_level`, which is still alive.
    fn wipe_frames_above(&mut self, base: u8) {
        let base = usize::from(base);
        let hwm = usize::from(self.arr_used_max);
        if hwm > base {
            let open_level = self.frame(base).open_struct_level;
            for f in self.frames.iter_mut().take(hwm).skip(base) {
                *f = Frame::empty();
            }
            self.frame(base).open_struct_level = open_level;
        }
        self.arr_used_max = u16::try_from(base).unwrap_or(u16::MAX);
    }

    /// Whether the innermost open array was opened at the current struct
    /// level, i.e. a `;`, `,` or `]` here belongs to it.
    fn innermost_array_is_own(&mut self) -> bool {
        let level = usize::from(self.array_nesting_level);
        self.array_nesting_level > 0
            && self.frame(level).open_struct_level == self.struct_nesting_level
    }

    fn value_outro(&mut self, val: &mut Validator) -> bool {
        // A ';' or '}' ends the whole value, so every '[' opened for it must
        // already be closed.
        if self.innermost_array_is_own() {
            val.last_error = Some(ErrorKind::GotIncompleteBvnrStream);
            return false;
        }
        if !self.finalize(val) || !val.on_value_outro() {
            return false;
        }
        self.wipe_frames_above(self.array_nesting_level);
        if self.struct_nesting_level == 0 {
            self.in_array_element = false;
            self.array_nesting_level = 0;
            self.curr_row_size = 0;
            self.array_row_size = 0;
        }
        self.state = St::ValueOutro;
        true
    }

    /// `bvn_type_trim_trailing_ws`: blanks beside the separator that ends a
    /// parameter are not part of it.
    fn type_trim_trailing_ws(&mut self) {
        while matches!(self.type_data.last(), Some(&(0x20 | 0x09))) {
            self.type_data.pop();
        }
    }

    fn push_type_byte(&mut self, val: &mut Validator) -> bool {
        if self.type_data.len() == TYPE_CAP {
            self.set_error(val, ErrorKind::TypeTooLong);
            return false;
        }
        self.type_data.push(self.byte);
        true
    }

    fn copy_type_byte(&mut self, val: &mut Validator) -> bool {
        // "float_fix" / "float_dec" are one keyword each: the suffix must be
        // the very next byte after "float", not separated by blanks or a
        // comment.
        if self.byte == b'_'
            && self.type_data == b"float"
            && self.text_bytes != self.type_kw_end.saturating_add(1)
        {
            self.set_error(val, ErrorKind::TypeParamWhitespace);
            return false;
        }
        if !self.push_type_byte(val) {
            return false;
        }
        self.state = St::CopyTypeByte;
        true
    }

    /// A `,` or `:`: at depth zero it ends a parameter (whitespace may follow);
    /// nested inside `{...}` / `[...]` it is an ordinary code byte.
    fn copy_type_sep_byte(&mut self, val: &mut Validator) -> bool {
        let nested = type_param_depth(&self.type_data) != 0;
        if !nested {
            if self.byte == b',' {
                self.type_trim_trailing_ws();
            }
            // The first ':' separates the family from its parameters; a later
            // one opens a profile namespace, inside which a blank is a code
            // byte. A ',' starts a parameter without a namespace.
            if self.byte == b':' {
                if self.type_family_colon {
                    self.type_param_ns = true;
                } else {
                    self.type_family_colon = true;
                }
            } else {
                self.type_param_ns = false;
            }
        }
        if !self.push_type_byte(val) {
            return false;
        }
        self.state = if nested {
            St::CopyTypeByte
        } else {
            St::TypeBodyOutro
        };
        true
    }

    /// Whitespace in the middle of a parameter: a code byte (space or tab
    /// only) in a namespaced profile parameter, otherwise it moves to
    /// `type_body_ws`, where a further parameter byte is refused.
    fn type_ws_byte(&mut self, val: &mut Validator) -> bool {
        if !self.type_param_ns || (self.byte != 0x20 && self.byte != 0x09) {
            self.state = St::TypeBodyWs;
            return true;
        }
        if !self.push_type_byte(val) {
            return false;
        }
        self.state = St::CopyTypeByte;
        true
    }

    /// `bvn_tf_family_done`: write the canonical family spelling.
    fn tf_family_done(&mut self, name: &[u8]) -> bool {
        self.type_data.clear();
        self.type_data.extend_from_slice(name);
        self.state = St::TypeBodyOutro;
        true
    }

    fn replace_escaped_byte(&mut self, val: &mut Validator) -> bool {
        if self.str_data.len() == usize::from(self.max_string_length) {
            self.set_error(val, ErrorKind::StringTooLong);
            return false;
        }
        let Some(rep) = decode_named_escape(self.byte) else {
            self.set_error(val, ErrorKind::IllegalEscapeSequence);
            return false;
        };
        self.str_data.push(rep);
        self.state = St::CopyStringByte;
        true
    }

    /// `\x` and `\u` are spec 1.1 escapes; earlier documents refuse them.
    fn escape_intro(&mut self, val: &mut Validator, next: St) -> bool {
        if !val.supports_1_1() {
            self.set_error(val, ErrorKind::IllegalEscapeSequence);
            return false;
        }
        self.state = next;
        true
    }

    fn escape_x_d1(&mut self, val: &mut Validator) -> bool {
        let Some(v) = hexval(self.byte) else {
            self.set_error(val, ErrorKind::IllegalEscapeSequence);
            return false;
        };
        self.esc_x_hi = v;
        self.state = St::EscX2;
        true
    }

    fn escape_x_d2(&mut self, val: &mut Validator) -> bool {
        let Some(v) = hexval(self.byte) else {
            self.set_error(val, ErrorKind::IllegalEscapeSequence);
            return false;
        };
        let b = (self.esc_x_hi << 4) | v;
        // An escape may not smuggle in a control byte a raw byte could not.
        if is_string_ctrl_reject(u32::from(b)) {
            self.set_error(val, ErrorKind::UnexpectedInputByte);
            return false;
        }
        if !self.string_push_byte(val, b) {
            return false;
        }
        self.str_has_raw_escape = true;
        self.state = St::CopyStringByte;
        true
    }

    fn escape_u_hex(&mut self, val: &mut Validator) -> bool {
        if self.byte == b'}' {
            if self.esc_u_digits == 0 {
                self.set_error(val, ErrorKind::IllegalEscapeSequence);
                return false;
            }
            let cp = self.esc_u_acc;
            if cp > 0x10_ffff || (0xd800..=0xdfff).contains(&cp) {
                self.set_error(val, ErrorKind::InvalidCodepoint);
                return false;
            }
            if is_string_ctrl_reject(cp) {
                self.set_error(val, ErrorKind::UnexpectedInputByte);
                return false;
            }
            // The checks above make every scalar valid; the fallback only
            // keeps this panic-free.
            let ch = char::from_u32(cp).unwrap_or(char::REPLACEMENT_CHARACTER);
            let mut buf = [0u8; 4];
            for &b in ch.encode_utf8(&mut buf).as_bytes() {
                if !self.string_push_byte(val, b) {
                    return false;
                }
            }
            self.state = St::CopyStringByte;
            return true;
        }
        let Some(v) = hexval(self.byte).filter(|_| self.esc_u_digits < 6) else {
            self.set_error(val, ErrorKind::IllegalEscapeSequence);
            return false;
        };
        self.esc_u_acc = (self.esc_u_acc << 4) | u32::from(v);
        self.esc_u_digits += 1;
        self.state = St::EscUHex;
        true
    }

    // ---------------------------------------------------------------------
    // Arrays and structs
    // ---------------------------------------------------------------------

    /// The frame at `level`. Levels never exceed `max_array_nesting`
    /// (`array_intro` refuses that), for which `new` allocated frames; the
    /// vector still grows rather than panic should that ever change.
    fn frame(&mut self, level: usize) -> &mut Frame {
        if level >= self.frames.len() {
            self.frames.resize_with(level + 1, Frame::empty);
        }
        &mut self.frames[level]
    }

    /// Restores the type, and the unit with its spelling, saved in `frame`;
    /// the unit only if `only_if_moved` is off or the serial has moved.
    fn restore_vtype_unit(val: &mut Validator, f: &Frame, only_if_moved: bool) {
        val.value_type = f.saved_vtype;
        if !only_if_moved || val.parsed_unit_serial != f.saved_unit_serial {
            val.parsed_unit = f.saved_vunit.clone();
            val.unit_src.clear();
            val.unit_src.extend_from_slice(&f.saved_usrc);
            val.parsed_unit_serial = f.saved_unit_serial;
        }
    }

    fn array_intro(&mut self, val: &mut Validator) -> bool {
        if self.max_array_items != 0 && self.array_items == self.max_array_items {
            self.set_error(val, ErrorKind::TooManyArrayItems);
            return false;
        }
        let level = self.array_nesting_level;
        if level == self.max_array_nesting {
            self.set_error(val, ErrorKind::ArrayNestingTooHigh);
            return false;
        }
        self.array_items = self.array_items.saturating_add(1);
        let (curr, row) = (self.curr_row_size, self.array_row_size);
        let f = self.frame(usize::from(level));
        f.saved_curr = curr;
        f.saved_row = row;
        f.saved_vtype = val.value_type;
        f.saved_vunit = val.parsed_unit.clone();
        f.saved_usrc.clear();
        f.saved_usrc.extend_from_slice(&val.unit_src);
        f.saved_unit_serial = val.parsed_unit_serial;
        // u64::MAX: row width not established yet (0 is a real width).
        let row_size = if f.in_dim_seq {
            f.dim_row_size
        } else {
            u64::MAX
        };
        self.array_row_size = row_size;
        self.curr_row_size = 1;
        // The new array tracks its own '/' sequence in the child frame;
        // start it clean so a sibling's row width cannot leak in.
        let snl = self.struct_nesting_level;
        let child = self.frame(usize::from(level) + 1);
        child.in_dim_seq = false;
        child.dim_row_size = 0;
        child.open_struct_level = snl;
        self.token = TokenKind::Null;
        self.in_array_element = true;
        val.acc_reset();
        if !val.on_array_intro() {
            return false;
        }
        self.array_nesting_level += 1;
        if u16::from(self.array_nesting_level) > self.arr_used_max {
            self.arr_used_max = u16::from(self.array_nesting_level);
        }
        self.state = St::ArrayIntro;
        true
    }

    /// `bvn_array_close_allowed`: a `]` closes the innermost `[` only where
    /// no struct has been opened inside it since.
    fn array_close_allowed(&mut self, val: &mut Validator) -> bool {
        if !self.innermost_array_is_own() {
            self.set_error(val, ErrorKind::UnexpectedInputByte);
            return false;
        }
        true
    }

    /// Pops one array level after its row closed.
    fn pop_array_level(&mut self, val: &mut Validator) {
        self.array_nesting_level = self.array_nesting_level.saturating_sub(1);
        let row = self.array_row_size;
        let level = usize::from(self.array_nesting_level);
        let f = self.frame(level);
        f.dim_row_size = row;
        let (curr, saved_row) = (f.saved_curr, f.saved_row);
        self.curr_row_size = curr;
        self.array_row_size = saved_row;
        if let Some(f) = self.frames.get(level) {
            Self::restore_vtype_unit(val, f, true);
        }
        self.state = St::ArrayOutro;
    }

    fn array_outro(&mut self, val: &mut Validator) -> bool {
        if !self.array_close_allowed(val) || !self.finalize(val) {
            return false;
        }
        if !val.on_array_outro(self.curr_row_size, &mut self.array_row_size) {
            return false;
        }
        self.pop_array_level(val);
        true
    }

    /// `]` straight after `[`: an empty row. The null placeholder that
    /// `array_intro` seeded is dropped, not emitted.
    fn array_outro_empty(&mut self, val: &mut Validator) -> bool {
        if !self.array_close_allowed(val) {
            return false;
        }
        self.token = TokenKind::Unknown;
        self.str_data.clear();
        self.curr_row_size = 0;
        if !val.on_array_outro(0, &mut self.array_row_size) {
            return false;
        }
        self.pop_array_level(val);
        true
    }

    fn new_array_value(&mut self, val: &mut Validator) -> bool {
        // A ',' separates elements of the innermost '[' only where no struct
        // has been opened inside it since.
        if !self.innermost_array_is_own() {
            self.set_error(val, ErrorKind::UnexpectedInputByte);
            return false;
        }
        if !self.finalize(val) {
            return false;
        }
        if self.max_array_items != 0 && self.array_items == self.max_array_items {
            self.set_error(val, ErrorKind::TooManyArrayItems);
            return false;
        }
        self.array_items = self.array_items.saturating_add(1);
        self.str_data.clear();
        self.in_array_element = true;
        val.acc_reset();
        if !val.on_new_array_value(self.curr_row_size, self.array_row_size) {
            return false;
        }
        let level = usize::from(self.array_nesting_level);
        self.frame(level).in_dim_seq = false;
        // The element starts with the annotation of the array again.
        let parent = level.saturating_sub(1);
        if let Some(par) = self.frames.get(parent) {
            Self::restore_vtype_unit(val, par, true);
        }
        self.curr_row_size = self.curr_row_size.saturating_add(1);
        self.token = TokenKind::Null;
        self.state = St::NewArrayValue;
        true
    }

    fn struct_intro(&mut self, val: &mut Validator) -> bool {
        if self.struct_nesting_level == self.max_struct_nesting {
            self.set_error(val, ErrorKind::StructNestingTooHigh);
            return false;
        }
        // A struct takes no type annotation; its members carry their own.
        if val.value_type.family != TypeFamily::Plain {
            self.set_error(val, ErrorKind::TypeValueMismatch);
            return false;
        }
        if !val.receive_event(Ev::StructStart) {
            return false;
        }
        self.struct_nesting_level += 1;
        self.token = TokenKind::Structure;
        self.state = St::StructIntro;
        true
    }

    fn struct_outro(&mut self, val: &mut Validator) -> bool {
        if self.struct_nesting_level == 0 {
            self.set_error(val, ErrorKind::IllegalStructClose);
            return false;
        }
        self.struct_nesting_level -= 1;
        if !val.receive_event(Ev::StructEnd) {
            return false;
        }
        self.state = St::StructOutro;
        true
    }

    // ---------------------------------------------------------------------
    // Octet streams (bvn_read_octet_stream, resumable)
    // ---------------------------------------------------------------------

    fn octet_byte(&mut self, val: &mut Validator, b: u8) -> Result<(), Error> {
        match self.octet_phase {
            OctetPhase::Tag => {
                self.octet_bytes += 1;
                if b == 0x00 {
                    self.token = TokenKind::Unknown;
                    self.str_data.clear();
                    if !val.receive_event(Ev::OctetStreamEnd) {
                        return Err(self.fail(val));
                    }
                    self.state = St::OctetStreamOutro;
                } else if b == 0x01 {
                    self.octet_phase = OctetPhase::LenLo;
                } else {
                    // Reported like a text error: the tag's own offset and
                    // the tag byte.
                    let reached = self.text_bytes + self.octet_bytes;
                    self.capture_os_error(
                        val,
                        ErrorKind::OctetStreamOutOfSync,
                        reached.saturating_sub(1),
                    );
                    val.error_byte = u32::from(b);
                    return Err(self.fail(val));
                }
            }
            OctetPhase::LenLo => self.octet_phase = OctetPhase::LenHi(b),
            OctetPhase::LenHi(lo) => {
                self.octet_bytes += 2;
                // A length of 0 means the maximal 65536.
                let len = usize::from(u16::from_le_bytes([lo, b]));
                self.octet_phase = OctetPhase::Payload(if len == 0 { 65_536 } else { len });
                self.octet_buf.clear();
            }
            OctetPhase::Payload(len) => {
                self.octet_buf.push(b);
                if self.octet_buf.len() == len {
                    self.octet_bytes += len as u64;
                    if !val.receive_octet_chunk(&self.octet_buf) {
                        return Err(self.fail(val));
                    }
                    self.octet_buf.clear();
                    self.octet_phase = OctetPhase::Tag;
                }
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------------
    // Error recovery (continue_on_error)
    // ---------------------------------------------------------------------

    /// `bvn_resync_close_own_arrays`: close the arrays opened inside the
    /// innermost open struct (or at top level), leaving an array the struct
    /// is an element of open, so start and end events keep nesting.
    fn resync_close_own_arrays(&mut self, val: &mut Validator) -> bool {
        while self.innermost_array_is_own() {
            if !val.receive_event(Ev::ArrayRowEnd) {
                return false;
            }
            self.array_nesting_level -= 1;
            let level = usize::from(self.array_nesting_level);
            let f = self.frame(level);
            let (curr, row) = (f.saved_curr, f.saved_row);
            self.curr_row_size = curr;
            self.array_row_size = row;
            if let Some(f) = self.frames.get(level) {
                Self::restore_vtype_unit(val, f, false);
            }
        }
        self.wipe_frames_above(self.array_nesting_level);
        if self.array_nesting_level == 0 {
            self.curr_row_size = 0;
            self.array_row_size = 0;
            self.array_items = 0;
        }
        true
    }

    /// `bvn_recover_in_comment`: a bad byte inside a comment is reported and
    /// dropped, and the comment goes on; nothing structural is unwound.
    /// Returns false when the lexer is not in a comment.
    fn recover_in_comment(&mut self, val: &mut Validator, ends_line: bool) -> bool {
        let first = match self.state {
            St::CommentIntro | St::IgnoreCommentByte => false,
            St::FirstCommentIntro | St::FirstCommentAfterEf | St::FirstCommentAfterEfBb => true,
            _ => return false,
        };
        self.utf8_need = 0;
        self.utf8_lo = 0;
        self.utf8_hi = 0;
        val.error_line = 0;
        val.error_column = 0;
        val.error_byte = 0;
        val.error_offset = 0;
        self.recovery_count = self.recovery_count.saturating_add(1);
        self.skipped_bytes = self.skipped_bytes.saturating_add(1);
        if first {
            // A first comment that needed recovery is no version directive.
            self.ver_checked = true;
            self.state = if ends_line {
                St::FirstBom
            } else {
                St::FirstCommentIntro
            };
        } else if ends_line {
            self.comment_outro();
        }
        true
    }

    /// `bvn_enter_resync`: unwind the broken statement's own arrays, clear
    /// all scratch state and start skipping to the next boundary.
    fn enter_resync(&mut self, val: &mut Validator) -> bool {
        self.resync_saved_struct_nesting = self.struct_nesting_level;
        if !self.resync_close_own_arrays(val) {
            val.value_type = TypeSpec::PLAIN;
            val.parsed_unit = Unit::no_unit();
            return false;
        }
        self.utf8_need = 0;
        self.utf8_lo = 0;
        self.utf8_hi = 0;
        self.bom_len = 0;
        self.str_data.clear();
        self.type_data.clear();
        self.inline_unit.clear();
        self.token = TokenKind::Unknown;
        val.acc_reset();
        val.value_type = TypeSpec::PLAIN;
        val.parsed_unit = Unit::no_unit();
        val.has_annotation_unit = false;
        val.error_line = 0;
        val.error_column = 0;
        val.error_byte = 0;
        val.error_offset = 0;
        // The unit went back to "none" under a frame that may still be open;
        // moving the serial makes that frame restore its own unit.
        val.parsed_unit_serial = val.parsed_unit_serial.wrapping_add(1);
        self.resync_array_depth = 0;
        self.resync_struct_depth = 0;
        // Inside a broken string its closing quote is still ahead, so skip
        // as string content. An escape cut short by the closing quote itself,
        // or a quote that was the offending byte elsewhere, flips that.
        self.state = match self.state {
            St::StringIntro | St::CopyStringByte | St::EscapeFromCopy => St::ResyncString,
            St::EscX1 | St::EscX2 | St::EscUOpen | St::EscUHex => {
                if self.byte == b'"' {
                    St::Resync
                } else {
                    St::ResyncString
                }
            }
            _ => {
                if self.byte == b'"' {
                    St::ResyncString
                } else {
                    St::Resync
                }
            }
        };
        self.recovery_count = self.recovery_count.saturating_add(1);
        self.resync_start_bytes = self.text_bytes;
        self.in_array_element = false;
        true
    }

    /// A `]` or `}` met while skipping: it either balances a bracket opened
    /// since recovery began or closes a real container, whose close events
    /// the validator then gets.
    fn resync_close_bracket(&mut self, val: &mut Validator) -> bool {
        if self.byte == b']' {
            if self.resync_array_depth > 0 {
                self.resync_array_depth -= 1;
            } else if self.innermost_array_is_own() {
                if !val.receive_event(Ev::ArrayRowEnd) {
                    return false;
                }
                self.array_nesting_level -= 1;
                let curr = self.curr_row_size;
                let level = usize::from(self.array_nesting_level);
                let f = self.frame(level);
                f.dim_row_size = curr;
                let (saved_curr, saved_row) = (f.saved_curr, f.saved_row);
                self.curr_row_size = saved_curr;
                self.array_row_size = saved_row;
                if let Some(f) = self.frames.get(level) {
                    Self::restore_vtype_unit(val, f, false);
                }
                self.in_array_element = self.array_nesting_level > 0;
            }
        } else if self.byte == b'}' {
            if self.resync_struct_depth > 0 {
                self.resync_struct_depth -= 1;
            } else if self.struct_nesting_level > 0 {
                // Arrays opened inside this struct go with it.
                if !self.resync_close_own_arrays(val) {
                    return false;
                }
                self.in_array_element = false;
                self.struct_nesting_level -= 1;
                if !val.receive_event(Ev::StructEnd) {
                    return false;
                }
            }
        }
        self.state = St::Resync;
        true
    }

    /// `bvn_resync_reset`: back to a clean assignment boundary, closing the
    /// structs opened since recovery began.
    fn resync_reset(&mut self, val: &mut Validator) -> bool {
        let mut ok = true;
        self.count_skipped();
        while self.struct_nesting_level > self.resync_saved_struct_nesting {
            self.struct_nesting_level -= 1;
            if !val.receive_event(Ev::StructEnd) {
                ok = false;
                break;
            }
        }
        if self.struct_nesting_level < self.resync_saved_struct_nesting {
            self.resync_saved_struct_nesting = self.struct_nesting_level;
        }
        self.token = TokenKind::Unknown;
        self.str_data.clear();
        self.type_data.clear();
        self.inline_unit.clear();
        self.utf8_need = 0;
        self.utf8_lo = 0;
        self.utf8_hi = 0;
        self.in_array_element = false;
        self.struct_nesting_level = self.resync_saved_struct_nesting;
        self.resync_array_depth = 0;
        self.resync_struct_depth = 0;
        if !self.resync_close_own_arrays(val) {
            ok = false;
        }
        val.value_type = TypeSpec::PLAIN;
        val.parsed_unit = Unit::no_unit();
        val.parsed_unit_serial = val.parsed_unit_serial.wrapping_add(1);
        val.has_annotation_unit = false;
        val.acc_reset();
        ok
    }

    fn resync_semicolon_reset(&mut self, val: &mut Validator) -> bool {
        let ok = self.resync_reset(val);
        self.state = St::ValueOutro;
        ok
    }
}

#[cfg(test)]
mod tests;
