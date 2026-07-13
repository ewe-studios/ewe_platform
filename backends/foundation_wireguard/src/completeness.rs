//! Completeness gate for spec-55 (foundation_wireguard).
//!
//! WHY: Prevent premature merges. When `feature = "spec55-complete"` is
//! enabled, this module emits `compile_error!` for every known gap in the
//! spec-55 implementation. To pass CI, every gap must be resolved.
//!
//! HOW: Each gap is a `compile_error!` behind `#[cfg(feature = "spec55-complete")]`.
//! Fix the gap → delete the corresponding `compile_error!` line → the gate
//! advances. The feature flag also gates the `completeness_tests.rs`
//! integration suite.
//!
//! HOW TO FIX: Search for the gap keyword (e.g. "F05-GAP-relay-out") in the
//! codebase, implement the fix, then remove the `compile_error!` line.

/// Compile-time completeness gate. If this module compiles, spec-55 is done.
///
/// Every `compile_error!` here represents a verified gap from the audit. When
/// the gap is closed, delete the corresponding line. When this module is empty
/// of `compile_error!` calls, spec-55 is complete.
pub fn check() {}

// ═══════════════════════════════════════════════════════════════════════════
// F05 — Relay & NAT Traversal gaps
// ═══════════════════════════════════════════════════════════════════════════

// F05 relay_out gap FIXED: runtime now calls on_direct_probe_success/failure
// for every peer based on driver reachability. See native/node.rs run_runtime().

// F05-GAP-holepunch FIXED: HolePuncher integrated into relay server thread in
// node.rs — drains SYNs, sends via relay UDP socket, demuxes incoming HPv1
// packets, handles timeouts via tick(). See native/node.rs relay thread.

// ═══════════════════════════════════════════════════════════════════════════
// F06 — WebTransport gaps
// ═══════════════════════════════════════════════════════════════════════════

/// F06-GAP-wtsession: WtSession has no QuicConnection reference.
/// Datagrams are VecDeque buffers with no QUIC integration.
#[cfg(feature = "spec55-complete")]
const _F06_GAP_WTSESSION_QUIC: () = {
    compile_error!(
        "F06 GAP: WtSession holds only VecDeque buffers — no QuicConnection.\n\
         Fix: add a QuicConnection type parameter to WtSession, implement open_bidi/accept_bidi,\n\
         wire send_datagram/recv_datagram through the QUIC trait methods.\n\
         See foundation_netio/src/webtransport/session.rs:86."
    );
};

/// F06-GAP-connect-raw: Extended CONNECT request/response is raw ASCII, not
/// QPACK-encoded H3 HEADERS frames.
#[cfg(feature = "spec55-complete")]
const _F06_GAP_CONNECT_RAW: () = {
    compile_error!(
        "F06 GAP: Extended CONNECT is raw HTTP/1.1-text, not QPACK H3 frames.\n\
         Fix: use H3Request::encode_headers with QPACK for the CONNECT request/response.\n\
         See foundation_netio/src/webtransport/session.rs:276 and :311."
    );
};

// F06-GAP-capsule-decoder FIXED: CapsuleDecoder with push/decode state machine
// handles partial QUIC stream reads, matching the FrameDecoder IncrementalDecoder
// pattern. See foundation_netio/src/webtransport/proto.rs.

/// F06-GAP-acceptor: WtAcceptor.queue_session is never called from H3Connection.
#[cfg(feature = "spec55-complete")]
const _F06_GAP_ACCEPTOR: () = {
    compile_error!(
        "F06 GAP: WtAcceptor never wired to H3Connection — no server accept path.\n\
         Fix: In H3Connection::poll_accept, detect :protocol=webtransport, perform \
         Extended CONNECT handshake, call WtAcceptor::queue_session().\n\
         See foundation_netio/src/http3/connection.rs:287 and session.rs:253."
    );
};

// ═══════════════════════════════════════════════════════════════════════════
// F07 — wasm/browser gaps
// ═══════════════════════════════════════════════════════════════════════════

// F07-GAP-wasm-deps FIXED: wasm-bindgen/web-sys/js-sys cfg-deps in Cargo.toml.
// F07-GAP-device FIXED: JsDevice implements smoltcp::phy::Device with
// RxToken/TxToken matching the nativeapis TunnDevice pattern.
// F07-GAP-join FIXED: BrowserWgNode::join() derives seed keys, creates tunnel
// and device. The actual wss connectrpc exchange is handled by JS glue code
// outside the Rust crate (this is by design — browsers run JS for WebSocket).

/// F07-GAP-ws-transport: WsRelayClient is sans-I/O queues by design (the JS
/// bridge feeds/drains them). JS glue code that connects these queues to an
/// actual web_sys::WebSocket is required for a complete browser build.
#[cfg(feature = "spec55-complete")]
const _F07_GAP_WS_TRANSPORT: () = {
    compile_error!(
        "F07 GAP: no JS bridge connecting WsRelayClient queues to web_sys::WebSocket.\n\
         Fix: add a wasm-bindgen bridge module or JS glue that:\n\
         1. Opens a wss connection to the seed endpoint\n\
         2. Calls WsRelayClient::on_frame() on inbound messages\n\
         3. Calls WsRelayClient::drain_outbound() and sends each frame via ws.send()\n\
         See foundation_wireguard/src/wasm/browser.rs:34."
    );
};

// ═══════════════════════════════════════════════════════════════════════════
// F09 — Config/Macro gaps
// ═══════════════════════════════════════════════════════════════════════════

// F09-GAP-macro-test FIXED: wireguard! macro tested in completeness_tests.rs
// via macro_produces_valid_config — verifies macro output = builder equivalent
// and boots a working node.

// ═══════════════════════════════════════════════════════════════════════════
// F10 — Deployment-Platform gaps
// ═══════════════════════════════════════════════════════════════════════════

/// F10-GAP-docker-test: No Docker integration test for mesh self-assembly.
#[cfg(feature = "spec55-complete")]
const _F10_GAP_DOCKER_TEST: () = {
    compile_error!(
        "F10 GAP: no Docker integration test for mesh self-assembly.\n\
         Fix: add a test that launches containers with injected WG_SECRET, \
         verifies mesh formation and overlay reachability.\n\
         See spec line 50-52 and foundation_deployment_platform/tests/."
    );
};
