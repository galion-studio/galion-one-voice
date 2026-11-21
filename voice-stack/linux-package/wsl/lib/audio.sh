#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# shellcheck shell=bash
# WSLg PulseAudio discovery. No sudo. No network. No audio is recorded.

_audio_lib=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=../../lib/common.sh
source "$_audio_lib/../../lib/common.sh"

# Characters this installer will not write into a unit, an env file, or asoundrc.
refuse_plain_path() {
  local value=$1 label=$2
  case "$value" in
    *[$'\n\r'"'"'"$`;&|<>*?!'\\]*)
      die "$label contains a character this installer will not write into a unit" 2
      ;;
  esac
}

# Print the unix socket of the WSLg PulseAudio server.
# GALION_WSLG_PULSE=skip and SKIP_AUDIO=1 force a miss.
# An explicit GALION_WSLG_PULSE path is used when it is a socket.
# Otherwise prefer /mnt/wslg/PulseServer, the socket WSLg publishes.
resolve_pulse_socket() {
  local path cand
  if [[ "${GALION_WSLG_PULSE:-}" == "skip" || "${SKIP_AUDIO:-0}" == 1 ]]; then
    return 1
  fi
  if [[ -n "${GALION_WSLG_PULSE:-}" ]]; then
    path=${GALION_WSLG_PULSE#unix:}
    if [[ -S "$path" ]]; then
      printf '%s\n' "$path"
      return 0
    fi
    echo "wsl-audio: GALION_WSLG_PULSE is not a socket ($path)" >&2
    return 1
  fi
  local -a cands=(
    /mnt/wslg/PulseServer
    /mnt/wslg/runtime-dir/pulse/native
  )
  if [[ -n "${PULSE_SERVER:-}" ]]; then
    cands+=("${PULSE_SERVER}")
  fi
  if [[ -n "${XDG_RUNTIME_DIR:-}" ]]; then
    cands+=("${XDG_RUNTIME_DIR}/pulse/native")
  fi
  for cand in "${cands[@]}"; do
    path=${cand#unix:}
    case "$path" in
      tcp:*|TCP:*|"") continue ;;
    esac
    if [[ -S "$path" ]]; then
      printf '%s\n' "$path"
      return 0
    fi
  done
  return 1
}

# OK when pactl answers, SOCKET when the socket exists and pactl is absent,
# UNREACHABLE when the socket exists and pactl does not answer.
probe_pulse() {
  local sock=$1 rc
  if ! command -v pactl >/dev/null 2>&1; then
    printf '%s\n' SOCKET
    return 0
  fi
  set +e
  if command -v timeout >/dev/null 2>&1; then
    PULSE_SERVER="unix:${sock}" timeout 5 pactl info >/dev/null 2>&1
  else
    PULSE_SERVER="unix:${sock}" pactl info >/dev/null 2>&1
  fi
  rc=$?
  set -e
  if [[ "$rc" -eq 0 ]]; then
    printf '%s\n' OK
  else
    printf '%s\n' UNREACHABLE
  fi
}

asoundrc_text() {
  local sock=$1
  cat <<EOF
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# WSLg PulseAudio client. voice-stack/linux-package/wsl/install.sh writes this.
# ALSA_CONFIG_PATH points here. ~/.asoundrc is left alone.
pcm.!default {
  type pulse
  server "unix:${sock}"
}
ctl.!default {
  type pulse
  server "unix:${sock}"
}
EOF
}

# True when an ALSA config file mentions Pulse or PipeWire.
# audio-io reaches WSLg through that plugin. This only looks at filenames.
pulse_plugin_present() {
  local dir entry name
  for dir in /usr/share/alsa/alsa.conf.d /etc/alsa/conf.d /usr/share/alsa; do
    [[ -d "$dir" ]] || continue
    for entry in "$dir"/*; do
      [[ -e "$entry" ]] || continue
      name=${entry##*/}
      name=${name,,}
      case "$name" in
        *pulse*|*pipewire*) return 0 ;;
      esac
    done
  done
  return 1
}
