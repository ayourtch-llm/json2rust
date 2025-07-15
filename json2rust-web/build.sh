#!/bin/bash

# Build script for json2rust-web WASM module
set -e

echo "Building json2rust-web WASM module..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "wasm-pack is not installed. Installing..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Build the WASM module
wasm-pack build --target web --out-dir pkg

echo "Build complete! You can now open index.html in a web browser."
echo "Note: You may need to serve the files over HTTP(S) instead of file:// due to CORS restrictions."
echo "Try: python3 -m http.server 8000"