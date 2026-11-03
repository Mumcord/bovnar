# Bovnar — Units & Currencies Cheat Sheet

> **Spec version:** 1.2
> **Status:** Reference — the symbol tables of the unit and currency registry
> **Scope:** 264 physical units, 166 fiat currencies, 50 cryptocurrencies, and every SI/IEC prefix.

---

## Table of Contents

1. [SI Prefixes](#1-si-prefixes)
2. [IEC Binary Prefixes](#2-iec-binary-prefixes)
3. [Prefix Validity Rules](#3-prefix-validity-rules)
4. [Physical Units](#4-physical-units)
    - 4.1 [SI Base Units](#41-si-base-units)
    - 4.2 [Named SI-Derived Units](#42-named-si-derived-units)
    - 4.3 [Non-SI Units Accepted with SI](#43-non-si-units-accepted-with-si)
    - 4.4 [Imperial & US Customary — Length](#44-imperial--us-customary--length)
    - 4.5 [Imperial & US Customary — Mass](#45-imperial--us-customary--mass)
    - 4.6 [Temperature](#46-temperature)
    - 4.7 [Pressure](#47-pressure)
    - 4.8 [Energy](#48-energy)
    - 4.9 [Power](#49-power)
    - 4.10 [Force](#410-force)
    - 4.11 [Speed & Rotation](#411-speed--rotation)
    - 4.12 [Acceleration](#412-acceleration)
    - 4.13 [Volume — US Liquid](#413-volume--us-liquid)
    - 4.14 [Volume — UK Imperial](#414-volume--uk-imperial)
    - 4.15 [Volume — US Apothecary & Dry](#415-volume--us-apothecary--dry)
    - 4.16 [Area](#416-area)
    - 4.17 [Angle](#417-angle)
    - 4.18 [Digital](#418-digital)
    - 4.19 [CGS Units](#419-cgs-units)
    - 4.20 [Radiation](#420-radiation)
    - 4.21 [Logarithmic](#421-logarithmic)
    - 4.22 [Electrical Power](#422-electrical-power)
    - 4.23 [Textile Linear Density](#423-textile-linear-density)
    - 4.24 [Old German Units](#424-old-german-units)
    - 4.25 [Additional Physical Units](#425-additional-physical-units)
    - 4.26 [Ratio and Proportion](#426-ratio-and-proportion)
    - 4.27 [Named Speeds & Acidity](#427-named-speeds--acidity)
    - 4.28 [Water Hardness](#428-water-hardness)
    - 4.29 [Water-Quality Instrument Scales](#429-water-quality-instrument-scales)
    - 4.30 [Survey, Typographic, Dry-Volume and Trade Units](#430-survey-typographic-dry-volume-and-trade-units)
5. [Currencies](#5-currencies)
    - 5.1 [The Mandatory Currency Sigil](#51-the-mandatory-currency-sigil)
    - 5.2 [ISO 4217 Fiat Currencies](#52-iso-4217-fiat-currencies)
    - 5.3 [Cryptocurrencies](#53-cryptocurrencies)
    - 5.4 [Currency Prefix Rules](#54-currency-prefix-rules)
6. [Symbol Disambiguation](#6-symbol-disambiguation)

- [See also](#see-also)

---

## 1. SI Prefixes

Written as `prefix~base`, or compactly without the separator: `k~m` and `km` are both the kilometre. The canonical output form always keeps the `~`.

| Name   | Symbol | Factor | Enum (`si_prefix_id_t`) |
|--------|--------|--------|--------------------------|
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

> `µ` is U+00B5 MICRO SIGN (UTF-8 `0xC2 0xB5`). U+03BC (Greek small letter mu) is also accepted on input; the canonical output is always U+00B5. ASCII `u` is accepted as an input-only alias for `µ` (e.g. `u~m` = `µ~m`); the canonical output is always `µ`.
> `da` is a two-character prefix: `da~m` = decametre.

**Prefix–base ambiguities** — the base unit is the longest alias suffix, so a bare token is always the unit; `~` selects the prefix reading:

| Bare token | Is a base unit | With `~` becomes prefix |
|------------|----------------|--------------------------|
| `m`  | meter (`bu_meter`)    | milli  |
| `d`  | day (`bu_day`)        | deci   |
| `h`  | hour (`bu_hour`)      | hecto  |
| `T`  | tesla (`bu_tesla`)    | tera   |
| `G`  | gauss (`bu_gauss`)    | giga   |
| `P`  | poise (`bu_poise`)    | peta   |
| `R`  | röntgen (`bu_roentgen`) | ronna |
| `f`  | *(nothing — `error_unit_illegal`; the farad is `F`, uppercase)* | femto  |
| `u`  | dalton (`bu_dalton`)  | micro (ASCII alias for `µ`) |
| `S`  | siemens (`bu_siemens`) | *(not a prefix — `S` has no prefix role)* |

Examples: bare `m` = metre; `m~s` and `ms` = millisecond. Bare `d` = day; `d~s` and `ds` = decisecond. Bare `min` = minute, so milli-inch needs the separator: `m~in`.

A compact spelling is refused when the token means something else in the wild: `usb`, `kt` and `ppt` are `error_unit_illegal` (write `u~sb`; for `kt`, `k~t` for the kilotonne or `kn` for the knot; for `ppt`, `pptr`, `‰` or `p~pt`). `pH`, `mph` and `kph` are units in their own right (§3.26–3.27 of the unit-system reference), so they parse as acidity and the two speeds — `p~H`, `m~ph` and `k~ph` are still the picohenry and the milli-/kilophot.

Currencies take the compact prefix too (`k$EUR` = `k~$EUR`, §5.4), but their `$` sigil is never optional.

---

## 2. IEC Binary Prefixes

Used **only** on `b` (bit) and `B` (byte). Written as `prefix~base` or compactly: `Ki~B` and `KiB` are both the kibibyte.

| Name  | Symbol | Factor   | Enum (`iec_prefix_id_t`) |
|-------|--------|----------|--------------------------|
| kibi  | `Ki`   | 2¹⁰      | `iec_kibi`  |
| mebi  | `Mi`   | 2²⁰      | `iec_mebi`  |
| gibi  | `Gi`   | 2³⁰      | `iec_gibi`  |
| tebi  | `Ti`   | 2⁴⁰      | `iec_tebi`  |
| pebi  | `Pi`   | 2⁵⁰      | `iec_pebi`  |
| exbi  | `Ei`   | 2⁶⁰      | `iec_exbi`  |
| zebi  | `Zi`   | 2⁷⁰      | `iec_zebi`  |
| yobi  | `Yi`   | 2⁸⁰      | `iec_yobi`  |
| robi †| `Ri`   | 2⁹⁰      | `iec_robi`  |
| quebi †| `Qi`   | 2¹⁰⁰     | `iec_quebi` |

† `Ri`/`Qi` are a forward-looking extension: IEC 80000-13 stops at yobi (`Yi`, 2⁸⁰).

---

## 3. Prefix Validity Rules

| Unit category | SI prefixes | IEC prefixes |
|---------------|-------------|--------------|
| All physical units (default) | All 24 allowed | Forbidden |
| `b` (bit) and `B` (byte) | Only ≥ kilo (`k`, `M`, `G`, …, `Q`) | All 10 allowed |
| `b` and `B` with sub-kilo SI (`d`, `c`, `m`, `µ`, `n`, `p`, `f`, `a`, `z`, `y`, `r`, `q`, `da`, `h`) | **Forbidden** | — |
| Currency codes | All 24 allowed | **Forbidden** |
| Old German units (`bu_pfund` … `bu_scheffel`) | **None** (`si_none` only) | **Forbidden** |

---

## 4. Physical Units

> **Symbol** = canonical serialized form (produced on output; accepted on input).
> **Long forms** = accepted on input only; never produced on output.
> **Factor** = conversion factor to SI base units unless noted.

### 4.1 SI Base Units

| Symbol | Long forms | Name | Enum |
|--------|-----------|------|------|
| `s`   | `sec`, `second`, `seconds` | second | `bu_second` |
| `m`   | `meter`, `metre`, `meters`, `metres` | metre | `bu_meter` |
| `g`   | `gram`, `grams` | gram | `bu_gram` |
| `A`   | `amp`, `amps`, `ampere`, `amperes` | ampere | `bu_ampere` |
| `K`   | `kelvin`, `kelvins` | kelvin | `bu_kelvin` |
| `mol` | `mole`, `moles` | mole | `bu_mol` |
| `cd`  | `candela`, `candelas` | candela | `bu_candela` |

> `g` (gram) is the base symbol; `k~g` = kilogram.

### 4.2 Named SI-Derived Units

| Symbol | Long forms | Name | Enum | SI definition |
|--------|-----------|------|------|---------------|
| `Hz`  | `hertz` | hertz | `bu_hertz` | s⁻¹ |
| `N`   | `newton`, `newtons` | newton | `bu_newton` | kg·m·s⁻² |
| `Pa`  | `pascal`, `pascals` | pascal | `bu_pascal` | kg·m⁻¹·s⁻² |
| `J`   | `joule`, `joules` | joule | `bu_joule` | kg·m²·s⁻² |
| `W`   | `watt`, `watts` | watt | `bu_watt` | kg·m²·s⁻³ |
| `V`   | `volt`, `volts` | volt | `bu_volt` | kg·m²·A⁻¹·s⁻³ |
| `Ω`   | `ohm`, `ohms`, `Ohm` | ohm | `bu_ohm` | kg·m²·A⁻²·s⁻³ — U+2126 OHM SIGN; U+03A9 (Greek capital omega) also accepted on input, canonical output is always U+2126 |
| `F`   | `farad`, `farads` | farad | `bu_farad` | kg⁻¹·m⁻²·A²·s⁴ |
| `C`   | `coulomb`, `coulombs` | coulomb | `bu_coulomb` | A·s |
| `S`   | `siemens`, `mho`, `mhos`, `℧` | siemens | `bu_siemens` | kg⁻¹·m⁻²·A²·s³ |
| `Wb`  | `weber`, `webers` | weber | `bu_weber` | kg·m²·A⁻¹·s⁻² |
| `T`   | `tesla`, `teslas` | tesla | `bu_tesla` | kg·A⁻¹·s⁻² |
| `H`   | `henry`, `henrys`, `henries` | henry | `bu_henry` | kg·m²·A⁻²·s⁻² |
| `lm`  | `lumen`, `lumens` | lumen | `bu_lumen` | cd·sr |
| `lx`  | `lux` | lux | `bu_lux` | cd·sr·m⁻² |
| `Bq`  | `becquerel`, `becquerels` | becquerel | `bu_becquerel` | s⁻¹ |
| `Gy`  | `gray`, `grays` | gray | `bu_gray` | m²·s⁻² |
| `Sv`  | `sievert`, `sieverts` | sievert | `bu_sievert` | m²·s⁻² |
| `kat` | `katal`, `katals` | katal | `bu_katal` | mol·s⁻¹ |
| `rad` | `radian`, `radians` | radian | `bu_radian` | dimensionless (m/m) |
| `sr`  | `steradian`, `steradians` | steradian | `bu_steradian` | dimensionless (m²/m²) |

### 4.3 Non-SI Units Accepted with SI

| Symbol | Long forms | Name | Enum | Factor / notes |
|--------|-----------|------|------|----------------|
| `L`, `l` | `liter`, `litre`, `liters`, `litres` | litre | `bu_liter` | 10⁻³ m³ |
| `min` | `minute`, `minutes` | minute | `bu_minute` | 60 s |
| `h`   | `hour`, `hours` | hour | `bu_hour` | 3600 s |
| `d`   | `day`, `days` | day | `bu_day` | 86400 s |
| `wk`  | `week`, `weeks` | week | `bu_week` | 604 800 s |
| `mo`  | `month`, `months` | month (Julian) | `bu_month` | 2 629 800 s (= 365.25 d / 12) |
| `fn`  | `fortnight`, `fortnights` | fortnight | `bu_fortnight` | 1 209 600 s (= 14 d) |
| `yr`  | `year`, `years` | year (Julian) | `bu_year` | 31 557 600 s |
| `°`, `deg` | `degr`, `degree`, `degrees` | degree (angle) | `bu_degree` | π/180 rad — U+00B0 |
| `t`   | `tonne`, `tonnes` | tonne | `bu_tonne` | 10³ kg |
| `bar` | `bars` | bar | `bu_bar` | 10⁵ Pa |
| `eV`  | `electronvolt`, `electronvolts` | electronvolt | `bu_electronvolt` | 1.602176634×10⁻¹⁹ J |
| `Da`  | `dalton`, `daltons`, `amu`, `u` | dalton | `bu_dalton` | 1.66053906892×10⁻²⁷ kg (CODATA 2022) |
| `au`  | — | astronomical unit | `bu_astronomical_unit` | 1.495978707×10¹¹ m |
| `ha`  | `hectare`, `hectares` | hectare | `bu_hectare` | 10⁴ m² |

### 4.4 Imperial & US Customary — Length

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `in`   | `inch`, `inches` | inch | `bu_inch` | 0.0254 m (exact) |
| `ft`   | `foot`, `feet` | foot | `bu_foot` | 0.3048 m (exact) |
| `yd`   | `yard`, `yards` | yard | `bu_yard` | 0.9144 m (exact) |
| `mi`   | `mile`, `miles` | statute mile | `bu_mile` | 1609.344 m (exact) |
| `nmi`  | `nautical_mile`, `nautical_miles` | nautical mile | `bu_nautical_mile` | 1852 m (exact) |
| `Å` (U+212B) | `angstrom`, `angstroms`, Å (U+00C5) | ångström | `bu_angstrom` | 10⁻¹⁰ m |
| `ly`   | `light_year`, `light_years` | light-year | `bu_light_year` | 9.4607304725808×10¹⁵ m |
| `pc`   | `parsec`, `parsecs` | parsec | `bu_parsec` | 3.085677581491367×10¹⁶ m |
| `fur`  | `furlong`, `furlongs` | furlong | `bu_furlong` | 201.168 m (exact) |
| `fath` | `fathom`, `fathoms` | fathom | `bu_fathom` | 1.8288 m (exact) |
| `thou` | `thou`, `mil`, `mils` | thou | `bu_thou` | 25.4×10⁻⁶ m (exact) |
| `ch`   | `chain`, `chains` | chain (Gunter's) | `bu_chain` | 20.1168 m (exact) |
| `rd`   | `rod`, `rods` | rod (pole, perch) | `bu_rod` | 5.0292 m (exact) |

> `thou` and `mil` are synonyms. Canonical output is `thou`. `mil` does **not** mean milliradian; milliradians are written `m~rad`.

### 4.5 Imperial & US Customary — Mass

| Symbol  | Long forms | Name | Enum | Factor |
|---------|-----------|------|------|--------|
| `lb`    | `lbs`, `pound`, `pounds` | pound (avoirdupois) | `bu_pound` | 0.45359237 kg (exact) |
| `oz`    | `ounce`, `ounces` | ounce (avoirdupois) | `bu_ounce` | 0.028349523125 kg (exact) |
| `gr`    | `grain`, `grains` | grain | `bu_grain` | 6.479891×10⁻⁵ kg (exact) |
| `st`    | `stone`, `stones` | stone | `bu_stone` | 6.35029318 kg (exact) |
| `tn_sh` | `short_ton`, `short_tons` | short ton (US) | `bu_short_ton` | 907.18474 kg (exact) |
| `tn_l`  | `long_ton`, `long_tons` | long ton (UK) | `bu_long_ton` | 1016.0469088 kg (exact) |
| `oz_t`  | `troy_ounce`, `troy_ounces` | troy ounce | `bu_troy_ounce` | 0.0311034768 kg (exact) |
| `ct`    | `carat`, `carats` | metric carat | `bu_carat` | 2×10⁻⁴ kg (exact) |
| `slug`  | `slugs` | slug | `bu_slug` | 14.593902937206364 kg (= `lb`·`gn`/`ft`) |
| `dr`    | `dram`, `drams` | dram (avoirdupois) | `bu_dram` | 1.7718451953125×10⁻³ kg (exact) |
| `dwt`   | `pennyweight`, `pennyweights` | pennyweight (troy) | `bu_pennyweight` | 1.55517384×10⁻³ kg (exact) |
| `lb_t`  | `troy_pound`, `troy_pounds`, `apothecary_pound` | troy pound (= apothecary pound) | `bu_troy_pound` | 0.3732417216 kg (exact, = 12 `oz_t`) |
| `dr_ap` | `apothecary_dram`, `apothecary_drams` | dram (apothecary) | `bu_apothecary_dram` | 3.8879346×10⁻³ kg (exact, = 3 `sc`) |
| `cwt_l` | `long_hundredweight`, `long_hundredweights` | hundredweight (long/imperial) | `bu_long_hundredweight` | 50.80234544 kg (exact, = 112 `lb`) |

> `dr_ap` is **2.2× the avoirdupois `dr`**, and the short hundredweight is exactly `h~lb`, so it has no unit of its own.

### 4.6 Temperature

| Symbol | Long forms | Name | Enum | Conversion |
|--------|-----------|------|------|------------|
| `°C`, `degC` | `degrC`, `degreeC`, `degreesC`, `celsius` | degree Celsius | `bu_celsius` | K = °C + 273.15 **(affine)** |
| `°F`, `degF` | `degrF`, `degreeF`, `degreesF`, `fahrenheit` | degree Fahrenheit | `bu_fahrenheit` | K = (°F + 459.67) × 5/9 **(affine)** |
| `°Ra`, `degRa`, `Ra` | `degrRa`, `degreeRa`, `degreesRa`, `rankine` | degree Rankine | `bu_rankine` | K = °Ra × 5/9 (linear) |
| `°De`, `degDe` | `degrDe`, `degreeDe`, `degreesDe`, `delisle` | degree Delisle | `bu_delisle` | K = 373.15 − °De × 2/3 **(affine)** |
| `°N`, `degN` | `degrN`, `degreeN`, `degreesN`, `newton_temperature` | degree Newton | `bu_newton_temp` | K = °N × 100/33 + 273.15 **(affine)** |
| `°Re`, `degRe` | `degrRe`, `degreeRe`, `degreesRe`, `reaumur` | degree Réaumur | `bu_reaumur` | K = °Re × 5/4 + 273.15 **(affine)** |
| `°Ro`, `degRo` | `degrRo`, `degreeRo`, `degreesRo`, `romer` | degree Rømer | `bu_romer` | K = (°Ro − 7.5) × 40/21 + 273.15 **(affine)** |

> Kelvin (`K`) is in §4.1. `R` is reserved for röntgen (§4.20). `N` alone is newton (§4.2); use `°N` or `degN` for Newton temperature.

**Temperature differences.** Every scale above is a *scale*: `25 °C` is 298.15 K. A *difference* of 25 degrees is 25 K, and these are the units for it. Each is a ratio scale (no offset) carrying the temperature-interval quantity kind, so `ΔK` never converts to `K` and `°C` never converts to `Δ°C` — those are `error_unit_mismatch`.

| Symbol | Long forms | Name | Enum | Conversion |
|--------|-----------|------|------|------------|
| `ΔK`, `delta_K` | `deltaK`, `delta_kelvin`, `deltakelvin`, `Δ°C`, `delta_degC`, `deltadegC`, `delta_celsius`, `deltacelsius` | kelvin interval | `bu_delta_kelvin` | 1 K exactly. `Δ°C` **is** this unit — the Celsius interval is the kelvin |
| `Δ°F`, `delta_degF` | `deltadegF`, `delta_fahrenheit`, `deltafahrenheit`, `Δ°Ra`, `delta_degRa`, `deltadegRa`, `delta_rankine`, `deltarankine` | Fahrenheit interval | `bu_delta_fahrenheit` | 5/9 K exactly. `Δ°Ra` **is** this unit |
| `Δ°De`, `delta_degDe` | `deltadegDe`, `delta_delisle`, `deltadelisle` | Delisle interval | `bu_delta_delisle` | −2/3 K exactly (Delisle runs backwards) |
| `Δ°N`, `delta_degN` | `deltadegN`, `delta_newton_temperature` | Newton interval | `bu_delta_newton_temp` | 100/33 K exactly |
| `Δ°Re`, `delta_degRe` | `deltadegRe`, `delta_reaumur`, `deltareaumur` | Réaumur interval | `bu_delta_reaumur` | 5/4 K exactly |
| `Δ°Ro`, `delta_degRo` | `deltadegRo`, `delta_romer`, `deltaromer` | Rømer interval | `bu_delta_romer` | 40/21 K exactly |

> `Δ` is U+0394; every unit here has ASCII spellings. There is deliberately no bare `delta_C` or `delta_F` — those read as a delta coulomb and a delta farad. Inside a **compound** the distinction does not arise and is not made: `W/(m²·ΔK)` and `W/(m²·K)` are the same unit, because an affine scale cannot appear in a compound at all, so the `K` there was already an interval.

### 4.7 Pressure

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `atm`  | `atmosphere`, `atmospheres` | standard atmosphere | `bu_atmosphere` | 101 325 Pa (exact) |
| `at`   | `atmosphere_technical` | atmosphere technical | `bu_atmosphere_technical` | 98 066.5 Pa (= 1 kgf/cm²) |
| `mmHg` | — | millimetre of mercury | `bu_mmhg` | 133.322387415 Pa |
| `Torr` | `torr` | torr | `bu_torr` | 101 325/760 Pa |
| `psi`  | — | pound-force per square inch | `bu_psi` | 6894.757293168362 Pa |
| `inHg` | `inch_hg`, `inch_mercury` | inch of mercury | `bu_inch_hg` | 3386.388640341 Pa (= 25.4 mmHg exactly) |
| `mH2O` | `metre_water`, `meter_water` | metre of water column | `bu_meter_water` | 9806.65 Pa (exact, conventional) |

> `at ≠ atm`: 1 at = 98 066.5 Pa; 1 atm = 101 325 Pa.

### 4.8 Energy

| Symbol  | Long forms | Name | Enum | Factor |
|---------|-----------|------|------|--------|
| `cal`   | `calorie`, `calories` | thermochemical calorie | `bu_calorie` | 4.184 J (exact) |
| `Btu`   | `btu`, `BTU` | International Table BTU | `bu_btu` | 1055.05585262 J |
| `erg`   | `ergs` | erg | `bu_erg` | 10⁻⁷ J (exact) |
| `thm`   | `therm`, `therms` | US therm | `bu_therm` | 1.05480400×10⁸ J (exact) |
| `ft_lb` | `foot_pound`, `foot_pounds` | foot-pound | `bu_foot_pound` | 1.3558179483314003 J (= `lbf`·`ft`) |
| `cal_IT` | `calorie_IT` | International Table calorie | `bu_calorie_it` | 4.1868 J (exact) |
| `Btu_th` | `BTU_th`, `btu_th` | thermochemical BTU | `bu_btu_th` | 23 722 880 951/22 500 000 J ≈ 1054.35026449 J |

> `BTU` (all-caps) is a valid alias: with no `$` sigil the token is a physical-unit lookup (the currency table is only consulted for `$`-prefixed tokens), and it matches `bu_btu`. `Btu` and `btu` are also accepted.

### 4.9 Power

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `hp`   | `horsepower` | mechanical horsepower | `bu_horsepower` | 745.6998715822702 W (= 550 `ft_lb`/s) |
| `PS`   | `CV`, `metric_horsepower` | metric horsepower | `bu_metric_horsepower` | 735.49875 W (exact) |

### 4.10 Force

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `lbf`  | `pound_force` | pound-force | `bu_pound_force` | 4.4482216152605 N |
| `dyn`  | `dyne`, `dynes` | dyne | `bu_dyne` | 10⁻⁵ N (exact) |
| `kip`  | `kips` | kip (kilopound-force) | `bu_kip` | 4448.2216152605 N |
| `kgf`  | `kilogram_force` | kilogram-force | `bu_kilogram_force` | 9.80665 N (exact) |

### 4.11 Speed & Rotation

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `kn`   | `knot`, `knots` | knot | `bu_knot` | 1852/3600 m/s |
| `rpm`  | — | revolutions per minute | `bu_rpm` | 1/60 s⁻¹ |

### 4.12 Acceleration

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `gn`   | `standard_gravity` | standard gravity | `bu_standard_gravity` | 9.80665 m·s⁻² (exact) |

### 4.13 Volume — US Liquid

| Symbol  | Long forms | Name | Enum | Factor |
|---------|-----------|------|------|--------|
| `gal`   | `gallon`, `gallons` | US liquid gallon | `bu_gallon` | 3.785411784×10⁻³ m³ (exact) |
| `qt`    | `quart`, `quarts` | US liquid quart | `bu_quart` | 9.46352946×10⁻⁴ m³ |
| `pt`    | `pint`, `pints` | US liquid pint | `bu_pint` | 4.73176473×10⁻⁴ m³ |
| `cup`   | `cups` | US cup | `bu_cup` | 2.365882365×10⁻⁴ m³ |
| `gi`    | `gill`, `gills` | US gill | `bu_gill` | 1.18294118250×10⁻⁴ m³ |
| `fl_oz` | `fluid_ounce`, `fluid_ounces` | US fluid ounce | `bu_fluid_ounce` | 2.95735295625×10⁻⁵ m³ |
| `tbsp`  | `tablespoon`, `tablespoons` | US tablespoon | `bu_tablespoon` | 1.478676478125×10⁻⁵ m³ (exact) |
| `tsp`   | `teaspoon`, `teaspoons` | US teaspoon | `bu_teaspoon` | 4.92892159375×10⁻⁶ m³ |
| `bbl`   | `barrel`, `barrels` | petroleum barrel | `bu_barrel` | 0.158987294928 m³ |

> `cup` (lowercase) = US cup; `CUP` (uppercase) = Cuban Peso. See §6.

### 4.14 Volume — UK Imperial

| Symbol     | Long forms | Name | Enum | Factor |
|------------|-----------|------|------|--------|
| `gal_uk`   | `gallon_uk`, `gallons_uk` | imperial gallon | `bu_gallon_uk` | 4.54609×10⁻³ m³ (exact) |
| `qt_uk`    | `quart_uk`, `quarts_uk` | imperial quart | `bu_quart_uk` | 1136.5225×10⁻⁶ m³ |
| `pt_uk`    | `pint_uk`, `pints_uk` | imperial pint | `bu_pint_uk` | 568.26125×10⁻⁶ m³ |
| `gi_uk`    | `gill_uk`, `gills_uk` | imperial gill | `bu_gill_uk` | 1.420653125×10⁻⁴ m³ (exact) |
| `fl_oz_uk` | `fluid_ounce_uk`, `fluid_ounces_uk` | imperial fluid ounce | `bu_fluid_ounce_uk` | 28.4130625×10⁻⁶ m³ |

### 4.15 Volume — US Apothecary & Dry

| Symbol  | Long forms | Name | Enum | Factor |
|---------|-----------|------|------|--------|
| `fl_dr`, `fl_drams` | `fluid_dram`, `fluid_drams` | US fluid dram | `bu_fluid_dram` | 3.6966911953125×10⁻⁶ m³ |
| `minim` | `minims` | US minim | `bu_minim` | 6.16115199218750×10⁻⁸ m³ |
| `pk`    | `peck`, `pecks` | US dry peck | `bu_peck` | 8.80976754172×10⁻³ m³ |
| `bsh`   | `bushel`, `bushels` | US bushel | `bu_bushel` | 3.523907016688×10⁻² m³ |

> `minim` not `min` (which is the minute).

### 4.16 Area

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `ac`   | `acre`, `acres` | acre | `bu_acre` | 4046.8564224 m² (exact) |
| `barn` | `barns` | barn | `bu_barn` | 10⁻²⁸ m² (exact) |

### 4.17 Angle

| Symbol   | Long forms | Name | Enum | Factor |
|----------|-----------|------|------|--------|
| `arcmin` | `arcminute`, `arcminutes` | arcminute | `bu_arcminute` | π/10800 rad |
| `arcsec` | `arcsecond`, `arcseconds` | arcsecond | `bu_arcsecond` | π/648000 rad |
| `grad`   | `gradian`, `gradians`, `gon` | gradian | `bu_grad` | π/200 rad |
| `rev`    | `turn`, `revolution`, `revolutions`, `turns` | revolution | `bu_revolution` | 2π rad |

### 4.18 Digital

| Symbol | Long forms | Name | Enum |
|--------|-----------|------|------|
| `b`    | `bit`, `bits` | bit | `bu_bit` |
| `B`    | `byte`, `bytes`, `Byte`, `Bytes` | byte | `bu_byte` |

> `b` and `B` are separate quantity kinds: `b` → `B` is **refused**, not
> divided by eight. Prefixes still convert within each (`Ki~B` → `B` is 1024).
> See doc/05 §3.17.

### 4.19 CGS Units

| Symbol | Long forms | Name | Enum | SI equivalent |
|--------|-----------|------|------|---------------|
| `P`    | `poise`, `poises` | poise (dynamic viscosity) | `bu_poise` | 0.1 Pa·s |
| `St`   | `stokes`, `stoke` | stokes (kinematic viscosity) | `bu_stokes` | 10⁻⁴ m²·s⁻¹ |
| `G`    | `gauss` | gauss (magnetic flux density) | `bu_gauss` | 10⁻⁴ T |
| `Mx`   | `maxwell`, `maxwells` | maxwell (magnetic flux) | `bu_maxwell` | 10⁻⁸ Wb |
| `Oe`   | `oersted`, `oersteds` | oersted (magnetic field strength) | `bu_oersted` | 1000/(4π) A/m |
| `sb`   | `stilb`, `stilbs` | stilb (luminance) | `bu_stilb` | 10⁴ cd/m² |
| `ph`   | `phot`, `phots` | phot (illuminance) | `bu_phot` | 10⁴ lx |
| `Gal`  | `galileo`, `galileos` | galileo (acceleration) | `bu_galileo` | 10⁻² m/s² |

### 4.20 Radiation

| Symbol | Long forms | Name | Enum | SI equivalent |
|--------|-----------|------|------|---------------|
| `Ci`   | `curie`, `curies` | curie (radioactivity) | `bu_curie` | 3.7×10¹⁰ Bq |
| `R`    | `roentgen`, `roentgens` | röntgen (radiation exposure) | `bu_roentgen` | 2.58×10⁻⁴ C/kg |
| `rem`  | `rems` | rem (dose equivalent) | `bu_rem` | 10⁻² Sv |

### 4.21 Logarithmic

| Symbol | Long forms | Name | Enum | Notes |
|--------|-----------|------|------|-------|
| `Np`   | `neper`, `nepers` | neper | `bu_neper` | dimensionless; does **not** convert to `dB` |
| `dB`   | `decibel`, `decibels` | decibel | `bu_decibel` | dimensionless; does **not** convert to `Np` |

> Relating two logarithmic scales is a change of base, not a multiplication, so
> the conversion engine cannot express it — and `dB` is written against both the
> power (10·log₁₀) and field (20·log₁₀) conventions without the annotation saying
> which. The library refuses the pair. See doc/05 §3.15.

### 4.22 Electrical Power

| Symbol | Long forms | Name | Enum | Notes |
|--------|-----------|------|------|-------|
| `var`  | `vars` | var (volt-ampere reactive) | `bu_var` | reactive power; same SI dim as W |
| `VA`   | `volt_ampere`, `volt_amperes` | volt-ampere | `bu_volt_ampere` | apparent power; same SI dim as W |

> `W`, `var`, and `VA` all have SI dimension kg·m²·s⁻³. `bvn_units_compatible` returns `true` across them; use `.components[0].base` to distinguish.

### 4.23 Textile Linear Density

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `tex`  | — | tex | `bu_tex` | 1×10⁻⁶ kg/m (ISO 1144) |
| `den`  | `denier`, `deniers` | denier | `bu_denier` | 1/9 000 000 kg/m |

> 9 den = 1 tex.

### 4.24 Old German Units

No Old German unit accepts any SI or IEC prefix (`bvn_prefix_unit_valid` rejects all non-`si_none` prefixes for `bu_pfund` … `bu_scheffel`). Enum values 100133–100145.

#### Mass (metric-compatible)

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `Pfd`  | `pfund`, `pfunds` | Pfund | `bu_pfund` | 0.5 kg (exact) |
| `Ztr`  | `zentner` | Zentner | `bu_zentner` | 50 kg (exact) |
| `dz`   | `doppelzentner` | Doppelzentner | `bu_doppelzentner` | 100 kg (exact) |
| `lot`  | `lots` | Lot | `bu_lot` | 15.625×10⁻³ kg (exact) |

#### Length (historical Prussian)

| Symbol    | Long forms | Name | Enum | Factor |
|-----------|-----------|------|------|--------|
| `prln`    | `prussian_line`, `linie` | Prussian line | `bu_prussian_line` | 313853/144000000 m ≈ 2.1795347×10⁻³ m |
| `prz`     | `prussian_zoll`, `zoll` | Prussian Zoll | `bu_prussian_zoll` | 313853/12000000 m ≈ 2.6154417×10⁻² m |
| `prf`     | `prussian_fuss`, `preussischer_fuss` | Prussian Fuß | `bu_prussian_fuss` | 3.13853×10⁻¹ m |
| `elle`    | `prussian_elle`, `preussische_elle` | Prussian Elle | `bu_prussian_elle` | 6.66937625×10⁻¹ m (exact) |
| `rute`    | `prussian_rute`, `preussische_rute` | Prussian Rute | `bu_prussian_rute` | 3.766236 m (exact) |
| `klafter` | `prussian_klafter` | Klafter | `bu_klafter` | 1.883118 m (exact) |
| `dt_mi`   | `deutsche_meile`, `german_mile` | Geographische Meile | `bu_german_mile` | 7420.44 m |

#### Area (historical Prussian)

| Symbol   | Long forms | Name | Enum | Factor |
|----------|-----------|------|------|--------|
| `morgen` | `prussian_morgen` | Morgen (Prussian) | `bu_morgen` | 2553.21604938528 m² (exact) |

#### Volume (historical Prussian)

| Symbol   | Long forms | Name | Enum | Factor |
|----------|-----------|------|------|--------|
| `schffl` | `scheffel`, `prussian_scheffel` | Scheffel (Prussian) | `bu_scheffel` | 54.961×10⁻³ m³ |

### 4.25 Additional Physical Units

#### Length

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `ftUS` | `survey_foot` | US survey foot | `bu_survey_foot` | 1200/3937 m ≈ 0.30480061 m |
| `lea`  | `league`, `leagues` | League (US statute, 3 mi) | `bu_league` | 4828.032 m |
| `cbl`  | `cable`, `cables` | Cable (international, ¹⁄₁₀ nmi) | `bu_cable` | 185.2 m |
| `hand` | `hands` | Hand (4 in) | `bu_hand` | 0.1016 m |

#### Mass

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `qntl` | `quintal`, `quintals` | Metric quintal | `bu_quintal` | 100 kg |
| `sc`   | `scruple`, `scruples` | Apothecary scruple (20 gr) | `bu_scruple` | 1.2959782×10⁻³ kg |

#### Signal Rate

| Symbol | Long forms | Name | Enum | SI dimension |
|--------|-----------|------|------|--------------|
| `Bd`   | `baud`, `bauds` | Baud (symbol/s) | `bu_baud` | s⁻¹ |

SI prefixes are accepted on all units in this section. IEC prefixes are rejected for all non-digital units.

### 4.26 Ratio and Proportion

Dimensionless scaling factors: `5 %` ≡ `0.05`, `250 ppm` ≡ `0.00025`. These do **not** accept SI or IEC prefixes.

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `%`   | `percent` | per cent | `bu_percent` | 10⁻² |
| `‰`   | `per_mille` | per mille | `bu_per_mille` | 10⁻³ |
| `‱`   | `per_myriad` | per myriad | `bu_per_myriad` | 10⁻⁴ |
| `pcm` | `per_cent_mille` | per cent mille | `bu_per_cent_mille` | 10⁻⁵ |
| `ppm` | — | parts per million | `bu_ppm` | 10⁻⁶ |
| `ppb` | — | parts per billion | `bu_ppb` | 10⁻⁹ |
| `pptr` | `parts_per_trillion`, `pptv` | parts per trillion | `bu_ppt` | 10⁻¹² |
| `ppq` | `parts_per_quadrillion`, `ppqv` | parts per quadrillion | `bu_ppq` | 10⁻¹⁵ |

---

### 4.27 Named Speeds & Acidity

| Symbol | Long forms | Name | Enum value | Factor |
|--------|-----------|------|------------|--------|
| `mph`  | — | mile per hour | `bu_mile_per_hour` | 0.44704 m·s⁻¹ (exact) |
| `kph`  | `kmh` | kilometre per hour | `bu_kilometer_per_hour` | 5/18 m·s⁻¹ (exact) |
| `pH`   | — | acidity | `bu_ph_scale` | 1 (dimensionless, own logarithmic kind) |

> `mi/h` and `k~m/h` are the same quantities written as compounds; both spellings are valid, but a
> named unit and its compound do not reconcile inside one value. `pH` is not `p~H` (picohenry), and
> `ph` is the phot.

### 4.28 Water Hardness

Six scales for one quantity — the concentration of dissolved alkaline-earth ions. All carry
mol·m⁻³ and convert into each other and into `m~mol/L`. None takes a prefix.

| Symbol | Long form | Enum value | Defined as | 1 mmol/L = |
|--------|-----------|------------|------------|------------|
| `°dH` | `german_hardness` | `bu_german_hardness` | 10 mg CaO / L | 5.6077 |
| `°e`, `°Clark` | `english_hardness`, `clark_degree` | `bu_english_hardness` | 1 grain CaCO₃ / imp. gallon | 7.0217 |
| `°fH` | `french_hardness` | `bu_french_hardness` | 10 mg CaCO₃ / L | 10.0086 |
| `°rH` | `russian_hardness` | `bu_russian_hardness` | 1 mg Ca / L | 40.078 |
| `°aH` | `american_hardness` | `bu_american_hardness` | 1 mg CaCO₃ / L ("ppm") | 100.086 |
| `gpg` | `grains_per_gallon` | `bu_grains_per_gallon` | 1 grain CaCO₃ / US gallon | 5.8468 |
| `val` | `vals`, `eq` | `bu_val` | ½ mol (divalent, water analysis) | 2.000 `mval/L` |

> Keep the degree sign: `dH` without it is the **decihenry**. Water chemistry's "ppm" is `°aH`,
> not Bovnar's dimensionless `ppm`. Millimoles per litre is just the compound `m~mol/L`, and
> `meq/L` is the same unit as `mval/L`. `gpg` (amount concentration) is not `gr/gal` (mass
> concentration).

**Conductivity and dissolved solids need no units of their own:** EC is `µS/cm`, `mS/cm`, `dS/m`
or `S/m` (`µmho/cm` too — `mho`/`℧` are the siemens); TDS is `mg/L`; resistivity is `MΩ·cm`.
"TDS in ppm" means mg/L, while Bovnar's `ppm` is the dimensionless 10⁻⁶.

### 4.29 Water-Quality Instrument Scales

| Symbol | Long form | Enum value | Method / definition | Converts to |
|--------|-----------|------------|---------------------|-------------|
| `NTU` | `nephelometric_turbidity` | `bu_turbidity_ntu` | white light, 90° (EPA 180.1) | itself only |
| `FNU` | `formazin_nephelometric` | `bu_turbidity_fnu` | near-IR 860 nm, 90° (ISO 7027) | itself only |
| `FTU` | `formazin_turbidity` | `bu_turbidity_ftu` | formazin, geometry unstated | itself only |
| `FAU` | `formazin_attenuation` | `bu_turbidity_fau` | attenuation at 0° (ISO 7027) | itself only |
| `JTU` | `jackson_turbidity` | `bu_turbidity_jtu` | visual candle method (historical) | itself only |
| `PSU` | `practical_salinity` | `bu_practical_salinity` | PSS-78 conductivity ratio | itself only |
| `CF` | `conductivity_factor` | `bu_conductivity_factor` | EC in mS/cm × 10 | `µS/cm`, `mS/cm`, `S/m` |

> The five turbidity scales agree on a formazin standard and not on real water — and `FAU` is not
> even the same optical quantity (attenuation, not scatter), while `JTU` is visual and relates to
> formazin only near 40 units. Bovnar refuses every conversion between them; report the method.
> `JTU` and `PSU` take no prefix. PSU is a conductivity ratio, **not** a mass fraction — for
> absolute salinity write `g/k~g`. Case matters: `cF` is the centifarad and `fau` the
> femto-astronomical-unit.

---

### 4.30 Survey, Typographic, Dry-Volume and Trade Units

Every unit here was, until it was added, the sole reason a run of UCUM, UDUNITS-2, QUDT, OM or
UN/ECE codes had to be refused. All are exact; the ones whose value is not a terminating decimal in
SI state a rational rather than a rounded double.

#### US survey lengths

`ftUS` (§4.4) has always been here and nothing was built on it. These are exact rational multiples
of it, and the survey foot is 2 ppm longer than the international one — which is small enough to
ignore and never small enough to be right.

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `inUS` | `survey_inch` | US survey inch | `bu_survey_inch` | 100/3937 m ≈ 0.02540005 m |
| `ydUS` | `survey_yard` | US survey yard | `bu_survey_yard` | 3600/3937 m ≈ 0.91440183 m |
| `fathUS` | `survey_fathom` | US survey fathom | `bu_survey_fathom` | 7200/3937 m ≈ 1.82880366 m |
| `rdUS` | `survey_rod` | US survey rod (pole, perch) | `bu_survey_rod` | 19800/3937 m ≈ 5.02921006 m |
| `chUS` | `survey_chain` | US survey chain (Gunter's) | `bu_survey_chain` | 79200/3937 m ≈ 20.11684023 m |
| `lkUS` | `survey_link` | US survey link | `bu_survey_link` | 792/3937 m ≈ 0.20116840 m |
| `furUS` | `survey_furlong` | US survey furlong | `bu_survey_furlong` | 792000/3937 m ≈ 201.16840234 m |
| `miUS` | `survey_mile`, `survey_miles` | US survey (statute) mile | `bu_survey_mile` | 6336000/3937 m ≈ 1609.34721869 m |

> The survey foot was withdrawn for new work at the end of 2022. That is a reason to **read** it
> carefully, not to refuse it: US land records, state-plane coordinates and a century of drawings
> are written in these. `ac` vs `acUS` differ by 4 ppm — twice the foot's 2 ppm, an area being a
> length squared — which is about ten square metres on a section of land.

#### Typographic lengths

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `pnt` | `point`, `points` | DTP point (¹⁄₇₂ in) | `bu_point` | 127/360000 m ≈ 0.35277778 mm |
| `pca` | `pica`, `picas` | pica (12 points, ⅙ in) | `bu_pica` | 127/30000 m ≈ 4.23333333 mm |
| `lne` | `line`, `lines` | line (¹⁄₁₂ in) | `bu_line` | 127/60000 m ≈ 2.11666667 mm |

> The symbols are not `pt` and `ln`: `pt` is the **pint**. A length answering to `pt` would be the
> same collision as `kt` for the knot, which §6 refuses outright. The **printer's** point
> (0.013837 in) is a different unit and is carried separately, as `pnt_pr` below.

#### US dry volumes and the trade measures

A dry quart is 16 % larger than the liquid `qt`. The peck and bushel here have always been the dry
ones; the gallon, quart and pint were only the liquid ones.

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `gal_dry` | `dry_gallon`, `dry_gallons` | US dry gallon | `bu_dry_gallon` | 268.8025 in³ = 4.40488377086×10⁻³ m³ |
| `qt_dry` | `dry_quart`, `dry_quarts` | US dry quart | `bu_dry_quart` | 1.101220942715×10⁻³ m³ |
| `pt_dry` | `dry_pint`, `dry_pints` | US dry pint | `bu_dry_pint` | 5.506104713575×10⁻⁴ m³ |
| `fbm` | `board_foot`, `board_feet` | board foot (144 in³) | `bu_board_foot` | 2.359737216×10⁻³ m³ |
| `cord` | `cord`, `cords` | cord (128 ft³) | `bu_cord` | 3.624556363776 m³ |
| `ac_ft` | `acre_foot`, `acre_feet` | acre-foot (survey) | `bu_survey_acre_foot` | 1233.4892384681489 m³ |

> `ac_ft` is built on the **survey** acre and foot, which is what UDUNITS, OM and every US water
> agency mean by an acre-foot. The international one is `ac·ft` = 1233.48183754752 m³.

#### And the singles

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `acUS` | `survey_acre`, `survey_acres` | US survey acre | `bu_survey_acre` | 62726400000/15499969 m² ≈ 4046.87261 m² |
| `darcy` | `darcy`, `darcys`, `darcies` | darcy (permeability) | `bu_darcy` | 1/1013250000000 m² ≈ 9.86923267×10⁻¹³ m² |
| `thm_ec` | `therm_EC` | EC therm (10⁵ `Btu`) | `bu_therm_ec` | 1.05505585262×10⁸ J (exact) |
| `ton_ref` | `refrigeration_ton`, `ton_of_refrigeration` | ton of refrigeration (12 000 `Btu`/h) | `bu_refrigeration_ton` | 52752792631/15000000 W ≈ 3516.85284 W |
| `DU` | `dobson`, `dobson_unit`, `dobson_units` | Dobson unit (ozone column) | `bu_dobson` | 4.462×10⁻⁴ mol·m⁻² (exact) |
| `shake` | `shake`, `shakes` | shake | `bu_shake` | 10⁻⁸ s (exact) |

> `darcy` takes prefixes, so the millidarcy every reservoir report is written in is `m~darcy`.
> `thm_ec` is the European gas-billing therm; native `thm` is the US therm, 0.24 % away.

#### Nine more the unit profiles needed

Each was refused across UCUM, UDUNITS-2, QUDT and OM for want of a native unit of the magnitude, and
every factor below is one **two or three publishers state independently**, agreeing to the last digit.

| Symbol | Long forms | Name | Enum | Factor |
|--------|-----------|------|------|--------|
| `pnt_pr` | `printers_point` | printer's point | `bu_printers_point` | 0.0003514598 m (exact, = 0.013837 `in`) |
| `pca_pr` | `printers_pica` | printer's pica | `bu_printers_pica` | 0.0042175176 m (exact, = 12 `pnt_pr`) |
| `hp_E` | `electric_horsepower` | electric horsepower | `bu_horsepower_electric` | 746 W (exact) |
| `hp_B` | `boiler_horsepower` | boiler horsepower | `bu_horsepower_boiler` | 9809.5 W |
| `abV` | `abvolt` | abvolt (CGS-EMU) | `bu_abvolt` | 10⁻⁸ V (exact) |
| `AT` | `assay_ton` | assay ton (short) | `bu_assay_ton` | 175/6000 kg ≈ 0.029166667 kg |
| `bsh_uk` | `bushel_uk` | imperial bushel | `bu_bushel_uk` | 0.03636872 m³ (exact, = 8 `gal_uk`) |
| `clo` | — | clo | `bu_clo` | 0.155 K·m²/W (exact) |
| `debye` | — | debye | `bu_debye` | 1/299792458000000000000000000000 C·m ≈ 3.3356410×10⁻³⁰ C·m |

> Each is a **near neighbour** of a unit already here, which is why it needed a row rather than an
> alias: `pnt_pr` is 0.37 % off `pnt`, `hp_E` 0.04 % off `hp`, `bsh_uk` 3.2 % off `bsh`, and `hp_B`
> thirteen times any horsepower. `abV` needed a unit because no decade prefix reaches 10⁻⁸ and
> prefixes do not stack. `AT` and `debye` are exact rationals — 175/6 g, and 10⁻²¹/c C·m, exact
> since the 2019 SI fixed c — of which their publishers state 7- and 6-digit roundings.

#### And the thirty-five the same sweep found next

The rest of the refusals **two or more vocabularies define**. Where publishers disagree the exact
definition decides: the CGS-ESU units are built on `c`, exact since 2019, so their five- to
seven-digit decimals are roundings of the rational stated here. The π-based rows carry
`.exact = false`. `yr` is still the Julian year — naming the other calendars is what lets a UDUNITS
or CF document that says `year` be read at all.

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

## 5. Currencies

### 5.1 The Mandatory Currency Sigil

As of spec 1.0 a currency code carries a **mandatory `$` sigil**: write `$USD`,
`$BTC`, `k~$EUR` (or compactly `k$EUR`), `$USD/oz_t`. The codes listed in the tables below are the bare
ISO 4217 / crypto identifiers — prefix each with `$` when you use it in a document.
A bare code (no `$`) is **not** a currency; it is matched against the physical-unit
table and raises `error_unit_illegal` if it is not a unit. This removes every
currency/unit namespace collision (e.g. `$CUP` the Cuban Peso vs `cup` the unit).

### 5.2 ISO 4217 Fiat Currencies

166 codes at ids **900000 … 900165**, the front of block 90 of the `value_base_unit_t` id space (`BVN_CURRENCY_FIRST` …). They have no named `bu_*` enumerators — they are resolved from the `$`-sigil code by `bvn_parse_currency_str` and carried as the numeric `base` value; query them with `bvn_unit_is_fiat` / `bvn_currency_info`, both of which read the catalogue row rather than testing the id's range.

> **Min** = minor unit exponent N: 1 major unit = 10^N minor units (e.g. 1 USD = 100 cents, N=2).
> Minor units are **bold** when they differ from 2. `numeric_code` is the ISO 4217 numeric identifier.

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
| `XTS` |  963 | **0** | Test (ISO 4217 reserved) |
| `YER` |  886 |   2 | Yemeni Rial |
| `ZAR` |  710 |   2 | South African Rand |
| `ZMW` |  967 |   2 | Zambian Kwacha |
| `ZWG` |  924 |   2 | Zimbabwe Gold |
| `ZWL` |  932 |   2 | Zimbabwean Dollar *(historical; superseded by ZWG 2024)* |

> `CLF` is the only currency with 4 minor units.
> `XTS` is the ISO 4217 testing code; present in the table but should not appear in production data.

### 5.3 Cryptocurrencies

50 codes at ids **900166 … 900215**, after the fiat codes in the same block 90. Like them they have no named `bu_*` enumerators — resolved by `bvn_parse_currency_str`, queried with `bvn_unit_is_crypto` / `bvn_currency_info`. `numeric_code = 0` for all.

> **Min** = `minor_unit` = on-chain decimal places. E.g. `<uint:64,$BTC>` stores satoshis; divide by 10⁸ to obtain BTC.

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

### 5.4 Currency Prefix Rules

All 24 SI prefixes are allowed on all currency units. IEC binary prefixes are forbidden on all currencies (`error_unit_illegal`). The `~` is optional — the `$` sigil already separates the prefix from the code — but the sigil is not: `kUSD` is `error_unit_illegal`.

| Example | Meaning |
|---------|---------|
| `k~$USD` (or `k$USD`) | thousands of USD (×10³) |
| `M~$EUR` (or `M$EUR`) | millions of EUR (×10⁶) |
| `G~$ETH` (or `G$ETH`) | giga-ETH = Gwei scale (×10⁹) |
| `m~$USD` (or `m$USD`) | milli-USD = one tenth of a cent (×10⁻³) |

---

## 6. Symbol Disambiguation

> The full token-by-token treatment — case traps, look-alike code points, same-dimension quantities, and
> abbreviations that are deliberately not units — is in [`07_bovnar_unit_ambiguities.md`](07_bovnar_unit_ambiguities.md).

As of spec 1.0 a currency is written **only** with the `$` sigil (§5.1), so the bare form is always a physical-unit lookup and the namespaces never collide:

| Token | Bare form (no `$`) | `$`-sigil form |
|-------|--------------------|----------------|
| `cup` / `CUP` | `cup` → US cup (`bu_cup`); `CUP` → `error_unit_illegal` | `$CUP` → Cuban Peso (ISO 4217:192) |
| `BTU`  | `BTU` → BTU (`bu_btu`); `Btu`, `btu` also accepted | *(not ISO 4217)* |
| `SOL`  | `SOL` → `error_unit_illegal` | `$SOL` → Solana (crypto) |
| `BAR`  | `BAR` → `error_unit_illegal`; use lowercase `bar` | *(not ISO 4217)* |
| `ERG`  | `ERG` → `error_unit_illegal`; use lowercase `erg` | *(not ISO 4217)* |
| `CAD`, `XAU` | `error_unit_illegal` (no physical unit) | `$CAD` → Canadian Dollar; `$XAU` → Gold (X-code) |

No bare token is simultaneously a valid physical unit and a currency: currencies live entirely under `$`, physical units entirely without it.

---

## See also

- [Unit & Currency Reference](05_bovnar_unit_system.md) — the full registry: dimensions, factors, prefix policy, and the C API
- [Unit Ambiguities](07_bovnar_unit_ambiguities.md) — which reading wins when a token could mean two things
- [Specification §11 — Units System](03_bovnar_spec.md#11-units-system) — how a unit attaches to a value
- [FAQ §4 — Units](02_bovnar_faq.md#4-units) — the questions these tables raise most often
- [Unit Profiles](11_bovnar_unit_profiles.md) — writing these same units as UCUM, UN/ECE, QUDT, UDUNITS, OM 2 or CF codes, and the profile-only units that exist in no native spelling

---

*The id space is blocked: the leading two digits of an id name its vocabulary. Native units 100000–100263 (264) · UCUM opaque units 200000–200039 (40) · UN/ECE opaque units 300000–300024 (25) · currencies 900000–900215 (166 fiat, 50 crypto). Blocks 40, 50, 60, 70 and 80 are reserved for QUDT, QUDT quantity kinds, UDUNITS, OM 2 and the CF standard names, which contribute no opaque units today.*
*The space is SPARSE — do not index an array by an id. `BVN_UNIT_SLOT_COUNT` = 330 is the row count of the library's dense tables, indexed by `bvni_unit_slot()`, and is not a bound on the enum.*

---

*End of Bovnar — Units & Currencies Cheat Sheet (Bovnar spec 1.2).*
