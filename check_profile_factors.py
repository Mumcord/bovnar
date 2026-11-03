#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_profile_factors.py — prove the profile tables against their publishers.

THE GAP THIS CLOSES. gen_profiles.py checks that every `.bovnar` target names
something the native registry has, and the per-profile test files check that a
code translates to the unit the table says. Neither checks the table against the
publication the codes come from, and doc/11 §9.2, §10.2 and §10.4 all say so:
five tables wrong in the same way would agree with each other perfectly. This is
the missing half. It reads the publisher's own machine-readable definitions,
resolves each code to a factor and a dimension vector, asks the REFERENCE
LIBRARY what the mapped native target is worth, and compares.

It found, on the tree it was written against: udunits' `oz` mapped to the
avoirdupois ounce when UDUNITS defines it as the US fluid ounce (a volume read
as a mass), the unqualified `calorie` mapped to the thermochemical one when
UDUNITS means the IT calorie, `year`/`month` mapped to the Julian year when
UDUNITS means the tropical one, and `acre`/`chain`/`rod`/`furlong` mapped onto
international lengths when UDUNITS builds them on the US survey foot. Extending
it to QUDT found six local names that vocabulary does not define, a month that
was the lunar one, and a rotational speed 2π out.

WHY IT ASKS THE LIBRARY RATHER THAN COMPUTING THE NATIVE SIDE ITSELF. A second
implementation of the native unit grammar in Python is exactly how a generated
table starts disagreeing with the parser it feeds — gen_profiles.py's own header
says this. So the native side of every comparison goes through bvn_parse_unit
and bvn_unit_to_si_factor via the ctypes bindings. If those are wrong this tool
cannot see it; what it can see is the table disagreeing with the publisher,
which is the failure mode nothing else covers.

THREE OUTCOMES, AND ONLY TWO ARE ERRORS.

  MISMATCH   the code exists upstream and means something else — a different
             dimension, or a factor outside tolerance. This is the one that
             corrupts data silently, and it fails the run.
  DEAD       the table accepts a spelling the publisher does not define. It can
             never be produced by a conforming producer, so it costs nothing but
             an unverifiable row. Reported, not fatal (--strict-dead to change).
  MISSING    the publisher defines a code that is EXACTLY a native unit and the
             table does not map it. Advisory: it is a coverage suggestion, not a
             defect, and the decision to carry a unit is editorial.

AND ONE COMPARISON THAT IS NOT A NUMBER AT ALL. Everything above compares a
factor and a dimension vector, and a dimension vector has no room for a QUANTITY
KIND: a bit, a radian, a steradian, a decibel and a pure ratio are all
[0,0,0,0,0,0,0], and a bit per second and a hertz are both T⁻¹ at factor 1. So a
mapping could turn a data rate into a frequency and score perfectly here. It
did, seven times over, until `check_quantity_kinds` was added — see the note on
that function. It uses QUDT's own unit-to-quantity-kind links to check the two
QUDT tables against each other, on the one axis the factor comparison is blind
to, and a disagreement there fails the run like a MISMATCH.

AND ONE THAT COMPARES TWO BOVNAR TABLES RATHER THAN A TABLE AND A PUBLISHER.
Everything above asks whether one of these tables agrees with the vocabulary it
came from. A defect can survive that by being consistent within its own
vocabulary: QUDT models the international unit as an amount of substance
throughout, so nothing internal to QUDT objects to `IU-PER-MilliGM` becoming
µmol/kg — while UCUM, and bovnar's own ucum table, hold [IU] to be arbitrary and
commensurable with nothing. `check_cross_reference` uses QUDT's published
ucumCode/udunitsCode as a claim that two rows in two different bovnar tables
describe one unit, and checks that they do. It found three defects the other two
could not.

ALL SEVEN PROFILES ARE COVERED, BUT NOT ALL EQUALLY. `ucum`, `udunits`, `qudt`,
`qudt-qk`, `om` and `cf` are checked against their publishers' own definitions.
`unece` is checked at ONE REMOVE, through QUDT's `qudt:uneceCommonCode`
cross-reference, because Rec 20 states its conversion factors in prose and there
is no primary artefact to resolve. That is QUDT asserting what a Rec 20 code
means, so a disagreement there is evidence that one of the two tables is wrong,
never proof of which — `class Unece` says what follows from that, and the run
prints the distinction rather than letting a secondary result read like a
primary one.

THE FOUR RESOLVERS ARE FOUR DIFFERENT SHAPES, because the publishers are:

  expression   UCUM and UDUNITS state a unit as an expression over other units,
               so each needs a small evaluator.
  table        QUDT states every unit's own multiplier and dimension vector, so
               reading it is a lookup.
  composition  OM states no multiplier at all. It states how a unit is BUILT --
               prefix and base, numerator and denominator, term and term, base
               and exponent — and the value falls out of walking that down to
               the SI base units. `class Om` is that walk, and it is the same
               structure om.bvnr's targets were derived from, which is the point
               of checking it: a target and a value that came from one reading
               of the ontology are compared against the library's independent
               reading of the target.
  two sources  CF states no units either; it states each standard name's
               `canonical_units` as a UDUNITS expression. So `class Cf` reads
               CF's table for the name and hands the string to the UDUNITS
               evaluator above. Both publishers are primary, and neither is
               being asked about the other's vocabulary.

Arbitrary and special units (UCUM `isArbitrary`/`isSpecial`, QUDT units with no
`conversionMultiplier`) have no factor to check and are skipped — they are
exactly the ones bovnar carries as opaque or refuses outright.

QUDT NEEDS NO EVALUATOR. UCUM and UDUNITS state a unit as an EXPRESSION over
other units, so both need a parser; QUDT states each unit's own
`conversionMultiplier` and its dimension vector as an IRI local name
("A0E0L1I0M0H0T0D0"), so reading it is a table lookup. Its quantity kinds have
no multiplier at all, and there the check is the claim doc/11 §12.3 actually
makes: that a kind maps to the COHERENT SI unit of that kind. Reporting a kind
as (1.0, its dimensions) turns that into two ordinary assertions — the
dimensions agree and the native factor is exactly 1 — so a kind mapped to a
non-coherent unit fails on the factor even though its dimensions are perfect.

THE TWO UNIT SYSTEMS ARE NOT THE SAME SYSTEM, and the corrections below are the
substance of the UCUM comparison rather than a detail of it:

  * UCUM's mass base is the GRAM; bovnar's factors are coherent SI, i.e. kg. A
    UCUM factor is divided by 1000^(mass exponent).
  * UCUM has no amount base — the mole is a NUMBER, Avogadro's. Anything with a
    mole in it therefore carries N_A that the bovnar factor does not, so the
    UCUM factor is divided by N_A^(amount exponent), using UCUM's OWN value of
    N_A so it cancels exactly rather than to fifteen digits.
  * UCUM's electrical base is CHARGE (dim Q); bovnar's is current. Q maps to
    current+1 and time+1, which is what makes `A` = `C/s` come out as a bare
    current and `C` as current·time.
  * UCUM's plane angle (dim A) is a base; bovnar's radian is dimensionless. The
    A component is dropped, which is what lets `rad`, `sr` and `deg` compare.

Usage:
    python3 check_profile_factors.py --fetch     # download, then check
    python3 check_profile_factors.py             # check; skip if no cache
    python3 check_profile_factors.py --strict    # a missing cache is a failure
    python3 check_profile_factors.py --verbose   # list every row checked
    python3 check_profile_factors.py --profile ucum
