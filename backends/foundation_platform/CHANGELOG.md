# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.6](https://github.com/ewe-studios/ewe_platform/compare/foundation_platform-v0.0.1...foundation_platform-v0.0.6) - 2026-11-12

### Added

- *(spec70)* phase D platform times come from the injected clock
- *(sse)* write SSE events through the async ResponseWriter

### Fixed

- medium-severity defect batch (all open medium issues) ([#422](https://github.com/ewe-studios/ewe_platform/pull/422))
- *(netio)* compute the load tracker connection rate
- *(foundation_toolings)* wake the builder on watch events and follow the injected clock
- *(foundation_wasm)* the WASI host installs a TimeSource ([#219](https://github.com/ewe-studios/ewe_platform/pull/219))
- *(spec70)* count one worker exit and re-raise a task panic
- *(spec70)* bound pool shutdown, require the Ctx clock, and repair CI
- *(spec70)* phase B reads engine and I/O time from the injected clock
- *(core)* allow a loaded host to fire a chain of timers late
- *(io-63)* retire the truncating sync response path

### Other

- Fix master: clear the clippy lints that rust-version 1.98 turned on ([#502](https://github.com/ewe-studios/ewe_platform/pull/502))
- Spec 76 waves 7 to 9 (db, netio, core take Ctx); import Ctx from foundation_context; retry a rate-limited publish ([#480](https://github.com/ewe-studios/ewe_platform/pull/480))
- Fix master's Cloudflare codegen check; spec 76 wave 1 (nine crates take Ctx) ([#473](https://github.com/ewe-studios/ewe_platform/pull/473))
- Spec 76 feature 02 (one Ctx, RpcCtx) and a version bump of every crate ([#467](https://github.com/ewe-studios/ewe_platform/pull/467))
- merge feature 04, HTTP keep-alive and WebSocket idle timeouts
- *(release)* bump foundation_http and foundation_netio to 0.0.9
- *(spec 70)* move inline tests out of foundation_wasmtime (feature 59)
- *(spec 70)* move inline tests out of foundation_platform (feature 59)
- run cargo fmt --all under the pinned rustfmt (feature 39)
