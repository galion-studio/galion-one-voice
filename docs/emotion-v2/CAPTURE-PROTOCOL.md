# Emotion V2 — controlled reference capture

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-07. Emotion V2 foundation, part B.

The spelling is **Galion**. The product is Galion Voice. The standard is **galion-voice-v2-432** (the 432 Hz standard).

This page is the capture procedure for Samantha's emotion references. The machine gate is [`capture-spec.toml`](capture-spec.toml). The checker is `tools/emotion-v2/refcheck`. It reads a WAV and that spec, and it writes a JSON report of PASS or FAIL.

The point of the gate is that a later emotion score measures the voice. The room, the microphone, the gain, and the distance are a constant. A take that carries a different room or a different chain fails here, before any emotion score exists.

## What this capture is

Samantha's twelve emotions, spoken by the owner, in one room, into one microphone:

Neutral, Calm, Happy, Amused, Excited, Tender, Sad, Disappointed, Angry, Fearful, Surprised, Disgusted.

The owner is the only voice. The sentence is one the owner chooses, in the owner's own words, and the same sentence is used for every emotion so a later score can compare the emotion. It is not a film line and not another person's wording.

The 432 Hz calibration take is a tone at the mouth position. It is not a voice, not Neutral, and not a clone reference.

## Consent

`consent = "owner-own-recordings-only"`.

The WAV is a recording the owner made of their own voice for this internal test register. It is not a recording of another person, not a clip from a film or a call, and not model output. The checker refuses a spec whose consent field is anything else. It does not synthesize speech and it does not clone a voice.

Originals stay on the machine that recorded them. They are not committed. Weights are not part of this step.

## One room, one microphone, fixed gain, fixed distance

Write a session card before arming record. One card is one session id. A change of room, microphone, gain, or distance is a new session id and a new manifest.

| Card field | Rule |
|---|---|
| Session id | `YYYY-MM-DD-<room>`, one room for the whole card |
| Room | The same closed room for the calibration tone and every emotion take |
| Microphone | One microphone. Model written on the card. It does not change |
| Distance | **30 cm** from the capsule to the mouth mark. The 432 Hz source sits on that same mark |
| Gain | Preamp gain set on the calibration tone, then left alone for every take |
| Processing | Recorder processing off: no AGC, no limiter, no noise reduction, no EQ, no high-pass, no compressor |
| Operator | The owner |

30 cm is close enough that the voice is in front of the room, and far enough that a small lean does not become the bass. Mark the spot. The mouth stays on the mark. The checker cannot see centimetres. A move shows up as level, spectral tilt, noise, or reverb, and the take fails.

Set the gain so the calibration tone lands in the loudness window and under the true-peak cap. After that the knob stays. If an emotion take misses the loudness window, record that take again at the same gain. If most takes miss in the same direction, the gain choice is wrong: start a new session id. Two gains never share a manifest.

The file that is hashed is the file the recorder wrote. Level, noise, and reverb are not repaired in an editor.

## Signal

Packed WAV, little-endian.

| Gate | Value |
|---|---|
| Sample rate | 48000 Hz |
| Bit depth | 24-bit signed integer PCM, three bytes per sample |
| Channels | 1 (mono) |
| Container | Format tag 1, or WAVE_FORMAT_EXTENSIBLE whose subformat is PCM |

24-bit samples packed in a 32-bit word are a different file. Float WAV is a different file. Both fail `bit_depth`.

Each take, including calibration, contains:

1. **0.5 s** of the quiet room before the sound (the noise-floor window is 400 ms).
2. The tone or the sentence.
3. **0.5 s** of the quiet room after the sound (the reverb estimate reads this tail).

Leave the input processing off so this quiet is the room, not a gate that closed.

## Loudness, peaks, noise, DC

