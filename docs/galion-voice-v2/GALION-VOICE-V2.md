# Galion Voice V2

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. This is the program plan. It is not a claim that V2 is finished,
and it is not V3.

The spelling is **Galion**. The product is Galion Voice.

Master slogan: "The seamless connection with the human being, on the voice only."

Research synthesis dated 2026-11-03 is
[`docs/research/galion-voice-v2/`](../research/galion-voice-v2/README.md).
It is not this plan. Domain claims there are desk synthesis.
**LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no
in-tree ORT, `paid_tiers=false`.

## 1. Version map

| Name | What it is | State |
|---|---|---|
| **V1** | Foundation. One dot, 80 ms heartbeat, Rika, EN cascade lane, Piper A21, the phone interim path (capture, `/turn`, `/reply`, turn worker). | Largely done on the machine that produced the phone delta, and landed in this tree by that delta. |
| **V2** | Architecture, the orb engine, a typed process bus, and a harder edge. Same stack. | This program. The orb foundation and the phone path are in the tree. The bus and the real ASR engine are specified, not swapped in. |
| **V3** | Production-ready Galion Voice. Real EN ASR ratified, reply model past the canned table, speech warm, edge on the VPS, gates R0–R3 green, consent story counsel has signed. | Later. Nothing in this plan pretends to be V3. |
| **Plan V4** | The ops dry-run that already exists: `scripts/deploy/harden-dryrun.sh`, `scripts/deploy/release-dryrun.sh`, `docs/agent-wave20/R0-R3-DRY-RUN.md`, `docs/deploy/TITANAXE-VPS.md`. | Written. `--execute` refuses. No SSH. |
| **Plan V5** | The deploy track for the stack as it is now, including the phone routes. | `docs/ops/PLAN-V5.md`. |

Ops numbers and product numbers are different series. Plan V4 is not "Voice V4".
The agent ledger titled "Galion Voice V4" is the wave ledger in `LEDGER.md`.
Plan V5 does not renumber it.

## 2. Theme

V2 is about **process communication**, **architecture**, and **security**.

The voice already has parts: a capture page, an 80 ms heartbeat, a turn worker,
a cascade lane, a speech stack, a renderer. They talk through cloned mutexes,
a one-slot job channel, and JSON fields added onto `/dot` whenever a new fact
was needed. That was the right way to get a phone to speak. It is a bad way
to grow.

V2 names the messages, puts backpressure on them, and carries a barge-in cancel
token through every stage. It makes the one mark move like something with mass,
driven by the sound that is actually playing. It keeps TLS at the edge and the
app on loopback, and it says how long a voice recording lives (not long, and
not on disk).

V2 does **not** invent a second tunnel, a second HTTP server, or a second
render crate. ngrok stays the public HTTPS until a human has finished
access setup on Titanaxe. That setup is per-user anonymized access codes,
not one shared key. Caddy is the later edge. The drafts are already in the
deploy notes.

## 3. The V1 stack this plan sits on

These are in the repository. V2 extends them.

**One mark.** `LOCKS.toml` `[ui]`: one dot, `#F5F5F7` on `#0A0A0C`, amber
`#F5A524` to violet `#7C3AED` on the background only, size and brightness only,
`chrome_allowed = false`. `galion-render` draws that mark from heartbeat state.
The phone page is `crates/galion-console/assets/v1-screen.html`.

**Heartbeat, 80 ms.** `LOCKS.toml` `[clock]`: 12.5 Hz, 24 kHz, 1920 samples,
60 ms compute budget inside the frame. `galion-heartbeat` `Heartbeat::step`
is the clock. The turn worker is forbidden to run inside that call.

**Rika.** On silence and an in-breath, Rika leans in on the feeling band
(about 160 ms in the stub measurement). It needs consent bits `sense` and
`adapt`. Both default deny. Rika does not read the transcript.

**Cascade EN lane.** `galion-cascade`: `Asr` → `Llm` → `Tts`, English only,
feature `cascade`, never the default build, never the 80 ms primary path.
`en_lane` is the demo lane (stub ASR, mock or canned reply, Piper or flite).

**Piper A21.** `en_US-ljspeech-medium`, CPU, out of process, sha256-pinned,
fetched by `scripts/a21/fetch-a21.sh`, not vendored. Owner decision t123u
(LEDGER): A21 is Piper. The scoped sunset exception is
`[formats.exceptions.a21_piper]` in `LOCKS.toml` (sunset `2027-02-01`).
Global format locks are unchanged. No in-tree ort. espeak-ng (GPL-3,
phonemizer) is waiting on counsel. Flite is the fallback. Tones are not a reply.

**PhoneLane.** `galion-cascade` `phone` (feature `cascade`): 16 kHz PCM in,
transcript, reply text, 24 kHz WAV out. `GALION_ASR` unset means stub
(fixtures decode, real speech returns `AsrUnratified`). `interim` is a
**placeholder**: canned English chosen by utterance length, labelled
`interim-placeholder`, not recognition. `GALION_LLM` defaults to `CannedLlm`
on this lane. `TurnWorker` in `galion-console` (feature `phone-turn`) runs
that lane on its own thread. `PhoneHost` polls it from the screen demo.
`WebSurface` serves `POST /mic`, `POST /turn`, `GET /reply/<id>.wav`, and
puts `reply` on `/dot`.