"""
import argparse
import os
import re
import sys
import xml.etree.ElementTree as ET

REPO = os.path.dirname(os.path.abspath(__file__))
GENDATA = os.path.join(REPO, "src", "gendata")
DEFAULT_CACHE = os.environ.get(
    "BVNR_VOCAB_DIR", os.path.join(REPO, "build", "vocab"))

sys.path.insert(0, REPO)
import bvnr_data  # noqa: E402

# Where each vocabulary's definitions come from. Fetched only on --fetch: a test
# must not depend on the network (check_web_links.py makes the same rule).
SOURCES = {
    "udunits": [
        ("udunits2-base.xml",
         "https://raw.githubusercontent.com/Unidata/UDUNITS-2/master/lib/udunits2-base.xml"),
        ("udunits2-derived.xml",
         "https://raw.githubusercontent.com/Unidata/UDUNITS-2/master/lib/udunits2-derived.xml"),
        ("udunits2-accepted.xml",
         "https://raw.githubusercontent.com/Unidata/UDUNITS-2/master/lib/udunits2-accepted.xml"),
        ("udunits2-common.xml",
         "https://raw.githubusercontent.com/Unidata/UDUNITS-2/master/lib/udunits2-common.xml"),
    ],
    "ucum": [
        ("ucum-essence.xml",
         "https://raw.githubusercontent.com/ucum-org/ucum/main/ucum-essence.xml"),
    ],
    # One file serves both QUDT namespaces' checks; `qudt-qk` additionally needs
    # the quantity-kind vocabulary. Both are served by content negotiation off
    # the versioned base URI, which is the form that actually resolves — the
    # GitHub raw path for the same file 404s.
    "qudt": [
        ("qudt-units.ttl", "https://qudt.org/3.1.0/vocab/unit"),
    ],
    "qudt-qk": [
        ("qudt-units.ttl", "https://qudt.org/3.1.0/vocab/unit"),
        ("qudt-quantitykinds.ttl", "https://qudt.org/3.1.0/vocab/quantitykind"),
    ],
    # UN/ECE is checked through QUDT's uneceCommonCode cross-reference, so it
    # needs the same file and no publication of its own. See class Unece for
    # what that does and does not prove.
    "unece": [
        ("qudt-units.ttl", "https://qudt.org/3.1.0/vocab/unit"),
    ],
    "om": [
        ("om-2.0.rdf",
         "https://raw.githubusercontent.com/HajoRijgersberg/OM/master/om-2.0.rdf"),
    ],
    # CF states each standard name's canonical_units as a UDUNITS expression, so
    # checking this vocabulary means reading TWO publishers: CF for the name's
    # units and Unidata for what those units are worth. Both are primary.
    #
    # PINNED TO THE VERSIONED PATH, not `current/`. cf.bvnr was built against
    # table v94 and THIRD_PARTY_NOTICES.md attributes that version; fetching
    # `current/` would silently move the thing this run compares against, so a
    # table that stopped matching its own recorded provenance would look like a
    # table that had gone wrong. Bumping the version is an edit here, in the
    # .bvnr header and in the notices file, in one commit.
    "cf": [
        ("cf-standard-name-table.xml",
         "https://cfconventions.org/Data/cf-standard-names/94/src/"
         "cf-standard-name-table.xml"),
        ("udunits2-base.xml",
         "https://raw.githubusercontent.com/Unidata/UDUNITS-2/master/lib/udunits2-base.xml"),
        ("udunits2-derived.xml",
         "https://raw.githubusercontent.com/Unidata/UDUNITS-2/master/lib/udunits2-derived.xml"),
        ("udunits2-accepted.xml",
         "https://raw.githubusercontent.com/Unidata/UDUNITS-2/master/lib/udunits2-accepted.xml"),
        ("udunits2-common.xml",
         "https://raw.githubusercontent.com/Unidata/UDUNITS-2/master/lib/udunits2-common.xml"),
    ],
}

# Bovnar's dimension order, as bvn_unit_dimension_vector fills it.
NDIM = 7
LENGTH, MASS, TIME, CURRENT, TEMP, AMOUNT, LUM = range(NDIM)

# Relative tolerance for a factor comparison, CALIBRATED against the two
# publishers rather than picked round.
#
# A publisher states decimals, and states them at whichever CODATA edition it was
# last revised against: UDUNITS writes the horsepower as 7.456999e2 W and the
# atomic mass unit at the 1986 adjustment, UCUM states that unit at 2018, and
# units.bvnr states it at 2022 (its header names the edition). So a correct row
# still disagrees in the seventh digit or so. Across all five tables every such
# disagreement lands at or below 6.81e-7. The genuine errors this tool was
# written to catch are above it: the US survey foot at 2e-6, the survey acre at
# 4e-6, the tropical year at 2.1e-5, the IT calorie at 6.7e-4.
#
# The gap is MUCH NARROWER than the survey foot suggests, and pretending
# otherwise would be the dangerous mistake here. UCUM's [ch_br] -- the BRITISH
# chain, built on a foot that differs from the international one in the seventh
# digit -- sits 7.87e-7 from bovnar's international chain. That is a genuinely
# different unit, and at a tolerance of 1e-6 it passed as a match. The real
# boundary is therefore:
#
#     largest publisher rounding seen     6.81e-7   (UDUNITS' 1986 CODATA amu
#                                                    against bovnar's 2022 one)
#     smallest genuine difference seen    7.87e-7   (British vs international)
#
# 7.5e-7 is the only value that separates them, and it separates them by 10 %.
# This is a measurement, not a margin: a real disagreement below 6.8e-7 -- the
# British/international foot ratio applied to a shorter chain, say -- would
# still pass. Nothing better is available without tracking each publisher's
# stated precision through its whole definition chain, which is the honest
# limit of a factor comparison against a source that publishes decimals.
TOL = 7.5e-7

# Differences that are MODELLING choices rather than errors. bovnar carries bit
# and byte as two base units of information with no factor between them; UCUM
# and UDUNITS both define the byte as the number 8, which is a different and
# equally defensible model, not a wrong conversion.
#
# QUDT goes further than the other two: it models information as ENTROPY, so its
# bit is ln(2) = 0.693 and its byte 8·ln(2) = 5.545, the SI-coherent unit of
# entropy being the nat. bovnar's bit is 1. qudt-qk.bvnr already refuses
# `InformationEntropy` for exactly this reason.
#
# THE WHOLE QUDT INFORMATION FAMILY IS THAT ONE DIFFERENCE, REPEATED. Every row
# below is a prefixed bit or byte, a rate of one, or a linear density of one,
# and each is off the native target by exactly ln 2 or 8·ln 2 — never by
# anything else. Listing them by name rather than widening the tolerance is
# deliberate: the ratio is 0.693, so a tolerance that admitted it would admit
# every real disagreement in the file. A row that stops being off by exactly the
# model factor stops being waived and fails, which is the property that matters.
_QUDT_INFO_MODEL = [
    "BIT", "BYTE",
    # decimal bit and byte
    "KiloBIT", "MegaBIT", "GigaBIT", "TeraBIT", "PetaBIT", "ExaBIT",
    "KiloBYTE", "MegaBYTE", "GigaBYTE", "TeraBYTE", "PetaBYTE", "ExaBYTE",
    # binary bit and byte
    "KibiBIT", "MebiBIT", "GibiBIT", "TebiBIT",
    "KibiBYTE", "MebiBYTE", "GibiBYTE", "TebiBYTE",
    # the shannon, which is the bit under its information-theory name
    "SHANNON", "SHANNON-PER-SEC",
    # rates
    "BIT-PER-SEC", "BYTE-PER-SEC",
    "KiloBIT-PER-SEC", "MegaBIT-PER-SEC", "GigaBIT-PER-SEC",
    "TeraBIT-PER-SEC", "PetaBIT-PER-SEC",
    "KiloBYTE-PER-SEC", "MegaBYTE-PER-SEC", "GigaBYTE-PER-SEC",
    # linear, areal and volumetric bit densities
    "BIT-PER-M", "BIT-PER-M2", "BIT-PER-M3", "GigaBIT-PER-M",
    "KibiBIT-PER-M", "KibiBIT-PER-M2", "KibiBIT-PER-M3",
    "MebiBIT-PER-M", "MebiBIT-PER-M2", "MebiBIT-PER-M3",
    "GibiBIT-PER-M", "GibiBIT-PER-M2", "GibiBIT-PER-M3",
]
# The same family again in Rec 20's numbering, reached through the same QUDT
# units and therefore carrying the same ln 2. These are UNECE codes, but the
# difference being waived is QUDT's model and not a claim about Rec 20 — which
# is the whole reason this vocabulary's results are labelled secondary.
_UNECE_INFO_MODEL = [
    "J63", "AD", "Q14", "Q12",                     # bit, byte, shannon, octet
    "C37", "D36", "B68", "E83", "E78",             # decimal bit
    "2P", "4L", "E34", "E35", "E36",               # decimal byte
    "C21", "D11", "B30",                           # binary bit
    "E64", "E63", "E62", "E61",                    # binary byte
    "B10", "P93", "Q13", "Q17",                    # rates
    "C74", "E20", "B80", "E84", "E79",
    "P94", "P95", "E68",
    "E88", "E89",                                  # densities
    "E72", "E73", "E74", "E75", "E76", "E77",
    "E69", "E70", "E71",
]
WAIVED_MODEL = {
    ("ucum", "bit"), ("ucum", "By"), ("ucum", "Bd"),
    ("udunits", "bit"), ("udunits", "byte"), ("udunits", "baud"),
    ("udunits", "Bd"), ("udunits", "bps"),
    # UDUNITS' own alias for the byte, and the same modelling difference: it
    # defines the octet as the number 8 where bovnar's B is a base unit of
    # information with no factor to the bit.
    ("udunits", "octet"),
    # ...and their regular plurals, which are rows of their own now.
    ("udunits", "bytes"), ("udunits", "octets"),
} | {("qudt", code) for code in _QUDT_INFO_MODEL} \
  | {("unece", code) for code in _UNECE_INFO_MODEL}

# Places where the PUBLISHER is wrong, or states a value rounded past the
# tolerance above, and bovnar is deliberately not following it. Each is reported
# as a note on every run rather than silently skipped: a waiver nobody sees is
# how a real regression hides behind an old excuse. Adding to this list is a
# claim about the publisher and needs the same evidence as changing a table.
WAIVED_UPSTREAM = {
    ("ucum", "ph"):
        "UCUM defines the phot as 1e-4 lx. A phot is one lumen per square "
        "centimetre, i.e. 1e4 lx — the value is inverted. Native ph is correct "
        "and the profile keeps mapping to it.",
    ("ucum", "m[Hg]"):
        "UCUM rounds the mercury column to 133.3220 kPa (7 digits); native mmHg "
        "is the exact conventional 133.322387415 Pa. 2.9 ppm, publisher "
        "rounding rather than a different unit.",
    # The three atoms UCUM builds ON the mercury column. Each disagrees with the
    # native target by the SAME 2.9 ppm and for the same reason — the rounding
    # is inherited, not independent — so each is waived by name rather than by
    # widening the tolerance, which would let a real disagreement through
    # everywhere else. The ratio to check against is 0.999997094.
    ("ucum", "[in_i'Hg]"):
        "Inherits m[Hg]'s rounding: UCUM's inch of mercury is its rounded metre "
        "of mercury times an inch. 2.9 ppm from the exact native inHg, the same "
        "ratio as m[Hg].",
    ("ucum", "[PRU]"):
        "Peripheral resistance unit, defined by UCUM as mm[Hg].s/ml, so it "
        "carries m[Hg]'s rounding unchanged. 2.9 ppm, same ratio as m[Hg].",
    ("ucum", "[wood'U]"):
        "Wood unit, defined by UCUM as mm[Hg].min/L, so it carries m[Hg]'s "
        "rounding unchanged. 2.9 ppm, same ratio as m[Hg].",
    ("unece", "MON"):
        "Reached through QUDT, which attaches uneceCommonCode \"MON\" to its own "
        "MO — a unit its description calls the SYNODIC month, 29.53059 days. Rec "
        "20's MON is a commercial month, so this is the cross-reference being "
        "wrong rather than the table: a trade code list does not mean the lunar "
        "cycle. Exactly the limit a secondary source has, and the reason this "
        "vocabulary's disagreements are evidence rather than proof.",
    # The CGS-ESU ohm and henry, and the mean calorie. QUDT states each at five
    # or three significant digits where the definition is exact and where the
    # OTHER publishers of the same unit state it precisely: UDUNITS has the
    # statohm at 898755400000 and OM at 898755200000, both within tolerance of
    # the exact 22468879468420441/25000 that c fixes; UCUM and OM both state the
    # mean calorie as exactly 4.19002 J. So this is QUDT rounding, corroborated
    # by the vocabularies beside it rather than asserted here.
    ("qudt", "OHM_Stat"):
        "QUDT rounds the statohm to 8.9876e11 (5 digits); the exact value is "
        "c·(10c) = 22468879468420441/25000 Ω = 898755178736.8176. 5.4 ppm — "
        "UDUNITS and OM both state it precisely.",
    ("qudt", "H_Stat"):
        "QUDT rounds the stathenry to 8.9876e11, the same 5-digit rounding it "
        "applies to the statohm, which is numerically the same value. 5.4 ppm.",
    ("qudt", "CAL_MEAN"):
        "QUDT rounds the mean calorie to 4.19 J (3 digits); UCUM's cal_m and "
        "OM's calorie-Mean both state 4.19002 J exactly. 4.8 ppm.",
    ("qudt", "TORR"):
        "QUDT rounds the torr to 133.322 Pa (6 digits); native Torr is the "
        "exact 101325/760. 2.8 ppm, the same publisher rounding as UCUM's "
        "mercury column.",
    # The EC therm, rounded to six digits by BOTH publishers that state it. The
    # evidence that it is a rounding rather than a different unit is in the same
    # two files: each states the US therm exactly (1.054804e8 J), so neither is
    # working from a different definition of a therm -- one number is written
    # long and the other short.
    ("udunits", "EC_therm"):
        "UDUNITS states the EC therm as 1.05506e8 J (6 digits); native thm_ec "
        "is the exact 100 000 Btu_IT = 105 505 585.262 J. 3.9 ppm, publisher "
        "rounding -- the same file states the US therm exactly.",
    ("udunits", "EC_therms"):
        "The regular plural of EC_therm, and the same 6-digit publisher "
        "rounding: 1.05506e8 J against the exact 105 505 585.262 J. 3.9 ppm.",
    ("om", "therm-EC"):
        "OM states the EC therm as 105 506 000 J (6 digits); native thm_ec is "
        "the exact 105 505 585.262 J. 3.9 ppm, the same rounding UDUNITS "
        "applies to the same unit.",
    # The magnetic pair of the ESU family, rounded by OM at the same place and
    # by the same ratio as each other -- 0.9999918 both, which is c truncated to
    # 2.9979e8 rather than the exact 2.99792458e8. OM's own statvolt, statampere
    # and statcoulomb are exact, so this is two coarse rows rather than a
    # different definition; native statWb and statT are statV·s and statV·s/cm²,
    # built on the c the 2019 SI fixed.
    ("om", "statweber"):
        "OM states the statweber as 299.79 (5 digits); native statWb is the "
        "exact statV·s = 299.792458 Wb. 8.2 ppm, OM rounding c to 2.9979e8 -- "
        "its own statvolt, statampere and statcoulomb are exact.",
    ("om", "stattesla"):
        "OM states the stattesla as 2 997 900 (5 digits); native statT is the "
        "exact statV·s/cm² = 2 997 924.58 T. 8.2 ppm, the same rounding of c "
        "OM applies to its statweber, and by the identical ratio.",
    # The four information units QUDT states at a value that is not its OWN
    # model. Every other member of the family is exactly ln 2 (or 8·ln 2) times
    # the corresponding power of two — see _QUDT_INFO_MODEL — and these four are
    # out by a further 8.9 and 7.5 with no pattern between them. Pebi- and Exbi-
    # name 2^50 and 2^60 and nothing else can, so the rows map and the
    # disagreement is recorded here, exactly as UCUM's inverted phot is.
    ("qudt", "PebiBIT"):
        "QUDT states the pebibit at 8.727e13 where its own bit (ln 2) times "
        "2^50 is 7.804e14 — a factor of 8.9 with no stated basis. PebiBYTE "
        "carries the same error and PebiBIT is exactly an eighth of it, so the "
        "two are one mistake. Native Pi~b is 2^50 bits.",
    ("qudt", "ExbiBIT"):
        "QUDT states the exbibit at 1.060e17 where its own bit times 2^60 is "
        "7.992e17 — a factor of 7.5, inherited from ExbiBYTE in the same way "
        "PebiBIT inherits PebiBYTE's. Native Ei~b is 2^60 bits.",
    ("qudt", "PebiBYTE"):
        "QUDT states the pebibyte at 6.981e14 where its own byte (8·ln 2) "
        "times 2^50 is 6.243e15 — a factor of 8.9. Native Pi~B is 2^50 bytes, "
        "which is what the name means in every standard that defines it.",
    ("qudt", "ExbiBYTE"):
        "QUDT states the exbibyte at 8.480e17 where its own byte times 2^60 is "
        "6.393e18 — a factor of 7.5. Native Ei~B is 2^60 bytes.",
    # Four thermochemical-BTU rates QUDT publishes at three or four significant
    # digits while publishing the same unit exactly elsewhere. The clearest is
    # BTU_TH-FT-PER-HR-FT2-DEG_F at 1.73 beside its own
    # BTU_TH-FT-PER-FT2-HR-DEG_F -- the identical unit with the operands in a
    # different order -- at 1.729577206. bovnar's Btu_th is exact
    # (23722880951/22500000 J), so the native side is the more precise of the
    # two and the rows map.
    ("qudt", "BTU_TH-PER-HR"):
        "QUDT rounds to 0.2929 (4 digits); the exact thermochemical BTU per "
        "hour is 0.29287507. 85 ppm, publisher rounding.",
    ("qudt", "KiloBTU_TH-PER-HR"):
        "The same 4-digit rounding as BTU_TH-PER-HR, a thousand times over: "
        "292.9 against the exact 292.87507. 85 ppm.",
    ("qudt", "BTU_TH-PER-MIN"):
        "QUDT rounds to 17.573 (5 digits); the exact value is 17.5725044. "
        "28 ppm, publisher rounding.",
    ("qudt", "S_Ab"):
        "QUDT gives the absiemens the dimension vector of the siemens per METRE "
        "(A0E2L-3I0M-1H0T3D0) while giving its own S the correct L-2. Its "
        "description says \"equal to 10^9 siemen\", its ucumCode says \"GS\", and "
        "its abohm, abfarad and abhenry all carry the conductance-shaped vector "
        "— four things against one. Native G~S is the conductance the name "
        "means; following the vector produced a conductivity, out by a metre.",
    ("qudt", "BTU_TH-FT-PER-HR-FT2-DEG_F"):
        "QUDT rounds to 1.73 (3 digits) while stating the SAME unit under "
        "BTU_TH-FT-PER-FT2-HR-DEG_F as 1.729577206 — the operands reordered and "
        "the value not rounded. 244 ppm, and QUDT's own table is the evidence.",
    # Rec 20's own codes for the two units above, reached through the same QUDT
    # rows and so carrying the same error.
    ("unece", "E60"):
        "Rec 20's pebibyte, reached through qudt:PebiBYTE — which QUDT states "
        "at 6.981e14 rather than 2^50 of its own byte. The cross-reference "
        "carries QUDT's error, not a statement about Rec 20; native Pi~B is "
        "2^50 bytes.",
    ("unece", "E59"):
        "Rec 20's exbibyte, reached through qudt:ExbiBYTE and carrying the same "
        "error as E60. Native Ei~B is 2^60 bytes.",
    # Rec 20's codes for the rounded QUDT rows above, inheriting the rounding
    # through the cross-reference rather than stating one of their own.
    ("unece", "J47"):
        "Rec 20's BTU (th) per hour, reached through qudt:BTU_TH-PER-HR and "
        "carrying its 4-digit rounding (0.2929 against the exact 0.29287507). "
        "85 ppm.",
    ("unece", "J51"):
        "Rec 20's BTU (th) per minute, carrying qudt:BTU_TH-PER-MIN's 5-digit "
        "rounding. 28 ppm.",
    ("unece", "J46"):
        "Rec 20's BTU (th) foot per hour square foot degree Fahrenheit, "
        "carrying qudt:BTU_TH-FT-PER-HR-FT2-DEG_F's 3-digit rounding — which "
        "QUDT contradicts in its own table. 244 ppm.",
    ("unece", "UA"):
        "Rec 20's torr, reached through qudt:TORR and carrying the same 6-digit "
        "rounding already waived there. 2.8 ppm; native Torr is the exact "
        "101325/760.",
    ("om", "metreOfMercury"):
        "The same 6-digit rounding of the mercury column OM applies to its "
        "millimetreOfMercury below, a thousand times over: 133322 against the "
        "exact conventional 133322.387415. 2.9 ppm.",
    ("om", "centimetreOfMercury"):
        "The same rounding again at the centimetre: 1333.22 against "
        "1333.22387415. 2.9 ppm.",
    ("om", "quart-Imperial"):
        "OM states the imperial quart at five digits, 0.0011365 m³ against the "
        "exact 0.0011365225 — 20 ppm. Its OWN imperial pint, gallon, gill and "
        "fluid ounce are exact and all four are mapped, so this is one coarse "
        "row rather than a different quart.",
    ("om", "millimetreOfMercury"):
        "OM rounds the mercury column to 133.322 Pa (6 digits); native mmHg is "
        "the exact conventional 133.322387415 Pa. 2.9 ppm, the same publisher "
        "rounding UCUM's m[Hg] and QUDT's TORR already carry.",
}


# Refusals the STALE REFUSALS sweep in check_profile matches, read one by one
# (2026-11-03) and kept. The sweep matches on value and dimension only, so it
# cannot see what each refusal is about; these are the ones where the match is
# a coincidence or drops what the refusal protects. A reviewed refusal is still
# listed under --verbose, and an entry the sweep no longer matches is reported,
# so a review cannot outlive the reason it was written for.
_R_SALINITY = ("CF's 1e-3 is a salinity scale (practical or absolute), not a "
               "mass fraction; the match would state a mass of salt CF does not")
_R_SALINITY_RATE = ("the match is the becquerel, an activity; this is a "
                    "salinity scale factor per second")
_R_SCALE = "a scale with an offset; the match is its unit, not the scale"
_R_CURRENCY = "a currency compound; the match drops the currency"
_R_ANNOTATION = "the ucumCode's annotation names the analyte; the match drops it"
_R_CONTRADICTS = "the publisher contradicts itself; the match follows one side"
_R_LOG_RATE = ("an information rate in a logarithmic base bovnar has no unit "
               "for; the match is the hertz")
_R_BIT_OCTET = "bit or octet is not settled; the match takes one"
_R_ROTATION = ("hertz and revolution per second are pooled; bovnar holds them "
               "2π apart")
REVIEWED_STALE = {
    **{("cf", c): _R_SALINITY for c in (
        "integral_wrt_depth_of_product_of_salinity_and_sea_water_density",
        "ocean_salt_x_transport", "ocean_salt_y_transport")},
    **{("cf", c): _R_SALINITY_RATE for c in (
        "tendency_of_sea_water_salinity",
        "tendency_of_sea_water_salinity_due_to_advection",
        "tendency_of_sea_water_salinity_due_to_horizontal_mixing",
        "tendency_of_sea_water_salinity_due_to_parameterized_eddy_advection",
        "tendency_of_sea_water_salinity_due_to_sea_ice_thermodynamics",
        "tendency_of_sea_water_salinity_due_to_vertical_mixing")},
    **{("om", c): _R_SCALE for c in (
        "CelsiusScale", "FahrenheitScale", "KelvinScale", "RankineScale",
        "ReaumurScale")},
    ("qudt-qk", "CelsiusTemperature"): _R_SCALE,
    **{("qudt", c): _R_CURRENCY for c in (
        "CCY_EUR-PER-KiloW-HR", "CCY_EUR-PER-M2", "CHF-PER-KiloGM")},
    **{("qudt-qk", c): _R_CURRENCY for c in (
        "CostPerArea", "CostPerMass", "CostPerEnergy", "CostPerPower")},
    **{("qudt", c): _R_ANNOTATION for c in (
        "GM_Carbon-PER-M2-DAY", "GM_Nitrogen-PER-M2-DAY", "M2-PER-GM_DRY")},
    ("qudt", "IU-PER-MilliGM"):
        "an international unit is assay-defined, not an amount of substance",
    **{("qudt", c): _R_CONTRADICTS for c in (
        "HP_Brake", "LA", "MI2", "MicroM-PER-MilliL")},
    **{("unece", c): _R_CONTRADICTS for c in ("M48", "MIK", "P30")},
    **{(ns, c): _R_LOG_RATE for ns, c in (
        ("qudt", "NAT-PER-SEC"), ("unece", "Q19"),
        ("qudt-qk", "InformationFlowRate"),
        ("qudt-qk", "LogarithmicMedianInformationFlow_SourceToBase10"),
        ("qudt-qk", "LogarithmicMedianInformationFlow_SourceToBaseE"))},
    **{("qudt-qk", c): _R_BIT_OCTET for c in (
        "BitRate", "BitTransmissionRate", "AreicDataVolume",
        "VolumicDataQuantity")},
    **{("qudt-qk", c): _R_ROTATION for c in (
        "RotationalFrequency", "RotationalVelocity")},
    ("udunits", "astronomical_unit_BIPM_2006"):
        "unreadable: the trailing digits scan as an exponent",
}
REVIEWED_STALE_SEEN = set()


# QUDT unit -> quantity kind links that QUDT itself has filed under the wrong
# kind, checked by check_quantity_kinds below. Same standing as WAIVED_UPSTREAM
# and reported the same way: a note on every run, never a silent skip.
# QUDT cross-references (its ucumCode / udunitsCode) that do not describe the
# unit they are attached to, checked by check_cross_reference below. Same
# standing as WAIVED_UPSTREAM and reported the same way. Keyed on
# (QUDT local name, the cross-referenced code).
WAIVED_XREF = {
    ("MicroGM-PER-CentiM2", "u[g].cm-2"):
        "[g] is UCUM's standard acceleration of free fall, and a METRIC atom "
        "(isMetric=\"yes\"), so u[g] is a micro-gn and the code is an "
        "acceleration per area. QUDT's label and multiplier are the microgram "
        "per square centimetre: a typo for ug.cm-2. It read as unreachable "
        "only while bovnar's table wrongly had [g] non-metric.",
    ("BIT-PER-SEC", "Bd"):
        "QUDT cross-references the bit per second to the BAUD. A baud is a "
        "SYMBOL rate and a bit per second an information rate; one symbol "
        "carries one bit only in a binary modulation. bovnar keeps Bd and b/s "
        "apart for that reason, and both rows are right.",
    ("BIT-PER-SEC", "bps"):
        "UDUNITS' bps is its baud — the same conflation as the ucumCode above.",
    ("MegaBIT-PER-SEC", "MBd"):
        "The megabaud, for the megabit per second: see BIT-PER-SEC ~ Bd.",
    ("TeraW-HR", "TW/h"):
        "A terawatt-HOUR cross-referenced to a terawatt PER hour. QUDT's own "
        "multiplier (3.6e15) is the energy, so the code is a typo for TW.h.",
    ("GM-PER-DEG_C", "d/Cel"):
        "'d' is the DAY. QUDT's multiplier (0.001) and its own label say gram "
        "per degree Celsius, so the code is a typo for g/Cel.",
    ("GM-PER-DEG_C", "d.Cel-1"):
        "The other spelling of the d/Cel typo: 'd' is the day.",
    ("DRAM_US", "fldr"):
        "UDUNITS' fldr is the FLUID dram, a volume. QUDT's own multiplier and "
        "dimension vector for DRAM_US are a mass (0.0038879346 kg, the "
        "apothecary dram), so the cross-reference names a different quantity "
        "entirely.",
    ("DeciM3-PER-MIN", "dm3.min-3"):
        "min-3 is the minute CUBED. QUDT's own multiplier (1.666667e-5) is a "
        "cubic decimetre per minute, so the exponent is a typo for -1.",
    ("DeciM3-PER-MIN", "dm3/min3"):
        "The same cubed minute as the other spelling of this code.",
    ("MilliL-PER-CentiM2-MIN", "mL.cm-2"):
        "The code has lost the minute. QUDT's own multiplier (1.666667e-4) "
        "carries it, so the row does too.",
    ("OHM-M2-PER-M", "Ohm2.m.m-1"):
        "Ω²·m/m is Ω². QUDT's dimension vector is the ohm-metre, so the code "
        "is a typo for Ohm.m2.m-1.",
    ("N-M-PER-KiloGM", "gp"):
        "UDUNITS' gp is the geopotential, an acceleration; the newton-metre "
        "per kilogram is a specific energy. The two differ by a length.",
    ("OZ_VOL_US", "fl oz"):
        "A SPACE MULTIPLIES in UDUNITS, and 'fl' is a femtolitre, so 'fl oz' "
        "reads as 1e-15 L times a fluid ounce. bovnar reproduces UDUNITS' own "
        "grammar; the code wanted UDUNITS' floz.",
    ("S_Ab", "GS"):
        "The gigasiemens is what the absiemens IS — this cross-reference is the "
        "evidence the row now follows. It disagrees only because QUDT's "
        "dimension vector for the unit does not; see the WAIVED_UPSTREAM entry.",
    # The thermal-conductivity pair: QUDT's UCUM form has lost the foot that
    # makes it a conductivity rather than a conductance per unit area. Both
    # spellings of the code carry the same loss.
    ("BTU_TH-FT-PER-FT2-HR-DEG_F", "[Btu_th].[ft_i]-2.h-1.[degF]-1"):
        "The code is a thermal conductance per unit area; QUDT's own multiplier "
        "(1.729577206) is the conductivity, i.e. that times a foot. The foot is "
        "missing from the code, not from the unit.",
    ("BTU_TH-FT-PER-FT2-HR-DEG_F", "[Btu_th]/([ft_i]2.h.[degF])"):
        "The same missing foot as the other spelling of this code.",
    # An affine scale inside a product. UCUM is an EXPRESSION profile, so `Cel`
    # translates as the atom it is — bovnar's °C — and a compound holding it has
    # no SI factor, exactly as the native °C/h has none (doc/11 §3.8). A flat
    # profile states the whole code and can say what it means, which is why the
    # QUDT rows read K. Neither is wrong; they are two grammars.
    ("CentiM-SEC-DEG_C", "cm.s.Cel-1"):
        "UCUM translates the atom Cel to the affine °C, and an affine scale in "
        "a product has no value (doc/11 §3.8); the flat QUDT code names the "
        "whole unit and so can read the interval, K. A grammar difference.",
    ("CentiM-SEC-DEG_C", "cm.s/Cel"):
        "The same grammar difference as the other spelling.",
    ("DEG_F-HR-FT2-PER-BTU_TH", "[degF].h-1.[ft_i]-2.[Btu_th]-1"):
        "As CentiM-SEC-DEG_C: UCUM's [degF] is the affine scale, the QUDT row "
        "reads the interval Δ°F. The code also inverts the hour and the square "
        "foot against QUDT's own multiplier.",
    ("DEG_F-HR-FT2-PER-BTU_TH", "[degF]/(h.[ft_i]2.[Btu_th])"):
        "The same as the other spelling of this code.",
    ("IU-PER-MilliGM", "[IU].mg-1"):
        "This is the cross-reference the row now follows: UCUM declares [IU] "
        "ARBITRARY, so the code is not commensurable with a molar amount and "
        "the QUDT row is refused. Kept here because the pair is still compared.",
    ("NP-PER-SEC", "Np.s-1"):
        "This cross-reference is the evidence the row now follows; it is "
        "listed so the pair keeps being compared rather than falling silent.",
    ("NP-PER-SEC", "Np/s"):
        "As the other spelling: the evidence for the fix, kept under comparison.",
}

WAIVED_KIND = {
    ("LM-PER-M2", "Luminance"):
        "A lumen per square metre is a LUX — illuminance, the luminous flux "
        "arriving on a surface. QUDT files it under Luminance (cd/m², the flux "
        "leaving one) while filing its own LUX under LuminousFluxPerArea, so "
        "the two spellings of one unit sit under two different kinds. Native "
        "lx is correct.",
    ("LM-PER-FT2", "Luminance"):
        "The same misfiling as LM-PER-M2, in square feet: lumens per unit area "
        "is illuminance, not luminance.",
    ("S_Ab", "ElectricConductivity"):
        "The same QUDT slip as the WAIVED_UPSTREAM entry for this unit, seen "
        "from the other side: having given the absiemens a conductivity's "
        "dimension vector, QUDT files it under ElectricConductivity. It is a "
        "conductance (10^9 S), which is what its own description and ucumCode "
        "say, so the kind link is wrong here rather than the unit.",
    ("KiloBYTE-PER-SEC", "DataRate"):
        "QUDT gives DataRate eight applicable units: seven bit rates and this "
        "one byte rate. bovnar carries bit and byte as separate kinds with no "
        "factor between them, so a kind cannot be both. The name and the "
        "seven-to-one majority settle it on the bit per second, and this one "
        "link is QUDT pooling what bovnar keeps apart — the same pooling that "
        "makes BitRate and RotationalFrequency unmappable.",
}


def zero():
    return [0] * NDIM


class Unresolved(Exception):
    pass


# ── the reference library, via the shipped ctypes bindings ──────────────────

def load_native():
    """(parse, si_factor, dims) bound to the built library, or None.

    Everything native goes through here rather than through a Python
    reimplementation of the unit grammar."""
    sys.path.insert(0, os.path.join(REPO, "python"))
    try:
        import bovnar
        from bovnar import units as bunits
        bovnar._ffi.load_library()
    except Exception:
        return None

    def resolve(text):
        """(factor, dims) for a native unit expression, or None if the library
        refuses it — an affine unit has no multiplicative factor, and that is an
        answer rather than a failure."""
        try:
            u = bovnar.parse_unit(text)
        except Exception:
            raise Unresolved("native parse failed: %r" % text)
        try:
            conv = bunits.unit_to_si_factor(u)
        except Exception:
            return None
        if conv.is_affine:
            return None
        try:
            d = bunits.unit_dimension_vector(u)
        except Exception:
            return None
        return (float(conv.factor), list(d))

    return resolve


# ── UDUNITS-2 ───────────────────────────────────────────────────────────────

UD_PFX = {
    'yotta': 24, 'zetta': 21, 'exa': 18, 'peta': 15, 'tera': 12, 'giga': 9,
    'mega': 6, 'kilo': 3, 'hecto': 2, 'deka': 1, 'deci': -1, 'centi': -2,
    'milli': -3, 'micro': -6, 'nano': -9, 'pico': -12, 'femto': -15,
    'atto': -18, 'zepto': -21, 'yocto': -24,
    'Y': 24, 'Z': 21, 'E': 18, 'P': 15, 'T': 12, 'G': 9, 'M': 6, 'k': 3,
    'h': 2, 'da': 1, 'd': -1, 'c': -2, 'm': -3, 'u': -6, 'µ': -6, 'n': -9,
    'p': -12, 'f': -15, 'a': -18, 'z': -21, 'y': -24,
}
UD_BASE = {
    'm':   (1.0, [1, 0, 0, 0, 0, 0, 0]),
    'kg':  (1.0, [0, 1, 0, 0, 0, 0, 0]),
    's':   (1.0, [0, 0, 1, 0, 0, 0, 0]),
    'A':   (1.0, [0, 0, 0, 1, 0, 0, 0]),
    'K':   (1.0, [0, 0, 0, 0, 1, 0, 0]),
    'mol': (1.0, [0, 0, 0, 0, 0, 1, 0]),
    'cd':  (1.0, [0, 0, 0, 0, 0, 0, 1]),
    'radian': (1.0, zero()), 'rad': (1.0, zero()), 'sr': (1.0, zero()),
}
_NUM = re.compile(r'\d+\.?\d*(?:[eE][-+]?\d+)?')


class Udunits:
    """UDUNITS-2's XML database. Definitions are strings like "6 US_survey_feet"
    or "kg.m2.s-3"; '.' and juxtaposition multiply, '/' divides."""

    name = "udunits"

    def __init__(self, cache):
        self.defs, self.real = {}, set()
        for fn in ("udunits2-base.xml", "udunits2-derived.xml",
                   "udunits2-accepted.xml", "udunits2-common.xml"):
            path = os.path.join(cache, fn)
            root = ET.parse(path).getroot()
            for u in root.findall("unit"):
                d = u.find("def")
                text = d.text.strip() if d is not None and d.text else None
                names = []
                for e in u.findall("symbol"):
                    if e.text:
                        names.append(e.text.strip())
                n = u.find("name")
                if n is not None:
                    for sub in ("singular", "plural"):
                        e = n.find(sub)
                        if e is not None and e.text:
                            names.append(e.text.strip())
                al = u.find("aliases")
                if al is not None:
                    for e in al.findall("symbol"):
                        if e.text:
                            names.append(e.text.strip())
                    for nm in al.findall("name"):
                        for sub in ("singular", "plural"):
                            e = nm.find(sub)
                            if e is not None and e.text:
                                names.append(e.text.strip())
                for x in names:
                    self.defs.setdefault(x, text if text is not None else "@BASE@")
                    self.real.add(x)
        # UDUNITS auto-pluralises unless <noplural/>, so the XML stores only the
        # singular and a def like "12 international_inches" names a spelling
        # that is never a key. Synthesise them -- but keep them OUT of `real`,
        # or every plural would be reported as an unmapped coverage gap.
        #
        # By UDUNITS' own rule (ut_form_plural), one plural per spelling, and
        # not "+s and +es for everything": that made `miles` the plural of the
        # MIL (mil + es, 2.54e-5 m) before the mile was reached, which went
        # unnoticed only while the table had no regular plurals to compare.
        def plural(n):
            if len(n) == 1:
                return n + "s"
            if n[-1] == "y":
                return n + "s" if n[-2] in "aeiou" else n[:-1] + "ies"
            if n[-1] in "sxz" or n.endswith(("ch", "sh")):
                return n + "es"
            return n + "s"
        for n in list(self.defs):
            self.defs.setdefault(plural(n), self.defs[n])
        plain = set(self.defs)
        names = set(plain)
        for nm in plain:
            for p in UD_PFX:
                names.add(p + nm)
        # Longest first: without prefixed spellings in the scan set "mm" reads
        # as m*m and "cm2" as c*m^2, both silently wrong by orders of magnitude.
        self._names = sorted(names, key=len, reverse=True)
        self._cache = {}

    def accepts(self, code):
        """Will UDUNITS take this spelling at all? Includes the auto-plurals,
        which are legal input even though the XML stores only the singular."""
        return code in self.defs or code in UD_BASE

    # The prefix spellings UDUNITS attaches to any unit name or symbol. Only
    # the ones that can produce a collision with a real code are needed here.
    _PREFIXES = ("da", "deci", "centi", "milli", "micro", "nano", "pico",
                 "femto", "atto", "kilo", "mega", "giga", "tera",
                 "d", "c", "m", "u", "n", "p", "f", "a", "k", "M", "G", "T",
                 "h", "E", "P", "Z", "Y")

    def prefixed_reading(self, code):
        """"<prefix> + <atom>" when UDUNITS' own language reaches `code` that
        way, else None.

        A code with no atom row is not automatically undefined: UDUNITS
        prefixes any name or symbol, so `dB` is deci + B and a real decibel.
        The same mechanism is what makes `nmi` a NANOMILE and `pH` a
        PICOHENRY, which is the case worth shouting about -- the atom-table
        walk cannot see either, and reported all three identically.
        """
        for pfx in sorted(self._PREFIXES, key=len, reverse=True):
            if code.startswith(pfx) and len(code) > len(pfx):
                rest = code[len(pfx):]
                if rest in self.real:
                    return "%s + %s" % (pfx, rest)
        return None

    def spellings(self):
        """Only what the XML states, so a coverage suggestion never proposes
        adding "meterses"."""
        return self.real

    def lookup(self, code, depth=0):
        if depth > 40:
            raise Unresolved(code)
        if code in UD_BASE:
            return UD_BASE[code]
        if code in self._cache:
            return self._cache[code]
        if code in self.defs and self.defs[code] != "@BASE@":
            r = self._eval(self.defs[code], depth + 1)
            self._cache[code] = r
            return r
        for p in sorted(UD_PFX, key=len, reverse=True):
            if code.startswith(p) and len(code) > len(p):
                rest = code[len(p):]
                if rest in self.defs or rest in UD_BASE:
                    f, d = self.lookup(rest, depth + 1)
                    return (f * 10.0 ** UD_PFX[p], d)
        raise Unresolved(code)

    def _scan(self, s):
        out, i, n = [], 0, len(s)
        while i < n:
            ch = s[i]
            if ch.isspace() or ch in '*·.':
                out.append('*')
                i += 1
                continue
            if ch in '/()':
                out.append(ch)
                i += 1
                continue
            if ch == '^':
                i += 1
                m = re.match(r'[-+]?\d+', s[i:])
                if not m:
                    raise Unresolved("bad exponent in %r" % s)
                out.append(('e', int(m.group(0))))
                i += m.end()
                continue
            hit = None
            for nm in self._names:
                if s.startswith(nm, i):
                    hit = nm
                    break
            if hit:
                out.append(('u', hit))
                i += len(hit)
                m2 = re.match(r'[-+]?\d+(?![\d.eE])', s[i:])
                if m2:
                    out.append(('e', int(m2.group(0))))
                    i += m2.end()
                continue
            m = _NUM.match(s, i)
            if m:
                out.append(('n', float(m.group(0))))
                i = m.end()
                continue
            raise Unresolved("char %r in %r" % (ch, s))
        return out

    def _eval(self, s, depth):
        toks = self._scan(s)
        pos = [0]

        def peek():
            return toks[pos[0]] if pos[0] < len(toks) else None

        def nxt():
            t = peek()
            pos[0] += 1
            return t

        def atom():
            t = nxt()
            if t == '(':
                f, d = expr()
                if peek() == ')':
                    nxt()
                return f, d
            if isinstance(t, tuple) and t[0] == 'n':
                return t[1], zero()
            if isinstance(t, tuple) and t[0] == 'u':
                if t[1] == 'pi':
                    import math
                    return math.pi, zero()
                return self.lookup(t[1], depth)
            raise Unresolved("token %r in %r" % (t, s))

        def power():
            f, d = atom()
            while isinstance(peek(), tuple) and peek()[0] == 'e':
                e = nxt()[1]
                f, d = f ** e, [x * e for x in d]
            return f, d

        def expr():
            f, d = power()
            while True:
                p = peek()
                if p == '*':
                    nxt()
                    g, e = power()
                    f *= g
                    d = [a + b for a, b in zip(d, e)]
                elif p == '/':
                    nxt()
                    g, e = power()
                    f /= g
                    d = [a - b for a, b in zip(d, e)]
                elif p is None or p == ')':
                    break
                else:
                    g, e = power()
                    f *= g
                    d = [a + b for a, b in zip(d, e)]
            return f, d

        return expr()

    def resolve(self, code):
        """(factor, dims) in coherent SI, or None when there is nothing to
        compare -- UDUNITS' affine and logarithmic units have no factor."""
        try:
            return self.lookup(code)
        except Unresolved:
            return None
        except RecursionError:
            return None


