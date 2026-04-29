---
title: Fuzz Testing in Rust — Overview
section: 00
---

# Fuzz Testing in Rust — Overview

## What is fuzz testing

Fuzz testing (fuzzing) is a dynamic analysis technique that feeds automatically generated, pseudo-random inputs into a program to discover crashes, panics, memory safety violations, and undefined behaviour. Unlike unit tests which verify known inputs produce known outputs, fuzzing explores the input space without prior knowledge of expected behaviour.

The core loop is simple:

1. Generate an input (seed, mutation, or crossover)
2. Run the target program with that input
3. If the program crashes or panics, save the input as a reproducer
4. Use coverage feedback to guide future input generation

## Why fuzz Rust code

Rust guarantees memory safety for safe code, but fuzzing remains valuable because:

- **Unsafe code exists.** Every `unsafe` block is a potential source of undefined behaviour. Fuzzing validates that unsafe code handles all inputs correctly.
- **Panics are bugs.** A panic in production code is a denial of service. Fuzzing finds inputs that trigger unexpected panics before they reach users.
- **Logic errors are language-agnostic.** Integer overflow (in release mode), division by zero, and algorithmic complexity attacks affect Rust just as they affect any language.
- **Edge cases multiply with combinators.** A JSON parser with `allOf`, `$ref`, `if/then/else` has a combinatorial input space that unit tests cannot cover exhaustively.
- **No-std and embedded code.** Fuzzing catches issues in environments where traditional testing is harder to run.

## How libFuzzer works

libFuzzer is an in-process, coverage-guided fuzzing engine built into LLVM. It works by:

- **Instrumenting the target binary.** The compiler inserts probes at every branch and basic block. These probes record which code paths are exercised.
- **Maintaining a corpus.** Each unique code path triggers a new input being added to the corpus. Inputs that reach new paths are preserved.
- **Mutating corpus entries.** The fuzzer applies byte-level mutations (bit flips, arithmetic changes, splicing from the corpus dictionary) to existing corpus entries.
- **Feedback loop.** When a mutation reaches a new code path, that input becomes a new corpus seed. Over time, the corpus grows to cover all reachable branches.

This is fundamentally more effective than black-generation (generating random inputs without feedback) because it systematically explores the decision tree of the target code.

## The `cargo-fuzz` tool

`cargo-fuzz` is the standard Rust interface to libFuzzer. It provides:

- `cargo fuzz init` — scaffolds a `fuzz/` subdirectory with `Cargo.toml`, target templates, and seed directories
- `cargo fuzz run <target>` — builds and executes a fuzz target
- `cargo fuzz list` — lists available fuzz targets
- `cargo fuzz tui <target>` — interactive terminal UI showing corpus statistics

Under the hood, `cargo-fuzz` generates a Cargo project with `libfuzzer-sys` as a dependency, configures the `[dependencies]` to point at your crate, and passes `--sanitizer=address` (or memory/thread) to the linker.

## What fuzzing is not

Fuzzing does not replace:

- **Unit tests.** Unit tests verify specific behaviours. Fuzzing verifies absence of crashes.
- **Property-based testing** (proptest, quickcheck). Those generate inputs from typed generators and check invariants. Fuzzing generates untyped byte sequences and checks for crashes. They are complementary — property tests catch logic bugs invariants, fuzzers catch memory and panic bugs.
- **Static analysis** (clippy, miri). Static tools find bugs without running code. Fuzzing finds bugs through execution. Use both.
- **Typestate or compile-time guarantees.** If your type system prevents invalid states, fuzzing becomes less critical for that code path.

---

## Further Reading

- [The FuzzBook](https://www.fuzzbook.org/) — Academic textbook on fuzzing techniques
- [libFuzzer documentation](https://llvm.org/docs/LibFuzzer.html) — LLVM upstream guide
- [cargo-fuzz repository](https://github.com/rust-fuzz/cargo-fuzz)
