---
title: Setup and Tooling
section: 01
---

# Setup and Tooling

## Prerequisites

Fuzzing with libFuzzer requires the nightly Rust compiler because libFuzzer is not part of the standard library and needs unstable flags for instrumentation.

```bash
# Install the nightly toolchain
rustup toolchain install nightly

# Add the rust-src component (needed by cargo-fuzz)
rustup component add rust-src --toolchain nightly
```

If your project uses a `rust-toolchain.toml` file, you can add nightly as an override or create a fuzz-specific toolchain file inside the `fuzz/` directory.

## Installing cargo-fuzz

```bash
cargo install cargo-fuzz
```

Verify the installation:

```bash
cargo fuzz --version
```

## Initializing a fuzz target in an existing crate

From your crate root (the directory containing `Cargo.toml`):

```bash
cargo fuzz init
```

This creates:

```
fuzz/
├── Cargo.toml
├── fuzz_targets/
│   └── my_fuzz_target.rs
└── corpus/
    └── my_fuzz_target/
```

The `fuzz/Cargo.toml` will look like:

```toml
[package]
name = "my_crate-fuzz"
version = "0.0.0"
publish = false
edition = "2021"

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
my_crate = { path = ".." }

[[bin]]
name = "my_fuzz_target"
path = "fuzz_targets/my_fuzz_target.rs"
```

## Standalone fuzz project structure

For larger crates, or when the fuzz target needs its own dependencies, create a standalone fuzz project:

```
my_crate/
├── Cargo.toml
├── src/
│   └── lib.rs
└── fuzz/
    ├── Cargo.toml          # Separate Cargo project
    ├── Cargo.lock          # Independent lockfile
    ├── fuzz_targets/
    │   ├── builder.rs
    │   ├── validation.rs
    │   └── parser.rs
    ├── seeds/
    │   ├── builder/
    │   ├── validation/
    │   └── parser/
    └── dict                # Dictionary file
```

The standalone `fuzz/Cargo.toml`:

```toml
[package]
name = "my_crate-fuzz"
version = "0.0.0"
publish = false
edition = "2021"

[workspace]          # Important: isolates from parent workspace

[package.metadata]
cargo-fuzz = true

[dependencies]
libfuzzer-sys = "0.4"
serde_json = "1.0"
my_crate = { path = ".." }

[[bin]]
name = "builder"
path = "fuzz_targets/builder.rs"

[[bin]]
name = "validation"
path = "fuzz_targets/validation.rs"
```

## Running the fuzzer

```bash
# Enter the fuzz directory
cd fuzz

# Run a specific target
cargo +nightly fuzz run my_fuzz_target

# Run with a timeout (stops after 60 seconds)
cargo +nightly fuzz run my_fuzz_target -- -max_total_time=60

# Run with address sanitizer
cargo +nightly fuzz run my_fuzz_target -- -sanitizer=address

# Run with memory sanitizer (slower, catches use-after-free)
cargo +nightly fuzz run my_fuzz_target -- -sanitizer=memory

# Run with thread sanitizer (catches data races)
cargo +nightly fuzz run my_fuzz_target -- -sanitizer=thread

# Use the TUI for interactive monitoring
cargo +nightly fuzz tui my_fuzz_target
```

## Understanding the output

When the fuzzer starts, you will see:

```
INFO: Running with entropic power schedule (0xFF, 100).
INFO: Seed        : 42
INFO: Loaded      : 1 modules (17 guards)
INFO: -max_len is not provided; libFuzzer will deterministically generate inputs
#0      READ    units: 1
#1      INIT    units: 1
#2      NEW     units: 1  L: 52/52 MS: 2 ChangeBit-CrossOver-
#5      NEW     units: 2  L: 8/52 MS: 1 CrossOver-
#12     pulse   units: 2
...
```

Key fields:

- **READ** — initial corpus was read
- **INIT** — initial corpus was processed
- **NEW** — a new unique code path was found (input added to corpus)
- **pulse** — periodic status update with current corpus count
- **DONE** — test complete (timeout reached or `max_total_time` exceeded)

## Stopping the fuzzer

- **Ctrl+C** — sends SIGINT. libFuzzer prints statistics and saves the corpus before exiting.
- **`-max_total_time=N`** — stops automatically after N seconds.
- **`-runs=N`** — stops after N iterations.

The fuzzer always writes its corpus to disk before exiting, so Ctrl+C is safe.

---

## Sanitizer comparison

| Sanitizer | What it catches | Overhead | When to use |
|-----------|----------------|----------|-------------|
| Address (asan) | Buffer overflows, use-after-free, stack-use-after-scope | ~2x | Default choice |
| Memory (msan) | Uninitialized memory reads | ~3x | Deep investigation |
| Thread (tsan) | Data races | ~5-15x | Multi-threaded targets |
| Undefined (ubsan) | UB per Rust spec (overflow, null deref) | ~1x | Complementary to asan |

---

## Common issues

**"command cargo-fuzz requires a nightly toolchain"**

Run with `cargo +nightly fuzz ...` or set `RUSTUP_TOOLCHAIN=nightly`.

**"no fuzz targets found"**

Check that `fuzz_targets/*.rs` exists and `Cargo.toml` has `[[bin]]` entries for each.

**"linker 'cc' not found" or "lld not found"**

Install LLVM/Clang: `sudo apt install lld clang` (Debian/Ubuntu) or `brew install llvm` (macOS).

**Build is extremely slow**

Fuzz builds compile libFuzzer instrumentation into every crate dependency. This is normal — the first build takes longer than subsequent incremental builds.
