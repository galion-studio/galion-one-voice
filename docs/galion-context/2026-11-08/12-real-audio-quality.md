# 12 — Real audio quality

Pair 01, emotion. Date 2026-11-08. Scope is voice audio quality only.

This file is a reading of the tree and of artifact headers. It does not
re-run Piper, Nano, the heartbeat suite, or `emotion_match`. It does not
listen to waveforms, decode private recordings, train, download, or
synthesize. It is not a product pass.

Detailed boot sequencing belongs to boot10 in this same
`docs/galion-context/2026-11-08/` pack. This note only states the audio
requirement that sits beside that sequence.

## Priority

**VOICE AUDIO QUALITY FIX.** The owner wants real sounds. A constant
sample, a sine, a noise bed, a formant fallback, or a post-hoc pitch
slide is not the voice. Real boot sounds sit in the same priority,
beside the voice, as designed or recorded assets. Boot10 owns how boot
runs. This file owns whether the sound that comes out is real speech
PCM and a real boot asset.

## What counts as the sound

| Source | What the bytes are | Product voice? |
|---|---|---|
| Piper A21, `SpeechBackend::A21Weights`, child engine writing raw s16le after a real `piper` utterance | Inference PCM from the pinned VITS pack | Yes, this is the live voice when the clone slot is absent |
| Chatterbox Nano `generate` / stream, stderr engine `chatterbox-nano` or `nano`, weights pinned and present | Inference PCM | Only after a logged fresh run. This pack has no such run |
| `GALION_CLONE_STUB_PCM=1`, engine `protocol-stub` | Protocol PCM. `protocol_chunks` writes one s16 length sample, then 200 copies of a constant `tone = 1000 + (len(text) % 50)` | No. Pipe clock only |
| `GALION_CLONE_STUB_FRAMES` | Same class of protocol frames | No |
| `run_unwired` / engine `unwired` | Zero samples | No |
| Flite via `dlopen` (`SpeechBackend::SystemFlite`) | Word synthesis at 16 kHz, resampled to 24 kHz. Label contains `NOT A21` | Interim words, not the neural voice |
| `RikaReplayBackend::synth` | 200 Hz sine at amplitude 0.1 for speech; white noise at RMS 0.001 / 0.003 for room and breath. Comment: "No WAV, no weights." | Harness fixture |
| Emotion V2 dataset `synthesize_stub` / `voice_id: stub-sine` | Tone and noise. Handoff: 192 clips from the stub generator | Not Samantha training data |
| Heartbeat `you_pcm` of zeros, or mixer tests filled with a constant | Control input to `step` | Not a performance |
| Boot asset | No boot wav, chime, or startup clip is in this tree | Missing. Required beside the voice |

`product_claim` in `docs/galion-voice-v2/FLASH-EVAL.md` is limited to
real Nano PCM on a stream path, or real Piper PCM from
`GALION_SPEECH=a21`. A protocol-stub clock stays
`product_claim: false`. `docs/galion-voice-v2/CLONE-WORKER.md` says the
stub's `time.sleep(0.05)` is the protocol gap, not a measured
time-to-first-audio.

`PiperProsody::with_emotion` returns `self` for every label, including
non-neutral (`crates/galion-ml-voice/src/speech/piper_worker.rs`). The
comment states that a non-neutral emotion is not applied. Emotion names
on that hook do not change the waveform.

Default Piper scales are deterministic: `noise_scale=0` and `noise_w=0`
unless `GALION_A21_NOISE=natural`, which uses `0.667` and `0.8`
(`PiperProsody::from_env`). The live default is the flat setting.

`EmotiveShaper` (`crates/galion-ml-voice/src/emotive.rs`) then pitch-shifts
by resampling (`ratio = 2^(st/12)`). The file says formants move with
the pitch. That is a DSP approximation of expression, after the network
has already spoken. `apply_frame_prosody` only scales energy and
prepends zeros. Pitch and rate are not rewritten there.

## Figures in this pack, traced, not remeasured

The owner brief for this pack states a fresh Piper reading of **cold
480 ms** and **warm 29 ms**. This file did not run Piper. No committed
bench row pairs those two numbers.

Nearest committed rows, each left on its own host and phrase:

