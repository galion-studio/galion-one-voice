# S27 — ear-side prosody

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S27, ear-side prosody |
| Written | 2026-11-24. No weight download, no voice file, no paid API, no merge. |
| Plan | `docs/horbm/research/R4-cpu-realtime-integration.md` §11 on draft PR #207, head `dd902bbcfe34e23f63745a19fc2fed8e468a3b09`. The draft is closed and unmerged. The ear in that note is VAD plus streaming ASR. This layer sits beside that ear. It does not add a phase, a model, or a weight. |
| Emotion core | Merged PR #263. `ListenerOffer::inbound_intent` builds an inbound `IntentMsg` the core can `step` later. This crate does not call `step`. |
| Locks | `LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not created. |

## Frame

One call is one ear frame: 80 ms, 16 kHz, 1280 mono samples. That is the capture quantum already used beside the locked heartbeat. Playback stays 24 kHz and is not this crate. `FRAME_MS` is asserted equal to `galion-emotion-core` `FRAME_MS`.

The frame returns pitch (F0) and voicing, energy (RMS), a speaking-rate estimate, the trailing pause, relative jitter, relative shimmer, and one continuous point: valence, arousal, dominance, approach. There is no category count and no Cowen list.

Pitch is `pitch-detection` 0.3.0 `YINDetector` (MIT OR Apache-2.0). That crate's clarity figure is not a 0..=1 score — a pure sine comes back on the right hertz with a negative clarity — so voicing is the normalised autocorrelation at the period YIN returned. The rest is glue. Jitter and shimmer are Boersma's relative local forms (1993). The point is a fixed coupling: louder, higher, and faster raise arousal, which is the direction Juslin and Laukka (Psychological Bulletin, 2003) treat as the reliable vocal cue. Valence is the weak axis. Roughness and a long pause pull it down. The coefficients were not fit on a corpus.

## Consent

Owner decision, Mate, 2026-11-24 21:31 CET: this layer starts on.

`Ear::new` senses. The off switch is `Ear::deny`, `EarConfig { sense: false }`, and the environment variable `GALION_EAR_SENSE` set to `0`, `off`, or `deny` (`Ear::from_env`, `Ear::from_process_env`). Unset stays on. While the switch is closed, or the purpose is workplace or education, `push` returns `FrameOut::Denied` before the slice is read. Closing the switch drops the detector and the rings. `remember` does not open a store. `PERSISTS` is false. Nothing is written either way.

`LOCKS.toml` `[consent] sense` stays `deny` for the rest of the session. The ear's own default is the new table `[consent.ear_prosody] sense = "allow"`, `off_env = "GALION_EAR_SENSE"`. `scripts/check-locks.sh` checks that table. `SOUL.md` is not in this tree and was not created.

Adapt stays off, so a heard frame steps the emotion core and does not mix the pull until adapt is granted as well. `push_frame` is the path: prosody, then `EmotionCore::step`, then a [`FloorCue`]. A voiced frame's cue is the fixed mark `.`. A quiet or denied frame is quiet. The mark is not a transcript.

`galion-conductor` is its own workspace (it depends on `galion-mne-contract`, also a workspace root), so this crate cannot name `Conductor`. The join is `crates/galion-conductor/tests/ear_prosody.rs`: the cue is applied with `Conductor::tick`. Two voiced frames and two quiet frames reach eager end-of-turn and then the stored miss clause. The off switch ticks quiet and does not step the core.

## Port

`ListenerOffer.point` is `[valence, arousal, dominance, approach]`, the same order and limits as the emotion core. `inbound_intent` copies that point in as a pull relative to a speaker baseline, source inbound, verb `hear`, factor mask 0. The ear does not write breath, energy, fatigue, or health, and it does not step the core. The core still mixes an inbound pull only when both sense and adapt are set.

## Cost on this host

Not a lock p95. One process, no cgroup claim. Intel Xeon as reported by the guest (the model string is the generic hypervisor name), 4 cores, Linux 6.12.113+, rustc 1.85.0. Signal: 200 Hz sine, amplitude 0.2, 1280 samples. Eight warmup frames, then 40. Release mean 53 µs, max 57 µs. Debug mean 1319 µs, max 1487 µs. The test ceiling is the 60 ms compute budget inside the frame. The release max is the number to quote.

## Tests

`cargo test -p horbm-ear`. Sines, a stepped sweep, an in-frame chirp, and noise are built in the test. No audio file is committed. The deny path, the prohibited purposes, withdrawal, and a bad buffer are covered. `scripts/check-locks.sh` is unchanged in what it checks. CI stays the existing Ubuntu `gates` workflow (`cargo test --workspace`). No Windows workflow was added.
