#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# One command for Samantha's live loop on Linux.
# No microphone, no speaker, no committed audio.
# Flite (libflite1) or a Piper voice, when present, is the mouth.
# The ear fetches vosk-model-small-en-us-0.15 (Apache-2.0) into a cache
# outside this repo. The sha256 pin is horbm/crates/horbm-e2e/vosk-model.manifest.
# The model is not committed. A failed fetch leaves asr=stub and says why.
# The pinned libvosk zip is Linux x86_64 only.
# Missing engines stay `stub` and are not given a timing.
# Ear prosody defaults on. GALION_EAR_SENSE=0|off|deny is the off switch;
# the Rust tests run both.

set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

# The generated sentences are spoken by libflite. Ubuntu's libflite1 package
# ships the slt voice. A machine without it keeps the ear on the stub.
if [[ "$(uname -s)" == "Linux" ]] && ! ldconfig -p 2>/dev/null | grep -q 'libflite_cmu_us_slt\.so'; then
  echo "horbm-e2e: libflite slt voice is not installed"
  if command -v sudo >/dev/null 2>&1 && sudo -n true 2>/dev/null && command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update -y && sudo apt-get install -y libflite1 || echo "horbm-e2e: could not install libflite1"
  fi
fi

manifest="$root/horbm/crates/horbm-e2e/Cargo.toml"
profile=(--release)
if [[ "${HORBM_E2E_DEBUG:-}" == "1" ]]; then
  profile=()
fi

# The crate's MSRV is 1.85. CI installs that toolchain as `cargo`. A machine
# whose default rustc is older can still run when rustup has 1.85.0.
rustc_num() {
  rustc --version | awk '{print $2}' | awk -F. '{printf "%d%03d%03d\n", $1, $2, $3}'
}
if [[ "$(rustc_num)" -ge 1085000 ]]; then
  cargo_cmd=(cargo)
elif rustup toolchain list 2>/dev/null | grep -q '^1.85.0'; then
  cargo_cmd=(cargo +1.85.0)
else
  echo "horbm-e2e needs rustc 1.85 or newer" >&2
  exit 1
fi

"${cargo_cmd[@]}" clippy --manifest-path "$manifest" --all-targets --no-deps -- -D warnings
"${cargo_cmd[@]}" test --manifest-path "$manifest" "${profile[@]}" -- --nocapture --test-threads=1

audio=$(git -C "$root" status --porcelain | grep -Ei '\.(wav|flac|mp3|ogg|opus)$' || true)
if [[ -n "${audio}" ]]; then
  echo "FAIL: voice audio would be committed" >&2
  echo "${audio}" >&2
  exit 1
fi
