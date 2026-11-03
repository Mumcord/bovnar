# Upstream notes

Places where the Bovnar specification (upstream `doc/`) is ambiguous or
disagrees with the C reference implementation. bovnar-rs follows the C
reference in every such case, so that it stays byte-identical on the
conformance IUT protocol; each entry below records the section, the issue and
what the C code does, for the upstream author.

Pinned upstream commit: `1bcf03b`.

## Entries

None yet.

### Units: `no_unit` inside a parenthesised group

- Section: doc/12 unit sub-grammar (`resolved-unit = "no_unit" | unit-expr`,
  `unit-factor = unit-component | "(" , unit-expr , ")"`); doc/03 §11.8.
- Issue: by the grammar `no_unit` is only a whole unit, never a factor, so
  `m·(no_unit)` should be `error_unit_illegal`.
- What C does: `bvn_parse_unit_expr` re-enters itself for a group and accepts
  `no_unit` there as an empty sub-expression. `m·(no_unit)` parses as `m`,
  while `(no_unit)` and `((no_unit))` are refused because their enclosing
  level ends up with no component.
### Unit parameter nested deeper than 16 groups is dropped silently

- Section: spec §11 (units in type annotations); `bvn_parse_type_annotation`.
- Issue: when a unit parameter nests `{}` or `[]` more than 16 deep, the parser
  flags the unit as bad but returns before recording its text. The validator
  only checks the unit when it has text, so the annotation is accepted with no
  unit at all, and every check after the unit (e.g. an invalid `_99` base in
  `<float:64,{{…{m}…}},_99>`) is skipped as well.
- C: `<float:64,{{{{{{{{{{{{{{{{{m}}}}}}}}}}}}}}}}}> 1` is accepted, reported
  as `float:64` without a unit parameter.

### float_fix range check gives up on huge literals

- Section: spec §6.4 (float_fix range); `bvn_float_str_fits_fix`.
- Issue: the check builds the literal as an exact rational; past the 32768-bit
  integer limit it gives up and reports "fits". `<float_fix:16,q8> 1e10000`
  is accepted while `1e9000` is `error_value_out_of_range`.
- C: a literal too large to represent is accepted as in range.

### float_fix rounds twice

- Section: spec §6.4; `bvn_float_str_fits_fix`.
- Issue: the literal is first rounded to a binary float of width+q+64 bits,
  then to the fixed-point step (ties to even). A value just below a tie can
  therefore round up across the range limit:
  `127.99804687499999999999999999919` is refused for `float_fix:16,q8`, and
  `127.998046875` is refused while `-128.001953125` is accepted.
- C: double rounding as described; the spec states a single round-half-even.

### Fractional ISO-8601 literals have a narrower year range

- Section: spec §6.7 (datetime literals).
- Issue: a literal with a sub-second fraction is refused when its UTC year,
  after applying the offset, falls outside 0000–9999; the same instant without
  a fraction is accepted.
- C: as described.

### The UDUNITS registry comment says space does not multiply; the code says it does

- Section: doc/11 §13.2; `src/utils/bovnar_profiles.c`, the `udunits` row of
  `bvn_profiles[]`.
- Issue: the comment above the row says "SPACE IS NOT IN THIS SET" and asks
  producers to write `kg*m-2*s-1`, while the row's multiplication set is
  `"* "` and doc/11 §13.2 says a space multiplies (UPR-043c pins it).
- C: a space multiplies; `udunits:kg m-2 s-1` equals `udunits:kg*m-2*s-1`.
  The comment is stale.

### `bvn_unit_to_profile` writes a kilotonne as UDUNITS `kilot`

- Section: doc/11 §5.3; `prefix_that_reads_back`.
- Issue: not a defect, but worth stating in §5.3: when the shortest prefix
  spelling would read back as another atom (`kt`, the knot), the writer falls
  back to the long spelling (`kilot`) rather than refusing.
- C: as described.

### A unit parameter nested deeper than 16 groups switches off the type checks

- Section: doc/03 §6 (type annotations), doc/05 (unit grammar);
  `src/bovnar.c`, the unit capture in the annotation parser.
- Issue: when the unit parameter opens more than 16 `[` or `{` groups, C
  flags the unit as illegal but records no unit text, and returns before the
  family checks run. The validator then never reports the flag, so
  `.f = <float:65,[[[[[[[[[[[[[[[[[> 0.5;` is accepted: the illegal width 65
  stands, and the unit is silently dropped. A writer cannot reproduce such a
  value, because `<float:65>` on its own is `error_illegal_value_type`.
  Found by fuzzing; bovnar-rs matches C, so its tree writer refuses to write
  a document that holds such a value.
