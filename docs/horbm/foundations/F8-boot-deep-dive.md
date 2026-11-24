# F8 — boot profile: the voice selects HORBM on CPU

Perpetual Open MIT License. (c) 2026 Galion Labs.

| Field | Value |
|---|---|
| Swarm | HORBM R4 phase 8. Four read-only slices, then this synthesis. |
| Written | 2026-11-24. No benchmark, no weight download, no voice audio, no paid API. |
| Voice tree | `galion-studio/galion-one-voice` `main` `204fd8cd6e399d0e1cced23c595214be94de1f57` |
| Open Mind tree | `galion-studio/obsidian-open-mind` `main` `0b0a898695b0525b179b5dd791bbfa8c281b7ffc` (2026-11-21). Read only. Not cloned into this repo. Not vendored. |
| Plan | `docs/horbm/research/R4-cpu-realtime-integration.md` §7, §10 rank 11, §11 phase 8, on draft PR #207, branch `cursor/horbm-r4-cpu-realtime-2180`. The draft is closed and unmerged. |
| Q1 | Settled for this phase, 2026-11-18, option A. HORBM retrieval answers first. Galion Research (the Rust engine in Open Mind crates `hermes-rs` and `hermes-gateway`) handles a hard question off the tick, with the 3 s cap. R4 §13 still prints Q1 as open, because that note was not edited. This phase follows the settlement. It does not call Galion Research. |
| This change | Crate `horbm-boot`. It reads a `hardware.json` fixture and selects the brain by the name `horbm`, mouth device `cpu`. `LOCKS.toml` `[boot]` records that choice. `SOUL.md` is not in this tree and was not created. Consent in `LOCKS.toml` is unchanged. |

## How to read a number

| Tag | Meaning |
|---|---|
| **[D]** | A constant or a behaviour already in a named file. |
| **[M]** | A measurement already written in this repo. This session did not repeat it. |
| **[C]** | A published source named by R1–R4. This session did not re-fetch it. |
| **[T]** | A target in R4. Not measured here. |

## What phase 8 is

R4 §11 phase 8: the voice selects HORBM on CPU regardless of the `api` line. The GPU flag is reported and does not change the mouth. The gate is a fixture `hardware.json` with no GPU and a fixture with a GPU, and both select the CPU brain. The dependency is `galion-boot`, read-only, the probe's JSON. **[T]** for the behaviour. The JSON shape below is **[D]**.

The owner rule that sits on top of that row: every Galion instance detects its hardware and reports the GPU flag, and the voice mouth stays on the CPU.

F3 §12 gap 1, on this `main`, is the same hole: `degrade.rs` sends a CPU box, and a GPU under 8192 MiB free, to `llm=api`. Nothing in the voice tree read that JSON. This crate is that reader. It does not edit Open Mind, so the probe still writes `api`. The voice no longer treats that word as a brain.

## 1. The probe document

Command `galion-boot hardware` (`crates/galion-boot/src/main.rs` lines 81–109). It writes three files under `--state-dir` (default `target/test-out/boot/state`): `hardware.json`, `machine.json`, `machine.txt`. Stdout is the `line` sentence plus a newline. Stdout is not the object. Pretty JSON, two-space indent, trailing newline (`main.rs` lines 101–103). `--apply` is not consulted for this command.

`lan_ok` defaults to false when `--lan` is omitted. `wan_ok` defaults to true when `--wan` is omitted (`main.rs` lines 87–88). The library `Probe` can point `proc_root` and `sys_root` elsewhere. The binary hardcodes `/proc` and `/sys`.

`hardware()` (`crates/galion-boot/src/hardware.rs` lines 71–119) always emits these 14 keys, in this order:

