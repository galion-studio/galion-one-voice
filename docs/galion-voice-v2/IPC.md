# V2-S1 · Process bus — message shapes

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Status: design for the bus. The code in this program still uses the V1 channels
(`MicTap`, `TurnInbox`, `ReplyBox`, `TurnWorker`'s `sync_channel`). Do not build
a second runtime beside those. The bus replaces them when V2-S1 is implemented.

The 80 ms heartbeat thread is never allowed to block on a model. Every send below
is `try_send`. A full queue drops or coalesces; it does not wait.

## Participants

| Process | Owns | Must not own |
|---|---|---|
| Capture | mic graph, VAD, utterance bytes | LLM, TTS, disk |
| Heartbeat | 80 ms `FrameIn` / `FrameOut`, Rika, consent bits | model weights |
| ASR | one utterance → transcript + label | the speakers |
| Reply | transcript → reply text | raw audio retention |
| Speech | reply text → 24 kHz PCM | the HTTP socket |
| Render | dot / orb from levels | inference |
| Gate | consent bits, barge-in cancel | a private copy of audio |

Today capture and render share the browser page, and ASR / reply / speech share
`PhoneLane` inside one worker thread. The bus is what lets those split later
without another ad-hoc mutex.

## Envelope

```text
BusMsg {
  id:        u64            // monotonic per sender
  turn_id:   u64            // the utterance this belongs to; 0 if none
  t_ms:      u64            // heartbeat time, not wall clock
  cancel:    u64            // drop anything with turn_id < cancel (barge-in)
  body:      one of the bodies below
}
```

`cancel` is the token `TurnWorker::supersede` already is. A message whose
`turn_id` is older than the latest cancel is discarded by the receiver before
any work, and again before publish, the way the worker already skips a job
that went stale while Piper was running.

## Bodies

```text
MicLevel      { rms: f32 }                          // already POST /mic; coalesce, keep latest
Utterance     { rate: u32, pcm: bytes }             // one VAD cut; max 4 MiB (MAX_BODY_BYTES)
PartialAsr    { text: text, source: label, final: bool }
ReplyText     { text: text, emotion: tag }          // emotion tag only if sense is granted
SpeechPcm     { rate: 24000, pcm: bytes, ms: u32 }  // drives the me band
OrbState      { mode: idle|listening|thinking|speaking|offline, level: f32, env: f32 }
Consent       { bits: u8, purpose: tag }            // mirrors EmotionWire; default deny
BargeIn       { turn_id: u64 }                      // sets cancel
Fault         { stage: text, kind: text }           // no raw audio, no transcript unless debug
```

`source` on `PartialAsr` is `stub-fixture`, `interim-placeholder`, or a future
ratified engine id. The UI must be able to tell a placeholder from recognition.
That label exists today as `AsrSource`.

## Backpressure

| Queue | Depth | When full |
|---|---|---|
| MicLevel | 1 | replace (latest RMS wins) |
| Utterance | 8 | drop oldest (`MAX_PENDING_TURNS`) |
| SpeechPcm / replies | 8 | evict oldest WAV (`MAX_STORED_REPLIES`) |
| worker job | 1 | `try_submit` returns the job; host keeps one pending, newest wins |
| worker reply | 4 | drop the reply; never block the worker |

No queue grows with call length. There is no disk spill.

## What "replace ad-hoc shared state" means

Today the screen demo shares four objects by cloning `Arc<Mutex<_>>` out of
`WebSurface` (`MicTap`, `TurnInbox`, `ReplyBox`) plus an `mpsc` pair inside
`TurnWorker`. That is the right shape for one process. V2-S1 names those
queues, gives them the envelope above, and adds the cancel token as a field
every stage checks. It does not add a broker, a socket framework, or a second
HTTP server.

Acceptance for the bus itself is in `GALION-VOICE-V2.md` under V2-S1.

— Maciej Grajczyk
