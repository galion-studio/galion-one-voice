#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Supervise samantha-local. Linux only. Loopback bind 127.0.0.1.
# Paths come from the env file. This script hard-codes no home and no Windows binary name.
#
#   samantha-local-supervise --systemd   one generation; systemd restarts on failure
#   samantha-local-supervise             restart loop with exponential backoff
#
# GET /health must return HTTP 200 and JSON "ok": true.
# N failures in a row (SAMANTHA_HEALTH_FAILS) restart the process.
# A clean exit 0 stops. A signal stop stops. Any other exit restarts.
# Exit 2 is a config error. The user unit sets RestartPreventExitStatus=2.

set -euo pipefail
export LC_ALL=C

mode=loop
env_file=${SAMANTHA_ENV_FILE:-}
child=
child_group=0
stop_requested=0
pid_path=
generation_reason="exit"
generation_status=0
generation_healthy=0
generation_started=0

die() {
  local code=$1
  shift
  printf 'samantha-supervise: %s\n' "$*" >&2
  exit "$code"
}

log() {
  printf '%s samantha-supervise: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)" "$*" >&2
}

usage() {
  cat <<'EOF'
usage: supervise.sh [--systemd|--check|--probe|--paths|--status|--stop] [--env FILE]

  (default)   foreground restart loop with backoff
  --systemd   run one generation and exit 1 on crash or unhealthy health
  --check     validate the env file and the binary, then exit
  --probe     one GET /health; exit 0 when healthy
  --paths     print bind, bin, ui, and workdir
  --status    report the restart-loop pid
  --stop      signal the restart-loop pid
  --env FILE  env file (default: $HOME/.config/galion/samantha-local.env)
EOF
}

pid_file_path() {
  if [[ -n "${XDG_RUNTIME_DIR:-}" && -d "$XDG_RUNTIME_DIR" && -w "$XDG_RUNTIME_DIR" ]]; then
    printf '%s\n' "$XDG_RUNTIME_DIR/samantha-local.pid"
    return 0
  fi
  if [[ -z "${HOME:-}" ]]; then
    return 1
  fi
  printf '%s\n' "$HOME/.local/share/galion/samantha-local.pid"
}

# Reachable from the EXIT trap.
# shellcheck disable=SC2317
release_pid() {
  local got
  [[ -n "$pid_path" && -f "$pid_path" ]] || return 0
  got=$(tr -cd '0-9' <"$pid_path" || true)
  if [[ "$got" == "$$" ]]; then
    rm -f -- "$pid_path"
  fi
}

acquire_pid() {
  local old dir
  if ! pid_path=$(pid_file_path); then
    die 2 "HOME is unset; no place for the pid file"
  fi
  dir=$(dirname -- "$pid_path")
  mkdir -p -- "$dir"
  if [[ -f "$pid_path" ]]; then
    old=$(tr -cd '0-9' <"$pid_path" || true)
    if [[ -n "$old" ]] && kill -0 -- "$old" 2>/dev/null; then
      die 1 "already running (pid ${old})"
    fi
  fi
  printf '%s\n' "$$" >"$pid_path"
}

cmd_status() {
  local path old
  if ! path=$(pid_file_path); then
    printf 'stopped\n'
    exit 1
  fi
  if [[ ! -f "$path" ]]; then
    printf 'stopped\n'
    exit 1
  fi
  old=$(tr -cd '0-9' <"$path" || true)
  if [[ -n "$old" ]] && kill -0 -- "$old" 2>/dev/null; then
    printf 'running %s\n' "$old"
    exit 0
  fi
  printf 'stopped\n'
  exit 1
}

