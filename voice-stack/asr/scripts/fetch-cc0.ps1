# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Download the LJ Speech clips listed in eval\cc0-ljspeech.sha256. Audio is not committed.
#   powershell -ExecutionPolicy Bypass -File scripts\fetch-cc0.ps1

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$DestDir = Join-Path $Root "weights\cc0"
$List = Join-Path $Root "eval\cc0-ljspeech.sha256"
$Base = "https://huggingface.co/datasets/flexthink/ljspeech/resolve/main/wavs"
New-Item -ItemType Directory -Force -Path $DestDir | Out-Null

function Get-Sha256Lower([string]$Path) {
    return (Get-FileHash -Algorithm SHA256 -Path $Path).Hash.ToLowerInvariant()
}

Get-Content $List | ForEach-Object {
    $line = $_.Trim()
    if ($line.Length -eq 0 -or $line.StartsWith("#")) { return }
    $parts = $line -split "\s+", 2
    if ($parts.Count -lt 2) { throw "bad sha line: $line" }
    $sum = $parts[0].ToLowerInvariant()
    $name = $parts[1].Trim()
    $dest = Join-Path $DestDir $name
    if ((Test-Path $dest) -and ((Get-Sha256Lower $dest) -eq $sum)) {
        Write-Host "ok   $dest"
        return
    }
    $id = [System.IO.Path]::GetFileNameWithoutExtension($name)
    $url = "$Base/${id}.wav?download=true"
    $part = "$dest.part"
    Write-Host "get  $url"
    Invoke-WebRequest -Uri $url -OutFile $part -UseBasicParsing
    $have = Get-Sha256Lower $part
    if ($have -ne $sum) {
        Remove-Item -Force $part
        throw "sha256 mismatch for $name"
    }
    Move-Item -Force $part $dest
    Write-Host "ok   $dest"
}
Write-Host "fetch-cc0: done"
