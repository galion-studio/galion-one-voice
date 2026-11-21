#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Remove the user install. Keeps downloaded models unless --purge-models.
# No sudo. Idempotent. --dry-run removes nothing.

set -euo pipefail

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=lib/common.sh
source "$ROOT/lib/common.sh"

DRY_RUN=0
PURGE_MODELS=0
PREFIX=""

usage() {
  cat <<'EOF'
usage: uninstall.sh [--dry-run] [--purge-models] [--prefix DIR]

Removes the binary, desktop file, icon, and user systemd unit.
Downloaded models stay unless --purge-models is set.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --purge-models) PURGE_MODELS=1 ;;
    --prefix)
      shift
      [[ $# -gt 0 ]] || die "missing value for --prefix" 2
      PREFIX=$1
      ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument: $1" 2 ;;
  esac
  shift
done

if [[ "$(id -u)" -eq 0 && -z "$PREFIX" ]]; then
  die "user uninstall does not use sudo. Re-run as your user." 2
fi

if [[ -n "$PREFIX" ]]; then
  case "$PREFIX" in
    /*) ;;
    *) PREFIX="$PWD/$PREFIX" ;;
  esac
  refuse_system_prefix "$PREFIX"
  layout_home=$PREFIX
else
  [[ -n "${HOME:-}" ]] || die "HOME is unset" 2
  layout_home=$HOME
fi

targets=(
  "$layout_home/.local/bin/galion-samantha"
  "$layout_home/.local/share/applications/galion-samantha.desktop"
  "$layout_home/.local/share/icons/hicolor/scalable/apps/galion-samantha.svg"
  "$layout_home/.config/systemd/user/galion-samantha.service"
  "$layout_home/.config/systemd/user/galion-samantha.service.d/reference.conf"
)

if [[ "$PURGE_MODELS" == 1 ]]; then
  targets+=("$layout_home/.local/share/galion/samantha")
fi

if [[ -z "$PREFIX" ]]; then
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "dry-run: would systemctl --user disable --now galion-samantha.service"
  elif command -v systemctl >/dev/null 2>&1; then
    systemctl --user disable --now galion-samantha.service >/dev/null 2>&1 || true
  fi
else
  echo "prefix uninstall: systemctl --user skipped"
fi

for path in "${targets[@]}"; do
  if [[ ! -e "$path" ]]; then
    echo "absent $path"
    continue
  fi
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "dry-run: would remove $path"
    continue
  fi
  rm -rf "$path"
  echo "removed $path"
done

if [[ "$DRY_RUN" == 1 ]]; then
  echo "SAMANTHA_UNINSTALL=DRY_RUN"
else
  echo "SAMANTHA_UNINSTALL=OK"
fi
