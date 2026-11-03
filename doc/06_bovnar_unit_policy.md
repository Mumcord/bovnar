# Bovnar — Unit Policy Reference

> **Spec version:** 1.2
> **Status:** Reference — the unit policy carried by `bvnr_unit_policy_t`, on the reader and on the writer.
> **Scope:** What a policy declares, the order its parts are consulted in, what it validates, what it converts, and the errors it raises. Behaviour only; the C declarations live in the read/write API reference.

Companion to [Unit & Currency Reference](05_bovnar_unit_system.md) (the registry a policy names units
from) and [Read/Write API](08_bovnar_readwrite_api.md) (the entry points that install one). Every
conversion and every error shown here was produced by running the reference implementation built
from this tree; the transcripts are output, not illustration.

A unit policy states, as **data**, what a document must contain and what unit the consumer wants
values delivered in. It is the declarative form of the `want_unit` hook: because every unit is
given as text, a binding can drive the whole feature without a per-value callback across an FFI
boundary. Nothing about it weakens the exactness contract — the policy chooses targets, and the
conversion itself is the same exact arbitrary-precision path `want_unit` uses.

## Table of Contents

1. [Overview](#1-overview)
    - 1.1 [What a unit policy is](#11-what-a-unit-policy-is)
    - 1.2 [What the parser enforces without one](#12-what-the-parser-enforces-without-one)
    - 1.3 [A policy and the want_unit hook](#13-a-policy-and-the-want_unit-hook)
2. [The policy object](#2-the-policy-object)
    - 2.1 [Per-field rules](#21-per-field-rules)
    - 2.2 [Conversion targets](#22-conversion-targets)
    - 2.3 [Normalisation](#23-normalisation)
    - 2.4 [Inexact results](#24-inexact-results)
    - 2.5 [Requiring a unit](#25-requiring-a-unit)
    - 2.6 [Requiring a dimension](#26-requiring-a-dimension)
    - 2.7 [Limits and lifetime](#27-limits-and-lifetime)
3. [Resolution order](#3-resolution-order)
    - 3.1 [The ladder](#31-the-ladder)
    - 3.2 [Validation runs on the written unit](#32-validation-runs-on-the-written-unit)
    - 3.3 [First match wins](#33-first-match-wins)
4. [Conversion semantics](#4-conversion-semantics)
    - 4.1 [Compatible is not convertible](#41-compatible-is-not-convertible)
    - 4.2 [A bare number matches only no_unit — and no_unit matches only a bare number](#42-a-bare-number-matches-only-no_unit--and-no_unit-matches-only-a-bare-number)
    - 4.3 [Exact, or not at all](#43-exact-or-not-at-all)
    - 4.4 [Quantity kinds keep the dimensionless units apart](#44-quantity-kinds-keep-the-dimensionless-units-apart)
5. [Writer-side policy](#5-writer-side-policy)
    - 5.1 [Validation only](#51-validation-only)
    - 5.2 [Why the producing side carries the promise](#52-why-the-producing-side-carries-the-promise)
    - 5.3 [Relationship to BVN_UNIT_REDUCE](#53-relationship-to-bvn_unit_reduce)
6. [Errors](#6-errors)
    - 6.1 [Rejected before the parse](#61-rejected-before-the-parse)
    - 6.2 [Raised during the parse](#62-raised-during-the-parse)
7. [Using a policy](#7-using-a-policy)
    - 7.1 [C](#71-c)
    - 7.2 [Python](#72-python)
    - 7.3 [Command line](#73-command-line)
8. [Worked examples](#8-worked-examples)
    - 8.1 [Deliver a document in SI](#81-deliver-a-document-in-si)
    - 8.2 [Assert a schema without converting](#82-assert-a-schema-without-converting)
    - 8.3 [Refuse to write a bare number](#83-refuse-to-write-a-bare-number)
- [See also](#see-also)


---

## 1. Overview

### 1.1 What a unit policy is

One `bvnr_unit_policy_t`, installed on a reader with `bvnr_reader_set_unit_policy` or on a writer
with `bvnr_writer_set_unit_policy`. It carries two independent kinds of statement:

- **Validation** — what the document must contain. These reject a document and never change a
  value: `require_unit`, `require_dimension_of`, and the assertion half of a per-field rule.
- **Conversion** — what unit the consumer wants values in: `rules`, `targets`, `normalise`, with
  `base` and `on_inexact` shaping the result.

A policy describes the **consumer**, not the document. It may be installed before or after the
reader or writer is opened, it survives re-opening on another document, and passing `NULL` clears
it.

### 1.2 What the parser enforces without one

Without any policy the parser still enforces everything the format itself guarantees: a unit
written on a value must exist in the registry, a unit parameter in a type annotation must agree
with an inline unit on the same value, and an exponent must be in range. What it does not do is
have an opinion about which units a *particular consumer* wanted. `.speed = 3 kg;` is a perfectly
valid document; only the application knows it is nonsense.

A policy is where that application knowledge goes.

### 1.3 A policy and the want_unit hook

Both may be set. The hook is more specific and wins: for each value the reader asks `want_unit`
first, falls back to `targets`, and falls back again to `normalise`. Normalising a document while
hand-handling the one field that needs something else is the intended combination.

The two differ in their exactness contract. A `want_unit` hook is strictly all-or-nothing. A
policy may be told to step over what it cannot deliver exactly — see [2.4](#24-inexact-results).

---

## 2. The policy object

### 2.1 Per-field rules

A rule names **one field**, by the dotted key path a value sits at, leading dot included, exactly
as the document writes its keys. It is the most specific thing a policy can say, and is consulted
before everything else in it.

```c
static const bvnr_unit_rule_t rules[] = {
    { ".inlet.temperature", "°C", 0, bvnr_rule_convert },
    { ".inlet.*",           "m",  0, bvnr_rule_require },
};
```

A path ending in `.*` matches every value below that point at any depth: `".inlet.*"` covers
`".inlet.temperature"` and `".inlet.pump.rpm"`, but not `".inlet"` itself. A prefix matches only
at a component boundary, so `".in.*"` does not claim `".inlet.a"`. Array elements sit at the path
of the assignment holding them, so one rule covers every element of an array.

`bvnr_rule_convert` asserts and converts; `bvnr_rule_require` asserts only, delivering the value
exactly as written — "this field is a length, in whatever length unit it chose".

A rule is an **assertion**, unlike a whole-document target: the caller named this field, so a
value that cannot be applied to it is `error_unit_mismatch` rather than a value passed through
quietly. Silence would defeat the point of naming it. A bare number fails a rule too — `.speed is
m/s` is not satisfied by a value with no unit.

**A rule that matches nothing is satisfied.** The assertion is about values the rule *reaches*, so
a path no value sits at makes no claim and the document passes:

```
--require-field .inlet.speed=m/s   on a document with .inlet.speed      asserted
--require-field .inlet.sped=m/s    on the same document                 ACCEPTED, checks nothing
--require-field .nope.*=m          on a document with no .nope          ACCEPTED, checks nothing
```

Read that line beside [§2.7](#27-limits-and-lifetime), which is the case it is most easily confused
with and which behaves the *opposite* way: a value the reader **cannot locate** — one nested past
the recorded path depth or length — is `error_unit_mismatch`, because "I could not check it" is not
"it holds". The distinction is between a rule that *found nothing to check* and one that *could not
check what it found*. Only the second is an error.

The practical consequence is worth stating plainly, because it runs against what the flag name
suggests: **`--require-field` does not require the field to exist.** A typo in the path — or a
rename in the document the policy was not updated for — turns that rule off silently, and the
document still validates. Nothing detects this for you; if a policy is load-bearing, assert that
the paths it names are the ones the document actually uses.

### 2.2 Conversion targets

`targets` applies to the whole document. Each numeric value is converted to the **first** target
it can validly convert to, so order is significant: a list of `{"m", "k~m"}` never selects
`k~m`. Each target may carry its own output base, `0` meaning the value keeps its own.

A value that matches no target is delivered untouched unless `normalise` catches it. A value
*already* in the unit a target names is also left untouched — unless that target asks for an
output base, which makes it a pure base conversion. The `converted` flag therefore means "the
policy restated this value", not "the policy looked at it".

### 2.3 Normalisation

`bvnr_normalise_si` catches whatever the targets did not, delivering it in coherent SI base units
with prefixes folded out. `base` sets the output base for those conversions.

Values with no coherent SI form are left alone rather than reinterpreted: currencies, and every
dimensionless unit (`%`, `ppm`, `dB`, `pH`, `rad`, `°`).

### 2.4 Inexact results

`bvnr_inexact_error` (the default) aborts the parse with `error_unit_inexact`. That is the right
behaviour when the caller named a target deliberately.

`bvnr_inexact_leave` delivers the value untouched instead, with `converted == false`. It exists
for `bvnr_normalise_si`, where every value *that has a coherent SI form* is a conversion candidate
and a single 5/18 factor would otherwise reject an ordinary document. A consumer using it must
read `converted` to know which values it happened to.

It applies to **every** result the conversion cannot deliver exactly — a rational with no
terminating expansion in the output base, and equally a genuinely **irrational** factor (a parsec,
a water hardness scale, a π-based angle converted to radian).

**"Exactly" is a property of the result, not of the factor**, so a rational factor rejects some
values in a document and not others: km/h → m/s is 5/18, and `90` converts to exactly `25` while
`100` converts to `250/9` and does not. Two speeds in one file, one accepted and one refused, is
the expected behaviour rather than a bug — an irrational factor is the only kind that fails for
every value:

```console
$ bovnar events --si heading.bvnr
… unit_inexact at line 2 col 32          the k~m/h, not the heading

$ bovnar events --si --leave-inexact heading.bvnr
"90.0" <float:64,_10,°>          left — dimensionless, never a candidate (§2.3)
"100.0" <float:64,_10,k~m/h>     left, 250/9 does not terminate
"5.0"  <float:64,_10,m>          already SI
```

**Read that first line carefully: it is the `k~m/h` that fails, not the degrees.** Under
`bvnr_normalise_si` a degree is not an inexact conversion — it is not a conversion at all, because
[§2.3](#23-normalisation) leaves every dimensionless unit alone. The heading validates under the
strict run too. What *does* abort a `--si` run on an irrational factor is a **dimensioned** one: a
parsec or a `°dH` water hardness, both of which have a coherent SI form and so are candidates.

A π-based angle reaches this path only when a target names its SI form outright —
`targets = {"rad"}` on a `°` value is `error_unit_inexact` strictly, and left untouched under
`bvnr_inexact_leave`. That is the same mechanism, reached deliberately rather than by
normalisation.

The tempting distinction — that an irrational factor is special because there is no rational to
hand over — belongs to [`want_unit_allow_nonterminating`](08_bovnar_readwrite_api.md), whose
fallback *is* the rational. This mode's fallback is the **native value**, and that works for an
irrational factor exactly as well as for a non-terminating one. It has to: a document carrying a
speed in km/h is entirely ordinary, and under `bvnr_normalise_si` the strict reading would reject
it — which is the case this mode exists for.

Only a **policy-chosen** target takes this path. A target named by the `want_unit` hook keeps the
strict all-or-nothing contract that hook was documented with, whatever `on_inexact` says
([1.3](#13-a-policy-and-the-want_unit-hook)).

### 2.5 Requiring a unit

`require_unit` demands that every numeric value carry one. A bare number fails, whether it was
written without a unit parameter or with an explicit `no_unit`.

### 2.6 Requiring a dimension

`require_dimension_of` demands that every numeric value be validly convertible to at least one of
the listed units: "this document is lengths and temperatures, in whatever unit it chose to write
them". It never changes a value.

A value with **no** unit satisfies this only if one of the listed units is itself `no_unit`, for
the reason in [4.2](#42-a-bare-number-matches-only-no_unit--and-no_unit-matches-only-a-bare-number).

### 2.7 Limits and lifetime

| Limit | Value | Applies to |
|---|---|---|
| `BVNR_MAX_UNIT_TARGETS` | 8 | `targets`, `require_dimension_of` |
| `BVNR_MAX_UNIT_RULES` | 8 | `rules` |
| `BVNR_MAX_UNIT_PATH` | 96 | a rule's `path`, NUL excluded |

Every unit string is parsed by the setter, so the strings themselves need not outlive the call.

**A rule needs to know where a value sits, and there is a depth past which the reader cannot say.**
A key path is recorded to a bounded depth (32 nested structs) and a bounded length (256 bytes, with
each key at most 64); a document that outruns either leaves the position of everything below it
*unknown*. The path machinery never reports a position it is unsure of — matching the wrong field
is the one failure a per-field rule must not have — so below that point a rule cannot be evaluated,
and **the parse fails with `error_unit_mismatch`**.

Refusing is the point. A rule is an assertion the caller made by naming a field, so "I could not
check it" is not "it holds"; a rule that quietly stopped applying once a document got deep would be
exactly the silence [§2.1](#21-per-field-rules) refuses for a value a rule cannot be applied to.
Nothing else in a policy is affected — `targets`, `normalise`, `require_unit` and
`require_dimension_of` ask about the value and never about where it sits, so a document of any
depth reads normally under those.

---

## 3. Resolution order

### 3.1 The ladder

For each numeric value, in order, stopping at the first that applies:

1. a **per-field rule** whose path matches;
2. the `want_unit` hook, if one is set;
3. the first **target** the value can validly convert to;
4. **normalisation**, if `bvnr_normalise_si` is set;
5. otherwise the value is delivered as written.

Validation is separate and does not participate in this ladder: it runs regardless of which rung
delivered the value.

### 3.2 Validation runs on the written unit

`require_unit` and `require_dimension_of` are evaluated on the unit the **document** wrote, before
any conversion. Validate what you were sent, convert for the consumer. This means they say the
same thing whether or not a conversion was also requested — a document is not made to pass by the
policy that was going to rewrite it anyway.

### 3.3 First match wins

Both `rules` and `targets` are first-match-wins, so order is part of the policy. Put `".a.b"`
before `".a.*"`, or the wildcard swallows the specific case.

---

## 4. Conversion semantics

### 4.1 Compatible is not convertible

Dimensional compatibility is not the test for whether a conversion can be performed. Currencies
carry no dimension vector and therefore fail a compatibility test even against themselves, yet
prefix conversion between them is exact and supported:

```text
5 k~$USD  ->  $USD      =  5000
```

Affine conversions have no single multiplicative factor, so a screen on "is there a conversion
factor" would drop every temperature in the format. Both of these work:

```text
212 °F  ->  °C          =  100
 25 °C  ->  K           =  298.15
```

### 4.2 A bare number matches only no_unit — and no_unit matches only a bare number

The fence runs **both ways**:

- a value with no unit only ever matches a target that is itself `no_unit`;
- a `no_unit` target only ever matches a value that has no unit.

A bare number is dimensionally compatible with `%` and with `ppm`, so without the fence a policy
naming `"%"` would deliver `0.25` as `25`, and one naming `no_unit` would deliver `35 %` as `0.35`
— the same silent factor-of-a-hundred from either side, arrived at through the machinery meant to
prevent it. Under `targets = ["%"]` a bare `0.25` is therefore delivered untouched, with
`converted == false`; under `targets = ["no_unit"]` a `35 %` is delivered untouched too.

```text
--unit %          0.25   (no unit)   delivered untouched
--unit %          5000 ppm       ->  0.5 %
--unit no_unit    0.25   (no unit)   matched; the conversion is the identity
--unit no_unit    25 %             delivered untouched
--unit no_unit    5000 ppm         delivered untouched
```

The second half is the one that surprises, so it is worth stating plainly: **`no_unit` as a target
does not strip units off dimensionless values.** There is no policy setting that turns `35 %` into
a bare number — by design, since that is precisely the rescale §4.2 exists to stop.

The fence is not "never convert dimensionless things": `ppm -> %` is a real and wanted conversion,
and it is unaffected. It is specifically about values that carry no unit at all.

### 4.3 Exact, or not at all

Nothing approximate is ever delivered. An exact **rational** and a terminating **positional
expansion** are different things, and only the second can be handed over as digits:

```text
100 mph   ->  m/s   base 10    44.704
100 km/h  ->  m/s   base 10    error_unit_inexact      (250/9)
212 °F    ->  °C    base 10    100
100 °F    ->  °C    base 10    error_unit_inexact      (340/9)
 12 in    ->  m     base 10    0.3048
  1 m     ->  k~m   base 10    0.001
```

Three things follow. Metric-to-metric is where the trouble is, not customary-to-metric: `km/h ->
m/s` carries a factor of 5/18 and does not terminate, while `mph -> m/s` is exactly `44.704` and
does. Whether a conversion terminates depends on the **value** as well as the units — `212 °F`
converts cleanly and `100 °F` does not, through the same 5/9 slope. And the output base matters
independently: `1 m -> k~m` is `0.001` in base 10 and non-terminating in base 2.

This is what `on_inexact` exists to shape; see [2.4](#24-inexact-results).

### 4.4 Quantity kinds keep the dimensionless units apart

Dimensionless does not collapse into one bucket. The library carries a quantity-kind vector
alongside the dimension vector, so decibels, the pH scale, angles, turbidity and practical
salinity are each their own kind and do not convert into one another:

```text
dB   ->  %          refused
pH   ->  no_unit    refused
B/s  ->  Hz         refused
rad  ->  °          allowed
```

Normalisation needs no hand-maintained skip list to keep these apart. The hazards are narrower
than the registry suggests: currencies ([4.1](#41-compatible-is-not-convertible)) and unitless
values ([4.2](#42-a-bare-number-matches-only-no_unit--and-no_unit-matches-only-a-bare-number)), not the logarithmic scales.

---

## 5. Writer-side policy

### 5.1 Validation only

The same policy object drives `bvnr_writer_set_unit_policy`, but only its validating half is
accepted: `require_unit`, `require_dimension_of`, and per-field rules in `bvnr_rule_require` mode.
A policy carrying `targets`, `normalise`, `base`, `on_inexact` or a `bvnr_rule_convert` rule is
rejected outright rather than half-honoured.

`bvnr_write_event` fails with `error_unit_mismatch`, and the writer latches it like any other
write error — query it with `bvnr_writer_get_error`.

The unit checked is the one the value will carry in the output, whether it was given inline or as
a parameter of its type annotation.

### 5.2 Why the producing side carries the promise

A reader can only reject a document somebody has already written. Only the writer can stop a bare
number reaching a file in the first place, which is what "hand the file to anyone and they have
everything required to interpret it" actually depends on.

### 5.3 Relationship to BVN_UNIT_REDUCE

The writer already has a value-rewriting mode: `BVN_UNIT_REDUCE` in
`bvnr_write_flags_t.unit_flags`, which folds prefixes out and rescales the value exactly. The
writer-side policy deliberately does not add a second one. Two rewriting paths arriving through
different doors, with different rules about exactness, is how two features end up disagreeing
about what a document says.

---

## 6. Errors

### 6.1 Rejected before the parse

The setter parses every unit string, so a malformed unit, an out-of-range count or an unusable
base is reported by a `false` return **before** the parse begins, rather than as a mid-document
error in somebody else's file. A rejected policy leaves the previous one in force.

### 6.2 Raised during the parse

| Error | Raised by |
|---|---|
| `error_unit_mismatch` | `require_unit` on a bare value; `require_dimension_of` matching nothing; a value at a path a rule names that is not convertible to the rule's unit; a value nested too deep for a rule to be evaluated at all ([§2.7](#27-limits-and-lifetime)) |
| `error_unit_inexact` | a conversion that cannot be delivered exactly, under `bvnr_inexact_error` |

---

## 7. Using a policy

### 7.1 C

```c
static const bvnr_unit_target_t targets[] = { { "m/s", 0 }, { "°C", 0 } };
bvnr_unit_policy_t p = {0};
p.targets     = targets;
p.num_targets = 2;
p.normalise   = bvnr_normalise_si;
p.on_inexact  = bvnr_inexact_leave;
p.require_unit = true;

if (!bvnr_reader_set_unit_policy(r, &p)) { /* a unit string was malformed */ }
```

### 7.2 Python

```python
from bovnar import Reader, UnitPolicy

with Reader() as r:
    r.set_unit_policy(UnitPolicy(
        targets=["m/s", "°C"],
        normalise_si=True,
        leave_inexact=True,
        require_unit=True,
    ))
    r.read_file("sensors.bvnr", on_verified=handler)
```

A malformed unit raises `BovnarArgumentError` before a byte is read.

### 7.3 Command line

`validate`, `events` and `query` accept the options below. `pretty-print` and `convert` do not
take a unit policy at all, and they differ in how they say so: `pretty-print` rejects each one by
name — `pretty-print: unknown option --si`, exit 2 — while `convert` **accepts and silently
ignores** every flag in the table, including the ones that take an argument. A `bovnar convert --si`
that appears to work has not normalised anything.

| Flag | Field |
|---|---|
| `--require-unit` | `require_unit` |
| `--require-dimension <unit>` | `require_dimension_of` (repeatable) |
| `--require-field <path>=<unit>` | a `bvnr_rule_require` rule |
| `--unit <unit>` | a target (repeatable, first match wins) |
| `--field <path>=<unit>` | a `bvnr_rule_convert` rule |
| `--si` | `bvnr_normalise_si` |
| `--base <N>` | `base` |
| `--leave-inexact` | `bvnr_inexact_leave` |

`query` takes all of them except `--base`, which it refuses by name (exit 2):
it prints the value it found in decimal, and a base would change nothing it
shows.

**A conversion flag on `validate` is not a no-op.** `validate` produces no output,
so it is easy to read `--unit` and `--si` there as flags that could not matter —
and they change the verdict, because a conversion the policy cannot deliver
exactly fails the parse. `bovnar validate --si` on a document holding
`100 k~m/h` reports `unit_inexact at line 1, col 32` where the bare `validate`
reports OK. That makes it a useful question in its own right — *would this
document survive normalisation?* — with `--leave-inexact` as the way to ask the
weaker one. The CLI's own help listed only the `--require-*` flags under
`validate` until this was written, so neither the behaviour nor its remedy was
discoverable from `bovnar --help`.

---

## 8. Worked examples

The document used throughout:

```bovnar
.inlet = {
    .temperature = 212 °F;
    .speed       = 100 mph;
};
.spare = 0.25;
```

### 8.1 Deliver a document in SI

```console
$ bovnar query --unit m/s .inlet.speed sensors.bvnr
44.704
$ bovnar query --unit °C .inlet.temperature sensors.bvnr
100
```

`.spare` carries no unit, so no target claims it and it is delivered as written.

### 8.2 Assert a schema without converting

```console
$ bovnar validate --require-field .inlet.speed=m/s sensors.bvnr
sensors.bvnr: OK

$ bovnar validate --require-unit sensors.bvnr
Validation failed: unit_mismatch at line 5, col 14
```

The first asserts that `.inlet.speed` is a speed without touching the value — `mph` is convertible
to `m/s`, so the assertion holds. The second fails on `.spare`, which is the bare number.

### 8.3 Refuse to write a bare number

Set the same `require_unit` policy on a writer and the bare value never reaches a file:

```c
bvnr_unit_policy_t p = {0};
p.require_unit = true;
bvnr_writer_set_unit_policy(w, &p);
/* bvnr_write_event for a value with no unit now fails with
 * error_unit_mismatch, latched on the writer. */
```

---

## See also

- [Unit & Currency Reference](05_bovnar_unit_system.md) — the unit registry and the notation grammar
- [Unit Ambiguities](07_bovnar_unit_ambiguities.md) — how a unit token is resolved, and the pairs that look interchangeable
- [Read/Write API](08_bovnar_readwrite_api.md) — the C declarations, the `want_unit` hook, and the read flags
- [Python Bindings](09_bovnar_python_bindings.md) — the `UnitPolicy` dataclass and `Reader.set_unit_policy`
- [Rust Bindings](14_bovnar_rust_bindings.md) — the `UnitPolicy` struct and `Reader::set_unit_policy`

---

*End of Bovnar — Unit Policy Reference (Bovnar spec 1.2).*
