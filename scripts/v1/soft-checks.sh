#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. Copyright (c) 2026 Maciej Grajczyk.
# V1-04 · soft checks for a V1 demo out dir (written by scripts/run-v1-demo.sh).
# Usage: scripts/v1/soft-checks.sh <V1_OUT>    env: V1_SPEECH_KIND (from run-v1-demo.sh), V1_STRICT=1
# Writes <V1_OUT>/V1-SUMMARY.txt + V1-SUMMARY.json.
# Exit: 0 ok (soft fails reported) · 1 a FAIL (or V1_STRICT=1 and a SOFTFAIL/WARN) · 3 nothing to check
set -uo pipefail
cd "$(dirname "$0")/../.."
OUT="${1:?usage: soft-checks.sh <V1_OUT>}"
[ -d "$OUT" ] || { echo "soft-checks: $OUT missing"; exit 3; }
OUT="$(cd "$OUT" && pwd)"
KIND_IN="${V1_SPEECH_KIND:-unknown}"
SUM="$OUT/V1-SUMMARY.txt"; : > "$SUM"
IDS=(); declare -A CS CN
chk(){ IDS+=("$1"); CS[$1]=$2; CN[$1]="$3"; printf '%-4s %-8s %s\n' "$1" "$2" "$3" | tee -a "$SUM"; }
echo "== V1 soft checks · $(date '+%Y-%m-%d %H:%M %Z') · $OUT" | tee -a "$SUM"

# speech WAV: V1-03 turn or EN cascade fallback
SP_WAV="$(ls "$OUT"/speech/*.wav "$OUT"/cascade/en-cascade.wav 2>/dev/null | head -n1)"
[ -n "$SP_WAV" ] && chk C0 PASS "speech turn WAV: ${SP_WAV#$OUT/} ($KIND_IN)" || chk C0 FAIL "no speech turn WAV (V1-03 hook and EN cascade both failed — see run log)"
# C1 live showcase
miss=""; for v in default opener workplace; do for a in live_output_stereo.wav console_capture.txt; do [ -s "$OUT/live/$v/$a" ] || miss+=" $v/$a"; done; done
[ -z "$miss" ] && chk C1 PASS "live 80 ms heartbeat: default/opener/workplace stereo + capture" || chk C1 FAIL "missing:$miss"
# C2 Rika lean-in
LI="$(grep -o 'lean_in_latency=[^ ]*' "$OUT/live/opener/console_capture.txt" 2>/dev/null | tail -n1)"
case "$LI" in *=160*) chk C2 PASS "Rika opener ${LI} ms (2 frames)";; "") chk C2 FAIL "no Rika summary in opener capture";; *) chk C2 WARN "Rika opener ${LI} (expected 160 ms on synthetic input)";; esac
# C3 consent hard-OFF
if cmp -s "$OUT/live/default/live_output_stereo.wav" "$OUT/live/workplace/live_output_stereo.wav" 2>/dev/null \
   && grep -q 'OFF(Gate(DenyProhibitedPurpose))' "$OUT/live/workplace/console_capture.txt" 2>/dev/null
then chk C3 PASS "workplace hard-OFF (me band == default, DenyProhibitedPurpose)"; else chk C3 FAIL "workplace not hard-OFF"; fi
# C4 audio format
if command -v python3 >/dev/null; then
  R="$(python3 - "$OUT" <<'PY'
import sys, wave, glob, os
d=sys.argv[1]; bad=[]; n=0
for f in sorted(set(glob.glob(d+"/speech/*.wav")+glob.glob(d+"/cascade/*.wav")+glob.glob(d+"/live/*/*.wav")+glob.glob(d+"/turn-*/*.wav"))):
    try:
        w=wave.open(f); n+=1
        if w.getframerate()!=24000 or w.getsampwidth()!=2: bad.append(os.path.relpath(f,d))
    except Exception: bad.append(os.path.relpath(f,d))
