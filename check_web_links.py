#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_web_links.py — every internal link in the published site resolves.

The gap this closes: bvnr_html_docs verifies the generated pages are FRESH, not
that their links go anywhere. gen_html_docs rewrites an inter-doc Markdown link
to its HTML page via SRC_TO_SLUG, and a document with no HTML page fell through
with the relative href the Markdown wrote — so "07_bovnar_unit_ambiguities.md" on
/docs/units/ resolved to /docs/units/07_bovnar_unit_ambiguities.md and 404'd, on three
pages, while the file served fine one directory away at /doc/. Nothing in the
suite noticed, because every page was perfectly up to date with its source.

What it checks, over web/**/*.{html,md}, web/sitemap.xml and the manifest:

  * the target exists under the web root (following web/doc -> ../doc);
  * it will actually REACH the server — that is, git tracks it, or a generator
    produces it (the PDFs into build/doc/pdf/, a language edition into web/<lang>/).
    Resolving against the disk instead passed here and failed in CI, because
    web/de/ is git-ignored: the answer has to be the same from any checkout;
  * publish_web.sh will actually upload it — a file that exists locally but is
    deliberately NOT uploaded is still a 404 on the live site, which is the one
    class of dead link that looks fine from a checkout;
  * a "#fragment" resolves to an id on the target page (HTML), or to a heading
    slug (Markdown), so a renamed section is caught as well as a moved file.

"Will be uploaded" is two different questions in two trees. Under web/ the
publisher subtracts: everything ships except its EXCLUDES. Under doc/ it adds:
nothing ships except its WEBDOCS, because doc/ holds working documents beside
published ones and the default there has to be private. Both lists are PARSED
from publish_web.sh rather than restated here: two copies would drift, and the
drift would silently switch this check off for whatever moved.

External hosts are reported but never fetched — a test must not depend on the
network.

Usage:
    python3 check_web_links.py            # report and exit 1 on any dead link
    python3 check_web_links.py --verbose  # also list what was checked
