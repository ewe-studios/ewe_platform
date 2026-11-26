# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.17](https://github.com/ewe-studios/ewe_platform/compare/foundation_connectrpc-v0.0.16...foundation_connectrpc-v0.0.17) - 2026-11-26

### Other

- updated the following local packages: foundation_macros, foundation_core, foundation_reactor, foundation_netio, foundation_http, foundation_auth

## [0.0.16](https://github.com/ewe-studios/ewe_platform/compare/foundation_connectrpc-v0.0.13...foundation_connectrpc-v0.0.16) - 2026-11-24

### Added

- *(wasm)* stream the response builder, inbound body, sink and DO client (spec 85 T3 to T6)
- *(netio)* stream wasm response bodies and use the exchange client (spec 85 T1, T2)

### Fixed

- *(http)* answer 405 with Allow for a known path and a wrong method
- *(ci)* drop the BoringSSL entries from the nested lockfiles
- *(ci)* refresh every stale nested lockfile
- close the spec 85 follow-up issues

### Other

- Merge master into the registrar branch
- spec 82: finish the proxy registration authority
- spec 82: add the registration registry and the enrollment proof API
- spec 57: finish social login end to end
- spec 81: add RSA-PSS secrets and the client revocation check
- spec 81: implement signed secrets, key loading and client auth providers
- spec 88 feature 03: finish the transport cases
- spec 88 feature 03: cover the connectrpc runtime on the wasm hosts
- spec 88 feature 02: prove the foundation_http wasm runtimes
- make the timing tests tolerate a loaded or emulated host
- spec 85: verify the wasm suites and fix the defects they found
- Merge spec 87: mutual TLS
- spec 84: context injection ([#679](https://github.com/ewe-studios/ewe_platform/pull/679))
- spec 86: auth primitives ([#676](https://github.com/ewe-studios/ewe_platform/pull/676))
- spec 89: one compile-time MIME table (draft) ([#656](https://github.com/ewe-studios/ewe_platform/pull/656))
- spec 76 completion, spec 80 (transport and migration groups), and the bindgen/syn fix ([#631](https://github.com/ewe-studios/ewe_platform/pull/631))
- Spec 65 feature 01: foundation_agent_rpc, an agent RPC model server (Connect/gRPC/gRPC-Web, token → mTLS) ([#126](https://github.com/ewe-studios/ewe_platform/pull/126))

## [0.0.13](https://github.com/ewe-studios/ewe_platform/compare/foundation_connectrpc-v0.0.12...foundation_connectrpc-v0.0.13) - 2026-11-12

### Other

- Fix ten medium issues from the spec 76 work: Ctx in valtron waits and the netio sync client, Windows builds, CI coverage ([#503](https://github.com/ewe-studios/ewe_platform/pull/503))
- Fix master: clear the clippy lints that rust-version 1.98 turned on ([#502](https://github.com/ewe-studios/ewe_platform/pull/502))
- Spec 76 waves 7 to 9 (db, netio, core take Ctx); import Ctx from foundation_context; retry a rate-limited publish ([#480](https://github.com/ewe-studios/ewe_platform/pull/480))
- Spec 76 waves 2 to 6: eleven crates take Ctx instead of a clock ([#476](https://github.com/ewe-studios/ewe_platform/pull/476))
