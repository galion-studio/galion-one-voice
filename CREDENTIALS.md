# Galion Voice — CREDENTIALS (third-party & borrowed pieces)

Perpetual Open MIT License · © 2026 Maciej Grajczyk · 2026-11-02 (Europe/Zagreb, UTC+1)
**LOCAL ONLY.** Code is MIT (this repo); weights and external tools keep their own licences. Only third-party / borrowed pieces are listed.

Sources scanned: `Cargo.toml` + member manifests, `Cargo.lock`, `LOCKS.toml`, `README.md`, `docs/**` (V1-DEMO, A21-BLOCKERS, A21-COUNSEL-MEMO, cascade, wave20/21), `scripts/a21/fetch-a21.sh`, `weights/.gitignore`, `weights/runtime/piper/PIPER-LICENSE.md`, source comments. No MULTI-LAB docs found. `rmp` was searched for and is not referenced anywhere. No vendored third-party source or copy-attribution comments were found in `*.rs`.

Rule: where origin or licence could not be determined from the repo or local package sources, it says **unknown — needs review**.

## 1. External runtimes, models, data (not vendored)

| Component | Origin | Licence | How used in Galion Voice |
|---|---|---|---|
| Piper runtime `piper_linux_x86_64` 2023.11.14-2 | rhasspy/piper (Michael Hansen) — https://github.com/rhasspy/piper | MIT (`weights/runtime/piper/PIPER-LICENSE.md`, © 2022 Michael Hansen) | A21 CPU TTS decoder, fetched sha256-pinned by `scripts/a21/fetch-a21.sh` to `weights/runtime/piper/`; run out of process by `A21WeightTts`. Not vendored (weights/.gitignore ignores all). |
| onnxruntime 1.14.1 (bundled in Piper tarball) | Microsoft ONNX Runtime — https://github.com/microsoft/onnxruntime | MIT (per fetch-a21.sh / A21-BLOCKERS.md) | Inference runtime inside Piper, out of process, CPU. Scoped sunset exception `[formats.exceptions.a21_piper]` (sunset 2027-02-01). Not an in-tree `ort` dependency. |
| espeak-ng 1.52 (libespeak-ng + espeak-ng-data, bundled in Piper tarball) | espeak-ng project — https://github.com/espeak-ng/espeak-ng | GPL-3 (per fetch-a21.sh / A21-BLOCKERS.md) | Phonemizer used by Piper. Separate process, not linked into Galion code. **Open: counsel decision required** (A21-BLOCKERS #2). |
| Voice model `en_US-ljspeech-medium.onnx` + `.onnx.json` | rhasspy/piper-voices on Hugging Face — https://huggingface.co/rhasspy/piper-voices (repo sha b4e358ffcb232591bf73a540e70f7de528e4174a) | MIT (repo, per A21-BLOCKERS.md) | Piper VITS 22.05 kHz voice for the A21 EN reply; fetched to `weights/tts/`, sha256-pinned, not vendored. |
| LJSpeech dataset (training data of the above voice) | LJ Speech Dataset (Keith Ito) — https://keithito.com/LJ-Speech-Dataset/ | Public domain (per A21-BLOCKERS.md) | Indirect only: the voice was trained on it. Voice choice/consent confirmation still open (A21-BLOCKERS #3). |
| libflite1 / CMU Flite 2.2 (voice `cmu_us_slt` etc.) | CMU Flite (Carnegie Mellon University), distro package `libflite1` — http://www.festvox.org/flite/ | unknown — needs review (licence not recorded in repo) | Fallback TTS when the A21 pack is missing; `dlopen` at runtime from `crates/galion-ml-voice/src/speech/flite.rs`. Not vendored, not linked at build time. |
| Mimi codec weights | Kyutai (kyutai-labs) — hf.co/kyutai/… (exact URL TODO in A21-COUNSEL-MEMO) | CC-BY-4.0 (per A21-COUNSEL-MEMO.md) | Planned A6 streaming codec. `crates/galion-mimi` is IS_STUB; `weights::load_codec` refuses. No weights in repo. Blocked on counsel. Convert-time helper `permute_qk_interleaved_to_neox()` mirrors upstream Moshi/Mimi interleaved RoPE layout (design reference, no code copied as far as documented). |
| Moshi / Moshiko dialogue weights | Kyutai (kyutai-labs) — hf.co/kyutai/… (exact URL TODO) | CC-BY-4.0 (per A21-COUNSEL-MEMO.md) | Planned (adapt manifest; int8 PTQ to `.gq`). Not loaded, not in repo. |
| PersonaPlex `nvidia/personaplex-7b-v1` | NVIDIA — https://huggingface.co/nvidia/personaplex-7b-v1 (gated) | NVIDIA Open Model License (per A21-COUNSEL-MEMO.md) | Path P-A: outputs only (eval data / possible KD later). Never redistributed; not in repo. |
| SenseVoice-Small | FunAudioLLM / FunASR (origin listed as TODO/“?” in A21-COUNSEL-MEMO) | unknown — needs review | Planned inbound SER (affect) reference; current affect head is an RMS/ZCR stub. Not loaded, not in repo. |
| softbuffer + winit | crates.io — https://crates.io/crates/softbuffer , https://crates.io/crates/winit | not vendored — licence not recorded in repo; unknown — needs review | Named only as the planned `window-native` backend in `galion-render` docs/comments. Not in Cargo.lock; nothing vendored. |
| minifb | crates.io | not used — unknown — needs review | Mentioned as alternative window backend in `galion-render/Cargo.toml` comment only. |
| Parakeet-TDT (NVIDIA), Whisper-class ASR | NVIDIA / OpenAI (design references) | CC-BY-4.0 / MIT per `docs/cascade/EN-CASCADE-LANE.md` (not verified) | Named as ASR slot classes for the EN cascade; current lane uses `StubAsr`. Not used. |
| SpeechT5, Pocket-TTS | design references | unknown — needs review | Named as alternative TTS slot classes in cascade docs. Not used. |
| sherpa-onnx | k2-fsa/sherpa-onnx (design reference) | unknown — needs review | Offline reference only (K16); not a runtime dependency. |
| Catmull-Rom cubic resampler | Well-known public algorithm (Catmull & Rom, 1974) | n/a — algorithm, in-tree implementation | `crates/galion-ml-voice/src/speech/mod.rs` resamples TTS output to 24 kHz. No copied-code attribution found. |

