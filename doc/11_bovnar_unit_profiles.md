# Bovnar — Unit Profiles

> **Spec version:** 1.2
> **Status:** Normative — the profile notation and the transliteration tables the parser translates through. The code is in `src/utils/bovnar_profiles.c` and pinned by `tests/bovnar_ucum_test.c`, `tests/bovnar_unece_test.c`, `tests/bovnar_qudt_test.c`, `tests/bovnar_udunits_test.c`, `tests/bovnar_om_test.c`, `tests/bovnar_cf_test.c`, `tests/bovnar_crossvocab_test.c` and `tests/bovnar_unit_source_test.c`. Section 10.4 lists the one part of this document that was not built, and §18.3 the licensing questions that are still open.
> **Scope:** How a foreign vocabulary's code may be written in the unit slot beside Bovnar's native notation, what it translates to, what it refuses, and what the format still guarantees once seven of them are admitted.

**Seven namespaces are defined.** Sections 1–10 specify the profile MECHANISM and, as its worked
example, the `ucum:` profile in full. Sections 11–13 specify the next four, §14 the suite that holds
them to each other and §15 the pass that closed their tables; §§16–17 specify the two vocabularies
admitted after that pass, which is why they sit after it rather than beside their siblings:

| Namespace | Vocabulary | Grammar | Codes carried | Section |
|---|---|---|---|---|
| `ucum:` | UCUM — Unified Code for Units of Measure | expression | 206 mapped + 41 arbitrary, **all 312 UCUM defines** | [2](#2-syntax)–[10](#10-cost-risk-and-what-is-left-out) |
| `unece:` | UN/ECE Recommendation 20 and 21 | flat | 1302 + 25 opaque | [11](#11-the-unece-profile) |
| `qudt:` | QUDT unit local names | flat | 2244, **all 2803 QUDT defines** | [12](#12-the-qudt-profiles) |
| `qudt-qk:` | QUDT quantity kinds | flat | 908, **all 1164 QUDT defines** | [12.3](#123-quantity-kinds-qudt-qk) |
| `udunits:` | UDUNITS-2, the CF/netCDF units syntax | expression | 768 (its regular plurals included), **all 580 UDUNITS defines** | [13](#13-the-udunits-profile) |
| `om:` | OM 2 — Ontology of units of Measure | flat | 1336 + 124 refused, **every unit individual OM states** | [16](#16-the-om-2-profile) |
| `cf:` | CF standard names | flat, read-only | 4454 + 617 refused, **all 5071 names of table v94** | [17](#17-the-cf-standard-name-profile) |

Companion to [Unit & Currency Reference](05_bovnar_unit_system.md) (the native registry and notation
grammar these profiles sit beside), [Unit Ambiguities](07_bovnar_unit_ambiguities.md) (how a unit token is
resolved, and the pairs that look interchangeable), and [Unit Policy](06_bovnar_unit_policy.md) (the
reader- and writer-side unit policies a profile unit has to survive unchanged).

**The four enumerable tables are closed against their publishers.** Every atom `ucum-essence.xml`
defines, every spelling the UDUNITS-2 database defines, and every local name and quantity kind QUDT
defines is now in one of the three lists — mapped, opaque, or refused with a reason. That was not
true before: 83 UCUM atoms, 299 UDUNITS spellings and some 3600 QUDT names came back as
`error_unit_illegal`, which tells a conforming producer their code is not a code of the vocabulary
they wrote it in. `unece` cannot be closed the same way and §11.1 says why. §15 records what the
pass changed and what it cost.

Every acceptance, refusal and conversion factor quoted below was produced by running the
reference implementation built from this tree, and the behavioural claims are pinned by the test
files named above (1335 assertions across the seven profiles, plus 4789 in the cross-vocabulary
suite). The VOCABULARY side — whether each foreign code is *worth* what the table says — is checked
too, but not by the same means and not to the same strength: `check_profile_factors.py` (§9.5)
resolves every mapped code against its publisher's own machine-readable definitions, and `unece`
alone is reached at one remove, through QUDT's cross-reference, because Rec 20 states its factors in
prose. §9.2 says what the generator proves on its own, §9.5 says what the outside check proves and
where its tolerance runs out, §10.2 records what is still uncovered, and §14 is explicit that five
tables wrong in the same way would agree with each other perfectly.

---

## Table of Contents

1. [Overview](#1-overview)
    - 1.1 [What a profile is](#11-what-a-profile-is)
    - 1.2 [Why a notation rather than more native units](#12-why-a-notation-rather-than-more-native-units)
    - 1.3 [The one thing that must not change](#13-the-one-thing-that-must-not-change)
2. [Syntax](#2-syntax)
    - 2.1 [The namespace discriminator](#21-the-namespace-discriminator)
    - 2.2 [Where a profile unit may appear, and in which documents](#22-where-a-profile-unit-may-appear-and-in-which-documents)
    - 2.3 [Five bytes the lexer has to learn](#23-five-bytes-the-lexer-has-to-learn)
    - 2.4 [Commas inside an annotation](#24-commas-inside-an-annotation)
    - 2.5 [Length budget](#25-length-budget)
    - 2.6 [Grammar](#26-grammar)
3. [Translation](#3-translation)
    - 3.1 [Three outcomes, and no fourth](#31-three-outcomes-and-no-fourth)
    - 3.2 [Atoms and prefixes](#32-atoms-and-prefixes)
    - 3.3 [Operators, exponents, grouping](#33-operators-exponents-grouping)
    - 3.4 [Annotations](#34-annotations)
    - 3.5 [Scale factors and the decade fold](#35-scale-factors-and-the-decade-fold)
    - 3.6 [Arbitrary units](#36-arbitrary-units)
    - 3.7 [Special units](#37-special-units)
    - 3.8 [Affine units](#38-affine-units)
4. [Semantics after translation](#4-semantics-after-translation)
    - 4.1 [Equality](#41-equality)
    - 4.2 [Compatibility and conversion](#42-compatibility-and-conversion)
    - 4.3 [Policy, normalisation, and the writer](#43-policy-normalisation-and-the-writer)
    - 4.4 [Invariants the profile preserves](#44-invariants-the-profile-preserves)
5. [Serialisation](#5-serialisation)
    - 5.1 [Canonical output](#51-canonical-output)
    - 5.2 [What canonical output loses, and on which path](#52-what-canonical-output-loses-and-on-which-path)
    - 5.3 [Emitting a profile code from a native unit](#53-emitting-a-profile-code-from-a-native-unit)
6. [The transliteration table](#6-the-transliteration-table)
    - 6.1 [Verified mappings](#61-verified-mappings)
    - 6.2 [Collisions — the same spelling, a different unit](#62-collisions--the-same-spelling-a-different-unit)
    - 6.3 [Traps that are not spelling collisions](#63-traps-that-are-not-spelling-collisions)
    - 6.4 [Codes with no Bovnar representation](#64-codes-with-no-bovnar-representation)
7. [Data model](#7-data-model)
    - 7.1 [The opaque units](#71-the-opaque-units)
    - 7.2 [Incommensurability, via the mechanism currencies already use](#72-incommensurability-via-the-mechanism-currencies-already-use)
    - 7.3 [The spelling on the data event](#73-the-spelling-on-the-data-event)
    - 7.4 [New error codes](#74-new-error-codes)
8. [API](#8-api)
    - 8.1 [C](#81-c)
    - 8.2 [Python](#82-python)
    - 8.3 [CLI](#83-cli)
9. [Build and conformance](#9-build-and-conformance)
    - 9.1 [Where the tables live](#91-where-the-tables-live)
    - 9.2 [What the generator checks, and what it does not](#92-what-the-generator-checks-and-what-it-does-not)
    - 9.3 [Tests](#93-tests)
    - 9.4 [One build switch per vocabulary](#94-one-build-switch-per-vocabulary)
    - 9.5 [The factor proof](#95-the-factor-proof)
    - 9.6 [Synchronisation between the tables](#96-synchronisation-between-the-tables)
10. [Cost, risk, and what is left out](#10-cost-risk-and-what-is-left-out)
    - 10.1 [What it cost](#101-what-it-cost)
    - 10.2 [What can go wrong](#102-what-can-go-wrong)
    - 10.3 [Deliberately not attempted](#103-deliberately-not-attempted)
    - 10.4 [Specified here but not built](#104-specified-here-but-not-built)
11. [The UNECE profile](#11-the-unece-profile)
    - 11.1 [Why this vocabulary](#111-why-this-vocabulary)
    - 11.2 [Flat, and why that is not a simplification](#112-flat-and-why-that-is-not-a-simplification)
    - 11.3 [Rec 21 packages, and the Rec 20 counts, as opaque units](#113-rec-21-packages-and-the-rec-20-counts-as-opaque-units)
    - 11.4 [A reading of the table](#114-a-reading-of-the-table)
12. [The QUDT profiles](#12-the-qudt-profiles)
    - 12.1 [Why this vocabulary](#121-why-this-vocabulary)
    - 12.2 [Local names, not IRIs](#122-local-names-not-iris)
    - 12.3 [Quantity kinds (`qudt-qk:`)](#123-quantity-kinds-qudt-qk)
    - 12.4 [The quantity-kind table: the ISO 80000 core](#124-the-quantity-kind-table-the-iso-80000-core)
13. [The UDUNITS profile](#13-the-udunits-profile)
    - 13.1 [An expression profile, sharing the UCUM parser](#131-an-expression-profile-sharing-the-ucum-parser)
    - 13.2 [Space multiplies, and now it can](#132-space-multiplies-and-now-it-can)
    - 13.3 [The near misses: codes that name a native unit and are not it](#133-the-near-misses-codes-that-name-a-native-unit-and-are-not-it)
    - 13.4 [Reference time is refused, and why](#134-reference-time-is-refused-and-why)
14. [The cross-vocabulary conformance suite](#14-the-cross-vocabulary-conformance-suite)
    - 14.1 [A concept table, checked pairwise](#141-a-concept-table-checked-pairwise)
    - 14.2 [The negative half is not optional](#142-the-negative-half-is-not-optional)
    - 14.3 [What it found, and what it cannot tell you](#143-what-it-found-and-what-it-cannot-tell-you)
15. [Closing the tables](#15-closing-the-tables)
    - 15.1 [What was wrong](#151-what-was-wrong)
    - 15.2 [How the rows were produced](#152-how-the-rows-were-produced)
    - 15.3 [What it cost, and what is still open](#153-what-it-cost-and-what-is-still-open)
16. [The OM 2 profile](#16-the-om-2-profile)
    - 16.1 [Why this vocabulary](#161-why-this-vocabulary)
    - 16.2 [The targets were derived from OM's own structure](#162-the-targets-were-derived-from-oms-own-structure)
    - 16.3 [What the derivation refuses](#163-what-the-derivation-refuses)
    - 16.4 [Which name a unit is written back as](#164-which-name-a-unit-is-written-back-as)
17. [The CF standard-name profile](#17-the-cf-standard-name-profile)
    - 17.1 [Why this vocabulary](#171-why-this-vocabulary)
    - 17.2 [The unit is CF's own `canonical_units`](#172-the-unit-is-cfs-own-canonical_units)
    - 17.3 [Read-only, and why a namespace may be](#173-read-only-and-why-a-namespace-may-be)
    - 17.4 [What is absent, and what it costs](#174-what-is-absent-and-what-it-costs)
18. [Provenance, licensing and attribution](#18-provenance-licensing-and-attribution)
    - 18.1 [What is taken from whom](#181-what-is-taken-from-whom)
    - 18.2 [Every table is an adaptation, and says so](#182-every-table-is-an-adaptation-and-says-so)
    - 18.3 [The two open questions](#183-the-two-open-questions)
- [See also](#see-also)

---

## 1. Overview

### 1.1 What a profile is

A **unit profile** is an alternative *spelling* for the unit slot of a type annotation. It is not a
second unit model. A profile expression is translated, at parse time, into exactly the same
`value_unit_t` a native expression produces, and from that point on nothing downstream can tell
which notation the document used:

```bovnar
#!bovnar 1.2
.systolic_a = <float_dec:64,ucum:mm[Hg]> 120.00;   # profile spelling
.systolic_b = <float_dec:64,mmHg>        120.00;   # native spelling — same value_unit_t
```

`bvn_unit_equal` reports those two units equal. `bvn_units_compatible`, `bvn_unit_convert_factor`,
the reader and writer unit policies, `bvnr_normalise_si`, the DOM and the
`want_unit` hook all keep working with no knowledge that a profile exists. That is the whole design
constraint, and §3.1 exists to hold it: a profile expression either becomes a real
`value_unit_t` or it becomes an error. There is no third state in which a value carries a unit the
rest of the library cannot reason about.

The general form is `namespace:code`, and seven namespaces are defined: `ucum`, `unece`, `qudt`,
`qudt-qk`, `udunits`, `om` and `cf`. An unknown namespace is an error (`error_unit_profile_unknown`), never a
passthrough — a consumer reads that code as "this build has no such profile", which is a different
problem from "that is not a unit".

Everything sections 2–10 say about `ucum:` describes the shared mechanism, because every profile
runs through the same translator, the same three outcomes and the same tables. What differs between
them is exactly two things:

- **The grammar.** An *expression* profile (`ucum`, `udunits`) writes a code as an expression over
  prefixed atoms, with operators, exponents and grouping. A *flat* profile (`unece`, `qudt`,
  `qudt-qk`, `om`, `cf`) has no operators and no prefixes at all: the whole code is one token, matched entire.
  That is not a simplification but a correctness requirement — UNECE's `KGM` is the kilogram, and a
  parser that decomposed it would find a `k` prefix on a `GM` that UNECE never defined, just as it
  would read `MTS` as a mega-`TS` and QUDT's `MI` (the mile) as a milli-anything.
- **The tables**, one set per namespace, generated from one data file per namespace by
  `gen_profiles.py`.

Two smaller switches ride on the registry row rather than on the grammar: which extra bytes multiply
and introduce an exponent (§13.1), and whether the vocabulary has UCUM's `{…}` annotations at all —
only `ucum` does, so `udunits:mL{x}` is `error_unit_illegal` rather than an annotated millilitre
(§3.4).

Adding a vocabulary is therefore a data file and a registry row, not a second parser.

### 1.2 Why a notation rather than more native units

Bovnar's native registry is 264 physical units and 216 currencies, hand-maintained in
`src/gendata/`. UCUM's atom table is larger — a complete clinical, apothecary, troy, avoirdupois
and CGS inventory — and its expression language is unbounded, so the set of valid UCUM codes cannot
be enumerated as a table of units at all.

Absorbing that into the native registry would mean adding several hundred aliases, most of them
spellings no Bovnar document would ever choose, and several of which collide with existing native
symbols (§6.2). A notation costs one table of mappings instead, keeps the two disambiguation
regimes apart, and lets a document produced by a UCUM-speaking system be read without the native
registry growing at all.

### 1.3 The one thing that must not change

The obvious way to get this wrong is to make `ucum:` a hole through which any string reaches a
value uninspected. Then `<float:64,ucum:metre>` parses, the typo survives to the consumer, and a
unit has arrived that the library cannot reason about — no dimension, no compatibility check, no
conversion. Every guarantee the unit system provides is a guarantee about units it understands.

So the rule in §3.1 is absolute, and every later section is written to keep it: a `ucum:`
expression is parsed as UCUM in full, against UCUM's own atom table, and anything that is not a
valid UCUM expression over known atoms is `error_unit_illegal` before translation is even
attempted. Passthrough exists (§3.6) but reaches only atoms UCUM itself defines and classifies as
arbitrary — assay quantities commensurable with nothing. It is never a fallback for something
unrecognised.

---

## 2. Syntax

### 2.1 The namespace discriminator

A profile unit is a namespace name, an ASCII colon, and a code:

```
ucum:mm[Hg]
ucum:10*3/uL
ucum:mL{total}
unece:KGM
qudt-qk:Mass
```

A **namespace name** is lowercase letters and digits, and may contain `-` — but not as its first
byte. The hyphen exists for `qudt-qk`, where one publisher's vocabulary is split into a unit
namespace and a quantity-kind namespace that must not be confused with it (§12.3). Because it may
not lead, `-qk:Mass` is not a namespace at all: it falls through to the native parser, which rejects
it exactly as it did before profiles existed.

```
$ bovnar validate t.bvnr        # .a = <float:64,-qk:Mass> 1.0;
Validation failed: unit_illegal at line 2, col 24
```

The colon is already an accepted byte inside a type-annotation body (state table
`copy_type_byte`, `[0x3a]`), and no native unit alias or currency code contains one, so the
discriminator is unambiguous against the entire existing registry with no lookahead.

It was also already an *error* before the profiles existed, which is what made the extension safe.
A `:` in a unit slot was `error_unit_illegal` then, and still is wherever the profile machinery
declines it as not a unit — in a document that has not opted in (§2.2), and in one that has, for a
code the named namespace does not define:

```
$ bovnar validate t.bvnr        # #!bovnar 1.1 / .a = <float:64,ucum:mmHg> 1.0;
Validation failed: unit_illegal at line 2, col 25
```

No document that parsed before spec 1.2 contained a `ucum:` unit, so no document changed meaning
when one started to parse. Contrast the alternative of a quoted form (`ucum:"mm[Hg]"`), which would put a
string-escape sub-language inside a type body for no gain.

The three refusals are distinguishable once a document *has* opted in, which is the whole point of
the split in §3.1:

```
$ bovnar validate t.bvnr        # #!bovnar 1.2 / .a = <float:64,zz:m> 1.0;
Validation failed: unit_profile_unknown at line 2, col 20

$ bovnar validate t.bvnr        # #!bovnar 1.2 / .a = <float:64,ucum:B[SPL]> 1.0;
Validation failed: unit_profile_unsupported at line 2, col 27

$ bovnar validate t.bvnr        # #!bovnar 1.2 / .a = <float:64,ucum:mmHg> 1.0;
Validation failed: unit_illegal at line 2, col 25
```

The last is worth reading twice: `mmHg` is a perfectly good *native* spelling and not a UCUM atom at
all — UCUM writes the millimetre of mercury `mm[Hg]`. Inside a namespace, only that namespace's
table is consulted.

### 2.2 Where a profile unit may appear, and in which documents

**The notation is gated on the declared spec version.** A profile unit needs a `#!bovnar` directive
declaring 1.2 or later — `#!bovnar 1.2` — exactly as the datetime family and the `\x`/`\u`
escapes need a declared 1.1. Without one it is `error_unit_illegal` — in a 1.1 document `ucum:mm[Hg]` is simply not a unit, the same way
`<datetime:64>` is simply not a value type in a 1.0 document. A document with **no** directive
declares nothing and therefore gets neither surface.

```bovnar
#!bovnar 1.2
.systolic = <float_dec:64,ucum:mm[Hg]> 120.00;   # OK
```

The gate is applied to the type-annotation unit parameter and to the inline unit suffix alike: an
inline unit is the same unit slot reached by another route, and gating one and not the other would
leave a hole exactly one comma wide.

Without the gate a document could carry a unit that every conforming reader of its own declared
version must reject, which is the interoperability hazard the directive exists to prevent. Native
units are unaffected in every version: the bump is additive, and `<float:64,mmHg>` parses under 1.0
as it always did.

The writer enforces the other half. A unit with no native spelling — one carrying an opaque base
unit (§7.1) — can only be emitted in this notation, so writing one without having emitted the opt-in
directive is `error_unsupported_spec_version` rather than a document the library cannot read back. A
*translated* unit needs no such guard when it is written from the unit alone: `ucum:mm[Hg]` parses
to `mmHg`, and `mmHg` is what `bvn_unit_to_string` gives back, which every version accepts. The
guard returns with the *spelling*: a writer handed `ucum:mm[Hg]` as the text to write (§5.2) is
writing profile notation, and needs the directive for it exactly as an opaque unit does.

Otherwise: everywhere a native unit may appear — as the unit parameter of a type annotation, and as
an inline unit suffix. Parameter ordering stays free (doc/05 §2.1) and the annotation/inline agreement rule
(doc/05 §2.2) is unchanged — the comparison is on the parsed `value_unit_t`, so the two spellings
may differ as long as they mean the same thing:

<!-- bovnar-example: rejected -->
```bovnar
#!bovnar 1.2
.a = <float:64,ucum:mm[Hg]> 120.0;
.b = 120.0 ucum:mm[Hg];
.c = <float:64,mmHg> 120.0 ucum:mm[Hg];   # OK — both parse to the same unit
.d = <float:64,m> 1.0 ucum:s;             # error_unit_mismatch, as always
```

A profile unit is a *unit*, so it is confined to the same type families (doc/05 §2.3): `uint`,
`sint`, `float`, `float_fix`, `float_dec`. A `ucum:` parameter on `utf8`, `bool` or `datetime` is
`error_illegal_value_type`, unchanged.

Currencies stay native-only. None of the seven vocabularies yields a monetary unit — QUDT's `USD`, OM's `om:euro` and
`EUR` are refused by name (§12.2) rather than mapped — and the `$` sigil rule (doc/05 §9.1) is
untouched.

### 2.3 Five bytes the lexer has to learn

UCUM needs `[`, `]`, `{`, `}` and `'` — the last for codes like `[arb'U]` and `[Amb'a'1'U]`. All
five were rejected by the type-body and inline-unit byte classes:

```
$ bovnar validate t.bvnr        # .a = <float:64,ucum:mm[Hg]> 1.0;   (before this change)
Validation failed: unexpected_input_byte at line 2, col 18
```

The change is **15 state-table entries** — five bytes in each of three states of
`src/lexer/bovnar_state_table.c`:

| Byte | Char | States extended |
|------|------|-----------------|
| 0x27 | `'`  | `copy_type_byte`, `type_body_outro`, `inline_unit_body` |
| 0x5B | `[`  | `copy_type_byte`, `type_body_outro`, `inline_unit_body` |
| 0x5D | `]`  | `copy_type_byte`, `type_body_outro`, `inline_unit_body` |
| 0x7B | `{`  | `copy_type_byte`, `type_body_outro`, `inline_unit_body` |
| 0x7D | `}`  | `copy_type_byte`, `type_body_outro`, `inline_unit_body` |

`type_body_outro` is extended alongside `copy_type_byte` because the two already carry an identical
unit-byte class; letting them diverge here would be the odd choice, and anything nonsensical that
reaches the type-spec parser through it is still `error_illegal_value_type`.

This is the only change to the unit *byte class* the profile requires — whitespace inside a
namespaced annotation parameter is the other lexer change, and has its own section (§13.2) — and it
is the one place where the profile is
*not* invisible to a native-only document: after it, `<float:64,m[s]>` reaches `bvn_parse_unit`
and fails there (`error_unit_illegal`) instead of failing in the lexer
(`error_unexpected_input_byte`). One error code moves for a family of inputs that were errors
before and are errors after. §9.3 pins that in the conformance corpus rather than leaving it to be
discovered.

Nothing else about the byte class changes. `<`, `>`, `"`, `;` and `#` stay out, so a brace cannot
swallow the rest of the document; whitespace stays out of an inline suffix, and inside an annotation
is admitted only within a parameter that carries a namespace (§13.2). Two consequences, and they land on different sides of
the lexer boundary:

```
$ bovnar validate t.bvnr        # .a = <float:64,ucum:mL{a;b}> 1.0;
Validation failed: unexpected_input_byte at line 2, col 25

$ bovnar validate t.bvnr        # .a = <float:64,ucum:mL{total> 1.0;
Validation failed: unit_illegal at line 2, col 29
```

That fences off one thing UCUM permits. UCUM allows any printable ASCII except braces inside an
annotation; this profile allows only the bytes a type body accepts, because `;` and `#` end a value
and `<` and `>` delimit a type annotation — so `;` is refused by the *lexer*, at the byte, while an
unclosed `{` simply runs to the value terminator and ends as a malformed unit. The alternative —
admitting a byte that terminates the construct it sits inside — is not a trade worth making for an
annotation the unit model ignores anyway.

### 2.4 Commas inside an annotation

A type-parameter list is comma-separated, and a UCUM annotation may contain a comma
(`{cells,total}`). The parameter scanner in `bvn_parse_type_annotation` therefore tracks brace depth: a
`,` at brace and bracket depth 0 ends the parameter, a `,` at either depth ≥ 1 is part of it.
Bracket depth needs no such treatment — `[` … `]` cannot contain a comma in any UCUM atom — but is
tracked anyway so that an unbalanced bracket is diagnosed as a malformed unit rather than as a
malformed parameter list.

```bovnar
#!bovnar 1.2
.a = <float:64,ucum:mL{cells,total}> 1.0;   # one unit parameter, not two
```

Depth is bounded by `BVN_UNIT_GROUP_MAX_DEPTH` (16), the same bound the native parenthesis parser
uses. UCUM annotations do not nest — `{` inside an annotation is not legal UCUM — so any depth
above 1 is already an error; the bound exists so that the *scanner* cannot be driven to recurse by
a hostile document before the parser gets to say so.

### 2.5 Length budget

Three caps sit in front of the profile parser, all of them 255 bytes, all of them pre-existing, and
which one fires decides which error a producer sees:

| Cap | Where | Overflow |
|---|---|---|
| the whole type-annotation body | `BVN_TYPE_BUF_CAP − 1`, in the lexer | `error_type_too_long` |
| one inline unit suffix | `BVN_INLINE_UNIT_BUF_CAP − 1`, in the lexer | `error_unit_too_long` |
| one unit *parameter* | `UINT8_MAX`, in `bvn_parse_type_annotation` | `error_unit_too_long` |

An annotated unit therefore reaches `error_type_too_long` first — the body cap counts the family
name, the width and the commas as well, so a unit parameter can never be the only thing over the
line. An inline unit has a buffer to itself and reaches `error_unit_too_long`:

```
$ bovnar validate t.bvnr        # .a = 1.0 ucum:mL{xxx…260 bytes…};
Validation failed: unit_too_long at line 2, col 265
```

UCUM codes with long annotations can exceed 255 — `mL/min/{1.73_m2}` does not, but a genuinely
chatty annotation will. No new limit and no new error: the profile inherits these, and a producer
that needs more than 255 bytes of unit is telling you the annotation is a field, not a unit.

### 2.6 Grammar

The formal rules live in [the EBNF](12_bovnar.ebnf) beside `unit-param`, which is
where the byte classes of §2.3 are also recorded:

```ebnf
unit-param   = profile-unit | native-unit ;
profile-unit = profile-name , ws , ":" , ws , profile-code ;
profile-name = name-head , {name-tail} ;         (* "ucum", "qudt-qk" *)
name-head    = lower-alpha ;                     (* a digit-led parameter is a width *)
name-tail    = lower-alpha | DIGIT | "-" ;
profile-code = profile-char , {profile-char} ;
profile-char = (* any unit-char, plus "," at brace or bracket depth >= 1 *) ;
```

`profile-code` is deliberately not given a grammar beyond its byte class. The sub-grammar is
**semantic**, as the native unit sub-grammar is (doc/05 §5.2): the lexer captures
bytes, and `bvn_parse_unit` — which dispatches on the namespace — decides whether
they are a valid code in that vocabulary. The normative grammar for what is
inside is each vocabulary's own, and restating them here would create a second
authority for each, to keep in step with the first.

---

## 3. Translation

### 3.1 Three outcomes, and no fourth

Every `ucum:` expression ends in exactly one of three states.

| Outcome | When | Result |
|---|---|---|
| **Translated** | Every atom maps onto the native registry, and the whole expression is representable | A normal `value_unit_t`; indistinguishable from the native spelling |
| **Profile-only** | The expression is valid UCUM and every atom is known, but one or more atoms are UCUM *arbitrary* units (§3.6) | A `value_unit_t` over UCUM's opaque units: comparable, never convertible |
| **Refused** | Anything else | An error code, at parse time, from the parser |

Refusal splits three ways by cause, because a producer needs to know which of these happened:

- not a valid UCUM expression, or an atom UCUM does not define → `error_unit_illegal` (32);
- valid UCUM, known atoms, no Bovnar representation (a special unit §3.7, an unrepresentable scale
  §3.5, more than `BVNR_MAX_UNIT_COMPONENTS` (32) components) → `error_unit_profile_unsupported` (50);
- the namespace before the colon is not a profile this build knows → `error_unit_profile_unknown` (49).

There is no "opaque string" outcome. A unit the library cannot reason about is a unit that has
escaped the enforcement point, and the format's only distinguishing claim is that units do not do
that.

### 3.2 Atoms and prefixes

UCUM separates prefix from atom by its own rule; Bovnar separates it by longest-alias-suffix match
with an optional explicit `~` (doc/05 §4.3). Translation happens **after** UCUM's split, on the
resolved (prefix, atom) pair, never by handing the raw UCUM string to the native parser. That is
what keeps the two disambiguation regimes from contaminating each other, and it is why the
collisions in §6.2 are harmless rather than fatal.

A code is matched as a whole atom first, so a complete atom always outranks a prefixed reading of
it — `mho` is the siemens, not milli-ho, and `cd` is the candela, not centi-day. Only if that fails
is the code split, and then **every** prefix that heads it is tried, longest first. Trying only the
longest is wrong: UCUM's prefixes are not suffix-free (`d` heads `da`), so `dar` — the deciare —
matches `da`, leaves `r`, and would be refused although it is legal UCUM. Verified:

```
ucum:ar    →  c~ha       1 in coherent SI = 100.0
ucum:dar   →  m~ha       1 in coherent SI = 10.0
ucum:har   →  ha         1 in coherent SI = 10000.0
```

A split whose atom is non-metric is not a match either, which is what keeps `k[arb'U]` an error
while the walk continues to a shorter prefix.

A UCUM prefix becomes the corresponding `si_prefix_id_t` on the translated component. Two things
can go wrong, and both are handled by the fold in §3.5 rather than by refusal:

- the native target does not accept prefixes at all (`prefix = none` or `ratio` in
  `src/gendata/units.bvnr` — `%`, `ppm`, `pH`, `PSU`, `mph`, the water-hardness degrees);
- the native target already carries a prefix because the mapping is not one-to-one
  (`kg` → `k~g`, `ar` → `c~ha`, `m[Hg]` → `k~mmHg`).

UCUM's binary prefixes (`Ki`, `Mi`, …) map to `prefix_iec` and are subject to the same
`bvn_prefix_unit_valid` check as natively — an IEC prefix on a byte is fine, on a metre it is not,
in both notations. UCUM's own atom table has no byte spelled `B`, so a binary-prefixed UCUM code
reaches that check through `By`.

### 3.3 Operators, exponents, grouping

| UCUM | Bovnar | Note |
|---|---|---|
| `.` | `·` | multiplication |
| `/` | `/` | division — see below |
| `m2`, `s-1` | `m²`, `s⁻¹` | exponent directly after the atom; range as natively (doc/05 §6) |
| `(` `)` | `(` `)` | grouping, mapped through the native group parser (doc/05 §5.2) |
| `1`, `10*0` | *(nothing)* | the unity atom; contributes no component, so `ucum:1` is `no_unit` |

The division rule is the one real difference and it must not be papered over. UCUM's `/` is a
binary operator evaluated left to right: `a/b/c` is `(a/b)/c` — and so is Bovnar's, whose latching
denominator gives `a·b⁻¹·c⁻¹`, the same thing. But UCUM's `/` applies only to the term that follows
it, whereas Bovnar's latches for the rest of the expression. `a/b.c` is `a·c/b` in UCUM and
`a·b⁻¹·c⁻¹` in Bovnar. Translation therefore works on UCUM's parse tree and emits *explicit signed
exponents*, never a `/`-bearing native string. The canonical form the formatter gives back is what
this table shows, and it is the honest witness — it re-parses to the same unit:

```
ucum:kg.m/s2      →  k~g·m/s²
ucum:kg/m.s2      →  k~g·s²/m         (kg·m⁻¹·s², NOT the kg·m⁻¹·s⁻² a latching read would give)
ucum:/min         →  min⁻¹
```

A leading `/` is UCUM's reciprocal form and needs the unity atom to be dropped: `/min` has no
numerator, and Bovnar has no way to write a bare `1` in a unit expression (a native `1` alone is
`error_unit_illegal`), so the translation is the negative exponent, verified:

```
ucum:/min    →  min⁻¹      1 in coherent SI = 0.016666666666666666
ucum:/uL     →  µ~L⁻¹      1 in coherent SI = 999999999.9999999
```

### 3.4 Annotations

A UCUM annotation `{…}` carries no meaning; UCUM defines `{anything}` standing alone as the unity.
The profile follows exactly, and states the consequence rather than hiding it:

```
ucum:mL{total}     →  m~L                       (equal to ucum:mL, and to native m~L)
ucum:{RBC}/uL      →  µ~L⁻¹
ucum:{cells}/uL    →  µ~L⁻¹                      (equal to the line above)
ucum:{RBC}         →  no_unit
```

`{RBC}/uL` and `{cells}/uL` compare equal because in UCUM they *are* equal. An annotation is a
comment, not a discriminator. If the distinction between red cells and cells matters to the
consumer, it is not a unit and does not belong in the unit slot — it belongs in a sibling field,
where Bovnar can type it and the reader can find it by key path.

The annotation text is preserved for round-trip (§5.2), and is preserved *verbatim*, including case
and spacing. It never affects equality, compatibility, conversion or normalisation.

**Annotations are UCUM's, and only UCUM's.** The registry row carries a flag, and it is false for
the other six namespaces, so `udunits:mL{x}` is `error_unit_illegal` rather than a millilitre with
a comment. Admitting the syntax everywhere would let a UDUNITS code that is simply wrong look like a
valid annotated one.

This is the valve. UCUM's own justification for annotations is that a code system which refuses the
expressiveness its users perceive gets adopted halfway, and halfway is as bad as not at all.
Bovnar has no such valve natively and gains one here — bounded to a notation where it is
semantically inert, rather than added to the native grammar where it would have to mean something.

### 3.5 Scale factors and the decade fold

UCUM writes powers of ten as `10*n` or `10^n` and uses them heavily in clinical counts
(`10*3/uL`). Bovnar has no component for a bare numeric factor. It does have prefixes, which are
exactly a decimal decade attached to a component — so a scale factor is discharged into a prefix.

Let translation produce components *c₁…cₙ* with exponents *eᵢ* and current prefix decades *pᵢ*,
plus a residual decade *D* accumulated from every `10*n`, every `10^n`, and every prefix that its
target refused to carry. Then:

> **Fold rule.** Choose the leftmost *i* such that *eᵢ* divides *D* exactly, and
> *p′ᵢ = pᵢ + D/eᵢ* is a decade with a defined SI prefix, and `bvn_prefix_unit_valid` accepts that
> prefix on *bᵢ*. Set *pᵢ ← p′ᵢ* and *D ← 0*. If no such *i* exists, the expression is
> `error_unit_profile_unsupported`.

The division by *eᵢ* is what makes it correct in a denominator, and getting it wrong is a
nine-orders-of-magnitude error rather than a cosmetic one. Verified against the implementation:

| UCUM | Fold | Bovnar | 1 unit in coherent SI |
|---|---|---|---|
| `10*3/uL` | *D* = +3, *e* = −1, *p* = −6 → *p′* = −9 | `n~L⁻¹` | `999999999999.9999` |
| `10*6/L`  | *D* = +6, *e* = −1, *p* = 0 → *p′* = −6  | `µ~L⁻¹` | `999999999.9999999` |
| `10*9/L`  | *D* = +9, *e* = −1, *p* = 0 → *p′* = −9  | `n~L⁻¹` | `999999999999.9999` |
| `10*12/L` | *D* = +12, *e* = −1, *p* = 0 → *p′* = −12 | `p~L⁻¹` | `999999999999999.9` |
| `10*3.m`  | *D* = +3, *e* = +1, *p* = 0 → *p′* = +3  | `k~m` | `1000.0` |
| `10*-3.g` | *D* = −3, *e* = +1, *p* = 0 → *p′* = −3  | `m~g` | `1e-06` |

(The last digits are the existing `bvni_ipow` rounding, not something the fold introduces; the
exact path is `bvn_unit_convert_rational`, doc/05 §12.4.)

**What the fold cannot do, and why that is stated rather than fixed.** SI prefix decades are
±1, ±2, ±3, and then multiples of three to ±30. There is no prefix for 10⁴, 10⁵, 10⁷ or 10⁸, so
`10*4/L` and `10*5.m` are `error_unit_profile_unsupported`. Two escapes were considered and both
rejected: synthesising a dimensionless factor component out of `%` and `ppm`
(`10*4` ≈ `%⁻²`) abuses the ratio units and produces a unit no human would recognise; and adding a
numeric-factor component to `value_unit_component_t` breaks the ABI for a case the clinical corpus
does not appear to need. A named refusal is better than either.

### 3.6 Arbitrary units

UCUM's arbitrary units — `[IU]`, `[arb'U]`, `[PFU]`, `[CFU]`, and the rest of that table — are
assay-defined. UCUM's own rule is that they are commensurable with nothing, including each other —
with one exception UCUM states itself: `[IU]` is defined as `1 [iU]`, the international unit in
two spellings. Bovnar carries that pair as **one** opaque unit: `ucum:[iU]` reads as `ucum:[IU]`,
the two compare equal, `[iU]/L` converts to `[IU]/mL` like any other volume change, and both are
written back as `ucum:[IU]`. They are also the single largest reason a real clinical code fails to
map (§6.4).

The profile admits all **41** of them, as a contiguous run at the bottom of UCUM's own block of the
`value_base_unit_t` id space (§7.1): **40** ids, one per UCUM arbitrary atom, with `[iU]` listed in
`.opaque_aliases` as a second spelling of `[IU]`'s id rather than an id of its own. An alias takes
its target's prefix rule, so `k[iU]` reads as `k[IU]`. Consequences, all of which fall out of the
existing machinery:

- they are **profile-only**: no native alias, reachable only through `ucum:`, and serialised back
  in profile notation (§5.1). Nothing about the native notation changes;
- they are **mutually incommensurable**, by the range predicate of §7.2 rather than by one quantity
  kind each;
- they take prefixes if and only if UCUM says so. Only `[IU]` and `[iU]` are metric in UCUM, so
  `k[IU]` translates while a prefix on `[arb'U]` is `error_unit_illegal` — UCUM's error, raised
  before translation;
- they never appear in an SI normal form, never produce a conversion factor, and
  `bvnr_normalise_si` leaves them alone, exactly as it already leaves currencies and the
  dimensionless kinds alone.

Note what is *not* in that list. UCUM's `U` is the **enzyme unit**, one micromole per minute, and it
is an ordinary mapped atom (`ucum:U` → `µ~mol/min`), not an arbitrary one. Only the bracketed
`[…'U]` spellings are assay-defined.

The critical thing this is **not**: a single shared "opaque" base unit. Collapsing `[IU]` and
`[PFU]` onto one id would make them compare equal, which is the silent-wrongness this format
exists to refuse. One id each is the only honest encoding, and the ids come from UCUM's table
rather than from anyone's judgement — including the one shared id, which is UCUM's own
`[IU]` = `1 [iU]` and the only alias `.opaque_aliases` admits.

### 3.7 Special units

UCUM classes as *special* the units defined by a function rather than a factor: the temperature
scales, the logarithmic ratios, the prism dioptre, the homeopathic potencies. Bovnar has native
forms for some, and those translate (§6.1): `Cel`, `[degF]`, `[degRe]`, `[degR]`, `[pH]`, `Np`, `B`,
`deg`, `gon`.

The rest are `error_unit_profile_unsupported`. The bel-with-a-reference family is the instructive
case: `B[SPL]`, `B[V]`, `B[W]`, `B[kW]`, `B[mV]`, `B[uV]` and `B[10.nV]` all differ from a plain bel
only in their reference level, and Bovnar's `dB` carries no reference. Translating them to `da~dB`
would discard exactly the information that distinguishes them and would let a sound-pressure level
compare equal to a voltage level. Refusing is the whole point.

`B` → `da~dB` is admitted because 1 bel *is* 10 decibels, exactly, in the logarithmic domain, and
`dB` carries its own quantity kind (`BVNI_KIND_LOG_DECIBEL`) so the result cannot drift into a
plain count. Verified: `da~dB` has factor `10.0`. It is a deliberate call and the one place in the
table where a prefix is used on a logarithmic unit.

### 3.8 Affine units

Nothing new. `Cel` translates to `°C`, and the native affine discipline (doc/05 §3, and the
`.affine`/`.offset` fields in `src/gendata/units.bvnr`) applies unchanged: an affine unit is valid
at exponent 1 only, and a compound containing one parses but yields no conversion value, because
the offset is a kelvin count and a product with signature `K·s⁻¹` has nowhere to put it.

`ucum:Cel/h` therefore behaves exactly as native `°C/h` does. The profile neither improves nor
worsens this, and it does not close the temperature-**difference** gap — UCUM has no delta scale
either, and writes a temperature difference as `K`. See §10.3.

---

## 4. Semantics after translation

### 4.1 Equality

`bvn_unit_equal` is unchanged: a multiset comparison of (base, exponent, prefix) triples. Because
translation produces ordinary components, a profile unit and a native unit that mean the same thing
compare equal, and the annotation/inline agreement check of doc/05 §2.2 works across notations:

```bovnar
#!bovnar 1.2
.v = <float:64,mmHg> 120.0 ucum:mm[Hg];      # OK
.w = <float:64,ucum:kg.m/s2> 1.0 k~g·m·s⁻²;  # OK
```

Two profile units compare through their translations, so `ucum:mL{a}` equals `ucum:mL{b}` equals
`ucum:mL`. §3.4 argues that is correct; §6.3 lists what it costs.

### 4.2 Compatibility and conversion

Unchanged, including the quantity-kind fences of `bvni_kinds_match`. A translated unit is subject
to every one of them:

- `ucum:bit` and `ucum:By` do not convert into each other, because `b` and `B` are separate
  information kinds;
- `ucum:[pH]` does not convert into a plain count, because `pH` has its own logarithmic kind;
- `ucum:rad/s` does not convert into `ucum:Hz`, because the angle kind separates angular frequency
  from frequency;
- arbitrary units convert into nothing at all (§7.2).

What the fences do **not** do is separate two named units that share a dimension. `unece:D13` (the
sievert) and `unece:GRY` (the gray) are dimensionally equal and convert with factor 1, as `Gy` and
`Sv` always have natively (doc/07 §9). The distinction survives in the *spelling*, which is what the
unit slot carries and what `BVN_UNIT_REDUCE` will not rewrite — and that is precisely why a table
row has to be chosen by the publisher's label rather than by its value (§9.5, §9.6).

### 4.3 Policy, normalisation, and the writer

`bvnr_unit_policy_t` takes its targets and rules as unit **strings**, parsed with `bvn_parse_unit`
at the point the policy is set. Those strings therefore accept the profile notation for free:

```c
static const bvnr_unit_rule_t rules[] = {
    { .path = ".patient.systolic", .unit = "ucum:mm[Hg]", .mode = bvnr_rule_require },
};
```

and the assertion is satisfied by a document that writes `mmHg`, or `ucum:mm[Hg]`, or `k~Pa` —
because the rule is dimensional, evaluated on the parsed unit, and none of those three spellings
survives into the comparison. Target lists, `bvnr_normalise_si`, `--require-dimension` and
`--field` behave identically.

The writer's validation half (`bvnr_writer_set_unit_policy`) is likewise unchanged. A producer can
be held to "every pressure is written in mm[Hg]" whether it spells that natively or not.

### 4.4 Invariants the profile preserves

All of the following hold for a profile unit exactly as they hold for a native one:

- the unit is inside the bytes, not in a sidecar attribute or an external table;
- it binds to one value, not to a variable or a file;
- the parser checks it, on every parse, with nothing for the application to call;
- a unit that does not parse stops the document — it does not arrive at the application as a string
  for the application to deal with;
- a unit that parses is dimensionally analysable, unless it is profile-only, in which case it is
  explicitly and detectably not (`bvn_unit_is_profile_only`) rather than silently wrong. §3.1 is
  what buys this, and it is the reason there is no opaque-passthrough outcome.

---

## 5. Serialisation

### 5.1 Canonical output

`bvn_unit_to_string` emits the **native** canonical form whenever every component has one:

```
parse "ucum:mm[Hg]"   → write "mmHg"
parse "ucum:kg.m/s2"  → write "k~g·m/s²"
parse "ucum:10*3/uL"  → write "n~L⁻¹"
parse "unece:KGM"     → write "k~g"
parse "qudt-qk:Mass"  → write "k~g"
```

A unit containing a profile-only component (§3.6, §11.3) has no native form, so the profile prefix
becomes part of its canonical spelling and the whole expression is emitted in profile notation, in
the namespace that owns the opaque base:

```
parse "ucum:[IU]"     → write "ucum:[IU]"
parse "ucum:[IU]/L"   → write "ucum:[IU].L-1"
parse "ucum:k[IU]"    → write "ucum:k[IU]"
parse "unece:XBX"     → write "unece:XBX"
```

The compound case is worth reading closely: the output is the profile's own *canonical* code, not
the input text. `[IU]/L` comes back as `[IU].L-1` because the writer emits explicit signed exponents
for the same reason the reader consumes them (§3.3) — a `/` would re-parse under UCUM's rule and
Bovnar's differently as soon as a third term appeared. Re-parsing either output yields the same
`value_unit_t`, so round-trip is closed in both cases without any state outside the struct.

The guard sits at the head of both formatters (`bvn_unit_to_string_ex` and the plain
`bvn_unit_to_string`), *before* flag handling, because `BVN_UNIT_REDUCE` folds a unit towards named
SI and an opaque unit has no SI form to fold towards. One unit that printed differently through the
two formatters would be a round-trip hole.

A unit that somehow mixed opaque bases from two namespaces has no single spelling and is refused
(negative return) rather than written in whichever namespace came first. No document can produce
one — a code is read in exactly one namespace — but the API can compose one, and the honest answer
there is a failure rather than a string that re-parses as something else.

**Three shapes have no spelling, not one.** The mixed-namespace case above is the one a reader
expects; the other two follow from what a *flat* vocabulary is (§2.2). `unece`, `qudt`, `qudt-qk`,
`om` and `cf` match one whole code entire — no operators, no exponents, no prefixes — so a flat
profile's opaque unit can be written only as a **single unprefixed component at exponent 1**:

| Unit | Spelling | Why |
|------|----------|-----|
| `unece:XBX` | `unece:XBX` | one code, exponent 1 — the only shape a flat vocabulary has |
| `bu_unece_box²` | *none* | UN/ECE has no notation for an exponent |
| `bu_unece_box · m` | *none* | and none for a product, native component or not |
| `ucum:[IU]²` | `ucum:[IU]2` | UCUM is an **expr** profile, so this one is spellable |

All three refusals are `bvn_unit_to_string` returning -1 and the writer reporting
`error_unit_illegal`; none of them can arise from a document, only from an API caller composing a
`value_unit_t` by hand. Such a unit stays **structurally valid** and fully usable in memory — it
compares, and it converts to itself — because a unit with no text is still a unit. `bvn_unit_valid`
answers the structural question and says so in its own contract; spellability is
`bvn_unit_to_string`'s answer, and the two are deliberately distinct.

### 5.2 What canonical output loses, and on which path

Formatting a `value_unit_t` is lossy in exactly one respect: annotations. `bvn_unit_to_string` on
`ucum:mL{total}` gives `m~L`, and `{total}` is gone; `ucum:{RBC}/uL` and `ucum:{cells}/uL` both give
`µ~L⁻¹`. Nothing else is lost — a translated unit's meaning survives exactly, and a profile-only
unit round-trips through §5.1.

**Which path a document takes decides whether it sees that loss.** Re-serialising a document the
library parsed — `bovnar pretty-print`, the reader-to-writer round trip — re-emits the type
annotation from the captured source text, so the producer's spelling survives verbatim, annotations
included:

```
$ cat ann.bvnr
#!bovnar 1.2
.a = <float:64,ucum:mL{total}> 1.0;
.b = 2.0 ucum:{RBC}/uL;

$ bovnar pretty-print ann.bvnr
#!bovnar 1.2
.a = <float:64,ucum:mL{total}> 1.0;
.b = <float:64,_10,ucum:{RBC}/uL> 2.0;
```

`.b` shows both halves of that at once: the inline unit was promoted to an annotation — which is
`pretty-print`'s ordinary behaviour, not the profile's — and the annotation text came through it
verbatim.

Constructing a document through the writer API **was** the path that lost it, and by default
still writes the canonical form: `bvnr_write_float_unit` and its siblings take a `value_unit_t`, and
a `value_unit_t` has nowhere to hold an annotation. What changed is that the spelling now has a way
across. The reader hands it out on every unit-bearing event as `bvnr_data_t.unit_source` (§7.3),
and the writer takes it back by either of two routes:

```c
/* the typed helpers: one shot, for the next value */
bvnr_write_version(w, 1, 2);
bvnr_writer_set_unit_source(w, "ucum:mL{total}", 14);
bvnr_write_float_unit(w, "a", 64, 1.5, mL);     /* .a = <float:64,ucum:mL{total}> 1.5e+0; */

/* bvnr_write_event: the same field the reader filled */
d.unit_source = "ucum:mL{total}";  d.unit_source_length = 14;
```

**The spelling is checked before it is written**, because the alternative is a document that says
two things. It must parse to the very unit it is handed over with (`error_unit_mismatch`
otherwise), it must be something a reader would take back as itself in that unit slot, and — being
profile notation — it needs the `#!bovnar 1.2` directive already emitted, exactly as an opaque unit
does (§2.2). doc/08 §3.1 has the full table. With `BVN_UNIT_REDUCE` or `BVN_UNIT_ASCII_EXP` set the
spelling is ignored: those flags ask for a spelling of their own.

**The two unit slots are not interchangeable**, and this is the one place a consumer meets that.
`ucum:mL{a,b}` and `udunits:kg m-2 s-1` are each a single annotation parameter (§2.4, §13.2) and
neither is a legal inline suffix, where a comma or a space ends the unit. A spelling read from an
annotation is therefore written back into an annotation without question and refused as an inline
suffix when it could not be read from one. Replaying a document's events never meets this — the
annotation events travel with the value — and a consumer forwarding only `ev_data` does.

What is still lost, and on purpose: **a spelling is not part of the unit.** `bvn_unit_equal`,
conversion and the policies never see it, a `value_unit_t` that was stored and re-used carries
none, and a value a unit policy *converted* has no document spelling for the unit it now carries.
The DOM keeps the spelling only where it differs from the canonical form (§7.3), and the Python
bindings keep it on a typed `Quantity` (§8.2).

### 5.3 Emitting a profile code from a native unit

`bvn_unit_to_profile(ns, value_unit_t, char*, size_t)` writes a code in the named vocabulary for a
unit that has one, for producers whose output field is foreign-typed;
`bvn_unit_to_ucum(value_unit_t, char*, size_t)` is the `"ucum"` case, kept for callers that predate
the other namespaces. Both return a negative length when they fail.

The mapping is driven by a **generated reverse table**, one per namespace, not by searching the
forward one. Three things depend on that.

First, the choice is deterministic and canonical. Searching the forward table picked
whichever row its length-sorted order reached first, which got the siemens (`mho`) and the calorie
(`cal_th`) wrong; the reverse table decides once, at build time, by a rule that differs with the
grammar because the competing rows differ with it:

| Grammar | The rows competing for one unit | Rule |
|---|---|---|
| expression (`ucum`, `udunits`) | *spellings of one atom* — `S` and `mho`, `cal` and `cal_th`, `L` and `l`, `mo` and `mo_j` | **shortest wins**, ties alphabetically: the vocabulary's own abbreviation is the short one |
| flat (`unece`, `qudt`, `qudt-qk`, `om`) | *different codes for one unit* — `JOU` and `J55`, `PAL` and `C55`, `TON_SHORT` and `TON` | **first declared wins**: the data file lists the canonical code first and its aliases after |

Applying the expression rule to a flat vocabulary is what made this worth stating. Every Rec 20 code
is two or three bytes, so "shortest, ties alphabetically" degenerates into "alphabetically first" and
picked the filler code every time: a joule came out as `J55` (Rec 20's *watt second*), a pascal as
`C55` (*newton per square metre*), a mole as `C34`, a tonne as `2U` (*megagram*), and QUDT's short ton
as the bare, ambiguous `TON`. Each is worth the right number and none is the code a reader of that
vocabulary expects to be handed.

A row may also carry **`.reverse = false`**, which removes it from the running in either grammar. It
is for the case no ordering can see: a code whose *value* is right and whose *meaning* is a different
quantity. UCUM's `eq` is worth a mole and means an equivalent — and, being a byte shorter than `mol`,
was the code every mole was written as. `[oz_ap]` is worth a troy ounce and says *apothecary* about a
document that never did. Both stay legal to read. A unit whose every row is flagged is a build error:
something this profile can read and cannot write is a hole, not a preference.

Second, each row carries the atom's own decade, which is what lets a base whose code is *itself*
prefixed be written at all. `m[Hg]` is a metre of mercury — bovnar's `mmHg` times 10³ — so writing
plain `mmHg` emits the prefix for `0 − 3` and produces `mm[Hg]`. The hectare is the same shape
(`ar` carries −2, so `ha` becomes `har`). Without the decade those bases had no UCUM form at all.

Third, **the key differs by grammar too**. An expression profile emits a prefix, so one row per base
reaches every decade: `g` alone spells the microgram `ug`. A flat profile has no prefix to emit —
`GRM`, `KGM`, `MGM` and `MC` are four separate Rec 20 codes for the gram — so there the key is
*(base, decade)* and a base contributes as many rows as it has codes. Keying a flat profile by base
alone would leave whichever code happened to be shortest and make every other decade unwritable.

It remains **partial by construction**, and in three different ways:

- *No table is complete.* A native unit outside a transliteration table has nowhere to go — the Old
  German units, the water-hardness degrees, the turbidity kinds, `PSU`, `CF`, `kph`, `rpm`,
  and every currency. Membership here is per-unit and not per-family: `mph` has both a UN/ECE
  and a QUDT code while `kph` has neither.
- *A flat profile can only write a single component.* A flat code names a whole unit, so there is no
  way to compose one out of parts: `unece:MSK` reads back as `m/s²`, but a native `m/s²` has no
  UNECE spelling this function can construct. The expression profiles have no such limit.
- *The dimensionless unit is spellable only in an expression profile.* `1` is a bare integer factor
  of 10⁰ in that grammar, so `ucum:1` and `udunits:1` read back as the dimensionless unit. A flat
  vocabulary has no integer-factor production, and the two that do own a unity code refuse it on
  purpose — QUDT's `UNITLESS` and `NUM` and OM's `one` are all `.unsupported`, reading *"the absence
  of a unit — write no unit at all"*. So the five flat namespaces refuse it, `cf` included, which is
  read-only anyway (§17.3).

| Native | `ucum` | `unece` | `qudt` | `udunits` |
|---|---|---|---|---|
| `k~g` | `kg` | `KGM` | `KiloGM` | `kg` |
| `h~Pa` | `hPa` | `A97` | `HectoPA` | `hPa` |
| `m/s` | `m.s-1` | — | — | `m.s-1` |
| `Mi~B` | — | `E63` | `MebiBYTE` | — |
| `mph` | — | `HM` | `MI-PER-HR` | — |
| `kph` | — | — | — | — |
| *(dimensionless)* | `1` | — | — | `1` |

The asymmetry is worth stating plainly: these profiles are good *readers* and partial *writers*. A
round trip that starts in a vocabulary returns to it; one that starts in Bovnar's native registry
may have nowhere to go.

Sweeping the whole native registry — all 264 physical units, each at the twelve prefixes
`si_none da h k M G T d c m µ n` — **865** combinations survive a native → UCUM → native round trip
unchanged, **1926** have no UCUM code, 377 are prefix/unit pairs `bvn_prefix_unit_valid` rejects
before the question arises, and **none round-trips to a different unit**. The last of those is the
invariant; the two counts move whenever the registry gains a unit, so `test_sweep_round_trip` in
`tests/bovnar_ucum_test.c` pins all three rather than leaving them as prose.

---

## 6. The transliteration table

### 6.1 Verified mappings

The shipped table is `src/gendata/ucum.bvnr`: 206 mapped **atoms**, 41 arbitrary units, 65 known
but refused, and UCUM's 20 prefix spellings. What follows is the whole mapped list, grouped as the
data file groups it — and it is the whole of it, checked row by row against the reference
implementation by `check_doc_profile_atoms.py`, which also fails if a mapped code is missing here. Note that these are *atoms*, which is how the table is organised and not how a
producer meets it — `mg/dL` is three atoms and two prefixes, not a row. §6.4 reads the same table
back in terms of whole codes.

Each Bovnar column was produced by parsing `ucum:<code>` with the reference implementation and
reading back `bvn_unit_to_string` and the coherent-SI factor. The UCUM column is the table author's;
§9.2 says what the generator checks about it and §9.5 what the outside proof does.

**SI base units**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `m` | `m` | `1.0` |
| `s` | `s` | `1.0` |
| `g` | `g` | `0.001` |
| `A` | `A` | `1.0` |
| `K` | `K` | `1.0` |
| `mol` | `mol` | `1.0` |
| `cd` | `cd` | `1.0` |
| `rad` | `rad` | `1.0` |
| `sr` | `sr` | `1.0` |

**Named SI-derived units**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `Hz` | `Hz` | `1.0` |
| `N` | `N` | `1.0` |
| `Pa` | `Pa` | `1.0` |
| `J` | `J` | `1.0` |
| `W` | `W` | `1.0` |
| `V` | `V` | `1.0` |
| `Ohm` | `Ω` | `1.0` |
| `F` | `F` | `1.0` |
| `C` | `C` | `1.0` |
| `S` | `S` | `1.0` |
| `mho` | `S` | `1.0` |
| `Wb` | `Wb` | `1.0` |
| `T` | `T` | `1.0` |
| `H` | `H` | `1.0` |
| `lm` | `lm` | `1.0` |
| `lx` | `lx` | `1.0` |
| `Bq` | `Bq` | `1.0` |
| `Gy` | `Gy` | `1.0` |
| `Sv` | `Sv` | `1.0` |
| `kat` | `kat` | `1.0` |

**Metric non-SI**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `L` | `L` | `0.001` |
| `l` | `L` | `0.001` |
| `t` | `t` | `1000.0` |
| `u` | `Da` | `1.66053906892e-27` |
| `eV` | `eV` | `1.602176634e-19` |
| `ar` | `c~ha` | `100.0` |
| `st` | `m³` | `1.0` |
| `bar` | `bar` | `100000.0` |
| `atm` | `atm` | `101325.0` |
| `Ao` | `Å` | `1e-10` |
| `b` | `barn` | `1e-28` |
| `AU` | `au` | `149597870700.0` |
| `pc` | `pc` | `3.085677581491367e+16` |

**Time**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `min` | `min` | `60.0` |
| `h` | `h` | `3600.0` |
| `d` | `d` | `86400.0` |
| `wk` | `wk` | `604800.0` |
| `mo` | `mo` | `2629800.0` |
| `mo_j` | `mo` | `2629800.0` |
| `a` | `yr` | `31557600.0` |
| `a_j` | `yr` | `31557600.0` |
| `a_t` | `yr_trop` | `31556925.9747` |
| `a_g` | `yr_greg` | `31556952.0` |
| `mo_s` | `mo_syn` | `2551442.976` |
| `mo_g` | `mo_greg` | `2629746.0` |

Bovnar's `yr` is 31557600 s = 365.25 d and its `mo` is 2629800 s = 30.4375 d, which are the
**Julian** year and month exactly — and which is what UCUM's unqualified `a` and `mo` are, so all
four spellings map with no loss. The tropical and gregorian variants do not (§6.4).

**Angle and logarithmic**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `deg` | `°` | `0.017453292519943295` |
| `gon` | `grad` | `0.015707963267948967` |
| `'` | `arcmin` | `0.0002908882086657216` |
| `''` | `arcsec` | `4.84813681109536e-06` |
| `circ` | `rev` | `6.283185307179586` |
| `Np` | `Np` | `1.0` |
| `B` | `da~dB` | `10.0` |
| `[pH]` | `pH` | `1.0` |
| `sph` | `sph` | `12.566370614359172` |

**Temperature**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `Cel` | `°C` | `1.0` |
| `[degF]` | `°F` | `0.5555555555555556` |
| `[degRe]` | `°Re` | `1.25` |
| `[degR]` | `°Ra` | `0.5555555555555556` |

**Length and area, US and Imperial**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `[in_i]` | `in` | `0.0254` |
| `[ft_i]` | `ft` | `0.3048` |
| `[yd_i]` | `yd` | `0.9144` |
| `[mi_i]` | `mi` | `1609.344` |
| `[nmi_i]` | `nmi` | `1852.0` |
| `[ft_us]` | `ftUS` | `0.3048006096012192` |
| `[fth_i]` | `fath` | `1.8288` |
| `[ly]` | `ly` | `9460730472580800.0` |
| `[mil_i]` | `thou` | `2.54e-05` |
| `[hd_i]` | `hand` | `0.1016` |
| `[in_us]` | `inUS` | `0.0254000508001016` |
| `[yd_us]` | `ydUS` | `0.9144018288036576` |
| `[fth_us]` | `fathUS` | `1.8288036576073152` |
| `[rd_us]` | `rdUS` | `5.029210058420117` |
| `[ch_us]` | `chUS` | `20.116840233680467` |
| `[lk_us]` | `lkUS` | `0.20116840233680466` |
| `[fur_us]` | `furUS` | `201.16840233680466` |
| `[mi_us]` | `miUS` | `1609.3472186944373` |
| `[mil_us]` | `m~inUS` | `2.54000508001016e-05` |
| `[rch_us]` | `h~ftUS` | `30.480060960121918` |
| `[rlk_us]` | `ftUS` | `0.3048006096012192` |
| `[pnt]` | `pnt` | `0.00035277777777777776` |
| `[pca]` | `pca` | `0.004233333333333334` |
| `[lne]` | `lne` | `0.002116666666666667` |
| `[pnt_pr]` | `pnt_pr` | `0.0003514598` |
| `[pca_pr]` | `pca_pr` | `0.0042175176` |
| `[mesh_i]` | `in⁻¹` | `39.37007874015748` |
| `[sin_i]` | `in²` | `0.00064516` |
| `[sft_i]` | `ft²` | `0.09290304` |
| `[syd_i]` | `yd²` | `0.83612736` |
| `[acr_us]` | `acUS` | `4046.872609874252` |
| `[srd_us]` | `rdUS²` | `25.292953811714074` |
| `[smi_us]` | `miUS²` | `2589998.470319521` |
| `[sct]` | `miUS²` | `2589998.470319521` |

The **whole** survey series maps, onto native units built on the survey foot rather than onto the
international ones they are 2 ppm from — that is what `inUS`, `ydUS`, `rdUS` and the rest above are
for, and §6.3 explains why the distinction is worth a unit each. Ramden's chain and link
(`[rch_us]`, `[rlk_us]`) are the survey foot at a hecto prefix and bare. The **British** series
`[ch_br]`, `[ft_br]`, `[yd_br]` is a different matter and is refused: those sit 7.9e-7 from the
international units of the same name, which §6.3 also covers.

**Mass**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `[lb_av]` | `lb` | `0.45359237` |
| `[oz_av]` | `oz` | `0.028349523125` |
| `[dr_av]` | `dr` | `0.0017718451953125` |
| `[gr]` | `gr` | `6.479891e-05` |
| `[stone_av]` | `st` | `6.35029318` |
| `[oz_tr]` | `oz_t` | `0.0311034768` |
| `[pwt_tr]` | `dwt` | `0.00155517384` |
| `[sc_ap]` | `sc` | `0.0012959782` |
| `[car_m]` | `ct` | `0.0002` |
| `[oz_ap]` | `oz_t` | `0.0311034768` |
| `[ston_av]` | `tn_sh` | `907.18474` |
| `[lton_av]` | `tn_l` | `1016.0469088` |
| `[scwt_av]` | `h~lb` | `45.359237` |
| `[lcwt_av]` | `cwt_l` | `50.80234544` |
| `[lb_tr]` | `lb_t` | `0.3732417216` |
| `[lb_ap]` | `lb_t` | `0.3732417216` |
| `[dr_ap]` | `dr_ap` | `0.0038879346` |

`[oz_ap]` is read and never written (`.reverse = false`, §5.3): the apothecary ounce *is* the troy
ounce by value, and writing a troy ounce back as `[oz_ap]` would tell a UCUM reader "apothecary"
about a document that never said it.

**Volume**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `[gal_us]` | `gal` | `0.003785411784` |
| `[qt_us]` | `qt` | `0.000946352946` |
| `[pt_us]` | `pt` | `0.000473176473` |
| `[foz_us]` | `fl_oz` | `2.95735295625e-05` |
| `[tbs_us]` | `tbsp` | `1.478676478125e-05` |
| `[tsp_us]` | `tsp` | `4.92892159375e-06` |
| `[cup_us]` | `cup` | `0.0002365882365` |
| `[gil_us]` | `gi` | `0.00011829411825` |
| `[fdr_us]` | `fl_dr` | `3.6966911953125e-06` |
| `[min_us]` | `minim` | `6.1611519921875e-08` |
| `[pk_us]` | `pk` | `0.00880976754172` |
| `[bu_us]` | `bsh` | `0.03523907016688` |
| `[bbl_us]` | `bbl` | `0.158987294928` |
| `[gal_br]` | `gal_uk` | `0.00454609` |
| `[qt_br]` | `qt_uk` | `0.0011365225` |
| `[pt_br]` | `pt_uk` | `0.00056826125` |
| `[foz_br]` | `fl_oz_uk` | `2.84130625e-05` |
| `[gil_br]` | `gi_uk` | `0.0001420653125` |
| `[bu_br]` | `bsh_uk` | `0.03636872` |
| `[cin_i]` | `in³` | `1.6387064e-05` |
| `[cft_i]` | `ft³` | `0.028316846592000004` |
| `[cyd_i]` | `yd³` | `0.764554857984` |
| `[gal_wi]` | `gal_dry` | `0.00440488377086` |
| `[dqt_us]` | `qt_dry` | `0.001101220942715` |
| `[dpt_us]` | `pt_dry` | `0.0005506104713575` |
| `[bf_i]` | `fbm` | `0.002359737216` |
| `[cr_i]` | `cord` | `3.624556363776` |
| `[crd_us]` | `cord` | `3.624556363776` |

**Pressure, energy, force, power**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `m[Hg]` | `k~mmHg` | `133322.387415` |
| `[psi]` | `psi` | `6894.757293168362` |
| `tex` | `tex` | `1e-06` |
| `[den]` | `den` | `1.1111111111111111e-07` |
| `att` | `at` | `98066.5` |
| `cal_th` | `cal` | `4.184` |
| `cal` | `cal` | `4.184` |
| `[Cal]` | `k~cal` | `4184.0` |
| `[Btu_IT]` | `Btu` | `1055.05585262` |
| `erg` | `erg` | `1e-07` |
| `[HP]` | `hp` | `745.6998715822702` |
| `[kn_i]` | `kn` | `0.5144444444444445` |
| `dyn` | `dyn` | `1e-05` |
| `gf` | `g·gn` | `0.00980665` |
| `[lbf_av]` | `lbf` | `4.4482216152605` |
| `[g]` | `gn` | `9.80665` |
| `m[H2O]` | `mH2O` | `9806.65` |
| `[in_i'Hg]` | `inHg` | `3386.388640341` |
| `[in_i'H2O]` | `in·mH2O/m` | `249.08890999999997` |
| `[PRU]` | `mmHg·s/m~L` | `133322387.415` |
| `[wood'U]` | `mmHg·min/L` | `7999343.2449` |
| `[Btu]` | `Btu_th` | `1054.3502644888888` |
| `[Btu_th]` | `Btu_th` | `1054.3502644888888` |
| `cal_IT` | `cal_IT` | `4.1868` |
| `cal_m` | `cal_m` | `4.19002` |
| `cal_[15]` | `cal_15` | `4.1858` |
| `cal_[20]` | `cal_20` | `4.1819` |
| `[Btu_59]` | `Btu_59` | `1054.8` |
| `[Btu_60]` | `Btu_60` | `1054.68` |
| `[Btu_m]` | `Btu_m` | `1055.87` |

`m[Hg]` is the row that makes the decade mechanism visible: it is a **metre** of mercury column, so
its native target carries the kilo that makes `mm[Hg]` come out as plain `mmHg`.

`gf` is one of two rows where a single UCUM atom becomes a two-component Bovnar expression: Bovnar
has no gram-force, but `g·gn` is gram times standard gravity, dimensions `[1,1,-2,0,0,0,0]`, factor
`0.00980665`. (The other is `U` below.) Nothing in the design requires a mapping to be atom-to-atom.

**CGS and radiation**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `P` | `P` | `0.1` |
| `St` | `St` | `0.0001` |
| `Gal` | `Gal` | `0.01` |
| `G` | `G` | `0.0001` |
| `Mx` | `Mx` | `1e-08` |
| `Oe` | `Oe` | `79.57747154594767` |
| `Bi` | `da~A` | `10.0` |
| `Ky` | `c~m⁻¹` | `100.0` |
| `sb` | `sb` | `10000.0` |
| `ph` | `ph` | `10000.0` |
| `Ci` | `Ci` | `37000000000.0` |
| `RAD` | `c~Gy` | `0.01` |
| `REM` | `rem` | `0.01` |
| `R` | `R` | `0.000258` |
| `[e]` | `e` | `1.602176634e-19` |
| `Gb` | `Oe·c~m` | `0.7957747154594768` |
| `Lmb` | `Lmb` | `3183.098861837907` |

`RAD` and `REM` are the case-**sensitive** spellings. UCUM's `[RAD]` and `[REM]` belong to its
case-insensitive variant, which this profile does not implement (§10.3), so they are not codes a
conforming producer of the case-sensitive vocabulary can emit and are not in the table.

**Digital and ratio**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `bit` | `b` | `1.0` |
| `By` | `B` | `1.0` |
| `Bd` | `Bd` | `1.0` |
| `%` | `%` | `0.01` |
| `[ppth]` | `‰` | `0.001` |
| `[pptr]` | `pptr` | `1e-12` |
| `[ppm]` | `ppm` | `1e-06` |
| `[ppb]` | `ppb` | `1e-09` |

**Chemical**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `eq` | `mol` | `1.0` |
| `osm` | `mol` | `1.0` |
| `U` | `µ~mol/min` | `1.6666666666666667e-08` |
| `g%` | `g/d~L` | `10.0` |

**Optics**

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `[diop]` | `m⁻¹` | `1.0` |

**Whole codes, as a producer meets them.** The same table read through the expression grammar:

| UCUM | Bovnar | 1 UCUM unit in coherent SI |
|---|---|---|
| `m/s` | `m/s` | `1.0` |
| `m/s2` | `m/s²` | `1.0` |
| `kg/m2` | `k~g/m²` | `1.0` |
| `kg.m/s2` | `k~g·m/s²` | `1.0` |
| `/min` | `min⁻¹` | `0.016666666666666666` |
| `/uL` | `µ~L⁻¹` | `999999999.9999999` |
| `10*3/uL` | `n~L⁻¹` | `999999999999.9999` |
| `10*6/L` | `µ~L⁻¹` | `999999999.9999999` |
| `mmol/L` | `m~mol/L` | `1.0` |
| `mg/dL` | `m~g/d~L` | `0.01` |
| `ng/mL` | `n~g/m~L` | `1.0000000000000002e-06` |
| `kat/L` | `kat/L` | `1000.0` |
| `meq/L` | `m~mol/L` | `1.0` |
| `U/L` | `µ~mol/min·L` | `1.6666666666666667e-05` |

### 6.2 Collisions — the same spelling, a different unit

These are the codes where the same bytes name different quantities in the two namespaces. They are
harmless **only** because translation runs on UCUM's resolved atom, never by handing the UCUM
string to `bvn_parse_unit` (§3.2) — every one of them would be a silent misreading if it did.
Verified against the native parser:

| Spelling | Native Bovnar | UCUM | Damage if confused |
|---|---|---|---|
| `st` | stone, `6.35029318` kg, dimension `[0,1,0,…]` | stere, `m³`, dimension `[3,0,0,…]` | mass read as volume |
| `B` | byte, information | bel, `da~dB` | a data size read as a level |
| `b` | bit, information | barn, `1e-28` m² | a data size read as an area |
| `Gb` | gigabit (`G~b`, factor `1e9`) | **gilbert**, `Oe·c~m` ≈ `0.7958` A, magnetomotive force | a data size read as a magnetomotive force |
| `a` | *not a unit* | year (Julian) | — (Bovnar declines the ambiguity; see [Unit Ambiguities](07_bovnar_unit_ambiguities.md)) |
| `ar` | *not a unit* | are, `c~ha` | — (the hectare is `ucum:har`, an easy off-by-100) |
| `AU` | *not a unit* | astronomical unit | — (Bovnar spells it `au`) |
| `gf` | *not a unit* | gram-force | — |
| `Cel` | *not a unit* | degree Celsius | — |

Every row above is asserted in `tests/bovnar_ucum_test.c`, and the cross-vocabulary suite pins the
same shape across the other four (§14.2). [Unit Ambiguities §17](07_bovnar_unit_ambiguities.md) is
the reader-facing index of all of them.

`st` and `Gb` are the dangerous two, because on those rows **both sides parse and the dimensions
differ** — everything else in the table fails on one side and is caught. They are also the argument
for the namespace being mandatory rather than a fallback: there is no reading of a bare `st` that
could be made to serve both populations, and a profile that guessed would be wrong for one of them
every time.

`Gb` earns a second look, because the plausible reading of it is the wrong one. UCUM's `b` is the
barn and is **non-metric**, so `G` + `b` genuinely is not a legal prefixed code — which invites the
conclusion that `ucum:Gb` must therefore be refused. It is not: `Gb` is an atom in its own right,
the gilbert. The code resolves, to a magnetomotive force, while the same bytes read natively are a
gigabit. "This prefix cannot attach to that atom" and "these bytes are not a code" are different
statements, and only the first is true here.

### 6.3 Traps that are not spelling collisions

Rows where the spellings differ but the meanings are close enough to be mapped wrongly by
hand. Each is a factor error, not a syntax error, so nothing would catch it downstream.

**`eq` is not `val`.** Bovnar's `val` is documented in `src/gendata/units.bvnr` as the equivalent
*as used in water analysis*, where the ions counted are divalent: its factor is `0.5`, so
`m~val/L` is 0.5 mmol/L. UCUM's `eq` is the generic equivalent, defined as one mole. Mapping
`eq` → `val` would be **wrong by exactly a factor of two** on every clinical electrolyte value.
The table maps `eq` → `mol` and `meq/L` → `m~mol/L`, and leaves `val` alone as the water-analysis
unit it is. Outside the profile `eq` keeps its native meaning, an alias of `val`, so `meq/L` and
`ucum:meq/L` differ by exactly two; the prefix is what decides
(see [`05_bovnar_unit_system.md`](05_bovnar_unit_system.md), water analysis).

**`osm` sits on exactly that footing, and used to be refused instead.** UCUM defines the osmole and
the equivalent identically — `<value Unit="mol" value="1"/>` for each — so the two rows have to
agree, and they did not: `osm` was refused as a "species-dependent relation to the mole". That is
equally true of the equivalent, which depends on valence, and it is a fact about what an osmolality
*means* rather than about what UCUM says the unit is worth. It was the same judgement made two ways
in one file, and `qudt.bvnr` had already made it a third way, mapping `MilliOSM-PER-KiloGM` straight
onto `m~mol/k~g`. `osm` now maps to `mol`, so `mosm` and `mosm/kg` come with it.

Both carry `.reverse = false`, which is not cosmetic. `eq` is two bytes against `mol`'s three, so
the shortest-wins rule made `eq` the code every mole was *written* as; `osm` and `mol` are both
three, and a tie is the only thing that stood between every amount of substance and being written
as an osmole.

**The calorie and the BTU each need two native units.** Bovnar's `cal` is `4.184` J — the
thermochemical calorie, which is what UCUM's unqualified `cal` is, so `cal` and `cal_th` both map to
it. Bovnar's `Btu` is `1055.05585262` J, the **IT** BTU, and `[Btu_IT]` maps to that.

The trap is the *other* convention in each pair, and it is closed by carrying it rather than by
refusing it. Native `Btu_th` (`1054.3502644888889` J) takes UCUM's `[Btu_th]` **and its unqualified
`[Btu]`**, which is thermochemical; native `cal_IT` (`4.1868` J) takes `cal_IT`. Mapping `[Btu]`
onto `Btu` instead would be wrong by about 0.07 %: small, dimensionally correct, and invisible to
every later check, which is the worst shape an error can have here — so the registry gained the unit
rather than the table gaining a refusal. The rest of the family followed the same way: `[Btu_59]`, `[Btu_60]`,
`[Btu_m]`, `cal_m`, `cal_[15]` and `cal_[20]` now map onto native units of their own, each stated at
the exact decimal UCUM states. Only `[Btu_39]` is still refused — UCUM is the sole vocabulary that
defines it, and one publisher naming a unit is a request rather than the corroboration the others
had. UDUNITS runs the other way and is a trap in the mirror image (§13.3).

**The apothecary dram is not the avoirdupois dram.** `[dr_ap]` is 3.8879346 g and bovnar's `dr` is
1.7718451953125 g — a factor of 2.2 apart. Both map, to *different* native units: `[dr_av]` → `dr`,
`[dr_ap]` → `dr_ap`. The apothecary *ounce* and *pound* are the troy ones, so `[oz_ap]` and
`[lb_ap]` land on `oz_t` and `lb_t` beside `[oz_tr]` and `[lb_tr]` — which is exactly what made the
dram a trap while it had no native unit of its own.

**The survey series is not the international series, at 2 ppm.** `[fur_us]`, `[ch_us]`, `[rd_us]`
and `[acr_us]` are built on the US survey foot (1200/3937 m); native `fur`, `ch`, `rd` and `ac` are
international. They were once mapped onto the international atoms — 2 ppm for the lengths, 4 ppm for
the area, dimensionally perfect, and far inside anything a later check would notice — and were
refused for a while on that account.

The registry now carries the survey series itself, so they map to their own units and the near-miss
is gone rather than merely fenced off: `[ft_us]` → `ftUS`, `[in_us]` → `inUS`, `[mi_us]` → `miUS`,
`[rd_us]` → `rdUS`, `[ch_us]` → `chUS`, `[lk_us]` → `lkUS`, `[fth_us]` → `fathUS`,
`[fur_us]` → `furUS`, `[acr_us]` → `acUS`.

**The British series has no native unit, and is refused by name.** `[ch_br]`, `[ft_br]`, `[yd_br]`
and the rest sit `7.9e-7` from the international units of the same name — a genuinely different
foot, and close enough that the factor proof matched the British chain to the international one
until its tolerance was tightened on the strength of exactly this pair (§9.5). Bovnar has no British
foot, so there is nothing to map them to.

They are `error_unit_profile_unsupported` — *named* refusals, not absences. That is the general
arrangement §15 describes: every code the publisher defines and this build cannot carry is listed,
so a producer who sends `[ft_br]` is told "you wrote it right, and this build cannot carry it"
rather than "that is not a code".

**The annotation is not a discriminator.** `ucum:{RBC}/uL` and `ucum:{cells}/uL` compare equal
(§3.4). That is UCUM's semantics faithfully implemented, and it is still a trap for anyone who
expected the unit slot to carry the analyte.

### 6.4 Codes with no Bovnar representation

| Class | Examples | Outcome |
|---|---|---|
| Arbitrary units (41) | `[IU]`, `[iU]`, `[arb'U]`, `[PFU]`, `[CFU]` | **Profile-only** (§3.6) — parsed, comparable, never convertible |
| Special units with a reference | `B[SPL]`, `B[V]`, `B[W]`, `B[kW]`, `B[mV]`, `B[10.nV]` | `error_unit_profile_unsupported` (§3.7) |
| Other special units | `[p'diop]`, `%[slope]`, `[hp'_X]`, `[m/s2/Hz^(1/2)]` | `error_unit_profile_unsupported` |
| Scale outside a prefix decade | `10*4`, `10*5`, `10*7`, `10*8` | `error_unit_profile_unsupported` (§3.5) |
| Constants as units | `[c]`, `[k]`, `[h]`, `[m_e]`, `[G]` | `error_unit_profile_unsupported` — `[e]` is the exception and maps to `e`, the elementary charge being a unit of charge rather than a dimensionless constant |
| Energy conventions with no native counterpart | `[Btu_39]` | `error_unit_profile_unsupported` (§6.3) — the other six now map |
| The British series | `[ch_br]`, `[ft_br]`, `[yd_br]`, `[in_br]`, `[mi_br]` | `error_unit_profile_unsupported` (§6.3) — bovnar has no British foot |
| Over 32 components | any expression exceeding `BVNR_MAX_UNIT_COMPONENTS` | `error_unit_profile_unsupported` |
| A string that is not a UCUM atom at all | `Cal`, `[zzz]`, `notacode` | `error_unit_illegal` |

`error_unit_illegal` is the last row and *only* the last row. Every atom UCUM actually defines is
either mapped, profile-only or named in the refusal list (§15), so a real code never falls through
as "not a code" — that outcome is reserved for a typo. The survey and apothecary series used to sit
here and no longer do: the registry gained `ftUS`, `chUS`, `acUS`, `dr_ap`, `Btu_th`, `cal_IT` and
their siblings, so those codes map (§6.3).

The middle rows are the honest cost of the design. A clinical corpus will hit
`error_unit_profile_unsupported` on real codes, and the profile does not pretend otherwise: it
names the failure rather than accepting the string and leaving the consumer to discover the
problem. Which of these should become real native units is a registry question this document does
not answer — and the answer has been *yes* twice already, which is why the tropical year and the
survey series are named in the paragraph above as codes that now map rather than here as codes that
do not. Neither of the two this paragraph named last is still waiting: osmolality turned out not to need a
unit at all, only consistency with the equivalent (§6.3), and the mean Gregorian month is now
native as `mo_greg`. What remains here is genuinely open — a reference level for the decibel, an
hour angle, a solar mass.

The last row is the distinction the whole error split exists for. An atom in the `.unsupported` list
is refused as *known and uncarryable*; an atom in none of the three atom lists is refused as *not a
code*.
The first tells a producer "write it another way", the second tells them "check your spelling", and
conflating them is how a legitimate code gets reported as a typo.

---

## 7. Data model

### 7.1 The opaque units

Profile units with no native spelling get ids of their own. Each profile owns a **block** of the
`value_base_unit_t` id space — 10 000 ids whose leading two decimal digits identify the vocabulary —
and its opaque units are numbered from the bottom of that block in declaration order. See
doc/05 §12.1 for the whole layout; `src/utils/bvn_internal_dims.h` holds the compile-time checks
that keep the blocks from overlapping.

**A block per profile, not one shared run.** UCUM's arbitrary atoms are not the only units with no
native spelling: UN/ECE's package and count codes (§11.3) are the same shape, and a later vocabulary
may add more. They used to share a single run appended after the last native unit, which meant a
profile that grew a row renumbered every profile below it — and the whole run moved whenever a
native unit was added. Separate blocks end both. Each is bracketed so §7.2 can test membership with
two comparisons, and the pair also tells the writer which namespace owns a given id. As shipped, in
`include/bovnar_profiles.gen.h`:

```c
#define BVN_PROFILE_UCUM_OPAQUE_FIRST  200000   /* 41 arbitrary atoms, [iU] = [IU] */
#define BVN_PROFILE_UCUM_OPAQUE_LAST   200039
#define BVN_PROFILE_UCUM_OPAQUE_COUNT      40
#define BVN_PROFILE_UNECE_OPAQUE_FIRST 300000   /* 5 counts + 20 packages */
#define BVN_PROFILE_UNECE_OPAQUE_LAST  300024
#define BVN_PROFILE_UNECE_OPAQUE_COUNT     25
#define BVN_PROFILE_QUDT_OPAQUE_FIRST  400000   /* FIRST > LAST: contributes none */
#define BVN_PROFILE_QUDT_OPAQUE_LAST   399999
#define BVN_PROFILE_QUDT_OPAQUE_COUNT       0
/* … one group per profile, plus a BVN_SLOT_<NS> macro each */
```

The empty-profile convention (`FIRST > LAST`) is what lets `qudt`, `qudt-qk`, `udunits`, `om` and
`cf` share one registry row shape with the two that do contribute. `om` is a deliberate case rather
than an accident of coverage: OM has its own arbitrary units, and giving them ids here would make
`om:InternationalUnit` incommensurable with the `ucum:[IU]` that is the same unit (§16.3).

Because the id space is now sparse, the tables the library indexes by base unit are **dense**: one
row per defined unit, indexed by `bvni_unit_slot()` rather than by the id. `BVN_UNIT_SLOT_COUNT` is
that row count — `bu_none`, then every native unit, then every opaque unit — and it is *not* a bound
on the enum. The generated table rows spell their index as `BVN_SLOT_UCUM(bu_ucum_iu)` and so on, so
a row and its enumerator cannot drift apart.

The per-profile pairs are what stop a unit being spelled in the wrong namespace. A single hardcoded
`"ucum:"` in the writer would have printed a UNECE package code as `ucum:XBX`, which re-parses as
nothing at all.

**The ids are assigned by `gen_profiles.py`, not written in the data files.** With one profile a
hand-written `.id` was reviewable; with several it is a renumbering trap, because inserting a row in
an earlier profile shifts every id below it. The generator assigns them in registry order and writes
the result to `include/bovnar_profiles.gen.h`, which is committed — so a shift appears as a diff in
review rather than as a silent ABI change.

Each entry carries the empty dimension vector, factor `1.0`, `.affine = false`, the vocabulary's own
metric flag as its prefix policy, and **no native alias** — the alias table is what makes a unit
reachable from native notation, and leaving it empty is what keeps these profile-only.

### 7.2 Incommensurability, via the mechanism currencies already use

The design sketched a new predicate in `bvni_dims_match` / `bvni_kinds_match` comparing arbitrary
multisets. It was not needed. The library already has a class of units with no SI conversion row —
currencies — and the machinery that handles them gives exactly the right answers here.

Two refusals, one each in the two functions everything else is built on:

- `bvn_unit_to_si_factor` sets `*ok = false` on an opaque component;
- `bvn_unit_dimension_vector` returns false on one.

`bvn_units_compatible` is built on both, so it reports false for anything containing an opaque
unit. `bvn_unit_convert_factor` then falls through to `bvn_unit_prefix_only_delta` — the
same-unit-apart-from-prefixes path that exists for currencies — and that is what produces the rest:

| Pair | Result | Why |
|---|---|---|
| `ucum:[IU]` → `ucum:[IU]` | factor 1 | prefix-only delta of zero |
| `ucum:[IU]` → `ucum:[PFU]` | refused | different bases |
| `ucum:[IU]` → `mol`, `%`, `no_unit` | refused | different bases, and no dimension to fall back on |
| `ucum:[IU]/L` → `ucum:[IU]/mL` | factor 0.001 | same bases, prefix delta 3 |

The last row is the one that matters: `[IU]/L` is a genuine concentration and rescaling its volume
is exact. A flat "anything arbitrary is incomparable" rule would have refused it.

Note the shape of the answer: `bvn_units_compatible` says *false* for `[IU]` against itself, while
`bvn_unit_convert_factor` returns 1. That reads oddly until you see it is exactly how `$USD`
against `$USD` has always behaved — compatibility is a statement about dimension, and neither a
currency nor an assay unit has one.

`bvni_is_opaque` is still a two-comparison range test over the whole block, regardless of how many
profiles contribute to it. The block is contiguous and sits above every native unit, and both facts
are static assertions in `bvn_internal_dims.h` rather than comments: a native unit appended past the
block's first id would otherwise become silently incommensurable with everything.

### 7.3 The spelling on the data event

`bvnr_data_t` carries the unit twice: `value_unit` is what it means, and `unit_source` /
`unit_source_length` are the bytes the producer wrote — following the `frac_data`/`frac_length`
precedent from spec 1.1, which solved the same problem for the one other value whose text says more
than its parsed form.

```c
const void* unit_source;          /* e.g. "ucum:mL{total}"; NULL when there is no unit */
uint32_t    unit_source_length;   /* at most BVNR_UNIT_SOURCE_MAX (255) */
```

**On the way in** the reader sets it on `ev_data` and on the type-annotation events whenever
`value_unit` is a real unit. It is the annotation's unit parameter, or the inline suffix when the
annotation carried no unit; where a value has both — `<float:64,mmHg> 120 ucum:mm[Hg]` — it is the
annotation's, which is the one a re-serialisation writes. It follows the unit through arrays: an
annotation inside an array holds for one element, and the spelling goes back to the array's own
with the unit. A `want_unit` conversion leaves it alone, so it is always the spelling of
`value_unit` and never of `conv.unit`.

**On the way out** the writer emits it in place of the canonical form, after the checks §5.2
describes. This section used to explain why that half was not built: every `bvnr_write_*` entry
point takes a `value_unit_t`, and carrying a string beside it looked like a new parameter on the
whole writer surface. It turned out to need one function. The helpers all reach the unit through
`bvnr_write_type_annotation`, so a spelling set once on the writer
(`bvnr_writer_set_unit_source`) and consumed by the next annotation covers every one of them, and a
caller of `bvnr_write_event` already has the field.

**The struct grew**, which is an ABI change — the same one the read-time conversion fields already
made in this release, under the same SOVERSION bump.

**The DOM** keeps the spelling per node (`bvn_dom_get_unit_source`) only where it differs from
`bvn_unit_to_string` of the node's unit, so a large array written in canonical units stores nothing
extra and `km`, `ucum:mm[Hg]` and `ucum:mL{total}` are each kept. A node whose value a unit policy
converted has none.

### 7.4 New error codes

Two, appended after `error_octet_stream_truncated` (48) and moving `BVN_ERROR_COUNT` with them —
the fuzz harnesses use that count as their bound for "is this a real error code" and trap above it,
so a stale count turns a legitimate new error into a fuzz crash.

| Code | Value | Meaning |
|---|---|---|
| `error_unit_profile_unknown` | 49 | The namespace before the `:` is not a profile this build supports |
| `error_unit_profile_unsupported` | 50 | Valid in its vocabulary over known codes, but no Bovnar representation (§6.4) |

Malformed input, and a code over something the vocabulary does not define, stay `error_unit_illegal`
(32). The split is what a producer needs: "you wrote it wrong" and "you wrote it right and we cannot
carry it" call for different fixes.

`error_octet_stream_forbidden` (51) and `error_type_param_whitespace` (52) were appended after
these two in the same release cycle, so the enum ends at 52 and `BVN_ERROR_COUNT` is 53. The static assertion in
`bvn_internal_dims.h` names the last enumerator explicitly, which is what makes that ordering a
build-time fact rather than a comment.

---

## 8. API

### 8.1 C

```c
/* Unchanged. Dispatches on a "name:" prefix; a native string behaves exactly as
 * it does today. This is the single door — every path that reaches a unit goes
 * through it, so a profile unit cannot arrive by a route that skips a check. */
value_unit_t bvn_parse_unit  (const uint8_t* unit, bool* ok);
value_unit_t bvn_parse_unit_n(const uint8_t* unit, uint32_t len, bool* ok);

/* Unchanged. Emits native form, or profile form when a component is
 * profile-only (section 5.1). */
int32_t bvn_unit_to_string(value_unit_t u, char* buf, size_t bufsize);

/* New. Why a rejected unit string is not a unit: error_unit_illegal,
 * error_unit_profile_unknown or error_unit_profile_unsupported. Re-parses, so it
 * is for the error path; a string that DOES parse reports error_none. */
error_code_t bvn_unit_error_code(const uint8_t* unit, uint32_t len);

/* New. True when a unit has no native spelling — it carries an OPAQUE base
 * unit, a UCUM arbitrary atom or a UNECE package/count code — so
 * bvn_unit_to_string emits profile notation, in the namespace that owns it. */
bool bvn_unit_is_profile_only(value_unit_t u);

/* New. A code in the named vocabulary (without the "<ns>:" prefix); negative on
 * failure (section 5.3). */
int32_t bvn_unit_to_profile(const char* ns, value_unit_t u,
                            char* buf, size_t bufsize);

/* New. bvn_unit_to_profile against "ucum", for callers that predate the other
 * vocabularies. */
int32_t bvn_unit_to_ucum(value_unit_t u, char* buf, size_t bufsize);

/* New. WHICH NAMESPACES THIS BUILD CARRIES (section 9.4). The vocabularies are
 * compiled per-namespace, so "there is no such profile" and "this build has no
 * such profile" are the same error code from the outside; this pair is how a
 * consumer finds out up front rather than inferring it from a failed parse.
 * bvn_unit_profile_name returns NULL past the end, and the ORDER is the
 * library's and not a contract -- the set is. Both are present in every
 * configuration; a build with all seven off answers 0 rather than dropping the
 * symbols, so a caller needs no second way to ask. */
uint32_t    bvn_unit_profile_count(void);
const char* bvn_unit_profile_name(uint32_t index);

/* New. Spell the unit of the NEXT value written through a bvnr_write_*_unit
 * helper the way the producer wrote it (sections 5.2, 7.3). One shot. */
bool bvnr_writer_set_unit_source(bvnr_writer_t* w,
                                 const char* text, uint32_t length);

/* New, in bovnar_dom.h. A node's unit as the document spelled it, where that
 * differs from the canonical form; NULL otherwise (section 7.3). */
const char* bvn_dom_get_unit_source(const bvn_dom_node_t* node,
                                    uint32_t* len_out);
```

`bvn_parse_unit` keeps its signature, which is why the policy strings of §4.3 work with no change
at all: `bvnr_unit_policy_t` parses its targets with it, so the notation arrives for free. One
struct changed for this feature — `bvnr_data_t` gained `unit_source`/`unit_source_length` (§7.3) —
so its ABI movement is that, the two error codes and the new entry points. (The release moved more
than the profiles did; the changelog has the whole list.)

### 8.2 Python

`parse_unit`, `unit_to_str`, `units_compatible` and `UnitPolicy` accept and produce every profile
notation with no signature change. Four additions mirror the C ones:

```python
bovnar.unit_to_profile(ns, vu)   # -> str; ns is one of the seven namespaces.
                                 #    Raises when the unit has no code in that
                                 #    vocabulary, which for the read-only "cf"
                                 #    is every unit (§17.3)
bovnar.unit_to_ucum(vu)          # -> str; the "ucum" case of the above
bovnar.unit_is_profile_only(vu)  # -> bool
bovnar.unit_error_code(s)        # -> int (error_code_t), 0 when s parses
```

A namespace this build was not compiled with (§9.4) raises like one that does not exist;
`bovnar.available_profiles()` lists the ones it has.

**The spelling travels through the typed tier.** `loads(typed=True)` puts it on each `Quantity` as
`unit_source`, and `dumps()` writes it back, so a typed round trip of a document spelled in a
profile is byte-identical — annotations, compact prefixes and space-multiplied UDUNITS codes
included. `dumps()` prepends `#!bovnar 1.2` when a kept spelling (or a profile-only unit) needs it
and not otherwise. An array has one annotation and so keeps one spelling: where its leaves disagree,
or one carries none, the array is written in canonical form.

```python
q = bovnar.loads(src, typed=True)["volume"]
q.unit_source                    # "ucum:mL{total}"
str(q.unit)                      # "m~L"
Quantity("1.5", vt, "m~L", unit_source="ucum:mL{total}")   # set by hand
data.unit_source_str()           # the same text, on a Reader callback payload
node.unit_source                 # and on a DomNode, where it differs from the canonical form
```

The existing `from_pint_unit` / `to_pint_unit` bridge is untouched.

### 8.3 CLI

`bovnar version` prints the compiled-in namespaces on a second line, which is where to look first
when a code that should parse does not:

```
bovnar 2.0.0 (spec 1.2)
unit profiles: ucum unece qudt qudt-qk udunits om cf
```

Otherwise, no new flags. `--unit`, `--field`, `--require-dimension` and `--require-field` take unit strings and
therefore take profile strings:

```
bovnar validate --require-field '.patient.systolic=ucum:mm[Hg]' chart.bvnr
bovnar validate --require-field '.line.qty=unece:XBX'           invoice.bvnr
bovnar events --unit 'ucum:mmol/L'   labs.bvnr
bovnar events --unit 'qudt:M-PER-SEC' telemetry.bvnr
```

`bovnar events` shows both halves of the translation, which is the quickest way to see what a code
became: the `type_param` row carries the source spelling and the `type_family` row the parsed unit.

```
  type_param      unit         "qudt:M-PER-SEC"
  type_end        type          <float:64,_10,m/s>
```

`bovnar pretty-print` re-emits the annotation from the captured source text (§5.2), so a document
that went in spelled `qudt:M-PER-SEC` comes out spelled `qudt:M-PER-SEC`.

---

## 9. Build and conformance

### 9.1 Where the tables live

One hand-edited data file per namespace in `src/gendata/`, in the same shape as `units.bvnr`,
`currencies.bvnr` and `prefixes.bvnr`:

| File | Lists |
|---|---|
| `ucum.bvnr` | 20 prefixes, 206 mapped, 40 opaque, 1 opaque alias, 65 unsupported |
| `unece.bvnr` | 1302 mapped, 25 opaque, 152 unsupported |
| `qudt.bvnr` | 2244 mapped, 564 unsupported |
| `qudt-qk.bvnr` | 908 mapped, 257 unsupported |
| `udunits.bvnr` | 41 prefixes, 768 mapped, 98 unsupported |
| `om.bvnr` | 1336 mapped, 124 unsupported |
| `cf.bvnr` | 4454 mapped, 617 unsupported |

Five list kinds, and a code belongs to exactly one of them: the vocabulary's prefix spellings with
their decades (expression profiles only — a flat profile has no prefix mechanism), the codes that
translate (with their native target expression and the vocabulary's metric flag), the opaque codes
(with no target at all), the opaque aliases (a second code for one of the opaque codes, sharing its
id — only `ucum:[iU]`), and the codes that are known but refused (with the reason).
`gen_profiles.py` generates the C lookup tables on every build, wired into CMake next to the other
generators; the generated files are never edited.

The native target is stored as **source text**, not as a pre-baked `value_unit_t`, and the C side
parses it with the same `bvn_parse_unit` every document goes through. That keeps the generator from
needing a second implementation of the unit grammar in Python, which is how a generated table starts
disagreeing with the parser it feeds.

### 9.2 What the generator checks, and what it does not

At build time `gen_profiles.py` refuses to emit a table when:

- a code appears in more than one of the lists (a code has exactly one outcome);
- a flat profile declares prefixes at all, or a prefix declares a decade with no SI prefix — the
  fold could never discharge it;
- two profiles claim the same block tag, a profile claims the native units' tag or the currencies',
  or a profile has more opaque units than its block holds — any of which would make one id answer to
  two vocabularies;
- an opaque name collides across profiles;
- a `.bovnar` target names a prefix or a unit this build's registry does not have;
- the longest profile code this table could emit would overflow `BVNR_UNIT_STRING_MAX` (1088), which
  is what sizes the stack buffers in the writer and the CLI;
- every row naming one unit carries `.reverse = false`, which would make that unit readable and
  unwritable.

The target check is the useful one day to day: a typo in a target would otherwise surface as
`error_unit_illegal` on a code that is perfectly valid, which is the most confusing failure
these tables can produce.

**What it does not check is the vocabulary side**, and that half now lives in a separate tool rather
than in the generator. `gen_profiles.py` still carries no foreign value: a data file states the
target and not what the publisher says the code is worth. Where a value was uncertain, or certainly
*close but not equal* to a native unit, the code went into `.unsupported` instead of being mapped
onto the nearly-right one. That is where `[Btu]`, `cal_IT` and `[dr_ap]` sat until the registry
gained units of exactly their value; all three map today, and the rule that kept them out is the
rule that still holds for whatever has no exact target.

`check_profile_factors.py` (CTest target `bvnr_profile_factors`) closes it for all seven profiles,
though not for all of them equally. It reads UCUM's `ucum-essence.xml`, the UDUNITS-2 XML database
and QUDT's Turtle vocabularies, resolves every code to a factor and a dimension vector, asks the
**reference library** — not a second Python implementation of the unit grammar — what the mapped
native target is worth, and compares. `unece` is reached at one remove; see §9.5.

The generator also emits the **reverse** tables §5.3 uses, choosing the canonical code for each slot
by the grammar's rule (shortest for an expression profile, first-declared for a flat one), honouring
`.reverse = false`, and recording that code's own decade. Deriving them rather than searching the
forward tables at run time is what makes `bvn_unit_to_profile` deterministic; the 689 round trips
quoted in §5.3 are the check that forward and reverse agree.

### 9.3 Tests

One test file per vocabulary, each pinning the section that specifies it, plus the suite that holds
them to each other:

| Test | Target | Assertions | Specifies |
|---|---|---|---|
| `tests/bovnar_ucum_test.c` | `bvnr_ucum_test` | 197 | §2–§10 |
| `tests/bovnar_unece_test.c` | `bvnr_unece_test` | 557 | §11 |
| `tests/bovnar_qudt_test.c` | `bvnr_qudt_test` | 201 | §12 |
| `tests/bovnar_udunits_test.c` | `bvnr_udunits_test` | 224 | §13 |
| `tests/bovnar_om_test.c` | `bvnr_om_test` | 110 | §16 |
| `tests/bovnar_cf_test.c` | `bvnr_cf_test` | 46 | §17 |
| `tests/bovnar_crossvocab_test.c` | `bvnr_crossvocab_test` | 4789 | §14 |

`bvnr_ucum_test` (labels `unit;si;profile;ucum`) carries the behavioural claims of sections 2–10: the three
outcomes with their exact error codes, equivalence with the native spelling, UCUM's non-latching
`/`, annotation inertness, every worked fold case in §3.5 including the four refused decades, each
collision in §6.2, arbitrary-unit incommensurability including the `[IU]/L` ↔ `[IU]/mL` case,
profile-only round-trip, the registry sweep of §5.3, the version gate, and the partiality of
`bvn_unit_to_ucum`.

`tests/bovnar_unit_ext_test.c` pins the block boundary: the last native unit sits below
its profile's `_OPAQUE_FIRST`, and `BVN_UNIT_SLOT_COUNT` grows by one dense table row.

**The conformance corpus covers the profiles too.** `bvnr_conformance` carries a `unit_profile`
group of **59 cases** (`UPR-001` … `UPR-053`), so a third-party implementation can be held to the
same rules rather than only the reference one being tested against itself. It covers the three
outcomes with their exact error codes, the version gate, annotations, the decade fold, the
error-code move of §2.3 (`<float:64,m[s]>` was `error_unexpected_input_byte` and is now
`error_unit_illegal`), and — for the four vocabularies of §11–§13 — flatness, the opaque counts,
quantity kinds, reference time, and cross-vocabulary agreement.

The agreement cases are written a particular way, and it is worth knowing why: each is an annotation
in one notation against an *inline unit* in another, for example
`<float:64,unece:KGM> 12.5 ucum:kg;`. The parser compares the parsed units, so such a case passes
only if both spellings produced the same `value_unit_t` — which lets the corpus test cross-vocabulary
equality without needing any comparison facility of its own, and without a conforming implementation
having to expose one.

### 9.4 One build switch per vocabulary

**`BVNR_WITH_<NAME>_PROFILE`, seven of them, all `ON` by default.** This section used to say the
profiles were unconditional and that the switch did not exist; §15.3 then measured what that cost —
the binary grew 65 % — and §10.4 carried the switch as specified-and-not-built. It is built.

| | |
|---|---|
| Options | `BVNR_WITH_UCUM_PROFILE`, `..._UNECE_...`, `..._QUDT_...`, `..._QUDT_QK_...`, `..._UDUNITS_...`, `..._OM_...`, `..._CF_...` |
| Default | `ON`, every one. The default build is byte-for-byte the build every consumer already has |
| What comes out | That vocabulary's atom, unsupported and reverse tables, its registry row, and the string literals they point into |
| What it saves | All seven off: **1.96 MB → 524 KB** of `libbvnr.so` (a 73 % reduction), measured Release+LTO on x86-64. `cf` alone is about 150 KB |
| Reported by | `bvn_unit_profile_count()` / `bvn_unit_profile_name(i)`, and `bovnar version` prints the list |

**What a switch does not change, and this is the part that matters.** Not the base-unit id space, not
the dense unit tables, not `value_unit_t`, not `error_code_t`, not one exported signature. The
generators still run whole: every `bu_ucum_*` enumerator keeps the value it has and the opaque blocks
stay where they are (§7.1), so two builds with different switches are ABI-compatible and differ only
in which namespaces they translate. A caller compiled against one can link the other.

**An absent namespace is `error_unit_profile_unknown`** — the same answer as a namespace no build
ever defined. That is deliberate and it is what this section always predicted the code was for: the
document is not wrong for naming `ucum`, and a build without `ucum` cannot read it either way. A
consumer that needs to distinguish the two asks `bvn_unit_profile_count`/`bvn_unit_profile_name`
before parsing rather than inferring it from an error afterwards.

**The cost of an absent namespace is symmetrical and total.** It cannot be read, and it cannot be
written: `bvn_unit_to_profile` and `bvn_unit_to_ucum` return −1 for it, and a unit carrying that
vocabulary's opaque units (§7.1) has no spelling at all, so `bvn_unit_to_string` fails rather than
inventing one. There is no half-way state in which a build carries the ids but not the words. A build
that must read `ucum:[IU]` needs `ucum` compiled in.

**How the reduced configurations are kept working.** The switches are compile-time, so an ordinary
build reaches exactly one of the 128 configurations they describe, and a configuration nothing
reaches is one that stops compiling without anyone noticing. `bvnr_profile_configs` compiles the
*amalgamation* twice under the strict `-Werror` flags an integrator uses, and runs a smoke test
against each.

*All seven at `0`* (`tests/profiles_off_smoke.c`) is where the compile itself is at risk: it pins the
contract above and would fail on the two things that actually break there — a registry array left
empty (C99 has no empty initialiser, which is why the table carries a sentinel row) and a table that
loses its last reference and trips `-Wunused-const-variable`.

*A partial set* (`tests/profiles_partial_smoke.c`, `ucum` and `qudt-qk` on) is where the interesting
mistakes live instead, and none of them is reachable when the answer is uniformly "none": an index
that walks absent profiles rather than skipping them, an id space that compacts when a block is
unused — `bu_unece_one` must stay `300000` with `unece` absent, or two builds would disagree about
what an id means — and an opaque unit whose profile is not compiled in, which stays incommensurable
and has no spelling. It also pins the distinction a mixed build is the only place to observe at
once: an absent namespace is `error_unit_profile_unknown` while a bad code in a *present* one is
`error_unit_illegal`.

Both are registered unconditionally of the switches, since a reduced build is the last place they
should be skipped. The per-vocabulary test suites are the other side of the same
arrangement: each is registered only when its own profile is on, and the whole-corpus gates — the
cross-vocabulary suite, the conformance runs, the Python suites — only when all seven are, because
their totals are single numbers that a partial build would make wrong rather than smaller.

### 9.5 The factor proof

> `check_profile_factors.py`, CTest target `bvnr_profile_factors`, labels
> `python;units;profile;vocab`. The `python` label records "drives libbvnr from a Python
> interpreter", which is what a sanitizer pass excludes with `-LE python`: ASan refuses to `dlopen`
> its own runtime into an uninstrumented interpreter, so the ctypes route below is unavailable
> there. The unsanitized pass still runs it.

Everything else in this document proves the tables are **self-consistent**. §14.3 is blunt that this
is not the same as correct: five tables wrong in the same way would agree with each other perfectly.
This tool is the outside check.

For each mapped row it resolves the publisher's own definition of the code to a factor and a
dimension vector, asks the reference library what the native target is worth, and compares. The
native side goes through `bvn_parse_unit` and `bvn_unit_to_si_factor` via the ctypes bindings, never
through a Python reimplementation of the unit grammar — that is exactly how a table starts
disagreeing with the parser it feeds (§9.1).

As of this tree it compares **10663 rows across the seven vocabularies** and reports **0 mismatches**,
0 dead rows and **0 coverage suggestions** — the tables are closed against their publishers, so
there is nothing left for the coverage half to propose.

**Three outcomes, and only one fails the build.**

| Outcome | Meaning | Fatal |
|---|---|---|
| mismatch | the code exists upstream and means something else — a different dimension, or a factor outside tolerance | **yes** |
| dead | the table accepts a spelling the publisher does not define; unreachable from a conforming producer | no (`--strict-dead`) |
| unmapped-but-exact | the publisher defines a code worth something this build already spells, and the table does not carry it | no — a coverage suggestion, and carrying a unit is editorial |

All 10 dead rows are `udunits` spellings the UDUNITS XML database does not define — `gramme`, `pc`,
`degree_Fahrenheit`, `deg`, `nmi`, `dB`, `decibel`, `Np`, `neper`, `pH`. They cost nothing but an
unverifiable row, and each is a spelling a real CF producer plausibly writes.

**What the coverage suggestion is matched against decides what it can find.** It began as an index
of the 180 native *symbols*, which meant a code could only ever be proposed when it was worth a bare
unprefixed atom — and a flat vocabulary spells every prefixed and every compound unit as one whole
token. `unece:A97` is the hectopascal, `KMQ` the kilogram per cubic metre, `MSK` the metre per
square second, `qudt:RAD-PER-SEC` the radian per second: not one of them is a native symbol, so not
one of them could be suggested, and they were exactly the codes a table is most likely to be missing
while its neighbours carry them. The index now also holds every `.bovnar` target any of the five
tables already uses — a target one table has written down is a spelling this build is known to
accept — which is what turned that whole class from invisible into a printed list. At its peak it
printed 1161 suggestions, divided as `qudt-qk` 718, `qudt` 222, `udunits` 113, `unece` 95, `ucum`
13. Every one of them has since been resolved, into a mapped row or a refusal with a reason, and the
count is 0.

**A suggestion asks whether a PRODUCER can write the code, not whether a row exists.** The last two
it printed were `udunits:kg` and `udunits:kilogram`, and neither was missing: an expression profile
reaches them through its prefix mechanism as `k` + `g` and `kilo` + `gram`, with no row and nothing
wrong. Asking the library settles it. A spelling that parses to the *wrong* unit is still reported,
because that is a worse defect than a missing row rather than a lesser one — which is exactly what
`udunits:pt` was (§13.4).

**The two systems are not the same system**, and the corrections are the substance of the UCUM
comparison rather than a detail of it. UCUM's mass base is the gram, so a factor carries 10³ per
mass exponent that bovnar's does not. UCUM has no amount base — the mole is Avogadro's *number* — so
a factor also carries N_A per amount exponent, divided out using UCUM's own value so it cancels
exactly. UCUM's electrical base is charge, which maps to current **and** time, and that is what makes
`A` = `C/s` come out as a bare current. UCUM's plane angle is a base and bovnar's radian is
dimensionless, so that component is dropped.

**The tolerance is measured, and the margin is thin.** A publisher states decimals, and a publisher
states them at whichever CODATA edition it was last revised against — UDUNITS writes the horsepower
as `7.456999e2 W` and the atomic mass unit at the 1986 adjustment, UCUM states the same unit at 2018,
and `units.bvnr` states it at 2022 (the one measured constant in the native registry; its header
names the edition, since an edition nobody wrote down is one nobody can check). So a correct row
still disagrees in the seventh digit. The genuine errors are far above that: the US survey foot
at `2e-6`, the survey acre at `4e-6`, the tropical year at `2.1e-5`, the IT calorie at `6.7e-4`.

What sets the bound is neither of those, but the closest real pair in any of the vocabularies:

| | |
|---|---|
| largest publisher rounding observed | `6.81e-7` — UDUNITS' 1986 CODATA atomic mass unit against bovnar's 2022 one |
| smallest genuine difference observed | `7.87e-7` — UCUM's **British** chain against the international one |

`TOL = 7.5e-7` is the only value that separates them, and it separates them by 10 %. This was found
the hard way: at the `1e-6` the tool originally used, `ucum:[ch_br]` matched bovnar's international
chain and was very nearly added to the table as a coverage gap. The British foot, yard, chain and
rod differ from the international ones only in the seventh digit, which puts a whole family of real
units inside the noise floor of a decimal comparison. They are kept out of `ucum.bvnr` by name, with
the reason written beside the row they would have joined, rather than left to the tolerance (§6.3).

A real disagreement below `6.8e-7` would still pass. That is the honest limit of comparing against
a source that publishes rounded decimals, and no amount of tuning removes it — only tracking each
publisher's stated precision through its whole definition chain would.

**Waivers are printed on every run** rather than silently skipped, because a waiver nobody sees is
how a regression hides behind an old excuse. There are two kinds:

- **Modelling (18 rows).** bovnar carries bit and byte as two base units of information with no
  factor between them; UCUM and UDUNITS both define the byte as the number 8. QUDT goes further and
  models information as *entropy*, so its bit is `ln 2` and its byte `8·ln 2`, the coherent unit
  being the nat — which is exactly why `qudt-qk.bvnr` already refuses `InformationEntropy`. A
  different model, not a wrong conversion.
- **The publisher is wrong or rounded (4 rows).** UCUM defines the phot as `1e-4 lx`; a phot is one
  lumen per square centimetre, i.e. `1e4 lx` — the value is inverted. Native `ph` is correct and the
  profile keeps mapping to it. UCUM also rounds the mercury column to `133.3220 kPa` (2.9 ppm) and
  QUDT the torr to `133.322 Pa` (2.8 ppm), both past the tolerance and both against exact
  conventional values bovnar carries. The fourth is `unece:MON`, below.

**It needs the publications, and a test must not fetch.** The files are cached under
`<build>/vocab/`; populate it once with `python3 check_profile_factors.py --fetch`. Without a cache
the test **skips green**, which is the same rule `bvnr_web_links` follows. CI should fetch and then
pass `--strict`, so that a skip there is a failure rather than a quiet pass.

**QUDT needs no evaluator, and its quantity kinds get a sharper check than the others.** UCUM and
UDUNITS state a unit as an expression over other units, so both need a parser. QUDT states each
unit's own `conversionMultiplier` and its dimension vector as an IRI local name
(`A0E0L1I0M0H0T0D0`), so reading it is a table lookup. A quantity kind has no multiplier at all —
and there the check is the claim §12.3 actually makes, that a kind maps to the **coherent** SI unit
of that kind. Reporting a kind as `(1.0, its dimensions)` turns that into two ordinary assertions —
the dimensions agree, and the native factor is exactly 1 — so `qudt-qk:Length` mapped to `ft` would
fail on the factor although its dimensions are perfect.

**`unece` is checked through a secondary source, and the difference is not cosmetic.** Rec 20 states
its conversion factors in prose, so there is no primary artefact to resolve. QUDT carries a
`qudt:uneceCommonCode` on many of its units, which gives a machine-readable UNECE-code-to-value map
at one remove — QUDT asserting what a Rec 20 code means, not UN/ECE. Three things follow, and the
tool implements all three rather than quietly treating the result as primary:

- **A disagreement is evidence, not proof.** It says one of the two tables is wrong, not which. The
  first one found is a case in point: QUDT attaches `MON` to its own `MO`, a unit its description
  calls the *synodic* month of 29.53059 days. Rec 20's `MON` is a commercial month, so the
  cross-reference is what is wrong there — a trade code list does not mean the lunar cycle — and
  bovnar's Julian month stays. It is carried as a waiver and printed every run.
- **A code QUDT does not mention is not thereby absent from Rec 20.** The dead-row check is
  therefore switched off for this vocabulary entirely, and the rows the cross-reference does not
  reach are counted and named on every run instead. There are 6: `CEL`, `FAH`, `GRY`, `HEN`, `MOL`
  and `TNE`.
- **A code several QUDT units claim is usable only when they agree.** The cross-reference covers
  1488 codes; 81 have more than one claimant. Most are aliases of one unit, but `J62` is claimed by
  both a barrels-per-hour and a barrels-per-second unit, which differ by 3600. 11 codes disagree
  that way and go unchecked — which is why `TNE`, claimed inconsistently, is in the uncovered list
  above.

The cross-reference also **grew the table**, from 100 mapped codes to 201 in one pass. The tool
listed 151 codes whose value is exactly a native unit and which `unece.bvnr` did not carry; 101 were
taken. What decided the other fifty is worth stating, because it is precisely what a value-only
match cannot do: **a suggestion is a claim about a number, not about a meaning.** Dimensions
collapse, so `D13` (the sievert) matched the gray, `D44` (var) and `D46` (volt-ampere) both matched
the watt, `NU` (the newton metre) matched the joule, and `C80` (the rad) matched the rem. Every one
of those pairs is dimensionally equal and freely convertible (doc/07 §9); what distinguishes them is
the *spelling*, which is what a unit slot carries. Accepted on the number alone, each would have put
a document's own vocabulary word for one quantity onto another. Every row was read against QUDT's
label for the unit carrying the code, and the ones left out divide into three groups:

| Left out | Why |
|---|---|
| ratios of two named units — `mg/kg`, `mL/L`, `cm³/m³`, `bar/bar` | each is worth exactly `ppm`, `‰` or 1, but the code says a ratio *of what*, and collapsing it onto the generic unit loses that |
| logarithmic and information scales — `phon`, `sone`, `Erlang`, `Nat`, `baud` | no native form, or a modelling difference: QUDT makes `baud` dimensionless where native `Bd` is a rate |
| a quantity that is not the matched unit — `J/m³` (energy density, matched `Pa`), `J/kg` (specific energy, matched `Gy`), `W/sr` (radiant intensity, matched `W`) | same dimension, different quantity |

`J39` (mean BTU) and `DRI` (UK dram) were left out for the reason §6.3 already gives for their UCUM
counterparts: native `Btu` is the IT one and native `dr` the avoirdupois, and a code whose label
says otherwise is a factor trap rather than a coverage gap.

Arbitrary and special units (`isArbitrary`, `isSpecial`, and any QUDT unit with no
`conversionMultiplier`) have no factor to check — they are exactly the ones carried as opaque or
refused. The standing risk of §10.2 is **reduced everywhere and retired nowhere**: four tables now
rest on their publishers, and the fifth rests on another publisher's reading of its publisher.

### 9.6 Synchronisation between the tables

Checking each table against its own publisher leaves a second question open: whether the five agree
with **each other** about what bovnar's registry contains. §14's concept table asks that of the
concepts it lists; this asks it of every native unit, and it is the same tool — the coverage half of
`check_profile_factors.py`, read across all five outputs at once rather than one profile at a time.

For each native unit, the question is which profiles map it against which vocabularies
*define* something equal to it. A unit that three tables carry and a fourth omits — while that
fourth vocabulary has a perfectly good code for it — is a synchronisation gap, and on the first
sweep there were 65 of them. They were overwhelmingly one-sided:

| Profile | Units its own vocabulary defines that the table omitted |
|---|---|
| `qudt` | 56 |
| `unece` | 9 |
| `ucum` | 8 |
| `udunits` | 5 |

`qudt` was the outlier by an order of magnitude: it had no curie, no dyne, no roentgen, no phot or
stilb, no poise or stokes, none of the imperial volumes, and neither the volt-ampere nor the var
although QUDT defines all of them. Closing it took the table from 158 mapped codes to 244.

**The gap could not be closed by the numbers**, and this is the same lesson §9.5's coverage
suggestion carries. Matching purely on value proposed the lux for a luminance, the rem for the rad,
and the watt for *both* the volt-ampere and the var — because those pairs are equal in SI and differ
only in which word the document uses (doc/07 §9). Every row was read against the publisher's own
label before it was accepted.

**A second pass closed the compound half of the same gap.** The first pass could only see codes
worth a bare native symbol (§9.5), which is why it reported nine for `unece` and none at all of the
kind a flat vocabulary is most likely to be missing. Once the index carried the tables' own targets,
another 71 rows landed — `unece` from 201 to 252 and `qudt` from 244 to 263, the counts of that pass
(§9.1 has today's) — and they divide into two groups that are worth naming, because both are gaps a
flat grammar creates and an expression grammar cannot have:

| Group | Examples | Why the expression profiles never had the gap |
|---|---|---|
| a **prefixed** unit | `unece:A97` hPa, `KVT` kV, `4H` µm, `2Q` kBq, `qudt:KiloCAL_TH` | `ucum` and `udunits` reach every decade by emitting a prefix; a flat vocabulary needs a separate code per decade, and there were 25 missing |
| a **compound** unit | `unece:MSK` m·s⁻², `KGS` kg·s⁻¹, `C65` Pa·s, `JE` J·K⁻¹, `qudt:M2-PER-SEC`, `RAD-PER-SEC` | `ucum:m/s2` is an expression; `unece:MSK` is one token that resembles no native spelling at all |

Most of the compound group is the coherent SI unit of a kind `qudt-qk.bvnr` already maps, which is
the sharpest form the desynchronisation took: the two QUDT namespaces disagreed about their own
publisher, `qudt-qk:KinematicViscosity` translating to `m²/s` while `qudt:M2-PER-SEC` — a unit QUDT
defines — was not a code this build would accept. §14's table now carries a row for each, so the
agreement is pinned rather than asserted.

Reading by label rather than by number mattered again, and in the same direction. The value alone
proposed the **watt** for `KVA` and `MVA` and the **hertz** for all three becquerel codes; apparent
power is not active power and an activity is not a frequency (doc/07 §9).

**It also found a row that was wrong rather than missing, and the shape of it is worth keeping.**
Three vocabularies have a code for the reciprocal minute and a *different* code for the revolution
per minute, and native `rpm` is neither: `rpm` counts revolutions where `rev` is an **angle** of 2π
radians, so `rpm` and `rev/min` differ by 2π and are genuinely held apart by the angle quantity kind
(doc/07 §9). `unece:C94` and `qudt:PER-MIN` — both labelled *reciprocal minute* — were mapped onto
`rpm`, which asserted a rotation the code does not make, while `unece:M46` and `udunits:rpm`, which
do mean the revolution per minute, were not carried at all. `qudt:REV-PER-MIN` had been correct
since it was written, and its own comment says why; the two reciprocal-minute codes were the mirror
error beside it. All four now agree:

| Concept | native | `unece` | `qudt` | `udunits` |
|---|---|---|---|---|
| reciprocal minute | `min⁻¹` | `C94` | `PER-MIN` | — |
| revolution per minute | `rev/min` | `M46` | `REV-PER-MIN` | `rpm` |

The precedent was already in the same file: `unece:C97`, the reciprocal second, maps to `s⁻¹` and
not to the hertz, for exactly this reason. `min⁻¹` is what it looks like beside a unit that has a
name.

What remains unclosed is deliberate: ratios of two named units, logarithmic scales with no native
form, and the British imperial series, whose members sit inside the tolerance of §9.5 and are kept
out by name instead.

---

## 10. Cost, risk, and what is left out

### 10.1 What it cost

| Area | Change |
|---|---|
| Lexer | 15 state-table entries across three states (§2.3); brace/bracket-depth tracking in the type-parameter scanner (§2.4) |
| Unit parser | `src/utils/bovnar_profiles.c` — namespace dispatch, the shared expression parser, the flat matcher, the translator, the fold |
| Registry | Five data files in `src/gendata/` in that pass, seven today (§9.1); `gen_profiles.py` emitting the per-profile and shared tables; one `value_base_unit_t` BLOCK per profile (UCUM 200000–200040, UN/ECE 300000–300024) |
| Version gate | Two checks in the validator — the annotation unit parameter and the inline unit suffix (§2.2); one guard in the writer |
| Compatibility | Two refusals, one each in `bvn_unit_to_si_factor` and `bvn_unit_dimension_vector` (§7.2) |
| Serialisation | One guard at the head of each of the two formatters (§5.1) |
| ABI | Two error codes; three new functions. **No struct changed** *(in that pass — `bvnr_data_t` has since gained `unit_source`, §7.3)* |
| Bindings | Four `ctypes` declarations and four wrappers |
| Tests | 604 assertions across five per-vocabulary files, 3551 in the cross-vocabulary suite, 47 conformance cases; two assertions widened in `bovnar_unit_ext_test.c` (the figures for this pass; §§16–17 added two more files, 137 assertions, 1225 cross-vocabulary assertions and 6 conformance cases) |

The unit parser is the bulk of it. Everything else is small, and — this is what §1.1 buys — the
DOM, the writer, the streaming reader, the policy engine and the CLI needed no work at all, because
a translated unit is an ordinary unit. The two formatter guards, the two compatibility refusals and
the writer's one spec-version check are the entire footprint outside the new file. (That was true
of translation, which is what this table costs out. Keeping a unit's *spelling* came later and did
touch the reader, the writer and the DOM — §7.3.)

### 10.2 What can go wrong

**The tables rest on their authors less than they did, and not nowhere.** §9.5's factor proof now
does confirm that UCUM's `[Btu_IT]` is 1055.05585262 J against `ucum-essence.xml`, and it has found
real errors — the US survey series, the IT calorie, the tropical year, a QUDT rotational speed 2π
out. What it cannot reach is the part of a row that is not a number:

- **What a code MEANS.** A factor comparison confirms that `unece:D13` is worth what native `Sv` is
  worth. It cannot confirm that D13 is the *sievert* rather than the gray, which is the same value
  and a different quantity — and, since the two convert freely (doc/07 §9), a wrong choice there
  survives into the document as a wrong word rather than a wrong number. Every one of those calls
  was made by reading the publisher's label, by hand, and a wrong reading survives every check in
  this repository.
- **The seventh digit.** §9.5's tolerance is measured at `7.5e-7` and the British imperial series
  sits at `7.9e-7`. A real disagreement below `6.8e-7` passes.
- **`unece` at all, primarily.** Rec 20 has no machine-readable factor artefact, so that table rests
  on QUDT's reading of it, on 6 rows the cross-reference does not cover at all, and on 11 codes
  whose QUDT claimants disagree.

The conservative default is what covers the rest: a code whose value was uncertain went into
`.unsupported` or was left out, so the failure mode is a refused code rather than a wrong number.

**Five tables multiply the exposure, and the cross-vocabulary suite does not divide it.** §14 proves
the five agree with each other, which is a real property and catches a whole class of single-table
error — it found the missing ampere on its first run. It cannot catch a *shared* error: five tables
wrong in the same way agree perfectly. That is now the argument for §9.5 rather than a gap beside
it, since the factor proof is the only check in the tree that looks outside the tree at all.

**The tables rot — and something now notices.** UCUM, Rec 20, QUDT and UDUNITS all revise; the data
files do not, and the ordinary build cannot tell, because §9.5's check skips without its cache and a
test must not reach the network. `.github/workflows/vocabulary-drift.yml` runs
`check_profile_factors.py --fetch --strict` weekly, outside the build that gates a pull request, so
a publisher's revision becomes a red scheduled run rather than a silent divergence. It watches what
it can: UDUNITS-2, UCUM and OM are read from their moving branch, QUDT and CF from the release the
tables were built against, and Rec 20 not at all — it has no machine-readable artefact to fetch
(§11.1). A failure is a prompt to review a table by hand; nothing is rewritten automatically.

**~~Whitespace inside a type annotation is accepted and not accumulated~~ — closed.** This section
used to record the one place in the format where a wrong unit was produced silently rather than
refused: a space **between** parameters and a space **inside** a unit were indistinguishable by the
time the parameter was scanned, so `<float:64,k g>` was accepted as `k~g` and
`<float:64,udunits:m s-1>` became `udunits:ms-1` — reciprocal milliseconds — for a value written as
a speed. (In this tree, while the notation was being built: no released reader has the compact form
or a profile, so neither was ever accepted in a release. What 1.1 did accept this way was
`<float:64,m in>`, as the minute.)

The fix is the one this section called for. Whitespace stays ignorable beside a separator (the
family `:`, a `,` between parameters, the closing `>`), and inside a *native* parameter it is
`error_type_param_whitespace`. That half was not a change to the grammar: doc/12 always put every
`ws` in an annotation beside a separator and never derived one in the middle of a parameter, so the
implementation was leniently wrong and now agrees with the normative grammar. It *is* a change to
what parses — a document relying on the old leniency now fails — and that is the point: those
documents carried a unit their author did not write.

**And once the two positions were distinguishable, a third answer became available for the one that
wanted it.** Inside a parameter carrying a profile *namespace*, whitespace is neither deleted nor
refused: it is kept verbatim and handed to the vocabulary. UDUNITS multiplies with a space, so
`udunits:kg m-2 s-1` — CF's commonest spelling — is now the unit it says it is, and `udunits:ms-1` is
still the reciprocal millisecond it has always been. A UCUM annotation keeps its spacing, which makes
§3.4's promise true for the first time. Every other vocabulary refuses a space through its own
grammar as `error_unit_illegal`, which is the right layer: "no such code", not "no whitespace here".
§13.2, which used to explain why the space-separated form could not be supported, now explains how it
is.

**The refusal set is where adopters leave.** §6.4 refuses the referenced bels, the other
function-defined special units, four decades of scale and the British series. A clinical corpus will meet several of those early, and
each is a reason to conclude the profile does not really support UCUM. The counter-argument — that
naming a refusal beats accepting a string you cannot reason about — is correct and will not always
be persuasive. §15 makes this worse before it makes it better: the refusal set is now much larger,
because every code the publisher defines and this build cannot carry is *named* rather than left to
fall through as `error_unit_illegal`. That is the honest arrangement and it is also the one where
an adopter can count what they are not getting.

**Annotation equality will surprise someone.** §3.4 is UCUM's rule faithfully applied, and it still
means two units a clinician reads as different compare as the same.

**One error code moved.** `<float:64,m[s]>` was `error_unexpected_input_byte` and is now
`error_unit_illegal` (§2.3). Both are refusals of the same document, but a consumer switching on the
exact code sees a change.

### 10.3 Deliberately not attempted

- **~~Temperature difference~~ — closed, natively.** `Cel` is still a scale here as in UCUM, and a
  `ucum:Cel` value is still a scale reading. What changed is that Bovnar now has a unit for the
  difference: `ΔK` and its five siblings, in their own quantity kind, so `ΔK → K` is
  `error_unit_mismatch` and `--si` on 25 Δ°C gives 25 ΔK rather than 298.15 K. This entry called it
  "the format's most concrete gap, worth more than this whole profile", and said the fix belonged in
  the *native registry* because importing it through a foreign notation would put it somewhere no
  native document could reach — which is where it went. See doc/temperature_difference.md.

  Two consequences for this document. **One row here was wrong and is corrected**:
  `qudt-qk:TemperatureDifference` mapped to `K`, which made it the same unit as
  `qudt-qk:ThermodynamicTemperature` — the confusion the code's own name rules out. It maps to `ΔK`.
  A quantity kind states a quantity and no unit, so there is no published unit string being diverged
  from. **The CF standard names that are differences by name** (`air_temperature_anomaly` and three
  siblings) are deliberately *not* changed the same way: CF states `canonical_units = "K"` for them,
  and overriding a publisher's stated unit from the sense of its name is a different decision, and one
  that would put `cf:air_temperature_anomaly` in disagreement with `udunits:K` for the same variable.
  CF 1.12's own answer is the `units_metadata` attribute, which is not in the unit slot and so still
  cannot be read by a profile — the converter §2 describes is where that call belongs.
- **Case-insensitive UCUM.** UCUM defines a case-insensitive variant. `ucum:` is the case-sensitive
  one only, which is why `RAD` and `REM` are in the table and UCUM's bracketed `[RAD]`/`[REM]` are
  not (§6.1). `ucum_ci:` is unreserved and undefined; it would need its own atom table and would
  make §6.2's collisions materially worse.
- **~~CF~~.** Superseded: `cf:` is a namespace now (§17), carrying the standard-name table this entry
  said would not fit. What remains not attempted is the other half of what CF is — a reference date
  embedded in the time unit, which still has nowhere to land in a per-value unit slot (§13.4), and
  the `units` strings themselves, which are UDUNITS syntax and reached through `udunits:` (§13).
  `cf:` is read-only for the reason §17 gives: dozens of standard names state the same unit, so
  writing one back would assert a quantity the unit does not know.
- **Exchange rates.** Unchanged and unchangeable: currencies carry no conversion table, and a
  cross-currency conversion is refused rather than guessed (doc/05 §9.6). No vocabulary here yields
  a currency, so the profiles never reach this.

### 10.4 Specified here but not built

One thing this document describes is not in the library, and three that were listed here have since
landed. They are together so the gap is one paragraph to read rather than four sections to
cross-check.

| Not built | Where it is described | Consequence |
|---|---|---|
| ~~Verbatim source preservation (`bvnr_data_t.unit_source`, writer re-emission)~~ | §5.2, §7.3 | **Built.** The reader delivers the spelling on every unit-bearing event, the writer re-emits it from `bvnr_write_event` and — through the one-shot `bvnr_writer_set_unit_source` — from every typed helper, and the DOM and the Python typed tier keep it |
| ~~The generator's factor proof against the publishers' own values~~ | §9.2, §9.5 | **Built** as `check_profile_factors.py`, outside the generator. All seven profiles, six against their own publishers and `unece` at one remove through QUDT |
| ~~`BVNR_WITH_UCUM_PROFILE` and feature reporting~~ | §9.4 | **Built** as seven per-vocabulary switches plus `bvn_unit_profile_count`/`bvn_unit_profile_name`. All seven off takes `libbvnr.so` from 1.96 MB to 524 KB |
| A machine check of `unece` against Rec 20 **itself** | §9.2, §9.5, §10.2 | Rec 20's factors are prose; the table is checked through QUDT's cross-reference instead, which is another publisher's reading |

The factor proof was the one worth building next and is now built (§9.5). It caught four wrong
udunits rows and two wrong UCUM spellings on its first run and found a defect in UCUM's own data;
extending it to QUDT caught six local names QUDT does not define, a month that was the lunar one and
a rotational speed 2π out. `unece` is reached through QUDT's cross-reference, which is a weaker
claim and is reported as one. §14's caveat is therefore reduced rather than lifted: the strongest
statement available for that table is that it agrees with another publisher's reading of Rec 20.
Verbatim source preservation was next and is now built (§7.3). It cost less than the paragraph
that deferred it predicted: one field pair on `bvnr_data_t` and one function, not a parameter on
every writer entry point.

---

## 11. The UNECE profile

> `unece:` — UN/ECE Recommendation 20, *Codes for Units of Measure Used in International Trade*, and
> Recommendation 21, *Codes for Passengers, Types of Cargo, Packages and Packaging Materials*.
> Data file `src/gendata/unece.bvnr` (1302 mapped, 25 opaque, 152 unsupported — every Rec 20 code
> QUDT's cross-reference reaches, which is not the same as every Rec 20 code; §11.1);
> pinned by `tests/bovnar_unece_test.c` (555 assertions).

### 11.1 Why this vocabulary

Rec 20 is, by message volume, the most widely deployed unit code list of the five: it is the unit
vocabulary of UN/EDIFACT, UBL, Peppol and EN 16931, ISO 20022, GS1, and OPC UA's `EUInformation`
structure. A producer whose unit arrives as `KGM` should not have to translate it before it can be
written down, and an industrial-telemetry consumer reading OPC UA payloads should not have to
maintain a second mapping table of its own.

**This is the one table that cannot be closed, and the reason is the vocabulary rather than the
effort.** The other four publish a machine-readable list of everything they define, so "every code
the publisher states is in one of the three lists" is a condition that can be met and checked. Rec
20 states its factors in *prose*. There is no artefact to enumerate, and this profile has always
been reached at one remove, through the `qudt:uneceCommonCode` cross-reference (§9.5). So the table
is now closed against **what that cross-reference reaches** — 1298 mapped and 156 refused, up from
252 and 7 — and a Rec 20 code that no QUDT unit claims is still outside it, because nothing in this
repository can say what it is worth. Where the cross-reference contradicts *itself*, the code is
refused saying so: 81 codes have more than one QUDT claimant, and `J62` is claimed by both a
barrels-per-hour and a barrels-per-second unit, 3600 apart.

Rec 20's Annex II/III sector qualifiers and the Rec 21 codes that name packaging *material* rather
than a countable package remain deliberately absent.

### 11.2 Flat, and why that is not a simplification

Rec 20 is a **flat** profile. A code is one whole token, looked up entire; no prefix is stripped and
no operator is recognised:

```bovnar
#!bovnar 1.2
.mass  = <float:64,unece:KGM> 12.5;      # → k~g
.speed = <float:64,unece:KMH> 88.0;      # → k~m/h
.temp  = <float:64,unece:CEL> 21.0;      # → °C
```

`unece:KGM/MTR`, `unece:MTR2` and `unece:kMTR` are all `error_unit_illegal`. This matters more than
it looks: `KGM` is the kilogram, and a parser that decomposed flat codes would find a `k` prefix on
a `GM` that Rec 20 never defined — and would read `MTS` (metre per second) as a mega-`TS`. A flat
vocabulary spells each prefixed unit with its own separate code, which is why `GRM`, `KGM`, `MGM`
and `MC` are four codes for the gram and why the reverse table (§5.3) is keyed by *(base, decade)*
for a flat profile where an expression profile needs only one row per base.

Case is Rec 20's own and is significant: `unece:kgm` is an error.

### 11.3 Rec 21 packages, and the Rec 20 counts, as opaque units

Rec 21's X-prefixed codes name countable packages, and Rec 20 has a handful of pure count codes.
Both become **opaque** units, through exactly the mechanism UCUM's arbitrary atoms use (§7.1, §7.2):
a `value_base_unit_t` in its profile's own block, no dimension of its own, and no native
spelling, so they serialise back as `unece:<code>`. There are 25: five counts (`C62`, `H87`, `NAR`,
`NPR`, `SET`) and twenty packages (`XBX`, `XPX`, `XCT`, …).

```bovnar
#!bovnar 1.2
.qty = <uint:32,unece:XBX> 12;    # 12 boxes
.pal = <uint:32,unece:XPX> 3;     # 3 pallets
```

| Pair | Result | Why |
|---|---|---|
| `unece:XBX` → `unece:XBX` | factor 1 | prefix-only delta of zero |
| `unece:XBX` → `unece:XPX` | refused | different bases |
| `unece:XBX` → `unece:C62` | refused | a box is not a bare count |
| `unece:XBX` → `k~g` | refused | a box has no mass |
| `unece:XBX` → `%` | refused | a box is not dimensionless — it has no dimension at all |

That is the right answer and not merely a convenient one. Twelve boxes and twelve pallets are not
the same quantity, nothing in the code list says how many of one make the other, and a format whose
whole claim is that a wrong unit fails loudly must not invent a factor here. The same reasoning
refuses `DZN` (dozen), `GRO` (gross), `DPC` and `DZP` as `error_unit_profile_unsupported`: they are
*scaled* counts, and an opaque base admits no multiplier.

Note the shape of the answer, which is the same oddity currencies already have:
`bvn_units_compatible` reports **false** for a box against itself while `bvn_unit_convert_factor`
returns 1. Compatibility is a statement about dimension, and neither a currency nor a package has
one.

### 11.4 A reading of the table

A sample across the shape of the vocabulary — base units, the per-decade codes a flat grammar
needs, the compound codes it needs for the same reason, and the numeric fillers:

| Rec 20 | Bovnar | 1 unit in coherent SI |
|---|---|---|
| `MTR` | `m` | `1.0` |
| `MMT` | `m~m` | `0.001` |
| `KMT` | `k~m` | `1000.0` |
| `4H` | `µ~m` | `1e-06` |
| `GRM` | `g` | `0.001` |
| `KGM` | `k~g` | `1.0` |
| `TNE` | `t` | `1000.0` |
| `LTR` | `L` | `0.001` |
| `MTK` | `m²` | `1.0` |
| `MTQ` | `m³` | `1.0` |
| `SEC` | `s` | `1.0` |
| `HUR` | `h` | `3600.0` |
| `CEL` | `°C` | `1.0` |
| `KEL` | `K` | `1.0` |
| `MTS` | `m/s` | `1.0` |
| `KMH` | `k~m/h` | `0.2777777777777778` |
| `MSK` | `m/s²` | `1.0` |
| `KGS` | `k~g/s` | `1.0` |
| `KMQ` | `k~g/m³` | `1.0` |
| `NEW` | `N` | `1.0` |
| `PAL` | `Pa` | `1.0` |
| `A97` | `h~Pa` | `100.0` |
| `BAR` | `bar` | `100000.0` |
| `C65` | `Pa·s` | `1.0` |
| `JOU` | `J` | `1.0` |
| `JE` | `J/K` | `1.0` |
| `K53` | `k~cal` | `4184.0` |
| `WTT` | `W` | `1.0` |
| `KWH` | `k~W·h` | `3600000.0` |
| `AMP` | `A` | `1.0` |
| `VLT` | `V` | `1.0` |
| `KVT` | `k~V` | `1000.0` |
| `OHM` | `Ω` | `1.0` |
| `HTZ` | `Hz` | `1.0` |
| `MOL` | `mol` | `1.0` |
| `CDL` | `cd` | `1.0` |
| `C81` | `rad` | `1.0` |
| `DD` | `°` | `0.017453292519943295` |
| `P1` | `%` | `0.01` |
| `59` | `ppm` | `1e-06` |
| `2Q` | `k~Bq` | `1000.0` |
| `GRY` | `Gy` | `1.0` |
| `D13` | `Sv` | `1.0` |
| `NU` | `N·m` | `1.0` |
| `D44` | `var` | `1.0` |
| `D46` | `VA` | `1.0` |
| `C97` | `s⁻¹` | `1.0` |
| `C94` | `min⁻¹` | `0.016666666666666666` |
| `M46` | `rev/min` | `0.10471975511965977` |

The last five rows are the ones §9.6 is about: `GRY`/`D13`, `NU`/`JOU` and `D44`/`D46` are pairs
that are equal in SI and different in meaning, and `C94`/`M46` is the pair that was actually wrong.

---

## 12. The QUDT profiles

> `qudt:` — QUDT unit local names (2244 mapped, 564 unsupported — every one of the 2803 local
> names QUDT defines, plus five refusals for names it does not: `USD`, `EUR` and three `DECIBEL_*`).
> `qudt-qk:` — QUDT quantity kinds (908 mapped, 257 unsupported — every one of the 1164 kinds,
> plus a refusal for `LogarithmicRatio`, which QUDT 3.1.0 does not define).
> Data files `src/gendata/qudt.bvnr` and `src/gendata/qudt-qk.bvnr`;
> pinned by `tests/bovnar_qudt_test.c` (216 assertions).

### 12.1 Why this vocabulary

QUDT is the unit ontology of the digital-twin and building-semantics world — DTDL, Brick,
ASHRAE 223P, IOF. It is also the only one of the five that supplies stable IRIs, which is precisely
what Bovnar does not have and does not need inside a document.

### 12.2 Local names, not IRIs

QUDT units are IRIs under `http://qudt.org/vocab/unit/`. This profile spells them by their **local
name** — the part after the last `/` — and rejects the full IRI:

```bovnar
#!bovnar 1.2
.v = <float:64,qudt:M-PER-SEC> 9.81;
.m = <float:64,qudt:KiloGM>    72.5;
.d = <uint:64,qudt:MebiBYTE>   16;
```

`qudt:http://qudt.org/vocab/unit/M` is `error_unit_illegal`. A unit slot is at most 255 bytes and a
`:` inside it already means something here; admitting a URI would put a second scheme separator
inside a namespace separator for no gain, since the local name is the identifying part.

Flat, for the same reason UNECE is (§11.2), and here the temptation is sharper because QUDT's naming
*looks* decomposable: `KiloGM` really is kilo + GM. It is not taken apart, because the naming is
regular enough to tempt a parser and irregular enough that the parser would be wrong — `MI` is the
mile, not a milli-anything.

**And the local name is the identifier, not the symbol.** QUDT's `PCA` carries the symbol `pc`, and
it is the *pica*, a typographic length; the parsec is `PARSEC`. A table built by matching symbols
rather than local names gets a length wrong by nineteen orders of magnitude, which is why the
`bvnr_profile_factors` gate (§9.5) resolves every row against QUDT's own definitions. (`qudt:PCA`
itself is not carried — the pica has no native form — so it is `error_unit_illegal`; the point is
what a symbol-matched table would have *done* with it.)

`MebiBYTE` is the one shape where a flat code names a **binary**-prefixed unit. Since a binary prefix
has no decade, the reverse table carries the IEC prefix identity alongside the decimal decade, and
an expression profile — which reaches a scale by emitting a decimal prefix, and there is none
meaning 2²⁰ — simply has no spelling for such a unit (§5.3).

Eight local names are refused rather than mapped: `UNITLESS` and `NUM` (the absence of a unit, which
Bovnar spells by omitting the slot), `MO` (QUDT's synodic month of 29.53 days, three per cent from
the Julian month native `mo` is), `USD` and `EUR` (currencies, which Bovnar carries natively with
the `$` sigil and minor-unit metadata), and `DECIBEL_M`/`DECIBEL_W`/`DECIBEL_V` (a reference level
`dB` cannot carry, §3.7).

### 12.3 Quantity kinds (`qudt-qk:`)

**A quantity kind is not a unit.** `Length` says what is being measured and nothing about the scale.
This namespace therefore has to decide what `qudt-qk:Length` means in a slot whose whole job is to
fix the scale, and there are only three honest answers:

1. **Refuse it.** Correct, and useless: it makes the namespace an error message.
2. **Carry the dimension without a scale.** Not representable. A `value_unit_t` is a product of
   prefixed base units; there is no way to spell "a length, scale unspecified" in one, and adding a
   fourth state to the unit model would break §1.3, the one thing that must not change.
3. **Translate to the coherent SI unit of the kind.** `Length` → `m`, `Mass` → `k~g`,
   `Velocity` → `m/s`. **This is what the profile does.**

Option 3 is not this table guessing. QUDT relates a `QuantityKind` to exactly one coherent SI unit,
and ISO 80000 defines a kind of quantity together with the coherent unit of its system. Writing
`<float:64,qudt-qk:Length> 3.0` means three metres because the coherent SI unit of length *is* the
metre. §9.5 turns that into a build gate: every mapped kind is checked to have a native factor of
exactly 1, so a kind mapped to a non-coherent unit fails even though its dimensions are perfect.

**Where it can still bite.** A producer who knows only the kind, and whose numbers are not in
coherent SI units, will write a wrong value that parses cleanly. If your lengths are in feet,
`qudt-qk:Length` is the wrong thing to write and `qudt:FT` is the right one. This namespace is for a
producer whose data is already coherent-SI and whose metadata carries a kind rather than a unit —
the common shape of a QUDT- or DTDL-sourced feed, and the only reason the namespace exists.

Two consequences follow from translating to a unit, and both are honest rather than accidental:

- Kinds that share a dimension share a unit. `qudt-qk:Energy` and `qudt-qk:Work` compare **equal**,
  as do `Speed` and `Velocity`, `Voltage` and `ElectricPotential`, `Force` and `Weight`. Bovnar's
  dimension vector cannot tell them apart, and neither can ISO 80000.
- A kind whose coherent unit Bovnar cannot state is **refused**, not approximated:
  `CelsiusTemperature` (an affine scale, not a coherent unit — write `°C`, or `K` for
  `ThermodynamicTemperature`), `LogarithmicRatio` (no reference level), `Dimensionless`,
  `DimensionlessRatio` and `Count` (the *absence* of a unit, which Bovnar spells by omitting the
  slot), `InformationEntropy` (QUDT's coherent unit there is the nat, §9.5), and `Currency`.
  Refusing costs an error message; approximating costs a number.

`qudt-qk` is also why a namespace may contain a hyphen (§2.1). It may not lead: `-qk:Mass` is not a
namespace and falls through to the native parser, which rejects it as it always did.

### 12.4 The quantity-kind table: the ISO 80000 core

The table is no longer short enough to print — 908 of QUDT's 1164 kinds map. What follows is the
ISO 80000 core it started as, which is still the part worth reading, and every row in it was
verified to have a native coherent-SI factor of exactly `1.0`, the claim §12.3 makes. For the rest,
read `src/gendata/qudt-qk.bvnr`.

**How the other 858 were chosen, and why not by dimension.** A kind states no multiplier, so there
is nothing to match on but the dimension vector — and dimensions cannot tell `Torque` from `Work`.
Both are `[2,1,-2]` at factor `1`, and answering "the coherent SI unit of this kind" with `J` for
both would be numerically perfect and would tell a reader of a torque that they had an energy. So
the coherent unit is taken from QUDT's own `qudt:applicableUnit` list instead: the member whose
`conversionMultiplier` is exactly `1` **is** the coherent unit of that kind, and QUDT lists `N-M`
under `Torque` and `J` under `Work`. That unit's `qudt:ucumCode` is then carried to native through
the UCUM table, so a quantity-kind row and a unit row cannot disagree.

**The 254 that are refused are almost all dimensionless**, and refused for a reason no amount of
work would remove: every native ratio has the same dimension vector as every other, so `Absorptance`,
`MassFraction` and `ActivityCoefficient` are each worth exactly what `%`, `ppm`, the radian and the
bit are worth. Nothing in this repository can say which one QUDT means, and QUDT states no
`ucumCode` for them to settle it. Guessing would put a turbidity reading and a percentage in the
same equivalence class.

| QUDT quantity kind | Bovnar | QUDT quantity kind | Bovnar |
|---|---|---|---|
| `Length` | `m` | `ElectricCharge` | `C` |
| `Mass` | `k~g` | `Voltage` | `V` |
| `Time` | `s` | `ElectricPotential` | `V` |
| `ElectricCurrent` | `A` | `Resistance` | `Ω` |
| `ThermodynamicTemperature` | `K` | `ElectricalResistance` | `Ω` |
| `AmountOfSubstance` | `mol` | `Capacitance` | `F` |
| `LuminousIntensity` | `cd` | `Inductance` | `H` |
| `Area` | `m²` | `Conductance` | `S` |
| `Volume` | `m³` | `MagneticFlux` | `Wb` |
| `Angle` | `rad` | `MagneticFluxDensity` | `T` |
| `PlaneAngle` | `rad` | `LuminousFlux` | `lm` |
| `SolidAngle` | `sr` | `Illuminance` | `lx` |
| `Velocity` | `m/s` | `Activity` | `Bq` |
| `Speed` | `m/s` | `AbsorbedDose` | `Gy` |
| `Acceleration` | `m/s²` | `DoseEquivalent` | `Sv` |
| `AngularVelocity` | `rad/s` | `CatalyticActivity` | `kat` |
| `Frequency` | `Hz` | `Force` | `N` |
| `VolumeFlowRate` | `m³/s` | `Weight` | `N` |
| `MassFlowRate` | `k~g/s` | `Pressure` | `Pa` |
| `Density` | `k~g/m³` | `Stress` | `Pa` |
| `MassDensity` | `k~g/m³` | `Energy` | `J` |
| `Momentum` | `k~g·m/s` | `Work` | `J` |
| `DynamicViscosity` | `Pa·s` | `Power` | `W` |
| `KinematicViscosity` | `m²/s` | `Torque` | `N·m` |
| `AmountOfSubstanceConcentration` | `mol/m³` | `HeatCapacity` | `J/K` |
| `SpecificHeatCapacity` | `J/k~g·K` | `ThermalConductivity` | `W/m·K` |

---

## 13. The UDUNITS profile

> `udunits:` — UDUNITS-2, Unidata's unit library, whose string grammar is the de-facto units syntax
> of netCDF and the CF conventions.
> Data file `src/gendata/udunits.bvnr` (41 prefixes, 768 mapped, 98 unsupported — every one of
> the 580 spellings the UDUNITS-2 database defines);
> pinned by `tests/bovnar_udunits_test.c` (224 assertions).

### 13.1 An expression profile, sharing the UCUM parser

UDUNITS is the second **expression** profile, and it runs through the same translator as UCUM with
two syntax differences configured on its registry row:

- `*` multiplies beside `.`;
- `^` introduces an exponent, beside the bare trailing digits both vocabularies allow.

A third difference runs the other way: UDUNITS has no `{…}` annotations, so `udunits:mL{x}` is
`error_unit_illegal` (§3.4).

The **division rule is identical**, which is worth stating because it is where a units parser most
often goes quietly wrong. In both vocabularies `/` inverts the term that follows it and nothing
else — ordinary left-to-right arithmetic. `kg/m*s` is `(kg/m)*s`, so the second term is positive;
`kg/m/s` is `kg·m⁻¹·s⁻¹`. The suite pins `udunits:kg/m*s` equal to `ucum:kg/m.s` — both formatting
back as `k~g·s/m` — for exactly this reason: getting it backwards turns a viscosity into its
reciprocal and still formats cleanly.

UDUNITS accepts symbols and spelled-out names for units *and* prefixes, which is why the table
carries 765 codes and 41 prefix spellings for a vocabulary no larger than UCUM's: `m`, `meter` and
`metre` are three rows, and `kilo` is a prefix beside `k`. `udunits:km`, `udunits:kilometer` and
`udunits:kilometre` are one unit.

```
udunits:m*s-1       →  m/s
udunits:kg*m-2*s-1  →  k~g/m²·s
udunits:m^2         →  m²
udunits:hPa         →  h~Pa
udunits:nit         →  cd/m²
```

### 13.2 Space multiplies, and now it can

UDUNITS multiplies with a space, and CF's commonest spelling of a flux is `kg m-2 s-1`. **Bovnar
accepts it in a type annotation**, and `' '` is in this profile's multiplication set beside `.` and
`*`.

```
udunits:kg m-2 s-1   →  k~g/m²·s     the same unit as udunits:kg*m-2*s-1
udunits:m s-1        →  m/s          a speed
udunits:ms-1         →  m~s⁻¹        a reciprocal MILLISECOND — also valid UDUNITS,
                                     and a different unit. The space is what
                                     tells the two apart
udunits:kg  m-2      →  error_unit_illegal   two operators in a row
```

**This section used to say the opposite, and the reason it could is the reason it no longer has to.**
While the lexer *deleted* whitespace inside a type annotation, the slot `udunits:m s-1` arrived here
as `udunits:ms-1` — a perfectly good UDUNITS expression meaning reciprocal milliseconds — so listing
space as an operator would not have made the space-separated form work. It would only have made the
wrong reading of it look supported. §10.2 recorded that as the one place in the format where a wrong
unit was produced silently rather than refused.

Closing that hole is what made this possible. Whitespace beside a parameter separator is still
ignorable, whitespace inside a *native* parameter is `error_type_param_whitespace`, and whitespace
inside a **namespaced** parameter is carried in verbatim for the vocabulary to interpret (spec §5.3).
So the space now reaches this parser as a space, and means here what it means in UDUNITS.

Three limits worth stating plainly:

- **A run of spaces is not collapsed.** `kg  m-2` is two multiplications in a row and fails as
  `error_unit_illegal`. Collapsing runs in the lexer would have cost a UCUM annotation the spacing
  §3.4 promises to keep verbatim, and a malformed expression getting the vocabulary's own error is
  the right outcome anyway.
- **The inline unit form cannot carry a space and never will.** Whitespace is what *terminates* that
  token, so `1.0 udunits:kg m-2` is a value, a unit and a stray token. The space-separated spelling
  is available in a type annotation only.
- **A line break is still an error.** No vocabulary spells a unit across a line.

`udunits:ms-1` is still pinned to `m~s⁻¹` explicitly, and §14.2 still pins `udunits:ms-1` against
`udunits:m*s-1` as a pair that must not compare equal — that pair matters *more* now, not less,
because the space is the only thing distinguishing the speed from the reciprocal millisecond.
Conformance cases UPR-043c…UPR-043f pin the whole boundary, including the annotation/inline agreement
between `udunits:kg m-2 s-1` and `udunits:kg*m-2*s-1`, which passes only if both spellings produce
the same `value_unit_t`.

### 13.3 The near misses: codes that name a native unit and are not it

UDUNITS is the profile with the most spellings that look like a native unit, carry the same
dimension, and differ only in factor. A factor-only error is the one shape nothing downstream can
catch, so none of them may be mapped onto the native unit whose name they borrow — the same
reasoning §6.3 applies to the BTU and the apothecary dram.

They were refused on that account for as long as there was nothing else to send them to. **The
registry now carries the units themselves**, so all but one map to a unit of their own and the
near-miss is gone rather than merely fenced off:

| UDUNITS code | Is | Native unit of that name | Apart by | Now |
|---|---|---|---|---|
| `year`, `yr` | tropical year, `3.15569259747e7` s | `yr`, the Julian year `31557600` s | 674 s — 11 min/yr | → `yr_trop` |
| `month` | a twelfth of the tropical year | `mo`, a twelfth of the Julian year | as above | **refused** |
| `calorie`, `cal`, `IT_calorie` | IT calorie, `4.1868` J | `cal`, thermochemical `4.184` J | 0.067 % | → `cal_IT` |
| `chain`, `rod`, `furlong`, `fathom` | built on the US **survey** foot | international, on the `0.3048` m foot | 2 ppm | → `chUS`, `rdUS`, `furUS`, `fathUS` |
| `acre` | US survey acre, `160` survey rod² | `ac`, international `4046.8564224` m² | 4 ppm | → `acUS` |
| `shake` | `1e-8` s | — | no SI prefix for 10⁻⁸ (§3.5) | → `shake` |

`month` is the one left, and it is left for a reason the others do not share: a twelfth of the
tropical year is not a unit anybody publishes under a name of its own, so there is nothing to add to
the registry that would not be an invention. It stays `error_unit_profile_unsupported`, named.

The spelled-out forms UDUNITS gives for what Bovnar actually means still map, and they remain the
unambiguous way to write these: `Julian_year` → `yr` and `thermochemical_calorie` → `cal`, both
exactly, beside `year` → `yr_trop` and `calorie` → `cal_IT`. The BTU
runs the other way from UCUM: UDUNITS' unqualified `Btu` is the IT BTU, which is exactly what native
`Btu` is, so it maps here although UCUM's unqualified `[Btu]` cannot (§6.3).

Two spelling traps in the same family, neither of them factor errors:

- **`oz` is a volume.** In UDUNITS it is a symbol of the US *fluid* ounce, not the avoirdupois
  ounce, so it translates to `fl_oz`. The mass is spelled `avoirdupois_ounce`. Reading `udunits:oz`
  as a mass was a dimension error, and the two are now not even compatible.
- **`b` is the barn**, not the bit — the collision §6.2 records for UCUM, present here for the same
  reason and resolved the same way. `bit` is the bit and `byte` the byte.

`unified_atomic_mass_unit`, UDUNITS' fourth spelling of the dalton, used to be absent for a reason
worth recording because it has now moved twice. At 24 bytes it pushed this profile's worst-case
emitted string past `BVNR_UNIT_STRING_MAX` and `gen_profiles.py` refused to generate the table
(§9.2); `thermochemical_calorie` was excluded the same way when that constant was 192. Closing the
table against the UDUNITS database brought a longer code still into view —
`astronomical_unit_BIPM_2006`, 27 bytes — and the constant went from 1024 to 1088 on its account.
All four dalton spellings map.

That row was then found **unreachable**, and is refused with a reason that has nothing to do with
its length: this is an *expression* profile, so a code ending in digits cannot be read at all — the
`2006` scans as an exponent (§3.3). Write `au`, `ua` or `astronomical_unit`. The constant has since
stopped depending on it either way: `BVNR_UNIT_STRING_MAX` is recomputed from `src/gendata` by both
generators on every build, and the header deliberately names no symbol as the extreme, because
whichever one it named would be overtaken by the next catalogue growth and the comment would outlive
the fact.

**The one thing an incomplete atom table costs that an error message cannot.** UDUNITS spells the
pint `pt`, and that spelling was not in this table. It did not therefore fail: with no atom row the
expression parser fell back on a *prefixed* reading, `p` + `t`, and `udunits:pt` parsed clean as a
**picotonne**. A document saying 473 mL came back as `1e-9` kg — a mass where a volume was written,
wrong by nine orders of magnitude, in the wrong dimension, with no diagnostic anywhere.

This is the failure mode that makes closing a table worth more than tidiness. Every other missing
code cost a producer a wrong error message; this one cost them a wrong number, and it was reachable
from a perfectly ordinary CF file. A whole atom outranks a prefixed reading (§3.2), so the row is
the entire fix — but nothing would have found it except enumerating the publisher's own list and
asking, for each spelling, not "is there a row" but "what does this parse to". `check_profile_factors.py`
now asks exactly that (§9.5), which is what turns this from a bug that was found once into a class
of bug the build checks for.

The refusals that are not near misses at all are CF constructs that are not units: `count`, and the
coordinate direction markers — `degrees_north`, `degrees_east` and every other spelling UDUNITS
gives them, `degreeN` through `degrees_true`, twenty in all. Before the table was closed the first
six were refused with a reason and their fourteen siblings came back as `error_unit_illegal`, so the
same concept produced two different errors depending on how it was spelled. A direction is a coordinate-system statement; write `°` and record
the direction in a sibling field. Note that UDUNITS' `1`, which CF uses for a dimensionless
variable, needs no row: the shared expression parser resolves a bare power of ten to unity before
any table is consulted, so `udunits:1` yields no unit exactly as `ucum:1` does.

### 13.4 Reference time is refused, and why

`<unit> since <timestamp>` is UDUNITS' most distinctive construct and the one most often asked for.
It is **refused**, as `error_unit_profile_unsupported`, for a structural reason rather than a
scheduling one.

A Bovnar timestamp is `<datetime:width,epoch>`. Two facts make the translation impossible as the
type system stands:

1. **The epoch is not a unit.** It lives in `value_type_spec_t.base` — the numeric-base parameter
   slot — as a small dense index. A unit-slot expression cannot reach it, and cannot select the
   `datetime` family either; the family comes from the annotation's first parameter.
2. **The carrier is defined as seconds.** `vt_datetime` is a signed integer count of *seconds*
   since the epoch. `days since 1970-01-01` would need the carrier rescaled, which no unit slot
   can do.

Either one alone would block it; both together mean this is a type-system change, not a unit
question. Implementing it would require `bvn_parse_unit` to return a family-and-epoch override
alongside the unit, a precedence rule for when an explicit `<datetime:64,unix>` disagrees with a
profile-implied one, and a carrier-scaling rule the datetime family does not currently have.

The refusal is driven by a `refuse_substr` hook on the registry row — any `udunits:` code containing
`since` is refused — rather than by an atom lookup, because by the time the parser sees it the
whitespace is gone and `days since 1970-01-01` is one unsplittable token. A row for `since` is kept
in `.unsupported` anyway, so that a bare `udunits:since` refuses identically and so that a
maintainer finds the reason where they look for it. Refusing costs a producer an error message that
says exactly this; accepting it would cost them a silently wrong instant.

---

## 14. The cross-vocabulary conformance suite

> `tests/bovnar_crossvocab_test.c` — 64 concepts, 6 vocabularies, 4789 assertions.

Every other profile test asks *does this vocabulary translate correctly?*. This one asks the
question that only exists once there are five: **do they agree?**

If any two disagree, the format's central promise fails — a document written by a UCUM-speaking
producer and read by a UNECE-speaking consumer would carry a unit that compares unequal to itself.

### 14.1 A concept table, checked pairwise

Each row is one physical concept and the way each vocabulary spells it:

```c
{ "kilogram", { "k~g", "kg", "ucum:kg", "unece:KGM", "qudt:KiloGM",
                "udunits:kg", "qudt-qk:Mass", NULL } },
```

Every spelling is checked against **every other** spelling in its row, not against a designated
reference. A reference-based check would pass if two non-reference spellings agreed with the
reference for different reasons; the pairwise form cannot. Each pair is checked for:

1. that both parse at all;
2. `bvn_unit_equal` — the same components, prefixes and exponents;
3. agreement of the coherent-SI factor, which pins the absolute scale;
4. dimensional compatibility;
5. round-trip: the canonical spelling re-parses to the same unit.

### 14.2 The negative half is not optional

A suite that only checked agreement would be satisfied by a translator that mapped everything onto
the metre. A second table pins the pairs that look interchangeable across vocabularies and are not:

| Pair | Why they must differ |
|---|---|
| `ucum:kg` vs `qudt:GM` | the kilogram is not the gram — the one SI base unit with a prefix in its name, and where a "tidied" table goes wrong by 10³ |
| `ucum:st` vs `unece:STI` | UCUM's `st` is the **stere**, a cubic metre; UNECE's `STI` is the stone |
| `qudt:MI` vs `qudt:MilliM` | QUDT's `MI` is the mile — a flat local name never decomposes |
| `udunits:ms-1` vs `udunits:m*s-1` | reciprocal milliseconds is not metres per second (§13.2) |
| `ucum:B` vs `qudt:BYTE` | UCUM's `B` is the **bel**; UCUM writes the byte `By` |
| `qudt:KiloBYTE` vs `qudt:KibiBYTE` | 10³ bytes is not 2¹⁰ bytes |
| `unece:MTS` vs `unece:MTK` | metre per second against square metre — two Rec 20 codes one letter apart |
| `unece:C94` vs `unece:M46` | the reciprocal minute against the revolution per minute — 2π apart, and held apart by the angle kind (§9.6) |

### 14.3 What it found, and what it cannot tell you

On its first run the suite failed on `ucum:A`: **the ampere was missing from the UCUM table**. One
of the seven SI base units had no UCUM spelling, and no single-vocabulary test had asked, because
each was written against the table it was testing. Asking every unit vocabulary for the same seven
concepts found it immediately. That is the argument for the suite in one line.

**What it cannot find is a concept nobody wrote a row for**, and that is the limit worth stating,
because it is the one this table keeps running into. A vocabulary left out of a row is
indistinguishable here from a vocabulary that has no code — "UNECE has no code for the katal" reads
exactly like
"nobody added `unece:KAT`", and for the tesla, the sievert, the katal, the radian, the steradian and
the newton metre it was the second. Each had been carried in `unece.bvnr` for a whole release and
omitted from the row that would have checked it. §9.6's coverage index is what finds *that* class;
this suite is what pins it once found. The two are not alternatives.

What it also cannot tell you: it proves the six unit vocabularies agree **with each other**, not that
they agree with their publishers — that is §9.5's job, and §9.5 reaches `unece` only at one remove
and reaches `CEL`, `FAH`, `GRY`, `HEN`, `MOL` and `TNE` not at all. **Five tables that are wrong in
the same way agree perfectly.** §10.2 is where what remains uncovered is recorded.

---

## 15. Closing the tables

### 15.1 What was wrong

The tables were **confident subsets**, and §11.1 argued for that: a code not in the table fails
loudly, which costs a producer an error message, while a code in it and wrong costs them a wrong
number. That argument is sound and it justified not *mapping* a code. It never justified what
actually happened to one.

`ucum.bvnr`'s own header states the contract:

> `.unsupported` — Listed so that it fails as `error_unit_profile_unsupported` ("you wrote it right
> and we cannot carry it") rather than `error_unit_illegal` ("that is not a UCUM atom").

An atom in none of the three lists is `error_unit_illegal`. Measured against the publishers, that
was being said about 83 of UCUM's 312 atoms, 299 of UDUNITS' 570 spellings, and some 3600 QUDT
names — a false statement to a conforming producer, indistinguishable from what they get for
`udunits:zzzq`. The refusal list exists precisely to prevent that, and a `.unsupported` row needs no
conversion factor, so closing this gap carried none of the risk the conservative default was
protecting against.

Two consequences were worse than a wrong error message:

- **`udunits:pt` was a picotonne.** §13.3 has the detail. An expression profile falls back on a
  prefixed reading when no atom matches, so a missing atom is not always an error — sometimes it is
  a different unit, silently.
- **The same concept gave two different errors.** `degrees_north` was refused with a reason and its
  fourteen sibling spellings were called illegal.

### 15.2 How the rows were produced

Nothing here was matched on numbers alone, because numbers cannot tell a sievert from a gray. In
order of preference:

1. **The publisher's own definition, transliterated.** UCUM defines `[PRU]` as `mm[Hg].s/ml`; each
   atom is replaced by whatever this table already maps it to, giving `mmHg·s/m~L`. The row is then
   one a reader can check against the publication rather than one that merely computes the same.
2. **The publisher's own cross-reference.** QUDT publishes `qudt:ucumCode` and `qudt:udunitsCode`,
   so a QUDT local name is carried to native through the UCUM table this repository has already
   proved against `ucum-essence.xml` — which is also why §14's suite cannot find these two tables
   disagreeing. UNECE goes through QUDT the same way (§11.1).
3. **A value match, fenced.** Only for a code with a **non-zero** dimension vector. Every
   dimensionless native unit shares one dimension vector, so "worth 1 and dimensionless" matches the
   bit, the radian, the turbidity units and every ratio alike; unfenced it proposed the
   nephelometric turbidity unit as a *bit*, parts-per-trillion as a *picoradian*, and QUDT's
   `GeneralizedMomentum` as an NTU.

**Every proposal was then verified through the reference library** — same factor, same dimension
vector, same tolerance as §9.5 — and anything that did not verify became a `.unsupported` row
instead. `check_profile_factors.py` re-proves all of it from the publishers' files on every run. For the
five tables this section closed that was 4635 rows against 0 mismatches; §16 and §17 have since
added two more vocabularies, and [§9.5](#95-the-factor-proof) carries the current total across all
seven.

### 15.3 What it cost, and what is still open

| | Before | After that pass |
|---|---|---|
| `ucum` | 141 mapped, 32 opaque, 56 refused | 157, 41, 114 — **all 312** |
| `udunits` | 251 mapped, 32 refused | 404, 180 — **all 570** |
| `qudt` | 263 mapped, 8 refused | 2056, 752 — **all 2803** |
| `qudt-qk` | 52 mapped, 7 refused | 910, 255 — **all 1164** |
| `unece` | 252 mapped, 7 refused | 1195, 257 — all QUDT's cross-reference reaches |
| rows `check_profile_factors.py` compares | 899 | 4635 |
| coverage suggestions outstanding | 1161 | 0 |
| `bovnar` binary | 617 KB | 1015 KB |

**Both columns are a record of that pass, not of today.** The tables have grown again since, as the
registry gained units that unblocked codes previously refused for want of a target — the survey-foot
series, the IT calorie, the tropical year. [§9.1](#91-where-the-tables-live) carries the current
figures and is checked against `src/gendata` on every build; this table is left as it was written,
because the point of it is what one pass cost.

**The binary is 65 % larger**, and that was the real price. It is table data — codes, targets and
refusal strings — and the refusal reasons are written as shared literals per family so the compiler
pools them, but a build that wanted only one vocabulary still paid for five.

**The per-profile build switch called for here now exists** (§9.4): seven `BVNR_WITH_<NAME>_PROFILE`
options, all on by default, and a build with every one off takes `libbvnr.so` from 1.96 MB to 524 KB.
The paragraph above is what it answers, and the id-space blocking mentioned two paragraphs down is
what made it cheap — because each vocabulary owns its own block, dropping one moves no other
vocabulary's ids, so the switches change what a build can *translate* and nothing about its ABI.

`BVNR_UNIT_STRING_MAX` went from 1024 to 1088 to admit `astronomical_unit_BIPM_2006` (§13.3), and
the UCUM block from 32 to 41 arbitrary atoms — which, with a block per profile, moved no other vocabulary's ids.

**What closing the tables did not do.** It did not make the rows better evidenced than §9.5 can
make them — the "what a code MEANS" gap in §10.2 is untouched, and it is now spread over four times
as many rows. It did not close `unece`, which has no publication to close against. It did not add a
single native unit: the British imperial series, the pre-metric French lengths, the typographic
points and picas, the water column and the CGS electrostatic units are all refused *by name* now
rather than *by omission*, which is an improvement in the error message and not in the coverage.
Growing the native registry to reach them is a separate decision, and doc/05 is where it belongs.

---

## 16. The OM 2 profile

> `om:` — OM 2, the Ontology of units of Measure (Rijgersberg, van Assem, Top; Wageningen), the
> second unit ontology of the semantic web beside QUDT.
> Data file `src/gendata/om.bvnr` (1336 mapped, 124 refused — every unit individual OM states);
> pinned by `tests/bovnar_om_test.c` (110 assertions); id block 70, contributing no opaque units.

### 16.1 Why this vocabulary

QUDT is the vocabulary of digital twins and building semantics; OM is the one agrifood, food
science, life-cycle assessment and FAIR/ELN data reach for, and it is what Wikidata's unit items
most often align to. A consumer reading either ontology's data had exactly one of the two namespaces
before this, and the missing half is not a small dialect of the first: OM's local names are English
words (`kilogramPerCubicmetre`), QUDT's are abbreviations (`KiloGM-PER-M3`), and nothing maps one to
the other without a table.

It is a **flat** profile for the same reason QUDT is (§12.2): `kilogramPerCubicmetre` is matched
entire. OM's names are regular enough to tempt a parser into decomposing them and irregular enough
that the parser would be wrong — OM writes the cubic metre in a denominator as `Cubicmetre` and in
`kilogramPerCubicDecimetre` as `CubicDecimetre`, and a decomposer would have to decide which of
those is the typo. Neither is: they are both the publisher's spelling, and this table follows the
publisher.

### 16.2 The targets were derived from OM's own structure

**This is the part that is not like QUDT.** QUDT states a `conversionMultiplier` per unit, so its
table is a value read. OM states no multiplier at all. It states how each unit is *built*:

| OM class | states | example |
|---|---|---|
| `PrefixedUnit` | `hasPrefix`, `hasUnit` | `kilogram` = kilo × `gram` |
| `UnitDivision` | `hasNumerator`, `hasDenominator` | `metrePerSecond-Time` |
| `UnitMultiplication` | `hasTerm1`, `hasTerm2` | `newtonMetre` |
| `UnitExponentiation` | `hasBase`, `hasExponent` | `squareMetre` |
| `SingularUnit` | `hasFactor`, `hasUnit` | `inch-International` = 0.0254 × `metre` |
| a named unit | `hasUnit`, no factor | `joule` **is** `newtonMetre` |

So every compound row's `.bovnar` target was **built from that composition, bottom-up**, out of the
atoms mapped by hand — not read off the local name. `gramPerPetalitre` is `g/P~L` because OM states
a numerator, a denominator and a prefix; a table built by reading names in English would have agreed
on most rows and disagreed silently on the rest, which is the failure mode this whole document is
organised against.

Two consequences worth stating, because both look like bugs until you know where they come from:

- **The named SI units come out as themselves.** OM defines the joule as the newton metre and the
  becquerel as the reciprocal second, so a derivation that followed `hasUnit` blindly would spell
  `om:joule` as `N·m`. The atoms are mapped first, so it does not.
- **A prefix on a compound lands on one component.** OM's `attomolar` is atto × `molar`, and `molar`
  is the mole per litre; the prefix multiplies the whole unit, so it may be attached to any
  component standing at exponent 1. The target is `a~mol/L`, worth 10⁻¹⁵ either way.

**Where the composition could not be followed, a fenced value match finished the job** — §15.2's
third preference, and used the same way here. OM builds the hectare as hecto × `are` and bovnar has
no `are`; it builds the micron as a factor on the metre and bovnar spells that `µ~m`, which is not
an atom of any table. In both cases the unit *as a whole* is worth exactly one native (optionally
prefixed) unit, and that match is taken only after the structural attempt fails, only when the
dimensions agree, and only when the native unit it lands on is itself near-coherent. Without that
last fence, 10⁵ metres — OM's `_100Kilometre`, for which no SI prefix exists — matched the
**petaångström**, a spelling that is worth the right number and that nobody would write. It is
refused instead.

The same walk is implemented independently in `check_profile_factors.py` (`class Om`), which
resolves every local name against `om-2.0.rdf` and compares it with what the library says the target
is worth: **1271 rows compared, no mismatch**. The three shapes of resolver this brings the tool to
— expression, table read, composition — are listed in its header.

### 16.3 What the derivation refuses

124 local names are carried as refusals rather than mappings, and four groups are worth naming:

- **`om:year`, and everything built on it.** OM's year is the **Gregorian** 31 556 952 s; native
  `yr` is the Julian 31 557 600 s. Twenty-one parts per million apart, dimensionally identical, and
  invisible to every test that does not check the number — the same shape as UDUNITS' tropical year
  (§13.3). `gigayear`, `reciprocalYear` and `cubicMetrePerYear` fall with it.
- **Units OM names without stating a magnitude.** `calorie-15C`, `BritishThermalUnit-Mean`,
  `colonyFormingUnit` and `bel` have a dimension or a symbol and no factor. A unit whose value the
  publisher does not state is not one this table can carry, whatever a reader might guess it to be.
- **The arbitrary units.** `om:InternationalUnit` is UCUM's `[IU]`, which bovnar already carries as
  an opaque base unit with an identity of its own (§7.1). A second, OM-flavoured identity for the
  same assay unit would make the two **incommensurable with each other** — two units that cannot be
  compared although they are the same unit. This is why block 70 contributes no opaque ids.
- **Money, scales and dimension one.** OM models currencies as units (`om:euro`); bovnar carries
  currencies in a system of their own (§7.2). `KelvinScale`, `CelsiusScale` and the other three
  `…Scale` individuals are scales, not units: OM relates each to its unit by a factor *and an
  offset*, so mapping one to a unit would read a Celsius-scale 20 as 20 K.
  `om:one`, `om:metrePerMetre` and `om:dozen` are dimension one, which bovnar spells by omitting the
  slot — translating them would make a count compare equal to a percentage.

Each is `error_unit_profile_unsupported`, not `error_unit_illegal`: the producer wrote a real OM
unit and is told bovnar cannot carry it, which is the distinction §3.1 exists for.

### 16.4 Which name a unit is written back as

`om:` is written as well as read, and several local names are worth the same native unit, so one of
them has to keep the reverse spelling and the rest carry `.reverse = false`. **Shape decides it
before length does**, which is a rule this vocabulary needed and the others did not:

| native unit | candidates | written back as | why |
|---|---|---|---|
| `da~A` | `abampere`, `biot`, `decaampere` | `decaampere` | the target carries a prefix, so the name that OM builds with a prefix wins over the shorter, stranger one |
| `m` | `metre`, `cubicMetrePerSquareMetre` | `metre` | a composed name is worth a metre and is not a spelling of one |
| `mmHg` | `millimetreOfMercury` | `millimetreOfMercury` | the only candidate, and 2.9 ppm from OM's rounded column — waived by name in `check_profile_factors.py`, exactly as UCUM's `m[Hg]` and QUDT's `TORR` are |

791 native units — counted as (unit, decade) pairs, which is how a flat vocabulary names them — have an OM spelling as a result. A compound unit still has none: a flat vocabulary
spells one whole code, so `bvn_unit_to_profile("om", m/s²)` returns -1 for the reason §5.1 gives.

---

## 17. The CF standard-name profile

> `cf:` — the CF conventions' standard names, the controlled vocabulary of netCDF climate and
> forecast data.
> Data file `src/gendata/cf.bvnr` (4454 mapped, 617 refused — all 5071 names of standard name table
> **v94**, 2026-06-09); pinned by `tests/bovnar_cf_test.c` (46 assertions); id block 80, contributing
> no opaque units. **Read-only** (§17.3).

### 17.1 Why this vocabulary

`udunits:` already speaks the unit half of a CF file. The other half is the `standard_name`
attribute, and the two are not independent: CF makes `units` optional exactly when the standard name
fixes it, so a pipeline holding a name and no units string had nothing to write down. This namespace
is that case — and it is the same slot `qudt-qk:` fills for QUDT-sourced data, which is why it has
the same shape.

### 17.2 The unit is CF's own `canonical_units`

A standard name is **not a unit**. It names what is being measured, so this namespace has to answer
with a unit or refuse the name, and §12.3's three options apply unchanged. It answers with the unit
**CF itself states**: every entry in the table carries a `canonical_units` field — a UDUNITS
expression, because CF's unit syntax is UDUNITS — and that field, not a reading of the name, is what
each row translates.

```bovnar
#!bovnar 1.2
.t   = <float:64,cf:air_temperature> 288.15;      # K, because CF says "K"
.wind = <float:64,cf:northward_wind> -3.4;        # m/s, because CF says "m s-1"
.rain = <float:64,cf:rainfall_flux> 0.00012;      # kg/m²·s
```

5071 names state only **116 distinct** canonical-unit strings, and only those 116 were mapped by
hand; every row then takes the target its own `canonical_units` names. Two standard names that state
the same units therefore cannot drift apart — there is no per-name unit to get wrong — and
`cf:air_temperature` and `cf:sea_water_temperature` are the same `value_unit_t`.

**The bite is `qudt-qk:`'s bite.** A producer whose numbers are not in the canonical units writes a
wrong value that parses cleanly. If your temperatures are in degrees Celsius, `cf:air_temperature`
is the wrong thing to write and `udunits:degC` is the right one — which is what CF's own conventions
already require of the `units` attribute.

The check is unusually strong for a profile of this size, because **both sides are primary**:
`check_profile_factors.py` (`class Cf`) re-evaluates each name's `canonical_units` with the same
UDUNITS evaluator the `udunits` profile is checked against, and compares that against the library's
reading of the target. 4434 rows compared, no mismatch. It is not the one-remove position `unece`
is in (§11.1): CF publishes the name-to-units mapping and Unidata publishes what the units are
worth, and neither is being asked about the other's vocabulary.

### 17.3 Read-only, and why a namespace may be

`bvn_unit_to_profile("cf", u)` returns **-1 for every unit**, and this is the first namespace of
which that is true. It is not a gap in the table:

> Sixty-nine standard names state `K`. Writing a kelvin back as one of them would pick a quantity
> out of a hat and assert it — `cf:air_temperature` for a sea-surface temperature is not a
> formatting choice, it is a false statement.

A unit does not know what quantity it measures, so there is nothing this direction could honestly
return. `gen_profiles.py` therefore marks the profile **unwritable** and emits no reverse rows at
all, rather than singling one row out to be wrong; the C side reaches -1 through the ordinary "no
row names this base" path, with no special case in the writer. A `.reverse` flag in such a data file
is a build error, because it would be describing a choice nothing makes.

The refusal is this namespace's own. The same kelvin still writes as `qudt:K` and `om:kelvin`, and a
`cf:` unit is not "profile only" (§5.1): it translates to an ordinary native unit, and written from
that unit alone it comes back as one. A document that *spelled* it `cf:air_temperature` keeps that
spelling when it is re-serialised (§5.2) — which is the text being preserved, not a unit being
translated into a standard name.

### 17.4 What is absent, and what it costs

| absent | count | why |
|---|---|---|
| deprecated aliases | 599 | CF keeps retired names in the table pointing at their replacements. A document written today should carry the replacement, and an alias fails as `error_unit_illegal`, which says so. |
| `canonical_units` of `1` | 571 | dimension one — spelled in bovnar by omitting the slot. Translating it would put a cloud fraction, a percentage and a bit in one equivalence class. |
| mole fractions | 7 | dimension one under another name, and refused for the same reason. |
| string-valued names | 17 | `region`, `area_type`, `platform_name`: CF states no units because they label a variable rather than measure one. |
| `1e-3`, `1e-6` | 19 | a bare scale factor. CF gives `sea_water_salinity` `1e-3`, which is a magnitude with no unit at all. |
| `dBZ`, °C in a product | 3 | a logarithm against a reference bovnar cannot state, and an affine unit beside another component (§3.8). |

Those are the 617 rows of `.unsupported`; with 4454 mapped they are the 5071 the upstream table
defines. **Per-`year` quantities used to be on this list and are not any more**: the registry gained
`yr_trop`, so `age_of_sea_ice` and `sea_water_age_since_surface_contact` carry it and the two sea
level tendencies carry `m/yr_trop`. That is the same change §13.3 records for UDUNITS, reaching a
second vocabulary.

**The size, measured rather than estimated.** This is by far the largest table in the repository:
5071 codes averaging 54 bytes, against 2808 QUDT rows averaging 12. `BVNR_UNIT_STRING_MAX` did not
have to move — the longest name is 166 bytes and the cap is 1088 — but the binary did:

| | stripped release `bovnar` |
|---|---|
| after §15 closed the five tables | 1015 KB |
| with `om:` and `cf:` | **1798 KB** |

Three quarters of that is `cf`, and nearly all of `cf` is the names themselves: 502 KB of generated
atom rows against OM's 89 KB. This is what finally forced the per-profile build switch §15.3 had been
asking for — it is the difference between a 1 MB binary and a 1.8 MB one for a consumer who reads no
netCDF at all, and `-DBVNR_WITH_CF_PROFILE=OFF` is now how they decline it (§9.4). The decision
recorded here is that completeness won *inside* the vocabulary: a standard name absent from the table
is indistinguishable, to a producer, from one bovnar has never heard of, and a vocabulary carried in
part is a vocabulary whose absences have to be documented one by one. Whether to carry the vocabulary
at all is the integrator's, which is the right place for it.

---

## 18. Provenance, licensing and attribution

### 18.1 What is taken from whom

Seven profiles carry identifier strings from **six external vocabularies**, and those strings are
not this project's to license. The tables map them onto Bovnar's own registry; the mapping, the
targets and the refusal rationales are original work here, and the identifiers are not.

What is taken is **identifiers** — a UCUM atom code, a QUDT or OM local name, a UN/ECE common code,
a UDUNITS spelling, a CF standard name — and two kinds of short text beside them, both worth naming
rather than glossing over. The first is the **conventional name of a unit**, where a row would
otherwise be unreadable: a three-letter Rec 20 code is unintelligible without one, so most rows of
`unece.bvnr` carry `# statute mile` or `# US petroleum barrel`, the refusal strings in `ucum.bvnr`
do the same, and a few of those match UCUM's own `<name>` field word for word — `%[slope]` is
"percent of slope" in both, because that is what the unit is called. The second is CF's
`canonical_units`, which `cf.bvnr` carries on every row and translates into that row's target,
because §17.2 is built on it and it cannot be paraphrased away.

What is *not* taken is any upstream definition, description, annotation, property or explanatory
prose — nor any upstream conversion factor, since every target resolves through the native registry
and §9.5 then compares that against the publisher's value rather than importing it. Where a row
carries reasoning, that reasoning was written here. And no upstream artefact is redistributed at
all: §9.5's factor proof fetches the publishers' own machine-readable definitions into a git-ignored
build directory, verifies the tables against them, and packages none of them.

| Namespace | Vocabulary | Version | Licence |
|---|---|---|---|
| `ucum:` | UCUM | 2.2 (rev. 2024-06-17) | UCUM Copyright Notice and License v1.1 — **restrictive; see §18.3** |
| `qudt:`, `qudt-qk:` | QUDT | 3.1.0 | CC BY 4.0, attribution to QUDT.org |
| `om:` | OM 2 | 2.0 | CC BY 4.0, © Rijgersberg, Willems, Top / Wageningen UR |
| `udunits:` | UDUNITS-2 | master | BSD 3-clause, © UCAR / Unidata |
| `cf:` | CF standard name table | v94 (2026-06-09) | **none stated by the publisher; see §18.3** |
| `unece:` | UN/CEFACT Rec 20 and 21 | verified via QUDT's `uneceCommonCode` | UN/CEFACT IPR Policy; what is verified against QUDT is CC BY 4.0 |

The full notices — copyright lines, licence URIs, the BSD text verbatim, and the modification
statement CC BY 4.0 requires — live in `THIRD_PARTY_NOTICES.md` at the root of the distribution,
and each `src/gendata/*.bvnr` file repeats its own block in its header so that the version and the
licence travel with the data rather than only with the notices file. That file has a second part,
outside the scope of this document, inventorying everything else this project ships that it did not
author — the website's fonts and libraries, the imagery, the toolchain whose output is committed,
and whatever later review adds to it.

### 18.2 Every table is an adaptation, and says so

No table here reproduces a vocabulary, and none could: §3.1's three outcomes mean a code whose value
Bovnar cannot state exactly is refused rather than approximated, so every table is a subset by
construction, and §6.4 is the standing list of what each one leaves out. That is what makes these
adaptations rather than copies, and CC BY 4.0 requires an adaptation to be marked as one — which is
why the notices file states it for QUDT and OM in those words, and why the `.bvnr` headers do too.

It also has a consequence worth stating outside the licence text: **a publisher's endorsement is
never implied.** QUDT.org, the OM authors, UCAR/Unidata and the CF and UN/CEFACT communities have no
involvement in this project. Where a translation is wrong, it is wrong here — which is exactly what
§9.5 exists to catch, and §10.2 is the honest account of what it still cannot.

### 18.3 The two open questions

**Three of the six are settled.** QUDT and OM are CC BY 4.0 and need attribution, which they now
have; UDUNITS-2 is BSD 3-clause and needs its notice carried into binary distributions, which
`pack_artifacts.cmake` now does. **One is low-risk but unconfirmed:** UN/ECE is unresolved on the
face of the UN's own website terms and universally implemented in practice — Rec 20 exists to be
implemented — and written confirmation has been sought rather than assumed. **Two are genuinely
open**, and neither is open because nobody has got round to it.

**UCUM.** Its licence is revocable, and §3(a) conditions the grant on not creating derivative works
of the UCUM table and not adding to, deleting from or modifying its content. `ucum.bvnr` is a subset
mapped onto a different unit model, which is a derivative work on the licence's own definition
whatever its purpose. Two things follow, and both matter. First: Bovnar's native notation was
designed and specified independently of UCUM, is not offered as a replacement for it, and the
`ucum:` namespace exists precisely so that a producer holding a UCUM code need not translate it by
hand — §1.2 is the whole argument for a notation rather than more native units, and it is an
interoperability argument. Second: a UCUM code is never silently reinterpreted, because §3.1 leaves
nowhere for a guess to hide. Written permission for the derived mapping is being sought; until it is
granted, `-DBVNR_WITH_UCUM_PROFILE=OFF` (§9.4) drops the table from the build entirely.

**CF.** The conventions *document* is CC0 1.0, but the standard name table is maintained separately
and carries no licence file and no in-band rights statement. Nothing suggests the CF community means
it to be restricted — a standard name exists to be written into data files worldwide — but there is
no grant to point at, so the table is attributed to the CF community and CEDA and an explicit
licence declaration has been requested. `-DBVNR_WITH_CF_PROFILE=OFF` is the same escape hatch.

Neither question is one this document can close by reasoning about it, and neither is recorded here
as settled. What is recorded is what was taken, from which published version, and under which terms
— so that the answer, when it arrives, lands against a provenance record rather than against a
table nobody can trace.

---

## See also

- [Unit & Currency Reference](05_bovnar_unit_system.md) — the native registry and notation grammar these profiles sit beside
- [Unit Ambiguities](07_bovnar_unit_ambiguities.md) — how a unit token is resolved natively, and the pairs that look interchangeable
- [Unit Ambiguities §17](07_bovnar_unit_ambiguities.md#17-the-same-spelling-in-another-namespace) — the reader-facing index of the cross-namespace collisions §6.2 records
- [Unit Policy](06_bovnar_unit_policy.md) — the reader- and writer-side policies a translated unit passes through unchanged
- [Read/Write API](08_bovnar_readwrite_api.md#112-reader-side-unit-policy-bvnr_reader_set_unit_policy) — the reader-side unit policy, and the `want_unit` hook a profile unit reaches unmodified
- [Conformance Test Tool](13_bovnar_conformance.md) — where the 59-case `unit_profile` group lives
- [Unit Cheatsheet](04_bovnar_unit_cheatsheet.md) — the native spellings the transliteration tables target
- [EBNF](12_bovnar.ebnf) — `unit-param`, `profile-unit` and the byte classes of §2.3

---

*End of Bovnar — Unit Profiles (Bovnar spec 1.2).*
