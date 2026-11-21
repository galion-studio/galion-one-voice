#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# User install for the local Samantha companion.
# No sudo. Idempotent. --dry-run writes nothing and fetches nothing.
#
#   ./install.sh                 install into $HOME
#   ./install.sh --dry-run       print the plan
#   ./install.sh --check         re-hash installed files, no network
#   ./install.sh --skip-models   unit, desktop, and binary only
#   ./install.sh --prefix DIR    install under DIR as if it were $HOME
#   ./install.sh --require-systemd
#                                fail if the user systemd bus is down

set -euo pipefail

ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=lib/common.sh
source "$ROOT/lib/common.sh"
# shellcheck source=lib/models.sh
source "$ROOT/lib/models.sh"

DRY_RUN=0
CHECK_ONLY=0
SKIP_MODELS=0
REQUIRE_SYSTEMD=0
PREFIX=""

usage() {
  cat <<'EOF'
usage: install.sh [--dry-run | --check] [--skip-models] [--prefix DIR] [--require-systemd]

Installs galion-samantha for the current user:
  ~/.local/bin/galion-samantha
  ~/.local/share/galion/samantha/          public Vosk and Piper packs
  ~/.local/share/applications/galion-samantha.desktop
  ~/.config/systemd/user/galion-samantha.service

Public model pins match scripts/asr/fetch-vosk.sh and scripts/a21/fetch-a21.sh.
GALION_SAMANTHA_REF is not downloaded. If it is set to an existing file, a
user drop-in points at that path. The file is not copied.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --check) CHECK_ONLY=1 ;;
    --skip-models) SKIP_MODELS=1 ;;
    --require-systemd) REQUIRE_SYSTEMD=1 ;;
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

if [[ "$DRY_RUN" == 1 && "$CHECK_ONLY" == 1 ]]; then
  die "use either --dry-run or --check" 2
fi

if [[ "$(id -u)" -eq 0 && -z "$PREFIX" ]]; then
  die "user install does not use sudo. Re-run as your user." 2
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

bin_dir="$layout_home/.local/bin"
data_dir="$layout_home/.local/share/galion/samantha"
app_dir="$layout_home/.local/share/applications"
icon_dir="$layout_home/.local/share/icons/hicolor/scalable/apps"
unit_dir="$layout_home/.config/systemd/user"
dropin_dir="$unit_dir/galion-samantha.service.d"

dest_bin="$bin_dir/galion-samantha"
dest_unit="$unit_dir/galion-samantha.service"
dest_desktop="$app_dir/galion-samantha.desktop"
dest_icon="$icon_dir/galion-samantha.svg"
src_unit="$ROOT/share/systemd/user/galion-samantha.service"
src_desktop="$ROOT/share/applications/galion-samantha.desktop.in"
src_icon="$ROOT/share/icons/hicolor/scalable/apps/galion-samantha.svg"
pins="$ROOT/share/models.pins"

if [[ -n "${GALION_SAMANTHA_BIN:-}" ]]; then
  src_bin=$GALION_SAMANTHA_BIN
elif [[ -x "$ROOT/target/release/galion-samantha" ]]; then
  src_bin="$ROOT/target/release/galion-samantha"
elif [[ -x "$ROOT/target/debug/galion-samantha" ]]; then
  src_bin="$ROOT/target/debug/galion-samantha"
else
  src_bin=""
fi

ensure_dirs() {
  local dir
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "dry-run: would create $bin_dir"
    echo "dry-run: would create $data_dir/state"
    echo "dry-run: would create $app_dir"
    echo "dry-run: would create $icon_dir"
    echo "dry-run: would create $unit_dir"
    return 0
  fi
  if [[ "$CHECK_ONLY" == 1 ]]; then
    local missing=0
    for dir in "$bin_dir" "$data_dir" "$app_dir" "$icon_dir" "$unit_dir"; do
      if [[ ! -d "$dir" ]]; then
        echo "MISSING $dir" >&2
        missing=1
      fi
    done
    return "$missing"
  fi
  mkdir -p "$bin_dir" "$data_dir/state" "$data_dir/models" "$app_dir" "$icon_dir" "$unit_dir"
  chmod 0700 "$data_dir" "$data_dir/state"
}

