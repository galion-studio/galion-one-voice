# F — persona

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | S19, persona. Roles: spec and loader, legal-move port, voice and style hooks, tests and this note. |
| Written | 2026-11-24. No voice audio, no recording, no weight, no network call. |
| Base | PR #269, branch `cursor/inner-life-layer-0a94`. This change stacks on that PR. The stack's target is `main`. |
| Crate | `horbm/crates/horbm-persona`. `Brain::boot` reads the file and copies the numbers into `galion-emotion-core`. |
| Locks | No change. `LOCKS.toml` `[consent] learn` stays `"deny"`, the same line #255 and #269 add. `SOUL.md` is not in this tree. Nothing was added. |

The persona is the character that stays put. The moment is the point that moves. Traits, honesty, boundaries, and speaking style load once. A step leaves that character and the baseline as they were.

## File

Schema `galion-horbm-persona/1`, `version = 1`. TOML. One file, one speaker, read at boot.

`[traits]` is an open table of short names to numbers from 0 to 1. Eight slots is a capacity. A ninth name is refused, and a word such as `"high"` is refused: the value is a number. Samantha carries warmth, curiosity, play, honesty, and attention. Aria carries those, and also precision and steadiness. She is Samantha's warmth and curiosity with calm precision: playful, honest, emotionally attentive, steady at work. The numbers are design knobs.

`[honesty]` requires `never_fabricate = true` and `unknown = "I don't know."` That sentence is the spoken miss the turn already uses (`UNKNOWN_REPLY`). A file that fabricates, or that picks another miss, is refused.

`[boundaries]` is an open list of short tokens, at most eight. The example files list `fake-sympathy`, `invented-past`, and `unsourced-fact`. A file may list a different set. The stand-in lists the same three.

`[style]` is brevity and warmth, each from 0 to 1, and `max_sentences` from 1 to 4. The voice hook is those three numbers plus a rate, `1 - 0.15 * brevity`. The hook does not read the moment. It carries no audio.

`[baseline]` is valence, arousal, dominance, approach, and `pitch_centre_hz`. The ranges match the emotion core. Pitch 216 is the design centre (432 / 2), passed through as a value, and is the same centre the stand-in uses. `Persona::baseline` copies the five numbers into a `VoiceBaseline`. The core stores that copy. The moment swings around it.

`[inner]` is three tokens, each 1 to 16 lowercase letters: care, want, expect. `Persona::inner_life` passes them to `InnerLife::from_prose`. A single word is used as written, so `focus` becomes the care. Samantha uses `heard`, `answer`, and `pause`, which fit the inner-life scanner. `Brain::open` and `Brain::boot` with no path parse `personas/samantha.toml` (`Persona::samantha`). That is the development persona. An explicit path replaces her. An unreadable or invalid file is refused. Boot returns the error and leaves no brain.

`personas/aria.toml` stays in the repo for later. It is inactive. Boot does not load it. Both files are design knobs. They carry no measured person, no performer, no donor, no clip, and no address. `VoiceBaseline::standin` remains the emotion-core fixture, not the speaker HORBM boots.

## Port

`LegalMovePort::verdict` is the call a dialogue manager makes for one candidate move. This crate publishes the port. A move is a `MoveClaim`: a tag, whether it claims a fact, whether a card is attached, and whether it is a sympathy script. When `fake-sympathy` is listed, a sympathy script is forbidden. When `unsourced-fact` is listed, a fact with no card is forbidden. A fact with a card is allowed. Any other listed tag is refused the same way. `Brain::admit_move` forwards the claim. The call does not allocate. `TURN_ALLOC_BUDGET` is 0. The same claim twice is the same verdict.

```shell
cargo test --manifest-path horbm/Cargo.toml
cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
bash scripts/check-locks.sh
```
