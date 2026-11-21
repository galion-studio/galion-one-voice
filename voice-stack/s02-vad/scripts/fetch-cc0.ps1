# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Download the CC0 speech clips and the MIT Silero VAD model. Nothing is committed.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$src = Join-Path $PSScriptRoot "sources.tsv"
$ua = "galion-s02-vad/0.1 (local VAD research; CC0 speech)"
Get-Content -LiteralPath $src | ForEach-Object {
    $line = $_
    if ($line -match '^\s*$' -or $line.StartsWith("#")) { return }
    $parts = $line -split "`t"
    if ($parts.Count -lt 4) { throw "bad sources line: $line" }
    $dest = $parts[0]
    $url = $parts[1]
    $sha = $parts[2].ToLower()
    $lic = $parts[3]
    $dir = Split-Path -Parent $dest
    if ($dir) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
    if (Test-Path -LiteralPath $dest) {
        $got = (Get-FileHash -Algorithm SHA256 -LiteralPath $dest).Hash.ToLower()
        if ($got -eq $sha) {
            Write-Host "ok  $dest"
            return
        }
        Write-Host "redo $dest (sha256 mismatch)"
    }
    Write-Host "get $dest"
    Invoke-WebRequest -Uri $url -OutFile $dest -UserAgent $ua
    $got = (Get-FileHash -Algorithm SHA256 -LiteralPath $dest).Hash.ToLower()
    if ($got -ne $sha) {
        throw "sha256 mismatch for $dest`n  expected $sha`n  got      $got"
    }
    Write-Host "ok  $dest  $lic"
}
Write-Host "ready. audio stays in samples/ and the model in models/. Both are gitignored."
