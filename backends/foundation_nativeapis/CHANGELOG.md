# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.17](https://github.com/ewe-studios/ewe_platform/compare/foundation_nativeapis-v0.0.16...foundation_nativeapis-v0.0.17) - 2026-11-26

### Other

- updated the following local packages: foundation_macros, foundation_core, foundation_reactor, foundation_db

## [0.0.16](https://github.com/ewe-studios/ewe_platform/compare/foundation_nativeapis-v0.0.15...foundation_nativeapis-v0.0.16) - 2026-11-24

### Other

- spec 84: context injection ([#679](https://github.com/ewe-studios/ewe_platform/pull/679))
- spec 89: one compile-time MIME table (draft) ([#656](https://github.com/ewe-studios/ewe_platform/pull/656))
- spec 76 completion, spec 80 (transport and migration groups), and the bindgen/syn fix ([#631](https://github.com/ewe-studios/ewe_platform/pull/631))

## [0.0.15](https://github.com/ewe-studios/ewe_platform/compare/foundation_nativeapis-v0.0.14...foundation_nativeapis-v0.0.15) - 2026-11-12

### Other

- Test every crate on Linux, macOS and Windows; queue CI jobs within the free tier ([#516](https://github.com/ewe-studios/ewe_platform/pull/516))
- Fix ten medium issues from the spec 76 work: Ctx in valtron waits and the netio sync client, Windows builds, CI coverage ([#503](https://github.com/ewe-studios/ewe_platform/pull/503))
- Migrate workflows to Blacksmith runners ([#500](https://github.com/ewe-studios/ewe_platform/pull/500))
- Spec 76 waves 7 to 9 (db, netio, core take Ctx); import Ctx from foundation_context; retry a rate-limited publish ([#480](https://github.com/ewe-studios/ewe_platform/pull/480))
- Spec 76 waves 2 to 6: eleven crates take Ctx instead of a clock ([#476](https://github.com/ewe-studios/ewe_platform/pull/476))
