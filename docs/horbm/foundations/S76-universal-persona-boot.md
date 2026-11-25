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

`Persona::load_development` reads `BootConfig`. `GALION_HORBM_DEV_PERSONA` wins when it is a non-empty id. Otherwise the id is the `development_persona` key in `development.toml`. The directory of `<id>.toml` files is `GALION_HORBM_PERSONA_DIR` when that value is non-empty. Otherwise it is the `persona_dir` key in `development.toml`, joined to the directory that holds that file. If neither source is present, the error is `development persona directory is not configured`. No absolute crate path is compiled into the binary. The boot file is found by walking from the process working directory. The id inside the loaded file must equal the configured id (`Persona::is_configured_boot`). A blank id or a missing file returns `PersonaError`. It does not panic.

`development.toml` in the repository is still the only assignment of the development id. Its `persona_dir` is `personas`, so a process that can see that file still loads the same persona the laptop already boots. `personas/samantha.toml` and `personas/aria.toml` stay data files. Aria is not selected unless the config names that id.

`Brain::boot(..., None)`, `Brain::try_open`, and `Brain::try_with_cards` use that loader and return `PersonaError` when it fails. `Brain::boot(..., Some(path))` still reads the given file and does not check it against the configured id. `Brain::with_persona` and `Brain::open_with_persona` take an already loaded persona and do not read configuration. `Brain::with_cards` and `Brain::open` keep the signatures callers outside this change already use. When configuration is present they install that persona. When it is missing they install the stand-in fixture. They do not panic.

Nothing on the turn or the retrieval query reads the persona file. Consent is unchanged: `remember` deny, `learn` deny, `learn_in_band` false. No research card was added.

## `horbm-inner` `PERSONA`

`horbm/crates/horbm-inner/src/lib.rs` still has `pub const PERSONA: &str = "samantha"`. It cannot become config-driven from this change. It is a const, the home floats are taken from `galion_emotion::SAMANTHA` in another crate, and `horbm` re-exports it as `SAMANTHA_INNER`. `horbm-inner` does not depend on `horbm-persona`. Pointing the const at `BootConfig` would add that dependency and edit crates this swarm does not own. Left as it is.

## Tests

`boot_installs_the_persona_the_env_names` points `GALION_HORBM_DEV_PERSONA` at a temporary id and `GALION_HORBM_PERSONA_DIR` at a directory that holds `<id>.toml`. `Brain::boot(..., None)` installs that id. Its baseline identity bits and valence differ from the shipped development file. `Brain::try_with_cards` installs the same id. An explicit path still installs the file it names.

`boot_refuses_when_nothing_is_configured` points the directory at an empty temp dir and unsets the id variable. `boot`, `try_with_cards`, and `try_open` return `development persona is not configured` and do not panic. `with_cards` and `open` return a brain whose persona is the stand-in fixture. The same boot error is returned for a blank id. A configured id with no file returns `cannot read persona file`. A file whose id does not match the configured id is refused.

`boot_without_a_file_loads_samantha` unsets both variables. The boot file is found from the working directory. Boot, open, and `with_cards` still install the id named in the shipped `development.toml`.

Nothing added here is Linux-only. Paths use `std::path`. The binary does not contain the crate directory. A relocated process needs `GALION_HORBM_PERSONA_DIR`, or a `development.toml` it can see from its working directory. The run below is Linux, rustc 1.85.0 (`4d91de4e4`, 2025-02-17).

| Command | Result |
|---|---|
| `cargo test --manifest-path horbm/Cargo.toml -p horbm-persona --test persona` | 16 passed |
| `cargo test --manifest-path horbm/Cargo.toml -p horbm --test persona` | 5 passed |
| `cargo test --manifest-path horbm/Cargo.toml --workspace` | 57 suites, 284 passed, 0 failed, 2 ignored |
| `cargo clippy --manifest-path horbm/Cargo.toml --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` | 114 suites, 1069 passed, 0 failed, 5 ignored |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |

The two ignored horbm tests are `timed_scan_of_one_thousand_rows` and `real_card_folder_is_read_and_not_written`. `cargo test` at the repo root rewrote `voice-stack/Cargo.lock` while resolving horbm's path dependencies. That file is outside this change and was restored. No `Cargo.toml` and no lockfile is committed. Current `main` at the last test run was `cf3d614`, merged into this branch before that run. The two extra horbm passes came in with that merge.