cmd_stop() {
  local path old i
  if ! path=$(pid_file_path); then
    die 2 "HOME is unset; no pid file"
  fi
  if [[ ! -f "$path" ]]; then
    printf 'samantha-supervise: stopped\n'
    exit 0
  fi
  old=$(tr -cd '0-9' <"$path" || true)
  if [[ -z "$old" ]] || ! kill -0 -- "$old" 2>/dev/null; then
    rm -f -- "$path"
    printf 'samantha-supervise: stopped\n'
    exit 0
  fi
  kill -s TERM -- "$old" 2>/dev/null || true
  i=15
  while ((i > 0)); do
    if ! kill -0 -- "$old" 2>/dev/null; then
      printf 'samantha-supervise: stopped\n'
      exit 0
    fi
    sleep 1 || true
    i=$((i - 1))
  done
  die 1 "pid ${old} did not exit"
}

load_env() {
  local line n=0 key
  [[ -n "$env_file" ]] || die 2 "env file path is empty"
  [[ -f "$env_file" ]] || die 2 "env file not found: ${env_file}"
  while IFS= read -r line || [[ -n "$line" ]]; do
    n=$((n + 1))
    [[ "$line" =~ ^[[:space:]]*$ ]] && continue
    [[ "$line" =~ ^[[:space:]]*# ]] && continue
    if [[ "$line" == *$'\r'* ]]; then
      die 2 "env file line ${n} contains a carriage return"
    fi
    if [[ ! "$line" =~ ^[A-Za-z_][A-Za-z0-9_]*=[A-Za-z0-9_./:@+-]*$ ]]; then
      die 2 "env file line ${n} is not a plain KEY=VALUE"
    fi
    key=${line%%=*}
    case "$key" in
      mode | child | child_group | stop_requested | pid_path | env_file | \
        generation_reason | generation_status | generation_healthy | generation_started | \
        IFS | SHELLOPTS | BASHOPTS | BASH_ENV)
        die 2 "env file sets reserved name ${key}"
        ;;
    esac
    printf -v "$key" '%s' "${line#*=}"
    export "${key?}"
  done <"$env_file"
}

set_default() {
  local name=$1 value=$2
  if [[ -z "${!name:-}" ]]; then
    printf -v "$name" '%s' "$value"
    export "${name?}"
  fi
}

need_uint() {
  local name=$1 value=$2 min=$3 max=$4
  if [[ ! "$value" =~ ^[0-9]+$ ]]; then
    die 2 "${name} must be an integer"
  fi
  if ((value < min || value > max)); then
    die 2 "${name} must be from ${min} to ${max}"
  fi
}

valid_bind() {
  local bind=$1 host port
  [[ "$bind" == 127.0.0.1:* ]] || return 1
  host=${bind%%:*}
  port=${bind#*:}
  [[ "$host" == "127.0.0.1" ]] || return 1
  [[ "$port" =~ ^[1-9][0-9]{0,4}$ ]] || return 1
  ((port >= 1 && port <= 65535)) || return 1
}

valid_path() {
  local p=$1
  [[ "$p" == /* ]] || return 1
  [[ "$p" != *$'\n'* && "$p" != *[[:space:]]* ]] || return 1
  [[ "$p" != *'$'* && "$p" != *'`'* && "$p" != *'~'* && "$p" != *\\* ]] || return 1
}

apply_defaults() {
  GALION_SAMANTHA_UI=${GALION_SAMANTHA_UI:-}
  SAMANTHA_LOCAL_WORKDIR=${SAMANTHA_LOCAL_WORKDIR:-}
  set_default SAMANTHA_HEALTH_FAILS 3
  set_default SAMANTHA_HEALTH_INTERVAL 5
  set_default SAMANTHA_HEALTH_GRACE 10
  set_default SAMANTHA_HEALTH_TIMEOUT 2
  set_default SAMANTHA_RESTART_BACKOFF_SEC 2
  set_default SAMANTHA_RESTART_BACKOFF_MAX_SEC 30
  set_default SAMANTHA_STOP_GRACE 5
}

validate_shape() {
  local base win_suffix
  [[ -n "${GALION_SAMANTHA_WS:-}" ]] || die 2 "GALION_SAMANTHA_WS is unset"
  [[ -n "${SAMANTHA_LOCAL_BIN:-}" ]] || die 2 "SAMANTHA_LOCAL_BIN is unset"
  if ! valid_bind "$GALION_SAMANTHA_WS"; then
    die 2 "refusing bind ${GALION_SAMANTHA_WS} (127.0.0.1:PORT only)"
  fi
  if ! valid_path "$SAMANTHA_LOCAL_BIN"; then
    die 2 "SAMANTHA_LOCAL_BIN must be an absolute path"
  fi
  base=${SAMANTHA_LOCAL_BIN##*/}
  win_suffix=$(printf '%s%s' '.' 'exe')
  # shellcheck disable=SC2254
  case "$base" in
    *"$win_suffix" | *"${win_suffix^^}") die 2 "refusing a Windows executable suffix" ;;
  esac
  if [[ -n "$GALION_SAMANTHA_UI" ]] && ! valid_path "$GALION_SAMANTHA_UI"; then
    die 2 "GALION_SAMANTHA_UI must be an absolute path"
  fi
  if [[ -n "$SAMANTHA_LOCAL_WORKDIR" ]] && ! valid_path "$SAMANTHA_LOCAL_WORKDIR"; then
    die 2 "SAMANTHA_LOCAL_WORKDIR must be an absolute path"
  fi
  need_uint SAMANTHA_HEALTH_FAILS "$SAMANTHA_HEALTH_FAILS" 1 1000
  need_uint SAMANTHA_HEALTH_INTERVAL "$SAMANTHA_HEALTH_INTERVAL" 1 86400
  need_uint SAMANTHA_HEALTH_GRACE "$SAMANTHA_HEALTH_GRACE" 0 86400
  need_uint SAMANTHA_HEALTH_TIMEOUT "$SAMANTHA_HEALTH_TIMEOUT" 1 60
  need_uint SAMANTHA_RESTART_BACKOFF_SEC "$SAMANTHA_RESTART_BACKOFF_SEC" 1 86400
  need_uint SAMANTHA_RESTART_BACKOFF_MAX_SEC "$SAMANTHA_RESTART_BACKOFF_MAX_SEC" 1 86400
  need_uint SAMANTHA_STOP_GRACE "$SAMANTHA_STOP_GRACE" 1 86400
  if ((SAMANTHA_RESTART_BACKOFF_MAX_SEC < SAMANTHA_RESTART_BACKOFF_SEC)); then
    die 2 "SAMANTHA_RESTART_BACKOFF_MAX_SEC is below the initial backoff"
  fi
}

validate_ready() {
  command -v curl >/dev/null 2>&1 || die 2 "curl is required"
  if [[ ! -f "$SAMANTHA_LOCAL_BIN" || ! -x "$SAMANTHA_LOCAL_BIN" ]]; then
    die 2 "server binary is not executable: ${SAMANTHA_LOCAL_BIN}"
  fi
  if [[ -n "$SAMANTHA_LOCAL_WORKDIR" && ! -d "$SAMANTHA_LOCAL_WORKDIR" ]]; then
    die 2 "workdir is not a directory: ${SAMANTHA_LOCAL_WORKDIR}"
  fi
  if [[ -n "$GALION_SAMANTHA_UI" && ! -d "$GALION_SAMANTHA_UI" ]]; then
    log "ui directory is missing; the server still starts"
  fi
}

print_paths() {
  printf 'bind=%s\n' "$GALION_SAMANTHA_WS"
  printf 'bin=%s\n' "$SAMANTHA_LOCAL_BIN"
  printf 'ui=%s\n' "$GALION_SAMANTHA_UI"
  printf 'workdir=%s\n' "$SAMANTHA_LOCAL_WORKDIR"
}

health_ok() {
  local body code
  body=$(
    curl --silent --show-error \
      --max-time "$SAMANTHA_HEALTH_TIMEOUT" \
      --connect-timeout "$SAMANTHA_HEALTH_TIMEOUT" \
      --noproxy '*' \
      --proto -all,http \
      --max-redirs 0 \
      --output - \
      --write-out $'\n%{http_code}' \
      "http://${GALION_SAMANTHA_WS}/health" 2>/dev/null || true
  )
  [[ -n "$body" ]] || return 1
  code=${body##*$'\n'}
  body=${body%$'\n'*}
  [[ "$code" == "200" ]] || return 1
  printf '%s' "$body" | grep -Eq '"ok"[[:space:]]*:[[:space:]]*true([^[:alnum:]_]|$)'
}

child_state() {
  local stat
  [[ -n "$child" ]] || return 0
  if ! command -v ps >/dev/null 2>&1; then
    if kill -0 -- "$child" 2>/dev/null; then
      printf 'S\n'
    fi
    return 0
  fi
  stat=$(ps -o stat= -p "$child" 2>/dev/null || true)
  stat=${stat//[[:space:]]/}
  [[ -n "$stat" ]] || return 0
  printf '%s\n' "${stat:0:1}"
}

reap_child() {
  local rc=0
  if [[ -z "$child" ]]; then
    generation_status=0
    return 0
  fi
  set +e
  wait "$child"
  rc=$?
  set -e
  child=
  if ((rc == 127)); then
    generation_status=1
  else
    generation_status=$rc
  fi
}

stop_child() {
  local sig i state
  [[ -n "$child" ]] || return 0
  state=$(child_state)
  if [[ -z "$state" || "$state" == "Z" ]]; then
    return 0
  fi
  for sig in TERM KILL; do
    if ((child > 1)); then
      if ((child_group == 1)); then
        kill -s "$sig" -- "-${child}" 2>/dev/null || kill -s "$sig" -- "$child" 2>/dev/null || true
      else
        kill -s "$sig" -- "$child" 2>/dev/null || true
      fi
    fi
    if [[ "$sig" == "KILL" ]]; then
      break
    fi
    i=$SAMANTHA_STOP_GRACE
    while ((i > 0)); do
      state=$(child_state)
      if [[ -z "$state" || "$state" == "Z" ]]; then
        return 0
      fi
      sleep 1 || true
      if ((stop_requested == 1)); then
        i=$((i - 1))
        continue
      fi
      i=$((i - 1))
    done
  done
}

start_child() {
  child=
  child_group=0
  if command -v setsid >/dev/null 2>&1; then
    child_group=1
    (
      if [[ -n "$SAMANTHA_LOCAL_WORKDIR" ]]; then
        cd -- "$SAMANTHA_LOCAL_WORKDIR" || exit 2
      fi
      # The subshell is not a session leader, so setsid execs the binary
      # in this pid. $! stays the server, and that pid is the process group.
      exec setsid "$SAMANTHA_LOCAL_BIN" < /dev/null
    ) &
  else
    (
      if [[ -n "$SAMANTHA_LOCAL_WORKDIR" ]]; then
        cd -- "$SAMANTHA_LOCAL_WORKDIR" || exit 2
      fi
      exec "$SAMANTHA_LOCAL_BIN" < /dev/null
    ) &
  fi
  child=$!
  log "started pid=${child} bind=${GALION_SAMANTHA_WS}"
}

sleep_slice() {
  local i=$1
  while ((i > 0 && stop_requested == 0)); do
    sleep 1 || true
    i=$((i - 1))
  done
}

watch_generation() {
  local fails=0 state next_probe grace_end
  generation_reason="exit"
  generation_status=0
  generation_healthy=0
  generation_started=$SECONDS
  grace_end=$((generation_started + SAMANTHA_HEALTH_GRACE))
  next_probe=$SECONDS
  start_child
  while ((stop_requested == 0)); do
    state=$(child_state)
    if [[ -z "$state" || "$state" == "Z" ]]; then
      generation_reason="exit"
      reap_child
      return 0
    fi
    if ((SECONDS >= next_probe)); then
      if health_ok; then
        if ((generation_healthy == 0)); then
          log "health ok bind=${GALION_SAMANTHA_WS}"
        fi
        generation_healthy=1
        fails=0
      elif ((stop_requested == 1)); then
        break
      elif ((SECONDS >= grace_end)); then
        fails=$((fails + 1))
        log "health failed ${fails}/${SAMANTHA_HEALTH_FAILS}"
        if ((fails >= SAMANTHA_HEALTH_FAILS)); then
          generation_reason="unhealthy"
          stop_child
          reap_child
          return 0
        fi
      fi
      next_probe=$((SECONDS + SAMANTHA_HEALTH_INTERVAL))
    fi
    sleep_slice 1
  done
  generation_reason="signal"
  stop_child
  reap_child
}

generation_should_stop() {
  if ((stop_requested == 1)); then
    return 0
  fi
  if [[ "$generation_reason" == "signal" ]]; then
    return 0
  fi
  if [[ "$generation_reason" == "exit" && "$generation_status" -eq 0 ]]; then
    return 0
  fi
  return 1
}

# Reachable from the signal trap.
# shellcheck disable=SC2317
on_term() {
  stop_requested=1
}

# Reachable from the EXIT trap.
# shellcheck disable=SC2317
cleanup() {
  stop_child || true
  if [[ -n "$child" ]]; then
    reap_child || true
  fi
  release_pid || true
}

prepare() {
  load_env
  apply_defaults
  validate_shape
}

run_systemd() {
  watch_generation
  if generation_should_stop; then
    log "stopped reason=${generation_reason} status=${generation_status}"
    exit 0
  fi
  log "exiting for restart reason=${generation_reason} status=${generation_status}"
  exit 1
}

run_loop() {
  local delay=$SAMANTHA_RESTART_BACKOFF_SEC
  acquire_pid
  while ((stop_requested == 0)); do
    watch_generation
    if generation_should_stop; then
      log "stopped reason=${generation_reason} status=${generation_status}"
      exit 0
    fi
    if ((generation_healthy == 1)); then
      delay=$SAMANTHA_RESTART_BACKOFF_SEC
    fi
    log "restart in ${delay}s reason=${generation_reason} status=${generation_status}"
    sleep_slice "$delay"
    if ((stop_requested == 1)); then
      log "stopped during backoff"
      exit 0
    fi
    if ((generation_healthy == 0)); then
      delay=$((delay * 2))
      if ((delay > SAMANTHA_RESTART_BACKOFF_MAX_SEC)); then
        delay=$SAMANTHA_RESTART_BACKOFF_MAX_SEC
      fi
    fi
  done
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --systemd) mode=systemd; shift ;;
    --check) mode=check; shift ;;
    --probe) mode=probe; shift ;;
    --paths) mode=paths; shift ;;
    --status) mode=status; shift ;;
    --stop) mode=stop; shift ;;
    --env)
      [[ $# -ge 2 ]] || die 2 "--env needs a file"
      env_file=$2
      shift 2
      ;;
    -h | --help) usage; exit 0 ;;
    *) die 2 "unknown argument ${1}" ;;
  esac
done

if [[ "$mode" == "status" ]]; then
  cmd_status
fi
if [[ "$mode" == "stop" ]]; then
  cmd_stop
fi

if [[ -z "$env_file" ]]; then
  if [[ -z "${HOME:-}" ]]; then
    die 2 "HOME is unset and SAMANTHA_ENV_FILE is unset"
  fi
  env_file=$HOME/.config/galion/samantha-local.env
fi

trap on_term TERM INT
trap cleanup EXIT

prepare

case "$mode" in
  paths)
    print_paths
    exit 0
    ;;
  check)
    validate_ready
    printf 'ok\n'
    exit 0
    ;;
  probe)
    command -v curl >/dev/null 2>&1 || die 2 "curl is required"
    if health_ok; then
      printf 'ok\n'
      exit 0
    fi
    printf 'unhealthy\n' >&2
    exit 1
    ;;
  systemd)
    validate_ready
    run_systemd
    ;;
  loop)
    validate_ready
    run_loop
    ;;
  *) die 2 "unknown mode ${mode}" ;;
esac
