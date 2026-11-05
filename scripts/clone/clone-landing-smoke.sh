#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Symphony B — clone landing smoke.
# Demo = spec = test. Harness before weights.
#
# Checks:
#   - LOCKS frame_ms is 80 (read only; this script does not edit the lock)
#   - reference_clips is true only by owner decision t623u (CC0 donors)
#   - the worker is CPU-only Nano, one thread, no GGUF, no denoise
#   - when a pinned wav is on disk, its sha256 matches REFS.sha256
#   - when the wav is absent, that persona is a clean skip (CI)
#
# Does not download weights, does not call generate, does not rewrite audio.
#
#   scripts/clone/clone-landing-smoke.sh
#
# Exit: 0 pass (skips allowed) · 1 gate failed
set -euo pipefail
cd "$(dirname "$0")/../.."

WORKER="scripts/clone/chatterbox_worker.py"
PINS="scripts/clone/REFS.sha256"
fail() { echo "clone-landing: $*" >&2; exit 1; }

grep -q '^frame_ms = 80' LOCKS.toml || fail "frame_ms is not 80"
grep -q '^reference_clips = true' LOCKS.toml && grep -q '^ref_clip_decision = "t623u"' LOCKS.toml || fail "reference_clips must be true with ref_clip_decision t623u"
grep -q 'model_primary = "chatterbox-nano"' LOCKS.toml || fail "primary model is not chatterbox-nano"

# torch is imported lazily inside the generate path (PR #53), never at module level.
if grep -E -q '^(import|from) (torch|gguf|llama_cpp)|import (gguf|llama_cpp)|ffmpeg|noisereduce|\.gguf' "$WORKER"; then
  fail "worker imports a forbidden runtime or a denoise tool"
fi
grep -q 'denoise refused' "$WORKER" || fail "worker does not refuse denoise"
grep -q 'CUDA_VISIBLE_DEVICES' "$WORKER" || fail "worker does not force CPU"

python3 - <<'PY' || fail "REFS.sha256 drifted from the current Samantha and Jarvis pins"
import pathlib
text = pathlib.Path("scripts/clone/REFS.sha256").read_text()
need = {
    "samantha": "1636428272b85d4b4f6c67bb859bfe33d9983ff9439646519305ff13e6921005",
    "jarvis": "60ad5457dac31af915a9679760705296f9aad17e15ee84885385509b58b8f6b6",
}
for persona, sha in need.items():
    line = next(ln for ln in text.splitlines() if ln.startswith(persona + " "))
    parts = line.split()
    assert parts[1] == sha, line
    assert parts[5] == "16"
    assert parts[6].endswith(".wav") and "mp3" not in parts[6]
    assert parts[8].startswith("preserve"), line
jarvis = next(ln for ln in text.splitlines() if ln.startswith("jarvis "))
assert "preserve-hub" in jarvis
assert jarvis.split()[3] == "44100" and jarvis.split()[4] == "1"
samantha = next(ln for ln in text.splitlines() if ln.startswith("samantha "))
assert samantha.split()[3] == "44100" and samantha.split()[4] == "1" and samantha.split()[8] == "preserve"
PY

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
# Without the stub flag and without weights the process exits 2 after the
# CPU line. That is the honest absent-weights path, not a landing failure.
set +e
python3 "$WORKER" >"$tmp/absent-pcm" 2>"$tmp/absent-err" <<'EOF'
{"text":"Hi.","model":"nano","persona":"jarvis","device":"cpu","denoise":false}
EOF
absent_status=$?
set -e
if [ -f weights/clone/nano/t3_nano_v1.safetensors ]; then
  [ "$absent_status" -eq 0 ] || fail "weights are present but the worker exited $absent_status: $(cat "$tmp/absent-err")"
else
  [ "$absent_status" -eq 2 ] || fail "expected exit 2 without weights, got $absent_status: $(cat "$tmp/absent-err")"
  grep -q 'galion-clone-absent' "$tmp/absent-err" || fail "absent marker missing"
  [ ! -s "$tmp/absent-pcm" ] || fail "absent weights wrote PCM"
