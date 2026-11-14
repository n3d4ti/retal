#!/usr/bin/env sh
# Builds the browser version into web/ (open web/index.html through any static web server;
# phones need HTTPS for the tilt sensor).
set -e
cd "$(dirname "$0")"
cargo build --release --target wasm32-unknown-unknown --bin retal
cp target/wasm32-unknown-unknown/release/retal.wasm web/retal.wasm
echo "Browser build ready in web/ (index.html, mq_js_bundle.js, tilt.js, retal.wasm)"
