# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Shared helpers for the Windows Samantha install. Dot-source this file.
# Writes are refused outside G:\galion\samantha. Nothing here edits the
# task scheduler, services, drivers, or Windows system files.

Set-StrictMode -Version 2.0

$script:SamanthaNetworkCalls = 0
$script:SamanthaExitCode = 0

function Join-WinPath {
    param(
        [Parameter(Mandatory = $true)][string]$Base,
        [Parameter(Mandatory = $true)][string]$Child
    )
    $left = $Base.TrimEnd('\')
    $right = ($Child -replace '/', '\').Trim('\')
    if ([string]::IsNullOrEmpty($right)) {
        return $left
    }
    return ($left + '\' + $right)
}

function Get-SamanthaConfig {
    param([Parameter(Mandatory = $true)][string]$CommonRoot)
    $path = Join-Path -Path $CommonRoot -ChildPath 'pins.psd1'
    if (-not (Test-Path -LiteralPath $path)) {
        throw "missing pin file: $path"
    }
    return (Import-PowerShellDataFile -Path $path)
}

function Get-SamanthaRepoRoot {
    param([Parameter(Mandatory = $true)][string]$CommonRoot)
    $dir = Split-Path -Path $CommonRoot -Parent
    $dir = Split-Path -Path $dir -Parent
    $dir = Split-Path -Path $dir -Parent
    return $dir
}

function Test-SamanthaHome {
    param([string]$InstallRoot)
    if ([string]::IsNullOrWhiteSpace($InstallRoot)) {
        return $false
    }
    $norm = ($InstallRoot -replace '/', '\').TrimEnd('\')
    return ($norm -match '^(?i)G:\\galion\\samantha$')
}

function Get-SamanthaHome {
    param([string]$InstallRoot)
    if ([string]::IsNullOrWhiteSpace($InstallRoot)) {
        if (-not [string]::IsNullOrWhiteSpace($env:GALION_SAMANTHA_HOME)) {
            $InstallRoot = $env:GALION_SAMANTHA_HOME
        }
        else {
            $InstallRoot = 'G:\galion\samantha'
        }
    }
    $norm = ($InstallRoot -replace '/', '\').TrimEnd('\')
    if (-not (Test-SamanthaHome -InstallRoot $norm)) {
        throw "Samantha home must be G:\galion\samantha. This install does not write models or build output on C:. Got: $InstallRoot"
    }
    return $norm
}

function Confirm-SamanthaWritePath {
    param([Parameter(Mandatory = $true)][string]$Path)
    $norm = ($Path -replace '/', '\')
    if ($norm -notmatch '^(?i)G:\\galion\\samantha(\\|$)') {
        throw "refusing to write outside G:\galion\samantha: $Path"
    }
    if ($norm -match '(?i)\\Windows\\|\\System32\\|\\SysWOW64\\|\\WinSxS\\') {
        throw "refusing a Windows system path: $Path"
    }
}

function Get-SamanthaWebAddr {
    param([Parameter(Mandatory = $true)][int]$Port)
    if ($Port -lt 1 -or $Port -gt 65535) {
        throw "port out of range: $Port"
    }
    return ('127.0.0.1:' + $Port)
}

function Test-SamanthaLoopbackAddr {
    param([string]$Addr)
    if ([string]::IsNullOrWhiteSpace($Addr)) {
        return $false
    }
    if ($Addr -notmatch '^(?i)127\.0\.0\.1:(\d+)$') {
        return $false
    }
    $port = [int]$Matches[1]
    return ($port -ge 1 -and $port -le 65535)
}

function Get-SamanthaScreenAddr {
    return '127.0.0.1:7878'
}

function Get-SamanthaCompanionAddr {
    return '127.0.0.1:7881'
}

function Test-SamanthaCompanionAddr {
    param(
        [string]$Addr,
        [string]$ScreenAddr
    )
    if ([string]::IsNullOrWhiteSpace($Addr) -or [string]::IsNullOrWhiteSpace($ScreenAddr)) {
        return $false
    }
    $port = 0
    $matched = $false
    if ($Addr -match '^(?i)127\.0\.0\.1:(\d+)$') {
        $matched = $true
        $port = [int]$Matches[1]
    }
    elseif ($Addr -match '^\[::1\]:(\d+)$') {
        $matched = $true
        $port = [int]$Matches[1]
    }
    if (-not $matched) {
        return $false
    }
    if ($port -lt 1 -or $port -gt 65535) {
        return $false
    }
    if ($Addr -eq $ScreenAddr) {
        return $false
    }
    return $true
}

function Test-SamanthaHealthBody {
    param([string]$Body)
    if ([string]::IsNullOrWhiteSpace($Body)) {
        return $false
    }
    $trim = $Body.Trim()
    if ($trim -match '^(?i)ok$') {
        return $true
    }
    if ($trim -match '(?i)"ok"\s*:\s*true') {
        return $true
    }
    return $false
}

function Get-SamanthaFileSha256 {
    param([Parameter(Mandatory = $true)][string]$Path)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        $bytes = $sha.ComputeHash($stream)
    }
    finally {
        $stream.Dispose()
        $sha.Dispose()
    }
    $text = -join ($bytes | ForEach-Object { $_.ToString('x2') })
    return $text
}

function Confirm-SamanthaFileHash {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Expected
    )
    $want = $Expected.ToLowerInvariant()
    $got = Get-SamanthaFileSha256 -Path $Path
    if ($got -ne $want) {
        Remove-Item -LiteralPath $Path -Force
        throw "sha256 MISMATCH for $Path expected $want got $got"
    }
    return $got
}

function Test-SamanthaZipEntryName {
    param([Parameter(Mandatory = $true)][string]$Name)
    if ([string]::IsNullOrWhiteSpace($Name)) {
        return $false
    }
    $normalized = $Name.Replace('\', '/')
    if ($normalized.StartsWith('/')) {
        return $false
    }
    if ($normalized.Contains(':')) {
        return $false
    }
    $parts = $normalized.Split('/')
    for ($i = 0; $i -lt $parts.Length; $i++) {
        $part = $parts[$i]
        if ($part -eq '..') {
            return $false
        }
        if ($part -eq '' -and $i -ne ($parts.Length - 1)) {
            return $false
        }
    }
    return $true
}

function Confirm-SamanthaRefPath {
    param(
        [string]$Path,
        [Parameter(Mandatory = $true)][string]$RepoRoot
    )
    if ([string]::IsNullOrWhiteSpace($Path)) {
        return
    }
    $trim = $Path.Trim()
    if ($trim -match '^(?i)(https?|ftp):') {
        throw 'GALION_SAMANTHA_REF must be a local file the owner supplies. This installer does not download reference audio.'
    }
    if ($trim -notmatch '(?i)\.(wav|wave)$') {
        throw "GALION_SAMANTHA_REF must be a local .wav path. Got: $trim"
    }
    $normRef = ($trim -replace '/', '\').TrimEnd('\')
    $normRepo = ($RepoRoot -replace '/', '\').TrimEnd('\')
    if ($normRef.StartsWith($normRepo + '\', [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'GALION_SAMANTHA_REF must sit outside the git checkout. Reference audio is not copied and is not committed.'
    }
}

function Format-SamanthaEnvLine {
    param(
        [Parameter(Mandatory = $true)][string]$Key,
        [AllowEmptyString()][string]$Value
    )
    if ($Key -notmatch '^[A-Z][A-Z0-9_]*$') {
        throw "refusing env key: $Key"
    }
    if ($Key -match 'KEY$|_KEY_|SECRET|TOKEN|PASSWORD') {
        throw "refusing to write a secret-shaped env key: $Key"
    }
    if ($null -eq $Value) {
        $Value = ''
    }
    if ($Value -match '[\r\n]') {
        throw "env value for $Key cannot contain a newline"
    }
    if ($Value -match '(?i)BEGIN (RSA |OPENSSH )?PRIVATE KEY|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9]{20,}|ghp_[A-Za-z0-9]{20,}') {
        throw "refusing to write a secret-shaped value for $Key"
    }
    return ($Key + '=' + $Value)
}

function Get-SamanthaLayoutDir {
    return @(
        'bin'
        'weights\asr'
        'weights\tts'
        'weights\vad'
        'weights\runtime'
        'env'
        'logs'
        'tmp'
        'target'
        'cargo-home'
    )
}

function Get-SamanthaEnvDocument {
    param(
        [Parameter(Mandatory = $true)][string]$InstallRoot,
        [AllowEmptyString()][string]$SamanthaRef,
        [Parameter(Mandatory = $true)][string]$Addr,
        [string]$CompanionAddr = '127.0.0.1:7881',
        [Parameter(Mandatory = $true)][int]$Seconds
    )
    $lines = New-Object System.Collections.Generic.List[string]
    [void]$lines.Add('# SPDX-License-Identifier: MIT')
    [void]$lines.Add('# Copyright (c) 2026 Galion Labs')
    [void]$lines.Add('# Local process environment for START-SAMANTHA. Not a machine-wide setting.')
    [void]$lines.Add('# No API key, token, or public model URL is stored here.')
    [void]$lines.Add('# GALION_LLM=canned is the existing offline rule table in galion-cascade.')
    [void]$lines.Add('# It is not the HORBM brain. Do not point this file at a paid API.')
    # Direct calls. An array of pairs would unroll an empty value on Windows PowerShell 5.1.
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_SAMANTHA_HOME' -Value $InstallRoot))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_SAMANTHA_REF' -Value $SamanthaRef))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_VOSK_MODEL' -Value (Join-WinPath -Base $InstallRoot -Child 'weights\asr\vosk-model-small-en-us-0.15')))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_VOSK_LIB' -Value (Join-WinPath -Base $InstallRoot -Child 'weights\asr\libvosk\libvosk.dll')))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_VOSK_BIN' -Value (Join-WinPath -Base $InstallRoot -Child 'bin\galion-asr-vosk.exe')))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_SILERO_VAD' -Value (Join-WinPath -Base $InstallRoot -Child 'weights\vad\silero_vad.onnx')))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_TTS_WEIGHTS' -Value (Join-WinPath -Base $InstallRoot -Child 'weights\tts')))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_A21_RUNTIME' -Value (Join-WinPath -Base $InstallRoot -Child 'weights\runtime\piper\piper.exe')))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_PIPER_LEGACY' -Value '1'))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_SPEECH' -Value 'auto'))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_LLM' -Value 'canned'))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_CLONE_DEVICE' -Value 'cpu'))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_CLONE_PERSONA' -Value 'samantha'))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_CLONE_THREADS' -Value '1'))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'CUDA_VISIBLE_DEVICES' -Value ''))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'V1_SCREEN_ADDR' -Value $Addr))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'GALION_SAMANTHA_WS' -Value $CompanionAddr))
    [void]$lines.Add((Format-SamanthaEnvLine -Key 'SAMANTHA_SECONDS' -Value ([string]$Seconds)))
    return (($lines.ToArray()) -join "`r`n") + "`r`n"
}

