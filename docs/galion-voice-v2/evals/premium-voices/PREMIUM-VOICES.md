# Premium Piper voices for Samantha and Jarvis (V2 sprint)

Owner ruling: Piper stays the engine. Only the voice model changes. Models are downloaded to
`/workspace/swarm/voice-v2-sprint/models/piper-premium/` (box). They are not committed: they are weights, see `.gitignore`.
Every file is verified against the Hugging Face LFS sha256, or for `.onnx.json` the git blob sha1. See `SHA256-VERIFY.txt`.
The baseline `en_US-ljspeech-medium` matches the PINNED sha `6f52a751…`.

## Method

* **Runtime:** legacy `piper` CLI 2023.11.14-2, the same binary for every candidate. It supports per-line `speaker_id`.
* **Lock:** the whole run was held under `flock latency-240/BENCH.lock`, quality phase included. Load average on the 8 vCPU
  box was 4.5 before and 6.8 after (other agents' unlocked jobs). Run date: 2026-11-04 21:56 CET.
* **Warm TTFA:** one persistent process, 3 warm-ups, then 20 × "Sure, I can help.". TTFA is the time from the stdin write to
  the first stdout PCM bytes. Unpinned, as in `latency-240/01-streaming-tts.md`. The legacy CLI emits a sentence only once
  it is complete, so TTFA ≈ short-sentence synthesis time. Multi-speaker models were timed at sid 0.
* **RTF 1 core:** `taskset -c 7`, median of 4 runs of a 2-clause sentence of about 9 s. RTF = wall time / audio duration.
* **Quality proxy:** torchaudio 2.6 `SQUIM_OBJECTIVE`. This is reference-free STOI, PESQ and SI-SDR at 16 kHz, averaged over
  4 sentences. **UTMOS and DNSMOS are not available offline on the box (none).** SQUIM estimates signal quality, not
  naturalness. Its spread across Piper voices is small, so it is used as a gate, not a ranking.
* **Speaker screen:** median F0 for every VCTK and ARU speaker, plus 60 LibriTTS-R and 40 LibriTTS speakers (seed 7), from
  `screen.py` → `screen-*.tsv`. Then pyin F0 median and the p10–p90 range of the first quality sentence (`pyin.txt`).
* **Licence filter:** dataset licence must be CC0, CC BY, MIT or public domain. Excluded models:
  * lessac: Blizzard licence
  * ryan, hfc_female, hfc_male, semaine: CC BY-NC-SA
  * l2arctic: CC BY-NC
  * northern_english_male, southern_english_female: CC BY-SA, not on the allowlist
  * alan, amy, jenny_dioco, kusal, danny, arctic: "see URL/LICENSE", unclear
  * sam: Apache-2.0, permissive, but not tested this round
* **No celebrity voices.** All speakers are anonymous corpus readers.

## Candidates

| persona | model (Piper) | speaker (sid) | speaker info | dataset licence | size MB | warm TTFA p50 / p95 ms | RTF 1 core | SQUIM PESQ | SQUIM STOI | SQUIM SI-SDR dB | F0 median Hz | F0 p10–p90 st |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| samantha | en_US-ljspeech-medium | - | single speaker | LJ Speech — public domain | 63.5 | 43 / 59 | 0.057 | 3.70 | 0.9935 | 26.1 | 204 | 10.1 |
| samantha | en_US-ljspeech-high | - | single speaker | LJ Speech — public domain | 114.2 | 119 / 138 | 0.204 | 3.86 | 0.9944 | 24.9 | 188 | 8.4 |
| samantha | en_GB-cori-high | - | single speaker | LibriVox — public domain | 114.2 | 140 / 168 | 0.212 | 3.81 | 0.9933 | 26.4 | 202 | 6.9 |
| samantha | en_US-kristin-medium | - | single speaker | LibriVox — public domain | 63.5 | 45 / 52 | 0.054 | 3.93 | 0.9964 | 28.8 | 152 | 5.9 |
| samantha | en_GB-alba-medium | - | single speaker | Edinburgh 10283/3270 — CC BY 4.0 | 63.2 | 52 / 79 | 0.063 | 4.03 | 0.9983 | 28.8 | 205 | 7.1 |
| samantha | en_GB-vctk-medium | p225 (107) | F, English, Southern England | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 4.00 | 0.9987 | 27.6 | 166 | 19.8 |
| samantha | en_GB-vctk-medium | p228 (90) | F, English, Southern England | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 4.04 | 0.9983 | 27.2 | 184 | 5.2 |
| samantha | en_GB-vctk-medium | p229 (85) | F, English, Southern England | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 4.01 | 0.9989 | 28.3 | 177 | 2.3 |
| samantha | en_GB-vctk-medium | p231 (15) | F, English, Southern England | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 3.78 | 0.9991 | 25.0 | 176 | 3.9 |
| samantha | en_GB-vctk-medium | p257 (18) | F, English, Southern England | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 3.58 | 0.9964 | 25.7 | 203 | 5.2 |
| samantha | en_GB-vctk-medium | p268 (66) | F, English, Southern England | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 3.94 | 0.9979 | 28.9 | 198 | 6.1 |
| samantha | en_US-libritts_r-medium | 2882 (185) | LibriTTS reader | LibriTTS-R (OpenSLR 141) — CC BY 4.0 | 78.6 | 32 / 36 | 0.054 | 3.91 | 0.9951 | 26.8 | 181 | 5.5 |
| samantha | en_US-libritts_r-medium | 288 (126) | LibriTTS reader | LibriTTS-R (OpenSLR 141) — CC BY 4.0 | 78.6 | 32 / 36 | 0.054 | 4.09 | 0.9980 | 28.2 | 178 | 6.0 |
| samantha | en_US-libritts_r-medium | 3118 (570) | LibriTTS reader | LibriTTS-R (OpenSLR 141) — CC BY 4.0 | 78.6 | 32 / 36 | 0.054 | 3.91 | 0.9941 | 28.3 | 170 | 4.9 |
| samantha | en_US-libritts-high | p3989 (226) | LibriTTS reader | LibriTTS (OpenSLR 60) — CC BY 4.0 | 136.7 | 118 / 154 | 0.207 | 3.55 | 0.9929 | 21.5 | 184 | 5.3 |
| samantha | en_US-libritts-high | p7245 (154) | LibriTTS reader | LibriTTS (OpenSLR 60) — CC BY 4.0 | 136.7 | 118 / 154 | 0.207 | 3.93 | 0.9972 | 25.8 | 185 | 4.1 |
| samantha | en_US-libritts-high | p5604 (579) | LibriTTS reader | LibriTTS (OpenSLR 60) — CC BY 4.0 | 136.7 | 118 / 154 | 0.207 | 4.21 | 0.9978 | 27.5 | 189 | 5.9 |
| samantha | en_US-libritts-high | p3003 (63) | LibriTTS reader | LibriTTS (OpenSLR 60) — CC BY 4.0 | 136.7 | 118 / 154 | 0.207 | 3.75 | 0.9952 | 26.8 | 179 | 6.1 |
| jarvis | en_GB-vctk-medium | p226 (95) | M, English, Surrey | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 3.98 | 0.9982 | 26.7 | 104 | 6.3 |
| jarvis | en_GB-vctk-medium | p232 (60) | M, English, Southern England | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 3.82 | 0.9976 | 28.2 | 103 | 6.9 |
| jarvis | en_GB-vctk-medium | p243 (81) | M, English, London | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 3.82 | 0.9982 | 27.2 | 113 | 4.6 |
| jarvis | en_GB-vctk-medium | p254 (76) | M, English, Surrey | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 4.06 | 0.9995 | 28.2 | 76 | 6.4 |
| jarvis | en_GB-vctk-medium | p258 (57) | M, English, Southern England | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 3.96 | 0.9992 | 28.4 | 114 | 3.4 |
| jarvis | en_GB-vctk-medium | p274 (10) | M, English, Essex | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 3.97 | 0.9988 | 28.6 | 92 | 5.0 |
| jarvis | en_GB-vctk-medium | p278 (64) | M, English, Cheshire | VCTK 0.92 (Edinburgh 10283/3443) — CC BY 4.0 | 77.0 | 36 / 53 | 0.055 | 4.04 | 0.9978 | 28.6 | 103 | 5.3 |
| jarvis | en_GB-aru-medium | 02 (9) | ARU speaker | ARU corpus (Liverpool) — CC BY 4.0 | 76.8 | 51 / 66 | 0.056 | 3.94 | 0.9980 | 29.1 | 90 | 5.8 |
| jarvis | en_GB-aru-medium | 01 (3) | ARU speaker | ARU corpus (Liverpool) — CC BY 4.0 | 76.8 | 51 / 66 | 0.056 | 3.72 | 0.9968 | 24.3 | 103 | 7.2 |
| jarvis | en_GB-aru-medium | 05 (7) | ARU speaker | ARU corpus (Liverpool) — CC BY 4.0 | 76.8 | 51 / 66 | 0.056 | 3.85 | 0.9971 | 24.9 | 106 | 5.4 |
| jarvis | en_GB-aru-medium | 12 (8) | ARU speaker | ARU corpus (Liverpool) — CC BY 4.0 | 76.8 | 51 / 66 | 0.056 | 3.88 | 0.9980 | 26.1 | 113 | 6.2 |
| jarvis | en_GB-aru-medium | 10 (2) | ARU speaker | ARU corpus (Liverpool) — CC BY 4.0 | 76.8 | 51 / 66 | 0.056 | 3.89 | 0.9973 | 27.7 | 127 | 7.9 |
| jarvis | en_GB-aru-medium | 06 (1) | ARU speaker | ARU corpus (Liverpool) — CC BY 4.0 | 76.8 | 51 / 66 | 0.056 | 3.69 | 0.9933 | 23.8 | 135 | 5.6 |
| jarvis | en_US-norman-medium | - | single speaker | LibriVox — public domain | 63.5 | 44 / 56 | 0.059 | 4.02 | 0.9983 | 29.4 | 108 | 12.8 |
| jarvis | en_US-john-medium | - | single speaker | LibriVox — public domain | 63.5 | 56 / 79 | 0.055 | 3.89 | 0.9977 | 24.2 | 100 | 7.8 |
| jarvis | en_US-joe-medium | - | single speaker | OHF voice-datasets — CC0 | 63.2 | 42 / 57 | 0.056 | 3.83 | 0.9970 | 27.6 | 90 | 6.5 |
| jarvis | en_US-libritts_r-medium | 1748 (564) | LibriTTS reader | LibriTTS-R (OpenSLR 141) — CC BY 4.0 | 78.6 | 32 / 36 | 0.054 | 4.06 | 0.9971 | 27.4 | 101 | 7.9 |
| jarvis | en_US-libritts_r-medium | 7833 (296) | LibriTTS reader | LibriTTS-R (OpenSLR 141) — CC BY 4.0 | 78.6 | 32 / 36 | 0.054 | 4.20 | 0.9961 | 27.8 | 102 | 7.2 |

## Choice

| persona | voice | why |
|---|---|---|
| **Samantha** (warm female) | `en_US-libritts_r-medium`, speaker **288** (sid 126) — LibriTTS-R, CC BY 4.0 | Best SQUIM PESQ of every *medium* female candidate (4.09, SI-SDR 28.2). Mid F0 of 178 Hz with a 6 st range: warm and expressive, not shrill. The baseline ljspeech-medium is 204 Hz with a 10 st range. Fastest model on the box: TTFA p50 32 ms / p95 36 ms, RTF 0.054 on 1 core, 78.6 MB. No accent claim, which matches `samantha.toml` (`accent_claim = "none"`). |
| **Jarvis** (refined, British-leaning male) | `en_GB-vctk-medium`, speaker **p226** (sid 95) — VCTK 0.92, CC BY 4.0 | English male from Surrey (Home Counties), the closest VCTK reader to the intended register. PESQ 3.98, STOI 0.998, F0 104 Hz with a controlled 6.3 st range. TTFA p50 36 ms / p95 53 ms, RTF 0.055 on 1 core, 77.0 MB. |

Runners-up:
* Samantha: VCTK **p228** (UK female, PESQ 4.04, F0 184 Hz).
* Jarvis: VCTK **p278** (Cheshire, PESQ 4.04, F0 103 Hz) and **p258** (Southern England; 3.4 st range is rather flat).

Rejected:
* VCTK **p254**: best STOI, but F0 76 Hz with p10 at 60 Hz means creak.
* All `-high` models (ljspeech-high, cori-high, libritts-high): **3.7× slower** (RTF ~0.21 on 1 core, TTFA p50 118–140 ms)
  with no proxy gain that would pay for it. They do not fit the 240 ms budget once ASR and LLM are added.

**Caveats:**
* Nobody has listened yet. The choice is metric + metadata only. The listening WAVs are in
  `/workspace/swarm/voice-v2-sprint/results/piper-premium/samples/`.
* **CC BY 4.0 requires attribution** (VCTK: CSTR Edinburgh; LibriTTS-R: Google / OpenSLR 141). It must be added to the
  credits/NOTICE before ship.
* `jarvis.toml` says `accent_claim = "intended-not-checkpoint"` and `JARVIS.md` says the checkpoint has no British accent.
  With VCTK p226 the checkpoint *does* carry an English accent, so those lines need an owner-approved edit if this is adopted.
  They were not changed here.
* `galion-piper-rt` (ORT runtime, other branch) has no `speaker_id` yet. Both chosen voices are multi-speaker packs.

## Respawn cost (a scale change on the legacy CLI)

`respawn.txt`, under BENCH.lock: spawning, loading the model and synthesising "Okay." takes a median of
**300 ms** for libritts_r-medium, **330 ms** for vctk-medium and 396 ms for ljspeech-medium.
That is more than the whole 240 ms budget. So a plan change must respawn *before* the reply is needed: `A21WeightTts::prepare()`
runs while the user is still speaking. Otherwise the per-call scales have to come to `galion-piper-rt`.
