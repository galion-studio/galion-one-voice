# VALIDATE12 — own-samantha-v2-emo-r2 on full 12 test lines

Completed 2026-11-05 21:39 CET (Europe/Zagreb). Real measured scores only.

## Setup

- Voice: `own-samantha-v2-emo-r2` (READ-ONLY production `own-samantha.cbxvoice` untouched).
- Binary: `emotion-fix/cbx-emo/target/release/cbx` (same as `run_cfg.sh`), `--intra-threads 2`, `nice -n 10`, fp32, `--seed 0`.
- Manifest: `test12.tsv` (same texts as `own-clone-analysis/wav/samantha/t01–t12.wav`).
- (a) **r2@T0**: `--temperature 0` defaults (out: `runs/sd_r2_t012`).
- (b) **r2@T0.8**: `--temperature 0.8 --top-k 1000 --top-p 0.95 --rep-unique --punc-norm` (out: `runs/sd_r2_up12`).
- Baseline: existing 12-line clips `own-clone-analysis/wav/samantha/` (symlinked as `runs/sam_base12`).
- Scorers: `galion-emotion` fp32 (`--ref clip-store-own/samantha/reference.wav`), `galion-ecapa`, `galion-wer`, `galion-mos`, `galion-hygiene`, `galion-artic`; persona target `targets/samantha.json` / `personas/samantha-persona.md`.

## Summary table (n=12)

| Setting | ECAPA mean±sd ↑ | WER ↓ | MOS ↑ | Emotion dist ↓ | SER (ang/neu/hap) | A/V/D means | words/s |
|---|---:|---:|---:|---:|---|---:|---:|
| baseline own-samantha | 0.7072±0.0315 | 5.0% | 3.728 | 0.5062 | ang 7, neu 3, hap 2 | 0.516/0.537/0.593 | 3.490 |
| r2@T0 | 0.7082±0.0324 | 1.8% | 4.104 | 0.4675 | ang 5, neu 6, hap 1 | 0.483/0.513/0.567 | 3.556 |
| r2@T0.8 (dev settings) | 0.7032±0.0339 | 6.8% | 4.102 | 0.4828 | ang 4, neu 6, hap 2 | 0.502/0.531/0.581 | 3.561 |

Also (not promotion gates): persona-prosody distance baseline 6.122 → T0 6.438 / T0.8 **4.759**; AVD distance baseline 0.0969 → T0 0.0973 / T0.8 0.1115; F0 median mean baseline 202.97 Hz → T0 181.71 / T0.8 183.20 (target ~176.8 Hz).

## Per-line SER labels

| Line | baseline | r2@T0 | r2@T0.8 |
|---|---|---|---|
| t01 | hap | hap | hap |
| t02 | ang | ang | ang |
| t03 | hap | neu * | hap |
| t04 | neu | neu | neu |
| t05 | neu | neu | neu |
| t06 | ang | ang | neu * |
| t07 | neu | neu | neu |
| t08 | ang | neu * | ang |
| t09 | ang | ang | ang |
| t10 | ang | ang | neu * |
| t11 | ang | neu * | neu * |
| t12 | ang | ang | ang |

`*` = label changed vs baseline.

### Label-change list

- **r2@T0**: t03: hap→neu, t08: ang→neu, t11: ang→neu
- **r2@T0.8**: t06: ang→neu, t10: ang→neu, t11: ang→neu

Angry count: baseline **7/12** → r2@T0 **5/12** → r2@T0.8 **4/12**. Neutral: 3 → 6 → 6.

## Per-line detail (emotion dist / ECAPA)

### baseline

| Line | Label | P(ang) | P(neu) | P(hap) | Emotion dist | ECAPA |
|---|---|---:|---:|---:|---:|---:|
| t01 | hap | 0.3231 | 0.2315 | 0.3961 | 0.4595 | 0.7611 |
| t02 | ang | 0.6116 | 0.3662 | 0.0201 | 0.6218 | 0.6817 |
| t03 | hap | 0.2100 | 0.3512 | 0.4379 | 0.4243 | 0.7230 |
| t04 | neu | 0.2978 | 0.6789 | 0.0234 | 0.6663 | 0.6773 |
| t05 | neu | 0.3854 | 0.5693 | 0.0453 | 0.3386 | 0.7430 |
| t06 | ang | 0.5431 | 0.2394 | 0.1411 | 0.5389 | 0.6799 |
| t07 | neu | 0.3807 | 0.6080 | 0.0092 | 0.7312 | 0.6616 |
| t08 | ang | 0.6831 | 0.2633 | 0.0529 | 0.4689 | 0.7279 |
| t09 | ang | 0.9243 | 0.0708 | 0.0043 | 0.5483 | 0.7207 |
| t10 | ang | 0.8582 | 0.1240 | 0.0171 | 0.5233 | 0.6723 |
| t11 | ang | 0.8237 | 0.1668 | 0.0087 | 0.3029 | 0.7197 |
| t12 | ang | 0.6042 | 0.3682 | 0.0271 | 0.4505 | 0.7181 |

