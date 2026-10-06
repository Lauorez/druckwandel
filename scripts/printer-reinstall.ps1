$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$package = Get-ChildItem -Path (Join-Path $workspace 'printer\artifacts\packages') `
    -Recurse -File -Filter 'CompanionApp_*.msix' -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -notmatch '[\\/]Dependencies[\\/]' } |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
if (-not $package) {
    throw 'Kein aktuelles Drucker-MSIX gefunden. Führe zuerst npm run printer:build aus.'
}
$packagePath = $package.FullName
$artifactRoot = $package.DirectoryName

$jobs = @(Get-PrintJob -PrinterName 'E-Rechnung' -ErrorAction SilentlyContinue)
if ($jobs.Count -gt 0) {
    throw "Clean-Reinstall abgebrochen: $($jobs.Count) aktiver Druckjob. Druckjob zuerst abbrechen."
}

$installedPackages = @(Get-AppxPackage -Name 'ERechnung.VirtualPrinter.PoC')
foreach ($installedPackage in $installedPackages) {
    Remove-AppxPackage -Package $installedPackage.PackageFullName
}

$deadline = (Get-Date).AddSeconds(30)
do {
    $remainingPackage = Get-AppxPackage -Name 'ERechnung.VirtualPrinter.PoC'
    $remainingPrinter = Get-Printer -Name 'E-Rechnung' -ErrorAction SilentlyContinue
    if (-not $remainingPackage -and -not $remainingPrinter) { break }
    Start-Sleep -Seconds 1
} while ((Get-Date) -lt $deadline)

if ($remainingPackage -or $remainingPrinter) {
    throw 'Alte Paket-/Queue-Registrierung wurde nicht vollständig entfernt. Spooler als Administrator neu starten.'
}

$dependencies = @(
    Get-ChildItem -LiteralPath (Join-Path $artifactRoot 'Dependencies\x64') -Filter '*.msix' -File -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty FullName
)
$installStarted = Get-Date
$installParameters = @{
    Path = $packagePath
    ForceApplicationShutdown = $true
}
if ($dependencies.Count -gt 0) {
    $installParameters.DependencyPath = $dependencies
}
Add-AppxPackage @installParameters

$deadline = (Get-Date).AddSeconds(30)
do {
    $package = Get-AppxPackage -Name 'ERechnung.VirtualPrinter.PoC'
    $printer = Get-Printer -Name 'E-Rechnung' -ErrorAction SilentlyContinue
    $createdEvent = Get-WinEvent -FilterHashtable @{
        LogName = 'Microsoft-Windows-PrintService/Operational'
        StartTime = $installStarted
        Id = 300
    } -ErrorAction SilentlyContinue | Where-Object { $_.Message -match 'E-Rechnung' } | Select-Object -First 1
    if ($package -and $printer -and $createdEvent) { break }
    Start-Sleep -Seconds 1
} while ((Get-Date) -lt $deadline)

if (-not $package -or -not $printer -or -not $createdEvent) {
    throw 'Der Drucker wurde nicht vollständig registriert: Paket, Queue oder PrintService-Create-Ereignis fehlt.'
}

$workflowServices = @(Get-Service -Name 'PrintWorkflowUserSvc*' -ErrorAction SilentlyContinue)
if ($workflowServices.Count -eq 0) {
    throw 'Der Drucker wurde installiert, aber Windows PrintWorkflowUserSvc wurde nicht gefunden.'
}
foreach ($workflowService in $workflowServices) {
    if ($workflowService.Status -eq 'Running') {
        Restart-Service -InputObject $workflowService -Force
    }
    else {
        Start-Service -InputObject $workflowService
    }
}
$workflowServices = @(Get-Service -Name 'PrintWorkflowUserSvc*')
if ($workflowServices | Where-Object Status -ne 'Running') {
    throw 'Windows PrintWorkflowUserSvc läuft nach der Installation nicht.'
}

Write-Output "Paket: $($package.PackageFullName)"
Write-Output "Drucker: $($printer.Name) ($($printer.PrinterStatus))"
Write-Output "PrintService Event 300: $($createdEvent.TimeCreated.ToString('o'))"
Write-Output "PrintWorkflow: $(($workflowServices | ForEach-Object { "$($_.Name)=$($_.Status)" }) -join ', ')"
