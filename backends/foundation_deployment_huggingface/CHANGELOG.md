# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.7](https://github.com/ewe-studios/ewe_platform/compare/foundation_deployment_huggingface-v0.1.6...foundation_deployment_huggingface-v0.1.7) - 2026-11-26

### Other

- updated the following local packages: foundation_macros, foundation_core, foundation_netio

## [0.1.6](https://github.com/ewe-studios/ewe_platform/compare/foundation_deployment_huggingface-v0.1.5...foundation_deployment_huggingface-v0.1.6) - 2026-11-24

### Added

- *(netio)* stream wasm response bodies and use the exchange client (spec 85 T1, T2)

### Other

- spec 84: context injection ([#679](https://github.com/ewe-studios/ewe_platform/pull/679))
- spec 89: one compile-time MIME table (draft) ([#656](https://github.com/ewe-studios/ewe_platform/pull/656))
- spec 76 completion, spec 80 (transport and migration groups), and the bindgen/syn fix ([#631](https://github.com/ewe-studios/ewe_platform/pull/631))

## [0.1.5](https://github.com/ewe-studios/ewe_platform/compare/foundation_deployment_huggingface-v0.1.4...foundation_deployment_huggingface-v0.1.5) - 2026-11-12

### Other

- Fix ten medium issues from the spec 76 work: Ctx in valtron waits and the netio sync client, Windows builds, CI coverage ([#503](https://github.com/ewe-studios/ewe_platform/pull/503))
