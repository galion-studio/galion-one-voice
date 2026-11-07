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
cp web/index.html web/samantha.html web/jarvis.html web/spectrum.html web/nebula.css web/spectrum.css web/boot.js web/spectrum.js web/cie.js web/lines.js serve.py dist/

# Cache-bust the wasm fetch. import.meta.url query strings are dropped
# by `new URL('file.wasm', import.meta.url)`, so the filename itself
# must carry the version.
sed -i "s|new URL('nebula_voice_v2_bg.wasm', import.meta.url)|new URL('nebula_voice_v2_bg.wasm?v=abde48e', import.meta.url)|" dist/pkg/nebula_voice_v2.js
echo "static site: $(pwd)/dist"
