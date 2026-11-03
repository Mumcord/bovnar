# Third-party notices

bovnar-rs is a Rust implementation of Bovnar
(<https://github.com/sothis/bovnar>, Copyright (c) 2026 Janos Sonntag). This
file records what bovnar-rs takes from that project and under which terms. The
complete upstream `LICENSE` and `THIRD_PARTY_NOTICES.md` are kept unchanged in
[`vendor/upstream/`](vendor/upstream/); the sections reproduced below are copied
from Part 1 of the upstream notices, and references in them to upstream file
paths (`src/gendata/`, `doc/`, `check_profile_factors.py`, ...) mean the
upstream repository.

## What bovnar-rs uses from Bovnar

* **Specification-derived code.** The reader follows the Bovnar specification
  (upstream `doc/`, licensed CC BY 4.0) and the behaviour of the C reference
  implementation (MIT). See the attribution in [`LICENSE`](LICENSE).
* **Conformance corpus.** `tests/conformance/corpus.json` is extracted from
  upstream `tests/bvnr_conformance.c` (MIT); see
  [`tests/conformance/README.md`](tests/conformance/README.md).
* **Unit, prefix and currency tables** derived from upstream
  `src/gendata/units.bvnr`, the SI/IEC prefix tables and
  `src/gendata/currencies.bvnr` (MIT for the project's own registry; ISO 4217
  and CODATA as recorded under "Other reference data" below).
* **Unit-profile tables** for five external vocabularies, each behind its own
  Cargo feature:

  | Profile | Cargo feature |
  |---|---|
  | UN/ECE Recommendations 20 and 21 | `profile-unece` |
  | QUDT units | `profile-qudt` |
  | QUDT quantity kinds | `profile-qudt-qk` |
  | UDUNITS-2 | `profile-udunits` |
  | OM 2 | `profile-om` |

* **Date algorithms.** The civil-date conversions follow the C reference, which
  derives them from Howard Hinnant's public-domain algorithms (section below).

## What bovnar-rs does NOT include

**The UCUM and CF tables are not included**, in source or in binary form, under
any feature. bovnar-rs never copies data from upstream `src/gendata/ucum.bvnr`
or `src/gendata/cf.bvnr`. Units in the `ucum:` and `cf:` namespaces are refused
with `unit_profile_unknown`, exactly as the C reference behaves when built with
`-DBVNR_WITH_UCUM_PROFILE=OFF -DBVNR_WITH_CF_PROFILE=OFF`. The upstream notices
for those two vocabularies therefore do not apply and are not reproduced here
(they remain readable in `vendor/upstream/THIRD_PARTY_NOTICES.md`).

---

# Notices copied from upstream (Part 1 — Unit vocabularies)

## What is taken, and what is not

**Taken:** identifier strings — a UCUM atom code, a QUDT or OM local name, a
UN/ECE common code, a UDUNITS spelling, a CF standard name. Alongside them, two
kinds of short upstream-derived text, both stated here rather than glossed over:

* **The conventional NAME of a unit**, where a row would otherwise be
  unreadable. A three-letter Rec 20 code is unintelligible without one, so most
  rows of `unece.bvnr` carry a trailing comment such as `# statute mile` or
  `# barrel, US petroleum`; the refusal strings in `ucum.bvnr` do the same, and a
  few of those match UCUM's own `<name>` field word for word (`%[slope]` is
  "percent of slope" in both). These are short factual designations of physical
  units, not the expressive content of any publication, and no mapping table can
  be written or reviewed without them.
* **CF's `canonical_units` field**, which `cf.bvnr` carries in a trailing comment
  on every row and translates into that row's target. §17.2 of
  `doc/11_bovnar_unit_profiles.md` is built on it: it is *what* a `cf:` name
  translates to, so it cannot be paraphrased away.

**Not taken:** no upstream definition, description, annotation, property,
`printSymbol` or explanatory prose. Where a `.bvnr` row carries reasoning — why a
code is refused, why a target is the one it is — that reasoning was written here.
No upstream conversion factor is copied either: every target resolves through
Bovnar's own registry, which `check_profile_factors.py` then compares against the
publisher's value rather than importing it.

**Not redistributed:** no upstream artefact ships with this project, and none is
tracked by git. The publishers' machine-readable definitions are fetched by
`check_profile_factors.py --fetch` into `build/vocab/`, which is git-ignored,
used to verify the tables against their sources, and never packaged. A release
tarball, a wheel, a `.wasm` and the amalgamation contain none of them.

**Adapted, in every case.** No table here reproduces a vocabulary. Each one
extracts that vocabulary's identifiers, maps them onto Bovnar's own unit
registry, and omits every code whose value this project cannot state exactly —
`doc/11_bovnar_unit_profiles.md` §6.4 and §18 record which, and why. Where the
licence below requires a modification notice, this paragraph is it.

