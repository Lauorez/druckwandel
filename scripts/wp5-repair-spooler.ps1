[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$principal = [Security.Principal.WindowsPrincipal]::new(
    [Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    $process = Start-Process `
        -FilePath powershell.exe `
        -Verb RunAs `
        -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`"" `
        -PassThru `
        -Wait
    exit $process.ExitCode
}

$spoolPath = [IO.Path]::GetFullPath(
    [Environment]::ExpandEnvironmentVariables('%SystemRoot%\System32\spool\PRINTERS'))
$expectedSpoolPath = [IO.Path]::GetFullPath("$env:SystemRoot\System32\spool\PRINTERS")
if (-not [StringComparer]::OrdinalIgnoreCase.Equals($spoolPath, $expectedSpoolPath)) {
    throw "Unerwarteter Spool-Pfad: $spoolPath"
}
if (-not (Test-Path -LiteralPath $spoolPath -PathType Container)) {
    throw "Windows-Spool-Verzeichnis fehlt: $spoolPath"
}

$activeJobs = @(
    Get-Printer | ForEach-Object {
        Get-PrintJob -PrinterName $_.Name -ErrorAction SilentlyContinue
    }
)
if ($activeJobs.Count -gt 0) {
    throw "Reparatur abgebrochen: Es existieren noch $($activeJobs.Count) sichtbare Druckjobs."
}

$logPath = Join-Path (Split-Path -Parent $PSScriptRoot) 'artifacts\wp5-spooler-repair.log'
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $logPath) | Out-Null

$removedFiles = [Collections.Generic.List[string]]::new()
$spoolerStopped = $false
try {
    Stop-Service -Name Spooler -Force
    $spoolerStopped = $true

    $orphanedFiles = @(
        Get-ChildItem -LiteralPath $spoolPath -Force -File |
            Where-Object { $_.Extension -in '.SHD', '.SPL', '.TMP' }
    )

    foreach ($file in $orphanedFiles) {
        $resolvedFile = [IO.Path]::GetFullPath($file.FullName)
        $parentPath = [IO.Path]::GetFullPath($file.DirectoryName)
        if (-not [StringComparer]::OrdinalIgnoreCase.Equals($parentPath, $spoolPath)) {
            throw "Datei liegt außerhalb des geprüften Spool-Verzeichnisses: $resolvedFile"
        }

        Remove-Item -LiteralPath $resolvedFile -Force
        $removedFiles.Add($resolvedFile)
    }
}
finally {
    if ($spoolerStopped) {
        Start-Service -Name Spooler
    }
}

$spooler = Get-Service -Name Spooler
$workflowServices = @(Get-Service -Name 'PrintWorkflowUserSvc*' -ErrorAction SilentlyContinue)
if ($workflowServices.Count -eq 0) {
    throw 'Windows PrintWorkflowUserSvc wurde nicht gefunden.'
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
$remainingFiles = @(
    Get-ChildItem -LiteralPath $spoolPath -Force -File |
        Where-Object { $_.Extension -in '.SHD', '.SPL', '.TMP' }
)

$lines = @(
    "Timestamp: $([DateTimeOffset]::Now.ToString('o'))"
    "Spooler: $($spooler.Status)"
    "PrintWorkflow: $(($workflowServices | ForEach-Object { "$($_.Name)=$($_.Status)" }) -join ', ')"
    "Removed: $($removedFiles.Count)"
) + ($removedFiles | ForEach-Object { "RemovedFile: $_" }) + @(
    "Remaining: $($remainingFiles.Count)"
)
$lines | Set-Content -LiteralPath $logPath -Encoding utf8

if ($spooler.Status -ne 'Running') {
    throw "Der Druckspooler läuft nach der Reparatur nicht: $($spooler.Status)"
}
if ($workflowServices | Where-Object Status -ne 'Running') {
    throw 'Mindestens ein Windows-PrintWorkflowUserSvc läuft nach der Reparatur nicht.'
}
if ($remainingFiles.Count -gt 0) {
    throw "Im Spool-Verzeichnis liegen weiterhin $($remainingFiles.Count) Jobdateien."
}

Write-Output "Spooler repariert. Entfernte verwaiste Jobdateien: $($removedFiles.Count)"
Write-Output "Protokoll: $logPath"