"""
import os
import re
import subprocess
import sys
import unicodedata

ROOT = os.path.dirname(os.path.abspath(__file__))
WEB = os.path.join(ROOT, "web")
SITE_HOSTS = ("www.bovnar.io", "bovnar.io")

# Paths publish_web.sh CREATES at stage time instead of taking from web/: the
# PDFs are generated into build/doc/pdf/ (git-ignored) and copied into the
# staging tree. They are live URLs, so the links to them are not dead — but they
# are also not in a checkout, so they have to be resolved against the build
# directory rather than the web root. When it has not been built the links are
# reported as unverifiable rather than passed silently: publish_web.sh already
# refuses to ship without them ("the download links will 404"), so the existence
# question is gated there, and the question HERE is whether a page names a file
# the generator actually produces.
STAGE_OVERLAY = {"doc/pdf/": os.path.join(ROOT, "build", "doc", "pdf")}

# The question this check has to answer is "will the server have this file?", and
# the answer must not depend on whose machine is asking. The site is committed
# files PLUS generated ones, and a generated file that happens to be sitting in
# the working tree is invisible to a fresh clone -- so resolving against the disk
# alone passes locally and fails in CI, which is the worst behaviour a gate can
# have. It did exactly that: web/de/ is git-ignored (gen_i18n.py writes it, and
# publish_web.sh runs that before staging), so the landing page's link to /de/
# resolved here and 404'd on a clean checkout.
#
# Everything below therefore resolves against `git ls-files` and an explicit list
# of generated prefixes, never against mere presence on disk.
#
# A language edition is legitimate only when its translation table exists -- the
# same condition CMake uses to register bvnr_i18n_<lang> -- so a link to /fr/ is
# still dead until web/i18n/fr.json is added.
GENERATED_LANG_SOURCES = os.path.join(WEB, "i18n")

# Attributes worth resolving. `href` covers links and stylesheets, `src` images
# and scripts; both can carry a "?v=" cache stamp that is not part of the path.
_ATTR_RE = re.compile(r'(?:href|src)\s*=\s*"([^"]*)"', re.I)
_MD_LINK_RE = re.compile(r'\[[^\]]*\]\(\s*<?([^)\s>]+)>?\s*(?:"[^"]*")?\)')
_ID_RE = re.compile(r'\bid\s*=\s*"([^"]+)"')
_NAME_RE = re.compile(r'<a\b[^>]*\bname\s*=\s*"([^"]+)"', re.I)
_LOC_RE = re.compile(r"<loc>\s*([^<\s]+)\s*</loc>", re.I)
_HEADING_RE = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$", re.M)
_FENCE_RE = re.compile(r"^(?:```|~~~)")


def _read_array(text, name):
    """Parse a bash array literal out of publish_web.sh."""
    m = re.search(r"^%s=\((.*?)^\)" % name, text, re.M | re.S)
    if not m:
        raise SystemExit(
            "check_web_links.py: no %s=( ... ) block in publish_web.sh — the "
            "publisher changed shape and this check would silently pass links "
            "to unpublished files" % name)
    out = []
    for line in m.group(1).splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        mm = re.match(r'"([^"]+)"', line)
        if mm:
            out.append(mm.group(1).strip("/"))
    if not out:
        raise SystemExit("check_web_links.py: %s block parsed empty" % name)
    return out


def read_publish_rules():
    """What publish_web.sh will and will not upload, parsed from the script.

    Two lists, because the publisher treats two trees differently. Under web/ it
    subtracts: everything ships except EXCLUDES. Under doc/ it adds: nothing
    ships except WEBDOCS, so that a working document dropped into doc/ is
    private by default instead of published by default.

    Both are parsed rather than restated here, for the reason in the module
    docstring: a second copy drifts, and the drift switches this check off for
    whatever moved.
    """
    with open(os.path.join(ROOT, "publish_web.sh"), encoding="utf-8") as f:
        text = f.read()
    return _read_array(text, "EXCLUDES"), _read_array(text, "WEBDOCS")


def is_excluded(rel, rules):
    """True when the live site will NOT have `rel`."""
    excludes, webdocs = rules
    rel = rel.strip("/")
    # doc/ is allowlisted. doc/pdf/ is the exception: it is not in WEBDOCS
    # because it is not in the repo -- gen_doc_pdfs.py writes it into build/ and
    # the publisher stages it from there (see STAGE_OVERLAY above).
    if rel == "doc" or rel.startswith("doc/"):
        if rel.startswith("doc/pdf/") or rel == "doc/pdf":
            return False
        inner = rel[len("doc/"):] if rel != "doc" else ""
        return not any(inner == w or inner.startswith(w + "/")
                       for w in webdocs)
    for e in excludes:
        if rel == e or rel.startswith(e + "/"):
            return True
    return False


def slugify(text):
    """GitHub's heading-anchor algorithm, as the raw Markdown links assume."""
    text = re.sub(r"`([^`]*)`", r"\1", text)               # inline code
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", text)   # links -> their text
    # Emphasis markers only. NOT "_": GitHub keeps the underscore as a word
    # character, which is the whole reason "#7-the-no_unit-keyword" resolves —
    # stripping it here reported 28 live anchors as broken.
    text = re.sub(r"[*~]", "", text)
    text = unicodedata.normalize("NFKD", text)
    out = []
    for ch in text.lower():
        if ch.isalnum() or ch in "-_":
            out.append(ch)
        elif ch in " \t":
            out.append("-")
        # everything else (punctuation, emoji, combining marks) is dropped
    return "".join(out)


def md_anchors(text):
    """Heading slugs of a Markdown document, deduplicated GitHub-style."""
    body = []
    fenced = False
    for line in text.splitlines():
        if _FENCE_RE.match(line.strip()):
            fenced = not fenced
            continue
        body.append("" if fenced else line)
    seen, out = {}, set()
    for _hashes, title in _HEADING_RE.findall("\n".join(body)):
        s = slugify(title)
        if not s:
            continue
        n = seen.get(s, 0)
        out.add(s if n == 0 else "%s-%d" % (s, n))
        seen[s] = n + 1
    return out


def html_anchors(text):
    return set(_ID_RE.findall(text)) | set(_NAME_RE.findall(text))


_anchor_cache = {}


def anchors_of(abspath):
    if abspath in _anchor_cache:
        return _anchor_cache[abspath]
    try:
        with open(abspath, encoding="utf-8", errors="replace") as f:
            text = f.read()
    except OSError:
        _anchor_cache[abspath] = None
        return None
    if abspath.endswith((".md", ".ebnf")):
        a = md_anchors(text)
    elif abspath.endswith((".html", ".htm")):
        a = html_anchors(text)
    else:
        a = None                       # not a document we can check inside
    _anchor_cache[abspath] = a
    return a


