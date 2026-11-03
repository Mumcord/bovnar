# src/gendata — catalogue data tables

Units, prefixes, and currencies are defined once, as **data**, in the `*.bvnr`
files here; the C tables are generated from them. So the enum, conversion table,
symbol map, parse/alias tables, prefix scale/policy tables, and currency
catalogue cannot drift out of sync. Covers 264 physical units (745 accepted
spellings), 34 SI/IEC prefixes, 216 currencies, and the atom tables of seven unit
profiles — 11 284 mapped codes and 1877 named refusals, which is every code UCUM,
UDUNITS-2, QUDT, OM 2 and the CF standard name table define, plus every UN/ECE
code QUDT's cross-reference reaches (doc/11 §15–§17).

Each `.bvnr` file's comment header documents its fields and the editing rules
(stable, append-only ids). **To add or change an entry, edit a record there and
rebuild — never edit the generated `*.gen.{h,inc}`.**

## Layout

| File | Location | Role |
|------|----------|------|
| `units.bvnr` | `src/gendata/` | the data: all 264 physical units |
| `prefixes.bvnr` | `src/gendata/` | the data: 24 SI + 10 IEC prefixes |
| `currencies.bvnr` | `src/gendata/` | the data: all 216 currencies |
| `ucum.bvnr` | `src/gendata/` | the data: the UCUM profile — prefixes, mapped atoms, arbitrary (opaque) units, and the atoms that are known but refused. All 312 atoms `ucum-essence.xml` defines |
| `unece.bvnr` | `src/gendata/` | the data: UN/ECE Rec 20 units, plus Rec 21 packages and Rec 20 counts as opaque units. Every Rec 20 code QUDT's cross-reference reaches — Rec 20 itself states its factors in prose, so there is no list to close against |
| `qudt.bvnr` | `src/gendata/` | the data: QUDT unit local names. All 2803, carried over through QUDT's own `ucumCode`/`udunitsCode` rather than by matching values |
| `qudt-qk.bvnr` | `src/gendata/` | the data: QUDT quantity kinds, each mapped to the **coherent SI unit** of the kind — taken from QUDT's own `applicableUnit` list, which is what separates `Torque` (`N·m`) from `Work` (`J`). All 1164 |
| `udunits.bvnr` | `src/gendata/` | the data: UDUNITS-2 atoms and prefixes, the CF/netCDF units syntax. All 570 spellings the database defines |
| `om.bvnr` | `src/gendata/` | the data: OM 2 local names. Every unit individual the ontology states, with each compound's target **built from OM's own composition** — prefix and base, numerator and denominator, term and term |
| `cf.bvnr` | `src/gendata/` | the data: CF standard names, all 5071 of table v94. A name translates to the `canonical_units` CF states for it, and the profile is **read-only**: a unit is never written back as a standard name |
| `gen_units.py` / `gen_prefixes.py` / `gen_currencies.py` / `gen_profiles.py` | repo root | the generators |
| `bvnr_data.py` | repo root | the small built-in `.bvnr` reader they use |

## Provenance and licensing

The seven profile files carry **identifier strings from six external
vocabularies** — UCUM, QUDT, OM 2, UDUNITS-2, the CF standard name table, and
UN/ECE Recommendations 20 and 21. Those strings belong to their publishers; the
mappings, targets and refusal rationales beside them are this project's own, and
`LICENSE` clause 4 draws that line.

