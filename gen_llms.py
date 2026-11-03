#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""Generate the LLM-facing plain-text/Markdown views of the Bovnar website.

Produces three files under web/ from the canonical Markdown sources (README.md
and doc/*.md), so they can never drift from the real documentation:

  web/llms.txt        curated content map per the llmstxt.org convention:
                      an H1 name, a one-line summary, and H2 sections of
                      annotated links to the Markdown docs and project resources.
  web/index.md        a clean Markdown rendering of the landing page (the
                      "/index.md" alternate advertised from index.html), sliced
                      from README's overview/features sections plus a docs index.
  web/llms-full.txt   the whole documentation set concatenated into one file,
                      the single-fetch view (llms-full.txt convention).

publish_web.sh runs this before staging, so the live site is always current.
Run it by hand any time to refresh the committed copies:  python3 gen_llms.py
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
WEB = os.path.join(ROOT, "web")
SITE = "https://www.bovnar.io"
REPO = "https://github.com/sothis/bovnar"

# Served Markdown docs (doc/ is symlinked to web/doc/), in reading order, with a
# display title, the URL slug, and a one-line description for the link maps.
DOCS = [
    ("01_bovnar_tutorial.md",     "Tutorial",
     "A guided introduction to writing and reading Bovnar documents."),
    ("02_bovnar_faq.md",          "FAQ",
     "Common questions on types, units, limits, encoding, and the API."),
    ("03_bovnar_spec.md",         "Specification",
     "The complete format specification (spec 1.2): grammar, types, units, limits."),
    ("04_bovnar_unit_cheatsheet.md","Unit & currency cheatsheet",
     "Every unit symbol, prefix, and currency code in one reference table."),
    ("05_bovnar_unit_system.md",  "Unit system",
     "The 264 physical units, SI/IEC prefixes, and 216 currencies, with dimensions and factors."),
    ("06_bovnar_unit_policy.md",  "Unit policy",
     "What a policy validates and converts, the order it resolves in, and the errors it raises."),
    ("07_bovnar_unit_ambiguities.md","Unit ambiguities",
     "Which reading wins when a token could mean two things, and how to write the other."),
    ("08_bovnar_readwrite_api.md","Read & Write C API",
     "The C reader/writer/DOM API reference (bovnar.h, bovnar_dom.h)."),
    ("09_bovnar_python_bindings.md","Python bindings",
     "The pure-ctypes Python package: loads/dumps, streaming, DOM, Quantity, NumPy/Pint bridges."),
    ("10_bovnar_streaming.md",    "Streaming & framing",
     "Octet streams, frames, multiplexing/demultiplexing, and embedded documents."),
    ("11_bovnar_unit_profiles.md","Unit profiles",
     "The ucum:, unece:, qudt:, qudt-qk:, udunits:, om: and cf: notations, their "
     "transliteration tables, what they refuse, and how they are checked "
     "against each other."),
    ("12_bovnar.ebnf",            "EBNF grammar",
     "The formal grammar, annotated against the reference lexer/validator."),
    ("13_bovnar_conformance.md",  "Conformance",
     "The 470-case conformance suite and the IUT protocol for third-party implementations."),
]

# External project resources for the llms.txt "Project" section.
PROJECT_LINKS = [
    (f"{SITE}/", "Website",
     "The Bovnar homepage and interactive browser playground."),
    (REPO, "Source (GitHub)",
     "The C99 reference implementation, Python bindings, and tooling (MIT-licensed)."),
    ("https://www.iana.org/assignments/media-types/text/vnd.bovnar",
     "IANA media type (text/vnd.bovnar)",
     "The registered media type for .bvnr documents."),
    ("https://doi.org/10.5281/zenodo.21443295",
     "DOI — Documentation & Specification (latest)",
     "Concept DOI for the archived, citable specification and documentation; "
     "resolves to the newest version."),
    ("https://doi.org/10.5281/zenodo.21443008",
     "DOI — Source (latest)",
     "Concept DOI for the archived, citable source code; resolves to the "
     "newest release."),
]


def read(path):
    with open(path, "r", encoding="utf-8") as fh:
        return fh.read()


def write(path, text):
    with open(path, "w", encoding="utf-8") as fh:
        fh.write(text)
    print(f"  wrote {os.path.relpath(path, ROOT)} ({len(text.encode('utf-8'))} bytes)")


def split_readme_sections(md):
    """Return (preamble, {h2_title: body}) split on level-2 headings.

    Badge lines ([![...]](...)) are stripped from the preamble. Body text keeps
    its original Markdown verbatim.
    """
    lines = md.splitlines()
    preamble = []
    sections = {}
    order = []
    cur = None
    buf = []
    for line in lines:
        m = re.match(r"^##\s+(.*\S)\s*$", line)
        if m:
            if cur is None:
                preamble = buf
            else:
                sections[cur] = "\n".join(buf).strip("\n")
            cur = m.group(1).strip()
            order.append(cur)
            buf = []
        else:
            buf.append(line)
    if cur is None:
        preamble = buf
    else:
        sections[cur] = "\n".join(buf).strip("\n")
    # drop badge/image-only lines and the horizontal rules from the preamble
    pre = [l for l in preamble
           if not l.strip().startswith("[![") and l.strip() != "---"]
    return "\n".join(pre).strip("\n"), sections, order


def tagline(preamble):
    for line in preamble.splitlines():
        s = line.strip()
        if s.startswith("**") and s.endswith("**") and len(s) > 4:
            return s.strip("*").strip()
    return "Unit-safe serialization for scientific and industrial systems."


def docs_index_md():
    out = ["## Documentation", ""]
    for slug, title, desc in DOCS:
        out.append(f"- [{title}]({SITE}/doc/{slug}) — {desc}")
    out.append("")
    out.append(f"Browse the documentation as HTML at [{SITE}/docs/]({SITE}/docs/). "
               f"A single-file copy of the whole set is at "
               f"[{SITE}/llms-full.txt]({SITE}/llms-full.txt).")
    return "\n".join(out)


def build_index_md(preamble, sections):
    """The Markdown landing page: title + tagline + the load-bearing README
    sections + a generated documentation index."""
    parts = [preamble, ""]
    for name in ("Links", "Overview", "Key Features", "Format at a Glance"):
        if name in sections:
            parts.append(f"## {name}")
            parts.append("")
            parts.append(sections[name])
            parts.append("")
    parts.append(docs_index_md())
    parts.append("")
    return "\n".join(parts).rstrip("\n") + "\n"


def build_llms_txt(tag):
    out = []
    out.append("# Bovnar (BVNR)")
    out.append("")
    out.append(f"> {tag}")
    out.append("")
    out.append("Bovnar is a self-describing data serialization format: every value "
               "carries its own type family, bit-width, numeric base, and physical "
               "unit inline, validated by the parser, with no external schema. This "
               "file maps the Markdown documentation and project resources.")
    out.append("")
    out.append("## Documentation")
    out.append("")
    for slug, title, desc in DOCS:
        out.append(f"- [{title}]({SITE}/doc/{slug}): {desc}")
    out.append("")
    out.append("## Project")
    out.append("")
    for url, title, desc in PROJECT_LINKS:
        out.append(f"- [{title}]({url}): {desc}")
    out.append("")
    out.append("## Optional")
    out.append("")
    out.append(f"- [Landing page (Markdown)]({SITE}/index.md): "
               "A clean Markdown rendering of the homepage.")
    out.append(f"- [Full documentation (single file)]({SITE}/llms-full.txt): "
               "The entire documentation set concatenated for one-fetch ingestion.")
    return "\n".join(out).rstrip("\n") + "\n"


def build_index_md_de(sections):
    """German counterpart of index.md — the Markdown edition of /de/. The
    documentation it links to is English (there is no German doc set yet)."""
    example = ""
    ov = sections.get("Overview", "")
    m = re.search(r"```bovnar\n.*?```", ov, re.S)
    if m:
        example = m.group(0)
    docs = "\n".join(
        f"- [{title}]({SITE}/docs/{slug}/) — {desc}"
        for slug, title, desc in [
            ("tutorial", "Tutorial", "Einführung in das Lesen und Schreiben von Bovnar-Dokumenten."),
            ("spec", "Spezifikation", "Die vollständige Formatspezifikation (Spec 1.2)."),
            ("units", "Einheitensystem", "264 physikalische Einheiten, SI/IEC-Präfixe und 216 Währungen."),
            ("api", "C-Lese-/Schreib-API", "Die C-Reader-/Writer-/DOM-API-Referenz."),
            ("python", "Python-Bindings", "Das reine ctypes-Python-Paket."),
            ("grammar", "EBNF-Grammatik", "Die formale Grammatik, kommentiert gegen die Referenzimplementierung."),
            ("faq", "FAQ", "Häufige Fragen zu Typen, Einheiten, Limits und der API."),
            ("conformance", "Konformität", "Die 470-Fall-Konformitätssuite und das IUT-Protokoll."),
            ("cheatsheet", "Einheiten- & Währungs-Spickzettel", "Alle Einheitensymbole, Präfixe und Währungscodes."),
            ("streaming", "Streaming & Framing", "Oktett-Streams, Frames, Multiplexing und eingebettete Dokumente."),
        ])
    parts = [
        "# Bovnar (BVNR)",
        "",
        "**Einheitensichere Serialisierung für Wissenschaft und Industrie — mit "
        "einer C99-Referenzimplementierung.**",
        "",
        "## Links",
        "",
        f"- **Website:** {SITE}",
        "- **IANA-Medientyp (`text/vnd.bovnar`):** "
        "https://www.iana.org/assignments/media-types/text/vnd.bovnar",
        "- **DOI — Dokumentation & Spezifikation (aktuell):** https://doi.org/10.5281/zenodo.21443295",
        "- **DOI — Quellcode (aktuell):** https://doi.org/10.5281/zenodo.21443008",
        "",
        "## Überblick",
        "",
        "In wissenschaftlichen und industriellen Systemen entstehen die teuren "
        "Fehler selten durch falsche Syntax — sondern durch Einheitenverwechslung: "
        "ein Wert wird in Pfund-Kraft gesendet und als Newton gelesen, Fuß als "
        "Meter. Die Zahl parst fehlerfrei; die Dimension ist falsch.",
        "",
        "Bovnar schließt diese Lücke. Jeder Wert in einem `.bvnr`-Dokument trägt "
        "seine eigene Typfamilie, Bitbreite, Zahlenbasis und **physikalische "
        "Einheit** — direkt im Byte-Strom, ohne externes Schema. Die Einheit ist "
        "kein Kommentar und keine Namenskonvention; sie ist Teil des Wertes und "
        "wird vom Parser validiert. Wird eine Messung als `m/s` annotiert und eine "
        "unpassende Inline-Einheit geschrieben, schlägt das Parsen mit "
        "`error_unit_mismatch` fehl.",
        "",
    ]
    if example:
        parts += [example, ""]
    parts += [
        "## Dokumentation",
        "",
        "*(Die Dokumentation ist derzeit auf Englisch verfügbar.)*",
        "",
        docs,
        "",
        f"Eine einzelne Datei mit der gesamten Dokumentation liegt unter "
        f"[{SITE}/llms-full.txt]({SITE}/llms-full.txt); die englische "
        f"Markdown-Startseite unter [{SITE}/index.md]({SITE}/index.md).",
        "",
    ]
    return "\n".join(parts).rstrip("\n") + "\n"


def build_llms_full(index_md):
    parts = []
    parts.append("# Bovnar (BVNR) — Full documentation")
    parts.append("")
    parts.append("This single file concatenates the Bovnar landing page and the "
                 "complete documentation set, for one-fetch ingestion by LLM tools. "
                 f"The canonical, individually-linkable Markdown sources live under "
                 f"{SITE}/doc/.")
    parts.append("")
    parts.append("<!-- ==================== Landing page ==================== -->")
    parts.append("")
    parts.append(index_md.rstrip("\n"))
    parts.append("")
    for slug, title, _desc in DOCS:
        body = read(os.path.join(ROOT, "doc", slug)).rstrip("\n")
        parts.append(f"<!-- ==================== {title} "
                     f"({SITE}/doc/{slug}) ==================== -->")
        parts.append("")
        parts.append(body)
        parts.append("")
    return "\n".join(parts).rstrip("\n") + "\n"


def main():
    check = "--check" in sys.argv[1:]
    readme = read(os.path.join(ROOT, "README.md"))
    preamble, sections, _order = split_readme_sections(readme)
    tag = tagline(preamble)

    index_md = build_index_md(preamble, sections)
    llms_txt = build_llms_txt(tag)
    llms_full = build_llms_full(index_md)
    index_md_de = build_index_md_de(sections)

    targets = {
        os.path.join(WEB, "index.md"): index_md,
        os.path.join(WEB, "llms.txt"): llms_txt,
        os.path.join(WEB, "llms-full.txt"): llms_full,
    }
    # The German Markdown edition lives under the gitignored web/de/ (like
    # web/de/index.html) — a build output, regenerated on every publish, so it is
    # written on a real run but not part of --check.
    de_target = os.path.join(WEB, "de", "index.md")

    if check:
        stale = []
        for path, text in targets.items():
            cur = read(path) if os.path.exists(path) else None
            if cur != text:
                stale.append(os.path.relpath(path, ROOT))
        if stale:
            print("gen_llms.py --check: stale/missing: " + ", ".join(stale),
                  file=sys.stderr)
            return 1
        print("gen_llms.py --check: llms.txt / index.md / llms-full.txt are current")
        return 0

    print("==> Generating LLM views (gen_llms.py)")
    for path, text in targets.items():
        write(path, text)
    os.makedirs(os.path.dirname(de_target), exist_ok=True)
    write(de_target, index_md_de)
    return 0


if __name__ == "__main__":
    sys.exit(main())