## 2. Rust crates (Cargo.lock, crates.io; no git deps)

Default build of most crates is zero-dependency; these come in only through optional features. Licences read from each crate's own `Cargo.toml` in the local cargo registry.

| Crate | Version(s) | Origin | Licence | How used |
|---|---|---|---|---|
| `allocator-api2` | 0.2.21 | crates.io — https://crates.io/crates/allocator-api2 | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `bitflags` | 2.13.2 | crates.io — https://crates.io/crates/bitflags | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `cassowary` | 0.3.0 | crates.io — https://crates.io/crates/cassowary | MIT / Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `castaway` | 0.2.4 | crates.io — https://crates.io/crates/castaway | MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `cfg-if` | 1.0.5 | crates.io — https://crates.io/crates/cfg-if | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `compact_str` | 0.8.2 | crates.io — https://crates.io/crates/compact_str | MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `crossbeam-deque` | 0.8.8 | crates.io — https://crates.io/crates/crossbeam-deque | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `crossbeam-epoch` | 0.9.21 | crates.io — https://crates.io/crates/crossbeam-epoch | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `crossbeam-utils` | 0.8.23 | crates.io — https://crates.io/crates/crossbeam-utils | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `crossterm` | 0.28.1 | crates.io — https://crates.io/crates/crossterm | MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `crossterm_winapi` | 0.9.1 | crates.io — https://crates.io/crates/crossterm_winapi | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `darling` | 0.20.11 | crates.io — https://crates.io/crates/darling | MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `darling_core` | 0.20.11 | crates.io — https://crates.io/crates/darling_core | MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `darling_macro` | 0.20.11 | crates.io — https://crates.io/crates/darling_macro | MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `either` | 1.19.0 | crates.io — https://crates.io/crates/either | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `equivalent` | 1.0.3 | crates.io — https://crates.io/crates/equivalent | Apache-2.0 OR MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `errno` | 0.3.14 | crates.io — https://crates.io/crates/errno | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `fnv` | 1.0.7 | crates.io — https://crates.io/crates/fnv | Apache-2.0 / MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `foldhash` | 0.1.5 | crates.io — https://crates.io/crates/foldhash | Zlib | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `hashbrown` | 0.15.5 | crates.io — https://crates.io/crates/hashbrown | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `heck` | 0.5.0 | crates.io — https://crates.io/crates/heck | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `ident_case` | 1.0.1 | crates.io — https://crates.io/crates/ident_case | MIT/Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `indoc` | 2.0.8 | crates.io — https://crates.io/crates/indoc | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `instability` | 0.3.10 | crates.io — https://crates.io/crates/instability | MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `itertools` | 0.13.0 | crates.io — https://crates.io/crates/itertools | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `itoa` | 1.0.18 | crates.io — https://crates.io/crates/itoa | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `libc` | 0.2.191 | crates.io — https://crates.io/crates/libc | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `linux-raw-sys` | 0.4.15 | crates.io — https://crates.io/crates/linux-raw-sys | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `lock_api` | 0.4.14 | crates.io — https://crates.io/crates/lock_api | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `log` | 0.4.35 | crates.io — https://crates.io/crates/log | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `lru` | 0.12.5 | crates.io — https://crates.io/crates/lru | MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `mio` | 1.2.4 | crates.io — https://crates.io/crates/mio | MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `parking_lot` | 0.12.5 | crates.io — https://crates.io/crates/parking_lot | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `parking_lot_core` | 0.9.12 | crates.io — https://crates.io/crates/parking_lot_core | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `paste` | 1.0.15 | crates.io — https://crates.io/crates/paste | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `proc-macro2` | 1.0.107 | crates.io — https://crates.io/crates/proc-macro2 | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `quote` | 1.0.47 | crates.io — https://crates.io/crates/quote | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `ratatui` | 0.29.0 | crates.io — https://crates.io/crates/ratatui | MIT | Direct optional dep of `crates/galion-console` (TUI console; pulls crossterm). |
| `rayon` | 1.12.0 | crates.io — https://crates.io/crates/rayon | MIT OR Apache-2.0 | Direct optional dep of `crates/galion-mimi` / `crates/galion-tensor` (parallel CPU kernels). |
| `rayon-core` | 1.13.0 | crates.io — https://crates.io/crates/rayon-core | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `redox_syscall` | 0.5.18 | crates.io — https://crates.io/crates/redox_syscall | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `rustix` | 0.38.44 | crates.io — https://crates.io/crates/rustix | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `rustversion` | 1.0.23 | crates.io — https://crates.io/crates/rustversion | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `ryu` | 1.0.23 | crates.io — https://crates.io/crates/ryu | Apache-2.0 OR BSL-1.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `scopeguard` | 1.2.0 | crates.io — https://crates.io/crates/scopeguard | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `signal-hook` | 0.3.18 | crates.io — https://crates.io/crates/signal-hook | Apache-2.0/MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `signal-hook-mio` | 0.2.5 | crates.io — https://crates.io/crates/signal-hook-mio | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `signal-hook-registry` | 1.4.8 | crates.io — https://crates.io/crates/signal-hook-registry | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `smallvec` | 1.16.4 | crates.io — https://crates.io/crates/smallvec | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `static_assertions` | 1.1.0 | crates.io — https://crates.io/crates/static_assertions | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `strsim` | 0.11.1 | crates.io — https://crates.io/crates/strsim | MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `strum` | 0.26.3 | crates.io — https://crates.io/crates/strum | MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `strum_macros` | 0.26.4 | crates.io — https://crates.io/crates/strum_macros | MIT | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `syn` | 2.0.119 | crates.io — https://crates.io/crates/syn | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `unicode-ident` | 1.0.27 | crates.io — https://crates.io/crates/unicode-ident | (MIT OR Apache-2.0) AND Unicode-3.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `unicode-segmentation` | 1.13.3 | crates.io — https://crates.io/crates/unicode-segmentation | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `unicode-truncate` | 1.1.0 | crates.io — https://crates.io/crates/unicode-truncate | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `unicode-width` | 0.1.14, 0.2.0 | crates.io — https://crates.io/crates/unicode-width | MIT OR Apache-2.0 | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. |
| `wasi` | 0.11.1+wasi-snapshot-preview1 | crates.io — https://crates.io/crates/wasi | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `winapi` | 0.3.9 | crates.io — https://crates.io/crates/winapi | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `winapi-i686-pc-windows-gnu` | 0.4.0 | crates.io — https://crates.io/crates/winapi-i686-pc-windows-gnu | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `winapi-x86_64-pc-windows-gnu` | 0.4.0 | crates.io — https://crates.io/crates/winapi-x86_64-pc-windows-gnu | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `windows-link` | 0.2.1 | crates.io — https://crates.io/crates/windows-link | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `windows-sys` | 0.59.0, 0.61.2 | crates.io — https://crates.io/crates/windows-sys | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `windows-targets` | 0.52.6 | crates.io — https://crates.io/crates/windows-targets | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `windows_aarch64_gnullvm` | 0.52.6 | crates.io — https://crates.io/crates/windows_aarch64_gnullvm | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `windows_aarch64_msvc` | 0.52.6 | crates.io — https://crates.io/crates/windows_aarch64_msvc | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `windows_i686_gnu` | 0.52.6 | crates.io — https://crates.io/crates/windows_i686_gnu | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `windows_i686_gnullvm` | 0.52.6 | crates.io — https://crates.io/crates/windows_i686_gnullvm | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `windows_i686_msvc` | 0.52.6 | crates.io — https://crates.io/crates/windows_i686_msvc | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `windows_x86_64_gnu` | 0.52.6 | crates.io — https://crates.io/crates/windows_x86_64_gnu | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `windows_x86_64_gnullvm` | 0.52.6 | crates.io — https://crates.io/crates/windows_x86_64_gnullvm | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |
| `windows_x86_64_msvc` | 0.52.6 | crates.io — https://crates.io/crates/windows_x86_64_msvc | unknown — needs review | Transitive (via ratatui/crossterm or rayon); platform-specific crates are only compiled on their targets. Source not present in local cargo registry, licence not verified. |

## 3. Open items

- espeak-ng GPL-3 (out of process via Piper) — counsel decision.
- `.onnx` + onnxruntime — Piper pin written (`[formats.exceptions.a21_piper]`, sunset 2027-02-01). Do not widen it.
- Mimi / Moshiko CC-BY-4.0 checklist and PersonaPlex P-A sign-off — A21-COUNSEL-MEMO.
- Unknowns: libflite1 / CMU Flite 2.2 (voice `cmu_us_slt` etc.), SenseVoice-Small, softbuffer + winit, minifb, SpeechT5, Pocket-TTS, sherpa-onnx, `crossterm_winapi`, `redox_syscall`, `wasi`, `winapi`, `winapi-i686-pc-windows-gnu`, `winapi-x86_64-pc-windows-gnu`, `windows-link`, `windows-sys`, `windows-targets`, `windows_aarch64_gnullvm`, `windows_aarch64_msvc`, `windows_i686_gnu`, `windows_i686_gnullvm`, `windows_i686_msvc`, `windows_x86_64_gnu`, `windows_x86_64_gnullvm`, `windows_x86_64_msvc`.

— Maciej Grajczyk
