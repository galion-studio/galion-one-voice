# Emotion-match (full range) — build report

Finished 2026-11-05 Europe/Zagreb. **No new emotion model fetched** (research running separately). **No laptop package yet.** Classifier is a **pluggable backend**; only `galion-emotion` is wired.

## What was built

| Piece | Path |
|---|---|
| Metric + catalog + blend | `galion-voicecheck/src/emotion_match.rs` |
| Pluggable backend trait | `galion-voicecheck/src/emo_backend.rs` (`EmotionBackend`; `GalionEmotionBackend` default; unknown names error) |
| Gate (WER ∧ ECAPA ∧ emotion) | `galion-voicecheck/src/gate.rs` |
| CLI / orchestration | `galion-voicecheck/src/main.rs` (`--emotion-window`, `--emo-min`, `--emotion-backend`, `--emotion-presence`, `--skip-emotion`) |
| Report leaderboard | `galion-voicecheck/src/report.rs` |
| Samantha config | `config/voices/own-samantha.json` (`emotion_enabled: true`, window, `emo_min: 0.83`) |
| Jarvis config | `config/voices/own-jarvis.json` (`emotion_enabled: false` — profile still computable; voice untouched) |
| Calibration (existing renders only) | `reports/emotion-match-calib/CALIBRATION.{json,md}` |

Binary: `galion-voicecheck/target/release/galion-voicecheck` (symlink `bin/galion-voicecheck`).  
Tests: **27 passed** (was 16; + emotion_match / emo_backend / emotion-gate cases).

## Formula (headline `emotion_match`, higher better)

1. Segment the **original** base into ~2.5 s windows (read-only; cuts go to temp under the run dir). Optional forced span via `--emotion-window` / config (Samantha also forces PROFILE **9.947–12.434**).
2. Score each window with the active backend → SER probs + A/V/D (+ proxies).
3. For each measurable category, pick the strongest window with strength ≥ presence (default 0.28) as the **moment**.
4. For each of the clone’s 12 lines: `match_i = max(0, 1 − RMS_7(v_line − v_moment))` on `(neu,hap,ang,sad,arousal,valence,dominance)`.
5. `score_C = 0.55·max_i match_i + 0.45·mean_i match_i`.
6. `emotion_match = Σ_C strength_C · score_C / Σ strength_C` over detected moments (aliases happy→joyful, reason→logic collapsed for the blend).

## Category measurability (honest)

| Category | Kind | Detector |
|---|---|---|
| joyful / happy | **direct model label** | SUPERB `hap` + valence |
| aggressive | **direct model label** | SUPERB `ang` |
| sad | **direct model label** | SUPERB `sad` (no Samantha short-window moment — max P(sad)≈0.043 < 0.28) |
| neutral | **direct model label** | SUPERB `neu` |
| yelling | **proxy A/V/D + prosody** | ang + high arousal + elevated F0/energy |
| fear | **proxy** | low valence + high arousal + low dominance (**no fear class**) |
| logic / reason | **proxy** | high neu + low arousal + steady F0 |
| love | **proxy** | high valence + low-mid arousal + soft level |
| seductive | **proxy** | high valence + low arousal + soft + shimmer (overlaps love) |
| surprise, disgust | **not measurable** | need a richer SER; not invented |

## Happy / joyful window used

- Analysis confirmation: FINDINGS / `original-emotion.json` **10.00–12.50 s**, P(hap)=**0.3631**, V=**0.6429** (top still `neu`).
- Finer PROFILE phrase grid: **9.947–12.434 s**, P(hap)=**0.3550**, V=**0.6407**.
- Calibration moments used the 2.5 s grid; config forces the PROFILE span into the scored set as well.
- Source: `clip-store-own/samantha/original.wav` (READ-ONLY).

## Per-render-set results (existing data only; no new renders)

