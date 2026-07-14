# Feature 09 — Config, `wireguard!` Macro & Builder

**Depends on:** 04
**Decisions:** [13](../../decisions/13-tri-config-and-macro.md) (proposed), [04](../../decisions/04-bootstrap-token-envelope.md), [12](../../decisions/12-ipam-addressing.md) (proposed)
**Status:** ⚠️ Partial (2026-07-14). Tasks 1,3,4,6 done: `WgConfig` + builder + serde + `from_env()`, `wireguard!` macro, `#[wireguard_main]` (supports `config = "path"` — equivalent to `clap --config`), builder/macro/TOML convergence tests. Task 2 partial: TOML loader + forgiving deserialize done; hot-reload for relay/security toggles not done. Task 5 partial: identity persistence format done; membership-snapshot VFS persistence not done. **Must do:** (1) Hot-reload for relay/security toggles; (2) Membership-snapshot VFS persistence.

## WHY

Make the crate **easy to instantiate** — the owner asked for parity with `foundation_proxy`: a config
file, a programmatic builder, and a macro, all converging on one `WgConfig`.

## WHAT

`shared::config` (types, serde, builder) + `foundation_macros::wireguard!` + `#[wireguard_main]` +
persistence:

1. `WgConfig` + sub-configs (network/node/dataplane/bootstrap/security/relay).
2. Programmatic builder.
3. `wireguard.toml` loader (`clap --config`), forgiving deserialize.
4. `wireguard!` macro (in `foundation_macros` — [feedback_macros_location]).
5. `#[wireguard_main]` entry + identity/membership persistence.

## HOW ([decision 13](../../decisions/13-tri-config-and-macro.md))

- `WgConfig` as in decision 13; secrets via `seed` (dev) / `seed_env` / `seed_file` (never a silent
  default — `feedback_no_silent_defaults`).
- Builder: `WgNode::builder().network(..).seed(..).dataplane(..).relay(..).build()?`.
- TOML: bare endpoint strings deserialize into full structs (as `foundation_proxy::BackendTarget`
  does); optional hot-reload for relay/security toggles (not identity/seed).
- `wireguard!` macro expands to a `WgConfig` literal (compile-time checked); mirrors
  `foundation_macros::proxy` structure (`src/wireguard.rs`).
- `#[wireguard_main]`: init valtron pool, build node from config/env, `join()`, hand `WgHandle` to the
  user body; hold the pool guard for the node's lifetime (documented, like `foundation_proxy`).
- **Persistence:** store the identity keypair + last-known membership snapshot for fast rejoin
  (feature 08 / 10 interplay).

## Task list

1. `WgConfig` + serde + builder; secret sourcing (inline/env/file).
2. `wireguard.toml` loader + `clap --config`; forgiving deserialize; optional hot-reload.
3. `wireguard!` macro in `foundation_macros` + re-export.
4. `#[wireguard_main]` attribute macro.
5. Identity/membership persistence.
6. Tests: the **same network** expressed via macro, builder, and TOML all produce an equal `WgConfig`
   and a working node (success criterion 7).

## Test plan / success

- Macro / builder / TOML converge on one `WgConfig`.
- `#[wireguard_main]` boots a joined node with a valtron pool.
- Restart rejoins as the same identity from persistence.
