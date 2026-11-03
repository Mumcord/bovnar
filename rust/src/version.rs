//! Bovnar specification versions (`#!bovnar M.N`, spec §3.4).

/// A specification version a document declares or a reader assumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpecVersion {
    /// Major version.
    pub major: u16,
    /// Minor version.
    pub minor: u16,
}

impl SpecVersion {
    /// Spec 1.0.
    pub const V1_0: Self = Self { major: 1, minor: 0 };
    /// Spec 1.1 (version directive, datetime family).
    pub const V1_1: Self = Self { major: 1, minor: 1 };
    /// Spec 1.2 (unit profiles).
    pub const V1_2: Self = Self { major: 1, minor: 2 };
}
