# Exploration: fnox — Secret Management with Encryption & Cloud Providers

**Source:** Jeff Dickey / @jdx ([github.com/jdx/fnox](https://github.com/jdx/fnox))
**Crate:** `fnox` v1.23.0 · MIT · Cross-platform (Linux, macOS, Windows)
**Tagline:** "Fort Knox for your secrets"

---

## Summary

Fnox provides a unified interface for managing secrets across development, CI, and production. It supports two orthogonal approaches: **encrypted-in-git** (secrets committed as ciphertext, decrypted locally) and **remote cloud providers** (secrets stored in AWS, Azure, GCP, Vault, etc.). The config file (`fnox.toml`) contains either encrypted blobs or references to remote secrets — and `fnox exec` resolves everything into environment variables before spawning a child process.

The key innovation is the **provider-agnostic model**: any secret can be sourced from any provider (age encryption, AWS KMS, 1Password, Bitwarden, etc.) and the resolution logic is identical across all of them. Profiles allow different providers per environment (dev vs. staging vs. production).

---

## Core Architecture

### Resolution Pipeline

```
fnox.toml                              fnox exec -- npm start
───────────                            ──────────────────────
[providers]
age = { type = "age",
  recipients = ["age1..."] }           1. Load & merge config (recursive)
                                       2. Build dependency graph (Kahn's algo)
[secrets]                              3. Resolve level 0 (no deps):
DATABASE_URL = {                         - age-encrypted secrets → decrypt
  provider = "age",                      - default values → use as-is
  value = "YWdlLWVu..." }               - env vars → read from parent
API_KEY = {                            4. Set resolved env vars
  provider = "aws-sm",                    → 1Password SDK can now read them
  value = "myapp/api-key" }            5. Resolve level 1 (depends on level 0):
                                         - 1Password secrets → fetch from API
                                       6. Set resolved env vars
                                       7. Create lease credentials (if configured)
                                       8. Strip env=false secrets from child
                                       9. Spawn child process with env
                                      10. Clean up temp files
```

### Key Files

| File | Role |
|------|------|
| `src/config.rs` | `Config`, `SecretConfig`, `ProfileConfig` — TOML schema, recursive loading, merging |
| `src/secret_resolver.rs` | `resolve_secret()`, `resolve_secrets_batch()` — priority chain, dependency graph |
| `src/providers/mod.rs` | `Provider` trait, `ProviderCapability`, `WizardCategory` — provider abstraction |
| `src/providers/age.rs` | age (file) encryption provider |
| `src/providers/aws_kms.rs` | AWS KMS encryption provider |
| `src/providers/aws_sm.rs` | AWS Secrets Manager remote provider |
| `src/providers/onepassword.rs` | 1Password CLI provider |
| `src/providers/bitwarden.rs` | Bitwarden CLI provider |
| `src/providers/vault.rs` | HashiCorp Vault provider |
| `src/providers/resolved.rs` | Provider resolution with caching |
| `src/lease.rs` | `LeaseLedger`, `LeaseRecord` — credential leasing, caching, encryption |
| `src/commands/exec.rs` | `ExecCommand` — spawn subprocess with resolved secrets |
| `src/hook_env.rs` | Shell integration — auto-load secrets on `cd` |
| `src/auth_prompt.rs` | Interactive auth retry — prompts for provider auth on failure |
| `src/temp_file_secrets.rs` | Ephemeral temp files for `as_file` secrets |
| `src/source_registry.rs` | Global config source cache for miette error spans |
| `src/mcp_server.rs` | MCP server — expose secrets/get_secret and exec to AI agents |

---

## Detailed Mechanisms

### 1. Provider Trait & Capabilities (`providers/mod.rs`)

The `Provider` trait is the core abstraction:

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    /// Get a single secret value
    async fn get_secret(&self, value: &str) -> Result<String>;
    
    /// Batch get multiple secrets (default: calls get_secret in a loop)
    async fn get_secrets_batch(
        &self,
        secrets: &[(String, String)],
    ) -> HashMap<String, Result<String>>;
    
    /// Encrypt a value (for Encryption-capable providers)
    async fn encrypt(&self, value: &str) -> Result<String>;
    
    /// What this provider can do
    fn capabilities(&self) -> &[ProviderCapability];
    
    /// Env vars this provider needs (for dependency graph)
    fn env_dependencies(&self) -> &[&str];
    
    /// Whether this provider needs interactive auth
    fn requires_interactive_auth(&self) -> bool;
}
```

**Three capability types:**
- `Encryption` — encrypt/decrypt locally (age, AWS KMS, Azure KMS, GCP KMS, FIDO2)
- `RemoteStorage` — store/fetch from remote (AWS SM, Azure SM, GCP SM, Vault, Bitwarden SM)
- `RemoteRead` — fetch from external service (1Password, Bitwarden CLI, Infisical, KeePass)

**22 providers** span the full spectrum: age, AWS KMS, Azure KMS, GCP KMS, AWS Secrets Manager, AWS Parameter Store, Azure Key Vault, GCP Secret Manager, 1Password, Bitwarden, Bitwarden SM, HashiCorp Vault, Infisical, KeePass, OS Keychain, password-store (pass), Doppler, PasswordState, Proton Pass, FIDO2, YubiKey, and plain (unencrypted defaults).

### 2. Secret Resolution Priority Chain (`secret_resolver.rs`)

When resolving a single secret, the priority is:

1. **Provider** (if specified with a value) → call `provider.get_secret(value)`
2. **Default value** (if specified) → use as-is
3. **Environment variable** → read `env::var(key)` from parent process
4. **Missing** → handle based on `if_missing` behavior

**Post-processing** is applied to whatever value is found:
- `json_path` — parse as JSON, extract dot-notation path (e.g., `"database.host"`)
- `line` — split on newlines, return Nth line (1-indexed)
- These are mutually exclusive

**if_missing behavior** follows a 6-level priority chain:
1. CLI flag (`--if-missing`)
2. Environment variable (`FNOX_IF_MISSING`)
3. Secret-level `if_missing`
4. Top-level config `if_missing`
5. Default env var (`FNOX_IF_MISSING_DEFAULT`)
6. Hard-coded default: `Warn`

### 3. Batch Resolution with Dependency Graph

The key insight: **providers can depend on env vars that are themselves secrets**. For example, 1Password needs `OP_SERVICE_ACCOUNT_TOKEN`, which might be an age-encrypted secret in the same config.

**Kahn's algorithm** computes resolution levels:

```
Level 0 (parallel): OP_SERVICE_ACCOUNT_TOKEN (age), PLAIN_VAR (env)
       ↓ env vars set
Level 1 (parallel): TUNNEL_TOKEN (1password), DB_PASSWORD (1password)
```

Within each level:
- Secrets are **batched by provider** (one API call for all 1Password secrets)
- Providers resolve in parallel (`buffer_unordered(10)`)
- No-provider secrets (default/env only) resolve in parallel too

**Cycle detection:** If a dependency cycle exists, the involved secrets are resolved best-effort in a final pass with a warning.

### 4. Config Loading & Merging (`config.rs`)

**Recursive directory search** — walks up from cwd to filesystem root, loading and merging config files at each level:

```
cwd/project-a/fnox.toml          (highest priority — nearest)
cwd/fnox.toml                    (parent directory)
~/fnox.toml                      (home directory)
~/.local/state/fnox/config.toml  (global config — lowest priority)
```

**Config file variants** (in load order):
- `fnox.toml` — main config (committed to git)
- `.fnox.toml` — dotfile variant (often gitignored)
- `fnox.{profile}.toml` — profile-specific (e.g., `fnox.staging.toml`)
- `.fnox.{profile}.toml` — profile-specific dotfile
- `fnox.local.toml` — local overrides (gitignored)
- `.fnox.local.toml` — local dotfile overrides

**Profile merging** — for non-default profiles, top-level secrets are the base and profile-specific secrets override them. The `--no-defaults` flag skips top-level merging entirely.

**Root flag** — `root = true` in a config file stops the upward recursion at that directory. Useful for monorepos where different subtrees have independent secret configs.

**Import directive** — `import = ["../shared-secrets.toml"]` pulls in another config file.

**Span-aware errors** — the `source_registry` module caches config file contents so parse errors show source-highlighted spans via miette:

```
Error: failed to parse config
  ╭─[fnox.toml:12:5]
  │
12│ DATABASE_URL = { provider = "aws-sm", value = 42 }
  │                                             ──
  │                                             expected string
──╯
```

### 5. Lease System (`lease.rs`)

For cloud providers that use temporary credentials (AWS STS, GCP OAuth, etc.), fnox supports **leasing**:

```toml
[leases.aws]
type = "aws-sts"
role_arn = "arn:aws:iam::123456789:role/dev"
duration = "1h"
```

**Lease lifecycle:**
1. `fnox lease create -i aws` — interactively create a lease
2. Credentials are optionally **encrypted** by an encryption provider before caching
3. Cached in `~/.local/state/fnox/leases/<project-hash>.toml`
4. On `fnox exec`, cached leases are reused if still valid (with 5-minute buffer)
5. If no valid cache exists, a fresh lease is created

**LeaseRecord structure:**
```rust
pub struct LeaseRecord {
    lease_id: String,
    backend_name: String,
    label: String,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    revoked: bool,
    cached_credentials: Option<IndexMap<String, String>>, // encrypted or plaintext
    encryption_provider: Option<String>,                  // which provider encrypted the cache
    config_hash: Option<String>,                          // invalidates cache when config changes
}
```

**Ledger concurrency:** Uses file locks (`fslock`) on a `.lock` sentinel file (not the data file itself, because `save()` uses atomic rename which replaces the inode). Lock scope is minimized:
1. Short lock → sync read → find cached entry → **release**
2. Async decrypt (no lock)
3. If cache miss → async lease creation (no lock)
4. Short lock → sync write → **release**

**Config hash** — when the backend config changes (e.g., role ARN rotation), the cached credentials are invalidated because the hash no longer matches.

### 6. exec Command (`commands/exec.rs`)

The `fnox exec` command is the main integration point:

1. **Resolve secrets batch** → get all secret values
2. **Set secrets as env** → temporarily set in parent process (for lease SDKs to find master creds)
3. **Resolve leases** → create/use leased credentials
4. **Set lease env vars** → `cmd.env(key, value)` for each lease credential
5. **Add regular secrets** → `cmd.env(key, value)` for each resolved secret
   - If `as_file = true` → write to temp file, set env to path
   - If `env = false` → `cmd.env_remove(key)` (strip inherited value)
   - Lease keys are protected — regular secrets don't overwrite lease credentials
6. **Drop temp env guard** → remove temporary env vars from parent
7. **Spawn child** → `cmd.spawn()`
8. **Signal forwarding** → SIGINT/SIGTERM forwarded to child (Unix only)
9. **Wait for exit** → propagate child's exit code
10. **Cleanup** → temp files dropped

### 7. Auth Retry (`auth_prompt.rs`)

When a provider auth fails (e.g., 1Password needs login), and we're in a TTY:

1. Detect auth error from provider response
2. Prompt user: `Provider '1password' needs authentication. Run 'op signin'? [Y/n]`
3. Run the auth command
4. Retry the secret fetch
5. If retry succeeds → continue
6. If retry fails → propagate error per `if_missing` policy

This works for both single-secret and batch resolution.

### 8. Shell Integration (`hook_env.rs`)

Similar to pitchfork/mise — installs a `cd` hook that auto-loads secrets:

```bash
# Generated by: eval "$(fnox activate bash)"
__fnox() {
    fnox export --shell-pid $$
}
chpwd_functions+=(__fnox)
```

The `fnox export` command outputs shell-compatible `export KEY=VALUE` lines for all resolved secrets, which are then `eval`'d into the shell environment.

### 9. MCP Server (`mcp_server.rs`)

Exposes two tools to AI agents (Claude, Cursor, etc.):

- **`get_secret`** — resolve and return a secret value
- **`exec`** — run a command with resolved secrets, return output

**Security controls:**
- `mcp.secrets` allowlist — restrict which secrets the MCP server can resolve
- `mcp.redact_output` — replace secret values in exec output with `[REDACTED]`
- `mcp.exec_timeout_secs` — timeout for exec subprocess

### 10. Secret Config Options (`SecretConfig`)

Each secret in `fnox.toml` supports:

| Field | Purpose |
|-------|---------|
| `provider` | Which provider to use |
| `value` | The provider-specific reference (encrypted blob, secret name, etc.) |
| `default` | Fallback value if provider fails |
| `description` | Human-readable description |
| `if_missing` | Error/Warn/Ignore when secret not found |
| `env` | Whether to inject into env vars (default: true) |
| `as_file` | Write to temp file, set env to path |
| `json_path` | Extract dot-notation path from JSON value |
| `line` | Extract Nth line from multi-line value |
| `sync` | Cached sync data (provider + encrypted value from `fnox sync`) |

### 11. CI Redaction (`commands/ci_redact.rs`)

For CI environments, `fnox ci-redact` outputs a shell function that redacts resolved secret values from stdout/stderr. This prevents accidental secret leaks in build logs.

### 12. Spanned Values (`spanned.rs`)

Provider names and values in the config are wrapped in `SpannedValue<T>` which tracks the byte range in the source file. This enables miette to highlight the exact location of errors:

```rust
pub struct SpannedValue<T> {
    value: T,
    span: Option<Range<usize>>,  // byte range in source
}
```

---

## Design Goals vs. Reality

| Goal | How Fnox Achieves It |
|------|---------------------|
| No vendor lock-in | Provider abstraction — switch any time |
| Team-friendly | Encrypted secrets in git, everyone can decrypt |
| Multi-environment | Profiles with inheritance (dev/staging/prod) |
| Shell integration | `cd` hook auto-exports secrets |
| Secure credential caching | Encrypt lease creds before persisting to disk |
| Batch efficiency | Group secrets by provider, resolve in parallel |
| Developer experience | Simple TOML config, `fnox exec -- cmd` |

---

## Key Takeaways for Our Platform

1. **Provider abstraction is powerful** — a single trait (`get_secret`, `encrypt`, `capabilities`, `env_dependencies`) unifies 22 different secret sources. The resolution logic is identical regardless of whether a secret comes from age encryption, AWS KMS, or 1Password.

2. **Dependency graph for secret resolution** — Kahn's algorithm handles the case where Provider A needs an env var that is itself a secret from Provider B. This is critical for chains like "age-encrypted 1Password token → 1Password secrets → AWS STS lease → AWS credentials".

3. **Lease system with encrypted caching** — temporary credentials are cached to disk but optionally encrypted by a local provider (age). The config hash invalidates stale caches when backend config changes. This is a great pattern for any system with expiring credentials.

4. **Config layering with profiles** — recursive directory search + profile merging gives a natural hierarchy: global defaults → project defaults → local overrides → profile-specific overrides. The `root` flag stops recursion at a boundary.

5. **Batch resolution by provider** — grouping secrets by provider and resolving in parallel (with `buffer_unordered`) is much more efficient than resolving one at a time, especially for remote providers with API rate limits.

6. **Auth retry with TTY detection** — when a provider auth fails and we're in a TTY, prompt the user to authenticate and retry once. In non-interactive mode (CI), fail immediately. This makes the same config work for both dev and CI.

7. **`as_file` secret injection** — for secrets that are too large for env vars or need to be passed as file paths (TLS certs, SSH keys), write to an ephemeral temp file and set the env var to the path. The temp file is kept alive for the child's lifetime.

8. **`env = false` stripping** — secrets marked `env = false` are actively removed from the child's environment via `cmd.env_remove(key)`, preventing stale inherited values from leaking through.
