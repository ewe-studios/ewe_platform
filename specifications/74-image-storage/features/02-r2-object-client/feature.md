---
feature: "Cloudflare R2 object client with multipart upload, resume and checksums"
status: "planned"
priority: "medium"
depends_on: ["01-storage-config-and-manifest"]
crates: []
issues: []
model: "sonnet"
branch: "spec74/f02-r2-client"
---

# Feature 02: Cloudflare R2 object client

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Amazon Web Services (AWS); Cloudflare R2 (R2); command line
interface (CLI); continuous integration (CI); gigabyte (GB); gibibyte (GiB); Hypertext
Transfer Protocol (HTTP); input/output (I/O); JavaScript Object Notation (JSON); Secure
Sockets Layer (SSL); SHA-256; Uniform Resource Identifier (URI); Uniform Resource Locator
(URL); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE
standard ASD-STE100.

## 1. Goal

Provide one tested client that uploads, downloads, lists, presigns and deletes R2 objects.
The client streams. It never holds a disk in memory. It survives a broken multipart upload
and resumes from the completed parts. It verifies every byte with SHA-256.

## 2. Measured state

1. The repository has no R2 client. `wrangler.toml` defines a local R2 binding for the
   database tests only.
2. No CI image has the AWS CLI. Feature 05 adds the mise pin; this feature writes the code and
   is tested on a developer machine and in CI after the pin lands.
3. R2 accepts one PUT up to 5 GiB and multipart parts of 5 MiB to 5 GiB, with at most 10,000
   parts. A 20 GB disk therefore needs multipart upload.
4. R2 documents that the newest AWS CLI versions need
   `AWS_REQUEST_CHECKSUM_CALCULATION=when_required` and
   `AWS_RESPONSE_CHECKSUM_VALIDATION=when_required`. The smoke test proves the setting.

## 3. Requirements

1. The client is `ci/platform-images/scripts/r2.nu`. It calls the AWS CLI v2; it implements no
   signing and no HTTP.
2. It reads credentials only from the environment: `R2_ACCOUNT_ID`,
   `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`. It never takes a secret as an argument and
   never writes one to a file.
3. A missing credential is a typed error that names the missing variable. The client does not
   fall back.
4. Upload uses multipart for a file above the part size. The part size comes from the storage
   config. All parts except the last are the same size.
5. Upload is resumable. On a retry, the client lists the uploaded parts and sends only the
   missing parts. It resumes at most `max_retries` times with exponential backoff.
6. Upload prints the part count, the bytes and the elapsed seconds. It never prints a
   credential or a presigned URL with its signature.
7. Download verifies the size from `HeadObject` and the SHA-256 from the manifest. A mismatch
   deletes the partial file and raises a typed error with the expected and the actual value.
8. Download resumes with HTTP range requests after a failure.
9. `presign` returns a URL with the configured expiry. `list` returns keys and sizes.
   `delete` removes one object and reports a missing object as a typed error.
10. The client masks `R2_SECRET_ACCESS_KEY`, `R2_ACCESS_KEY_ID` and the signature part of a
    presigned URL in every message it prints.

## 4. Design

### 4.1 Surface

```nu
# r2.nu (sourced by storage.nu consumers)

export def "r2 upload" [
    storage: record,      # from load-storage
    source: path,
    key: string,
    --sha256: string = "",   # expected; computed when empty
]: nothing -> record     # { key, bytes, parts, sha256, seconds }

export def "r2 download" [
    storage: record,
    key: string,
    dest: path,
    --sha256: string,        # required: verify before returning
    --expected-bytes: int = -1,
]: nothing -> record     # { key, dest, bytes, seconds }

export def "r2 presign" [
    storage: record,
    key: string,
    --expires-s: int = 3600,
]: nothing -> string

export def "r2 list" [
    storage: record,
    prefix: string,
]: nothing -> list<record>  # { key, bytes, last_modified }

export def "r2 delete" [storage: record, key: string]: nothing -> nothing
```

### 4.2 Environment

```nu
AWS_ENDPOINT_URL = $"https://($account).r2.cloudflarestorage.com"
AWS_DEFAULT_REGION = "auto"
AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY   # from R2_* after the check
AWS_REQUEST_CHECKSUM_CALCULATION = "when_required"
AWS_RESPONSE_CHECKSUM_VALIDATION = "when_required"
```

The endpoint uses the S3 API host, not a custom domain. The client never uses a public
custom domain.

### 4.3 Upload algorithm

1. `HeadObject` the key. If it exists with the same size and the stored SHA-256 metadata
   matches, return `deduplicated` and do nothing. This makes a nightly publish cheap.
2. Compute the SHA-256 by streaming through `shasum -a 256` (macOS) or `sha256sum` (Linux).
   Never `open --raw` a disk file.
3. `CreateMultipartUpload` with metadata `sha256=<hex>`.
4. `ListParts`. The set of part numbers is the completed set.
5. For each part number from 1 to `ceil(bytes / part_bytes)` that is not completed: cut the
   part with `dd if=<source> bs=<part_bytes> skip=<n> count=1 of=<tmp>` and call
   `UploadPart`. The temporary file is at most one part (64 MiB by default). Delete it after
   the call.
