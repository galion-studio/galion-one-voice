# Pi synthesis A2

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-06.

`galion-ml-voice` default features leave CUDA off. The gate is `crates/galion-ml-voice/tests/no_cuda_default.rs`. It reads `crates/galion-ml-voice/Cargo.toml` and walks the `default` feature list, including same-crate features the list turns on. A missing `default` key is Cargo's empty set.

This run parsed one feature key, `speech`. The default closure was empty. Speech sources were left as they were.

## Host

Linux 6.6.87.2-microsoft-standard-WSL2 x86_64.

`rustc 1.99.1 (58cf8f9c3 2026-10-13)`. `cargo 1.99.1 (5f94df478 2026-08-27)`.

Worktree `src/swarms/e42/synthesis-a2`. Branch `cursor/symphony-v2-emotion42-synthesis-a2-d379`.

Each cargo command ran under `flock /home/galion/workspace/galion/tmp/cargo-build.lock` after `source /home/galion/workspace/galion/env.sh`. Log times below are UTC.

## Default-feature test

```bash
flock /home/galion/workspace/galion/tmp/cargo-build.lock cargo test --offline -p galion-ml-voice --test no_cuda_default -- --nocapture
```

Start `2026-11-06T16:58:43Z`. End `2026-11-06T16:58:43Z`. Exit 0.

```text
   Compiling galion-ml-voice v0.0.1 (/home/galion/workspace/galion/src/swarms/e42/synthesis-a2/crates/galion-ml-voice)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.53s
     Running tests/no_cuda_default.rs (target/debug/deps/no_cuda_default-28103ec66dfe9f73)

running 10 tests
test comment_and_other_table_do_not_count ... ok
test explicit_cuda_in_default_is_detected ... ok
test non_cuda_default_stays_clean ... ok
test dependency_cuda_feature_is_detected ... ok
test optional_dependency_named_cuda_is_detected ... ok
test cuda_name_match_is_case_insensitive ... ok
test transitive_cuda_feature_is_detected ... ok
test weak_dependency_cuda_feature_is_detected ... ok
test missing_default_key_is_an_empty_list ... ok
manifest=/home/galion/workspace/galion/src/swarms/e42/synthesis-a2/crates/galion-ml-voice/Cargo.toml
feature-keys=["speech"]
default-closure={}
test default_features_do_not_enable_cuda ... ok

test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
```

Result: **PASS**.

The host test builds `x86_64-unknown-linux-gnu`. The Pi target check is the next command.

## aarch64 check

```bash
rustup target add aarch64-unknown-linux-gnu
flock /home/galion/workspace/galion/tmp/cargo-build.lock cargo check --offline --target aarch64-unknown-linux-gnu -p galion-ml-voice
```

`rustup target add` started `2026-11-06T16:57:00Z` and exited 0. It printed `info: component rust-std for target aarch64-unknown-linux-gnu is up to date`. Installed targets after that command: `aarch64-unknown-linux-gnu` and `x86_64-unknown-linux-gnu`.

`cargo check` started `2026-11-06T16:57:00Z`, ended `2026-11-06T16:57:01Z`, and exited 0.

```text
    Checking galion-emotion v0.1.0 (/home/galion/workspace/galion/src/swarms/e42/synthesis-a2/galion-emotion)
    Checking galion-tensor v0.0.1 (/home/galion/workspace/galion/src/swarms/e42/synthesis-a2/crates/galion-tensor)
    Checking galion-ml-voice v0.0.1 (/home/galion/workspace/galion/src/swarms/e42/synthesis-a2/crates/galion-ml-voice)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.42s
```

Result: **PASS** as a typecheck on this host. `cargo check` covers the package library and `galion-voice-compile`. It leaves the integration test to `cargo test`.

## Size and memory

| Field | Record |
|---|---|
| Binary size | unmeasured |
| RAM | unmeasured |

`cargo check` writes metadata. The aarch64 artifact from this check is `target/aarch64-unknown-linux-gnu/debug/deps/libgalion_ml_voice-2df50bbcb8799637.rmeta`. Pi binary size stays unmeasured.

`stat -c %s` on the host debug test harness `target/debug/deps/no_cuda_default-28103ec66dfe9f73` printed `8672744`. That count is the x86_64 test executable. The Pi binary size in the table stays unmeasured.

RAM was not sampled.

## Pi status

**PASS.** `cargo check --offline --target aarch64-unknown-linux-gnu -p galion-ml-voice` exited 0 in 0.42s. Default features do not enable CUDA (`default-closure={}`). Binary size unmeasured. RAM unmeasured.

— Maciej Grajczyk
