#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
#
# HORBM research-data boundary. Rules and the false-positive policy:
# docs/horbm/RESEARCH-DATA.md
#
# The dialogue markers are listed only in this file. The searches skip it.
# Exit 0 when the tree is clean, 1 when a boundary is crossed.
set -euo pipefail
set -f
cd "$(dirname "$0")/.."

# 64 KiB. Tracked jsonl/csv/tsv on main at 02f8ac90 are all under 16 KiB.
LARGE=65536
SELF=scripts/check-research-boundary.sh
fail=0

note() {
  printf 'FAIL: %s\n' "$1"
  fail=1
}

is_data_ext() {
  case "$1" in
    jsonl|parquet|csv|tsv|txt|json|zip|gz|tgz|tar|bz2|xz|arrow) return 0 ;;
  esac
  return 1
}

is_bulk_path() {
  case "$1" in
    *.jsonl|*.parquet|*.csv|*.tsv) return 0 ;;
    *.jsonl.gz|*.parquet.gz|*.csv.gz|*.tsv.gz) return 0 ;;
    *.jsonl.bz2|*.csv.bz2|*.tsv.bz2|*.parquet.bz2) return 0 ;;
    *.jsonl.xz|*.csv.xz|*.tsv.xz|*.parquet.xz) return 0 ;;
  esac
  return 1
}

is_test_path() {
  local comp
  local IFS='/'
  for comp in $(printf '%s' "$1" | tr '[:upper:]' '[:lower:]'); do
    case "$comp" in
      test|tests|testdata) return 0 ;;
    esac
  done
  return 1
}

is_card_path() {
  local path comp base
  path=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')
  base=${path##*/}
  case "$base" in
    *research-card*|*research_card*|*sample-card*|*sample_card*) return 0 ;;
  esac
  local IFS='/'
  for comp in $path; do
    case "$comp" in
      test|tests|testdata|fixture|fixtures) return 0 ;;
    esac
  done
  return 1
}

norm_id() {
  local s prev
  s=$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]')
  while true; do
    prev=$s
    case "$s" in
      *.jsonl|*.parquet|*.csv|*.tsv|*.txt|*.json|*.zip|*.tgz|*.tar|*.gz|*.bz2|*.xz|*.arrow)
        s=${s%.*}
        ;;
      *) break ;;
    esac
    [[ $s == "$prev" ]] && break
  done
  printf '%s' "$s" | tr -d '_-'
}

is_dataset_id() {
  case "$1" in
    empatheticdialogues|kdconv|personachat|convai2|blendedskilltalk|lccc|lcccbase|lccclarge|dailydialog|dailydialogue|dailydialogues)
      return 0
      ;;
    opensubtitles|opensubtitles[0-9]*)
      return 0
      ;;
  esac
  return 1
}

