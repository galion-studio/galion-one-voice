# Voice card: Samantha (Galion Voice V2 reference)

Stage 0 pre-clone analysis of `Samantha-sent-2026-11-05.wav` (17.407 s, 44100 Hz, 16-bit, 2 ch). Standard galion-voice-v2-432.

Pitch centres on 176 Hz (F3 +48 cents at A4=432 Hz) with moderate intonation: 3.0 st SD, 9.5 st p5-p95 range. Pace is moderate (5.41 syllables/s while speaking) and fluent: 24.9 pauses/min, 23% of the time silent. Dynamics: steady, even loudness (LRA 1.9 LU, frame range 28.8 dB). Emotion baseline (emotion2vec+): neutral on the whole clip; 3 s windows {"angry":1,"fearful":1,"neutral":8}. Band-limited: 99% of energy below 2670 Hz (0.41% above 4 kHz), so breath, sibilance and air are mostly missing. Room: noise floor -53.4 dBFS, SNR about 32 dB, RT60 not measurable.

| Area | Key numbers |
|---|---|
| Pitch | median 176.236 Hz (F3 47.773 cents, A4=432), p5-p95 154.774-267.952 Hz (9.502 st), SD 3.045 st |
| Timbre | F1/F2/F3 464.325/1749.881/2670.311 Hz, tilt -4.53 dB/oct, f99 2670.1 Hz, 0.40% of energy above 4 kHz |
| Tempo | 5.405 syl/s articulation, 4.142 syl/s overall, 24.852 pauses/min, median pause 0.47 s, pause ratio 0.234 |
| Voicing | jitter 2.993%, shimmer 8.186%, HNR proxy 13.084 dB |
| Loudness | -20.682 LUFS (mono), true peak -1.916 dBTP, LRA 1.93 LU |
| Articulation | fricative centroid 4146.527 Hz, sibilant 5044.111 Hz, burst 2756.001 Hz, VOT 51.25 ms |
| Emotion | whole clip: neutral; windows: {"angry":1,"fearful":1,"neutral":8} |
| Environment | noise floor -53.394 dBFS, SNR ~32.202 dB, RT60 unmeasured, dual-mono true, clipped 0, refcheck FAIL |

## Limits
- Band-limited: 99% of energy below 2670 Hz (0.41% above 4 kHz), so breath, sibilance and air are mostly missing.
- Noise floor -53.4 dBFS is above the -60 dBFS capture target.
- Stereo file is dual-mono (L/R correlation 1.0000); no stereo information, downmix to mono loses nothing.
- refcheck sample_rate: 44100.00 (limit 48000 Hz)
- refcheck bit_depth: 16.00 (limit 24 pcm_integer bit)
- refcheck channels: 2.00 (limit 1 ch)
- refcheck duration: 17.41 (limit 3.0..12.0 s)
- refcheck noise_floor: -53.39 (limit <= -60 dBFS)
- refcheck reverb: n/a (limit <= 0.40 s)

## Prep before cloning
- refprep HF shelf ON (f99 2670 Hz < trigger 3500 Hz): +2.5 dB at 2800 Hz, exciter mix 0.08. Restored highs are estimated, not recovered.
- refprep denoise gate ON (frame p90/p10 35.5 dB vs skip-at 45 dB).
- Loudness to -23 LUFS, true peak <= -1.0 dBTP; resample to 24000 Hz; select mode ecapa_consistent.
- Downmix to mono first (dual-mono).
- Cut into 3-12 s takes at word gaps for Emotion V2 references.
- Capture spec (48 kHz / 24-bit mono) is not met by this file; a re-record is the real fix, upsampling adds nothing.
- Best fix: re-record with full bandwidth and a phonetically balanced multi-clip set (capture kit).

## Not measured
- `tempo.words_per_s`: needs a transcript; no ASR brick is wired into stage 0 yet (galion-asr-vosk is the candidate)
- `voicing.breath_events`: no breath detector brick yet; band-limited sources also hide breath noise
- `environment.rt60_s`: refcheck: dynamic range is under 30 dB, so the tail is not visible
- `emotion.gate_scores`: pending: the per-emotion 0.83 gate score compares a render with that emotion's reference clip of the same voice; a single reference only gives the baseline posterior above

Consent: Samantha.wav, Slavik's own recording; local zero-shot conditioning only; no signed consent_scope training-data-generation yet (not checked by stage 0; galion-consent gates cloning).
