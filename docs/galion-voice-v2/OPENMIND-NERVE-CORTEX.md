# Open Mind nerve and cortex

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. This is the voice-side seam. It is not a copy of Obsidian
Open Mind, and it is not a claim that the nerve or Hermes is running in
this workspace.

Code: `crates/galion-openmind`. Slot:
`galion-student/configs/openmind-slot.toml` (`schema = "galion.slot.v2"`).
Side-band types: `crates/galion-sideband`.

## 1. What Obsidian Open Mind is

Obsidian Open Mind is Galion's Rust agent stack: one process family for a
cheap CPU-only machine. The public description is "Rust-first open agent
stack for cheap CPU-only devices." The pieces this seam talks to are:

| Piece | Where it lives in that repo | What it is |
|---|---|---|
| Nerve | `crates/openmind-nerve` | Loopback service. Live unit name `galion-nerve`, port **8670**. Observes the box and accepts pushed events. |
| Nerve event | `crates/openmind-nerve/src/evt/mod.rs` `NerveEvent` | One JSON object per NDJSON line. Schema version **1**. |
| Nerve/cortex wire | `crates/nerve-cortex-contract` | The cortex (the incident one, below) parses that line without linking the whole nerve crate. |
| Voice backends | `crates/galion-voice-core` `VoiceBackend` | Poll-based speech engine. Mock, a CPU pipeline, the PersonaPlex relay, and — on this side — the student. |
| Hermes | `crates/hermes-rs`, `crates/hermes-gateway` | The analytical agent and its gateway process. |
| Voice slot record | `config/voice/galion-one-voice.toml` | A snapshot of live settings. Not read by Galion Voice. |

Galion Voice does not link any of those crates. The types in
`galion-openmind` were written to match the public wire, field for field,
after reading that tree. They are a consume contract, not a fork.

## 2. Where it lives

**Source.** <https://github.com/galion-studio/obsidian-open-mind>

Read on 2026-11-03 from `main` at
`7ba85b603d8b0b87c4d4dede1056e1ee80c767ac`
("Merge swarm/main-staging into main"). Links below use that commit so a
later edit of `main` does not silently change what this note claims.

**On a VPS, when a human has deployed it.** The intended tree is
`/opt/galion/openmind`. The voice snapshot in that repo records the
`galion-one` unit as
`ExecStart /opt/galion/openmind/dist/bin-ws/galion-one`
(`config/voice/galion-one-voice.toml`, `[sources_vps]`). That path is the
human's install. This repository does not create it.

**On this Cursor host.** `/opt/galion/openmind` is not present. This
program does not clone the Open Mind tree, does not vendor it, does not
SSH to Titanaxe, and does not download Chatterbox or any other weight.
The mouth model named in that voice snapshot is a record of another
machine. It is not fetched here. V2-S14 is unchanged.

## 3. Two different things called cortex

