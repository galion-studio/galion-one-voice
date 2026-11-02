# First-principles pass (FP-01…04) · Summary
Galion Voice — Perpetual Open MIT License · © 2026 Maciej Grajczyk · 2026-11-02, finished ≈16:20 (Europe/Zagreb, UTC+1)
**LOCAL ONLY.** I did not deploy, SSH or push. The interface is **one dot** at `http://127.0.0.1:7878/`.

## TL;DR
- The V1 demo is one command, one process, one port and one dot.
- The reply is now real neural speech (A21 = Piper VITS on CPU). It **plays out loud**, and the dot is in the *speaking* state for exactly as long as the words play.
- All four dot modes (idle, rika, listening, speaking) were seen live on the feed.
- Every local test passed: `--headless` rc 0, `--check` rc 0, the soft gate passed, and the strict run passed.
- Still open: the A21 licence/LOCKS items, the stubbed ASR/LLM/Mimi, no microphone, and the VPS (SSH denied).

## 1. Deleted / simplified (FP-01-DELETED.md, FP-02 §5, FP-03)
**Second screen and extra surfaces removed:**
- `scripts/v1/screen-host.py` and the whole `:7870` mirror (`V1_SCREEN_HOST`, `SCREEN_PORT`, `V1_SCREEN_DIR`). FP-01.
- The `--web` launcher in `run-dot-console-demo.sh`, which was a second launcher on :7878. FP-01.
- The `dot-viz-window` feature and the `--window` stub. FP-02.

**Render and server trimmed:**
- The PNG writes in `run-v1-screen.sh` (FP-01) and in `--web` mode (FP-02).
- The double-browser `--open` bug, which could open two windows. FP-01.
- `GET /frame` and `latest_json()`, which had no consumer. The HTTP surface is now `/`, `/dot` and `/health` only. FP-02.

**Turn plumbing simplified (FP-02):**
- Each turn used to re-exec the binary and rebind the port. Now there is **one process**: `galion-live-demo --reply … --loops N`.
- `V1_TURN_INPUT` and the alternate mode are gone.
- `scripts/v1/pad-wav.py` and the per-turn `turn-N/` directories are deleted.
- The unbounded `DotSink.lines` buffer is deleted.

**Dead code removed:** the `V1_SPEECH_CMD` / `scripts/v1/speech.sh` branch, which pointed at a file that never existed. FP-03.

**Kept by design:**
- The background wash, which LOCKS requires.
- The synthetic you band, which is the only source of the Rika lean-in until there is a microphone.
- The dev TUI and console panels, which are not on the V1 path.

## 2. Sample-voice test results (FP-03 + FP-04)
**Voice and speed:**
- **A21 (default):** Piper `en_US-ljspeech-medium` on CPU. "you said: hello galion" becomes 1.36 s of audio, 24 kHz.
- **ASR check:** offline ASR heard "you said hello gallon".
- **Speed:** RTF ≈ 0.03–0.05 and about 0.5 s cold per call. The full cascade takes about 2.4 s.
- **Determinism:** the output is deterministic (same sha256 across runs).
- **Long sentence:** 72 chars gives 5.76 s and plays cleanly.

**Fallbacks:**
- `GALION_SPEECH=flite` gives 1.92 s of robotic words, rc 0.
- `GALION_SPEECH=off`, or `a21` with no pack, gives **exit 3** (`SPEECH=ABSENT`). It never plays tones as the reply.

**Speakers:** `play.rs` (paplay, pw-play, aplay or ffplay) plays the WAV on every speaking onset and kills it on barge-in.
- A capture of the PulseAudio `speakers.monitor` showed the words reached the sink, about 3.1 s audible per 21 s check.
- An ASR pass over the monitor capture heard the words (FP-03).

**Dot feed (live, 80 ms polling):** `idle › rika › listening › idle › speaking › idle › rika › listening › idle › speaking › listening (barge-in) › idle › speaking › idle`.
- Per loop: 202 frames, `late=0`, `me_onsets=3`, `yields=2`, `lean_in_frames=21`, Rika lean-in 160 ms.

**Test runs (final tree):**

| Run | Result | Time |
|---|---|---|
| `--headless` | rc 0 (`fails=0 soft=0 speech=words`, C0–C12 PASS) | 18 s |
| `galion-gates/v1-demo.sh` | V1 PASS | 17 s |
| `V1_STRICT=1` | rc 0 | 18 s |
| `--check` on a side port | PASS | 21–23 s |
| `fetch-a21.sh --check` | A21=PRESENT | – |
| cargo tests (FP-02/03) | green | – |
| `check-locks.sh` and `check-no-forbidden.sh` | OK | – |

## 3. Every bug fixed
**FP-01:**
1. The double-browser `--open` could open two windows.
2. The `:7870` mirror competed with the one URL.

