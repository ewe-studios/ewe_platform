//! Browser `WebSocket` bridge for wasm (F51 Stage 6).
//!
//! WHY: F51 unifies the connection-owning client across native and wasm. On
//! native a WebSocket is an HTTP/1.1 Upgrade driven by a valtron task; on wasm
//! the browser owns the socket entirely through the `WebSocket` Web API. This
//! module bridges that event-driven browser socket into the **same** shared
//! [`WebSocketClient`] surface — a boxed inbound stream plus a
//! [`MessageDelivery`] — so a caller writes one `open_websocket` call that works
//! on both targets.
//!
//! WHAT: [`connect`] opens a `web_sys::WebSocket`, wires its `open`/`message`/
//! `error`/`close` events into a bounded pipe, and returns a [`WebSocketClient`].
//! [`WasmWebSocketBridge`] is the `Iterator<Item = WebSocketStreamItem>` the
//! client drains: each `next()` first flushes any queued outbound messages to
//! `WebSocket.send`, then yields the next inbound message (or `Pending` while the
//! JS event loop still owes us events).
//!
//! HOW: Inbound events are decoded to [`WebSocketMessage`] inside JS callbacks
//! (`Closure`s, kept alive by the bridge) and pushed onto a `PipeSender`. Binary
//! frames arrive as `ArrayBuffer` (we set `binaryType = "arraybuffer"`) and are
//! copied via `js_sys::Uint8Array`. The bridge is `!Send` by construction (it
//! holds JS handles), so — exactly like `foundation_compact::SendWrapper` — we
//! assert `Send` only on single-threaded wasm, where no other thread exists.
//!
//! Platform notes (documented no-ops, not a compile-time fork):
//! - **Custom headers** ([`WebSocketConnectConfig::extra_headers`]) cannot be set
//!   on a browser `WebSocket` — the API exposes no header hook. They are ignored.
//! - **Ping/Pong** frames are managed by the browser; user-level `Ping`/`Pong`
//!   sends are no-ops.
//! - **Reconnect** ([`Reconnect::Yes`]) is native-only; the browser owns the
//!   socket lifecycle, so it is not layered here.

use std::cell::Cell;
use std::rc::Rc;

use foundation_core::valtron::{Pipe, PipeReceiver, Stream, TryRecvError};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{BinaryType, CloseEvent, Event, MessageEvent, WebSocket};

use crate::websocket::shared::client::{
    MessageDelivery, WebSocketClient, WebSocketConnectConfig, WebSocketStreamItem, WsProgress,
};
use crate::websocket::shared::error::WebSocketError;
use crate::websocket::shared::message::WebSocketMessage;

/// Inbound pipe item: a decoded message, or a transport error.
type Inbound = Result<WebSocketMessage, WebSocketError>;

/// Depth of the bounded inbound/outbound pipes (mirrors [`MessageDelivery`]).
const PIPE_DEPTH: usize = 64;

/// Open a browser `WebSocket` and wrap it in the shared [`WebSocketClient`].
///
/// Honours [`WebSocketConnectConfig::subprotocols`]; other fields are best-effort
/// (see the module docs). Returns the client and its [`MessageDelivery`] handle.
///
/// # Errors
///
/// Returns [`WebSocketError`] if the browser rejects the URL or subprotocols.
pub fn connect(
    url: &str,
    config: &WebSocketConnectConfig,
) -> Result<(WebSocketClient, MessageDelivery), WebSocketError> {
    let ws = match config.subprotocols.as_deref() {
        Some(protocols) => WebSocket::new_with_str(url, protocols),
        None => WebSocket::new(url),
    }
    .map_err(|e| WebSocketError::ProtocolError(format!("failed to open WebSocket: {e:?}")))?;

    // Binary frames as ArrayBuffer so `on_message` can copy them synchronously.
    ws.set_binary_type(BinaryType::Arraybuffer);

    // Inbound: JS callbacks → pipe → the bridge iterator.
    let (inbound_tx, inbound_rx) = Pipe::<Inbound>::with_depth(PIPE_DEPTH);
    // Outbound: MessageDelivery → the bridge iterator → WebSocket.send.
    let (delivery, outbound_rx) = MessageDelivery::new();

    let open_flag = Rc::new(Cell::new(false));

    // on_open: mark the socket ready so queued sends can flush.
    let on_open = {
        let open_flag = open_flag.clone();
        Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_evt: Event| {
            open_flag.set(true);
        }))
    };

    // on_message: decode text / binary and push it inbound.
    let on_message = {
        let inbound_tx = inbound_tx.clone();
        Closure::<dyn FnMut(MessageEvent)>::wrap(Box::new(move |evt: MessageEvent| {
            let data = evt.data();
            let message = if let Some(text) = data.as_string() {
                WebSocketMessage::Text(text)
            } else {
                // binaryType = arraybuffer → data is an ArrayBuffer.
                let bytes = js_sys::Uint8Array::new(&data).to_vec();
                WebSocketMessage::Binary(bytes)
            };
            let _ = inbound_tx.try_send(Ok(message));
        }))
    };

    // on_error: surface a transport error, then let close end the stream.
    let on_error = {
        let inbound_tx = inbound_tx.clone();
        Closure::<dyn FnMut(Event)>::wrap(Box::new(move |_evt: Event| {
            let _ = inbound_tx.try_send(Err(WebSocketError::ProtocolError(
                "browser WebSocket error".to_string(),
            )));
        }))
    };

    // on_close: emit a Close message and close the pipe so the stream ends.
    let on_close = {
        let inbound_tx = inbound_tx.clone();
        Closure::<dyn FnMut(CloseEvent)>::wrap(Box::new(move |evt: CloseEvent| {
            let _ = inbound_tx.try_send(Ok(WebSocketMessage::Close(evt.code(), evt.reason())));
            inbound_tx.close();
        }))
    };

    ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));
    ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));

    let bridge = WasmWebSocketBridge {
        ws,
        _on_open: on_open,
        _on_message: on_message,
        _on_error: on_error,
        _on_close: on_close,
        inbound_rx,
        outbound_rx,
        open_flag,
        established_emitted: false,
        finished: false,
    };

    let client = WebSocketClient::new(Box::new(bridge), delivery.clone());
    Ok((client, delivery))
}

