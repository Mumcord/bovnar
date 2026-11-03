# Bovnar 2.0.0 — units from other vocabularies, and a reader that converts

**The 2 is the library's, not the format's.** Bovnar 2.0.0 implements **spec
1.2**, which is additive over 1.1, and the version directive stays in the 1.x
line. (A few accepted inputs read differently — a `tai` leap-second literal
above all — and a handful of malformed ones are no longer let through; see
*Compatibility*.) The major moved because programs built against
1.1 have to change — the C ABI breaks and so does the Python API. From this
release on, the library version and the spec version are separate numbers.

**Spec 1.2 lets a document write its units in the vocabulary the data came
from.** A unit parameter of the form `namespace:code` is read as a code of UCUM,
UN/ECE Rec 20/21, QUDT, UDUNITS-2, OM 2 or the CF standard-name table and
translated, at parse time, into the same unit a native spelling produces.

```bovnar
#!bovnar 1.2
.systolic = <float:64,ucum:mm[Hg]>        120.0;   # UCUM          → mmHg
.weight   = <float_dec:64,unece:KGM>       12.5;   # UN/ECE Rec 20 → k~g
.speed    = <float:64,qudt:M-PER-SEC>       9.81;  # QUDT          → m/s
.flux     = <float:64,udunits:kg m-2 s-1>   0.5;   # UDUNITS / CF
.volume   = <float:64,ucum:mL{total}>       1.5;   # the annotation is kept
```

Like every construct added after 1.0 it is **opt-in per document**: it needs the
`#!bovnar 1.2` directive, and without one `ucum:mm[Hg]` is simply not a unit.
Native units are unaffected in every version — the registry is not gated, so a
new unit or the compact `km` reads in any document. What a 1.1 reader accepted and
this one treats differently is listed, in full, under *Compatibility* below.

For C consumers: rebuild against the new headers. `libbvnr.so` moves to
SOVERSION 2, so a binary built against 1.1 fails to load rather than misreading
a grown struct. For Python consumers: see *Breaking changes*; a requirement
pinned below 2 keeps the 1.1 API.

## Install

```bash
pip install --upgrade bovnar     # pure-ctypes bindings + bundled shared library
```

Prebuilt archives for Linux and Windows (MinGW64 and MSVC), the single-file
amalgamation and the source tarball are attached to the GitHub release, with a
`SHA256SUMS` file.

## What's new

- **Unit profiles (spec 1.2)** — seven namespaces: `ucum`, `unece`, `qudt`,
  `qudt-qk`, `udunits`, `om` and `cf`. A code either becomes a real unit or is
  an error; there is no state in which a value carries a unit the parser cannot
  reason about. `ucum:kg`, `unece:KGM`, `qudt:KiloGM`, `udunits:kg` and
  `om:kilogram` are one and the same unit, and a cross-vocabulary suite checks
  every spelling against every other. Units with no SI definition — UCUM's
  `[IU]`, a UN/ECE package count — are carried as opaque units: equal to
  themselves, convertible to nothing. `cf:` is read-only. Each vocabulary has
  its own build switch (`-DBVNR_WITH_<NAME>_PROFILE=OFF`).
- **A unit keeps its spelling** — `bvnr_data_t.unit_source` hands the reader's
  consumer the unit exactly as it was written, beside what it means, and the
  writer writes it back (`bvnr_writer_set_unit_source`, or the same field on
  `bvnr_write_event`). A spelling that disagrees with its unit is refused, not
  written.
- **Read-time unit policy and lossless conversion** —
  `bvnr_reader_set_unit_policy` and the `want_unit` hook convert values as they
  are read, in exact rational arithmetic: a conversion that cannot be exact is
  refused (`error_unit_inexact`), never rounded. A policy can also *assert* —
  require a unit on every number, require a dimension, or pin one field by path —
  and the writer enforces the validation half on what it emits. On the CLI:
  `--unit`, `--field`, `--si`, `--require-unit`, `--require-dimension`,
  `--require-field`.
- **A temperature difference is a unit** — `ΔK` (also spelled `Δ°C`) and five
  siblings, in
  their own quantity kind, so a difference no longer converts as a scale reading.
- **A larger native registry** — 264 physical units, up from 163, with their
  conversion factors checked against their own definitions.
- **Compact prefix form** — `km` beside `k~m`. The canonical form is unchanged
  (`k~m`); a document that is re-serialised keeps the spelling it was written in.
- **`text_only`** — a reader flag (and `--text-only`) that refuses a document
  containing an octet stream, for channels that are not byte-transparent.
- **A document tree a tenth the size** — a DOM node is 48 bytes, down from 656:
  the unit is shared between the values that carry it instead of copied onto
  each. A megabyte of small integers parses into about 32 MB, not 340. The DOM
  API is unchanged.
