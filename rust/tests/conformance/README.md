# Conformance corpus

`corpus.json` holds the 470 test cases of the Bovnar conformance suite,
extracted from `tests/bvnr_conformance.c` of Bovnar
(<https://github.com/sothis/bovnar>, pinned commit `1bcf03b`),
Copyright (c) 2026 Janos Sonntag, licensed under the MIT License (see
`vendor/upstream/LICENSE`). The inputs, identifiers and descriptions are the
upstream author's; this repository only changes their encoding.

## Format

Each case records the upstream fields (`id`, `group`, `description`,
`expect`, `expected_error`, `continue_on_error`, `limits` with the `--max-*`
overrides, `dom_validate`, `expect_key`, `expect_data`, `iut_skip_reason`),
the `input` as base64, and, for every streaming case, the `reference` result:
the exact stdout (base64) and exit code of the C reference IUT adapter
`bvnr_conformance_iut`, run with the case's limit options. DOM-tier cases
(`dom_validate: true`) are not part of the streaming IUT protocol and only
carry their expected error.

The reference is built with the UCUM and CF unit profiles switched off,
because bovnar-rs does not ship them. Cases whose reference result therefore
differs from the upstream expectation are marked
`"profile_off_deviation": true`; bovnar-rs must match the reference, not the
upstream expectation, for those.

## Regenerating

```sh
scripts/build-cref.sh <upstream> <cref-build-dir>
python3 -I scripts/extract-corpus.py <upstream> <cref-build-dir> tests/conformance/corpus.json
```

Run the upstream driver against the Rust adapter with:

```sh
cargo build --release -p bovnar-iut
<cref-build-dir>/tests/bvnr_conformance --iut target/release/bovnar-iut
```
