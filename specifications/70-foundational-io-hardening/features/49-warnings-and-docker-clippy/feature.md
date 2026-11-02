---
feature: "Lane A warnings: reactor wasm dead code, testbed deno_v8 lint, foundation_deployment_docker clippy debt"
status: "pending"
lane: "A"
model: "haiku"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_reactor", "foundation_testbed", "foundation_deployment_docker", "foundation_connectrpc", "foundation_netio", "foundation_deployment", "foundation_keychain"]
catalogue_rows: [13, 14, 15, 31]
---

# Feature 49: warnings and clippy debt in lane A

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## E1. `Poll::rearm_readable` is dead on wasm (backlog §16)

`foundation_reactor/src/poll/mod.rs` ~323 is `pub(crate)`, called only from a native path
(`reactor.rs` ~607). Gate the method with the same `cfg` as its call site.
Verify: `cargo check --tests --target wasm32-unknown-unknown -p foundation_netio 2>&1 | rg rearm_readable` is empty.

## E2. `deno_v8` unused-dependency lint in `foundation_testbed` (backlog §16)

It is listed only to pick `deno_core`'s V8 backend. Add `use deno_v8 as _;` in the
embedded JS module under the same feature, with a comment that says why. Verify with the
lint that reported it (cargo nightly `unused_dependencies`) if available, else
`cargo check -p foundation_testbed --features wasm-embedded-js` without warnings.

## E3. `foundation_deployment_docker` clippy debt (backlog §8, ~231 lints)

Fix all clippy warnings in this crate (default features and `--all-features`). Mechanical
fixes only; no behaviour change. Then add the crate's `--all-features` clippy run to
`ci/packages.toml` if the CI does not lint it, so it stays at zero. Remove the docker part of
the §8 entry (lane B removes the rest).

## E4. `foundation_connectrpc` clippy debt (backlog §1)

`cargo clippy -p foundation_connectrpc --profile uat --all-targets --all-features` reports
`doc_markdown` (most), cast lints in `tests/` and `src/shared/transport/conn.rs`,
`src/shared/client.rs`, and `needless_pass_by_value`, `must_use`, `struct_excessive_bools`,
`similar_names`, `unused_async` in examples. Fix all; mechanical only (a cast becomes
`try_from` with the error handled, or carries an `#[allow]` with a one-line reason where the
value is proven in range). Delete the §1 entry. Verify the same command prints 0 warnings.

## E5. `foundation_netio` and `foundation_deployment` warnings (seen in feature 42)

- `foundation_netio`: the unused `rustls-pemfile` dependency and the remaining lib clippy
  warnings (`cargo clippy -p foundation_netio --profile dev --all-targets`). Remove the
  dependency if nothing uses it (check every feature), fix the lints.
- `foundation_deployment`: "unused `self` argument" (clippy). Fix it.
Verify both crates' clippy print 0 warnings.

## E6. `foundation_keychain` clippy debt (found in feature 44)

174 lib warnings (for example `manual_let_else` in `provisioning_route`) plus test warnings.
Fix all; mechanical only. Delete the §22 clippy entry.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo clippy -p foundation_deployment_docker --profile dev --all-targets --all-features 2>&1 | rg -c '^warning'   # 0
python3 scripts/ci/ci.py run foundation_deployment_docker --profile dev
python3 scripts/ci/ci.py run foundation_reactor --profile dev
python3 scripts/ci/ci.py run foundation_testbed --profile dev
python3 scripts/ci/ci.py run foundation_connectrpc --profile dev
```

## Done when

- [ ] The three warnings sources are at zero; backlog entries (or their docker part) deleted.
