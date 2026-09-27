[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$TestRoot,

    [int]$Port = 9223
)

$ErrorActionPreference = "Stop"
$workspace = Split-Path -Parent $PSScriptRoot
if (-not (Split-Path -Leaf $TestRoot).StartsWith("erechnung-wp")) {
    throw "Nur isolierte Testwurzeln mit Präfix erechnung-wp verwenden. Keinen echten Nutzerbestand."
}
if (-not (Test-Path -LiteralPath $TestRoot)) {
    throw "Die Testwurzel fehlt."
}

Write-Host "WP14 nativer Lauf erwartet ein Debug-Fenster mit ERECHNUNG_TEST_ROOT=$TestRoot und WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=$Port"
& node (Join-Path $workspace "scripts\smoke-release.mjs") views $TestRoot $Port
if ($LASTEXITCODE -ne 0) { throw "smoke-release.mjs ist fehlgeschlagen." }
$report = Join-Path $workspace "artifacts\wp14-native-smoke.json"
if (-not (Test-Path -LiteralPath $report)) { throw "Es entstand kein artifacts/wp14-native-smoke.json." }
Write-Output "Nativer WP14-Lauf: $report"
