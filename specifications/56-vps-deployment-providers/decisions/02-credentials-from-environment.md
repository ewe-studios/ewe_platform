# 02 — Credentials from the environment

**Date:** 2026-07-17
**Status:** Open

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

### 1. Variable names

Proposal — vendor-native names first, since anyone deploying to these providers
already has them exported for `terraform`/`doctl`/`hcloud`/`linode-cli`:

| Provider | Primary | Notes |
|---|---|---|
| DigitalOcean | `DIGITALOCEAN_TOKEN` | what `doctl`/terraform use; `DIGITALOCEAN_ACCESS_TOKEN` also common |
| Hetzner | `HCLOUD_TOKEN` | what `hcloud`/terraform use |
| Linode | `LINODE_TOKEN` | what `linode-cli`/terraform use |

Should we also accept an `EWE_`-prefixed override (`EWE_DIGITALOCEAN_TOKEN`) that
wins, so a workspace can pin a token without disturbing the vendor tools'
environment?

### 2. Absent or empty token

`from_env()` must **fail loudly** — no anonymous client, no deferred error at the
first request. Spec-53's audit found the opposite pattern repeatedly (a silently
dropped `subnet`, a `memory = "256m"` that parsed to nothing), and the house rule
is explicit: never invent a silent fallback for missing identity. The error should
name the variable it looked for.

### 3. Rejected token

A 401 from the vendor should map to a distinct error variant, not a generic
transport failure — "your token is wrong" and "the network is down" want different
responses from the caller.

### 4. Leakage

The token must never reach a log line, a `Debug` impl, or an error message. The
Cloudflare client's `Debug` should be checked for this too — it holds a token.

### 5. Is `from_env` the only constructor?

Proposal: **no.** Keep `new(token)` for tests and callers with their own secret
store, exactly as Cloudflare does; `from_env()` is the ergonomic default. This
also keeps the mock-server tests honest — they inject a token rather than
mutating the process environment (which is global and would race under
`--test-threads`).

## To resolve

1. Variable names, and whether an `EWE_`-prefixed override wins.
2. Confirm: `from_env()` errors when absent (naming the variable), and a
   `new(token)` constructor stays for tests/secret stores.
3. Whether a 401 gets its own error variant.
