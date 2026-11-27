# Spec 92 Learnings

## Why this spec exists (2026-11-27)

Specs 77 and 83 merged with three kinds of debt:

1. **Proof debt.** The real Cloudflare container path was red, and no live run had used the
   owner's account. The real AWS and GCP KMS paths had never run.
2. **Fidelity debt.** The DST harness used a scripted registry, so the production registry
   code was not under simulation.
3. **Small correctness debt.** Eleven findings stayed open because each feature could land
   without them.

This spec exists to pay those debts. A feature of this spec is done when its issues are
closed, not when its code compiles.

## Feature 03: GCP KMS over gRPC (2026-11-28)

### The planned crate layout cannot work: Cargo rejects the cycle

The plan put the gRPC client in `foundation_auth`, which is the natural direction:
`foundation_auth` uses the RPC client of `foundation_connectrpc`.

Cargo rejects a package cycle even when the edge is optional and its feature is off.
`foundation_connectrpc` declares `foundation_auth` as an optional dependency behind its
`auth` feature, so the plan failed at the first `cargo check`:

```
error: cyclic package dependency: package foundation_auth
    ... which satisfies path dependency foundation_connectrpc
    ... which satisfies path dependency foundation_auth
```

A target-specific or optional declaration does not hide the edge: dependency resolution runs
over the whole graph. A dev-dependency edge is exempt, but the client is production code.

Dead ends tried: `default-features = false` (fails), a target-specific dependency table
(fails), and a renamed dependency (the cycle is by package ID, not by name).

Decision: a new crate, `foundation_gcp_kms`, above both crates. It holds the generated Cloud
KMS messages, the gRPC backend and its tests. `foundation_auth` keeps the REST backend and
gains only the `KeyStoreError::Config` variant. Issue #1107 records the cycle and the likely
fix: move the `foundation_connectrpc` auth glue out of that crate. It is not done here
because the feature brief forbids a change to `foundation_connectrpc` without a defect in the
gRPC client.

### The plan named two Amazon Web Services (AWS) calls

`GenerateDataKey` and `ScheduleKeyDeletion` do not exist in `google.cloud.kms.v1`. They are
AWS KMS names. The real calls are `CreateCryptoKey`, `GetCryptoKey`, `DestroyCryptoKeyVersion`
and `DeleteCryptoKey` (long-running). The emulator answers `unimplemented` for the two names.
The test records the exact answer. Issue #1119 corrects the plan.

### The protobuf tree needs less than expected

`protoc` ships the `google.protobuf` well-known types, and `buffa-build` maps them to
`buffa-types`. Do not vendor them: a copy is never read. Only `google/api`,
`google/longrunning` and `google/rpc` need a copy in `proto/`.

`buffa-build`'s `include_file` option builds the module tree for the generated packages, so
no hand-written `generated.rs` nesting is needed.

### localgcp does not authenticate

The emulator XORs the plaintext with the key and ignores `additional_authenticated_data`.
A wrong key returns garbage with a successful status, not an error. The wrong-key test
therefore proves the sealer's ChaCha20-Poly1305 tag, not a KMS refusal. The real service
refuses the call, and feature 04's live test asserts that. Issue #1117 records it.

### A service task needs the process ID

`StartedService` did not expose the process ID, so a task that leaves a service running
could not stop it precisely. `StartedService::pid` now has a default `None`, and the native
host returns the child's ID. The stop action signals that ID, never a program name. A
process check (`kill -0`) writes an error to standard error when the process is gone, so the
check discards its output; otherwise a normal stop looks like a failure.

### The transport has no TLS

`H2Transport` is h2c only. The real Cloud KMS gRPC endpoint needs TLS, so this path serves a
local emulator and the live path stays REST. The transport rejects an `https` endpoint as a
configuration error instead of failing later. Issue #1122 records it.

### The pinned toolchain formats more than the last author did

`cargo fmt --check` on the `feat/live-kms` head reports diffs in files that no recent change
touched, so the `foundation_ci` and `foundation_tasks` formatting jobs would fail. The base
branch formats `foundation_ci`, and this change formats `foundation_tasks` because it touches
the crate. Issue #1120 records it.
## The DST turn hang is a `Wait` re-drive (2026-11-27)

`Stream::Wait` is not `Pending`. It means "run the producer and check again". On a
multi-threaded pool another worker sends the wake. On the single-threaded simulation engine
nobody else runs, so a parked `Wait` never wakes. The layer was the stream-to-future
adapter, not the readiness harness: `sim_honours_depends_readiness` passed, and the control
test passed with a producer thread.

The fix is a signal, not a new engine. `drives_its_producer()` is `true` for a stream whose
`next()` runs the engine. The adapter re-polls such a stream with a bounded budget. A stream
that never makes progress still parks after it wakes itself. The wasm budget is 1, so the
adapter yields to the event loop at once.

Two traps:

1. A wrapper that forwards `wake`/`wake_send` but not `drives_its_producer` or
   `can_make_progress` hides the fix or the regression. Every forwarding combinator now
   forwards both.
2. A `Wait` does not always mean "run again". The single-threaded driver also returns `Wait`
   after it sleeps out its wait budget for a parked task. Re-driving that `Wait` 100 times
   cost 10 s and broke `depends_on_wake_signal_parks_then_resumes`. The driver now answers
   `can_make_progress()`, and the adapter re-drives only when the engine has runnable work.
3. `foundation_core`'s default and `multi` CI steps were already red (#1108, #1109): the
   `sim_tests` target imported a module gated on `testing`, and `SimRuntime::start` had no
   `# Panics` section. Run clippy on the crate you change, not only on your own targets.
