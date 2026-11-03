//! Port of `python/tests/test_currency_units.py`: the currency/crypto unit
//! extension (id blocks, catalogue metadata, predicates, code lookup).
//!
//! Not ported:
//! - `TestCurrencyInfoImmutable::test_frozen_dataclass_raises_on_mutation`:
//!   it checks that a frozen dataclass raises at run time when assigned to;
//!   Rust enforces immutability at compile time, so there is nothing to run.

use std::collections::BTreeSet;

use bovnar::BaseUnit;
use bovnar::currency::{
    all_crypto, all_currencies, all_fiat, currency_info, from_code, is_crypto, is_currency,
    is_fiat, minor_unit,
};
use bovnar::enums::{
    CURRENCY_COUNT, CURRENCY_FIRST, CURRENCY_LAST, UNIT_BLOCK_SIZE, UNIT_NATIVE_FIRST,
    UNIT_NATIVE_LAST,
};

fn all_currency_bases() -> Vec<i32> {
    BaseUnit::MEMBERS
        .iter()
        .map(|m| m.1)
        .filter(|b| (CURRENCY_FIRST..=CURRENCY_LAST).contains(b))
        .collect()
}

fn physical_names() -> BTreeSet<&'static str> {
    BaseUnit::MEMBERS
        .iter()
        .filter(|m| m.1 <= UNIT_NATIVE_LAST)
        .map(|m| m.0)
        .collect()
}

// ── TestEnumContiguity ────────────────────────────────────────────────────

#[test]
fn test_block_start() {
    assert_eq!(CURRENCY_FIRST, 900000);
}

#[test]
fn test_run_is_contiguous() {
    let mut bases = all_currency_bases();
    bases.sort();
    assert_eq!(bases, (CURRENCY_FIRST..=CURRENCY_LAST).collect::<Vec<_>>());
}

#[test]
fn test_total_currency_count() {
    assert_eq!(all_currency_bases().len() as i32, CURRENCY_COUNT);
    assert_eq!(CURRENCY_COUNT, 216);
}

#[test]
fn test_fits_inside_its_block() {
    const { assert!(CURRENCY_COUNT <= UNIT_BLOCK_SIZE) };
}

#[test]
fn test_units_and_currencies_do_not_overlap() {
    const { assert!(UNIT_NATIVE_LAST < CURRENCY_FIRST) };
    const { assert!(CURRENCY_FIRST - UNIT_NATIVE_FIRST > UNIT_BLOCK_SIZE) };
}

#[test]
fn test_currencies_added_after_release_are_ordinary_rows() {
    for code in ["ZWG", "XCG"] {
        let base = BaseUnit::from_name(code).unwrap();
        assert!((CURRENCY_FIRST..=CURRENCY_LAST).contains(&base));
        assert!(is_fiat(base) && !is_crypto(base));
    }
}

// ── TestFiatMetadata ──────────────────────────────────────────────────────

#[test]
fn test_fiat_metadata() {
    let cases: [(&str, u16, u8, &str); 22] = [
        ("USD", 840, 2, "Dollar"),
        ("EUR", 978, 2, "Euro"),
        ("GBP", 826, 2, "Pound"),
        ("JPY", 392, 0, "Yen"),
        ("CHF", 756, 2, "Franc"),
        ("KWD", 414, 3, "Kuwaiti"),
        ("BHD", 48, 3, "Bahraini"),
        ("OMR", 512, 3, "Omani"),
        ("JOD", 400, 3, "Jordanian"),
        ("TND", 788, 3, "Tunisian"),
        ("CLF", 990, 4, "Fomento"),
        ("VND", 704, 0, "Dong"),
        ("CLP", 152, 0, "Chilean"),
        ("KRW", 410, 0, "Korean"),
        ("ISK", 352, 0, "Icelandic"),
        ("XAU", 959, 0, "Gold"),
        ("XAG", 961, 0, "Silver"),
        ("XPT", 962, 0, "Platinum"),
        ("XPD", 964, 0, "Palladium"),
        ("XDR", 960, 0, "Drawing"),
        ("ZWG", 924, 2, "Zimbabwe"),
        ("XCG", 532, 2, "Caribbean"),
    ];
    for (code, num, mu, name_fragment) in cases {
        let base = from_code(code).unwrap();
        let info = currency_info(base).unwrap();
        assert_eq!(info.code, code);
        assert_eq!(info.numeric_code, num, "{code}");
        assert_eq!(info.minor_unit, mu, "{code}");
        assert!(!info.is_crypto);
        assert!(info.is_fiat());
        assert!(
            info.name
                .to_lowercase()
                .contains(&name_fragment.to_lowercase()),
            "{code}: {}",
            info.name
        );
    }
}