**Edge.** The app binds loopback only. ngrok is the interim public HTTPS
(`docs/agent-v1-demo/EDGE-NGROK.md`). Caddy on Titanaxe `51.38.137.193` is
drafted (`docs/deploy/TITANAXE-VPS.md`) and not installed from here.

**LOCKS.** `LOCKS.toml` is the contract. Global values are unchanged. The only
addition in this program is the sunset-dated A21 Piper exception above.
GGML-family containers stay out. `forbidden_runtimes` stays as written.
`paid_tiers = false`. Chatterbox is not a lock exception.

## 4. How a phone turn moves today

```text
phone browser                         loopback 127.0.0.1:7878
  tap → getUserMedia
  AudioWorklet → 16 kHz VAD
  POST /mic   {rms}        ───────►  MicTap            (latest only)
  POST /turn  WAV          ───────►  TurnInbox         (≤ 8, RAM)
                                     PhoneHost::drive_me   every 80 ms, try_*
                                         │
                                         ▼
                                     TurnWorker thread
                                       PhoneLane
                                         S4 PhoneTurnAsr
                                         S5 CannedLlm (or echo)
                                         S6 SpeechStack (Piper, else flite)
                                         │
                                         ▼
                                     ReplyBox  GET /reply/<id>.wav
                                     /dot "reply"
  fetch WAV → AudioContext
  AnalyserNode envelope → orb spring
  you speak during playback → stop, new /turn, worker supersede
```

The heartbeat thread only calls `try_submit` and `try_recv`. Piper cold-start
and synthesis happen on `galion-turn-worker`. A newer turn id cancels an older
reply before publish. That cancel is the seed of the V2 barge-in token.

Piper on a `PhoneLane` is one out-of-process worker for the life of that lane
(stdin JSON, stdout raw PCM). `PhoneLane::new` pre-warms it, so the first
caller turn does not pay process start. Cold vs warm first-clause PCM is
`scripts/bench-piper-warmth.sh`. See V2-S6.

What is deliberately missing: a real ASR engine, a `.gq` reply model, pointing
PNG or the window at the spring (the page still draws the phone; `/dot`
`engine` is a one-step Luti witness beside the `DotParams` golden), and any
path that opens SSH.

## 5. Sectors

Master slogan: "The seamless connection with the human being, on the voice only."

Order below is dependency order, not a calendar. Priority is separate.

**Priority (Maciej Grajczyk, 2026-11-03).**

1. **V2-S14** — real-voice cloning on the Chatterbox architecture, local CPU, self-offline. Detail after V2-S13, and in `VOICE-CLONING.md`.
2. Everything else in this list, in the order written.

Sector numbers are not renumbered. V2-S15 (Hermes) is an approved gateway beside local models. It is not priority 1.

### V2-S1 · Process bus

**Goal.** One typed message envelope between capture, heartbeat, ASR, reply,
speech, render, and the consent gate. Replace the habit of adding another
mutex field every time a stage needs a fact. Schemas, backpressure, and a
barge-in cancel token are the whole sector. Detail: `IPC.md`.

**Depends on.** V1 `TurnInbox`, `ReplyBox`, `MicTap`, `TurnWorker` (they are
the queues to name, not to throw away). Heartbeat 80 ms rule.

**Acceptance.**

1. A message older than `cancel` is dropped before ASR and again before the
   reply is published. The existing `stale_reply_dropped_after_supersede` test
   still passes, and a bus-level test does the same with two stages.
2. Mic levels coalesce to one slot. Utterances cap at 8. Replies cap at 8.
   A full job queue does not block `Heartbeat::step`.
3. No new crate whose job is "a message broker". The types live next to the
   worker that already exists.
4. A slow `/turn` body cannot stall `/dot` (already true: `slow_client_does_not_block_dot`).

**Risks.** A bus that is only documentation, so the next feature adds a fifth
mutex. Or a bus that is a framework and breaks the offline, std-only web surface.

**LOCKS.** The 80 ms frame and the 60 ms budget. The bus is not allowed to
allocate inside `Heartbeat::step` beyond what that function already does.
Cancel must not be implemented by killing the process.

**Owner decision.** None to start. The envelope in `IPC.md` can land as types
without a lock change. Splitting the worker into separate OS processes (so a
future Vosk sidecar can sit on the same bus) needs an owner yes, because it
changes failure and supervision.

### V2-S2 · Orb / graphic virtual engine

**Goal.** The one mark becomes a fluid orb: continuous motion, distinct idle /
listen / think / speak, microphone energy on the way in, reply-audio envelope
on the way out. This is the foundation of Galion's graphic engine. It is not
a skin on top of a dashboard. Motion contract: `ORB.md`.

