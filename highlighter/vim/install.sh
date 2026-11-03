#!/bin/bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
#
# Install the Bovnar syntax + filetype detection for Vim and (if present) Neovim.
set -euo pipefail
cd "$(dirname "$0")"

install_to() {
    local root="$1"
    mkdir -p "$root/syntax" "$root/ftdetect"
    cp bovnar.vim    "$root/syntax/bovnar.vim"
    cp bovnar-ft.vim "$root/ftdetect/bovnar.vim"
    echo "Installed Bovnar Vim files to: $root"
}

# Vim
install_to "${HOME}/.vim"

# Neovim reads its own runtime path (not ~/.vim) by default.
if command -v nvim >/dev/null 2>&1; then
    install_to "${XDG_CONFIG_HOME:-$HOME/.config}/nvim"
fi

echo "Open a .bvnr file to verify highlighting (re-open already-open files)."
