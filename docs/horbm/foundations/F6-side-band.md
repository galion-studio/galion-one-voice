# F6 — Galion Research side band

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM R4 phase 6, Galion Research side band |
| Written | 2026-11-24. No weight download, no voice audio, no paid API, no merge. |
| Parent | Phase 5 conductor, PR #254, branch `cursor/horbm-phase5-conductor-5ab1`, commit `577c333a4177228e760fead27dc14c2ba4593a4b`. This pull request stacks on #254. |
| Plan | `docs/horbm/research/R4-cpu-realtime-integration.md` §11 on draft PR #207. The draft is closed and unmerged. Phase 6 there is the side band: the local clause plays, a deeper answer may arrive later, and the cap is 3 seconds. |
| This change | `crates/galion-conductor` (`src/research.rs`, the conductor tick, the phase 6 gate), this note, one pointer in `F5-conductor-deep-dive.md`, and one new gate in `LOCKS.toml`. `scripts/check-locks.sh` requires that gate. |
| Mark, 2026-11-25 | The conductor posts the side-band mark when a Galion Research request starts and ends. Types live in `crates/galion-sideband` (`src/mark.rs`). `LOCKS.toml` was not edited again. |

The work was split four ways, then written in this tree: the client types and the cap rule, the conductor wiring, the virtual-clock gate, and the lock plus this note.

## How to read a number

| Tag | Meaning |
|---|---|
| **[D]** | A constant already named in `LOCKS.toml` or in this crate. |
| **[T]** | A target in R4. Not a wall-clock measurement. |
| **[V]** | A result on the virtual frame clock. The counter moves only when a test calls `tick`. |

## 1. What phase 6 asks

R4 §11 phase 6, in the words this crate uses: a lookup may ask Galion Research while the conductor keeps the live turn. The local brain answers first. A reply is merged only inside the 3 second cap. Otherwise the turn continues on the local clause. A money action is not sent. The tick test still shows no research call inside the heartbeat frame function.

The swarm rules on top of that note:

- The side band never blocks the voice path.
- Merge only if the answer's arrival on the virtual frame clock is within 3 seconds. Otherwise drop it, log the drop, and keep going.
- Barge-in still yields within 200 ms, including while a request is pending, and that yield cancels the request.
- A listed backchannel never cancels, including `yeah` while a request is pending.
- CPU-only by default. Fully offline. If Galion Research is unreachable, the side band reports unavailable on the ask itself and the turn does not gain a frame.
- Tests use a fake client behind a trait. No paid API.

## 2. What landed

`ResearchClient` is the trait. `ask`, `poll`, and `cancel` return without sleeping and without a socket. `OfflineClient` is the default inside `Conductor::new`. `FakeResearch` scripts a virtual delay. Neither is a network client.

The conductor asks only after `ClauseBrain::answer` has returned, and only when `classify` says the partial is a lookup. The rule is the same lexical test as `horbm::is_deep_question` (a question mark, and either eight words or why / how / explain). The `horbm` crate is a separate workspace on a newer rustc and is not linked. An imperative payment (`send money`, `wire money`, `transfer money`, `make a payment`, and no question mark) is refused and the client is not called. The trait has no action method, so a money action cannot be posted. A question that mentions a payment is a lookup, not an order.

`tick` does not wait. On the eager frame the local pick is stored and the ask returns. The clause stays behind the gate until the confirm frame, as in phase 5. `research_clause` fills only when a result merges. The local `clause` string is left as the brain wrote it.

The cap is [`SIDE_BAND_CAP_MS`](../../../crates/galion-conductor/src/clock.rs) = 3000, the same number as `LOCKS.toml` `[gates] side_band_cap_ms`. The bound is inclusive: an arrival at 3000 ms merges, an arrival at 3100 ms does not. The answer carries its own elapsed time from `ask`. A still-pending ask is dropped on the first frame whose wait is past 3000 ms, and the client is told to cancel so a later body cannot land. The trace line is counts and an event name. It does not contain the question or the answer.

Barge-in and a resumed speculation both cancel a pending ask and clear a merged sentence. A listed backchannel does not. The phase 5 silence schedule is unchanged: silence on the frame that contains 200 ms, which is 160 ms after onset on this clock.

