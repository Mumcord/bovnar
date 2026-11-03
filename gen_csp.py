#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""Compute and stamp a hash-based Content-Security-Policy into the served pages.

The Bovnar site is static and self-hosted, with no server-side templating, so a
per-request nonce is impossible. Every inline <script> is therefore pinned by the
SHA-256 of its own bytes: the CSP lists 'sha256-...' for each, and the browser
recomputes and compares. No 'unsafe-inline' for scripts — an injected inline
script (or an onerror handler smuggled through the Markdown the doc drawer
renders into innerHTML) has no matching hash and does not run.

The hashes change whenever an inline script is edited, so this file is the tool
that recomputes them and cmake/csp.cmake (via --check) fails the suite when the
stamped policy no longer matches the scripts it is meant to authorise — the same
anti-drift discipline the ?v= cache stamps and the WASM artifact already have.

  ./gen_csp.py           rewrite the CSP <meta> in index.html, impressum.html, 404.html
  ./gen_csp.py --check   verify only; exit non-zero on drift

The translated editions carry ONE extra inline script (the __BVNR_I18N__ boot
blob gen_i18n.py injects). gen_i18n.py imports script_hashes()/build_meta() from
here and adds that hash when it writes web/<lang>/index.html, so the tooling for
the policy lives in one place.
"""
from __future__ import annotations

import base64
import hashlib
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
WEB = ROOT / "web"

# A <script> element, capturing its attributes and its exact byte content.
# An attribute value never contains '>' on these pages, so [^>]* is safe; the
# content is captured verbatim (no entity decoding) because that is what the
# browser hashes.
SCRIPT_RE = re.compile(r"<script\b([^>]*)>(.*?)</script>", re.S)
# The page documents its own markup inside HTML comments -- "the init calls below
# run in an inline end-of-body <script>" is one of several. A bare scan opens a
# phantom element at that mention and closes it at the next REAL </script>,
# swallowing everything between (here: the whole <style> block and the JSON-LD)
# into one hash that matches nothing the browser runs. That was harmless only by
# luck: no executable script happened to sit inside the swallowed span. One added
# there would have inherited the phantom's hash instead of getting its own, and
# the browser would have refused to run it -- with --check still passing, because
# the stamp and the scan agree on the same wrong answer. Mask comment bodies
# before scanning, preserving length so the offsets still index the real bytes.
COMMENT_RE = re.compile(r"<!--.*?-->", re.S)
# type= values the browser does NOT execute, so CSP never checks them.
DATA_TYPES = re.compile(r'type\s*=\s*["\']?(application/(ld\+)?json)', re.I)
HAS_SRC = re.compile(r'\bsrc\s*=', re.I)

CSP_META_RE = re.compile(
    r'<meta http-equiv="Content-Security-Policy" content="[^"]*">\n?')

# ── Google Analytics (GA4) ──────────────────────────────────────────────────
# One measurement ID for the whole site, defined here because the tag and the
# policy that admits it have to agree; gen_html_docs.py imports these for the
# generated doc pages so the snippet exists in exactly one place.
#
# The inline gating script is hashed like every other inline script on these
# pages -- it carries no src, so script_hashes() picks it up on its own. The
# loader it creates on the granted path still needs its ORIGIN named in
# script-src: injecting a <script> from JavaScript does not bypass CSP, and a
# hash cannot cover a remote file. What gtag.js does afterwards needs three more
# allowances that are easy to miss until the browser console fills with
# violations: it sends its beacons with fetch/sendBeacon (connect-src), falls
# back to an image pixel where that fails (img-src), and frames a sync document
# for the advertising identifier (frame-src). default-src 'none' means anything
# not named here is blocked, so an incomplete list fails closed -- the banner
# works, the visitor consents, and no data is ever collected. Not hypothetical:
# the wildcard trap described further down did exactly that.
GA_MEASUREMENT_ID = "G-S8GFD3W21V"

# The tag is CONSENT-GATED: nothing from googletagmanager.com is requested, and
# no analytics identifier is stored, until the visitor clicks Allow. That is why
# the loader is not a plain <script src> in <head> -- a <head> tag fires on load,
# which under GDPR/TTDSG is exactly what may not happen before opt-in. The
# loader element is created by CONSENT_JS instead, and only on the granted path.
#
# Consent is remembered in localStorage, NOT a cookie, so the choice itself
# needs no consent to store. Three states: absent (ask), "granted", "denied".
# Everything is wrapped in try/catch because localStorage throws outright in
# some privacy modes -- and there, failing to read the choice must leave
# analytics OFF rather than default it on.
CONSENT_KEY = "bovnar-analytics-consent"
CONSENT_JS = """
(function () {
  var KEY = '%s', ID = '%s';
  var box = document.getElementById('bvnr-consent');
  function load() {
    if (window.__bvnrAnalytics) return;
    window.__bvnrAnalytics = 1;
    window.dataLayer = window.dataLayer || [];
    window.gtag = function () { dataLayer.push(arguments); };
    var s = document.createElement('script');
    s.async = true;
    s.src = 'https://www.googletagmanager.com/gtag/js?id=' + ID;
    document.head.appendChild(s);
    gtag('js', new Date());
    gtag('config', ID);
  }
  function save(v) { try { localStorage.setItem(KEY, v); } catch (e) {} }
  var seen = null;
  try { seen = localStorage.getItem(KEY); } catch (e) {}
  if (seen === 'granted') { load(); return; }
  if (seen === 'denied' || !box) return;
  box.hidden = false;
  var yes = document.getElementById('bvnr-consent-accept');
  var no  = document.getElementById('bvnr-consent-decline');
  if (yes) yes.addEventListener('click', function () {
    save('granted'); box.hidden = true; load();
  });
  if (no) no.addEventListener('click', function () {
    save('denied'); box.hidden = true;
  });
})();
""" % (CONSENT_KEY, GA_MEASUREMENT_ID)

# Withdrawing has to be as easy as granting, so the privacy pages carry a button
# that clears the stored choice; the banner then asks again on the next load.
CONSENT_RESET_JS = """
(function () {
  var btn = document.getElementById('bvnr-consent-reset');
  var out = document.getElementById('bvnr-consent-state');
  if (!btn) return;
  btn.addEventListener('click', function () {
    try { localStorage.removeItem('%s'); } catch (e) {}
    if (out) out.hidden = false;
  });
})();
""" % CONSENT_KEY

# Self-contained so the banner looks the same on the app pages, the doc pages and
# the 404 card, none of which share a stylesheet. [hidden] needs restating: the
# attribute's default display:none loses to the display:flex below.
CONSENT_CSS = """
#bvnr-consent{position:fixed;left:0;right:0;bottom:0;z-index:2147483000;
 display:flex;flex-wrap:wrap;gap:.75rem 1.25rem;align-items:center;
 justify-content:center;padding:.85rem 1.15rem;background:#1b1b1b;color:#ededed;
 border-top:1px solid #3e3e42;box-shadow:0 -2px 14px rgba(0,0,0,.4);
 font:14px/1.5 -apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif}
