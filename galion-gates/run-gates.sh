#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Live CPU test gates G0–G3 (local) + R0–R3 (remote VPS, PENDING unless GALION_VPS_HOST is set).
#   G0 env probe        : CPU/ISA/RAM/toolchain + bandwidth 3x (CV<5% target) + scalar ref ops
#   G1 tensor correct   : cargo test --workspace (all crates) + G0 scalar reference self-checks
#   G2 GGUF absence     : case-insensitive grep over the code workspace (zero hits)
#   G3 purity denylist  : candle/ort/tract/burn/llama/ggml/gguf absent from manifests + cargo tree
#   --- soft gates (A16): recorded in JSON/summary; FAIL only counts if GALION_GATES_HARD=1 ---
#   G4 render snapshot  : galion-gates/g4-render.sh (NullSurface dot + terminal snapshot vs golden)
#   G6 Rika R1–R9       : galion-gates/g6-rika.sh (slot; A10 owns)
#   PS presence replay  : galion-gates/ps-presence.sh (A13; galion-presence on Mock, specs/presence*.json)
#   LK Eureka locks     : scripts/check-locks.sh (A1)
#   DG demo golden      : galion-demo/golden.sh (A14; canned demo sha256 vs galion-gates/golden/demo.sha256)
#   LV live demo smoke  : galion-gates/lv-live-demo.sh (W21-05; scripts/run-live-demo.sh headless, SKIP if absent)
#   V1 V1 demo smoke    : galion-gates/v1-demo.sh (V1-04; scripts/run-v1-demo.sh --headless, SKIP if absent)
#   OP one-percent      : galion-gates/op-one-percent.sh (V6 engine account; stub vs measured)
#   SY symphony frame   : galion-gates/sy-frame.sh (80 ms tick + 1% stub-step ceiling; GALION_GATES_NO_SY=1 skips)
# Usage: galion-gates/run-gates.sh [--strict-cv]
#   env GALION_VPS_HOST=user@host [GALION_VPS_KEY=path] enables remote gates.
#   env GALION_GATES_HARD=1 makes soft-gate failures (G4/G6/PS/LK/DG/LV/V1/OP/SY) count toward the exit code.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
TS="$(date +%Y%m%dT%H%M%S%z)"
OUT="$HERE/results"; mkdir -p "$OUT" "$HERE/.build"
LOG="$OUT/$TS.log"; JSON="$OUT/$TS.json"
STRICT_CV=0; [ "${1:-}" = "--strict-cv" ] && STRICT_CV=1
declare -A ST; declare -A NOTE
say(){ echo "$*" | tee -a "$LOG"; }
mark(){ ST[$1]=$2; NOTE[$1]="$3"; say "[$2] $1 — $3"; }
DENY='candle|ort|tract|burn|llama|ggml|gguf'
DENY_CRATE_RE="^(candle(-[a-z0-9_-]+)?|ort(-sys)?|tract(-[a-z0-9_-]+)?|burn(-[a-z0-9_-]+)?|llama[a-z0-9_-]*|ggml[a-z0-9_-]*|gguf[a-z0-9_-]*)$"
SELF_EXCL=(--exclude-dir=target --exclude-dir=.git --exclude-dir=results --exclude-dir=.build
           --exclude-dir=galion-gates --exclude=check-no-forbidden.sh)

say "== Galion Voice CPU gates @ $TS  root=$ROOT"

# ---------- G0 env probe ----------
CPU="$(lscpu 2>/dev/null | awk -F: '/Model name/{gsub(/^ +/,"",$2);print $2;exit}')"
ISA="$(grep -m1 '^flags\|^Features' /proc/cpuinfo | tr ' ' '\n' | grep -E -x 'avx512f|avx512bw|avx512_vnni|avx2|fma|f16c|sse4_2|asimd|sve|bf16|amx_tile' | sort -u | tr '\n' ' ')"
MEM_MB="$(awk '/MemTotal/{print int($2/1024)}' /proc/meminfo)"
RUSTC="$(rustc --version 2>/dev/null || echo none)"
say "G0 cpu=$CPU arch=$(uname -m) cores=$(nproc) mem_mb=$MEM_MB isa=[$ISA] rustc=$RUSTC"
PROBE_JSON='{}'
if rustc -O --edition 2021 "$HERE/probe/g0_probe.rs" -o "$HERE/.build/g0_probe" >>"$LOG" 2>&1; then
  PROBE_JSON="$("$HERE/.build/g0_probe")"; say "G0 probe: $PROBE_JSON"
  CV="$(echo "$PROBE_JSON" | sed -E 's/.*"bw_cv_pct":([0-9.]+).*/\1/')"
  if awk "BEGIN{exit !($CV<5)}"; then mark G0 PASS "probe ok, bandwidth CV=${CV}% (<5%)"
  elif [ $STRICT_CV = 1 ]; then mark G0 FAIL "bandwidth CV=${CV}% >= 5% (strict)"
  else mark G0 PASS "probe ok; WARN bandwidth CV=${CV}% >= 5% (shared box noise; strict on VPS)"; fi
