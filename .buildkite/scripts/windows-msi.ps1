# Build release binary and package MSI with cargo-wix on Windows MSVC.
$ErrorActionPreference = "Stop"

function Assert-Command([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command not found on PATH: $Name"
    }
}

Write-Host "=== Toolchain ==="
Assert-Command rustc
Assert-Command cargo
Assert-Command cmake

$rustcHost = (rustc -vV | Select-String "^host:").ToString()
Write-Host $rustcHost
if ($rustcHost -notmatch "windows-msvc") {
    throw "Expected host windows-msvc; got: $rustcHost"
}

# WiX (candle/light) required by cargo-wix
$hasWix = (Get-Command candle -ErrorAction SilentlyContinue) -and (Get-Command light -ErrorAction SilentlyContinue)
if (-not $hasWix) {
    Write-Host "WiX not on PATH; trying Chocolatey install of wixtoolset..."
    if (Get-Command choco -ErrorAction SilentlyContinue) {
        choco install wixtoolset -y --no-progress
        # Refresh common WiX install locations for this session
        $wixBins = @(
            "${env:ProgramFiles(x86)}\WiX Toolset v3.14\bin",
            "${env:ProgramFiles(x86)}\WiX Toolset v3.11\bin",
            "${env:ProgramFiles}\WiX Toolset v3.14\bin"
        ) | Where-Object { Test-Path $_ }
        foreach ($bin in $wixBins) {
            $env:Path = "$bin;$env:Path"
        }
    }
    Assert-Command candle
    Assert-Command light
}

Write-Host "=== cargo-wix ==="
if (-not (Get-Command cargo-wix -ErrorAction SilentlyContinue)) {
    cargo install cargo-wix --locked
}

Write-Host "=== Init wix project if missing ==="
if (-not (Test-Path "wix\main.wxs")) {
    cargo wix init --force
}

Write-Host "=== Build + package MSI ==="
cargo wix --nocapture

$msi = Get-ChildItem -Path "target\wix\*.msi" -ErrorAction SilentlyContinue
if (-not $msi) {
    throw "No MSI found under target\wix\"
}

Write-Host "MSI artifacts:"
$msi | ForEach-Object { Write-Host " - $($_.FullName) ($([math]::Round($_.Length / 1MB, 2)) MB)" }

& "$PSScriptRoot\annotate-msi-release.ps1"
