# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Galion Labs
#
# Public model pins for the Windows laptop install.
# Sha256 values are lowercase hex. File pins are checked again after download.
# Zip pins are checked before unpack, then the zip is removed.
@{
    Home = 'G:\galion\samantha'
    ListenHost = '127.0.0.1'
    # One-dot HTTP screen. scripts/run-v1-screen.sh. Not the companion socket.
    ScreenPort = 7878
    # galion-samantha-ws/1. PR #220. GET /health on this port.
    CompanionPort = 7881
    CompanionSchema = 'galion-samantha-ws/1'
    RustMin = '1.85.0'
    RustHost = 'x86_64-pc-windows-msvc'
    HeartbeatMs = 80
    Persona = 'samantha'
    PiperVoice = 'en_US-ljspeech-medium'
    # Names only. The installer never calls the task scheduler.
    ProtectedScheduledTasks = @(
        'GalionLaptopLink'
        'GalionShellLink'
    )
    AllowedUrlHosts = @(
        'alphacephei.com'
        'github.com'
        'huggingface.co'
    )
    Downloads = @(
        @{
            Id = 'vosk-model-small-en-us-0.15'
            Kind = 'zip'
            Url = 'https://alphacephei.com/vosk/models/vosk-model-small-en-us-0.15.zip'
            Sha256 = '30f26242c4eb449f948e42cb302dd7a686cb29a3423a8367f99ff41780942498'
            FileName = 'vosk-model-small-en-us-0.15.zip'
            DestDir = 'weights\asr'
            ReadyFile = 'weights\asr\vosk-model-small-en-us-0.15\am\final.mdl'
            AlsoReady = 'weights\asr\vosk-model-small-en-us-0.15\conf\model.conf'
            Unpack = $true
            UnpackFolder = 'vosk-model-small-en-us-0.15'
            PlaceAs = ''
            WriteShaSidecar = $false
            License = 'Apache-2.0'
            Note = 'Same URL and sha256 as scripts/asr/fetch-vosk.sh. The model zip is not tied to one OS.'
        }
        @{
            Id = 'libvosk-win64-0.3.45'
            Kind = 'zip'
            Url = 'https://github.com/alphacep/vosk-api/releases/download/v0.3.45/vosk-win64-0.3.45.zip'
            Sha256 = 'f1dcc9cca460630f81ea8f71794f69c80bed6556d2a4e6237b5785e1d2dff34b'
            FileName = 'vosk-win64-0.3.45.zip'
            DestDir = 'weights\asr'
            ReadyFile = 'weights\asr\libvosk\libvosk.dll'
            AlsoReady = 'weights\asr\libvosk\libgcc_s_seh-1.dll'
            Unpack = $true
            UnpackFolder = 'vosk-win64-0.3.45'
            PlaceAs = 'weights\asr\libvosk'
            WriteShaSidecar = $false
            License = 'Apache-2.0'
            Note = 'Windows x64 libvosk 0.3.45. sha256 computed 2026-11-21 from the release zip. The Linux pin in scripts/asr/fetch-vosk.sh is a different archive.'
        }
        @{
            Id = 'silero-vad-v6.2.3'
            Kind = 'file'
            Url = 'https://github.com/snakers4/silero-vad/raw/5cd7945676eb32225748052e2e6a0580e4686a08/src/silero_vad/data/silero_vad.onnx'
            Sha256 = '1a153a22f4509e292a94e67d6f9b85e8deb25b4988682b7e174c65279d8788e3'
            FileName = 'silero_vad.onnx'
            DestDir = 'weights\vad'
            ReadyFile = 'weights\vad\silero_vad.onnx'
            AlsoReady = ''
            Unpack = $false
            UnpackFolder = ''
            PlaceAs = ''
            WriteShaSidecar = $true
            License = 'MIT'
            Note = 'Silero VAD v6.2.3 ONNX at commit 5cd7945676eb32225748052e2e6a0580e4686a08. sha256 computed 2026-11-21. The LICENSE file at that commit is MIT. CPU file only. Not linked into the Rust workspace.'
        }
        @{
            Id = 'piper-voice-onnx'
            Kind = 'file'
            Url = 'https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ljspeech/medium/en_US-ljspeech-medium.onnx'
            Sha256 = '6f52a751e2349abe7a76735eb09dc1875298c77ea2342ffd2fef79ff81b87f22'
            FileName = 'en_US-ljspeech-medium.onnx'
            DestDir = 'weights\tts'
            ReadyFile = 'weights\tts\en_US-ljspeech-medium.onnx'
            AlsoReady = ''
            Unpack = $false
            UnpackFolder = ''
            PlaceAs = ''
            WriteShaSidecar = $true
            License = 'MIT'
            Note = 'Same URL and sha256 as scripts/a21/fetch-a21.sh and the PINNED table in crates/galion-ml-voice. Stock Piper voice en_US-ljspeech-medium. Not a Samantha clone.'
        }
        @{
            Id = 'piper-voice-json'
            Kind = 'file'
            Url = 'https://huggingface.co/rhasspy/piper-voices/resolve/main/en/en_US/ljspeech/medium/en_US-ljspeech-medium.onnx.json'
            Sha256 = '141d612cc0a95ed7efc1ca936b845c2364967f2e9217c5dbfcf69fc4d6c65860'
            FileName = 'en_US-ljspeech-medium.onnx.json'
            DestDir = 'weights\tts'
            ReadyFile = 'weights\tts\en_US-ljspeech-medium.onnx.json'
            AlsoReady = ''
            Unpack = $false
            UnpackFolder = ''
            PlaceAs = ''
            WriteShaSidecar = $true
            License = 'MIT'
            Note = 'Piper voice config beside the onnx file. sha256 matches scripts/a21/fetch-a21.sh. The json bytes were hashed again on 2026-11-21 and matched this pin.'
        }
        @{
            Id = 'piper-windows-amd64-2023.11.14-2'
            Kind = 'zip'
            Url = 'https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip'
            Sha256 = 'f3c58906402b24f3a96d92145f58acba6d86c9b5db896d207f78dc80811efcea'
            FileName = 'piper_windows_amd64.zip'
            DestDir = 'weights\runtime'
            ReadyFile = 'weights\runtime\piper\piper.exe'
            AlsoReady = 'weights\runtime\piper\onnxruntime.dll'
            Unpack = $true
            UnpackFolder = 'piper'
            PlaceAs = ''
            WriteShaSidecar = $false
            License = 'MIT (piper and onnxruntime). espeak-ng data in the zip is GPL-3 and is used only for phonemes.'
            Note = 'Windows piper 2023.11.14-2 CLI. sha256 computed 2026-11-21 from the release zip. The Linux archive in scripts/a21/fetch-a21.sh is a different file. galion-piper-rt is not this zip.'
        }
    )
    Builds = @(
        @{
            Id = 'dot_console_demo'
            CargoArgs = @('build', '--release', '--locked', '-p', 'galion-console', '--features', 'dot-viz-web', '--example', 'dot_console_demo')
            OutputRel = 'target\release\examples\dot_console_demo.exe'
            InstallName = 'dot_console_demo.exe'
        }
        @{
            Id = 'galion-asr-vosk'
            CargoArgs = @('build', '--release', '--locked', '-p', 'galion-asr-vosk', '--bin', 'galion-asr-vosk')
            OutputRel = 'target\release\galion-asr-vosk.exe'
            InstallName = 'galion-asr-vosk.exe'
        }
    )
}