| Set | WER | ECAPA | emotion_match |
|---|---:|---:|---:|
| Samantha BASE self-ceiling | — | — | **0.8808** |
| Samantha merge-v2 (prod pre-fix) | 0.01764 | 0.7057 | **0.8389** |
| Samantha global-fix coherent | 0.01455 | 0.7081 | **0.8164** |
| Samantha refprep | 0.02566 | 0.6739 | 0.8394 |
| Samantha refprep-baseline | 0.05026 | 0.7072 | 0.8191 |
| Jarvis BASE self-ceiling | — | — | **0.9409** |
| Jarvis merge-v2 | 0.01852 | 0.7331 | **0.9054** |
| Jarvis global-fix coherent | 0.02963 | 0.7303 | 0.8800 |

### Samantha leaderboard highlights

- **merge-v2:** joyful/love/seductive ≈ 0.859 (best t03); aggressive/yelling/fear ≈ 0.808 (best t09); neutral 0.867.
- **coherent:** aggressive/yelling/fear **up** to ≈ 0.862 (best t02); joyful/love/seductive **down** to ≈ 0.778 (best t04). Angry-tail match improved; happy-moment match got worse.

### Jarvis

Moments are weak-joyful / strong-neutral / calm “love-proxy” on early windows. Emotion gate **OFF** by default (`own-jarvis.json`). Production Jarvis voice not touched.

## Recommended thresholds

- **Blended gate `emo_min = 0.83`** ≈ merge-v2 0.8389 − ~0.01 noise margin. Candidate must not meaningfully regress vs the pre-fix voice.
- **Per-emotion soft mins** (diagnostic only, not hard gates yet): joyful 0.84, aggressive/yelling/fear 0.79, logic 0.77, love/seductive 0.76, neutral 0.78.
- Full gate order: **WER ≤ 0.018 ∧ ECAPA > 0.7057 ∧ emotion_match ≥ 0.83**.

## Does coherent pass all three on existing box renders?

| Gate | Bar | Coherent | Pass? |
|---|---:|---:|---|
| WER | ≤ 0.018 | 0.01455 | YES |
| ECAPA | > 0.7057 | 0.7081 | YES |
| emotion_match | ≥ 0.83 | 0.8164 | **NO** |

**No.** Coherent would be rejected by the new emotion gate until joyful/happy match recovers (or the threshold is deliberately relaxed by the owner).

## Pluggable backend

```text
config "emotion_backend" / --emotion-backend NAME
  └─ emo_backend::resolve()
       ├─ "galion-emotion" (default) → GalionEmotionBackend
       └─ anything else → hard error until registered
```

Drop-in later: implement `EmotionBackend::score_files` → same `files[].ser/avd/proxies` JSON shape; moment/leaderboard/gate code stays unchanged.

## Not done (per steering)

- No new ONNX / emotion2vec+ fetch.
- No laptop `emotion-match-update` package.
- No git commit/push, no deploy, no Jarvis voice change, no production profile writes.

## Universal backend follow-up

See `UNIVERSAL-EMOTION.md`. emo_min kept at 0.83. Samantha merge-v2/coherent fail the new metric; Jarvis passes.

## EfficientAT integration (2026-11-05 Europe/Zagreb)

PRIMARY detector for **yelling / crying / laughter** in the universal stack.

| Item | Value |
|---|---|
| Model | https://huggingface.co/Chris-Kode/efficientat-mn10-as-onnx |
| Licence | MIT |
| ONNX sha256 | `bec20620ab8be5e41297b45845f6fb116fbe42cd4da339ad7d44f27630b7a504` |
| Size | 20,200,590 bytes |
| Runtime | Rust `ort` → `scorers/universal/bin/galion-efficientat` |
| Sample rate | 32 kHz (linear resample) |
| Pooling | window 2.5 s / hop 1.25 s; max (primary) + mean |
| Attribution | EfficientAT mn10_as © 2022 Florian Schmid; ONNX export Chris-Kode |
| Leaderboard label | `efficientat` |

CLAP kept for love / seductive / logic / reason. emotion2vec+ kept for its trained classes. emo_min **0.83** unchanged. Full numbers in `UNIVERSAL-EMOTION.md`. Manifest: `scorers/universal/licenses/{SHA256SUMS,LICENSES.md}` + `efficientat/state_class_map.json`.
