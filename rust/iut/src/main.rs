//! Conformance IUT adapter for bovnar-rs, protocol `bvnr-conformance-v1`
//! (upstream `doc/13_bovnar_conformance.md` §6–7).
//!
//! Reads a Bovnar document from stdin and writes the event log to stdout
//! (exit 0), or a single `ERROR <code_name>` line (exit 1). Mirrors the
//! reference adapter `tests/bvnr_conformance_iut.c`, including how it reads
//! its limit options, so the upstream driver can run against this binary:
//!
//! ```text
//! bvnr_conformance --iut target/release/bovnar-iut
//! ```

use std::io::{self, Read, Write};
use std::process::ExitCode;

use bovnar::Limits;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let limits = limits_from_args(&args);

    let mut input = Vec::new();
    if io::stdin().lock().read_to_end(&mut input).is_err() {
        return respond(b"ERROR read_failed\n", ExitCode::FAILURE);
    }

    match bovnar::conformance::event_log(&input, &limits) {
        Ok(log) => respond(log.as_bytes(), ExitCode::SUCCESS),
        Err(err) => respond(
            format!("ERROR {}\n", err.kind().as_str()).as_bytes(),
            ExitCode::FAILURE,
        ),
    }
}

fn respond(out: &[u8], code: ExitCode) -> ExitCode {
    let mut stdout = io::stdout().lock();
    if stdout.write_all(out).and_then(|()| stdout.flush()).is_err() {
        eprintln!("bovnar-iut: failed to write to stdout");
        return ExitCode::FAILURE;
    }
    code
}

/// Applies the `--max-*` options of doc/13 §6.4 to the default limits.
///
/// Follows the C adapter exactly: an option takes the next argument as its
/// value, unknown options are ignored, a value that is not a plain decimal
/// number counts as 0, values are truncated to the width of the C field, and
/// 0 keeps the default.
fn limits_from_args(args: &[String]) -> Limits {
    let defaults = Limits::default();
    let mut limits = defaults.clone();
    let mut i = 0;
    while i + 1 < args.len() {
        let value = parse_u64(&args[i + 1]);
        let known = match args[i].as_str() {
            "--max-id" => set(
                &mut limits.max_identifier_length,
                value as u16,
                defaults.max_identifier_length,
            ),
            "--max-str" => set(
                &mut limits.max_string_length,
                value as u16,
                defaults.max_string_length,
            ),
            "--max-num" => set(
                &mut limits.max_number_length,
                value as u16,
                defaults.max_number_length,
            ),
            "--max-sym" => set(
                &mut limits.max_symbol_length,
                value as u16,
                defaults.max_symbol_length,
            ),
            "--max-ref" => set(
                &mut limits.max_reference_length,
                value as u16,
                defaults.max_reference_length,
            ),
            "--max-snest" => set(
                &mut limits.max_struct_nesting,
                value as u8,
                defaults.max_struct_nesting,
            ),
            "--max-anest" => set(
                &mut limits.max_array_nesting,
                value as u8,
                defaults.max_array_nesting,
            ),
            "--max-aitems" => set(&mut limits.max_array_items, value, defaults.max_array_items),
            _ => false,
        };
        i += if known { 2 } else { 1 };
    }
    limits
}

/// Stores `value`, where 0 means the default, as in the C flags. A repeated
/// option overrides the earlier one even with 0: C stores the 0 in the flag
/// and only then falls back to the default.
fn set<T: PartialEq + Default>(field: &mut T, value: T, default: T) -> bool {
    *field = if value == T::default() {
        default
    } else {
        value
    };
    true
}

/// Parses a decimal number spanning the whole argument, saturating on
/// overflow like the C adapter's `strtoull`; anything else is 0.
fn parse_u64(s: &str) -> u64 {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return 0;
    }
    s.parse().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn applies_known_options_and_ignores_unknown_ones() {
        let limits = limits_from_args(&args(&[
            "--verbose",
            "--max-id",
            "5",
            "--max-snest",
            "2",
            "--max-aitems",
            "3",
        ]));
        assert_eq!(limits.max_identifier_length, 5);
        assert_eq!(limits.max_struct_nesting, 2);
        assert_eq!(limits.max_array_items, 3);
        assert_eq!(
            limits.max_string_length,
            Limits::default().max_string_length
        );
    }

    #[test]
    fn zero_or_garbage_keeps_the_default_and_wide_values_truncate() {
        let limits = limits_from_args(&args(&[
            "--max-id",
            "0",
            "--max-str",
            "5x",
            "--max-snest",
            "258",
        ]));
        assert_eq!(limits.max_identifier_length, 255);
        assert_eq!(limits.max_string_length, 65_535);
        assert_eq!(limits.max_struct_nesting, 2);
    }

    #[test]
    fn trailing_option_without_value_is_ignored() {
        assert_eq!(limits_from_args(&args(&["--max-id"])), Limits::default());
    }

    #[test]
    fn repeated_option_with_zero_restores_the_default() {
        let limits = limits_from_args(&args(&["--max-id", "2", "--max-id", "x"]));
        assert_eq!(limits.max_identifier_length, 255);
        let limits = limits_from_args(&args(&["--max-aitems", "3", "--max-aitems", "0"]));
        assert_eq!(limits, Limits::default());
    }
}