**FP-02:**
3. The reply words showed as *listening*. They were in the you band; they are now in the me band, shown as *speaking*.
4. The 640 ms mock reply truncated the words. Fixed with `me_planned` from `ReplyVoice::planned()`.
5. When the port was busy, the demo silently fell back to PNG-only and the dot sat idle forever. It now exits 69.
6. Every turn re-executed the binary and rebound the port, leaving a ~1 s gap with no feed.
7. A forever loop could hot-spin on a failing binary.
8. A sibling `cargo build` could swap the binary mid-run. The demo now runs a private copy.
9. The dot clock reset each loop, which froze the feed decimator. It is now monotonic.
10. `DotSink.lines` grew without limit.
11. PNGs were written every frame in web mode.

**FP-03:**
12. **A21 was ABSENT.** It is now built: Piper VITS plus a CPU runtime, sha256-pinned and fetched by `scripts/a21/fetch-a21.sh`, with an honest `present-verified` status.
13. **Nothing played the reply aloud in the live demo.** `play.rs` now does, stopping on barge-in.
14. Stale "flite interim / A21 ABSENT" text in docs and labels; the `V1_SPEECH_CMD` dead branch.

**FP-04:**
15. `--check` reported PASS when the live build failed and only the synthetic fallback ran. It now exits 1.
16. Soft check C7 labelled A21 speech "NOT A21, NOT neural".
17. C11 checked only flite; it now checks the A21 runtime too. (I also fixed an unbound `ROOT` crash I introduced along the way.)
18. C9 still said "no weights on disk" and "me band = tone".
19. The headless EN turn used `--in`, which is the you band, so the gate never tested *speaking*. It now uses `--reply`, and a new check C12 covers it.
20. Runbook drift.

## 4. Parallel agent findings
- **FP-01 (evaluate/delete):**
  - There is one page with one canvas and no text nodes.
  - Complexity is concentrated in the dev TUI (about 2.9k lines, not user-facing) and in a 162-line orchestrator.
  - It flagged that the live demo never played audio. FP-03 fixed this.
- **FP-02 (backend wiring):**
  - The me/you band mix-up was the root cause of "speaking looks like listening".
  - The whole live path is now one 80 ms Heartbeat + Rika process. Max step is 14 µs and the frame budget is never exceeded.
- **FP-03 (TTS/A21):**
  - A21 is built on CPU with no GPU and no Rust `ort` crate, running out of process.
  - `auto` tries A21, then flite, then exit 3.
  - Playback is wired. Determinism and ASR intelligibility were verified.
- **FP-04 (E2E/UX):**
  - E2E is green on the final tree and all 4 modes appear on the feed.
  - Real-life gaps (see `FP-04-UX.md`):
    - There is no microphone; the you band is scripted.
    - Rika fires only when there is audible breath.
    - The reply is an echo ("you said: …").
    - Listening and speaking look nearly identical without sound (radius ranges overlap).
    - There is a small player start-up lag that I did not measure.
    - A crash looks like idle.

## 5. What remains broken / open
1. **A21 licensing/LOCKS (owner + counsel):**
   - `.onnx` plus the out-of-process onnxruntime is an exception to `formats.foreign_runtime_allowed=false`. It is logged as t123u in LEDGER, but LOCKS is not amended. Needs owner ratification.
   - **espeak-ng (GPL-3)** is Piper's phonemizer and needs a counsel decision.
   - The LJSpeech voice choice and consent need confirming.
   - The Galion-native `.gq` decoder is not built (`GALION_NATIVE_DECODER_WIRED=false`).
   - The runtime is pinned to **linux x86_64 only**.
2. **Stubs:**
   - Mimi codec `IS_STUB` (the demo runs `--mimi off`). Mimi and student weights are still blocked on the A21 counsel memo.
   - StubAsr, MockLlm (echo) and FixedEmotion.
   - **No microphone.**
3. **Deploy:** SSH to **51.38.137.193** is denied, so nothing is deployed. The release gates R0–R3 are PENDING. The page binds to loopback only and there is no reverse proxy.
4. **UX:** listening and speaking are hard to tell apart visually, and a crash is indistinguishable from idle. Both are owner calls under the one-dot lock.

## 6. Language breakdown (one line)
- **Rust:** about 23k LOC across 13 crates, covering the Heartbeat, Rika, live loop, cascade, A21 slot, playback and the std-only HTTP server.
- **Shell:** about 1k lines of orchestration and gates (`run-v1-demo.sh`, `soft-checks.sh`, `fetch-a21.sh`, gates).
- **HTML/JS:** one 65-line page (one `<canvas>`, polls `/dot` every 80 ms).
- **External binaries:** Piper, onnxruntime and espeak-ng (CPU, out of process), with flite as the fallback.
- **Python:** only the legacy `galion-demo/`, which is not on the V1 path.

Details: `FP-01-STATUS.md`, `FP-01-DELETED.md`, `FP-02-STATUS.md`, `FP-03-STATUS.md`, `FP-04-STATUS.md`, `FP-04-UX.md`, `A21-BLOCKERS.md`, `docs/V1-DEMO.md`.
— Maciej Grajczyk
