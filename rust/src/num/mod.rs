//! Numeric and datetime literal checks.
//!
//! Ports of the C reference's validation helpers (`bovnar_utils.c`,
//! `bvn_float.c`, `bvn_datetime.c` and the ISO-8601 part of
//! `bovnar_validator.c`). They decide whether a literal is accepted or refused
//! exactly as the reference does, quirks included, so the reader's
//! accept/refuse behaviour stays byte-identical to it. Literals are the raw
//! token bytes; the C code sees the same bytes NUL-terminated.

mod bigint;
mod datetime;
mod fix;
mod int_range;

pub(crate) use datetime::{
    epoch_index_of, epoch_name, epoch_seconds_to_civil, i64_to_decimal, iso_to_epoch_seconds,
    str_is_iso_datetime, str_is_plain_integer,
};
pub(crate) use fix::float_str_fits_fix;
pub(crate) use int_range::{validate_sint_range, validate_uint_range};

/// The value of digit `c` in `base`, or `base` itself when `c` is not a digit
/// of that base (`bvn_char_to_digit`).
///
/// Bases up to 62 use `0-9`, `a-z`, `A-Z`; up to base 36 the letters are
/// case-insensitive, above it upper case continues after lower case. Base 64
/// is the standard Base64 alphabet and base 85 is Ascii85 (`!` to `u`).
pub(crate) fn char_to_digit(c: u8, base: u32) -> u32 {
    let c = u32::from(c);
    if base == 64 {
        return match c {
            0x41..=0x5a => c - 0x41,
            0x61..=0x7a => c - 0x61 + 26,
            0x30..=0x39 => c - 0x30 + 52,
            0x2b => 62,
            0x2f => 63,
            _ => base,
        };
    }
    if base == 85 {
        return if (0x21..=0x75).contains(&c) {
            c - 0x21
        } else {
            base
        };
    }
    let d = match c {
        0x30..=0x39 => c - 0x30,
        0x61..=0x7a => 10 + (c - 0x61),
        0x41..=0x5a if base > 36 => 36 + (c - 0x41),
        0x41..=0x5a => 10 + (c - 0x41),
        _ => return base,
    };
    if d < base {
        d
    } else {
        base
    }
}

/// Whether `s` is one of the special numbers `inf`, `nan` or `ninf`
/// (`bvn_is_special_number_string`).
pub(crate) fn is_special_number(s: &[u8]) -> bool {
    matches!(s, b"inf" | b"nan" | b"ninf")
}

/// Whether `s` is a non-empty digit string in `base`, with an optional
/// leading `-` except in bases 64 and 85, where `+` and `-` are digits
/// (`bvn_validate_digits_for_base`).
fn validate_digits_for_base(s: &[u8], base: u32) -> bool {
    let digits = match s {
        [b'-', rest @ ..] if base != 64 && base != 85 => rest,
        _ => s,
    };
    !digits.is_empty() && digits.iter().all(|&c| char_to_digit(c, base) < base)
}

