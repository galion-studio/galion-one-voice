# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Start the local one-dot server and open http://127.0.0.1:7878/ .
# Dry-run: powershell -NoProfile -File Start-Samantha.ps1 -DryRun
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [switch]$DryRun,
    [string]$InstallRoot
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
. (Join-Path -Path $PSScriptRoot -ChildPath 'Samantha.Common.ps1')

$planOnly = [bool]$DryRun
if (-not $planOnly) {
    if (-not $PSCmdlet.ShouldProcess('127.0.0.1:7878', 'Start Samantha')) {
        $planOnly = $true
    }
}

try {
    Invoke-SamanthaStart -InstallRoot $InstallRoot -PlanOnly:$planOnly
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