| Field | JSON type | Source |
|---|---|---|
| `processors` | number | `available_parallelism`, or 0 |
| `arch` | string | `std::env::consts::ARCH` of the probe binary |
| `os` | string | `std::env::consts::OS` of the probe binary |
| `ram_total_mib` | number | `MemTotal:` KiB, integer `/ 1024`. Missing file → 0 |
| `ram_free_mib` | number | `MemAvailable:` only. `MemFree` is ignored |
| `gpu` | null or object | below |
| `worker_mib` | number | always 512 (`WORKER_MIB`, line 15) **[D]** |
| `cap` | number 0..=100 | `(ram_free_mib / 512).min(100)` |
| `weights` | bool | `openmind_survival::degrade::profile` |
| `mouth` | bool | same profile. RAM threshold, not an audio device |
| `llm` | string | `Llm::as_str` |
| `rule` | string | first matching rule |
| `line` | string | the rule's sentence |
| `at` | string | `zagreb_time::now_rfc3339`. A stamp, not a route |

`gpu` (`hardware.rs` lines 97–102):

| Situation | JSON |
|---|---|
| `nvidia-smi` first CSV row parses (`name`, `memory.total`, `memory.used`) | `{"name","total_mib","used_mib","free_mib"}`. `free_mib` is `total.saturating_sub(used)`. There is no `vendor` field. |
| No NVIDIA row, and `{sys}/class/drm` has a `card*` node whose name has no `-` | `{"name":"drm","total_mib":0,"used_mib":0,"free_mib":0}` |
| Neither | `null`. The key is still present. |

One object, not an array. The first CSV row is the only card. A comma inside the GPU name shifts the columns and `parse_nvidia` returns `None` (`tests/boot.rs` line 120).

`degrade::profile` (`crates/openmind-survival/src/degrade.rs`). Thresholds `MIN_GPU_FREE_MIB` and `MIN_RAM_MIB` are both 8192 **[D]** (lines 53–54). First match wins.

| Order | `rule` | Condition | `llm` | `weights` |
|---|---|---|---|---|
| 1 | `gpu-room` | GPU present, free ≥ 8192, RAM ≥ 8192 | `local-gpu` | true |
| 2 | `wan-down-lan-up` | WAN down, LAN up | `lan` | false |
| 3 | `offline` | WAN down, LAN down | `cpu-light` | false |
| 4 | `gpu-low` | GPU present, and `gpu-room` missed | `api` | false |
| 5 | `cpu-only` | otherwise, including a CPU with WAN up | `api` | false |

`mouth` is `ram_total_mib >= 8192`, set after the rule (`degrade.rs` line 102). It means "enough RAM for the voice worker" (`Profile` comment, lines 46–48). A 16000 MiB CPU with WAN up is `mouth: true` and `llm: "api"`. A 2048 MiB CPU is `mouth: false` and still `llm: "api"`. Neither boolean is a playback device.

`Llm::as_str` is exactly `local-gpu`, `api`, `lan`, `cpu-light` (lines 33–39). No other spelling.

The boot test `hardware_profiles` (`tests/boot.rs` lines 75–120) is the source of the two gate fixtures' measured fields.

CPU call: meminfo `MemTotal: 16384000 kB`, `MemAvailable: 8192000 kB` (16000 and 8000 MiB), `nvidia-smi` fails, LAN down, WAN up, no DRM node yet. Asserted: the CPU sentence, `weights` false, `mouth` true, `gpu` null, `cap` 15. The sentence is the `cpu-only` line (`degrade.rs` `LINE_CPU`). That rule's `llm` is `api`. The test does not assert the token `api` on this call. The fixture includes it because that is the rule that owns the asserted sentence.

GPU call: the same meminfo and the runner stdout `RTX 4090, 24564, 1000`. Asserted: `weights` true, `gpu.free_mib` 23564. Free GPU and RAM both clear 8192, so `profile` takes `gpu-room`: `llm` `local-gpu`, `weights` true, `mouth` true, line "Obsidian has room for local weights." The test does not assert `llm` on this call. The fixture includes it because that is the rule those signals hit.

Third call, not a gate file: after `class/drm/card0` is created, `nvidia-smi` fails, LAN up, WAN down. Asserted: `llm` equals `"lan"`. The GPU value is the DRM zero object. `machine()` on that value has `provider` `"cpu"` and `gpu` the string `"drm"` (`boot.rs` lines 111–118).

