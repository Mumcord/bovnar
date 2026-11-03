#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
bvnr_data.py — a tiny standalone reader for the codegen data documents.

The *.bvnr source-of-truth files use only a fixed, simple subset of the bovnar
grammar: a document is a sequence of `.key = value;` assignments whose values are
strings, numbers, booleans, bare symbols, structs `{ … }`, and arrays `[ … ]`.
Inline `<type>` annotations are skipped (codegen only needs the value).

Parsing them here — instead of through the compiled libbvnr — means the
generators have NO dependency on a built bovnar, so a clean checkout regenerates
the tables with plain Python and there is no bootstrap cycle.

`load(text)` returns the same nested dict/list/scalar structure that
`bovnar.loads` returns for these documents (verified by comparison), so it is a
drop-in replacement for the generators' purposes.
"""
import re

_NUMBER = re.compile(r"[-]?(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?$")
_IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_\-]*")
# token terminators for a bare scalar (number / symbol)
_SCALAR_END = set(" \t\r\n;,]}#")


class _Parser:
    def __init__(self, text):
        self.s = text
        self.i = 0
        self.n = len(text)

    def _err(self, msg):
        line = self.s.count("\n", 0, self.i) + 1
        raise ValueError("bvnr parse error at line %d: %s" % (line, msg))

    def _ws(self):
        """Skip whitespace and `#` comments (this also skips the `#!bovnar …`
        version directive, which begins with `#`)."""
        while self.i < self.n:
            c = self.s[self.i]
            if c in " \t\r\n":
                self.i += 1
            elif c == "#":
                while self.i < self.n and self.s[self.i] != "\n":
                    self.i += 1
            else:
                break

    def _peek(self):
        return self.s[self.i] if self.i < self.n else ""

    def _skip_annotation(self):
        """Skip a leading `<…>` type annotation, if present."""
        self._ws()
        if self._peek() == "<":
            while self.i < self.n and self.s[self.i] != ">":
                self.i += 1
            if self.i < self.n:
                self.i += 1  # consume '>'

    def document(self):
        d = {}
        while True:
            self._ws()
            if self.i >= self.n:
                return d
            if self._peek() != ".":
                self._err("expected `.key` at top level, got %r" % self._peek())
            k, v = self._assignment()
            d[k] = v

    def _assignment(self):
        self.i += 1  # consume '.'
        m = _IDENT.match(self.s, self.i)
        if not m:
            self._err("expected identifier after `.`")
        key = m.group(0)
        self.i = m.end()
        self._ws()
        if self._peek() != "=":
            self._err("expected `=` after .%s" % key)
        self.i += 1  # consume '='
        val = self._value()
        self._ws()
        if self._peek() == ";":
            self.i += 1
        return key, val

    def _value(self):
        self._skip_annotation()
        self._ws()
        c = self._peek()
        if c == '"':
            return self._string()
        if c == "{":
            return self._struct()
        if c == "[":
            return self._array()
        if c == "":
            self._err("unexpected end of input where a value was expected")
        # An empty slot (".x = ;" or a gap between array commas) is the null
        # value, not an empty symbol. _scalar() stops immediately on the
        # terminator and returned "", which is falsy like None but is not None --
        # so `value is None` checks in a generator would have missed it.
        if c in ";,]}":
            return None
        return self._scalar()

    def _string(self):
        self.i += 1  # consume opening '"'
        out = []
        while self.i < self.n:
            c = self.s[self.i]
            self.i += 1
            if c == '"':
                return "".join(out)
            if c == "\\":
                e = self.s[self.i] if self.i < self.n else ""
                self.i += 1
                # spec 1.1 codepoint and byte escapes. Unknown escapes used to
                # fall through to the escaped character itself, so "caf\\u{e9}"
                # silently became "cafu{e9}" -- a unit or currency NAME would
                # have shipped corrupted into the generated tables.
                if e == "u":
                    if self.i < self.n and self.s[self.i] == "{":
                        end = self.s.find("}", self.i)
                        if end < 0:
                            self._err("unterminated \\u{...} escape")
                        cp = self.s[self.i + 1:end]
                        self.i = end + 1
                        try:
                            out.append(chr(int(cp, 16)))
                        except ValueError:
                            self._err("bad codepoint in \\u{%s}" % cp)
                        continue
                    self._err("\\u must be followed by {")
                if e == "x":
                    hx = self.s[self.i:self.i + 2]
                    if len(hx) != 2:
                        self._err("truncated \\xHH escape")
                    self.i += 2
                    try:
                        out.append(chr(int(hx, 16)))
                    except ValueError:
                        self._err("bad hex in \\x%s" % hx)
                    continue
                named = {'"': '"', "\\": "\\", "n": "\n", "t": "\t",
                         "r": "\r", "v": "\v", "f": "\f", "0": "\0"}
                if e not in named:
                    self._err("unknown escape \\%s" % e)
                out.append(named[e])
            else:
                out.append(c)
        self._err("unterminated string")

    def _struct(self):
        self.i += 1  # consume '{'
        d = {}
        while True:
            self._ws()
            c = self._peek()
            if c == "}":
                self.i += 1
                return d
            if c != ".":
                self._err("expected `.key` or `}` in struct, got %r" % c)
            k, v = self._assignment()
            d[k] = v

    def _array(self):
        self.i += 1  # consume '['
        lst = []
        while True:
            self._ws()
            c = self._peek()
            if c == "]":
                self.i += 1
                return lst
            # An UNMATCHED '}' inside an array used to spin here forever. A
            # closing delimiter is one of the characters _value() reports as the
            # empty slot `[1, , 3]` is made of, so it returned None WITHOUT
            # consuming anything, and the loop asked again, and again. A stray
            # brace -- one duplicated line in units.bvnr -- turned every
            # generator into a hang: no message, no exit, and in CI a timeout
            # rather than a diagnosis.
            #
            # An array is closed by ']' and by nothing else, so say so.
            if c in "}" or c == "":
                self._err("unterminated array: expected `]`, got %r"
                          % (c or "end of input"))
            lst.append(self._value())
            self._ws()
            c = self._peek()
            if c == ",":
                self.i += 1
            elif c != "]":
                # Same reasoning one step along: after an element the only legal
                # continuations are ',' and ']'. Anything else would either spin
                # or be silently swallowed by the next _value().
                self._err("expected `,` or `]` after an array element, got %r"
                          % (c or "end of input"))

    def _scalar(self):
        start = self.i
        while self.i < self.n and self.s[self.i] not in _SCALAR_END:
            self.i += 1
        tok = self.s[start:self.i]
        # `on`/`off` are boolean synonyms in the format, and returning them as
        # strings was actively dangerous: the string "off" is TRUTHY, so a field
        # written `.exact = off;` -- perfectly legal bovnar -- would have been
        # read as exact and shipped a wrong unit table. Same for the null and
        # non-finite keywords, which became the strings "null", "nan", "inf".
        if tok in ("true", "on"):
            return True
        if tok in ("false", "off"):
            return False
        if tok == "null":
            return None
        if tok in ("nan", "inf", "ninf"):
            return float({"nan": "nan", "inf": "inf", "ninf": "-inf"}[tok])
        if tok.startswith("+"):
            # The format has no leading '+'; accepting one here would let a
            # literal through that the reference reader rejects.
            self._err("a leading '+' is not a valid bovnar number: %r" % tok)
        if _NUMBER.match(tok):
            if any(ch in tok for ch in ".eE"):
                return float(tok)
            return int(tok)
        return tok  # bare symbol (e.g. `default`, `info`, `kilo`)


def load(text):
    """Parse a bvnr data document (str) into a nested dict/list/scalar value."""
    if isinstance(text, bytes):
        text = text.decode("utf-8")
    return _Parser(text).document()


def write_if_changed(dest, text):
    """Write `text` to `dest` only when the content differs.

    The generators are re-run whenever a build directory lacks the .gen.inc
    fragments -- which a FRESH build directory always does -- and three of their
    outputs are committed headers under include/. Rewriting those unconditionally
    meant configuring any new build tree modified the SOURCE tree: a read-only
    checkout could not be configured at all (PermissionError on
    include/bovnar_units.gen.h), and two build directories from one checkout
    raced on the same files. Content-comparing first makes the normal case a
    no-op, so an up-to-date checkout is never written to.
    """
    try:
        with open(dest, "r", encoding="utf-8") as f:
            if f.read() == text:
                return False
    except (OSError, UnicodeDecodeError):
        pass
    with open(dest, "w", encoding="utf-8") as f:
        f.write(text)
    return True
