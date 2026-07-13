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

/// F05-GAP-relay-out: `relay_out` queue in node.rs is never populated.
/// No code routes tunnel-driver WG packets into the relay input queue.
#[cfg(feature = "spec55-complete")]
const _F05_GAP_RELAY_OUT: () = {
    compile_error!(
        "F05 GAP: relay_out queue never populated — relay server runs on empty input.\n\
         Fix: populate relay_out from the runtime when a peer's connectivity path is Relayed. \
         See native/node.rs:74 and native/node.rs:260."
    );
};

/// F05-GAP-ladder: The connectivity ladder (RelayClient) is never driven.
/// on_direct_probe_failure/success are defined but never called from runtime.
#[cfg(feature = "spec55-complete")]
const _F05_GAP_LADDER: () = {
    compile_error!(
        "F05 GAP: connectivity ladder never driven — no probe failure/success calls.\n\
         Fix: call RelayClient::on_direct_probe_success/failure from the runtime loop \
         based on TunnelDriver reachability. See native/node.rs:517 (on_membership_update is called)."
    );
};

/// F05-GAP-holepunch: HolePuncher is never instantiated outside tests.
/// No demux on WG socket to route HPv1 packets.
#[cfg(feature = "spec55-complete")]
const _F05_GAP_HOLEPUNCH: () = {
    compile_error!(
        "F05 GAP: HolePuncher never instantiated in runtime — no demux.\n\
         Fix: add HolePuncher to Runtime struct, call drain_syns + send via WG socket, \
         demux incoming HPv1 packets. See native/relay.rs:333 and native/node.rs."
    );
};

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

/// F06-GAP-capsule-decoder: decode_capsule is one-shot, no incremental decoder.
/// Cannot handle partial reads on a QUIC stream.
#[cfg(feature = "spec55-complete")]
const _F06_GAP_CAPSULE_DECODER: () = {
    compile_error!(
        "F06 GAP: decode_capsule is one-shot — no IncrementalDecoder pattern.\n\
         Fix: implement a CapsuleDecoder with AccumulatingBuffer + Step, matching FrameDecoder\n\
         pattern in foundation_netio/src/http3/frame.rs:370.\n\
         See foundation_netio/src/webtransport/proto.rs:71."
    );
};

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

/// F07-GAP-wasm-deps: No wasm-bindgen/web-sys deps in Cargo.toml.
/// The crate cannot access browser APIs.
#[cfg(feature = "spec55-complete")]
const _F07_GAP_WASM_DEPS: () = {
    compile_error!(
        "F07 GAP: no wasm-bindgen/web-sys deps in Cargo.toml.\n\
         Fix: add [target.'cfg(target_family = \"wasm\")'.dependencies] with \
         wasm-bindgen, web-sys (WebSocket, RtcPeerConnection, etc.), and js-sys.\n\
         See foundation_wireguard/Cargo.toml."
    );
};

/// F07-GAP-device: JsDevice doesn't implement smoltcp::phy::Device.
/// smoltcp cannot drive I/O through it.
#[cfg(feature = "spec55-complete")]
const _F07_GAP_DEVICE: () = {
    compile_error!(
        "F07 GAP: JsDevice does not implement smoltcp::phy::Device.\n\
         Fix: implement Device for JsDevice with RxToken/TxToken, or use \
         foundation_nativeapis::NetStack's existing Device impl via a bridge.\n\
         See foundation_wireguard/src/wasm/device.rs:17."
    );
};

/// F07-GAP-join: BrowserWgNode has no join() method; no connectrpc bootstrap.
#[cfg(feature = "spec55-complete")]
const _F07_GAP_JOIN: () = {
    compile_error!(
        "F07 GAP: BrowserWgNode has no join() method for connectrpc bootstrap.\n\
         Fix: implement BrowserWgNode::join() using foundation_netio WebSocket to \
         connect to wss seed, exchange Join/PullMembership/Announce messages.\n\
         See foundation_wireguard/src/wasm/browser.rs:82."
    );
};

/// F07-GAP-ws-transport: WsRelayClient is pure VecDeque — no WebSocket I/O.
#[cfg(feature = "spec55-complete")]
const _F07_GAP_WS_TRANSPORT: () = {
    compile_error!(
        "F07 GAP: WsRelayClient is entirely in-memory queues — no actual WebSocket.\n\
         Fix: integrate with foundation_netio::websocket for browser WS connections.\n\
         Use web_sys::WebSocket for wasm builds.\n\
         See foundation_wireguard/src/wasm/browser.rs:34."
    );
};

// ═══════════════════════════════════════════════════════════════════════════
// F09 — Config/Macro gaps
// ═══════════════════════════════════════════════════════════════════════════

/// F09-GAP-macro-test: wireguard! macro has zero test coverage.
/// The spec requires tri-config convergence (macro = builder = TOML).
#[cfg(feature = "spec55-complete")]
const _F09_GAP_MACRO_TEST: () = {
    compile_error!(
        "F09 GAP: wireguard! macro never tested — no tri-config convergence test.\n\
         Fix: add a test in tests/config_tests.rs that calls wireguard!{} and \
         asserts the resulting WgConfig equals the builder and TOML equivalents. \
         See spec line 46 and wireguard.rs macro definition."
    );
};

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
