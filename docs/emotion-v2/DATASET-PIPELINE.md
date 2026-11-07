# Emotion V2 dataset pipeline

Generic training-data builder for the twelve Emotion V2 labels. It lives in
`tools/emotion-v2/dataset` and is Python, on the same side of the repo as the
Chatterbox worker and the pipeline tests. It does not link into the Rust
symphony crates, and it does not touch deployed voices, `SOUL.md`,
`config.yaml`, or `LOCKS.toml`.

The default backend renders a synthetic harmonic stack. It does not read a
reference recording and it does not clone a person. Tests use that stub, plus
sines and noise.

## What a run produces

A run writes one directory:

```text
datasets/emotion-v2/<voice_id>/<run_id>/
  manifest.jsonl          generic training manifest, one clip per line
  rejected.jsonl          lines the quality gate dropped (no audio)
  run.json                pitch standard, licences, plugin sources, counts
  audio/<emotion>/*.wav   augmented clips (PCM 16-bit)
  personaplex/train.jsonl Moshi-format manifest (path, duration)
  personaplex/eval.jsonl  held-out split (empty unless --eval-fraction)
  personaplex/prompts.jsonl   side metadata, not the trainer manifest
  personaplex/stereo/*.wav    24 kHz stereo, left = line, right = silence
  personaplex/stereo/*.json   word alignments for the left channel
```

`manifest.jsonl` fields, in order:

| Field | Meaning |
|---|---|
| `audio_path` | Path relative to the run directory |
| `text` | Scenario line |
| `emotion` | One of the twelve names below |
| `intensity` | 0 to 1, from the scenario bank |
| `environment` | Environment name from the matrix |
| `snr_db` | Noise level in dB. `null` means dry, no noise added |
| `voice_id` | Profile id |
| `scores` | `wer`, `ecapa`, `emotion`. `null` when that scorer is absent |
| `sha256` | SHA-256 of the wav bytes |

Further keys record the sample rate, duration, the 432 Hz pitch centre, the
backend, whether the signal is synthetic, the scenario id, the
impulse-response and noise source plus licence, and `telephony_hz` (`8000`
for the phone environment, otherwise `null`). Scores are measured on the dry
render. Environment and SNR describe the augmented file.

The twelve emotions, in dataset order:

Neutral, Calm, Happy, Amused, Excited, Tender, Sad, Disappointed, Angry,
Fearful, Surprised, Disgusted.

## Where files go

The output root defaults to `datasets/emotion-v2/` at the repo root. The run
directory is `datasets/emotion-v2/<voice_id>/<run_id>/`. Override it with
`--output-root`. That default directory is git-ignored, and so are wav, flac,
mp3, and ogg files under `tools/emotion-v2/`. Do not commit clips, weights,
or consent records for a real person.

The tool refuses to write inside the checkout anywhere except
`datasets/emotion-v2/`. A temporary directory outside the checkout is
accepted, and that is what the tests use.

## How to run the stub

Requires Python 3.11+ and numpy. From the repo root, with
`tools/emotion-v2/dataset` on `PYTHONPATH`:

```bash
PYTHONPATH=tools/emotion-v2/dataset \
  python3 -m emotion_v2_dataset run \
  --run-id demo \
  --emotions "Neutral|Happy" \
  --environments "dry studio|small room" \
  --max-scenarios-per-emotion 1
```

`--profile` defaults to `tools/emotion-v2/dataset/data/stub_profile.json`
(`voice_id` `stub-sine`, synthetic, pitch centre 216 Hz on the 432 Hz
standard). `--scenarios` defaults to `data/scenarios.json`, two original
lines per emotion.

Nothing is downloaded. A full nominal matrix is 12 emotions × 2 lines × 8
environments = 192 clips. Pass `--max-scenarios-per-emotion` and
`--environments` to take a slice.

## How to add a voice

1. Write a profile JSON. Required fields:

   ```json
   {
     "voice_id": "studio-fixture",
     "kind": "real",
     "pitch_standard_hz": 432,
     "pitch_centre_hz": 180,
     "references": {
       "Neutral": "references/Neutral",
       "Calm": "references/Calm"
     }
   }
   ```

   `kind` is `synthetic` or `real`. `pitch_standard_hz` must be `432`.
   `pitch_centre_hz` is that voice's fundamental on the 432 Hz standard
   (A4 = 432). A speaking fundamental may sit near 440 Hz; that is not a
   retune. Any `retune_hz`, `retune_to_hz`, or `target_a4_hz` is refused
   (a target of 440 Hz is the concert-pitch case). `a4_hz`, if set, must be
   432. Emotion pitch offsets are cents relative to `pitch_centre_hz`
   (`2^(cents/1200)`). The ratio `440/432` is never applied.

   `references` maps each emotion name to a directory path. The path is
   stored and is not opened. A `real` profile must name all twelve emotions
   after the capture plugin has had a chance to fill gaps. A `synthetic`
   profile may omit them. The stub backend does not read those directories.

