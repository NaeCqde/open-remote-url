@echo off
echo Installing Open Remote URL...
for %%f in ("%~dp0open-remote-url-*.exe") do (
    echo Installing %%~nxf...
    "%%f" --install
)
pause
