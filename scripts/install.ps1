[CmdletBinding()]
param(
    [int]$QueueTimeoutSeconds = 30
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path $PSScriptRoot -Parent

& (Join-Path $PSScriptRoot "install-dev-cert.ps1")

$package = Get-ChildItem -Path (Join-Path $repositoryRoot "artifacts\packages") -Recurse -File |
    Where-Object { $_.Extension -in ".msix", ".msixbundle" } |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1

if (-not $package) {
    throw "Kein MSIX-Paket gefunden. Führe zuerst .\scripts\build.ps1 aus."
}

Add-AppxPackage -Path $package.FullName -ForceApplicationShutdown
Write-Host "Installed: $($package.FullName)"

$deadline = [DateTimeOffset]::UtcNow.AddSeconds($QueueTimeoutSeconds)
do {
    $printer = Get-Printer -Name "E-Rechnung" -ErrorAction SilentlyContinue
    if ($printer) {
        break
    }

    Start-Sleep -Seconds 1
} while ([DateTimeOffset]::UtcNow -lt $deadline)

if (-not $printer) {
    throw "Paket wurde installiert, aber die Druckerwarteschlange erschien nicht innerhalb von $QueueTimeoutSeconds Sekunden. Prüfe Ereignisanzeige und docs/testing.md."
} else {
    Write-Host "Printer ready: $($printer.Name)"
}
