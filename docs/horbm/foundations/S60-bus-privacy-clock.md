# S60 — bus privacy and one clock

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S60. Keep private speech out of bus logs, and give the bus one clock. |
| Written | 2026-11-25. No weight download, no voice audio, no paid API, no merge. |
| Base | `main` `3c6751141f510f4c6538cd1cb07bafa2f63a98b5` |
| Gap | F1 cascade audit: (1) derived Debug on BusMsg/BusBody prints PCM and words; (2) BusMsg.t_ms and the stamp clock are different clocks with no conversion. |
| This change | Redacting Debug on the bus message and body, one anchor conversion, and this note. The bus send/recv path is unchanged. |
| Owns | `crates/galion-console/**` and this note. |
| Not edited | `horbm/crates/horbm`, `horbm-soak`, `galion-gates`, `galion-tensor`, `LOCKS.toml`, `SOUL.md`, root workspace members, workflows. |
| Consent | `remember=deny`, `learn=deny`, `learn_in_band=false`. Not edited. Ear prosody stays on (`GALION_EAR_SENSE` is the off switch). Samantha is the only persona. Aria is parked. |

## What F1 asked for

`docs/horbm/foundations/F1-cascade-audit.md` leaves two gaps.

Under `galion-heartbeat`: `BusMsg.t_ms` ("heartbeat time, frame × 80") and the stamp clock (monotonic microseconds) are different clocks. Nothing converts one into the other.

Under Process bus: a log of the bus bodies would contain PCM and words. The stamp line does not copy those bodies. A caller that debug-prints a `BusMsg` still would.

The stamp line in `galion-cascade` already omits PCM and words. `Domain04Stamps::log_line` in `crates/galion-cascade/src/stamps.rs` prints lengths and timings. `None` prints as `absent`. A zero is a real sample of `StampClock`, not a missing stage. The `frame_ms` key on that line is the locked quantum, `galion_heartbeat::FRAME_MS` (80). It is not `BusMsg.t_ms`. The stamps stay on the monotonic clock. `Heartbeat::step` is not called from the stamp sheet.

This swarm closes the bus `Debug` hole and adds the missing conversion. It does not stamp the bus and does not add a `BusBody` variant.

## Debug prints lengths, not speech

`BusMsg` and `BusBody` no longer derive `Debug`. `PartialEq`, the constructors, and `try_send` are unchanged. The bytes and the words stay in the value for the worker. They stay out of the log.

A `{:?}` log shows:

- `BusMsg`: `id`, `turn_id`, `t_ms`, `cancel`, and the body
- Bodies: the variant kind, numeric metadata (`rms`, `rate`, `ms`, `bits`, `is_final`, orb `level` / `env`, persona id and source enums), and lengths
- `Utterance` and `SpeechPcm`: `pcm_bytes` is the byte length, never the bytes
- `PartialAsr`: `text_chars`, `source_chars` (char counts), `is_final`. Not the transcript and not the source string
- `ReplyText`: `text_chars`, `emotion_chars` (`Option`). Not the words and not the emotion tag
- `Consent`: `bits`, `purpose_chars`. Not the purpose string
- `Fault`: `stage_chars`, `kind_chars`. Not the strings

| Variant | `{:?}` |
|---|---|
| `MicLevel` | `rms` |
| `Utterance` | `rate`, `pcm_bytes` |
| `SpeechPcm` | `rate`, `ms`, `pcm_bytes` |
| `PartialAsr` | `text_chars`, `source_chars`, `is_final` |
| `ReplyText` | `text_chars`, `emotion_chars` (`Option`) |
| `OrbState` | mode, `level`, `env` |
| `Consent` | `bits`, `purpose_chars` |
| `BargeIn` | `turn_id` |
| `Persona` | id, source |
| `Fault` | `stage_chars`, `kind_chars` |

`SendError` still derives `Debug`. `SendError::Full` and `SendError::TooLarge` carry a `BusMsg`, so their derived `Debug` inherits the redaction. `OrbMode`, `PersonaId`, and `PersonaSource` still derive `Debug`. They are enums, not speech.

The test is `debug_print_keeps_pcm_and_words_out_of_the_log` in `crates/galion-console/src/bus.rs`. It plants the canary `s60-private-speech-canary` in every text field and inside the PCM bytes, plus the byte run 201,202,203,204, and fails if any of that appears in `{:?}`.

