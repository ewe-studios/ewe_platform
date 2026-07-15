# Artifact export — how the tar stream went from wedged to working

The `oci`/`docker`/`tar` exporters stream the built image back to the client
through the session (`FileSend/DiffCopy`, ~3.9 MB for the alpine test image).
That transfer traverses **two nested H2 connections** (buildkitd's Level 2
inside our Level 1 Control bidi) plus a loopback TCP hop, all driven by
valtron pumps — which made it the first BuildKit flow big and fast enough to
expose every latent throughput bug in the stack. This documents the chain, in
the order the fixes peeled.

## Baseline: wedged forever at 32 KiB

Neither our H2 server (`foundation_http::h2_connection`) nor our H2 client
(`foundation_netio::H2Channel`) ever sent `WINDOW_UPDATE`. HTTP/2 flow-control
windows are **cumulative over the connection's lifetime** (RFC 9113 §5.2), so
any connection that ever carries more than the 64 KiB initial window stalls
permanently. All prior traffic (stats, health checks, dockerfiles) was small
enough to fit; the tar stalled after one window and buildkitd's exporter
context eventually canceled: `failed to copy to tar: context canceled`.

**Fix:** consumed request DATA is credited back (connection + stream windows)
on both the server and client paths. Credits only cover bytes actually
delivered into handler pipes, so pipe depth remains the true backpressure;
frames the pipe can't take yet are buffered per-stream **uncredited**, which
caps that backlog at one window.

## Second wedge: response-side deadlock in H2 dispatch

`dispatch_h2_stream` joined the request-side futures (body feeder + request
reader) with the response side before closing the stream. buildkitd's fsutil
holds its request stream open until it sees our trailers; we waited for its
request EOF before sending them. Mutual wait, forever.

**Fix:** the request and response sides race (`futures::future::select`); the
response side alone decides the RPC outcome and closes the stream (with real
gRPC trailing HEADERS — the new `H2Frame::Trailers`), abandoning a still-open
request side, which RFC 9113 permits.

## Crawl #1: ~30 KB/s from per-frame credit chatter

Immediate per-frame crediting emitted a 13-byte WINDOW_UPDATE **pair** for
every DATA frame, and the sender's view of its window fragmented into
ever-smaller DATA frames (observed shrinking to ~1 KB) — a fragmentation
spiral through a poll-quantized tunnel.

**Fix:** credits batch and flush at 32 KiB (half the default stream window),
plus a one-time post-handshake connection-window raise (+4 MiB) so bulk
uploads aren't paced by credit round trips.

## Crawl #2: ~1–2 s per 32 KiB chunk from scheduler latency

Two valtron scheduler defects (details in
`backends/foundation_core/src/valtron/docs/scheduler_latency_and_depends_guard.md`):

1. Workers slept to the **furthest** sleeper deadline (`max_duration()`)
   instead of the nearest — a 10 ms-polling pump next to any longer sleeper
   woke when the longest timer expired.
2. Workers hosting only **readiness-parked** tasks (pipe waits, no deadline)
   blind-slept `DEFAULT_YIELD_WAIT_TIME` (4 s), and a cross-thread pipe send
   cannot interrupt a plain sleep — every cross-worker handoff could stall
   for seconds.

A third valtron bug — the `Depends` spin-guard counting lifetime violations
instead of consecutive ones — outright killed the transfer task at ~29 s
before the crawls could even finish (same doc, fix #1).

**Fix:** sleep to `min_duration()`; workers with readiness-parked tasks poll
at a 1 ms quantum; spin-guard counter resets on healthy behavior.

## Result

| Stage | Behavior |
|---|---|
| Baseline | wedged at 32 KiB, export canceled |
| + window credits | 384 KiB then `FlowControlError` (pipe overrun) |
| + uncredited overflow buffering | full transfer, ~30 KB/s, test timeout |
| + batched credits + window raise | still ~1 s/chunk (scheduler-bound) |
| + valtron scheduler fixes | **passes**: 3.86 MB OCI tar, credit cadence ~70 ms (from ~1–2 s), `build_with_oci_export` green in the 12/12 suite |

The remaining latency floor is the pumps' 10 ms socket polling — they park on
timers because raw sockets have no valtron readiness signal without reactor
(iogate/epoll) integration. Wiring the H2 pumps to the spec-41 reactor is the
next lever if export throughput ever matters beyond tests.
