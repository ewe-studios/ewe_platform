# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.7](https://github.com/ewe-studios/ewe_platform/compare/foundation_html-v0.0.4...foundation_html-v0.0.7) - 2026-11-13

### Added

- *(valtron)* name a task poll that blocks a worker
- *(foundation_core)* add a close-only pipe readiness
- *(core)* read every timestamp from the injected Clock
- *(valtron)* make the engine clock replaceable at startup

### Fixed

- medium-severity defect batch (all open medium issues) ([#422](https://github.com/ewe-studios/ewe_platform/pull/422))
- *(foundation_toolings)* wake the builder on watch events and follow the injected clock
- *(spec70)* count one worker exit and re-raise a task panic
- *(spec70)* bound pool shutdown, require the Ctx clock, and repair CI
- *(spec70)* phase B reads engine and I/O time from the injected clock
- *(valtron)* prune a bridged waker when its wait is gone
- *(html)* report errors through foundation_errstacks
- *(release)* bump valtron version group to 0.0.7 and land features 38 and 40

### Other

- Spec 76 feature 02 (one Ctx, RpcCtx) and a version bump of every crate ([#467](https://github.com/ewe-studios/ewe_platform/pull/467))
- *(foundation_core)* bump to 0.0.13 and update every consumer
- *(spec 70)* move inline tests out of foundation_html (feature 59)
- run cargo fmt --all under the pinned rustfmt (feature 39)
