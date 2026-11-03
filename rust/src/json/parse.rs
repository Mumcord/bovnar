//! The JSON reader of `bovnar convert`, ported from the C CLI
//! (`json_parse_value_depth` and friends in upstream `src/bovnar.c`).
//!
//! It keeps the reference's exact acceptance, quirks included, because the
//! converter's output is defined by it:
//!
//! - The input ends at its first NUL byte (the reference parses a
//!   NUL-terminated buffer); anything after it is trailing content.
//! - An array or object still open when the input ends after `[`, `{` or a
//!   `,` is accepted as closed (the reference's loops stop at the
//!   terminator). See `UPSTREAM_NOTES.md`.
//! - Whitespace is C `isspace`: space, tab, LF, VT, FF, CR.
//! - At most 512 nested containers.
//!
//! The tree is an arena of nodes and the parser keeps an explicit stack, so
//! nothing recurses on input depth.

use alloc::string::String;
use alloc::vec::Vec;

use super::JsonError;

/// The reference's `JSON_MAX_DEPTH`: a value may sit inside at most this
/// many containers.
pub(super) const MAX_DEPTH: usize = 512;

/// One JSON value. Containers refer to their children by arena index.
pub(super) enum Node {
    Null,
    Bool(bool),
    /// A non-negative integer literal.
    Uint(u64),
    /// A negative integer literal (and `-0`).
    Int(i64),
    /// A literal with a fraction or an exponent.
    Float(f64),
    Str(Vec<u8>),
    Array(Vec<usize>),
    Object(Vec<(Vec<u8>, usize)>),
}

/// A parsed JSON document: `nodes[root]` is the top-level value.
pub(super) struct Tree {
    pub(super) nodes: Vec<Node>,
    pub(super) root: usize,
}

impl Tree {
    pub(super) fn node(&self, i: usize) -> &Node {
        // Indices come from the parser and are always in range.
        self.nodes.get(i).unwrap_or(&Node::Null)
    }
}

/// An open container on the parser's stack.
struct Frame {
    node: usize,
    /// The key of the member currently being parsed (objects only).
    key: Option<Vec<u8>>,
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
    nodes: Vec<Node>,
    stack: Vec<Frame>,
}

/// C `isspace` in the C locale.
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// Parses one JSON value from the start of `input`. Returns the tree and the
/// offset just past the value and any whitespace after it.
pub(super) fn parse(input: &[u8]) -> Result<(Tree, usize), JsonError> {
    let mut p = Parser {
        input,
        pos: 0,
        nodes: Vec::new(),
        stack: Vec::new(),
    };
    let root = p.run()?;
    p.skip_ws();
    let end = p.pos;
    Ok((
        Tree {
            nodes: p.nodes,
            root,
        },
        end,
    ))
}

