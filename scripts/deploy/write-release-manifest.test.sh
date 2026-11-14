#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
# Local checks for the release manifest and the stage unit label.
# No socket. No /opt/galion write. Temp directories only.
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT="$PWD"
WRITE="$ROOT/scripts/deploy/write-release-manifest.sh"
RENDER="$ROOT/scripts/deploy/render-stage-unit.sh"
UNIT="$ROOT/deploy/systemd/templates/galion-voice-stage.service"
OVR_TEMPLATE="$ROOT/deploy/systemd/templates/galion-voice-stage.override"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

command -v jq >/dev/null || fail "jq is required"
command -v sha256sum >/dev/null || fail "sha256sum is required"

expect_rc() {
  local want=$1
  shift
  set +e
  "$@" >"$tmp/out" 2>"$tmp/err"
  local rc=$?
  set -e
  if [[ "$rc" != "$want" ]]; then
    echo "--- stdout ---" >&2
    cat "$tmp/out" >&2 || true
    echo "--- stderr ---" >&2
    cat "$tmp/err" >&2 || true
    fail "want exit $want got $rc: $*"
  fi
}

# The writer and the renderer stay local.
grep -E 'ssh|scp|curl|systemctl|nc ' "$WRITE" "$RENDER" && fail "deploy script names a remote command" || true

sha_a=$(printf 'a%.0s' {1..40})
sha_b=$(printf 'b%.0s' {1..40})
stamp_a="2026-11-14T12:00:00+01:00"
stamp_b="2026-11-14T13:00:00+01:00"

voice="$tmp/voice"
mkdir -p "$voice" "$tmp/src"
printf 'voice-binary-bytes\n' >"$tmp/src/dot_console_demo"
printf 'helper-a-bytes\n' >"$tmp/src/helper-a"
printf 'helper-b-bytes\n' >"$tmp/src/helper-b"
hash_bin=$(sha256sum "$tmp/src/dot_console_demo" | awk '{print $1}')
hash_a=$(sha256sum "$tmp/src/helper-a" | awk '{print $1}')
hash_b=$(sha256sum "$tmp/src/helper-b" | awk '{print $1}')

expect_rc 2 "$WRITE" --execute --root "$voice" --sha "$sha_a" --binary "$tmp/src/dot_console_demo"
grep -q 'local files only' "$tmp/err" || fail "execute refusal text"

expect_rc 1 "$WRITE" --root "$voice" --sha "abc" --binary "$tmp/src/dot_console_demo"
expect_rc 1 "$WRITE" --root "$voice" --sha "ABCDEF0" --binary "$tmp/src/dot_console_demo"
expect_rc 1 "$WRITE" --root "$voice" --sha "$sha_a" --binary "$tmp/src/dot_console_demo" --built-at "2026-11-14T12:00:00+00:00"
expect_rc 1 "$WRITE" --root "$voice" --sha "$sha_a" --binary "$tmp/src/dot_console_demo" --built-at "2026-11-14T12:00:00Z"

"$WRITE" \
  --root "$voice" \
  --sha "$sha_a" \
  --binary "$tmp/src/dot_console_demo" \
  --feature speech \
  --feature dot-viz-web \
  --feature speech \
  --helper "$tmp/src/helper-b" \
  --helper "$tmp/src/helper-a" \
  --built-at "$stamp_a" >"$tmp/out"

man="$voice/release/$sha_a/MANIFEST.json"
[[ -f "$man" ]] || fail "manifest missing"
cmp "$tmp/src/dot_console_demo" "$voice/release/$sha_a/dot_console_demo" || fail "binary bytes changed"
cmp "$tmp/src/helper-a" "$voice/release/$sha_a/helpers/helper-a" || fail "helper-a bytes changed"
cmp "$tmp/src/helper-b" "$voice/release/$sha_a/helpers/helper-b" || fail "helper-b bytes changed"
[[ ! -e "$voice/release/current" ]] || fail "script created the current symlink"

jq -e --arg sha "$sha_a" --arg at "$stamp_a" --arg hash "$hash_bin" --arg ha "$hash_a" --arg hb "$hash_b" '
  .sha == $sha
  and .built_at == $at
  and .timezone == "Europe/Zagreb"
  and .binary == "dot_console_demo"
  and .binary_sha256 == $hash
  and .features == ["dot-viz-web", "speech"]
  and .helpers == [
    {"name":"helper-a","path":"helpers/helper-a","sha256":$ha},
    {"name":"helper-b","path":"helpers/helper-b","sha256":$hb}
  ]
' "$man" >/dev/null || fail "manifest fields"

jq -e --arg sha "$sha_a" --arg hash "$hash_bin" '
  .timezone == "Europe/Zagreb"
  and .current == null
  and (.releases | length) == 1
  and .releases[0].sha == $sha
  and .releases[0].binary_sha256 == $hash
  and .releases[0].manifest == ("release/" + $sha + "/MANIFEST.json")
