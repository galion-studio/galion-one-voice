# A14 — Demo = Spec = Test = Teacher (MIT) — re-pinned 2026-11-02

## Golden
- `./golden.sh` = `galion_demo.py selftest` + `gen --canned` + 4 canned runs on the same input, then sha256 over all outputs → `out/canned/demo.artifact`, pinned in `galion-gates/golden/demo.sha256`.
  | variant | flags | expected |
  |---|---|---|
  | default | (none) — consent bits 0 | no reading, neutral-warm, Rika OFF |
  | opener | `--consent sense,adapt --purpose personal --rika-opener` | counterbalance (F) + Rika wordless-hum opener |
  | workplace | `--consent sense,adapt --purpose workplace --rika-opener` | **hard OFF**: wav byte-identical to default (checked with `cmp`) |
  | education | `--consent sense,adapt --purpose education --rika-opener` | **hard OFF**: same |
- Current pin: **`63133d731231fb5f55aa0945453c597002be6348c9f677f0257793d54ba6c8a7`** (previous `d21cad79…964f` retired: counterbalance + opener + KD changed behaviour; re-pinned in the same change set).
- `./golden.sh --write` re-pins; only with a reviewed behaviour change. Capture line `RTF=canned (not a model benchmark …)` (decision J). `GALION_MODEL` is dropped in canned mode (verified: golden passes with it set).
- Gate wiring: `galion-gates/run-gates.sh` soft gate **DG** (`soft DG "$ROOT/galion-demo/golden.sh"`), in JSON + summary; counts as FAIL only with `GALION_GATES_HARD=1` (same rule as G4/G6/PS/LK). A16 to confirm.
- Tests: `python3 -m unittest discover -s tests` (7 tests: selftest, angry de-escalation, joy shared below, no-consent neutral-warm, default-deny + prohibited purposes, opener + hard-off end-to-end, determinism).

## Counterbalance fix (decision F) — LANDED
Port of galion-emotion `StubExpressor::respond` + `plan` + `film::check_counterbalance` + `EmotionFrame::label` (A09 §4 table). The agent band's pitch (`220 Hz · 2^(st/12)`) and level (`energy`) are driven from Galion's **target**, never the caller frame. Per-turn caller frame = mean of the turn's 20 ms stub frames (D 0.5, conf 0.3).
Canned demo turns (opener variant): neutral → neutral-warm; excited → shared slightly below (V +0.55 A 0.51); angry (V −0.71 A 0.79) → V +0.20 A 0.20 D 0.35, 250 ms pre-pause. Old mirror behaviour is rejected on 2/3 turns (and on the selftest's anger/anxiety/sadness cases); joy mirroring still passes.

## Failing specs → KD targets — LANDED (`--kd-out`)
One JSON line per failing spec instance, sorted keys, deterministic:
`{"band":"me|feeling","canned":{…},"expected":{…},"frame":<n>,"frame_ms":20|80,"spec":"<id>","teacher":"moshiko|sensevoice-small|r-fixture","teacher_output":null}`
`teacher_output` is always `null`: no teacher has been run and no weights are on the box — the row says *what* to distil, not an invented answer.
| spec | canned result (opener variant) | KD rows |
|---|---|---|
| F-counterbalance | PASS 3/3 (now a regression check, not a KD target) | 0 |
| D-onset p50 ≤ 300 ms | PASS p50 280 ms (angry turn 520 ms incl. deliberate 250 ms pre-pause) | 0 (rows emitted per turn if p50 fails) |
| E-in→A-out ≤ 2 heartbeats | FAIL: fixed 12×20 ms lag = 3 heartbeats | 3 (feeling, moshiko) |
| D-language | FAIL: tones, not speech | 3 (me, moshiko) |
| emotion-ser | FAIL: RMS/ZC heuristic | 3 (feeling, sensevoice-small) |
| E-Rika-opener | PASS (evidence hb 41 → armed 42 → hum 54 → onset 57, caught) | 0 (row if a check fails) |
No-consent / prohibited variants emit 3 rows (D-language only); affect specs are SKIP ("no reading").

## Demo opener (Rika) — LANDED, consent-gated, OFF by default
- Input gets a Rika segment: utterance 1 (0.3–1.5 s) → reply → silence → **breath 3.3–3.6 s** (unvoiced, high ZCR) → hesitation → utterance 2 at 4.6 s.
- `Rika` class = port of `rika.rs` `Rika::step` on the RMS/ZCR stub path, same v0 constants (Q 6, hysteresis 2, T 25, D 4, R 100, shift 6 dB, breath ZCR ≥ 0.3, voiced RMS ≥ 0.02 & ZCR ≤ 0.25, soft ack after 12 armed frames, max one per refractory, never before the user has spoken, never while Galion speaks). Click track (3.125 heartbeat period) can never satisfy the 2-frame hysteresis.
- Cue: lean-in (prosody only, held me-band onset while ARMED) and one **SoftBreath** rendered as a wordless 80 ms hum ("mm", Hann envelope, ~14–17 dB below reply). There is no lexical cue.
- Gate: `--rika-opener` (feature flag, default off; also turns on the opt-in soft ack) **and** SENSE+ADAPT **and** purpose not workplace/education. If the gate fails, `step()` never calls the feature closure — the user band is not read.
- Rust `galion-emotion` R1–R9 (G6) remain authoritative; this is the canned demo rendering only.

## Still open
- E-in→A-out, D-language, emotion-ser: need a teacher run (Moshiko / SenseVoice-Small) → blocked on weights + licence review (A21). KD rows are ready to be filled.
- Demo does not call the real Rust Rika/heartbeat (pure-stdlib by design); a later FFI/subprocess hook can replace the port.
