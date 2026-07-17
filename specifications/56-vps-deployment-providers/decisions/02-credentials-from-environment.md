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

| Provider | Override (wins) | Vendor-native | Used by |
|---|---|---|---|
| Hetzner | `EWE_HCLOUD_TOKEN` | `HCLOUD_TOKEN` | `hcloud`, terraform |
| DigitalOcean | `EWE_DIGITALOCEAN_TOKEN` | `DIGITALOCEAN_TOKEN` | `doctl`, terraform |
| Linode | `EWE_LINODE_TOKEN` | `LINODE_TOKEN` | `linode-cli`, terraform |

Precedence is strictly `EWE_*` → vendor-native. When **neither** is set,
`from_env()` errors **naming both** names it looked for — a "credentials not
found" that does not say what it looked for is a scavenger hunt.

`DIGITALOCEAN_ACCESS_TOKEN` is also common in the wild; worth accepting as a
third fallback for DO, or worth leaving out to keep the rule "two names, in
order"? (Minor; not blocking.)

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

1. **Names:** `EWE_<VENDOR>_TOKEN` wins, else the vendor-native name.
2. **Absent:** `from_env()` errors immediately, naming both.
3. **Rejected:** 401 gets its own variant.
4. **Leakage:** never in `Debug`, logs, or errors (audit Cloudflare too).
5. **Constructors:** `from_env()` + `new(token)` + `with_client(http, token)`.

Open (minor, not blocking): whether to accept `DIGITALOCEAN_ACCESS_TOKEN` as a
third fallback for DO, or keep the rule at exactly two names in order.
