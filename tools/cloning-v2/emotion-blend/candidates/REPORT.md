# Emotion-blend Samantha candidates — 12-line results

**When:** 2026-11-05T22:58:31+01:00 (Europe/Zagreb (UTC+1))
**Worker:** C — `emotion-blend/candidates/`
**Base:** production `own-samantha` (merged v2: emo-r2 conditioning, T0, timbre-match 0.5)
**Keep rule:** WER ≤ 0.018 **AND** ECAPA > 0.7057 (strict vs merged baseline ECAPA 0.7057)

## Candidate table

| Candidate | ECAPA ↑ | WER ↓ | MOS ↑ | Emo dist orig ↓ | Emo dist calm ↓ | SER labels | F0 Hz | Pace | HF f99 | clicks/clips | KEEP |
|---|---:|---:|---:|---:|---:|---|---:|---:|---:|---|---|
| own-samantha (merged v2 baseline) | 0.7057 | 0.018 | 4.13 | 0.4651 | — | ang7, neu4, hap1 | 181.1 | 3.556 | 2686 | 4/0 | baseline |
| own-samantha-v2-blend-ref | 0.3441 | 0.962 | 3.33 | 0.5623 | 0.5675 | sad5, neu4, hap2, ang1 | 183.6 | 3.432 | 2590 | 1/0 | **FAIL** |
| own-samantha-v2-blend-pros | 0.7051 | 0.018 | 4.12 | 0.4600 | 0.4464 | ang7, neu4, hap1 | 181.1 | 3.556 | 2652 | 4/0 | **FAIL** |
| own-samantha-v2-blend-both | 0.3455 | 0.958 | 3.33 | 0.5589 | 0.5695 | sad5, neu4, hap2, ang1 | 183.8 | 3.432 | 2569 | 1/0 | **FAIL** |
| own-samantha-v2-blend-global | 0.7081 | 0.015 | 3.88 | 0.4373 | 0.4376 | ang9, neu2, hap1 | 186.2 | 3.317 | 2704 | 4/0 | **PASS** |
| own-samantha-v2-blend-pros-f0energy-fail (archived) | 0.6808 | — | — | — | — | — | — | — | — | — | **FAIL** |

## Pass / fail

**Passing:** `own-samantha-v2-blend-global`

**Failing:** `own-samantha-v2-blend-ref`, `own-samantha-v2-blend-pros`, `own-samantha-v2-blend-both`, `own-samantha-v2-blend-pros-f0energy-fail`

## Method notes

### (a) `own-samantha-v2-blend-ref`
- FINDINGS: happy-evidence 10–12.5s sits outside Turbo first-6s LM crop; do not maximize angry tail.
- Assembled `prep/samantha-blend-ref.wav` from ECAPA-consistent windows (floor median−max(0.10,1.4826·MAD), hard ≥0.65): order **10–12.5 (hap mix) → 12.5–15 (ang) → 5–7.5 → 2.5–5 → 0–2.5 → 15–17.41**; **dropped 7.5–10** (ECAPA 0.556 vs original).
- Encoded with `cbx-blend voice add --voice-wav original.wav --conditioning-wav samantha-blend-ref.wav` (speaker embeddings from full original; conditioning from montage).
- Rendered T0/fp32/seed0 via `cbx-speak-voice` with voice-defaults timbre 0.5.

### (b) `own-samantha-v2-blend-pros`
- Post-only on `merge-v2/wav/samantha/` (no re-TTS).
- Light energy-envelope expand ×1.03 around median RMS; RMS preserved; peak ≤ −0.09 dBFS; **no mean F0 shift**.
- F0-contour expand attempt archived (`wav-f0energy-fail/`, ECAPA 0.6808) — confirms pitch post-hack hurts ECAPA.

### (c) `own-samantha-v2-blend-both`
- (a) renders + same energy ×1.03 post layer as (b).

