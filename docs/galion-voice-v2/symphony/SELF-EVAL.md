# Symphony self-eval

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A section is complete only when its gate passes and the row is written here.

CPU foundation for these rows: Rust on CPU, no GPU crate, no GGUF import, `LOCKS.toml` untouched, `frame_ms = 80`. Non-inference work stays inside the frame. The foundation's 800 µs tick aim is the bar; the gate below is the measurement we actually ran.

Gate command:

```bash
scripts/symphony-de.sh
```

| Section | Gate | Result | Evidence |
|---|---|---|---|
| D Persona / emotion | `Feeling::live` is galion-emotion `e` + OKLCH + that persona's column. No canned line. Jarvis hub texture is kept. Feeling is written inside `Heartbeat::step`. SENSE off clears it. A late frame holds it. Step does not allocate. | **PASS** | `cargo test -p galion-emotion --lib persona::` (4). `cargo test -p galion-heartbeat --test heartbeat feeling_` (3). `cargo test -p galion-heartbeat --test zero_alloc feeling_step_allocates_nothing` (1). |
| D CPU tick | 200 feeling steps, Jarvis, tired caller, SENSE on, debug build. Batch must finish in under 50 ms (inside one 80 ms frame with margin). | **PASS** | One local run: 200 steps in 54.9 µs, average 274 ns. The batch is under the 800 µs single-tick aim. This is one debug sample, not a datacenter benchmark. |
| E One-dot | `/` is the only recorder (one canvas, `postTurn`). `/v2` redirects and does not record. An accepted utterance sets `Next::Recorded`. Words then choose Speak, Skill, or Program. No second screen. | **PASS** | `cargo test -p galion-console --features phone-turn --lib workflow::` (2). `phone_host::tests::drive_posts_mic_level_and_utterance_on_the_bus`. `phone_host::tests::words_choose_a_program_without_a_second_screen`. |

`CANNED_EMPATHY` is false. `STRIP_REFERENCE_TEXTURE` is false. The workflow hook does not keep the transcript.

— Maciej Grajczyk