**Depends on.** V1 dot tokens, the phone page, `/dot` `mode` + `level`, and
the reply WAV the page already plays.

**Landed in this program.** The page runs a spring-damper at display rate.
Listening radius tracks mic energy. Thinking is the wait between accept and
playback. Speaking radius tracks an `AnalyserNode` on the playback graph.
The server drains reply PCM into the me band so `/dot` hears the reply too.
The same spring is in `galion-render` (`OrbEngine`, design in
`ANIMATION-ENGINE.md`). The easing profile is named **Luti** (one-UI:
`docs/galion-voice-v2/ONE-UI.md`). PNG, the terminal, and `NullSurface`
still draw the discrete golden. `/dot` adds an in-process `engine` account
(frames, last dt, settle error, cost hint) from one 240 Hz Luti step per
handoff. That account is not a second product and not a scrape endpoint.

**Still open.** PNG, terminal, and the future softbuffer window do not
sample the spring yet. `WebSurface::present` steps one Luti frame at 240 Hz
for the `/dot` account only; the published radius stays `DotParams`.
`NullSurface` still records `DotParams`, so this landing does not re-bless
G4. Switching a drawn surface over is an owner step, and it re-blesses G4
only if the recorded dot stops being `DotParams`. A shader is a later
engine step, not a requirement for the phone to feel alive.

**Acceptance.**

1. `screen_page_is_phone_path_and_v2_orb` passes.
2. Idle motion is continuous across `/dot` polls (60 fps clock, not 12.5 Hz steps).
3. While a reply plays, the radius changes with the WAV envelope.
4. Still one filled mark. No menu, face, waveform strip, or second dot.
5. G4 golden still matches, because `dot.rs` geometry was not retuned.

**Risks.** Calling a CSS scale "the engine" and stopping. Or breaking
`chrome_allowed` by adding captions onto the user surface. The hint line
"Tap the dot to talk" is the smallest prompt that makes a phone mic usable.
It is a tension with `chrome_allowed = false`. It is not a lock edit.

**LOCKS.** `[ui]` unchanged. Gradient stays off the mark. `dot_states` stays
three names until an owner adds `thinking`. The page may move as if thinking;
it must not pretend the lock already lists that state.

**Owner decision.** Whether `thinking` becomes a locked state. Whether the
hint sentence is allowed on the user surface or must be replaced by motion
alone. Whether PNG and the window sample `OrbEngine` (the module is in the
crate; surfaces still use `DotParams`, so G4 stays put until that switch).

### V2-S3 · Secure edge

**Goal.** TLS only at the edge. App on loopback. CORS allowlist. CSP and
Permissions-Policy. Ephemeral turn buffers. A written retention story. No raw
audio on disk by default. Detail: `SECURITY-EDGE.md`.

**Depends on.** `WebSurface` loopback refusal (V1). ngrok now, Caddy later
(Plan V5). The inbox and reply box caps (V1).

**Landed in this program.** Security headers on every response. CSP that
matches the one-file page and the blob worklet. `GALION_CORS_ORIGINS` exact
allow-list (`*` only when the variable is unset, for the local demo). Foreign
`Origin` is 403 with no allow-origin header. Phone HTTP session token in the
`POST /session` body (there is no WebSocket). Loopback bind stays; `0.0.0.0`
only with the public-bind flag plus session auth and an exact origin list.
Art. 50 disclosure on the first turn (HR and EN). Retention note. Demo log
omits transcript text and PCM.

**Still open.** The owner fills the real public origin when a name exists.
HSTS belongs on ngrok/Caddy, not on loopback HTTP. Counsel still owns the
public-demo notice. Login and the exact-origin rule are decided below.

**Acceptance.**

1. `refuses_public_bind` passes.
2. `responses_carry_edge_security_headers` passes.
3. `cors_allowlist_is_exact_and_star_is_default` passes.
4. `scripts/check-phone-pipeline.sh` against loopback: `/health`, `/dot`,
   `POST /mic` 204, `OPTIONS /turn` 204, `POST /turn` 202.
5. A running screen with `--no-png` creates no WAV under the repo.

**Risks.** Leaving `GALION_CORS_ORIGINS` unset on a public tunnel, which keeps
`Access-Control-Allow-Origin: *`. Staging must set exact origins.
CSP `unsafe-inline` is required by the single included page; a nonce means
splitting the page, which is a later change, not a silent break.

**LOCKS.** Loopback is already required by the render crate's own comment and
test, matching the build spec. No lock key needs to change. Consent `sense`
stays deny; prosody classification stays unwired.

**Owner decision (2026-11-03, YES).** Staging and any public demo sit behind
login: ngrok OAuth or basic auth. CORS on that demo is an exact `https://`
origin list. No wildcard. The unset-variable `*` remains the local loopback
default only, and it is not a staging setting. Counsel on the GDPR notice is
still open. Detail: `SECURITY-EDGE.md`, Plan V5.

### V2-S4 · Real EN ASR

