[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path $PSScriptRoot -Parent

& dotnet test (Join-Path $repositoryRoot "tests\PrintCore.Tests\PrintCore.Tests.csproj") --configuration Release
if ($LASTEXITCODE -ne 0) {
    throw "Core tests failed."
}

if ($IsWindows -or $PSVersionTable.PSEdition -eq "Desktop") {
    & (Join-Path $PSScriptRoot "check-environment.ps1")
    Get-AppxPackage -Name "ERechnung.VirtualPrinter.PoC" | Format-Table Name, Version, Status
    Get-Printer -Name "E-Rechnung" -ErrorAction SilentlyContinue | Format-Table Name, DriverName, PortName
}