/// Whether `s` is a well-formed numeric literal in `base`: optional sign,
/// mantissa with at most one `.`, optional exponent (`e`/`E` for bases up to
/// 14, `p`/`P` for power-of-two bases up to 16; the exponent is decimal with
/// an optional sign) (`bvn_validate_number_in_base`).
///
/// Bases 64 and 85 take no float syntax at all: their alphabets contain the
/// sign and dot characters, so the literal must be a pure digit string.
pub(crate) fn validate_number_in_base(s: &[u8], base: u32) -> bool {
    if s.is_empty() {
        return false;
    }
    if base == 64 || base == 85 {
        return validate_digits_for_base(s, base);
    }
    let mut i = usize::from(matches!(s.first(), Some(b'-' | b'+')));
    if i >= s.len() {
        return false;
    }
    let exp_e = base <= 14;
    let exp_p = base & base.wrapping_sub(1) == 0 && base <= 16;
    let (mut has_dot, mut has_exp) = (false, false);
    let (mut has_mant_digit, mut has_exp_digit) = (false, false);
    while let Some(&c) = s.get(i) {
        if c == b'.' {
            if has_dot || has_exp {
                return false;
            }
            has_dot = true;
        } else if (matches!(c, b'e' | b'E') && exp_e) || (matches!(c, b'p' | b'P') && exp_p) {
            if has_exp || !has_mant_digit {
                return false;
            }
            has_exp = true;
            if matches!(s.get(i + 1), Some(b'+' | b'-')) {
                i += 1;
            }
        } else if c == b'+' || c == b'-' {
            return false;
        } else if has_exp {
            if !c.is_ascii_digit() {
                return false;
            }
            has_exp_digit = true;
        } else {
            if char_to_digit(c, base) >= base {
                return false;
            }
            has_mant_digit = true;
        }
        i += 1;
    }
    has_mant_digit && (!has_exp || has_exp_digit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_per_base() {
        assert_eq!(char_to_digit(b'7', 8), 7);
        assert_eq!(char_to_digit(b'8', 8), 8);
        assert_eq!(char_to_digit(b'F', 16), 15);
        assert_eq!(char_to_digit(b'f', 16), 15);
        assert_eq!(char_to_digit(b'z', 36), 35);
        assert_eq!(char_to_digit(b'Z', 36), 35);
        assert_eq!(char_to_digit(b'Z', 62), 61);
        assert_eq!(char_to_digit(b'a', 62), 10);
        assert_eq!(char_to_digit(b'A', 62), 36);
        assert_eq!(char_to_digit(b'A', 64), 0);
        assert_eq!(char_to_digit(b'0', 64), 52);
        assert_eq!(char_to_digit(b'/', 64), 63);
        assert_eq!(char_to_digit(b'=', 64), 64);
        assert_eq!(char_to_digit(b'!', 85), 0);
        assert_eq!(char_to_digit(b'u', 85), 84);
        assert_eq!(char_to_digit(b'v', 85), 85);
        assert_eq!(char_to_digit(b'-', 10), 10);
    }

    #[test]
    fn specials() {
        assert!(is_special_number(b"inf"));
        assert!(is_special_number(b"nan"));
        assert!(is_special_number(b"ninf"));
        assert!(!is_special_number(b"-inf"));
        assert!(!is_special_number(b"infinity"));
        assert!(!is_special_number(b"NaN"));
        assert!(!is_special_number(b""));
    }

    #[test]
    fn number_syntax() {
        assert!(validate_number_in_base(b"-1.5e+3", 10));
        assert!(validate_number_in_base(b"+.5", 10));
        assert!(validate_number_in_base(b"5.", 10));
        assert!(!validate_number_in_base(b".", 10));
        assert!(!validate_number_in_base(b"-", 10));
        assert!(!validate_number_in_base(b"", 10));
        assert!(!validate_number_in_base(b"1e", 10));
        assert!(!validate_number_in_base(b"e5", 10));
        assert!(!validate_number_in_base(b"1.2.3", 10));
        assert!(!validate_number_in_base(b"1e5.0", 10));
        assert!(!validate_number_in_base(b"1e2e3", 10));
        assert!(validate_number_in_base(b"1.8p-3", 16));
        assert!(validate_number_in_base(b"1e5", 16));
        assert!(validate_number_in_base(b"ff", 16));
        assert!(!validate_number_in_base(b"1p3", 10));
        assert!(validate_number_in_base(b"101p2", 2));
        assert!(validate_number_in_base(b"1e2", 2));
        assert!(validate_number_in_base(b"1e2", 15) && !validate_number_in_base(b"1f", 15));
        // Base 64/85: punctuation is a digit, '+' and '-' included.
        assert!(validate_number_in_base(b"+/==", 85));
        assert!(validate_number_in_base(b"-1", 85));
        assert!(validate_number_in_base(b"+/", 64));
        assert!(!validate_number_in_base(b"-1", 64));
        assert!(!validate_number_in_base(b"a=", 64));
    }
}
