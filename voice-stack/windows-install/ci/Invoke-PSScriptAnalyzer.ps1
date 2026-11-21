# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Fail if PSScriptAnalyzer reports a warning or an error under this sector.
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$root = Split-Path -Path $PSScriptRoot -Parent
if (-not (Get-Module -ListAvailable -Name PSScriptAnalyzer)) {
    if ($null -eq (Get-PackageProvider -Name NuGet -ErrorAction SilentlyContinue)) {
        Install-PackageProvider -Name NuGet -Force -Scope CurrentUser | Out-Null
    }
    Set-PSRepository -Name PSGallery -InstallationPolicy Trusted
    Install-Module -Name PSScriptAnalyzer -Force -Scope CurrentUser -SkipPublisherCheck
}
Import-Module -Name PSScriptAnalyzer

$issues = @(Invoke-ScriptAnalyzer -Path $root -Recurse -Severity Warning, Error)
if ($issues.Count -gt 0) {
    $issues | Format-Table -AutoSize -Wrap | Out-String | Write-Output
    exit 1
}
Write-Output 'PSScriptAnalyzer: clean'
exit 0
