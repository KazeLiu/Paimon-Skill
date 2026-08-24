@echo off
setlocal

set "APP_DIR=%~dp0"
set "APP_EXE=%APP_DIR%src-tauri\target\release\paimon-desktop.exe"

cd /d "%APP_DIR%"
if not exist "%APP_DIR%package.json" (
  echo [ERROR] package.json not found: %APP_DIR%
  pause
  exit /b 1
)

echo Building the latest desktop app...
call npm run tauri build
if errorlevel 1 (
  echo.
  echo [ERROR] Build failed. Please keep this window open and check the output above.
  pause
  exit /b 1
)

if not exist "%APP_EXE%" (
  echo [ERROR] Executable not found: %APP_EXE%
  pause
  exit /b 1
)

echo Starting Paimon Desktop...
start "Paimon Desktop" "%APP_EXE%"
exit /b 0
