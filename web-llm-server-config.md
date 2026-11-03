# Web-server configuration for LLM visibility

Most of the "make the site visible to LLMs" work ships with the static site
(`web/`): `robots.txt` with a `Content-Signal:` directive, `/llms.txt`,
`/llms-full.txt`, `/index.md` and the per-doc `/doc/*.md` Markdown, the
`<link rel="alternate" type="text/markdown">` in the page head, and an
off-screen Markdown-URL hint. Those deploy via `publish_web.sh` and need nothing
on the server.

Three techniques are **protocol-level** and therefore live in the web-server
config (they cannot ride the `rsync` of `web/` into the doc root). `www.bovnar.io`
runs **nginx** — the tailored block for the live config is below; an Apache
equivalent follows as a portable alternative. All three are additive and safe:
they only add response headers and an `Accept`-based redirect for the home page.

0. **HTTP/2** (`http2 on;`) — ALPN advertised only `http/1.1` before, so the page
   and its subresources shared one connection pool with the usual six-connection
   cap. Clients that do not speak h2 still negotiate 1.1.
1. **`text/markdown` MIME type** for `.md` files (RFC 7763) — so `/index.md` and
   `/doc/*.md` are served as Markdown, not `text/plain` or a download. `.ebnf`
   (the single non-Markdown document) gets `text/plain` for the same reason.
2. **`Link: …; rel="alternate"; type="text/markdown"`** response header on the
   home page, plus **`Vary: Accept`** — protocol-level discovery of the Markdown
   edition before the HTML body is read.
3. **`Accept: text/markdown` content negotiation** — when a client (Claude Code,
   Cursor, …) prefers Markdown, serve `/index.md` for `/`.

Validate afterwards with <https://acceptmarkdown.com> and
<https://isitagentready.com>.

---

## nginx (the live config)

`www.bovnar.io` runs nginx. Only the canonical-host `server{}` block serves the
site, so that is the only block that changes; the apex/`.com`/`.net`/`.org`
redirect blocks stay as they are. Two nginx specifics are handled below:

- **A `map` must live in the `http{}` context** — so it goes at the top of the
  site config file, *above* the `server` blocks (a site file under
  `sites-enabled/` is included inside `http{}`).
- **`add_header` replaces inherited headers at a new level.** The moment
  `location = /` declares its own `add_header`, the server-level headers stop
  inheriting *there* — so every such location `include`s
  `snippets/bovnar-headers.conf` (documented below) instead of restating them
  one at a time and losing one of them the next time the set grows.

