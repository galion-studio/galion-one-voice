#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Section C. Texture stays. No spectral number unless both the reference and a clean baseline exist.
set -euo pipefail
cd "$(dirname "$0")/../.."
fail() { echo "C FAIL — $*"; exit 1; }
grep -E -q 'denoise|noisereduce|highpass|strip_texture' scripts/clone/chatterbox_worker.py && fail "clone worker names a denoise/strip step"
grep -q -i 'do not strip' docs/galion-voice-v2/symphony/CLIP-STORE.md || fail "clip-store runbook does not keep texture"
NOTE="$(python3 - <<'PY'
import os, sys, math, struct, wave

def load_wav(path):
    with wave.open(path, "rb") as w:
        ch, sw, sr, n = w.getnchannels(), w.getsampwidth(), w.getframerate(), w.getnframes()
        raw = w.readframes(n)
    if sw != 2:
        sys.exit("only 16-bit wav is measured")
    samples = struct.unpack("<" + "h" * (len(raw) // 2), raw)
    if ch > 1:
        samples = samples[0::ch]
    return sr, samples

def transient(samples):
    if len(samples) < 2:
        return 0.0
    acc = 0.0
    prev = samples[0]
    for x in samples[1:]:
        d = float(x) - float(prev)
        acc += d * d
        prev = x
    return acc / (len(samples) - 1)

store = os.environ.get("GALION_CLIP_STORE", "").strip()
cands = []
if store:
    cands.append(os.path.join(store, "jarvis", "reference.wav"))
cands.append("docs/galion-voice-v2/personas/refs/jarvis.wav")
ref = next((p for p in cands if os.path.isfile(p)), "")
base = os.environ.get("GALION_TEXTURE_BASELINE", "").strip()
if not base and store:
    guess = os.path.join(store, "jarvis", "clean-speech.wav")
    if os.path.isfile(guess):
        base = guess
if not ref or not base:
    print("spectral=not-measured")
    sys.exit(0)
_, ref_pcm = load_wav(ref)
_, base_pcm = load_wav(base)
ref_e = transient(ref_pcm)
base_e = transient(base_pcm)
# Retention: the reference's transient energy is at least the clean baseline.
# Both numbers are measured from the files just read. No baseline is invented.
print(f"spectral=measured ref_transient={ref_e:.6g} baseline_transient={base_e:.6g}")
if not math.isfinite(ref_e) or not math.isfinite(base_e) or ref_e < base_e:
    sys.exit(2)
PY
)" || fail "texture energy fell below the clean baseline"
echo "C PASS — worker has no denoise step; clip-store runbook says do not strip; ${NOTE}"
