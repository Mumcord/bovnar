---
title: 'Bovnar: a unit-safe serialization format with parse-time dimensional validation'
tags:
  - C
  - Python
  - serialization
  - units of measure
  - dimensional analysis
  - metrology
  - data interchange
authors:
  - name: Janos Sonntag
    orcid: 0009-0008-1299-6534
    affiliation: 1
affiliations:
  - name: Independent Researcher, Germany
    index: 1
date: 10 October 2026
bibliography: paper.bib
---

# Summary

Bovnar (BVNR) is a text-based serialization format in which every value carries
its own type family, bit width, numeric base and **physical unit** inline, in
the byte stream, with no external schema. The unit is part of the value's
grammar rather than a naming convention layered over it, so a document that
contradicts itself does not parse:

```
.speed = <float:64,m/s> 9.81 k~m/h;
#                            ^ error_unit_mismatch, at parse time, for every consumer
```

No units library had to be called and no validation pass had to be remembered:
the check is the parse. The same holds for assertions a reader makes about data
it did not write — `bovnar validate --require-unit`, or
`--require-dimension m`, which accepts a document of lengths in whatever unit it
chose to write them.

The reference implementation is a dependency-free C99 library providing a
streaming (SAX-style) reader, a DOM API, a canonicalizing writer, a
command-line tool, a WebAssembly build, and pure-`ctypes` Python bindings with
optional NumPy and Pint [@pint] bridges. It ships a registry of 264 physical
units (SI base and derived units, IEC binary prefixes [@si; @iec80000-13]),
216 fiat and cryptocurrency denominations treated as first-class dimensions,
and nine time epochs, with leap seconds resolved from the IERS TAI-UTC table:
`2016-12-31T23:59:60Z` is a representable instant in TAI and, correctly, is not
one in POSIX time.

# Statement of need

In scientific and industrial data exchange, the expensive failures are rarely
syntactic. A value sent in pound-force and read as newtons, or in feet and read
as metres, parses perfectly; only the dimension is wrong. The canonical case
remains the loss of the Mars Climate Orbiter, where a ground-software module
supplied impulse in pound-force-seconds to a navigation system expecting
newton-seconds [@mco1999]. The general-purpose interchange formats offer no
defence: JSON [@rfc8259], YAML [@yaml] and CBOR [@rfc8949] model numbers,
strings and containers, and a unit can only be smuggled through them as a
naming convention or a sibling key that nothing enforces.

Bovnar targets the case where the receiving party may not share the sender's
schema, or where an archive may be opened decades after its writer is gone:
scientific instrumentation and metrology, industrial telemetry, IoT sensor
networks, long-term measurement archival, and mixed text–binary log streams.

# State of the field

Existing approaches to units in data all sit at a different layer than the wire
format, and each leaves a gap:

- **Unit code systems** — UCUM [@ucum], UN/CEFACT Recommendation 20 [@unece20],
  QUDT [@qudt] and UDUNITS-2 [@udunits] — standardize how a unit *string* is
  spelled, each for its own community. None says anything about where that
  string lives, what it is attached to, or who checks it: validation happens if
  and when an application asks for it.
- **CF conventions** [@cf] over netCDF [@netcdf] attach a `units` attribute to
  an entire variable, checked after the fact by a separate tool. That is the
  right model for a homogeneous array and no model at all for a heterogeneous
  document mixing configuration, measurements, timestamps and a binary payload.
  Within earth-system science, CF and its tooling remain the appropriate
  choice, and Bovnar does not attempt to displace them.
- **In-memory quantity libraries** such as Pint [@pint] and `astropy.units`
  [@astropy2022] enforce dimensions rigorously inside one process, but the unit
  is lost at the serialization boundary unless both endpoints agree on an
  out-of-band encoding.
- **Language-level units of measure**, as in F# [@kennedy2010], are checked at
  compile time and do not cross the wire at all.

Bovnar occupies the space between JSON (no type, no unit) and netCDF (arrays,
external schema, binary container): heterogeneous, self-describing documents in
which the enforcement point is the parser. Because these code systems address a
different layer, they are components rather than rivals: specification 1.2
adds seven unit *profiles* that translate a foreign code into a native unit at
parse time — `ucum:`, `unece:`, `qudt:`, `qudt-qk:`, `udunits:`, `om:` (OM 2)
and the read-only `cf:` (CF standard names). A document opts in to the
notation with its version directive.
Because every profile resolves to the same internal representation, a code
written in one vocabulary compares equal to the same quantity written in
another, and the reference implementation checks that property across the
vocabularies with a cross-vocabulary test suite.

Beyond units, the format provides several features aimed at measurement data:
native binary embedding through length-prefixed octet streams (no Base64
expansion and no escaping, at the cost of a document that must be treated as
binary in transit), native multi-dimensional array syntax that does not reduce
to nested lists, intra-document references, decimal
and fixed-point float families for exact monetary arithmetic, arbitrary-width
integers, and optional error recovery for parsing unreliable log streams.

# Quality control

The implementation is roughly 30,000 lines of C against a test suite of
comparable size: 183 CTest targets covering unit, integration, concurrency and
socket-pair round-trip tests, a Python test suite, self-contained fuzz
harnesses for the reader, writer, DOM and utility layers (with optional
libFuzzer/AFL++ targets), and ASan/UBSan builds. Conformance is defined
independently of the implementation by a 470-case suite with a documented
implementation-under-test protocol, so a third-party parser can be measured
against the same corpus. Continuous integration runs the full suite on Linux
and builds the MSVC and MinGW Windows targets. The command-line tool includes a
benchmark mode for measuring parsing throughput across payload shapes.

# Availability

Bovnar is MIT licensed and developed at <https://github.com/sothis/bovnar>;
the unit-profile tables additionally carry identifier strings from the
vocabularies they translate, which remain under their publishers' own licences
and are recorded in the repository's third-party notices. This paper describes
version 2.0.0 of the reference implementation, which implements version 1.2
of the format; the two are versioned separately. The format is defined by a versioned
specification with a formal EBNF grammar, and has a registered IANA media type,
`text/vnd.bovnar` [@iana_bovnar]; an Internet-Draft describing it has been
prepared for the IETF independent submission stream and is included in the
repository. Releases are archived on Zenodo [@bovnar_software; @bovnar_docs].

# Acknowledgements

The unit and currency registries draw on the SI Brochure [@si], IEC 80000-13
[@iec80000-13] and ISO 4217, and the leap-second table on the IERS TAI-UTC
record.

# References
