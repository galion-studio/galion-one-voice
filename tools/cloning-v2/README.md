# Cloning V2 tooling (emotion blend)

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.

Box-local cbx voice-profile prep and checks. See
`docs/galion-voice-v2/symphony/EMOTION-BLEND.md`. Experimental; OFF by default.
Do not commit WAVs, `.cbxvoice`, or clip-store audio.

Reference fidelity (branch A) lives in `fidelity/`: best-segment conditioning
for a 6 s or 50 s donor, opt-in spectral envelope, and
`fidelity/scripts/eval-laptop.sh` for the offline WER / ECAPA / MOS-proxy /
persona emotion_match report. It does not clone and does not evaluate consent.
