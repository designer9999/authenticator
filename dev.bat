@echo off
title Authenticator - Dev Mode
cd /d "%~dp0"
echo.
echo  ========================================
echo   Authenticator - Tauri + Svelte 5
echo  ========================================
echo.
echo  Starting dev server...
echo  (Press Ctrl+C to stop)
echo.
npm run tauri dev
pause
