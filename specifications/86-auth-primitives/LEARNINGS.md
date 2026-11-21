# LEARNINGS — Auth Primitives

## Why a separate small specification (2026-11-15)

The auth types were spread across three specifications:

- Spec 77 feature 00 wanted a redacted `Secret` and feature 02 wanted `check_bearer`, because
  the sandbox service needs a bearer token and the sandbox spec did not want to depend on the
  agent RPC stack.
- Spec 81 owned `ClientAuth`, `AuthInfo` and the providers, because the agent RPC server and
  the proxy need them.

The result would be two redacted string types and two bearer parsers in one workspace. A
security primitive is the worst place for a copy: a fix to one copy (a control-byte check, a
constant-time compare) does not reach the other.

The owner direction: one small specification owns the blocks in `foundation_auth`, before the
consumers. Spec 81 keeps the signed secrets, the key files and the provider set. Spec 87 keeps
mTLS. Spec 77 and spec 83 use the blocks.

## Why `Secret` replaces `ConfidentialText` instead of joining it (2026-11-15)

`foundation_auth` already has `ConfidentialText`: a `Zeroizing<String>` newtype with serde. It
is the right shape and the wrong name for a server credential, and its `Display` prints
`Confidential(<first char><stars>)`.

A second type would be a copy with a different name. So the type is renamed `Secret`, the old
name becomes an alias for one release, and the `Display` form becomes `<redacted>`.

The `Display` change is a behaviour change. Its reasons:

1. The first character is information about the secret. On a token with a known prefix it says
   almost nothing; on a password it says one character.
2. The star count gives the length, which narrows a brute-force search.
3. A fixed string is easier to assert in a test: one expected value, not a generated one.

## Why one `constant_time_eq` and not a new crate (2026-11-15)

The workspace has five comparisons: one public and four private copies, all in
`foundation_auth`. A `subtle` dependency would add a crate for a 10-line loop. The loop is
short, it is the same loop in all five places, and one public function removes the copies.

A test that wants its own copy may have one, and it must say why. A test that compares two
values it wrote itself does not need the protection.

## Why `ClientAuth` has no `Ctx` parameter (2026-11-15)

The alternative, `authenticate(&self, ctx: &Ctx, facts: &RequestFacts)`, matches the rest of
the workspace and makes a provider stateless.

It fails one case: the connectrpc interceptor calls `AuthFunc::authenticate(&headers, &url)`,
and that hook has no `Ctx`. A provider that needs the `Ctx` must get it at construction. That
is also the right lifetime: a provider is a service with a trust store and a key, built once
at start-up, not a value built per call.

So a provider takes its `Ctx` in `new`, and the trait method takes the per-request facts.

## Why `CallCredentials` is a struct and not three arguments (2026-11-15)

A third provider is certain to arrive (a signed secret in spec 81, mTLS in spec 87, a JWT in
spec 81). Each new fact (a peer certificate, a client IP, a forwarded principal) would change
the trait and every provider. A struct changes one place and the provider that reads the new
field.

`CallCredentials` borrows, so it is `Copy` and adds no allocation to the request path. The name
and the four fields come from spec 81 feature 00 section 8.1; this specification moves them and
does not redesign them.

## Why "no credential" is `Ok(None)` and not an error (2026-11-15)

A composite provider (`AnyOf`, spec 83 feature 03) tries several providers in order. It must
tell three outcomes apart:

| Outcome | Meaning | What the composite does |
|---|---|---|
| `Ok(Some(info))` | The caller is known. | Stops and uses the identity. |
| `Ok(None)` | The call has no credential of this kind. | Tries the next provider. |
| `Err(_)` | The call has a credential of this kind and it is bad. | Stops and refuses the call. |

With `Missing` as an error variant, the composite cannot tell "no header of my kind" from "a
bad header". A mTLS provider would refuse a token call, and a token provider would refuse a
certificate call. So absence is `Ok(None)` and `AuthError` keeps only real failures. The
remaining variants (`Invalid`, `Expired`, `NotYetValid`, `Revoked`, `Spent`, `Forbidden`,
`Unavailable`, `Misconfigured`) come from spec 81 feature 00 section 8.1 and map to the Connect
codes of that table.

## What a consumer must not do

1. Define a redacted string type. Use `Secret`.
2. Parse `Authorization` by hand. Use `Secret::from_bearer` or `extract_bearer_token`.
3. Compare a token with `==`. Use `Secret::ct_eq` or `constant_time_eq`.
4. Put a credential in an error message. `AuthError` has the reason only.
5. Build `Bearer ` with `format!`. Use `bearer_header`.

## What the implementation found (2026-11-22)

### The fifth comparison

The specification counted four copies of `constant_time_eq` in `foundation_auth`.
The tree had a fifth public copy in `foundation_keychain::core::crypto`. That
crate already depends on `foundation_auth`, so its function became a `pub use`
re-export of the shared one. The source scan in the tests and the `grep` in
section 8 now find one definition.

### `get()` was the alias's real cost

`ConfidentialText::get()` returned an owned `String`, and about twenty call
sites used it. Two used it as a function value: `.map(ConfidentialText::get)`.
An alias to `Secret` cannot keep `get()` and also make `expose()` the only
reader, so every call site moved to `expose()`. A mapped call becomes a closure
that owns the text, for example `.map(|token| token.expose().to_owned())`.

### The S6 test lives in connectrpc

`auth_info_single_home` needs both `foundation_auth::AuthInfo` and
`foundation_connectrpc::AuthInfo`. Only a connectrpc test can name both paths
without a dev-dependency cycle from `foundation_auth` back to connectrpc. The
test is in `backends/foundation_connectrpc/tests/auth_tests.rs`.

### The timing test measures the comparison, not the provider

A measurement that includes `Box::pin`, an allocation and the future poll has
too much noise. `static_token_constant_time` checks `AuthError::Invalid` through
the provider, then measures `Secret::ct_eq` for a first-byte difference and a
last-byte difference over 1000 samples. A bounded ratio of 0.25 to 4.0 catches
an early-return comparison, which is about 32 times faster on the first byte,
and tolerates a loaded machine.

### The section 8 `struct Secret` grep is too broad

`grep -rn "struct Secret" backends/` matches the generated Docker API type
`foundation_deployment_docker::...::Secret`, its related `Secret*` types, the
Cloudflare secrets store types, and two test-only `SecretHandler` structs. The
one credential type is `foundation_auth::server_credential::Secret`. The
command cannot show "one hit" without a narrower pattern. Issue #674 records
the command defect.
