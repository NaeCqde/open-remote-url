#!/bin/bash
echo "Installing Open Remote URL Client..."
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
chmod +x "$DIR/open-remote-url"
"$DIR/open-remote-url" --install
