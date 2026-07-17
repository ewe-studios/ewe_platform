# 02 — Credentials from the environment

**Date:** 2026-07-17
**Status:** **Resolved** (2026-07-17, owner)

## The question

The owner's requirement: **the user provides credentials over environment
variables.** That settles the source, but not the shape — and the details are
where credential handling usually goes wrong.

## Prior art in this tree

`CloudflareClient::from_env()` exists and is the closest model. It reads a token
plus a zone id from the environment and returns `Result<Self, CloudflareError>`.
Note `foundation_deployment_cloudflare` also offers `new(token, zone_id)` and
`with_client(...)`, so the env path is a convenience, not a constraint.

## What to decide

### 1. Variable names — **resolved** (owner, 2026-07-17)

**Vendor-native names, with an `EWE_`-prefixed override that wins.** Anyone who
already deploys to these providers has the vendor name exported for
`terraform`/`doctl`/`hcloud`/`linode-cli`, so the common case needs no setup; and
a workspace that wants a different token can pin one without disturbing the
environment every other tool reads.

**The `EWE_` prefix is mechanical, not a new name.** Each provider declares the
vendor spelling(s) its ecosystem uses; the override for each is literally
`EWE_` + that name. Strip the prefix and you are back at the vendor's variable —
so there is no third spelling to invent, remember, or get wrong:

```rust
// the provider declares only what the vendor uses:
const VENDOR_VARS: &[&str] = &["DIGITALOCEAN_TOKEN", "DIGITALOCEAN_ACCESS_TOKEN"];

// resolution is derived from it — overrides first, then vendor-native:
//   EWE_DIGITALOCEAN_TOKEN
//   EWE_DIGITALOCEAN_ACCESS_TOKEN
//   DIGITALOCEAN_TOKEN
//   DIGITALOCEAN_ACCESS_TOKEN
```

| Provider | Vendor spelling(s) it declares | Used by |
|---|---|---|
| Hetzner | `HCLOUD_TOKEN` | `hcloud`, terraform |
| DigitalOcean | `DIGITALOCEAN_TOKEN`, `DIGITALOCEAN_ACCESS_TOKEN` | `doctl`, terraform (both spellings are live in the wild) |
| Linode | `LINODE_TOKEN` | `linode-cli`, terraform |

**Precedence: every `EWE_*` before every vendor-native.** An override says "use
*this* token for this workspace", and that intent should beat any vendor variable
lying around in the environment — not just its own twin. Within each tier, the
declared order wins.

When **none** are set, `from_env()` errors **naming every name it looked for** —
a "credentials not found" that does not say what it looked for is a scavenger
hunt.

This is also what settles DO's two spellings: they are not a special case, just a
provider that declares two. Someone with only `DIGITALOCEAN_ACCESS_TOKEN`
exported works out of the box, and their override is `EWE_DIGITALOCEAN_ACCESS_TOKEN`
— derived, not invented.

An earlier draft here proposed a *fixed* `EWE_DIGITALOCEAN_TOKEN` alongside an
optional `DIGITALOCEAN_ACCESS_TOKEN` fallback — which quietly invented a third
spelling whose relationship to the vendor's names was arbitrary. Deriving the
override from each declared name removes the question instead of answering it.

### 2. Absent or empty token — **resolved: fail loudly**

`from_env()` errors immediately — no anonymous client, no deferred failure at the
first request — and the error **names both** variables it looked for. Spec-53's
audit found the opposite pattern repeatedly (a silently dropped `subnet`, a
`memory = "256m"` that parsed to nothing), and the house rule is explicit: never
invent a silent fallback for missing identity.

```
Err("no Hetzner token: set EWE_HCLOUD_TOKEN or HCLOUD_TOKEN")
```

### 3. Rejected token — **resolved: its own variant**

A 401 maps to a distinct variant, never a generic transport/API error. "Your token
is wrong" and "the network is down" want different responses from the caller, and
a deploy that dies on a typo'd token should say which it was.

```rust
Err(HetznerError::Unauthorized)          // 401 — the token was rejected
Err(HetznerError::Transport(..))         // the network
Err(HetznerError::Api { status, message })  // everything else
```

### 4. Leakage — **resolved: never in `Debug`, logs, or errors**

A hand-written `Debug` that redacts the token, and no error message that
interpolates it. **Also audit `CloudflareClient`** for the same while we are here
— it holds a token and derives its own `Debug`.

Linode adds one more secret to keep out of everything: the `root_pass` its create
call requires (feature 03).

### 5. Is `from_env` the only constructor? — **resolved: no**

`from_env()` is the ergonomic default; `new(token)` stays for tests and callers
with their own secret store, and `with_client(http, token)` injects the transport
— exactly the three `CloudflareClient` already offers.

This also keeps the mock-server tests honest: they **inject** a token rather than
mutating the process environment, which is global and would race under
`--test-threads`.

## Resolved — summary

1. **Names:** a provider declares the vendor spelling(s) it uses; the override is
   `EWE_` + that name, derived. All `EWE_*` are tried before all vendor-native.
2. **Absent:** `from_env()` errors immediately, naming both.
3. **Rejected:** 401 gets its own variant.
4. **Leakage:** never in `Debug`, logs, or errors (audit Cloudflare too).
5. **Constructors:** `from_env()` + `new(token)` + `with_client(http, token)`.

Nothing outstanding. DO's two spellings are not a special case — it simply
declares two vendor names, and the `EWE_` override is derived from each.
