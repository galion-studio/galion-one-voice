# Emotion blend (Cloning V2)

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Symphony section. Index: [README.md](README.md).

Opt-in conditioning prep for Chatterbox Turbo ONNX (cbx) voice profiles.
Experimental. **OFF by default.** No change to Piper live TTS. No edit of
`LOCKS.toml` / `SOUL.md` / `config.yaml`. Voice profiles and WAVs stay off git
(consent scope: local zero-shot conditioning). Jarvis production profile stays
unchanged. This landing does not train Piper.

## Recognition (Rust-first)

Recognition on this path is Rust. Window scores and the live inbound detector
both live in Rust. A score this landing did not measure is **BLOCKED**.

### Window scorer (blend builders)

Both builders start one sequential Rust process, two threads, and read its
JSON. The command is `score --threads 2 --out <json> <window.wav…>`.

| Caller | Flag | Binary role |
|---|---|---|
| Montage `tools/cloning-v2/pipeline/emotion-blend-ref` | `--scorer` | Rust fp32 `galion-emotion` executable. Metadata records `scorer: galion-emotion fp32, threads=2`. |
| Coherent rotation `tools/cloning-v2/emotion-blend/global-fix/refbuilder` (`galion-blend-ref`) | `--emotion-scorer` | Same `score --threads 2` invocation. A failed scorer writes no prepared reference. |

Autocheck resolves that binary at
`tools/cloning-v2/scorers/emotion/target/release/galion-emotion`
(`Paths::discover` in `tools/cloning-v2/emotion-blend/autocheck`). The
`tools/cloning-v2/scorers/` tree is absent from this checkout, so a fresh
window score on this machine is **BLOCKED**.

The JSON the builders require, when a scorer run exists, carries SER
probabilities (`neu` / `hap` / `ang` / `sad`), AVD (`arousal` / `valence` /
`dominance`), and acoustic proxies. Those fields are the scorer contract.
Until a run exists on this machine, the score table below stays **BLOCKED**.

### Live inbound detector

The voice-path detector is the Rust crate `galion-emotion`.

- `GatedDetector` checks `ConsentGate` before it reads PCM. Default is deny.
- The runtime estimator is `LoudnessEstimator` (RMS and zero-crossing rate),
  exposed as `EnergyStubDetector`. Valence from this estimator is unknown.
- `EstimatorKind::Learned` is named. The learned head is not in this crate.
- `ReferenceEstimator` replays an offline label track. It loads no model.

`galion-emotion` README states the crate is Rust-first and that the shipped
detector is a stub. No separate speech-emotion model is linked into that crate.

## Scores

This page records a figure only after this checkout measured it. WER, ECAPA,
MOS, and accuracy were not measured here. Emotion distance was not measured
here. Prior write-ups stay in `tools/cloning-v2/emotion-blend/analysis/`
and `tools/cloning-v2/emotion-blend/global-fix/`. This page's own rows are
the **BLOCKED** table below.

| Score | Result | Why it is blocked |
|---|---|---|
| WER | **BLOCKED** | `tools/cloning-v2/scorers/quality/bin/galion-wer` is absent. Not run. |
| ECAPA | **BLOCKED** | `tools/cloning-v2/scorers/ecapa/target/release/galion-ecapa` is absent. Not run. |
| MOS | **BLOCKED** | No MOS log was produced for this landing. |
| Accuracy | **BLOCKED** | No accuracy log was produced. The live detector is the loudness stub. |
| Emotion distance | **BLOCKED** | Needs the missing Rust emotion-scorer output. Not run. |

Promotion of `own-samantha` or any coherent-rotation / montage candidate is
**BLOCKED** until those rows are measured on this machine. Both entry points
refuse protected names `own-samantha` / `own-jarvis` and refuse an existing
profile.

## Root cause

The cbx speech-encoder graph, as traced in
`tools/cloning-v2/emotion-blend/global-fix/REPORT-coherent.md`, feeds LM
conditioning from the first **6 s** of 16 kHz `audio_features` (96000 samples)
and prompt tokens plus speaker features from the first **10 s** of 24 kHz
audio (240000 samples). Speaker embeddings still pool the full input.
Expressive moments after that window never reach LM conditioning on a straight
encode of the merged reference.

Evidence paths (the numeric tables stay in those files):
`tools/cloning-v2/emotion-blend/analysis/FINDINGS.md`,
`tools/cloning-v2/emotion-blend/global-fix/REPORT-coherent.md`.

## Two opt-in paths (do not conflate)

| Path | Entry | What it does |
|---|---|---|
| Coherent rotation (`--blend-preserve`) | `tools/cloning-v2/pipeline/bin/cbx-clone-voice --blend-preserve` | Cyclic start so the first 6 s match whole-source SER/AVD/prosody mean and population SD from the Rust scorer. Full duration kept for identity. Builder: `emotion-blend/global-fix/refbuilder` (`galion-blend-ref`). |
| Montage (`--emotion-blend`) | `tools/cloning-v2/pipeline/bin/cbx-add-voice --emotion-blend` | Diverse ≤6 s montage from Rust window scores; encodes original and montage; keeps original speaker tensors byte-identical via `cbx-blend`. Builder: `pipeline/emotion-blend-ref`. |

Shared render serialisation: `emotion-blend/render.lock` and `/tmp/cbx-render.lock`,
nice 10, 2 threads.

Automatic check, when the scorer binaries exist:
`tools/cloning-v2/emotion-blend/autocheck` (`galion-voicecheck`).
`require_tools` fails closed while `galion-wer`, `galion-ecapa`, or the Rust
emotion scorer is missing. That closed failure is the **BLOCKED** gate.

## Gate

**PASS** when a Rust scorer run on this machine writes WER, ECAPA, MOS, and
accuracy into a log, and a promotion decision is taken from that log.

**FAIL** when that log exists and a guard named in the log misses.

**BLOCKED** while `tools/cloning-v2/scorers/` is absent or no log from this
landing contains those four scores. That is the state of this landing
(2026-11-06, Europe/Zagreb).

## Build (box)

```bash
cd tools/cloning-v2
nice -n 10 cargo test --offline -j 2 --manifest-path pipeline/emotion-blend-ref/Cargo.toml
nice -n 10 cargo test --offline -j 2 --manifest-path emotion-blend/autocheck/Cargo.toml
nice -n 10 cargo build --offline -j 2 --manifest-path emotion-blend/global-fix/refbuilder/Cargo.toml
# cbx / cbx-blend (timbre fork) needs the local ORT/cbx deps already on the box:
nice -n 10 cargo build --release --offline -j 2 --manifest-path pipeline/cbx-v2-timbre/Cargo.toml
```

These commands build and test the Rust builders. WER, ECAPA, MOS, and
accuracy stay **BLOCKED** until a separate measured log exists.

## Related

- [CLONE.md](CLONE.md) — Nano/Turbo clone landing
- [CBX-CLONE-WORKER.md](../CBX-CLONE-WORKER.md) — separate cbx clone worker (loopback)
- Tooling tree: `tools/cloning-v2/`
- Live detector: `galion-emotion/` (`detect`, `loudness`)

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
