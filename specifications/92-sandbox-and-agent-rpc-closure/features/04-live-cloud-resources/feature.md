---
feature: "Live cloud resources with OpenTofu"
status: "pending"
priority: "high"
model: "sonnet"
phase: "2"
depends_on: ["00-live-proof"]
crates: ["foundation_auth"]
---

# Feature 04: Live cloud resources with OpenTofu

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations.** Key Management Service (KMS); OpenTofu (`tofu`); workload identity
federation (WIF); OpenID Connect (OIDC); Identity and Access Management (IAM); infrastructure
as code (IaC).

## 1. Goal

Prove AWS KMS and GCP KMS against the real services, with the test resources created and
destroyed by **OpenTofu**, so a run is repeatable and leaves nothing behind.

The tests do not create infrastructure by hand. One `tofu apply` makes a key, the test uses
it, and one `tofu destroy` removes it. The state file lives in a temporary directory and
never enters the repository.

## 2. The resources

`ci/live-kms/` holds one root module with two providers.

| Provider | Resource | Output |
|---|---|---|
| `aws` | `aws_kms_key` (symmetric, `ENCRYPT_DECRYPT`, rotation on) and `aws_kms_alias` `alias/ewe-live-test` | `aws_key_arn`, `aws_region` |
| `aws` | `aws_kms_key` for the "wrong key" case | `aws_wrong_key_arn` |
| `google` | `google_project_service` for `cloudkms.googleapis.com`, then `google_kms_key_ring` and `google_kms_crypto_key` | `gcp_key_name`, `gcp_project`, `gcp_location` |

Every resource carries the tag or label `purpose = "ewe-live-test"` and
`created-by = "tofu"`. A sweep task finds resources with that label, so an interrupted run is
still cleanable.

## 3. Tasks

| Task | Work |
|---|---|
| T1 | The module above, with variables `region`, `project`, `location` and `ttl_days` (default 1). A key policy that allows only the caller's principal. No secret in variables or outputs. |
| T2 | `mise` tasks: `live:up` runs `tofu init -input=false` and `tofu apply -auto-approve` in a temporary state directory and prints the outputs; `live:down` runs `tofu destroy -auto-approve`; `live:sweep` lists resources with the label and removes them. |
| T3 | A script `ci/live-kms/run-tests.sh`: `live:up`, export the outputs, run the four gated tests (AWS seal/open, AWS wrong key, GCP seal/open, GCP wrong key), then `live:down` in a `trap`, and print the result. It exits non-zero if a test fails; it destroys the resources either way. |
| T4 | The gated tests read `AWS_KMS_KEY_ARN`, `AWS_WRONG_KEY_ARN`, `GCP_KMS_KEY_NAME`, `GCP_KMS_PROJECT`, `GCP_KMS_LOCATION` and the credentials. A missing value prints its name and the test returns. |
| T5 | Optional CI: one workflow that runs `run-tests.sh` when the repository holds the AWS role and GCP WIF secrets, and says plainly when it did not run. |

## 4. What the operator provides

| Provider | Value | Least privilege |
|---|---|---|
| AWS | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` (or a role), `AWS_REGION` | `kms:CreateKey`, `CreateAlias`, `DeleteAlias`, `DescribeKey`, `Encrypt`, `Decrypt`, `GenerateDataKey`, `ScheduleKeyDeletion`, `TagResource` |
| GCP | `GOOGLE_APPLICATION_CREDENTIALS` or `gcloud auth application-default login`, `GCP_PROJECT` | `roles/cloudkms.admin` on the project, or on one key ring, plus `roles/serviceusage.serviceUsageAdmin` to enable the API |

A scratch account or project is enough. The resources cost near zero and are destroyed by
`live:down`.

## 5. Tests

| Test | Oracle |
|---|---|
| `aws_live_seal_and_open`, `aws_live_wrong_key` | the sealer contract, expected outcome |
| `gcp_live_seal_and_open`, `gcp_live_wrong_key` | the same |
| `live_resources_are_gone` (after `live:down`) | conservation: the key no longer exists |

## 6. Verify

```bash
tofu -chdir=ci/live-kms fmt -check
tofu -chdir=ci/live-kms validate
ci/live-kms/run-tests.sh      # with the credentials of the table
```

## 7. Done when

- `run-tests.sh` creates the resources, passes the four tests and destroys the resources.
- The resources are gone after the run, and `live:sweep` finds nothing.
- The result is recorded in `progress.md` with the date, the region, the project and the
  location.
