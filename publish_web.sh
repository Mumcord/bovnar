#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# publish_web.sh — deploy the Bovnar website (everything under ./web) to the
# live server's web root with rsync.
#
# The web/doc directory is a symlink to ../doc (the canonical Markdown sources).
# This script stages the site into a temp directory, *resolving that symlink* so
# the docs land as real files under <webroot>/doc — never as a dangling link.
#
# Only the documents named in the WEBDOCS table below are published out of doc/.
# Anything else there — working notes, drafts, papers under submission — stays
# private without needing to be listed anywhere. Add a document to WEBDOCS to
# publish it; that is the only step.
#
# The documentation PDFs and the bovnar-docs-pdf.zip bundle are NOT kept in the
# repository; they are generated under build/doc/pdf/ (git-ignored). Pass --pdf
# to (re)build them with gen_doc_pdfs.py before uploading. Without --pdf the
# script ships whatever already exists in build/doc/pdf/ (and warns if empty).
#
# Usage:
#   ./publish_web.sh [options] [user@host:/var/www/html]
#
# The destination may be given as the last argument, or via the
# BOVNAR_PUBLISH_DEST environment variable. The argument wins if both are set.
#
# Options:
#   --pdf            Regenerate the doc PDFs + zip bundle before publishing.
#   --delete         Pass --delete to rsync (prune files removed locally).
#   -n, --dry-run    Show what rsync would transfer; do not build or upload.
#   -h, --help       Show this help and exit.
#
# Examples:
#   ./publish_web.sh me@bovnar.org:/var/www/html
#   ./publish_web.sh --pdf me@bovnar.org:/var/www/html
#   ./publish_web.sh --pdf --delete me@bovnar.org:/var/www/html
#   BOVNAR_PUBLISH_DEST=me@bovnar.org:/var/www/html ./publish_web.sh --pdf
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WEB_DIR="$ROOT/web"
GEN_PDF="$ROOT/gen_doc_pdfs.py"
PDF_BUILD_DIR="$ROOT/build/doc/pdf"   # where gen_doc_pdfs.py writes its output

# ── What gets published out of doc/ ─────────────────────────────────────────
#
# web/doc is a symlink to ../doc, so everything in doc/ used to reach the live
# site by default and each thing that should not was subtracted afterwards. That
# is the wrong way round for a directory that holds working documents next to
# published ones: the German Wikipedia draft, the Internet-Draft and the JOSS
# paper each shipped -- or would have -- until someone noticed and added a line.
# The JOSS paper actually did, and sat readable at /doc/joss_paper/paper.md.
#
# So doc/ is now allowlisted: nothing under it is published unless it is named
# here. A new working document in doc/ is private by default, and forgetting
# this table means a document is missing from the site -- loud, and the right
# way for it to fail -- rather than a private draft being world-readable, which
# is silent and the wrong way.
#
# Entries are paths relative to doc/ and may name a file or a directory. The
# PDFs are NOT here: they are generated into build/doc/pdf/ and dropped into the
# staging tree separately (step 2 below).
WEBDOCS=(
    "01_bovnar_tutorial.md"
    "02_bovnar_faq.md"
    "03_bovnar_spec.md"
    "04_bovnar_unit_cheatsheet.md"
    "05_bovnar_unit_system.md"
    "06_bovnar_unit_policy.md"
    "07_bovnar_unit_ambiguities.md"
    "08_bovnar_readwrite_api.md"
    "09_bovnar_python_bindings.md"
    "10_bovnar_streaming.md"
    "11_bovnar_unit_profiles.md"
    "12_bovnar.ebnf"
    "13_bovnar_conformance.md"
)

# Paths (relative to the web root) that must NOT be published. These are dev
# helpers that live under web/ itself. Anything under doc/ is governed by
# WEBDOCS above and needs no entry here.
EXCLUDES=(
    "httpd.sh"
    # Translation SOURCE tables. They are a build input -- gen_i18n.py has
    # already baked them into web/<lang>/index.html by the time we stage -- and
    # they carry the _orphaned graveyard of superseded strings. There is no
    # reason to serve them from the live root.
    "i18n"
)

BUILD_PDF=0
DRY_RUN=0
RSYNC_DELETE=0
DEST="${BOVNAR_PUBLISH_DEST:-}"

# The header block: line 2 through the last comment line before the first line
# of code. The range used to be hard-coded (2,32p), which printed shell source
# when the header was short and truncated the help the moment it grew -- adding
# five lines above cut --help off mid-sentence. Ending it at the first
# non-comment line means it cannot drift again.
usage() { sed -n '2,/^[^#]/p' "${BASH_SOURCE[0]}" | sed -n 's/^# \{0,1\}//p'; }

