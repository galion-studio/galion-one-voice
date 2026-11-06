# galion-voicecheck (Worker B) — REPORT

**When:** 2026-11-05 ~22:41 CET (Europe/Zagreb, UTC+1)  
**Owner:** Mate  
**Scope:** box-local only — no git commits/pushes, no deploys, no laptop test, no Piper training  
**Tree:** `cloning-v2/emotion-blend/autocheck/`

## What was built

| Path | Role |
|---|---|
| `Cargo.toml` + `src/*.rs` | Rust CLI `galion-voicecheck` |
| `target/release/galion-voicecheck` | release binary |
| `PROFILE-FORMAT.md` | profile schema for worker A alignment |
| `../analysis/profile-{samantha,jarvis}.json` | seeded from A-global + merge-v2 baselines |
| `fixes/fix-{a,b,c}-*.sh` | pluggable fix plugins (simple B implementations) |
| `bin/hook-on-voice-add.sh` | post-`cbx voice add` hook |
| `bin/watch-voices.sh` | optional polling watcher (no systemd, no inotify required) |
| `fixtures/text12/t01..t12.txt` | VALIDATE12 / merge-v2 12-line set |
| `reports/*.json` + `*.md` | per-voice reports |
| `../render.lock` | box-wide flock serialisation for TTS |

### CLI usage

```bash
# Automatic check after clone (hook):
emotion-blend/autocheck/bin/hook-on-voice-add.sh \
  --voice own-samantha-v2-check-foo \
  --base /workspace/swarm/voice-v2-sprint/clip-store-own/samantha/original.wav

# Dry-run on existing renders (no TTS):
galion-voicecheck check --voice own-samantha --dry-run --reuse-scores \
  --renders-dir cloning-v2/merge-v2/wav/samantha

# Gate unit helper:
galion-voicecheck gate --wer 0.018 --ecapa 0.7058

# Optional watcher (foreground; Ctrl-C):
emotion-blend/autocheck/bin/watch-voices.sh
```

Override plugins: `VOICECHECK_FIX_A` / `_B` / `_C` → path to replacement scripts (worker A/C).

### Pipeline per voice

1. Load `analysis/profile-<persona>.json` (or A-global blend schema).
2. Render 12 lines @ T0 via `pipeline/bin/cbx-speak-voice` under `flock emotion-blend/render.lock` (nice 10, 2 threads) — skipped in `--dry-run`.
3. Line-by-line pitch/energy/pace + SER/AVD vs base (`galion-emotion`, `galion-ecapa`, `galion-wer`).
4. Try fixes (a) expressive ref (b) light prosody (c) combined — pluggable.
5. Keep a fix only if **WER ≤ wer_max** AND **ECAPA > ecapa_min** (Samantha defaults 0.018 / 0.7057).
6. Write `reports/<voice>-*.json` + `.md`. New voices only `own-<p>-v2-check-*`.

## Tests

```
cargo test -j 2
# 6 passed (gate.rs):
#  - samantha exact baseline ECAPA 0.7057 → REJECT (strict >)
#  - WER at cap + ECAPA 0.7058 → KEEP
#  - WER above cap → REJECT even if ECAPA better
#  - both fail / Jarvis custom thresholds / floating WER edge
```

## Dry-run results (merge-v2 existing renders, `--reuse-scores`)

Real numbers from `merge-v2/scores/*` (same wavs under `merge-v2/wav/{samantha,jarvis}/`):

| Voice | WER | ECAPA | Emo dist | Gate vs baseline |
|---|---:|---:|---:|---|
| own-samantha | **0.0180** | **0.7057** | 0.4651 | **REJECT** (ECAPA not strictly > 0.7057) |
| own-jarvis | **0.0190** | **0.7331** | 0.5020 | **REJECT** (ECAPA not strictly > 0.7331) |

Reports: `reports/own-samantha-dryrun.{json,md}`, `reports/own-jarvis-dryrun.{json,md}`.  
Fixes correctly **skipped** in dry-run (no voice writes). Production `.cbxvoice` sha256 unchanged vs merge-v2.

## Live smoke (2 lines, `--skip-fixes`)

Rendered t01–t02 for `own-samantha` through flock+cbx-speak-voice, then scored:

| n | WER mean | ECAPA mean | Emo dist |
|--:|---:|---:|---:|
| 2 | 0.028 | 0.6969 | 0.5572 |

Report: `reports/own-samantha-smoke2.{json,md}`. No fix voices created. No full 12-line live fix run (worker C owns Samantha candidates).

## Constraints respected

- nice -n 10, 2 threads, CPU only; flock serialises TTS box-wide
- never overwrote `own-samantha` / `own-jarvis` (sha256 match merge-v2 live)
- originals / reference.wav read-only
- no git commits/pushes, no deploys, nothing to laptop, no Piper training
- no systemd service installed

## Note for parent (owner follow-up pt759u)

Worker B **did not** push/merge/deploy or run the laptop 12-line test — those are explicitly out of this worker’s constraints and must wait until A (global fix) + B (this check) + C (Samantha candidates) are done, then parent/owner can approve the irreversible steps. Jarvis deployed version must stay unchanged per owner.
