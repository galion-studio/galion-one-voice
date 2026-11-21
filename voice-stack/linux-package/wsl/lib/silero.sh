#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# shellcheck shell=bash
# Silero VAD v5 ONNX. The file is the MIT model, not a voice and not a clip.

_silero_lib=$(CDPATH='' cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=../../lib/models.sh
source "$_silero_lib/../../lib/models.sh"

fetch_silero() {
  local data=$1
  local pin="$_silero_lib/../share/silero.pin"
  # shellcheck disable=SC1090
  source "$pin"
  : "${SILERO_URL:?}"
  : "${SILERO_SHA256:?}"
  fetch_checked "$SILERO_URL" "$data/models/vad/silero_vad.onnx" "$SILERO_SHA256"
}
