# Jarvis texture

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-03. Symphony section. Index: [README.md](README.md).

The spelling is **Galion**. The product is Galion Voice.

The Jarvis reference carries the Tony Stark hub: clicks, alarms, and room ambience. That texture is the character. The clone keeps it. A denoise pass that strips the room leaves a different voice, and the gate fails.

The file and its hash are in [CLIP-STORE.md](CLIP-STORE.md):

| | |
|---|---|
| Path | `docs/galion-voice-v2/personas/refs/jarvis.wav` |
| Also | `/workspace/galion-clip-store/jarvis/reference.wav` |
| Shape | 15.95 s, 48000 Hz, stereo, 16-bit |
| sha256 | `c2200b59fa78b3c98d13ac5dc7db3a6c14e802310dca73033d7770bd4b532c60` |

WAV only. The git slot is `jarvis.placeholder`. The audio is gitignored.

## Behaviour that travels with the texture

Jarvis is an internal test voice. Calm, precise, dry, brief. The range is narrow: calm, mild amusement, focused attention, quiet satisfaction. Tags stay off. The words stay on the matter. When the empathy weight rises he goes lower, slower, and quieter, on the Jarvis column in [SAMANTHA-V2.md](../SAMANTHA-V2.md). The full instruction file is [JARVIS.md](../personas/JARVIS.md).

The hub is part of the reference, not a line he speaks about. He does not narrate the clicks.

## Gate

**PASS** when a spectral or energy check shows the texture band still present beside the speech, compared with a clean-speech-only baseline of the same take, and the sha256 of the ref still matches.

**FAIL** when a prep step high-passes, denoises, or otherwise removes the hub before the clone.

**BLOCKED** when `jarvis.wav` is not on the machine, so the check has no input. This landing does not commit the WAV.

Record the curves and the hash in [SELF-EVAL.md](SELF-EVAL.md).
# Jarvis hub texture

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. The hub is a layer. The toggle mixes it. The asset stays.

Master slogan: "The seamless connection with the human being, on the voice only."

Jarvis, on the test path, carries a Tony Stark hub: clicks, alarms, and
virtual ambience. Those three are character. They are one layer, stored
and mixed apart from the speech asset. Hearing the hub means the layer
is on. Hearing the speech alone means the layer is off. Off is a mute
of that layer. It is not a denoise, a declick, or a rewrite of the asset.

The code is `crates/galion-ml-voice/src/jarvis_texture.rs`. The check is
`scripts/eval/jarvis-texture.sh`. The record is `SELF-EVAL.md`.

This file does not open a wav, does not clear rights, and does not edit
`LOCKS.toml`. The hold in `VOICE-CLONING.md` stays closed.

## Toggle ABI

Persona is `jarvis`. One boolean. Default is on, so a missing field is
not a clean.

Wire, one JSON object, one line:

```text
{"persona":"jarvis","hub_texture":true}
{"persona":"jarvis","hub_texture":false}
{"persona":"jarvis"}
```

| Field | Type | Rule |
|---|---|---|
| `persona` | string | Optional. When present it is `jarvis`. Any other id is refused. |
| `hub_texture` | bool | Optional. `true` mixes the layer. `false` does not. Absent means `true`. |

`hub_texture` is the only off switch. A second key that names a cleaner
(`denoise`, `declick`, `highpass`, and the rest of the forbid-list) makes
the line invalid. The parser is `parse_toggle_line`. The value is
`TextureToggle { enabled: bool }`. `TextureToggle::default` and
`TextureToggle::from_flag(None)` are on.

Rust mix, sample-aligned, one rate:

```text
apply_hub_toggle(asset, asset_rate, layer, layer_rate, toggle) -> pcm
```

| | Off (`enabled = false`) | On (`enabled = true`) |
|---|---|---|
| Read | `asset`, `layer` | `asset`, `layer` |
| Write | nothing | nothing |
| Result | a new buffer equal to `asset` | a new buffer, `asset[i] + layer[i]` |
| Length | `layer.len() == asset.len()` | same |
| Rate | `layer_rate == asset_rate`, and at least 16 kHz | same |

The speech asset is the clean-speech baseline. The layer is the hub:
clicks, alarms, and the virtual bed, already summed into one buffer.
Both buffers stay the caller's. The function returns a new `Vec`. A
later worker that learns this line still must not write the asset, and
still must not send a reference clip until `CLIP-INGEST.md` says the
gate is open. This scaffold does not put the mix on the clone worker.

Preparation that may sit in front of the mix, because it does not remove
the hub: `downmix`, `mono`, `dcblock`, `dcblocker`, `gain`,
`peaknormalize`, `peaknorm`, `normalize`, `resample`, `sincresample`.
A resampler has to keep the band from 20 Hz through the click region.
The toggle itself does not resample. Unknown stage names are not a
permit to strip; the band check still runs.

The mix is off the 80 ms heartbeat. `Heartbeat::step` does not read the
flag and does not add the layer.

## Energy-band check

`eval_hub_toggle(asset, rate, layer, off_pcm, on_pcm, stages)` measures
three bands on the clean-speech baseline, on the layer, on the off
render, and on the on render. It fails if the off render is a cleaned
asset, or if the on render does not carry the layer.

