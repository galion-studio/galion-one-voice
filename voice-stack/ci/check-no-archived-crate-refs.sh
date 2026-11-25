#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Fail if a tracked file outside archive/ and docs/ names an archived crate.
#
# Legal mentions:
#   * the dated archive path archive/YYYY-MM-DD/... that holds the crate
#   * a retirement line that already says the crate is archived, retired,
#     or not built (the S55 live-demo scripts and galion-gates notes)
#
# Rust idents (hyphens turned into underscores) are never legal outside
# archive/ and docs/. This script does not name a live path dependency.
# Needles come from archive/**/ARCHIVED.md so a newly archived crate is
# picked up without editing this file. This file is scanned too: it builds
# sample names from pieces so the source does not contain those names.
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

needles=()

add_needle() {
  local n="$1" existing
  [[ -z "$n" ]] && return 0
  if [[ ${#needles[@]} -gt 0 ]]; then
    for existing in "${needles[@]}"; do
      [[ "$existing" == "$n" ]] && return 0
    done
  fi
  needles+=("$n")
}

load_needles() {
  local note former base seam_parent
  needles=()
  while IFS= read -r note; do
    [[ -n "$note" ]] || continue
    while IFS= read -r former; do
      [[ -n "$former" ]] || continue
      base="${former##*/}"
      if [[ "$base" == *-* ]]; then
        add_needle "$base"
        add_needle "${base//-/_}"
      fi
      # voice-stack/<bridge>/seam/<pkg> → needle "<bridge>/seam"
      if [[ "$former" == */seam/* || "$former" == */seam ]]; then
        seam_parent="${former%%/seam*}"
        add_needle "${seam_parent##*/}/seam"
      fi
    done < <(sed -n 's/^Former path: `\([^`]*\)`.*/\1/p' "$note")
  done < <(git ls-files 'archive/**/ARCHIVED.md')
  if [[ ${#needles[@]} -eq 0 ]]; then
    printf 'FAIL: no Former path entries under archive/**/ARCHIVED.md\n' >&2
    exit 1
  fi
}

# 0 when every occurrence of needle sits in one archive/YYYY-MM-DD/ token.
occurrences_are_archive_paths() {
  local line="$1" needle="$2" rest="$line"
  local pre token after tail span saw=0
  [[ -n "$needle" ]] || return 1
  while [[ "$rest" == *"$needle"* ]]; do
    saw=1
    pre="${rest%%"$needle"*}"
    token="${pre##*[[:space:]]}"
    after="${rest#*"$needle"}"
    tail=""
    if [[ "$after" =~ ^([^[:space:]]*) ]]; then
      tail="${BASH_REMATCH[1]}"
    fi
    span="${token}${needle}${tail}"
    if [[ ! "$span" =~ archive/[0-9]{4}-[0-9]{2}-[0-9]{2}/ ]]; then
      return 1
    fi
    rest="$after"
  done
  [[ "$saw" -eq 1 ]]
}

# Prose that already records the retirement. Rust idents do not use this.
line_is_retirement() {
  local line="$1"
  if [[ "$line" =~ is\ archived|are\ archived|archived\;|archived\.|archived\)|archived\ at\ |retired\ with\ |does\ not\ build\ |is\ not\ built ]]; then
    return 0
  fi
  if [[ "$line" == *"belonged to "* && "$line" == *"horbm-duplex"* ]]; then
    return 0
  fi
  return 1
}

needle_is_rust_ident() {
  local needle="$1"
  [[ "$needle" != *[-/]* ]]
}

# 0 when the line is a violation.
line_is_stale() {
  local line="$1" needle
  for needle in "${needles[@]}"; do
    [[ "$line" == *"$needle"* ]] || continue
    if occurrences_are_archive_paths "$line" "$needle"; then
      continue
    fi
    if needle_is_rust_ident "$needle"; then
      return 0
    fi
    if line_is_retirement "$line"; then
      continue
    fi
    return 0
  done
  return 1
}

scan_tree() {
  local f hits line file lineno text rest fail=0
  local -a args=()
  for needle in "${needles[@]}"; do
    args+=(-e "$needle")
  done
  while IFS= read -r -d '' f; do
    case "$f" in
      archive/*|docs/*) continue ;;
    esac
    [[ -f "$f" ]] || continue
    hits="$(grep -nI -H -F "${args[@]}" -- "$f" || true)"
    [[ -n "$hits" ]] || continue
    while IFS= read -r line; do
      [[ -n "$line" ]] || continue
      file="${line%%:*}"
      rest="${line#*:}"
      lineno="${rest%%:*}"
      text="${rest#*:}"
      if line_is_stale "$text"; then
        printf 'FAIL: %s:%s: %s\n' "$file" "$lineno" "$text" >&2
        fail=1
      fi
    done <<< "$hits"
  done < <(git ls-files -z)
  if [[ "$fail" -ne 0 ]]; then
    printf 'FAIL: a tracked file outside archive/ and docs/ names an archived crate\n' >&2
    exit 1
  fi
  printf 'ok: archived crate names outside archive/ and docs/ are archive paths or retirement lines\n'
}

have_needle() {
  local want="$1" needle
  for needle in "${needles[@]}"; do
    [[ "$needle" == "$want" ]] && return 0
  done
  return 1
}

self_test() {
  # Pieces, so this file does not itself contain an archived crate name.
  local v2="galion-v2-""live"
  local demo="galion-live-""demo"
  local v2_ident="${v2//-/_}"
  local demo_ident="${demo//-/_}"
  local seam="horbm-bridge/""seam"
  local want
  for want in "$v2" "$v2_ident" "$demo" "$demo_ident" "$seam"; do
    if ! have_needle "$want"; then
      printf 'self-test: missing needle %s\n' "$want" >&2
      exit 1
    fi
  done

  expect_stale() {
    if line_is_stale "$1"; then
      return 0
    fi
    printf 'self-test: expected stale: %s\n' "$1" >&2
    exit 1
  }
  expect_ok() {
    if line_is_stale "$1"; then
      printf 'self-test: expected allowed: %s\n' "$1" >&2
      exit 1
    fi
  }

  expect_stale "\`${v2}\` \`PIPER_HZ\`"
  expect_stale "crates/${v2}/src/vad.rs"
  expect_stale "${v2_ident}::audio::rms_db"
  expect_stale "${v2_ident}::audio is archived"
  expect_stale "voice-stack/${seam}/horbm"
  expect_stale "name = \"${demo}\""
  expect_stale "Showcase WAVs belonged to ${demo}."
  expect_ok "archive/2026-11-24/crates/${v2}/src/vad.rs"
  expect_ok "Same shape as archive/2026-11-24/crates/${v2}."
  expect_ok "\`archive/2026-11-24/voice-stack/${seam}/horbm\` is the old type seam."
  expect_ok "The ${demo} crate is archived under"
  expect_ok "It does not build ${demo} and it does not check showcase WAVs."
  expect_ok "Showcase WAVs belonged to ${demo}. The replacement is horbm-duplex."
  expect_ok "echo \"live-demo: ${demo} is archived at archive/2026-11-25/crates/${demo}\""
  expect_ok "chk C1 PASS \"horbm-duplex tests passed (${demo} archived; no showcase WAVs)\""

  scan_tree
  printf 'self-test ok\n'
}

load_needles

if [[ "${1:-}" == "--self-test" ]]; then
  self_test
  exit 0
fi

scan_tree
