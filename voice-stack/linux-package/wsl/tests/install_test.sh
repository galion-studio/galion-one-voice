#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Offline tests for the WSL installer. No model download. No deploy.

set -euo pipefail

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
REPO=$(CDPATH='' cd -- "$ROOT/../../.." && pwd)

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

WORK=$(mktemp -d)
SERVER_PID=""
cleanup() {
  if [[ -n "${SERVER_PID}" ]]; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" >/dev/null 2>&1 || true
  fi
  rm -rf "$WORK"
}
trap cleanup EXIT

pin="$ROOT/share/silero.pin"
sources="$REPO/voice-stack/vad/scripts/sources.tsv"
sha=2623a2953f6ff3d2c1e61740c6cdb7168133479b267dfef114a4a3cc5bdd788f
url=https://raw.githubusercontent.com/snakers4/silero-vad/v5.1.2/src/silero_vad/data/silero_vad.onnx
grep -q "$sha" "$pin" || fail "silero pin missing the hash"
grep -q "$url" "$pin" || fail "silero pin missing the url"
grep -q "$sha" "$sources" || fail "silero hash drifted from sources.tsv"
grep -q "$url" "$sources" || fail "silero url drifted from sources.tsv"
grep "$sha" "$sources" | grep -q 'silero_vad\.onnx' || fail "silero hash is not the onnx line"
if grep "$sha" "$sources" | grep -Eq '\.wav|\.mp3|\.flac'; then
  fail "silero hash is attached to a clip"
fi

vosk_sha=30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498
piper_sha=6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22
grep -q "$vosk_sha" "$ROOT/../share/models.pins" || fail "vosk pin missing"
grep -q "$piper_sha" "$ROOT/../share/models.pins" || fail "piper pin missing"
grep -q "$vosk_sha" "$REPO/scripts/asr/fetch-vosk.sh" || fail "vosk hash drifted"
grep -q "$piper_sha" "$REPO/scripts/a21/fetch-a21.sh" || fail "piper hash drifted"

fake="$WORK/fakebin"
mkdir -p "$fake"
cat >"$fake/curl" <<EOF
#!/bin/sh
echo called >>"$WORK/curl.log"
exit 99
EOF
chmod 0755 "$fake/curl"

dry="$WORK/dry-home"
dry_var="$WORK/dry-var"
PATH="$fake:$PATH" \
  env -u PULSE_SERVER -u XDG_RUNTIME_DIR \
  GALION_WSLG_PULSE=skip GALION_VOICE_VAR="$dry_var" \
  "$ROOT/install.sh" --dry-run --prefix "$dry" >"$WORK/dry.out"
[[ ! -e "$dry" ]] || fail "dry-run created $dry"
[[ ! -e "$dry_var" ]] || fail "dry-run created $dry_var"
[[ ! -e "$WORK/curl.log" ]] || fail "dry-run invoked curl"
[[ ! -e "$ROOT/var" ]] || fail "dry-run created the repo var directory"
grep -q 'GALION_WSL_INSTALL=DRY_RUN' "$WORK/dry.out" || fail "dry-run summary missing"
grep -q 'WSL_AUDIO=SKIPPED' "$WORK/dry.out" || fail "dry-run did not skip audio"
grep -q 'https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip' "$WORK/dry.out" || fail "dry-run omitted Vosk"
grep -q "$vosk_sha" "$WORK/dry.out" || fail "dry-run omitted the Vosk sha256"
grep -q "$url" "$WORK/dry.out" || fail "dry-run omitted Silero"
grep -q 'en_US-ljspeech-medium.onnx' "$WORK/dry.out" || fail "dry-run omitted Piper LJ Speech"
grep -q 'cargo build --release --locked' "$WORK/dry.out" || fail "dry-run omitted the CPU build"
if grep -q 'upload.wikimedia.org\|\.wav' "$WORK/dry.out"; then
  fail "dry-run mentioned a speech clip"
fi

set +e
"$ROOT/install.sh" --dry-run --prefix /etc >"$WORK/etc.out" 2>"$WORK/etc.err"
etc_rc=$?
set -e
[[ "$etc_rc" -eq 2 ]] || fail "prefix /etc should exit 2, got $etc_rc"

set +e
GALION_VOICE_VAR="$REPO/not-the-wsl-var" \
  "$ROOT/install.sh" --dry-run --prefix "$WORK/badvar" >"$WORK/badvar.out" 2>"$WORK/badvar.err"
badvar_rc=$?
set -e
[[ "$badvar_rc" -eq 2 ]] || fail "repo model dir should exit 2, got $badvar_rc"

set +e
GALION_WSLG_PULSE=skip GALION_VOICE_VAR="$WORK/req-var" \
  "$ROOT/install.sh" --prefix "$WORK/req" --skip-build --skip-models --require-audio \
  >"$WORK/req.out" 2>"$WORK/req.err"