| Band | What it is | Measure |
|---|---|---|
| click | UI clicks, short and broadband | High-pass at 6000 Hz. A sample counts when the residual exceeds 8 times a 30 Hz envelope of that residual, and exceeds a floor of 0.02. Energy is the residual inside a ±2 sample mask. |
| alarm | Hub alarm | Band-pass at 3200 Hz, Q = 8. |
| ambience | Virtual room, the low bed | Low-pass at 250 Hz, summed on 20 ms frames whose broadband RMS is at or below the 40th percentile. |

Let `B` be the baseline (the speech asset), `L` the layer, `O` the off
render, `N` the on render, for one band.

- The layer carries the character. Each of click, alarm, and ambience is
  at least `0.01` of the layer's total energy. Below that the layer is
  silent in that character and the check fails.
- Off matches the asset. Sample for sample, the absolute difference stays
  within `1e-5`. A denoise of a baked mix does not land on the asset, so
  it fails here. Band energy stays within `0.02` of `B`, scaled by the
  larger of `B` and `L`. Below that window, the asset was stripped.
  Above it, the layer is still mixed and the toggle did not turn off.
- On clears the baseline. `N >= B + 0.70 * L` in every band. The `0.70`
  keep is the fraction of the layer's band energy that has to show up
  above the clean speech. Missing the layer, or a mix that never added
  it, fails.
- On still contains the speech. Total energy of the on render stays at
  least `0.98` of the asset's total energy.

A forbidden stage fails the check even when both renders are a correct
mix. The stage list is not a substitute for `hub_texture: false`.

The self-eval builds a synthetic baseline (phrased harmonics, pauses at
zero, partials at or under 1100 Hz) and a synthetic layer (impulses, a
3200 Hz burst inside the phrases, a 90 Hz and 180 Hz bed). It runs the
toggle both ways. It also feeds a declick, alarm notch, and bed gate of
the baked sum as a fake off render, and that path must fail. No film
sample is used. The gitignored `jarvis.wav` is not opened. A measurement
of that file is BLOCKED until the file is on a machine this check can
read and the rights gate says it may.

```text
scripts/eval/jarvis-texture.sh
```

Exit 0 prints one JSON line with `"result":"PASS"`. The wav field on
that line is `"BLOCKED"`. A red test is FAIL and the script exits 1
before the line.

## Forbid-list

These stages are cleaners. They are refused on the toggle line and in
`stages`. Matching is ASCII letters and digits, lower case, longest
token. The authoritative table is `FORBIDDEN_CLEAN` in the Rust file.

| Token | Class |
| `deepfilternet` | neural denoise |
| `deepfilter` | neural denoise |
| `rnnoise` | neural denoise |
| `facebookdenoiser` | neural denoise |
| `resembleenhance` | enhance |
| `podcastenhance` | enhance |
| `dialoguenhance` | enhance |
| `speechenhance` | enhance |
| `voiceenhance` | enhance |
| `enhancevoice` | enhance |
| `denoiser` | denoise |
| `denoise` | denoise |
| `noisereduce` | spectral denoise |
| `noisereduction` | spectral denoise |
| `noisered` | spectral denoise |
| `spectralsubtraction` | spectral denoise |
| `spectralsubtract` | spectral denoise |
| `spectralgate` | spectral gate |
| `wiener` | spectral denoise |
| `afftdn` | ffmpeg denoise |
| `anlmdn` | ffmpeg denoise |
| `arnndn` | ffmpeg denoise |
| `auphonic` | hosted clean |
| `krisp` | hosted denoise |
| `izotope` | restoration |
| `audiorestoration` | restoration |
| `declick` | click removal |
| `decrackle` | crackle removal |
| `dehum` | hum removal |
| `dereverb` | room removal |
| `deroom` | room removal |
| `voiceisolation` | voice isolation |
| `vocalisolation` | voice isolation |
| `isolatevoice` | voice isolation |
| `isolatevocal` | voice isolation |
| `stemseparat` | source separation |
| `demucs` | source separation |
| `spleeter` | source separation |
| `noisegate` | noise gate |
| `noisegating` | noise gate |
| `expandergate` | noise gate |
| `stripsilence` | silence strip |
| `silenceremove` | silence strip |
| `removesilence` | silence strip |
| `vadtrim` | silence strip |
| `removenoise` | noise removal |
| `removebackground` | background removal |
| `backgroundremoval` | background removal |
| `cleanreference` | clean prep |
| `cleanaudio` | clean prep |
| `cleanvoice` | clean prep |
| `audioclean` | clean prep |
| `highpass` | band cut |
| `lowpass` | band cut |
| `highcut` | band cut |
| `lowcut` | band cut |

A high-pass that "removes rumble" and a low-pass that "removes clicks"
are on this list because the rumble and the clicks are the hub. The
allowed DC block is the stage named `dcblock` or `dcblocker`, not a
`highpass`.

## What stays closed

The program brief names a gitignored `jarvis.wav` (48 kHz, stereo,
16-bit). This change does not hash it and does not mix it. Rights for a
film reference stay on the hold. Piper and flite remain the speaking
path. The synthetic check is the gate that can run here.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
