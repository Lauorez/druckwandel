[CmdletBinding()]
param(
    [int]$QueueTimeoutSeconds = 30
)

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path $PSScriptRoot -Parent

& (Join-Path $PSScriptRoot "install-dev-cert.ps1")

$package = Get-ChildItem -Path (Join-Path $repositoryRoot "artifacts\packages") -Recurse -File |
    Where-Object {
        $_.Extension -in ".msix", ".msixbundle" -and
        $_.FullName -notmatch "[\\/]Dependencies[\\/]"
    } |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1

if (-not $package) {
    throw "Kein MSIX-Paket gefunden. Führe zuerst .\scripts\build.ps1 aus."
}

$dependencyArchitecture = if ($package.Name -match "arm64") { "arm64" } else { "x64" }
$dependencyPath = Join-Path $package.DirectoryName "Dependencies\$dependencyArchitecture"
$dependencies = @(
    Get-ChildItem -Path $dependencyPath -File -Filter "*.msix" -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty FullName
)

$installParameters = @{
    Path                     = $package.FullName
    ForceApplicationShutdown = $true
}
if ($dependencies.Count -gt 0) {
    $installParameters.DependencyPath = $dependencies
}

Add-AppxPackage @installParameters
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
    $workflowServices = @(Get-Service -Name "PrintWorkflowUserSvc*" -ErrorAction SilentlyContinue)
    if ($workflowServices.Count -eq 0) {
        throw "Windows PrintWorkflowUserSvc wurde nicht gefunden."
    }
    foreach ($workflowService in $workflowServices) {
        if ($workflowService.Status -eq "Running") {
            Restart-Service -InputObject $workflowService -Force
        } else {
            Start-Service -InputObject $workflowService
        }
    }
    if (Get-Service -Name "PrintWorkflowUserSvc*" | Where-Object Status -ne "Running") {
        throw "Windows PrintWorkflowUserSvc läuft nach der Installation nicht."
    }

    Write-Host "Printer ready: $($printer.Name)"
}
