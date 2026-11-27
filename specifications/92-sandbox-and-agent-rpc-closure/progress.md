# Spec 92 Progress: closing specs 77 and 83

_Last updated: 2026-11-28._

## Status at a glance

| Feature | State |
|---|---|
| 00 Live proof on the real services | not started |
| 01 DST fidelity: the real registry | not started |
| 02 Sandbox follow-ups | in progress: 11 of 12 issues closed; #1095 left |
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

## Feature 02 (2026-11-28)

Branch `fix/sandbox-follow-ups`, pull request #1111. It closes the rows of this feature
that do not need a Linux host with KVM.

- The first change closed #935, #966, #969, #970, #972, #1013, #1022 and #1023. It also
  fixed the serve token test hang (#1104) with a watchdog and the host clock, and fixed a
  dead export variant (#1106). The Linux run then showed the root cause of the hang: the
  slave test support leaked a second valtron pool's lifecycle guard, so every later
  `crate::pool::pool()` blocked. The slave tests now use the one shared test pool, and the
  serve token tests pass on Linux.
- The second change closes #977. `UnpackBegin` reads the `ready` line as a readiness
  future, and `UnpackChunk` and `UnpackCommit` write and read through non-blocking pipes
  that park on writability. `tests/slave/unpack.rs` fills every task worker with writers
  to a probe child that reads nothing for one second, then shows that a queued task runs;
  one writer streams a 100 MiB archive. The test compiles and runs on Linux only.
- The third change closes #978 as a design decision. Spec 77 feature 12 section 6.6 now
  names the `exec` child form and the reason, with the decision date 2026-11-27.
- The fourth change closes #1008. It clears the pedantic lints of `foundation_jsonschema`
  and of `foundation_compact` in its wasm runs, and removes the `clippy-deny-warnings`
  override of `foundation_jsonschema` from `ci/packages.toml`. `foundation_compact` has no
  override.
- #1095 (a Firecracker guest network) is left for its own change. It needs a Linux host
  with KVM, and this machine has none.

Verified on this host: format, clippy and tests of `foundation_sandbox`,
`foundation_deployment_docker`, `foundation_deployment_cloudflare`,
`foundation_compact` and `foundation_jsonschema`. The Linux-only test targets could not be
compiled here because no Linux cross C compiler is installed; the `sandbox-e2e` job
proves them.

## Next

1. Feature 02, #1095: the Firecracker guest network on a Linux host with KVM.
2. Feature 00, task T2 first: make `sandbox-cloudflare` green. The red run's log names the
   step and now prints the failing value.
3. Feature 00, task T1: one live run against the owner's account, recorded here.
4. Feature 00, tasks T3 to T6: the KMS and WebCrypto tests.