# A data file whose name, or a parent directory, is one of the corpora.
path_is_dataset_file() {
  local path=$1 base ext rest comp
  base=${path##*/}
  ext=$(printf '%s' "${base##*.}" | tr '[:upper:]' '[:lower:]')
  is_data_ext "$ext" || return 1
  if is_dataset_id "$(norm_id "$base")"; then
    return 0
  fi
  rest=${path%/*}
  [[ $rest == "$path" ]] && return 1
  local IFS='/'
  for comp in $rest; do
    if is_dataset_id "$(norm_id "$comp")"; then
      return 0
    fi
  done
  return 1
}

check_names() {
  local path
  while IFS= read -r path; do
    [[ -z $path ]] && continue
    if path_is_dataset_file "$path"; then
      note "dataset path: $path"
    fi
  done < <(git ls-files | grep -E -i \
    -e 'empathetic[-_]?dialogues' \
    -e 'kdconv' \
    -e 'persona[-_]?chat' \
    -e 'conv[-_]?ai[-_]?2' \
    -e 'blended[-_]?skill[-_]?talk' \
    -e '(^|/)lccc([_./-]|base|large|$)' \
    -e 'daily[-_]?dialog' \
    -e 'open[-_]?subtitles' \
    || true)
}

check_large() {
  local path bytes
  while IFS= read -r -d '' path; do
    is_bulk_path "$path" || continue
    is_test_path "$path" && continue
    bytes=$(wc -c < "$path" | tr -d '[:space:]')
    if [[ "$bytes" -ge $LARGE ]]; then
      note "large data file (${bytes} bytes): $path"
    fi
  done < <(git ls-files -z -- \
    '*.jsonl' '*.parquet' '*.csv' '*.tsv' \
    '*.jsonl.gz' '*.parquet.gz' '*.csv.gz' '*.tsv.gz' \
    '*.jsonl.bz2' '*.csv.bz2' '*.tsv.bz2' '*.parquet.bz2' \
    '*.jsonl.xz' '*.csv.xz' '*.tsv.xz' '*.parquet.xz')
}

check_markers() {
  local label mode needle hits line
  while IFS='|' read -r label mode needle; do
    [[ -z ${label:-} ]] && continue
    if [[ $mode == i ]]; then
      hits=$(git grep -I -l -i -F -e "$needle" -- . ":!$SELF" || true)
    else
      hits=$(git grep -I -l -F -e "$needle" -- . ":!$SELF" || true)
    fi
    while IFS= read -r line; do
      [[ -z $line ]] && continue
      [[ $line == "$SELF" ]] && continue
      note "copied dialogue ($label): $line"
    done <<< "$hits"
  done <<'EOF'
EmpatheticDialogues|f|people_comma_
DailyDialog|f|Say , Jim , how about going for a few beers after dinner ?
DailyDialog|f|__eou__
PersonaChat/ConvAI2|i|i have went to mexico 4 times this year
Blended Skill Talk|i|i like to shoot a bow
KdConv|f|对《我喜欢上你时的内心活动》这首歌有了解吗
LCCC|f|那 我 的 嘴巴 可能 要 烂掉
EOF
}

check_paths() {
  local hits line rest
  hits=$(git grep -I -n -i -E \
    -e 'horbm[/\\]+research([/\\"[:space:]]|$)' \
    -e '[/\\](empathetic[-_]?dialogues|kdconv|persona[-_]?chat|conv[-_]?ai[-_]?2|blended[-_]?skill[-_]?talk|lccc([_-](base|large))?|daily[-_]?dialog(ue)?s?|open[-_]?subtitles[0-9]*)([/\\"[:space:]]|$)' \
    -e 'GALION_HORBM_RESEARCH_DIR[[:space:]]*=[[:space:]]*[^[:space:]#]' \
    -- . ":!$SELF" || true)
  while IFS= read -r line; do
    [[ -z $line ]] && continue
    [[ $line == "$SELF:"* ]] && continue
    case "$line" in
      *http://*|*https://*) continue ;;
    esac
    # docs/horbm/research/ is the in-repo note path, not the laptop root.
    rest=$(printf '%s\n' "$line" | awk '{gsub(/[Dd]ocs\/[Hh]orbm\/[Rr]esearch/, ""); print}')
    if printf '%s\n' "$rest" | grep -E -i -q \
      -e 'horbm[/\\]+research([/\\"[:space:]]|$)' \
      -e '[/\\](empathetic[-_]?dialogues|kdconv|persona[-_]?chat|conv[-_]?ai[-_]?2|blended[-_]?skill[-_]?talk|lccc([_-](base|large))?|daily[-_]?dialog(ue)?s?|open[-_]?subtitles[0-9]*)([/\\"[:space:]]|$)' \
      -e 'GALION_HORBM_RESEARCH_DIR[[:space:]]*=[[:space:]]*[^[:space:]#]'
    then
      note "hard-coded research path: $line"
    fi
  done <<< "$hits"
}

card_seen=""

flag_card() {
  case "
$card_seen
" in
    *"
$1
"*) return ;;
  esac
  card_seen="$card_seen
$1"
  note "research-card fixture: $1"
}

check_cards() {
  local path ext base
  while IFS= read -r path; do
    [[ -z $path ]] && continue
    is_card_path "$path" || continue
    flag_card "$path"
  done < <(git grep -I -l -i -E \
    -e '"conv_id"' \
    -e '"utterance_idx"' \
    -e '"selfeval"' \
    -e '"speaker_idx"' \
    -e '"persona"[[:space:]]*:[[:space:]]*"[^"[:cntrl:]]* [^"[:cntrl:]]*"' \
    -e '"attrname"' \
    -e '"research_card"' \
    -e 'your persona:' \
    -- '*.json' '*.jsonl' '*.md' '*.txt' '*.csv' '*.tsv' || true)

  while IFS= read -r path; do
    [[ -z $path ]] && continue
    is_card_path "$path" || continue
    if grep -q -e '"claim_type"' -e 'claim_type:' "$path"; then
      flag_card "$path"
    fi
  done < <(git grep -I -l -e '"said_by"' -e 'said_by:' -- '*.json' '*.jsonl' '*.md' '*.txt' || true)

  while IFS= read -r path; do
    [[ -z $path ]] && continue
    base=${path##*/}
    ext=$(printf '%s' "${base##*.}" | tr '[:upper:]' '[:lower:]')
    case "$ext" in
      json|jsonl|md|txt|csv|tsv|yml|yaml) flag_card "$path" ;;
    esac
  done < <(git ls-files | grep -E -i -e 'research[-_]cards?' -e 'sample[-_]cards?' || true)
}

check_names
check_large
check_markers
check_paths
check_cards

if [[ $fail -eq 0 ]]; then
  printf 'OK: research boundary holds\n'
  exit 0
fi
exit 1
