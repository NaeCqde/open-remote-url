#!/bin/bash
cd "$(dirname "$0")"
echo "Opening Open Remote URL config..."
for exe in open-remote-url-*; do
    [ -f "$exe" ] || continue
    chmod +x "$exe"
    echo "Config for $exe..."
    "./$exe" --config
done
