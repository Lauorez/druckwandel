[CmdletBinding()]
param(
    [ValidateRange(10, 180)]
    [int]$QueueTimeoutSeconds = 45
)

$ErrorActionPreference = "Stop"
$logPath = Join-Path $env:TEMP "E-Rechnungs-Assistent-Deinstallation.log"
$packageName = "ERechnung.VirtualPrinter.PoC"
$printerName = "E-Rechnung"

function Write-SetupLog {
    param([Parameter(Mandatory = $true)][string]$Message)

    $line = "{0}  {1}" -f (Get-Date).ToString("o"), $Message
    Add-Content -LiteralPath $logPath -Value $line -Encoding UTF8
    Write-Output $Message
}

try {
    Set-Content -LiteralPath $logPath -Value "E-Rechnungs-Assistent – Deinstallation" -Encoding UTF8
    Import-Module (Join-Path $PSHOME "Modules\Appx\Appx.psd1") -Force
    Import-Module (Join-Path $PSHOME "Modules\PrintManagement\PrintManagement.psd1") -Force
    $packages = @(Get-AppxPackage -Name $packageName)
    if ($packages.Count -eq 0) {
        Write-SetupLog "Der E-Rechnungsdrucker ist bereits entfernt."
        exit 0
    }

    $activeJobs = @(Get-PrintJob -PrinterName $printerName -ErrorAction SilentlyContinue)
    if ($activeJobs.Count -gt 0) {
        throw "Der Drucker kann nicht entfernt werden, solange noch ein Druckauftrag läuft."
    }

    foreach ($package in $packages) {
        Remove-AppxPackage -Package $package.PackageFullName
    }

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($QueueTimeoutSeconds)
    do {
        $remainingPackage = Get-AppxPackage -Name $packageName
        $remainingPrinter = Get-Printer -Name $printerName -ErrorAction SilentlyContinue
        if (-not $remainingPackage -and -not $remainingPrinter) {
            Write-SetupLog "Der E-Rechnungsdrucker wurde entfernt."
            exit 0
        }
        Start-Sleep -Milliseconds 500
    } while ([DateTimeOffset]::UtcNow -lt $deadline)

    throw "Windows hat den E-Rechnungsdrucker nicht vollständig entfernt."
} catch {
    $message = $_.Exception.Message
    try {
        Write-SetupLog "FEHLER: $message"
    } catch {
        # Preserve the original removal error if logging fails.
    }
    Write-Error $message
    exit 1
}
