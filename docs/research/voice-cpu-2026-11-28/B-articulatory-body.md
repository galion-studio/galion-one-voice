# B — articulatory body on CPU

Perpetual Open MIT License. (c) 2026 Galion Labs.

Research only. No product code. No pull request. Written 2026-11-28.

Stars and `pushed_at` dates are the GitHub repository API on that day. A date is a last push, not a promise that the tip builds. CPU speed is quoted only where a source states a number or a real-time audio callback. Otherwise it is **unmeasured**. "Sounds human" is written only where a listening result was opened. A demo you can click is not that result.

`parallel-cli` 0.10.4 installed and `parallel-cli auth --json` returned `authenticated: false`. The searches below are GitHub repository search plus the pages those hits and the web search opened. Hugging Face was searched the same way. Nothing here was listened to.

## 1. Ten lines on the stack we already have

1. The 7881 mouth is Piper `en_US-ljspeech-medium` only when `voice-stack/local-server` is built with `--features piper`. Without that feature the CPU mouth is an in-process tone. `GALION_MOUTH=native` (also `horbm` or `text`) selects `NativeMouth`, which speaks `horbm::TurnOut` `first_clause` then `rest` as text and returns no PCM (`voice-stack/local-server/src/mouth.rs`, `docs/horbm/foundations/S48-native-mouth.md`).
2. `crates/galion-source-filter` does not synthesise a word. On each 80 ms frame it shapes Piper or flite PCM with biquads: spectral tilt, an F1/F2 shift, dips at 250 Hz and 1.5 kHz, then exhale noise. Neutral controls leave the samples alone (`crates/galion-source-filter/src/lib.rs`).
3. Those four knobs are read off `horbm_breath::Frame`: open quotient, breathiness, nasal coupling, mouth openness. On one Linux Xeon-class host, release, full controls, the median was 32 833 ns per 24 kHz frame, RTF 0.000410 (`docs/horbm/foundations/F-mouth-source-filter.md`). That number is a filter, not a tract.
4. `horbm/crates/horbm-breath` plans phrase pauses and ticks relative lung volume, phase, flow, and relative subglottal pressure. Pressure is not cm H₂O. The same frame carries Fant open quotient, breathiness, nasal coupling, and mouth openness (`horbm/crates/horbm-breath/src/lib.rs`, `src/glottis.rs`). `horbm-e2e` calls `Lungs`. A search of `voice-stack/local-server` finds no `horbm_breath` call, so the 7881 mouth does not tick it.
5. `voice-stack/emotion-in` turns mic frames into a PAD target (YIN F0, RMS, syllable rate, pauses). Its README says the real `horbm` crate is not linked. The live brain stores `rate`, `energy`, `pitch_st`, and `pause_ms` from the empathy tick (`horbm/crates/horbm/src/brain.rs`).
6. `Brain::apply_prosody` multiplies rate and adds pause. It does not write a pitch contour. Duplex `ProsodyEar` reports `f0_hz` and does not build a `Heard` (`docs/horbm/PLAN-brain.md`). `galion-cascade` is the English ASR → reply → TTS lane, stubs included (`crates/galion-cascade/src/lib.rs`). It is not a body.
7. Lungs, already planned: relative volume, relative pressure, loudness, inhale noise, phrase groups (`docs/galion-voice-v2/symphony/LUNGS.md`, `horbm-breath`). Declination of F0 from falling pressure is specified there and is not a rendered contour. The source-filter does not read `pressure`.
8. Throat, already planned: Rd, open quotient, phonation label, spectral tilt, jitter and shimmer, optional Liljencrants–Fant pulse (`docs/galion-voice-v2/symphony/THROAT.md`, `tools/symphony/throat`). `horbm-breath` already computes the open quotient from Rd. It does not emit the pulse.
9. Mouth, already planned: F1–F3 direction, lip class, jaw open/mid/close, tongue front/back and high/low, as planning coordinates, not millimetres (`docs/galion-voice-v2/symphony/MOUTH.md`). The live filter only shifts F1 and F2.
10. Nose, already planned: `velum_open` from 0 to 1, a murmur, an A1–P0 direction whose slope ships at 0 (`docs/galion-voice-v2/symphony/NOSE.md`). `horbm-breath` carries `nasal_coupling` in 0..1. The filter renders that as two fixed dips. The string "HORBM interactive (12)" is not a file in this checkout. The nearest note is the 12 cross-cultural prosody categories in `docs/galion-voice-v2/symphony/ARTICULATION-V2-SPEC.md`.