`machine.json` is a second document (`hardware.rs` lines 124–144). `gpu` there is a string (`none`, `drm`, or the card name). `provider` is `"gpu"` when `weights` is true, otherwise `"cpu"`. It has no `llm` and no `rule`. A parser that accepts a string `gpu` would treat this file as the probe and could read `provider: "gpu"` as a mouth device. `horbm-boot` rejects a string `gpu` and does not select from it.

There is no brain field, no model id, and no engine name in either file. The `line` sentences for `cpu-only`, `gpu-low`, `wan-down-lan-up`, and `offline` contain the word Hermes as English prose. That word is not a model id. This phase calls the Rust engine Galion Research and does not route on the sentence.

On Windows the binary still reads `/proc` and `/sys`. A missing meminfo becomes RAM 0, `mouth` false, `cap` 0, and the process still writes the file. `nvidia-smi` is still spawned. A parsed row with RAM 0 misses `gpu-room` and hits `gpu-low`: `llm` `api`, and the `gpu` object is still present. A reader that treats `api` as "no card" drops that object. This crate does not. The probe was not executed on Windows in this session. `tests/boot.rs` imports `std::os::unix::fs::PermissionsExt` with no `cfg`, so that integration test does not compile on Windows. `hardware.rs` itself has no Unix-only import. This voice crate does not compile `galion-boot`.

`galion-one-voice` `Cargo.toml` does not name `galion-boot`, `openmind-survival`, or `obsidian-open-mind`. `galion-boot` depends on path crates inside Open Mind (`openmind-survival`, `galion-common`, `zagreb-time`). The voice crate parses the JSON with `serde_json` and takes none of those dependencies.

Parser rules this crate follows:

- Any key order. Unknown keys are ignored. A `brain` or `mouth_device` key in the file does not override the selection.
- `gpu` null or missing → not present. An object with `name`, `total_mib`, `used_mib`, `free_mib` → present, including `name` `"drm"` and zeros. A string `gpu` is `machine.json` and is an error. `free_mib` is reported as written. It is not recomputed, and it does not pick a device.
- `llm`, `rule`, and `line` are stored. A missing string is empty. The brain is still `horbm`.
- `mouth` and `weights` are stored. Missing becomes false. `mouth` true or false does not change `mouth_device`.
- A leading UTF-8 BOM is stripped, so a file saved on Windows still parses. The body is JSON. A trailing newline is fine.
- `processors`, `arch`, `os`, `at`, `cap`, and `worker_mib` are not selection inputs.

## 2. Where a brain is chosen today

Nothing on this `main` reads `hardware.json`. The only in-tree mention before this change is `docs/horbm/foundations/F3-compute-audit.md`. `crates/horbm-keys` says so in its module comment and still does not read the file.

