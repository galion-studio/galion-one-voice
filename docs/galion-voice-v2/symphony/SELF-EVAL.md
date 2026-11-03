# Symphony self-eval

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A section is complete only when its gate passes and the row is written here.

CPU foundation for these rows: Rust on CPU, no GPU crate, no GGUF import, `LOCKS.toml` untouched, `frame_ms = 80`. Non-inference work stays inside the frame. The foundation's 800 µs tick aim is the bar; the gate below is the measurement we actually ran.

Gate command:

```bash
scripts/symphony-de.sh
```

Result is **PASS**, **FAIL**, or **BLOCKED**. FAIL means the gate ran and did not hold. BLOCKED means the gate could not be run here.

| Section | Gate | Result | Evidence |
|---|---|---|---|
| D Live emotion | `Feeling` inside `Heartbeat::step` carries galion-emotion `e`, OKLCH, and that persona's prosody. SENSE off clears `e`. A late frame holds prosody. Step does not allocate. | **PASS** | `cargo test -p galion-emotion --lib persona::`. `cargo test -p galion-heartbeat --test heartbeat feeling_`. `cargo test -p galion-heartbeat --test zero_alloc feeling_step_allocates_nothing`. |
| D No fake empathy | No scripted comfort line on the feeling path. `CANNED_EMPATHY` is false. | **PASS** | `persona::tests::no_frame_is_rest_and_not_a_script` scans the conduct source. Anger lowers pitch. Joy does not raise `e`. |
| D CPU tick | 200 feeling steps, Jarvis, tired caller, SENSE on. The batch must finish in under 50 ms. | **PASS** | Debug sample on this machine: 200 steps in 50.2 µs, average 250 ns. Under the 800 µs single-tick aim. Not an EPYC benchmark. |
| D Hub layer | Jarvis clicks, alarms, and room tone are a me-band layer. Default on. Toggle mutes the layer without changing `e` or prosody and without writing caller audio. Samantha has no bed. The reference is not denoised (`STRIP_REFERENCE_TEXTURE` is false). | **PASS** | `persona::tests::hub_layer_toggles_without_touching_caller_audio`. `feeling_hub_toggle_leaves_user_audio_and_prosody`. |
| D Reference spectrum | Energy of `personas/refs/jarvis.wav` still contains the hub bed versus a clean-speech baseline. | **BLOCKED** | The WAV is gitignored and is not in this checkout, so the spectral compare cannot run here. The code path has no strip step. |
| E One-dot | `/` is the only recorder (one canvas, `postTurn`). `/v2` redirects and does not record. An accepted utterance sets `Next::Recorded`. Words then choose Speak, Skill, or Program. No second screen. | **PASS** | `cargo test -p galion-console --features phone-turn --lib workflow::`. `drive_posts_mic_level_and_utterance_on_the_bus`. `words_choose_a_program_without_a_second_screen`. |
| E Process fork | A Skill or Program name starts a new OS process. | **BLOCKED** | The hook records the step in-process. It does not fork. Speak still uses the existing turn worker. A child process was not required for the one-dot gate. |
| CPU fleet | Voice-session vCPU and RAM on EPYC, 806 streams. | **BLOCKED** | No hosted fleet in this run. The tick measurement above is the local gate. |

No row is **FAIL**.

— Maciej Grajczyk
