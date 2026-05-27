#!/bin/bash
echo "Uninstalling Open Remote URL Client..."
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
chmod +x "$DIR/open-remote-url-client"
"$DIR/open-remote-url-client" --uninstall