This crate still does not name the heartbeat frame function, and it still does not read a wall clock. The first landing did not link `galion-sideband`, because that client times the cap with a wall clock and speaks the old access-layer name. A virtual-clock gate cannot sit on that clock. Section 7 links the mark types only. The conductor still does not call that client, and it still does not use that name. No runtime was added. The trait is the place a later local engine can plug in. Nothing here opens a socket.

`RUNS_INSIDE_HEARTBEAT_STEP` stays `false`.

## 3. Phase gate

Host: Linux, rustc 1.83.0. `cargo clippy --manifest-path crates/galion-conductor/Cargo.toml --all-targets -- -D warnings` passed. No `RUSTFLAGS`. No `target-cpu`.

```text
cargo test --manifest-path crates/galion-conductor/Cargo.toml -- --nocapture
```

`bash scripts/check-locks.sh` prints `OK: all Eureka locks hold`.

29 tests passed: 10 in the library (4 of them the cap, the classifier, the fake clock, and the offline client), 9 in `tests/phase_gate.rs`, 10 in `tests/phase6_gate.rs`. The phase 5 lines still print. The phase 6 lines:

```text
phase6 gate: arrival_ms=2900 cap_ms=3000 clock=virtual merged=true result=pass
phase6 gate: arrival_ms=3100 cap_ms=3000 clock=virtual merged=false dropped=true drop_elapsed_ms=3040 result=pass
phase6 gate: barge_in_ms=160 limit_ms=200 clock=virtual pending_cancelled=true result=pass
phase6 gate: backchannel=yeah frames=6 frame_ms=80 cancel=false request_cancelled=false result=pass
phase6 gate: offline=unavailable added_latency_ms=0 clock=virtual result=pass
```

`drop_elapsed_ms=3040` is the frame wait, not the scripted arrival. The ask starts on the eager frame (160 ms for this fixture). The 3100 ms answer is not ready yet when the wait first passes 3000 ms, so that frame drops it. `arrival_ms=2900` is the answer's own stamp. The frame that notices it is later, and 2900 is inside the cap, so it merges. `barge_in_ms=160` is the same mixer schedule as phase 5. `added_latency_ms=0` is the commit time of an offline lookup minus the commit time of a local kettle hit. Both commit at 240 ms.

## 4. Laptop test

From `G:\galion\work\galion-one-voice`, after this branch is checked out. These commands do not download a voice, do not open a microphone, and do not merge. Rust 1.83 or newer. The tests do not open a socket.

```powershell
Set-Location G:\galion\work\galion-one-voice
cargo test --manifest-path crates\galion-conductor\Cargo.toml -- --nocapture
if ($LASTEXITCODE -ne 0) { throw "conductor gate failed" }
cargo clippy --manifest-path crates\galion-conductor\Cargo.toml --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { throw "conductor clippy failed" }
bash scripts/check-locks.sh
if ($LASTEXITCODE -ne 0) { throw "locks failed" }
```

Pass is the five `phase6 gate:` lines above, the two `phase5 gate:` lines, and `OK: all Eureka locks hold`. `barge_in_ms` is at most 200. The backchannel line has `cancel=false` and `request_cancelled=false`. The offline line has `added_latency_ms=0`. Do not set `RUSTFLAGS` to `-C target-cpu=native`.

## 5. LOCKS.toml and SOUL.md

Owner rule 2026-11-24 19:15 allows an edit when the architecture needs it.

`LOCKS.toml` changes, and only this:

1. New key `[gates] side_band_cap_ms = 3000`. `scripts/check-locks.sh` requires that key to be 3000.

No other lock value moved. `barge_in_max_ms` stays 200. `backchannel_cancels` stays false. Consent stays `deny` on every bit. `prohibited_overridable` stays false. `formats.exceptions.v2s14_chatterbox.reference_clips` stays the existing owner pin.

`SOUL.md` is not in this checkout. It was not created. The conductor does not define a persona. On a drop or an unavailable engine the local clause keeps playing. The persona file `docs/galion-voice-v2/personas/GALION-RESEARCH.md` already has the words for a timeout and an unreachable engine. This crate does not speak those sentences, and that file was not edited.

