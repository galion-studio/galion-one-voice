# F5 — conductor deep dive

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM R4 phase 5, conductor |
| Written | 2026-11-24. No weight download, no voice audio, no paid API, no merge. |
| Voice tree | `galion-studio/galion-one-voice` `main` `204fd8cd6e399d0e1cced23c595214be94de1f57` |
| Plan | `docs/horbm/research/R4-cpu-realtime-integration.md` §11 on draft PR #207, head `dd902bbcfe34e23f63745a19fc2fed8e468a3b09`, branch `cursor/horbm-r4-cpu-realtime-2180`. The draft is closed and unmerged. |
| Open inputs | PR #250 `cursor/horbm-f1-cascade-stamps-2163` @ `40917538faf0c5d415e5d94e44bec2b8439f4abc`. PR #252 `cursor/horbm-phase4-brain-70b3` @ `6eb04d2980f39b83bcf4d4a0fec2c74e34b3611c`. PR #253 `cursor/f2-retrieval-thread-3cd7` @ `de26ba67db0d0eba83a0e66f1630b71da9e19d72`. None of these are merged. |
| This change | `crates/galion-conductor` and this note. `LOCKS.toml` is untouched. `SOUL.md` is not in this checkout. |

The dive was split four ways, then written up here: floor control already on `main`, the MnE contract and the frame step, the open phase interfaces, and the foundations, locks, and clocks.

## How to read a number

| Tag | Meaning |
|---|---|
| **[D]** | A constant or a behaviour already in a named file on `main`, or on the PR ref named beside it. |
| **[M]** | A measurement already written in this repo. This session did not repeat it. |
| **[T]** | A target in R4 or in domain 03 / domain 04. Not a wall-clock measurement. |
| **[V]** | A result on the virtual frame clock in `crates/galion-conductor`. The counter moves only when a test calls `tick`. |

## 1. What phase 5 asks

R4 §11, phase 5, quoted from the unmerged note:

> Conductor: eager end-of-turn, epoch rollback, barge-in at the lock, wordless reaction from a synthetic clip or silence. MnE intent in, read-back out. Gate: locks hold in the simulator — barge-in ≤ 200 ms on the virtual clock, no cancel on a listed backchannel. Depends on phase 4 and `mne-contract`.

