# Galion Voice V2

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. This is the program plan. It is not a claim that V2 is finished,
and it is not V3.

The spelling is **Galion**. The product is Galion Voice.

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
render crate. ngrok stays the public HTTPS until a human has an SSH key on
Titanaxe. Caddy is the later edge. The drafts are already in the deploy notes.

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
(LEDGER): A21 is Piper. LOCKS is not amended. espeak-ng (GPL-3, phonemizer)
is waiting on counsel. Flite is the fallback. Tones are not a reply.

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

**LOCKS.** `LOCKS.toml` is the contract. This program does not change its
values. GGML-family containers stay out. `forbidden_runtimes` stays as written.
`paid_tiers = false`.

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

What is deliberately missing: a real ASR engine, a `.gq` reply model, a warm
Piper process across turns (the stack is built once per worker, which already
avoids per-turn process start when the backend allows it; Piper today is still
out of process per `speak`), wiring that orb integrator into the surfaces
(the step itself is in `galion-render`; the page still draws the phone), and
any path that opens SSH.

## 5. Sectors

Order is dependency order, not a calendar.

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
`ANIMATION-ENGINE.md`). Surfaces still draw the discrete golden.

**Still open.** PNG, terminal, and the future softbuffer window do not
sample the spring yet. `NullSurface` still records `DotParams`, so this
landing does not re-bless G4. Switching a surface over is an owner step,
and it re-blesses G4 only if the recorded dot stops being `DotParams`.
A shader is a later engine step, not a requirement for the phone to feel alive.

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
matches the one-file page and the blob worklet. `GALION_CORS_ORIGINS` allowlist
(default remains `*` so a changing ngrok host still works). Retention note.
Demo log omits transcript text.

**Still open.** Default the allowlist to the real public names once they are
stable. HSTS belongs on ngrok/Caddy, not on loopback HTTP. A one-page notice
before a demo is shown to anyone outside the project.

**Acceptance.**

1. `refuses_public_bind` passes.
2. `responses_carry_edge_security_headers` passes.
3. `cors_allowlist_is_exact_and_star_is_default` passes.
4. `scripts/check-phone-pipeline.sh` against loopback: `/health`, `/dot`,
   `POST /mic` 204, `OPTIONS /turn` 204, `POST /turn` 202.
5. A running screen with `--no-png` creates no WAV under the repo.

**Risks.** `Access-Control-Allow-Origin: *` left on a public demo forever.
CSP `unsafe-inline` is required by the single included page; a nonce means
splitting the page, which is a later change, not a silent break.

**LOCKS.** Loopback is already required by the render crate's own comment and
test, matching the build spec. No lock key needs to change. Consent `sense`
stays deny; prosody classification stays unwired.

**Owner decision.** The public origin list. Whether the demo is open or behind
edge auth (`basic_auth` is commented in the Caddy draft). Counsel on showing
the demo to other people (GDPR notice, ngrok as processor).

### V2-S4 · Real EN ASR

**Goal.** After the owner picks A, B, or C, replace the interim placeholder.
Until then the placeholder stays labelled and the default stays refusal.

**Depends on.** `docs/policy/RATIFY-EN-ASR-PHONE-PATH.md`. V2-S1 if the engine
is a sidecar (option A wants a process, which the bus can host). A21 counsel
only if option B widens the ONNX exception.

**The choice, restated.**

- **A.** Vosk, out of process, Apache-2.0, not GGML, not ONNX. Recommended in
  the draft. Needs a sunset-dated LOCKS exception the owner writes.
- **B.** Extend the A21 ONNX exception to an ASR model. Better accuracy, wider
  exception. Only if A21 is ratified and Vosk is not good enough.
- **C.** No foreign ASR. Keep the stub until Whisper-tiny English runs as
  safetensors on `galion-tensor`. The phone has no real recognition until then.
- **Rejected.** whisper.cpp. It needs the GGML-family container ban lifted
  everywhere. Not offered.

**Acceptance, once a letter is picked.**

