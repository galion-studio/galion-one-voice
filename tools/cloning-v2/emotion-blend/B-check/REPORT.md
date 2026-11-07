# B-check REPORT — automatic per-voice check (`galion-voicecheck`)

Finished 2026-11-05 ~23:00 CET (Europe/Zagreb). Rust-first; real numbers only; dry-mode tests (no new renders, no voice writes).
How to run / pipeline: `README.md`. Plug-in contract: `INTERFACE.md`.

## Built
- Crate `galion-voicecheck/` (src: main.rs CLI/orchestration, gate.rs, analysis.rs scorer runner, dsp.rs builtin fixes, stages.rs plug-ins + watcher, report.rs).
- CLI `bin/galion-voicecheck <voice> --base <recording.wav>`; hook `bin/galion-voice-add`; watcher `bin/galion-voicecheck-watch` (NOT started).
- Fix (a) provider auto-detected: A-global `pipeline/emotion-blend-ref/target/release/galion-blend-ref` + `cbx-blend --conditioning-wav` encode. Fix (b): builtin Rust light prosody layer. C-sam dir does not exist (worker C writes to `emotion-blend/candidates/`, no stage exe) → builtin/stub interface documented.

## Tests
- Unit: 16/16 pass (13 gate/naming/protection/watch-filter, 3 DSP) — `logs/unit-tests.log`.
- E2E dry runs (merge-v2 renders as pre-fix; base = clip-store-own/<p>/reference.wav):

| Run | Pre-fix ECAPA / WER | fix a | fix b | fix c | Winner |
|---|---|---|---|---|---|
| own-samantha (`runs/own-samantha/e2e-dry-samantha-merge-v2`) bar 0.7057 | 0.7057 / 1.76 % (fails: not > bar) | ref built, encode+render skipped (dry) | 0.7100 / 2.07 % reject (WER) | skipped (needs a renders) | none |
| own-jarvis (`runs/own-jarvis/e2e-dry-jarvis-merge-v2`) per-voice bar 0.7327 | 0.7331 / 1.85 % | skipped (dry) | 0.7349 / 1.85 % reject (WER) | skipped | none |
| own-samantha + fix-a renders = global-fix coherent (`…/e2e-dry-samantha-fixA-globalfix-coherent`) | 0.7057 / 1.76 % | 0.7081 / 1.46 % KEEP | 0.7100 / 2.07 % reject | 0.7092 / 1.46 % KEEP | c → would save `own-samantha-v2-blend-c` (dry) |
| own-samantha + refprep stand-in a (`…/e2e-dry-samantha-plumbing-standin-a`) | 0.7057 / 1.76 % | 0.6739 / 2.57 % reject | 0.7100 / 2.07 % reject | 0.6771 / 2.57 % reject | none |
| gate-path demo, WER bar relaxed to 0.021 (`…/demo-relaxed-wer-winner-path`) | — | — | 0.7100 / 2.07 % KEEP | — | b → `own-samantha-v2-blend-b` (dry; demo only) |

MOS is reported, not gated: fix a 3.876 / fix c 3.974 vs pre-fix 4.132 in the global-fix run.
- Watcher (temp voices dir + temp state): seeds existing; detects new `own-samantha-v2-newtest` (base resolved), flags `own-zed` NEEDS-BASE, ignores `*-v2-blend-*`.
- Add hook refuses `own-samantha` (protected) and existing names.

## Safety
No renders, no encodes, no voice-file writes by B. own-jarvis sha256 unchanged (73fc04d8…). **own-samantha.cbxvoice changed at 22:56:00 CET by another process** (now 695b7266…, equal timestamp to new `own-samantha-v2-blendfix.cbxvoice` + `own-samantha.cbxvoice.bak-pre-blendfix-20261105`) — not B-check (dry runs only; code refuses protected names).


## Emotion-match update (same night)

See `EMOTION-MATCH.md` and `reports/emotion-match-calib/`. Added full-range emotion moments + leaderboard + blended `emotion_match` gate (pluggable `galion-emotion` backend). Unit tests 27/27. Recommended `emo_min=0.83`. Coherent candidate passes WER+ECAPA on existing renders but **fails** emotion_match (0.8164 < 0.83) due to joyful regression. Laptop package deferred; no new SER model fetched.
