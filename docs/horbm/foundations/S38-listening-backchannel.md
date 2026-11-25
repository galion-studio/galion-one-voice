# S38 — listening backchannels

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S38, full-duplex listening |
| Written | 2026-11-24. No weight download, no voice file, no paid API, no merge. |
| Base | PR #295, branch `cursor/horbm-s32-ear-eot-e0f5`. This change stacks on that branch. It does not retarget `main`. |
| Persona | Samantha only. Aria does not nod. A clone-pack id does not nod. |
| Mouth | Generated tone by default. Piper only with the existing `piper` feature, and only for the short token. No audio file is committed. |
| Locks | `LOCKS.toml` was not edited. `gates.backchannel_cancels` stays false. `SOUL.md` is not in this tree and was not created. |

## What it decides

While the ear says the user still holds the turn, Samantha may make one short listening sound: `mm-hm`, `yeah`, `right`, or a soft breath-laugh. The sound is on her band. The user's turn stays open.

The slot is the quiet frame after a mild clause-final fall (about 1.8 semitones, energy down, finished word, 80 ms pause). That frame is still `TurnMsg::Hold`. The next quiet frame, at 160 ms, is where that fall yields, and a yield is not a nod.

A nod does not fire:

- mid-word (the frame is still voiced)
- on a rising question contour
- on a filler or an unfinished word (`um`, `and`, …)
- on `TurnMsg::Yield` (she would be taking the turn)
- sooner than 4.8 s after the previous nod (the middle of the 4–6 s band, 60 frames)
- for any persona other than Samantha
- when `GALION_BACKCHANNEL` is `0` or `off` (the config flag defaults on)

The nod is not a barge-in. It does not cancel the user, does not bump the epoch, and does not call the brain. The existing rule that a listed backchannel from the user does not cancel Samantha is unchanged.

There is no `horbm-duplex` crate in this tree. The duplex path is the voice loop: one me-band frame while the you-band partial continues. `galion-conductor` decides the same nod and does not play it. The two crates do not share a workspace, so each keeps a copy of the policy.

## Denied ear

`GALION_EAR_SENSE=0|off|deny`, `Ear::deny`, and `sense: false` already return `TurnMsg::PauseOnly` and drop the contour. Backchannels stay off on that path.

A pause timer cannot tell a clause-final fall from a question rise. Nodding on silence alone would talk over questions. End-of-turn keeps the pause-only rule it already had. The listening layer adds nothing.

## Choice and level

The token follows Samantha's moment (valence and arousal). Rest is `mm-hm`. A warm mid arousal is the breath-laugh. A high arousal is `yeah`. A steady mid arousal is `right`. An inbound feeling on the bus, when the frame is present, tugs that choice; a sad listener pulls a warm moment back to `mm-hm`. An outbound or absent wire is not a listener.

The level is a fraction of ordinary speech, clamped to 0.22..=0.55. The generated tone peaks at or under 110. Fake-mouth speech peaks at or above 200. The breath-laugh is the softest shape, so a tugged `mm-hm` can sit a little above that laugh and still under ordinary speech. Piper, behind its feature, speaks the same token and then scales the samples by that fraction. It does not open the reply queue.

## Files

- `crates/horbm-ear/src/eot.rs`, `src/lib.rs` — `ListenCue` beside the turn message
- `crates/galion-conductor/src/backchannel.rs`, `src/conductor.rs`, `src/lib.rs`, `tests/backchannel.rs`
- `horbm/crates/horbm-voice/src/backchannel.rs`, `src/turn.rs`, `src/mouth.rs`, `src/piper_mouth.rs`, `src/lib.rs`, `Cargo.toml`, `tests/backchannel.rs`
- this note

`LOCKS.toml` and `SOUL.md` were not changed.
