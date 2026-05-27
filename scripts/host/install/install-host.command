#!/bin/bash
cd "$(dirname "$0")"
echo "Installing Open Remote URL Host..."
chmod +x ./open-remote-url-host
./open-remote-url-host --install