# ── UCUM ────────────────────────────────────────────────────────────────────

UCUM_NS = {'u': 'http://unitsofmeasure.org/ucum-essence'}
# UCUM's own base dimensions onto bovnar's vector. A (plane angle) is dropped:
# bovnar's radian is dimensionless. Q (charge) is current AND time, which is
# what makes C/s come out as a bare current.
UCUM_DIM = {
    'L': [(LENGTH, 1)],
    'M': [(MASS, 1)],
    'T': [(TIME, 1)],
    'C': [(TEMP, 1)],
    'F': [(LUM, 1)],
    'Q': [(CURRENT, 1), (TIME, 1)],
    'A': [],
}


class Ucum:
    name = "ucum"

    def __init__(self, cache):
        root = ET.parse(os.path.join(cache, "ucum-essence.xml")).getroot()
        self.base, self.defs, self.skip = {}, {}, {}
        for e in root.findall('u:base-unit', UCUM_NS):
            d = zero()
            for idx, v in UCUM_DIM.get(e.get('dim'), []):
                d[idx] += v
            self.base[e.get('Code')] = (1.0, d)
        self.pfx = {e.get('Code'): float(e.find('u:value', UCUM_NS).get('value'))
                    for e in root.findall('u:prefix', UCUM_NS)}
        for e in root.findall('u:unit', UCUM_NS):
            code = e.get('Code')
            v = e.find('u:value', UCUM_NS)
            # A special unit is a function (dB, Cel, prism dioptre) and an
            # arbitrary one is assay-defined; neither has a factor to check, and
            # both are exactly what bovnar refuses or carries as opaque.
            if e.get('isSpecial') == 'yes':
                self.skip[code] = 'special'
                continue
            if e.get('isArbitrary') == 'yes':
                self.skip[code] = 'arbitrary'
                continue
            if v is None:
                self.skip[code] = 'no value'
                continue
            self.defs[code] = (v.get('value'), v.get('Unit'))
        known = set(self.defs) | set(self.base)
        names = set(known)
        for nm in known:
            for p in self.pfx:
                names.add(p + nm)
        # Longest-known-code first, or "m[Hg]" scans as m * [Hg] and [Hg] is not
        # an atom at all.
        self._names = sorted(names, key=len, reverse=True)
        self._cache = {}
        self._na = None

    def accepts(self, code):
        """Does UCUM define this atom at all? A special (dB, Cel) or arbitrary
        ([IU]) unit is defined -- it simply has no multiplicative value -- so it
        must count as existing or every one would be reported as a dead row."""
        return code in self.defs or code in self.base or code in self.skip

    def spellings(self):
        """Only atoms with a comparable value; the special and arbitrary ones
        have nothing to offer a coverage suggestion."""
        return set(self.defs) | set(self.base)

    def avogadro(self):
        """UCUM's OWN value of N_A, so the mole correction cancels exactly."""
        if self._na is None:
            self._na = self.lookup('mol')[0]
        return self._na

    def lookup(self, code, depth=0):
        if depth > 40:
            raise Unresolved(code)
        if code in self.base:
            return self.base[code]
        if code in self._cache:
            return self._cache[code]
        if code in self.defs:
            val, unit = self.defs[code]
            f, d = self._eval(unit, depth + 1)
            r = (float(val) * f if val else f, d)
            self._cache[code] = r
            return r
        for p in sorted(self.pfx, key=len, reverse=True):
            if code.startswith(p) and len(code) > len(p):
                rest = code[len(p):]
                if rest in self.defs or rest in self.base:
                    f, d = self.lookup(rest, depth + 1)
                    return (f * self.pfx[p], d)
        raise Unresolved(code)

    def _eval(self, s, depth=0):
        f, d, sign, i, n = 1.0, zero(), 1, 0, len(s)
        while i < n:
            ch = s[i]
            if ch == '.':
                sign = 1
                i += 1
                continue
            if ch == '/':
                sign = -1
                i += 1
                continue
            if ch == '(':
                j, lvl = i + 1, 1
                while j < n and lvl:
                    if s[j] == '(':
                        lvl += 1
                    elif s[j] == ')':
                        lvl -= 1
                    j += 1
                sf, sd = self._eval(s[i + 1:j - 1], depth + 1)
                f *= sf ** sign
                d = [a + sign * b for a, b in zip(d, sd)]
                i = j
                continue
            m = re.match(r'10[*^]([+-]?\d+)', s[i:])
            if m:
                f *= (10.0 ** int(m.group(1))) ** sign
                i += m.end()
                continue
            hit = None
            for nm in self._names:
                if s.startswith(nm, i):
                    hit = nm
                    break
            if hit:
                i += len(hit)
                e2 = re.match(r'[+-]?\d+', s[i:])
                exp = 1
                if e2:
                    exp = int(e2.group(0))
                    i += e2.end()
                bf, bd = self.lookup(hit, depth)
                f *= (bf ** exp) ** sign
                d = [a + sign * exp * b for a, b in zip(d, bd)]
                continue
            m = _NUM.match(s, i)
            if m:
                f *= float(m.group(0)) ** sign
                i = m.end()
                continue
            raise Unresolved("char %r in %r" % (ch, s))
        return f, d

    def resolve(self, code):
        try:
            return self.lookup(code)
        except Unresolved:
            return None
        except RecursionError:
            return None

    def normalise(self, up, native_dims):
        """UCUM's system is not SI's. Bring a UCUM (factor, dims) onto bovnar's
        terms, given what the native side says the dimensions are.

        The mass base is the gram, so a factor carries 1000^(mass exponent) that
        bovnar's does not. And UCUM has no amount base -- the mole is Avogadro's
        NUMBER -- so a factor also carries N_A^(amount exponent). Both are read
        off the NATIVE dimension vector, because that is the side that has an
        amount dimension at all."""
        f, d = up
        f = f / (1000.0 ** native_dims[MASS])
        if native_dims[AMOUNT]:
            f = f / (self.avogadro() ** native_dims[AMOUNT])
        d = list(d)
        d[AMOUNT] = native_dims[AMOUNT]      # UCUM cannot express it; trust native
        return f, d