## One conversion between the two clocks

The two clocks share a scale and not an origin.

| Clock | Where | Unit | Origin |
|---|---|---|---|
| Frame | `BusMsg.t_ms` | milliseconds, discrete | Heartbeat frame 0 of the sender. `t_ms` = frame index × `galion_heartbeat::FRAME_MS` (80). |
| Stamp | `StampClock::us` | monotonic microseconds | `std::time::Instant` captured at `StampClock::start`. |

`StampClock::us` is not wall time and not a frame index. It saturates at `u64::MAX` instead of wrapping. A zero is a real sample. `None` on a stamp sheet is `absent`.

The only conversion is `BusStampAnchor` in `crates/galion-console/src/bus_clock.rs`, re-exported from the console crate under feature `phone-turn`. The anchor stores the two `u64`s. It does not hold a `StampClock`.

| Name | Value |
|---|---|
| `BUS_FRAME_MS` | 80, asserted equal to `galion_heartbeat::FRAME_MS` |
| `STAMP_US_PER_MS` | 1_000 |
| `BusStampAnchor { frame_ms, stamp_us }` | one pair taken at the same moment |

`to_stamp_us(frame_ms)` is `anchor.stamp_us + (frame_ms - anchor.frame_ms) * STAMP_US_PER_MS`, computed in `i128`. A negative result saturates at 0. A result above `u64::MAX` saturates at `u64::MAX`.

`to_frame_ms(stamp_us)` takes `delta_us = stamp_us - anchor.stamp_us` in `i128`. `delta_ms` is `delta_us.div_euclid(STAMP_US_PER_MS)`, so a negative residue floors toward -infinity. Sub-millisecond residue is dropped. `ms` is `anchor.frame_ms + delta_ms`. A result at or below 0 is 0. A result at or above `u64::MAX` is `u64::MAX` before the snap. The snap is absolute from zero: `ms - (ms % 80)`.

Round-trip identity `to_frame_ms(to_stamp_us(t)) == t` holds when `t` and the anchor's `frame_ms` are both multiples of 80. The lattice is measured from zero. An anchor or an input off that lattice does not round-trip to itself. A stamp that falls 79_999 µs after a frame boundary stays on that frame. One microsecond before a frame boundary belongs to the previous frame. Sub-millisecond residue is dropped, not rounded.

`div_euclid` is what makes that boundary rule hold for a stamp before the anchor. One microsecond before the anchor, truncating division yields a 0 ms delta and stays on the anchor's frame. `div_euclid` yields -1 ms. When the anchor sits on a later frame, the snap then lands on the previous frame. At frame 0 that residue saturates at 0.

`ProcessBus::try_send` does not call the conversion. The sender's `t_ms` is stored as written. `try_send_keeps_the_frame_clock_the_sender_wrote` sends `t_ms = 160` and reads 160 back. The anchor's stamp for that same frame is a different number. This is not a second clock inside the bus. It is the map a log reader uses when a stamp sheet and a bus message must be compared.

Tests in `bus_clock.rs`: `frame_lattice_round_trips_through_the_stamp_clock`, `off_lattice_milliseconds_snap_down_to_the_frame`, `anchor_not_at_zero_converts_both_ways`, `times_before_the_origin_saturate_at_zero`, `saturation_at_the_top_of_u64`.

## How to run

The bus module compiles only with feature `phone-turn`. Default `cargo test -p galion-console` does not build it. The privacy test lives in `bus`. The clock tests live in `bus_clock`. Run both:

```bash
cargo test -p galion-console --features phone-turn --lib bus::
cargo test -p galion-console --features phone-turn --lib bus_clock::
```

That feature is Linux-oriented in the wider phone lane (the cascade phone ASR parent uses Unix `poll`). The privacy and clock tests are pure. They do not open a microphone, a model, or a socket. The new code has no `cfg(unix)` of its own.

## What did not change

- Queue depths, `try_send`, cancel, barge-in, persona floor, consent deny defaults
- No new `BusBody` variant, no domain 04 stamps written from the bus
- No new crate dependency
- `remember` / `learn` / `learn_in_band` untouched
- No voice audio, weights, or secrets in git
