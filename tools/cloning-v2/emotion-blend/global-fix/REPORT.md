# Global conditioning reference fix — coherent prefix variant

Status: implementation built; Samantha 12-line gate PASS, Jarvis validation in progress. All times Europe/Zagreb (CET). Experimental **OFF by default**; no production profile/default is edited. This is independent of the parallel `A-global` montage variant / `cbx-add-voice`.

## Findings first

`../analysis/FINDINGS.md`, `profile-{samantha,jarvis}.json` (extended autocheck schema) and `profile-*-detailed.json` were published at 22:38 CET. Original 2.5s windows and all 12 merged-production lines have fp32 SER/AVD and Rust acoustic metrics; Rust contour supplements supply F0 slope, Hz SD and min/max.

Samantha original: neutral 4/7, angry 3/7 hard window labels; happy evidence peaks at 10–12.5s, P(hap)=0.3631. No “sweet/playful” detector exists. **Hard-label entropy does not collapse** (0.9852→1.2807 bits comparing windows vs clone lines). Line-level descriptive flattening is between-unit F0-mean SD (20.03→8.67 Hz), energy-range SD (2.15→0.92 dB), and arousal variance (~0.003577→0.002485). **Equal-window cross-check substantially qualifies this:** Samantha arousal SD instead grows 0.059805→0.062946, F0-mean SD only falls 20.03→15.96 Hz, energy-range SD only falls 2.15→1.93 dB, hard-label entropy rises 0.9852→1.2088. The robust descriptive changes are narrower pitch range (8.63→7.19 st), fewer pauses (0.154→0.070), faster speech, NOT an across-the-board emotion/entropy collapse. Different texts/durations are a confound, so these are descriptive, not a causal decomposition. Jarvis neutral hard labels remain 6/6→12/12; valence/arousal lower, while F0 range grows (not flattened).

## Code/graph root-cause audit

- `pipeline/cbx-v2-timbre/src/audio.rs:61–68`: encode prep is sinc resampling to 24k + true-peak limit. **No loudness normalization, trim, denoise or declick in the production reference encode path.** Those options exist in rejected refprep experiments, not this production path.
- `src/chatterbox.rs:254–300`: Rust passes the entire prepared reference into ONNX and serializes the four outputs, without an extra Rust temporal average. Voice profile is static and reused for all texts.
- Actual cached fp32 speech_encoder graph `/Slice_5`: resampled 16k **first 96000 samples = 6s** feed `audio_features` (LM conditioning). `/Slice`: 24k **first 240000 samples = 10s** feed `audio_tokens` and `speaker_features`. `speaker_embeddings` derives from full input and `/ReduceMean` speaker pooling. Traced ancestry, not inferred from a help string. Graph listing: `encoder-graph-audit.txt`; resolved constants, ancestry and graph SHA256 in `encoder-graph-values.json`; independent resolved evidence in `../A-global/encoder-graph-evidence.json`.
- Thus default LM conditioning never observes Samantha 10–12.5s happy-evidence window or late >12.5s angry mode. Merged r2 rotation starts at 2.25s: LM sees approximately 2.25–8.25s, still excluding the bright 10–12.5s interval. This is a concrete selection bottleneck. It **does not prove** the learned model can reproduce every source emotion if shown more regions.
- Full-input speaker pooling is for identity and is intentionally many-to-one. Static conditioning and LM/decoder learned priors remain architectural bottlenecks outside this reference fix; changing them would be a synthesis-stack change.
- Temperature 0 greedily selects LM tokens, but decoder contains unseeded RandomNormalLike. T0 is not byte-deterministic and cannot be assumed to remove expressiveness by itself. Existing T0.8 evidence does not establish a universal remedy.
- Timbre-match is post-decoder EQ. It cannot recover unseen reference sequences; it changes SER/identity modestly and is not the source of first6s loss.

## Cheap existing-render ablations

From `emotion-fix/scores/sd_r2_t012_*`, `sd_r2_up12_*`, merged scores, and scoreboard (same 12 lines):

| Samantha condition | ECAPA | WER | Emotion distance |
|---|---:|---:|---:|
| r2 @ T0, before timbre | 0.7082 | 0.0176 | 0.4675 |
| r2 @ T0.8, before timbre | 0.7032 | 0.0684 | 0.4828 |
| r2 @ T0, timbre 0.5 (production) | 0.7057 | 0.0176 | 0.4651 |

Higher temperature worsens WER here, and post EQ slightly changes emotion distance without repairing conditioning selection. These are existing independent decodes, not controlled identical-noise causal estimates. Jarvis timbre 0→0.5 preserves WER 0.0185, identity 0.7328→0.7331, emotion distance 0.5113→0.5020. Refprep's phrase selection/HF/normalization bundle fails the previous shared acceptance gate and is not promoted.

## Built (shared reference stage, not synthesis)

