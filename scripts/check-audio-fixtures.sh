#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
# Fail when a tracked wav, mp3, opus, flac, ogg, or wave file is larger than
# the tiny fixture ceiling, unless that exact blob is already recorded.
# The record is frozen: adding a line fails this check.
set -euo pipefail
cd "$(dirname "$0")/.."

MAX=4096
LEGACY=scripts/audio-legacy.sha256
# Count of entries when the guard landed. Growth fails. Shrink is a deletion.
LEGACY_MAX=84

fail=0
for pat in '*.wav' '*.flac' '*.mp3' '*.opus' '*.ogg'; do
  if ! grep -q -x -F "$pat" .gitignore; then
    echo "FAIL: .gitignore is missing a line that is exactly ${pat}"
    fail=1
  fi
done
for pat in 'tools/local-studio/out/' 'studio-out/'; do
  if ! grep -q -x -F "$pat" .gitignore; then
    echo "FAIL: .gitignore is missing ${pat}"
    fail=1
  fi
done

if [[ ! -f "$LEGACY" ]]; then
  echo "FAIL: missing ${LEGACY}"
  exit 1
fi

declare -A ALLOW
legacy_count=0
while IFS=$'\t' read -r hash size path || [[ -n "${hash:-}" ]]; do
  [[ -z "${hash:-}" ]] && continue
  [[ "$hash" == \#* ]] && continue
  legacy_count=$((legacy_count + 1))
  if [[ "$path" == tools/local-studio/* || "$path" == studio-out/* ]]; then
    echo "FAIL: legacy list must not grandfather studio output: ${path}"
    fail=1
  fi
  ALLOW["$path"]="${hash} ${size}"
done < "$LEGACY"

if (( legacy_count > LEGACY_MAX )); then
  echo "FAIL: ${LEGACY} has ${legacy_count} entries. The frozen ceiling is ${LEGACY_MAX}."
  fail=1
fi

is_audio() {
  local lower="${1,,}"
  case "$lower" in
    *.wav|*.wave|*.mp3|*.opus|*.flac|*.ogg) return 0 ;;
    *) return 1 ;;
  esac
}

while IFS= read -r -d '' path; do
  is_audio "$path" || continue
  [[ -f "$path" ]] || continue
  size=$(wc -c < "$path" | tr -d '[:space:]')
  if (( size <= MAX )); then
    continue
  fi
  hash=$(sha256sum -- "$path" | awk 'NR==1 { print $1 }')
  expected="${ALLOW[$path]:-}"
  if [[ -z "$expected" ]]; then
    echo "FAIL: ${path} is ${size} bytes, above ${MAX}. Reference clips and renders stay out of git."
    fail=1
    continue
  fi
  want_hash="${expected%% *}"
  want_size="${expected##* }"
  if [[ "$hash" != "$want_hash" || "$size" != "$want_size" ]]; then
    echo "FAIL: ${path} no longer matches the legacy fixture record."
    fail=1
  fi
done < <(git ls-files -z)

if (( fail != 0 )); then
  echo "Audio fixture guard failed. Ceiling is ${MAX} bytes."
  exit 1
fi
echo "OK: tracked audio above ${MAX} bytes matches the frozen legacy list."
