# FP-04 · E2E test + real-life UX + runbook — STATUS
Galion Voice — Perpetual Open MIT License · © 2026 Maciej Grajczyk · 2026-11-02 15:58–16:20 (Europe/Zagreb, UTC+1)
**LOCAL ONLY.** I made no SSH connection to and no deploy on 51.38.137.193, and I did not push or use gh. Live runs used side ports (17904–17908) so they did not collide with :7878.

## Results (latest tree, after FP-01/02/03 landed)
| Test | Result | rc | Time | Evidence |
|---|---|---|---|---|
| `run-v1-demo.sh --headless` (final) | **PASS** `fails=0 soft=0 speech=words`, C0–C12 PASS | 0 | 18 s | `/tmp/fp04/headless4.txt` |
| `galion-gates/v1-demo.sh` (soft gate, final) | **V1 PASS** (speech=words) | 0 | 17 s | `/tmp/fp04/gate3.txt` |
| `V1_STRICT=1 … --headless` | **PASS** | 0 | 18 s | `/tmp/fp04/strict2.txt` |
| `FEED_PORT=17906 … --check` (live, A21, playback on) | **PASS**: `/health`, `/dot`, `/`; `frames=202 late=0 me_onsets=3 yields=2 lean_in_frames=21` | 0 | 23 s | `/tmp/fp04/final.txt` |
| `/dot` feed sampled every 80 ms during the live check | all 4 modes: idle 103 · rika 14 · listening 34 · speaking 30 | – | – | `/tmp/fp04/dot4.jsonl` |
| Speakers (`parec speakers.monitor` during the live check) | words reached the sink: ≈3.1 s audible in 21.4 s; log says "local playback via paplay on each speaking onset" | – | – | `/tmp/fp04/monitor.raw` |
| `FEED_PORT=17908 … --check` (after my --check fix) | **PASS** | 0 | 21 s | `/tmp/fp04/ok.txt` |
| `--check` with the live build forced to fail (fake cargo) | **FAIL**, as it should be after the fix | 1 | 17 s | `/tmp/fp04/fb.txt` |
| Cascade `GALION_SPEECH=a21` | speech, a21-weights, 1360 ms of audio | 0 | 2.45 s | `/tmp/fp04/casc-a21` |
| Cascade `GALION_SPEECH=flite` | speech, system-flite, 1920 ms | 0 | 2.03 s | |
| Cascade `GALION_SPEECH=off` | `SPEECH=ABSENT` (never tones) | **3** | 1.93 s | |
| Long sample sentence (A21, 72 chars) | 5.76 s audio, played via paplay | 0 | – | `/tmp/fp04/casc-long` |
| Piper direct on CPU | RTF 0.027 (2.3 s audio in 0.06 s infer), 0.47 s wall incl. load, 22.05 kHz | 0 | 0.47 s | `/tmp/fp04/piper.wav` |
| `scripts/a21/fetch-a21.sh --check` (no network) | `A21=PRESENT`, smoke 52 KB PCM in 501 ms on CPU | 0 | – | |

### Earlier runs (tree mid-edit, kept for honesty)
- 16:00 `--headless` → PASS rc 0 in 16 s, but on **flite**: A21 was not wired yet. Soft check C7 claimed "NOT A21" correctly at that time.
- 16:00 `--check` → rc 0 **while the live build was failing**. FP-02 was mid-edit (`DotSink::new` type error), and the synthetic fallback was reported as PASS. That is bug B1 below.
- 16:01 `--check` (A21 present, before FP-02's `--reply`) → PASS, but the dot went only `idle › listening › idle › speaking › idle`. There was no rika, because the words were in the you band.
- 16:03 2-turn alternate run → all 4 modes seen, then **rc 2**: a sibling rewrote `run-v1-demo.sh` while bash was executing it ("unexpected EOF"). That is an edit race, not a product bug, and it did not recur.

## Bugs fixed by FP-04
| # | Bug | Fix |
|---|---|---|
| B1 | `--check` returned PASS (rc 0) when the live build failed and only the synthetic fallback page ran | `run-v1-demo.sh`: the fallback under `--check` still runs the screen check, then **exits 1** with "Heartbeat+Rika+words NOT verified". Verified both ways |
| B2 | Soft check C7 always said "(interim, NOT A21, NOT neural)", even when A21 spoke | The label now comes from `speech_backend`: "A21 neural CPU (Piper VITS)" or "interim flite" |
| B3 | C11 checked only libflite, not the A21 runtime | C11 PASS = Piper runtime + `weights/tts/*.onnx` when A21 is used, otherwise libflite. I also fixed the `ROOT: unbound variable` crash my first edit introduced |
| B4 | C9 said "DECODER_WIRED=false, no weights on disk" and "me band IS_STUB tone" while A21 was present and the me band was the words | C9 is conditional: "A21 cascade TTS built … me band = these words (--reply); Mimi codec still IS_STUB" |
| B5 | The headless EN turn fed the words with `--in` (you band, so the dot showed *listening*). The gate never exercised the real speaking path | Headless EN turn → `--reply <wav> --loops 1 --mimi off`, the same as the live path. New check **C12** asserts `voice=reply-wav` |
| B6 | The runbook said "no weights", lacked C12 and a side-port example, and called the Piper weights "BLOCKED" in §7 | `docs/V1-DEMO.md` updated (small edits, still one dot) |

## Files touched
`scripts/run-v1-demo.sh` (headless EN turn, `--check` fallback rc) · `scripts/v1/soft-checks.sh` (C7/C9/C11 labels, C12) ·
`docs/V1-DEMO.md` · new `FP-04-UX.md`, `FP-04-STATUS.md`, `FP-SUMMARY.md`. The backup of soft-checks is at `/tmp/fp04/soft-checks.sh.bak`.

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