while [ $# -gt 0 ]; do
    case "$1" in
        --pdf|--build-pdf) BUILD_PDF=1 ;;
        --delete)          RSYNC_DELETE=1 ;;
        -n|--dry-run)      DRY_RUN=1 ;;
        -h|--help)         usage; exit 0 ;;
        -*) echo "publish_web.sh: unknown option: $1" >&2; exit 2 ;;
        *)  DEST="$1" ;;
    esac
    shift
done

if [ -z "$DEST" ]; then
    echo "publish_web.sh: no destination given." >&2
    echo "Pass it as an argument or set BOVNAR_PUBLISH_DEST, e.g.:" >&2
    echo "  ./publish_web.sh me@host:/var/www/html" >&2
    exit 2
fi

# ── 1. Optionally (re)build the PDFs + zip into build/doc/pdf/ ───────────────
if [ "$BUILD_PDF" -eq 1 ]; then
    echo "==> Building documentation PDFs + zip (gen_doc_pdfs.py)"
    if [ "$DRY_RUN" -eq 1 ]; then
        echo "    [dry-run] would run: python3 $GEN_PDF"
    else
        python3 "$GEN_PDF"
    fi
fi

# Warn if we are about to ship a site with no PDFs at all.
if ! ls "$PDF_BUILD_DIR"/*.pdf >/dev/null 2>&1; then
    echo "WARNING: no PDFs in build/doc/pdf/ — the download links will 404." >&2
    echo "         Re-run with --pdf to generate them." >&2
fi

# ── 1a0. Content-stamp the fonts and the stylesheet that reaches them ───────
# Runs before everything else that hashes a stamped file: this step rewrites
# web/fonts/fonts.css, and gen_html_docs.py stamps its own link to that file
# from those very bytes.
echo "==> Stamping fonts (gen_font_stamps.py)"
if [ "$DRY_RUN" -eq 1 ]; then
    python3 "$ROOT/gen_font_stamps.py" --check
else
    python3 "$ROOT/gen_font_stamps.py"
fi

# ── 1a. Stamp the Content-Security-Policy into the served-directly pages ─────
# index.html and impressum.html carry a hash-based CSP that must match their
# inline scripts. gen_i18n.py restamps the translated editions itself.
echo "==> Stamping CSP (gen_csp.py)"
if [ "$DRY_RUN" -eq 1 ]; then
    python3 "$ROOT/gen_csp.py" --check
else
    python3 "$ROOT/gen_csp.py"
fi

# ── 1a2. Regenerate the LLM-facing views ────────────────────────────────────
# llms.txt, index.md, and llms-full.txt are built from README.md + doc/*.md so
# they can never drift from the real documentation. gen_llms.py --check fails the
# dry run if the committed copies are stale.
echo "==> Generating LLM views (gen_llms.py)"
if [ "$DRY_RUN" -eq 1 ]; then
    python3 "$ROOT/gen_llms.py" --check
else
    python3 "$ROOT/gen_llms.py"
fi

# ── 1a3. Render the docs as indexable HTML pages (web/docs/) ─────────────────
echo "==> Generating HTML doc pages (gen_html_docs.py)"
if [ "$DRY_RUN" -eq 1 ]; then
    python3 "$ROOT/gen_html_docs.py" --check
else
    python3 "$ROOT/gen_html_docs.py"
fi

# ── 1a4. Regenerate sitemap.xml with git-derived lastmod dates ───────────────
# Runs after the doc/LLM generators so every referenced file exists.
echo "==> Generating sitemap (gen_sitemap.py)"
if [ "$DRY_RUN" -eq 1 ]; then
    python3 "$ROOT/gen_sitemap.py" --check
else
    python3 "$ROOT/gen_sitemap.py"
fi

# ── 1b. Regenerate the translated editions ──────────────────────────────────
# web/<lang>/index.html is generated, git-ignored, and rebuilt on every publish,
# so a translated page can never lag behind the English source it is spliced
# from. gen_i18n.py exits non-zero if any string is untranslated or if a script
# path would break one directory down, which fails the publish before upload.
for _lang_table in "$WEB_DIR"/i18n/*.json; do
    [ -e "$_lang_table" ] || continue
    _lang="$(basename "$_lang_table" .json)"
    echo "==> Generating web/$_lang/ from index.html + i18n/$_lang.json"
    if [ "$DRY_RUN" -eq 1 ]; then
        echo "    [dry-run] would run: python3 $ROOT/gen_i18n.py $_lang"
        python3 "$ROOT/gen_i18n.py" --check "$_lang"
    else
        python3 "$ROOT/gen_i18n.py" "$_lang"
    fi
done

# ── 2. Stage the site, resolving the web/doc symlink to real files ──────────
STAGE="$(mktemp -d "${TMPDIR:-/tmp}/bovnar-publish.XXXXXX")"
trap 'rm -rf "$STAGE"' EXIT

# -L dereferences symlinks (so web/doc -> ../doc becomes a real doc/ tree).
cp -rL "$WEB_DIR"/. "$STAGE"/

# Rebuild the staged doc/ from WEBDOCS alone. The copy above brought the whole
# directory across; this throws that away and puts back only what is named, so
# a document nobody listed cannot reach the server by being in the tree.
rm -rf "$STAGE/doc"
mkdir -p "$STAGE/doc"
for rel in "${WEBDOCS[@]}"; do
    if [ ! -e "$ROOT/doc/$rel" ]; then
        echo "publish_web.sh: WEBDOCS names doc/$rel, which does not exist." >&2
        echo "  A typo here silently drops a document from the site; fix the" >&2
        echo "  entry or remove it. Nothing has been uploaded." >&2
        exit 1
    fi
    mkdir -p "$STAGE/doc/$(dirname "$rel")"
    cp -rL "$ROOT/doc/$rel" "$STAGE/doc/$rel"
done

# Every document with an HTML page under /docs/<slug>/ must ship its Markdown
# source too: the page advertises it as rel="alternate", the nginx config gives
# it a rel="canonical" back, and the landing page links it directly. Publishing
# the page without the source turns all three into 404s, which is exactly the
# failure this allowlist could newly cause -- so it is checked rather than
# trusted. gen_html_docs.DOCS is the source of truth for what has a page.
python3 - "$ROOT" "$STAGE" <<'PYEOF' || exit 1
import os, sys
root, stage = sys.argv[1], sys.argv[2]
sys.path.insert(0, root)          # publish may be run from any directory
from gen_html_docs import DOCS
missing = [src for src, *_ in DOCS
           if not os.path.exists(os.path.join(stage, "doc", src))]
if missing:
    sys.stderr.write(
        "publish_web.sh: these documents have an HTML page under /docs/ but are "
        "not in WEBDOCS, so the page would link at a raw source that is not "
        "there: %s\n" % ", ".join(missing))
    raise SystemExit(1)
PYEOF

# Drop in the generated PDFs (they live under build/, not in the repo tree).
# After the doc/ rebuild above, which would otherwise delete them again.
if ls "$PDF_BUILD_DIR"/* >/dev/null 2>&1; then
    mkdir -p "$STAGE/doc/pdf"
    cp "$PDF_BUILD_DIR"/* "$STAGE/doc/pdf/"
fi

# Drop excluded paths (web/ dev helpers) from the staging tree.
for rel in "${EXCLUDES[@]}"; do
    rm -rf "$STAGE/$rel"
done

# ── 3. Upload with rsync ────────────────────────────────────────────────────
# --chmod normalises permissions on the destination: directories 755, files 644.
# Without it, `-a` (which implies -p) would copy the staging dir's own mode onto
# the web root — and `mktemp -d` makes that 0700, leaving /var/www/html
# un-traversable by the web server (403). Forcing world-readable/-traversable
# perms here is both the fix and idempotent on every subsequent publish.
RSYNC_OPTS=(-az --human-readable --chmod=D755,F644)
# --delete prunes everything at the destination that is not in the staging tree,
# and the staging tree is only what lives under web/. Anything the SERVER owns
# must be protected explicitly -- above all .well-known/acme-challenge, which is
# where certbot writes ACME challenges: deleting it breaks TLS renewal. Use
# --delete-after so a transfer that fails partway does not leave the live site
# with files already removed.
[ "$RSYNC_DELETE" -eq 1 ] && RSYNC_OPTS+=(--delete-after --exclude='/.well-known/')
[ "$DRY_RUN" -eq 1 ] && RSYNC_OPTS+=(--dry-run --itemize-changes)

echo "==> Publishing to: $DEST"
[ "$DRY_RUN" -eq 1 ] && echo "    [dry-run] rsync ${RSYNC_OPTS[*]} <staged web/> $DEST/"

# Trailing slash on the source = copy the *contents* of the staging dir into
# the destination directory (not the staging dir itself).
rsync "${RSYNC_OPTS[@]}" "$STAGE"/ "$DEST"/

[ "$DRY_RUN" -eq 1 ] || echo "==> Done."
