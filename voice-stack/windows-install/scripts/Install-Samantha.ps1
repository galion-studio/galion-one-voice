# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Install Samantha on the owner Windows laptop.
# Everything large goes under G:\galion\samantha. C: is not used for models or the build.
# Dry-run:  powershell -NoProfile -File Install-Samantha.ps1 -DryRun
# Real run: powershell -NoProfile -File Install-Samantha.ps1 -SamanthaRef G:\galion\samantha\owner-ref.wav
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [switch]$DryRun,
    [switch]$Check,
    [string]$InstallRoot,
    [string]$RepoRoot,
    [string]$SamanthaRef,
    [int]$Port = 7878,
    [int]$Seconds = 3600,
    [switch]$SkipBuild
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
. (Join-Path -Path $PSScriptRoot -ChildPath 'Samantha.Common.ps1')

$planOnly = [bool]$DryRun
if (-not $planOnly -and -not $Check) {
    if (-not $PSCmdlet.ShouldProcess('G:\galion\samantha', 'Install Samantha')) {
        $planOnly = $true
    }
}

try {
    Invoke-SamanthaInstall -CommonRoot $PSScriptRoot -PlanOnly:$planOnly -Check:$Check -InstallRoot $InstallRoot -RepoRoot $RepoRoot -SamanthaRef $SamanthaRef -Port $Port -Seconds $Seconds -SkipBuild:$SkipBuild
}
catch {
    Write-Output ('ERROR: ' + $_.Exception.Message)
    $script:SamanthaExitCode = 1
}

if (-not $script:SamanthaExitCode) {
    $script:SamanthaExitCode = 0
}
if ($env:SAMANTHA_NO_EXIT -eq '1') {
    $env:SAMANTHA_EXIT_CODE = [string]$script:SamanthaExitCode
    return
}
exit $script:SamanthaExitCode
