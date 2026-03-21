# Mise Task Reference - EWE Platform

Complete reference for all mise tasks in the EWE Platform project.

---

## Quick Reference by Category

### Setup & Installation

```bash
mise run setup              # Full environment setup
mise run setup:tools        # Install Rust tools
mise run setup:wasm         # Install WASM targets
mise run setup:check        # Verify installation
```

### Building

```bash
mise run build:all          # Debug build all
mise run build:release      # Release build all
mise run build:wasm         # WASM build
mise run build:demos        # Build demo WASM
mise run build:tests        # Build WASM tests
mise run clean              # Clean build artifacts
```

### Testing

```bash
mise run test:all           # All tests
mise run test:unit          # Unit tests only
mise run test:integration   # Integration tests only
mise run test:quick         # Quick smoke test
mise run test:nostd         # foundation_nostd tests
mise run test:wasm          # WASM compatibility tests
mise run nextest            # Run via nextest/bacon
```

### Code Quality

```bash
mise run quality            # fmt + clippy + tests
mise run verify-all         # Full CI verification
mise run clippy             # Lint checks
mise run fmt                # Format code
mise run fmt:check          # Check formatting
mise run audit              # Security audit
```

### Documentation

```bash
mise run doc                # Generate docs
mise run doc:open           # Generate and open docs
mise run doc:nostd          # foundation_nostd docs
```

### Benchmarking

```bash
mise run bench              # All benchmarks
mise run bench:condvar      # CondVar benchmarks
mise run bench:nostd        # foundation_testing benchmarks
```

### Development

```bash
mise run sandbox            # Run sandbox binary
mise run bacon              # Start bacon language server
mise run publish            # Publish to crates.io
```

### Git & Submodules

```bash
mise run git:update-submodules   # Update all submodules
mise run update-submodules       # (alias)
```

---

## Detailed Task Descriptions

### Setup Tasks

#### `setup`

**Description:** Complete development environment setup

**Dependencies:** `setup:tools`, `setup:wasm`

**Usage:**
```bash
mise run setup
```

---

#### `setup:tools`

**Description:** Install rustfmt, clippy, rust-analyzer, and cargo tools

**Commands:**
- `rustup component add rustfmt clippy rust-analyzer`
- Installs cargo-nextest and cargo-audit via mise

**Usage:**
```bash
mise run setup:tools
```

---

#### `setup:wasm`

**Description:** Install WASM compilation targets

**Commands:**
- `rustup target add wasm32-unknown-unknown`
- `rustup target add wasm32-wasip1`

**Usage:**
```bash
mise run setup:wasm
```

---

#### `setup:check`

**Alias:** `check-tools`

**Description:** Verify all installed tools and versions

**Usage:**
```bash
mise run setup:check
```

---

### Build Tasks

#### `build:all`

**Alias:** `build-all`

**Description:** Build all packages in debug mode

**Command:** `cargo build --all`

**Usage:**
```bash
mise run build:all
```

---

#### `build:release`

**Alias:** `build-release`

**Description:** Build all packages in release mode

**Command:** `cargo build --all --release`

**Usage:**
```bash
mise run build:release
```

---

#### `build:wasm`

**Alias:** `build-wasm`

**Description:** Build foundation_nostd for WASM target

**Command:** `cargo build --package foundation_nostd --target wasm32-unknown-unknown`

**Usage:**
```bash
mise run build:wasm
```

---

#### `build:demos`

**Alias:** `build-demos`

**Description:** Build demo WASM binaries and generate WAT files

**Commands:**
```bash
RUSTFLAGS='-C link-arg=-s' cargo build --package intro --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/debug/intro.wasm ./assets/public/intro.wasm
wasm2wat ./assets/public/intro.wasm -o ./assets/public/intro.wat
```

**Usage:**
```bash
mise run build:demos
```

---

#### `build:tests`

**Alias:** `build-tests`

**Description:** Build all WASM integration test packages

**Usage:**
```bash
mise run build:tests
```

---

### Testing Tasks

#### `test:all`

**Alias:** `test-all`

**Description:** Run all tests (unit + integration)

**Dependencies:** `test:unit`, `test:integration`

**Usage:**
```bash
mise run test:all
```

---

#### `test:unit`

**Alias:** `test-unit`

**Description:** Run unit tests (--lib tests in all crates)

**Command:** `cargo test --lib --all`

**Usage:**
```bash
mise run test:unit
```

---

#### `test:integration`

**Alias:** `test-integration`

**Description:** Run workspace-level integration tests

**Command:** `cargo test --package ewe_platform_tests`

**Usage:**
```bash
mise run test:integration
```

---

#### `test:quick`

**Alias:** `test-quick`

**Description:** Quick smoke test for fast feedback

**Command:** `cargo test --package foundation_nostd --lib`

**Usage:**
```bash
mise run test:quick
```

---

#### `test:nostd`

**Alias:** `test-nostd`

**Description:** All foundation_nostd tests

**Dependencies:** `test:nostd:unit`, `test:nostd:integration`

**Usage:**
```bash
mise run test:nostd
```

---

#### `test:nostd:unit`

**Description:** foundation_nostd unit tests only

**Command:** `cargo test --package foundation_nostd --lib`

---

#### `test:nostd:integration`

**Description:** foundation_nostd integration tests

