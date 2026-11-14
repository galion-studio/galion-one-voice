#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
# G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
# Developed and distributed by Galion Labs
# Perpetual Open MIT License
# Copyright © 2026 Galion Labs. All rights reserved.
#
# Renders the stage unit into a local directory. The Description that names
# the release takes SHA= from the override file. The unit template is copied
# unchanged and must not contain a commit hash.
# Does not install, does not talk to systemd, does not open a socket.
# --execute is refused. Output under /etc is refused.

set -euo pipefail

repo=$(cd "$(dirname "$0")/../.." && pwd)
unit="$repo/deploy/systemd/templates/galion-voice-stage.service"
templates="$repo/deploy/systemd/templates"
override=""
out=""
check_only=0

usage() {
  echo "usage: render-stage-unit.sh --override FILE --out DIR [--unit FILE] [--check]" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --execute)
      echo "refusing: local files only (no server)" >&2
      exit 2
      ;;
    --override)
      [[ $# -ge 2 ]] || usage
      override=$2
      shift 2
      ;;
    --out)
      [[ $# -ge 2 ]] || usage
      out=$2
      shift 2
      ;;
    --unit)
      [[ $# -ge 2 ]] || usage
      unit=$2
      shift 2
      ;;
    --check)
      check_only=1
      shift
      ;;
    -h|--help)
      usage
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage
      ;;
  esac
done

[[ -f "$unit" ]] || { echo "unit template missing: $unit" >&2; exit 1; }

desc=$(grep -E '^Description=' "$unit" || true)
[[ -n "$desc" ]] || { echo "unit template has no Description: $unit" >&2; exit 1; }
if printf '%s\n' "$desc" | grep -Eq '[0-9a-f]{7,}'; then
  echo "stage unit Description has a hard-coded SHA; put the SHA in the override" >&2
  exit 1
fi
if printf '%s\n' "$desc" | grep -q '@SHA@'; then
  echo "stage unit Description must stay plain; the override carries the SHA" >&2
  exit 1
fi

if [[ "$check_only" -eq 1 ]]; then
  echo "stage unit Description takes its SHA from the override"
  exit 0
fi

[[ -n "$override" && -n "$out" ]] || usage
[[ -f "$override" ]] || { echo "override file missing: $override" >&2; exit 1; }

sha_line=$(grep -E '^SHA=' "$override" | tail -n 1 || true)
sha=${sha_line#SHA=}
sha=${sha//[[:space:]]/}
if [[ ! "$sha" =~ ^[0-9a-f]{7,40}$ ]]; then
  echo "override SHA must be 7 to 40 lowercase hex characters" >&2
  exit 1
fi

case "$out" in
  /etc|/etc/*)
    echo "refusing: render writes a local directory, not /etc" >&2
    exit 2
    ;;
esac
mkdir -p "$out"
out=$(cd "$out" && pwd)
case "$out" in
  /etc|/etc/*)
    echo "refusing: render writes a local directory, not /etc" >&2
    exit 2
    ;;
  "$templates"|"$templates"/*)
    echo "refusing: render does not write into the unit templates" >&2
    exit 1
    ;;
esac

cp -L -- "$unit" "$out/galion-voice-stage.service"
mkdir -p "$out/galion-voice-stage.service.d"
cat >"$out/galion-voice-stage.service.d/override.conf" <<EOF
# Rendered by scripts/deploy/render-stage-unit.sh
# SHA comes from the override file. The unit template has no commit hash.
[Unit]
Description=Galion Voice stage (${sha})

[Service]
Environment=GALION_RELEASE_SHA=${sha}
EOF

printf 'rendered stage unit for %s\n' "$sha"
