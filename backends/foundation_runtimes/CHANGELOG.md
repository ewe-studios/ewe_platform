# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.6](https://github.com/ewe-studios/ewe_platform/compare/foundation_runtimes-v0.0.5...foundation_runtimes-v0.0.6) - 2026-11-12

### Added

- *(spec70)* phase D platform times come from the injected clock
- *(valtron)* make the engine clock replaceable at startup

### Fixed

- *(deployment_platform)* take the Docker container clock from the caller
- *(spec70)* bound pool shutdown, require the Ctx clock, and repair CI
- *(release)* bump valtron version group to 0.0.7 and land features 38 and 40

### Other

- Spec 76 feature 02 (one Ctx, RpcCtx) and a version bump of every crate ([#467](https://github.com/ewe-studios/ewe_platform/pull/467))
