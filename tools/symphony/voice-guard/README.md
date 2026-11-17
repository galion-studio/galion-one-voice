# galion-voice-guard

Perpetual Open MIT License. (c) 2026 Galion Labs.

Fail-closed checks for one rendered clip, or one stream of 80 ms frames, before the audio reaches listening-back or a listener. A failed report marks the clip bad and gives the reason. The samples stay where they are.

The report schema is `galion-voice-guard/1`. Thresholds are `configs/symphony/voice-guard.toml`. A number in that file is a published bar only where the comment says so.

## Reused, not rewritten

| Check | What it calls |
| --- | --- |
| True peak | `ebur128` 0.1.11 (MIT, Sebastian Dröge). ITU-R BS.1770, 4× oversampling under 96 kHz. |
| Clipping, sample-jump click, cut-off jump | The galion-hygiene definitions in `tools/cloning-v2/scorers/quality/galion-quality/src/bin/galion-hygiene.rs`. |
| AR click | Forward LPC residual. Godsill and Rayner, *Digital Audio Restoration* (Springer, 1998); Levinson-Durbin as in Markel and Gray, *Linear Prediction of Speech* (1976). |
| Dropout | Energy mask. The gap is the 0.20 s pause already used by `galion-articulation` select (`PAUSE_S`, the refprep split). Silero and webrtc VAD are not linked. |
| Repeated frame | Normalised correlation of successive 80 ms frames. A pitch lag that tiles the frame is excluded (Rabiner, IEEE Trans. ASSP, 1977). |
| Babble | Spectral flatness (Johnston, IEEE J. Sel. Areas Commun., 1988) via `rustfft`. |
| Buzz | Praat HNR (Boersma, IFA Proceedings 17, 1993) through `galion-articulation` / `praatfan_rust` (MIT). |
| Speaker drift | TractPrint v2, `galion_articulation::score` (PR #191). An ECAPA cosine is passed in by the caller from `galion-ecapa`. This crate does not run that model and does not load its weights. |
| Sample rate, channels, bit depth | The allowed set in the config. |

Neural quality models (DNSMOS, UTMOS, NISQA) are a stub unless the crate is built with `--features neural` and `[neural]` names a scorer that is already on disk. Nothing is downloaded. `ort` is not a dependency.

## Hook

`galion_voice_guard::before_listen` is the call `galion-mne` (PR #196) makes between sound out (PR #199) and listening-back (PR #197). `Err` means do not play the clip and do not call listening-back.

```rust
match galion_voice_guard::before_listen(&pcm, sample_rate_hz, &cfg, baseline, embedding_cos) {
    Ok(report) => { /* listening-back may see pcm */ }
    Err(report) => { /* report.bad, report.reason, report.bad_frames */ }
}
```

`admit` is the same gate for an interleaved buffer. `check_frames` takes one slice per 80 ms frame.

## Run

From the repository root:

```bash
cargo run --manifest-path tools/symphony/voice-guard/Cargo.toml -- check \
  --wav clip.wav --config configs/symphony/voice-guard.toml --out /tmp/guard.json
```

Exit 0 is pass. Exit 1 is fail (the JSON is still written). Exit 2 is a bad argument or an unreadable file.

`--baseline` is a mono WAV for TractPrint. `--embedding-cos` is a cosine the caller already computed. `--no-release` skips the cut-off check on a stream that has not ended. `--raw FILE --sr HZ` reads little-endian f32 mono.

```bash
cargo test --manifest-path tools/symphony/voice-guard/Cargo.toml
```

Tests synthesize tones and inject clicks, clipping, NaN, a subnormal, DC, a dropout, a copied frame, a hard ending, white noise, and an octave shift. No audio file is committed. `cargo test` here was run with rustc 1.85. `encoding_rs` is pinned at 0.8.35 because 0.8.44 asks for rustc 1.88; that pin matches `galion-articulation`'s lock.
