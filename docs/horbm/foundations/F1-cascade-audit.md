# F1 — Voice cascade audit, and domain 04 stamps

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM foundations, agent F1 of 4 (timestamps / cascade) |
| Written | 2026-11-24. No model download, no paid API, no voice audio committed. |
| Repo | `galion-studio/galion-one-voice`, read on `main` at `4759d41` before this branch. |
| Research read | `docs/horbm/research/R1`–`R4` on the unmerged branches `cursor/horbm-r1-retrieval-core-690c`, `cursor/horbm-r2-ranking-eureka-906f`, `cursor/horbm-r3-memory-persona-3e32`, `cursor/horbm-r4-cpu-realtime-2180`. R4 is draft PR #207, closed unmerged. |
| This change | Phase 1 of R4 §11: domain 04 stamps on the cascade that already runs. `LOCKS.toml` and `SOUL.md` were not edited. `SOUL.md` is not in this checkout. |

## How to read a number

Same tags as R4. A tag without a source is not a result.

| Tag | Meaning |
|---|---|
| **[D]** | A design constant already in a named file. |
| **[M]** | A measurement already written in this repo. This session did not repeat it. |
| **[C]** | A published source, as R4 and domain 04 already cite it. |
| **[T]** | A target. Not measured here. |

## Q1 is closed as policy

Owner ruling 2026-11-18, option A: HORBM answers first on the hot path. A small local S0 may later fill gaps, and those fills are marked as guesses. Galion Research (the crates `hermes-rs` / `hermes-gateway` in `galion-studio/obsidian-open-mind`) handles deep questions, with the 3 s cap. That answers R4 open question Q1.

This phase does not put that brain on the hot path. The cascade that runs today is still the one audited below. S0 in this tree cannot speak (`INFERENCE_IMPLEMENTED = false` in `crates/galion-cascade/src/student.rs`).

## What phase 1 added

R4 §11 phase 1: stamps only, the domain 04 list from `t_vad_silence_start` through `t_tts_first_pcm`, lengths not transcripts. Gate: a dry run prints zeros or absent, and the log has no PCM and no words.

The list in `docs/research/galion-voice-v2/domains/04-latency-streaming.md` §7 is:

`t_vad_silence_start`, `t_eou_decided`, `t_asr_final`, `t_llm_req`, `t_llm_first_token`, `t_first_clause_flushed`, `t_tts_first_pcm`, `t_first_media_frame_sent`, `t_first_audio_heard` (external probe).

`crates/galion-cascade/src/stamps.rs` records all nine. Times are microseconds from a `StampClock` (`std::time::Instant`), or `absent` when the stage did not run. A zero is a real sample of that clock, not a missing stage. Lengths are `pcm_in_samples`, `pcm_out_samples`, `heard_chars`, `reply_chars`. `frame_ms` is `galion_heartbeat::FRAME_MS`, which the heartbeat crate asserts is 80 **[D]**.

`Heartbeat::step` is not called. The stamp line is not written from inside the tick.

Dry run (no microphone, no model, no socket):

```
domain04 turn_id=0 frame_ms=80 t_vad_silence_start=absent t_eou_decided=absent t_asr_final=absent t_llm_req=absent t_llm_first_token=absent t_first_clause_flushed=absent t_tts_first_pcm=absent t_first_media_frame_sent=absent t_first_audio_heard=absent pcm_in_samples=0 pcm_out_samples=0 heard_chars=0 reply_chars=0
```

`cargo run -p galion-cascade --example domain04_dry_run` prints that line. `stamp_log_clean` accepts only the fixed keys and values that are `absent` or an unsigned integer. `RIFF`, a transcript word, or a second line fails the test.

On a real `PhoneLane` turn the sheet is filled only for stages the lane actually runs:

