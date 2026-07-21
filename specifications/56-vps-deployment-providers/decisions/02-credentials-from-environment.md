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
| Hetzner | `HCLOUD_TOKEN`, `HETZNER_API_TOKEN` | `hcloud` + terraform; **lego** (see the amendment below) |
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

### Amendment (2026-07-17): Hetzner declares **two** spellings, and two it must not

Prompted by the owner having `HETZNER_API_TOKEN` exported. The first draft assumed
`HCLOUD_TOKEN` was the only Hetzner spelling. It is not — and the correction was
worth making because **guessing here means picking up a credential for the wrong
product**.

Verified against each tool's source rather than from memory (an earlier claim here
was recalled, and wrong):

| Variable | Read by | Endpoint it authenticates against | Declared? |
|---|---|---|---|
| `HCLOUD_TOKEN` | `hcloud` CLI (`"HCLOUD_" + ToUpper("token")`), Terraform/OpenTofu (`EnvDefaultFunc("HCLOUD_TOKEN", nil)`) | `api.hetzner.cloud` | **yes**, first |
| `HETZNER_API_TOKEN` | `lego` — the ACME library behind Traefik and much of cert-manager (`hetznerv1`) | **`api.hetzner.cloud/v1`** — the same endpoint we call | **yes** |
| `HETZNER_API_KEY` / `HETZNER_TOKEN` | `lego` (`legacy`, which lego itself deprecates) | `dns.hetzner.com` — the **DNS** API | **no** |
| `TF_VAR_hcloud_token` | a Terraform config that declared `variable "hcloud_token"` | — | **no** |

**Why the DNS variables are excluded.** Hetzner DNS is a different product with
different tokens. Reading one would send a DNS token to the Cloud API, earn a 401,
and have us report "check your token" — when the truth is "that token is for
another Hetzner product". That is exactly the confusion §3's `Unauthorized`
variant exists to prevent, so admitting the variable would undo it. lego draws the
same line: it deprecates its DNS-API path in favour of the Cloud API.

**Why `TF_VAR_hcloud_token` is excluded.** `TF_VAR_*` is *Terraform's* namespace,
not Hetzner's: `TF_VAR_x` sets whatever input variable a config declared as
`variable "x"`. The suffix is the config author's choice, so honouring this one
spelling would serve one popular style and silently miss `variable "token"`.

Nothing about the mechanism changed — this is §1 working as designed. A provider
declares the vendor spellings its ecosystem uses; DigitalOcean already declared
two for the same reason. Hetzner's resolution order is now:

```
EWE_HCLOUD_TOKEN  →  EWE_HETZNER_API_TOKEN  →  HCLOUD_TOKEN  →  HETZNER_API_TOKEN
```

## Resolved — summary

1. **Names:** a provider declares the vendor spelling(s) it uses; the override is
   `EWE_` + that name, derived. All `EWE_*` are tried before all vendor-native.
2. **Absent:** `from_env()` errors immediately, naming both.
3. **Rejected:** 401 gets its own variant.
4. **Leakage:** never in `Debug`, logs, or errors (audit Cloudflare too).
5. **Constructors:** `from_env()` + `new(token)` + `with_client(http, token)`.

Nothing outstanding. DO's two spellings are not a special case — it simply
declares two vendor names, and the `EWE_` override is derived from each.
