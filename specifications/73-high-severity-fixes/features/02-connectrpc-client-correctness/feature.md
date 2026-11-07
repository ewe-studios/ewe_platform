---
feature: "ConnectRPC clients decompress responses, enforce deadlines and forward bidi requests"
status: "completed"
priority: "high"
depends_on: ["01-connectrpc-grpc-status-conversion"]
crates: ["foundation_connectrpc"]
issues: [259, 260, 261]
model: "sonnet"
branch: "spec73/f02-connectrpc-client-correctness"
---

# Feature 02: ConnectRPC client correctness

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Google Remote Procedure Call (gRPC); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Close three client defects:
[#259](https://github.com/ewe-studios/ewe_platform/issues/259) (no decompression),
[#260](https://github.com/ewe-studios/ewe_platform/issues/260) (deadline not enforced) and
[#261](https://github.com/ewe-studios/ewe_platform/issues/261) (`bidi_stream` drops its
requests).

## 2. Problem (measured on master, 2026-11-07)

1. **#261 — real.** `backends/foundation_connectrpc/src/shared/client.rs:759` is
   `bidi_stream_inner`; line 762 takes `_reqs: impl Stream<Item = Req>` and never reads it.
   The caller's request stream is dropped. A bidi call sends no requests.
2. **#260 — already fixed. Close it with evidence.** `with_deadline` (`client.rs:1749`) and
   `with_deadline_at` (`client.rs:1704`) race every client call against
   `valtron::sleep_async(remaining)` and cancel on expiry. `BidiStream::send`,
   `close_request`, `receive`, `response_headers` and `response_trailers` all use
   `with_deadline_at`. Commits `0ada43f20` and `b79503f24` landed this. The feature only
   adds a test that pins the behaviour, or closes the issue with the commit evidence.
3. **#259 — real, and wider than the issue says.** The streaming readers pass `None`:
   - gRPC: `shared/protocol/grpc.rs:606` — `read_frames(body, &tx, None, read_max)`;
   - Connect: `shared/protocol/connect.rs:295` — `read_response_frames(..., None, ...)`;
   - unary gRPC: `shared/protocol/grpc.rs:460` `decode_unary_response(body)` has no
     compression parameter, and its own comment says the caller "must decompress separately
     (or thread the `CompressionRegistry` through)".
   A response frame with the compressed flag set therefore fails with "compressed frame
   received but no decompressor was negotiated" (`envelope/mod.rs:217`). The server path
   already negotiates (`negotiate_compression`, `compression/mod.rs:364`), so only the
   client is wrong.

## 3. Requirements

1. **#261:** no client API accepts a request stream and ignores it. A `BidiStream` has one
   writer, the facade's `send`, and every caller sends through it. Removing the dead
   parameter is the honest fix; the compiler then points at every migrated call site.
2. **#260:** no change to the mechanism. The feature adds a test that pins the existing
   deadline behaviour, so a later change cannot remove it silently.
3. **#259:** every client response path selects a decompressor from the response head before
   the first frame: gRPC `grpc-encoding`, gRPC-Web `grpc-encoding`, Connect
   `content-encoding` (unary) and `Connect-Content-Encoding` (streaming). Identity needs no
   decompressor. An unsupported name is a typed `Code::Unimplemented` error that names it.
   This covers unary, server stream, client stream and bidi.
4. No body is buffered whole (root rule 4). The decompressor works per frame, as the envelope
   reader already does.
5. Public API changes are expected (the `bidi_stream` parameter and `decode_unary_response`).
   Bump `foundation_connectrpc` and every consumer requirement in the same change.

## 4. Design (completed 2026-11-07)

### 4.1 #261, remove the dead request-stream parameter

The connection has one request writer: `ClientSender`. `BidiStream` owns it and exposes
`send`, `send_headers`, `close_request` and `split`. A second task cannot forward `reqs`
through the same sender, because the sender is not shareable and the two writers would race.
So forwarding `reqs` would make the facade's `send` unusable, and removing the facade's
writer would remove the only way to do interleaved duplex work (the BuildKit session in
`foundation_deployment_docker/src/buildkit/session.rs:816` needs it).

Change:
1. `Client::bidi_stream(&self, ctx: Ctx) -> ConnectResult<BidiStream<Req, Res>>` — drop the
   `reqs` parameter and the `_reqs` binding in `bidi_stream_inner`.
2. Update every caller to the facade: `backends/foundation_connectrpc_codegen/src/generator.rs`
   (two `bidi_stream(ctx, reqs)` strings), `backends/foundation_macros/src/connectrpc_service.rs`
   (two generated method bodies), `foundation_deployment_docker` (passes
   `futures::stream::pending()`, so the call loses an argument and keeps `split`), the two
   examples, and the five test files.
3. Regenerate any checked-in generated client that carries the old signature.

### 4.2 #260, pin the behaviour

No code change. Add `client_deadline_cancels_a_call_without_a_server` to
`tests/client_deadline_tests.rs`, or confirm an existing test already covers it. Record the
commit evidence (`0ada43f20`, `b79503f24`) in the PR and close the issue.

### 4.3 #259, thread the compression registry into every client response path

1. Add a lifetime to `ClientExchangeConfig` and a field:
   ```rust
   pub struct ClientExchangeConfig<'a> {
       pub cancel: CancelSignal,
       pub read_max_bytes: usize,
       /// The compressors the client accepts, for the response head.
       pub compression: &'a CompressionRegistry,
   }
   ```
   `Client::build_streaming_exchange` passes `&self.config.compression`.
2. Add one helper, `fn response_decompressor(head: &SimpleHeaders, registry: &CompressionRegistry) -> ConnectResult<Option<Arc<dyn Compressor>>>`,
   in `shared/compression/mod.rs`. It reads the protocol's header name (the caller gives the
   per-protocol set), returns `None` for a missing or `identity` value, returns the
   registered compressor, and returns `Code::Unimplemented` with the name for an unknown
   value.
3. Each client `new_conn` reads the head first, picks the decompressor, then passes it:
   - `grpc.rs` `read_grpc_response`: `read_frames(body, tx, decompressor, read_max)`;
   - `connect.rs` `read_connect_response`: `read_response_frames(..., decompressor, ...)`;
   - `grpc_web.rs` client `new_conn`: same, with its own header set.
4. Unary: `ProtocolClient::decode_unary_response(&self, head: &SimpleHeaders, body: Bytes, compression: &CompressionRegistry) -> ConnectResult<Bytes>`.
   The gRPC impl decompresses a frame with the compressed flag; Connect and gRPC-Web do the
   same for their body shapes. `client.rs:1016` passes `&reply.headers` and
   `&self.config.compression`.
5. Remove the "caller must decompress separately" comment in `grpc.rs:462`.

## 5. Tasks

- [ ] Verify all three defects on master. Record the evidence. (#260 is already fixed.)
- [ ] Remove the `reqs` parameter and migrate every caller and generator.
- [ ] Add the deadline pin test.
- [ ] Thread the registry into the three protocol readers and the unary decoder.
- [ ] Add one regression test per real defect, each failing on the old code.
- [x] Bump `foundation_connectrpc` and every consumer requirement.
- [x] Run the verify commands. Record the results.
- [ ] Close #259 and #261 in the PR body; close #260 with the commit evidence.

## 9. Result (2026-11-07)

Branch `spec73/f02-connectrpc-client-correctness`, commit `05bef17fd`, merged into
`spec73/high-severity-fixes` as `832897fba`. Thirty files changed.

1. **#261, one writer.** `Client::bidi_stream` takes only `ctx`. The generated wrappers in
   `foundation_connectrpc_codegen` (three arms) and `foundation_macros` (three bodies) no
   longer take `reqs`. The callers use `send` and `close_request` on the facade.
2. **#259, response decompression.** `ClientExchangeConfig` carries the compression registry.
   `response_decompressor` picks the decompressor from the response head for gRPC, gRPC-Web
   and Connect. The unary decoder decompresses a frame with the compressed flag set.
   An unknown name is `Code::Unimplemented` and the message names it.
3. **#260** needs no code change. The new test
   `client_deadline_cancels_a_call_without_a_server` passes on the old code. Close the issue
   with the commit evidence (`0ada43f20`, `b79503f24`).
4. **Old-code evidence** (temporary worktree at `48421e32c`):
   - gRPC streaming: failed with "compressed frame received but no decompressor was
     negotiated";
   - gRPC unary: failed in `decode_unary_response`, which had no registry;
   - Connect streaming: the same decompressor error;
   - unknown encoding: does not compile on the old code (`ClientExchangeConfig` had no
     `compression` field); the old reader ignored `grpc-encoding`;
   - bidi: the old signature dropped the three messages; the receive hit the 15 s deadline.
5. **Version.** `foundation_connectrpc` 0.0.7 to 0.0.8. The requirements in
   `foundation_proxy` and `foundation_deployment_docker` move to 0.0.8. The workspace
   `Cargo.lock` is regenerated. The other 11 lockfiles do not name the crate.
   `cargo metadata --locked` passes in the workspace and in three example workspaces.
6. **Deviations from the design:**
   - `response_decompressor` takes the per-protocol header names as a parameter. The design
     text named them, but its signature omitted them.
   - `decode_unary_response` also takes `read_max_bytes`, so a compressed body cannot inflate
     without a bound.
   - The `'a` borrow cannot enter the `'static` reader tasks. Each protocol clones the
     registry into its task, and `foundation_proxy` adds the field in three places.
   - The generator and the macro also lose the `reqs` parameter from the wrapper signatures.
     Requirement 1 forbids a client API that takes a request stream and ignores it.
7. **New defects.** #378 (high): request compression corrupts server-streaming requests,
   because the streaming writer never sets `FLAG_COMPRESSED`; fixed in the same spec. #379
   (low): `foundation_connectrpc_codegen` fails clippy with `-D warnings` (78 errors).

## 6. Tests

All tests go in `backends/foundation_connectrpc/tests/` as top-level integration test files.
The crate has no `tests/mod.rs`; Cargo discovers each file.

1. `a_compressed_grpc_response_is_decompressed` — a gzip frame with the compressed flag on
   the gRPC streaming path; assert the decoded message. Old code: the "no decompressor"
   error.
2. `a_compressed_unary_grpc_response_is_decompressed` — the same for the unary path.
3. `a_compressed_connect_stream_response_is_decompressed` — the Connect streaming path.
4. `an_unknown_response_encoding_is_unimplemented` — an unregistered name; assert the typed
   error names it.
5. `bidi_send_reaches_the_server` — three messages sent through the facade, then
   `close_request`; assert the server handler received all three and the half-close. This
   proves the one-writer path that replaces the dead parameter.
6. `client_deadline_cancels_a_call_without_a_server` — a `SimulatedClock`, a server double
   that never answers, and a bounded wait; assert `DeadlineExceeded`.

## 7. Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo fmt -p foundation_connectrpc
cargo clippy --profile uat -p foundation_connectrpc --no-deps --all-targets -- -D warnings
cargo test --profile uat -p foundation_connectrpc
```

## 8. Done when

- No client API takes a request stream and ignores it; every caller uses the facade.
- A compressed response decodes on every call kind and every protocol.
- An unknown response encoding is a typed error that names it.
- The deadline behaviour has a test that passes on master (it is already correct).
- `foundation_connectrpc` and every consumer are bumped for the API change.
- #259 and #261 are closed by the PR; #260 is closed with the commit evidence.