# ── QUDT ────────────────────────────────────────────────────────────────────

# QUDT states a dimension vector as an IRI local name, "A0E0L1I0M0H0T0D0", and
# the letters are its own: A is amount of substance, E electric current, H
# thermodynamic temperature, I luminous intensity, D a dimensionless marker this
# comparison ignores.
QKDV = re.compile(r'A(-?[\d.]+)E(-?[\d.]+)L(-?[\d.]+)I(-?[\d.]+)'
                  r'M(-?[\d.]+)H(-?[\d.]+)T(-?[\d.]+)D(-?[\d.]+)')
QKDV_SLOT = {'L': LENGTH, 'M': MASS, 'T': TIME,
             'E': CURRENT, 'H': TEMP, 'A': AMOUNT, 'I': LUM}


def parse_qkdv(local):
    """The dimension IRI's local name -> bovnar's vector, or None if any
    exponent is fractional (QUDT has a few; bovnar's are integers)."""
    m = QKDV.fullmatch(local)
    if not m:
        return None
    vals = dict(zip("AELIMHTD", m.groups()))
    d = zero()
    for letter, slot in QKDV_SLOT.items():
        v = float(vals[letter])
        if v != int(v):
            return None
        d[slot] = int(v)
    return d


def parse_ttl(path, subject_prefix):
    """Subject local name -> {predicate local name: [values]}.

    A hand-rolled scan rather than rdflib, which is not a dependency this repo
    has. It is safe on exactly the shape QUDT publishes -- one subject per
    block, starting in column 0, properties indented, terminated by a lone '.'
    -- and it reads only three literal-valued predicates, so it never has to
    understand a quoted string containing a semicolon.
    """
    out, subj, props = {}, None, None
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith(subject_prefix + ":"):
                if subj:
                    out[subj] = props
                subj = line.strip().split()[0][len(subject_prefix) + 1:]
                props = {}
                continue
            if subj is None:
                continue
            t = line.strip()
            if t == ".":
                out[subj] = props
                subj, props = None, None
                continue
            m = re.match(r'(?:qudt):(\w+)\s+(.+?)\s*[;.]?$', t)
            if m:
                props.setdefault(m.group(1), []).append(m.group(2))
    if subj:
        out[subj] = props
    return out