#[test]
fn test_aed_enum_value() {
    assert_eq!(BaseUnit::AED, 900000);
}

#[test]
fn test_zwl_enum_value() {
    assert_eq!(BaseUnit::ZWL, 900165);
}

#[test]
fn test_usd_enum_value() {
    assert_eq!(BaseUnit::USD, 900143);
}

#[test]
fn test_eur_enum_value() {
    assert_eq!(BaseUnit::EUR, 900043);
}

#[test]
fn test_jpy_enum_value() {
    assert_eq!(BaseUnit::JPY, 900067);
}

// ── TestCryptoMetadata ────────────────────────────────────────────────────

#[test]
fn test_crypto_metadata() {
    let cases: [(&str, u8, &str); 18] = [
        ("BTC", 8, "Bitcoin"),
        ("ETH", 18, "Ethereum"),
        ("SOL", 9, "Solana"),
        ("XRP", 6, "XRP"),
        ("BNB", 18, "BNB"),
        ("ADA", 6, "Cardano"),
        ("LTC", 8, "Litecoin"),
        ("DOT", 10, "Polkadot"),
        ("XMR", 12, "Monero"),
        ("ETC", 18, "Classic"),
        ("BCH", 8, "Bitcoin Cash"),
        ("XLM", 7, "Stellar"),
        ("DOGE", 8, "Dogecoin"),
        ("LINK", 18, "Chainlink"),
        ("USDT", 6, "Tether"),
        ("USDC", 6, "USD Coin"),
        ("AVAX", 18, "Avalanche"),
        ("ATOM", 6, "Cosmos"),
    ];
    for (code, mu, name_fragment) in cases {
        let base = from_code(code).unwrap();
        let info = currency_info(base).unwrap();
        assert_eq!(info.code, code);
        assert_eq!(info.numeric_code, 0, "{code}");
        assert_eq!(info.minor_unit, mu, "{code}");
        assert!(info.is_crypto);
        assert!(!info.is_fiat());
        assert!(
            info.name
                .to_lowercase()
                .contains(&name_fragment.to_lowercase()),
            "{code}: {}",
            info.name
        );
    }
}

#[test]
fn test_btc_enum_value() {
    assert_eq!(BaseUnit::BTC, 900166);
}

#[test]
fn test_eth_enum_value() {
    assert_eq!(BaseUnit::ETH, 900167);
}

#[test]
fn test_atom_enum_value() {
    assert_eq!(BaseUnit::ATOM, 900199);
}

#[test]
fn test_doge_is_4_char() {
    assert_eq!(BaseUnit::name(BaseUnit::DOGE).unwrap().len(), 4);
}

#[test]
fn test_link_is_4_char() {
    assert_eq!(BaseUnit::name(BaseUnit::LINK).unwrap().len(), 4);
}

#[test]
fn test_usdt_is_4_char() {
    assert_eq!(BaseUnit::name(BaseUnit::USDT).unwrap().len(), 4);
}

#[test]
fn test_usdc_is_4_char() {
    assert_eq!(BaseUnit::name(BaseUnit::USDC).unwrap().len(), 4);
}

#[test]
fn test_avax_is_4_char() {
    assert_eq!(BaseUnit::name(BaseUnit::AVAX).unwrap().len(), 4);
}

#[test]
fn test_atom_is_4_char() {
    assert_eq!(BaseUnit::name(BaseUnit::ATOM).unwrap().len(), 4);
}

// ── TestPredicates ────────────────────────────────────────────────────────

