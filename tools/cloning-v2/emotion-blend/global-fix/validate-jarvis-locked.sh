#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
G=emotion-blend/global-fix
CBX=pipeline/cbx-v2-timbre/target/release/cbx
BATCH=emotion-fix/cbx-emo/target/release/cbx
mkdir -p "$G/wav" "$G/raw" "$G/scores"
for p in jarvis; do
 name="own-${p}-v2-blendfix-coherent"
 ref="../clip-store-own/$p/reference.wav"
 nmr=../clip-store-own/jarvis/reference.wav; [[ "$p" != jarvis ]] || nmr=../clip-store-own/samantha/reference.wav
 if [[ ! -e "$G/refs/$p-blend.wav" ]]; then
  nice -n 10 "$G/refbuilder/target/debug/galion-blend-ref" --input "../clip-store-own/$p/original.wav" --output "$G/refs/$p-blend.wav" --meta "$G/refs/$p-blend.json" --blend-preserve --emotion-scorer scorers/emotion/target/release/galion-emotion > "$G/logs/prep-$p.log" 2>&1
 fi
 [[ ! -e "$HOME/.cache/huggingface/cbx/voices/$name.cbxvoice" ]] || { echo "existing voice $name"; exit 2; }
 nice -n 10 "$CBX" --intra-threads 2 --ep cpu voice add --name "$name" --voice-wav "$G/refs/$p-blend.wav" --dtype fp32 > "$G/logs/encode-$p.log" 2>&1
 nice -n 10 "$BATCH" --intra-threads 2 --ep cpu batch --voice "$name" --manifest emotion-fix/test12.tsv --out-dir "$G/raw/$p" --dtype fp32 --temperature 0 --seed 0 --repetition-penalty 1.2 > "$G/logs/render-$p.log" 2>&1
 mkdir "$G/wav/$p" "$G/scores/$p"
 for wav in "$G/raw/$p"/t*.wav; do
  nice -n 10 "$CBX" --intra-threads 2 timbre apply --input "$wav" --reference "$ref" --strength 0.5 --out "$G/wav/$p/$(basename "$wav")" >> "$G/logs/timbre-$p.log" 2>&1
 done
 nice -n 10 scorers/ecapa/target/release/galion-ecapa --ref "$ref" "$G/wav/$p"/t*.wav --threads 2 --out "$G/scores/$p/ecapa.json" > "$G/logs/ecapa-$p.log" 2>&1
 nice -n 10 scorers/quality/bin/galion-wer --text-dir emotion-fix/validation12/texts --jobs 2 --out "$G/scores/$p/wer.json" "$G/wav/$p"/t*.wav > "$G/logs/wer-$p.log" 2>&1
 nice -n 10 scorers/emotion/target/release/galion-emotion score --ref "$ref" --threads 2 --out "$G/scores/$p/emotion.json" "$G/wav/$p"/t*.wav > "$G/logs/emotion-$p.log" 2>&1
 nice -n 10 scorers/quality/bin/galion-mos --nmr "$nmr" --threads 2 --out "$G/scores/$p/mos.json" "$G/wav/$p"/t*.wav > "$G/logs/mos-$p.log" 2>&1
 echo "$(date '+%F %T %Z') FINISHED $p"
done
sha256sum -c "$G/protected-before.sha256"

python3 emotion-blend/global-fix/summarize_results.py
