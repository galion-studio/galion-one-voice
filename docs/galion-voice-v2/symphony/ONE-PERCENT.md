# One-percent engine

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. V6 non-inference account, wired into symphony.

The spelling is **Galion**. The product is Galion Voice.

The 80 ms heartbeat is the clock. Inference (ASR, reply, speech, clone)
stays off this path. Symphony counts only the engine that runs on the tick:
`Heartbeat::step`, the feeling band inside it, and the account itself.
The clock is read outside `Heartbeat::step`. That call still does not read
a clock.

Code: `crates/galion-symphony`. Gate hook: `galion_symphony::gate_report`
and `galion-gates/op-one-percent.sh`.

## Bars

The tick is 80 ms (`LOCKS.toml` `[clock] frame_ms`, unchanged).
1% of that tick is 800 µs.

| Id | What it measures | Pass |
|---|---|---|
| **P1** | Non-inference CPU as a share of the window. Sum of on-CPU time, divided by `frames × 80 ms`. | share ≤ 1% |
| **P3** | p99 of one tick's non-inference wall time (`Instant` around one step). | p99 < 800 µs |
| **pause** | A late frame is a pause. | `catch_up` stays 0 |

`p3_much_less` is a recorded flag, true when p99 is strictly under 80 µs
(one tenth of the 800 µs budget). The flag is how ≪ is marked. The gate's
exit code uses the 800 µs ceiling. A host that lands between 80 µs and
800 µs still passes, and the flag says the tenth was missed.

P1 is the window share. One tick over 800 µs pauses that tick even when
the surrounding frames keep the share ≤ 1%. The next tick is the next
tick. Nothing is replayed to catch up.

A frame is late when the scheduler says so, when the you-band is not one
full frame (the heartbeat already pauses that), or when the step itself
exceeds 800 µs.

## Stub and measured

The two series do not mix. A stub sample cannot satisfy a measured claim.

**Stub.** Caller-supplied nanoseconds. Instrument `injected`. No clock.
The gate script is seven frames of 50 µs and one late frame of 40 µs.
p99 is 50 µs, one pause, share under 1%. This checks the arithmetic and
the pause rule.

**Measured.** This process, in a release build.

- P3 instrument `instant`: wall time of one `Heartbeat::step`.
- P1 instrument `schedstat`: on-CPU time from `/proc/thread-self/schedstat`
  (the thread, so a neighbouring test thread is not the sample). The
  kernel publishes that counter on a schedule point, so the window yields
  once before the opening read and once before the closing read. The yield
  is outside `Heartbeat::step`. Waiting time is not counted.
- Fallback instrument `stat`: `utime+stime` in `CLK_TCK` quanta. A counter
  that did not move is an upper bound (`below-resolution` or `unresolved`),
  never a reported 0%.

`product_claim` is false. There is no code path that sets it true.
A release run on this build machine is a local observation. It is not an
EPYC node, not a CX23 or CX43, not a phone, and not a VPS. Hosted session
vCPU figures stay capacity context until a run on that host is recorded
here with the same labels.

Debug crate tests check the same ceiling (p99 < 800 µs, share ≤ 1%).
They are not the number in the gate JSON. The gate runs the example with
`--release`.

## Gate

```bash
galion-gates/op-one-percent.sh
```

Exit 0 is PASS, 1 is FAIL, 3 is SKIP (crate missing). The last stdout line
is `OP <STATUS> — <note>`. `galion-gates/run-gates.sh` calls it as soft
gate **OP**. `.github/workflows/gates.yml` runs it on the V2 checks.

PASS means all of these:

- `cargo test -p galion-symphony` is green
- the release JSON has a stub series and a measured series
- `product_claim` is false
- stub and measured P1 pass, and the stub instrument is `injected`
- measured P3 p99 < 800 µs, instrument `instant`
- the stub script paused its late frame
- `late_is_pause` is true and `catch_up_steps` is 0
- measured CPU source is `schedstat` or `stat`

The result line is copied into `SELF-EVAL.md` when this section is marked
complete. A missing counter stays unresolved and the gate fails, rather
than printing a share of zero.

## What stays outside

`LOCKS.toml` is unchanged. The 80 ms period and the 60 ms compute budget
inside the frame are the existing locks. This account does not add a lock
and does not load weights. No new inference runtime. No SSH.

— Maciej Grajczyk