```nginx
# AI/LLM content negotiation: does the client prefer Markdown? (technique 3)
# Browsers send "text/html,…" with no text/markdown token and never match, so
# they are unaffected. AI tools (Claude Code, Cursor, …) send
# "Accept: text/markdown[, text/html;q=…]" and match. Pragmatic token test, not a
# full q-value parse — fine because such clients list Markdown as their primary
# type. MUST be in the http{} context, hence above the server blocks.
map $http_accept $bvnr_wants_md {
    default            0;
    "~*text/markdown"  1;
}

# Cache policy. Every versioned asset on the page is requested as
# <path>?v=<content-hash> (wasm/build_wasm.sh stamps them from the file's own
# bytes), so a fingerprinted URL can safely be cached forever -- that is the
# entire point of the fingerprint, and the server was not telling anyone. A
# request for the same path WITHOUT ?v= is not necessarily the same bytes
# forever, so it gets a short TTL and revalidates instead. HTML is left alone:
# it carries an ETag and must reflect a publish immediately.
#
# Only a 12-hex-char stamp qualifies, because that is the shape build_wasm.sh
# emits and cmake/cache_stamps.cmake verifies. A hand-written stamp gets the
# short TTL rather than a year: fonts/fonts.css shipped as ?v=20260724 for
# months, correct only while someone remembered to bump it.
map $arg_v $bvnr_cache_ctl {
    default            "public, max-age=3600";
    "~^[0-9a-f]{12}$"  "public, max-age=31536000, immutable";
}

# Canonical host (only the additions inside are new; TLS/redirect blocks unchanged).
server {
    listen 443 ssl;
    listen [::]:443 ssl;
    server_name www.bovnar.io;

    root /var/www/html;
    index index.html index.htm index.nginx-debian.html;

    # ── Compression ──────────────────────────────────────────────────────────
    # nginx compresses text/html and nothing else unless told otherwise, so the
    # site shipped 791 KB per cold visit that did not need to move: llms-full.txt
    # 560->185 KB, bovnar_wasm_core.js 252->92 KB, highlight.js 122->41 KB, the
    # spec .md 115->38 KB. Measured against the live server; HTML was already
    # gzipped (376->129 KB) because that one is nginx's built-in default.
    # text/html is deliberately absent below: listing it is a duplicate and nginx
    # warns.
    #
    # gzip_vary is not cosmetic. The live server already gzips HTML while sending
    # only "Vary: Accept" (from the home page's add_header), never
    # "Vary: Accept-Encoding" -- so a shared cache may hand the compressed body
    # to a client that never asked for it. nginx appends Accept-Encoding to Vary
    # itself once this is on.
    gzip              on;
    gzip_vary         on;
    gzip_comp_level   6;
    gzip_min_length   256;
    gzip_proxied      any;
    gzip_types        text/plain text/css text/xml text/markdown
                      application/javascript application/json
                      application/manifest+json application/xml
                      application/rss+xml image/svg+xml;

    # Branded 404 page (web/404.html). The =404 in try_files below triggers it.
    error_page 404 /404.html;
    location = /404.html { internal; }

    # Home page: advertise + serve the Markdown edition to AI tools.
    # Declares add_header, so it must repeat the server-level HSTS (see above).
    location = / {
        # 3. Content negotiation: 302 Markdown-preferring agents to /index.md.
        #    302 (not an internal rewrite) keeps HTML and Markdown at distinct
        #    URLs so caches key correctly on "Vary: Accept".
        if ($bvnr_wants_md) { return 302 /index.md; }

        # 2. Protocol-level discovery of the Markdown alternate + the Vary.
        add_header Link '</index.md>; rel="alternate"; type="text/markdown"' always;
        add_header Vary 'Accept' always;

        # 3. Revalidate. See "Cache-Control" below -- a document with no
        #    Cache-Control is not uncached, it is cached by guesswork.
        add_header Cache-Control "no-cache" always;

        include snippets/bovnar-headers.conf;

        try_files /index.html =404;
    }

    # German home page: the same, pointing at /de/index.md.
    location = /de/ {
        if ($bvnr_wants_md) { return 302 /de/index.md; }
        add_header Link '</de/index.md>; rel="alternate"; type="text/markdown"' always;
        add_header Vary 'Accept' always;
        add_header Cache-Control "no-cache" always;
        include snippets/bovnar-headers.conf;
        try_files /de/index.html =404;
    }

    # 1. Serve .md as Markdown (RFC 7763) with a UTF-8 charset. Empty types{}
    #    drops any inherited extension map so default_type wins; charset_types
    #    adds text/markdown to the set that gets "; charset=…" appended.
    location ~ \.md$ {
        types         { }
        default_type  text/markdown;
        charset       utf-8;
        charset_types text/markdown;
        add_header Cache-Control "no-cache" always;
        include snippets/bovnar-headers.conf;
        try_files $uri =404;
    }

    # The one non-Markdown document. Nothing maps .ebnf, so it fell through to
    # nginx's default_type (application/octet-stream) and the browser offered a
    # download instead of showing the grammar -- while its nine Markdown siblings
    # on /docs/ displayed inline, and /docs/grammar/ advertises it as
    # <link rel="alternate" type="text/plain">, which the server contradicted.
    location ~ \.ebnf$ {
        types         { }
        default_type  text/plain;
        charset       utf-8;
        charset_types text/plain;
        add_header Cache-Control "no-cache" always;
        include snippets/bovnar-headers.conf;
        try_files $uri =404;
    }

    # site.webmanifest: nginx's mime.types has no .webmanifest entry, so it went
    # out as application/octet-stream. Browsers are lenient about it, but the
    # spec type is application/manifest+json -- and naming it also brings the
    # file under gzip_types above.
    location ~ \.webmanifest$ {
        types         { }
        default_type  application/manifest+json;
        charset       utf-8;
        try_files $uri =404;
    }

    # Static assets only -- see the $bvnr_cache_ctl map. Declaring add_header
    # here stops this location inheriting the server-level headers, so it
    # includes them (the same trap as the home-page locations above). pdf and zip
    # are in the list because the documentation ships as both.
    location ~* \.(js|css|woff2|jpe?g|png|svg|ico|pdf|zip)$ {
        # A renamed document's PDF, same map and same reason as the .md and
        # .ebnf locations: 301 before try_files can answer 404.
        if ($bvnr_doc_moved) { return 301 $bvnr_doc_moved; }

        add_header Cache-Control "$bvnr_cache_ctl" always;
        include snippets/bovnar-headers.conf;
        try_files $uri =404;
    }

    location / {
        # A rendered page that has been renamed: 301 before try_files can answer
        # (see the $bvnr_page_moved map). This lives here rather than in a
        # location /docs/ of its own: a prefix block would take over every
        # /docs/ request and have to restate this one's headers and try_files,
        # and $bvnr_page_moved is empty for everything else, so the test costs
        # one variable lookup and changes nothing it does not name.
        if ($bvnr_page_moved) { return 301 $bvnr_page_moved; }

        add_header Cache-Control "no-cache" always;
        include snippets/bovnar-headers.conf;
        try_files $uri $uri/ =404;
    }

    # … existing TLS block, ssl_stapling, and server-level HSTS unchanged …
}
```

Reload with `nginx -t && systemctl reload nginx`.

Location precedence works out: `= /` (exact) handles the home page, `~ \.md$`
(regex) handles `.md` files — including the `/index.md` redirect target — and the
prefix `location /` handles everything else unchanged. `/llms.txt` and
`/llms-full.txt` stay `text/plain` (they are `.txt`); to serve them as Markdown
too, widen the regex to `~ (\.md$|^/llms(-full)?\.txt$)`.

---

## Apache

Requires `mod_headers`, `mod_mime`, and `mod_rewrite` (all standard). Put this in
the site's `<VirtualHost>` / `<Directory>` block, or in a `.htaccess` at the doc
root **only if `AllowOverride` already permits `FileInfo Indexes`** — otherwise
Apache returns 500. Prefer the vhost.

