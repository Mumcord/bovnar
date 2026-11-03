#!/bin/bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# Install the Bovnar syntax definition and colour scheme for Sublime Text.
set -euo pipefail
cd "$(dirname "$0")"

dest=""

# Locate the Sublime Text user Packages directory (ST4, then ST3 / Flatpak / macOS).
for base in \
    "${XDG_CONFIG_HOME:-$HOME/.config}/sublime-text/Packages" \
    "${XDG_CONFIG_HOME:-$HOME/.config}/sublime-text-3/Packages" \
    "$HOME/.var/app/com.sublimetext.three/config/sublime-text-3/Packages" \
    "$HOME/Library/Application Support/Sublime Text/Packages" \
    "$HOME/Library/Application Support/Sublime Text 3/Packages"; do
    if [ -d "$base" ]; then
        dest="$base/Bovnar"
        break
    fi
done

# Default to the Sublime Text 4 location if none exist yet.
: "${dest:=$HOME/.config/sublime-text/Packages/Bovnar}"

mkdir -p "$dest"
cp Bovnar.sublime-syntax "$dest"/
cp Bovnar-Cyberpunk.sublime-color-scheme "$dest"/

echo "Installed Bovnar Sublime Text package to: $dest"
echo "Activate via: Preferences > Select Color Scheme... > Bovnar Cyberpunk"
