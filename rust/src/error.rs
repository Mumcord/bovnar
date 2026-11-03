//! The error type, mirroring `python/bovnar/exceptions.py`.

use std::fmt;

use crate::enums::ErrorCode;
use crate::ffi;

/// The C library's error names (`error_code_t`): the kind of an [`Error`]
/// that has one, as [`Error::kind`] reports it.
pub type ErrorKind = ErrorCode;

/// Where in the input a parse error was raised.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Position {
    pub line: u64,
    pub column: u64,
    /// The byte offset from the start of the input.
    pub offset: u64,
    /// The byte the error was raised on.
    pub byte: u32,
}

/// Everything a bovnar call can fail with: Python's exception classes in
/// `exceptions.py`, each with the library's error code where there is one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The reader or DOM parser refused the input (`BovnarParseError`).
    /// `position` is `None` where the library reports none: the DOM parser
    /// gives only the code, and so do the stream helpers' framing errors.
    Parse {
        code: i32,
        position: Option<Position>,
    },
    /// The writer refused an event or ran out of room (`BovnarWriteError`).
    Write { code: i32, offset: u64 },
    /// A caller-side mistake caught before or by the library
    /// (`BovnarArgumentError`): `code` names the library's verdict where it
    /// gave one (an unparsable unit, a refused unit policy), and `message`
    /// says what was wrong.
    Argument {
        code: Option<ErrorKind>,
        message: String,
    },
    /// The library could not allocate (Python's `MemoryError`); names the
    /// call that failed.
    OutOfMemory(&'static str),
    /// The `std::io::Read` or `std::io::Write` behind a `*_reader` /
    /// `to_writer` entry point failed. `code` is what the C library reports
    /// for an unreadable fd or an unwritable fd sink in Python's `read_fd`,
    /// `parse_fd` and `to_fd` (`reading_from_source_fd`, `writing_to_sink`);
    /// `io_kind` and `message` keep what the I/O error said.
    Io {
        code: ErrorKind,
        io_kind: std::io::ErrorKind,
        message: String,
    },
}

impl Error {
    /// An argument error the library gave no code for.
    pub fn argument(message: impl Into<String>) -> Self {
        Error::Argument {
            code: None,
            message: message.into(),
        }
    }

    /// An argument error with the library's code.
    pub fn argument_with(code: ErrorKind, message: impl Into<String>) -> Self {
        Error::Argument {
            code: Some(code),
            message: message.into(),
        }
    }

    /// An I/O error of the caller's reader or writer, under the library's
    /// `code` for that failure.
    pub(crate) fn io(code: ErrorKind, err: &std::io::Error) -> Self {
        Error::Io {
            code,
            io_kind: err.kind(),
            message: err.to_string(),
        }
    }

    /// A parse error the library reported without a position.
    pub(crate) fn parse_code(code: i32) -> Self {
        Error::Parse {
            code,
            position: None,
        }
    }

    /// The raw C error code, or `None` where the library gave none.
    pub fn code_raw(&self) -> Option<i32> {
        match self {
            Error::Parse { code, .. } | Error::Write { code, .. } => Some(*code),
            Error::Argument { code, .. } => code.map(i32::from),
            Error::Io { code, .. } => Some(i32::from(*code)),
            Error::OutOfMemory(_) => None,
        }
    }

    /// The kind: the C error name, when the library gave a code (Python's
    /// `.code`). `None` for an argument error caught before the library and
    /// for an allocation failure.
    pub fn kind(&self) -> Option<ErrorKind> {
        self.code_raw().and_then(ErrorCode::from_raw)
    }

    /// Same as [`Error::kind`] (the name Python uses).
    pub fn code(&self) -> Option<ErrorCode> {
        self.kind()
    }

    /// Where a parse error was raised, when the library said.
    pub fn position(&self) -> Option<Position> {
        match self {
            Error::Parse { position, .. } => *position,
            _ => None,
        }
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
            Error::Parse { code, position } => {
                write!(f, "Parse error: {} (code {code})", ffi::error_name(*code))?;
                if let Some(p) = position {
                    write!(
                        f,
                        " at line {} col {} offset {}",
                        p.line, p.column, p.offset
                    )?;
                }
                Ok(())
            }
            Error::Write { code, offset } => write!(
                f,
                "Write error: {} (code {code}) at offset {offset}",
                ffi::error_name(*code)
            ),
            Error::Argument { message, .. } => f.write_str(message),
            Error::OutOfMemory(call) => write!(f, "out of memory: {call} failed"),
            Error::Io { code, message, .. } => write!(
                f,
                "I/O error: {} ({message})",
                ffi::error_name(i32::from(*code))
            ),
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
