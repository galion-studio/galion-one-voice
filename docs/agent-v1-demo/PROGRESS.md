# V1 demo · Progress · 2026-11-02 (Europe/Zagreb)

One command: `scripts/run-v1-demo.sh` → **http://127.0.0.1:7878/** (one dot). Runbook: `docs/V1-DEMO.md`.
LOCAL ONLY. Deploy target 51.38.137.193 is PENDING SSH access.

| ID | Lane | Deliverable | State |
|---|---|---|---|
| V1-01 | Screen | `scripts/run-v1-screen.sh`, single one-dot page at `/` | DONE (V1-01-STATUS.md) |
| V1-02 | Wiring | `scripts/run-v1-demo.sh` live loop (heartbeat + Rika + speech turn → :7878) | DONE (V1-02-STATUS.md) |
| V1-03 | Speech | EN cascade speaks words (interim flite); A21 slot ABSENT; A21-BLOCKERS.md | DONE (V1-03-STATUS.md) |
| FP-03 | TTS / audio | A21 BUILT: Piper VITS en_US-ljspeech-medium on CPU (MIT, sha256-pinned, `scripts/a21/fetch-a21.sh`); flite = fallback only; reply words play to speakers when speaking starts | DONE (FP-03-STATUS.md) |
| V1-04 | Runbook + gates | `--headless` / `--check` hardening, `scripts/v1/soft-checks.sh`, soft gate V1, `docs/V1-DEMO.md` | DONE (V1-04-STATUS.md) |

FRXDEL answers (FRXDEL-ANSWERS.md): Q1 YES plumbing · Q2 NO, it must say words (met with interim flite) · Q3 YES, A21 funded · Q4 YES 51.38.137.193 · Q5 YES one-dot is final.

## Gates (local)
`galion-gates/run-gates.sh` at 14:43 CET: G0–G3 PASS (G1: 326 tests), soft G4/G6/PS/LK/DG/LV PASS, R0–R3 PENDING (no VPS), fails=0.
The first V1 run was SOFTFAIL: a concurrent sibling `cargo build` swapped the live binary mid-showcase. I hardened it with one retry. Re-run `galion-gates/v1-demo.sh`: **V1 PASS (speech=words)**.
`scripts/run-v1-demo.sh --headless`: `fails=0 soft=0 speech=words`.
Limits (16:10 CET, FP-03): the me band is the A21 reply words (`--reply`, 1360 ms), played to the speakers. A21 is PRESENT (Piper, CPU). Still open: the LOCKS onnx exception needs ratifying, espeak-ng GPL-3 needs counsel, Mimi IS_STUB, StubAsr, MockLlm. See A21-BLOCKERS.md.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
