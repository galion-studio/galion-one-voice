# Swarm 10 — latency-harness stream-clone (B3)

Perpetual Open MIT License. (c) 2026 Galion Labs.
Europe/Zagreb. 2026-11-15 22:12 +01:00.

The spelling is **Galion**. The product is Galion Voice.

| | |
|---|---|
| Role | stream-clone (Lane B, item B3) |
| Plan | [#151](https://github.com/galion-studio/galion-one-voice/pull/151) `Swarm 10 latency-harness plan`, branch `cursor/latency-harness-plan-68d6`. This PR does not edit `PLAN.md`. |
| Lane A | [#153](https://github.com/galion-studio/galion-one-voice/pull/153) `Swarm 10 clone-compiler plan`. A0 is BLOCKED-OWNER. No A1 generate PR was open (`Swarm 10 clone-compiler generate` in the title, search at 22:12 +01:00, 0 hits). |
| Host | `cursor` (`hostname`). Not the NEXT-ITERATION host `grok-bot-vm-511777615`. |
| Commit base | `8980e1a9b4214806dd62cfccf6e2aaf093ed86b0` |
| Measurement | **not measured** |

## What was checked

Read: Swarm 10 briefs (common rules and Lane B row B3), backlog B3, `NEXT-ITERATION.md` gap 5 (cited, not committed), `docs/galion-voice-v2/STREAM-TTFA.md`, `docs/galion-voice-v2/symphony/TTFA.md`, `docs/galion-voice-v2/HARD-LOOP.md` gate 6, and the lane plan above.

`scripts/eval/clone-stream-ttfa.sh` remains the Piper live path. This role does not relabel that clock as Nano.

A1 has not landed real PCM. The harness does not call the worker on this host. It does not set `GALION_CLONE_REF_SHA` or `GALION_CLONE_STUB_PCM`. `LOCKS.toml` and `scripts/clone/REFS.sha256` are not edited. `SOUL.md` and `config.yaml` are not in this checkout.

## What changed

- `scripts/eval/nano_original_ref_ttfa.py` and `scripts/eval/nano-original-ref-ttfa.sh`. They score original-ref Nano first PCM and the 80 ms first-playable frame. `ttfa_ms` is the time until 1920 samples (one 80 ms frame at 24 kHz) are in hand. `galion-clone-first` records the first PCM byte. `claim_80ms` is true only when both t273 personas were measured and that `ttfa_ms` is at most 80. A protocol-stub, Piper, or Flite capture is `FAIL` and has no `ttfa_ms`. Whole-utterance only at or above 1500 ms is `HARD-FAIL`. No real PCM is `BLOCKED` (exit 2).
- `scripts/eval/test_nano_original_ref_ttfa.py`. Fixture clocks are not a measurement of this host.
- `.github/workflows/gates.yml` runs those tests, then the harness. Exit 2 is printed as BLOCKED and is not a product pass. The same shape as the existing stream-TTFA step.
- `docs/galion-voice-v2/STREAM-TTFA.md` points at the new script. Piper numbers already on that page stay Piper.

No `galion-ttfa-measured/v1` file is written. Onset, barge-in, and e-to-a were not measured, and they are not invented. No WAV was written.

## Tests

2026-11-15 22:12 +01:00, host `cursor`, this worktree.

```text
python3 scripts/eval/test_nano_original_ref_ttfa.py
----------------------------------------------------------------------
Ran 15 tests in 0.057s

OK
```

Exit 0.

```text
scripts/check-locks.sh
OK: all Eureka locks hold (LOCKS.toml)
```

Exit 0.

```text
scripts/eval/nano-original-ref-ttfa.sh
```

Exit 2. Stdout, one JSON object (pretty-printed here only for the note; the script prints one line):

```json
{
  "gate": "BLOCKED",
  "measurement": "not measured",
  "measured": false,
  "product_claim": false,
  "claim_80ms": false,
  "stub": false,
  "stub_clock": "FAIL",
  "engine": null,
  "frame_bar": "not-measured",
  "frame_bar_ms": 80,
  "host": "cursor",
  "locks_edited": false,
  "refs_sha256_edited": false,
  "measured_file": null,
  "blockers": [
    "A0: samantha t273 digest is not the active REFS.sha256 allowlist",
    "A0: jarvis t273 digest is not the active REFS.sha256 allowlist",
    "A1: samantha original ref wav is absent",
    "A1: jarvis original ref wav is absent",
    "A1: torch is not importable",
    "A1: chatterbox is not importable",
    "A1: pinned nano safetensors are absent or not pinned",
    "A1: tokenizer tree is missing or not pinned"
  ]
}
```

There is no `ttfa_ms` on that object. 80 ms is not claimed. Piper was not run. A protocol stub was not run as the product clock.

`cargo test` was not run. This diff does not change Rust. CI on the pull request runs the workspace tests.

## CI

Not observed yet on this head. Filled in after the pull-request checks finish.

`list_workflow_runs` for `gates.yml` on `8980e1a9b421` returned run 43660665688, conclusion `success`. The lane plan also names `playwright` run 43660665724 and `emotion-v2` run 43660665722 as success on that SHA. This worker did not re-fetch those two run objects.

## Decisions for Maciej

1. **A0.** Re-open the t273 originals on the live clone path (a lock-and-ledger edit he makes), or state that main stays on the t623u CC0 pins and the symphony landing bar must be re-stated. This PR does not choose. Until then the harness stays BLOCKED and will not time those refs.
2. Do not read a Piper clock (including the cited cold 398 ms / warm 20 ms / loopback 78 ms numbers) as this row. `claim_80ms` stays false until a measured original-ref Nano file has `ttfa_ms` ≤ 80.
3. Live `/one` stays Piper plus the Samantha persona until a clone mouth is actually live. This note does not say the clone is live.

— Maciej Grajczyk

---

© G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
