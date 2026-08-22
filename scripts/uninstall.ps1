[CmdletBinding()]
param(
    [int]$QueueTimeoutSeconds = 30
)

$ErrorActionPreference = "Stop"
$packages = Get-AppxPackage -Name "ERechnung.VirtualPrinter.PoC"

if (-not $packages) {
    Write-Host "E-Rechnung Virtual Printer is not installed."
    return
}

$packages | ForEach-Object { Remove-AppxPackage -Package $_.PackageFullName }

$deadline = [DateTimeOffset]::UtcNow.AddSeconds($QueueTimeoutSeconds)
do {
    $printer = Get-Printer -Name "E-Rechnung" -ErrorAction SilentlyContinue
    if (-not $printer) {
        Write-Host "Package removed and printer queue disappeared."
        return
    }

    Start-Sleep -Seconds 1
} while ([DateTimeOffset]::UtcNow -lt $deadline)

throw "Das Paket wurde entfernt, die Queue 'E-Rechnung' ist nach $QueueTimeoutSeconds Sekunden aber noch vorhanden."
