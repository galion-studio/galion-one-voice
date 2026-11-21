# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Fetch the free CPU models and start the Galion One page on 127.0.0.1:7881.
# Everything this script writes stays on G:\galion. It refuses to start when
# G: is missing. It does not download a voice clone.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows\Start-GalionOne.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows\Start-GalionOne.ps1 -DryRun
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\windows\Start-GalionOne.ps1 -FetchOnly

[CmdletBinding()]
param(
    [switch]$DryRun,
    [switch]$FetchOnly,
    [switch]$StartOnly
)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Assert-GalionDrive {
    if (-not (Test-Path -LiteralPath "G:\")) {
        throw "G: is not available. On this laptop Galion lives only under G:\galion. Refusing to continue."
    }
}

Assert-GalionDrive

$Repo = "G:\galion\work\galion-one-voice"
$CargoHome = "G:\galion\ci\toolchains\cargo"
$RustupHome = "G:\galion\ci\toolchains\rustup"
$TargetDir = "G:\galion\work\target-voice"
$TempDir = "G:\galion\tmp\win"
$NpmCache = "G:\galion\tmp\npm"
$PipCache = "G:\galion\tmp\pip"
$HfHome = "G:\galion\tmp\hf"
$AppDataDir = "G:\galion\tmp\appdata"
$LocalAppDataDir = "G:\galion\tmp\localappdata"
$ProfileDir = "G:\galion\tmp\profile"
$LogDir = "G:\galion\work\logs"
$VarDir = Join-Path $Repo "var"

$env:CARGO_HOME = $CargoHome
$env:RUSTUP_HOME = $RustupHome
$env:CARGO_TARGET_DIR = $TargetDir
$env:TEMP = $TempDir
$env:TMP = $TempDir
$env:NPM_CONFIG_CACHE = $NpmCache
$env:PIP_CACHE_DIR = $PipCache
$env:HF_HOME = $HfHome
$env:HUGGINGFACE_HUB_CACHE = Join-Path $HfHome "hub"
$env:TRANSFORMERS_CACHE = Join-Path $HfHome "transformers"
$env:APPDATA = $AppDataDir
$env:LOCALAPPDATA = $LocalAppDataDir
$env:USERPROFILE = $ProfileDir
$env:HOME = $ProfileDir
$env:OMP_NUM_THREADS = "1"
$env:OMP_WAIT_POLICY = "PASSIVE"

$Marker = Join-Path $Repo "voice-stack\local-server\Cargo.toml"
if (-not (Test-Path -LiteralPath $Marker)) {
    throw "Repo is not at $Repo. Clone galion-one-voice there. This script does not use another drive."
}

Write-Host "repo=$Repo"
Write-Host "cargo_home=$CargoHome"
Write-Host "rustup_home=$RustupHome"
Write-Host "target=$TargetDir"
Write-Host "temp=$TempDir"
Write-Host "logs=$LogDir"
Write-Host "models=$VarDir"
Write-Host "listen=127.0.0.1:7881"

if ($DryRun) {
    Write-Host "dry-run: no download and no build"
    exit 0
}

$dirs = @(
    $CargoHome, $RustupHome, $TargetDir, $TempDir, $NpmCache, $PipCache, $HfHome,
    $AppDataDir, $LocalAppDataDir, $ProfileDir, $LogDir,
    (Join-Path $VarDir "vad"),
    (Join-Path $VarDir "asr"),
    (Join-Path $VarDir "voices"),
    (Join-Path $VarDir "piper"),
    (Join-Path $VarDir "knowledge")
)
foreach ($dir in $dirs) {
    if (-not $dir.StartsWith("G:\")) { throw "refusing to create $dir because it is not on G:" }
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
}

function Get-Sha256Lower([string]$Path) {
    return (Get-FileHash -Algorithm SHA256 -Path $Path).Hash.ToLowerInvariant()
}

function Save-Checked([string]$Url, [string]$Dest, [string]$Sha) {
    if (-not $Dest.StartsWith("G:\")) { throw "refusing download path $Dest" }
    if ((Test-Path -LiteralPath $Dest) -and ((Get-Sha256Lower $Dest) -eq $Sha)) {
        Write-Host "ok   $Dest"
        return
    }
    $part = "$Dest.part"
    Write-Host "get  $Url"
    Invoke-WebRequest -Uri $Url -OutFile $part -UseBasicParsing
    $have = Get-Sha256Lower $part
    if ($have -ne $Sha) {
        Remove-Item -Force $part
        throw "sha256 mismatch for $Url"
    }
    Move-Item -Force $part $Dest
    Write-Host "ok   $Dest"
}

$Silero = Join-Path $VarDir "vad\silero_vad.onnx"
$ModelDir = Join-Path $VarDir "asr\vosk-model-small-en-us-0.15"
$LibDir = Join-Path $VarDir "asr\libvosk"
$Onnx = Join-Path $VarDir "voices\en_US-ljspeech-medium.onnx"
$Json = Join-Path $VarDir "voices\en_US-ljspeech-medium.onnx.json"
$Piper = Join-Path $VarDir "piper\piper.exe"

if (-not $StartOnly) {
    Save-Checked "https://raw.githubusercontent.com/snakers4/silero-vad/v5.1.2/src/silero_vad/data/silero_vad.onnx" $Silero "2623a2953f6ff3d2c1e61740c6cdb7168133479b267dfef114a4a3cc5bdd788f"
    $modelReady = (Test-Path (Join-Path $ModelDir "am\final.mdl")) -and (Test-Path (Join-Path $ModelDir "conf\model.conf"))
    if (-not $modelReady) {
        $zip = Join-Path $VarDir "asr\vosk-model-small-en-us-0.15.zip"
        Save-Checked "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip" $zip "30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498"
        Expand-Archive -Force -Path $zip -DestinationPath (Join-Path $VarDir "asr")
        Remove-Item -Force $zip
    }
    if (-not (Test-Path (Join-Path $LibDir "libvosk.dll"))) {
        $zip = Join-Path $VarDir "asr\vosk-win64-0.3.45.zip"
        Save-Checked "https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-win64-0.3.45.zip" $zip "f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b"
        $extract = Join-Path $VarDir "asr\vosk-win64-extract"
        if (Test-Path $extract) { Remove-Item -Recurse -Force $extract }
        Expand-Archive -Force -Path $zip -DestinationPath $extract
        New-Item -ItemType Directory -Force -Path $LibDir | Out-Null
        Copy-Item -Recurse -Force (Join-Path $extract "vosk-win64-0.3.45\*") $LibDir
        Remove-Item -Recurse -Force $extract
        Remove-Item -Force $zip
    }
    Save-Checked "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ljspeech/medium/en_US-ljspeech-medium.onnx" $Onnx "6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22"
    Save-Checked "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ljspeech/medium/en_US-ljspeech-medium.onnx.json" $Json "141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860"
    if (-not (Test-Path $Piper)) {
        $zip = Join-Path $VarDir "piper_windows_amd64.zip"
        Save-Checked "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip" $zip "f3c58906402b24f3a96d92145f58acba6d86c9b5db896d207f78dc80811efcea"
        Expand-Archive -Force -Path $zip -DestinationPath $VarDir
        Remove-Item -Force $zip
    }
    Write-Host "models ready under $VarDir"
}

if ($FetchOnly) { exit 0 }

foreach ($need in @($Silero, (Join-Path $ModelDir "am\final.mdl"), (Join-Path $LibDir "libvosk.dll"), $Onnx, $Json, $Piper)) {
    if (-not (Test-Path -LiteralPath $need)) { throw "missing $need. Run this script without -StartOnly." }
}

if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) {
    throw "rustc is not on PATH. Install rustup with RUSTUP_HOME=$RustupHome and CARGO_HOME=$CargoHome so the toolchain stays on G:."
}

Push-Location $Repo
try {
    cargo build --release --manifest-path voice-stack\vad\Cargo.toml --bin vad-ear
    if ($LASTEXITCODE -ne 0) { throw "vad-ear build failed" }
    cargo build --release --manifest-path voice-stack\Cargo.toml -p samantha-local-server
    if ($LASTEXITCODE -ne 0) { throw "samantha-local build failed" }
}
finally { Pop-Location }

$VadBin = Join-Path $TargetDir "release\vad-ear.exe"
$Server = Join-Path $TargetDir "release\samantha-local.exe"
if (-not (Test-Path $VadBin)) { throw "missing $VadBin" }
if (-not (Test-Path $Server)) { throw "missing $Server" }

$LaptopVosk = "G:\galion\models\asr\vosk-model-small-en-us-0.15"
$LaptopLibDir = "G:\galion\models\asr\vosk-win64-0.3.45"
$LaptopSilero = Join-Path $Repo "voice-stack\vad\models\silero_vad.onnx"
$PiperVar = Join-Path $Repo "voice-stack\tts-piper-stream\var"
$OrtRoot = "G:\galion\toolchains\onnxruntime\onnxruntime-win-x64-cpu-1.31.1"
$OrtCache = "G:\galion\tmp\cache\ort.pyke.io"
if (-not $OrtCache.StartsWith("G:\")) { throw "refusing ORT cache $OrtCache" }
New-Item -ItemType Directory -Force -Path $OrtCache | Out-Null
$OrtDll = Join-Path $OrtRoot "lib\onnxruntime.dll"
if (-not (Test-Path -LiteralPath $OrtDll)) { $OrtDll = Join-Path $OrtRoot "onnxruntime.dll" }
if (Test-Path -LiteralPath $OrtDll) { $env:ORT_DYLIB_PATH = $OrtDll } else { $env:ORT_DYLIB_PATH = $OrtRoot }
$env:ORT_CACHE_DIR = $OrtCache
$env:GALION_VAD_HANG_MS = "640"
$env:GALION_VAD_COMMIT_MS = "640"
if (Test-Path -LiteralPath $LaptopSilero) { $env:GALION_SILERO_VAD = $LaptopSilero } else { $env:GALION_SILERO_VAD = $Silero }
$env:GALION_VAD_BIN = $VadBin
$env:GALION_VAD_REQUIRE = "silero"
if (Test-Path -LiteralPath (Join-Path $LaptopVosk "am\final.mdl")) { $env:GALION_VOSK_MODEL = $LaptopVosk } else { $env:GALION_VOSK_MODEL = $ModelDir }
$LaptopLib = Join-Path $LaptopLibDir "libvosk.dll"
if (Test-Path -LiteralPath $LaptopLib) {
    $env:GALION_VOSK_LIB = $LaptopLib
    $env:PATH = "$LaptopLibDir;$env:PATH"
} else {
    $env:GALION_VOSK_LIB = Join-Path $LibDir "libvosk.dll"
}
$PiperOnnx = Join-Path $PiperVar "voices\en_US-ljspeech-medium.onnx"
if (-not (Test-Path -LiteralPath $PiperOnnx)) { $PiperOnnx = Join-Path $PiperVar "en_US-ljspeech-medium.onnx" }
if (Test-Path -LiteralPath $PiperOnnx) {
    $env:GALION_PIPER_MODEL = $PiperOnnx
    $env:GALION_PIPER_CONFIG = "$PiperOnnx.json"
} else {
    $env:GALION_PIPER_MODEL = $Onnx
    $env:GALION_PIPER_CONFIG = $Json
}
$PiperExe = Join-Path $PiperVar "piper\piper.exe"
if (-not (Test-Path -LiteralPath $PiperExe)) { $PiperExe = Join-Path $PiperVar "piper.exe" }
if (-not (Test-Path -LiteralPath $PiperExe)) { $PiperExe = $Piper }
$env:GALION_A21_RUNTIME = $PiperExe
$env:GALION_SAMANTHA_WS = "127.0.0.1:7881"
$Knowledge = "G:\galion\horbm\knowledge"
if (-not (Test-Path -LiteralPath $Knowledge)) {
    $Knowledge = Join-Path $Repo "voice-stack\local-server\knowledge"
}
$env:GALION_HORBM_KNOWLEDGE = $Knowledge

$Ui = Join-Path $Repo "voice-stack\local-server\ui"
$OutLog = Join-Path $LogDir "samantha-local.out.log"
$ErrLog = Join-Path $LogDir "samantha-local.err.log"
$proc = Start-Process -FilePath $Server -ArgumentList @("--addr", "127.0.0.1:7881", "--ui", $Ui) -PassThru -RedirectStandardOutput $OutLog -RedirectStandardError $ErrLog -WindowStyle Hidden
$ready = $false
for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Seconds 1
    try {
        $health = Invoke-WebRequest -Uri "http://127.0.0.1:7881/health" -UseBasicParsing -TimeoutSec 2
        if ($health.StatusCode -eq 200 -and $health.Content -match '"ok"\s*:\s*true') { $ready = $true; break }
    } catch { }
    if ($proc.HasExited) { break }
}
if (-not $ready) {
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
    throw "samantha-local did not answer on 127.0.0.1:7881. See $ErrLog"
}
Write-Host "open http://127.0.0.1:7881/"
Start-Process "http://127.0.0.1:7881/"
Wait-Process -Id $proc.Id
exit $proc.ExitCode
