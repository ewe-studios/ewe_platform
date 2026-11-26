# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.9](https://github.com/ewe-studios/ewe_platform/compare/foundation_deployment_platform-v0.1.8...foundation_deployment_platform-v0.1.9) - 2026-11-26

### Other

- updated the following local packages: foundation_macros, foundation_core, foundation_netio, foundation_wireguard, foundation_sshkit, foundation_deployment_docker

## [0.1.8](https://github.com/ewe-studios/ewe_platform/compare/foundation_deployment_platform-v0.1.7...foundation_deployment_platform-v0.1.8) - 2026-11-24

### Added

- *(netio)* stream wasm response bodies and use the exchange client (spec 85 T1, T2)

### Fixed

- *(#769)* generate the Docker list response types

### Other

- *(docker)* bump foundation_deployment_docker to 0.1.8
- spec 84: context injection ([#679](https://github.com/ewe-studios/ewe_platform/pull/679))
- spec 89: one compile-time MIME table (draft) ([#656](https://github.com/ewe-studios/ewe_platform/pull/656))
- spec 76 completion, spec 80 (transport and migration groups), and the bindgen/syn fix ([#631](https://github.com/ewe-studios/ewe_platform/pull/631))

## [0.1.7](https://github.com/ewe-studios/ewe_platform/compare/foundation_deployment_platform-v0.1.6...foundation_deployment_platform-v0.1.7) - 2026-11-12

### Other

- Fix ten medium issues from the spec 76 work: Ctx in valtron waits and the netio sync client, Windows builds, CI coverage ([#503](https://github.com/ewe-studios/ewe_platform/pull/503))
- Fix master: clear the clippy lints that rust-version 1.98 turned on ([#502](https://github.com/ewe-studios/ewe_platform/pull/502))
- Spec 76 waves 7 to 9 (db, netio, core take Ctx); import Ctx from foundation_context; retry a rate-limited publish ([#480](https://github.com/ewe-studios/ewe_platform/pull/480))
- Spec 76 waves 2 to 6: eleven crates take Ctx instead of a clock ([#476](https://github.com/ewe-studios/ewe_platform/pull/476))
