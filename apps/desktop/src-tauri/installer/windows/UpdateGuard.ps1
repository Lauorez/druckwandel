[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet("PrepareUpdate", "RecordInstalledVersion", "AssertUserDataKept")]
    [string]$Action,

    [string]$IncomingVersion = "",

    [string]$IsolatedRoot = ""
)

$ErrorActionPreference = "Stop"

function Get-Roots {
    if ($IsolatedRoot) {
        return [pscustomobject]@{
            Documents = Join-Path $IsolatedRoot "documents"
            Local     = Join-Path $IsolatedRoot "local"
            Roaming   = Join-Path $IsolatedRoot "roaming"
        }
    }
    $documents = [Environment]::GetFolderPath("MyDocuments")
    if (-not $documents) {
        throw "Der Dokumente-Ordner konnte nicht ermittelt werden."
    }
    return [pscustomobject]@{
        Documents = $documents
        Local     = Join-Path $env:LOCALAPPDATA "de.erechnung.converter"
        Roaming   = Join-Path $env:APPDATA "de.erechnung.converter"
    }
}

function Convert-AppVersion {
    param([string]$Value)
    if ([string]::IsNullOrWhiteSpace($Value)) { return $null }
    $numeric = ($Value -split "[^0-9.]")[0]
    if ([string]::IsNullOrWhiteSpace($numeric)) { return $null }
    $parts = @($numeric.Split("."))
    while ($parts.Count -lt 3) { $parts += "0" }
    return [version](($parts[0..2] -join ".") + ".0")
}

function Get-PreviousVersion {
    param($Roots)
    $marker = Join-Path $Roots.Local "installed-version.txt"
    if (Test-Path -LiteralPath $marker) {
        return (Get-Content -LiteralPath $marker -TotalCount 1 -ErrorAction SilentlyContinue)
    }
    if ($IsolatedRoot) { return $null }
    $uninstall = Get-ChildItem -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall" -ErrorAction SilentlyContinue
    foreach ($key in $uninstall) {
        $name = (Get-ItemProperty -LiteralPath $key.PSPath -ErrorAction SilentlyContinue).DisplayName
        $version = (Get-ItemProperty -LiteralPath $key.PSPath -ErrorAction SilentlyContinue).DisplayVersion
        if ($name -like "*E-Rechnungs-Assistent*" -and $version) {
            return [string]$version
        }
    }
    return $null
}

function Copy-IfPresent {
    param([string]$Source, [string]$Destination)
    if (-not (Test-Path -LiteralPath $Source)) { return }
    $parent = Split-Path -Parent $Destination
    New-Item -ItemType Directory -Force -Path $parent | Out-Null
    if ((Get-Item -LiteralPath $Source).PSIsContainer) {
        Copy-Item -LiteralPath $Source -Destination $Destination -Recurse -Force
    } else {
        Copy-Item -LiteralPath $Source -Destination $Destination -Force
    }
}

function Save-UpdateSnapshot {
    param($Roots, [string]$Version)
    $stamp = if ($Version) { $Version } else { Get-Date -Format "yyyyMMdd-HHmmss" }
    $target = Join-Path $Roots.Local "update-backup\$stamp"
    New-Item -ItemType Directory -Force -Path $target | Out-Null
    Copy-IfPresent -Source (Join-Path $Roots.Local "workspace\workspace.sqlite3") -Destination (Join-Path $target "workspace.sqlite3")
    Copy-IfPresent -Source (Join-Path $Roots.Roaming "correction-memory.json") -Destination (Join-Path $target "correction-memory.json")
    Copy-IfPresent -Source (Join-Path $Roots.Roaming "archive-signing-key-v1.bin") -Destination (Join-Path $target "archive-signing-key-v1.bin")
    Copy-IfPresent -Source (Join-Path $Roots.Documents "E-Rechnungsarchiv\archiv.sqlite3") -Destination (Join-Path $target "archiv.sqlite3")
    Copy-IfPresent -Source (Join-Path $Roots.Documents "E-Rechnungsarchiv\Steuerkanzlei\exporte.sqlite3") -Destination (Join-Path $target "exporte.sqlite3")
    Copy-IfPresent -Source (Join-Path $Roots.Local "backup-state.json") -Destination (Join-Path $target "backup-state.json")
    $readme = @(
        "Wiederherstellung nach fehlgeschlagenem Update",
        "",
        "Die Anwendungsdateien wurden aktualisiert. Rechnungen im Ordner Dokumente\E-Rechnungsarchiv bleiben unangetastet.",
        "Falls Entwürfe oder das Vorlagengedächtnis fehlen:",
        "1. Anwendung schließen.",
        "2. workspace.sqlite3 nach %LOCALAPPDATA%\de.erechnung.converter\workspace\ kopieren.",
        "3. correction-memory.json und archive-signing-key-v1.bin in den Roaming-Anwendungsordner kopieren.",
        "4. archiv.sqlite3 nur zurückkopieren, wenn die aktuelle Archivdatenbank beschädigt ist.",
        "Keine Abwärtsinstallation. Eine portable .erechnung-Sicherung ist der bevorzugte Weg."
    ) -join "`r`n"
    Set-Content -LiteralPath (Join-Path $target "WIEDERHERSTELLUNG.txt") -Value $readme -Encoding UTF8
    Write-Output $target
}

$roots = Get-Roots

if ($Action -eq "PrepareUpdate") {
    $previous = Get-PreviousVersion -Roots $roots
    $incoming = Convert-AppVersion $IncomingVersion
    $installed = Convert-AppVersion $previous
    if ($incoming -and $installed -and $incoming -lt $installed) {
        throw "Eine neuere Version ($previous) ist bereits installiert. Eine Abwärtsinstallation ist nicht zulässig."
    }
    Save-UpdateSnapshot -Roots $roots -Version $IncomingVersion | Out-Null
    exit 0
}

if ($Action -eq "RecordInstalledVersion") {
    if ([string]::IsNullOrWhiteSpace($IncomingVersion)) {
        throw "Die installierte Version konnte nicht aufgezeichnet werden."
    }
    New-Item -ItemType Directory -Force -Path $roots.Local | Out-Null
    Set-Content -LiteralPath (Join-Path $roots.Local "installed-version.txt") -Value $IncomingVersion -Encoding ASCII
    Write-Output "Installierte Version: $IncomingVersion"
    exit 0
}

$archive = Join-Path $roots.Documents "E-Rechnungsarchiv"
$drafts = Join-Path $roots.Documents "E-Rechnung Entwürfe"
$workspace = Join-Path $roots.Local "workspace"
foreach ($path in @($archive, $drafts, $workspace)) {
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Nutzerdaten fehlen nach der Deinstallation: $path"
    }
}
Write-Output "Nutzerdaten sind erhalten."
exit 0
