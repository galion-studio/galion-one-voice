# F — pre-clone analysis

Perpetual Open MIT License. (c) 2026 Galion Labs.

One stage, before any clone. `galion-preclone` reads a reference and writes `galion-preclone/1`, a single JSON profile. Voice, the emotion hook, the room, and the segment table are sections of that file. They are not separate pipelines.

The stage analyses. It does not clone, does not download a weight, and does not store a recording. Tests synthesize every signal (formant pulse, white and pink noise, an exponential tail, two F0s, a chord). Real references stay on the owner's machine.

## Profile

`galion-preclone --wav REF.wav --out PROFILE.json [--target-f0 HZ] [--moments FRAMES.json]`

`compute` is `cpu`. Floats are decimal-rounded and times come from sample counts, so the same samples and the same moment list produce the same bytes. The path of the wav is not in the file.

| Section | What it is |
|---|---|
| `voice` | Steady baseline of the **kept** target regions: F0, LPC formants, centroid, harmonicity HNR. |
| `emotion` | Not that baseline. `moment.hook` is `galion-feeling`, `tick_ms` is 80. Frames are stored only when the emotion core already produced them. This stage does not step the moment. |
| `environment` | refcheck noise floor, SNR, Schroeder RT60, noise tilt, `music_bed`. |
| `energy_vad` | The existing 20 ms energy gate (floor −60 dBFS, +12 dB, clamp −48 dBFS). Silence only. |
| `segments` | `target`, `other`, `music`, `noise`. `kept` is the target region a later clone may use. |

The target is an F0 in hertz, not an embedding of a person. With no `--target-f0`, the speech F0 that occupies the most frames is the target. A tie keeps the lower frequency. A segment is kept when it is target, at least 0.30 s, and (when the take has a real bed) at least 6 dB above the floor.

SNR uses refcheck's quietest 400 ms window as the bed and the median active window as speech-plus-noise, then `10 log10(10^(diff/10) - 1)`. RT60 is `galion_refcheck::dsp::estimate_rt60` (Schroeder T20, −5 dB to −20 dB, extrapolated to −60 dB). Music is tonal energy that does not sit on one harmonic grid.

## CPU

The measurement is CPU DSP (`rustfft`, hound, refcheck). `--features gpu` only opens the eligibility gate: a GPU counts as eligible when that feature is compiled and `/dev/nvidia*` or `/dev/kfd` is visible. There is no GPU kernel. Eligibility does not change the profile.

## Gates

Synthetic, 16 kHz, in `tools/cloning-v2/preclone/tests/gates.rs`.

| Gate | Tolerance |
|---|---|
| White noise mixes at 10, 20, 30 dB SNR | ±1.5 dB |
| Pink mix at 15 dB SNR | ±2.0 dB |
| White vs pink bed | tilt label `white` / `pink` |
| Exponential tails at 0.30 s and 0.60 s RT60 | ±0.05 s |
| Two-speaker mix, target 140 Hz against 230 Hz | precision ≥ 0.90, recall ≥ 0.80; the other speaker is labelled and is not kept |
| Chord bed | labelled `music` for ≥ 0.50 s, `music_bed` true |
| Clean formant pulse, F0 140 Hz, formants 500 / 1500 / 2500 Hz | F0 within 4 Hz, each formant within 80 Hz |
| Same input | identical profile bytes |

Precision is kept time inside the target interval, divided by kept time. Recall is that overlap divided by the target interval.

`LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not created.
