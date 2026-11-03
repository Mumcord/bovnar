/*
 * bovnar_parser_wasm.js — the parser behind window.BovnarParser.
 *
 * Loads the real C reference parser (compiled to WASM) and makes it the sole
 * authority behind BovnarParser.parseFaithful / parseBovnar, producing the exact
 * output shapes the playground and the live demos consume with true
 * type/unit/value validation.
 *
 * Both surfaces are assembled from the reference *verified event stream*
 * (bvnr_wasm_events, run in resync mode) plus the resync error list
 * (bvnr_wasm_errors). Resync means a malformed assignment is skipped, not fatal,
 * so the tree and the demos recover and show every well-formed assignment past
 * an error.
 *
 * The WASM module loads asynchronously, so until it is ready parseFaithful/
 * parseBovnar return empty results (an empty tree / no events); a
 * 'bovnar-wasm-ready' event fires once the module is live so the playground and
 * demos can re-render against the real parser. The dispatcher functions have
 * stable identity, so consumers that captured `const { parseBovnar } =
 * BovnarParser` at load time pick up the parser transparently once it arrives.
 *
 * The C event stream carries no per-token position, so line numbers (demo
 * gutters) are recovered from the source text by a forward scan over assignments
 * in document order. Everything else — including whether an annotation was
 * written or synthesised — is read out of the stream itself.
 */
