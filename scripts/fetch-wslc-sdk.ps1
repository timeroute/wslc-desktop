#Requires -Version 5.1
<#
.SYNOPSIS
  Download Microsoft.WSL.Containers and stage wslcsdk.dll for MSI packaging.

.DESCRIPTION
  Fetches the NuGet package and copies the win-x64 native runtime next to the
  WiX sources so `cargo wix` can embed it beside wslc-desktop.exe.
  Does not redistribute Microsoft binaries into git — run before packaging.

.PARAMETER Version
  NuGet package version. Defaults to $env:WSLC_SDK_VERSION or 2.9.3
  (the version validated by wslc-rs against the preview C SDK).

.PARAMETER OutDir
  Staging directory for the native DLL (default: wix\sdk\win-x64).
#>
param(
    [string]$Version = $(if ($env:WSLC_SDK_VERSION) { $env:WSLC_SDK_VERSION } else { "2.9.3" }),
    [string]$OutDir = ""
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
if (-not $OutDir) {
    $OutDir = Join-Path $repoRoot "wix\sdk\win-x64"
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

# .nupkg is a zip; Expand-Archive wants a .zip extension on some PowerShell versions.
$zipPath = Join-Path $cacheRoot "microsoft.wsl.containers.$Version.zip"
Copy-Item -Force $nupkgPath $zipPath
Write-Host "Extracting $zipPath"
Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

$nativeDll = Join-Path $extractDir "runtimes\win-x64\native\wslcsdk.dll"
if (-not (Test-Path $nativeDll)) {
    throw "wslcsdk.dll not found at $nativeDll"
}

$notice = Join-Path $extractDir "NOTICE.txt"
Copy-Item -Force $nativeDll (Join-Path $OutDir "wslcsdk.dll")
if (Test-Path $notice) {
    Copy-Item -Force $notice (Join-Path $OutDir "NOTICE.txt")
}

Write-Host "Staged SDK files:"
Get-ChildItem $OutDir | ForEach-Object { Write-Host "  $($_.FullName) ($($_.Length) bytes)" }
