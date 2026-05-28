#!/bin/bash
echo "Uninstalling Open Remote URL..."
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
for exe in "$DIR"/open-remote-url-*; do
    [ -f "$exe" ] || continue
    chmod +x "$exe"
    echo "Uninstalling $(basename "$exe")..."
    "$exe" --uninstall
done
if [ -n "$DISPLAY" ] || [ -n "$WAYLAND_DISPLAY" ]; then
    read -r -p "Press Enter to close..." _
fi