**Goal.** Letter A is chosen. Build a Vosk sidecar later. Until the WER bar
passes, `interim` stays labelled and the unset default stays refusal.

**Depends on.** `docs/policy/RATIFY-EN-ASR-PHONE-PATH.md` (letter A recorded).
V2-S1 if the engine is a sidecar. The A21 ONNX pin is not widened for ASR.

**Owner decision (2026-11-03, YES, letter A).** Maciej Grajczyk. Vosk,
out of process, is the EN ASR choice.

- Model pin to fetch later, not in git: `vosk-model-small-en-us-0.15` (small-en). sha256 is not pinned yet. No weights in this change.
- Grammar mode for intents. Open dictation is not the product path.
- Separate process. PCM stays off disk. The worker thread runs it, not `Heartbeat::step`.
- `GALION_ASR=interim` is retired only after the WER bar: about 15% word error on the five scripted phrases in the policy draft. Until that measurement exists, the UI keeps the `interim-placeholder` label.
- B and C are not the choice. whisper.cpp stays rejected.
- The Vosk LOCKS exception is not written here. Global locks stay. The Piper pin is the only exception text.

**Scaffold (docs only).** No sidecar binary, no vendor directory, no download.
The later process boundary is the same shape as the warm Piper worker: one
long-lived process, stdin/stdout PCM16, grammar string supplied by the intent
list, failure returns the labelled placeholder rather than a fake transcript.

**Acceptance, when the sidecar exists.**

1. Five scripted English phrases return transcripts with the words in them,
   not the length-bucket placeholder. The bar is about 15% word error on those phrases.
2. `GALION_ASR=interim` still exists until that bar passes, and the UI
   label cannot say the words were recognised when they were not.
3. `scripts/check-locks.sh` and `scripts/check-no-forbidden.sh` stay green.
4. Partials do not push the heartbeat past `onset` / barge-in budgets. ASR
   runs on the worker, not inside `step`.

**Risks.** Vendoring a GGML runtime to make the demo clever. Shipping
`interim` as if it were recognition. Writing a Vosk exception that also
loosens `ggml` or `ort`.

**LOCKS.** `ggml_container_allowed = false`. `foreign_runtime_allowed = false`.
`forbidden_runtimes` includes `ggml` and `ort`. Weight formats stay
safetensors (plus the Piper pin). A Vosk exception, if it is ever written,
is a later owner edit.

### V2-S5 · Reply intelligence

**Goal.** The phone reply is `CannedLlm` now (a short English table, honest
when it does not know the sentence). The slot it stands in for is a small
`.gq` student on `galion-tensor`, when that student exists. Emotion in both
directions only when consent says so.

**Depends on.** V2-S4 for replies that depend on real words. V2-S9 for the
student weights. `galion-emotion` consent gate. A8 distillation, which is
blocked on GPU budget, A21 legal, and R0.

**Today.** `CannedLlm` matches intents and, if handed `Angry` or `Sad`,
answers that before the table. `PhoneLane` passes `Emotion::Neutral` always,
because reading affect from the caller's voice is the `sense` bit and the
default is deny. `ProsodyEmotion` is tested and unwired. `ReplyLlm` still
defaults to echo outside the phone lane so old fixtures keep their strings.

**Acceptance.**

1. Phone tests: "hello how are you" → the canned well-being line, not
   `you said: …`, unless `GALION_LLM=echo`.
2. With `sense` deny, a loud utterance does not change the reply emotion.
3. With `sense` granted in a test only, a future wire may pass `ProsodyEmotion`
   into the lane. That test must not be the default demo.
4. Unknown sentences get the "say a bit more" line, not a made-up fact.
5. When a `.gq` model exists, it implements `Llm` and the canned table remains
   the fallback. No second reply stack.

**Risks.** Letting the canned table pretend it understood. Letting prosody run
on by default. Pulling a chat API into the binary. The reply slot is local.

**LOCKS.** `.gq` only for the small model (`Llm` trait comment, G2 / formats).
No GGML. `sense` deny. Workplace and education purposes hard-off. Emotion out
is Galion's own voice, counterbalancing rather than mirroring (existing
emotion crate). The phone lane does not express that yet; V2-S6 is the hook.

**Owner decision (2026-11-03, YES).** The first student size is **S0**
(about 0.5B, `galion-student/configs/s0.toml`). S1 and S2 stay later sizes.
`CannedLlm` remains the phone fallback until readiness tests say the student
can speak. `sense` for a demo is still undecided. No weights are trained here.

### V2-S6 · Speech runtime

**Goal.** Keep Piper warm so the first phone reply does not pay process
start. Hybrid rendering stays: Piper when the pack is present, flite when it
is not, failure when neither is (no silent tones as the reply). Emotion
prosody is a hook, not a second voice.

**Depends on.** A21 pack on disk for the neural path. V2-S5 if prosody should
follow reply emotion. The turn worker builds `PhoneLane` once. That lane now
owns the Piper process.

