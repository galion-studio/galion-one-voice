# Symphony self-eval

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03.

A section is complete when its gate passes and the result is written here.
A skip of a missing gitignored wav is a pass of that gate. A missing model
weight is blocked, not a fake pass. Measurable lines only.

Harness: `scripts/clone/clone-landing-smoke.sh`. Re-run it. Do not copy a
TTFA number from a protocol stub.

## B · Clone landing

| Gate | Result | Evidence |
|---|---|---|
| Current Samantha and Jarvis pins, WAV only, no model swap (`nano`) | PASS | `scripts/clone/REFS.sha256`; persona TOML `model = "nano"`; unit `reference_pins_keep_the_current_wavs_and_jarvis_texture` |
| `present` / `clip_path` / `send_reference` wired; `GALION_CLIP_STORE` optional | PASS | `docs/galion-voice-v2/personas/voice/{samantha,jarvis}.toml` |
| sha256 checked when the wav is present; clean skip when absent | PASS | smoke line `status=skip` on this machine (files not in the work tree); unit `absent_reference_is_skipped_and_a_bad_file_is_refused` |
| Unconsented path, MP3, hash mismatch, denoise: no PCM | PASS | smoke exits 3 on those requests; stdout empty |
| Jarvis hub texture is a separate toggleable layer, not stripped | PASS | `texture_layer=false` logs `present=1 enabled=0 stripped=0`; `strip_texture` exits 3; clip round-trip keeps the layer |
| CPU-only Chatterbox Nano, out of process, **1 vCPU** this child | PASS | smoke `threads=1 device=cpu model=nano`; `GALION_CLONE_THREADS=8` logs `asked=8 applied=1`; GPU device exits 3 |
| No GGUF, no GPU, PyTorch not linked into the crate | PASS | smoke `gguf=false`; worker source has no `import torch`, no `.gguf`; `in_tree_ggml` stays false because `LOCKS.toml` was not edited |
| `frame_ms = 80` | PASS | read from `LOCKS.toml`; not edited |
| Voice-session planning budget recorded | PASS | ~1.0 vCPU average, burst ~4 across the cascade (not this child), RAM 0.65–1.15 GB planning only, not measured |
| Owner t273 local send; public demo still closed | PASS | `LEDGER.md`; page `rights` string unchanged |
| Galion-format emit (`GALIONVC`), not safetensors-as-runtime, not GGUF, not `.gq` weights | PASS | `speech/galion_clip.rs`; magic `GALIONVC`; `.gq` writer still deferred |
| Lock flip `reference_clips=true` | BLOCKED-on-owner | `LOCKS.toml` still `reference_clips = false`. This PR does not edit it. Owner flips the key. |
| Model PCM / Chatterbox generate | BLOCKED | weights not fetched (~3 GB pinned safetensors). Worker does not import torch and does not call generate. No TTFA product claim. |

Section B gate: **PASS** for wiring, the Galion clip, and the texture toggle. Two items stay blocked: the lock flip (**BLOCKED-on-owner**) and Chatterbox generate (**BLOCKED**, weights not fetched). Generate stays out of process and CPU-only when an owner fetches the pack. It must not strip the hub layer.

Smoke on this machine, refs absent: `PASS frame_ms=80 session_vcpu=1 device=cpu gguf=false denoise=false texture_toggle=separate galion_clip=GALIONVC lock_flip=BLOCKED-on-owner`. Both sha256 rows printed `status=skip`. `generate=blocked reason=weights-not-fetched`. Speech unit tests 30/30. Persona config test passed.

This file does not grade sections A, C, D, E, F, or G. C's texture rule is enforced here for the reference bytes (they are not cleaned). It is not a spectral measurement against a clean-speech baseline.

— Maciej Grajczyk
