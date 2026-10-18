@echo off
setlocal
cd /d "%~dp0"
echo ===================================================
echo     Paseo Desktop Auto-Pull and .EXE Builder
echo ===================================================
node scripts/update-and-build-exe.mjs
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [ERROR] Build failed! Check the logs above.
    pause
    exit /b %ERRORLEVEL%
)
echo.
echo [DONE] Build completed successfully!
pause
