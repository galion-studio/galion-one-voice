# HORBM notes

Perpetual Open MIT License. (c) 2026 Galion Labs.

Index of `docs/horbm/` on `main` `86f9f0a703a01c73b29e017662ba143af04ef958`. The layer audit is [PLAN-brain.md](PLAN-brain.md). Portability is [PORTABILITY.md](PORTABILITY.md). Research data, what may be committed, and the corpus gate are [RESEARCH-DATA.md](RESEARCH-DATA.md).

## Foundation notes

- [F-breath.md](foundations/F-breath.md) — Lungs from the moment's arousal and the inside factors, not from a category name. Still `horbm-breath`. Duplex breath is still a stand-in (`horbm/crates/horbm-duplex/src/lib.rs`).
- [F-duplex-s26.md](foundations/F-duplex-s26.md) — One `Duplex::tick` steps both bands; overlap other than a listed backchannel stops the me band at 160 ms. The note names one persona. `ConfiguredPersona` loads the configured development persona (`horbm/crates/horbm-duplex/src/persona.rs` `open_persona`).
- [F-duplex-s28.md](foundations/F-duplex-s28.md) — Live adapters on the S26 clock: recognizer, `CoreAffect`, `ProsodyEar`. The note's persona port names one persona. That port is now `ConfiguredPersona`.
- [F-duplex-s33.md](foundations/F-duplex-s33.md) — Echo cleans the mic frame before prosody and the recognizer; the far end is the previous mouth frame. Still `PlaybackEcho` (`horbm/crates/horbm-duplex/src/lib.rs`).
- [F-ear.md](foundations/F-ear.md) — Voice tick order: previous mouth frame, ear, `Brain::turn`, one mouth frame. Still `horbm-voice`. The live host also passes a `Heard` (`voice-stack/local-server/src/live.rs` `ProsodyBrain::turn`). `HorbmTurn::turn` stays the text path.
- [F-emotion-core.md](foundations/F-emotion-core.md) — The moment is valence, arousal, dominance, and approach on an 80 ms frame, not a category list. `Brain::turn_with_ear` still does not call `Brain::step_moment`.
- [F-inner-life.md](foundations/F-inner-life.md) — Four slots each of care, want, and expect, tokens of at most 16 bytes. `Brain::record_inner` now calls `inner_turn` after the clause. `PersonaInner` in `horbm-inner` is the slower layer beside it.
- [F-integration-deep-dive.md](foundations/F-integration-deep-dive.md) — Read-only synthesis of phases 2, 3, and 4, with the PR table as it stood then. Later notes S50 through S76, and [PLAN-brain.md](PLAN-brain.md), are the current wiring.
- [F-moment-voice.md](foundations/F-moment-voice.md) — Map the 80 ms moment onto the mouth and leave the baseline put. Still the moment map in `horbm-voice` (`horbm/crates/horbm-voice/src/lib.rs`).
- [F-mouth-source-filter.md](foundations/F-mouth-source-filter.md) — Shape PCM after Piper or flite, on the 80 ms frame. Still `crates/galion-source-filter`. The brain plan does not retune it.
- [F-persona.md](foundations/F-persona.md) — The character stays put while the moment moves. Honesty is `never_fabricate` and `I don't know.` Boot is now `Persona::load_development` ([S76-universal-persona-boot.md](foundations/S76-universal-persona-boot.md)). `PersonaLine` and `StyleHook::cap_sentences` came after this note.
- [F-pre-clone-analysis.md](foundations/F-pre-clone-analysis.md) — One JSON profile before a clone (schema galion-preclone, version 1). Still `tools/cloning-v2/preclone`. Not a brain turn.
- [F-soak.md](foundations/F-soak.md) — Duplex soak on a generated sine, prosody on, echo on. [S75-soak-rss.md](foundations/S75-soak-rss.md) locks growth at `HEAP_GROWTH_KIB` 128 (`horbm/crates/horbm-soak/src/lib.rs`), live heap bytes, end minus the warmup sample. `VmRSS` is printed and is not gated. The gate is not raised.
- [F-turn-eval-s43.md](foundations/F-turn-eval-s43.md) — Pause takeover, turn latency, and backchannel shape against Full-Duplex-Bench. Still `horbm-eval` `run_stand_in` (`horbm/crates/horbm-eval/src/lib.rs`).
- [F-voice-native-path.md](foundations/F-voice-native-path.md) — `VoiceLoop` on the virtual 80 ms clock. Piper and Vosk stay feature-gated. The live host's ear point is the later path in `voice-stack/local-server/src/live.rs`.
- [F1-cascade-audit.md](foundations/F1-cascade-audit.md) — Cascade timestamps and domain 04 stamps. `crates/galion-cascade` is still a workspace member. The `horbm` crates do not call it.
- [F2-retrieval-audit.md](foundations/F2-retrieval-audit.md) — Retrieval API audit. It says `Brain::turn` is unchanged. Superseded as a map by `asked`, `keys_path`, and `DiaryScan` (`horbm/crates/horbm/src/retrieve.rs`), recorded in S54, S58, and S71.
- [F3-compute-audit.md](foundations/F3-compute-audit.md) — Int8 key file, mapped at open, dotted with `galion-tensor`. Still `horbm-keys`. [S51-hkb1-strings-scale.md](foundations/S51-hkb1-strings-scale.md) adds the version-2 string table and per-key scale. [S57-honest-simd.md](foundations/S57-honest-simd.md) is the kernel report.
- [F4b-dialogue-manager.md](foundations/F4b-dialogue-manager.md) — Layer 4 picks one move after the evidence is in. `TurnIntent` `Correct`, `Forget`, and `Recall`, and `Dialogue::decide_held`, extend it (`horbm/crates/horbm/src/dialogue/intent.rs`, `horbm/crates/horbm/src/dialogue/mod.rs`).
- [F5-conductor-deep-dive.md](foundations/F5-conductor-deep-dive.md) — Conductor floor, the MnE frame, and the clocks. Duplex still uses `galion_conductor::Conductor` (`horbm/crates/horbm-duplex/src/lib.rs`).
- [F6-side-band.md](foundations/F6-side-band.md) — Galion Research off the tick, with the 3 second cap. Still `RESEARCH_CAP` and the ask after `TurnOut` is dropped (`horbm/crates/horbm/src/lib.rs`). [S50-local-clause-first.md](foundations/S50-local-clause-first.md) is that order.
- [F7-learn-deep-dive.md](foundations/F7-learn-deep-dive.md) — Offline learn pass, deny by default, one sourced row, withdraw leaves none. Still `LearnPass`. `retract_clause`, `SessionSeal::note_taught`, and `HostSession::finish` came later (`horbm/crates/horbm/src/learn.rs`, `voice-stack/local-server/src/host.rs`).
- [F7-memory.md](foundations/F7-memory.md) — Person diary, separate from truth cards, deny until `grant`. Pins, `forget`, and `list_person` are the later store (`horbm/crates/horbm/src/memory.rs`).
- [FP-first-principles-audit.md](foundations/FP-first-principles-audit.md) — A turn is a lookup. No student decode on the tick. Still `LOCKS.toml` `[horbm]`. The cascade lane and the chatterbox copies it names are still in the tree.
- [S27-ear-prosody.md](foundations/S27-ear-prosody.md) — One ear frame is 80 ms, 16 kHz, 1280 samples, sensing on. Still `crates/horbm-ear`. `heard_axes` and the live-host `Heard` came after.
- [S32-end-of-turn.md](foundations/S32-end-of-turn.md) — Prosody plus an ASR partial decides whether the user has finished. Still `eot::Scorer`. `Scorer::push_prosody` now yields a clear fall when there is no partial (`crates/horbm-ear/src/eot.rs`).
- [S37-listener-feel.md](foundations/S37-listener-feel.md) — A listener reading separate from the moment and the baseline. The note names one persona's `hear_frame`. `listen::hear_frame` keeps an id with no profile on `UNIVERSAL` (`crates/horbm-ear/src/listen.rs`).
- [S38-listening-backchannel.md](foundations/S38-listening-backchannel.md) — One short nod while the user holds the turn. The note names one persona. The nod is `VoiceLoop::tick_listen` (`horbm/crates/horbm-voice/src/lib.rs`).
- [S48-native-mouth.md](foundations/S48-native-mouth.md) — Speak `TurnOut.first_clause`, then `rest`, as text, with no PCM. Still `GALION_MOUTH=native` (`voice-stack/local-server/src/lib.rs`).
- [S50-local-clause-first.md](foundations/S50-local-clause-first.md) — `Brain::turn` returns the local clause before Galion Research is asked. Still the order in `horbm/crates/horbm/src/lib.rs` and `Brain::arm_follow`.
- [S51-hkb1-strings-scale.md](foundations/S51-hkb1-strings-scale.md) — HKB1 version 2 stores a string `item_id` and an int8 scale. Version 1 still loads, with no string table and scale 1. Still `horbm-brain` and `horbm-keys`.
- [S52-cascade-portable.md](foundations/S52-cascade-portable.md) — Cascade speech workers sit behind `cfg(unix)`. `crates/galion-cascade` is still a member. The brain crates do not call it.
- [S53-timing-gates.md](foundations/S53-timing-gates.md) — The one-percent tick budget is 800 µs, judged at the 99th percentile. Still `TICK_BUDGET` (`crates/galion-ml-voice/src/artifact.rs`).
- [S54-diary-scan.md](foundations/S54-diary-scan.md) — `DiaryScan` scores HKB1 and a person-diary arena with `dot_i8_for`. The gap this note left (the thread did not call the scan) was closed by S58 and then split by S71: scan only when a key file is passed (`horbm/crates/horbm-brain/src/scan.rs`).
- [S55-retire-live-demo.md](foundations/S55-retire-live-demo.md) — Retire `galion-live-demo` and keep `galion-heartbeat`. The crate is `archive/2026-11-25/crates/galion-live-demo/ARCHIVED.md`. The live loop is `horbm-duplex` and `scripts/horbm-e2e-smoke.sh`.
- [S57-honest-simd.md](foundations/S57-honest-simd.md) — `galion-tensor` reports the kernel it will actually run. Scalar stays the oracle. Still the rule `horbm-keys` cites (`crates/horbm-keys/src/lib.rs`).
- [S58-diary-scan-wired.md](foundations/S58-diary-scan-wired.md) — A granted ask always built a `DiaryScan`, and a missing key path failed the grant. Superseded by [S71-retrieval-key-scan.md](foundations/S71-retrieval-key-scan.md): unset `keys_path` stays `asked_lexical` (`horbm/crates/horbm/src/retrieve.rs` `asked`).
- [S67-scan-open-capped.md](foundations/S67-scan-open-capped.md) — `DiaryScan::open_capped` calls `open` with `SCAN_CAP` (4). Still that function. S71 is the caller on the retrieval thread.
- [S71-retrieval-key-scan.md](foundations/S71-retrieval-key-scan.md) — `keys_path` `None` is lexical search. `Some`, with `remember` granted, builds `DiaryScan::open_capped` at setup. Still `asked`. `RetrievalThread::reload` and `set_learned` (`LEARNED_ROW_CAP` 256) extend the same thread.
- [S72-person-file-grant.md](foundations/S72-person-file-grant.md) — `EpisodicStore::open_from_flag` opens the file only for the trimmed word `grant`. Still that function. `Brain::open` calls it (`horbm/crates/horbm/src/brain.rs`).
- [S73-hold-topic.md](foundations/S73-hold-topic.md) — `decide` calls `decide_held` with the hold flag clear. A hold turns `OfferTopic` into `Ask`. `Brain::turn_with_ear` passes `DialogueLean.hold_topic`.
- [S74-session-seal.md](foundations/S74-session-seal.md) — One RAM clause, `seal` writes once, and `Brain::turn` does not call it. Still true. `note_taught` and `HostSession::finish` seal at session finish (`voice-stack/local-server/src/host.rs`).
- [S75-soak-rss.md](foundations/S75-soak-rss.md) — Soak growth is live heap bytes, and the gate stays 128 KiB. `HEAP_GROWTH_KIB` is 128 (`horbm/crates/horbm-soak/src/lib.rs`). `VmRSS` is printed and is not gated. A later granted host soak uses that same gate.
- [S76-universal-persona-boot.md](foundations/S76-universal-persona-boot.md) — Development boot is `Persona::load_development` and `BootConfig`, not a loader named for one persona. Still `horbm/crates/horbm-persona/development.toml`. The note's remark that this plan names one persona is out of date: the audit follows this note. `PersonaLine` and the sentence cap came after it.
- [phase4-brain-deep-dive.md](foundations/phase4-brain-deep-dive.md) — Phase 4 picks one stored clause. Still `horbm-brain`. The turn in crate `horbm` is the later brain. [PLAN-brain.md](PLAN-brain.md) is the current layer audit.