```apache
# 1. Serve .md as Markdown (RFC 7763), and the one non-Markdown document as
#    plain text -- unmapped, .ebnf is offered as a download rather than shown.
AddType text/markdown .md
AddType text/plain .ebnf
AddCharset UTF-8 .md .ebnf

# 2. Advertise the Markdown alternate + vary on the home page.
<If "%{REQUEST_URI} == '/'">
    Header set Link "</index.md>; rel=\"alternate\"; type=\"text/markdown\""
    Header append Vary "Accept"
</If>

# 3. Content negotiation for the home page: serve /index.md to Markdown-preferring
#    clients (302 so caches key on Vary: Accept rather than pinning the HTML).
RewriteEngine On
RewriteCond %{HTTP_ACCEPT} text/markdown [NC]
RewriteRule ^$ /index.md [R=302,L]
```

---

## Notes

- The `302` (not an internal rewrite) keeps HTML and Markdown at distinct URLs, so
  intermediary caches key correctly on `Vary: Accept`.
- `robots.txt` already carries `Content-Signal: search=yes, ai-input=yes,
  ai-train=yes`. Change `ai-train=yes` to `ai-train=no` if you do **not** want the
  content used for model training; the other two signals keep search and live-AI
  context enabled.
- Nothing here changes the human-facing site: browsers send
  `Accept: text/html,…` and continue to receive `index.html`.
- The `$bvnr_cache_ctl` map only ever sees a stamp the *page* asked for, so an
  asset reached from somewhere else never earned the immutable year: the woff2
  files are named only inside `fonts/fonts.css`, which stamped none of them, and
  ~440 kB of fonts that never change revalidated hourly on the critical path.
  `./gen_font_stamps.py` now derives those stamps (and the pages' stamp for
  `fonts.css` itself) from the bytes, gated by the `bvnr_font_stamps` test — the
  same contract `cmake/cache_stamps.cmake` gives the WASM artifacts.
- **Documents send `Cache-Control: no-cache`.** Only the stamped-asset location
  set `Cache-Control` for a long time; HTML, `.md` and `.ebnf` sent none at all,
  which is not the same as "do not cache". RFC 9111 §4.2.2 lets a browser invent
  a freshness lifetime from `Last-Modified` — commonly a tenth of the file's age
  — and these documents sit unchanged for weeks between publishes, so the
  invented lifetime ran to days. A returning visitor kept a months-old landing
  page, and the inline documentation viewer kept re-rendering months-old
  Markdown, without issuing a single request. `no-cache` still lets the browser
  store the copy; it only forbids reusing it without asking, and the `ETag`
  makes the answer a 304. The stamped assets are untouched — that is the whole
  point of stamping them.

## Optional: consolidate the raw `.md` to the HTML docs for search

The same documentation is served three ways — the HTML pages `/docs/<slug>/` (the
canonical, indexable form), the raw Markdown `/doc/*.md` (for LLM tools), and the
PDFs. To keep search engines from treating the raw `.md` as competing pages while
LLM tools keep fetching it, advertise the HTML page as canonical via a response
header on the `.md`. This is a search-only signal; it does not affect what LLM
crawlers receive.

The map holds the **complete** `Link` header value (RFC 8288: the URI in angle
brackets) for each doc `.md`, and an empty string for everything else. Add it to
the `http{}` context beside the other `map`:

```nginx
map $uri $bvnr_canon_hdr {
    default                              "";
    "/doc/01_bovnar_tutorial.md"         "<https://www.bovnar.io/docs/tutorial/>; rel=\"canonical\"";
    "/doc/02_bovnar_faq.md"              "<https://www.bovnar.io/docs/faq/>; rel=\"canonical\"";
    "/doc/03_bovnar_spec.md"             "<https://www.bovnar.io/docs/spec/>; rel=\"canonical\"";
    "/doc/04_bovnar_unit_cheatsheet.md"  "<https://www.bovnar.io/docs/cheatsheet/>; rel=\"canonical\"";
    "/doc/05_bovnar_unit_system.md"      "<https://www.bovnar.io/docs/units/>; rel=\"canonical\"";
    "/doc/06_bovnar_unit_policy.md"      "<https://www.bovnar.io/docs/policy/>; rel=\"canonical\"";
    "/doc/07_bovnar_unit_ambiguities.md" "<https://www.bovnar.io/docs/ambiguities/>; rel=\"canonical\"";
    "/doc/08_bovnar_readwrite_api.md"    "<https://www.bovnar.io/docs/api/>; rel=\"canonical\"";
    "/doc/09_bovnar_python_bindings.md"  "<https://www.bovnar.io/docs/python/>; rel=\"canonical\"";
    "/doc/10_bovnar_streaming.md"        "<https://www.bovnar.io/docs/streaming/>; rel=\"canonical\"";
    "/doc/11_bovnar_unit_profiles.md"    "<https://www.bovnar.io/docs/profiles/>; rel=\"canonical\"";
    "/doc/12_bovnar.ebnf"                "<https://www.bovnar.io/docs/grammar/>; rel=\"canonical\"";
    "/doc/13_bovnar_conformance.md"      "<https://www.bovnar.io/docs/conformance/>; rel=\"canonical\"";
    "/doc/14_bovnar_rust_bindings.md"    "<https://www.bovnar.io/docs/rust/>; rel=\"canonical\"";
}
```

The map lists all thirteen documents, in document order. It listed nine for a
while — the unit policy, the ambiguity reference and the UCUM profile joined the
numbered set after this map was written, and the grammar was left out on the
argument that `/docs/grammar/` was too niche to have anything to compete with.
The argument does not survive the file being served at two URLs either way, and
a map that has to be remembered when a document is added is a map that will be
out of date again.

Then, inside the existing `location ~ \.md$`, add the header — nginx omits an
`add_header` whose value evaluates to the empty string, so `/index.md` and
`/de/index.md` get no canonical header, while the doc `.md` do:

```nginx
    # (add to the existing .md location; also include the shared header snippet
    #  there, since declaring add_header stops the location inheriting it)
    add_header Link "$bvnr_canon_hdr" always;
    include snippets/bovnar-headers.conf;
```

`/doc/12_bovnar.ebnf` is not matched by the `.md` location — it has its own, for
the MIME type (above), or it downloads instead of displaying. The same
`add_header Link "$bvnr_canon_hdr"` line goes there; one map serves both.

## Renamed documents: 301 the old paths

The documentation set has been renumbered twice, most recently when every
document gained a two-digit prefix. A rename is invisible to everyone already
holding the old URL, and those URLs are not ours to recall — the **IANA
registration for `text/vnd.bovnar` cites the specification, the unit system and
the grammar by their pre-renumbering paths**, as do released tarballs and any
third-party link. Each rename turned all of them into 404s.

A `map` of old path to current path, plus one `if` in each document location,
turns them into 301s. `if` is safe in this shape: `return` is one of the two
directives nginx documents as reliable inside `if` in a location.

A **rendered page** can be renamed too, and one now has: `/docs/ucum/` became
`/docs/profiles/` when the UCUM profile document grew to cover five vocabularies
and was retitled. That path was in the sitemap and is therefore indexed, so it
gets the same treatment through a second map, `$bvnr_page_moved`, applied in the
`/docs/` location. The two maps are kept apart because they answer different
things — one the Markdown source, the other the HTML rendering — and a single map
keyed on both would 301 a `.md` request to an HTML page.

```nginx
map $uri $bvnr_doc_moved {
    default                             "";
    "/doc/0_bovnar_tutorial.md"         "/doc/01_bovnar_tutorial.md";
    "/doc/6_bovnar_faq.md"              "/doc/02_bovnar_faq.md";
    "/doc/1_bovnar_spec.md"             "/doc/03_bovnar_spec.md";
    "/doc/8_unit_cheatsheet.md"         "/doc/04_bovnar_unit_cheatsheet.md";
    "/doc/cheatsheet.md"                "/doc/04_bovnar_unit_cheatsheet.md";
    "/doc/2_bovnar_unit_system.md"      "/doc/05_bovnar_unit_system.md";
    "/doc/parser_level_unit_policy.md"  "/doc/06_bovnar_unit_policy.md";
    "/doc/unit_ambiguities.md"          "/doc/07_bovnar_unit_ambiguities.md";
    "/doc/3_bovnar_readwrite_api.md"    "/doc/08_bovnar_readwrite_api.md";
    "/doc/4_bovnar_python_bindings.md"  "/doc/09_bovnar_python_bindings.md";
    "/doc/9_bovnar_streaming.md"        "/doc/10_bovnar_streaming.md";
    "/doc/10_bovnar_ucum_profile.md"    "/doc/11_bovnar_unit_profiles.md";
    "/doc/11_bovnar_ucum_profile.md"    "/doc/11_bovnar_unit_profiles.md";
    "/doc/ucum_profile.md"              "/doc/11_bovnar_unit_profiles.md";
    "/doc/5_bovnar.ebnf"                "/doc/12_bovnar.ebnf";
    "/doc/7_bovnar_conformance.md"      "/doc/13_bovnar_conformance.md";
    # A renamed document renames its PDF too. The asset location below applies
    # this same map, so the old download keeps working instead of 404ing the
    # moment a --delete publish prunes it.
    "/doc/pdf/bovnar-ucum-profile.pdf"  "/doc/pdf/bovnar-unit-profiles.pdf";
}

# Renamed RENDERED pages. Separate from $bvnr_doc_moved above: that one answers
# the Markdown source, this one the HTML rendering, and a single map keyed on
# both would 301 a .md request to an HTML page.
map $uri $bvnr_page_moved {
    default                             "";
    "/docs/ucum/"                       "/docs/profiles/";
    "/docs/ucum"                        "/docs/profiles/";
}
```

```nginx
    # first line of location ~ \.md$, location ~ \.ebnf$ AND the static-asset
    # location (a renamed document renames its PDF), so the redirect happens
    # before try_files can answer 404
    if ($bvnr_doc_moved) { return 301 $bvnr_doc_moved; }

    # and the same shape at the top of location /, for a renamed RENDERED page
    if ($bvnr_page_moved) { return 301 $bvnr_page_moved; }
```

Two things to keep right when adding an entry:

- **No key may name a document that exists today.** Two keys carry a two-digit
  prefix. `/doc/10_bovnar_ucum_profile.md` is there because the UCUM profile held
  slot 10 briefly and streaming holds it now, which is why
  `/doc/10_bovnar_streaming.md` is a value here and never a key; and
  `/doc/11_bovnar_ucum_profile.md` is there because that document kept its number
  and changed its name when it grew to cover five vocabularies. A key that
  collides with a live document redirects it away from itself.
- **A deleted document is not a moved one.** `/doc/iana_media_type.md` (earlier
  `/doc/9_iana_media_type.md`) explained how to apply for the media type and was
  removed once the type was registered. It has no successor, so it is absent
  here and 404 is the honest answer.

The redirects are the stopgap, not the fix: the standing IANA registration keeps
citing the old paths until the update requested in `doc/ietf/` is accepted. See
the next section for the part of this that nginx cannot reach.

## What the redirects do *not* cover