| Record | Cold | Warm | Where |
|---|---|---|---|
| Sprint Gate 1, real CPU Piper, n=24, 8 vCPU Xeon, `en_US-ljspeech-medium` | spawn to first PCM p50 334.3 ms, p95 440.3 ms | 1-word "Yes." p50 25.9 / p95 35.2; 4-word 42.3 / 60.2 | `docs/galion-voice-v2/sprint-v2/SPRINT-REPORT.md` section 3 |
| Latency-240 contributions, legacy vs `galion-piper-rt` | legacy first live clause 392 ms; rt cold 1405–1440 ms, paid at start behind pre-warm | legacy 1-word 19.0 / 27.4; rt 1-word 14.4 / 16.1 | `docs/galion-voice-v2/sprint-v2/latency-240/CONTRIBUTIONS.md` |
| Same note, "−29 ms" | — | A p50 *delta estimate* (71.3 to about 42), Piper-only replay, not a warm time of 29 ms | `docs/galion-voice-v2/sprint-v2/latency-240/01-streaming-tts.md` |
| Premium-voice bench, legacy CLI, phrase "Sure, I can help." | respawn median 300 ms (libritts_r), 330 ms (vctk), 396 ms (ljspeech) | chosen Samantha pack p50 32 / p95 36; chosen Jarvis pack p50 36 / p95 53 | `docs/galion-voice-v2/evals/premium-voices/PREMIUM-VOICES.md` |
| Stage budget | — | Target 30 ms p50, from a measured 71.3 | `docs/galion-voice-v2/sprint-v2/latency-240/BUDGET.md` |

Do not replace the brief's 480 / 29 with any row above. Do not treat the
budget target, the "−29 ms" delta, or a respawn median as that fresh pair.

**Heartbeat.** The brief states that 13 heartbeat tests pass in a
separate run. This file did not execute `cargo test`. No in-tree log
names those 13. Reading the sources shows 41 `#[test]` functions under
`crates/galion-heartbeat`:

| File | Tests |
|---|---|
| `tests/heartbeat.rs` | 16 |
| `tests/symphony_wire.rs` | 11 |
| `src/mixer.rs` | 9 |
| `tests/zero_alloc.rs` | 2 |
| `tests/presence_p6.rs` | 2 |
| `tests/frame_budget.rs` | 1 |

The allocation checks are `tests/zero_alloc.rs`
(`step_allocates_nothing_with_rika_on`,
`feeling_step_allocates_nothing`) and
`tests/symphony_wire.rs::steps_allocate_nothing`. They arm a counting
allocator around `Heartbeat::step`. The inputs are scripted frames,
zeros, or a stub RMS/ZCR path. `tests/frame_budget.rs` says so in its
header: stub-path timing, not a model, codec, clone, or host-CPU result.

A green allocation run proves `step` did not allocate. It does not prove
the voice, the prosody, the boot sound, or the product.

**Nano.** The brief states there is no fresh Nano run. This file did not
start one. Historical sprint Gate 2 (`SPRINT-REPORT.md` section 4) records
CPU Chatterbox timings on the order of 2.0–2.8 s per about 4.5 s of audio.
`FLASH-EVAL.md` still has no product `ttfa_ms` until a model run is
written down. Those older seconds are not a fresh run for this pack.

**Emotion persona gate.** Threshold remains `emotion_match` ≥ 0.83
(`docs/emotion-v2/EMOTION-V2-FOUNDATION-SPEC.md` section 7,
`docs/emotion-v2/HANDOFF.md`, `tools/emotion-v2/thresholds.toml` via
`docs/emotion-v2/CI.md`). Historical persona numbers in the foundation
spec, laptop, 7 November 2026, single-reference SUPERB 4-class head plus
embedding cosine at 0.70 / 0.30:

| Voice | Historical `emotion_match` | Against 0.83 |
|---|---|---|
| Samantha raw | 0.491 | under |
| Samantha after room-mic normalization | 0.461 | under |
| Jarvis | 0.807 | under |

The handoff summarizes Samantha as about 0.46–0.81. The foundation spec
says the 4-class mismatch caps the float near 0.80: sad renders fall to
about 0.00–0.04 and lean angry 0.5–0.7, against a mixed reference
(neutral 0.345, happy 0.203, angry 0.136, sad 0.317). Four classes cannot
see eight of the twelve sectors. This file did not rescore.

