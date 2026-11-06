#!/usr/bin/env bash
# Offline fidelity eval for galion-laptop (RTX 4060 8 GB).
#
# CPU DSP by default. It does not synthesize, does not load Qwen, and does not
# deploy. Piper stays the TTS. Optional ONNX scorers run one clip at a time
# with at most 2 threads so an 8 GB card is not asked to hold a second model.
#
#   tools/cloning-v2/fidelity/scripts/eval-laptop.sh
#
# Cache (CC0 audio, wavs, reports) stays outside the repo:
#   ${GALION_FIDELITY_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/galion-fidelity}
#
# Optional, all local, none required for the DSP report:
#   GALION_ECAPA_BIN / GALION_ECAPA_MODEL   galion-ecapa, --threads 2
#   GALION_WER_BIN                          galion-wer (needs a transcript)
#   GALION_FIDELITY_REF_TEXT / GALION_FIDELITY_HYP_TEXT
#   GALION_PERSONA_JSON                     persona_match.py or score JSON
#   GALION_FIDELITY_LONG_WAV                one public-domain wav, same speaker, outside the repo
#
# Rollout gates are printed for reporting only: WER <= 1.8%, ECAPA > 0.7057,
# emotion_match >= 0.83. A missing model leaves that field unmeasured.

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIDELITY="$(cd "$HERE/.." && pwd)"
REPO="$(cd "$FIDELITY/../../.." && pwd)"
CACHE="${GALION_FIDELITY_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/galion-fidelity}"

