# FP — first-principles audit

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Date | 2026-11-24 |
| Base | `main` `4b828b6a578b5a37c745c9a818d8ec152f16ffb9` (merge of PR #250) |
| Question | What is the HORBM voice path if Piper and the old cascade are not the architecture? |
| This change | Archive two trees nothing links. Leave the loopback server, the linked `horbm` crate, and the open foundation crates in place. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in the tree and was not created. |

HORBM is the heuristic open-retrieval brain. A turn is a lookup. There is no LLM on the hot path. Galion Research is off the tick, with the 3 second cap already in `voice-stack/research-gateway`. Owner instruction of 2026-11-24: do not build that brain on the old voice infrastructure or on the Piper TTS pipeline.

R4 (`docs/horbm/research/R4-cpu-realtime-integration.md` on closed, unmerged draft PR #207, §11) still says phase 4 leaves the Piper path unchanged. That sentence is research. It is not the architecture to implement. Phase 1 of that note is already on `main`: domain 04 stamps in `crates/galion-cascade` (PR #250, `4b828b6`). PR #251 is superseded by PR #253. Do not land both.

## Top findings

1. The process that speaks is `samantha-local` (`voice-stack/local-server`), loopback port 7881. The brain on that process is already `horbm::Brain::turn` through `voice-stack/horbm-bridge`: cards, then `I don't know.` The mouth on that process is Piper. Brain and mouth are different crates. Only the mouth is the old pipeline.
2. `crates/galion-v2-live` is the older duplex lane (Piper child, Vosk sidecar, clone worker, optional cloud reply). It was its own workspace. Nothing in `samantha-local` or in CI compiled it. It is archived below.
3. `voice-stack/horbm-bridge/seam/horbm` was a second package named `horbm`. Its `turn` returned `seam:horbm-not-landed`. The bridge links `horbm/crates/horbm`. The seam is archived below.
4. `CannedLlm` and `PhoneLane` (`crates/galion-cascade`) answer the console phone worker, not 7881. Default phone budget for Hermes is 15 seconds. That is the wrong cap. The 3 second cap is `RESEARCH_CAP` in the research gateway.
5. When `GALION_RESEARCH_BIND` is set, `route_heard` calls Research inside the turn, before the mouth starts. A deep miss can hold the first PCM for the whole cap. The local clause has to play first. Research lands on a later turn.
6. No MIT or Apache-2.0 speech synthesizer is a dependency of this tree. `galion-tts-piper` is MIT, but the sound is an ONNX pack plus espeak-ng (GPL-3, still counsel-open in the Piper pin). Flite is a dlopen on the cascade fallback, not a vendored MIT crate. Chatterbox is PyTorch, out of process, and the 7881 gate already refuses it on CPU. Do not recode Piper. Do not adopt those three as the native mouth.
7. `LOCKS.toml` is still a gate. `.github/workflows/gates.yml` runs `scripts/check-locks.sh` on every pull request. `galion-heartbeat`, `galion-presence`, and `galion-emotion` tests read the file. The Piper pin, the Chatterbox pin, consent deny, and the runtime ban are machine-checked. Calling the file "unlocked" in prose does not retire it. This change does not edit it.
8. `SOUL.md` and `config.yaml` were never committed. `git log --all -- SOUL.md` is empty. Docs that still name them as locked files are stale. `scripts/eval/nano-original-ref-ttfa.sh` hashes them only when they exist.
9. PR #249 merged as `204fd8cd` by `cursor[bot]` at 2026-11-24 17:07 UTC, about twelve minutes after the PR opened. The GitHub `gates` job on that head had finished green at 17:06 UTC. The PowerShell laptop block in the PR body had no recorded result at merge time. A later laptop run of that main is reported as 1034 passed, 0 failed. The process mistake is a bot merge before the laptop section was filled in. The commit itself was not left red.
10. GitHub Actions on `204fd8cd` reports `emotion-v2` / `harness-cpu-stub` and `one-desktop` / `playwright` as success. The emotion harness re-ran here: 38 tests, OK. There is no workflow job named `docker/test-target`. The only `docker run` is `voice-stack-wsl.yml`, which is path-filtered and did not run on that commit. This machine has no Docker client, so that job was not reproduced here.

## What this change did

| Path | Action |
|---|---|
| `crates/galion-v2-live` | Moved to `archive/2026-11-24/crates/galion-v2-live` |
| `voice-stack/horbm-bridge/seam/horbm` | Moved to `archive/2026-11-24/voice-stack/horbm-bridge/seam/horbm` |
| `LOCKS.toml` | No change |
| `SOUL.md` | Still absent. Not created |
| `config.yaml` | Still absent. Not created |

Voice-stack comments and `voice-stack/INTEGRATION.md` that cited the old lane as if it were the running server now cite the archive. `samantha-local` does not gain or lose a dependency. Root `Cargo.toml` members are unchanged. Neither tree was a member.

Not moved, on purpose: `horbm`, `horbm-keys`, `galion-cascade`, `galion-heartbeat`, `galion-console` (the bus), `galion-tensor`, `mne-contract`, `horbm-bridge`, `local-server`, `tts-piper-stream`, `galion-piper-rt`. Open PRs #252 and #253 still need the first set. The running server still needs the last set for PCM.

## KEEP / ARCHIVE / REBUILD

Verdicts are one each. "Stays" means this pull request does not move it. ARCHIVE rows are the only moves.

| Piece | Verdict | This PR | Reason |
|---|---|---|---|
| `horbm/crates/horbm` | KEEP | stays | 7881 brain. Cards, then a fixed miss. No token decode. |
| `voice-stack/horbm-bridge` (linked adapter) | KEEP | stays | `galion-sam-horbm` calls `horbm::Brain::turn`. |
| `voice-stack/local-server` | KEEP | stays | The process operators run. Loopback 7881. |
| `voice-stack/research-gateway` | KEEP | stays | Rust client, `RESEARCH_CAP` of 3 seconds. The call site is the rebuild row below. |
| `voice-stack/asr` | KEEP | stays | Mic turns. Text turns do not need it. Missing model fails closed. |
| `voice-stack/empathy`, `emotion-prosody` | KEEP | stays | Mood knobs on the turn. They do not choose the sentence. |
| `voice-stack/audio-io`, `contract`, `pipeline-orchestrator` | KEEP | stays | Frame helpers on the live stack. |
| `crates/horbm-keys` | KEEP | stays | Mapped int8 arena (PR #249). The turn does not call it yet. |
| `crates/galion-tensor` | KEEP | stays | Scalar and SIMD int8 dot. No new SIMD crate. |
| `crates/galion-heartbeat` | KEEP | stays | 80 ms frame. Stamps read `FRAME_MS`. Not linked by `samantha-local`. |
| `crates/galion-cascade` | KEEP | stays | PR #250 stamps live here. PR #252 still edits this crate. `PhoneLane` inside it is not the 7881 brain. |
| `crates/galion-console` | KEEP | stays | Bus stays. PR #250 edited the turn worker. That worker is the phone lane, not 7881. |
| `tools/symphony/mne-contract` | KEEP | stays | 80 ms intent wire for a later conductor. The mouth and throat crates are not this contract. |
| `crates/galion-sideband` | KEEP | stays | Non-blocking submit and a 3 second cap. 7881 does not call it yet. |
| `LOCKS.toml` `[consent]`, `[formats]` ban, `[licence]` | KEEP | not edited | Deny by default, no candle/ort/tract/burn/llama/ggml in the workspace, MIT, no paid tier. Tests and `check-locks.sh` still require the file. |
| `voice-stack/tts-piper-stream`, `runtimes/galion-piper-rt` | REBUILD | stays | Today's PCM. ONNX and espeak-ng sit in the child. Not the HORBM mouth. Removing them silences 7881. |
| Research `ask` inside `Brain::turn` before the mouth | REBUILD | stays | Speak the card or `I don't know.` on this turn. Post Research off that audio. Same 3 second cap. |
| `CannedLlm`, `PhoneLane` | REBUILD | stays | Rule table and a 15 second phone budget. Not used by 7881. Leave the crate until #252 lands its own edit. |
| `crates/galion-live-demo`, `crates/galion-student-backend` | REBUILD | stays | A second "live" path: heartbeat demo plus an S0 stub that cannot speak (`INFERENCE_IMPLEMENTED = false`). Workspace members. `scripts/run-live-demo.sh` still builds them. |
| `crates/galion-ml-voice` speech stack, Flite dlopen | REBUILD | stays | Cascade fallback when the Piper pack is missing. `gates.yml` tests `--features speech`. Not an MIT synthesizer to wrap. |
| `tools/symphony` mouth, throat, sound-out, and `galion-mne` | REBUILD | stays | Old body. `gates.yml` tests each manifest. `mne-contract` stays. Do not stack HORBM on this integrator. |
| `voice-stack/samantha-voice-clone` | REBUILD | stays | Optional CUDA Chatterbox on 7881. Default stays on Piper. CPU clone is already refused. |
| `voice-stack/one-dot-ui` | REBUILD | stays | Preview page. Same default port number as the speaker. It does not run Piper. Two binders will fight. |
| `tools/cloning-v2` duplicate `chatterbox-rs` and ort scorers | REBUILD | stays | Three workspaces named `chatterbox-rs`, plus a second `galion-emotion` that pulls `ort`. `voice-v2` and `eval-golden` still name those paths. `emotion-v2` CI still runs `emotion-match`. Not safe to move in this pass. |
| `galion-probe` and `LOCKS.toml` `[probe]` | REBUILD | stays | Student-size and GB/s table. The probe binary still reads it. Not the int8 key arena. |
| Piper pin, Chatterbox pin, V4 clock/bands/gates/ui ceremony | REBUILD | not edited | Historical voice locks. `check-locks.sh` and `include_str` tests still enforce them. Retire the checker and the pins in one later change. |
| `crates/galion-v2-live` | ARCHIVE | moved | Unlinked old lane. See `archive/2026-11-24/crates/galion-v2-live/ARCHIVED.md`. |
| `voice-stack/horbm-bridge/seam/horbm` | ARCHIVE | moved | Unlinked stub, duplicate package name `horbm`. See its `ARCHIVED.md`. |
| `SOUL.md`, `config.yaml` | ARCHIVE | absent | Never in git. Do not recreate them as locks. |

Counts: **KEEP 15, ARCHIVE 3, REBUILD 11.** Two ARCHIVE rows are moves. The third is a file that is already gone and must stay gone.

`apps/nebula-voice-v2` is not in the table. It draws a nebula and has no TTS dependency. Its workflow is path-filtered. It is not the mouth and it is not deleted here.

## Minimal HORBM-native voice path

The native path reuses what already works. It does not recode Piper, and it does not invent a synthesizer.

1. `samantha-local` stays the process: loopback 7881, a text turn or a mic turn, 80 ms slices on the socket it already paces.
2. The turn brain stays `horbm::Brain` card retrieval, later the FTS cell from PR #253 or the int8 arena in `horbm-keys`, with no `CannedLlm`, no `PhoneLane`, and no student decode.
3. A miss speaks the card or `I don't know.` on this turn. Galion Research is posted through the existing 3 second client, off that turn's first audio.
4. Speech leaves through the `Mouth` trait `samantha-local` already has. Piper, ONNX, and espeak-ng remain that server's current implementation. They are not the HORBM-native mouth. This tree has no MIT or Apache-2.0 synthesizer to put in their place, so a native `Mouth` speaks text on the socket it already emits and returns no PCM until a real speaker of that licence is chosen. Do not fill the gap with Flite, Chatterbox, or a rewrite of Piper.
5. `Heartbeat::step`, the symphony mouth/throat/sound-out stack, `galion-live-demo`, the archived v2 lane, and the cascade phone worker stay off this path.

What 7881 must keep so it still speaks today: `local-server`, the linked `horbm-bridge`, `horbm/crates/horbm`, empathy and prosody knobs, `tts-piper-stream`, `galion-piper-rt` when that binary is installed, `asr` for mic turns, and the research-gateway library. The energy VAD in `live.rs` is the default ear. `voice-stack/vad` is optional and is excluded from the voice-stack workspace because it links `ort`.

## CI, deploy, merges

Twelve workflows live under `.github/workflows`. None is named deploy. `gates.yml` is the wide Rust job (`cargo test --workspace`, lock check, symphony manifests, clone scripts, phone lane). `voice-host.yml` verifies systemd units and cross-checks `galion-console` on arm64. It does not ship a host. `voice-stack-wsl.yml` runs the installer inside `ubuntu:24.04` only when WSL paths change.

On `204fd8cd` (PR #249 on main), GitHub recorded success for gates, emotion-v2, one-desktop, mouth-gpu, voice-host, voice-stack empathy, linux-package, horbm router, and s06-guard. `nebula-voice-v2`, `s14-windows-install`, and `voice-stack wsl` did not run: path filters. The emotion harness passed again in this workspace (38 tests). The owner's later laptop run of that main is 1034 passed, 0 failed.

`one-desktop.yml` pins `ubuntu-24.04` because Playwright 1.56.1 refuses Chromium on the newer GitHub image. A laptop run that ignores that pin can fail while GitHub stays green.

Duplicate package names still in tree, not moved: `chatterbox-rs` three times under `tools/cloning-v2/pipeline`, `galion-emotion` also under `tools/cloning-v2/scorers/emotion`, `galion-voicecheck` twice, `galion-blend-ref` twice. The archived seam was the second `horbm`.

Do not merge pull requests from the agent. PR #249 is the example: green GitHub checks are not a filled-in laptop section.

## LOCKS.toml and dependencies

No edit. A rewrite that drops a key `scripts/check-locks.sh` asserts will fail `gates`. A rewrite that only changes prose in `[meta]` is safe for the checker and still leaves every test that `include_str!`s the Piper pin and the Rika lines. Do that rewrite when the readers move, not before.

Proposed, not applied:

| Section | Later verdict | Why it waits |
|---|---|---|
| `[consent]` | KEEP | Deny by default, four bits, workplace and education prohibited, wipe on withdraw. Drop only `rika_requires` when the file is rewritten. |
| `[formats]` ban | KEEP | The brain rule F3 already uses. Root workspace complies. `check-no-forbidden.sh` only scans `galion-tensor` plus the root manifest. |
| `[licence]` | KEEP | Perpetual Open MIT, `paid_tiers` false. |
| `[clock]`, `[bands]`, `[gates]`, `[ui]` | REBUILD | 80 ms, the 8-byte wire, barge-in 200 ms, and the one-dot palette are still code. The V4 ceremony around Rika and an OS scheduler is not a retrieval rule. Move the numbers next to the crates, then stop reading them from this file. |
| `[probe]` | ARCHIVE later | Student decoder sizing. `galion-probe` still reads it. |
| `[formats.exceptions.a21_piper]` | ARCHIVE later | onnxruntime 1.31.1, espeak-ng, sunset 2027-02-01. 7881 still resolves this runtime. |
| `[formats.exceptions.v2s14_chatterbox]` | ARCHIVE later | PyTorch clone pin. `galion-consent` still parses `reference_clips`. |

Dependency weight that is real and not removed here:

- `runtimes/galion-piper-rt` is excluded from the root workspace and still pulls `ort` 2.0.0-rc.15 with a CUDA feature. That exclusion is why `cargo tree --workspace` stays clean. The binary is still the preferred Piper child.
- `voice-stack/vad` is excluded for the same `ort` reason.
- The three `chatterbox-rs` trees are outside the root workspace and each carries its own `ort` pin. They are not what 7881 speaks with.
- `tools/cloning-v2/scorers/ecapa/.cargo/config.toml` sets `target-cpu=native` and an absolute library path from another machine. That is not portable Rust. Fix it when that scorer is rebuilt, do not copy the path.

A deploy note and an old demo note still contain a VPS address and a tunnel hostname. They were not copied into this file.

## Ordered next steps

1. Laptop test this branch. Do not merge it from the agent, and do not enable auto-merge.
2. Review PR #253. Treat PR #251 as superseded. Do not merge #251.
3. Keep PR #252 from making Piper part of the brain. A heuristic pick may replace `CannedLlm` on the phone lane. The 7881 mouth stays behind `Mouth`.
4. Move the Research `ask` off the turn that starts the mouth. Speak the local clause first.
5. Add a `Mouth` implementation that does not spawn Piper. Text on the existing socket is enough until a MIT or Apache-2.0 speaker is actually available. Leave `PiperMouth` in the binary so today's server still speaks.
6. When 7881 no longer looks up `galion-piper-rt`, archive that runtime and the `a21_piper` pin together, and edit `check-locks.sh` in the same change.
7. After `tools/symphony/voice-v2` and `voice-stack/eval-golden` stop pointing at `tools/cloning-v2/pipeline/cbx-v2*`, archive the three `chatterbox-rs` copies. Same for the ort scorers once `eval-golden` stops joining those paths.
8. Retire `galion-live-demo` only after `scripts/run-live-demo.sh` and `galion-gates/lv-live-demo.sh` no longer build it. `galion-heartbeat` stays.

## Laptop test

Run on the GALION-STUDIO checkout. Rust 1.85 or newer, `x86_64-pc-windows-msvc`. Cargo stays off `C:`. Do not set `RUSTFLAGS` to `-C target-cpu=native`. Do not download weights. This does not merge.

```powershell
cd G:\galion\work\galion-one-voice
git fetch origin
git checkout cursor/fp-first-principles-audit-cf9f
$env:CARGO_TARGET_DIR = "G:\galion\samantha\target-fp-audit"
$env:CARGO_HOME = "G:\galion\samantha\cargo"
if (Test-Path crates\galion-v2-live) { throw "galion-v2-live is still at the old path" }
if (Test-Path voice-stack\horbm-bridge\seam\horbm) { throw "unlinked horbm seam is still at the old path" }
if (-not (Test-Path archive\2026-11-24\crates\galion-v2-live\ARCHIVED.md)) { throw "v2 lane archive note missing" }
if (-not (Test-Path archive\2026-11-24\voice-stack\horbm-bridge\seam\horbm\ARCHIVED.md)) { throw "seam archive note missing" }
bash scripts/check-locks.sh
cargo test --workspace
cargo test -p horbm-keys
cargo test -p horbm-keys --features simd-avx2,simd-avx512
cargo test --manifest-path voice-stack\Cargo.toml -p galion-sam-horbm -p samantha-local-server
cargo test -p galion-cascade --features cascade --lib
```

`bash` is Git Bash or the WSL bash already used for `scripts/check-locks.sh`. If that script is run with `python` on Windows instead, the gate is the same file: it must print `OK: all Eureka locks hold`.

The voice-stack line does not build `voice-stack/vad` (excluded, `ort`) and does not build `runtimes/galion-piper-rt`. Build the Piper runtime only on a machine that already has the ONNX pack and espeak-ng data. Do not download them for this test.

Emotion harness, no weights, from the repo root:

```powershell
$env:EMOTION_V2_BACKEND = "stub"
$env:HF_HUB_OFFLINE = "1"
$env:PYTHONPATH = "tools/emotion-v2"
python -m unittest discover -s tools/emotion-v2/harness/tests -v
```

Playwright stays the GitHub job `one-desktop` on `ubuntu-24.04`. Do not treat a local failure on a newer image as a regression of this branch unless `npx playwright test` in `e2e/one` fails on that same image.
