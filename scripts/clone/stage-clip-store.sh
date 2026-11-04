#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Copy gitignored persona WAVs into $GALION_CLIP_STORE, outside the work tree.
# WAVs only. Writes reference.wav.sha256 from the bytes. Does not fetch.
# Does not commit. Does not trim. An mp3 is a refusal.
#
#   GALION_CLIP_STORE=/absolute/outside/path scripts/clone/stage-clip-store.sh
#
# Exit: 0 staged or nothing to copy · 2 store or clearance refused · 3 not a WAV
set -euo pipefail
cd "$(dirname "$0")/../.."
REPO="$PWD"
STORE="${GALION_CLIP_STORE:-}"
REFS="${GALION_CLIP_REFS:-$REPO/docs/galion-voice-v2/personas/refs}"
REC="${GALION_CLIP_CLEARANCE:-$REPO/docs/galion-voice-v2/symphony/clearance-t273.rec}"

if [ -z "$STORE" ]; then
  echo "stage-clip-store: refusing; set GALION_CLIP_STORE to an absolute path outside the work tree." >&2
  exit 2
fi
case "$STORE" in
  /*) ;;
  *) echo "stage-clip-store: refusing; GALION_CLIP_STORE must be absolute." >&2; exit 2 ;;
esac

repo_real="$(pwd -P)"
mkdir -p "$STORE"
store_real="$(cd "$STORE" && pwd -P)"
case "$store_real" in
  "$repo_real"|"$repo_real"/*)
    echo "stage-clip-store: refusing; the clip store must sit outside the git work tree." >&2
    exit 2
    ;;
esac

command -v sha256sum >/dev/null || { echo "stage-clip-store: sha256sum required" >&2; exit 2; }

wav_header() {
  # Twelve bytes as hex so a null in the size field is not truncated.
  # RIFF is 52494646. WAVE is 57415645. A renamed mp3 fails this.
  local f="$1"
  local magic
  magic="$(od -An -t x1 -N 12 "$f" | tr -d ' \n')"
  [ "${#magic}" -eq 24 ] || return 1
  [ "${magic:0:8}" = "52494646" ] || return 1
  [ "${magic:16:8}" = "57415645" ] || return 1
}

clearance_ok() {
  [ -f "$REC" ] || return 1
  grep -q '^decision=t273$' "$REC" || return 1
  grep -q '^owner=Maciej Grajczyk$' "$REC" || return 1
  grep -q '^send_reference=yes$' "$REC" || return 1
  grep -q '^public_demo=no$' "$REC" || return 1
  grep -q '^private_cleared=yes$' "$REC" || return 1
  grep -q '^wav_only=yes$' "$REC" || return 1
  grep -q 'Samantha · PRIVATE CLEARED' "$REPO/LEDGER.md" || return 1
  grep -q 'Jarvis · PRIVATE CLEARED' "$REPO/LEDGER.md" || return 1
  grep -q '^lock_edited=no$' "$REC" || return 1
  # Empty counsel is the recorded state. A non-empty value is a human's
  # later edit. A missing key is a refusal. This script invents no name.
  grep -q -E '^counsel=.*$' "$REC" || return 1
}

need_clearance=0
for persona in samantha jarvis; do
  for ext in mp3 flac ogg opus m4a aac mp4 mkv mov webm wave; do
    if [ -e "$REFS/$persona.$ext" ]; then
      echo "stage-clip-store: refusing; $persona.$ext is not a WAV." >&2
      exit 3
    fi
  done
  if [ -f "$REFS/$persona.wav" ]; then
    need_clearance=1
  fi
done

if [ "$need_clearance" -eq 1 ]; then
  if ! clearance_ok; then
    echo "stage-clip-store: refusing; clearance t273 is not recorded ($REC)." >&2
    exit 2
  fi
fi

staged=0
for persona in samantha jarvis; do
  src="$REFS/$persona.wav"
  if [ ! -f "$src" ]; then
    echo "absent $persona"
    continue
  fi
  if ! wav_header "$src"; then
    echo "stage-clip-store: refusing; $persona.wav is not a RIFF WAVE." >&2
    exit 3
  fi
  dest="$store_real/$persona"
  mkdir -p "$dest"
  cp -f "$src" "$dest/reference.wav"
  sha="$(sha256sum "$dest/reference.wav" | awk '{print $1}')"
  printf '%s  reference.wav\n' "$sha" > "$dest/reference.wav.sha256"
  echo "staged $persona $sha"
  staged=1
done

if [ "$staged" -eq 0 ]; then
  echo "stage-clip-store: no landing WAV to copy."
fi
exit 0
