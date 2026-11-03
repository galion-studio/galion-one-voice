#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Local checks for the clone-weight fetch. No GitHub upload. No 4.6 GiB blob.
set -euo pipefail
cd "$(dirname "$0")/../.."
FETCH="$PWD/scripts/clone/fetch-clone-weights.sh"
PUB="$PWD/scripts/clone/publish-clone-weights.sh"
ROOT="$PWD"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail() {
  echo "FAIL: $*" >&2
  echo "--- stdout ---" >&2
  cat "$tmp/out" >&2 || true
  echo "--- stderr ---" >&2
  cat "$tmp/err" >&2 || true
  exit 1
}

expect_rc() {
  local want=$1
  shift
  set +e
  "$@" >"$tmp/out" 2>"$tmp/err"
  local rc=$?
  set -e
  if [ "$rc" != "$want" ]; then
    fail "want exit $want got $rc: $*"
  fi
}

# Production pins: five safetensors, exact release names, no audio rows.
rows=$(awk 'NF && $1 !~ /^#/' "$ROOT/scripts/clone/PINS.sha256")
row_n=$(printf '%s\n' "$rows" | wc -l)
[ "$row_n" -eq 5 ] || fail "pin rows $row_n"
printf '%s\n' "$rows" | grep -q 'shared/s3gen.safetensors 2b78103c654207393955e4900aac14a12de8ef25f4b09424f1ef91941f161d4e 1056484620 ResembleAI/chatterbox-nano s3gen.safetensors galion-clone-shared-s3gen.safetensors' || fail "s3gen pin"
printf '%s\n' "$rows" | grep -q 'shared/s3gen_meanflow.safetensors d65cb687a2ed581ee6cc297e919ffefa63386944f42364ae13b78a594945514f 1064875036 ResembleAI/chatterbox-nano s3gen_meanflow.safetensors galion-clone-shared-s3gen_meanflow.safetensors' || fail "meanflow pin"
printf '%s\n' "$rows" | grep -q 'shared/ve.safetensors f0921cab452fa278bc25cd23ffd59d36f816d7dc5181dd1bef9751a7fb61f63c 5695784 ResembleAI/chatterbox-nano ve.safetensors galion-clone-shared-ve.safetensors' || fail "ve pin"
printf '%s\n' "$rows" | grep -q 'nano/t3_nano_v1.safetensors 72b110185087d945dbdf54dee4e333848e1811bdd5fd6cb16ceb8da50006f0c9 869899204 ResembleAI/chatterbox-nano t3_nano_v1.safetensors galion-clone-nano-t3_nano_v1.safetensors' || fail "nano pin"
printf '%s\n' "$rows" | grep -q 'turbo/t3_turbo_v1.safetensors fcf1f8c1d651bb7e3acd69ee5be269b4ac10c02980b7708213d598bc9f7cdf87 1915480052 ResembleAI/chatterbox-turbo t3_turbo_v1.safetensors galion-clone-turbo-t3_turbo_v1.safetensors' || fail "turbo pin"
printf '%s\n' "$rows" | grep -Ei 'wav|samantha|jarvis|\.pt' && fail "audio slipped into pin rows" || true
grep -q 'clone-weights-v1' "$ROOT/scripts/clone/PINS.sha256" || fail "tag missing from pins"
grep -q 'CLONE_RELEASE_TAG=clone-weights-v1' "$ROOT/scripts/clone/weights-release.sh" || fail "tag missing from release names"
tracked=$(git ls-files '*.safetensors')
[ -z "$tracked" ] || fail "tracked safetensors: $tracked"
extra=$(git status --porcelain -- weights docs/galion-voice-v2/personas/refs | grep -Ei 'safetensors|\.wav|\.pt' || true)
[ -z "$extra" ] || fail "weight or clip in the work tree: $extra"

expect_rc 2 bash "$ROOT/scripts/clone/weights-release.sh"
grep -q 'source this file' "$tmp/err" || fail "weights-release executed quietly"

expect_rc 2 env -u GALION_CLONE_FETCH "$FETCH"
grep -q 'GALION_CLONE_FETCH' "$tmp/err" || fail "env refusal text"
grep -qi 'http' "$tmp/err" && fail "env refusal leaked a url" || true

