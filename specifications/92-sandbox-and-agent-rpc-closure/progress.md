# Spec 92 Progress: closing specs 77 and 83

_Last updated: 2026-11-27._

## Status at a glance

| Feature | State |
|---|---|
| 00 Live proof on the real services | in progress: pull request #1075 (the Cloudflare Worker path), pull request #1055 (moto for AWS KMS) |
| 01 DST fidelity: the real registry | stage A in progress: branch `fix/dst-wait-redrive`; stage B not started |
| 02 Sandbox follow-ups | in progress: branch `fix/sandbox-follow-ups`; issues #1104 and #1105 now belong to it |
| 03 GCP KMS over gRPC | in progress: branch `feat/kms-grpc`, based on `feat/live-kms` |
| 04 Live cloud resources with the CLI | in progress: pull request #1103; AWS passed live, GCP waits for billing |

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

## Today (2026-11-27, evening)

- **Feature 04: the AWS half passed live.** `foundation-ci live-kms` made two keys and two
  aliases in Amazon Web Services (AWS), from `AWS_REGION` in the shell. The seal and open
  tests passed, the wrong key gave HTTP 400, and the teardown removed every resource.
  Pull request #1103 holds the task and the four gated tests.
- **Feature 04: the GCP half waits for billing.** `gcloud kms keyrings list` still returns
  `FAILED_PRECONDITION: Billing is disabled for project 287273334770` for
  `ewestudiosnigeria`, and the same for `artfforts` (855260392529). Billing account
  `01DEB9-857B6C-880803` is open, but no project links to it. The owner says billing is on;
  the API does not agree yet. The GCP proof runs when the call succeeds.
- **Feature 02: two new findings from the continuous integration (CI) log.** Issue #1104
  (`severity: high`) records the `serve::tcp_*` hang that held the `sandbox-e2e` job for
  two hours. Issue #1105 (`severity: medium`) records the `subreaper_adopts` failure.
- **Feature 01 is split in two stages.** Stage A fixes the `Stream::Wait` arm on branch
  `fix/dst-wait-redrive`. Stage B moves the real registry under the simulator (#908).
- **The merge order of the CI agent:** #1102, then master into #1075, then #1075, then
  master into #1055, then #1055, then #1049, then #1103, then one full `packages=all` run.

## Next

1. Feature 01 stage A: review the pull request and route any failing check.
2. Feature 03: the protobuf messages, the gRPC protocol, the localgcp task and its CI job.
3. Feature 04: run the GCP half when the billing link exists.
4. Feature 00: make `sandbox-cloudflare` green through #1075.
5. The Cloudflare demo under `examples/`, after #1075 merges.
