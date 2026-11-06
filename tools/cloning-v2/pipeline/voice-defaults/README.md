# Per-voice render defaults (box-local)

Only `own-samantha.json` and `own-jarvis.json` opt into `--timbre-match 0.5`.
All other voices keep the binary default `--timbre-match 0` (off, bit-identical).

Use `pipeline/bin/cbx-speak-voice` which reads these configs when present.
