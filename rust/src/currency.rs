//! Currency metadata, mirroring `python/bovnar/currency.py`. The table is
//! the C library's own catalogue; base units are plain `BaseUnit` ids.

use crate::enums::BaseUnit;
use crate::error::{Error, Result};
use crate::ffi;

pub use crate::enums::{CURRENCY_COUNT, CURRENCY_FIRST, CURRENCY_LAST};

/// Metadata for one fiat or crypto currency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrencyInfo {
    /// The base-unit id.
    pub base: BaseUnit,
    /// ISO 4217 or conventional ticker code.
    pub code: String,
    /// ISO 4217 numeric code; 0 for crypto.
    pub numeric_code: u16,
    /// Decimal places: 1 major unit = 10^minor_unit minor units.
    pub minor_unit: u8,
    pub is_crypto: bool,
    pub name: String,
}

impl CurrencyInfo {
    pub fn is_fiat(&self) -> bool {
        !self.is_crypto
    }
}

/// The code, like Python's `str(info)`.
impl std::fmt::Display for CurrencyInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.code)
    }
}

/// True if `base` is any fiat, precious metal or crypto currency.
pub fn is_currency(base: BaseUnit) -> bool {
    ffi::currency_kind(base as i32).0
}

/// True if `base` is an ISO 4217 fiat currency or precious metal.
pub fn is_fiat(base: BaseUnit) -> bool {
    ffi::currency_kind(base as i32).1
}

/// True if `base` is a cryptocurrency.
pub fn is_crypto(base: BaseUnit) -> bool {
    ffi::currency_kind(base as i32).2
}

/// The record for `base`; fails when it is not a currency.
pub fn currency_info(base: BaseUnit) -> Result<CurrencyInfo> {
    let (code, numeric_code, minor_unit, is_crypto, name) = ffi::currency_info(base as i32)
        .ok_or_else(|| Error::argument(format!("{} is not a currency unit", base.name())))?;
    Ok(CurrencyInfo {
        base,
        code,
        numeric_code,
        minor_unit,
        is_crypto,
        name,
    })
}

/// The minor-unit exponent of `base` (USD 2, JPY 0, KWD 3).
pub fn minor_unit(base: BaseUnit) -> Result<u8> {
    Ok(currency_info(base)?.minor_unit)
}

/// The human-readable name of `base`.
pub fn currency_name(base: BaseUnit) -> Result<String> {
    Ok(currency_info(base)?.name)
}

/// The code of `base`.
pub fn currency_code(base: BaseUnit) -> Result<String> {
    Ok(currency_info(base)?.code)
}

/// The base unit of a code; surrounding space and case are ignored.
pub fn from_code(code: &str) -> Result<BaseUnit> {
    match ffi::parse_currency(&code.trim().to_ascii_uppercase()) {
        0 => Err(Error::argument(format!("unknown currency code: {code:?}"))),
        base => BaseUnit::from_raw(base)
            .ok_or_else(|| Error::argument(format!("C currency id {base} is not in BaseUnit"))),
    }
}

/// Every currency, fiat and crypto, in catalogue order.
pub fn all_currencies() -> impl Iterator<Item = CurrencyInfo> {
    (CURRENCY_FIRST..)
        .map_while(BaseUnit::from_raw)
        .map_while(|base| currency_info(base).ok())
}

/// Every fiat currency and precious metal.
pub fn all_fiat() -> impl Iterator<Item = CurrencyInfo> {
    all_currencies().filter(CurrencyInfo::is_fiat)
}

/// Every cryptocurrency.
pub fn all_crypto() -> impl Iterator<Item = CurrencyInfo> {
    all_currencies().filter(|c| c.is_crypto)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_codes_and_rows() {
        let usd = from_code(" usd ").unwrap();
        assert!(is_currency(usd) && is_fiat(usd) && !is_crypto(usd));
        let info = currency_info(usd).unwrap();
        assert_eq!((info.code.as_str(), info.numeric_code), ("USD", 840));
        assert_eq!(minor_unit(from_code("JPY").unwrap()).unwrap(), 0);
        assert!(from_code("NOPE").is_err());
        assert!(currency_info(BaseUnit::None).is_err());
        let all = all_currencies().count();
        assert!(all > 100);
        assert_eq!(all, all_fiat().count() + all_crypto().count());
        assert!(all_crypto().any(|c| c.code == "BTC"));
    }
}