/// The iterator half of a browser `WebSocket`, yielding the shared
/// [`WebSocketStreamItem`]. Holds the socket and its JS callbacks alive.
struct WasmWebSocketBridge {
    ws: WebSocket,
    // Closures must outlive the socket — dropping them detaches the handlers.
    _on_open: Closure<dyn FnMut(Event)>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    _on_error: Closure<dyn FnMut(Event)>,
    _on_close: Closure<dyn FnMut(CloseEvent)>,
    inbound_rx: PipeReceiver<Inbound>,
    outbound_rx: PipeReceiver<WebSocketMessage>,
    open_flag: Rc<Cell<bool>>,
    established_emitted: bool,
    finished: bool,
}

// SAFETY: single-threaded wasm only — there are no other threads, so asserting
// `Send` for the JS handles held here is vacuously sound. Mirrors
// `foundation_compact::SendWrapper`'s own gate. On wasm+atomics this impl is
// absent and the `+ Send` bound on `BoxedWebSocketStream` fails to compile, as
// intended (the browser-WebSocket path targets single-threaded wasm).
#[cfg(all(
    target_family = "wasm",
    not(target_os = "emscripten"),
    not(target_feature = "atomics")
))]
unsafe impl Send for WasmWebSocketBridge {}

impl WasmWebSocketBridge {
    /// Push one outbound message to the browser socket.
    fn send_outbound(&self, message: WebSocketMessage) -> Result<(), WebSocketError> {
        let result = match message {
            // No wire frame — the browser has no explicit "established" signal.
            WebSocketMessage::ConnectionEstablished => return Ok(()),
            WebSocketMessage::Text(text) => self.ws.send_with_str(&text),
            WebSocketMessage::Binary(data) => self.ws.send_with_u8_array(&data),
            // The browser manages ping/pong itself; user-level sends are no-ops.
            WebSocketMessage::Ping(_) | WebSocketMessage::Pong(_) => return Ok(()),
            WebSocketMessage::Close(code, reason) => {
                return self
                    .ws
                    .close_with_code_and_reason(code, &reason)
                    .map_err(|e| {
                        WebSocketError::ProtocolError(format!("WebSocket close failed: {e:?}"))
                    });
            }
        };
        result
            .map_err(|e| WebSocketError::ProtocolError(format!("WebSocket send failed: {e:?}")))
    }
}

impl Iterator for WasmWebSocketBridge {
    type Item = WebSocketStreamItem;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        // Flush queued outbound messages once the socket is open.
        if self.open_flag.get() {
            while let Ok(message) = self.outbound_rx.try_recv() {
                if let Err(e) = self.send_outbound(message) {
                    self.finished = true;
                    return Some(Stream::Next(Err(e)));
                }
            }

            // Emit ConnectionEstablished once, mirroring the native task (the
            // client's message iterator skips it).
            if !self.established_emitted {
                self.established_emitted = true;
                return Some(Stream::Next(Ok(WebSocketMessage::ConnectionEstablished)));
            }
        }

        // Deliver the next inbound message, or report progress while we wait for
        // the JS event loop to hand us one.
        match self.inbound_rx.try_recv() {
            Ok(item) => Some(Stream::Next(item)),
            Err(TryRecvError::Empty) => {
                if self.open_flag.get() {
                    Some(Stream::Pending(WsProgress::Reading))
                } else {
                    Some(Stream::Pending(WsProgress::Connecting))
                }
            }
            Err(TryRecvError::Closed) => {
                self.finished = true;
                None
            }
        }
    }
}