**Command:** `cargo test --package ewe_platform_tests --lib`

---

#### `test:nostd:std`

**Description:** Run tests with std feature enabled

**Commands:**
- `cargo test --package foundation_nostd --features std`
- `cargo test --package foundation_testing`

---

#### `test:nostd:no-std`

**Description:** Run tests in no_std mode

**Command:** `cargo test --package foundation_nostd --no-default-features`

---

#### `test:wasm`

**Alias:** `test-wasm`

**Description:** WASM compilation verification

**Dependencies:** `test:wasm:build`, `test:wasm:verify`

---

#### `test:wasm:build`

**Description:** Build for all WASM configurations

**Commands:**
- no_std debug build
- std debug build
- no_std release build

---

#### `test:wasm:verify`

**Description:** Verify WASM artifacts exist

---

#### `test:wasm:node`

**Alias:** `wasm-tests`

**Description:** Build and run all WASM tests with Node.js

**Dependencies:** `build:tests`

**Usage:**
```bash
mise run test:wasm:node
```

---

#### `test:wasm:node-single`

**Alias:** `wasm-test`

**Description:** Build and run a single WASM test

**Environment:** `TARGET_TEST` (default: `tests_callfunction`)

**Usage:**
```bash
TARGET_TEST=my_test mise run test:wasm:node-single
```

---

### Code Quality Tasks

#### `quality`

**Description:** Run all quality checks

**Dependencies:** `fmt:check`, `clippy`, `test:unit`

**Usage:**
```bash
mise run quality
```

---

#### `verify-all`

**Alias:** `verify`

**Description:** Full CI verification

**Dependencies:** `quality`, `test:all`

**Usage:**
```bash
mise run verify-all
```

---

#### `clippy`

**Description:** Run clippy with zero-warnings policy

**Command:** `cargo clippy --all-targets --all-features -- -D warnings`

**Usage:**
```bash
mise run clippy
```

---

#### `fmt`

**Alias:** `lint`

**Description:** Format all code

**Command:** `cargo fmt --all`

**Usage:**
```bash
mise run fmt
```

---

#### `fmt:check`

**Alias:** `fmt-check`

**Description:** Check if code is formatted

**Command:** `cargo fmt --all -- --check`

**Usage:**
```bash
mise run fmt:check
```

---

#### `audit`

**Description:** Run cargo security audit

**Command:** `cargo audit`

**Usage:**
```bash
mise run audit
```

---

### Documentation Tasks

#### `doc`

**Description:** Generate Rust documentation

**Command:** `cargo doc --no-deps --all-features`

**Output:** `target/doc/index.html`

---

#### `doc:open`

**Alias:** `doc-open`

**Description:** Generate and open documentation in browser

**Command:** `cargo doc --no-deps --all-features --open`

---

#### `doc:nostd`

**Alias:** `doc-nostd`

**Description:** Generate foundation_nostd documentation

**Command:** `cargo doc --package foundation_nostd --no-deps --all-features --open`

---

### Benchmarking Tasks

#### `bench`

**Alias:** `bench-all`

**Description:** Run all Criterion benchmarks

**Command:** `cargo bench`

**Results:** `target/criterion/report/index.html`

---

#### `bench:condvar`

**Alias:** `bench-condvar`

**Description:** Run CondVar-specific benchmarks

**Command:** `cargo bench --bench condvar_bench`

---

#### `bench:nostd`

**Description:** Run foundation_testing benchmarks

**Command:** `cargo bench --package foundation_testing`

---

### Development Tasks

#### `sandbox`

**Description:** Run ewe_platform sandbox binary

**Command:** `cargo run --profile dev --bin ewe_platform sandbox`

---

#### `bacon`

**Description:** Start bacon language server for IDE integration

**Command:** `bacon -j bacon-ls`

---

#### `publish`

**Description:** Publish all packages to crates.io

---

### Git Tasks

#### `git:update-submodules`

**Alias:** `update-submodules`

**Description:** Update all git submodules to latest remote

**Commands:**
```bash
git submodule update --remote .agents
git submodule update --remote tools/dawn
git submodule update --remote tools/emsdk
git submodule update --remote infrastructure/llama-bindings/llama.cpp/
```

---

### Example Tasks

#### `examples:todo:serve`

**Description:** Serve todo example on localhost:8080

**Directory:** `examples/todo`

---

#### `examples:todo:tailwind`

**Description:** Watch and rebuild Tailwind CSS

**Directory:** `examples/todo`

**Command:** `npx tailwindcss -i ./web/css/main.tailwind -o ./web/css/main.css --watch`

---

### Foundation NoStd Tasks

#### `nostd:build`

**Description:** Build foundation_nostd and foundation_testing

---

#### `nostd:clippy`

**Description:** Run clippy on foundation_nostd packages

---

#### `nostd:fmt`

**Description:** Format foundation_nostd code

---

#### `nostd:quality`

**Description:** All quality checks for foundation_nostd

**Dependencies:** `nostd:fmt-check`, `nostd:clippy`, `test:nostd:std`

---

## Task Naming Conventions

- Use colons for namespaces: `category:action`
- Use kebab-case within names: `build:all` not `build:All`
- Aliases use dashes: `build-all` maps to `build:all`

---

## Related Documentation

- [Getting Started](./getting-started.md)
- [Advanced Usage](./advanced-usage.md)
