# Persona emotion target files

Format any new voice can use: `pipeline/personas/<voice-id>.toml`, schema `galion-persona-emotion-targets/1`.

A file states who the persona is and which emotions it is judged on, with a numeric target for each. Emotions not listed are **n/a** (not scored as 0). Gate is 0.83 over the listed emotions only.

See `scorers/persona/INTERFACE.md` for the loader, detector hook, and CLIs.

Shipped files:

- `own-samantha.toml` — joyful/happy, aggressive, love, laughter, logic, neutral, seductive
- `own-jarvis.toml` — neutral, logic (seductive is not listed)
