#!/bin/bash
cd "$(dirname "$0")"
echo "Uninstalling Open Remote URL Client..."
chmod +x ./open-remote-url
./open-remote-url --uninstall
