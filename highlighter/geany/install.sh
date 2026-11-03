#!/bin/bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# Install the Bovnar custom filetype for Geany and register the .bvnr extension.
set -euo pipefail
cd "$(dirname "$0")"

cfg="${XDG_CONFIG_HOME:-$HOME/.config}/geany"
mkdir -p "$cfg/filedefs"
cp filetypes.Bovnar.conf "$cfg/filedefs/filetypes.Bovnar.conf"
echo "Installed filetype: $cfg/filedefs/filetypes.Bovnar.conf"

# Geany detects a file's type from filetype_extensions.conf, NOT from the
# 'extension=' key of a custom filetype file. The user copy is merged over the
# system defaults, so we only need to ensure the Bovnar line is present in the
# [Extensions] group (replacing any stale one).
fe="$cfg/filetype_extensions.conf"
if [ -f "$fe" ]; then
    tmp="$(mktemp)"
    awk '
        BEGIN { in_ext = 0; inserted = 0; seen_ext = 0 }
        /^\[Extensions\]/ { print; in_ext = 1; seen_ext = 1; next }
        /^\[/ {                                   # leaving a section header
            if (in_ext && !inserted) { print "Bovnar=*.bvnr;"; inserted = 1 }
            in_ext = 0; print; next
        }
        in_ext && /^[ \t]*Bovnar[ \t]*=/ { next } # drop existing Bovnar mapping
        { print }
        END {
            if (in_ext && !inserted) print "Bovnar=*.bvnr;"
            else if (!seen_ext) { print "[Extensions]"; print "Bovnar=*.bvnr;" }
        }
    ' "$fe" > "$tmp"
    mv "$tmp" "$fe"
else
    printf '[Extensions]\nBovnar=*.bvnr;\n' > "$fe"
fi
echo "Registered .bvnr -> Bovnar in: $fe"
echo "Restart Geany to activate."