impl Parser<'_> {
    /// The byte at `i`, or 0 past the end (the reference's terminator).
    fn at(&self, i: usize) -> u8 {
        self.input.get(i).copied().unwrap_or(0)
    }

    fn cur(&self) -> u8 {
        self.at(self.pos)
    }

    fn skip_ws(&mut self) {
        while self.cur() != 0 && is_space(self.cur()) {
            self.pos += 1;
        }
    }

    fn syntax(&self) -> JsonError {
        JsonError::Syntax {
            offset: self.pos as u64,
        }
    }

    fn push(&mut self, node: Node) -> usize {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// The key the value being parsed sits under, for diagnostics: the
    /// innermost object member, `None` at the top level.
    fn current_key(&self) -> Option<String> {
        self.stack
            .iter()
            .rev()
            .find_map(|f| f.key.as_deref())
            .map(|k| String::from_utf8_lossy(k).into_owned())
    }

    fn run(&mut self) -> Result<usize, JsonError> {
        // Parse a value; when it completes, attach it to the innermost open
        // container and decide what that container expects next.
        'value: loop {
            let mut done = self.value()?;
            loop {
                let Some(top) = self.stack.last() else {
                    return Ok(done);
                };
                let parent = top.node;
                let is_object = matches!(self.nodes.get(parent), Some(Node::Object(_)));
                let key = self.stack.last_mut().and_then(|f| f.key.take());
                match (self.nodes.get_mut(parent), key) {
                    (Some(Node::Object(members)), Some(key)) => members.push((key, done)),
                    (Some(Node::Array(items)), _) => items.push(done),
                    _ => return Err(self.syntax()),
                }
                self.skip_ws();
                let close = if is_object { b'}' } else { b']' };
                if self.cur() == b',' {
                    self.pos += 1;
                    self.skip_ws();
                    if self.cur() == 0 {
                        // The reference's loop condition: the container
                        // ends with the input.
                        self.stack.pop();
                        done = parent;
                        continue;
                    }
                    if is_object {
                        self.member_key()?;
                    }
                    continue 'value;
                }
                if self.cur() == close {
                    self.pos += 1;
                    self.stack.pop();
                    done = parent;
                    continue;
                }
                return Err(self.syntax());
            }
        }
    }

    /// Reads `"key"` `:` for the innermost (object) frame.
    fn member_key(&mut self) -> Result<(), JsonError> {
        let key = self.string()?;
        self.skip_ws();
        if self.cur() != b':' {
            return Err(self.syntax());
        }
        self.pos += 1;
        self.skip_ws();
        if let Some(top) = self.stack.last_mut() {
            top.key = Some(key);
        }
        Ok(())
    }

    /// Parses a scalar, or opens a container. Returns the index of a
    /// completed value; for a non-empty container it pushes a frame and
    /// loops until a scalar or an empty container completes.
    fn value(&mut self) -> Result<usize, JsonError> {
        loop {
            self.skip_ws();
            if self.cur() == 0 {
                return Err(self.syntax());
            }
            if self.stack.len() > MAX_DEPTH {
                return Err(JsonError::TooDeep {
                    offset: self.pos as u64,
                });
            }
            match self.cur() {
                b'"' => {
                    let s = self.string()?;
                    return Ok(self.push(Node::Str(s)));
                }
                c @ (b'[' | b'{') => {
                    self.pos += 1;
                    let is_object = c == b'{';
                    let node = self.push(if is_object {
                        Node::Object(Vec::new())
                    } else {
                        Node::Array(Vec::new())
                    });
                    self.skip_ws();
                    let close = if is_object { b'}' } else { b']' };
                    if self.cur() == close {
                        self.pos += 1;
                        return Ok(node);
                    }
                    if self.cur() == 0 {
                        // Unterminated at the end of the input: accepted.
                        return Ok(node);
                    }
                    self.stack.push(Frame { node, key: None });
                    if is_object {
                        self.member_key()?;
                    }
                }
                _ => return self.scalar(),
            }
        }
    }

    fn scalar(&mut self) -> Result<usize, JsonError> {
        let rest = self.input.get(self.pos..).unwrap_or_default();
        for (word, node) in [
            (&b"null"[..], Node::Null),
            (b"true", Node::Bool(true)),
            (b"false", Node::Bool(false)),
        ] {
            if rest.starts_with(word) {
                self.pos += word.len();
                return Ok(self.push(node));
            }
        }
        let c = self.cur();
        if c != b'-' && !c.is_ascii_digit() {
            return Err(self.syntax());
        }
        let (len, is_float) = scan_number(rest).ok_or_else(|| self.syntax())?;
        let text = rest.get(..len).unwrap_or_default();
        // The scanned text is ASCII by construction.
        let text = core::str::from_utf8(text).unwrap_or_default();
        let node = if is_float {
            let v: f64 = text.parse().map_err(|_| self.syntax())?;
            if v.is_infinite() {
                return Err(JsonError::FloatOverflow {
                    key: self.current_key(),
                });
            }
            Node::Float(v)
        } else {
            // The reference converts with strtoll/strtoull, which read the
            // whole digit run: `0999...` past 64 bits is out of range even
            // though the number token ends after the `0`.
            let sign = usize::from(text.starts_with('-'));
            let run = sign
                + rest
                    .iter()
                    .skip(sign)
                    .take_while(|b| b.is_ascii_digit())
                    .count();
            let digits =
                core::str::from_utf8(rest.get(..run).unwrap_or_default()).unwrap_or_default();
            let out_of_range = JsonError::IntegerOutOfRange {
                offset: self.pos as u64,
            };
            if sign == 1 {
                digits.parse::<i64>().map_err(|_| out_of_range)?;
                Node::Int(text.parse().unwrap_or(0))
            } else {
                digits.parse::<u64>().map_err(|_| out_of_range)?;
                Node::Uint(text.parse().unwrap_or(0))
            }
        };
        self.pos += len;
        Ok(self.push(node))
    }

    /// `parse_json_string`: a two-pass port. The first pass finds the closing
    /// quote, skipping four bytes after every `\u`; the second decodes.
    fn string(&mut self) -> Result<Vec<u8>, JsonError> {
        if self.cur() != b'"' {
            return Err(self.syntax());
        }
        self.pos += 1;
        let start = self.pos;
        loop {
            match self.cur() {
                0 | b'"' => break,
                b'\\' => {
                    self.pos += 1;
                    if self.cur() == 0 {
                        return Err(self.syntax());
                    }
                    if self.cur() == b'u' {
                        if (1..=4).any(|k| self.at(self.pos + k) == 0) {
                            return Err(self.syntax());
                        }
                        self.pos += 4;
                    }
                }
                _ => {}
            }
            self.pos += 1;
        }
        if self.cur() != b'"' {
            return Err(self.syntax());
        }
        let end = self.pos;
        let raw = self.input.get(start..end).unwrap_or_default();
        let out = decode_string(raw).map_err(|nul| {
            if nul {
                JsonError::Nul {
                    offset: start as u64,
                }
            } else {
                JsonError::Syntax {
                    offset: start as u64,
                }
            }
        })?;
        self.pos += 1;
        Ok(out)
    }
}

