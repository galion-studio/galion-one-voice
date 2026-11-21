# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
# Dry-run, pin, and guard tests. They do not download models or build Rust.

Set-StrictMode -Version 2.0

BeforeAll {
    $script:Sector = Split-Path -Path $PSScriptRoot -Parent
    $script:Scripts = Join-Path -Path $script:Sector -ChildPath 'scripts'
    $script:Repo = Split-Path -Path (Split-Path -Path (Split-Path -Path $script:Scripts -Parent) -Parent) -Parent
    . (Join-Path -Path $script:Scripts -ChildPath 'Samantha.Common.ps1')
    $script:Config = Get-SamanthaConfig -CommonRoot $script:Scripts
}

Describe 'home and paths' {
    It 'accepts only G:\galion\samantha' {
        Test-SamanthaHome -InstallRoot 'G:\galion\samantha' | Should -BeTrue
        Test-SamanthaHome -InstallRoot 'g:\galion\samantha\' | Should -BeTrue
        Test-SamanthaHome -InstallRoot 'C:\galion\samantha' | Should -BeFalse
        Test-SamanthaHome -InstallRoot 'D:\galion\samantha' | Should -BeFalse
        Test-SamanthaHome -InstallRoot 'G:\galion' | Should -BeFalse
        Test-SamanthaHome -InstallRoot 'G:\galion\samantha\extra' | Should -BeFalse
    }

    It 'refuses a home that is not the Samantha folder' {
        { Get-SamanthaHome -InstallRoot 'C:\Users\Public\samantha' } | Should -Throw '*G:\galion\samantha*'
    }

    It 'refuses writes outside the Samantha folder' {
        { Confirm-SamanthaWritePath -Path 'C:\Windows\System32\drivers\etc\hosts' } | Should -Throw '*outside*'
        { Confirm-SamanthaWritePath -Path 'C:\galion\samantha\weights\tts\voice.onnx' } | Should -Throw '*outside*'
        { Confirm-SamanthaWritePath -Path 'G:\galion\samantha\weights\tts\en_US-ljspeech-medium.onnx' } | Should -Not -Throw
    }

    It 'binds the UI to 127.0.0.1 only' {
        Get-SamanthaWebAddr -Port 7878 | Should -Be '127.0.0.1:7878'
        Get-SamanthaScreenAddr | Should -Be '127.0.0.1:7878'
        Get-SamanthaCompanionAddr | Should -Be '127.0.0.1:7881'
        Test-SamanthaLoopbackAddr -Addr '127.0.0.1:7878' | Should -BeTrue
        Test-SamanthaLoopbackAddr -Addr '0.0.0.0:7878' | Should -BeFalse
        Test-SamanthaLoopbackAddr -Addr 'localhost:7878' | Should -BeFalse
        Test-SamanthaLoopbackAddr -Addr '192.168.1.20:7878' | Should -BeFalse
        Test-SamanthaCompanionAddr -Addr '127.0.0.1:7881' -ScreenAddr '127.0.0.1:7878' | Should -BeTrue
        Test-SamanthaCompanionAddr -Addr '[::1]:7881' -ScreenAddr '127.0.0.1:7878' | Should -BeTrue
        Test-SamanthaCompanionAddr -Addr '127.0.0.1:7878' -ScreenAddr '127.0.0.1:7878' | Should -BeFalse
        Test-SamanthaCompanionAddr -Addr '0.0.0.0:7881' -ScreenAddr '127.0.0.1:7878' | Should -BeFalse
        Test-SamanthaHealthBody -Body 'ok' | Should -BeTrue
        Test-SamanthaHealthBody -Body '{"ok":true}' | Should -BeTrue
        Test-SamanthaHealthBody -Body '{"ok": true, "schema":"galion-samantha-ws/1"}' | Should -BeTrue
        Test-SamanthaHealthBody -Body '{"ok":false}' | Should -BeFalse
        Test-SamanthaHealthBody -Body '' | Should -BeFalse
        { Get-SamanthaWebAddr -Port 0 } | Should -Throw '*port*'
    }
}