The universal scorer JSON is a different formula (emotion2vec+,
EfficientAT, CLAP). It is not the persona gate. Scores below are
rounded to three decimals:

| File | Row | `emotion_match` |
|---|---|---|
| `tools/cloning-v2/scorers/universal/results/samantha-universal/RESULTS.json` | `BASE-self-ceiling` | 0.904 (embedding similarity 1.0, a ceiling) |
| same | set `merge-v2` n=12 | 0.774 |
| same | set `global-fix-coherent` n=12 | 0.751 |
| `tools/cloning-v2/scorers/universal/results/jarvis-universal/RESULTS.json` | `BASE-self-ceiling` | 0.948 |
| same | set `merge-v2` n=12 | 0.880 |
| same | set `global-fix-coherent` n=12 | 0.890 |

Jarvis clearing 0.83 on that JSON does not clear the persona gate. The
Samantha sets on that JSON are still under 0.83. Render paths inside
those JSON files point at `/workspace/swarm/...` wav directories. Those
wavs were not opened.

`docs/emotion-v2/NEBULA-SCORING.md` states the offline nebula field does
not affect `emotion_match` and does not lower 0.83.

## Perceptual fidelity already on paper

`docs/galion-voice-v2/sprint-v2/results/emotion-sweep/SWEEP.md` is an
acoustic sweep, not a listening test. Reply line is fixed. Metrics are
means over 8 Piper renders. The user side is labeled synthetic. V1
baselines use deterministic noise 0.

Resting references in that file (length scale, noise, pitch plan, F0):

| Persona | Pack | length_scale | noise_scale | noise_w | pitch st | F0 Hz | File duration from header |
|---|---|---|---|---|---|---|---|
| Samantha rest | libritts_r speaker 126 | 1.000 | 0.679 | 0.800 | +0.40 | 180.6 | `samantha-reference-rest.wav` is in the set below |
| Jarvis rest | vctk speaker 95 | 1.053 | 0.498 | 0.593 | −0.30 | 111.0 | `jarvis-reference-rest.wav` |

