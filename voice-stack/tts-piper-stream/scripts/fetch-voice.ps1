# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Download the stock female Piper fallback into this crate's var\ directory.
# Voice pins match scripts/a21/fetch-a21.sh. The Windows runtime pin is the
# sha256 of rhasspy/piper 2023.11.14-2 piper_windows_amd64.zip.
# Does not download a Samantha, Jarvis, or s711 reference.
# Usage: powershell -ExecutionPolicy Bypass -File scripts\fetch-voice.ps1 [-Check]

param([switch]$Check)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$VDir = Join-Path $Root "var\voices"
$Var = Join-Path $Root "var"
New-Item -ItemType Directory -Force -Path $VDir | Out-Null
$Onnx = "en_US-ljspeech-medium.onnx"
$Json = "en_US-ljspeech-medium.onnx.json"
$OnnxSha = "6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22"
$JsonSha = "141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860"
$ZipSha = "f3c58906402b24f3a96d92145f58acba6d86c9b5db896d207f78dc80811efcea"
$VoiceBase = "https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ljspeech/medium"
$ZipUrl = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip"

function Get-Sha([string]$Path) {
    return (Get-FileHash -Algorithm SHA256 -Path $Path).Hash.ToLower()
}
function Save-Pinned([string]$Url, [string]$Dest, [string]$Sha) {
    if ((Test-Path $Dest) -and (Get-Sha $Dest) -eq $Sha) { Write-Host "ok   $Dest"; return }
    if ($Check) { Write-Host "MISSING/BAD $Dest"; exit 1 }
    $part = "$Dest.part"
    Write-Host "get  $Url"
    Invoke-WebRequest -Uri $Url -OutFile $part -UseBasicParsing
    if ((Get-Sha $part) -ne $Sha) { Remove-Item -Force $part; Write-Host "sha256 mismatch for $Url"; exit 1 }
    Move-Item -Force $part $Dest
    Write-Host "ok   $Dest"
}
Save-Pinned "$VoiceBase/$Onnx" (Join-Path $VDir $Onnx) $OnnxSha
Save-Pinned "$VoiceBase/$Json" (Join-Path $VDir $Json) $JsonSha
$Exe = Join-Path $Var "piper\piper.exe"
if (-not (Test-Path $Exe)) {
    if ($Check) { Write-Host "MISSING $Exe"; exit 1 }
    $zip = Join-Path $Var "piper_windows_amd64.zip"
    Save-Pinned $ZipUrl $zip $ZipSha
    Expand-Archive -Force -Path $zip -DestinationPath $Var
    Remove-Item -Force $zip
}
if (-not (Test-Path $Exe)) { Write-Host "MISSING $Exe"; exit 1 }
Write-Host "ok   $Exe"
Write-Host "voice=$(Join-Path $VDir $Onnx)"
Write-Host "config=$(Join-Path $VDir $Json)"
Write-Host "runtime=$Exe"
