//! How a document is read: limits plus the C reader's policy flags.

use crate::Limits;

/// Everything that controls a read: the [`Limits`] and the policy flags of the
/// C reference's `bvnr_read_flags_t` (upstream `doc/08` §1.8 and §1.13).
///
/// All flags are off by default, as in C. Build it with the chained setters:
///
/// ```
/// use bovnar::{ErrorKind, ReadOptions};
///
/// let options = ReadOptions::new().strict_version(true).text_only(true);
/// let err = bovnar::parse_with_options(b"#!bovnar 1.3\n.a = 1;", &options).unwrap_err();
/// assert_eq!(err.kind(), ErrorKind::UnsupportedSpecVersion);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReadOptions {
    /// Resource limits.
    pub limits: Limits,
    /// Refuse a `#!bovnar M.N` directive naming a version this crate does not
    /// support (another major, or a minor above 2) with
    /// [`ErrorKind::UnsupportedSpecVersion`](crate::ErrorKind::UnsupportedSpecVersion).
    /// When off, such a version is recorded
    /// ([`Reader::declared_version`](crate::Reader::declared_version)) but
    /// accepted. A malformed directive is always
    /// [`ErrorKind::InvalidSpecVersion`](crate::ErrorKind::InvalidSpecVersion).
    pub strict_version: bool,
    /// Refuse any octet stream with
    /// [`ErrorKind::OctetStreamForbidden`](crate::ErrorKind::OctetStreamForbidden),
    /// raised at the stream's opening `0x00` before any payload is read. For
    /// channels that must stay text: line-ending normalisation silently
    /// corrupts binary payloads. A document without an octet stream reads
    /// identically either way.
    pub text_only: bool,
    /// Recovery mode (`continue_on_error`): after an error the reader skips
    /// to the next point it can resynchronise on, reports the error as
    /// [`Event::Recovered`](crate::Event::Recovered) and carries on.
    pub recovery: bool,
}

impl ReadOptions {
    /// Default limits, all flags off.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the limits.
    #[must_use]
    pub fn limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// Sets [`ReadOptions::strict_version`].
    #[must_use]
    pub fn strict_version(mut self, on: bool) -> Self {
        self.strict_version = on;
        self
    }

    /// Sets [`ReadOptions::text_only`].
    #[must_use]
    pub fn text_only(mut self, on: bool) -> Self {
        self.text_only = on;
        self
    }

    /// Sets [`ReadOptions::recovery`].
    #[must_use]
    pub fn recovery(mut self, on: bool) -> Self {
        self.recovery = on;
        self
    }
}

impl From<Limits> for ReadOptions {
    fn from(limits: Limits) -> Self {
        Self::new().limits(limits)
    }
}
