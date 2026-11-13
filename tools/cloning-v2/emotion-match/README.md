# emotion_match

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

`emotion_match = 0.70*category_blend+0.30*emb_sim`. Those weights are locked.
The Emotion V2 release bar is **0.83**, also locked, and it is applied **per
sector**. One weak sector cannot hide inside a blended float. This tool will
not lower the gate.

The historical `--ref REF.wav clip.wav…` path still compares every render to
**one** mixed reference. That is the laptop series in
`docs/emotion-v2/EMOTION-V2-FOUNDATION-SPEC.md` §7, and it is why Samantha
capped near 0.49 and Jarvis near 0.807: both voices were scored against a
single clip. Use `--manifest` for the V2 gate.

## Per-sector mode

```bash
python tools/cloning-v2/emotion-match/emotion_match.py \
  --manifest /path/to/sector-refs.json \
  --renders-dir /path/to/renders \
  --backend stub \
  --out /tmp/emotion-match.json
```

`--backend stub` is the CPU path: deterministic sine-fixture posteriors, no
weights, no torch, no network. `--backend superb` uses the local
`superb/wav2vec2-base-superb-er` snapshot (`GALION_EMOTION_MODEL` or the
default laptop cache). It does not download.

`--renders-dir` has one subdirectory per emotion (`neutral` or `Neutral`, and
the other eleven, plus `seductive` only when Samantha's overlay is in the
run). You can also list lines in the manifest, or pass clip paths on the
command line (`happy/s01.wav`, `happy-s01.wav`).

Exit status: `0` when every scored sector is at or above 0.83, `1` when a
score fails the gate, `2` when the manifest, a reference file, or a sha256
pin is unusable. A missing sector reference is a hard failure. The tool will
not fall back to a single mixed clip.

## Manifest

Schema `galion-emotion-v2-sector-refs/1`. Paths are relative to `--root`, or
to the manifest's directory when `--root` is omitted. If `gate`, `formula`,
or `weights` are present they must match the locked values.

```json
{
  "schema": "galion-emotion-v2-sector-refs/1",
  "persona": "samantha",
  "gate": 0.83,
  "formula": "0.70*category_blend+0.30*emb_sim",
  "weights": {"category_blend": 0.70, "emb_sim": 0.30},
  "references": {
    "neutral": {
      "path": "refs/neutral.wav",
      "sha256": "<sha256 of that capture-kit reference>"
    }
  },
  "lines": [
    {"id": "neutral-s01", "emotion": "neutral", "path": "renders/neutral/s01.wav"}
  ]
}
```

The twelve shared sectors are Neutral, Calm, Happy, Amused, Excited, Tender,
Sad, Disappointed, Angry, Fearful, Surprised, and Disgusted. `seductive` is
Samantha-only and is required only when a scored line uses it.

## What this does not include

Real per-sector reference clips. Those come from the capture kit in
`docs/emotion-v2/CAPTURE-PROTOCOL.md` (five takes per sector, owner's
consent, Slavik's too if a take is used for training). They are not in git,
and this directory's `.gitignore` refuses audio and weights.

The report JSON has a per-sector object and an aggregate (`emotion_match` is
the mean, `min_emotion_match` is the weakest sector, `pass` is true only
when every sector clears 0.83). Scoring against real capture-kit references
is the next step, on a machine that already holds those WAV files.

## Tests

CPU stub and sines only. No committed audio.

```bash
python -m unittest discover -s tools/cloning-v2/emotion-match/tests -v
```
