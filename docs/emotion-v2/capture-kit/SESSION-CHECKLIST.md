# Emotion V2 — session checklist

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. One card is one session id. A change of room, microphone, gain,
or distance is a new session id and a new manifest. Fill this card before
arming record. Keep it as `SESSION.txt` next to the session manifest.

The procedure behind every row is [`../CAPTURE-PROTOCOL.md`](../CAPTURE-PROTOCOL.md).

## Before the session

- [ ] Mate's recording consent is signed for this session (private copy of
      [consent.template.json](consent.template.json)). Do not record without it.
      Do not invent a signature.
- [ ] Line to record: `samantha` / `jarvis`  (circle one)
- [ ] Session id: `YYYY-MM-DD-<room>` ________________________________
- [ ] Room (same closed room for the tone and every take): _______________
- [ ] Microphone (one model, written, does not change): _________________
- [ ] Distance: 30 cm from the capsule to the mouth mark. Mark the spot.
- [ ] The 432 Hz source will sit on that same mark for calibration.
- [ ] Preamp gain will be set on the calibration tone, then left alone.
- [ ] Recorder processing off: no AGC, no limiter, no noise reduction, no EQ,
      no high-pass, no compressor.
- [ ] Format set to 48 kHz, 24-bit packed integer PCM, mono WAV.
- [ ] Carrier sentence (same wording for all 12 official emotions) is chosen
      in the owner's own words and written here:
      ________________________________________________________________
      Suggested starting points are in [PROMPTS.md](PROMPTS.md). The owner
      may replace them. This is not a film line and not another person's wording.
- [ ] Operator is the owner.

## Calibration (first)

- [ ] 432 Hz source on the mouth mark. A loudspeaker or an oscillator. Not a person.
- [ ] Record `calibration/take-01.wav` at the session gain.
- [ ] 0.5 s of quiet room before the tone, 0.5 s after.
- [ ] Format check:
      `python3 tools/emotion-v2/capture-kit/write_manifest.py check --wav <file> --role calibration`
- [ ] Acoustic check:
      `cargo run --manifest-path tools/emotion-v2/refcheck/Cargo.toml -- check --wav <file> --spec docs/emotion-v2/capture-spec.toml --role calibration`
- [ ] If the tone clips or misses the loudness window, start a **new session id**
      at a lower or higher gain. Two gains never share a manifest.

## Official emotions (12 × 5 takes, ≥ 3 PASS each)

Record in sector order, or any order, but finish a sector before a long break
if you can. Neutral is an emotion. It is not the 432 Hz tone.

For every take: mouth on the mark, same gain, 0.5 s quiet before and after,
same carrier sentence. A failed take stays in the folder. If three passes are
not in hand, record `take-06` and so on at the same gain. If the room or the
gain has to change, start a new session id.

### Grounded

- [ ] `neutral` — take-01 to take-05, at least 3 acoustic PASS
- [ ] `calm` — take-01 to take-05, at least 3 acoustic PASS

### Bright

- [ ] `happy` — take-01 to take-05, at least 3 acoustic PASS
- [ ] `amused` — take-01 to take-05, at least 3 acoustic PASS
- [ ] `excited` — take-01 to take-05, at least 3 acoustic PASS
- [ ] `tender` — take-01 to take-05, at least 3 acoustic PASS

### Heavy

- [ ] `sad` — take-01 to take-05, at least 3 acoustic PASS
- [ ] `disappointed` — take-01 to take-05, at least 3 acoustic PASS

### Sharp

- [ ] `angry` — take-01 to take-05, at least 3 acoustic PASS
- [ ] `fearful` — take-01 to take-05, at least 3 acoustic PASS
- [ ] `surprised` — take-01 to take-05, at least 3 acoustic PASS
- [ ] `disgusted` — take-01 to take-05, at least 3 acoustic PASS

## Optional, Samantha only, after the twelve

- [ ] Not recording `seductive` (default), or
- [ ] Samantha session only: five optional `seductive` takes, same room / mic /
      gain / 30 cm. These do not count toward the session seal. Do not record
      them on a Jarvis card.

## After the takes

- [ ] Write the format + sha256 session manifest
      (`write_manifest.py session --dir ... --line ...`).
- [ ] Run `refcheck` `manifest` so acoustic verdicts are stored on the same file.
- [ ] `chmod a-w` every WAV. Originals are read-only.
- [ ] `verify-manifest --require-session` PASSes (hashes match, WAVs stay
      read-only, ≥ 1 PASS calibration, ≥ 3 PASS takes for each of the twelve).
- [ ] Select one acoustic PASS take per official emotion into
      `per-sector-refs.json` via `write_manifest.py refs`. Do not rewrite audio.
- [ ] WAVs and any signed consent copy stay outside git.
- [ ] If any take will be used for training: stop. Slavik's signed
      `consent_scope` including `training-data-generation` must already exist.
      Do not invent that record. Do not start
      `tools/emotion-v2/dataset` without it.

## Session card fields (copy into SESSION.txt)

```
session_id:
line:
room:
microphone:
distance: 30 cm
gain_position:
processing: off
carrier_sentence:
operator:
calibration_file:
standard: galion-voice-v2-432
consent: owner-own-recordings-only
recording_consent: Mate, signed private copy on file (yes/no, date only if signed)
training_consent: not used / Slavik signed training-data-generation (do not invent)
```
