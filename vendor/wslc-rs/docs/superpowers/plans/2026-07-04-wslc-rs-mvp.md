# wslc-rs MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build and publish the initial `wslc-sys` and `wslc` crates from the existing WSLC Rust API design.

**Architecture:** `wslc-sys` owns raw ABI-compatible types and constants. `wslc` owns runtime SDK loading, safe builders, input validation, HRESULT/error mapping, RAII handle release, callback-backed output capture, docs, examples, and tests.

**Tech Stack:** Rust 2021, `thiserror`, `bitflags`, `libloading`, `windows-sys`, Cargo workspace, crates.io.

---

### Task 1: Workspace and Tests

**Files:**
- Create: `Cargo.toml`
- Create: `crates/wslc-sys/Cargo.toml`
- Create: `crates/wslc-sys/src/lib.rs`
- Create: `crates/wslc/Cargo.toml`
- Create: `crates/wslc/src/lib.rs`
- Create: `crates/wslc/tests/api_contract.rs`

- [ ] Write failing API contract tests for validation, HRESULT mapping, and SDK absence.
- [ ] Run `cargo test -p wslc --test api_contract` and confirm failures.

### Task 2: Raw ABI

**Files:**
- Modify: `crates/wslc-sys/src/lib.rs`

- [ ] Define opaque settings, raw handles, constants, structs, callback types, and function pointer signatures matching the current public `wslcsdk.h`.
- [ ] Add compile-time size/alignment tests.
- [ ] Run `cargo test -p wslc-sys`.

### Task 3: Safe Wrapper Core

**Files:**
- Create: `crates/wslc/src/error.rs`
- Create: `crates/wslc/src/raw.rs`
- Create: `crates/wslc/src/strings.rs`
- Create: `crates/wslc/src/com.rs`
- Modify: `crates/wslc/src/lib.rs`

- [ ] Implement `Error`, `WslcErrorKind`, `ComponentFlags`, HRESULT checks, runtime SDK loading, COM initialization, and string conversion helpers.
- [ ] Run API contract tests until the pure validation tests pass.

### Task 4: Service, Session, Image, Container, Process

**Files:**
- Create: `crates/wslc/src/service.rs`
- Create: `crates/wslc/src/session.rs`
- Create: `crates/wslc/src/image.rs`
- Create: `crates/wslc/src/container.rs`
- Create: `crates/wslc/src/process.rs`
- Create: `crates/wslc/src/storage.rs`
- Modify: `crates/wslc/src/lib.rs`

- [ ] Implement the MVP blocking API: service checks, session lifecycle, image pull, container create/start/stop/delete, process options, output capture, and VHD option types.
- [ ] Add integration tests gated behind `--features integration`.
- [ ] Run `cargo test`.

### Task 5: Docs and Publishing

**Files:**
- Modify: `README.md`
- Modify: `crates/wslc/README.md`
- Modify: `crates/wslc-sys/README.md`
- Modify: `examples/hello.rs`

- [ ] Document runtime SDK loading, Windows host target, preview status, feature gates, and examples.
- [ ] Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo package -p wslc-sys --allow-dirty`, `cargo package -p wslc --allow-dirty`.
- [ ] Initialize git if needed, create the GitHub repository, push the code, publish `wslc-sys`, then publish `wslc`.