## 2. What a candidate has to accept

One 80 ms `Frame` already has: relative `pressure`, `open_quotient`, `breathiness`, `nasal_coupling`, `mouth_openness`, plus `phase` / `volume` / `flow`. The symphony planners add, and do not yet radiate: tongue, lips, jaw, and a velum. Pitch on the live turn is one sentence offset in semitones, not a curve.

A body model plugs in by taking that frame and radiating PCM, or by emitting the three streams [FARGAN](https://arxiv.org/html/2405.21069) already eats: 18 Bark cepstra, a pitch period, and a voicing bit, at 10 ms. FARGAN stays the vocoder. This note does not propose rewriting it. The Xiph demo page states 600 MFLOPS and less than 1% of one laptop core ([Opus 1.5 demo](https://gitlab.xiph.org/xiph/opus-website/-/blob/b34047420fc435937925b04ea9fa58456d5f8aec/demo/opus-1.5/index.html)). The C code and the PyTorch trainer live in [xiph/opus](https://github.com/xiph/opus) (`dnn/torch/fargan/README.md`; 3397 stars; last push 2026-11-27; `COPYING` is a 3-clause BSD text; the GitHub license field is `NOASSERTION`). [xiph/LPCNet](https://github.com/xiph/LPCNet) (1224 stars, BSD-3-Clause, last push 2024-09-21) tells readers to switch to FARGAN and describes its own synthesis as about 3 GFLOP on a SIMD CPU.

## 3. Search misses, so they are not candidates

- GitHub repository search for `DiffVTL` in name, description, and readme returned `total_count: 0` on 2026-11-28. The differentiable tracts that did appear are [davidmarttila/vocal-tract-grad](https://github.com/davidmarttila/vocal-tract-grad) and [echen01/diffTLM](https://github.com/echen01/diffTLM), below.
- GitHub search for `TubeTalker` returned YouTube-chat projects, not Brad Story's airway model. The model is described as C compiled to a MATLAB mex, with a compute-time to real-time ratio of about 8:1 on a PC laptop or MacBook Air ([Story 2011 PDF](https://bpb-us-e2.wpmucdn.com/sites.arizona.edu/dist/f/80/files/2023/10/Story-2011_0-1.pdf)). No current public source tree was found. [LeTalker](https://sites.arizona.edu/sapl/research/le-talker/) is a different, slow MATLAB lumped-element fold model.
- GitHub search for `"EMA-to-speech"` matched email-to-speech repositories. The articulatory-feature vocoders that did appear are SPARC and STArK, below. They are neural decoders, not a CPU tract.

## 4. Candidates

Body columns are what the opened source actually names. A blank is **unknown**, not "absent."

### 4.1 Pink Trombone and the Rust ports

The control set used by the neural controller is seven numbers plus F0: `voiceness`, `intensity`, `tongueIndex`, `tongueDiameter`, `constrictionIndex`, `constrictionDiameter`, `lipDiameter`, and `log2(f0 / 187 Hz)` ([samuel-tts on Hugging Face](https://huggingface.co/vvolhejn/samuel-tts)). A small dataset lists the same family plus `throat_diam` ([mcamara/10-random-configurations-pink-trombone](https://huggingface.co/datasets/mcamara/10-random-configurations-pink-trombone)). Subglottal pressure in cm H₂O, a jaw, and a velum are not in that list. The card says the voice "sounds like a synthetic mouth." That is the author's description, not a MOS.

| Repo | Stars | Last push | License | Language | CPU | Body | Heard |
|---|---:|---|---|---|---|---|---|
| [zakaton/Pink-Trombone](https://github.com/zakaton/Pink-Trombone) | 231 | 2026-11-20 | GPL-3.0 | JavaScript | unmeasured; it is a browser instrument | the programmable Pink Trombone; exact slider list not opened | unknown |
| [lostmsu/pink-trombone](https://github.com/lostmsu/pink-trombone) | 34 | 2021-08-02 | MIT | Rust | unmeasured | README: two-dimensional digital waveguide of the Pink Trombone. Velum not named in the README that was opened | unknown |
| [sunsided/synthie-trombone](https://github.com/sunsided/synthie-trombone) | 3 | 2026-11-23 | EUPL-1.2 | Rust | release build is what the author says can keep up with the sound card; a debug build cannot. 96 000 steps per second of audio. No RTF number | tongue, up to four constrictions, velum; `VoiceSettings.nasality`. No cm H₂O | author: "between a vowel and a very confused trombone" |
| [PaulBatchelor/voc](https://github.com/PaulBatchelor/voc) | 155 | 2019-01-12 | none on the API (archived) | CWEB | a JACK demo is described; no RTF | Kelly–Lochbaum tube plus an LF glottis ([pbat.ch/proj/voc](https://pbat.ch/proj/voc/)). Project page says abandoned; pieces moved to sndkit | unknown |
| [vvolhejn/samuel](https://github.com/vvolhejn/samuel) | 129 | 2026-11-22 | GPL-3.0 | Python | unmeasured | predicts the seven Pink Trombone controls above. Not a second tract | "synthetic mouth" on the model card |
| [davidmarttila/vocal-tract-grad](https://github.com/davidmarttila/vocal-tract-grad) | 41 | 2023-07-16 | none on the API | Python | unmeasured; offline gradient descent | glottal source and tract area of Pink Trombone | a subjective test beat evolutionary search and a neural net at matching human vowels through Pink Trombone ([repo README](https://github.com/davidmarttila/vocal-tract-grad/)) |

Plug. `mouth_openness` → `tongueDiameter` and `lipDiameter`. `breathiness` and open quotient → `voiceness` (higher open quotient, less pressed; the scale is a knob, not a published map). `pressure` / `loudness_gain` → `intensity`. `pitch_st` plus the baseline centre → F0. `nasal_coupling` has a home only where a velum exists: synthie names one; the Samuel seven do not. Jaw has no pin. Do not also run `galion-source-filter` on the same samples. The waveguide is the filter. If the waveguide sounds thin, take its F0, a voicing bit, and 18 Bark cepstra of its frame and hand those to FARGAN. That is a feed, not a rewrite.

`lostmsu` is the MIT Rust port. [sunsided](https://github.com/sunsided/synthie-trombone) writes that the line-by-line port allocates on the audio path, which is why that author rewrote it. EUPL-1.2 is copyleft. Private use is allowed by the task. Putting it inside the MIT tree is a separate decision, and this note does not make it.

### 4.2 VocalTractLab

[TUD-STKS/VocalTractLabBackend-dev](https://github.com/TUD-STKS/VocalTractLabBackend-dev): 22 stars, C++, GPL-3.0, last push 2026-06-14. C API in `VocalTractLabApi.h` ([README](https://github.com/TUD-STKS/VocalTractLabBackend-dev)). Stable Windows build named on the site is 2.4, 5 December 2025, GPL ([VTL 2.4 manual](https://vocaltractlab.de/download-vocaltractlab/VTL2.4-manual.pdf), [download page](https://www.vocaltractlab.de/index.php?page=vocaltractlab-download)).

The manual is the body list. Geometric tract reduced to 19 controls (the 19 names were not copied out here). Phonetic layer: tongue height, tongue frontness, lip rounding, velum position, and three constriction degrees (bilabial, apico-alveolar, dorso-velar). Glottis: geometric models and a triangular two-mass model. Excitation for the frequency-domain vowel is the LF model. Time-domain view shows trachea (23 sections), glottis, oral tube, nasal cavity, and paranasal sinuses as Helmholtz resonators. Subglottal input impedance is a display, not a cm H₂O knob we measured. The manual says VTL is not yet a full text-to-speech system; connected speech comes from gestural scores or from phone sequences plus durations and pitch.

CPU. Marwitz, Stone, and Birkholz, ESSV 2018: after a Cholesky solver, synthesis reached a real-time factor of 2 on the PC hardware of that paper, down from about 4.5 ([PDF](https://www.vocaltractlab.de/publications/marwitz-2018-essv.pdf)). RTF 2 is slower than real time. No 2025 measurement was opened. [FBerthommier/covtl-console](https://github.com/FBerthommier/covtl-console) says the native synthesis holds one core for the whole job. Stars of that console were not fetched.

Heard. The manual describes synthesis and the site links example videos. No MOS figure was opened. Unknown as a listening result.

Plug. This is the only opened model that already has a subglottal system, a glottis, a velum, a tongue, lips, a jaw (through the geometric parameters), and a nose. `horbm-breath.pressure` would scale the model's lung drive. `open_quotient` would bias the LF or the geometric glottis, not replace VTL's own Rd. `nasal_coupling` would move velum position. `mouth_openness` is too poor for 19 geometric controls; the symphony tongue, lip, and jaw targets are the rest, and they are still directions with unknown millimetres. GPL-3.0 stays out of the MIT crates. Use it as the reference anatomy, or as an offline teacher that writes F0, voicing, and cepstra for FARGAN. [paul-krug/VocalTractLab-Python](https://github.com/paul-krug/VocalTractLab-Python) is 34 stars, GPL-3.0, last push 2025-05-07, a Python wrapper, alpha by its own README. [quantling/paule](https://github.com/quantling/paule) is 9 stars, GPL-3.0, last push 2025-03-27: it plans VTL control trajectories. Speed unmeasured.

### 4.3 CHIMERA

[takuma104/physical-modeling-voice-codec](https://github.com/takuma104/physical-modeling-voice-codec): 8 stars, Python, MIT, last push 2026-11-26. The model card [takuma104/pmvc-tts](https://huggingface.co/takuma104/pmvc-tts) says the network writes 21 channels at 100 frames per second for tongue, jaw, lips, soft palate, vocal folds, and how hard the lungs push, and that CHIMERA, named as Jameson Hodge's airway simulation, makes the sound. The card says CHIMERA is used unmodified and is MIT. The CHIMERA repository's star count was not fetched. CPU speed: unmeasured. Heard: the card does not report a MOS.

Plug. This vector is the closest public match to the planners we already have: lungs, folds, tongue, jaw, lips, velum. It is Python, so it does not sit on the 80 ms Rust tick until someone measures it. Same FARGAN feed as above if the physical waveform is the teacher and FARGAN is the radiator. [ludi-uni/voice-identity-experiments](https://github.com/ludi-uni/voice-identity-experiments) is 0 stars, license `NOASSERTION`, last push 2026-11-09, described as CHIMERA voice-identity experiments. [smulelabs/ctract](https://github.com/smulelabs/ctract) is 5 stars, MIT, Python, last push 2026-11-13: "an inverse articulatory synthesis solver for CHIMERA." Inversion, not the radiator.

### 4.4 Gnuspeech, Praat, Maeda

[Gnuspeech TRAcT manual 0.9](https://cspages.ucalgary.ca/~hilld/papers/tract-manual.pdf): eight oral tube radii, a velar opening, a nasal tube, and separate energy for voicing, whisper, frication, and breathy noise. Subglottal pressure as cm H₂O is not in the pages opened. Tongue, lips, and jaw are not the controls; the tube radii are. Speed: unmeasured in the pages opened.

Repos. [nygard/GnuSpeech](https://github.com/nygard/GnuSpeech): 47 stars, Objective-C, license `NOASSERTION`, last push 2015-09-16. [astoeckel/gnuspeech_trm](https://github.com/astoeckel/gnuspeech_trm): 12 stars, C++, GPL-3.0, last push 2024-01-16. [mym-br/gnuspeech_sa](https://github.com/mym-br/gnuspeech_sa): 11 stars, C++, GPL-3.0, last push 2024-12-27. [emer/gospeech](https://github.com/emer/gospeech): 3 stars, Go, BSD-3-Clause, last push 2024-01-22, from the earlier VocalTractLab search. Heard: unknown.

[Praat articulatory synthesis](https://www.fon.hum.uva.nl/praat/manual/Articulatory_synthesis.html): muscle targets on an Artword. The tutorial names `Lungs`, `Interarytenoid` (glottis), `LevatorPalatini` (nasopharyngeal port), `Masseter` (jaw), and `OrbicularisOris` (lips). The full muscle list was not copied. Physics: Boersma 1998, chapters 2 and 3, as that page cites. Speed, same page: a 0.5 s [əpə] took about 5 minutes on a 1997 computer and about 1 second on a 2019 computer. That 2019 figure is about twice real time, and it is one anecdote, not a benchmark table. License: GPL-3.0 or later ([praat.org license](https://praat.org/manual/License.html)). Code: [praat/praat.github.io](https://github.com/praat/praat.github.io), 2039 stars, C, last push 2026-11-27, license field null on the API. Heard: unknown. Plug: `pressure` → `Lungs`, open quotient → `Interarytenoid` only as a direction, `nasal_coupling` → lower `LevatorPalatini`, `mouth_openness` → `Masseter` inverted (open mouth, less masseter). Too slow on the cited 2019 machine for the 80 ms tick. A Praat run can still be the offline teacher for FARGAN features.

Maeda. [FBerthommier/Syllable_Synthesis](https://github.com/FBerthommier/Syllable_Synthesis): 2 stars, Python, MIT, last push 2025-05-27, "Articulatory Syllable Synthesis Using the Maeda Model." Which of Maeda's parameters the code exposes was not opened. Speed unmeasured. Heard: unknown. The original Maeda distribution was not found in this search.

### 4.5 Klatt and other formant ports

These are filters with formant knobs, not a tongue. They match the F1/F2 shift we already apply, and they are the honest CPU fallback if a waveguide fails the ear.

| Repo | Stars | Last push | License | Language | CPU | What it exposes |
|---|---:|---|---|---|---|---|
| [chdh/klatt-syn](https://github.com/chdh/klatt-syn) | 80 | 2026-06-26 | MIT | TypeScript | unmeasured | Klatt formant synthesizer. Parameter list not opened |
| [guestdaniel/tdklatt](https://github.com/guestdaniel/tdklatt) | 23 | 2018-05-06 | MIT | Python | unmeasured | Klatt in Python 3. Parameter list not opened |
| [TTWNO/klatt](https://github.com/TTWNO/klatt) | 3 | 2025-02-06 | Apache-2.0 | Rust, `no_std` with `libm` | unmeasured | README: speech-sounding samples from voice parameters, not text. The parameter struct was not opened |
| [yuanhao-chen-nyoeghau/klatt-dx](https://github.com/yuanhao-chen-nyoeghau/klatt-dx) | 0 | 2025-07-16 | none on the API | Rust | unmeasured | "Klatt Formant Synthesis Visualizer." Not opened past the search hit |
| [praat-enthusiast/KlattGrid-scripts](https://github.com/praat-enthusiast/KlattGrid-scripts) | 2 | 2022-08-23 | none on the API | Praat scripts | inherits Praat | voice-quality edits on KlattGrid. Not a synthesizer of its own |

Plug. Our F1/F2 targets and the tilt from open quotient become formant frequencies, bandwidths, and source tilt. `nasal_coupling` becomes a nasal formant or anti-formant if the port has one; TTWNO's README does not say that it does. `pressure` becomes amplitude. The formant frame's cepstrum, F0, and voicing are a legal FARGAN input. [ctoth/Qlatt](https://github.com/ctoth/Qlatt) is 13 stars, TypeScript, license none on the API, last push 2026-11-19: a WebAudio Klatt with a WASM worklet. Speed unmeasured.

### 4.6 Area-function and other small tracts

| Repo | Stars | Last push | License | Language | Note |
|---|---:|---|---|---|---|
| [Simon-Stone/AreaFunctionSynthesis](https://github.com/Simon-Stone/AreaFunctionSynthesis) | 0 | 2022-12-12 | GPL-3.0 | Makefile (search label) | "speech from a one-dimensional model of the vocal tract area function." Speed unmeasured. Parameters not opened |
| [kinoh/VoiceSynthesis](https://github.com/kinoh/VoiceSynthesis) | 8 | 2014-02-12 | MIT | C++ | search description: "Real-time articulatory synthesizer." No RTF number in the search hit. Body parameters not opened |
| [audiojs/voice](https://github.com/audiojs/voice) | 1 | 2026-09-14 | NOASSERTION | JavaScript | umbrella for vocal-tract, voder, and glottal-source atoms. Not opened past the search hit |
| [INCzipper/DreamTract](https://github.com/INCzipper/DreamTract) | 0 | 2025-07-19 | GPL-2.0 | C++ | "another perspective on vocal synthesis." Not opened |
| [Jazvie/sf_vocoder](https://github.com/Jazvie/sf_vocoder) | 0 | 2025-04-08 | none on the API | Python | "WIP source-filter vocoder." Not a body |
| [PapayaResearch/vocaltrax](https://github.com/PapayaResearch/vocaltrax) | 4 | 2025-06-03 | MIT | Python | NeurIPS 2024 Audio Imagination: articulatory synthesis by optimization, JAX. Offline. Not a 80 ms tick |

`kinoh/VoiceSynthesis` is the small MIT C++ hit that claims real time in its description. The claim has no number attached. Treat the speed as unmeasured until the binary is timed.

### 4.7 Neural source-filter, DDSP, differentiable tubes

These take acoustic features or invert a tube. They do not replace `horbm-breath`.

[magenta/ddsp](https://github.com/magenta/ddsp): 3406 stars, Python, Apache-2.0, last push 2026-10-19. Harmonic-plus-noise synthesizer. The controls that match us are F0 and loudness, which we already have as `pitch_st` and `loudness_gain`. No tongue, velum, or subglottal pressure. CPU speed: unmeasured here. Our own articulation note already set it aside for cloning ([ARTICULATION-LAYER.md](../../galion-voice-v2/symphony/ARTICULATION-LAYER.md)). A DDSP harmonic frame can be analysed into cepstra for FARGAN; that is a second vocoder in front of the one we would keep, so it is not the first test.

[nii-yamagishilab/project-NN-Pytorch-scripts](https://github.com/nii-yamagishilab/project-NN-Pytorch-scripts): 367 stars, Python, BSD-3-Clause, last push 2026-09-04. `project/01-nsf` holds cyclic-noise NSF, hn-NSF, and hn-sinc-NSF ([MaxMax2016/nsf-hifigan overview](https://github.com/MaxMax2016/nsf-hifigan), which describes that tree). Wang, Takaki, and Yamagishi: the NSF models generated waveforms at least 100 times faster than their WaveNet vocoder, and the best model was comparable to WaveNet on a single-speaker Japanese corpus ([arXiv:1904.12088](https://arxiv.org/abs/1904.12088)). Absolute CPU RTF: unknown. Inputs are acoustic features (F0 and a spectrum), not articulators. [Takaaki-Saeki/simplified_neural_source_filter](https://github.com/Takaaki-Saeki/simplified_neural_source_filter): 14 stars, MIT, Python, last push 2021-08-04. [34j/neural-source-filter](https://github.com/34j/neural-source-filter): 7 stars, BSD-3-Clause, archived, last push 2026-11-02. [proger/nsf](https://github.com/proger/nsf): 3 stars, MIT, last push 2025-11-11. [uthree/oscillators](https://github.com/uthree/oscillators): 0 stars, MIT, last push 2024-08-27, "oscillator implementations for neural source filter models." None of these expose a velum.

[echen01/diffTLM](https://github.com/echen01/diffTLM): 23 stars, Python, MIT, last push 2026-11-25. NeurIPS 2026. Differentiable acoustic tube. The project page says frequency-domain simulation was 70× faster than time-domain wave solvers on an RTX 4090 ([echen01.github.io/vocal-tract](https://echen01.github.io/vocal-tract/)). That is a GPU figure. CPU RTF: unmeasured. The fit estimates a glottal pulse and then a tube area. It is an inverter and a teacher, not the 80 ms mouth. Its area function could be rendered by a CPU waveguide, or its waveform could be analysed into FARGAN features. [articulatory/articulatory](https://github.com/articulatory/articulatory): 58 stars, Python, Apache-2.0, last push 2024-02-14, "Deep Articulatory Synthesis and Inversion." The README body was not opened, so its controls and speed are unknown.

SPARC and STArK decode articulatory features with a neural vocoder. [Lab-MSP/stark-large](https://huggingface.co/Lab-MSP/stark-large) says STArK resynthesises with the SPARC vocoder. [nzxyin/sparc-wavenext-24k](https://huggingface.co/nzxyin/sparc-wavenext-24k) takes 12 EMA channels plus F0, loudness, and periodicity at 50 Hz. That is a GPU-class vocoder on top of an inversion we already run out of process (`tools/symphony/articulation`). It does not take `horbm-breath` pressure. CPU speed: unmeasured. Code repo [Berkeley-Speech-Group/Speech-Articulatory-Coding](https://github.com/Berkeley-Speech-Group/Speech-Articulatory-Coding) is already the one the articulation layer cites. Not a new radiator.

### 4.8 LF glottis, already ours

The LF pulse and the Rd regressions are already written in `tools/symphony/throat` from Fant, Liljencrants and Lin (1985) and Fant and Kruckenberg (1996), as that note cites, and `horbm-breath` already maps Rd to open quotient. VTL's frequency-domain vowel uses the same LF model ([manual](https://vocaltractlab.de/download-vocaltractlab/VTL2.4-manual.pdf)). Voc's page says its excitation is LF ([pbat.ch/proj/voc](https://pbat.ch/proj/voc/)). A new LF library would duplicate the throat crate. The missing piece is the tract in front of that pulse.

## 5. Rank

Judged only on this job: accept the controls we already tick, run on CPU at or under real time with a source for that claim, a license we could read, and a path into FARGAN that is not a rewrite.

1. **[sunsided/synthie-trombone](https://github.com/sunsided/synthie-trombone).** Rust, tongue, constrictions, velum, a release build the author says meets the sound card. EUPL-1.2. Three stars, first commit 2026-11-02. Human-likeness is the author's joke, not a test. First bench because it is the only Rust tract that names a velum and a real-time callback in a source we opened.
2. **[lostmsu/pink-trombone](https://github.com/lostmsu/pink-trombone).** Same family, MIT, Rust, 34 stars, last push 2021-08-02. Use this if EUPL must not enter the tree. Velum and RTF are unknown. sunsided says this port allocates.
3. **[takuma104/physical-modeling-voice-codec](https://github.com/takuma104/physical-modeling-voice-codec) with CHIMERA.** MIT, and the 21-channel vector matches lungs, folds, tongue, jaw, lips, and palate better than Pink Trombone's seven. Python. Speed unknown, so it is not the first clock test.
4. **[TUD-STKS/VocalTractLabBackend-dev](https://github.com/TUD-STKS/VocalTractLabBackend-dev).** The anatomy reference: subglottal system, glottis, velum, tongue, lips, jaw, nose, LF. GPL-3.0. The only speed number opened is RTF 2 in 2018, which fails the live tick. Offline teacher for FARGAN features, not the mouth.
5. **[TTWNO/klatt](https://github.com/TTWNO/klatt).** Apache-2.0, Rust, `no_std`. Not a body. It is the CPU formant path if the waveguide fails a listening check, and its frame can become FARGAN's cepstra, pitch, and voicing. Speed unmeasured.

Praat's articulatory synth and TubeTalker fail the speed bar on the numbers that were opened (about 2× and about 8× real time). DDSP, NSF, diffTLM, SPARC, and STArK fail the body-on-CPU bar: they are feature vocoders or GPU inversion.

## 6. Best first test

Out of tree. No change to Piper, to `galion-source-filter`, or to FARGAN.

1. Build [sunsided/synthie-trombone](https://github.com/sunsided/synthie-trombone) in release, or [lostmsu/pink-trombone](https://github.com/lostmsu/pink-trombone) if the license must be MIT.
2. Hold one vowel. Map one rest `horbm_breath::Frame` onto intensity, voiceness, tongue diameter, lip diameter, and, on synthie only, velum from `nasal_coupling`. Leave jaw unused and write that down.
3. Time one second of 16 kHz audio on this CPU. Pass only if the wall clock is under one second. The author text is not that measurement.
4. Listen. If it is a toy mouth, keep the recording as the failure, and only then analyse that same waveform into 18 Bark cepstra, a pitch period, and a voicing bit and run the existing `fargan_demo` synthesis ([FARGAN README](https://github.com/xiph/opus/blob/main/dnn/torch/fargan/README.md)). Compare the two files. Do not train, and do not fork FARGAN.

If that second file is the one a person would keep, the body stays the controller and FARGAN stays the radiator. VocalTractLab is the check on whether the seven Pink Trombone numbers are enough anatomy, run offline, not on the tick.

## Sources

- [VTL 2.4 manual](https://vocaltractlab.de/download-vocaltractlab/VTL2.4-manual.pdf) (manual dated 5 Dec 2025 in the file)
- [VTL download](https://www.vocaltractlab.de/index.php?page=vocaltractlab-download)
- [VocalTractLabBackend-dev](https://github.com/TUD-STKS/VocalTractLabBackend-dev)
- [Marwitz, Stone, Birkholz 2018, ESSV](https://www.vocaltractlab.de/publications/marwitz-2018-essv.pdf)
- [paul-krug/VocalTractLab-Python](https://github.com/paul-krug/VocalTractLab-Python)
- [quantling/paule](https://github.com/quantling/paule)
- [covtl-console](https://github.com/FBerthommier/covtl-console)
- [zakaton/Pink-Trombone](https://github.com/zakaton/Pink-Trombone)
- [lostmsu/pink-trombone](https://github.com/lostmsu/pink-trombone)
- [sunsided/synthie-trombone](https://github.com/sunsided/synthie-trombone)
- [PaulBatchelor/voc](https://github.com/PaulBatchelor/voc) and [project page](https://pbat.ch/proj/voc/)
- [vvolhejn/samuel](https://github.com/vvolhejn/samuel) and [samuel-tts](https://huggingface.co/vvolhejn/samuel-tts)
- [Pink Trombone parameter dataset](https://huggingface.co/datasets/mcamara/10-random-configurations-pink-trombone)
- [vocal-tract-grad](https://github.com/davidmarttila/vocal-tract-grad/)
- [Story 2011 TubeTalker PDF](https://bpb-us-e2.wpmucdn.com/sites.arizona.edu/dist/f/80/files/2023/10/Story-2011_0-1.pdf)
- [Story 2013, PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC3596841/)
- [LeTalker](https://sites.arizona.edu/sapl/research/le-talker/)
- [TubeTalker demos](https://sites.arizona.edu/sapl/research/tube-talker-speech-synthesis-demonstrations/)
- [Gnuspeech TRAcT manual](https://cspages.ucalgary.ca/~hilld/papers/tract-manual.pdf)
- [nygard/GnuSpeech](https://github.com/nygard/GnuSpeech)
- [astoeckel/gnuspeech_trm](https://github.com/astoeckel/gnuspeech_trm)
- [Praat articulatory synthesis](https://www.fon.hum.uva.nl/praat/manual/Articulatory_synthesis.html)
- [Praat license](https://praat.org/manual/License.html)
- [praat/praat.github.io](https://github.com/praat/praat.github.io)
- [FBerthommier/Syllable_Synthesis](https://github.com/FBerthommier/Syllable_Synthesis)
- [chdh/klatt-syn](https://github.com/chdh/klatt-syn)
- [TTWNO/klatt](https://github.com/TTWNO/klatt)
- [FARGAN paper](https://arxiv.org/html/2405.21069)
- [FARGAN README in opus](https://github.com/xiph/opus/blob/main/dnn/torch/fargan/README.md)
- [Opus 1.5 demo text](https://gitlab.xiph.org/xiph/opus-website/-/blob/b34047420fc435937925b04ea9fa58456d5f8aec/demo/opus-1.5/index.html)
- [xiph/opus](https://github.com/xiph/opus) and [xiph/LPCNet](https://github.com/xiph/LPCNet)
- [takuma104/physical-modeling-voice-codec](https://github.com/takuma104/physical-modeling-voice-codec)
- [pmvc-tts model card](https://huggingface.co/takuma104/pmvc-tts)
- [smulelabs/ctract](https://github.com/smulelabs/ctract)
- [magenta/ddsp](https://github.com/magenta/ddsp)
- [NSF paper](https://arxiv.org/abs/1904.12088)
- [nii-yamagishilab/project-NN-Pytorch-scripts](https://github.com/nii-yamagishilab/project-NN-Pytorch-scripts)
- [echen01/diffTLM](https://github.com/echen01/diffTLM) and [project page](https://echen01.github.io/vocal-tract/)
- [articulatory/articulatory](https://github.com/articulatory/articulatory)
- [STArK checkpoint card](https://huggingface.co/Lab-MSP/stark-large)
- [SPARC WaveNeXt card](https://huggingface.co/nzxyin/sparc-wavenext-24k)
- [PapayaResearch/vocaltrax](https://github.com/PapayaResearch/vocaltrax)
- [Simon-Stone/AreaFunctionSynthesis](https://github.com/Simon-Stone/AreaFunctionSynthesis)
- [kinoh/VoiceSynthesis](https://github.com/kinoh/VoiceSynthesis)
