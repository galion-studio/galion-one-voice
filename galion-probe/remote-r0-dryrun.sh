#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License.
# R0 (VPS probe) — DRY RUN ONLY. Prints the exact commands an operator would run; executes NOTHING remote.
# There is deliberately no --execute flag: R0 is blocked until SSH access is granted and a human runs these.
# Usage: GALION_VPS_HOST=user@host [GALION_VPS_KEY=path] ./remote-r0-dryrun.sh
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"; ROOT="$(cd "$HERE/.." && pwd)"
HOST="${GALION_VPS_HOST:-<user@vps-host>}"; KEY="${GALION_VPS_KEY:+-i $GALION_VPS_KEY }"
for a in "$@"; do case "$a" in --execute|-x) echo "refused: R0 is dry-run only (no SSH from agents)" >&2; exit 2;; esac; done
TS="$(date +%Y%m%dT%H%M%S)"; BUNDLE="/tmp/galion-r0-$TS.tgz"
# Local, side-effect-free preflight
echo "# preflight (local)"
for f in Cargo.toml src/main.rs run-probe.sh validate.sh schema/probe-result.schema.json; do
  [ -f "$HERE/$f" ] && echo "#   ok  galion-probe/$f" || { echo "#   MISSING galion-probe/$f"; exit 1; }; done
grep -q '^\[probe\]' "$ROOT/LOCKS.toml" && echo "#   ok  LOCKS.toml [probe]" || { echo "#   MISSING LOCKS [probe]"; exit 1; }
grep -q '^\[dependencies\]' "$HERE/Cargo.toml" && ! sed -n '/^\[dependencies\]/,/^\[/p' "$HERE/Cargo.toml" | grep -q '=' \
  && echo "#   ok  zero crates.io deps (builds offline on bare VPS)"
cat <<CMDS

# ---- operator commands (NOT executed) ----
tar -C "$ROOT" -czf $BUNDLE LOCKS.toml galion-probe --exclude=galion-probe/target --exclude=galion-probe/results
scp ${KEY}-o BatchMode=yes $BUNDLE $HOST:/tmp/
ssh ${KEY}-o BatchMode=yes $HOST 'mkdir -p /tmp/galion-r0 && tar -C /tmp/galion-r0 -xzf /tmp/$(basename $BUNDLE) \\
  && cd /tmp/galion-r0/galion-probe && command -v cargo && ./run-probe.sh --strict; echo rc=\$?'
scp ${KEY}$HOST:'/tmp/galion-r0/galion-probe/results/probe-*.json' "$HERE/results/"
"$HERE/validate.sh" "$HERE"/results/probe-<vps-host>-*.json
# R0 PASS  <=>  validate OK  AND  .g0=="PASS" (CV < bw_cv_max_pct over bw_runs, mean >= bw_min_gbs_nt)
# Then fill the student-size row in v4/agents/A15-probe-sizing.md from .student_sizing / .student_recommendation
CMDS
echo "# R0 status: PENDING (dry-run; host=$HOST)"
