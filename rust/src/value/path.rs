//! Lookup paths: `.key` and `[index]` steps (`.servers[0].port`).

/// One step of a path.
pub(super) enum Step<'a> {
    Key(&'a str),
    Index(usize),
}

/// The steps of a path, parsed lazily.
pub(super) struct Steps<'a> {
    rest: &'a str,
    first: bool,
}

impl<'a> Steps<'a> {
    pub(super) fn new(path: &'a str) -> Self {
        Self {
            rest: path,
            first: true,
        }
    }

    /// What is left of the path.
    pub(super) fn rest(&self) -> &'a str {
        self.rest
    }

    /// The next step: `Some(None)` at the end, `None` for a malformed path.
    pub(super) fn next(&mut self) -> Option<Option<Step<'a>>> {
        let first = core::mem::replace(&mut self.first, false);
        if self.rest.is_empty() {
            return Some(None);
        }
        if let Some(after) = self.rest.strip_prefix('[') {
            let close = after.find(']')?;
            let index = after.get(..close)?.parse().ok()?;
            self.rest = after.get(close + 1..)?;
            return Some(Some(Step::Index(index)));
        }
        // A leading dot is optional on the first key only.
        let key_start = match self.rest.strip_prefix('.') {
            Some(after) => after,
            None if first => self.rest,
            None => return None,
        };
        let end = key_start.find(['.', '[']).unwrap_or(key_start.len());
        let key = key_start.get(..end)?;
        if key.is_empty() {
            return None;
        }
        self.rest = key_start.get(end..)?;
        Some(Some(Step::Key(key)))
    }
}
