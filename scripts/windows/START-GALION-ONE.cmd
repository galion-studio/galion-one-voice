@echo off
setlocal
if not exist G:\ (
  echo G: is not available. Galion on this laptop lives under G:\galion only.
  exit /b 1
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0Start-GalionOne.ps1" %*
exit /b %ERRORLEVEL%
