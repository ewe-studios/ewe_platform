---
title: Interpreting Results
section: 05
---

# Interpreting Results

## When the fuzzer crashes

When libFuzzer detects a crash, panic, or sanitizer violation, it:

1. Prints the crash input to stderr (base64-encoded if binary)
2. Saves the reproducer to `fuzz/artifacts/<target_name>/CRASH-*`
3. Saves a stats file to `fuzz/artifacts/<target_name>/stats.txt`
4. Exits with code 77 (SIGABRT)

```
ERROR: found a crash, saving in 'fuzz/artifacts/builder/Crash-abcdef1234567890'
NOTE: libFuzzer has rudimentary symbolization support.
      To improve the output, please compile with -fsanitize=address
      and link with libFuzzer's runtime:
        -fsanitize=fuzzer
==42== ERROR: libFuzzer: deadly signal
    #0 0x... in __rust_start_panic
    #1 0x... in std::panicking::panic_impl
    #2 0x... in my_crate::keywords::properties::validate
    ...
```

## Triage a crash

### Step 1: Reproduce

The saved artifact is a raw byte file. Reproduce the crash:

```bash
cargo +nightly fuzz run builder fuzz/artifacts/builder/Crash-abcdef1234567890
```

### Step 2: Minimize

Shrink the input to the smallest form that still triggers the crash:

```bash
cargo +nightly fuzz run builder -- fuzz/artifacts/builder/Crash-abcdef1234567890 -minimize_crash=1
```

This iteratively removes bytes while keeping the crash. The minimized version is easier to understand and makes for a better regression test.

### Step 3: Read the input

If the target expects JSON, the artifact is a JSON file:

```bash
cat fuzz/artifacts/builder/Crash-abcdef1234567890
```

If it is binary, inspect it:

```bash
xxd fuzz/artifacts/builder/Crash-abcdef1234567890 | head -20
```

### Step 4: Understand the panic

Check the backtrace printed during the crash. The top frame usually points to the exact line that panicked. For example:

```
thread 'main' panicked at 'index out of bounds: the len is 0 but the index is 0'
  at backends/my_crate/src/keywords/properties.rs:42
```

This tells you:
- **What**: index out of bounds
- **Where**: `properties.rs:42`
- **Why**: accessing element 0 of an empty array

### Step 5: Fix

Write the fix. Add a test using the minimized input:

```rust
#[test]
fn regression_crash_abcdef1234567890() {
    let input = br#"{"type": "object", "properties": null}"#;
    let schema: Value = serde_json::from_slice(input).unwrap();
    // This should not panic
    let _ = my_crate::compile(&schema);
}
```

## When there is no crash but the fuzzer found "something"

Sometimes the fuzzer reports issues without crashing:

- **Timeout** (`-timeout=N`): A single input took longer than N seconds. May indicate a hang or exponential backtracking.
- **OOM** (Out of Memory): The fuzzer ran out of memory processing an input. May indicate unbounded recursion or allocation.
- **Sanitizer violations**: AddressSanitizer or UndefinedBehaviorSanitizer detected issues that libFuzzer considers fatal even though the process did not panic.

All of these save an artifact in `fuzz/artifacts/` for triage.

## Understanding fuzzer statistics

After a fuzzing run, libFuzzer prints a summary:

```
stat::number_of_executed_units: 1234567
stat::total_number_of_executed_units: 1234567
stat::slowest_unit_to_run: 2ms
stat::peak_rss_mb: 45
stat::number_of_favours_added: 12
stat::number_of_favours_merged: 3
stat::number_of_removed: 0
stat::number_of_merged: 156
stat::edges_covered: 892
stat::edges_total: 1234
stat::number_of_artifacts: 0
```

Key metrics:

| Metric | Meaning |
|--------|---------|
| `number_of_executed_units` | Total inputs tested during this run |
| `edges_covered / edges_total` | Branch coverage — higher is better |
| `number_of_favours_added` | New unique paths discovered |
| `peak_rss_mb` | Peak memory usage (sanity check for leaks) |
| `number_of_artifacts` | Crashes found (should be 0 in a clean run) |
| `slowest_unit_to_run` | Longest single input (watch for outliers) |

## Corpus growth over time

Plot corpus size vs. time to assess effectiveness:

- **Steep growth early**: Good. The fuzzer is discovering paths quickly.
- **Plateau after 30 minutes**: Coverage is saturating. Either the target is well-tested, or you need better seeds/dictionary.
- **No growth**: The target is either trivial or the input format is wrong (fuzzer generates inputs that always fail parsing).

## Debugging fuzz targets

### Run with `-verbosity=1` for verbose output

```bash
cargo +nightly fuzz run builder -- -verbosity=1
```

This prints each new input as it is discovered, along with which new edges were covered.

### Run with `-print_final_stats=1` for detailed statistics

```bash
cargo +nightly fuzz run builder -- -print_final_stats=1 -max_total_time=60
```

### Use `-runs=N` to replay corpus inputs

```bash
cargo +nightly fuzz run builder -- -runs=1000
```

Runs exactly 1000 iterations without generating new inputs, useful for regression testing.

### Enable backtraces

Set `RUST_BACKTRACE=1` to get full Rust backtraces on panic:

```bash
RUST_BACKTRACE=1 cargo +nightly fuzz run builder
```

---

## Checklist for crash triage

- [ ] Reproduce the crash with the saved artifact
- [ ] Minimize the input
- [ ] Read and understand the minimized input
- [ ] Read the backtrace to find the panic location
- [ ] Identify the root cause in the source code
- [ ] Write the fix
- [ ] Add the minimized input as a regression test
- [ ] Re-run the fuzzer to verify the fix
- [ ] Commit the artifact as a test case (optional)
