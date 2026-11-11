---
spec: "specifications/74-image-storage"
created: 2026-11-08
updated: 2026-11-08
---

# Learnings: Spec 74

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations in this file.** continuous integration (CI), Cloudflare R2 (R2), Docker Hub
(DH), GitHub Container Registry (GHCR), Open Container Initiative (OCI), S3 (the object
storage protocol of Amazon), virtual machine (VM), AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## Research findings

### Cloudflare is not a Docker registry

R2 is object storage with the S3 protocol. It has no OCI registry API. Three ways exist to
put a registry in front of it. Each has a large cost:

1. **A Worker (`serverless-registry`).** The proxied request body limit is 100 MB on a Free
   or Pro zone. A normal `docker push` sends each layer as one request. A 1.4–7.9 GiB layer
   fails with error 413. A client that sends OCI chunked uploads (`regctl`,
   `registry-push`) works around the limit, but the Worker must also finalise objects above
   about 4 GiB, and the reference project has reported defects there (issue 42). The Free
   plan gives 100,000 requests per day, and each push and pull uses many requests.
2. **The `registry:2` binary with the R2 S3 driver.** This works and is documented. It needs
   a server to host the HTTP front. The server becomes a bottleneck and a component to
   secure and monitor. It adds no value over Docker Hub or GHCR for public images.
3. **A static OCI layout in R2.** This works for pulls with `skopeo`/`regctl` and a custom
   domain. It has no push API and awkward authentication. It is a mirror, not a store.

**Consequence.** Keep OCI images in an OCI registry. Use R2 for artifacts that are not OCI
images. This is the main design decision of the spec.

### R2 is a very good object store for disks

The decisive property is the zero egress fee. A 20 GB disk that a CI job downloads many times
costs $0 for egress. S3 and Docker Hub private storage charge for that traffic. The storage
price is $0.015 per GiB-month with 10 GiB free. Our expected 80 GiB of disks cost about
$1.05 per month.

R2 accepts objects of 5 TiB and parts of 5 GiB. A 20 GB disk needs 320 parts at 64 MiB. This
is far below the 10,000-part limit. A single PUT is limited to 5 GiB, so the client must use
multipart upload, never a signed single PUT of the whole disk.

### Docker Hub is free for the current public images

Docker Hub public repositories are free and unlimited in number. The current images total
19.14 GiB for one tag of each. The account also holds moving tags, hash tags and build
caches, so 40–60 GiB is the expected stored size. The free plan has no storage charge today.
Docker says storage billing is delayed with advance notice.

The free plan has two weaknesses for CI:

- **Pull limits.** 100 pulls per 6 hours without a login and 200 per 6 hours with a free
  login, per account. A matrix of 40 packages can use a large part of that budget. The
  existing `images.nu exists` function already retries on HTTP 429 (`images.nu:359`).
- **One private repository.** A provisioned OS disk needs a private repository. The free
  plan cannot hold two OS disks.

A private disk on Docker Hub also meets the 10 GB per-layer limit problem. The disk is one
stream of 15–20 GB. It would need to be split into layers, and the private storage costs
$9 per user per month. R2 is the better store for it.

### GHCR is the best zero-cost registry when the images are public

GHCR public packages are free with unlimited storage and bandwidth, and no pull limit.
`GITHUB_TOKEN` authenticates a push or pull inside GitHub Actions. The VM *container images*
contain no OS bits, so they can be public. Only the provisioned disks must stay private.
Private GHCR packages are currently free, but GitHub has not promised a free tier. The spec
therefore keeps the registry choice as configuration and does not depend on free private
GHCR storage.

### The repository already has the Cloudflare pieces

`wrangler.toml` defines a local D1 database and a local R2 bucket for the database tests.
The `wrangler` version is pinned in `mise.toml`. No workflow has a Cloudflare API token. The
spec needs one scoped R2 token and four repository values. It needs no Worker, no domain and
no API token for the Cloudflare control plane.

### There is no local R2 emulator with the S3 protocol

`wrangler dev --local` provides an R2 binding to a Worker and a Cloudflare-shaped REST API
for the tests. It does not expose the S3 protocol that `aws s3` or `rclone` uses. A Nushell
client that uses S3 cannot test against it. The local test uses MinIO, which speaks the S3
protocol that R2 documents. The real-R2 smoke workflow is the proof of R2 compatibility.
This follows rule 3: use the real local service when one exists; here the local real service
is MinIO for the S3 protocol, and the real R2 bucket is tested in a gated workflow.

### The macOS and Windows images do not contain operating system bits

dockurr downloads the installer from the vendor on first boot. The image is 0.12–0.16 GiB.
The repository README already explains why a provisioned disk is not a public image. The
archive in R2 changes nothing about that licence position. It only removes the repeat
install cost.

### Existing per-run installs are mostly correct

The iOS SDK question (2026-11-08) found that the macOS provisioning script installs the
pinned iOS platform once, into a manually installed full Xcode, and records it in
`/usr/local/platform-images/status/ios-sdk.txt`. The `macos-ssh` job only verifies it. Two
installs remain per run:

1. The Rust toolchain and the two mobile targets. This is deliberate. The image ships rustup
   without a toolchain, and the repository's `rust-toolchain.toml` selects the version.
   Baking it would make the image stale on every toolchain bump.
2. The Android NDK (`scripts/setup/android-ndk.sh`). This is a candidate to bake in the same
   way as the iOS platform. It is not part of this spec; report it as a separate improvement.

## Dead ends

1. **R2 as a push-heavy OCI registry.** Rejected after the 100 MB body limit and the
   reported large-layer finalisation defects. Do not retry without a new Cloudflare API.
2. **Docker Hub private storage for disks.** Rejected on cost, the 10 GB layer limit and the
   licence position.
3. **A `registry:2` server on the self-hosted Apple host.** Rejected because it adds a server
   and gives no cost or reliability gain for public images.
4. **Uploading a disk with one signed PUT.** Rejected because the R2 single-request limit is
   5 GiB, below a compressed disk. Multipart is mandatory.
