#!/bin/bash
echo "Uninstalling Open Remote URL Host..."
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
chmod +x "$DIR/open-remote-url-host"
"$DIR/open-remote-url-host" --uninstall