else mark G0 FAIL "probe failed to compile (see log)"; fi

# ---------- G1 tensor op correctness ----------
REFOK="$(echo "$PROBE_JSON" | grep -o '"ref_ops_pass":[a-z]*' | cut -d: -f2)"
G1N=""
if [ "$REFOK" != "true" ]; then G1F=1; G1N="scalar ref self-checks FAILED; "; else G1F=0; G1N="scalar ref ops 5/5; "; fi
if ! command -v cargo >/dev/null; then G1F=1; G1N+="cargo missing"
else
  ( cd "$ROOT" && { cargo test --workspace --offline 2>&1 || { echo '-- offline failed, retrying online --'; cargo test --workspace 2>&1; }; } ) > "$OUT/$TS.cargo-test.txt"; RC=$?
  cat "$OUT/$TS.cargo-test.txt" >> "$LOG"
  SUM="$(grep -E '^test result:' "$OUT/$TS.cargo-test.txt" | awk '{p+=$4; f+=$6} END{printf "%d passed, %d failed", p, f}')"
  NTESTS="$(grep -E '^test result:' "$OUT/$TS.cargo-test.txt" | awk '{p+=$4+$6} END{print p+0}')"
  if [ $RC -ne 0 ]; then G1F=1; G1N+="cargo test FAILED (rc=$RC; $SUM)"
  elif [ "${NTESTS:-0}" -eq 0 ]; then G1F=1; G1N+="cargo test ran 0 tests (no op tests yet)"
  else G1N+="cargo test $SUM"; fi
fi
[ $G1F = 0 ] && mark G1 PASS "$G1N" || mark G1 FAIL "$G1N"

# ---------- G2 GGUF absence ----------
HITS="$(grep -RIni "${SELF_EXCL[@]}" -e 'gguf' "$ROOT" 2>/dev/null || true)"
FILES="$(find "$ROOT" \( -path '*/target' -o -path '*/.git' \) -prune -o -type f -iname '*gguf*' -print)"
if [ -z "$HITS$FILES" ]; then mark G2 PASS "0 'gguf' refs, 0 *.gguf files in code workspace"
else say "$HITS"; say "$FILES"; mark G2 FAIL "$(echo "$HITS$FILES" | grep -c .) GGUF hit(s)"; fi
DOCHITS="$(grep -RIli --exclude-dir=galion-voice-code --exclude-dir=target --exclude-dir=.git -e gguf /workspace 2>/dev/null | wc -l)"
say "G2 advisory: $DOCHITS non-code files under /workspace mention 'gguf' (docs/research; not gated)"

# ---------- G3 purity denylist ----------
MHITS="$(grep -RInE --include=Cargo.toml -e "^\s*($DENY)[-a-z0-9_]*\s*=" -e "package\s*=\s*\"($DENY)" "$ROOT" | grep -v '/target/' || true)"
G3N=""; G3F=0
if [ -n "$MHITS" ]; then say "$MHITS"; G3F=1; G3N="denylisted dep in Cargo.toml; "; else G3N="manifests clean; "; fi
TREE="$( cd "$ROOT" && cargo tree --workspace -e normal,build --prefix none --format '{p}' --offline 2>/dev/null || cargo tree --workspace -e normal,build --prefix none --format '{p}' 2>/dev/null )"
if [ -z "$TREE" ]; then G3F=1; G3N+="cargo tree unavailable"
else
  BAD="$(echo "$TREE" | awk '{print $1}' | sort -u | grep -E "$DENY_CRATE_RE" || true)"
  NDEPS="$(echo "$TREE" | awk '{print $1}' | sort -u | wc -l)"
  if [ -n "$BAD" ]; then G3F=1; G3N+="runtime tree has: $(echo $BAD)"; else G3N+="cargo tree clean ($NDEPS crates: $(echo "$TREE" | awk '{print $1}' | sort -u | tr '\n' ' '))"; fi
fi
[ $G3F = 0 ] && mark G3 PASS "$G3N" || mark G3 FAIL "$G3N"

