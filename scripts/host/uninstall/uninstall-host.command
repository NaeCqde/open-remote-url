#!/bin/bash
cd "$(dirname "$0")"
echo "Uninstalling Open Remote URL Host..."
chmod +x ./open-remote-url-host
./open-remote-url-host --uninstall