Describe 'reference audio' {
    It 'records a local wav outside the checkout and does not accept a URL' {
        $repo = 'C:\src\galion-one-voice'
        { Confirm-SamanthaRefPath -Path '' -RepoRoot $repo } | Should -Not -Throw
        { Confirm-SamanthaRefPath -Path 'G:\galion\samantha\owner-ref.wav' -RepoRoot $repo } | Should -Not -Throw
        { Confirm-SamanthaRefPath -Path 'D:\voices\owner.wav' -RepoRoot $repo } | Should -Not -Throw
        { Confirm-SamanthaRefPath -Path 'https://example.com/samantha.wav' -RepoRoot $repo } | Should -Throw '*local file*'
        { Confirm-SamanthaRefPath -Path 'G:\galion\samantha\owner.mp3' -RepoRoot $repo } | Should -Throw '*.wav*'
        { Confirm-SamanthaRefPath -Path 'C:\src\galion-one-voice\docs\galion-voice-v2\personas\refs\samantha.wav' -RepoRoot $repo } | Should -Throw '*outside the git checkout*'
    }
}

Describe 'hashes and archives' {
    It 'hashes the NIST abc vector' {
        $path = Join-Path -Path ([System.IO.Path]::GetTempPath()) -ChildPath ('s14-abc-' + [guid]::NewGuid().ToString('n') + '.txt')
        try {
            [System.IO.File]::WriteAllBytes($path, [byte[]](97, 98, 99))
            $hash = Get-SamanthaFileSha256 -Path $path
            $hash | Should -Be 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
        }
        finally {
            if (Test-Path -LiteralPath $path) {
                Remove-Item -LiteralPath $path -Force
            }
        }
    }

    It 'deletes a file whose sha256 does not match' {
        $path = Join-Path -Path ([System.IO.Path]::GetTempPath()) -ChildPath ('s14-bad-' + [guid]::NewGuid().ToString('n') + '.txt')
        [System.IO.File]::WriteAllBytes($path, [byte[]](97, 98, 99))
        { Confirm-SamanthaFileHash -Path $path -Expected '0000000000000000000000000000000000000000000000000000000000000000' } | Should -Throw '*MISMATCH*'
        Test-Path -LiteralPath $path | Should -BeFalse
    }

    It 'rejects zip entries that escape the destination' {
        Test-SamanthaZipEntryName -Name 'piper/piper.exe' | Should -BeTrue
        Test-SamanthaZipEntryName -Name 'piper/' | Should -BeTrue
        Test-SamanthaZipEntryName -Name '../Windows/notepad.exe' | Should -BeFalse
        Test-SamanthaZipEntryName -Name '/etc/passwd' | Should -BeFalse
        Test-SamanthaZipEntryName -Name 'C:/Windows/notepad.exe' | Should -BeFalse
        Test-SamanthaZipEntryName -Name 'piper/../../outside.dll' | Should -BeFalse
    }
}

Describe 'env file' {
    It 'points at the local reference and does not store a secret' {
        $text = Get-SamanthaEnvDocument -InstallRoot 'G:\galion\samantha' -SamanthaRef 'D:\voices\owner.wav' -Addr '127.0.0.1:7878' -Seconds 3600
        $text | Should -Match 'GALION_SAMANTHA_REF=D:\\voices\\owner\.wav'
        $text | Should -Match 'GALION_LLM=canned'
        $text | Should -Match 'GALION_PIPER_LEGACY=1'
        $text | Should -Match 'GALION_SILERO_VAD=G:\\galion\\samantha\\weights\\vad\\silero_vad\.onnx'
        $text | Should -Match 'V1_SCREEN_ADDR=127\.0\.0\.1:7878'
        $text | Should -Match 'GALION_SAMANTHA_WS=127\.0\.0\.1:7881'
        $text | Should -Not -Match 'https://'
        $text | Should -Not -Match 'GALION_LLM_KEY'
        $text | Should -Not -Match 'GALION_LLM_URL'
        $text | Should -Not -Match 'GALION_CLONE_FETCH'
        { Format-SamanthaEnvLine -Key 'GALION_LLM_KEY' -Value 'not-used' } | Should -Throw '*secret-shaped*'
        { Format-SamanthaEnvLine -Key 'NOTE' -Value 'sk-abcdefghijklmnopqrstuvwxyz' } | Should -Throw '*secret-shaped*'
    }
}

