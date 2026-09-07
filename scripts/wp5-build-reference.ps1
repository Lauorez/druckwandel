$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$project = Join-Path $workspace 'drucker\src\CompanionApp\CompanionApp.csproj'
$pfx = Join-Path $workspace 'drucker\.cert\ERechnung.Dev.pfx'
if (-not (Test-Path -LiteralPath $pfx -PathType Leaf)) {
    throw 'Das bestätigte Development-Zertifikat unter drucker\.cert\ERechnung.Dev.pfx fehlt.'
}

dotnet publish $project `
    -p:Platform=x64 `
    -p:RuntimeIdentifier=win-x64 `
    -p:Configuration=Debug `
    -p:GenerateAppxPackageOnBuild=true `
    -p:AppxPackageSigningEnabled=false `
    -p:UapAppxPackageBuildMode=SideloadOnly `
    -p:AppxBundle=Never
if ($LASTEXITCODE -ne 0) { throw "WP5-Build fehlgeschlagen ($LASTEXITCODE)." }

$package = Get-ChildItem (Split-Path -Parent $project) -Recurse -Filter 'CompanionApp_*.msix' |
    Where-Object { $_.FullName -like '*\AppPackages\*' -and $_.FullName -notmatch '[\\/]Dependencies[\\/]' } |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if (-not $package) { throw 'WP5-MSIX wurde nicht gefunden.' }

$signTool = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin' -Recurse -Filter 'signtool.exe' |
    Where-Object { $_.FullName -like '*\x64\signtool.exe' } |
    Sort-Object FullName -Descending |
    Select-Object -First 1
if (-not $signTool) { throw 'signtool.exe wurde nicht gefunden.' }

& $signTool.FullName sign /fd SHA256 /f $pfx /p 'ERechnung-Dev-Only' $package.FullName
if ($LASTEXITCODE -ne 0) { throw "MSIX-Signatur fehlgeschlagen ($LASTEXITCODE)." }
& $signTool.FullName verify /pa $package.FullName
if ($LASTEXITCODE -ne 0) { throw "MSIX-Signaturprüfung fehlgeschlagen ($LASTEXITCODE)." }

Write-Output $package.FullName
