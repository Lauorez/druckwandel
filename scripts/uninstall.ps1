[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$packages = Get-AppxPackage -Name "ERechnung.VirtualPrinter.PoC"

if (-not $packages) {
    Write-Host "E-Rechnung Virtual Printer is not installed."
    return
}

$packages | ForEach-Object { Remove-AppxPackage -Package $_.PackageFullName }
Write-Host "Package removed. Windows removes the associated printer queue with the package."
