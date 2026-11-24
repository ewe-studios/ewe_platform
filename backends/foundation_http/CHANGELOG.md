# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.15](https://github.com/ewe-studios/ewe_platform/compare/foundation_http-v0.0.12...foundation_http-v0.0.15) - 2026-11-24

### Added

- *(wasm)* stream the response builder, inbound body, sink and DO client (spec 85 T3 to T6)
- *(netio)* stream wasm response bodies and use the exchange client (spec 85 T1, T2)

### Fixed

- *(http)* answer 405 with Allow for a known path and a wrong method
- *(http)* answer 500 when a wasm handler panics
- *(#770)* gate the native HTTP server to Unix

### Other

- spec 88 feature 02: prove the foundation_http wasm runtimes
- spec 85: verify the wasm suites and fix the defects they found
- spec 84: context injection ([#679](https://github.com/ewe-studios/ewe_platform/pull/679))
- spec 88: wasm runtime coverage (draft) ([#655](https://github.com/ewe-studios/ewe_platform/pull/655))
- spec 89: one compile-time MIME table (draft) ([#656](https://github.com/ewe-studios/ewe_platform/pull/656))
- spec 76 completion, spec 80 (transport and migration groups), and the bindgen/syn fix ([#631](https://github.com/ewe-studios/ewe_platform/pull/631))
- split spec 65 into focused specifications; Cloudflare control moves to spec 77 ([#553](https://github.com/ewe-studios/ewe_platform/pull/553))

### Added

- `Router::allowed_methods`, `RouteSegment::allowed_methods` and `RouteMethod::allowed_methods` name the methods of a matched path, in `Allow` header order.
- `native::writer::send_method_not_allowed` answers 405 with an `Allow` header.

### Fixed

- A panicking wasm handler answers 500 and the module keeps serving. The host entries run each dispatch poll inside a guarded JavaScript call (issue #742). The guarded future is public as `wasm::panic_guard::catch_panic`.
- A known path with a wrong method answers 405 with `Allow` on the native HTTP/1 server and on both wasm entries, not 404 (issue #743).

## [0.0.12](https://github.com/ewe-studios/ewe_platform/compare/foundation_http-v0.0.11...foundation_http-v0.0.12) - 2026-11-12

### Other

- Fix ten medium issues from the spec 76 work: Ctx in valtron waits and the netio sync client, Windows builds, CI coverage ([#503](https://github.com/ewe-studios/ewe_platform/pull/503))
- Spec 76 waves 7 to 9 (db, netio, core take Ctx); import Ctx from foundation_context; retry a rate-limited publish ([#480](https://github.com/ewe-studios/ewe_platform/pull/480))
- Spec 76 waves 2 to 6: eleven crates take Ctx instead of a clock ([#476](https://github.com/ewe-studios/ewe_platform/pull/476))