The published registration at
<https://www.iana.org/assignments/media-types/text/vnd.bovnar> cites the
specification, the unit system, the grammar, the FAQ and the conformance suite
as **`github.com/sothis/bovnar/blob/main/doc/<old-name>`** URLs, not
`www.bovnar.io` ones. No configuration of this web server can affect them — they
are served by GitHub from the repository tree, so the only things that reach them
are a change to the repository or a corrected registration.

The repository change was made: `doc/1_bovnar_spec.md`,
`doc/2_bovnar_unit_system.md`, `doc/5_bovnar.ebnf`, `doc/6_bovnar_faq.md` and
`doc/7_bovnar_conformance.md` are back as **pointer files** — each names the
document it moved to, in the repository and on the site, and says it is not
normative. All five cited URLs resolve again. They are not in `WEBDOCS` in
`publish_web.sh`, so they exist for GitHub only and never reach the site, where
the `map` above already answers those paths with a 301.

The pointers are the stopgap, not the fix: they keep a published registration
resolving, but the registry entry still depends on this repository preserving
five filenames indefinitely. The `Published specification` field in the
registration template in `doc/ietf/draft-sonntag-bovnar-00.md` uses
`www.bovnar.io` paths instead — redirected across renames rather than
invalidated by them — and §"Corrections to the Existing Registration" there <!-- bovnar:no-section-check: the target is doc/ietf/draft-sonntag-bovnar-00.md, which this checker does not index -->
lists the replacement as one of the changes requested. The pointer files stay
until that update is accepted.

## Response headers (`/etc/nginx/snippets/bovnar-headers.conf`)

The pages carry their own hash-based CSP in a `<meta>`, which covers scripts,
styles and everything else that loads — but `frame-ancestors` is ignored in a
`<meta>` by specification, so until this snippet existed the site could be framed
by anyone. That directive only works as a header, so it is one here, alongside
the two headers no static site should be without.

Kept in one file because `add_header` is not additive: a location that declares
any header of its own stops inheriting the entire server-level set. Every
location in the config below that adds a header therefore `include`s this file,
which is what stops the set from quietly thinning out one location at a time.

```nginx
##
# bovnar — the header set every response from the canonical host carries.
#
# add_header is NOT additive across configuration levels: the moment a location
# declares one header of its own, it stops inheriting the entire set from the
# server block. That trap already cost this config its HSTS in four locations
# once. Keeping the set in one file and including it wherever headers are
# declared means the next location cannot silently ship a thinner set.
#
#   Strict-Transport-Security  — HTTPS-only for a year.
#   X-Content-Type-Options     — no MIME sniffing; every type here is declared.
#   Referrer-Policy            — send the full URL same-origin, the bare origin
#                                cross-origin, nothing when downgrading.
#   Content-Security-Policy    — framing only. The pages carry their own full
#                                policy in a <meta>, where frame-ancestors is
#                                ignored by definition, so the site was
#                                embeddable in anyone's iframe. A header is the
#                                only place that directive works; the two
#                                policies are enforced together, and this one
#                                restricts nothing else.
##
add_header Strict-Transport-Security "max-age=31536000" always;
add_header X-Content-Type-Options "nosniff" always;
add_header Referrer-Policy "strict-origin-when-cross-origin" always;
add_header Content-Security-Policy "frame-ancestors 'none'" always;
```

## Complete assembled nginx config

The live `www.bovnar.io` config — this block is a verbatim copy of
`/etc/nginx/sites-available/bovnar` as applied, with every AI/SEO, MIME,
compression and caching addition folded in (the two apex/`.com`/`.net`/`.org`
redirect blocks are unchanged from the original):

