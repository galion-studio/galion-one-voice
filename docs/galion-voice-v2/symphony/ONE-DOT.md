# One dot

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. From the repository root.

One surface. The page is `crates/galion-console/assets/v1-screen.html`, served at `/`. `crates/galion-console/assets/v2-screen.html` only redirects (`location.replace("/")`). The UI lock is one dot, `dot_count` 1, `chrome_allowed` false. This page does not edit `LOCKS.toml`.

The workflow on the page is: tap, mic, `vadFrame` cuts the utterance, `endUtterance` calls `postTurn`, `postTurn` does `fetch("turn")` with a WAV body. That is the auto-record path. There is no second panel.

Persona and empathy on that page are specified in `docs/galion-voice-v2/EMPATHY.md` and the persona files. The HTTP gate that needs a live server is `scripts/eval/v2-persona-selftest.sh`. The symphony check cites it and does not start it.

## Run

```bash
bash scripts/one-ui.sh
bash scripts/run-v1-screen.sh
bash scripts/run-dot-console-demo.sh --headless
```

`scripts/one-ui.sh` prints where the page lives and exits 0. It does not need a Python virtualenv.

`scripts/run-v1-screen.sh` serves `http://127.0.0.1:7878/`. The turn worker sits off the 80 ms thread. `--check` is the server self-test (page, `/dot`, `/mic`, `/turn`) and it builds the console. The symphony self-eval does not start that server.

`bash scripts/run-v1-screen.sh --check` is the command when you want the live hook, after the build cache is warm. The contract test that fires inside `scripts/symphony/self-eval.sh` reads the page source and the lock. It does not open a browser.

## Limits

A green contract test is not a microphone session. Art. 50 disclosure stays on the page. The circle fill stays the locked colour. Empathy moves the wash, not a second face.

— Maciej Grajczyk
