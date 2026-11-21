#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Offline tests for the user install. No public model download.

set -euo pipefail

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
REPO=$(CDPATH='' cd -- "$ROOT/../.." && pwd)
# shellcheck source=lib/models.sh
source "$ROOT/lib/models.sh"

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${HTTP_PID:-}" ]]; then
    kill "$HTTP_PID" >/dev/null 2>&1 || true
    wait "$HTTP_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${WORK:-}" && -d "$WORK" ]]; then
    rm -rf "$WORK"
  fi
}
trap cleanup EXIT

WORK=$(mktemp -d)
pins="$ROOT/share/models.pins"

for sha in \
  30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498 \
  bbdc8ed85c43979f6443142889770ea95cbfbc56cffb5c5dcd73afa875c5fbb2 \
  a50cb45f355b7af1f6d758c1b360717877ba0a398cc8cbe6d2a7a3a26e225992 \
  6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22 \
  141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860
do
  grep -q "$sha" "$pins" || fail "pin file missing $sha"
  if ! grep -q "$sha" "$REPO/scripts/asr/fetch-vosk.sh" && ! grep -q "$sha" "$REPO/scripts/a21/fetch-a21.sh"; then
    fail "sha $sha is not in the existing fetch scripts"
  fi
done

fake="$WORK/fakebin"
mkdir -p "$fake"
cat >"$fake/curl" <<EOF
#!/bin/sh
echo called >>"$WORK/curl.log"
exit 99
EOF
chmod 0755 "$fake/curl"

dry_home="$WORK/dry-home"
PATH="$fake:$PATH" "$ROOT/install.sh" --dry-run --prefix "$dry_home" >"$WORK/dry.out"
[[ ! -e "$dry_home" ]] || fail "dry-run created $dry_home"
[[ ! -e "$WORK/curl.log" ]] || fail "dry-run invoked curl"
grep -q 'SAMANTHA_INSTALL=DRY_RUN' "$WORK/dry.out" || fail "dry-run summary missing"
grep -q 'https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip' "$WORK/dry.out" || fail "dry-run omitted the Vosk URL"
grep -q '30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498' "$WORK/dry.out" || fail "dry-run omitted the Vosk sha256"

set +e
"$ROOT/install.sh" --dry-run --prefix /etc >"$WORK/etc.out" 2>"$WORK/etc.err"
etc_rc=$?
set -e
[[ "$etc_rc" -eq 2 ]] || fail "prefix /etc should exit 2, got $etc_rc"

if [[ ! -x "$ROOT/target/debug/galion-samantha" ]]; then
  cargo build --manifest-path "$ROOT/Cargo.toml"
fi
bin_src="$ROOT/target/debug/galion-samantha"
[[ -x "$bin_src" ]] || fail "debug binary missing"

prefix="$WORK/home"
GALION_SAMANTHA_BIN=$bin_src "$ROOT/install.sh" --prefix "$prefix" --skip-models >"$WORK/inst.out"
grep -q 'SAMANTHA_INSTALL=OK' "$WORK/inst.out" || fail "install did not report OK"
grep -q 'prefix install: systemctl --user skipped' "$WORK/inst.out" || fail "prefix install called systemctl"
[[ -x "$prefix/.local/bin/galion-samantha" ]] || fail "binary not installed"
[[ -f "$prefix/.config/systemd/user/galion-samantha.service" ]] || fail "unit not installed"
[[ -f "$prefix/.local/share/applications/galion-samantha.desktop" ]] || fail "desktop file not installed"

GALION_SAMANTHA_BIN=$bin_src "$ROOT/install.sh" --prefix "$prefix" --skip-models >"$WORK/inst2.out"
grep -q 'unchanged '"$prefix/.local/bin/galion-samantha" "$WORK/inst2.out" || fail "second install was not idempotent"
grep -q 'SAMANTHA_INSTALL=OK' "$WORK/inst2.out" || fail "second install failed"

GALION_SAMANTHA_BIN=$bin_src "$ROOT/install.sh" --check --prefix "$prefix" --skip-models >"$WORK/check.out"
grep -q 'SAMANTHA_INSTALL=CHECKED' "$WORK/check.out" || fail "check failed"

if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate "$prefix/.local/share/applications/galion-samantha.desktop"
fi

unit="$prefix/.config/systemd/user/galion-samantha.service"
grep -q '^NoNewPrivileges=yes$' "$unit" || fail "unit missing NoNewPrivileges"
grep -q '^ProtectSystem=strict$' "$unit" || fail "unit missing ProtectSystem"
grep -q '^ProtectHome=yes$' "$unit" || fail "unit missing ProtectHome"
grep -q '^IPAddressAllow=localhost$' "$unit" || fail "unit missing IPAddressAllow"
grep -q '0.0.0.0' "$unit" && fail "unit mentions 0.0.0.0"

data="$prefix/.local/share/galion/samantha"
GALION_SAMANTHA_HOME=$data "$prefix/.local/bin/galion-samantha" check --allow-missing >"$WORK/check-bin.out"
grep -q 'horbm":"unlinked' "$WORK/check-bin.out" || fail "check did not report horbm unlinked"
grep -q 'public_models_ready=no' "$WORK/check-bin.out" || fail "models should be absent"

ref="$WORK/local-ref.txt"
printf 'not audio\n' >"$ref"
GALION_SAMANTHA_BIN=$bin_src GALION_SAMANTHA_REF=$ref \
  "$ROOT/install.sh" --prefix "$prefix" --skip-models >"$WORK/ref.out"
dropin="$prefix/.config/systemd/user/galion-samantha.service.d/reference.conf"
[[ -f "$dropin" ]] || fail "reference drop-in missing"
grep -q "GALION_SAMANTHA_REF=$ref" "$dropin" || fail "drop-in missing the reference path"
# The reference file stays where it was. The install must not copy it.
find "$prefix" -type f -name 'local-ref.txt' | grep -q . && fail "install copied the reference file"

