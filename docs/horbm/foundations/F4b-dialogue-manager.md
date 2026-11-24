# F4b — Dialogue manager

Perpetual Open MIT License. (c) 2026 Galion Labs.

Layer 4 of the HORBM turn. It decides the next move after the evidence is in hand, and it keeps the talk alive until the person ends it. CPU only. No weights, no network, no audio, no file write.

## How to read a number

| Tag | Meaning |
|---|---|
| **[D]** | A design constant in this layer. |
| **[R]** | Read from code on this branch. Not a timing. |

## Where it sits

`Brain::turn` is unchanged up to the gathered clause. **[R]** Empathy (layer 3) steps the mood. The retrieval cell, the card pick, the idle fixture, and the research port still run in that order. Layer 4 then picks a move. `finish` still splits the clause. Barge-in and `RetrievalThread::try_cell` are not this layer. Do not call `Brain::turn` from `Heartbeat::step`.

The state tracker reuses `galion_sam_empathy::NameBook` (32 names). A ring of 32 turns stores mood, the focus name, a topic word, and the clause we spoke. It does not store the heard line and it does not write disk. `Brain::note_name` fills both books, so pronoun rewrite is unchanged.

## Moves

`answer`, `cite`, `ask`, `comfort`, `offer_topic`, `stop`. The heuristic scores them and the legal-move filter masks the rest. Ask always stays legal. **[D]**

| Move | When it wins | Spoken line |
|---|---|---|
| `stop` | Goodbye, or the whole line is `stop` / `please stop` / `be quiet` | `Talk soon.` |
| `cite` | A cite cue, and a real source is already in hand or in the ring | That stored clause |
| `answer` | A real source (`card:`, `retrieve:`, `fixture:`, `research:`) | The gathered clause |
| `offer_topic` | A switch cue with a topic already in the ring, or a granted check-in | `Back to {topic}?` or the stored callback |
| `comfort` | A vent, and no sourced clause | `I don't know.` |
| `ask` | Anything still open, including an answer the filter removed | `I don't know.` |

`honest:unknown` is not a source. An answer without a card does not speak. Comfort does not add a sympathy script. The feeling stays on the mood metadata. A vent that also has a card still answers the card.

## Policy and the bandit

The policy is the heuristic. Same lines, same move, same words. `TurnOut.arm` is the move name. `TurnOut.propensity` is a fixed weight for that move (stop 0.9, answer 0.8, cite 0.7, offer 0.6, comfort 0.5, ask 0.4). **[D]** An empty line stays arm `none` and propensity 0.

`OfflineBandit` is off. `observe` counts a move only when the bandit is enabled and learn consent is granted. The count does not pick the next move. `choose_steered` is the optional count tie-in. `Dialogue::decide` does not call it. Zero counts match the heuristic. An illegal arm stays at `i32::MIN` no matter the count.

## Consent

`DialogueConsent` starts denied. Check-ins start off. A flag string grants only when it is `grant` after trim. `yes` and `true` deny. Check-ins cannot be granted by a flag. The person turns them on with `check ins on`, or the host sets `DialogueConsent.check_ins`. **[D]**

Remember here is the callback list, not the retrieval diary. `note_callback` returns false and stores nothing when remember is denied. Withdrawal clears the callbacks and the ring. A callback is spoken only when remember is granted, check-ins are on, and the line is a short greeting with no sourced clause. The words are the stored text (`how did the interview go?` only if that string was stored). The layer does not invent it.

Learn follows the grant string from PR #255 (`GALION_HORBM_LEARN=grant`). This layer does not parse `LOCKS.toml`. On this branch `[consent]` has no `learn` key yet. That key is PR #255. The code defaults deny anyway.

`LOCKS.toml` was not edited. `SOUL.md` is not in this tree and was not created.

## Gate

```text
bash scripts/check-locks.sh
cargo test --manifest-path horbm/Cargo.toml -p horbm --test dialogue
cargo test --manifest-path horbm/Cargo.toml
cargo test --workspace
```

The dialogue test is the layer gate: each move in its scenario, no answer without a card, consent denied stores no callback and no bandit count, check-ins off by default, and two runs of the same lines match. No audio file. No network in the turn.