**On "verified against".** Each entry below names the published version the table
is currently proved against by `check_profile_factors.py`, and the date that
version was retrieved into the verification cache. It is a statement about what
the rows demonstrably agree with today — which is the checkable claim, and the
one that matters for attribution.

---

## UN/ECE — Recommendations 20 and 21

| | |
|---|---|
| **Used in** | `src/gendata/unece.bvnr`, the `unece:` profile |
| **Vocabulary** | Recommendation No. 20, *Codes for Units of Measure Used in International Trade*, and Recommendation No. 21, *Codes for Passengers, Types of Cargo, Packages and Packaging Materials* |
| **Verified against** | QUDT 3.1.0's `qudt:uneceCommonCode` cross-reference — see below |
| **Home** | <https://unece.org/trade/uncefact/cl-recommendations> |
| **Extracted** | UN/ECE common codes, and the conventional name of a unit in a trailing comment on most rows |

> Contains UN/ECE common codes from UN/CEFACT Recommendations 20 and 21,
> published by the United Nations Economic Commission for Europe. Neither UNECE
> nor UN/CEFACT endorses this project or its use of the codes.

**Verified at one remove, through QUDT.** Recommendation 20 states its conversion
factors in prose, so there is no machine-readable publication to resolve against.
Every row of `unece.bvnr` is therefore proved against QUDT's published
`uneceCommonCode` cross-reference rather than against a UNECE artefact
(`check_profile_factors.py`, class `Unece`), and that cross-reference — not this
project's reading of a code list — is what decided the table's contents when it
was closed: `git log src/gendata/unece.bvnr` records rows being retargeted, and
others left out, on the strength of QUDT's label for the unit carrying the code.

Nothing in this repository reads or ships a UNECE publication; `check_profile_factors.py`
lists no UNECE source, and no UNECE file is tracked by git. What this project
cannot state is where each code was *first* encountered by its author over the
months the table was written by hand, which is why nothing here claims a UNECE
document was never consulted. What is UNECE's in the result is the codes.

**Status: formally unresolved, practically low risk.** The generic United
Nations website terms permit only personal, non-commercial download, without a
right to redistribute or to create derivative works. Those terms plainly are not
aimed at the code lists: Rec 20 exists to be implemented, and is implemented in
UN/EDIFACT, UBL, Peppol/EN 16931, ISO 20022, OPC UA and QUDT among many others.
The operative instrument is the UN/CEFACT Intellectual Property Rights Policy,
under which UN/CEFACT deliverables are made available for implementation
royalty-free.

**Open action:** obtain written confirmation of the reuse terms from
<cefact@unece.org>, and record it here.

---

## QUDT — Quantities, Units, Dimensions and Data Types

| | |
|---|---|
| **Used in** | `src/gendata/qudt.bvnr`, `src/gendata/qudt-qk.bvnr`, `src/gendata/unece.bvnr` |
| **Verified against** | QUDT 3.1.0 (`owl:versionIRI <http://qudt.org/3.1.0/vocab/unit>`), retrieved 2026-07-30 |
| **Source** | <https://qudt.org/3.1.0/vocab/unit> and <https://qudt.org/3.1.0/vocab/quantitykind> |
| **Home** | <https://qudt.org/> |
| **Extracted** | unit local names, quantity-kind local names, and the `qudt:uneceCommonCode` cross-reference |

> Contains information from the QUDT Ontologies, which is made available under
> the Creative Commons Attribution 4.0 International License (CC BY 4.0),
> <https://creativecommons.org/licenses/by/4.0/>. Attribution is made to
> QUDT.org.
>
> **Modified.** Unit and quantity-kind local names were extracted and mapped
> onto Bovnar's native unit registry; names whose value this project cannot
> state exactly were omitted. QUDT.org does not endorse this project or its use
> of the material.

CC BY 4.0 section 2(a)(1) licenses the sui generis database rights as well as the
copyright, so the extraction is covered whichever right applies. There is no
ShareAlike term: the attribution above is the whole of the obligation, and it
imposes nothing on this project's own MIT-licensed code.

---

## UDUNITS-2 — Unidata units library

| | |
|---|---|
| **Used in** | `src/gendata/udunits.bvnr` (the `udunits:` profile), and `src/gendata/cf.bvnr` indirectly — CF states its `canonical_units` in UDUNITS syntax |
| **Verified against** | `master` branch (`udunits2-base.xml`, `-derived.xml`, `-accepted.xml`, `-common.xml`), retrieved 2026-07-30 |
| **Source** | <https://github.com/Unidata/UDUNITS-2> |
| **Home** | <https://www.unidata.ucar.edu/software/udunits/> |
| **Extracted** | unit symbols, singular and plural spellings, and prefix spellings |