Phone lane, `crates/galion-cascade/src/phone.rs` lines 93–116. `LlmMode` is `Canned`, `Echo`, `Hermes`, `Student`. `GALION_LLM` unset, empty, or `hermes` selects the loopback Galion Research gateway. `canned` is the rule table. `echo` and `mock` are the echo. `student` is the blocked S0 probe. Every other string, including `horbm`, is `PhoneError::UnknownLlm`. This phase does not edit `phone.rs`. Phase 4 (PR #252, open, not merged) is the change that adds `GALION_LLM=horbm`. Depending on that enum would make this crate fail to compile on `main`, and it would collide with that PR.

The 7881 mind already returns the name `horbm` (`voice-stack/local-server/src/brain.rs` lines 84–87, `HorbmMind::name`). It does not open `hardware.json`. PR #252 also edits `brain.rs`, `live.rs`, `pipeline.rs`, and `server.rs`. This phase does not edit those files, so the two PRs do not share a hunk. A process that already calls `HorbmMind` is on the named brain before this crate runs. This crate is the piece that reads the probe and refuses to leave that name when the file says `api` or `local-gpu`.

`horbm::Brain::turn` on `main` answers from cards, then may ask Galion Research under `RESEARCH_CAP`, which is 3 seconds (`horbm/crates/horbm/src/route.rs`). This crate does not call `Brain::turn` and does not open a socket.

Other `GALION_LLM` readers (`student.rs`, `reflect.rs`, `hermes.rs`, `galion-render` persona, `crates/galion-console` turn worker) do not read the probe. They are not the phase 8 selector. `crates/galion-v2-live` treats `openrouter` as the only cloud mode and is outside the root workspace. It is not the mouth.

## 3. Unmerged work this crate does not import

| PR | State | What it adds | Used here |
|---|---|---|---|
| #252 `cursor/horbm-phase4-brain-70b3` @ `6eb04d2980f39b83bcf4d4a0fec2c74e34b3611c` | open, not merged | `GALION_LLM=horbm`, crate `horbm-brain`, `LlmMode::Horbm` | No. The brain name this crate returns is the same string, so the wire can pass it through when that PR lands. |
| #253 `cursor/f2-retrieval-thread-3cd7` @ `de26ba67db0d0eba83a0e66f1630b71da9e19d72` | open, not merged | `horbm::retrieve`, `RetrievalThread`, `MemoryHitCell` | No. `Brain::turn` on that branch is unchanged, and this phase does not query memory. |
| #251 | open, not merged | a second phase-2 crate, `horbm-retrieval` | No. |
| #183 `galion-hw` | open, not merged | a different hardware crate. F3 records that no `galion-hw` crate is on `main`. | No. Phase 8 reads the probe file that already exists. |
| #207 R4 note | closed, unmerged | the plan | Read. Not merged. Not edited. |

Symbols that exist only on those branches are not named by this crate. There is no dependency on `horbm-brain`, `horbm-retrieval`, `HorbmChat`, `LlmMode::Horbm`, or `RetrievalThread`.

## 4. GPU flag versus the mouth

The probe reports a card. Several voice programs already have their own device policy. Phase 8 does not replace those programs. It does not call `nvidia-smi`. It does not set `RUSTFLAGS`. It does not download a weight.

| Path | What it does with a GPU | This phase |
|---|---|---|
| `runtimes/galion-piper-rt/src/device.rs` `parse_request` (lines 74–86) | Empty, `cpu`, and `auto` stay on CPU. `cuda` or `gpu` is a request, and only when a CUDA ORT library is on disk. | Untouched. Piper stays the out-of-process exception in `LOCKS.toml`. |
| `tools/symphony/sound-out/src/device.rs`, `tools/symphony/voice-v2/src/device.rs` | With `GALION_DEVICE` unset, a visible NVIDIA node sets the request to CUDA. The sound-out note says the Chatterbox worker process stays on CPU. | Untouched. |
| `voice-stack/samantha-voice-clone/src/device.rs` | `auto` and `cpu` stay on CPU even when the torch probe says CUDA. | Untouched. |
| `voice-stack/local-server/src/mouth.rs` | Piper unless `GALION_SAMANTHA_DEVICE=cuda` and the local files and a visible CUDA device all agree. `auto` stays on Piper. | Untouched. The boot flag does not set that env. |
| `voice-stack/mouth/README.md` | "Do not set `GALION_LLM`." CPU is the default. | Untouched. |
| `galion-probe/run-probe.sh` line 9 | `RUSTFLAGS` defaults to `-C target-cpu=native` when unset. | Untouched. That script is a STREAM/GEMV probe, not the voice brain. This crate sets no `RUSTFLAGS`. |
| `crates/galion-tensor/src/features.rs` | CPU SIMD level. No GPU. | Untouched. Phase 8 does not add a second detect. |

`LOCKS.toml` `[formats] forbidden_runtimes` still lists `candle`, `ort`, `tract`, `burn`, `llama`, `ggml`. This crate links `serde_json` 1.0.134 (the same pin as `voice-stack/local-server`) and nothing else. No `tokio`. No `target-cpu=native`. `forbid(unsafe_code)`. SIMD is not used; there is no kernel to dispatch.

## 5. The selector

Crate `crates/horbm-boot`, member of the root workspace, so `cargo test --workspace` runs it. `.github/workflows/gates.yml` also runs the phase gate by name.

`LOCKS.toml` `[boot]` records the same names. `scripts/check-locks.sh` requires `brain = "horbm"`, `mouth_device = "cpu"`, `gpu_moves_mouth = false`, `probe_llm_selects_brain = false`, and `report_gpu = true`. A crate test reads that table and compares it to `BRAIN_NAME` and `MOUTH_DEVICE`. Consent keys in the same file are unchanged.

`VoiceBoot::select` / `VoiceBoot::open` always set:

| Field | Value |
|---|---|
| `brain` | `horbm` (`BRAIN_NAME`) |
| `mouth_device` | `cpu` (`MOUTH_DEVICE`) |
| `gpu` | the probe object, or absent |
| `probe_llm`, `probe_rule`, `probe_line` | copied, ignored for the choice |
| `probe_mouth` | the RAM boolean, copied |
| `probe_weights` | copied |

`selects_cpu_brain()` is true when those two names are in place. The binary `horbm-boot <hardware.json>` prints one line and exits 0. Exit 2 is a usage error, a JSON error, or `machine.json`. The binary does not read `GALION_LLM`.

The three committed fixtures are under `crates/horbm-boot/fixtures/`. `processors`, `arch`, `os`, and `at` are stand-ins (`at` is the fixed stamp `2026-11-18T12:00:00+01:00`). The selector ignores them. The rest of each file is the probe shape from §1.

| File | What it stands for | `llm` | GPU |
|---|---|---|---|
| `hardware-no-gpu.json` | The boot test's CPU call | `api` / `cpu-only` | null |
| `hardware-gpu.json` | The boot test's RTX 4090 call | `local-gpu` / `gpu-room` | RTX 4090, free 23564 |
| `hardware-gpu-low.json` | The degrade table row GPU free 4000, RAM 16000. Not a literal `boot.rs` snapshot. `used_mib` 20564 is `24564 - 4000`, so `free_mib` matches the writer. | `api` / `gpu-low` | RTX 4090, free 4000 |

All three select `horbm` / `cpu`. The low-card file is the case F3 names in the same breath as the CPU box: a card is present, `llm` is still `api`, and the mouth does not move.

`lan`, `cpu-light`, and any other `llm` string take the same names. An empty object takes the same names, with the GPU not reported. A 2048 MiB document with `mouth: false` takes the same names. Q7 in R4, whether a small machine may run the worker, stays a separate lock. This crate does not start a worker and does not switch a small machine onto an API model.

## 6. Phase gate

Linux, rustc 1.85.0 (the CI pin in `.github/workflows/gates.yml`) and rustc 1.83.0, both on this host. `cargo check -p horbm-boot` for `x86_64-pc-windows-gnu` and `x86_64-pc-windows-msvc` compiled (the std for both targets; this host has no MSVC linker, so the Windows run of the tests is the laptop section). `cargo clippy -p horbm-boot --all-targets -- -D warnings` is clean on both toolchains. `cargo test --workspace` on rustc 1.85.0 passed on this host. No network inside the tests. No audio. No timing.

Command, and the lines it prints:

```text
cargo test -p horbm-boot fixture_without_a_gpu_and_fixture_with_a_gpu_both_select_the_cpu_brain -- --nocapture
```

```text
phase 8 gate
brain=horbm mouth=cpu gpu=false probe_llm=api probe_rule=cpu-only probe_mouth=true probe_weights=false
brain=horbm mouth=cpu gpu=true name="RTX 4090" total_mib=24564 used_mib=1000 free_mib=23564 probe_llm=local-gpu probe_rule=gpu-room probe_mouth=true probe_weights=true
brain=horbm mouth=cpu gpu=true name="RTX 4090" total_mib=24564 used_mib=20564 free_mib=4000 probe_llm=api probe_rule=gpu-low probe_mouth=true probe_weights=false
```

The first line is the fixture with no GPU. The second and third are fixtures with a GPU. `probe_llm` is `api` on the first and the third, and `local-gpu` on the second. `brain` is `horbm` on all three. `mouth` in the decision (the device) is `cpu` on all three. `gpu=true` is the report and is not the device.

The same test spawns the `horbm-boot` binary on each fixture file and requires that stdout to be the decision line. Twelve tests in the crate cover the four `llm` tokens, a DRM card, a string `gpu` (refused), a UTF-8 BOM, unknown keys, a partial object, and the `[boot]` table in `LOCKS.toml`. `cargo test -p horbm-boot` runs them.

## 7. Laptop test

GALION-STUDIO is the Windows machine. `scripts/windows/Start-GalionOne.ps1` keeps Cargo off `C:` and uses this checkout. Rust there is 1.85 or newer, host `x86_64-pc-windows-msvc`. From PowerShell, after this branch is checked out. These commands do not download a voice, do not merge, and do not set `RUSTFLAGS`.

```powershell
Set-Location G:\galion\work\galion-one-voice
$env:CARGO_HOME = "G:\galion\ci\toolchains\cargo"
$env:RUSTUP_HOME = "G:\galion\ci\toolchains\rustup"
$env:CARGO_TARGET_DIR = "G:\galion\work\target-voice"
Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue

cargo test -p horbm-boot fixture_without_a_gpu_and_fixture_with_a_gpu_both_select_the_cpu_brain -- --nocapture
if ($LASTEXITCODE -ne 0) { throw "phase 8 gate failed" }

cargo test -p horbm-boot
if ($LASTEXITCODE -ne 0) { throw "horbm-boot tests failed" }

cargo run -p horbm-boot -- crates\horbm-boot\fixtures\hardware-no-gpu.json
if ($LASTEXITCODE -ne 0) { throw "no-gpu fixture failed" }
cargo run -p horbm-boot -- crates\horbm-boot\fixtures\hardware-gpu.json
if ($LASTEXITCODE -ne 0) { throw "gpu fixture failed" }
cargo run -p horbm-boot -- crates\horbm-boot\fixtures\hardware-gpu-low.json
if ($LASTEXITCODE -ne 0) { throw "gpu-low fixture failed" }
```

The test prints the three lines in §6. Each `cargo run` prints one of those lines and exits 0. `gpu=true` appears on the two card fixtures. `brain=horbm` and `mouth=cpu` appear on all three.

To read a real probe file from this laptop, point the same binary at that `hardware.json`. Do not point it at `machine.json` (`gpu` is a string there and the binary exits 2). The binary does not spawn `nvidia-smi`. If `G:` is not the drive this checkout uses, drop the three `$env:` lines and run the same `cargo` lines from the checkout. Still do not set `RUSTFLAGS` to `-C target-cpu=native`.

## 8. What stays open

1. Open Mind `degrade.rs` still writes `llm=api` for `cpu-only` and `gpu-low`, and `llm=local-gpu` for `gpu-room`. This phase does not edit that repo. The voice reader is the override.
2. `phone.rs` on `main` still rejects `GALION_LLM=horbm`. That wire is PR #252. This crate already returns the name `horbm`, so the wire does not need a second policy when it lands. Until it lands, setting `GALION_LLM=horbm` in the phone process is still `UnknownLlm`.
3. The 7881 process names its mind `horbm` and does not open `hardware.json`. Wiring `VoiceBoot::open` into `local-server` would edit files PR #252 also edits. The binary in §7 is the boot step that can run beside that process without that collision.
4. Phase 2 (PR #253 and PR #251) is the retrieval thread. This crate does not query it. Galion Research stays off this path. The 3 s cap remains `RESEARCH_CAP` in the `horbm` crate, for a later phase.
5. `galion-probe/run-probe.sh` still defaults `target-cpu=native`. Out of scope. This crate does not.
6. An explicit `GALION_SAMANTHA_DEVICE=cuda` or `GALION_TTS_DEVICE=cuda` can still request the Piper or s711 opt-in those programs already document. The boot GPU flag does not set those variables.
7. Q7, the 8192 MiB `mouth` threshold, is unchanged. This crate reports `probe_mouth` and still names the CPU brain.
8. `LOCKS.toml` `[boot]` now records the same choice the crate makes: brain `horbm`, mouth device `cpu`, the probe `llm` line does not select the brain, a GPU flag is reported and does not move the mouth. `scripts/check-locks.sh` checks those five keys. Consent keys are unchanged. `SOUL.md` is not in this tree. Phase 8 does not change the persona, so no `SOUL.md` was created.
