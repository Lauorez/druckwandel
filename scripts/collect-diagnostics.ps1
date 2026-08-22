[CmdletBinding()]
param(
    [string]$OutputRoot,
    [ValidateRange(1, 1440)]
    [int]$EventLookbackMinutes = 30,
    [switch]$IncludeJobMetadata
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path $PSScriptRoot -Parent

if (-not $IsWindows -and $PSVersionTable.PSEdition -eq "Core") {
    throw "Diagnosedaten können nur auf Windows erfasst werden."
}

if (-not $OutputRoot) {
    $timestamp = [DateTimeOffset]::Now.ToString("yyyyMMdd-HHmmss")
    $OutputRoot = Join-Path $repositoryRoot "artifacts\diagnostics\$timestamp"
}
New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null

$packages = @(Get-AppxPackage -Name "ERechnung.VirtualPrinter.PoC")
$printers = @(Get-Printer -Name "E-Rechnung" -ErrorAction SilentlyContinue)
$spooler = Get-Service -Name "Spooler"

$summary = [ordered]@{
    collectedAt      = [DateTimeOffset]::Now
    windowsVersion   = [Environment]::OSVersion.Version.ToString()
    powerShellVersion = $PSVersionTable.PSVersion.ToString()
    spoolerStatus    = $spooler.Status.ToString()
    packages         = @($packages | Select-Object Name, Version, Status, PackageFullName, PackageFamilyName)
    printers         = @($printers | Select-Object Name, DriverName, PortName, PrinterStatus, Type)
    jobMetadataIncluded = [bool]$IncludeJobMetadata
}
$summary | ConvertTo-Json -Depth 5 | Set-Content -Encoding utf8 -Path (Join-Path $OutputRoot "summary.json")

$startTime = (Get-Date).AddMinutes(-$EventLookbackMinutes)
$eventLogs = @(
    "Microsoft-Windows-PrintService/Admin",
    "Microsoft-Windows-PrintService/Operational",
    "Microsoft-Windows-AppModel-Runtime/Admin",
    "Microsoft-Windows-AppXDeploymentServer/Operational"
)

foreach ($eventLog in $eventLogs) {
    $safeName = $eventLog -replace "[^A-Za-z0-9.-]", "_"
    $outputPath = Join-Path $OutputRoot "$safeName.json"
    try {
        $events = @(Get-WinEvent `
            -FilterHashtable @{ LogName = $eventLog; StartTime = $startTime } `
            -ErrorAction Stop |
            Select-Object TimeCreated, Id, LevelDisplayName, ProviderName, Message)
        ConvertTo-Json -InputObject $events -Depth 4 |
            Set-Content -Encoding utf8 -Path $outputPath
    } catch {
        [ordered]@{ error = $_.Exception.Message } |
            ConvertTo-Json |
            Set-Content -Encoding utf8 -Path $outputPath
    }
}

if ($IncludeJobMetadata -and $packages.Count -gt 0) {
    $localStateRoot = Join-Path `
        $env:LOCALAPPDATA `
        "Packages\$($packages[0].PackageFamilyName)\LocalState\ERechnung"
    $metadataDestination = Join-Path $OutputRoot "job-metadata"
    $logDestination = Join-Path $OutputRoot "job-logs"
    New-Item -ItemType Directory -Force -Path $metadataDestination, $logDestination | Out-Null

    Get-ChildItem -Path (Join-Path $localStateRoot "PrintJobs") -Filter "*.json" -File -ErrorAction SilentlyContinue |
        Copy-Item -Destination $metadataDestination -Force
    Get-ChildItem -Path (Join-Path $localStateRoot "Logs") -Filter "*.jsonl" -File -ErrorAction SilentlyContinue |
        Copy-Item -Destination $logDestination -Force
}

Write-Host "Diagnostics written to: $OutputRoot"
Write-Host "PDF content was not copied. Review document names and source-app fields before sharing metadata."
