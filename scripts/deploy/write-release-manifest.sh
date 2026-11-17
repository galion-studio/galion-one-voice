#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
# G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
# Developed and distributed by Galion Labs
# Perpetual Open MIT License
# Copyright © 2026 Galion Labs. All rights reserved.
#
# Writes <root>/release/<sha>/MANIFEST.json and rebuilds <root>/release/INDEX.json.
# Local directory only. Does not open a socket, does not install a unit,
# and does not move the current symlink.
#
#   scripts/deploy/write-release-manifest.sh \
#     --root /opt/galion/voice \
#     --sha <git-sha> \
#     --binary <path-to-binary> \
#     --feature dot-viz-web \
#     --helper <path-to-copied-helper>
#
# --built-at is optional. The default is the clock in Europe/Zagreb.
# --execute is refused.

set -euo pipefail
export LC_ALL=C

root=""
sha=""
binary=""
built_at=""
features=()
helpers=()

usage() {
  echo "usage: write-release-manifest.sh --root DIR --sha HEX --binary FILE [--feature NAME]... [--helper FILE]... [--built-at STAMP]" >&2
  exit 1
}

json_escape() {
  local s=$1
  s=${s//\\/\\\\}
  s=${s//\"/\\\"}
  printf '%s' "$s"
}

require_name() {
  local kind=$1 name=$2
  if [[ ! "$name" =~ ^[A-Za-z0-9][A-Za-z0-9._+-]*$ ]]; then
    echo "refusing $kind name: $name" >&2
    exit 1
  fi
  case "$name" in
    MANIFEST.json|INDEX.json|current|helpers)
      echo "refusing reserved $kind name: $name" >&2
      exit 1
      ;;
  esac
}

sha256_file() {
  sha256sum -- "$1" | awk '{print $1}'
}

abs_dir() {
  (cd "$1" && pwd)
}

abs_file() {
  local dir base
  dir=$(cd "$(dirname "$1")" && pwd)
  base=$(basename "$1")
  printf '%s/%s' "$dir" "$base"
}

under_root() {
  local root_abs=$1 path=$2
  case "$path" in
    "$root_abs"|"$root_abs"/*) ;;
    *)
      echo "refusing path outside root: $path" >&2
      exit 1
      ;;
  esac
}

valid_stamp() {
  [[ "$1" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\+0[12]:00$ ]]
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --execute)
      echo "refusing: local files only (no server)" >&2
      exit 2
      ;;
    --root)
      [[ $# -ge 2 ]] || usage
      root=$2
      shift 2
      ;;
    --sha)
      [[ $# -ge 2 ]] || usage
      sha=$2
      shift 2
      ;;
    --binary)
      [[ $# -ge 2 ]] || usage
      binary=$2
      shift 2
      ;;
    --feature)
      [[ $# -ge 2 ]] || usage
      features+=("$2")
      shift 2
      ;;
    --helper)
      [[ $# -ge 2 ]] || usage
      helpers+=("$2")
      shift 2
      ;;
    --built-at)
      [[ $# -ge 2 ]] || usage
      built_at=$2
      shift 2
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

[[ -n "$root" && -n "$sha" && -n "$binary" ]] || usage
[[ -d "$root" ]] || { echo "root is not a directory: $root" >&2; exit 1; }
[[ "$sha" =~ ^[0-9a-f]{7,40}$ ]] || { echo "sha must be 7 to 40 lowercase hex characters" >&2; exit 1; }
[[ -f "$binary" ]] || { echo "binary is not a file: $binary" >&2; exit 1; }

root=$(abs_dir "$root")
binary=$(abs_file "$binary")

if [[ -z "$built_at" ]]; then
  built_at=$(TZ=Europe/Zagreb date --iso-8601=seconds)
fi
valid_stamp "$built_at" || {
  echo "built_at must be Europe/Zagreb local time, offset +01:00 or +02:00: $built_at" >&2
  exit 1
}

bin_name=$(basename "$binary")
require_name binary "$bin_name"

if [[ ${#features[@]} -gt 0 ]]; then
  mapfile -t features < <(printf '%s\n' "${features[@]}" | sort -u)
fi
for feat in "${features[@]+"${features[@]}"}"; do
  require_name feature "$feat"
done

declare -A helper_seen=()
helper_names=()
helper_srcs=()
if [[ ${#helpers[@]} -gt 0 ]]; then
  for src in "${helpers[@]}"; do
    [[ -f "$src" ]] || { echo "helper is not a file: $src" >&2; exit 1; }
    src=$(abs_file "$src")
    name=$(basename "$src")
    require_name helper "$name"
    if [[ -n "${helper_seen[$name]:-}" ]]; then
      echo "refusing duplicate helper name: $name" >&2
      exit 1
    fi
    helper_seen[$name]=1
    helper_names+=("$name")
    helper_srcs+=("$src")
  done
fi

rel_dir="$root/release/$sha"
if [[ -L "$rel_dir" ]]; then
  echo "refusing to write through a symlink: $rel_dir" >&2
  exit 1
fi
mkdir -p "$rel_dir"
under_root "$root" "$rel_dir"

dest_bin="$rel_dir/$bin_name"
under_root "$root" "$dest_bin"
if [[ "$binary" != "$dest_bin" ]]; then
  if [[ -e "$dest_bin" ]]; then
    have=$(sha256_file "$dest_bin")
    want=$(sha256_file "$binary")
    if [[ "$have" != "$want" ]]; then
      echo "refusing to replace $dest_bin with different bytes" >&2
      exit 1
    fi
  else
    cp -L -- "$binary" "$dest_bin"
  fi
fi
bin_hash=$(sha256_file "$dest_bin")

helper_paths=()
helper_hashes=()
if [[ ${#helper_names[@]} -gt 0 ]]; then
  mkdir -p "$rel_dir/helpers"
  under_root "$root" "$rel_dir/helpers"
  order=$(printf '%s\n' "${!helper_names[@]}" | while read -r i; do
    printf '%s\t%s\n' "${helper_names[$i]}" "$i"
  done | sort -t $'\t' -k1,1 | cut -f2)
  sorted_names=()
  sorted_srcs=()
  while read -r i; do
    sorted_names+=("${helper_names[$i]}")
    sorted_srcs+=("${helper_srcs[$i]}")
  done <<<"$order"
  helper_names=("${sorted_names[@]}")
  helper_srcs=("${sorted_srcs[@]}")
  for i in "${!helper_names[@]}"; do
    name=${helper_names[$i]}
    src=${helper_srcs[$i]}
    dest="$rel_dir/helpers/$name"
    under_root "$root" "$dest"
    if [[ "$src" != "$dest" ]]; then
      if [[ -e "$dest" ]]; then
        have=$(sha256_file "$dest")
        want=$(sha256_file "$src")
        if [[ "$have" != "$want" ]]; then
          echo "refusing to replace $dest with different bytes" >&2
          exit 1
        fi
      else
        cp -L -- "$src" "$dest"
      fi
    fi
    helper_paths+=("helpers/$name")
    helper_hashes+=("$(sha256_file "$dest")")
  done
fi

emit_features() {
  local trailing=$1
  shift
  local -a arr=()
  if [[ $# -gt 0 ]]; then
    arr=("$@")
  fi
  local n=${#arr[@]} i comma
  if [[ "$n" -eq 0 ]]; then
    if [[ "$trailing" == "comma" ]]; then
      printf '  "features": [],\n'
    else
      printf '  "features": []\n'
    fi
    return
  fi
  printf '  "features": [\n'
  for i in "${!arr[@]}"; do
    comma=,
    [[ "$i" -eq $((n - 1)) ]] && comma=
    printf '    "%s"%s\n' "$(json_escape "${arr[$i]}")" "$comma"
  done
  if [[ "$trailing" == "comma" ]]; then
    printf '  ],\n'
  else
    printf '  ]\n'
  fi
}

emit_helpers() {
  local trailing=$1
  local n=${#helper_names[@]} i comma
  if [[ "$n" -eq 0 ]]; then
    if [[ "$trailing" == "comma" ]]; then
      printf '  "helpers": [],\n'
    else
      printf '  "helpers": []\n'
    fi
    return
  fi
  printf '  "helpers": [\n'
  for i in "${!helper_names[@]}"; do
    comma=,
    [[ "$i" -eq $((n - 1)) ]] && comma=
    printf '    {"name": "%s", "path": "%s", "sha256": "%s"}%s\n' \
      "$(json_escape "${helper_names[$i]}")" \
      "$(json_escape "${helper_paths[$i]}")" \
      "${helper_hashes[$i]}" \
      "$comma"
  done
  if [[ "$trailing" == "comma" ]]; then
    printf '  ],\n'
  else
    printf '  ]\n'
  fi
}

manifest_tmp=$(mktemp)
trap 'rm -f "$manifest_tmp"' RETURN
{
  printf '{\n'
  printf '  "sha": "%s",\n' "$sha"
  printf '  "built_at": "%s",\n' "$built_at"
  printf '  "timezone": "Europe/Zagreb",\n'
  printf '  "binary": "%s",\n' "$(json_escape "$bin_name")"
  printf '  "binary_sha256": "%s",\n' "$bin_hash"
  emit_features comma "${features[@]+"${features[@]}"}"
  emit_helpers none
  printf '}\n'
} >"$manifest_tmp"
mv "$manifest_tmp" "$rel_dir/MANIFEST.json"
trap - RETURN

# Rebuild the index from every release manifest on disk.
index_work=$(mktemp -d)
trap 'rm -rf "$index_work"' RETURN

read_string_field() {
  local file=$1 key=$2
  sed -n "s/^  \"${key}\": \"\\(.*\\)\",$/\\1/p" "$file" | head -n 1
}

shopt -s nullglob
for dir in "$root/release"/*; do
  [[ -L "$dir" ]] && continue
  [[ -d "$dir" ]] || continue
  base=$(basename "$dir")
  [[ "$base" =~ ^[0-9a-f]{7,40}$ ]] || continue
  file="$dir/MANIFEST.json"
  [[ -f "$file" ]] || continue
  parsed_sha=$(read_string_field "$file" sha)
  parsed_at=$(read_string_field "$file" built_at)
  parsed_bin=$(read_string_field "$file" binary)
  parsed_hash=$(read_string_field "$file" binary_sha256)
  [[ "$parsed_sha" == "$base" ]] || { echo "manifest sha does not match directory: $file" >&2; exit 1; }
  valid_stamp "$parsed_at" || { echo "manifest built_at is not Europe/Zagreb: $file" >&2; exit 1; }
  [[ "$parsed_hash" =~ ^[0-9a-f]{64}$ ]] || { echo "manifest binary_sha256 is not 64 hex: $file" >&2; exit 1; }
  slot="$index_work/${parsed_at}__${parsed_sha}"
  mkdir -p "$slot"
  printf '%s\n' "$parsed_sha" >"$slot/sha"
  printf '%s\n' "$parsed_at" >"$slot/built_at"
  printf '%s\n' "$parsed_bin" >"$slot/binary"
  printf '%s\n' "$parsed_hash" >"$slot/binary_sha256"
  awk '
    /^  "features": \[\],$/ { exit }
    /^  "features": \[/ { on=1; next }
    on && /^  \],$/ { exit }
    on && /^    "/ {
      s=$0
      sub(/^    "/, "", s)
      sub(/",?$/, "", s)
      print s
    }
  ' "$file" >"$slot/features"
  awk '
    /^  "helpers": \[\],?$/ { exit }
    /^  "helpers": \[/ { on=1; next }
    on && /^  \],?$/ { exit }
    on && /^    \{"name": / {
      line=$0
      name=line; sub(/^    \{"name": "/, "", name); sub(/".*/, "", name)
      path=line; sub(/.*"path": "/, "", path); sub(/".*/, "", path)
      hash=line; sub(/.*"sha256": "/, "", hash); sub(/".*/, "", hash)
      printf "%s\t%s\t%s\n", name, path, hash
    }
  ' "$file" >"$slot/helpers"
