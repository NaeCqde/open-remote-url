@echo off
echo Opening Open Remote URL config...
for %%f in ("%~dp0open-remote-url-*.exe") do (
    echo Config for %%~nxf:
    "%%f" --config
)
pause
