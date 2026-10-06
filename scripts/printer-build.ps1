$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$pfx = Join-Path $workspace 'printer\.cert\ERechnung.Dev.pfx'
if (-not (Test-Path -LiteralPath $pfx -PathType Leaf)) {
    throw 'Das Development-Zertifikat unter printer\.cert\ERechnung.Dev.pfx fehlt.'
}

& (Join-Path $workspace 'printer\scripts\build.ps1') -Platform x64 -Configuration Release
if ($LASTEXITCODE -ne 0) { throw "Der Drucker-Build ist fehlgeschlagen ($LASTEXITCODE)." }
