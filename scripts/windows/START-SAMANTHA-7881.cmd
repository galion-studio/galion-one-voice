@echo off
rem SPDX-License-Identifier: MIT
rem Copyright (c) 2026 Galion Labs
rem Laptop start for Galion One on 127.0.0.1:7881.
rem Copy or run as G:\galion\work\START-SAMANTHA-7881.cmd
rem Everything stays on G:. A missing G: stops before any directory is created.
rem Cards are read-only. A miss is I don't know.
rem Brain::open in the orchestrator stays unlinked. This script does not open a memory database.

setlocal
if not exist G:\ (
  echo G: is not available. On this laptop Galion lives only under G:\galion. Refusing to continue.
  exit /b 1
)

set "REPO=G:\galion\work\galion-one-voice"
set "CARGO_HOME=G:\galion\ci\toolchains\cargo"
set "RUSTUP_HOME=G:\galion\ci\toolchains\rustup"
set "CARGO_TARGET_DIR=G:\galion\work\target-voice"
set "TEMP=G:\galion\tmp\win"
set "TMP=G:\galion\tmp\win"
set "NPM_CONFIG_CACHE=G:\galion\tmp\npm"
set "PIP_CACHE_DIR=G:\galion\tmp\pip"
set "HF_HOME=G:\galion\tmp\hf"
set "HUGGINGFACE_HUB_CACHE=G:\galion\tmp\hf\hub"
set "APPDATA=G:\galion\tmp\appdata"
set "LOCALAPPDATA=G:\galion\tmp\localappdata"
set "USERPROFILE=G:\galion\tmp\profile"
set "HOME=G:\galion\tmp\profile"
set "ORT_CACHE_DIR=G:\galion\tmp\cache\ort.pyke.io"
set "LOGDIR=G:\galion\work\logs"

if not exist "%REPO%\voice-stack\local-server\Cargo.toml" (
  echo Repo is not at %REPO%. Clone galion-one-voice there.
  exit /b 1
)

if not exist "G:\galion\ci\toolchains\cargo" mkdir "G:\galion\ci\toolchains\cargo"
if not exist "G:\galion\ci\toolchains\rustup" mkdir "G:\galion\ci\toolchains\rustup"
if not exist "G:\galion\work\target-voice" mkdir "G:\galion\work\target-voice"
if not exist "G:\galion\tmp\win" mkdir "G:\galion\tmp\win"
if not exist "G:\galion\tmp\npm" mkdir "G:\galion\tmp\npm"
if not exist "G:\galion\tmp\pip" mkdir "G:\galion\tmp\pip"
if not exist "G:\galion\tmp\hf" mkdir "G:\galion\tmp\hf"
if not exist "G:\galion\tmp\appdata" mkdir "G:\galion\tmp\appdata"
if not exist "G:\galion\tmp\localappdata" mkdir "G:\galion\tmp\localappdata"
if not exist "G:\galion\tmp\profile" mkdir "G:\galion\tmp\profile"
if not exist "G:\galion\tmp\cache\ort.pyke.io" mkdir "G:\galion\tmp\cache\ort.pyke.io"
if not exist "G:\galion\work\logs" mkdir "G:\galion\work\logs"

set "GALION_VOSK_MODEL=G:\galion\models\asr\vosk-model-small-en-us-0.15"
set "VOSK_DIR=G:\galion\models\asr\vosk-win64-0.3.45"
set "GALION_VOSK_LIB=%VOSK_DIR%\libvosk.dll"
set "PATH=%VOSK_DIR%;%PATH%"
set "GALION_SILERO_VAD=%REPO%\voice-stack\vad\models\silero_vad.onnx"
set "PIPER_VAR=%REPO%\voice-stack\tts-piper-stream\var"
set "ORT_ROOT=G:\galion\toolchains\onnxruntime\onnxruntime-win-x64-cpu-1.31.1"
if exist "%ORT_ROOT%\lib\onnxruntime.dll" (
  set "ORT_DYLIB_PATH=%ORT_ROOT%\lib\onnxruntime.dll"
) else if exist "%ORT_ROOT%\onnxruntime.dll" (
  set "ORT_DYLIB_PATH=%ORT_ROOT%\onnxruntime.dll"
) else (
  set "ORT_DYLIB_PATH=%ORT_ROOT%\"
)
set "GALION_VAD_HANG_MS=640"
set "GALION_VAD_COMMIT_MS=640"
set "GALION_VAD_BIN=%CARGO_TARGET_DIR%\release\vad-ear.exe"
set "GALION_VAD_REQUIRE=silero"
set "GALION_SAMANTHA_WS=127.0.0.1:7881"
set "OMP_NUM_THREADS=1"
set "OMP_WAIT_POLICY=PASSIVE"

if exist "%PIPER_VAR%\voices\en_US-ljspeech-medium.onnx" (
  set "GALION_PIPER_MODEL=%PIPER_VAR%\voices\en_US-ljspeech-medium.onnx"
  set "GALION_PIPER_CONFIG=%PIPER_VAR%\voices\en_US-ljspeech-medium.onnx.json"
) else if exist "%PIPER_VAR%\en_US-ljspeech-medium.onnx" (
  set "GALION_PIPER_MODEL=%PIPER_VAR%\en_US-ljspeech-medium.onnx"
  set "GALION_PIPER_CONFIG=%PIPER_VAR%\en_US-ljspeech-medium.onnx.json"
)
if exist "%PIPER_VAR%\piper\piper.exe" set "GALION_A21_RUNTIME=%PIPER_VAR%\piper\piper.exe"
if exist "%PIPER_VAR%\piper.exe" set "GALION_A21_RUNTIME=%PIPER_VAR%\piper.exe"

if exist "G:\galion\horbm\knowledge\" (
  set "GALION_HORBM_KNOWLEDGE=G:\galion\horbm\knowledge"
) else (
  set "GALION_HORBM_KNOWLEDGE=%REPO%\voice-stack\local-server\knowledge"
)
rem ReasoningRouter reads those cards first. Leave GALION_RESEARCH_BIND unset
rem unless the gateway is already listening on 127.0.0.1:7911. Do not use 8787.

set "SERVER=%CARGO_TARGET_DIR%\release\samantha-local.exe"
set "UI=%REPO%\voice-stack\local-server\ui"
if not exist "%SERVER%" (
  echo missing %SERVER%
  echo Build it with scripts\windows\Start-GalionOne.ps1 so the toolchain stays on G:.
  exit /b 1
)

echo listen=http://127.0.0.1:7881/
echo vad=%GALION_SILERO_VAD%
echo vosk=%GALION_VOSK_MODEL%
echo piper=%PIPER_VAR%
echo ort=%ORT_DYLIB_PATH%
echo A card miss is I don't know.
"%SERVER%" --addr 127.0.0.1:7881 --ui "%UI%" >> "%LOGDIR%\samantha-local.out.log" 2>> "%LOGDIR%\samantha-local.err.log"
exit /b %ERRORLEVEL%