| Stamp | What this cascade does |
|---|---|
| `t_vad_silence_start` | Stays absent unless a caller calls `PhoneLane::note_vad_silence_start`. The phone lane never sees the VAD hang. |
| `t_eou_decided` | Set when `turn` or `hear` accepts an already-closed PCM buffer. That is this cascade's end-of-utterance, not the 160 ms hang. |
| `t_asr_final` | Set when `PhoneTurnAsr::transcribe` returns text. Character count is stored. The text is not. |
| `t_llm_req` / `t_llm_first_token` | Set around `Llm::reply_deltas`. `CannedLlm` delivers the whole line in one delta, so the two times sit close together. |
| `t_first_clause_flushed` | Set when the first clause is handed to `SpeechStack::speak`. |
| `t_tts_first_pcm` | Set when the first PCM buffer comes back. Sample count is stored. The samples are not. |
| `t_first_media_frame_sent` | Stays absent. The bus publishes `SpeechPcm` after the turn returns. |
| `t_first_audio_heard` | Stays absent. Domain 04 says an external probe owns it. |

A finished `PhoneTurn` carries the sheet. The turn worker (`crates/galion-console/src/turn_worker.rs`, feature `phone-turn`) appends `log_line()` to `stages` when a `PhoneTurn` is produced. An error reply (no speech backend, ASR refused) does not yet copy the partial sheet onto `stages`. The sheet is still on `PhoneLane::stamps()`.

## The path that actually runs

There is no single process that is "the cascade". Three stacks sit side by side. Phase 1 stamps the one the phone screen already calls.

1. **Phone lane (stamped).** `galion-console` `TurnWorker` (off the 80 ms thread) calls `galion-cascade` `PhoneLane`: 16 kHz PCM in, ASR, reply, 24 kHz WAV out. Feature `cascade` on `galion-cascade`, feature `phone-turn` on the console. The bus is `ProcessBus` in `crates/galion-console/src/bus.rs`. `try_send` only. `Heartbeat::step` does not send on it.
2. **Live call (not stamped).** `crates/galion-v2-live` is its own Cargo workspace, not a member of the root workspace. `src/vad.rs` and `src/endpoint.rs` decide end of turn. `src/bin/call.rs` drives them. `cargo test --workspace` does not build this crate.
3. **Samantha voice-stack (not stamped).** `voice-stack/` is its own workspace: `vad`, `tts-piper-stream`, `horbm-bridge`, `pipeline-orchestrator`, `research-gateway`. The bridge links crate `horbm` (`horbm/`, also its own workspace, and `exclude`d from the root `Cargo.toml`).

R4's picture (VAD on the 80 ms frame, streaming ASR, retrieval thread, warm Piper, Galion Research off the tick) is the target. The code is the three stacks above.

## Piece by piece

### `galion-heartbeat` — works

Path: `crates/galion-heartbeat`. `FRAME_MS = 80` **[D]**, `SAMPLE_RATE = 24_000` **[D]**, `FRAME_SAMPLES = 1_920` **[D]**, `FRAME_HZ_TENTHS = 125` (12.5 Hz) **[D]**. `symphony::STEP_BUDGET_US = 800` **[D]** (1% of the 80 ms tick). Mixer `YIELD_MS = 800` **[D]** is a different 800: overlap before the floor yields, not the step budget.

`Heartbeat::step` is documented and tested as never blocking and never allocating. Tests: `tests/heartbeat.rs`, `tests/zero_alloc.rs` (counting allocator, Rika on), `tests/frame_budget.rs` (asserts the 800 µs ceiling), `tests/presence_p6.rs`, `tests/symphony_wire.rs` (median warm p99 and max against `STEP_BUDGET_US`). `docs/galion-voice-v2/symphony/SELF-EVAL.md` records several host passes, including one release p99 of 3206 ns **[M]** and later debug passes around 100 µs **[M]**. Those passes are not one number. This session did not re-time the tick.

Status: **works** as the frame clock. It does not run ASR, a reply, or TTS. Stamps do not belong inside `step`.

Gap: the bus clock (`BusMsg.t_ms`, "heartbeat time, frame × 80") and the stamp clock (monotonic microseconds) are not the same clock. Nothing converts one into the other.

### Process bus — works, carries PCM and words

Path: `crates/galion-console/src/bus.rs`, compiled only with feature `phone-turn`.