def sources():
    """(abspath, relpath, kind) for every file whose links we resolve.

    followlinks=True so web/doc -> ../doc is descended: the raw Markdown is a
    PUBLISHED surface (every page advertises "also available as Markdown" and
    links to /doc/<file>), so its cross-document links are live URLs too and rot
    the same way. A realpath guard keeps a symlink cycle from looping.
    """
    seen_dirs = set()
    for dirpath, dirnames, filenames in os.walk(WEB, followlinks=True):
        real = os.path.realpath(dirpath)
        if real in seen_dirs:
            dirnames[:] = []
            continue
        seen_dirs.add(real)
        dirnames[:] = [d for d in dirnames if d not in (".git", "fonts")]
        for fn in sorted(filenames):
            ap = os.path.join(dirpath, fn)
            rel = os.path.relpath(ap, WEB).replace(os.sep, "/")
            if fn.endswith((".html", ".htm")):
                yield ap, rel, "html"
            elif fn.endswith(".md"):
                yield ap, rel, "md"
            elif fn == "sitemap.xml":
                yield ap, rel, "sitemap"
            elif fn.endswith(".webmanifest"):
                yield ap, rel, "manifest"


def extract(abspath, kind):
    with open(abspath, encoding="utf-8", errors="replace") as f:
        text = f.read()
    if kind == "html":
        return _ATTR_RE.findall(text)
    if kind == "md":
        # a Markdown page may embed raw HTML too
        return _MD_LINK_RE.findall(text) + _ATTR_RE.findall(text)
    if kind == "sitemap":
        return _LOC_RE.findall(text)
    if kind == "manifest":
        return re.findall(r'"src"\s*:\s*"([^"]+)"', text)
    return []


def classify(target):
    """(kind, path, fragment) — kind is 'site', 'rel', 'self', 'skip', 'external'."""
    t = target.strip()
    if not t:
        return "skip", "", ""
    low = t.lower()
    if low.startswith(("mailto:", "tel:", "data:", "javascript:")):
        return "skip", "", ""
    frag = ""
    if "#" in t:
        t, frag = t.split("#", 1)
    t = t.split("?", 1)[0]                       # drop the cache stamp
    if not t:
        return "self", "", frag
    for scheme in ("http://", "https://", "//"):
        if low.startswith(scheme):
            rest = t[len(scheme):] if not scheme == "//" else t[2:]
            host = rest.split("/", 1)[0]
            if host in SITE_HOSTS:
                path = rest[len(host):] or "/"
                return "site", path, frag
            return "external", t, frag
    if t.startswith("/"):
        return "site", t, frag
    return "rel", t, frag


def resolve(rel_dir, kind, path):
    """Site-relative path (no leading slash) for a link, or None if it escapes."""
    if kind == "site":
        p = path.lstrip("/")
    else:
        p = os.path.normpath(os.path.join(rel_dir, path)).replace(os.sep, "/")
    if p in ("", "."):
        p = ""
    if p.startswith(".."):
        return None
    return p


def overlay_lookup(sitepath):
    """('ok'|'missing'|'unbuilt', detail) for a stage-time path, or (None, None)."""
    for prefix, localdir in STAGE_OVERLAY.items():
        if not sitepath.startswith(prefix):
            continue
        name = sitepath[len(prefix):]
        if not os.path.isdir(localdir):
            return "unbuilt", localdir
        if name and os.path.isfile(os.path.join(localdir, name)):
            return "ok", None
        return "missing", localdir
    return None, None


def tracked_paths():
    """Every repo-relative path git tracks, or None outside a checkout.

    Compared against the REALPATH of a target, so web/doc -> ../doc resolves to
    the tracked doc/<file> without this needing to know about the symlink.
    """
    try:
        r = subprocess.run(["git", "-C", ROOT, "ls-files", "-z"],
                           stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                           check=True)
    except (OSError, subprocess.CalledProcessError):
        return None                      # a tarball, not a checkout
    return {n for n in r.stdout.decode("utf-8", "replace").split("\0") if n}


def is_tracked(abspath, tracked):
    if tracked is None:
        return True                      # cannot tell; fall back to existence
    rp = os.path.relpath(os.path.realpath(abspath), ROOT).replace(os.sep, "/")
    return rp in tracked


def generated_langs():
    """Language prefixes gen_i18n.py can produce, from the translation tables."""
    out = set()
    if os.path.isdir(GENERATED_LANG_SOURCES):
        for fn in os.listdir(GENERATED_LANG_SOURCES):
            if fn.endswith(".json"):
                out.add(os.path.splitext(fn)[0] + "/")
    return out


def target_file(sitepath):
    """(abspath, servable_relpath) a URL maps to, or (None, sitepath)."""
    ap = os.path.join(WEB, sitepath) if sitepath else WEB
    if os.path.isdir(ap):
        idx = os.path.join(ap, "index.html")
        if os.path.isfile(idx):
            return idx, (sitepath.rstrip("/") + "/index.html").lstrip("/")
        return None, sitepath
    if os.path.isfile(ap):
        return ap, sitepath
    return None, sitepath