The UDUNITS-2 licence requires this notice to be reproduced in source
distributions and in the documentation accompanying binary distributions. It is
reproduced here in full:

> Copyright 2025 University Corporation for Atmospheric Research and contributors.
> All rights reserved.
>
> This software was developed by the Unidata Program Center of the
> University Corporation for Atmospheric Research (UCAR)
> <http://www.unidata.ucar.edu>.
>
> Redistribution and use in source and binary forms, with or without modification,
> are permitted provided that the following conditions are met:
>
>    1) Redistributions of source code must retain the above copyright notice,
>       this list of conditions and the following disclaimer.
>    2) Redistributions in binary form must reproduce the above copyright notice,
>       this list of conditions and the following disclaimer in the documentation
>       and/or other materials provided with the distribution.
>    3) Neither the names of the development group, the copyright holders, nor the
>       names of contributors may be used to endorse or promote products derived
>       from this software without specific prior written permission.
>    4) This license shall terminate automatically and you may no longer exercise
>       any of the rights granted to you by this license as of the date you
>       commence an action, including a cross-claim or counterclaim, against
>       the copyright holders or any contributor alleging that this software
>       infringes a patent. This termination provision shall not apply for an
>       action alleging patent infringement by combinations of this software with
>       other software or hardware.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
> FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE CONTRIBUTORS
> OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
> WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
> CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS WITH THE SOFTWARE.

Clause 3 is why nothing in this project claims Unidata's or UCAR's endorsement.

---

## OM 2 — Ontology of units of Measure

| | |
|---|---|
| **Used in** | `src/gendata/om.bvnr`, the `om:` profile |
| **Verified against** | OM 2.0 (`om-2.0.rdf`), retrieved 2026-07-30 |
| **Source** | <https://github.com/HajoRijgersberg/OM> |
| **Home** | <http://www.ontology-of-units-of-measure.org/> |
| **Extracted** | unit local names, and the unit compositions (prefix/base, numerator/denominator, term/term, base/exponent) from which each translation target was derived |

> (c) 2005-2025 Hajo Rijgersberg, Don Willems, Jan Top, Wageningen University
> and Research Centre, The Netherlands. Used under the Creative Commons
> Attribution 4.0 International License (CC BY 4.0),
> <https://creativecommons.org/licenses/by/4.0/>.
>
> **Modified.** Local names were extracted and their translation targets built
> from OM's own stated composition; units whose value this project cannot state
> exactly were omitted. The authors do not endorse this project or its use of
> the material.

OM is published by an EEA maker, so the EU sui generis database right
(sections 87a ff. UrhG) is the relevant right alongside copyright. CC BY 4.0
section 2(a)(1)
licenses it expressly, so the extraction is covered.

---

## Other reference data

Two further external sources are used, neither of them a unit vocabulary and
neither raising the questions above:

* **ISO 4217** currency codes, numeric codes and minor-unit digits, in
  `src/gendata/currencies.bvnr`. The three-letter code list is published by the
  ISO 4217 Maintenance Agency (SIX Group) for free use, and codes and numeric
  identifiers are facts rather than expression. Cryptocurrency tickers are not
  ISO 4217 and are conventional.
* **CODATA 2022** for the one measured constant in `src/gendata/units.bvnr` —
  the atomic mass constant behind `Da`/`u`/`amu`. CODATA recommended values are
  published for unrestricted use; the edition is named in that file's header
  because a value without its edition cannot be checked. Every other factor in
  the registry is exact by definition or by convention.

---

# Notice copied from upstream (Part 2)

## Date algorithms — Howard Hinnant

| | |
|---|---|
| **Affects** | `include/bvn_gregorian_date.h`, `src/utils/bvn_gregorian_date.c` |
| **Status** | Donated to the public domain by the author |

> The civil-date ↔ day-number conversions are derived from Howard Hinnant's
> *chrono-Compatible Low-Level Date Algorithms*,
> <https://howardhinnant.github.io/date_algorithms.html>, of which the author
> writes: "Consider these donated to the public domain."

This is the only algorithm in the C sources derived from a published
implementation rather than from a specification or from first principles, and
`bvn_gregorian_date.h` has credited it in its header since it was written.
There is no obligation attached; it is recorded here because a notices file that
lists a webfont and omits an algorithm is not telling the whole story.

The claim that it is the *only* one is bounded, not absolute: it rests on a
sweep of every source comment in `src/`, `include/` and `python/bovnar/` for
external URLs and for the phrases a derivation is normally marked with, which
found this and nothing else.

