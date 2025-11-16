@echo off
REM Build RetLister with MinGW for Windows XP using old runtime

gcc -O2 -static-libgcc -mwindows -D_WIN32_WINNT=0x0501 -D__MSVCRT_VERSION__=0x0700 RetLister.c -o RetLister.exe -lcomctl32 -lwininet -lws2_32 -lmsvcrt

if %ERRORLEVEL% EQU 0 (
    echo Build successful: RetLister.exe (MinGW - Windows XP MSVCRT)
) else (
    echo Build failed
)

pause
