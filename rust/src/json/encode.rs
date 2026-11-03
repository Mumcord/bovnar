//! JSON tree to Bovnar events, as `cmd_convert_json_to_bvnr` writes them.
//!
//! The mapping is structural: an object is a struct, an array a bracket
//! array whose elements are written in place (nested arrays as nested rows,
//! objects as structs). Scalars keep their JSON type: non-negative integers
//! become `<uint>`, negative ones `<sint>`, numbers with a fraction or
//! exponent `<float>`.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use super::parse::{Node, Tree};
use super::JsonError;
use crate::event::{Event, TokenKind, TypeAnnotation};
use crate::reader::validate_utf8;
use crate::types::{TypeFamily, TypeSpec};
use crate::writer::{Sink, Writer};
use crate::{num, ErrorKind, Limits};

/// A memory sink with the reference's fixed capacity.
struct Capped {
    buf: Vec<u8>,
    cap: u64,
}

impl Sink for Capped {
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), ErrorKind> {
        let len = (self.buf.len() as u64).saturating_add(bytes.len() as u64);
        if len > self.cap {
            return Err(ErrorKind::SinkBufferExhausted);
        }
        self.buf.extend_from_slice(bytes);
        Ok(())
    }
}

enum Frame<'t> {
    /// Members of an object; `close` writes a struct end when done (every
    /// object but the root). `key` is the member being written.
    Members {
        members: &'t [(Vec<u8>, usize)],
        next: usize,
        close: bool,
        key: Option<&'t [u8]>,
    },
    /// Elements of an array.
    Elements { items: &'t [usize], next: usize },
}

/// `json_key_is_valid_bvnr_id`: a letter, `_` or a UTF-8 leader from 0xC3
/// first, then letters, digits, `_`, `+`, `-` or bytes 0x80 and up (except
/// 0xC2).
fn key_is_identifier(key: &[u8]) -> bool {
    let Some((&first, rest)) = key.split_first() else {
        return false;
    };
    let start = first.is_ascii_alphabetic() || first == b'_' || (0xc3..=0xf4).contains(&first);
    start
        && rest.iter().all(|&c| {
            c.is_ascii_alphanumeric() || matches!(c, b'_' | b'+' | b'-') || (c >= 0x80 && c != 0xc2)
        })
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// `json_string_fits_bvnr`: no control byte a Bovnar string cannot carry,
/// and well-formed UTF-8.
fn check_string(s: &[u8], key: Option<&[u8]>) -> Result<(), JsonError> {
    if let Some(&byte) = s
        .iter()
        .find(|&&c| c <= 0x08 || (0x0e..=0x1f).contains(&c) || c == 0x7f)
    {
        return Err(JsonError::ControlCharacter {
            key: key.map(lossy),
            byte,
        });
    }
    if !validate_utf8(s) {
        return Err(JsonError::InvalidUtf8 {
            key: key.map(lossy),
        });
    }
    Ok(())
}

const fn annotation(family: TypeFamily) -> TypeAnnotation<'static> {
    TypeAnnotation {
        text: b"",
        family,
        width: None,
        base: None,
        q: None,
        unit: None,
        epoch: None,
        synthesized: false,
    }
}

const fn data(kind: TokenKind, text: &[u8]) -> Event<'_> {
    Event::Data {
        kind,
        text,
        frac: b"",
        value_type: TypeSpec::PLAIN,
        unit: b"",
        inline_unit: b"",
    }
}

struct Encoder<'t> {
    tree: &'t Tree,
    writer: Writer<Capped>,
}

impl<'t> Encoder<'t> {
    fn emit(&mut self, event: &Event<'_>) -> Result<(), JsonError> {
        self.writer.write_event(event).map_err(JsonError::Writer)
    }

    /// One object member (`write_bvn_value`); containers push a frame.
    fn member(
        &mut self,
        key: &'t [u8],
        node: usize,
        stack: &mut Vec<Frame<'t>>,
    ) -> Result<(), JsonError> {
        if !key_is_identifier(key) {
            return Err(JsonError::InvalidKey { key: lossy(key) });
        }
        let tree = self.tree;
        let node = tree.node(node);
        if let Node::Str(s) = node {
            check_string(s, Some(key))?;
        }
        self.key(key)?;
        match node {
            Node::Null => self.emit(&data(TokenKind::Null, b"")),
            Node::Bool(b) => self.emit(&data(TokenKind::Bool, bool_text(*b))),
            Node::Uint(v) => self.number(TypeFamily::Uint, &v.to_string()),
            Node::Int(v) => self.number(TypeFamily::Sint, &v.to_string()),
            Node::Float(v) => self.number(TypeFamily::Float, &num::shortest_bin53(*v)),
            Node::Str(s) => self.emit(&data(TokenKind::String, s)),
            Node::Array(items) => {
                self.emit(&Event::ArrayRowStart)?;
                stack.push(Frame::Elements { items, next: 0 });
                Ok(())
            }
            Node::Object(members) => {
                self.emit(&Event::StructStart)?;
                stack.push(Frame::Members {
                    members,
                    next: 0,
                    close: true,
                    key: None,
                });
                Ok(())
            }
        }
    }

