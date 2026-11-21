# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# shellcheck shell=bash
# Download the public Vosk and Piper packs. Pins live in share/models.pins.
# A sha256 mismatch deletes the partial file and returns 1. No reference audio.

_models_lib=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib/common.sh
source "$_models_lib/common.sh"

load_pins() {
  local pins=$1
  # shellcheck disable=SC1090,SC1091
  source "$pins"
  : "${VOSK_MODEL_URL:?}"
  : "${VOSK_MODEL_SHA256:?}"
  : "${VOSK_LIB_URL:?}"
  : "${VOSK_LIB_SHA256:?}"
  : "${PIPER_RT_URL:?}"
  : "${PIPER_RT_SHA256:?}"
  : "${PIPER_VOICE_BASE:?}"
  : "${PIPER_ONNX_NAME:?}"
  : "${PIPER_ONNX_SHA256:?}"
  : "${PIPER_JSON_SHA256:?}"
}

fetch_checked() {
  local url=$1 dest=$2 sha=$3
  if file_ok "$dest" "$sha"; then
    echo "ok   $dest"
    return 0
  fi
  if [[ "${DRY_RUN:-0}" == 1 ]]; then
    echo "dry-run: would fetch $url"
    echo "dry-run: would write $dest"
    echo "dry-run: sha256 $sha"
    return 0
  fi
  if [[ "${CHECK_ONLY:-0}" == 1 ]]; then
    echo "MISSING/BAD $dest" >&2
    return 1
  fi
  command -v curl >/dev/null 2>&1 || die "curl is required to fetch models" 2
  command -v sha256sum >/dev/null 2>&1 || die "sha256sum is required" 2
  mkdir -p "$(dirname "$dest")"
  echo "get  $url"
  local part="${dest}.part"
  rm -f "$part"
  if ! curl -fL --retry 3 --retry-delay 2 --max-time 180 -sS -o "$part" "$url"; then
    rm -f "$part"
    echo "samantha: download failed for $url" >&2
    return 1
  fi
  if file_ok "$part" "$sha"; then
    mv -f "$part" "$dest"
    echo "ok   $dest"
    return 0
  fi
  rm -f "$part"
  echo "samantha: sha256 MISMATCH for $url" >&2
  return 1
}

write_stamp() {
  local dest=$1 sha=$2
  if [[ "${DRY_RUN:-0}" == 1 || "${CHECK_ONLY:-0}" == 1 ]]; then
    return 0
  fi
  mkdir -p "$(dirname "$dest")"
  printf '%s\n' "$sha" >"$dest"
}

stamp_ok() {
  local stamp=$1 sha=$2
  [[ -f "$stamp" ]] && [[ "$(tr -d '[:space:]' <"$stamp")" == "$sha" ]]
}

