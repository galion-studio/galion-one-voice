# Emotion storage and unfinished work

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
2026-11-08. Docs only. Tip read: `origin/main` `3e1f5a12c53cd8e0dd47d111676d61d608540559`.

This note records what the current tree executes, what is only a name, and
what is still missing. It does not add a classifier, a database, or an
empathy path. It does not edit `LOCKS.toml`.

## Not read

`/opt/galion` is absent. `/opt/galion/SAMANTHA-V2-BRIEFING.md` was not read.
The existing key to `galion@51.38.137.193` was rejected. No SSH retry.

Files copied on 7 November around 21:16 CET whose own dates are 1–3 November
are not a Samantha handoff from the evening of 7 November. A storage journal
dated 7–8 November, where one exists outside this checkout, is an
infrastructure migration plan by another executor. It is not proof that an
emotion store was enabled or populated.

Owner plan named in this task, not a lock change: Research Ultra $220 and
One $20. `LOCKS.toml` still has `paid_tiers = false`. No billing code is
added here.

## Live detector

`galion-emotion/README.md` and `galion-emotion/src/detect.rs` on this tip:

- The live inbound detector is `EnergyStubDetector` over
  `GatedDetector<LoudnessEstimator>`.
- The features are RMS and zero-crossing rate.
- Arousal and dominance come from loudness.
- Valence is not invented from those features. It stays 0 and
  `valence_known()` is false unless another estimator already passed an
  opinion (`Estimate.valence: Option<f32>`).
- The gate runs before PCM is read. A refused gate returns `NotPermitted`
  and does not infer.
- The file says a later learned head would be another `AffectEstimator`.
  That head is not in this crate. No weights are loaded.

`ReferenceEstimator` replays an offline label track. The fixture
`galion-emotion/fixtures/affect-ref-synthetic.txt` is hand-written. It is
not a model output and not a populated store.

## What executes, versus a name

### Rust counterbalance

`StubExpressor::respond` in `galion-emotion/src/express.rs` is the outbound
policy that runs. Angry, anxious, sad, and tired are answered with lower
arousal and positive valence. Joy and excited take a fraction of the
caller's positive valence and arousal (`0.9` and `0.85`), floored at the
neutral-warm base. That is the only shared positive case. The function does
not copy the caller frame.

`StubExpressor::voice_color` calls that policy and builds the empathy weight
`e`, prosody, and OKLCH. Call sites that execute it, not merely name it:

- `galion-emotion/src/duplex.rs`
- `galion-emotion/src/film.rs`
- `galion-emotion/src/persona.rs`
- `crates/galion-console/src/dotviz.rs`
- `crates/galion-console/examples/dot_console_demo.rs`

Those calls do not make the RMS detector a learned classifier. If the
inbound frame has no valence opinion, `e` stays 0 for a low-confidence or
empty caller (`express.rs` tests: `voice_color(None).e == 0`, low
confidence is not a guess).

### Python `mirror`

`galion-demo/galion_demo.py` `respond()` is the port of
`StubExpressor::respond`. The turn path uses `respond()`, not `mirror()`.

`mirror()` returns `dict(c)`. Its docstring says it is the old canned
behaviour, kept so the self-test can reject it. `check_counterbalance`
returns a violation when the target valence is negative or when anger or
anxiety is met with arousal at least as high as the caller's.
`tests/test_galion_demo.py` asserts the counterbalanced target passes and
`mirror(c)` fails.

`docs/agent-wave20/W20-04-STATUS.md` (2026-11-02) still says the Python demo
mirrors and that the A14 fix is open. That sentence does not match the
current `galion_demo.py`. The Rust policy and the current Python `respond()`
agree. `mirror` is a name of the rejected function, not the function the
demo speaks with.

## Consent bits

`LOCKS.toml` `[consent]`:

```text
sense = "deny"
adapt = "deny"
remember = "deny"
share = "deny"
withdraw_wipes_ram_history = true
rika_requires = ["sense", "adapt"]
```

`galion-emotion/src/band.rs` `consent_bits`:

| Bit | Value | Meaning in that file |
|---|---|---|
| `SENSE` | `1 << 0` | May affect be inferred from the caller? |
| `ADAPT` | `1 << 1` | May inferred affect steer prosody? |
| `REMEMBER` | `1 << 2` | May a baseline persist past the session? Comment: default off. |
| `SHARE` | `1 << 3` | May the stream leave the voice runtime? |

