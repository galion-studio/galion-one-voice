# Eureka

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb · 2026-11-03.

The spelling is **Galion**. The product is Galion Voice.

This is the symphony standard for the clock. It cites `LOCKS.toml`. It does
not edit it. A value in the lock file wins if a sentence here ever drifts.

Master slogan: "The seamless connection with the human being, on the voice only."

## Champion

**A conversation is one heartbeat, not a pipeline.**

One 80 ms clock. Three bands: you · me · feeling. Cheap CPU. The heartbeat
tests itself.

`Heartbeat::step` in [`crates/galion-heartbeat`](../../../crates/galion-heartbeat)
is that clock. One call per frame carries the you band, the me band, and the
feeling band. Rika runs inside the call, on the feeling band. The turn worker,
the clone worker, and Piper stay off that call.

The feeling decision made in frame *t* is audible in frame *t+1*
([`FrameOut::applied`](../../../crates/galion-heartbeat/src/lib.rs)). With
Rika's two-frame hysteresis that is evidence → armed → audible = 160 ms.
That delay is the frame. It is one step, applied on the next tick.

## Where the proof lives

| | Path |
|---|---|
| Heartbeat crate | [`crates/galion-heartbeat`](../../../crates/galion-heartbeat) |
| The one call | [`Heartbeat::step`](../../../crates/galion-heartbeat/src/lib.rs) |
| Harness replay | [`presence::RikaReplayBackend`](../../../crates/galion-heartbeat/src/presence.rs), bin [`galion-heartbeat-presence`](../../../crates/galion-heartbeat/src/bin/presence.rs) |
| Frame self-eval | [`tests/heartbeat.rs`](../../../crates/galion-heartbeat/tests/heartbeat.rs), [`tests/zero_alloc.rs`](../../../crates/galion-heartbeat/tests/zero_alloc.rs), [`tests/presence_p6.rs`](../../../crates/galion-heartbeat/tests/presence_p6.rs) |
| Spec the crate replays | [`specs/presence.json`](../../../specs/presence.json) (P1–P6), [`specs/presence-wav.json`](../../../specs/presence-wav.json) (P7–P8) |
| R1–R9 teacher | [`galion-emotion` `rika.rs`](../../../galion-emotion/src/rika.rs) via [`galion-gates/g6-rika.sh`](../../../galion-gates/g6-rika.sh) |
| Presence replay gate | [`galion-gates/ps-presence.sh`](../../../galion-gates/ps-presence.sh) |
| Phone loopback self-eval | [`docs/eval/PRESENCE-EVAL.md`](../../eval/PRESENCE-EVAL.md) |
| Symphony ledger | [`SELF-EVAL.md`](SELF-EVAL.md) |

Demo = Spec = Test = Teacher. For this clock the four are the same artifact:
`specs/presence.json` is the spec, `galion-heartbeat-presence` replays it,
`presence_p6.rs` and the PS gate test it, and the next run is taught by that
same file. The canned Python demo under `galion-demo/` does not call
`Heartbeat::step`. G6 on the Rust stub path stays the teacher.

## Standards

1. **Demo = Spec = Test = Teacher.** One artifact proves presence. A demo
   without the matching spec and test is unfinished.
2. **Harness before model.** Gates and specs come before a weight download.
   `galion-heartbeat` ships with no weights. The `presence` feature is the
   harness (`Cargo.toml`: runtime builds use `default-features = false`).
   Rika is off in [`HeartbeatConfig::default`](../../../crates/galion-heartbeat/src/lib.rs).
   G6 runs the RMS/ZCR stub. PS runs the mock backend. The student is a later
   backend.
3. **The heartbeat tests itself.** P1–P8, R1–R9, the presence replay, and the
   zero-allocation check are the inner loop. A nightly ledger is what
   "deployed" means. Logged scores are the record. There is no separate claim
   of awareness.
4. **Hot path.** `Heartbeat::step` does not block and does not allocate.
   No locks, channels, sleeps, clock reads, or file or network I/O. State is
   fixed-size inside the struct. `tests/zero_alloc.rs` counts allocations on
   a full R1 moment with Rika on.
5. **Late = pause.** A frame the scheduler flags late, or whose you-band PCM
   is not exactly `FRAME_SAMPLES` (1920), is delivered with `paused = true`.
   The me band starts nothing new. The next frame does not catch up. Rika is
   told the frame was late, so hysteresis does not advance on a scheduler
   artefact (R9, `late_frame_is_a_pause_no_extra_trigger_no_burst`).
6. **80 ms frame.** `FRAME_MS = 80` is the scheduling quantum. `LOCKS.toml`
   `[clock]` has `frame_ms = 80`, 12.5 Hz, 24 kHz, 1920 samples, and
   `frame_budget_p95_ms = 60` inside the frame. `[bands]` names `you`, `me`,
   `feeling`. The test `frame_constants_match_locks` reads the lock file and
   checks those numbers. This page does not change them.
