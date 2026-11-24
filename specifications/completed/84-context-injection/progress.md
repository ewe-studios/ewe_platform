# Spec 84 Progress: Context Injection

_Last updated: 2026-11-23._

**Completed.** All tasks are done. Merged into master as `ca83b5115` (#679).

## Status at a glance

| Task | State |
|---|---|
| T1 — `Timer`, `EntropySource`, `IdSource` traits and their implementations | ✅ Done |
| T2 — `Env`, `EnvError`, `Ctx::env`, `Ctx::require_env`; the host builders; `ValtronTimer` | ✅ Done |
| T3 — `#[valtron(env = …)]`, `#[valtron_test(env = …, seed = …)]`, `single::initialize_pool_with` | ✅ Done |
| T4 — The effect lint in the core crates | ✅ Done, with one recorded exception |
| T5 — `foundation_ai` reads IDs and time through `Env` | ✅ Done for the agentic layer |
| T6 — The DST replay test and harness | ✅ Done |

## What landed

### T1 — the effect traits (`foundation_context`)

The new module `foundation_context::env` holds:

- `Timer` and `TimerFuture`, the wait effect. `sleep_until` takes a `Ctx`, so the
  timer reads no clock and an `Env` cannot close an `Arc` cycle.
- `EntropySource`, `OsEntropy` and `SeededEntropy`, the random-byte effect.
- `IdSource`, `Scru128Source` and `CounterSource`, the identifier effect.
  `Scru128Source` builds `foundation_compact::ids::Generator` from the injected
  clock and entropy. A simulated clock at the UNIX epoch yields timestamp 0,
  which scru128 rejects, so the adapter clamps it to 1.

`Ctx` gains `env`, `require_env` and a hidden `strong_count` for the lifecycle
test. The crate version is 0.0.4 and every consumer requirement moved with it.

### T2 — the environment and the host builders (`foundation_core`)

`foundation_core::env` builds `system`, `wasm`, `simulated` and `install`, and
holds `ValtronTimer`. `ValtronTimer` waits through `sleep_until_async`, which
now exists in both executor builds. On wasm the single-threaded task future is
wrapped in `foundation_compact::SendWrapper`, because the `Timer` trait keeps
the `Send` bound on every target (spec 84, section 6.6).

`single::SingleConfig`, `single::initialize_pool_with` and `single::root_ctx`
exist. The plain `initialize_pool` and `initialize_current_thread` install the
host effects. The crate version is 0.0.22.

### T3 — the macro argument (`foundation_macros`)

`#[valtron]` and `#[valtron_test]` accept `env = <expression>`. The expression
gives an `Env` or a `Result<Env, E>`. The macro installs it in the root `Ctx`
before the pool starts. Without the argument, a test gets
`foundation_core::env::simulated` from its `seed`, and an entry point gets
`foundation_core::env::system`. The crate version is 0.0.23.

**Decision recorded:** the requirement 5.6 text also describes `env = name` as
a binding name. The design across specs 82, 83 and `env.md` uses an expression
(for example `env = Env::system()`). The expression form is the one implemented.
The test-side "simulated from the seed" behaviour is met by the default, so
`#[valtron_test(seed = 7)]` needs no `env` argument.

### T4 — the effect lint

A `clippy.toml` with the five disallowed global calls exists in
`foundation_core`, `foundation_auth`, `foundation_connectrpc`, `foundation_http`
and `foundation_ai`. The planned `foundation_platform_sandbox` crate does not
exist yet (spec 77), so it has no file. The `no_global_clock_in_core` test in
`foundation_core/tests` proves the configuration in the five crates.

The test exemption of section 6, item 2 is a file-level
`#![allow(clippy::disallowed_methods)]` in each test file that builds an effect
on purpose. An integration test belongs to its package, so it shares the package
`clippy.toml`; a nested `clippy.toml` would not apply.

### T5 — `foundation_ai`

`SessionId::from_id` exists. The agent loop, the memory hierarchy, the
embedding provider, the agent tool and the memory tool read IDs from
`Ctx::require_env`. A missing `Env` becomes an `AgenticError::Unexpected`
`FailedAction` record, never a global fallback. `with_text_protocol_tool_calls`
and `vacuous_answer_redirect` return their `EnvError` to the loop.

The test `the_loop_mints_ids_from_the_env_and_repeats_them` runs one turn
twice under a counter `Env` and gets the same consumption.

The crate version is 0.0.10.

### T6 — the DST replay

`foundation_testing::dst::replay` builds a simulated `Env` and records its IDs
and waits. `foundation_testing/tests/dst_replay.rs` runs two child processes of
its own binary and compares the traces. `foundation_core/tests/dst_replay.rs`
runs the same law in one process with a hand-moved clock. The crate version is
0.0.8.

## Known gaps and follow-ups

- **Provider backends (#677).** `anthropic_messages_provider`, `openai_provider`,
  `openai_responses_provider` and `workers_ai` still call the global scru128
  generator. Each file carries a scoped `#[allow(clippy::disallowed_methods)]`
  with a comment. `MessageApi::append`, `fresh_message_id` and
  `fresh_record_id` have the same allowance, because they have no `Ctx` and no
  `Env`. Issue #677 has the fix plan.
- **`foundation_platform_sandbox`.** Spec 84 names this crate in T4, but spec 77
  has not created it. No clippy file exists until the crate does.
- **Wasm test execution.** `cargo test -p foundation_context --target
  wasm32-unknown-unknown` builds the test binary but no wasm runner is
  configured, so the binary cannot execute here. The wasm gate in CI builds the
  crate (`wasm = "enforce"`).
- **Default-feature build of `foundation_ai`.** The checks and tests ran with
  `--no-default-features --features testing`. The default feature set builds
  llama.cpp and Candle, which need a long native build and a large disk budget.
  The skill and provider code paths that this change touches compile in the
  `testing` set.

## Next steps

1. Pick up issue #677 for the provider backends and the serde defaults.
2. When spec 77 creates `foundation_platform_sandbox`, add its `clippy.toml` and
   add the crate to `no_global_clock_in_core`.
3. Add a wasm-bindgen test runner to the CI image if the team wants the
   `wasm32-unknown-unknown` test command from section 10 to execute.
