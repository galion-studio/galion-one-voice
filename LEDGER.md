# LEDGER — Galion Voice V4 (A20)
*Seeded 2026-11-02 00:40 CET; refreshed 2026-11-02 ~00:55 CET; 2nd refresh ~01:05 CET (Europe/Zagreb, UTC+1) from every v4/agents/A*-STATUS.md. Not a git repo: "commit/path" = file path.*

| Agent | Wave | Task | Status | Commit / path | Gate result | Wave-2 job in flight |
|---|---|---|---|---|---|---|
| A1 | W0 | Locks | landed + verified; G2 forbid strings reworded (W2-G2 DONE) | LOCKS.toml, scripts/check-locks.sh, v4/agents/W2-G2-STATUS.md | LK PASS; G2 PASS (0 refs) | J1 DONE |
| A2 | W1 | OS Voice image spec | landed (doc); X4 README merged LOCALLY | v4/agents/A02-galion-os-voice-image.md, v4/docs/GALION-OS-README.md | n/a | J9 local DONE; public-repo push pending / in flight |
| A3 | W1 | VoiceBackend slot | landed (doc + slot v2 toml); trait names [verify] | galion-student/configs/openmind-slot.toml | n/a; configs/*.toml carry forbid strings → G2 | J10 verify traits vs galion-voice-core |
| A4 | W1 | SIMD/int8 GEMV | landed | crates/galion-tensor/src/{simd,gemv_i8}.rs, benches/gemv.rs | G1 bit-exact vs scalar PASS | — |
| A5 | W2 | Conv stream / attention / KV ring | landed + verified incl. strided + transposed conv | crates/galion-tensor/src/{conv_stream,attention,kv}.rs (+ resample conv) | 60 unit + 5 integration pass; workspace green | J6 DONE |
| A6 | W2 | Mimi stub | landed (stub, IS_STUB=true); real kernels not yet reported | crates/galion-mimi/ | 6 tests pass | J7 IN FLIGHT (no W2 update in A06-STATUS.md) |
| A7 | W1 | .gq reader + spec | landed (reader); writer BLOCKED | crates/galion-tensor/src/gq.rs, v4/agents/W2-A07-STATUS.md | 8 gq tests pass; G5 undefined (no writer) | J8 BLOCKED — weights URIs/sha256 TBD (A21) |
| A8 | W1 | Student KD plan | landed (doc + s0/s1 toml) | v4/agents/A08-student-kd-plan.md | n/a | — (blocked: H6 GPU budget, A21 legal, A15 R0) |
| A9 | W2 | AffectStore / FiLM | landed | galion-emotion/src/{store,film}.rs | 46 unit + 2 doc pass | — (demo mirror fix → A14) |
| A10 | W3 | Rika + G6 + heartbeat | landed; real heartbeat step() wired | galion-emotion/src/rika.rs, galion-heartbeat, galion-gates/g6-rika.sh, specs/presence.json P6 | G6 PASS 9/9; heartbeat 14 tests; lean_in_ms=160 (stub-measured); VPS latency PENDING | J3 DONE |
| A11 | W2 | Live console | landed; real tensor/emotion feeds | crates/galion-console/src/live.rs | crate tests pass | J5 DONE (mic/loudness stubs = separate job) |
| A12 | W1 | One-dot render + NullSurface | landed | galion-render/src/{palette,dot}.rs | 17 / 21(full) pass; G4 golden blessed | — |
| A13 | W1 | Presence harness | landed; PS gate in runner + WAV loader | specs/presence*.json, crates/galion-presence/src/wav.rs | PS PASS (6+2 specs); 22 tests pass | J4 DONE |
| A14 | W2 | Canned demo golden | landed; re-pin not yet reported | galion-demo/golden.sh, galion-gates/golden/demo.sha256 (d21cad79…) | golden stable; not in runner | J2 IN FLIGHT (no W2 update in A14-STATUS.md) |
| A15 | W1 | Probe v0.1.0 | landed (local) | galion-probe/, LOCKS.toml [probe] | G0 PASS(warn) G1 PASS G3 PASS; R0 PENDING (no SSH) | — |
| A16 | W2 | Gates + runner | landed + verified (local) | galion-gates/{g4-render.sh,g6-rika.sh,run-gates.sh} | latest run 00:57: all PASS | J12 IN FLIGHT (A16-STATUS.md not yet updated) |
| A17 | W2 | Hermes sideband types | landed (docs + types) | crates/galion-sideband/ | 1 test pass | — (C5/C6 owner decisions) |
| A18 | W2 | Cascade lane | landed (design doc) | v4/agents/A18-cascade-lane.md | n/a | — (A21 licence, E9/K15/K16 owner) |
| A19 | W1 | Branding | not in this wave (PR #3 assets) | PR #3, NOT touched | — | — |
| A20 | W3 | Ledger + integration | landed; 2nd refresh | LEDGER.md, v4/agents/A20-* | see gates line | J11 DONE |
| A21 | W0 | Licence/weights memo | landed (draft, needs counsel) | v4/agents/A21-licence-weights.md | n/a | — (A5/A4/K8/W4 owner/counsel) |
| A22 | W0 | Deploy posture | landed (dry-run scripts); VPS deploy BLOCKED | scripts/deploy/{harden,release}-dryrun.sh | --execute exits 2 | — (no SSH; root pw rotation + key install need a human) |

**Gates (latest run `galion-gates/results/20261102T005720+0100.json`, 00:57 CET):** G0 PASS · G1 PASS · G2 PASS · G3 PASS · G4 PASS · G6 PASS · PS PASS · LK PASS · G5 not in runner (writer blocked) · **R0–R3 PENDING (no SSH)**.
**PR #3 not touched. No GitHub writes. No VPS.**

**2026-11-02 16:10 CET · FP-03 · owner decision t123u: A21 = Piper (CPU, MIT).** The A21 TTS slot now speaks via the Piper VITS `en_US-ljspeech-medium` voice.
- The pack is `.onnx`, sha256 `6f52a751…7f22`. The runtime is piper 2023.11.14-2 with onnxruntime 1.14.1, run out of process on CPU.
- This is a scoped exception to `formats.foreign_runtime_allowed=false` and `weight_formats`. LOCKS.toml is **not** amended (pending ratification).
- espeak-ng is GPL-3, pending counsel.
- Flite is the fallback only. Weights are not vendored (`scripts/a21/fetch-a21.sh`).
- See docs/agent-v1-demo/A21-BLOCKERS.md and FP-03-STATUS.md.

**2026-11-02 · Galion Voice V2 program · Plan V5.** Phone path from the local V1 delta landed on mainline (`PhoneLane`, `CannedLlm`, `TurnWorker`, `PhoneHost`, `POST /turn`, `GET /reply`, mic tap). V2 orb foundation is the same page: spring-damper, mic energy, playback envelope. Docs: `docs/galion-voice-v2/GALION-VOICE-V2.md`, `docs/ops/PLAN-V5.md`, `docs/brand/GALION-VOICE-BRAND.md`. LOCKS.toml values not edited. No SSH. No VPS deploy. ASR still unratified (A/B/C). `GALION_ASR=interim` remains a labelled placeholder. Plan V4 dry-runs stay the deploy refusal (`--execute` exits 2).

**2026-11-02 · V2-S2 animation engine (foundation).** Design: `docs/galion-voice-v2/ANIMATION-ENGINE.md`. Rust spring-damper in `galion-render` (`spring`, `OrbEngine`): radius and brightness, variable dt, display rates 60/120/240, fed by `DotParams` targets. Not called from `Heartbeat::step` or `NullSurface`. G4 golden not re-blessed. No wgpu. No video feature. LOCKS.toml not edited. No white paper in the tree.

**2026-11-03 · Presence-eval virtual user (T0–T2, loopback).** `scripts/eval/virtual-user.sh` and `galion-presence-eval`. Injects text (`text-inject`, not recognition) or a fixture WAV into `PhoneLane`, replies with `CannedLlm` and Piper or flite when one is installed, captures the reply WAV, and writes `galion-gates/results/presence-eval/<stamp>.json` (also `latest.json`). Scores task facts, `TurnWorker::supersede` cancel, and the interim label (`interim-placeholder`, `claims_recognition` false). TTFA is filled only from a captured WAV and is labelled `loopback-full-utterance`, not PSTN. `pstn` stays `not_run`. No engagement metric. LOCKS.toml not edited. No SSH. Doc: `docs/eval/PRESENCE-EVAL.md`.

**2026-11-03 · V2-S3 secure edge (domain 07).** `GALION_CORS_ORIGINS` exact allow-list; `Access-Control-Allow-Origin: *` only when the variable is unset (local V1). Foreign `Origin` → 403 and no allow-origin header. No WebSocket: session token is minted by `POST /session` and returned in the body; a token in the URL is 400. Loopback bind unchanged; `0.0.0.0` refused unless `GALION_ALLOW_PUBLIC_BIND=1` plus `GALION_REQUIRE_SESSION=1` and an exact origin list (not set here). ngrok staging auth documented (OAuth or basic; no secrets in the repo). Turn PCM zeroised on drop; phone log has no PCM or transcript. AI Act Art. 50 first-turn disclosure, Croatian and English, on for the demo (`GALION_AI_DISCLOSURE=0` strips it). Tests: `cors_allowlist_is_exact_and_star_is_default`, `foreign_origin_is_rejected_without_acao`, `responses_carry_edge_security_headers`, `refuses_public_bind`. No SSH. LOCKS.toml not edited. No public bind.

**2026-11-03 · V2-S6 warm Piper.** One out-of-process Piper worker per `PhoneLane` (`--json-input --output_raw`; the 2023.11.14-2 runtime's stderr line `Real-time factor:` frames the raw PCM). No disk PCM cache. No in-tree ONNX Runtime. LOCKS.toml not edited. Flite stays the fallback when the A21 pack is absent. `GALION_SPEECH=off` still publishes the two-tone WAV with `backend=tone` and a `NoSpeech` error. The first clause flushes at punctuation or 8 words (`SpeechStack::begin_stream`). `PhoneLane::new` pre-warms with a discarded "Hello." so the first caller turn is warm. `TurnWorker::spawn` returns after that pre-warm, so the one-slot queue is empty. A superseded turn drops clauses not yet sent and does not kill the process. Bench: `scripts/bench-piper-warmth.sh` (does not pre-warm; prints JSON; invents no number when the pack is missing). Three runs on this build machine, voice `en_US-ljspeech-medium`, phrase `Hi! This clause is the warm path check.`, clock = speak entry until first-clause PCM (`Hi!`): cold 413 / 357 / 411 ms, warm 26 / 25 / 24 ms, `piper_spawns=1` each run. Not a phone or VPS measurement, and not a product latency claim.