The meter is **mono ITU-R BS.1770-4**: K-weighting, 400 ms blocks, 100 ms hop, absolute gate -70 LUFS, relative gate -10 LU, channel weight 1. One full-scale 1 kHz sine reads about -3.0 LUFS on this meter.

| Check | Pass |
|---|---|
| Integrated loudness | **-20.0 LUFS ± 2.0 LU** |
| True peak | **≤ -1.0 dBTP**, 4× oversampled |
| Noise floor | Quietest 400 ms window **≤ -60 dBFS** RMS |
| DC offset | **≤ -50 dBFS** (20 log10 of the absolute mean) |
| Emotion duration | **3.0 s to 12.0 s**, including the quiet ends |
| Calibration duration | **2.0 s to 8.0 s** |

-60 dBFS is a quiet room at this gain. A take that fails it is the room. The repair is the room, or a new session in a quieter room. It is not a denoiser.

## Clipping

A clipped sample is a full-scale code: the positive rail (`2^23 - 1`) or the negative rail (`-2^23`) in 24-bit PCM.

- `clipping` allows **0** rail samples.
- `true_peak` still fails when no sample hits the rail and an inter-sample peak exceeds **-1.0 dBTP**.
- One rail sample fails the take.
- The waveform is not redrawn and not limited after the fact.
- If the calibration tone clips, the gain is too hot. New session id, lower gain, calibration first.
- If one emotion take clips, that take stays in the folder (it will hash as FAIL) and another take is recorded at the same gain.

## Reverb estimate

`reverb` is a Schroeder T20: after the last moment of the sustain, reverse-integrate the tail with the quiet-window power removed, and extrapolate the slope from **-5 dB to -20 dB**. The pass limit is **≤ 0.40 s**.

This is a screen, not an ISO 3382 lab number. A dry close capture falls well under 0.40 s. A live room fails. The trailing 0.5 s is what makes the slope visible. A file that ends while the voice is still loud has no estimate and fails. De-reverb is not applied to the original.

## Spectral tilt and the 432 Hz grid

The reference frequency is **432 Hz**. Octave bands are centered at `432 × 2^k` for `k = -2 … 4`:

| k | Center |
|---|---|
| -2 | 108 Hz |
| -1 | 216 Hz |
| 0 | 432 Hz |
| 1 | 864 Hz |
| 2 | 1728 Hz |
| 3 | 3456 Hz |
| 4 | 6912 Hz |

Band edges are the geometric half-octaves around each center. Tilt is the slope of power spectral density (mean power per bin, 10 log10, so bandwidth does not add a false +3 dB/octave) against log2 of the center. The emotion-take target is **-6.0 dB/octave ± 2.0**.

The fit uses the active part of the take (frames within 20 dB of the peak), Hann windows, FFT size 8192. A hidden EQ or a change of microphone moves this slope. The original is not equalized to force a pass.

The calibration take does not use tilt. Its gate is `calibration_tone`: the dominant frequency is **432 Hz ± 2 Hz**.

## Calibration take

Once per session, before the emotions:

1. Put the 432 Hz source on the mouth mark. A loudspeaker or an oscillator. Not a person.
2. Record `calibration/take-01.wav` with the same microphone, gain, and distance as the emotions.
3. Run the checker with `--role calibration`.

The tone has the same loudness, true-peak, noise, clipping, DC, and reverb limits as a voice take. Its duration window is 2.0 s to 8.0 s. The session seal wants at least **1** passing calibration take.

## Twelve emotions

Record **5** takes of each emotion. At least **3** of those must PASS. A failed take stays in the folder and in the manifest; it is not deleted to make the folder look clean. If three passes are not in hand, record another take at the same gain. If the room or the gain has to change, start a new session id.

Slugs are the folder names:

| Emotion | Folder | Record | Passing minimum |
|---|---|---|---|
| Neutral | `neutral` | 5 | 3 |
| Calm | `calm` | 5 | 3 |
| Happy | `happy` | 5 | 3 |
| Amused | `amused` | 5 | 3 |
| Excited | `excited` | 5 | 3 |
| Tender | `tender` | 5 | 3 |
| Sad | `sad` | 5 | 3 |
| Disappointed | `disappointed` | 5 | 3 |
| Angry | `angry` | 5 | 3 |
| Fearful | `fearful` | 5 | 3 |
| Surprised | `surprised` | 5 | 3 |
| Disgusted | `disgusted` | 5 | 3 |

Neutral is an emotion. It is not the 432 Hz tone.

Layout, outside git:

```text
originals/<session-id>/calibration/take-01.wav
originals/<session-id>/neutral/take-01.wav
originals/<session-id>/neutral/take-02.wav
...
originals/<session-id>/disgusted/take-05.wav
originals/<session-id>/MANIFEST.json
originals/<session-id>/SESSION.txt
```

`SESSION.txt` is the card (room, microphone, gain position, 30 cm, the sentence, the owner). It may be kept with the manifest. The WAVs may not be committed.

## Originals are read-only, with a sha256 manifest

The checker opens audio read-only. After the takes are accepted:

```bash
SPEC=docs/emotion-v2/capture-spec.toml
DIR=originals/<session-id>

cargo run --manifest-path tools/emotion-v2/refcheck/Cargo.toml -- \
  manifest --dir "$DIR" --spec "$SPEC" --out "$DIR/MANIFEST.json"

find "$DIR" -name '*.wav' -exec chmod a-w {} +

cargo run --manifest-path tools/emotion-v2/refcheck/Cargo.toml -- \
  verify-manifest --manifest "$DIR/MANIFEST.json" --spec "$SPEC" --require-session
```

`manifest --spec` hashes every WAV (sha256 of the file bytes) and stores the checker's verdict. `chmod a-w` clears write bits. `verify-manifest` reads the files again and compares sha256. It requires the write bits to stay clear. With `--require-session` it runs the checker on those same bytes again, so a verdict rewritten in the JSON does not seal, and it requires at least 3 PASS takes for each of the twelve emotions and at least 1 PASS calibration take.

A changed byte fails the hash. A writable WAV fails the seal. Replacing an original means a new session id, not an edit of a sealed file.

The manifest is JSON (hashes, lengths, verdicts). It contains no audio. Commit the manifest only when the hashes are what you intend to publish; the WAVs stay outside git.

## Checker

From the repo root:

```bash
cargo run --manifest-path tools/emotion-v2/refcheck/Cargo.toml -- \
  check --wav path/to/take.wav --spec docs/emotion-v2/capture-spec.toml --role emotion

cargo run --manifest-path tools/emotion-v2/refcheck/Cargo.toml -- \
  check --wav path/to/calibration.wav --spec docs/emotion-v2/capture-spec.toml --role calibration
```

`--role` defaults to `emotion`. Exit 0 is PASS, exit 1 is FAIL, exit 2 is a broken file or a broken spec. Stdout is the JSON report.

Checks, in order: `spec_identity`, `sample_rate`, `bit_depth`, `channels`, `duration`, `integrated_lufs`, `true_peak`, `noise_floor`, `clipping`, `dc_offset`, `reverb`, then `spectral_tilt` (emotion) or `calibration_tone` (calibration). The verdict is PASS only when every check passes.

The tool is its own Cargo workspace. It is not a member of the voice workspace, and it does not load a model.

Tests (synthetic sine and noise only; they do not read a voice):

```bash
cargo test --manifest-path tools/emotion-v2/refcheck/Cargo.toml
```

## Gate

**PASS** when a sealed session verifies: hashes match, WAVs are read-only, one calibration take passes, and each of the twelve emotions has at least three passing takes, all on one card (one room, one microphone, one gain, 30 cm).

**FAIL** when a take misses a row in the spec, or the manifest does not match the bytes.

**BLOCKED** when the owner has not recorded the session. There is no substitute clip and no generated voice.