- `refbuilder/`: standalone Rust `galion-blend-ref`, no ONNX dependency itself; it invokes the existing fp32 Rust emotion scorer once on 2.5s source windows.
- `pipeline/bin/cbx-clone-voice`: new safe clone entrypoint, unique name/work directory required; protected production names and existing voices refused; serializes scoring/encoding with `emotion-blend/render.lock`; CPU, nice 10, two threads.
- `--blend-preserve`: optimize the overlap-weighted **mean and population SD** of 4 SER + 3 AVD + energy/pause/rate/tilt descriptors in the first6s versus the entire source. Only quiet20ms boundary cuts considered. Choose one coherent cyclic reference rotation, never a phoneme/token montage; keep full duration for full-input identity conditioning. Require ≥5% objective improvement, otherwise leave start unchanged. Preserve relative loudness, pitch, rate and pauses; only a 5ms fade at the original end/start join avoids a discontinuity.
- Mode is global/data-driven, no Samantha/Jarvis constants or manual emotional targets. It preserves source distribution approximately under finite window resolution; it cannot guarantee model-level temporal emotion reproduction.
- Flag omitted: **byte-copy input WAV** (verified cmp on both Samantha and Jarvis; enabled short-reference no-op also identical; existing outputs refused, see `safety-tests.txt`), no model/scorer load in builder. Existing render wrapper, voices and defaults remain untouched.

### Enable

```sh
nice -n 10 pipeline/bin/cbx-clone-voice \
  --name own-NEW-v2-blendfix-coherent \
  --voice-wav /absolute/original.wav \
  --work-dir /absolute/NEW-AUDIT-DIRECTORY \
  --blend-preserve
```

Build: `nice -n 10 cargo build --offline -j 2 --manifest-path emotion-blend/global-fix/refbuilder/Cargo.toml`. Binary currently `refbuilder/target/debug/galion-blend-ref`; CLI wrapper permits `CBX_BLEND_REF_BINARY` override. No production synthesis source modified by this variant.

## Validation table

Samantha: 12 real T0/fp32 renders, legacy repetition penalty1.2, same timbre0.5 stage as merged production. **PASS only on requested WER/ECAPA guard**: WER0.01455 ≤0.018 and ECAPA0.7081 >0.7057. MOS regresses3.8757 vs4.1323, so not recommended for production on this result alone. Emotion distance improves0.437251 vs0.4651, but angry labels increase7→9 (neu2/hap1): do not use angry counts as a standalone emotion-distance proxy. No statistical/repeated-noise win claimed.

| Voice | n | WER | ECAPA | MOS | Emotion distance | Guard |
|---|---:|---:|---:|---:|---:|---|
| Samantha merged baseline |12|0.018|0.7057|4.1323|0.4651|baseline|
| Samantha coherent-prefix |12|0.01455|0.7081|3.8757|0.437251|PASS WER/identity only; quality regression|
| Jarvis merged baseline |12|0.019|0.7331|4.3141|0.5020|baseline|
| Jarvis coherent-prefix |pending|—|—|—|—|PENDING|

Required guard is WER ≤0.018 / ECAPA >0.7057 Samantha; WER ≤0.019 / ECAPA >0.7331 Jarvis, all 12 T0 lines. This is stricter than the older scoreboard's ≤0.01 identity-loss gate.


## Coherent versus montage (do not conflate)

Parallel worker A-global's `pipeline/bin/cbx-add-voice --emotion-blend` / `pipeline/emotion-blend-ref` chooses diverse three2s excerpts, encodes original+montage and retains exact original identity tensors. My `cbx-clone-voice --blend-preserve` keeps a coherent cyclic source sequence and encodes all four tensors from that full rotated source; original speaker embeddings are NOT guaranteed byte-identical. This is an intentional independent comparison, not a replacement for that worker's code. Candidate C owns montage synthesis. No montage-versus-coherent winner is established without matched completed12-line scores.

Measured prep-only objective (not generated emotion fidelity): Samantha start4.36s, cost0.571338→0.104628; Jarvis start13.76s, cost0.190711→0.048328. Jarvis wraps across original end into the beginning; relative timing/gain retained, five-ms old end/start fade only.

Samantha candidate expressive metrics across12 lines: F0 mean193.055Hz; F0 range9.30754st; arousal population SD0.0598544; speech rate3.35278syllables/s; pause ratio0.0678205; hard-label entropy1.04085bits. Versus production per-line F0 range7.97304st and arousal SD0.049854, these increase; this does NOT prove human emotional blend preservation. Mean F0 moves above production185.731Hz, and MOS worsens. Composite emotion-distance improvement and increased angry classifications can coexist.

Samantha finished before parent dual-lock steering; all its TTS held shared render.lock and no second TTS was observed. After steering, coherent wrapper/validation holds BOTH shared render.lock and /tmp/cbx-render.lock, including post-apply output stages. Jarvis currently queued, not scored. Original/reference and production-profile checksums verified unchanged in protected-verify.log. No git/deploy/laptop operations.


## Automatically completed validation

The completed-score table in [VALIDATION.md](VALIDATION.md) and `results12.json` supersedes any earlier pending status above. No global pass unless `global_guard_pass` is true.
