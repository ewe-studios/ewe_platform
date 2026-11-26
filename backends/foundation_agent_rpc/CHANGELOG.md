# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.5](https://github.com/ewe-studios/ewe_platform/compare/foundation_agent_rpc-v0.0.4...foundation_agent_rpc-v0.0.5) - 2026-11-26

### Other

- updated the following local packages: foundation_core, foundation_netio, foundation_db, foundation_http, foundation_auth, foundation_ai, foundation_connectrpc

## [0.0.3](https://github.com/ewe-studios/ewe_platform/releases/tag/foundation_agent_rpc-v0.0.3) - 2026-11-24

### Added

- *(agent-rpc)* gate the server to Unix and re-enable the Windows run
- *(netio)* stream wasm response bodies and use the exchange client (spec 85 T1, T2)

### Fixed

- *(http)* answer 405 with Allow for a known path and a wrong method

### Other

- spec 82: add the enrollment TLS client config
- spec 82: add the registration registry and the enrollment proof API
- spec 57: finish social login end to end
- spec 81: add RSA-PSS secrets and the client revocation check
- spec 81: implement signed secrets, key loading and client auth providers
- spec 88 feature 03: cover the connectrpc runtime on the wasm hosts
- spec 88 feature 02: prove the foundation_http wasm runtimes
- Merge spec 87: mutual TLS
- spec 84: context injection ([#679](https://github.com/ewe-studios/ewe_platform/pull/679))
- spec 89: one compile-time MIME table (draft) ([#656](https://github.com/ewe-studios/ewe_platform/pull/656))
- spec 76 completion, spec 80 (transport and migration groups), and the bindgen/syn fix ([#631](https://github.com/ewe-studios/ewe_platform/pull/631))
- split spec 65 into focused specifications; Cloudflare control moves to spec 77 ([#553](https://github.com/ewe-studios/ewe_platform/pull/553))
- Spec 65 feature 01: foundation_agent_rpc, an agent RPC model server (Connect/gRPC/gRPC-Web, token → mTLS) ([#126](https://github.com/ewe-studios/ewe_platform/pull/126))
