#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Bats-free smoke for deploy/linux. Linux only. No server build. No download.
# systemd-analyze offline treats a user unit as a system unit because it has
# no User= line. A user unit must not set User=. The "runs as root" row is
# that analyzer artifact.

set -euo pipefail
export LC_ALL=C

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
REPO=$(CDPATH='' cd -- "$ROOT/../.." && pwd)
SUPER=$ROOT/supervise.sh
INSTALL=$ROOT/install.sh
UNIT=$ROOT/samantha-local.service
EXAMPLE=$ROOT/samantha-local.env.example
WORK=$(mktemp -d)
LAST_ERR=
pids=()

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  if [[ -n "$LAST_ERR" && -f "$LAST_ERR" ]]; then
    printf '--- stderr ---\n' >&2
    cat "$LAST_ERR" >&2
  fi
  exit 1
}

kill_tree() {
  local pid=$1 sig=$2 kids k
  kids=$(ps -o pid= --ppid "$pid" 2>/dev/null || true)
  for k in $kids; do
    kill_tree "$k" "$sig"
  done
  kill -s "$sig" -- "$pid" 2>/dev/null || true
}

cleanup() {
  local p
  if ((${#pids[@]} > 0)); then
    for p in "${pids[@]}"; do
      kill_tree "$p" TERM
    done
    sleep 0.2
    for p in "${pids[@]}"; do
      kill_tree "$p" KILL
    done
  fi
  case "$WORK" in
    /tmp/* | /var/tmp/*) rm -rf -- "$WORK" ;;
  esac
}
trap cleanup EXIT

export XDG_RUNTIME_DIR=$WORK/run
mkdir -p "$XDG_RUNTIME_DIR" "$WORK/ui" "$WORK/work" "$WORK/bin"

need() {
  command -v "$1" >/dev/null 2>&1 || fail "missing $1"
}

need shellcheck
need systemd-analyze
need curl
need python3
need date

shellcheck -x "$SUPER" "$INSTALL" "$ROOT/ci/smoke.sh"
printf 'OK: shellcheck\n'

scan_tree() {
  local hits bad file
  local -a files=(
    "$SUPER"
    "$INSTALL"
    "$UNIT"
    "$EXAMPLE"
    "$ROOT/README.md"
    "$REPO/.github/workflows/deploy-linux.yml"
  )
  hits=$(grep -nE '\b([0-9]{1,3}\.){3}[0-9]{1,3}\b' "${files[@]}" || true)
  bad=$(printf '%s\n' "$hits" | grep -v '127\.0\.0\.1' || true)
  if printf '%s\n' "$bad" | grep -q '[0-9]'; then
    printf '%s\n' "$bad" >&2
    fail "found an address other than 127.0.0.1"
  fi
  for file in "${files[@]}"; do
    if grep -nE '/home/|/Users/|\.exe' "$file"; then
      fail "found a home path or a Windows executable name in ${file}"
    fi
    if grep -nE 'api[_-]?key|password|secret|BEGIN [A-Z]+ PRIVATE' "$file"; then
      fail "found a credential-shaped string in ${file}"
    fi
  done
}
scan_tree
printf 'OK: tree scan\n'

for key in \
  '^Description=Samantha local voice server \(loopback\)$' \
  '^EnvironmentFile=%h/\.config/galion/samantha-local\.env$' \
  '^ExecStart=%h/\.local/bin/samantha-local-supervise --systemd$' \
  '^Restart=on-failure$' \
  '^RestartSec=2$' \
  '^RestartPreventExitStatus=2$' \
  '^StartLimitIntervalSec=120$' \
  '^StartLimitBurst=8$' \
  '^NoNewPrivileges=yes$' \
  '^ProtectSystem=strict$' \
  '^ProtectHome=yes$' \
  '^PrivateTmp=yes$' \
  '^RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6$' \
  '^IPAddressDeny=any$' \
  '^IPAddressAllow=localhost$' \
  '^MemoryMax=2G$' \
  '^WantedBy=default\.target$'
do
  grep -Eq "$key" "$UNIT" || fail "unit missing $key"
done
if grep -Eq '^User=' "$UNIT"; then
  fail "user unit must not set User="
fi
if grep -Eq '0\.0\.0\.0|GALION_LLM|GALION_SAMANTHA_REF=' "$UNIT"; then
  fail "unit binds a wildcard, sets an LLM, or stores a reference path"
fi
printf 'OK: unit keys\n'

verify_out=$WORK/verify.out
set +e
systemd-analyze verify "$UNIT" >"$verify_out" 2>&1
verify_rc=$?
set -e
allowed='^samantha-local\.service: Command .+/\.local/bin/samantha-local-supervise is not executable: No such file or directory$'
while IFS= read -r line || [[ -n "$line" ]]; do
  [[ -z "$line" ]] && continue
  if [[ "$line" =~ $allowed ]]; then
    continue
  fi
  fail "systemd-analyze verify: $line"
done <"$verify_out"
if [[ "$verify_rc" -ne 0 && ! -s "$verify_out" ]]; then
  fail "systemd-analyze verify exited ${verify_rc} with no diagnostic"
fi
sec=$(systemd-analyze security --offline=yes --no-pager "$UNIT" 2>&1) || fail "systemd-analyze security failed"
score=$(printf '%s\n' "$sec" | sed -n 's/.*Overall exposure level for .*: \([0-9][0-9]*\(\.[0-9][0-9]*\)\?\).*/\1/p' | head -n 1)
[[ -n "$score" ]] || fail "no exposure score"
if awk -v s="$score" -v c="5.0" 'BEGIN { exit !(s > c) }'; then
  fail "exposure ${score} is worse than 5.0"
fi
printf 'OK: systemd-analyze exposure %s\n' "$score"

paths_out=$WORK/paths.out
"$SUPER" --paths --env "$EXAMPLE" >"$paths_out"
grep -q '^bind=127\.0\.0\.1:7881$' "$paths_out" || fail "example bind"
grep -q '^bin=/path/to/samantha-local$' "$paths_out" || fail "example bin"
set +e
"$SUPER" --check --env "$EXAMPLE" >"$WORK/check.out" 2>"$WORK/check.err"
check_rc=$?
set -e
[[ "$check_rc" -eq 2 ]] || fail "example --check should exit 2, got ${check_rc}"
printf 'OK: example env\n'

expect_exit() {
  local want=$1
  shift
  local err=$WORK/expect.err
  LAST_ERR=$err
  set +e
  "$@" >"$WORK/expect.out" 2>"$err"
  local rc=$?
  set -e
  [[ "$rc" -eq "$want" ]] || fail "exit ${rc}, want ${want}: $*"
}

bad_env=$WORK/bad.env
cat >"$bad_env" <<'EOF'
SAMANTHA_LOCAL_BIN=/path/to/samantha-local
GALION_SAMANTHA_WS=127.0.0.1:7881
EVIL=$(id)
EOF
expect_exit 2 "$SUPER" --paths --env "$bad_env"
grep -q 'not a plain KEY=VALUE' "$LAST_ERR" || fail "substitution was not refused"

cat >"$bad_env" <<'EOF'
SAMANTHA_LOCAL_BIN=/path/to/samantha-local
GALION_SAMANTHA_WS=localhost:7881
EOF
expect_exit 2 "$SUPER" --paths --env "$bad_env"
grep -q 'refusing bind' "$LAST_ERR" || fail "hostname bind was not refused"

z=0
wild="${z}.${z}.${z}.${z}:9"
cat >"$bad_env" <<EOF
SAMANTHA_LOCAL_BIN=/path/to/samantha-local
GALION_SAMANTHA_WS=${wild}
EOF
expect_exit 2 "$SUPER" --paths --env "$bad_env"
grep -q 'refusing bind' "$LAST_ERR" || fail "wildcard bind was not refused"

cat >"$bad_env" <<'EOF'
SAMANTHA_LOCAL_BIN=/path/to/samantha-local
GALION_SAMANTHA_WS=[::1]:7881
EOF
expect_exit 2 "$SUPER" --paths --env "$bad_env"

win_suffix=$(printf '%s%s' '.' 'exe')
win_bin=$WORK/bin/samantha-local${win_suffix}
printf '#!/bin/sh\nexit 0\n' >"$win_bin"
chmod 755 "$win_bin"
cat >"$bad_env" <<EOF
SAMANTHA_LOCAL_BIN=${win_bin}
GALION_SAMANTHA_WS=127.0.0.1:7881
EOF
expect_exit 2 "$SUPER" --check --env "$bad_env"
grep -q 'Windows executable suffix' "$LAST_ERR" || fail "Windows suffix was not refused"

cat >"$bad_env" <<'EOF'
SAMANTHA_LOCAL_BIN=samantha-local
GALION_SAMANTHA_WS=127.0.0.1:7881
EOF
expect_exit 2 "$SUPER" --paths --env "$bad_env"
printf 'OK: refusals\n'

free_port() {
  python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()'
}

cat >"$WORK/health.py" <<'PY'
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def do_GET(self):
        mode = os.environ.get("STUB_HEALTH", "ok")
        path = self.path.split("?", 1)[0]
        if path != "/health":
            body, code = b"no", 404
        elif mode == "bad":
            body, code = b'{"ok":false}', 200
        elif mode == "status":
            body, code = b'{"ok":true}', 503
        elif mode == "gap":
            HITS["n"] += 1
            # Two failures, one success, two failures. A streak of 3 never forms.
            if HITS["n"] in (1, 2, 4, 5):
                body, code = b'{"ok":false}', 200
            else:
                body, code = b'{"ok":true}', 200
        else:
            body, code = b'{"ok":true}', 200
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, fmt, *args):
        return

HITS = {"n": 0}

ThreadingHTTPServer.allow_reuse_address = True
port = int(os.environ["STUB_PORT"])
ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
PY

cat >"$WORK/bin/stub" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$(date +%s%3N)" >> "$STUB_LOG"
case "${STUB_MODE:-ok}" in
  crash) exit 1 ;;
  exit0) exit 0 ;;
  *)
    export STUB_HEALTH=${STUB_MODE:-ok}
    exec python3 "$STUB_PY"
    ;;