Depths **[D]**: mic 1, utterance 8, reply 8, partial 8, fault 8, job 1, worker reply 4. Utterance cap `4 * 1024 * 1024` bytes **[D]**. Speech rate 24_000 **[D]**. Empathy switch: on 0.65, off 0.50, 8 frames (8 × 80 ms = 640 ms) **[D]**.

`BusBody::Utterance` and `SpeechPcm` hold PCM bytes. `PartialAsr` and `ReplyText` hold strings. Cancel drops `turn_id < cancel` before ASR and again before publish. Sends are `try_send`. The worker thread is not killed.

Tests live in the console crate under the `phone-turn` feature (`turn_worker` tests drive the bus). Default `cargo test -p galion-console` does not compile this module.

Status: **works** for the phone screen. It is not a domain 04 log. The stamp line is appended to the worker's `stages` string on success. The bus message itself was not given a new variant.

Gap: a log of the bus bodies would contain PCM and words. The stamp line does not copy those bodies. Callers that debug-print a `BusMsg` still would.

### `galion-cascade` — works as a fallback lane, not as HORBM

Path: `crates/galion-cascade`. Always compiled: trait stubs (`MockAsr`, `MockLlm`, `MockTts`), `CannedLlm`, the Galion Research loopback client (`src/hermes.rs`, `src/hermes_stream.rs`), `student`, `offline`, `reflect`, `research_handoff`, `route`, and now `stamps`. Feature `cascade` adds `en_lane` and `phone` (deps `galion-ml-voice`, `galion-asr-vosk`).

Constants on the stub lane **[D]**: `ONSET_P50_BUDGET_MS = 800`, `BARGE_IN_BUDGET_MS = 200`, `SAMPLE_RATE_HZ = 16_000`. The barge number matches `LOCKS.toml` `barge_in_max_ms = 200` **[D]**. The onset number does not match `onset_p50_max_ms = 300` **[D]**. The file comment says "Budgets from A18 design." The lock is the stricter number. This note does not edit the lock or the constant.

`PhoneLane` (feature `cascade`) is the runnable turn: 16 kHz in **[D]**, 24 kHz WAV out **[D]**, `SILENCE_RMS = 0.003` **[D]**. ASR modes: stub (default), vosk, off. The env value `interim` is forced back to stub. `AsrMode::Interim` exists for tests and invents one of three length-bucket sentences. Those words are not recognition.

LLM modes: canned, echo, Galion Research loopback (default when `GALION_LLM` is unset), student. Unknown values error. The lane does not fall back from a dead research gateway to the rule table.

`StageMs` is still there: durations in milliseconds for ASR, reply, TTS, first PCM, and first audio. The new sheet is absolute monotonic times. Both are kept. `StageMs` was not replaced.

Tests: the default lib tests (95, including the five stamp tests) do not need a speech backend. With `--features cascade`, phone tests cover stub decode, silence, the interim label, a missing Vosk model (no invented transcript), and, when a backend exists, a full 24 kHz turn whose stamp line has lengths and no words. This host had a backend; the full-turn test passed. CI may not. The speech-off stamp test does not need one.

Status: **works** as the fallback lane. **Stub** as a brain: see CannedLlm and S0 below.

Gaps: VAD silence is absent. Media send and heard-audio are absent. Error replies on the worker do not carry the partial sheet. `ONSET_P50_BUDGET_MS` disagrees with the lock.

### `CannedLlm` — works, and it is a rule table

Path: `crates/galion-cascade/src/canned.rs`. Always compiled. An intent table maps phrases to short English lines. Unknown input gets an honest "say that again" style line. It does not retrieve cards and it does not decode tokens. `ReplyLlm::from_choice(None)` is still the echo mock; `GALION_LLM=canned` selects the table. Phone turns default to the Galion Research client, not this table (`PhoneConfig::from_env`).

Tests in the same file check the table, word-boundary matching, and the echo default.

Status: **works** as a stub. R4 phase 4 is the phase that replaces a chat-turn answer with a heuristic pick. Not this phase.

### S0 student — stub

Path: `crates/galion-cascade/src/student.rs`. Reads a `.gq` through `galion-tensor` if `GALION_STUDENT_GQ` or `weights/student/s0.gq` is present. `INFERENCE_IMPLEMENTED = false` **[D]**. The spoken line is "Student brain unavailable." It does not echo the caller. Tests cover a missing file, a bad file, and an opened file that still cannot speak.

