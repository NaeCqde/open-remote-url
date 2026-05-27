#!/bin/bash
cd "$(dirname "$0")"
echo "Installing Open Remote URL Client..."
chmod +x ./open-remote-url-client
./open-remote-url-client --install