**Landed.** When the A21 pack is present, `PhoneLane::new` starts one Piper
child (`--json-input --output_raw`) and discards a short "Hello." so the
first caller turn is warm. Raw PCM is framed by the runtime's
`Real-time factor:` line on stderr (piper 2023.11.14-2 logs it after flushing
stdout). Nothing is written to disk. Flite is unchanged when the pack is
absent. Streaming text flushes a clause at punctuation or 8 words
(`SpeechStack::begin_stream`); a whole reply takes the same split. Cold vs
warm first-clause PCM is `scripts/bench-piper-warmth.sh` (one JSON object,
no invented number when the pack is missing). `GALION_SPEECH=off` is still
`NoSpeech`, and the turn worker still publishes the two-tone WAV with
`backend=tone` and `error` set. A superseded turn drops clauses not yet sent
and does not kill Piper.

**Acceptance.**

1. Two turns on one `PhoneLane` reuse the stack (the phone test
   `full_turn_emits_24k_speech_wav` already does a second `turn_wav` on the
   same lane).
2. A cold-start measurement is written down: first `speak` vs second, same
   process. The target is to cut the cold start, not to hit a number invented
   here. Record the number in LEDGER when it is measured on a real pack.
3. `GALION_SPEECH=off` yields `NoSpeech` and the worker publishes the
   two-tone signal with `backend=tone` and `error` set, so the phone hears
   that the turn arrived and the log says it was not speech.
4. Prosody hook: `Emotion` on the reply can later change rate or the Piper
   noise flag. It does not change the voice identity, and it does nothing
   while emotion is `Neutral` and consent is deny.

**Risks.** Caching audio on disk "for warmth". Linking onnxruntime into the
Rust binary (the exception is out of process). Treating flite as the product
voice.

**LOCKS.** A21 `.onnx` is `[formats.exceptions.a21_piper]`, sunset
`2027-02-01`. Out of process. No in-tree ort. Do not widen it to ASR or to
Chatterbox. espeak-ng GPL-3 waits on counsel (`docs/agent-v1-demo/A21-BLOCKERS.md`).

**Owner decision (2026-11-03, YES).** The exception text above is the
ratification of t123u. Counsel on espeak-ng is still open. Whether
`GALION_A21_NOISE=natural` is allowed in the demo is still open. Piper stays
the fallback under V2-S14 until a local clone worker exists and rights are clear.

### V2-S7 · CI/CD track

**Goal.** Every change is a LEDGER line. PR checks cover locks, the forbidden
list, and the tests. A green PR may merge. Deploy is a dry-run until SSH
exists, then the same scripts with a human key. Plan V5 is the operational
version of this sector.

**Depends on.** `scripts/check-locks.sh`, `scripts/check-no-forbidden.sh`,
`galion-gates/run-gates.sh`, `scripts/check-phone-pipeline.sh`,
`scripts/deploy/*-dryrun.sh`.

**Acceptance.**

1. A PR that edits `LOCKS.toml` values without a LEDGER line is called out.
   The lock file's own rule is that an owner changes values.
2. `cargo test --workspace` green on Rust 1.85 (the crate MSRV).
3. `cargo test -p galion-render --features web` green (the phone HTTP surface
   is not in the default feature set).
4. `cargo test -p galion-cascade --features cascade` and
   `cargo test -p galion-console --features phone-turn` green.
5. Phone smoke: `scripts/check-phone-pipeline.sh`.
6. Deploy workflow refuses to SSH while no key is configured. It prints the
   dry-run. It does not grow a fallback that "tries anyway".

**Risks.** Making phone-turn a default feature and slowing every console build
for no reason. The screen script opts in with `dot-viz-web`, which pulls
`phone-turn`. Default console features stay the developer console.

**LOCKS.** Gate thresholds only tighten. G2 stays a hard absence of the
GGML-family container name in the tree. Docs in this program avoid that name
for that reason; the rejection is written as the lock already writes it.

**Owner decision.** Auto-merge is the owner's policy: green checks, no lock
edit, no secret. `.github/workflows/gates.yml` runs the local gates on pull
requests. It does not deploy. See Plan V5 for what green means with and
without an SSH key.

### V2-S8 · Branding and docs

**Goal.** One visual language for Galion Voice: the name, the voice, the
colours, the orb as the mark. Docs agree with the code. Detail:
`docs/brand/GALION-VOICE-BRAND.md` and the root `README.md`.

**Depends on.** `LOCKS.toml` `[ui]` and `[licence]`. V2-S2 for the orb.

**Acceptance.**

1. README describes V1 → V2, how to run `scripts/run-v1-screen.sh` and ngrok,
   and points at this file and Plan V5.
2. Brand doc uses the lock colours and does not invent a second studio name.
3. Licence text matches the lock: code MIT, weights separate, `paid_tiers = false`.
4. No doc in this program tells the reader the interim ASR recognised the words.

**Risks.** A brand pass that adds a logo asset the lock forbids on the user
surface. The mark is the orb. It is drawn, not a file dropped on the page.