Q1 is still an open question inside that R4 file (§13). The settlement is in the later notes, not in R4. F1 (`docs/horbm/foundations/F1-cascade-audit.md` on PR #250) records it: owner ruling 2026-11-18, option A. HORBM answers first. A later local S0 may fill gaps and must mark them as guesses. Galion Research (the Rust engine, crates `hermes-rs` / `hermes-gateway` in `obsidian-open-mind`) handles hard questions off the tick, with the 3 s cap. This phase does not call Galion Research.

Phase 4 and the phase 1 stamps are open. The conductor does not depend on either branch. It carries a `ClauseBrain` seam and a fake brain, and it uses the phase 1 stamp names on the virtual clock.

## 2. Floor control already on main

Nothing on `main` is the phase 5 conductor. The pieces are real, and they do not share one clock.

| Piece | Where | What it already does |
|---|---|---|
| Ear | `voice-stack/vad/src/turn.rs` | `Event::{SpeechStart, Speculate, Resume, Hold, SpeechEnd}`. Speculate is 60 ms of silence with a transcript **[D]**. `SpeechEnd` is the contract name `commit`. `barge_ms` default 80. Frame is 20 ms. |
| Same machine, energy | `crates/galion-v2-live/src/endpoint.rs` | `EpEv::{Speculate, Resume, Commit, Hold}`. `spec_ms` 60, `commit_ms` 160, `hold_ms` 500 **[D]**. |
| Live duplex | `crates/galion-v2-live/src/bin/call.rs` | Its own energy `Vad`, hang 300 ms. Duck at 80 ms, pause when `voiced_in_ms >= 180` so silence lands by 200 ms, yield and `gen += 1` at 800 ms **[D]** (`call.rs` around the comment "silence at <=200 ms"). Times are `Instant` plus a 20 ms sleep. |
| Orchestrator floor | `voice-stack/pipeline-orchestrator/src/pipeline.rs` | `Floor::barge` bumps `epoch` with `wrapping_add(1)` and calls `stop` before it returns (`pipeline.rs` `fn barge`). The elapsed time is `Instant`. The epoch never moves backward. |
| Mixer | `crates/galion-heartbeat/src/mixer.rs` | `DUCK_MS = 80`, `PAUSE_MS = 200`, `YIELD_MS = 800`, `RESUME_QUIET_MS = 240`, `DUCK_GAIN = 0.35` **[D]**. Silence starts on the frame that contains 200 ms: frames 0 and 1 play, frame 2 (`t = 160`) is silence (`silence_starts_on_the_frame_that_would_pass_200ms`). No wall clock. Nothing outside `mixer.rs` constructs a `DualBandMixer`. The module says backchannel PCM is not a lane. |
| Process bus | `crates/galion-console/src/bus.rs` | `BusBody::BargeIn` raises a cancel token. `turn_id < cancel` is stale. There is no `EndOfTurn` body and no epoch field. The header says nothing here runs inside the frame step. |
| Presence mock | `crates/galion-presence` | Barge-in milliseconds are `frames * frame_ms`. The mock default is one extra frame, so the fixture reports 80 ms against the 200 ms lock **[D]**. |
| Heartbeat | `crates/galion-heartbeat/src/lib.rs:283` | `Heartbeat::step` never blocks, never allocates, and never reads a clock **[D]**. It does not call the mixer, the VAD, the floor, or the bus. |

`EagerEndOfTurn` and `TurnResumed` are not Rust names on `main`. They are the domain 03 words (`docs/research/galion-voice-v2/domains/03-dialogue-bargein.md` §1 and §6). The Symphony plan (`docs/galion-voice-v2/sprint-v2/research/SYMPHONY-STACK-V2-PLAN.md` §2.3 and §4.4) is the reading this conductor follows: stable prefix unchanged for two ticks (160 ms), one frame of hang instead of a 160 ms endpoint **[T]**, and on resume discard the speculative cell and bump the epoch. R4 §4.4 says HORBM's rollback is "discard the cell".

The only backchannel list on `main` is inline in `call.rs` (Samantha `Mm-hm.` / `Yeah.` / `Oh!`, Jarvis `Mm.` / `Quite.` / `Right.`). A short turn is ignored when it has at most two words and the text contains `mm`, `hm`, `yeah`, `right`, `quite`, `oh`, `uh`, `huh`, or `okay` (`call.rs` `only_bc`). Domain 03 names `uh-huh` / `mm` / `yeah` / `okay` / `right` / `da` / `aha` and says backchannels never cancel **[T]**. The bus does not special-case that list. `Floor::barge` does not either.

No filler wav is in the tree. `call.rs` synthesizes those lines at runtime. Person reference clips stay outside git (`clip_store` refuses a store inside the repo). `tools/symphony/nose` can render a wordless gesture into a buffer and does not commit audio. It is not on the call path or the frame step. `galion_live_demo::ToneVoice` is a wordless harmonic, also off this path. R4 §12 allows silence or a generated tone, not a voice.

## 3. MnE, stamps, and the brain seam

`tools/symphony/mne-contract` (`galion-mne-contract`) is the intent and read-back contract **[D]**. Schema `galion-mne-contract`, version 1, `FRAME_MS` 80. One document is either `DocKind::Intent` or `DocKind::Readback`. `validate` rejects a mix, requires the 80 ms grid, and requires a stand-in body to carry at least one field. The crate is its own workspace. It is not a root workspace member. `galion-heartbeat` and `galion-cascade` do not name it. Symphony tools (`galion-mne`, mouth, feeling, listen-back) path-depend on it from their own workspaces.

Phase 1 stamps are not on `main`. On PR #250 the names are `t_vad_silence_start`, `t_eou_decided`, `t_asr_final`, `t_llm_req`, `t_llm_first_token`, `t_first_clause_flushed`, `t_tts_first_pcm`, `t_first_media_frame_sent`, `t_first_audio_heard`, plus `turn_id`, `frame_ms`, `pcm_in_samples`, `pcm_out_samples`, `heard_chars`, `reply_chars`. Times are `Instant` microseconds or `absent`. The log line has no transcript and no PCM. `t_first_audio_heard` stays absent because the probe is outside the process. This conductor keeps those names and fills times from the virtual clock. `t_tts_first_pcm` and `t_first_audio_heard` stay absent: Piper is not in this crate, and the heard-audio probe is not either.

Phase 4 (`horbm-brain` on PR #252) is not on `main`. Its pick returns `Answer { id, clause, first, rest, hit }` with the fixture clause `The kettle is on the hob.` (`first` = `The kettle`, `rest` = `is on the hob.`) and the miss `I don't know.` (`horbm-say-i-dont-know`). `Heuristic::answer` is synchronous. `GALION_LLM=horbm` is wired only on that branch. This crate does not name `horbm-brain`. `ClauseBrain::answer` is the seam an adapter can wrap after #252 merges. `lead` is `Answer.first`. `rest` is `Answer.rest`.

Phase 2 (PR #253) says `RetrievalThread::ask` must not run inside the frame step. `try_cell` is the non-waiting read between frames. This crate does not call that thread. `answer` is documented as a read of a pick that is already in memory.

F3 (`docs/horbm/foundations/F3-compute-audit.md` on `main`) already owns the int8 arena, the scalar dot, and the SIMD features on `galion-tensor`. This phase does not add a dot, a key file, or a `target-cpu` flag.

`LOCKS.toml` `[clock] frame_ms = 80` and `[gates] barge_in_max_ms = 200` **[D]**. Neither file is edited. `SOUL.md` is absent.

## 4. What landed

`crates/galion-conductor` is its own workspace, the same shape as `tools/symphony/feeling`. `galion-mne-contract` is a workspace root inside this repo. Cargo refuses that crate as a path dependency of a root-workspace member ("multiple workspace roots"). A separate workspace is the dependency that builds. The root `Cargo.toml` member list is unchanged, so `cargo test --workspace` does not build this crate and does not pull `serde` into the voice lock. There is no `tokio`, no `unsafe`, no SIMD, and no socket.

The host calls `Conductor::tick` once per virtual frame. That call may allocate. It is not the heartbeat frame function. `src/` does not name `Heartbeat::step`, `Instant`, `SystemTime`, `tokio`, or `TcpStream`. A test reads the sources and fails if those strings appear.

Clock. `VirtualClock` stores a frame index. `t_ms = frame * 80`. `advance` runs at the end of `tick`. A 30 ms sleep in `a_wall_sleep_does_not_move_the_virtual_clock` leaves the next frame at 80 ms **[V]**.

Eager end-of-turn. Two ticks of the same partial is the stable prefix (160 ms) **[T]**. The next quiet frame is the hang (80 ms) **[T]**. A quiet frame also counts toward the two unchanged ticks, so a one-frame utterance still ends on the following quiet frame. `tick` calls `ClauseBrain::answer` once, stores the pick against the current epoch, and hands out the wordless reaction. The clause stays behind the gate (`clause` is `None`, `speculative` is true).

Commit. The second quiet frame releases the clause. `FakeBrain` returns `The kettle is on the hob.` with a two-word lead when the partial contains `kettle`, and `I don't know.` otherwise. An empty query returns an empty clause and does not speak the miss. This matches the phase 4 fixture without linking that crate.

Epoch rollback. Speech while the floor is `Speculating` drops the cell, clears the speculative stamps, bumps the epoch, and does not set `cancel`. The brain is not called again for that discarded pick.

Barge-in at the lock. While `Speaking` or `Ducked`, speech that is not on the list adds 80 ms of overlap. The pause predicate is the mixer's: `overlap_ms + FRAME_MS > 200` arms silence on the next frame **[D]**. From the first overlap frame, silence starts 160 ms later, which is the frame that contains 200 ms. 160 ≤ 200 **[V]**. That frame sets `cancel`, bumps the epoch, and drops the clause. A blip of one frame ducks and does not arm silence, same as the mixer.

Listed backchannel. The list is the union of domain 03 and the `call.rs` `only_bc` tokens, plus `mm-hm` / `mhm`. At most two words, every word on the list. `yeah` for six frames (the span from the commit frame to the last of those frames is 480 ms, past the lock) leaves `cancel` false, the epoch unchanged, and the clause in place **[V]**. `yeah stop` is not on the list.

Wordless reaction. Default is 1920 zeros, one 80 ms frame at 24 kHz, generated in memory. `ReactionKind::Synthetic` is a 220 Hz sine, amplitude 1000, same length. Neither path reads a file. Neither path is a recording of a person.

MnE. `tick` takes an optional intent document. `read_back` checks `DocKind::Intent`, runs `validate`, and returns a read-back on the same `t_ms` grid. The body is a stand-in: rest breath, loudness 0 for silence or −12 for the sine, nose `nasalance` 0.35. No formant. Empty text. A read-back handed back in as if it were an intent is rejected.

Stamps filled on the virtual clock, first write wins:

| Name | When |
|---|---|
| `t_vad_silence_start` | Start of the first quiet frame after speech. |
| `t_eou_decided` | End of the hang frame. 80 ms after silence start **[V]**. |
| `t_asr_final`, `t_llm_req`, `t_llm_first_token` | Same virtual instant as the eager pick. The fake brain returns in that call. |
| `t_first_media_frame_sent` | The reaction buffer is handed out. `pcm_out_samples` is 1920. |
| `t_first_clause_flushed` | The confirm frame. `reply_chars` is the character count, not the words. |
| `t_tts_first_pcm`, `t_first_audio_heard` | Absent. |

`pcm_in_samples` stays 0. The ear is a partial string, not a recording. `stamp_log_is_lengths_only` rejects a line that still contains a word after the fixed keys are removed. The kettle clause and the miss line are absent from the log **[V]**.

`RUNS_INSIDE_HEARTBEAT_STEP` is `false`.

## 5. Phase gate

Host: Linux, rustc 1.83.0, this crate's `rust-version` is 1.83 so a 1.85 laptop compiler still accepts it. `cargo clippy --manifest-path crates/galion-conductor/Cargo.toml --all-targets -- -D warnings` passed. `cargo check --manifest-path crates/galion-conductor/Cargo.toml --target x86_64-pc-windows-msvc --tests` finished on the same host (typecheck only; the tests were not executed under Windows). No `RUSTFLAGS`. No `target-cpu`.

```text
cargo test --manifest-path crates/galion-conductor/Cargo.toml -- --nocapture
```

14 tests passed (6 in the library, 8 in `tests/phase_gate.rs`). The two lock lines:

```text
phase5 gate: barge_in_ms=160 limit_ms=200 clock=virtual frame_ms=80 result=pass
phase5 gate: backchannel=yeah frames=6 frame_ms=80 span_ms=480 cancel=false epoch_unchanged=true result=pass
```

`barge_in_ms=160` is the mixer schedule: silence on the frame that contains 200 ms. The assertion is `barge_ms <= 200` and `barge_ms == 160`. The 160 is a frame count times 80, not a wall-clock sample. The backchannel line is six virtual frames of `yeah` with `cancel=false` and the same epoch.

## 6. Laptop test

From `G:\galion\work\galion-one-voice`, after this branch is checked out. These commands do not download a voice, do not open a microphone, and do not merge. Rust 1.83 or newer. The first build fetches the pinned `serde` / `toml` crates for `galion-mne-contract` if they are not already in the cargo registry. The tests themselves do not open a socket.

```powershell
Set-Location G:\galion\work\galion-one-voice
cargo test --manifest-path crates\galion-conductor\Cargo.toml -- --nocapture
if ($LASTEXITCODE -ne 0) { throw "conductor gate failed" }
cargo clippy --manifest-path crates\galion-conductor\Cargo.toml --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { throw "conductor clippy failed" }
```

Pass is the two `phase5 gate:` lines above, with `barge_in_ms` at most 200 and `cancel=false` on the backchannel line. Do not set `RUSTFLAGS` to `-C target-cpu=native`.

## 7. Left open

- PR #252 is still the real clause pick. `FakeBrain` is the stand-in. An adapter can implement `ClauseBrain` around `Heuristic::answer` after that PR merges. This branch does not stack on it.
- PR #250 is still the cascade stamp sheet on `Instant`. The names match. The clocks do not. A later host can copy these virtual microseconds into that sheet. This branch does not edit `galion-cascade` (that file is already touched by both open PRs).
- PR #253's retrieval thread is not called. A slow brain must not do the work inside `answer`.
- Piper is unchanged. `t_tts_first_pcm` stays absent. The reaction is not speech audio.
- The mixer's 800 ms yield and 240 ms resume are named (`MIXER_YIELD_MS`, `MIXER_RESUME_QUIET_MS`) and are not the cancel path. Cancel for a real interruption is the 200 ms lock. A listed backchannel never enters that path.
- Galion Research is not on this path. Phase 6 is the side band.
- Do not merge this pull request. The laptop test comes first.
