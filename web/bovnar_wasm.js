// Bovnar WASM — ergonomic wrapper around the emscripten module.
//
// SOURCE OF TRUTH: wasm/index.mjs. build_wasm.sh copies this file to
// web/bovnar_wasm.js (rewriting the core import to a cache-busted URL) and to
// dist/wasm/. Editing a copy appears to work and is silently reverted by the
// next build — that has already happened once, which is why
// bvnr_wasm_wrapper_sync now fails the suite when the two diverge.
//
// The C reference parser, compiled to WebAssembly. This IS the reference
// implementation: it synthesises default type annotations and performs full
// type/value/unit validation, so error_unit_mismatch and friends are reported
// exactly as the native `bovnar` CLI reports them.
//
// Usage:
//   import { loadBovnar } from 'bovnar-wasm';
//   const bvnr = await loadBovnar();
//   bvnr.validate('.v = <float:64,m/s> 9.81;');   // -> { ok, error, ... }
//   bvnr.toJSON('.v = 9.81 m/s;');                // -> { ok, value, ... }
//   bvnr.events('.v = 9.81 m/s;');                // -> { ok, events:[...] }
//   bvnr.eventsConvert('.v = 9.81 k~m/s;', 'm/s'); // -> events with `converted`
//   bvnr.version();                                // -> "2.0.0"
//
// `input` may be a string (UTF-8 encoded for you) or a Uint8Array (passed
// through verbatim — required if the document embeds octet streams with NULs).

import createBovnar from './bovnar_wasm_core.js?v=72dca6e2bfe2';

const enc = new TextEncoder();

function toBytes(input) {
  if (input instanceof Uint8Array) return input;
  if (typeof input === 'string') return enc.encode(input);
  throw new TypeError('bovnar: input must be a string or Uint8Array');
}

