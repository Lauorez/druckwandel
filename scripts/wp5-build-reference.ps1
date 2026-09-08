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

$nugetPackages = if ($env:NUGET_PACKAGES) { $env:NUGET_PACKAGES } else { Join-Path $env:USERPROFILE '.nuget\packages' }
$signTool = Get-ChildItem -Path (Join-Path $nugetPackages 'microsoft.windows.sdk.buildtools') -Recurse -File -Filter 'signtool.exe' -ErrorAction SilentlyContinue |
    Where-Object { $_.DirectoryName -match '[\\/]x64$' } |
    Sort-Object FullName -Descending |
    Select-Object -First 1
if (-not $signTool) {
    $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    if (Test-Path -LiteralPath $kits) {
        $signTool = Get-ChildItem -LiteralPath $kits -Recurse -File -Filter 'signtool.exe' -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -like '*\x64\signtool.exe' } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
    }
}
if (-not $signTool) { throw 'signtool.exe wurde nicht gefunden (NuGet Windows SDK BuildTools oder Windows Kits).' }

$cer = Join-Path $workspace 'drucker\.cert\ERechnung.Dev.cer'
if (Test-Path -LiteralPath $cer) {
    Import-Certificate -FilePath $cer -CertStoreLocation 'Cert:\CurrentUser\TrustedPeople' | Out-Null
    Import-Certificate -FilePath $cer -CertStoreLocation 'Cert:\CurrentUser\Root' | Out-Null
}

& $signTool.FullName sign /fd SHA256 /f $pfx /p 'ERechnung-Dev-Only' $package.FullName
if ($LASTEXITCODE -ne 0) { throw "MSIX-Signatur fehlgeschlagen ($LASTEXITCODE)." }
& $signTool.FullName verify /pa $package.FullName
if ($LASTEXITCODE -ne 0) {
    Write-Warning "signtool verify /pa ohne maschinenweites Zertifikatvertrauen fehlgeschlagen. Die Signatur wurde trotzdem geschrieben."
}

Write-Output $package.FullName
