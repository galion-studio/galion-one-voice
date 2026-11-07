#!/bin/sh
# Build the static browser site into dist/. dist/ is gitignored.
set -eu
cd "$(dirname "$0")"
rustup target add wasm32-unknown-unknown >/dev/null
# wasm dependencies in the lockfile need Rust 1.85.
cargo +1.85.0 build --release --target wasm32-unknown-unknown --lib
BINDGEN="${WASM_BINDGEN:-wasm-bindgen}"
rm -rf dist
mkdir -p dist
"$BINDGEN" --target web --out-dir dist/pkg target/wasm32-unknown-unknown/release/nebula_voice_v2.wasm
cp web/index.html web/samantha.html web/jarvis.html web/spectrum.html web/nebula.css web/spectrum.css web/boot.js web/gpu.js web/spectrum.js web/cie.js web/lines.js serve.py dist/

# Stamp every asset URL with the content version so a cached edge cannot
# mix HTML/JS from one build with wasm from another.
VERSION="${NEBULA_ASSET_VER:-$(git rev-parse --short=12 HEAD)}"
for f in dist/*.html dist/*.js dist/pkg/nebula_voice_v2.js; do
  sed -i "s/[?]v=[A-Za-z0-9._-]*/?v=${VERSION}/g" "$f"
done

# Cache-bust the wasm fetch. import.meta.url query strings are dropped
# by `new URL('file.wasm', import.meta.url)`, so the filename itself
# must carry the version.
sed -i "s|new URL('nebula_voice_v2_bg.wasm', import.meta.url)|new URL('nebula_voice_v2_bg.wasm?v=${VERSION}', import.meta.url)|" dist/pkg/nebula_voice_v2.js
sed -i "s|new URL('nebula_voice_v2_bg.wasm?v=[^']*'|new URL('nebula_voice_v2_bg.wasm?v=${VERSION}'|" dist/pkg/nebula_voice_v2.js
echo "static site: $(pwd)/dist  version=${VERSION}"
