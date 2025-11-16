@echo off
REM Build RetLister Win32 Application for Windows XP (32-bit)

REM Setup VS environment for x86
call "C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars32.bat"

REM Use old platform toolset libraries
set LIB=C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x86;C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\ucrt\x86;%LIB%

cl.exe /TC /O2 /W3 /MD /D_CRT_SECURE_NO_WARNINGS /D_WIN32_WINNT=0x0501 RetLister.c /Fe:RetLister.exe /link /SUBSYSTEM:WINDOWS,5.01 /ENTRY:WinMainCRTStartup user32.lib gdi32.lib comctl32.lib wininet.lib ucrt.lib vcruntime.lib

if %ERRORLEVEL% EQU 0 (
    echo Build successful: RetLister.exe
    echo NOTE: Copy these DLLs to XP: ucrtbase.dll, vcruntime140.dll
) else (
    echo Build failed
)

pause