#[test]
fn test_is_currency_true() {
    for base in [
        BaseUnit::USD,
        BaseUnit::EUR,
        BaseUnit::JPY,
        BaseUnit::XAU,
        BaseUnit::BTC,
        BaseUnit::ETH,
        BaseUnit::DOGE,
        BaseUnit::USDT,
    ] {
        assert!(is_currency(base), "{base}");
    }
}

#[test]
fn test_is_currency_false_physical() {
    for base in [
        BaseUnit::NONE,
        BaseUnit::METER,
        BaseUnit::GRAM,
        BaseUnit::SECOND,
        BaseUnit::HERTZ,
        BaseUnit::PASCAL,
        BaseUnit::BUSHEL,
        BaseUnit::DENIER,
    ] {
        assert!(!is_currency(base), "{base}");
    }
}

#[test]
fn test_is_fiat_true() {
    for base in [
        BaseUnit::USD,
        BaseUnit::EUR,
        BaseUnit::GBP,
        BaseUnit::XAU,
        BaseUnit::XDR,
    ] {
        assert!(is_fiat(base), "{base}");
    }
}

#[test]
fn test_is_fiat_false_for_crypto() {
    for base in [BaseUnit::BTC, BaseUnit::ETH, BaseUnit::SOL, BaseUnit::DOGE] {
        assert!(!is_fiat(base), "{base}");
    }
}

#[test]
fn test_is_crypto_true() {
    for base in [
        BaseUnit::BTC,
        BaseUnit::ETH,
        BaseUnit::SOL,
        BaseUnit::DOGE,
        BaseUnit::USDT,
    ] {
        assert!(is_crypto(base), "{base}");
    }
}

#[test]
fn test_is_crypto_false() {
    for base in [BaseUnit::USD, BaseUnit::EUR, BaseUnit::XAU, BaseUnit::METER] {
        assert!(!is_crypto(base), "{base}");
    }
}

// ── TestReverseCodeLookup ─────────────────────────────────────────────────

#[test]
fn test_from_code_exact() {
    for (code, expected) in [
        ("USD", BaseUnit::USD),
        ("EUR", BaseUnit::EUR),
        ("JPY", BaseUnit::JPY),
        ("BTC", BaseUnit::BTC),
        ("ETH", BaseUnit::ETH),
        ("DOGE", BaseUnit::DOGE),
        ("USDT", BaseUnit::USDT),
        ("AVAX", BaseUnit::AVAX),
        ("XAU", BaseUnit::XAU),
    ] {
        assert_eq!(from_code(code).unwrap(), expected, "{code}");
    }
}

#[test]
fn test_from_code_roundtrip_all() {
    for info in all_currencies() {
        assert_eq!(from_code(&info.code).unwrap(), info.base);
    }
}

// ── TestIterators ─────────────────────────────────────────────────────────

#[test]
fn test_all_fiat_count() {
    assert_eq!(all_fiat().count(), 166);
}

#[test]
fn test_all_crypto_count() {
    assert_eq!(all_crypto().count(), 50);
}

#[test]
fn test_all_currencies_count() {
    assert_eq!(all_currencies().count(), 216);
}

#[test]
fn test_all_fiat_are_not_crypto() {
    for info in all_fiat() {
        assert!(!info.is_crypto);
    }
}

#[test]
fn test_all_crypto_are_not_fiat() {
    for info in all_crypto() {
        assert!(info.is_crypto);
    }
}

#[test]
fn test_all_fiat_have_nonzero_numeric_code() {
    for info in all_fiat() {
        assert!(info.numeric_code > 0, "{} has numeric_code=0", info.code);
    }
}

#[test]
fn test_all_crypto_have_zero_numeric_code() {
    for info in all_crypto() {
        assert_eq!(info.numeric_code, 0);
    }
}

// ── TestMinorUnit3 / 0 / 4 ────────────────────────────────────────────────

#[test]
fn test_three_decimal_currencies() {
    for code in ["BHD", "JOD", "KWD", "OMR", "TND", "IQD", "LYD"] {
        assert_eq!(minor_unit(from_code(code).unwrap()).unwrap(), 3, "{code}");
    }
}