2. For a real voice, or for `--backend chatterbox`, write a consent JSON and
   pass `--consent`. Required fields:

   * `voice_id` — same id as the profile
   * `owner` — who granted consent
   * `consent_scope` — a list that includes `training-data-generation`
   * `signed_date` — `YYYY-MM-DD`, not in the future

   Placeholders (`todo`, `tbd`, `unknown`, …) are refused. The clone-time
   scope `local-zero-shot-conditioning` does not authorize this pipeline.
   A voice id containing `armstrong` is refused, matching the existing
   celebrity block. An example for the synthetic stub id is
   `data/consent.example.json`. It is not a person's consent.

3. Pick a backend.

   * `stub` (default) emits sines and noise. A synthetic profile does not
     need consent. A real profile still needs the consent record, and the
     stub still emits a synthetic signal. The manifest sets
     `synthetic_signal` true and `render_backend` to `stub`, so the file is
     not presented as a clone.
   * `chatterbox` is the real-voice slot. Consent is checked before any
     engine is called. This package does not import Chatterbox, does not
     download weights, and the CLI has no engine, so `--backend chatterbox`
     refuses even with a valid consent file. An operator can pass an engine
     object to `ChatterboxBackend` from Python after the gate passes. This
     tree does not do that, and the tests inject a sine generator only.

4. Run:

   ```bash
   PYTHONPATH=tools/emotion-v2/dataset \
     python3 -m emotion_v2_dataset run \
     --profile /path/to/voice.json \
     --consent /path/to/consent.json \
     --backend stub \
     --run-id 20261107 \
     --today 2026-11-07
   ```

5. Leave the run directory untracked. Copy or upload it from outside git.

## Pipeline steps

1. Load the profile, the scenario bank, and the environment list. Resolve
   optional plugins (below). Apply the pitch-standard gate.
2. Render each selected line with the backend, dry, at 24 kHz.
3. Quality filter. WER, ECAPA, and emotion scorers run only when a scorer
   object is configured or the CI plugin provides one. A missing scorer
   leaves that score `null` and does not reject the line, unless
   `--require-scorers` is set. Default thresholds, applied only to scores
   that exist: WER above 0.35, ECAPA below 0.50, emotion match below 0.40.
   Rejected lines go to `rejected.jsonl` and are not augmented.
4. Augment each accepted line across the environment matrix.
5. Write wavs, `manifest.jsonl`, and the PersonaPlex export.

## Environments, sources, and licences

Augmentation is post-render. Impulse responses and noise are synthesized in
this process. No room library and no noise recording is downloaded.

| Environment | What is applied | Nominal SNR |
|---|---|---|
| dry studio | Unit impulse, no noise | dry (`snr_db` null) |
| small room | Short synthetic room, filtered noise | 25 dB |
| living room | Longer synthetic room, filtered noise | 20 dB |
| car | Short cabin-like decay, low rumble | 12 dB |
| cafe/babble | Longer decay, detuned tones plus noise (not speech) | 8 dB |
| street | Short decay, gust-like noise | 10 dB |
| phone/narrowband 8 kHz | Mild decay, line noise, then 300–3400 Hz brickwall and decimation to 8 kHz | 15 dB |
| headset/bluetooth codec | Very short decay, hiss, 80–7500 Hz brickwall, ITU-T G.711 mu-law | 18 dB |

Each environment also lists extra SNR levels in code (`snr_levels`). The
default run uses the nominal level only. `--snr 20|10|5` replaces the
nominal level on the noisy environments. Dry studio stays dry.

Source and licence recorded on every manifest row and in `run.json`:

* Impulse: in-process direct path, sparse early taps, exponential RT60
  decay. Licence `CC0-1.0`. Dry studio uses an in-process unit impulse,
  same licence.
* Noise: in-process filtered noise and detuned tones. Licence `CC0-1.0`.
  Dry studio records noise source `none`.
* Phone: in-process FFT brickwall and integer decimation. Not a carrier
  recording and not a downloaded telephony codec.
* Headset: in-process brickwall plus an ITU-T G.711 mu-law round trip,
  which is an open specification. It is not a Bluetooth SBC bitstream and
  not a vendor codec.

## PersonaPlex export

NVIDIA has not published a fine-tune file format.

* The README documents inference, voice prompts, and text role prompts only:
  <https://github.com/NVIDIA/personaplex/blob/main/README.md>
