#!/bin/sh
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
# Clone PCM → voices/compiled/<persona>.gq. Does not start PyTorch.
set -eu
cd "$(dirname "$0")/../.."
exec cargo run --quiet -p galion-ml-voice --bin galion-voice-compile -- "$@"