Describe 'pins match the existing fetch scripts' {
    It 'reuses the Vosk model and Piper voice hashes' {
        $vosk = Get-Content -LiteralPath (Join-Path -Path $script:Repo -ChildPath 'scripts/asr/fetch-vosk.sh') -Raw
        $piper = Get-Content -LiteralPath (Join-Path -Path $script:Repo -ChildPath 'scripts/a21/fetch-a21.sh') -Raw
        $byId = @{}
        foreach ($pin in $script:Config.Downloads) {
            $byId[$pin.Id] = $pin
        }
        $vosk | Should -Match ([regex]::Escape($byId['vosk-model-small-en-us-0.15'].Sha256))
        $vosk | Should -Match ([regex]::Escape($byId['vosk-model-small-en-us-0.15'].Url))
        $piper | Should -Match ([regex]::Escape($byId['piper-voice-onnx'].Sha256))
        $piper | Should -Match ([regex]::Escape($byId['piper-voice-json'].Sha256))
        # fetch-a21.sh joins a base URL with the file name. The pin stores the joined URL.
        $piperBase = 'https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ljspeech/medium'
        $piper | Should -Match ([regex]::Escape($piperBase))
        $piper | Should -Match 'en_US-ljspeech-medium\.onnx'
        $byId['piper-voice-onnx'].Url | Should -Be ($piperBase + '/en_US-ljspeech-medium.onnx')
        $byId['piper-voice-json'].Url | Should -Be ($piperBase + '/en_US-ljspeech-medium.onnx.json')
        $byId['piper-windows-amd64-2023.11.14-2'].Sha256 | Should -Not -Be 'a50cb45f355b7af1f6d758c1b360717877ba0a398cc8cbe6d2a7a3a26e225992'
        $byId['libvosk-win64-0.3.45'].Sha256 | Should -Not -Be 'bbdc8ed85c43979f6443142889770ea95cbfbc56cffb5c5dcd73afa875c5fbb2'
    }

    It 'only downloads from the pinned public hosts' {
        foreach ($pin in $script:Config.Downloads) {
            $uri = [Uri]$pin.Url
            $script:Config.AllowedUrlHosts | Should -Contain $uri.Host
            $pin.Sha256 | Should -Match '^[0-9a-f]{64}$'
        }
    }

    It 'checks downloaded files in /tmp/s14-pins when this machine has them' {
        $known = @{
            'silero_vad.onnx' = 'silero-vad-v6.2.3'
            'piper_windows_amd64.zip' = 'piper-windows-amd64-2023.11.14-2'
            'vosk-win64-0.3.45.zip' = 'libvosk-win64-0.3.45'
            'en_US-ljspeech-medium.onnx.json' = 'piper-voice-json'
        }
        $byId = @{}
        foreach ($pin in $script:Config.Downloads) {
            $byId[$pin.Id] = $pin.Sha256
        }
        $dir = Join-Path -Path ([System.IO.Path]::GetTempPath()) -ChildPath 's14-pins'
        if (-not (Test-Path -LiteralPath $dir)) {
            Set-ItResult -Skipped -Because 'the hashed downloads are not in the temp dir on this host'
            return
        }
        foreach ($name in $known.Keys) {
            $path = Join-Path -Path $dir -ChildPath $name
            if (-not (Test-Path -LiteralPath $path)) {
                Set-ItResult -Skipped -Because "$name is not in the temp dir"
                return
            }
            $got = Get-SamanthaFileSha256 -Path $path
            $got | Should -Be $byId[$known[$name]]
        }
    }
}

