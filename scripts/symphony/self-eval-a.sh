#!/bin/sh
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
# Section A gate. PASS is a zero exit and the lines in
# docs/galion-voice-v2/symphony/SELF-EVAL.md.
set -eu
cd "$(dirname "$0")/../.."
echo "section A compiler — artifact unit tests"
cargo test -p galion-ml-voice --lib artifact::
echo "section A compiler — runtime path, alloc, tick"
cargo test -p galion-ml-voice --test voice_artifact -- --test-threads=1 --nocapture
echo "section A PASS"
