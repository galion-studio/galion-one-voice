# Swarm 10 — clone-compiler generate

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Written 2026-11-15 22:16 +01:00.

The spelling is **Galion**. The product is Galion Voice.

Role: generate (A1). Lane plan: [PR #153](https://github.com/galion-studio/galion-one-voice/pull/153) (`docs/swarm-10/2026-11-15/clone-compiler/PLAN.md` on `cursor/swarm-10-clone-compiler-plan-94c0`). This PR does not edit that plan.

## Verdict

**BLOCKED.** No Nano PCM was produced. Protocol-stub PCM was not used. Piper was not run and is not the original clone.

A0 stays an owner decision. This worker did not edit `LOCKS.toml` or `scripts/clone/REFS.sha256`. The CPU venv from `scripts/clone/requirements.txt` is staged on this machine only, under gitignored `.venv-clone/`. The tokenizer was not staged: the operator tree is absent, and this run did not download it.

## Host

| | |
|---|---|
| Hostname | `cursor` |
| OS | Linux 6.12.113+ x86_64, 4 CPUs |
| When | 2026-11-15 22:07 +01:00 through 22:16 +01:00 |
| rustc | 1.85.0 (4d91de4e4 2025-02-17), via `cargo +1.85.0` |
| Python | 3.12.3 |
| Base | `origin/main` `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0` |
| Branch | `cursor/swarm-10-clone-compiler-generate-9e37` |

This is not the NEXT-ITERATION host (`grok-bot-vm-511777615`). Numbers below are from commands run here.

## What was on disk

| Piece | Result |
|---|---|
| `docs/galion-voice-v2/personas/refs/samantha.wav` | absent (`test -f` exit 1). Placeholder text only. |
| `docs/galion-voice-v2/personas/refs/jarvis.wav` | absent (`test -f` exit 1). Placeholder text only. |
| `weights/clone/` | absent (`test -d` exit 1). `weights/` contains only `.gitignore`. |
| `/workspace/galion-symphony-v2/clone-env/ckpt-nano/` | absent (`test -d` exit 1). |
| System `python3` `import torch` | `ModuleNotFoundError: No module named 'torch'` before the venv existed. |
| `LOCKS.toml` `[formats.exceptions.v2s14_chatterbox]` | `reference_clips = true`, `ref_clip_decision = "t623u"`. sha256 `b97fa057f6136d724f1283bdeb77d9d175a08a0245177ad85ae8e94ca3040f59` before and after. |
| `scripts/clone/REFS.sha256` | Live rows are the t623u pins (`samantha` `e4bc60c3…84a4641`, `jarvis` `60ad5457…58b8f6b6`). The t273 rows stay commented. sha256 `e4260b9549e024a4292ec702dd135141f71de000fa630856703ce6c01f8ac03e` before and after. |

No `GALION_CLONE_REF_SHA` or `GALION_CLONE_REF_ALLOW` override was added. Voice TOML hashes were not edited.

## What changed

| Path | Change |
|---|---|
| `scripts/clone/stage-clone-prep.sh` | Stages a CPU venv or copies the five pinned tokenizer text files. Refuses audio, `.pt`, and safetensors. Does not fetch safetensors, does not call generate, and exits if `REFS.sha256` or `LOCKS.toml` changes. |
| `scripts/clone/stage-clone-prep.test.sh` | Offline checks for that script. |
| `.gitignore` | Ignores `.venv-clone/`, the path named in `scripts/clone/requirements.txt`. |
| `.github/workflows/gates.yml` | Runs the offline staging test. No pip, no generate. |
| this file | The A1 notes. |

Not committed: `.venv-clone/` (1.8G on this machine), tokenizer json, WAV, PCM, safetensors. `git check-ignore` reports `.venv-clone/bin/python` ignored by `.gitignore`. `git status` after the venv install did not list it.

## Tokenizer staging

```text
bash scripts/clone/stage-clone-prep.sh --tokenizer
```

Exit 1. Stderr: `stage-clone-prep: BLOCKED tokenizer source absent: /workspace/galion-symphony-v2/clone-env/ckpt-nano`. Stdout: `stage-clone-prep: pcm=not-run`. No `weights/clone/tokenizer/` directory was created. The five files were not downloaded.

## CPU venv

The first `GALION_CLONE_PREP=1 scripts/clone/stage-clone-prep.sh --venv` failed because `python3 -m venv` reported `ensurepip is not available`. This machine then installed the Ubuntu package `python3.12-venv` (local apt only; not a repo change). The script now exits 1 with `BLOCKED python3 ensurepip is missing` when that module is absent, instead of leaving only the interpreter traceback.

Second run, same command, exit 0:

```text
stage-clone-prep: venv=ready path=/agent/repos/galion-one-voice/.venv-clone
stage-clone-prep: pcm=not-run
```

Probe on that interpreter:

```text
torch=2.6.0+cpu cuda_build=None
from_local=ckpt_dir,device,nano
```

`pip` resolved `chatterbox-tts @ git+https://github.com/resemble-ai/chatterbox.git` to commit `ab55923c27757c90cfcc407baa57dc25a7d25217`. `pip show` still prints version `0.1.7`. That git tree's `ChatterboxTurboTTS.from_local` includes `nano`. This note does not claim the PyPI 0.1.7 wheel has `nano`. The install pulled `torch==2.6.0+cpu` and `torchaudio==2.6.0+cpu` (the CPU index). The venv is not on the default `PATH`. A later generate has to call `.venv-clone/bin/python`.

`--check` after that install:

```text
stage-clone-prep: tokenizer=absent dest=/agent/repos/galion-one-voice/weights/clone/tokenizer
stage-clone-prep: venv=ready path=/agent/repos/galion-one-voice/.venv-clone
stage-clone-prep: pcm=not-run
stage-clone-prep: BLOCKED
```

Exit 1. Venv ready does not make generate ready.

## Generate attempt

Both lines from NEXT-ITERATION §4.1, `GALION_CLONE_LOCAL_REFS=1`, `GALION_CLONE_MODEL=nano`, no stub, no stream, no clip store. System `python3` at 22:07 +01:00:

| Persona | Exit | Wall | stdout |
|---|---|---|---|
| samantha | 2 | 0.026 s | 0 bytes |
| jarvis | 2 | 0.025 s | 0 bytes |

Stderr, both:

```text
galion-clone-cpu: threads=1 device=cpu denoise=false
galion-clone-budget: session_vcpu=1 threads=1 burst_max=4 ram_gb=0.65-1.15 device=cpu model=nano gguf=false
galion-clone-absent: pinned safetensors missing
```

The same Samantha line through `.venv-clone/bin/python` after the venv was ready: exit 2, wall 0.024 s, stdout 0 bytes, same `galion-clone-absent: pinned safetensors missing`.

`main()` returns that absent line before `resolve_allowlisted`. This VM therefore did not reach `reference sha256 is not allowlisted`. That refuse is what the plan cites from the other host, where the t273 wavs and the pinned safetensors were present. It was not re-run here. The live allowlist on this main is still the t623u rows, so an original-ref generate stays refused until Maciej edits the pins himself.

No file was written under `out/`. No WAV was committed. Engine was not `chatterbox-nano`. First-PCM and `ttfa_ms` were not measured.

## Tests run here

| Command | Result |
|---|---|
| `bash scripts/clone/stage-clone-prep.test.sh` | `stage-clone-prep.test: ok`, exit 0 |
| `python3 scripts/clone/test_worker_gate.py` | 13 tests, 0.287 s, OK, exit 0 |
| `bash scripts/check-locks.sh` | `OK: all Eureka locks hold (LOCKS.toml)`, exit 0 |
| `bash scripts/check-no-forbidden.sh` | `OK: forbidden-format + forbidden-runtime gates pass`, exit 0 |
| `cargo +1.85.0 test --workspace` | exit 0. Sum of `test result:` lines: 883 passed, 0 failed, 5 ignored. Default features. This does not include `--features speech`. |
| `cargo +1.85.0 test -p galion-ml-voice --features speech --lib` | exit 0. `109 passed; 0 failed; 1 ignored` in 0.62 s. |

The speech-feature run is a clean tree: the seven NEXT-ITERATION failures were not reproduced here, because the t273 wavs and the pinned safetensors are absent. That does not close A5. A5 belongs to the truth worker.

## CI

Not run at the time this note was written. The PR checks are the record. Required workflows named in the lane plan: `test`, `emotion-v2 harness (CPU stub)`, `playwright`.

## Open questions for Maciej

1. **A0.** Re-open the t273 originals (he edits `LOCKS.toml` and `REFS.sha256`), or state that main stays on the t623u CC0 donors and symphony B/C will be re-stated. This PR does not choose.
2. This VM has no original wavs, no `weights/clone` safetensors, and no `/workspace/galion-symphony-v2/clone-env/ckpt-nano/`. Tokenizer staging cannot finish until that tree, or another pin-matching copy, is on the machine. This PR did not fetch it.
3. The staged venv is local to this checkout and is gitignored. It is not a live clone mouth. `/one` stays Piper + Samantha persona until a clone mouth is actually live.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
