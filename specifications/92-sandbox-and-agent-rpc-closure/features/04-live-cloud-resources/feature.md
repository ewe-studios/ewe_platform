---
feature: "Live cloud resources with the cloud CLI"
status: "pending"
priority: "high"
model: "sonnet"
phase: "2"
depends_on: ["00-live-proof"]
crates: ["foundation_auth"]
---

# Feature 04: Live cloud resources with the cloud CLI

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations.** Key Management Service (KMS); command-line interface (CLI); workload
identity federation (WIF); OpenID Connect (OIDC); Identity and Access Management (IAM).

## 1. Goal

Prove AWS KMS and GCP KMS against the real services, with the test resources created and
destroyed by the cloud command-line Interface (CLI), so a run is repeatable and leaves nothing
behind.

The owner chose the CLI over OpenTofu: both CLIs are already authenticated on the machine, the
resources are two keys and one key ring, and a CLI script has no state file to leak or drift.
The script creates the resources, runs the tests, and deletes them in a `trap`, pass or fail.

## 2. The resources

The task `foundation-ci live-kms` makes the resources with the CLI. It lives in `foundation_ci` on the task system (`foundation_tasks` `TaskSet`), like every other CI task, so one command runs the same steps locally and in CI. A shell script is not allowed in its place.

| Cloud | Command | Result |
|---|---|---|
| AWS | `aws kms create-key` twice, then `aws kms create-alias` | two ARNs: `alias/ewe-live-test`, `alias/ewe-live-test-wrong` |
| AWS teardown | `aws kms schedule-key-deletion --pending-window-in-days 7`, `aws kms delete-alias` | the key is deleted at the window's end; it costs nothing before that |
| GCP | `gcloud kms keyrings create ewe-live-test`, then `gcloud kms keys create ewe-live-test` | one key name |
| GCP teardown | `gcloud kms keys versions destroy`, `gcloud kms keys destroy` | the key is `DESTROY_SCHEDULED`; its versions are gone |

Every resource carries the tag or label `purpose=ewe-live-test`. A key ring cannot be deleted
in Google Cloud; it costs nothing and stays listed. A scratch project is the way to remove it
completely, and the script says so in its header.

## 3. Tasks

| Task | Work |
|---|---|
| T1 | The create and delete commands of section 2, with `region`, `project` and `location` read from the environment. A key policy that allows only the caller's principal. No secret in a variable, an output or a log. |
| T2 | The script's `up` and `down` steps are the CLI commands of section 2; `sweep` lists keys with the `purpose=ewe-live-test` tag or label and schedules their deletion. |
| T3 | `foundation-ci live-kms` in `backends/foundation_ci/src/execution/live_kms.rs`: create the resources, export the values, run the four gated tests (AWS seal/open, AWS wrong key, GCP seal/open, GCP wrong key), then delete the resources from a guard, and print the result. A failed test still runs the teardown; `--keep` is the only skip. Every process goes through `ctx.exec`. |
| T4 | The gated tests read `AWS_KMS_KEY_ARN`, `AWS_WRONG_KEY_ARN`, `GCP_KMS_KEY_NAME`, `GCP_KMS_PROJECT`, `GCP_KMS_LOCATION` and the credentials. A missing value prints its name and the test returns. |
| T5 | Optional CI: one workflow that runs `foundation-ci live-kms` when the repository holds the AWS role and GCP WIF secrets, and says plainly when it did not run. |

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
cargo run --locked --profile uat -p foundation_ci -- list | grep live-kms
cargo run --locked --profile uat -p foundation_ci -- live-kms   # with the credentials of the table
```

## 7. Done when

- `foundation-ci live-kms` creates the resources, passes the four tests and deletes the resources.
- The resources are gone after the run, and `live:sweep` finds nothing.
- The result is recorded in `progress.md` with the date, the region, the project and the
  location.