Examples of how small the acoustic move is (`SWEEP.md` persona table,
ΔF0 versus that persona's rest):

| Case | e | pitch st (plan) | F0 Hz | ΔF0 st |
|---|---|---|---|---|
| Samantha, user happy, rest history | 0.00 | +0.78 | 185.1 | +0.42 |
| Samantha, user sad, rest history | 0.57 | −0.19 | 172.1 | −0.83 |
| Samantha, user angry, rest history | 0.49 | +0.02 | 174.0 | −0.65 |

Angry is answered in the quieter direction. The F0 move is a fraction of
a semitone to under one semitone. Nobody has signed a listening result
that this is audible as that emotion. Sprint Gate 3
(`SPRINT-REPORT.md` section 5) already called contrast modest and said
the whisper tag is only partly honoured (voiced ratio 0.52).

`PREMIUM-VOICES.md` chose Samantha `en_US-libritts_r-medium` speaker 288
and Jarvis `en_GB-vctk-medium` p226 from SQUIM and F0 metadata. The same
file says SQUIM estimates signal quality, not naturalness, that UTMOS and
DNSMOS were unavailable, and that nobody had listened yet. The choice is
still a proposal: `jarvis.toml` accent lines were not edited.

## Artifact metadata (headers only)

Committed listening-set WAVs under
`docs/galion-voice-v2/evals/listening-set/`. Headers were read with the
standard library `wave` module (channels, sample width, rate, frame
count). Samples were not read and nothing was played.

| Group | Files | Header | Duration min / median / max | Bytes |
|---|---|---|---|---|
| `piper-premium/samples` | 36 | mono s16, 22050 Hz, uncompressed | 3.349 / 4.940 / 6.669 s | 7,606,992 |
| `emotion-sweep` | 46 | mono s16, 22050 Hz, uncompressed | 2.009 / 3.797 / 4.684 s | 7,814,076 |

Named files:

| File | Bytes | Frames | Header seconds |
|---|---|---|---|
| `piper-premium/samples/samantha-en_US-libritts_r-medium-288.wav` | 147,732 | 73,844 | 3.349 |
| `piper-premium/samples/jarvis-en_GB-vctk-medium-p226.wav` | 240,404 | 120,180 | 5.450 |
| `emotion-sweep/samantha-v1-baseline.wav` | 125,996 | 62,976 | 2.856 |
| `emotion-sweep/jarvis-v1-baseline.wav` | 163,372 | 81,664 | 3.704 |

`SWEEP.md` lists Samantha V1 speech as 2.846 s and Jarvis V1 speech as
3.698 s. Those are speech-length figures in that note. The header
seconds above are the whole file. They were not recomputed from samples.

No boot-sound file is in the listening set or elsewhere in the tree
(search for a boot chime or startup clip in crates and docs returned
none).

## Defects and stream risks

Each item is a code or doc fact. None of them was reproduced by a new run.

1. **Emotion label does not move Piper.** `with_emotion` clones the
   current scales for every string. A caller that only uses this hook
   ships the same waveform for neutral and for angry.
2. **Default noise is zero.** `from_env` picks the deterministic pair
   unless `GALION_A21_NOISE=natural`. Flat noise is a drier, more
   synthetic reading of the same network. The premium and sweep notes
   that sound more alive use non-zero noise, which is not the default.
3. **Prosody DSP is an approximation.** `EmotiveShaper` resamples pitch
   and moves formants with it. `apply_frame_prosody` is gain plus leading
   zeros. Neither is a new performance by the speaker.
4. **Protocol stub can be mistaken for speech.** `scripts/clone/nano_stream.py`
   `protocol_chunks` and `scripts/clone/chatterbox_worker.py` `run_stub`
   emit a constant s16 value. The first write can be a single sample so a
   clock can stamp TTFA. `FLASH-EVAL.md` calls that STUB-PASS, never
   HARD-PASS.
5. **Flite is a second voice.** `crates/galion-ml-voice/src/speech/flite.rs`
   is classic Flite, 16 kHz, cubic-resampled to 24 kHz, used when the
   neural child is absent. Real words, wrong identity.
6. **Harness speech is a sine.** `presence.rs` `synth` generates 200 Hz
   plus noise. `step_live` can take real PCM, and it zero-pads a short
   buffer and truncates a long one to `FRAME_SAMPLES`. A short you-band
   is also a pause (`galion-heartbeat` crate docs: PCM that is not
   exactly one frame is `paused`).
7. **Odd PCM byte.** Piper streaming keeps a trailing odd byte inside the
   utterance buffer (`synthesize_stream` drains `len & !1` and leaves the
   leftover in `pcm_buf`). `read_utterance` then `pcm_buf.clear()` at the
   boundary, so a final unpaired byte is dropped rather than carried into
   the next utterance. The Nano ring (`symphony.rs` `push_s16le`) does
   carry one odd byte in the caller's `carry`. Two policies, one pipe
   family.
8. **Ring overflow is discarded.** `NanoRing::push` counts samples that
   do not fit in `dropped` and does not retry them. A full ring is a hole
   in the waveform, not a catch-up. `drain_frame` returns a frame only
   when 1920 samples are queued; a short remainder stays queued and the
   tick pauses.
9. **Resample is Catmull-Rom, mono, deterministic** (`speech/mod.rs`
   `resample`). The comment calls it adequate for 16 kHz to 24 kHz. It is
   not a band-limited converter. Flite and any rate change go through it.
   Piper premium files on disk are 22050 Hz; the heartbeat clock is
   24 kHz (`FRAME` 1920 samples, 80 ms). A path that plays those wavs
   unchanged is off-rate. A path that resamples them uses the cubic
   converter above.
10. **Piper still speaks a whole sentence before the first flush.**
    `01-streaming-tts.md`: VITS emits the sentence at once, so streaming
    only starts playback at the first `read`. Quality of the attack is
    the whole clause arriving together.
11. **Dataset and capture stubs.** Emotion V2 handoff: the dataset
    pipeline's reported run used a stub tone generator, 192 clips, not
    real Samantha data. Capture PR text commits no audio. Stub tests are
    not the 0.83 gate.
12. **Premium choice is unheard, and the accent lines disagree.**
    `PREMIUM-VOICES.md` caveats: no listening; CC BY attribution still
    required before ship; `jarvis.toml` has
    `accent_claim = "intended-not-checkpoint"` and `JARVIS.md` says the
    checkpoint has no British accent, while VCTK p226 is an English
    Surrey reader. Those lines were not edited. `galion-piper-rt` had no
    `speaker_id` in that note, so the chosen multi-speaker packs may not
    play on the rt runtime.
13. **No boot asset.** A boot that emits the protocol tone, the 200 Hz
    sine, or silence is the defect this priority is for. The sequence
    itself is boot10's document.

## Acceptance

These checks are the bar. This pass performed none of them. Heartbeat
allocation, a stub clock, SQUIM, and a self-ceiling score do not close
any row.

| Check | Pass condition | Evidence today |
|---|---|---|
| Engine identity | Live and boot playback report real Piper PCM or a logged Nano `generate`/`stream`. Engine is not `protocol-stub`, `unwired`, or Flite-as-product | Code paths exist for all four. No fresh Nano log. No boot asset |
| Listening | Blind MOS or paired comparison on the committed listening set and on sad / angry / happy versus rest, same sentence, named listeners, date, host | `PREMIUM-VOICES.md`: nobody has listened. `SWEEP.md` is F0/RMS only |
| Prosody is perceptible | Listeners name the intended emotion above chance, and the acoustic move is reported with the same render. `with_emotion` alone cannot be the implementation under test | Sweep ΔF0 is under about 1 st in the rows quoted. Hook is a no-op |
| Persona gate | `emotion_match` ≥ 0.83 on the persona scorer (0.70 category + 0.30 embedding, per-sector once V2 references exist). Universal JSON and nebula distance reported beside it, not instead of it | Historical Samantha 0.491 / 0.461, Jarvis 0.807. Not remeasured |
| Piper timing | Cold and warm on a named host, runtime, pack, phrase, and n. The brief's 480 ms / 29 ms either gains a bench file or stays labeled as the brief | Historical rows differ and are listed above |
| Heartbeat | Allocation tests stay green in their own run | Brief: 13 tests passed separately. Source count is 41. Not a voice pass |
| Nano | A fresh CPU log with engine `chatterbox-nano` or `nano`, first-chunk PCM, host, and `product_claim` policy applied | Brief: no fresh run. Sprint Gate 2 is historical |
| Waveform integrity | Odd-byte policy is one policy; ring `dropped` stays zero on a normal utterance; 22050 Hz assets are not played as 24 kHz; joins across `EmotiveShaper` chunks stay click-free in a recorded capture | Risks are in the code review above. No new capture |
| Boot sound | A real asset plays at boot, identified by boot10's sequence, and is not the stub tone or the harness sine | Asset absent in this tree |
| Naturalness models | If SQUIM is cited, it is labeled signal quality. UTMOS or DNSMOS, if used, is a new run with the tool version | Premium note: UTMOS and DNSMOS were unavailable. Not run here |

## Order of work

1. Keep the live lane on real Piper PCM. Refuse protocol-stub, unwired,
   Flite, and the harness sine as the product sound. Add a real boot
   asset beside that voice and let boot10's note own the sequence.
2. Make emotion change the sound people hear: non-zero noise when the
   owner wants a natural reading, and a path that actually applies the
   current blend. `with_emotion` as written does not. Treat
   `EmotiveShaper` as a temporary pitch tool and say so in any demo.
3. Run the listening and the persona gate on real renders. Publish the
   brief's Piper 480 ms / 29 ms as a bench with host and phrase, or keep
   calling it the brief. Do not promote heartbeat allocation, SQUIM, or
   the universal self-ceiling into that result.
4. Repair the stream edges (odd byte, ring drop, 22050 versus 24 kHz,
   sentence-sized first audio) with captures, after the sound is real.
5. A fresh Nano run is a separate clock. Until it exists, Nano is not
   the quality claim.

## Out of scope

No inference, download, training, synthesis, deploy, merge, or edit to
`LOCKS.toml`, weights, or private audio. The 7 November handoff's merge
wording is older than current `main` (those emotion branches now have
merge commits). That does not move the historical persona scores, and it
does not authorize a live voice switch.