* The project page describes the training mix (Fisher English, plus synthetic
  dialogues rendered with Chatterbox) and does not define a dump:
  <https://research.nvidia.com/labs/adlr/personaplex/>
* The paper is the same: <https://arxiv.org/abs/2602.06053>
* The weights card names Fisher and does not define a dump:
  <https://huggingface.co/nvidia/personaplex-7b-v1>
* Issue #5 asks for fine-tune code. NVIDIA did not provide a schema. A
  comment points at Kyutai's Moshi fine-tune:
  <https://github.com/NVIDIA/personaplex/issues/5>

PersonaPlex is a Moshi fine-tune. The default adapter
(`--personaplex-adapter moshi`) writes the format published by
[kyutai-labs/moshi-finetune](https://github.com/kyutai-labs/moshi-finetune/blob/main/README.md):

```json
{"path": "stereo/clip.wav", "duration": 1.0}
```

Stereo wav, 24 kHz, left channel is the rendered line (the channel Moshi
learns), right channel is silence (there is no user turn). Beside each wav,
[annotate.py](https://github.com/kyutai-labs/moshi-finetune/blob/main/annotate.py)
writes:

```json
{"alignments": [["notebook", [0.2, 0.5], "SPEAKER_MAIN"]]}
```

The default sidecar is that object and nothing else. Word times are a
uniform split of the known transcript across the clip. They are not Whisper
times. This pipeline does not download an aligner. Run `annotate.py` before
a real training job if you need model-derived times.

These clips are single-speaker emotion lines, not full-duplex conversations.
The adapter is a bridge onto the stereo layout. It does not claim the result
is a Fisher-style dialogue. `prompts.jsonl` carries emotion, intensity, and
a suggested text prompt for an operator. The Moshi trainer does not read it.

Paths in `train.jsonl` are relative to `personaplex/` unless
`--absolute-paths` is set. The Moshi README's examples are relative to the
process working directory, so run the trainer with that directory as cwd or
pass absolute paths.

`--personaplex-adapter community` is a separate adapter for the unofficial
trainer at
<https://github.com/emotion-machine-org/personaplex-finetune/blob/main/AGENTS.md>.
That trainer uses the same `{path, duration}` JSONL, labels the agent
`SPEAKER_BROKER` (and a user `SPEAKER_CLIENT`, which this data does not
have), and stores a `text_prompt` in the sidecar. It is not the NVIDIA or
Kyutai default.

`--eval-fraction` (default 0) holds out a stable slice into `eval.jsonl`.

## Plug-in points

Three sibling branches are not merged here and are not edited by this tool.
Until they import, the local defaults are used and `run.json` records
`local`. A plugin that imports but does not match the contract is recorded
as `local-mismatch` and the local behaviour stands.

Taxonomy, branch `cursor/emotion-v2-taxonomy`. Import name
`emotion_v2_taxonomy`:

```python
def load_taxonomy() -> list[str]:
    """The twelve canonical names. The set must match exactly."""
```

This pipeline keeps its own order even when the plugin matches, so manifests
stay stable.

Capture, branch `cursor/emotion-v2-capture`. Import name
`emotion_v2_capture`:

```python
def resolve_references(voice_id: str) -> dict[str, str]:
    """Emotion name -> reference directory. Directories are not opened."""
```

Profile entries win. Unknown emotion keys are ignored. The strings are
copied into `run.json` under `reference_dirs`.

Scorers, branch `cursor/emotion-v2-ci`. Import name `emotion_v2_ci`:

```python
def build_scorers(config: dict) -> list:
    """Each item has .name in {'wer', 'ecapa', 'emotion'} and .score(clip) -> float."""
```

`clip` has `audio`, `text`, `emotion`, `voice_id`, and `intensity`. Pass
`scorers=` on `RunConfig` to inject scorers without the plugin. Scorer names
outside those three are ignored and listed in `run.json`.

Put the plugin packages on `PYTHONPATH`. This tool already calls them.

## Tests

From `tools/emotion-v2/dataset`:

```bash
python3 -m unittest discover -s tests -v
```

The tests render the stub voice only. Chatterbox is exercised with a sine
engine behind the consent gate, and with the CLI, which has no engine and
must refuse. No test writes into `datasets/emotion-v2/`.

## Safety

* Do not commit audio, clips, weights, or secrets.
* Do not point the Chatterbox engine at a real person's recordings unless
  that voice's consent record includes `training-data-generation`.
* The stub path never reads reference audio, including when a profile lists
  a path.
* Pitch stays on the 432 Hz standard. A retune to 440 Hz is refused.