**LOCKS.** `chrome_allowed = false`. `paid_tiers = false`. `free_forever = true`.
A Local / Plus / Ultra price list is not in this repository. It is not
documented as a product.

**Owner decision.** The master slogan is "The seamless connection with the human being, on the voice only."
It is the one public line (`docs/brand/GALION-VOICE-BRAND.md`). It is not a stack of slogans, and it is not chrome on the dot.
If a freemium split is ever real, it is an owner edit of `[licence]` plus counsel. Not a README sentence.

### V2-S9 · Student and Open Mind slot

**Goal.** Keep the distilled-student slot honest and ready for V2-S5. Configs
live in `galion-student/` (`student.toml`, `distill.toml`, `openmind-slot.toml`).
Nothing is trained. No weights are in git. The Open Mind slot is a contract
for a future local model, not a running backend.

**Depends on.** A8 KD plan, A21 legal clearance, GPU budget, R0 so a training
host is even discussable. `galion-tensor` `.gq` reader (writer still blocked).

**Acceptance.**

1. No weight file under `weights/` is committed (`weights/.gitignore` is `*`).
2. The student README still says config stubs only.
3. When a student exists, it plugs into `Llm` / the Open Mind slot. It does
   not replace `CannedLlm` until `can_speak`-style readiness is true and tests
   name the fallback.
4. No NC data in the loader's allowlist (existing student README).

**Risks.** Downloading a teacher and calling it the student. Vendoring weights
against `weights_vendored = false`.

**LOCKS.** safetensors / `.gq`, no GGML, no vendored weights, licence split
between code and weights.

**Owner decision (2026-11-03, YES).** S0 is the first student size. Canned
replies stay the fallback until readiness tests pass. Training go-ahead and
the teacher checkpoint are still not given. Configs stay stubs. The Open Mind
slot file may still say S1; that string is not a licence to skip S0.

The voice-side nerve and cortex seam is
[OPENMIND-NERVE-CORTEX.md](OPENMIND-NERVE-CORTEX.md) and
`crates/galion-openmind`. It does not train a student, and it does not put
Hermes on the 80 ms tick.

### V2-S10 · Gates R0–R3

**Goal.** Remote probe and gate run on the VPS, unchanged in spirit from
`docs/agent-wave20/R0-R3-DRY-RUN.md`. R0 reachability, R1–R3 the strict gate
run, results recorded. Status: **approved-pending-setup**. A human installs
per-user anonymized access codes. This program does not connect.

**Depends on.** Plan V5 human steps H1–H4. `galion-gates/run-gates.sh` remote
block, which connects only if `GALION_VPS_HOST` is set.

**Acceptance.**

1. With the variable unset, R0–R3 stay PENDING. An agent run must not set it.
2. `--execute` on the dry-run scripts still exits 2.
3. After a human key exists, R0 is a prebuilt probe, not a build on the VPS,
   and the result JSON lands in `galion-gates/results/` with a LEDGER line.

**Risks.** A "quick" SSH from an agent that found a key or a code in the
environment. Plan V5 says the human installs access and this program does
not go looking. One shared `galion` pubkey is not the approved design.

**LOCKS.** Probe section stays as written. Thresholds only tighten.

**Owner decision (2026-11-03, YES, corrected by t198u).** Admin approves
access setup on Titanaxe `51.38.137.193`. The design is **per-user anonymized
access codes**: every user gets their own code. There is no shared key.
Status is **approved-pending-setup**. The human does the install. No code is
invented here, and nothing connects. Public name and the first release (H2, H4)
are still open.

### V2-S11 · Presence

**Goal.** The presence harness (`galion-presence`, `specs/presence.json`)
stays the place barge-in, onset, and lean-in are measured, before a model is
good. V2 phone barge-in should eventually be a presence spec, not only a
manual phone test.

**Depends on.** V1 harness and the WAV fixtures. V2-S1 cancel token so a
barge-in during TTS is a spec assertion (`me` stops, new turn wins).

**Acceptance.**

1. Existing presence tests stay green (PS gate).
2. A later spec, not required to merge this plan, replays a you-band WAV,
   asserts barge-in under the lock budget (200 ms target in the cascade
   constants), and does not add engagement / dwell metrics. The harness
   rejects those names on purpose.
3. Mock backend remains the default. The student is another backend later.

**Phone loopback eval.** `scripts/eval/virtual-user.sh` runs T0–T2 offline on `127.0.0.1`: text or fixture WAV in, CannedLlm plus Piper or flite out, reply WAV captured when a backend exists. The JSON report is `galion-gates/results/presence-eval/`. TTFA there is a loopback full-utterance time. PSTN is not run. It does not add an engagement metric. See `docs/eval/PRESENCE-EVAL.md`.

**Risks.** Teaching the harness to score "how long the user stayed". That
list is already rejected. Do not reopen it in V2.

**LOCKS.** Barge-in and onset budgets. No engagement metric.

