//! The error type, mirroring `python/bovnar/exceptions.py`.

use std::fmt;

use crate::enums::ErrorCode;
use crate::ffi;

/// Everything a bovnar call can fail with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The reader or DOM parser refused the input (`BovnarParseError`).
    Parse {
        code: i32,
        line: u64,
        column: u64,
        offset: u64,
        byte: u32,
    },
    /// The writer refused an event or ran out of room (`BovnarWriteError`).
    Write { code: i32, offset: u64 },
    /// A caller-side mistake caught before reaching the library
    /// (`BovnarArgumentError`).
    Argument(String),
}

impl Error {
    /// The raw C error code, or `None` for an argument error.
    pub fn code_raw(&self) -> Option<i32> {
        match self {
            Error::Parse { code, .. } | Error::Write { code, .. } => Some(*code),
            Error::Argument(_) => None,
        }
    }

    /// The error code as an enum, when the binding knows it.
    pub fn code(&self) -> Option<ErrorCode> {
        self.code_raw().and_then(ErrorCode::from_raw)
    }

    /// The C name of the error code (`bvn_error_to_string`), such as
    /// `"unexpected_input_byte"`.
    pub fn name(&self) -> Option<&'static str> {
        self.code_raw().map(ffi::error_name)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse {
                code,
                line,
                column,
                offset,
                ..
            } => {
                write!(f, "Parse error: {} (code {code})", ffi::error_name(*code))?;
                if *line != 0 {
                    write!(f, " at line {line} col {column} offset {offset}")?;
                }
                Ok(())
            }
            Error::Write { code, offset } => write!(
                f,
                "Write error: {} (code {code}) at offset {offset}",
                ffi::error_name(*code)
            ),
            Error::Argument(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for Error {}

/// Shorthand for results of this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    fn snake(camel: &str) -> String {
        let mut out = String::new();
        for (i, ch) in camel.chars().enumerate() {
            if ch.is_ascii_uppercase() && i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        }
        out
    }

    /// The Rust enum and the C names agree, code by code. The variants follow
    /// the C enumerators; bvn_error_to_string spells one of them shorter.
    #[test]
    fn error_codes_match_c_names() {
        let mut raw = 0;
        while let Some(code) = ErrorCode::from_raw(raw) {
            let variant = snake(&format!("{code:?}"));
            let variant = variant.strip_prefix("got_").unwrap_or(&variant);
            assert_eq!(variant, ffi::error_name(raw), "code {raw}");
            raw += 1;
        }
        assert!(raw > 50, "only {raw} codes mirrored");
    }
}