    /// The key of an assignment. The checks the C writer makes on a key
    /// (`bvn_validate_id_for_writer`) come first, with its error codes:
    /// past `json_key_is_valid_bvnr_id` only length and UTF-8 remain.
    fn key(&mut self, key: &[u8]) -> Result<(), JsonError> {
        let kind = if key.len() > 255 {
            Some(ErrorKind::IdentifierTooLong)
        } else if !validate_utf8(key) {
            Some(ErrorKind::InvalidArgument)
        } else {
            None
        };
        if let Some(kind) = kind {
            return Err(JsonError::Writer(crate::Error::without_position(kind)));
        }
        self.emit(&Event::AssignmentStart { key })
    }

    fn number(&mut self, family: TypeFamily, text: &str) -> Result<(), JsonError> {
        self.emit(&Event::TypeAnnotation(annotation(family)))?;
        self.emit(&data(TokenKind::Number, text.as_bytes()))
    }

    /// One array element (`write_bvn_element`); containers push a frame.
    fn element(
        &mut self,
        node: usize,
        key: Option<&[u8]>,
        stack: &mut Vec<Frame<'t>>,
    ) -> Result<(), JsonError> {
        let tree = self.tree;
        match tree.node(node) {
            Node::Array(items) => {
                self.emit(&Event::ArrayRowStart)?;
                stack.push(Frame::Elements { items, next: 0 });
                Ok(())
            }
            Node::Object(members) => {
                self.emit(&Event::StructStart)?;
                stack.push(Frame::Members {
                    members,
                    next: 0,
                    close: true,
                    key: None,
                });
                Ok(())
            }
            Node::Null => self.emit(&data(TokenKind::Null, b"")),
            Node::Bool(b) => self.emit(&data(TokenKind::Bool, bool_text(*b))),
            Node::Uint(v) => self.emit(&data(TokenKind::ArrayNumber, v.to_string().as_bytes())),
            Node::Int(v) => self.emit(&data(TokenKind::ArrayNumber, v.to_string().as_bytes())),
            Node::Float(v) => {
                let text = num::fmt_g(*v, 17);
                // "%.17g" renders 2.0 as "2", which would read back as a
                // uint: the reference annotates such an element as a float.
                if !text.contains(['.', 'e', 'E']) {
                    self.emit(&Event::TypeAnnotation(annotation(TypeFamily::Float)))?;
                }
                self.emit(&data(TokenKind::ArrayNumber, text.as_bytes()))
            }
            Node::Str(s) => {
                check_string(s, key)?;
                self.emit(&data(TokenKind::ArrayString, s))
            }
        }
    }
}

fn bool_text(b: bool) -> &'static [u8] {
    if b {
        b"true"
    } else {
        b"false"
    }
}

/// The key diagnostics name: the member being written in the innermost
/// object (`json_cur_key`).
fn current_key<'t>(stack: &[Frame<'t>]) -> Option<&'t [u8]> {
    stack.iter().rev().find_map(|f| match f {
        Frame::Members { key, .. } => *key,
        Frame::Elements { .. } => None,
    })
}

/// Writes the tree as a pretty-printed Bovnar document. The output may hold
/// at most `8 * input_len + 65536` bytes, the reference's buffer.
pub(super) fn encode(tree: &Tree, input_len: usize, limits: &Limits) -> Result<Vec<u8>, JsonError> {
    let Node::Object(root) = tree.node(tree.root) else {
        return Err(JsonError::RootNotObject);
    };
    let cap = (input_len as u64).saturating_mul(8).saturating_add(65_536);
    let sink = Capped {
        buf: Vec::new(),
        cap,
    };
    let mut enc = Encoder {
        tree,
        writer: Writer::with_limits(sink, true, limits),
    };
    let mut stack = alloc::vec![Frame::Members {
        members: root,
        next: 0,
        close: false,
        key: None,
    }];
    while let Some(top) = stack.last_mut() {
        match top {
            Frame::Members {
                members,
                next,
                close,
                key,
            } => {
                let Some((k, child)) = members.get(*next) else {
                    let close = *close;
                    stack.pop();
                    if close {
                        enc.emit(&Event::StructEnd)?;
                    }
                    continue;
                };
                *next += 1;
                *key = Some(k.as_slice());
                let k = k.as_slice();
                let child = *child;
                enc.member(k, child, &mut stack)?;
            }
            Frame::Elements { items, next } => {
                let Some(&child) = items.get(*next) else {
                    stack.pop();
                    enc.emit(&Event::ArrayRowEnd)?;
                    continue;
                };
                *next += 1;
                let key = current_key(&stack);
                enc.element(child, key, &mut stack)?;
            }
        }
    }
    enc.writer.finish().map_err(JsonError::Writer)?;
    Ok(enc.writer.into_sink().buf)
}