install_public_models() {
  local data=$1 pins=$2
  load_pins "$pins"
  local rc=0
  local asr="$data/models/asr"
  local tts="$data/models/tts"
  local stamps="$data/models/stamps"
  local model_dir="$asr/vosk-model-small-en-us-0.15"
  local lib_dir="$asr/libvosk"
  local piper_dir="$data/models/runtime/piper"
  local arch
  arch=$(uname -m)

  if [[ "${DRY_RUN:-0}" == 1 ]]; then
    echo "dry-run: would create $asr $tts $stamps"
  elif [[ "${CHECK_ONLY:-0}" != 1 ]]; then
    mkdir -p "$asr" "$tts" "$stamps" "$data/state"
  fi

  if [[ -f "$model_dir/am/final.mdl" && -f "$model_dir/conf/model.conf" ]] && stamp_ok "$stamps/vosk-model.sha256" "$VOSK_MODEL_SHA256"; then
    echo "ok   $model_dir"
  elif [[ "${CHECK_ONLY:-0}" == 1 ]]; then
    echo "MISSING/BAD $model_dir" >&2
    rc=1
  elif [[ "${DRY_RUN:-0}" == 1 ]]; then
    echo "dry-run: would fetch $VOSK_MODEL_URL"
    echo "dry-run: sha256 $VOSK_MODEL_SHA256"
  else
    command -v unzip >/dev/null 2>&1 || die "unzip is required for the Vosk model" 2
    fetch_checked "$VOSK_MODEL_URL" "$asr/vosk-model-small-en-us-0.15.zip" "$VOSK_MODEL_SHA256" || return 1
    rm -rf "$model_dir"
    unzip -q -o "$asr/vosk-model-small-en-us-0.15.zip" -d "$asr"
    rm -f "$asr/vosk-model-small-en-us-0.15.zip"
    if [[ -f "$model_dir/am/final.mdl" && -f "$model_dir/conf/model.conf" ]]; then
      write_stamp "$stamps/vosk-model.sha256" "$VOSK_MODEL_SHA256"
      echo "ok   $model_dir"
    else
      echo "samantha: Vosk model archive did not contain the expected files" >&2
      return 1
    fi
  fi

  fetch_checked "$PIPER_VOICE_BASE/$PIPER_ONNX_NAME" "$tts/$PIPER_ONNX_NAME" "$PIPER_ONNX_SHA256" || rc=1
  fetch_checked "$PIPER_VOICE_BASE/$PIPER_ONNX_NAME.json" "$tts/$PIPER_ONNX_NAME.json" "$PIPER_JSON_SHA256" || rc=1
  if [[ "${DRY_RUN:-0}" != 1 && "${CHECK_ONLY:-0}" != 1 && "$rc" -eq 0 ]]; then
    file_sha256 "$tts/$PIPER_ONNX_NAME" >"$tts/$PIPER_ONNX_NAME.sha256"
    file_sha256 "$tts/$PIPER_ONNX_NAME.json" >"$tts/$PIPER_ONNX_NAME.json.sha256"
  fi

  if [[ "$arch" != "x86_64" ]]; then
    echo "samantha: pinned Piper and libvosk binaries are linux x86_64 (this machine is $arch)" >&2
    echo "samantha: the Vosk model and Piper voice files are still installed when the download ran" >&2
    if [[ "$rc" -eq 0 ]]; then
      echo "MODELS=PARTIAL"
    else
      echo "MODELS=ABSENT"
    fi
    return "$rc"
  fi

  if [[ -f "$lib_dir/libvosk.so" ]] && stamp_ok "$stamps/libvosk.sha256" "$VOSK_LIB_SHA256"; then
    echo "ok   $lib_dir/libvosk.so"
  elif [[ "${CHECK_ONLY:-0}" == 1 ]]; then
    echo "MISSING/BAD $lib_dir/libvosk.so" >&2
    rc=1
  elif [[ "${DRY_RUN:-0}" == 1 ]]; then
    echo "dry-run: would fetch $VOSK_LIB_URL"
    echo "dry-run: sha256 $VOSK_LIB_SHA256"
  else
    command -v unzip >/dev/null 2>&1 || die "unzip is required for libvosk" 2
    fetch_checked "$VOSK_LIB_URL" "$asr/vosk-linux-x86_64-0.3.45.zip" "$VOSK_LIB_SHA256" || return 1
    rm -rf "$asr/vosk-linux-x86_64-0.3.45" "$lib_dir"
    unzip -q -o "$asr/vosk-linux-x86_64-0.3.45.zip" -d "$asr"
    mkdir -p "$lib_dir"
    cp -a "$asr/vosk-linux-x86_64-0.3.45/." "$lib_dir/"
    rm -rf "$asr/vosk-linux-x86_64-0.3.45" "$asr/vosk-linux-x86_64-0.3.45.zip"
    if [[ -f "$lib_dir/libvosk.so" ]]; then
      write_stamp "$stamps/libvosk.sha256" "$VOSK_LIB_SHA256"
      echo "ok   $lib_dir/libvosk.so"
    else
      echo "samantha: libvosk archive did not contain libvosk.so" >&2
      return 1
    fi
  fi

  if [[ -x "$piper_dir/piper" ]] && stamp_ok "$stamps/piper-runtime.sha256" "$PIPER_RT_SHA256"; then
    echo "ok   $piper_dir/piper"
  elif [[ "${CHECK_ONLY:-0}" == 1 ]]; then
    echo "MISSING/BAD $piper_dir/piper" >&2
    rc=1
  elif [[ "${DRY_RUN:-0}" == 1 ]]; then
    echo "dry-run: would fetch $PIPER_RT_URL"
    echo "dry-run: sha256 $PIPER_RT_SHA256"
  else
    command -v tar >/dev/null 2>&1 || die "tar is required for the Piper runtime" 2
    fetch_checked "$PIPER_RT_URL" "$data/models/runtime/piper_linux_x86_64.tar.gz" "$PIPER_RT_SHA256" || return 1
    mkdir -p "$data/models/runtime"
    tar xzf "$data/models/runtime/piper_linux_x86_64.tar.gz" -C "$data/models/runtime"
    rm -f "$data/models/runtime/piper_linux_x86_64.tar.gz"
    if [[ -x "$piper_dir/piper" ]]; then
      write_stamp "$stamps/piper-runtime.sha256" "$PIPER_RT_SHA256"
      echo "ok   $piper_dir/piper"
    else
      echo "samantha: Piper archive did not contain an executable piper" >&2
      return 1
    fi
  fi

  if [[ "$rc" -eq 0 ]]; then
    echo "MODELS=PRESENT"
  else
    echo "MODELS=ABSENT"
  fi
  return "$rc"
}

smoke_piper() {
  local data=$1
  local piper="$data/models/runtime/piper/piper"
  local model="$data/models/tts/en_US-ljspeech-medium.onnx"
  if [[ "${DRY_RUN:-0}" == 1 ]]; then
    echo "dry-run: would smoke-test the Piper runtime and delete the raw PCM"
    return 0
  fi
  if [[ "${CHECK_ONLY:-0}" == 1 ]]; then
    return 0
  fi
  [[ -x "$piper" && -f "$model" ]] || return 0
  local raw
  raw=$(mktemp "${TMPDIR:-/tmp}/samantha-piper-smoke.XXXXXX")
  local t0 t1 n
  t0=$(date +%s%N)
  if ! echo "hello galion" | "$piper" --quiet --model "$model" --noise_scale 0 --noise_w 0 --output_raw >"$raw" 2>/dev/null; then
    rm -f "$raw"
    echo "samantha: Piper smoke test failed" >&2
    return 1
  fi
  t1=$(date +%s%N)
  n=$(wc -c <"$raw" | tr -d ' ')
  rm -f "$raw"
  echo "smoke: ${n} B raw PCM in $(( (t1 - t0) / 1000000 )) ms (CPU, deleted)"
  if [[ "$n" -le 1000 ]]; then
    echo "samantha: Piper smoke PCM was too small" >&2
    return 1
  fi
  return 0
}