## 6. Left open

- A real local engine can implement `ResearchClient`. This crate does not ship one, and it does not bind a port.
- `openmind-sandbox` is not called. Tools stay off this path.
- PR #252 is still the real clause pick. `FakeBrain` is the stand-in.
- The root workspace does not build `galion-conductor`. `galion-mne-contract` is its own workspace root, same as phase 5. `galion-sideband` is a root-workspace member.
- The `horbm` mark still records `posted: false`. This pass does not edit that crate. The post is the conductor log in section 7.
- Do not merge the original phase 6 pull request from this note. Auto-merge stays off.

## 7. Side-band mark

The gap in `F-integration-deep-dive.md` was that phase 6 did not post the side-band mark. The conductor now does, on the same virtual frame that opens or closes the Galion Research request. The 3 second cap is unchanged. The mark is the wire `galion-sideband` already uses. `SidebandClient` is not called, so the cap is not moved onto a wall clock.

| When | Mark | Wire |
|---|---|---|
| Lookup ask opens | start, `posted = true` | `ToHermes::Intent` with `Intent::Lookup { query }` |
| Body arrives at or under 3000 ms | end, merged | `FromHermes::Knowledge`, source `research` |
| Wait passes 3000 ms, or the body itself is past the cap | end, dropped | `FromHermes::NoKnowledge { reason: Timeout }` |
| Barge-in, or a resumed speculation | end, cancelled | `FromHermes::NoKnowledge { reason: Cancelled }` |
| Engine reports unavailable on the ask | start, then end on that same frame | `FromHermes::NoKnowledge { reason: Unreachable }` |
| Not a lookup, or an imperative payment | no mark | the client is not called |
| `SidebandConsent::Deny` | no mark | the ask still runs |

`Cancelled` is a new reason on the existing `NoKnowledgeReason` enum. The other four reasons are unchanged. The source string is `research`. It is not a URL and it is not an address.

`REMEMBER_LOCK` and `LEARN_LOCK` are `"deny"`, the same strings as `LOCKS.toml` `[consent] remember` and `[consent] learn`. `LEARN_IN_BAND` is false. Posting a mark does not grant remember, does not grant learn, and does not teach. The default session is `SidebandConsent::Post`, so a lookup writes the start and the end. `set_sideband_consent(SidebandConsent::Deny)` writes neither. `LOCKS.toml` was not edited for this.

The trace line is still counts and an event name. The question and the answer are on the mark, because that is the lookup and knowledge wire, and they are not copied into the trace.

Host for this pass: Linux, rustc 1.85.1. No `RUSTFLAGS`. No `target-cpu`.

```text
cargo test --manifest-path crates/galion-sideband/Cargo.toml
cargo test --manifest-path crates/galion-conductor/Cargo.toml -- --nocapture
```

`galion-sideband` is a root-workspace member, so the first command is run from the repo root as `cargo test -p galion-sideband`. 13 tests passed: 3 client, 5 lang, 4 mark, 1 types.

`galion-conductor` is its own workspace. 50 tests passed: 13 in the library, 8 in `tests/backchannel.rs`, 1 in `tests/ear_prosody.rs`, 3 in `tests/end_of_turn.rs`, 10 in `tests/phase6_gate.rs`, 9 in `tests/phase_gate.rs`, 6 in `tests/sideband_mark.rs`. The phase 6 lines in section 3 still print. The mark lines:

```text
phase6 mark: posted=true edge=start+end arrival_ms=2900 cap_ms=3000 result=pass
phase6 mark: posted=true edge=end kind=dropped elapsed_ms=3040 cap_ms=3000 result=pass
phase6 mark: posted=true edge=end kind=cancelled barge_in_ms=160 result=pass
phase6 mark: consent=deny posted=false learn_in_band=false result=pass
```

`elapsed_ms=3040` is the same frame wait as section 3. `barge_in_ms=160` is the same mixer schedule. Nothing in this pass is Linux-only. There is no `cfg` for the host, no hard-coded path, and no socket.
