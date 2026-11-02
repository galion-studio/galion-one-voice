#!/usr/bin/env bash
# Galion Voice canned demo — MIT. Regenerates all artifacts in ./out (no network, no model needed).
set -euo pipefail
cd "$(dirname "$0")"
python3 galion_demo.py all
sha256sum out/*.wav out/console_capture.txt > out/SHA256SUMS
echo "artifacts in $(pwd)/out"