Status: **stub**. Option A says a later S0 may fill gaps and must mark those fills as guesses. Nothing here does that yet.

### Galion Research client inside the cascade — works off the tick

Path: `crates/galion-cascade/src/hermes.rs` and `hermes_stream.rs`. Loopback HTTP, std `TcpStream`, no TLS. Default timeout 15_000 ms **[D]** (`voice_timeout_default_is_fifteen_seconds_and_clamped`). That is the phone completion budget, not the 3 s side-band cap. The 3 s cap is `galion-sideband::RETRIEVAL_CAP` (3_000 ms) **[D]** and `galion_research_gateway::RESEARCH_CAP` (3 seconds) **[D]** in `voice-stack/research-gateway`.

The client refuses a list of scripted comfort lines, strips a leading emotion tag, and can stream the first sentence to TTS before the rest arrives (`early_reply.rs`, first-sentence word cap 16 **[D]**). It is not called from `Heartbeat::step`.

Status: **works** as a loopback client when a gateway is up. A dead gateway speaks a failure line. It is not the hot-path brain option A asked for. Deep questions belong on the side band, after HORBM misses, under the 3 s cap. The phone lane still calls this client for an ordinary turn when `GALION_LLM` is unset.

### `galion-asr-vosk` — works out of process, model not in git

Path: `crates/galion-asr-vosk`. The library speaks NDJSON to the `galion-asr-vosk` binary. The binary dlopens libvosk. This crate does not link it and does not download a model. Protocol **[D]**: 16_000 Hz, 320 samples per 20 ms frame, max utterance 60 seconds of samples. Default model dir `weights/asr/vosk-model-small-en-us-0.15` **[D]**, not vendored. The worker can emit partials and a final. The phone lane's `transcribe` waits for the final text of one buffer. It does not feed partials into the reply while the person is still speaking.

Tests: `src/lib.rs` checks the live screen script does not default to `interim`. `tests/protocol.rs` covers the NDJSON codec. A missing model returns `AsrUnavailable`. The transcript is not invented.

The request line carries PCM (base64). That is the worker protocol, not the stamp log. The stamp log stores `pcm_in_samples` and `heard_chars`.

Status: **works** when the model and library are on disk. **Missing** as a streaming partial source for domain 04: there is no `t_asr_partial` in the domain 04 list, and this lane does not stamp partials.

### VAD — two implementations, neither stamps the cascade

**voice-stack** `voice-stack/vad` (`galion-sam-vad`). 20 ms frames **[D]**, 16_000 Hz **[D]**, 320 samples **[D]**, 4 frames per 80 ms heartbeat **[D]**. `TurnCfg::default` **[D]**: onset 60 ms, hangover 500 ms, speculate 60 ms, commit 160 ms, hold 500 ms, start threshold 0.5, keep 0.35, barge 80 ms. Silero is used when the ONNX file loads. Otherwise the energy gate from `galion-v2-live` `vad.rs` is the score. `vad-ear` (`src/bin/vad_ear.rs`) is a stdin/stdout process. A missing model still answers `backend=energy`. The Windows `Send` impl for `Peek` is in `voice-stack/tts-piper-stream/src/session.rs` (the Piper drain, not the VAD). Both the `Peek` impl and the `vad-ear` bin are on this tree. Commit `4409e6b` is not an ancestor of `4759d41`; the code those notes describe is present.

**galion-v2-live** `src/vad.rs`: energy VAD, `min_voiced_ms = 60` **[D]**, hang passed in by the caller. `call.rs` constructs `Vad::new(300.0)` and then sets `min_voiced_ms = 40` **[D]**. `endpoint.rs` **[D]**: speculate 60 ms, commit 160 ms, hold 500 ms. Speculate may start a reply; commit is what may play. That is the domain 04 partial-commit shape, and it is not wired to `PhoneLane` or to `Domain04Stamps`.

Status: **works** inside each crate (energy always; Silero only with a model file that is not in git). **Missing** as the ear of the stamped cascade. `t_vad_silence_start` stays absent on every phone turn this phase runs.

