#!/bin/sh
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
# Clone PCM → voices/compiled/<persona>.gq. Does not start PyTorch.
set -eu
cd "$(dirname "$0")/../.."
exec cargo run --quiet -p galion-ml-voice --bin galion-voice-compile -- "$@"
