@echo off
REM SPDX-License-Identifier: MIT
REM Copyright (c) 2026 Galion Labs
REM Start the local Samantha server and open http://127.0.0.1:7878/
REM Does not change scheduled tasks, services, drivers, or Windows system files.
setlocal
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\Start-Samantha.ps1" %*
exit /b %ERRORLEVEL%
