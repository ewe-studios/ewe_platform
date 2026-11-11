---
feature: "Cost report, lifecycle rules and prune schedule"
status: "planned"
priority: "medium"
depends_on: ["02-r2-object-client"]
crates: []
issues: []
model: "haiku"
branch: "spec74/f06-cost-report"
---

# Feature 06: Cost report, lifecycle rules and prune schedule

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Cloudflare R2 (R2); continuous integration (CI); gigabyte
(GB); gibibyte (GiB); JavaScript Object Notation (JSON); terabyte (TB); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Show the owner what the stores cost. Stop the bucket from growing without limit. The owner
sees a monthly line with the stored bytes and the estimated cost, and the bucket cleans its
own temporary data.

## 2. Measured state

1. R2 has no report in this repository.
2. The expected steady state is 60 GiB of OCI images (in a registry, not R2) plus 80 GiB of
   disks in R2 with two versions of each OS. R2 charges about $1.05 per month for 80 GiB
   after the 10 GiB free tier.
3. R2 lifecycle rules can expire a prefix and abort incomplete multipart uploads. They cannot
   keep "the newest N" versions. Feature 01 owns that logic.
4. The repository's `wrangler.toml` is for local tests only. The report uses the S3 API, not
   `wrangler`.

## 3. Requirements

1. `nu ci/platform-images/scripts/r2.nu report --config <path>`:
   - lists `disks/`, `buildcache/` and `tmp/` with sizes;
   - sums the bytes per prefix and in total;
   - computes the estimated cost: `max(0, GiB − 10) × price_per_gib`, with the price a config
     field (default 0.015);
   - prints a table and a one-line total;
   - writes `reports/<YYYY-MM>.json` with the same values, the price and the date;
   - exits 1 when the total is above the owner threshold (`[storage.r2] cost_alert_usd`,
     default 5) and prints the reason.
2. The report requires only list and head operations. It does not download an object.
3. `ci/platform-images/r2-lifecycle.json` holds the lifecycle configuration:
   - expire `tmp/` objects after 1 day;
   - abort incomplete multipart uploads after 1 day;
   - no rule for `disks/` or `buildcache/`.
   The README gives the `aws s3api put-bucket-lifecycle-configuration` command, with the
   endpoint.
4. A monthly workflow job runs `report` and `vm.nu disk prune` for each OS and base hash. It
   writes the report to the job summary and keeps the JSON as an artifact for 90 days.
5. The report masks credentials like every other client command.

## 4. Design

### 4.1 Output

```
prefix            objects        bytes      GiB
disks/                  6    85899345920   80.00
buildcache/             2     5368709120    5.00
tmp/                    0             0     0.00
total                   8    91268055040   85.00

R2 Standard storage: 85.00 GiB, 10.00 GiB free, 75.00 GiB billed
Price: $0.015 per GiB-month
Estimated: $1.13 per month
Threshold: $5.00 (below)
```

### 4.2 JSON

```json
{
  "schema": 1,
  "generated_at": "2026-12-01T04:17:00Z",
  "month": "2026-12",
  "price_per_gib": 0.015,
  "free_gib": 10,
  "prefixes": {
    "disks/": {"objects": 6, "bytes": 85899345920},
    "buildcache/": {"objects": 2, "bytes": 5368709120},
    "tmp/": {"objects": 0, "bytes": 0}
  },
  "total_bytes": 91268055040,
  "estimated_usd": 1.13,
  "threshold_usd": 5.0,
  "over_threshold": false
}
```

The JSON is small; parse it in full. The listing is streamed in pages of 1000 keys.

### 4.3 Lifecycle

```json
{
  "Rules": [
    {
      "ID": "expire-tmp",
      "Status": "Enabled",
      "Filter": {"Prefix": "tmp/"},
      "Expiration": {"Days": 1},
      "AbortIncompleteMultipartUpload": {"DaysAfterInitiation": 1}
    }
  ]
}
```

R2 applies a lifecycle rule to new objects within a short delay. The report counts a `tmp/`
object until the rule removes it.

### 4.4 Schedule

The monthly job runs on the first day of the month at 04:17 UTC in
`.github/workflows/storage-r2-smoke.yaml` or in a separate
`.github/workflows/storage-report.yaml`. Use a separate workflow, so a smoke failure does not
block the report.

## 5. Files

| File | Change |
| --- | --- |
| `ci/platform-images/scripts/r2.nu` | Add `main report`. |
| `ci/platform-images/r2-lifecycle.json` | New lifecycle configuration. |
| `.github/workflows/storage-report.yaml` | New monthly workflow. |
| `ci/platform-images/README.md` | Lifecycle command; report command; threshold. |
| `ci/platform-images/tests/r2_tests.nu` | Report tests. |

## 6. Tasks

- [ ] Write `main report` with paged listing and the cost formula.
- [ ] Add the JSON output and the threshold exit.
- [ ] Add the lifecycle JSON and the README command.
- [ ] Add the monthly workflow with the artifact and the job summary.
- [ ] Add the report tests.
- [ ] Run the report against the real bucket once and record the result in `progress.md`.

## 7. Tests

| Test | Asserts |
| --- | --- |
| `test report sums prefixes` | A fixture with three prefixes gives the exact bytes and object count. |
| `test report cost formula` | 5 GiB gives $0; 85 GiB gives $1.125 rounded to $1.13; 0 GiB gives $0. |
| `test report threshold` | A total above the threshold exits 1 and names the threshold. |
| `test report pages` | A fixture with 2500 keys needs three list calls and counts all keys. |
| `test report writes json` | The JSON has the schema, month, price and total. |
| `test report redacts` | A forced error does not contain a secret. |

## 8. Verification

```bash
docker compose -f ci/platform-images/tests/docker-compose.yml up -d minio
mise run test:ci-storage -- --case report
mise run storage:report
```

## 9. Done when

- The report prints the table and the estimate from the real bucket.
- The lifecycle rule exists on the bucket and `tmp/` objects expire.
- The monthly workflow writes the report to the summary and keeps the artifact.
- The threshold is documented and configurable.
- `progress.md` and `learnings.md` record the feature and the first real cost.

## 10. Risks

| Risk | Control |
| --- | --- |
| A listing of a large bucket hits the free operation tier. | Paged listing; one report per month; the operation count is in the output. |
| The lifecycle rule expires a disk. | The rule names only `tmp/`; a review checks the JSON before apply. |
| The estimate differs from the Cloudflare invoice (minimum storage, IA class). | The report names Standard class and the price date; the invoice is the authority. |
| The monthly workflow fails without secrets on a fork. | The same secrets check as the smoke workflow. |
