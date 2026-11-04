# FP-01 · Deleted / disabled (one-dot only)
Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

Rule: the interface is ONE solid dot on near-black at `http://127.0.0.1:7878/`. Anything else goes.
Backups of the pre-FP-01 files are in `/workspace/fp01-backup/` (box scratch, outside the repo).

| Removed / disabled | Why |
|---|---|
| `scripts/v1/screen-host.py` (deleted, 119 lines) | Second port `:7870` mirror. It had its own CSS fallback page, `/audio.wav`, `/v1/state`, and a static file server. It competed with the one URL, and nothing used it by default. |
| `V1_SCREEN_HOST`, `SCREEN_PORT=7870`, `V1_SCREEN_DIR`, `V1_STATE_FILE` export in `scripts/run-v1-demo.sh` | These only fed the deleted mirror. `galion-demo/v1/screen/` never existed. The port check now covers only `:7878`. `state.json` is still written because `soft-checks.sh` C5 reads it. |
| `--web` in `scripts/run-dot-console-demo.sh` (now exits 64 with a pointer) | It was a second launcher on `:7878` that competed with `run-v1-screen.sh`. The script stays for headless PNG frames, which the gate uses. |
| PNG writing in `scripts/run-v1-screen.sh` (`--png-dir/--png-latest-only` → `--no-png`) | The screen does not need it. It caused disk I/O on every frame. |
| `/dot` JSON echo in `run-v1-screen.sh --check` and the trailing log tail | Verbose status output. The check still verifies that the page has one canvas and that `/dot` cycles through its modes. |
| Double-browser bug in `run-v1-screen.sh --open` | It could open chromium AND firefox (two windows). Now only the first browser found opens. |
| `docs/V1-DEMO.md` row for `V1_SCREEN_HOST`/7870; the "optional web" mention in the galion-console README | These describe removed things. |

**Not touched (by design):** A21 neural TTS slot and flite. FP-03 builds A21 (CPU-only, MIT, e.g. Piper) to replace flite, so the slot is NOT flagged for removal. Heartbeat/Rika core logic is FP-02's. `v1-screen.html` was already one canvas and no text, so it is unchanged.

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
