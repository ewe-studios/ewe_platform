---
feature: "Remove the foundation_auth dependency from foundation_connectrpc"
status: "pending"
priority: "high"
model: "sonnet"
phase: "2"
depends_on: ["03-gcp-kms-grpc"]
crates: ["foundation_connectrpc", "foundation_connectrpc_auth", "foundation_agent_rpc", "foundation_agent_rpc_dst"]
---

# Feature 05: Remove the foundation_auth dependency from foundation_connectrpc

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations.** Authentication (auth); remote procedure call (RPC); WebAssembly (wasm).

## 1. Goal

`foundation_connectrpc` must not depend on `foundation_auth`. The two crates are layers: the
auth crate builds on the RPC crate. The current back edge makes a cycle, and the cycle blocks
any crate that needs both (#1107). It also drags wasm-bindgen glue into the `rpc` feature
(#756).

## 2. The problem

- `foundation_connectrpc/Cargo.toml:23` declares `foundation_auth` as an optional dependency,
  and line 59 again for the wasm target.
- The `auth` feature (line 105) enables it. The `rpc` feature (line 88) includes `auth`.
- `foundation_gcp_kms` needs the RPC client and `foundation_auth`. Cargo rejects the cycle
  `foundation_gcp_kms -> foundation_auth -> foundation_connectrpc -> foundation_auth`, even
  though the last edge is optional and off. Feature 03 works around this with a new crate that
  avoids the RPC client's auth types; the back edge stays.

## 3. The change

1. New crate `backends/foundation_connectrpc_auth`:
   - It depends on `foundation_connectrpc` (no `auth` feature) and `foundation_auth`.
   - It holds the current `src/shared/auth.rs` (737 lines) moved, not copied: the interceptor,
     the bearer and signed-secret providers, and the policy types.
   - It takes `tests/auth_tests.rs` and `tests/client_auth_interceptor_tests.rs` with it.
2. `foundation_connectrpc`:
   - Delete the `foundation_auth` dependency and the `auth` and wasm `auth` features.
   - Remove `auth` from the `rpc` feature set.
   - Keep `Interceptor`, `Request`, `RpcCtx` and the rest of the shared client surface; they
     do not use auth.
3. Move every caller (19 uses) to the new crate:
   - `foundation_agent_rpc/src/host/native/listener.rs`
   - `foundation_agent_rpc/src/host/worker/object.rs`
   - `foundation_agent_rpc/src/host/worker/entry.rs`
   - `foundation_agent_rpc_dst/src/lib.rs`
   - `foundation_connectrpc`'s own tests, which move.
4. Keep the crate graph acyclic and prove it.

## 4. Tests

| Test | Oracle |
|---|---|
| the moved `auth_tests` and `client_auth_interceptor_tests` | no regression |
| `cargo tree -p foundation_connectrpc -e normal` contains no `foundation_auth` | conservation |
| a crate that depends on both crates builds (the DST crate and `foundation_gcp_kms`) | the cycle is gone |
| the `rpc` feature builds on wasm32 without wasm-bindgen auth glue | #756 |

## 5. Verify

```bash
cargo tree -p foundation_connectrpc -e normal | grep foundation_auth && echo "STILL PRESENT"
cargo test -p foundation_connectrpc_auth --profile uat
cargo test -p foundation_connectrpc --profile uat
cargo test -p foundation_agent_rpc --profile uat
cargo check -p foundation_connectrpc --no-default-features --features rpc --target wasm32-unknown-unknown
cargo clippy -p foundation_connectrpc -p foundation_connectrpc_auth --all-targets -- -D warnings
cargo fmt -p foundation_connectrpc -p foundation_connectrpc_auth -- --check
```

## 6. Done when

#1107 and #756 are closed in the change that removes the dependency. The tree command prints
nothing, and every moved test passes in its new home.
