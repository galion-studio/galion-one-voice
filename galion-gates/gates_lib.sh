# Galion Voice — Perpetual Open MIT License
# Shared decisions for run-gates.sh, g4-render.sh, and selftest.sh.
# Sourced, not executed. No I/O.

# "remote" when a VPS host is configured, otherwise "skip".
# Skip is the honest result of a local run with no VPS settings.
# A configured host that does not answer is not decided here; run-gates.sh
# marks that case PENDING.
vps_mode() {
  if [ -n "${GALION_VPS_HOST:-}" ]; then
    printf 'remote\n'
  else
    printf 'skip\n'
  fi
}

# Hard failures are the only status that makes run-gates.sh exit 1.
# SKIP, SOFTFAIL, PENDING, PASS, and SEE_LOG do not.
is_hard_fail() {
  [ "${1:-}" = FAIL ]
}

# True when rustc version MAJOR.MINOR is at least 1.85.
# galion-render's path dependencies set rust-version = "1.85".
rustc_at_least_185() {
  local ver="${1:-0}"
  local maj rest min
  maj="${ver%%.*}"
  rest="${ver#*.}"
  min="${rest%%.*}"
  case "$maj" in
    ''|*[!0-9]*) return 1 ;;
  esac
  case "$min" in
    ''|*[!0-9]*) min=0 ;;
  esac
  if [ "$maj" -gt 1 ]; then
    return 0
  fi
  [ "$maj" -eq 1 ] && [ "$min" -ge 85 ]
}