req_rc=$?
set -e
[[ "$req_rc" -eq 1 ]] || fail "--require-audio should exit 1 when audio is skipped, got $req_rc"
[[ ! -e "$WORK/req/.local/bin/galion-one-voice" ]] || fail "--require-audio wrote files after the audio failure"

prefix="$WORK/home"
var="$WORK/var"
env -u PULSE_SERVER -u XDG_RUNTIME_DIR \
  GALION_WSLG_PULSE=skip GALION_VOICE_VAR="$var" \
  "$ROOT/install.sh" --prefix "$prefix" --skip-build --skip-models >"$WORK/inst.out"
grep -q 'GALION_WSL_INSTALL=OK' "$WORK/inst.out" || fail "install did not report OK"
grep -q 'WSL_AUDIO=SKIPPED' "$WORK/inst.out" || fail "install did not skip audio"
grep -q 'prefix install: systemctl --user skipped' "$WORK/inst.out" || fail "prefix install called systemctl"
[[ -x "$prefix/.local/bin/galion-one-voice" ]] || fail "launcher not installed"
[[ -f "$prefix/.config/systemd/user/galion-one-voice.service" ]] || fail "unit not installed"
[[ ! -e "$prefix/.asoundrc" ]] || fail "install wrote ~/.asoundrc"
[[ ! -e "$prefix/.config/galion/one-voice.asoundrc" ]] || fail "skip path wrote an asoundrc"
unit="$prefix/.config/systemd/user/galion-one-voice.service"
grep -q '^Environment=GALION_SAMANTHA_WS=127\.0\.0\.1:7881$' "$unit" || fail "unit bind"
grep -q '^NoNewPrivileges=yes$' "$unit" || fail "unit sandbox"
grep -q '^IPAddressAllow=localhost$' "$unit" || fail "unit localhost allow"
if grep -q '0\.0\.0\.0' "$unit"; then
  fail "unit mentions 0.0.0.0"
fi
envf="$prefix/.config/galion/one-voice.env"
grep -q "^GALION_SILERO_VAD=$var/models/vad/silero_vad.onnx$" "$envf" || fail "env missing silero"
grep -q "^GALION_VOSK_MODEL=$var/models/asr/vosk-model-small-en-us-0.15$" "$envf" || fail "env missing vosk"
grep -q "^GALION_A21_MODEL=$var/models/tts/en_US-ljspeech-medium.onnx$" "$envf" || fail "env missing piper"
grep -q '^GALION_SAMANTHA_WS=127\.0\.0\.1:7881$' "$envf" || fail "env bind"
if grep -q 'PULSE_SERVER=' "$envf"; then
  fail "skipped audio still set PULSE_SERVER"
fi
grep -q '# BEGIN galion-one-voice' "$prefix/.bashrc" || fail "bashrc block missing"
dropin="$prefix/.config/systemd/user/galion-one-voice.service.d/paths.conf"
grep -q "^ReadOnlyPaths=$var$" "$dropin" || fail "drop-in missing var"
if grep -q '0\.0\.0\.0' "$dropin" "$envf"; then
  fail "generated file mentions 0.0.0.0"
fi

env -u PULSE_SERVER -u XDG_RUNTIME_DIR \
  GALION_WSLG_PULSE=skip GALION_VOICE_VAR="$var" \
  "$ROOT/install.sh" --prefix "$prefix" --skip-build --skip-models >"$WORK/inst2.out"
grep -q "unchanged $unit" "$WORK/inst2.out" || fail "second install was not idempotent"
grep -q "unchanged $prefix/.local/bin/galion-one-voice" "$WORK/inst2.out" || fail "launcher was rewritten"
grep -q 'GALION_WSL_INSTALL=OK' "$WORK/inst2.out" || fail "second install failed"

env -u PULSE_SERVER -u XDG_RUNTIME_DIR \
  GALION_WSLG_PULSE=skip GALION_VOICE_VAR="$var" \
  "$ROOT/install.sh" --check --prefix "$prefix" --skip-build --skip-models >"$WORK/check.out"
grep -q 'GALION_WSL_INSTALL=CHECKED' "$WORK/check.out" || fail "check failed"

# Automatic discovery with no WSLg socket and no inherited Pulse server.
if [[ ! -S /mnt/wslg/PulseServer && ! -S /mnt/wslg/runtime-dir/pulse/native ]]; then
  auto="$WORK/auto"
  env -u PULSE_SERVER -u XDG_RUNTIME_DIR -u GALION_WSLG_PULSE \
    GALION_VOICE_VAR="$WORK/auto-var" \
    "$ROOT/install.sh" --prefix "$auto" --skip-build --skip-models >"$WORK/auto.out"
  grep -q 'WSL_AUDIO=SKIPPED' "$WORK/auto.out" || fail "auto discovery did not skip"
  grep -q 'GALION_WSL_INSTALL=OK' "$WORK/auto.out" || fail "auto discovery install failed"