Each profile file carries a `SOURCE, VERSION AND LICENSING` block in its header
naming the exact upstream version, the retrieval date, the licence and what was
extracted, so the provenance travels with the data. The full notices — copyright
lines, licence texts, and the modification statement CC BY 4.0 requires — are in
[`THIRD_PARTY_NOTICES.md`](../../THIRD_PARTY_NOTICES.md) at the repo root, and
`doc/11_bovnar_unit_profiles.md` §18 is the reader-facing account, including the
two questions that are still open (UCUM's derivative-works bar, and the CF
table's absent licence).

**Bumping a vocabulary to a new upstream version is three edits in one commit:**
the rows, the `.bvnr` header block, and the notices file — plus, for QUDT and CF,
the pinned URI in `check_profile_factors.py`, which fetches that exact published
version and never a `current/` alias, so the table can be traced to what it was
built from. UDUNITS-2, UCUM and OM are fetched from their moving default branch
on purpose: that is the drift the weekly `vocabulary-drift` workflow watches for.

## Building

The generators parse the `.bvnr` files with `bvnr_data.py` (a small built-in
reader) and need only **Python 3** — no bovnar library. CMake runs them at
configure time, regenerating when the snippets are missing or always with
`-DBVNR_REGEN_TABLES=ON`, so a clean checkout builds with just Python 3 + a C
compiler. To regenerate by hand, from the repo root:

```
python3 gen_units.py && python3 gen_prefixes.py && \
python3 gen_currencies.py && python3 gen_profiles.py
```

Generated outputs:

- **`*.gen.h` enum headers → `include/`** — committed (public API / install set).
- **`*.gen.inc` snippets → `<build>/generated/`** — build artifacts, **not
  committed**. CMake adds that directory to the private include path;
  `amalgamate.py` inlines the snippets via `BVNR_GENERATED_DIR`.

## Wiring map

Each hand-written span was replaced with an `#include` of a generated fragment.

| Generated file | Included from |
|----------------|---------------|
| `include/bovnar_units.gen.h` | `value_base_unit_e` enum in `bovnar.h` |
| `include/bovnar_si_prefix.gen.h` / `bovnar_iec_prefix.gen.h` | the two prefix enums in `bovnar.h` |
| `<build>/generated/bovnar_si_conv_table.gen.inc` | `si_conv_table` in `bovnar_si_units.c` |
| `<build>/generated/bovnar_base_unit_str.gen.inc` | `base_unit_str()` in `bovnar_utils.c` |
| `<build>/generated/bovnar_bu_table.gen.inc` | `bu_table` in `bovnar_utils.c` |
| `<build>/generated/bovnar_bu_index.gen.inc` | `bu_first_for_len`/`bu_max_len` in `bovnar_utils.c` |
| `<build>/generated/bovnar_prefix_policy.gen.inc` | `bu_prefix_policy` in `bovnar_si_units.c` |
| `<build>/generated/bovnar_si_pfx_table.gen.inc` / `bovnar_iec_pfx_table.gen.inc` | scale tables in `bvn_unit_impl.h` |
| `<build>/generated/bovnar_si_prefix_str.gen.inc` / `bovnar_iec_prefix_str.gen.inc` | symbol maps in `bovnar_utils.c` |
| `<build>/generated/bovnar_si_table.gen.inc` / `bovnar_iec_table.gen.inc` | prefix parse tables in `bovnar_utils.c` |
| `<build>/generated/bovnar_currency_table.gen.inc` | `g_currency_table` in `bovnar_currency.c` |
| `include/bovnar_profiles.gen.h` | every profile's opaque-unit enumerators, the whole block's FIRST/LAST bracketing macros, and a per-profile FIRST/LAST pair. **The ids are assigned by `gen_profiles.py`, not written in the data files** — with several profiles sharing the block, hand-numbering means renumbering every later profile whenever an earlier one grows a row. This header is committed, so a shift shows up in review |
| `<build>/generated/bovnar_profile_<ns>_prefix.gen.inc` / `…_atom.gen.inc` / `…_unsupported.gen.inc` / `…_reverse.gen.inc` | the per-namespace profile tables in `bovnar_profiles.c`. A *flat* profile (`unece`, `qudt`, `qudt-qk`) has no prefix mechanism and so no `_prefix` file |
| `<build>/generated/bovnar_profiles_conv.gen.inc` / `bovnar_profiles_policy.gen.inc` | `si_conv_table` and the prefix policy in `bovnar_si_units.c` |
| `<build>/generated/bovnar_profiles_str.gen.inc` | `base_unit_str()` in `bovnar_utils.c` |

The generated files carry **data only**. The conversion/parse/lookup functions,
the range predicates, `bvn_currency_index()`, and the rule body of
`bvn_prefix_unit_valid` stay hand-written (logic, not data).