' "$voice/release/INDEX.json" >/dev/null || fail "index fields"

# Idempotent manifest when the stamp is fixed.
cp "$man" "$tmp/man1"
"$WRITE" \
  --root "$voice" \
  --sha "$sha_a" \
  --binary "$voice/release/$sha_a/dot_console_demo" \
  --feature dot-viz-web \
  --feature speech \
  --helper "$voice/release/$sha_a/helpers/helper-a" \
  --helper "$voice/release/$sha_a/helpers/helper-b" \
  --built-at "$stamp_a" >/dev/null
cmp "$tmp/man1" "$man" || fail "manifest changed on a second write"

printf 'other-binary\n' >"$tmp/src/other-bin"
"$WRITE" \
  --root "$voice" \
  --sha "$sha_b" \
  --binary "$tmp/src/other-bin" \
  --built-at "$stamp_b" >/dev/null
ln -s "$sha_b" "$voice/release/current"
"$WRITE" \
  --root "$voice" \
  --sha "$sha_b" \
  --binary "$voice/release/$sha_b/other-bin" \
  --built-at "$stamp_b" >/dev/null
jq -e --arg a "$sha_a" --arg b "$sha_b" '
  .current == $b
  and .releases[0].sha == $a
  and .releases[1].sha == $b
  and .releases[1].features == []
  and .releases[1].helpers == []
' "$voice/release/INDEX.json" >/dev/null || fail "index order or current"

# Clock path: the stamp is Europe/Zagreb and falls inside the run window.
clock_root="$tmp/clock"
mkdir -p "$clock_root"
before=$(TZ=Europe/Zagreb date +%s)
"$WRITE" --root "$clock_root" --sha "$sha_a" --binary "$tmp/src/dot_console_demo" >/dev/null
after=$(TZ=Europe/Zagreb date +%s)
clock_stamp=$(jq -r '.built_at' "$clock_root/release/$sha_a/MANIFEST.json")
[[ "$clock_stamp" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\+0[12]:00$ ]] || fail "clock stamp $clock_stamp"
clock_epoch=$(TZ=Europe/Zagreb date -d "$clock_stamp" +%s)
[[ "$clock_epoch" -ge "$before" && "$clock_epoch" -le "$after" ]] || fail "clock stamp outside the run"
jq -e '.timezone == "Europe/Zagreb"' "$clock_root/release/$sha_a/MANIFEST.json" >/dev/null || fail "clock timezone"

# Stage unit: Description has no commit hash. The override supplies it.
desc=$(grep -E '^Description=' "$UNIT")
[[ "$desc" == "Description=Galion Voice stage" ]] || fail "unit Description changed: $desc"
printf '%s\n' "$desc" | grep -Eq '[0-9a-f]{7,}' && fail "unit Description contains a SHA" || true
grep -q '^SHA=$' "$OVR_TEMPLATE" || fail "override template must ship with SHA empty"

expect_rc 0 "$RENDER" --check
expect_rc 2 "$RENDER" --execute --override "$OVR_TEMPLATE" --out "$tmp/rendered"
expect_rc 1 "$RENDER" --override "$OVR_TEMPLATE" --out "$tmp/rendered-empty"
grep -q 'override SHA' "$tmp/err" || fail "empty SHA refusal"

printf 'SHA=%s\n' "$sha_a" >"$tmp/stage.override"
"$RENDER" --override "$tmp/stage.override" --out "$tmp/rendered" >"$tmp/out"
cmp "$UNIT" "$tmp/rendered/galion-voice-stage.service" || fail "rendered unit is not the template"
grep -q "^Description=Galion Voice stage ($sha_a)$" "$tmp/rendered/galion-voice-stage.service.d/override.conf" || fail "drop-in Description"
grep -q "^Environment=GALION_RELEASE_SHA=$sha_a$" "$tmp/rendered/galion-voice-stage.service.d/override.conf" || fail "drop-in environment"
grep -q '@SHA@' "$tmp/rendered/galion-voice-stage.service.d/override.conf" && fail "placeholder left in drop-in" || true
# The copied unit still has the plain Description.
grep -q '^Description=Galion Voice stage$' "$tmp/rendered/galion-voice-stage.service" || fail "rendered unit Description"

bad_unit="$tmp/bad.service"
sed 's/^Description=.*/Description=Galion Voice stage (0123456789abcdef)/' "$UNIT" >"$bad_unit"
expect_rc 1 "$RENDER" --unit "$bad_unit" --override "$tmp/stage.override" --out "$tmp/bad-out"
grep -q 'hard-coded SHA' "$tmp/err" || fail "hard-coded SHA was accepted"

expect_rc 2 "$RENDER" --override "$tmp/stage.override" --out /etc/systemd/system

echo "OK: release manifest and stage unit label"
