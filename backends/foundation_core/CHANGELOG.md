# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.24](https://github.com/ewe-studios/ewe_platform/compare/foundation_core-v0.0.23...foundation_core-v0.0.24) - 2026-11-26

### Other

- *(core)* hold foundation_sandbox to the no-global-clock rule
- *(sandbox)* rename to foundation_sandbox; move the binaries and facades in

## [0.0.23](https://github.com/ewe-studios/ewe_platform/compare/foundation_core-v0.0.22...foundation_core-v0.0.23) - 2026-11-24

### Fixed

- close the spec 85 follow-up issues

### Other

- spec 88 feature 03: finish the transport cases
- spec 88 feature 01: prove the valtron runtime on every wasm host
- widen the concurrent-tasks bound for a loaded host
- start the deadline probe timer before the park
- fix the clippy patterns and the poll bounds
- make the timing tests tolerate a loaded or emulated host

## [0.0.21](https://github.com/ewe-studios/ewe_platform/compare/foundation_core-v0.0.20...foundation_core-v0.0.21) - 2026-11-12

### Other

- Fix ten medium issues from the spec 76 work: Ctx in valtron waits and the netio sync client, Windows builds, CI coverage ([#503](https://github.com/ewe-studios/ewe_platform/pull/503))
- Migrate workflows to Blacksmith runners ([#500](https://github.com/ewe-studios/ewe_platform/pull/500))
- Fix master: clear the clippy lints that rust-version 1.98 turned on ([#502](https://github.com/ewe-studios/ewe_platform/pull/502))
- Spec 76 waves 7 to 9 (db, netio, core take Ctx); import Ctx from foundation_context; retry a rate-limited publish ([#480](https://github.com/ewe-studios/ewe_platform/pull/480))
