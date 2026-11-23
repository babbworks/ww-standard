#!/bin/sh
# Build the engine for the browser and place it beside the page.
#   rustup target add wasm32-unknown-unknown   (once)
#   ./web/build.sh && python3 -m http.server -d web 8000
set -e
cd "$(dirname "$0")/.."
cargo build -p stream-wasm --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/stream_wasm.wasm web/stream.wasm
echo "web/stream.wasm: $(wc -c < web/stream.wasm) bytes"
