# Bovnar — Unit & Currency Reference

> **Spec version:** 1.2
> **Status:** Normative — the unit and currency registry the parser validates against
> **Scope:** Physical units, currency codes, prefix rules, disambiguation, C/Python APIs, and validation.

---

## Table of Contents

1. [Overview](#1-overview)
    - 1.1 [Design Principles](#11-design-principles)
2. [Syntax — Annotation as a Type Parameter](#2-syntax--annotation-as-a-type-parameter)
    - 2.1 [Parameter Ordering Flexibility](#21-parameter-ordering-flexibility)
    - 2.2 [Inline Unit Suffix](#22-inline-unit-suffix)
    - 2.3 [Applicable Type Families](#23-applicable-type-families)
3. [Physical Base Units](#3-physical-base-units)
    - 3.1 [SI Base Units](#31-si-base-units)
    - 3.2 [Named SI-Derived Units](#32-named-si-derived-units)
    - 3.3 [Non-SI Units Accepted for Use with SI](#33-non-si-units-accepted-for-use-with-si)
    - 3.4 [Imperial and US Customary Units](#34-imperial-and-us-customary-units)
    - 3.5 [Pressure Units](#35-pressure-units)
    - 3.6 [Energy Units](#36-energy-units)
    - 3.7 [Power Units](#37-power-units)
    - 3.8 [Force Units](#38-force-units)
    - 3.9 [Speed and Rotational Frequency Units](#39-speed-and-rotational-frequency-units)
    - 3.10 [Volume Units](#310-volume-units)
    - 3.11 [Area Units](#311-area-units)
    - 3.12 [Angle Units](#312-angle-units)
    - 3.13 [CGS Units](#313-cgs-units)
    - 3.14 [Radiation Units](#314-radiation-units)
    - 3.15 [Logarithmic Units](#315-logarithmic-units)
    - 3.16 [Electrical Power Units](#316-electrical-power-units)
    - 3.17 [Digital Units](#317-digital-units)
    - 3.18 [Textile Linear Density](#318-textile-linear-density)
    - 3.19 [US Apothecary / Dry Volume](#319-us-apothecary--dry-volume)
    - 3.20 [Old German Units](#320-old-german-units)
    - 3.21 [Additional Length Units](#321-additional-length-units)
    - 3.22 [Additional Mass Units](#322-additional-mass-units)
    - 3.23 [Acceleration](#323-acceleration)
    - 3.24 [Signal Rate](#324-signal-rate)
    - 3.25 [Ratio and Proportion Units](#325-ratio-and-proportion-units)
    - 3.26 [Named Speed Units](#326-named-speed-units)
    - 3.27 [Acidity](#327-acidity)
    - 3.28 [Water Hardness](#328-water-hardness)
    - 3.29 [Conductivity and Dissolved Solids](#329-conductivity-and-dissolved-solids)
    - 3.30 [Turbidity and Salinity](#330-turbidity-and-salinity)
    - 3.31 [Sentinel Value](#331-sentinel-value)
    - 3.32 [Units the Unit Profiles Needed](#332-units-the-unit-profiles-needed)
4. [Prefixes](#4-prefixes)
    - 4.1 [SI Prefixes](#41-si-prefixes)
    - 4.2 [IEC Binary Prefixes](#42-iec-binary-prefixes)
    - 4.3 [Compact Prefix Form](#43-compact-prefix-form)
5. [Unit Notation Grammar](#5-unit-notation-grammar)
    - 5.1 [Simple Units](#51-simple-units)
    - 5.2 [Compound Units](#52-compound-units)
    - 5.3 [Separators](#53-separators)
    - 5.4 [Denominator Semantics](#54-denominator-semantics)
6. [Exponents](#6-exponents)
    - 6.1 [Unicode Superscript Form](#61-unicode-superscript-form)
    - 6.2 [ASCII Caret Form](#62-ascii-caret-form)
    - 6.3 [Exponent Edge Cases](#63-exponent-edge-cases)
7. [The `no_unit` Keyword](#7-the-no_unit-keyword)
8. [Constraints and Limits](#8-constraints-and-limits)
9. [Currency Codes](#9-currency-codes)
    - 9.1 [The `$` Sigil Rule](#91-the--sigil-rule)
    - 9.2 [ISO 4217 Fiat Currencies and Precious Metals](#92-iso-4217-fiat-currencies-and-precious-metals)
    - 9.3 [Cryptocurrencies](#93-cryptocurrencies)
    - 9.4 [Prefix Rules for Currency Units](#94-prefix-rules-for-currency-units)
    - 9.5 [Compound Currency Expressions](#95-compound-currency-expressions)
    - 9.6 [Compatibility Rules](#96-compatibility-rules)
    - 9.7 [Type Pairing Recommendations](#97-type-pairing-recommendations)
10. [Symbol Disambiguation](#10-symbol-disambiguation)
    - 10.1 [The Namespace Rule as Disambiguator](#101-the-namespace-rule-as-disambiguator)
    - 10.2 [Exhaustive Conflict Table](#102-exhaustive-conflict-table)
    - 10.3 [The CUP Case in Detail](#103-the-cup-case-in-detail)
    - 10.4 [The Mandatory Currency Sigil](#104-the-mandatory-currency-sigil)
11. [C Data Model](#11-c-data-model)
    - 11.1 [Enumerations](#111-enumerations)
    - 11.2 [Structures](#112-structures)
    - 11.3 [Convenience Macros](#113-convenience-macros)
12. [C API Functions](#12-c-api-functions)
    - 12.1 [Parsing a Unit String](#121-parsing-a-unit-string)
    - 12.2 [Serializing a Unit](#122-serializing-a-unit)
    - 12.3 [Prefix Factor and Exponent Queries](#123-prefix-factor-and-exponent-queries)
    - 12.4 [SI Conversion API](#124-si-conversion-api)
    - 12.5 [Currency API](#125-currency-api)
    - 12.6 [Unit Profile API (spec 1.2)](#126-unit-profile-api-spec-12)
    - 12.7 [Python API](#127-python-api)
13. [Integration with the Parser Event Stream](#13-integration-with-the-parser-event-stream)
    - 13.1 [Full event sequence — physical unit](#131-full-event-sequence--physical-unit)
    - 13.2 [Full event sequence — currency unit](#132-full-event-sequence--currency-unit)
    - 13.3 [Inline unit suffix — event stream view](#133-inline-unit-suffix--event-stream-view)
    - 13.4 [Practical callback](#134-practical-callback)
14. [Validation Errors](#14-validation-errors)
15. [Annotated Examples](#15-annotated-examples)
    - 15.1 [Physical Quantities](#151-physical-quantities)
    - 15.2 [Digital Storage](#152-digital-storage)
    - 15.3 [Compound SI Quantities](#153-compound-si-quantities)
    - 15.4 [Currency Amounts and Rates](#154-currency-amounts-and-rates)
    - 15.5 [Error Cases](#155-error-cases)
- [See also](#see-also)

---

## 1. Overview

The Bovnar quantity annotation system is an **optional, per-value annotation** that attaches a physical unit or currency denomination to any numeric field. It is part of the type annotation (`<family:width,_base,unit>`) and applies to the `uint`, `sint`, `float`, `float_fix`, and `float_dec` type families.

Two distinct namespaces share the annotation slot:

- **Physical units** — 264 named base units covering SI, Imperial, CGS, radiation, surveying, culinary, Old German, and digital storage quantities.
- **Currency codes** — 216 monetary denominations: 166 ISO 4217 alphabetic codes (including precious-metal X-codes; 4 are historical — HRK retired 2023-01-01, SLL replaced by SLE 2022, ZWL superseded by ZWG 2024, BGN retired 2026-01-01 — and `ANG` coexists with its successor `XCG`, which inherited its numeric code 532; see §9.2) and 50 cryptocurrency tickers.

Both namespaces are syntactically unified: the same grammar, the same `~` prefix separator, the same compound-unit operators (`·`, `*`, `/`), and the same `value_unit_t` data model apply to both. They are separated purely by a token-classification rule described in §9.1 and §10.

Annotations are **descriptive**, not prescriptive: the *validator* checks form and type, and never rejects a document because its units do not add up. The library does provide dimensional analysis and unit conversion as an explicit, opt-in service — `bvn_units_compatible`, `bvn_unit_convert_factor`, `bvn_unit_convert_value` and the exact-rational `bvn_unit_convert_rational` (§12.4), plus the reader's `want_unit` hook, which converts at read time (§1.10 of the read/write API). None of that runs unless you ask for it. Exchange-rate arithmetic is the one thing the library genuinely does not do: currencies carry no conversion table, and a cross-currency conversion is always refused rather than guessed (§9.6).

### 1.1 Design Principles

- **SI-first.** All SI base units, all 22 BIPM-2019 named derived units, and all 24 current SI prefixes (quecto … quetta) are supported.
- **Binary-prefix aware.** IEC 80000-13 binary prefixes (kibi … yobi) are supported for digital storage quantities, plus `Ri`/`Qi` (robi, quebi) as a forward-looking extension — those two are a proposal, not part of IEC 80000-13, which stops at yobi.
- **Compound units.** Derived quantities (m/s, kg·m/s², USD/oz_t) are expressed inline without separate schema definitions.
- **Two exponent notations.** Unicode superscript (`m²`, `s⁻²`) and ASCII caret (`m^2`, `s^-2`) are accepted equivalently.
- **Currency as a first-class unit.** ISO 4217 and cryptocurrency codes participate in all unit composition rules — prefixes, compound expressions, and the `value_unit_t` representation — with no special-case parsing.
- **Dimensionless values.** The keyword `no_unit` is the canonical representation of a dimensionless quantity.

---

## 2. Syntax — Annotation as a Type Parameter

The unit or currency code occupies the **third positional parameter class** of a type annotation, after the optional bit-width and optional base. Parameter classes are identified by their content, not by position:

```
type-spec       = param-type [ ":" type-param-list ]
type-param-list = type-param { "," type-param }
type-param      = width-param   (* plain decimal integer, e.g. 32    *)
                | base-param    (* "_" + decimal integer,  e.g. _16  *)
                | unit-param    (* everything else,        e.g. m/s  *)
```

> **A unit may also be written in a foreign notation.** A `name:` namespace hands
> the parameter to a **unit profile**, which translates it into the same
> `value_unit_t` this document describes — so `<float_dec:64,ucum:mm[Hg]>` and
> `<float_dec:64,mmHg>` are the same unit to every part of the library. Seven
> namespaces are defined — `ucum`, `unece`, `qudt`, `qudt-qk`, `udunits`, `om`
> and `cf`; everything in this reference applies to the
> result unchanged. **The notation is spec 1.2**, so a document must opt in with
> a `#!bovnar 1.2` directive. A native unit is unaffected in every version. See
> [Unit Profiles](11_bovnar_unit_profiles.md).

### 2.1 Parameter Ordering Flexibility

```bovnar
.val = <uint:32,_10,no_unit> 42;
#      <uint:_10,no_unit,32>        — identical (parameter order is free)
#      <uint:no_unit,_10,32>        — identical
```

### 2.2 Inline Unit Suffix

A unit may be written directly after a scalar value literal, between the value and the terminating `;`:

```bovnar
.distance  = 1500 m;            # inline physical unit
.speed     = 9.81 m/s;          # compound inline unit
.price     = 19.99 $USD;        # inline currency (mandatory $ sigil)
.gold_rate = 2351.40 $USD/oz_t; # inline compound currency/unit
.ratio     = 3.14 no_unit;      # explicit dimensionless
```

The inline unit uses the **same character set** and **same semantic parser** (`bvn_parse_unit`) as the type-annotation unit parameter. It is terminated by ASCII whitespace, `#` (comment), or `;`.

**A native unit contains no whitespace, in either position.** Inline, whitespace ends the token, so `1.0 k g` is a value with a stray token after it (`error_unexpected_input_byte`). In a type annotation, whitespace is legal only beside a separator — the family `:`, a `,` between parameters, the closing `>` — and inside a native unit parameter it is `error_type_param_whitespace` (spec [§5.3](03_bovnar_spec.md#53-parameter-order)). Whitespace there used to be deleted, so `<float:64,m in>` was accepted as `min` — the minute — which made it the one place in the format where a wrong unit was produced silently instead of refused.

A parameter carrying a **profile namespace** is the exception: there whitespace is kept verbatim and the vocabulary decides what it means, because UDUNITS multiplies with a space and `udunits:kg m-2 s-1` is the commonest spelling of a flux in CF metadata. That is a property of the foreign notation, not of the native one — see doc/11 §13.2.

#### Constraints

| Situation | Result |
|-----------|--------|
| No annotation unit; inline unit present | Inline unit becomes the effective unit |
| Annotation has no unit; inline unit present | Inline unit becomes the effective unit |
| Annotation unit present; no inline unit | Annotation unit is the effective unit |
| Annotation unit **equals** inline unit | Valid; the common unit is used |
| Annotation unit **differs** from inline unit | `error_unit_mismatch` |
| Inline unit inside an array element | `error_unexpected_input_byte` |

When both are present, equality is checked after parsing via `bvn_unit_equal`, a structural comparison of the parsed `value_unit_t` values: the two units must have the same number of components and the same *set* of components (matching base, exponent, and prefix). The comparison is **order-insensitive** — unit multiplication is commutative, so components are matched as multisets and reordered spellings such as `N·m` and `m·N` (or `m·s⁻¹` and `m/s`) compare as equal. (It is *not* a raw `memcmp`, which would wrongly reject reordered components.)

<!-- bovnar-example: rejected -->
```bovnar
.v = <float:64,m/s> 9.81 m·s⁻¹;   # OK: both parse to m/s
.v = <float:64,m> 1.0 s;           # ERROR: error_unit_mismatch
```

### 2.3 Applicable Type Families

| Type family | Unit / currency parameter |
|-------------|--------------------------|
| `uint`      | Supported |
| `sint`      | Supported |
| `float`     | Supported (binary floating-point; discouraged for monetary amounts — see §9.7) |
| `float_fix` | Supported (wrong for monetary values — see §9.7) |
| `float_dec` | Supported; **recommended** for monetary amounts |
| `utf8`      | Parameterless: a unit (or any other parameter) is `error_illegal_value_type` |

---

## 3. Physical Base Units

Bovnar supports 264 physical base units. Currency codes are a separate namespace and are covered in §9.

> **Reading this section:** The *Symbol* column gives the canonical serialized form. *Long forms* are accepted on input but never produced on output. *Enum value* is the `value_base_unit_t` constant used in the C API.

### 3.1 SI Base Units

| Symbol | Long forms | Name | Enum value | Notes |
|--------|-----------|------|------------|-------|
| `s`    | `sec`, `second`, `seconds` | second | `bu_second` | SI base unit of time |
| `m`    | `meter`, `metre`, `meters`, `metres` | meter | `bu_meter` | SI base unit of length |
| `g`    | `gram`, `grams` | gram | `bu_gram` | SI base unit of mass is kg; `g` carries the prefix |
| `A`    | `amp`, `amps`, `ampere`, `amperes` | ampere | `bu_ampere` | SI base unit of electric current |
| `K`    | `kelvin`, `kelvins` | kelvin | `bu_kelvin` | SI base unit of thermodynamic temperature |
| `mol`  | `mole`, `moles` | mole | `bu_mol` | SI base unit of amount of substance |
| `cd`   | `candela`, `candelas` | candela | `bu_candela` | SI base unit of luminous intensity |

> **Note on the kilogram:** Bovnar uses `g` (gram) as the base unit symbol so that the `k~` (kilo) prefix can be attached explicitly: `k~g` = kilogram. This is consistent with how the SI formally defines the kilogram as a prefixed gram.

### 3.2 Named SI-Derived Units

| Symbol | Long forms | Name | Enum value | SI Definition |
|--------|-----------|------|------------|---------------|
| `Hz`   | `hertz` | hertz | `bu_hertz` | s⁻¹ |
| `N`    | `newton`, `newtons` | newton | `bu_newton` | kg·m·s⁻² |
| `Pa`   | `pascal`, `pascals` | pascal | `bu_pascal` | kg·m⁻¹·s⁻² |
| `J`    | `joule`, `joules` | joule | `bu_joule` | kg·m²·s⁻² |
| `W`    | `watt`, `watts` | watt | `bu_watt` | kg·m²·s⁻³ |
| `V`    | `volt`, `volts` | volt | `bu_volt` | kg·m²·A⁻¹·s⁻³ |
| `Ω`    | `ohm`, `ohms`, `Ohm` | ohm | `bu_ohm` | kg·m²·A⁻²·s⁻³ — U+2126 OHM SIGN, UTF-8: `0xE2 0x84 0xA6`; U+03A9 (Greek capital omega) also accepted on input, canonical output is always U+2126 |
| `F`    | `farad`, `farads` | farad | `bu_farad` | kg⁻¹·m⁻²·A²·s⁴ |
| `C`    | `coulomb`, `coulombs` | coulomb | `bu_coulomb` | A·s |
| `S`    | `siemens`, `mho`, `mhos`, `℧` | siemens | `bu_siemens` | kg⁻¹·m⁻²·A²·s³ |
| `Wb`   | `weber`, `webers` | weber | `bu_weber` | kg·m²·A⁻¹·s⁻² |
| `T`    | `tesla`, `teslas` | tesla | `bu_tesla` | kg·A⁻¹·s⁻² |
| `H`    | `henry`, `henrys`, `henries` | henry | `bu_henry` | kg·m²·A⁻²·s⁻² |
| `°C`   | `degC`, `degrC`, `degreeC`, `degreesC`, `celsius` | degree Celsius | `bu_celsius` | K = °C + 273.15 (affine); BIPM Table 4 entry 14 |
| `lm`   | `lumen`, `lumens` | lumen | `bu_lumen` | cd·sr |
| `lx`   | `lux` | lux | `bu_lux` | cd·sr·m⁻² |
| `Bq`   | `becquerel`, `becquerels` | becquerel | `bu_becquerel` | s⁻¹ |
| `Gy`   | `gray`, `grays` | gray | `bu_gray` | m²·s⁻² |
| `Sv`   | `sievert`, `sieverts` | sievert | `bu_sievert` | m²·s⁻² |
| `kat`  | `katal`, `katals` | katal | `bu_katal` | mol·s⁻¹ |
| `rad`  | `radian`, `radians` | radian | `bu_radian` | dimensionless (plane angle; m/m) |
| `sr`   | `steradian`, `steradians` | steradian | `bu_steradian` | dimensionless (solid angle; m²/m²) |

> **Photometry: the steradian is carried, not dropped.** `lm`, `lx` and `ph` are
> defined *through* the steradian, so they carry its quantity kind (§3.12 and
> §11 of [Unit Ambiguities](07_bovnar_unit_ambiguities.md)); `cd` and `sb` do not.
> The SI dimension vector cannot tell them apart — every photometric unit reduces
> to candela in base dimensions — so without the kind the library both refused
> `lm ↔ cd·sr` and converted `lm ↔ cd` at factor 1, which is the same claim with
> the `sr` silently dropped. What holds now:
>
> | Converts | Refused |
> |----------|---------|
> | `lm` ↔ `cd·sr`, `lx` ↔ `lm/m²` ↔ `cd·sr/m²`, `ph` ↔ `lx`, `sb` ↔ `cd/m²` | `lm` ↔ `cd` (flux vs intensity), `lx` ↔ `cd/m²` and `ph` ↔ `sb` (illuminance vs luminance) |

### 3.3 Non-SI Units Accepted for Use with SI

| Symbol | Long forms | Name | Enum value | Notes |
|--------|-----------|------|------------|-------|
| `L`, `l` | `liter`, `litre`, `liters`, `litres` | liter | `bu_liter` | 10⁻³ m³ |
| `min`  | `minute`, `minutes` | minute | `bu_minute` | 60 s |
| `h`    | `hour`, `hours` | hour | `bu_hour` | 3600 s |
| `d`    | `day`, `days` | day | `bu_day` | 86400 s |
| `wk`   | `week`, `weeks` | week | `bu_week` | 604800 s |
| `yr`   | `year`, `years` | year | `bu_year` | 31557600 s (Julian year) |
| `mo`   | `month`, `months` | month (Julian) | `bu_month` | 2629800 s (= 365.25 d / 12) |
| `fn`   | `fortnight`, `fortnights` | fortnight | `bu_fortnight` | 1209600 s (= 14 d) |
| `°`, `deg` | `degr`, `degree`, `degrees` | degree (angle) | `bu_degree` | π/180 rad — U+00B0 |
| `t`    | `tonne` | tonne | `bu_tonne` | 10³ kg |
| `bar`  | — | bar | `bu_bar` | 10⁵ Pa |
| `eV`   | `electronvolt` | electronvolt | `bu_electronvolt` | 1.602176634×10⁻¹⁹ J |
| `Da`   | `dalton`, `amu`, `u` | dalton | `bu_dalton` | 1.66053906892×10⁻²⁷ kg (CODATA 2022) |
| `au`   | — | astronomical unit | `bu_astronomical_unit` | 1.495978707×10¹¹ m |
| `ha`   | `hectare` | hectare | `bu_hectare` | 10⁴ m² |

### 3.4 Imperial and US Customary Units

#### Length

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `in`   | `inch`, `inches` | inch | `bu_inch` | 0.0254 m (exact) |
| `ft`   | `foot`, `feet` | foot | `bu_foot` | 0.3048 m (exact) |
| `yd`   | `yard`, `yards` | yard | `bu_yard` | 0.9144 m (exact) |
| `mi`   | `mile`, `miles` | statute mile | `bu_mile` | 1609.344 m (exact) |
| `nmi`  | `nautical_mile`, `nautical_miles` | nautical mile | `bu_nautical_mile` | 1852 m (exact) |
| `Å` (U+212B) | `angstrom`, `angstroms`, Å (U+00C5), Å (U+0041 U+030A) | ångström | `bu_angstrom` | 10⁻¹⁰ m |
| `ly`   | `light_year`, `light_years` | light-year | `bu_light_year` | 9.4607304725808×10¹⁵ m |
| `pc`   | `parsec`, `parsecs` | parsec | `bu_parsec` | 3.085677581491367×10¹⁶ m |
| `fur`  | `furlong`, `furlongs` | furlong | `bu_furlong` | 201.168 m (exact) |
| `fath` | `fathom`, `fathoms` | fathom | `bu_fathom` | 1.8288 m (exact) |
| `thou` | `thou`, `mil`, `mils` | thou | `bu_thou` | 25.4×10⁻⁶ m (exact) |
| `ch`   | `chain`, `chains` | chain (Gunter's) | `bu_chain` | 20.1168 m (exact) |
| `rd`   | `rod`, `rods` | rod (pole, perch) | `bu_rod` | 5.0292 m (exact) |

> **Thou vs mil:** Both `thou` and `mil` are accepted for 1/1000 of an inch (25.4 µm). The canonical output form is `thou`. Note that `mil` does **not** mean milliradian; milliradians are written `m~rad`.

#### Mass

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `lb`   | `lbs`, `pound`, `pounds` | pound (avoirdupois) | `bu_pound` | 0.45359237 kg (exact) |
| `oz`   | `ounce`, `ounces` | ounce (avoirdupois) | `bu_ounce` | 0.028349523125 kg (exact) |
| `gr`   | `grain`, `grains` | grain | `bu_grain` | 6.479891×10⁻⁵ kg (exact) |
| `st`   | `stone`, `stones` | stone | `bu_stone` | 6.35029318 kg (exact) |
| `tn_sh`| `short_ton`, `short_tons` | short ton (US ton) | `bu_short_ton` | 907.18474 kg (exact) |
| `tn_l` | `long_ton`, `long_tons` | long ton (UK ton) | `bu_long_ton` | 1016.0469088 kg (exact) |
| `oz_t` | `troy_ounce`, `troy_ounces` | troy ounce | `bu_troy_ounce` | 0.0311034768 kg (exact) |
| `ct`   | `carat`, `carats` | metric carat | `bu_carat` | 2×10⁻⁴ kg (exact) |
| `slug` | `slugs` | slug | `bu_slug` | 14.593902937206364 kg (= `lb`·`gn`/`ft`) |
| `dr`   | `dram`, `drams` | dram (avoirdupois) | `bu_dram` | 1.7718451953125×10⁻³ kg (exact) |
| `dwt`  | `pennyweight`, `pennyweights` | pennyweight (troy) | `bu_pennyweight` | 1.55517384×10⁻³ kg (exact) |
| `lb_t` | `troy_pound`, `troy_pounds`, `apothecary_pound` | troy pound (= apothecary pound) | `bu_troy_pound` | 0.3732417216 kg (exact, = 12 `oz_t`) |
| `dr_ap`| `apothecary_dram`, `apothecary_drams` | dram (apothecary) | `bu_apothecary_dram` | 3.8879346×10⁻³ kg (exact, = 3 `sc`) |
| `cwt_l`| `long_hundredweight`, `long_hundredweights` | hundredweight (long/imperial) | `bu_long_hundredweight` | 50.80234544 kg (exact, = 112 `lb`) |

> The apothecary dram is **2.2×** the avoirdupois `dr` and the two are a classic near miss; the
> apothecary POUND, by contrast, *is* the troy pound (twelve apothecary ounces, and an apothecary
> ounce is a troy ounce), so both spellings are one unit. The SHORT hundredweight is exactly 100 lb,
> which the prefix mechanism already spells `h~lb`, so it has no unit of its own.

#### Temperature

| Symbol | Long forms | Name | Enum value | Conversion |
|--------|-----------|------|------------|------------|
| `°C`, `degC` | `degrC`, `degreeC`, `degreesC`, `celsius` | degree Celsius | `bu_celsius` | K = °C + 273.15 (affine) — also §3.2 (BIPM named derived unit) |
| `°F`, `degF` | `degrF`, `degreeF`, `degreesF`, `fahrenheit` | degree Fahrenheit | `bu_fahrenheit` | K = (°F + 459.67) × 5/9 (affine) |
| `°Ra`, `degRa` | `degrRa`, `degreeRa`, `degreesRa`, `rankine` | degree Rankine | `bu_rankine` | K = °Ra × 5/9 (linear) |
| `°De`, `degDe` | `degrDe`, `degreeDe`, `degreesDe`, `delisle` | degree Delisle | `bu_delisle` | K = 373.15 − °De × 2/3 (affine) |
| `°N`, `degN` | `degrN`, `degreeN`, `degreesN`, `newton_temperature` | degree Newton | `bu_newton_temp` | K = °N × 100/33 + 273.15 (affine) |
| `°Re`, `degRe` | `degrRe`, `degreeRe`, `degreesRe`, `reaumur` | degree Réaumur | `bu_reaumur` | K = °Re × 5/4 + 273.15 (affine) |
| `°Ro`, `degRo` | `degrRo`, `degreeRo`, `degreesRo`, `romer` | degree Rømer | `bu_romer` | K = (°Ro − 7.5) × 40/21 + 273.15 (affine) |

> Kelvin (`K`) is the SI base unit (§3.1). `Ra` not `R` — `R` is reserved for the röntgen (`bu_roentgen`).

#### Temperature Differences

Every unit above is a **scale**: `25 °C` is 298.15 K, and Bovnar converts it that way. A **difference** of 25 degrees is 25 K, and these are the units that say so.

| Symbol | Long forms | Name | Enum value | Conversion |
|--------|-----------|------|------------|------------|
| `ΔK`, `delta_K` | `deltaK`, `delta_kelvin`, `deltakelvin`, `Δ°C`, `delta_degC`, `deltadegC`, `delta_celsius`, `deltacelsius` | kelvin interval | `bu_delta_kelvin` | 1 K exactly (linear). `Δ°C` **is** this unit |
| `Δ°F`, `delta_degF` | `deltadegF`, `delta_fahrenheit`, `deltafahrenheit`, `Δ°Ra`, `delta_degRa`, `deltadegRa`, `delta_rankine`, `deltarankine` | Fahrenheit interval | `bu_delta_fahrenheit` | 5/9 K exactly (linear). `Δ°Ra` **is** this unit |
| `Δ°De`, `delta_degDe` | `deltadegDe`, `delta_delisle`, `deltadelisle` | Delisle interval | `bu_delta_delisle` | −2/3 K exactly (linear) |
| `Δ°N`, `delta_degN` | `deltadegN`, `delta_newton_temperature` | Newton interval | `bu_delta_newton_temp` | 100/33 K exactly (linear) |
| `Δ°Re`, `delta_degRe` | `deltadegRe`, `delta_reaumur`, `deltareaumur` | Réaumur interval | `bu_delta_reaumur` | 5/4 K exactly (linear) |
| `Δ°Ro`, `delta_degRo` | `deltadegRo`, `delta_romer`, `deltaromer` | Rømer interval | `bu_delta_romer` | 40/21 K exactly (linear) |

**Six rows, eight spellings.** The degree Celsius interval *is* the kelvin (SI Brochure 9th ed. §2.3.1) and the Rankine degree *is* the Fahrenheit degree, so `Δ°C` and `Δ°Ra` are aliases rather than units of their own — two units that had to compare equal and convert by exactly 1 would be a distinction with no content.

**They are ratio scales.** `.affine = false`, `.offset = 0.0`. That is what lets them compose where the scales cannot: `Δ°F/k~m` is a lapse rate and `Δ°Re^-1` an expansion coefficient, while `°F/k~m` and `°Re^-1` have no SI meaning at all (§9.4 — an affine scale is meaningful only alone at exponent 1).

**They carry their own quantity kind**, so:

```
ΔK   → K       error_unit_mismatch      the whole point
°C   → Δ°C     error_unit_mismatch      a reading is not an interval, and only
                                        the author can decide which one it was
Δ°C  → ΔK      factor 1                 the same unit
Δ°F  → ΔK      factor 5/9               exact; lossless only for multiples of 9
Δ°De → ΔK      factor −2/3              Delisle runs backwards
--si on 25 Δ°C                          25 ΔK, not 298.15 K
[<float:64,K> 1.0, <float:64,ΔK> 2.0]   error_array_element_type_mismatch
```

A bare array is homogeneous in its unit (spec [§7.4](03_bovnar_spec.md#74-element-homogeneity)), so the last line needs no rule of its own.

**Inside a compound the distinction does not arise, and is not made.** `W/(m²·ΔK)` and `W/(m²·K)` are the *same unit*, as are `ΔK/k~m` and `K/k~m`, and `ΔK^-1` and `K^-1`. The reason is the one above: an affine scale cannot appear in a compound at all, so a `K` there was already an interval and there is nothing to separate. The quantity kind is therefore significant only for a lone unit at exponent 1 — precisely where the affine offset was the hazard. Every U-value written before these units existed keeps its meaning.

> `Δ` is U+0394 (`0xCE 0x94`); every unit has ASCII spellings, formed as `delta_` plus the scale's own short form. There is deliberately no bare `delta_C` or `delta_F`: those read as a delta coulomb and a delta farad, and the scales themselves do not alias bare `C` or `F` either. `delta_K` is unambiguous because `K` is the kelvin and nothing else.

### 3.5 Pressure Units

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `atm`  | `atmosphere`, `atmospheres` | standard atmosphere | `bu_atmosphere` | 101325 Pa (exact) |
| `at`   | `atmosphere_technical` | atmosphere technical | `bu_atmosphere_technical` | 98066.5 Pa (= 1 kgf/cm²) |
| `mmHg` | — | millimetre of mercury | `bu_mmhg` | 133.322387415 Pa |
| `Torr` | `torr` | torr | `bu_torr` | 101325/760 Pa |
| `psi`  | — | pound-force per square inch | `bu_psi` | 6894.757293168362 Pa |
| `inHg` | `inch_hg`, `inch_mercury` | inch of mercury | `bu_inch_hg` | 3386.388640341 Pa (= 25.4 mmHg exactly) |
| `mH2O` | `metre_water`, `meter_water` | metre of water column | `bu_meter_water` | 9806.65 Pa (exact, conventional) |

> `mH2O` takes prefixes, so the spellings people actually write fall out of it: `c~mH2O` (or the
> compact `cmH2O`) is the centimetre of water a ventilator is set in, and `m~mH2O` the millimetre.
> The value is the conventional column — water at 1000 kg/m³ under standard gravity — the same kind
> of convention that fixes `mmHg` at exactly 133.322387415 Pa. A column measured at a stated
> temperature (UDUNITS' `water_4C`, QUDT's `IN_H2O`) is a *different* unit and is refused, not
> rounded onto this one.

### 3.6 Energy Units

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `cal`  | `calorie`, `calories` | thermochemical calorie | `bu_calorie` | 4.184 J (exact) |
| `Btu`  | `btu` | International Table BTU | `bu_btu` | 1055.05585262 J |
| `erg`  | `ergs` | erg | `bu_erg` | 10⁻⁷ J (exact) |
| `thm`  | `therm`, `therms` | US therm | `bu_therm` | 1.05480400×10⁸ J (exact) |
| `ft_lb` | `foot_pound`, `foot_pounds` | foot-pound | `bu_foot_pound` | 1.3558179483314003 J (= `lbf`·`ft`) |
| `cal_IT` | `calorie_IT` | International Table calorie | `bu_calorie_it` | 4.1868 J (exact) |
| `Btu_th` | `BTU_th`, `btu_th` | thermochemical BTU | `bu_btu_th` | 23722880951/22500000 J ≈ 1054.35026449 J |

> **Two calories and two BTUs, and the pairs cross over.** `cal` is the THERMOCHEMICAL calorie and
> `Btu` the INTERNATIONAL TABLE BTU, which is not an inconsistency but the two vocabularies' own
> defaults: UCUM's unqualified `cal` is thermochemical and its unqualified `[Btu]` is too, while
> UDUNITS' unqualified `calorie` is the IT one. Each pair is 0.067 % apart — dimensionally identical
> and numerically wrong if confused — so both members of both pairs are carried and every profile
> code lands on the one its own vocabulary says it means. Both BTUs are one pound of water raised
> one degree Fahrenheit, i.e. the corresponding calorie × 453.59237 g/lb × 5/9; that terminates for
> the IT calorie and does not for the thermochemical one, which is why `Btu_th` states an exact
> rational rather than a decimal.

> **`BTU` alias note:** `BTU` (all uppercase, three characters) is a valid alias for `bu_btu`. Because currencies require the `$` sigil, the bare token `BTU` is a physical-unit lookup and resolves to `bu_btu`; `Btu` and `btu` are also accepted. See §10.2 for the complete look-alike table.

### 3.7 Power Units

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `hp`   | `horsepower` | mechanical horsepower | `bu_horsepower` | 745.6998715822702 W (= 550 `ft_lb`/s) |
| `PS`   | `CV`, `metric_horsepower` | metric horsepower | `bu_metric_horsepower` | 735.49875 W (exact) |

### 3.8 Force Units

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `lbf`  | `pound_force` | pound-force | `bu_pound_force` | 4.4482216152605 N |
| `dyn`  | `dyne`, `dynes` | dyne | `bu_dyne` | 10⁻⁵ N (exact) |
| `kip`  | `kips` | kip (kilopound-force) | `bu_kip` | 4448.2216152605 N |
| `kgf`  | `kilogram_force` | kilogram-force | `bu_kilogram_force` | 9.80665 N (exact) |

### 3.9 Speed and Rotational Frequency Units

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `kn`   | `knot`, `knots` | knot | `bu_knot` | 1852/3600 m/s |
| `rpm`  | — | revolutions per minute | `bu_rpm` | 1/60 s⁻¹ |

> `kn` has dimension m·s⁻¹. `rpm` has dimension s⁻¹ (rotational frequency, not linear speed); it is grouped here by convention.

### 3.10 Volume Units

#### US Liquid Volume

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `gal`  | `gallon`, `gallons` | US liquid gallon | `bu_gallon` | 3.785411784×10⁻³ m³ (exact) |
| `qt`   | `quart`, `quarts` | US liquid quart | `bu_quart` | 9.46352946×10⁻⁴ m³ |
| `pt`   | `pint`, `pints` | US liquid pint | `bu_pint` | 4.73176473×10⁻⁴ m³ |
| `cup`  | `cups` | US cup | `bu_cup` | 2.365882365×10⁻⁴ m³ |
| `gi`   | `gill`, `gills` | US gill | `bu_gill` | 1.18294118250×10⁻⁴ m³ |
| `fl_oz`| `fluid_ounce`, `fluid_ounces` | US fluid ounce | `bu_fluid_ounce` | 2.95735295625×10⁻⁵ m³ |
| `tbsp` | `tablespoon`, `tablespoons` | US tablespoon | `bu_tablespoon` | 1.478676478125×10⁻⁵ m³ (exact) |
| `tsp`  | `teaspoon`, `teaspoons` | US teaspoon | `bu_teaspoon` | 4.92892159375×10⁻⁶ m³ |
| `bbl`  | `barrel`, `barrels` | petroleum barrel | `bu_barrel` | 0.158987294928 m³ |

> **`cup` disambiguation:** The canonical symbol for the US cup volume unit is `cup` (all lowercase). The Cuban Peso (ISO 4217 code 192) is written with the mandatory currency sigil as `$CUP`; the bare uppercase token `CUP` is `error_unit_illegal`. The two cannot be confused. See §10.3 for full details.

#### UK Imperial Volume

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `gal_uk` | `gallon_uk`, `gallons_uk` | imperial gallon | `bu_gallon_uk` | 4.54609×10⁻³ m³ (exact) |
| `qt_uk`  | `quart_uk`, `quarts_uk` | imperial quart | `bu_quart_uk` | 1136.5225×10⁻⁶ m³ |
| `pt_uk`  | `pint_uk`, `pints_uk` | imperial pint | `bu_pint_uk` | 568.26125×10⁻⁶ m³ |
| `gi_uk`  | `gill_uk`, `gills_uk` | imperial gill | `bu_gill_uk` | 1.420653125×10⁻⁴ m³ (exact) |
| `fl_oz_uk` | `fluid_ounce_uk`, `fluid_ounces_uk` | imperial fluid ounce | `bu_fluid_ounce_uk` | 28.4130625×10⁻⁶ m³ |

### 3.11 Area Units

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `ac`   | `acre`, `acres` | acre | `bu_acre` | 4046.8564224 m² (exact) |
| `barn` | `barns` | barn | `bu_barn` | 10⁻²⁸ m² (exact) |

### 3.12 Angle Units

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `arcmin` | `arcminute`, `arcminutes` | arcminute | `bu_arcminute` | π/10800 rad |
| `arcsec` | `arcsecond`, `arcseconds` | arcsecond | `bu_arcsecond` | π/648000 rad |
| `grad`   | `gradian`, `gradians`, `gon` | gradian | `bu_grad` | π/200 rad |
| `rev`    | `turn`, `revolution`, `revolutions`, `turns` | revolution | `bu_revolution` | 2π rad |

> Angle is one **shared** quantity kind, so `°` → `rad` works and only the factor
> is irrational. What the kind stops is an angle drifting into a plain count:
> `rev/min` is an angular rate and `rpm` a cycle rate, and they differ by exactly
> 2π. `sr` carries the kind at weight 2, because a steradian *is* `rad²` — and so
> do the photometric units built on it (`lm` = cd·sr, `lx` = lm/m², `ph` = lm/cm²;
> see the note under §3.2).

### 3.13 CGS Units

| Symbol | Long forms | Name | Enum value | SI equivalent |
|--------|-----------|------|------------|---------------|
| `P`    | `poise`, `poises` | poise (dynamic viscosity) | `bu_poise` | 0.1 Pa·s |
| `St`   | `stokes`, `stoke` | stokes (kinematic viscosity) | `bu_stokes` | 10⁻⁴ m²·s⁻¹ |
| `G`    | `gauss` | gauss (magnetic flux density) | `bu_gauss` | 10⁻⁴ T |
| `Mx`   | `maxwell`, `maxwells` | maxwell (magnetic flux) | `bu_maxwell` | 10⁻⁸ Wb |
| `Oe`   | `oersted`, `oersteds` | oersted (magnetic field strength) | `bu_oersted` | 1000/(4π) A/m |
| `sb`   | `stilb`, `stilbs` | stilb (luminance) | `bu_stilb` | 10⁴ cd/m² |
| `ph`   | `phot`, `phots` | phot (illuminance) | `bu_phot` | 10⁴ lx |
| `Gal`  | `galileo`, `galileos` | galileo (acceleration) | `bu_galileo` | 10⁻² m/s² |

> **`sb` and `ph` are not the same quantity**, though the SI dimension vector says
> they are: both reduce to cd·m⁻² and both are 10⁴ of their SI counterpart. The
> stilb is a *luminance* (cd/cm²), the phot an *illuminance* (lm/cm²), and a lumen
> is a candela-**steradian**. Bovnar carries that steradian as a quantity kind, so
> `ph` converts with `lx` and `sb` with `cd/m²`, and neither converts with the
> other — see §3.12 and the note under §3.2.

### 3.14 Radiation Units

| Symbol | Long forms | Name | Enum value | SI equivalent |
|--------|-----------|------|------------|---------------|
| `Ci`   | `curie`, `curies` | curie (radioactivity) | `bu_curie` | 3.7×10¹⁰ Bq |
| `R`    | `roentgen`, `roentgens` | röntgen (radiation exposure) | `bu_roentgen` | 2.58×10⁻⁴ C/kg |
| `rem`  | `rems` | rem (dose equivalent) | `bu_rem` | 10⁻² Sv |

### 3.15 Logarithmic Units

| Symbol | Long forms | Name | Enum value | Notes |
|--------|-----------|------|------------|-------|
| `Np`   | `neper`, `nepers` | neper | `bu_neper` | dimensionless logarithmic ratio |
| `dB`   | `decibel`, `decibels` | decibel | `bu_decibel` | dimensionless logarithmic ratio |

> **Neper and decibel do not convert into each other.** `bvn_units_compatible`
> reports `false` for the pair, and every conversion entry point refuses it.
>
> A logarithmic level is not a linear quantity: 20 dB is a ratio of 100, not
> twice the ratio 10 dB names. Relating two logarithmic scales is a change of
> base, which the multiply-by-a-factor model every conversion entry point is
> built on cannot express — so bovnar gives `Np`, `dB` and `pH` a quantity kind
> each and refuses every pair, rather than applying a factor that is only ever
> right by coincidence.
>
> ISO 80000-3 does state 1 Np = 8.685889… dB, for a level referred consistently
> to the same kind of quantity. Two things stop that from being a conversion
> this library can perform. It is a relation between *levels*, so it does not
> compose with the exponents and prefixes a `value_unit_t` carries; and `dB` is
> written in practice against both the power convention (10·log₁₀) and the field
> convention (20·log₁₀), with nothing in the annotation recording which — so the
> number a reader would need is not determined by the unit alone.
>
> The same reasoning keeps a level out of a plain number: `dB` → dimensionless is
> refused too.

### 3.16 Electrical Power Units

| Symbol | Long forms | Name | Enum value | Notes |
|--------|-----------|------|------------|-------|
| `var`  | `vars` | var (volt-ampere reactive) | `bu_var` | reactive power; same SI dimension as W |
| `VA`   | `volt_ampere`, `volt_amperes` | volt-ampere | `bu_volt_ampere` | apparent power; same SI dimension as W |

> **Watt, var, and VA:** All three carry the same SI dimensional signature (kg·m²·s⁻³). `bvn_units_compatible` returns `true` when comparing them. They are kept as distinct base units because they represent distinct AC power interpretations.

### 3.17 Digital Units

| Symbol | Long forms | Name | Enum value |
|--------|-----------|------|------------|
| `b`    | `bit`, `bits` | bit | `bu_bit` |
| `B`    | `byte`, `bytes`, `Byte`, `Bytes` | byte | `bu_byte` |

> **Bit and byte do not convert into each other.** They are separate quantity
> kinds, so `b` → `B` is refused rather than divided by eight. A byte is eight
> bits *on the wire*, but a value annotated in bytes and one annotated in bits
> are usually not the same measurement — a link rate quoted in `M~b/s` and a file
> size in `M~B` — and silently trading one for the other is the class of error
> this format exists to prevent. Convert explicitly in the application if that
> is genuinely what you mean.
>
> Prefixes still work within each: `Ki~B` → `B` is 1024, `M~b` → `k~b` is 1000.
> Both units take IEC binary prefixes as well as SI ones, and SI prefixes on them
> are restricted to kilo and above (§6.3).

### 3.18 Textile Linear Density

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `tex`  | — | tex | `bu_tex` | 1×10⁻⁶ kg/m (ISO 1144) |
| `den`  | `denier`, `deniers` | denier | `bu_denier` | 1/9000000 kg/m |

### 3.19 US Apothecary / Dry Volume

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `fl_dr`  | `fluid_dram`, `fluid_drams` | US fluid dram | `bu_fluid_dram` | 3.6966911953125×10⁻⁶ m³ |
| `minim`  | `minims` | US minim | `bu_minim` | 6.16115199218750×10⁻⁸ m³ |
| `pk`     | `peck`, `pecks` | US dry peck | `bu_peck` | 8.80976754172×10⁻³ m³ |
| `bsh`    | `bushel`, `bushels` | US bushel | `bu_bushel` | 3.523907016688×10⁻² m³ |

> `minim` (not `min`, which is the minute) avoids ambiguity.

### 3.20 Old German Units

Old German units fall into metric-compatible units (still in use in DACH regions) and historical pre-metric Prussian units. The prefix `pr` is reserved for Prussian symbols. No German unit accepts any non-trivial SI or IEC prefix; `bvn_prefix_unit_valid` rejects any non-`si_none`/`iec_none` prefix for every unit from `bu_pfund` through `bu_scheffel`.

#### Metric-Compatible German Units — Mass

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `Pfd`  | `pfund`, `pfunds` | Pfund | `bu_pfund` | 0.5 kg (exact) |
| `Ztr`  | `zentner` | Zentner | `bu_zentner` | 50 kg (exact) |
| `dz`   | `doppelzentner` | Doppelzentner | `bu_doppelzentner` | 100 kg (exact) |
| `lot`  | `lots` | Lot | `bu_lot` | 15.625×10⁻³ kg (exact) |

#### Historical German Units — Length (Prussian)

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `prln` | `prussian_line`, `linie` | Prussian line | `bu_prussian_line` | 313853/144000000 m ≈ 2.1795347×10⁻³ m |
| `prz`  | `prussian_zoll`, `zoll` | Prussian Zoll | `bu_prussian_zoll` | 313853/12000000 m ≈ 2.6154417×10⁻² m |
| `prf`  | `prussian_fuss`, `preussischer_fuss` | Prussian Fuß | `bu_prussian_fuss` | 3.13853×10⁻¹ m |
| `elle` | `prussian_elle`, `preussische_elle` | Prussian Elle | `bu_prussian_elle` | 6.66937625×10⁻¹ m (exact; 25½ Zoll) |
| `rute` | `prussian_rute`, `preussische_rute` | Prussian Rute | `bu_prussian_rute` | 3.766236 m (exact) |
| `klafter` | `prussian_klafter` | Klafter | `bu_klafter` | 1.883118 m (exact) |
| `dt_mi` | `deutsche_meile`, `german_mile` | Geographische Meile | `bu_german_mile` | 7420.44 m |

#### Historical German Units — Area (Prussian)

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `morgen` | `prussian_morgen` | Morgen (Prussian) | `bu_morgen` | 2553.21604938528 m² (exact) |

#### Historical German Units — Volume (Prussian)

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `schffl` | `scheffel`, `prussian_scheffel` | Scheffel (Prussian) | `bu_scheffel` | 54.961×10⁻³ m³ |

> Every Prussian unit here reproduces its historical definition from the 1816 Fuß (0.313853 m):
> Zoll = Fuß/12, Linie = Zoll/12, Rute = 12 Fuß, Klafter = 6 Fuß, Elle = 25½ Zoll, Morgen = 180
> square Ruten. `test_unit_factors_derived.py` checks each of them against that definition.

> The German units carry ids **100133–100145**, inside the native unit block like every other physical unit — see §12.1 for the block layout. They used to sit at 348–360, appended past the whole currency range, because the id space was one flat counter and the currencies had been dropped into the middle of it. The space is blocked now, so a unit's id no longer says anything about when it was added.

### 3.21 Additional Length Units

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `ftUS` | `survey_foot` | US survey foot | `bu_survey_foot` | 1200/3937 m ≈ 0.304800609… m |
| `lea`  | `league`, `leagues` | statute league | `bu_league` | 4828.032 m (= 3 statute miles) |
| `cbl`  | `cable`, `cables` | cable length | `bu_cable` | 185.2 m |
| `hand` | `hands` | hand | `bu_hand` | 0.1016 m (= 4 in, exact) |

> `ftUS` (US survey foot) differs from `ft` (international foot, 0.3048 m exactly) by about 2 ppm. Used in US geodetic surveying.

### 3.22 Additional Mass Units

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `qntl` | `quintal`, `quintals` | quintal | `bu_quintal` | 100 kg (exact) |
| `sc`   | `scruple`, `scruples` | apothecary scruple | `bu_scruple` | 1.2959782×10⁻³ kg (= 20 grains) |

### 3.23 Acceleration

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `gn`   | `standard_gravity` | standard gravity | `bu_standard_gravity` | 9.80665 m·s⁻² (exact, CGPM 1901) |

### 3.24 Signal Rate

| Symbol | Long forms | Name | Enum value | Notes |
|--------|-----------|------|------------|-------|
| `Bd`   | `baud`, `bauds` | baud | `bu_baud` | 1 symbol/s = 1 s⁻¹ (ITU-T V.662) |

### 3.25 Ratio and Proportion Units

Dimensionless scaling factors. A value carrying one of these units reduces to the
numeric value multiplied by the factor in the canonical (dimensionless) base
representation — e.g. `5 %` ≡ `0.05`, `250 ppm` ≡ `0.00025`. Like the other
dimensionless ratios they carry an empty dimension vector, but unlike `rad`/`sr`
they do **not** accept SI or IEC prefixes (a prefixed `%` is meaningless).

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `%`   | `percent` | per cent | `bu_percent` | 10⁻² |
| `‰`   | `per_mille` | per mille | `bu_per_mille` | 10⁻³ |
| `‱`   | `per_myriad` | per myriad | `bu_per_myriad` | 10⁻⁴ |
| `pcm` | `per_cent_mille` | per cent mille | `bu_per_cent_mille` | 10⁻⁵ |
| `ppm` | — | parts per million | `bu_ppm` | 10⁻⁶ |
| `ppb` | — | parts per billion | `bu_ppb` | 10⁻⁹ |
| `pptr` | `parts_per_trillion`, `pptv` | parts per trillion | `bu_ppt` | 10⁻¹² |
| `ppq` | `parts_per_quadrillion`, `ppqv` | parts per quadrillion | `bu_ppq` | 10⁻¹⁵ |

> **Why `pptr` and not `ppt`.** Two independent reasons, and either alone would
> settle it. `ppt` already resolved — as the compact form of `p~pt`, the
> picopint — so claiming it as an alias would have taken a spelling away from the
> unit that held it, which §4.3's guarantee forbids and `gen_units.py` refuses at
> build time. And `ppt` is ambiguous in the field: parts per **thousand** in some
> industries, parts per **trillion** in atmospheric chemistry, a factor of 10⁹
> apart. UCUM splits the same ambiguity the same way — `[ppth]` for per thousand,
> `[pptr]` for per trillion — so the symbol here is borrowed rather than
> invented. The compact token `ppt` is refused outright (§4.3); write `pptr`,
> `‰`, or `p~pt` if the picopint really is what you meant.
>
> `pptv` and `ppqv`, the "by volume" spellings, are accepted as aliases because
> UDUNITS-2 defines them as the same 10⁻¹² and 10⁻¹⁵. Both profiles that name
> these units — UCUM's `[pptr]` and UDUNITS' `ppt`/`pptv`/`ppq`/`ppqv` — were
> refused for want of a native target until these rows existed.

### 3.26 Named Speed Units

Speeds people write as one token. `mi/h` and `k~m/h` express the same quantities
as compounds and remain valid; these are separate base units, not shorthands the
parser expands, so an annotation of `mph` does **not** reconcile with an inline
`mi/h` (the comparison is structural — see §2.2). Neither accepts a prefix: they
already carry one, or have no meaningful prefixed form. `kn` (the knot, §3.9)
belongs to the same family.

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `mph` | — | mile per hour | `bu_mile_per_hour` | 0.44704 m·s⁻¹ (exact) |
| `kph` | `kmh` | kilometre per hour | `bu_kilometer_per_hour` | 5/18 m·s⁻¹ (exact; the decimal does not terminate) |

### 3.27 Acidity

The pH scale: dimensionless by construction, being a negative decimal logarithm
of hydrogen-ion activity. It carries a factor of 1 and accepts **no** prefix — a
milli-pH is not a quantity. It is modelled for the same reason `dB` and `Np` are:
a logarithmic scale is still a statement about what the number means, and `7.2`
alone is not.

Because the scale is logarithmic, the value is a label, not something to sum or
average — the same caveat that applies to `dB` and `Np`.

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `pH` | — | pH (acidity) | `bu_ph_scale` | 1 (dimensionless) |

> **`pH` vs `p~H`:** the two differ by one character and by seven orders of
> dimension. `pH` is the acidity scale; `p~H` is the picohenry. Case matters too:
> `ph` is the phot (§3.13). This is exactly why acidity had to become a unit —
> without it, `pH` resolves as a compact prefixed henry.

### 3.28 Water Hardness

Six scales for one physical quantity: the concentration of dissolved
alkaline-earth ions (Ca²⁺, Mg²⁺). Each is *defined* as a mass of a reference compound per litre,
but the scales use **different** compounds — `°dH` counts CaO, most of the others CaCO₃, `°rH`
counts Ca — so mass concentration is not their common ground. Amount concentration is: every scale
carries mol·m⁻³ (which is exactly mmol·L⁻¹), converts into every other, and into `m~mol/L`.

None of them takes a prefix. **Millimoles per litre needs no unit of its own** — it is the compound
`m~mol/L` (or `mmol/L`).

| Symbol | Long form | Name | Enum value | Defined as | Factor (mol·m⁻³ = mmol·L⁻¹) |
|--------|-----------|------|------------|------------|------------------------------|
| `°dH` | `german_hardness` | German degree | `bu_german_hardness` | 10 mg CaO / L | 0.178326 |
| `°e`, `°Clark` | `english_hardness`, `clark_degree` | English (Clark) degree | `bu_english_hardness` | 1 grain CaCO₃ / imperial gallon | 0.142415 |
| `°fH` | `french_hardness` | French degree | `bu_french_hardness` | 10 mg CaCO₃ / L | 0.099914 |
| `°rH` | `russian_hardness` | Russian degree | `bu_russian_hardness` | 1 mg Ca / L | 0.024951 |
| `°aH` | `american_hardness` | American degree | `bu_american_hardness` | 1 mg CaCO₃ / L | 0.009991 |
| `gpg` | `grains_per_gallon` | grains per US gallon | `bu_grains_per_gallon` | 1 grain CaCO₃ / US gallon | 0.171034 |
| `val` | `vals`, `eq` | equivalent (water analysis) | `bu_val` | ½ mol — see below | 0.5 (as `mol`, not a concentration) |

Reading across, with 1 mmol·L⁻¹ as the reference:

| 1 mmol/L equals | `°dH` | `°e` | `°fH` | `°rH` | `°aH` | `gpg` | `mval/L` |
|-----------------|-------|------|-------|-------|-------|-------|----------|
| | 5.6077 | 7.0217 | 10.0086 | 40.078 | 100.086 | 5.8468 | 2.000 |

> **`gpg` is a unit, `gr/gal` is a compound, and they are not the same thing.** `gpg` is an amount
> concentration and converts with the other hardness scales; `gr/gal` (grain per US gallon) is a
> *mass* concentration and deliberately does not. Both spellings stay valid.

> **`val` is the equivalent as *water analysis* uses it.** The ions counted are divalent, so one
> equivalent is half a mole and `m~val/L` = 0.5 mmol/L, which is what the hardness tables state.
> An equivalent of a *monovalent* species is one mole — Bovnar cannot know the species from the
> unit, so for that case write the amount directly (`m~mol/L`). `eq` is accepted as a spelling of
> `val` (below) and means the same divalent equivalent — it is **not** UCUM's generic equivalent of
> one mole: native `meq/L` is half of `ucum:meq/L`. A clinical `meq/L` of a monovalent ion (Na⁺,
> K⁺, Cl⁻) is one millimole per litre, so write it as `m~mol/L` or as `ucum:meq/L`.
>
> This is settled, not provisional: `eq` stays a spelling of `val` and will not be redefined as a
> mole, because that would silently halve every `meq/L` written in the water-analysis sense.
> Inside a `ucum:` unit, `eq` is UCUM's and means one mole (see
> [`11_bovnar_unit_profiles.md`](11_bovnar_unit_profiles.md)).

> **Exactness.** Every degree is derived from a molar mass (IUPAC 2021: Ca 40.078, C 12.011,
> O 15.999 g·mol⁻¹), so the factors are measurement-derived rather than exact rationals. They carry
> `.exact = false`, which means a **read-time lossless conversion** (`want_unit`, §1.10 of the
> read/write API) refuses them with `error_unit_inexact` (47) rather than inventing precision —
> the same answer it gives for a π-based angle. `bvn_unit_convert_value` and
> `bvn_unit_to_si_factor`, which work in double precision, convert normally.

> **`mval/L` and `meq/L` are one unit.** `eq` is an accepted spelling of `val`, so the German and
> English conventions both work and both carry the divalent caveat above.

> **`dH` is not `°dH`.** Without the degree sign the token is the decihenry, and it stays that way.
> Likewise water chemistry writes the American scale as "ppm"; Bovnar's `ppm` is the dimensionless
> 10⁻⁶ and is *not* interchangeable with `°aH`. See [`07_bovnar_unit_ambiguities.md`](07_bovnar_unit_ambiguities.md).

### 3.29 Conductivity and Dissolved Solids

Two quantities that water data quotes constantly and that need **no unit of their own** — the
existing tables already say them exactly.

| Quantity | Write | Notes |
|----------|-------|-------|
| Electrical conductivity (EC) | `µS/cm`, `uS/cm`, `mS/cm`, `dS/m`, `S/m` | siemens per length; `dS/m` is the soil-salinity convention and equals `mS/cm` |
| … in pre-SI spelling | `µmho/cm`, `mmho/cm` | `mho`, `mhos` and `℧` (U+2127) are accepted spellings of the siemens |
| Total dissolved solids (TDS) | `mg/L`, `µg/L`, `g/L`, `k~g/m³` | mass concentration |
| Resistivity (the reciprocal) | `MΩ·cm`, `M~Ω·c~m` | ultrapure-water convention |

One quantity does get a unit of its own, because the hydroponic scale is a
rescaling nothing else in the table expresses:

| Symbol | Long form | Name | Enum value | Method | Prefixes |
|--------|-----------|------|------------|--------|----------|
| `CF` | `conductivity_factor` | conductivity factor | `bu_conductivity_factor` | EC in mS/cm × 10 | no |

`CF` carries the dimensions of conductivity, so unlike the turbidity scales
below it *does* convert: 1 CF = 0.1 mS/cm = 100 µS/cm, exactly, and into `S/m`
and `dS/m` with it. Only the uppercase spelling is the conductivity factor —
`cF` is the centifarad.

> **"ppm" for TDS.** Water data writes TDS in "ppm", meaning milligrams per litre. Bovnar's `ppm`
> is the dimensionless 10⁻⁶ — for a dilute aqueous solution at 1 kg/L the two are *numerically*
> the same, so writing `ppm` is defensible here in a way it is not for hardness (§3.28), but the
> dimensions differ: `ppm` is a ratio, `mg/L` is a concentration, and they do not convert into one
> another. Pick the one you mean.

> **TDS meter scales.** A conductivity meter reading "TDS ppm" applies a conversion factor to EC
> (the 500/442/700 scales). That factor is a property of the instrument and the water, not of a
> unit — record the EC in `µS/cm` and the factor separately, or record the TDS as `mg/L`.

### 3.30 Turbidity and Salinity

Six scales defined by a **measurement method** rather than by a physical quantity. They are
dimensionless, and each carries its own quantity kind, so none of them converts to another or to a
plain number — the conversion a factor cannot express is refused instead of guessed.

| Symbol | Long form | Name | Enum value | Method | Prefixes |
|--------|-----------|------|------------|--------|----------|
| `NTU` | `nephelometric_turbidity` | nephelometric turbidity unit | `bu_turbidity_ntu` | white light, 90° detector (EPA 180.1) | yes |
| `FNU` | `formazin_nephelometric` | formazin nephelometric unit | `bu_turbidity_fnu` | near-infrared 860 nm, 90° detector (ISO 7027) | yes |
| `FTU` | `formazin_turbidity` | formazin turbidity unit | `bu_turbidity_ftu` | formazin-calibrated, geometry **unstated** (ISO 7027:1984) | yes |
| `FAU` | `formazin_attenuation` | formazin attenuation unit | `bu_turbidity_fau` | attenuation at 0°, in the transmitted beam (ISO 7027) | yes |
| `JTU` | `jackson_turbidity` | Jackson turbidity unit | `bu_turbidity_jtu` | visual candle turbidimeter (historical) | no |
| `PSU` | `practical_salinity` | practical salinity unit | `bu_practical_salinity` | PSS-78 conductivity ratio | no |

None of the five turbidity scales converts to any other:

| | `NTU` | `FNU` | `FTU` | `FAU` | `JTU` |
|---|---|---|---|---|---|
| **`NTU`** | ✓ | — | — | — | — |
| **`FNU`** | — | ✓ | — | — | — |
| **`FTU`** | — | — | ✓ | — | — |
| **`FAU`** | — | — | — | ✓ | — |
| **`JTU`** | — | — | — | — | ✓ |

> **One kind per method, and that is the whole content of the table.** The formazin-calibrated
> scales (`NTU`, `FNU`, `FTU`) are numerically equal *on a formazin standard* — which is exactly the
> trap. On real water white light and near-infrared respond differently to particle size and colour,
> so the number alone does not say what was measured; `FTU` says even less, since its geometry is
> unstated by definition. `FAU` is not the same optical quantity at all: it measures how much light
> the sample removes from the beam, not how much it scatters sideways, and it is the instrument of
> choice above ~40 FNU where nephelometry saturates. `JTU` is the visual candle method, whose
> published equivalence to formazin ("1 JTU ≈ 1 NTU") holds near 40 units and nowhere else, being
> nonlinear and sample-dependent. Bovnar refuses every one of these conversions rather than implying
> a factor exists — report the method.
>
> `NTU`, `FNU`, `FTU` and `FAU` accept prefixes (`m~NTU` is real in ultrapure-water work). `JTU`
> does not: the candle method cannot resolve below roughly 25 JTU, so a milli-JTU is not a
> measurement.
>
> Watch the case. `fau` is the femto-astronomical-unit, `cF` the centifarad — the turbidity and
> conductivity scales are uppercase only. See [`07_bovnar_unit_ambiguities.md`](07_bovnar_unit_ambiguities.md).

> **PSU is not per-mille.** Practical salinity is a conductivity ratio, so it is dimensionless by
> construction and `PSU` is a label rather than a unit — SI-minded texts write *S*_P = 35 with no
> unit at all. It is **not** a mass fraction: *S*_P 35 corresponds to about 35.165 g/kg absolute
> salinity, so equating it with `‰` or `g/kg` is wrong by roughly half a percent. For absolute
> salinity write the mass fraction directly: `g/k~g`. The scale is bounded by construction, so it
> takes no prefix.

### 3.31 Sentinel Value

`bu_none` (value `0`) is the internal representation of "no base unit", used for the `no_unit` keyword and as the default when no unit annotation is present.

---

### 3.32 Units the Unit Profiles Needed

Every unit in this section was, until it was added, the **sole** reason a run of UCUM, UDUNITS-2,
QUDT, OM or UN/ECE codes had to be refused as `error_unit_profile_unsupported`: the publisher
defines the unit, the value is exactly stateable, and there was no native unit to translate onto.
None of them is new physics. All are exact — the ones whose value is not a terminating decimal in
SI state a rational in `units.bvnr` rather than the repr of a double.

#### US survey lengths

`bu_survey_foot` (§3.21) has been in the registry all along and nothing was built on it, so every
survey length above the foot had to be refused. Each below is an exact rational multiple of the
survey foot's 1200/3937 m.

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `inUS` | `survey_inch` | US survey inch | `bu_survey_inch` | 100/3937 m |
| `ydUS` | `survey_yard` | US survey yard | `bu_survey_yard` | 3600/3937 m |
| `fathUS` | `survey_fathom` | US survey fathom | `bu_survey_fathom` | 7200/3937 m |
| `rdUS` | `survey_rod` | US survey rod (pole, perch) | `bu_survey_rod` | 19800/3937 m |
| `chUS` | `survey_chain` | US survey chain (Gunter's) | `bu_survey_chain` | 79200/3937 m |
| `lkUS` | `survey_link` | US survey link | `bu_survey_link` | 792/3937 m |
| `furUS` | `survey_furlong` | US survey furlong | `bu_survey_furlong` | 792000/3937 m |
| `miUS` | `survey_mile`, `survey_miles` | US survey (statute) mile | `bu_survey_mile` | 6336000/3937 m |
| `acUS` | `survey_acre`, `survey_acres` | US survey acre | `bu_survey_acre` | 43 560 `ftUS`² = 62726400000/15499969 m² |

> **The survey foot is 2 ppm longer than the international foot**, and the survey acre 4 ppm larger
> than the international acre — an area being a length squared. Small enough to ignore and never
> small enough to be right: on a section of land the acre difference is about ten square metres.
> The survey foot was withdrawn for new work at the end of 2022, which is a reason to read it
> carefully rather than to refuse it, since US land records, state-plane coordinates and a century
> of engineering drawings are written in it.

#### Typographic lengths

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `pnt` | `point`, `points` | DTP point (1/72 in) | `bu_point` | 127/360000 m |
| `pca` | `pica`, `picas` | pica (12 points) | `bu_pica` | 127/30000 m |
| `lne` | `line`, `lines` | line (1/12 in) | `bu_line` | 127/60000 m |

> The symbols are deliberately **not** `pt` and `ln`. `pt` is the pint and has been since before
> these existed; a length answering to it would change what an existing spelling means, which
> §10 forbids and which `gen_units.py` now refuses at build time. UCUM's own bracketed spellings
> `[pnt]`, `[pca]` and `[lne]` name them unambiguously, so the symbols follow those. The
> **printer's** point (0.013837 in) is a different unit and has a row of its own, `pnt_pr`, at the end of §3.32.

#### US dry volumes, and the trade measures built on feet

A dry quart is **16 per cent larger** than the liquid quart `qt`. The peck (`pk`) and bushel (`bsh`)
in §3.19 have always been the dry ones; the gallon, quart and pint were only the liquid ones, so a
document that meant dry had no way to say so.

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `gal_dry` | `dry_gallon`, `dry_gallons` | US dry gallon | `bu_dry_gallon` | 268.8025 in³ = 4.40488377086×10⁻³ m³ |
| `qt_dry` | `dry_quart`, `dry_quarts` | US dry quart | `bu_dry_quart` | 1.101220942715×10⁻³ m³ |
| `pt_dry` | `dry_pint`, `dry_pints` | US dry pint | `bu_dry_pint` | 5.506104713575×10⁻⁴ m³ |
| `fbm` | `board_foot`, `board_feet` | board foot (144 in³) | `bu_board_foot` | 2.359737216×10⁻³ m³ |
| `cord` | `cord`, `cords` | cord (128 ft³) | `bu_cord` | 3.624556363776 m³ |
| `ac_ft` | `acre_foot`, `acre_feet` | acre-foot (survey) | `bu_survey_acre_foot` | `acUS`·`ftUS` = 1233.4892384681489 m³ |

> `ac_ft` is the acre-foot on the **survey** acre and foot, which is what UDUNITS, OM and every US
> water agency mean by it. The international acre-foot is `ac·ft` = 1233.48183754752 m³ — 5.8 ppm
> away, and a different unit rather than a rounding.

#### Permeability, energy, power, ozone and one very short time

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `darcy` | `darcy`, `darcys`, `darcies` | darcy (permeability) | `bu_darcy` | 1/1013250000000 m² |
| `thm_ec` | `therm_EC` | EC therm (10⁵ `Btu`) | `bu_therm_ec` | 1.05505585262×10⁸ J (exact) |
| `ton_ref` | `refrigeration_ton`, `ton_of_refrigeration` | ton of refrigeration | `bu_refrigeration_ton` | 12 000 `Btu`/h = 52752792631/15000000 W |
| `DU` | `dobson`, `dobson_unit`, `dobson_units` | Dobson unit (ozone column) | `bu_dobson` | 4.462×10⁻⁴ mol·m⁻² (exact) |
| `shake` | `shake`, `shakes` | shake | `bu_shake` | 10⁻⁸ s (exact) |

> `darcy` takes prefixes, so the millidarcy every reservoir report is written in is `m~darcy` (or
> compactly `mdarcy`). `thm_ec` is the therm European gas billing uses; native `thm` is the US
> therm, 0.24 per cent away, and no SI prefix reaches 10⁵ — which is why it needed a unit of its
> own rather than a prefixed `Btu`. `shake` exists for the same reason at the other end: there is
> no SI prefix at 10⁻⁸.

---

#### Nine more, from two and three vocabularies at once

Each was refused across UCUM, UDUNITS-2, QUDT and OM for one reason only — no native unit of the
magnitude. One publisher naming a unit is a request; **two or three naming it independently at the
same value** is evidence, and every factor below is one all of its publishers agree on to the last
digit they state.

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `pnt_pr` | `printers_point` | printer's point | `bu_printers_point` | 0.0003514598 m (exact, = 0.013837 `in`) |
| `pca_pr` | `printers_pica` | printer's pica | `bu_printers_pica` | 0.0042175176 m (exact, = 12 `pnt_pr`) |
| `hp_E` | `electric_horsepower` | electric horsepower | `bu_horsepower_electric` | 746 W (exact) |
| `hp_B` | `boiler_horsepower` | boiler horsepower | `bu_horsepower_boiler` | 9809.5 W |
| `abV` | `abvolt` | abvolt (CGS-EMU) | `bu_abvolt` | 10⁻⁸ V (exact) |
| `AT` | `assay_ton` | assay ton (short) | `bu_assay_ton` | 175/6000 kg ≈ 0.029166667 kg |
| `bsh_uk` | `bushel_uk` | imperial bushel | `bu_bushel_uk` | 0.03636872 m³ (exact, = 8 `gal_uk`) |
| `clo` | — | clo | `bu_clo` | 0.155 K·m²/W (exact) |
| `debye` | — | debye | `bu_debye` | 1/299792458000000000000000000000 C·m ≈ 3.3356410×10⁻³⁰ C·m |

> **Each is a near neighbour of a unit already here, which is why it needed its own row rather than
> an alias.** `pnt` is the DTP point (¹⁄₇₂ in) and `pnt_pr` the printer's point (0.013837 in), 0.37 %
> apart; `hp` is the mechanical horsepower, `PS` the metric one and `hp_E` the electric 746 W, while
> `hp_B` is thirteen times any of them; `bsh` is the US **dry** bushel and `bsh_uk` the imperial one,
> 3.2 % larger. Mapping any of these onto its neighbour is the error doc/11 §6.3 warns about —
> dimensionally perfect, inside anything a later check would notice, and wrong.
>
> `abV` needed a unit rather than a prefix because there is no decade prefix at 10⁻⁸ and prefixes do
> not stack. `AT` and `debye` are stated as exact rationals: the assay ton is 175/6 g, chosen so that
> milligrams recovered from one assay ton read as troy ounces per short ton, and the debye has been
> exactly 10⁻²¹/c C·m since the 2019 SI fixed c. Their publishers state 7- and 6-digit roundings of
> those; the rational is what they are rounding.

#### And the thirty-five the same sweep found next

Grouping every profile refusal by (dimension, value) turned up 35 groups **two or more vocabularies
define**. The nine above were the first pass; these are the rest that name a real unit. What is left
out is a *constant* rather than a unit (UCUM's `[pi]`), a value only one publisher states, or a
dimensionless "relative permeability" that is a number.

**Where the publishers disagree, the exact definition decides.** The CGS-ESU units are built on `c`,
which the 2019 SI fixed, so every one is an exact rational and the publishers' five- to seven-digit
decimals are roundings of it — which is why `statΩ` is 22468879468420441/25000 here and
898 755 400 000, 898 755 200 000 and 898 760 000 000 in UDUNITS, OM and QUDT. The sidereal hour,
minute and second are derived from the sidereal **day** rather than taken from the three slightly
inconsistent decimals the publishers state for them, so the four stay consistent with each other.
The π-based rows carry `.exact = false`, as the oersted and the parsec do: a value that is not a
rational cannot be converted losslessly, and saying so is what that flag is for.

**`yr` is still the Julian year and `mo` still a twelfth of it.** Naming the others does not make the
short spellings ambiguous — it gives the other calendars somewhere to go, which is what lets a
UDUNITS or CF document that says `year` be read at all. It was refused outright before.

**Calendar and sidereal time**

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `yr_trop` | `tropical_year` | tropical year | `bu_year_tropical` | 31 556 925.9747 s (the tropical year UDUNITS and CF carry) |
| `yr_greg` | `gregorian_year` | Gregorian year | `bu_year_gregorian` | 31 556 952 s (exact, = 365.2425 d) |
| `yr_sid` | `sidereal_year` | sidereal year | `bu_year_sidereal` | 31 558 149.7632 s |
| `yr_com` | `common_year` | common year | `bu_year_common` | 31 536 000 s (exact, = 365 d) |
| `mo_syn` | `synodal_month` | synodal month | `bu_month_synodal` | 2 551 442.976 s (the synodal, lunar month) |
| `mo_trop` | `tropical_month` | tropical month | `bu_month_tropical` | 2 360 584.6848 s |
| `mo_greg` | `gregorian_month` | mean Gregorian month | `bu_month_gregorian` | 2 629 746 s |
| `mo_sid` | `sidereal_month` | sidereal month | `bu_month_sidereal` | 2 360 591.5104 s |
| `d_sid` | `sidereal_day` | sidereal day | `bu_day_sidereal` | 86 164.0905 s |
| `h_sid` | `sidereal_hour` | sidereal hour | `bu_hour_sidereal` | `d_sid`/24 = 3590.1704375 s |
| `min_sid` | `sidereal_minute` | sidereal minute | `bu_minute_sidereal` | `d_sid`/1440 |
| `s_sid` | `sidereal_second` | sidereal second | `bu_second_sidereal` | `d_sid`/86400 |

**CGS electrostatic (ESU)**

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `statV` | `statvolt` | statvolt | `bu_statvolt` | 149896229/500000 V = 299.792458 V (exact, = c×10⁻⁶) |
| `statA` | `statampere` | statampere | `bu_statampere` | 1/2997924580 A (exact) |
| `statC` | `statcoulomb`, `franklin` | statcoulomb (franklin) | `bu_statcoulomb` | 1/2997924580 C (exact) |
| `statF` | `statfarad` | statfarad | `bu_statfarad` | 25000/22468879468420441 F (exact) |
| `statΩ` | `statohm` | statohm | `bu_statohm` | 22468879468420441/25000 Ω ≈ 898 755 178 737 Ω (exact) |
| `statH` | `stathenry` | stathenry | `bu_stathenry` | 22468879468420441/25000 H (exact) |
| `statS` | `statsiemens`, `statmho` | statsiemens | `bu_statsiemens` | 25000/22468879468420441 S (exact) |
| `statWb` | `statweber` | statweber | `bu_statweber` | 149896229/500000 Wb = 299.792458 Wb (exact, = statV·s) |
| `statT` | `stattesla` | stattesla | `bu_stattesla` | 149896229/50 T = 2 997 924.58 T (exact, = statV·s/cm²) |

**Luminance**

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `Lmb` | `lambert` | lambert | `bu_lambert` | 10⁴/π cd·m⁻² |
| `apostilb` | `blondel` | apostilb (blondel) | `bu_apostilb` | 1/π cd·m⁻² |
| `footlambert` | `foot_lambert` | foot-lambert | `bu_footlambert` | 1/π cd·ft⁻² |

**Water-vapour permeance**

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `perm_0C` | — | perm (0 °C) | `bu_perm_0c` | 5.72135×10⁻¹¹ kg·Pa⁻¹·s⁻¹·m⁻² (exact) |
| `perm_23C` | — | perm (23 °C) | `bu_perm_23c` | 5.74525×10⁻¹¹ kg·Pa⁻¹·s⁻¹·m⁻² (exact) |
| `perm_m` | `perm_metric` | metric perm | `bu_perm_metric` | 8.68127×10⁻¹¹ kg·Pa⁻¹·s⁻¹·m⁻² (exact) |

**Heat conventions**

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `cal_m` | `mean_calorie` | mean calorie | `bu_calorie_mean` | 4.19002 J (exact) |
| `cal_15` | `calorie_15C` | 15 °C calorie | `bu_calorie_15c` | 4.1858 J (exact) |
| `cal_20` | `calorie_20C` | 20 °C calorie | `bu_calorie_20c` | 4.1819 J (exact) |
| `Btu_59` | `btu_59F` | 59 °F BTU | `bu_btu_59f` | 1054.8 J (exact) |
| `Btu_60` | `btu_60F` | 60 °F BTU | `bu_btu_60f` | 1054.68 J (exact) |
| `Btu_m` | `mean_btu` | mean BTU | `bu_btu_mean` | 1055.87 J (exact) |

**And the singles**

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `e` | `elementary_charge` | elementary charge | `bu_elementary_charge` | 1.602176634×10⁻¹⁹ C (exact, the 2019 SI definition) |
| `sph` | `spere`, `spat` | spere (spat) | `bu_spere` | 4π sr |
| `cml` | `circular_mil` | circular mil | `bu_circular_mil` | π/4 `thou`² ≈ 5.0670748×10⁻¹⁰ m² |
| `unit_pole` | `unitpole` | unit pole | `bu_unit_pole` | 4π×10⁻⁸ Wb |
| `hp_W` | `water_horsepower` | water horsepower | `bu_horsepower_water` | 746.043 W |

---

## 4. Prefixes

A prefix is attached to a base unit symbol either with the `~` separator — `prefix~baseunit`, the canonical form — or **compactly**, without it: `kg` is `k~g`, `MiB` is `Mi~B`. Both spellings parse to the same `value_unit_t`. `bvn_unit_to_string` — and a writer handed only a `value_unit_t` — always emits the separated form; a document that is *re-serialised* keeps the spelling it was written in. `bovnar pretty-print --canonical` does not keep the spelling either way: it writes every unit in reduced form and rescales the value to match (`1.0 km` becomes `1000.0 m`, `2.5 kg` becomes `2500.0 g`). A prefix on a base unit, a currency or a non-SI unit is folded into the value; a unit that is, or reduces to, a single named derived SI unit keeps its prefix, in the separated form (`kPa` → `k~Pa`, `k~N·m` → `k~J`), so `2.5 k~N` and `2500 N` are not unified. An integer that would stop being an integer, or stop fitting its width, fails with `error_value_out_of_range`; nothing is rounded. See §4.3 for what the compact form does and does not accept.

### 4.1 SI Prefixes

All 24 current SI prefixes are supported, from quecto (10⁻³⁰) to quetta (10³⁰).

| Prefix | Symbol | Factor | Enum value |
|--------|--------|--------|------------|
| quetta | `Q`    | 10³⁰   | `si_quetta` |
| ronna  | `R`    | 10²⁷   | `si_ronna`  |
| yotta  | `Y`    | 10²⁴   | `si_yotta`  |
| zetta  | `Z`    | 10²¹   | `si_zetta`  |
| exa    | `E`    | 10¹⁸   | `si_exa`    |
| peta   | `P`    | 10¹⁵   | `si_peta`   |
| tera   | `T`    | 10¹²   | `si_tera`   |
| giga   | `G`    | 10⁹    | `si_giga`   |
| mega   | `M`    | 10⁶    | `si_mega`   |
| kilo   | `k`    | 10³    | `si_kilo`   |
| hecto  | `h`    | 10²    | `si_hecto`  |
| deca   | `da`   | 10¹    | `si_deca`   |
| *(none)* | —    | 10⁰    | `si_none`   |
| deci   | `d`    | 10⁻¹   | `si_deci`   |
| centi  | `c`    | 10⁻²   | `si_centi`  |
| milli  | `m`    | 10⁻³   | `si_milli`  |
| micro  | `µ` *(or `u`)* | 10⁻⁶ | `si_micro`  |
| nano   | `n`    | 10⁻⁹   | `si_nano`   |
| pico   | `p`    | 10⁻¹²  | `si_pico`   |
| femto  | `f`    | 10⁻¹⁵  | `si_femto`  |
| atto   | `a`    | 10⁻¹⁸  | `si_atto`   |
| zepto  | `z`    | 10⁻²¹  | `si_zepto`  |
| yocto  | `y`    | 10⁻²⁴  | `si_yocto`  |
| ronto  | `r`    | 10⁻²⁷  | `si_ronto`  |
| quecto | `q`    | 10⁻³⁰  | `si_quecto` |

> **Encoding note:** `µ` is U+00B5 (MICRO SIGN), UTF-8: `0xC2 0xB5`. U+03BC (GREEK SMALL LETTER MU) is a distinct code point but is also accepted on input; the canonical output form is always U+00B5. ASCII `u` is accepted as an input-only alias for `µ` (e.g. `u~m` parses as `µ~m`); the canonical output form is always `µ`. Note `u` is also the bare symbol for the dalton, but the two never collide: the base unit is whatever the longest alias suffix says, so `u` is the dalton and the `u` in `um` or `u~m` is the prefix.

#### Prefix–Symbol Ambiguities

Several prefix symbols overlap with base unit symbols. A base unit symbol is matched as the **longest alias suffix** of the component, so a bare unit always wins over a prefixed reading of the same token: `min` is the minute, never milli-inch, and `cd` is the candela, never centi-day. Where that is not the reading you want, the `~` separator states it: `m~in` is milli-inch, `m~` introduces the milli prefix, bare `m` is the meter.

| Symbol | As prefix | As base unit |
|--------|-----------|--------------|
| `m`    | milli     | meter        |
| `d`    | deci      | day          |
| `h`    | hecto     | hour         |
| `T`    | tera      | tesla        |
| `f`    | femto     | *(none)* — the farad is `F`, uppercase |
| `a`    | atto      | *(none)*     |
| `u`    | micro (ASCII alias for `µ`) | dalton |
| `S`    | *(none)*  | siemens      |

`d~s` = decisecond; `d` alone = day.

### 4.2 IEC Binary Prefixes

IEC 80000-13 binary prefixes are used for digital quantities (`b` and `B` only). The table below also lists `Ri` and `Qi`; the standard stops at `Yi` (2⁸⁰) and those two are an unratified extension.

| Prefix | Symbol | Factor | Enum value |
|--------|--------|--------|------------|
| kibi   | `Ki`   | 2¹⁰    | `iec_kibi` |
| mebi   | `Mi`   | 2²⁰    | `iec_mebi` |
| gibi   | `Gi`   | 2³⁰    | `iec_gibi` |
| tebi   | `Ti`   | 2⁴⁰    | `iec_tebi` |
| pebi   | `Pi`   | 2⁵⁰    | `iec_pebi` |
| exbi   | `Ei`   | 2⁶⁰    | `iec_exbi` |
| zebi   | `Zi`   | 2⁷⁰    | `iec_zebi` |
| yobi   | `Yi`   | 2⁸⁰    | `iec_yobi` |
| robi   | `Ri`   | 2⁹⁰    | `iec_robi` |
| quebi  | `Qi`   | 2¹⁰⁰   | `iec_quebi` |

#### Prefix–Unit Validity Constraints

- **IEC prefixes** (`Ki`…`Qi`) are only permitted on `b` and `B`. `Ki~m` → `error_unit_illegal`.
- **SI sub-kilo prefixes** (`d`, `c`, `m`, `µ`, `n`, `p`, `f`, `a`, `z`, `y`, `r`, `q`, `da`, `h`) are forbidden on `b` and `B`.
- **German units** (`bu_pfund` through `bu_scheffel`) accept only `si_none`/`iec_none`.
- **Ratio units** (`%`, `‰`, `‱`, `pcm`, `ppm`, `ppb`, `pptr`, `ppq`) likewise: a prefixed per-cent is meaningless. `k~%` → `error_unit_illegal` (§3.25).
- **Scales that are already a scale** — `pH`, `mph`, `kph`, the five water-hardness degrees, `gpg`, `CF`, `PSU`, `JTU` — take no prefix either. Each carries its reason in §3.26–§3.30; `NTU`, `FNU`, `FTU` and `FAU` *do* take one, because a milli-NTU is a real ultrapure-water measurement.
- **Currency units** accept SI prefixes of any magnitude (see §9.4). IEC prefixes are forbidden on all currency codes.

<!-- bovnar-example: rejected -->
```bovnar
.valid1   = <uint:64,Ki~B>  8;     # OK: IEC prefix on byte
.valid2   = <uint:32,M~b>   100;   # OK: SI mega on bit
.valid3   = <float_dec:64,k~$USD> 250.0;  # OK: kilo on currency
.invalid1 = <uint:64,Ki~m>  1;     # ERROR: IEC prefix on meter
.invalid2 = <uint:32,m~B>   512;   # ERROR: SI milli on byte
.invalid3 = <float_dec:64,Ki~$USD> 1;     # ERROR: IEC prefix on currency
```

### 4.3 Compact Prefix Form

A prefixed physical unit may be written without the `~` separator, in the spelling the rest of the world already uses:

```bovnar
.mass      = <float:64,kg>   72.5;      # same unit as k~g
.distance  = <float:64,km>    1.5;      # k~m
.period    = <float:64,ms>   16.7;      # m~s
.frequency = <float:64,MHz> 2400.0;     # M~Hz
.pressure  = <float:64,hPa> 1013.25;    # h~Pa
.energy    = <float:64,MeV>   13.6;     # M~eV
.amount    = <float:64,mmol>   2.5;     # m~mol
.memory    = <uint:64,MiB>   512;       # Mi~B
.force     = <float:64,kg·m/s²> 9.81;   # compounds, exponents and groups too
```

Both spellings produce the same `value_unit_t`, so they reconcile against each other (`<float:64,k~m> 1.5 km` is valid) and against a stored annotation. **The canonical output form is unchanged**: `bvn_unit_to_string` emits `k~g` for either spelling. What the compact form does survive is a re-serialisation: the reader hands the spelling over beside the unit (`unit_source`, §12.1), and `pretty-print` or a typed Python round trip writes `km` back as `km`. `--canonical` is the mode that does not.

The rules are exactly the rules of the separated form, with one addition:

- **A bare unit alias always wins.** The base symbol is the longest alias suffix, so `min` is the minute (not milli-inch), `cd` the candela (not centi-day), `ft` the foot (not femto-tonne), `at` the technical atmosphere (not atto-tonne), `dB` the decibel (not deci-byte). The separated form reaches the other reading: `m~in`, `c~d`, `f~t`.
- **Where two prefixed readings compete, the longer base symbol wins** — for the same reason. `dat` is deci-`at` (technical atmosphere), not deca-tonne; `dau` is deci-`au`, not deca-dalton. Write `da~t` / `da~u` for those.
- **A prefix cannot be stacked.** `kkg` and `k~kg` are both `error_unit_illegal`.
- **Prefix–unit validity is unchanged.** `Kim` fails for the same reason `Ki~m` does; so do `mB`, `kPfd`, `kppm`.
- **A few compact spellings are refused by name.** A token that is a well-known annotation for something *else* would otherwise become a valid — and quietly wrong — unit, which is the failure mode the format exists to prevent. These stay `error_unit_illegal`:

  | Refused | Would have meant | Usually means | Write instead |
  |---------|------------------|---------------|---------------|
  | `usb`   | microstilb       | the bus       | `u~sb` |
  | `kt`    | kilotonne        | *also* the knot | `k~t` (mass) or `kn` (speed) |
  | `ppt`   | picopint (a volume) | parts per trillion, *or* per thousand | `pptr` (10⁻¹²), `‰` (10⁻³), `p~pt` (the volume) |

  `pH`, `mph` and `kph` were on this list until the quantities they name became
  units of their own (§3.26, §3.27). A bare alias outranks any prefixed reading,
  so they now need no exception — and `p~H`, `m~ph`, `k~ph` still mean what they
  always did.

  The last two rows are the harder cases. `kt` is a standard abbreviation for two units Bovnar *does* model — the kilotonne in climate and energy data, the knot in marine and aviation data — and reading a speed as a mass is precisely what this format exists to stop. `ppt` is worse: the compact rule makes it a *volume*, while every real use of the token is a dimensionless ratio, and the two ratios it might mean are 10⁹ apart. A token only the author can resolve is one the author has to resolve.

  Note what is **not** done about `ppt`: the parts-per-trillion unit added in §3.25 does not claim the spelling. A bare alias outranks any prefixed reading (rule 2 above), so claiming it would take `ppt` away from `p~pt` and change what an existing document means — which the rule at the top of `units.bvnr` forbids and `gen_units.py` refuses at build time. The unit is spelled `pptr`, following UCUM's own `[pptr]`, and the ambiguous token is refused instead.

  The list lives in `src/gendata/units.bvnr` (`.compact_exceptions`) and applies to the compact spelling only — `p~H` is still picohenry, `k~t` still the kilotonne, `p~pt` still the picopint.

Because a compact spelling is only ever reached where the separated form would have been a parse error, no document that parsed before this existed can parse differently now.

For a token-by-token list of every spelling that could be read two ways — including case traps, look-alike characters and abbreviations that are deliberately not units — see [`07_bovnar_unit_ambiguities.md`](07_bovnar_unit_ambiguities.md).

Currencies take the compact prefix too — `k$EUR` is `k~$EUR`. The `$` sigil already separates the prefix from the code (no prefix symbol and no currency code contains a `$`), so nothing is left for the `~` to resolve. What the sigil rule still requires is the sigil itself: `kUSD` is `error_unit_illegal`, because a bare code is never a currency (§10.4).

---

## 5. Unit Notation Grammar

### 5.1 Simple Units

```
unit-component = [ prefix [ "~" ] ] base-unit [ unit-exponent ]
```

The `~` separator is optional (`km` = `k~m`, `k$USD` = `k~$USD`; §4.3). The currency `$` sigil is not.

```bovnar
.temperature = <float:64,K>      300.0;   # kelvin
.distance    = <float:64,k~m>    1.5;     # kilometer
.frequency   = <float:64,M~Hz>   2400;    # megahertz
.storage     = <uint:64,Ki~B>    1024;    # kibibytes
.altitude    = <float:64,km>     10.5;    # kilometer, compact spelling
.fund_nav    = <float_dec:64,k~$USD> 250.0; # $250,000
```

### 5.2 Compound Units

```ebnf
compound-unit  = "no_unit"
               | unit-component { unit-sep unit-component }

unit-sep       = "*" | "/" | "·"           (* · = U+00B7 MIDDLE DOT *)

unit-component = [ prefix [ "~" ] ] base-unit [ unit-exponent ]

unit-exponent  = [ exp-sign ] exp-digit { exp-digit }
               | "^" [ "-" | "+" ] ASCII-digit { ASCII-digit }

exp-sign       = "⁺" | "⁻"

exp-digit      = "⁰" | "¹" | "²" | "³" | "⁴" | "⁵"
               | "⁶" | "⁷" | "⁸" | "⁹"
```

An exponent is **one or more** digits in either notation — `m¹⁰⁰` and `m^100`
are units, and this production is what §6 states in prose. `⁰` is a digit here
only in a *multi-digit* exponent: it exists so `m¹⁰⁰` can be written, and a
leading `m⁰` is still no unit (§6). At most three digits are scanned, so
`m^1000` fails as an unrecognised token rather than as an over-large exponent.

Currency codes participate in compound expressions using the same separators:

```bovnar
.gold_spot    = <float_dec:64,$USD/oz_t>   2351.40;  # $/troy oz
.rent         = <float_dec:64,$EUR/m²>       12.50;  # €/m²
.billing_rate = <float_dec:64,$EUR/h>       150.00;  # €/h
.eur_usd      = <float_dec:64,$USD/$EUR>      1.0842; # exchange rate
```

> The sub-grammar is **semantic**, not lexical. The outer lexer captures the type-annotation body as a raw byte sequence; `bvn_parse_unit` parses the unit string portion after the lexer finishes.

### 5.3 Separators

| Character | Code point | UTF-8 bytes | Meaning |
|-----------|-----------|-------------|---------|
| `*`       | U+002A    | `0x2A`      | Multiplication |
| `·`       | U+00B7    | `0xC2 0xB7` | Multiplication (preferred visual form) |
| `/`       | U+002F    | `0x2F`      | Division |

`·` and `*` are semantically identical and may be mixed freely.

### 5.4 Denominator Semantics

The first `/` sets a latching "in-denominator" flag to `true` for all subsequent components. Additional `/` separators do not toggle back to the numerator. `k~g·m/s²` stores:

```
[0]: gram,   exp_linear,     si_kilo  ← numerator
[1]: meter,  exp_linear,     si_none  ← numerator
[2]: second, exp_neg_square, si_none  ← denominator (exponent negated)
```

Both `k~g·m/s²` and `k~g·m·s⁻²` parse to identical `value_unit_t` representations.

#### Parenthesised grouping

A `(…)` group is a sub-expression evaluated independently. Like any factor it
obeys the latching denominator, so a `/` before a group negates the group's net
component exponents as a whole. This makes the readable denominator form work and
compose correctly:

```bovnar
.pressure = <float:64,k~g/(m·s²)> 101325;  # kg·m⁻¹·s⁻² — same as k~g/m·s²
.areal    = <float:64,(k~g/m)·s²> 1.0;     # kg·m⁻¹·s²  — grouping changes the s sign
.rate     = <float:64,m/(s·s)> 1.0;        # m·s⁻²
```

`k~g/(m·s²)` and `k~g/m·s²` therefore parse to the **same** `value_unit_t` (the
canonical, parenless form is what the writer emits). Rules: an explicit separator
is required before a group (`m·(s)`, not `m(s)`); a group is not followed by its
own exponent (`(m·s)²` is rejected — write `m²·s²`); parentheses must balance;
empty groups (`()`) are rejected; nesting is bounded at 16. Unmatched or malformed
groups raise `error_unit_illegal`.

---

## 6. Exponents

Exponents are integers in the range **−100 … +100**, with **zero reserved**: a
component raised to zero is malformed, not dimensionless, so `m^0` and `m⁰` are
not units. The bounds are `BVN_EXPONENT_MIN` and `BVN_EXPONENT_MAX` in
`include/bovnar.h`.

The named enumerators of `unit_exponent_t` cover only ±1…±9 and are kept because
that is what most units need and what existing source says. They are **not** the
whole domain — any integer in the range is a valid exponent, so a `switch` over
the type needs a `default` and code must not assume it can enumerate the values.

At most **three digits** are scanned, which is exactly what 100 needs. That is
what makes an over-long exponent fail cleanly: `m^1000` leaves a digit where the
scan expects the `^`, so no exponent is recognised at all and the whole token
fails to resolve as a base symbol. A value that scans but lands outside the
range — `m^200` — is refused on the range check.

### 6.1 Unicode Superscript Form

| Glyph | Code point | UTF-8 bytes      | Maps to |
|-------|-----------|------------------|---------|
| `⁰`   | U+2070    | `0xE2 0x81 0xB0` | digit 0 (multi-digit only) |
| `¹`   | U+00B9    | `0xC2 0xB9`      | `exp_linear` |
| `²`   | U+00B2    | `0xC2 0xB2`      | `exp_square` |
| `³`   | U+00B3    | `0xC2 0xB3`      | `exp_cubic` |
| `⁴`   | U+2074    | `0xE2 0x81 0xB4` | `exp_quartic` |
| `⁵`   | U+2075    | `0xE2 0x81 0xB5` | `exp_quintic` |
| `⁶`   | U+2076    | `0xE2 0x81 0xB6` | `exp_sextic` |
| `⁷`   | U+2077    | `0xE2 0x81 0xB7` | `exp_septic` |
| `⁸`   | U+2078    | `0xE2 0x81 0xB8` | `exp_octic` |
| `⁹`   | U+2079    | `0xE2 0x81 0xB9` | `exp_nonic` |
| `⁺`   | U+207A    | `0xE2 0x81 0xBA` | positive sign (no-op) |
| `⁻`   | U+207B    | `0xE2 0x81 0xBB` | negate exponent |

Digits **combine**, most-significant first, so `m¹⁰⁰` is the metre to the
hundredth and `s⁻¹²` is a reciprocal twelfth power. Note that the glyphs are not
one width — `¹ ² ³` are two UTF-8 bytes and `⁰ ⁴`–`⁹` are three — so `⁻¹⁰⁰` is
eleven bytes, and a parser must consume one digit at a time rather than assume a
stride.

`⁰` is a digit only *within* a multi-digit exponent. A lone `m⁰` is still not a
unit, because zero is reserved.

### 6.2 ASCII Caret Form

| ASCII form | Equivalent Unicode | Parsed as |
|------------|--------------------|-----------|
| `m^2`      | `m²`               | `exp_square` |
| `s^-2`     | `s⁻²`              | `exp_neg_square` |
| `m^+2`     | `m²`               | `exp_square` |
| `kg^1`     | `kg¹`              | `exp_linear` |
| `m^100`    | `m¹⁰⁰`             | exponent 100 |
| `s^-100`   | `s⁻¹⁰⁰`            | exponent −100 |

Up to **three ASCII digits** are permitted after the caret — exactly what
`BVN_EXPONENT_MAX` needs. `m^1000` is not an out-of-range exponent but an
unrecognised token: the scan stops after three digits, finds no `^`, and the
whole string fails to resolve.

### 6.3 Exponent Edge Cases

- **`exp_invalid` (value 0):** Zero-initialized sentinel. API functions that have an error output path reject it and signal an error. The two prefix query functions (`bvn_unit_prefix_factor`, `bvn_unit_prefix_exponent`) silently skip components with `exp_invalid`.
- **`exp_linear` (value 1):** Stored for both an explicit `¹`/`^1` and any component written without an exponent suffix.

---

## 7. The `no_unit` Keyword

The literal `no_unit` declares a value as **explicitly dimensionless**:

```bovnar
.ratio       = <float:64,no_unit> 0.95;
.count       = <uint:32,no_unit>  1000;
```

`bvn_parse_unit` detects `no_unit` via `memcmp` and returns `BVN_UNIT_NONE` (`num_components = 0`).

**Omitting the unit parameter** inside an annotation (e.g. `<float:64>`) yields the same `BVN_UNIT_NONE` (`num_components = 0`). Only a **fully unannotated** value (`1.0`) gets `BVN_UNIT_NO_PREFIX(bu_none)` (`num_components = 1`, `base == bu_none`), from default type synthesis. The two states are semantically equivalent — they compare as compatible via `bvn_units_compatible` and both serialize to `"no_unit"` — but they are structurally distinct.

---

## 8. Constraints and Limits

| Constraint | Value | Error on violation |
|------------|-------|--------------------|
| Maximum components per compound unit | 32 (`BVNR_MAX_UNIT_COMPONENTS`) | `error_unit_illegal` |
| Empty component between separators (e.g. `m//s`) | Not allowed | `error_unit_illegal` |
| Maximum raw unit string length | 255 bytes, but *which* cap fires depends on where the unit is written | `error_type_too_long` in an annotation, `error_unit_too_long` inline — see below |
| Null or empty unit string | Rejected by `bvn_parse_unit` | `ok = false` |

`a/b/c` parses as `a / (b·c)` → components `[a, b⁻¹, c⁻¹]`. The "no toggle back" rule means `/` always adds to the denominator; it never re-enters the numerator.

> **An over-long unit in an *annotation* is `error_type_too_long`, not `error_unit_too_long`.**
> Three 255-byte caps sit in front of the unit parser and the one that fires decides the code. The
> whole type-annotation body has its own cap, and it counts the family name, the width and the
> commas as well — so a unit parameter can never be the only thing over the line, and an annotated
> unit always reaches `error_type_too_long` first. An **inline** unit suffix has a buffer to
> itself and is the path that reaches `error_unit_too_long`. doc/11 §2.5
> tabulates all three caps beside the profile grammar.

---

## 9. Currency Codes

Currency amounts are dimensional quantities in financial computing. `$19.99 USD` carries a denomination dimension just as `9.81 m/s²` carries an acceleration dimension. Bovnar extends the unit system with 216 currency and cryptocurrency codes so that monetary data can be annotated and round-tripped with the same precision guarantees as physical measurements.

### 9.1 The `$` Sigil Rule

As of spec 1.0 a currency is recognised **only** in its `$`-sigil form (`$USD`, `$BTC`, or prefixed `k~$EUR` / `k$EUR`). The sigil — and nothing else — dispatches a component to the currency table; see §10.4 for the full rules and rationale.

Classification happens at the lookup stage, per unit component:

1. A component introduced by `$` (after any SI/IEC prefix and its `~`) is looked up in the **currency table**. If the code is not found there, `error_unit_illegal` is raised — a `$` introduces a currency and nothing else.
2. A component **without** a `$` is looked up only in the **physical unit table**. A bare code such as `USD` or `CUP` is therefore never a currency; if it is not a physical unit it is `error_unit_illegal`.

Because the sigil is mandatory, a bare uppercase code can never collide with a physical-unit symbol — present or future — so the two namespaces are disjoint by construction. The collision cases this resolves are catalogued for reference in §10.

### 9.2 ISO 4217 Fiat Currencies and Precious Metals

166 ISO 4217 alphabetic codes are supported, including precious-metal X-codes. They occupy the front of **block 90** of the `value_base_unit_t` id space (§12.1) — ids **900000 … 900165**, alphabetical from `AED` to `ZWL` — and, unlike physical units, have **no named `bu_*` enumerators**: such a currency is resolved from its `$`-sigil code by `bvn_parse_currency_str` and carried as the numeric `base` value. The catalogue in `bovnar_currency.c` is index-aligned to the block, so a lookup is `base - BVN_CURRENCY_FIRST`.

The alphabetical order is a convenience, not a contract. Whether a currency is fiat or crypto is a **column of the catalogue**, read by `bvn_unit_is_fiat` / `bvn_unit_is_crypto`, not a sub-range of the ids — so a new currency of either kind appends at the end of `currencies.bvnr` and renumbers nothing. Before 2.0 it *was* a sub-range, which is why `ZWG` and `XCG` had to be stranded at 378–379 outside the currency range entirely; the block has 10 000 ids and they are ordinary rows now.

Four codes are historical and retained for compatibility: `HRK` (Croatian Kuna, retired 2023-01-01 when Croatia adopted the Euro), `SLL` (Sierra Leonean Leone (old), replaced by `SLE` in 2022), `ZWL` (Zimbabwean Dollar, superseded by `ZWG` Zimbabwe Gold in 2024), and `BGN` (Bulgarian Lev, retired 2026-01-01 when Bulgaria adopted the Euro); `ANG` (Netherlands Antillean Guilder) likewise coexists with its successor `XCG` (Caribbean Guilder, which inherits ANG's numeric code 532).

The `minor_unit` field carries the exponent N such that 1 major unit = 10^N minor units (e.g. 1 USD = 100 cents, N=2). Applications reading integer-annotated values (e.g. `<uint:64,$KWD>`) should call `bvn_currency_minor_unit` to determine the correct decimal shift. Minor units are **bold** below when they differ from 2. `Num` is the ISO 4217 numeric identifier.

| Code | Num | Min | Name |
|------|----:|----:|------|
| `AED` |  784 |   2 | UAE Dirham |
| `AFN` |  971 |   2 | Afghan Afghani |
| `ALL` |    8 |   2 | Albanian Lek |
| `AMD` |   51 |   2 | Armenian Dram |
| `ANG` |  532 |   2 | Netherlands Antillean Guilder |
| `AOA` |  973 |   2 | Angolan Kwanza |
| `ARS` |   32 |   2 | Argentine Peso |
| `AUD` |   36 |   2 | Australian Dollar |
| `AWG` |  533 |   2 | Aruban Florin |
| `AZN` |  944 |   2 | Azerbaijani Manat |
| `BAM` |  977 |   2 | Bosnia-Herzegovina Convertible Mark |
| `BBD` |   52 |   2 | Barbados Dollar |
| `BDT` |   50 |   2 | Bangladeshi Taka |
| `BGN` |  975 |   2 | Bulgarian Lev *(historical; retired 2026-01-01)* |
| `BHD` |   48 | **3** | Bahraini Dinar |
| `BIF` |  108 | **0** | Burundian Franc |
| `BMD` |   60 |   2 | Bermudian Dollar |
| `BND` |   96 |   2 | Brunei Dollar |
| `BOB` |   68 |   2 | Boliviano |
| `BRL` |  986 |   2 | Brazilian Real |
| `BSD` |   44 |   2 | Bahamian Dollar |
| `BTN` |   64 |   2 | Bhutanese Ngultrum |
| `BWP` |   72 |   2 | Botswana Pula |
| `BYN` |  933 |   2 | Belarusian Ruble |
| `BZD` |   84 |   2 | Belize Dollar |
| `CAD` |  124 |   2 | Canadian Dollar |
| `CDF` |  976 |   2 | Congolese Franc |
| `CHF` |  756 |   2 | Swiss Franc |
| `CLF` |  990 | **4** | Unidad de Fomento |
| `CLP` |  152 | **0** | Chilean Peso |
| `CNY` |  156 |   2 | Chinese Yuan |
| `COP` |  170 |   2 | Colombian Peso |
| `CRC` |  188 |   2 | Costa Rican Colon |
| `CUP` |  192 |   2 | Cuban Peso |
| `CVE` |  132 |   2 | Cape Verdean Escudo |
| `CZK` |  203 |   2 | Czech Koruna |
| `DJF` |  262 | **0** | Djiboutian Franc |
| `DKK` |  208 |   2 | Danish Krone |
| `DOP` |  214 |   2 | Dominican Peso |
| `DZD` |   12 |   2 | Algerian Dinar |
| `EGP` |  818 |   2 | Egyptian Pound |
| `ERN` |  232 |   2 | Eritrean Nakfa |
| `ETB` |  230 |   2 | Ethiopian Birr |
| `EUR` |  978 |   2 | Euro |
| `FJD` |  242 |   2 | Fijian Dollar |
| `FKP` |  238 |   2 | Falkland Islands Pound |
| `GBP` |  826 |   2 | Pound Sterling |
| `GEL` |  981 |   2 | Georgian Lari |
| `GHS` |  936 |   2 | Ghanaian Cedi |
| `GIP` |  292 |   2 | Gibraltar Pound |
| `GMD` |  270 |   2 | Gambian Dalasi |
| `GNF` |  324 | **0** | Guinean Franc |
| `GTQ` |  320 |   2 | Guatemalan Quetzal |
| `GYD` |  328 |   2 | Guyanese Dollar |
| `HKD` |  344 |   2 | Hong Kong Dollar |
| `HNL` |  340 |   2 | Honduran Lempira |
| `HRK` |  191 |   2 | Croatian Kuna *(historical; retired 2023-01-01)* |
| `HTG` |  332 |   2 | Haitian Gourde |
| `HUF` |  348 |   2 | Hungarian Forint |
| `IDR` |  360 |   2 | Indonesian Rupiah |
| `ILS` |  376 |   2 | Israeli New Shekel |
| `INR` |  356 |   2 | Indian Rupee |
| `IQD` |  368 | **3** | Iraqi Dinar |
| `IRR` |  364 |   2 | Iranian Rial |
| `ISK` |  352 | **0** | Icelandic Krona |
| `JMD` |  388 |   2 | Jamaican Dollar |
| `JOD` |  400 | **3** | Jordanian Dinar |
| `JPY` |  392 | **0** | Japanese Yen |
| `KES` |  404 |   2 | Kenyan Shilling |
| `KGS` |  417 |   2 | Kyrgyzstani Som |
| `KHR` |  116 |   2 | Cambodian Riel |
| `KMF` |  174 | **0** | Comorian Franc |
| `KPW` |  408 |   2 | North Korean Won |
| `KRW` |  410 | **0** | South Korean Won |
| `KWD` |  414 | **3** | Kuwaiti Dinar |
| `KYD` |  136 |   2 | Cayman Islands Dollar |
| `KZT` |  398 |   2 | Kazakhstani Tenge |
| `LAK` |  418 |   2 | Laotian Kip |
| `LBP` |  422 |   2 | Lebanese Pound |
| `LKR` |  144 |   2 | Sri Lankan Rupee |
| `LRD` |  430 |   2 | Liberian Dollar |
| `LSL` |  426 |   2 | Lesotho Loti |
| `LYD` |  434 | **3** | Libyan Dinar |
| `MAD` |  504 |   2 | Moroccan Dirham |
| `MDL` |  498 |   2 | Moldovan Leu |
| `MGA` |  969 |   2 | Malagasy Ariary |
| `MKD` |  807 |   2 | Macedonian Denar |
| `MMK` |  104 |   2 | Myanmar Kyat |
| `MNT` |  496 |   2 | Mongolian Togrog |
| `MOP` |  446 |   2 | Macanese Pataca |
| `MRU` |  929 |   2 | Mauritanian Ouguiya |
| `MUR` |  480 |   2 | Mauritian Rupee |
| `MVR` |  462 |   2 | Maldivian Rufiyaa |
| `MWK` |  454 |   2 | Malawian Kwacha |
| `MXN` |  484 |   2 | Mexican Peso |
| `MYR` |  458 |   2 | Malaysian Ringgit |
| `MZN` |  943 |   2 | Mozambican Metical |
| `NAD` |  516 |   2 | Namibian Dollar |
| `NGN` |  566 |   2 | Nigerian Naira |
| `NIO` |  558 |   2 | Nicaraguan Cordoba |
| `NOK` |  578 |   2 | Norwegian Krone |
| `NPR` |  524 |   2 | Nepalese Rupee |
| `NZD` |  554 |   2 | New Zealand Dollar |
| `OMR` |  512 | **3** | Omani Rial |
| `PAB` |  590 |   2 | Panamanian Balboa |
| `PEN` |  604 |   2 | Peruvian Sol |
| `PGK` |  598 |   2 | Papua New Guinean Kina |
| `PHP` |  608 |   2 | Philippine Peso |
| `PKR` |  586 |   2 | Pakistani Rupee |
| `PLN` |  985 |   2 | Polish Zloty |
| `PYG` |  600 | **0** | Paraguayan Guarani |
| `QAR` |  634 |   2 | Qatari Riyal |
| `RON` |  946 |   2 | Romanian Leu |
| `RSD` |  941 |   2 | Serbian Dinar |
| `RUB` |  643 |   2 | Russian Ruble |
| `RWF` |  646 | **0** | Rwandan Franc |
| `SAR` |  682 |   2 | Saudi Riyal |
| `SBD` |   90 |   2 | Solomon Islands Dollar |
| `SCR` |  690 |   2 | Seychellois Rupee |
| `SDG` |  938 |   2 | Sudanese Pound |
| `SEK` |  752 |   2 | Swedish Krona |
| `SGD` |  702 |   2 | Singapore Dollar |
| `SHP` |  654 |   2 | Saint Helena Pound |
| `SLE` |  925 |   2 | Sierra Leonean Leone |
| `SLL` |  694 |   2 | Sierra Leonean Leone (old) *(historical; replaced by SLE 2022)* |
| `SOS` |  706 |   2 | Somali Shilling |
| `SSP` |  728 |   2 | South Sudanese Pound |
| `SRD` |  968 |   2 | Surinamese Dollar |
| `STN` |  930 |   2 | Sao Tome and Principe Dobra |
| `SVC` |  222 |   2 | Salvadoran Colon |
| `SYP` |  760 |   2 | Syrian Pound |
| `SZL` |  748 |   2 | Swazi Lilangeni |
| `THB` |  764 |   2 | Thai Baht |
| `TJS` |  972 |   2 | Tajikistani Somoni |
| `TMT` |  934 |   2 | Turkmenistan Manat |
| `TND` |  788 | **3** | Tunisian Dinar |
| `TOP` |  776 |   2 | Tongan Pa'anga |
| `TRY` |  949 |   2 | Turkish Lira |
| `TTD` |  780 |   2 | Trinidad and Tobago Dollar |
| `TWD` |  901 |   2 | New Taiwan Dollar |
| `TZS` |  834 |   2 | Tanzanian Shilling |
| `UAH` |  980 |   2 | Ukrainian Hryvnia |
| `UGX` |  800 | **0** | Ugandan Shilling |
| `USD` |  840 |   2 | US Dollar |
| `UYU` |  858 |   2 | Uruguayan Peso |
| `UZS` |  860 |   2 | Uzbekistani Som |
| `VES` |  928 |   2 | Venezuelan Bolivar Soberano |
| `VND` |  704 | **0** | Vietnamese Dong |
| `VUV` |  548 | **0** | Vanuatu Vatu |
| `WST` |  882 |   2 | Samoan Tala |
| `XAF` |  950 | **0** | CFA Franc BEAC |
| `XAG` |  961 | **0** | Silver |
| `XAU` |  959 | **0** | Gold |
| `XCD` |  951 |   2 | East Caribbean Dollar |
| `XCG` |  532 |   2 | Caribbean Guilder |
| `XDR` |  960 | **0** | Special Drawing Rights |
| `XOF` |  952 | **0** | CFA Franc BCEAO |
| `XPD` |  964 | **0** | Palladium |
| `XPF` |  953 | **0** | CFP Franc |
| `XPT` |  962 | **0** | Platinum |
| `XTS` |  963 | **0** | Test currency (ISO 4217 reserved; do not use in production) |
| `YER` |  886 |   2 | Yemeni Rial |
| `ZAR` |  710 |   2 | South African Rand |
| `ZMW` |  967 |   2 | Zambian Kwacha |
| `ZWG` |  924 |   2 | Zimbabwe Gold |
| `ZWL` |  932 |   2 | Zimbabwean Dollar *(historical; superseded by ZWG 2024)* |

> `CLF` (Unidad de Fomento) is the only currency with 4 minor units **in this catalogue**. ISO 4217 also defines `UYW` with 4, but the catalogue omits the fund and bond codes (`BOV CHE CHW COU MXV USN UYI UYW`, `XBA`–`XBD`, `XSU`, `XUA`) and the no-currency code `XXX`: they denote accounting units and placeholders rather than money a value can be denominated in. The four historical codes `HRK`, `SLL`, `ZWL`, and `BGN` are retained for compatibility but should not be used for new data.

### 9.3 Cryptocurrencies

50 cryptocurrencies are supported, with 3- or 4-letter uppercase tickers. They occupy ids **900166 … 900215**, after the fiat codes in the same block 90, and like them have **no named `bu_*` enumerators** — they are resolved by `bvn_parse_currency_str` and carried as the numeric `base` value. That they happen to follow the fiat run carries no meaning: `bvn_unit_is_crypto` reads the catalogue row, not the id (§9.2). The `minor_unit` field holds the canonical on-chain decimal places. `numeric_code = 0` for all cryptocurrencies.

> **Min** = `minor_unit` = on-chain decimal places. E.g. `<uint:64,$BTC>` stores satoshis; divide by 10⁸ to obtain whole BTC.

| Code   | Min | Subunit | Name |
|--------|----:|---------|------|
| `BTC`  |   8 | satoshi | Bitcoin |
| `ETH`  |  18 | wei | Ethereum |
| `SOL`  |   9 | lamport | Solana |
| `XRP`  |   6 | drop | XRP |
| `BNB`  |  18 | — | BNB |
| `ADA`  |   6 | lovelace | Cardano |
| `LTC`  |   8 | — | Litecoin |
| `DOT`  |  10 | planck | Polkadot |
| `XMR`  |  12 | piconero | Monero |
| `ETC`  |  18 | — | Ethereum Classic |
| `BCH`  |   8 | — | Bitcoin Cash |
| `XLM`  |   7 | stroop | Stellar |
| `FIL`  |  18 | — | Filecoin |
| `ICP`  |   8 | — | Internet Computer |
| `TRX`  |   6 | — | TRON |
| `EOS`  |   4 | — | EOS |
| `VET`  |  18 | — | VeChain |
| `NEO`  |   0 | — | Neo |
| `ZEC`  |   8 | — | Zcash |
| `UNI`  |  18 | — | Uniswap |
| `ARB`  |  18 | — | Arbitrum |
| `SUI`  |   9 | — | Sui |
| `TON`  |   9 | — | Toncoin |
| `INJ`  |  18 | — | Injective |
| `SEI`  |   6 | — | Sei |
| `APT`  |   8 | — | Aptos |
| `TAO`  |   9 | — | Bittensor |
| `WIF`  |   6 | — | dogwifhat |
| `DOGE` |   8 | koinu | Dogecoin |
| `LINK` |  18 | — | Chainlink |
| `USDT` |   6 | — | Tether |
| `USDC` |   6 | — | USD Coin |
| `AVAX` |  18 | — | Avalanche |
| `ATOM` |   6 | — | Cosmos |
| `POL`  |  18 | — | Polygon |
| `NEAR` |  24 | — | NEAR Protocol |
| `ALGO` |   6 | — | Algorand |
| `HBAR` |   8 | — | Hedera |
| `AAVE` |  18 | — | Aave |
| `MKR`  |  18 | — | Maker |
| `DAI`  |  18 | — | Dai |
| `STX`  |   6 | — | Stacks |
| `GRT`  |  18 | — | The Graph |
| `LDO`  |  18 | — | Lido DAO |
| `BONK` |   5 | — | Bonk |
| `PEPE` |  18 | — | Pepe |
| `SHIB` |  18 | — | Shiba Inu |
| `JUP`  |   6 | — | Jupiter |
| `PYTH` |   6 | — | Pyth Network |
| `RUNE` |   8 | — | THORChain |

### 9.4 Prefix Rules for Currency Units

**All SI prefixes** are permitted on all currency units. `k~$USD` denotes "values in thousands of USD" — a common scale annotation in financial reporting. As with a physical unit (§4.3), the `~` is optional: `k$USD` is the same unit, because the `$` sigil already separates the prefix from the code.

```bovnar
.fund_nav   = <float_dec:64,k~$USD>    250.0;   # $250,000
.gdp        = <float_dec:64,M$EUR>  42800.0;    # €42.8 billion — compact prefix
.eth_gwei   = <float_dec:64,G~$ETH>    35.0;    # 35 Gwei gas price
```

The sigil itself is **not** optional in either spelling: `kUSD` is `error_unit_illegal`, exactly as bare `USD` is.

**IEC binary prefixes** (`Ki~`, `Mi~`, …) are **forbidden** on all currency units, compact spelling included (`Ki$USD` is an error too). `bvn_currency_prefix_valid()` returns `false` for any IEC prefix; the parser raises `error_unit_illegal`.

### 9.5 Compound Currency Expressions

Currency codes participate in compound unit expressions using the existing separators:

```bovnar
.gold_spot    = <float_dec:64,$USD/oz_t>    2351.40;  # $/troy oz
.wheat        = <float_dec:64,$USD/bsh>        5.82;  # $/bushel
.rent         = <float_dec:64,$EUR/m²>        12.50;  # €/m²
.billing_rate = <float_dec:64,$EUR/h>         150.00; # €/h
.eur_usd      = <float_dec:64,$USD/$EUR>        1.0842; # exchange rate
.eth_btc      = <float_dec:64,$BTC/$ETH>       0.05610; # cross-crypto rate
```

Currency × currency compounds (`USD·EUR`) are syntactically valid and produce no error. Their financial interpretation is the application's responsibility.

#### Exchange Rate Timestamps

Bovnar annotates denomination; it does not store exchange rates or timestamps. The timestamp belongs in a separate field:

```bovnar
.snapshot = {
    .epoch    = <uint:64,s>              1716400000;
    .eur_usd  = <float_dec:64,$USD/$EUR>        1.0842;
};
```

### 9.6 Compatibility Rules

**`bvn_units_compatible()` returns `false` for every unit containing a currency — including a currency against itself.** It is dimension-based, and a currency deliberately has no SI conversion row, so `bvn_unit_dimension_vector` fails and the compatibility test fails with it. `bvn_units_compatible($USD, $USD)` is `false`.

Do not use it as the gate before converting money: it refuses the identity. Use the conversion functions directly — `bvn_unit_convert_factor`, `bvn_unit_convert_value`, `bvn_unit_convert_rational` — which carry their own currency path and report success or failure themselves.

What that path accepts is **the same currency expression, differing at most in prefixes**:

| From | To | Result |
|------|----|--------|
| `$USD` | `$USD` | 1 (identity) |
| `k~$USD` | `$USD` | 1000 — a prefix delta is an exact power of ten (or two, for IEC), so it stays lossless |
| `$USD·oz_t⁻¹` | `oz_t⁻¹·$USD` | 1 — the match is a multiset, because unit multiplication commutes |
| `$USD` | `$EUR` | refused — two different currency ids, and the library holds no exchange rate |
| `$USD/oz_t` | `$USD/g` | **refused**, though only the denominator changes and no rate is involved. A compound carrying a currency is matched as a whole; the physical part is not converted independently. Convert the physical quantity separately if you need this |
| `$USD` | `m`, `no_unit`, `%` | refused |

The mechanism is `bvn_unit_prefix_only_delta` in `bovnar_si_units.c`, consulted only *after* `bvn_units_compatible` has said no — so a unit that does have an SI row keeps taking the ordinary, fully general path.

### 9.7 Type Pairing Recommendations

| Use case | Recommended annotation | Rationale |
|----------|----------------------|-----------|
| Decimal monetary amount | `<float_dec:64,$USD>` | Exact decimal; 16 significant digits |
| High-precision / actuarial | `<float_dec:128,$USD>` | 34 significant digits |
| Integer minor-unit storage | `<uint:64,$USD>` | Value in cents; app reads `minor_unit()` |
| Negative balances in minor units | `<sint:64,$USD>` | |
| Zero-minor-unit currency | `<uint:64,$JPY>` | Integer is the only correct representation |
| 3-minor-unit currency | `<uint:64,$KWD>` | Value in fils |
| Commodity price | `<float_dec:64,$USD/oz_t>` | $/troy oz |
| Exchange rate | `<float_dec:64,$USD/$EUR>` | USD per EUR |
| On-chain satoshi balance | `<uint:64,$BTC>` | Integer satoshis |
| Human-readable BTC amount | `<float_dec:64,$BTC>` | |

> **`float` (binary floating-point) is discouraged** for monetary amounts. Binary fractions cannot represent 0.10 USD exactly. Use `float_dec` for decimal-exact storage.
>
> **`float_fix` is wrong** for monetary values. Q-format stores values as `integer × 2^(-N)` — a binary fractional resolution. No power of 2 equals a power of 10 (except 2⁰ = 10⁰ = 1), so no Q value exactly represents cents.

---

## 10. Symbol Disambiguation

This section documents how a physical-unit token and a currency token are kept apart. The mandatory `$` currency sigil (§10.4) is the normative rule and resolves every potential collision; the look-alike tables that follow are retained for reference.

### 10.1 The Namespace Rule as Disambiguator

The mandatory `$` sigil (§9.1, normative rule in §10.4) makes the two namespaces disjoint by construction: the sigil — and nothing else — selects the currency table, so a bare token is **always** a physical-unit lookup and can never be mistaken for a currency.

| Written token | Looked up in | Result |
|---|---|---|
| `$USD`, `$BTC`, `k~$EUR`, `k$EUR` | currency table (sigil present) | currency, or `error_unit_illegal` if the code is unknown |
| `m`, `Hz`, `cup`, `BTU`, `k~g` (no `$`) | physical unit table only | physical unit, or `error_unit_illegal` if unknown |
| `USD`, `CUP`, `XYZ` (no `$`) | physical unit table only | `error_unit_illegal` — not physical units, and a bare code is never a currency |

Because the lookup is sigil-driven rather than spelling-driven, **case is no longer load-bearing for disambiguation**: `cup` and `CUP` are both physical-unit lookups (the first matches `bu_cup`, the second is `error_unit_illegal`), and the Cuban Peso is written `$CUP`.

### 10.2 Exhaustive Conflict Table

Before the sigil, several uppercase tokens *looked* like they could be either a physical unit or a currency. The sigil removes the ambiguity outright; the table below is retained as a reference for the codes that previously needed disambiguation. In every row, the bare form is a physical-unit lookup and the currency is only ever the `$`-prefixed form.

| Token | Bare form (no `$`) | `$`-sigil form | Notes |
|-------|--------------------|----------------|-------|
| `cup` / `CUP` | `cup` → US cup (`bu_cup`); `CUP` → `error_unit_illegal` | `$CUP` → Cuban Peso (ISO 4217:192) | the classic look-alike, now fully separated |
| `BTU` | `BTU` → International Table BTU (`bu_btu`); `Btu`, `btu` also accepted | *(not ISO 4217)* | uppercase `BTU` is a physical alias, not a currency |
| `SOL` | `SOL` → `error_unit_illegal` (no physical unit) | `$SOL` → Solana (crypto) | |
| `BAR` | `BAR` → `error_unit_illegal`; use lowercase `bar` | *(not ISO 4217)* | |
| `ERG` | `ERG` → `error_unit_illegal`; use lowercase `erg` | *(not ISO 4217)* | |
| `CAD`, `AUD`, `GBP`, `XAU` | `error_unit_illegal` (no physical unit) | `$CAD`, `$AUD`, `$GBP`, `$XAU` → the respective currencies | |

**Key finding:** with the sigil, no token is simultaneously a valid bare physical-unit symbol and a valid currency — currencies live entirely under `$`, physical units entirely without it.

### 10.3 The CUP Case in Detail

`CUP` is the classic example because the ISO 4217 code for the Cuban Peso shares its letters with the English word for the culinary measure. Under the sigil rule the two are unambiguous:

| Written in BVNR | Resolved as | Enum value | SI factor |
|-----------------|-------------|------------|-----------|
| `cup`           | US cup      | `bu_cup`   | 2.365882365×10⁻⁴ m³ |
| `cups`          | US cup (long form) | `bu_cup` | 2.365882365×10⁻⁴ m³ |
| `CUP`           | *(error)* — bare uppercase is not a physical unit | — | `error_unit_illegal` |
| `$CUP`          | Cuban Peso  | `CUP_` *(currency enum, ISO 4217:192)* | — (monetary, no SI factor) |

```bovnar
.recipe_volume = <float_dec:32,cup>  2.0;    # 2 US cups (volume)
.balance       = <float_dec:64,$CUP> 15.00;  # 15 Cuban Pesos (currency)
# .bad         = <float_dec:64,CUP>  15.00;  # error_unit_illegal: bare 'CUP' is not a unit
```

Calling `bvn_unit_to_si_factor` on a `$CUP` unit returns `*ok = false` because `bvn_find_si_conv` skips currency enum values. Calling `bvn_unit_is_currency` on a `cup` unit returns `false`. The two are completely disjoint in both parsing and the conversion API — and, because the peso *must* be written `$CUP`, a user who writes the bare `CUP` intending the culinary cup gets an immediate `error_unit_illegal` (the correct spelling is lowercase `cup`) rather than a silently-accepted currency.

### 10.4 The Mandatory Currency Sigil

A currency code carries a **mandatory `$` sigil** as of spec 1.0. This is the resolution of the `CUP` (Cuban Peso) vs `cup` (the physical cup) namespace collision: a currency is *only* recognised in its sigil form, so a bare code can never collide with a physical-unit symbol — present or future.

```bovnar
# Currencies — the '$' sigil is required:
.price   = <float_dec:64,$USD>      19.99;   # US Dollar
.btc     = <uint:64,$BTC>        54782000;   # Bitcoin (satoshis)
.fund    = <float_dec:64,k~$EUR>   250.0;    # kilo-Euro (prefix before the sigil)
.aum     = <float_dec:64,M$EUR>    42.8;     # mega-Euro, compact prefix (= M~$EUR)
.spot    = <float_dec:64,$USD/oz_t> 2351.40; # currency / physical-unit compound

# A bare code is no longer a currency:
.volume  = <float_dec:32,cup>        2.0;    # the physical 'cup', unambiguously
# .bad   = <float_dec:64,USD> 1.0;            # error_unit_illegal: bare 'USD' is not a unit (needs '$')
```

The sigil attaches directly before the currency code, after any SI/IEC prefix and its optional `~` (`k~$EUR` or `k$EUR`, `M~$USDT` or `M$USDT`; §4.3). It is accepted in inline units and type annotations alike, and the writer emits it on output so values round-trip. Because this **breaks** any document that used bare currency codes, it was made before the 1.0 freeze — afterwards it would be an incompatible change (see the Versioning & Stability section of the specification).

---

## 11. C Data Model

### 11.1 Enumerations

#### `prefix_system_t`

```c
typedef enum prefix_system_e {
    prefix_si,      /* SI decimal prefixes (or no prefix) */
    prefix_iec      /* IEC binary prefixes                */
} prefix_system_t;
```

#### `si_prefix_id_t`

```c
typedef enum si_prefix_id_e {
    si_none = 0,
    si_quecto, si_ronto, si_yocto, si_zepto, si_atto,
    si_femto,  si_pico,  si_nano,  si_micro, si_milli,
    si_centi,  si_deci,
    si_deca,   si_hecto, si_kilo,  si_mega,  si_giga,
    si_tera,   si_peta,  si_exa,   si_zetta, si_yotta,
    si_ronna,  si_quetta
} si_prefix_id_t;
```

#### `iec_prefix_id_t`

```c
typedef enum iec_prefix_id_e {
    iec_none = 0,
    iec_kibi, iec_mebi, iec_gibi, iec_tebi, iec_pebi,
    iec_exbi, iec_zebi, iec_yobi, iec_robi, iec_quebi
} iec_prefix_id_t;
```

#### `value_base_unit_t`

A base unit's id is **blocked**, not a running counter: the leading two decimal digits name the vocabulary it comes from and the four digits after them its position within that vocabulary. Each block therefore holds 10 000 ids, and a vocabulary that grows shifts nothing outside itself.

| Block | Ids | Vocabulary | Source |
|-------|-----|------------|--------|
| 10 | 100000–109999 | native bovnar units | `src/gendata/units.bvnr` |
| 20 | 200000–209999 | UCUM opaque units | `src/gendata/ucum.bvnr` |
| 30 | 300000–309999 | UN/ECE opaque units | `src/gendata/unece.bvnr` |
| 40 | 400000–409999 | QUDT opaque units | `src/gendata/qudt.bvnr` |
| 50 | 500000–509999 | QUDT quantity kinds | `src/gendata/qudt-qk.bvnr` |
| 60 | 600000–609999 | UDUNITS-2 opaque units | `src/gendata/udunits.bvnr` |
| 70 | 700000–709999 | OM 2 opaque units | `src/gendata/om.bvnr` |
| 80 | 800000–809999 | CF standard names | `src/gendata/cf.bvnr` |
| 90 | 900000–909999 | currencies | `src/gendata/currencies.bvnr` |

`bu_none` is 0 and belongs to no block; every tag between the native units and the currencies is now taken, so a further vocabulary needs one outside 10–90. Blocks 40–80 currently contribute no ids at all: those vocabularies map every code onto a native unit, and the block tag reserves their room rather than filling it. Within block 10 the native units run contiguously from `bu_bit` = 100000 to `bu_stattesla` = 100263, in the order of `units.bvnr`; `BVN_UNIT_NATIVE_FIRST`, `BVN_UNIT_NATIVE_LAST` and `BVN_UNIT_NATIVE_COUNT` are generated beside the enum, so read the bounds from those rather than from this sentence. Currencies run contiguously from 900000 and — unlike every other block — have **no named `bu_*` enumerators** (see §9.2/§9.3): a currency is written by its ISO 4217 code behind a `$` sigil, resolved by `bvn_parse_currency_str` and carried as the numeric `base` value. `bvn_unit_is_currency(base)` is a bounds check over block 90.

Only a vocabulary that contributes units of its *own* takes a block. A unit profile is a spelling for the unit slot, so most of its codes translate to native units and carry native ids; the ones that get a block id of their own are the **opaque** units — codes with no native equivalent and no dimension, such as UCUM's assay-defined `[IU]` or UN/ECE's package types — which need an identity precisely because nothing else can stand in for them.

> **Why blocks.** One flat counter makes every vocabulary's ids a function of every other vocabulary's size, and that was not hypothetical. This space previously had physical units at 1–133 *and again* at 348–396 because the currencies had been dropped in between; two currencies stranded at 378–379, outside their own range, because that range had been frozen; and the profiles' units pinned above all of it. Adding a currency shifted units. The 2.0 renumbering ended that, and made an id self-describing: 200017 is UCUM's, whatever else the build contains.
>
> The price is a **sparse** space, so do not index an array by a base unit id. The library's own tables are dense, one row per defined unit, indexed by `bvni_unit_slot()`; `BVN_UNIT_SLOT_COUNT` is that row count. A bounds check is no longer a membership test either — ask `bvn_unit_valid` on the unit, or `bvn_unit_is_currency` on the base.

```c
typedef enum value_base_unit_e {
    bu_none = 0,            /* dimensionless / no unit; in no block */

    /* Digital — block 10 opens here */
    bu_bit = 100000, bu_byte,

    /* SI base */
    bu_second, bu_meter, bu_gram, bu_ampere, bu_kelvin,
    bu_mol, bu_candela,

    /* Named SI-derived */
    bu_hertz, bu_newton, bu_pascal, bu_joule, bu_watt,
    bu_volt,  bu_ohm,   bu_farad,  bu_coulomb, bu_siemens,
    bu_weber, bu_tesla, bu_henry,  bu_lumen,   bu_lux,
    bu_becquerel, bu_gray, bu_sievert, bu_katal,
    bu_liter, bu_minute, bu_hour, bu_day, bu_degree, bu_celsius,
    bu_radian, bu_steradian,
    bu_tonne, bu_bar,
    bu_electronvolt, bu_dalton, bu_astronomical_unit,
    bu_hectare, bu_week, bu_year,

    /* Imperial/US — length */
    bu_inch, bu_foot, bu_yard, bu_mile, bu_nautical_mile,
    bu_angstrom, bu_light_year, bu_parsec, bu_furlong, bu_fathom,

    /* Imperial/US — mass */
    bu_pound, bu_ounce, bu_grain, bu_stone, bu_short_ton,
    bu_long_ton, bu_troy_ounce, bu_carat,

    /* Temperature */
    bu_fahrenheit,

    /* Pressure */
    bu_atmosphere, bu_mmhg, bu_torr, bu_psi,

    /* Energy */
    bu_calorie, bu_btu, bu_erg, bu_therm,

    /* Power */
    bu_horsepower,

    /* Force */
    bu_pound_force, bu_dyne, bu_kip,

    /* Speed */
    bu_knot,

    /* Volume — US */
    bu_gallon, bu_gallon_uk, bu_quart, bu_pint, bu_cup,
    bu_fluid_ounce, bu_tablespoon, bu_teaspoon, bu_barrel,

    /* Area */
    bu_acre, bu_barn,

    /* Angle */
    bu_arcminute, bu_arcsecond, bu_grad,

    /* CGS */
    bu_poise, bu_stokes, bu_gauss, bu_maxwell, bu_oersted,
    bu_stilb, bu_phot, bu_galileo,

    /* Radiation */
    bu_curie, bu_roentgen, bu_rem,

    /* Logarithmic */
    bu_neper, bu_decibel,

    bu_rankine,             /* Temperature — absolute Fahrenheit scale */
    bu_slug,                /* Imperial mass */
    bu_thou,                /* Imperial length */

    /* Volume — UK imperial */
    bu_pint_uk, bu_fluid_ounce_uk, bu_quart_uk,

    /* Electrical power */
    bu_var, bu_volt_ampere,

    /* Force (additional) */
    bu_kilogram_force,

    /* Pressure (additional) */
    bu_inch_hg,

    /* Rotational frequency */
    bu_rpm,

    /* Energy (additional) */
    bu_foot_pound,

    /* Mass (additional) */
    bu_dram, bu_pennyweight,

    /* Length (additional) */
    bu_chain, bu_rod,

    /* Volume (additional) */
    bu_gill, bu_gill_uk,

    /* Acceleration / gravity */
    bu_standard_gravity,

    /* Power (additional) */
    bu_metric_horsepower,

    /* Angle (additional) */
    bu_revolution,

    /* Time (additional) */
    bu_month, bu_fortnight,

    /* Pressure (additional) */
    bu_atmosphere_technical,

    /* Textile */
    bu_tex, bu_denier,

    /* Apothecary / dry volume */
    bu_fluid_dram, bu_minim, bu_peck, bu_bushel,

    /* Old German */
    bu_pfund, bu_zentner, bu_doppelzentner, bu_lot,
    bu_prussian_line, bu_prussian_zoll, bu_prussian_fuss,
    bu_prussian_elle, bu_prussian_rute, bu_klafter,
    bu_german_mile, bu_morgen, bu_scheffel,

    /* Additional physical units */
    bu_survey_foot, bu_league, bu_cable, bu_hand,
    bu_quintal, bu_scruple, bu_baud,

    /* Historical temperature scales */
    bu_delisle, bu_newton_temp, bu_reaumur, bu_romer,

    /* Dimensionless ratio units */
    bu_percent, bu_per_mille, bu_per_myriad,
    bu_per_cent_mille, bu_ppm, bu_ppb,   /* bu_ppt and bu_ppq close
                                          * block 10 — see the note below */

    /* … the pH scale, the named speed units, the five water-hardness
     * degrees, the concentration units, and the turbidity and salinity
     * scales — then: */

    /* Temperature differences (Δ°C shares bu_delta_kelvin, Δ°Ra shares
     * bu_delta_fahrenheit — the same interval, so an alias not an id) */
    bu_delta_kelvin, bu_delta_fahrenheit, bu_delta_delisle,
    bu_delta_newton_temp, bu_delta_reaumur, bu_delta_romer,

    /* … the six units the profiles needed, the US survey lengths, the
     * typographic lengths, the US dry volumes and the trade measures, and
     * the rest of block 10 up to bu_stattesla = 100263. The whole run is
     * generated into include/bovnar_units.gen.h from
     * src/gendata/units.bvnr, in that file's order — read it there rather
     * than from this abridged listing. */

    /* Block 90 — the currencies — has NO named enumerators: a currency is
     * resolved by string via bvn_parse_currency_str and carried as the
     * numeric base value, and the catalogue in bovnar_currency.c is
     * index-aligned to BVN_CURRENCY_FIRST. */

    /* Blocks 20–80 — the profiles' opaque units — are generated into
     * include/bovnar_profiles.gen.h: bu_ucum_iu = 200000, bu_unece_one =
     * 300000, and so on. Only 20 and 30 contribute any today. */
} value_base_unit_t;

/* Not a member of the enum, and NOT a bound on it: the number of rows in the
 * dense tables the library indexes by bvni_unit_slot(). The id space is
 * sparse, so it has no "one past the end". */
/* #define BVN_UNIT_SLOT_COUNT 331 */
```

#### `unit_exponent_t`

```c
#define BVN_EXPONENT_MIN	(-100)
#define BVN_EXPONENT_MAX	( 100)

typedef enum unit_exponent_e {
    exp_range_min  = BVN_EXPONENT_MIN,
    exp_range_max  = BVN_EXPONENT_MAX,
    exp_invalid    =   0,
    exp_linear     =   1,  exp_square    =  2,  exp_cubic     =  3,
    exp_quartic    =   4,  exp_quintic   =  5,  exp_sextic    =  6,
    exp_septic     =   7,  exp_octic     =  8,  exp_nonic     =  9,
    exp_neg_linear =  -1,  exp_neg_square= -2,  exp_neg_cubic = -3,
    exp_neg_quartic=  -4,  exp_neg_quintic=-5,  exp_neg_sextic= -6,
    exp_neg_septic =  -7,  exp_neg_octic = -8,  exp_neg_nonic = -9,
} unit_exponent_t;
```

**The named enumerators are not the range.** They cover ±1…±9 and are kept because callers use them;
the type carries any integer in `[BVN_EXPONENT_MIN, BVN_EXPONENT_MAX]` = ±100, which is what §6 and
the grammar describe. `exp_range_min`/`exp_range_max` are enumerators so that the underlying type is
wide enough for them on every conforming compiler; neither is a valid exponent to *use*. Do not
switch exhaustively over this enum, and do not assume a value outside the named nine is invalid —
ask `bvn_int_to_exponent`, which returns `exp_invalid` for anything out of range.

`exp_invalid` (value `0`) is the zero-initialization sentinel. Always initialize components before use.

### 11.2 Structures

#### `value_unit_component_t`

```c
typedef struct value_unit_component_s {
    value_base_unit_t   base;
    unit_exponent_t     exponent;
    value_unit_prefix_t prefix;
} value_unit_component_t;
```

where:

```c
typedef struct value_unit_prefix_s {
    prefix_system_t system;
    union {
        si_prefix_id_t   si;
        iec_prefix_id_t  iec;
    } id;
} value_unit_prefix_t;
```

#### `value_unit_t`

```c
#define BVNR_MAX_UNIT_COMPONENTS  32

typedef struct value_unit_s {
    uint32_t               num_components;
    value_unit_component_t components[BVNR_MAX_UNIT_COMPONENTS];
} value_unit_t;
```

For an explicit `no_unit` annotation, `num_components == 0` (`BVN_UNIT_NONE`). For an absent unit parameter in an explicit annotation (e.g. `<float:64>`), `num_components == 1` with `base == bu_none`.

#### `bvnr_data_t` (unit field)

```c
typedef struct bvnr_data_s {
    token_type_t       type;
    value_type_spec_t  value_type;
    value_unit_t       value_unit;   /* parsed physical unit or currency */
    const void*        data;
    uint32_t           length;
    const void*        frac_data;    /* spec 1.1 — ISO datetime sub-second digits, else NULL */
    uint32_t           frac_length;  /* spec 1.1 — length of frac_data, else 0 */
    const void*        unit_source;        /* library 2.0 — value_unit as the document spelled it */
    uint32_t           unit_source_length; /* library 2.0 — its length, 0 with NULL */
    bool               converted;    /* a read-time conversion restated this value */
    bvnr_converted_t   conv;         /* its result: the unit delivered, and the digits */
} bvnr_data_t;
```

`unit_source` is the unit's own **spelling**, where `value_unit` is its meaning.
The two differ wherever formatting a `value_unit_t` is lossy — `km` and `k~m` are
one unit, and so are `ucum:mL{total}` and `m~L` — and the writer re-emits the
spelling when it is handed back. See
[Unit Profiles §5.2](11_bovnar_unit_profiles.md#52-what-canonical-output-loses-and-on-which-path).

`converted` and `conv` are the unit-facing half of the struct: when a `want_unit`
hook or a [unit policy](06_bovnar_unit_policy.md) restated a value, `conv`
carries the unit it was delivered in and the converted digits, while
`value_unit` still carries the unit the **document** wrote. A consumer that
requested a conversion must read `converted` to know whether it happened — see
[Unit Policy §2.2](06_bovnar_unit_policy.md#22-conversion-targets).

### 11.3 Convenience Macros

```c
/* Dimensionless or single-component without prefix */
#define BVN_UNIT_NO_PREFIX(b)          \
    ((value_unit_t){ .num_components = 1, .components = {{  \
        .base=(b), .exponent=exp_linear,                    \
        .prefix.system=prefix_si, .prefix.id.si=si_none }}})

/* Single-component with SI prefix */
#define BVN_UNIT_SI(b, p)  \
    ((value_unit_t){ .num_components = 1, .components = {{  \
        .base=(b), .exponent=exp_linear,                    \
        .prefix.system=prefix_si, .prefix.id.si=(p) }}})

/* Single-component with IEC prefix */
#define BVN_UNIT_IEC(b, p) \
    ((value_unit_t){ .num_components = 1, .components = {{  \
        .base=(b), .exponent=exp_linear,                    \
        .prefix.system=prefix_iec, .prefix.id.iec=(p) }}})

/* Single-component with SI prefix and explicit exponent */
#define BVN_UNIT_SI_EXP(b, p, e) \
    ((value_unit_t){ .num_components = 1, .components = {{  \
        .base=(b), .exponent=(e),                           \
        .prefix.system=prefix_si, .prefix.id.si=(p) }}})

/* Empty unit — explicit no_unit / internal sentinel */
#define BVN_UNIT_NONE  ((value_unit_t){ .num_components = 0 })

/* Two-component compound unit, both SI-prefixed */
#define BVN_UNIT_COMPOUND2(b1,p1,e1, b2,p2,e2)                        \
    ((value_unit_t){ .num_components = 2, .components = {              \
        { .base=(b1), .exponent=(e1),                                  \
          .prefix.system=prefix_si, .prefix.id.si=(p1) },              \
        { .base=(b2), .exponent=(e2),                                  \
          .prefix.system=prefix_si, .prefix.id.si=(p2) }}})
```

Usage:

```c
value_unit_t kg  = BVN_UNIT_SI(bu_gram, si_kilo);
value_unit_t gib = BVN_UNIT_IEC(bu_byte, iec_gibi);
value_unit_t m2  = BVN_UNIT_SI_EXP(bu_meter, si_none, exp_square);
value_unit_t usd = BVN_UNIT_NO_PREFIX(            /* a currency: ids come from */
    bvn_parse_currency_str((const uint8_t *)"USD", 3));  /* the catalogue, not an enumerator */
value_unit_t none = BVN_UNIT_NONE;
```

---

## 12. C API Functions

### 12.1 Parsing a Unit String

```c
value_unit_t bvn_parse_unit  (const uint8_t* unit, bool* ok);
value_unit_t bvn_parse_unit_n(const uint8_t* unit, uint32_t len, bool* ok);
```

`bvn_parse_unit` parses a NUL-terminated UTF-8 unit string. `bvn_parse_unit_n` is the length-bounded variant used internally when the unit string is a slice of a larger type-annotation buffer. Both set `*ok = false` on any parse error (unknown prefix, unknown base unit, unknown currency code, too many components, empty component, or NULL input).

Single-pass parsing algorithm:

1. `memcmp` against `"no_unit"` → return `BVN_UNIT_NONE` immediately on match.
2. Scan for separator characters to distinguish simple vs. compound paths.
3. For compound units, split on `0x2A` (`*`), `0x2F` (`/`), `0xC2 0xB7` (`·`); parse each slice as a component; negate denominator exponents.
4. For each component, if it is introduced by the `$` sigil (after any prefix and its optional `~`), look the code up in the currency table; otherwise look it up in the physical unit table only. A bare code is never a currency.

```c
bool ok;
value_unit_t u = bvn_parse_unit((const uint8_t *)"k~g·m/s²", &ok);
/* u.num_components == 3:
   [0]: bu_gram,   exp_linear,     si_kilo
   [1]: bu_meter,  exp_linear,     si_none
   [2]: bu_second, exp_neg_square, si_none */

value_unit_t c = bvn_parse_unit((const uint8_t *)"$USD/oz_t", &ok);
/* c.num_components == 2:
   [0]: the id of USD, exp_linear,  si_none  (currency)
   [1]: bu_troy_ounce, exp_neg_linear, si_none */
```

### 12.2 Serializing a Unit

```c
int32_t bvn_unit_to_string   (value_unit_t u, char* buf, size_t bufsize);
int32_t bvn_unit_to_string_ex(value_unit_t u, char* buf, size_t bufsize,
                               bvn_unit_flags_t flags);
```

Serializes `u` to a canonical UTF-8 string. Returns the number of bytes written (excluding NUL) or `-1` on buffer overflow or invalid component.

| Flag | Effect |
|------|--------|
| `BVN_UNIT_FLAGS_NONE` | Unicode superscript exponents, no reduction |
| `BVN_UNIT_ASCII_EXP` | ASCII caret form (`^N`) for all exponents |
| `BVN_UNIT_REDUCE` | Reduce via `bvn_unit_reduce` before serializing |

> **`BVN_UNIT_REDUCE` changes the unit, and this function does not change the value.**
> Reduction folds every prefix out, so `k~g` serializes as `"g"` — a string that
> denotes a quantity 1000× smaller than what you passed in. The value has to move
> with it, and the **writer** does that for you (5 `k~m` is written as `5000 m`,
> in exact rational arithmetic); nothing else does.
>
> **A direct caller must not use `bvn_unit_reduce`'s `scale` for this.** That is
> the scale to the *fully reduced* unit, and this function does not always emit
> the fully reduced unit: where the reduction lands on a named SI unit the
> formatter re-attaches the prefix, so `k~N` comes back `"k~N"` with nothing to
> rescale, while `bvn_unit_reduce` still reports 1000. Applying it there
> multiplies by a thousand twice over, and the two cases are indistinguishable
> from outside — both are a lone unit carrying a kilo prefix.
>
> The recipe is to convert to the unit that is actually **emitted**, which is
> what `bvnr_writer.c` does:
>
> ```c
> char ubuf[BVNR_UNIT_STRING_MAX];
> if (bvn_unit_to_string_ex(u, ubuf, sizeof ubuf, BVN_UNIT_REDUCE) < 0)
>         return false;                       /* overflowing reduction */
> bool ok = false;
> value_unit_t emitted = bvn_parse_unit((const uint8_t *)ubuf, &ok);
> if (!ok || bvn_unit_equal(emitted, u))
>         return true;                        /* nothing moved */
> /* now convert the value from `u` to `emitted` — exactly, if it must be exact */
> bvn_unit_convert_rational(vnum, vden, u, emitted, out_num, out_den, &exact);
> ```
>
> `bvn_unit_reduce`'s `scale` remains the right answer for its own returned unit;
> it is simply not the unit this function prints.
>
> The collapse never *substitutes* one named unit for another. `Sv` and `Gy` share
> a dimension vector, as do `Bq`, `Bd` and `Hz`, and `W`, `VA` and `var`; each
> reduces to itself. A lone base at an exponent other than 1 is a different case
> and still collapses — `s⁻¹` is what `Hz` names, and `m~s⁻¹` comes back `k~Hz`.
>
> **A reduction that drops a component returns `-1` rather than a unit.** When a
> summed exponent leaves the range the format can spell, or more bases survive
> than a unit may carry, `bvn_unit_reduce` drops a component — so the result is a
> *different* unit, not a shorter spelling of the same one. `m¹⁰⁰·m²` is `m¹⁰²`,
> and such a unit used to serialise as `"no_unit"`. Without `BVN_UNIT_REDUCE`
> these units write normally.
>
> A folded scale that no `double` holds is **not** that case. `y~m²⁰` reduces to
> `m²⁰` with a scale of 10⁻⁴⁸⁰: `bvn_unit_reduce` still raises `*overflow` for it
> and leaves `*scale` at 1.0, because the scale is what it cannot return, but the
> unit is intact and this function writes it. The value is moved by the recipe
> above, which is exact and never passes through a `double`; where the moved
> value has no form in its own type, the writer reports
> `error_value_out_of_range`, about the value. Rewriting an equivalent dose as an absorbed dose in the
> document would be a stronger act than offering the conversion, which §12.4 still
> does when a caller asks.

```c
char buf[64];
value_unit_t u = /* k~g·m/s² */;
bvn_unit_to_string(u, buf, sizeof(buf));
/* buf == "k~g·m/s²" */

bvn_unit_to_string_ex(u, buf, sizeof(buf), BVN_UNIT_ASCII_EXP);
/* buf == "k~g*m/s^2" */
```

#### Validation Predicate

```c
bool bvn_unit_valid(value_unit_t u);
```

Returns `true` if every component has a valid exponent, known base unit (physical or currency), and a legal prefix for that base. Both serialization functions call this before writing.

### 12.3 Prefix Factor and Exponent Queries

```c
double  bvn_unit_prefix_factor  (value_unit_t u);
int32_t bvn_unit_prefix_exponent(value_unit_t u);
```

`bvn_unit_prefix_factor` returns the multiplicative scale contributed by the prefixes, ignoring base-unit identity. For `si_none`/`iec_none` prefixes the factor is 1.0.

`bvn_unit_prefix_exponent` returns the sum of `(prefix_base_exponent × unit_exponent)` across all components — the component exponent **signed**, not its magnitude, so `m/k~m` is `-3` and `k~m⁻¹` is `-3` while `k~m` is `+3`.

> **The sum is only meaningful when one prefix system is in play.** SI exponents are powers of ten and IEC exponents powers of two, and this function adds them into a single integer: `Ki~B/k~s` returns `7`, which is neither 2⁷ nor 10⁷ — the actual scale is 1.024. For a mixed unit use `bvn_unit_prefix_factor`, which multiplies the two systems' factors correctly, or read the components yourself.

### 12.4 SI Conversion API

Functions in `bovnar_si_units.h` provide dimensional analysis, compatibility checking, and value conversion between compatible physical units. **A currency has no SI conversion row at all** — it has a catalogue row in `bovnar_currency.c` instead — so `bvn_unit_to_si_factor` and `bvn_unit_dimension_vector` set `*ok = false` / return `false` for one, and `bvn_units_compatible` reports a currency incompatible even with itself. `bvn_units_convertible`, `bvn_unit_convert_value` and `bvn_unit_convert_rational` still handle currencies, through the prefix-only path described below; that difference is the reason the first of those three exists.

```c
/* Full SI factor (physical units only) */
double bvn_unit_to_si_factor(value_unit_t u,
                              bool   *is_affine,
                              double *affine_offset,
                              bool   *ok);

/* SI dimension vector */
bool bvn_unit_dimension_vector(value_unit_t u,
                                int32_t dims[bvn_si_dim_count]);

/* Dimensional compatibility check */
bool bvn_units_compatible(value_unit_t a, value_unit_t b);

/* The predicate to SCREEN a conversion target with */
bool bvn_units_convertible(value_unit_t a, value_unit_t b);

/* The coherent SI form of a unit, if it has one */
bool bvn_unit_si_normal_form(value_unit_t u, value_unit_t *out);

/* Conversion factor: value_in_b = value_in_a × k */
double bvn_unit_convert_factor(value_unit_t a, value_unit_t b,
                                bool *ok, bool *requires_affine);

/* Unit reduction */
value_unit_t bvn_unit_reduce(value_unit_t u,
                              double *scale, bool *overflow);

/* Prefix validity */
bool bvn_prefix_unit_valid(value_unit_prefix_t prefix,
                            value_base_unit_t base);

/* Convert one value (handles the affine scales) */
bool bvn_unit_convert_value(double value, value_unit_t from,
                             value_unit_t to, double *out);

/* Convert one EXACT rational — the lossless path */
bool bvn_unit_convert_rational(const bvn_int_t *vnum, const bvn_int_t *vden,
                                value_unit_t from, value_unit_t to,
                                bvn_int_t *out_num, bvn_int_t *out_den,
                                bool *exact);

/* Exponent integer conversion */
int32_t        bvn_exponent_to_int (unit_exponent_t e);
unit_exponent_t bvn_int_to_exponent(int32_t n);
```

`bvn_unit_convert_rational` is the engine behind the reader's `want_unit` hook and behind every conversion a unit policy performs: it converts the exact rational `vnum/vden` in arbitrary precision, so a 1056-bit float or a 512-bit integer converts with no loss beyond the library's own declared factor. It sets `*exact = false` when the true factor is irrational (a π-based angle, a parsec, a water-hardness scale) — the result is then only an approximation and a lossless consumer must reject it. It returns `false` in three cases a caller reporting a diagnosis must tell apart: the units are dimensionally incompatible, the unit is structurally invalid, or the exact factor needs more than `BVN_INT_MAX_BITS` (reachable only from deliberately extreme units — `Q~m¹⁰⁰·Q~g¹⁰⁰` to `q~m¹⁰⁰·q~g¹⁰⁰` needs 10^12000 — and **not** a statement that the units disagree). Ask `bvn_units_convertible` to separate the first two from the third. To render the result, see `bvn_rational_to_str` and `bvn_rational_str_bufsize` in §3.4 of the [read/write API](08_bovnar_readwrite_api.md) — the renderer never truncates, so size the buffer with the second before calling the first.

**Every out-parameter above is optional.** `NULL` means "do not report this one"; the function behaves identically otherwise and its return value is unchanged. That makes `bvn_unit_dimension_vector(u, NULL)` the bare predicate "does `u` have a dimension vector at all", and `bvn_unit_si_normal_form(u, NULL)` the predicate "does `u` have an SI form". The rule does not extend to an argument the answer is made *of*: a `NULL` `bvn_int_t` in `bvn_unit_convert_rational`, or a `NULL` buffer handed to a formatter, is a refused call.

**`bvn_units_convertible` is the one to screen a conversion target with**, and it is not `bvn_units_compatible`. That function answers "do these two carry the same physical dimension", and a currency deliberately carries none — so it reports `false` for `k~$USD → $USD` and even for `$USD → $USD`, both of which the conversion entry points perform correctly. `bvn_unit_convert_factor` is not the answer either: it reports `*ok = false` for `°F → °C`, because an affine conversion has no single multiplicative factor, so screening on it drops every temperature in the format. `bvn_units_convertible` is "dimensionally compatible, **or** the same unit apart from its prefixes", which is the set `bvn_unit_convert_value` and `bvn_unit_convert_rational` draw from — a **superset** of what they accept, not the same set.

It is a *screen*, not a guarantee, and the gap is exactly **two** shapes wide. Code that screens with this must still handle a conversion that declines, and the two reasons it can decline are worth telling apart:

- **An affine scale outside "alone, at exponent 1".** `s/°C` is dimensionally compatible with `s/K` and passes here; the conversion entry points refuse it, because an affine scale means nothing at an exponent other than 1 (§3.12 above; see also [Unit Ambiguities §10](07_bovnar_unit_ambiguities.md)). This one is reachable from ordinary units and is the one a policy has to fence.
- **Capacity.** The exact factor needs more than `BVN_INT_MAX_BITS`, as the paragraph above describes: `Q~m¹⁰⁰ → q~m¹⁰⁰` is 10⁶⁰⁰⁰ and declines, while remaining a perfectly well-defined conversion between compatible units. This is *why* the screen answers `true` there — separating "too large to compute" from "these units disagree" is the question it exists to answer, and it is reachable only from deliberately extreme prefixes and exponents.

Nothing else is in the gap. `python/tests/test_convertible_implies_convertible.py` sweeps the catalogue and its quotients and squares — a domain that cannot reach the capacity bound — and requires every pair that passes the screen and is then refused to be the affine shape; a companion case pins the capacity shape directly.

The reader's unit policy closes the gap on its own side rather than living with it: `bvn_policy_selects` asks `bvni_unit_affine_misplaced` before it asks this, so a whole-document **target** never selects such a pair — leaving the value untouched, as an unmatched target should — while a per-field **rule** or a `require_dimension_of` naming it is `error_unit_mismatch`, because a rule is an assertion the caller made by naming the field. Screening on the dimensional answer alone let `--field .rate=K/h` report OK against a `°C/h` document.

**`bvn_unit_si_normal_form`** writes the coherent SI form of `u` — the product of SI base units carrying the same dimension, prefixes folded out (mass comes back as `k~g`, the SI base unit for mass being the kilogram). It returns `false`, leaving `*out` untouched, when there is no such form to name:

- a **currency**, which has no dimension vector at all;
- any **dimensionless** unit (`%`, `ppm`, `dB`, `pH`, `rad`, `°`, the turbidity scales). Deliberate: normalising a ratio would silently turn `35 %` into a bare `0.35`, and normalising an angle would need the irrational factor between `°` and `rad`;
- a unit whose SI form the conversion engine would then refuse. The dimension vector does not determine a unit: `lm`, `lx` and `ph` carry the steradian's quantity kind, which no dimension vector can express, so rebuilding them from dimensions yields `cd` and `cd/m²` — a different quantity (§3.2). `s/°C` has the dimensions of `s/K` and is unconvertible for the affine reason above.

The form returned is checked against `u` with `bvn_units_convertible` before it is returned, so a form this function gives back is one the conversion entry points accept. A temperature **interval** normalises to `ΔK` and never to `K`, which the dimension vector cannot say — both are Θ¹ — so the quantity kind is consulted (§3.12). This is the function `bvnr_normalise_si` is built on.

For affine units (`bu_celsius`, `bu_fahrenheit`), `*is_affine` is set to `true` and `*affine_offset` receives the additive offset applied after multiplying by the returned factor.

**An affine unit has an SI value only alone, at exponent 1.** `°C²`, `°C·°F`, `°C/h` and `°C·m` all set `*ok = false`; `bvn_unit_convert_value` and `bvn_unit_convert_rational` refuse them, and the reader turns that into `error_unit_mismatch` (38). The offset is a number of kelvin, and a product whose SI unit is `K·s⁻¹` or `K·m` has nowhere to put it: the earlier code added it unscaled, so `20 °C/h` converted to `K/h` as 983360 and `20 °C·m` to `K·m` as 293.15 whatever the metres did. Both are arithmetic on a quantity that does not exist. `°C/h` still **parses** and is still a legal annotation — a consumer that means a temperature *difference* can read the components itself and apply its own semantics; what the library will not do is hand back a number for it. This matches pint, which forbids an offset unit inside a product outright.

```c
bool ok, affine;
double offset;
value_unit_t u = bvn_parse_unit((const uint8_t *)"°C", &ok);
double f = bvn_unit_to_si_factor(u, &affine, &offset, &ok);
/* f == 1.0, affine == true, offset == 273.15 */
```

### 12.5 Currency API

```c
#include "bovnar_currency.h"

/* Classification */
bool bvn_unit_is_currency(int base);
bool bvn_unit_is_fiat    (int base);
bool bvn_unit_is_crypto  (int base);

/* Minor-unit exponent: 1 major unit = 10^N minor units */
uint8_t bvn_currency_minor_unit(int base, bool *ok);

/* Full currency metadata */
const bvn_currency_info_t *bvn_currency_info(int base);

/* Look up by 3–4 char code string; returns 0 (bu_none) on failure */
int bvn_parse_currency_str(const uint8_t *s, uint32_t len);

/* Prefix validity for a specific currency */
bool bvn_currency_prefix_valid(int base, int prefix_system);
```

The `bvn_currency_info_t` structure:

```c
typedef struct {
    char     code[5];           /* "USD", "BTC", etc.                  */
    uint16_t numeric_code;      /* ISO 4217 numeric code (0 for crypto) */
    uint8_t  minor_unit;        /* decimal places                       */
    bool     is_crypto;         /* true for cryptocurrencies            */
    char     name[48];          /* "US Dollar", "Bitcoin", etc.         */
} bvn_currency_info_t;
```

Examples:

```c
bool ok;
int kwd = bvn_parse_currency_str((const uint8_t *)"KWD", 3);
int jpy = bvn_parse_currency_str((const uint8_t *)"JPY", 3);
int usd = bvn_parse_currency_str((const uint8_t *)"USD", 3);
uint8_t n = bvn_currency_minor_unit(kwd, &ok);     /* n=3, ok=true  */
uint8_t m = bvn_currency_minor_unit(jpy, &ok);     /* m=0, ok=true  */
uint8_t x = bvn_currency_minor_unit(bu_meter, &ok);/* x=0, ok=false */

const bvn_currency_info_t *ci = bvn_currency_info(usd);
/* ci->code="USD", ci->numeric_code=840, ci->minor_unit=2,
   ci->is_crypto=false, ci->name="US Dollar" */

int cv = bvn_parse_currency_str((const uint8_t *)"EUR", 3);  /* cv=177 */
int cc = bvn_parse_currency_str((const uint8_t *)"DOGE", 4); /* cc=326 */
int cx = bvn_parse_currency_str((const uint8_t *)"xyz", 3);  /* cx=0   */

/* Distinguishing cup (volume) from CUP (currency) in code: */
value_unit_t volume   = bvn_parse_unit((const uint8_t *)"cup", &ok);
value_unit_t currency = bvn_parse_unit((const uint8_t *)"CUP", &ok);
assert(!bvn_unit_is_currency(volume.components[0].base));   /* true */
assert( bvn_unit_is_currency(currency.components[0].base)); /* true */
```

### 12.6 Unit Profile API (spec 1.2)

Four functions serve the profile notations. `bvn_parse_unit` itself is unchanged and takes every notation; these cover what a caller needs *around* it.

```c
error_code_t bvn_unit_error_code(const uint8_t *unit, uint32_t len);
bool         bvn_unit_is_profile_only(value_unit_t u);
int32_t      bvn_unit_to_profile(const char *ns, value_unit_t u,
                                 char *buf, size_t bufsize);
int32_t      bvn_unit_to_ucum(value_unit_t u, char *buf, size_t bufsize);
```

**`bvn_unit_error_code`** says *why* a unit string `bvn_parse_unit` rejected is not a unit — `error_unit_illegal` for malformed input or an unknown atom, `error_unit_profile_unknown` for an unrecognised namespace, `error_unit_profile_unsupported` for a valid profile expression with no representation here. It re-parses, so it is for the error path only; a string that does parse returns `error_none`.

**`bvn_unit_is_profile_only`** is true when a unit has no native spelling, which is exactly the units carrying an **opaque** base unit: a UCUM arbitrary atom (`[IU]`, `[PFU]`, …) or a UNECE package or count code (`XBX`, `C62`, …). For those, `bvn_unit_to_string` emits the profile form in the namespace that *owns* the unit, and re-parsing that output yields the same unit.

**`bvn_unit_to_profile`** writes a code in the named vocabulary — any of the seven namespaces, without the `<ns>:` prefix, though the read-only `cf` has a code for no unit — returning its length or a negative value. **`bvn_unit_to_ucum`** is the `ucum` case, kept for callers that predate the others. Both are **partial by construction**, in three ways: the Old German units, the water-hardness degrees, the turbidity kinds, `PSU`, `CF`, `kph` and every currency have no code in any of these vocabularies (membership is per unit, not per family: `mph` is `unece:HM` and `qudt:MI-PER-HR`, while `kph` has nothing); an opaque unit belonging to a *different* profile has none either; and a **flat** vocabulary (`unece`, `qudt`, `qudt-qk`, `om`) can spell only a single component — prefixed or not, `k~m` is `unece:KMT` — so `k~m/h` has no UNECE form even though `unece:KMH` parses to exactly it. All three are refused rather than approximated.

```c
char buf[BVNR_UNIT_STRING_MAX];
bool ok = true;
value_unit_t u = bvn_parse_unit((const uint8_t *)"ucum:mm[Hg]", &ok);   /* == mmHg */
bvn_unit_to_string(u, buf, sizeof buf);                                 /* "mmHg"    */
bvn_unit_to_ucum  (u, buf, sizeof buf);                                 /* "mm[Hg]"  */
```

Note that **in a document** the notation is gated on an explicit `#!bovnar 1.2` directive (§11.9 of the specification). These API entry points have no document and therefore no version, so they accept the notation unconditionally.

### 12.7 Python API

```python
from bovnar.enums import BaseUnit
from bovnar.currency import (
    is_currency, is_fiat, is_crypto,
    minor_unit, currency_info, currency_name, from_code,
    all_fiat, all_crypto,
)

assert is_currency(BaseUnit.USD)        # True
assert is_fiat(BaseUnit.XAU)            # True (gold is ISO 4217 X-code)
assert is_crypto(BaseUnit.ETH)          # True
assert not is_fiat(BaseUnit.BTC)        # True (BTC is crypto, not fiat)

assert minor_unit(BaseUnit.USD)  == 2   # cents
assert minor_unit(BaseUnit.JPY)  == 0   # indivisible
assert minor_unit(BaseUnit.KWD)  == 3   # fils
assert minor_unit(BaseUnit.BTC)  == 8   # satoshis
assert minor_unit(BaseUnit.ETH)  == 18  # wei

info = currency_info(BaseUnit.EUR)
assert info.code == "EUR"
assert info.numeric_code == 978
assert info.minor_unit == 2

btc = from_code("BTC")
assert btc == BaseUnit.BTC

fiat_count   = sum(1 for _ in all_fiat())    # 166
crypto_count = sum(1 for _ in all_crypto())  # 50
```

---

## 13. Integration with the Parser Event Stream

Unit information flows into the application through two paths:

1. **Type-annotation unit** — parsed from `<family:…,unit-param>` by the lexer, validated by the validator, delivered in the `ev_type_annotation_type_family_parameter` unit event.
2. **Inline unit suffix** — parsed from the suffix following a scalar value literal before the terminating `;`.

In both cases the effective unit is reported in the `bvnr_data_t.value_unit` field of the `ev_data` event.

### 13.1 Full event sequence — physical unit

```
Input: .force = <float:64,k~g·m/s²> 9.81;

ev_assignment_start          data = "force"
ev_type_annotation_start     data = "float:64,k~g·m/s²"
ev_type_annotation_type_family  data = "float"
ev_type_annotation_type_family_parameter   ← width=64
ev_type_annotation_type_family_parameter   ← unit:
    value_unit = { num_components=3,
      [0] bu_gram,   exp_linear,     {prefix_si, si_kilo}
      [1] bu_meter,  exp_linear,     {prefix_si, si_none}
      [2] bu_second, exp_neg_square, {prefix_si, si_none} }
ev_type_annotation_end
ev_data   data="9.81"
```

### 13.2 Full event sequence — currency unit

```
Input: .price = <float_dec:64,$USD> 19.99;

ev_assignment_start          data = "price"
ev_type_annotation_start     data = "float_dec:64,$USD"
ev_type_annotation_type_family  data = "float_dec"
ev_type_annotation_type_family_parameter   ← width=64
ev_type_annotation_type_family_parameter   ← unit:
    value_unit = { num_components=1,
      [0] USD, exp_linear, {prefix_si, si_none} }
ev_type_annotation_end
ev_data   data="19.99"
```

For events with an explicit type annotation but no unit parameter, the unit event is **not emitted**. For `no_unit`, the unit event IS emitted with `BVN_UNIT_NONE` (`num_components=0`). For synthesised (default) type annotations, the unit event IS emitted with `BVN_UNIT_NO_PREFIX(bu_none)`.

### 13.3 Inline unit suffix — event stream view

```
Input: .distance = 1500 m;

ev_assignment_start      data = "distance"
ev_type_annotation_start data = "uint"      ← synthesised
ev_type_annotation_type_family  data = "uint"
ev_type_annotation_type_family_parameter  ← width=64  (synthesised)
ev_type_annotation_type_family_parameter  ← base=10   (synthesised)
ev_type_annotation_type_family_parameter  ← unit: BVN_UNIT_NO_PREFIX(bu_none)
ev_type_annotation_end
ev_data   data="1500"
    value_unit = { num_components=1,
      [0] bu_meter, exp_linear, {prefix_si, si_none} }
```

The `value_unit` field of `ev_data` always reflects the final, reconciled unit.

### 13.4 Practical callback

```c
bool my_verified_handler(void* userdata, bvnr_event_t ev, bvnr_data_t* d)
{
    if (ev != ev_type_annotation_type_family_parameter)
        return true;

    value_unit_t u = d->value_unit;
    if (u.num_components == 0 ||
        (u.num_components == 1 && u.components[0].base == bu_none))
        return true;

    if (bvn_unit_is_currency(u.components[0].base)) {
        const bvn_currency_info_t *ci =
            bvn_currency_info(u.components[0].base);
        printf("currency: %s  minor_unit=%u\n",
               ci->code, ci->minor_unit);
        return true;
    }

    char unit_str[128];
    bvn_unit_to_string(u, unit_str, sizeof(unit_str));
    printf("unit: %s  (prefix_factor: %g)\n",
           unit_str, bvn_unit_prefix_factor(u));
    return true;
}
```

---

## 14. Validation Errors

The validator raises the following unit-specific errors:

| Error code | Value | Trigger condition |
|------------|-------|-------------------|
| `error_unit_illegal` | 32 | Unparseable unit string: unknown prefix, unknown base unit, unknown currency code after `$`, a bare token in neither the physical-unit table nor (lacking the `$` sigil) recognised as a currency (e.g. `XYZ`, or bare `USD`), invalid prefix–unit combination (e.g. IEC prefix on a currency, sub-kilo SI prefix on byte), empty component between separators (e.g. `m//s`), or more than `BVNR_MAX_UNIT_COMPONENTS` (32) components |
| `error_unit_too_long` | 22 | An **inline** unit suffix exceeds its 255-byte lexer buffer. A unit written in an *annotation* reaches the type-annotation body's own cap first and raises `error_type_too_long` (21) instead — see §8 |
| `error_unit_mismatch` | 38 | An inline unit suffix and an explicit type-annotation unit are both present, but parse to different `value_unit_t` representations |
| `error_unexpected_input_byte` | 15 | An inline unit suffix appears inside an array element |

All four errors are raised during the `on_unverified` → validator phase. In `continue_on_error` mode the parser invokes `on_error` and enters the resync state machine, which resumes at the next `;` at the current nesting depth or at the start of the next assignment, whichever comes first (specification §13.2). `bvnr_reader_get_error`, `…_line`, `…_column`, `…_byte`, and `…_offset` all report the location of the offending token.

---

## 15. Annotated Examples

### 15.1 Physical Quantities

```bovnar
# Thermodynamic temperature
.ambient_temp  = <float:64,K>          293.15;

# Temperature in Celsius (affine: K = °C + 273.15)
.room_temp     = <float:32,°C>          20.0;

# Velocity and acceleration
.wind_speed    = <float:64,m/s>         12.5;
.gravity       = <float:64,m/s²>         9.80665;
.gravity_asc   = <float:64,m/s^2>        9.80665;  # ASCII caret — identical

# Pressure and energy
.tire_pressure = <float:32,k~Pa>       250.0;
.heat_energy   = <float:64,k~J>       5400.0;

# Flow rate (liters per minute)
.pump_flow     = <float:32,L/min>       15.0;

# Angles
.bearing       = <float:64,°>          270.0;
.phase         = <float:64,rad>          1.5708;

# Volume — US culinary
.recipe_water  = <float_dec:32,cup>      2.0;   # "cup" (lowercase) = US cup
.recipe_flour  = <float_dec:32,tbsp>     3.0;

# Duration
.shelf_life    = <uint:32,wk>           52;
.service_life  = <float:64,yr>          10.0;
```

### 15.2 Digital Storage

```bovnar
.packet_size = <uint:32,B>      1500;
.cache_size  = <uint:64,Ki~B>    512;
.ram_size    = <uint:64,Mi~B>   4096;
.disk_size   = <uint:64,Gi~B>    500;
.link_rate   = <uint:32,M~b>    1000;
.nic_speed   = <float:64,G~b/s>   10.0;
```

### 15.3 Compound SI Quantities

```bovnar
# Force: Newton = k~g·m/s²
.force          = <float:64,k~g·m/s²>    9.81;
.force_alt      = <float:64,k~g·m·s⁻²>  9.81;   # identical internal form

# Energy: Joule = k~g·m²/s²
.kinetic_energy = <float:64,k~g·m²/s²> 1000.0;

# Electric field
.field_strength = <float:64,V/m>         150.0;

# Torque
.torque         = <float:64,N·m>          25.0;
```

### 15.4 Currency Amounts and Rates

```bovnar
# ── Fiat scalar amounts ────────────────────────────────────────────────────
.price_usd     = <float_dec:64,$USD>   19.99;
.balance_eur   = <float_dec:64,$EUR>  342.00;
.yen_fee       = <uint:64,$JPY>           500;    # zero minor unit — integer only
.kwd_invoice   = <uint:64,$KWD>          3500;    # 3.500 KWD in fils

# "CUP" (uppercase) is the Cuban Peso, NOT the US cup volume unit:
.cup_balance   = <float_dec:64,$CUP>    25.00;   # 25 Cuban Pesos

# ── Crypto scalar amounts ──────────────────────────────────────────────────
.btc_sat       = <uint:64,$BTC>   54782000;       # on-chain satoshis
.eth_readable  = <float_dec:64,$ETH>    2.5;
.doge_bag      = <float_dec:64,$DOGE> 42000.0;
.usdt_stable   = <float_dec:64,$USDT>  5000.00;

# ── Compound units ─────────────────────────────────────────────────────────
.gold_price    = <float_dec:64,$USD/oz_t>   2351.40;  # $/troy oz
.wheat         = <float_dec:64,$USD/bsh>       5.82;  # $/bushel
.rent          = <float_dec:64,$EUR/m²>       12.50;  # €/m²
.billing_rate  = <float_dec:64,$EUR/h>        150.00; # €/h
.eur_usd       = <float_dec:64,$USD/$EUR>       1.0842; # exchange rate

# ── Reporting scale ────────────────────────────────────────────────────────
.fund_nav      = <float_dec:64,k~$USD>    250.0;     # $250,000
.gdp           = <float_dec:64,M~$EUR> 42800.0;      # €42.8 billion

# ── Exchange rate with timestamp ───────────────────────────────────────────
.snapshot = {
    .epoch    = <uint:64,s>              1716400000;
    .eur_usd  = <float_dec:64,$USD/$EUR>        1.0842;
};

# ── Array of prices ────────────────────────────────────────────────────────
.tier_prices   = <float_dec:64,$USD> [9.99, 19.99, 49.99, 99.99];
```

### 15.5 Error Cases

<!-- bovnar-example: rejected -->
```bovnar
# Empty component → error_unit_illegal
.bad1 = <float:64,m//s>      1.0;

# Too many components (33 > 32) → error_unit_illegal
.bad2 = <float:64,m*s*g*A*K*mol*cd*b*V*Hz*N*Pa*J*W*Ω*F*C*S*Wb*T*H*lm*lx*Bq*Gy*kat*L*min*h*d*bar*eV*Da> 1.0;

# Unknown base unit → error_unit_illegal
.bad3 = <float:64,foobar>    1.0;

# IEC prefix on a currency → error_unit_illegal
.bad4 = <float_dec:64,Ki~$USD> 1.0;

# Annotation unit differs from inline unit → error_unit_mismatch
.bad5 = <float:64,m> 1.0 s;

# Inline unit inside an array → error_unexpected_input_byte
.bad6 = <float:64,m> [1.0 m, 2.0 m];   # ERROR: suffix inside array

# Correct: dimensionless explicit
.ok1  = <uint:32,no_unit>    42;

# Correct: omitted unit (same behaviour as no_unit)
.ok2  = <uint:32>            42;

# Correct: BTU is a valid alias for bu_btu (currency lookup returns 0, physical table matches)
.ok3  = <float:64,BTU>      1.0;    # valid: same as Btu or btu

# Correct: sub-kilo SI prefix on currency is accepted
.ok4  = <float_dec:64,m~$USD> 0.001; # valid: milli-dollar (one tenth of a cent)

# Correct: cup (volume) vs CUP (currency) — both valid, different meaning
.vol  = <float_dec:32,cup>  2.0;    # US cup (236.6 mL)
.bal  = <float_dec:64,$CUP> 25.00;   # Cuban Peso
```

---

## See also

- [Specification §11 — Units System](03_bovnar_spec.md#11-units-system) — how a unit is attached to a value
- [Unit & Currency Cheat Sheet](04_bovnar_unit_cheatsheet.md) — every symbol in this registry, in table form
- [Unit Ambiguities](07_bovnar_unit_ambiguities.md) — every token that could plausibly mean two things
- [Unit Profiles](11_bovnar_unit_profiles.md) — writing UCUM, UN/ECE, QUDT, UDUNITS, OM 2 and CF codes in a unit slot, and where the vocabularies disagree
- [Read & Write API](08_bovnar_readwrite_api.md) — `bvn_parse_unit`, `bvn_unit_to_string`, and read-time conversion
- [Python Bindings](09_bovnar_python_bindings.md) — the same unit model from Python, with the NumPy and pint bridges

---

*End of Bovnar — Unit & Currency Reference (Bovnar spec 1.2).*