function Write-SamanthaUtf8 {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Text
    )
    Confirm-SamanthaWritePath -Path $Path
    $parent = Split-Path -Path $Path -Parent
    if (-not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    $utf8 = New-Object System.Text.UTF8Encoding $false
    [System.IO.File]::WriteAllText($Path, $Text, $utf8)
}

function Get-SamanthaLauncherText {
    param([Parameter(Mandatory = $true)][string]$StartScript)
    $lines = @(
        '@echo off'
        'REM SPDX-License-Identifier: MIT'
        'REM Copyright (c) 2026 Galion Labs'
        'REM Generated by Install-Samantha.ps1. Local server and loopback UI only.'
        'setlocal'
        ('"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "' + $StartScript + '" %*')
        'exit /b %ERRORLEVEL%'
    )
    return (($lines -join "`r`n") + "`r`n")
}

function Test-SamanthaWindowsHost {
    return ($env:OS -eq 'Windows_NT')
}

function Get-SamanthaRustReport {
    if (-not (Test-SamanthaWindowsHost)) {
        return @{ Ok = $false; Skipped = $true; Message = 'Rust check skipped (not Windows).' }
    }
    $rustc = Get-Command -Name rustc -ErrorAction SilentlyContinue
    if ($null -eq $rustc) {
        return @{
            Ok = $false
            Skipped = $false
            Message = 'rustc is not on PATH. Need Rust 1.85 or newer, host x86_64-pc-windows-msvc. This script does not run rustup-init and does not write a toolchain onto C:.'
        }
    }
    $versionText = (& rustc --version)
    $verbose = @(& rustc -vV)
    $okVersion = $false
    if ($versionText -match 'rustc (\d+)\.(\d+)\.(\d+)') {
        $parsed = [version]($Matches[1] + '.' + $Matches[2] + '.' + $Matches[3])
        if ($parsed -ge [version]'1.85.0') {
            $okVersion = $true
        }
    }
    $hostLine = ''
    foreach ($line in $verbose) {
        if ($line -like 'host:*') {
            $hostLine = $line
        }
    }
    $msvcHost = ($hostLine -match 'x86_64-pc-windows-msvc')
    if ($okVersion -and $msvcHost) {
        return @{ Ok = $true; Skipped = $false; Message = ('Rust: ' + $versionText + ' (' + $hostLine + ')') }
    }
    return @{
        Ok = $false
        Skipped = $false
        Message = ('Rust is present but not usable here: ' + $versionText + ' ; ' + $hostLine + '. Need rustc >= 1.85 and host x86_64-pc-windows-msvc.')
    }
}

