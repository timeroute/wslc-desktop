# Vendored `wslc` 0.1.9

Upstream: <https://crates.io/crates/wslc>

Local change: `Session::open_default()` calls `WslcCreateSession` with a **null**
settings pointer so the host opens the same per-user default session as the
`wslc` CLI (`wslc-cli-<user>`). Reserved CLI names cannot be passed explicitly
from the SDK.
