//! Format sniffing: is a byte buffer JSON or Bovnar?

/// What [`detect`] recognised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Format {
    /// Starts like a JSON object or array.
    Json,
    /// Starts like a Bovnar document.
    Bovnar,
    /// Neither, or too little to tell.
    Unknown,
}

/// How far [`detect`] looks before giving up: a long run of blank lines or
/// comments does not make it read the whole input.
const SNIFF_LIMIT: usize = 64 * 1024;

/// Guesses the format of `input` from its first significant bytes.
///
/// The rules, in order, after skipping a UTF-8 byte-order mark and any
/// whitespace (space, tab, LF, CR, VT, FF):
///
/// 1. `#!bovnar` (the version directive) is Bovnar.
/// 2. `#` starts a Bovnar comment, which runs to the next LF or CR; it is
///    skipped, and the scan continues. Comments alone are Bovnar.
/// 3. `.` followed by an identifier start (an ASCII letter, `_`, or a UTF-8
///    leader byte 0xC3 to 0xF4) is a Bovnar assignment.
/// 4. `{` or `[` is JSON, unless a comment came first (JSON has none).
/// 5. Anything else, and empty or blank input, is [`Format::Unknown`].
///
/// Only the first 64 KiB are examined, so the cost is bounded by that
/// prefix; a buffer whose first significant byte lies beyond it is
/// `Unknown`. A positive answer is a guess, not a validation: parse the
/// input to know whether it is well-formed.
///
/// ```
/// use bovnar::{detect, Format};
///
/// assert_eq!(detect(b"  {\"a\": 1}"), Format::Json);
/// assert_eq!(detect(b"#!bovnar 1.2\n.a = 1;"), Format::Bovnar);
/// assert_eq!(detect(b"# settings\n.a = 1;"), Format::Bovnar);
/// assert_eq!(detect(b"hello"), Format::Unknown);
/// ```
pub fn detect(input: &[u8]) -> Format {
    let input = input.get(..SNIFF_LIMIT).unwrap_or(input);
    let mut rest = input.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(input);
    let mut saw_comment = false;
    loop {
        let blank = rest
            .iter()
            .take_while(|&&b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
            .count();
        rest = rest.get(blank..).unwrap_or_default();
        match rest {
            [] => {
                return if saw_comment {
                    Format::Bovnar
                } else {
                    Format::Unknown
                }
            }
            [b'#', b'!', b'b', b'o', b'v', b'n', b'a', b'r', ..] if !saw_comment => {
                return Format::Bovnar
            }
            [b'#', ..] => {
                saw_comment = true;
                let line = rest
                    .iter()
                    .position(|&b| b == b'\n' || b == b'\r')
                    .unwrap_or(rest.len());
                rest = rest.get(line..).unwrap_or_default();
            }
            [b'.', next, ..]
                if next.is_ascii_alphabetic() || *next == b'_' || (0xc3..=0xf4).contains(next) =>
            {
                return Format::Bovnar
            }
            [b'{' | b'[', ..] if !saw_comment => return Format::Json,
            _ => return Format::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs() {
        assert_eq!(detect(b""), Format::Unknown);
        assert_eq!(detect(b" \n\t"), Format::Unknown);
        assert_eq!(detect(b"\xEF\xBB\xBF[1]"), Format::Json);
        assert_eq!(detect(b"\xEF\xBB\xBF.k = 1;"), Format::Bovnar);
        assert_eq!(detect(b"# only a comment"), Format::Bovnar);
        assert_eq!(detect(b"# c\n{\"a\":1}"), Format::Unknown);
        assert_eq!(detect(b"# c\r\n#!bovnar 1.2\n.a=1;"), Format::Bovnar);
        assert_eq!(detect(b".1"), Format::Unknown);
        assert_eq!(detect(b"."), Format::Unknown);
        assert_eq!(detect(b"\"str\""), Format::Unknown);
        assert_eq!(detect(b".\xc3\xa4 = 1;"), Format::Bovnar);
    }

    #[test]
    fn bounded_prefix() {
        let mut long = alloc::vec![b' '; SNIFF_LIMIT];
        long.extend_from_slice(b"{}");
        assert_eq!(detect(&long), Format::Unknown);
    }
}
