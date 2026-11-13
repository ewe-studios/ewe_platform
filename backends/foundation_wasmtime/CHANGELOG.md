# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1](https://github.com/ewe-studios/ewe_platform/compare/foundation_wasmtime-v0.1.0...foundation_wasmtime-v0.3.1) - 2026-11-13

### Fixed

- medium-severity defect batch (all open medium issues) ([#422](https://github.com/ewe-studios/ewe_platform/pull/422))

### Other

- Take the security fixes a stale rust-version held back; check the iOS target in CI ([#501](https://github.com/ewe-studios/ewe_platform/pull/501))
- Spec 76 waves 7 to 9 (db, netio, core take Ctx); import Ctx from foundation_context; retry a rate-limited publish ([#480](https://github.com/ewe-studios/ewe_platform/pull/480))
- Spec 76 waves 2 to 6: eleven crates take Ctx instead of a clock ([#476](https://github.com/ewe-studios/ewe_platform/pull/476))
- Spec 76 feature 02 (one Ctx, RpcCtx) and a version bump of every crate ([#467](https://github.com/ewe-studios/ewe_platform/pull/467))
- *(spec 70)* move inline tests out of foundation_wasmtime (feature 59)
- run cargo fmt --all under the pinned rustfmt (feature 39)