6. `CompleteMultipartUpload` with the ETags from `ListParts` plus the new ones, sorted by part
   number.
7. On an error before complete: leave the multipart upload in place and retry. On a fatal
   error: `AbortMultipartUpload`, then raise.
8. Record the SHA-256 in the object metadata. The manifest is uploaded last by feature 04.

### 4.4 Download algorithm

1. `HeadObject` gives the size. Compare it with `--expected-bytes` when given.
2. If the destination exists with a size below the object size, resume with
   `GetObject --range bytes=<size>-` into a part file and append it with `cat`.
3. Otherwise download the whole object in one streamed `GetObject`.
4. Compute the SHA-256 with the external tool and compare with `--sha256`.
5. On a mismatch: delete the destination, raise an error with both values.

### 4.5 Redaction

Build every message through one function:

```nu
def redact [text: string]: nothing -> string
```

It replaces the value of `R2_SECRET_ACCESS_KEY`, the value of `R2_ACCESS_KEY_ID` and the
`X-Amz-Signature=` value in a URL with `***`. Every `print`, `error make` and progress line
uses it.

## 5. Files

| File | Change |
| --- | --- |
| `ci/platform-images/scripts/r2.nu` | New client. |
| `ci/platform-images/tests/r2_tests.nu` | New tests. |
| `ci/platform-images/tests/docker-compose.yml` | MinIO server, pinned image, one bucket, one access key. |
| `ci/platform-images/tests/mod.nu` | Start and stop MinIO for the R2 cases. |
| `ci/platform-images/README.md` | Section that names the client and the local test. |
| `mise.toml` | Pin `aws-cli` and `zstd` (feature 02 owns the pin for the client). |

## 6. Tasks

- [ ] Add the MinIO compose file with a pinned image and a fixed test credential.
- [ ] Write the credential check, the endpoint builder and `redact`.
- [ ] Write `r2 upload` with multipart, resume and deduplication.
- [ ] Write `r2 download` with range resume, size and SHA-256 verification.
- [ ] Write `r2 presign`, `r2 list` and `r2 delete`.
- [ ] Write the tests in section 7.
- [ ] Add the `aws-cli` and `zstd` pins to `mise.toml`.
- [ ] Document the environment variables and the redaction rule.

## 7. Tests

The tests run against MinIO over the S3 protocol. MinIO is the local form of the protocol; it
is not R2. Feature 07 proves the real service.

| Test | Asserts |
| --- | --- |
| `test r2 upload download round trip` | 200 MiB random file, 8 MiB parts; the downloaded SHA-256 matches. |
| `test r2 upload deduplicates` | A second upload of the same key makes no new parts. |
| `test r2 upload resumes` | Fail after part 3, run again; `list-parts` shows 3 before the retry; the complete object matches. |
| `test r2 upload uses multipart` | A file above the part size sends more than one part. |
| `test r2 download verifies size` | A wrong `--expected-bytes` fails and removes the file. |
| `test r2 download verifies sha` | A wrong `--sha256` fails, names both values and removes the file. |
| `test r2 presign` | A presigned GET returns 200 before the expiry and 403 after a 1 s expiry with a wait of 2 s. |
| `test r2 list prefix` | Returns only the keys under the prefix, with sizes. |
| `test r2 delete missing` | A missing key is a typed error with the key. |
| `test r2 redacts secrets` | A forced error message contains `***` and not the test secret. |
| `test r2 missing credential` | An unset variable is a typed error that names it. |

## 8. Verification

```bash
docker compose -f ci/platform-images/tests/docker-compose.yml up -d minio
mise run test:ci-storage -- --case r2
docker compose -f ci/platform-images/tests/docker-compose.yml down -v
```

Manually, with a real bucket and a 64 MiB file:

```bash
mise run storage:r2-smoke
```

## 9. Done when

- Every test in section 7 passes.
- A 200 MiB upload and download uses less than 200 MiB of memory for the client process. The
  part file is the only large temporary file.
- A forced network failure resumes without a new part 1.
- No message contains a credential.
- `progress.md` and `learnings.md` record the feature.

## 10. Risks

| Risk | Control |
| --- | --- |
| The AWS CLI loads a user profile and ignores the environment. | Pass `--no-sign-request` never; set `AWS_CONFIG_FILE=/dev/null` and `AWS_SHARED_CREDENTIALS_FILE=/dev/null` so no profile interferes. |
| `dd` blocks on a sparse or growing file. | The disk archive is written with the VM stopped; document the precondition. |
| The part size differs between the first upload and a resume. | The part size comes from the config; a mismatch with `ListParts` is a typed error and aborts the upload. |
| The checksum takes as long as the upload. | The SHA-256 is computed once per file; the manifest stores it; a deduplicated upload skips it. |
| MinIO accepts something R2 rejects. | Feature 07 runs the same client against the real bucket in the nightly workflow. |