function Get-SamanthaMsvcReport {
    if (-not (Test-SamanthaWindowsHost)) {
        return @{ Ok = $false; Skipped = $true; Message = 'MSVC check skipped (not Windows).' }
    }
    $programFilesX86 = [Environment]::GetEnvironmentVariable('ProgramFiles(x86)')
    if (-not [string]::IsNullOrWhiteSpace($programFilesX86)) {
        $vswhere = Join-Path -Path $programFilesX86 -ChildPath 'Microsoft Visual Studio\Installer\vswhere.exe'
        if (Test-Path -LiteralPath $vswhere) {
            $found = @(& $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
            if ($LASTEXITCODE -eq 0 -and $found.Count -gt 0 -and -not [string]::IsNullOrWhiteSpace([string]$found[0])) {
                return @{ Ok = $true; Skipped = $false; Message = ('MSVC C++ tools: ' + [string]$found[0]) }
            }
        }
    }
    $cl = Get-Command -Name cl.exe -ErrorAction SilentlyContinue
    if ($null -ne $cl) {
        return @{ Ok = $true; Skipped = $false; Message = ('MSVC cl.exe: ' + $cl.Source) }
    }
    return @{
        Ok = $false
        Skipped = $false
        Message = 'MSVC C++ tools were not found (vswhere component VC.Tools.x86.x64, or cl.exe on PATH). This script does not download Visual Studio and does not write it onto C:.'
    }
}

function Invoke-SamanthaDownload {
    param(
        [Parameter(Mandatory = $true)][string]$Url,
        [Parameter(Mandatory = $true)][string]$Destination
    )
    Confirm-SamanthaWritePath -Path $Destination
    $script:SamanthaNetworkCalls = $script:SamanthaNetworkCalls + 1
    $curl = Get-Command -Name curl.exe -ErrorAction SilentlyContinue
    if ($null -eq $curl) {
        Invoke-WebRequest -Uri $Url -OutFile $Destination -UseBasicParsing
        return
    }
    & curl.exe -fL --retry 3 --output $Destination $Url
    if ($LASTEXITCODE -ne 0) {
        if (Test-Path -LiteralPath $Destination) {
            Remove-Item -LiteralPath $Destination -Force
        }
        throw "download failed ($LASTEXITCODE): $Url"
    }
}

function Expand-SamanthaZip {
    param(
        [Parameter(Mandatory = $true)][string]$ZipPath,
        [Parameter(Mandatory = $true)][string]$Destination
    )
    Confirm-SamanthaWritePath -Path $Destination
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [System.IO.Compression.ZipFile]::OpenRead($ZipPath)
    try {
        foreach ($entry in $zip.Entries) {
            if (-not (Test-SamanthaZipEntryName -Name $entry.FullName)) {
                throw ('refusing zip entry: ' + $entry.FullName)
            }
            if ($entry.FullName.EndsWith('/')) {
                continue
            }
            $relative = $entry.FullName.Replace('/', '\')
            $outPath = Join-WinPath -Base $Destination -Child $relative
            Confirm-SamanthaWritePath -Path $outPath
            $parent = Split-Path -Path $outPath -Parent
            if (-not (Test-Path -LiteralPath $parent)) {
                New-Item -ItemType Directory -Path $parent -Force | Out-Null
            }
            $outStream = [System.IO.File]::Open($outPath, [System.IO.FileMode]::Create)
            try {
                $inStream = $entry.Open()
                try {
                    $inStream.CopyTo($outStream)
                }
                finally {
                    $inStream.Dispose()
                }
            }
            finally {
                $outStream.Dispose()
            }
        }
    }
    finally {
        $zip.Dispose()
    }
}

function Test-SamanthaPinReady {
    param(
        [Parameter(Mandatory = $true)][string]$InstallRoot,
        $Pin
    )
    $ready = Join-WinPath -Base $InstallRoot -Child $Pin.ReadyFile
    if (-not (Test-Path -LiteralPath $ready)) {
        return $false
    }
    if (-not [string]::IsNullOrWhiteSpace([string]$Pin.AlsoReady)) {
        $also = Join-WinPath -Base $InstallRoot -Child $Pin.AlsoReady
        if (-not (Test-Path -LiteralPath $also)) {
            return $false
        }
    }
    if ($Pin.Kind -eq 'file') {
        $got = Get-SamanthaFileSha256 -Path $ready
        return ($got -eq $Pin.Sha256.ToLowerInvariant())
    }
    return $true
}

function Invoke-SamanthaPin {
    param(
        [Parameter(Mandatory = $true)][string]$InstallRoot,
        $Pin,
        [switch]$PlanOnly
    )
    $ready = Join-WinPath -Base $InstallRoot -Child $Pin.ReadyFile
    Write-Output ('pin: ' + $Pin.Id + ' (' + $Pin.License + ')')
    Write-Output ('pin: ' + $Pin.Note)
    if ($PlanOnly) {
        Write-Output ('dry-run: get  ' + $Pin.Url)
        Write-Output ('dry-run: sha256 ' + $Pin.Sha256)
        Write-Output ('dry-run: dest ' + $ready)
        return
    }
    if (Test-SamanthaPinReady -InstallRoot $InstallRoot -Pin $Pin) {
        Write-Output ('ok   ' + $ready)
        return
    }
    $part = Join-WinPath -Base $InstallRoot -Child ('tmp\' + $Pin.FileName + '.part')
    Confirm-SamanthaWritePath -Path $part
    Write-Output ('get  ' + $Pin.Url)
    Invoke-SamanthaDownload -Url $Pin.Url -Destination $part
    Confirm-SamanthaFileHash -Path $part -Expected $Pin.Sha256 | Out-Null
    if ($Pin.Kind -eq 'file') {
        $destDir = Split-Path -Path $ready -Parent
        if (-not (Test-Path -LiteralPath $destDir)) {
            New-Item -ItemType Directory -Path $destDir -Force | Out-Null
        }
        if (Test-Path -LiteralPath $ready) {
            Remove-Item -LiteralPath $ready -Force
        }
        [System.IO.File]::Move($part, $ready)
        if ($Pin.WriteShaSidecar) {
            $side = $ready + '.sha256'
            $sideText = $Pin.Sha256.ToLowerInvariant() + '  ' + $Pin.FileName + "`n"
            Write-SamanthaUtf8 -Path $side -Text $sideText
        }
        Write-Output ('ok   ' + $ready)
        return
    }
    $unpack = Join-WinPath -Base $InstallRoot -Child ('tmp\unpack-' + $Pin.Id)
    if (Test-Path -LiteralPath $unpack) {
        Remove-Item -LiteralPath $unpack -Recurse -Force
    }
    New-Item -ItemType Directory -Path $unpack -Force | Out-Null
    Expand-SamanthaZip -ZipPath $part -Destination $unpack
    Remove-Item -LiteralPath $part -Force
    $unpacked = Join-WinPath -Base $unpack -Child $Pin.UnpackFolder
    if (-not (Test-Path -LiteralPath $unpacked)) {
        throw ('archive did not contain ' + $Pin.UnpackFolder)
    }
    if (-not [string]::IsNullOrWhiteSpace([string]$Pin.PlaceAs)) {
        $place = Join-WinPath -Base $InstallRoot -Child $Pin.PlaceAs
    }
    else {
        $place = Join-WinPath -Base $InstallRoot -Child (Join-WinPath -Base $Pin.DestDir -Child $Pin.UnpackFolder)
    }
    Confirm-SamanthaWritePath -Path $place
    if (Test-Path -LiteralPath $place) {
        Remove-Item -LiteralPath $place -Recurse -Force
    }
    $placeParent = Split-Path -Path $place -Parent
    if (-not (Test-Path -LiteralPath $placeParent)) {
        New-Item -ItemType Directory -Path $placeParent -Force | Out-Null
    }
    [System.IO.Directory]::Move($unpacked, $place)
    Remove-Item -LiteralPath $unpack -Recurse -Force -ErrorAction SilentlyContinue
    if (-not (Test-SamanthaPinReady -InstallRoot $InstallRoot -Pin $Pin)) {
        throw ('unpack did not produce ' + $ready)
    }
    Write-Output ('ok   ' + $ready)
}

function Invoke-SamanthaCargoBuild {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string]$InstallRoot,
        [Parameter(Mandatory = $true)][string[]]$CargoArgs,
        [switch]$PlanOnly
    )
    $target = Join-WinPath -Base $InstallRoot -Child 'target'
    $cargoHome = Join-WinPath -Base $InstallRoot -Child 'cargo-home'
    $tmp = Join-WinPath -Base $InstallRoot -Child 'tmp'
    $line = 'cargo ' + ($CargoArgs -join ' ')
    if ($PlanOnly) {
        Write-Output ('dry-run: ' + $line)
        Write-Output ('dry-run: CARGO_TARGET_DIR=' + $target)
        if ($env:SAMANTHA_KEEP_CARGO_HOME -eq '1') {
            Write-Output 'dry-run: CARGO_HOME left as the owner already set it (SAMANTHA_KEEP_CARGO_HOME=1)'
        }
        else {
            Write-Output ('dry-run: CARGO_HOME=' + $cargoHome)
        }
        Write-Output ('dry-run: TMP=' + $tmp)
        return
    }
    Confirm-SamanthaWritePath -Path $target
    $env:CARGO_TARGET_DIR = $target
    if ($env:SAMANTHA_KEEP_CARGO_HOME -ne '1') {
        Confirm-SamanthaWritePath -Path $cargoHome
        $env:CARGO_HOME = $cargoHome
    }
    $env:TMP = $tmp
    $env:TEMP = $tmp
    Push-Location -LiteralPath $RepoRoot
    try {
        Write-Output $line
        & cargo @CargoArgs
        if ($LASTEXITCODE -ne 0) {
            throw "cargo failed ($LASTEXITCODE): $line"
        }
    }
    finally {
        Pop-Location
    }
}

function Copy-SamanthaBuiltBinary {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination,
        [switch]$PlanOnly
    )
    if ($PlanOnly) {
        Write-Output ('dry-run: copy ' + $Source + ' -> ' + $Destination)
        return
    }
    if (-not (Test-Path -LiteralPath $Source)) {
        throw "build did not produce $Source"
    }
    Confirm-SamanthaWritePath -Path $Destination
    Copy-Item -LiteralPath $Source -Destination $Destination -Force
    Write-Output ('ok   ' + $Destination)
}

