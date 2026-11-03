//! Reading a caller's `std::io::Read` for the entry points that mirror
//! Python's file-descriptor ones (`read_fd`, `parse_fd`).
//!
//! The C library takes input from an fd or from memory only: `bvnr_pull_fn`
//! exists, but `bovnar.h` has no constructor for a source that calls one (the
//! source layout is private to the library). So the input is read into memory
//! here and handed to the memory API, which reads it exactly as the fd source
//! would, `max_file_size` included.

use std::io::Read;

use crate::enums::ErrorCode;
use crate::error::{Error, Result};

/// Reads `input` to its end, or to one byte past `cap` when there is one,
/// so that a caller can tell an input over the cap from one at it. An I/O
/// error is `reading_from_source_fd`, the code the C fd source gives.
pub(crate) fn read_capped(mut input: impl Read, cap: Option<u64>) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    let result = match cap {
        Some(cap) => input
            .by_ref()
            .take(cap.saturating_add(1))
            .read_to_end(&mut data),
        None => input.read_to_end(&mut data),
    };
    match result {
        Ok(_) => Ok(data),
        Err(e) => Err(Error::io(ErrorCode::ReadingFromSourceFd, &e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_everything_without_a_cap() {
        assert_eq!(read_capped(&b"abcdef"[..], None).unwrap(), b"abcdef");
    }

    #[test]
    fn reads_one_byte_past_the_cap() {
        assert_eq!(read_capped(&b"abcdef"[..], Some(6)).unwrap(), b"abcdef");
        assert_eq!(read_capped(&b"abcdef"[..], Some(3)).unwrap(), b"abcd");
        assert_eq!(
            read_capped(std::io::repeat(b'x'), Some(9)).unwrap().len(),
            10
        );
    }

    struct Failing;

    impl Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("disk on fire"))
        }
    }

    #[test]
    fn an_io_error_keeps_its_kind_and_message() {
        let err = read_capped(Failing, None).unwrap_err();
        assert_eq!(err.kind(), Some(ErrorCode::ReadingFromSourceFd));
        assert_eq!(
            err,
            Error::Io {
                code: ErrorCode::ReadingFromSourceFd,
                io_kind: std::io::ErrorKind::Other,
                message: "disk on fire".into(),
            }
        );
        assert_eq!(
            err.to_string(),
            "I/O error: reading_from_source_fd (disk on fire)"
        );
    }
}
