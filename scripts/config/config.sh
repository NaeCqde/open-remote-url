#!/bin/bash
echo "Opening Open Remote URL config..."
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
for exe in "$DIR"/open-remote-url-*; do
    [ -f "$exe" ] || continue
    chmod +x "$exe"
    echo "Config for $(basename "$exe")..."
    "$exe" --config
done
if [ -n "$DISPLAY" ] || [ -n "$WAYLAND_DISPLAY" ]; then
    read -r -p "Press Enter to close..." _
fi