Describe 'dry-run' {
    It 'prints the plan and does not touch the network' {
        $script:SamanthaNetworkCalls = 0
        $out = @(Invoke-SamanthaInstall -CommonRoot $script:Scripts -PlanOnly -RepoRoot $script:Repo)
        $text = $out -join "`n"
        $script:SamanthaNetworkCalls | Should -Be 0
        $script:SamanthaExitCode | Should -Be 0
        $text | Should -Match 'G:\\galion\\samantha'
        $text | Should -Match '127\.0\.0\.1:7878'
        $text | Should -Match '127\.0\.0\.1:7881/health'
        $text | Should -Match 'galion-samantha-ws/1'
        $text | Should -Match 'dry-run: no download, no build, no write'
        $text | Should -Match 'GalionLaptopLink'
        $text | Should -Match 'GalionShellLink'
        $text | Should -Match 'not built on Windows'
        $text | Should -Match 'chatterbox_worker.py is not started'
        $text | Should -Match 'FRAME_MS is 80'
        foreach ($pin in $script:Config.Downloads) {
            $text | Should -Match ([regex]::Escape($pin.Sha256))
            $text | Should -Match ([regex]::Escape($pin.Url))
        }
        $cargo = @($out | Where-Object { $_ -like 'dry-run: cargo *' })
        $cargo.Count | Should -Be 2
        ($cargo -join "`n") | Should -Match 'galion-console'
        ($cargo -join "`n") | Should -Match 'galion-asr-vosk'
        ($cargo -join "`n") | Should -Match '--locked'
        ($cargo -join "`n") | Should -Not -Match 'galion-piper-rt'
        ($cargo -join "`n") | Should -Not -Match '--offline'
        $text | Should -Not -Match 'GALION_CLONE_FETCH=1'
    }

    It 'prints the start command and does not launch a process' {
        $before = @(Get-Process -Name dot_console_demo -ErrorAction SilentlyContinue).Count
        $out = @(Invoke-SamanthaStart -PlanOnly)
        $text = $out -join "`n"
        $text | Should -Match 'dry-run: start G:\\galion\\samantha\\bin\\dot_console_demo.exe --headless --realtime --no-png --web 127\.0\.0\.1:7878 --seconds 3600'
        $text | Should -Match 'dry-run: start G:\\galion\\samantha\\bin\\samantha-local.exe --addr 127\.0\.0\.1:7881'
        $text | Should -Match 'dry-run: GET http://127\.0\.0\.1:7881/health  \(galion-samantha-ws/1, 200 and ok\)'
        $text | Should -Match 'dry-run: open http://127\.0\.0\.1:7878/'
        $text | Should -Match 'no process started'
        $script:SamanthaExitCode | Should -Be 0
        $after = @(Get-Process -Name dot_console_demo -ErrorAction SilentlyContinue).Count
        $after | Should -Be $before
    }
}

Describe 'source guards' {
    It 'does not name scheduler, service, or driver commands in the scripts' {
        $files = @(
            (Join-Path -Path $script:Scripts -ChildPath 'Samantha.Common.ps1')
            (Join-Path -Path $script:Scripts -ChildPath 'Install-Samantha.ps1')
            (Join-Path -Path $script:Scripts -ChildPath 'Start-Samantha.ps1')
            (Join-Path -Path $script:Sector -ChildPath 'START-SAMANTHA.cmd')
        )
        $banned = @(
            '\bschtasks\b'
            '\bStop-ScheduledTask\b'
            '\bUnregister-ScheduledTask\b'
            '\bRegister-ScheduledTask\b'
            '\bDisable-ScheduledTask\b'
            '\bEnable-ScheduledTask\b'
            '\bSet-ScheduledTask\b'
            '\bStart-ScheduledTask\b'
            '\bStop-Service\b'
            '\bSet-Service\b'
            '\bRestart-Service\b'
            '\bpnputil\b'
            '\bbcdedit\b'
            '\bsetx\b'
            'SetEnvironmentVariable'
            '\bsshd\b'
            '\btaskkill\b'
            '\bnvidia-smi\b'
            '\bsc\.exe\b'
            '\breg\.exe\b'
            '\bInvoke-Expression\b'
        )
        foreach ($file in $files) {
            $raw = Get-Content -LiteralPath $file -Raw
            $raw = [regex]::Replace($raw, '(?s)<#.*?#>', '')
            $kept = New-Object System.Collections.Generic.List[string]
            foreach ($line in ($raw -split "`n")) {
                $trim = $line.Trim()
                if ($trim.StartsWith('#') -or $trim.StartsWith('REM ') -or $trim.StartsWith('::')) {
                    continue
                }
                $hash = $line.IndexOf('#')
                if ($hash -ge 0) {
                    $line = $line.Substring(0, $hash)
                }
                [void]$kept.Add($line)
            }
            $code = $kept -join "`n"
            foreach ($pattern in $banned) {
                $code | Should -Not -Match $pattern
            }
        }
    }

    It 'stops only the server process this script started' {
        $start = Get-Content -LiteralPath (Join-Path -Path $script:Scripts -ChildPath 'Samantha.Common.ps1') -Raw
        $hits = [regex]::Matches($start, 'Stop-Process[^\r\n]*')
        $hits.Count | Should -Be 2
        foreach ($hit in $hits) {
            $hit.Value | Should -Match '-Id \$'
            $hit.Value | Should -Not -Match '-Name'
        }
    }

    It 'keeps https URLs in the pin file rather than the scripts' {
        $scripts = Get-ChildItem -Path $script:Scripts -Filter '*.ps1'
        foreach ($file in $scripts) {
            $text = Get-Content -LiteralPath $file.FullName -Raw
            $text | Should -Not -Match 'https://'
        }
    }

    It 'does not add this folder to the root Cargo workspace' {
        $cargo = Get-Content -LiteralPath (Join-Path -Path $script:Repo -ChildPath 'Cargo.toml') -Raw
        $cargo | Should -Not -Match 'voice-stack'
    }
}

