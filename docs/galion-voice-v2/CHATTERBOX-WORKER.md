# V2-S14 · Chatterbox Nano worker — build plan

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A build plan. Record only.

Master slogan: "The seamless connection with the human being, on the voice only."

This file is how the out-of-process Chatterbox worker gets from the current
stub to a CPU `generate` call, and how a ~10 s reference wav would condition
that call **after rights**. It does not wire `generate`. It does not download
weights. It does not run a model. It does not fetch a clip. It does not edit
`LOCKS.toml`.

The scaffold that already landed is `CLONE-WORKER.md`
(`scripts/clone/chatterbox_worker.py`, `clone_worker.rs`). The architecture
it borrows is `VOICE-CLONING.md`. The CPU time-to-first-audio harness is
`FLASH-EVAL.md`. Personas as registers, before any clip, are `SAMANTHA-V2.md`
§4.

## What the tree does today

| Fact | Where it lives |
|---|---|
| One long-lived child, text in, 24 kHz s16le PCM out | `CloneWorker`, same shape as `PiperWorker` |
| Request line has `text`, `model`, `persona`, `tags` | `clone_worker::request_line` |
| A non-empty `reference`, `audio_prompt`, `audio_prompt_path`, or `clip` exits 3 and writes no PCM | the Python stub |
| Nano primary, Turbo the option, one process shape | `CloneModel`, `GALION_CLONE_MODEL` |
| Piper, then flite, when the slot is absent or a speak fails | `SpeechStack::detect`, `fallback_piper` |
| Weights are pinned safetensors, not in git | `scripts/clone/PINS.sha256`, `weights/.gitignore` |
| Fetch refuses unless `GALION_CLONE_FETCH=1` | `scripts/clone/fetch-clone-weights.sh` |
| The stub does not import torch and does not call `generate` | `chatterbox_worker.py` |
| `reference_clips = false` | `[formats.exceptions.v2s14_chatterbox]`, decision t230u |

`GALION_SPEECH=auto` stays on Piper or flite while those weights are absent.
CI does not set the fetch flag. Protocol PCM (`GALION_CLONE_STUB_PCM=1`) is a
pipe test. It is not a voice and not a Flash result.

## Architecture

The later child is still one OS process for the life of the speech slot.
Rust never links PyTorch. The phone lane, the bus, and `Heartbeat::step`
stay on the parent side of the pipes.

```text
turn worker (not Heartbeat::step)
  SpeechStack
    CloneTts  ── stdin JSON line ──►  python3 chatterbox worker
    (Rust)    ◄── stdout s16le 24 kHz ─  PyTorch, CPU, this process only
              ◄── stderr markers ─────
    on fault or absence
      Piper A21 (own process, own pin) → flite
```

Inside the child, when a later change wires it, the public Chatterbox path
from `VOICE-CLONING.md` is the one to call. Zero-shot. No fine-tune.

- **Primary.** Chatterbox Nano, the CPU English model listed at 110M. Files already pinned: `nano/t3_nano_v1.safetensors` plus shared `s3gen`, `s3gen_meanflow`, and `ve`. `ve` is the VoiceEncoder weight the public tree describes.
- **Option.** Chatterbox Turbo, listed at 350M. Same process and the same stdin line. The T3 file swaps to `turbo/t3_turbo_v1.safetensors`. One model is resident. Switching models is a respawn, not a second copy of the weights in RAM.
- **Conditioning, after the rights gate only.** `generate` takes `audio_prompt_path`. The three pieces the public Python builds from that wav are a VoiceEncoder speaker embedding, speech-token prompt tokens, and an S3Gen reference embedding. T3 samples speech tokens. S3Gen and the vocoder turn them into audio. Upstream `generate()` applies a Perth watermark before returning samples.
- **English phone path.** Multilingual V3 stays out. Its T3 can build a Llama backbone. `in_tree_llama` on the Chatterbox pin stays false, and the name stays out of every Galion manifest.
- **Rate.** The parent contract is `CLONE_RATE_HZ` = 24000, mono s16le, which is the Galion clock. Public ports describe 24 kHz. That match was not re-measured. If a future `generate` returns another rate, the child resamples to 24 kHz before stdout. The heartbeat does not resample.