def _num(vals):
    if not vals:
        return None
    try:
        return float(vals[0].split('^^')[0].strip('"'))
    except ValueError:
        return None


class Qudt:
    """QUDT unit local names. Nothing to evaluate: each unit states its own
    conversionMultiplier and dimension vector, so this is a table read rather
    than an expression parser."""

    name = "qudt"

    def __init__(self, cache):
        self.rows = parse_ttl(os.path.join(cache, "qudt-units.ttl"), "unit")

    def accepts(self, code):
        return code in self.rows

    def spellings(self):
        return set(self.rows)

    def resolve(self, code):
        p = self.rows.get(code)
        if p is None:
            return None
        # An affine unit has no multiplicative factor; the native side returns
        # None for the same reason, so both drop out of the comparison.
        if _num(p.get("conversionOffset")):
            return None
        mult = _num(p.get("conversionMultiplier"))
        dv = p.get("hasDimensionVector")
        if mult is None or not dv:
            return None
        d = parse_qkdv(dv[0].split(":")[-1])
        if d is None:
            return None
        return (mult, d)


class QudtQk(Qudt):
    """QUDT quantity kinds.

    A kind has no scale, so there is no multiplier to compare. What CAN be
    checked is the thing doc/11 §12.3 actually claims: that the code maps to the
    COHERENT SI unit of the kind. That is two assertions -- the dimensions
    agree, and the native target's factor is exactly 1 -- and both fall out of
    the ordinary comparison once a kind is reported as (1.0, its dimensions).
    A row mapped to a non-coherent unit (`FT` for Length, say) fails on the
    factor even though its dimensions are perfect.
    """

    name = "qudt-qk"

    def __init__(self, cache):
        self.rows = parse_ttl(os.path.join(cache, "qudt-quantitykinds.ttl"),
                              "quantitykind")

    def resolve(self, code):
        p = self.rows.get(code)
        if p is None:
            return None
        dv = p.get("hasDimensionVector")
        if not dv:
            return None
        d = parse_qkdv(dv[0].split(":")[-1])
        if d is None:
            return None
        return (1.0, d)


class Unece(Qudt):
    """UN/ECE Rec 20, checked THROUGH QUDT rather than against Rec 20 itself.

    THIS IS A SECONDARY SOURCE and everything below follows from that. Rec 20
    publishes a code list whose conversion factors are prose, so there is no
    primary artefact to resolve. QUDT carries a `qudt:uneceCommonCode` on many
    of its units, which gives a machine-readable UNECE-code-to-value map at one
    remove — it is QUDT asserting what a Rec 20 code means, not UN/ECE. A
    disagreement found here is evidence that one of the two tables is wrong, not
    proof of which.

    Two consequences, both of them about not overclaiming:

      * `accepts` is ALWAYS TRUE. A code QUDT does not mention is not thereby
        absent from Rec 20 — QUDT simply has no unit carrying it. Reporting such
        a row as "dead" would be asserting something this source cannot support,
        so the dead check is switched off for this vocabulary entirely and the
        uncovered rows are counted separately instead.
      * A code SEVERAL QUDT units claim is only usable when they agree. 81 codes
        have more than one claimant; most are aliases of one unit, but some are
        not — J62 is claimed by both a barrels-per-hour and a barrels-per-second
        unit, which differ by 3600. Where the claimants disagree the cross-
        reference cannot say what the code means, and the row goes unchecked.
    """

    name = "unece"
    secondary = True

    def __init__(self, cache):
        Qudt.__init__(self, cache)
        claims = {}
        for local, p in self.rows.items():
            for v in p.get("uneceCommonCode", []):
                code = v.split("^^")[0].strip('"').strip()
                claims.setdefault(code, []).append(local)
        self._val, self.ambiguous = {}, set()
        for code, locals_ in claims.items():
            vals = [Qudt.resolve(self, l) for l in locals_]
            vals = [v for v in vals if v is not None]
            if not vals:
                continue
            f0, d0 = vals[0]
            if all(d == d0 and close(f, f0) for f, d in vals):
                self._val[code] = (f0, d0)
            else:
                self.ambiguous.add(code)

    def accepts(self, code):
        return True          # see the class docstring: cannot disprove absence

    def spellings(self):
        return set(self._val)

    def resolve(self, code):
        return self._val.get(code)


# ── OM 2 ────────────────────────────────────────────────────────────────────

OM_NS = "{http://www.ontology-of-units-of-measure.org/resource/om-2/}"
OM_RDF = "{http://www.w3.org/1999/02/22-rdf-syntax-ns#}"

# OM states a Dimension individual's exponents one property at a time.
OM_DIMPROPS = (
    ("hasSILengthDimensionExponent", LENGTH),
    ("hasSIMassDimensionExponent", MASS),
    ("hasSITimeDimensionExponent", TIME),
    ("hasSIElectricCurrentDimensionExponent", CURRENT),
    ("hasSIThermodynamicTemperatureDimensionExponent", TEMP),
    ("hasSIAmountOfSubstanceDimensionExponent", AMOUNT),
    ("hasSILuminousIntensityDimensionExponent", LUM),
)

# The seven SI base units, pinned. OM defines the gram as 1e-3 kilogram and the
# kilogram as kilo x gram, so the mass chain is a cycle and has to be cut
# somewhere; cutting it at the coherent SI base is the convention bovnar's own
# factors use.
OM_BASE = {
    "metre": LENGTH, "kilogram": MASS, "second-Time": TIME, "ampere": CURRENT,
    "kelvin": TEMP, "mole": AMOUNT, "candela": LUM,
}


class Om:
    """OM 2's OWL/RDF file, resolved by COMPOSITION.

    Unlike QUDT, OM does not state a multiplier per unit. It states how the unit
    is BUILT -- a PrefixedUnit's prefix and base, a UnitDivision's numerator and
    denominator, a UnitMultiplication's two terms, a UnitExponentiation's base
    and exponent, a SingularUnit's factor against the unit it is defined from --
    and a value falls out of walking that structure down to the SI base units.
    So this resolver is a small evaluator, like the UCUM and UDUNITS ones and
    unlike the QUDT table read.

    A named unit states its definition as `hasUnit` with NO factor: the joule IS
    the newton metre, the becquerel IS the reciprocal second. That case is what
    makes the coherent derived units resolve to 1 without a list of them here.

    A unit OM gives a dimension and no definition at all -- calorie-15C,
    InternationalUnit, the currencies -- resolves to None and is skipped, which
    is right: those are exactly the rows om.bvnr refuses.
    """

    name = "om"

    def __init__(self, cache):
        root = ET.parse(os.path.join(cache, "om-2.0.rdf")).getroot()
        self.props = {}
        for e in root:
            about = e.get(OM_RDF + "about")
            if not about:
                continue
            local = about.rsplit("/", 1)[-1]
            # A subject appears in several elements; merge rather than replace.
            p = self.props.setdefault(local, {})
            for c in e:
                tag = c.tag.split("}")[-1]
                res = c.get(OM_RDF + "resource")
                if tag == "type":
                    continue
                val = res.rsplit("/", 1)[-1] if res else (c.text or "").strip()
                p.setdefault(tag, []).append(val)
        self.dims = {}
        for local, p in self.props.items():
            if any(k in p for k, _ in OM_DIMPROPS):
                d = zero()
                for k, slot in OM_DIMPROPS:
                    d[slot] = int(p[k][0]) if k in p else 0
                self.dims[local] = d
        self._cache = {}

    @staticmethod
    def _one(p, key):
        v = p.get(key)
        return v[0] if v else None

    def accepts(self, code):
        """Is this local name a UNIT of OM's?

        Recognised by what the individual states, not by its rdf:type: OM has a
        class per prefixed shape (SquarePrefixedMetre, MolePerPrefixedMetre, a
        dozen more), so a type list would miss the ~280 individuals carrying
        one. A Dimension states its own exponents and a prefix states a factor
        with no unit to apply it to, so both are excluded."""
        p = self.props.get(code)
        if p is None:
            return False
        if any(k in p for k, _ in OM_DIMPROPS):
            return False
        if "hasFactor" in p and "hasUnit" not in p:
            return False
        return ("hasDimension" in p or "hasUnit" in p or "hasNumerator" in p or
                "hasTerm1" in p or "hasBase" in p or "symbol" in p)

    def spellings(self):
        return {c for c in self.props if self.accepts(c)}

    def resolve(self, code, depth=0):
        if code in self._cache:
            return self._cache[code]
        if depth > 12:
            return None
        self._cache[code] = None            # cycle guard
        r = self._resolve(code, depth)
        self._cache[code] = r
        return r

    def _resolve(self, code, depth):
        if code in OM_BASE:
            d = zero()
            d[OM_BASE[code]] = 1
            return (1.0, d)
        p = self.props.get(code)
        if p is None:
            return None
        one = self._one
        if "hasPrefix" in p and "hasUnit" in p:
            f = one(self.props.get(one(p, "hasPrefix"), {}), "hasFactor")
            base = self.resolve(one(p, "hasUnit"), depth + 1)
            if f is None or base is None:
                return None
            return (float(f) * base[0], base[1])
        if "hasNumerator" in p and "hasDenominator" in p:
            a = self.resolve(one(p, "hasNumerator"), depth + 1)
            b = self.resolve(one(p, "hasDenominator"), depth + 1)
            if a is None or b is None or b[0] == 0.0:
                return None
            return (a[0] / b[0], [x - y for x, y in zip(a[1], b[1])])
        if "hasTerm1" in p and "hasTerm2" in p:
            a = self.resolve(one(p, "hasTerm1"), depth + 1)
            b = self.resolve(one(p, "hasTerm2"), depth + 1)
            if a is None or b is None:
                return None
            return (a[0] * b[0], [x + y for x, y in zip(a[1], b[1])])
        if "hasBase" in p and "hasExponent" in p:
            a = self.resolve(one(p, "hasBase"), depth + 1)
            n = one(p, "hasExponent")
            if a is None or n is None:
                return None
            n = int(float(n))
            return (a[0] ** n, [x * n for x in a[1]])
        if "hasFactor" in p and "hasUnit" in p:
            a = self.resolve(one(p, "hasUnit"), depth + 1)
            f = one(p, "hasFactor")
            if a is None or f is None:
                return None
            return (float(f) * a[0], a[1])
        if "hasUnit" in p:
            return self.resolve(one(p, "hasUnit"), depth + 1)
        return None


# ── CF standard names ───────────────────────────────────────────────────────

class Cf:
    """The CF standard name table, resolved THROUGH UDUNITS.

    A standard name has no factor of its own. What it has is a
    `canonical_units` field, and that field is a UDUNITS expression -- CF's unit
    syntax is UDUNITS -- so the check is: evaluate the name's canonical_units
    with the same evaluator the udunits profile is checked against, and compare
    it with what the library says cf.bvnr's target is worth.

    Both sides are primary. This is not the one-remove position `unece` is in:
    CF publishes the name-to-units mapping itself, and Unidata publishes what
    the units are worth, so a disagreement found here is a defect in cf.bvnr
    rather than evidence about a third party's cross-reference.

    Three canonical_units strings resolve to None and are skipped, each for a
    reason the udunits profile already has: `degree_C` is affine, `dB` is
    logarithmic, and `W m-2 sr-1 (m-1)-1` uses a parenthesised negative exponent
    the UDUNITS evaluator here does not implement. The first two have no factor
    to compare in any case.

    ALIASES ARE NOT SPELLINGS. CF's deprecated names live in `<alias>` elements
    and cf.bvnr deliberately carries none of them, so they are kept out of
    `spellings()` too: reporting 599 absent aliases as a coverage gap would be
    proposing exactly what that file refuses to do.
    """

    name = "cf"

    def __init__(self, cache):
        self.ud = Udunits(cache)
        root = ET.parse(os.path.join(cache,
                                     "cf-standard-name-table.xml")).getroot()
        self.rows = {}
        for e in root.findall("entry"):
            self.rows[e.get("id")] = (e.findtext("canonical_units") or "").strip()
        self.version = (root.findtext("version_number") or "?").strip()
        self._cache = {}

    def accepts(self, code):
        return code in self.rows

    def spellings(self):
        return set(self.rows)

    def resolve(self, code):
        cu = self.rows.get(code)
        if not cu:
            return None
        if cu not in self._cache:
            try:
                self._cache[cu] = self.ud._eval(cu, 0)
            except (Unresolved, ValueError, RecursionError, ZeroDivisionError):
                self._cache[cu] = None
        return self._cache[cu]


VOCABS = {"udunits": Udunits, "ucum": Ucum, "qudt": Qudt, "qudt-qk": QudtQk,
          "unece": Unece, "om": Om, "cf": Cf}


# ── the comparison ──────────────────────────────────────────────────────────