case "$CACHE" in
  "$REPO"|"$REPO"/*)
    echo "galion-fidelity: cache must be outside the repo ($CACHE)" >&2
    exit 2
    ;;
esac

mkdir -p "$CACHE/cc0" "$CACHE/out"
cd "$FIDELITY"

if ! cargo test --offline --manifest-path "$FIDELITY/Cargo.toml"; then
  cargo test --manifest-path "$FIDELITY/Cargo.toml"
fi
if ! cargo build --release --offline --manifest-path "$FIDELITY/Cargo.toml"; then
  cargo build --release --manifest-path "$FIDELITY/Cargo.toml"
fi
BIN="$FIDELITY/target/release/galion-fidelity"

"$BIN" self-check > "$CACHE/out/self-check.json"
python3 "$FIDELITY/py/crosscheck_fidelity.py" "$CACHE/out/self-check.json"
"$BIN" synth-eval --out-dir "$CACHE/out/synthetic"

download() {
  local url="$1" dest="$2"
  if [[ -s "$dest" ]]; then
    return 0
  fi
  curl -fsSL --retry 3 --max-time 90 -o "$dest.partial" "$url" && mv "$dest.partial" "$dest"
}

set +e
download "https://raw.githubusercontent.com/OwenTyme/voice-zero/main/voices/amy_koenig.flac" "$CACHE/cc0/amy_koenig.flac"
download "https://raw.githubusercontent.com/OwenTyme/voice-zero/main/voices/alan_davis_drake.flac" "$CACHE/cc0/alan_davis_drake.flac"
# Public-domain LibriVox (archive.org licenseurl publicdomain). One reader, used only as a long-donor drill.
download "https://archive.org/download/love_friendship_1107_librivox/lovefriendship_bronte_aak_64kb.mp3" "$CACHE/cc0/lovefriendship_bronte_aak_64kb.mp3"
set -e

decode() {
  local src="$1" dest="$2" extra=()
  if [[ ! -s "$src" ]]; then
    return 1
  fi
  if [[ -n "${3:-}" ]]; then
    extra=(-ss "$3" -t "$4")
  fi
  ffmpeg -y -loglevel error "${extra[@]}" -i "$src" -ac 1 -ar 24000 "$dest"
}

set +e
decode "$CACHE/cc0/amy_koenig.flac" "$CACHE/cc0/amy.wav"
decode "$CACHE/cc0/alan_davis_drake.flac" "$CACHE/cc0/alan.wav"
# Public-domain LibriVox, one speaker. 50 s from 30 s in, if the file arrived.
# First 50 s. This 64 kbps public-domain track is about 48 s, so the cut is the whole reading.
decode "$CACHE/cc0/lovefriendship_bronte_aak_64kb.mp3" "$CACHE/cc0/long.wav" 0 50
set -e

run_pair() {
  local name="$1" ref="$2"
  shift 2
  if [[ ! -s "$ref" ]]; then
    echo "skip $name (no wav)"
    return 0
  fi
  local hyp="$CACHE/out/${name}-cond.wav"
  local plan="$CACHE/out/${name}-plan.json"
  local report="$CACHE/out/${name}-eval.json"
  "$BIN" prepare "$@" --out-wav "$hyp" --out-json "$plan"
  local ecapa_args=() persona_args=() text_args=()
  if [[ -n "${GALION_ECAPA_BIN:-}" && -x "${GALION_ECAPA_BIN}" ]]; then
    local ecapa_json="$CACHE/out/${name}-ecapa.json"
    local model_args=()
    if [[ -n "${GALION_ECAPA_MODEL:-}" ]]; then
      model_args=(--model "$GALION_ECAPA_MODEL")
    fi
    if "$GALION_ECAPA_BIN" --ref "$ref" "$hyp" --threads 2 "${model_args[@]}" --out "$ecapa_json"; then
      ecapa_args=(--ecapa-json "$ecapa_json")
    fi
  fi
  if [[ -n "${GALION_PERSONA_JSON:-}" && -f "${GALION_PERSONA_JSON}" ]]; then
    persona_args=(--persona-json "$GALION_PERSONA_JSON")
  fi
  if [[ -n "${GALION_FIDELITY_REF_TEXT:-}" && -n "${GALION_FIDELITY_HYP_TEXT:-}" ]]; then
    text_args=(--ref-text "$GALION_FIDELITY_REF_TEXT" --hyp-text "$GALION_FIDELITY_HYP_TEXT")
  elif [[ -n "${GALION_WER_BIN:-}" && -x "${GALION_WER_BIN}" && -n "${GALION_FIDELITY_REF_TEXT:-}" ]]; then
    local wer_json="$CACHE/out/${name}-wer.json"
    if "$GALION_WER_BIN" --ref "$GALION_FIDELITY_REF_TEXT" --jobs 1 --out "$wer_json" "$hyp"; then
      local hyp_text
      hyp_text="$(python3 - "$wer_json" <<'PY'
import json, sys
doc = json.load(open(sys.argv[1], encoding="utf-8"))
files = doc.get("files") or []
print(files[0].get("hyp", "") if files else "")
PY
)"
      text_args=(--ref-text "$GALION_FIDELITY_REF_TEXT" --hyp-text "$hyp_text")
    fi
  fi
  "$BIN" eval --ref "$ref" --hyp "$hyp" --out "$report" "${ecapa_args[@]}" "${persona_args[@]}" "${text_args[@]}"
}

# Short CC0 pieces, each alone. Amy and Alan are different speakers: do not average them.
run_pair amy "$CACHE/cc0/amy.wav" --input "$CACHE/cc0/amy.wav"
run_pair alan "$CACHE/cc0/alan.wav" --input "$CACHE/cc0/alan.wav"

LONG="${GALION_FIDELITY_LONG_WAV:-$CACHE/cc0/long.wav}"
if [[ -s "$LONG" ]]; then
  run_pair long "$LONG" --input "$LONG"
  # Same-speaker multi-reference: two halves of the long clip.
  if ffmpeg -y -loglevel error -i "$LONG" -t 20 -ac 1 -ar 24000 "$CACHE/out/long-a.wav" \
    && ffmpeg -y -loglevel error -ss 20 -i "$LONG" -t 20 -ac 1 -ar 24000 "$CACHE/out/long-b.wav"; then
    run_pair long-multi "$LONG" --input "$CACHE/out/long-a.wav" --input "$CACHE/out/long-b.wav"
  fi
fi

python3 - "$CACHE/out" <<'PY'
import json, pathlib, sys
out = pathlib.Path(sys.argv[1])
print("galion-fidelity reports:", out)
for path in sorted(out.rglob("*.json")):
    if path.name not in {"self-check.json", "summary.json", "short-eval.json"} and not path.name.endswith("-eval.json") and path.name != "short-plan.json":
        if "eval" not in path.name and path.name != "summary.json":
            continue
    try:
        doc = json.loads(path.read_text())
    except Exception as exc:
        print(f"  {path.name}: unreadable ({exc})")
        continue
    if "embed_cosine" in doc:
        print(
            f"  {path.name}: embed_cosine={doc.get('embed_cosine')} "
            f"mos_proxy={doc.get('mos_proxy')} wer={doc.get('wer')} "
            f"ecapa={doc.get('ecapa')} emotion_match={doc.get('emotion_match')}"
        )
    elif path.name == "summary.json" or path.name == "self-check.json":
        print(f"  {path.name}: written")
print("Rollout gate (reporting only, not measured unless the fields above are numbers):")
print("  WER <= 1.8%   ECAPA > 0.7057   emotion_match >= 0.83")
print("DSP embed_cosine is not ECAPA.")
PY

if command -v nvidia-smi >/dev/null 2>&1; then
  nvidia-smi --query-gpu=name,memory.total --format=csv,noheader > "$CACHE/out/gpu.txt" || true
else
  echo "no nvidia-smi (CPU DSP path)" > "$CACHE/out/gpu.txt"
fi
echo "gpu: $(cat "$CACHE/out/gpu.txt")"
echo "command: tools/cloning-v2/fidelity/scripts/eval-laptop.sh"