7. **Cut order, once per section.** Question the requirement. Delete what the
   heartbeat does not need. Simplify what remains. Accelerate the hot path.
   Automate the gate. A second pass is a new section.
8. **Three self-eval loops.** Inner, per turn: confidence and groundedness,
   logged. Middle, per session. Outer, per release, against a human-calibrated
   anchor. The model does not grade itself alone. When support is missing,
   "I don't know" is a success. The frame harness and the loopback virtual
   user exist today. The outer human anchor and the VPS gates R0–R3 are still
   open (`docs/galion-voice-v2/GALION-VOICE-V2.md`, V2-S10).

`specs/presence.json` rejects engagement, retention, session length, dwell,
and speech elicited. A self-eval row uses the same ban.

## Three bands

`LOCKS.toml` `[bands]`: names `you`, `me`, `feeling`. Count 3. Full duplex.
Rika does not read the transcript.

| Band | In `step` | On a late frame |
|---|---|---|
| you | `FrameIn::you_pcm`, mono, 24 kHz. Length must be 1920 samples. | Short PCM is the same pause as `late = true`. |
| me | `me_speaking` in. `me_onset_allowed`, `me_gain` (never above 1), prosody out. | No new onset. Nothing starts on the next frame to make up the gap. |
| feeling | Rika, inside the call, after the consent gate. Decision in `decided`, audible in `applied` next frame. | `late` is passed in. Hysteresis does not move. |

Consent stays the lock: `sense` and `adapt` default deny. Workplace and
education stay prohibited. Without `sense`, the feeling band does not read PCM.

## Symphony map

Each row is the cut and the gate. A row is done only when that gate passes
and the result is written in [`SELF-EVAL.md`](SELF-EVAL.md). An open gate is
blocked.

| Section | Cut | Gate |
|---|---|---|
| A Compiler | The symphony runs a Galion path. `LOCKS.toml` `[formats]` still lists `safetensors`, with `.gq` later and reader-only until a student exists. The Chatterbox pin stays the out-of-process sunset exception already written there (`2027-02-01`). This page leaves the lock as it is. | The speaking crate compiles. A round-trip fixture exists. A raw Chatterbox file is not the primary runtime path. |
| B Clone landing | A voice lands only from refs the rights record has cleared. Today both slots are text placeholders (`personas/refs/`). [`RIGHTS-CLEARANCE.md`](../RIGHTS-CLEARANCE.md) leaves Samantha and Jarvis closed. No clip sha256 is recorded, because there is no clip. MP3s are not refs. | Worker PCM, and a sha256 that matches a clip `LEDGER.md` has cleared. Until that line exists, this row stays open. |
| C Jarvis texture | When a cleared clip exists, hub clicks, alarms, and ambience stay. Texture is character. | Spectral or energy check: the texture band is still there against a clean-speech-only baseline. |
| D Persona / emotion | Personality, behavior, and emotion live in the voice. The feeling band is this 80 ms clock. Empathy is the weight in [`EMPATHY.md`](../EMPATHY.md). Persona instructions are [`personas/`](../personas/README.md). | Those docs stay. Emotion maps to prosody and wash. No scripted empathy line. |
| E One-dot | One mark. Speak, then the workflow continues. The surface is [`ONE-UI.md`](../ONE-UI.md). | One `/` surface. The mic path records without a second panel. A test shows the workflow hook firing. |
| F Docs / runbooks | How to run the clock and the harness, in this file and in `SELF-EVAL.md`. | The commands below run as written. |
| G Latency | The live bar is the frame: 80 ms. Research text in [`domains/04-latency-streaming.md`](../../research/galion-voice-v2/domains/04-latency-streaming.md) is desk synthesis. The lock wins. | `frame_constants_match_locks` passes. A streaming measurement, or an honest stub measurement, is a row in `SELF-EVAL.md`. |

## Runbook

From the repository root:

```bash
cargo test --offline -p galion-heartbeat
cargo run --offline -p galion-heartbeat --bin galion-heartbeat-presence -- specs/presence.json
galion-gates/g6-rika.sh
galion-gates/ps-presence.sh
scripts/eval/virtual-user.sh
```

`galion-heartbeat-presence` exits 0 when the replay is green, 1 when it is
red, 2 on a bad spec or a missing file. The virtual user writes
`galion-gates/results/presence-eval/` and is the phone-path loopback
described in `docs/eval/PRESENCE-EVAL.md`. It does not measure PSTN.

Copy the command, the exit, and the measured number into
[`SELF-EVAL.md`](SELF-EVAL.md). A section with no row there is blocked.

— Maciej Grajczyk
