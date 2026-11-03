# TODO

## Handover 2026-11-03 — the whole-project review, moved to another machine

State at handover: everything below is committed on `main` in one commit, not
pushed, not tagged (2.0.0 waits for the maintainer's word). Verified on that
tree: a clean rebuild with all 183 CTest tests passing, the ASan+UBSan build
157/157 (`-LE python`), a clang `BVNR_WERROR` build clean, and a GCC 13
`BVNR_WERROR` build clean in an Ubuntu 24.04 container (the CI compiler).
The WASM module, `web/`, the LLM views, the sitemap and the IETF draft
renderings were regenerated from the committed sources.

What the review changed is in CHANGELOG.md under 2.0.0, "Fixed — a
whole-project review before release". What it left open is the next section.

To pick it up on a new machine:

- `build/` and `build-asan/` are git-ignored and do not travel. Configure
  them again: `cmake -B build -DCMAKE_BUILD_TYPE=RelWithDebInfo .` and
  `cmake -B build-asan -DBVNR_SANITIZE=ON -DCMAKE_BUILD_TYPE=Debug .`, then
  `./run_tests.sh` (it builds first; `ctest` alone does not).
- `web/de/` is git-ignored as well; `python3 gen_i18n.py de` recreates it.
- After any C change, regenerate in this order: `wasm/build_wasm.sh` (needs
  emsdk), `gen_html_docs.py`, `gen_llms.py`, `gen_csp.py` if an inline script
  in `web/index.html` changed, `gen_i18n.py de` last. The WASM freshness
  stamp now also covers `wasm/build_wasm.sh`, and the sync gate now compares
  every file of `dist/wasm` with its source, so an edit to `wasm/README.md`,
  `wasm/package.json`, `LICENSE` or `THIRD_PARTY_NOTICES.md` also needs a
  `build_wasm.sh` run.
- The IETF draft renderings are rebuilt by hand from
  `doc/ietf/draft-sonntag-bovnar-00.md` with `kramdown-rfc` and
  `xml2rfc --text --html`; `check_ietf_draft.py` checks all four.
- `check_profile_factors.py` needs the publishers' files: run it with
  `--fetch` once (network) before `--strict`. The review's run against them
  compared 10 917 rows with no mismatch.
- The CI tests job now configures with `-DBVNR_WERROR=ON`. The first push
  runs it on GitHub for real; if a runner's GCC warns where 13 and 15 did
  not, that job is where it shows.

## Open 2026-11-03 — decisions the whole-project review left to the maintainer

1. **The 2.0.0 date.** Set to 2026-11-04 on 2026-11-03 in CHANGELOG,
   CITATION.cff (`date-released`), the JOSS paper and both wiki drafts (was
   2026-10-10). If the tag lands on another day, move them again.
2. **Native `eq`** — decided 2026-11-03: it stays an alias of `val`, the
   water-analysis equivalent (½ mol), so native `meq/L` is half of
   `ucum:meq/L`. Stated as settled in doc/05, with the clinical trap in doc/07,
   the profile note in doc/11 and the FAQ; pinned in `bovnar_ucum_test`.
3. **`ucum:[IU]` and `ucum:[iU]`** — done 2026-11-03: one opaque id, `[iU]`
   an alias of `[IU]` through `.opaque_aliases` in `ucum.bvnr` (doc/11 §3.6,
   `test_arbitrary` in `bovnar_ucum_test`). UCUM's block is 200000–200039.
4. **Quoted float literals take a leading `+`** — decided 2026-11-03: kept.
   doc/03 §6.3 ("Signs in Quoted String Literals") and the IETF draft say a
   quoted float may begin with one `+` and a quoted integer may not.
5. **`-2026-06-15` under `#!bovnar 1.1`** — decided 2026-11-03: the reader's
   `error_invalid_datetime_literal` where the value ends stays; doc/03 §5.1
   and the IETF draft now say a leading `-` keeps the skeleton.
6. **Profile coverage.** After the eight mapped in this round,
   `check_profile_factors.py` still lists 53 refusals an existing native
   expression would cover. They are coverage to review, not errors.

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
