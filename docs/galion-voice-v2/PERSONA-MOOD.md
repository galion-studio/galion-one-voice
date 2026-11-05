# Persona mood → persona-conditioned voice (PersonaPlex-style, Piper path)

Status: V2 sprint, local branch `swarm/wt16-premium-emotion` (not pushed).
Owner rulings applied: t641u (PersonaPlex-style design), reinstated by t644u. Piper stays the TTS engine.

## What it does

Each persona has its **own persistent mood**. The mood reacts to the user's recognised emotion.
The voice is conditioned on **both**: the persona's mood and the user's emotion.

```
user voice ──GatedDetector<LoudnessEstimator> (consent gate)──┐
user text  ──text_affect (lexicon, alloc-free)────────────────┤ UserObservation::fuse
                                                              ▼
                        appraise(persona profile, observation)  ← Samantha mirrors + softens
                                                              │   Jarvis steadies, stays composed
                                                              ▼
            PersonaMood: decay to rest (half-life) + inertia (lerp by reactivity·√conf)
                                                              ▼
            PersonaConditioning {persona, mood VAD, user VAD+conf, empathy e}   ← shared interface
                     │                                       │
                     ▼                                       ▼
   PiperVoiceProfile::prosody → PiperProsodyVector    future in-house dual-band decoder
   (length/noise/noise_w/silence at Piper spawn,      (PR #73 PersonaPlex re-anchor doc):
    pitch + energy + pre-pause post-synthesis)         consumes `to_array()` (dim 10, v1)
                     ▼
   A21WeightTts (ConditionedDecoder) → legacy Piper CLI → EmotiveShaper → PCM
```

## Code

| piece | where | notes |
|---|---|---|
| `Persona`, `PersonaProfile`, `SAMANTHA`, `JARVIS` | `galion-emotion/src/persona_mood.rs` | appraisal, mood dynamics, Piper voice profile per persona |
| `UserObservation::{from_frame, from_text, fuse}` | same | voice gives arousal (+ valence only if the estimator knows it); text gives valence; confidence `1−(1−cv)(1−ct)` |
| `text_affect` | same | ~50-word lexicon, negation flips, `!` raises arousal; confidence ≤ 0.6 |
| `PersonaMood::{observe, tick, conditioning}` | same | exponential decay to the persona's rest mood, user cue fades (8 s half-life) |
| `PersonaConditioning` + `ConditionedDecoder` trait | same | **the shared interface**. `CONDITIONING_VERSION = 1`, `to_array()` → `[is_samantha, is_jarvis, mood_v, mood_a, mood_d, user_v, user_a, user_d, user_conf, e]` |
| `PiperProsodyVector` | same | `length_scale, noise_scale, noise_w, sentence_silence_s, pitch_shift_st, energy, pre_pause_ms` |
| `EmotiveShaper`, `median_f0`, `active_rms_dbfs` | `crates/galion-ml-voice/src/emotive.rs` | streaming resample pitch shift + gain with soft knee; identity is bit-exact |
| `PiperProsody::from_vector`, per-line `speaker_id` | `crates/galion-ml-voice/src/speech/piper_worker.rs` | scales rounded to 2 dp so mood drift does not respawn Piper every turn |
| `A21WeightTts: ConditionedDecoder`, `prepare()`, `synthesize_pcm()`, `set_speaker()` | `crates/galion-ml-voice/src/speech/weights.rs` | **no plan ⇒ byte-identical V1 path** |
| listening set | `crates/galion-ml-voice/examples/emotion_sweep.rs` | persona × user emotion × mood history, measured F0/rate/RMS |

V1 building blocks reused: `EmotionFrame` (VAD + confidence), `empathy_weight` (`e`), `GatedDetector` + `DefaultConsentGate`
(no reading without consent), and the persona TOML `e0` register rows. A unit test pins each persona's resting voice to its
TOML `e0` row (`resting_voice_matches_persona_toml_e0`).

## Persona profiles

| | Samantha | Jarvis |
|---|---|---|
| rest mood (v, a, d) | 0.25, 0.35, 0.45 (warm) | 0.10, 0.28, 0.62 (steady) |
| reactivity / half-life | 0.45 / 20 s | 0.25 / 12 s |
| joy mirrored | 0.9 | 0.45 |
| sorrow mirrored (floor) | 0.45 (≥ −0.35) | 0.2 (≥ −0.12) |
| arousal mirrored (cap) | 0.7 (≤ 0.8) | 0.2 (≤ 0.5) |
| under user distress (v < −0.2) | calms arousal ×0.75, dominance → 0.4 (soft) | calms ×0.7, dominance → 0.68 (composed) |
| resting voice | ls 1.00, ns 0.667, nw 0.8, pitch 0, energy 1.00, pre-pause 0 | ls 1/0.96, ns 0.5, nw 0.6, pitch −0.3 st, energy 0.92, pre-pause 80 ms |
| pace gain per Δarousal | −0.40 | −0.15 |
| pitch gain per valence / Δarousal | 1.6 / 2.2 st | 0.6 / 0.8 st |
| energy gain per Δarousal | 0.45 | 0.20 |
| empathy (`e`=1): slower / pitch / energy / extra pause | +0.10 / −0.5 st / ×0.80 / +250 ms | +0.04 / −0.1 st / ×0.88 / +40 ms |
| bounds | ±3 st, ls 0.80–1.30, energy 0.60–1.20 | ±1.5 st, ls 0.92–1.18, energy 0.75–1.05 |

