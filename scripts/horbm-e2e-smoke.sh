#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# One command for Samantha's live loop on Linux.
# No microphone. No committed audio.
# Playback is cpal/ALSA when this is Linux and the ALSA headers are installed.
# The loopback buffer remains the fallback.
# Voice activity is Silero when the official model downloads. The energy gate
# remains the fallback. The model is cached outside the repo.
# Flite (libflite1) or a Piper voice, when present, is the mouth.
# Missing engines stay `stub` and are not given a timing.
# Ear prosody defaults on. GALION_EAR_SENSE=0|off|deny is the off switch;
# the Rust tests run both.

set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

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

features=()
if [[ "$(uname -s)" == "Linux" && "${HORBM_E2E_NO_ALSA:-}" != "1" ]]; then
  if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists alsa; then
    features+=(alsa)
  fi
fi
if [[ "${HORBM_E2E_NO_SILERO:-}" != "1" ]]; then
  features+=(silero)
fi

# No sound card: ALSA's null PCM, so the cpal host still opens. snd-dummy is
# not required. A card already listed under /proc/asound/cards is left alone.
if [[ "$(uname -s)" == "Linux" && -z "${ALSA_CONFIG_PATH:-}" ]]; then
  have_card=0
  if [[ -r /proc/asound/cards ]] && grep -Eq '^[[:space:]]*[0-9]' /proc/asound/cards; then
    have_card=1
  fi
  if [[ "$have_card" -eq 0 ]]; then
    asound_dir=$(mktemp -d "${TMPDIR:-/tmp}/horbm-e2e-asound.XXXXXX")
    cat > "$asound_dir/asoundrc" << 'EOF'
pcm.!default {
    type null
}
ctl.!default {
    type null
}
EOF
    export ALSA_CONFIG_PATH="$asound_dir/asoundrc"
    trap 'rm -rf "$asound_dir"' EXIT
  fi
fi

feature_args=()
if [[ ${#features[@]} -gt 0 ]]; then
  joined=$(IFS=,; echo "${features[*]}")
  feature_args=(--features "$joined")
fi

# ONNX Runtime's CPU build links libstdc++. On this image `cc` is clang and
# does not search GCC's library directory. Use gcc as the linker when it exists.
if [[ "$(uname -s)" == "Linux" ]] && command -v gcc >/dev/null 2>&1; then
  if [[ ${#cargo_cmd[@]} -gt 1 ]]; then
    host=$(rustc "${cargo_cmd[1]}" -vV | awk '/^host:/{print $2}')
  else
    host=$(rustc -vV | awk '/^host:/{print $2}')
  fi
  if [[ -n "$host" ]]; then
    key="CARGO_TARGET_$(printf '%s' "$host" | tr '[:lower:]-' '[:upper:]_')_LINKER"
    export "${key}=gcc"
  fi
fi

"${cargo_cmd[@]}" clippy --manifest-path "$manifest" --all-targets --no-deps "${feature_args[@]}" -- -D warnings
"${cargo_cmd[@]}" test --manifest-path "$manifest" "${profile[@]}" "${feature_args[@]}" -- --nocapture --test-threads=1

audio=$(git -C "$root" status --porcelain | grep -Ei '\.(wav|flac|mp3|ogg|opus|onnx)$' || true)
if [[ -n "${audio}" ]]; then
  echo "FAIL: voice audio or a model binary would be committed" >&2
  echo "${audio}" >&2
  exit 1
fi