function Import-SamanthaEnvFile {
    param([Parameter(Mandatory = $true)][string]$Path)
    $raw = [System.IO.File]::ReadAllText($Path)
    if ($raw.Length -gt 0 -and [int][char]$raw[0] -eq 0xFEFF) {
        $raw = $raw.Substring(1)
    }
    foreach ($line in ($raw -split "`r?`n")) {
        $trim = $line.Trim()
        if ($trim -eq '' -or $trim.StartsWith('#')) {
            continue
        }
        $splitAt = $trim.IndexOf('=')
        if ($splitAt -lt 1) {
            throw "bad env line: $trim"
        }
        $key = $trim.Substring(0, $splitAt)
        $value = $trim.Substring($splitAt + 1)
        if ($key -match 'KEY$|_KEY_|SECRET|TOKEN|PASSWORD') {
            throw "refusing to load a secret-shaped env key: $key"
        }
        Set-Item -Path ('Env:' + $key) -Value $value
    }
}

function Wait-SamanthaHealth {
    param(
        [Parameter(Mandatory = $true)][string]$Url,
        [int]$Tries = 50,
        [switch]$RequireOk
    )
    for ($i = 0; $i -lt $Tries; $i++) {
        try {
            $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 2
            if ($response.StatusCode -eq 200) {
                if (-not $RequireOk) {
                    return $true
                }
                if (Test-SamanthaHealthBody -Body ([string]$response.Content)) {
                    return $true
                }
            }
        }
        catch {
            $null = $_.Exception.Message
        }
        Start-Sleep -Milliseconds 100
    }
    return $false
}