def close(a, b, tol=TOL):
    if a == b:
        return True
    if b == 0.0:
        return abs(a) < tol
    return abs(a - b) / abs(b) < tol


class NativeIndex:
    """Every native unit's (factor, dims), so a publisher's code can be matched
    by VALUE rather than by spelling.

    Built from src/gendata/units.bvnr through the reference library, one entry
    per canonical symbol. Units sharing a dimension and a factor -- Hz and Bq,
    Gy and Sv -- collapse onto whichever comes first, which is why a suggestion
    is advisory: it says "this code is worth what some native unit is worth",
    not "map it to that one".

    THE BARE SYMBOLS ARE NOT ENOUGH, and this is the half that matters for
    keeping five tables in step. A flat vocabulary spells a PREFIXED or COMPOUND
    unit as one whole token -- unece:A97 is the hectopascal, unece:KMQ the
    kilogram per cubic metre, qudt:RAD-PER-SEC the radian per second -- and none
    of those is a native symbol, so an index of symbols alone can never propose
    them. It found nothing for exactly the codes that a table is most likely to
    be missing while its neighbours carry them.

    So every `.bovnar` target any of the five profile tables already uses is
    indexed too. That is the right source: a target one table has written down
    is a spelling this build is known to accept, and a code from another
    vocabulary worth the same thing is a synchronisation gap by construction.
    Symbols come first, so a code that is worth a bare native unit is still
    reported against that unit rather than against some compound equal to it.
    """

    def __init__(self, native):
        self.rows = []
        seen = set()

        def add(expr):
            if expr in seen:
                return
            seen.add(expr)
            try:
                r = native(expr)
            except Unresolved:
                return
            if r is None:
                return
            self.rows.append((expr, r[0], list(r[1])))

        doc = bvnr_data.load(
            open(os.path.join(GENDATA, "units.bvnr"), "rb").read())
        for u in doc["units"]:
            add(u["symbol"])
        targets = set()
        for other in sorted(VOCABS):
            d = bvnr_data.load(
                open(os.path.join(GENDATA, other + ".bvnr"), "rb").read())
            targets.update(m["bovnar"] for m in d.get("mapped", []))
        for expr in sorted(targets):
            add(expr)

    def match(self, vocab, up):
        for sym, nf, nd in self.rows:
            uf, ud = (vocab.normalise(up, nd)
                      if hasattr(vocab, "normalise") else up)
            if list(ud) == nd and close(uf, nf):
                return (sym, nf)
        return None


def check_codes_read_back(verbose):
    """Every code the tables declare must be READABLE through its own profile.

    The one direction nothing checked. check_targets_parse (gen_profiles.py)
    proves a row's `.bovnar` TARGET parses; check_profile below proves the
    resulting factor matches the publisher. Neither asks whether the `.code`
    itself survives the grammar it belongs to, and in an EXPRESSION profile it
    may not: a trailing run of digits is an exponent there, so "m2" is m² and
    "m100" is m¹⁰⁰. `udunits:astronomical_unit_BIPM_2006` sat in .mapped for
    exactly that reason -- the scanner took "2006" as the exponent, asked the
    atom table for "astronomical_unit_BIPM_", and refused the whole token. The
    row was dead, and its 27 bytes had pushed BVNR_UNIT_STRING_MAX from 1024 to
    1088 on the way in.

    Stated as a round trip against the built library rather than as a rule about
    digits, because the failure is "the grammar cannot reach this row" and a
    restatement of the grammar here would drift from it. A code that resolves to
    the WRONG unit is caught by the same comparison, which a shape rule would
    miss entirely.

    Needs no publisher data, so it runs whether or not the cache is present.
    """
    sys.path.insert(0, os.path.join(REPO, "python"))
    try:
        import bovnar
        from bovnar.enums import ErrorCode
        bovnar._ffi.load_library()
    except Exception:
        return None, 0

    unsupported = int(ErrorCode.UNIT_PROFILE_UNSUPPORTED)
    problems = []
    checked = 0
    for ns in sorted(VOCABS):
        path = os.path.join(GENDATA, ns + ".bvnr")
        if not os.path.exists(path):
            continue
        doc = bvnr_data.load(open(path, "rb").read().decode("utf-8"))

        for rec in doc.get("mapped", []):
            checked += 1
            qualified = "%s:%s" % (ns, rec["code"])
            try:
                got = bovnar.parse_unit(qualified)
            except Exception:
                problems.append(
                    "%s is declared .mapped -> %r but the profile cannot READ "
                    "it; in an expression grammar a code ending in digits is "
                    "unreachable (the digits scan as an exponent)"
                    % (qualified, rec["bovnar"]))
                continue
            try:
                want = bovnar.parse_unit(rec["bovnar"])
            except Exception:
                problems.append("%s declares the target %r, which does not parse"
                                % (qualified, rec["bovnar"]))
                continue
            if bovnar.unit_to_str(got) != bovnar.unit_to_str(want):
                problems.append(
                    "%s reads back as %s, but the row declares %s (%r)"
                    % (qualified, bovnar.unit_to_str(got),
                       bovnar.unit_to_str(want), rec["bovnar"]))

        for rec in doc.get("opaque", []):
            checked += 1
            qualified = "%s:%s" % (ns, rec["code"])
            try:
                bovnar.parse_unit(qualified)
            except Exception:
                problems.append(
                    "%s is declared .opaque but the profile cannot read it"
                    % qualified)

        # An alias must read as its target, the same unit and not a lookalike.
        for rec in doc.get("opaque_aliases", []):
            checked += 1
            qualified = "%s:%s" % (ns, rec["code"])
            target = "%s:%s" % (ns, rec["of"])
            try:
                got = bovnar.parse_unit(qualified)
                want = bovnar.parse_unit(target)
            except Exception:
                problems.append(
                    "%s is declared an alias of %s but does not read"
                    % (qualified, target))
                continue
            if not bovnar.units_equal(got, want):
                problems.append("%s is declared an alias of %s but reads as %s"
                                % (qualified, target,
                                   bovnar.unit_to_str(got)))

        # A named refusal must arrive as "known, and not carryable" rather than
        # as "that is not a code" -- the distinction the whole .unsupported list
        # exists to make, and one no test had ever exercised at scale.
        for rec in doc.get("unsupported", []):
            checked += 1
            qualified = "%s:%s" % (ns, rec["code"])
            code = bovnar.unit_error_code(qualified)
            if int(code) != unsupported:
                problems.append(
                    "%s is a named refusal, so it must report "
                    "unit_profile_unsupported; it reports %s"
                    % (qualified, getattr(code, "name", code)))

    if verbose:
        print("  read-back: %d codes checked, %d problem(s)"
              % (checked, len(problems)))
    return problems, checked


def check_doc_error_claims(verbose):
    """Every "`code` … error_unit_*" claim in the profile docs, against the reader.

    doc/11 §6.4 tabulates what UCUM codes do — refused, profile-only, or not a
    code at all — and doc/11 §6.3 explains the near-misses in prose. Both are
    hand-maintained copies of what `src/gendata/ucum.bvnr` decides, and both went
    stale the moment the registry grew: `[Btu]`, `[Btu_th]`, `cal_IT`, `[ch_us]`,
    `[acr_us]`, `[dr_ap]` and `[lb_ap]` were listed as
    `error_unit_profile_unsupported` long after native units arrived that let
    them map, and `[ch_br]`/`[ft_br]`/`[yd_br]` were listed as
    `error_unit_illegal` after they became named refusals.

    None of that broke anything — the docs understated the profile, which is the
    direction no test notices, because nothing fails when a capability goes
    unclaimed. That is precisely why it needs a gate rather than a reading.

    The claims are extracted rather than restated here: a backticked token
    followed by an `error_unit_*` code on the same line, in a table row or a
    sentence, with the namespace taken from the surrounding section. Restating
    them would create a third copy to drift."""
    sys.path.insert(0, os.path.join(REPO, "python"))
    try:
        import bovnar
        from bovnar.enums import ErrorCode
        bovnar._ffi.load_library()
    except Exception:
        return None, 0

    path = os.path.join(REPO, "doc", "11_bovnar_unit_profiles.md")
    if not os.path.exists(path):
        return [], 0

    # A code is only meaningful inside its namespace, and §6.3/§6.4 are UCUM's.
    section = re.compile(r"^#{2,4}\s+(\d+)")
    err = re.compile(r"error_unit_(?:illegal|profile_unsupported|profile_unknown)")
    tick = re.compile(r"`([^`\n]+)`")

    def claims(line):
        """(token, expected) pairs. A TABLE ROW states one outcome for a whole
        list of examples, so every backticked token in the example cells is a
        claim -- taking only the one nearest the error code checked a tenth of
        the table and is how the stale rows survived a reading."""
        if line.lstrip().startswith("|") and line.count("|") >= 3:
            cells = [c.strip() for c in line.strip().strip("|").split("|")]
            outcome = next((c for c in reversed(cells) if err.search(c)), None)
            if not outcome:
                return
            want = err.search(outcome).group(0)
            for cell in cells:
                if err.search(cell):
                    continue          # the outcome cell itself
                for t in tick.findall(cell):
                    yield t, want
            return
        m = err.search(line)
        if not m:
            return
        toks = tick.findall(line[:m.start()])
        if toks:
            yield toks[-1], m.group(0)

    problems = []
    checked = 0
    in_ucum = False
    for line in open(path, encoding="utf-8"):
        m = section.match(line)
        if m:
            in_ucum = m.group(1) == "6"
        if not in_ucum:
            continue
        for tok, want in claims(line):
            # Only bracket/word atoms; skip prose, identifiers and macros.
            if not re.fullmatch(r"[\[\]A-Za-z0-9_'./*%^()-]{1,32}", tok):
                continue
            if tok.isupper() and "_" in tok and not tok.startswith("["):
                continue          # BVNR_MAX_UNIT_COMPONENTS and friends
            checked += 1
            got = "error_" + ErrorCode(
                bovnar.unit_error_code("ucum:" + tok)).name.lower()
            if got != want:
                extra = ""
                if got == "error_none":
                    try:
                        u = bovnar.parse_unit("ucum:" + tok)
                        extra = (" (profile-only)" if bovnar.unit_is_profile_only(u)
                                 else " (it maps to %s)" % bovnar.unit_to_str(u))
                    except Exception:
                        pass
                problems.append(
                    "doc/11 §6: `ucum:%s` is documented as %s; the reader says "
                    "%s%s" % (tok, want, got, extra))
    if verbose:
        print("  doc/11 §6: %d error claim(s) checked, %d problem(s)"
              % (checked, len(problems)))
    return problems, checked


def check_doc_profile_spellings(verbose):
    """doc/11 §5.3's spelling table, and its "nowhere to go" list, against the
    writer they describe.

    Two hand-maintained claims about `bvn_unit_to_profile`, and nothing compared
    either to it. Both had gone STALE IN THE SAFE-LOOKING DIRECTION -- they
    understated what the writer can do, which no test notices because nothing
    fails when a capability is unclaimed:

      * the table showed `Mi~B` with no UN/ECE spelling (it is `E63`) and `mph`
        with none anywhere (UN/ECE `HM`, QUDT `MI-PER-HR`);
      * the prose listed `mph` among the units that "have nowhere to go", beside
        `kph` and `rpm`, which genuinely have none.

    A reader takes those as the answer and writes their own translation, or
    concludes the profile is thinner than it is. Checked here rather than in
    check_doc_unit_tables.py because that tool compares doc/04 and doc/05 to
    src/gendata, and this is a different question: what the WRITER emits, which
    only the built library can answer."""
    sys.path.insert(0, os.path.join(REPO, "python"))
    try:
        import bovnar
        bovnar._ffi.load_library()
    except Exception:
        return None, 0

    path = os.path.join(REPO, "doc", "11_bovnar_unit_profiles.md")
    if not os.path.exists(path):
        return [], 0
    text = open(path, encoding="utf-8").read()
    problems = []
    checked = 0

    def emitted(ns, unit):
        try:
            return bovnar.unit_to_profile(ns, bovnar.parse_unit(unit))
        except Exception:
            return None

    # --- the table: | Native | `ucum` | `unece` | `qudt` | `udunits` | -------
    m = re.search(r"\|\s*Native\s*\|\s*`ucum`\s*\|\s*`unece`\s*\|\s*`qudt`\s*\|"
                  r"\s*`udunits`\s*\|\n\|[-\s|]+\|\n((?:\|.*\|\n)+)", text)
    if not m:
        problems.append("doc/11: the §5.3 spelling table is not where this "
                        "check expects it — it cannot be verified")
    else:
        cols = ["ucum", "unece", "qudt", "udunits"]
        for row in m.group(1).strip().splitlines():
            cells = [c.strip() for c in row.strip().strip("|").split("|")]
            if len(cells) != 5:
                continue
            native = cells[0].strip("`")
            if native.startswith("*"):          # the "(dimensionless)" row
                continue
            for ns, cell in zip(cols, cells[1:]):
                checked += 1
                want = None if cell in ("—", "-", "") else cell.strip("`")
                got = emitted(ns, native)
                if got != want:
                    problems.append(
                        "doc/11 §5.3: the table says %s writes as %s in `%s`; "
                        "the writer emits %s"
                        % (native, "—" if want is None else repr(want),
                           ns, "nothing" if got is None else repr(got)))

    # --- the prose: units said to have nowhere to go -------------------------
    m = re.search(r"\*No table is complete\.\*(.+?)\n-\s", text, re.S)
    if m:
        sentence = m.group(1)
        # Only the run before the per-unit caveat, which names units that DO
        # have spellings in order to make the point that membership varies.
        sentence = sentence.split("Membership here")[0]
        for unit in re.findall(r"`([^`]+)`", sentence):
            checked += 1
            have = {ns: emitted(ns, unit) for ns in sorted(VOCABS)}
            have = {k: v for k, v in have.items() if v}
            if have:
                problems.append(
                    "doc/11 §5.3 lists %s among the units with nowhere to go, "
                    "but the writer emits %s"
                    % (unit, ", ".join("%s:%s" % kv for kv in sorted(have.items()))))

    if verbose:
        print("  doc/11 §5.3: %d spelling claim(s) checked, %d problem(s)"
              % (checked, len(problems)))
    return problems, checked


