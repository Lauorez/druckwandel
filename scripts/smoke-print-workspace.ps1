[CmdletBinding()]
param([ValidateRange(10,180)][int]$TimeoutSeconds = 90)

# Intentionally exercises the installed virtual printer and normal user profile.
# Creates one clearly named synthetic draft; does not export, learn, or delete.
$ErrorActionPreference = 'Stop'
$workspaceRoot = Split-Path -Parent $PSScriptRoot
$installedApp = Join-Path $env:LOCALAPPDATA 'Druckwandel\erechnung-desktop.exe'
if (-not (Test-Path -LiteralPath $installedApp)) { throw 'Install the application first.' }
if (Get-Process erechnung-desktop -ErrorAction SilentlyContinue) { throw 'Close the application normally before this cold-start test.' }
$printer = Get-Printer -Name 'E-Rechnung'
if ($printer.DriverName -ne 'Microsoft Virtual Print Class Driver') { throw 'Unexpected printer driver; test stopped.' }

$testName = 'WP7 Drucktest ' + [Guid]::NewGuid().ToString('N')
$inboxPath = Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'E-Rechnung Druckeingang'
$startedAt = [DateTime]::UtcNow
Add-Type -AssemblyName System.Drawing
$document = New-Object System.Drawing.Printing.PrintDocument
$font = New-Object System.Drawing.Font('Arial',12)
try {
    $document.DocumentName = $testName
    $document.PrinterSettings.PrinterName = 'E-Rechnung'
    $document.PrintController = New-Object System.Drawing.Printing.StandardPrintController
    if (-not $document.PrinterSettings.IsValid) { throw 'Virtual printer is not ready.' }
    $content = @"
WP7 - Technischer Drucktest

Testbetrieb Musterwerkstatt
Teststrasse 1
12345 Musterstadt

Rechnung Nr.: WP7-TEST
Rechnungsdatum: 06.09.2026

Beschreibung                  Menge       Einzelpreis       Gesamt
Testleistung                       1          100,00 EUR       100,00 EUR

Nettobetrag: 100,00 EUR
Umsatzsteuer 19 %: 19,00 EUR
Gesamtbetrag: 119,00 EUR

Nur technischer Test - keine echte Rechnung.
$testName
"@
    $document.add_PrintPage({
        param($sender,$eventArgs)
        $eventArgs.Graphics.DrawString($content,$font,[System.Drawing.Brushes]::Black,[single]70,[single]70)
        $eventArgs.HasMorePages = $false
    })
    $document.Print()
} finally { $document.Dispose(); $font.Dispose() }

$deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
$handoff = $null
$receipt = $null
while ([DateTime]::UtcNow -lt $deadline) {
    if (-not $handoff) {
        foreach ($file in @(Get-ChildItem -LiteralPath $inboxPath -Filter '*.printjob.json' -File -ErrorAction SilentlyContinue | Where-Object LastWriteTimeUtc -GE $startedAt.AddSeconds(-2))) {
            try { $candidate = Get-Content -LiteralPath $file.FullName -Raw | ConvertFrom-Json } catch { continue }
            if ($candidate.documentName -eq $testName) { $handoff = $candidate; break }
        }
    }
    if ($handoff) {
        $receiptPath = Join-Path $inboxPath ($handoff.jobId + '.review.json')
        if (Test-Path -LiteralPath $receiptPath) {
            try { $receipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json } catch { $receipt = $null }
            if ($receipt) { break }
        }
    }
    Start-Sleep -Milliseconds 300
}
if (-not $handoff -or -not $receipt) { throw 'Timed out waiting for the print handoff and opening receipt.' }
if ($receipt.status -ne 'opened') { throw ('Print was not opened: ' + $receipt.message) }
$process = Get-Process erechnung-desktop | Where-Object Path -EQ $installedApp
if (@($process).Count -ne 1) { throw 'Expected exactly one installed application process.' }

$workspaceData = Join-Path $env:LOCALAPPDATA 'de.erechnung.converter\workspace'
$pdfPath = Join-Path $inboxPath $handoff.pdfFileName
$reportDirectory = Join-Path $workspaceRoot 'artifacts'
New-Item -ItemType Directory -Path $reportDirectory -Force | Out-Null
& node (Join-Path $PSScriptRoot 'verify-print-workspace.mjs') $workspaceData $handoff.jobId $pdfPath (Join-Path $reportDirectory 'wp7-print-smoke.json')
if ($LASTEXITCODE -ne 0) { throw 'Persistent workspace verification failed.' }
Write-Output ('Test draft retained: ' + $testName)
