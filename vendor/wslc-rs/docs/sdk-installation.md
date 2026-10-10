# Installing the WSLC SDK

This project loads Microsoft's preview WSLC SDK (`wslcsdk.dll`) at runtime. The
Rust crates do not redistribute Microsoft binaries, so a Windows host that calls
real WSLC APIs must install WSL and make the SDK native directory available on
`PATH`.

Official entry points:

- [WSL developer documentation](https://wsl.dev/)
- [WSL development loop](https://wsl.dev/dev-loop/)
- [WSL container API reference](https://wsl.dev/api-reference/)
- [Microsoft.WSL.Containers NuGet package](https://www.nuget.org/packages/Microsoft.WSL.Containers)
- [WSL container overview](https://learn.microsoft.com/en-us/windows/wsl/wsl-container)

## Prerequisites

Run these commands in an elevated PowerShell session if WSL is not already
installed or current:

```powershell
wsl --install --no-distribution
wsl --update
wsl --status
```

After installation or update, restart Windows if WSL asks you to do so.

## Download the SDK

Install the preview SDK package with NuGet. The repository CI currently uses
`2.9.3`; change the version when validating against a newer preview.

```powershell
$env:WSLC_SDK_VERSION = "2.9.3"
$env:WSLC_SDK_ROOT = "$PWD\.wslc-sdk"

nuget install Microsoft.WSL.Containers `
  -Version $env:WSLC_SDK_VERSION `
  -OutputDirectory $env:WSLC_SDK_ROOT `
  -DirectDownload `
  -NonInteractive

$packageDir = Join-Path $env:WSLC_SDK_ROOT "Microsoft.WSL.Containers.$($env:WSLC_SDK_VERSION)"
$nativeDir = Join-Path $packageDir "runtimes\win-x64\native"

if (-not (Test-Path (Join-Path $nativeDir "wslcsdk.dll"))) {
  throw "wslcsdk.dll was not found under $nativeDir"
}

$env:PATH = "$nativeDir;$env:PATH"
```

To make the SDK available in future terminals, add `$nativeDir` to the user or
machine `PATH`.

## Add the Rust Crate

For a normal application, depend on the safe wrapper:

```powershell
cargo add wslc
```

Only depend on `wslc-sys` directly when you need the raw ABI surface or are
building another wrapper.

## Smoke Tests

First check that the Rust workspace builds without calling the SDK:

```powershell
cargo test --workspace
```

Then run a host-safe WSLC probe:

```powershell
cargo test -p wslc --features integration --test integration_smoke
```

For the real registry/container path, configure a Docker Hub mirror if your
network needs one, then run the ignored Alpine test:

```powershell
$env:WSLC_REGISTRY_MIRROR = "<your-registry>"
cargo test -p wslc --features integration --test integration_smoke -- --ignored --nocapture
```

The mirror value is a registry host such as `mirror.example.com`, not an
`https://` URL. The SDK also supports per-registry mirror variables such as
`WSLC_REGISTRY_MIRROR_GHCR_IO`.

## Example Programs

The safe crate includes small examples for validating an installed SDK:

```powershell
cargo run -p wslc --example service_info
cargo run -p wslc --example list_images
cargo run -p wslc --example hello
cargo run -p wslc --example container_inspect
cargo run -p wslc --example vhd_volume
```

`hello` and `container_inspect` may pull an image. Set `WSLC_REGISTRY_MIRROR`
before running them when direct Docker Hub access is unavailable.

## GitHub Actions

GitHub-hosted Windows runners can install `Microsoft.WSL.Containers` and compile
the integration tests, but they may report
`MissingComponents(ComponentFlags(WSL_PACKAGE))` because the hosted image does
not expose the full WSLC runtime environment. Use the manual `WSLC network E2E`
workflow on a self-hosted Windows runner with WSL installed for the full Alpine
pull-and-run test.

## Troubleshooting

- `SdkNotFound`: `wslcsdk.dll` is not on `PATH`, or the SDK package was not
  downloaded.
- `MissingComponents(ComponentFlags(WSL_PACKAGE))`: update or install WSL on the
  host, then restart if required.
- Registry pull failures: set `WSLC_REGISTRY_MIRROR` or the matching
  per-registry mirror environment variable.
- Preview SDK mismatch: update the NuGet package version and rerun the
  integration tests.
