#!/usr/bin/env bash
# Builds the browser version of the client into web/pkg.
#
# One-time setup:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.130   # must match Cargo.lock
#
# Then run `cargo run -p subhunt-server` and open http://localhost:8080
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build -p subhunt-client --profile web --target wasm32-unknown-unknown
wasm-bindgen --no-typescript --remove-name-section --remove-producers-section --target web --out-dir web/pkg --out-name subhunt \
  target/wasm32-unknown-unknown/web/subhunt-client.wasm

# Pre-compress the big files. The server sends the .gz copy to browsers that
# accept gzip, which cuts the download to about a quarter of the size.
if command -v gzip >/dev/null; then
  gzip -9 -k -f web/pkg/subhunt_bg.wasm web/pkg/subhunt.js
fi

echo "Built web/pkg. Start the server and open http://localhost:8080"
