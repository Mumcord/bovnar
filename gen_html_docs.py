#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""Render the Bovnar documentation as standalone, SEO-optimized HTML pages.

The canonical docs are Markdown (doc/*.md). They are already published as raw
Markdown (/doc/*.md, great for LLMs) and as PDFs (/doc/pdf/*.pdf), but neither
gives a search engine an indexable HTML page with its own <title>, meta
description, canonical URL, and heading structure. This generator produces one
crawlable page per doc under web/docs/<slug>/index.html, plus a /docs/ index.

Design notes:
  * Inter-doc links (e.g. "02_bovnar_faq.md#foo") are rewritten to the HTML URL
    "/docs/faq/#foo".
  * Heading ids use a GitHub-style slugify so the docs' existing intra-page
    "#anchor" links resolve (the same slugs the raw Markdown assumes).
  * The FAQ page carries FAQPage structured data built from its Q&A headings.
  * Pages are self-contained (inline CSS, no executable inline scripts) with a
    locked-down CSP, so they need no gen_csp restamping.

publish_web.sh runs this before staging. Run by hand: python3 gen_html_docs.py
"""
import base64
import datetime
import hashlib
import html
import os
import re
import subprocess
import sys

import markdown

# The analytics tag and the CSP hosts that admit it live in gen_csp.py, next to
# the policy machinery for the hand-written pages, so the site has one snippet
# and one measurement ID rather than a copy per generator.
import gen_csp

ROOT = os.path.dirname(os.path.abspath(__file__))


def shallow_clone():
    """True when ROOT is a git repo that was cloned without its history.

    git_date() cannot be trusted there: a shallow clone grafts the boundary
    commit into a parentless root, so `git log -- doc/anything.md` answers with
    that one commit for *every* path and both dates below silently collapse to
    the day of the clone. That is not a fallback, it is wrong data -- it would
    date all ten pages "written and last touched today" in their meta tags and
    schema.org graph, which --check then reports as ten stale pages and a plain
    run would write out for real. Callers refuse instead of guessing.

    Not a git repo at all is a different case and stays supported: the dates
    already in the generated pages are kept (page_date()), and only a page that
    does not exist yet falls back to the file mtime."""
    try:
        out = subprocess.run(
            ["git", "-C", ROOT, "rev-parse", "--is-shallow-repository"],
            capture_output=True, text=True, check=True)
        return out.stdout.strip() == "true"
    except Exception:
        return False


def has_history():
    """True when ROOT is the top of a git checkout, so `git log` can date a file.

    Asked of the toplevel rather than "am I inside a work tree": a source
    tarball unpacked somewhere under an unrelated repository is inside THAT
    repository's work tree, and `git log -- doc/x.md` there answers with
    nothing, or with somebody else's dates."""
    try:
        out = subprocess.run(
            ["git", "-C", ROOT, "rev-parse", "--show-toplevel"],
            capture_output=True, text=True, check=True)
        return os.path.realpath(out.stdout.strip()) == os.path.realpath(ROOT)
    except Exception:
        return False


_PAGE_DATE_RE = {
    True:  re.compile(r'article:published_time" content="(\d{4}-\d\d-\d\d)"'),
    False: re.compile(r'article:modified_time" content="(\d{4}-\d\d-\d\d)"'),
}


def page_date(slug, first):
    """The date the page already generated for `slug` carries, or None.

    The source tarball and the sdist are `git archive` output: every file has
    the commit's mtime and there is no log to ask. Falling back to that mtime
    stamped all the pages with one date, so `--check` -- which CTest runs --
    reported thirteen of fourteen committed pages stale in the release's own
    tarball. Without history the date a page was generated with is the only
    record there is of when its source was written, so it is kept."""
    try:
        with open(os.path.join(OUT_DIR, slug, "index.html"),
                  encoding="utf-8") as fh:
            m = _PAGE_DATE_RE[first].search(fh.read())
        return m.group(1) if m else None
    except OSError:
        return None


def dirty(relpath):
    """True when `relpath` has changes git has not recorded yet."""
    try:
        out = subprocess.run(
            ["git", "-C", ROOT, "status", "--porcelain", "--", relpath],
            capture_output=True, text=True, check=True)
        return bool(out.stdout.strip())
    except Exception:
        return False


def git_date(relpath, first=False):
    """Last- (or, first=True, first-) commit date YYYY-MM-DD of a repo file;
    falls back to the file mtime, then today. Meaningless in a shallow clone --
    see shallow_clone(), which main() checks before any of this runs.

    AN EDITED DOC IS DATED TODAY, and that is what makes generate-then-commit
    self-consistent. The last-commit date of a doc you are in the middle of
    editing is the date of the version you just replaced, so a page generated
    from it carries a modified_time that goes stale the moment you commit --
    and --check then fails on CI, where the tree is clean and the doc's last
    commit is the one you just made. It went unnoticed for as long as it did
    only because every doc edit here had landed on the same calendar day as the
    previous one, which makes the wrong date and the right one the same string.
    Answering "today" for a dirty file makes the page agree with the commit that
    is about to contain it."""
    if not first and dirty(relpath):
        return datetime.date.today().isoformat()
    args = ["git", "-C", ROOT, "log", "--format=%cs", "--", relpath]
    if first:
        args = ["git", "-C", ROOT, "log", "--diff-filter=A", "--follow",
                "--format=%cs", "--", relpath]
    try:
        out = subprocess.run(args, capture_output=True, text=True,
                             check=True).stdout.strip().splitlines()
        if out:
            return out[-1] if first else out[0]
    except Exception:
        pass
    try:
        return datetime.date.fromtimestamp(
            os.path.getmtime(os.path.join(ROOT, relpath))).isoformat()
    except OSError:
        return datetime.date.today().isoformat()
DOC_DIR = os.path.join(ROOT, "doc")
OUT_DIR = os.path.join(ROOT, "web", "docs")
SITE = "https://www.bovnar.io"


def css_stamp(relpath):
    """?v=<first 12 of sha256> for a stylesheet these pages link.

    The landing page content-stamps every versioned asset (wasm/build_wasm.sh)
    and the server now serves a 12-hex ?v= as immutable for a year. The doc
    pages linked both stylesheets bare, so an edit to either reached a returning
    visitor only after the short fallback TTL expired. Stamping here costs
    nothing extra to police: --check regenerates these pages, so a changed
    stylesheet that was not re-generated fails the suite like any other drift.
    """
    with open(os.path.join(ROOT, "web", relpath), "rb") as fh:
        return "?v=" + hashlib.sha256(fh.read()).hexdigest()[:12]
VERSION = "1.2"   # the SPEC version: the docs describe the format

# source, clean URL slug, PDF slug, title, meta description.
DOCS = [
    ("01_bovnar_tutorial.md", "tutorial", "bovnar-tutorial",
     "Bovnar Tutorial",
     "A guided, example-driven introduction to reading and writing Bovnar (BVNR) "
     "documents — types, units, arrays, references, and the streaming API."),
    ("02_bovnar_faq.md", "faq", "bovnar-faq",
     "Bovnar FAQ",
     "Frequently asked questions about Bovnar types, units, limits, encoding, and "
     "the C and Python APIs."),
    ("03_bovnar_spec.md", "spec", "bovnar-specification",
     "Bovnar Specification (v1.2)",
     "The complete Bovnar (BVNR) format specification, version 1.2: grammar, type "
     "system, physical units, numeric bases, limits, and error semantics."),
    ("04_bovnar_unit_cheatsheet.md", "cheatsheet", "bovnar-cheatsheet",
     "Bovnar Unit & Currency Cheat Sheet",
     "A one-page reference of every Bovnar unit symbol, SI/IEC prefix, and ISO 4217 "
     "and cryptocurrency code."),
    ("05_bovnar_unit_system.md", "units", "bovnar-unit-system",
     "Bovnar Unit & Currency System",
     "Every Bovnar physical unit, SI and IEC prefix, and currency — 264 units and "
     "216 currencies with dimensions and exact conversion factors."),
    ("06_bovnar_unit_policy.md", "policy", "bovnar-unit-policy",
     "Bovnar Unit Policy Reference",
     "The Bovnar unit policy: what it validates, what it converts, the order it "
     "resolves in, and the errors it raises, on both the reader and the writer."),
    ("07_bovnar_unit_ambiguities.md", "ambiguities", "bovnar-unit-ambiguities",
     "Bovnar Unit Ambiguities",
     "Every Bovnar token that could plausibly mean two things — what the parser "
     "reads it as, and how to write the other meaning."),
    ("08_bovnar_readwrite_api.md", "api", "bovnar-readwrite-api",
     "Bovnar C Read & Write API",
     "The Bovnar C reader, writer, and DOM API reference (bovnar.h, bovnar_dom.h), "
     "including read-time lossless unit and base conversion."),
    ("09_bovnar_python_bindings.md", "python", "bovnar-python-bindings",
     "Bovnar Python Bindings",
     "The pure-ctypes Bovnar Python package: loads/dumps, the streaming reader, the "
     "DOM, Quantity, and the NumPy and Pint bridges."),
    ("10_bovnar_streaming.md", "streaming", "bovnar-streaming",
     "Bovnar Streaming, Framing & Multiplexing",
     "Bovnar octet streams, frames, multiplexing/demultiplexing, and embedded "
     "documents for transport over sockets and files."),
    ("11_bovnar_unit_profiles.md", "profiles", "bovnar-unit-profiles",
     "Bovnar Unit Profiles",
     "Writing UCUM, UN/ECE, QUDT, UDUNITS, OM 2 and CF codes in a Bovnar unit slot: the "
     "transliteration tables, the collisions between the namespaces, what has "
     "no representation, and the cross-vocabulary conformance suite."),
    ("12_bovnar.ebnf", "grammar", "bovnar-grammar",
     "Bovnar EBNF Grammar",
     "The formal EBNF grammar of the Bovnar format, annotated against the reference "
     "lexer and validator."),
    ("13_bovnar_conformance.md", "conformance", "bovnar-conformance",
     "Bovnar Conformance Testing",
     "The Bovnar 470-case conformance test suite and the IUT protocol for validating "
     "third-party implementations."),
    ("14_bovnar_rust_bindings.md", "rust", "bovnar-rust-bindings",
     "Bovnar Rust Bindings",
     "The Bovnar Rust crate: parse/to_string, the streaming reader and writer, the DOM, "
     "Quantity and the exact numbers, and JSON conversion."),
]

# filename (and .ebnf) -> clean slug, for inter-doc link rewriting.
SRC_TO_SLUG = {src: slug for src, slug, *_ in DOCS}

# Retired document paths, kept in doc/ as pointers to the current file.
#
# The IANA registration for text/vnd.bovnar cites five documents by their
# pre-renumbering path, and a published registration cannot be edited in place.
# The web server 301s those paths, but the registration links into the GitHub
# tree, which no configuration of ours can redirect -- so the files stay, saying
# where the document went, until the corrected registration is accepted.
#
# A pointer declares itself on line 1 rather than being listed here, because a
# list of them is one more hand-maintained thing to forget: every tool that
# walks doc/ tests the line instead. The marker is inside a comment in both
# formats -- an HTML comment in Markdown, an ISO 14977 comment in EBNF -- so it
# is invisible when rendered and legal where it sits.
RETIRED_MARKER = "bovnar:retired-path"


def is_retired_pointer(path):
    """True if doc/<path> is a pointer to a renamed document, not a document.

    Cheap enough to call while walking a directory: reads one line. The two
    stdlib-only checkers (check_doc_layout.py, check_doc_refs.py) repeat this
    test rather than importing it, for the same reason check_doc_layout repeats
    gh_slug -- importing this module would drag markdown and gen_csp into a
    checker that deliberately needs neither.
    """
    try:
        with open(path, encoding="utf-8") as fh:
            return RETIRED_MARKER in fh.readline()
    except OSError:
        return False


def gh_slug(text):
    """GitHub-style heading slug: lowercase, drop punctuation (keep word chars,
    spaces, hyphens), spaces -> hyphens. Matches the anchors the raw Markdown
    already links to."""
    s = text.strip().lower()
    s = s.replace("`", "").replace("*", "")
    s = re.sub(r"<[^>]+>", "", s)
    s = re.sub(r"[^\w\s-]", "", s)
    return s.strip().replace(" ", "-")


class GHSlugSeen(dict):
    """python-markdown toc slugify gets (value, sep); GitHub disambiguates
    repeats with -1, -2, … We track counts across the whole document."""
    def __call__(self, value, sep):
        base = gh_slug(value)
        n = self.get(base, 0)
        self[base] = n + 1
        return base if n == 0 else f"{base}-{n}"


# ```bovnar fences -- 188 of them across the doc set, the single most common
# tagged language and the one this site is about. Pygments has no Bovnar lexer,
# so codehilite fell back to plain text and every .bvnr sample on /docs/ rendered
# flat while the SAME block, read through the drawer on the landing page, came
# out fully coloured. Rather than port the highlighter to Python (a second
# implementation to keep in lock-step with the vim syntax file), these blocks are
# lifted out before conversion, re-inserted tagged, and coloured in the browser
# by the very same web/bovnar_highlight.js the drawer and the playground use.
# Only column-0 fences are taken: an indented fence belongs to a list item, and
# hoisting it out of that context would change the document structure. Every
# indented fence in doc/ is ```bash, so nothing is missed.
BVNR_FENCE = re.compile(r"^```(?:bovnar|bvnr)[ \t]*\n(.*?)^```[ \t]*$", re.S | re.M)
# Alphanumeric so no Markdown construct can touch it, and unique enough that a
# document could never contain it by accident.
BVNR_MARK = "BVNRHLBLOCK{}ENDBVNRHLBLOCK"


def render_markdown(text):
    blocks = []

    def stash(m):
        blocks.append(m.group(1))
        return "\n\n" + BVNR_MARK.format(len(blocks) - 1) + "\n\n"

    text = BVNR_FENCE.sub(stash, text)
    md = markdown.Markdown(
        extensions=["extra", "sane_lists", "tables", "fenced_code",
                    "codehilite", "toc"],
        extension_configs={
            "codehilite": {"guess_lang": False, "noclasses": False},
            "toc": {"slugify": GHSlugSeen()},
        },
    )
    out = md.convert(text)
    for i, code in enumerate(blocks):
        # The placeholder stood alone in its own block, so Markdown wrapped it in
        # a paragraph. Swap the paragraph, not just the text, or the code block
        # would end up nested inside a <p>.
        marker = f"<p>{BVNR_MARK.format(i)}</p>"
        if marker not in out:                       # never silently drop a block
            raise SystemExit(f"gen_html_docs.py: lost .bvnr code block {i}")
        out = out.replace(
            marker,
            '<div class="codehilite"><pre><code class="language-bovnar">'
            + html.escape(code.rstrip("\n")) + "</code></pre></div>")
    return out


def rewrite_links(body):
    """Point inter-doc Markdown links at their HTML pages.

    A document that has no HTML page of its own is still SERVED, as raw Markdown
    under /doc/ (web/doc is a symlink to doc/, and publish_web.sh excludes only
    the working files). Such a link has to become absolute: left as the relative
    "07_bovnar_unit_ambiguities.md" the Markdown source writes, it resolves against the
    page's own directory — /docs/units/07_bovnar_unit_ambiguities.md — and 404s. Three
    published pages (units, cheatsheet, faq) linked to the ambiguity reference
    that way, so every "see 07_bovnar_unit_ambiguities.md" on the live site was dead while
    /doc/07_bovnar_unit_ambiguities.md served fine.
    """
    def repl(m):
        href = m.group(1)
        frag = ""
        path = href
        if "#" in href:
            path, frag = href.split("#", 1)
            frag = "#" + frag
        # strip a leading ./ or /doc/ so both bare and absolute forms match
        key = path.rsplit("/", 1)[-1]
        slug = SRC_TO_SLUG.get(key)
        if slug:
            return f'href="{SITE}/docs/{slug}/{frag}"'
        # No HTML page — but if the target really is a document we ship, point at
        # the raw copy. Requiring it to exist keeps a genuine typo visible as a
        # broken relative link rather than dressing it up as a plausible URL.
        if (path and not path.startswith(("http:", "https:", "/", "#", "mailto:"))
                and key.endswith((".md", ".ebnf", ".svg"))
                and os.path.exists(os.path.join(DOC_DIR, key))):
            return f'href="/doc/{key}{frag}"'
        return m.group(0)
    return re.sub(r'href="([^"]+)"', repl, body)


# Styling lives in the external /docs/docs.css (shares the landing's design
# tokens and /fonts/fonts.css). The two inline theme scripts below are
# byte-identical on every page, so their CSP hashes are computed once.
THEME_INIT_JS = (
    "(function(){try{var s=localStorage.getItem('bovnar-theme');"
    "var d=s==='dark'||(s!=='light'&&window.matchMedia&&"
    "window.matchMedia('(prefers-color-scheme: dark)').matches);"
    "if(!d)document.documentElement.setAttribute('data-theme','light');}"
    "catch(e){document.documentElement.setAttribute('data-theme','light');}})();")

THEME_TOGGLE_JS = (
    "(function(){var root=document.documentElement,"
    "btn=document.getElementById('theme-toggle'),"
    "meta=document.querySelector('meta[name=\"theme-color\"]');"
    "function light(){return root.getAttribute('data-theme')==='light';}"
    "function sync(){if(btn)btn.setAttribute('aria-label',"
    "light()?'Switch to dark theme':'Switch to light theme');"
    "if(meta)meta.setAttribute('content',light()?'#e8e8e8':'#1e1e1e');}"
    "sync();if(btn)btn.addEventListener('click',function(){"
    "var n=light()?'dark':'light';"
    "if(n==='light')root.setAttribute('data-theme','light');"
    "else root.removeAttribute('data-theme');sync();"
    "try{localStorage.setItem('bovnar-theme',n);}catch(e){}});"
    "try{var mq=window.matchMedia('(prefers-color-scheme: dark)');"
    "mq.addEventListener('change',function(e){"
    "var s=localStorage.getItem('bovnar-theme');"
    "if(s==='dark'||s==='light')return;"
    "if(e.matches)root.removeAttribute('data-theme');"
    "else root.setAttribute('data-theme','light');sync();});}catch(e){}})();")


# These are long documents -- the specification runs past 100 kB of Markdown --
# and the only way back to the table of contents at the top was the scrollbar or
# Ctrl+Home. Offer the button once the reader is a screenful down, and keep it
# out of the tab order until then (display:none, see docs.css).
BACK_TOP_JS = (
    "(function(){var b=document.getElementById('to-top');if(!b)return;"
    "var shown=false,tick=0;"
    "function sync(){var y=window.pageYOffset||"
    "document.documentElement.scrollTop||0;var want=y>400;"
    "if(want===shown)return;shown=want;b.classList.toggle('show',want);}"
    "window.addEventListener('scroll',function(){if(tick)return;"
    "tick=requestAnimationFrame(function(){tick=0;sync();});},"
    "{passive:true});"
    "b.addEventListener('click',function(){"
    "var r=window.matchMedia&&"
    "window.matchMedia('(prefers-reduced-motion: reduce)').matches;"
    "window.scrollTo({top:0,behavior:r?'auto':'smooth'});"
    # The button hides itself on the way up; without this, focus would fall to
    # <body> and a keyboard user would resume tabbing from nowhere.
    "var h=document.querySelector('header.site .brand');"
    "if(h&&h.focus)h.focus({preventScroll:true});});sync();})();")


# Colour the .bvnr blocks with the shared highlighter (loaded as a same-origin
# <script>, so no CSP hash is needed for it -- only for this hook). The module
# injects its own palette, dark and light, keyed on the same html[data-theme]
# attribute these pages use, so there is nothing to style here. Without
# JavaScript the blocks stay legible plain text, which is what they were before.
BOVNAR_HL_JS = (
    "(function(){if(!window.BovnarHighlight)return;"
    "var b=document.querySelectorAll('pre code.language-bovnar');"
    "for(var i=0;i<b.length;i++)"
    "b[i].innerHTML=BovnarHighlight.highlight(b[i].textContent);})();")


def _sha(js):
    return "sha256-" + base64.b64encode(
        hashlib.sha256(js.encode()).digest()).decode()


# Same rule gen_csp.py applies to the hand-written pages: mask comment bodies
# first (a page that mentions "<script>" in prose otherwise opens a phantom
# element), then skip anything with src= or a non-executable type=.
_SCRIPT_RE = re.compile(r"<script\b([^>]*)>(.*?)</script>", re.S)
_COMMENT_RE = re.compile(r"<!--.*?-->", re.S)
_DATA_TYPE_RE = re.compile(r'type\s*=\s*["\']?(application/(ld\+)?json)', re.I)
_HAS_SRC_RE = re.compile(r"\bsrc\s*=", re.I)


def inline_scripts_of(text):
    masked = _COMMENT_RE.sub(lambda m: " " * len(m.group(0)), text)
    return [text[m.start(2):m.end(2)] for m in _SCRIPT_RE.finditer(masked)
            if not _HAS_SRC_RE.search(m.group(1))
            and not _DATA_TYPE_RE.search(m.group(1))]


def csp_for(scripts):
    """The CSP meta for a page carrying exactly `scripts` inline.

    Built per page rather than as one constant listing every hash the generator
    knows. The highlighter hook is only emitted where there are .bvnr blocks to
    colour, so a shared constant left its hash standing on the five pages without
    them — an allowance for a script that is not there. Harmless (a hash permits
    one exact byte sequence, not a class of them) but it is drift, and it is the
    kind nobody sees: the page works either way. Deriving the list from what the
    page actually emits makes --check an exact-match test instead of a
    nothing-is-blocked one.
    """
    hashes = " ".join("'%s'" % _sha(js) for js in scripts)
    return ("default-src 'none'; "
            f"script-src 'self' {gen_csp.GA_SCRIPT_SRC} {hashes}; "
            "style-src 'self' 'unsafe-inline'; "
            f"img-src 'self' data: {gen_csp.GA_IMG_SRC}; font-src 'self'; "
            # These pages fetch nothing of their own; connect-src exists purely
            # so the analytics beacon is not blocked by default-src 'none'.
            f"connect-src {gen_csp.GA_CONNECT_SRC}; "
            # Likewise: these pages frame nothing, so frame-src is here only so
            # the tag's advertising-identifier sync is not caught by
            # default-src 'none'.
            f"frame-src {gen_csp.GA_FRAME_SRC}; "
            "base-uri 'none'; form-action 'none'")

THEME_TOGGLE_BTN = (
    '<button type="button" class="theme-toggle" id="theme-toggle" '
    'aria-label="Switch to light theme" title="Toggle light / dark theme">'
    '<svg class="icon-moon" viewBox="0 0 24 24" fill="none" stroke="currentColor" '
    'stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">'
    '<path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>'
    '<svg class="icon-sun" viewBox="0 0 24 24" fill="none" stroke="currentColor" '
    'stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">'
    '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41'
    'M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/>'
    '</svg></button>')

MARK_SVG = (
    '<svg viewBox="0 0 64 64" aria-hidden="true"><defs><linearGradient id="g" '
    'x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#6cb0ea"/>'
    '<stop offset="1" stop-color="#0a85d6"/></linearGradient></defs>'
    '<path d="M28 13 L12.5 32 L28 51" fill="none" stroke="url(#g)" stroke-width="6.6" '
    'stroke-linecap="round" stroke-linejoin="round"/>'
    '<path d="M36 13 L51.5 32 L36 51" fill="none" stroke="url(#g)" stroke-width="6.6" '
    'stroke-linecap="round" stroke-linejoin="round"/>'
    '<path d="M32 20 L44 32 L32 44 L20 32 Z" fill="#33a78f"/>'
    '<path d="M32 20 L20 32 L32 32 Z" fill="#9cdcfe"/>'
    '<path d="M32 20 L44 32 L32 32 Z" fill="#4ec9b0"/>'
    '<path d="M20 32 L32 44 L32 32 Z" fill="#3fb89e"/>'
    '<path d="M44 32 L32 44 L32 32 Z" fill="#2c9281"/></svg>')

# Same wording and same order as the landing page's nav for the same
# destinations: it called /docs/ "Docs" here and "Documentation" there, and
# lower-cased the "Start" the landing capitalises -- so the two bars named the
# same places differently, one click apart. The set is a subset (a doc page has
# no reason to link the landing's individual sections); the labels are not.
# The nbsp keeps each label on one line, which the landing gets from having
# every narrow-viewport case hidden by a media query instead.
HEADER = (
    '<header class="site">'
    '<a class="brand" href="/">' + MARK_SVG +
    'BOVNAR<span class="ext">.bvnr</span></a>'
    '<nav>'
    '<a href="/#playground">Playground</a>'
    '<a href="/#quickstart">Quick&nbsp;Start</a>'
    '<a href="/docs/">Documentation</a>'
    '<a class="cta" href="https://github.com/sothis/bovnar">GitHub&nbsp;↗</a>'
    + THEME_TOGGLE_BTN +
    '</nav></header>')

BACK_TOP_BTN = (
    '<button type="button" class="to-top" id="to-top" '
    'aria-label="Back to top" title="Back to top">'
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" '
    'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">'
    '<path d="M12 19V6M5 13l7-7 7 7"/></svg></button>')

# The Impressum belongs in every footer, not just the landing page's. These pages
# are indexable and rank on their own, so a reader arriving from a search engine
# lands here with no route to the provider identification that § 5 DDG requires
# to be reachable from any page of the offering.
FOOTER = (
    '<footer>Bovnar (BVNR) spec ' + VERSION +
    ' · <a href="/">Home</a> · <a href="/docs/">All docs</a> · '
    '<a href="https://github.com/sothis/bovnar">Source</a> · '
    '<a href="/impressum.html">Impressum</a> · '
    '<a href="/privacy.html">Privacy</a> · MIT-licensed'
    '</footer>')


def page(title, description, canonical, body, extra_head="", og_dates=None):
    esc = html.escape
    date_meta = ""
    if og_dates:
        pub, mod = og_dates
        date_meta = (f'<meta property="article:published_time" content="{pub}">\n'
                     f'<meta property="article:modified_time" content="{mod}">\n')
    # The highlighter is only worth 13 kB to a page that has .bvnr blocks to
    # colour; the docs index and the cheatsheet have none.
    bvnr_hl = ""
    has_hl = 'class="language-bovnar"' in body
    # exactly the inline scripts this page emits, in no particular order
    page_csp = csp_for([THEME_INIT_JS, THEME_TOGGLE_JS, BACK_TOP_JS,
                        gen_csp.CONSENT_JS]
                       + ([BOVNAR_HL_JS] if has_hl else []))
    if has_hl:
        bvnr_hl = (f'<script src="/bovnar_highlight.js'
                   f'{css_stamp("bovnar_highlight.js")}"></script>\n'
                   f'<script>{BOVNAR_HL_JS}</script>\n')
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="{page_csp}">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<script>{THEME_INIT_JS}</script>
<title>{esc(title)}</title>
<meta name="description" content="{esc(description)}">
<meta name="theme-color" content="#1e1e1e">
<meta name="robots" content="index, follow">
<link rel="canonical" href="{canonical}">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="icon" type="image/png" sizes="32x32" href="/favicon-32.png">
<meta property="og:type" content="article">
<meta property="og:site_name" content="Bovnar">
<meta property="og:title" content="{esc(title)}">
<meta property="og:description" content="{esc(description)}">
<meta property="og:url" content="{canonical}">
<meta property="og:image" content="{SITE}/bovnar-og.png">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="Bovnar (BVNR) — unit-safe serialization">
{date_meta}
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="{esc(title)}">
<meta name="twitter:description" content="{esc(description)}">
<meta name="twitter:image" content="{SITE}/bovnar-og.png">
<meta name="twitter:image:alt" content="Bovnar (BVNR) — unit-safe serialization">
<link rel="stylesheet" href="/fonts/fonts.css{css_stamp("fonts/fonts.css")}">
<link rel="stylesheet" href="/docs/docs.css{css_stamp("docs/docs.css")}">
{extra_head}</head>
<body>
{HEADER}
<main>
{body}
</main>
{FOOTER}
{BACK_TOP_BTN}
{bvnr_hl}<script>{THEME_TOGGLE_JS}</script>
<script>{BACK_TOP_JS}</script>
{gen_csp.consent_banner("en")}
</body>
</html>
"""


def faq_items(md_text):
    """(question, answer) pairs from the FAQ Markdown — bold questions ending in
    '?' followed by their answer, up to the next question or heading."""
    items = []
    pattern = re.compile(r"^\*\*(.+?\?)\*\*\s*$", re.M)
    matches = list(pattern.finditer(md_text))
    for i, m in enumerate(matches):
        q = m.group(1).strip()
        start = m.end()
        end = matches[i + 1].start() if i + 1 < len(matches) else len(md_text)
        ans = md_text[start:end]
        h = re.search(r"^#{1,6}\s", ans, re.M)
        if h:
            ans = ans[:h.start()]
        ans = re.sub(r"\s+", " ", re.sub(r"[`*#>|]", "", ans)).strip()
        if len(ans) < 20:
            continue
        items.append((q, ans[:600]))
    return items


def json_ld(obj):
    import json
    blob = json.dumps(obj, ensure_ascii=False).replace("</", "<\\/")
    return f'<script type="application/ld+json">{blob}</script>\n'


def breadcrumb(name, url):
    return {
        "@type": "BreadcrumbList",
        "itemListElement": [
            {"@type": "ListItem", "position": 1, "name": "Home", "item": f"{SITE}/"},
            {"@type": "ListItem", "position": 2, "name": "Documentation",
             "item": f"{SITE}/docs/"},
            {"@type": "ListItem", "position": 3, "name": name, "item": url},
        ],
    }


# @id values mirror the entities defined in the landing page's JSON-LD graph, so
# search engines merge them into one site-wide entity graph across all pages.
AUTHOR = {"@type": "Person", "@id": f"{SITE}/#author", "name": "Janos Sonntag"}
PUBLISHER = {
    "@type": "Organization",
    "@id": f"{SITE}/#organization",
    "name": "Bovnar",
    "url": f"{SITE}/",
    "logo": {"@type": "ImageObject", "url": f"{SITE}/apple-touch-icon.png",
             "width": 180, "height": 180},
}
WEBSITE_REF = {"@type": "WebSite", "@id": f"{SITE}/#website",
               "name": "Bovnar", "url": f"{SITE}/"}


def doc_schema(title, desc, canonical, published, modified, faqs=None):
    """TechArticle + BreadcrumbList (+ FAQPage) structured data for a doc page.
    `title` is the clean doc title (no site suffix); published/modified are the
    git-derived ISO dates."""
    graph = [
        {
            "@type": "TechArticle",
            "headline": title,
            "description": desc,
            "url": canonical,
            "mainEntityOfPage": canonical,
            "inLanguage": "en",
            "image": [f"{SITE}/bovnar-og.png"],
            "datePublished": published,
            "dateModified": modified,
            "author": AUTHOR,
            "publisher": PUBLISHER,
            "isPartOf": WEBSITE_REF,
        },
        breadcrumb(title, canonical),
    ]
    if faqs:
        graph.append({
            "@type": "FAQPage",
            "mainEntity": [
                {"@type": "Question", "name": q,
                 "acceptedAnswer": {"@type": "Answer", "text": a}}
                for q, a in faqs
            ],
        })
    return json_ld({"@context": "https://schema.org", "@graph": graph})


def build_index_page():
    rows = []
    for src, slug, pdf, title, desc in DOCS:
        # The grammar is shipped as EBNF/plain text, not Markdown — label its
        # raw-source link accordingly so the format link doesn't lie.
        raw_label = "EBNF" if src.endswith(".ebnf") else "Markdown"
        rows.append(
            f'<li><a href="/docs/{slug}/"><strong>{html.escape(title)}</strong></a>'
            f'<div class="d">{html.escape(desc)}</div>'
            f'<div class="fmt"><a href="/doc/{src}">{raw_label}</a> · '
            f'<a href="/doc/pdf/{pdf}.pdf">PDF</a></div></li>')
    body = (
        '<h1>Bovnar documentation</h1>'
        '<p class="doc-meta">Tutorial, specification, unit system, C and Python '
        'APIs, grammar, FAQ, and more — as HTML, Markdown, and PDF.</p>'
        '<ul class="doclist">' + "".join(rows) + '</ul>')
    schema = json_ld({
        "@context": "https://schema.org",
        "@graph": [
            {"@type": "CollectionPage", "url": f"{SITE}/docs/",
             "name": "Bovnar documentation", "inLanguage": "en",
             "isPartOf": WEBSITE_REF, "publisher": PUBLISHER},
            {"@type": "BreadcrumbList", "itemListElement": [
                {"@type": "ListItem", "position": 1, "name": "Home", "item": f"{SITE}/"},
                {"@type": "ListItem", "position": 2, "name": "Documentation",
                 "item": f"{SITE}/docs/"}]},
        ],
    })
    return page(
        "Documentation · Bovnar",
        "Bovnar (BVNR) documentation: tutorial, specification, unit system, C and "
        "Python APIs, grammar, FAQ, conformance, and streaming.",
        f"{SITE}/docs/", body, schema)


# A doc source that has lost its content still renders — as an empty page that
# --check then happily calls "current", because the generated file matches the
# source it was generated from. That is how doc/09 shipped empty for one commit:
# a tool truncated it, the page regenerated to match, and every gate agreed.
# Nothing downstream can catch this, so catch it here, at the only point that
# sees the source.
MIN_DOC_BYTES = 2000


def build_doc_page(src, slug, pdf, title, desc):
    path = os.path.join(DOC_DIR, src)
    text = open(path, encoding="utf-8").read()
    if len(text.encode("utf-8")) < MIN_DOC_BYTES or "\n" not in text.strip():
        raise SystemExit(
            "gen_html_docs.py: %s is %d bytes — a documentation source this "
            "short has almost certainly been truncated by a tool. Restore it "
            "(git checkout) rather than publishing an empty page."
            % (src, len(text.encode("utf-8"))))
    if not src.endswith(".ebnf") and not re.search(r"^#\s+\S", text, re.M):
        raise SystemExit(
            "gen_html_docs.py: %s has no top-level heading; refusing to "
            "generate a page from it." % src)
    extra_head = ""
    if src.endswith(".ebnf"):
        body_md = "```\n" + text + "\n```"
        h1 = title
        rendered = render_markdown(body_md)
    else:
        m = re.search(r"^#\s+(.*)$", text, re.M)
        h1 = m.group(1).strip() if m else title
        text_wo_h1 = re.sub(r"^#\s+.*$", "", text, count=1, flags=re.M)
        rendered = rewrite_links(render_markdown(text_wo_h1))
    canonical = f"{SITE}/docs/{slug}/"
    faqs = faq_items(text) if slug == "faq" else None
    published = modified = None
    if not has_history():
        published, modified = page_date(slug, True), page_date(slug, False)
    published = published or git_date(f"doc/{src}", first=True)
    modified = modified or git_date(f"doc/{src}")
    # Advertise this doc's raw-source counterpart for LLM/agent tools (RFC 7763
    # for Markdown; the grammar ships as EBNF/plain text, so label + media type
    # follow the actual format rather than claiming Markdown).
    is_ebnf = src.endswith(".ebnf")
    raw_label = "EBNF" if is_ebnf else "Markdown"
    raw_type = "text/plain" if is_ebnf else "text/markdown"
    extra_head = (f'<link rel="alternate" type="{raw_type}" href="/doc/{src}">\n'
                  + doc_schema(title, desc, canonical, published, modified, faqs))
    meta = (f'<p class="doc-meta">Bovnar (BVNR) spec {VERSION} documentation · '
            f'Also available as <a href="/doc/{src}">{raw_label}</a> and '
            f'<a href="/doc/pdf/{pdf}.pdf">PDF</a>.</p>')
    body = f"<h1>{html.escape(h1)}</h1>\n{meta}\n{rendered}"
    # `title` already leads with "Bovnar", so no redundant "· Bovnar" suffix.
    return page(title, desc, canonical, body, extra_head, og_dates=(published, modified))


def main():
    check = "--check" in sys.argv[1:]
    if shallow_clone():
        print("gen_html_docs.py: shallow git clone -- the per-file commit dates "
              "these pages embed are unavailable here, so every page would be "
              "dated today. Fetch the full history ('git fetch --unshallow', or "
              "fetch-depth: 0 for actions/checkout) and re-run.", file=sys.stderr)
        return 2
    targets = {}
    for src, slug, pdf, title, desc in DOCS:
        targets[os.path.join(OUT_DIR, slug, "index.html")] = \
            build_doc_page(src, slug, pdf, title, desc)
    targets[os.path.join(OUT_DIR, "index.html")] = build_index_page()

    # Every page's CSP must name exactly the inline scripts that page carries.
    # gen_csp.py polices index.html, impressum.html and 404.html; these eleven are
    # generated here and were covered by nothing, so a hash that stopped matching
    # its script would have shipped a page whose own JavaScript the browser
    # refuses to run — silently, since the page renders either way. Checked on
    # the text about to be written, so it holds for --check and for a real run.
    csp_bad = []
    for path, text in targets.items():
        declared = set(re.findall(r"'sha256-([A-Za-z0-9+/=]+)'",
                                  re.search(r'Content-Security-Policy" content="'
                                            r'([^"]*)"', text).group(1)))
        actual = {base64.b64encode(hashlib.sha256(js.encode()).digest()).decode()
                  for js in inline_scripts_of(text)}
        if declared != actual:
            csp_bad.append("%s (%d script(s), %d hash(es); %d unhashed, %d orphan)"
                           % (os.path.relpath(path, ROOT), len(actual),
                              len(declared), len(actual - declared),
                              len(declared - actual)))
    if csp_bad:
        print("gen_html_docs.py: CSP does not match the page's own scripts: "
              + "; ".join(csp_bad), file=sys.stderr)
        return 1

    if check:
        stale = [os.path.relpath(p, ROOT) for p, t in targets.items()
                 if (not os.path.exists(p)) or open(p, encoding="utf-8").read() != t]
        if stale:
            print("gen_html_docs.py --check: stale/missing: " + ", ".join(stale),
                  file=sys.stderr)
            return 1
        print(f"gen_html_docs.py --check: {len(targets)} HTML doc pages are "
              f"current, and each CSP names exactly its own inline scripts")
        return 0

    print("==> Generating HTML doc pages (gen_html_docs.py)")
    for path, text in targets.items():
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(text)
    print(f"  wrote {len(targets)} pages under web/docs/")
    return 0


if __name__ == "__main__":
    sys.exit(main())
