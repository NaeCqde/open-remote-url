@echo off
echo Uninstalling Open Remote URL...
for %%f in ("%~dp0open-remote-url-*.exe") do (
    echo Uninstalling %%~nxf...
    "%%f" --uninstall
)
pause
