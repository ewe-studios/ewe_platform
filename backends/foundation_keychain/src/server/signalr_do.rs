//! SignalR Durable Object — real-time vault notifications (spec-57, F010 Stage 3).
//!
//! WHY: Bitwarden clients use SignalR over WebSocket for real-time sync. Each
//! user gets a dedicated DO instance (keyed by user UUID) that accepts WebSocket
//! connections from all their devices and broadcasts vault-change notifications.
//!
//! WHAT: [`SignalRHub`] implements `worker::DurableObject` on top of the general
//! machinery in `foundation_deployment_cloudflare::workers`:
//!   - `fetch` → `/ws` upgrade, `/notify` broadcast, `/health` check
//!   - `websocket_message` → SignalR handshake → MessagePack frames
//!   - `alarm` → periodic ping (every 15s)
//!   - `websocket_close/error` → lifecycle tracking
//!
//! HOW: Uses [`accept_websocket`] + [`do_connect`] from the deployment_cloudflare
//! workers module for transport, and [`crate::core::notifications`] for
//! MessagePack framing (VarInt + rmpv — portable, shared with native backend).

use std::cell::RefCell;

use foundation_deployment_cloudflare::workers::durable_object::{do_broadcast, do_connect};
use foundation_deployment_cloudflare::workers::websocket::{
    accept_websocket, close_socket, SignalrHandshaker,
};
use worker::{
    durable_object, DurableObject, Env, Request, Response, Result as WorkerResult, State,
    WebSocket, WebSocketIncomingMessage,
};

use crate::core::notifications::{self, Notification, UpdateType};

/// A SignalR MessagePack WebSocket hub backed by a Cloudflare Durable Object.
///
/// Each DO instance is keyed by user UUID in `wrangler.toml`:
/// ```toml
/// [[durable_objects.bindings]]
/// name = "SIGNALR_HUB"
/// class_name = "SignalRHub"
/// ```
///
/// All WebSocket connections for a single user land on the same DO, so
/// broadcasting to all of a user's devices is a local operation.
#[durable_object]
pub struct SignalRHub {
    /// Per-connection handshake state (one DO instance = one user, many
    /// connections — we track handshake state in the message handler).
    handshaker: RefCell<SignalrHandshaker>,
    /// Total messages received (for health metrics).
    messages: RefCell<u64>,
    /// Durable Object state (storage, WebSocket accept, alarm).
    state: State,
    /// Worker env bindings (unused — D1/KV accessed via the fetch handler).
    #[allow(dead_code)]
    env: Env,
}

impl DurableObject for SignalRHub {
    fn new(state: State, env: Env) -> Self {
        Self {
            handshaker: RefCell::new(SignalrHandshaker::new()),
            messages: RefCell::new(0),
            state,
            env,
        }
    }

    /// Entry point — WebSocket upgrade, notification dispatch, or health check.
    async fn fetch(&self, mut req: Request) -> WorkerResult<Response> {
        let path = req.path();
        let method = req.method().to_string().to_uppercase();

        match (method.as_str(), path.as_str()) {
            // WebSocket upgrade — standard SignalR connect endpoint.
            ("GET", "/ws") => {
                let (server, mut resp) = accept_websocket()?;
                // Accept on the DO state so websocket_message events fire.
                do_connect(&self.state, &server, &[])?;
                Ok(resp)
            }

            // Notification dispatch — called by the API handler to push an
            // event to all connected WebSockets for this user.
            ("POST", "/notify") => {
                let body = req.text().await.unwrap_or_default();
                match serde_json::from_str::<(Option<serde_json::Value>, i32)>(&body) {
                    Ok((payload, update_type)) => {
                        let notif = Notification::new(
                            match update_type {
                                1 => UpdateType::SyncCipherUpdate,
                                2 => UpdateType::SyncCipherCreate,
                                3 => UpdateType::SyncLoginDelete,
                                4 => UpdateType::SyncFolderDelete,
                                5 => UpdateType::SyncCiphers,
                                6 => UpdateType::SyncVault,
                                7 => UpdateType::SyncOrgKeys,
                                8 => UpdateType::SyncFolderCreate,
                                9 => UpdateType::SyncFolderUpdate,
                                10 => UpdateType::SyncCipherDelete,
                                11 => UpdateType::SyncSettings,
                                12 => UpdateType::SyncLogOut,
                                14 => UpdateType::SyncSendUpdate,
                                15 => UpdateType::SyncSendDelete,
                                _ => UpdateType::SyncVault,
                            },
                            payload,
                        );
                        self.broadcast(&notif);
                    }
                    Err(_) => {
                        return Response::error("invalid notification payload".to_string(), 400);
                    }
                }
                Response::ok("ok")
            }

            // Health ping — used by the fetch handler to check DO liveness.
            ("GET", "/health") => {
                let msgs = *self.messages.borrow();
                Response::from_json(&serde_json::json!({
                    "messages": msgs,
                    "sockets": foundation_deployment_cloudflare::workers::durable_object::connected_count(&self.state, None),
                }))
            }

            _ => Response::error("not found".to_string(), 404),
        }
    }

