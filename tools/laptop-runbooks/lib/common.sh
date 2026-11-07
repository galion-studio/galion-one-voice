# Shared helpers for the laptop runbooks.
# Sourced, not executed. Audio stays outside the git repo. Nothing here merges
# or pushes, and nothing edits LOCKS.toml, persona TOML, or PR #97.

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  echo "source this file; do not execute it" >&2
  exit 2
fi

laptop_repo_root() {
  # BASH_SOURCE[1] is the runbook that called this function
  # (tools/laptop-runbooks/*.sh). The repo root is two directories up.
  local caller="${BASH_SOURCE[1]}"
  local scripts
  scripts="$(cd "$(dirname "$caller")" && pwd)"
  (cd "$scripts/../.." && pwd)
}

# Fixed results directory. Override with GALION_LAPTOP_OUT. Must sit outside
# the git checkout so a person's wav and the observation JSON are not committed.
laptop_out_dir() {
  local repo="$1"
  local out="${GALION_LAPTOP_OUT:-${XDG_CACHE_HOME:-$HOME/.cache}/galion-laptop-runbooks}"
  if [[ "$out" != /* ]]; then
    echo "GALION_LAPTOP_OUT must be an absolute path ($out)" >&2
    return 2
  fi
  case "$out" in
    "$repo"|"$repo"/*)
      echo "results directory must be outside the repo ($out)" >&2
      return 2
      ;;
  esac
  mkdir -p "$out"
  (cd "$out" && pwd)
}

laptop_require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "missing prerequisite: $cmd" >&2
    return 2
  fi
}

# Absolute path of an existing file, or failure. Rejects anything inside the repo.
laptop_abs_outside_repo() {
  local repo="$1" path="$2" what="$3"
  if [[ ! -f "$path" ]]; then
    echo "missing $what: $path" >&2
    return 2
  fi
  if [[ -L "$path" ]]; then
    echo "$what must be a regular file, not a symlink: $path" >&2
    return 2
  fi
  local abs
  abs="$(cd "$(dirname "$path")" && pwd)/$(basename "$path")"
  case "$abs" in
    "$repo"|"$repo"/*)
      echo "$what must stay outside the repo (not committed): $abs" >&2
      return 2
      ;;
  esac
  printf '%s\n' "$abs"
}

# Resolve a PR checkout without merging it into this branch.
# Prints the tree root. Creates a detached worktree under the cache when
# GALION_PR*_ROOT is unset and this checkout does not already contain $marker.
laptop_resolve_tree() {
  local repo="$1" out="$2" env_name="$3" ref="$4" marker="$5" dest_name="$6"
  local override="${!env_name:-}"
  if [[ -n "$override" ]]; then
    if [[ ! -f "$override/$marker" ]]; then
      echo "$env_name is set but $marker is missing under $override" >&2
      return 2
    fi
    (cd "$override" && pwd)
    return 0
  fi
  if [[ -f "$repo/$marker" ]]; then
    printf '%s\n' "$repo"
    return 0
  fi
  local dest="$out/worktrees/$dest_name"
  if [[ -f "$dest/$marker" ]]; then
    printf '%s\n' "$dest"
    return 0
  fi
  laptop_require_cmd git
  echo "fetching origin/$ref into a detached worktree at $dest" >&2
  echo "this does not merge and does not push" >&2
  # git writes "Preparing worktree" on stdout. Keep that off the path we print.
  git -C "$repo" fetch --no-tags origin "$ref" >&2
  mkdir -p "$(dirname "$dest")"
  if [[ -e "$dest" ]]; then
    echo "worktree path exists but is missing $marker: $dest" >&2
    return 2
  fi
  git -C "$repo" worktree add --detach "$dest" "origin/$ref" >&2
  if [[ ! -f "$dest/$marker" ]]; then
    echo "origin/$ref does not contain $marker" >&2
    return 2
  fi
  printf '%s\n' "$dest"
}

laptop_audio_snapshot() {
  local repo="$1" dest="$2"
  git -C "$repo" status --porcelain -uall -- \
    '*.wav' '*.wave' '*.flac' '*.mp3' '*.ogg' '*.WAV' >"$dest" || true
}

laptop_audio_unchanged() {
  local before="$1" after="$2"
  if cmp -s "$before" "$after"; then
    return 0
  fi
  echo "own-voice or other audio showed up in the git repo during this run:" >&2
  diff -u "$before" "$after" >&2 || true
  echo "the runbook does not commit audio. Move the file out of the repo." >&2
  return 2
}