- C: accepts the document; expected would be `error_unit_illegal` (or at
  least `error_illegal_value_type` for the width).

### The writer takes an empty struct under a type annotation

- Section: doc/08 (writer), doc/03 §8 (structs); `src/writer/bovnar_writer.c`,
  `ev_struct_start` and `ev_assignment_start`.
- Issue: the reader refuses every struct that stands under a type
  annotation (`.a = <sint> {};` and `.a = <sint> [{}];` are
  `error_type_value_mismatch`), but the writer only refuses the first
  member of such a struct. An empty typed struct is written without error
  and produces a document the reader rejects. Found by fuzzing.
- C: the writer accepts `<sint>` + struct start + struct end. bovnar-rs
  refuses the struct start instead (`error_type_value_mismatch`), so its
  writer never emits what its reader refuses.

### The writer counts a nested row as two elements

- Section: doc/03 §7 (arrays, row widths); `src/writer/bovnar_writer.c`,
  `ev_array_row_start` (`arr_count += w_after_dim ? 1 : 2`).
- Issue: the reader counts every element of a row once, a nested array
  (even one continued with `/`) included: `[1]/[[2]]` and
  `[[1],[2]]/[3,4]` are valid, `[1,2]/[[3]]` is
  `error_array_row_size_mismatch`. The writer counts a new nested row as two
  and a `/`-continued one as one more, so it refuses the valid documents and
  writes the invalid ones. The comment there ("one more per nested row")
  does not match the reader. Found by fuzzing the tree writer.
- C: as described. bovnar-rs counts a nested row once and a continued row
  not at all, like the reader.

### An untyped integer inherits a datetime type from the element before it

- Section: doc/03 §6.6 (type inference for untyped values), doc/08 (writer);
  `src/validator/bovnar_validator.c`, `bvn_emit_default_type_annotation`.
- Issue: within one assignment, a plain integer after a datetime (explicit,
  even on a `null`, or an ISO literal) is typed as that datetime:
  `[<datetime:64,gps> null, -9223372036854775808]` is
  `error_value_out_of_range`, while `[null, -9223372036854775808]` is valid.
  The state survives `null`, nested rows and empty structs, and is reset by
  a struct with members. This is not stated in the spec, and the C writer
  does not model it: it writes the integer untyped and so produces a
  document its reader rejects. Found by fuzzing the writer.
- C: as described. bovnar-rs reads the same way and its writer validates
  such an integer against the inherited datetime type.

### `bovnar convert` accepts JSON containers left open at the end of input

- Section: README, `bovnar convert` (JSON is RFC 8259); `json_parse_array`
  and `json_parse_object` in `src/bovnar.c`.
- Issue: the loops run `while (**p)`, so an array or object still open when
  the input ends right after `[`, `{` or a `,` is returned as complete.
  `{` converts to an empty document, `{"a":{"b":1,` fails only because the
  outer object then sees the end of input instead of `}`.
- What C does: accepts the truncated container. bovnar-rs does the same.

### `bovnar convert` reads an integer past the end of the JSON number

- Section: `json_parse_value_depth` in `src/bovnar.c`.
- Issue: the number is scanned by the RFC 8259 grammar (`0` cannot be
  followed by digits), but converted with `strtoll`/`strtoull`, which read
  the whole digit run. `099999999999999999999` is therefore reported as
  "integer literal out of range" instead of a malformed number.
- What C does: the range error wins. bovnar-rs does the same.

### The shortest float text picks its last digit from 21 truncated digits

- Section: `bvnf_shorten_digits` / `bvnf_to_str_dec` in
  `src/utils/bvn_float.c` (used by `bvnr_write_float`, and so by
  `bovnar convert` for every JSON float outside an array).
- Issue: the decimal digits are extracted 21 deep and truncated; when the
  shortest form needs 17 digits, the choice between the two 17-digit
  neighbours is made from those 21 digits only, so it is not always the
  nearer one: 2.7880911803355767e-266 is written as
  `2.7880911803355766e-266`. Both read back as the same double. The search
  also checks reading back at 53 bits with no exponent floor, so subnormals
  get up to 17 digits (`5e-324` is written as `4.9406564584124654e-324`).
- What C does: as described. bovnar-rs ports the algorithm digit for digit.

### The JSON loss report counts nested arrays and nulls under an array's unit

- Section: `json_loss_scan` in `src/bovnar.c`; doc/03 (an annotation before
  a bracket array applies to its elements).
- Issue: the unit of `<float:64,m> [[1.0], [null]]` is counted for the outer
  array, each nested array and each null, as well as each number: five
  "value(s) lose their unit" where the document wrote one unit.
- What C does: as described. bovnar-rs reports the same counts.
