# LEARNINGS — ConnectRPC Proxy

_No implementation yet — this file is seeded at spec creation and updated after
each milestone/task. See `.agents/skills/specifications-management/skill.md`._

## Key finding (creation)

A reverse proxy does **not** need to decode protobuf/RPC payloads to route RPC
traffic effectively — but it **does** need to be HTTP/2-faithful: no H2→H1
downgrade, `te: trailers` preserved, and HTTP/2 trailing HEADERS forwarded.
"Just forward it like raw TCP" is only correct for a single opaque backend; the
moment you want routing, load balancing, health, or TLS termination, you need
the HTTP layer to be correct first, and only then consider RPC awareness.

## Decision (2026-10-18)

Ship **B + D**, with D resolving C. B is the transparent, H2-correct HTTP
forward (`http`/`https` endpoints); D embeds `foundation_connectrpc` as the RPC
engine (`connect`/`grpc`/`grpc-web` endpoints) for method routing, stream-level
LB, gRPC health, deadline pass-through, and gRPC-Web ↔ gRPC translation. The
operator selects the path **per endpoint** by declaring the backend protocol,
extending `BackendProtocol` in `foundation_proxy`'s `config.rs`. D is chosen
over C because it reuses connectrpc's existing handlers rather than
re-implementing the wire formats inside the proxy.

## Grounding (2026-10-18)

Wire-format claims are confirmed by the external [Connect protocol
spec](https://connectrpc.com/docs/protocol/) (fetched via `curl`; the page is
static/SSR) and the [gRPC over HTTP/2
spec](https://github.com/grpc/grpc/blob/master/doc/PROTOCOL-HTTP2.md). Key
confirmations: Connect uses *no* HTTP trailers (renders trailing metadata as
`trailer-`-prefixed headers), frames streaming as `Envelope-Flags Message-Length
Message`, and requires H2 only for client-streaming/bidi. gRPC mandates H2,
`te: trailers`, and delivers status in trailing HEADERS.

## Pre-existing gap (2026-10-18)

`foundation_connectrpc`'s own H2 transport does **not** yet forward HTTP/2
trailing HEADERS to its caller — `tests/grpc_tests.rs` documents this explicitly
("`grpc-status` trailers are not yet verifiable … not plumbed into the seam's
`ClientConn`"). So the proxy's missing-trailer gap (§3 item 3) has a sibling in
the RPC layer: even with the proxy fixed, a connectrpc gRPC client cannot
observe `grpc-status` until the H2 transport gains a trailer pipe. The
end-to-end "assert `grpc-status` at the client" test is the correct red test —
it fails until trailer plumbing lands in both places. This was found by reading
the existing tests, which is exactly why the validation strategy leans on them.
