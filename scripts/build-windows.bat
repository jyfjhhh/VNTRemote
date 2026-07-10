@echo off
REM VNTRemote Windows Build Script
REM Requires: Rust, Node.js, Git Bash or PowerShell

echo ========================================
echo   VNTRemote - Windows Build
echo ========================================

REM Check prerequisites
where rustc >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Rust not found! Install from https://rustup.rs
    exit /b 1
)

where node >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Node.js not found! Install from https://nodejs.org
    exit /b 1
)

echo [OK] Rust: 
rustc --version
echo [OK] Node: 
node --version

REM Install frontend deps
echo.
echo [1/3] Installing frontend dependencies...
cd frontend
call npm install
if %ERRORLEVEL% neq 0 (
    echo [ERROR] npm install failed
    exit /b 1
)
cd ..

REM Build
echo.
echo [2/3] Building VNTRemote...
cargo tauri build --bundles msi
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Build failed
    exit /b 1
)

echo.
echo [3/3] Done!
echo.
echo Output: src-tauri\target\release\bundle\
echo   - vnt-remote.exe  (Portable)
echo   - VNTRemote_*.msi (Installer)
echo.
pause