#[test]
fn test_zero_decimal_currencies() {
    for code in [
        "JPY", "KRW", "VND", "ISK", "CLP", "DJF", "GNF", "KMF", "PYG", "RWF", "UGX", "VUV", "XAF",
        "XOF", "XPF", "XAU", "XAG", "XPT", "XPD", "XDR",
    ] {
        assert_eq!(minor_unit(from_code(code).unwrap()).unwrap(), 0, "{code}");
    }
}

#[test]
fn test_clf_is_4_decimals() {
    assert_eq!(minor_unit(BaseUnit::CLF).unwrap(), 4);
}

// ── TestCryptoMinorUnits ──────────────────────────────────────────────────

#[test]
fn test_btc_satoshi() {
    assert_eq!(minor_unit(BaseUnit::BTC).unwrap(), 8);
}

#[test]
fn test_eth_wei() {
    assert_eq!(minor_unit(BaseUnit::ETH).unwrap(), 18);
}

#[test]
fn test_sol_lamport() {
    assert_eq!(minor_unit(BaseUnit::SOL).unwrap(), 9);
}

#[test]
fn test_xrp_drop() {
    assert_eq!(minor_unit(BaseUnit::XRP).unwrap(), 6);
}

#[test]
fn test_xlm_stroop() {
    assert_eq!(minor_unit(BaseUnit::XLM).unwrap(), 7);
}

#[test]
fn test_dot_planck() {
    assert_eq!(minor_unit(BaseUnit::DOT).unwrap(), 10);
}

#[test]
fn test_xmr_piconero() {
    assert_eq!(minor_unit(BaseUnit::XMR).unwrap(), 12);
}

#[test]
fn test_eos_four_decimals() {
    assert_eq!(minor_unit(BaseUnit::EOS).unwrap(), 4);
}

#[test]
fn test_neo_is_indivisible() {
    // NEO cannot be held or transferred fractionally; the 8 decimals belong
    // to its companion gas token.
    assert_eq!(minor_unit(BaseUnit::NEO).unwrap(), 0);
}

#[test]
fn test_usdt_six_decimals() {
    assert_eq!(minor_unit(BaseUnit::USDT).unwrap(), 6);
}

#[test]
fn test_usdc_six_decimals() {
    assert_eq!(minor_unit(BaseUnit::USDC).unwrap(), 6);
}

// ── TestCurrencyInfoImmutable ─────────────────────────────────────────────

#[test]
fn test_str_returns_code() {
    assert_eq!(currency_info(BaseUnit::EUR).unwrap().to_string(), "EUR");
}

// ── TestFromCodeCaseFolding ───────────────────────────────────────────────

#[test]
fn test_case_insensitive() {
    for variant in ["usd", "USD", "Usd", " USD ", "uSD"] {
        assert_eq!(from_code(variant).unwrap(), BaseUnit::USD, "{variant:?}");
    }
}

#[test]
fn test_crypto_case_insensitive() {
    for variant in ["btc", "BTC", "Btc"] {
        assert_eq!(from_code(variant).unwrap(), BaseUnit::BTC, "{variant:?}");
    }
}

#[test]
fn test_four_char_case_insensitive() {
    for variant in ["doge", "DOGE", "Doge"] {
        assert_eq!(from_code(variant).unwrap(), BaseUnit::DOGE, "{variant:?}");
    }
}

// ── TestFromCodeUnknown ───────────────────────────────────────────────────

#[test]
fn test_raises_key_error() {
    for bad in [
        "XYZ", "AAA", "BBB", "foo", "METER", "SECOND", "", "US", "USDO",
    ] {
        assert!(from_code(bad).is_err(), "{bad:?}");
    }
}

// ── TestCurrencyCodeUnique ────────────────────────────────────────────────

#[test]
fn test_all_codes_are_unique() {
    let codes: Vec<String> = all_currencies().map(|i| i.code).collect();
    let unique: BTreeSet<&String> = codes.iter().collect();
    assert_eq!(codes.len(), unique.len());
}

#[test]
fn test_codes_are_uppercase() {
    for info in all_currencies() {
        assert_eq!(
            info.code,
            info.code.to_uppercase(),
            "{} is not uppercase",
            info.code
        );
    }
}

#[test]
fn test_fiat_codes_are_3_chars() {
    for info in all_fiat() {
        assert_eq!(info.code.chars().count(), 3, "{} is not 3 chars", info.code);
    }
}