- **Python** — `Unit` objects that compare by meaning, `UnitPolicy`,
  `available_profiles()`, `unit_to_profile()`, and a typed
  `loads()` → `dumps()` round trip that is byte-identical for a profile-spelled
  document.
- **Tooling** — the mapped profile rows are checked against their publishers'
  own machine-readable definitions on a weekly schedule (UN/ECE only as far as
  QUDT's cross-reference reaches, since Rec 20 publishes none); the
  conformance suite is at **470 cases**; the web, vim, VS Code and Sublime
  highlighters know the profile notation.

## Breaking changes

- **ABI.** `bvnr_data_t` and `bvnr_read_flags_t` grew, `value_unit_t` holds up to
  32 components, and every `value_base_unit_t` id was renumbered into
  per-vocabulary blocks. No document changed — a unit is written by its
  spelling, never by its id. Code that names units by enumerator needs only a
  rebuild.
- **Python.** `Quantity.unit` is a `Unit` object (use `str(q.unit)`, or
  `q.unit.raw` for the struct), the value accessors are uniformly properties
  (`q.decimal`, not `q.decimal()`), and `Quantity()` validates its arguments.
  A DOM node's `unit_str` is `''` for a value without a unit, as
  `Quantity.unit_str` is (it was `'no_unit'`).
- **Removed names.** The currency id sub-ranges are gone, because currencies
  are catalogue rows now and their order carries no meaning: in C,
  `BVN_CURRENCY_FIAT_FIRST`/`_LAST`, `BVN_CURRENCY_CRYPTO_FIRST`/`_LAST`, the
  `BVN_CURRENCY_EXT_*` segment and `BVN_VALUE_BASE_UNIT_COUNT_CURRENCY`, and
  the enumerators `bu_zwg` and
  `bu_xcg`; in Python, `CURRENCY_FIAT_*`, `CURRENCY_CRYPTO_*`, `CURRENCY_EXT_*`
  and `BaseUnit._SENTINEL`. Use `BVN_CURRENCY_FIRST`/`_LAST`/`_COUNT`,
  `bvn_unit_is_currency`, `bvn_unit_is_fiat`/`bvn_unit_is_crypto` and
  `bvn_unit_valid`. No exported function was removed and no prototype changed.
- **Writer events are checked.** `bvnr_write_event` now refuses a unit
  parameter whose text does not name the annotation's unit, a value whose unit
  contradicts the annotation it is written under, a value the annotation's
  *type* does not admit (300 under `<uint:8>`, a string under a numeric
  family, an integer past 64 bits with no width declared), and an inline unit
  on an array element, a special number or a datetime (`error_unit_mismatch`,
  `error_unit_illegal`, `error_value_out_of_range`,
  `error_type_value_mismatch`). The helpers never produced such events; a
  caller that built them by hand was writing a document that said two things.
- **CLI exit status.** `0` success, `1` the document or the I/O failed, `2`
  the command line was wrong — uniformly, which moves several usage errors
  (no file, unknown command, bad `--from`) from 1 to 2. Extra arguments and
  unknown options, which some commands ignored, are refused; a failed write to
  stdout and an `events -c` run that recovered from errors are no longer
  exit 0.

## New error codes (appended, non-breaking)

`error_unit_inexact` (47), `error_octet_stream_truncated` (48),
`error_unit_profile_unknown` (49), `error_unit_profile_unsupported` (50),
`error_octet_stream_forbidden` (51), `error_type_param_whitespace` (52).
Existing codes are unchanged.

## Compatibility

Checked by running a build of 1.1.0 beside this one over the 1.1.0 examples,
documentation and conformance inputs, 170 000 unit spellings and 80 000 mutated
documents. No unit spelling that 1.1.0 accepted is refused or means something
else. What does differ:

- **Two corrections refuse documents that 1.1 accepted**, and apply whatever
  version a document declares:
  - **Whitespace inside a type-annotation parameter is an error**
    (`error_type_param_whitespace`). `<float:64,m in>` used to be read as `min`
    — the minute — and `<uint:6 4>` as a 64-bit width, with the space silently
    deleted; such a document carried a unit or a type its author did not write.
    Whitespace beside a separator — `<float : 64 , m>` — is as legal as it
    always was.
  - **A bare array is homogeneous in its unit**, not merely in its dimension:
    an array whose elements are annotated with different units of one dimension
    — metres beside feet — is refused by the document tier (`bovnar validate`,
    `bvn_dom_parse`, Python `dom_parse`; the streaming reader and `loads()`
    still deliver such an array element by element). A consumer that read such
    an array as one block, which the specification said it could, was computing
    wrong numbers.
- **A few accepted documents read as a different value.** A `tai` datetime
  literal spelling a known leap second — `2016-12-31T23:59:60Z` — now has its
  own value, one second lower than before; 1.1 collapsed it onto the second
  after it. `0e999999999999999999999` is 0, where the number helpers (and so
  `query` and `convert`) delivered +infinity. And a path into a nested
  `/`-matrix resolved to the wrong element: in `[[1]/[2]] / [[3]/[4]]`,
  `[1][0][0][0]` answered 1 and answers 3.
- **Reader fixes that refuse, or re-read, input 1.1 let through:** a byte-order
  mark after leading whitespace; a malformed `#!bovnar` directive with no
  trailing newline; an array left open at its `;` (`.a = [1;`), which used to
  absorb the next assignment — also inside a struct; a `,` after the value of
  a struct member inside an array (`.f = [{.x = 7, 1;}];`, which 1.1 read as
  `.x = 71`), and a `]` there that closed the array from inside its own
  element (`.a = [{.x = 1];};`); whitespace or a comment inside `float_fix` /
  `float_dec` (`<float _fix:32,q8>`); a `float_fix` literal too large for any
  float (`<float_fix:16,q8> 1e9862`, which 1.1 read as `inf`); a reference with bytes directly after an
  index (`&.a[0]b`); a comment cut off inside a UTF-8 sequence at end-of-input;
  quoted digits that spell a special number and exceed their width
  (`<uint:8,_36> "inf"` is 24171); a fractional datetime literal whose UTC
  year leaves 0000–9999; a width of zero on `utf8` or `bool` (`<utf8:0>`,
  `error_illegal_value_type` like any other parameter there); a type annotation
  before an empty struct (`<utf8> {}`, `<uint:8> [{}]` — before a struct with
  members it was always `error_type_value_mismatch`). And in `<float:64,s>[<float:64> 1, 3]` the `3` keeps
  the array's unit, where 1.1 delivered it with none.
- **Two array rules are now applied as written.** The width of a `/`-row
  counted a nested array twice: `[[1]]/[null, null]` was accepted and is
  `error_array_row_size_mismatch`, while `[[1], [2]]/[null, null]` was refused
  and is valid. And the document tier judged homogeneity against the first
  non-null element only, so a `null` there hid a mismatch behind it
  (`[[null], [1], ["x"]]`); the verdict no longer depends on element order.
- **Three fixes also *accept* documents 1.1 refused:** the row width above;
  rectangular nestings of `/`-matrices such as
  `[[[1,1]/[2,2], null], [null, null]]`, which were refused as ragged; and a
  `#!bovnar` directive with more than 21 spaces before its version.
- **Two limits now hold where they did not**, for a reader that sets them:
  `max_array_items` never limited an array of structs, and `max_file_size`
  depended on how the source was chunked.
- **Eight unit factors were corrected** — `elle` by 3.3×10⁻⁴,
  `klafter`/`rute`/`morgen` by about 10⁻⁶, and `inHg`, `Da`, `slug`, `ft_lb`
  below 2×10⁻⁹. A document's digits are unchanged; what they convert to is not.
- **1.2 constructs are opt-in** — they require the `#!bovnar 1.2` directive.

The specification's §17 records all of these.

## Open questions this release does not close

The profile tables restate other publishers' vocabularies. QUDT and OM (CC BY
4.0) and UDUNITS-2 (BSD 3-clause) are attributed as their licences require.
**UCUM's licence conditions its grant on not creating derivative tables, and the
CF standard-name table carries no licence statement**; permission and a
declaration have been requested for those two and neither has arrived. UN/ECE
Recommendation 20 exists to be implemented and is universally implemented, but
the UN's website terms do not say so; written confirmation has been asked for
there too. The UCUM and CF
tables ship enabled, and `-DBVNR_WITH_UCUM_PROFILE=OFF` /
`-DBVNR_WITH_CF_PROFILE=OFF` remove them from a build entirely. The record is in
[`THIRD_PARTY_NOTICES.md`](https://github.com/sothis/bovnar/blob/v2.0.0/THIRD_PARTY_NOTICES.md) and doc/11 §18.

## Documentation

Full docs — tutorial, specification, unit/currency reference, unit profiles, C
and Python API, formal EBNF grammar, FAQ, conformance protocol, and cheat sheet —
are available at **https://www.bovnar.io** (each also downloadable as **PDF**),
and in the [`doc/`](https://github.com/sothis/bovnar/tree/main/doc) directory.
See [`CHANGELOG.md`](https://github.com/sothis/bovnar/blob/v2.0.0/CHANGELOG.md) for the complete change list.

## License

MIT for the source, CC BY 4.0 for the documentation, CC0 1.0 for the examples.
The identifier strings in the unit-profile tables belong to their publishers and
are not covered by the MIT grant; see
[`THIRD_PARTY_NOTICES.md`](https://github.com/sothis/bovnar/blob/v2.0.0/THIRD_PARTY_NOTICES.md).
