# Emotion V2 — capture kit

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-14. Ready-to-run kit for the controlled reference capture
in [`../CAPTURE-PROTOCOL.md`](../CAPTURE-PROTOCOL.md). The taxonomy is
[`../TAXONOMY.md`](../TAXONOMY.md) (PR #89): twelve official emotions in four
sectors, plus a Samantha-only optional `seductive` slot that is not part of the
inventory and is not scored for Jarvis.

This kit is documentation and small scripts. It does not contain audio. It does
not fill in or fabricate a consent record. It does not merge, deploy, or change
the live scorer.

## What you get

| File | Role |
|---|---|
| [SESSION-CHECKLIST.md](SESSION-CHECKLIST.md) | One card per session, walked before arming record |
| [FILENAME-SCHEME.md](FILENAME-SCHEME.md) | Layout, take names, and the per-sector refs contract |
| [PROMPTS.md](PROMPTS.md) | Shared carrier plus per-sector prompt lines for Samantha and Jarvis |
| [prompts.json](prompts.json) | Machine-readable copy of those lines |
| [CONSENT-TEMPLATE.md](CONSENT-TEMPLATE.md) | Who must sign, and when |
| [consent.template.json](consent.template.json) | Unfilled template. Not a consent record |
| [manifest.template.json](manifest.template.json) | Session take manifest with sha256 fields |
| [per-sector-refs.template.json](per-sector-refs.template.json) | `line/emotion` → ref clip path + sha256 |
| `tools/emotion-v2/capture-kit/write_manifest.py` | Validates WAV format and writes the sha256 manifests |

## Rules the protocol already set

- 5 takes of each of the 12 official emotions, at least 3 acoustic PASS takes each
- One 432 Hz calibration take per session, before the emotions, at least 1 PASS
- 48 kHz, 24-bit packed integer PCM, mono, little-endian WAV
- 30 cm, one room, one microphone, one gain for the whole session
- Standard `galion-voice-v2-432`. Consent string `owner-own-recordings-only`
- Owner's own recordings only. Originals stay read-only and out of git
- The same carrier sentence is used for every official emotion on a line, so a
  later score compares the emotion, not the wording

The twelve official slugs are: `neutral`, `calm`, `happy`, `amused`, `excited`,
`tender`, `sad`, `disappointed`, `angry`, `fearful`, `surprised`, `disgusted`.

Sectors (taxonomy #89): Grounded (`neutral`, `calm`), Bright (`happy`,
`amused`, `excited`, `tender`), Heavy (`sad`, `disappointed`), Sharp (`angry`,
`fearful`, `surprised`, `disgusted`).

## Who records which line

Both lines are the owner's own voice, spoken in the character of that line.

- **Samantha** — all twelve official emotions. Optional `seductive` takes may
  be recorded after the twelve. They do not count toward the session seal.
- **Jarvis** — the twelve official emotions only. Do not record `seductive`
  for Jarvis. That slot is n/a for his persona.

Record one session per line unless both lines truly share the same room,
microphone, gain, and 30 cm mark on the same card.

## Consent

- Mate's recording consent is required before any take is recorded.
- If any take is used for training, Slavik's signed record with
  `consent_scope` including `training-data-generation` is required first.
- Do not invent a signer, a date, or a signature. Leave the template blank
  until the named person actually signs a private copy. See
  [CONSENT-TEMPLATE.md](CONSENT-TEMPLATE.md).

The format script does not write consent and will refuse the unfilled
template if you pass it as a consent file.

## Run the kit

From the repo root, after the takes for one session are on disk (outside git):

```bash
DIR=originals/samantha/2026-11-14-studio

python3 tools/emotion-v2/capture-kit/write_manifest.py session \
  --dir "$DIR" \
  --line samantha \
  --out "$DIR/MANIFEST.json"
```

That checks each WAV's format (48 kHz / 24-bit packed PCM / mono) and writes
sha256 of the file bytes. It does not score loudness, reverb, or tilt. After
format PASS, run the existing acoustic checker and seal from
[`../CAPTURE-PROTOCOL.md`](../CAPTURE-PROTOCOL.md):

```bash
SPEC=docs/emotion-v2/capture-spec.toml

cargo run --manifest-path tools/emotion-v2/refcheck/Cargo.toml -- \
  manifest --dir "$DIR" --spec "$SPEC" --out "$DIR/MANIFEST.json"

find "$DIR" -name '*.wav' -exec chmod a-w {} +

cargo run --manifest-path tools/emotion-v2/refcheck/Cargo.toml -- \
  verify-manifest --manifest "$DIR/MANIFEST.json" --spec "$SPEC" --require-session
```

`refcheck` rewrites `MANIFEST.json` with acoustic verdicts and stays the
session seal. The capture-kit script uses the same file fields (`path`,
`sha256`, `bytes`, `emotion`, `role`, `verdict`) so the two tools can share
the file. `verdict` stays `null` until `refcheck` fills it.

### Per-sector reference map (agent 3)

After a sealed session, pick one acoustic PASS take per official emotion and
add it to the shared refs file. The lookup key is `line/emotion`. Each entry
is a path to the original take plus the sha256 of those bytes:

```bash
python3 tools/emotion-v2/capture-kit/write_manifest.py refs \
  --line samantha \
  --emotion neutral \
  --wav "$DIR/neutral/take-03.wav" \
  --out originals/per-sector-refs.json
```

Repeat for each official emotion, and for Jarvis on his session. Do not copy
or rewrite the WAV. The map points at the read-only original.

Schema: `galion-emotion-v2-per-sector-refs/1`. Contract in
[FILENAME-SCHEME.md](FILENAME-SCHEME.md) and
[per-sector-refs.template.json](per-sector-refs.template.json). When this kit
was written, branch `cursor/emotion-v2-per-sector-refs` was not yet on the
remote. The field names (`line`, `emotion`, `path`, `sha256`) are the agreed
shape. If that branch publishes a list form instead of a map, the script
accepts either and rewrites the map plus a `refs_list` array.

## Tests

```bash
python3 -m unittest discover -s tools/emotion-v2/capture-kit/tests -v
```

Tests use synthetic silence and sine only. They do not read a voice. They
do not write a consent record.

## Out of scope

No live deploy. No merge. No audio in git. No filled consent. No change to
`docs/emotion-v2/capture-spec.toml` or to the refcheck seal. Optional
`seductive` takes are never required for a PASS session.