def main():
    verbose = "--verbose" in sys.argv
    if not os.path.isdir(WEB):
        raise SystemExit("check_web_links.py: no web/ directory at %s" % WEB)
    rules = read_publish_rules()
    tracked = tracked_paths()
    langs = generated_langs()
    dead, unpublished, badfrag, untracked = [], [], [], []
    checked = ext = staged = unbuilt = gen = offsite = 0
    for ap, rel, kind in sources():
        # A source publish_web.sh excludes has no links on the live site, because
        # it is not on the live site. Reading it anyway turned its own table of
        # contents into seven "links to a file that is never uploaded" — every
        # one of them a fragment pointing at the document it was written in.
        if is_excluded(rel, rules):
            offsite += 1
            continue
        rel_dir = os.path.dirname(rel)
        for raw in extract(ap, kind):
            lkind, path, frag = classify(raw)
            if lkind == "skip":
                continue
            if lkind == "external":
                ext += 1
                continue
            if lkind == "self":
                sitepath = rel
            else:
                sitepath = resolve(rel_dir, lkind, path)
                if sitepath is None:
                    dead.append((rel, raw, "escapes the web root"))
                    continue
            checked += 1
            ov, detail = overlay_lookup(sitepath)
            if ov is not None:
                # staged at publish time from a build directory, not from web/
                if ov == "ok":
                    staged += 1
                elif ov == "unbuilt":
                    unbuilt += 1
                else:
                    dead.append((rel, raw,
                                 "%s is not produced into %s" % (sitepath, detail)))
                continue
            # A language edition is generated by gen_i18n.py into a git-ignored
            # directory, so it is legitimately absent from a clean checkout. Its
            # translation table has to exist, though — that is what separates
            # /de/ from a link to a language nobody has translated.
            lang_pref = next((L for L in langs if sitepath.startswith(L)), None)
            tf, served = target_file(sitepath)
            if tf is None and lang_pref:
                gen += 1
                continue
            if tf is None:
                dead.append((rel, raw, "no such file: %s" % (sitepath or "/")))
                continue
            if is_excluded(served, rules) or is_excluded(sitepath, rules):
                unpublished.append((rel, raw, served))
                continue
            # Present on disk, but will a clone have it? Only "committed" and
            # "generated" reach the server; anything else is a link that works
            # here and 404s everywhere else.
            if not is_tracked(tf, tracked):
                if lang_pref:
                    gen += 1
                    continue
                untracked.append((rel, raw, served))
                continue
            if frag:
                anc = anchors_of(tf)
                if anc is not None and frag not in anc:
                    badfrag.append((rel, raw, served, frag))
            if verbose:
                print("  ok   %-42s -> %s%s" % (raw[:42], served,
                                                "#" + frag if frag else ""))
    print("checked %d internal link(s) across web/ (%d external, not fetched)"
          % (checked, ext))
    if offsite:
        print("  %d source file(s) skipped — publish_web.sh does not upload them"
              % offsite)
    if staged:
        print("  %d resolved against a publish-time staging directory" % staged)
    if unbuilt:
        print("  %d NOT VERIFIED — their staging directory has not been built "
              "(run gen_doc_pdfs.py); publish_web.sh gates their existence"
              % unbuilt)
    if gen:
        print("  %d into a generated language edition (%s) — produced by "
              "gen_i18n.py, which publish_web.sh runs before staging"
              % (gen, ", ".join(sorted(L.rstrip('/') for L in langs)) or "none"))
    if tracked is None:
        print("  NOTE: not a git checkout, so 'committed' could not be checked; "
              "targets were resolved against the working tree only")
    fail = False
    if dead:
        fail = True
        print("\n%d DEAD link(s) — the target does not exist:" % len(dead))
        for src, raw, why in dead:
            print("  %-34s %-40s %s" % (src, raw[:40], why))
    if unpublished:
        fail = True
        print("\n%d link(s) to a file publish_web.sh does NOT upload — these exist"
              "\nin the checkout and 404 on the live site:" % len(unpublished))
        for src, raw, served in unpublished:
            print("  %-34s %-40s (excluded: %s)" % (src, raw[:40], served))
    if untracked:
        fail = True
        print("\n%d link(s) to a file that is NOT COMMITTED and not generated —"
              "\nthese resolve in this working tree and 404 from a clean clone:"
              % len(untracked))
        for src, raw, served in untracked:
            print("  %-34s %-40s (untracked: %s)" % (src, raw[:40], served))
    if badfrag:
        fail = True
        print("\n%d link(s) whose #fragment is not on the target page:"
              % len(badfrag))
        for src, raw, served, frag in badfrag:
            print("  %-34s %-40s no '#%s' in %s" % (src, raw[:40], frag, served))
    if fail:
        return 1
    print("every internal link resolves, and none points at an excluded file")
    return 0


if __name__ == "__main__":
    sys.exit(main())