### r2@T0

| Line | Label | P(ang) | P(neu) | P(hap) | Emotion dist | ECAPA |
|---|---|---:|---:|---:|---:|---:|
| t01 | hap | 0.2325 | 0.2115 | 0.5512 | 0.4879 | 0.7656 |
| t02 | ang | 0.6543 | 0.3347 | 0.0098 | 0.6377 | 0.7059 |
| t03 | neu | 0.0997 | 0.5518 | 0.3477 | 0.3882 | 0.7099 |
| t04 | neu | 0.0389 | 0.9135 | 0.0476 | 0.6018 | 0.7038 |
| t05 | neu | 0.0247 | 0.9509 | 0.0244 | 0.4879 | 0.6601 |
| t06 | ang | 0.4984 | 0.1797 | 0.2522 | 0.4872 | 0.7742 |
| t07 | neu | 0.1528 | 0.8335 | 0.0132 | 0.4842 | 0.6822 |
| t08 | neu | 0.3889 | 0.3914 | 0.2192 | 0.4030 | 0.7075 |
| t09 | ang | 0.8167 | 0.1719 | 0.0098 | 0.4195 | 0.7096 |
| t10 | ang | 0.6172 | 0.2086 | 0.1716 | 0.3008 | 0.7021 |
| t11 | neu | 0.3898 | 0.5872 | 0.0221 | 0.3364 | 0.6945 |
| t12 | ang | 0.8139 | 0.1087 | 0.0688 | 0.5753 | 0.6833 |

### r2@T0.8

| Line | Label | P(ang) | P(neu) | P(hap) | Emotion dist | ECAPA |
|---|---|---:|---:|---:|---:|---:|
| t01 | hap | 0.1785 | 0.3180 | 0.5007 | 0.4774 | 0.6657 |
| t02 | ang | 0.5830 | 0.3510 | 0.0636 | 0.5744 | 0.7259 |
| t03 | hap | 0.1212 | 0.4316 | 0.4461 | 0.4377 | 0.6936 |
| t04 | neu | 0.1329 | 0.6906 | 0.1764 | 0.5526 | 0.7363 |
| t05 | neu | 0.2321 | 0.7089 | 0.0588 | 0.4708 | 0.6725 |
| t06 | neu | 0.0719 | 0.5545 | 0.3648 | 0.4838 | 0.6507 |
| t07 | neu | 0.1550 | 0.7997 | 0.0451 | 0.4708 | 0.6864 |
| t08 | ang | 0.4754 | 0.2111 | 0.3128 | 0.4898 | 0.7302 |
| t09 | ang | 0.6262 | 0.3512 | 0.0206 | 0.3914 | 0.7439 |
| t10 | neu | 0.2846 | 0.5533 | 0.1620 | 0.5206 | 0.7151 |
| t11 | neu | 0.3820 | 0.5191 | 0.0961 | 0.3963 | 0.6707 |
| t12 | ang | 0.6933 | 0.2290 | 0.0736 | 0.5282 | 0.7473 |

## Verdict

Identity floor = baseline ECAPA − 0.01 = **0.6972**. Emotion must be closer (lower distance) than baseline **0.5062**.

- **r2@T0**: ECAPA 0.7082 (Δ +0.0010) → OK; emotion 0.4675 (Δ -0.0387) → closer. **REAL WIN**
- **r2@T0.8**: ECAPA 0.7032 (Δ -0.0040) → OK; emotion 0.4828 (Δ -0.0234) → closer. **REAL WIN**

**Overall: real win on the stated gates for both temperatures.** Stronger joint profile is **r2@T0** (best emotion distance, identity slightly up, WER 1.8%, MOS ~4.10); **r2@T0.8** keeps the development persona-prosody win (4.76 vs 6.12) with a small ECAPA dip still inside the 0.01 guard, but WER rises to 6.8%. Prefer T0 for production-shaped decoding unless persona distance is the primary objective.

## Notes / limits

- Four-line dev optimism (REPORT.md): matched emo 0.4603 / ECAPA 0.7380 does **not** fully carry; 12-line T0.8 emo 0.4828 / ECAPA 0.7032.
- Decoder `RandomNormalLike` nodes remain unseeded; even T0 is not fully bit-deterministic.
- SER angry tags are noisy on TTS; treat label shifts as supporting evidence, not human emotion judgment.
- No originals modified; no git; no Piper training; no laptop writes.