payload="$tmp/payload.bin"
printf 'galion-nano-weights-test-v1\n' > "$payload"
sha=$(sha256sum "$payload" | awk '{print $1}')
size=$(stat -c %s "$payload")
pins="$tmp/pins.sha256"
cat > "$pins" <<EOF
# test pins
shared/s3gen.safetensors $sha $size ResembleAI/chatterbox-nano s3gen.safetensors galion-clone-shared-s3gen.safetensors
shared/s3gen_meanflow.safetensors $sha $size ResembleAI/chatterbox-nano s3gen_meanflow.safetensors galion-clone-shared-s3gen_meanflow.safetensors
shared/ve.safetensors $sha $size ResembleAI/chatterbox-nano ve.safetensors galion-clone-shared-ve.safetensors
nano/t3_nano_v1.safetensors $sha $size ResembleAI/chatterbox-nano t3_nano_v1.safetensors galion-clone-nano-t3_nano_v1.safetensors
turbo/t3_turbo_v1.safetensors $sha $size ResembleAI/chatterbox-turbo t3_turbo_v1.safetensors galion-clone-turbo-t3_turbo_v1.safetensors
EOF

srv="$tmp/srv"
rel_dir="$srv/releases/download/clone-weights-v1"
mkdir -p "$rel_dir"
for asset in \
  galion-clone-shared-s3gen.safetensors \
  galion-clone-shared-s3gen_meanflow.safetensors \
  galion-clone-shared-ve.safetensors \
  galion-clone-nano-t3_nano_v1.safetensors \
  galion-clone-turbo-t3_turbo_v1.safetensors
do
  cp "$payload" "$rel_dir/$asset"
done
hf="$srv/ResembleAI/chatterbox-nano/resolve/main"
mkdir -p "$hf" "$srv/ResembleAI/chatterbox-turbo/resolve/main"
for name in s3gen.safetensors s3gen_meanflow.safetensors ve.safetensors t3_nano_v1.safetensors; do
  cp "$payload" "$hf/$name"
done
cp "$payload" "$srv/ResembleAI/chatterbox-turbo/resolve/main/t3_turbo_v1.safetensors"

port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')
python3 -m http.server "$port" --bind 127.0.0.1 --directory "$srv" >"$tmp/http.log" 2>&1 &
http_pid=$!
trap 'kill "$http_pid" 2>/dev/null || true; rm -rf "$tmp"' EXIT
ready=0
for _ in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20; do
  if curl -sf -o /dev/null "http://127.0.0.1:${port}/"; then
    ready=1
    break
  fi
  sleep 0.1
done
[ "$ready" = 1 ] || fail "local server did not start"

base="http://127.0.0.1:${port}"
dest="$tmp/weights"
expect_rc 0 env \
  GALION_CLONE_FETCH=1 \
  GALION_CLONE_PINS="$pins" \
  GALION_CLONE_WEIGHTS="$dest" \
  GALION_CLONE_RELEASE_BASE="$base" \
  "$FETCH" --nano
grep -q 'CLONE=PRESENT (nano safetensors only, not vendored)' "$tmp/out" || fail "nano present line"
[ -f "$dest/nano/t3_nano_v1.safetensors" ] || fail "nano file missing"
[ ! -e "$dest/turbo/t3_turbo_v1.safetensors" ] || fail "turbo fetched under --nano"
grep -q 'skip turbo/' "$tmp/out" || fail "turbo not skipped"
got=$(sha256sum "$dest/shared/ve.safetensors" | awk '{print $1}')
[ "$got" = "$sha" ] || fail "ve hash"

# Second fetch is a local hit. A fake curl must not run.
mkdir -p "$tmp/bin"
cat > "$tmp/bin/curl" <<'EOF'
#!/bin/sh
echo "curl should not run" >&2
exit 99
EOF
chmod +x "$tmp/bin/curl"
expect_rc 0 env \
  PATH="$tmp/bin:$PATH" \
  GALION_CLONE_FETCH=1 \
  GALION_CLONE_PINS="$pins" \
  GALION_CLONE_WEIGHTS="$dest" \
  GALION_CLONE_RELEASE_BASE="$base" \
  "$FETCH" --check --nano
grep -q 'curl should not run' "$tmp/err" && fail "check contacted the network" || true

rm -rf "$dest"
expect_rc 0 env \
  GALION_CLONE_FETCH=1 \
  GALION_CLONE_PINS="$pins" \
  GALION_CLONE_WEIGHTS="$dest" \
  GALION_CLONE_WEIGHTS_SOURCE=hf \
  GALION_CLONE_HF_BASE="$base" \
  "$FETCH" --nano
grep -q "get  ${base}/ResembleAI/chatterbox-nano/resolve/main/t3_nano_v1.safetensors" "$tmp/out" || fail "hf url"
[ ! -e "$dest/turbo/t3_turbo_v1.safetensors" ] || fail "hf turbo fetched under --nano"

