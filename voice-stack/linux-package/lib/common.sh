# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# shellcheck shell=bash
# Shared helpers for the Samantha user install. No network. No sudo.

die() {
  echo "samantha: $*" >&2
  exit "${2:-1}"
}

file_sha256() {
  sha256sum "$1" | awk '{print $1}'
}

file_ok() {
  local path=$1 want=$2
  [[ -f "$path" ]] || return 1
  [[ "$(file_sha256 "$path")" == "$want" ]]
}

same_file() {
  local a=$1 b=$2
  [[ -f "$a" && -f "$b" ]] && cmp -s "$a" "$b"
}

install_file() {
  local src=$1 dest=$2 mode=$3
  if [[ "${CHECK_ONLY:-0}" == 1 ]]; then
    if [[ ! -f "$dest" ]]; then
      echo "MISSING $dest" >&2
      return 1
    fi
    if [[ -n "$src" && -f "$src" ]] && ! cmp -s "$src" "$dest"; then
      echo "MISMATCH $dest" >&2
      return 1
    fi
    echo "ok   $dest"
    return 0
  fi
  if [[ "${DRY_RUN:-0}" == 1 ]]; then
    echo "dry-run: would install $src -> $dest (mode $mode)"
    return 0
  fi
  mkdir -p "$(dirname "$dest")"
  if same_file "$src" "$dest"; then
    chmod "$mode" "$dest"
    echo "unchanged $dest"
    return 0
  fi
  local tmp
  tmp="${dest}.tmp.$$"
  cp -f "$src" "$tmp"
  chmod "$mode" "$tmp"
  mv -f "$tmp" "$dest"
  echo "installed $dest"
}

render_template() {
  local src=$1 dest=$2 bin=$3 icon=$4
  local tmp line
  if [[ "${DRY_RUN:-0}" == 1 ]]; then
    echo "dry-run: would render $src -> $dest"
    return 0
  fi
  tmp=$(mktemp)
  while IFS= read -r line || [[ -n "$line" ]]; do
    line=${line//@BIN@/$bin}
    line=${line//@ICON@/$icon}
    printf '%s\n' "$line" >>"$tmp"
  done <"$src"
  chmod 0644 "$tmp"
  if [[ "${CHECK_ONLY:-0}" == 1 ]]; then
    if [[ ! -f "$dest" ]]; then
      rm -f "$tmp"
      echo "MISSING $dest" >&2
      return 1
    fi
    if ! cmp -s "$tmp" "$dest"; then
      rm -f "$tmp"
      echo "MISMATCH $dest" >&2
      return 1
    fi
    rm -f "$tmp"
    echo "ok   $dest"
    return 0
  fi
  mkdir -p "$(dirname "$dest")"
  if [[ -f "$dest" ]] && cmp -s "$tmp" "$dest"; then
    rm -f "$tmp"
    echo "unchanged $dest"
    return 0
  fi
  mv -f "$tmp" "$dest"
  echo "installed $dest"
}

refuse_system_prefix() {
  local prefix=$1
  case "$prefix" in
    /|/etc|/etc/*|/usr|/usr/*|/opt|/opt/*|/bin|/bin/*|/sbin|/sbin/*|/lib|/lib/*|/lib64|/lib64/*|/var|/var/*)
      die "refusing prefix $prefix (user install stays under a home directory)" 2
      ;;
  esac
}