done
shopt -u nullglob

current="null"
current_link="$root/release/current"
if [[ -L "$current_link" ]]; then
  target=$(readlink "$current_link")
  current_sha=$(basename "$target")
  if [[ "$current_sha" =~ ^[0-9a-f]{7,40}$ ]]; then
    current=$(printf '"%s"' "$current_sha")
  fi
fi

index_tmp=$(mktemp)
trap 'rm -rf "$index_work"; rm -f "$index_tmp"' RETURN
{
  printf '{\n'
  printf '  "timezone": "Europe/Zagreb",\n'
  printf '  "current": %s,\n' "$current"
  mapfile -t slots < <(find "$index_work" -mindepth 1 -maxdepth 1 -type d | sort)
  if [[ ${#slots[@]} -eq 0 ]]; then
    printf '  "releases": []\n'
  else
    printf '  "releases": [\n'
    for s in "${!slots[@]}"; do
      slot=${slots[$s]}
      rel_sha=$(cat "$slot/sha")
      rel_at=$(cat "$slot/built_at")
      rel_bin=$(cat "$slot/binary")
      rel_hash=$(cat "$slot/binary_sha256")
      mapfile -t rel_features <"$slot/features"
      # Drop the empty line mapfile keeps when the file is empty.
      if [[ ${#rel_features[@]} -eq 1 && -z "${rel_features[0]}" ]]; then
        rel_features=()
      fi
      helper_names=()
      helper_paths=()
      helper_hashes=()
      while IFS=$'\t' read -r n p h; do
        [[ -n "$n" ]] || continue
        helper_names+=("$n")
        helper_paths+=("$p")
        helper_hashes+=("$h")
      done <"$slot/helpers"
      comma=,
      [[ "$s" -eq $((${#slots[@]} - 1)) ]] && comma=
      printf '    {\n'
      printf '      "sha": "%s",\n' "$rel_sha"
      printf '      "built_at": "%s",\n' "$rel_at"
      printf '      "binary": "%s",\n' "$(json_escape "$rel_bin")"
      printf '      "binary_sha256": "%s",\n' "$rel_hash"
      # emit_features uses 2-space indent. Re-indent by prefixing two spaces
      # is wrong for nested objects. Write the arrays at index-entry indent.
      if [[ ${#rel_features[@]} -eq 0 ]]; then
        printf '      "features": [],\n'
      else
        printf '      "features": [\n'
        for i in "${!rel_features[@]}"; do
          fcomma=,
          [[ "$i" -eq $((${#rel_features[@]} - 1)) ]] && fcomma=
          printf '        "%s"%s\n' "$(json_escape "${rel_features[$i]}")" "$fcomma"
        done
        printf '      ],\n'
      fi
      if [[ ${#helper_names[@]} -eq 0 ]]; then
        printf '      "helpers": [],\n'
      else
        printf '      "helpers": [\n'
        for i in "${!helper_names[@]}"; do
          hcomma=,
          [[ "$i" -eq $((${#helper_names[@]} - 1)) ]] && hcomma=
          printf '        {"name": "%s", "path": "%s", "sha256": "%s"}%s\n' \
            "$(json_escape "${helper_names[$i]}")" \
            "$(json_escape "${helper_paths[$i]}")" \
            "${helper_hashes[$i]}" \
            "$hcomma"
        done
        printf '      ],\n'
      fi
      printf '      "manifest": "release/%s/MANIFEST.json"\n' "$rel_sha"
      printf '    }%s\n' "$comma"
    done
    printf '  ]\n'
  fi
  printf '}\n'
} >"$index_tmp"
mv "$index_tmp" "$root/release/INDEX.json"
rm -rf "$index_work"
trap - RETURN

printf 'wrote release/%s/MANIFEST.json\n' "$sha"
