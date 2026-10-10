# Annotate the Buildkite build with MSI artifact summary.
$ErrorActionPreference = "Stop"

$msis = @(Get-ChildItem -Path "target\wix\*.msi" -ErrorAction SilentlyContinue)
if ($msis.Count -eq 0) {
    Write-Host "No MSI to annotate"
    exit 0
}

$branch = if ($env:BUILDKITE_BRANCH) { $env:BUILDKITE_BRANCH } else { "(unknown)" }
$commit = if ($env:BUILDKITE_COMMIT) { $env:BUILDKITE_COMMIT.Substring(0, [Math]::Min(12, $env:BUILDKITE_COMMIT.Length)) } else { "(unknown)" }
$tag = $env:BUILDKITE_TAG

$lines = @()
$lines += "### Windows MSI ready"
$lines += ""
$lines += "- Branch: ``$branch``"
$lines += "- Commit: ``$commit``"
if ($tag) {
    $lines += "- Tag: ``$tag``"
}
$lines += ""
$lines += "| Artifact | Size |"
$lines += "|----------|------|"
foreach ($m in $msis) {
    $mb = [math]::Round($m.Length / 1MB, 2)
    $lines += "| ``$($m.Name)`` | ${mb} MB |"
}
$lines += ""
$lines += "Download from this build's **Artifacts** tab (``target/wix/*.msi``)."

$body = ($lines -join "`n") + "`n"
$tmp = Join-Path $env:TEMP "wslc-msi-annotate.md"
Set-Content -Path $tmp -Value $body -Encoding utf8

if (Get-Command buildkite-agent -ErrorAction SilentlyContinue) {
    & buildkite-agent annotate --style "success" --context "windows-msi" --file $tmp
} else {
    Write-Host $body
}
