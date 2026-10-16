# App-only lifecycle test: use the real Slint binary and validators, without
# requiring a virtual printer on the hosted runner's noninteractive desktop.
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
Set-Location $workspace
./scripts/build-windows-installer.ps1 -SkipChecks -SkipBuild -AppOnly
if ($LASTEXITCODE -ne 0) {throw 'Testinstaller-Build fehlgeschlagen.'}
$setup = Get-ChildItem artifacts/windows/*-AppOnly-Setup.exe | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
$installDirectory = Join-Path $env:TEMP 'Druckwandel-Installer-Test'
$process = Start-Process -FilePath $setup.FullName -ArgumentList @('/S',"/D=$installDirectory") -PassThru -Wait
if ($process.ExitCode -ne 0) {throw "Installation fehlgeschlagen ($($process.ExitCode))."}
# Exercise the update path as well; the old uninstaller runs with /UPDATE.
$process = Start-Process -FilePath $setup.FullName -ArgumentList @('/S',"/D=$installDirectory") -PassThru -Wait
if ($process.ExitCode -ne 0) {throw "Aktualisierung fehlgeschlagen ($($process.ExitCode))."}
$key = 'HKCU:\Software\Classes\erechnung-review\shell\open\command'
$expected = '"' + (Join-Path $installDirectory 'Druckwandel.exe') + '" "%1"'
if ((Get-Item $key).GetValue('') -ne $expected) {throw 'Falscher Protokollhandler.'}
if (-not (Test-Path (Join-Path $installDirectory 'Druckwandel.exe'))) {throw 'Slint-EXE fehlt.'}
$process = Start-Process -FilePath (Join-Path $installDirectory 'uninstall.exe') -ArgumentList @('/S',"_?=$installDirectory") -PassThru -Wait
if ($process.ExitCode -ne 0) {throw "Deinstallation fehlgeschlagen ($($process.ExitCode))."}
if (Test-Path $key) {throw 'Protokollhandler wurde nicht entfernt.'}
if (Test-Path (Join-Path $installDirectory 'Druckwandel.exe')) {throw 'Slint-EXE wurde nicht entfernt.'}
Remove-Item artifacts/windows/*-AppOnly-Setup.exe* -Force
Write-Output 'Installation, Protokollzuordnung und Deinstallation bestanden.'
