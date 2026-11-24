# F — duplex loop (S26)

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S26. One closed full-duplex harness. |
| Crate | `horbm/crates/horbm-duplex`. Member of the `horbm` workspace. The root workspace excludes `horbm`. |
| Clock | 80 ms, 16 kHz, 1280 samples. Virtual. `Duplex::tick` is the only step. |
| Persona | Samantha. Any other id or voice is refused. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not added. |
| Live | S28 wires the real adapters. This page's virtual-clock test stays the CI gate. See [F-duplex-s28.md](F-duplex-s28.md). |

Both bands are stepped on the same call. The you band is one scripted frame. The me band is the mouth frame, when Samantha is speaking. A listed backchannel keeps the turn. Any other overlap stops the me band on the frame that starts 160 ms after the onset. That is inside the 200 ms barge-in lock.

## Path

Ear, then the recognizer port, then the emotion port, then `horbm::Brain` (the cell is written while the person is still speaking), then `galion_conductor::Conductor`, then the breath port, then a `horbm_voice::Mouth`. The side band is the conductor's offline client. This script is not a lookup, so the tick does not wait on it.

## Ports

These crates are open and are not linked. The integration test holds the stand-ins. The traits are the seam.

| Port | Open change | Stand-in in `tests/duplex.rs` |
|---|---|---|
| `Recognizer` | PR 279, `horbm-asr::SpeechRecognizer` | one scripted word per voiced frame, samples ignored |
| `Emotion` | PR 263, `EmotionCore::step` | Samantha baseline each frame |
| `Breath` | PR 278, `plan_speech` | exhale while the me band is speaking, no samples |
| `Persona` | PR 276, `Persona::samantha` | brevity 0.55, warmth 0.8, miss `I don't know.` |

## Budgets

Frame counts are the gate. The 60 ms compute budget inside a frame is a wall-clock measurement for an isolated box, and this harness does not claim it.

| Stage | Budget | Source |
|---|---|---|
| Ear, recognizer, brain read | 0 extra frames | R4 §4.3. The cell is filled during speech. |
| Conductor hang | 1 frame (80 ms) | R4 §4.3 eager end of turn |
| Reaction | within 1 frame of eager | R4 §4.3 wordless reaction |
| Voice, from eager end of turn | p50 ≤ 200 ms, p95 ≤ 240 ms, onset ≤ 300 ms | R4 §4.3, §12, `onset_p50_max_ms` |
| Affect to voice | ≤ 2 frames, ≤ 160 ms | `affect_to_voice_max_frames`, `affect_to_voice_max_ms` |
| Barge-in to silence | ≤ 160 ms, and ≤ 200 ms | this harness, `barge_in_max_ms` |
| Backchannel | cancels stay 0 | `backchannel_cancels = false` |
| First spoken piece | ≤ 2 words | R4 §11 phase 4 |

The measured line is printed by `full_duplex_samantha_both_bands`. It has counts only. This run printed:

```text
duplex s26 frame_ms=80 clock=virtual persona=samantha ear_frames=0 recognizer_frames=0 brain_frames=0 conductor_frames=1 commit_frames=1 reaction_frames=0 voice_ms=160 affect_frames=2 barge_ms=160 barge_frames=2 backchannel_frames=6 backchannel_cancels=0 overlap_live_frames=8 you_frames=20 me_audible_frames=10 first_clause_words=2 research_frames=0 result=pass
```

## Left where it was

`horbm-voice::VoiceLoop` still runs ear, brain, and mouth without this conductor. Nothing was deleted. No Windows workflow was added. The Ubuntu job `voice-stack horbm router` tests this crate.
