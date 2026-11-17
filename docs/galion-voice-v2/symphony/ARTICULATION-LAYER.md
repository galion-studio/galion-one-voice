# Symphony articulation layer (throat, mouth, nose)

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. 2026-11-17. Owner request t1370u (with t1371u–t1376u steering).

The articulation layer is a new application layer under the Symphony stack. It recognises how a voice is produced as sound passes through the **throat** (glottal source), the **mouth** (vocal-tract shape and articulators) and the **nose** (nasal resonance). Cloning uses it twice:

1. **At analysis time (stage 0):** it fills the `articulation` section of the voice card, so cloning knows how this speaker articulates.
2. **At clone time:** the same numbers become a target and a score (TractPrint), so a clone can be checked, and later steered, to articulate like the reference.

It is universal. The voice is a pluggable input (one reference WAV, one clone WAV). No voice name or voice-specific constant appears in the code. Samantha is just the first test voice.

## Where it sits

```
reference.wav ──► stage 0 voice card ──► clone conditioning ──► clone.wav ──► gate
                    │  refcheck, hygiene,          (cbx / Moshi)              │ WER, ECAPA, emotion2vec+
                    │  emotion2vec+, persona                                   │ + TractPrint (this layer)
                    └─ articulation (this layer) ──────── target ─────────────┘
```

* **Stage 0 voice card** (branch `swarm11/stage0-voice-card`, parallel agent). This layer adds one `articulation` section and does not duplicate anything that agent already measures (refcheck loudness/RT60/noise, hygiene, emotion2vec+). The existing prose and prosody articulation profile (`tools/cloning-v2/emotion-blend/A-global/build_profile.py`: VSA, fricative centroids, VOT, pause rate) stays where it is. This layer adds the parts that profile lacks: articulator positions, glottal source, nasality, mouth shapes, all on the 80 ms tick.
* **Clone conditioning or post-filter.** Not wired yet. The worst TractPrint features are emitted as a list (`worst`), which is the hook for picking conditioning clips (best-of-N) or a later post-filter.
* **Gate scorer.** TractPrint is reported next to WER, ECAPA and emotion2vec+. It is **not** a blocking gate yet: there is no agreed bar (see "Honest results" below).

## Bricks (reused, as-is)

