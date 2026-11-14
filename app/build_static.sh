#!/usr/bin/env sh
# Builds the static website version of Retal into public-html/.
# Upload that folder to any static host (GitHub Pages, Netlify, Cloudflare Pages, an S3 bucket...).
set -e
cd "$(dirname "$0")"

# 1. compile the game for the browser (also refreshes web/)
./build_web.sh

# 2. copy the site files
mkdir -p public-html
for f in index.html mq_js_bundle.js tilt.js net.js retal.wasm manifest.webmanifest icon-192.png icon-512.png; do
    cp "web/$f" "public-html/$f"
done
# keep a config.js the person already edited (it holds the multiplayer server address)
if [ ! -f public-html/config.js ]; then
    cp web/config.js public-html/config.js
fi
# GitHub Pages: serve files as-is (no Jekyll processing)
touch public-html/.nojekyll

echo "Static site ready in public-html/ ($(du -sh public-html | cut -f1))"