#[test]
fn test_crypto_codes_are_3_or_4_chars() {
    for info in all_crypto() {
        let n = info.code.chars().count();
        assert!((3..=4).contains(&n), "{} length out of range", info.code);
    }
}

#[test]
fn test_no_collision_with_single_letter_si_units() {
    let si_single = ["K", "A", "W", "V", "F", "N", "C", "S", "T", "J"];
    for info in all_currencies() {
        assert!(
            !si_single.contains(&info.code.as_str()),
            "{} collides with SI unit",
            info.code
        );
    }
}

#[test]
fn test_no_collision_with_two_letter_mixed_case_si_units() {
    let si_two = ["Hz", "Pa", "Wb", "Bq", "Gy", "Sv", "PS", "VA"];
    for info in all_currencies() {
        assert!(
            !si_two.contains(&info.code.as_str()),
            "{} collides with SI unit",
            info.code
        );
    }
}

#[test]
fn test_physical_unit_enum_names_do_not_appear_as_currency_member_names() {
    let physical = physical_names();
    let currency_member_names: BTreeSet<&str> = BaseUnit::MEMBERS
        .iter()
        .filter(|m| (CURRENCY_FIRST..=CURRENCY_LAST).contains(&m.1))
        .map(|m| m.0)
        .collect();
    let clash: Vec<_> = physical.intersection(&currency_member_names).collect();
    assert!(clash.is_empty(), "Name collision(s): {clash:?}");
}

// ── TestEnumCoverageComplete ──────────────────────────────────────────────

#[test]
fn test_every_currency_base_has_table_entry() {
    let missing: Vec<i32> = all_currency_bases()
        .into_iter()
        .filter(|&b| currency_info(b).is_err())
        .collect();
    assert!(missing.is_empty(), "Missing table entries: {missing:?}");
}

#[test]
fn test_table_size_matches_enum_range() {
    let n_block = CURRENCY_LAST - CURRENCY_FIRST + 1;
    let n_table = all_currencies().count() as i32;
    assert_eq!(n_table, n_block);
    assert_eq!(n_block, CURRENCY_COUNT);
}

// ── TestPhysicalUnitCollisions ────────────────────────────────────────────

/// Wire code → disambiguated member name.
const KNOWN_DISAMBIGUATIONS: [(&str, &str); 1] = [("CUP", "CUP_")];

#[test]
fn test_cup_physical_unit_is_still_present() {
    assert_eq!(BaseUnit::CUP, 100080);
}

#[test]
fn test_cup_cuban_peso_is_cup_underscore() {
    assert_eq!(BaseUnit::CUP_, 900033);
}

#[test]
fn test_cup_wire_code_is_still_cup() {
    assert_eq!(currency_info(BaseUnit::CUP_).unwrap().code, "CUP");
}

#[test]
fn test_cup_from_code_returns_cuban_peso() {
    assert_eq!(from_code("CUP").unwrap(), BaseUnit::CUP_);
}

#[test]
fn test_cup_from_code_does_not_return_physical_cup() {
    assert_ne!(from_code("CUP").unwrap(), BaseUnit::CUP);
}

#[test]
fn test_physical_cup_is_not_a_currency() {
    assert!(!is_currency(BaseUnit::CUP));
}

#[test]
fn test_cuban_peso_is_a_fiat_currency() {
    assert!(is_fiat(BaseUnit::CUP_));
}

#[test]
fn test_all_known_collisions_are_accounted_for() {
    let physical = physical_names();
    let codes: BTreeSet<String> = all_currencies().map(|i| i.code).collect();
    let conflicts: BTreeSet<&str> = physical
        .into_iter()
        .filter(|n| codes.contains(*n))
        .collect();
    let known: BTreeSet<&str> = KNOWN_DISAMBIGUATIONS.iter().map(|k| k.0).collect();
    assert_eq!(conflicts, known);
}

#[test]
fn test_disambiguated_names_not_in_physical_range() {
    for (_, enum_name) in KNOWN_DISAMBIGUATIONS {
        let member = BaseUnit::from_name(enum_name).unwrap();
        assert!(
            member >= CURRENCY_FIRST,
            "{enum_name} is in the physical range"
        );
    }
}

