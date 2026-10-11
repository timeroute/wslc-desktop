#Requires -Version 5.1
<#
.SYNOPSIS
  Authenticode-sign Windows binaries / MSI with signtool (SHA-256 + RFC3161 timestamp).

.DESCRIPTION
  Expects a PFX on disk (from CI secrets) and Windows SDK signtool.exe.
  Retries timestamping a few times — TSA endpoints are occasionally flaky.

.PARAMETER Path
  One or more .exe / .msi files to sign.

.PARAMETER PfxPath
  Path to the code-signing PFX. Defaults to $env:CODESIGN_PFX.

.PARAMETER PfxPassword
  PFX password. Defaults to $env:CODESIGN_PASSWORD / WINDOWS_CODESIGN_PASSWORD.

.PARAMETER TimestampUrl
  RFC3161 timestamp URL. Defaults to DigiCert.
#>
param(
    [Parameter(Mandatory = $true)]
    [string[]] $Path,

    [string] $PfxPath = $env:CODESIGN_PFX,

    [string] $PfxPassword = $(
        if ($env:CODESIGN_PASSWORD) { $env:CODESIGN_PASSWORD }
        else { $env:WINDOWS_CODESIGN_PASSWORD }
    ),

    [string] $TimestampUrl = $(
        if ($env:WINDOWS_CODESIGN_TIMESTAMP_URL) { $env:WINDOWS_CODESIGN_TIMESTAMP_URL }
        else { 'http://timestamp.digicert.com' }
    )
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Find-SignTool {
    $cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }

    $kitRoot = "${env:ProgramFiles(x86)}\Windows Kits\10\bin"
    if (-not (Test-Path $kitRoot)) {
        throw "signtool.exe not found (Windows SDK missing under '$kitRoot')"
    }
    $candidate = Get-ChildItem -Path $kitRoot -Recurse -Filter signtool.exe -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\x64\\signtool\.exe$' } |
        Sort-Object FullName -Descending |
        Select-Object -First 1
    if (-not $candidate) {
        throw 'signtool.exe not found under Windows Kits\10\bin'
    }
    return $candidate.FullName
}

if ([string]::IsNullOrWhiteSpace($PfxPath) -or -not (Test-Path -LiteralPath $PfxPath)) {
    throw "PFX not found. Set -PfxPath or CODESIGN_PFX (got '$PfxPath')."
}
if ($null -eq $PfxPassword) {
    throw 'PFX password missing. Set -PfxPassword or CODESIGN_PASSWORD / WINDOWS_CODESIGN_PASSWORD.'
}

$files = @()
foreach ($p in $Path) {
    if (-not (Test-Path -LiteralPath $p)) {
        throw "File to sign not found: $p"
    }
    $files += (Resolve-Path -LiteralPath $p).Path
}

$signtool = Find-SignTool
Write-Host "Using signtool: $signtool"
Write-Host "Timestamp URL: $TimestampUrl"

foreach ($file in $files) {
    Write-Host "Signing $file"
    $ok = $false
    foreach ($attempt in 1..4) {
        & $signtool sign `
            /fd SHA256 `
            /td SHA256 `
            /tr $TimestampUrl `
            /f $PfxPath `
            /p $PfxPassword `
            /d 'WSLc Desktop' `
            $file
        if ($LASTEXITCODE -eq 0) {
            $ok = $true
            break
        }
        Write-Warning "signtool failed (exit $LASTEXITCODE), attempt $attempt/4"
        Start-Sleep -Seconds (10 * $attempt)
    }
    if (-not $ok) {
        throw "Failed to sign: $file"
    }

    & $signtool verify /pa /v $file
    if ($LASTEXITCODE -ne 0) {
        throw "signtool verify failed: $file"
    }
    $sig = Get-AuthenticodeSignature -FilePath $file
    if ($sig.Status -ne 'Valid') {
        throw "Authenticode status for $file is $($sig.Status)"
    }
    if ($null -eq $sig.TimeStamperCertificate) {
        throw "No timestamp countersignature on $file"
    }
    Write-Host "Verified: $file (signer=$($sig.SignerCertificate.Subject))"
}

Write-Host 'All files signed and verified.'