`encode_with_consent`: if the direction is inbound and `SENSE` is unset, the
frame argument is replaced with `None`. The wire does not carry a guessed
caller emotion.

`DefaultConsentGate::denied()` starts at `ConsentState::Denied`. `check`
returns `DenyNoConsent` for Unknown or Denied, `DenyWithdrawn` after
withdraw, and `DenyProhibitedPurpose` for workplace and education even if
someone calls `grant` with those purposes. Tests: `default_is_deny`.

`AffectStore::adapt_target` returns `None` unless `ADAPT` is set.
`share_frame` returns `None` unless `SHARE` is set and
`confirm_share_spoken` has run. `baseline_for_persist` returns `None` unless
`REMEMBER` is set and the baseline count is non-zero.

## Persistence: plan versus proof

`AffectStore` (`galion-emotion/src/store.rs`) is an in-memory ring.

- `Drop` for `Inner` wipes the ring and the baseline. The comment says
  persisting the baseline is the host's job, and only with `REMEMBER`, via
  `baseline_for_persist()` **before** drop.
- This pass found no disk, SQLite, or file write in that store.
- `rika.rs` says RAM only, wiped on withdraw.
- `consent.rs` says affect is computed on device and never persisted by
  default.

So the implementation that exists is a RAM ring plus a gate that can hand a
baseline to a host. There is no proof in this tree that a host enables that
hand-off, writes it, or reloads it. A plan that says "memory" or "emotion
store" is not this proof.

`voice_color` does not write that store. The nebula live feed
(`docs/emotion-v2/NEBULA-LIVE-FEED.md`) smooths a posterior it is given. It
does not score audio and does not commit audio.

## Uncommitted Nebula work (not in this commit)

On branch `cursor/nebula-v3-integration-b3fd`, the working tree still has
untracked files that this docs branch does not contain and must not stage:

| Path | State |
|---|---|
| `docs/nebula-v3/DESIGN.md` | untracked |
| `docs/nebula-v3/ROSTER.md` | untracked |
| `apps/nebula-voice-v2/src/v3/mod.rs` | untracked |
| `apps/nebula-voice-v2/src/v3/input.rs` | untracked |
| `apps/nebula-voice-v2/src/v3/field.rs` | **absent** |

`input.rs` parses a live-feed or frame JSON into a hold or a frame. It does
not run the RMS detector. `field.rs` was the planned gas step and was not
written. No 120 fps result exists for that work. CPU step time would not
have been a 120 fps pass: this machine has no `nvidia-smi` and no
`/dev/dri`.

Those files stay dirty in the integration worktree. They are not in the
commit that adds this note.

## Gaps

1. No learned inbound head. Live valence from audio is 0 unless a fixture or
   another estimator supplies it.
2. No host was shown saving `baseline_for_persist`. Session drop wipes RAM.
3. `REMEMBER` and `SHARE` default deny. Share also needs a spoken
   confirmation stub, not a recording.
4. November 2 status text still says the Python demo mirrors. The current
   demo does not speak `mirror()`.
5. Empathy weight `e` runs only where `voice_color` is called. It is not a
   classifier and not a stored biography.
6. Nebula V3 gas field is unfinished and unpublished. The time-blend in
   `apps/nebula-voice-v2/src/demo.rs` `frame_at` is still the explicit demo
   path on `main`. It is not a live sense grant.
7. VPS briefing unread.

## Acceptance, when someone later builds

Not done in this change.

- A live mic frame with default consent produces no inbound valence. The
  test already expected by `detect.rs`: RMS does not invent valence.
- After `withdraw`, `AffectStore` RAM is empty and `baseline_for_persist`
  is `None`. A new process does not reload a file, because no file is part
  of the contract yet.
- `galion_demo.respond` stays the counterbalance port. A test that calls
  `mirror` must still fail `check_counterbalance` for angry, anxious, sad,
  and tired.
- A nebula frame with `present: false` or without `sense` does not paint a
  time-blend and does not open a database.
- Any 120 fps claim names a presented GPU or browser frame. A CPU gas step
  is not that claim.
