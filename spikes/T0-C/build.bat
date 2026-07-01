@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"

set "SDK_PATH=C:\Program Files (x86)\Windows Kits\10"
set "SDK_VER=10.0.22621.0"
set "OUTPUT_DIR=%~dp0build"

if not exist "%OUTPUT_DIR%" mkdir "%OUTPUT_DIR%"

echo Compiling GoProCamFilterImpl.cpp...
cl.exe /EHsc /W3 /MP /Gy /Gm- /GS- /GR- /fp:fast /D "_UNICODE" /D "UNICODE" /D "WIN32" /D "_WIN32_WINNT=0x0A00" /D "_DEBUG" /D "DEBUG" /D "_WINDOWS" /D "_USRDLL" /D "GOProCamFilter_EXPORTS" /I "%SDK_PATH%\include\%SDK_VER%\ucrt" /I "%SDK_PATH%\include\%SDK_VER%\um" /I "%SDK_PATH%\include\%SDK_VER%\shared" /c "%~dp0src\GoProCamFilterImpl.cpp" /Fo"%OUTPUT_DIR%\GoProCamFilterImpl.obj"

if errorlevel 1 goto :error

echo Compiling GoProCamPin.cpp...
cl.exe /EHsc /W3 /MP /Gy /Gm- /GS- /GR- /fp:fast /D "_UNICODE" /D "UNICODE" /D "WIN32" /D "_WIN32_WINNT=0x0A00" /D "_DEBUG" /D "DEBUG" /D "_WINDOWS" /D "_USRDLL" /D "GOProCamFilter_EXPORTS" /I "%SDK_PATH%\include\%SDK_VER%\ucrt" /I "%SDK_PATH%\include\%SDK_VER%\um" /I "%SDK_PATH%\include\%SDK_VER%\shared" /c "%~dp0src\GoProCamPin.cpp" /Fo"%OUTPUT_DIR%\GoProCamPin.obj"

if errorlevel 1 goto :error

echo Linking...
link.exe /DLL /NOLOGO /OUT:"%OUTPUT_DIR%\GoProCamFilter.dll" /DEF:"%~dp0src\GoProCamFilter.def" /LIBPATH:"%SDK_PATH%\lib\%SDK_VER%\um\x64" kernel32.lib user32.lib advapi32.lib ole32.lib oleaut32.lib strmiids.lib uuid.lib

if errorlevel 1 goto :error

echo.
echo Build successful!
echo Output: %OUTPUT_DIR%\GoProCamFilter.dll
echo.
goto :done

:error
echo.
echo BUILD FAILED!
exit /b 1

:done