### Piper and sound-out — three mouths

1. **Phone mouth.** `galion-ml-voice` `SpeechStack` (`src/speech/mod.rs`). `detect()` never downloads. Order depends on `GALION_SPEECH` and what is on disk: clone worker, A21 Piper (`en_US-ljspeech-medium`, out of process, one warm process), libflite, or none. `PhoneLane::new` calls `prewarm()`. `Warmth` stores cold/warm/last first-PCM milliseconds and the Piper pid and spawn count. No PCM is stored on `Warmth`. `LOCKS.toml` `[formats.exceptions.a21_piper]` **[D]**: out of process, weights not vendored, sunset 2027-02-01, sha256 pinned. This session did not re-measure TTFA.
2. **voice-stack mouth.** `voice-stack/tts-piper-stream`. Streaming Piper, Windows reader thread, `unsafe impl Send for Peek` behind `cfg(windows)`. Its own workspace.
3. **Symphony mouth.** `tools/symphony/sound-out`. `PiperBackend::real_if_present` does not download. `moshi.rs` is a stub: `realtime_on_cpu: false` **[D]**, does not look for weights. CI runs `cargo test --manifest-path tools/symphony/sound-out/Cargo.toml`.

Prior loop measurement, not re-run, from `docs/galion-voice-v2/sprint-v2/SPRINT-REPORT.md` as R4 quotes it **[M]**: first content audio p50 608 ms, p95 775 ms, on a loop that used a cloud model. The 2026-11-04 ruling removes that model. Warm Piper alone in that report was inside one frame for a short clause. This session did not repeat either number. A green `full_turn` test on this host is not a latency result.

Status: Piper path **works** when the pack or flite is present, **missing** when neither is (the lane returns `NoSpeech` and does not invent a tone as speech; the console worker then publishes its own tone and an error label). Moshi **stub**. Clone lane is not the content path (R4 bottleneck 4; lock sunset 2027-02-01 **[D]**).

### `voice-stack/horbm-bridge` — linked, and a miss says it does not know

Path: `voice-stack/horbm-bridge`, package `galion-sam-horbm`. Not a root workspace member.

The brief's phrase "brain not landed" is not in this tree. What the code does:

- Crate `horbm` (`horbm/crates/horbm`) is a path dependency. `horbm_linked()` returns true. `Brain::turn` order is empathy, then cards, then an optional research ask, then exactly `UNKNOWN_REPLY` = `I don't know.` **[D]** (`horbm/crates/horbm/src/route.rs`). No gap filler sentence.
- `CardEngine` / `CardBrain` read a knowledge directory when it already exists. They do not create it. Memory and case paths are not opened.
- `FakeEngine` echoes. Tests and `galion-sam-horbm tick` use it.
- `HorbmEngine` (feature `horbm`) adapts `horbm::Brain` to `ReplyEngine`.
- `ReasoningRouter` sends a deep question to `galion-research-gateway` and keeps the 3 s cap. A shallow miss stays `I don't know.`
- `voice-stack/pipeline-orchestrator` (`galion-sam-orch`) `BrainOpen::open` still returns the error `horbm is not linked; Brain::open stays in crate horbm` (`src/samantha_traits.rs`). That function is not the path `HorbmEngine::open` uses. The comment in the bridge says so.

Tests: `voice-stack/horbm-bridge/tests/bridge.rs`, `tests/router_gateway.rs`, and the `horbm` crate's `tests/turn.rs`, `tests/cards.rs`, `tests/pack_bench.rs`. They are not part of root `cargo test --workspace`.

Status: **works** as a card-or-honest-miss reply engine when that workspace is built and a card directory exists. **Stub** relative to option A on the phone hot path: `PhoneLane` does not call it. The orchestrator's `Brain::open` is still the not-linked error.

### `galion-sideband` — works as types and a non-blocking client

