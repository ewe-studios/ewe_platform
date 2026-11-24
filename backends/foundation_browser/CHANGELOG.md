# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.9](https://github.com/ewe-studios/ewe_platform/compare/foundation_browser-v0.0.7...foundation_browser-v0.0.9) - 2026-11-24

### Added

- *(netio)* stream wasm response bodies and use the exchange client (spec 85 T1, T2)

### Fixed

- *(http)* answer 405 with Allow for a known path and a wrong method
- *(#769)* generate the Docker list response types

### Other

- spec 88 feature 02: prove the foundation_http wasm runtimes
- spec 84: context injection ([#679](https://github.com/ewe-studios/ewe_platform/pull/679))
- spec 88: wasm runtime coverage (draft) ([#655](https://github.com/ewe-studios/ewe_platform/pull/655))
- spec 89: one compile-time MIME table (draft) ([#656](https://github.com/ewe-studios/ewe_platform/pull/656))
- spec 76 completion, spec 80 (transport and migration groups), and the bindgen/syn fix ([#631](https://github.com/ewe-studios/ewe_platform/pull/631))
- split spec 65 into focused specifications; Cloudflare control moves to spec 77 ([#553](https://github.com/ewe-studios/ewe_platform/pull/553))

## [0.0.7](https://github.com/ewe-studios/ewe_platform/compare/foundation_browser-v0.0.6...foundation_browser-v0.0.7) - 2026-11-12

### Other

- Fix ten medium issues from the spec 76 work: Ctx in valtron waits and the netio sync client, Windows builds, CI coverage ([#503](https://github.com/ewe-studios/ewe_platform/pull/503))
- Fix master: clear the clippy lints that rust-version 1.98 turned on ([#502](https://github.com/ewe-studios/ewe_platform/pull/502))
- Spec 76 waves 7 to 9 (db, netio, core take Ctx); import Ctx from foundation_context; retry a rate-limited publish ([#480](https://github.com/ewe-studios/ewe_platform/pull/480))
- Spec 76 waves 2 to 6: eleven crates take Ctx instead of a clock ([#476](https://github.com/ewe-studios/ewe_platform/pull/476))
