#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_doc_python_examples.py — doc/09's `# →` expectations, actually evaluated.

THE GAP THIS CLOSES. doc/09 is the Python reference and it states its results the
way a reader reads them:

    bovnar.unit_to_profile("unece", kg)   # → "KGM"
    f = bovnar.unit_factor("M~Hz")        # → 1_000_000.0

Nothing ran any of it. check_doc_examples.py parses the embedded *bovnar*
documents, check_doc_unit_tables.py checks doc/09's BaseUnit range table, and the
76 KB of Python either side of those was the copy nobody executed. Three claims
in it were wrong at once, all in the section describing `UnitFlags.REDUCE`: that
`unit_reduce` yields "the canonical named SI unit" (it yields `m·g/s²` where the
formatter yields `N`), that a caller should apply `unit_reduce(vu).scale` to a
value it is also going to serialise (that double-applies wherever the formatter
re-attaches a prefix — `k~N` is out by 1000), and that a reduction overflows the
"±9 exponent range" (it is ±100, and has been since the exponent range grew).

WHAT IS CHECKED. Every ```python block is executed in order, in one namespace, so
a block may build on the one before it exactly as a reader would. On each line
carrying a `# →` comment, the expression to its left is evaluated a second time
and its result compared with what the comment claims.

WHAT IS SKIPPED, AND LOUDLY. A block that needs something this gate cannot supply
— a file on disk, a handler defined elsewhere, an optional third-party import —
raises, and the whole block is skipped from that point. The count of skipped
blocks is printed on every run, so "nothing is checked any more" cannot look like
success. A block may also opt out with a `# check-doc-python: skip` line, which
is the honest way to exclude a snippet that is deliberately illustrative.

Comparison is deliberately loose about FORM and strict about VALUE: the comment
is prose written for a reader, so `"KGM"`, `'KGM'` and `KGM` all match the string
KGM, `1_000_000.0` matches the float, and any trailing commentary after two
spaces or an em-dash is ignored. A comment that is not parseable as a value at
all (`# → a Quantity`) is not a claim and is skipped.