    /// Handle incoming WebSocket messages — SignalR handshake first, then
    /// MessagePack frames.
    async fn websocket_message(
        &self,
        ws: WebSocket,
        message: WebSocketIncomingMessage,
    ) -> WorkerResult<()> {
        match message {
            WebSocketIncomingMessage::String(text) => {
                let mut h = self.handshaker.borrow_mut();
                h.process(&ws, &text)?;
            }
            WebSocketIncomingMessage::Binary(data) => {
                if !self.handshaker.borrow().is_connected() {
                    return Ok(()); // ignore until handshake complete
                }

                // Decode VarInt + MessagePack frame.
                if let Some((msg_type, _payload)) = decode_signalr_frame(&data) {
                    if msg_type == 6 {
                        // Ping — respond with MessagePack [6] (encoded as
                        // `\x91\x06` — fixarray of 1 element, value 6).
                        let _ = ws.send_with_bytes(vec![0x91, 0x06]);
                    }
                    *self.messages.borrow_mut() += 1;
                }
            }
        }
        Ok(())
    }

    async fn websocket_close(
        &self,
        ws: WebSocket,
        code: usize,
        reason: String,
        was_clean: bool,
    ) -> WorkerResult<()> {
        let _ = (code, reason, was_clean);
        close_socket(&ws, None, None);
        Ok(())
    }

    async fn websocket_error(&self, _ws: WebSocket, _error: worker::Error) -> WorkerResult<()> {
        Ok(())
    }

    /// Periodic alarm — ping all connected sockets (keep-alive).
    async fn alarm(&self) -> WorkerResult<Response> {
        // MessagePack `[6]` = `\x91\x06` (fixarray[1], uint 6)
        do_broadcast(&self.state, &[0x91, 0x06]);
        Response::ok("pinged")
    }
}

impl SignalRHub {
    /// Broadcast a [`Notification`] to every connected WebSocket as a
    /// SignalR invocation frame.
    fn broadcast(&self, notif: &Notification) {
        let frame = match encode_signalr_notification(notif) {
            Ok(f) => f,
            Err(_) => return,
        };
        do_broadcast(&self.state, &frame);
    }
}

// ── SignalR frame helpers ────────────────────────────────────────────────

/// Decode a VarInt (up to 5 bytes) from the start of a byte slice.
fn decode_varint(data: &[u8]) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift = 0;
    for (i, &byte) in data.iter().enumerate().take(5) {
        value |= u64::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            return Some((value, i + 1));
        }
        shift += 7;
        if shift >= 35 {
            return None;
        }
    }
    None
}

/// Encode a u32 as a VarInt byte vector.
fn encode_varint(mut value: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5);
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
    buf
}

/// Decode a SignalR MessagePack frame: `(message_type, payload_bytes)`.
fn decode_signalr_frame(data: &[u8]) -> Option<(u8, &[u8])> {
    let (len, consumed) = decode_varint(data)?;
    let payload = data.get(consumed..consumed + len as usize)?;
    // Payload is a MessagePack array `[msg_type, ..]`. First element is the
    // hub invocation message type.
    let msg_type: u8 = rmp_serde::from_slice(payload).ok()?;
    Some((msg_type, payload))
}

/// Encode a [`Notification`] as a complete SignalR WebSocket binary frame:
/// VarInt length prefix + `[1, {Target: "...", Arguments: [payload_json]}]`.
fn encode_signalr_notification(notif: &Notification) -> Result<Vec<u8>, String> {
    let payload_json = notif
        .payload
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok())
        .unwrap_or_default();

    let target = match notif.update_type {
        UpdateType::SyncCipherCreate => "SyncCipherCreate",
        UpdateType::SyncCipherUpdate => "SyncCipherUpdate",
        UpdateType::SyncLoginDelete => "SyncLoginDelete",
        UpdateType::SyncFolderDelete => "SyncFolderDelete",
        UpdateType::SyncCiphers => "SyncCiphers",
        UpdateType::SyncVault => "SyncVault",
        UpdateType::SyncOrgKeys => "SyncOrgKeys",
        UpdateType::SyncFolderCreate => "SyncFolderCreate",
        UpdateType::SyncFolderUpdate => "SyncFolderUpdate",
        UpdateType::SyncCipherDelete => "SyncCipherDelete",
        UpdateType::SyncSettings => "SyncSettings",
        UpdateType::SyncLogOut => "SyncLogOut",
        UpdateType::SyncSendCreate => "SyncSendCreate",
        UpdateType::SyncSendUpdate => "SyncSendUpdate",
        UpdateType::SyncSendDelete => "SyncSendDelete",
        UpdateType::AuthRequest => "AuthRequest",
        UpdateType::AuthRequestResponse => "AuthRequestResponse",
    };

    // SignalR hub invocation: `[1, {Target: "...", Arguments: [...]}]`
    let invocation = rmp_serde::encode::to_vec_named(&(
        1u8,
        serde_json::json!({
            "Target": target,
            "Arguments": [payload_json],
        }),
    ))
    .map_err(|e| e.to_string())?;

    let mut frame = encode_varint(invocation.len() as u32);
    frame.extend_from_slice(&invocation);
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trip() {
        for val in [0u32, 1, 127, 128, 16383, 16384, 2097151, 268435456] {
            let enc = encode_varint(val);
            let (dec, n) = decode_varint(&enc).expect("decode varint");
            assert_eq!(dec, val as u64, "varint round-trip fail at {val}");
            assert_eq!(n, enc.len());
        }
    }

    #[test]
    fn notification_frame_is_valid() {
        let notif = Notification::new(
            UpdateType::SyncCipherCreate,
            Some(serde_json::json!({"id": "test-123"})),
        );
        let frame = encode_signalr_notification(&notif).expect("encode");
        let (msg_type, _payload) = decode_signalr_frame(&frame).expect("decode");
        assert_eq!(msg_type, 1); // Invocation
    }
}
