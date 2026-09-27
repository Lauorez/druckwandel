$ErrorActionPreference = 'Stop'

$workspace = Split-Path -Parent $PSScriptRoot
$pfx = Join-Path $workspace 'drucker\.cert\ERechnung.Dev.pfx'
if (-not (Test-Path -LiteralPath $pfx -PathType Leaf)) {
    throw 'Das Development-Zertifikat unter drucker\.cert\ERechnung.Dev.pfx fehlt.'
}

& (Join-Path $workspace 'drucker\scripts\build.ps1') -Platform x64 -Configuration Release
if ($LASTEXITCODE -ne 0) { throw "WP5-Build fehlgeschlagen ($LASTEXITCODE)." }
