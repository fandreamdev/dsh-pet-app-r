@echo off
rem dsh-pet-rust - one-click release run.
rem Usage: double-click, or pass "rebuild" to force rebuild then run.
setlocal EnableExtensions
cd /d "%~dp0"

set "DIST=dist-win\dsh-pet-rust.exe"

if /i "%~1"=="rebuild" goto rebuild
if exist "%DIST%" goto run

:rebuild
echo [dsh-pet-rust] Building release (first run / after code changes)...
call cargo build --release
if errorlevel 1 (
  echo [dsh-pet-rust] Build failed. See errors above.
  pause
  exit /b 1
)
where node >nul 2>nul
if errorlevel 1 (
  echo [dsh-pet-rust] Node.js not found; cannot stage assets ^(scripts\stage-dist.mjs^).
  pause
  exit /b 1
)
echo [dsh-pet-rust] Staging self-contained dist-win ^(exe + assets^)...
call node scripts\stage-dist.mjs
if errorlevel 1 (
  echo [dsh-pet-rust] Staging failed.
  pause
  exit /b 1
)

:run
echo [dsh-pet-rust] Launching...
start "" "%DIST%"
endlocal