fi

# A socket that is not a Pulse server still installs, and the audio check
# does not fail the run.
sockdir="$WORK/pulse"
mkdir -p "$sockdir"
python3 -c 'import os, socket, sys, time
path = sys.argv[1]
try:
    os.remove(path)
except FileNotFoundError:
    pass
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.bind(path)
s.listen(1)
time.sleep(30)
' "$sockdir/native" &
SERVER_PID=$!
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  [[ -S "$sockdir/native" ]] && break
  sleep 0.05
done
[[ -S "$sockdir/native" ]] || fail "test socket was not created"
pulse_prefix="$WORK/pulse-home"
env -u PULSE_SERVER -u XDG_RUNTIME_DIR \
  GALION_WSLG_PULSE="$sockdir/native" GALION_VOICE_VAR="$WORK/pulse-var" \
  "$ROOT/install.sh" --prefix "$pulse_prefix" --skip-build --skip-models >"$WORK/pulse.out"
grep -E -q 'WSL_AUDIO=(OK|SOCKET|UNREACHABLE)' "$WORK/pulse.out" || fail "socket was ignored: $(cat "$WORK/pulse.out")"
grep -q 'GALION_WSL_INSTALL=OK' "$WORK/pulse.out" || fail "socket install failed"
asound="$pulse_prefix/.config/galion/one-voice.asoundrc"
[[ -f "$asound" ]] || fail "asoundrc missing"
grep -q 'type pulse' "$asound" || fail "asoundrc is not pulse"
grep -q "unix:${sockdir}/native" "$asound" || fail "asoundrc omitted the socket"
grep -q "^PULSE_SERVER=unix:${sockdir}/native$" "$pulse_prefix/.config/galion/one-voice.env" || fail "env omitted PULSE_SERVER"
[[ ! -e "$pulse_prefix/.asoundrc" ]] || fail "socket install wrote ~/.asoundrc"
env -u PULSE_SERVER -u XDG_RUNTIME_DIR \
  GALION_WSLG_PULSE="$sockdir/native" GALION_VOICE_VAR="$WORK/pulse-var" \
  "$ROOT/install.sh" --prefix "$pulse_prefix" --skip-build --skip-models >"$WORK/pulse2.out"
grep -q "unchanged $asound" "$WORK/pulse2.out" || fail "asoundrc was not idempotent"
# --skip-audio wins over a live socket.
skip_prefix="$WORK/skip-home"
env -u PULSE_SERVER -u XDG_RUNTIME_DIR \
  GALION_WSLG_PULSE="$sockdir/native" GALION_VOICE_VAR="$WORK/skip-var" \
  "$ROOT/install.sh" --skip-audio --prefix "$skip_prefix" --skip-build --skip-models >"$WORK/skip.out"
grep -q 'WSL_AUDIO=SKIPPED' "$WORK/skip.out" || fail "--skip-audio did not skip"
[[ ! -e "$skip_prefix/.config/galion/one-voice.asoundrc" ]] || fail "--skip-audio wrote an asoundrc"
kill "$SERVER_PID" >/dev/null 2>&1 || true
wait "$SERVER_PID" >/dev/null 2>&1 || true
SERVER_PID=""

launcher="$ROOT/share/galion-one-voice"
set +e
env -u GALION_ONE_VOICE_ENV HOME="$WORK/empty-home" \
  GALION_SAMANTHA_WS=0.0.0.0:7881 \
  "$launcher" >"$WORK/pub.out" 2>"$WORK/pub.err"
pub_rc=$?
set -e
[[ "$pub_rc" -eq 2 ]] || fail "public bind should exit 2, got $pub_rc"
mkdir -p "$WORK/empty-home"
helper="$WORK/helper.sh"
cat >"$helper" <<'EOF'
#!/bin/sh
echo "helper-ran"
EOF
chmod 0755 "$helper"
env -u GALION_ONE_VOICE_ENV HOME="$WORK/empty-home" \
  GALION_SAMANTHA_WS=127.0.0.1:7881 \
  GALION_ONE_VOICE_BIN="$helper" \
  "$launcher" >"$WORK/helper.out"
grep -q 'helper-ran' "$WORK/helper.out" || fail "override binary did not run"

"$ROOT/ci/verify-unit.sh" >"$WORK/verify.out"
grep -q 'OK: galion-one-voice.service' "$WORK/verify.out" || fail "unit verify failed"

found=$(find "$ROOT" -type f \( -name '*.wav' -o -name '*.mp3' -o -name '*.flac' -o -name '*.ogg' -o -name '*.opus' \) | wc -l | tr -d ' ')
[[ "$found" == "0" ]] || fail "wsl tree contains audio files"

echo "OK: wsl install tests"