Locks the build keeps, by leaving `LOCKS.toml` alone:

- t230u: PyTorch, CPU, separate process, lane "V2-S14 clone TTS only", `in_process = false`, sunset `2027-02-01`. Global `foreign_runtime_allowed` stays false. The pin is not a licence to link the runtime, and it is not an ASR pin.
- `in_tree_ort = false`. ONNX Runtime stays inside the Piper process under `[formats.exceptions.a21_piper]`. A community ONNX export of Chatterbox does not ride that pin.
- `in_tree_ggml = false`. A GGML-family port is the wrong shape in this process and in any other, because `ggml_container_allowed = false` is anywhere, cascade lane included. Do not port the net onto candle, including "for a moment".
- `weights_vendored = false`. `weight_formats` lists safetensors. `.pt` is not a lock weight format. The fetch script already refuses `.pt` and audio suffixes. Nothing in this plan is fetched.
- `reference_clips = false`, and the pin notes say no film audio and no celebrity clone. The clip section below is what happens after an owner changes that. This plan is not that change.

CPU and memory stay t204u and t205u: full threading, SIMD, maximum memory, no artificial cap in the child, self-offline, no cloud accelerator. Recording that approval does not change host settings. The child does not set a thread cap of its own.

The 80 ms heartbeat never blocks on this process. Cancel is a token. It does not kill the child. `Drop` kills the child only when the lane goes away.

Perth stays a separate package. Its licence was not read on 2026-11-03. Do not vendor it. The Nano path keeps the watermark beside the Art. 50 disclosure already spoken on the first turn (HR and EN, `SECURITY-EDGE.md`). If the package cannot be imported, the child writes no samples and the parent uses Piper. A missing watermark is not a silent skip.

Weights live under `weights/clone/` or `GALION_CLONE_WEIGHTS`, gitignored, sha256-checked against `scripts/clone/PINS.sha256` by `weights_ready`. `SpeechStack` selects the slot only when that check passes. The protocol stub can still be started by the Flash harness or by running the script; that start is not a speech-slot selection. CI has neither the files nor the fetch flag.

## IPC

The landed request is one UTF-8 JSON line on stdin. `request_line` writes it. Control characters become spaces. The line carries no path.
# V2-S14 · Chatterbox Nano worker

Europe/Zagreb. The worker is runnable. A clone is live only when the gate below is open.

The spelling is **Galion**. The product is Galion Voice.


One OS process, PyTorch CPU, not linked into any Galion crate. Nano is the primary. Turbo is the same process with `GALION_CLONE_MODEL=turbo`. Piper speaks when this child is absent, unwired, or dead. Flite speaks when the Piper pack is missing. No GGML-family port. No in-tree ONNX Runtime. No candle. The child does not set a thread cap (t204u, t205u: full cores, full memory, self-offline).

`LOCKS.toml` is not edited by this change. `[formats.exceptions.v2s14_chatterbox]` still has `reference_clips = false`. That value blocks conditioning. `/v2/state` reports `"clone_live":false` until the gate is actually open.

## Run

Weights stay out of git. CI does not set the fetch flag and does not install PyTorch.

```bash
GALION_CLONE_FETCH=1 scripts/clone/fetch-clone-weights.sh

That fetches the pinned safetensors and the tokenizer text into `weights/clone/`. A hash mismatch deletes the partial. `.pt` and audio suffixes are refused.

On the machine that will speak, in a venv that is not committed:

```bash
python3 -m venv .venv-clone && . .venv-clone/bin/activate
pip install torch torchaudio --index-url https://download.pytorch.org/whl/cpu
pip install 'chatterbox-tts @ git+https://github.com/resemble-ai/chatterbox.git'

