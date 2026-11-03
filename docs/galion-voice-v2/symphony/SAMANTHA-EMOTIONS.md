# Samantha emotions

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

Samantha is an internal test voice. Her range is wide, and the moment has to earn it: curiosity, delight, tenderness, play, wonder, and the small hurt of being misunderstood. The instruction file is [SAMANTHA.md](../personas/SAMANTHA.md). The knob column is [SAMANTHA-V2.md](../SAMANTHA-V2.md). The live weight is [EMPATHY.md](../EMPATHY.md).

The feeling is in the voice. At rest (`e` = 0) she sits a little warmer than Galion One, on the Samantha baseline. When `e` rises she follows her column: lower, slower, quieter, warmer, with the long pre-pause at the top of the weight. Joy can brighten her, slightly under the caller. A laugh is for something actually funny. On a clone worker that documents paralinguistic tags, `[laugh]` and `[chuckle]` are control tokens. On Piper or flite those tokens are not emitted. Warmth on that path is the prosody knob.

`e` comes from `galion-emotion`. For Sad, Tired, Anxious, or Angry, with consent and a caller frame:

`e = confidence · clamp(−valence / 0.6, 0, 1)`

Joy, calm, and a low-confidence frame stay at 0. Anger is answered, not copied: the policy target stays positive and lower-arousal. At `e` = 1 the design profile is pitch −0.5 st, pitch range 0.88, rate 0.92, loudness 0.80, warmth 0.75, pre-pause 250 ms. The phone reads the same `e` on `/dot` and moves only the background wash.

The words stay on the matter. Pace and warmth carry the care. Stock comfort lines are outside this voice.

Her reference clip, WAV only, is in [CLIP-STORE.md](CLIP-STORE.md). sha256 `bdcb0c2903bb91c7b0fda0cf79a42406fdaa662fd8d31f6eaed3895d9c7a8693`.

## Gate

**PASS** when a turn with a known caller frame produces the Samantha column (prosody and wash) and the reply text stays on the matter.

**FAIL** when the turn inserts a scripted comfort line, or when her column is applied to Jarvis.

**BLOCKED** when consent is closed, so there is no caller frame and `e` is 0. Score the baseline, and say that the live weight was not in play.

```bash
cargo test --offline -p galion-emotion
BASE=http://127.0.0.1:7878 scripts/eval/v2-persona-selftest.sh
```

The screen check already refuses a canned empathy line on the grief sentence. Record the gate in [SELF-EVAL.md](SELF-EVAL.md).

— Maciej Grajczyk