(function () {
  'use strict';

  var EV = {
    STREAM_START: 'ev_stream_start', STREAM_END: 'ev_stream_end',
    ASSIGNMENT_START: 'ev_assignment_start', ASSIGNMENT_END: 'ev_assignment_end',
    TYPE_ANN_START: 'ev_type_annotation_start',
    TYPE_ANN_FAMILY: 'ev_type_annotation_type_family',
    TYPE_ANN_PARAM: 'ev_type_annotation_type_family_parameter',
    TYPE_ANN_END: 'ev_type_annotation_end',
    STRUCT_START: 'ev_struct_start', STRUCT_END: 'ev_struct_end',
    ARRAY_ROW_START: 'ev_array_row_start', ARRAY_ROW_END: 'ev_array_row_end',
    ARRAY_DIM_START: 'ev_array_dim_start',
    DATA: 'ev_data',
    OCTET_STREAM_START: 'ev_octet_stream_start',
    OCTET_STREAM_END: 'ev_octet_stream_end',
    ERROR: 'ev_error',
  };

  var wasm = null;
  import('./bovnar_wasm.js?v=331191581f3a')
    .then(function (m) { return m.loadBovnar(); })
    .then(function (b) {
      wasm = b;
      try { window.dispatchEvent(new Event('bovnar-wasm-ready')); } catch (_) {}
    })
    .catch(function (e) {
      if (window.console) console.warn('bovnar: WASM parser failed to load', e);
    });

  /* Blank out string literals ("…") and '#' comments — replacing their bytes with
   * spaces while preserving offsets and newlines — so the key scanner below can
   * never match a '.key =' that merely appears inside a string or a comment.
   * (Octet-stream content is base64/hex, which can't contain '.', '#' or '"', so
   * it needs no masking.) */
  function maskStringsAndComments(text) {
    var out = '', n = text.length, mode = 0;   // 0 = code, 1 = string, 2 = comment
    for (var i = 0; i < n; i++) {
      var c = text[i];
      if (mode === 2) { if (c === '\n') { mode = 0; out += '\n'; } else out += ' '; continue; }
      if (mode === 1) {
        if (c === '\\') { out += ' '; if (i + 1 < n) { out += (text[i + 1] === '\n' ? '\n' : ' '); i++; } continue; }
        if (c === '"') { mode = 0; out += ' '; continue; }
        out += (c === '\n') ? '\n' : ' '; continue;
      }
      if (c === '#') { mode = 2; out += ' '; continue; }
      if (c === '"') { mode = 1; out += ' '; continue; }
      out += c;
    }
    return out;
  }

  /* ── source line / explicit-annotation scanner ───────────────────────── */
  function makeScan(text) {
    // Structural scanning runs on the masked copy (offsets map 1:1 to the source,
    // so line numbers and the value slice below are still correct).
    var masked = maskStringsAndComments(text);
    var pos = 0;
    // Incremental line counter. This used to rescan from offset 0 on every call,
    // and next() calls it once per assignment -- O(assignments x document), i.e.
    // quadratic: ~46M charCodeAt calls for a 1000-assignment, 46 KB document, on
    // the main thread, every debounce tick. The scan is strictly monotonic (pos
    // only advances and each match index is >= the previous pos), so count
    // newlines only over the span since the last query. lastIdx is reset if a
    // caller ever asks for an earlier offset, so a non-monotonic use stays correct.
    var lastIdx = 0, lastLine = 1, lastNL = -1;
    function lineAt(idx) {
      if (idx > masked.length) idx = masked.length;
      if (idx < lastIdx) { lastIdx = 0; lastLine = 1; lastNL = -1; }
      for (var i = lastIdx; i < idx; i++)
        if (masked.charCodeAt(i) === 10) { lastLine++; lastNL = i; }
      lastIdx = idx;
      return lastLine;
    }
    /* The DISPLAY column the C lexer reports: 1-based, one per code point, and a
       tab advances to the next multiple of 4 (the playground's jumpTo converts it
       back). Only ever asked for at a key, so it walks a single line. */
    function colAt(idx) {
      var col = 1;
      for (var i = lastNL + 1; i < idx; i++) {
        var c = masked.charCodeAt(i);
        if (c === 9) col = (((col - 1) >> 2) + 1) * 4 + 1;
        else if (c < 0xdc00 || c > 0xdfff) col++;   // skip a surrogate pair's tail
      }
      return col;
    }
    return function next(key) {
      var needle = '.' + key;
      var i = masked.indexOf(needle, pos);
      while (i >= 0) {
        var before = i === 0 ? '\n' : masked[i - 1];
        var j = i + needle.length;
        while (j < masked.length && (masked[j] === ' ' || masked[j] === '\t')) j++;
        var after = masked[j];
        if (/[\s{};]/.test(before) && (after === '=' || after === '{')) {
          pos = j;
          var line = lineAt(i);            /* lineAt first: colAt needs its newline */
          return { line: line, col: colAt(i) };
        }
        i = masked.indexOf(needle, i + 1);
      }
      return { line: lineAt(pos), col: 1 };
    };
  }

  /* ── event → value helpers ───────────────────────────────────────────── */
  var NUMERIC = { uint: 1, sint: 1, float: 1, float_fix: 1, float_dec: 1, datetime: 1 };
  function isSpecial(t) { return /^(inf|ninf|-inf|nan)$/i.test(t || ''); }
  function dataKind(e) {
    if (e.family === 'datetime') return 'datetime';
    switch (e.tok) {
      case 'string': case 'array_string': return 'string';
      case 'symbol': return 'symbol';
      case 'reference': return 'reference';
      case 'null': return 'null';
      case 'bool': return 'bool';
      default: return isSpecial(e.text) ? 'special' : 'number';
    }
  }
  function dataValue(e) {
    if (e.tok === 'bool') return e.text === 'true';
    if (e.tok === 'null') return null;
    return e.text != null ? e.text : '';   // numbers kept as text; demos parseFloat()
  }
  function scalarNode(e) {
    var num = NUMERIC[e.family] ? true : false;
    var width = num && ('width' in e) ? e.width : null;
    var base = num && ('base' in e) ? e.base : null;
    var vt = { familyName: e.family, family: 'vt_' + e.family, width: width, base: base, epoch: e.epoch };
    var type = e.tok === 'reference' ? 'reference' : (e.tok === 'null' ? 'null' : 'scalar');
    return { type: type, kind: dataKind(e), text: e.text != null ? e.text : '',
             value: dataValue(e), familyName: e.family, width: width, base: base,
             unit: e.unit || null, epoch: e.epoch || null,
             valueType: vt, valueUnit: e.unit || null };
  }
  /*
   * value_type_spec_t.base is three different things depending on the family,
   * and the C says which by the parameter event it emits beside it: type_base
   * for a real radix (uint/sint/float), type_q for float_fix's fixed-point
   * scaling, and nothing at all for float_dec. A datetime puts its epoch there
   * as a small dense index (bovnar.h: 0 = unix, 1 = tai) and reports the name
   * separately. Reconstructing the annotation from the resolved value therefore
   * has to branch the same way the reader does, or `<float_fix:16,q8,°C>` reads
   * as "base 8" and `<datetime:64,tai>` as "base 1" — a radix neither value has.
   *
   * A radix of 0 means "no explicit base", which resolves to decimal: the CLI
   * prints that as <uint:64,_10,no_unit>, and so does this.
   */
  /*
   * The radix the carrier is actually written in. uint/sint/float keep it in the
   * slot — 0 meaning "not written", which is decimal, and float admits only _10
   * or _16. The other three cannot take a base parameter at all: the reader
   * answers error_illegal_value_type for <float_fix:…,_16>, <float_dec:…,_16>
   * and <datetime:…,_16>, because their carriers are decimal by definition — a
   * Q-format fraction, an IEEE 754-2008 decimal, and a signed second count
   * "validated exactly like sint (signed, decimal)" (spec §6.2). Their slot is
   * busy holding something else (q, nothing, the epoch index), so the radix is
   * stated here rather than read out of it.
   */
  function annBase(v) {
    switch (v.familyName) {
      case 'float_fix': case 'float_dec': case 'datetime': return 10;
      default: return v.base || 10;
    }
  }
  function annFromValue(v, synthesized) {
    var params = [];
    if (v.width != null) params.push({ kind: 'width', text: String(v.width), value: v.width });
    var base = annBase(v);
    params.push({ kind: 'base', text: '_' + base, value: base });
    if (v.familyName === 'float_fix' && v.base != null)
      params.push({ kind: 'q', text: 'q' + v.base, value: v.base });
    /* Dimensionless is a statement, not an absence: bvn_unit_to_string answers
       "no_unit" for such a value and the CLI prints it — <uint:64,_10,no_unit>.
       The reader only emits the parameter for the synthesised default and for an
       annotation that spells it out (an explicit one that merely omits the unit
       is a structurally distinct internal state, spec §11), but these rows show
       the RESOLVED annotation — the same reason base 0 reads as 10 — so every
       unitless value says so. Except a datetime, whose parameter slot holds the
       epoch reported below; it has no unit to have none of. */
    if (v.familyName !== 'datetime')
      params.push({ kind: 'unit', text: v.unit || 'no_unit' });
    if (v.epoch)         params.push({ kind: 'epoch', text: v.epoch });
    return { synthesized: synthesized, familyName: v.familyName,
             family: 'vt_' + v.familyName, params: params };
  }

  /* ── parseFaithful: assemble the tree from the verified event stream ─────
   *
   * Arrays live on the SAME container stack as structs, because in bovnar an
   * array element is a value like any other — including a struct or another
   * array. A flat "one array being built" variable cannot express that: it made
   * a struct inside an array open a keyed assignment in the enclosing scope
   * (stealing the array's key), committed the array — empty — at that struct's
   * first field, and dropped every following element on the floor. `.a =
   * [{.b=3;},{.c=4;}];` rendered as two top-level structs, one of them keyed
   * `null`, around a phantom 1×0 array.
   *
   * The stack has to be lazy about closing an array because the C stream has no
   * "array end" event: a row ends with ev_array_row_end, and only the NEXT event
   * says whether the array continues with a '/'-dimension row (announced by
   * ev_array_dim_start), whether a sibling sub-array begins, or whether the
   * array is over. So settle() pops arrays whose last row has closed, and runs
   * before any event that proves the array cannot continue.
   *
   * Elements land in the innermost OPEN row; that single rule is what makes a
   * struct inside an array an element instead of an assignment. The resulting
   * shape matches bvnr_wasm_doc's DOM projection node for node.
   */
  function buildTree(events, scan) {
    var root = { type: 'stream', children: [] };
    var stack = [{ kind: 'struct', node: root }];
    var key = null, line = 1, col = 1;
    /* Whether the annotation now in effect was SYNTHESISED by the reader rather
       than written by the author. The stream says so itself: the reader spells
       the defaults it invented out in full — a synthesised annotation always
       carries a type_base parameter of 0 ("no explicit base"), which no written
       annotation can produce, since `_0` is not a radix anyone may write. This
       used to be guessed from the source text with a `=\s*<` regex, which could
       only ever see an annotation sitting directly after the `=` and therefore
       called every per-element annotation in an array (§7.5) synthesised. */
    var annSynth = false;
    /* An annotation has completed (ev_type_annotation_end) and no value has
       claimed it yet. The C emits one group per element whose type differs from
       the one in effect, so this is how an element learns it carries its own. */
    var annPending = false;
    /* assignment -> the synth flag of its ARRAY-LEVEL annotation, the one the
       document wrote before the '[' (`.a = <uint:8> [1,2];`). That one describes
       the array itself, so it belongs to the assignment — but it is rebuilt from
       a resolved element in the post-pass below, long after the flag has moved
       on. An annotation emitted INSIDE a row is a different thing entirely: it
       belongs to the element it precedes, and is attached there. */
    var arrayLevelSynth = new Map();
    /* The unit exactly as the current annotation spells it. bvn_unit_to_string
       canonicalises (`m·s⁻¹` and `deg` come back as `m/s` and `°`), and a reader
       echoing a document back — the demos print the decoded unit beside the
       value — has to be able to show the spelling that was written. Only ever
       attached to a value that HAS a unit: a datetime's epoch arrives in the
       same parameter slot, and `<datetime:64,tai>` does not make `tai` a unit. */
    var annUnit = null;
    var oct = null;       // the octet node currently collecting chunks
    /* struct_close events already spent by a resynced array_row_end (below) */
    var swallowStructClose = 0;
    function top() { return stack[stack.length - 1]; }
    /* the row currently accepting elements, or null between rows / in a struct */
    function openRow() { var f = top(); return f.kind === 'array' ? f.row : null; }
    function closeArray(f) {
      /* bvn_dom_array_dims() is the '/'-row count (src/dom/bovnar_dom_builder.c),
         which is exactly the number of rows opened here. */
      f.node.dims = f.node.rows.length;
      var e0 = f.node.rows[0] && f.node.rows[0].elements[0];
      if (f.assign && f.unitText && !f.assign.unitText && e0 && e0.unit)
        f.assign.unitText = f.unitText;
    }
    function settle() {
      while (top().kind === 'array' && !top().row) closeArray(stack.pop());
    }
    function haveArray() {
      for (var k = 0; k < stack.length; k++) if (stack[k].kind === 'array') return true;
      return false;
    }
    /* An element of the open row, or a keyed assignment in the enclosing scope.
       Returns the assignment when it made one (null for an array element). */
    function attach(value) {
      var row = openRow();
      if (row) { row.elements.push(value); return null; }
      var assign = { type: 'assignment', key: key, line: line, col: col,
                     ann: null, value: value };
      top().node.children.push(assign);
      key = null;
      return assign;
    }
    for (var i = 0; i < events.length; i++) {
      var e = events[i];
      switch (e.ev) {
        case 'assign_start': {
          settle();
          var s = scan(e.text); key = e.text; line = s.line; col = s.col;
          annUnit = null;
          break;
        }
        case 'type_start': annUnit = null; annSynth = false; break;
        case 'type_param':
          if (e.tok === 'unit' && e.text && e.text !== 'no_unit') annUnit = e.text;
          /* the reader's "no explicit base" — see annSynth above */
          else if (e.tok === 'type_base' && !e.base) annSynth = true;
          break;
        case 'type_end': annPending = true; break;
        case 'struct_open': {
          settle();
          var snode = { type: 'struct', children: [] };
          attach(snode);
          stack.push({ kind: 'struct', node: snode });
          key = null;
          break;
        }
        // Close anything still open inside the struct before popping it: the C
        // stream emits array_row_end → struct_close, so an array that is the
        // struct's last field is still on the stack here.
        case 'struct_close':
          /* Already closed by the row end that preceded it — spending it again
             would pop the ENCLOSING struct, and every field after it (`.z` in
             `.s = {.a = [{bad}]; .z = 5;};`) would leave the struct it is
             written in. */
          if (swallowStructClose) { swallowStructClose--; break; }
          while (stack.length > 1 && top().kind === 'array') closeArray(stack.pop());
          if (stack.length > 1) stack.pop();
          break;
        case 'array_row_start': {
          /* Pop finished sub-arrays — but never one waiting for its next
             '/'-dimension row, which this event is about to open. */
          while (top().kind === 'array' && !top().row && !top().pendingDim)
            closeArray(stack.pop());
          if (top().kind === 'array' && top().pendingDim) {
            top().pendingDim = false;
            top().row = { elements: [] };
            top().node.rows.push(top().row);
            break;
          }
          /* A new array: an element of the open row (a nested sub-array), or the
             value of the pending assignment. A sub-array shares its parent's
             assignment so the element annotation the C emitted in the innermost
             first row still lands on the array that was assigned. */
          var anode = { type: 'array', dims: 0, rows: [] };
          var parent = top();
          var assign = attach(anode);
          if (!assign && parent.kind === 'array') assign = parent.assign;
          /* An ARRAY-LEVEL annotation precedes the row (`.a = <uint:8> [1,2];`);
             a per-element one is emitted inside it, after this point — so annUnit
             here is the spelling the document actually wrote for the array as a
             whole, and null when it wrote none. An annotation still pending here
             is that array-level one, and it belongs to this assignment. */
          if (annPending) {
            annPending = false;
            if (assign && !arrayLevelSynth.has(assign))
              arrayLevelSynth.set(assign, annSynth);
          }
          var frame = { kind: 'array', node: anode, row: { elements: [] },
                        assign: assign, unitText: annUnit,
                        pendingDim: false };
          anode.rows.push(frame.row);
          stack.push(frame);
          break;
        }
        case 'array_row_end':
          /* Resync unwinds a broken element in the opposite order: the reader
             ends the ROW while the element struct is still open, and only then
             emits struct_close. Taken literally that leaves the row open for the
             rest of the document, and every following assignment lands in it as
             an element — `.a = [{.b = <uint:8> 999;}, …]; .c = 42; .d = 7;` came
             back as a three-element array with no .c and no .d at all. The row
             ends where it was opened, so unwind to that array. */
          if (haveArray()) while (top().kind !== 'array') { stack.pop(); swallowStructClose++; }
          settle();                    /* the innermost sub-array closed first */
          if (top().kind === 'array') top().row = null;
          break;
        case 'array_dim_start':
          /* The next row continues THIS array rather than starting a new one. */
          if (top().kind === 'array') top().pendingDim = true;
          break;
        case 'octet_start': {
          settle();
          /* attached here, not at octet_end, so its chunks accumulate into a node
             that is already parented — an octet stream in an array row is an
             element like any other value */
          oct = { type: 'octet', chunks: [], total: 0 };
          attach(oct);
          break;
        }
        case 'octet_end':
          oct = null;
          break;
        case 'data': {
          if (e.tok === 'octet_stream') {
            if (oct) { oct.chunks.push({ length: e.bytes || 0 }); oct.total += (e.bytes || 0); }
            break;
          }
          settle();
          var inRow = !!openRow();
          var node = scalarNode(e);
          var a = attach(node);
          // Only numeric families carry a meaningful (synthesised or explicit)
          // type annotation; reference/symbol/null/string/bool values have no
          // family, so emitting annFromValue for them would render a bogus
          // "synthesised → family vt_undefined" node in the playground tree.
          if (a && NUMERIC[node.familyName]) a.ann = annFromValue(node, annSynth);
          if (a && annUnit && node.unit) a.unitText = annUnit;
          /* An element claims the annotation that precedes it; that is the whole
             of §7.5, and it is why the tree can show one per element instead of
             one per array. The reader emits a group only where the type in effect
             CHANGES, so an element that simply inherits it carries none — and its
             row shows the resolved type beside the value, as the C does. */
          if (annPending && inRow && NUMERIC[node.familyName])
            node.ann = annFromValue(node, annSynth);
          annPending = false;
          break;
        }
        case 'stream_end': settle(); break;
        default: break;   /* type_* events are folded into the data node's resolved type */
      }
    }
    /* Resync can truncate a document mid-container. Every node is already
       attached where it belongs, so this only finishes the arrays' bookkeeping. */
    while (stack.length > 1) {
      var f = stack.pop();
      if (f.kind === 'array') closeArray(f);
    }
    /* ── an array's type annotation ──────────────────────────────────────
     *
     * The elements of one array may each carry their own annotation (§7.5), and
     * their encodings are free to differ as long as the physical dimension
     * agrees (§7.4) — `[<uint:8> 1, <sint:16> -2, <float:64> 3.5]` is a document
     * `bovnar validate` accepts. The C says so plainly: it emits one annotation
     * group per element whose type differs from the one in effect, three of them
     * for that array. This used to assume there was only ever one, hoist the
     * first element's onto the assignment and drop the rest — presenting the
     * array as a uint:8, a type neither -2 nor 3.5 has.
     *
     * So an annotation is placed where the stream emitted it, and nowhere else:
     * one written before the '[' is the array's and hangs off the assignment,
     * one emitted inside a row is the element's and hangs off that element
     * (attached above, as the elements arrive). Hoisting an in-row annotation to
     * the assignment because it HAPPENS to govern every element would put the
     * same event in two different places depending on the data, in a tree whose
     * whole claim is that it shows the events in the order the reader delivers
     * them.
     *
     * mixedTypes is what remains of that question, and it is a rendering hint
     * rather than a placement: it says the elements do not all resolve to one
     * type, so the row's collapsed one-line summary should print the type beside
     * each value. When they agree, that would be the same annotation repeated
     * once per element.
     */
    function typeKey(v) {
      if (v.type !== 'scalar' || !NUMERIC[v.familyName]) return null;
      return v.familyName + ':' + v.width
           + (v.familyName === 'float_fix' ? ',q' + v.base : '')
           + ',' + (v.epoch || '') + ',' + (v.unit || '');
    }
    /* Every numeric leaf of this array, its own rows and its sub-arrays' — but
       NOT a struct element's fields, which are that struct's business and carry
       no obligation to match anything outside it (§7.4 "fields free"). */
    function typedLeaves(arr, out) {
      for (var r = 0; r < arr.rows.length; r++) {
        var els = arr.rows[r].elements;
        for (var c = 0; c < els.length; c++) {
          if (els[c].type === 'array') typedLeaves(els[c], out);
          else if (typeKey(els[c])) out.push(els[c]);
        }
      }
      return out;
    }
    /* One verdict for the whole assignment, sub-arrays included: a matrix must
       not print its element types on one row and omit them on the next. */
    function markArray(arr, mixed) {
      arr.mixedTypes = mixed;
      for (var r = 0; r < arr.rows.length; r++) {
        var els = arr.rows[r].elements;
        for (var c = 0; c < els.length; c++)
          if (els[c].type === 'array') markArray(els[c], mixed);
      }
    }
    function settleArray(assign) {
      var leaves = typedLeaves(assign.value, []), mixed = false;
      for (var i = 1; i < leaves.length; i++)
        if (typeKey(leaves[i]) !== typeKey(leaves[0])) { mixed = true; break; }
      markArray(assign.value, mixed);
      /* Rebuilt from the first resolved element, exactly as the scalar path does
         — the annotation events carry the declared spelling, and these rows show
         what the reader resolved it to. */
      if (arrayLevelSynth.has(assign) && leaves.length)
        assign.ann = annFromValue(leaves[0], arrayLevelSynth.get(assign));
    }
    function visitValue(v) {
      if (v.type === 'struct') { visitScope(v); return; }
      if (v.type !== 'array') return;
      for (var r = 0; r < v.rows.length; r++)
        for (var c = 0; c < v.rows[r].elements.length; c++)
          visitValue(v.rows[r].elements[c]);
    }
    function visitScope(node) {
      for (var i = 0; i < node.children.length; i++) {
        var a = node.children[i];
        if (!a.value) continue;
        if (a.value.type === 'array' && !a.ann) settleArray(a);
        visitValue(a.value);
      }
    }
    visitScope(root);
    return root;
  }
  function mapErrors(errObj) {
    return (errObj.errors || []).map(function (e) {
      return { code: e.error_name, codeNum: e.error, message: e.error_name,
               line: e.line, col: e.column, offset: e.offset, tier: 'stream' };
    });
  }
  /*
   * The streaming pass alone is not the whole of validity: array homogeneity,
   * struct shape and duplicate keys are document-tier rules (spec §7.4, §12.4),
   * checked when the document is materialised. bvnr_read() never sees them, so
   * the playground reported a green "ok · no errors" for documents `bovnar
   * validate` rejects -- `.a = [{.b=3;},{.c=4;}];` (struct_shape_mismatch),
   * `.x=1; .x=2;` (duplicate_struct_key), `[1,"two"]` (array_element_type_mismatch).
   * src/bovnar.c had exactly this hole in `bovnar events` and closed it the same
   * way: run the DOM pass too, and only when the streaming tier came back clean
   * (a resynced document is already reported, and its DOM is not the one the
   * author wrote). The error carries no position -- bvn_dom_doc_get_parse_error
   * is a code, not a site -- so it is flagged as document-tier and rendered
   * without a line:column jump target.
   *
   * Called from parseFaithful, NOT from the shared rawParse: the playground is
   * the only surface that shows this channel, and the demos re-parse a freshly
   * encoded frame every animation step -- a text the memo below can never hit.
   * The pass costs ~64 us against ~204 us for the parse itself on a demo-sized
   * frame, so charging it to the demos would be a third more work per frame for
   * a verdict nothing there displays.
   */
  /*
   * bvn_dom_doc_get_parse_error is a code and nothing else — no offset, no line.
   * But every one of these rules is a property of a node in the tree that is
   * already built, so the site can be recovered: the second occurrence of a
   * repeated key, the array whose elements disagree. Best effort by design; when
   * nothing matches, the error keeps its positionless form and the playground
   * renders it without a jump target rather than sending the caret somewhere
   * invented.
   */
  function elementKind(v) {
    if (!v) return '?';
    if (v.type === 'struct' || v.type === 'array' || v.type === 'octet') return v.type;
    if (v.type === 'null') return null;          /* a null element matches anything */
    if (v.type === 'reference') return 'reference';
    return v.kind === 'special' ? 'number' : v.kind;
  }
  function structShape(v) {
    var keys = [];
    for (var i = 0; i < v.children.length; i++) keys.push(v.children[i].key);
    return keys.sort().join('\u0000');
  }
  function elementsAgree(row, shape) {
    var kind = null, structShapeSeen = null, len = null;
    for (var i = 0; i < row.length; i++) {
      var k = elementKind(row[i]);
      if (k === null) continue;                  /* nulls are shape-free */
      if (shape === 'kind') {
        if (kind === null) kind = k; else if (kind !== k) return false;
      } else if (shape === 'struct' && row[i].type === 'struct') {
        var sh = structShape(row[i]);
        if (structShapeSeen === null) structShapeSeen = sh;
        else if (structShapeSeen !== sh) return false;
      } else if (shape === 'length' && row[i].type === 'array') {
        var n = row[i].rows.length ? row[i].rows[0].elements.length : 0;
        if (len === null) len = n; else if (len !== n) return false;
      }
    }
    return true;
  }
  function arrayBreaks(v, code) {
    var r;
    if (code === 'array_element_type_mismatch') {
      for (r = 0; r < v.rows.length; r++)
        if (!elementsAgree(v.rows[r].elements, 'kind')) return true;
    } else if (code === 'struct_shape_mismatch') {
      for (r = 0; r < v.rows.length; r++)
        if (!elementsAgree(v.rows[r].elements, 'struct')) return true;
    } else if (code === 'array_row_size_mismatch') {
      for (r = 1; r < v.rows.length; r++)
        if (v.rows[r].elements.length !== v.rows[0].elements.length) return true;
      for (r = 0; r < v.rows.length; r++)
        if (!elementsAgree(v.rows[r].elements, 'length')) return true;
    }
    return false;
  }
  function locateDocError(code, tree) {
    function walk(children) {
      var seen = {}, i, a, hit;
      for (i = 0; i < children.length; i++) {
        a = children[i];
        if (code === 'duplicate_struct_key' && a.key != null) {
          /* the SECOND one: the first is the definition, the repeat is the fault */
          if (Object.prototype.hasOwnProperty.call(seen, a.key)) return a;
          seen[a.key] = 1;
        }
        if (a.value && a.value.type === 'struct') {
          hit = walk(a.value.children);
          if (hit) return hit;
        } else if (a.value && a.value.type === 'array') {
          if (arrayBreaks(a.value, code)) return a;
          hit = walkArray(a.value);
          if (hit) return hit;
        }
      }
      return null;
    }
    /* a struct or sub-array nested inside a row can break the rule too */
    function walkArray(v) {
      var hit, r, i, el;
      for (r = 0; r < v.rows.length; r++)
        for (i = 0; i < v.rows[r].elements.length; i++) {
          el = v.rows[r].elements[i];
          if (el.type === 'struct') { hit = walk(el.children); if (hit) return hit; }
          else if (el.type === 'array') { hit = walkArray(el); if (hit) return hit; }
        }
      return null;
    }
    try { return walk(tree.children); } catch (_) { return null; }
  }
  function docTierError(text) {
    try {
      /* validate() runs the document tier itself and says which tier refused.
         toJSON() was the probe before it did, and is a poor one: it also
         refuses a valid document JSON cannot express (a fractional datetime).
         It stays as the fallback for a core that predates `tier`. */
      var d = wasm.validate(text);
      if (d && d.tier === undefined) d = wasm.toJSON(text);
      else if (d && d.tier !== 'document') return null;
      if (!d || d.ok || !d.error) return null;
      return { code: d.error_name, codeNum: d.error, message: d.error_name,
               line: 0, col: 0, offset: 0, tier: 'document' };
    } catch (_) { return null; }
  }
  /* Single source of both streams, shared by parseFaithful and parseBovnar.
     bvnr_wasm_parse runs ONE reader carrying both callbacks; the two older
     exports each construct their own reader and walk the whole document, so
     using them together cost two passes and two JSON round-trips. The fallback
     is kept for a module that predates the combined export -- note the probe is
     on wasm.parse, which index.mjs defines only when the compiled core actually
     exports it, so a current wrapper paired with a stale core still falls back
     rather than throwing.

     One-entry memo on the input string: the same document is parsed more than
     once (the bovnar-wasm-ready re-render, an example button re-clicked, and
     every demo step re-parses its just-encoded frame). Keyed on string identity,
     so any real edit misses immediately; one entry, so it cannot grow. */
  var lastText = null, lastRaw = null;
  function rawParse(text) {
    if (text === lastText && lastRaw) return lastRaw;
    var events, errObj;
    if (typeof wasm.parse === 'function') {
      var both = wasm.parse(text);
      events = both.events || [];
      errObj = both;
    } else {
      events = wasm.events(text).events || [];
      errObj = wasm.errors(text);
    }
    lastRaw = { events: events, errors: mapErrors(errObj) };
    lastText = text;
    return lastRaw;
  }

  function wasmFaithful(text, opts) {
    var raw = rawParse(text);
    if (!raw.tree) raw.tree = buildTree(raw.events, makeScan(text));
    if (!(opts && opts.documentTier))
      return { events: raw.events, errors: raw.errors, tree: raw.tree };
    /* Memoised beside the tree, and kept OUT of raw.errors: that array is what
       the demos read, and they have no channel for a positionless
       document-tier error -- they would refuse a config over an error reported
       at "line 0", and pay for the extra DOM pass on every animation frame. */
    if (raw.faithfulErrors === undefined) {
      var docErr = raw.errors.length ? null : docTierError(text);
      if (docErr) {
        var site = locateDocError(docErr.code, raw.tree);
        if (site) { docErr.line = site.line; docErr.col = site.col; }
      }
      raw.faithfulErrors = docErr ? raw.errors.concat([docErr]) : raw.errors;
    }
    return { events: raw.events, errors: raw.faithfulErrors, tree: raw.tree };
  }

  /* ── parseBovnar: translate the verified event stream to the live cb ─────
   *
   * ev_assignment_end is this shim's own event — bvnr_event_t has no such
   * member — so it is on us to close each assignment exactly once, when its
   * VALUE ends. That needs the same container stack buildTree uses, and for the
   * same reason: a struct or an array inside an array row is an element, not an
   * assignment, and must not close one. A single "an array is open" flag emitted
   * an assignment_end at the first field of a struct inside an array, one more
   * after each element struct, and never closed the assignment that actually
   * owned the array. */
  function wasmBovnar(text, cb) {
    var raw = rawParse(text);
    var events = raw.events;
    var scan = makeScan(text);
    var line = 0;
    function E(type, data) { cb({ type: type, data: data || {}, line: line }); }
    var stack = [{ kind: 'struct' }];
    function top() { return stack[stack.length - 1]; }
    /* true while the innermost container is an array row accepting elements —
       i.e. a value ending right now is an element and closes no assignment */
    function inRow() { var f = top(); return f.kind === 'array' && !!f.row; }
    function endValue() {
      if (inRow()) return;              /* an element closes no assignment */
      E(EV.ASSIGNMENT_END);
      top().pending = false;
    }
    /* In resync mode a malformed assignment is skipped after its
       ev_assignment_start, so its value events never arrive. Close it when the
       next event proves it is over, or the stream would carry an
       ev_assignment_start with no end and a consumer's nesting would drift for
       the rest of the document. */
    function flushPending() { if (!inRow() && top().pending) endValue(); }
    /* close every array whose last row has ended (see buildTree: the C stream
       has no array-end event, so this is deferred until an event proves it) */
    function settle() {
      while (top().kind === 'array' && !top().row) { stack.pop(); endValue(); }
    }
    function haveArray() {
      for (var k = 0; k < stack.length; k++) if (stack[k].kind === 'array') return true;
      return false;
    }
    /* Close what is still open. A document that dies mid-container gets no
       ev_stream_end at all — the reader stops at the error — so without this the
       last assignment, struct and row are never closed and a consumer's nesting
       never returns to the top. ev_stream_end itself is NOT synthesised: its
       absence is how the C says the document was cut short. */
    function drain() {
      while (stack.length > 1) {
        var f = stack.pop();
        if (f.kind === 'array') { if (f.row) E(EV.ARRAY_ROW_END); }
        else E(EV.STRUCT_END);
        endValue();
      }
      flushPending();
    }
    var sawStreamEnd = false;
    /* struct_close events already accounted for by a resynced array_row_end */
    var swallowStructClose = 0;
    for (var i = 0; i < events.length; i++) {
      var e = events[i];
      switch (e.ev) {
        case 'stream_start': E(EV.STREAM_START); break;
        case 'assign_start':
          settle();
          flushPending();              /* the previous assignment was resynced away */
          line = scan(e.text).line;
          E(EV.ASSIGNMENT_START, { key: e.text });
          top().pending = true;
          break;
        case 'type_start': E(EV.TYPE_ANN_START); break;
        case 'type_family': E(EV.TYPE_ANN_FAMILY, { family: e.family }); break;
        case 'type_param': {
          var d;
          if (e.tok === 'type_width')      d = { kind: 'width', value: e.width, text: String(e.width) };
          else if (e.tok === 'type_base')  d = { kind: 'base',  value: e.base,  text: '_' + e.base };
          else if (e.tok === 'unit')     { if (e.text === 'no_unit') break;   // synthesized "no unit" sentinel — not a real unit
                                           d = { kind: 'unit', text: e.text }; }
          else if (e.tok === 'type_q')     d = { kind: 'q',     text: e.text };
          else                             d = { kind: 'param', text: e.text };
          E(EV.TYPE_ANN_PARAM, d);
          break;
        }
        case 'type_end': E(EV.TYPE_ANN_END); break;
        case 'struct_open':
          settle();
          E(EV.STRUCT_START);
          stack.push({ kind: 'struct' });
          break;
        // Close a struct-trailing array (array_row_end → struct_close) before
        // closing the struct, so it stays parented to this struct rather than
        // the outer scope.
        case 'struct_close':
          if (swallowStructClose) { swallowStructClose--; break; }
          while (stack.length > 1 && top().kind === 'array') { stack.pop(); endValue(); }
          flushPending();      /* last field resynced away */
          E(EV.STRUCT_END);
          if (stack.length > 1) stack.pop();
          endValue();          /* a struct that is an array element closes nothing */
          break;
        case 'array_row_start':
          /* pop finished sub-arrays, but never one awaiting its '/'-dimension row */
          while (top().kind === 'array' && !top().row && !top().pendingDim) {
            stack.pop(); endValue();
          }
          if (top().kind === 'array' && top().pendingDim) {
            top().pendingDim = false;
            top().row = true;
          } else {
            stack.push({ kind: 'array', row: true, pendingDim: false });
          }
          E(EV.ARRAY_ROW_START);
          break;
        case 'array_row_end':
          /* see buildTree: resync ends the row while the element struct is still
             open. Close that struct here — the row it lives in is over — and
             swallow the struct_close that follows, so the stream stays nested
             instead of ending a row inside a struct it also never left. */
          while (haveArray() && top().kind !== 'array') {
            flushPending();              /* the field the resync abandoned */
            stack.pop();
            E(EV.STRUCT_END);
            swallowStructClose++;
            endValue();
          }
          settle();                      /* the innermost sub-array closed first */
          E(EV.ARRAY_ROW_END);
          if (top().kind === 'array') top().row = false;
          break;
        case 'array_dim_start':
          if (top().kind === 'array') top().pendingDim = true;
          E(EV.ARRAY_DIM_START);
          break;
        case 'octet_start': settle(); E(EV.OCTET_STREAM_START); break;
        case 'octet_end': E(EV.OCTET_STREAM_END); endValue(); break;
        case 'data':
          if (e.tok === 'octet_stream') {
            E(EV.DATA, { kind: 'octet', value: null, text: '', length: e.bytes });
          } else {
            settle();
            E(EV.DATA, { kind: dataKind(e), value: dataValue(e),
                         text: e.text != null ? e.text : '', unit: e.unit || null });
            endValue();
          }
          break;
        case 'stream_end':
          drain();
          sawStreamEnd = true;
          E(EV.STREAM_END);
          break;
        default: break;
      }
    }
    if (!sawStreamEnd) drain();
    // Same pass as the events above -- this was a second full walk of the
    // document. mapErrors already normalised error_name onto .code/.message.
    var errs = raw.errors;
    for (var k = 0; k < errs.length; k++)
      cb({ type: EV.ERROR, data: { msg: errs[k].message, code: errs[k].code }, line: errs[k].line });
  }

  /*
   * peekVersion() lived here: a regex for the `#!bovnar` directive, exported
   * beside the parser. Nothing on the site called it, and it did not agree with
   * the reader it sat next to -- /m let it match a directive on ANY line, so a
   * `#!bovnar 1.1` written after a comment or after an assignment read as 1.1
   * where the C declares no version at all and then refuses the 1.1 construct
   * with error_illegal_value_type. A second implementation of a spec rule, on
   * the public surface of the page that promises the reference one.
   *
   * The C already answers this: bvnr_wasm_parse and bvnr_wasm_errors report
   * declared_version straight from bvnr_reader_get_declared_version(). Use that.
   */

  /* ── dispatcher: WASM once ready, empty results until then ───────────────── */
  /*
   * parseFaithful(text[, { documentTier: true }]) -> { events, errors, tree }
   *
   * The document tier is opt-in because it is a second full pass (bvn_dom_parse
   * plus the JSON projection, ~64 us against ~204 us for the parse itself on a
   * demo-sized frame) and its finding has no line:column. The playground asks
   * for it; the demos, which re-parse a freshly encoded frame every animation
   * step, do not.
   */
  function parseFaithful(text, opts) {
    // A throw inside the shim used to return the same empty result as "WASM not
    // loaded yet", so the playground reported a green "ok - 0 events - no errors"
    // for a document it had actually failed on. Flag it so the caller can tell.
    var failed = false;
    if (wasm) {
      try { return wasmFaithful(text, opts); }
      catch (e) { failed = true; if (window.console) console.warn('bovnar wasm parseFaithful failed', e); }
    }
    return { events: [], errors: [], tree: { type: 'stream', children: [] }, failed: failed };
  }
  function parseBovnar(text, cb) {
    if (wasm) { try { return wasmBovnar(text, cb); } catch (e) { if (window.console) console.warn('bovnar wasm parseBovnar failed', e); } }
  }

  window.BovnarParser = { parseFaithful: parseFaithful, parseBovnar: parseBovnar,
                          EV: EV,
                          isWasmReady: function () { return !!wasm; } };
})();