def load_native_convertible():
    """A bound `bvn_units_convertible`, or None.

    Separate from load_native() because it asks a different question. That one
    reports a factor and a dimension vector, which is everything a value
    comparison needs and NOT everything a unit is: the dimension vector is
    [0,…,0] for the bit, the radian, the steradian, the decibel and every ratio
    alike, so two units can agree on both and still be different quantities.
    bvn_units_convertible is the predicate that also compares the QUANTITY KINDS
    the library tracks beside the dimensions (bvni_kind_table in
    bovnar_si_units.c), and it is the only thing here that can see the
    difference between a bit per second and a hertz."""
    sys.path.insert(0, os.path.join(REPO, "python"))
    try:
        import bovnar
        bovnar._ffi.load_library()
    except Exception:
        return None

    cache = {}

    def convertible(a, b):
        key = (a, b)
        if key in cache:
            return cache[key]
        try:
            ua, ub = bovnar.parse_unit(a), bovnar.parse_unit(b)
            r = bool(bovnar.units_convertible(ua, ub))
        except Exception:
            r = None                      # the library refuses one of them
        cache[key] = r
        return r

    return convertible


def check_cross_reference(cache, convertible, verbose):
    """QUDT's ucumCode/udunitsCode links, as a check of one table against another.

    WHAT THIS SEES THAT NOTHING ELSE DOES. Every other comparison here is one
    bovnar table against one publisher. This is one bovnar table against
    ANOTHER, with a publisher's own cross-reference as the claim that the two
    rows describe the same unit: QUDT states, for most of the units it defines,
    the UCUM code and the UDUNITS code for that unit, and bovnar maps all three
    vocabularies independently. So `qudt:X` and `ucum:<X's ucumCode>` must land
    on units that are inter-convertible — same dimension AND same quantity kind.

    Three defects it found that neither the factor comparison nor
    check_quantity_kinds could:

      qudt:NP-PER-SEC -> s^-1        a neper per second is not a reciprocal
                                     second. QUDT files it under a quantity kind
                                     literally named "Unknown", so the kind check
                                     had nothing to compare; its ucumCode "Np/s"
                                     said so plainly.
      qudt:IU-PER-MilliGM -> µmol/kg QUDT models the international unit as an
                                     amount of substance and is self-consistent
                                     about it, so nothing internal to QUDT could
                                     object. Its ucumCode is "[IU].mg-1", and
                                     UCUM declares [IU] ARBITRARY — bovnar's own
                                     ucum table carries it as an opaque unit
                                     commensurable with nothing. One library,
                                     two answers for one quantity.
      qudt:S_Ab -> a conductivity    QUDT gives the absiemens the dimension
                                     vector of the siemens per metre, and its
                                     quantity kind to match, so both internal
                                     checks agreed with it. Its ucumCode is "GS".

    A cross-reference is EVIDENCE, not proof — the same standing `class Unece`
    has, and for the same reason: a disagreement says one of the two tables is
    wrong, never which. Seventeen of them are QUDT's own (a code that reads
    "TW/h" for the terawatt-hour, "d/Cel" for the gram per degree Celsius, a
    thermal conductivity whose UCUM form has lost its foot), and each is waived
    by name in WAIVED_XREF with what the evidence was."""
    units = parse_ttl(os.path.join(cache, "qudt-units.ttl"), "unit")
    doc = bvnr_data.load(open(os.path.join(GENDATA, "qudt.bvnr"), "rb").read())
    tgt = {m["code"]: m["bovnar"] for m in doc.get("mapped", [])}
    bad, notes, checked, unreachable = [], [], 0, 0
    for local in sorted(units):
        mine = tgt.get(local)
        if mine is None:
            continue
        for pred, ns in (("ucumCode", "ucum"), ("udunitsCode", "udunits")):
            for raw in units[local].get(pred, []):
                code = raw.split("^^")[0].strip().strip('"')
                if not code:
                    continue
                if (local, code) in WAIVED_XREF:
                    notes.append("  %-26s ~ %s:%-24s %s"
                                 % (local, ns, code, WAIVED_XREF[(local, code)]))
                    continue
                ok = convertible(mine, "%s:%s" % (ns, code))
                if ok is None:
                    # The library refuses the cross-referenced spelling — the
                    # code names something that vocabulary does not define, or
                    # something bovnar carries as unsupported. Not a claim about
                    # the row under test.
                    unreachable += 1
                    continue
                checked += 1
                if not ok:
                    bad.append("  %-26s -> %-20s is not convertible with "
                               "%s:%s" % (local, mine, ns, code))
                elif verbose:
                    print("    xref ok %-24s -> %-14s ~ %s:%s"
                          % (local, mine, ns, code))
    return bad, notes, checked, unreachable


def check_quantity_kinds(cache, convertible, verbose):
    """QUDT's own unit -> quantity-kind links, as a check on both tables.

    THE HOLE THIS CLOSES. Every other comparison in this file is a NUMBER
    against a NUMBER: the publisher's factor and dimension vector against the
    native target's. That cannot see a quantity kind, because a kind is exactly
    what a dimension vector has no room for — a bit, a radian, a steradian, a
    decibel and a pure ratio are all [0,0,0,0,0,0,0], and a bit per second and a
    hertz are both T⁻¹ at factor 1. So a mapping could turn a data rate into a
    frequency, a bit density into a reciprocal length, or a radiance into an
    irradiance, and pass every check here with a perfect score.

    That is not hypothetical. It is what the tables did: qudt:ExaBIT-PER-SEC was
    E~Hz, qudt-qk:BitRate and ByteRate were Hz, BitTransmissionRate and
    ByteTransmissionRate were s⁻¹, LinearBitDensity was m⁻¹, DataRate was Hz,
    and PowerPerAreaAngle and TotalRadiance were W/m² beside a `Radiance` that
    correctly carried the steradian. Seven of them, each a unit the library's own
    bvn_units_convertible calls incompatible with the one it was standing in for.

    The check needs no new source. QUDT already states, for nearly every unit it
    defines, the quantity kind that unit measures (`qudt:hasQuantityKind`) — and
    bovnar maps both vocabularies. A kind translates to the COHERENT unit of the
    kind (see qudt-qk.bvnr), so a unit OF that kind must be convertible with it.
    Two tables, one publisher's cross-reference between them, and an assertion
    neither table can satisfy on its own.

    Reported per unit/kind pair. A pair where either side is unmapped, or where
    the library refuses one of the two expressions, is skipped rather than
    guessed at; WAIVED_KIND holds the links QUDT itself has filed wrongly."""
    units = parse_ttl(os.path.join(cache, "qudt-units.ttl"), "unit")
    tgt = {}
    for ns in ("qudt", "qudt-qk"):
        doc = bvnr_data.load(
            open(os.path.join(GENDATA, ns + ".bvnr"), "rb").read())
        tgt[ns] = {m["code"]: m["bovnar"] for m in doc.get("mapped", [])}
    bad, notes, checked = [], [], 0
    for local in sorted(units):
        utarget = tgt["qudt"].get(local)
        if utarget is None:
            continue
        for ref in units[local].get("hasQuantityKind", []):
            kind = ref.split(":")[-1].strip()
            ktarget = tgt["qudt-qk"].get(kind)
            if ktarget is None:
                continue
            if (local, kind) in WAIVED_KIND:
                notes.append("  %-16s ~ %-22s %s"
                             % (local, kind, WAIVED_KIND[(local, kind)]))
                continue
            ok = convertible(utarget, ktarget)
            if ok is None:
                continue
            checked += 1
            if not ok:
                bad.append("  %-26s -> %-16s is not convertible with "
                           "%s -> %s" % (local, utarget, kind, ktarget))
            elif verbose:
                print("    kind ok %-24s -> %-14s ~ %s" % (local, utarget, kind))
    return bad, notes, checked


def agrees_with_split(vocab, code, split, target, native):
    """Does our target mean the same as the publisher's prefix+atom reading?"""
    up = vocab.resolve(code)
    nat = native(target)
    if up is None or nat is None:
        return True                      # nothing comparable; do not cry wolf
    try:
        nf, nd = nat
        uf, ud = vocab.normalise(up, nd) if hasattr(vocab, "normalise") else up
        return list(ud) == list(nd) and close(uf, nf)
    except Exception:                                         # noqa: BLE001
        return True


def check_profile(ns, vocab, native, native_index, verbose):
    """-> (mismatches, dead, missing) as lists of printable lines."""
    doc = bvnr_data.load(open(os.path.join(GENDATA, ns + ".bvnr"), "rb").read())
    mapped = doc.get("mapped", [])
    unsupported = {r["code"] for r in doc.get("unsupported", [])}
    opaque = ({r["code"] for r in doc.get("opaque", [])}
              | {r["code"] for r in doc.get("opaque_aliases", [])})
    declared = {m["code"] for m in mapped} | unsupported | opaque

    mismatch, dead, missing, notes, checked = [], [], [], [], 0

    for m in mapped:
        code, target = m["code"], m["bovnar"]
        if (ns, code) in WAIVED_MODEL:
            if verbose:
                print("    waived  %-24s -> %-14s (modelling difference)"
                      % (code, target))
            continue
        if (ns, code) in WAIVED_UPSTREAM:
            notes.append("  %-20s -> %-14s %s"
                         % (code, target, WAIVED_UPSTREAM[(ns, code)]))
            continue
        if not vocab.accepts(code):
            # A code the publisher does not define as an ATOM is not
            # automatically a defect, and lumping the cases together made this
            # report unreadable: it named ten UDUNITS rows of which one was
            # correct, five were deliberate, and four were hazards. The three
            # outcomes are distinguishable.
            split = (vocab.prefixed_reading(code)
                     if hasattr(vocab, "prefixed_reading") else None)
            writable = m.get("reverse", True)
            if split is not None:
                # The publisher's own language reaches it as prefix + atom. Ours
                # must AGREE with that reading -- `udunits:dB` is deci + B and a
                # decibel either way, while `udunits:nmi` is nano + mi, a
                # NANOMILE, against the nautical mile this table used to give
                # it: 1.15e9 out, in the same dimension, undetectable. And
                # `udunits:pH` is pico + H, an inductance rather than an
                # acidity.
                if not agrees_with_split(vocab, code, split, target, native):
                    dead.append("  %-24s -> %-16s %s reads this as %s, a "
                                "DIFFERENT quantity"
                                % (code, target, ns.upper(), split))
            elif writable:
                # Nothing in the publisher's vocabulary reaches it, and we EMIT
                # it: a document carrying this code claims one the other side
                # cannot parse, which is the one thing a namespace exists to
                # prevent.
                dead.append("  %-24s -> %-16s not defined by %s, and this row "
                            "is WRITABLE" % (code, target, ns.upper()))
            # else: not defined, but `.reverse = false` -- accepted as a
            # courtesy on read and never emitted. Deliberate; not reported.
            continue
        up = vocab.resolve(code)
        if up is None:
            continue                      # affine/logarithmic: nothing to compare
        nat = native(target)
        if nat is None:
            continue                      # affine target: same
        nf, nd = nat
        uf, ud = vocab.normalise(up, nd) if hasattr(vocab, "normalise") else up
        checked += 1
        if list(ud) != list(nd):
            mismatch.append(
                "  DIM   %-24s -> %-14s %s says %s, native is %s"
                % (code, target, ns.upper(), ud, nd))
        elif not close(uf, nf):
            mismatch.append(
                "  FAC   %-24s -> %-14s %s says %.12g, native is %.12g "
                "(ratio %.10g)" % (code, target, ns.upper(), uf, nf, uf / nf))
        elif verbose:
            print("    ok      %-24s -> %-14s %.10g" % (code, target, nf))

    # Coverage: a publisher's code whose VALUE is exactly a native unit, and
    # which the table does not carry.
    #
    # This used to ask whether the code parsed as a native unit expression,
    # which only ever fires when the two vocabularies happen to spell a unit the
    # same way -- so it found a little for ucum and udunits and nothing at all
    # for the flat vocabularies, where a code like "KVA" resembles no native
    # spelling. Matching on the VALUE instead is what makes the suggestion
    # useful for exactly the profiles that needed it.
    for code in sorted(vocab.spellings()):
        if code in declared or (ns, code) in WAIVED_MODEL:
            continue
        up = vocab.resolve(code)
        if up is None:
            continue
        # A coverage suggestion should ask whether a PRODUCER can write this and
        # get the right unit, not whether the table has a row for it. An expr
        # profile reaches "kg" as k + g and "kilogram" as kilo + gram through
        # the prefix mechanism, with no row and nothing missing; reporting those
        # as gaps is how a closed table still looks open. Asking the library
        # settles it, and settles it the only way that matters — a spelling that
        # parses to the WRONG unit is still reported, because it is a worse
        # defect than a missing row rather than a lesser one.
        try:
            reached = native("%s:%s" % (ns, code))
        except Unresolved:
            reached = None       # the library refuses it: a genuine gap
        if reached is not None:
            f, d = vocab.normalise(up, reached[1]) \
                if hasattr(vocab, "normalise") else up
            if list(d) == list(reached[1]) and close(f, reached[0]):
                continue
        hit = native_index.match(vocab, up)
        if hit:
            missing.append("  %-22s == native %-12s (%.10g)" % (code, hit[0],
                                                                hit[1]))

    # STALE REFUSALS. A code in `.unsupported` whose value an existing native
    # expression already covers.
    #
    # The MISSING sweep above only looks at codes the table has NOT declared, so
    # a refusal is invisible to it forever: the reason was true when it was
    # written and nothing re-asks. That is how om:are (a centihectare),
    # om:poundal (lb·ft/s²), qudt:DeciM3-PER-MIN (a litre per minute) and forty
    # more sat refused while the registry could spell every one of them, and how
    # a refusal written because a unit was missing outlives the unit being
    # added.
    #
    # Advisory, like MISSING, and for a stronger reason: the value matching is
    # not proof. A dimensionless refusal matches the first dimensionless native
    # unit by construction, om:shake (1e-8 s) "matches" c~P/bar because the
    # dimensions happen to agree, and a footcandle matches cd/ft² — a LUMINANCE
    # — as readily as the lm/ft² it actually is. The output is a list to read,
    # not a list to apply.
    stale = []
    for r in doc.get("unsupported", []):
        code = str(r["code"])
        try:
            up = vocab.resolve(code)
        except Exception:
            continue
        if up is None:
            continue
        # A DIMENSIONLESS refusal matches something dimensionless by
        # construction -- every native ratio, the bit, the radian and the
        # decibel share the empty vector -- and "nothing here can tell which
        # one it is" is the reason most of them were refused. Reporting those
        # would bury the real finds under nine hundred non-answers.
        if not any(up[1]):
            continue
        hit = native_index.match(vocab, up)
        if hit:
            if (ns, code) in REVIEWED_STALE:
                REVIEWED_STALE_SEEN.add((ns, code))
                if verbose:
                    print("    reviewed %-26s == native %-14s kept: %s"
                          % (code, hit[0], REVIEWED_STALE[(ns, code)]))
                continue
            stale.append("  %-28s == native %-16s (refused: %s)"
                         % (code, hit[0], r.get("why", "")[:44]))

    prefixed = check_prefixed_refusals(vocab, doc, native)
    return mismatch, dead, missing, notes, checked, stale, prefixed


