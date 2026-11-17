# galion-hw

Galion lands on any device with a processor: phone, PC, smart TV, router,
clock. `galion-hw` looks at what is there and writes one JSON device profile.
CPU is always the fallback. If a GPU with room is found (NVIDIA GeForce via
CUDA, Apple via Metal, AMD via ROCm, Intel via OpenVINO, anything else via
Vulkan / DirectML) voice roles (ASR, TTS, emotion scoring, reasoning) move to
it. `GALION_ACCEL=cpu` (or `--cpu`) keeps everything on CPU.

```text
galion-hw                     # profile JSON (detect + table)
galion-hw --facts             # raw facts only
galion-hw --cpu               # force CPU profile
galion-hw --simulate crates/galion-hw/sim/lowram-nogpu.json
galion-hw --out /var/lib/galion/hardware-profile.json
```

Bricks and licences: [BRICKS.md](BRICKS.md).

## Tier table

| tier | when | asr / tts / emotion / reasoning |
|---|---|---|
| micro | RAM < 1 GiB (router, clock) | remote / tiny / remote / remote |
| small | RAM < 4 GiB (TV, old phone) | small / small / tiny / remote |
| medium | RAM < 16 GiB or < 8 threads | small / medium / small / small |
| large | RAM >= 16 GiB and >= 8 threads, CPU | large / large / medium / medium |
| gpu-small | GPU usable VRAM 2–6 GiB | GPU small, TTS on CPU |
| gpu-medium | 6–12 GiB | GPU medium, TTS large |
| gpu-large | >= 12 GiB | GPU large |

Size classes map onto packs already in this repo: Vosk small
(`vosk-model-small-en-us-0.15`) / large (`vosk-model-en-us-0.22`); Piper
quality `x_low`/`low`/`medium`/`high`. "remote" means the role runs on another
Galion instance (the laptop or Titan). A GPU without a VRAM number (phone
Adreno, most iGPUs) is listed in `backends` but CPU stays primary.

Threads: core 0 stays with the bus / heartbeat / audio I/O, as in the
galion-cores v3 map; the rest is `threads.compute`.

## Where Galion OS calls it

1. **Boot**: obsidian-open-mind `crates/galion-boot/src/hardware.rs` (`hardware()`,
   B54) builds hardware.json from nvidia-smi + /proc/meminfo today. It should run
   `galion-hw --out <state>/hardware-profile.json` once per boot (or link the
   crate) and take `primary`, `tier` and `facts.gpus` from it, keeping its own
   degrade rules (`openmind_survival::degrade`) for the network side.
2. **Instance join** (galion-one `host.rs`): each new device a user adds sends
   its profile, so the gallery knows which instance can take GPU work.
3. **Voice host launch** (this repo): `galion-pin` / `galion-cores` keep the
   core map; launchers read `threads.compute` and per-role `backend`.
   `runtimes/galion-piper-rt` can read `ort_providers`.

Nothing reads the profile yet. Wiring any live service to it needs Mate's card.

G.A.L.I.O.N. — General Autonomous Local Intelligence Open Network
Developed and distributed by Galion Labs
Perpetual Open MIT License
Copyright © 2026 Galion Labs. All rights reserved.
