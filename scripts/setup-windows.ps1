[CmdletBinding()]
param(
    [switch]$InstallPrerequisitesOnly,
    [switch]$SkipToolInstall,
    [switch]$SkipInstaller,
    [switch]$SkipChecks
)

$ErrorActionPreference = "Stop"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$workspace = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $workspace
$toolsRoot = Join-Path $env:LOCALAPPDATA "ERechnungsAssistent\tools"
$nodeHome = Join-Path $toolsRoot "node"
$dotnetHome = Join-Path $env:LOCALAPPDATA "Microsoft\dotnet"
$cargoHome = Join-Path $env:USERPROFILE ".cargo\bin"

function Add-UserPathEntry {
    param([Parameter(Mandatory = $true)][string]$Directory)

    if (-not (Test-Path -LiteralPath $Directory)) {
        return
    }
    $user = [Environment]::GetEnvironmentVariable("Path", "User")
    $parts = @()
    if ($user) {
        $parts = @($user.Split(";", [StringSplitOptions]::RemoveEmptyEntries))
    }
    if ($parts -notcontains $Directory) {
        [Environment]::SetEnvironmentVariable("Path", ($Directory, $user -join ";").Trim(";"), "User")
    }
    if ($env:Path -notlike "*$Directory*") {
        $env:Path = "$Directory;$env:Path"
    }
}

function Refresh-SessionPath {
    $machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $user = [Environment]::GetEnvironmentVariable("Path", "User")
    $env:Path = "$machine;$user"
    foreach ($directory in @($cargoHome, $dotnetHome, $nodeHome, (Join-Path $env:APPDATA "npm"))) {
        Add-UserPathEntry -Directory $directory
    }
}

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "'$Command $($Arguments -join ' ')' ist fehlgeschlagen (Fehlercode $LASTEXITCODE)."
    }
}

function Get-NpmCommand {
    $npm = Get-Command npm.cmd -ErrorAction SilentlyContinue
    if ($npm) {
        return $npm.Source
    }
    $npm = Get-Command npm -ErrorAction SilentlyContinue
    if ($npm) {
        return $npm.Source
    }
    throw "npm wurde nicht gefunden. Node.js liegt nach der Benutzerinstallation unter $nodeHome."
}

function Save-RemoteFile {
    param(
        [Parameter(Mandatory = $true)][string]$Url,
        [Parameter(Mandatory = $true)][string]$Path
    )

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    Write-Host "Lade $Url ..."
    $client = New-Object System.Net.WebClient
    $client.Headers.Add("User-Agent", "erechnungs-assistent-windows-setup")
    $client.DownloadFile($Url, $Path)
}

function Test-NodeVersion {
    $node = Get-Command node -ErrorAction SilentlyContinue
    if (-not $node) {
        return $false
    }
    $raw = (& node -v).TrimStart("v")
    $version = [version]$raw.Split("-")[0]
    return $version.Major -ge 22
}

function Test-DotNetSdk10 {
    $dotnet = Get-Command dotnet -ErrorAction SilentlyContinue
    if (-not $dotnet) {
        return $false
    }
    $sdks = & dotnet --list-sdks 2>$null
    return [bool]($sdks | Where-Object { $_ -like "10.*" })
}

function Test-Rust {
    return [bool](Get-Command rustup -ErrorAction SilentlyContinue) -and
        [bool](Get-Command cargo -ErrorAction SilentlyContinue)
}

function Get-MissingTools {
    $missing = @()
    if (-not (Test-NodeVersion)) { $missing += "Node.js 22+" }
    if (-not (Test-Rust)) { $missing += "Rust (rustup)" }
    if (-not (Test-DotNetSdk10)) { $missing += ".NET SDK 10" }
    return $missing
}

