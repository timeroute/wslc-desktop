# wslc-sys

Raw Rust ABI definitions and runtime FFI boundary helpers for the preview
Microsoft WSL Container SDK (`wslcsdk.h` / `wslcsdk.dll`).

Policy:

- ABI types, constants, callback signatures, and function pointer signatures.
- Runtime loading for `wslcsdk.dll` plus checked wrappers around raw SDK calls.
- Callback trampolines, COM/CoTaskMem helpers, and Windows wait helpers stay in
  this crate so the safe `wslc` crate does not contain `unsafe` blocks.
- No SDK binary redistribution.
- Safe users should depend on `wslc`.

This crate includes layout tests for the opaque settings blobs and image info
struct, plus the low-level runtime boundary used by `wslc`.

It also mirrors `WSLCLogsFlags` bit values from the WSLC shared IDL so safe
wrappers can share the same flag constants even though the public C header does
not yet export a logs entry point.
