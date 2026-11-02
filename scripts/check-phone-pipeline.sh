#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
# Phone-path smoke: /health /dot /mic /turn (and the same through NGROK_URL when set).
# Does not deploy. Does not open SSH. Loopback only unless you pass a URL.
#
#   scripts/check-phone-pipeline.sh
#   NGROK_URL=https://example.ngrok-free.dev scripts/check-phone-pipeline.sh
#   BASE_URL=http://127.0.0.1:7878 scripts/check-phone-pipeline.sh --no-start
#
# Exit 0 pass, 1 fail. Starts run-v1-screen.sh --check when nothing is listening,
# unless --no-start is given.
set -uo pipefail
cd "$(dirname "$0")/.."
BASE="${BASE_URL:-http://127.0.0.1:7878}"
START=1
for a in "$@"; do
  case "$a" in
    --no-start) START=0 ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "unknown arg: $a" >&2; exit 64 ;;
  esac
done

pass=0; fail=0
ok(){ echo "  PASS $*"; pass=$((pass+1)); }
bad(){ echo "  FAIL $*"; fail=$((fail+1)); }

probe(){
  local base="$1" label="$2"
  local hdr=()
  [[ "$base" == https://* ]] && hdr=(-H "ngrok-skip-browser-warning: 1")
  local code
  code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 8 "${hdr[@]}" "$base/health" || echo 000)
  [[ "$code" == "200" ]] && ok "$label /health" || bad "$label /health ($code)"
  code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 8 "${hdr[@]}" "$base/dot" || echo 000)
  [[ "$code" == "200" ]] && ok "$label /dot" || bad "$label /dot ($code)"
  code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 8 "${hdr[@]}" -X POST -H 'Content-Type: application/json' --data '{"rms":0.01}' "$base/mic" || echo 000)
  [[ "$code" == "204" ]] && ok "$label POST /mic" || bad "$label POST /mic ($code)"
  code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 8 "${hdr[@]}" -X OPTIONS -H 'Origin: https://phone.example' -H 'Access-Control-Request-Method: POST' "$base/turn" || echo 000)
  [[ "$code" == "204" ]] && ok "$label OPTIONS /turn" || bad "$label OPTIONS /turn ($code)"
  local wav
  wav=$(mktemp)
  python3 - "$wav" <<'PY'
import struct,sys
n=320
open(sys.argv[1],"wb").write(b"RIFF"+struct.pack("<I",36+n*2)+b"WAVEfmt "+struct.pack("<IHHIIHH",16,1,1,16000,32000,2,16)+b"data"+struct.pack("<I",n*2)+b"\x00"*(n*2))
PY
  code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 8 "${hdr[@]}" -X POST -H 'Content-Type: audio/wav' --data-binary @"$wav" "$base/turn" || echo 000)
  rm -f "$wav"
  [[ "$code" == "202" ]] && ok "$label POST /turn" || bad "$label POST /turn ($code)"
}

if ! curl -fsS --max-time 2 "$BASE/health" >/dev/null 2>&1; then
  if [[ "$START" == "1" && "$BASE" == http://127.0.0.1:* ]]; then
    echo "nothing on $BASE — running scripts/run-v1-screen.sh --check"
    if ! scripts/run-v1-screen.sh --check; then
      echo "screen self-check failed"; exit 1
    fi
    echo
    echo "embedded screen check passed; probing $BASE once more"
  else
    echo "nothing listening at $BASE (pass --no-start only when a server is already up)"
    exit 1
  fi
fi

echo "Galion Voice phone pipeline · $BASE"
probe "$BASE" local
if [[ -n "${NGROK_URL:-}" ]]; then
  echo "public edge · $NGROK_URL"
  probe "${NGROK_URL%/}" ngrok
fi
echo "result: $pass passed, $fail failed"
[[ "$fail" == "0" ]]