## Crates

Each line is the opening of that crate's `lib.rs`, in this tree's words where the comment names a persona.

- `horbm` (`horbm/crates/horbm/src/lib.rs`) — One HORBM turn. Member of `horbm/Cargo.toml`.
- `horbm-brain` (`horbm/crates/horbm-brain/src/lib.rs`) — Phase 4 brain: one stored clause, the HKB1 file, and `DiaryScan`.
- `horbm-breath` (`horbm/crates/horbm-breath/src/lib.rs`) — The configured development persona's lungs. Emotion stays an open point on the 80 ms clock.
- `horbm-duplex` (`horbm/crates/horbm-duplex/src/lib.rs`) — Closed full-duplex loop. One `Duplex::tick` is one 80 ms frame.
- `horbm-e2e` (`horbm/crates/horbm-e2e/src/lib.rs`) — Ear, brain, emotion, mouth, and an output sink. Own workspace (`horbm/crates/horbm-e2e/Cargo.toml`), not a member of `horbm/Cargo.toml`. `scripts/horbm-e2e-smoke.sh` is the entry. The crate comment names a persona; this index does not.
- `horbm-eval` (`horbm/crates/horbm-eval/src/lib.rs`) — Closed evaluation gate for the HORBM layers that are on main.
- `horbm-inner` (`horbm/crates/horbm-inner/src/lib.rs`) — Inner life for whatever persona the host configured.
- `horbm-persona` (`horbm/crates/horbm-persona/src/lib.rs`) — The steady character. A persona file is TOML, read once at boot.
- `horbm-soak` (`horbm/crates/horbm-soak/src/lib.rs`) — Long-call soak for the configured development persona's duplex loop. `HEAP_GROWTH_KIB` is 128.
- `horbm-voice` (`horbm/crates/horbm-voice/src/lib.rs`) — HORBM-native voice turn. One tick is one 80 ms conductor frame.
- `horbm-ear` (`crates/horbm-ear/src/lib.rs`) — Ear-side prosody. One call is one 80 ms frame, 16 kHz, 1280 samples. Sensing starts on. Member of the root workspace.
- `horbm-keys` (`crates/horbm-keys/src/lib.rs`) — Mapped int8 key arena. Member of the root workspace.
- `horbm-device` (`crates/horbm-device/src/lib.rs`) — Device profile at boot: thread counts, frame, compute budget, backends. The NVIDIA probe is Linux-only behind `cfg`. Member of the root workspace.

Research cards stay off this repo. The rule is [RESEARCH-DATA.md](RESEARCH-DATA.md).
