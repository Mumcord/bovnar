# Bovnar — Read & Write API

> **Spec version:** 1.2
> **Status:** Reference — the C reader, writer, and DOM as implemented
> **Scope:** Every function needed to read and write Bovnar streams, in the order you call them.

The writer uses the same event/data model as the reader — `bvnr_event_t` and `bvnr_data_t` — so the two APIs are deliberately symmetric. Learn one, the other follows.

---

## Table of Contents

1. [Reader](#1-reader)
    - 1.1 [`bvnr_reader_create` / `bvnr_reader_destroy`](#11-bvnr_reader_create--bvnr_reader_destroy)
    - 1.2 [`bvnr_source_from_fd`](#12-bvnr_source_from_fd)
    - 1.3 [`bvnr_source_from_mem`](#13-bvnr_source_from_mem)
    - 1.4 [`bvnr_open_read_source`](#14-bvnr_open_read_source)
    - 1.5 [`bvnr_open_read_mem`](#15-bvnr_open_read_mem)
    - 1.6 [`bvnr_read`](#16-bvnr_read)
    - 1.7 [`bvnr_reader_get_error` and friends](#17-bvnr_reader_get_error-and-friends)
    - 1.8 [Version directive (spec 1.1)](#18-version-directive-spec-11)
    - 1.9 [Datetime family (spec 1.1)](#19-datetime-family-spec-11)
    - 1.10 [Read-time lossless unit / base conversion (`want_unit`)](#110-read-time-lossless-unit--base-conversion-want_unit)
    - 1.11 [`bvn_parse_uint64` / `bvn_parse_int64` / `bvn_parse_double`](#111-bvn_parse_uint64--bvn_parse_int64--bvn_parse_double)
    - 1.12 [Reader-side unit policy (`bvnr_reader_set_unit_policy`)](#112-reader-side-unit-policy-bvnr_reader_set_unit_policy)
    - 1.13 [Refusing the binary half (`text_only`)](#113-refusing-the-binary-half-text_only)
2. [Writer](#2-writer)
    - 2.1 [`bvnr_writer_create` / `bvnr_writer_destroy`](#21-bvnr_writer_create--bvnr_writer_destroy)
    - 2.2 [`bvnr_sink_to_fd`](#22-bvnr_sink_to_fd)
    - 2.3 [`bvnr_sink_to_mem`](#23-bvnr_sink_to_mem)
    - 2.4 [`bvnr_sink_bytes_written`](#24-bvnr_sink_bytes_written)
    - 2.5 [`bvnr_open_write_sink`](#25-bvnr_open_write_sink)
    - 2.6 [`bvnr_open_write_mem`](#26-bvnr_open_write_mem)
    - 2.7 [`bvnr_write_event`](#27-bvnr_write_event)
    - 2.8 [`bvnr_write_version`](#28-bvnr_write_version)
    - 2.9 [`bvnr_write_finish`](#29-bvnr_write_finish)
    - 2.10 [`bvnr_writer_get_error` and friends](#210-bvnr_writer_get_error-and-friends)
    - 2.11 [`bvn_format_uint64` / `bvn_format_int64` / `bvn_format_double`](#211-bvn_format_uint64--bvn_format_int64--bvn_format_double)
    - 2.12 [`bvnr_write_bvnf_base` / `bvnr_write_bvnf_base_unit`](#212-bvnr_write_bvnf_base--bvnr_write_bvnf_base_unit)
    - 2.13 [`bvnr_write_bvni` / `bvnr_write_bvni_unit`](#213-bvnr_write_bvni--bvnr_write_bvni_unit)
    - 2.14 [`BVN_TYPE_FLOAT_BASE`](#214-bvn_type_float_base)
    - 2.15 [Writer-side unit policy (`bvnr_writer_set_unit_policy`)](#215-writer-side-unit-policy-bvnr_writer_set_unit_policy)
    - 2.16 [Value helpers](#216-value-helpers)
3. [Shared](#3-shared)
    - 3.1 [`bvnr_write_type_annotation`](#31-bvnr_write_type_annotation)
    - 3.2 [`bvn_parse_unit` / `bvn_parse_unit_n`](#32-bvn_parse_unit--bvn_parse_unit_n)
    - 3.3 [`bvn_unit_to_string` / `bvn_unit_to_string_ex`](#33-bvn_unit_to_string--bvn_unit_to_string_ex)
    - 3.4 [`bvn_unit_convert_value` *(bovnar_si_units.h)*](#34-bvn_unit_convert_value-bovnar_si_unitsh)
    - 3.5 [`bvn_error_to_string`](#35-bvn_error_to_string)
    - 3.6 [Canonicalising observer (`bvnr_canon_observer_*`)](#36-canonicalising-observer-bvnr_canon_observer_)
    - 3.7 [Unit inspection helpers](#37-unit-inspection-helpers)
4. [DOM API (`bovnar_dom.h`)](#4-dom-api-bovnar_domh)
    - 4.1 [Parsing and lifetime](#41-parsing-and-lifetime)
    - 4.2 [Navigation](#42-navigation)
    - 4.3 [Type inspection](#43-type-inspection)
    - 4.4 [Typed value accessors](#44-typed-value-accessors)
    - 4.5 [Building a tree](#45-building-a-tree)
    - 4.6 [Minimal example](#46-minimal-example)
    - 4.7 [Parsing under a unit policy](#47-parsing-under-a-unit-policy)
5. [Complete Read Example](#5-complete-read-example)
6. [Inline Unit Suffix — Reading](#6-inline-unit-suffix--reading)
    - 6.1 [Reading inline unit values](#61-reading-inline-unit-values)
7. [Complete Write Example](#7-complete-write-example)

- [See also](#see-also)

---

## 1. Reader

---

### 1.1 `bvnr_reader_create` / `bvnr_reader_destroy`

```c
bvnr_reader_t *bvnr_reader_create(void);
void           bvnr_reader_destroy(bvnr_reader_t *r);
```

Allocate and free a reader on the heap. `bvnr_reader_create` returns `NULL` if allocation fails. Always pair with `bvnr_reader_destroy` when done.

```c
bvnr_reader_t *r = bvnr_reader_create();
if (!r) { perror("alloc"); exit(1); }

/* ... open, read ... */

bvnr_reader_destroy(r);
```

---

### 1.2 `bvnr_source_from_fd`

```c
void bvnr_source_from_fd(bvnr_source_t *s, int fd);
```

Initialise the source `s` to read from an open, readable POSIX file descriptor. The caller retains ownership of `fd`; the library will not close it.

- `s` — pointer to a caller-allocated `bvnr_source_t` (stack is fine).
- `fd` — any readable descriptor: a file, a pipe, a socket.

```c
bvnr_source_t src;
int fd = open("config.bvnr", O_RDONLY);
if (fd < 0) { perror("open"); exit(1); }

bvnr_source_from_fd(&src, fd);
/* pass &src to bvnr_open_read_source */
```

---

### 1.3 `bvnr_source_from_mem`

```c
void bvnr_source_from_mem(bvnr_source_t *s, const void *buf, uint64_t len);
```

Initialise the source `s` to read from an in-memory buffer. The buffer must remain valid for the duration of `bvnr_read`. No copy is made.

- `buf` — pointer to the Bovnar data.
- `len` — number of bytes in the buffer.

```c
static const char payload[] =
    ".host = \"localhost\";\n"
    ".port = <uint:16> 8080;\n";

bvnr_source_t src;
bvnr_source_from_mem(&src, payload, sizeof(payload) - 1);
```

---

### 1.4 `bvnr_open_read_source`

```c
bool bvnr_open_read_source(bvnr_reader_t        *r,
                         const bvnr_source_t  *src,
                         const bvnr_sink_t    *src_mirror,
                         bvnr_read_flags_t    *options);
```

Attach the source to the reader and configure it. Must be called before `bvnr_read`. Returns `false` on invalid arguments (`error_invalid_argument`).

- `r` — reader obtained from `bvnr_reader_create`.
- `src` — source initialised with one of the `bvnr_source_from_*` functions.
- `src_mirror` — optional sink that mirrors the raw input bytes as they are consumed (useful for debugging; pass `NULL` in production).
- `options` — configuration struct. Zero-initialise to get all defaults. The most important fields are:

```c
typedef struct bvnr_read_flags_s {
    void  *userdata;
    bool (*on_unverified)(void *userdata, bvnr_event_t, bvnr_data_t *);
    bool (*on_verified)  (void *userdata, bvnr_event_t, bvnr_data_t *);
    bool   continue_on_error;
    bvnr_on_error_fn on_error;
    uint64_t max_file_size;        /* 0 → no total-byte cap (default; text still bounded by max_text_bytes); set positive to cap */
    uint8_t  max_struct_nesting;   /* 0 → 64 internal default; hard cap 255 */
    uint8_t  max_array_nesting;    /* 0 → 64 internal default; hard cap 255 */
    /* ... the remaining size limits, plus strict_version, text_only (§1.13),
     * and the want_unit hook and its two knobs (§1.10) ... */
} bvnr_read_flags_t;
```

The full struct is in `include/bovnar.h`, and the specification lists it in
§16.4.

`on_verified` is the callback you will implement almost always. `on_unverified` fires before semantic validation — use it only for diagnostics or partial inspection. The one exception is read-time unit conversion: `want_unit` (§1.10) runs ahead of **both** callbacks, so with it installed an `on_unverified` consumer also sees a populated `converted`/`conv` on `ev_data`. Both callbacks must return `true` to continue parsing, `false` to abort (sets `error_scanner_callback_failed`).

The `options` pointer is not stored; the struct is read during `bvnr_open_read_source` only, so it may live on the stack.

**`max_text_bytes` is not a total-size cap.** It counts the bytes the *text*
scanner consumes; an octet stream's binary payload does not count towards it. A
212-byte document whose body is a 200-byte octet stream is accepted at
`max_text_bytes = 8`. If you are bounding memory or guarding against a hostile
input, `max_file_size` is the one that counts every byte. Both are exact at their
boundary: a document needing N is accepted at N and refused at N-1.

**Reader default limits.** When a `bvnr_read_flags_t` field is set to `0`, the reader substitutes an internal default. For the nesting fields, the default is **64** (not 255); the hard maximum is 255. For `max_array_items` and `max_text_bytes`, the default is **2 147 483 647** — permissive but finite. **`max_file_size` differs: `0` means no total-byte cap**. That is not endless: the text is still bounded by `max_text_bytes`, so an endless text stream raises that as well; only octet-stream payload runs without bound. Setting `max_file_size` explicitly to `16777216` (16 MiB) is recommended for production.

```c
static bool on_event(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
    /* handle ev / d ... */
    return true;
}

bvnr_read_flags_t opts = {
    .on_verified   = on_event,
    .userdata      = &my_ctx,
    .max_file_size = 16777216,  /* 16 MiB cap */
    /* max_array_nesting: 0 → 64 internal default; hard cap 255 */
};

if (!bvnr_open_read_source(r, &src, NULL, &opts)) {
    fprintf(stderr, "open failed\n");
    return -1;
}
```

---

### 1.5 `bvnr_open_read_mem`

```c
bool bvnr_open_read_mem(bvnr_reader_t     *r,
                         const void        *buf,
                         uint64_t           len,
                         void              *mirror_buf,
                         uint64_t           mirror_cap,
                         bvnr_read_flags_t *options);
```

Convenience wrapper that constructs a memory source (and optionally a memory mirror sink) internally. Equivalent to calling `bvnr_source_from_mem` followed by `bvnr_open_read_source`. Pass `NULL` / `0` for `mirror_buf` / `mirror_cap` to skip mirroring.

```c
bvnr_read_flags_t opts = { .on_verified = on_event, .userdata = &ctx };

if (!bvnr_open_read_mem(r, payload, payload_len, NULL, 0, &opts))
    return -1;
```

---

### 1.6 `bvnr_read`

```c
bool bvnr_read(bvnr_reader_t *r);
```

Drive the parser until EOF or a fatal error. Fires the registered callbacks for every event. Returns `true` on clean completion, `false` on any error.

This is the only call needed after `bvnr_open_read_*`. The reader does not allocate during this call; all buffering is internal to the reader struct.

```c
if (!bvnr_read(r)) {
    fprintf(stderr, "parse error: %s at line %" PRIu64 " col %" PRIu64 "\n",
            bvn_error_to_string(bvnr_reader_get_error(r)),
            bvnr_reader_get_error_line(r),
            bvnr_reader_get_error_column(r));
    bvnr_reader_destroy(r);
    return -1;
}
```

---

### 1.7 `bvnr_reader_get_error` and friends

```c
error_code_t bvnr_reader_get_error     (const bvnr_reader_t *r);
uint64_t     bvnr_reader_get_error_line   (const bvnr_reader_t *r);
uint64_t     bvnr_reader_get_error_column (const bvnr_reader_t *r);
uint64_t     bvnr_reader_get_error_offset (const bvnr_reader_t *r);
uint32_t     bvnr_reader_get_error_byte   (const bvnr_reader_t *r);
uint64_t     bvnr_reader_get_recovery_count(const bvnr_reader_t *r);
```

The three position counters measure **different things**, and a tool placing a
caret needs the right one:

| Getter | Unit | Base | Note |
|---|---|---|---|
| `..._line` | lines | 1 | |
| `..._column` | **characters** | 1 | a multi-byte UTF-8 sequence advances it by one; a tab advances to the next multiple of 4 |
| `..._offset` | **bytes** | 0 | use this to index the input buffer |

In `.x = "café"; .y = @;` the `@` is reported at column 19 and offset 19 — the
19th character, and the 20th byte, because `é` is two bytes: a 1-based character
count and a 0-based byte count that happen to land on the same number. One more
non-ASCII character pulls them apart: in `.x = "çafé"; .y = @;` the `@` is at
column 19 and offset 20. This matters here more than in most formats: unit
symbols are routinely non-ASCII (`µ~m`, `°C`, `Ω`), so the two disagree often.

All five error/location getters above (everything except `bvnr_reader_get_recovery_count`) describe the error that **ended** the read, so they are meaningful when `bvnr_read` returned `false`, and inside an `on_error` callback, where they agree with its arguments. Errors recovered under `continue_on_error` are reported through `on_error` only: after a read that returned `true`, `bvnr_reader_get_error` is `error_none` and the position getters return `0`, however many errors were recovered (spec §13.4). `bvnr_reader_get_error_byte` returns the raw byte value that triggered the error. `bvnr_reader_get_recovery_count` is the exception: it returns how many errors triggered entry into resync mode in `continue_on_error` mode (and so is meaningful even when `bvnr_read` ultimately returned `true`). This count is incremented at error entry, not when resync completes at `";". `bvnr_reader_get_skipped_bytes` is the same kind of thing and answers the question the count cannot: how much of the input recovery threw away. One skipped assignment and a whole discarded struct both report a single recovery, and the skipped bytes were never parsed, so nothing else in the API mentions them — a non-zero total means the document your callbacks saw is not the whole document.

```c
if (!bvnr_read(r)) {
    error_code_t ec = bvnr_reader_get_error(r);
    fprintf(stderr,
            "%s at line %" PRIu64 ", col %" PRIu64
            ", offset %" PRIu64 ", byte 0x%02X\n",
            bvn_error_to_string(ec),
            bvnr_reader_get_error_line(r),
            bvnr_reader_get_error_column(r),
            bvnr_reader_get_error_offset(r),
            bvnr_reader_get_error_byte(r));
}
```

---

### 1.8 Version directive (spec 1.1)

```c
bool        bvnr_reader_get_declared_version(
                const bvnr_reader_t *r, uint16_t *major, uint16_t *minor);
bool        bvnr_peek_version(
                const void *buf, uint64_t len, uint16_t *major, uint16_t *minor);
uint32_t    bvnr_version(void);
const char *bvnr_version_string(void);
void        bvnr_spec_version(uint16_t *major, uint16_t *minor);
```

A document may begin with a `#!bovnar <major>.<minor>` directive (see spec §3.4).
After `bvnr_read`, `bvnr_reader_get_declared_version` returns `true` and fills
`major`/`minor` when the document carried one (either out pointer may be NULL).
Set `bvnr_read_flags_t.strict_version` to reject a version newer than this build
supports with `error_unsupported_spec_version`; by default such a version is
recorded but accepted. A malformed directive is always
`error_invalid_spec_version`.

`bvnr_peek_version` scans a raw buffer for the directive without a full parse
(handy before opening a writer to round-trip it). `bvnr_version` /
`bvnr_version_string` return the library version; `bvnr_spec_version` returns the
highest spec version this build understands (`BVNR_SPEC_VERSION_*`).

```c
uint16_t maj, min;
if (bvnr_reader_get_declared_version(r, &maj, &min))
    printf("document declares bovnar %u.%u\n", maj, min);
```

To emit a directive, call `bvnr_write_version` right after opening the writer
(see §2.5), or set `bvnr_write_flags_t.emit_version` to stamp the current spec
version automatically.

---

### 1.9 Datetime family (spec 1.1)

```c
const char *bvnr_datetime_epoch_name (value_type_spec_t vt);
int32_t     bvnr_datetime_epoch_mjd  (value_type_spec_t vt);
int32_t     bvnr_datetime_epoch_index(const char *name);
bool        bvnr_write_datetime(bvnr_writer_t *w, const char *key,
                                uint32_t width, const char *epoch, int64_t value);
```

A `<datetime:width,epoch>` value (family `vt_datetime`) is a **signed integer
count of seconds since a named epoch** — a timestamp, distinct from a *duration*
(a number with a time unit, e.g. `<float:64,s>`). The carrier is validated like
`sint`; the epoch is stored as a small dense index in `value_type_spec_t.base`
(not a numeric base — the carrier is always decimal). These two helpers recover
the epoch from a spec: `bvnr_datetime_epoch_name` returns its lowercase name
(`"unix"` — the default — `"tai"`, `"gps"`, `"mjd"`, `"ntp"`, `"galileo"`,
`"glonass"`, `"y2000"`, `"beidou"`), and `bvnr_datetime_epoch_mjd` returns its
Modified Julian Day (the `bvn_epoch_t` value from `bvn_datetime.h`). Pass that to
`bvn_dt_epoch_seconds_to_datetime()` to convert to a civil date/time.

```c
/* on a datetime data event: */
int64_t secs;  bvn_parse_int64((const char *)d->data, d->value_type, &secs);
bvn_datetime_t civil;
bvn_dt_epoch_seconds_to_datetime(&civil,
    (bvn_epoch_t)bvnr_datetime_epoch_mjd(d->value_type), secs);
```

To **write** a datetime, use the typed helper `bvnr_write_datetime` — `epoch` is
an epoch name (or `NULL` for unix; an unknown name is `error_invalid_argument`),
`value` is signed seconds. `bvnr_datetime_epoch_index` maps a name to the index
stored in `value_type_spec_t.base` (the inverse of `bvnr_datetime_epoch_name`),
for building a spec by hand. The document must declare `#!bovnar 1.1` to be
re-read (emit the directive with `bvnr_write_version`).

```c
bvnr_write_version(w, 1, 1);
bvnr_write_datetime(w, "created", 64, "gps", 1750000000);  /* <datetime:64,gps> */
```

The family is spec 1.1: it requires a `#!bovnar 1.1` declaration, and in a
1.0/unversioned document a `datetime` annotation is `error_illegal_value_type`.

**ISO-8601 literals and fractional seconds.** A datetime may be written as an
ISO-8601 literal (`2026-06-15T12:00:00.5Z`) instead of a raw integer; the reader
converts it to the whole-second carrier you receive in `d->data`. When the
literal carries a fractional second, the verbatim digits (no leading `.`) are
delivered alongside the carrier in two `bvnr_data_t` fields, `frac_data` and
`frac_length` — `NULL`/`0` for every other value. The carrier is unchanged (the
value floors to the written second), so the fraction is informational, but it
lets a consumer see sub-second precision the integer cannot hold, and the writer
re-emits it: a datetime data event whose `frac_data` is set is serialised back as
an ISO literal so the value round-trips. Like `d->data`, `frac_data` is **not
NUL-terminated** — bound the read by `frac_length`.

```c
/* on a datetime data event: */
if (d->frac_data && d->frac_length) {
    /* d->frac_data[0 .. frac_length) are the sub-second digits, e.g. "5" */
}
```

**The unit's own spelling (`unit_source`, library 2.0).** `d->value_unit` is what a
unit *means*; `d->unit_source` / `d->unit_source_length` are what the document
*wrote*. The two differ wherever formatting a `value_unit_t` is lossy: `km` and
`k~m` are one unit, `ucum:mm[Hg]` and `mmHg` are one unit, and `ucum:mL{total}`
parses to `m~L` with nowhere in the struct for `{total}` to live.

The reader sets the pair on `ev_data` and on the type-annotation events whenever
`value_unit` is a real unit, and leaves it `NULL`/`0` otherwise — no unit,
`no_unit`, a datetime's epoch. It is the annotation's unit parameter, or the
inline suffix when the annotation carried no unit; where a value has both, the
annotation's. Like `d->data` it is reader-owned, valid for the callback only and
**not NUL-terminated**. A `want_unit` conversion leaves it alone: it is the
spelling of `value_unit`, never of `conv.unit`.

```c
/* on a data event: */
if (d->unit_source) {
    /* d->unit_source[0 .. unit_source_length) is e.g. "ucum:mL{total}",
     * while bvn_unit_to_string(d->value_unit, ...) gives "m~L" */
}
```

The writer honours it in the other direction — see
[§3.1](#31-bvnr_write_type_annotation).

---

### 1.10 Read-time lossless unit / base conversion (`want_unit`)

```c
/* bvnr_read_flags_t */
bool (*want_unit)(void *userdata, const bvnr_data_t *data,
                  value_unit_t *want, uint32_t *want_base);

/* bvnr_data_t */
bool             converted;   /* true when this value was converted */
bvnr_converted_t conv;        /* the exact converted value (see below) */

typedef struct bvnr_converted_s {
    value_unit_t     unit;    /* the target unit */
    const char*      text;    /* exact value in `base`, NUL-terminated;
                               * NULL if it does not terminate in `base` */
    uint32_t         length;  /* strlen(text), 0 when text is NULL */
    uint32_t         base;    /* base text is rendered in (2..62, 64, 85) */
    const struct bvn_int_s* num;   /* exact numerator (signed) */
    const struct bvn_int_s* den;   /* exact denominator (> 0) */
} bvnr_converted_t;

/* bvnr_read_flags_t */
bool     want_unit_allow_nonterminating;  /* opt in to rational-only results */
uint32_t max_conversion_length;           /* work limit; 0 = 1024 */
```

By default the reader hands you every numeric value exactly as written and you
convert it yourself (§3.4, `bvn_unit_convert_rational`). Set `want_unit` to have
the reader do it for you — **losslessly**, in exact arbitrary-precision
arithmetic, for a value of any width and any base.

When `want_unit` is non-NULL, the reader calls it for every numeric value (with
or without a unit) after validation and before **either** value callback — it can
abort the parse, and the two views of one value must not disagree. An
`on_unverified` consumer therefore also sees a populated `converted`/`conv` on
`ev_data`, even though everything else about that event is still the
pre-validation view. Inspect `data->value_unit` (native unit),
`data->value_type`, or `data->data` and either:

- fill `*want` with the target unit and `*want_base` with the output base and
  **return `true`**, or
- **return `false`** (or leave `want_unit` NULL) to receive the value untouched.

`*want_base` accepts any base bvnr can write — `2..62` plus `64` (Base64) and
`85` (Ascii85) — or `0` to keep the value's own. Anything else is an error, never
a quiet substitution. Note that `64` and `85` have no sign character, so a
negative result in those is rejected too.

Requesting `*want` equal to the native unit with a different `*want_base`
performs a pure **base conversion** (e.g. a hex integer delivered in decimal).
The choice is usually key-specific; track "the current key" (from the earlier
`ev_assignment_start` event) in your `userdata`.

On a valid request the value arrives with `data->converted == true` and
`data->conv` holding the **exact** result — the value in the requested unit and
base as both a positional string (`conv.text`) and a reduced rational
(`conv.num`/`conv.den`, `bvn_int.h`). Everything in `conv` is reader-owned and
valid only for the callback — copy `text` (or the rational) to retain it.
`data->data` / `data->value_unit` keep the original digits and unit.

**The conversion is lossless.** A 1056-bit binary float or a 512-bit integer
converts with no loss beyond the library's own declared factor — the result
widens as needed rather than rounding into a `double`. Every numeric family is
eligible: `uint`/`sint` of any width and base (including a multiprecision integer
or a non-decimal base written as a string literal like `<uint:32,_16> "FF"`),
`float`, `float_dec`, `float_fix`. A datetime is never offered.

Once the hook has asked for a conversion, the value either arrives converted or
the parse **stops**. Nothing approximate is delivered, and nothing is silently
skipped:

| Condition | Error |
|-----------|-------|
| `*want` dimensionally incompatible with the value's unit (seconds for a length; one currency for another) | `error_unit_mismatch` |
| the true factor is irrational (a π-based angle, e.g. degree → radian) | `error_unit_inexact` |
| the value's unit puts an affine scale inside a compound (`°C/h`, `°C·m`, `°C²`): the offset is a number of kelvin and the product has nowhere to add it | `error_unit_mismatch` |
| the exact result has no terminating expansion in `*want_base` (e.g. `1 m → mile` in base 10), and `want_unit_allow_nonterminating` is off | `error_unit_inexact` |
| the literal is finite but too extreme to build an exact rational from (e.g. `1e1000000`) | `error_value_out_of_range` |
| `*want_base` is not a base bvnr writes, or the result is negative in base 64/85, or out of memory | `error_invalid_argument` |
| the exact result would be longer than `max_conversion_length` characters | `error_value_out_of_range` |

Only `nan`/`inf`/`ninf` are handed over untouched (`converted == false`, no
error): they carry no finite value, so no conversion was possible or promised.

#### Bounding the work

Rendering an exact expansion is **quadratic in its digit count**, and the count
follows the *magnitude of the value's exponent*, not the literal's length:
`1e-9800` is seven characters and expands to 9800 digits. Left unbounded, a
one-kilobyte document of such values costs minutes of CPU while looking trivial.

`max_conversion_length` caps the characters a conversion may produce; `0` selects
`BVNR_DEFAULT_MAX_CONVERSION_LENGTH` (1024), generous next to any real
measurement. Anything longer is `error_value_out_of_range`, rejected before the
digits are generated. Raise it if you genuinely need thousands of exact digits.
Like `max_number_length` and `max_array_items`, it is here so a consumer of
untrusted input is not at the mercy of the input's shape.

#### Exact-but-not-writable results

Plenty of everyday conversions are exact as a rational yet have no finite
positional expansion in the output base — `km/h → m/s` is `5/18`, `°F → °C` and
`m → km` in base 2 are the same story. By default those abort with
`error_unit_inexact` rather than round.

Set `want_unit_allow_nonterminating` when your consumer can take a rational. The
value then arrives normally with `conv.num`/`conv.den` **exact** and
`conv.text == NULL` (`conv.length == 0`) — always NULL-check `conv.text` before
printing it. An irrational factor still aborts even with the flag set: there is
no exact rational to hand over in the first place.

```c
static bool want_unit(void *ud, const bvnr_data_t *d,
                      value_unit_t *want, uint32_t *want_base)
{
    /* deliver every length in metres, base 10; leave everything else */
    bool ok;
    value_unit_t metre = bvn_parse_unit((const uint8_t *)"m", &ok);
    if (bvn_units_compatible(d->value_unit, metre)) {
        *want = metre;
        *want_base = 10;
        return true;
    }
    return false;
}

static bool on_event(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
    if (ev == ev_data && d->converted && d->conv.text)
        printf("= %s m\n", d->conv.text);   /* e.g. "5 k~m" -> "5000" */
    return true;
}

bvnr_read_flags_t opts = { .on_verified = on_event, .want_unit = want_unit };
```

---

### 1.11 `bvn_parse_uint64` / `bvn_parse_int64` / `bvn_parse_double`

```c
bool bvn_parse_uint64(const char *s, value_type_spec_t vt, uint64_t *out);
bool bvn_parse_int64 (const char *s, value_type_spec_t vt, int64_t  *out);
bool bvn_parse_double(const char *s, value_type_spec_t vt, double   *out);
```

Convert the raw token string received in `ev_data` into a C numeric type. The `vt` argument — taken directly from `d->value_type` — supplies the base and bit-width for range checking.

The `data` pointer inside `bvnr_data_t` is **not NUL-terminated**, and for null/empty values `d->length` may be `0`. Always guard the copy by `d->length` and NUL-terminate manually.

- Returns `true` and writes `*out` on success.
- Returns `false` if the string is not representable in the declared type — this
  includes exceeding `vt.width`, so `<uint:8>` rejects `"256"`. A width above 64
  is not checked: the 64-bit out parameter is the binding limit there.
- Returns `false` for anything a value token cannot look like: an empty string,
  leading whitespace, a leading `+`, or a sign on an unsigned carrier. (The C
  standard library's `strtoull` accepts all four — the last by wrapping `-1` into
  `18446744073709551615` — which is why these helpers do not simply forward to
  it.)
- Acceptance matches the reader exactly: anything these accept, a document can
  carry, and vice versa.

```c
static bool on_event(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
    if (ev != ev_data) return true;

    char buf[256];
    if (d->length >= sizeof(buf)) return false;
    if (d->length) memcpy(buf, d->data, d->length);
    buf[d->length] = '\0';

    switch (d->value_type.family) {
    case vt_uint: {
        uint64_t v;
        if (bvn_parse_uint64(buf, d->value_type, &v))
            printf("uint = %" PRIu64 "\n", v);
        break;
    }
    case vt_sint: {
        int64_t v;
        if (bvn_parse_int64(buf, d->value_type, &v))
            printf("sint = %" PRId64 "\n", v);
        break;
    }
    case vt_float: {
        double v;
        if (bvn_parse_double(buf, d->value_type, &v))
            printf("float = %g\n", v);
        break;
    }
    default:
        break;
    }
    return true;
}
```

### 1.12 Reader-side unit policy (`bvnr_reader_set_unit_policy`)

What `want_unit` does through a callback, stated as data: what the document must
contain, and what unit the consumer wants values delivered in. Everything is
expressed as unit **text**, so a binding can drive it without a function pointer.

```c
#define BVNR_MAX_UNIT_TARGETS 8u
#define BVNR_MAX_UNIT_RULES   8u
#define BVNR_MAX_UNIT_PATH    96u

typedef struct bvnr_unit_target_s {
    const char *unit;   /* "m/s"; parsed when the policy is set */
    uint32_t    base;   /* output base, 0 = the value's own */
} bvnr_unit_target_t;

typedef struct bvnr_unit_rule_s {
    const char            *path;   /* ".inlet.temperature", or ".inlet.*" */
    const char            *unit;
    uint32_t               base;   /* output base for bvnr_rule_convert, 0 = own */
    bvnr_unit_rule_mode_t  mode;   /* bvnr_rule_convert | bvnr_rule_require */
} bvnr_unit_rule_t;

typedef struct bvnr_unit_policy_s {
    /* per-field rules — consulted before everything below */
    const bvnr_unit_rule_t     *rules;
    uint32_t                    num_rules;
    /* conversion */
    const bvnr_unit_target_t   *targets;
    uint32_t                    num_targets;
    uint32_t                    base;         /* output base for `normalise` */
    bvnr_unit_normalise_t       normalise;    /* none | bvnr_normalise_si */
    bvnr_unit_inexact_policy_t  on_inexact;   /* error | leave */
    /* validation — these reject a document and never change a value */
    bool                        require_unit;
    const char * const         *require_dimension_of;
    uint32_t                    num_require_dimension_of;
} bvnr_unit_policy_t;

bool bvnr_reader_set_unit_policy(bvnr_reader_t *r, const bvnr_unit_policy_t *p);
```

Every unit string is parsed by the setter, so a typo is a `false` return before
the parse begins rather than an error inside somebody's document; a rejected
policy leaves the previous one in force. The strings need not outlive the call.
The policy may be installed before or after `bvnr_open_read_*`, and it survives
re-opening the reader on another document — it describes the consumer, not the
document. Passing `NULL` clears it.

#### Per-field rules

A rule names ONE field, by the dotted key path a value sits at — leading dot
included, exactly as the document writes its keys. A path ending in `.*` names
everything below that point at any depth. Rules are consulted **before**
everything else in the policy: they are the most specific thing it can say.

```c
static const bvnr_unit_rule_t rules[] = {
    { ".inlet.temperature", "°C",  0, bvnr_rule_convert },
    { ".inlet.*",           "m",   0, bvnr_rule_require },
};
bvnr_unit_policy_t p = {0};
p.rules = rules; p.num_rules = 2;
```

Unlike a whole-document target, **a rule is an assertion**: the caller named
this field, so a value that cannot be applied to it is `error_unit_mismatch`
rather than a value passed through quietly — silence would defeat the point of
naming it. A bare number fails a rule too: ".speed is m/s" is not satisfied by a
value with no unit. `bvnr_rule_require` asserts without converting.

First match wins, so order matters: put `".a.b"` before `".a.*"`. A prefix only
matches at a component boundary, so `".in.*"` does not claim `".inlet.a"`. Array
elements sit at the path of the assignment holding them, so one rule covers a
whole array. Paths are capped at `BVNR_MAX_UNIT_PATH` characters and 32 levels
of nesting; a value deeper than the tracker can describe is at an **unknown**
path and matches no rule at all, rather than matching the wrong one.

#### Conversion

Each numeric value is converted to the first target it can validly convert to,
so **order is significant**: a list of `{"m", "k~m"}` never selects `k~m`. A
value that matches no target is delivered untouched — that is the normal outcome
for a value no rule mentions, not a failure. A value already in the unit a
target names is also left alone (unless that target asks for an output base,
which makes it a pure base conversion), so `converted` means "the policy
restated this value", not "the policy looked at it".

```c
static const bvnr_unit_target_t targets[] = { { "°C", 0 }, { "m", 0 } };
bvnr_unit_policy_t p = {0};
p.targets = targets; p.num_targets = 2;
bvnr_reader_set_unit_policy(r, &p);
```

| document | delivered |
|----------|-----------|
| `<float:64,°F> 212.0` | `100` in `°C` |
| `<float:64,in> 12.0` | `0.3048` in `m` |
| `<float:64,%> 35.0` | untouched — no target is a ratio |
| `<float:64> 0.25` | untouched — see the fence below |

**A value with no unit only ever matches a target that is itself `no_unit`.** A
bare number is dimensionally compatible with `%` and with `ppm`, so without that
fence a policy naming `"%"` would deliver `0.25` as `25` — the silent rescale the
format exists to prevent, arrived at through the machinery meant to prevent it.
The fence works in both directions: a policy naming `no_unit` will not restate
`35 %` as `0.35`.

Selection uses `bvn_units_convertible`, not `bvn_units_compatible`. That is what
keeps `k~$USD -> $USD` available: a currency carries no dimension by design, so
`bvn_units_compatible` reports it incompatible with itself although the
conversion is exact and well defined.

#### Normalisation

`normalise = bvnr_normalise_si` catches whatever the targets did not, delivering
it in coherent SI base units with prefixes folded out:

| document | delivered |
|----------|-----------|
| `<float:64,in> 12.0` | `0.3048` in `m` |
| `<float:64,°F> 212.0` | `373.15` in `K` |
| `<float:64,g> 5.0` | `0.005` in `k~g` — mass normalises to the kilogram |
| `<float:64,%> 35.0` | untouched |
| `<float:64,$USD> 5.0` | untouched |

Every **dimensionless** unit is left exactly as written — `%`, `ppm`, `dB`, `pH`,
`rad`, `°`, the turbidity scales — along with the currencies. Normalising a ratio
would silently restate `35 %` as `0.35`, and normalising an angle would need the
irrational factor between `°` and `rad`. See `bvn_unit_si_normal_form`.

#### Exactness

The conversion is the same exact path `want_unit` uses, and nothing approximate
is ever delivered. A blanket mode makes every value a conversion candidate,
though, so a single ordinary factor — `42 km/h` is `35/3 m/s`, exact as a
rational with no terminating base-10 expansion — would otherwise reject a
document nobody had a complaint about. `on_inexact = bvnr_inexact_leave` hands
that value over in its native unit instead, visible as `converted == false` and
never as rounded digits. A genuinely irrational factor still aborts, since there
is nothing exact to hand over either way.

#### Validation

`require_unit` and `require_dimension_of` reject a document; they never change a
value. Both are evaluated on the unit the **document** wrote, before any
conversion — validate what you were sent, convert for the consumer — so they mean
the same thing whether or not a conversion was also requested. A failure is
`error_unit_mismatch`.

```c
static const char *lengths[] = { "m" };
bvnr_unit_policy_t p = {0};
p.require_unit             = true;   /* reject any bare number */
p.require_dimension_of     = lengths;/* every value must be a length */
p.num_require_dimension_of = 1;
```

`require_dimension_of` asks about the dimension, not the unit: `5 k~m`, `12 in`
and `3 mi` all satisfy `{"m"}`. A value must satisfy at least one entry. The same
unitless fence applies — a bare number satisfies a requirement only if one of the
listed units is itself `no_unit`.

#### Where this sits relative to `want_unit`

The hook is more specific and wins. For each value the reader asks the hook
first, then the per-field `rules`, then `targets`, and falls back last to
`normalise`. Setting several is a legitimate combination: normalise the
document, name the two fields that need something else, and hand-handle one more
in the hook.

---

### 1.13 Refusing the binary half (`text_only`)

```c
bvnr_read_flags_t f = {0};
f.text_only = true;              /* an octet stream is error_octet_stream_forbidden */
```

Bovnar is a text/binary hybrid: an octet stream carries arbitrary bytes inside an
otherwise readable document, framed by `0x00` and length-prefixed so a reader can
skip it without inspecting it (spec §9). That design is what makes binary payloads
free of escaping and expansion — and it is also the one part of a document that a
*text-shaped* pipeline destroys silently. Line-ending normalisation rewrites a
`0x0D` inside a chunk payload, the length prefix then points at the wrong byte,
and what arrives looks like a malformed document rather than a mangled one. A
`git` checkout without `-text`, a `sed` filter, or a log aggregator that assumes
lines will each do it.

`text_only` lets a consumer state that this channel carries text, and have the
parser enforce it:

```
$ bovnar validate --text-only telemetry.bvnr
Validation failed: octet_stream_forbidden at line 3, col 6
```

This is the same move as `--require-unit`: the format permits something, and a
consumer that cannot accept it says so where it is checkable rather than hoping.
A producer can use it the same way, to guarantee that a channel it publishes
stays transport-safe.

The refusal fires at the stream's **opening `0x00`**, before any payload is read
— the point is that such a document should not have reached this reader at all,
so the earliest refusal is the useful one, and a large binary region is never
touched. The flag changes nothing else: a document without an octet stream parses
identically with it set.

Available on the CLI for `validate` and `events`. `query` refuses the option
rather than ignoring it, because that command parses through the DOM, which takes
a unit policy but not the read flags — an assertion silently not in force is
worse than none.

---

## 2. Writer

---

### 2.1 `bvnr_writer_create` / `bvnr_writer_destroy`

```c
bvnr_writer_t *bvnr_writer_create(void);
void           bvnr_writer_destroy(bvnr_writer_t *w);
```

Allocate and free a writer on the heap. Mirrors the reader lifecycle exactly. Returns `NULL` on allocation failure.

```c
bvnr_writer_t *w = bvnr_writer_create();
if (!w) { perror("alloc"); exit(1); }

/* ... open, write events, finish ... */

bvnr_writer_destroy(w);
```

---

### 2.2 `bvnr_sink_to_fd`

```c
void bvnr_sink_to_fd(bvnr_sink_t *s, int fd);
```

Initialise sink `s` to write serialised bytes to an open, writable POSIX file descriptor. The caller retains ownership of `fd`.

```c
bvnr_sink_t sink;
bvnr_sink_to_fd(&sink, STDOUT_FILENO);
/* or: int fd = open("out.bvnr", O_WRONLY|O_CREAT|O_TRUNC, 0644); */
```

---

### 2.3 `bvnr_sink_to_mem`

```c
void bvnr_sink_to_mem(bvnr_sink_t *s, void *buf, uint64_t cap);
```

Initialise sink `s` to write into a caller-provided memory buffer of `cap` bytes. Writing beyond `cap` produces `error_sink_buffer_exhausted`. Use `bvnr_sink_bytes_written` to query how many bytes were actually written — but see the note there: for output produced by a *writer*, `bvnr_writer_bytes_written` is the one that works.

```c
char out[4096];
bvnr_sink_t sink;
bvnr_sink_to_mem(&sink, out, sizeof(out));
```

---

### 2.4 `bvnr_sink_bytes_written`

```c
uint64_t bvnr_sink_bytes_written(const bvnr_sink_t *s);
```

Return the number of bytes pushed into a memory sink so far. Only meaningful for sinks created with `bvnr_sink_to_mem`.

**Not for writer output.** `bvnr_open_write_sink` takes a *copy* of the sink, so
everything a writer emits advances the copy and never the caller's struct — this
counter stays 0 for the whole lifetime of the writer. Use it only when you push
into the sink yourself. To size writer output, ask the writer:

```c
bvnr_write_finish(w);
uint64_t n = bvnr_writer_bytes_written(w);   /* NOT bvnr_sink_bytes_written(&sink) */
fwrite(out, 1, n, stdout);
bvnr_writer_destroy(w);                       /* read the count before destroying */
```

---

### 2.5 `bvnr_open_write_sink`

```c
bool bvnr_open_write_sink(bvnr_writer_t        *w,
                            const bvnr_sink_t    *sink,
                            bool                  pretty,
                            bvnr_write_flags_t   *options);
```

Attach `sink` to the writer and configure it. Must be called before any `bvnr_write_event`. Returns `false` on invalid arguments.

- `pretty` — when `true`, the serialiser emits newlines and indentation. When `false`, output is compact (single line per assignment, no extra whitespace).
- `options` — configuration struct. Zero-initialise for defaults.

```c
typedef struct bvnr_write_flags_s {
    /* Regrouped for exposition — see include/bovnar.h for the declaration
       order, and zero-initialise rather than relying on this one. ... */

    /* ── Writer enforces these ─────────────────────────────────── */
    uint8_t  max_struct_nesting;      /* 0 → 64 internal default; hard cap 255 */
    uint8_t  max_array_nesting;       /* 0 → 64 internal default; hard cap 255 */
    void    *userdata;
    bool   (*on_event)(void *userdata, bvnr_event_t, bvnr_data_t *);
    bvn_unit_flags_t unit_flags;      /* controls unit annotation format */
    bool     emit_version;            /* emit a leading "#!bovnar M.N" on open */

    /* ── Present for API symmetry with bvnr_read_flags_t;          ─
       the writer does not read or enforce these fields. Set to 0. */
    uint16_t max_identifier_length;
    uint16_t max_string_length;
    uint16_t max_number_length;
    uint16_t max_symbol_length;
    uint16_t max_reference_length;
    uint64_t max_array_items;
    uint64_t max_text_bytes;
    uint64_t max_file_size;
    bool     continue_on_error;       /* no-op in the writer */
    bvnr_on_error_fn on_error;        /* no-op in the writer */

    uint64_t _reserved[4];            /* padding for future fields; leave zero */
} bvnr_write_flags_t;
```

> **Writer limits.** Only `max_struct_nesting`, `max_array_nesting`, `on_event`, `userdata`,
> `unit_flags` and `emit_version` have any effect on the writer. All other fields are present solely to keep
> `bvnr_write_flags_t` structurally parallel to `bvnr_read_flags_t`; they are silently
> ignored. In particular, `continue_on_error`, `on_error`, and all per-token-length fields
> have no effect. The writer never internally limits array items, text bytes, or file size.

`on_event` in the write flags fires for each event as it is serialised — useful for logging or auditing. Pass `NULL` if not needed.

`unit_flags` controls how unit annotations are serialised by the writer. The valid flags are:

| Flag | Value | Effect |
|------|-------|--------|
| `BVN_UNIT_FLAGS_NONE` | `0` | Default: Unicode superscript exponents, no reduction |
| `BVN_UNIT_REDUCE` | `1 << 0` | Reduce compound units to canonical form before serialising |
| `BVN_UNIT_ASCII_EXP` | `1 << 1` | Use `^N` ASCII caret notation instead of Unicode superscripts |
| `BVN_NUM_CANONICAL` | `1 << 2` | Render a *rescaled* float in float shape (`5000.0`, not `5000`), so a value that took the rescale lands on the same spelling as one that did not. What `bovnar pretty-print --canonical` sets beside `BVN_UNIT_REDUCE` |

`BVN_UNIT_REDUCE` folds every prefix out of the unit, and the **writer scales the value to match**: `5` with unit `k~m` is written as `5000 m`, not `5 m`. The rescale runs in exact rational arithmetic, so a value far wider than a double keeps every digit; where the scaled value cannot be written exactly in the value's own base the write fails with `error_value_out_of_range` rather than rounding. The rescaled value must also still satisfy the type it was declared with: an `uint`/`sint` that is no longer integral or no longer fits its width, and a `float_fix` that no longer fits its Q format (`<float_fix:32,q16>` leaves 15 integer bits, so 30000 km in metres does not fit), are refused the same way — the writer must not emit a document this library's own reader rejects. (A direct caller of `bvn_unit_to_string_ex` gets no such help — see the note on that function in §3.3.)

These flags can be OR-combined: `BVN_UNIT_REDUCE | BVN_UNIT_ASCII_EXP`. The flags are fixed at open time. To change serialisation behaviour, destroy the writer and open a new one with the updated `unit_flags`. The getter `bvnr_writer_unit_flags(w)` is used internally by the Python FFI layer to retrieve the live flags before each unit serialisation call; there is no public setter.

```c
bvnr_sink_t sink;
bvnr_sink_to_fd(&sink, fd);

bvnr_write_flags_t opts = { 0 };  /* all defaults */
if (!bvnr_open_write_sink(w, &sink, /*pretty=*/true, &opts))
    return -1;
```

---

### 2.6 `bvnr_open_write_mem`

```c
bool bvnr_open_write_mem(bvnr_writer_t      *w,
                           void               *buf,
                           uint64_t            cap,
                           bool                pretty,
                           bvnr_write_flags_t *options);
```

Convenience wrapper that constructs a memory sink internally and calls `bvnr_open_write_sink`. Equivalent to `bvnr_sink_to_mem` + `bvnr_open_write_sink`. To retrieve the written byte count after finishing, call `bvnr_writer_bytes_written`.

```c
char out[4096];
bvnr_write_flags_t opts = { 0 };
if (!bvnr_open_write_mem(w, out, sizeof(out), false, &opts))
    return -1;
```

---

### 2.7 `bvnr_write_event`

```c
bool bvnr_write_event(bvnr_writer_t *w, bvnr_event_t ev, bvnr_data_t *data);
```

Emit one event to the writer. Every value in the output goes through this function: the helpers in [§2.16](#216-value-helpers) and the multiprecision writers in §2.12–2.13 are sequences of these events, and a caller replaying a reader's stream calls it directly. It serialises the event and the data it describes directly into the configured sink.

Returns `true` on success, `false` on any serialisation error.

The event sequence you must emit for every assignment mirrors exactly what the reader delivers to `on_verified`. At minimum, for a typed scalar value:

```
ev_assignment_start          (data->data = key, data->length = key length)
ev_type_annotation_start     (data->data = annotation text, or NULL for no annotation)
ev_type_annotation_type_family
ev_type_annotation_type_family_parameter   (for each parameter)
ev_type_annotation_end
ev_data                      (data->data = value string, data->value_type/value_unit set)
```

The `BVN_TYPE_*` macros build `value_type_spec_t` literals conveniently:

```c
#define BVN_TYPE_PLAIN          ((value_type_spec_t){ .family = vt_plain })
#define BVN_TYPE_UTF8           ((value_type_spec_t){ .family = vt_utf8  })
#define BVN_TYPE_BOOL           ((value_type_spec_t){ .family = vt_bool  })
#define BVN_TYPE_UINT(w)        ((value_type_spec_t){ .family = vt_uint,      .width = (w) })
#define BVN_TYPE_SINT(w)        ((value_type_spec_t){ .family = vt_sint,      .width = (w) })
#define BVN_TYPE_FLOAT(w)       ((value_type_spec_t){ .family = vt_float,     .width = (w) })
/* float_fix: .base is repurposed to store Q (fractional bits). */
#define BVN_TYPE_FLOAT_FIX(w,q) ((value_type_spec_t){ .family = vt_float_fix, .width = (w), .base = (q) })
/* float_dec: base field is unused (always 0).                   */
#define BVN_TYPE_FLOAT_DEC(w)   ((value_type_spec_t){ .family = vt_float_dec, .width = (w) })
/* float with explicit numeral base (for base-16 output):        */
#define BVN_TYPE_FLOAT_BASE(w,b) ((value_type_spec_t){ .family = vt_float, .width = (w), .base = (b) })
/* uint/sint with explicit numeral base:                         */
#define BVN_TYPE_UINT_BASE(w,b) ((value_type_spec_t){ .family = vt_uint, .width = (w), .base = (b) })
#define BVN_TYPE_SINT_BASE(w,b) ((value_type_spec_t){ .family = vt_sint, .width = (w), .base = (b) })
```

The maximum bit-width accepted for `uint` and `sint` is `BVN_MAX_INT_WIDTH` (defined as `32768u` in `bovnar.h`). The validator and writer reject any declared `uint`/`sint` width exceeding this limit with `error_illegal_value_type`.

> **Critical:** The writer dispatches `ev_type_annotation_type_family_parameter` events on `d->type`, not on `d->value_type`. For each parameter event, `d.type` must be set to the appropriate `token_type_t` value: `token_is_type_width` for the width parameter, `token_is_type_base` for the base parameter, `token_is_type_q` for the Q (fractional bits) parameter of `float_fix`, and `token_is_unit` for the unit parameter. An unrecognised `d.type` causes the writer to emit nothing for that event — the annotation will be silently incomplete. **Use `bvnr_write_type_annotation` (see §3.1) to avoid this complexity entirely.**

**Example: write `.port = <uint:16> 8080;`**

```c
/* Helper: emit a fully-typed uint16 assignment */
static bool write_uint16(bvnr_writer_t *w,
                          const char *key, uint16_t value)
{
    char valbuf[16];
    snprintf(valbuf, sizeof(valbuf), "%" PRIu16, value);

    value_type_spec_t vt = BVN_TYPE_UINT(16);
    value_unit_t      vu = BVN_UNIT_NONE;

    bvnr_data_t d;

    /* 1. Assignment start — key */
    d = (bvnr_data_t){ .data = (void*)key, .length = (uint32_t)strlen(key) };
    if (!bvnr_write_event(w, ev_assignment_start, &d)) return false;

    /* 2. Type annotation — use bvnr_write_type_annotation for the full sequence */
    if (!bvnr_write_type_annotation(w, vt, vu)) return false;

    /* 3. Value */
    d = (bvnr_data_t){
        .type       = token_is_number,
        .value_type = vt,
        .value_unit = vu,
        .data       = valbuf,
        .length     = (uint32_t)strlen(valbuf),
    };
    return bvnr_write_event(w, ev_data, &d);
}
```

**Example: write a string assignment `.host = "localhost";`**

```c
static bool write_string(bvnr_writer_t *w,
                           const char *key, const char *value)
{
    value_type_spec_t vt = BVN_TYPE_UTF8;
    value_unit_t      vu = BVN_UNIT_NONE;
    bvnr_data_t d;

    d = (bvnr_data_t){ .data = (void*)key, .length = (uint32_t)strlen(key) };
    if (!bvnr_write_event(w, ev_assignment_start, &d)) return false;

    d = (bvnr_data_t){ .value_type = vt };
    if (!bvnr_write_event(w, ev_type_annotation_start, &d)) return false;
    if (!bvnr_write_event(w, ev_type_annotation_type_family, &d)) return false;
    if (!bvnr_write_event(w, ev_type_annotation_end, &d)) return false;

    d = (bvnr_data_t){
        .type       = token_is_string,   /* required: says how the value is spelled */
        .value_type = vt,
        .data       = (void*)value,
        .length     = (uint32_t)strlen(value),
    };
    return bvnr_write_event(w, ev_data, &d);
}
```

The `ev_data` event's `type` is not optional. Left at zero it is
`token_is_identifier`, which is not a value, and the write fails with
`error_type_value_mismatch` after the annotation has already gone out.

**Example: open and close a struct**

```c
static bool write_struct_start(bvnr_writer_t *w, const char *key)
{
    bvnr_data_t d = { .data = (void*)key, .length = (uint32_t)strlen(key) };
    if (!bvnr_write_event(w, ev_assignment_start, &d)) return false;
    return bvnr_write_event(w, ev_struct_start, &(bvnr_data_t){0});
}

static bool write_struct_end(bvnr_writer_t *w)
{
    return bvnr_write_event(w, ev_struct_end, &(bvnr_data_t){0});
}
```

---

### 2.8 `bvnr_write_version`

```c
bool bvnr_write_version(bvnr_writer_t *w, uint16_t major, uint16_t minor);
```

Emit a leading `#!bovnar <major>.<minor>` version directive (spec §3.4). Must be
called immediately after `bvnr_open_write_*` and before any value; calling it
once output has begun is `error_invalid_argument`. Use it to round-trip a
directive read from a source document, or pass `BVNR_SPEC_VERSION_MAJOR` /
`BVNR_SPEC_VERSION_MINOR` to stamp the current spec version. Setting
`bvnr_write_flags_t.emit_version` is equivalent to calling it with the current
spec version right after open.

```c
bvnr_open_write_sink(w, &sink, true, NULL);
bvnr_write_version(w, 1, 1);          /* "#!bovnar 1.1\n" */
bvnr_write_uint(w, "port", 16, 443);
bvnr_write_finish(w);
```

---

### 2.9 `bvnr_write_finish`

```c
bool bvnr_write_finish(bvnr_writer_t *w);
```

Flush any buffered output and finalise the stream. Must be called after all `bvnr_write_event` calls and before `bvnr_writer_destroy`. Returns `false` if any struct or array is still open, or an assignment, a type annotation or a `/` row separator was left with nothing after it (`error_got_incomplete_bvnr_stream`), if writing the trailing semicolon fails, or if flushing the write buffer to the sink fails.

**64 KiB write buffer.** The writer accumulates output into an internal 64 KiB buffer. Individual `bvnr_write_event` calls do not push bytes to the sink immediately; instead bytes accumulate in the buffer and are forwarded to the sink only when the buffer is full or when `bvnr_write_finish` is called. This means the sink receives large contiguous writes rather than one tiny push per token, which is important for fd-based sinks. `bvnr_writer_bytes_written` always reflects the true total of bytes handed off to the sink plus bytes still in the buffer, so the count is accurate at any point during writing.

```c
if (!bvnr_write_finish(w)) {
    fprintf(stderr, "write finish failed: %s\n",
            bvn_error_to_string(bvnr_writer_get_error(w)));
}
bvnr_writer_destroy(w);
```

---

### 2.10 `bvnr_writer_get_error` and friends

```c
error_code_t     bvnr_writer_get_error       (const bvnr_writer_t *w);
uint64_t         bvnr_writer_get_error_offset(const bvnr_writer_t *w);
uint64_t         bvnr_writer_bytes_written   (const bvnr_writer_t *w);
bvn_unit_flags_t bvnr_writer_unit_flags      (const bvnr_writer_t *w);
```

The writer error API is smaller than the reader's: there are **no** `bvnr_writer_get_error_line` or `bvnr_writer_get_error_column` functions. The writer has no lexer and therefore cannot track source positions. Use `bvnr_writer_get_error_offset` (byte count into the output stream) and `bvnr_writer_get_error` (error code) for diagnostics. `bvnr_writer_bytes_written` returns the total bytes emitted to the sink so far — available at any point, not only after errors.

`bvnr_writer_unit_flags` returns the `bvn_unit_flags_t` bitmask currently stored in the writer object (set via `bvnr_write_flags_t.unit_flags` at open time). The writer uses these flags whenever it serialises a unit annotation string (via `bvn_unit_to_string_ex`). This function is primarily used by the Python bindings FFI layer to retrieve the live flags before each unit serialisation call.

```c
if (!bvnr_write_event(w, ev_data, &d)) {
    fprintf(stderr, "write error: %s at offset %" PRIu64 "\n",
            bvn_error_to_string(bvnr_writer_get_error(w)),
            bvnr_writer_get_error_offset(w));
    return -1;
}
```

---

### 2.11 `bvn_format_uint64` / `bvn_format_int64` / `bvn_format_double`

```c
int32_t bvn_format_uint64(char *buf, size_t bufsize,
                            uint64_t value, uint32_t base, uint32_t min_digits);

int32_t bvn_format_int64(char *buf, size_t bufsize,
                           int64_t value, uint32_t base, uint32_t min_digits);

int32_t bvn_format_double(char *buf, size_t bufsize,
                            double value, value_type_spec_t vt);
```

Produce the value string that goes into `bvnr_data_t.data` when writing numeric values. All three return the number of bytes written (excluding NUL terminator), or `-1` on buffer overflow.

- `base` — numeric base (2–62, 64, 85). Use `10` for the common case.
- `min_digits` — zero-pad to at least this many digits. Pass `0` for no padding.
- For `bvn_format_double`, the type spec `vt` controls the output precision according to `vt.width`. Because the input is a C `double`, the effective precision is capped at the 64-bit format (a wider `vt.width` yields no extra digits); render the full precision of a 128/256-bit value with the arbitrary-precision writer `bvnr_write_bvnf_base` instead.

```c
char buf[32];

/* Format 255 as a base-16 value, minimum 2 digits → "ff" */
int32_t n = bvn_format_uint64(buf, sizeof(buf), 255, 16, 2);
/* buf = "ff", n = 2 */

/* Format 9.81 as a 64-bit float */
value_type_spec_t vt = BVN_TYPE_FLOAT(64);
n = bvn_format_double(buf, sizeof(buf), 9.81, vt);
/* buf = "9.81e+0", n = 7 — the writer emits an exponent for every non-zero
   value (zero is written as "0.0" / "-0.0"), so a float always re-reads as a
   float and never as an integer carrier */
```

These strings are then placed into `bvnr_data_t.data` / `.length` before calling `bvnr_write_event(w, ev_data, &d)`.

---

### 2.12 `bvnr_write_bvnf_base` / `bvnr_write_bvnf_base_unit`

```c
bool bvnr_write_bvnf_base(bvnr_writer_t *w, const char *key,
                            const bvn_float_t *f,
                            uint32_t width, uint32_t base);

bool bvnr_write_bvnf_base_unit(bvnr_writer_t *w, const char *key,
                                 const bvn_float_t *f,
                                 uint32_t width, uint32_t base,
                                 value_unit_t unit);
```

Write an arbitrary-precision `bvn_float_t` value at any valid `float` width (0, 16, or any multiple of 32 up to 32768) and in either base 10 or base 16. The float string is generated internally by `bvn_float_to_str` and then validated by the writer before being sent to the sink — the call fails and sets an error if the generated string does not satisfy the writer's type constraints.

`bvnr_write_bvnf_base` is the no-unit variant; `bvnr_write_bvnf_base_unit` attaches a physical unit to the type annotation.

**Base 10** — the value string uses the same decimal format as `bvn_format_double` and is emitted as a bare number token (`token_is_number`). The type annotation is `<float:W>` or `<float:W,_10>`.

**Base 16** — the value string uses a binary-exponent hexadecimal format: `[−]D.DDDDp[+|−]EEE` where `D.DDDD` are lowercase hex mantissa digits and `EEE` is the decimal binary exponent. The string is emitted as a quoted string token (`token_is_string`) because non-decimal literals must be quoted in BVNR. The type annotation is `<float:W,_16>`.

```c
#include "bvn_float.h"

bvn_float_t *f = bvn_float_alloc(1024u);
bvn_float_from_str(f, "3.14159265358979323846", 10);

/* .pi_dec = <float:1024> 3.14... ; */
bvnr_write_bvnf_base(w, "pi_dec", f, 1024u, 10u);

/* .pi_hex = <float:256,_16> "1.921fb54442d18p+1"; */
bvn_float_t *g = bvn_float_alloc(256u);
bvn_float_from_double(g, 3.14159265358979323846);
bvnr_write_bvnf_base(w, "pi_hex", g, 256u, 16u);

bvn_float_free(f);
bvn_float_free(g);
```

`width` 0 uses the precision stored in the `bvn_float_t` itself. An invalid width (not 0, not 16, not a positive multiple of 32, or greater than `BVN_FLOAT_MAX_PREC`) causes the call to return `false` with `error_illegal_value_type`.

These functions supersede calling `bvnr_write_bvnf` / `bvnr_write_bvnf_unit` when base 16 output or widths greater than 128 are needed. `bvnr_write_bvnf` and `bvnr_write_bvnf_unit` remain available as convenience wrappers that always use base 10.

---

### 2.13 `bvnr_write_bvni` / `bvnr_write_bvni_unit`

```c
bool bvnr_write_bvni(bvnr_writer_t *w, const char *key,
                      const bvn_int_t *n,
                      uint32_t width, uint32_t base);

bool bvnr_write_bvni_unit(bvnr_writer_t *w, const char *key,
                            const bvn_int_t *n,
                            uint32_t width, uint32_t base,
                            value_unit_t unit);
```

Write an arbitrary bit-width integer. The type family (`uint` or `sint`) is determined by the `negative` flag of the `bvn_int_t`: negative values produce `<sint:W,...>`, non-negative values produce `<uint:W,...>`. Any numeric base supported by the writer (2–62, 64, 85) may be specified.

The integer string is generated by `bvn_int_to_str` and then validated before reaching the sink. For non-decimal bases the string is emitted as a quoted token (`token_is_string`); for base 10 it is emitted as a bare number token (`token_is_number`).

```c
#include "bvn_int.h"

bvn_int_t *n = bvn_int_alloc();

/* 128-bit unsigned max in decimal */
bvn_int_from_str(n, "340282366920938463463374607431768211455", 10);
bvnr_write_bvni(w, "u128max", n, 128u, 10u);

/* 256-bit value in hex — emitted as <uint:256,_16> "deadbeef..."; */
bvn_int_from_str(n, "deadbeefcafebabe0011223344556677"
                    "8899aabbccddeeff0011223344556677", 16);
bvnr_write_bvni(w, "wide_hex", n, 256u, 16u);

/* signed negative in hex */
bvn_int_from_str(n, "-7fffffff", 16);
bvnr_write_bvni(w, "neg_hex", n, 32u, 16u);

bvn_int_free(n);
```

`width` 0 defaults to 64 bits for range validation. A `width` that cannot hold the value causes the call to return `false` with `error_value_out_of_range`.

---

### 2.14 `BVN_TYPE_FLOAT_BASE`

```c
#define BVN_TYPE_FLOAT_BASE(w, b)  /* value_type_spec_t */
```

Convenience macro that constructs a `value_type_spec_t` for a `float` with explicit width `w` and base `b`. The existing `BVN_TYPE_FLOAT(w)` macro always yields base 0 (equivalent to base 10); `BVN_TYPE_FLOAT_BASE` is needed when base 16 must be encoded in the type spec before being passed to `bvnr_write_type_annotation` or `bvnr_write_event` directly.

```c
value_type_spec_t vt16 = BVN_TYPE_FLOAT_BASE(256u, 16u);
/* → { .family = vt_float, .width = 256, .base = 16 } */

value_type_spec_t vt10 = BVN_TYPE_FLOAT_BASE(64u, 10u);
/* equivalent to BVN_TYPE_FLOAT(64) */
```

### 2.15 Writer-side unit policy (`bvnr_writer_set_unit_policy`)

```c
bool bvnr_writer_set_unit_policy(bvnr_writer_t *w, const bvnr_unit_policy_t *p);
```

The producing half of the contract in 1.12: refuse to **emit** a value a reader
under the same policy would refuse to read.

This is the half the format's promise rests on. A reader can only reject a
document somebody already wrote; only the writer can stop a bare number reaching
a file in the first place, which is what "hand the file to anyone and they have
everything required to interpret it" actually depends on.

```c
static const char *lengths[] = { "m" };
bvnr_unit_policy_t p = {0};
p.require_unit             = true;    /* no bare numeric values */
p.require_dimension_of     = lengths; /* and every one of them a length */
p.num_require_dimension_of = 1;
bvnr_writer_set_unit_policy(w, &p);
```

`bvnr_write_event` (and every `bvnr_write_*` helper above it) then fails with
`error_unit_mismatch`, latched into the writer like any other write error and
readable with `bvnr_writer_get_error`. The unit checked is the one the value
will carry in the output — whether it was given inline on the value or as a
parameter of its type annotation, and a single annotation covers every element
of the array under it.

**Validation only.** `require_unit`, `require_dimension_of` and per-field rules
in `bvnr_rule_require` mode are accepted; a policy carrying `targets`,
`normalise`, `base`, `on_inexact` or a `bvnr_rule_convert` rule is rejected
(`false`) rather than half-honoured. The writer already has a
value-rewriting mode — `BVN_UNIT_REDUCE` in `bvnr_write_flags_t.unit_flags`,
which folds prefixes out and rescales the value exactly — and a second one
arriving through a different door, with different rules about exactness, is how
two features end up disagreeing about what a document says.

Like the reader's, the policy may be set before or after `bvnr_open_write_*`,
survives re-opening the writer on another document, and is cleared with `NULL`.

---

### 2.16 Value helpers

One call per assignment: the key, its type annotation and its value, for the
common scalar types. Each is the event sequence of §2.7 written out for you, and
is what most producers should use.

```c
bool bvnr_write_string(bvnr_writer_t *w, const char *key, const char *value);
bool bvnr_write_plain (bvnr_writer_t *w, const char *key, const char *value);
bool bvnr_write_null  (bvnr_writer_t *w, const char *key);
bool bvnr_write_bool  (bvnr_writer_t *w, const char *key, bool value);
bool bvnr_write_uint(bvnr_writer_t *w, const char *key,
                     uint32_t width, uint64_t value);
bool bvnr_write_sint(bvnr_writer_t *w, const char *key,
                     uint32_t width, int64_t value);
bool bvnr_write_float(bvnr_writer_t *w, const char *key,
                      uint32_t width, double value);
bool bvnr_write_float_fix(bvnr_writer_t *w, const char *key,
                          uint32_t width, uint32_t q, double value);
bool bvnr_write_float_dec(bvnr_writer_t *w, const char *key,
                          uint32_t width, double value);

bool bvnr_write_uint_unit(bvnr_writer_t *w, const char *key,
                          uint32_t width, uint64_t value, value_unit_t unit);
bool bvnr_write_sint_unit(bvnr_writer_t *w, const char *key,
                          uint32_t width, int64_t value, value_unit_t unit);
bool bvnr_write_float_unit(bvnr_writer_t *w, const char *key,
                           uint32_t width, double value, value_unit_t unit);
bool bvnr_write_float_fix_unit(bvnr_writer_t *w, const char *key,
                               uint32_t width, uint32_t q,
                               double value, value_unit_t unit);
bool bvnr_write_float_dec_unit(bvnr_writer_t *w, const char *key,
                               uint32_t width,
                               double value, value_unit_t unit);

bool bvnr_write_struct_start(bvnr_writer_t *w, const char *key);
bool bvnr_write_struct_end  (bvnr_writer_t *w);
```

| Helper | Writes | Notes |
|--------|--------|-------|
| `bvnr_write_string` | `.key = "value";` | `value` is escaped for you. It must be valid UTF-8 and may not hold a C0 control other than `\t` `\n` `\v` `\f` `\r`, nor DEL (`error_unexpected_input_byte`, `error_invalid_utf8_byte`) |
| `bvnr_write_plain` | `.key = value;` | a bare **symbol** — `value` must be a valid symbol, it is not quoted |
| `bvnr_write_null` | `.key = ;` | the null value |
| `bvnr_write_bool` | `.key = true;` / `false` | |
| `bvnr_write_uint` / `_sint` | `.key = <uint:16> 443;` | `width` 0 writes `<uint>` / `<sint>` (64 bits); a value that does not fit `width` is `error_value_out_of_range` |
| `bvnr_write_float` | `.key = <float:64> 1e-1;` | `width` 0 writes `<float>`; `nan`, `inf` and `ninf` are written as those words; formatted as by `bvn_format_double` (§2.11) |
| `bvnr_write_float_fix` | `.key = <float_fix:32,q16> 1.5e+0;` | `q` is the number of fractional bits |
| `bvnr_write_float_dec` | `.key = <float_dec:64> 1.25e+0;` | |
| `…_unit` | `.key = <float:64,k~m> 1e+0;` | the same, with `unit` as the annotation's unit parameter; honours `bvnr_writer_set_unit_source` (§3.1) and the writer's unit policy (§2.15) |
| `bvnr_write_struct_start` / `_end` | `.key = {` … `};` | every helper called between the two writes a member |

All return `false` on failure with the reason latched in the writer
(`bvnr_writer_get_error`); once a write has failed, every later one fails too.
A `double` carries 53 significant bits whatever `width` says — for the full
precision of a wider `float`, `float_dec` or `float_fix`, or an integer past 64
bits, use `bvnr_write_bvnf_base` (§2.12) and `bvnr_write_bvni` (§2.13). A
timestamp has its own helper, `bvnr_write_datetime` (§1.9). There is no array
helper: an array is written as events (`ev_array_row_start`, one `ev_data` per
element, `ev_array_row_end`).

```c
bvnr_write_string(w, "host", "localhost");       /* .host = "localhost";      */
bvnr_write_uint  (w, "port", 16, 443);           /* .port = <uint:16> 443;    */
bvnr_write_struct_start(w, "limits");            /* .limits = {               */
bvnr_write_bool  (w, "strict", true);            /*     .strict = true;       */
bvnr_write_null  (w, "owner");                   /*     .owner = ;            */
bvnr_write_struct_end(w);                        /* };                        */
```

---

## 3. Shared

---

### 3.1 `bvnr_write_type_annotation`

```c
bool bvnr_write_type_annotation(bvnr_writer_t *w,
                                 value_type_spec_t vt,
                                 value_unit_t vu);
```

Emit a complete type-annotation event sequence (`ev_type_annotation_start`, `ev_type_annotation_type_family`, zero or more `ev_type_annotation_type_family_parameter` events, `ev_type_annotation_end`) in a single call. Returns `false` on any serialisation error.

This is the **preferred** way to write type annotations. Using `bvnr_write_event` directly for parameter events requires setting `d.type` to the appropriate `token_type_t` constant for each parameter; `bvnr_write_type_annotation` handles this correctly and automatically.

The function emits parameters as follows:

- **Width** — emitted for numeric families when `vt.width != 0`. A width of `0` is **not** written to the stream (the absence implies the default width of 64 on the reader side via `bvn_effective_width`). Also emitted for `datetime` when `vt.width != 0`. It is **not** emitted for `utf8` or `bool`, which are parameterless — a width on them is rejected by the type-spec validator (`error_illegal_value_type`).
- **Base** — emitted for `float` when `vt.base` is non-zero and not `10`; emitted for `uint`/`sint` when `vt.base` is non-zero and not `10`.
- **Q** — emitted for `float_fix` when `vt.base` (which stores Q) is non-zero. A Q value of `0` is therefore not written explicitly.
- **Unit** — emitted when `vu.num_components > 0`. `BVN_UNIT_NONE` (num_components == 0) produces no unit parameter; a lone `bu_none` component — `BVN_UNIT_NO_PREFIX(bu_none)`, num_components == 1 — is normalised to `BVN_UNIT_NONE` on entry and likewise produces **no** unit parameter.

```c
value_type_spec_t vt = BVN_TYPE_FLOAT(64);
value_unit_t      vu = BVN_UNIT_COMPOUND2(
                           bu_meter,  si_none, exp_linear,
                           bu_second, si_none, exp_neg_square);

/* Emits: <float:64,m/s²> */
if (!bvnr_write_type_annotation(w, vt, vu)) return false;
```

#### Writing a unit the way it was spelled (`bvnr_writer_set_unit_source`)

```c
bool bvnr_writer_set_unit_source(bvnr_writer_t *w,
                                 const char *text, uint32_t length);
```

`vu` is a `value_unit_t`, and a `value_unit_t` has nowhere to hold a spelling,
so the unit is written in canonical form: a producer holding `ucum:mL{total}`
gets `m~L`. `bvnr_writer_set_unit_source` hands the spelling over separately.
The **next** `bvnr_write_type_annotation` — which every `bvnr_write_*_unit`
helper calls — emits `text` for its unit and then forgets it:

```c
bool ok = false;
value_unit_t mL = bvn_parse_unit((const uint8_t *)"m~L", &ok);

bvnr_write_version(w, 1, 2);                       /* profile notation needs it */
bvnr_writer_set_unit_source(w, "ucum:mL{total}", 14);
bvnr_write_float_unit(w, "volume", 64, 1.5, mL);   /* <float:64,ucum:mL{total}> */
bvnr_write_float_unit(w, "other",  64, 2.5, mL);   /* <float:64,m~L> — one shot */
```

It is one shot, and for the **next value only**. If that value takes no unit — a
string, a bool, a struct, a bare number — the spelling does not travel on to
whichever later value happens to carry one: the write fails with
`error_unit_mismatch` (for a helper that writes no annotation at all, at the
key that follows it, or at `bvnr_write_finish` if there is none). It is also dropped when a write fails, when the writer is
re-opened, and by passing `NULL` or a zero length. The text is copied.

A caller driving `bvnr_write_event` directly needs no setter: set
`bvnr_data_t.unit_source` on the unit-parameter event (annotation slot) or on
the data event (inline suffix). Either way the spelling is **checked before a
byte of it is written**, because a spelling that disagrees with the unit is a
document that says two things:

| The spelling… | Error |
|---|---|
| parses to a different unit than the one given | `error_unit_mismatch` |
| does not parse | whatever `bvn_unit_error_code` says: `error_unit_illegal`, `error_unit_profile_unknown`, `error_unit_profile_unsupported` |
| is longer than `BVNR_UNIT_SOURCE_MAX` (255) bytes, or than what is left of an annotation's 255 after the family and the parameters before it | `error_unit_too_long` |
| is profile notation and no `#!bovnar 1.2` directive was emitted | `error_unsupported_spec_version` |
| would not be read back as itself in that unit slot | `error_unit_illegal` |
| was set for a value that turned out to carry no unit | `error_unit_mismatch` |
| is headed for an inline suffix on an array element | `error_unit_illegal` — the format has no inline unit inside `[…]` |

The unit parameter's `data` text is held to the same table. It is what the
reader fills on that event and what every caller before 2.0 passed, and the
writer emits it when there is no `unit_source` — so it has to name
`value_unit` too, and where both fields are set they must agree. (A datetime's
parameter is its epoch, and there the one text accepted is the epoch's name.)
"`value_unit`" here is the parameter event's own, or — for a caller that sets
the unit only on `ev_type_annotation_start` and nothing but `type` on the
parameter events — the one given there. A unit named only on the start event
is written as the annotation's unit parameter; a parameter event naming a
different one, and a second unit parameter, are refused.

**A value agrees with its annotation.** The serialiser writes the unit the
annotation names and never the value's, so a data event that follows an
annotation naming a unit must carry that unit or none. `<float:64,k~g>`
followed by a value whose `value_unit` is metres is `error_unit_mismatch`; it
used to be written, as kilograms. The annotation governs everything written
under it: each element of an annotated array, in each `/`-joined row, is held
to it in the same way, and `<float:64,no_unit>` refuses a value that has a
unit.

**One document can be read and not written back:** a bare value with an inline
unit of some 243 bytes or more. The reader delivers an inline unit inside a
synthesised annotation (`<float:64,_10,…>`), which is a dozen bytes longer than
the slot the unit came from; where that no longer fits in the annotation's 255,
the replay is refused with `error_unit_too_long`.

The fifth row is the one a unit parser cannot answer, and it is answered by
reading the text back through the real reader. The two unit slots end in
different places: `ucum:mL{a,b}` and `udunits:kg m-2 s-1` are each one
annotation parameter and neither is a legal inline suffix, so the same spelling
is written in one slot and refused in the other.

**A unit flag wins.** With `BVN_UNIT_REDUCE` or `BVN_UNIT_ASCII_EXP` set in
`bvnr_write_flags_t.unit_flags`, `unit_source` is ignored, unchecked, and the
flagged form is written: those flags ask for a particular spelling themselves.
The `data` text is still checked there, in the form the flag produces — it may
be `m` for a `value_unit` of `k~m` under `BVN_UNIT_REDUCE`, and may not be `s`.

---

### 3.2 `bvn_parse_unit` / `bvn_parse_unit_n`

```c
value_unit_t bvn_parse_unit  (const uint8_t *unit, bool *ok);
value_unit_t bvn_parse_unit_n(const uint8_t *unit, uint32_t len, bool *ok);
```

Parse a compound unit string (e.g. `"k~g·m/s²"`) into a `value_unit_t`. Both set `*ok` to `false` and return a zeroed unit on any error.

`bvn_parse_unit` requires a NUL-terminated string. `bvn_parse_unit_n` accepts a length `len` and does **not** require a NUL terminator — use this variant when the unit string is a substring of a larger buffer (as is the case inside the parser itself).

This is useful when reading: after `ev_type_annotation_type_family_parameter`, the unit is already parsed for you in `d->value_unit`. You only need `bvn_parse_unit` / `bvn_parse_unit_n` if you are constructing a unit from a string yourself (e.g. from a config or CLI argument).

The validator also calls `bvn_parse_unit_n` internally when processing an **inline unit suffix** (the optional unit token that may follow a scalar value before its terminating `;`). You do not need to call either function yourself to consume inline units; the parsed result is automatically placed in `d->value_unit` of the `ev_data` event, exactly as for annotation-specified units.

```c
bool ok;
value_unit_t u = bvn_parse_unit((const uint8_t *)"k~g·m/s²", &ok);
if (!ok) {
    fprintf(stderr, "bad unit\n");
    return;
}
/* u now holds { num_components=3, [{bu_gram,exp_linear,si_kilo},
                                    {bu_meter,exp_linear,si_none},
                                    {bu_second,exp_neg_square,si_none}] } */

/* Length-bounded variant — no NUL needed */
const uint8_t *annotation = (const uint8_t *)"float:64,m/s";
value_unit_t u2 = bvn_parse_unit_n(annotation + 9, 3, &ok); /* "m/s" */
```

---

### 3.3 `bvn_unit_to_string` / `bvn_unit_to_string_ex`

```c
int32_t bvn_unit_to_string(value_unit_t u, char *buf, size_t bufsize);

int32_t bvn_unit_to_string_ex(value_unit_t u, char *buf, size_t bufsize,
                               bvn_unit_flags_t flags);
```

Serialise a `value_unit_t` back into its canonical string form. Numerator components are joined by `·`, followed by `/` and denominator components joined by `·`. Returns bytes written (excluding NUL), or `-1` on buffer overflow.

`bvn_unit_to_string` is equivalent to calling `bvn_unit_to_string_ex` with `flags = BVN_UNIT_FLAGS_NONE`.

`bvn_unit_to_string_ex` accepts a `bvn_unit_flags_t` bitmask that controls output format:

| Flag | Effect |
|------|--------|
| `BVN_UNIT_FLAGS_NONE` | Default: Unicode superscript exponents, no reduction |
| `BVN_UNIT_REDUCE` | Reduce compound unit to canonical named SI unit before serialising |
| `BVN_UNIT_ASCII_EXP` | Use `^N` ASCII caret notation instead of Unicode superscripts |

These flags can be OR-combined. The writer uses `bvn_unit_to_string_ex` internally, passing the flags from `bvnr_writer_unit_flags(w)`.

> **`BVN_UNIT_REDUCE` here returns only the reduced UNIT.** `bvn_unit_to_string_ex` discards the scale `bvn_unit_reduce` folded out, so `k~g` serialises as `"g"` — a string denoting a quantity 1000× smaller than the unit passed in. The writer above moves the value with it; nothing else does.
>
> **A direct caller must not reach for `bvn_unit_reduce`'s `scale` to do it.** That scale is to the *fully reduced* unit, and this function does not always emit the fully reduced unit: where the reduction lands on a named SI unit the formatter re-attaches the prefix, so `k~N` comes back `"k~N"` with nothing to rescale while `bvn_unit_reduce` still reports 1000 — and `k~g` comes back `"g"`, where the 1000 must be applied. The two are indistinguishable from outside, both being a lone unit with a kilo prefix, so the scale has to come from the unit that is actually emitted: format with this function, parse the result back, and convert from the original to *that*. The [unit-system reference](05_bovnar_unit_system.md) §12.2 gives the eight lines, which are the ones `bvn_ser_reduced_number` runs. The collapse never substitutes one named unit for another: `Sv` stays `Sv`, `Bq` stays `Bq`. A lone base at an exponent other than 1 still collapses, though — `s⁻¹` becomes `Hz` and `m~s⁻¹` becomes `k~Hz`. And a reduction that **overflows** (a summed exponent outside ±100, too many surviving bases, or a scale out of float range) returns `-1`: it drops a component, so what it produces is a different unit rather than a shorter spelling of the same one.

> **Note on writer usage:** When driving the writer manually via `bvnr_write_event`, do **not** pass a unit string in `bvnr_data_t.data` for the `ev_type_annotation_start` event — the serialiser ignores that field and derives the annotation from `data->value_type` and the subsequent parameter events. Use `bvnr_write_type_annotation` (§3.1) to emit a complete, correct type annotation in one call.

```c
value_unit_t u = BVN_UNIT_COMPOUND2(
    bu_gram,   si_kilo,  exp_linear,
    bu_second, si_none,  exp_neg_square);

char buf[64];
int32_t n = bvn_unit_to_string(u, buf, sizeof(buf));
/* buf = "k~g/s²", n = 7 */

n = bvn_unit_to_string_ex(u, buf, sizeof(buf), BVN_UNIT_ASCII_EXP);
/* buf = "k~g/s^2", n = 7 */
```

---

### 3.4 `bvn_unit_convert_value` *(bovnar_si_units.h)*

```c
bool bvn_unit_convert_value(double value, value_unit_t from,
                            value_unit_t to, double *out);
```

Convert one numeric quantity from unit `from` into unit `to`, writing the result
to `*out`. Handles both the simple multiplicative case (`5 k~m → 5000 m`) and the
affine case (`25 °C → 298.15 K`), routing the latter through SI base units. It
answers the same question the reader's `want_unit` hook (§1.10) does — may this
value be delivered in that unit, and what is it there — in `double`, and is the
C equivalent of the Python `convert_value`. It is **not** the routine the hook
runs: the hook works in exact rationals (`bvn_unit_convert_rational`, below).

Returns `false` — leaving `*out` untouched — when the two units are
dimensionally **incompatible** or have no SI mapping. That boolean is the
"validly convert only" guard; the same verdict from the exact engine is what
the reader reports as `error_unit_mismatch`.

```c
double m;
if (bvn_unit_convert_value(5.0,
        bvn_parse_unit((const uint8_t *)"k~m", &ok),
        bvn_parse_unit((const uint8_t *)"m",   &ok), &m))
    printf("%.0f m\n", m);          /* 5000 m */
```

For the standalone factor (without applying it) or to detect the affine case,
see `bvn_unit_convert_factor` in `bovnar_si_units.h`.

`bvn_unit_convert_value` works in `double`, so it is lossy for wide values. For a
**lossless** conversion — the engine behind the reader's `want_unit` hook (§1.10) —
use the exact-rational pair, also in `bovnar_si_units.h`:

```c
bool    bvn_unit_convert_rational(const bvn_int_t *vnum, const bvn_int_t *vden,
                                  value_unit_t from, value_unit_t to,
                                  bvn_int_t *out_num, bvn_int_t *out_den,
                                  bool *exact);
int32_t bvn_rational_to_str(const bvn_int_t *num, const bvn_int_t *den,
                            uint32_t base, char *buf, size_t bufsize, bool *exact);
size_t  bvn_rational_str_bufsize(const bvn_int_t *num, const bvn_int_t *den,
                                 uint32_t base);
bool    bvn_rational_base_valid(uint32_t base);   /* static inline */
```

`bvn_unit_convert_rational` converts the exact rational `vnum/vden` (parse a wire
value into one with `bvn_float_parse_rational` for floats, or `bvn_int_from_str`
for integers) from `from` into `to`, writing the exact reduced result to
`out_num`/`out_den`. It returns `false` for dimensionally incompatible units, and
sets `*exact = false` when the true factor is irrational (π-based angles) — the
result is then only an approximation and a lossless consumer must reject it.
`bvn_rational_to_str` renders an exact rational in any base bvnr can write —
`2..62` plus `64` and `85` (`bvn_rational_base_valid`) — setting `*exact = true`
and writing the full expansion when it terminates. It distinguishes two
failures:

| Return | Meaning |
|--------|---------|
| `BVN_RATIONAL_NONTERMINATING` (`-2`) | the expansion is infinite in this base. The rational itself is still exact, so use `num`/`den`. |
| `-1` | bad arguments, an unsupported base, a negative value in the sign-less bases 64/85, out of memory, or a `bufsize` too small. |

The buffer is **never truncated** — half an exact expansion is simply a different
number — so size it with `bvn_rational_str_bufsize`, which upper-bounds the
result from the operands' bit lengths.

Both handle any value width — a 1056-bit float, a 512-bit integer — with no
precision loss. The exactness they promise is bounded by the unit table's own
declared factors: every non-irrational unit carries an exact rational `to_si`
factor (see `src/gendata/units.bvnr`), and `gen_units.py` refuses to generate a
table in which a rounded decimal is passed off as exact.

---

### 3.5 `bvn_error_to_string`

```c
const char *bvn_error_to_string(error_code_t code);
```

Return a short, static, human-readable description of an error code. The returned pointer is valid for the lifetime of the program; do not free it.

```c
fprintf(stderr, "error: %s\n", bvn_error_to_string(bvnr_reader_get_error(r)));
/* e.g. "error: value_out_of_range" */
```

**Unit-related error codes** (for reference):

| Code | Value | String | Trigger |
|------|-------|--------|---------|
| `error_unit_illegal` | 32 | `"unit_illegal"` | Unparseable unit string (unknown base, bad prefix, empty component, more than `BVNR_MAX_UNIT_COMPONENTS` = 32 components) |
| `error_unit_too_long` | 22 | `"unit_too_long"` | An **inline** unit suffix exceeds its 255-byte lexer buffer; a unit in an annotation raises `error_type_too_long` instead |
| `error_unit_mismatch` | 38 | `"unit_mismatch"` | Inline unit suffix present, type-annotation unit also present, and the two differ; or a `want_unit` target dimensionally incompatible with the value's unit (§1.10) |
| `error_unit_inexact` | 47 | `"unit_inexact"` | A `want_unit` conversion could not be delivered exactly: irrational factor, or a non-terminating expansion in the output base without `want_unit_allow_nonterminating` (§1.10) |
| `error_unit_profile_unknown` | 49 | `"unit_profile_unknown"` | A unit written in the `name:` profile notation names a profile this build does not have (spec 1.2) |
| `error_unit_profile_unsupported` | 50 | `"unit_profile_unsupported"` | A valid profile expression over known atoms with no representation in the unit model — a special unit carrying a reference level, a scale factor outside the SI prefix decades, or more components than a unit may hold (spec 1.2) |
| `error_octet_stream_forbidden` | 51 | `"octet_stream_forbidden"` | The document contains an octet stream and the reader was opened with `text_only`. Reported at the stream's opening `0x00`, before its payload is read |
| `error_type_param_whitespace` | 52 | `"type_param_whitespace"` | Whitespace split a type-annotation parameter in two (spec §5.3). Legal beside a separator — the family `:`, a `,` between parameters, before the closing `>` — and an error inside a parameter, reported at the first byte after it. `<float:64,k g>` used to be accepted as `k~g` and `<uint:6 4>` as a 64-bit width |

### 3.6 Canonicalising observer (`bvnr_canon_observer_*`)

An adapter that lets a reader's event stream drive the serializer directly, without
constructing a `bvnr_writer_t`. Installed as a reader callback it re-emits every event
to a sink, producing a canonical — or pretty-printed — copy of the input. This is what
`bovnar events -d` prints its re-serialisation with. (`bovnar pretty-print` is the other
way to do the same job: a `bvnr_writer_t` fed from `on_verified`, which is what lets
`--canonical` set `unit_flags`.)

```c
typedef struct bvnr_canon_observer_s bvnr_canon_observer_t;

bvnr_canon_observer_t *bvnr_canon_observer_create(const bvnr_sink_t *sink, bool pretty);
void bvnr_canon_observer_set_version(bvnr_canon_observer_t *obs,
                                     uint16_t major, uint16_t minor);
bool bvnr_canon_observer_on_event(void *obs, bvnr_event_t ev, bvnr_data_t *data);
bool bvnr_canon_observer_finish(bvnr_canon_observer_t *obs);
void bvnr_canon_observer_destroy(bvnr_canon_observer_t *obs);
```

`pretty` selects the indented form; `false` emits the compact canonical form.
`bvnr_canon_observer_create` returns `NULL` if the sink is missing or has no push
function.

`bvnr_canon_observer_on_event` has the signature of a reader callback — `void *` first
— so the observer handle can be passed as `userdata` and the function used as
`on_verified` directly. Call `bvnr_canon_observer_finish` after `bvnr_read` returns to
flush the trailing state, then `bvnr_canon_observer_destroy`.

**It does not re-validate.** The events already came from a validating reader, so the
observer reproduces faithfully what it is fed rather than checking it a second time.
For the same reason its array-nesting cap is set to the maximum: the reader has already
enforced its own limit, and a second, stricter cap here would reject a document the
reader accepted.

**Version directives.** A canonical copy of a spec-1.1 or 1.2 document needs the directive its
constructs require in order to re-read. `bvnr_canon_observer_set_version` records one to
prepend; it is emitted lazily, just before the first event. Call it before any event is
fed — typically from the reader's callback once `bvnr_reader_get_declared_version`
resolves (§1.8). A version of `0.0`, or a call made after output has begun, is ignored.

```c
bvnr_sink_t sink;
bvnr_sink_to_fd(&sink, STDOUT_FILENO);

bvnr_canon_observer_t *canon = bvnr_canon_observer_create(&sink, /*pretty=*/true);

bvnr_read_flags_t flags = {0};
flags.userdata    = canon;
flags.on_verified = bvnr_canon_observer_on_event;

bvnr_open_read_source(r, &src, NULL, &flags);
bvnr_read(r);
bvnr_canon_observer_finish(canon);
bvnr_canon_observer_destroy(canon);
```

### 3.7 Unit inspection helpers

Three more entry points from `bovnar_si_units.h`. The Python bindings expose all of
them — see [Extended unit functions](09_bovnar_python_bindings.md#52-extended-unit-functions) —
and they are listed here so the C side is not the poor relation.

```c
bool    bvn_unit_dimension_vector(value_unit_t u, int32_t dims[bvn_si_dim_count]);
bool    bvn_prefix_unit_valid(value_unit_prefix_t prefix, value_base_unit_t base);
int32_t bvn_exponent_to_int(unit_exponent_t e);
unit_exponent_t bvn_int_to_exponent(int32_t n);
```

**`bvn_unit_dimension_vector`** fills the seven SI exponents, indexed by
`bvn_si_dim_idx_t` — `bvn_si_dim_meter`, `_kilogram`, `_second`, `_ampere`, `_kelvin`,
`_mol`, `_candela`, with `bvn_si_dim_count` (7) as the array length. Two units with
the same vector are the same physical quantity however they were spelled:

```text
m/s          ok=1  [ 1, 0, -1, 0, 0, 0, 0]
k~g·m/s²     ok=1  [ 1, 1, -2, 0, 0, 0, 0]
N            ok=1  [ 1, 1, -2, 0, 0, 0, 0]      same vector as the compound above
%            ok=1  [ 0, 0,  0, 0, 0, 0, 0]      dimensionless, but it HAS a vector
$USD         ok=0                               currencies carry no dimension
```

The `false` return for a currency is the same fact §4.1 of the unit policy reference
records: a currency has no dimension vector, so a screen built on dimensions alone
declines conversions the engine can actually perform.

**`bvn_prefix_unit_valid`** answers whether a prefix may sit on a base unit — the IEC
prefixes belong to `bit` and `byte` only:

```c
value_unit_prefix_t p = { .system = prefix_iec, .id.iec = iec_gibi };
bvn_prefix_unit_valid(p, bu_byte);    /* true  */
bvn_prefix_unit_valid(p, bu_meter);   /* false */
```

**`bvn_exponent_to_int` / `bvn_int_to_exponent`** convert between the `unit_exponent_t`
enum and a plain integer: `bvn_exponent_to_int(exp_neg_square)` is `-2`, and
`bvn_int_to_exponent(-2)` is `exp_neg_square`.

---

## 4. DOM API (`bovnar_dom.h`)

The SAX reader above streams events. When you instead want the whole document in
memory for random-access queries — without writing a callback — use the DOM API
in `include/bovnar_dom.h`. It parses a document into a tree of `bvn_dom_node_t`
owned by a `bvn_dom_doc_t`, which you navigate and read with typed accessors.

### 4.1 Parsing and lifetime

```c
bvn_dom_doc_t *bvn_dom_doc_create(void);
void           bvn_dom_doc_destroy(bvn_dom_doc_t *doc);   /* frees the whole tree; NULL-safe */
bvn_dom_doc_t *bvn_dom_parse(const void *data, uint32_t len);
bvn_dom_doc_t *bvn_dom_parse_fd(int fd);
bvn_dom_doc_t *bvn_dom_parse_fd_ex(int fd, uint64_t max_bytes);
error_code_t   bvn_dom_doc_get_parse_error(const bvn_dom_doc_t *doc);
```

`bvn_dom_parse` returns NULL **only** on allocation failure — a *malformed*
document still returns a non-NULL doc, so check `bvn_dom_doc_get_parse_error()`
(`error_none` means it parsed cleanly), not the pointer, to detect a parse error.
The `bvn_dom_parse_fd*` variants instead return NULL on **any** failure
(allocation, I/O, or exceeding the size cap). `bvn_dom_parse_fd_ex` caps the
accumulated input at `max_bytes`, but only *downward*: `0` or any value above the
built-in hard cap of 256 MiB is clamped to that cap — there is no unlimited mode.
(The cap is a build-time constant of the library, `BVN_DOM_FD_MAX_BYTES` in
`src/dom/bovnar_dom_builder.c`; it is not in the public headers.) Free every
returned doc with `bvn_dom_doc_destroy`.

**The DOM reads at the format's nesting limit, not the reader's default.** A
`bvnr_reader_t` opened with zeroed flags stops at 64 levels of struct and 64 of
array nesting (§1.4); every `bvn_dom_parse*` function sets both to the hard
maximum of **255**. A document nested 65 to 255 deep therefore parses through
the DOM and is `error_struct_nesting_too_high` / `error_array_nesting_too_high`
from a default streaming reader — set `max_struct_nesting` and
`max_array_nesting` to 255 on the reader to make the two agree, which is what
the `bovnar` CLI does in every subcommand. The DOM's other limits are the
reader's defaults.

### 4.2 Navigation

```c
bvn_dom_node_t *bvn_dom_lookup(const bvn_dom_doc_t *doc, const char *path); /* dot path, e.g. "server.tls.cert" */
bvn_dom_node_t *bvn_dom_struct_get(const bvn_dom_node_t *node, const char *key);
bvn_dom_node_t *bvn_dom_array_at(const bvn_dom_node_t *node, uint32_t index);
uint32_t        bvn_dom_struct_count(const bvn_dom_node_t *node);
uint32_t        bvn_dom_array_count(const bvn_dom_node_t *node);
uint32_t        bvn_dom_array_dims(const bvn_dom_node_t *node);   /* number of `/`-separated dimensions */
const bvn_dom_entry_t *bvn_dom_struct_entries(const bvn_dom_node_t *node);
const bvn_dom_entry_t *bvn_dom_doc_entries(const bvn_dom_doc_t *doc);
uint32_t               bvn_dom_doc_count(const bvn_dom_doc_t *doc);
```

Each `bvn_dom_entry_t` is `{ char *key; bvn_dom_node_t *value; }`. A missing key
or out-of-range index returns NULL.

### 4.3 Type inspection

```c
typedef enum bvn_dom_type_e {
    BVN_DOM_NULL, BVN_DOM_INT, BVN_DOM_FLOAT, BVN_DOM_STRING, BVN_DOM_SYMBOL,
    BVN_DOM_REFERENCE, BVN_DOM_STRUCT, BVN_DOM_ARRAY, BVN_DOM_OCTET_STREAM, BVN_DOM_BOOL
} bvn_dom_type_t;

bvn_dom_type_t    bvn_dom_node_type(const bvn_dom_node_t *node);
bool              bvn_dom_is_null(const bvn_dom_node_t *node);
value_type_spec_t bvn_dom_get_value_type(const bvn_dom_node_t *node);
value_unit_t      bvn_dom_get_unit(const bvn_dom_node_t *node);
int32_t           bvn_dom_get_unit_string(const bvn_dom_node_t *node, char *buf, size_t bufsize);
double            bvn_dom_get_value_in_base_units(const bvn_dom_node_t *node);
```

`bvn_dom_get_value_in_base_units` scales a numeric node into coherent SI —
`1.5 k~m` → `1500.0`, and for an affine scale it applies the offset too, so
`25 °C` → `298.15`. Two caveats it cannot signal:

- **It returns `0.0` for every failure**, which a caller cannot tell from a
  genuine zero. A non-numeric node, a currency (no SI mapping), a structurally
  invalid unit, and an affine unit inside a compound (`°C/h`) all give `0.0`, as
  does `0.0 m`. When the difference matters, read `bvn_dom_get_unit` and call
  `bvn_unit_to_si_factor` yourself — it has an `ok` out-param.
- **It is `double`.** A value wider than a double is rounded here. The lossless
  route is the reader's `want_unit` hook (§1.10) or `bvn_unit_convert_rational`.

### 4.4 Typed value accessors

Each accessor returns `false` (leaving the out-param **unchanged** — no clamping
or truncation) when the node is NULL, not of the requested kind, or does not fit
the target type. "Kind" is strict with one deliberate exception:
`bvn_dom_get_float` also succeeds on an **integer** node and returns its value
as a `double` (a wide integer is rounded) — a caller asking for "the number"
does not have to know whether the document wrote `5` or `5.0`. The reverse does
not hold: the integer accessors fail on a float node. Pointer results are **borrowed** — valid only until the owning
document is destroyed; do not free them.

```c
bool bvn_dom_get_bool  (const bvn_dom_node_t *node, bool   *out);
bool bvn_dom_get_float (const bvn_dom_node_t *node, double *out);
bool bvn_dom_get_i64/u64/i32/u32/i16/u16/i8/u8(const bvn_dom_node_t *node, /* int type */ *out);
bool bvn_dom_get_string   (const bvn_dom_node_t *node, const char    **out, uint32_t *len); /* NUL-terminated; *len excludes NUL */
bool bvn_dom_get_symbol   (const bvn_dom_node_t *node, const char    **out, uint32_t *len);
bool bvn_dom_get_reference(const bvn_dom_node_t *node, const char    **out, uint32_t *len);
bool bvn_dom_get_octets   (const bvn_dom_node_t *node, const uint8_t **out, uint32_t *len); /* raw bytes, NOT NUL-terminated */
```

Wide integers (> 64 bits) are not stored inline; read them as a borrowed bigint
or render them to a string:

```c
const bvn_int_t *bvn_dom_get_bigint(const bvn_dom_node_t *node); /* NULL unless the int is wider than 64 bits */
char            *bvn_dom_int_to_str(const bvn_dom_node_t *node, uint32_t base); /* caller owns; free with bvn_dom_free_string */
void             bvn_dom_free_string(char *s);
```

For a `datetime` node written as a literal with a fractional second (spec 1.1),
the verbatim sub-second digits are available separately; the node's integer value
is still the whole-second epoch count read via `bvn_dom_get_i64`:

```c
const char *bvn_dom_get_datetime_fraction(const bvn_dom_node_t *node, uint32_t *len_out);
```

A value node also keeps its unit's **spelling** (library 2.0) where that says
something `bvn_dom_get_unit` cannot — `ucum:mL{total}` is the unit `m~L`, and
the annotation survives only here:

```c
const char *bvn_dom_get_unit_source(const bvn_dom_node_t *node, uint32_t *len_out);
```

The string is NUL-terminated and owned by the node. It is `NULL` for a node with
no unit, for one the document already wrote in canonical form (so
`bvn_unit_to_string` of the node's unit *is* the spelling, and a large array in
canonical units stores nothing extra), and for one whose value a unit policy
converted — the document's spelling describes a unit that node no longer has.

### 4.5 Building a tree

The DOM is also writable, e.g. to construct a document programmatically and hand
it to a serialiser. `bvn_dom_node_alloc` / the `bvn_dom_node_from_*` constructors
make nodes; the `*_add`/`*_append` functions attach them.

```c
bvn_dom_node_t *bvn_dom_node_alloc(bvn_dom_type_t t);
void            bvn_dom_node_destroy(bvn_dom_node_t *n);
bvn_dom_node_t *bvn_dom_node_from_i64/u64/i32/u32/i16/u16/i8/u8(/* value */);
bvn_dom_node_t *bvn_dom_node_from_bigint(bvn_int_t *bigint, value_type_spec_t vt, value_unit_t vu);
bool bvn_dom_struct_add   (bvn_dom_node_t *s, const char *key, uint32_t klen, bvn_dom_node_t *val);
bool bvn_dom_doc_add      (bvn_dom_doc_t  *d, const char *key, uint32_t klen, bvn_dom_node_t *val);
bool bvn_dom_array_append (bvn_dom_node_t *a, bvn_dom_node_t *elem);
char *bvn_dom_strdup(const char *s, uint32_t len);
```

**Ownership.** `bvn_dom_struct_add` / `bvn_dom_doc_add` / `bvn_dom_array_append`
**always** take ownership of the value node: on success the container owns it, and
on **every** failure path the node is destroyed internally — so never destroy it
yourself after the call (doing so double-frees).

**What they refuse.** Each returns `false` — having destroyed the value — when
the container is NULL or of the wrong kind (`bvn_dom_array_append` on anything
but a `BVN_DOM_ARRAY`, `bvn_dom_struct_add` on anything but a `BVN_DOM_STRUCT`),
when the value is NULL, and, for the two keyed ones, when the scope **already
has that key**: a hand-built tree cannot hold the duplicate a parsed document is
refused for (`error_duplicate_struct_key`). The duplicate test is a linear scan
of the scope. `bvn_dom_node_from_bigint` is the
one exception with *asymmetric* ownership: on success it takes ownership of the
`bvn_int_t`; on failure (NULL return) it does not, and you still own it.

### 4.6 Minimal example

```c
bvn_dom_doc_t *doc = bvn_dom_parse(buf, (uint32_t)len);
if (!doc) { /* out of memory */ }
if (bvn_dom_doc_get_parse_error(doc) != error_none) {
    /* malformed input — inspect the code */
} else {
    bvn_dom_node_t *port = bvn_dom_lookup(doc, "server.port");
    uint16_t p;
    if (port && bvn_dom_get_u16(port, &p)) { /* use p */ }
}
bvn_dom_doc_destroy(doc);
```

### 4.7 Parsing under a unit policy

```c
bvn_dom_doc_t *bvn_dom_parse_policy(const void *data, uint32_t len,
                                    const bvnr_unit_policy_t *p);
bvn_dom_doc_t *bvn_dom_parse_fd_policy(int fd, uint64_t max_bytes,
                                       const bvnr_unit_policy_t *p);
```

The DOM takes the same policy the streaming reader does (1.12), so a
random-access consumer can assert what it expects to find and store the unit it
wants back. `NULL` behaves exactly like the plain forms.

A validation failure lands in `bvn_dom_doc_get_parse_error()` as
`error_unit_mismatch`, like any other parse error. A policy the library refuses —
a malformed unit, a rule path naming nothing — is `error_invalid_argument`, so a
mistake in the **policy** is never mistaken for a fault in the **document**.

A value the policy converted is stored **converted**: its digits, its unit and
its base are the conversion's, because a caller who asked for metres and got the
document's feet back would have no way to notice. An integer that converts to a
fraction — 5 g in kilograms is 0.005 — is stored as a float, since that is what
it now is.

```c
static const bvnr_unit_rule_t rules[] = {
    { ".inlet.temperature", "°C", 0, bvnr_rule_convert },
};
bvnr_unit_policy_t p = {0};
p.rules = rules; p.num_rules = 1;

bvn_dom_doc_t *doc = bvn_dom_parse_policy(buf, len, &p);
bvn_dom_node_t *n  = bvn_dom_lookup(doc, ".inlet.temperature");
double v; bvn_dom_get_float(n, &v);      /* 100.0, from a document in °F */
```

This is also what `bovnar query` uses, so the command line has the same reach:

```bash
bovnar query --field .inlet.temperature=°C .inlet.temperature sensors.bvnr
bovnar query --require-unit .inlet.flow sensors.bvnr
```

---

## 5. Complete Read Example

```c
#include <fcntl.h>
#include <inttypes.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include "bovnar.h"

typedef struct { char key[256]; } ctx_t;

static bool on_verified(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
    ctx_t *ctx = ud;

    if (ev == ev_assignment_start) {
        size_t n = d->length < 255 ? d->length : 255;
        memcpy(ctx->key, d->data, n);
        ctx->key[n] = '\0';
        return true;
    }

    if (ev != ev_data || d->length == 0) return true;

    char vbuf[256];
    size_t n = d->length < 255 ? d->length : 255;
    memcpy(vbuf, d->data, n);
    vbuf[n] = '\0';

    switch (d->value_type.family) {
    case vt_uint: {
        uint64_t v;
        bvn_parse_uint64(vbuf, d->value_type, &v);
        printf(".%-20s = %" PRIu64 "  (uint%u)\n",
               ctx->key, v, bvn_effective_width(d->value_type));
        break;
    }
    case vt_float: {
        double v;
        bvn_parse_double(vbuf, d->value_type, &v);
        printf(".%-20s = %g  (float%u)\n",
               ctx->key, v, bvn_effective_width(d->value_type));
        break;
    }
    case vt_utf8:
        printf(".%-20s = \"%s\"\n", ctx->key, vbuf);
        break;
    default:
        printf(".%-20s = %s\n", ctx->key, vbuf);
        break;
    }
    return true;
}

int main(int argc, char **argv)
{
    if (argc < 2) { fprintf(stderr, "usage: %s <file>\n", argv[0]); return 1; }

    int fd = open(argv[1], O_RDONLY);
    if (fd < 0) { perror(argv[1]); return 1; }

    bvnr_reader_t *r = bvnr_reader_create();
    ctx_t ctx = {0};

    bvnr_source_t src;
    bvnr_source_from_fd(&src, fd);

    bvnr_read_flags_t opts = {
        .on_verified   = on_verified,
        .userdata      = &ctx,
        .max_file_size = 16777216,
    };

    if (!bvnr_open_read_source(r, &src, NULL, &opts)) {
        fputs("failed to open reader\n", stderr);
        bvnr_reader_destroy(r);
        close(fd);
        return 1;
    }

    int ret = 0;
    if (!bvnr_read(r)) {
        fprintf(stderr, "%s at line %" PRIu64 " col %" PRIu64 "\n",
                bvn_error_to_string(bvnr_reader_get_error(r)),
                bvnr_reader_get_error_line(r),
                bvnr_reader_get_error_column(r));
        ret = 1;
    }

    bvnr_reader_destroy(r);
    close(fd);
    return ret;
}
```

---

## 6. Inline Unit Suffix — Reading

A scalar value may carry an **inline unit suffix** directly after the literal, before its terminating `;`:

```bovnar
.speed = 9.81 m/s;            # no annotation; inline unit
.mass  = <float:64> 70.5 k~g; # annotation without unit; inline unit adopted
.dist  = <float:64,m> 1.5 m;  # annotation and inline agree — valid
```

From the application's perspective, inline units and annotation units are transparent: both end up in `d->value_unit` of the `ev_data` event. No special handling is needed.

The only behavioral difference occurs when **both** are present and **disagree**: the validator raises `error_unit_mismatch` (38) and parsing fails:

<!-- bovnar-example: rejected -->
```bovnar
.bad = <float:64,m> 1.0 s;    # annotation says m, inline says s → error_unit_mismatch
```

Inline unit suffixes are **illegal inside array elements**. The lexer rejects them with `error_unexpected_input_byte`.

### 6.1 Reading inline unit values

```c
/* The callback below works identically whether the unit came from a
 * type annotation or from an inline suffix — no change needed.        */
static bool on_verified(void *ud, bvnr_event_t ev, bvnr_data_t *d)
{
    if (ev != ev_data) return true;

    char unit_str[128] = "no_unit";
    /* dimensionless: num_components==0 (BVN_UNIT_NONE) or
       num_components==1 with base==bu_none (BVN_UNIT_NO_PREFIX(bu_none)) */
    bool is_dim = (d->value_unit.num_components == 0) ||
                  (d->value_unit.num_components == 1 &&
                   d->value_unit.components[0].base == bu_none);
    if (!is_dim)
        bvn_unit_to_string(d->value_unit, unit_str, sizeof(unit_str));

    char val[256];
    size_t n = d->length < 255 ? d->length : 255;
    memcpy(val, d->data, n);
    val[n] = '\0';

    printf("value=%s  unit=%s\n", val, unit_str);
    return true;
}
```

---

## 7. Complete Write Example

```c
#include <inttypes.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include "bovnar.h"

/* Emit: .velocity = <float:64,m/s²> 9.81e+0; */
static bool write_velocity(bvnr_writer_t *w)
{
    value_type_spec_t vt = BVN_TYPE_FLOAT(64);
    value_unit_t      vu = BVN_UNIT_COMPOUND2(
                               bu_meter,  si_none, exp_linear,
                               bu_second, si_none, exp_neg_square);

    char valbuf[32];
    bvn_format_double(valbuf, sizeof(valbuf), 9.81, vt);

    bvnr_data_t d;

    d = (bvnr_data_t){ .data = "velocity", .length = 8 };
    if (!bvnr_write_event(w, ev_assignment_start, &d)) return false;

    if (!bvnr_write_type_annotation(w, vt, vu)) return false;

    d = (bvnr_data_t){
        .type       = token_is_number,
        .value_type = vt,
        .value_unit = vu,
        .data       = valbuf,
        .length     = (uint32_t)strlen(valbuf),
    };
    return bvnr_write_event(w, ev_data, &d);
}

int main(void)
{
    bvnr_writer_t *w = bvnr_writer_create();

    bvnr_sink_t sink;
    bvnr_sink_to_fd(&sink, STDOUT_FILENO);

    bvnr_write_flags_t opts = { 0 };
    if (!bvnr_open_write_sink(w, &sink, /*pretty=*/true, &opts)) {
        fputs("failed to open writer\n", stderr);
        bvnr_writer_destroy(w);
        return 1;
    }

    int ret = 0;
    if (!write_velocity(w) || !bvnr_write_finish(w)) {
        fprintf(stderr, "write error: %s\n",
                bvn_error_to_string(bvnr_writer_get_error(w)));
        ret = 1;
    }

    bvnr_writer_destroy(w);
    return ret;
}
/* Output: .velocity = <float:64,m/s²> 9.81e+0;
 * (bvn_format_double writes an exponent on every non-zero float -- see 2.11) */
```

---

## See also

- [Specification §16 — Reference API](03_bovnar_spec.md#16-reference-api) — the normative summary of these entry points
- [Unit & Currency Reference](05_bovnar_unit_system.md) — the unit model behind `bvn_parse_unit` and `want_unit`
- [Streaming, Framing & Multiplexing](10_bovnar_streaming.md) — protocols layered on this event API
- [Python Bindings](09_bovnar_python_bindings.md) — the same API from Python
- [FAQ §12 — C API](02_bovnar_faq.md#12-c-api) — common questions about these functions

---

*End of Bovnar — Read & Write API (Bovnar spec 1.2).*