function Install-UserNode {
    $index = Invoke-RestMethod -Uri "https://nodejs.org/dist/index.json"
    $release = $index | Where-Object { $_.version.StartsWith("v22.") -and $_.lts } | Select-Object -First 1
    if (-not $release) {
        throw "Keine Node.js-22-LTS-Version gefunden."
    }
    $zipName = "node-$($release.version)-win-x64.zip"
    $zipPath = Join-Path $env:TEMP $zipName
    Save-RemoteFile -Url "https://nodejs.org/dist/$($release.version)/$zipName" -Path $zipPath
    $extract = Join-Path $env:TEMP "node-extract-$(Get-Random)"
    New-Item -ItemType Directory -Force -Path $extract | Out-Null
    tar --force-local -xf $zipPath -C $extract
    $payload = Get-ChildItem -LiteralPath $extract -Directory | Select-Object -First 1
    if (-not $payload -or -not (Test-Path -LiteralPath (Join-Path $payload.FullName "node.exe"))) {
        throw "Das Node.js-Paket enthält keine node.exe."
    }
    if (Test-Path -LiteralPath $nodeHome) {
        Remove-Item -LiteralPath $nodeHome -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $nodeHome) | Out-Null
    Move-Item -LiteralPath $payload.FullName -Destination $nodeHome
    Add-UserPathEntry -Directory $nodeHome
}

function Install-UserRust {
    $rustupInit = Join-Path $env:TEMP "rustup-init.exe"
    Save-RemoteFile -Url "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" -Path $rustupInit
    & $rustupInit -y --default-toolchain 1.88.0 --default-host x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) {
        throw "rustup-init ist fehlgeschlagen (Fehlercode $LASTEXITCODE)."
    }
    Add-UserPathEntry -Directory $cargoHome
    Refresh-SessionPath
    Invoke-Checked -Command "rustup" -Arguments @("toolchain", "install", "1.88.0")
}

function Install-UserDotNet {
    $script = Join-Path $env:TEMP "dotnet-install.ps1"
    Save-RemoteFile -Url "https://dot.net/v1/dotnet-install.ps1" -Path $script
    & $script -Channel 10.0 -InstallDir $dotnetHome
    Add-UserPathEntry -Directory $dotnetHome
}

function Install-Prerequisites {
    Write-Host "Installiere fehlende Werkzeuge nur für das aktuelle Benutzerkonto (ohne Administrator)."
    New-Item -ItemType Directory -Force -Path $toolsRoot | Out-Null
    if (-not (Test-NodeVersion)) {
        Install-UserNode
        Refresh-SessionPath
    }
    if (-not (Test-Rust)) {
        Install-UserRust
        Refresh-SessionPath
    }
    if (-not (Test-DotNetSdk10)) {
        Install-UserDotNet
        Refresh-SessionPath
    }
    $stillMissing = Get-MissingTools
    if ($stillMissing.Count -gt 0) {
        throw "Nach der Benutzerinstallation fehlen weiterhin: $($stillMissing -join ', ')."
    }
}

function Get-NativeProcessorArchitecture {
    if ($env:PROCESSOR_ARCHITEW6432) {
        return $env:PROCESSOR_ARCHITEW6432
    }
    return $env:PROCESSOR_ARCHITECTURE
}

function Get-ForwardedSetupArguments {
    $arguments = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", $PSCommandPath
    )
    if ($InstallPrerequisitesOnly) { $arguments += "-InstallPrerequisitesOnly" }
    if ($SkipToolInstall) { $arguments += "-SkipToolInstall" }
    if ($SkipInstaller) { $arguments += "-SkipInstaller" }
    if ($SkipChecks) { $arguments += "-SkipChecks" }
    return $arguments
}

if ($env:OS -ne "Windows_NT") {
    throw "Dieses Setup läuft nur unter Windows 11."
}

$nativeArch = Get-NativeProcessorArchitecture
if ($nativeArch -ne "AMD64") {
    throw "Der Windows-Installer-Build ist für Windows 11 x64 (AMD64). Gefundene Prozessorarchitektur: $nativeArch"
}

if (-not [Environment]::Is64BitProcess) {
    $sysnative = Join-Path $env:WINDIR "Sysnative\WindowsPowerShell\v1.0\powershell.exe"
    $system32 = Join-Path $env:WINDIR "System32\WindowsPowerShell\v1.0\powershell.exe"
    $powershell64 = if (Test-Path -LiteralPath $sysnative) { $sysnative } else { $system32 }
    if (-not (Test-Path -LiteralPath $powershell64)) {
        throw "64-Bit-PowerShell wurde nicht gefunden. Bitte System32-PowerShell auf dem x64-System starten."
    }
    Write-Host "32-Bit-PowerShell erkannt. Starte 64-Bit-PowerShell neu ..."
    & $powershell64 @(Get-ForwardedSetupArguments)
    exit $LASTEXITCODE
}

