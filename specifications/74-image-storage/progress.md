---
spec: "specifications/74-image-storage"
created: 2026-11-08
updated: 2026-11-23
---

# Progress: Spec 74

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations in this file.** continuous integration (CI), Cloudflare R2 (R2), GitHub
Container Registry (GHCR), Open Container Initiative (OCI), pull request (PR), virtual
machine (VM), AeroSpace and Defence Industries Association of Europe (ASD), the owner of the
STE standard ASD-STE100.

## Status: owner decisions recorded (2026-11-23), awaiting approval to start feature 01

Research is complete. The owner answered the four decisions in `requirements.md` section 13.
No feature has started. Feature 04's design is updated for the owner's single-image
direction and needs a review before it starts.

## What is decided

| Question | Decision |
| --- | --- |
| Registry | Keep Docker Hub as the default push target. Feature 03 adds the selectable backend. |
| VM state | Build one image for the volume and the installed software. Do not copy a vendor OS disk. |
| Retention | One version per OS and base. |
| Cost | The $5 per month threshold is accepted. Free options come first. |
| Bulk store | R2, S3, GCS, MEGA or a local path, selected by one config field. |
| File format | `tar.zst` by default; `qcow2`, `ova`, `vhdx` and `raw` are selectable. |

Measured facts that do not change:

| Question | Answer in this spec |
| --- | --- |
| Can Cloudflare store a Docker repository? | Not directly. R2 stores objects. A Worker can speak OCI, but the proxied body limit is 100 MB and large-layer pushes are fragile. |
| Does Docker Hub support our image sizes? | Yes for the current public images. The largest image is 7.89 GiB. The commonly reported layer cap is 10 GB. Measure before a migration. |
| What is the cheapest plan? | Public OCI images: Docker Hub or GHCR, $0. VM volume images and build artifacts: the free tiers first, then R2 at about $0.45–$1.13 per month at the expected sizes. |
| Is an OCI registry on R2 recommended? | No, not now. Revisit only if the registry fails the size or rate test in feature 07. |

## Feature progress

| Feature | Status | Branch | Note |
| --- | --- | --- | --- |
| 01 storage configuration and manifest | planned | — | Blocks 02 and 04. |
| 02 bulk store client | planned | — | Waits for 01. One contract for R2, S3, GCS, MEGA and local. |
| 03 registry backend | planned | — | Independent of R2. |
| 04 One image for the VM volume | planned | — | Uses 01 and 03. Design updated; needs review. |
| 05 CI wiring and documentation | planned | — | After 03 and 04. |
| 06 cost report and lifecycle | planned | — | After 02. |
| 07 verification and migration | planned | — | Last. |

## Next step

1. Review the updated feature 04 design (`features/04-vm-volume-image/feature.md`).
2. Approve the start of feature 01.
3. Start feature 01 on branch `spec74/f01-storage-config`.

## Measured facts to keep

- Docker Hub compressed sizes on 2026-11-08: `ci-base` 1.40 GiB, `ci-base-cuda` 6.57 GiB,
  `ewe-platform-ci` 2.72 GiB, `ewe-platform-ci-cuda` 7.89 GiB, the four VM images 0.12–0.16
  GiB each. One full set is 19.14 GiB.
- R2: $0.015 per GiB-month, 10 GiB-month free, $0 egress, 5 TiB object, 5 GiB part.
- Docker Hub pull limit: 100 per 6 hours anonymous, 200 per 6 hours with a free login.
- The repository already uses a local R2 binding in `wrangler.toml` for tests.
