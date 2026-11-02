# Feature 15 learnings

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol version 2 (HTTP/2); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## E1. The gRPC health probe leak

- The leak had two parts. The probe ran on a throwaway thread bounded by an
  `mpsc::Receiver::recv_timeout` backstop. The real leak was the fire-and-forget
  `H2Pump` task: when the probe's own timer resolved and the probe future was
  dropped, nothing told the pump to stop, so it parked on the silent socket
  until the peer's idle timeout and kept one open file descriptor per probe.
- The fix is to bound the pump itself. `H2Transport` now carries a `read_timeout`
  (`Duration::ZERO` = disabled, backward-compatible). The pump records
  `last_progress` and updates it on every successful read and write. A
  top-of-loop check (which skips `PumpPhase::Handshaking`, since that has its own
  deadline) fails the call with `TransportError::Timeout` when the peer makes no
  progress for the read timeout. Once the pump ends, its `H2Socket` drops and the
  file descriptor closes.
- One empty non-ACK SETTINGS frame is enough for the client handshake. The
  client's `client_handshake_step` sends its preface and SETTINGS, reads the
  server's non-ACK SETTINGS, queues its own ACK, and returns `Ok(())`; the pump
  then moves to `SendingRequest`. The server does not need to ACK anything for
  the pump to reach `WaitingResponse`.
- The test backend must set its accepted socket back to blocking. On this
  machine (macOS) an accepted socket inherits non-blocking from a non-blocking
  listener, so the backend's first read after the available bytes returned
  `EAGAIN` and the backend closed the connection early. The probe then failed in
  about 12 ms with "transport closed without response head", not at the timeout.
  `stream.set_nonblocking(false)` after `accept` fixes it.
- The probe latency is now about the probe timeout (the internal `select` timer
  and the pump's read timeout both fire there). 20 probes of a 300 ms timeout
  finish in about 6 s.

## E2. Backend-state persistence lost update

- `FileStateStore` uses plain `std::fs` inside its async methods and each future
  is ready on its first poll. That means the read-modify-write race is real and
  narrow: several control-socket threads can each read the same `proxy_state`
  file, change one field and write it back before another writes, so the last
  writer wins and earlier updates are lost.
- The store keeps its one-document shape (the "Done when" checklist requires old
  state files to stay loadable), so the chosen fix is a mutex around the
  read-modify-write, not one record per backend. A `futures_util::lock::Mutex`
  (an async mutex) is the right tool: the critical section spans `.await`
  points, so a `std::sync::Mutex` would make the future `!Send`.
- The test proves the fix: 16 threads each save a different backend 50 times. It
  fails on every run without the lock and passes with it.

## E3. ACME RFC 8555 account flow

- The newAccount answer's `Location` header is the account URL (RFC 8555 7.3).
  The client must store it and sign every later JWS with it as the `kid`
  (RFC 8555 6.2). Signing with the `jwk` after the account exists is wrong.
- Each answer carries a single-use `Replay-Nonce`. The client must store that
  nonce and send it on the next request. A fresh nonce is fetched only when no
  nonce is stored or the CA answers `badNonce`.
- `std::thread::sleep` inside an `async fn` blocks the thread that drives the
  future. `sleep_async` (the valtron timer) yields, so other futures on the
  same thread run. The regression test drives the DNS wait and a heartbeat
  together with `futures_lite::future::zip` on one thread: a blocking sleep
  serialises them (about 4 s), an async sleep overlaps them (about 2 s).
- The strict mock CA must not validate a JWS nonce twice. The first `/finalize`
  handler called `validate_jws` and then `serve_acme_post`, which validated the
  same nonce again and answered "unknown or reused nonce". The fix renders a
  validated 2xx answer with a helper that does not re-validate.
- The three new plain `#[test]`s each need `initialize_pool`, because the
  `DynNetClient` send path spawns tasks on the valtron pool. One test run alone
  failed with "pool not initialized"; the full suite passed because the two
  older tests initialised the pool in the same process. Each test must set up
  its own pool.

## E4. SSL redirect limits are config, not constants

- The redirect loop cut a head that spanned reads, because it did one `read`
  and answered whatever that read returned. The fix reads into a reusable
  buffer until the blank line (`\r\n\r\n`) or the head limit, then answers.
- A head that reaches the limit without its blank line gets a 431. The check
  order matters: look for the blank line first, then the limit, so a head that
  ends exactly at the limit still redirects.
- The accepted socket must be set blocking before the read. On BSD/macOS an
  accepted socket inherits the listener's non-blocking mode, so without
  `set_nonblocking(false)` the read would return `WouldBlock` at once and never
  wait for the rest of a split head.
- The accept loop parks on listener readiness via `foundation_iogate::FdReadiness`
  (re-exported from `foundation_reactor`) and links `ShutdownSignal::register_wake`,
  the same pattern the HTTP front end uses. On non-unix, or when the fd cannot
  be registered, it falls back to sleeping the configured accept poll.

## Dead ends

- None for E1, E2, E3 or E4.
