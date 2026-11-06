# galion-voicecheck — fix-stage plug-in interface

Fixes (a) and (b) are pluggable; (c) = (a)'s renders + (b)'s layer. Provider resolution per stage
(first match wins; `galion-voicecheck stages` prints the result and every path searched):

1. per-voice config `config/voices/<voice>.json` → `"stages": {"reference": [cmd...], "prosody": [cmd...]}`
2. env `GALION_VC_STAGE_REFERENCE` / `GALION_VC_STAGE_PROSODY` (whitespace-split command)
3. worker dirs `emotion-blend/A-global/` and `emotion-blend/C-sam/`:
   `voicecheck-stage.json` = `{"reference":{"cmd":[...]}, "prosody":{"cmd":[...]}}`, or executables
   `voicecheck-reference` / `voicecheck-prosody` (also under `bin/`)
4. reference only: A-global's shared builder `pipeline/emotion-blend-ref/target/release/galion-blend-ref`
   (auto-detected; currently ACTIVE) — crop-aware ≤6 s diverse montage, called as
   `galion-blend-ref --input BASE --output D/reference.wav --meta D/reference.json --scorer galion-emotion`
5. builtin (Rust, in this crate)

## Contracts (all run under `nice -n 10`; stages must NOT render — rendering is done by the checker under the lock)

**reference (fix a)**
`<cmd> --voice V --base BASE.wav --out-dir D --analysis PREFIX_ANALYSIS.json` → must write `D/reference.wav`, exit 0.
The checker then encodes a candidate profile `own-<p>-v2-blend-cand-a-<stamp>` with
`flock /tmp/cbx-render.lock nice -n 10 cbx-blend --intra-threads 2 --inter-threads 2 voice add --dtype fp32 --name CAND --voice-wav BASE --conditioning-wav D/reference.wav`
(identity tensors from BASE, conditioning tokens/features from the reference — A-global's cbx-blend; falls back to plain
`cbx voice add --voice-wav reference.wav` if cbx-blend is missing) and renders the 12 lines with `cbx-speak-voice --voice CAND`
plus the source voice's own defaults (timbre-match/dtype) under `flock /tmp/cbx-render.lock`.

**prosody (fix b)**
`<cmd> --in-dir RENDERS --out-dir D --base BASE.wav --analysis ANALYSIS.json` → must write `D/<id>.wav` for every `RENDERS/<id>.wav`.
`ANALYSIS.json` = `{"base": {...base metrics...}, "variant": {"lines": [{id, f0_*, energy_dr_db, pause_ratio, wps, ser, avd, ecapa, wer, ...}]}}`.

## Builtins
- reference: `galion-voicecheck expressive-ref --base B --out-dir D [--target-s 10]` — phrases split on ≥0.2 s pauses,
  scored z(F0 sd)+z(F0 p5–p95 range)+z(energy sd)+0.5·z(level), top phrases (≈60 % of speech, 6–10 s) ordered
  most-expressive-FIRST (the Turbo speech encoder only sees the first 6 s / 10 s — A-global encoder-graph evidence).
- prosody: `galion-voicecheck layer --in-dir R --out-dir D --analysis A.json` — per line: energy expansion
  k = 1 + 0.5·(base_DR/line_DR − 1) clamped [1, 1.2], gain ≤ ±3 dB; lengthen internal pauses ≥80 ms halfway toward
  the base pause ratio (≤ +150 ms per pause, ≤ +10 % duration) by repeating the gap's own room tone; peak ≤ −1 dBFS.
  No pitch/spectral change (pitch shifting cost ECAPA in earlier tests).