The Chatterbox tree has to be one that loads Nano through `ChatterboxTurboTTS.from_local(..., device="cpu", nano=True)`. That is upstream master as read on 2026-11-03. Perth is imported by `generate()`. It is not copied into this repo. If the import fails, the child writes no PCM and the lane uses Piper.

```bash
GALION_CLONE_GENERATE=1 GALION_CLONE_MODEL=nano GALION_SPEECH=clone \
  python3 scripts/clone/chatterbox_worker.py

The phone lane uses the same flags. `GALION_SPEECH=auto` selects this slot only when `weights_ready` matches `scripts/clone/PINS.sha256`. Without `GALION_CLONE_GENERATE=1` the child stays unwired and Piper speaks. `GALION_CLONE_STUB_PCM=1` is a pipe test. It is not a voice and not a Flash result.

Speak line, still with no path:

```text
{"text":"Hello.","model":"nano","persona":"samantha","tags":true}
```

`model` is `nano` or `turbo`. `persona` is `samantha` or `jarvis`. Any other persona string is Samantha, which is the rule in `persona_name`. Jarvis sets `tags` false. The stub also strips bracket tags when the persona is Jarvis, so a parent bug cannot leave them in. Samantha may keep tags for the later model (`[laugh]`, `[chuckle]`, and the set Nano and Turbo already document).

Stdout is raw s16le at 24 kHz. There is no WAV header and no file.

Stderr is the frame, after stdout is flushed, same idea as Piper's `Real-time factor:` line:

| Line | Meaning |
|---|---|
| `galion-clone-utt: <nbytes>` | Utterance boundary. The parent reads it in `read_utterance`. |
| `galion-clone-engine: protocol-stub` | Pipe test. `product_claim` stays false. |
| `galion-clone-engine: unwired` | Weights may be on disk. `generate` was not called. No PCM. |
| `galion-clone-engine: model` | Reserved for a real `generate`. This tree does not emit it. |
| `galion-clone-persona: <id>` | Register id. Not a performer. |
| `galion-clone-refuse: …` | Exit 3. The parent surfaces the tail and does not publish PCM. |
| `galion-clone-absent` | Exit 2, before the loop, when the stub is off and the pinned files are missing. |

The parent polls stdout and stderr with a 60 s ceiling, the same ceiling as Piper. That ceiling is a hang guard. It is not a latency target. The retained stderr tail is capped the way Piper caps `err_log`. The phone log does not receive the text, the PCM, or a path.

A speak that returns no PCM, or an `unwired` marker, is an error. `fallback_piper` then replaces the slot with A21 for the rest of the lane. The lane process is not restarted. If Piper is absent too, the error stands and flite is only in the picture when `detect` never selected the clone slot. `GALION_SPEECH=off` is still `NoSpeech` and the two-tone signal.

Barge-in uses the cancel token already on `SpeechStack`. Clauses not yet written are dropped. The child stays up. A buffer that returns after the turn is stale must be dropped before publish. The heartbeat does not wait on the pipe.

The bus `Persona` body (`IPC.md`, `SAMANTHA-V2.md` §4) is where the id is chosen. The clone child does not read the bus. The turn worker copies the id onto the JSON line. Depth 1. A cancelled turn does not get a special exemption.

### Conditioning line, specified now, refused now

`audio_prompt_path` does not join the speak line. A phone body must not be able to name a file, and a log of speak lines must not contain a path. Conditioning is a different stdin line, sent once per persona per process, only after the gates in the next section:

```text
{"op":"condition","persona":"samantha","audio_prompt_path":"/path/outside/the/repo/samantha.wav"}
```

Today the stub treats that key as a reference and exits 3 with no PCM. This plan leaves that refusal in place. A later patch may accept `op` only when both gates below are open. Until that patch exists, the parent does not send the line. `request_line` keeps writing the four speak fields and nothing else.

When that patch exists, success is stderr, not PCM:

```text
galion-clone-conditioned: samantha 10.2
galion-clone-utt: 0
```

The number is the clip length in seconds. The path is not echoed. The parent must treat `galion-clone-conditioned:` as a finished op with an empty buffer, and must not call `fallback_piper` for it. Today's parent treats empty PCM as failure, so sending this line before that parent change would demote the lane to Piper. The speak line that follows uses the cached conditioning and still has no path.

`{"op":"drop","persona":"samantha"}` zeros that persona's embeddings and forgets the path. It is the withdrawal op. It also writes no PCM.

A speak line whose `model` does not match the model loaded in that process is a refuse, and the parent respawns onto the other pinned T3 file. Both models are not loaded together.

## Lifecycle

The slot is born in `SpeechStack::detect` and dies with the lane.

1. **Absent.** `weights_ready` is false and the protocol stub is off. `CloneTts::try_new` fails. `detect` selects Piper, then flite. The script, if started anyway, exits 2 with `galion-clone-absent`. This is the CI state and the default demo state.
2. **Selected, not started.** `CloneTts` exists. `ensure` spawns on the first `synthesize`. `PhoneLane` pre-warm calls `speak("Hello.")` and discards it, which would be that first call once the slot is selected. The discarded line is text only. It carries no clip.
3. **Spawn.** `python3` on `scripts/clone/chatterbox_worker.py`, stdin/stdout/stderr pipes, stdout and stderr non-blocking. One child. `clone_spawns` counts starts. A child found dead *before* the next speak is spawned again. A child that dies *during* a speak fails that speak, and `fallback_piper` keeps Piper for the rest of the lane. There is no restart loop inside one utterance.
4. **Load (later).** Before the stdin loop, the child loads the four pinned safetensors for the model it was spawned with. No network. A `.pt` file is a fault, not a fallback format. Load failure exits. The parent treats that as a child that died during the first speak, and `fallback_piper` takes the lane.
5. **Ready (later).** One stderr line, `galion-clone-ready: nano cpu 24000`, before the first request. The current parent ignores unknown stderr lines. The first speak's clock still includes whatever load is left.
6. **Idle.** Weights resident. Conditioned embeddings resident only after a successful `condition` op. The process blocks on stdin.
7. **Speak.** Today: `unwired`, or protocol PCM. Later: `generate`, Perth, stdout PCM, `galion-clone-engine: model`, then `galion-clone-utt:`. Turbo without a cleared clip does not call `generate`. Turbo's public code asserts a reference longer than 5 seconds, so a clip-less Turbo call would fail that assert. Nano, if `generate` errors without `audio_prompt_path`, reports `unwired` and Piper speaks. Nobody invents a wav to make the call succeed.
8. **Refuse.** Exit 3 on any reference key, until the gates open. The process death plus `fallback_piper` takes the clone slot off the lane. That is the hold, and it stays.
9. **Cancel.** Unsent clauses are dropped. The child is not killed. PCM that arrives for a stale `turn_id` is dropped before publish.
10. **Teardown.** `Drop` closes stdin and kills the child. RAM embeddings die with it. Nothing is written on the way down. No PCM file, no conditioning checkpoint.

After sunset `2027-02-01` the t230u pin is no longer the authority for this process. Stopping it is an owner action. This plan does not turn the flag.

`GALION_CLONE_WEIGHTS` may point at a directory outside the repo. An empty value means the default `weights/clone` path. Neither path is a licence to commit the files.

## Clip pipeline after rights

Two gates, both closed, both required. Either one missing means the child keeps the exit-3 refusal and this repository still has no wav.

1. **Ledger.** Rights, consent, and licence for that specific clip, written in `LEDGER.md`: who consented, which performance, which licence, the duration, and whether a public demo is allowed. Seat 8. The comparison the owner requires is OpenAI's Sky voice, withdrawn in May 2024 after Scarlett Johansson said it resembled her performance in *Her*. Jarvis (an Iron Man film voice) and Samantha (the 2014 film *Her*) stay owner-directed research targets. The dispute is the warning, not a licence.
2. **Lock.** An owner edit of `[formats.exceptions.v2s14_chatterbox]` `reference_clips`. t230u set it false, and the notes forbid film audio and a celebrity clone. This document does not flip it. A true value is a new decision. It is not a reading of t230u.

`SAMANTHA-V2.md` §4 already separates the register from the clip. Ids, prosody, and wash may move mid-session. Fetching a clip, conditioning this worker, or a public demo in either film voice still waits on the two gates. The persona id on the bus remains `samantha` or `jarvis`. Conditioning does not put a performer's name on the dot.

After both gates, the order is:

1. A human places one mono wav per persona outside the git work tree, mode `0600`, owned by the user who runs the lane. Not under `weights/clone/` (that directory is the safetensors pin). Not fetched by `fetch-clone-weights.sh`, which already refuses audio suffixes. No rip script in the repo. No agent download.
2. The child accepts the file only when it is longer than 5 seconds (Turbo's assert in `tts_turbo.py`) and inside an 8–12 second window. The owner's target is about 10 seconds. One file then serves Nano and Turbo. A clip outside the window is refused. It is not padded and it is not looped. The 8–12 window is this plan's check. It is not an upstream constant. The encoder window constant on the Turbo class is 15× the 16 kHz rate, and the decoder window is 10× the synthesis rate (`VOICE-CLONING.md`). Those constants are why ~10 s was chosen. They were not re-measured here.
3. The child reads the path once and resamples to the conditioner's 16 kHz rate in RAM. Upstream `generate` wants `audio_prompt_path`, so a rate conversion may use a temp file created for that call and unlinked before the op returns. The human's file stays where they put it. The VoiceEncoder embedding, the speech-token prompt, and the S3Gen reference stay in RAM, keyed by persona.
4. Later speak lines for that persona use the cache. They do not repeat the path. There is no training step and no fine-tune.
5. Samantha's file is not Jarvis's file. Two keys. Jarvis still has tags stripped. Samantha may keep the documented bracket tags.
6. A public demo is a separate sentence in the ledger. A private `condition` op is not that sentence.
7. Withdrawal is `op: drop`, then the human deletes the file. There is no grace period, in the spirit of `withdraw_wipes_ram_history`. Restarting the process drops the embeddings as well, because they were never saved.
8. Generated reply PCM follows the existing reply box: RAM, cap 8, no disk (`SECURITY-EDGE.md`). The reference wav is not copied into the repo. A phone `POST /turn` does not carry the path or the wav. Art. 50 stays on the first turn. The disclosure says Galion is an AI assistant, not a person. It does not claim a performer licensed the voice. The ledger is the licence record.
9. Perth runs inside `generate` on the samples that come back. The first build watermarks that whole buffer. Partial-buffer watermarking waits on a reading of Perth during the streaming eval. That reading is not this plan.

## TTFA notes

No number is claimed here. Nothing in this plan was timed. `FLASH-EVAL.md` is the harness, and its default line stays the only honest output:

```text
{"measured":false,"product_claim":false,"engine":null,"note":"GALION_FLASH_EVAL is not 1; no TTFA number"}
```

There is no `ttfa_ms` key unless `GALION_FLASH_EVAL=1`. A protocol-stub clock is a test of the pipe. `product_claim` is true only for `engine=model`. This tree never sets that engine, because `generate` is not wired. Do not copy a stub number into `LEDGER.md`.

When an owner later allows a model run, the clock is the one `FLASH-EVAL.md` already names:

- start: the parent has written one speak line (`text`, model, persona)
- stop: the first PCM sample is in the parent's buffer
- machine: this CPU
- not a phone, not a VPS, not PSTN

Upstream `generate()` returns when the utterance is finished. The first byte and the last byte arrive together. Worker TTFA on that API equals full-utterance time. PR 528, open as of the 2026-06-17 read recorded in `VOICE-CLONING.md`, proposes `ChatterboxTurboTTS.stream()` so playback can start earlier. There is no Chatterbox model named Flash. The owner's "Flash streaming eval" is this CPU question. It is planned. It is not run.

`SpeechStack` already flushes a clause at punctuation or 8 words for the clone slot as well as for Piper (`CLAUSE_WORDS`). That split is how the parent can shorten the wait without `stream()`. It is still unmeasured for Chatterbox. It is not a streaming engine.

`CloneWorker` stores `cold_ms` on the first `synthesize` and `warm_ms` on the second, the same counters as Piper. A pre-warm `Hello.` consumes the first call and is discarded. A later ledger line has to say whether the published number includes that pre-warm. This file does not publish one.

Conditioning time is not TTFA. It runs once per persona per process, on the turn-worker thread, before a caller turn, and only after both rights gates. It does not run inside `Heartbeat::step`.

The Nano README claim of about 3× realtime on 8 CPU cores is a vendor claim (`VOICE-CLONING.md`). It is not a time-to-first-audio figure, and it is not converted into milliseconds here.

`barge_in_max_ms` is 200. That budget is the cancel path. It is not a synthesis budget. A slow `generate` must not stall `/dot`. `onset_p50_max_ms` is 300. This plan does not claim Nano meets it. Presence-eval TTFA stays `loopback-full-utterance` (`docs/eval/PRESENCE-EVAL.md`). It does not gain an engagement metric.

The five steps already listed in `FLASH-EVAL.md` are the real run. This plan adds one constraint to step 2: that first `generate` still sends no `audio_prompt_path`. A clock taken with a film clip is a different measurement and needs its own ledger line, after both gates. Piper's recorded warmth in `LEDGER.md` (one build machine, one phrase, cold in the hundreds of milliseconds, warm in the tens) belongs to Piper. It is not a Nano target.

## What this change refuses to do

- Edit `LOCKS.toml`, including `reference_clips`.
- Download Nano or Turbo, or run `fetch-clone-weights.sh`.
- Import torch in the stub, or add it to a Cargo manifest.
- Call `generate`, arm `GALION_FLASH_EVAL`, or write `ttfa_ms`.
- Add a wav, a temp clip, or a film file.
- Vendor Perth.
- Spawn the sixteen Chatterbox seats. They stay unspawned. Seat 5's contract is this file. The seat itself was not launched.
- Open a second HTTP server, a broker, or a cloud TTS client. Those APIs stay dropped (`VOICE-CLONING.md`).

A later build, in order, when an owner asks for it: keep the refusal and the pins; load Nano on CPU behind a default-off switch so CI stays unwired; emit `engine=model` only from a real `generate`; resample to 24 kHz inside the child if needed; respawn for Turbo; only then, after the ledger line and the owner lock edit, accept `op: condition` for one ~10 s wav per persona. The flagged Flash eval is after that, three CPU runs, written into `LEDGER.md` with the host and the model id. Still no public demo until the ledger says the demo is allowed.
Stdout is raw 24 kHz s16le. A non-empty `reference`, `audio_prompt`, `audio_prompt_path`, or `clip` on that line exits 3 and writes no PCM. Jarvis forces tags off. Samantha may keep `[laugh]` and `[chuckle]`.

Conditioning is a different line, sent by the parent only after the gate passes. The path is not written to stderr.

{"op":"condition","persona":"samantha","audio_prompt_path":"/absolute/outside/reference.wav"}

Success is `galion-clone-conditioned: samantha 10.000` and `galion-clone-utt: 0`. Later speak lines for that persona use the RAM embedding. `{"op":"drop","persona":"samantha"}` drops it. Nothing is written on the way down.

`generate` returns when the utterance is finished. There is no model named Flash. A clock with `engine=model` is a real generate. CI does not arm one. See `FLASH-EVAL.md`.

## What Maciej supplies

Two files per persona, outside the git tree. This repository does not fetch them, record them, trim them, or invent a sha256.

$GALION_CLIP_STORE/samantha/reference.wav
$GALION_CLIP_STORE/samantha/reference.wav.sha256
$GALION_CLIP_STORE/samantha/consent.txt
$GALION_CLIP_STORE/jarvis/reference.wav
$GALION_CLIP_STORE/jarvis/reference.wav.sha256
$GALION_CLIP_STORE/jarvis/consent.txt

`GALION_CLIP_STORE` is an absolute path outside the work tree. Unset means the store does not exist. There is no default under the repo, under `weights/`, or under `/tmp`.

Each wav:

- mono, 16-bit PCM
- about 10 seconds, inside 8–12 seconds (Turbo's public assert is longer than 5 seconds; the same file serves Nano and Turbo)
- one clip per persona
- Samantha: the performance in *Her* (2014)
- Jarvis: one Iron Man film performance, title and year copied from the instrument

A longer file is the wrong object. This tree does not cut it down.

`consent.txt` is text. Every field from `RIGHTS-CLEARANCE.md` / `CLIP-INGEST.md` has to be filled. No brackets, no empty value, no placeholder word:

`date` (include `CET` or `Zagreb`), `persona`, `work`, `performer`, `grantor`, `instrument`, `grantee`, `scope` (`local-zero-shot-conditioning`), `public_demo` (`no` until a public line exists), `expiry`, `operator`, `withdrawal`, `counsel`, `sha256`, `duration_s`.

```bash
GALION_CLIP_STORE=/absolute/outside scripts/clone/ingest-clip.sh samantha
GALION_CLIP_STORE=/absolute/outside scripts/clone/ingest-clip.sh jarvis

