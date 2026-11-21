# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Run the mouth TTFA bench inside Galion-Ubuntu. Logs stay on G:.
# The guest JSON is the result. Exit 2 means a section was unmeasured.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\bench-ttfa-gpu.ps1

[CmdletBinding()]
param()
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
if (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

if (-not (Test-Path -LiteralPath "G:\")) {
    throw "G: is not available. On this laptop Galion lives only under G:\galion. Refusing to continue."
}

$Checkout = if ($env:GALION_WSL_CHECKOUT) { $env:GALION_WSL_CHECKOUT } else { "/mnt/g/galion/src/galion-one-voice" }
$Distro = if ($env:GALION_WSL_DISTRO) { $env:GALION_WSL_DISTRO } else { "Galion-Ubuntu" }
$Script = "$Checkout/scripts/bench-ttfa-gpu.sh"

$Bash = @"
set -euo pipefail
export TMPDIR=/mnt/g/galion/tmp
export XDG_CACHE_HOME=/mnt/g/galion/cache
export GALION_VOICE_TEST_DIR=/mnt/g/galion/logs/voice-test
mkdir -p "`$TMPDIR" "`$XDG_CACHE_HOME" "`$GALION_VOICE_TEST_DIR"
bash "$Script"
"@

& wsl.exe -d $Distro -- bash -lc $Bash
if ($LASTEXITCODE -eq 2) {
    Write-Output "bench-ttfa-gpu: exit 2. Read G:\galion\logs\voice-test\ttfa-gpu.json. An unmeasured section has no millisecond field."
    exit 2
}
if ($LASTEXITCODE -ne 0) {
    throw "bench-ttfa-gpu.sh exited $LASTEXITCODE"
}
