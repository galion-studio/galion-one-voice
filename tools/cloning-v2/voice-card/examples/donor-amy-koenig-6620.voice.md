# Voice card: CC0 donor: Amy Koenig (LibriVox reader 6620)

Stage 0 pre-clone analysis of `amy-koenig-6620-ref.wav` (9.6 s, 44100 Hz, 16-bit, 1 ch). Standard galion-voice-v2-432.

Pitch centres on 177 Hz (F#3 -45 cents at A4=432 Hz) with moderate intonation: 3.1 st SD, 9.3 st p5-p95 range. Pace is moderate (4.41 syllables/s while speaking) and fluent: 13.4 pauses/min, 12% of the time silent. Dynamics: steady, even loudness (LRA 2.9 LU, frame range 28.1 dB). Emotion baseline (emotion2vec+): neutral on the whole clip; 3 s windows {"neutral":5}. Room: noise floor -59.7 dBFS, SNR about 34 dB, RT60 not measurable.

| Area | Key numbers |
|---|---|
| Pitch | median 176.966 Hz (F#3 -45.071 cents, A4=432), p5-p95 145.438-248.519 Hz (9.275 st), SD 3.056 st |
| Timbre | F1/F2/F3 556.591/1534.57/2800.466 Hz, tilt -2.774 dB/oct, f99 8204.2 Hz, 3.30% of energy above 4 kHz |
| Tempo | 4.408 syl/s articulation, 3.902 syl/s overall, 13.378 pauses/min, median pause 0.515 s, pause ratio 0.115 |
| Voicing | jitter 3.006%, shimmer 9.519%, HNR proxy 12.491 dB |
| Loudness | -22.532 LUFS (mono), true peak -4.119 dBTP, LRA 2.89 LU |
| Articulation | fricative centroid 5545.544 Hz, sibilant 6302.452 Hz, burst 3650.11 Hz, VOT 42.5 ms |
| Emotion | whole clip: neutral; windows: {"neutral":5} |
| Environment | noise floor -59.655 dBFS, SNR ~34.452 dB, RT60 unmeasured, dual-mono n/a (mono file), clipped 0, refcheck FAIL |

## Limits
- Noise floor -59.7 dBFS is above the -60 dBFS capture target.
- refcheck sample_rate: 44100.00 (limit 48000 Hz)
- refcheck bit_depth: 16.00 (limit 24 pcm_integer bit)
- refcheck integrated_lufs: -22.53 (limit -20.0 ± 2.0 LUFS)
- refcheck noise_floor: -59.66 (limit <= -60 dBFS)
- refcheck reverb: n/a (limit <= 0.40 s)
- refcheck spectral_tilt: -2.77 (limit -6.0 ± 2.0 dB/octave)

## Prep before cloning
- refprep HF shelf off (f99 8204 Hz >= trigger 3500 Hz): no HF restoration needed.
- refprep denoise gate ON (frame p90/p10 38.3 dB vs skip-at 45 dB).
- Loudness to -23 LUFS, true peak <= -1.0 dBTP; resample to 24000 Hz; select mode ecapa_consistent.
- Capture spec (48 kHz / 24-bit mono) is not met by this file; a re-record is the real fix, upsampling adds nothing.

## Not measured
- `tempo.words_per_s`: needs a transcript; no ASR brick is wired into stage 0 yet (galion-asr-vosk is the candidate)
- `voicing.breath_events`: no breath detector brick yet; band-limited sources also hide breath noise
- `environment.rt60_s`: refcheck: dynamic range is under 30 dB, so the tail is not visible
- `emotion.gate_scores`: pending: the per-emotion 0.83 gate score compares a render with that emotion's reference clip of the same voice; a single reference only gives the baseline posterior above

Consent: LibriVox 'Love and Friendship' track 1 (lovefriendship_bronte_aak.mp3), archive.org Public Domain Mark 1.0; cut 10.55-20.15 s as in docs/galion-voice-v2/CLONE-CC0-DONORS.md (not checked by stage 0; galion-consent gates cloning).
