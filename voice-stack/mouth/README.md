<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# Mouth (TTS on 127.0.0.1:8002)

CPU Piper is the default. The CUDA execution provider is used only when `GALION_TTS_DEVICE=cuda` and the GPU library actually loads. If the device, the `galion-piper-rt` binary, or the CUDA build of onnxruntime is missing, the process logs `falling back to CPU` and the session stays on the CPU provider. `auto` does not select the GPU.

This process binds `127.0.0.1` only. It does not call Ollama, Hermes, or a paid API. It does not speak a clone. Samantha, Jarvis, and s711 clips stay off this path. The clone child in the rest of the tree stays on CPU.

`LOCKS.toml` still describes the Piper pin as CPU. This opt-in does not edit that file. The CPU library and the voice pack are unchanged.

## Run

From the repository:

```bash
python3 voice-stack/mouth/serve.py
```

`GET http://127.0.0.1:8002/health` returns JSON. `ok` is false, and the status is 503, until the Piper runtime and `en_US-ljspeech-medium` are on disk. `POST /synthesize` with `{"text":"Hello."}` returns a WAV only after a real child produced PCM.

`GALION_MOUTH_BIND` may select another `127.0.0.1` port. Any other address is refused.

## WSL, on the G: drive

The distro is `Galion-Ubuntu` at `G:\galion\wsl\Galion-Ubuntu`. The checkout used by the voice package is `G:\galion\src\galion-one-voice` (`/mnt/g/galion/src/galion-one-voice` in the guest).

1. Install a current NVIDIA driver in Windows. Do not install a second NVIDIA driver inside the distro. WSL uses the Windows driver.
2. In the guest, `nvidia-smi -L` should list the GPU. That is the check. This repository does not record a laptop timing.
3. Install the CUDA user libraries that match the ONNX Runtime GPU build. ONNX Runtime 1.31.1 publishes two Linux x64 GPU archives: `onnxruntime-linux-x64-gpu_cuda13-1.31.1.tgz` (CUDA 13.0 or newer, cuDNN 9) and `onnxruntime-linux-x64-gpu_cuda12-1.31.1.tgz` (CUDA 12.8 or newer, cuDNN 9). The `ort` crate this runtime already uses looks for the CUDA 13 sonames (`libcudart.so.13`, `libcudnn.so.9`) when it preloads. Pick the archive that matches the toolkit you install. Unpack it on G:, not into git:

```bash
mkdir -p /mnt/g/galion/ort /mnt/g/galion/tmp /mnt/g/galion/cache /mnt/g/galion/logs/voice-test
# unpack the official tarball yourself; this tree does not download it
export GALION_ORT_CUDA_LIB=/mnt/g/galion/ort/onnxruntime-linux-x64-gpu_cuda13-1.31.1/lib/libonnxruntime.so.1.31.1
export GALION_CUDA_LIB_DIR=/usr/local/cuda/lib64
```

`GALION_CUDA_LIB_DIR` is prepended to `LD_LIBRARY_PATH` for the child when the device is CUDA. Point it at the toolkit `lib64` and the cuDNN directory if those are not already on the default linker path. A toolkit unpacked under `/mnt/g/galion/cuda` is the right place if you do not want it inside the distro disk.

4. Build `galion-piper-rt` with the existing script (`scripts/a21/build-piper-rt.sh`) after `scripts/a21/fetch-a21.sh`. The binary enables the CUDA provider API and still defaults to CPU. Put `CARGO_TARGET_DIR` on G: if you do not want the build under the distro home, for example `/mnt/g/galion/target`.
5. Start the mouth:

```bash
export TMPDIR=/mnt/g/galion/tmp
export XDG_CACHE_HOME=/mnt/g/galion/cache
export GALION_TTS_DEVICE=cuda
export GALION_A21_RUNTIME=/mnt/g/galion/src/galion-one-voice/weights/runtime/piper-rt/galion-piper-rt
python3 voice-stack/mouth/serve.py
```

The CUDA arena cap is 2 GiB (`GALION_TTS_CUDA_ARENA_BYTES` overrides it). That is a ceiling for an 8 GB laptop GPU, not a measurement of Piper. Convolution search is heuristic, so the first utterance does not run an exhaustive workspace search. Input shapes change with the sentence, so CUDA graphs stay off.

A user unit is optional. CPU is still the default if you omit `GALION_TTS_DEVICE`. Keep `TemporaryDirectory` and `XDG_CACHE_HOME` on the G: paths above. `PrivateTmp` does not redirect `TMPDIR` when you set it.

```ini
[Service]
Environment=GALION_MOUTH_BIND=127.0.0.1:8002
Environment=TMPDIR=/mnt/g/galion/tmp
Environment=XDG_CACHE_HOME=/mnt/g/galion/cache
Environment=GALION_TTS_DEVICE=cuda
Environment=GALION_ORT_CUDA_LIB=/mnt/g/galion/ort/onnxruntime-linux-x64-gpu_cuda13-1.31.1/lib/libonnxruntime.so.1.31.1
ExecStart=/usr/bin/python3 /mnt/g/galion/src/galion-one-voice/voice-stack/mouth/serve.py
```

Do not set `GALION_LLM`. Do not point this unit at Ollama.

## Bench

On the laptop, from the guest:

```bash
export TMPDIR=/mnt/g/galion/tmp
export XDG_CACHE_HOME=/mnt/g/galion/cache
scripts/bench-ttfa-gpu.sh
```

From Windows, `scripts/bench-ttfa-gpu.ps1` runs that script in `Galion-Ubuntu` and leaves the log at `G:\galion\logs\voice-test\ttfa-gpu.json`. Override the guest checkout with `GALION_WSL_CHECKOUT` if yours is not `/mnt/g/galion/src/galion-one-voice`.

The JSON has `cpu` and `cuda`. A section with `"measured": false` has no millisecond field and no WAV. CUDA is `"skipped": true` when this machine has no visible GPU, the runtime is not `galion-piper-rt`, or the CUDA library is not on disk. A child that falls back to CPU is not stored as a CUDA result. WAV files are written only for a measured section. Do not commit them.

Exit 0 means the CPU section measured. Exit 2 means the CPU section did not (pack or runtime missing). That is a report, not a filled-in time. CI runs the same script without a GPU and without treating a skip as a measured GPU result.
