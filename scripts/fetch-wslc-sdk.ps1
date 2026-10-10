#Requires -Version 5.1
<#
.SYNOPSIS
  Optional: download Microsoft.WSL.Containers for local `cargo run` / PATH setup.

.DESCRIPTION
  The MSI does **not** bundle wslcsdk.dll. LIVE mode loads the host copy
  (WSL install dir or PATH). Use this script only when developing without a
  system SDK on PATH.

.PARAMETER Version
  NuGet package version. Defaults to $env:WSLC_SDK_VERSION or 3.0.1.

.PARAMETER OutDir
  Directory that receives wslcsdk.dll (default: .wslc-sdk\native).
#>
param(
    [string]$Version = $(if ($env:WSLC_SDK_VERSION) { $env:WSLC_SDK_VERSION } else { "3.0.1" }),
    [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
if (-not $OutDir) {
    $OutDir = Join-Path $repoRoot ".wslc-sdk\native"
}

$cacheRoot = Join-Path $repoRoot ".wslc-sdk"
$nupkgName = "microsoft.wsl.containers.$Version.nupkg"
$nupkgPath = Join-Path $cacheRoot $nupkgName
$extractDir = Join-Path $cacheRoot "Microsoft.WSL.Containers.$Version"
$downloadUrl = "https://api.nuget.org/v3-flatcontainer/microsoft.wsl.containers/$Version/$nupkgName"

New-Item -ItemType Directory -Force -Path $cacheRoot | Out-Null
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

if (-not (Test-Path $nupkgPath)) {
    Write-Host "Downloading $downloadUrl"
    Invoke-WebRequest -Uri $downloadUrl -OutFile $nupkgPath -UseBasicParsing
} else {
    Write-Host "Using cached $nupkgPath"
}

if (Test-Path $extractDir) {
    Remove-Item -Recurse -Force $extractDir
}
New-Item -ItemType Directory -Force -Path $extractDir | Out-Null

$zipPath = Join-Path $cacheRoot "microsoft.wsl.containers.$Version.zip"
Copy-Item -Force $nupkgPath $zipPath
Write-Host "Extracting $zipPath"
Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

$nativeDll = Join-Path $extractDir "runtimes\win-x64\native\wslcsdk.dll"
if (-not (Test-Path $nativeDll)) {
    throw "wslcsdk.dll not found at $nativeDll"
}

Copy-Item -Force $nativeDll (Join-Path $OutDir "wslcsdk.dll")
Write-Host "Staged $(Join-Path $OutDir 'wslcsdk.dll')"
Write-Host "Add to PATH for this session:"
Write-Host "  `$env:PATH = '$OutDir;' + `$env:PATH"