// ── TestBaseUnitBlocks ────────────────────────────────────────────────────

#[test]
fn test_no_sentinel() {
    assert_eq!(BaseUnit::from_name("_SENTINEL"), None);
}

#[test]
fn test_native_units_are_one_contiguous_run() {
    let mut native: Vec<i32> = BaseUnit::MEMBERS
        .iter()
        .map(|m| m.1)
        .filter(|b| (UNIT_NATIVE_FIRST..=UNIT_NATIVE_LAST).contains(b))
        .collect();
    native.sort();
    assert_eq!(
        native,
        (UNIT_NATIVE_FIRST..=UNIT_NATIVE_LAST).collect::<Vec<_>>()
    );
}

#[test]
fn test_every_member_is_none_a_unit_or_a_currency() {
    for &(name, b) in BaseUnit::MEMBERS {
        assert!(
            b == 0
                || (UNIT_NATIVE_FIRST..=UNIT_NATIVE_LAST).contains(&b)
                || (CURRENCY_FIRST..=CURRENCY_LAST).contains(&b),
            "{name}"
        );
    }
}

// ── TestPrefixRulesC ──────────────────────────────────────────────────────
// The contract of bvn_currency_prefix_valid(base, prefix_system), checked
// without calling C code.

const SI: i32 = 0;
const IEC: i32 = 1;

fn prefix_valid(base: i32, prefix_system: i32) -> bool {
    if (CURRENCY_FIRST..=CURRENCY_LAST).contains(&base) {
        return prefix_system != IEC;
    }
    true
}

#[test]
fn test_iec_prefix_invalid_on_currency() {
    for code in ["USD", "EUR", "BTC", "ETH", "DOGE"] {
        assert!(!prefix_valid(from_code(code).unwrap(), IEC), "{code}");
    }
}

#[test]
fn test_si_prefix_valid_on_currency() {
    for code in ["USD", "EUR", "BTC", "ETH", "DOGE"] {
        assert!(prefix_valid(from_code(code).unwrap(), SI), "{code}");
    }
}

#[test]
fn test_iec_prefix_valid_on_physical_units_not_blocked() {
    for base in [BaseUnit::METER, BaseUnit::GRAM, BaseUnit::SECOND] {
        assert!(prefix_valid(base, IEC), "{base}");
    }
}

// ── TestParseCurrencyStr ──────────────────────────────────────────────────
// The contract of bvn_parse_currency_str(): anything in the table parses,
// anything outside does not.

fn is_upper_alpha(token: &str) -> bool {
    !token.is_empty() && token.chars().all(|c| c.is_alphabetic() && c.is_uppercase())
}

fn would_parse(token: &str) -> bool {
    let n = token.chars().count();
    if !(3..=4).contains(&n) || !is_upper_alpha(token) {
        return false;
    }
    from_code(token).is_ok()
}

#[test]
fn test_known_codes_parse() {
    for code in [
        "USD", "EUR", "GBP", "JPY", "CHF", "AUD", "CAD", "CNY", "XAU", "XAG", "XDR", "BHD", "KWD",
        "OMR", "CLF", "BTC", "ETH", "SOL", "XRP", "ADA", "LTC", "DOGE", "LINK", "USDT", "USDC",
        "AVAX", "ATOM",
    ] {
        assert!(would_parse(code), "{code}");
    }
}

#[test]
fn test_unknown_or_invalid_codes_reject() {
    for bad in [
        "XYZ", "AAA", "US", "METER", "usd", "btc", "123", "", "LONGCODE",
    ] {
        assert!(!would_parse(bad), "{bad:?}");
    }
}

// ── TestParseCurrencyStrRejects ───────────────────────────────────────────

#[test]
fn test_rejects_malformed_tokens() {
    let codes: BTreeSet<String> = all_currencies().map(|i| i.code).collect();
    for token in ["AB", "ABCDE", "Usd", "u$d", "123", ""] {
        if token.is_empty() {
            continue;
        }
        let n = token.chars().count();
        let ok = (3..=4).contains(&n) && is_upper_alpha(token);
        assert!(!ok || !codes.contains(token), "{token:?}");
    }
}

