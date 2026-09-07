[CmdletBinding()]
param(
    [switch]$InstallPrerequisitesOnly,
    [switch]$SkipToolInstall,
    [switch]$SkipInstaller,
    [switch]$SkipChecks
)

$ErrorActionPreference = "Stop"
$workspace = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $workspace

function Test-Administrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Refresh-SessionPath {
    $machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $user = [Environment]::GetEnvironmentVariable("Path", "User")
    $env:Path = "$machine;$user"
    $cargo = Join-Path $env:USERPROFILE ".cargo\bin"
    if (Test-Path -LiteralPath $cargo) {
        $env:Path = "$cargo;$env:Path"
    }
    $npmGlobal = Join-Path $env:APPDATA "npm"
    if (Test-Path -LiteralPath $npmGlobal) {
        $env:Path = "$npmGlobal;$env:Path"
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

function Get-VsWhere {
    Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
}

function Get-MsBuildPath {
    $vswhere = Get-VsWhere
    if (-not (Test-Path -LiteralPath $vswhere)) {
        return $null
    }
    return & $vswhere -latest -products * -requires Microsoft.Component.MSBuild -find "MSBuild\**\Bin\MSBuild.exe" |
        Select-Object -First 1
}

function Get-SignToolPath {
    $kits = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
    if (-not (Test-Path -LiteralPath $kits)) {
        return $null
    }
    return Get-ChildItem -LiteralPath $kits -Recurse -File -Filter "signtool.exe" -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -like "*\x64\signtool.exe" } |
        Sort-Object FullName -Descending |
        Select-Object -First 1
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

function Test-WebView2 {
    $key = "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
    return Test-Path -LiteralPath $key
}

function Get-MissingTools {
    $missing = @()
    if (-not (Test-NodeVersion)) { $missing += "Node.js 22+" }
    if (-not (Test-Rust)) { $missing += "Rust (rustup)" }
    if (-not (Test-DotNetSdk10)) { $missing += ".NET SDK 10" }
    if (-not (Get-MsBuildPath)) { $missing += "Visual Studio MSBuild" }
    if (-not (Get-SignToolPath)) { $missing += "Windows SDK / signtool" }
    if (-not (Test-WebView2)) { $missing += "WebView2 Runtime" }
    if (-not (Get-Command winget -ErrorAction SilentlyContinue) -and $missing.Count -gt 0) {
        $missing += "winget"
    }
    return $missing
}

function Install-WingetPackage {
    param(
        [Parameter(Mandatory = $true)][string]$Id,
        [string]$Override = ""
    )

    $arguments = @(
        "install", "--id", $Id, "-e",
        "--accept-package-agreements", "--accept-source-agreements",
        "--disable-interactivity"
    )
    if ($Override) {
        $arguments += @("--override", $Override)
    }
    Write-Host "Installiere $Id ..."
    & winget @arguments
    $accepted = @(0, 3010, -1978335189, -1978335135)
    if ($accepted -notcontains $LASTEXITCODE) {
        throw "winget install $Id ist fehlgeschlagen (Fehlercode $LASTEXITCODE)."
    }
    Refresh-SessionPath
}

function Install-Prerequisites {
    if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
        throw "winget fehlt. Unter Windows 11 App Installer / 'App-Installer' aus dem Microsoft Store aktualisieren."
    }

    if (-not (Test-NodeVersion)) {
        Install-WingetPackage -Id "OpenJS.NodeJS.LTS"
    }
    if (-not (Test-Rust)) {
        Install-WingetPackage -Id "Rustlang.Rustup"
        Refresh-SessionPath
        if (Get-Command rustup -ErrorAction SilentlyContinue) {
            Invoke-Checked -Command "rustup" -Arguments @("toolchain", "install", "1.88.0")
            Invoke-Checked -Command "rustup" -Arguments @("default", "1.88.0")
        }
    }
    if (-not (Test-DotNetSdk10)) {
        Install-WingetPackage -Id "Microsoft.DotNet.SDK.10"
    }
    if (-not (Test-WebView2)) {
        Install-WingetPackage -Id "Microsoft.EdgeWebView2Runtime"
    }
    if (-not (Get-SignToolPath)) {
        try {
            Install-WingetPackage -Id "Microsoft.WindowsSDK.10.0.26100"
        } catch {
            Write-Warning "Windows-SDK 26100 konnte nicht einzeln installiert werden. Es wird mit den Build Tools mitinstalliert."
        }
    }
    if (-not (Get-MsBuildPath)) {
        $vsOverride = @(
            "--wait", "--passive", "--norestart",
            "--add", "Microsoft.VisualStudio.Workload.VCTools",
            "--add", "Microsoft.VisualStudio.Workload.ManagedDesktopBuildTools",
            "--add", "Microsoft.VisualStudio.Workload.UniversalBuildTools",
            "--add", "Microsoft.VisualStudio.Component.Windows11SDK.26100",
            "--includeRecommended"
        ) -join " "
        $installed = $false
        foreach ($vsId in @("Microsoft.VisualStudio.2026.BuildTools", "Microsoft.VisualStudio.2022.BuildTools")) {
            try {
                Install-WingetPackage -Id $vsId -Override $vsOverride
                $installed = $true
                break
            } catch {
                Write-Warning "$vsId konnte nicht installiert werden: $_"
            }
        }
        if (-not $installed) {
            throw "Visual Studio Build Tools konnten nicht installiert werden. Installiere Visual Studio 2026 mit .NET Desktop, WinUI/MSIX und Windows 11 SDK 26100."
        }
    }

    Refresh-SessionPath
    $stillMissing = Get-MissingTools
    if ($stillMissing.Count -gt 0) {
        throw "Nach der Werkzeuginstallation fehlen weiterhin: $($stillMissing -join ', ')."
    }
}

if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) {
    throw "Dieses Setup läuft nur unter Windows 11."
}

