#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
#
# Offline checks for CPU-venv / tokenizer staging. No pip, no generate,
# no edit of REFS.sha256 or LOCKS.toml, no WAV.
set -euo pipefail
cd "$(dirname "$0")/../.."
STAGE="$PWD/scripts/clone/stage-clone-prep.sh"
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

refs_before=$(sha256sum "$ROOT/scripts/clone/REFS.sha256" | awk '{print $1}')
locks_before=$(sha256sum "$ROOT/LOCKS.toml" | awk '{print $1}')

assert_locks() {
  local refs_after locks_after
  refs_after=$(sha256sum "$ROOT/scripts/clone/REFS.sha256" | awk '{print $1}')
  locks_after=$(sha256sum "$ROOT/LOCKS.toml" | awk '{print $1}')
  [ "$refs_after" = "$refs_before" ] || fail "REFS.sha256 changed"
  [ "$locks_after" = "$locks_before" ] || fail "LOCKS.toml changed"
}

names=(
  vocab.json
  merges.txt
  added_tokens.json
  special_tokens_map.json
  tokenizer_config.json
)

write_pins() {
  local src="$1"
  local pins="$2"
  : > "$pins"
  local name path sha size
  for name in "${names[@]}"; do
    path="$src/$name"
    sha=$(sha256sum "$path" | awk '{print $1}')
    size=$(stat -c %s "$path")
    printf 'tokenizer/%s %s %s test %s deadbeef\n' "$name" "$sha" "$size" "$name" >> "$pins"
  done
}

# No mode, and venv without the prep flag, are refusals. Neither creates a venv.
expect_rc 2 bash "$STAGE"
grep -q 'use one of' "$tmp/err" || fail "usage text"
expect_rc 2 env GALION_CLONE_VENV="$tmp/venv" bash "$STAGE" --venv
grep -q 'GALION_CLONE_PREP' "$tmp/err" || fail "venv refusal text"
[ ! -e "$tmp/venv" ] || fail "venv created without GALION_CLONE_PREP"
assert_locks

# Missing operator tree is BLOCKED. Destination is not created. PCM is not run.
expect_rc 1 env \
  GALION_CLONE_TOKENIZER_SRC="$tmp/missing-src" \
  GALION_CLONE_WEIGHTS="$tmp/weights-missing" \
  bash "$STAGE" --tokenizer
grep -q 'BLOCKED tokenizer source absent' "$tmp/err" || fail "missing source text"
grep -q 'pcm=not-run' "$tmp/out" || fail "missing source still claims pcm"
[ ! -d "$tmp/weights-missing" ] || fail "weights dir created for a missing source"
assert_locks

# A source that does not match the pins leaves the destination untouched.
bad="$tmp/bad-src"
mkdir -p "$bad/tokenizer"
printf 'not-the-pinned-bytes\n' > "$bad/tokenizer/vocab.json"
printf 'merges\n' > "$bad/merges.txt"
printf '{}\n' > "$bad/added_tokens.json"
printf '{}\n' > "$bad/special_tokens_map.json"
printf '{}\n' > "$bad/tokenizer_config.json"
printf 'RIFF-not-audio' > "$bad/samantha.wav"
printf 'weights' > "$bad/ve.safetensors"
# Pins describe different bytes than the source.
cat > "$tmp/bad.pins" <<'EOF'
tokenizer/vocab.json 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef 4 test vocab.json deadbeef
tokenizer/merges.txt 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef 4 test merges.txt deadbeef
tokenizer/added_tokens.json 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef 4 test added_tokens.json deadbeef
tokenizer/special_tokens_map.json 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef 4 test special_tokens_map.json deadbeef
tokenizer/tokenizer_config.json 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef 4 test tokenizer_config.json deadbeef
EOF
expect_rc 2 env \
  GALION_CLONE_TOKENIZER_SRC="$bad" \
  GALION_CLONE_TOKENIZER_PINS="$tmp/bad.pins" \
  GALION_CLONE_WEIGHTS="$tmp/weights-bad" \
  bash "$STAGE" --tokenizer
