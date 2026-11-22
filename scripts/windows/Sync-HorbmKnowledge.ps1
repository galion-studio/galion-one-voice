# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Copy the galion-knowledge core card pack into a local folder.
# Public HTTPS git clone. No tokens. The cards are not committed.
# Default target is G:\galion\horbm\knowledge. A missing folder leaves the
# 7881 server on the example kettle card.
param(
    [string]$Target = "G:\galion\horbm\knowledge"
)
$ErrorActionPreference = "Stop"
if ($Target -eq "G:\galion\horbm\knowledge" -and -not (Test-Path -LiteralPath "G:\")) {
    throw "G: is not mounted. Refusing to create the knowledge directory elsewhere."
}
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    throw "git is required"
}
$Repo = "https://github.com/galion-studio/galion-knowledge.git"
$Tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("galion-knowledge-" + [guid]::NewGuid().ToString("n"))
New-Item -ItemType Directory -Force -Path $Tmp | Out-Null
try {
    Write-Host "clone $Repo"
    git clone --depth 1 --filter=blob:none --sparse $Repo (Join-Path $Tmp "src")
    if ($LASTEXITCODE -ne 0) { throw "git clone failed" }
    git -C (Join-Path $Tmp "src") sparse-checkout set --skip-checks v2/data/cards v2/shards/manifest.json
    if ($LASTEXITCODE -ne 0) { throw "git sparse-checkout failed" }
    New-Item -ItemType Directory -Force -Path $Target | Out-Null
    foreach ($name in @("horbm", "world", "diary")) {
        $dest = Join-Path $Target $name
        if (Test-Path -LiteralPath $dest) { Remove-Item -LiteralPath $dest -Recurse -Force }
        $src = Join-Path $Tmp "src\v2\data\cards\$name"
        if (Test-Path -LiteralPath $src) { Copy-Item -LiteralPath $src -Destination $dest -Recurse }
    }
    Copy-Item -LiteralPath (Join-Path $Tmp "src\v2\shards\manifest.json") -Destination (Join-Path $Target "manifest.json") -Force
} finally {
    if (Test-Path -LiteralPath $Tmp) { Remove-Item -LiteralPath $Tmp -Recurse -Force }
}
$count = (Get-ChildItem -LiteralPath $Target -Recurse -Filter *.json | Where-Object { $_.Name -ne "manifest.json" }).Count
Write-Host "cards=$count"
Write-Host "target=$Target"
Write-Host "GALION_HORBM_KNOWLEDGE=$Target"
