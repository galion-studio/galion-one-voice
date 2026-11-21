# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Download Vosk small-en, the Windows libvosk zip, and optionally whisper.cpp ggml files.
#   powershell -ExecutionPolicy Bypass -File scripts\fetch-models.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\fetch-models.ps1 -Whisper

param([switch]$Whisper)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$W = Join-Path $Root "weights"
$ModelUrl = "https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip"
$ModelSha = "30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498"
$WinUrl = "https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-win64-0.3.45.zip"
$WinSha = "f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b"
$TinyUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en.bin?download=true"
$TinySha = "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f"
$BaseUrl = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin?download=true"
$BaseSha = "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002"
$ModelDir = Join-Path $W "asr\vosk-model-small-en-us-0.15"
$LibDir = Join-Path $W "asr\libvosk"

function Get-Sha256Lower([string]$Path) {
    return (Get-FileHash -Algorithm SHA256 -Path $Path).Hash.ToLowerInvariant()
}

function Save-Checked([string]$Url, [string]$Dest, [string]$Sha) {
    if ((Test-Path $Dest) -and ((Get-Sha256Lower $Dest) -eq $Sha)) {
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

New-Item -ItemType Directory -Force -Path (Join-Path $W "asr") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $W "whisper") | Out-Null
$ignore = Join-Path $W ".gitignore"
if (-not (Test-Path $ignore)) { Set-Content -Path $ignore -Value "*`n!.gitignore`n" }

$modelReady = (Test-Path (Join-Path $ModelDir "am\final.mdl")) -and (Test-Path (Join-Path $ModelDir "conf\model.conf"))
if (-not $modelReady) {
    $zip = Join-Path $W "asr\vosk-model-small-en-us-0.15.zip"
    Save-Checked $ModelUrl $zip $ModelSha
    Expand-Archive -Force -Path $zip -DestinationPath (Join-Path $W "asr")
    Remove-Item -Force $zip
}
Write-Host "ok   $ModelDir"

if (-not (Test-Path (Join-Path $LibDir "libvosk.dll"))) {
    $zip = Join-Path $W "asr\vosk-win64-0.3.45.zip"
    Save-Checked $WinUrl $zip $WinSha
    $extract = Join-Path $W "asr\vosk-win64-extract"
    if (Test-Path $extract) { Remove-Item -Recurse -Force $extract }
    Expand-Archive -Force -Path $zip -DestinationPath $extract
    New-Item -ItemType Directory -Force -Path $LibDir | Out-Null
    Copy-Item -Recurse -Force (Join-Path $extract "vosk-win64-0.3.45\*") $LibDir
    Remove-Item -Recurse -Force $extract
    Remove-Item -Force $zip
}
Write-Host "ok   $(Join-Path $LibDir 'libvosk.dll')"

if ($Whisper) {
    Save-Checked $TinyUrl (Join-Path $W "whisper\ggml-tiny.en.bin") $TinySha
    Save-Checked $BaseUrl (Join-Path $W "whisper\ggml-base.en.bin") $BaseSha
}
Write-Host "fetch-models: done"
