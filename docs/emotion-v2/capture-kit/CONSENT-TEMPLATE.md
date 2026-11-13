# Emotion V2 — consent template

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb.

This is a template. It is not a consent record. No signer, date, or
signature is filled in here. Do not invent one. Do not copy the template
into a session folder and call it signed.

The machine file is [consent.template.json](consent.template.json). A signed
copy, when one exists, stays with the originals outside git.

## What is required

1. **Mate's recording consent is required** before any take is recorded.
   The WAV must be a recording the owner made of their own voice for this
   internal test register (`consent = "owner-own-recordings-only"`). It is
   not a recording of another person, not a clip from a film or a call, and
   not model output.
2. **If any take is used for training**, Slavik's signed consent record is
   required first, and `consent_scope` must include `training-data-generation`.
   The clone-time scope `local-zero-shot-conditioning` does not authorize
   training-data generation. See `tools/emotion-v2/dataset/emotion_v2_dataset/consent.py`
   and [`../DATASET-PIPELINE.md`](../DATASET-PIPELINE.md).

Until those signatures exist, the capture may still proceed only with (1) in
hand, and the takes stay in the reference register. They are not training
data.

## How to use the template

1. Copy `consent.template.json` to a private path outside the repo,
   for example next to the session originals.
2. The named person fills `signer_name`, `signed_date` (`YYYY-MM-DD`, not in
   the future), and `signature`, and sets `granted` to `true`, only when they
   actually agree.
3. Leave every other copy blank. Never commit a signed file. Never point the
   dataset tool at the unfilled template.

The format script will refuse this template if you pass it as `--consent`.
It does not write a consent file of its own.

## Fields in the JSON template

**Recording consent (Mate)**

- `required_from`: Mate Grajczyk
- `purpose`: owner's own voice, Emotion V2 controlled-reference register
- `signer_name`, `signed_date`, `signature`: left empty in the template
- `granted`: `false` in the template

**Training consent (Slavik), only if a take is used for training**

- `required_from`: Slavik
- `required_when`: if and only if any take from the session is used for
  training-data generation
- `consent_scope`: `["training-data-generation"]` (the scope that would be
  granted; not evidence that it has been)
- `voice_id`, `owner`, `signer_name`, `signed_date`, `signature`: left empty
- `granted`: `false` in the template

The example at `tools/emotion-v2/dataset/data/consent.example.json` is a
synthetic fixture for `stub-sine`. It is not a person's consent and it is
not this template.
