# OpenMind stack — desktop pair 03

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

Written 2026-11-08T04:30:42+01:00 (Europe/Zagreb, CET). This file is a read of source. It does not deploy, pair a device, call a model, or change a lock.

Pair 03 is the desktop reading. This note is file `04` in `docs/galion-context/2026-11-08/`. It maps Galion OS, the operator desk, Obsidian Open Mind, and Galion One, then lists the interfaces that exist in code and the Voice / One work that is still open.

## What was read

Authoritative OpenMind line, per [docs/SOURCE-OF-TRUTH.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/SOURCE-OF-TRUTH.md): GitHub `galion-studio/obsidian-open-mind`, branch `main`.

Pinned here:

| Item | Value |
|---|---|
| Commit | [`7ba85b603d8b0b87c4d4dede1056e1ee80c767ac`](https://github.com/galion-studio/obsidian-open-mind/commit/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac) |
| Subject | Merge `swarm/main-staging` into `main` (PR #12, publish 2026-10-28) |
| Committer time on that object | 2026-10-28T17:09:32Z |
| Default branch at read time | `main` at that same SHA |

GitHub `pushed_at` on the repository object was 2026-11-07T12:33:33Z. That stamp is not a newer `main` commit. Other branches exist (`save/2026-11-06-*`, `cursor/galion-pricing-footer-b040`, `swarm/android-admin-pairing`, and others). They are not the source of truth. This note does not treat them as shipped.

Voice integration was read from `galion-one-voice` `main` [`3e1f5a12c53cd8e0dd47d111676d61d608540559`](https://github.com/galion-studio/galion-one-voice/commit/3e1f5a12c53cd8e0dd47d111676d61d608540559) (merge of PR #102). The seam document already pins the same OpenMind SHA: [OPENMIND-NERVE-CORTEX.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md).

This machine, 2026-11-08: `/opt/galion` is absent. No SSH, no health GET, no gateway call, no key read. VPS sentences below are quotes from OpenMind docs with their own dates. They are not a live probe from this note.

Older OpenMind pages disagree with `7ba85b60`. Where they disagree, the file path at that commit wins. In particular:

- [README.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/README.md) still says several crates are absent from the snapshot and that the public release is 2026-10-31. Those crates are in this tree. The README feature table is a 2026-10-18 description.
- [docs/STATUS.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/STATUS.md) computes 60.7% unweighted done on 2026-10-21 against `main` `13a296c`, and says "Done" does not mean live. That percentage is not today's completion.
- [docs/crate-index.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/crate-index.md) (2026-10-19) still lists `openmind-route`, `openmind-fallback`, `openmind-origin`, and `galleon-voice` as their own crates. [SOURCE-OF-TRUTH.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/SOURCE-OF-TRUTH.md) records the 2026-10-20 fold: route and fallback live in `openmind-provider`, the voice engine winner is `galion-voice-core`, the listener winner is `galion-origin`.
- [docs/memory-api.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/memory-api.md) (2026-10-19) says the spare memory server 404s `POST /recall`. [api.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/openmind-memory/src/api.rs) on this commit routes `/recall`. The October study is a historical gap, not the current router.
- [docs/provider-map.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/provider-map.md) (2026-10-19) says every mapped crate opens no socket. That sentence covers the parsers it names. It does not cover `galion-one`, `galion-desk`, `galion-health`, or `hermes-gateway`, which do dial loopback.

## How the four names sit

```text
Galion OS          plan: ARM sandbox beside a stock Android phone
                   not a desktop image, not shipped by either repo read here

operator desk      galion-desk on 127.0.0.1:8646
                   JSON snapshot of the pod; POST applies a small local act

Obsidian Open Mind Rust workspace on the CPU box
                   Galion One binary, Hermes gateway, memory, nerve,
                   provider rows, voice traits

Galion Voice       this repository
                   product voice named Galion One
                   consumes Open Mind by wire; does not vendor it
```

**Galion OS** is a plan, copied into OpenMind on 2026-10-20. [docs/swarm/plans/galion-os.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/swarm/plans/galion-os.md) calls it the main admin interface: a secure, private, anonymous ARM sandbox that lives beside the owner's normal Android apps and desktop and does not replace them. The staged README says status is planning and that no build starts until the owner answers the acceptance questions. [docs/SLAVIK-TODO.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/SLAVIK-TODO.md) item 9 (2026-10-21) still asks for the exact phone model and a privacy confirmation before a build. The voice Ultra page says the same split from the other side: Galion OS is the tenant operating environment, and `galion-one-voice` does not ship the image ([PLAN-ULTRA.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/symphony/PLAN-ULTRA.md)).

**The desktop surface in code is `galion-desk`**, not Galion OS. It is the operator's desk endpoint. Default bind is `127.0.0.1:8646` (`DESK_BIND`). `GET /health` returns `{"ok":true}` and touches nothing. `GET /desk` returns one JSON snapshot. `POST /desk` applies one act: `note`, `drop`, `voice`, `stamp`, `hold`, `wake`, `rest`, or `focus`. Notes append to `desk-notes.jsonl`. `voice` writes the One voice file only when the name is `samantha` or `jarvis`. `wake` and `rest` POST to the configured Ollama `/api/generate` (default model string in the wake body is `galion:latest`). The Hermes mindpack is a separate binary, `desk-hermes`, described as a read-only open of Hermes state. Source: [crates/galion-desk/src/lib.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-desk/src/lib.rs), [crates/galion-desk/src/config.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-desk/src/config.rs). The 2026-10-20 cutover checklist in [docs/cutover/desk.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/cutover/desk.md) is unchecked and says the live VPS binary that day was an Oct 10 build whose source was not in git. This note did not check whether that cutover later happened.

**Obsidian Open Mind** is the Rust agent stack. The repository description is "Rust-first open agent stack for cheap CPU-only devices." [docs/main.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/main.md) (2026-10-18) is the dictated base: Linux and Rust, MIT, Europe/Zagreb timestamps, CPU-light box, extra compute added as external rows, real-money access not approved on that date. Later, the Galion OS plan records a redesign-only lift for Revolut-shaped mocks and still forbids transfers. A provider is a row (`cpu_local`, `lan`, `gpu`, `remote`) with approval `pending | approved | rejected` and health `unknown | up | down`. `ComputeProvider::health` must not open a socket. New rows start `Pending`. `Kind::Remote` is never chosen by the fallback. GPU is the first label in the choice order and is not a device the registry uses. Source: [crates/openmind-provider/src/lib.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/openmind-provider/src/lib.rs).

**Galion One is two different programs that share a name.**

| Name in prose | What the code is |
|---|---|
| OpenMind `galion-one` | HTTP process: text UI on `127.0.0.1:8091` (`ONE_BIND`), mouth on `127.0.0.1:8002` (`ONE_MOUTH_BIND`). Crate description: "the operator's own ear, mind and mouth on one box." |
| Voice persona Galion One | Product voice in [personas/GALION-ONE.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/personas/GALION-ONE.md). Samantha and Jarvis are internal test voices. |

The OpenMind voice snapshot still records the default persona as Samantha, voice id 0, and says the file is a record of live settings, not read by any binary yet: [config/voice/galion-one-voice.toml](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/config/voice/galion-one-voice.toml) (recorded 2026-10-20). The desk `voice` act accepts only `samantha` and `jarvis`. The product voice name Galion One is not a value that act writes.

## Interfaces at `7ba85b60`

State words: **code** means the function or route is in this commit; **stub** means the code is present and stops short of the real system; **record** means a dated document, not a probe from 2026-11-08; **proposed** means a plan with no listener in the file cited.

### Model and runtime

| Surface | State | Evidence |
|---|---|---|
| `VoiceBackend` trait: `name`, `capabilities`, `start`, `send_audio`, `end_of_user_turn`, `interrupt`, `poll`, `close`. `DuplexMode` is `Full` or `HalfWithBargeIn`. | code | [backend.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-voice-core/src/backend.rs). Comment names mock, PersonaPlex relay, and an M2 CPU pipeline. |
| `galion-voice-cpu` default features | stub until a feature is turned on | [Cargo.toml](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-voice-cpu/Cargo.toml): `default = []`. `whisper`, `kitten`, and `chatterbox` are optional. The crate comment says the text model is pluggable. |
| `galion-voice-serve` | code for ear and mouth wires; text model unwired | [serve.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-voice-cpu/src/serve.rs): ear `POST /v1/audio/transcriptions`, mouth `POST /synthesize`, `POST /v1/voice` is a round trip "for tests; no text model is wired in, so no paid API can be reached", `GET /health` is 200 only when every engine this process was asked to run is loaded. |
| llama.cpp, exo, DR Tulu, whisper/piper fixture parsers | stub parsers | [provider-map.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/provider-map.md): they parse caller-held bodies. Ports are constants. `dials_network` is false. `requires_gpu` / `uses_gpu` return false. |
| `persona-plex-relay` binary | stub | [main.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/persona-plex-relay/src/main.rs): "load config, print the plan, exit. Opens no sockets." |
| Persona Plex v2 GPU channel | proposed | README roadmap: real GPU host is "after the release". Weights are not bundled. |
| Voice-side student | stub | [galion-student-backend](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/crates/galion-student-backend/src/lib.rs): `load_slot` returns `CounselPending` until counsel is cleared, then `NotPlaced` or `Unpinned`. `sha256` on the manifest artifacts is `None`. Slot `[abi].target_verified = false` in [openmind-slot.toml](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/galion-student/configs/openmind-slot.toml). V2-S9 says the slot is a contract, not a running backend. |
| Galion One local mouth | stub unless `GALION_MOUTH` is set | [galion-one main.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-one/src/main.rs) `voice_fold`: "Today it is the Unavailable stub." `mouth_health` returns 503 `unavailable` with reason `no speech backend configured; set GALION_MOUTH to a galion-voice-serve address`, or proxies one GET to that address. |
| Hermes mind | code for a gateway process; paid provider not assumed | [hermes-gateway main.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/hermes-gateway/src/main.rs): config from `$HERMES_HOME/config.yaml`, key from `$API_SERVER_KEY` only, `--mock` is offline and spends nothing. This note did not start it and did not read a key. [SLAVIK-TODO.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/SLAVIK-TODO.md) item 4: a real paid benchmark stays off until the owner says yes. Item "OpenRouter: dodano 50 USD kredita" is a 2026-10-21 note, not a check that a call works on 2026-11-08. |
| Voice Hermes handoff | code for a local queue; transport unwired | [OPENMIND-NERVE-CORTEX.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md) §6: `CortexHandoff::local()` is the default. No gateway process, no secret, no HTTP client. `NullTransport` yields `NoKnowledge { reason: Unreachable }`. Cap is 3000 ms. |

### Galion One HTTP (text process)

From `handle` in [galion-one/src/main.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-one/src/main.rs). Defaults from [docs/one-paths.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/one-paths.md): page root `/workspace/galion-light` (`GALION_HOME`), Hermes dir `/workspace/hermes` (`ONE_HERMES_DIR`), bind `127.0.0.1:8091`, mouth bind `127.0.0.1:8002`.

| Route | Auth in this handler | Role |
|---|---|---|
| `GET /`, `/index.html`, `/nerve`, `/admin`, `/space`, font, mark | none | static page files under the page root; missing file becomes the bytes `missing` or empty |
| `GET /health` | none | body `ok\n`, status 200 |
| `GET /link` | none | JSON of ear, mouth, mind, eye addresses |
| `GET /api/v1/catalog` | none | reads `one/catalog.json` |
| `GET /api/v1/services` | none | probes Hermes and memory `/health`, plus a `/proc` comm check for `link-watch` |
| `GET /api/v1/awake` | none | awake JSON |
| `GET /me` | cookie read | `{"you":true\|false}` |
| `POST /pair` | pairing code | see pairing below |
| `GET /api/v1/analytics`, `POST /admin/say`, `GET /session`, `GET\|POST /files`, `GET /model`, `POST /model`, `GET /models`, `POST /voice` | cookie `galion_admin` | 401 `{"error":"pair"}` without a token |
| `POST /hear`, `POST /reply` | cookie | without a token the status is 200 and the body is `{"error":"pair"}` |

`GET /api/v1/services` dials `GALION_MIND` (default `127.0.0.1:8642`) and `GALION_MEMORY` (default `127.0.0.1:8420`) with `GET /health`. A closed port is `ok: false`. Source: [services.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-one/src/services.rs).

Mouth listener, same binary: `GET /health` and `POST /synthesize` (WAV out, or 500 `{"error":"voice"}` when synth returns empty).

[docs/cutover/galion-one.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/cutover/galion-one.md) records a VPS cutover at 2026-10-21 08:39:24 CEST from `swarm/one-no-shellout` `40454f90`, and a 17:22 probe that day: `8091/health` 200, `8002/health` 503. The 503 is described there as a hardcoded loading stub from before this mouth-health rewrite. This note did not repeat that probe. Do not treat 503, or 200, as the state of any host on 2026-11-08.

### Pairing

Three different pairing mechanisms. None of them was exercised against a live phone or a live admin store.

| Mechanism | State | What the code does |
|---|---|---|
| Galion One `POST /pair` | code, operator store required | [pair](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-one/src/main.rs) compares SHA-256 of the posted code with `codeHash` in an existing store file. No store: `{"error":"no"}` and it does not create the file. Five failures set `lockedUntil` 15 minutes ahead. Success returns `{"ok":true}` and a `galion_admin` cookie token. The store is rewritten in place at mode `0600`. |
| `omnode pair` | stub | [pair.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/edge/openmind-node/src/pair.rs): "v0.1 `pair` is a stub: it prints the signed payload and a terminal QR and does NOT contact the URL." `verify` is implemented and fail-closed. README usage line: `omnode pair --no-qr` sends nothing. |
| Console Android pairing | code, fixture test only | [console-pairing-e2e-20261027.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/real-tests/console-pairing-e2e-20261027.md): box-only, fake loopback API, fixture tokens, "no VPS". `POST /_ui/pair` with a good code returned 303 and set `galion_admin` plus `galion_device`. That run does not enroll a handset. |
| Hermes device enrollment | off unless env is set | `hermes-gateway` comment: `/openmind/*` enrollment is off unless `HERMES_RS_ENROLL=1` and `HERMES_RS_ENROLL_URL` are set. |

### Health

| Surface | Bind in source | Body |
|---|---|---|
| Galion One text | `127.0.0.1:8091/health` | plain `ok\n` |
| Galion One mouth | `127.0.0.1:8002/health` | JSON; 503 when no backend is configured |
| `galion-desk` | `127.0.0.1:8646/health` | `{"ok":true}` |
| `openmind-memory` | host/port from env; historical docs name `127.0.0.1:8420` | `GET /health` JSON from `engine.health()` |
| `galion-health` | `GALION_HEALTH_ADDR`, default `127.0.0.1:8660` | loopback page. Routes in the handler include `/status.json`, `/json`, `/healthz`, `/`. Probes are `GET <path>` on loopback plus `systemctl is-active`. Default check list includes memory 8420, hermes-rs 8642, desk 8646, tuner 8650, one 8091, console 8080 `/`, cloudflared 20241 `/ready`. Source: [galion-health/src/main.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-health/src/main.rs). |
| Nerve | voice seam documents `127.0.0.1:8670` | observation. Galion Voice does not start it. |

[docs/health-now.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/health-now.md) is one GET each on 2026-10-19: `8091/health` 200 `ok`, `8420/health` 200 JSON, `8080/health` 404. [docs/next.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/next.md) says the same three expectations "after any future switch" and tells the reader not to free 8080 and not to point memory clients at a new process on 8420. Those lines are instructions from 2026-10-19, superseded for git workflow by SOURCE-OF-TRUTH, and they are not a 2026-11-08 measurement.

### Memory

Router in [openmind-memory/src/api.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/openmind-memory/src/api.rs):

| Implemented as a handler | Returns 501 in this file |
|---|---|
| `GET /health` | `POST /seed` |
| `POST /recall` | `POST /v2/offload/compact` |
| `POST /capture` | `POST /v2/instance/destroy` |
| `POST /search/memories` | the 15 listed `/v2/conversation/*`, `/v2/atomic/*`, `/v2/scenario/*`, `/v2/core/*`, `/v2/pipeline/status` routes |
| `POST /search/conversations` | |
| `POST /session/end` | |
| `POST /v2/offload/ingest` | |
| `POST /v2/offload/query-mmd` | |
| `POST /v2/offload/read-ref` | |

v1 auth: if `OPENMIND_MEMORY_API_KEY` or `TDAI_GATEWAY_API_KEY` is unset, `verify` returns ok and the process logs `auth disabled`. If a key is set, a missing or wrong bearer is 401. v2 paths that are recognized also require a non-empty bearer and `x-tdai-service-id`. This note did not set either variable and did not send a body.

Schema tooling on this commit includes `openmind-memory schema <db>` and a `user_version` report ([main.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/openmind-memory/src/main.rs)). [docs/memory-schema.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/memory-schema.md) describes version 4 and an import flag. That page was written on branch `swarm/memory-schema`, based on main `13a296c`, on 2026-10-20. The `schema` subcommand is on `7ba85b60`. This note did not open a database, so it does not claim a live file is at `user_version` 4.

The 2026-10-19 memory study says the selected Hermes plugin was the Python `memory_tencentdb` provider, base URL `http://127.0.0.1:8420`, and that the study did not claim the Rust `hermes-gateway` binary issues those calls. Voice must not point its hot path at that port. [docs/next.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/next.md) says the same.

### Nerve

OpenMind event, schema version 1, [evt/mod.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/openmind-nerve/src/evt/mod.rs): fields `v`, `seq`, `ts` (Europe/Zagreb RFC 3339), `t`, `source`, `kind`, `severity`, optional `unit`, `group`, `key`, `msg`, `data`, `count`. HTTP ingest names `POST /event`, `POST /signal`, `POST /actions`. Stream name in the comment is `GET /events/stream`.

Voice side, [crates/galion-openmind](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/crates/galion-openmind/src/lib.rs): three kinds `voice.frame_miss`, `voice.fallback`, `voice.backend_up`. `note` only enqueues. A loopback `POST /event` happens only when `GALION_NERVE_URL` or `NERVE_URL` is a numeric loopback `http://` URL. Hostnames, `https`, and non-loopback addresses are refused. Unset means the RAM ring only. No PCM and no transcript on the line. The crate depends on `galion-sideband` and does not git-depend on OpenMind.

Incident cortex (`gaia-cortex` / nerve detectors) and the voice "cortex" (Hermes on the side band) are different. The voice seam says it does not ask the incident cortex to restart a unit.

## Pricing (owner statement, then the files)

Owner statement for this note, 2026-11-08, Europe/Zagreb: current pricing is **Galion One $20** and **Research Ultra $220 planned**.

The repositories do not yet use those two product names for those two numbers.

| File | What it says | What it does not say |
|---|---|---|
| [PLAN-PLUS.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/symphony/PLAN-PLUS.md) (2026-11-03) | Plus is $20 per month. Voice-to-voice sits inside that $20. COGS named as CX23-class, about $6. Rows `P-price` and `P-voice` are `open`. | It does not name the SKU "Galion One". |
| [PLAN-ULTRA.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/symphony/PLAN-ULTRA.md) (2026-11-03, source note t284u) | Ultra is $220 per month. Galion Research is a separate hosted console in the same owner note. Ultra does not include a second machine for it. | It does not name the SKU "Research Ultra" as a second price. |
| [LOCKS.toml](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/LOCKS.toml) `[licence]` | `free_forever = true`, `paid_tiers = false` | A price in a plan page does not flip this. |
| [GALION-VOICE-BRAND.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/brand/GALION-VOICE-BRAND.md) | The $220 Ultra host plan is not the brand. The one-dot page does not show a price. Putting the plan into `[licence]` needs an owner edit, counsel, and a ledger line. | |
| OpenMind README "Planned: Web3 license key system" | Planned, not implemented. Nothing in that repository checks, issues, or requires a key. | No $20 and no $220 in the pages read for this note. |

This file records the owner figures. It does not edit `LOCKS.toml`, does not add a checkout, and does not treat a gateway, an API key, or a paid call as working.

## TODOs, Voice and One first

These are the next safe readings and decisions. They are not permission to train, deploy, pair, or spend.

1. **Keep the voice student a stub.** `load_slot` refuses until counsel is cleared and every artifact is on disk with a pinned sha256. The manifest pins are `None`. V2-S9 says canned replies stay the fallback and the training go-ahead is not given. Do not download a teacher and call it the student.
2. **Keep Hermes off the 80 ms tick.** The voice handoff is `try_send` plus a 3 s cap, default local, no key. A missing nerve or a missing gateway is `NoKnowledge` or a RAM ring, and the local reply continues. Do not add an HTTP client on the frame.
3. **Do not vendor OpenMind.** The consume contract is the wire in `crates/galion-openmind`, pinned to `7ba85b60`. A later read has to name a new SHA before the seam claims a new field.
4. **Treat Galion One the product voice and `galion-one` the binary as separate.** The product persona is Galion One. The binary's recorded persona and the desk voice act are Samantha / Jarvis. Wiring the product name into `POST /voice` or the desk act is an owner decision, not a rename this note makes.
5. **Do not assume the mouth speaks.** Without `GALION_MOUTH` pointing at a `galion-voice-serve` that has actually loaded an engine, `GET :8002/health` is a 503 `unavailable`. `galion-voice-cpu` default features load no Whisper, Kitten, or Chatterbox. `POST /v1/voice` has no text model and must stay unable to reach a paid API.
6. **Do not assume pairing.** One `POST /pair` needs an operator-provisioned store and does not create one. `omnode pair` prints a payload and contacts nothing. The console e2e used fixture tokens on a box. Hermes enrollment stays off unless both enroll env vars are set. None of that was run here.
7. **Do not point Voice at memory port 8420, and do not assume the live binary matches `api.rs`.** Source on `7ba85b60` implements Hermes v1 recall, capture, search, session end, and three offload routes. `/seed`, compact, instance destroy, and the 15 v2 data routes return 501. Auth is disabled when no key env is set. The October docs still describe an older gap and a live process this note did not see.
8. **Desk cutover is a separate owner step.** `galion-desk` in git is the configurable binary. The 2026-10-20 runbook says the VPS was still on the Oct 10 binary and the checklist was open. `wake` will POST to Ollama if someone sends that act. Do not send it from Voice.
9. **Galion OS stays a plan.** Phone model and the privacy confirmation are still the gate in the 2026-10-21 owner list. Voice does not ship the image. Desktop work that is real today is `galion-desk`, `room`, and `galion-health`, on loopback.
10. **Pricing names need an owner edit before they are product copy.** The 2026-11-08 figures are One $20 and Research Ultra $220 planned. The 2026-11-03 pages say Plus $20 and Ultra $220, with `paid_tiers = false`. Aligning those names is a documentation decision for the owner. It is not a lock change and not a checkout.
11. **Paid calls stay unverified.** `--mock` is the gateway path that spends nothing. A historical credit line and a historical "key absent" line both exist in October notes. This read did not resolve them and did not call OpenRouter, Exo, llama.cpp, or a GPU host.
12. **Refresh STATUS.md only in the OpenMind repo, on its own PR, if someone is asked.** The 60.7% figure is tied to `13a296c` on 2026-10-21. Quoting it as the state of `7ba85b60` would be wrong.

## Sources

OpenMind, all at `7ba85b603d8b0b87c4d4dede1056e1ee80c767ac` (commit 2026-10-28):

- [Repository](https://github.com/galion-studio/obsidian-open-mind)
- [README.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/README.md)
- [docs/SOURCE-OF-TRUTH.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/SOURCE-OF-TRUTH.md) (2026-10-20)
- [docs/STATUS.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/STATUS.md) (computed 2026-10-21)
- [docs/main.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/main.md) (2026-10-18)
- [docs/SLAVIK-TODO.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/SLAVIK-TODO.md) (2026-10-21)
- [docs/swarm/plans/galion-os.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/swarm/plans/galion-os.md) (2026-10-20)
- [docs/swarm/galion-os-staging/README.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/swarm/galion-os-staging/README.md)
- [crates/galion-one/src/main.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-one/src/main.rs)
- [crates/galion-one/src/services.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-one/src/services.rs)
- [docs/one-paths.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/one-paths.md) (2026-10-19)
- [docs/cutover/galion-one.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/cutover/galion-one.md) (2026-10-21)
- [config/voice/galion-one-voice.toml](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/config/voice/galion-one-voice.toml) (2026-10-20)
- [crates/galion-desk/src/lib.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-desk/src/lib.rs)
- [crates/galion-desk/src/config.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-desk/src/config.rs)
- [docs/cutover/desk.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/cutover/desk.md) (2026-10-20)
- [crates/galion-health/src/main.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-health/src/main.rs)
- [docs/health-now.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/health-now.md) (2026-10-19)
- [crates/openmind-memory/src/api.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/openmind-memory/src/api.rs)
- [crates/openmind-memory/src/main.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/openmind-memory/src/main.rs)
- [docs/memory-api.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/memory-api.md) (2026-10-19)
- [docs/memory-schema.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/memory-schema.md) (2026-10-20, branch note)
- [crates/openmind-provider/src/lib.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/openmind-provider/src/lib.rs)
- [docs/provider-map.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/provider-map.md) (2026-10-19)
- [crates/galion-voice-core/src/backend.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-voice-core/src/backend.rs)
- [crates/galion-voice-cpu/Cargo.toml](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-voice-cpu/Cargo.toml)
- [crates/galion-voice-cpu/src/serve.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-voice-cpu/src/serve.rs)
- [crates/persona-plex-relay/src/main.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/persona-plex-relay/src/main.rs)
- [crates/hermes-gateway/src/main.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/hermes-gateway/src/main.rs)
- [crates/openmind-nerve/src/evt/mod.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/openmind-nerve/src/evt/mod.rs)
- [edge/openmind-node/src/pair.rs](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/edge/openmind-node/src/pair.rs)
- [docs/real-tests/console-pairing-e2e-20261027.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/real-tests/console-pairing-e2e-20261027.md) (2026-10-27)

Voice, all at `3e1f5a12c53cd8e0dd47d111676d61d608540559`:

- [docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md)
- [docs/galion-voice-v2/GALION-VOICE-V2.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/GALION-VOICE-V2.md) (V2-S9)
- [docs/galion-voice-v2/personas/GALION-ONE.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/personas/GALION-ONE.md)
- [galion-student/configs/openmind-slot.toml](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/galion-student/configs/openmind-slot.toml)
- [crates/galion-student-backend/src/lib.rs](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/crates/galion-student-backend/src/lib.rs)
- [crates/galion-openmind/src/lib.rs](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/crates/galion-openmind/src/lib.rs)
- [docs/galion-voice-v2/symphony/PLAN-PLUS.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/symphony/PLAN-PLUS.md) (2026-11-03)
- [docs/galion-voice-v2/symphony/PLAN-ULTRA.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/symphony/PLAN-ULTRA.md) (2026-11-03)
- [docs/brand/GALION-VOICE-BRAND.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/brand/GALION-VOICE-BRAND.md)
- [LOCKS.toml](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/LOCKS.toml) `[licence]`

Local check, this workspace, 2026-11-08: `/opt/galion` absent.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
