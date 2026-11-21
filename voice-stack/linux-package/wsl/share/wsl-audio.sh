#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Source from a shell. Exports the WSLg PulseAudio socket when it exists.
# shellcheck shell=bash

GALION_CPU=1
export GALION_CPU
sock=""
if [ "${GALION_WSLG_PULSE:-}" = "skip" ]; then
  sock=""
elif [ -n "${GALION_WSLG_PULSE:-}" ] && [ -S "${GALION_WSLG_PULSE#unix:}" ]; then
  sock=${GALION_WSLG_PULSE#unix:}
elif [ -S /mnt/wslg/PulseServer ]; then
  sock=/mnt/wslg/PulseServer
elif [ -S /mnt/wslg/runtime-dir/pulse/native ]; then
  sock=/mnt/wslg/runtime-dir/pulse/native
fi
if [ -n "$sock" ]; then
  PULSE_SERVER="unix:${sock}"
  export PULSE_SERVER
  if [ -f "${HOME:-}/.config/galion/one-voice.asoundrc" ]; then
    ALSA_CONFIG_PATH="${HOME}/.config/galion/one-voice.asoundrc"
    export ALSA_CONFIG_PATH
  fi
else
  unset PULSE_SERVER
fi
unset sock
