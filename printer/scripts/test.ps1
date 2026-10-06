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
    & dotnet build `
        (Join-Path $repositoryRoot "tests\PrintSmokeSender\PrintSmokeSender.csproj") `
        --configuration Release
    if ($LASTEXITCODE -ne 0) {
        throw "Interactive print smoke sender build failed."
    }

    Get-AppxPackage -Name "ERechnung.VirtualPrinter.PoC" | Format-Table Name, Version, Status
    Get-Printer -Name "E-Rechnung" -ErrorAction SilentlyContinue | Format-Table Name, DriverName, PortName
}
