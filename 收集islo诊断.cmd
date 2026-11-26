@echo off
rem Portable package: the diagnostics script is embedded in islo.exe.
start "" "%~dp0islo.exe" --collect-diagnostics
