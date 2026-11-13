# Galion OS boot sounds

2026-11-08, Europe/Zagreb (CET, UTC+1). Documentation only.

This note is the boot-sound inventory for Galion Operating System, paired with the OS base note in this folder. It records what the public repositories and a separate local inventory actually contain, which sounds are speech or test fixtures, and which acceptance checks a real boot sound still needs.

No boot recording was added. No sound was synthesized, decoded, played, or uploaded. No service was installed and no image was built.

## Result

No confirmed real Galion OS boot or startup recording exists in the reviewed trees, and no code plays one at boot.

Real boot sounds stay an owner priority beside voice quality. A generated tone, a warm-up sentence, a cascade demo, a listening-set clip, or a voice reference is a different object. None of those is the boot sound.

Objective file measurements for a boot asset are **not measured**. There was no confirmed public boot file eligible to hash or probe. An empty measurement is not a duration of zero and not a quality pass. No one listened to a boot sound for this note, so there is no perceptual result.

## What was read

| Source | Tip used | What it is |
|---|---|---|
| [galion-operating-system](https://github.com/galion-studio/galion-operating-system) | `e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d` (2026-11-02) | Public OS repository. README, MIT licence, and `docs/voice-profile/` (`README.md`, `voice-first.md`, `hermes-later.md`, `PACKAGING.md`, `FB0-SURFACE.md`). Remote read. The tree has no audio file. |
| galion-one-voice | `3e1f5a12c53cd8e0dd47d111676d61d608540559` | Voice repository at the parent of this note. Boot-order mentions, the cascade earcon generator, speech warm-up, demo playback, and tracked `.wav` paths. |
| Separate local inventory | 2026-11-08, read-only | Filename and document review on a machine that is not this checkout. Integrated below. Private paths and attachment identifiers from that inventory are not copied here. |

This checkout also has no `/opt/galion` directory. That is this environment. It does not describe the owner's laptop. The local inventory reported the same absence on its machine.

## Spec text and runtime proof

These sentences are written down. None of them was observed on a booting machine.

| Written claim | Where | Runtime status |
|---|---|---|
| Planned image boot is Linux, then static musl `galion-init` as PID 1, then ALSA, then `galion-voiced`. | OS `docs/voice-profile/voice-first.md` | Spec. The OS repository has no Cargo workspace and no `galion-init` binary. This voice tree has no `galion-init` or `galion-voiced` crate. |
| Image boot order is kernel, then `galion-init`, then ALSA, then `galion-voiced`. Hermes is on another machine. | OS `docs/voice-profile/PACKAGING.md` section 6 | Same. Section 8 of that file says the repository holds the README, the licence, and the packaging note. |
| The production image has a read-only root, no shell, and no display server. A framebuffer, if present, shows one dot. | `voice-first.md`, `FB0-SURFACE.md` | `FB0-SURFACE.md` is marked **SPEC**. Its "present" step is framebuffer drawing. It is not a sound. |
| Gentle sounds for starting, finishing, notices, and questions, silent at night or when muted. | OS root README, and `docs/voice-profile/hermes-later.md` | Prose on the later Hermes GUI profile. No filename, licence, player, or volume. |
| A learning-stack brief (about 2026-11-01 23:55 Europe/Zagreb) and a voice-sector note (2026-11-01) describe kernel or Linux, then `galion-init`, then ALSA, then `galion-voiced`, with Hermes optional and off the audio heartbeat. | Local inventory of those notes | The notes call the sequence proposed. They name no startup-sound file, licence, hook, or completion gate. |

Older local titles that could not be opened, so they add nothing: `A3-eureka-os`, `D19-openmind-hermes-os`, `Q-OS`, and `SAMANTHA-V2-BRIEFING`. The OS repository checkout and `/opt/galion` were absent on that machine.

## Local inventory, sanitized

The local pass counted **47 audio paths**. That count includes duplicate copies and one raw companion. It is not 47 distinct sounds, and it is not 47 boot sounds.

| Group | Role in that inventory |
|---|---|
| `en-cascade-earcon.wav` | The only cue-like name. Cascade-demo metadata points at it. No boot designation and no public provenance in that pass. The file was not decoded. No quality figure was taken. |
| Other cascade demo names (`en-cascade`, `en-cascade-speech`, `en-cascade-fixture`, `en-roundtrip`) | Demo speech or test signals. |
| Sample-voice names (speech and one sung note) | Public hard-test inputs in a sample-voice note. Not boot sounds. Licence lines in that note were not re-checked upstream. |
| Piper stream demo names, including one raw s16le companion | Historical speech output. |
| `p7-turn-mono-8k16.wav` and `p8-barge-stereo-8k16.wav`, plus duplicate copies | Presence and console fixtures. |
| Persona reference names, older reference copies, and clone-smoke outputs | Private references or clone outputs. Waveforms were not opened. |
| Opaque attachment names | Not designated as boot. Not opened, probed, or hashed. |

No local note showed a boot recording, a successful startup playback, device-ready ordering for a sound, once-per-boot behaviour, a mixer setting, mute handling, or a captured boot recording.

## Voice repository: what the audio actually is

Tracked audio on `3e1f5a12` is 84 `.wav` files. None of the filenames contain boot, chime, startup, earcon, or splash. None of those 84 files was decoded for this note.

| Path class | Files | What the surrounding docs say they are |
|---|---:|---|
| `docs/galion-voice-v2/evals/listening-set/emotion-sweep/` | 46 | Emotion-sweep listening renders for the Samantha and Jarvis eval. Speech. |
| `docs/galion-voice-v2/evals/listening-set/piper-premium/` | 36 | Premium-voice listening renders from the V2 sprint. Speech. |
| `specs/fixtures/p7-turn-mono-8k16.wav`, `specs/fixtures/p8-barge-stereo-8k16.wav` | 2 | You-band fixtures for `galion-presence` and the console mic path. The presence README describes the loader (integer PCM, mono or stereo, 8–192 kHz). The `8k16` token is in the filename. This note did not probe the bytes, so that token is not a measured format. |

`scripts/eval/sample-voices/SAMPLE-VOICES.md` names four further clips (`pdx-voice.wav`, `armstrong-en.wav`, `voicezero-amy_koenig.wav`, `voicezero-alan_davis_drake.wav`) and says they are not committed. This checkout has the note and not the four files. The note dates the fetch 2026-11-03 and points at public URLs. Those clips are hard-test inputs. They are not boot sounds. This review did not fetch them and did not re-verify the upstream licence.

The OS repository at the tip above contains zero audio files.

### Cascade earcon (synthesized diagnostic)

`crates/galion-cascade/src/en_lane.rs` function `earcon` builds a deterministic tone ladder in memory: one 80 ms frame per character at the lane rate (`LANE_RATE_HZ`, 24 kHz), alphanumeric characters mapped onto a C-major pentatonic, amplitude coefficient `0.3` in the generator, silent lead-in and lead-out frames. `wav_bytes` wraps that PCM as mono 16-bit RIFF/WAVE. The example writes `en-cascade-earcon.wav` under the demo output directory. That file is not in git.

The module comment calls this render a diagnostic. The spoken reply is a separate buffer. `scripts/run-en-cascade-demo.sh` documents the earcon as tones and not the reply. `--play` sends `en-cascade.wav` (the speech file) to `paplay`, `aplay`, or `afplay` when a person passes the flag. It does not play the earcon, and it does not run at operating-system boot.

Those generator constants are source text. They are not a probe of a file on disk, and they are not a listening result.

### Speech warm-up (not a boot sound)

`crates/galion-ml-voice/src/speech/mod.rs` `SpeechStack::prewarm` speaks three throwaway lines (`PREWARM_LINES`: "Hello.", "Okay, I hear you.", "That sounds good, tell me more about it.") and keeps no PCM. The comment says this discards a short synthesis so the first caller turn is not the cold start. Research notes that mention a dummy sentence at process start are describing this warm-up. It is not an authored boot recording and must not be labelled as one.

### Demo speaker path (not boot wiring)

`crates/galion-live-demo/src/play.rs` `PlaySink` starts a system player when the live-demo me-band becomes voiced, and stops it on barge-in. The spawn is in `on_frame`, after `Heartbeat::step`. `GALION_PLAY=0` disables it. That is reply playback for a demo process. There is no PID 1, no image boot event, and no once-per-boot asset.

`deploy/systemd/galion-clone-cbx.service` is the clone worker unit. It does not name a boot sound.

## Gaps

- No canonical boot-sound path, event name, licence, or checksum.
- No `galion-init` implementation, so nothing can play after ALSA becomes ready.
- No image, so the written boot order has not been run.
- No night-mute or hardware-mute behaviour for a boot sound. The README sentence is unimplemented.
- No measured hash, container, sample rate, channel count, bit depth, duration, peak, clipping count, or silence for a boot asset.
- No captured boot: no onset time from the boot event to first audio, no underrun count, no heard comparison of file versus speaker.
- Voice-quality numbers that already exist (TTFA, RTF, SQUIM, listening sets) measure speech. They do not grade a boot sound, and this note does not repeat them.

A disconnected laptop was outside both reviews. Absence here does not prove a recording is missing from a machine that was not searched.

## Changes that are still required

These are the changes a later implementation needs. They are not done in this note.

1. Recover the owner's real boot recording, or commission one, with a licence that can ship in an MIT image. Record the canonical event (the moment the sound means: power-on, ALSA ready, or voice ready). Leave clone references, listening-set renders, sample-voice clips, cascade output, and warm-up speech out of that slot.
2. When a cleared file exists, store it as its own image artifact: original bytes unchanged, a `NOTICE` with source and licence, and a SHA-256. Publish format, rate, channels, bit depth, and duration from a probe of that file. Keep peak, full-scale sample count, and silence ratio beside the probe, with the silence threshold written down. Those numbers still do not replace listening.
3. In `galion-init`, after the output device is usable and before `galion-voiced` depends on that device: play the file once; stay off `Heartbeat::step`; on a missing file, a missing device, or mute, log the failure and continue with no retry loop. Do not call `earcon` or `prewarm` to fill the gap.
4. Keep any later resample or gain change as its own reviewed change. The shipped original stays the reference.
5. On the chosen hardware and output route, capture one boot. Measure event to first audible onset, completion, underruns, and errors. Listen to the original and to the capture. Track that result separately from voice quality.

## Acceptance tests

| Id | Check | Pass condition | Status now |
|---|---|---|---|
| A1 | Asset identity | One declared path. SHA-256 matches the notice. Container, rate, channels, bit depth, and duration match the notice. Missing file fails the test. | Not run. No asset. |
| A2 | Exclusion | The path is outside listening-set renders, `specs/fixtures`, sample-voice clips, persona references, cascade demo output, and warm-up text. | Not run. No asset. |
| A3 | Once per boot | A boot log shows one play, after the output device is ready, and no second play on the same boot. | Not run. `galion-init` is absent. |
| A4 | Honest failure | Missing file, missing device, and mute each produce one error record and zero plays. They do not restart a loop. | Not run. |
| A5 | Heartbeat | The play is outside `Heartbeat::step`. A frame-budget sample during playback stays within the existing 80 ms budget. | Not run. |
| A6 | Heard boot | A named person, on a named route, compares the original file with a capture of that boot and records the result. | Not run. No listening was done for this note. |
| A7 | Voice quality stays separate | Speech TTFA, RTF, and listening scores are reported on their own lines. A boot-sound pass does not move them, and a voice-quality pass does not create a boot sound. | Open. Existing speech numbers were not re-measured here. |

A1 through A5 can be automated once the file and `galion-init` exist. A6 cannot be replaced by A1. Metadata and a perceptual test answer different questions. The packaging boot paragraph and a log from a real boot answer different questions too.

## Evidence file

Cleared counts and the explicit empty measurement live in `10-galion-os-boot-sounds.evidence.json` beside this note. That file has no audio bytes and no private path.