rm -rf "$dest"
bad_pins="$tmp/bad-sha.sha256"
sed "s/$sha/0000000000000000000000000000000000000000000000000000000000000000/" "$pins" > "$bad_pins"
expect_rc 1 env \
  GALION_CLONE_FETCH=1 \
  GALION_CLONE_PINS="$bad_pins" \
  GALION_CLONE_WEIGHTS="$dest" \
  GALION_CLONE_RELEASE_BASE="$base" \
  "$FETCH" --nano
grep -q 'sha256 MISMATCH' "$tmp/err" || fail "mismatch text"
[ ! -e "$dest/shared/s3gen.safetensors" ] || fail "bad download kept"
[ ! -e "$dest/shared/s3gen.safetensors.part" ] || fail "partial kept"

rm -rf "$dest"
rm -f "$rel_dir/galion-clone-shared-s3gen.safetensors"
expect_rc 1 env \
  GALION_CLONE_FETCH=1 \
  GALION_CLONE_PINS="$pins" \
  GALION_CLONE_WEIGHTS="$dest" \
  GALION_CLONE_RELEASE_BASE="$base" \
  "$FETCH" --nano
grep -q 'BLOCKED galion-clone-shared-s3gen.safetensors HTTP 404' "$tmp/err" || fail "blocked text"
grep -q 'CLONE-WEIGHTS.md' "$tmp/err" || fail "blocked doc pointer"
[ ! -e "$dest/shared/s3gen.safetensors" ] || fail "404 left a file"
cp "$payload" "$rel_dir/galion-clone-shared-s3gen.safetensors"

wav_pins="$tmp/wav.sha256"
cat > "$wav_pins" <<EOF
samantha.wav $sha $size ResembleAI/chatterbox-nano ve.safetensors galion-clone-shared-ve.safetensors
shared/s3gen.safetensors $sha $size ResembleAI/chatterbox-nano s3gen.safetensors galion-clone-shared-s3gen.safetensors
shared/s3gen_meanflow.safetensors $sha $size ResembleAI/chatterbox-nano s3gen_meanflow.safetensors galion-clone-shared-s3gen_meanflow.safetensors
shared/ve.safetensors $sha $size ResembleAI/chatterbox-nano ve.safetensors galion-clone-shared-ve.safetensors
nano/t3_nano_v1.safetensors $sha $size ResembleAI/chatterbox-nano t3_nano_v1.safetensors galion-clone-nano-t3_nano_v1.safetensors
turbo/t3_turbo_v1.safetensors $sha $size ResembleAI/chatterbox-turbo t3_turbo_v1.safetensors galion-clone-turbo-t3_turbo_v1.safetensors
EOF
expect_rc 2 env GALION_CLONE_FETCH=1 GALION_CLONE_PINS="$wav_pins" GALION_CLONE_WEIGHTS="$tmp/unused" "$FETCH"
grep -q 'refusing samantha.wav' "$tmp/err" || fail "wav refusal"

cat > "$wav_pins" <<EOF
shared/s3gen.safetensors $sha $size ResembleAI/chatterbox-nano s3gen.safetensors galion-clone-samantha.safetensors
shared/s3gen_meanflow.safetensors $sha $size ResembleAI/chatterbox-nano s3gen_meanflow.safetensors galion-clone-shared-s3gen_meanflow.safetensors
shared/ve.safetensors $sha $size ResembleAI/chatterbox-nano ve.safetensors galion-clone-shared-ve.safetensors
nano/t3_nano_v1.safetensors $sha $size ResembleAI/chatterbox-nano t3_nano_v1.safetensors galion-clone-nano-t3_nano_v1.safetensors
turbo/t3_turbo_v1.safetensors $sha $size ResembleAI/chatterbox-turbo t3_turbo_v1.safetensors galion-clone-turbo-t3_turbo_v1.safetensors
EOF
expect_rc 2 env GALION_CLONE_FETCH=1 GALION_CLONE_PINS="$wav_pins" GALION_CLONE_WEIGHTS="$tmp/unused" "$FETCH" --nano
grep -q 'persona reference name' "$tmp/err" || fail "persona refusal"