1. Five scripted English phrases return transcripts with the words in them,
   not the length-bucket placeholder. The policy draft's bar is about 15% word
   error on those phrases.
2. `GALION_ASR=interim` still exists until the owner removes it, and the UI
   label cannot say the words were recognised when they were not.
3. `scripts/check-locks.sh` and `scripts/check-no-forbidden.sh` stay green.
4. Partials do not push the heartbeat past `onset` / barge-in budgets. ASR
   runs on the worker, not inside `step`.

**Risks.** An agent "temporarily" vendors a GGML runtime to make the demo
clever. That is a lock break, not a shortcut. Shipping `interim` as if it
were recognition. The page and the log already carry the label; V3 must not
drop it.

**LOCKS.** `ggml_container_allowed = false`. `foreign_runtime_allowed = false`.
`forbidden_runtimes` includes `ggml` and `ort`. Weight formats are safetensors.
Agents do not edit these values. Option A or B is an owner edit plus a LEDGER line.

**Owner decision.** The letter A, B, or C. Signature line is on the policy draft.

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

**Owner decision.** When `sense` may be granted for a demo. Which student
size (S0 / S1 / S2) is the reply model. Not decided here.

### V2-S6 · Speech runtime

**Goal.** Keep Piper warm so the first phone reply does not pay process
start. Hybrid rendering stays: Piper when the pack is present, flite when it
is not, failure when neither is (no silent tones as the reply). Emotion
prosody is a hook, not a second voice.

**Depends on.** A21 pack on disk for the neural path. V2-S5 if prosody should
follow reply emotion. The worker already builds `PhoneLane` once, so the
stack object is warm; the Piper process may still start per `speak`.

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

**LOCKS.** A21 `.onnx` is the scoped exception t123u, not yet written into
LOCKS. Do not widen it. espeak-ng GPL-3 waits on counsel (`docs/agent-v1-demo/A21-BLOCKERS.md`).

**Owner decision.** Ratify the A21 exception text. Counsel on espeak-ng.
Whether `GALION_A21_NOISE=natural` is allowed in the demo.

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

**Owner decision.** If a freemium split is ever real, it is an owner edit of
`[licence]` plus counsel. Not a README sentence.

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

**Owner decision.** Training go-ahead. Which checkpoint is allowed as teacher.
Not this program.

### V2-S10 · Gates R0–R3

**Goal.** Remote probe and gate run on the VPS, unchanged in spirit from
`docs/agent-wave20/R0-R3-DRY-RUN.md`. R0 reachability, R1–R3 the strict gate
run, results recorded. Still blocked on a human SSH key.

**Depends on.** Plan V5 human steps H1–H4. `galion-gates/run-gates.sh` remote
block, which connects only if `GALION_VPS_HOST` is set.

**Acceptance.**

1. With the variable unset, R0–R3 stay PENDING. An agent run must not set it.
2. `--execute` on the dry-run scripts still exits 2.
3. After a human key exists, R0 is a prebuilt probe, not a build on the VPS,
   and the result JSON lands in `galion-gates/results/` with a LEDGER line.

**Risks.** A "quick" SSH from an agent that found a key in the environment.
Plan V5 says the key is human-installed and this program does not go looking.

**LOCKS.** Probe section stays as written. Thresholds only tighten.

**Owner decision.** Install the key. Choose the public name. Approve the
first real release.

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

## 7. What stays human

- SSH key on Titanaxe. No deploy from this program.
- ASR letter A, B, or C, and any LOCKS edit that follows.
- A21 exception text inside LOCKS, and espeak-ng counsel.
- Whether the hint line and a `thinking` state may exist on the locked surface.
- Public demo notice and CORS names.
- Turning auto-merge on in the GitHub settings, if branch protection is not already doing it. The workflow file only runs checks.

## 8. Non-goals

- A second voice stack beside PhoneLane.
- A new tunnel product. ngrok now, Caddy later.
- GGML, in-tree or out of process.
- Editing `LOCKS.toml` values.
- Calling the interim transcript recognition.
- Calling the spring page V3.

— Maciej Grajczyk
