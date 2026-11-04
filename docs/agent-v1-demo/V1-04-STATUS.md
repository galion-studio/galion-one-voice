# V1-04 · Runbook + gates · STATUS — DONE (local)

*Galion Voice V1 demo · 2026-11-02 ~14:50 (Europe/Zagreb, UTC+1) · signed Maciej Grajczyk*
LOCAL ONLY. Deploy to 51.38.137.193 is deferred (SSH denied). No push, no gh, no weights.

## Run
```bash
scripts/run-v1-demo.sh              # live: open http://127.0.0.1:7878/ (single UI entry, V1-01 page). Ctrl-C stops
scripts/run-v1-demo.sh --headless   # artifacts + soft checks → galion-demo/out/v1/V1-SUMMARY.{txt,json}
scripts/run-v1-demo.sh --check      # one live turn + curl / /dot /health (bounded), exit 0/1
galion-gates/v1-demo.sh             # soft gate V1 (also called by galion-gates/run-gates.sh)
```

## What I did (hardened V1-02's script, no fork)
`scripts/run-v1-demo.sh` is owned by V1-02 (wiring). My changes are additive and marked `V1-04`:
- **`--headless`**: no ports. Runs locks → speech turn → `run-live-demo.sh --showcase` (Rika opener, workplace hard-OFF) → EN turn (speech WAV fed as the you band) → one-dot PNG frames → `scripts/v1/soft-checks.sh`.
- **Single UI entry = `http://127.0.0.1:$FEED_PORT/` (7878)**, served by `galion-live-demo --web` with V1-01's `SCREEN_HTML`. V1-02's `screen-host.py` mirror on :7870 is now opt-in (`V1_SCREEN_HOST=1`), so only one URL is advertised. The last line is `GALION_V1_SCREEN=http://127.0.0.1:7878/`.
- **One showcase retry** in `--headless` (a concurrent cargo build can swap the shared release binary).
- **Bounded steps**: curl `--max-time 3 --retry 3`, a check-turn watchdog (`V1_TIMEOUT`, default 120 s → exit 124), and a port-busy precheck (exit 69). Exit codes are documented.
- The V1-03 hook keeps the full output and reads `GALION_SPEECH_KIND=`. The speech label comes from `en-cascade.json` `reply_kind` / `speech_backend`. A missing libflite (exit 3) gets an explicit message.
- New `scripts/v1/soft-checks.sh` (C0–C11) and new `galion-gates/v1-demo.sh`. `galion-gates/run-gates.sh` gained soft gate **V1** (in the JSON and summary; `GALION_GATES_NO_V1=1` skips it; it counts only with `GALION_GATES_HARD=1`). G0–G3 are untouched.
- Runbook: `docs/V1-DEMO.md`, which includes the FRXDEL answers, the libflite1 requirement, interim flite with A21 ABSENT, exit 3, and the deploy-pending note.

## Verification (local, 2026-11-02 CET)
- `scripts/run-v1-demo.sh --headless` → `fails=0 soft=0 speech=words`, ~20 s. C0–C8 and C11 PASS, C9 INFO (A21 absent), C10 PENDING (deploy).
- `galion-gates/v1-demo.sh` → `V1 PASS (speech=words)`.
- `FEED_PORT=7978 scripts/run-v1-demo.sh --check` → `PASS check (screen + live /dot fed by Heartbeat+Rika)`.
- `galion-gates/run-gates.sh`: G0–G3 PASS, soft gates PASS. The first V1 run was SOFTFAIL from a concurrent sibling cargo build, and I hardened that with one showcase retry. Re-run: V1 PASS.
- Known limits: the me band is still tone (the words are on the cascade/you side), and Rika lean-in needs `V1_TURN_INPUT=alternate` or padding.

## Wiring summary (from sibling STATUS files)
| Lane | State | Key fact |
|---|---|---|
| V1-01 screen | DONE | one dot, `http://127.0.0.1:7878/`. Modes idle / rika / listening / speaking. Page `crates/galion-console/assets/v1-screen.html` |
| V1-02 wiring | DONE | `galion-live-demo --web :7878` (real 80 ms Heartbeat + Rika per turn), `V1_TURN_INPUT=alternate` (speech/synthetic), pad-wav, fallback `run-v1-screen.sh` |
| V1-03 speech | DONE | EN reply = spoken words via interim **libflite** (cmu_us_slt), A21 slot ABSENT, exit 3 if no speech backend |
| V1-04 runbook | DONE | this file |

## A21 blocker pointer
`docs/agent-v1-demo/A21-BLOCKERS.md` (V1-03) and `docs/agent-wave20/A21-COUNSEL-MEMO.md`. Open items: counsel sign-off (P-A, and whether flite is OK as an interim voice), A21 TTS weights + sha256, the Galion-native decoder (`DECODER_WIRED=false`), Mimi weights (`IS_STUB`), and ASR weights.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