write_reference_dropin() {
  local ref=${GALION_SAMANTHA_REF:-}
  local dest="$dropin_dir/reference.conf"
  if [[ -z "$ref" ]]; then
    echo "reference: unset"
    return 0
  fi
  case "$ref" in
    *[$'\n$`"\;&|<>*?!'\\]*)
      die "GALION_SAMANTHA_REF contains a character this installer will not write into a unit" 2
      ;;
  esac
  if [[ ! -f "$ref" ]]; then
    echo "reference: missing" >&2
    return 1
  fi
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "dry-run: would write a systemd drop-in for GALION_SAMANTHA_REF (file not copied)"
    return 0
  fi
  local tmp
  tmp=$(mktemp)
  {
    printf '%s\n' '# SPDX-License-Identifier: MIT'
    printf '%s\n' '# Copyright (c) 2026 Galion Labs'
    printf '%s\n' '# Local reference path. The audio file is not copied.'
    printf '%s\n' '[Service]'
    if [[ "$ref" == *' '* ]]; then
      printf 'Environment="GALION_SAMANTHA_REF=%s"\n' "$ref"
      printf 'ReadOnlyPaths="%s"\n' "$ref"
    else
      printf 'Environment=GALION_SAMANTHA_REF=%s\n' "$ref"
      printf 'ReadOnlyPaths=%s\n' "$ref"
    fi
  } >"$tmp"
  if [[ "$CHECK_ONLY" == 1 ]]; then
    if [[ ! -f "$dest" ]] || ! cmp -s "$tmp" "$dest"; then
      rm -f "$tmp"
      echo "MISMATCH $dest" >&2
      return 1
    fi
    rm -f "$tmp"
    echo "ok   $dest"
    return 0
  fi
  mkdir -p "$dropin_dir"
  if [[ -f "$dest" ]] && cmp -s "$tmp" "$dest"; then
    rm -f "$tmp"
    echo "unchanged $dest"
    return 0
  fi
  mv -f "$tmp" "$dest"
  chmod 0644 "$dest"
  echo "installed $dest"
}

enable_user_unit() {
  if [[ -n "$PREFIX" ]]; then
    echo "prefix install: systemctl --user skipped"
    return 0
  fi
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "dry-run: would systemctl --user daemon-reload"
    echo "dry-run: would systemctl --user enable galion-samantha.service"
    return 0
  fi
  if [[ "$CHECK_ONLY" == 1 ]]; then
    return 0
  fi
  if ! command -v systemctl >/dev/null 2>&1; then
    echo "samantha: systemctl is not installed. The unit file is in place." >&2
    [[ "$REQUIRE_SYSTEMD" == 1 ]] && return 1
    return 0
  fi
  if ! systemctl --user daemon-reload; then
    echo "samantha: systemd --user bus is not available. The unit file is in place." >&2
    echo "samantha: when a user session exists: systemctl --user enable --now galion-samantha.service" >&2
    [[ "$REQUIRE_SYSTEMD" == 1 ]] && return 1
    return 0
  fi
  systemctl --user enable galion-samantha.service
  echo "enabled galion-samantha.service"
}

rc=0
ensure_dirs || rc=1

if [[ -z "$src_bin" && "$DRY_RUN" == 1 ]]; then
  echo "dry-run: would install $dest_bin from make release"
elif [[ -z "$src_bin" && "$CHECK_ONLY" == 1 ]]; then
  if [[ ! -x "$dest_bin" ]]; then
    echo "MISSING $dest_bin" >&2
    rc=1
  else
    echo "ok   $dest_bin"
  fi
elif [[ -z "$src_bin" ]]; then
  if [[ -x "$dest_bin" ]]; then
    echo "unchanged $dest_bin"
  else
    die "no galion-samantha binary. Run make release in $ROOT first." 1
  fi
else
  install_file "$src_bin" "$dest_bin" 0755 || rc=1
fi

install_file "$src_unit" "$dest_unit" 0644 || rc=1
install_file "$src_icon" "$dest_icon" 0644 || rc=1
render_template "$src_desktop" "$dest_desktop" "$dest_bin" "$dest_icon" || rc=1
write_reference_dropin || rc=1

if [[ "$SKIP_MODELS" == 1 ]]; then
  echo "models: skipped"
elif [[ "$rc" -eq 0 || "$CHECK_ONLY" == 1 || "$DRY_RUN" == 1 ]]; then
  install_public_models "$data_dir" "$pins" || rc=1
  if [[ "$rc" -eq 0 ]]; then
    smoke_piper "$data_dir" || rc=1
  fi
fi

if [[ "$rc" -eq 0 ]]; then
  enable_user_unit || rc=1
fi

if [[ "$rc" -eq 0 ]]; then
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "SAMANTHA_INSTALL=DRY_RUN"
  elif [[ "$CHECK_ONLY" == 1 ]]; then
    echo "SAMANTHA_INSTALL=CHECKED"
  else
    echo "SAMANTHA_INSTALL=OK"
  fi
else
  echo "SAMANTHA_INSTALL=FAILED" >&2
fi
exit "$rc"
