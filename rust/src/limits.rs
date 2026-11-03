//! Resource limits for reading untrusted input.

/// Upper bounds the reader enforces while parsing.
///
/// The first eight fields are the reader limits of the conformance protocol
/// (upstream `doc/13_bovnar_conformance.md` §6.4). Their types and defaults
/// match the C reference (`bvnr_read_flags_t` and the internal defaults in
/// `src/lexer/bvn_val_impl.h`), so a document is accepted or refused exactly
/// where the reference accepts or refuses it. The last two fields are specific
/// to bovnar-rs and cap total memory use.
///
/// Lengths are counted in bytes. Start from [`Limits::default`] and tighten
/// what you need:
///
/// ```
/// use bovnar::Limits;
/// let mut limits = Limits::default();
/// limits.max_string_length = 1024;
/// limits.max_input_bytes = 64 * 1024;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Limits {
    /// Longest identifier (struct key). Default: 255.
    pub max_identifier_length: u16,
    /// Longest string literal. Default: 65535.
    pub max_string_length: u16,
    /// Longest number literal. Default: 65535.
    pub max_number_length: u16,
    /// Longest symbol. Default: 255.
    pub max_symbol_length: u16,
    /// Longest reference. Default: 65535.
    pub max_reference_length: u16,
    /// Deepest struct nesting. Default: 64.
    pub max_struct_nesting: u8,
    /// Deepest array nesting. Default: 64.
    pub max_array_nesting: u8,
    /// Most items in one array. Default: 2147483647.
    pub max_array_items: u64,
    /// Largest input in bytes, octet-stream payloads included. Default: 16 MiB.
    ///
    /// The C reference is unlimited by default; bovnar-rs is not, because it
    /// reads untrusted network data and a bounded default is the safe one.
    pub max_input_bytes: u64,
    /// Most nodes a materialised document may hold. Default: 1048576.
    ///
    /// Bounds the memory of a fully built document independently of input
    /// size, since many tiny values cost far more as nodes than as bytes.
    pub max_dom_nodes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_identifier_length: 255,
            max_string_length: 65_535,
            max_number_length: 65_535,
            max_symbol_length: 255,
            max_reference_length: 65_535,
            max_struct_nesting: 64,
            max_array_nesting: 64,
            max_array_items: 2_147_483_647,
            max_input_bytes: 16 * 1024 * 1024,
            max_dom_nodes: 1 << 20,
        }
    }
}