```nginx
##
# bovnar — canonical host: www.bovnar.io
# Every other name (apex bovnar.io + all .com/.net/.org) redirects here.
##

# AI/LLM content negotiation: does the client prefer Markdown? (http{} context)
map $http_accept $bvnr_wants_md {
    default            0;
    "~*text/markdown"  1;
}

# Cache lifetimes. Every versioned asset is requested as <path>?v=<content-hash>
# (wasm/build_wasm.sh stamps them from the file's own bytes), so a fingerprinted
# URL is safe to cache forever -- that is the point of the fingerprint. A bare
# path is not necessarily the same bytes forever, so it revalidates sooner.
# Only a 12-hex-char stamp earns the immutable year, because that is what
# build_wasm.sh emits (sha256 of the file, cut -c1-12) and it is verified by
# cmake/cache_stamps.cmake. Anything else -- a hand-written date, a hand-edited
# value -- falls to the short TTL instead of pinning a stale asset in caches for
# a year. fonts/fonts.css was exactly that case: ?v=20260724, kept correct only
# by whoever remembered to bump it. It is content-hashed now, and this pattern
# means the next one to slip through is merely uncached, not wrong.
map $arg_v $bvnr_cache_ctl {
    default            "public, max-age=3600";
    "~^[0-9a-f]{12}$"  "public, max-age=31536000, immutable";
}

# Renamed documents. The doc set has been renumbered twice -- most recently in
# "docs+web: renumber the documentation set", which gave every document a
# two-digit prefix -- and each rename silently broke every URL already published
# under the old name. Those URLs are not ours to recall: the IANA registration
# for text/vnd.bovnar cites the specification, the unit system and the grammar
# by their pre-renumbering paths, and so do released tarballs, the PyPI page's
# history and whatever anyone else linked. A 301 keeps them working and tells
# caches and crawlers the name is final, which a 404 cannot.
#
# Every key here is a path that was live at some point; none of them collides
# with a document that exists today (the old single-digit prefixes and the two
# unprefixed working titles are all retired). /doc/10_bovnar_ucum_profile.md is
# the one two-digit key: the UCUM profile briefly held slot 10, which streaming
# occupies now -- so it is listed and /doc/10_bovnar_streaming.md is not.
#
# Not listed: /doc/iana_media_type.md (and its earlier /doc/9_iana_media_type.md
# name). That document was deleted on purpose once the media type was registered
# -- it described how to apply -- so it has no successor to point at and 404 is
# the honest answer.
map $uri $bvnr_doc_moved {
    default                             "";
    "/doc/0_bovnar_tutorial.md"         "/doc/01_bovnar_tutorial.md";
    "/doc/6_bovnar_faq.md"              "/doc/02_bovnar_faq.md";
    "/doc/1_bovnar_spec.md"             "/doc/03_bovnar_spec.md";
    "/doc/8_unit_cheatsheet.md"         "/doc/04_bovnar_unit_cheatsheet.md";
    "/doc/cheatsheet.md"                "/doc/04_bovnar_unit_cheatsheet.md";
    "/doc/2_bovnar_unit_system.md"      "/doc/05_bovnar_unit_system.md";
    "/doc/parser_level_unit_policy.md"  "/doc/06_bovnar_unit_policy.md";
    "/doc/unit_ambiguities.md"          "/doc/07_bovnar_unit_ambiguities.md";
    "/doc/3_bovnar_readwrite_api.md"    "/doc/08_bovnar_readwrite_api.md";
    "/doc/4_bovnar_python_bindings.md"  "/doc/09_bovnar_python_bindings.md";
    "/doc/9_bovnar_streaming.md"        "/doc/10_bovnar_streaming.md";
    "/doc/10_bovnar_ucum_profile.md"    "/doc/11_bovnar_unit_profiles.md";
    "/doc/11_bovnar_ucum_profile.md"    "/doc/11_bovnar_unit_profiles.md";
    "/doc/ucum_profile.md"              "/doc/11_bovnar_unit_profiles.md";
    "/doc/5_bovnar.ebnf"                "/doc/12_bovnar.ebnf";
    "/doc/7_bovnar_conformance.md"      "/doc/13_bovnar_conformance.md";
    # A renamed document renames its PDF too. The asset location below applies
    # this same map, so the old download keeps working instead of 404ing the
    # moment a --delete publish prunes it.
    "/doc/pdf/bovnar-ucum-profile.pdf"  "/doc/pdf/bovnar-unit-profiles.pdf";
}

# Renamed RENDERED pages. Separate from $bvnr_doc_moved above: that one answers
# the Markdown source, this one the HTML rendering, and a single map keyed on
# both would 301 a .md request to an HTML page.
map $uri $bvnr_page_moved {
    default                             "";
    "/docs/ucum/"                       "/docs/profiles/";
    "/docs/ucum"                        "/docs/profiles/";
}

# Search consolidation: the full "Link: rel=canonical" value (RFC 8288 — URI in
# angle brackets) for each raw document; empty for /index.md and /de/index.md,
# so nginx adds no canonical header there (it omits an empty-valued add_header).
#
# These keys were left on the pre-renumbering paths when the documents were
# renamed, so the map had stopped matching anything: every raw .md served since
# went out with no canonical header, pointing search engines at nothing while
# the HTML edition under /docs/<slug>/ competed with it. Keys are the current
# filenames; the old ones now 301 here via $bvnr_doc_moved above, so a request
# for a renamed document lands on the new path and picks the header up there.
map $uri $bvnr_canon_hdr {
    default                              "";
    "/doc/01_bovnar_tutorial.md"         "<https://www.bovnar.io/docs/tutorial/>; rel=\"canonical\"";
    "/doc/02_bovnar_faq.md"              "<https://www.bovnar.io/docs/faq/>; rel=\"canonical\"";
    "/doc/03_bovnar_spec.md"             "<https://www.bovnar.io/docs/spec/>; rel=\"canonical\"";
    "/doc/04_bovnar_unit_cheatsheet.md"  "<https://www.bovnar.io/docs/cheatsheet/>; rel=\"canonical\"";
    "/doc/05_bovnar_unit_system.md"      "<https://www.bovnar.io/docs/units/>; rel=\"canonical\"";
    "/doc/06_bovnar_unit_policy.md"      "<https://www.bovnar.io/docs/policy/>; rel=\"canonical\"";
    "/doc/07_bovnar_unit_ambiguities.md" "<https://www.bovnar.io/docs/ambiguities/>; rel=\"canonical\"";
    "/doc/08_bovnar_readwrite_api.md"    "<https://www.bovnar.io/docs/api/>; rel=\"canonical\"";
    "/doc/09_bovnar_python_bindings.md"  "<https://www.bovnar.io/docs/python/>; rel=\"canonical\"";
    "/doc/10_bovnar_streaming.md"        "<https://www.bovnar.io/docs/streaming/>; rel=\"canonical\"";
    "/doc/11_bovnar_unit_profiles.md"    "<https://www.bovnar.io/docs/profiles/>; rel=\"canonical\"";
    "/doc/12_bovnar.ebnf"                "<https://www.bovnar.io/docs/grammar/>; rel=\"canonical\"";
    "/doc/13_bovnar_conformance.md"      "<https://www.bovnar.io/docs/conformance/>; rel=\"canonical\"";
    "/doc/14_bovnar_rust_bindings.md"    "<https://www.bovnar.io/docs/rust/>; rel=\"canonical\"";
}

# 1) Serve the site over HTTPS on the canonical host only
server {
    listen 443 ssl;
    listen [::]:443 ssl;
    # ALPN offered only http/1.1 before this, so every visit fetched the page and
    # its 15 subresources over HTTP/1.1 with the usual 6-connection cap and
    # head-of-line blocking. Directive form, not "listen ... http2", which nginx
    # deprecated in 1.25.1. Clients that do not speak h2 still negotiate 1.1.
    http2 on;
    server_name www.bovnar.io;

    root /var/www/html;
    index index.html index.htm index.nginx-debian.html;

    # Compression. nginx compresses text/html and nothing else unless told, and
    # the http{} block leaves gzip_types commented out -- so llms-full.txt (560
    # KB), bovnar_wasm_core.js (252 KB), highlight.js (122 KB) and the doc .md
    # all went out raw: 791 KB per cold visit, 58% of the text bytes on the wire.
    # text/html is deliberately absent: listing it is a duplicate and nginx warns.
    # gzip_vary is required for correctness, not tuning -- without it a gzipped
    # response carries no "Vary: Accept-Encoding" and a shared cache may hand the
    # compressed body to a client that never asked for it.
    gzip              on;
    gzip_vary         on;
    gzip_comp_level   6;
    gzip_min_length   256;
    gzip_proxied      any;
    gzip_types        text/plain text/css text/xml text/markdown
                      application/javascript application/json
                      application/manifest+json application/xml
                      application/rss+xml image/svg+xml;

    # Label the charset instead of leaving it to the client's locale. llms.txt
    # and llms-full.txt are text/plain and carry 19 kB of non-ASCII (µ, Ω, ×, —)
    # with no in-band declaration to fall back on, so a browser guessing
    # windows-1252 rendered them as mojibake. The .md and .ebnf locations each
    # said utf-8 already; saying it once at the top covers text/html, text/plain
    # and text/xml (nginx's default charset_types) and cannot drift per location.
    # No source_charset is set, so nginx labels the bytes, it does not re-code
    # them.
    charset utf-8;
    # nginx's default charset_types leaves out text/css and the manifest type,
    # and both files here contain non-ASCII. A stylesheet with no charset falls
    # back to the encoding of the document that linked it, which is UTF-8 today
    # but is not something a stylesheet should have to depend on.
    # text/html is deliberately absent: charset_types always includes it and
    # naming it again makes nginx warn on every config test and reload (the same
    # reason gzip_types above leaves it out).
    charset_types text/xml text/plain text/css
                  application/javascript application/manifest+json;

    # Branded 404 page (web/404.html).
    error_page 404 /404.html;
    location = /404.html { internal; }

    # Home page: advertise + serve the Markdown edition to AI tools. Declaring
    # add_header here drops the whole inherited set, so the snippet is included.
    location = / {
        if ($bvnr_wants_md) { return 302 /index.md; }
        add_header Link '</index.md>; rel="alternate"; type="text/markdown"' always;
        add_header Vary 'Accept' always;
        add_header Cache-Control "no-cache" always;
        include snippets/bovnar-headers.conf;
        try_files /index.html =404;
    }

    # German home page, pointing at /de/index.md.
    location = /de/ {
        if ($bvnr_wants_md) { return 302 /de/index.md; }
        add_header Link '</de/index.md>; rel="alternate"; type="text/markdown"' always;
        add_header Vary 'Accept' always;
        add_header Cache-Control "no-cache" always;
        include snippets/bovnar-headers.conf;
        try_files /de/index.html =404;
    }

    # .md as Markdown (RFC 7763) + UTF-8; plus the canonical->HTML header for the
    # doc .md (empty, hence omitted, for /index.md and /de/index.md).
    location ~ \.md$ {
        # A document that has been renamed: 301 to its current path before
        # try_files can turn it into a 404 (see the $bvnr_doc_moved map).
        if ($bvnr_doc_moved) { return 301 $bvnr_doc_moved; }

        types         { }
        default_type  text/markdown;
        charset       utf-8;
        charset_types text/markdown;
        add_header Link "$bvnr_canon_hdr" always;
        add_header Cache-Control "no-cache" always;
        include snippets/bovnar-headers.conf;
        try_files $uri =404;
    }

    # The one non-Markdown document. Nothing maps .ebnf, so it fell through to
    # default_type (application/octet-stream) and the browser offered a download
    # instead of showing the grammar -- while its nine Markdown siblings on
    # /docs/ display inline, and /docs/grammar/ advertises it as
    # <link rel="alternate" type="text/plain">, which the server contradicted.
    location ~ \.ebnf$ {
        # The grammar was 5_bovnar.ebnf until the renumbering; same 301 as the
        # .md location above, and the IANA registration still cites the old name.
        if ($bvnr_doc_moved) { return 301 $bvnr_doc_moved; }

        types         { }
        default_type  text/plain;
        charset       utf-8;
        charset_types text/plain;
        # The grammar had no canonical header while its Markdown siblings did,
        # so /doc/12_bovnar.ebnf and /docs/grammar/ competed as two URLs for one
        # document. Same map, same reason.
        add_header Link "$bvnr_canon_hdr" always;
        add_header Cache-Control "no-cache" always;
        include snippets/bovnar-headers.conf;
        try_files $uri =404;
    }

    # mime.types has no .webmanifest entry, so the manifest went out as
    # application/octet-stream. Naming it also brings it under gzip_types.
    location ~ \.webmanifest$ {
        types         { }
        default_type  application/manifest+json;
        charset       utf-8;
        try_files $uri =404;
    }

    # Static assets -- see the $bvnr_cache_ctl map. Nothing sent Cache-Control at
    # all before this, so every asset revalidated on every visit. Declaring
    # add_header here stops this location inheriting the server-level headers, so
    # it includes them (same trap as the home-page locations above).
    #
    # pdf and zip are here because the documentation ships as both: ten PDFs and
    # a 2.5 MB bundle, none of which matched this list, so each download
    # revalidated on every visit while every other asset on the site was told
    # what to do. They are regenerated on publish, hence the map's short default
    # rather than a year -- a stale PDF for an hour, not for a year.
    location ~* \.(js|css|woff2|jpe?g|png|svg|ico|pdf|zip)$ {
        # A renamed document's PDF, same map and same reason as the .md and
        # .ebnf locations: 301 before try_files can answer 404.
        if ($bvnr_doc_moved) { return 301 $bvnr_doc_moved; }

        add_header Cache-Control "$bvnr_cache_ctl" always;
        include snippets/bovnar-headers.conf;
        try_files $uri =404;
    }

    # Documents carry no ?v= stamp, so they need the opposite of the asset rule
    # above: revalidate every time. Sending no Cache-Control at all is not
    # "no caching" -- RFC 9111 lets a browser guess a freshness lifetime from
    # Last-Modified (commonly a tenth of the file's age), and these files sit
    # unchanged for weeks between publishes, so the guess ran to days. A
    # returning visitor kept a months-old landing page and months-old .md behind
    # the inline doc viewer, fetching nothing and asking nothing. "no-cache"
    # still stores the copy; it just makes the browser ask, and the ETag turns
    # that into a 304.
    location / {
        # A rendered page that has been renamed: 301 before try_files can answer
        # (see the $bvnr_page_moved map). This lives here rather than in a
        # location /docs/ of its own: a prefix block would take over every
        # /docs/ request and have to restate this one's headers and try_files,
        # and $bvnr_page_moved is empty for everything else, so the test costs
        # one variable lookup and changes nothing it does not name.
        if ($bvnr_page_moved) { return 301 $bvnr_page_moved; }

        add_header Cache-Control "no-cache" always;
        include snippets/bovnar-headers.conf;
        try_files $uri $uri/ =404;
    }

    # TLS — the multi-SAN cert lineage is named "bovnar.com"; do NOT repoint at
    # /etc/letsencrypt/live/bovnar.io/ (no such path).
    ssl_certificate         /etc/letsencrypt/live/bovnar.com/fullchain.pem;
    ssl_certificate_key     /etc/letsencrypt/live/bovnar.com/privkey.pem;
    ssl_trusted_certificate /etc/letsencrypt/live/bovnar.com/chain.pem;
    include                 /etc/letsencrypt/options-ssl-nginx.conf;
    ssl_dhparam             /etc/letsencrypt/ssl-dhparams.pem;

    # ssl_stapling removed: Let's Encrypt has retired OCSP and its certificates
    # carry no responder URL, so nginx logged "ssl_stapling ignored" on every
    # config test and reload while stapling nothing. A standing benign warning
    # is how a real one gets missed.

    include snippets/bovnar-headers.conf;
}

# 2) HTTPS redirect for every non-canonical name -> https://www.bovnar.io
server {
    listen 443 ssl default_server;
    listen [::]:443 ssl default_server;
    http2 on;
    server_name bovnar.io
                bovnar.com www.bovnar.com
                bovnar.net www.bovnar.net
                bovnar.org www.bovnar.org;

    ssl_certificate     /etc/letsencrypt/live/bovnar.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/bovnar.com/privkey.pem;
    include             /etc/letsencrypt/options-ssl-nginx.conf;
    ssl_dhparam         /etc/letsencrypt/ssl-dhparams.pem;

    include snippets/bovnar-headers.conf;

    return 301 https://www.bovnar.io$request_uri;
}

# 3) HTTP redirect for every name (incl. www.bovnar.io) -> https://www.bovnar.io
server {
    listen 80 default_server;
    listen [::]:80 default_server;
    server_name bovnar.io www.bovnar.io
                bovnar.com www.bovnar.com
                bovnar.net www.bovnar.net
                bovnar.org www.bovnar.org;

    return 301 https://www.bovnar.io$request_uri;
}
```

Apply with `nginx -t && systemctl reload nginx`, then validate with
acceptmarkdown.com / isitagentready.com.

## Analytics (optional, operational)

To see whether any of this pays off, log requests to the AI endpoints and
distinguish AI traffic — no repo change, just your existing access log:

- Watch the paths `/llms.txt`, `/llms-full.txt`, `/index.md`, and `/doc/*.md`.
- Bucket by `User-Agent` to spot AI crawlers (`GPTBot`, `ClaudeBot`,
  `PerplexityBot`, `Google-Extended`, …) and by `Accept: text/markdown`.
- Bucket by `Referer` host to catch human-initiated fetches from `chatgpt.com`,
  `claude.ai`, `perplexity.ai`.

Example (nginx combined log):

```sh
awk '$7 ~ /\.md$|llms(-full)?\.txt/' /var/log/nginx/access.log \
  | grep -iE 'GPTBot|ClaudeBot|PerplexityBot|Google-Extended|text/markdown'
```
