# Symphony self-eval

Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A section is complete only when its gate PASSes and the
result is written here.

| Section | Gate | Status | Provenance |
|---|---|---|---|
| ONE-PERCENT | `galion-gates/op-one-percent.sh` | PASS | stub script + measured local release |

## ONE-PERCENT

Recorded run: **2026-11-03 10:53 Europe/Zagreb**. Gate line:

```text
OP PASS — P1≤1% (ok) P3 p99=3206ns ratio=249 much_less=yes cpu=schedstat stub=8 measured=256 product_claim=false
```

Host: this build machine (Intel Xeon, 4 threads, x86_64), release.
`product_claim` is false. This is not an EPYC node, not a CX23 or CX43,
not a phone, and not a VPS.

**Measured** (`schedstat` for P1, `instant` for P3), 256 silence frames,
Rika v0 on, sense and adapt granted:

| Field | Value |
|---|---|
| kind | measured |
| P1 share | 0.00004113 (cpu 842273 ns, note `ok`) |
| P1 pass | true |
| P3 p99 | 3206 ns |
| P3 max | 24522 ns |
| P3 ratio | 249 (800 µs / p99) |
| p3_much_less | true (p99 under 80 µs) |
| pauses | 0 |

**Stub** (`injected`), the fixed script of seven 50 µs frames and one late
40 µs frame:

| Field | Value |
|---|---|
| kind | stub |
| n | 8 |
| pauses | 1 |
| p99 | 50000 ns |
| share | 0.00060937 |
| P1 pass | true |

`late_is_pause` is true. `catch_up_steps` is 0.

Three other release runs the same morning, same binary shape, had p99 of
3177 ns, 3342 ns, and 5104 ns. The number moves. The gate run in the table
is the one this section records. Debug builds are slower and are not this
table. The crate tests in debug only hold the 800 µs ceiling.

`LOCKS.toml` was not edited.

— Maciej Grajczyk