# ---------- Soft gates G4 / G6 / LK (A16) — never change G0–G3 results ----------
HARD="${GALION_GATES_HARD:-0}"
soft(){ # id script...
  local id="$1"; shift
  if [ ! -x "$1" ]; then mark "$id" SKIP "$(basename "$1") not present"; return; fi
  local out rc; out="$("$@" 2>&1)"; rc=$?; echo "$out" >> "$LOG"
  local last; last="$(echo "$out" | grep -E "^($id|OK|FAIL)" | tail -n1)"; [ -n "$last" ] || last="$(echo "$out" | tail -n1)"; last="$(echo "$last" | sed -E "s/^$id [A-Z]+ — //")"
  case $rc in 0) mark "$id" PASS "$last";; 3) mark "$id" SKIP "$last";; *) mark "$id" "$([ "$HARD" = 1 ] && echo FAIL || echo SOFTFAIL)" "rc=$rc $last";; esac
}
soft G4 "$HERE/g4-render.sh"
soft G6 "$HERE/g6-rika.sh"
export GALION_GATES_TS="$TS"; soft PS "$HERE/ps-presence.sh"   # A13 presence replay (S8), reports results/<ts>.presence*.json
soft LK "$ROOT/scripts/check-locks.sh"
soft DG "$ROOT/galion-demo/golden.sh"   # A14 canned demo golden (galion-gates/golden/demo.sha256)
[ "${GALION_GATES_NO_LV:-0}" = 1 ] && mark LV SKIP "GALION_GATES_NO_LV=1" || soft LV "$HERE/lv-live-demo.sh"   # W21-05 live demo smoke (headless)
[ "${GALION_GATES_NO_V1:-0}" = 1 ] && mark V1 SKIP "GALION_GATES_NO_V1=1" || soft V1 "$HERE/v1-demo.sh"   # V1-04 V1 demo smoke (headless)
[ "${GALION_GATES_NO_OP:-0}" = 1 ] && mark OP SKIP "GALION_GATES_NO_OP=1" || soft OP "$HERE/op-one-percent.sh"   # V6 one-percent engine (symphony)
[ "${GALION_GATES_NO_SY:-0}" = 1 ] && mark SY SKIP "GALION_GATES_NO_SY=1" || soft SY "$HERE/sy-frame.sh"   # symphony 80 ms / 1% tick

# ---------- Remote R0–R3 (VPS) ----------
if [ -n "${GALION_VPS_HOST:-}" ]; then
  SSHO=(-o BatchMode=yes -o ConnectTimeout=8); [ -n "${GALION_VPS_KEY:-}" ] && SSHO+=(-i "$GALION_VPS_KEY")
  if ssh "${SSHO[@]}" "$GALION_VPS_HOST" true 2>>"$LOG"; then
    ssh "${SSHO[@]}" "$GALION_VPS_HOST" 'mkdir -p ~/galion-gates-run' &&
    tar -C "$ROOT" --exclude=target --exclude=.git --exclude=galion-gates/results --exclude=galion-gates/.build -czf - . |
      ssh "${SSHO[@]}" "$GALION_VPS_HOST" 'tar -C ~/galion-gates-run -xzf -' &&
    ssh "${SSHO[@]}" "$GALION_VPS_HOST" 'bash ~/galion-gates-run/galion-gates/run-gates.sh --strict-cv' 2>&1 | tee -a "$LOG" | sed 's/^/[vps] /'
    RRC=${PIPESTATUS[0]}
    for g in 0 1 2 3; do mark R$g "$([ $RRC = 0 ] && echo PASS || echo SEE_LOG)" "remote run rc=$RRC"; done
  else for g in 0 1 2 3; do mark R$g PENDING "VPS $GALION_VPS_HOST unreachable (ssh BatchMode)"; done; fi
else for g in 0 1 2 3; do mark R$g PENDING "GALION_VPS_HOST not set — VPS not reachable from this run"; done; fi

# ---------- summary ----------
FAILS=0; { echo "{\"ts\":\"$TS\",\"root\":\"$ROOT\",\"probe\":$PROBE_JSON,\"gates\":{"
  first=1; for g in G0 G1 G2 G3 G4 G6 PS LK DG LV V1 OP SY R0 R1 R2 R3; do
    [ $first = 1 ] || echo ","; first=0
    printf '"%s":{"status":"%s","note":"%s"}' "$g" "${ST[$g]}" "$(echo "${NOTE[$g]}" | sed 's/"/\\"/g')"
    [ "${ST[$g]}" = FAIL ] && FAILS=$((FAILS+1)); done; echo "}}"; } > "$JSON"
say "== SUMMARY: G0=${ST[G0]} G1=${ST[G1]} G2=${ST[G2]} G3=${ST[G3]} | soft G4=${ST[G4]} G6=${ST[G6]} PS=${ST[PS]} LK=${ST[LK]} DG=${ST[DG]} LV=${ST[LV]} V1=${ST[V1]} OP=${ST[OP]} SY=${ST[SY]} | R0-R3=${ST[R0]}  fails=$FAILS  json=$JSON"
exit $([ $FAILS = 0 ] && echo 0 || echo 1)
