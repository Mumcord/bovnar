# Security Policy

The Bovnar parser is intended to be pointed at untrusted bytes arriving over a
socket. Anything reachable from `bvnr_read()` on hostile input is handled as a
security report rather than an ordinary bug.

## Reporting a Vulnerability

**Do not open a public issue.** Email **bovnar@mail.de** with the details and a
reproducer, and allow time for a fix before disclosing.

Please include, where you have them:

1. The output of `bovnar version` — the library version and the supported spec
   version.
2. Platform, architecture and compiler, including whether the build was
   sanitized (`-DBVNR_SANITIZE=ON`).
3. A reproducer. Attach the input as a file rather than pasting it: a document
   containing an octet stream carries NUL and unpaired CR and does not survive
   copy-paste, and a corrupted one reproduces a different problem.
4. The observed effect — the sanitizer report, the crashing stack, or the
   incorrect value, with the reported line and column where the parser named
   one.

Expect an acknowledgement within a few days. A fix ships in the next release,
with the reporter credited in `CHANGELOG.md` unless they ask otherwise.

## Supported Versions

| Version | Supported |
|---|---|
| 2.0.x | Yes |
| 1.1.x | No — upgrade to 2.0.x, which reads 1.0 and 1.1 documents (see below for what it now refuses); the C ABI and the Python API changed, so this one is a rebuild and not a drop-in |
| 1.0.x | No |

The **format** and the reference implementation are versioned separately, both
semantically, and a fix lands on the newest release of the implementation only.
Within a major version of the format it is additive: a document valid under spec
1.0 stays valid, and decodes to the same values, under every 1.x spec, so a
newer reader does not need a document migrated — with the exceptions the
specification's §17 lists for 1.2: two corrections (whitespace inside a
type-annotation parameter, and a bare array mixing units of one dimension, are
now refused) and a list of reader defects that let malformed input
through or, in two array rules, judged a document by the order or nesting of
its elements. The implementation's own major
moves when a program built against it has to change, which is what 2.0.0 was;
`CHANGELOG.md` says which spec each release implements.

## In Scope

- Memory-safety defects — out-of-bounds access, use-after-free, uninitialized
  reads — reachable from the reader, the writer, the DOM, or the unit and
  datetime utilities.
- A crash, hang, or unbounded allocation on attacker-controlled input, including
  input that is malformed, truncated, or deliberately adversarial.
- A value that decodes to something other than what the document says, or a
  document that is accepted where the specification requires it to be rejected.
  A unit or a type that is silently wrong is a correctness defect with security
  consequences, not a cosmetic one.
- Anything the specification's Security Considerations describes as prevented
  and that turns out not to be — see `doc/03_bovnar_spec.md` and the
  corresponding section of `doc/ietf/draft-sonntag-bovnar-00.md`.

## Out of Scope

These are documented properties of the format, not defects. They are listed so a
report does not have to guess.

- **Resource exhaustion under a default configuration.** The reader's
  `max_file_size` defaults to unlimited so that an endless stream can be parsed
  at all. A consumer of untrusted input is expected to set it, along with
  `max_struct_nesting` and `max_array_nesting`. See the limits section of the
  specification.
- **Data loss in recovery mode.** `-c` / resync deliberately skips a broken
  assignment and continues. A consumer that does not check the recovery counters
  cannot tell a complete document from a partial one; that is the documented
  trade-off of the mode.
- **Corruption of an octet stream in transit.** Chunks are length-prefixed, so a
  channel that rewrites bytes — line-ending normalisation above all —
  desynchronises them unrecoverably. Use a binary-safe transport, or refuse the
  binary half outright with `--text-only`.
- **What a payload means.** An octet stream is opaque bytes. A consumer that
  interprets or executes one is responsible for validating it.
- **Confusable or non-normalized keys.** Keys are compared as bytes; the parser
  applies no Unicode normalisation and does not reject look-alike identifiers.
- **The document itself.** A Bovnar document is passive data: no scripting, no
  macros, no external entity references. A `&.path` reference names a value in
  the *same* document and triggers no network or filesystem access, so it is not
  an SSRF or XXE vector.

## Hardening

`BVNR_HARDEN` is on by default (stack protector on the CLI, `_FORTIFY_SOURCE=2`,
`-Wformat` security checks). CI runs the test suite a second time under
ASan and UBSan with `-fno-sanitize-recover=all` (all but the Python tests, which
load the library into an unsanitized interpreter), and the reader, writer, DOM and
utility layers each have a self-contained fuzz harness registered as a CTest
target, with optional libFuzzer and AFL++ builds behind `BVNR_FUZZ_EXTERNAL`.

GCC has no MemorySanitizer, so uninitialized *heap* reads are not covered by the
sanitizer runs above; a clang/MSan finding is therefore especially welcome.