fn hex4(s: &[u8]) -> Option<u32> {
    if s.len() < 4 {
        return None;
    }
    s.iter().take(4).try_fold(0u32, |v, &c| {
        let d = char::from(c).to_digit(16)?;
        Some((v << 4) | d)
    })
}

/// Decodes the body of a JSON string. `Err(true)` for `\u0000`, `Err(false)`
/// for any other defect.
fn decode_string(raw: &[u8]) -> Result<Vec<u8>, bool> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while let Some(&c) = raw.get(i) {
        if c == b'\\' {
            i += 1;
            let e = *raw.get(i).ok_or(false)?;
            match e {
                b'"' | b'\\' | b'/' => out.push(e),
                b'b' => out.push(0x08),
                b'f' => out.push(0x0c),
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'u' => {
                    let mut cp = hex4(raw.get(i + 1..).unwrap_or_default()).ok_or(false)?;
                    i += 4;
                    if (0xd800..=0xdbff).contains(&cp) {
                        let tail = raw.get(i + 1..).unwrap_or_default();
                        if tail.len() < 6
                            || tail.first() != Some(&b'\\')
                            || tail.get(1) != Some(&b'u')
                        {
                            return Err(false);
                        }
                        let lo = hex4(tail.get(2..).unwrap_or_default()).ok_or(false)?;
                        if !(0xdc00..=0xdfff).contains(&lo) {
                            return Err(false);
                        }
                        cp = 0x10000 + (((cp - 0xd800) << 10) | (lo - 0xdc00));
                        i += 6;
                    } else if (0xdc00..=0xdfff).contains(&cp) {
                        return Err(false);
                    }
                    if cp == 0 {
                        return Err(true);
                    }
                    let ch = char::from_u32(cp).ok_or(false)?;
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                _ => return Err(false),
            }
        } else if c < 0x20 {
            return Err(false);
        } else {
            out.push(c);
        }
        i += 1;
    }
    Ok(out)
}

/// `scan_json_number`: the RFC 8259 number grammar. Returns the length and
/// whether it has a fraction or exponent.
fn scan_number(s: &[u8]) -> Option<(usize, bool)> {
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let mut p = 0;
    let mut is_float = false;
    if at(p) == b'-' {
        p += 1;
    }
    if at(p) == b'0' {
        p += 1;
    } else if at(p).is_ascii_digit() {
        while at(p).is_ascii_digit() {
            p += 1;
        }
    } else {
        return None;
    }
    if at(p) == b'.' {
        is_float = true;
        p += 1;
        if !at(p).is_ascii_digit() {
            return None;
        }
        while at(p).is_ascii_digit() {
            p += 1;
        }
    }
    if matches!(at(p), b'e' | b'E') {
        is_float = true;
        p += 1;
        if matches!(at(p), b'+' | b'-') {
            p += 1;
        }
        if !at(p).is_ascii_digit() {
            return None;
        }
        while at(p).is_ascii_digit() {
            p += 1;
        }
    }
    Some((p, is_float))
}