Describe 'tree hygiene' {
    It 'copies the repo LICENSE byte for byte' {
        $repoLicense = [System.IO.File]::ReadAllBytes((Join-Path -Path $script:Repo -ChildPath 'LICENSE'))
        $sectorLicense = [System.IO.File]::ReadAllBytes((Join-Path -Path $script:Sector -ChildPath 'LICENSE'))
        $sectorLicense.Length | Should -Be $repoLicense.Length
        $same = $true
        for ($i = 0; $i -lt $repoLicense.Length; $i++) {
            if ($repoLicense[$i] -ne $sectorLicense[$i]) {
                $same = $false
                break
            }
        }
        $same | Should -BeTrue
    }

    It 'puts an SPDX line on every new file except the copied LICENSE' {
        $files = Get-ChildItem -Path $script:Sector -Recurse -File
        foreach ($file in $files) {
            if ($file.Name -eq 'LICENSE') {
                continue
            }
            $text = Get-Content -LiteralPath $file.FullName -Raw
            $text | Should -Match 'SPDX-License-Identifier: MIT'
            $text | Should -Match 'Copyright \(c\) 2026 Galion Labs'
        }
    }

    It 'does not commit audio, models, or binaries' {
        $banned = @('*.wav', '*.wave', '*.mp3', '*.flac', '*.ogg', '*.opus', '*.onnx', '*.zip', '*.exe', '*.dll')
        foreach ($pattern in $banned) {
            $found = @(Get-ChildItem -Path $script:Sector -Recurse -File -Filter $pattern)
            $found.Count | Should -Be 0
        }
    }
}

Describe 'script entry points' {
    It 'dry-run install exits 0' {
        $pwsh = (Get-Command -Name pwsh -ErrorAction SilentlyContinue)
        if ($null -eq $pwsh) {
            $pwsh = Get-Command -Name powershell
        }
        $scriptPath = Join-Path -Path $script:Scripts -ChildPath 'Install-Samantha.ps1'
        $outFile = Join-Path -Path ([System.IO.Path]::GetTempPath()) -ChildPath 's14-install-out.txt'
        $errFile = Join-Path -Path ([System.IO.Path]::GetTempPath()) -ChildPath 's14-install-err.txt'
        $proc = Start-Process -FilePath $pwsh.Source -ArgumentList @('-NoProfile', '-File', $scriptPath, '-DryRun') -Wait -PassThru -RedirectStandardOutput $outFile -RedirectStandardError $errFile
        $proc.ExitCode | Should -Be 0
        $text = Get-Content -LiteralPath $outFile -Raw
        $text | Should -Match 'dry-run: no download, no build, no write'
    }

    It 'dry-run start exits 0' {
        $pwsh = (Get-Command -Name pwsh -ErrorAction SilentlyContinue)
        if ($null -eq $pwsh) {
            $pwsh = Get-Command -Name powershell
        }
        $scriptPath = Join-Path -Path $script:Scripts -ChildPath 'Start-Samantha.ps1'
        $outFile = Join-Path -Path ([System.IO.Path]::GetTempPath()) -ChildPath 's14-start-out.txt'
        $errFile = Join-Path -Path ([System.IO.Path]::GetTempPath()) -ChildPath 's14-start-err.txt'
        $proc = Start-Process -FilePath $pwsh.Source -ArgumentList @('-NoProfile', '-File', $scriptPath, '-DryRun') -Wait -PassThru -RedirectStandardOutput $outFile -RedirectStandardError $errFile
        $proc.ExitCode | Should -Be 0
        $text = Get-Content -LiteralPath $outFile -Raw
        $text | Should -Match 'no process started'
    }
}
