# galion-mne

Perpetual Open MIT License. (c) 2026 Galion Labs.

Swarm 13 integrator (S8). One CLI over the drafts already stacked on this branch. It does not implement the seven layer crates.

```text
S1 contract + fixtures     waiting   tools/symphony/mne-contract
        |  intent in, read-back out (their schema, not this crate)
S2 feeling                 waiting   tools/symphony/mne-feeling
        |  until then: galion-emotion-render (PR #192)
S3 throat                  waiting   tools/symphony/mne-throat
S4 mouth                   waiting   tools/symphony/mne-mouth
S5 nose + wordless sounds  waiting   tools/symphony/mne-nose
        |  until then: galion-articulation decode (PR #191)
        |  air: galion-lungs (PR #195), already wired
S6 sound out               waiting   tools/symphony/mne-sound
        |  until then: articulation closed-loop stand-in
        |  galion-voice-guard (S9) admits the PCM or refuses it
S7 listening back          waiting   tools/symphony/mne-listen
        |  until then: the same decoder reads the tone back, one extra pass
```

Upstream memory, persona, and inner life are not in this binary. `--text` and `--pin` stand in for S1's contract so the stack can be run. When `galion-mne-contract` lands, those flags stop being the contract.

```bash
cargo test --manifest-path tools/symphony/mne/Cargo.toml
cargo run --manifest-path tools/symphony/mne/Cargo.toml -- layers
cargo run --manifest-path tools/symphony/mne/Cargo.toml -- \
  run --text "Hello." --pin embarrassment \
  --voice tools/symphony/voice-v2/voices/synthetic.toml --out /tmp/mne
```

The voice file is the pluggable baseline (profile toml). The tone in tests is synthetic. Nothing is downloaded.

`run` calls `galion-voice-guard` on the mixed PCM before the read-back. The current stand-in (impulse-train vowel plus breath) is refused, so listening-back does not see it. A clean tone is admitted by `galion_voice_guard::before_listen`.
