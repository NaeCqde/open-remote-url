#!/bin/bash
cd "$(dirname "$0")"
echo "Installing Open Remote URL..."
for exe in open-remote-url-*; do
    [ -f "$exe" ] || continue
    chmod +x "$exe"
    echo "Installing $exe..."
    "./$exe" --install
done
