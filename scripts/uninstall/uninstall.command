#!/bin/bash
cd "$(dirname "$0")"
echo "Uninstalling Open Remote URL..."
for exe in open-remote-url-*; do
    [ -f "$exe" ] || continue
    chmod +x "$exe"
    echo "Uninstalling $exe..."
    "./$exe" --uninstall
done