fi
status=0
set +e
GALION_CLONE_STUB_PCM=1 python3 "$WORKER" >"$tmp/pcm" 2>"$tmp/err" <<'EOF'
{"text":"Hi.","model":"nano","persona":"jarvis","device":"cpu","denoise":false}
EOF
status=$?
set -e
[ "$status" -eq 0 ] || fail "stub speak failed ($status): $(cat "$tmp/err")"
grep -q 'galion-clone-cpu: threads=1 device=cpu denoise=false' "$tmp/err" || fail "cpu pin missing: $(cat "$tmp/err")"
grep -q 'galion-clone-texture-layer: persona=jarvis present=1 enabled=1 stripped=0' "$tmp/err" || fail "hub layer not on by default: $(cat "$tmp/err")"
grep -q 'galion-clone-emit: galion-vc' "$tmp/err" || fail "galion emit marker missing"
grep -q 'session_vcpu=1 threads=1' "$tmp/err" || fail "session budget missing: $(cat "$tmp/err")"
grep -q 'gguf=false' "$tmp/err" || fail "gguf flag missing: $(cat "$tmp/err")"
grep -q 'model=nano' "$tmp/err" || fail "model swapped or missing: $(cat "$tmp/err")"
[ -s "$tmp/pcm" ] || fail "stub produced no PCM"
echo "clone-landing: protocol PCM ok (not a model voice)"

set +e
GALION_CLONE_STUB_PCM=1 python3 "$WORKER" >"$tmp/denied" 2>"$tmp/deny-err" <<'EOF'
{"text":"Hi.","persona":"jarvis","denoise":true,"texture":"clean"}
EOF
deny_status=$?
set -e
[ "$deny_status" -eq 3 ] || fail "denoise was not refused ($deny_status)"
[ ! -s "$tmp/denied" ] || fail "denoise wrote PCM"
grep -q 'denoise refused' "$tmp/deny-err" || fail "denoise reason missing"

set +e
GALION_CLONE_DEVICE=cuda GALION_CLONE_STUB_PCM=1 python3 "$WORKER" >"$tmp/gpu" 2>"$tmp/gpu-err"
gpu_status=$?
set -e
[ "$gpu_status" -eq 3 ] || fail "GPU device was not refused ($gpu_status)"
[ ! -s "$tmp/gpu" ] || fail "GPU path wrote PCM"

set +e
GALION_CLONE_THREADS=8 GALION_CLONE_STUB_PCM=1 python3 "$WORKER" >"$tmp/thr" 2>"$tmp/thr-err" <<'EOF'
{"text":"Hi.","persona":"samantha"}
EOF
thr_status=$?
set -e
[ "$thr_status" -eq 0 ] || fail "thread clamp failed ($thr_status): $(cat "$tmp/thr-err")"
grep -q 'asked=8 applied=1' "$tmp/thr-err" || fail "thread override was not clamped to 1 vCPU"

set +e
GALION_JARVIS_TEXTURE=0 GALION_CLONE_STUB_PCM=1 python3 "$WORKER" >"$tmp/off" 2>"$tmp/off-err" <<'EOF'
{"text":"Hi.","persona":"jarvis","device":"cpu","denoise":false,"texture":"preserve-hub","texture_layer":false}
EOF
off_status=$?
set -e
[ "$off_status" -eq 0 ] || fail "texture toggle off failed: $(cat "$tmp/off-err")"
[ -s "$tmp/off" ] || fail "texture toggle off produced no PCM"
grep -q 'present=1 enabled=0 stripped=0' "$tmp/off-err" || fail "toggle off stripped or dropped the layer: $(cat "$tmp/off-err")"
grep -q 'denoise=false' "$tmp/off-err" || fail "toggle off denoised"

set +e
GALION_CLONE_STUB_PCM=1 python3 "$WORKER" >"$tmp/strip" 2>"$tmp/strip-err" <<'EOF'
{"text":"Hi.","persona":"jarvis","strip_texture":true}
EOF
strip_status=$?
set -e
[ "$strip_status" -eq 3 ] || fail "strip_texture was not refused"
[ ! -s "$tmp/strip" ] || fail "strip_texture wrote PCM"

grep -q 'GALIONVC' crates/galion-ml-voice/src/speech/galion_clip.rs || fail "galion clip magic missing"
echo "clone-landing: lock_flip=OPEN-t623u reference_clips=true (CC0 donors only)"

# Unconsented name, MP3, and a hash mismatch must not yield PCM.
set +e
GALION_CLONE_STUB_PCM=1 python3 "$WORKER" >"$tmp/mp3" 2>"$tmp/mp3-err" <<'EOF'
{"text":"Hi.","persona":"jarvis","reference":"jarvis.mp3"}
EOF
mp3_status=$?
set -e
[ "$mp3_status" -eq 3 ] || fail "mp3 was not refused"
[ ! -s "$tmp/mp3" ] || fail "mp3 wrote PCM"
grep -q 'mp3 is not a clone reference' "$tmp/mp3-err" || fail "mp3 reason missing"

