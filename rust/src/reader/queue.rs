//! Verified events waiting to be handed out by the reader.
//!
//! The validator produces events while the lexer works through a chunk of
//! input; the reader hands them out one at a time afterwards. Their text is
//! copied into one shared arena, so queuing an event costs no allocation of
//! its own, and the arena is reused once the queue has drained.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::ops::Range;

use crate::event::{Event, TokenKind, TypeAnnotation};
use crate::types::{TypeFamily, TypeSpec};
use crate::Error;

/// A queued event; byte ranges point into [`EventQueue::arena`].
#[derive(Debug, Clone)]
pub(crate) enum QueuedEvent {
    StreamStart,
    AssignmentStart(Range<usize>),
    TypeAnnotation {
        text: Range<usize>,
        family: TypeFamily,
        width: Option<u32>,
        base: Option<u32>,
        q: Option<u32>,
        unit: Option<Range<usize>>,
        epoch: Option<&'static str>,
        synthesized: bool,
    },
    Data {
        kind: TokenKind,
        text: Range<usize>,
        frac: Range<usize>,
        value_type: TypeSpec,
        unit: Range<usize>,
        inline_unit: Range<usize>,
    },
    StructStart,
    StructEnd,
    ArrayRowStart,
    ArrayRowEnd,
    ArrayDimStart,
    OctetStreamStart,
    OctetStreamChunk(Range<usize>),
    OctetStreamEnd,
    StreamEnd,
    Recovered(Error),
}

/// Queue of verified events plus the bytes they refer to.
#[derive(Debug, Default)]
pub(crate) struct EventQueue {
    events: VecDeque<QueuedEvent>,
    arena: Vec<u8>,
}

impl EventQueue {
    /// Copies `bytes` into the arena and returns where they landed.
    pub(crate) fn store(&mut self, bytes: &[u8]) -> Range<usize> {
        let start = self.arena.len();
        self.arena.extend_from_slice(bytes);
        start..self.arena.len()
    }

    pub(crate) fn push(&mut self, event: QueuedEvent) {
        self.events.push_back(event);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub(crate) fn pop(&mut self) -> Option<QueuedEvent> {
        self.events.pop_front()
    }

    /// Releases the arena; only valid once every queued event is gone.
    pub(crate) fn reset_arena(&mut self) {
        if self.events.is_empty() {
            self.arena.clear();
        }
    }

    /// Resolves a queued event against the arena.
    pub(crate) fn resolve<'a>(&'a self, event: &QueuedEvent) -> Event<'a> {
        let bytes = |r: &Range<usize>| self.arena.get(r.clone()).unwrap_or_default();
        match event {
            QueuedEvent::StreamStart => Event::StreamStart,
            QueuedEvent::AssignmentStart(key) => Event::AssignmentStart { key: bytes(key) },
            QueuedEvent::TypeAnnotation {
                text,
                family,
                width,
                base,
                q,
                unit,
                epoch,
                synthesized,
            } => Event::TypeAnnotation(TypeAnnotation {
                text: bytes(text),
                family: *family,
                width: *width,
                base: *base,
                q: *q,
                unit: unit.as_ref().map(bytes),
                epoch: *epoch,
                synthesized: *synthesized,
            }),
            QueuedEvent::Data {
                kind,
                text,
                frac,
                value_type,
                unit,
                inline_unit,
            } => Event::Data {
                kind: *kind,
                text: bytes(text),
                frac: bytes(frac),
                value_type: *value_type,
                unit: bytes(unit),
                inline_unit: bytes(inline_unit),
            },
            QueuedEvent::StructStart => Event::StructStart,
            QueuedEvent::StructEnd => Event::StructEnd,
            QueuedEvent::ArrayRowStart => Event::ArrayRowStart,
            QueuedEvent::ArrayRowEnd => Event::ArrayRowEnd,
            QueuedEvent::ArrayDimStart => Event::ArrayDimStart,
            QueuedEvent::OctetStreamStart => Event::OctetStreamStart,
            QueuedEvent::OctetStreamChunk(data) => Event::OctetStreamChunk(bytes(data)),
            QueuedEvent::OctetStreamEnd => Event::OctetStreamEnd,
            QueuedEvent::StreamEnd => Event::StreamEnd,
            QueuedEvent::Recovered(error) => Event::Recovered(*error),
        }
    }
}