**Owner decision.** When a phone recording is allowed to become a fixture.
A fixture of a person's voice is personal data; it needs the same care as
V2-S3, and it does not belong in git by default.

### V2-S12 · Consent and Rika on the bus

**Goal.** Consent bits travel with the bus envelope (`Consent` body in
`IPC.md`), not as a global that a worker thread guesses. Rika still leans in
only when `sense` and `adapt` are granted, still on the feeling band, still
without reading the transcript.

**Depends on.** V2-S1. `galion-emotion` gate. Heartbeat Rika path (V1, G6).

**Acceptance.**

1. Default session: `sense` deny, phone lane emotion fixed `Neutral`, Rika off
   unless the demo granted bits the way `dot_console_demo` already can.
2. Revoking `sense` mid-call drops any affect reading on the next frame
   (existing feed tests) and stops any future prosody classify.
3. G6 stays green.

**Risks.** The screen demo grants personal-assistant consent so the synthetic
loop can show Rika. That grant is the demo's, not the phone caller's. The
phone path must not inherit it for `ProsodyEmotion`. `PhoneLane` does not.

**LOCKS.** `[consent]` default deny. Prohibited purposes not overridable.
`rika_requires = ["sense", "adapt"]`. `withdraw_wipes_ram_history = true`.

**Owner decision.** The consent UI, if any, is an owner design. V2 does not
add a settings page (that would be chrome). A future gesture has to be
designed against `chrome_allowed`.

### V2-S13 · Multilingual hold

**Goal.** English only on the phone path and the cascade route. The
multilingual notes under `docs/multilingual/` stay a plan. Polish and the
wider set do not enter `LanguageId` until that plan is approved. The route
already rejects a non-English pin.

**Depends on.** Owner approval of the multilingual plan. A speech stack that
can say more than the LJSpeech voice. Not V2's critical path.

**Acceptance.** `Lang::parse("pl")` is `None`. Phone ASR modes do not grow a
language flag in this program.

**Risks.** A helpful extra language in the canned table. The table is English.

**LOCKS.** The cascade comment and the route tests are the constraint. No
lock key lists languages; the code does.

**Owner decision.** When the multilingual plan is approved.

### V2-S14 · Real-voice cloning (Chatterbox, local) — priority 1

**Goal.** One local, self-offline clone path for two research-target voices.
No cloud TTS API. No fine-tune. Piper remains the speaking fallback until
this worker exists and the rights gate is clear. Full note:
`VOICE-CLONING.md`.

**Legal hold.** Cloning a performer's voice from a film without rights is
high risk. The comparison the owner requires is OpenAI's Sky voice, which
was withdrawn after Scarlett Johansson objected that it resembled her
performance in *Her*. Jarvis (an Iron Man film voice) and Samantha (the 2014
film *Her*) are **owner-directed research targets**. Rights, consent, and
licence must be cleared before any clip is ingested or any public demo uses
a clone. This repository does not fetch film audio and does not store a clip.

**Architecture to adopt, not to vendor.** Resemble AI Chatterbox, MIT for
the code in `resemble-ai/chatterbox` (copyright 2025 Resemble AI, read
2026-11-03). Zero-shot: a reference clip conditions a VoiceEncoder embedding,
speech-token prompt, and an S3Gen reference. T3 then emits speech tokens;
S3Gen and a vocoder emit audio. No training step in that API.

- **Primary:** Chatterbox-Nano, listed at 110M, the CPU-sized English model. The upstream README claims about 3× realtime on 8 cores. That is a vendor claim, not a Galion measurement.
- **Quality option:** Chatterbox-Turbo, listed at 350M, same family. Turbo's own code requires a reference longer than 5 seconds. The owner target is one clip of about 10 seconds per persona.
- **Streaming eval:** the owner name is a Flash streaming eval on CPU. Upstream master exposes `generate()`, not a model named Flash. Open PR 528 proposes `ChatterboxTurboTTS.stream()` for near-real-time CPU playback and was still open when this note was written. The eval is planned. It is not run here.
- **Samantha:** paralinguistic tags (`[laugh]`, `[chuckle]`, and the set Turbo/Nano already document) carry warmth. Jarvis stays calm and precise: short command lines, tags off.
- **Watermark:** upstream `generate()` applies a Perth watermark. The Nano path should keep a watermark beside the Art. 50 disclosure. Perth's own licence was not read in this session. Do not vendor it yet.
- **Worker:** a later out-of-process process, same shape as the warm Piper worker. Piper speaks if the clone worker is absent. No PCM on disk.

**CPU and memory (t204u, t205u).** Admin approves full threading, SIMD, and
maximum memory for Chatterbox Nano/Turbo workers, for every Galion Voice
worker, and for this Linux Cursor host. No artificial caps. Still
self-offline. No cloud. This decision does not change system settings,
governors, or service limits.

**Personas.** Samantha is the conversational, emotional layer. Jarvis is the
calm command and interface layer. Both are research targets under the legal
hold. They are not shipped voices.

