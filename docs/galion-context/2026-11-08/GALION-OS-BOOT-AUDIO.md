# Galion OS boot process and sounds

Written 2026-11-08, Europe/Zagreb (CET). Documentation only. No sound was played, synthesized, uploaded, or probed.

Two evidence lines are kept apart.

- **Remote, this review.** Files read from git: voice `main` `3e1f5a12`, operating system `e48f3d4f6b` (2026-11-02, 9 files), Open Mind `7ba85b603d` (committer 2026-10-28).
- **Local inventory, same day, separate pass.** A local read of notes and filenames. It did not open the three git tips above as its proof, and this note does not treat copy timestamps as authorship dates. The full local write-up stays off git because it names private paths. The public summary is below.

`/opt/galion` was absent in the local pass. `/opt/galion/SAMANTHA-V2-BRIEFING.md` was not read here either. No SSH.

## Result

No confirmed real boot or startup sound, and no playback call that would play one, was found in the remote trees or in the local filename inventory.

The same conclusion is published, still unmerged, in [pull request 108](https://github.com/galion-studio/galion-one-voice/pull/108) (`c995a898`, `10-galion-os-boot-sounds.md` and `10-galion-os-boot-sounds.evidence.json`). That note counts the tracked wav files in this repository as listening-set speech and presence fixtures. It treats the cascade earcon as a generated demo tone ladder and a warm-up utterance as not a boot sound. It records no format measurement and no listening pass, because no file qualified. This page does not add a sound.

That is not proof the disconnected laptop or the unread VPS disk is empty.

Real boot sounds are an owner priority, next to the voice-quality fix. A generated tone, a speech fixture, a warm-up sentence, or a clone reference must not be renamed into a boot sound.

Because no file qualified as a public boot candidate, this review did not compute hashes, sample rate, channels, bit depth, duration, or peak. Those fields are **unmeasured**, not zero and not passing.

## Specified boot (remote docs)

`galion-operating-system` `docs/voice-profile/voice-first.md` (dated 2026-11-02 CET) plans: Linux, static musl `galion-init` as PID 1, then ALSA, then `galion-voiced`. It says no image is built in that repository.

`docs/voice-profile/PACKAGING.md` repeats that order and describes:

| Name | Specified job | Binary in the OS repo |
|---|---|---|
| `galion-init` | PID 1. Mounts, network, clock, supervises the voice daemon. No shell on the production image. | Absent. |
| `galion-voiced` | 24 kHz, 80 ms frame, bands you / me / feeling, ALSA, Opus. T-Thin first. T-Edge only if a probe passes. | Absent. |
| `galion-probe` | Named beside init for a small release profile. | Absent. |

The image builder is unchosen. The root README asks for gentle sounds for starting, finishing, notices, and questions, silent at night or when muted. It names no file, no rate, and no service.

A local note dated about 23:55 CET on 1 November proposes the same chain and adds an optional Hermes gateway after `galion-voiced`. It is a proposal. It names no startup recording. A second local note the same evening (about 23:00, polished 23:10) repeats the supervising init and leaves the image builder open. Those notes are historical plans, not a current image.

## Implemented boot (remote code)

Open Mind `crates/galion-boot` renders and supervises services. Commands in `main.rs`: `hardware`, `secrets-restore`, `tunnel-env`, `render-units`, `supervise`, `status`, `fire-loops`. A scan of that file found no wav, chime, volume, or playback call.

`deploy/staged/boot.toml` is labelled staged, not installed. Its services are `guardian-tick`, `galion-hardware`, `galion-fire-loops`, and `galion-origin`. “Splash” in the origin description is a page, not an audio file.

`deploy/boot/README.md` says that older unit directory is deprecated, was never installed, and nothing live reads it. `deploy/boot/rendered/galion-one.service` (rendered 2026-10-19 16:50 +02:00) starts `galion-one` on loopback and says it is not installed. No pre-start sound.

So the code that exists is a staged supervisor. It is not the OS Voice PID-1 sequence, and it does not play a chime.

## Local audio that is not a boot sound

The local pass counted **47 audio paths**, including duplicates and a raw companion. They are not 47 distinct sounds. None was designated a boot asset. Private clone references and unclassified home files were not opened. Public-looking items in that count are still the wrong kind of file:

- Cascade demo speech and an earcon tied to that demo, not to boot. Provenance of the earcon as a real recording was not established. It was not decoded.
- Sample-voice and streaming-demo speech, including CC0 test inputs named in an existing sample note. Not startup cues.
- Repository test fixtures for turns and barge-in. Filenames were not treated as a measured format.
- A research note on a persistent worker: a dummy sentence at process start is a **warm-up**, not an authored boot sound.

Tracked listening-set wav files inside voice `main` (under `docs/galion-voice-v2/evals/listening-set/`) are eval speech. No boot path references them. They were not hashed for this note.

Open Mind has `crates/openmind-speech/fixtures/piper_silence.wav` with an MIT SPDX sidecar, and `galion_voice_tts.wav`. Bytes were not fetched. Directory and name make them speech-test fixtures. `galion-boot` does not reference them.

`docs/agent-wave21/W21-02-STATUS.md` describes a generated pentatonic earcon for a demo reply. `docs/V1-DEMO.md` treats that earcon as the wrong answer when the reply should be words. Speech synthesis and that earcon can be real PCM. They are not the owner-required boot asset.

## Readiness, mute, consent

| Topic | Remote evidence | Gap |
|---|---|---|
| ALSA | Named between init and `galion-voiced` in the OS plan. | No code here opens a device at boot. |
| 80 ms clock | Packaging and voice locks. | No reserved phase for a chime. A chime must not block the heartbeat. |
| Night / mute | One sentence in the OS README. | No flag in `galion-boot`. |
| Accessibility | Call-purpose rules in the voice tree. | No boot-sound caption, haptic, or reduced-sound path. |
| Consent | Caller affect defaults to deny (`LOCKS` and `galion-emotion`). | A machine boot sound is unspecified. It is not caller affect. |
| Device log | `galion-boot hardware` can write json when applied. | No log line that a sound played. VPS state was not read. |

## Later work (not done)

1. Recover the real recordings and their licence. Do not substitute a private reference or a synth.
2. After a file is approved and public, record SHA-256 and the measured format. Objective numbers are not a listening score.
3. Play it once, after the output device is ready, from the real init or unit, with a log of device, hash, and exit code. Missing file, missing device, or mute fails honestly and does not loop.
4. Keep native identity. Any gain or rate change is a separate reviewed change.
5. On identified hardware, log time from the boot event to first audible onset, completion, and underruns. Voice-quality acceptance is a different test.
6. Do not accept Piper output, the cascade earcon, or a warm-up sentence as that evidence.

## Unmet

No `galion-init` or `galion-voiced` binary in the OS repository. No image builder chosen. No boot-sound asset. Staged units are not installed. VPS briefing unread. No device log.