// ── TestBvnrFormatExamples ────────────────────────────────────────────────

/// The currency codes in the unit annotations of `snippet`: a hand-written
/// equivalent of the Python pattern
/// `<[^>]*,\s*(?:[a-zA-Z]+~)?([A-Z]{3,4})(?:[/·][^>]*)?>`, trying the same
/// alternatives in the same (greedy, backtracking) order.
fn extract_currency_codes(snippet: &str) -> Vec<String> {
    let chars: Vec<char> = snippet.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '<'
            && let Some((code, end)) = match_annotation(&chars, i + 1)
        {
            found.push(code);
            i = end;
            continue;
        }
        i += 1;
    }
    found
}

/// Matches the pattern after its `<`; returns the code and the index past `>`.
fn match_annotation(c: &[char], start: usize) -> Option<(String, usize)> {
    // `[^>]*` is greedy: try the last comma before the first `>` first.
    let close = (start..c.len()).find(|&k| c[k] == '>').unwrap_or(c.len());
    for comma in (start..close).rev().filter(|&k| c[k] == ',') {
        let mut ws_end = comma + 1;
        while ws_end < c.len() && c[ws_end].is_whitespace() {
            ws_end += 1;
        }
        for after_ws in (comma + 1..=ws_end).rev() {
            if let Some(m) = match_unit(c, after_ws) {
                return Some(m);
            }
        }
    }
    None
}

fn match_unit(c: &[char], start: usize) -> Option<(String, usize)> {
    // Optional `[a-zA-Z]+~`, greedy: with the prefix first, then without.
    let mut letters = start;
    while letters < c.len() && c[letters].is_ascii_alphabetic() {
        letters += 1;
    }
    let mut starts = Vec::new();
    for end in (start + 1..=letters).rev() {
        if end < c.len() && c[end] == '~' {
            starts.push(end + 1);
        }
    }
    starts.push(start);
    for code_start in starts {
        for len in [4, 3] {
            let code_end = code_start + len;
            if code_end > c.len() || !c[code_start..code_end].iter().all(char::is_ascii_uppercase) {
                continue;
            }
            if let Some(end) = match_tail(c, code_end) {
                return Some((c[code_start..code_end].iter().collect(), end));
            }
        }
    }
    None
}

/// `(?:[/·][^>]*)?>` at `k`; returns the index past `>`.
fn match_tail(c: &[char], k: usize) -> Option<usize> {
    if k < c.len() && (c[k] == '/' || c[k] == '·') {
        let close = (k + 1..c.len()).find(|&j| c[j] == '>')?;
        return Some(close + 1);
    }
    (k < c.len() && c[k] == '>').then_some(k + 1)
}

#[test]
fn test_currency_code_extractable() {
    for (snippet, expected_codes) in [
        (".price = <float_dec:64,USD> 19.99;", ["USD"]),
        (".rate  = <float_dec:64,USD/oz_t> 2351.0;", ["USD"]),
        (".rent  = <float_dec:64,EUR/h> 150.0;", ["EUR"]),
        (".btc_balance = <float_dec:64,BTC> 0.5;", ["BTC"]),
        (".doge = <uint:64,DOGE> 100000;", ["DOGE"]),
        (".link = <float_dec:64,LINK> 12.5;", ["LINK"]),
        (".kwd  = <uint:64,KWD> 1500;", ["KWD"]),
        (".jpy  = <uint:64,JPY> 10000;", ["JPY"]),
        (".fx   = <float_dec:64,USD/EUR> 1.0842;", ["USD"]),
    ] {
        let found = extract_currency_codes(snippet);
        for code in expected_codes {
            assert!(found.iter().any(|f| f == code), "{snippet}: {found:?}");
        }
    }
}

#[test]
fn test_all_documented_example_codes_are_in_table() {
    for code in [
        "USD", "EUR", "GBP", "JPY", "CHF", "KWD", "BHD", "OMR", "CLF", "XAU", "XAG", "XPT", "XDR",
        "BTC", "ETH", "SOL", "XRP", "ADA", "DOGE", "USDT", "AVAX",
    ] {
        assert!(from_code(code).is_ok(), "{code}");
    }
}
