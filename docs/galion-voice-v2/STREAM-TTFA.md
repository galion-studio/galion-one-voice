# V2-S14 · Chatterbox Nano stream (time to first audio)

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Merge is held (admin t298 and t299).

The spelling is **Galion**. The product is Galion Voice.

## HARD-PASS

A HARD-PASS, and any merge gate for this PR, needs real engine PCM
measured on this box. The engine is Piper (`GALION_SPEECH=a21`,
`en_US-ljspeech-medium`) or Chatterbox Nano when a local CPU run actually
calls `stream` or `generate` and returns PCM. `product_claim` is true
only for that PCM.

A protocol-stub clock does not count. The 15 ms first-PCM figure was the
stub. `product_claim` is false on that clock. It is not a TTFA result and
it is not an 80 ms result. The two-token flush described below is 80 ms
of speech-token time at 25 Hz. That arithmetic was not measured here.

`LOCKS.toml` was not edited. No WAV is committed.

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
2. Otherwise a token loop with the same KV cache as `inference_turbo`, yielding each token, then meanflow on the stable prefix. Lookahead is 3 tokens. The first vocoder call is planned at 5 tokens (lookahead plus 2). At 25 Hz those 2 tokens are 80 ms of speech time. That is a plan. It is not a measured TTFA. Noise already drawn is kept. Each flush vocodes the prefix and yields the new tail.
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

`GALION_SPEECH=a21 scripts/eval/clone-stream-ttfa.sh` is the live gate.
Piper is the hard path. Nano streaming stays in `scripts/clone/nano_stream.py`.

The gate prints `PASS` only when Piper `en_US-ljspeech-medium` has written
real PCM, `engine` is `piper`, `product_claim` is true, and
`piper_measured` is true. A protocol-stub clock, including a 15 ms pipe
clock, is `FAIL`. `product_claim` stays false for that clock. `stub` on
the Piper report stays false.

CC0 source URLs are listed in `scripts/eval/sample-voices/SAMPLE-VOICES.md`.
The audio files are not in git. Piper does not condition on them.

## Why Nano has no real clock on this box

A Nano HARD-PASS needs torch, the pinned safetensors, and a CPU call that
returns PCM from `stream` or from the chunked meanflow path. This box has
none of that:

- `python3` has no `torch`.
- `weights/clone/` is absent, so the pinned Nano tensors are not on disk.
- Master `ChatterboxTurboTTS` has `generate()`, which returns when the
  utterance is finished. `stream()` is still only on open PR 528.
- No `GALION_CLONE_CKPT` was loaded, and no Nano PCM was written.

`model_measured` stays false. The clone smoke's 1.8–2.4 s figure is from
another machine. It is not a clock from this box, and it is not a stub.

### Coordinator box (t301)

Reported by the coordinator. Not re-labeled as a different run.

| | |
|---|---|
| cold spawn, first PCM byte | 393.2 ms |
| SpeechStack cold first clause | 530 ms |
| SpeechStack warm first clause | 18 ms |
| voice | `en_US-ljspeech-medium` |
| engine | `piper` |
| product_claim | true |
| stub | false |

### This agent, same gate, 2026-11-03

`GALION_SPEECH=a21 scripts/eval/clone-stream-ttfa.sh` on this CPU after
`scripts/a21/fetch-a21.sh`. Cold spawn, first PCM byte. Not the
SpeechStack clock above. Two runs, not averaged.

| run | ttfa_ms | utt_ms | warm_ttfa_ms | pcm_bytes |
|---|---|---|---|---|
| 1 | 379.0 | 379.1 | 67.1 | 25088 |
| 2 | 331.8 | 331.8 | 37.8 | 25088 |
| 3 | 315.0 | 319.4 | 39.3 | 25088 |

Both runs are real Piper PCM on this box. Voice
`en_US-ljspeech-medium`, engine `piper`, `product_claim` true,
`piper_measured` true, `model_measured` false, `stub` false. They are
not an 80 ms claim. Admin t298 and t299 still hold merge. A stub clock
with `product_claim` false does not add to this table.

Without `GALION_SPEECH=a21` the same script exits 2 and prints `FAIL`.
That is the stub rejection, not a pass.

`Heartbeat::step` is unchanged. A late frame stays a pause. `drain_frame`
copies one 1920-sample frame outside `step` and does not generate.

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
