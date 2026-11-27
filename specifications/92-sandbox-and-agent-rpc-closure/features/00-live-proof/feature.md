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
| T1 | Run the live Cloudflare path from a local machine: deploy the sandbox Worker to the owner's account, set `SANDBOX_TOKEN`, wait for `Probe` 200, then run the end-to-end suite against that URL. Record the date, the account, the Worker name and the result in `progress.md` | Spec 83 feature 00 T7 |
| T2 | Make the `sandbox-cloudflare` GitHub Actions job green. It runs `wrangler dev --local` with local containers on the runner, so it needs no Cloudflare account. Read the red run's log, fix the cause, keep the readiness probe and the spec print | The red check |
| T3 | AWS KMS: add an ignored-by-default test for `AwsKmsBackend`. It creates a key with the given policy, seals and opens, refuses a wrong key, and schedules deletion in a `finally` block. A workflow runs it when the `AWS_ROLE_ARN` secret exists | #984 (AWS half) |
| T4 | GCP KMS: the same test for `GcpKmsBackend` with a key ring and a key. The workflow runs it through workload identity federation | #984 (GCP half) |
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

## 4. What each task needs

| Task | Credential or tool | When it is absent |
|---|---|---|
| T1, T2 | `CLOUDFLARE_API_TOKEN` (Workers Scripts Edit, D1 Edit, R2 Edit, Workers AI Read), `CLOUDFLARE_ACCOUNT_ID` | The test prints the reason and returns |
| T3 | An AWS role through GitHub OIDC, or a scratch account key | The test is ignored and the workflow reports that it skipped |
| T4 | A GCP service account through workload identity federation | The same |
| T5 | Docker | The test prints the reason and returns |
| T6 | A headless browser | The job reports that it skipped |

A test that skips must say which value is missing. A skip is not a pass; the workflow that is
meant to run it fails when the value is absent and the run is a release run.

## 5. Tests

| Test | Where | Oracle |
|---|---|---|
| Live Cloudflare acceptance | `foundation_sandbox/tests/e2e_cloudflare/` with `EWE_SANDBOX_WORKER_URL` set to the deployed Worker | the shared scenarios |
| `sandbox-cloudflare` green | the CI job | expected outcome |
| AWS seal and open | `foundation_auth/tests/kms/aws_tests.rs` (ignored by default) | the sealer contract |
| GCP seal and open | `foundation_auth/tests/kms/gcp_tests.rs` (ignored by default) | the sealer contract |
| Vault Transit | `foundation_auth/tests/kms/vault_transit_tests.rs` | the sealer contract |
| WebCrypto runtime | `foundation_auth/tests/wasm/webcrypto_sealer_tests.rs` | the sealer contract plus a refused export |

## 6. Verify

```bash
cargo test -p foundation_auth --features testing --profile uat kms
cargo test -p foundation_auth --features testing --profile uat keystore
cargo test -p foundation_sandbox --features "testing slave cloudflare" --profile uat cloudflare
```

The two real-cloud tests run only with their credentials. The job that runs them reports
`sandbox-cloudflare`, `kms-aws` and `kms-gcp` as separate checks, so a skip is visible.

## 7. Done when

- The live Cloudflare run is recorded with its date and result.
- `sandbox-cloudflare` is green on `master`.
- The AWS and GCP tests pass against the real services, and their workflow is green.
- The Vault and WebCrypto tests run in the normal jobs.
- #984 and #985 are closed.
