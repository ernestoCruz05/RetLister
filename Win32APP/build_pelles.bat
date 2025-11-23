@echo off
REM Build with Pelles C for Windows XP

REM Ensure working directory is the script's directory
cd /d %~dp0

set INCLUDE=C:\Program Files\PellesC\Include\Win;C:\Program Files\PellesC\Include
set LIB=C:\Program Files\PellesC\Lib\Win;C:\Program Files\PellesC\Lib

"C:\Program Files\PellesC\bin\pocc.exe" /Tx86-coff /Ze /Ot /Ob1 /W1 /Gd /D_WIN32_WINNT=0x0501 RetLister.c

REM Link explicitly for XP (subsystem 5.01) and Pelles runtime (pcrt.lib)
"C:\Program Files\PellesC\bin\polink.exe" /SUBSYSTEM:WINDOWS,5.01 /MACHINE:X86 /OUT:RetLister.exe ^
  RetLister.obj ^
  kernel32.lib user32.lib gdi32.lib comctl32.lib comdlg32.lib shell32.lib advapi32.lib wininet.lib ole32.lib oleaut32.lib uuid.lib ^
  crt.lib

if %ERRORLEVEL% EQU 0 (
    echo Build successful: RetLister.exe (Pelles C - XP targeted)
    del RetLister.obj
) else (
    echo Build failed
)

pause
