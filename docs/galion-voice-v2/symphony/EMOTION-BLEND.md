# Emotion blend (Cloning V2)

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Symphony section. Index: [README.md](README.md).

Opt-in conditioning prep for Chatterbox Turbo ONNX (cbx) voice profiles.
Experimental. **OFF by default.** No change to Piper live TTS. No edit of
`LOCKS.toml` / `SOUL.md` / `config.yaml`. Voice profiles and WAVs stay off git
(consent scope: local zero-shot conditioning).

## Root cause

The cbx speech-encoder ONNX graph feeds LM conditioning from the first **6 s**
of 16 kHz `audio_features` and prompt tokens from the first **10 s** of 24 kHz
audio. Speaker embeddings still pool the full input. Expressive moments after
that window (Samantha happy evidence ~9.9–12.4 s) never reach LM conditioning
on a straight encode of the merged reference.

Evidence and measurements: `tools/cloning-v2/emotion-blend/analysis/FINDINGS.md`,
`tools/cloning-v2/emotion-blend/global-fix/REPORT-coherent.md`.

## Two opt-in paths (do not conflate)

| Path | Entry | What it does |
|---|---|---|
| Coherent rotation (`--blend-preserve`) | `tools/cloning-v2/pipeline/bin/cbx-clone-voice --blend-preserve` | Cyclic start so the first 6 s match whole-source SER/AVD/prosody mean+SD; full duration kept for identity. Builder: `emotion-blend/global-fix/refbuilder` (`galion-blend-ref`). |
| Montage (`--emotion-blend`) | `tools/cloning-v2/pipeline/bin/cbx-add-voice --emotion-blend` | Diverse ≤6 s montage; encodes original+montage; keeps original speaker tensors byte-identical via `cbx-blend`. Builder: `pipeline/emotion-blend-ref`. |

Both refuse protected names `own-samantha` / `own-jarvis` and existing profiles.
Shared render serialisation: `emotion-blend/render.lock` and `/tmp/cbx-render.lock`,
nice 10, 2 threads.

## Samantha promotion (2026-11-05)

Owner/admin accepted coherent-rotation Samantha for production on this box and
the clone worker voice store (MOS tradeoff 4.13 → ~3.88).

| Voice | n | WER | ECAPA | MOS | Emotion distance | Gate |
|---|---:|---:|---:|---:|---:|---|
| own-samantha merge-v2 (prod before) | 12 | 0.018 | 0.7057 | 4.1323 | 0.4651 | baseline |
| own-samantha-v2-blendfix-coherent | 12 | 0.01455 | 0.7081 | 3.8757 | 0.437251 | PASS WER≤0.018, ECAPA>0.7057 |

Jarvis coherent-rotation failed its own guard (WER 0.02963 / ECAPA 0.7303) and
was **not** promoted. Jarvis production profile remains unchanged.

Timbre-match stays 0.5; T0/fp32 defaults. Automatic check:
`tools/cloning-v2/emotion-blend/autocheck` (`galion-voicecheck`).

## Build (box)

```bash
cd tools/cloning-v2
nice -n 10 cargo test --offline -j 2 --manifest-path pipeline/emotion-blend-ref/Cargo.toml
nice -n 10 cargo test --offline -j 2 --manifest-path emotion-blend/autocheck/Cargo.toml
nice -n 10 cargo build --offline -j 2 --manifest-path emotion-blend/global-fix/refbuilder/Cargo.toml
# cbx / cbx-blend (timbre fork) needs the local ORT/cbx deps already on the box:
nice -n 10 cargo build --release --offline -j 2 --manifest-path pipeline/cbx-v2-timbre/Cargo.toml
```

## Related

- [CLONE.md](CLONE.md) — Nano/Turbo clone landing
- [CBX-CLONE-WORKER.md](../CBX-CLONE-WORKER.md) — separate cbx clone worker (loopback)
- Tooling tree: `tools/cloning-v2/`

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
