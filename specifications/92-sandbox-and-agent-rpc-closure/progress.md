# Spec 92 Progress: closing specs 77 and 83

_Last updated: 2026-11-28._

## Status at a glance

| Feature | State |
|---|---|
| 00 Live proof on the real services | not started |
| 01 DST fidelity: the real registry | not started |
| 02 Sandbox follow-ups | not started |
| 03 GCP KMS over gRPC | not started |
| 04 Live cloud resources with OpenTofu | not started |

## What is already true

Specs 77 and 83 are merged on `master` (`348620340`, then `39cd005f1` and `c9a913dc0`). The
combined pull request #1009 and the dependency pull request #1040 are merged.

The owner rejected spec 77 feature 16 (Docker Sandboxes, `sbx`) on 2026-11-27; decision D18.

The owner confirms that `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_WORKERS_AI_API_TOKEN` are in
the shell environment and in `~/.userenv`, which unblocks spec 83 feature 00 task T7.

## Work in review (2026-11-27)

- **Feature 00, task T3 (AWS KMS locally)** is pull request #1055. It adds the endpoint
  override (`AWS_ENDPOINT_URL_KMS`), the moto test, `mise run kms:moto` and the free CI job
  `kms-local`. It also fixes a duplicate `Content-Type` (#1054) and makes `Decrypt` send
  `KeyId`, so a backend for one key cannot open another key's blob.
- The full `packages=all` run on the earlier master head is still working. It is superseded,
  but its `sandbox-cloudflare` log is read before it is discarded.

Local stack verified on this host: moto serves AWS KMS (create, encrypt, decrypt, data key,
alias) on port 5055; Vault serves Transit on 8200 with its engine enabled by an init service;
localgcp v0.6.0 serves Cloud KMS on h2c `:8091` and is not yet wired to a client.

## Feature 00 task T2: the Cloudflare create (2026-11-28)

The create timeout has two causes.

1. `ControlService::create` pulled the create progress iterator again in the same poll after
   `Stream::Wait`. The iterator is a driving valtron stream: `Wait` means that the queue is
   empty and that it must run again. On the single-threaded wasm host that loop starved the
   event loop, so the create never finished and the Worker stopped answering. The service
   now parks on the injected timer for `SandboxLimits::create_poll_interval` (default 4 ms)
   and then pulls again. The test `tests/control/service_create.rs` fails on the old code.
   The remote create also uses `boot_timeout` (10 s), the create deadline, for the whole
   stream instead of the unary `rpc_timeout` (5 s). A cold create takes about 3.5 s on the
   local emulated container, so 5 s was too short.
2. The slave did not detect the Cloudflare container runtime: the runtime writes neither
   `/.dockerenv` nor `/run/.containerenv`. The slave ran the virtual-machine boot duties,
   failed, and never listened on the slave port. It now also treats the `CLOUDFLARE_*`
   environment as a container (issue #1133).

Local result on macOS with `wrangler dev --local`: a create returns `Created` in about
1.5 s and the Worker stays healthy. The full end-to-end suite on the local Worker reports
7 passed and 10 failed. The passed tests include `bad_id`, `not_found`, `token` and
`model_token`, which all need a live Worker.

The job still needs three independent fixes, each with an issue:

- #1140: a streaming scoped call (`Exec`, `FileRead`) reaches the route check as a Connect
  envelope and is refused.
- #1141: a unary response arrives with `Content-Encoding: gzip` and the client decodes the
  compressed bytes as JSON.
- #1142: the sandbox Durable Object never builds the model configuration, so a model
  sandbox create is refused.

The create fix alone cannot make `sandbox-cloudflare` green.

## Next

1. Feature 00, task T2 first: make `sandbox-cloudflare` green. The red run's log names the
   step and now prints the failing value.
2. Feature 00, task T1: one live run against the owner's account, recorded here.
3. Feature 00, tasks T3 to T6: the KMS and WebCrypto tests.
