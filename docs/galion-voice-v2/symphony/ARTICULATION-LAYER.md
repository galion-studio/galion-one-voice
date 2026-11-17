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

* **Stage 0 voice card** (branch `swarm11/stage0-voice-card`, parallel agent). This layer proposes one `articulation` section (integration is not yet coordinated or implemented) and does not duplicate anything that agent already measures (refcheck loudness/RT60/noise, hygiene, emotion2vec+). The existing prose and prosody articulation profile (`tools/cloning-v2/emotion-blend/A-global/build_profile.py`: VSA, fricative centroids, VOT, pause rate) stays where it is. This layer adds the parts that profile lacks: articulator positions, glottal source, nasality, mouth shapes, all on the 80 ms tick.
* **Clone conditioning or post-filter.** Not wired yet. The worst TractPrint features are emitted as a list (`worst`), which is the hook for picking conditioning clips (best-of-N) or a later post-filter.
* **Gate scorer.** TractPrint is reported next to WER, ECAPA and emotion2vec+. It is **not** a blocking gate yet: there is no agreed bar (see "Honest results" below).

## Bricks (reused, as-is)

| Part | Brick | Licence | Language, where it runs | Works |
|---|---|---|---|---|
| Pitch, voicing, HNR, Burg formants F1–F4, bandwidths | [praatfan_rust](https://github.com/ucpresearch/praatfan-core-clean) v0.1.16 (clean-room Praat) | MIT | Rust, in process | yes (built and run on GALION-STUDIO WSL) |
| Articulator traces: tongue dorsum/body/tip, lower incisor (jaw), upper and lower lip, 12 EMA channels at 50 Hz, plus source pitch, loudness, periodicity and a 64-d speaker vector | [SPARC](https://github.com/Berkeley-Speech-Group/Speech-Articulatory-Coding) 0.1.0, weights `cheoljun95/Speech-Articulatory-Coding` | MIT (code, PyPI); MIT (HF weights card) | Python/PyTorch, out of process (`workers/sparc_worker.py`) | yes: reference plus five comparisons on GALION-STUDIO WSL |
| Glottal source: GCI variability, open quotient (QOQ), NAQ, H1–H2, HRF (IAIF + SE-VQ) and phonation (jitter, shimmer, APQ, PPQ) | [DisVoice](https://github.com/jcvasquezc/DisVoice) 0.1.10 | MIT | Python, out of process (`workers/disvoice_worker.py`) | yes: reference plus five comparisons on GALION-STUDIO WSL |
| Nasal, labial, dental, velar, stop, vocalic, … posteriors per 10 ms | [Phonet](https://github.com/jcvasquezc/phonet) 0.3.7 | MIT | Python/TensorFlow, out of process (same worker) | yes: reference plus five comparisons on GALION-STUDIO WSL |
| Mouth shapes A–H, X (language-independent `phonetic` recogniser) | [Rhubarb Lip Sync](https://github.com/DanielSWolf/rhubarb-lip-sync) 1.13.0 | MIT | C++ CLI, out of process | yes: reference plus five comparisons on GALION-STUDIO WSL |

Python is used where no suitable working Rust equivalent was found (searched crates.io and GitHub on 2026-11-17: no Rust acoustic-to-articulatory inversion, IAIF/GCI, jitter/shimmer, phonological-posterior or lip-sync crate). Nothing was ported or rewritten (owner t1375u). The ML bricks run in their own process, so the locked main workspace stays free of `ort` and `candle` (LOCKS `formats.forbidden_runtimes`). The Rust tool has its own cargo workspace, like `tools/emotion-v2/refcheck`.

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

Licences are on hold for internal development (owner t1360u). Direct selected bricks advertise MIT. **Transitive exception:** DisVoice imports praat-parselmouth 0.4.7 (GPL-3.0), even for the chosen glottal/phonation path; this dependency must be recorded and reviewed before distribution. TensorFlow is Apache-2.0, PyTorch BSD-style. SPARC model MIT metadata does not establish freedom from patents (Berkeley lists SPARC as patent-pending). No distribution clearance is asserted.

## Data contract (80 ms tick)

One JSON object, the voice card's proposed `articulation` section (`schema: 1`):

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
* Sections: **throat** = `f0_hz`, `f0_st`, `voicing`, `hnr_db`, `sparc_periodicity`, `sparc_loudness`, `phon_voice`, DisVoice glottal and phonation statics. **mouth** = `f1..f4_hz`, `b1..b3_hz`, `vtl_cm`, `ema_*`, `lip_aperture`, Phonet place and manner posteriors, Rhubarb mouth shapes. **nose** = `phon_nasal`. This is a **nasal speech-class posterior, not a measurement of nasal resonance or clinical nasalance**. No acoustic A1–P0, nasal formant or anti-formant measurement is implemented yet. Those fields must remain unavailable, not inferred from this posterior.
* Settings are chosen from the voice itself (Praat conventions): two-pass pitch range (0.75·q25, 1.5·q75), formant ceiling 5500 Hz if median F0 ≥ 165 Hz else 5000 Hz.

## Our own new idea: TractPrint

Reused bricks give separate numbers; none of them says "does this clone articulate like this reference?". ECAPA checks identity, WER checks words, emotion2vec+ checks feeling. Nothing checks the throat, mouth and nose together.

TractPrint (Rust, `src/score.rs`):

* Every track becomes a distribution over ticks (nine quantiles). Two voices are compared per track with a quantile-function distance (Wasserstein-1 over 5–95 %), scaled by the reference's own spread (IQR): `sim = 1 / (1 + W1 / IQR_ref)`.
* Mouth shapes use 1 − total-variation distance of the A–H/X histograms. Static glottal numbers use `1 − |a−b| / (|a|+|b|)`. Skewness/kurtosis are retained in output but excluded from scoring because short-clip sign flips were unstable in the first experiment.
* A section is the mean of its features. The total is the **geometric mean of the sections**, so a clone that gets the mouth right but loses the nasal resonance cannot hide behind the other two.
* It needs no time alignment and no same text, so a clone saying new words can still be checked against the reference.
* It reports the five worst features, so the next step (conditioning choice or post-filter) knows what to fix.
* SPARC's EMA lives in a speaker-agnostic template space, so the mouth section measures *how* the speaker moves, not the size of their head.

Why prototype this: it aggregates existing source and tract outputs and a nasal-class proxy in one explainable, alignment-free report. **No superiority or uniqueness claim is established.** This is an experimental heuristic, not a validated speaker-identity/articulation metric. Quantiles of SPARC EMA can be similar between different speakers, and aggregation can dilute important pitch or identity differences. It cannot recover exact individual anatomy from one recording.

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

Real run on GALION-STUDIO WSL, 2026-11-17 (Europe/Zagreb), with threads capped at four and `nice -n 19` to leave room for A3. The original WAV is read-only; no audio is in the commit.

Reference: `/workspace/samantha-original/Samantha-sent-2026-11-05.wav`, 17.407188 s, 218 ticks. Median [10th, 90th percentile]:

| Feature | Samantha |
|---|---|
| F0 | 177.503 [160.418, 231.065] Hz |
| HNR (CC) | 17.719 [11.961, 23.523] dB |
| F1 / F2 / F3 / F4 | 431.512 / 1857.581 / 2715.308 / 4081.669 Hz |
| Formant-dispersion VTL estimate | 14.709 [12.996, 16.679] cm (acoustic estimate, not anatomy) |
| IAIF quasi-open quotient (mean) | 0.344 |
| IAIF NAQ (mean) | 0.012 |
| IAIF H1–H2 (mean) | 8.1618 dB |
| DisVoice jitter / shimmer (mean) | 3.6458 / 7.0984 (upstream units, not clinical local-jitter/shimmer) |
| SPARC lip-aperture proxy | −0.057 [−2.912, 1.811] template units (standardised channels; not millimetres or a physical gap) |
| Phonet nasal-class posterior | 0.099 [0.002, 0.638] (not nasal resonance) |

Rhubarb 80 ms mouth-shape counts: B 93, C 48, X 26, A 19, E 16, F 10, H 3, G 2, D 1. The 12 SPARC EMA channels were returned (tongue dorsum/body/tip, jaw/lower-incisor, upper/lower lip x/y).

TractPrint v1, 77 compared features, after excluding unstable skewness/kurtosis:

| Local sample label (provenance not independently verified) | Overall | Throat | Mouth | Nasal-class proxy |
|---|---:|---:|---:|---:|
| `samples/v2/samantha/old-t01-baseline.wav` | 0.850 | 0.796 | 0.853 | 0.906 |
| `samples/own-samantha-fp32.wav` | 0.833 | 0.790 | 0.849 | 0.862 |
| `samples/v2/pipeline/samantha/t01.wav` | 0.785 | 0.747 | 0.799 | 0.811 |
| Control: Piper LJSpeech | 0.799 | 0.749 | 0.834 | 0.818 |
| Control: Piper Norman (male) | 0.795 | 0.708 | 0.805 | 0.882 |

**Important negative result:** the male control still scores 0.795; this metric is not sufficiently discriminative for a hard gate. Acoustic-only results were 0.852 baseline, 0.830 FP32, 0.602 pipeline t01, 0.775 LJSpeech, 0.578 Norman. The aggregated ML channels dilute that discrimination. The score needs same-text/emotion labelled validation and weighting calibration, not an invented pass threshold.

Other limitations:

* The glottal HRF output is unstable (reference mean 985.1372, with large outliers); it is exposed for inspection, not asserted as physiological truth. Creak, breathiness and open quotient are proxies/estimates, not direct laryngoscopy.
* No clone-time synthesis, conditioning, post-filter, facial-expression reconstruction, or physically validated nasal resonance is implemented. SPARC decode / VocalTractLab are research candidates only.
* Runtime is offline; tick compatibility does not mean the full models run in an 80 ms real-time budget.
* Rust tests passed on laptop Rust 1.100 and 1.85: two synthetic-signal integration tests, no voice audio fixtures. Cargo.lock pins `encoding_rs` to 0.8.35 because 0.8.44 needs Rust 1.88.
* Distributions depend on what is said and the emotion. A fair bar needs reference and clone sets with comparable text and emotion.
* Not wired into conditioning, the voice card writer or the gate yet. That needs the stage-0 branch to land first.
