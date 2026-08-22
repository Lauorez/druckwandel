[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$repositoryRoot = Split-Path $PSScriptRoot -Parent
$versionPath = Join-Path $repositoryRoot "VERSION"
$propsPath = Join-Path $repositoryRoot "Directory.Build.props"
$manifestPath = Join-Path $repositoryRoot "src\CompanionApp\Package.appxmanifest"

if (-not (Test-Path $versionPath)) {
    throw "VERSION file not found: $versionPath"
}

$version = (Get-Content -Raw -Path $versionPath).Trim()
if ($version -notmatch '^(\d+)\.(\d+)\.(\d+)-([a-z]+)\.(\d+)$') {
    throw "VERSION must use the form Major.Minor.Patch-label.Iteration. Found: $version"
}

$versionPrefix = "$($Matches[1]).$($Matches[2]).$($Matches[3])"
$versionSuffix = "$($Matches[4]).$($Matches[5])"
$expectedMsixVersion = "$versionPrefix.$($Matches[5])"

[xml]$props = Get-Content -Raw -Path $propsPath
$properties = $props.Project.PropertyGroup
if ($properties.VersionPrefix -ne $versionPrefix) {
    throw "Directory.Build.props VersionPrefix is '$($properties.VersionPrefix)', expected '$versionPrefix'."
}
if ($properties.VersionSuffix -ne $versionSuffix) {
    throw "Directory.Build.props VersionSuffix is '$($properties.VersionSuffix)', expected '$versionSuffix'."
}
if ($properties.InformationalVersion -ne $version) {
    throw "Directory.Build.props InformationalVersion is '$($properties.InformationalVersion)', expected '$version'."
}
if ($properties.FileVersion -ne $expectedMsixVersion) {
    throw "Directory.Build.props FileVersion is '$($properties.FileVersion)', expected '$expectedMsixVersion'."
}

[xml]$manifest = Get-Content -Raw -Path $manifestPath
$manifestVersion = $manifest.Package.Identity.Version
if ($manifestVersion -ne $expectedMsixVersion) {
    throw "MSIX manifest version is '$manifestVersion', expected '$expectedMsixVersion'."
}

Write-Host "Product version: $version"
Write-Host "MSIX version:    $expectedMsixVersion"