set +e
# The single quotes are the point: the installer must see a literal dollar.
# shellcheck disable=SC2016
GALION_SAMANTHA_REF='/tmp/$(echo owned)' \
GALION_SAMANTHA_BIN=$bin_src \
  "$ROOT/install.sh" --prefix "$prefix" --skip-models >"$WORK/badref.out" 2>"$WORK/badref.err"
bad_rc=$?
set -e
[[ "$bad_rc" -eq 2 ]] || fail "dangerous reference path should exit 2, got $bad_rc"

log="$WORK/serve.log"
GALION_SAMANTHA_HOME=$data GALION_SAMANTHA_WS=127.0.0.1:0 GALION_SAMANTHA_REF=$ref \
  "$prefix/.local/bin/galion-samantha" serve >"$log" 2>&1 &
SERVER_PID=$!
addr=""
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  if grep -q 'listening on ' "$log"; then
    addr=$(sed -n 's/.*listening on \([^ ]*\).*/\1/p' "$log" | head -n 1)
    break
  fi
  sleep 0.05
done
[[ -n "$addr" ]] || fail "server did not listen: $(cat "$log")"
health=$(curl -sf "http://$addr/health")
printf '%s\n' "$health" | grep -q '"tick_ms":80' || fail "health missing tick"
printf '%s\n' "$health" | grep -q '"horbm":"unlinked"' || fail "health linked a brain"
printf '%s\n' "$health" | grep -q '"reference":"set"' || fail "health did not see the reference"
printf '%s\n' "$health" | grep -q "$ref" && fail "health leaked the reference path"
sleep 0.45
dot=$(curl -sf "http://$addr/dot")
ticks=$(printf '%s\n' "$dot" | sed -n 's/.*"ticks":\([0-9][0-9]*\).*/\1/p')
[[ -n "$ticks" && "$ticks" -ge 3 ]] || fail "tick did not advance (ticks=${ticks:-none})"
page=$(curl -sf "http://$addr/")
printf '%s\n' "$page" | grep -q '#F5F5F7' || fail "page missing the dot fill"
printf '%s\n' "$page" | grep -qi '<button' && fail "page has a button"
turn=$(curl -s -o "$WORK/turn.body" -w '%{http_code}' -X POST "http://$addr/turn")
[[ "$turn" == "501" ]] || fail "POST /turn returned $turn"
grep -q 'reply_engine_unlinked' "$WORK/turn.body" || fail "turn invented a reply"
kill "$SERVER_PID"
wait "$SERVER_PID" || true
SERVER_PID=""

set +e
GALION_SAMANTHA_HOME=$data GALION_SAMANTHA_WS=0.0.0.0:7881 \
  "$prefix/.local/bin/galion-samantha" serve >"$WORK/public.out" 2>"$WORK/public.err"
pub_rc=$?
set -e
[[ "$pub_rc" -eq 2 ]] || fail "public bind should exit 2, got $pub_rc"

blob_dir="$WORK/blobs"
mkdir -p "$blob_dir"
printf 'not-the-model' >"$blob_dir/blob"
HTTP_PORT=18765
python3 -m http.server --bind 127.0.0.1 "$HTTP_PORT" --directory "$blob_dir" >"$WORK/http.log" 2>&1 &
HTTP_PID=$!
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  if curl -sf -o /dev/null "http://127.0.0.1:$HTTP_PORT/blob"; then
    break
  fi
  sleep 0.05
done
bad_dest="$WORK/bad-download"
set +e
fetch_checked "http://127.0.0.1:$HTTP_PORT/blob" "$bad_dest" "0000000000000000000000000000000000000000000000000000000000000000"
fetch_rc=$?
set -e
[[ "$fetch_rc" -eq 1 ]] || fail "sha mismatch should return 1, got $fetch_rc"
[[ ! -e "$bad_dest" && ! -e "$bad_dest.part" ]] || fail "sha mismatch left a file behind"

good_sha=$(sha256sum "$blob_dir/blob" | awk '{print $1}')
good_dest="$WORK/good-download"
fetch_checked "http://127.0.0.1:$HTTP_PORT/blob" "$good_dest" "$good_sha"
fetch_checked "http://127.0.0.1:$HTTP_PORT/blob" "$good_dest" "$good_sha" >"$WORK/second-fetch.out"
grep -q "ok   $good_dest" "$WORK/second-fetch.out" || fail "second fetch was not a hash hit"
kill "$HTTP_PID"
wait "$HTTP_PID" || true
HTTP_PID=""

"$ROOT/uninstall.sh" --dry-run --prefix "$prefix" >"$WORK/un-dry.out"
[[ -x "$prefix/.local/bin/galion-samantha" ]] || fail "uninstall dry-run removed the binary"
grep -q 'SAMANTHA_UNINSTALL=DRY_RUN' "$WORK/un-dry.out" || fail "uninstall dry-run summary missing"
"$ROOT/uninstall.sh" --prefix "$prefix" >"$WORK/un.out"
[[ ! -e "$prefix/.local/bin/galion-samantha" ]] || fail "uninstall left the binary"
[[ -d "$data" ]] || fail "uninstall removed models without --purge-models"
grep -q 'SAMANTHA_UNINSTALL=OK' "$WORK/un.out" || fail "uninstall summary missing"

found=$(find "$ROOT" -type f \( -name '*.wav' -o -name '*.mp3' -o -name '*.flac' -o -name '*.ogg' -o -name '*.opus' \) | wc -l | tr -d ' ')
[[ "$found" == "0" ]] || fail "package tree contains audio files"

echo "OK: install tests"
