# Platform

A series of crates providing different functionality and binaries for projects.

## Test Structure

This project uses Rust's idiomatic test organization:

### Unit Tests
- Located within each crate's `src/` directory using `#[cfg(test)]`
- Run with: `cargo test -p <crate-name>`

### Integration Tests
- Located in each crate's `tests/` directory
- Tests that verify a single crate's functionality
- Run with: `cargo test -p <crate-name>`

**Key Crates with Integration Tests:**
- `backends/foundation_core/tests/` - Core HTTP, WebSocket, event source tests
- `backends/foundation_nostd/tests/` - WASM-compatible synchronization primitive tests  
- `backends/foundation_macros/tests/` - Proc-macro tests
- `backends/foundation_testing/tests/` - Testing infrastructure self-tests

### Cross-Crate Integration Tests
- Located in `tests/` directory (ewe_platform_tests crate)
- Tests that verify interaction between multiple crates
- Run with: `cargo test -p ewe_platform_tests`

### Why This Structure?

Tests were migrated from a centralized `tests/` crate into their respective crates to:
- Follow Rust best practices (tests live with the code they test)
- Eliminate circular dependency concerns
- Make dev-dependencies work correctly (dev-deps only activate when testing the declaring crate)
- Improve discoverability and maintainability

## Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p foundation_core
cargo test -p foundation_nostd
cargo test -p foundation_testing
cargo test -p foundation_macros

# With specific features
cargo test -p foundation_core --features std,multi
```

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

## Binaries

See the [bin](./bin) directory

## Crates

See the [crates](./crates) and [backend](./backends) directories.


## Project Templates

See the [templates](./templates) directory.

## Development Environments

Virtual machines for cross-platform testing and artifact builds, all managed via `docker compose`.
Each VM runs as a **profile** so you can start only what you need.

### Quick Start

```bash
# Start individual VMs
docker compose up macos -d       # macOS Sequoia (iOS build + simulator)
docker compose up android -d     # Android emulator
docker compose up windows -d     # Windows 11 VM (testing + artifacts)

# Start all at once
docker compose up -d
```

### Accessing the VMs

| VM      | Web UI                    | VNC             | RDP              |
|---------|---------------------------|-----------------|------------------|
| macOS   | http://localhost:8006/    | localhost:5900  | —                |
| Android | http://localhost:8007/    | —               | —                |
| Windows | http://localhost:8008/    | —               | localhost:3389   |

**Windows RDP credentials:** user `Docker`, password `admin`

### Requirements

- KVM support — verify with `sudo kvm-ok` (should report "KVM acceleration can be used")
- At least 4 GB RAM and 64 GB free disk per VM
- Hardware virtualization (Intel VT-x / AMD-V) enabled in BIOS

### Persistent Storage

| VM      | Disk Location                          | Shared Folder                     |
|---------|----------------------------------------|-----------------------------------|
| macOS   | `~/Boxxed/@dev/macos/`                 | project root → `/shared`          |
| Android | `./android_shared/`                    | `./android_shared/shared`         |
| Windows | `~/Boxxed/@dev/windows/`               | project root → `Shared` on desktop|

### Notes

- First boot takes 10–15 minutes — base images are downloaded automatically.
- Use `docker compose stop <vm>` to pause; `docker compose down <vm>` to remove.
