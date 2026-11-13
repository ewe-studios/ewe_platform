# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.5](https://github.com/ewe-studios/ewe_platform/compare/foundation_deployment_hetzner-v0.1.3...foundation_deployment_hetzner-v0.1.5) - 2026-11-12

### Added

- *(spec70)* phase D platform times come from the injected clock
- *(spec70)* phase C service times come from the injected clock
- *(valtron)* name a task poll that blocks a worker
- *(foundation_core)* add a close-only pipe readiness
- *(core)* read every timestamp from the injected Clock
- *(valtron)* make the engine clock replaceable at startup
- *(sse)* write SSE events through the async ResponseWriter

### Fixed

- medium-severity defect batch (all open medium issues) ([#422](https://github.com/ewe-studios/ewe_platform/pull/422))
- *(netio)* compute the load tracker connection rate
- *(foundation_toolings)* wake the builder on watch events and follow the injected clock
- *(deployment_platform)* take the Docker container clock from the caller
- *(spec70)* count one worker exit and re-raise a task panic
- *(spec70)* bound pool shutdown, require the Ctx clock, and repair CI
- *(spec70)* phase B reads engine and I/O time from the injected clock
- *(valtron)* prune a bridged waker when its wait is gone
- *(io-63)* retire the truncating sync response path
- *(genapi)* honour the declared selection and pass the profile through
- *(release)* bump valtron version group to 0.0.7 and land features 38 and 40
- *(auth)* revert the incomplete sha2 0.11 bump across the workspace

### Other

- Fix ten medium issues from the spec 76 work: Ctx in valtron waits and the netio sync client, Windows builds, CI coverage ([#503](https://github.com/ewe-studios/ewe_platform/pull/503))
- Spec 76 waves 7 to 9 (db, netio, core take Ctx); import Ctx from foundation_context; retry a rate-limited publish ([#480](https://github.com/ewe-studios/ewe_platform/pull/480))
- Spec 76 waves 2 to 6: eleven crates take Ctx instead of a clock ([#476](https://github.com/ewe-studios/ewe_platform/pull/476))
- Spec 76 feature 02 (one Ctx, RpcCtx) and a version bump of every crate ([#467](https://github.com/ewe-studios/ewe_platform/pull/467))
- *(release)* bump foundation_http and foundation_netio to 0.0.9
- *(foundation_core)* bump to 0.0.13 and update every consumer
- run cargo fmt --all under the pinned rustfmt (feature 39)
- *(db)* bump the three table-syntax foundation_db consumers to 0.0.5
- *(deps)* bump sha2 from 0.10.9 to 0.11.0
- deployable test awaits the async state-store init
- label-value tests out of deployable.rs
- WIP #140: move HTTP callers to DynNetClient / HttpClientBuilder (not yet verified)
- async StateStore; deployment crates and proxy await it (spec 50 F04 T8)
