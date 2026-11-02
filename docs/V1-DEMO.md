# Galion Voice — V1 demo runbook

Perpetual Open MIT License · © 2026 Maciej Grajczyk · 2026-11-02 (Europe/Zagreb, UTC+1)
**LOCAL ONLY.** Deploy to 51.38.137.193 is deferred: SSH was denied. No push, no gh, no training. The only weights are the A21 Piper TTS pack (`scripts/a21/fetch-a21.sh`).

## 1. One command

```bash
cd /workspace/galion-voice-code
scripts/run-v1-demo.sh               # live demo: open http://127.0.0.1:7878/  (Ctrl-C stops)
scripts/run-v1-demo.sh --headless    # no ports/display: artifacts + soft checks → galion-demo/out/v1/V1-SUMMARY.txt
scripts/run-v1-demo.sh --check       # one live loop (~16–23 s), curls / /dot /health on :7878, exit 0/1
FEED_PORT=17906 scripts/run-v1-demo.sh --check   # side port when :7878 is in use
```

**Single UI entry (V1-01): `http://127.0.0.1:7878/`.** That is the same page as `scripts/run-v1-screen.sh`.
In the full demo, `galion-live-demo --web 127.0.0.1:7878` serves it and drives it with the real live loop.
Press `F` or double-click for fullscreen. Nothing else opens: no second window, no captions.

| flag / env | meaning |
|---|---|
| `--loops N` | live loops of ONE `galion-live-demo` process (0 = forever, the default; ~16 s per loop) |
| `--text "hello galion"` | EN cascade input text |
| `--consent sense,adapt` · `--purpose personal` · `--no-opener` | consent bits / purpose / Rika opener off |
| `--headless [--seconds N]` | V1-04 gate path. Writes `live/{default,opener,workplace,en-turn}`, `screen/` PNGs, `V1-SUMMARY.{txt,json}` |
| `V1_STRICT=1` | soft-check failures (e.g. tone instead of words) exit 1 |
| `FEED_PORT=7878` | the one UI/feed port (FP-01 deleted the `:7870` screen-host mirror and `V1_SCREEN_HOST`) |
| `GALION_PLAY=0` / `GALION_PLAYER="cmd args"` | FP-03 local speaker playback of the reply words: off, or override the player. The default is the first one found of paplay, pw-play, `aplay -q` and ffplay (`galion-live-demo --no-play` also turns it off) |
| `V1_TIMEOUT=120` | per-step timeout (s) |

Exit codes: `0` ok · `1` check / soft-check FAIL · `64` bad arg · `69` port busy (`pkill -f galion-live-demo`) · `124` timeout.
Last stdout line: `GALION_V1_SCREEN=http://127.0.0.1:7878/` (live) or `GALION_V1_DEMO_OUT=<dir>` (headless).

### Prerequisites
- Rust stable + cargo (`--offline` builds), python3, curl, coreutils `timeout`.
- **A21 voice pack (FP-03, CPU only, MIT):** run `scripts/a21/fetch-a21.sh` once. It fetches Piper 2023.11.14-2 and the
  `en_US-ljspeech-medium` voice into `weights/` (sha256-pinned, not vendored); `--check` verifies them without network.
  - **Fallback only:** `libflite1` (`apt install libflite1`) is used when the A21 pack is missing.
  - **Neither present:** the EN cascade **exits 3** (`SPEECH=ABSENT`) and never passes tones off as the reply. Check C11 reports this.
  - **Options:** `GALION_SPEECH=auto|a21|flite|off` (default `auto` = A21 first), `GALION_A21_RUNTIME=<piper>`, `GALION_A21_NOISE=natural`, `GALION_FLITE_VOICE=slt|…`.
- Speakers: PulseAudio/PipeWire (`paplay`) or ALSA (`aplay`) for the reply words.

## 2. What runs behind the one screen

```
[0] scripts/check-locks.sh            V4 Eureka locks: 80 ms clock, one dot, consent deny-by-default
[1] speech turn → WAV                 EN cascade: StubAsr → MockLlm "you said: …" → **spoken words** via A21
                                      (Piper VITS ljspeech, CPU, ~0.5 s) · flite only if A21 missing · exit 3 if no speech
[2] galion-live-demo --web :7878      ONE process: Heartbeat::step every 80 ms + RikaReplayBackend (D20 lean-in)
    --reply en-cascade.wav --loops N  you band = synthetic room → breath → voice → barge-in (rika, listening)
    --mimi off --realtime             me band  = the cascade WORDS (ReplyVoice) → dot "speaking" for their length
                                      + paplay/aplay plays the same WAV to the speakers on each speaking onset (stops on barge-in)
                                      serves / + /dot + /health only (no /frame, no PNGs). Details: docs/agent-v1-demo/FP-02-STATUS.md
[3] V1-01 page (/)                    polls /dot every 80 ms: idle · rika (lean-in) · listening · speaking
fallback: live build fails → scripts/run-v1-screen.sh (same page, synthetic pump, same port; `--check` then exits 1)
```

## 3. What the screen shows (V1-01, final V1 UI per Q5)

