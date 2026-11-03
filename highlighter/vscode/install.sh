#!/bin/bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# Install the Bovnar syntax extension for the VS Code family (Code, Insiders,
# Code-OSS / VSCodium). Drops the extension folder into each editor's per-user
# extensions directory; VS Code picks it up on the next restart.
set -euo pipefail
cd "$(dirname "$0")"

src="bovnar-highlight"

# Per-user extension directories for the common VS Code variants.
candidates=(
    "${HOME}/.vscode/extensions"            # VS Code (deb/rpm/snap)
    "${HOME}/.vscode-insiders/extensions"   # VS Code Insiders
    "${HOME}/.vscode-oss/extensions"        # Code - OSS
    "${HOME}/.vscodium/extensions"          # VSCodium
)

install_to() {
    local ext="$1"
    # VS Code forms an extension's identity from "<publisher>.<name>" and
    # expects the folder to follow "<publisher>.<name>-<version>"; a folder
    # without that (and a package.json without "publisher") is skipped.
    local dest="$ext/bovnar.bovnar-highlight-2.0.0"
    # Drop any prior install: every versioned folder, plus the old plain name.
    rm -rf "$ext"/bovnar.bovnar-highlight-* "$ext/bovnar-highlight"
    mkdir -p "$ext"
    cp -r "$src" "$dest"

    # VS Code trusts its scan cache and will not notice a hand-copied folder
    # until the cache is rebuilt: removing extensions.json forces a full
    # re-scan (and a fresh cache) on the next start.
    rm -f "$ext/extensions.json"
    # Surgically drop any stale ".obsolete" marker from an earlier
    # publisher-less install (it keys on "undefined_publisher.bovnar-*").
    # Leave other extensions' markers intact.
    if [ -f "$ext/.obsolete" ] && grep -q 'bovnar' "$ext/.obsolete" \
       && command -v python3 >/dev/null 2>&1; then
        python3 - "$ext/.obsolete" <<'PY' || true
import json, sys
p = sys.argv[1]
try:
    d = json.load(open(p))
except Exception:
    sys.exit(0)
d = {k: v for k, v in d.items() if 'bovnar' not in k}
json.dump(d, open(p, 'w'))
PY
    fi
    echo "Installed to: $dest"
}

installed=0
for ext in "${candidates[@]}"; do
    # Only touch editors that have been run before (their dir exists).
    if [ -d "$ext" ]; then
        install_to "$ext"
        installed=1
    fi
done

# No editor dir found yet — fall back to the standard VS Code location.
if [ "$installed" -eq 0 ]; then
    install_to "${HOME}/.vscode/extensions"
fi

echo "Restart VS Code to activate. Optional theme: Preferences > Color Theme > 'Bovnar Cyberpunk'."