The first run prints `duration_s` and `sha256` measured from the wav. Copy those two into `consent.txt` and into `reference.wav.sha256`. The script does not write them for you. A mismatch leaves the wav in place.

## When `/v2/state` says clone_live

`clone_live` is true only when all of these are true for the current persona:

1. `scripts/clone/chatterbox_worker.py` is present.
2. The pinned Nano (or `GALION_CLONE_MODEL`) files match the sizes in `PINS.sha256`.
3. The wav and `consent.txt` pass the checker.
4. `reference_clips` is true in `[formats.exceptions.v2s14_chatterbox]`.

Otherwise the field is false and `clone_block` names the first miss (`store`, `wav`, `mono`, `duration`, `consent`, `expired`, `celebrity`, `sha`, `lock`, `weights`, `worker`, or `pending` when a store is set but no voice has been cloned in this process yet).

Since 2026-11-04 (owner decision: consent is checked only when a voice is cloned) this is computed once, at clone time, by the Rust gate in `crates/galion-consent` when the clone slot is built (`CloneTts::try_new`), by `ingest-clip.sh`, and before the `gqvoice` example spawns a real clone. The verdict is kept in memory; `/v2/state` and every turn only read it. No turn runs a consent check or spawns a process for it. `rights` stays `hold` until that gate is open, then `cleared`. `film_voice` stays false unless `public_demo` is `yes`. The page mouth stays the browser. The flag is readiness, not a claim that the dot is speaking the film.

Today the lock is false, so a placed wav still reports `clone_block` `lock` and `clone_live` false. Opening the lock is an owner edit of `LOCKS.toml`. This change does not make it.

Restart the demo after the files and the lock change. The floor reads the gate once at start.

## What this change does not do

- Tick a box in `RIGHTS-CLEARANCE.md`, or paste a LEDGER line that still has a blank field.
- Place, fetch, or hash a film wav.
- Vendor Perth, PyTorch, or the Chatterbox tree.
- Run generate in CI, or publish a TTFA number.
- Open a public demo. `public_demo: no` is the default. Art. 50 stays on. No nameplate on the mark.

— Maciej Grajczyk
