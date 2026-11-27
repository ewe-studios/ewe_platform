# Spec 92 — Closing specs 77 and 83

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations.** continuous integration (CI); Cloudflare Worker (Worker); Durable Object
(DO); Key Management Service (KMS); deterministic simulation testing (DST); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Status: **draft**, owner direction 2026-11-27.

## 1. Goal

Turn "implemented" into "proven and clean" for spec 77 (the sandbox) and spec 83 (agent RPC).

Every feature of the two specs is merged on `master`. This spec adds no capability. It does
three things:

1. Run the real services and the real environments in CI, and make those jobs green.
2. Finish the one design gap that changes what the tests prove (the DST harness).
3. Close the recorded correctness and coverage findings, oldest first.

A feature of this spec is done when its issues are closed, not when its code compiles.

## 2. Scope

| # | Feature | What it closes |
|---|---|---|
| 00 | [Live proof](features/00-live-proof/feature.md) | The Cloudflare live path, the red `sandbox-cloudflare` job, the AWS and GCP KMS tests, the WebCrypto runtime test |
| 01 | [DST fidelity](features/01-dst-fidelity/feature.md) | #908 part (b): the DST `TestWorld` runs the real registry and session |
| 02 | [Sandbox follow-ups](features/02-sandbox-follow-ups/feature.md) | #977, #978, #966, #969, #970, #972, #1013, #1022, #1023, #935, #1008 |
| 03 | [GCP KMS over gRPC](features/03-gcp-kms-grpc/feature.md) | The Cloud KMS protos, a gRPC path in `GcpKmsBackend`, and localgcp as the local emulator (#984, GCP half) |
| 04 | [Live cloud resources with the cloud CLI](features/04-live-cloud-resources/feature.md) | Real AWS and GCP KMS resources, created and destroyed by the cloud CLI, for the gated proof (#984) |

Out of scope:

- Feature 16 (Docker Sandboxes, `sbx`) is rejected by the owner (spec 77 decision D18).
- New drivers, new deploy targets and new sandbox images.
- Spec 69 work, which another person owns.
- The `deno_*` set and issue #139 (replace `deno_core` with the `v8` crate). The study is
  done: Deno is only the embedded test runtime in `foundation_testbed`, the move is 2,300 to
  3,000 new lines, and no `deno_*` alert is open. The recommendation is to bump the whole
  Deno set to `deno_core` 0.413 as one group and keep #139 as the contingency; the assessment
  is recorded on #139. This spec does not carry that bump.

## 3. Rules

1. Every task names the issue it closes.
2. A test against a real service uses that service, not a stub (specifications rule 3). When
   no local form exists, the test is ignored by default and runs in a release workflow.
3. A secret never enters the repository, a log, a plan or a test fixture. Read it from the
   environment; the owner keeps `CLOUDFLARE_ACCOUNT_ID` and
   `CLOUDFLARE_WORKERS_AI_API_TOKEN` in the shell and in `~/.userenv`.
4. **Local first.** A test that needs a real service runs on a developer machine against that
   service. A missing credential makes the test print which value is missing and return; it
   never makes the test fail, and a skip is not a pass. CI runs such a test only when the
   owner adds the secret; the workflow gates on it and says when it did not run.
5. A change that needs a new credential names the exact scope in its task. The operator
   grants the least privilege.
6. Keep the spec's `progress.md` and `learnings.md` current (spec 77 rule 10).
7. Every bug found becomes an issue with a severity (spec 77 rule 9).

## 4. Credentials the features need

| Feature | Credential | Scope |
|---|---|---|
| 00 Cloudflare live | `CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID` | Workers Scripts Edit, D1 Edit, R2 Edit, Workers AI Read |
| 00 AWS KMS | `AWS_ROLE_ARN` through GitHub OIDC, or `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` for a scratch account | `kms:CreateKey`, `Encrypt`, `Decrypt`, `DescribeKey`, `GenerateDataKey`, `ScheduleKeyDeletion` |
| 00 GCP KMS | `GOOGLE_APPLICATION_CREDENTIALS` through workload identity federation | `roles/cloudkms.admin` on one key ring, for the test to create and delete keys |
| 00 Vault | none: the dev server runs in Docker | — |

The owner already provides the first row. The AWS and GCP rows are new; the feature says what
happens when they are absent (the test prints the reason and returns).

## 5. Done when

- The full `check.yaml` run with `packages=all` is green on `master`, including
  `sandbox-cloudflare` and `sandbox-e2e`.
- The Cloudflare live test runs against the owner's account and records its date and result.
- The AWS KMS and GCP KMS tests pass against the real services in a manual workflow, and the
  Vault test passes in the normal run.
- The WebCrypto sealer has a runtime test in the browser job.
- #908 part (b) lands: the DST `TestWorld` uses the real registry, and 32 seeds give the same
  trace twice.
- The ten issues of feature 02 are closed.

## 6. Order

1. Feature 00 first: the red job blocks every claim about the Cloudflare path.
2. Feature 03 after it: the GCP gRPC path and localgcp, so the KMS tests run locally.
3. Feature 04: the live resources with the cloud CLI, when the operator provides the credentials.
4. Feature 01: it changes what the DST proves and touches `foundation_ai` and
   `foundation_core`.
5. Feature 02 last: a list of independent, small corrections, in severity order.
