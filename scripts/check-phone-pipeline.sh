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
#
# GALION_CORS_ORIGINS unset → V1 wildcard, no session token.
# When it is set to exact origins, this probe sends the first origin and a
# token taken from POST /session's body. A literal "*" is not the wildcard.
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

# First exact origin in GALION_CORS_ORIGINS, or empty when the V1 wildcard applies.
cors_origin() {
  local raw="${GALION_CORS_ORIGINS:-}" o
  [[ -z "$raw" ]] && return 0
  local IFS=','
  for o in $raw; do
    o="${o#"${o%%[![:space:]]*}"}"
    o="${o%"${o##*[![:space:]]}"}"
    [[ -z "$o" || "$o" == "*" ]] && continue
    printf '%s' "$o"
    return 0
  done
}

probe(){
  local base="$1" label="$2"
  local hdr=() oh=() sess=() origin opt_origin
  [[ "$base" == https://* ]] && hdr=(-H "ngrok-skip-browser-warning: 1")
  origin="$(cors_origin || true)"
  if [[ -n "$origin" ]]; then
    oh=(-H "Origin: $origin")
    local tok
    tok=$(curl -sS --max-time 8 "${hdr[@]}" "${oh[@]}" -X POST \
      -H 'Content-Type: application/json' --data '{"op":"open"}' "$base/session" \
      | python3 -c 'import json,sys
try:
    print(json.load(sys.stdin).get("token",""), end="")
except Exception:
    pass' || true)
    if [[ -n "$tok" ]]; then
      sess=(-H "X-Galion-Session: $tok")
      ok "$label POST /session"
    else
      bad "$label POST /session"
    fi
    opt_origin="$origin"
  else
    opt_origin="https://phone.example"
  fi
  local code
  code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 8 "${hdr[@]}" "$base/health" || echo 000)
  [[ "$code" == "200" ]] && ok "$label /health" || bad "$label /health ($code)"
  code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 8 "${hdr[@]}" "$base/dot" || echo 000)
  [[ "$code" == "200" ]] && ok "$label /dot" || bad "$label /dot ($code)"
  code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 8 "${hdr[@]}" "${oh[@]}" "${sess[@]}" -X POST -H 'Content-Type: application/json' --data '{"rms":0.01}' "$base/mic" || echo 000)
  [[ "$code" == "204" ]] && ok "$label POST /mic" || bad "$label POST /mic ($code)"
  code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 8 "${hdr[@]}" -X OPTIONS -H "Origin: $opt_origin" -H 'Access-Control-Request-Method: POST' "$base/turn" || echo 000)
  [[ "$code" == "204" ]] && ok "$label OPTIONS /turn" || bad "$label OPTIONS /turn ($code)"
  local wav
  wav=$(mktemp)
  python3 - "$wav" <<'PY'
import struct,sys
n=320
open(sys.argv[1],"wb").write(b"RIFF"+struct.pack("<I",36+n*2)+b"WAVEfmt "+struct.pack("<IHHIIHH",16,1,1,16000,32000,2,16)+b"data"+struct.pack("<I",n*2)+b"\x00"*(n*2))
PY
  code=$(curl -sS -o /dev/null -w '%{http_code}' --max-time 8 "${hdr[@]}" "${oh[@]}" "${sess[@]}" -X POST -H 'Content-Type: audio/wav' --data-binary @"$wav" "$base/turn" || echo 000)
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

if [[ -n "${GALION_CORS_ORIGINS:-}" && -z "$(cors_origin)" ]]; then
  echo "GALION_CORS_ORIGINS must list exact origins. A literal * is not the V1 wildcard." >&2
  exit 1
fi

echo "Galion Voice phone pipeline · $BASE"
probe "$BASE" local
if [[ -n "${NGROK_URL:-}" ]]; then
  echo "public edge · $NGROK_URL"
  probe "${NGROK_URL%/}" ngrok
fi
echo "result: $pass passed, $fail failed"
[[ "$fail" == "0" ]]
