# LEARNINGS — Proxy Registration and Registrar

## One spec, two halves (2026-11-14)

The proxy registration authority and the registrar sidecar are two ends of one contract. Put
them in one specification, or the proto, the failure table and the task order drift apart.

They are still two features with two owners:

- Feature 00 extends `foundation_proxy`. It owns the wire contract, the registry, leases, the
  rotation rule and the router swap.
- Feature 01 is a new crate, `foundation_registrar`. It owns discovery, probes, metrics and the
  reconcile loop.

## Read the existing proxy first

`foundation_proxy` already has a fixed router, health probes, a Unix control socket, a persisted
state store and TLS. The registration work extends them. A second probe loop or a second control
channel is a defect.

## The two auth links

- The sidecar authenticates only to the proxy registration authority: enrollment token one time,
  then mTLS, or a Unix socket with file permissions.
- The proxy ends client TLS. It never sees the app's client credential.
- The sidecar never takes part in client TLS.

## Health has Kubernetes semantics

A startup probe gates liveness and readiness. A liveness failure is a failure state. A readiness
failure removes the backend from rotation but keeps it alive. A missed report lets the lease
expire; a later report brings the backend back. No operator action is needed except for an
expired or revoked certificate.

## Rotation must not drop live connections

A backend that leaves gets no new requests. Its open connections finish, up to
`connection_drain_timeout`. The router swaps whole immutable snapshots with a generation number;
no request sees a half-built table.

## Stage 1 (T1–T6) landed (2026-11-23)

The proto, the registry, leases, the rotation rule and the address policy are in
`backends/foundation_proxy`. What the work taught:

- **Generated enums are open.** `buffa` puts every enum field behind
  `buffa::EnumValue<E>`, so a handler reads `field.as_known()` and writes
  `E::VARIANT.into()`. A message literal also needs `..Default::default()` for its
  hidden `__buffa_unknown_fields`.
- **`ServiceRuntime` must accept runtimes.** Rebuilding a service from
  `ServiceConfig::backends` would make new `BackendRuntime` values and lose the
  in-flight count. `ServiceRuntime::with_backends` takes the existing `Arc`s, and
  the registry reuses one when the address and weight did not change.
- **Two sweeps, not one, remove a stale entry.** The sweep that finds an expired
  lease sets `Stale { since: now }`; the entry is removed only by a later sweep
  at least `stale_remove_after` after that `since`. A single large clock advance
  is not enough.
- **A blocking clock cannot drive the async calls.** `block_on_future` needs
  `Future + 'static`; a test that borrows the registry and request uses
  `futures_lite::future::block_on` instead. Production calls are plain `async`.
- **The ownership check, not a host prefix, protects a backend ID.** The
  `<host_id>/<entry>` form is a convention; a strict prefix rule would make row
  7.8 (a conflicting registration) unreachable. The registry keys ownership by
  the exact ID and lets a stale owner be replaced.

### Gap found outside this feature

`foundation_auth` has no public way to make the enrollment **server proof** of
spec 81 `auth.md` section 9.1.1: `SigningKey::sign` is `pub(crate)`. Enrollment
in feature 00 can verify the token, spend a use, sign the certificate signing
request (CSR) and record the client, but it cannot fill `EnrollResponse`
`server_proof` until `foundation_auth` exposes a public signer for that message.
This needs a GitHub issue against `foundation_auth`; it is not a spec 82 change.


## Stage 2 (T7–T11) landed (2026-11-23)

- **The host `Env` supplies the entropy.** `SigningKey::generate_ed25519` and
  `SecretIssuer::issue` read `ctx.require_env()`, and `Ctx::host()` installs no
  `Env`. The authority now takes the token seed from the `Env` entropy and
  returns a typed error when the host did not install one. Tests call
  `foundation_core::env::install(&ctx, foundation_core::env::system)`.
- **The start future must be boxed.** Adding the authority's own future to
  `ProxyServer::start` made the compiler overflow the trait solver at every
  caller that used `BlockingFutureExt::block` on the result. `start` now returns
  `Pin<Box<dyn Future<...> + Send>>`, and the crate sets
  `#![recursion_limit = "512"]` for the send check.