esac
EOF
chmod 755 "$WORK/bin/stub"

write_env() {
  local dest=$1 port=$2
  cat >"$dest" <<EOF
SAMANTHA_LOCAL_BIN=${WORK}/bin/stub
GALION_SAMANTHA_UI=${WORK}/ui
SAMANTHA_LOCAL_WORKDIR=${WORK}/work
GALION_SAMANTHA_WS=127.0.0.1:${port}
SAMANTHA_HEALTH_FAILS=${FAILS:-3}
SAMANTHA_HEALTH_INTERVAL=${INTERVAL:-1}
SAMANTHA_HEALTH_GRACE=${GRACE:-0}
SAMANTHA_HEALTH_TIMEOUT=${TIMEOUT:-1}
SAMANTHA_RESTART_BACKOFF_SEC=${BACKOFF:-1}
SAMANTHA_RESTART_BACKOFF_MAX_SEC=${BACKOFF_MAX:-8}
SAMANTHA_STOP_GRACE=${STOP_GRACE:-1}
EOF
}

start_python() {
  local port=$1 mode=$2
  STUB_PORT=$port STUB_HEALTH=$mode python3 "$WORK/health.py" &
  pids+=("$!")
  local i
  for ((i = 0; i < 50; i++)); do
    if curl --silent --max-time 1 --noproxy '*' --proto -all,http \
      --output /dev/null "http://127.0.0.1:${port}/health"; then
      return 0
    fi
    sleep 0.05
  done
  fail "python health server did not bind"
}

