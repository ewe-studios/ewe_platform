---
spec: "specifications/74-image-storage"
created: 2026-11-08
updated: 2026-11-08
---

# Progress: Spec 74

_Written in ASD-STE100 Simplified Technical English (STE)._

**Abbreviations in this file.** continuous integration (CI), Cloudflare R2 (R2), GitHub
Container Registry (GHCR), Open Container Initiative (OCI), pull request (PR), virtual
machine (VM), AeroSpace and Defence Industries Association of Europe (ASD), the owner of the
STE standard ASD-STE100.

## Status: planned, awaiting owner approval

Research is complete. No feature has started. The requirements file holds the measured
state, the options analysis and the cost estimate.

## What is decided

| Question | Answer in this spec |
| --- | --- |
| Can Cloudflare store a Docker repository? | Not directly. R2 stores objects. A Worker can speak OCI, but the proxied body limit is 100 MB and large-layer pushes are fragile. |
| Does Docker Hub support our image sizes? | Yes for the current public images. The largest image is 7.89 GiB. The commonly reported layer cap is 10 GB. Measure before a migration. |
| What is the cheapest plan? | Public OCI images: Docker Hub or GHCR, $0. VM disks and large artifacts: private R2, about $0.45–$1.13 per month at the expected sizes. |
| Is an OCI registry on R2 recommended? | No, not now. Revisit only if the registry fails the size or rate test in feature 07. |

## Feature progress

| Feature | Status | Branch | Note |
| --- | --- | --- | --- |
| 01 storage configuration and manifest | planned | — | Blocks 02 and 04. |
| 02 R2 object client | planned | — | Waits for 01. |
| 03 registry backend | planned | — | Independent of R2. |
| 04 VM disk store | planned | — | Needs the owner licence decision. |
| 05 CI wiring and documentation | planned | — | After 03 and 04. |
| 06 cost report and lifecycle | planned | — | After 02. |
| 07 verification and migration | planned | — | Last. |

## Next step

The owner answers the four decisions in `requirements.md` section 13. Then the main session
reviews this spec and approves the start of feature 01.

## Measured facts to keep

- Docker Hub compressed sizes on 2026-11-08: `ci-base` 1.40 GiB, `ci-base-cuda` 6.57 GiB,
  `ewe-platform-ci` 2.72 GiB, `ewe-platform-ci-cuda` 7.89 GiB, the four VM images 0.12–0.16
  GiB each. One full set is 19.14 GiB.
- R2: $0.015 per GiB-month, 10 GiB-month free, $0 egress, 5 TiB object, 5 GiB part.
- Docker Hub pull limit: 100 per 6 hours anonymous, 200 per 6 hours with a free login.
- The repository already uses a local R2 binding in `wrangler.toml` for tests.