**LOCKS.** Do not edit locks for this sector. PyTorch (the upstream stack)
stays out of process; `foreign_runtime_allowed` is still false, so a later
owner exception would be required before a worker is linked or fetched.
No GGML port (`chatterbox.cpp` and GGUF builds are the wrong shape). No
in-tree ort (a community ONNX export does not extend the Piper pin). The
multilingual T3 uses a Llama backbone; `llama` is a forbidden runtime name
inside the tree. Weights are not downloaded and not vendored. `.pt` files
are not a lock weight format.

**External APIs.** ElevenLabs, Cartesia, Play.ht, hosted Resemble cloning,
Inworld, Fish Audio, and Azure Personal Voice are **dropped**. Notes on why
they were looked at, and then dropped, are in `VOICE-CLONING.md`.

**Swarm.** Sixteen Chatterbox-local seats are planned and not spawned.
The earlier external-API seat list is dropped with those vendors. Slogan
research seats are a separate plan (`SLOGAN-AGENTS.md`) and are also not spawned.

**Owner decision (2026-11-03, YES).** Maciej Grajczyk. Priority 1 of the V2
backlog. Record only.

### V2-S15 · Hermes gateway

**Goal.** High-capability model calls for research, prediction, and
agent-assist go through **Hermes** (Galion Research / the Hermes access
layer already typed in `galion-sideband`). Local models stay the voice path.
Hermes is not on the 80 ms heartbeat. The 3 s retrieval cap stands.

**Owner decision (t198u, 2026-11-03, YES).** Hermes is the approved external
gateway beside local models. A t197u note that named OpenRouter was replaced
by this decision before any OpenRouter approval was kept. Do not call Hermes
from this record. No secrets in the repo. Gateway choice C5 in the sideband
crate is now this approval; the transport is still a trait, not a client.

The hook that offers `Turn`, `Intent`, and `Affect` is
`galion_openmind::CortexHandoff`. It uses the types already in
`galion-sideband` and only `try_send`s. Detail:
[OPENMIND-NERVE-CORTEX.md](OPENMIND-NERVE-CORTEX.md).

### Open Mind nerve and cortex

The voice engine's seam to Obsidian Open Mind is
[OPENMIND-NERVE-CORTEX.md](OPENMIND-NERVE-CORTEX.md). Nerve notes are
`voice.frame_miss`, `voice.fallback`, and `voice.backend_up` (groups
`galion-os` and `hermes`). Cortex, for this program, means Hermes taking
the analytical problem on the side band. The Open Mind repository is not
vendored. `/opt/galion/openmind` is the path a human deploy uses on the VPS.
This program does not SSH, and it does not download a model.

## 6. What this program landed

- Phone modules from the V1 delta: `phone.rs`, `canned.rs`, `turn_worker.rs`,
  `web.rs` turn/reply/mic surface, `wav.rs`, the phone page.
- `PhoneHost` so the screen demo drains turns on the 80 ms loop without
  blocking it, and plays reply PCM into the me band.
- V2 orb foundation on that same page (spring, mic energy, playback envelope).
- Rust spring foundation in `galion-render` (`ANIMATION-ENGINE.md`). Not
  sampled by `NullSurface`. G4 golden not re-blessed.
- Security headers, CORS allowlist switch, retention note.
- `scripts/check-phone-pipeline.sh` and a thicker `run-v1-screen.sh --check`.
- This plan, Plan V5, the brand doc, the README.
- Voice-side Open Mind seam: `crates/galion-openmind` and
  `docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md`. Local nerve notes when no
  daemon is present. Hermes handoff through `galion-sideband`. Not inside
  `Heartbeat::step`. LOCKS.toml not edited.

## 7. What stays human

- Titanaxe access codes. Status is approved-pending-setup. The human installs per-user anonymized codes on `51.38.137.193`. No shared key. No deploy from this program.
- The Vosk sidecar, its sha256 pin, and any later Vosk line in LOCKS. Letter A is chosen. `interim` stays until the WER bar.
- espeak-ng counsel. The Piper exception text is already in LOCKS, sunset `2027-02-01`.
- Rights clearance before any Jarvis or Samantha reference clip. No film audio is fetched here.
- A Chatterbox owner exception, if a worker is ever fetched. Not written now.
- Whether the hint line and a `thinking` state may exist on the locked surface.
- The public-demo GDPR notice, and the exact origin string once a name exists. Login and "no wildcard on staging" are decided.
- Turning auto-merge on in the GitHub settings, if branch protection is not already doing it. The workflow file only runs checks.

## 8. Non-goals

- A second voice stack beside PhoneLane. A clone worker, when it exists, is another out-of-process speech backend with Piper behind it.
- A cloud or paid voice-cloning API.
- A new tunnel product. ngrok now, Caddy later.
- GGML, in-tree or out of process.
- Editing `LOCKS.toml` values beyond the A21 Piper sunset pin.
- Calling the interim transcript recognition.
- Calling the spring page V3.
- Ingesting a film clip or shipping a celebrity voice before rights clearance.

— Maciej Grajczyk
