#!/bin/bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# Register the Bovnar VS Code grammar as a TextMate bundle in CLion (and other
# JetBrains IDEs). This gives full regex-level highlighting via the shared
# source.bovnar grammar, unlike the keyword-based native file type (install.sh).
#
# JetBrains stores user TextMate bundles in
#   <config>/options/textmate.xml
# inside a <component name="TextMateUserBundlesSettings"> whose body is a JSON
# blob. The schema (verified against TextMateUserBundlesSettings.kt) is a MAP
# keyed by bundle path:
#   {"bundles":{"<path>":{"name":"<label>","enabled":true}}}
# (an array there is rejected with a kotlinx deserialization error and the whole
# file is discarded.) The IDE rewrites this file on exit, so CLion MUST be closed
# while this script runs.
set -euo pipefail
cd "$(dirname "$0")"

bundle="$(cd ../vscode/bovnar-highlight 2>/dev/null && pwd)" || true
[ -n "$bundle" ] && [ -f "$bundle/package.json" ] || {
    echo "error: VS Code grammar bundle not found at ../vscode/bovnar-highlight" >&2
    exit 1
}

# Match the actual IDE launcher path (e.g. /snap/clion/465/bin/clion or a
# Toolbox bin/clion) rather than the bare string "clion", which would also match
# this script's own path/command line and self-trip the guard.
if pgrep -f 'clion/[^ ]*/bin/clion( |$)' >/dev/null 2>&1; then
    echo "CLion appears to be running. Quit it fully first (it rewrites textmate.xml" >&2
    echo "on exit and would discard this change), then re-run this script." >&2
    exit 1
fi

case "$(uname -s)" in
    Darwin) root="$HOME/Library/Application Support/JetBrains" ;;
    *)      root="${XDG_CONFIG_HOME:-$HOME/.config}/JetBrains" ;;
esac
[ -d "$root" ] || { echo "error: JetBrains config dir not found: $root" >&2; exit 1; }

found=0
for cfg in "$root"/CLion* "$root"/clion*; do
    [ -d "$cfg" ] || continue
    opts="$cfg/options"
    mkdir -p "$opts"
    xml="$opts/textmate.xml"
    [ -f "$xml" ] && cp "$xml" "$xml.bak"

    BUNDLE="$bundle" XML="$xml" python3 - <<'PY'
import os, re, json, xml.sax.saxutils as su

bundle = os.environ["BUNDLE"]
xml = os.environ["XML"]
comp = "TextMateUserBundlesSettings"

# Read existing JSON payload from the component's CDATA/body, if present.
data = {"bundles": {}}
if os.path.exists(xml):
    text = open(xml, encoding="utf-8").read()
    m = re.search(r'<component name="%s">(.*?)</component>' % comp, text, re.S)
    if m:
        body = m.group(1).strip()
        cd = re.match(r'<!\[CDATA\[(.*)\]\]>$', body, re.S)
        raw = cd.group(1) if cd else su.unescape(body)
        raw = raw.strip()
        if raw:
            try:
                data = json.loads(raw)
            except ValueError:
                data = {"bundles": {}}
# bundles is a MAP keyed by path; tolerate/repair a stale list form.
if not isinstance(data.get("bundles"), dict):
    data["bundles"] = {}

# Upsert the Bovnar bundle keyed by its path.
data["bundles"][bundle] = {"name": "bovnar-highlight", "enabled": True}

payload = json.dumps(data, separators=(",", ":"))
component = '  <component name="%s"><![CDATA[%s]]></component>' % (comp, payload)
out = "<application>\n%s\n</application>" % component
open(xml, "w", encoding="utf-8").write(out)
print("Registered TextMate bundle in:", xml)
PY
    found=$((found + 1))
done

if [ "$found" -eq 0 ]; then
    echo "No CLion configuration directory found under $root/CLion<version>." >&2
    echo "Start CLion once, quit it, then re-run this script." >&2
    exit 1
fi

echo
echo "Done. Start CLion; .bvnr files now use the source.bovnar TextMate grammar."
echo "Bundle path: $bundle  (kept in place and read live — do not delete it)."
