# V2-S14 · Chatterbox Nano stream (time to first audio)

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A streaming hook and a gate. Not a model measurement.

The spelling is **Galion**. The product is Galion Voice.

## What was verified

Read on 2026-11-03 from `resemble-ai/chatterbox` master (Nano landed in
`5de7a54`, 2026-07-21; the files below were still the master tree):

| API | On master? | What it does to first audio |
|---|---|---|
| `ChatterboxTurboTTS.generate` | yes | Samples every speech token, then `s3gen.inference(..., finalize=True)`, then Perth. Returns when the utterance is finished. The clone smoke measured 1.8–2.4 s before any audio. |
| `ChatterboxTurboTTS.stream` | no | Proposed on [PR 528](https://github.com/resemble-ai/chatterbox/pull/528), still open. Not in PyPI `chatterbox-tts` 0.1.7. |
| `S3Token2Wav.forward(..., finalize=False, skip_vocoder=True, noised_mels=...)` | yes | Holds back the last 3 speech tokens. The docstring names `S3GenStreamer`, which is not in the tree. |
| `hift_inference(mels, cache_source)` | yes | Vocoder with a source cache. |
| `T3.inference_turbo` | yes | Returns only after the last token. It is not called. |

Speech tokens are 25 Hz (`S3_TOKEN_RATE`). Meanflow uses 2 CFM steps in
`generate`. `basic_euler` prints `S3 Token -> Mel Inference...` on stdout.
That pipe is the PCM wire, so the worker duplicates stdout onto stderr
before the import.

## Hook

`scripts/clone/nano_stream.py` chooses one path:

1. `model.stream`, if the local install grew it (PR 528 or a later merge). `chunk_tokens` is 2, not the PR default of 24. Twenty-four tokens is 960 ms of speech before the first flush.
2. Otherwise a token loop with the same KV cache as `inference_turbo`, yielding each token, then meanflow on the stable prefix. Lookahead is 3. The first vocoder call is at 5 tokens: 80 ms of stable speech plus the lookahead the flow drops. Noise already drawn is kept. Each flush vocodes the prefix and yields the new tail.
3. If the object can only speak through `generate`, the worker writes no PCM. `SpeechStack` hands the line to Piper, then flite. Piper is unchanged.

`GALION_CLONE_STREAM=1` is the opt-in. It needs `GALION_CLONE_CKPT`, a flat
directory (`ve.safetensors`, `s3gen_meanflow.safetensors`,
`t3_nano_v1.safetensors`, `vocab.json`). The pinned tree under
`weights/clone/` is not that directory. Nothing here downloads it.

A reference clip still exits 3. `GALION_CLONE_LOCAL_REFS=1` plus
`GALION_CLONE_REF_SHA` or `GALION_CLONE_REF_ALLOW` is the operator
allowlist. `LOCKS.toml` is not read and was not edited.

Streamed chunks are not Perth-watermarked. Watermarking in upstream is
inside `generate()`, and that call is the latency we are removing. Perth
is not vendored.

## Clock and gate

`scripts/eval/clone-stream-ttfa.sh` prints one JSON object and exits 1 on
FAIL.

| Check | FAIL when |
|---|---|
| Source | `.generate(` appears in the worker or `nano_stream.py` |
| Path | stderr `galion-clone-path` is not `stream`, or no chunk marker precedes `galion-clone-utt:` |
| Whole utterance | first PCM is at or after 1500 ms, or it arrives with a finished utterance that long |
| Heartbeat | `crates/galion-heartbeat` names `generate(`, `chatterbox`, or `CloneFrameDrain` |
| Reference | a clip without the local flag does not exit 3 with an empty stdout |
| Fixture | an in-script whole-utterance child is not judged FAIL |

The protocol stub splits its old tone into two writes with a 50 ms gap.
That proves the pipe. It is not a Nano TTFA. `model_measured` stays false
until `GALION_CLONE_STREAM_EVAL=1` and the child reports `engine=model`.
Do not copy the stub's `ttfa_ms` into `LEDGER.md`.

The 80–160 ms band is the budget for the first hearable chunk: one
heartbeat of speech tokens, plus the lookahead and one short meanflow
pass. The smoke's RTF was about 0.49 on CPU, so the token loop is the
part that used to be hidden behind the vocoder. Whether a given box lands
inside 80–160 ms is a measured `engine=model` run, not a number this tree
invents.

`examples/flash_ttfa.rs` uses the same first-byte clock. `product_claim`
is true only for `engine=model` and `path=stream`.

## Heartbeat

`Heartbeat::step` is unchanged. It does not allocate and it does not
generate. The playback ring is
`CloneFrameDrain` in the clone worker: one producer, one consumer, four
frames, no lock. `drain_frame` copies 1920 samples into a caller buffer
and returns. A full ring drops the newest samples. One call never plays
the backlog.

The host does this beside the tick, not inside it:

```text
Heartbeat::step(...)
drain_frame(&mut frame)   // no generate, no allocation
```

The worker thread is the only producer. `synthesize` still returns the
whole clause for callers that want one buffer. The ring is how the tick
hears the first frame while that call is still reading.

## Piper

`SpeechStack::fallback_piper` is the same handoff. Unwired, a refused
hook, or a child that writes no PCM falls through to Piper, then flite.
`GALION_SPEECH=auto` without the pinned weights never selects the clone
slot.

— Maciej Grajczyk