port=$(free_port)
start_python "$port" ok
write_env "$WORK/probe.env" "$port"
expect_exit 0 "$SUPER" --probe --env "$WORK/probe.env"
kill_tree "${pids[-1]}" TERM
wait "${pids[-1]}" 2>/dev/null || true

port=$(free_port)
start_python "$port" bad
write_env "$WORK/probe-bad.env" "$port"
expect_exit 1 "$SUPER" --probe --env "$WORK/probe-bad.env"
grep -q 'unhealthy' "$LAST_ERR" || fail "probe did not report unhealthy"
kill_tree "${pids[-1]}" TERM
wait "${pids[-1]}" 2>/dev/null || true

port=$(free_port)
start_python "$port" status
write_env "$WORK/probe-503.env" "$port"
expect_exit 1 "$SUPER" --probe --env "$WORK/probe-503.env"
kill_tree "${pids[-1]}" TERM
wait "${pids[-1]}" 2>/dev/null || true

port=$(free_port)
write_env "$WORK/probe-down.env" "$port"
expect_exit 1 "$SUPER" --probe --env "$WORK/probe-down.env"
printf 'OK: health probe\n'

run_fg() {
  local secs=$1 err=$2
  shift 2
  LAST_ERR=$err
  "$@" >"${err}.out" 2>"$err" &
  local pid=$!
  pids+=("$pid")
  local i
  for ((i = 0; i < secs * 10; i++)); do
    if ! kill -0 -- "$pid" 2>/dev/null; then
      set +e
      wait "$pid"
      local rc=$?
      set -e
      printf '%s\n' "$rc"
      return 0
    fi
    sleep 0.1
  done
  kill_tree "$pid" TERM
  sleep 0.3
  kill_tree "$pid" KILL
  wait "$pid" 2>/dev/null || true
  printf '124\n'
}

