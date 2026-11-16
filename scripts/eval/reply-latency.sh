#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Maciej Grajczyk.
#
# Reply latency from a loopback stub through optional local Piper.
# Prints the example's JSON. Exits 1 when the stub measurement did not
# show the first sentence ahead of the buffered body. Does not call a
# paid model and does not download weights.
#
#   scripts/eval/reply-latency.sh
set -euo pipefail
cd "$(dirname "$0")/../.."

out=$(cargo run -q -p galion-cascade --features cascade --example reply_latency)
printf '%s\n' "$out"
python3 - "$out" <<'PY'
import json, sys
d = json.loads(sys.argv[1])
assert d["measured"] is True, d
assert d["paid"] is False
h = d["hermes"]
assert h["transport"] == "loopback-stub"
assert h["buffered_text_ok"] and h["stream_text_ok"]
assert h["stream_first_sentence_ms"] + 100 < h["buffered_llm_ms"], h
assert h["stream_first_sentence_ms"] < h["stream_done_ms"]
p = d["piper"]
if p["measured"]:
    assert p["cold_spawn_to_pcm_ms"] > p["warm_write_to_pcm_ms"] > 0, p
    assert isinstance(p["warm_samples_ms"], list) and len(p["warm_samples_ms"]) >= 3
    r = d["reply"]
    assert r["stream_first_audio_ms"] < r["buffered_first_audio_ms"], r
    phone = r.get("phone_first_audio_ms")
    if phone is not None:
        assert phone < r["buffered_first_audio_ms"], r
else:
    assert d["reply"]["buffered_first_audio_ms"] is None
    assert d["reply"]["stream_first_audio_ms"] is None
asr = d["asr"]
assert asr["measured"] is True and asr["engine"] == "stub-fixture"
assert asr["n"] >= 10
PY
