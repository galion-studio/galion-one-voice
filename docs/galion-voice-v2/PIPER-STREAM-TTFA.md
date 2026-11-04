# Piper stream TTFA (t301 live demo)

`scripts/eval/piper-stream-ttfa.sh` prints one JSON object from a real Piper
CPU child (`GALION_SPEECH=a21`, repo `weights/runtime/piper/piper`, A21 pack
sha256-checked against `PINNED`). No stub clock.

| field | clock |
|---|---|
| `cold_first_pcm_ms` / `warm_first_pcm_ms` | SpeechStack via `scripts/bench-piper-warmth.sh` (speak entry -> first clause PCM) |
| `cold_spawn_first_pcm_byte_ms` | fresh `piper --output_raw --json-input`, spawn -> first stdout byte |
| `raw_spawn.warm_line_first_pcm_byte_ms` | second JSON line on the same child -> first byte |
| `drain` | rough demo: PCM resampled 22050 -> 24000, 1920-sample (80 ms) frames, 80 ms tick from the first byte over real arrival times; `late` tick = pause (`_` in `ticks`) |

Exit 0 only when `warm_first_pcm_ms` is measured; exit 2 (measured:false, no
ms fields) when the pack or runtime is missing. `--wav OUT` writes the drained
24 kHz audio for listening — never commit WAVs.

Relates to #54 (stream TTFA) and #55 (80 ms PCM drain).

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