$osArch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($osArch -ne [System.Runtime.InteropServices.Architecture]::X64) {
    throw "Der Windows-Installer-Build ist für x64. Gefundene Architektur: $osArch"
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

$missingTools = @()
if (-not $SkipToolInstall) {
    $missingTools = Get-MissingTools
}

if ($missingTools.Count -gt 0) {
    Write-Host "Fehlende Werkzeuge: $($missingTools -join ', ')"
    if (-not (Test-Administrator)) {
        Write-Host "Hebe die Werkzeuginstallation einmalig per UAC an ..."
        $arguments = @(
            "-NoProfile",
            "-ExecutionPolicy", "Bypass",
            "-File", $PSCommandPath,
            "-InstallPrerequisitesOnly"
        )
        $elevated = Start-Process -FilePath "powershell.exe" -Verb RunAs -Wait -PassThru -ArgumentList $arguments
        if ($elevated.ExitCode -ne 0) {
            throw "Die Werkzeuginstallation (Administrator) ist fehlgeschlagen (Fehlercode $($elevated.ExitCode))."
        }
        Refresh-SessionPath
        $missingTools = Get-MissingTools
        if ($missingTools.Count -gt 0) {
            throw "Nach der Administrator-Installation fehlen weiterhin: $($missingTools -join ', '). PowerShell neu starten und das Skript erneut ausführen."
        }
    } else {
        Install-Prerequisites
    }
}

if (Get-Command rustup -ErrorAction SilentlyContinue) {
    Invoke-Checked -Command "rustup" -Arguments @("show")
}

$certificateScript = Join-Path $workspace "drucker\scripts\create-dev-cert.ps1"
Invoke-Checked -Command "powershell.exe" -Arguments @(
    "-NoProfile",
    "-ExecutionPolicy", "Bypass",
    "-File", $certificateScript
)

if (Test-Path -LiteralPath (Join-Path $workspace "package-lock.json")) {
    Invoke-Checked -Command "npm.cmd" -Arguments @("ci")
} else {
    Invoke-Checked -Command "npm.cmd" -Arguments @("install")
}

Invoke-Checked -Command "npm.cmd" -Arguments @("run", "validators:fetch")

$java = Join-Path $workspace "apps\desktop\src-tauri\resources\validators\jre\bin\java.exe"
if (-not (Test-Path -LiteralPath $java)) {
    throw "Windows-JRE fehlt nach validators:fetch ($java)."
}

Invoke-Checked -Command "npm.cmd" -Arguments @("run", "demo:invoice")

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
Write-Host "Windows-Setup abgeschlossen."
Write-Host "Installer: $($setup.FullName)"
Write-Host "Musterrechnung: $demo"
Write-Host "Kopie neben dem Installer: $(Join-Path $setup.DirectoryName 'muster-rechnung.pdf')"
Write-Host "Als Nächstes nur die Setup-Datei installieren, danach die Musterrechnung in der App öffnen."
