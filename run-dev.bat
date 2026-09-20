@echo off
setlocal
cd /d "%~dp0src-tauri"
cargo tauri dev
if errorlevel 1 (
  echo.
  echo Dev launcher failed.
  pause
)
