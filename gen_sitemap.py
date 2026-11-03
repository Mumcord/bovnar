#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""Generate web/sitemap.xml with accurate, git-derived <lastmod> dates.

The sitemap was hand-maintained, so its <lastmod> dates drifted from the actual
content (PDFs stamped 2026-06-01 long after the docs changed). This builds it
from the real last-commit date of each source file, falling back to the file
mtime and then today. It also lists the new HTML doc pages (/docs/<slug>/) as the
canonical, indexable form of each document.

publish_web.sh runs this after the doc/LLM generators. Run: python3 gen_sitemap.py
"""
import datetime
import os
import subprocess
import sys

from gen_html_docs import DOCS, SITE  # single source of truth for the doc set

ROOT = os.path.dirname(os.path.abspath(__file__))


def git_date(relpath):
    """Last-commit date (YYYY-MM-DD) of a repo file, or None if untracked."""
    try:
        out = subprocess.run(
            ["git", "-C", ROOT, "log", "-1", "--format=%cs", "--", relpath],
            capture_output=True, text=True, check=True).stdout.strip()
        return out or None
    except Exception:
        return None


def mtime_date(path):
    try:
        return datetime.date.fromtimestamp(os.path.getmtime(path)).isoformat()
    except OSError:
        return None


def lastmod(relpath):
    """Prefer the git commit date; fall back to file mtime, then today."""
    return (git_date(relpath)
            or mtime_date(os.path.join(ROOT, relpath))
            or datetime.date.today().isoformat())


def newest(relpaths):
    dates = [lastmod(p) for p in relpaths]
    return max(dates)


def url(loc, date, priority):
    return (f"  <url>\n    <loc>{loc}</loc>\n    <lastmod>{date}</lastmod>\n"
            f"    <priority>{priority}</priority>\n  </url>")


def build():
    doc_srcs = [f"doc/{src}" for src, *_ in DOCS]
    home = lastmod("web/index.html")
    newest_doc = newest(doc_srcs)

    entries = []
    # Landing pages (the German edition is generated from index.html).
    entries.append(url(f"{SITE}/", home, "1.0"))
    entries.append(url(f"{SITE}/de/", home, "0.9"))
    # HTML documentation — the canonical, indexable form.
    entries.append(url(f"{SITE}/docs/", newest_doc, "0.8"))
    for src, slug, _pdf, _title, _desc in DOCS:
        entries.append(url(f"{SITE}/docs/{slug}/", lastmod(f"doc/{src}"), "0.7"))
    # PDF editions (a distinct, searchable format; built from the same sources).
    for src, _slug, pdf, _title, _desc in DOCS:
        entries.append(url(f"{SITE}/doc/pdf/{pdf}.pdf", lastmod(f"doc/{src}"), "0.6"))
    # LLM-facing entry points.
    entries.append(url(f"{SITE}/index.md", lastmod("README.md"), "0.7"))
    entries.append(url(f"{SITE}/de/index.md", lastmod("README.md"), "0.6"))
    entries.append(url(f"{SITE}/llms.txt", lastmod("README.md"), "0.5"))
    entries.append(url(f"{SITE}/llms-full.txt", newest_doc, "0.5"))

    return ('<?xml version="1.0" encoding="UTF-8"?>\n'
            '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
            + "\n".join(entries) + "\n</urlset>\n")


def main():
    check = "--check" in sys.argv[1:]
    out = build()
    path = os.path.join(ROOT, "web", "sitemap.xml")
    if check:
        cur = open(path, encoding="utf-8").read() if os.path.exists(path) else None
        if cur != out:
            print("gen_sitemap.py --check: web/sitemap.xml is stale", file=sys.stderr)
            return 1
        print("gen_sitemap.py --check: web/sitemap.xml is current")
        return 0
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(out)
    n = out.count("<url>")
    print(f"==> Wrote web/sitemap.xml ({n} URLs)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
