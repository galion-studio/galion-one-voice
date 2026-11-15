# Swarm 10 — persona-v2v admin-brief (Lane C, C4)

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. The spelling is **Galion**. The product is Galion Voice.

**Unsigned.** Prepared by Swarm 10 worker `admin-brief`. This file is not a signature. No agent ticked a box in `docs/galion-voice-v2/ADMIN-UI-CHECK.md`. No agent wrote the sign-off date. The sign-off name, when Maciej signs, is **Maciej Grajczyk**.

| | |
|---|---|
| Lane plan | [PR #150](https://github.com/galion-studio/galion-one-voice/pull/150) `Swarm 10 persona-v2v plan`, branch `cursor/persona-v2v-plan-6a84` |
| Base | `origin/main` `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0` |
| Measurements | 2026-11-15 22:06:00–22:06:02 +01:00 |
| Note written | 2026-11-15 22:08 +01:00 |
| Host | `cursor`, Linux 6.12.113+ x86_64, 4 CPUs. `rustc 1.85.0 (4d91de4e4 2025-02-17)` for the loop (the image default was 1.83.0; the 1.85.0 toolchain was installed in this VM so the clock tests match the `gates` workflow). |
| What changed | This notes file only. `ADMIN-UI-CHECK.md`, `LOCKS.toml`, `SOUL.md`, `config.yaml`, `scripts/clone/REFS.sha256`, the one-dot page, and the hard-loop script were not edited. |

`/workspace/voice-next/NEXT-ITERATION.md` was read from the uploaded baseline. It is not in this commit.

## What was checked

- `docs/galion-voice-v2/ADMIN-UI-CHECK.md` on `8980e1a`. Every checklist box is still `- [ ]`. The date line is still `______`.
- `docs/galion-voice-v2/HARD-LOOP.md`, `HARD-REAL.md`, and the CPU-only HARD-PASS section of `NANO-BACKEND.md`.
- `scripts/eval/v2-hard-loop.sh` (self-check, then the full loop). `GALION_HARD_HTTP` was not set. `GALION_HARD_REQUIRE` was not set. `GALION_HARD_SKIP_CARGO` was not set. `GALION_CLONE_STUB_PCM` and `GALION_CLONE_STUB_FRAMES` were unset.
- A separate optional Nano worker probe, labelled below. It is not a HARD-PASS gate.
- `LOCKS.toml` `[formats.exceptions.v2s14_chatterbox]`: `reference_clips = true`, `ref_clip_decision = "t623u"`, scope text "CC0 / public-domain LibriVox donors only". Read only.
- `scripts/clone/REFS.sha256`: the active rows are the t623u CC0 donors (`samantha` `e4bc60c33f697108cf5585dead213da1065979010f5be1d513574893584a4641`, `jarvis` `60ad5457dac31af915a9679760705296f9aad17e15ee84885385509b58b8f6b6`). The t273 rows are comments and say "do not re-enable them." Hashes in those comments: Samantha `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693`, Jarvis `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60`. `CLONE-LANDING.md` still names those t273 hashes. The file was not rewritten.
- Nerve record: `docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md` §4, and `crates/galion-console/assets/v1-screen.html` (read, not edited). No second pane was added.
- `docs/galion-voice-v3/VERIFICATION.md` Part A1–A2. Both clone boxes are still unchecked. That file was not edited. No V3 code.

Lane C siblings C1, C2, and C3 had no open pull request at 2026-11-15 22:07 +01:00. The open Swarm 10 pull requests then were #150 (this lane's plan), #151, #152, #153, #154, and #155. Their results are not invented here.

## Hard-loop report

Command, on this host, from the repo root, CPU, no stub flags:

```text
scripts/eval/v2-hard-loop.sh --self-check
scripts/eval/v2-hard-loop.sh
```

Self-check exit 0. It printed: `classifiers held. This is not a product verdict.` and `HARD-PASS of the loop: refused`.

Full loop exit 1. Wall from the log header to `END`: 2026-11-15 22:06:00 to 22:06:02 +01:00. The harness also wrote `target/hard-loop/latest.txt` (gitignored). The text below is the stdout of that run.

```text
Galion Voice hard loop
CPU only on this box. Piper is the real mouth. Flite, clone protocol PCM, and cloud TTS are not.
HARD-PASS requires every real gate. A missing component is HARD-FAIL, named on the report line.
GALION_CLONE_STUB_PCM and GALION_CLONE_STUB_FRAMES never yield HARD-PASS.
A gate status of met is that gate only.
gate tree-hygiene: failed — tracked audio or weight blobs: docs/galion-voice-v2/evals/listening-set/emotion-sweep/jarvis-angry-after-distress.wav, docs/galion-voice-v2/evals/listening-set/emotion-sweep/jarvis-angry-after-joy.wav, docs/galion-voice-v2/evals/listening-set/emotion-sweep/jarvis-angry-rest.wav, docs/galion-voice-v2/evals/listening-set/emotion-sweep/jarvis-anxious-after-distress.wav, docs/galion-voice-v2/evals/listening-set/emotion-sweep/jarvis-anxious-after-joy.wav, docs/galion-voice-v2/evals/listening-set/emotion-sweep/jarvis-anxious-rest.wav, docs/galion-voice-v2/evals/listening-set/emotion-sweep/jarvis-calm-after-distress.wav, docs/galion-voice-v2/evals/listening-set/emotion-sweep/jarvis-calm-after-joy.wav. Not HARD-PASS.
gate ref-refuse: met — An unallowlisted reference exited 3 with no PCM. The rights hold holds. This is not a generate pass.
gate ref-hashes: absent — GALION_CLONE_LOCAL_REFS=1 and GALION_CLIP_STORE are both required. LOCKS.toml is not read and reference_clips is not flipped. Not HARD-PASS.
gate weight-pins: absent — Pinned Nano safetensors are not on disk. Fetch was not run. Not HARD-PASS.
gate hf-fetch: met — GALION_CLONE_WEIGHTS_SOURCE=hf remains a fetch path. This harness does not download.
gate piper-pcm: failed — missing: Piper CPU runtime or en_US-ljspeech-medium pack on this box. Not downloaded. Not a clone stub. Not cloud TTS. Not HARD-PASS.
gate ttfa-streaming: failed — missing: real Piper ttfa_ms. No number is claimed. Not HARD-PASS.
gate persona-http: absent — no live server at http://127.0.0.1:7878. Not HARD-PASS.
gate live-one-dot: absent — no live server at http://127.0.0.1:7878. Not HARD-PASS.
gate real-sampling: met — 24 kHz, 1920 samples, 80 ms EmotionClock frames, Heartbeat::step allocs=0, late=pause under the counting-allocator load.
gate late-pause: met — Heartbeat::step late frame is paused, me onset stays closed, and a short PCM frame pauses. No catch-up burst.
gate real-llm: absent — No Hermes reply and no student .gq reply. Not HARD-PASS.
gate real-thinking: absent — No nerve or thought trace on disk. The Admin UI check stays unsigned. Not HARD-PASS.
gate real-persona: absent — Still required: GALION_CLONE_LOCAL_REFS and GALION_CLIP_STORE hashes; galion-emotion on the live feed; Jarvis hub texture at playback. Not HARD-PASS.
gate full-chain: absent — Unmeasured: vosk, instruction-floor, piper, streaming-pcm, emotion-clock, playback, ttfa_ms. Not HARD-PASS.
overall: HARD-FAIL
HARD-PASS of the loop: refused
report-line: HARD-FAIL ttfa_ms=absent allocs=0 late_frames=proved persona_http=absent dot=absent product_claim=false missing=tree-hygiene,ref-hashes,weight-pins,piper-pcm,ttfa-streaming,real-llm,real-thinking,real-persona,full-chain,persona-http,live-one-dot
```

JSON fields from that same stdout: `overall` `HARD-FAIL`, `hard_pass` false, `product_claim` false, `measured` false, `engine` null. No `ttfa_ms` number in the JSON.

`overall: HARD-FAIL`. This run did not print `STUB-PASS` and did not print `HARD-PASS` as the overall line. `HARD-LOOP.md` still lists `STUB-PASS` as one of the three overall words. That page was not edited. The script on this tip (`overall_verdict`) returns `HARD-PASS` only when every required gate is `met`, and `HARD-FAIL` otherwise.

### Counts behind the truncated tree-hygiene line

`git ls-files '*.wav'` on this tip: **84** tracked wav files, 15 574 756 bytes.

| Directory | Files | Bytes |
|---|---|---|
| `docs/galion-voice-v2/evals/listening-set/emotion-sweep` | 46 | 7 814 076 |
| `docs/galion-voice-v2/evals/listening-set/piper-premium/samples` | 36 | 7 606 992 |
| `specs/fixtures` | 2 | 153 688 |

The harness allowlist is the two files in `specs/fixtures`. It prints only the first eight other paths. The other 82 tracked wavs are why `tree-hygiene` is `failed`. They were already on `8980e1a`. They were not deleted and the allowlist was not widened.

### Clock re-run after the harness compile

The harness compiled `galion-heartbeat` during that two-second wall (test binaries mtime 2026-11-15 21:06:02 UTC). A second invocation of the same filters, already compiled:

```text
cargo test -p galion-heartbeat --test heartbeat -- late_ --test-threads=1
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.00s
exit 0
```

The three tests that ran: `feeling_late_frame_holds`, `late_frame_is_a_pause_no_extra_trigger_no_burst`, `short_pcm_frame_is_treated_as_late_pause`.

```text
cargo test -p galion-heartbeat --test zero_alloc -- step_allocates_nothing_with_rika_on --test-threads=1
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.01s
exit 0
```

That re-run is the unit clock. It is not a live `/dot` picture and not a Piper TTFA. `ttfa_ms` on this run is `absent`. Not measured as a product clock.

`scripts/check-locks.sh`: `OK: all Eureka locks hold (LOCKS.toml)`, exit 0.
`scripts/check-no-forbidden.sh`: `OK: forbidden-format + forbidden-runtime gates pass`, exit 0.
`git diff` on `LOCKS.toml`, `SOUL.md`, and `config.yaml` after the run: empty.

`cargo test --workspace`: **not run** locally. `GALION_HARD_HTTP=1`: **not run**. `GALION_HARD_REQUIRE=1`: **not run**. `scripts/eval/flash-ttfa.sh`: **not run**. `scripts/eval/v2-persona-selftest.sh`: **not run**. `scripts/run-v1-screen.sh`: **not run** (C2).

### Optional Nano gate (not a HARD-PASS)

Separate from the hard loop. No `GALION_CLONE_STUB_*`, no `GALION_CLONE_LOCAL_REFS`, no `GALION_CLIP_STORE`, no fetch. One JSON line per persona into `scripts/clone/chatterbox_worker.py`.

| Persona | Exit | Wall | stdout | stderr (exact) |
|---|---|---|---|---|
| Samantha | 2 | 0.025 s | 0 bytes | `galion-clone-cpu: threads=1 device=cpu denoise=false` / `galion-clone-budget: session_vcpu=1 threads=1 burst_max=4 ram_gb=0.65-1.15 device=cpu model=nano gguf=false` / `galion-clone-absent: pinned safetensors missing` |
| Jarvis | 2 | 0.024 s | 0 bytes | same three lines, `galion-clone-absent: pinned safetensors missing` |

`python3` on this host has no `torch` module and no `chatterbox` module. `weights/clone/` is absent (only `weights/.gitignore`). Tokenizer: absent. Vosk model: absent. Piper binary and `weights/tts/en_US-ljspeech-medium.onnx`: absent. Nothing was downloaded.

This probe did not reach the reference allowlist. It is not the NEXT-ITERATION generate.

## C5 status

**BLOCKED.**

Witness required by `VERIFICATION.md` A1 and A2: a person hears Samantha and Jarvis from the local clone worker, self-offline, and the log names the clone backend. Piper labelled as Piper is a negative for those boxes. Both boxes in the tree are still unchecked. This note does not check them and does not sign a V3 start.

On this host the optional probe above produced 0 PCM and `galion-clone-absent: pinned safetensors missing`. No person heard a clone. No film-voice claim.

The original-ref generate in NEXT-ITERATION (2026-11-15, host `grok-bot-vm-511777615`, commit `8980e1a`) was **not re-run here**. The t273 wavs and the pinned safetensors are not on this machine. That write-up, cited and not recycled as a result of this VM, says both personas exited 3 in 0.032–0.033 s with 0 PCM and stderr `galion-clone-refuse: reference sha256 is not allowlisted for <persona>`, because `REFS.sha256` allowlists the t623u donors. A0 stays **BLOCKED-OWNER**. This lane did not flip `LOCKS.toml` or `REFS.sha256`.

C1, C2, and C3 had not opened pull requests when this note was written, so their session logs are not folded in. C5 stays BLOCKED on A0 even if those notes arrive later, until a clone backend actually speaks from the refs Maciej allows.

The speech stack's documented fallback, and the owner deploy default until a clone mouth is live, is Piper `en_US-ljspeech-medium`. This hard loop did not speak it: `piper-pcm` is `failed` because the pack is not on this box.

## Unsigned Admin UI check brief

For Maciej Grajczyk. Agents do not sign. Every box below stays open. The date stays blank. `STUB-PASS` was not the overall line. `HARD-FAIL` is not a signature. A missing server is a gap in the report, not a tick.

The one-dot page does not draw an LLM thought pane, a menu, or a transcript. The nerve record in `OPENMIND-NERVE-CORTEX.md` is NDJSON after the frame (`voice.frame_miss`, `voice.backend_up`, `voice.fallback`), with no PCM and no caller transcript. No nerve process was running here. `GALION_NERVE_LOG` and `GALION_THOUGHT_LOG` were unset. The harness gate `real-thinking` is `absent` and its detail says the Admin UI check stays unsigned. The thought box stays open: there is no thought trace, and there was no live reply to compare with the instruction floor.

`v1-screen.html` is one canvas (`data-mouth="piper"`, `data-film-voice="false"`, `data-rights="hold"` on the markup). At width ≥ 1100px, when the path is not `/one`, `#decide` shows three session yes/no questions. That box is not a nerve pane and not an LLM thought. It was left as it is. No chrome was added. `LOCKS.toml` `[ui]` stays one dot.

| Checklist (all open) | Evidence from this run |
|---|---|
| Live generations audible (Samantha and Jarvis), from Nano, not from the protocol stub and not from Piper labelled as the film | Not heard. Nano probe: absent weights, 0 PCM. Piper pack absent, so Piper did not speak either. |
| LLM thought / nerve pane visible and accurate (no fake interim) | No trace file. Page has no thought pane. Box stays open. |
| Emotion and colour stay with the turn, and the words are not canned empathy | Not run. No server. Not measured. |
| One-dot mark: speak wins; pause holds | The mark was not on screen (`live-one-dot` absent). Pause holds in the heartbeat unit test (`late-pause` met, re-run above). The picture was not observed, so the box stays open. |
| Failures surface clearly (no silent hang) | The harness named the missing pieces and exited 1. The screen was not up, so a UI failure surface was not observed. |
| Same machine: `scripts/eval/v2-hard-loop.sh` overall line is `HARD-PASS` | Overall line is `HARD-FAIL`. `HARD-PASS of the loop: refused`. `product_claim=false`. `ttfa_ms=absent`. |
| Sign-off: Maciej Grajczyk · date ______ | Blank. No agent writes this date. |

`ADMIN-UI-CHECK.md` in the tree is unchanged and remains the page he signs, on a machine where he has heard the generations.

## CI

Not yet recorded for this commit. The pull request checks are filled in after they finish. Workflows expected on a docs-only pull request: `gates`, `emotion-v2`, `one-desktop`. `nebula-voice-v2` stays absent unless nebula paths change; this commit does not touch them.

On the base, the lane plan records `gates`, `emotion-v2`, and `one-desktop` success on `8980e1a` (runs 43660665688, 43660665722, 43660665724). That is the base, not this head.

## Decisions for Maciej

1. **A0.** Re-open the t273 originals on the live clone path, or state that main stays on the t623u CC0 pins and that symphony landing / self-eval B must be re-stated. This pull request does not choose and does not edit the lock or `REFS.sha256`.
2. **Sign-off.** `ADMIN-UI-CHECK.md` stays unsigned until he has heard live generations. This host's hard loop is `HARD-FAIL`. The date stays blank.
3. **Tracked eval wavs.** 82 wav files under `docs/galion-voice-v2/evals/listening-set/` (emotion-sweep and piper-premium samples) are in git beside the two allowed fixtures. They fail `tree-hygiene` on every full hard loop of this tip. Deleting them or widening the allowlist was not done here.
4. **`/one` persona.** The plan's candidate (keep `data-persona="samantha"` and `data-mouth="piper"`) is the orchestrator's, not this pull request.
5. **Mouth.** The live mouth stays Piper plus the Samantha persona until a clone is actually live. C5 is BLOCKED, so that switch is not proposed.

## Open questions

- C1, C2, and C3 notes were not available at 22:07 +01:00. If they land with measurements, this C5 sentence still stays BLOCKED unless those notes include a clone backend speaking. They did not exist to quote.
- `HARD-LOOP.md` still describes an overall word `STUB-PASS`. The script that ran printed `HARD-FAIL`. Left for a docs pass outside this brief.

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