There is one solid dot (`#F5F5F7`), centred, on obsidian `#0A0A0C` with a slow amber → violet background wash. There is no text, no buttons and no chrome.
Modes: **idle** (slow pulse) · **rika** (held breath, the dot leans to 0.112 when Rika lean-in fires, ≈160 ms) ·
**listening** (larger, almost still) · **speaking** (one step per 80 ms, driven by me-band RMS). If the feed drops for more than 1 s, the dot shows the idle pulse.
Snapshot: `docs/agent-v1-demo/img/v1-01-screen.png`.

## 4. Soft checks (`scripts/v1/soft-checks.sh`, gate `galion-gates/v1-demo.sh` → `V1` in run-gates)

| id | check |
|---|---|
| C0 | the speech turn wrote a WAV |
| C1 | live showcase default/opener/workplace: stereo WAV + capture |
| C2 | Rika opener `lean_in_latency=160 ms` |
| C3 | workplace purpose is hard-OFF (me band byte-identical to default) |
| C4 | all WAVs are 24 kHz PCM16 |
| C5 | one-dot frames exist |
| C6 | `check-locks.sh` OK |
| C7 | **EN reply says words** (Q2): `reply_kind=speech` in en-cascade.json. SOFTFAIL if it falls back to the earcon |
| C12 | EN turn: the reply words are in the **me** band (`voice=reply-wav`), so the dot shows *speaking* (FP-04) |
| C11 | speech backend present: A21 Piper pack + runtime (or the libflite1 fallback) |
| C8 | brand: Galion Voice only on V1 surfaces |
| C9 | A21 status (INFO): present-verified (sha256) + Piper CPU decoder |
| C10 | deploy 51.38.137.193 PENDING |

The gate is soft and recorded under `V1=` in the run-gates summary. `GALION_GATES_HARD=1` makes it count, and `GALION_GATES_NO_V1=1` skips it.

## 5. FRXDEL decisions (docs/agent-v1-demo/FRXDEL-ANSWERS.md)

| # | Question | Answer | Effect on V1 |
|---|---|---|---|
| Q1 | Plumbing demo for internal review? | YES | showing timing, duplex, Rika and consent is enough; it does not need to sound like a person |
| Q2 | Tone-only EN reply OK? | **NO**, it must say actual words | **met**: A21 (Piper, CPU) speaks the words (C7 PASS); flite is the fallback only |
| Q3 | A21 speech-weight path funded? | YES, real weights coming | owner t123u: A21 built on Piper (MIT, CPU). LOCKS ratification and espeak-ng GPL-3 counsel are still open |
| Q4 | Server target? | YES: **51.38.137.193** | deploy deferred: SSH denied |
| Q5 | Is the single-screen one-dot the final V1 UI? | YES | no UI additions |

## 6. Deploy (deferred)

Target **51.38.137.193** is PENDING SSH access. Nothing has connected. Once access exists, run the same command on the host, then
`scripts/a21/fetch-a21.sh` on the host, then `ssh -L 7878:127.0.0.1:7878 <user>@51.38.137.193` and open `http://127.0.0.1:7878/`. The page binds loopback only by design. Public
exposure needs a reverse proxy, which is not built. Release steps: `scripts/deploy/release-dryrun.sh` (dry-run only). R0–R3 gates stay PENDING.

## 7. Honest limits / A21 blockers

- **Real speech (FP-03):** the EN reply is neural speech from **A21**, the Piper VITS `en_US-ljspeech-medium` voice. It runs on CPU in a separate process, is deterministic and is sha256-verified.
  - Speed: RTF ≈ 0.05, about 0.5 s per turn.
  - Flite is used only if the pack is missing.
  - Still open: the `.onnx` runtime is an owner-approved exception to `formats.foreign_runtime_allowed=false` (LEDGER, LOCKS not amended), and espeak-ng (GPL-3) needs counsel. Full list: `docs/agent-v1-demo/A21-BLOCKERS.md`.
- **Me band = the cascade words (FP-02).** `galion-live-demo --reply <wav>` plays the cascade reply in the me band (ReplyVoice), and
  `me_planned` holds the turn for its full length, so the dot is *speaking* exactly while the words play. FP-03: with `--realtime` the same
  WAV is also **played to the speakers** (paplay/aplay) on each speaking onset and killed on barge-in. There is no audio element on the page.
- **Rika lean-in** comes from the synthetic you band (room + in-breath) on every loop (`lean_in_frames=21`/loop). The old
  `V1_TURN_INPUT=alternate` and `scripts/v1/pad-wav.py` are gone.
- ASR is a stub (fixture codes), the LLM is MockLlm (echo), and emotion is fixed.
- **Mimi / student weights (BACKLOG #5/#10, still BLOCKED on A21-COUNSEL):** counsel sign-off on path P-A plus the CC-BY checklist, per `docs/agent-wave20/A21-COUNSEL-MEMO.md`. It blocks real Mimi
  weights (#5), `gq-pack` + G5 (#10), and every weight on disk. Mimi stays `IS_STUB` (ZeroWeights/DryBypass) and the student backend is a stub.
- There is no microphone. Input is a WAV or synthetic audio. Emotion comes from a heuristic proxy. The RTF figure measures the stub path.

— Maciej Grajczyk