function Invoke-SamanthaInstall {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$CommonRoot,
        [switch]$PlanOnly,
        [switch]$Check,
        [string]$InstallRoot,
        [string]$RepoRoot,
        [string]$SamanthaRef,
        [int]$Port = 7878,
        [int]$Seconds = 3600,
        [switch]$SkipBuild
    )
    $script:SamanthaExitCode = 0
    $resolvedHome = Get-SamanthaHome -InstallRoot $InstallRoot
    if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
        $RepoRoot = Get-SamanthaRepoRoot -CommonRoot $CommonRoot
    }
    $config = Get-SamanthaConfig -CommonRoot $CommonRoot
    if ($Port -eq [int]$config.CompanionPort) {
        throw "port $Port is the companion socket. The one-dot screen stays on $($config.ScreenPort)."
    }
    $addr = Get-SamanthaWebAddr -Port $Port
    $companion = Get-SamanthaWebAddr -Port ([int]$config.CompanionPort)
    if ($Seconds -lt 1 -or $Seconds -gt 86400) {
        throw "SAMANTHA_SECONDS must be 1..86400. Got: $Seconds"
    }
    if ([string]::IsNullOrWhiteSpace($SamanthaRef) -and -not [string]::IsNullOrWhiteSpace($env:GALION_SAMANTHA_REF)) {
        $SamanthaRef = $env:GALION_SAMANTHA_REF
    }
    Confirm-SamanthaRefPath -Path $SamanthaRef -RepoRoot $RepoRoot

    Write-Output 'Samantha Windows install'
    Write-Output ('home: ' + $resolvedHome)
    Write-Output ('repo: ' + $RepoRoot)
    Write-Output ('ui:   http://' + $addr + '/  (one-dot screen)')
    Write-Output ('ws:   http://' + $companion + '/health  (' + $config.CompanionSchema + ')')
    Write-Output 'heartbeat: galion-heartbeat FRAME_MS is 80. This installer does not change that clock.'
    Write-Output 'mouth: stock Piper en_US-ljspeech-medium via piper.exe. Not a film voice and not a clone.'
    $protected = @($config.ProtectedScheduledTasks) -join ', '
    Write-Output ('protected scheduled tasks left untouched: ' + $protected)
    Write-Output 'services, drivers, GPU or CPU acceleration software, and Windows system files are not modified.'
    Write-Output 'galion-piper-rt is not built on Windows. runtimes/galion-piper-rt uses libc dlopen and sched_setaffinity. GALION_PIPER_LEGACY=1 selects the pinned piper 2023.11.14-2 CLI.'
    Write-Output 'scripts/clone/chatterbox_worker.py is not started. Clone weights are not downloaded. GALION_CLONE_FETCH is not set. GALION_CLONE_DEVICE=cpu is recorded for this process only.'

    if ($Check -and -not (Test-SamanthaWindowsHost)) {
        Write-Output 'Check mode is for the Windows laptop. No files were written.'
        $script:SamanthaExitCode = 2
        return
    }
    if ($Check) {
        foreach ($pin in $config.Downloads) {
            $ready = Join-WinPath -Base $resolvedHome -Child $pin.ReadyFile
            if (-not (Test-Path -LiteralPath $ready)) {
                Write-Output ('MISSING ' + $ready)
                $script:SamanthaExitCode = 1
                continue
            }
            if ($pin.Kind -eq 'file') {
                $got = Get-SamanthaFileSha256 -Path $ready
                if ($got -ne $pin.Sha256.ToLowerInvariant()) {
                    Write-Output ('BAD ' + $ready)
                    $script:SamanthaExitCode = 1
                }
                else {
                    Write-Output ('ok   ' + $ready)
                }
            }
            else {
                Write-Output ('ok   ' + $ready)
            }
        }
        return
    }

    $rust = Get-SamanthaRustReport
    $msvc = Get-SamanthaMsvcReport
    Write-Output $rust.Message
    Write-Output $msvc.Message
    if (-not $PlanOnly) {
        if (-not (Test-SamanthaWindowsHost)) {
            Write-Output 'Install-Samantha.ps1 runs on Windows. This host is not Windows. No files were written.'
            $script:SamanthaExitCode = 2
            return
        }
        if (-not $rust.Ok -or -not $msvc.Ok) {
            Write-Output 'Install stopped before any download or build. Fix Rust and MSVC, then run this script again.'
            $script:SamanthaExitCode = 2
            return
        }
    }

    foreach ($dir in (Get-SamanthaLayoutDir)) {
        $full = Join-WinPath -Base $resolvedHome -Child $dir
        if ($PlanOnly) {
            Write-Output ('dry-run: mkdir ' + $full)
        }
        else {
            Confirm-SamanthaWritePath -Path $full
            New-Item -ItemType Directory -Path $full -Force | Out-Null
        }
    }

    foreach ($pin in $config.Downloads) {
        Invoke-SamanthaPin -InstallRoot $resolvedHome -Pin $pin -PlanOnly:$PlanOnly
    }

    $envText = Get-SamanthaEnvDocument -InstallRoot $resolvedHome -SamanthaRef $SamanthaRef -Addr $addr -CompanionAddr $companion -Seconds $Seconds
    $envPath = Join-WinPath -Base $resolvedHome -Child 'env\samantha.env'
    $startScript = Join-Path -Path $CommonRoot -ChildPath 'Start-Samantha.ps1'
    $launcherPath = Join-WinPath -Base $resolvedHome -Child 'START-SAMANTHA.cmd'
    if ($PlanOnly) {
        Write-Output ('dry-run: write ' + $envPath)
        if ([string]::IsNullOrWhiteSpace($SamanthaRef)) {
            Write-Output 'dry-run: GALION_SAMANTHA_REF is empty. Pass -SamanthaRef with a local .wav outside the git checkout. The file is not copied.'
        }
        else {
            Write-Output ('dry-run: GALION_SAMANTHA_REF=' + $SamanthaRef + ' (path only, file is not copied)')
        }
        Write-Output ('dry-run: write ' + $launcherPath)
        Write-Output 'dry-run: no download, no build, no write'
    }
    else {
        Write-SamanthaUtf8 -Path $envPath -Text $envText
        Write-Output ('ok   ' + $envPath)
        if ([string]::IsNullOrWhiteSpace($SamanthaRef)) {
            Write-Output 'GALION_SAMANTHA_REF is empty. Stock Piper speaks until you set a local .wav and run this script again. Nothing was downloaded for that reference.'
        }
        else {
            Write-Output ('GALION_SAMANTHA_REF=' + $SamanthaRef + ' (path recorded, file not copied)')
        }
        $launcher = Get-SamanthaLauncherText -StartScript $startScript
        Write-SamanthaUtf8 -Path $launcherPath -Text $launcher
        Write-Output ('ok   ' + $launcherPath)
    }

    if ($SkipBuild) {
        Write-Output 'build skipped (-SkipBuild)'
    }
    else {
        foreach ($build in $config.Builds) {
            Invoke-SamanthaCargoBuild -RepoRoot $RepoRoot -InstallRoot $resolvedHome -CargoArgs $build.CargoArgs -PlanOnly:$PlanOnly
            $built = Join-WinPath -Base $resolvedHome -Child $build.OutputRel
            $installed = Join-WinPath -Base $resolvedHome -Child ('bin\' + $build.InstallName)
            Copy-SamanthaBuiltBinary -Source $built -Destination $installed -PlanOnly:$PlanOnly
        }
    }

    Write-Output ('start: ' + $launcherPath)
    Write-Output ('or:    ' + $startScript)
    if ($PlanOnly) {
        $script:SamanthaExitCode = 0
    }
}