- **The old address serves until the new one is ready.** On an address change
  the old runtime stays in the router (`Live`, health from the last report)
  until the replacement reports `READY`; then the old moves to `Draining`. This
  is what makes redeploy 7.7 lose no request. A `drain_timeout` cap removes a
  kept-over old entry whose replacement never becomes ready.
- **One control channel.** `issue-registrar-token` and `revoke-registrar` are
  new arms of the existing `dispatch`, and `list` appends the registered rows.
  The response of `list` stays an array, so an existing reader of `data[0]` is
  not broken.
- **The server proof fits the public API now.** `SecretIssuer::sign_enroll_proof`
  signs the secret ID, the nonce, the SHA-256 of the certificate signing request
  (CSR) and the host. `SecretVerifyingKey::verify_enroll_proof` (via
  `SignedSecret::verifying_key`) checks it.
- **Unix socket ownership is a real check.** The socket is chmodded to `0o660`,
  then its owner and group are compared with the process's effective IDs. The
  test proves the refusal with an unwritable directory, which is portable to a
  non-root test runner.

## Feature 01 (registrar sidecar) landed (2026-11-23)

- **Generate from the proxy's proto file, not its crate.** `foundation_registrar`'s
  `build.rs` points `buffa-build` at `../foundation_proxy/proto`. The crate has no
  runtime dependency on `foundation_proxy`; only the tests use the real
  registration authority as a dev-dependency. A generated message field is
  `buffa::MessageField<T, buffa::Inline<T>>`, so a request literal uses
  `Some(value).into()` (the `From<Option<T>>` impl is generic over the pointer).
- **The server proof ties the token, the nonce and the CSR text.** The sidecar
  decodes the token, calls
  `SignedSecret::verifying_key().verify_enroll_proof(secret.id(), nonce,
  Sha256(csr_pem), host_id, server_proof)`, and only then stores the certificate.
  The private key never leaves the state directory.
- **Two thirds of the certificate life must include the CA backdate.** The CA
  backdates `not_before` by five minutes. A test certificate with a 60-second
  lifetime is therefore already past two thirds at issue time. Use a lifetime
  much longer than the backdate (900 s in the test) or the renewal fires at once.
- **A shared simulated clock drives both ends.** Passing the same `Ctx` to the
  proxy authority and the client lets `advance_wall` move the certificate
  lifetime without a real wait; the TLS time providers read the same clock.
- **A blocking process cannot be bounded by an async timer on the same thread.**
  An `exec` probe that runs `Command::output()` inline delays every other probe.
  Run the command on a dedicated thread and await a `oneshot` channel, raced with
  `sleep_async`; then the timeout fails that check (section 6.13).
- **`DockerClient` is a concrete type, but the sidecar needs a fake.** The small
  `ContainerApi` trait is the seam: production wraps `DockerClient`, tests return
  generated `ContainerInspectResponse` and `ContainerStatsResponse` values.
  `container_list` itself still returns `serde_json::Value` (see the finding
  below).
- **A removed address keeps serving until the replacement is ready.** The
  reconciler sends `Register` with the new address and then reports the new
  backend's health; the proxy retires the old address only when the new one
  reports `READY`. The end-to-end test sends requests through the upgrade to
  prove no request failed.
- **The mtime is the change signal, and a parse error keeps it.** `ManifestSource`
  compares the file's modification time; on a parse error it keeps the last good
  manifest and the old mtime, so every loop retries and logs the error
  (section 6.7). SIGHUP forces a read.
- **A rejected backend stays dirty.** A `RegisterResult` with a
  `rejected_reason` (another sidecar owns the ID) is not stored as accepted, so
  the next loop sends it again until the proxy's lease frees the ID (section 6.9).

### Finding outside this feature

`foundation_deployment_docker::DockerClient::container_list` returns
`serde_json::Value`, while the other container calls use generated types. The
generated API has no `ContainerSummary` type for `GET /containers/json`, so the
registrar reads `Id` and `State` from the raw value. Root rule 5 says to fix the
generator and regenerate; this needs a GitHub issue against
`foundation_deployment_docker` (severity: medium).
