# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Run the dry-run and pin tests. No model download and no cargo build.
Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$root = Split-Path -Path $PSScriptRoot -Parent
# windows-latest already ships an older Pester. ListAvailable is true, but 5.6.1 is not there.
$pester561 = @(Get-Module -ListAvailable -Name Pester | Where-Object { $_.Version -eq [version]'5.6.1' })
if ($pester561.Count -eq 0) {
    if ($null -eq (Get-PackageProvider -Name NuGet -ErrorAction SilentlyContinue)) {
        Install-PackageProvider -Name NuGet -Force -Scope CurrentUser | Out-Null
    }
    Set-PSRepository -Name PSGallery -InstallationPolicy Trusted
    Install-Module -Name Pester -RequiredVersion 5.6.1 -Force -Scope CurrentUser -SkipPublisherCheck -AllowClobber
}
Import-Module -Name Pester -RequiredVersion 5.6.1 -Force

$config = New-PesterConfiguration
$config.Run.Path = Join-Path -Path $root -ChildPath 'tests'
$config.Run.Exit = $true
$config.Output.Verbosity = 'Detailed'
Invoke-Pester -Configuration $config
