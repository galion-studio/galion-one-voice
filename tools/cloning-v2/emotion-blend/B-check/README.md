# B-check — `galion-voicecheck` (Cloning V2 automatic per-voice check)

Rust crate: `galion-voicecheck/` → binary `galion-voicecheck/target/release/galion-voicecheck` (symlink `bin/galion-voicecheck`).

## Run
```bash
B=/workspace/swarm/voice-v2-sprint/cloning-v2/emotion-blend/B-check/bin
# full check of a voice (renders 12 lines @T0 via pipeline/bin/cbx-speak-voice under flock /tmp/cbx-render.lock)
$B/galion-voicecheck own-newvoice --base /path/recording.wav
# dry mode on existing renders (no renders, no voice writes)
$B/galion-voicecheck own-samantha --base .../clip-store-own/samantha/reference.wav --dry-run --renders cloning-v2/merge-v2/wav/samantha
#   optionally feed existing fix renders: --fix-renders a=DIR[,c=DIR]
# hook for the clone step (cbx voice add + check; refuses own-samantha/own-jarvis/existing names)
$B/galion-voice-add --name own-newvoice --voice-wav /path/recording.wav
# watcher for voices added any other way (seeds existing voices, checks only new own-*.cbxvoice)
$B/galion-voicecheck-watch            # loop every 60 s;  --once for one pass; --dry-run to only log
# tests
cd galion-voicecheck && nice -n 10 cargo test --release --offline -j 2
```
Options: `--threshold` (ECAPA bar, default 0.7057), `--wer-max` (0.018), `--require-beat-prefix`, `--fixes a,b,c`,
`--persona`, `--mos-nmr`, `--texts`, `--no-save`, `--tag`, `--emotion-window START-END`, `--emo-min`, `--emotion-backend`, `--skip-emotion`. Per-voice defaults: `config/voices/<voice>.json`
(`base`, `persona`, `ecapa_threshold`, `wer_max`, `require_beat_prefix`, `mos_nmr`, `stages`, `emotion_enabled`, `emotion_window`, `emo_min`). Examples: `own-samantha.json`, `own-jarvis.json`.

Emotion match (pluggable backend, default `galion-emotion`): segment the base into ~2.5 s windows, find moments (joyful/aggressive/…), score each of the 12 lines against each moment, emit a per-emotion leaderboard and a blended `emotion_match` (higher better). Gate when `emotion_enabled`: WER ≤ 0.018 AND ECAPA > bar AND emotion_match ≥ emo_min. Surprise/disgust are reported as not measurable with the current 4-class SER.

## Pipeline
1. Render `emotion-fix/test12.tsv` (same 12 texts as VALIDATE12) at temperature 0, seed 0, via `pipeline/bin/cbx-speak-voice`
   (per-voice defaults such as timbre-match apply), one line at a time, each in `flock /tmp/cbx-render.lock`, nice 10, 2 threads, waits for ≥3.8 GB free.
2. Analyse base + each variant: galion-emotion fp32 (SER probs, A/V/D, F0 mean/sd/range, energy DR, pause ratio), w/s,
   galion-ecapa vs base, galion-wer (Vosk), galion-mos, galion-hygiene, galion-artic. Line-by-line deltas vs base.
3. Fixes: (a) conditioning reference from the base's most expressive moments (A-global galion-blend-ref active, builtin fallback),
   (b) light prosody layer (builtin; pluggable), (c) both. See INTERFACE.md.
4. Gate (unit-tested, `src/gate.rs`): keep iff WER(mean_raw) ≤ 0.018 AND mean ECAPA > bar (strict). Winner = highest ECAPA.
   Pre-fix numbers are always reported. Winner saved as NEW `own-<p>-v2-blend-<fix>` (create_new byte copy; bumps -2…)
   + `approvals/PENDING-<name>.json` (+ proposed voice-defaults for b/c). Production profiles and voice-defaults are never written.

Outputs: `runs/<voice>/<tag>/{report.json,report.md,base,prefix,fix-a,fix-b,fix-c}`, copies in `reports/<voice>/<tag>.{json,md}` + `LATEST`.
