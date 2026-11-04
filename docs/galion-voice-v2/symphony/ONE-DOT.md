# One-dot

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

One surface. The circle is the product. A person speaks, the page records, and the workflow continues. There is no second AI panel.

The page is `crates/galion-console/assets/v1-screen.html`. `GET /v2` redirects to `/`. The circle, the microphone, the text box, and the persona live on `/`. Motion is [ORB.md](../ORB.md) and [ANIMATION-ENGINE.md](../ANIMATION-ENGINE.md). The UI note is [ONE-UI.md](../ONE-UI.md).

```bash
scripts/one-ui.sh
scripts/run-v1-screen.sh           # http://127.0.0.1:7878/
scripts/run-v1-screen.sh --check   # includes the /v2 redirect
```

On `/` the page posts mic energy to `/mic` and, when the person pauses, a WAV to `/turn`. The reply is `/reply/<id>.wav`. Typed lines go to `POST /v2/say` and are spoken on the same page. The check in `scripts/run-v1-screen.sh --check` covers the canvas, `/dot`, `/mic`, `/turn`, the redirect, and a matter-of-fact reply on the grief line.

What happens after the recording is [AUTO-WORKFLOW.md](AUTO-WORKFLOW.md). The mechanism can change. The process is the contract: one circle, then the work.

## Gate

**PASS** when `GET /` is the only screen, a mic turn records without a second panel, `GET /v2` returns the redirect to `/`, and the workflow hook in [AUTO-WORKFLOW.md](AUTO-WORKFLOW.md) fires in the same test.

**FAIL** when a second product UI is served, or when `/v2` renders its own page.

**BLOCKED** when the screen process is not running and the check has not been started.

Record the gate in [SELF-EVAL.md](SELF-EVAL.md).
# One dot

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. From the repository root.

One surface. The page is `crates/galion-console/assets/v1-screen.html`, served at `/`. `crates/galion-console/assets/v2-screen.html` only redirects (`location.replace("/")`). The UI lock is one dot, `dot_count` 1, `chrome_allowed` false. This page does not edit `LOCKS.toml`.

The workflow on the page is: tap, mic, `vadFrame` cuts the utterance, `endUtterance` calls `postTurn`, `postTurn` does `fetch("turn")` with a WAV body. That is the auto-record path. There is no second panel.

Persona and empathy on that page are specified in `docs/galion-voice-v2/EMPATHY.md` and the persona files. The HTTP gate that needs a live server is `scripts/eval/v2-persona-selftest.sh`. The symphony check cites it and does not start it.

## Run

bash scripts/one-ui.sh
bash scripts/run-v1-screen.sh
bash scripts/run-dot-console-demo.sh --headless

`scripts/one-ui.sh` prints where the page lives and exits 0. It does not need a Python virtualenv.

`scripts/run-v1-screen.sh` serves `http://127.0.0.1:7878/`. The turn worker sits off the 80 ms thread. `--check` is the server self-test (page, `/dot`, `/mic`, `/turn`) and it builds the console. The symphony self-eval does not start that server.

`bash scripts/run-v1-screen.sh --check` is the command when you want the live hook, after the build cache is warm. The contract test that fires inside `scripts/symphony/self-eval.sh` reads the page source and the lock. It does not open a browser.

## Limits

A green contract test is not a microphone session. Art. 50 disclosure stays on the page. The circle fill stays the locked colour. Empathy moves the wash, not a second face.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
