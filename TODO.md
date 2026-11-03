# TODO

## Closed 2026-11-03 — the open items of the 2026-10-12 spec review

1. **`[]` was formally ambiguous in the grammar.** `row-content` now needs a
   `filled-elem` when the row has one element, so `[]` derives only as the
   empty row (`present-value` split out of `raw-value` in `doc/12_bovnar.ebnf`;
   both ABNF copies in the IETF draft; one sentence in spec §7.2 and the
   draft). No change to what the reader accepts: `[]`/`[ ]` are empty,
   `[<uint:8>]` is one typed null.
2. **`bvnr_reader_get_error` after a recovered error.** New spec §13.4, plus
   the read/write API doc, the Python error-state properties and the header: the getters describe the error that
   ended the read; after a read that returned true they give `error_none` and 0.
   Tested in `bvnr_high_severity_test` (`test_error_getters_after_recovery`)
   and asserted on every input by the reader fuzz harness.
3. **Appendix C** lists the five length limits, their 65 535 ceiling and the
   unit group-nesting limit.
4. **FAQ datetime passage** (`doc/02_bovnar_faq.md`): the error split now
   matches the spec (a wrong-width field is `error_invalid_datetime_literal`),
   and injectivity is limited to the leap seconds the table records.

Review coverage: spec §12–15 and §16–18 were read in full against the code,
and the whole spec was read for cross-section contradictions (two reviewers,
every claim reproduced). The verified wording findings are fixed in doc/03,
doc/12 note n) and the IETF draft.

## Closed 2026-11-03 — the implementation leads from that review

All five fixed, each with a regression test (`test_recovery_review_2026_11` in
`bvnr_high_severity_test`, `test_writer_review_2026_11` in `bvnr_writer_test`)
and a CHANGELOG entry under 2.0.0:

1. A stray multi-byte character is one error, not one per byte.
2. `strict_version` ends the read under `continue_on_error` (an unsupported
   version, and under `strict_version` a malformed directive).
3. `bvnr_reader_get_skipped_bytes` counts from the offending byte up to, not
   including, the resume boundary.
4. `bvnr_write_bvnf` checks the width before writing the key.
5. The writer's key cap is 65 535 bytes, the reader field's ceiling, not 255.

Nothing is open.

## How the review was run

Two read-only subagents, one on §12–15, one on §16–18 plus a whole-spec
consistency pass, each reproducing its claims with the CLI or small C probes
against `build/libbvnr.so`; every finding was re-checked before it was fixed.
