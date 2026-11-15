# Swarm 10 — clone-compiler texture

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Written 2026-11-15 22:09 +01:00.

Lane: clone-compiler. Role: texture. Item A3 and symphony self-eval section C.
Plan: [PR #153](https://github.com/galion-studio/galion-one-voice/pull/153) (`docs/swarm-10/2026-11-15/clone-compiler/PLAN.md`). This PR does not edit that file.

**Section C: BLOCKED.** The original Jarvis clip is not on this machine. No spectral number was measured. No Nano clone PCM was produced. Piper samples were not used.

## Host

| | |
|---|---|
| Hostname | `cursor` |
| Kernel | Linux 6.12.113+ x86_64 |
| CPUs | 4 |
| RAM | 15 GiB |
| Python | 3.12.3 |
| Rust for `jarvis-texture.sh` | 1.85.0 (`4d91de4e4`, 2025-02-17), via `rustup run 1.85.0` |
| Default rustc before that | 1.83.0 (not used for the script) |
| Tree | `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0` plus this branch |
| `torch` | `ModuleNotFoundError: No module named 'torch'` |
| `chatterbox` | `ModuleNotFoundError: No module named 'chatterbox'` |
| `weights/` | only `.gitignore`. No `weights/clone` |
| `GALION_CLIP_STORE` | unset |
| `/workspace/galion-clip-store` | absent |

## Original clip

Looked up 2026-11-15 22:03 +01:00.

`docs/galion-voice-v2/personas/refs/` contains `.gitignore`, `jarvis.placeholder`, and `samantha.placeholder`.

`test -f docs/galion-voice-v2/personas/refs/jarvis.wav` exited 1.
`test -f docs/galion-voice-v2/personas/refs/jarvis-clean-baseline.wav` exited 1.

The t273 digest named by `JARVIS-TEXTURE.md` and the commented row in `scripts/clone/REFS.sha256` is `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60`. That file was not here, so it was not hashed and not denoised.

`docs/galion-voice-v2/personas/voice/jarvis.toml` `[reference]` on this tree: `texture = "preserve-hub"`, `denoise = false`, `texture_stripped = false`, `texture_toggle = "GALION_JARVIS_TEXTURE"`, `texture_default = "on"`. There is no `[texture]` table. The toml `sha256` is the live t623u pin `60ad5457dac31af915a9679760705296f9aad17e15ee84885385509b58b8f6b6`. That pin was not edited.

`LOCKS.toml` lines read, not edited: `reference_clips = true`, `ref_clip_decision = "t623u"`.

Listening-set files under `docs/galion-voice-v2/evals/listening-set/` include names such as `jarvis-en_US-ljspeech-medium-single.wav`. They were not opened. They are not the original clip.

Spectral comparison: **not measured**.

## What changed

- `scripts/eval/jarvis-texture.sh`: comment only. The printed JSON and the exit code are unchanged. The comment says the JSON is the synthetic toggle, and that a missing original clip is BLOCKED.
- `docs/galion-voice-v2/symphony/JARVIS-TEXTURE.md`: a Swarm 10 record that this host had no original wav.
- This notes file.

`LOCKS.toml`, `SOUL.md`, `config.yaml`, `scripts/clone/REFS.sha256`, and the voice toml hashes were not edited. No wav, PCM, or weight file was added.

## Commands run

### `scripts/eval/jarvis-texture.sh`

Finished before 2026-11-15 22:09:34 +01:00. `rustup run 1.85.0 bash scripts/eval/jarvis-texture.sh`. Exit 0. Stderr empty. Stdout:

```text
running 9 tests
.........
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.15s

{"gate":"jarvis-texture","result":"PASS","wav":"BLOCKED","keep_ratio":0.70,"off":"asset","on":"asset+layer"}
```

This is the synthetic layer toggle (`cargo test -p galion-ml-voice --lib jarvis_texture`). `wav` is `BLOCKED` because the script does not open the owner wav. It does not close section C. It is not Nano PCM.

### Section C gate (`scripts/symphony/eval.py --only C --no-record`)

Run on `8980e1a` before the comment edit. Exit 1. Stdout:

```text
C OPEN — checker self-test passed on a synthetic tone; jarvis.toml has no [texture] preserve toggle
== symphony A–G: 0 PASS, 1 OPEN, exit 1
```

The self-test is a synthetic tone. The gate looks for a `[texture] preserve` table. The policy on this tree is `[reference] texture = "preserve-hub"`. No `[texture]` table was added. The matrix in `SELF-EVAL.md` was not rewritten (that recorder is not this PR).

### `scripts/symphony/check-c-texture.sh`

Same tree, before this PR's edits. Exit 1. Stdout:

```text
C FAIL — clone worker names a denoise/strip step
```

The script greps `chatterbox_worker.py` for `denoise|noisereduce|highpass|strip_texture`. Those strings are in the refuse paths (`galion-clone-refuse: denoise refused; texture is character` and `galion-clone-refuse: texture layer is not stripped`). The grep does not show that a denoise ran. This PR does not change that script (truth / self-eval own other gates; the plan's texture file split leaves the worker's refuse lines alone).

### Worker refuses

Weights are absent, so a request without `GALION_CLONE_STUB_PCM=1` exits 2 before the JSON is read. `strip_texture` on that path, 0 stdout bytes, exit 2, stderr ends with `galion-clone-absent: pinned safetensors missing`.

With `GALION_CLONE_STUB_PCM=1` the process reads the request. Stub PCM is not a clone and not a spectral measurement. Where the refuse fired, stdout was 0 bytes.

| Request | Env | Exit | Stdout | Stderr line |
|---|---|---|---|---|
| `strip_texture: true`, `denoise: false` | `GALION_CLONE_STUB_PCM=1` | 3 | 0 | `galion-clone-refuse: texture layer is not stripped` |
| `denoise: true` | `GALION_CLONE_STUB_PCM=1` | 3 | 0 | `galion-clone-refuse: denoise refused; texture is character` |
| `texture: "clean"` | `GALION_CLONE_STUB_PCM=1` | 3 | 0 | `galion-clone-refuse: denoise refused; texture is character` |
| `denoise: false`, persona jarvis | `GALION_JARVIS_TEXTURE=0` and `GALION_CLONE_STUB_PCM=1` | 0 | 402 | `galion-clone-texture-layer: persona=jarvis present=1 enabled=0 stripped=0` |
| `denoise: false`, persona jarvis | `GALION_CLONE_STUB_PCM=1` | 0 | 402 | `galion-clone-texture-layer: persona=jarvis present=1 enabled=1 stripped=0` |

The 402-byte stdout on the last two rows is protocol-stub PCM (`galion-clone-emit: galion-vc`). It was left in `/tmp` and is not a hub measurement. `enabled=0` with `stripped=0` is the layer mute. It does not rewrite an asset. There was no asset on disk.

Text used: `Here. What do you need?`. Device `cpu`. Persona `jarvis`.

### Not run

- Nano `generate` from the original clip. A0 is still the t623u allowlist, the wav is absent, and torch / chatterbox / `weights/clone` are absent. Generate is the A1 lane.
- A spectral compare. No original wav and no `jarvis-clean-baseline.wav`.
- `scripts/symphony/self-eval.sh` with `--record`. That would rewrite the A–G matrix. Not this role.
- Full `cargo test --workspace`. Not run on this VM. CI runs it.

## CI

Head `3b139efd8b30207bacb5f4c91f7a4fd2770c5152`. All three registered checks completed `success` (GitHub Actions, 2026-11-15, Europe/Zagreb +01:00):

| Check | Conclusion | Run |
|---|---|---|
| `test` | success | [43762200767](https://github.com/galion-studio/galion-one-voice/actions/runs/43762200767) (22:10:36–22:12:42 +01:00) |
| `emotion-v2 harness (CPU stub)` | success | [43762200762](https://github.com/galion-studio/galion-one-voice/actions/runs/43762200762) (22:10:36–22:10:50 +01:00) |
| `playwright` | success | [43762200778](https://github.com/galion-studio/galion-one-voice/actions/runs/43762200778) (22:10:35–22:11:21 +01:00) |

`nebula-voice-v2` did not register a check on this head. The plan treats that path filter as not a failure. No check was skipped, cancelled, or failed. This notes edit is a later commit, so its own CI is a separate head.

## Open questions

The section C checker still reports OPEN because `jarvis.toml` has no `[texture] preserve` table, while `preserve-hub` already sits under `[reference]`. Adding that table would not put the original clip on disk. `check-c-texture.sh` exits 1 because the worker source names the refuse tokens. Neither result is a measured spectrum.

## Decisions for Maciej

1. A0 stays open. Section C on the original Jarvis clip (`c2200b59…`, hub kept, no denoise) needs that wav on a machine that may read it, plus a clean-speech baseline of the same take, plus original-ref Nano PCM from A1. This VM had none of those. This PR does not choose t273 or t623u.
2. The t623u jarvis pin's own note says the hub is not baked into that donor. A Piper listening-set file would not close section C either.
3. Live mouth stays Piper until a clone mouth is actually live. Nothing here says the clone spoke.