grep -q 'pin mismatch' "$tmp/err" || fail "mismatch text"
[ ! -d "$tmp/weights-bad" ] || fail "destination created on pin mismatch"
assert_locks

# Flat and nested copies of the same name must agree.
split="$tmp/split-src"
mkdir -p "$split/tokenizer"
printf 'flat\n' > "$split/vocab.json"
printf 'nested\n' > "$split/tokenizer/vocab.json"
for name in merges.txt added_tokens.json special_tokens_map.json tokenizer_config.json; do
  printf '%s\n' "$name" > "$split/$name"
done
write_pins "$split" "$tmp/split.pins"
expect_rc 2 env \
  GALION_CLONE_TOKENIZER_SRC="$split" \
  GALION_CLONE_TOKENIZER_PINS="$tmp/split.pins" \
  GALION_CLONE_WEIGHTS="$tmp/weights-split" \
  bash "$STAGE" --tokenizer
grep -q 'disagrees' "$tmp/err" || fail "disagree text"
[ ! -d "$tmp/weights-split" ] || fail "destination created on disagree"
assert_locks

# Matching text files are copied. A neighbouring WAV and safetensors file are not.
good="$tmp/good-src"
mkdir -p "$good"
for name in "${names[@]}"; do
  printf 'galion-tokenizer-test %s\n' "$name" > "$good/$name"
done
printf 'not-a-reference\n' > "$good/jarvis.wav"
printf 'not-a-weight\n' > "$good/t3_nano_v1.safetensors"
write_pins "$good" "$tmp/good.pins"
expect_rc 0 env \
  GALION_CLONE_TOKENIZER_SRC="$good" \
  GALION_CLONE_TOKENIZER_PINS="$tmp/good.pins" \
  GALION_CLONE_WEIGHTS="$tmp/weights-good" \
  bash "$STAGE" --tokenizer
grep -q 'tokenizer=ready' "$tmp/out" || fail "ready text"
grep -q 'pcm=not-run' "$tmp/out" || fail "ready path claims pcm"
[ -f "$tmp/weights-good/tokenizer/vocab.json" ] || fail "vocab not staged"
cmp -s "$good/vocab.json" "$tmp/weights-good/tokenizer/vocab.json" || fail "vocab bytes differ"
find "$tmp/weights-good" -type f \( -name '*.wav' -o -name '*.safetensors' -o -name '*.pt' \) | grep -q . && fail "audio or weights copied" || true
assert_locks

# --check is ready only when the destination matches and a venv probe would pass.
# This fixture has no venv, so the tokenizer can be ready while the venv stays BLOCKED.
expect_rc 1 env \
  GALION_CLONE_TOKENIZER_PINS="$tmp/good.pins" \
  GALION_CLONE_WEIGHTS="$tmp/weights-good" \
  GALION_CLONE_VENV="$tmp/no-venv" \
  bash "$STAGE" --check
grep -q 'tokenizer=ready' "$tmp/out" || fail "check tokenizer"
grep -q 'venv=absent' "$tmp/out" || fail "check venv"
grep -q 'pcm=not-run' "$tmp/out" || fail "check pcm"
grep -q 'BLOCKED' "$tmp/err" || fail "check blocked"
assert_locks

# The documented venv path is gitignored so a local install cannot be committed.
git check-ignore -q .venv-clone/bin/python || fail ".venv-clone is not ignored"
# This run must not stage audio, weights, or tokenizer bytes into the work tree.
# Existing tracked fixture WAVs are out of scope for this check.
extra=$(git status --porcelain | grep -Ei 'safetensors|\.wav|\.pt|tokenizer/' || true)
[ -z "$extra" ] || fail "staging left audio or tokenizer bytes in the work tree: $extra"

echo "stage-clone-prep.test: ok"