Path: `crates/galion-sideband`. `RETRIEVAL_CAP = 3_000 ms` **[D]**. `ActionTier::Money` is refused by `voice_may_request`. `SidebandClient::submit` and `poll` are `try_send` / `try_recv`. Past the cap, `poll` returns `NoKnowledge::Timeout` and drops a late reply. `NullTransport` answers `Unreachable`. `Turn` still holds `text: String`. Domain 07, as R4 quotes it, asks logs to store lengths and latencies, not the transcript. This type is the side-band contract, not the stamp log.

Tests: `tests/types.rs`, `tests/client.rs`, `tests/lang.rs`.

Status: **works**. It is not on the 80 ms tick. Phase 6 of R4 is the phase that posts `Lookup` through it after a local clause. Not this phase.

### `LOCKS.toml` — locked, not edited

Read, not changed. The deadlines that bind a later cascade **[D]**, locked 2026-11-02, Maciej Grajczyk:

| Lock | Value |
|---|---|
| `clock.frame_ms` | 80, not configurable |
| `clock.frame_hz` | 12.5 |
| `clock.frame_budget_p95_ms` | 60 |
| `bands.duplex` | full |
| `gates.barge_in_max_ms` | 200 |
| `gates.onset_p50_max_ms` | 300 |
| `gates.affect_to_voice_max_ms` | 160 |
| `probe.realtime_headroom` | 2.0 |
| `consent.default` | deny |
| `formats.ggml_container_allowed` | false |
| `licence.paid_tiers` | false |

Piper exception: CPU, separate process, sunset 2027-02-01 **[D]**. Chatterbox exception: out of process, sunset 2027-02-01 **[D]**.

`scripts/check-locks.sh` is the CI check. This branch does not touch the file.

## Top gaps

1. **The hot path is not HORBM.** Option A is policy. The phone lane still answers with the rule table, the echo mock, the Galion Research loopback client, or the blocked S0 line. Crate `horbm` can answer from cards or say it does not know, and the phone lane does not call it. `galion-sam-orch` `Brain::open` still reports that the brain is not linked.
2. **The ear is not on the stamp clock.** `t_vad_silence_start` is absent on every cascade turn. `voice-stack/vad` and `galion-v2-live`'s endpointer already have silence, speculate, and commit, on different hang numbers (500 ms default vs 300 ms in `call.rs` vs 160 ms commit). None of them call `note_vad_silence_start`. `t_first_media_frame_sent` and `t_first_audio_heard` stay absent.
3. **PCM and words still move beside the stamp line.** The bus bodies and the Vosk request carry PCM. Partial ASR, reply text, and `galion-sideband::Turn` carry words. The stamp line does not. A worker error's `stages` string does not yet include the partial sheet. A debug print of a `BusMsg` is not the stamp log.
4. **The measured turn gap is the old loop.** Sprint-report first content audio p50 608 ms **[M]** used a cloud model the 2026-11-04 ruling removed. This session did not measure a replacement. R4's 200 ms p50 target **[T]** is still a target. Three mouths and three workspaces mean a number from one of them is not a number for the others.
5. **The onset constant and the lock disagree, and S0 cannot speak.** Cascade `ONSET_P50_BUDGET_MS` is 800 **[D]**. The lock is 300 **[D]**. `INFERENCE_IMPLEMENTED` is false **[D]**. A later S0 fill has to be marked as a guess; there is no guess tag on a reply yet.

## What this session did not do

No change to `LOCKS.toml`. No edit to a missing `SOUL.md`. No model download. No voice file. No paid call. No claim that a turn gap was measured. Phases 2–9 of R4 §11 were not started. F2 (retrieval), F3 (int8 arena), and F4 (brain pick) are other agents.

## Tests run here

| Command | Result |
|---|---|
| `cargo test -p galion-cascade --lib` | 95 passed |
| `cargo test -p galion-cascade --features cascade --lib` | 126 passed (a speech backend was present on this host) |
| `cargo run -p galion-cascade --example domain04_dry_run` | one line, times `absent`, lengths `0` |
| `cargo test -p galion-console --features phone-turn --lib turn_worker` | 10 passed |

`cargo test --workspace` is the CI job in `.github/workflows/gates.yml`. It uses default features, so it runs the stamp tests and does not compile `phone.rs` or `turn_worker.rs`. The `--features cascade` and `--features phone-turn` commands are the ones that compile the glue.
