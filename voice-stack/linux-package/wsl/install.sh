#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Idempotent WSL install for Galion One Voice.
# Builds the voice-stack workspace on CPU, fetches the public models into
# gitignored var/, points ALSA at the WSLg PulseAudio server, and installs a
# user unit on 127.0.0.1:7881.
#
#   ./install.sh
#   ./install.sh --dry-run
#   ./install.sh --check
#   ./install.sh --skip-build --skip-models
#   ./install.sh --prefix DIR
#   ./install.sh --skip-audio
#   ./install.sh --require-audio
#   ./install.sh --require-systemd
#
# No sudo. No paid API. No reference clip. No deploy.

set -euo pipefail

ROOT=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=../lib/common.sh
source "$ROOT/../lib/common.sh"
# shellcheck source=../lib/models.sh
source "$ROOT/../lib/models.sh"
# shellcheck source=lib/audio.sh
source "$ROOT/lib/audio.sh"
# shellcheck source=lib/silero.sh
source "$ROOT/lib/silero.sh"

DRY_RUN=0
CHECK_ONLY=0
SKIP_MODELS=0
SKIP_BUILD=0
SKIP_AUDIO=0
REQUIRE_AUDIO=0
REQUIRE_SYSTEMD=0
PREFIX=""

usage() {
  cat <<'EOF'
usage: install.sh [--dry-run | --check] [--skip-build] [--skip-models]
                  [--skip-audio | --require-audio] [--require-systemd]
                  [--prefix DIR]

CPU build of the voice-stack workspace, public models under
voice-stack/linux-package/wsl/var/ (gitignored), WSLg PulseAudio, and a
systemd user unit that binds 127.0.0.1:7881.

Windows reaches that port through WSL localhost forwarding. The bind stays
on numeric loopback.

--dry-run prints the plan and writes nothing. --check re-reads files and
does not use the network. A second install leaves matching files unchanged.
--prefix DIR installs the unit under DIR as if it were $HOME and does not
call systemctl. --skip-audio skips PulseAudio setup, the same result as a
container with no WSLg socket.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    --check) CHECK_ONLY=1 ;;
    --skip-build) SKIP_BUILD=1 ;;
    --skip-models) SKIP_MODELS=1 ;;
    --skip-audio) SKIP_AUDIO=1 ;;
    --require-audio) REQUIRE_AUDIO=1 ;;
    --require-systemd) REQUIRE_SYSTEMD=1 ;;
    --prefix)
      shift
      [[ $# -gt 0 ]] || die "missing value for --prefix" 2
      PREFIX=$1
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1" 2 ;;
  esac
  shift
done

if [[ "$DRY_RUN" == 1 && "$CHECK_ONLY" == 1 ]]; then
  die "use either --dry-run or --check" 2
fi
if [[ "$SKIP_AUDIO" == 1 && "$REQUIRE_AUDIO" == 1 ]]; then
  die "use either --skip-audio or --require-audio" 2
fi
if [[ "$(id -u)" -eq 0 && -z "$PREFIX" ]]; then
  die "user install does not use sudo. Re-run as your user, or pass --prefix for a throwaway home." 2
fi

REPO=$(CDPATH='' cd -- "$ROOT/../../.." && pwd)
if [[ -n "${GALION_VOICE_VAR:-}" ]]; then
  VAR=$GALION_VOICE_VAR
  case "$VAR" in
    /*) ;;
    *) VAR="$PWD/$VAR" ;;
  esac
else
  VAR="$ROOT/var"
fi
case "$VAR" in
  "$REPO" | "$REPO"/*)
    if [[ "$VAR" != "$ROOT/var" && "$VAR" != "$ROOT/var/"* ]]; then
      die "model directory inside the repo must be voice-stack/linux-package/wsl/var" 2
    fi
    ;;
esac

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

refuse_plain_path "$VAR" "model directory"
refuse_plain_path "$REPO" "repository path"
refuse_plain_path "$layout_home" "home"

bin_dir="$layout_home/.local/bin"
unit_dir="$layout_home/.config/systemd/user"
dropin_dir="$unit_dir/galion-one-voice.service.d"
state_dir="$layout_home/.local/share/galion/one-voice"
conf_dir="$layout_home/.config/galion"
env_dir="$layout_home/.config/environment.d"

dest_bin="$bin_dir/galion-one-voice"
dest_unit="$unit_dir/galion-one-voice.service"
dest_dropin="$dropin_dir/paths.conf"
dest_env="$conf_dir/one-voice.env"
dest_asound="$conf_dir/one-voice.asoundrc"
dest_snippet="$conf_dir/wsl-audio.sh"
src_snippet="$ROOT/share/wsl-audio.sh"
dest_envd="$env_dir/90-galion-one-voice.conf"
dest_bashrc="$layout_home/.bashrc"
src_bin="$ROOT/share/galion-one-voice"
src_unit="$ROOT/share/systemd/user/galion-one-voice.service"
pins="$ROOT/../share/models.pins"
target_dir="$REPO/voice-stack/target"
release_dir="$target_dir/release"
ui_dir="$REPO/voice-stack/local-server/ui"
pkg_bin="$REPO/voice-stack/linux-package/target/release/galion-samantha"
manifest="$REPO/voice-stack/Cargo.toml"

begin_mark="# BEGIN galion-one-voice"
end_mark="# END galion-one-voice"
# Literal text for the user's bashrc. The dollar signs must not expand here.
# shellcheck disable=SC2016
bashrc_line='[ -f "$HOME/.config/galion/wsl-audio.sh" ] && . "$HOME/.config/galion/wsl-audio.sh"'

AUDIO_SOCK=""
AUDIO_STATE=SKIPPED

commit_text() {
  local dest=$1 mode=$2 tmp
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "dry-run: would install $dest (mode $mode)"
    cat >/dev/null
    return 0
  fi
  tmp=$(mktemp)
  cat >"$tmp"
  if [[ "$CHECK_ONLY" == 1 ]]; then
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
    local got want_n got_n
    got=$(stat -c '%a' "$dest")
    want_n=$((8#$mode))
    got_n=$((8#$got))
    if [[ "$want_n" -ne "$got_n" ]]; then
      rm -f "$tmp"
      echo "MISMATCH mode $dest" >&2
      return 1
    fi
    rm -f "$tmp"
    echo "ok   $dest"
    return 0
  fi
  mkdir -p "$(dirname "$dest")"
  if [[ -f "$dest" ]] && cmp -s "$tmp" "$dest"; then
    chmod "$mode" "$dest"
    rm -f "$tmp"
    echo "unchanged $dest"
    return 0
  fi
  chmod "$mode" "$tmp"
  mv -f "$tmp" "$dest"
  echo "installed $dest"
}

remove_owned() {
  local dest=$1
  if [[ "$DRY_RUN" == 1 ]]; then
    if [[ -e "$dest" ]]; then
      echo "dry-run: would remove $dest"
    else
      echo "dry-run: would leave $dest absent"
    fi
    return 0
  fi
  if [[ "$CHECK_ONLY" == 1 ]]; then
    if [[ -e "$dest" ]]; then
      echo "MISMATCH $dest (expected absent)" >&2
      return 1
    fi
    echo "ok   absent $dest"
    return 0
  fi
  if [[ -e "$dest" ]]; then
    rm -f "$dest"
    echo "removed $dest"
  else
    echo "unchanged absent $dest"
  fi
}

note_wsl() {
  if [[ -n "${WSL_DISTRO_NAME:-}" ]]; then
    echo "wsl: distro ${WSL_DISTRO_NAME}"
  elif grep -qi microsoft /proc/version 2>/dev/null; then
    echo "wsl: Microsoft kernel"
  else
    echo "wsl: no WSL marker in this environment"
  fi
  if [[ -f /etc/wsl.conf ]] && grep -Eq '^[[:space:]]*systemd[[:space:]]*=[[:space:]]*true[[:space:]]*$' /etc/wsl.conf; then
    echo "wsl: systemd=true in /etc/wsl.conf"
  elif [[ -n "${WSL_DISTRO_NAME:-}" ]] || grep -qi microsoft /proc/version 2>/dev/null; then
    echo "wsl: systemd user units need [boot] systemd=true in /etc/wsl.conf. This script does not edit that file."
  fi
}

configure_audio() {
  local sock=""
  AUDIO_SOCK=""
  AUDIO_STATE=SKIPPED
  if sock=$(resolve_pulse_socket); then
    refuse_plain_path "$sock" "PulseAudio socket"
    AUDIO_SOCK=$sock
    AUDIO_STATE=$(probe_pulse "$sock")
  fi
  echo "WSL_AUDIO=${AUDIO_STATE}"
  case "$AUDIO_STATE" in
    SKIPPED)
      echo "wsl-audio: no WSLg PulseAudio socket; audio check skipped"
      ;;
    OK)
      echo "wsl-audio: PulseAudio server unix:${AUDIO_SOCK}"
      ;;
    SOCKET)
      echo "wsl-audio: socket unix:${AUDIO_SOCK} (pactl is not installed; the socket check is enough)"
      ;;
    UNREACHABLE)
      echo "wsl-audio: socket unix:${AUDIO_SOCK} did not answer pactl; continuing" >&2
      ;;
    *)
      echo "wsl-audio: unknown state ${AUDIO_STATE}" >&2
      return 1
      ;;
  esac
  if [[ -n "$AUDIO_SOCK" ]] && ! pulse_plugin_present; then
    echo "wsl-audio: ALSA Pulse plugin not found. Install libasound2-plugins so capture can reach WSLg."
  fi
  if [[ "$REQUIRE_AUDIO" == 1 && "$AUDIO_STATE" != "OK" && "$AUDIO_STATE" != "SOCKET" ]]; then
    echo "wsl-audio: --require-audio and the Pulse server is not usable" >&2
    return 1
  fi
  return 0
}

ensure_dirs() {
  local dir missing
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "dry-run: would create $bin_dir"
    echo "dry-run: would create $state_dir"
    echo "dry-run: would create $unit_dir"
    echo "dry-run: would create $conf_dir"
    return 0
  fi
  if [[ "$CHECK_ONLY" == 1 ]]; then
    missing=0
    for dir in "$bin_dir" "$state_dir" "$unit_dir" "$conf_dir" "$env_dir"; do
      if [[ ! -d "$dir" ]]; then
        echo "MISSING $dir" >&2
        missing=1
      fi
    done
    return "$missing"
  fi
  mkdir -p "$bin_dir" "$state_dir" "$unit_dir" "$dropin_dir" "$conf_dir" "$env_dir" "$VAR"
  if [[ "$SKIP_BUILD" != 1 ]]; then
    mkdir -p "$release_dir"
  fi
  chmod 0700 "$state_dir" "$conf_dir"
}

rustc_at_least_185() {
  local v rest major minor
  v=$(rustc --version 2>/dev/null | awk '{print $2}')
  [[ -n "$v" ]] || return 1
  major=${v%%.*}
  rest=${v#*.}
  minor=${rest%%.*}
  [[ "$major" =~ ^[0-9]+$ && "$minor" =~ ^[0-9]+$ ]] || return 1
  [[ "$major" -gt 1 ]] && return 0
  [[ "$major" -eq 1 && "$minor" -ge 85 ]]
}

build_workspace() {
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "dry-run: would cargo build --release --locked --manifest-path $manifest"
    echo "dry-run: CARGO_TARGET_DIR=$target_dir (CPU; the vad crate stays excluded)"
    return 0
  fi
  if [[ "$SKIP_BUILD" == 1 ]]; then
    echo "build: skipped"
    return 0
  fi
  if [[ "$CHECK_ONLY" == 1 ]]; then
    if [[ -x "$release_dir/samantha-local" || -x "$release_dir/galion-samantha" || -x "$pkg_bin" ]]; then
      echo "ok   workspace release binary"
      return 0
    fi
    echo "MISSING workspace release binary under $release_dir" >&2
    return 1
  fi
  if ! command -v cargo >/dev/null 2>&1 || ! command -v rustc >/dev/null 2>&1; then
    die "cargo and rustc 1.85 or newer are required. Install rustup, then re-run." 2
  fi
  if ! rustc_at_least_185; then
    die "rustc $(rustc --version | awk '{print $2}') is older than 1.85. The voice-stack workspace asks for 1.85." 2
  fi
  local -a missing=()
  if ! command -v gcc >/dev/null 2>&1; then
    missing+=(build-essential)
  fi
  if ! command -v pkg-config >/dev/null 2>&1; then
    missing+=(pkg-config)
  elif ! pkg-config --exists alsa; then
    missing+=(libasound2-dev)
  fi
  if [[ ${#missing[@]} -gt 0 ]]; then
    die "missing CPU build packages: ${missing[*]}. Install them with apt, then re-run." 2
  fi
  echo "build: CPU release of the voice-stack workspace (no CUDA feature; vad/ort stays excluded)"
  if ! CARGO_TARGET_DIR="$target_dir" GALION_CPU=1 \
    cargo build --release --locked --manifest-path "$manifest"; then
    echo "wsl: workspace build failed" >&2
    return 1
  fi
}

fetch_models() {
  local rc=0
  if [[ "$SKIP_MODELS" == 1 ]]; then
    echo "models: skipped"
    return 0
  fi
  install_public_models "$VAR" "$pins" || rc=1
  if [[ "$rc" -eq 0 ]]; then
    fetch_silero "$VAR" || rc=1
  fi
  if [[ "$rc" -eq 0 ]]; then
    smoke_piper "$VAR" || rc=1
  fi
  return "$rc"
}

env_text() {
  cat <<EOF
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Generated by voice-stack/linux-package/wsl/install.sh.
GALION_CPU=1
GALION_SAMANTHA_WS=127.0.0.1:7881
GALION_SAMANTHA_HOME=${VAR}
GALION_SILERO_VAD=${VAR}/models/vad/silero_vad.onnx
GALION_VOSK_MODEL=${VAR}/models/asr/vosk-model-small-en-us-0.15
GALION_VOSK_LIB=${VAR}/models/asr/libvosk/libvosk.so
GALION_A21_MODEL=${VAR}/models/tts/en_US-ljspeech-medium.onnx
GALION_A21_RUNTIME=${VAR}/models/tts/piper/piper
GALION_ASR=vosk
GALION_SPEECH=a21
GALION_VOICE_TARGET=${release_dir}
GALION_VOICE_UI=${ui_dir}
GALION_PACKAGE_BIN=${pkg_bin}
GALION_ONE_VOICE_ENV=${dest_env}
EOF
  if [[ -n "$AUDIO_SOCK" ]]; then
    printf 'PULSE_SERVER=unix:%s\n' "$AUDIO_SOCK"
    printf 'ALSA_CONFIG_PATH=%s\n' "$dest_asound"
  fi
}

dropin_text() {
  local sock_dir
  cat <<EOF
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Paths for the WSL unit. install.sh rewrites this file.
[Service]
ReadOnlyPaths=${VAR}
ReadOnlyPaths=${release_dir}
ReadOnlyPaths=${ui_dir}
EOF
  if [[ -n "$AUDIO_SOCK" ]]; then
    sock_dir=$(dirname "$AUDIO_SOCK")
    printf 'ReadOnlyPaths=%s\n' "$sock_dir"
    printf 'ReadOnlyPaths=%s\n' "$dest_asound"
  fi
}

envd_text() {
  printf '%s\n' '# SPDX-License-Identifier: MIT'
  printf '%s\n' '# Copyright (c) 2026 Galion Labs'
  printf '%s\n' 'GALION_CPU=1'
  if [[ -n "$AUDIO_SOCK" ]]; then
    printf 'PULSE_SERVER=unix:%s\n' "$AUDIO_SOCK"
    printf 'ALSA_CONFIG_PATH=%s\n' "$dest_asound"
  fi
}

ensure_bashrc() {
  local tmp
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "dry-run: would ensure the galion-one-voice block in $dest_bashrc"
    return 0
  fi
  if [[ "$CHECK_ONLY" == 1 ]]; then
    if [[ -f "$dest_bashrc" ]] && grep -qxF "$begin_mark" "$dest_bashrc" && grep -qxF "$bashrc_line" "$dest_bashrc"; then
      echo "ok   $dest_bashrc"
      return 0
    fi
    echo "MISSING $dest_bashrc" >&2
    return 1
  fi
  if [[ -f "$dest_bashrc" ]] && grep -qxF "$begin_mark" "$dest_bashrc" && grep -qxF "$bashrc_line" "$dest_bashrc" && grep -qxF "$end_mark" "$dest_bashrc"; then
    echo "unchanged $dest_bashrc"
    return 0
  fi
  tmp=$(mktemp)
  if [[ -f "$dest_bashrc" ]]; then
    awk -v b="$begin_mark" -v e="$end_mark" '
      $0 == b { skip = 1; next }
      $0 == e { skip = 0; next }
      skip { next }
      { print }
    ' "$dest_bashrc" >"$tmp"
  fi
  {
    if [[ -s "$tmp" ]]; then
      cat "$tmp"
      printf '\n'
    fi
    printf '%s\n%s\n%s\n' "$begin_mark" "$bashrc_line" "$end_mark"
  } >"$dest_bashrc"
  rm -f "$tmp"
  echo "installed $dest_bashrc"
}

write_tree() {
  local rc=0
  install_file "$src_bin" "$dest_bin" 0755 || rc=1
  install_file "$src_unit" "$dest_unit" 0644 || rc=1
  install_file "$src_snippet" "$dest_snippet" 0644 || rc=1
  commit_text "$dest_env" 0644 < <(env_text) || rc=1
  commit_text "$dest_dropin" 0644 < <(dropin_text) || rc=1
  commit_text "$dest_envd" 0644 < <(envd_text) || rc=1
  if [[ -n "$AUDIO_SOCK" ]]; then
    commit_text "$dest_asound" 0644 < <(asoundrc_text "$AUDIO_SOCK") || rc=1
  else
    remove_owned "$dest_asound" || rc=1
  fi
  ensure_bashrc || rc=1
  return "$rc"
}

enable_user_unit() {
  if [[ -n "$PREFIX" ]]; then
    echo "prefix install: systemctl --user skipped"
    return 0
  fi
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "dry-run: would systemctl --user daemon-reload"
    echo "dry-run: would systemctl --user enable galion-one-voice.service"
    return 0
  fi
  if [[ "$CHECK_ONLY" == 1 ]]; then
    return 0
  fi
  if ! command -v systemctl >/dev/null 2>&1; then
    echo "wsl: systemctl is not installed. The unit file is in place." >&2
    [[ "$REQUIRE_SYSTEMD" == 1 ]] && return 1
    return 0
  fi
  if ! systemctl --user daemon-reload; then
    echo "wsl: systemd --user bus is not available. The unit file is in place." >&2
    echo "wsl: when a user session exists: systemctl --user enable --now galion-one-voice.service" >&2
    [[ "$REQUIRE_SYSTEMD" == 1 ]] && return 1
    return 0
  fi
  systemctl --user enable galion-one-voice.service
  echo "enabled galion-one-voice.service"
  if [[ -x "$release_dir/samantha-local" || -x "$release_dir/galion-samantha" || -x "$pkg_bin" ]]; then
    if systemctl --user start galion-one-voice.service; then
      echo "started galion-one-voice.service on 127.0.0.1:7881"
    else
      echo "wsl: the unit is enabled and did not start. If 127.0.0.1:7881 is taken, stop the other listener and run systemctl --user start galion-one-voice.service" >&2
      return 1
    fi
  else
    echo "wsl: unit enabled. The server binary is not built yet, so it was not started."
  fi
}

rc=0
note_wsl || rc=1
if [[ "$rc" -eq 0 ]]; then
  configure_audio || rc=1
fi
if [[ "$rc" -eq 0 ]]; then
  ensure_dirs || rc=1
fi
if [[ "$rc" -eq 0 ]]; then
  build_workspace || rc=1
fi
if [[ "$rc" -eq 0 ]]; then
  fetch_models || rc=1
fi
if [[ "$rc" -eq 0 ]]; then
  write_tree || rc=1
fi
if [[ "$rc" -eq 0 ]]; then
  enable_user_unit || rc=1
fi

if [[ "$rc" -eq 0 ]]; then
  if [[ "$DRY_RUN" == 1 ]]; then
    echo "GALION_WSL_INSTALL=DRY_RUN"
  elif [[ "$CHECK_ONLY" == 1 ]]; then
    echo "GALION_WSL_INSTALL=CHECKED"
  else
    echo "GALION_WSL_INSTALL=OK"
  fi
else
  echo "GALION_WSL_INSTALL=FAILED" >&2
fi
exit "$rc"
