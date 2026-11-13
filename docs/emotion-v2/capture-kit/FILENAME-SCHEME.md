# Emotion V2 — filename scheme and manifests

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. Layout for capture originals and for the per-sector reference
map that the scorer fix (branch `cursor/emotion-v2-per-sector-refs`) consumes.

No audio belongs in git. Paths below live under an `originals/` tree that is
gitignored at the repo root.

## Session tree

One session is one line, one room, one microphone, one gain, 30 cm:

```text
originals/<line>/<session-id>/SESSION.txt
originals/<line>/<session-id>/MANIFEST.json
originals/<line>/<session-id>/calibration/take-01.wav
originals/<line>/<session-id>/neutral/take-01.wav
originals/<line>/<session-id>/neutral/take-02.wav
...
originals/<line>/<session-id>/disgusted/take-05.wav
```

- `<line>` is `samantha` or `jarvis` (lowercase).
- `<session-id>` is `YYYY-MM-DD-<room>`. `<room>` is lowercase ASCII, digits
  and hyphens only, for example `2026-11-14-studio`.
- Official emotion folders are the taxonomy slugs: `neutral`, `calm`, `happy`,
  `amused`, `excited`, `tender`, `sad`, `disappointed`, `angry`, `fearful`,
  `surprised`, `disgusted`.
- Take files are `take-NN.wav` with a two-digit number starting at `01`.
  Extra takes to reach three passes are `take-06` and up. Do not reuse a
  number and do not delete a failed take.
- Calibration lives in `calibration/`, never under an emotion folder. The
  calibration take is a 432 Hz tone, not Neutral, and not a clone reference.

Optional Samantha-only slot, after the twelve, same session card:

```text
originals/samantha/<session-id>/seductive/take-01.wav
```

`seductive` is not a folder on a Jarvis session. `refcheck` does not require
it for a sealed session.

The take path is enough to recover line, session, role, emotion, and take
number. A flat export name, if you need one for a handoff folder, is:

```text
<line>-<session-id>-<emotion>-take-<NN>.wav
<line>-<session-id>-calibration-take-<NN>.wav
```

Do not flatten the originals tree itself. `refcheck` and this kit both read
folder names for `role` and `emotion`.

## Session manifest (`MANIFEST.json`)

Compatible with `tools/emotion-v2/refcheck` (`SessionFile` / `Manifest` in
`tools/emotion-v2/refcheck/src/manifest.rs`). `sha256` is the lowercase hex
digest of the **file bytes**, not a decoded PCM hash.

```json
{
  "tool": "capture-kit",
  "standard": "galion-voice-v2-432",
  "persona": "samantha",
  "consent": "owner-own-recordings-only",
  "originals": "read-only",
  "line": "samantha",
  "session_id": "2026-11-14-studio",
  "files": [
    {
      "path": "neutral/take-01.wav",
      "sha256": "<64 lowercase hex>",
      "bytes": 0,
      "emotion": "neutral",
      "role": "emotion",
      "verdict": null,
      "format_verdict": "PASS"
    }
  ]
}
```

- `path` is relative to the session directory (the folder that contains
  `MANIFEST.json`).
- `emotion` is the slug, or `null` for calibration.
- `role` is `emotion` or `calibration`.
- `verdict` is the acoustic seal (`PASS` / `FAIL`) filled by `refcheck`. The
  format script leaves it `null` so it does not pretend to have scored the room.
- `format_verdict` is added by the format script. `refcheck` ignores unknown
  fields. A later `refcheck manifest` may omit it; the sha256 fields stay.
- `tool` is `capture-kit` until `refcheck` rewrites the file; `refcheck`
  writes `tool: "refcheck"` and does not require that name on the way in.

See [manifest.template.json](manifest.template.json).

## Per-sector refs map (`line/emotion` → path + sha256)

This is the contract for agent 3's per-emotion reference lookup. Schema id:
`galion-emotion-v2-per-sector-refs/1`.

Canonical form is a map whose keys are `{line}/{emotion}`:

```json
{
  "schema": "galion-emotion-v2-per-sector-refs/1",
  "standard": "galion-voice-v2-432",
  "consent": "owner-own-recordings-only",
  "originals": "read-only",
  "refs": {
    "samantha/neutral": {
      "line": "samantha",
      "emotion": "neutral",
      "path": "originals/samantha/2026-11-14-studio/neutral/take-03.wav",
      "sha256": "<64 lowercase hex>"
    }
  }
}
```

Rules:

- One entry per official `(line, emotion)` pair used as a scorer reference.
- `path` points at the selected original take. The file is not copied and not
  rewritten. The selected take must be an acoustic PASS in the session manifest.
- `sha256` is the lowercase hex digest of those same file bytes, and must
  match the session manifest row for that path.
- Optional `seductive` may appear only as `samantha/seductive`. Never
  `jarvis/seductive`.
- Empty `path` or `sha256` in the template means "not yet selected". That is
  not a sealed reference.

The writer also emits `refs_list`, an array of the same objects, so a consumer
that wants a list can ignore the map. On read, either `refs` (map or list) or
`refs_list` is accepted.

See [per-sector-refs.template.json](per-sector-refs.template.json).