export async function loadBovnar(opts) {
  const Module = await createBovnar(opts);

  // Call a `char* fn(const char* buf, int len)` export with a byte buffer,
  // then read back + free the returned NUL-terminated JSON string.
  function callJson(fnName, input) {
    const bytes = toBytes(input);
    const len = bytes.length;
    const inPtr = Module._malloc(len || 1);
    // emscripten's malloc RETURNS 0 on failure, it does not throw. Without this
    // check a large paste that exhausts the heap would memcpy the input over
    // address 0 -- the null page and the module's static data -- silently
    // corrupting the parser for the rest of the session instead of failing.
    if (!inPtr) throw new Error('bovnar: out of memory');
    if (len) Module.HEAPU8.set(bytes, inPtr);
    let outPtr = 0;
    try {
      outPtr = Module.ccall(fnName, 'number', ['number', 'number'], [inPtr, len]);
      // A null result is the C side failing to allocate even its own error
      // object. JSON.parse('') turned that into "SyntaxError: Unexpected end of
      // JSON input", which names neither the cause nor this library.
      if (!outPtr) throw new Error('bovnar: out of memory');
      return JSON.parse(Module.UTF8ToString(outPtr));
    } finally {
      Module._free(inPtr);
      if (outPtr) Module.ccall('bvnr_wasm_free', 'void', ['number'], [outPtr]);
    }
  }

  // A base crosses as a C int. `base | 0` wrapped 4294967306 to 10 and turned
  // 2.5 into 2, each reported as a successful conversion; anything that is not
  // a non-negative integer the C side can hold goes over as -1, which it
  // refuses as error_invalid_argument.
  function wasmBase(base) {
    const b = Number(base);
    return Number.isInteger(b) && b >= 0 && b <= 0x7fffffff ? b : -1;
  }

  // Same, for the conversion variant: (buf, len, unit, base, allow) where
  // `unit` is a NUL-terminated C string we allocate alongside the document.
  function callJsonConvert(input, unit, base, allowNonterminating) {
    const bytes = toBytes(input);
    const len = bytes.length;
    // The unit crosses as a NUL-terminated C string, so a NUL inside it ended it
    // early: 'm\0anything' converted to metres as if the rest had not been
    // written. No unit contains a NUL, so such a string is an unparseable unit
    // and must come back as one (error_unit_illegal, decided by the C side like
    // every other bad unit). Swapping the NUL for another control byte keeps
    // the string whole and just as unparseable.
    const unitBytes = enc.encode(String(unit || '').replace(/\0/g, '\x01'));
    // Both allocations happen before the try, so a failure in the second used to
    // leak the first. Allocate, check for the null emscripten returns on OOM,
    // and let the finally below release whichever succeeded.
    let inPtr = 0, uPtr = 0, outPtr = 0;
    try {
      inPtr = Module._malloc(len || 1);
      if (!inPtr) throw new Error('bovnar: out of memory');
      uPtr = Module._malloc(unitBytes.length + 1);
      if (!uPtr) throw new Error('bovnar: out of memory');
      // Re-read HEAPU8 after every _malloc: growth swaps the ArrayBuffer and
      // detaches any view captured earlier.
      if (len) Module.HEAPU8.set(bytes, inPtr);
      Module.HEAPU8.set(unitBytes, uPtr);
      Module.HEAPU8[uPtr + unitBytes.length] = 0;
      outPtr = Module.ccall(
        'bvnr_wasm_events_convert', 'number',
        ['number', 'number', 'number', 'number', 'number'],
        [inPtr, len, uPtr, wasmBase(base), allowNonterminating ? 1 : 0]);
      if (!outPtr) throw new Error('bovnar: out of memory');
      return JSON.parse(Module.UTF8ToString(outPtr));
    } finally {
      if (inPtr) Module._free(inPtr);
      if (uPtr) Module._free(uPtr);
      if (outPtr) Module.ccall('bvnr_wasm_free', 'void', ['number'], [outPtr]);
    }
  }

  return {
    /** Validate a document exactly as `bovnar validate` does: the streaming
     *  pass (stops at the first error), then -- if that was clean -- the
     *  document tier, which holds the rules no single value can break: array
     *  homogeneity, one shape per array of records, no repeated key. Returns
     *  { ok, error, error_name, tier, line, column, byte, offset,
     *  recovery_count, declared_version }.
     *
     *  `tier` is null when ok, 'stream' for an error at a position, and
     *  'document' for a whole-document rule -- which HAS no position: line,
     *  column, byte and offset are 0 then, and must not be shown as 0:0. */
    validate: (input) => callJson('bvnr_wasm_validate', input),

    /** Validate in resync mode, collecting EVERY error. Returns
     *  { ok, errors: [{ error, error_name, line, column, byte, offset, tier }],
     *  declared_version }.
     *
     *  When the streaming pass finds nothing, the document tier is consulted
     *  as in validate(), and its error -- there is at most one -- is the single
     *  entry, with tier 'document' and a zero position. So `ok` here and in
     *  validate() always agree. */
    errors: (input) => callJson('bvnr_wasm_errors', input),

    /** Parse and project the validated DOM to plain JSON. Returns
     *  { ok, error, error_name, value }. Unit/type metadata is dropped from
     *  `value`; use events() for that.
     *
     *  One valid document is refused, as `bovnar convert` refuses it: a
     *  datetime written with sub-second digits, whose JSON form would be a
     *  different instant. That comes back as value_out_of_range with a
     *  `detail` string naming the key. */
    toJSON: (input) => callJson('bvnr_wasm_to_json', input),

    /** The reference (verified) event stream. Returns { ok, error, error_name,
     *  events: [{ seq, ev, tok, text?, family?, width?, base?, unit?, ... }] }.
     *
     *  This is the STREAMING tier, and `ok` speaks for that tier only: it is
     *  what a reader's on_verified callback sees, and a reader does not know
     *  that two keys collide or that an array's elements disagree. A document
     *  can be ok here and fail validate() with tier 'document'. Ask validate()
     *  (or errors()) whether a document is valid; ask this what it contains.
     *
     *  Runs in resync mode, so a broken assignment is skipped rather than
     *  aborting the read and its events are simply absent. `ok` accounts for
     *  that: it is false if ANY error was reported, including a recovered one,
     *  and `error`/`error_name` name the first. */
    events: (input) => callJson('bvnr_wasm_events', input),

    /** Events AND errors from a SINGLE reader pass:
     *  { ok, error, error_name, events:[...], errors:[...], declared_version }.
     *  Prefer this over calling events() and errors() for the same document —
     *  that walks it twice and pays two JSON round-trips.
     *
     *  Like events(), this is the STREAMING tier: `errors` holds what the
     *  reader raised, each with tier 'stream', and never a document-tier
     *  error -- so it can be ok for a document validate() refuses. (A caller
     *  that needs both pays for the second pass by calling validate() or
     *  doc() when this one comes back clean.)
     *
     *  `ok` is false if ANY error was reported, including ones the reader
     *  recovered from, and `error`/`error_name` name the first — matching
     *  errors()[0] and events(). All three count reported errors rather than
     *  reading the reader's final code, which continue_on_error clears on a
     *  clean EOF; deriving the status from that code alone reported
     *  {"ok":false,"error_name":"none"} here, and plain {"ok":true} in events().
     *
     *  Defined only when the compiled module actually exports it: this wrapper
     *  and the core are separate URLs with separate cache entries, so a current
     *  wrapper can be paired with a stale core. Callers feature-test `parse`,
     *  and an unconditional definition would make that test lie and turn a
     *  recoverable fallback into a throw. */
    ...(typeof Module._bvnr_wasm_parse === 'function'
        ? { parse: (input) => callJson('bvnr_wasm_parse', input) }
        : {}),

    /** The event stream with the reader's LOSSLESS unit/base conversion armed,
     *  so numeric events also carry `converted` (the exact value, or null when
     *  it has no finite expansion in `base`), `converted_base` and
     *  `converted_unit`.
     *
     *  `unit` is a unit string ("m", "k~m/s"); pass '' to keep each value's own
     *  unit, which together with a `base` is a pure base conversion. `base` is
     *  the output base (0 keeps the value's own; otherwise 2..62, 64 or 85).
     *  With `allowNonterminating` a result that is exact as a rational but does
     *  not terminate in `base` arrives as `"converted": null` instead of being
     *  skipped. An unparseable `unit` comes back as error_unit_illegal.
     *
     *  Note: this surface runs in resync mode, so a value the conversion
     *  rejects is skipped along with its assignment rather than aborting the
     *  call — it will not appear in `events` — but `ok` is then false and
     *  `error_name` names the rejection (unit_mismatch, unit_inexact). A
     *  `base` that is not 0 or a non-negative integer is invalid_argument. */
    eventsConvert: (input, unit = '', base = 0, allowNonterminating = false) =>
      callJsonConvert(input, unit, base, allowNonterminating),

    /** Full document tree in document order, each value carrying its resolved
     *  type fields (familyName/width/base/unit/epoch). Returns
     *  { ok, error, error_name, tree: [{ key, value }] }. Built from the DOM,
     *  so `ok` covers both tiers. Used by the web playground's parser
     *  shim to reconstruct the tree + event-stream shapes. */
    doc: (input) => callJson('bvnr_wasm_doc', input),

    /** Library version string, e.g. "2.0.0". */
    version: () => Module.ccall('bvnr_wasm_version', 'string', [], []),

    /** The raw emscripten Module, for advanced use. */
    Module,
  };
}

export default loadBovnar;
