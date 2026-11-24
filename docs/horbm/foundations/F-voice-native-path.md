# F-voice — HORBM-native voice path

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | S10, four seats: architecture, ear, mouth, tests and this page |
| Written | 2026-11-24. No voice audio, no recording, no model download. |
| Repo | `galion-studio/galion-one-voice`, rebased onto `main` `8caa52a` (brain #252 and conductor #254). |
| This change | `horbm/crates/horbm-voice`. Additive. Nothing was archived. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this checkout and was not created. |

The brain is the center. One conductor frame (80 ms, virtual clock only) polls the ear, and on end of speech calls `horbm::Brain::turn`, then pulls one mouth frame. Piper and Vosk are optional adapters (`--features piper` and `--features vosk`). The default crate does not depend on them. The default build is CPU. The `gpu` feature compiles a device probe and returns GPU only when a device is present. This loop is not `Heartbeat::step`.

`galion-conductor` is on `main` and is its own workspace, so this crate keeps that clock's numbers (`FRAME_MS` 80, barge-in silence on the frame that contains 200 ms, reported as 160 ms) instead of a path dependency. A listed backchannel does not cancel. The host calls `VoiceLoop::tick` once per conductor frame. `Brain::turn` is the phase 4 clause pick: a card hit speaks the stored clause, and a miss is `I don't know.` Open retrieval (#253) and the learn pass (#255) are not called from this tick.

Gates, on the virtual clock: first mouth frame is one frame after end of speech (80 ms, budget 160 ms, inside the published 240 ms target). Barge-in yields at 160 ms (lock 200). Six frames of `yeah` (480 ms) do not cancel.

## KEEP / ADAPT / ARCHIVE

Recommendations only. This change does not move or delete any of these.

| Component | Call | Why |
|---|---|---|
| `horbm/crates/horbm` | KEEP | The turn. Empathy, cards, research cap, honest miss. |
| `horbm/crates/horbm-voice` | KEEP | This loop. Ear trait, brain, mouth trait, virtual clock. |
| `crates/galion-conductor` | KEEP | Floor clock this loop is driven by. Own workspace, so the numbers are copied, not linked. |
| `crates/galion-heartbeat` | KEEP | Band step. This loop runs beside it, not inside it. |
| `crates/galion-presence` | KEEP | The 200 ms barge-in gate. Re-checked here on the virtual clock. |
| `voice-stack/empathy` | KEEP | Already inside `Brain::turn`. |
| `voice-stack/research-gateway` | KEEP | Deep miss. The brain holds the 3 s cap. This loop dials nothing. |
| `voice-stack/contract` | KEEP | Shared types. Not the turn. |
| `voice-stack/audio-io` | KEEP | Mic and speaker. The host feeds frames in. |
| `voice-stack/emotion-in` | KEEP | Feeling from the mic. Not the reply. |
| `voice-stack/one-dot-ui` | KEEP | Locked one-dot UI. |
| `crates/galion-cascade` | KEEP | Chat door. Voice does not go through it. |
| `crates/galion-sideband` | KEEP | Phase 6. This tick does not post a mark. |
| `tools/symphony/mouth` | KEEP | Per-frame articulation targets. Not the speaker process. |
| `tools/symphony/throat`, `lungs`, `listen-back` | KEEP | Organs beside the loop. Not the brain. |
| `apps/nebula-voice-v2` | KEEP | Visual. Not the speech turn. |
| `voice-stack/horbm-bridge` | KEEP | The 7881 brain still loads cards through it. The turn tick is `VoiceLoop`, not a second brain. |
| `voice-stack/local-server` | KEEP | Loopback host. Each spoken turn is one `VoiceLoop::tick`. Piper (`--features piper`) and the clone worker are mouth adapters. CPU is the default device. The default build does not link Piper. |
| `voice-stack/tts-piper-stream` | ADAPT | Optional mouth (`feature = "piper"`). Not a core dependency. |
| `runtimes/galion-piper-rt` | ADAPT | Stays the out-of-process CPU runtime behind that adapter. Lock pin unchanged. Not linked here. |
| `voice-stack/samantha-voice-clone` | ADAPT | Mouth adapter on the 7881 server. Selected only for an explicit cuda opt-in. CPU does not select it. |
| `voice-stack/asr` | ADAPT | Optional ear (`feature = "vosk"`). `FakeAsr` is the test double. |
| `crates/galion-asr-vosk` | ADAPT | Out-of-process worker. Not linked. |
| `voice-stack/vad` | ADAPT | Endpoint into the ear. `ort` stays out of this workspace. Not linked. |
| `voice-stack/emotion-prosody` | ADAPT | The loop already passes the brain's rate, energy, pitch, and pause. Apply them on the mouth. |
| `voice-stack/linux-package` | ADAPT | Supervisor. Does not link HORBM yet. |
| `voice-stack/eval-golden` | ADAPT | Still scores a Piper round-trip. Score this loop later. |
| `voice-stack/latency-harness` | ADAPT | Measure this clock later. |
| `crates/galion-ml-voice` | ADAPT | Clip compiler. Not the live turn. |
| `voice-stack/horbm-bridge/seam/horbm` | ARCHIVE | Old type seam. The bridge manifest says it is not linked. |
| `voice-stack/pipeline-orchestrator` | ARCHIVE | Old listen / think / speak loop. `horbm::Brain` is not linked. |
| `crates/galion-v2-live` | ARCHIVE | Out-of-process Piper, Vosk, and LLM lane. The brain is not the center. |

Archive means a later pass moves the tree aside. It does not mean delete. The audit swarm owns that pass.

## Host wiring (S13)

The loopback server now calls `VoiceLoop::tick` for each spoken turn. Piper is a mouth adapter (`--features piper`). The clone worker is the other mouth adapter, and only for an explicit cuda opt-in. The default build's mouth is the in-process tone, so Piper is not in that cargo tree. CPU stays the boot device (`brain` `horbm`, `mouth_device` `cpu`). The `[boot]` table from the open boot pull is not on this branch, so those two values live in the server instead of `LOCKS.toml`.

Nothing was moved to `archive/`. `voice-stack/pipeline-orchestrator` is still the mic endpointer and the gated mouth test. `voice-stack/horbm-bridge` still loads the cards. The unlinked seam and `crates/galion-v2-live` were already unused; the open audit pull moves those, and this branch does not move them again.
