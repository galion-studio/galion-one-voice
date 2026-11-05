# Shared LTAS tone-colour / depth (`--timbre-match`)

Reference-driven, voice-agnostic post stage in `cbx-v2-timbre`.

## Flags
- `--timbre-match <0..1>` default **0** (off, bit-identical)
- `--timbre-ref <reference.wav>` required when match > 0

## Offline
```
cbx timbre apply --input clone.wav --reference ref.wav --strength 0.5 --out out.wav
cbx timbre analyze --input clip.wav --reference ref.wav
```

## Recommended
`--timbre-match 0.5` with each voice’s `clip-store-own/<voice>/reference.wav`.

See `REPORT.md` for scored tables.
