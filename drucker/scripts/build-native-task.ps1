[CmdletBinding()]
param(
    [ValidateSet("x64", "ARM64")][string]$Platform = "x64",
    [ValidateSet("Debug", "Release")][string]$Configuration = "Release"
)
$ErrorActionPreference = "Stop"
$workspace = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$runtime = if ($Platform -eq "ARM64") { "win-arm64" } else { "win-x64" }
$output = Join-Path $workspace "artifacts\printer-native\$runtime"
& dotnet publish (Join-Path $workspace "drucker\src\VirtualPrinter.Tasks\VirtualPrinter.Tasks.csproj") `
    -c $Configuration -r $runtime -p:Platform=$Platform `
    -p:PublishAot=true -p:NativeLib=Shared -o $output
if ($LASTEXITCODE -ne 0) { throw "Der native Drucker-Background-Task konnte nicht gebaut werden." }
$winmd = Join-Path $workspace "drucker\src\VirtualPrinter.Tasks\bin\$Platform\$Configuration\net10.0-windows10.0.26100.0\$runtime\ERechnung.VirtualPrinter.Tasks.winmd"
Copy-Item -LiteralPath $winmd -Destination $output -Force