bad="$tmp/store"
mkdir -p "$bad/jarvis" "$bad/samantha"
printf 'not-the-hub' > "$bad/jarvis/reference.wav"
set +e
GALION_CLIP_STORE="$bad" GALION_CLONE_STUB_PCM=1 python3 "$WORKER" >"$tmp/badpcm" 2>"$tmp/bad-err" <<EOF
{"text":"Hi.","persona":"jarvis","audio_prompt_path":"$bad/jarvis/reference.wav","reference":"$bad/jarvis/reference.wav","denoise":false,"texture":"preserve-hub"}
EOF
bad_status=$?
set -e
[ "$bad_status" -eq 3 ] || fail "bad sha256 was accepted ($bad_status): $(cat "$tmp/bad-err")"
[ ! -s "$tmp/badpcm" ] || fail "bad sha256 wrote PCM"
grep -q 'sha256 mismatch' "$tmp/bad-err" || fail "mismatch reason missing: $(cat "$tmp/bad-err")"

check_persona() {
  local persona="$1" repo_rel="$2" store_rel="$3" sha="$4" rate="$5" channels="$6" bits="$7" texture="$8"
  local path="" source="absent"
  if [ -n "${GALION_CLIP_STORE:-}" ] && [ -f "$GALION_CLIP_STORE/$store_rel" ]; then
    path="$GALION_CLIP_STORE/$store_rel"
    source="store"
  elif [ -f "$repo_rel" ]; then
    path="$repo_rel"
    source="repo"
  fi
  if [ -z "$path" ]; then
    echo "clone-landing: sha256 persona=$persona status=skip reason=ref-absent"
    return 0
  fi
  case "$path" in
    *.mp3|*.MP3) fail "$persona path is mp3" ;;
  esac
  got=$(sha256sum "$path" | awk '{print $1}')
  [ "$got" = "$sha" ] || fail "$persona sha256 $got != $sha ($path)"
  python3 - "$path" "$rate" "$channels" "$bits" <<'PY'
import sys, wave
path, rate, channels, bits = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4])
with wave.open(path, "rb") as w:
    if w.getframerate() != rate or w.getnchannels() != channels or w.getsampwidth() != bits // 8:
        raise SystemExit(
            "header %s rate=%s ch=%s width=%s" % (path, w.getframerate(), w.getnchannels(), w.getsampwidth())
        )
PY
  set +e
  GALION_CLONE_STUB_PCM=1 python3 "$WORKER" >"$tmp/spoke" 2>"$tmp/spoke-err" <<EOF
{"text":"Hi.","model":"nano","persona":"$persona","device":"cpu","denoise":false,"texture":"$texture","audio_prompt_path":"$path","reference":"$path"}
EOF
  spoke_status=$?
  set -e
  [ "$spoke_status" -eq 0 ] || fail "$persona consented ref refused: $(cat "$tmp/spoke-err")"
  [ -s "$tmp/spoke" ] || fail "$persona produced no protocol PCM"
  grep -q "galion-clone-ref: ok persona=$persona sha256=$sha" "$tmp/spoke-err" || fail "$persona ref log missing: $(cat "$tmp/spoke-err")"
  grep -q "texture=$texture" "$tmp/spoke-err" || fail "$persona texture missing"
  grep -q 'denoise=false' "$tmp/spoke-err" || fail "$persona denoise flag missing"
  echo "clone-landing: sha256 persona=$persona status=pass source=$source texture=$texture pcm=protocol-stub"
}

# Pins are the spec. Parse them rather than retyping the layout.
while read -r persona sha duration rate channels bits repo_rel store_rel texture; do
  case "$persona" in
    ''|'#'*) continue ;;
  esac
  check_persona "$persona" "$repo_rel" "$store_rel" "$sha" "$rate" "$channels" "$bits" "$texture"
done < <(grep -v '^#' "$PINS" | grep -v '^$')

if [ ! -f weights/clone/nano/t3_nano_v1.safetensors ]; then
  echo "clone-landing: generate=blocked reason=weights-not-fetched model=chatterbox-nano device=cpu"
else
  echo "clone-landing: generate=unwired reason=chatterbox-generate-not-called weights=present"
fi
echo "clone-landing: PASS frame_ms=80 session_vcpu=1 device=cpu gguf=false denoise=false texture_toggle=separate galion_clip=GALIONVC lock_flip=OPEN-t623u"
