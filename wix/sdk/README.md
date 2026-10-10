# SDK staging (optional, local dev only)

The MSI **does not** bundle `wslcsdk.dll`. LIVE mode loads the host copy from
WSL install directories or `PATH`.

For local `cargo run` without a system SDK:

```powershell
.\scripts\fetch-wslc-sdk.ps1
$env:PATH = "$(Resolve-Path .\.wslc-sdk\native);$env:PATH"
cargo run
```