| Part | Brick | Licence | Language, where it runs | Works |
|---|---|---|---|---|
| Pitch, voicing, HNR, Burg formants F1–F4, bandwidths | [praatfan_rust](https://github.com/ucpresearch/praatfan-core-clean) v0.1.16 (clean-room Praat) | MIT | Rust, in process | yes (built and run on GALION-STUDIO WSL) |
| Articulator traces: tongue dorsum/body/tip, lower incisor (jaw), upper and lower lip, 12 EMA channels at 50 Hz, plus source pitch, loudness, periodicity and a 64-d speaker vector | [SPARC](https://github.com/Berkeley-Speech-Group/Speech-Articulatory-Coding) 0.1.0, weights `cheoljun95/Speech-Articulatory-Coding` | MIT (code, PyPI); MIT (HF weights card) | Python/PyTorch, out of process (`workers/sparc_worker.py`) | see PR report |
| Glottal source: GCI variability, open quotient (QOQ), NAQ, H1–H2, HRF (IAIF + SE-VQ) and phonation (jitter, shimmer, APQ, PPQ) | [DisVoice](https://github.com/jcvasquezc/DisVoice) 0.1.10 | MIT | Python, out of process (`workers/disvoice_worker.py`) | see PR report |
| Nasal, labial, dental, velar, stop, vocalic, … posteriors per 10 ms | [Phonet](https://github.com/jcvasquezc/phonet) 0.3.7 | MIT | Python/TensorFlow, out of process (same worker) | see PR report |
| Mouth shapes A–H, X (language-independent `phonetic` recogniser) | [Rhubarb Lip Sync](https://github.com/DanielSWolf/rhubarb-lip-sync) 1.13.0 | MIT | C++ CLI, out of process | see PR report |

Python is used only where no working Rust piece exists (searched crates.io and GitHub on 2026-11-17: no Rust acoustic-to-articulatory inversion, IAIF/GCI, jitter/shimmer, phonological-posterior or lip-sync crate). Nothing was ported or rewritten (owner t1375u). The ML bricks run in their own process, so the locked main workspace stays free of `ort` and `candle` (LOCKS `formats.forbidden_runtimes`). The Rust tool has its own cargo workspace, like `tools/emotion-v2/refcheck`.

### Found, not used (yet)

| Project | Licence | Why not now |
|---|---|---|
| [vox_box](https://github.com/andrewcsmith/vox_box.rs) (Rust LPC, formants, pitch) | MIT | last release 2017, self-described incomplete; praatfan_rust covers it and is validated against Praat |
| [Phonia](https://github.com/mkpoli/phonia) (Rust phonetics toolkit) | see repo | young; overlaps praatfan_rust |
| [Praat](https://github.com/praat/praat) / [parselmouth](https://github.com/YannickJadoul/Parselmouth) | GPL-3.0 | praatfan_rust gives the same numbers under MIT, in Rust |
| [COVAREP](https://github.com/covarep/covarep) IAIF, [GFM-IAIF](https://github.com/operrotin/GFM-IAIF) | LGPL-3.0 | MATLAB/Python; DisVoice has the same IAIF under MIT |
| [NasalityAutomeasure](https://github.com/stylerw/styler_praat_scripts) (A1–P0), [nasals](https://github.com/wendyelviragarcia/nasals) | Praat scripts | need TextGrid vowel labels; A1–P0 is not comparable across speakers (its README); Phonet gives a label-free nasal posterior |
| [STArK](https://github.com/Lab-MSP/STArK) (text → SPARC articulation) | MIT + SPARC terms | text-to-articulation, useful later for clone-time targets |
| [VocalTractLab](https://github.com/TUD-STKS/VocalTractLab-dev), [Pink Trombone](https://github.com/zakaton/Pink-Trombone) | GPL-3.0 | articulatory synthesisers for mimicking; SPARC's own decoder already resynthesises from articulation |
| [DDSP](https://github.com/magenta/ddsp) | Apache-2.0 | source-filter synthesis; no voice-clone recipe that beats SPARC decode here |
| [NVIDIA Audio2Face-3D](https://github.com/NVIDIA/Audio2Face-3D) | SDK MIT; models NVIDIA Open Model Licence | facial expression from audio; needs TensorRT; a later "face" band, not this layer |
| [HeadAudio](https://github.com/met4citizen/HeadAudio), [wLipSync](https://github.com/mrxz/wLipSync) | MIT | browser-only visemes; Rhubarb is the CLI equivalent |

Licences are on hold for internal development (owner t1360u). Everything chosen above is MIT anyway.

## Data contract (80 ms tick)

One JSON object, the voice card's `articulation` section (`schema: 1`):

```json
{
  "schema": 1, "tick_ms": 80, "duration_s": 17.41, "n_ticks": 218,
  "tracks": { "f1_hz": { "section": "mouth", "unit": "Hz", "source": "praatfan formant burg",
                         "ticks": [null, 431.2, 455.0, …] }, … },
  "visemes": ["X", "B", "C", …],
  "statics": { "glottal.global avg avg QOQ": ["throat", 0.41], … },
  "summary": { "f1_hz": { "section": "mouth", "n": 134, "mean": …, "q": [q05, q10, q25, q40, q50, q60, q75, q90, q95] }, … },
  "bricks": [ { "name": "praatfan_rust", "licence": "MIT", "lang": "Rust", "process": "in process", … }, … ]
}
```

* One value per 80 ms tick (LOCKS `clock.frame_ms = 80`), the median of the brick's frames inside that tick. `null` means not measured (silent, unvoiced, or that brick did not run).
* Formants, bandwidths, F0 and HNR are kept only in ticks that are at least half voiced.
* Sections: **throat** = `f0_hz`, `f0_st`, `voicing`, `hnr_db`, `sparc_periodicity`, `sparc_loudness`, `phon_voice`, DisVoice glottal and phonation statics. **mouth** = `f1..f4_hz`, `b1..b3_hz`, `vtl_cm`, `ema_*`, `lip_aperture`, Phonet place and manner posteriors, Rhubarb mouth shapes. **nose** = `phon_nasal`.
* Settings are chosen from the voice itself (Praat conventions): two-pass pitch range (0.75·q25, 1.5·q75), formant ceiling 5500 Hz if median F0 ≥ 165 Hz else 5000 Hz.

## Our own new idea: TractPrint

Reused bricks give separate numbers; none of them says "does this clone articulate like this reference?". ECAPA checks identity, WER checks words, emotion2vec+ checks feeling. Nothing checks the throat, mouth and nose together.

TractPrint (Rust, `src/score.rs`):

* Every track becomes a distribution over ticks (nine quantiles). Two voices are compared per track with a quantile-function distance (Wasserstein-1 over 5–95 %), scaled by the reference's own spread (IQR): `sim = 1 / (1 + W1 / IQR_ref)`.
* Mouth shapes use 1 − total-variation distance of the A–H/X histograms. Static glottal numbers use `1 − |a−b| / (|a|+|b|)`.
* A section is the mean of its features. The total is the **geometric mean of the sections**, so a clone that gets the mouth right but loses the nasal resonance cannot hide behind the other two.
* It needs no time alignment and no same text, so a clone saying new words can still be checked against the reference.
* It reports the five worst features, so the next step (conditioning choice or post-filter) knows what to fix.
* SPARC's EMA lives in a speaker-agnostic template space, so the mouth section measures *how* the speaker moves, not the size of their head.

Why it beats what exists: it is the only single score that looks at the source, the tract and the nasal port together, alignment-free, per 80 ms tick, for any voice.

## How to run

```bash
cd tools/symphony/articulation
cargo build --release
VENV=~/a11-artic/venv RHUBARB=~/a11-artic/rhubarb/rhubarb ./run.sh ref.wav out/
VENV=~/a11-artic/venv RHUBARB=~/a11-artic/rhubarb/rhubarb ./run.sh clone.wav out/
target/release/galion-articulation compare --ref out/ref.articulation.json --clone out/clone.articulation.json
```

Audio never goes into git. Outputs (JSON) stay outside the repo too, because a per-tick track of a real voice is voice data.

## Honest results and what does not work yet

See the PR description for the measured numbers. In short:

* The acoustic-only TractPrint does **not yet separate** same-voice clones from other female voices well. It does separate a male voice. It needs the SPARC, glottal and nasal sections and a labelled set (same voice vs other voices) before any bar can be set.
* Distributions depend on what is said and the emotion. A fair bar needs reference and clone sets with comparable text and emotion.
* Not wired into conditioning, the voice card writer or the gate yet. That needs the stage-0 branch to land first.
