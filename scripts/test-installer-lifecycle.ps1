[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$root = Join-Path $env:TEMP ("erechnung-installer-lifecycle-" + [guid]::NewGuid().ToString("n"))
$guard = Join-Path (Split-Path -Parent $PSScriptRoot) "apps\desktop\src-tauri\installer\windows\UpdateGuard.ps1"
New-Item -ItemType Directory -Force -Path @(
    "$root\documents\E-Rechnungsarchiv",
    "$root\documents\E-Rechnung Entwürfe",
    "$root\local\workspace",
    "$root\roaming"
) | Out-Null
Set-Content -LiteralPath "$root\documents\E-Rechnungsarchiv\archiv.sqlite3" -Value "archive" -Encoding ASCII
Set-Content -LiteralPath "$root\local\workspace\workspace.sqlite3" -Value "workspace" -Encoding ASCII
Set-Content -LiteralPath "$root\roaming\correction-memory.json" -Value "{}" -Encoding ASCII
Set-Content -LiteralPath "$root\local\installed-version.txt" -Value "0.3.1" -Encoding ASCII

& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $guard -Action PrepareUpdate -IncomingVersion "0.2.2" -IsolatedRoot $root
if ($LASTEXITCODE -eq 0) { throw "Abwärtsinstallation wurde nicht blockiert." }

& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $guard -Action PrepareUpdate -IncomingVersion "0.4.0" -IsolatedRoot $root
if ($LASTEXITCODE -ne 0) { throw "Aktualisierungssicherung ist fehlgeschlagen." }
$recorded = Get-Content -LiteralPath "$root\local\installed-version.txt" -TotalCount 1
if ($recorded -ne "0.3.1") {
    throw "Die Versionsmarke darf erst nach erfolgreicher Installation wechseln."
}
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $guard -Action RecordInstalledVersion -IncomingVersion "0.4.0" -IsolatedRoot $root
if ($LASTEXITCODE -ne 0) { throw "Die Versionsmarke nach der Installation fehlt." }
$recorded = Get-Content -LiteralPath "$root\local\installed-version.txt" -TotalCount 1
if ($recorded -ne "0.4.0") { throw "Die installierte Version wurde nicht festgehalten." }
$backup = Get-ChildItem -LiteralPath "$root\local\update-backup" -Directory | Select-Object -First 1
if (-not $backup) { throw "Es entstand kein Update-Snapshot." }
if (-not (Test-Path -LiteralPath (Join-Path $backup.FullName "workspace.sqlite3"))) {
    throw "Der Entwurfssnapshot fehlt."
}
if (-not (Test-Path -LiteralPath (Join-Path $backup.FullName "WIEDERHERSTELLUNG.txt"))) {
    throw "Der Wiederherstellungshinweis fehlt."
}

# Simulate application uninstall that must not touch documents or workspace originals.
Remove-Item -LiteralPath "$root\local\installed-version.txt" -ErrorAction SilentlyContinue
& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $guard -Action AssertUserDataKept -IsolatedRoot $root
if ($LASTEXITCODE -ne 0) { throw "Die Prüfung der erhaltenen Nutzerdaten ist fehlgeschlagen." }
if (-not (Test-Path -LiteralPath "$root\documents\E-Rechnungsarchiv\archiv.sqlite3")) {
    throw "Das Archiv darf bei der Deinstallation nicht gelöscht werden."
}

Remove-Item -LiteralPath $root -Recurse -Force
Write-Output "Isolierte Installer-Szenarien bestanden."
