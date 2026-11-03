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

— Maciej Grajczyk
