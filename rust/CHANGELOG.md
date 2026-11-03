# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Streaming reader for Bovnar specification 1.2 (`Reader`): chunked input,
  constant memory, byte-identical events and error codes to the C reference
  at upstream commit `1bcf03b`.
- Read options: `strict_version`, `text_only` and `recovery`.
- Document tree (`parse`, `parse_with`, `parse_with_options`) with lossless
  numbers, path lookup, builders and the document-tier checks of the C
  reference (unique keys, array homogeneity, struct shape, row geometry).
- Writer for events and trees (`Writer`, `to_string`, `to_vec`, compact
  variants), pretty-printed output byte-identical to the C reference.
- Units of measure: native unit system with SI and IEC prefixes, compound
  units and currencies; optional unit profiles UNECE, QUDT, QUDT-QK, UDUNITS
  and OM behind `profile-*` features. UCUM and CF are not included.
- serde support (`serde` feature): `bovnar::serde::to_string` / `from_str`.
- Streaming helpers: record framing (`write_frame`, `FrameDecoder`,
  `FrameReader`, `read_doc_stream`) and document-in-document embedding.
- Resource limits (`Limits`) for every input-dependent size.
- `no_std` support (with `alloc`); builds for `wasm32-unknown-unknown` and
  `wasm32-wasip2`.
- Conformance IUT adapter (`iut/`), differential tester against the C
  reference (`diff/`), fuzz targets (`fuzz/`) and helper scripts.
- CI: tests on Linux, macOS and Windows, MSRV, WebAssembly, feature
  combinations, generated table check, upstream conformance suite and
  differential test against the C reference.
- Writer checks that are stricter than the C writer where the C writer
  accepts input its own reader rejects, including correct counting of nested
  array rows (see `UPSTREAM_NOTES.md`).
