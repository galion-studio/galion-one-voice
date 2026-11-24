# Phase 1 deep dive — domain-04 timestamps

Owner: Mate Grajczyk, Galion Labs. Settled 2026-11-18, Q1 option A: HORBM retrieval answers first on the hot path. A small local S0 model fills gaps marked as guesses later. Galion Research (the Rust engine in `hermes-rs` / `hermes-gateway`; the product name is Galion Research) handles hard questions off the tick with a 3 second cap. This phase does not build retrieval, the index, or that side band.

Plan: HORBM R4, `docs/horbm/research/R4-cpu-realtime-integration.md` on PR #207 (`cursor/horbm-r4-cpu-realtime-2180`). Section 11, phase 1.

## What this phase builds

The domain-04 list, and nothing after it:

`t_vad_silence_start`, `t_eou_decided`, `t_asr_final`, `t_llm_req`, `t_llm_first_token`, `t_first_clause_flushed`, `t_tts_first_pcm`

One `turn_id`. Values are heartbeat milliseconds (`frame × 80`, `galion_heartbeat::FRAME_MS`). A missing stage is absent. Lengths travel with the sheet (`pcm_samples`, `text_chars`, `reply_chars`, `audio_samples`). The sheet does not store a transcript, a reply, or PCM.

`t_first_media_frame_sent` and `t_first_audio_heard` are outside this list. They stay out.

## What already existed

| Piece | Path | State |
|---|---|---|
| Heartbeat clock | `crates/galion-heartbeat/src/lib.rs` `FRAME_MS = 80` | Works. `Heartbeat::step` never allocates and never blocks. Stamps stay off that call. |
| Process bus | `crates/galion-console/src/bus.rs` (`phone-turn`) | Works. `BusMsg.t_ms` is heartbeat time. `try_send` never waits. Bodies already carry transcripts (`PartialAsr`, `ReplyText`) and PCM (`Utterance`, `SpeechPcm`). Those bodies are not the stamp log. |
| Audio heartbeat bus | `voice-stack/audio-io/src/bus.rs` | Works for 24 kHz frames. It is not the turn-stamp lane. |
| Cascade traits | `crates/galion-cascade/src/lib.rs` | Mock ASR → LLM → TTS. No VAD and no end-of-utterance. |
| Phone cascade | `crates/galion-cascade/src/phone.rs` (feature `cascade`) | Real S4/S5/S6 path. `StageMs` is wall durations, not the domain-04 list. |
| Turn worker | `crates/galion-console/src/turn_worker.rs` | Runs `PhoneLane` off the 80 ms thread. Publishes reply text and PCM. |
| `galion-tensor` | `crates/galion-tensor` | SIMD is opt-in behind features, scalar by default. Phase 1 does not call it. |
| HORBM brain | `horbm/` (workspace `exclude`) | Cards, mood, research cap. No domain-04 stamps. Not linked into this phase. |
| Reply bridge | `voice-stack/horbm-bridge` | Clause splitter and a mood-tick timer (`measure.rs`). Not the domain-04 list. Its own Cargo workspace. |
| Latency harness | `voice-stack/latency-harness` | Times a pipeline and keeps fixed phrases (`hello there`). Not reused. A stamp log must not carry those words. |
| Domain 04 | `docs/research/galion-voice-v2/domains/04-latency-streaming.md` §7 | The name list. Research text, not a measurement. |
| Locks | `LOCKS.toml` | `frame_ms = 80`. Rika does not read transcripts. Consent defaults to deny. No in-tree `ort`. Not edited. |
| `SOUL.md` | not in this repo | Nothing to edit. |
| galion-knowledge / obsidian-open-mind | not vendored here | Phase 2's memory schema. Not cloned. |

## Gaps

- The cascade does not decide VAD silence or end of utterance. Those two stamps stay absent until an ear calls `TurnStamps::note_vad_silence_start` and `note_eou_decided`. This phase does not add a recognizer.
- `PhoneLane::speak_fixed` starts a fresh sheet (persona floor, no ASR). `PhoneLane::speak_kept` continues a sheet `PhoneLane::hear` already wrote, so `t_asr_final` and the heard length stay. An error reply carries that sheet. Stages that did not run stay absent.
- The mock cascade clock is `VirtualHeartbeat`: one frame per recorded stage, so tests do not sleep. The phone lane uses `HeartbeatClock`, wall time quantized down to 80 ms. `Some(0)` means the stage finished inside the first frame. `None` means it did not run.
- Turn gap is `t_tts_first_pcm - t_vad_silence_start` when both are present and ordered. On a dry run it is absent. This phase does not claim a latency number.
- `Heartbeat::step` is unchanged. Publishing a stamp allocates a queue node. That stays on the turn path.

## Risks

- `BusBody::TurnStamps`, `PhoneTurn.stamps`, `TurnReply.stamps`, and `Cascade.last_stamps` are additive fields. A parallel edit of those structs will conflict.
- The process bus compiles only with `galion-console` feature `phone-turn`. The dry-run gate does not need that feature.
- A log line that prints `ReplyText`, `SpeechPcm`, or a `Debug` of `PhoneTurn` would leak words or samples. The stamp log is a separate formatter. The bench prints only that formatter.
- Do not point the voice at an API model. Q1 option A keeps retrieval on the hot path later. This phase does not select a model.

## What landed

| File | Role |
|---|---|
| `crates/galion-turn-stamps` | Sheet, heartbeat clocks, stamp lane, log, bench `horbm-stamp-bench` |
| `crates/galion-cascade/src/lib.rs` | `Cascade::turn` fills the sheet. VAD and end of utterance stay absent. |
| `crates/galion-cascade/src/phone.rs` | Phone lane records the stages it actually runs. |
| `crates/galion-console/src/bus.rs` | `BusBody::TurnStamps` on the existing `try_send` lane |
| `crates/galion-console/src/turn_worker.rs` | Carries the sheet on `TurnReply` |
| `crates/galion-console/src/phone_host.rs` | Publishes the sheet on the process bus |

No new crates.io dependency. No `tokio`. No `target-cpu=native`. No SIMD. No weights. No network in the tests.

## Phase gate

Dry run. Stdout is absent or zero. The same string has no PCM and no words.

```powershell
Set-Location G:\galion\work\galion-one-voice
cargo run --offline --quiet -p galion-turn-stamps --bin horbm-stamp-bench -- --dry-run
```

Stdout:

```text
turn_id=0
t_vad_silence_start=absent
t_eou_decided=absent
t_asr_final=absent
t_llm_req=absent
t_llm_first_token=absent
t_first_clause_flushed=absent
t_tts_first_pcm=absent
pcm_samples=0
text_chars=0
reply_chars=0
audio_samples=0
turn_gap_ms=absent
```

The library test `dry_run_prints_absent_or_zero_and_no_words` checks that string. `mock_turn_stamps_lengths_not_words` runs the mock cascade and checks the log does not contain the transcript.

```powershell
cargo test --offline -p galion-turn-stamps -p galion-cascade --lib
```