### (d) `own-samantha-v2-blend-global`
- Reused Worker A's `own-samantha-v2-blendfix-coherent` timbre-0.5 wavs from `global-fix/wav/samantha/` (coherent prefix rotation selected start 4.36 s).
- Independently scored here including calm-target emotion distance.

### Calm target
`prep/samantha-calm-target.wav` = original 0–7.5 s (neu-dominant windows), unnormalised.

## Safety

- `own-samantha.cbxvoice` sha256 unchanged: `c06681cf37758c3b4fb4328e087b83c14c51c45c01041f5eeeddfa81dd3ebc97`
- No Jarvis changes; originals read-only; no git/deploy/delete/laptop/Piper.
- Renders serialized on `emotion-blend/render.lock`; `nice -n 10`; 2 threads; CPU.

## Artefacts

- Wavs: `own-samantha-v2-blend-{ref,pros,both,global}/wav/t01–t12.wav`
- Scores: `scores/<tag>/{ecapa,wer,mos,hygiene,emotion_orig,emotion_calm,pros}.json*`
- Prep: `prep/samantha-blend-ref.wav`, `prep/samantha-calm-target.wav`, `prep/blend-ref-manifest.json`
- Machine-readable: `results12.json`

## Incident: own-samantha overwrite (restored)

At 22:56 CET `own-samantha.cbxvoice` was found equal to `own-samantha-v2-blendfix-coherent.cbxvoice` (sha256 `695b7266…`), not Worker C's blend-ref (`fa3a8f21…`). Worker C's voice-add log recorded own-samantha unchanged at 22:45 (`c06681cf…`). Restored at ~22:58 CET by byte-copy from `own-samantha-v2-emo-r2.cbxvoice` (merged production source). Post-restore sha256 again `c06681cf37758c3b4fb4328e087b83c14c51c45c01041f5eeeddfa81dd3ebc97`. Jarvis untouched (`73fc04d8…`).

## Update: (e) emotion-blend montage (after steering)

**When:** 2026-11-05T23:29:01+01:00 (Europe/Zagreb)
**Locks:** every speak holds `emotion-blend/render.lock` **and** `/tmp/cbx-render.lock`.
**Production voice:** live `own-samantha` sha256 `695b7266…` (Admin Slavik–approved blendfix). Worker C no longer restores.

| Candidate | ECAPA | WER | MOS | Emo→orig | Emo→calm | Labels | F0 | KEEP |
|---|---:|---:|---:|---:|---:|---|---:|---|
| own-samantha-v2-blend-montage | — | — | — | — | — | — | — | **FAIL** (unspeakable) |
| own-samantha-v2-blend-montagectl | 0.6270 | 0.041 | 4.01 | 0.5214 | 0.4974 | ang8, neu4 | 190.4 | **FAIL** |

### Official path failure
`cbx-add-voice --emotion-blend` built conditioning from windows **[1,4,6]** (includes happy **9.95–11.95 s**) then `cbx-blend voice add --conditioning-wav`.
Speak on both `cbx` and `cbx-blend` dies: `conditional_decoder … Invalid input shape: {0}` (`/f0_predictor/condnet/condnet.0/Conv`).
Profile size **780680** vs healthy **~781480**. Reproduced as `own-samantha-v2-blend-montageb`.

### Control
Same `conditioning.wav` encoded with **normal** `cbx voice add` → `own-samantha-v2-blend-montagectl` speaks; 12-line numbers above. **Not** a keep-eligible official (e) promote — diagnostic only.

### Waiter fix
`pgrep -f cbx-blend` matched the waiter bash cmdline and never cleared. `render12.sh` now relies on blocking dual `flock` only (no pgrep binary wait).

## Correction: do not restore own-samantha

Earlier Worker C restores of `own-samantha` from emo-r2 were **wrong**. Live hash `695b7266…` is the approved blendfix promote. No further writes to `own-samantha` / `own-jarvis`.