Mapping (`da = mood.arousal − 0.35`, `v = mood.valence`, `e` = V1 empathy weight of the **user** cue):

| knob | formula | applied |
|---|---|---|
| length_scale (pace) | `clamp(base·(1 + pace·da) + slow·e)` | Piper spawn arg (pitch-compensated: `ls·2^(st/12)`) |
| pitch shift (st) | `clamp(base + pv·v + pa·da + ep·e)` | post-synthesis resample (`EmotiveShaper`) |
| energy | `clamp(base·(1 + ea·da)·(1 + (ee−1)·e))` | post-synthesis gain, soft knee above 0.8 FS |
| noise_scale (tone variation) | `clamp(base + na·da + 0.05·v, 0.2, 0.9)` | Piper spawn arg |
| noise_w (rhythm variation) | `clamp(base + nwa·da, 0.2, 1.0)` | Piper spawn arg |
| sentence_silence | `clamp(base − 0.25·da, 0.08, 0.45)` | Piper spawn arg |
| pre-pause | `base + ep_ms·e` | silence before the first sample |

So **both inputs reach the voice**: the persona mood drives pace/pitch/energy/noise, and the user's own cue drives the empathy
terms (`e`) directly, even before the mood has moved. Test `both_inputs_affect_output` holds one fixed and varies the other.

## Hot path

`text_affect`, `fuse`, `observe`, `conditioning`, `piper()`, `to_array()` and `EmotiveShaper::process` allocate nothing
(`galion-emotion/tests/persona_mood_alloc.rs`: counting allocator, 10 000 iterations × 2 personas, 0 allocations).
The Piper side reuses one shaped-PCM buffer.

## Legacy Piper limits and how they are handled

* Scales are CLI args, read once at spawn. A plan change therefore **respawns** Piper (~model load). The plan is rounded to
  2 dp, and `A21WeightTts::prepare()` lets the caller respawn and warm while the user is still speaking, so the respawn is off
  the reply's critical path. Pitch and energy never respawn (post-synthesis).
* `speaker_id` is per JSON line (no respawn), so multi-speaker packs (VCTK, LibriTTS-R) work on the same worker.
* `galion-piper-rt` (ORT runtime, branch `swarm/lat240-streaming-tts`) has neither speaker id nor per-call scales yet.
  It needs both before it can run this path. This is a V2 blocker for the streaming runtime, not for the CLI path.

## Honest limits

* Voice emotion recognition is the real Rust path, `GatedDetector<LoudnessEstimator>`. It is consent-gated, gives
  arousal/dominance from loudness only, and reports **valence unknown**. Valence comes from text. No SER model is faked.
* The listening set uses synthetic user audio (Piper, loudness scaled per emotion), labelled as such.
* Expressiveness is bounded by Piper: it is prosody shaping, not acted emotion.

## Listening set (measured)

Full table: [`evals/emotion-sweep/SWEEP.md`](evals/emotion-sweep/SWEEP.md) and `sweep.csv`.
WAVs (box, not committed): `/workspace/swarm/voice-v2-sprint/results/emotion-sweep/`.
That is 36 persona cases, plus 2 V1 baselines, 2 resting references and 6 synthetic user clips.
Voices: Samantha `en_US-libritts_r-medium` sid 126; Jarvis `en_GB-vctk-medium` sid 95 (p226).
Every case speaks the same reply line, and metrics are means over 8 renders. Measured ranges vs each persona's resting reference:

| | Samantha | Jarvis |
|---|---|---|
| planned pitch shift | −0.53 … +1.25 st | −0.42 … −0.19 st (never above rest) |
| measured ΔF0 | −1.27 … +0.99 st | −0.47 … −0.02 st |
| planned length_scale | 0.947 … 1.103 | 1.048 … 1.081 |
| planned energy | 0.84 … 1.06 | 0.88 … 0.91 |
| measured ΔRMS | −1.5 … +1.2 dB | −1.1 … +0.3 dB |
| pre-pause | 0 … 142 ms | 80 … 103 ms |

* Samantha, happy user after joy: +1.0 st and about 8 % faster.
* Samantha, sad user after distress: −1.3 st, length 1.10, −1.5 dB, and a 142 ms pause.
* An angry user is *not* mirrored. Samantha softens (e = 0.49 ⇒ lower, slower, quieter). Jarvis stays at or below rest on
  pitch, pace and loudness, which matches `JARVIS.md` ("anger is answered by steadiness").
* The same user cue lands differently depending on mood history (rest / after-joy / after-distress). That is the
  persistent-mood part.
* Measured rate is noisy at about ±4 %, because VITS duration sampling is stochastic (noise_w). The planned length_scale is exact.
