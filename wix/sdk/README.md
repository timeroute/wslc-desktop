# Bundled WSL Containers SDK

`wslcsdk.dll` is **not** committed here. Before `cargo wix`, stage the win-x64 native runtime:

```powershell
.\scripts\fetch-wslc-sdk.ps1
```

This downloads [Microsoft.WSL.Containers](https://www.nuget.org/packages/Microsoft.WSL.Containers) (default `2.9.3`, override with `$env:WSLC_SDK_VERSION`) into `wix\sdk\win-x64\`, which WiX installs beside `wslc-desktop.exe` so `LoadLibrary("wslcsdk.dll")` succeeds without a PATH entry.

GitHub Actions runs the same script before packaging.