cat > "$wav_pins" <<EOF
shared/s3gen.safetensors $sha 2147483648 ResembleAI/chatterbox-nano s3gen.safetensors galion-clone-shared-s3gen.safetensors
shared/s3gen_meanflow.safetensors $sha $size ResembleAI/chatterbox-nano s3gen_meanflow.safetensors galion-clone-shared-s3gen_meanflow.safetensors
shared/ve.safetensors $sha $size ResembleAI/chatterbox-nano ve.safetensors galion-clone-shared-ve.safetensors
nano/t3_nano_v1.safetensors $sha $size ResembleAI/chatterbox-nano t3_nano_v1.safetensors galion-clone-nano-t3_nano_v1.safetensors
turbo/t3_turbo_v1.safetensors $sha $size ResembleAI/chatterbox-turbo t3_turbo_v1.safetensors galion-clone-turbo-t3_turbo_v1.safetensors
EOF
expect_rc 2 env GALION_CLONE_FETCH=1 GALION_CLONE_PINS="$wav_pins" GALION_CLONE_WEIGHTS="$tmp/unused" "$FETCH" --nano
grep -q 'under 2 GiB' "$tmp/err" || fail "size cap"

empty="$tmp/empty-weights"
mkdir -p "$empty"
expect_rc 1 env \
  PATH="$tmp/bin:$PATH" \
  GALION_CLONE_FETCH=1 \
  GALION_CLONE_WEIGHTS="$empty" \
  "$FETCH" --check --nano
grep -q 'MISSING/BAD' "$tmp/err" || fail "check missing"
grep -q 'curl should not run' "$tmp/err" && fail "real check contacted the network" || true
grep -q 'CLONE=ABSENT' "$tmp/err" || fail "absent line"

expect_rc 2 env -u GALION_CLONE_PUBLISH "$PUB"
grep -q 'GALION_CLONE_PUBLISH' "$tmp/err" || fail "publish env gate"

expect_rc 2 env \
  -u GALION_CLONE_PINS -u GALION_CLONE_WEIGHTS -u GALION_CLONE_RELEASE_BASE \
  -u GALION_CLONE_WEIGHTS_SOURCE -u GALION_CLONE_HF_BASE \
  GALION_CLONE_PUBLISH=1 \
  "$PUB"
grep -q 'BLOCKED' "$tmp/err" || fail "publish blocked on a clean tree"

cat > "$tmp/bin/gh" <<'EOF'
#!/bin/sh
echo "gh should not run" >&2
exit 99
EOF
chmod +x "$tmp/bin/gh"
expect_rc 2 env \
  PATH="$tmp/bin:$PATH" \
  GALION_CLONE_PUBLISH=1 \
  GALION_CLONE_WEIGHTS="$tmp/do-not-publish" \
  "$PUB" --upload
grep -q 'refuses path overrides' "$tmp/err" || fail "upload override"
grep -q 'gh should not run' "$tmp/err" && fail "upload called gh" || true

pub_w="$tmp/pub-weights"
mkdir -p "$pub_w/shared" "$pub_w/nano" "$pub_w/turbo"
cp "$payload" "$pub_w/shared/s3gen.safetensors"
cp "$payload" "$pub_w/shared/s3gen_meanflow.safetensors"
cp "$payload" "$pub_w/shared/ve.safetensors"
cp "$payload" "$pub_w/nano/t3_nano_v1.safetensors"
cp "$payload" "$pub_w/turbo/t3_turbo_v1.safetensors"
expect_rc 0 env \
  PATH="$tmp/bin:$PATH" \
  GALION_CLONE_PUBLISH=1 \
  GALION_CLONE_PINS="$pins" \
  GALION_CLONE_WEIGHTS="$pub_w" \
  "$PUB"
grep -q 'gh release create clone-weights-v1' "$tmp/out" || fail "printed release"
grep -q -- '--latest=false' "$tmp/out" || fail "latest flag"
grep -q 'galion-clone-nano-t3_nano_v1.safetensors' "$tmp/out" || fail "nano asset"
grep -q 'galion-clone-turbo-t3_turbo_v1.safetensors' "$tmp/out" || fail "turbo asset"
grep -q 'verified' "$tmp/out" || fail "verify line"
grep -q 'gh should not run' "$tmp/err" && fail "print called gh" || true

printf 'RIFF' > "$pub_w/samantha.wav"
expect_rc 2 env \
  PATH="$tmp/bin:$PATH" \
  GALION_CLONE_PUBLISH=1 \
  GALION_CLONE_PINS="$pins" \
  GALION_CLONE_WEIGHTS="$pub_w" \
  "$PUB"
grep -q 'refusing samantha.wav' "$tmp/err" || fail "publish wav"
grep -q 'gh release create' "$tmp/out" && fail "wav still printed a release" || true

echo "fetch-clone-weights tests passed"
