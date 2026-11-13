# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.7](https://github.com/ewe-studios/ewe_platform/compare/foundation_keychain-v0.1.2...foundation_keychain-v0.1.7) - 2026-11-12

### Added

- *(spec70)* phase D platform times come from the injected clock
- *(spec70)* phase C service times come from the injected clock
- *(valtron)* name a task poll that blocks a worker
- *(foundation_core)* add a close-only pipe readiness
- *(core)* read every timestamp from the injected Clock
- *(valtron)* make the engine clock replaceable at startup
- *(http)* resolve ServeFuture to the closed ServeOutcome enum
- *(sse)* write SSE events through the async ResponseWriter
- *(cloudflare)* split generated groups into per-group crates (feature 22 E3)
- *(io)* wake-capable shutdown signal wakes the parked accept loop (feature 31)

### Fixed

- medium-severity defect batch (all open medium issues) ([#422](https://github.com/ewe-studios/ewe_platform/pull/422))
- fix netio: compute the load tracker connection rate ([#337](https://github.com/ewe-studios/ewe_platform/pull/337))
- update the example locks for foundation_netio 0.0.8
- *(netio)* compute the load tracker connection rate
- *(foundation_toolings)* wake the builder on watch events and follow the injected clock
- *(foundation_http)* take the app clock and RNG from an injected Ctx
- *(spec70)* count one worker exit and re-raise a task panic
- *(spec70)* bound pool shutdown, require the Ctx clock, and repair CI
- *(spec70)* phase B reads engine and I/O time from the injected clock
- *(valtron)* prune a bridged waker when its wait is gone
- *(keychain)* fill the key buffer through the uninitialised entropy path
- *(keychain)* avoid a literal Err in the error trace test
- *(keychain)* report errors through foundation_errstacks
- *(io-63)* retire the truncating sync response path
- *(keychain)* clear the clippy warnings
- *(foundation_keychain)* read the request body in the async future, capped (spec 70 feature 64)
- *(foundation_keychain)* serve handlers as ConnectionResult::Async (spec 70 feature 44)
- *(keychain)* clean clippy pedantic warnings in the moved SignalR framing
- *(keychain)* move SignalR framing to core and drop wasm warnings
- *(openapi)* type boolean and array query parameters; regenerate providers
- *(auth)* configurable error-body cap + drop dead WebAuthn config
- *(compact)* expose uuid v4 from foundation_compact entropy
- *(keychain)* enable the wasm RNG source for uuid
- *(release)* bump valtron version group to 0.0.7 and land features 38 and 40
- *(auth)* revert the incomplete sha2 0.11 bump across the workspace

### Other

- Fix ten medium issues from the spec 76 work: Ctx in valtron waits and the netio sync client, Windows builds, CI coverage ([#503](https://github.com/ewe-studios/ewe_platform/pull/503))
- Spec 76 waves 7 to 9 (db, netio, core take Ctx); import Ctx from foundation_context; retry a rate-limited publish ([#480](https://github.com/ewe-studios/ewe_platform/pull/480))
- Spec 76 waves 2 to 6: eleven crates take Ctx instead of a clock ([#476](https://github.com/ewe-studios/ewe_platform/pull/476))
- Fix master's Cloudflare codegen check; spec 76 wave 1 (nine crates take Ctx) ([#473](https://github.com/ewe-studios/ewe_platform/pull/473))
- Spec 76 feature 02 (one Ctx, RpcCtx) and a version bump of every crate ([#467](https://github.com/ewe-studios/ewe_platform/pull/467))
- *(release)* bump foundation_http and foundation_netio to 0.0.9
- *(foundation_keychain)* report streamed-body timeouts and assert progress ([#359](https://github.com/ewe-studios/ewe_platform/pull/359))
- *(foundation_core)* bump to 0.0.13 and update every consumer
- *(keychain)* move the util tests out of src
- run cargo fmt --all under the pinned rustfmt (feature 39)
- *(db)* bump foundation_db to 0.0.5 and every consumer's requirement
- *(compact)* bump to 0.2.3 for the new uuid module
- *(keychain)* remove the unused rmpv dependency
- Merge remote-tracking branch 'origin/master' into fix/io-01-valtron-parking
- Spec 50 F05 T3: drop _async suffixes from storage trait methods
- WIP #140: move HTTP callers to DynNetClient / HttpClientBuilder (not yet verified)
- Rename the async storage traits to plain names; drop SessionManager's _async suffixes
- Imports at the top of the file, not inside functions or as full paths in bodies (#131 review)
- await the async StorageProvider::new
