# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.4](https://github.com/ewe-studios/ewe_platform/compare/foundation_repl-v0.2.2...foundation_repl-v0.2.4) - 2026-11-12

### Added

- *(spec70)* phase D platform times come from the injected clock

### Fixed

- medium-severity defect batch (all open medium issues) ([#422](https://github.com/ewe-studios/ewe_platform/pull/422))
- *(foundation_toolings)* wake the builder on watch events and follow the injected clock
- *(spec70)* count one worker exit and re-raise a task panic
- *(spec70)* bound pool shutdown, require the Ctx clock, and repair CI

### Other

- Spec 76 waves 7 to 9 (db, netio, core take Ctx); import Ctx from foundation_context; retry a rate-limited publish ([#480](https://github.com/ewe-studios/ewe_platform/pull/480))
- Fix master's Cloudflare codegen check; spec 76 wave 1 (nine crates take Ctx) ([#473](https://github.com/ewe-studios/ewe_platform/pull/473))
- Spec 76 feature 02 (one Ctx, RpcCtx) and a version bump of every crate ([#467](https://github.com/ewe-studios/ewe_platform/pull/467))
- *(spec 70)* move inline tests out of five lane B crates (feature 59)
- run cargo fmt --all under the pinned rustfmt (feature 39)
