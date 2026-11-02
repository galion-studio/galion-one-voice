#!/usr/bin/env bash
# A14 — canned demo golden hash. Usage: ./golden.sh [--write]   (MIT)
# Artifact = out/canned/demo.artifact = sha256 list of every canned output, covering 3 consent variants:
#   default   : no consent (default deny)           -> neutral-warm, Rika OFF, no reading
#   opener    : SENSE+ADAPT, personal, --rika-opener -> counterbalance (F) + Rika wordless-hum opener
#   workplace / education : SENSE+ADAPT + opener requested -> hard OFF; agent band must equal default
# plus KD target exports (failing specs, decision D). Deterministic: no wall clock, GALION_MODEL dropped.
set -euo pipefail
cd "$(dirname "$0")"
GOLD=../galion-gates/golden/demo.sha256
O=out/canned; mkdir -p "$O"
PY=(env -u GALION_MODEL python3 galion_demo.py)
"${PY[@]}" selftest >/dev/null || { echo "DEMO GOLDEN FAIL selftest"; exit 1; }
"${PY[@]}" gen --canned --in "$O/demo_input.wav" >/dev/null
run(){ local tag="$1"; shift; "${PY[@]}" run --canned --in "$O/demo_input.wav" --out "$O/${tag}_output_stereo.wav" \
        --capture "$O/${tag}_capture.txt" --kd-out "$O/${tag}_kd_targets.jsonl" "$@" >/dev/null; }
run default
run opener    --consent sense,adapt --purpose personal  --rika-opener
run workplace --consent sense,adapt --purpose workplace --rika-opener
run education --consent sense,adapt --purpose education --rika-opener
for p in workplace education; do
  cmp -s "$O/default_output_stereo.wav" "$O/${p}_output_stereo.wav" || { echo "DEMO GOLDEN FAIL $p not hard-off (agent band differs from default)"; exit 1; }
  grep -q 'Rika OFF (DenyProhibitedPurpose)' "$O/${p}_capture.txt" || { echo "DEMO GOLDEN FAIL $p capture lacks hard-off line"; exit 1; }
done
grep -q 'PASS F-counterbalance' "$O/opener_capture.txt" || { echo "DEMO GOLDEN FAIL counterbalance"; exit 1; }
# capture paths are in the header; they are fixed above so the hash is stable.
( cd "$O" && sha256sum demo_input.wav \
    default_output_stereo.wav default_capture.txt default_kd_targets.jsonl \
    opener_output_stereo.wav opener_capture.txt opener_kd_targets.jsonl \
    workplace_output_stereo.wav workplace_capture.txt workplace_kd_targets.jsonl \
    education_output_stereo.wav education_capture.txt education_kd_targets.jsonl ) > "$O/demo.artifact"
H=$(sha256sum "$O/demo.artifact" | cut -d' ' -f1)
if [[ "${1:-}" == "--write" ]]; then
  mkdir -p "$(dirname "$GOLD")"; { echo "$H  demo.artifact"; sed 's/^/# /' "$O/demo.artifact"; } > "$GOLD"; echo "golden written: $H"; exit 0
fi
WANT=$(grep -v '^#' "$GOLD" | cut -d' ' -f1)
if [[ "$H" == "$WANT" ]]; then echo "DEMO GOLDEN PASS $H"; else echo "DEMO GOLDEN FAIL got=$H want=$WANT"; exit 1; fi
