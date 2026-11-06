# Cloning V2 → production merge report

**When:** 2026-11-05 ~22:22 CET (Europe/Zagreb)  
**Approval:** Owner Mate, 2026-11-05 22:17 CET (t753u)  
**Scope:** box-local only — no git commits/pushes, no VPS/live deploy  
**Artefacts:** `merge-v2/results12.json`, this file. Scoreboard left to worker sand-subagent-c39d6808.

## What changed

### Backups (checksum-verified; nothing deleted)

| Path | Role |
|---|---|
| `~/.cache/huggingface/cbx/voices/own-samantha.cbxvoice.bak-pre-v2-20261105` | pre-merge Samantha profile (sha256 `7b2ead400cbc517c…`) |
| `~/.cache/huggingface/cbx/voices/own-jarvis.cbxvoice.bak-pre-v2-20261105` | pre-merge Jarvis profile (sha256 `73fc04d8e8aafb20…`) |
| `clip-store-own/{samantha,jarvis}/consent.txt.bak-pre-v2-20261105` | metadata backup |
| `clip-store-own/{samantha,jarvis}/reference.wav.sha256.bak-pre-v2-20261105` | metadata backup |

`original.wav` / `reference.wav` under clip-store-own were **not** modified (READ-ONLY). No `Samantha.wav` / `Jarvis.wav` originals touched.

### Samantha production profile

- **Merged conditioning** from `own-samantha-v2-emo-r2` into `own-samantha.cbxvoice` (byte-copy of profile).
- Live sha256 now equals emo-r2: `c06681cf37758c3b4fb4328e087b83c14c51c45c01041f5eeeddfa81dd3ebc97`
- Pre-merge bak sha256: `7b2ead400cbc517ca15e4bbd2540fa5a28c1e66cc2b33a5bb481f2cf05e8cf20`
- Default render: **temperature 0**, **fp32** (via `pipeline/voice-defaults/own-samantha.json`).

### Jarvis production profile

- **Conditioning UNCHANGED** (live sha256 still equals bak-pre-v2).
- Live sha256: `73fc04d8e8aafb20a3294b38e39746408dbd1b74f804f9089eff147a9d94155d`

### Shared timbre (both voices)

- Enabled as **per-voice default** only for `own-samantha` and `own-jarvis`:
  - `pipeline/voice-defaults/own-samantha.json` → `--timbre-match 0.5 --timbre-ref clip-store-own/samantha/reference.wav`
  - `pipeline/voice-defaults/own-jarvis.json` → `--timbre-match 0.5 --timbre-ref clip-store-own/jarvis/reference.wav`
- Wrapper: `pipeline/bin/cbx-speak-voice` (and `cbx-batch-voice`) reads those configs; binary default remains `--timbre-match 0` for all other voices.
- Binary: `pipeline/cbx-v2-timbre/target/release/cbx`

### Explicitly NOT applied

- Jarvis `hf-e-mild` (rejected)
- Jarvis gentler warmth (`emo-m*`, `r6`, `r10`, `ln`)
- Samantha `hf-e-mild`
- Natural pauses (`cbx-v2-prosody` stays off)

## 12-line test method

Same texts as `emotion-fix/VALIDATE12.md` / `test12.tsv`.

| Voice | Wav construction |
|---|---|
| Samantha merged | Existing `emotion-fix/runs/sd_r2_t012` (own-samantha-v2-emo-r2 @ T0/fp32/seed0) → `cbx timbre apply --strength 0.5` → `merge-v2/wav/samantha/` |
| Jarvis merged | Baseline `own-clone-analysis/wav/jarvis/` → same timbre apply 0.5 → `merge-v2/wav/jarvis/` |

Timbre apply is the same Rust post-decoder stage as `speak --timbre-match` (documented in `pipeline/cbx-v2-timbre/REPORT.md`). Post-only avoids re-TTS OOM under box load (nice 10, 2 threads, CPU only, ≤1 heavy job at a time).

Scorers: `galion-ecapa`, `galion-emotion` (fp32), `galion-artic`, `galion-{wer,mos,hygiene}`, plus cbx `analyze` for F0 median / pace w/s.

### Baselines

- Samantha: `sam_base12` scores (VALIDATE12 / own-clone-analysis wavs) — ECAPA 0.7072, WER 0.050, MOS 3.73, emo dist 0.506, ang 7/12, F0 203 Hz.
- Jarvis: `jar_old` emo/ecapa/pros + freshly scored `jar_base12_{mos,wer,hygiene}` on the same baseline wavs with the same tools.

## New vs baseline (n=12)

| Voice | ECAPA ↑ | WER ↓ | MOS ↑ | Emo dist ↓ | SER labels | F0 Hz | Pace w/s | HF f99 | clicks/clipped |
|---|---:|---:|---:|---:|---|---:|---:|---:|---|
| Samantha baseline | 0.7072 | 0.050 | 3.73 | 0.5062 | ang 7, neu 3, hap 2 | 203.0 | 3.490 | 2814 | 32/9 |
| Samantha merged | 0.7057 | 0.018 | 4.13 | 0.4651 | ang 7, neu 4, hap 1 | 181.1 | 3.556 | 2686 | 4/0 |
| Jarvis baseline | 0.7327 | 0.019 | 4.30 | 0.5114 | neu 12 | 142.4 | 3.311 | 6023 | 39/4 |
| Jarvis merged | 0.7331 | 0.019 | 4.31 | 0.5020 | neu 12 | 142.4 | 3.311 | 6377 | 4/0 |

Pitch targets: Samantha ~178 Hz (merged 181.1); Jarvis ~129–132 Hz (merged 142.4, unchanged vs baseline — conditioning not retargeted).

## Pass / fail

Fail rule: ECAPA drops **> 0.01** vs that voice's baseline → restore from `.bak-pre-v2-20261105`.

| Voice | ECAPA base | ECAPA merged | Δ (base−merged) | Verdict |
|---|---:|---:|---:|---|
| own-samantha | 0.7072 | 0.7057 | +0.0015 | **PASS** |
| own-jarvis | 0.7327 | 0.7331 | -0.0004 | **PASS** |

Both voices **PASS**. No restore performed.

## Notes / limits

- Samantha SER angry count after r2+timbre is still 7/12 (r2@T0 alone was 5/12 in VALIDATE12); emotion **distance** still improves 0.5062 → 0.4651. SER labels on TTS remain noisy.
- Jarvis F0 stays ~142 Hz (above 129–132 target); this merge did not include warmth/rotation variants (explicitly excluded).
- Clicks fall for both (Sam 32→4, Jar 39→4 on hygiene) after the timbre path's declick/limiter stage relative to the older baseline encode hygiene numbers.
- Scoreboard `BOARD.md` is owned by sand-subagent-c39d6808 — not edited here; pick up `merge-v2/results12.json`.
