# FFI and Safety Boundary

## Principle

`wslc-sys` is the unsafe boundary. The safe `wslc` crate must not contain
`unsafe` blocks or `extern "system"` declarations.

The boundary is split like this:

- `wslc-sys/src/lib.rs`: ABI-compatible types, constants, callback signatures,
  and function pointer signatures from `wslcsdk.h`.
- `wslc-sys/src/runtime.rs`: dynamic loading of `wslcsdk.dll`, COM helpers,
  `CoTaskMemFree`, HRESULT conversion, Windows handle waiting, SDK callback
  trampolines, and checked wrappers around raw SDK calls.
- `wslc/src`: input validation, owned Rust types, builders, RAII ownership, and
  public safe API only.

The `wslc` test suite includes a source-level contract test that fails if
`unsafe` or `extern "system"` appears in `crates/wslc/src`.

## Responsibilities

The runtime boundary must handle:

- raw handle ownership and release order;
- HRESULT mapping while preserving the original code;
- SDK-allocated `PWSTR` / `PSTR` memory release with `CoTaskMemFree`;
- C string and UTF-16 string lifetime conversion;
- callback context lifetime and panic containment;
- process stdout/stderr buffer copying during callback execution;
- COM initialization and uninitialization;
- Windows `HANDLE` waits.

The safe crate must handle:

- rejecting invalid user input before loading or calling the SDK when possible;
- keeping COM guards alive for objects that need COM during release;
- avoiding `Send` / `Sync` promises for SDK handles until thread-safety is
  explicitly established;
- returning typed Rust errors and preserving original HRESULT values.

## Callback Rules

SDK callbacks are allowed to cross only through `wslc-sys::runtime`.

Callbacks must:

- return quickly;
- copy SDK-owned byte buffers immediately;
- catch or contain panics before returning across FFI;
- treat null context/data pointers as no-op callbacks.

Callbacks must not:

- store SDK-owned raw pointers after the callback returns;
- run blocking user logic while holding internal locks;
- let panic unwind across the FFI boundary.

## Drop Rules

`Drop` implementations in `wslc` call safe runtime wrapper methods only.
Release errors are ignored in `Drop`, because `Drop` cannot return errors and
must not panic during cleanup.

## Verification

Use these checks before release:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

On a machine with the WSLC SDK DLL on `PATH`, also run:

```powershell
cargo test -p wslc --features integration --test integration_smoke
```