Open Mind uses "cortex" for the incident engine (`crates/gaia-cortex` and
`openmind-nerve`'s own cortex detectors). That cortex correlates unit
failures, asks the nerve to restart an allowlisted unit, and never runs a
shell command itself. Galion Voice does **not** send it problems to solve,
and it does not ask it to restart anything.

In this program, **cortex** means the analytical takeover: Hermes, on the
side band already typed in `galion-sideband`. Research, lookup, prediction,
and agent-assist go there. The voice keeps speaking from local state while
that happens. V2-S15 is that decision. This note is the hook.

Nerve events may still be visible to Open Mind's incident cortex if a
human is running `galion-nerve`, because they travel on the same NDJSON
stream. That is observation. It is not the path that answers a question.

## 4. How the nerve sees the voice engine

Two directions.

**Voice observes itself and tells the nerve.** After each heartbeat frame
(never inside `Heartbeat::step`), the host may enqueue a `VoiceNote`:

| Kind | When | Group | Severity |
|---|---|---|---|
| `voice.frame_miss` | `FrameOut::paused` is true (late frame, or the you-band PCM was not one frame) | `galion-os` | `medium` |
| `voice.backend_up` | A voice backend became available | `galion-os` | `info` |
| `voice.fallback` | The supervisor left one backend and continued on another | `hermes` | `high` |

A healthy frame emits nothing. The nerve does not get a line at 12.5 Hz.

`voice.fallback` is grouped `hermes` on purpose. The audio path has
already switched backend. Understanding why is analytical work, so the
event is labelled for the Hermes group. `voice.frame_miss` and
`voice.backend_up` stay on `galion-os`: they are the voice clock and the
voice process, not a thinking task.

Those three strings are `[hooks].nerve_kinds` in the slot file. A unit
test reads the file and fails if the crate and the slot disagree. The
slot values were not changed.

**The nerve observes the voice process from outside.** When a human runs
`openmind-nerve`, it already probes health, systemd units, and journals.
The voice snapshot names the mouth component `galion-one-mouth` and the
nerve at `127.0.0.1:8670`. That probe is the nerve's own loop. Galion
Voice does not start it.

Groups `galion-os` and `hermes` are inventory groups in
`openmind_nerve::evt::classify_unit`. A unit whose name starts with
`galion-os` is `galion-os`. A unit whose name starts with `hermes` is
`hermes`. The local NDJSON line sets `group` directly. The push body also
carries `data.unit` so that, if the nerve rebuilds the event from
`POST /event` (`from_http_event`), it classifies the same way:

| Group | `data.unit` |
|---|---|
| `galion-os` | `galion-os-voice` |
| `hermes` | `hermes-sideband` |

`source` on the voice line is `galion-voice`. The nerve's HTTP ingest
rewrites the streamed source to `http` and keeps the original fields in
`data`. Both shapes are accepted by `nerve-cortex-contract`: the full
schema-v1 object, and the older B33 envelope
`{source, kind, severity, data}`.

No PCM, no transcript, and no affect sample is put on a nerve line. The
message strings are fixed: "voice frame missed", "voice backend fell
back", "voice backend up".

## 5. What happens when no nerve is running

`NerveEmitter::note` copies the note into a bounded queue and returns.
It does not format JSON, read the clock, take the line lock, or open a
socket. A thread named `voice-nerve` formats the line and stores it in a
RAM ring (64 lines by default, oldest dropped).

Optional, still on that thread:

* `GALION_NERVE_LOG` — append the NDJSON line to a file. If the path
  cannot be opened, the ring still holds the line (`log_skipped`).
* `GALION_NERVE_URL`, or else `NERVE_URL` — `POST /event` to a **numeric
  loopback** `http://` address, for example `http://127.0.0.1:8670`.
  The path in the URL is ignored; the worker always posts `/event`.

Anything that is not numeric loopback HTTP is refused. Hostnames are
refused so the parser never blocks in DNS. `https://` is refused.
`0.0.0.0` and every non-loopback address are refused. A refused URL
increments `forward_refused` and is not contacted. Unset means local
only, which is the default and the state of this workspace.

A dead nerve costs one failed connect, then a 2 s back-off. The frame
host does not see that. A full queue returns `NoteDrop::QueueFull`.
The frame continues.

`note` itself allocates nothing on the caller after the queue exists
(`tests/note_alloc.rs`). It is still a channel, and `Heartbeat::step`
forbids channels. Call it after `step` returns. `note_from_frame(paused,
frame)` is the mapping from that return value. Fallback and backend-up
are supervisor calls, not guesses from the heartbeat.

Dropping the emitter joins `voice-nerve`. Do that at session end. A slow
post can make the join wait. It must not run inside the tick.

## 6. How Hermes takes over

The audio tick does not ask Hermes anything.

At a **turn boundary** the turn worker (the same thread that already
owns ASR, the canned reply, and speech) may call `CortexHandoff::offer`
with three `galion-sideband` values:

| Event | Type | Role |
|---|---|---|
| `ToHermes::Intent` | `Intent::Chat`, `Lookup { query }`, or `Action { name, tier }` | What the engine thinks the person wants. Starts the 3 s clock. |
| `ToHermes::Turn` | `Turn` | The words of the turn. Text only. |
| `ToHermes::Affect` | `Affect` | Valence, arousal, confidence. Clamped before send. |

`offer` only `try_send`s onto the `hermes-sideband` thread inside
`SidebandClient`. A full queue drops. It never waits. `poll` is the same
non-blocking poll the side band already has.

The retrieval cap is `galion_sideband::RETRIEVAL_CAP` (3_000 ms), the
same number as `[latency].hermes_cap_ms` in the slot. Past the cap,
`poll` returns `NoKnowledge { reason: Timeout }`. If nothing is listening,
`NullTransport` returns `NoKnowledge { reason: Unreachable }`. The voice
then uses its local filler or backchannel (the canned reply, today) and
does not invent a fact. The slot names that
`hermes_timeout_action = "filler_or_backchannel"`. This crate does not
synthesize the filler. The phone lane already has the local reply.

`Intent::Action` at tier `Money` is not offered
(`Intent::voice_may_request` is false). Turn text and affect may still
be queued so Hermes has context. The payment order is not.

`CortexHandoff::local()` is the default: no gateway process, no secret,
no HTTP client. `hermes-rs` and `hermes-gateway` are where a later
transport would talk, on the side thread, behind `HermesTransport`.
That transport is still a trait. This landing does not call it, and it
does not read a key. V2-S15 stands: do not call Hermes from the record
that approved the gateway.

Consent is unchanged. Affect on this band is whatever the caller already
decided was allowed. The phone lane still passes neutral while `sense`
is deny. This hook does not grant `sense`.

## 7. The fourth voice backend

`openmind-slot.toml` says `StudentBackend` is the fourth Open Mind
`VoiceBackend`, after Mock, CpuPipeline, and Relay/PersonaPlex.

`galion-voice-core` on the commit above defines the trait. These methods
were read from `crates/galion-voice-core/src/backend.rs`:

`name`, `capabilities`, `start`, `send_audio`, `end_of_user_turn`,
`interrupt`, `poll`, `close`.

`DuplexMode` is `Full` or `HalfWithBargeIn`. `BackendEvent` is `Audio`,
`Text`, `TurnDone`, or `Unavailable`. A session tick
(`galion-voice-core` `Session::tick`) is one audio frame: VAD, send,
poll, playout. That tick is the Open Mind voice hot path. Hermes is not
inside it. Neither is `POST /event`.

The slot's `[abi].target_verified` stays `false`. That flag means
"verified on the box." The crate is not on this host, so the flag was
not flipped. The GitHub read is recorded here instead.

`VoiceBackendSlot` in this crate is the voice-side code carried in
`data.code` and, for a fallback, in `from` / `to`:

| Slot | Code | Wire name |
|---|---|---|
| Mock | 1 | `mock` |
| CpuPipeline | 2 | `cpu_pipeline` |
| Relay | 3 | `relay` |
| Student | 4 | `student` |

A fallback from relay to student stores code 772 (relay in the high
byte, student in the low byte). The JSON also names `from` and `to`, so a
reader does not have to decode the number. The student in this repository is still the silent stub in
`galion-student-backend`. This seam does not load weights and does not
replace `CannedLlm`.

## 8. Dependency and consume path

Do not add a `git` dependency on `obsidian-open-mind`. Do not copy its
crates into this tree. The versions move on their own schedule, and the
voice binary must build with the nerve absent.

When a human has deployed Open Mind:

1. The nerve listens on loopback `127.0.0.1:8670` (`openmind-nerve serve`).
2. The voice process is given `GALION_NERVE_URL=http://127.0.0.1:8670`
   if that human wants the push. Until then the RAM ring is the whole
   observation path.
3. Hermes, if it is used, is reached by a future `HermesTransport` on the
   side-band thread. The 80 ms loop still only `try_send`s and `try_recv`s.
4. `/opt/galion/openmind` is that human's checkout or install prefix. It
   is not a path Galion Voice opens by itself.

`galion-openmind` depends only on `galion-sideband` (std, no crates.io
crates). No new lock value. `LOCKS.toml` was not edited.

## 9. What this landing does not do

* It does not run inside `Heartbeat::step`, and it does not allocate on
  the caller of `note`.
* It does not put Hermes, HTTP, or a file write on the audio tick.
* It does not vendor Open Mind, clone it, or SSH to install it.
* It does not download Chatterbox, Piper, Vosk, or a student weight.
* It does not change `nerve_kinds`, `hermes_cap_ms`, or any other slot
  value. A comment under `nerve_kinds` points here.
* It does not grant consent, and it does not read the caller's voice for
  affect.
* It does not ask `gaia-cortex` to restart a unit.
* It does not synthesize filler audio. Timeout and unreachable are
  `NoKnowledge`. The local reply path is the filler.

## 10. Check

```bash
cargo test -p galion-openmind
scripts/check-locks.sh
scripts/check-no-forbidden.sh
```

`note` under a stuck worker returns in well under one 80 ms frame.
`offer` does the same. A URL of `http://10.1.2.3:8670` is refused and
still leaves `voice.frame_miss` in the local ring.

— Maciej Grajczyk

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
