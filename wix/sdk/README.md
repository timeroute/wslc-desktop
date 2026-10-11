# SDK notes

The MSI does **not** embed `wslcsdk.dll`. At runtime the app:

1. Reads the WSL package version from `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss\Msi\Version`
2. Picks a matching [Microsoft.WSL.Containers](https://www.nuget.org/packages/Microsoft.WSL.Containers) release
3. Caches `wslcsdk.dll` under `%LOCALAPPDATA%\WSLcDesktop\sdk\`

Optional offline helper for developers:

```powershell
.\scripts\fetch-wslc-sdk.ps1
$env:PATH = "$(Resolve-Path .\.wslc-sdk\native);$env:PATH"
```
