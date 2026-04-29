---
title: Advanced Techniques
section: 06
---

# Advanced Techniques

## Coverage-guided fuzzing internals

Understanding how libFuzzer explores code helps you write better targets.

### Edge coverage

libFuzzer instruments every branch (conditional jump) in the compiled binary. After each test, it checks which edges were traversed. If an edge was new, the input is kept in the corpus.

This means:
- Functions with no branches (straight-line code) are covered in a single iteration.
- Functions with deep nested conditionals require exponentially more iterations.
- Functions that parse complex formats (JSON, URI, regex) benefit most from fuzzing.

### Data flow aware fuzzing (dataflow)

libFuzzer supports a dataflow mode that compares the fuzz input against an additional instrumentation that tracks which bytes influenced which comparisons. This helps the fuzzer solve comparison-based "guards":

```rust
// Without dataflow, the fuzzer has random luck to find "version"
if input == "version" {
    // deep code path that is hard to reach
}
```

```bash
cargo +nightly fuzz run target -- -data_flow_trace=1
```

This requires compiling with clang's dataflow instrumentation and is slower. Use it when a target is stuck at a comparison guard.

### Entropic power schedule

libFuzzer's default power schedule (`-entropic=true`, enabled by default) uses a feedback mechanism that assigns "energy" to corpus entries based on their usefulness. Entries that frequently lead to new paths get more mutations. This self-tunes over time and usually needs no configuration.

## Multiple fuzz targets

Organize multiple targets to test different code paths:

```
fuzz/
├── Cargo.toml
├── fuzz_targets/
│   ├── compiler.rs      # Schema compilation
│   ├── validator.rs     # Instance validation
│   ├── uri_resolver.rs  # URI resolution
│   ├── json_pointer.rs  # JSON pointer traversal
│   └── regex_checker.rs # Format regex validation
├── seeds/
│   ├── compiler/
│   ├── validator/
│   ├── uri_resolver/
│   ├── json_pointer/
│   └── regex_checker/
└── dict
```

Each target should have its own seed directory and its own corpus. A shared dictionary file is fine when targets operate on the same format.

Run all targets:

```bash
for target in compiler validator uri_resolver json_pointer regex_checker; do
    cargo +nightly fuzz run "$target" -- -max_total_time=300 &
done
wait
```

## CI integration

Add a short fuzzing step to your CI pipeline to catch regressions:

```yaml
# .github/workflows/fuzz.yml
name: Fuzz
on:
  push:
    branches: [main]
  schedule:
    - cron: '0 3 * * *'  # Daily at 3 AM UTC

jobs:
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly

      - name: Install cargo-fuzz
        run: cargo install cargo-fuzz

      - name: Fuzz builder (60s)
        run: |
          cd fuzz
          cargo +nightly fuzz run builder -- -max_total_time=60

      - name: Fuzz validation (60s)
        run: |
          cd fuzz
          cargo +nightly fuzz run validation -- -max_total_time=60
```

For nightly runs, increase the timeout to 10-30 minutes per target.

## Fuzzing no_std crates

no_std crates work with libFuzzer because the fuzz target itself is `std` — only the code under test is `no_std`. The setup is identical:

```toml
# fuzz/Cargo.toml
[dependencies]
libfuzzer-sys = "0.4"
my_no_std_crate = { path = ".." }  # Has #![no_std]
```

The fuzzer runs in a std environment; your crate runs in no_std mode. No special flags needed.

## Fuzzing with custom allocators

If your crate uses a custom allocator (`#[global_allocator]`), the fuzzer may conflict with it. The fix is to use the `dumb` allocator for fuzz builds:

```rust
#[cfg(fuzzing)]
use std::alloc::System as GlobalAllocator;

#[cfg(not(fuzzing))]
use my_custom_allocator as GlobalAllocator;
```

Or pass `--cfg fuzzing` in `RUSTFLAGS`:

```bash
RUSTFLAGS='--cfg fuzzing' cargo +nightly fuzz run target
```

## Regression testing with fuzz corpus

The fuzz corpus doubles as a regression test suite. Add a test that replays all corpus inputs:

```rust
#[test]
fn fuzz_corpus_regression() {
    let corpus_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("fuzz/corpus/builder");

    for entry in std::fs::read_dir(corpus_dir).unwrap() {
        let path = entry.unwrap().path();
        let data = std::fs::read(&path).unwrap();
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&data) {
            // Must not panic
            let _ = my_crate::compile(&json);
        }
    }
}
```

This runs in normal `cargo test`, making the corpus part of your test suite.

## Memory sanitizer (msan)

MemorySanitizer detects uses of uninitialized memory. It is the most sensitive sanitizer and catches bugs that AddressSanitizer misses:

```bash
# msan requires a clean build with msan instrumented libraries
RUSTFLAGS="-Zsanitizer=memory" cargo +nightly fuzz run target -- -runs=10000
```

msan is slower (~3x overhead) and requires nightly-only flags. Use it periodically, not on every CI run.

## Thread sanitizer (tsan)

ThreadSanitizer detects data races in multi-threaded code:

```bash
RUSTFLAGS="-Zsanitizer=thread" cargo +nightly fuzz run target -- -runs=10000
```

tsan only applies if your target uses threads. Most fuzz targets are single-threaded by default, but if your crate spawns threads internally (e.g., for parallel validation), use tsan.

## Distributed fuzzing

Run the same target on multiple machines to accelerate coverage:

```bash
# Machine 1
cargo +nightly fuzz run target -- -jobs=1 -workers=0 -max_total_time=3600

# Machine 2 (same corpus directory, sync via rsync or shared NFS)
cargo +nightly fuzz run target -- -jobs=1 -workers=0 -max_total_time=3600
```

libFuzzer supports worker mode (`-workers=N`) for local parallelization:

```bash
# Run 4 workers on one machine
cargo +nightly fuzz run target -- -jobs=4 -workers=4 -max_total_time=3600
```

Workers share a single corpus directory. They coordinate through file locking to avoid duplicate work.

## Performance tips

- **Use `-opt_level=3`**: Faster execution means more iterations per second.
  ```bash
  cargo +nightly fuzz run target -- -jobs=4 -workers=4
  ```
- **Keep targets small**: Each target should do one thing. Less code per iteration = more iterations.
- **Use seeds**: Good seeds reduce the time to reach deep code from hours to minutes.
- **Use a dictionary**: Dictionary tokens make mutations more effective, increasing the yield of each iteration.
- **Profile iterations/second**: libFuzzer prints this. If it drops below 1000/sec, check for slow code paths or expensive operations in your target.

## When NOT to fuzz

- **Pure data transformations** with no branching (e.g., base64 encoding, XOR). The code path is a straight line — one test input covers everything.
- **Well-tested combinators** with exhaustive property tests. If proptest covers the input space with invariants, fuzzing adds little.
- **GUI or network-dependent code** where the input is not a byte sequence.
- **Code with exhaustive unit tests** that cover all branches and edge cases.

---

## Checklist for advanced fuzzing

- [ ] Each code path has its own fuzz target
- [ ] Seed corpus covers all major entry points
- [ ] Dictionary includes all keywords and structural tokens
- [ ] CI runs short fuzz sessions on every push
- [ ] Nightly CI runs extended fuzz sessions (10-30 min per target)
- [ ] Crash artifacts are minimized and added as regression tests
- [ ] Corpus is periodically reviewed for redundant entries
- [ ] msan/tsan run periodically on separate CI jobs