Usage:  python3 check_doc_python_examples.py [repo-root]
Exit 0 when every stated result matches, 1 with a list when not.
"""
import ast
import contextlib
import io as _io
import os
import re
import sys

REPO = os.path.dirname(os.path.abspath(__file__))
DOC = "doc/09_bovnar_python_bindings.md"

BLOCK = re.compile(r"```python\n(.*?)```", re.S)
# "<expr>   # → <expected>" — the expression is everything before the comment.
ARROW = re.compile(r"^(?P<code>.*?)\s*#\s*→\s*(?P<want>.+?)\s*$")
SKIP_MARKER = "check-doc-python: skip"
# `[^\S\n]*`, NOT `\s*`: `\s` matches a newline, so a greedy `\s*#` before a
# standalone `# →` line swallows the line break and DELETES the line, which
# shifted every claim after it onto the following statement.
ARROW_STRIP = re.compile(r"[^\S\n]*#\s*→.*$", re.M)


def wanted_value(text):
    """The value a `# →` comment claims, or None when it is prose.

    The comment is written for a reader, so it may carry commentary after the
    value ("1.0  (NOT 0.0254 — the inch has no prefix)"). Everything from a
    double space or an em-dash is dropped before parsing.
    """
    text = re.split(r"\s\s+|\s—\s", text, maxsplit=1)[0].strip()
    # "50 (ErrorCode.UNIT_PROFILE_UNSUPPORTED)" — the value, then a gloss.
    text = re.sub(r"\s+\(.*\)$", "", text).strip()
    if not text:
        return None, False
    try:
        return ast.literal_eval(text), True
    except (ValueError, SyntaxError):
        pass
    # A bare word the document did not quote: "KGM", "True", "None".
    if re.fullmatch(r"[A-Za-z_][\w.\[\]-]*", text):
        return text, True
    return None, False


def matches(got, want):
    """Value equality, tolerant of how the document spells the value."""
    if got == want:
        return True
    # A ValueUnit the document spelled the way the formatter spells it.
    if isinstance(want, str) and type(got).__name__ == "ValueUnit":
        try:
            import bovnar
            return bovnar.unit_to_str(got) == want
        except Exception:
            return False
    # A string result the comment left unquoted, or vice versa.
    if isinstance(want, str) and not isinstance(got, str):
        return repr(got) == want or str(got) == want
    if isinstance(got, str) and not isinstance(want, str):
        return got == str(want)
    # A tuple/list written the other way round.
    if isinstance(got, (tuple, list)) and isinstance(want, (tuple, list)):
        return list(got) == list(want)
    return False


def expression_of(code):
    """The trailing expression of a line, or None.

    `f = bovnar.unit_factor("M~Hz")` claims something about the right-hand side;
    a bare call claims something about itself. A statement that is neither (an
    import, a `with`) makes no claim.
    """
    code = code.strip()
    if not code or code.startswith(("#", "import ", "from ", "with ", "def ",
                                    "class ", "for ", "if ", "return", "@")):
        return None
    m = re.match(r"^[A-Za-z_][\w, ]*=\s*(?!=)(.+)$", code, re.S)
    if m:
        return m.group(1).strip()
    return code


def statements(block):
    """Top-level statements of a block, as (source, first line, last line).

    Parsed with `ast` rather than split on newlines. Splitting on newlines is
    what kept this gate's coverage at a third: a multi-line call, a `with`, a
    `def` — every one of them raised SyntaxError or IndentationError on its
    first line and abandoned the whole block, which then looked like "needs
    something this gate cannot supply" when it needed nothing of the sort.
    """
    try:
        tree = ast.parse(block)
    except SyntaxError:
        return None
    lines = block.split("\n")
    out = []
    for node in tree.body:
        first = node.lineno
        last = getattr(node, "end_lineno", node.lineno)
        # Decorators sit above the node's own lineno.
        for dec in getattr(node, "decorator_list", []):
            first = min(first, dec.lineno)
        src = "\n".join(lines[first - 1:last])
        out.append((src, first, last))
    return out


def check(repo):
    path = os.path.join(repo, DOC)
    if not os.path.exists(path):
        print("check_doc_python_examples: %s is absent" % DOC)
        return [], 0, 0
    sys.path.insert(0, os.path.join(repo, "python"))
    try:
        import bovnar
        bovnar._ffi.load_library()
    except Exception:
        print("check_doc_python_examples: no built library; skipped")
        return [], 0, 0

    with open(path, encoding="utf-8") as f:
        text = f.read()
    ns = {"bovnar": bovnar}
    problems, checked, skipped = [], 0, 0

    for block in BLOCK.findall(text):
        raw_block = block
        # A fenced block nested inside a blockquote carries the "> " markers.
        if all(ln.startswith(">") or not ln.strip()
               for ln in block.split("\n") if ln):
            block = re.sub(r"^> ?", "", block, flags=re.M)
        if SKIP_MARKER in block:
            skipped += 1
            continue
        block_line = text[:text.index(raw_block)].count("\n") + 1
        # A `# →` comment belongs to the statement that ENDS on its line.
        claims, last_code = {}, 0
        for offset, raw in enumerate(block.split("\n"), start=1):
            m = ARROW.match(raw)
            if m:
                # A continuation line — `# → …` with no code of its own —
                # claims something about the statement it sits under.
                claims[offset if m.group("code").strip() else last_code] = \
                    m.group("want")
            if (m.group("code") if m else raw).strip():
                last_code = offset
        stmts = statements(ARROW_STRIP.sub("", block))
        if stmts is None:
            skipped += 1
            continue
        for src, first, last in stmts:
            out = _io.StringIO()
            try:
                with contextlib.redirect_stdout(out):
                    exec(compile(src, DOC, "exec"), ns)      # noqa: S102
            except Exception:
                # Genuinely needs something this gate cannot supply: a file, a
                # handler defined elsewhere, an optional third-party import.
                skipped += 1
                break
            want_text = claims.get(last)
            if want_text is None:
                continue
            want, is_claim = wanted_value(want_text)
            if not is_claim:
                continue
            expr = expression_of(src)
            if expr is None:
                continue
            if expr.startswith("print("):
                # The claim is about what reached stdout, not about None.
                checked += 1
                shown = out.getvalue().strip()
                if not matches(shown, want) and shown != str(want):
                    problems.append("%s:%d: `%s` is documented as printing %r; "
                                    "it prints %r"
                                    % (DOC, block_line + last - 1,
                                       expr.replace("\n", " ")[:70], want,
                                       shown))
                continue
            try:
                with contextlib.redirect_stdout(_io.StringIO()):
                    got = eval(compile(expr, DOC, "eval"), ns)   # noqa: S307
            except Exception:
                continue
            checked += 1
            if not matches(got, want):
                problems.append("%s:%d: `%s` is documented as %r; it is %r"
                                % (DOC, block_line + last - 1,
                                   expr.replace("\n", " ")[:70], want, got))
    return problems, checked, skipped


def property_calls(repo):
    """Documented properties written as method calls, anywhere under doc/.

    `Quantity.decimal`, `.fraction`, `.stored_value`, `.unit_str` and their
    neighbours were unified into properties, and doc/09 went on documenting all
    six as `q.decimal()` — eleven sites, including the member table that is the
    reference for them. The Python tests assert that calling one raises
    TypeError, so the library was right, the reference was wrong, and nothing
    compared the two.

    A property is never callable, so `.name()` is unambiguous. The reverse — a
    method named bare — is ordinary prose ("`unit_to_str` returns …") and is
    not flagged. A name that is a property on one class and a method on another
    is ambiguous and is left alone.
    """
    import bovnar
    props, methods = set(), set()
    for cls in vars(bovnar).values():
        if not isinstance(cls, type):
            continue
        for name, member in vars(cls).items():
            if name.startswith("_"):
                continue
            (props if isinstance(member, property) else methods).add(name)
    names = props - methods
    if not names:
        return []
    pattern = re.compile(r"\.(%s)\s*\(" % "|".join(sorted(names)))
    problems = []
    for root, dirs, files in os.walk(repo):
        dirs[:] = [d for d in dirs if d not in
                   (".git", "build", "web", "node_modules", "__pycache__")]
        for name in files:
            if not name.endswith((".md", ".txt")):
                continue
            # Dated records describe the API as it was on the day they were
            # written. CHANGELOG.md's own migration note — "drop the
            # parentheses, `q.decimal()` becomes `q.decimal`" — has to be
            # allowed to quote the spelling it is retiring.
            if name == "CHANGELOG.md" or name.startswith("RELEASE_NOTES"):
                continue
            path = os.path.join(root, name)
            with open(path, encoding="utf-8", errors="replace") as f:
                # Rust bindings call these as methods (`q.decimal()`), so a
                # fenced block tagged ```rust (also ```rust,ignore and the
                # like) is not Python and is not checked. Every other line,
                # Python and untagged blocks included, is checked as before.
                in_rust = False
                for lineno, line in enumerate(f, start=1):
                    fence = line.strip()
                    if fence.startswith("```"):
                        tag = fence[3:].split(",")[0].strip()
                        in_rust = not in_rust and tag == "rust"
                        continue
                    if in_rust:
                        continue
                    m = pattern.search(line)
                    if m:
                        problems.append(
                            "%s:%d: `.%s()` — %s is a property, not a method; "
                            "calling it raises TypeError"
                            % (os.path.relpath(path, repo), lineno,
                               m.group(1), m.group(1)))
    return problems


def main(argv):
    repo = os.path.abspath(argv[1]) if len(argv) > 1 else REPO
    sys.path.insert(0, REPO)
    import bvnr_build_env
    bvnr_build_env.require_build(repo, "check_doc_python_examples", ("library",))
    problems, checked, skipped = check(repo)
    try:
        problems = problems + property_calls(repo)
    except Exception:
        pass
    if problems:
        sys.stderr.write("check_doc_python_examples: %s states results the "
                         "library does not produce:\n" % DOC)
        for p in problems:
            sys.stderr.write("    %s\n" % p)
        return 1
    print("check_doc_python_examples: %s — %d `# →` expectation(s) evaluated, "
          "%d block(s) skipped as needing more than this gate supplies"
          % (DOC, checked, skipped))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