# The prefixes a publisher spells as a WORD in front of a code it also defines
# bare. Built from prefixes.bvnr so it cannot drift from the parser's own set.
def _prefix_words():
    doc = bvnr_data.load(open(os.path.join(GENDATA, "prefixes.bvnr"), "rb").read())
    return {p["name"].lower(): p["symbol"] for p in doc["si_prefixes"]}


def check_prefixed_refusals(vocab, doc, native):
    """Refusals the table's OWN neighbours already answer.

    A vocabulary that spells a prefixed unit as one token -- om:kilocalorie-Mean,
    qudt:MilliDARCY, qudt:KiloMIL_Circ -- names the prefix in the code. When the
    bare code beside it is already mapped, `<prefix>~<that target>` is a spelling
    this build accepts and is worth exactly what the publisher says the prefixed
    code is worth. A refusal in that position is not a judgement about the unit;
    it is a row nobody revisited, and every one of them said so in terms that
    were false -- "bovnar has no unit of this magnitude" beside a k~cal_m that
    spells it, "no native unit of this dimension is a decade away" beside an
    m~darcy exactly a decade away.

    This is deliberately NOT the value-matching sweep above, which is advisory
    because a factor and a dimension cannot tell a footcandle from a luminance.
    Three things have to agree here -- the publisher's own prefix WORD, the bare
    sibling already being mapped, and the value -- so a hit is a defect rather
    than a coincidence, and it is reported as one. The value-only sweep proposed
    `_100Kilometre == P~Å` and `litrePer100Kilometre == p~ha` on the same data;
    neither survives the name test.
    """
    if native is None:
        return []
    mapped = {m["code"]: m["bovnar"] for m in doc.get("mapped", [])}
    out = []
    for r in doc.get("unsupported", []):
        code = str(r["code"])
        low = code.lower().lstrip("_")
        for word, sym in _prefix_words().items():
            if not low.startswith(word):
                continue
            rest = code[len(word):].lstrip("-_")
            sib = next((c for c in mapped if c.lower() == rest.lower()), None)
            if not sib:
                break
            expr = "%s~%s" % (sym, mapped[sib])
            try:
                nr = native(expr)
            except Unresolved:
                break
            if nr is None:
                break
            try:
                up = vocab.resolve(code)
            except Exception:
                break
            if up is None:
                break
            uf, ud = (vocab.normalise(up, nr[1])
                      if hasattr(vocab, "normalise") else up)
            if list(ud) == list(nr[1]) and close(uf, nr[0]):
                out.append("  %-26s is %s of %s, which maps to %s — so it is "
                           "%s, not a refusal (stated reason: %s)"
                           % (code, word, sib, mapped[sib], expr,
                              r.get("why", "")[:40]))
            break
    return out


def fetch(cache, which):
    import urllib.request
    os.makedirs(cache, exist_ok=True)
    for ns in which:
        for fn, url in SOURCES[ns]:
            dest = os.path.join(cache, fn)
            with urllib.request.urlopen(url, timeout=30) as r:
                data = r.read()
            with open(dest, "wb") as f:
                f.write(data)
            print("  fetched %-26s %7d bytes" % (fn, len(data)))


def have_cache(cache, ns):
    return all(os.path.exists(os.path.join(cache, fn))
               for fn, _ in SOURCES[ns])


DOC_ROW_CLAIMS = (
    ("om", r"is worth: \*\*(\d+) rows compared, no mismatch\*\*"),
    ("cf", r"reading of the target\. (\d+) rows compared, no mismatch"),
)


def doc_row_claims():
    """{vocab: (stated, line)} for doc/11's per-vocabulary row counts."""
    path = os.path.join(REPO, "doc", "11_bovnar_unit_profiles.md")
    if not os.path.exists(path):
        return {}
    with open(path, encoding="utf-8") as f:
        text = f.read()
    out = {}
    for vocab, pattern in DOC_ROW_CLAIMS:
        m = re.search(pattern, text)
        if m:
            out[vocab] = (int(m.group(1)), text.count("\n", 0, m.start()) + 1)
    return out


def main(argv):
    ap = argparse.ArgumentParser(
        description="check the unit-profile tables against their publishers")
    ap.add_argument("--fetch", action="store_true",
                    help="download the vocabularies into the cache first")
    ap.add_argument("--cache", default=DEFAULT_CACHE,
                    help="where the publisher files live (default %s)"
                         % DEFAULT_CACHE)
    ap.add_argument("--profile", action="append",
                    choices=sorted(VOCABS), help="restrict to one vocabulary")
    ap.add_argument("--strict", action="store_true",
                    help="a missing cache or library is a failure, not a skip")
    ap.add_argument("--strict-dead", action="store_true",
                    help="also fail on a spelling the publisher does not define")
    ap.add_argument("--verbose", action="store_true")
    args = ap.parse_args(argv[1:])

    which = args.profile or sorted(VOCABS)

    if args.fetch:
        print("check_profile_factors: fetching into %s" % args.cache)
        fetch(args.cache, which)

    native = load_native()
    if native is None:
        msg = ("the reference library is not loadable — set LIBBOVNAR_PATH or "
               "build libbvnr first")
        if args.strict:
            print("check_profile_factors: %s" % msg)
            return 1
        print("check_profile_factors: SKIPPED — %s" % msg)
        return 0

    # Before anything that needs the network: the tables' own read direction.
    # This asks only the built library and src/gendata, so it runs on every
    # invocation rather than only the ones with a populated cache.
    rb_problems, rb_checked = check_codes_read_back(args.verbose)
    if rb_problems:
        print("check_profile_factors: %d code(s) the tables declare cannot be "
              "read back as declared:" % len(rb_problems))
        for msg in rb_problems[:20]:
            print("  %s" % msg)
        if len(rb_problems) > 20:
            print("  ... and %d more" % (len(rb_problems) - 20))
        return 1
    if rb_checked:
        print("check_profile_factors: %d profile codes read back as declared"
              % rb_checked)

    # ...and the documentation's account of what the WRITER emits.
    ds_problems, ds_checked = check_doc_profile_spellings(args.verbose)
    if ds_problems:
        print("check_profile_factors: %d documented spelling(s) disagree with "
              "the writer:" % len(ds_problems))
        for msg in ds_problems:
            print("  %s" % msg)
        return 1
    if ds_checked:
        print("check_profile_factors: %d documented spelling claims agree with "
              "the writer" % ds_checked)

    ec_problems, ec_checked = check_doc_error_claims(args.verbose)
    if ec_problems:
        print("check_profile_factors: %d documented error claim(s) disagree "
              "with the reader:" % len(ec_problems))
        for msg in ec_problems:
            print("  %s" % msg)
        return 1
    if ec_checked:
        print("check_profile_factors: %d documented error claims agree with the "
              "reader" % ec_checked)

    # ...and the per-vocabulary row counts the document states. These are
    # MEASURED figures, so they go stale exactly when the tables grow: doc/11 §16.2
    # said 1198 rows for om against 1275, and doc/11 §17.2 said 4430 for cf against
    # 4434. Checked against this run rather than against a remembered one.
    doc_rows = doc_row_claims()

    available = [ns for ns in which if have_cache(args.cache, ns)]
    if not available:
        msg = ("no publisher files under %s — run with --fetch (needs network)"
               % args.cache)
        if args.strict:
            print("check_profile_factors: %s" % msg)
            return 1
        print("check_profile_factors: SKIPPED — %s" % msg)
        return 0

    native_index = NativeIndex(native)
    total_mismatch = total_dead = total_missing = total_checked = 0
    doc_row_bad = []
    total_stale = 0
    prefixed_bad = []
    for ns in available:
        vocab = VOCABS[ns](args.cache)
        if args.verbose:
            print("  %s:" % ns)
        mism, dead, missing, notes, checked, stale, prefixed = check_profile(
            ns, vocab, native, native_index, args.verbose)
        if prefixed:
            prefixed_bad.append((ns, prefixed))
        if getattr(vocab, "secondary", False):
            doc = bvnr_data.load(
                open(os.path.join(GENDATA, ns + ".bvnr"), "rb").read())
            uncovered = sorted(m["code"] for m in doc.get("mapped", [])
                               if vocab.resolve(m["code"]) is None
                               and (ns, m["code"]) not in WAIVED_UPSTREAM)
            print("\n%s — checked through a SECONDARY source (QUDT's "
                  "uneceCommonCode), so a disagreement is evidence, not proof."
                  % ns.upper())
            print("  %d row(s) the cross-reference does not cover: %s"
                  % (len(uncovered), ", ".join(uncovered) or "none"))
            if vocab.ambiguous:
                print("  %d code(s) claimed by QUDT units that disagree, so "
                      "unusable here" % len(vocab.ambiguous))
        total_checked += checked
        if ns in doc_rows:
            stated, line = doc_rows[ns]
            if stated != checked:
                doc_row_bad.append(
                    "doc/11_bovnar_unit_profiles.md:%d: says %d rows compared "
                    "for %s; this run compared %d"
                    % (line, stated, ns, checked))
        if mism:
            print("\n%s — THE TABLE DISAGREES WITH THE PUBLISHER (%d):"
                  % (ns.upper(), len(mism)))
            for line in mism:
                print(line)
        if notes:
            print("\n%s — waived, the publisher is wrong or rounded (%d):"
                  % (ns.upper(), len(notes)))
            for line in notes:
                print(line)
        if dead:
            print("\n%s — spellings %s does not define (%d, not fatal):"
                  % (ns.upper(), ns.upper(), len(dead)))
            for line in dead:
                print(line)
        if missing and args.verbose:
            print("\n%s — publisher codes that are exactly a native unit and "
                  "are unmapped (%d, advisory):" % (ns.upper(), len(missing)))
            for line in missing:
                print(line)
        if stale and args.verbose:
            print("\n%s — REFUSALS an existing native expression now covers "
                  "(%d, advisory — read them, do not apply them):"
                  % (ns.upper(), len(stale)))
            for line in stale:
                print(line)
        total_stale += len(stale)
        total_mismatch += len(mism)
        total_dead += len(dead)
        total_missing += len(missing)

    # The one comparison that is not a number against a number. It needs both
    # QUDT vocabularies cached, since it is QUDT's own link between them.
    kind_bad, kind_checked = [], 0
    xref_bad, xref_checked, xref_skipped = [], 0, 0
    if "qudt" in available and "qudt-qk" in available:
        convertible = load_native_convertible()
        if convertible is not None:
            kind_bad, kind_notes, kind_checked = check_quantity_kinds(
                args.cache, convertible, args.verbose)
            if kind_notes:
                print("\nQUANTITY KIND — waived, QUDT files the unit under the "
                      "wrong kind (%d):" % len(kind_notes))
                for line in kind_notes:
                    print(line)
            if kind_bad:
                print("\nQUANTITY KIND — a unit is not convertible with the "
                      "coherent unit of the kind QUDT says it measures (%d):"
                      % len(kind_bad))
                for line in kind_bad:
                    print(line)
            xref_bad, xref_notes, xref_checked, xref_skipped = \
                check_cross_reference(args.cache, convertible, args.verbose)
            if xref_notes:
                print("\nCROSS-REFERENCE — waived, QUDT's own code does not "
                      "describe the unit it is attached to (%d):"
                      % len(xref_notes))
                for line in xref_notes:
                    print(line)
            if xref_bad:
                print("\nCROSS-REFERENCE — bovnar's translation of a QUDT code "
                      "and of the UCUM/UDUNITS code QUDT gives for it are not "
                      "the same unit (%d):" % len(xref_bad))
                for line in xref_bad:
                    print(line)

    print("\ncheck_profile_factors: %d row(s) compared across %s; "
          "%d mismatch, %d dead, %d unmapped-but-exact%s"
          % (total_checked, ", ".join(available), total_mismatch, total_dead,
             total_missing, "" if args.verbose else
             " (--verbose to list the last two)"))
    reviewed_here = {k for k in REVIEWED_STALE if k[0] in available}
    print("check_profile_factors: %d refusal(s) an existing native expression "
          "would now cover%s; %d more reviewed and kept (REVIEWED_STALE)"
          % (total_stale, "" if args.verbose else " (--verbose to list them)",
             len(REVIEWED_STALE_SEEN)))
    outdated = sorted(reviewed_here - REVIEWED_STALE_SEEN)
    if outdated:
        print("check_profile_factors: %d REVIEWED_STALE entr%s no longer "
              "match%s a refusal — remove %s:"
              % (len(outdated), "y" if len(outdated) == 1 else "ies",
                 "es" if len(outdated) == 1 else "",
                 "it" if len(outdated) == 1 else "them"))
        for ns, code in outdated:
            print("  %s:%s" % (ns, code))
    if prefixed_bad:
        print("\ncheck_profile_factors: refusal(s) the table's OWN mapped rows "
              "already answer — the code names a prefix, the bare sibling is "
              "mapped, and the value agrees:")
        for ns, lines in prefixed_bad:
            print("  %s:" % ns)
            for line in lines:
                print("  " + line)
    if kind_checked:
        print("check_profile_factors: %d unit/quantity-kind pair(s) checked "
              "for quantity kind; %d disagree" % (kind_checked, len(kind_bad)))
    if xref_checked:
        print("check_profile_factors: %d QUDT cross-reference(s) compared "
              "against the ucum/udunits tables; %d disagree (%d not reachable)"
              % (xref_checked, len(xref_bad), xref_skipped))
    if doc_row_bad:
        print("\ncheck_profile_factors: %d documented row count(s) disagree "
              "with this run:" % len(doc_row_bad))
        for msg in doc_row_bad:
            print("  %s" % msg)
    elif doc_rows:
        print("check_profile_factors: %d documented row count(s) agree with "
              "this run" % len(doc_rows))
    if total_mismatch or kind_bad or xref_bad or doc_row_bad or prefixed_bad:
        return 1
    if total_dead and args.strict_dead:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