export STUB_PY=$WORK/health.py

FAILS=5 INTERVAL=1 GRACE=30 TIMEOUT=1 BACKOFF=2 BACKOFF_MAX=8 STOP_GRACE=1
port=$(free_port)
export STUB_LOG=$WORK/crash.log STUB_MODE=crash STUB_PORT=$port
: >"$STUB_LOG"
write_env "$WORK/crash.env" "$port"
rc=$(run_fg 15 "$WORK/crash.err" "$SUPER" --systemd --env "$WORK/crash.env")
[[ "$rc" -eq 1 ]] || fail "systemd crash exit ${rc}"
starts=$(wc -l <"$STUB_LOG")
starts=${starts// /}
[[ "$starts" -eq 1 ]] || fail "systemd mode restarted a one-shot crash (${starts})"
grep -q 'reason=exit' "$WORK/crash.err" || fail "crash log missing reason=exit"
printf 'OK: systemd restarts on crash via exit 1\n'

FAILS=2 INTERVAL=1 GRACE=0 TIMEOUT=1 BACKOFF=1 BACKOFF_MAX=8 STOP_GRACE=1
port=$(free_port)
export STUB_LOG=$WORK/bad.log STUB_MODE=bad STUB_PORT=$port
: >"$STUB_LOG"
write_env "$WORK/bad.env" "$port"
rc=$(run_fg 15 "$WORK/bad.err" "$SUPER" --systemd --env "$WORK/bad.env")
[[ "$rc" -eq 1 ]] || fail "unhealthy exit ${rc}"
starts=$(wc -l <"$STUB_LOG")
starts=${starts// /}
[[ "$starts" -eq 1 ]] || fail "unhealthy systemd mode started ${starts} times"
grep -q 'health failed 2/2' "$WORK/bad.err" || fail "health counter did not reach 2"
grep -q 'reason=unhealthy' "$WORK/bad.err" || fail "missing unhealthy reason"
printf 'OK: unhealthy N restarts\n'

FAILS=3 INTERVAL=1 GRACE=0 TIMEOUT=1 BACKOFF=1 BACKOFF_MAX=4 STOP_GRACE=1
port=$(free_port)
export STUB_LOG=$WORK/clean.log STUB_MODE=exit0 STUB_PORT=$port
: >"$STUB_LOG"
write_env "$WORK/clean.env" "$port"
rc=$(run_fg 10 "$WORK/clean.err" "$SUPER" --systemd --env "$WORK/clean.env")
[[ "$rc" -eq 0 ]] || fail "clean exit ${rc}"
starts=$(wc -l <"$STUB_LOG")
starts=${starts// /}
[[ "$starts" -eq 1 ]] || fail "clean exit restarted (${starts})"
printf 'OK: clean exit stays down\n'

FAILS=5 INTERVAL=1 GRACE=30 TIMEOUT=1 BACKOFF=2 BACKOFF_MAX=8 STOP_GRACE=1
port=$(free_port)
export STUB_LOG=$WORK/loop.log STUB_MODE=crash STUB_PORT=$port
: >"$STUB_LOG"
write_env "$WORK/loop.env" "$port"
LAST_ERR=$WORK/loop.err
"$SUPER" --env "$WORK/loop.env" >"$WORK/loop.out" 2>"$WORK/loop.err" &
loop_pid=$!
pids+=("$loop_pid")
ready=0
for ((i = 0; i < 100; i++)); do
  if [[ -f "$STUB_LOG" ]] && [[ "$(wc -l <"$STUB_LOG" | tr -d ' ')" -ge 3 ]]; then
    ready=1
    break
  fi
  if ! kill -0 -- "$loop_pid" 2>/dev/null; then
    fail "restart loop exited early"
  fi
  sleep 0.2
done
[[ "$ready" -eq 1 ]] || fail "restart loop did not start three times"
kill -s TERM -- "$loop_pid"
set +e
wait "$loop_pid"
loop_rc=$?
set -e
[[ "$loop_rc" -eq 0 ]] || fail "loop stop exit ${loop_rc}"
grep -q 'restart in 2s' "$WORK/loop.err" || fail "missing 2s backoff"
grep -q 'restart in 4s' "$WORK/loop.err" || fail "backoff did not double"
printf 'OK: restart loop backoff\n'

FAILS=3 INTERVAL=1 GRACE=2 TIMEOUT=1 BACKOFF=1 BACKOFF_MAX=4 STOP_GRACE=1
port=$(free_port)
export STUB_LOG=$WORK/ok.log STUB_MODE=ok STUB_PORT=$port
: >"$STUB_LOG"
write_env "$WORK/ok.env" "$port"
LAST_ERR=$WORK/ok.err
"$SUPER" --env "$WORK/ok.env" >"$WORK/ok.out" 2>"$WORK/ok.err" &
ok_pid=$!
pids+=("$ok_pid")
saw=0
for ((i = 0; i < 50; i++)); do
  if grep -q 'health ok' "$WORK/ok.err" 2>/dev/null; then
    saw=1
    break
  fi
  sleep 0.1
done
[[ "$saw" -eq 1 ]] || fail "healthy server was not reported ok"
sleep 2
starts=$(wc -l <"$STUB_LOG")
starts=${starts// /}
[[ "$starts" -eq 1 ]] || fail "healthy server was restarted (${starts})"
kill -s TERM -- "$ok_pid"
set +e
wait "$ok_pid"
ok_rc=$?
set -e
[[ "$ok_rc" -eq 0 ]] || fail "healthy stop exit ${ok_rc}"
if grep -q 'restart in' "$WORK/ok.err"; then
  fail "healthy server scheduled a restart"
fi
printf 'OK: healthy server stays up\n'

# The listener is already up. The child only has to stay alive.
# Probes are bad, bad, ok, bad, bad. N=3 must not restart when ok clears the streak.
printf '#!/bin/sh\nsleep 30\n' >"$WORK/bin/stay"
chmod 755 "$WORK/bin/stay"
FAILS=3 INTERVAL=1 GRACE=0 TIMEOUT=1 BACKOFF=1 BACKOFF_MAX=4 STOP_GRACE=1
port=$(free_port)
start_python "$port" gap
cat >"$WORK/gap.env" <<EOF
SAMANTHA_LOCAL_BIN=${WORK}/bin/stay
GALION_SAMANTHA_UI=${WORK}/ui
SAMANTHA_LOCAL_WORKDIR=${WORK}/work
GALION_SAMANTHA_WS=127.0.0.1:${port}
SAMANTHA_HEALTH_FAILS=3
SAMANTHA_HEALTH_INTERVAL=1
SAMANTHA_HEALTH_GRACE=0
SAMANTHA_HEALTH_TIMEOUT=1
SAMANTHA_RESTART_BACKOFF_SEC=1
SAMANTHA_RESTART_BACKOFF_MAX_SEC=4
SAMANTHA_STOP_GRACE=1
EOF
LAST_ERR=$WORK/gap.err
"$SUPER" --systemd --env "$WORK/gap.env" >"$WORK/gap.out" 2>"$WORK/gap.err" &
gap_pid=$!
pids+=("$gap_pid")
sleep 6
if ! kill -0 -- "$gap_pid" 2>/dev/null; then
  set +e
  wait "$gap_pid"
  gap_rc=$?
  set -e
  fail "streak reset exited ${gap_rc}"
fi
kill -s TERM -- "$gap_pid"
set +e
wait "$gap_pid"
gap_rc=$?
set -e
[[ "$gap_rc" -eq 0 ]] || fail "streak reset stop exit ${gap_rc}"
if grep -q 'health failed 3/3' "$WORK/gap.err"; then
  fail "a healthy probe did not clear the failure streak"
fi
printf 'OK: a healthy probe clears the failure streak\n'

prefix=$WORK/prefix
mkdir -p "$prefix/.config/galion" "$WORK/real-ui" "$WORK/real-work"
real_bin=$WORK/bin/samantha-local
printf '#!/bin/sh\nexit 0\n' >"$real_bin"
chmod 755 "$real_bin"
cat >"$prefix/.config/galion/samantha-local.env" <<EOF
# seeded
SAMANTHA_LOCAL_BIN=${real_bin}
GALION_SAMANTHA_UI=${WORK}/real-ui
SAMANTHA_LOCAL_WORKDIR=${WORK}/real-work
GALION_SAMANTHA_WS=127.0.0.1:7881
SAMANTHA_HEALTH_FAILS=3
SAMANTHA_HEALTH_INTERVAL=5
SAMANTHA_HEALTH_GRACE=10
SAMANTHA_HEALTH_TIMEOUT=2
SAMANTHA_RESTART_BACKOFF_SEC=2
SAMANTHA_RESTART_BACKOFF_MAX_SEC=30
SAMANTHA_STOP_GRACE=5
EOF
chmod 600 "$prefix/.config/galion/samantha-local.env"
marker=$WORK/systemctl-called
mkdir -p "$WORK/fakebin"
cat >"$WORK/fakebin/systemctl" <<EOF
#!/bin/sh
echo called > "$marker"
exit 99
EOF
chmod 755 "$WORK/fakebin/systemctl"

dry=$WORK/dry-home
dry_out=$WORK/dry.out
PATH="$WORK/fakebin:$PATH" "$INSTALL" install --prefix "$dry" --dry-run >"$dry_out"
[[ ! -e "$dry" ]] || fail "dry-run created ${dry}"
grep -q 'GALION_LINUX_SUPERVISE=dry-run' "$dry_out" || fail "dry-run marker"
grep -q 'dry-run: would install' "$dry_out" || fail "dry-run plan"
[[ ! -e "$marker" ]] || fail "dry-run called systemctl"

inst_out=$WORK/inst.out
PATH="$WORK/fakebin:$PATH" "$INSTALL" install --prefix "$prefix" >"$inst_out"
[[ ! -e "$marker" ]] || fail "prefix install called systemctl"
grep -q 'GALION_LINUX_SUPERVISE=installed' "$inst_out" || fail "install marker"
[[ -x "$prefix/.local/bin/samantha-local-supervise" ]] || fail "supervisor not installed"
[[ -f "$prefix/.config/systemd/user/samantha-local.service" ]] || fail "unit not installed"
grep -q '^WantedBy=default\.target$' "$prefix/.config/systemd/user/samantha-local.service" || fail "installed unit"
grep -q "ReadOnlyPaths=${real_bin}" "$prefix/.config/systemd/user/samantha-local.service.d/paths.conf" || fail "drop-in binary"
grep -q "ReadOnlyPaths=${WORK}/real-ui" "$prefix/.config/systemd/user/samantha-local.service.d/paths.conf" || fail "drop-in ui"
grep -q "ReadWritePaths=${WORK}/real-work" "$prefix/.config/systemd/user/samantha-local.service.d/paths.conf" || fail "drop-in workdir"
grep -q '^# seeded$' "$prefix/.config/galion/samantha-local.env" || fail "env file was replaced"
mode=$(stat -c '%a' "$prefix/.config/galion/samantha-local.env")
[[ "$mode" == "600" ]] || fail "env mode ${mode}"

inst2=$WORK/inst2.out
"$INSTALL" install --prefix "$prefix" >"$inst2"
grep -q "unchanged $prefix/.local/bin/samantha-local-supervise" "$inst2" || fail "script was rewritten"
grep -q "unchanged $prefix/.config/systemd/user/samantha-local.service" "$inst2" || fail "unit was rewritten"
grep -q "unchanged $prefix/.config/galion/samantha-local.env" "$inst2" || fail "env was rewritten"
grep -q 'GALION_LINUX_SUPERVISE=installed' "$inst2" || fail "second install marker"

set +e
"$INSTALL" enable --prefix "$prefix" >"$WORK/enable.out" 2>"$WORK/enable.err"
enable_rc=$?
set -e
[[ "$enable_rc" -eq 2 ]] || fail "enable --prefix exit ${enable_rc}"
grep -q 'refuses --prefix' "$WORK/enable.err" || fail "enable --prefix message"

set +e
status_out=$("$INSTALL" status --prefix "$prefix" 2>&1)
status_rc=$?
set -e
[[ "$status_rc" -eq 1 ]] || fail "status exit ${status_rc}"
printf '%s\n' "$status_out" | grep -q '^stopped$' || fail "status output"

"$INSTALL" stop --prefix "$prefix" >"$WORK/stop.out"
grep -q 'stopped' "$WORK/stop.out" || fail "stop output"
"$INSTALL" logs --prefix "$prefix" >"$WORK/logs.out"
grep -q 'journalctl --user -u samantha-local.service' "$WORK/logs.out" || fail "logs hint"

old_unit=$prefix/.config/systemd/user/samantha-local.service
printf 'OLD-UNIT\n' >"$old_unit"
"$INSTALL" install --prefix "$prefix" >"$WORK/inst3.out"
[[ -f "${old_unit}.bak" ]] || fail "missing unit backup"
grep -q 'OLD-UNIT' "${old_unit}.bak" || fail "backup contents"
grep -q '^WantedBy=default\.target$' "$old_unit" || fail "unit was not replaced"

roll=$WORK/roll.out
"$INSTALL" rollback --prefix "$prefix" >"$roll"
grep -q 'GALION_LINUX_SUPERVISE=rolled-back' "$roll" || fail "rollback marker"
grep -q 'restored ' "$roll" || fail "backup was not restored"
grep -q 'OLD-UNIT' "$old_unit" || fail "restored unit contents"
[[ ! -e "$prefix/.local/bin/samantha-local-supervise" ]] || fail "supervisor remained"
[[ ! -e "$prefix/.config/systemd/user/samantha-local.service.d" ]] || fail "drop-in remained"
grep -q '^# seeded$' "$prefix/.config/galion/samantha-local.env" || fail "rollback removed the env file"
grep -q 'kept ' "$roll" || fail "rollback did not say it kept the env file"
printf 'OK: install, enable refusal, rollback\n'

grep -q 'systemctl --user enable samantha-local.service' "$INSTALL" || fail "enable command missing"
grep -q 'loginctl enable-linger' "$ROOT/README.md" || fail "linger docs missing"
grep -q 'WantedBy=default.target' "$UNIT" || fail "boot target missing"
printf 'OK: boot autostart is wired\n'
printf 'OK: deploy/linux smoke\n'