function Invoke-SamanthaStart {
    [CmdletBinding()]
    param(
        [string]$InstallRoot,
        [switch]$PlanOnly
    )
    $script:SamanthaExitCode = 0
    $resolvedHome = Get-SamanthaHome -InstallRoot $InstallRoot
    $addr = Get-SamanthaScreenAddr
    $companion = Get-SamanthaCompanionAddr
    $seconds = 3600
    $exe = Join-WinPath -Base $resolvedHome -Child 'bin\dot_console_demo.exe'
    $socketExe = Join-WinPath -Base $resolvedHome -Child 'bin\samantha-local.exe'
    Write-Output 'Samantha local server. Loopback only.'
    Write-Output 'The one-dot page stays on 127.0.0.1:7878. The companion socket is galion-samantha-ws/1 on 127.0.0.1:7881.'
    Write-Output 'Heartbeat FRAME_MS is 80.'
    if ($PlanOnly) {
        Write-Output ('dry-run: start ' + $exe + ' --headless --realtime --no-png --web ' + $addr + ' --seconds ' + $seconds)
        Write-Output ('dry-run: start ' + $socketExe + ' --addr ' + $companion)
        Write-Output ('dry-run: GET http://' + $companion + '/health  (galion-samantha-ws/1, 200 and ok)')
        Write-Output ('dry-run: open http://' + $addr + '/')
        Write-Output 'dry-run: no process started and no browser opened'
        Write-Output 'dry-run: scheduled tasks, services, drivers, and Windows system files are not modified'
        $script:SamanthaExitCode = 0
        return
    }
    if (-not (Test-SamanthaWindowsHost)) {
        Write-Output 'Start-Samantha.ps1 runs on Windows. This host is not Windows. No process was started.'
        $script:SamanthaExitCode = 2
        return
    }
    $envFile = Join-WinPath -Base $resolvedHome -Child 'env\samantha.env'
    if (-not (Test-Path -LiteralPath $envFile)) {
        Write-Output ('missing ' + $envFile + '. Run Install-Samantha.ps1 first.')
        $script:SamanthaExitCode = 1
        return
    }
    Import-SamanthaEnvFile -Path $envFile
    if (-not (Test-SamanthaLoopbackAddr -Addr $env:V1_SCREEN_ADDR)) {
        throw ('refusing one-dot address ' + $env:V1_SCREEN_ADDR + '. Only 127.0.0.1:port is accepted.')
    }
    $addr = $env:V1_SCREEN_ADDR
    if (-not [string]::IsNullOrWhiteSpace([string]$env:GALION_SAMANTHA_WS)) {
        $companion = [string]$env:GALION_SAMANTHA_WS
    }
    if (-not (Test-SamanthaCompanionAddr -Addr $companion -ScreenAddr $addr)) {
        throw ('refusing companion address ' + $companion + '. galion-samantha-ws/1 stays on 127.0.0.1 or [::1], and it is not the one-dot screen.')
    }
    $parsedSeconds = 0
    if (-not [int]::TryParse($env:SAMANTHA_SECONDS, [ref]$parsedSeconds)) {
        throw 'SAMANTHA_SECONDS is not an integer'
    }
    if ($parsedSeconds -lt 1 -or $parsedSeconds -gt 86400) {
        throw 'SAMANTHA_SECONDS must be 1..86400'
    }
    $seconds = $parsedSeconds
    $exe = Join-WinPath -Base $resolvedHome -Child 'bin\dot_console_demo.exe'
    if ($exe.ToLowerInvariant().StartsWith('g:\galion\samantha\bin\') -eq $false) {
        throw "refusing to start a binary outside G:\galion\samantha\bin: $exe"
    }
    if (-not (Test-Path -LiteralPath $exe)) {
        Write-Output ('missing ' + $exe + '. Run Install-Samantha.ps1 first.')
        $script:SamanthaExitCode = 1
        return
    }
    $mode = [string]$env:GALION_ASR
    if ($mode -eq 'interim') {
        Write-Output 'refusing GALION_ASR=interim on the live screen. Placeholder transcripts are test-only.'
        $mode = ''
    }
    $model = [string]$env:GALION_VOSK_MODEL
    $finalMdl = Join-WinPath -Base $model -Child 'am\final.mdl'
    $modelConf = Join-WinPath -Base $model -Child 'conf\model.conf'
    if ([string]::IsNullOrWhiteSpace($mode)) {
        if ((Test-Path -LiteralPath $finalMdl) -and (Test-Path -LiteralPath $modelConf)) {
            $env:GALION_ASR = 'vosk'
            Write-Output ('mic ASR: vosk (' + $model + ')')
        }
        else {
            $env:GALION_ASR = 'stub'
            Write-Output ('mic ASR: Vosk model missing at ' + $model)
            Write-Output 'typed text still works. Mic turns do not invent a transcript.'
        }
    }
    else {
        $env:GALION_ASR = $mode
        Write-Output ('mic ASR: ' + $env:GALION_ASR)
    }
    $piper = [string]$env:GALION_A21_RUNTIME
    if (-not (Test-Path -LiteralPath $piper)) {
        Write-Output ('Piper runtime missing at ' + $piper + '. The UI can still open. Speech falls through inside the existing stack.')
    }
    $vad = [string]$env:GALION_SILERO_VAD
    if (-not (Test-Path -LiteralPath $vad)) {
        Write-Output ('Silero VAD file missing at ' + $vad + '. dot_console_demo does not load that ONNX itself.')
    }
    else {
        Write-Output ('Silero VAD file: ' + $vad + '. The current one-dot server does not run this ONNX in-process.')
    }
    $ref = [string]$env:GALION_SAMANTHA_REF
    if ([string]::IsNullOrWhiteSpace($ref)) {
        Write-Output 'GALION_SAMANTHA_REF is empty. Stock Piper voice speaks. No reference audio is loaded.'
    }
    elseif (-not (Test-Path -LiteralPath $ref)) {
        Write-Output ('GALION_SAMANTHA_REF is not a file yet: ' + $ref)
        Write-Output 'Stock Piper voice speaks until that local wav is present. It was not downloaded.'
    }
    else {
        Write-Output ('GALION_SAMANTHA_REF is a local file. It is not copied.')
    }
    $piperDir = Split-Path -Path $piper -Parent
    $voskDir = Split-Path -Path ([string]$env:GALION_VOSK_LIB) -Parent
    $prefix = @()
    if (-not [string]::IsNullOrWhiteSpace($piperDir)) {
        $prefix += $piperDir
    }
    if (-not [string]::IsNullOrWhiteSpace($voskDir)) {
        $prefix += $voskDir
    }
    if ($prefix.Count -gt 0) {
        $env:PATH = (($prefix -join ';') + ';' + $env:PATH)
    }
    $argList = @(
        '--headless'
        '--realtime'
        '--no-png'
        '--web'
        $addr
        '--seconds'
        ([string]$seconds)
    )
    Write-Output ('start ' + $exe + ' ' + ($argList -join ' '))
    Write-Output 'GALION_LLM=canned uses the existing offline rule table. It is not HORBM and it does not call a paid API.'
    if (-not (Test-Path -LiteralPath $socketExe)) {
        Write-Output ('missing ' + $socketExe + '. The companion health check is GET http://' + $companion + '/health (galion-samantha-ws/1). This checkout does not build that binary.')
        $script:SamanthaExitCode = 1
        return
    }
    $screenOut = Join-WinPath -Base $resolvedHome -Child 'logs\screen.out.log'
    $screenErr = Join-WinPath -Base $resolvedHome -Child 'logs\screen.err.log'
    $socketOut = Join-WinPath -Base $resolvedHome -Child 'logs\companion.out.log'
    $socketErr = Join-WinPath -Base $resolvedHome -Child 'logs\companion.err.log'
    $env:GALION_SAMANTHA_WS = $companion
    $screen = $null
    $socket = $null
    try {
        $screen = Start-Process -FilePath $exe -ArgumentList $argList -WorkingDirectory $resolvedHome -RedirectStandardOutput $screenOut -RedirectStandardError $screenErr -PassThru
        Write-Output ('start ' + $socketExe + ' --addr ' + $companion)
        $socket = Start-Process -FilePath $socketExe -ArgumentList @('--addr', $companion) -WorkingDirectory $resolvedHome -RedirectStandardOutput $socketOut -RedirectStandardError $socketErr -PassThru
        $health = 'http://' + $companion + '/health'
        $healthy = Wait-SamanthaHealth -Url $health -RequireOk
        if (-not $healthy) {
            Write-Output ('companion did not answer ' + $health + ' with 200 and ok (galion-samantha-ws/1)')
            if (Test-Path -LiteralPath $socketErr) {
                Get-Content -LiteralPath $socketErr -Tail 40 | ForEach-Object { Write-Output $_ }
            }
            $script:SamanthaExitCode = 1
            return
        }
        $page = 'http://' + $addr + '/'
        try {
            Start-Process -FilePath $page
        }
        catch {
            Write-Output ('browser did not open: ' + $_.Exception.Message)
            Write-Output ('open ' + $page + ' yourself')
        }
        Write-Output ('one-dot screen: ' + $page)
        Write-Output ('companion health: ' + $health)
        Wait-Process -Id $screen.Id
        $script:SamanthaExitCode = 0
    }
    finally {
        if ($null -ne $socket -and -not $socket.HasExited) {
            Stop-Process -Id $socket.Id -Force -ErrorAction SilentlyContinue
        }
        if ($null -ne $screen -and -not $screen.HasExited) {
            Stop-Process -Id $screen.Id -Force -ErrorAction SilentlyContinue
        }
    }
}
