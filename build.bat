@echo off
REM Build helper for RustBlox. Pass "debug" for a debug build, anything else
REM builds release. The exe is left in target\<profile>\rustblox.exe

setlocal
set PROFILE=release
if /I "%~1"=="debug" set PROFILE=debug

where cargo >nul 2>nul
if errorlevel 1 (
    if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
        set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
    ) else (
        echo cargo was not found. Install Rust from https://rustup.rs and try again.
        exit /b 1
    )
)

echo Building RustBlox [%PROFILE%]

if "%PROFILE%"=="debug" (
    cargo build
) else (
    cargo build --release
)
if errorlevel 1 exit /b 1

echo.
echo Running tests
cargo test --quiet
if errorlevel 1 exit /b 1

echo.
echo Done: target\%PROFILE%\rustblox.exe
endlocal