$currentBuild = [Environment]::OSVersion.Version.Build
if ($currentBuild -lt 26100) {
    throw "Windows 11 24H2 (Build 26100) oder neuer ist erforderlich. Gefunden: $currentBuild"
}

Refresh-SessionPath

if ($InstallPrerequisitesOnly) {
    Install-Prerequisites
    Write-Host "Build-Werkzeuge sind eingerichtet."
    return
}

if (-not $SkipToolInstall) {
    $missingTools = Get-MissingTools
    if ($missingTools.Count -gt 0) {
        Write-Host "Fehlende Werkzeuge: $($missingTools -join ', ')"
        Install-Prerequisites
        Refresh-SessionPath
    }
}

if (Get-Command rustup -ErrorAction SilentlyContinue) {
    Invoke-Checked -Command "rustup" -Arguments @("show")
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$msvcLinker = $null
if (Test-Path -LiteralPath $vswhere) {
    $msvcLinker = & $vswhere -latest -products * -find "VC\Tools\MSVC\**\bin\Hostx64\x64\link.exe" |
        Select-Object -First 1
}
if (-not $msvcLinker -and -not (Get-Command link.exe -ErrorAction SilentlyContinue)) {
    Write-Warning "Kein MSVC-Linker gefunden. Der Tauri-Build braucht vorhandene C++-Build-Tools; ohne Administrator können sie nicht nachinstalliert werden."
}

$certificateScript = Join-Path $workspace "drucker\scripts\create-dev-cert.ps1"
Invoke-Checked -Command "powershell.exe" -Arguments @(
    "-NoProfile",
    "-ExecutionPolicy", "Bypass",
    "-File", $certificateScript
)

$npm = Get-NpmCommand
if (Test-Path -LiteralPath (Join-Path $workspace "package-lock.json")) {
    Invoke-Checked -Command $npm -Arguments @("ci")
} else {
    Invoke-Checked -Command $npm -Arguments @("install")
}

Invoke-Checked -Command $npm -Arguments @("run", "validators:fetch")

$java = Join-Path $workspace "apps\desktop\src-tauri\resources\validators\jre\bin\java.exe"
if (-not (Test-Path -LiteralPath $java)) {
    throw "Windows-JRE fehlt nach validators:fetch ($java)."
}

Invoke-Checked -Command $npm -Arguments @("run", "demo:invoice")

if ($SkipInstaller) {
    Write-Host "Werkzeuge, Validatoren und Musterrechnung sind eingerichtet. Installer-Build wurde übersprungen."
    return
}

$installerScript = Join-Path $workspace "scripts\build-windows-installer.ps1"
$installerArguments = @(
    "-NoProfile",
    "-ExecutionPolicy", "Bypass",
    "-File", $installerScript
)
if ($SkipChecks) {
    $installerArguments += "-SkipChecks"
}
Invoke-Checked -Command "powershell.exe" -Arguments $installerArguments

$setup = Get-ChildItem -LiteralPath (Join-Path $workspace "artifacts\windows") -File -Filter "E-Rechnungs-Assistent-*-x64-Setup.exe" |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
$demo = Join-Path $workspace "artifacts\demo\muster-rechnung.pdf"
if (-not $setup) {
    throw "Die Setup-Datei wurde nicht erzeugt."
}
if (-not (Test-Path -LiteralPath $demo)) {
    throw "Die Musterrechnung wurde nicht erzeugt."
}

Copy-Item -LiteralPath $demo -Destination (Join-Path $setup.DirectoryName "muster-rechnung.pdf") -Force

Write-Host ""
Write-Host "Windows-Setup abgeschlossen (ohne Administratorrechte)."
Write-Host "Installer: $($setup.FullName)"
Write-Host "Musterrechnung: $demo"
Write-Host "Kopie neben dem Installer: $(Join-Path $setup.DirectoryName 'muster-rechnung.pdf')"
Write-Host "Als Nächstes nur die Setup-Datei installieren (aktueller Benutzer), danach die Musterrechnung in der App öffnen."
