# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.9](https://github.com/ewe-studios/ewe_platform/compare/foundation_proxy-v0.1.2...foundation_proxy-v0.1.9) - 2026-11-13

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
- *(http,netio)* park pushed body and accept loop on readiness
- *(netio)* wire a Waker-capable readiness source into async body reads

### Fixed

- medium-severity defect batch (all open medium issues) ([#422](https://github.com/ewe-studios/ewe_platform/pull/422))
- *(connectrpc)* send handler response headers on every streaming transport
- *(connectrpc)* compress and flag streaming client requests
- *(connectrpc)* decompress client responses and make bidi one writer
- *(foundation_nativeapis)* report setregs failures and reject negative truncate
- update the example locks for foundation_netio 0.0.8
- *(netio)* compute the load tracker connection rate
- *(foundation_toolings)* wake the builder on watch events and follow the injected clock
- *(deployment_platform)* take the Docker container clock from the caller
- *(foundation_http)* take the app clock and RNG from an injected Ctx
- *(spec70)* count one worker exit and re-raise a task panic
- *(spec70)* warm up the health probe before the leak baseline
- *(spec70)* thread the clock into the VFS IPC transport and repair the wireguard test
- *(spec70)* bound pool shutdown, require the Ctx clock, and repair CI
- *(spec70)* phase B reads engine and I/O time from the injected clock
- *(valtron)* prune a bridged waker when its wait is gone
- *(connectrpc)* wake the receiver when the response head is published
- *(connectrpc)* park the H2 client pump on reactor readiness
- *(netio)* give every async response reader a non-blocking socket
- *(proxy)* stop a blocking body read and a skipped timer stalling a worker
- *(keychain)* fill the key buffer through the uninitialised entropy path
- *(proxy)* report errors through foundation_errstacks
- *(io-63)* retire the truncating sync response path
- *(proxy)* clear the clippy and test warnings
- *(foundation_proxy)* no sleep-poll on a worker; H2 and H3 forwards are async (spec 70 feature 65 E3)
- *(foundation_proxy)* serve control-socket connections as futures (spec 70 feature 65 E4)
- *(foundation_proxy)* upgrade, TCP tunnel, gRPC-Web/Connect relays and passthrough listeners are futures, not threads (spec 70 feature 65 E1, E2)
- *(foundation_proxy)* stream the client request body into the upstream request, capped (spec 70 feature 65 E5)
- *(foundation_proxy)* plain state load takes the shared state-file lock (spec 70 feature 65 E6)
- *(foundation_proxy)* async HTTP relay, cross-process state lock, drop unused cloudflare dependency (spec 70 feature 45)
- *(vfs)* rebuild TursoDelta on the local turso engine (feature 19 E4)
- *(vfs)* stream copy and copy_async in configured chunks (feature 19 E9)
- *(proxy)* clear the last feature-gated warning
- *(proxy)* move the SSL redirect limits into config
- *(proxy)* follow RFC 8555 in the ACME account flow
- *(proxy)* serialise the persisted state read-modify-write
- *(proxy)* bound the gRPC health probe with an H2 read timeout
- *(openapi)* type boolean and array query parameters; regenerate providers
- *(release)* bump valtron version group to 0.0.7 and land features 38 and 40
- *(auth)* revert the incomplete sha2 0.11 bump across the workspace

### Other

- Fix ten medium issues from the spec 76 work: Ctx in valtron waits and the netio sync client, Windows builds, CI coverage ([#503](https://github.com/ewe-studios/ewe_platform/pull/503))
- Fix master: clear the clippy lints that rust-version 1.98 turned on ([#502](https://github.com/ewe-studios/ewe_platform/pull/502))
- Spec 76 waves 7 to 9 (db, netio, core take Ctx); import Ctx from foundation_context; retry a rate-limited publish ([#480](https://github.com/ewe-studios/ewe_platform/pull/480))
- Spec 76 waves 2 to 6: eleven crates take Ctx instead of a clock ([#476](https://github.com/ewe-studios/ewe_platform/pull/476))
- Fix master's Cloudflare codegen check; spec 76 wave 1 (nine crates take Ctx) ([#473](https://github.com/ewe-studios/ewe_platform/pull/473))
- Spec 76 feature 02 (one Ctx, RpcCtx) and a version bump of every crate ([#467](https://github.com/ewe-studios/ewe_platform/pull/467))
- merge the #378 request compression fix
- *(foundation_core)* bump to 0.0.13 and update every consumer
- *(proxy)* fail a hung e2e call on its own bound
- *(nativeapis)* bump to 0.0.7 for the new SyscallAction variant
- *(proxy)* move the control-socket tests out of src
- WIP feature 75: remove debug print from h3 request body test
- WIP feature 75: deadline/needs_polling plumbing, ResumeSignal in QUIC/H3, shutdown wake, kill check after sleep
- WIP feature 75: EventReadiness deadline/needs_polling, executor sleeper deadlines (incomplete, stopped at low disk)
- Merge remote-tracking branch 'origin/wip/f72-h3-request-body' into fix/io-41-50-lane-a
- io_uring tests fail when EWE_REQUIRE_IO_URING=1 and io_uring is missing; feature 72 partial (spec 70 f72 E2)
- stream the H3 forward response body; bound LimitedBatch/EOF reader pieces to batch_size (spec 70 f70 E3)
- non-blocking io_uring upstream dial with connect timeout (spec 70 f70 E2)
- delete polling splice_bidirectional; completion splice test uses aio::splice (spec 70 f70 E1)
- run cargo fmt --all under the pinned rustfmt (feature 39)
- *(proxy)* make the forward_http outgoing request a real test
- *(platform)* share a daemon-pinging docker_available helper
- *(nativeapis)* bump foundation_nativeapis to 0.0.4 and every consumer's requirement
- *(db)* bump foundation_db to 0.0.5 and every consumer's requirement
- Merge remote-tracking branch 'origin/master' into fix/io-01-valtron-parking
- say why the proxy keeps NativeHttpClient; ewe_deployables on the builder
- ACME client and health probe on DynNetClient (#140 group B)
- docker integration tests run on valtron, not tokio
- async StateStore; deployment crates and proxy await it (spec 50 F04 T8)
- a timed-out test fails in bounded time instead of hanging the suite
- add ProxyServer::shutdown_async; async tests stop blocking workers
