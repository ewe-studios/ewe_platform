---
feature: "Live proof on the real services"
status: "pending"
priority: "high"
model: "sonnet"
phase: "1"
depends_on: []
crates: ["foundation_sandbox", "foundation_auth", "infrastructure/sandbox_worker"]
---

# Feature 00: Live proof on the real services

_Written in ASD-STE100 Simplified Technical English (STE)._

## 1. Goal

Prove the paths that no local stub can prove, and make the jobs that carry them green.

1. The Cloudflare Worker path runs against a real account, with the owner's credentials.
2. `sandbox-cloudflare` is green in CI, not red.
3. AWS KMS and GCP Cloud KMS pass against the real services.
4. The WebCrypto sealer has a runtime test in the browser job.

## 2. Tasks

| Task | Work | Closes |
|---|---|---|
| T1 | Run the live Cloudflare path from a local machine: the demo `examples/cf-sandbox-demo` deploys the Worker and its container with `mise run deploy`, prints its URL, and its `test/live.sh` runs the shared scenarios against that URL. Record the date, the account, the Worker name and the result in `progress.md` (owner decision 2026-11-28: the demo is the live proof) | Spec 83 feature 00 T7 |
| T2 | The `sandbox-cloudflare` job keeps the local `wrangler dev` run. Local mode gives a working container TCP port to one container per Worker process (#1221), so the four two-container tests cannot pass; the job is allowed to fail and names the issue, and its 13 passing tests still report. The live proof is T1 | #1221 |
| T3 | AWS KMS: add an endpoint override (`AWS_ENDPOINT_URL_KMS`) and a local test against **moto**, which runs on every machine and in a free CI job with no secret. The same test runs against the real service when `AWS_ROLE_ARN` exists. It creates a key, seals and opens, refuses a wrong key, and schedules deletion in a `finally` block | #984 (AWS half) |
| T4 | GCP KMS: add the gRPC path to `GcpKmsBackend` (generated Cloud KMS protos, h2c endpoint) and the test with a key ring and a key against localgcp; the same test runs against the real project through workload identity federation when that credential exists | #984 (GCP half) |
| T5 | Run the Vault Transit test in the normal `sandbox-e2e` job (Docker is present there), not only by hand | #984 note |
| T6 | WebCrypto: a `wasm-bindgen-test` in the browser job that seals and opens with `WebCryptoSealer`, and proves `exportKey` on its key fails | #985 |

## 3. Where each test runs

**Local first.** Every test runs on a developer machine with the real service. The live
Cloudflare test deploys to the owner's account, runs the suite against the deployed Worker,
and records the result. Nothing waits for CI.

**A missing credential skips, it does not fail.** Each test reads its value from the
environment. When the value is absent it prints which value is missing and returns. A skip
is a skip, not a pass: the run says so in its output.

**CI is opt-in.** The live Cloudflare test and the two KMS tests get a workflow that runs
them only when the repository holds the secret:

1. A `preflight` job reads `secrets.CLOUDFLARE_API_TOKEN` (and the KMS secrets) into its
   environment and writes one `has-live-credentials` output.
2. The live job has `if: needs.preflight.outputs.has-live-credentials == 'true'`.
3. The preflight job prints which values are missing, so a skipped live run is visible.

The owner adds the secrets when the live run is wanted in CI. Until then the job does not
run, and the local script is the proof.

## 4. Emulators: what fits

| Service | Emulator | Fits? | Why |
|---|---|---|---|
| AWS KMS | **moto** (`motoserver/moto`, Apache-2.0) | **Yes** | REST API with the real SigV4 shape; implements create, describe, encrypt, decrypt, generate-data-key and schedule-deletion; free; one container |
| AWS KMS | LocalStack | Yes, with a licence | KMS is available from LocalStack's paid Hobby plan, not the free community edition |
| GCP KMS | official `gcloud` emulators | **No** | The official emulator groups are bigtable, datastore, firestore, pubsub and spanner. Cloud KMS has none |
| GCP KMS | **localgcp** (MIT, v0.6.0) | **Yes, with a gRPC client** | It emulates Cloud KMS over gRPC on `:8091`, in process, no credential. `foundation_connectrpc` already has the h2c transport (`native/transport/h2.rs`) and a protobuf codec (`shared/codec/proto.rs`). The missing pieces are the generated Cloud KMS protos and a protocol choice in `GcpKmsBackend` |

The decision:

1. **AWS KMS:** moto, locally and in a free CI job with no secret; the real AWS service behind
   the same test when the role exists.
2. **GCP KMS:** add a **gRPC path** to `GcpKmsBackend` and run it against localgcp, locally
   and in a free CI job. The stack already speaks gRPC: the h2c transport and the buffa
   protobuf codec exist. The work is the generated Cloud KMS protos and the protocol choice,
   not a new transport. The real GCP project stays the gated proof, over REST or gRPC with
   workload identity federation.
3. **localgcp runs as a binary**, not a container: its ghcr.io image needs a login, but the
   project publishes signed release binaries for Linux, macOS and Windows (v0.6.0). A mise
   task downloads the pinned release, checks the checksum, starts it, saves the PID and stops
   it the same way the other harness scripts do.

## 5. What each task needs

| Task | Credential or tool | When it is absent |
|---|---|---|
| T1, T2 | `CLOUDFLARE_API_TOKEN` (Workers Scripts Edit, D1 Edit, R2 Edit, Workers AI Read), `CLOUDFLARE_ACCOUNT_ID` | The test prints the reason and returns |
| T3 | An AWS role through GitHub OIDC, or a scratch account key | The test is ignored and the workflow reports that it skipped |
| T4 | A GCP service account through workload identity federation | The same |
| T5 | Docker | The test prints the reason and returns |
| T6 | A headless browser | The job reports that it skipped |

A test that skips must say which value is missing. A skip is not a pass; the workflow that is
meant to run it fails when the value is absent and the run is a release run.

## 6. Tests

| Test | Where | Oracle |
|---|---|---|
| Live Cloudflare acceptance | `foundation_sandbox/tests/e2e_cloudflare/` with `EWE_SANDBOX_WORKER_URL` set to the deployed Worker | the shared scenarios |
| `sandbox-cloudflare` green | the CI job | expected outcome |
| AWS seal and open | `foundation_auth/tests/kms/aws_tests.rs` (ignored by default) | the sealer contract |
| GCP seal and open | `foundation_auth/tests/kms/gcp_tests.rs` (ignored by default) | the sealer contract |
| Vault Transit | `foundation_auth/tests/kms/kms_vault_transit.rs` | the sealer contract |
| WebCrypto runtime | `foundation_auth/tests/wasm/webcrypto_sealer_tests.rs` | the sealer contract plus a refused export |

## 7. Verify

```bash
cargo test -p foundation_auth --features testing --profile uat kms
cargo test -p foundation_auth --features testing --profile uat keystore
cargo test -p foundation_sandbox --features "testing slave cloudflare" --profile uat cloudflare
```

The two real-cloud tests run only with their credentials. The job that runs them reports
`sandbox-cloudflare`, `kms-aws` and `kms-gcp` as separate checks, so a skip is visible.

## 8. Done when

- The live Cloudflare run is recorded with its date and result.
- `sandbox-cloudflare` is green on `master`.
- The AWS and GCP tests pass against the real services, and their workflow is green.
- The Vault and WebCrypto tests run in the normal jobs.
- #984 and #985 are closed.
