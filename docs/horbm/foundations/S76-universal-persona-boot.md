# S76 — Universal persona boot

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM S76, universal persona boot |
| Written | 2026-11-25. No weight download, no voice audio, no paid API, no merge. |
| Base | `main` `fea54fb9af9eb098170c443cd7d67a556a113638` (PR #330) |
| Spec | Mate t1855u/t1856u. Development boot loads whatever id `BootConfig` names. `docs/horbm/PLAN-brain.md` (PR #327) still says boot is one named persona; this note follows the later rule. |
| This change | `horbm-persona` boot loader, `Brain::boot` / `Brain::with_cards` / `development_persona`, the persona tests, and this note. No new dependency. |
| Not edited | `LOCKS.toml`, `SOUL.md`, `LEDGER.md`, workflows, root and horbm workspace manifests, `Cargo.lock`. `horbm-inner`'s `PERSONA` const, listed below. `SOUL.md` is not in this tree. |

## What it decides

Development boot no longer calls a loader named for one persona. `Persona::samantha` and `SAMANTHA_TOML` are gone.

`Persona::load_development` reads `BootConfig`. `GALION_HORBM_DEV_PERSONA` wins when it is a non-empty id. Otherwise the id is the `development_persona` key in `development.toml`. The file is `personas/<id>.toml` under the persona root. The id inside the file must equal that configured id (`Persona::is_configured_boot`). A blank id, a missing `development.toml` when the environment is also unset, or a missing file returns `PersonaError`. It does not panic.

The root is `GALION_HORBM_PERSONA_DIR` when that value is a non-empty path. Otherwise it is this crate's directory, recorded at compile time with `CARGO_MANIFEST_DIR`. `development.toml` in the repository is still the only assignment of the development id. The shipped file names the same persona the laptop already boots. `personas/samantha.toml` and `personas/aria.toml` stay data files. Aria is not selected unless the config names that id.

`Brain::boot(..., None)` uses that loader. `Brain::boot(..., Some(path))` still reads the given file and does not check it against the configured id. `Brain::with_cards` and `Brain::open` use the same loader. Their signatures still return `Self`, because callers outside this change construct a brain that way. A missing configuration aborts those two constructors. `Brain::boot` resolves the persona before `open`, so the error path returns `PersonaError` and does not reach that abort.

Nothing on the turn or the retrieval query reads the persona file. Consent is unchanged: `remember` deny, `learn` deny, `learn_in_band` false. No research card was added.

## `horbm-inner` `PERSONA`

`horbm/crates/horbm-inner/src/lib.rs` still has `pub const PERSONA: &str = "samantha"`. It cannot become config-driven from this change. It is a const, the home floats are taken from `galion_emotion::SAMANTHA` in another crate, and `horbm` re-exports it as `SAMANTHA_INNER`. `horbm-inner` does not depend on `horbm-persona`. Pointing the const at `BootConfig` would add that dependency and edit crates this swarm does not own. Left as it is.

## Tests

`boot_installs_the_persona_the_env_names` points `GALION_HORBM_DEV_PERSONA` at a temporary id and `GALION_HORBM_PERSONA_DIR` at a directory that holds `personas/<id>.toml`. `Brain::boot(..., None)` installs that id. Its baseline identity bits and valence differ from the shipped development file. `Brain::with_cards` installs the same id. An explicit path still installs the file it names.

`boot_refuses_when_nothing_is_configured` points the directory at an empty temp dir and unsets the id variable. Boot returns `development persona is not configured` and does not panic. The same error is returned for a blank id. A configured id with no file returns `cannot read persona file`. A file whose id does not match the configured id is refused.

`boot_without_a_file_loads_samantha` unsets both variables. Boot, open, and `with_cards` still install the id named in the shipped `development.toml`.

Nothing added here is Linux-only. Paths use `std::path`. The persona root baked into the binary is the crate directory on the machine that compiled it. A relocated binary needs `GALION_HORBM_PERSONA_DIR` or that directory still present. The run below is Linux, rustc 1.85.0 (`4d91de4e4`, 2025-02-17).

| Command | Result |
|---|---|
| `cargo test --manifest-path horbm/Cargo.toml -p horbm-persona --test persona` | 16 passed |
| `cargo test --manifest-path horbm/Cargo.toml -p horbm --test persona` | 5 passed |
| `cargo test --manifest-path horbm/Cargo.toml --workspace` | 57 suites, 282 passed, 0 failed, 2 ignored |
| `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` | 114 suites, 1069 passed, 0 failed, 5 ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |

The two ignored horbm tests are `timed_scan_of_one_thousand_rows` and `real_card_folder_is_read_and_not_written`. `cargo test` at the repo root rewrote `voice-stack/Cargo.lock` while resolving horbm's path dependencies. That file is outside this change and was restored. No `Cargo.toml` and no lockfile is committed. Current `main` at the test run was `26239c9` (PR #327, the brain plan), merged into this branch before the workspace run.
