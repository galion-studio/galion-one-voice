# Clone unblock with CC0 donors (owner decision t623u, 2026-11-04)

Owner: Maciej Grajczyk. `LOCKS.toml` now has `reference_clips = true` **only** in
`[formats.exceptions.v2s14_chatterbox]`, with `ref_clip_decision = "t623u"` and the scope string.
The global `[formats] reference_clips` is unchanged. The no-celebrity note and the 2027-02-01 sunset are kept.
No Armstrong audio and no film audio. The old t273 clip-store wavs are not used. Their REFS rows are commented out, and nothing was renamed.

## Donors (CC0 / public domain)

| persona | performer | source recording | cut | ref |
|---|---|---|---|---|
| samantha | Amy Koenig (LibriVox reader 6620) | LibriVox "Love and Friendship" track 1 (`love_friendship_1107_librivox`, archive.org licenseurl publicdomain/mark/1.0) | 10.55–20.15 s → 9.600 s | 44.1 kHz mono s16, sha256 `e4bc60c3…4641` |
| jarvis | Alan Davis Drake (LibriVox reader 254) | LibriVox "Celebration of Dialects and Accents Vol 1" track 30 (`dialect_accent_0909_librivox`, licenseurl licenses/publicdomain) | 26.59–37.30 s → 10.710 s | 44.1 kHz mono s16, sha256 `60ad5457…b6` |

Both performers are the voices of the Voice Zero CC0 1.0 sample set. Those samples (6.40 s / 7.06 s) are below the
gate's 8 s minimum, so the refs were re-cut from the same performers' original LibriVox recordings. Both recordings
state "this recording is in the public domain". The source is the archive.org MP3 and the reference is a WAV decode. There is
no denoise and no normalisation. Cut boundaries were chosen at word gaps (edges −63/−56 dB).

Store (outside the tree): `$GALION_CLIP_STORE/<persona>/{reference.wav, reference.wav.sha256, consent.txt}`.
`consent.txt` holds every REQUIRED field. The grantor is the performer through the LibriVox public-domain dedication. No signed
release exists, and the file says so. It also records public_demo: no, expiry 2027-02-01 (the LOCKS sunset), and counsel: not consulted.

## Code

- `galion-consent`: the `weights` check now also reads `scripts/clone/TOKENIZER.sha256`. Before, it only read
  `PINS.sha256`, which has no tokenizer rows, so the gate refused real weights with "weights absent".
  Test: `tokenizer_rows_in_their_own_table_count_as_pinned`.
- `examples/clone_probe.rs`: one real clone through `CloneTts::try_new` + `synthesize`, and a WAV out.
- REFS.sha256, persona tomls, check-locks, clone-landing smoke, and test pins were moved to the new refs.

## Measured 2026-11-04 12:14–12:20 CET (under BENCH.lock, Nano, CPU, 1 session vCPU, debug build)

| persona | galion-consent check | try_new | synth (incl. worker spawn + load + cond) | audio | ECAPA vs own ref | vs other ref |
|---|---|---|---|---|---|---|
| samantha | ready 9.600 s public=no | ok | 26.6 s | 4.32 s | **0.729** | 0.117 |
| jarvis | ready 10.710 s public=no | ok | 25.4 s | 4.64 s | **0.788** | 0.104 |

The ref-vs-ref cosine is 0.132. Piper vs refs is −0.026 / 0.031. Earlier clones from the shorter Voice Zero samples scored 0.62–0.79.
Negative controls: LOCAL_REFS unset → refused. Old store → refused at duration.
`try_new` takes about 120 s in a debug build because it hashes the 3 GB of weights.

## CC0 licence checkbox (owner t628u, 2026-11-04 20:49 CET)
CC0 donors are ON again (the 16:01 "off" was a wrong click). A CC0 voice is used only after the caller ticks the CC0 box.
- UI: `crates/galion-console/assets/v1-screen.html` shows an unticked checkbox naming "CC0 1.0 Universal (Public Domain Dedication)" with a link to https://creativecommons.org/publicdomain/zero/1.0/. Ticking posts `POST /v2/cc0 {"accept":true}` with the session.
- Server (Rust): `galion_consent::Cc0Acceptances` keeps acceptance per session in memory only (sha256 of the token; no token, no disk). `/turn` and `/v2/say` mark the active session. The persona floor reports `clone_block:"cc0_licence"` and `CloneTts::synthesize` refuses a CC0 voice until the active session accepted. The gate marks a voice CC0 when its consent.txt `instrument` / `grantor` names CC0 or a public-domain dedication.
- Re-run 2026-11-04 20:54-21:04 CET (debug clone_probe, under BENCH.lock): unticked = refused (`cc0_licence`) for both voices; ticked = cloned. ECAPA clone vs own donor: Samantha 0.6886, Jarvis 0.7974; cross-speaker ≤0.135; Piper vs donor ≤0.03.
