# F — emotion core

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | S11, moment-to-moment emotion core. Roles: state, inside factors, intent wire, tests and this note. |
| Written | 2026-11-24. No voice audio, no recording, no weight, no network call. |
| Base | `main` `4b828b6`. |
| Crate | `crates/galion-emotion-core`. HORBM calls it from `horbm::Brain`. |
| Locks | The four EmotionWire bits are unchanged. `[consent] learn` is listed in [`F-inner-life.md`](F-inner-life.md). `SOUL.md` is not in this tree. |

The core is the speaker's moment: a point and a velocity on valence, arousal, dominance, and approach. It is not a list of emotions. The twelve names in `tools/emotion-v2/harness/emotions.py`, and the design coordinates in `emotion-v2/emotions.toml`, are a projection (`project_universal`). The step does not compute that projection and does not write a Cowen pin. The 8-byte `EmotionWire` is produced by `galion-emotion::encode_with_consent`. The three-bit affect label is cleared. Those three bits cannot hold an open state.

## Baseline and moment

`VoiceBaseline` is who is speaking (persona id, voice id, rest point, design pitch centre). `EmotionCore::step` never assigns it. `MomentState` is how they feel now, plus a velocity in axis units per second. At rest factors and no pull, the moment stays on the baseline. A pull or a factor moves only the moment. `persisted_baseline` returns that same baseline, and only when `remember` is set. It does not store the swing.

The clock is `LOCKS.toml` `clock.frame_ms` (80). `VirtualClock::advance` yields 0, 80, 160, …. A step whose `t_ms` is not the next frame is refused and leaves the core as it was.

## Inside factors

Breath, energy, fatigue, and health are inputs, not categories. Breath phases use the MnE names `rest`, `inhale`, `hold`, `speech`, `exhale`. When a tick does not bring a breath, a 4.00 s stand-in cycle fills the slot. It is not the lungs planner and it is not a litre.

The couplings are stand-in knobs (`FATIGUE_AROUSAL` and the rest in `factors.rs`). Published fatigue magnitude is still none. The knobs exist so a closed eval can see the moment move. Offsets are measured from rest (energy 0.70, fatigue 0.05, health 1, lung volume 0.55). The spring (`STIFFNESS` 8, `DAMPING` 6) chases baseline plus offset plus an admitted pull. No libm on this path.

## Intent, consent, ports

The inner-life model is `inner`, written up in [`F-inner-life.md`](F-inner-life.md). `standins/` still holds four fixed files (`persona.txt`, `memory.txt`, `inner-life.txt`, `intent.txt`), schema `galion-emotion-standin/1`. Persona and memory stay stand-ins. `inner-life.txt` is the seed for the first care, want, and expectation. The intent script still expands to one `IntentMsg` per frame for the closed eval that does not run an inner-life turn.

`IntentMsg` is 72 bytes, magic `GEMI`, version 1, little-endian. The verb is an open ASCII token of at most 16 bytes, not a category. A pull is an offset from the baseline. Encode and decode are byte-identical for a valid message.

Consent is EmotionWire byte 7, low four bits, default deny: `sense`, `adapt`, `remember`, `share`. Her own intent moves the moment with every bit off. An inbound pull is mixed only when `sense` and `adapt` are both on. Inbound factor fields are ignored. `remember` gates `persisted_baseline`. `share` gates `shared_wire`. `local_wire` stays inside the runtime either way. Outbound expression does not require the caller's sense, which is the rule already in `galion-emotion`.

`Brain::step_moment` steps this core. `Brain::tick_frame` still steps only empathy. `Brain::conductor_cue` is the conductor's read: baseline, moment, flow, factors, breath, and the consent flags. The conductor does not own the state and this crate does not grant the floor.

## Budget and what is absent

`STEP_ALLOC_BUDGET` is 0. `STEP_FLOP_BUDGET` is 96, counted by hand from `apply`. The measured check is `tests/zero_alloc.rs`: a thousand steps, the cue, and the local wire, with a counting allocator.

Not in this crate: a lungs planner, a category classifier, and voice audio. `EmotionCore::step` still does not write `VoiceBaseline`. `LOCKS.toml` `[consent] learn` is the inner-life weight gate (`deny`); the four EmotionWire bits are unchanged. The list of that edit is in [`F-inner-life.md`](F-inner-life.md). `galion-feeling` remains the document chase over `galion-mne-contract`. This core is the fixed-size state the brain and the conductor call.

```shell
cargo test -p galion-emotion-core
cargo test --manifest-path horbm/Cargo.toml -p horbm --test moment
bash scripts/check-locks.sh
```