#bvnr-consent[hidden]{display:none}
#bvnr-consent p{margin:0;max-width:70ch;color:#ededed}
#bvnr-consent a{color:#9cdcfe}
#bvnr-consent .bvnr-consent-actions{display:flex;gap:.5rem;flex:none}
#bvnr-consent button{font:inherit;padding:.4rem 1rem;border-radius:4px;
 cursor:pointer;border:1px solid #55555a;background:transparent;color:#ededed}
#bvnr-consent button:hover{border-color:#3794ff}
#bvnr-consent button.bvnr-consent-yes{background:#0e639c;border-color:#0e639c;
 color:#fff}
#bvnr-consent button.bvnr-consent-yes:hover{background:#1177bb}
"""

# The banner is prose, so the translated edition needs its own wording; the
# English text is what gen_i18n.py extracts and de.json answers for.
CONSENT_TEXT = {
    "en": {
        "label": "Analytics consent",
        "msg": ("This site can use Google Analytics to count visits. It stays "
                "off until you allow it — nothing is loaded from Google and "
                "no analytics identifier is stored before then."),
        "link": "Privacy notice",
        "href": "/privacy.html",
        "yes": "Allow analytics",
        "no": "Decline",
    },
    "de": {
        "label": "Einwilligung zur Reichweitenmessung",
        "msg": ("Diese Website kann Google Analytics zur Reichweitenmessung "
                "einsetzen. Das bleibt deaktiviert, bis Sie zustimmen — "
                "vorher wird nichts von Google geladen und keine Analyse-Kennung "
                "gespeichert."),
        "link": "Datenschutzerklärung",
        "href": "/datenschutz.html",
        "yes": "Analyse erlauben",
        "no": "Ablehnen",
    },
}


def consent_banner(lang: str = "en") -> str:
    """The whole gate as one fragment: styles, markup, and the gating script.

    Emitted at the end of <body> rather than in <head> -- CONSENT_JS looks the
    banner up by id as it runs, so the element has to exist by then, and a page
    that has not finished parsing has no banner to show.
    """
    t = CONSENT_TEXT[lang]
    return (
        f"<style>{CONSENT_CSS}</style>\n"
        f'<div id="bvnr-consent" role="region" aria-label="{t["label"]}" hidden>\n'
        f'  <p>{t["msg"]} <a href="{t["href"]}">{t["link"]}</a></p>\n'
        f'  <div class="bvnr-consent-actions">\n'
        f'    <button type="button" id="bvnr-consent-decline">{t["no"]}</button>\n'
        f'    <button type="button" id="bvnr-consent-accept" '
        f'class="bvnr-consent-yes">{t["yes"]}</button>\n'
        f"  </div>\n"
        f"</div>\n"
        f"<script>{CONSENT_JS}</script>")

GA_SCRIPT_SRC = "https://www.googletagmanager.com"

# A CSP host wildcard has to match at least ONE label, so '*.analytics.google.com'
# admits region1.analytics.google.com and REFUSES the bare analytics.google.com --
# which is one of the hosts GA4 posts to. Google's own CSP guide prints only the
# wildcard form, which is what makes this the trap it is: the policy reads as
# though the host is covered, the browser disagrees, and the only symptom is a
# property that stops recording. Any host GA reaches at its apex is therefore
# named twice below: once wildcarded for the regional shards, once bare.
_GA_MEASURE_IMG = "https://*.google-analytics.com https://www.googletagmanager.com"
_GA_MEASURE_CONNECT = ("https://*.google-analytics.com "
                       "https://analytics.google.com "
                       "https://*.analytics.google.com "
                       "https://*.googletagmanager.com")

# Reached only when the property has Google Signals or an Ads link enabled, which
# this one does -- Google's tag diagnostic reported stats.g.doubleclick.net and
# www.google.com blocked, and those are requests no amount of measurement config
# makes. They carry ADVERTISING identifiers, not just measurement ones, so the
# analytics section of privacy.html and datenschutz.html says so: the policy and
# the consent notice have to describe the same site. Dropping the three
# _GA_ADS_* values from the joins below -- and the matching paragraph from both
# privacy pages -- is what turning advertising features back off looks like.
#
# Google documents one more entry here, 'https://*.google.<TLD>', for the
# country-domain copies of the remarketing pixel. It is not enumerated: CSP
# permits no wildcard right of the host, so honouring it literally means listing
# every domain in google.com/supported_domains -- ~190 hosts, about 5 KB, stamped
# into every page of this site. If the diagnostic ever names a specific country
# domain as blocked, add that one host here rather than the whole list.
_GA_ADS_IMG = ("https://*.g.doubleclick.net "
               "https://www.google.com https://google.com")
_GA_ADS_CONNECT = ("https://*.g.doubleclick.net "
                   "https://www.google.com https://google.com "
                   "https://pagead2.googlesyndication.com")
# gtag frames a googletagmanager document to sync the advertising identifier.
# frame-src has no fallback of its own here beyond default-src 'none', so
# without naming it the sync is refused like anything else unlisted.
_GA_ADS_FRAME = "https://www.googletagmanager.com"

GA_IMG_SRC = _GA_MEASURE_IMG + " " + _GA_ADS_IMG
GA_CONNECT_SRC = _GA_MEASURE_CONNECT + " " + _GA_ADS_CONNECT
GA_FRAME_SRC = _GA_ADS_FRAME

# Directives that never change. default-src 'none' denies anything not named
# below; the exceptions (base-uri, form-action) do not fall back to it, so they
# are stated. frame-ancestors is intentionally absent: it is ignored in a <meta>
# policy and only warns -- real clickjacking protection needs an X-Frame-Options
# / frame-ancestors *header*, which is server config this repo cannot set.
STATIC_DIRECTIVES = [
    "default-src 'none'",
    # 'wasm-unsafe-eval' lets the emscripten parser compile its WebAssembly
    # without opening the door to string eval; 'self' covers the dynamically
    # injected marked/highlight.js and the import() of the WASM glue.
    "script-src 'self' 'wasm-unsafe-eval' " + GA_SCRIPT_SRC + " {script_hashes}",
    # 'unsafe-inline' for STYLE only, and deliberately. The page has two inline
    # <style> blocks, ~21 style="" attributes, and a <style> the highlighter
    # creates at runtime and fills with the token palette; pinning all of those
    # by hash is impractical (attributes need 'unsafe-hashes', the runtime block
    # cannot be hashed ahead of time) and style injection is a far weaker vector
    # than script. Scripts carry the strict policy; styles do not.
    "style-src 'self' 'unsafe-inline'",
    # data: for the inline SVG favicon; the google hosts for the GA pixel
    "img-src 'self' data: " + GA_IMG_SRC,
    "font-src 'self'",
    # 'self' is the doc drawer's XHR to doc/*.md; the rest is where gtag.js
    # sends its measurement beacons.
    "connect-src 'self' " + GA_CONNECT_SRC,
    # The page frames nothing of its own; this is the tag's identifier sync.
    "frame-src " + GA_FRAME_SRC,
    "manifest-src 'self'",
    "base-uri 'none'",
    "form-action 'none'",
]

# 404.html is a standalone card: one inline theme-boot script, no WASM, no
# manifest, no webfont. It gets its own reduced policy rather than the app one --
# the page has no use for 'wasm-unsafe-eval' or for 'self' anywhere it can be
# avoided, and a policy is only worth having if it stays as narrow as the page it
# guards. The one thing it does share with the app pages is the analytics tag: a
# 404 is exactly the hit worth measuring, so this policy carries the GA hosts and
# (unlike before the tag) a connect-src, which the beacon needs.
STANDALONE_DIRECTIVES = [
    "default-src 'none'",
    "script-src " + GA_SCRIPT_SRC + " {script_hashes}",
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data: " + GA_IMG_SRC,
    "font-src 'self'",
    "connect-src " + GA_CONNECT_SRC,
    "frame-src " + GA_FRAME_SRC,
    "base-uri 'none'",
    "form-action 'none'",
]

# The pages served directly, each with the directive set it is stamped from. The
# translated editions are not here: gen_i18n.py stamps those through stamp().
PAGES = [
    (WEB / "index.html", STATIC_DIRECTIVES),
    (WEB / "impressum.html", STATIC_DIRECTIVES),
    (WEB / "404.html", STANDALONE_DIRECTIVES),
    # The two privacy pages are prose + the consent controls: no WASM, no
    # manifest, nothing fetched but the analytics beacon, so the reduced policy
    # fits them the same way it fits the 404 card.
    (WEB / "privacy.html", STANDALONE_DIRECTIVES),
    (WEB / "datenschutz.html", STANDALONE_DIRECTIVES),
]


def sha256_b64(text: str) -> str:
    return base64.b64encode(hashlib.sha256(text.encode("utf-8")).digest()).decode()


def mask_comments(html: str) -> str:
    """Same-length copy of html with HTML comment bodies blanked out."""
    return COMMENT_RE.sub(lambda m: " " * (m.end() - m.start()), html)


def script_hashes(html: str) -> list[str]:
    """'sha256-...' for every executable inline <script> in the page, in order."""
    out = []
    # Scan the masked copy so a <script> written inside a comment cannot open an
    # element; hash the ORIGINAL bytes at those offsets, which is what the
    # browser sees. Masking is length-preserving, so the offsets carry over.
    for m in SCRIPT_RE.finditer(mask_comments(html)):
        attrs = m.group(1)
        if HAS_SRC.search(attrs) or DATA_TYPES.search(attrs):
            continue
        out.append("'sha256-" + sha256_b64(html[m.start(2):m.end(2)]) + "'")
    return out


def build_meta(hashes: list[str], directives: list[str] | None = None) -> str:
    directives = [
        d.format(script_hashes=" ".join(hashes)) if "{script_hashes}" in d else d
        for d in (directives or STATIC_DIRECTIVES)
    ]
    return ('<meta http-equiv="Content-Security-Policy" content="'
            + "; ".join(directives) + '">')


def policy_for(html: str, extra_hashes: list[str] | None = None,
               directives: list[str] | None = None) -> str:
    hashes = script_hashes(html)
    if extra_hashes:
        hashes += extra_hashes
    return build_meta(hashes, directives)


def _stamp(html: str, meta: str) -> str:
    """Insert or replace the CSP meta, as the first thing after <meta charset>."""
    html = CSP_META_RE.sub("", html, count=1)
    # Placed right after charset (and before the first <script>) so it governs
    # every script, including the pre-paint theme/language boot scripts.
    anchor = re.search(r'(<meta charset="[^"]*">\n)', html)
    if not anchor:
        raise SystemExit("gen_csp.py: no <meta charset> to anchor the policy to")
    at = anchor.end()
    return html[:at] + meta + "\n" + html[at:]


def stamp(html: str, extra_hashes: list[str] | None = None) -> str:
    """Return html with its CSP <meta> recomputed from its own inline scripts.

    Used by gen_i18n.py on the final translated page, whose boot script is one
    more inline <script> than the English source has -- computing the policy from
    the finished bytes means that hash is included with no bookkeeping."""
    return _stamp(html, policy_for(html, extra_hashes))


def process(path: Path, check: bool, directives: list[str] | None = None) -> int:
    html = path.read_text(encoding="utf-8")
    # hashes are of the scripts as they are now
    meta = policy_for(html, directives=directives)
    want = _stamp(html, meta)
    if check:
        if html != want:
            cur = CSP_META_RE.search(html)
            print(f"error: {path.relative_to(ROOT)} has a stale or missing CSP.",
                  file=sys.stderr)
            print("  run ./gen_csp.py to restamp it.", file=sys.stderr)
            if cur:
                print(f"  stamped: {cur.group(0).strip()[:90]}...", file=sys.stderr)
            return 1
        print(f"{path.relative_to(ROOT)}: CSP matches "
              f"{len(script_hashes(html))} inline script(s)")
        return 0
    if html != want:
        path.write_text(want, encoding="utf-8")
        print(f"stamped {path.relative_to(ROOT)} "
              f"({len(script_hashes(want))} script hashes)")
    else:
        print(f"{path.relative_to(ROOT)}: already current")
    return 0


def main() -> int:
    check = "--check" in sys.argv[1:]
    rc = 0
    for page, directives in PAGES:
        if page.exists():
            rc |= process(page, check, directives)
    return rc


if __name__ == "__main__":
    sys.exit(main())