print(("OK %d" % n) if not bad else "BAD "+" ".join(bad))
PY
)"; case "$R" in "OK 0") chk C4 FAIL "no WAVs";; OK*) chk C4 PASS "${R#OK } WAVs at 24 kHz PCM16";; *) chk C4 FAIL "$R";; esac
else chk C4 SKIP "python3 missing"; fi
# C5 one-dot screen frames
NPNG="$(ls "$OUT"/screen/frame_*.png 2>/dev/null | wc -l)"; NLD="$(ls "$OUT"/live/opener/dot/*.png 2>/dev/null | wc -l)"
if [ -s "$OUT/screen/latest.png" ] || [ "$NLD" -gt 0 ]; then chk C5 PASS "one-dot frames: screen=$NPNG live/opener=$NLD"
elif [ -f "$OUT/state.json" ]; then chk C5 PASS "one-dot served live (state.json present; web mode)"
else chk C5 FAIL "no one-dot frames"; fi
# C6 V4 locks (one dot, 80 ms, consent deny)
LK="$(scripts/check-locks.sh 2>&1 | tail -n1)"; case "$LK" in OK*) chk C6 PASS "$LK";; *) chk C6 FAIL "$LK";; esac
# C7 words (FRXDEL Q2: must say actual words). V1-03 JSON: reply_kind=speech; speech_backend=system-flite (interim)
KIND=tone; J="$(ls "$OUT"/speech/*.json "$OUT"/cascade/en-cascade.json 2>/dev/null | head -n1)"
case "$KIND_IN" in *words*) KIND=words;; esac
if [ -n "$J" ]; then
  grep -Eq '"(reply_kind)" *: *"speech"|"(tts|tts_kind|speech|kind)" *: *"words"' "$J" && KIND=words
  SB="$(sed -n 's/.*"speech_backend":"\([^"]*\)".*/\1/p' "$J")"; ST="$(sed -n 's/.*"spoken_text":"\([^"]*\)".*/\1/p' "$J")"
  A21="$(sed -n 's/.*"a21_weights":"\([^"]*\)".*/\1/p' "$J")"
fi
case "$SB" in a21*) SBL="A21 neural CPU (Piper VITS)";; *flite*) SBL="flite FALLBACK (A21 pack missing), NOT neural";; *) SBL="?";; esac   # FP-04: label from backend
if [ "$KIND" = words ]; then chk C7 PASS "EN reply speaks words \"${ST:-?}\" via ${SB:-?} ($SBL)"
else chk C7 SOFTFAIL "EN reply is NOT words ($KIND_IN) — Q2 requires words; need A21 (scripts/a21/fetch-a21.sh) or libflite1"; fi
# C11 speech backend dependency (FP-03/04: A21 Piper runtime first, flite fallback only)
PIPER="weights/runtime/piper/piper"   # cwd = repo root
if [ "${SB:-}" = a21-weights ] && [ -x "$PIPER" ] && ls weights/tts/*.onnx >/dev/null 2>&1; then chk C11 PASS "A21 runtime present (weights/runtime/piper + weights/tts/*.onnx, CPU)"
elif { PATH="$PATH:/sbin:/usr/sbin" ldconfig -p 2>/dev/null | grep -q 'libflite.so.1'; } || ls /usr/lib/*/libflite.so.1 /usr/lib/libflite.so.1 /usr/lib64/libflite.so.1 >/dev/null 2>&1; then chk C11 PASS "libflite1 present (fallback; A21 pack missing — scripts/a21/fetch-a21.sh)"
else chk C11 SOFTFAIL "no speech backend — EN cascade exits 3 (scripts/a21/fetch-a21.sh, or apt install libflite1)"; fi
# C12 (FP-04) EN turn: the reply words sit in the ME band (dot "speaking"), not the you band
EL="$OUT/live/en-turn/console_capture.txt"
if [ -f "$EL" ] && grep -q "voice=reply-wav" "$EL"; then chk C12 PASS "EN turn me band = reply words (voice=reply-wav)"
else chk C12 SOFTFAIL "EN turn me band is not the reply words (no voice=reply-wav in en-turn capture)"; fi
# C8 brand
B="FRXDEL"" STUDIOS"
BH="$(grep -RIl --exclude-dir=target --exclude=V1-SUMMARY.txt -- "$B" docs/V1-DEMO.md docs/agent-v1-demo/PROGRESS.md docs/agent-v1-demo/V1-0*-STATUS.md \
      crates/galion-console crates/galion-live-demo scripts/v1 galion-demo/v1 "$OUT" 2>/dev/null || true)"
[ -z "$BH" ] && chk C8 PASS "brand: Galion Voice only on V1 surfaces" || chk C8 FAIL "brand hit in: $(echo $BH)"
if [ "${A21:-absent}" = present-verified ]; then chk C9 INFO "A21 cascade TTS built: Piper CPU weights present-verified (sha256); me band = these words (--reply); Mimi codec still IS_STUB (--mimi off); counsel sign-off pending"
else chk C9 INFO "A21 weights ${A21:-absent} (docs/agent-v1-demo/A21-BLOCKERS.md); fetch with scripts/a21/fetch-a21.sh; speech falls back to flite; Mimi IS_STUB"; fi
chk C10 PENDING "deploy target 51.38.137.193 — SSH denied; local only (scripts/deploy/*-dryrun.sh)"

FAILS=0; SOFT=0
for i in "${IDS[@]}"; do case "${CS[$i]}" in FAIL) FAILS=$((FAILS+1));; SOFTFAIL|WARN) SOFT=$((SOFT+1));; esac; done
{ printf '{"out":"%s","speech":"%s","speech_backend":"%s","a21":"%s","checks":{' "$OUT" "$KIND" "${SB:-}" "${A21:-absent}"; f=1
  for i in "${IDS[@]}"; do [ $f = 1 ] || printf ','; f=0; printf '"%s":{"status":"%s","note":"%s"}' "$i" "${CS[$i]}" "$(echo "${CN[$i]}" | sed 's/\\/\\\\/g; s/"/\\"/g')"; done
  printf '},"fails":%d,"soft":%d}\n' "$FAILS" "$SOFT"; } > "$OUT/V1-SUMMARY.json"
echo "== V1 RESULT: fails=$FAILS soft=$SOFT speech=$KIND summary=$SUM" | tee -a "$SUM"
[ "$FAILS" -gt 0 ] && exit 1
[ "${V1_STRICT:-0}" = 1 ] && [ "$SOFT" -gt 0 ] && exit 1
exit 0
